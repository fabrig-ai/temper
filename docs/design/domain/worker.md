# The worker

Provisional, 2026-10-04, revised 2026-10-22. What temper's worker does,
as a domain layer: its root, which composes jig's hub with smith's host
domain and temper's checkout; the items an assignment gives a run's
workspace; and the checkout, which is temper's own. The worker hosts the
runs of agent tasks (jig's `domain/engine.md`, section 7). Its hub, a
hosted run's lifecycle, its failures, turns, facts and losing contact
are jig's (jig's `domain/hosts.md`); the agent it starts is smith's, as
agent.md says, and the worker is smith's host (smith's `domain/host.md`).
The mechanics are those of skein's `programming-model.md`. What is still
open is listed in section 12.

## 1. In one page

- **A worker is an execution host,** jig's third shape: agents on
  workers (jig's `domain/hosts.md`, section 3). The engine decides what
  work exists and keeps it in its store; the agent does the LLM work. The
  worker prepares workspaces, starts agent processes and keeps them in
  bounds, pushes the changes they make, and relays everything else
  between a run and the engine.
- **The worker keeps nothing durable.** What must survive lives in the
  engine's store (claims, transcripts, results) or on the forge (pushed
  branches, saved work). A worker can be killed at any moment and any
  worker can take any run; its checkouts are caches.
- **It hosts runs, not jobs.** A run may finish once, or wait for
  messages, take them and park (smith's `domain/run.md`, section 6): it
  starts fresh or from its transcript, takes messages, makes calls, sends
  turns, and parks or ends (jig's `domain/hosts.md`, 6.2).
- **Policy is data.** The worker interprets no workflow vocabulary: no
  task, role, verdict or tool names. An assignment says which
  repositories to check out and how, what to save, and the charter to
  hand the agent, which is smith's and which it carries as bytes.
- **Its one forge write is a git push:** a change its run delivered, or
  saved work. Push credentials stay in the worker's protocol layer; an
  agent never sees them. Everything else that touches the forge goes
  through the engine.
- **The checkout is temper's** (section 5): git workspaces cached by
  workstream, a merge in progress as a place to start, a push that
  commits exactly the tree the run checked and is never forced, and no
  credentials on disk.
- **The rest is jig's and smith's:** admission, fenced attempts, turns
  and answers kept until the engine acknowledges them, losing contact and
  shutting down are the hub's (jig's `domain/hosts.md`, section 6); the
  agent process, its channel, its watchdog and cancel then kill are
  smith's host domain's (smith's `domain/host.md`, section 4).
- **The domain is complete** (programming-model.md, section 4). A world of
  domains and fakes runs everything the worker does, with no protocol and
  no io (section 10).

## 2. The worker in the system

```
engine   jig's core, the forge connector, the store: what runs, with what; commits what runs say
worker   jig's hub: workspaces, agent processes, pushing their changes; relays the rest
agent    smith: LLM work, one run per agent process, reporting to the worker
```

- **The contract is jig's** (jig's `domain/hosts.md`, section 2). The
  worker dials the engine and keeps one channel open, through jig's
  worker link in its protocol layer (jig's `protocol/channel.md`,
  section 10); it says its hello first; the engine holds the claim and
  fences attempts; the worker keeps turns and answers until the engine
  acknowledges them, once committed (jig's `domain/hosts.md`, 6.1 and
  6.4). temper adds its parts beside jig's records: the workspace items
  down, what a run left on the forge up (4.1; `../protocol/channel.md`,
  sections 4 and 5).
- **The engine never reads a repository.** It works from what the forge
  connector reads; what a checkout holds (a repository's checks, its
  `AGENTS.md`) is for the run to find (agent.md, section 2).
- **The stop bound** the hello states is jig's (jig's `domain/hosts.md`,
  6.6), with temper's push as the delivery and temper's save as the save:
  the time the link takes to notice the loss, then the grace, then the
  longer of the time smith's agent may take to be gone after a stop and a
  push's deadline, then a save's commit and push. The worker's
  configuration derives it (`../protocol/configuration.md`, section 4);
  the domain takes it whole.
- **Losing contact and shutting down** are the hub's (jig's
  `domain/hosts.md`, 6.6 and 6.7). Past the grace the worker cancels its
  runs itself, saving their work first. Shutting down, it still says its
  slots and refuses every assignment as busy; it gives up the turns and
  answers it keeps, counted, only once no run is left and the engine is
  still out of reach past the grace.

## 3. Structure

```
temper-worker-domain                 the worker's root: faces the protocol layer; routes and translates
├── jig-domain-host                  jig's hub: hosted runs, admit, prepare, start, relay, park or end
├── temper-worker-domain-checkout    the workspace: prepare, merge, commit, push, save; the cache
└── smith-host-domain                the agent: processes, spawn, channel, watchdog, cancel then kill
```

The tree is jig's worker root (jig's `domain/hosts.md`, section 4), with
temper's checkout as its workspace capability and smith's agent
processes as its agent capability. The root owns its children's state
and routes between them. The three share no types, so the root
translates between them through small total functions, keeping for each
workspace what each side names it by, and temper's own parts: the
workspace items down, what a run left on the forge up. smith's payloads
are handed out to the protocol layer to decode and encode (jig's
`domain/hosts.md`, 6.8); the root links no codec of smith's. A hand-off
goes from a capability to the hub, from the hub to a capability, and at
most once more back and forth, so each entry point completes them before
it returns, with no ready list.

The root's vocabulary toward the protocol layer (`Event`, `Request`) is
what crosses: jig's link records, with temper's parts beside an
assignment and an answer; smith's payloads by hand-out; the agent
processes' io, as smith's host domain defines it; and the checkout's git
and file operations.

## 4. Hosted runs

### 4.1 The assignment

What the engine gives the worker for one run is jig's assignment (jig's
`domain/hosts.md`, section 2; jig's `domain/engine.md`, 7.1), with
temper's workspace items beside it:

- **Run and attempt.** The engine's names for the run (its task's number
  and the run's) and for this attempt at it; every message about the run
  carries both.
- **Workspace.** A workstream, which names the checkout so later runs of
  the same work find it cached: the number of the task that holds what is
  written (jig's `domain/engine.md`, 7.1), by which the checkout keys its
  cache. And the repositories, each with:
  - its deployment tag, by which what the run left names it;
  - its name, the directory it sits in and what the agent calls it, one
    safe path component;
  - its remote: the URL git fetches from and pushes to, which the
    engine's protocol layer makes from its forge's configuration
    (`../protocol/channel.md`, section 4), so a worker needs no forge
    configuration;
  - its git, where its forge has its own: the program every git
    operation on it runs, a fake forge's, which the domain carries to
    each operation and never reads; otherwise the worker's own git
    (`../protocol/git.md`, section 3);
  - where to start: a base branch, a branch, a commit by its full object
    id, saved work, or **a merge in progress**: a branch, and the commit
    of its base to merge into it (section 5);
  - whether it may be written, and if so the branch a change is pushed
    to, and the head that branch must still be at, if the assignment
    names one (section 5);
  - an optional landing branch for an inherited writable change producer
    that starts on its push branch: the comparison for existing work,
    separate from the checkout start and the push's expected head;
  - the identity for every git operation on it: one of temper's
    application credentials (jig's `domain/engine.md`, 12.1), granted
    with the assignment, which the protocol layer maps to a token and a
    commit author; and on GitHub the account whose entry of that
    identity's value authenticates git on it (`../protocol/channel.md`,
    sections 4 and 6), carried with the identity into each git operation
    that reaches the forge, which the domain copies and never reads.
- **Saving.** Whether to save unfinished work, and to which branch.
- **Charter.** smith's, as the engine wrote it (agent.md, section 4),
  carried as bytes. The worker gives it to the agent with the workspace
  it prepared: where the repositories sit, which may be written, and for
  a merge in progress the files in conflict (smith's `domain/run.md`,
  3.2).
- **Transcript,** for a run that resumes: its current window, and the
  calls committed after its last turn (jig's `domain/engine.md`, 7.2;
  jig's `domain/hosts.md`, 6.4). A delivery among those calls comes with
  its evidence, smith's record of the push's terminal, which must decode
  and agree with the outcome the engine committed (4.3;
  `../protocol/channel.md`, section 2). The worker carries the rest
  opaque, within its limits.
- **Grants.** The credentials the run's LLM accounts and git identities
  need, as names the protocol layer fills (`../protocol/credentials.md`,
  sections 3 and 4). The hub hands the agent its LLM accounts' grants
  only; the worker keeps the git identities' for its own git operations.

Its sizes and names are checked at the entrance against the worker's
limits, and so is a repository named twice: what is beyond them is
refused as an invalid start that names it (jig's `domain/hosts.md`, 6.3).
A run with no repository needs no workspace: the worker prepares nothing
and starts smith with none (jig's `domain/hosts.md`, section 7). A chat
that names no repository runs so.

### 4.2 Lifecycle

A hosted run's lifecycle is the hub's, in nine steps (jig's
`domain/hosts.md`, 6.2). What the worker puts in them:

- **Step 2, prepare:** the checkout prepares the workspace from the items
  (section 5).
- **Step 3, start:** smith's agent process (section 6), with the charter
  and the transcript decoded by hand-out, and the workspace as the
  checkout prepared it.
- **Step 4, relay:** smith's `deliver` is a push, which the checkout
  serves, one at a time (section 5; agent.md, section 6); every host tool
  goes to the engine, and its answer comes back.
- **Step 6, stop:** a push in flight goes on to its end, and what it
  lands counts (section 5).
- **Step 7, save,** if the assignment asks, at park, at an unfinished end
  and at cancel: commit what the writable repositories hold and push it
  to the saved-work branch, so a later run on any worker starts from it.
  A run that ended with a landed change has nothing left to save, and
  nor has one whose agent never started, nor one that started from a
  merge in progress, whose next attempt prepares the merge again; one
  that landed mid-run, then parked, failed or was cancelled still saves.
- **Step 8, release** the workspace to the cache (section 5).
- **Step 9, answer:** jig's answer, with temper's parts beside it: what
  the run left on the forge (the last commit landed in each repository,
  the head a pull request is to show) and how its save went, each
  repository named by its tag (`../protocol/channel.md`, section 5).

A push that lands while its run winds down is the run's result (smith's
`domain/run.md`, 8.2). The hub arms no timers: each wait is bounded
below it, by the deadlines of git operations, the agent's watchdog and
wall time, and the grace of cancel, then kill.

### 4.3 Failures

jig's types (jig's `domain/hosts.md`, 6.3), with temper's detail:

- **Refused** at the entrance: busy, or invalid, saying what is beyond
  the limits or which item the workspace cannot take. Once prepared, a
  start smith refuses, or whose charter, transcript, call names or
  delivery evidence do not decode or agree, is refused as invalid too.
- **Preparation failed:** transient (the forge could not be reached, the
  worker's side failed or ran out of time, another run of the workstream
  holds its checkout, or every checkout is held), or permanent until
  something changes on the forge, naming the repository: the forge does
  not have it, a branch or a commit, or it refuses the identity.
- **The run failed,** the **agent failed,** or the run was
  **cancelled,** as jig's say, each with bounded detail for operators
  that no LLM is shown.

**A configured model-call bound** is reported as jig's
`RunFailure::Exceeded(Exceeded { bound, value })`. Smith's
`CompletionFailure::Limit { which, bound }` supplies `value` unchanged:
the configured value in the bound's own unit, never a measured size.
The worker classifies both `Limit` and `Limit::Path(Cap)` exhaustively,
without a wildcard, so an added smith cap requires a classification.

| Smith limit | Jig bound | Unit |
|---|---|---|
| `Limit::Path(Cap::Reasoning)` | `Reasoning` | bytes, per item or in all |
| `Limit::Path(Cap::Metadata)` | `Metadata` | bytes |
| `Limit::Path(Cap::OutputItems)` | `Items` | answer items |
| `Limit::Path(Cap::Answer)` | `Answer` | bytes |
| `Limit::Path(Cap::Tools)` | `Tools` | tools |
| `Limit::Path(Cap::HistoryItems)` | `HistoryItems` | request messages and blocks, counted together |
| `Limit::History` | `History` | a retained-history ceiling on messages, blocks or bytes; the tag does not identify which |
| `Limit::Window` | `Window` | tokens |
| `Limit::Path(Cap::Output)` | `Output` | declared output-ceiling tokens |

For `History`, the value is the ceiling that compaction could not bring
the retained history under. It is carried unchanged, without assigning
it a unit the tag does not identify.

- **The included bounds name the call's allowance.** The answer's
  reasoning, metadata, items and bytes, and the request's tools, history,
  window and declared output ceiling become `Exceeded`. The tasks child
  holds the run at once without counting a try (jig's `domain/tasks.md`,
  5.5). The classification does not infer a measured size from the
  failure or require proof that every provider response will repeat.
- **Other limits stay `Model`.** `Cap::Request` stays `Model` rather than
  being identified with a retained-history or model-window bound.
  `Cap::RequestHead` depends on the credential. `Limit::Call` and
  `Limit::Message` have no production constructor. `Cap::ResponseHead`,
  `ResponseFields`, `Skip`, `Field`, `Retained`, `Tokens`, `Depth`,
  `Strings`, `Number` and `ErrorBody` can describe framing or a transient
  error body, rather than the call's configured allowance.
  `Cap::AccessToken` and `Cap::AccountId` depend on credentials.
  `Limit::Pool` is a connection reservation, which can depend on
  credential-bearing request headers; `CompletionFailure::ApplicationLimit`
  also covers temporary call-slot refusal. None is classified as one of
  the nine call bounds. Other completion failures retain their `Model`
  classification.

## 5. Checkouts

The checkout child domain is the worker's workspace capability (jig's
`domain/hosts.md`, section 7): it owns the workspaces on the worker's
disk. Its vocabulary is typed git and file operations (make a workspace,
clone, fetch, check out, merge, commit, push), which the protocol layer
runs as contained processes and whose output it parses.

- **A cache, keyed by workstream.** The runs of one piece of work (a
  change's producing, its repairs, its resolutions) share a key, so they
  find the repositories already cloned. The engine runs them one at a
  time; a worker given a second while the first holds the checkout fails
  its prepare as transient. A cached checkout is reused only if it holds
  exactly the repositories named, by directory and remote, and is reset
  to the assignment's starting point: nothing local is authoritative.
  One that a killed or broken git may have damaged is not trusted, and is
  rebuilt from an empty directory, as a new one is. The cache holds a
  fixed number of checkouts; once they are all made, a new workstream
  takes the least recently used one that no run holds.
- **Repositories side by side.** A workspace holds one directory per
  repository, so code that refers to a sibling by path works, and the
  agent's write authority is the list of writable ones.
- **One operation at a time.** A run holds its checkout from the moment
  its prepare is admitted, so a cancel aborts a prepare rather than
  waiting it out. A checkout runs one operation at a time (a prepare, a
  push or a save), each a sequence of git operations with deadlines as
  data, one for the forge and one for the disk; the checkout arms no
  timers.
- **Prepare** fetches what the assignment names and checks out exactly its
  starting point. A branch or base that does not exist is a preparation
  failure, permanent until something changes on the forge: the worker
  creates no branch but by pushing a run's change, and branches temper
  needs before then are made by the engine, as effects (forge.md,
  section 11).
- **An existing producer branch** also names its landing branch. After
  fetching the start, prepare fetches that landing branch on the same
  remote and freezes its tip for this hold; a missing landing branch fails
  preparation. The next hold fetches it afresh, so the landing tip may move
  between attempts. This comparison changes neither the checkout start nor
  ownership, drift admission or the saved-work baseline.
- **A merge in progress** fetches the branch and the base commit, checks
  out the branch, and merges the base into it as git does, leaving the
  files in conflict with their markers and the merge's state in the
  repository. The agent edits the files and runs the checks; it has no
  git writes (smith's `domain/tools.md`, section 5). Its push commits the
  tree as the merge, with two parents, the branch's head and the base
  commit, even when the tree is unchanged from either; a file the merge
  left in conflict that still holds a conflict marker is refused at
  push, and the run is told which.
- **Push commits what the run checked.** When a run asks to deliver, at
  `finish` with a change or mid-run, it has run the repository's checks
  with nothing else writing to the checkout (smith's `domain/run.md`,
  8.1); the worker commits exactly that tree and pushes the commit, with
  the run's title and body. A push is a fast-forward and never forced: if
  the branch moved incompatibly with the run's commit, the push fails and
  the run is told. When the assignment names an expected head, the
  remote must also still be at that exact head, so no movement of the
  branch since the run started goes unseen; without one, a rewind of the
  branch to an ancestor does (section 12).
- **Each repository lands on its own.** Pushing several is not atomic:
  the run is told done only if every repository with a change landed it,
  moved if a branch cannot take the commit as a fast-forward, and nothing
  to push if none had a change. A repository whose head this hold's last
  push already landed is that landing again, not unchanged: a `finish`
  after a mid-run delivery, with nothing new, delivers what the run
  delivered. On a new hold of an inherited producer that starts on its
  push branch, an unchanged head not contained in the fetched landing tip
  is existing unlanded work. The worker pushes that head again, requiring
  the remote to equal the assignment's expected head, or the fetched start
  when none was supplied. Only successful or verified push returns its
  landing. An equal or already-contained head is unchanged. Ordinary
  runs, read-only items, saved starts, repairs and resolutions do not use
  this comparison; saves still compare against the actual checkout start.
  A push the forge refused (permissions, a
  protected branch, a hook) is a landing of its own, which a retry will
  not change.
- **Pushes are never cancelled.** A push in flight runs to its end, which
  its deadline bounds, so one that lands is reported landed; one that
  ended ambiguously (out of time, broken, the forge lost on the way) is
  verified against the forge, by fetching its branch.
- **Saved work** is ordinary commits on the saved-work branch, pushed the
  same way. A run that resumes from saved work starts where the last one
  stopped. Work done since the last save is lost if the worker dies.
- **No credentials on disk.** Every git operation that reaches the forge,
  or commits, names the repository's identity, which the protocol layer
  maps to credentials or an author for that invocation only, never into a
  checkout's configuration, where an agent could read them.

## 6. Agent processes

The agent child is smith's host domain, `smith-host-domain` (smith's
`domain/host.md`, section 4): jig's agent processes (jig's
`domain/hosts.md`, 5.1). One process per run, whose process tree is the
run's containment boundary (section 9); one channel over its pipes,
whose rules smith's host domain enforces; a watchdog on progress and a
bound on wall time; cancel, then kill; the slot back only once the
process has exited and its tree is empty. temper's choices:

- **Agent processes, always.** Every run is a process of its own, also
  when the engine and the worker share a process
  (`../protocol/README.md`, section 8); `smith-inline-agent` is not used
  (`../protocol/llm.md`, section 7).
- **temper's own binary is the agent:** `temper agent`, smith's agent
  shell (`../protocol/README.md`, section 4). temper builds no code into
  it.
- **No credentials in its environment.** The tokens its LLM calls use
  come as grants on its channel (`../protocol/credentials.md`,
  section 3); git's never reach it.
- **The repository's checks are a long operation** the run reports, so
  the watchdog stretches to their span: a two-hour test suite is not a
  hang.
- **Its limits** are smith's host limits, and the bounds the hello
  states, from the agent's configuration (`../protocol/configuration.md`,
  section 5).
- **Its program endpoints,** in the hello's bounds: the endpoint numbers
  the agent's configuration serves by a program, its `endpoint_programs`
  keys, as jig's reference launcher declares them (jig's
  `domain/hosts.md`, 8.1). temper's stock agent serves none, an empty
  list that proves no endpoint is a program; a chat whose ordinary
  source is one is refused before it starts, `HostRefused` in its
  status (`agent.md`, 4.6).

## 7. Facts

- **A run's facts** go up to the engine as jig's activity, through the
  hub (jig's `domain/hosts.md`, 6.5): smith's channel facts, decoded by
  hand-out, which also feed the watchdog. A run's live text goes up
  beside them when its assignment asks for it: smith's pieces, handed
  from its channel to the hub as jig's pieces (jig's `domain/hosts.md`,
  6.5), each also progress for the watchdog, and best effort as facts
  are.
- **The worker's own facts:** each child domain's, content-free (tokens,
  kinds and counts, never a name, a branch or a message), gathered after
  every entry point into one bounded queue the loop drains, for
  operators. What does not fit is dropped and counted.

Nothing the worker or the engine decides depends on a fact arriving, and
no fact holds a slot.

## 8. Turns

Turns are the hub's, as jig's `domain/hosts.md`, 6.4 says: smith's
bytes, of which the worker reads only what comes beside them, kept until
the engine acknowledges them, and lost with the run when the worker
dies, so the task's next run starts from the last committed turn.

## 9. Below the domain

What the protocol and io layers owe the domain. The worker's protocol
layer is the worker's service (`../protocol/README.md`, section 7): jig's
worker link, smith's host half and agent processes, temper's identities'
table and the git component.

- **The engine:** jig's channel (jig's `protocol/channel.md`), dialled by
  the worker and authenticated, carrying the hub's link vocabulary, with
  temper's parts as temper's bytes under temper's payload version
  (`../protocol/channel.md`). The run's facts go after what the domain
  emitted, so none goes before the hello. The root keeps each run's
  items until the engine has its answer, so the parts going up name each
  repository by its tag, and the engine's protocol layer, which keeps no
  state, needs nothing else to read them. In one process the link joins
  the engine by an in-process address (`../protocol/README.md`,
  section 8).
- **Agents:** spawning the agent within a deadline, in a contained
  process tree, with proof that the tree is empty after it stops; where
  skein gives no contained trees (jig's `protocol/README.md`,
  section 10), each agent in a process group of its own, which a stop
  signals whole. smith's channel over its pipes (smith's host half); an
  environment without forge credentials. smith's payloads (the charter,
  the transcript, turns, facts) are decoded and encoded by hand-out (jig's
  `domain/hosts.md`, 6.8). The root completes the decoded start with
  where the repositories sit, as the checkout prepared them, and, for a
  merge in progress, which files are in conflict.
- **Git:** each typed operation is a fixed sequence of git invocations
  under the operation's one deadline (`../protocol/git.md`, section 4),
  its output parsed into a typed outcome. A commit is named by its full
  object id (a SHA-1 id zero-padded to a SHA-256 id's 32 bytes); a merge
  leaves its conflicts in the working tree and says which files; a
  commit may have several parents. An operation that ran out of time or
  was cancelled ends only once its process has exited, so nothing of it
  touches the workspace afterwards. A repository's identity is mapped to
  credentials and an author from the identities' table, per invocation,
  in git's environment only, never in an argument, a URL or a file
  (`../protocol/git.md`, section 5).
- **Files:** workspace directories under the worker's `root`
  (`../protocol/configuration.md`, section 4). Making one empties it
  first, so what the cache evicted, or a worker left behind before a
  restart, cannot fail a prepare.

## 10. The world

The worker's worlds run the domain against neighbours that share none of
its types (testing-strategy.md, section 4). `testing.md` places them
among temper's tiers and keeps their fakes.

- **The checkout's world** (`tests/worker/checkout`): the checkout, the
  world its parent, against a fake forge and a fake disk. Its stories: a
  change prepared, edited, pushed and landed; a workstream reusing its
  workspace, and others evicting the least recently used; a held
  workstream busy, and a full cache refused; a spec beyond the limits
  refused; a spec naming what the forge lacks failing for good; an
  unreachable forge failing for now and a refusing one for good; a
  branch another party advanced making the push moved, nothing forced;
  saved work where a later run resumes; an abort ending a prepare under
  way; merges that conflict, keep their markers until resolved and
  commit both parents, a stale expected head refused, and a push that
  lands but times out; a first hold pushing, releasing, and a new hold
  delivering the same existing diff without another edit; landing ancestry,
  missing landing branches, moved remote heads and unchanged save semantics.
- **The worker's world** (`tests/worker/domain`): jig's hub, smith's host
  domain and the checkout, against scripted engine and agent peers, the
  protocol layer's routing played with smith's codecs, and the fake
  forge and disk. Its stories: a prepared run delivering its complete
  tree; a cancel mid-delivery waiting for the landing; contact restored
  within the grace, and lost past it, saving before the cancellation's
  answer; the hello carrying the configured stop bound whole and the
  workstreams the checkout holds; a branch the forge lacks failing the
  prepare, naming the repository; a save saying whether each repository
  started from saved work; turns surviving busy, reconnects and crossed
  acknowledgements; a worker shutting down past the grace giving up what
  it keeps, and a channel reopened in time getting every turn. Its
  runtime world's referee checks the whole boundary (`testing.md`,
  6.2).
- **The system world** (`tests/system`): the engine's and the worker's
  roots, smith's real agent domain over skein's fake LLM, jig's fake
  store and parties, and the fake forge, which the engine and the
  worker's git share, so a branch the worker pushes is the head the
  engine reads. Its stories: a chat's report through the worker and
  smith; a host call and its committed answer across the agent's
  channel; a person's words and stop reaching smith; an adopted
  repository prepared and mounted in smith; a parked run resuming
  through both roots; a withheld commit holding the assignment.
- **The one-process world** (`tests/one-process`): temper's services on
  skein's simulator, the worker's agents smith's processes over skein's
  fake LLM (`testing.md`, 2.5).

Across them, each world checks what is its: that the engine commits each
attempt's turns and answer once and acknowledges them in time; that each
relayed call reaches the engine once and is answered once; that what
lands is exactly the tree the agent left (a merge with both parents when
it started from one) and saved work exactly the tree at the stop; that
no git operation runs while a run's agent may, but the push it asked
for; and that nothing is live once it settles.

## 12. Open questions

- **A push's message:** the worker commits the run's title and body; how
  a change's pull request takes them, and whether a squash keeps them.
- **An assignment naming an expected head.** The worker enforces one when
  given (section 5), but the engine names none, so a branch another
  party rewound to an ancestor since the run started still takes a push
  that restores what that party removed. With branches held by one task
  (jig's `domain/connectors.md`, 3.3), such a rewind is drift the forge
  looks for after the push (forge.md, 8.5). Naming the head prepare saw
  needs a decision for a branch that did not exist then.
- **Several workers in one world:** the whole worker's world runs one;
  placement across several is exercised with scripted workers in the
  engine's world.
- **Checks without an LLM:** validating an exact head needs a checkout
  and the repository's checks but no conversation. It could be a run
  whose outcome is its checks' result, for repositories without CI
  (forge.md, section 20).
- **Code graph:** indexing a checkout for codebase-memory-mcp as part of
  preparing it, and keeping that index with the cache.
- **A workspace's repositories in parallel:** a checkout runs one
  operation at a time, so its repositories are cloned, fetched and pushed
  one after another.
- **jig's:** saving periodically, and a worker's failed dials before its
  first channel (jig's `domain/hosts.md`, section 12).
