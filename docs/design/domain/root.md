# temper's root

Provisional, 2026-10-04, revised 2026-10-23. What temper's engine root
is and does: `temper-engine-domain`, the top-level domain of temper's
engine, which composes jig's core with the forge connector in the shape
jig's `domain/root.md` gives every application's root. It says what the
root composes and carries, its vocabulary toward the protocol layer, the
one shape as temper holds it, how it routes, translates and puts outputs
together, its store vocabulary, its part in restart and stopping, its
limits, and what it never does. The core is jig's `domain/engine.md`;
the forge is forge.md; the worker the root assigns runs to is worker.md,
and the agent agent.md. What is still open is listed in section 13.

## 1. In one page

- **temper's root composes jig's core and the forge.** Its children are
  jig's core, `jig-domain-core`, and the forge connector's four crates.
  It alone faces the engine's protocol layer (section 3;
  `../protocol/README.md`, section 6).
- **It routes and translates; jig's core decides.** What commits
  together, what waits for a commit, the order of a restart and the check
  on every effect are the core's or the journal's (jig's
  `domain/root.md`, section 1). The root carries them through skein-lib's
  journal, its only way out (section 4).
- **It speaks for the forge to the core.** The forge's crates name no
  type of jig's core: the root makes, from what the forge says, what the
  core takes (an effect's description, a verdict, a procedure's step and
  its delegates, a hold's reason, a result), and back (sections 5 and 6).
- **Every run is on a worker.** temper runs no agent in the engine: the
  root has no hub child, and speaks jig's hub's link vocabulary to
  workers, which in standalone share the engine's process (section 2).
- **The owner keeps the value.** The root puts each output together from
  its owners' parts: an assignment from the core's part and the forge's
  workspace and brief sections, a call's answer from the core's part and
  the forge's (section 7).
- **One store vocabulary.** Its keys, records and ranges wrap the core's
  and the forge's; it has no records of its own (section 8).
- **Bounded, and checked at start.** Every event reserves the room of the
  whole route it may take; the root's own limits sum with its children's
  into its worst case, and limits that do not add up refuse the start
  (section 10).
- **On jig's rails.** The engine's world runs the root with its real
  children over jig's store; the conformance world runs it on jig's
  harness, under jig's referee (section 12).

## 2. What the root is

```
temper-engine-domain                       the root: the journal, routing, translation, putting outputs together
├── jig-domain-core                        jig's core and its children (jig's domain/engine.md, section 3)
└── temper-engine-domain-forge             the forge connector's top (forge.md, section 3)
    ├── skein-forge-client                 the working set, keeping up, fresh reads, the request budget, effects' keys, write lanes (skein's forge-client.md)
    ├── temper-engine-domain-forge-change  the change procedure and the landing queues
    └── temper-engine-domain-forge-issues  the goals' issues
```

- **One connector, the forge.** Configuration names the core one
  connector, the forge, and a start naming another is refused
  (`../protocol/configuration.md`, section 2). The root knows the forge
  by that number and matches on its variants exhaustively.
- **No hub, no agent in the engine.** The root links `jig-domain-host`
  for the hub's link vocabulary alone: a worker's hello and its records
  up and down (section 3). The engine has no slots of its own
  (`fleet.engine_slots` 0) and temper's charters run on workers
  (`hosts: "workers"`), so the fleet places every run on a worker (jig's
  `domain/hosts.md`, section 3).
- **Workers in the engine's process, in standalone.** The worker joins
  the engine by an in-process address (`../protocol/README.md`, section
  8), so the core is configured with its workers in process
  (`workers_in_process`, which jig's configuration sets from an
  in-process hosts' listener), and a restart settles their claims at once
  (jig's `domain/engine.md`, section 6). temper's domain worlds run their
  workers apart.
- **Its state:** its children's; the journal; what it keeps of its
  configuration (the forge's number and the forge's charter); its facts
  waiting to be drained; and the values in flight between a hand-off and
  its answer, each bounded (section 10): a worker's record until the
  fleet takes it, a run's brief sections and workspace while its claim is
  prepared, its assignment until the claim's decision, the loads it makes
  for the core, host calls being decoded and rendered, and the forge's
  reads, effects, subscriptions and adoptions in flight. None of it is
  stored.
- **Siblings share no types.** The core never names the forge's types,
  nor the forge the core's; the forge's top names jig's people child's
  outcomes alone, for its keyed adoptions (forge.md, section 4).

The crate, one module per part:

```
temper-engine-domain/src/
├── lib.rs              the tree, re-exports
├── boundary.rs         the root's vocabulary: events in, requests out (section 3)
├── admission.rs        what the journal takes before any child changes (section 4)
├── domain.rs           Domain, step, fire, release: the one shape (section 4)
├── decision.rs         the decision and the journal's limits (section 4)
├── route.rs, route/    which child each event and hand-off goes to (section 5)
├── translate.rs        Config and the small total functions (section 6)
├── assemble.rs         assignments and briefs from their owners' parts (section 7)
├── link.rs             the hub's link records, and what a run left (sections 3 and 7)
├── store.rs, loads.rs  keys, records, ranges and pages (section 8)
├── archive.rs          loads outside the working set (section 8)
├── restart.rs          the restart script's steps (section 9)
├── stopping.rs         the drain (section 9)
├── deadline.rs         the children's timers
├── limits.rs           the root's limits and its worst case (section 10)
└── tests.rs
```

## 3. Its vocabulary

What crosses the domain boundary, in `boundary.rs`: an exhaustive enum
per direction, one variant per peer. The engine's service on the other
side, its components and what its routing does with each, is
`../protocol/README.md`, section 6; in standalone it shares its process
with the worker's (`../protocol/README.md`, section 8).

- **In, `Event`:**
  - the store's answers (`StoreInput`): a commit durable, with the
    store's fill; a commit failed; a page loaded, or not; a core read's
    page;
  - a worker's channel (`WorkerInput::Link`): a hello, a record up with
    temper's `Parts` beside an answer, a loss; and the protocol layer's
    hand-backs: a host call decoded into the root's `Tool`, a decided
    answer rendered, an ended run's outcome read;
  - a party's (`PartyInput`), among them signing in and out, a keyed
    request, an application request, reads, watches and their
    deliveries, a project's period;
  - accounts' events, and a generation of a git identity
    (`Event::Credential`, section 7.1);
  - the forge's events: its API's answers and its webhooks' hints;
  - a released output addressed to a child (`Event::Released`), and the
    service's `Restart`, `Resume` and `Stop`.
- **Out, `Request`:** to the store, a commit, a page to load, a core
  read's load; to a worker, a link record, an assignment, a refusal, a
  turn answered busy, a call to decode or dropped, an answer to render,
  an ended outcome to read; to a party, replies, read answers, documents,
  a watch refused or ended, a run's text; the forge's API calls
  (`Request::Forge`); accounts' requests; a released output back to its
  child (`Request::ToChild`); a forge event the drain hands back
  (`Request::Busy`); `Drained`; `Stop`.
- **temper's parts on the hub's records** (`../protocol/channel.md`,
  sections 4 and 5): the assignment's `ForgeWorkspace` down; up, beside
  an answer, `Parts`: the heads the run pushed, the repositories it
  saved, a permanent failure to prepare one.
- **Facts,** which decide nothing: the root's one, a failed preparation
  (`Fact::PreparationFailed`, section 7.4), at most 32 waiting to be
  drained, the rest counted as lost; and the children's own.
- **Tools.** The root's `Tool` is a host call in typed form, as the
  protocol layer decodes it (`../protocol/llm.md`, section 4): jig's
  operations on tasks, notes, the forge's reads, effects and topics, and
  the refusal of a call whose shape is past its bounds. Which tools
  temper's agents get, and what each answers, is agent.md, 4.3.

## 4. The one shape, and the journal

jig's shape (jig's `domain/root.md`, section 3), with temper's
admission:

```
try_step(root, event):
    a store's answer, a released output, Resume:   always stepped
    a core read's page:   kept, in order, until the journal takes its route
    a worker's record:    handed back unless the journal takes the route and the root can keep its bytes
    anything else:        handed back unless the journal takes the route
step(root, event):
    route the event to its child; drain every hand-off it starts, through a bounded work queue
    the core finishes its decision: its writes and held outputs into one decision
    journal.accept: at most one commit, the deployment's record its first write
    outputs marked now: through the door
release(root):   (the service's iterate)
    after a step: the door's outputs and the commit
    otherwise:    one held output whose commit is durable, then the next commit
```

- **The room is the whole route's.** A step drains every hand-off its
  decision starts, so any event may reach the whole route; each reserves
  the same room, the sum of what every child may write and hold along it,
  with the call records (section 10). An event the journal cannot take
  changes nothing.
- **Handed back, not answered.** An input the journal cannot take returns
  to its owner unchanged, who offers it again; the root answers nobody
  busy itself. Busy answers are the core's (jig's `domain/engine.md`,
  5.1).
- **The deployment's record rides every commit:** the core's numbers,
  with the commit's own, so a restart reads the last durable commit from
  it.
- **Released in order:** one held output per call, and the door's outputs
  first, after the step that staged them. A released output addressed to
  a child (a fleet's start, a relayed message, a forge entry to make, a
  load) comes back as `Event::Released`, one more step of this shape.
- **Owner progress is a step of its own.** `Resume` admits one: the
  forge's continuation when it has one, else a due task's activation,
  else the fleet's.
- **Timers** fire through the same admission (`fire`): accounts' and
  views' always; the forge's from its restart step on; the core's others
  once decisions open.
- **A decision past its reserve** fails stop, as jig's journal says
  (jig's `domain/root.md`, section 4).

## 5. Routing

### 5.1 From the protocol layer

Each event goes to the child it is for, by its variant: the store's
answers to the journal and to the load that asked (section 8); a
worker's hello and records, a party's requests, reads and watches, a
project's period (`../protocol/configuration.md`, section 8), accounts'
events and a git identity's generation to the core; the forge's events
to the forge; a timer to the child that armed it. What temper adds:

- **A worker's bytes stay with the root.** A turn's transcript and an
  answer's outcome and parts are kept under tokens the fleet names, until
  the core takes or drops them; the fleet sees their metadata alone.
- **An ended run's outcome** goes out to the protocol layer to read, with
  the change it may name; its typed end comes back to the core, temper's
  parts translated (7.4).
- **An adoption,** an application request, goes to the core to admit by
  key, then to the forge (5.4).
- **A forge's set up,** `SetUp { forge }` (`../client/README.md`,
  section 8), goes to the forge, which decides it by the forge's offer
  (forge.md, 4.1), refusing it there or asking the protocol layer's box
  link, which carries the ask to the box and its lines back
  (`../protocol/forge.md`, 2.8). The root's configuration carries each
  configured forge's number, kind, mode and setup start, and whether
  serve runs under the box, which the root gives the forge at
  construction; the root's event `BoxClosed`, from the link at the
  pipe's close, goes to the forge. It is keyed: the core admits it by key,
  of the deployment's scope, first, and its answer comes back from the
  link to the forge, which keeps it as the request's outcome, in memory
  alone. Nothing of it is committed: the states and offers it moves are
  in memory, as every connection's is.
- **A watch of the deployment's forges,** the subject `forges`
  (`../client/forge.md`, 5.4), goes to the core to admit on the party's
  deployment role, and its documents come from the forge as a change's
  panel's do (below).
- **A hint that comes before decisions open** is dropped, since the
  restart reads afresh (9.1).
- **A watch of the forge's subject,** a change's panel
  (`../client/forge.md`, 9.2), goes to the core to admit on the
  party's role in its project; the core's ask for its document goes to
  the forge (5.3), which makes it or answers unknown; the forge's marks
  of its subjects go to the core; and each document leaves held in the
  decision that asked for it, out to the protocol layer to encode once
  that decision's commit is durable.

### 5.2 Host calls

- **Fenced, then decoded by hand-out.** The fleet admits a call only from
  its task's live attempt; the root hands it out (`HostRequest::Decode`),
  and the protocol layer decodes its input into the root's `Tool`
  (`../protocol/llm.md`, section 4). No domain parses a call's input
  (jig's `domain/engine.md`, 4.4).
- **Checked for shape** against the core's bounds before any child sees
  it: a call past them becomes its refusal, answered and recorded as any
  decided call is.
- **An undecodable call decides nothing.** The protocol layer answers it
  as an error naming what is wrong; the root relays that answer to the
  run with no commit and no record, and the run may ask again with a
  corrected input under a new name.
- **Invalid or busy, answered at once:** a call naming no live attempt or
  no completion is answered invalid; one that finds the call records, or
  the loads it needs, full is answered busy, and its run asks again after
  a backoff.
- **Routed by tool** (agent.md, 4.3 lists them). A call the core has a
  record of is answered from it (7.3). Otherwise the core checks it
  against its task's authority; then jig's operations, notes and
  subscriptions to tasks and timers go to the core as named actions, and
  the forge's reads, effects and topics to the forge (5.5); a proposal of
  a forge effect goes to the forge, which keeps it, then to the core.
- **Rendered by hand-out.** A decided call's answer goes out to be
  rendered (`HostDelivery::Render`) and comes back as smith takes it; the
  core keeps the rendered answer with the call's record, so a call asked
  again is answered without rendering again. A read's answer leaves
  through the door.

### 5.3 The core's asks of the forge

The closed list (jig's `domain/connectors.md`, section 2), each by the
forge's number:

| The core asks | The root |
|---|---|
| describe, keep or drop an effect; judge a requirement | gives it to the forge (5.5) |
| judge items of temper's kinds of settings; tell their new values | gives them to the forge (5.6) |
| the holdings of a task, of a delegate batch, of a procedure's batch | has the forge name the branches each task will hold, before the core admits the batch |
| take or hand down a hold; end a topic | gives it to the forge |
| a claim | claims the forge's writer slots for the attempt's branches in the claim's decision, and starts the run through the fleet, with its grants, once the claim commits (7.1) |
| gather, cut or drop a brief section | gives it to the forge (7.2) |
| adopt a repository; an application admitted, answered or dropped | gives it to the forge (5.4) |
| an attempt ended | gives the forge what the attempt pushed, then has it read its branches afresh before their writer slot frees (7.4) |
| close, release | has the forge settle the task's effects, then release its holds (forge.md, section 11) |
| project a goal; forget a projection | gives it to the forge's issues (forge.md, section 12) |
| a subscription made or dropped | makes or drops the forge's subscription under the core's number, and answers the call |
| start a procedure | starts the forge's change procedure (5.4) |
| a step made or refused; a delegate or a repair refused | records the change's delegate, or holds the change |
| a document or a read of the forge's; a document dropped | gives it to the forge, which makes it from its records and working set, within the scope the party named (forge.md, section 2; `../client/forge.md`, section 9) |

### 5.4 The forge's routes

What temper adds to the core's usual routes (jig's `domain/engine.md`,
4.4):

| What arrives | Route | Then |
|---|---|---|
| a forge API answer, or a hint | the forge: its working set (forge.md, 5.2); each change it moves, stepped | commit; news to subscribers through the core; changes stepped |
| a change due | the core starts its procedure → the forge's change procedure, on fresh facts → its decision in the core's terms (section 6): a delegate, an effect, a wait, a hold, its result | commit; delegates made; effects made after the commit |
| a merge's outcome | the forge → the change's result; the landing as news on its branch's topic → each goal's subscription, waking it by overlap (forge.md, 7.3) | commit; the change closes, its branch released |
| a landing branch's CI failing | the change procedure asks a queue repair → a change task into that branch, first in its queue (forge.md, section 9) | commit; the repair runs |
| an adoption | the core's people child admits it by key → the forge locates the repository → jig's adoption request on the party's behalf (`adopt_repository_ask`), which the core judges as the party's own → the forge reads what temper may do there, and seeds collaborators' roles (section 6) | commit; the outcome kept under the request's key, answered once durable (forge.md, section 4) |
| a goal changing | the core → the goal's issue → its effects | commit; effects made |
| a task closing | the core → the forge settles the task's effects, then releases its branches | commit; releases made |
| a run's answer | the fleet → the core, temper's parts translated (7.4) | commit; acknowledged |

### 5.5 Effects and verdicts

- **From an agent's call:** the forge describes the effect (its kind, the
  resource's name, temper's access to it) and keeps it under a token; the
  core checks it against authority and asks for its verdicts; the root
  routes each to the forge's judges and back; the core's answer (allow,
  propose, wait, refuse) goes to the forge, which enqueues the effect
  under a key derived from the call, keeps it as a proposal, or drops it.
  All within one step, one decision (jig's `domain/root.md`, section 9).
- **From the change procedure:** its effect is described and checked the
  same way before the step's decision is complete; one the core refuses
  leaves the change's state as it was.
- **The forge is every requirement's judge:** temper's landing rules are
  the forge's criteria (section 6; forge.md, 8.7). Designed, not built:
  the forge judges two standing requirements over the landing rules it
  holds (`../protocol/configuration.md`, 13.3), and the root gives it,
  with each ask, the merge's resources by the forge's names, the path
  read back as section 6 makes it.
- **Made after the commit:** the core's held output to make an entry is
  released once its commit is durable, fed back to the forge, which makes
  its API call (`Request::Forge`) behind every commit before it.
- **Priced at nothing.** Only LLMs are counted: the root gives no forge
  effect a price, and a forge request is load, within the forge's
  request budget (forge.md, 5.4), never spend.

### 5.6 Settings

Designed, not built (`../protocol/configuration.md`, section 13).

- **temper's kinds are the forge's.** A change of settings naming items
  of temper's kinds reaches the root within the change's decision, as a
  requirement's verdict is asked (jig's `domain/engine.md`, 17.5); the
  root gives each item to the forge, which accepts or refuses it with
  its reason, and carries the answer back. Once the change commits, the
  root gives the forge the kinds' new values; then each live change the
  rules cover is stepped.
- **The root routes, never judges** (section 11): what a landing rule
  means and whether it is well formed is the forge's.
- **The forge's charter is read through the core** when a change is
  made, by kind, scope and number (jig's `domain/engine.md`, 17.5), and
  kept on the change (section 6).

## 6. Translation

Small total functions, one per pair of vocabularies, matching
exhaustively with no `_` arm (jig's `domain/root.md`, section 6).

- **Configuration,** `Config`, field by field as
  `../protocol/configuration.md`, section 3 says: `core` to the core
  whole; `forge` to the forge, its namespace the deployment's id;
  `identities`, each forge's git identity, to the forge; `landing`
  translated (below); `forge_charter` kept, the charter of every agent
  task the forge's procedures make. Designed, not built: `landing` and
  `forge_charter` leave `Config` for settings, and `core` carries
  temper's stock defaults and its kinds' declarations
  (`../protocol/configuration.md`, 13.4). The root holds its configuration to
  its limits as it is made, and a start that breaks one is refused,
  naming it (`../protocol/configuration.md`, section 2): one connector;
  the configured workers within the fleet's configured count and name bytes
  (jig's Membership);
  the deployment's provider other than 0, the forge's sign-in; each
  charter's run within an assignment's bytes; the forge's charter
  one of the core's; the resume limit within a row's bytes; each seeded
  owner's project naming its escalation role; the landing rules and the
  permission mappings within a row's bytes, and within the forge's
  judges' tables.
- **The landing policy,** `LandingPolicy`: the deployment's rules and
  each project's, each a `LandingRule` on a pattern of landing branches.
  Each criterion of a rule (CI, up to date, a gate, an approval) becomes
  two values under one index: a requirement on the forge's merge, for
  authority, judged by the forge and guarded or observed as forge.md, 8.7
  says; and the forge's own criterion, by which its judge reads the
  facts. A rule that does not enforce gives the forge its criteria and
  authority no requirement. As the root is made, the deployment's
  requirements join the configured rules and each project's its policy;
  a rule's gates become the forge's gate templates, which every change
  into a covered branch asks for (forge.md, 8.3). Designed, not built:
  nothing is compiled. The rules are values of temper's kinds, which the
  forge holds as it is told them (5.6); the stock rules carry two
  standing requirements on the forge's merge, and no rule or policy of
  jig's changes with a landing rule (`../protocol/configuration.md`,
  13.3).
- **Names.** The root makes two names from the forge's own (the forge,
  the repository's id, what in it): a path for authority (`forge`, the
  forge's host, the owner, the name, then `branch` and its segments, or
  `pull` or `issue` and a number; forge.md, section 2); and, for a hold,
  three segments, the forge's number, the repository's id and the
  branch, which it reads back for the forge's branches only. A saved
  repository's tag becomes the task's saved resource, its path the
  forge's number and the repository's id (forge.md, section 4), and
  back.
- **The change procedure's decisions** become the core's procedure
  steps:

  | The change decides | The core hears |
  |---|---|
  | finished, merged | its result: the change and its pull request, the merge's id its words (below) |
  | a delegate: producing, a repair (CI, a semantic conflict, a gate's remarks), a resolution, a gate | a batch of one delegate (below); the step waits |
  | ready; a wait | a wait; ready steps it again |
  | a queue repair | the repair started (5.4) and a wait, or, when it cannot start, held for its effects |
  | held: its branch moved or missing, its pull request retargeted or closed | held for drift |
  | held: stalled | held, stalled |
  | held: past its repairs, resolutions or updates | held by its procedure |
  | held: a delegate or an effect failed for good; an approval declined | held, an effect failed |

- **A landed change's result** is a change of the forge's connector and
  kind, its resource the pull request, and its words the merge commit's
  id as text: in lowercase hexadecimal, as a delegate's commits are
  (below), shortened to its first digits, as a short id is, when the
  task's contract bounds its words under the id's 40 or 64. A result's
  words are text that summarises it (jig's `domain/tasks.md`, section 3,
  and `client/domain/contract.md`, 5.3), never the commit's bytes; the
  whole id stays the change's and its pull request's (`forge.md`, 8.2).
  The short id keeps any bound the task's creator chose: a bound under
  the id's digits is not refused, and the result never passes it.
- **A change's delegates,** in the core's terms: an agent task under the
  forge's charter for producing, repairs, resolutions, and agents' and
  checks' gates (designed, not built: the charter the change was made
  with, kept on it, which its procedure's delegation grants;
  `../protocol/configuration.md`, 13.2); a person task for the project's maintainers for a
  person's gate. Its parameters name the repository, its base and the
  change's branch, and a resolution's base commit or a gate's number and
  head, each commit as text: its id in lowercase hexadecimal, 40 digits
  for a SHA-1 id and 64 for a SHA-256 one, as the forge's ids are
  written (skein's `forge.md`, 5.1), since a task's parameters
  are text (jig's `client/domain/contract.md`, 5.3); its contract is a
  change, or a gate's verdict; its authority is the change's, its budget
  what the change has left, with no delegation of its own.
- **A collaborator's permission** on the forge (read, write, admin)
  becomes the role its project's configured mapping names, as a
  repository is adopted (forge.md, section 4).
- **A repository as a notes scope** is jig's resource scope on the
  forge's connector, its pattern the repository's path; the root
  translates nothing of it (jig's `domain/engine.md`, section 10).
- **The hub's link vocabulary** to the fleet's and back: a hello with its
  bounds, the worker's stop bound among them (worker.md, section 2), a
  run's phases, activity and counts. A run's live piece goes to the
  core as the run's text instead (jig's `domain/engine.md`, section 11):
  one of a kind other than text, or past the offsets the core keeps, is
  dropped and counted. A message relayed to a run
  keeps its kind in its words: a result, a proposal decided, a person
  task stalled and an effect not made each lead with what it is and its
  source's number.

## 7. Putting outputs together

### 7.1 An assignment

The core's part (the run, its attempt and conversation, its ask for the
run's live content, its charter, its brief's core sections, its start's
messages, its transcript's current window, the calls committed since its
last turn; jig's `domain/engine.md`, 7.1) and the forge's (the
workspace, the brief's forge sections), put together while the task is
preparing, held until its claim commits:

- **The repositories:** for a change's delegate, the change's
  repository; otherwise the project's home repository, those its spec
  names and those it saved work in; and, for every run, the project's
  context repositories, read only. Each item is the forge's: where it
  starts, and the branch it pushes to, the change's branch for a
  change's delegate and the task's own `r` branch for any other run
  (forge.md, section 11; worker.md, 4.1), pushed only when the task
  holds that branch, or the change it works for does, and its authority
  grants the push; otherwise read only.
- **The workstream:** the change's task for its delegates, so a change's
  producing, repairs and resolutions find its checkout cached; the task's
  own otherwise.
- **Saving:** the task's saved-work branch (forge.md, section 11) for a
  run that may write, so the unfinished work it leaves is saved and the
  next run starts from it (jig's `domain/engine.md`, 7.4; worker.md,
  4.2).
- **Grants:** the LLM accounts its charter's models need, the core's;
  and each repository's forge's git identity, one of temper's
  application credentials, by number and generation (jig's
  `domain/engine.md`, 12.1). Their values come from the forge's records
  in jig's secrets' store, generation 1 at each start and the next at
  each change of value, each announced before the restart
  (`../protocol/credentials.md`, section 4). The forge's API token or App
  key, and a Forgejo forge's webhook secret, stay in the protocol layer,
  and are never granted.
- **The writes it claims:** the branches it pushes, the core's writer
  slots in the claim's commit and the forge's after it (5.3).

### 7.2 The forge's brief sections

- **For a change's delegate,** once the change's pull request is open,
  one section at the change's last head, required: CI's failures, with
  the failed step and its output's end, for a CI repair; reviews'
  remarks for a gate's repair; the pull request against its base for
  producing and a gate, and with what landed in the base and the files
  in conflict for a resolution; and both CI's failures and the pull
  request with what its clean update merged in for a semantic repair
  (forge.md, section 2). Each kind has its budget (`ci`, `reviews`,
  `pull`).
- **For a task holding no run branch in its project's adopted home**
  (forge.md, section 11), one section, required: why its runs read the
  home without pushing, bounded by its words.
- **Planned with the core's.** The brief child places, cuts and orders
  them with the core's sections (jig's `domain/engine.md`, section 9);
  the forge gathers each and keeps it, cut to its allotment, and the root
  takes it once the brief is complete. A section whose bytes the forge no
  longer has sends the brief back, as a missing one does.

### 7.3 A call's answer

The core's part (its status, an effect's entry, why it was refused) and
the forge's (what an effect made, a read's answer, the forge's refusal),
which the forge keeps under the call's key and the root writes beside
the call's record in the same commit. Asked again, the answer is put
together again from both. A run's read is answered through the door.

### 7.4 What a run left

- **Pushed heads,** from the answer's parts, kept until the core says the
  attempt has ended; then given to the forge against the branches the
  attempt was granted, which it reads afresh before their writer slot
  frees (forge.md, 8.2).
- **Saved work:** the repositories saved, as the task's saved resources,
  which its next run starts from.
- **A failed preparation** names its repository and why: what the forge
  lacks (the repository, the branch, the commit), or that it refused the
  identity (`../protocol/channel.md`, section 5). The root reports it as
  a fact of the attempt: its task, its attempt, the repository's tag, the
  reason. Whether the task tries again follows the answer's class, as for
  any failure (worker.md, 4.3).
- **Past their bounds,** an answer's parts fail its run as invalid,
  whatever the answer says.

## 8. Its store vocabulary

- **`Key`, `Record`, `Write`** wrap the core's and the forge's, one
  variant each, the core's first; a write saves a record under its own
  key or erases a key (jig's `domain/engine.md`, 5.6).
- **No records of its own.** The deployment's record is the core's; a
  call's record is the core's part and the forge's (7.3).
- **`Range`:** the core's ranges the root loads; the census of ended
  owners (jig's `protocol/store.md`, 7.2); and `Range::Forge`, every
  record of the forge's, all live (`../protocol/store.md`, section 2).
  The core's: at restart, the deployment's record, people, live tasks and
  goals' projections, claims and calls; on demand, a task's result, an
  ended read position, a task's current window, an attempt's turns, a
  decided escalation or proposal, a page of notes.
- **Pages,** bounded in rows and bytes, are checked whole (range, order,
  cursor, each row's bytes) before any row is kept; a page past its
  bytes comes back cut, saying how much.
- **Loads for the core.** A run's preparation (its transcript's window,
  its dependencies' and inputs' results), a delegate batch's inputs, and
  the core's historical reads (ended conversations, decided escalations
  and proposals) are loads the root makes for the core and answers back
  in its terms; a full table of them answers busy.

## 9. Restart and stopping

### 9.1 Restart

The core's script (jig's `domain/engine.md`, section 6); the root does
each step the core asks for:

1. **The core's records:** every configured account wanted; the
   deployment's record, which binds the forge's namespace to the
   deployment's id and gives the journal its last durable commit;
   people, the configured projects and owners seeded at a first start;
   live tasks and goals' projections; claims; calls. A page past its
   bytes, or a record past the limits, refuses the start, naming its
   step.
2. **The inbox:** the census of ended owners.
3. **The forge's records,** `Range::Forge`, a page at a time; then the
   forge reconciles them. Designed, not built: before them, the forge
   hears its kinds' current values (5.6).
4. **Runs adopted:** the claims to the fleet, those on in-process
   workers settled at once (section 2).
5. **The forge reads afresh,** then **settles its outbox** (forge.md,
   section 16).
6. **Decisions open:** every due task is activated.

Until decisions open, the forge's hints are dropped and its timers fire
from its restore on.

### 9.2 Stopping

The core drains as jig's `domain/engine.md`, 6.1 says. What the root
adds:

- **the forge's events** go back to their owner undecided
  (`Request::Busy`), and no change steps;
- **a relayed call** is answered busy;
- **a committed forge entry** is not made: its notification is retired,
  its outbox row stays durable, and the next start's settlement makes it;
- **`Drained`** once nothing waits for a commit.

## 10. Limits and worst cases

The root carries configured core.policy_bytes unchanged and derives one
effective core envelope for runtime, store opening and sizing. Startup
checks that envelope against the codec and checks that a policy at its
configured bound plus its owned row carrier fits the journal. Transcript
capacity never enlarges the configured policy budget. Restore refuses an
over-bound stored settings policy by setting number and source, and an
over-bound project policy by project number.

- **The core's children's limits** are `engine.limits`, jig's part:
  tasks, authority, people, the fleet, accounts, notes, documents,
  templates and call records (jig's `protocol/configuration.md`, section
  6).
- **The root's own,** `root.` in temper's limits
  (`../protocol/configuration.md`, section 7):
  - the journal (`JournalLimits`): commits in flight; outputs held across
    decisions; writes per commit, the deployment's record among them;
    outputs per decision; a row's bytes; a result's bytes; a charter's
    bytes;
  - loads: in flight, rows per page, the bytes kept and a page's bytes;
  - briefs: gathers, sections per gather, parts per section, a section's
    read bytes, a budget per kind of section (the task, dependencies, CI,
    reviews, the pull request, attempts, the plan, notes), a brief's
    bytes and the gather's deadline.

    The gather's deadline bounds a connector that never answers. Its
    default, 30 seconds, outlasts a brief's sequential forge reads: a
    pull section reads Item, Pull, PullFiles and Compare, each expanded
    into GitHub requests, with file contents adding reads. Even six to
    eight sequential requests at a slow but healthy two seconds each,
    with a token mint first and one retry after backoff, can occupy
    about 20 seconds. Thirty leaves room that ten cannot, rather than
    failing a healthy brief for waiting, and still ends a stuck connector
    within the same sitting. These are planning latencies, not a bound
    on every forge response.

  They give the core its resume limit, within a row's bytes, its
  charters' bytes, its load slots and its briefs'.
- **Checked against each other** at start: the held outputs at least
  three decisions' worth; a decision's outputs at least what every
  worker's fleet events may ask in one; a commit's writes at least the
  route's and the call records; a row's bytes at least the largest task
  row and the forge's largest answer; a result's bytes at least a brief,
  a task's result and a party's words; the forge's named calls at least
  the call records, and its brief sections and bytes at least what the
  briefs may plan.
- **The room per event** (section 4): the writes the children may make
  along the whole route, hand-offs included, with the call records; the
  held outputs of one decision.
- **The watches' rooms are temper's:** 16 a party, 16 a stream and 16 in
  all, above jig's profiles' 4, since temper's client opens 16
  (`../protocol/configuration.md`, section 7). The rest of the views'
  limits are jig's small profile's: 4 runs watched, 65,536 bytes of a
  run's text, 32 subjects marked per change, 32 facts.
- **The store's fill mark and a read's bytes** are jig's small profile's
  numbers, 268,435,456 and 65,536 bytes, held as the root's constants
  (section 13).
- **The worst case:** the core's, the forge's, the journal's, the loads',
  the landing policy's (designed, not built: the forge's rules, priced
  by the forge, and the settings' values, by jig) and the permission
  mappings', and every value in
  flight (section 2), summed in checked arithmetic (programming-model.md,
  6.3). The service computes it at start; limits whose sum overflows, or
  that break a check above, refuse the start.

## 11. What the root never does

- decide anything of the core's or the forge's: what to commit, what
  waits, whether an effect may be made, what a change does next, the
  order of a restart, whether a setting of temper's kinds is well
  formed;
- send an output any way but through the journal;
- keep anything between steps but its children, the journal and the
  values in flight of section 2;
- read or rewrite a value it carries, beyond wrapping and translating
  it;
- give the core a type of the forge's, or the forge one of the core's;
- route by content beyond the variant;
- price a forge effect, or grant the forge's API token, App key or
  webhook secret.

## 12. The world

The root has no child domain of its own; its worlds are the engine's
(`testing.md`, section 2):

- **The engine's world,** `tests/engine/domain`: the root with jig's real
  children and the forge's crates, over jig's store through the service's
  store routing, an independent fake forge below, a scripted person and
  worker, and the protocol layer played (the hub's records, host calls
  decoded and rendered, ended outcomes read, accounts). The engine
  restarts cold at drawn moments and at each commit a scenario chooses.
- **The conformance world,** `tests/conformance`: the root on jig's
  harness and its outside referee, with jig's shared scenarios bound to
  the forge and temper's own stories, crashed at every commit (jig's
  `domain/testing.md`, sections 5 and 8).
- **The system worlds:** the root with the worker and smith's agent
  (`tests/system`), and with temper's client (`tests/client`)
  (`testing.md`, 2.1).

Its stories, as the root meets them: a chat that answers, parks and
resumes from its transcript's window; a chat past its resume limit
starting fresh, its tail in its brief; a tool call whose answer is lost
with the channel and the engine, asked again after a restart and
answered from its record, made once; an undecodable call answered with
no commit and no record, then decided once under a new name; a lost
attempt told of the calls committed after its last turn; calls under
backpressure handed back and decided once; a failed preparation reported
as a fact of its attempt and retried by its answer's class; an answer
pushing more heads than its task may hold, failing as invalid; saved
work reaching the next attempt's checkout; an adoption refused for a
forge not configured, a repository not found or not readable, and one
adopted with its collaborators seeded into roles, sent again and
answered its kept outcome, across a restart too; a forge effect
committed before its write and made once, and one proposed and kept
until its holder accepts; a stop with a committed forge effect, making
no commit and no write; a start refused for live tasks past lowered
limits, or for a malformed page. The forge's own stories are forge.md,
section 18's.

Its Open chats' stories (`agent.md`, 4.6), in the engine's world on
a chat ended by its requester, as temper's is configured (temper-init's
golden shows the flag shipped): a chat whose run answers and
finishes parks, its reply committed and no try spent, still at rest
past its idle interval; its person's next words wake it once; its
person's Close, the client's `close_chat`, ends it with its committed
turns kept and its delegates' fates as the Close names them; a goal's
delegate in the same world still ends by its result; and a chat whose
ordinary source is a program endpoint its worker's hello declares is
refused with its typed cause, starts nothing, and reads `HostRefused`.
In the root with the worker and smith's agent, an Open chat's text-only
reply ends its turn after one call, with no nudge and no failed run.
The engine's other stories run its walking chat, ended by its result
as jig's default chat is: what they test is a result's terminal path,
which temper's goals still take.

Its referees hold the root to jig's promises from outside (jig's
`domain/core.md`, section 11): the walking referee reads durable rows,
never the root's state; a referee per promise rejects a fault injected
against it; jig's conformance referee checks that nothing a party, a
host or the forge saw depended on a lost commit, and that every effect
the fake forge saw, in its own terms, is the one the core decided. Each
step's memory is measured against the declared worst case.

## 13. Open questions

- **What the root keeps between steps** (section 2). jig's root keeps
  nothing between steps but its children and the journal (jig's
  `domain/root.md`, section 7). Whether each value in flight moves to its
  owner. Default: kept, each bounded.
- **The forge's work done in the root:** choosing a run's repositories,
  planning the forge's brief sections from a change's state, making a
  change's delegates (sections 6 and 7). jig's connectors gather their
  workspace items and plan their sections themselves (jig's
  `domain/connectors.md`, sections 9 and 10), but temper's forge names no
  type of jig's tasks. Default: translation in the root.
- **The store's fill mark and a read's bytes** (section 10): taken from
  `engine.limits` as the core's other limits are, or kept as the root's
  constants. Default: constants.
- **A failed preparation's reason on the task's page** (7.4): jig's core
  keeps an attempt's end as a class only; temper would ask jig to keep
  its detail. Default: the fact alone.
- **Procedures other than the forge's change:** the boundary carries a
  procedure's step out and its decision back (`Delivery::Procedure`,
  `PartyInput::ProcedureStep`), which temper's worlds use; temper's
  service steps none, and stops if one is asked. Default: none.
