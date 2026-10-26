"""Pure file contracts and trouble decisions for the browser cluster."""
import importlib.machinery
import importlib.util
import json
from pathlib import Path
import unittest
from unittest.mock import Mock, patch
import types

ROOT = Path(__file__).resolve().parents[2]
WEB = ROOT / 'cluster-web'
CLUSTER = ROOT / 'cluster'
loader = importlib.machinery.SourceFileLoader('cluster_web_trouble', str(WEB / 'checks/trouble'))
spec = importlib.util.spec_from_loader(loader.name, loader)
trouble = importlib.util.module_from_spec(spec)
loader.exec_module(trouble)


def entry(**changes):
    value = dict(n=1, kind='exception', text='broken', address='http://127.0.0.1:9000/chats/12',
                 call=1, during='wait')
    value.update(changes)
    return json.dumps(value)


class FilesTests(unittest.TestCase):
    def test_copies_and_engine_line(self):
        names = ['llm.json', 'settings.json', 'agent.json', 'w1.json', 'w1.digest', 'w1.secret',
                 'w2.json', 'w2.digest', 'w2.secret', 'forge.json']
        for name in names:
            with self.subTest(name=name):
                self.assertEqual((WEB / name).read_bytes(), (CLUSTER / name).read_bytes())
        before = (CLUSTER / 'engine.json').read_bytes()
        self.assertEqual(before.count(b'"bundle": null'), 1)
        self.assertEqual((WEB / 'engine.json').read_bytes(), before.replace(
            b'"bundle": null', b'"bundle": "{proc.engine.dir}/bundle"'))

    def test_experiment(self):
        data = (WEB / 'experiment.json').read_bytes()
        exp = json.loads(data)
        self.assertEqual(data, (json.dumps(exp, sort_keys=True, separators=(',', ':')) + '\n').encode())
        base = json.loads((CLUSTER / 'experiment.json').read_bytes())
        self.assertEqual(set(exp), set(base))
        for field in ('format', 'cost', 'driver', 'faults', 'planted'):
            self.assertEqual(exp[field], base[field])
        self.assertEqual(exp['name'], 'cluster-web')
        self.assertEqual(exp['dirs'], ['client', 'hand', 'spent'])
        processes = base['processes']
        next(p for p in processes if p['name'] == 'engine')['files'].append(dict(path='bundle', tree='bundle'))
        self.assertEqual(exp['processes'][:5], processes)
        self.assertEqual(exp['processes'][5:], [dict(
            name='browser', after=['engine'], env={}, memory_bytes=None,
            argv=['{browser.program}', 'hand', '--chromium', '{browser.chromium}',
                  '--fonts', '{browser.fonts}', '--scratch', '{proc.browser.dir}',
                  '--out', '{dir.hand}', '--origin', 'http://{proc.engine.ready.clients}',
                  '--size', '1440x900', '--listen', '127.0.0.1:0'],
            ready=[dict(stream='stdout', prefix='listening ', capture='address')])])
        ctl = ['{artifact.temper}', 'ctl', '{dir.client}']
        expected = [
            dict(name='sign-in', argv=ctl + ['sign-in', '-'], env={},
                 stdin='http://{proc.engine.ready.link}\n'),
            dict(name='settings', argv=ctl + ['change-settings', '{experiment}/settings.json',
                 '--against', '1', '--key', 'lab-settings'], env={}),
            dict(name='add-key', argv=ctl + ['add-key', '1', '{proc.llm.dir}/llm.key',
                 '--deployment', '--key', 'cluster-web-llm'], env={}),
            dict(name='usable', argv=ctl + ['wait', 'accounts', '--until', 'usable',
                 '--timeout', '200'], env={}),
            dict(name='relink', argv=['/bin/sh', '{experiment}/relink', '{artifact.temper}',
                 '{dir.spent}'], env={}, stdin='http://{proc.engine.ready.link}\n',
                 **{'await': dict(process='engine', stream='stdout', prefix='http://', capture='link')}),
            dict(name='browser-sign-in', argv=['{runner}/hand/sign-in', '{proc.browser.ready.address}'],
                 env={'PATH': '/usr/bin:/bin'}, stdin='http://{action.relink.link}\n'),
            dict(name='signed-in', argv=['{runner}/hand/browse', '{proc.browser.ready.address}',
                 'wait', 'heading', 'Chats', '--exact', '--timeout', '120'], env={'PATH': '/usr/bin:/bin'}),
            dict(name='smoke', argv=['/bin/sh', '{experiment}/smoke', '{runner}/hand/browse',
                 '{proc.browser.ready.address}'], env={'PATH': '/usr/bin:/bin'})]
        self.assertEqual(exp['actions'], expected)
        self.assertEqual(exp['collect'], [dict(name='hand', env={}, scan=[],
                         argv=['/bin/cp', '-R', '--', '{dir.hand}/.', '{collect.dir}/.'])])
        self.assertEqual(exp['checks'], [dict(name='trouble', env={},
                         argv=['/usr/bin/python3', '-I', '-B', '-S', '{experiment}/checks/trouble',
                               '{collected.hand}/trouble.jsonl'])])


class TroubleTests(unittest.TestCase):
    def test_pages(self):
        for path, expected in [('/', 'inbox'), ('/chats', 'chats'), ('/chats/123', 'chat'),
                               ('/accounts', 'accounts'), ('/tasks/9', 'task'),
                               ('/projects/32/adopt', 'adopt'),
                               ('/jig/v1/sign-in/link?secret=invented', 'sign-in'),
                               ('/chats/9?x=y#z', 'chat'), ('/unknown', 'other'),
                               ('/chats/no', 'other')]:
            for host in ('127.0.0.1', '[::1]', 'localhost'):
                with self.subTest(path=path, host=host):
                    self.assertEqual(trouble.page(f'http://{host}:9000{path}'), expected)
        for address in ('https://localhost/chats', 'http://example.org/chats', 'file:///chats', 'http://['):
            self.assertEqual(trouble.page(address), 'off-origin')
        self.assertEqual(trouble.page('about:blank'), 'blank')
        self.assertEqual(trouble.page('about:blank?x=y#z'), 'blank')

    def test_empty_and_sign_in(self):
        self.assertEqual(trouble.judge([]), (0, []))
        self.assertEqual(trouble.judge([entry(during='sign-in')]), (0, []))

    def test_group_count_and_first_text(self):
        self.assertEqual(trouble.judge([entry(), entry(text='later', address='http://localhost:1/chats/99')]),
                         (1, [dict(fail='browser/exception/chat', summary='2 entries; first: broken')]))

    def test_signature_order(self):
        self.assertEqual(trouble.judge([entry(kind='network'), entry(), entry(kind='network')]),
                         (1, [dict(fail='browser/network/chat', summary='2 entries; first: broken'),
                              dict(fail='browser/exception/chat', summary='1 entry; first: broken')]))

    def test_unreadable_and_failure(self):
        self.assertEqual(trouble.judge(['{', entry()]), (1, [
            dict(undecided='trouble.jsonl line 1 unreadable'),
            dict(fail='browser/exception/chat', summary='1 entry; first: broken')]))

    def test_unreadable_only(self):
        for line in ('{', '[]', '{}', entry(kind='Bad'), entry(kind='x/y'), entry(n=True),
                     entry(call=-1), entry(text=3), entry(during=[])):
            with self.subTest(line=line):
                self.assertEqual(trouble.judge([line]),
                                 (2, [dict(undecided='trouble.jsonl line 1 unreadable')]))

    def test_summary_cut(self):
        status, records = trouble.judge([entry(text='x' * 300)])
        self.assertEqual(status, 1)
        self.assertEqual(records[0]['summary'], ('1 entry; first: ' + 'x' * 300)[:200])


class LauncherTests(unittest.TestCase):
    """Exercise command construction only; no build or lab command runs."""
    def launcher(self, codes=None, dirty=False, target='/invented/target', heavy='/invented/heavy'):
        path = ROOT / 'cluster-web-judge'
        source = path.read_text().split("<<'PYTHON'\n", 1)[1].rsplit('\nPYTHON', 1)[0]
        # Load imports and definitions, leaving the command's entry point uncalled.
        namespace = {}
        with patch('shutil.which', return_value=heavy) as which:
            exec(compile(source[:source.index('\ntry:\n')], str(path), 'exec'), namespace)
        which.assert_called_once_with('heavy')
        calls = []
        codes = codes or {}
        def run(argv, repository, **kwargs):
            words = list(map(str, argv))
            calls.append(words)
            verb = words[1]
            output = {'status': ' M lab/README.md\n' if dirty else '',
                      'rev-parse': 'commit-C\n', 'subject': 'subject-D\n',
                      'validate': 'key-K\n', 'judge': 'pass\n'}.get(verb, '')
            return types.SimpleNamespace(returncode=codes.get(verb, 0), stdout=output)
        namespace['run'] = run
        namespace['sys'] = types.SimpleNamespace(
            argv=[str(path), str(path), '/invented/lab'], path=[], stderr=Mock())
        namespace['os'] = types.SimpleNamespace(X_OK=1, access=lambda *args: True,
                                                environ={'CARGO_TARGET_DIR': target})
        scratch = Mock()
        scratch.__enter__ = Mock(return_value='/invented/scratch')
        scratch.__exit__ = Mock(return_value=False)
        namespace['tempfile'] = types.SimpleNamespace(TemporaryDirectory=lambda **kw: scratch)
        copy = Mock()
        namespace['shutil'] = types.SimpleNamespace(copyfile=copy)
        pack = Mock(return_value=b'invented tar bytes')
        module = types.ModuleType('look_pack')
        module.pack = pack
        with patch.dict('sys.modules', look_pack=module), patch.object(Path, 'is_file', return_value=True), \
                patch.object(Path, 'write_bytes') as write, patch.object(Path, 'chmod'), patch('sys.stdout'):
            status = namespace['main']()
        self.copied = copy.call_args_list
        return status, calls, pack, write

    def test_builds_through_heavy_where_it_is_on_path(self):
        status, calls, pack, write = self.launcher()
        self.assertEqual(status, 0)
        self.assertEqual(calls[:4], [
            ['git', 'status', '--porcelain'], ['git', 'rev-parse', 'HEAD'],
            ['heavy', 'cargo', 'build', '--release', '-p', 'temper'],
            ['heavy', 'scripts/bundle.sh', '--release', '/invented/scratch/bundle']])
        self.assertEqual([call[0] for call in calls if len(call) > 1 and
                          call[1] in ('subject', 'validate', 'judge')], ['/invented/lab'] * 3)
        self.assertEqual(calls[4][1:], [
            'subject', '--repository', str(ROOT.parent), '--commit', 'commit-C',
            '--exclude', 'lab', '--artifact', 'temper=/invented/scratch/temper',
            '--artifact', 'bundle=/invented/scratch/bundle.tar'])
        self.assertEqual([call[1:] for call in calls[5:]], [
            ['validate', 'lab/cluster-web', '--subject', 'subject-D'],
            ['judge', 'lab/cluster-web', '--subject', 'subject-D']])
        pack.assert_called_once_with(Path('/invented/scratch/bundle'))
        write.assert_called_once_with(b'invented tar bytes')
        self.assertEqual([c.args for c in self.copied],
                         [(Path('/invented/target/release/temper'), Path('/invented/scratch/temper'))])

    def test_builds_plainly_where_no_heavy_is_on_path(self):
        status, calls, *_ = self.launcher(heavy=None)
        self.assertEqual(status, 0)
        self.assertEqual(calls[2:4], [
            ['cargo', 'build', '--release', '-p', 'temper'],
            ['scripts/bundle.sh', '--release', '/invented/scratch/bundle']])

    def test_dirty_tree_and_build_failure_stop(self):
        status, calls, _, _ = self.launcher(dirty=True)
        self.assertEqual((status, len(calls)), (2, 1))
        status, calls, _, _ = self.launcher(codes={'cargo': 1})
        self.assertEqual((status, len(calls)), (4, 3))

    def test_relative_target_and_verdict_statuses(self):
        for code, expected in ((1, 0), (2, 2), (3, 3), (4, 4)):
            with self.subTest(code=code):
                status, calls, _, _ = self.launcher(codes={'judge': code}, target='alternate-target')
                self.assertEqual(status, expected)
                self.assertEqual(self.copied[0].args[0], ROOT.parent / 'alternate-target/release/temper')


if __name__ == '__main__':
    unittest.main()
