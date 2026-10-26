"""Live 0a's shape, rendered configuration and retained ground (qa-live.md, 3.1)."""
import json
from pathlib import Path
import re
import subprocess
import tempfile
import unittest

LAB = Path(__file__).resolve().parents[2]
LIVE = LAB / 'live-0a'
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


class Live0aTests(unittest.TestCase):
    def test_experiment_shape_and_canonical_json(self):
        raw = (LIVE / 'experiment.json').read_bytes()
        experiment = json.loads(raw)
        self.assertEqual(raw, canonical(experiment))
        self.assertEqual(experiment['format'], 'lab-experiment/1')
        self.assertEqual(experiment['name'], 'live-0a')
        self.assertIsNone(experiment['driver'])
        self.assertNotIn('cluster', experiment)
        self.assertEqual(len(experiment['processes']), 1)
        box = experiment['processes'][0]
        self.assertEqual(box['name'], 'box')
        self.assertEqual(box['argv'], [
            '{artifact.temper}', 'box', '--config', '{proc.box.dir}/config',
            '--data', '{proc.box.dir}/data', '--cache', '{proc.box.dir}/cache'])
        self.assertEqual(box['env'], dict(TEMPER_ECHO='steps'))
        self.assertEqual(box['bootstrap'], dict(action='sign-in', stream='stdout'))
        self.assertEqual([action['name'] for action in experiment['actions']],
                         ['sign-in', 'forge-setup', 'person'])
        self.assertEqual([collection['name'] for collection in experiment['collect']],
                         ['temper', 'forge'])
        for collection in experiment['collect']:
            self.assertNotIn('cluster', collection)
            self.assertTrue(collection['argv'])
            self.assertEqual(collection['scan'], [])
        self.assertEqual([check['name'] for check in experiment['checks']], ['ground'])
        self.assertEqual(experiment['checks'][0]['argv'], [
            '{experiment}/checks/ground', '{collected.temper}', '{collected.forge}',
            '{dir.ground}/forge-setup.txt'])
        for entry in box['files']:
            with self.subTest(source=entry['from']):
                self.assertTrue((LIVE / entry['from']).is_file())
        for name in re.findall(r'\{experiment\}/([^"\s]+)', raw.decode()):
            with self.subTest(script=name):
                path = LIVE / name
                self.assertTrue(path.is_file())
                self.assertTrue(path.stat().st_mode & 0o100)

    def test_temper_template_renders_owned_forge_and_worker(self):
        text = render('temper.json')
        self.assertNotRegex(text, r'\{[\w-]+(?:\.[\w-]+)+\}')
        configuration = json.loads(text)
        for member in ('charters', 'templates', 'policies', 'rules'):
            self.assertNotIn(member, configuration['engine'])
        self.assertFalse((LIVE / 'settings.json').exists())
        application = configuration['application']
        self.assertEqual(len(application['setup']), 1)
        setup = application['setup'][0]
        self.assertEqual(setup['forge'], 1)
        self.assertEqual(setup['start'], 'asked')
        self.assertEqual(set(setup['mode']), {'owned'})
        owned = setup['mode']['owned']
        self.assertEqual(owned['forgejo'], NAMES['{program.forgejo}'])
        self.assertEqual(owned['runner']['binary'], NAMES['{program.forgejo-runner}'])
        self.assertEqual(owned['listen'], dict(socket='127.0.0.1:30300'))
        forge = application['forges'][0]
        self.assertEqual(forge['api'], dict(socket='127.0.0.1:30300'))
        self.assertEqual(forge['git'], 'http://127.0.0.1:30300')
        self.assertIsNone(application['demo'])
        for service in ('clients', 'webhooks'):
            self.assertEqual(application[service]['listener'], dict(socket='127.0.0.1:0'))
        agent = configuration['workers'][0]['agent']
        self.assertEqual(agent['program'], NAMES['{artifact.temper}'])
        self.assertEqual(agent['configuration'], '/sample/box/config/agent.json')

    def test_other_templates_render_without_lab_names(self):
        for name in ('agent.json', 'worker.secret', 'worker.digest'):
            with self.subTest(file=name):
                text = render(name)
                self.assertNotRegex(text, r'\{[\w-]+(?:\.[\w-]+)+\}')
                if name == 'agent.json':
                    self.assertIsInstance(json.loads(text), dict)


class GroundTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        self.temper = root / 'temper'
        self.forge = root / 'forge'
        self.temper.mkdir()
        self.forge.mkdir()
        self.answer = root / 'forge-setup.txt'
        # Only fields the check needs, copied from prep/run4's retained answers.
        self.forges = dict(forges=[dict(
            number=1, mode='owned', state='connected',
            first=dict(org='work', repository='hello', project=dict(new='hello'), state='made'))])
        self.me = dict(projects=[dict(name=dict(bytes='Project 1')),
                                 dict(name=dict(bytes='hello'))])
        self.tasks = dict(workflow_runs=[dict(
            run_number=1, workflow_id='ci.yml', event='push', head_branch='main', status='success')])
        self.status = dict(state='success')
        self.repository = dict(full_name='work/hello', private=True, default_branch='main')
        self.printout = ('setting up\nAdopted work/hello on forge 1 as owned.\n'
                         "It is the project's home.\n")

    def write_fixture(self):
        for directory, name, value in (
                (self.temper, 'forges', self.forges), (self.temper, 'me', self.me),
                (self.forge, 'tasks', self.tasks), (self.forge, 'status', self.status),
                (self.forge, 'repository', self.repository)):
            (directory / (name + '.json')).write_bytes(canonical(value))
        self.answer.write_text(self.printout)

    def run_ground(self, *arguments):
        return subprocess.run([str(LIVE / 'checks/ground'), *map(str, arguments)],
                              capture_output=True, text=True, timeout=5)

    def judge_fixture(self):
        self.write_fixture()
        return self.run_ground(self.temper, self.forge, self.answer)

    def assert_failure(self, signature):
        result = self.judge_fixture()
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual(result.stderr, '')
        lines = [json.loads(line) for line in result.stdout.splitlines()]
        self.assertEqual(len(lines), 1, lines)
        self.assertEqual(set(lines[0]), {'fail', 'summary'})
        self.assertEqual(lines[0]['fail'], signature)
        self.assertIsInstance(lines[0]['summary'], str)
        self.assertTrue(lines[0]['summary'])

    def test_pass(self):
        result = self.judge_fixture()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, '')
        self.assertEqual(result.stderr, '')

    def test_forge_not_connected(self):
        self.forges['forges'][0]['state'] = 'disconnected'
        self.assert_failure('forge/not-connected')

    def test_adoption_missing_first_not_made(self):
        self.forges['forges'][0]['first']['state'] = 'pending'
        self.assert_failure('adoption/missing')

    def test_adoption_missing_printout_line(self):
        self.printout = self.printout.replace('Adopted work/hello on forge 1 as owned.\n', '')
        self.assert_failure('adoption/missing')

    def test_project_missing(self):
        self.me['projects'].pop()
        self.assert_failure('project/missing')

    def test_repository_missing(self):
        self.repository['full_name'] = 'work/other'
        self.assert_failure('repository/missing')

    def test_repository_shape(self):
        self.repository['private'] = False
        self.assert_failure('repository/shape')

    def test_actions_first_run_missing(self):
        self.tasks['workflow_runs'] = []
        self.assert_failure('actions/first-run/missing')

    def test_actions_first_run_failure(self):
        self.tasks['workflow_runs'][0]['status'] = 'failure'
        self.assert_failure('actions/first-run/failure')

    def test_status_failure(self):
        self.status['state'] = 'failure'
        self.assert_failure('status/failure')

    def test_missing_input_is_undecided(self):
        for path, name in (
                (self.temper / 'forges.json', 'forges'), (self.temper / 'me.json', 'me'),
                (self.forge / 'tasks.json', 'tasks'), (self.forge / 'status.json', 'status'),
                (self.forge / 'repository.json', 'repository'), (self.answer, 'forge-setup')):
            with self.subTest(input=name):
                self.write_fixture()
                path.unlink()
                result = self.run_ground(self.temper, self.forge, self.answer)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertEqual(result.stderr, '')
                self.assertEqual([json.loads(line) for line in result.stdout.splitlines()],
                                 [dict(undecided=name + ' unreadable')])

    def test_wrong_argument_count_is_undecided(self):
        self.write_fixture()
        arguments = [self.temper, self.forge, self.answer, self.answer]
        for count in (0, 1, 2, 4):
            with self.subTest(count=count):
                result = self.run_ground(*arguments[:count])
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertEqual(result.stderr, '')
                lines = [json.loads(line) for line in result.stdout.splitlines()]
                self.assertEqual(len(lines), 1, lines)
                self.assertEqual(set(lines[0]), {'undecided'})
                self.assertIsInstance(lines[0]['undecided'], str)
                self.assertTrue(lines[0]['undecided'])


if __name__ == '__main__':
    unittest.main()
