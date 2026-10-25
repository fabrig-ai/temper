"""A missed welcome step stops before any chat, under the real POSIX shell."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

STEPS = Path(__file__).resolve().parents[2] / 'welcome-chatgpt/steps'


class WelcomeWalk(unittest.TestCase):
    def test_missed_chatgpt_button_stops_before_chat(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            log = root / 'calls.jsonl'
            browse = root / 'browse'
            browse.write_text('''#!/usr/bin/python3
import json
from pathlib import Path
import sys
args = sys.argv[2:]
with Path(%r).open('a') as output:
    output.write(json.dumps(args) + '\\n')
sys.exit(1 if args[:3] == ['wait', 'button', 'Continue with ChatGPT'] else 0)
''' % str(log))
            browse.chmod(0o700)
            result = subprocess.run(['/bin/sh', str(STEPS), 'model', str(browse),
                                     '127.0.0.1:9100', str(root)],
                                    capture_output=True, text=True, env={})
            calls = [json.loads(line) for line in log.read_text().splitlines()]
            self.assertEqual(result.returncode, 1)
            self.assertEqual(calls[-3:], [
                ['wait', 'button', 'Continue with ChatGPT', '--exact', '--timeout', '120'],
                ['outline'], ['shot']])
            self.assertFalse(any(call[0] in {'press', 'type', 'reload'} for call in calls))
            self.assertFalse((root / 'chat').exists())
