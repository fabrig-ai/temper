"""The key walks retain browser-only forge setup and the committed reply proof."""
import json
from pathlib import Path
import re
import runpy
import unittest

LAB = Path(__file__).resolve().parents[2]
NAMES = ('welcome-github', 'welcome-forgejo-here')
ANSWER = 'Hello from the local fake model.'


def conversation(texts):
    return {'activity': {'rows': [
        {'event': {'owner': {'decoded': {'title': ANSWER}}}},
        {'event': {'owner': {'decoded': {'turn': {'decoded': {'messages': [
            {'speaker': speaker, 'blocks': [{'text': {'text': text}}]}
            for speaker, text in texts]}}}}}},
    ]}}


class KeyWalks(unittest.TestCase):
    def test_committed_reply_and_trouble_for_each_walk(self):
        for name in NAMES:
            proof = runpy.run_path(str(LAB / name / 'checks/proof'))
            with self.subTest(experiment=name):
                self.assertEqual(proof['reply'](conversation([('assistant', ANSWER)])), (0, []))
                self.assertEqual(proof['reply'](conversation([('user', ANSWER)]))[1][0]['fail'], 'reply/missing')
                self.assertEqual(proof['reply'](conversation([
                    ('assistant', 'Wrong'), ('assistant', ANSWER)]))[1][0]['fail'], 'reply/first-assistant')
                self.assertEqual(proof['trouble']([]), (0, []))
                self.assertEqual(proof['trouble'](['{"kind":"network","during":"sign-in"}'])[0], 1)
                with self.assertRaises(ValueError):
                    proof['trouble'](['{}'])

    def test_phases_and_key_capture_are_browser_actions(self):
        for name in NAMES:
            exp = json.loads((LAB / name / 'experiment.json').read_text())
            with self.subTest(experiment=name):
                self.assertEqual([a['name'] for a in exp['actions']], [
                    'sign-in', 'relink', 'browser-sign-in', 'welcome', 'forge', 'model', 'ready', 'chat'])
                for action in exp['actions'][3:]:
                    self.assertEqual(action['argv'][2], action['name'])
                    self.assertEqual(action['argv'][-1], '{proc.box.ready.key}')
                box, browser = exp['processes']
                self.assertIn({'capture': 'key', 'prefix': 'box: model key ', 'stream': 'stderr'}, box['ready'])
                self.assertIn('--fake-backends', box['argv'])
                self.assertEqual(exp['driver'], None)
                if name == 'welcome-github':
                    peer = browser['argv'].index('--peer')
                    self.assertEqual(browser['argv'][peer + 1], 'http://{proc.box.ready.github}')
                    self.assertIn({'capture': 'github', 'prefix': 'box: fake github http://', 'stream': 'stderr'}, box['ready'])
                else:
                    self.assertNotIn('--peer', browser['argv'])
                    self.assertIn({'path': 'cache', 'tree': 'forgejo-cache'}, box['files'])

    def test_shared_evidence_scripts_match_chatgpt(self):
        for name in NAMES:
            for relative in ('checks/proof', 'relink', 'collect-chat'):
                with self.subTest(experiment=name, script=relative):
                    self.assertEqual((LAB / name / relative).read_bytes(),
                                     (LAB / 'welcome-chatgpt' / relative).read_bytes())

    def test_every_welcome_call_is_identical(self):
        steps = sorted(LAB.glob('welcome-*/steps'))
        self.assertGreaterEqual(len(steps), 3)
        pattern = re.compile(rb'^call\(\) \{\n.*?^\}\n', re.M | re.S)
        expected = pattern.findall((LAB / 'welcome-chatgpt/steps').read_bytes())
        self.assertEqual(len(expected), 1)
        for path in steps:
            with self.subTest(steps=str(path)):
                self.assertEqual(pattern.findall(path.read_bytes()), expected)
