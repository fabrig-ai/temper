"""Run both walks in the POSIX shell: pasted bytes, phases, and stop on failure."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

LAB = Path(__file__).resolve().parents[2]
NAMES = ('welcome-github', 'welcome-forgejo-here')
KEY = 'fake key with spaces\nand another line\n'


class KeyWalks(unittest.TestCase):
    def run_phase(self, name, phase, fail=None, address='http://localhost:8000/chats/7', key=True, previous='expected'):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            log = root / 'calls.jsonl'
            log.write_text('')
            if previous == 'expected':
                previous = {'forge': 'welcome', 'model': 'forge', 'ready': 'model', 'chat': 'ready'}.get(phase)
            if previous is not None:
                (root / 'phase').write_text(previous + '\n')
            key_path = root / 'model key'
            if key:
                key_path.write_text(KEY)
            browse = root / 'browse stand-in'
            browse.write_text('''#!/usr/bin/python3
import json
from pathlib import Path
import sys
args = sys.argv[2:]
stdin = sys.stdin.read() if args[0] == 'type' else ''
with Path(%r).open('a') as output:
    output.write(json.dumps([args, stdin]) + '\\n')
if args[0] == 'address':
    print(%r)
if args[:len(%r)] == %r:
    sys.exit(3)
''' % (str(log), address, fail or ['never'], fail or ['never']))
            browse.chmod(0o700)
            result = subprocess.run(['/bin/sh', str(LAB / name / 'steps'), phase,
                                     str(browse), '127.0.0.1:9100', str(root), str(key_path)],
                                    capture_output=True, text=True, env={})
            calls = [json.loads(line) for line in log.read_text().splitlines()]
            return (result, calls,
                    (root / 'chat').read_text() if (root / 'chat').exists() else None,
                    (root / 'phase').read_text().strip() if (root / 'phase').exists() else None)

    def test_each_walk_pastes_the_named_file_and_retains_chat(self):
        for name in NAMES:
            with self.subTest(experiment=name):
                result, calls, chat, _ = self.run_phase(name, 'walk')
                self.assertEqual((result.returncode, result.stderr), (0, ''))
                self.assertEqual(calls[0], [['wait', 'button', 'Sign out', '--exact', '--timeout', '120'], ''])
                self.assertIn([['type', 'text field', 'Key (pasted once)'], KEY], calls)
                self.assertIn([['press', 'button', 'Start chat', '--exact'], ''], calls)
                self.assertEqual(chat, '7\n')
                verbs = [args[0] for args, _ in calls]
                self.assertEqual(verbs.count('go'), 1, 'no navigation shortcuts after /welcome')
                self.assertEqual(verbs.count('reload'), 1)
                self.assertLess(verbs.index('address'), verbs.index('reload'))
                self.assertEqual(calls[-2][0][:3], ['wait', 'text', 'Hello from the local fake model.'])
                if name == 'welcome-github':
                    self.assertIn([['press', 'choice', 'fake-person/widget', '--exact'], ''], calls)
                else:
                    for field, text in [('Organization', 'work'), ('Repository', 'hello'),
                                        ('Into a new project, named', 'hello')]:
                        self.assertIn([['type', 'text field', field], text], calls)

    def test_each_phase_can_run_separately(self):
        for name in NAMES:
            walk, all_calls, _, _ = self.run_phase(name, 'walk')
            self.assertEqual(walk.returncode, 0)
            separated = []
            for phase in ('welcome', 'forge', 'model', 'ready', 'chat'):
                result, calls, _, _ = self.run_phase(name, phase)
                self.assertEqual((result.returncode, result.stderr), (0, ''))
                separated.extend(calls)
            self.assertEqual(separated, all_calls)

    def test_every_missed_wait_records_page_and_stops_the_walk(self):
        for name in NAMES:
            _, calls, _, _ = self.run_phase(name, 'walk')
            for index, (args, _) in enumerate(calls):
                if args[0] != 'wait':
                    continue
                # Repeated waits fail at their first occurrence.
                first = next(i for i, (a, _) in enumerate(calls) if a == args)
                if first != index:
                    continue
                with self.subTest(experiment=name, wait=args):
                    result, stopped, _, _ = self.run_phase(name, 'walk', fail=args)
                    self.assertEqual(result.returncode, 3)
                    self.assertEqual(stopped, calls[:index + 1] + [[['outline'], ''], [['shot'], '']])

    def test_failed_key_paste_does_not_submit(self):
        for name in NAMES:
            result, calls, _, _ = self.run_phase(name, 'model', fail=['type'])
            self.assertEqual(result.returncode, 3)
            self.assertFalse(any(args[:3] == ['press', 'button', 'Add key'] for args, _ in calls))
            result, calls, _, _ = self.run_phase(name, 'model', key=False)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(any(args[:3] == ['press', 'button', 'Add key'] for args, _ in calls))

    def test_bad_chat_address_stops_before_reload_and_collection(self):
        for name in NAMES:
            for address in ('http://localhost:8000/chats', 'http://localhost:8000/chats/nope'):
                result, calls, chat, _ = self.run_phase(name, 'chat', address=address)
                self.assertEqual(result.returncode, 1)
                self.assertIsNone(chat)
                self.assertFalse(any(args[0] == 'reload' for args, _ in calls))

    def test_a_failed_phase_prevents_later_actions_from_browsing(self):
        for name in NAMES:
            failed, _, _, state = self.run_phase(name, 'forge', fail=['wait'])
            self.assertEqual(failed.returncode, 3)
            self.assertEqual(state, 'running forge')
            for previous in (None, state, 'welcome'):
                result, calls, _, _ = self.run_phase(name, 'model', previous=previous)
                self.assertEqual(result.returncode, 1)
                self.assertEqual(calls, [])
            for phase in ('forge', 'ready', 'chat'):
                result, calls, _, _ = self.run_phase(name, phase, previous=state)
                self.assertEqual(result.returncode, 1)
                self.assertEqual(calls, [])

    def test_unknown_phase(self):
        for name in NAMES:
            result, calls, _, _ = self.run_phase(name, 'unknown')
            self.assertEqual(result.returncode, 2)
            self.assertEqual(calls, [])
