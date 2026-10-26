"""Pure live experiment contracts and gallery frame decisions."""
from contextlib import redirect_stdout
import copy
import hashlib
import importlib.machinery
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch
import types

ROOT = Path(__file__).resolve().parents[3]
LIVE = ROOT / 'lab/look-live'
WEB = ROOT / 'lab/cluster-web'
LOOK = ROOT / 'lab/look'
SCENES = ('chat-sent', 'chat-reloaded', 'chat-closing')
loader = importlib.machinery.SourceFileLoader('look_live_gallery', str(LIVE / 'checks/gallery'))
spec = importlib.util.spec_from_loader(loader.name, loader)
gallery = importlib.util.module_from_spec(spec)
loader.exec_module(gallery)


def read(path):
    return json.loads(path.read_bytes())


def canonical(value):
    return (json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False) + '\n').encode()


def template(path):
    # Leave placeholders alone; the copied templates double their JSON braces.
    return json.loads(path.read_text().replace('{{', '{').replace('}}', '}'))


class FilesTests(unittest.TestCase):
    def test_copies_and_only_two_charter_changes(self):
        copies = ['agent.json', 'engine.json', 'relink', 'w1.json', 'w1.digest', 'w1.secret',
                  'w2.json', 'w2.digest', 'w2.secret', 'forge.json']
        for name in copies:
            with self.subTest(name=name):
                self.assertEqual((LIVE / name).read_bytes(), (WEB / name).read_bytes())
        self.assertEqual((LIVE / 'stage').read_bytes(), (LOOK / 'stage').read_bytes())
        before, after = read(WEB / 'settings.json'), read(LIVE / 'settings.json')
        before['items'][0]['set'].update(waiting=300_000_000_000, time=310_000_000_000)
        self.assertEqual(before, after)
        self.assertEqual((LIVE / 'settings.json').read_bytes(), canonical(after))
        self.assertFalse((LIVE / 'checks/trouble').exists())
        self.assertEqual({p.relative_to(LIVE).as_posix() for p in LIVE.rglob('*') if p.is_file()},
                         set(copies) | {'settings.json', 'llm.json', 'stage', 'steps', 'experiment.json',
                                        'checks/gallery', 'gallery/chat.json', 'gallery/chat-closing.json'} |
                         {'scenes/' + name + '.json' for name in SCENES})

    def test_cue_is_the_only_model_change(self):
        expected = template(WEB / 'llm.json')
        actual = template(LIVE / 'llm.json')
        cue = next(c for c in actual['cued'] if c['cue'] == 'look-live-chat')
        self.assertEqual(cue, dict(cue='look-live-chat', turns=[
            dict(lines=[dict(text='First reply.'), dict(call='wait', arguments={})], finish='tool_calls', tokens=1),
            dict(lines=[dict(text='Second reply.'), dict(call='wait', arguments={})], finish='tool_calls', tokens=1),
            dict(lines=[dict(text='Live chat closed.')], finish='stop', tokens=1)]))
        actual['cued'].remove(cue)
        self.assertEqual(actual, expected)

    def test_live_and_gallery_scenes(self):
        chat = read(LOOK / 'scenes/chat.json')
        names = [n for n in chat['names'] if n not in ('Accept', 'Accept goal', 'Reject with reason')]
        composer = dict(role='textbox', name='Message', match='exact')
        dialog = dict(role='dialog', name='Close', match='exact')
        # A reload is a fresh document: its focus is the platform's (ui-web.md, 7).
        focuses = [composer, None, dict(role='button', name='', match='words')]
        for name, focus in zip(SCENES, focuses):
            with self.subTest(scene=name):
                path = LIVE / 'scenes' / (name + '.json')
                scene = read(path)
                self.assertEqual(path.read_bytes(), canonical(scene))
                self.assertEqual(scene['format'], 'look-scene/1')
                self.assertEqual(scene['name'], name)
                self.assertEqual(scene['widths'], chat['widths'])
                self.assertEqual(scene['names'], names)
                # The judge's: ids unique, each departure's line once a width.
                judged = scene['expectations'] + scene['departures']
                self.assertEqual(len({d['id'] for d in judged}), len(judged))
                lines = [(d['width'], d['signature']) for d in scene['departures']]
                self.assertEqual(len(set(lines)), len(lines))
                self.assertEqual(scene['page'], dict(live='/chats/1', ready=dialog if name == 'chat-closing'
                                                    else composer, focus=focus))
                if name == 'chat-closing':
                    self.assertIsNone(scene['mockup'])
                    # A modal's live tree holds the dialog alone: no banner region.
                    self.assertEqual(scene['regions'], [dict(name='dialog', page=dialog, mockup=None)])
                else:
                    self.assertEqual(scene['mockup'], dict(path='/chat.html?world=solo&task=T6&bare',
                                                         ready=chat['mockup']['ready']))
                    self.assertTrue((ROOT / 'docs/design/client/mockups' /
                                     scene['mockup']['path'].split('?')[0][1:]).is_file())
                    regions = [copy.deepcopy(r) for r in chat['regions']
                               if r['name'] in ('banner', 'navigation', 'chatbar', 'composer')]
                    next(r for r in regions if r['name'] == 'chatbar')['page'] = dict(
                        role='region', name='T1', match='words')
                    # The T6 mockup's composer is T6's.
                    next(r for r in regions if r['name'] == 'composer')['mockup']['name'] = 'Message T6'
                    self.assertEqual(scene['regions'], regions)
        gallery_names = (ROOT / 'tools/gallery/names.txt').read_text().splitlines()
        for name in ('chat', 'chat-closing'):
            path = LIVE / 'gallery' / (name + '.json')
            scene = read(path)
            self.assertEqual(path.read_bytes(), canonical(scene))
            self.assertEqual(scene, dict(format='look-scene/1', name=name, widths=chat['widths'],
                                        names=names, mockup=None, regions=[], expectations=[], departures=[],
                                        page=dict(path='/chat-solo-T6.html' if name == 'chat' else '/chat-closing.html',
                                                  ready=dict(role='main', name='', match='words'))))
            self.assertIn(scene['page']['path'][1:-5], gallery_names)

    def test_experiment_commands_and_environments(self):
        exp = read(LIVE / 'experiment.json')
        base = read(WEB / 'experiment.json')
        self.assertEqual((LIVE / 'experiment.json').read_bytes(), canonical(exp))
        self.assertEqual(set(exp), set(base))
        for field in ('format', 'cost', 'driver', 'faults', 'planted', 'processes'):
            self.assertEqual(exp[field], base[field])
        self.assertEqual(exp['name'], 'look-live')
        self.assertEqual(exp['dirs'], ['client', 'hand', 'look', 'page', 'spent'])
        self.assertEqual(exp['actions'][:7], base['actions'][:7])
        browse = ['{runner}/hand/browse', '{proc.browser.ready.address}']
        env = {'PATH': '/usr/bin:/bin'}
        capture = ['{runner}/look/capture', '--browser', '{browser.program}', '--chromium',
                   '{browser.chromium}', '--fonts', '{browser.fonts}']
        expected = [dict(name='drain', argv=browse + ['trouble'], env=env),
                    read(LOOK / 'experiment.json')['actions'][0]]
        for phase, scene in zip(('send', 'reload', 'close'), SCENES):
            expected.extend([
                dict(name=phase, argv=['/bin/sh', '{experiment}/steps', phase] + browse, env=env),
                dict(name='capture-' + scene[5:], env=env, argv=capture + [
                    '--mockups', '{source}/docs/design/client/mockups', '--live',
                    '{proc.browser.ready.address}', '--live-out', '{dir.hand}', '--ready-seconds', '40',
                    '--out', '{dir.look}/' + scene, '{experiment}/scenes/' + scene + '.json'])])
        self.assertEqual(exp['actions'][7:], expected)
        self.assertEqual(exp['collect'], [
            dict(name='look', env={}, scan=[], argv=['/bin/cp', '-R', '--', '{dir.look}/.', '{collect.dir}/.']),
            base['collect'][0], dict(name='gallery', env=env, scan=[], argv=capture + [
                '--page', '{dir.page}/gallery', '--ready-seconds', '40', '--out', '{collect.dir}/capture',
                '{experiment}/gallery/chat.json', '{experiment}/gallery/chat-closing.json'])])
        captures = [a for scene in SCENES for a in ('--capture', '{collected.look}/' + scene)]
        scenes = ['{experiment}/scenes/' + scene + '.json' for scene in SCENES]
        self.assertEqual(exp['checks'], [
            dict(name='look-' + kind, env=env, argv=['{runner}/look/check', kind] + captures + scenes)
            for kind in ('structure', 'style')] + [dict(name='gallery', env={}, argv=[
                '/usr/bin/python3', '-I', '-B', '-S', '{experiment}/checks/gallery', '--live',
                '{collected.look}', '--gallery', '{collected.gallery}/capture', '--scenes', '{experiment}/scenes',
                'chat-sent=chat', 'chat-reloaded=chat', 'chat-closing=chat-closing:modal'])])


class LauncherTests(unittest.TestCase):
    """Construct the real commands without invoking a build or a lab run."""
    def launcher(self, *, dirty=False, codes=None, heavy='/invented/heavy'):
        path = ROOT / 'lab/look-live-judge'
        source = path.read_text().split("<<'PYTHON'\n", 1)[1].rsplit('\nPYTHON', 1)[0]
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
            output = {'status': ' M invented\n' if dirty else '', 'rev-parse': 'commit-C\n',
                      'subject': 'subject-D\n', 'validate': 'key-K\n', 'judge': 'pass\n'}.get(verb, '')
            return types.SimpleNamespace(returncode=codes.get(verb, 0), stdout=output)
        namespace['run'] = run
        namespace['sys'] = types.SimpleNamespace(
            argv=[str(path), str(path), '/invented/lab'], path=[], stderr=Mock())
        namespace['os'] = types.SimpleNamespace(X_OK=1, access=lambda *args: True,
                                                environ={'CARGO_TARGET_DIR': 'target'})
        scratch = Mock()
        scratch.__enter__ = Mock(return_value='/invented/scratch')
        scratch.__exit__ = Mock(return_value=False)
        namespace['tempfile'] = types.SimpleNamespace(TemporaryDirectory=lambda **kw: scratch)
        copy = Mock()
        namespace['shutil'] = types.SimpleNamespace(copyfile=copy)
        pack = Mock(return_value=b'invented tar bytes')
        pack_module = types.ModuleType('look_pack')
        pack_module.pack = pack
        bar = Mock(return_value=('live bar\n', True))
        bar_module = types.ModuleType('look_bar')
        bar_module.bar = bar
        with patch.dict('sys.modules', look_pack=pack_module, look_bar=bar_module), \
                patch.object(Path, 'is_file', return_value=True), patch.object(Path, 'write_bytes'), \
                patch.object(Path, 'chmod'), patch('sys.stdout'):
            status = namespace['main']()
        return status, calls, pack, copy, bar

    def test_builds_through_heavy_where_it_is_on_path(self):
        status, calls, pack, copy, bar = self.launcher()
        self.assertEqual(status, 0)
        self.assertEqual(calls[:5], [
            ['git', 'status', '--porcelain'], ['git', 'rev-parse', 'HEAD'],
            ['heavy', 'cargo', 'build', '--release', '-p', 'temper', '-p', 'temper-client-gallery'],
            ['heavy', 'scripts/bundle.sh', '--release', '/invented/scratch/bundle'],
            [str(ROOT / 'target/release/temper-client-gallery'), '/invented/scratch/gallery']])
        self.assertEqual([call[0] for call in calls if len(call) > 1 and
                          call[1] in ('subject', 'validate', 'judge')], ['/invented/lab'] * 3)
        self.assertEqual(calls[5][1:], [
            'subject', '--repository', str(ROOT), '--commit', 'commit-C', '--exclude', 'lab',
            '--artifact', 'temper=/invented/scratch/temper', '--artifact', 'bundle=/invented/scratch/bundle.tar',
            '--artifact', 'gallery=/invented/scratch/gallery.tar'])
        self.assertEqual([call[1:] for call in calls[6:]], [
            ['validate', 'lab/look-live', '--subject', 'subject-D'],
            ['judge', 'lab/look-live', '--subject', 'subject-D']])
        self.assertEqual([call.args for call in pack.call_args_list], [
            (Path('/invented/scratch/gallery'),), (Path('/invented/scratch/bundle'),)])
        copy.assert_called_once_with(ROOT / 'target/release/temper', Path('/invented/scratch/temper'))
        bar.assert_called_once_with(Path.home() / 'src/rust/tmp/lab', ROOT / 'lab/look-live', 'key-K',
                                    Path.home() / 'src/rust/tmp/lab/judgements/look-live')

    def test_builds_plainly_where_no_heavy_is_on_path(self):
        status, calls, *_ = self.launcher(heavy=None)
        self.assertEqual(status, 0)
        self.assertEqual(calls[2:4], [
            ['cargo', 'build', '--release', '-p', 'temper', '-p', 'temper-client-gallery'],
            ['scripts/bundle.sh', '--release', '/invented/scratch/bundle']])

    def test_dirty_build_and_validation_failures_stop(self):
        status, calls, _, _, bar = self.launcher(dirty=True)
        self.assertEqual((status, len(calls)), (2, 1))
        bar.assert_not_called()
        status, calls, _, _, bar = self.launcher(codes={'cargo': 1})
        self.assertEqual((status, len(calls)), (4, 3))
        status, calls, _, _, bar = self.launcher(codes={'validate': 2})
        self.assertEqual((status, calls[-1][1]), (2, 'validate'))
        bar.assert_not_called()
        for code, expected in ((1, 0), (2, 2), (3, 3), (4, 4)):
            with self.subTest(code=code):
                self.assertEqual(self.launcher(codes={'judge': code})[0], expected)


FRAME = '''banner
  navigation "Main navigation"
    link "Chats"
  button "Sign out"
main
  region
    heading 1
    button "Close"
  textbox "Message"
  button "Send"
dialog "Close"
  button "Close"
'''
CONTENT = '''  region "Conversation"
    list (2)
    button "Close"
    button "Folded call"
    region "Delegate"
      heading 2
      button "Chats"
'''


def tree(text):
    """Tree entries whose outline is text: a line's indent its depth."""
    entries = []
    for line in text.splitlines():
        own = line.lstrip(' ')
        role, name = gallery.label(own)
        role, _, level = role.partition(' ')
        entries.append(dict(depth=(len(line) - len(own)) // 2, role=role, name=name or '',
                            level=int(level) if level.isdigit() else 0, node=None))
    return entries


class GalleryTests(unittest.TestCase):
    def compare(self, live=FRAME, reference=FRAME):
        return gallery.compare(live, reference, 'chat-sent', 'desktop')

    def test_projects_count_as_one_or_more(self):
        # The banner's unnamed buttons are its projects, data: their number
        # and names never compared, but a banner without one still fails.
        live = FRAME.replace('banner\n', 'banner\n  button\n  button\n')
        reference = FRAME.replace('banner\n', 'banner\n  button\n')
        self.assertEqual(self.compare(live, reference), [])
        self.assertEqual(gallery.frame(live)['banner > button'], 1)
        self.assertEqual(self.compare(FRAME, reference), [dict(
            fail='chat-sent/desktop/gallery/missing/button', summary='banner > button: live 0, gallery 1')])

    def test_a_trailing_count_is_left_off_a_name(self):
        names = ['Inbox', 'Chats', 'Step 2']
        def links(*words):
            return gallery.outline([dict(depth=0, role='banner', name='', level=0)] + [
                dict(depth=1, role='link', name=word, level=0) for word in words], names)
        self.assertEqual(links('Inbox 2', 'Inbox', 'Inbox 12'),
                         'banner\n  link "Inbox"\n  link "Inbox"\n  link "Inbox"\n')
        # Digits alone after a space; a name of the scene's is kept whole.
        self.assertEqual(links('Inbox2', 'Inbox 2a', 'Inbox  2', 'Step 2', 'Mail 2'),
                         'banner\n' + '  link\n' * 3 + '  link "Step 2"\n  link\n')
        self.assertEqual(gallery.compare(links('Inbox 2'), links('Inbox'), 'chat-sent', 'desktop'), [])
        self.assertEqual({r['fail'] for r in gallery.compare(
            links('Messages 2'), links('Inbox'), 'chat-sent', 'desktop')},
            {'chat-sent/desktop/gallery/missing/link/Inbox', 'chat-sent/desktop/gallery/extra/link'})

    def test_outline_is_labs_with_no_dialog_line(self):
        entries = tree(FRAME + CONTENT.replace('  ', '', 1))
        self.assertEqual(gallery.outline(entries, ['Main navigation', 'Chats', 'Sign out', 'Close',
                                                   'Message', 'Send']), '''banner
  navigation "Main navigation"
    link "Chats"
  button "Sign out"
main
  region
    heading 1
    button "Close"
  textbox "Message"
  button "Send"
button "Close"
region
  list (0)
  button "Close"
  button
  region
    heading 2
    button "Chats"
''')

    def test_agrees_and_ignores_whole_conversation(self):
        self.assertEqual(self.compare(), [])
        live = FRAME.replace('  textbox', CONTENT + '  textbox')
        reference = FRAME.replace('  textbox', '  region\n    button "Different content"\n  textbox')
        self.assertEqual(self.compare(live, reference), [])
        entries = gallery.frame(live)
        self.assertNotIn('main', entries)
        self.assertEqual(entries['main > region > button "Close"'], 1)
        self.assertEqual(entries['dialog "Close" > button "Close"'], 1)
        self.assertNotIn('main > region "Conversation" > button "Close"', entries)

    def test_missing_extra_and_counts(self):
        records = self.compare(FRAME.replace('    link "Chats"\n', '').replace(
            '  button "Send"\n', '  button "Send"\n  button "Send"\n'))
        self.assertEqual(records, [
            dict(fail='chat-sent/desktop/gallery/missing/link/Chats',
                 summary='banner > navigation "Main navigation" > link "Chats": live 0, gallery 1'),
            dict(fail='chat-sent/desktop/gallery/extra/button/Send',
                 summary='main > button "Send": live 2, gallery 1')])

    def test_landmarks_are_distinct(self):
        live = FRAME.replace('  button "Sign out"\n', '').replace(
            '    link "Chats"\n', '    link "Chats"\n    button "Sign out"\n')
        self.assertEqual({r['fail'] for r in self.compare(live)}, {
            'chat-sent/desktop/gallery/extra/button/Sign%20out',
            'chat-sent/desktop/gallery/missing/button/Sign%20out'})

    def test_composer_at_any_depth_and_multiple_dialogs(self):
        text = 'main\n  region\n  form\n    textbox "Message"\n    button "Send"\n'
        self.assertEqual(gallery.frame(text), {
            'main > region': 1, 'main > form > textbox "Message"': 1,
            'main > form > button "Send"': 1})
        self.assertEqual(gallery.frame('dialog\n  button\ndialog\n  button\n'),
                         {'dialog': 2, 'dialog > button': 2})

    def test_modal_pair_compares_the_dialog_alone(self):
        # The lab's outline prints no dialog line; under a modal the live tree
        # holds the dialog's lines alone, outside every landmark.
        modal = FRAME + 'button "Confirm"\nbutton "Cancel"\n'
        live = 'button "Confirm"\nbutton "Cancel"\n'
        self.assertEqual(gallery.compare(live, modal, 'chat-closing', 'desktop', modal=True), [])
        self.assertEqual(gallery.compare('button "Confirm"\n', modal, 'chat-closing', 'desktop', modal=True),
                         [dict(fail='chat-closing/desktop/gallery/missing/button/Cancel',
                               summary='button "Cancel": live 0, gallery 1')])

    def test_lost_banner_fails_a_pair_not_declared_modal(self):
        lost = FRAME[FRAME.index('main'):]
        fails = {r['fail'] for r in self.compare(lost)}
        self.assertIn('chat-sent/desktop/gallery/missing/banner', fails)
        self.assertIn('chat-sent/desktop/gallery/missing/button/Sign%20out', fails)

    def test_modal_pair_syntax(self):
        status, records = gallery.judge(Path('/nonexistent'), Path('/nonexistent'), Path('/nonexistent'),
                                        ['chat-closing=chat-closing:other'])
        self.assertEqual((status, records), (2, [dict(
            undecided='chat-closing=chat-closing:other: expected LIVE=GALLERY[:modal]')]))

    def test_signature_encoding_and_roles(self):
        for entry, signature in [('banner > button "a > é/\\\""', 'button/a%20%3E%20%C3%A9%2F%22'),
                                  ('main > region > heading 1', 'heading-1'),
                                  ('banner > list (2)', 'list-2')]:
            self.assertEqual(gallery.entry_signature('extra', entry), 'extra/' + signature)

    def test_unreadable_outline(self):
        for text in ('   button\n', '  button\n', 'banner\n    button\n', 'banner\n  button "bad\\q"\n'):
            with self.subTest(text=text), self.assertRaises(ValueError):
                gallery.frame(text)


class GalleryInputsTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.live = self.root / 'live'
        self.reference = self.root / 'gallery'
        self.scenes = self.root / 'scenes'
        self.scenes.mkdir()
        (self.scenes / 'chat-sent.json').write_bytes(canonical(dict(names=[
            'Main navigation', 'Chats', 'Sign out', 'Close', 'Message', 'Send'])))
        self.pairs = ['chat-sent=chat']
        self.capture(self.live / 'chat-sent', 'chat-sent')
        self.capture(self.reference, 'chat')

    def capture(self, directory, scene, text=FRAME, widths=('desktop', 'phone')):
        directory.mkdir(parents=True, exist_ok=True)
        records = []
        for width in widths:
            data = canonical(dict(tree=tree(text)))
            path = scene + '/' + width + '/page/tree.json'
            output = directory / path
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_bytes(data)
            records.append(dict(name=width, page=dict(tree=dict(
                path=path, bytes=len(data), sha256='sha256:' + hashlib.sha256(data).hexdigest()))))
        (directory / 'capture.json').write_bytes(canonical(dict(format='look-capture/1', scenes=[
            dict(name=scene, widths=records)])))

    def judge(self):
        return gallery.judge(self.live, self.reference, self.scenes, self.pairs)

    def test_success_and_status_one_with_undecided(self):
        self.assertEqual(self.judge(), (0, []))
        self.capture(self.live / 'chat-sent', 'chat-sent', FRAME.replace('  button "Send"\n', ''))
        self.pairs.append('absent=chat')
        status, records = self.judge()
        self.assertEqual(status, 1)
        self.assertEqual(len([r for r in records if 'fail' in r]), 2)
        self.assertIn('undecided', records[-1])

    def test_absent_or_unreadable_record_scene_and_width(self):
        path = self.live / 'chat-sent/capture.json'
        original = path.read_bytes()
        for data in (None, b'{', b'\xff', b'[]', b'{}', canonical(dict(format='other')),
                     canonical(dict(format='look-capture/1', scenes=[]))):
            with self.subTest(data=data):
                if data is None:
                    path.unlink()
                else:
                    path.write_bytes(data)
                status, records = self.judge()
                self.assertEqual(status, 2)
                self.assertIn('undecided', records[0])
                path.write_bytes(original)
        self.capture(self.live / 'chat-sent', 'chat-sent', widths=('desktop',))
        status, records = self.judge()
        self.assertEqual(status, 2)
        self.assertIn('phone', records[0]['undecided'])
        self.pairs = ['bad-pair', '../escape=chat', 'chat-sent=absent']
        self.assertEqual(self.judge()[0], 2)
        self.capture(self.live / 'chat-sent', 'chat-sent')
        self.pairs = ['chat-sent=chat']
        for names in (None, b'{', canonical(dict(names='Chats')), canonical(dict(names=[1]))):
            with self.subTest(names=names):
                path = self.scenes / 'chat-sent.json'
                path.unlink(missing_ok=True)
                if names is not None:
                    path.write_bytes(names)
                status, records = self.judge()
                self.assertEqual(status, 2)
                self.assertIn('chat-sent=chat', records[0]['undecided'])

    def test_tree_missing_unreadable_size_and_digest(self):
        directory = self.live / 'chat-sent'
        record = read(directory / 'capture.json')
        width = record['scenes'][0]['widths'][0]
        outline = width['page']['tree']
        path = directory / outline['path']
        original = path.read_bytes()
        for data in (None, b'wrong size', b'x' * len(original)):
            with self.subTest(data=data):
                if data is None:
                    path.unlink()
                else:
                    path.write_bytes(data)
                self.assertEqual(self.judge()[0], 2)
                path.write_bytes(original)
        for data in (b'\xff', b'[]', b'{"tree":{}}', b'{"tree":[{"depth":"0","role":"main","name":""}]}',
                     b'{"tree":[{"depth":0,"role":"main"}]}'):
            with self.subTest(data=data):
                current = read(directory / 'capture.json')
                file = current['scenes'][0]['widths'][0]['page']['tree']
                (directory / file['path']).write_bytes(data)
                file.update(bytes=len(data), sha256='sha256:' + hashlib.sha256(data).hexdigest())
                (directory / 'capture.json').write_bytes(canonical(current))
                self.assertEqual(self.judge()[0], 2)

    def test_output_contract(self):
        self.capture(self.live / 'chat-sent', 'chat-sent', FRAME.replace('    link "Chats"\n', ''))
        output = io.StringIO()
        with redirect_stdout(output):
            status = gallery.main(['--live', str(self.live), '--gallery', str(self.reference),
                                   '--scenes', str(self.scenes), *self.pairs])
        self.assertEqual(status, 1)
        self.assertTrue(all(set(json.loads(line)) == {'fail', 'summary'} for line in output.getvalue().splitlines()))


if __name__ == '__main__':
    unittest.main()
