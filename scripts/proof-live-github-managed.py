"""Private orchestration for proof-live-github-managed.sh; no external credentials.

HTTP is always sent to explicitly assigned loopback peers. The Rust relay alone
can reach GitHub, and only in the owner's live invocation. Bodies and subprocess
output remain in memory; diagnostic failures contain only the current step.
"""
import base64
import html
from html.parser import HTMLParser
import http.client
import json
import os
from pathlib import Path
import queue
import re
import signal
import shutil
import struct
import subprocess
import sys
import threading
import time
from urllib.parse import urlencode, urlsplit

OWNER = 'freeekanayaka'
NAME = 'temper-probe-2'
REPOSITORY = OWNER + '/' + NAME
BASE = '/repos/' + REPOSITORY
BOUND = 16 * 1024 * 1024
API_VERSION = '2026-03-10'  # Pinned skein-forge github/pin.rs, as in temper's client.


def require(condition):
    if not condition:
        raise RuntimeError('sitting condition failed')


class InstallationRefused(RuntimeError):
    """Only the installation-list request received the relay's refusal status."""


def private(path, data):
    with open(os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600), 'wb') as output:
        output.write(data)


class Process:
    """Drain both pipes without persisting raw output or exposing it on failure."""
    def __init__(self, argv, cwd, env, control=False):
        self.proc = subprocess.Popen(argv, cwd=cwd, env=env,
                                     stdin=subprocess.PIPE if control else subprocess.DEVNULL,
                                     stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.lines = queue.Queue(maxsize=4096)
        self.overflow = False
        self.diagnostics = set()
        self.threads = []
        for pipe in (self.proc.stdout, self.proc.stderr):
            def read(pipe=pipe):
                for line in iter(pipe.readline, b''):
                    try:
                        self.lines.put_nowait(line[:65536])
                    except queue.Full:
                        self.overflow = True
                pipe.close()
            thread = threading.Thread(target=read, daemon=True)
            thread.start()
            self.threads.append(thread)

    def line(self, prefix, timeout=30):
        until = time.monotonic() + timeout
        while time.monotonic() < until:
            require(not self.overflow)
            try:
                line = self.lines.get(timeout=0.1).decode()
            except queue.Empty:
                require(self.proc.poll() is None)
                continue
            self.diagnostics.update(re.findall(r'\b(?:engine|application|workers)(?:\.[a-z_]+|\[\d+\])+', line))
            for category in ('No such file', 'Permission denied', 'Operation not permitted', 'Invalid argument', 'engine alone', 'standalone'):
                if category in line:
                    self.diagnostics.add(category)
            if line.startswith(prefix):
                return line.strip()[len(prefix):]
        raise RuntimeError('process readiness timed out')

    def stop(self):
        if self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait(timeout=5)
        for thread in self.threads:
            thread.join(timeout=1)


class Form(HTMLParser):
    def __init__(self, body):
        super().__init__(convert_charrefs=True)
        self.action = None
        self.manifest = None
        self.create = None
        self.feed(body.decode())

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == 'form':
            require(self.action is None)
            self.action = attrs['action']
        if tag == 'input' and attrs.get('name') == 'manifest':
            self.manifest = attrs['value']
        if tag == 'a' and attrs.get('id') == 'create':
            self.create = attrs['href']


def crc32c(data):
    crc = 0xffffffff
    for byte in data:
        crc ^= byte
        for _ in range(8):
            crc = (crc >> 1) ^ (0x82f63b78 if crc & 1 else 0)
    return crc ^ 0xffffffff


def app_records(directory):
    """Read this fresh deployment's checksummed KV records, never write the store.

    skein-kv snapshot.rs/frame.rs and jig-protocol-store secrets.rs define these
    version-one formats. Refuse an incomplete frame, snapshot or changed format.
    Only the App ID and key application records are returned, in memory.
    """
    def u32(data, at):
        return struct.unpack_from('>I', data, at)[0]

    def checksum(data):
        require(len(data) >= 4 and u32(data, len(data)-4) == crc32c(data[:-4]))

    rows = {}
    start = 1
    snapshot = directory / 'snapshot'
    if snapshot.exists():
        data = snapshot.read_bytes()
        require(len(data) <= BOUND and data[:6] == b'SKVS\0\1')
        checksum(data[:18])
        start = int.from_bytes(data[6:14], 'big')
        at, count, payload = 18, 0, 0
        while data[at:at+4] != b'SNPE':
            n, size = u32(data, at), u32(data, at+4)
            end = at + size + 12
            checksum(data[at:end])
            cursor = at + 8
            for _ in range(n):
                kl, vl = u32(data, cursor), u32(data, cursor+4)
                cursor += 8
                key = data[cursor:cursor+kl]
                cursor += kl
                value = data[cursor:cursor+vl]
                cursor += vl
                require(len(key) == kl and len(value) == vl and key not in rows)
                rows[key] = value
                count += 1
                payload += kl + vl
            require(cursor == end-4)
            at = end
        require(at+28 == len(data) and data[-4:] == b'END!')
        checksum(data[at:-4])
        require(int.from_bytes(data[at+4:at+12], 'big') == count)
        require(int.from_bytes(data[at+12:at+20], 'big') == payload)
    last = start - 1
    for log in sorted(directory.glob('log-[0-9]' + '[0-9]' * 19)):
        data = log.read_bytes()
        require(len(data) <= BOUND)
        at = 0
        while at < len(data):
            require(data[at:at+6] == b'SKVC\0\1')
            number = int.from_bytes(data[at+6:at+14], 'big')
            n, size = u32(data, at+14), u32(data, at+18)
            end = at + 26 + size
            require(end <= len(data))
            checksum(data[at:end])
            cursor = at + 22
            operations = []
            for _ in range(n):
                kind = data[cursor]
                kl = u32(data, cursor+1)
                cursor += 5
                key = data[cursor:cursor+kl]
                cursor += kl
                require(kind in (1, 2))
                value = None
                if kind == 1:
                    vl = u32(data, cursor)
                    cursor += 4
                    value = data[cursor:cursor+vl]
                    cursor += vl
                    require(len(value) == vl)
                operations.append((key, value))
            require(cursor == end-4)
            if number >= start:
                require(number == last+1)
                for key, value in operations:
                    if value is None:
                        rows.pop(key, None)
                    else:
                        rows[key] = value
                last = number
            at = end

    def get(item):
        key = b'\2\0' + ((1 << 48) | (1 << 16) | item).to_bytes(8, 'big')
        value = rows[key]
        require(value[:1] == b'\1')
        return value[1:]
    return int.from_bytes(get(6), 'little'), get(8)


class Sitting:
    def __init__(self, mode, port, scratch, evidence):
        # Fail before starting a peer or creating an App if signing support is absent.
        from cryptography.hazmat.primitives.serialization import load_der_private_key
        require(callable(load_der_private_key))
        require(mode in ('fake', 'fake-wide', 'live'))
        self.wide = mode == 'fake-wide'
        self.mode = 'fake' if self.wide else mode
        self.wait_secs = int(os.environ.get('TEMPER_SITTING_WAIT_SECS', '0')) if self.mode == 'fake' else 0
        require(self.wait_secs >= 0)
        self.port = int(port)
        self.root = Path(__file__).resolve().parent.parent
        self.scratch = Path(scratch).resolve(strict=True)
        self.evidence = Path(evidence).resolve()
        require(not self.evidence.is_relative_to(self.scratch))
        self.run = time.strftime('%Y%m%dT%H%M%S', time.gmtime()) + os.urandom(2).hex()
        self.prefix = 'temper/live-' + self.run + '/'
        self.branch = self.prefix + 'managed'
        self.processes = []
        self.step = 'startup'
        self.pull = None
        self.pushed = False
        self.token = None
        self.api = None
        self.jwt_key = None
        self.app_id = None
        self.secrets = {'App_key_DER': [], 'App_key_base64': [], 'JWT': [], 'installation_token': []}
        self.relay = None
        self.scanned = False
        self.retained = False
        self.last_request = None
        self.binary = Path(os.environ['CARGO_TARGET_DIR']) / 'debug/temper'
        artifacts = [json.loads(line) for line in (self.scratch/'build.jsonl').read_text().splitlines()
                     if line.startswith('{')]
        self.harness = next(row['executable'] for row in artifacts
                            if row.get('reason') == 'compiler-artifact'
                            and row.get('target', {}).get('name') == 'live_github_managed'
                            and row.get('executable'))
        for attribute, name in (('binary', 'temper'), ('harness', 'managed-relay')):
            destination = self.scratch/name
            shutil.copyfile(getattr(self, attribute), destination)
            destination.chmod(0o700)
            setattr(self, attribute, str(destination))
        # No git configuration, credential helper, hooks, proxy or user HOME is read.
        self.env = {'PATH': os.environ['PATH'], 'HOME': str(self.scratch),
                    'TMPDIR': str(self.scratch), 'LANG': 'C.UTF-8',
                    'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': '/dev/null',
                    'GIT_TERMINAL_PROMPT': '0'}
        self.allowed = set()
        # The wrapper's cwd ownership rule includes the orchestrator on interruption.
        os.chdir(self.scratch)

    def mark(self, message):
        self.step = 'printing one-time sign-in' if message.startswith('one-time sign-in:') else message
        print(message, flush=True)

    def start(self, argv, name, extra=None):
        self.step = 'starting ' + name
        cwd = self.scratch/name
        cwd.mkdir(mode=0o700)
        proc = Process(argv, cwd, self.env | (extra or {}), control=name == 'relay')
        self.processes.append(proc)
        return proc

    def command(self, args, data=None, cwd=None, extra_env=None, ready=False):
        # The person's Enter belongs to this process, never to a ctl or git child.
        result = subprocess.run(args, input=b'' if data is None else data, cwd=cwd or self.scratch,
                                env=self.env | (extra_env or {}), stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, timeout=60)
        if ready and result.returncode != 0:
            # Only our fixed diagnostic vocabulary escapes the private client output.
            failed = re.search(rb'Ready failed at semantic step (\d+)', result.stderr)
            if failed:
                self.step += ' at semantic step ' + failed[1].decode()
        require(result.returncode == 0 and len(result.stdout) <= BOUND)
        return result.stdout

    def ctl(self, *words, data=None):
        self.step = 'ctl ' + words[0]
        return self.command([str(self.binary), 'ctl', str(self.scratch/'client'), *words], data)

    def http(self, address, method, target, body=b'', authorization=None, media='application/json'):
        self.step = 'HTTP ' + method + ' ' + target.split('?')[0]
        self.last_request = (address, method, target.split('?')[0])
        require(address in self.allowed and target.startswith('/') and not target.startswith('//'))
        conn = http.client.HTTPConnection(address, timeout=30)
        headers = {'User-Agent': 'temper-managed-sitting', 'Content-Type': media,
                   'Accept': 'application/vnd.github+json', 'X-GitHub-Api-Version': API_VERSION}
        if authorization:
            headers['Authorization'] = authorization
        try:
            conn.request(method, target, body, headers)
            response = conn.getresponse()
            self.last_status = response.status
            data = response.read(BOUND+1)
            require(len(data) <= BOUND)
            return response.status, dict(response.getheaders()), data
        finally:
            conn.close()

    def page(self, method, url, body=b''):
        parts = urlsplit(html.unescape(url))
        require(self.mode == 'fake' and parts.scheme == 'http'
                and parts.netloc in self.allowed and not parts.fragment)
        return self.http(parts.netloc, method, parts.path + ('?' + parts.query if parts.query else ''),
                         body, media='application/x-www-form-urlencoded')

    def jwt(self):
        from cryptography.hazmat.primitives import hashes
        from cryptography.hazmat.primitives.asymmetric import padding
        encode = lambda data: base64.urlsafe_b64encode(data).rstrip(b'=')
        now = int(time.time())
        claims = json.dumps({'iat': now-30, 'exp': now+300, 'iss': str(self.app_id)}, separators=(',', ':')).encode()
        message = encode(b'{"alg":"RS256","typ":"JWT"}') + b'.' + encode(claims)
        token = (message + b'.' + encode(self.jwt_key.sign(message, padding.PKCS1v15(), hashes.SHA256()))).decode()
        self.secrets['JWT'].append(token)
        return 'Bearer ' + token

    def ask(self, method, path, body=None, app=False, expected=200):
        status, _, data = self.http(self.api, method, path,
                                   b'' if body is None else json.dumps(body).encode(),
                                   self.jwt() if app else 'Bearer ' + self.token)
        if status == 403 and method == 'GET' and path.split('?')[0] == '/installation/repositories':
            raise InstallationRefused('installation repositories refused')
        require(status in (expected if isinstance(expected, tuple) else (expected,)))
        return json.loads(data) if data else None

    def fake(self):
        document = {'listen': {'socket': '127.0.0.1:0'}, 'admin_token': str(self.scratch/'fake-admin'),
                    'owner': OWNER,
                    'repositories': [{'owner': OWNER, 'name': NAME, 'landing': 'main', 'ci': None,
                                      'id': 1},
                                     {'owner': OWNER, 'name': 'temper-probe', 'landing': 'main',
                                      'ci': None, 'id': 2}],
                    'github': {'app': {'id': 4242, 'slug': 'temper-test', 'bot': 4243, 'key': 0},
                               'accounts': [{'login': OWNER, 'id': 10, 'kind': 'user', 'installation': None},
                                            {'login': 'managed-other', 'id': 20, 'kind': 'user',
                                             'installation': {'id': 1, 'repositories': 'all'}}],
                               'token_life_seconds': 3600}}
        path = self.scratch/'fake.json'
        private(path, json.dumps(document).encode())
        fake = self.start([str(self.binary), 'fake', 'forge', '--document', str(path), '--ready', 'listening '], 'forge')
        address = fake.line('listening ')
        require(re.fullmatch(r'127\.0\.0\.1:\d+', address))
        self.allowed.add(address)
        self.fake_address = address
        return address

    def configure(self, git, web):
        template = (self.root/'crates/temper/tests/support/github/engine.json').read_text()
        for name, value in {'proc.forge.ready.address': self.api, 'proc.llm.ready.address': '127.0.0.1:9',
                            'story.clients': '127.0.0.1:' + str(self.port)}.items():
            template = template.replace('{' + name + '}', value)
        value = json.loads(template.replace('{{', '{').replace('}}', '}').replace('"acme"', json.dumps(OWNER)))
        # Ready makes a project with policy 1, as a fresh temper init deployment does.
        value['engine']['policies'] = value['engine']['projects'].copy()
        # Deployment roster, seeded project 1, and Ready's new project.
        value['engine']['limits']['overrides']['people.projects'] = 3
        value['application']['forges'][0].update({'git': 'http://' + git, 'web': web})
        if self.mode == 'live':
            value['application']['clients']['bundle'] = str(self.scratch/'bundle')
        digests = self.scratch/'data/secrets'
        digests.mkdir(parents=True, mode=0o700)
        private(digests/'w1.digest', (self.root/'crates/temper/tests/support/github/w1.digest').read_bytes())
        directory = self.scratch/'config'
        (directory/'charters').mkdir(parents=True, mode=0o700)
        private(directory/'charters/chat.txt', b'No model calls in this sitting.\n')
        private(directory/'temper.json', json.dumps(value).encode())
        server = self.start([str(self.binary), 'serve', '--config', str(directory),
                             '--data', str(self.scratch/'data'), '--cache', str(self.scratch/'cache')], 'engine')
        self.engine = server
        link = 'http://' + server.line('http://')
        parts = urlsplit(link)
        require(parts.netloc == '127.0.0.1:' + str(self.port))
        self.allowed.add(parts.netloc)
        return link

    def connect_fake(self, link):
        self.step = 'fake sign-in and Connect'
        self.ready_client(link, 'sign-in')
        # The native shell keeps the session; ctl also needs the public engine address.
        private(self.scratch/'client/address', ('http://' + urlsplit(link).netloc + '\n').encode())
        output = self.ctl('forge', 'connect', '1', 'github', '--key', 'managed-connect').decode()
        url = next(line for line in output.splitlines() if line.startswith('http://'))
        status, _, body = self.page('GET', url)
        require(status == 200)
        form = Form(body)
        manifest = json.loads(form.manifest)
        require(manifest['redirect_url'].startswith('http://127.0.0.1:' + str(self.port) + '/'))
        require(manifest['setup_url'].startswith('http://127.0.0.1:' + str(self.port) + '/'))
        status, _, body = self.page('POST', form.action, urlencode({'manifest': form.manifest}).encode())
        require(status == 200)
        create = Form(body).create
        status, headers, _ = self.page('GET', create)
        require(status == 302)
        selected = '1,2' if self.wide else '1'
        status, headers, _ = self.page('GET', headers['Location'] + '&account=' + OWNER + '&repository_ids=' + selected)
        require(status == 302)
        status, _, body = self.page('GET', headers['Location'])
        require(status == 200 and b'temper has the installation' in body)
        self.ctl('forge', 'list', '--until', 'settled')
        self.mark('Connect, manifest creation and selected installation completed')

    def ready_client(self, link, mode):
        self.step = 'signed-in Ready ' + mode
        self.command([self.harness, '--ignored', '--exact', 'managed_ready_client', '--nocapture'],
                     data=(link + '\n').encode(), extra_env={'TEMPER_MANAGED_CLIENT': mode}, ready=True)

    def wait_ready(self):
        from cryptography.hazmat.primitives.serialization import load_der_private_key
        until = time.monotonic() + (1200 if self.mode == 'live' else 30)
        while time.monotonic() < until:
            try:
                app_id, key = app_records(self.scratch/'data/state/secrets')
                encoded_key = base64.b64encode(key).decode()
                if encoded_key not in self.secrets['App_key_DER']:
                    self.secrets['App_key_DER'].append(encoded_key)
                    self.secrets['App_key_base64'].append(encoded_key)
                self.jwt_key = load_der_private_key(key, password=None)
                self.app_id = app_id
                installations = self.ask('GET', '/app/installations?per_page=100&page=1', app=True)
                if not installations:
                    # Twenty minutes of human clicks must fit the relay's request cap.
                    time.sleep(5)
                    continue
                if self.mode == 'fake':
                    other = [row for row in installations if row['account']['login'] == 'managed-other']
                    require(len(other) == 1 and other[0]['id'] == 1)
                    installations = [row for row in installations if row['account']['login'] == OWNER]
                require(len(installations) == 1 and installations[0]['account']['login'] == OWNER)
                installation = installations[0]['id']
                row = self.ask('GET', '/app/installations/' + str(installation), app=True)
                require(row['account']['login'] == OWNER)
                self.token = self.ask('POST', '/app/installations/' + str(installation) + '/access_tokens', app=True, expected=201)['token']
                self.secrets['installation_token'].append(self.token)
                repos = self.ask('GET', '/installation/repositories?per_page=100&page=1')
                require(repos['total_count'] == 1 and len(repos['repositories']) == 1
                        and repos['repositories'][0]['owner']['login'] == OWNER
                        and repos['repositories'][0]['name'] == NAME
                        and repos['repositories'][0].get('full_name', REPOSITORY) == REPOSITORY)
                return
            except (FileNotFoundError, KeyError):
                time.sleep(0.25)
        raise RuntimeError('installation readiness timed out')

    def push_fake(self):
        # skein's GitHub fake speaks its typed private git protocol, as in the
        # real-loop/process tiers; it does not implement smart HTTP packfiles.
        auth = 'Basic ' + base64.b64encode(('x-access-token:' + self.token).encode()).decode()
        def git(name, body):
            status, _, data = self.http(self.git, 'POST', '/.fake/git/' + name,
                                        json.dumps(body).encode(), auth)
            require(status == 200)
            return json.loads(data)
        cloned = git('clone', {'repository': REPOSITORY})
        parent = next(row['commit'] for row in cloned['branches'] if row['branch'] == 'main')
        objects = git('objects', {'commits': [parent]})
        tree = next(row['tree'] for row in objects['objects'] if row['commit'] == parent)
        encode = lambda text: base64.b64encode(text.encode()).decode()
        tree.append({'path': encode('probe/' + self.run + '.md'),
                     'content': encode('temper managed live run ' + self.run + '\n')})
        commit = git('commit', {'parent': parent, 'tree': tree,
                               'message': encode('Exercise the managed installation')})['commit']
        self.pushed = True
        require(git('create', {'repository': REPOSITORY, 'branch': self.branch,
                               'commit': parent})['result'] == 'created')
        require(git('push', {'repository': REPOSITORY, 'branch': self.branch,
                             'commit': commit, 'expected': parent})['result'] == 'pushed')
        self.fake_parent, self.fake_commit = parent, commit

    def planted_refusals(self):
        require(self.mode == 'fake' and self.pull is not None)
        basic = 'Basic ' + base64.b64encode(('x-access-token:' + self.token).encode()).decode()
        cases = [
            ('another installation token', self.api, 'POST', '/app/installations/1/access_tokens',
             {}, self.jwt(), '/app/installations/1/access_tokens'),
            ('another repository read', self.api, 'GET', '/repos/' + OWNER + '/temper-probe?planted=1',
             None, 'Bearer ' + self.token, '/repos/' + OWNER + '/temper-probe'),
            ('push outside the run prefix', self.git, 'POST', '/.fake/git/push',
             {'repository': REPOSITORY, 'branch': 'outside-run', 'commit': self.fake_commit,
              'expected': self.fake_parent}, basic, '/.fake/git/push'),
            ('second conversion', self.api, 'POST', '/app-manifests/plantedsecond/conversions',
             {}, None, '/app-manifests/[code]/conversions'),
        ]
        journal = self.scratch/'relay.jsonl'
        for label, address, method, target, body, auth, path in cases:
            before = len(journal.read_bytes().splitlines())
            status, _, _ = self.http(address, method, target,
                                     b'' if body is None else json.dumps(body).encode(), auth)
            require(status == 403)
            rows = [json.loads(line) for line in journal.read_bytes().splitlines()[before:]]
            require(any(row.get('method') == method and row.get('path') == path
                        and row.get('status') == 403 and row.get('refused') is True
                        and row.get('upstream_request_bytes') == 0 for row in rows))
            self.mark('relay refused and logged: ' + label)

    def scan_secrets(self):
        if self.relay is None or self.scanned:
            return
        # Stop temper before the relay drains its workers and scans both files.
        for process in reversed(self.processes):
            if process is not self.relay:
                process.stop()
        require(self.relay.proc.poll() is None)
        self.relay.proc.stdin.write(json.dumps(self.secrets).encode() + b'\n')
        self.relay.proc.stdin.close()
        try:
            status = self.relay.proc.wait(timeout=45)
        except subprocess.TimeoutExpired:
            self.relay.stop()
            raise RuntimeError('relay shutdown timed out') from None
        for thread in self.relay.threads:
            thread.join(timeout=1)
        if status != 0:
            # Only the fixed field label may leave the relay's private pipe.
            while not self.relay.lines.empty():
                line = self.relay.lines.get_nowait().decode(errors='replace').strip()
                if re.fullmatch(r'secret bytes found: (pem|webhook_secret|client_secret|client_id|App_key_DER|App_key_base64|JWT|installation_token)', line):
                    print(line, file=sys.stderr, flush=True)
            require(False)
        self.relay.line('no secret bytes in the relay log or the record')
        self.scanned = True
        self.mark('no secret bytes in the relay log or the record')

    def journal_rows(self):
        journal = self.scratch/'relay.jsonl'
        return [json.loads(line) for line in journal.read_bytes().splitlines()] if journal.exists() else []

    def report_refusal(self):
        rows = [row for row in reversed(self.journal_rows()) if row.get('refused')]
        row = rows[0] if rows else None
        if self.last_request is not None:
            address, method, path = self.last_request
            if path.startswith('/app-manifests/'):
                path = '/app-manifests/[code]/conversions'
            if address in (self.api, getattr(self, 'git', None)):
                row = next((entry for entry in rows if entry.get('method') == method
                            and entry.get('path') == path), row)
        if row is not None:
            # Only fixed labels and a numeric status can reach the terminal.
            reason, status = row.get('reason'), row.get('upstream_status')
            require(reason in (None, 'guard', 'redirect') or
                    isinstance(reason, str) and re.fullmatch(r'learn:[a-z0-9-]+', reason))
            require(status is None or type(status) is int and 100 <= status <= 599)
            print('relay refusal: reason=' + (reason or 'null') + ' upstream_status=' +
                  ('null' if status is None else str(status)), file=sys.stderr, flush=True)
        return row

    def retain_journal(self):
        if self.relay is None or self.retained:
            return
        require(self.scanned)
        destination = self.evidence.parent/('m3-relay-' + self.run + '.jsonl')
        private(destination, (self.scratch/'relay.jsonl').read_bytes())
        require(destination.stat().st_mode & 0o777 == 0o600)
        self.retained = True
        self.mark('relay journal retained: ' + str(destination))

    def check_wide_refusal(self):
        row = self.report_refusal()
        require(row is not None and row['path'] == '/installation/repositories'
                and row['reason'] == 'learn:installation-repositories-not-exactly-probe-2'
                and row['status'] == 403 and row['upstream_status'] == 200
                and row['rows'] == 2 and row['total_count'] == 2)
        self.engine.stop()
        # Read both fake repositories directly: the guard correctly refuses access.
        # A fresh fake starts with main alone and no pull requests in either repo.
        basic = 'Basic ' + base64.b64encode(('x-access-token:' + self.token).encode()).decode()
        for name in (NAME, 'temper-probe'):
            repository = OWNER + '/' + name
            status, _, data = self.http(self.fake_address, 'POST', '/.fake/git/clone',
                                        json.dumps({'repository': repository}).encode(), basic)
            require(status == 200)
            branches = json.loads(data)['branches']
            require(len(branches) == 1 and branches[0]['branch'] == 'main')
            # GitHub's issues listing includes pull requests of every state.
            status, _, data = self.http(self.fake_address, 'GET', '/repos/' + repository +
                                        '/issues?state=all&sort=updated&direction=asc&per_page=100&page=1',
                                        authorization='Bearer ' + self.token)
            require(status == 200 and json.loads(data) == [])
        self.scan_secrets()  # Drain the relay before checking the complete write history.
        rows = self.journal_rows()
        require(not any(row.get('listener') == 'git' or
                        row.get('method') in ('POST', 'PATCH', 'PUT', 'DELETE')
                        and row.get('path', '').startswith(('/repos/', '/repositories/'))
                        for row in rows))
        self.mark('fake-wide refused the installation; nothing written to either repository')
        self.retain_journal()
        self.mark('fake-wide expected refusal completed')

    def push(self):
        if self.mode == 'fake':
            self.push_fake()
            self.open_pull()
            return
        # The App key stays in its store. Git and its HTTP child inherit the
        # installation token through transient config variables, never argv or disk.
        authorization = base64.b64encode(('x-access-token:' + self.token).encode()).decode()
        config = {'GIT_CONFIG_COUNT': '1', 'GIT_CONFIG_KEY_0': 'http.extraHeader',
                  'GIT_CONFIG_VALUE_0': 'Authorization: Basic ' + authorization}
        url = 'http://' + self.git + '/' + REPOSITORY + '.git'
        checkout = self.scratch/'checkout'
        self.command(['git', '-c', 'credential.helper=', '-c', 'core.hooksPath=/dev/null', '-c', 'http.followRedirects=false',
                      'clone', '--quiet', '--single-branch', '--branch', 'main',
                      url, str(checkout)], extra_env=config)
        self.command(['git', 'checkout', '-b', self.branch], cwd=checkout)
        probe = checkout/'probe'
        probe.mkdir(exist_ok=True)
        (probe/(self.run + '.md')).write_text('temper managed live run ' + self.run + '\n')
        self.command(['git', 'add', '--', 'probe/' + self.run + '.md'], cwd=checkout)
        self.command(['git', '-c', 'user.name=temper managed sitting', '-c', 'user.email=temper@localhost',
                      '-c', 'core.hooksPath=/dev/null', 'commit', '-m', 'Exercise the managed installation'], cwd=checkout)
        self.pushed = True # Clean even if the push's response is interrupted.
        self.command(['git', '-c', 'credential.helper=', '-c', 'core.hooksPath=/dev/null', '-c', 'http.followRedirects=false',
                      'push', 'origin', 'HEAD:refs/heads/' + self.branch], cwd=checkout, extra_env=config)
        self.open_pull()

    def open_pull(self):
        self.mark('branch pushed with the installation token: ' + self.branch)
        pull = self.ask('POST', BASE + '/pulls', {'title': 'temper managed sitting ' + self.run,
                       'head': self.branch, 'base': 'main', 'body': 'Temporary managed installation proof.'}, expected=201)
        self.pull = pull['number']
        self.mark('pull request opened with the installation token')

    def cleanup_remote(self):
        # Recover an open pull even when the create response was interrupted.
        numbers = set(() if self.pull is None else (self.pull,))
        if self.pushed:
            pulls = self.ask('GET', BASE + '/pulls?' + urlencode({'state': 'open',
                              'head': OWNER + ':' + self.branch, 'base': 'main', 'per_page': 1}))
            for pull in pulls:
                if pull['head']['ref'] == self.branch:
                    require((self.mode == 'fake' and 'repo' not in pull['head'])
                            or pull['head']['repo']['full_name'] == REPOSITORY)
                    numbers.add(pull['number'])
        for number in numbers:
            self.ask('PATCH', BASE + '/issues/' + str(number), {'state': 'closed'})
            closed = self.ask('GET', BASE + '/pulls/' + str(number))
            require(closed['state'] == 'closed')
        if numbers:
            self.pull = None
            self.mark('pull request closed')
        if self.pushed:
            self.ask('DELETE', BASE + '/git/refs/heads/' + self.branch, expected=(204, 404))
            self.pushed = False
            self.mark('run branch deleted')

    def run_sitting(self):
        upstream = self.fake() if self.mode == 'fake' else ''
        relay = self.start([self.harness, '--ignored', '--exact', 'managed_relay', '--nocapture'], 'relay',
                           {'TEMPER_MANAGED_MODE': self.mode, 'TEMPER_MANAGED_SCRATCH': str(self.scratch),
                            'TEMPER_MANAGED_UPSTREAM': upstream, 'TEMPER_MANAGED_PREFIX': self.prefix,
                            'TEMPER_MANAGED_EVIDENCE': str(self.evidence)})
        self.relay = relay
        until = time.monotonic() + 30
        ready = self.scratch/'relay-ready.json'
        while not ready.exists():
            require(relay.proc.poll() is None and time.monotonic() < until)
            time.sleep(0.05)
        addresses = json.loads(ready.read_text())
        self.api, self.git = addresses['api'], addresses['git']
        self.allowed.update((self.api, self.git))
        self.mark('private guarded API and git relay started')
        link = self.configure(self.git, 'http://' + upstream if self.mode == 'fake' else 'https://github.com')
        if self.mode == 'fake':
            self.connect_fake(link)
        else:
            self.mark('ssh -L ' + str(self.port) + ':127.0.0.1:' + str(self.port) + ' hetzner')
            self.mark('one-time sign-in: ' + link)
            self.mark('Connect GitHub; create the App; install on freeekanayaka/temper-probe-2 alone; in Ready choose Start here')
        self.step = 'waiting for the exact installation'
        try:
            self.wait_ready()
        except InstallationRefused:
            if not self.wide:
                raise
            self.check_wide_refusal()
            return
        require(not self.wide)  # A wide installation must never reach adoption or writes.
        self.mark('installation reaches exactly freeekanayaka/temper-probe-2')
        if self.mode == 'fake':
            self.ready_client(link, 'adopt')
        # Owner confirms the UI has reached the adopted card; a blank line has no authority value.
        # The dry run reads it too, so every prompt of the sitting has been answered once before.
        self.mark('after the adopted card appears, press Enter here')
        if self.mode == 'fake':
            time.sleep(self.wait_secs)
        require(sys.stdin.readline() == '\n')
        self.mark('repository adopted from Ready')
        require(self.evidence.stat().st_mode & 0o777 == 0o600)
        self.mark('redacted conversion retained: ' + str(self.evidence))
        self.step = 'pushing branch and opening pull request'
        self.push()
        if self.mode == 'fake':
            self.planted_refusals()
        self.cleanup_remote()
        self.mark('delete this run\'s GitHub App, then press Enter here')
        require(sys.stdin.readline() == '\n')
        if self.mode == 'live':
            until = time.monotonic() + 120
            while True:
                status, _, _ = self.http(self.api, 'GET', '/app', authorization=self.jwt())
                # GitHub answered 404 for a deleted App's JWT at the sitting of 10-24; 401 stays accepted.
                if status in (401, 404):
                    break
                require(status == 200 and time.monotonic() < until)
                time.sleep(2)
            self.mark('deleted App JWT refused with ' + str(status))
        else:
            self.mark('fake has no App-deletion HTTP control; deletion and JWT 401 check skipped')
        self.step = 'scanning relay artifacts for secret bytes'
        self.scan_secrets()
        self.retain_journal()
        self.mark('managed sitting completed')

    def finish(self):
        try:
            if self.token and (self.pull is not None or self.pushed):
                self.cleanup_remote()
        finally:
            try:
                self.scan_secrets()
                self.retain_journal()
            finally:
                for process in reversed(self.processes):
                    process.stop()


def main():
    os.umask(0o077)
    sitting = Sitting(*sys.argv[1:])
    def interrupt(_signal, _frame):
        raise KeyboardInterrupt
    signal.signal(signal.SIGTERM, interrupt)
    signal.signal(signal.SIGHUP, interrupt)
    try:
        sitting.run_sitting()
    except BaseException:
        print('managed sitting failed during: ' + sitting.step, file=sys.stderr, flush=True)
        if sitting.mode == 'live' and sitting.app_id is not None:
            print('delete this sitting\'s GitHub App id ' + str(sitting.app_id) + '; conversion evidence remains at ' + str(sitting.evidence), file=sys.stderr)
        if hasattr(sitting, 'last_status'):
            print('last HTTP status: ' + str(sitting.last_status), file=sys.stderr)
        sitting.report_refusal()
        for process in sitting.processes:
            if process.diagnostics:
                print('startup fields/categories: ' + ', '.join(sorted(process.diagnostics)), file=sys.stderr)
        return 1
    finally:
        sitting.finish()
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except Exception:
        # Exceptions from parsers, subprocesses and TLS must never print raw data.
        print('managed sitting failed; private processes stopped', file=sys.stderr)
        sys.exit(1)
