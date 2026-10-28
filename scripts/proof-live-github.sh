#!/bin/sh
# Owner-run only, domain/testing.md 2.7. Implementers compile this tier offline.
set -eu
umask 077
cd "$(dirname "$0")/.."
# The machine's build queue where it has one, plain cargo where it hasn't
# (agents' docs/design/build-infrastructure.md 12).
heavy=$(command -v heavy || true)
case ${1:-} in freeekanayaka|fabrig-ai) owner=$1 ;; *) printf '%s\n' 'usage: proof-live-github.sh {freeekanayaka|fabrig-ai} [FIXTURES]' >&2; exit 2 ;; esac
[ "$#" -le 2 ] || exit 2
fixtures=$(readlink -f "${2:-$HOME/src/rust/tmp/github-conformance}")
[ -d "$fixtures" ] && [ "$(stat -c %a "$fixtures")" = 700 ] || {
    printf '%s\n' 'fixtures must be a directory with mode 0700' >&2; exit 2;
}
# The App key is the only credential; app.json supplies public App/installation ids.
python3 -c 'from cryptography.hazmat.primitives.serialization import load_pem_private_key'
export NEXTEST_DOUBLE_SPAWN=0
# shellcheck disable=SC2086
$heavy cargo nextest run -p temper --profile live --no-run
scratch=$(readlink -f "$(mktemp -d "${TMPDIR:-/tmp}/temper-live-github.XXXXXX")")
cleanup() {
    status=$?
    # Backstop failures must not skip removal of the run's private state.
    set +e
    # The supervisor returns only after settling its owned descendants.
    : > "$scratch/stop-runner"
    [ -z "$runner" ] || wait "$runner" 2>/dev/null
    # Python signs an App JWT as the managed harness does, then mints a cleanup token.
    # Bearers enter curl's configuration on stdin, never argv, a log, or a copied file.
    python3 - "$scratch" "$fixtures" "$owner" <<'PY'
import base64
import json
import pathlib
import re
import subprocess
import sys
import time
import urllib.parse
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.asymmetric import padding
from cryptography.hazmat.primitives.serialization import load_pem_private_key
scratch, fixtures, owner = map(pathlib.Path, sys.argv[1:])
owner = str(owner)
manifest = scratch / 'run.json'
if not manifest.exists():
    sys.exit(0)
run = json.loads(manifest.read_text())
if run.get('cleaned') is True:
    sys.exit(0)
# Refuse tampered metadata before constructing any URL or reading the key.
if (run.get('owner') != owner or run.get('repository') != 'temper-probe'
        or not re.fullmatch(r'\d{8}T\d{6}[0-9a-f]{4}', run.get('run', ''))
        or run.get('prefix') != 'temper/live-' + run['run'] + '-'):
    raise SystemExit('backstop: invalid run metadata')
count_file = scratch / 'count.txt'
try:
    count = int(count_file.read_text().strip())
except (OSError, ValueError):
    raise SystemExit('backstop: missing or invalid count; refusing uncounted cleanup')
if not 0 <= count <= 400:
    raise SystemExit('backstop: count outside cap')
token = None
start = count
base = '/repos/' + owner + '/temper-probe'
prefix = run['prefix']

def ask(method, path, body=None, authorization=None):
    global count
    # Keep the final slot for revoking this backstop's own token, even after a cut.
    limit = 400 if method == 'DELETE' and path == '/installation/token' else 399
    if count >= limit:
        raise RuntimeError('backstop: request cap reached')
    count += 1
    count_file.write_text(str(count) + '\n')
    bearer = authorization if authorization is not None else token
    if not bearer or any(c.isspace() or c in '\\"' for c in bearer):
        raise RuntimeError('backstop: invalid bearer shape')
    config = 'header = "Authorization: Bearer ' + bearer + '"\n'
    args = ['curl', '--silent', '--show-error', '--connect-timeout', '15', '--max-time', '30',
            '--max-filesize', '16777216', '-K', '-', '-X', method, '-H', 'User-Agent: temper-live-backstop',
            '-H', 'Accept: application/vnd.github+json', '-w', '\n%{http_code}',
            'https://api.github.com' + path]
    if body is not None:
        args.extend(['-H', 'Content-Type: application/json', '--data', json.dumps(body)])
    result = subprocess.run(args, input=config.encode(), stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    if result.returncode:
        raise RuntimeError('backstop: curl failed')
    data, status = result.stdout.rsplit(b'\n', 1)
    return int(status), data

try:
    app = json.loads((fixtures / 'app.json').read_text())
    installation = next(row['id'] for row in app['installations'] if row['account'] == owner)
    key = load_pem_private_key((fixtures / 'app.pem').read_bytes(), password=None)
    # Same RS256 construction and clock allowance as proof-live-github-managed.py.
    encode = lambda data: base64.urlsafe_b64encode(data).rstrip(b'=')
    now = int(time.time())
    claims = json.dumps({'iat': now-30, 'exp': now+300, 'iss': str(app['id'])}, separators=(',', ':')).encode()
    message = encode(b'{"alg":"RS256","typ":"JWT"}') + b'.' + encode(claims)
    jwt = (message + b'.' + encode(key.sign(message, padding.PKCS1v15(), hashes.SHA256()))).decode()
    status, data = ask('POST', '/app/installations/' + str(installation) + '/access_tokens',
                       {'repositories': ['temper-probe']}, authorization=jwt)
    if status != 201:
        raise RuntimeError('backstop: cannot mint cleanup token')
    token = json.loads(data)['token']
    query = urllib.parse.urlencode({'state': 'open', 'head': owner + ':' + prefix + '1/c2', 'per_page': 100})
    status, data = ask('GET', base + '/pulls?' + query)
    if status != 200:
        raise RuntimeError('backstop: cannot list run pulls')
    for pull in json.loads(data):
        if (not pull['head']['ref'].startswith(prefix)
                or pull['head']['repo']['full_name'] != owner + '/temper-probe'):
            raise RuntimeError('backstop: pull outside run')
        number = pull['number']
        if not isinstance(number, int) or number <= 0:
            raise RuntimeError('backstop: invalid pull number')
        status, _ = ask('PATCH', base + '/pulls/' + str(number), {'state': 'closed'})
        if status != 200:
            print('backstop: pull left=' + str(number), file=sys.stderr)
    path = base + '/git/matching-refs/heads/' + urllib.parse.quote(prefix, safe='/')
    status, data = ask('GET', path)
    if status != 200:
        raise RuntimeError('backstop: cannot list run branches')
    for row in json.loads(data):
        name = row['ref'].removeprefix('refs/heads/')
        if not name.startswith(prefix):
            raise RuntimeError('backstop: branch outside run')
        status, _ = ask('DELETE', base + '/git/refs/heads/' + urllib.parse.quote(name, safe='/'))
        if status not in (204, 404):
            print('backstop: branch left=' + name, file=sys.stderr)
except (RuntimeError, ValueError, KeyError, TypeError, OSError, StopIteration):
    print('backstop: cleanup incomplete; state=' + str(scratch)
          + ' (removed on exit unless TEMPER_LIVE_GITHUB_KEEP=1)', file=sys.stderr)
finally:
    if token is not None:
        try:
            status, _ = ask('DELETE', '/installation/token')
            if status not in (204, 401):
                raise RuntimeError('backstop: cleanup token revocation failed')
        except (RuntimeError, ValueError, TypeError):
            print('backstop: cleanup token revocation incomplete', file=sys.stderr)
    print('live github backstop requests=' + str(count-start) + ' total=' + str(count) + ' state=' + str(scratch)
          + ' (removed on exit unless TEMPER_LIVE_GITHUB_KEEP=1)')
PY
    backstop_status=$?
    [ "$status" -ne 0 ] || status=$backstop_status
    if [ "${TEMPER_LIVE_GITHUB_KEEP:-}" = 1 ]; then
        printf 'live github state kept at %s (TEMPER_LIVE_GITHUB_KEEP=1)\n' "$scratch" >&2
    elif [ -n "$scratch" ]; then
        # Only the canonical mktemp path made above may be removed.
        case "$scratch" in
            */temper-live-github.[a-zA-Z0-9][a-zA-Z0-9][a-zA-Z0-9][a-zA-Z0-9][a-zA-Z0-9][a-zA-Z0-9])
                if rm -rf -- "$scratch"; then
                    if [ "$status" -ne 0 ]; then
                        printf '%s\n' 'live github state removed; rerun with TEMPER_LIVE_GITHUB_KEEP=1 to keep it' >&2
                    fi
                else
                    status=1
                fi
                ;;
        esac
    fi
    exit "$status"
}
runner=
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
trap 'exit 129' HUP
status=0
# shellcheck disable=SC2086
TEMPER_LIVE_GITHUB_OWNER=$owner TEMPER_LIVE_GITHUB_FIXTURES=$fixtures TMPDIR=$scratch \
    python3 scripts/owned-process.py --stop-file "$scratch/stop-runner" -- \
    $heavy cargo nextest run -p temper --profile live --run-ignored only --no-capture \
    -E 'binary(=live_github) & test(=a_chat_lands_a_change_on_github_com_through_a_restart)' &
runner=$!
wait "$runner" || status=$?
runner=
exit "$status"
