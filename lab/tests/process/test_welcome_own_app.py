"""The shell pastes files, runs the printed login and stops at a missed step."""
import json
from pathlib import Path
import shlex
import subprocess
import tempfile
import unittest

STEPS = Path(__file__).resolve().parents[2] / 'welcome-own-app/steps'


class OwnAppWalk(unittest.TestCase):
    def run_phase(self, phase, fail=None):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            log = root / 'calls'
            browse = root / 'browse'
            browse.write_text('''#!/usr/bin/python3
import json
from pathlib import Path
import sys
args = sys.argv[2:]
value = sys.stdin.read() if args[0] == 'type' else None
if args[:3] == ['press', 'button', 'Use your ChatGPT subscription']:
    value = (Path(%r).parent / 'login-args').exists()
with Path(%r).open('a') as output:
    output.write(json.dumps([args, value]) + '\\n')
if args == ['outline']:
    print('code "On each host, run: temper program-login claude-person"')
sys.exit(1 if args[:3] == %r else 0)
''' % (str(log), str(log), fail))
            browse.chmod(0o700)
            key = root / 'key'
            pem = '-----BEGIN RSA PRIVATE KEY-----\nbytes\n-----END RSA PRIVATE KEY-----\n'
            key.write_text(pem)
            temper = root / 'temper'
            temper.write_text('#!/bin/sh\nprintf "%s\\n" "$@" > ' + shlex.quote(str(root / 'login-args')) + '\n')
            temper.chmod(0o700)
            command = shlex.join([str(temper), 'program-login', '--config', '/c', '--data', '/d', '--cache', '/cache with space']) + ' NAME (the name the Model step shows)'
            extra = [str(key)] if phase == 'forge' else [command] if phase == 'model' else []
            result = subprocess.run(['/bin/sh', str(STEPS), phase, str(browse), '127.0.0.1:9100', str(root), *extra],
                                    capture_output=True, text=True, env={})
            calls = [json.loads(line) for line in log.read_text().splitlines()]
            args = (root / 'login-args').read_text().splitlines() if (root / 'login-args').exists() else None
            return result, calls, args, pem

    def test_pem_pasted_unchanged_and_install_before_adoption(self):
        result, calls, _, pem = self.run_phase('forge')
        self.assertEqual(result.returncode, 0, result.stderr)
        pasted = [value for args, value in calls if args[:3] == ['type', 'text field', 'Private key']]
        self.assertEqual(pasted, [pem])
        acts = [args[:3] for args, _ in calls if args[0] == 'press']
        self.assertLess(acts.index(['press', 'button', 'Install on GitHub']), acts.index(['press', 'button', 'Adopt fake-person/widget']))

    def test_missed_connect_stops_before_install_and_adoption(self):
        result, calls, args, _ = self.run_phase('forge', ['press', 'button', 'Connect the App'])
        self.assertEqual(result.returncode, 1)
        self.assertEqual([call[0] for call in calls[-2:]], [['outline'], ['shot']])
        self.assertFalse(any('Install on GitHub' in call[0] for call in calls))
        self.assertIsNone(args)

    def test_model_runs_printed_command_with_visible_name(self):
        result, calls, args, _ = self.run_phase('model')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(args, ['program-login', '--config', '/c', '--data', '/d', '--cache', '/cache with space', 'claude-person'])
        self.assertEqual(calls[-1][0][:3], ['wait', 'heading', 'fake-person/widget is ready'])
        self.assertEqual([value for args, value in calls if args[:3] == ['press', 'button', 'Use your ChatGPT subscription']], [True])

    def test_missing_program_name_does_not_run_terminal_login(self):
        result, _, args, _ = self.run_phase('model', ['wait', 'text field', 'Name'])
        self.assertEqual(result.returncode, 1)
        self.assertIsNone(args)

    def test_failed_phase_latches_both_walks_before_later_browser_calls(self):
        for steps in [STEPS, STEPS.parent.parent / 'welcome-chatgpt/steps']:
            with self.subTest(steps=steps), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                browse = root / 'browse'
                log = root / 'calls'
                browse.write_text('#!/bin/sh\necho called >> ' + shlex.quote(str(log)) + '\nexit 1\n')
                browse.chmod(0o700)
                argv = ['/bin/sh', str(steps), 'welcome', str(browse), '127.0.0.1:9100', str(root)]
                first = subprocess.run(argv, capture_output=True, env={})
                self.assertEqual(first.returncode, 1)
                self.assertTrue((root / 'stopped').exists())
                before = log.read_bytes()
                argv[2] = 'model'
                later = subprocess.run(argv, capture_output=True, env={})
                self.assertEqual(later.returncode, 1)
                self.assertEqual(log.read_bytes(), before)
