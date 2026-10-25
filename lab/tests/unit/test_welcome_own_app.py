"""Program evidence must join the named source to a connected worker's login."""
from copy import deepcopy
import json
from pathlib import Path
import runpy
import unittest

ROOT = Path(__file__).resolve().parents[2]
PROOF = runpy.run_path(str(ROOT / 'welcome-own-app/checks/proof'))
LOGIN = runpy.run_path(str(ROOT / 'welcome-own-app/program-login'))
NAME = 'claude-person'


def accounts():
    return {'accounts': {'usable': True, 'party': [{'sources': [
        {'kind': 'program', 'name': NAME, 'usable': True}]}],
        'program_hosts': [{'connected': True, 'programs': [{'name': NAME, 'state': 'usable'}]}]}}


class OwnAppProof(unittest.TestCase):
    def test_matching_source_and_worker_pass(self):
        self.assertEqual(PROOF['program'](accounts(), NAME), (0, []))

    def test_unrelated_usable_account_or_login_cannot_pass(self):
        for field in ['source', 'worker', 'connection', 'state', 'account']:
            with self.subTest(field=field):
                doc = deepcopy(accounts())
                a = doc['accounts']
                if field == 'source':
                    a['party'][0]['sources'][0]['name'] = 'other'
                elif field == 'worker':
                    a['program_hosts'][0]['programs'][0]['name'] = 'other'
                elif field == 'connection':
                    a['program_hosts'][0]['connected'] = False
                elif field == 'state':
                    a['program_hosts'][0]['programs'][0]['state'] = 'not_signed_in'
                else:
                    a['party'][0]['sources'][0]['usable'] = False
                self.assertEqual(PROOF['program'](doc, NAME)[0], 1)

    def test_trouble_including_sign_in_fails(self):
        self.assertEqual(PROOF['trouble']([]), (0, []))
        self.assertEqual(PROOF['trouble'](['{"kind":"network"}'])[0], 1)

    def test_both_printed_suffixes_use_the_page_name(self):
        outline = '  code "On each host, run: temper program-login claude-person"\n'
        prefix = "'/subject/temper' program-login --config '/c' --data '/d' --cache '/cache with space'"
        expected = ['/subject/temper', 'program-login', '--config', '/c', '--data', '/d', '--cache', '/cache with space', NAME]
        for suffix in [' source', ' NAME (the name the Model step shows)']:
            self.assertEqual(LOGIN['command'](prefix + suffix, outline), expected)

    def test_missing_or_ambiguous_page_name_refuses(self):
        for outline in ['', 'temper program-login a\ntemper program-login b']:
            with self.assertRaises(ValueError):
                LOGIN['command']('junk', outline)

    def test_experiments_expose_each_step_without_ctl_forge_shortcuts(self):
        for name, phases in [('welcome-chatgpt', ['welcome', 'forge', 'model', 'ready', 'chat']),
                             ('welcome-own-app', ['welcome', 'forge', 'model', 'ready'])]:
            experiment = json.loads((ROOT / name / 'experiment.json').read_text())
            self.assertEqual([a['name'] for a in experiment['actions']][-len(phases):], phases)
            self.assertFalse(any('forge-connect' in a['argv'] for a in experiment['actions']))
        own = json.loads((ROOT / 'welcome-own-app/experiment.json').read_text())
        self.assertIn('http://{proc.box.ready.github}', own['processes'][1]['argv'])
