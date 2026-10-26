"""QA fixture contracts; no service startup, credential handoff or store access."""
import copy
import json
import subprocess
import sys
import tempfile
from unittest.mock import patch
from pathlib import Path
import re
import unittest

LAB = Path(__file__).resolve().parents[2]
CLUSTER = LAB / 'cluster'
SMOKE = LAB / 'chat-change-smoke'
sys.path.insert(0, str(SMOKE / 'smoke'))
import _runtime_llm

FORGE = '{proc.forge.ready.address}'
KINDS = [1, 2, 3, 4, 7, 8, 10, 11, 12, 13]


def read(path):
    # Lab templates escape literal JSON braces; leave substitutions as strings.
    return json.loads(path.read_text().replace('{{', '{').replace('}}', '}'))


class AssemblyTests(unittest.TestCase):
    def test_s_inherits_canonical_templates_without_setup_or_worker_exceptions(self):
        for name in ('agent.json', 'llm.json', 'settings.json', 'forge.json',
                     'engine.json', 'w1.json', 'w2.json',
                     'w1.secret', 'w1.digest', 'w2.secret', 'w2.digest'):
            with self.subTest(name=name):
                self.assertEqual((SMOKE / name).read_bytes(), (CLUSTER / name).read_bytes())

    def test_stock_documents_and_settings_action_order(self):
        family = ('cluster', 'chat-change', 'chat-change-killed', 'chat-change-smoke',
                  'cluster-web', 'look-live', 'live-0b')
        for name in (*family, 'live-0a'):
            root = LAB / name
            with self.subTest(experiment=name):
                document = 'temper.json' if name.startswith('live-') else 'engine.json'
                engine = read(root / document)['engine']
                for member in ('charters', 'templates', 'policies', 'rules'):
                    self.assertNotIn(member, engine)
                for filename in ('chat.txt', 'coding.txt'):
                    self.assertFalse((root / filename).exists())
                exp = json.loads((root / 'experiment.json').read_bytes())
                for process in exp['processes']:
                    for entry in process.get('files', []):
                        self.assertNotIn(entry.get('from'), ('chat.txt', 'coding.txt'))
                if name in ('cluster', 'live-0a'):
                    self.assertFalse(any(a['name'] == 'settings' for a in exp['actions']))
                else:
                    self.assertEqual(exp['actions'][0]['name'], 'sign-in')
                    self.assertEqual(exp['actions'][1], dict(name='settings', env={}, argv=[
                        '{artifact.temper}', 'ctl', '{dir.client}', 'change-settings',
                        '{experiment}/settings.json', '--against', '1', '--key', 'lab-settings']))
                self.assertEqual((root / 'settings.json').exists(), name != 'live-0a')
                if name in family:
                    data = (root / 'settings.json').read_bytes()
                    self.assertEqual(data, (json.dumps(json.loads(data), sort_keys=True,
                                     separators=(',', ':'), ensure_ascii=False) + '\n').encode())

    def test_all_remote_workers_admit_their_configured_agent_turns(self):
        # Startup requires equality; the first frozen S failed before sign-in
        # because the agent admitted eight turns while its worker allowed one.
        for root in (CLUSTER, SMOKE, LAB / 'cluster-web', LAB / 'look-live'):
            agent = read(root / 'agent.json')
            turns = agent['profile']['policy']['max_turns']
            self.assertEqual(turns, 8)
            for name in ('w1', 'w2'):
                with self.subTest(root=root.name, worker=name):
                    worker, = read(root / (name + '.json'))['workers']
                    self.assertEqual(worker['limits']['overrides']['hub.turns'], turns)
                    self.assertEqual(worker['slots'], 1)
                    app = read(root / (name + '.json'))['application']['workers'][0]
                    self.assertEqual(app, dict(root='workspaces', git='{artifact.git}'))

    def test_canonical_records_only_setup_uses_the_actual_forge(self):
        app = read(CLUSTER / 'engine.json')['application']
        forge, = app['forges']
        self.assertEqual(forge, dict(number=1, kind='forgejo', api=dict(socket=FORGE),
                                    transport='plaintext', git='http://' + FORGE))
        self.assertNotIn('token', app['identities'][0])
        self.assertEqual(app['identities'][0]['forge'], 1)
        setup, = app['setup']
        self.assertEqual(setup['directory'], '{proc.forge.dir}/setup')
        self.assertEqual(setup['mode']['fake']['listen'], dict(socket=FORGE))
        self.assertEqual(setup['mode']['fake']['git'], '{artifact.git}')
        self.assertEqual(app['webhooks'], dict(listener=dict(socket='127.0.0.1:0')))
        for root in (CLUSTER, SMOKE, LAB / 'cluster-web', LAB / 'look-live'):
            exp = read(root / 'experiment.json')
            self.assertFalse(any('reconcile' in action['argv'] for action in exp['actions']))
            for process in exp['processes']:
                self.assertFalse(any(entry.get('from', '').startswith('forge-1.')
                                     for entry in process.get('files', [])))
            self.assertFalse(list(root.glob('forge-1.*')))

    def test_tls_root_and_model_agree_for_engine_and_both_agents(self):
        for root in (CLUSTER, SMOKE):
            llm = read(root / 'llm.json')
            engine = read(root / 'engine.json')['engine']['llm']
            agent = read(root / 'agent.json')['endpoints'][0]
            endpoint = engine['endpoints'][0]
            self.assertEqual(llm['tls']['root'], endpoint['trust'])
            self.assertEqual(agent['trust_der'], endpoint['trust'])
            self.assertEqual(endpoint['trust'], '{proc.llm.dir}/root.der')
            for item in (endpoint, agent):
                self.assertEqual((item['transport'], item['dialect'], item['server_name']),
                                 ('tls', 3, 'skein.test'))
            self.assertEqual(endpoint['address']['socket'], agent['address'])
            self.assertEqual(agent['address'], '{proc.llm.ready.address}')
            for model in (llm['models'][0], engine['models'][0], agent['models'][0]):
                self.assertEqual((model['name'], model['output'], model['window']),
                                 ('fake', 8192, 1048576))

    def test_old_chat_scripts_and_wait_window_are_preserved(self):
        llm = read(CLUSTER / 'llm.json')
        for ordinal in ('first', 'second'):
            script = next(s for s in llm['cued'] if s['cue'] == f'cluster-{ordinal}-chat')
            turns = script['turns']
            word = ordinal.capitalize()
            self.assertEqual(turns[0]['lines'], [dict(text=f'{word} worker replied.'),
                                               dict(call='wait', arguments={})])
            self.assertEqual(turns[1]['lines'], [dict(text=f'{word} chat closed.')])
            self.assertEqual([t['finish'] for t in turns], ['tool_calls', 'stop'])
            self.assertTrue(all(t['tokens'] == 1 for t in turns))
        chat = json.loads((CLUSTER / 'settings.json').read_bytes())['items'][0]['set']
        self.assertEqual((chat['waiting'], chat['time'], chat['reserve_time']),
                         (120_000_000_000, 130_000_000_000, 10_000_000_000))

    def test_notice_delegates_change_to_coding_then_reviews_named_head(self):
        scripts = {s['cue']: s['turns'] for s in read(CLUSTER / 'llm.json')['cued']}
        chat = scripts['Add a file NOTICE to widget']
        batch = chat[0]['lines'][0]
        self.assertEqual(batch['call'], 'delegate')
        change, = batch['arguments']['batch']
        self.assertEqual(change['executor'], dict(kind='procedure', connector=1, code=2))
        self.assertEqual(change['spec'], dict(words='@coding NOTICE', parameters=[
            dict(name=1, kind='resource', connector=1, resource=4294967297),
            dict(name=2, kind='bytes', value='main')]))
        self.assertEqual(change['contract'], dict(kind='change', connector=1, change_kind=1, words=128))
        self.assertEqual(change['authority']['delegation'], dict(
            kinds=[dict(kind='agent', number=2)], tasks=4, depth=2))
        self.assertEqual(chat[1]['lines'], [dict(text='I asked for the change; its task follows it.'),
                                          dict(call='wait', arguments={})])
        self.assertEqual(chat[2]['lines'], [dict(text="The change's task has ended; it says how.")])
        self.assertEqual([t['finish'] for t in chat], ['tool_calls', 'tool_calls', 'stop'])
        coding = scripts['@coding NOTICE']
        self.assertEqual(coding[0]['lines'], [dict(call='write', arguments=dict(
            path='NOTICE', content=(SMOKE / 'NOTICE').read_text()))])
        self.assertEqual(coding[1]['lines'], [dict(call='shell', arguments=dict(
            command='test -f NOTICE && grep -q Maintained NOTICE'))])
        self.assertEqual(coding[2]['lines'], [dict(call='finish', arguments=dict(
            form='change', fields=dict(title='Add NOTICE', body='Adds NOTICE, as asked.')))])
        review = scripts['Review this change at the named head']
        self.assertEqual(review[0]['lines'], [dict(call='finish', arguments=dict(
            form='verdict', label='1', text='Checked', fields={}, items=[]))])
        self.assertTrue(all(t['tokens'] == 20 for t in chat))
        for turns in (coding, review):
            self.assertTrue(all(t['tokens'] == 20 and t['finish'] == 'tool_calls' for t in turns))

    def test_delegated_authority_is_under_project_and_rules_ceiling(self):
        engine = read(CLUSTER / 'engine.json')['engine']
        # Project grants remain narrow; the stock rules cover all forge hosts.
        self.assertNotIn('rules', engine)
        ceiling = engine['projects'][0]['policy']['ceiling']
        self.assertEqual([g['kind'] for g in ceiling['grants']], KINDS)
        self.assertEqual(ceiling['delegation']['kinds'], [dict(kind='charter', number=1),
            dict(kind='charter', number=2), dict(kind='procedure', number=2)])
        for g in ceiling['grants']:
            self.assertEqual(g['connector'], 1)
            self.assertEqual(g['pattern'], dict(segments=['forge', FORGE], last='open', value='fake'))
        for project in engine['projects']:
            policy = project['policy']
            self.assertEqual(policy['ceiling'], ceiling)
            self.assertEqual(policy['projections'], ceiling['grants'])
            self.assertEqual(policy['roles'][0]['authority']['grants'], ceiling['grants'])
        turns = next(s['turns'] for s in read(CLUSTER / 'llm.json')['cued']
                     if s['cue'] == 'Add a file NOTICE to widget')
        authority = turns[0]['lines'][0]['arguments']['batch'][0]['authority']
        self.assertEqual([g['kind'] for g in authority['grants']], [2, 3, 4, 10, 11, 12])
        for grant in authority['grants']:
            self.assertIn(dict(connector=grant['connector'], kind=grant['kind'], pattern=dict(
                segments=grant['segments'], last=grant['terminal'], value=grant['last'])), ceiling['grants'])
        self.assertLessEqual(authority['budget']['spend'], 10)  # Stock maximum run spend.

    def test_landing_requires_ci_up_to_date_and_exact_review(self):
        app = read(CLUSTER / 'engine.json')['application']
        rule, = app['landing']['projects'][0]['rules']
        self.assertEqual(rule, dict(connector=1, kind=4, pattern=dict(
            segments=['forge', FORGE, 'fake', 'widget', 'branch'], last='exact', value='main'),
            enforce=True, ci=True, up_to_date=True,
            gates=[dict(number=1, blocking=True, freshness='exact')], approvals=[]))
        # Shipped polling/window defaults are selected by omission.
        overrides = app['limits']['overrides']
        for suffix in ('poll_ms', 'poll_max_ms', 'slow_ms', 'hinted_ms', 'window_ms'):
            self.assertNotIn('forge.client.' + suffix, overrides)
        forge = read(SMOKE / 'forge.json')
        self.assertEqual(forge['repositories'][0], dict(name='widget', landing='main',
            ci=dict(path='NOTICE', green=(SMOKE / 'NOTICE').read_text().strip())))

    def test_two_remote_charters_and_capacities_cover_the_chain(self):
        config = read(CLUSTER / 'engine.json')
        engine = config['engine']
        self.assertEqual(engine['limits']['overrides']['fleet.engine_slots'], 0)
        self.assertEqual(config['application']['workers'], [])
        self.assertEqual(config['application']['charter'], 2)
        items = json.loads((CLUSTER / 'settings.json').read_bytes())['items']
        self.assertEqual([i['setting'] for i in items], [dict(kind='charter', number=1),
                         dict(kind='charter', number=2), dict(kind='template', number=1)])
        charters = [i['set'] for i in items[:2]]
        for charter, filename in zip(charters, ('chat.txt', 'coding.txt')):
            self.assertEqual(charter['hosts'], 'workers')
            self.assertEqual(charter['instructions'],
                             (LAB.parent / 'crates/temper-service/src/stock' / filename).read_text())
        self.assertTrue(all(charters[1][flag] for flag in ('inspect', 'modify', 'shell')))
        self.assertEqual(items[2]['set']['budget'], dict(as_allowed=dict(cap=50)))
        limits = engine['limits']['overrides']
        for key, needed in {'tasks.tasks': 4, 'tasks.charters': 2, 'tasks.tree_tasks': 4,
                            'tasks.parameters': 2, 'tasks.authority_grants': len(KINDS),
                            'authority.grants': len(KINDS), 'authority.requirements': 3,
                            'documents.rows': 32, 'tasks.result_bytes': 128}.items():
            self.assertGreaterEqual(limits[key], needed, key)
        for name in ('w1', 'w2'):
            worker, = read(CLUSTER / (name + '.json'))['workers']
            self.assertEqual(worker['slots'], 1)
            self.assertEqual(worker['limits']['overrides']['hub.slots'], 1)
            self.assertGreaterEqual(worker['limits']['overrides']['hub.event_bytes'], limits['tasks.message_bytes'])

    def test_canonical_keeps_case_two_forge_and_process_harness_link(self):
        exp = read(CLUSTER / 'experiment.json')
        self.assertEqual([p['name'] for p in exp['processes']], ['forge', 'llm', 'engine', 'w1', 'w2'])
        for field in ('actions', 'checks', 'collect', 'dirs', 'faults'):
            self.assertEqual(exp[field], [])
        engine = next(p for p in exp['processes'] if p['name'] == 'engine')
        self.assertFalse(any('charters/' in entry['path'] for entry in engine['files']))
        self.assertIn(dict(stream='stdout', prefix='http://', capture='link'), engine['ready'])
        self.assertIn(dict(stream='stderr', prefix='listening webhooks ', capture='webhooks'), engine['ready'])
        forge, llm = exp['processes'][:2]
        self.assertEqual(forge['argv'][forge['argv'].index('--seed') + 1], '17')
        self.assertEqual(llm['after'], ['forge'])
        self.assertNotIn('--seed', llm['argv'])

    def test_s_process_contract_adapts_fake_seeds_private_bootstrap_and_runtime_llm(self):
        canonical = read(CLUSTER / 'experiment.json')['processes']
        smoke = read(SMOKE / 'experiment.json')['processes']
        for process in canonical:
            if process['name'] == 'forge':
                process['argv'][process['argv'].index('--seed') + 1] = '{seed.forge}'
            elif process['name'] == 'llm':
                process['argv'] = ['/usr/bin/python3', '-I', '-B', '{experiment}/smoke/_runtime_llm.py',
                                   *process['argv'], '--seed', '{seed.llm}']
            elif process['name'] == 'engine':
                process['ready'].remove(dict(stream='stdout', prefix='http://', capture='link'))
                process['bootstrap'] = dict(stream='stdout', action='sign-in')
        self.assertEqual(smoke, canonical)

    def test_runtime_llm_changes_only_notice_result_wake_after_rendering(self):
        canonical = (CLUSTER / 'llm.json').read_bytes()
        original = read(SMOKE / 'llm.json')
        expected = copy.deepcopy(original)
        notice = next(s for s in expected['cued'] if s['cue'] == _runtime_llm.CUE)
        notice['turns'][2]['finish'] = 'tool_calls'
        notice['turns'][2]['lines'] = [
            {'text': "The change's task has ended; it says how."}, {'call': 'wait', 'arguments': {}}]
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'llm.json'
            source.write_text(json.dumps(original).replace('{proc.llm.dir}', directory).replace(
                '{proc.forge.ready.address}', '127.0.0.1:7602'))
            source_bytes = source.read_bytes()
            argv = ['/subject/temper', 'fake', 'llm', '--document', str(source),
                    '--ready', 'listening ', '--seed', '17']
            with patch.object(_runtime_llm.os, 'execv') as execute:
                _runtime_llm.main(argv)
            runtime = source.with_name('llm.s-runtime.json')
            expected = json.loads(json.dumps(expected).replace('{proc.llm.dir}', directory).replace(
                '{proc.forge.ready.address}', '127.0.0.1:7602'))
            self.assertEqual(json.loads(runtime.read_bytes()), expected)
            self.assertEqual(source.read_bytes(), source_bytes)
            self.assertEqual(runtime.stat().st_mode & 0o777, 0o600)
            execute.assert_called_once_with(argv[0], [*argv[:4], str(runtime), *argv[5:]])
            # Exercise the manifest interpreter/exec boundary with a local
            # canary, without starting temper, a model or any network peer.
            runtime.unlink()
            canary = Path(directory) / 'fake-subject'
            canary.write_text('#!/usr/bin/python3\nimport json,sys\nfrom pathlib import Path\n'
                'print(json.dumps([sys.argv[1:],json.loads(Path(sys.argv[4]).read_bytes())]))\n')
            canary.chmod(0o700)
            called = subprocess.run(['/usr/bin/python3', '-I', '-B', str(SMOKE / 'smoke/_runtime_llm.py'),
                str(canary), *argv[1:]], capture_output=True, timeout=5, check=True, env={})
            recorded_argv, recorded_document = json.loads(called.stdout)
            self.assertEqual(recorded_argv, ['fake', 'llm', '--document', str(runtime), *argv[5:]])
            self.assertEqual(recorded_document, expected)
            self.assertEqual(called.stderr, b'')
        self.assertEqual((SMOKE / 'llm.json').read_bytes(), canonical)
        for mutation in ('extra-turn', 'wrong-finish', 'duplicate-cue'):
            document = read(SMOKE / 'llm.json')
            notice = next(s for s in document['cued'] if s['cue'] == _runtime_llm.CUE)
            if mutation == 'extra-turn':
                notice['turns'].append(notice['turns'][2])
            elif mutation == 'wrong-finish':
                notice['turns'][2]['lines'][0]['call'] = 'shell'
            else:
                document['cued'].append(copy.deepcopy(notice))
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                _runtime_llm.adapt(document)

    def test_s_process_templates_have_ready_predecessors_and_regular_inputs(self):
        exp = read(SMOKE / 'experiment.json')
        names = {p['name'] for p in exp['processes']}
        ready, ancestors = {}, {}
        for process in exp['processes']:
            name = process['name']
            before = set(process.get('after', []))
            self.assertLessEqual(before, set(ancestors))
            ancestors[name] = before | set().union(*(ancestors[p] for p in before))
            ready[name] = {r['capture'] for r in process['ready']}
            for entry in process['files']:
                source = SMOKE / entry['from']
                self.assertTrue(source.is_file() and not source.is_symlink())
                for owner, capture in re.findall(r'\{proc\.([^.{}]+)\.ready\.([^.{}]+)\}', source.read_text()):
                    self.assertIn(owner, ancestors[name], (name, owner))
                    self.assertIn(capture, ready[owner])
            self.assertEqual(process['env'], {})
        self.assertEqual(names, {'forge', 'llm', 'engine', 'w1', 'w2'})

    def test_s_seed_private_signin_and_runner_check_inputs(self):
        exp = read(SMOKE / 'experiment.json')
        self.assertEqual(exp['format'], 'lab-experiment/1')
        self.assertIsNone(exp['driver'])
        self.assertEqual(exp['faults'], [])
        self.assertEqual(exp['planted'], [])
        self.assertEqual([a['name'] for a in exp['actions']],
                         ['sign-in', 'settings', 'add-key', 'usable', 'forge', 'adopt', 'story'])
        self.assertEqual(exp['actions'][0]['argv'][-2:], ['sign-in', '-'])
        self.assertFalse(any('stdin' in a for a in exp['actions']))
        self.assertNotIn('await', exp['actions'][0])
        engine = next(p for p in exp['processes'] if p['name'] == 'engine')
        self.assertEqual(engine['bootstrap'], dict(stream='stdout', action='sign-in'))
        self.assertFalse(any(r['stream'] == 'stdout' for r in engine['ready']))
        for process in exp['processes']:
            self.assertFalse(any(r['capture'] == 'link' for r in process['ready']))
        for action in exp['actions'][3:5]:
            self.assertEqual(action['argv'][-2:], ['--timeout', '200'])
        self.assertEqual(exp['actions'][-1]['argv'][-1], 'http://{proc.engine.ready.clients}/')
        self.assertIn('STORY_SECONDS = 220', (SMOKE / 'smoke/_story.py').read_text())
        self.assertEqual(exp['collect'], [
            dict(name='forge', argv=['{experiment}/checks/end-state', 'collect',
                '{proc.forge.dir}/observations.jsonl', '{proc.forge.ready.address}', '{collect.dir}'],
                env={'PATH': '/usr/bin:/bin'}, scan=[]),
            dict(name='observations', cluster='stopped', argv=['/bin/cp', '--',
                '{proc.forge.dir}/observations.jsonl', '{collect.dir}/observations.jsonl'], env={}, scan=[]),
            dict(name='store', cluster='stopped', argv=['/bin/cp', '-R', '--',
                '{proc.engine.dir}/data/state/store/.', '{collect.dir}/.'], env={}, scan=[]),
        ])
        self.assertEqual([c['name'] for c in exp['checks']], ['end-state', 'promises'])
        end = next(c for c in exp['checks'] if c['name'] == 'end-state')
        self.assertEqual(end['argv'], ['{experiment}/checks/end-state',
            'judge', '{collected.forge}'])
        self.assertNotIn('cluster', end)
        promises = next(c for c in exp['checks'] if c['name'] == 'promises')
        self.assertEqual(promises['cluster'], 'stopped')
        self.assertEqual(promises['argv'][-3:], ['1', '{collected.observations}/observations.jsonl',
                                              '{collected.store}'])
        # No credential template anywhere in the executable assembly.
        text = (SMOKE / 'experiment.json').read_text()
        self.assertNotIn('ready.link', text)
        self.assertNotIn('host', text.lower())
        for filename in ('host', 'host.py', '_host_native.py', '_host_output.py'):
            self.assertFalse((SMOKE / filename).exists())
        self.assertNotIn('secret=', text)
        self.assertNotIn('--boundary', ' '.join(end['argv']))
