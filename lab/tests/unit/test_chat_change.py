"""H/P identity, driver boundary and free rehearsal contracts (draft/qa-lab.md, 3 and 6)."""
import json
from pathlib import Path
import re
import unittest

LAB = Path(__file__).resolve().parents[2]
S = LAB / 'chat-change-smoke'
H = LAB / 'chat-change'
P = LAB / 'chat-change-killed'
REHEARSAL = LAB / 'chat-change-rehearsal'
QA = (LAB.parent / 'docs/design/draft/qa-lab.md').read_text()


def canonical(value):
    return (json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False) + '\n').encode()


def read(root, name='experiment.json'):
    return json.loads((root / name).read_bytes())


def tree(root):
    return {str(path.relative_to(root)): (path.read_bytes(), path.stat().st_mode & 0o100)
            for path in root.rglob('*') if path.is_file()}


def requested_words():
    words = re.search(r'Asked for in these words:\*\* `([^`]+)`', QA)[1]
    return ' '.join(words.split())


class ChatChangeTests(unittest.TestCase):
    def test_h_inherits_s_documents_and_assembly_without_story(self):
        smoke = tree(S)
        healthy = tree(H)
        expected_names = {name for name in smoke if not name.startswith('smoke/')}
        expected_names.update(('charter.md', 'smoke/_runtime_llm.py'))
        self.assertEqual(set(healthy), expected_names)
        for name in expected_names - {'experiment.json', 'README.md', 'charter.md'}:
            with self.subTest(name=name):
                self.assertEqual(healthy[name], smoke[name])
        expected = read(S)
        expected.update(name='chat-change', cost='temper-m1',
                        about="H: ask a chat for NOTICE through ctl and follow its change to landing.",
                        driver=read(H)['driver'])
        expected['actions'] = [a for a in expected['actions'] if a['name'] != 'story']
        self.assertEqual(read(H), expected)
        self.assertEqual([a['name'] for a in expected['actions']],
                         ['sign-in', 'settings', 'add-key', 'usable', 'forge', 'adopt'])
        self.assertEqual(expected['faults'], [])
        self.assertEqual(expected['planted'], [])

    def test_charter_is_qa_block_with_one_final_newline(self):
        charter = QA.split('`charter.md`, whole:\n\n```\n', 1)[1].split('\n```', 1)[0] + '\n'
        self.assertEqual((H / 'charter.md').read_bytes(), charter.encode())

    def test_shared_adapter_keeps_root_open_and_no_h_smoke_story(self):
        adapter = (S / 'smoke/_runtime_llm.py').read_bytes()
        self.assertIn(b'NOTICE root for S, H and P', adapter)
        self.assertIn(b'Keep the root chat open after its delegate settles.', adapter)
        for root in (H, P):
            self.assertEqual((root / 'smoke/_runtime_llm.py').read_bytes(), adapter)
            self.assertEqual({p.name for p in (root / 'smoke').iterdir()}, {'_runtime_llm.py'})
            self.assertFalse((root / 'smoke/story').exists())
            self.assertFalse((root / 'smoke/_story.py').exists())

    def test_p_prime_is_h_exact_bytes_and_tree(self):
        healthy, planted = tree(H), tree(P)
        experiment, mode = planted.pop('experiment.json')
        healthy_experiment, healthy_mode = healthy.pop('experiment.json')
        self.assertEqual(planted, healthy)
        self.assertEqual(mode, healthy_mode)
        document = json.loads(experiment)
        self.assertEqual(document['faults'], [dict(kill='engine', signal='SIGKILL', after_actions=[1, 1])])
        self.assertEqual(document['planted'], ['stop/engine/signal:SIGKILL'])
        document.update(faults=[], planted=[])
        self.assertEqual(canonical(document), healthy_experiment)

    def test_driver_tool_mounts_and_harmless_provision(self):
        self.assertEqual(read(H)['driver'], dict(charter='charter.md', tools=[dict(
            name='ctl', argv=['/usr/bin/dash', '{experiment}/tools/ctl', '{artifact.temper}',
                              '{dir.client}', 'http://{proc.engine.ready.clients}/'],
            env={}, writes=['{dir.client}'],
            reads=['{experiment}/tools/ctl', '{artifact.temper}'],
            provision=dict(args=['me'], status=0))]))
        self.assertEqual((H / 'tools/ctl').read_bytes(), (S / 'tools/ctl').read_bytes())

    def test_scripts_canonical_and_use_only_ctl(self):
        for name in ('correct.json', 'wrong.json'):
            script = read(REHEARSAL, name)
            with self.subTest(name=name):
                self.assertEqual((REHEARSAL / name).read_bytes(), canonical(script))
                self.assertEqual(set(script), {'format', 'turns'})
                self.assertEqual(script['format'], 'lab-script/1')
                for turn in script['turns']:
                    self.assertLessEqual(set(turn), {'capture', 'output'})
                    for regex in turn.get('capture', {}).values():
                        self.assertEqual(re.compile(regex).groups, 1)
                    for output in turn['output']:
                        if 'call' in output:
                            self.assertEqual(output['call'], 'ctl')
                            self.assertEqual(output['namespace'], 'mcp__lab')
                            self.assertEqual(set(output['arguments']), {'args', 'stdin'})
                            self.assertIsNone(output['arguments']['stdin'])
                self.assertEqual(json.loads(script['turns'][-1]['output'][0]['text']),
                                 dict(done=True, retried=[], unseen=[], surprises=[]))

    def test_correct_follows_captured_root_and_change_wrong_asks_for_nothing(self):
        turns = read(REHEARSAL, 'correct.json')['turns']
        calls = [turn['output'][0]['arguments']['args'] for turn in turns[:-1]]
        self.assertEqual(calls, [
            ['chat', '--project', '1', '--key', 'notice', requested_words()],
            ['wait', 'task', '{capture.root}', '--until', 'replied', '--timeout', '200'],
            ['task', '{capture.root}', '--tree'],
            ['wait', 'task', '{capture.child}', '--until', 'ended', '--timeout', '220'],
            ['task', '{capture.child}']])
        self.assertEqual(set(turns[1]['capture']), {'root'})
        self.assertEqual(set(turns[3]['capture']), {'child'})
        self.assertEqual(re.search(turns[1]['capture']['root'], 'done: task 41, chat\n')[1], '41')
        tree_output = ('tree:\n41 · Add a file NOTICE to widget · Running\n'
                       '52 · @coding NOTICE · Waiting for news\n'
                       '53 · @coding NOTICE · Running\n')
        # The endpoint searches JSON-encoded input_text items from MCP output;
        # middle dots there are literal \u00b7 escapes, unlike ctl's stdout.
        tool_output = [dict(type='input_text', text=json.dumps(dict(
            status=0, signal=None, stdout=tree_output, stderr=''), ensure_ascii=False))]
        for output in (tree_output, json.dumps(tool_output)):
            self.assertEqual(re.search(turns[3]['capture']['child'], output)[1], '52')
        wrong = read(REHEARSAL, 'wrong.json')['turns']
        self.assertEqual(len(wrong), 2)
        self.assertEqual(wrong[0]['output'][0]['arguments']['args'], ['me'])


if __name__ == '__main__':
    unittest.main()
