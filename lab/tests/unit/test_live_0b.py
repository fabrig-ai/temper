"""Live 0b's shape, budgets and held-account scripts (qa-live.md, 3.1)."""
import json
import math
from pathlib import Path
import re
import subprocess
import tempfile
import unittest

LAB = Path(__file__).resolve().parents[2]
LIVE = LAB / 'live-0b'
NAMES = {
    '{port.forgejo}': '30300',
    '{program.forgejo}': '/sample/bin/forgejo',
    '{program.forgejo-runner}': '/sample/bin/forgejo-runner',
    '{artifact.temper}': '/sample/bin/temper',
    '{proc.box.dir}': '/sample/box',
}


def canonical(value):
    return (json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False) + '\n').encode()


def render(name):
    text = (LIVE / name).read_text()
    for key, value in NAMES.items():
        text = text.replace(key, value)
    return text.replace('{{', '{').replace('}}', '}')


class Live0bTests(unittest.TestCase):
    def test_experiment_shape_and_canonical_json(self):
        raw = (LIVE / 'experiment.json').read_bytes()
        experiment = json.loads(raw)
        self.assertEqual(raw, canonical(experiment))
        self.assertEqual(experiment['format'], 'lab-experiment/1')
        self.assertEqual(experiment['name'], 'live-0b')
        self.assertIsNone(experiment['driver'])
        self.assertEqual(experiment['held'], ['chatgpt'])
        self.assertEqual([action['name'] for action in experiment['actions']],
                         ['sign-in', 'settings', 'forge-setup', 'person', 'import', 'usable', 'chat'])
        self.assertEqual([action['name'] for action in experiment['release']], ['export'])
        actions = experiment['actions'] + experiment['release']
        self.assertEqual([action['name'] for action in actions
                          if '{held.chatgpt}' in action['argv']], ['import', 'export'])
        self.assertEqual(raw.decode().count('{held.chatgpt}'), 2)
        self.assertEqual(experiment['meter']['name'], 'spend')
        self.assertEqual([check['name'] for check in experiment['checks']], ['ground', 'chat'])
        self.assertEqual(experiment['cost'], 'temper-live-0b')

        def sources(value):
            if isinstance(value, dict):
                for key, child in value.items():
                    if key == 'from':
                        yield child
                    yield from sources(child)
            elif isinstance(value, list):
                for child in value:
                    yield from sources(child)

        for name in sources(experiment):
            with self.subTest(source=name):
                self.assertTrue((LIVE / name).is_file())
        for name in re.findall(r'\{experiment\}/([^"\s]+)', raw.decode()):
            with self.subTest(script=name):
                path = LIVE / name
                self.assertTrue(path.is_file())
                if path.suffix != '.json':
                    self.assertTrue(path.stat().st_mode & 0o100)

    def test_rendered_model_and_budgets(self):
        text = render('temper.json')
        self.assertNotRegex(text, r'\{[\w-]+(?:\.[\w-]+)+\}')
        configuration = json.loads(text)
        engine = configuration['engine']
        models = {model['name']: model for model in engine['llm']['models']}
        self.assertTrue(models)
        for model in models.values():
            self.assertEqual(model['window'], 131072)
            self.assertEqual(model['prices']['unit'], 1000)
        for member in ('charters', 'templates', 'policies', 'rules'):
            self.assertNotIn(member, engine)
        self.assertEqual(engine['projects'], [])  # Forge setup creates hello after settings.
        settings = json.loads((LIVE / 'settings.json').read_bytes())
        self.assertEqual([i['setting'] for i in settings['items']], [
            dict(kind='charter', number=1), dict(kind='charter', number=2),
            dict(kind='rules'), dict(kind='policy', number=1)])
        charters = [i['set'] for i in settings['items'][:2]]
        for charter in charters:
            with self.subTest(charter=charter['number']):
                self.assertEqual(charter['spend'], 280)
                model = models[charter['model']['name']]
                self.assertGreaterEqual(charter['spend'],
                                        2 * math.ceil((model['window'] + model['output']) / 1000))
        rules, policy = [i['set'] for i in settings['items'][2:]]
        self.assertEqual(rules['maximum_run_spend'], 280)
        self.assertEqual(rules['ceiling']['spend'], 600)
        self.assertEqual(policy['ceiling']['spend'], 600)
        self.assertEqual(policy['roles'][0]['authority']['spend'], 600)
        self.assertEqual(policy['permissions'], [dict(connector=1, permission=1, role=0)])
        experiment = json.loads((LIVE / 'experiment.json').read_bytes())
        self.assertEqual(experiment['actions'][1], dict(name='settings', env={}, argv=[
            '{artifact.temper}', 'ctl', '{dir.client}', 'change-settings',
            '{experiment}/settings.json', '--against', '1', '--key', 'lab-settings']))

        def periods(value):
            if isinstance(value, dict):
                for key, child in value.items():
                    if key == 'period_spend':
                        yield child
                    yield from periods(child)
            elif isinstance(value, list):
                for child in value:
                    yield from periods(child)

        spends = list(periods(settings))
        self.assertTrue(spends)
        self.assertEqual(spends, [600] * len(spends))
        agent = json.loads(render('agent.json'))
        for endpoint in agent['endpoints']:
            for model in endpoint['models']:
                self.assertIn(model['name'], models)
                self.assertEqual(model['window'], models[model['name']]['window'])
                self.assertEqual(model['prices']['unit'], models[model['name']]['prices']['unit'])


class ScriptTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.ground = self.root / 'ground'
        self.ground.mkdir()
        self.client = self.root / 'client'
        self.held = self.root / 'held.json'
        self.temper = self.root / 'temper'
        self.calls = self.root / 'calls.jsonl'

    def fake_temper(self, answers):
        # Exact argv select each fixture; unexpected calls fail without a real service.
        (self.root / 'answers.json').write_bytes(canonical(answers))
        self.temper.write_text('''#!/usr/bin/env -S python3 -I -B
import json
from pathlib import Path
import sys
root = Path(__file__).resolve().parent
with (root / 'calls.jsonl').open('a') as out:
    out.write(json.dumps(sys.argv[1:]) + '\\n')
answers = json.loads((root / 'answers.json').read_bytes())
for answer in answers:
    if sys.argv[1:] == answer['argv']:
        sys.stdout.write(answer['stdout'])
        sys.exit(answer['status'])
sys.exit(99)
''')
        self.temper.chmod(0o700)

    def answer(self, arguments, stdout='', status=0):
        return dict(argv=['ctl', str(self.client), *arguments], stdout=stdout, status=status)

    def recorded_calls(self):
        return [json.loads(line) for line in self.calls.read_text().splitlines()]

    def run_script(self, name, *arguments):
        return subprocess.run([str(LIVE / name), *map(str, arguments)],
                              capture_output=True, text=True, timeout=5)

    def test_import_retains_account_printout_and_key(self):
        arguments = ['import', str(self.held), '--key', 'lab-import']
        printout = 'moving record\nmoved in: account 7\n'
        self.fake_temper([self.answer(arguments, printout)])
        result = self.run_script('seed/import', self.temper, self.client, self.held, self.ground)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.ground / 'account').read_text(), '7\n')
        self.assertEqual((self.ground / 'import.txt').read_text(), printout)
        self.assertEqual(result.stdout, printout)
        self.assertEqual(self.recorded_calls(), [['ctl', str(self.client), *arguments]])

    def test_import_preserves_failure_status_without_account(self):
        arguments = ['import', str(self.held), '--key', 'lab-import']
        self.fake_temper([self.answer(arguments, 'import refused\n', 23)])
        result = self.run_script('seed/import', self.temper, self.client, self.held, self.ground)
        self.assertEqual(result.returncode, 23, result.stderr)
        self.assertFalse((self.ground / 'account').exists())
        self.assertEqual((self.ground / 'import.txt').read_text(), 'import refused\n')
        self.assertEqual(self.recorded_calls(), [['ctl', str(self.client), *arguments]])

    def test_export_uses_retained_account(self):
        (self.ground / 'account').write_text('7\n')
        arguments = ['export', '7', str(self.held)]
        self.fake_temper([self.answer(arguments)])
        result = self.run_script('release/export', self.temper, self.client, self.held, self.ground)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.recorded_calls(), [['ctl', str(self.client), *arguments]])

    def test_export_without_account_never_calls_temper(self):
        self.fake_temper([])
        for account in (None, ''):
            with self.subTest(account=account):
                if account is not None:
                    (self.ground / 'account').write_text(account)
                result = self.run_script('release/export', self.temper, self.client,
                                         self.held, self.ground)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse(self.calls.exists())

    def test_meter_sums_pool_rows_across_projects_live_and_ended(self):
        def row(spend, funder):
            return dict(chip=dict(details=dict(owner=dict(details=dict(
                budget=dict(numbers=dict(spent=spend), funder=funder))))))

        answers = [self.answer(['me', '--json'], json.dumps(dict(
            projects=[dict(project=3), dict(project=8)])))]
        expected = [['ctl', str(self.client), 'me', '--json']]
        # Both response shapes contain pool rows and delegates with distinct spends.
        for project, spends in ((3, (2, 3, 5)), (8, (7, 11, 13))):
            for ended in (False, True):
                arguments = ['tasks', '--project', str(project)]
                if ended:
                    arguments.append('--ended')
                    value = dict(live=[row(spends[1], dict(pool=project)),
                                       row(101, dict(task=42))],
                                 ended=dict(rows=dict(rows=[row(spends[2], dict(pool=project)),
                                                            row(103, dict(task=42))])))
                else:
                    value = dict(tasks=dict(rows=[row(spends[0], dict(pool=project)),
                                                  row(107, dict(task=42))]))
                arguments.append('--json')
                answers.append(self.answer(arguments, json.dumps(value)))
                expected.append(['ctl', str(self.client), *arguments])
        self.fake_temper(answers)
        result = self.run_script('meter/spend', self.temper, self.client)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.splitlines()[-1], '41')
        self.assertEqual(self.recorded_calls(), expected)

    def conversation(self, messages):
        turn = dict(decoded=dict(messages=messages))
        event = dict(owner=dict(decoded=dict(turn=turn)))
        return dict(activity=dict(rows=[dict(event=event)]))

    def test_chat_passes_on_assistant_words(self):
        chat = self.root / 'chat.json'
        chat.write_bytes(canonical(self.conversation([
            dict(speaker='assistant', blocks=[dict(text=dict(text='I am ready.'))])])))
        result = self.run_script('checks/chat', chat)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, '')
        self.assertEqual(result.stderr, '')

    def test_chat_no_reply_for_blank_or_absent_assistant(self):
        for messages in (
                [dict(speaker='assistant', blocks=[dict(text=dict(text=' \n\t'))])],
                [dict(speaker='user', blocks=[dict(text=dict(text='Please reply.'))])]):
            with self.subTest(messages=messages):
                chat = self.root / 'chat.json'
                chat.write_bytes(canonical(self.conversation(messages)))
                result = self.run_script('checks/chat', chat)
                self.assertEqual(result.returncode, 1, result.stderr)
                self.assertEqual(result.stderr, '')
                lines = [json.loads(line) for line in result.stdout.splitlines()]
                self.assertEqual(len(lines), 1)
                self.assertEqual(set(lines[0]), {'fail', 'summary'})
                self.assertEqual(lines[0]['fail'], 'no-reply')
                self.assertIsInstance(lines[0]['summary'], str)
                self.assertTrue(lines[0]['summary'])

    def test_chat_unreadable_input_is_undecided(self):
        chat = self.root / 'chat.json'
        for text in (None, '{broken json'):
            with self.subTest(text=text):
                if text is not None:
                    chat.write_text(text)
                result = self.run_script('checks/chat', chat)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertEqual(result.stderr, '')
                self.assertEqual([json.loads(line) for line in result.stdout.splitlines()],
                                 [dict(undecided='chat unreadable')])

    def test_chat_wrong_argument_count_is_undecided(self):
        for arguments in ((), ('one', 'two')):
            with self.subTest(arguments=arguments):
                result = self.run_script('checks/chat', *arguments)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertEqual(result.stderr, '')
                self.assertEqual([json.loads(line) for line in result.stdout.splitlines()],
                                 [dict(undecided='usage: chat CHAT_JSON')])

    def test_ground_pass(self):
        temper = self.root / 'temper-collection'
        forge = self.root / 'forge-collection'
        temper.mkdir()
        forge.mkdir()
        for directory, name, value in (
                (temper, 'forges', dict(forges=[dict(number=1, mode='owned', state='connected',
                    first=dict(org='work', repository='hello', project=dict(new='hello'), state='made'))])),
                (temper, 'me', dict(projects=[dict(name=dict(bytes='hello'))])),
                (forge, 'tasks', dict(workflow_runs=[dict(run_number=1, workflow_id='ci.yml',
                    event='push', head_branch='main', status='success')])),
                (forge, 'status', dict(state='success')),
                (forge, 'repository', dict(full_name='work/hello', private=True, default_branch='main'))):
            (directory / (name + '.json')).write_bytes(canonical(value))
        answer = self.ground / 'forge-setup.txt'
        answer.write_text("Adopted work/hello on forge 1 as owned.\nIt is the project's home.\n")
        result = self.run_script('checks/ground', temper, forge, answer)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, '')
        self.assertEqual(result.stderr, '')


if __name__ == '__main__':
    unittest.main()
