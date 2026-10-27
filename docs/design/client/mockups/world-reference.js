// The retired first edition's world: its state at hour 0, and the steps that
// move it on (domain/testing.md, 5.3; draft/reference-case.md is the second edition). Times are minutes from hour 0,
// which is Thursday 22 October, 08:00 UTC.
//
// A world holds facts as the engine keeps them (a phase, what a task
// waits on, its wake policy), names as the deployment's configuration
// gives them (charters, templates, host kinds), and words as a person or
// an agent wrote them, each with its writer. The pages turn them into
// words; nothing in the pages names this workflow.
(function () {
  const MOCKUPS = window.MOCKUPS = window.MOCKUPS || { worlds: {} };

  function initial() {
    return {
      now: 0,
      person: 'Pat',
      project: 'factory',

      // What the deployment's configuration names.
      config: {
        templates: { chat: 'chat', goal: 'goal' },
        charters: ['Coordinator', 'Owner', 'Advisor', 'Survey', 'Implementer', 'Investigation', 'Helper'],
        period: 'week',
        allotment: 240,
        // An ops application reports workers' health (gap 6.4).
        ops: true,
        // What a new chat's first run takes.
        next: { account: 'Claude', model: 'Opus · xhigh' },
        ways: ['Continue with ChatGPT', 'Use an OpenAI API key'],
        restart: 'temper serve',
      },

      repos: {
        kit: {
          name: 'acme/kit', forge: 1, branch: 'main',
          tip: '4e1a9c',
          ci: { state: 'passing', at: -29 },
          after: { name: 'slow tier', state: 'passing', head: '4e1a9c', at: -21 },
          queue: ['T26', 'T28', 'T31'],
        },
        app: {
          name: 'acme/app', forge: 1, branch: 'main',
          tip: '91c07e',
          ci: { state: 'passing', at: -655 },
          after: { name: 'slow tier', state: 'passing', head: '91c07e', at: -630 },
          queue: [],
        },
      },

      // One forge, GitHub, through temper's App (client forge.md, 5.4).
      forges: [
        { n: 1, kind: 'GitHub', web: 'https://github.com', mode: 'connected', state: 'connected', login: 'temper-acme[bot]', id: 190244117,
          app: { slug: 'temper-acme', installs: [{ account: 'acme', id: 55120934, all: false, names: ['kit', 'app'] }] },
          load: { used: 1840, rate: 5000, window: 'hour', reserve: 500, uses: [['fresh reads', 920], ['writes', 140], ['keeping up', 610], ['the slow pass', 170]], outbox: { waiting: 2, uncertain: 0 }, hooks: [['acme/kit', -1], ['acme/app', -6]] } },
      ],

      landings: [
        { task: 'T24', repo: 'kit', at: -72 },
        { task: 'T22', repo: 'kit', at: -410, title: 'Fake server: the scripted core' },
        { task: 'T19', repo: 'app', at: -660, title: 'Retry on a dropped stream' },
      ],

      findings: [
        {
          id: 'F', repo: 'kit', state: 'open',
          title: 'reconnect_after_drop flakes',
          signature: 'fake_server::reconnect_after_drop · timed out waiting for reconnect',
          rate: '3 of the last 40 runs',
          seen: { heads: 2, tip: 0 },
          goal: 'T12', fix: 'T31', opened: -128, from: 'T14',
        },
      ],

      workers: [
        {
          name: 'ws', kind: 'ws', what: "Pat's workstation", slots: 2,
          note: 'Small. Its load is your own.',
          logins: [['claude-pat', 'signed in'], ['claude-pat-2', 'signed in'], ['chatgpt-pat', 'signed in']],
          health: { load: '0.6', builds: 0, disk: '212 GB', floor: '50 GB', memory: '48%' },
        },
        {
          name: 's1', kind: 's1', what: 'Server', slots: 7,
          logins: [['claude-pat', 'signed in'], ['chatgpt-pat', 'signed in']],
          health: { load: '0.9', builds: 6, disk: '142 GB', floor: '80 GB', memory: '61%' },
        },
      ],

      accounts: [
        {
          name: 'Claude', provider: 'Anthropic', holder: 'Pat', plan: 'Claude subscription',
          sources: [
            { name: 'claude-pat', kind: 'program', program: 'Claude Code', hosts: ['ws', 's1'], use: 'In use',
              usage: { window: 22, windowReset: 160, week: 58, weekReset: 'Mon 00:00', pace: 48 } },
            { name: 'claude-pat-2', kind: 'program', program: 'Claude Code', hosts: ['ws'], use: 'Idle',
              usage: { window: 0, windowReset: null, week: 3, weekReset: 'Mon 00:00', pace: 48 } },
          ],
        },
        {
          name: 'ChatGPT', provider: 'OpenAI', holder: 'Pat', plan: 'ChatGPT plan',
          sources: [
            { name: 'pat@example.com', kind: 'sign-in', use: 'In use' },
          ],
        },
      ],

      // Each task as the engine keeps it. `phase` is jig's; `waitsOn` is
      // what the engine computes; `wake` is its wake policy's timer or
      // subscription; `run` its live run.
      tasks: {
        T1: {
          title: 'Director', by: 'Pat', ref: 'director', template: 'chat', charter: 'Coordinator',
          model: 'Opus · xhigh', account: 'Claude', hostKind: 'ws', requester: 'Pat',
          phase: 'idle', waitsOn: 'delegates', wake: { every: 120, next: 70, words: true },
          spend: 6.2, budget: null, made: -2310,
        },
        T7: {
          title: "Owners' status digest", by: 'T1', ref: 'old shape', charter: 'Helper', parent: 'T1',
          model: 'GPT · medium', account: 'ChatGPT', hostKind: 'ws',
          phase: 'idle', waitsOn: 'news', wake: { every: 120, next: 65 },
          spend: 0.9, made: -2200,
          brief: "Every two hours, read each owner's ledger and write the director a digest.",
        },
        T8: {
          title: 'Completion checker', by: 'T1', ref: 'old shape', charter: 'Helper', parent: 'T1',
          model: 'GPT · medium', account: 'ChatGPT', hostKind: 'ws',
          phase: 'idle', waitsOn: 'news', wake: { on: 'T12' },
          spend: 0.4, made: -2200,
          brief: "When an owner reports done, check its draft's items against what landed.",
        },
        T12: {
          title: 'Harness', by: 'T1', ref: 'B', template: 'goal', charter: 'Owner', parent: 'T1',
          model: 'GPT · high', account: 'ChatGPT', hostKind: 's1', worker: 's1',
          phase: 'idle', waitsOn: 'delegates', run: { n: 6, waiting: true },
          tracked: true, priority: 1, spend: 18.6, budget: 60, made: -955,
          pool: { name: 'Codex slots', size: 3 },
          wakesOn: ["its delegates' questions and results", 'failures in its tree', 'landings that overlap what it writes', 'messages'],
          brief: "Own the draft Harness: kit's fake server, the flaky tests and the build path. Take taste from one-shot Opus advisors; bring significant decisions to the director.",
          input: 'kit · docs/design/draft/harness.md @ 7f3e2d1',
          writes: 'kit: crates/fake-server, tests/, scripts/build',
        },
        T13: {
          title: "Advise: the fake server's API", by: 'T12', charter: 'Advisor', parent: 'T12',
          model: 'Opus · xhigh', account: 'Claude', hostKind: 's1',
          phase: 'done', ended: -700, spend: 1.3, made: -760,
          brief: "Read the draft's fake server section and the three candidate shapes; recommend one, with its reasons.",
          result: 'Shape 2: a scripted server with typed replies; streams as a reply kind, not a mode.',
        },
        T14: {
          title: "Survey: kit's flaky tests", by: 'T12', charter: 'Survey', parent: 'T12',
          model: 'GPT · medium', account: 'ChatGPT', hostKind: 's1',
          phase: 'done', ended: -128, spend: 0.8, made: -300,
          brief: "Run kit's suite 40 times and list every test that fails without a change, with its signature.",
          result: 'One flake: reconnect_after_drop, 3 in 40, timed out waiting for reconnect.',
        },
        // Each change is temper's change procedure; under it, the tasks it
        // makes: here, the agent that produced its branch.
        T24: {
          title: 'Fake server: replay fixtures', by: 'T12', kind: 'change', parent: 'T12', repo: 'kit',
          stage: 'landed', made: -290, ended: -72, pr: 212, merge: '4e1a9c7',
        },
        T25: {
          title: "Produce T24's branch", by: 'T24', briefBy: 'T12', charter: 'Implementer', parent: 'T24',
          model: 'GPT · high', account: 'ChatGPT', hostKind: 's1',
          phase: 'done', made: -290, ended: -150, spend: 3.4,
          brief: 'Add replay fixtures to the fake server: record a session and replay it byte for byte, with tests.',
          result: 'Pushed temper/24/fake-server-replay-fixtures at 8b2d410.',
        },
        T26: {
          title: 'Fake server: typed errors', by: 'T12', kind: 'change', parent: 'T12', repo: 'kit',
          stage: 'first', updating: true, made: -285, pr: 215, ready: -112,
        },
        T27: {
          title: "Produce T26's branch", by: 'T26', briefBy: 'T12', charter: 'Implementer', parent: 'T26',
          model: 'GPT · high', account: 'ChatGPT', hostKind: 's1',
          phase: 'done', made: -285, ended: -125, spend: 2.9,
          brief: 'Give the fake server typed errors in place of strings, with tests for each.',
          result: 'Pushed temper/26/fake-server-typed-errors at 51ce0a7.',
        },
        T28: {
          title: 'Build path: one cache per lane', by: 'T12', kind: 'change', parent: 'T12', repo: 'kit',
          stage: 'queued', made: -280, pr: 216, ready: -80,
        },
        T29: {
          title: "Produce T28's branch", by: 'T28', briefBy: 'T12', charter: 'Implementer', parent: 'T28',
          model: 'GPT · medium', account: 'ChatGPT', hostKind: 's1',
          phase: 'done', made: -280, ended: -95, spend: 2.2,
          brief: 'One build cache per lane in scripts/build, so lanes stop evicting each other.',
          result: 'Pushed temper/28/build-path-one-cache-per-lane at c40e9b2.',
        },
        T31: {
          title: 'Fix the reconnect race', by: 'T12', ref: 'f1', kind: 'change', parent: 'T12', repo: 'kit',
          stage: 'queued', made: -128, pr: 218, ready: -12, fixes: 'F',
        },
        T32: {
          title: "Produce T31's branch", by: 'T31', briefBy: 'T12', charter: 'Implementer', parent: 'T31',
          model: 'GPT · medium', account: 'ChatGPT', hostKind: 's1',
          phase: 'done', made: -128, ended: -20, spend: 1.1,
          brief: "Fix reconnect_after_drop's race (finding F): the reconnect timer must not fire before the drop is seen. Prove it with 200 runs.",
          result: 'Pushed temper/31/fix-the-reconnect-race at e7a31d5.',
        },
        T33: {
          title: 'Benchmark: the fake server under load', by: 'T12', charter: 'Implementer', parent: 'T12',
          model: 'GPT · medium', account: 'ChatGPT', hostKind: 's1', worker: 's1',
          phase: 'running', run: { n: 1, since: -55, doing: 'a tool · shell' }, pool: true,
          spend: 1.6, made: -56,
          brief: 'Measure replies per second and tail latency at 1, 8 and 64 clients. Needs s1 quiet.',
        },
        T34: {
          title: 'Fake server: streaming replies', by: 'T12', charter: 'Implementer', parent: 'T12',
          model: 'GPT · high', account: 'ChatGPT', hostKind: 's1', worker: 's1',
          phase: 'running', run: { n: 1, since: -40, doing: 'writing' }, pool: true,
          spend: 2.4, made: -41,
          brief: 'Add streamed replies to the fake server as a reply kind (T13), with its tests. Hand back the branch.',
        },
        T35: {
          title: 'Investigate: the slow build path', by: 'T12', charter: 'Investigation', parent: 'T12',
          model: 'GPT · medium', account: 'ChatGPT', hostKind: 's1', worker: 's1',
          phase: 'running', run: { n: 1, since: -25, doing: 'a tool · shell' }, pool: true,
          spend: 0.9, made: -26,
          brief: "Find why kit's build path takes 9 minutes from clean; name the cause with evidence.",
        },
      },

      // The order of each parent's children.
      children: {
        T1: ['T12', 'T7', 'T8'],
        T12: ['T13', 'T14', 'T24', 'T26', 'T28', 'T31', 'T33', 'T34', 'T35'],
        T24: ['T25'],
        T26: ['T27'],
        T28: ['T29'],
        T31: ['T32'],
      },

      // Ended chats are rows of the ended-tasks index, not live tasks.
      ended: [
        { task: 'T3', title: 'Why did kit’s slow tier fail on Monday?', by: 'Pat', charter: 'Coordinator', template: 'chat', ended: -3900, spend: 0.7 },
        { task: 'T2', title: 'The process documents, read aloud', by: 'Pat', charter: 'Coordinator', template: 'chat', ended: -5400, spend: 1.2 },
      ],

      // The inbox: entries waiting for the person (none), and the replies
      // in their chats and the results of what they asked for.
      inbox: [],

      transcripts: {
        // An agent's task: its run's turns, the last still streaming.
        T34: [
          { who: 'tools', at: -39, calls: [['read', 6], ['recall', 1]] },
          { who: 'agent', at: -30, text: 'T13’s shape: a stream is a reply kind, not a mode. I’ll add `Reply::Stream`, its chunks and a closing status, and leave `Reply::Once` as it is.' },
          { who: 'tools', at: -29, calls: [['edit', 5], ['shell', 3]] },
          { who: 'agent', live: true, at: -1, text: 'Chunks go out in order, each after the delay its script names. The tests cover an empty stream, one chunk, and a stream cut' },
        ],
        T1: [
          { day: 'Wednesday 21 October', earlier: 142 },
          { day: 'Today · Thursday 22 October' },
          { who: 'you', at: -65, text: "Morning. I'm landing Export's draft within the hour." },
          { who: 'agent', at: -64, read: true, text: "Noted. I'll keep s1's three spare slots free until then." },
          { who: 'tools', at: -50, calls: [['recall', 2], ['read_pull', 3], ['read_items', 1], ['workers', 1, '6.7'], ['accounts', 2, '6.3']] },
          { who: 'agent', at: -50, text: [
            '**Check · two-hour timer**',
            '',
            '| | |',
            '|---|---|',
            '| Harness | T12 on s1 · 3 delegates running (T33 benchmark, T34 streaming, T35 build path) · 3 changes in kit’s queue |',
            '| Landed | kit: T24 replay fixtures at 06:48, slow tier passed · app: nothing since yesterday |',
            '| Findings | F, kit’s reconnect flake · B’s · fix f1 (T31) ready, 3rd in kit’s queue |',
            '| Workers | ws idle · s1 4 of 7 slots, disk 142 GB free (floor 80) |',
            '| Plans | Claude week 58%, above its pace of 48% · ChatGPT week 22%, under its 28% |',
            '| Spend | since 05:10: Harness 3.1 · Director 0.4 |',
            '| Needs you | nothing |',
            '',
            'Next check at 09:10.',
          ].join('\n') },
        ],
      },

      // Notes, each an entry: a one-line description, a body, a scope and
      // its writer (jig's domain/engine.md, section 10).
      notes: [
        { scope: 'project', by: 'T1', at: -50, name: 'priorities', text: 'T12 Harness: the fake server, then the flakes, then the build path.' },
        { scope: 'project', by: 'T1', at: -1300, name: 'standing rules', text: '- Claude for taste; implementers, surveys and investigations on Codex.\n- Budgets are Pat’s: ask before raising one.\n- Credentials are never copied between machines.\n- Owners check shared version numbers before each landing.' },
        { scope: 'project', by: 'T1', at: -64, name: 'next', text: 'Export: start its owner when its draft lands (Pat, 06:55).' },
        { scope: 'goal', goal: 'T12', by: 'T12', at: -72, name: 'fake server', text: 'Replay fixtures landed (T24); typed errors and streaming next.' },
        { scope: 'goal', goal: 'T12', by: 'T12', at: -128, name: 'F', text: 'reconnect_after_drop flakes 3 in 40. Cause: the reconnect timer races the drop. Fix f1 = T31.' },
        { scope: 'goal', goal: 'T12', by: 'T12', at: -26, name: 'build path', text: 'One cache per lane (T28); the slow path is being investigated (T35).' },
        { scope: 'goal', goal: 'T12', by: 'T12', at: -56, name: 'benchmark', text: 'T33 needs s1 quiet for its numbers.' },
      ],

      // What the engine records, in its own terms; a reason is the
      // writer's words.
      activity: {
        T12: [
          { at: -12, what: 'queued', task: 'T31', text: '3rd in kit’s queue, CI passed at its head' },
          { at: -25, what: 'started', task: 'T35', text: 'took the third slot of Codex slots' },
          { at: -72, what: 'landed', task: 'T24', text: 'in acme/kit at 4e1a9c; slow tier passed after it' },
          { at: -128, what: 'ended', task: 'T14', text: 'done; finding F opened from its result', gap: '6.5' },
        ],
      },
      history: {
        T12: [
          { at: -26, by: 'T12', what: 'delegated T35', reason: 'The slow build path.' },
          { at: -128, by: 'T12', what: 'delegated T31', reason: 'f1 for F.' },
          { at: -955, by: 'T1', what: 'made T12: tracked, priority 1, budget 60, host kind s1, a pool of 3 slots', reason: 'Harness is the first draft to own.' },
        ],
      },
    };
  }

  // Each step names the case's section, its time and what to look at.
  const steps = [
    {
      id: '0', label: 'Hour 0', hour: 0, act: 1, title: 'The start',
      summary: 'The director runs on ws. B runs on s1, with three Codex slots. F is an open finding, a kit test that flakes; it is B’s, and its fix f1 is a ready change.',
      look: [
        ['chat.html?task=T1', 'The director’s chat: its last check, its notes, its delegates’ cards'],
        ['board.html', 'Board: one goal, Harness (B)'],
        ['tasks.html', 'Tasks: the whole tree, from the director down'],
        ['task.html?task=T12', 'Harness: its pool, F and f1'],
        ['task.html?task=T34', 'An agent at work: its transcript, streaming'],
        ['changes.html', 'Changes: kit and app, f1 3rd in kit’s queue, F'],
        ['workers.html', 'Workers: ws and s1'],
        ['accounts.html', 'Accounts: the two plans'],
      ],
      apply() {},
    },
  ];

  // The case's later steps, listed so the step bar shows the whole story.
  const upcoming = [
    [1, '4.1', 'A starts and designs'],
    [1, '4.2', 'A known flake at kit’s tip'],
    [1, '4.3', 'The two-hour check finds s1 short'],
    [1, '4.4', 'The pace'],
    [1, '4.5', 'Sign-ins and limits'],
    [1, '4.6', 'ws hangs'],
    [2, '5.1', 'The person holds the director and rewrites the rules'],
    [2, '5.2', 'Every owner but A pauses'],
    [2, '5.3', 'A moves to the new shape'],
    [2, '5.4', 'The person changes direction'],
    [2, '5.5', 'A’s plan and its first milestone'],
    [2, '5.6', 'The person’s priorities, misread'],
    [3, '6.1', 'M1’s design and its tasks'],
    [3, '6.2', 'A blocker in a paused owner’s crate'],
    [3, '6.3', 'Landing on a tired machine'],
    [3, '6.4', 'Red main'],
    [3, '6.5', 'A budget question reaches the person'],
    [3, '6.6', 'The director’s word, late'],
    [3, '6.7', 'The cap'],
    [3, '6.8', 'M1’s end and its QA pass'],
    [4, '7.1', 'B wraps up, and C takes its work on'],
    [4, '7.2', 'Two owners, one repin'],
    [4, '7.3', 'The end'],
  ];

  const acts = {
    1: 'Act 1 · the old shape',
    2: 'Act 2 · the change of shape',
    3: 'Act 3 · the new shape',
    4: 'Act 4 · two owners at once',
  };

  MOCKUPS.worlds.reference = {
    name: 'Reference case',
    about: 'the retired first edition (docs/design/domain/testing.md, 5.3; docs/design/draft/reference-case.md is the second edition): a director, owners on two machines, pools, findings, an ops application, moved on step by step',
    HOUR0: Date.UTC(2026, 9, 22, 8, 0),
    initial, steps, upcoming, acts,
  };
})();
