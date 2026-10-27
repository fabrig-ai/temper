# The QA loop on real backends

**Parked** with the lab (`qa-lab.md`), 2026-10-26. Its ladder of rungs
(section 3) is taken up by `../qa.md`, section 3, for QA after landing.

Draft, 2026-10-23. The QA loop on ChatGPT and an owned Forgejo:
`qa-loop.md`'s M3 begun on one real LLM and one real forge, as the
backends roadmap makes them work end to end. It records a session with
the user on 2026-10-23. The lab's records, runner, outcomes, verdicts
and findings are skein's `lab.md`'s, amended with this draft for what
real backends need (its 2.3, 4.8, 6.1, 6.5, 8 and 9.3); the experiments'
cluster form, seed, `ctl` tool and checks follow `qa-lab.md`'s, cited
and not restated. This draft says what real backends add: a ladder of
stories (section 3), a run from scratch on the machine's binaries and a
moved account (section 4), the sign-in, the one act a person does
(section 5), bounds on two spenders and one shared account (section 6),
confinement (section 7), findings (section 8), running unattended
(section 9) and its milestones (section 10). What it waits for is
section 11.

## 1. In one page

- **Up a ladder, one rung at a time:** the ground (an owned Forgejo set
  up through `ctl`, the account moved in and out), then the three
  stories the fakes' roadmap delivers: A, a chat's change landed; B, A
  with a green light, a failed gate's repair, then an advisor and two
  delegates, in three rungs; C, the reference case, act by act. A rung
  opens once its story is green on fakes and the rung below passes on
  real (section 3).
- **Every run from scratch:** a fresh deployment on the run's
  directories, `temper box` with one owned Forgejo that the seed sets up
  with `ctl forge-setup`; Forgejo and its runner from binaries the
  machine keeps at absolute paths, pinned by version and digest, never
  fetched by a run (section 4).
- **One signed-in ChatGPT account, moved, never copied:** a person signs
  in once (section 5); the lab holds the record in a private file; each
  run takes it, its seed imports it into the fresh deployment, and its
  release exports it back before the cluster stops (4.3). No sign-in is
  automated.
- **Everything through `temper ctl`:** the seed, the driver and the
  checks reach temper through its tools; the driver's wrapper takes the
  areas its rung's story needs, never accounts or forges (4.4).
- **Two spenders, one account:** the driver's Codex and temper's own
  agents both spend the ChatGPT account the user's development shares.
  The driver is bounded as the lab bounds it; temper by its own budgets
  inside and the lab's meter outside; and a brake keeps runs off the
  account near its limits (section 6).
- **Caps learned, not guessed:** a rung's first runs are generous, its
  class then twice its largest pass, a pass far above the median a cost
  finding; a class's values are tuned without making new keys (6.4).
- **Confined by the lab, on the fleet's servers:** one user; the
  agent's commands in Codex's sandbox, temper reached through `ctl`
  alone, every action audited from the host's events. The host's own
  tools, which Codex cannot list, are the accepted risk, so the lab
  never runs on a person's computer (section 7).
- **Real runs find, fakes hold:** a failure's transcripts, the driver's
  and temper's model's, become a fake LLM's script; the fix lands with a
  test in the lowest world that reproduces it, and the real key is
  judged again. The words a model reads are a class of their own, which
  only a real run rechecks (section 8).
- **Unattended:** the director climbs the ladder at main's subjects,
  under the brake; the person is asked only for a sign-in (section 9).

## 2. What it is for, and what not

- **For what fakes cannot say:** whether a real model, reading temper's
  charter, tools and refusals, does what the stories need; whether
  Forgejo and its Actions runner behave as temper's model of them, and
  the fake forge, say; what a story costs on a real account.
- **For the stories the fakes' roadmap delivers,** in its order, once
  each is green on fakes: real runs follow, never lead.
- **Not the box's or the first run's QA** (`qa-loop.md`, section 2): the
  box is the cluster here, and its forge's set-up a seed action.
- **Not GitHub, nor a second LLM,** yet (section 12).
- **Not a gate:** runs are paid and on demand; a verdict informs the
  director, and the gate stays the worlds'.
- **Not the corner cases:** the domain's are the worlds'; real runs walk
  the stories' paths, and their failures go down (section 8).

## 3. The ladder

### 3.1 The rungs

| Rung | Story | Driver | What passes |
|---|---|---|---|
| 0a, the forge | the seed's `ctl forge-setup 1`: Forgejo and its runner prepared and started, temper's take-over, `work/hello` made with the starter workflow and adopted as the home of a new project, `hello` (`../protocol/setup.md`, 6.5 and 6.7) | none | the forge connected; the starter workflow's first run green on Forgejo's Actions; the repository adopted, read from `ctl forges`' first repository, made in `hello`, and from `forge-setup`'s printed adoption, since no ctl read lists a project's repositories |
| 0b, the account | 0a, then the held account imported, one chat turn in `hello`, and the release | none | the account usable once imported; the chat's reply; the record given back to the lab (4.3) |
| A | a person asks a chat for a one-file change on `work/hello`; it is delegated, pushed, passes CI on Actions and an agent's review, and lands: `qa-lab.md`'s story on real backends | yes | landed on `main`, the file as asked, nothing else changed |
| B0 | A, with a question the delegate asks and the person's green light (`reference-case.md`, 4.2 and 7.2) | yes | A's end; nothing pushed before the answer |
| B1 | B0, with a gate the first push fails and the change's repair (`reference-case.md`, 4.3) | yes | B0's end after a red run and a green one on Actions, the repair the delegate's |
| B2 | B1, with an advisor's report and two delegates, one depending on the other (`reference-case.md`, 4.1 and 4.2) | yes | both landed in order; the report delivered before the dependent's start |
| C1 to C6 | the reference case's acts 1 to 6 (`reference-case.md`, sections 4 to 9), each from the state the act before it leaves | yes | the act's end, as the case states it |

- **Each rung is an experiment** of temper's `lab/`, `live-<rung>`, in
  `qa-lab.md`'s form: its cluster (4.1), its seed, its charter for the
  driver, its checks of the story's end read through `ctl` and the
  forge's API.
- **0a and 0b have no driver:** cheap smokes of the ground, which the
  director runs first on a machine and after any change to the box, the
  set-up or the account's move.

### 3.2 Climbing

- **A rung opens** when its story's fake experiment has passed at a
  subject of main, and the rung below has passed on real at a subject no
  older.
- **A rung is judged** by `lab judge`, three seeds (skein's `lab.md`,
  section 8). A seed fixes the lab's draws, not a real model's
  sampling: three runs are three samples, and the campaign reports each
  rung's passes over every subject it ran, its rate.
- **A rung passes** at a verdict of pass. Flaky is no pass: its failures
  are triaged and filed with their reproduction rates (`lab.md`, 9.3).
- **Down before up:** a rung that fails or is flaky stops the climb
  above it until its findings are fixed, or parked by the director as
  the backend's. A rung below is judged again when a change touches it.

## 4. A run from scratch

### 4.1 The cluster

```
run R, seed S, scratch directory D
├── temper   temper box on D's three directories (../protocol/README.md, 4), configured by the experiment:
│            one owned Forgejo, asked, its binaries by absolute path; ChatGPT's models; no demo
└── seed     sign in by the printed link; ctl forge-setup 1; ctl import the held account; wait for it usable
```

- **The box is the cluster,** since `forge-setup` is offered under it
  alone (`../client/tools.md`, area 6): one process to the runner,
  Forgejo, its runner and `temper serve` in its tree, stopped whole
  (`lab.md`, 5.2).
- **The configuration is the experiment's,** a template rendered into
  the run's configuration directory before the box starts, so that
  nothing is defaulted by `temper init` or fetched. Its owned forge
  names `forgejo` and `runner.binary` by the lab's paths (4.2), with
  `"start": "asked"`; its models are ChatGPT's, priced as 6.2 says; its
  project's spend per period is the rung's (6.2).
- **The seed** signs in as `qa-lab.md`'s does (its 5.1), by the link
  serve prints; runs `ctl forge-setup 1`, which waits for the forge and
  adopts `work/hello`; does the person's own act at first sign-in with
  Forgejo's CLI on the run's work path (`ada`'s password change waived,
  a `read:repository` token for the checks, private, never printed, and
  removed once read); imports the held account and waits for it usable
  (4.3). For C, it also brings the act's prepared state (section 13).
- **One kept port:** the box's clients and webhooks listen on
  `127.0.0.1:0`, read from serve's `listening` lines
  (`TEMPER_ECHO=steps`); only Forgejo takes the port the machine keeps
  for the lab (section 13).
- **Nothing outlives a run** but the held account and the machine's
  binaries: no store, no Forgejo, no cache. A run that passes leaves
  nothing (`lab.md`, 5.1); one that does not keeps its scratch for its
  findings.

### 4.2 Forgejo's binaries

- **The machine's, by absolute path.** Each machine that runs the lab
  keeps Forgejo and its runner at paths of its own, pinned by version
  (the fleet's `TEMPER_FORGEJO` and `TEMPER_FORGEJO_RUNNER`, Forgejo
  17.0.0 and runner 13.2.0 at this writing). The lab's settings name
  them as programs, `forgejo` and `forgejo-runner`, each with its digest
  and version (`lab.md`, 2.3), and the experiment's configuration names
  them by `{program.forgejo}` and `{program.forgejo-runner}`.
- **In the key, checked every run:** a binary changed is a new key, and
  one that differs from the settings' digest refuses the run; temper
  checks the version at each command, as it does for any owned forge
  (`../protocol/setup.md`, 6.1).
- **Never fetched by a run:** `temper init`'s `"fetched"` is a person's
  default (`../protocol/setup.md`, 6.6). A run names paths, so its
  set-up takes seconds and needs no network beyond loopback.
- **An upgrade** is the machine owner's: new binaries at new paths, the
  settings changed, the next runs new keys.

### 4.3 The account, moved in and out

The record of one signed-in ChatGPT account is a held file of the lab,
`chatgpt` (`lab.md`, 4.8): private, in one place at a time.

1. **Taken:** at its provision, the run moves the file into its own
   directory, `{held.chatgpt}`; no other run takes it until it is given
   back.
2. **Imported:** the seed's `ctl import {held.chatgpt}` sends the record
   to the fresh deployment, which takes it into its secrets' store as
   the account of its subject; `ctl` then removes the file
   (`../client/tools.md`, area 5; jig's `domain/accounts.md`, section
   6). `ctl wait accounts --until usable` follows.
3. **Used:** temper's agents use it through the run, refreshing it as
   they need; each refresh rotates its refresh token, which only the
   deployment holds.
4. **Exported:** once the drive and the live checks are done, the
   release's `ctl export ACCOUNT {held.chatgpt}`, ACCOUNT the number
   `ctl import` printed, which the seed keeps in a run directory,
   withdraws the account,
   writes its record, and the deployment erases it; the runner gives
   the file back to the lab. The release runs however the drive ended,
   a bound or a cut included, while the box is up (`lab.md`, 4.8).
5. **Kept by a crash:** when the box is down before the release, or the
   release fails, the record stays in the run's kept store, and the
   held file stays taken, naming the run. The director recovers it:
   `temper serve` started again on the run's kept directories, `ctl
   export` to a private file, `lab held put chatgpt FILE`. Failing
   that, the person signs in again (section 5).

- **Never copied:** a refresh by one copy signs every other out (jig's
  `domain/providers.md`, 2.3). A move leaves one holder: the file, or
  one deployment.
- **The driver never holds it:** its wrapper refuses area 5 (4.4), and
  neither its sandbox nor its tools' views reach the lab's files
  (section 7).
- **One held account a lab,** since a lab runs one cluster at a time
  (`lab.md`, 3.2). A second machine's lab has a sign-in of its own.

### 4.4 The driver's tool

- **`qa-lab.md`'s `ctl` wrapper** (its 6.2): the seed's client
  directory, the engine's address pinned, `sign-in`, `sign-out` and
  `--call` refused. It takes the areas its rung's story needs: area 1
  for A and B; for C, areas 2 and 3's tools as its acts need them
  (goals, priorities, people's roles; `../client/tools.md`, section 5).
- **Never areas 5 and 6:** accounts and forges are the seed's and the
  release's.
- **The tool's view** writes the client directory alone; the agent's own
  commands read the source and write their work directory (`lab.md`,
  6.2).

## 5. The sign-in

The one act a person does: once for each lab, and again only when the
record is lost or has lapsed.

1. On a machine with a browser, `temper box` on throwaway directories
   with ChatGPT's provider; `temper ctl connect PROVIDER` prints the
   address, which the person opens and approves (`../client/tools.md`,
   area 5). The provider's redirect reaches the engine's loopback, so
   the browser is on the engine's machine (`../protocol/credentials.md`,
   section 3).
2. `temper ctl export ACCOUNT FILE` writes the record to a private file,
   and the throwaway deployment erases it.
3. The file is moved to the lab's machine, copied and then removed where
   it was, and `lab held put chatgpt FILE` takes it in.

- **A person's acts, never the lab's:** no sign-in is automated, and
  nothing in the lab opens a provider's page.
- **The account the user develops with,** signed in apart: its own
  sign-in, so its own refresh tokens; only the account's limits are
  shared (6.3).
- **Asked for by the director** when the seed's `wait accounts --until
  usable` fails on an imported record, or a recovery fails (4.3); the
  request names the lab's machine.
- **Lapsing:** the record's refresh token lasts 30 days (jig's
  `domain/providers.md`, 2.3), and any run within them renews it. A lab
  quiet for longer needs a sign-in.

## 6. Bounds

### 6.1 The driver's

As `lab.md`'s 6.5: `driver_seconds` and `driver_actions` enforced as
they pass, `driver_input_tokens` observed per turn with its overshoot
recorded. Each rung names classes of its own (6.4).

### 6.2 The subject's

temper's agents spend the same account through their own runs, which
the driver's bounds do not see. Two bounds hold them:

- **Inside, temper's budgets:** the project's spend per period, the
  rung's, given at the deployment's first start
  (`../protocol/configuration.md`, section 8), and its charter's
  per-run caps (`../domain/agent.md`, 4.6). The configuration prices its
  models so that the deployment's unit is a thousand tokens, input
  (cached included) and output alike: spend then counts what the
  account's limits follow. A story that reaches the budget waits, as
  temper's runs wait on any budget, until the drive's time ends it: a
  failure, triaged as cost or as a cap too small (6.4).
- **Outside, the lab's meter** (`lab.md`, 6.5): a command of the
  experiment's that prints temper's spend so far. Until `ctl` reads a
  project's period, it sums `spent` over the tasks `ctl tasks` lists at
  the top of their trees, a delegate's spend being counted up the chain
  that funded it (jig's `domain/authority.md`, section 1). Past the
  class's `subject_spend`, the runner ends the drive and the run fails
  `bound/subject_spend`; a meter that fails twice running fails the run,
  so that no subject spends unmetered.

### 6.3 The shared account

The driver's Codex, temper's agents and the user's development share one
ChatGPT account's five-hour and weekly windows.

- **The brake** (`lab.md`, 6.5): before each run, the lab asks the
  driver's host for the account's windows, with no model turn, and
  refuses the run past the settings' thresholds; `lab judge` stops
  there, and the director judges again later. Start at half the
  five-hour window and three-quarters of the week, so that development
  keeps its share; the director adjusts.
- **A pause:** the brake also refuses while the lab's `pause` file
  exists, the user's switch or the director's.
- **Each run records the windows** its host last reported, so that the
  campaign shows what a rung costs in the account's own terms, its
  scarcest measure.

### 6.4 Calibration

- **A rung starts generous:** a class `temper-live-<rung>-calibration`
  whose values only a runaway reaches, the director's estimate four
  times over, for the driver's time, actions and tokens and temper's
  spend.
- **Then the rung's class,** `temper-live-<rung>`: each value twice the
  largest of the calibration key's passing runs, rounded up, reported to
  the director before any run under it, as `qa-lab.md`'s 10.1 does.
- **A pass far above the median,** past twice the rung's median use of
  any value, files a cost finding: a loop, a retry, a charter that
  wanders.
- **A bound failure is triaged:** a regression is a finding; a class too
  tight is raised, with the run that showed it in the director's notes.
- **Tuned without new keys:** a class's values are in each run's record,
  not in the key, and a run failed only by a bound since raised is no
  longer judged, its seed attempted again (`lab.md`, 2.3 and section 8).
  A class renamed is a new key.
- **Time:** `driver_seconds` bounds the drive, and a run's longest
  follows from the class (`lab.md`, 6.5). Real Actions take minutes a
  run: B1's two runs and C's acts get classes of their own.

### 6.5 A run's end

In this order, however the drive ended:

1. the driver ended, by its report, a bound or a cut;
2. the meter's last reading;
3. the live collections and checks: temper's documents through `ctl`,
   the forge's state through its API;
4. the release: the account exported and given back (4.3);
5. the box stopped, Forgejo and its runner with it;
6. the stopped collections, temper's `store/` and `transcripts/` and
   Forgejo's work path copied at rest, then the stopped checks.

temper spends between 1 and 5 only for the seconds they take: no task is
cancelled first, since the stop ends every run.

## 7. Confinement

- **Why it matters more here:** on real backends, a driver that leaves
  its tools reaches more: a real account's record, a forge's admin
  token, the machine's other credentials. And a driver that works around
  `ctl` makes a verdict say nothing of the tools.
- **One user, on the fleet's servers.** The runner, the box and the
  driver's host run as the lab's user, on servers kept for the fleet,
  never a person's computer (`lab.md`, 6.1). The run's cgroups bound
  what they use and end them whole (`lab.md`, 5.2); they confine no
  file.
- **What confines:** the agent's commands run in Codex's sandbox,
  reading the source and writing their work directory, with no network
  (`lab.md`, 6.2); each `ctl` call runs in a view of its own that writes
  the client directory alone, and the wrapper refuses the sign-in,
  accounts and forges (4.4). Neither reaches the lab, the held account,
  temper's store or Forgejo's files.
- **What does not:** the host's own process. A built-in tool of Codex's
  that acts in it, rather than as a sandboxed command, reaches what the
  lab's user reaches, the held account and temper's store among them.
  Codex 0.162.1 cannot report which such tools its model is given
  (o/lab-driver's closing note), so they are recorded as unobserved
  rather than shown absent (`lab.md`, 6.1): the accepted risk, and why
  the lab stays off a person's computer.
- **Audited from its events:** an action of any kind but a sandboxed
  command or a `lab` tool call ends the run in error, its transcript
  kept, and the director reads it as a boundary incident. So a run in
  which the driver went around `ctl` is never judged.
- **M1 unblocked too:** the same admission holds on fakes, so
  `qa-lab.md`'s paid driver runs, refused until now for want of that
  list, can start.

## 8. Findings

- **Classes** (`lab.md`, 9.3), with one added for real models:
  `words`, the text temper's model reads, its charter, its tools'
  declarations, its refusals' and errors' messages. A `words` fix lands
  with a world's test where the text's content is asserted, and only a
  real run rechecks it: the rung's key at a subject that carries it.
- **Down to fakes first:** a failure's transcripts, the driver's calls
  (`lab.md`, 6.3) and temper's model's exchanges (its `transcripts/`,
  collected at rest), become a script of the fake LLM in skein's
  vocabulary (`qa-lab.md`, 7.1). Run on fakes, it shows whether temper
  fails without the real model; run with the owned Forgejo and the fake
  LLM, whether Forgejo is the cause. Where it reproduces names the class
  and the lowest world that takes its test.
- **A backend that surprises** temper's model of it, or the fake's, is
  two findings: `fake`, the fake taught the real behaviour, with the
  recorded exchange as its evidence; and temper's handling, a `world` or
  `below`, its test on the corrected fake.
- **Cost findings** (6.4) are classed by their cause, most often
  `words`: a charter that sends the model round.
- **Routed through the director** to each owner, as `qa-lab.md`'s 10.3
  says; the lab commits no test, and its records stay under
  `~/src/rust/tmp/lab/`.

## 9. Unattended

- **The person:** the sign-in (section 5), once a lab and when it lapses
  or is lost. Nothing else waits on them.
- **The director:** runs 0a and 0b on a new machine and after changes
  to the box, the set-up or the account's move; judges each open rung at
  a subject of main when a landed change touches its story; reruns a
  failing seed once for a finding's filing; recovers a held account kept
  by a crash (4.3); reports each campaign's verdicts, findings and usage.
- **Under the brake:** a braked judge resumes later, from the evidence it
  has; it is never forced.
- **Paid runs counted:** each campaign plans its paid runs and holds some
  for attempts, as `qa-lab.md`'s 10.2 does; past its plan, the director
  is asked first.

## 10. Milestones

- **L1, the ground and A:** the lab's amendments built (`lab.md`, 2.3,
  4.8, 6.1, 6.5 and 8); `ctl export` and `import`; 0a and 0b passing on
  hel8; A calibrated, its class set, and A passing.
- **L2, B:** B0, B1 and B2 in turn, each once its fake story is green.
- **L3, C:** the reference case act by act, once a prepared state is
  designed (section 13).

## 11. What it waits for

- **The backends roadmap's B5:** the change-shaped delegation (M2) and
  the live sitting with the shipped charter (M3). A on real is B5's QA.
- **The account's move:** jig's (`domain/accounts.md`, section 6, its
  Later), and `ctl export` and `import`, now in area 5
  (`../client/tools.md`).
- **The lab's amendments:** held files and the release (4.8), named
  programs (2.3), the meter and the brake (6.5), the host's unobserved
  items recorded (6.1), a class's values out of the key and superseded
  bounds (2.3, section 8), the `words` class (9.3).
- **The machine:** the settings naming Forgejo's binaries; a port for
  the owned Forgejo (section 13).
- **For B1,** failed CI's repair on real Actions, which the backends
  roadmap leaves out of B5; **for C,** a prepared state.

## 12. Later

- **Claude as a second LLM:** a key the seed adds, or a program's login
  on the lab's machine, since only a signed-in record moves (jig's
  `domain/accounts.md`, section 6).
- **GitHub as a second forge,** on the probe App and repositories alone.
- **Rates over many runs,** when three samples a key stop saying enough
  (`qa-loop.md`, section 12).
- **Parallel labs:** one cluster per lab, so one lab per slot, each with
  its own held account and sign-in.

## 13. Open questions

- **The owned Forgejo's port:** its `HTTP_PORT` is configured, not
  reported (`../protocol/setup.md`, 6.2), where the lab expects a
  process to bind port 0 and print its address (`lab.md`, 4.4). Until
  it can, the experiment names a port the machine keeps for the lab.
- **C's prepared state:** an act's start is the act before it's end. A
  passing run's stopped copies of temper's store and Forgejo's work
  path hold no account once the release has run; how a run starts from
  them, writable, with the account moved in, is open.
- **B1's red run on real Actions:** fakes fail CI by a content cue
  (`qa-lab.md`, 7.2); a real workflow fails by content too, but a real
  model may meet the rule at its first push, and then the run walked
  B0's story, not B1's. Whether such a run is recorded as not having
  exercised B1, or the experiment forces the first run red, is open.
- **Keeping a quiet lab's record alive:** a run with no driver that only
  moves the account in, refreshes it and moves it out, or a sign-in
  after each quiet month.
- **A ctl read of a project's period,** its spend and its budget, in
  place of the meter's sum.
