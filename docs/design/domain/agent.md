# The agent

Provisional, 2026-10-05, revised 2026-10-23. temper's agent is smith's:
the standard agent built from smith, the kit for LLM agents, on skein
like temper. smith's design is its own (smith's `domain/run.md`,
`domain/session.md`, `domain/tools.md`, `domain/host.md`), and how a
run's charter, tools, contract, messages and delivery become smith's is
jig's (jig's `domain/hosts.md`, sections 8 and 9). This document says
what temper fills in: the charters its engine writes, the host tools its
engine serves (4.3, the one table of them), its result contracts and
conventions, delivery as a push, and the agents temper runs. What is
still open is listed in section 10.

## 1. In one page

- **temper runs smith's stock agent,** on workers, one process per run:
  temper's own binary run as `temper agent`, smith's agent shell
  (`../protocol/README.md`, section 4). temper builds no code into it. A
  run's charter and its host's answers are all that make it temper's.
- **The engine writes the charter** (section 4): the task's instructions,
  its brief rendered, every host tool, a result contract with temper's
  verdicts and a change's `title` and `body`, `.temper/pre-pr` and
  `AGENTS.md` as conventions, and a budget in the deployment's unit with
  its models' prices.
- **The host tools are temper's** (4.3): jig's core's tools under temper's
  own names, and the forge's reads, effect and topics.
- **The worker is smith's host** (section 6; worker.md): it prepares the
  workspace from the forge, supervises the agent's process with smith's
  host domain, delivers a change by pushing it, and relays the rest to
  the engine.
- **The engine keeps what smith leaves to hosts:** transcripts, turn by
  turn, in its store; host tools' calls, decided once; what follows an
  answer (jig's `domain/engine.md`, section 7).
- **One translation, temper's own.** `temper-engine-smith`, which the
  engine's routing calls, is the only place where temper's charter,
  tools and results become smith's, and smith's back. temper uses
  neither `jig-protocol-charter` nor `jig-protocol-tools`, since its root
  has a `Tool` vocabulary of its own (`../protocol/llm.md`, sections 4
  and 5). smith interprets no temper vocabulary, and the worker none of
  either.

## 2. The agent in the system

```
engine   jig's core and the forge, the store; writes each run's charter and serves its tools
worker   smith's host: workspaces, agent processes, pushing their changes; relays the rest
agent    smith: LLM work, one run per process, reporting to its worker
```

- **The engine is the only client** of the store and of the forge. It
  answers the host tools a run calls, and commits the run's turns and
  result. The worker's one write is pushing a run's branch, over git.
- **The engine never reads a repository.** It works from what the forge
  connector reads; what a workspace holds, such as a repository's
  `AGENTS.md` or its checks, is for the run to find.
- **The agent acts on the world only through the worker:** a delivery,
  which the worker makes as a push, and host tools, which the worker
  relays to the engine. It never pushes, merges or writes to a system on
  its own authority.
- **Retrying a failed run is decided above the run,** by the engine,
  across attempts. A run answers once and is done.

## 3. What temper takes from smith

- **In the domains:** the engine's root links no smith crate; charters,
  transcripts and results are bytes to jig's core (jig's
  `domain/hosts.md`, 6.8). The worker's root composes `smith-host-domain`
  as its agent child (worker.md, section 3). `temper-engine-smith`, the
  translation of section 4, is step code over `smith-domain`,
  `smith-domain-run` and `smith-domain-session`.
- **In the protocol layers:** smith's codecs, its host half and its agent
  shell (`../protocol/README.md`, section 5).
- **In the worlds:** smith's agent domain over skein's fake LLM, its
  agent processes, and its codecs (section 9).
- **LLM accounts are jig's,** made from the client, on skein's accounts
  driver. A run's grants reach it through its worker and smith's host
  half: for each LLM provider it uses, a credential with its scheme, or
  a program source's name, whose login the host checks and the program
  reads (`../protocol/credentials.md`, section 3; jig's
  `protocol/llm.md`, section 6). The agent runs the loop itself, or
  through the program (smith's `protocol/programs.md`, section 2).

## 4. temper's charter

The core decides a run's charter in its own terms (jig's
`domain/engine.md`, 7.1); `temper-engine-smith` writes it as smith's
(smith's `domain/run.md`, 3.1) from the root's assignment, and the
engine's routing encodes it (`../protocol/llm.md`, section 5).

### 4.1 Instructions and brief

- **Instructions:** the charter's, from the deployment's configuration
  (jig's `protocol/configuration.md`, section 3): a chat's, a
  coordinator's, a reviewer's with a lens, a change's producer's; until
  an amendment gives a task its own (jig's `domain/tasks.md`, section
  7), whole, where an empty text leaves it none, not its charter's.
  The task's next run carries them; a run already live hears them as
  the amendment's words. Designed, not built: a charter is a setting in
  the store, read from the revision the task runs under (jig's
  `domain/engine.md`, 17.3), temper's stock chat's and coding agent's
  held in the binary as their defaults
  (`../protocol/configuration.md`, 13.4).
- **The brief:** the sections jig's core gathers (jig's
  `domain/engine.md`, section 9) and the forge's (forge.md, section 2),
  each rendered as one of smith's titled sections, titled by its kind:
  `Task`, `Lineage`, `Dependencies`, `Plan`, `Earlier attempts`,
  `Calls`, `Waiting`, `Notes`, `Transcript tail`, and the forge's
  `Pull request`, `CI` and `Reviews`. A section its gather could not fill
  says why: its read failed, timed out, or exceeded its limit. smith adds
  the sections about its own mechanics, and each repository's guide.

### 4.2 Workspace

The connectors name the repositories, where each starts, which may be
written and the branch a change goes to (jig's `domain/engine.md`, 7.1;
worker.md, 4.1); the worker prepares them and gives smith the workspace:
each repository's directory and name, whether it may be written, and for
a merge in progress the files in conflict (worker.md, section 5). Every
temper workspace is git repositories. A run with no repository, such as
a chat that names none, runs with no workspace (worker.md, 4.1).

### 4.3 Tools

- **smith's own families** by the charter's policy: inspecting,
  modifying and the shell in the workspace, and sub-agents, each as the
  charter's `inspect`, `modify`, `shell` and `agents` say; `wait`, always;
  `deliver`, for a change's contract, and mid-run for a run with a
  repository it may write, as a chat delivers a small fix (jig's
  `domain/hosts.md`, section 8; smith's `domain/run.md`, 8.4), but
  never in an Open run (4.6).
- **Every host tool below,** declared in every charter, each with the
  charter's call timeout. The task's authority decides each call by its
  family (jig's `domain/authority.md`, sections 3 and 8.5): a call beyond
  it is answered beyond authority, naming what it lacked, and the run may
  `propose` it instead.

| Tool | Family | What it does | What it names, as the run saw it | What it answers |
|---|---|---|---|---|
| `delegate` | delegate | makes a batch of direct delegates, each with what a party's `Create` names (a deadline as seconds after it is made, or none, to end with its creator's) (jig's `domain/tasks.md`, section 4), tracked as a goal at a priority (`tracked`, the lower first) or untracked | | their numbers, their specs' revisions, and what a budget given `AsAllowed` was given; refused, and why |
| `change` | delegate | one change to one of the project's repositories, landed (forge.md, section 8): the run gives the words and, where the project has more than one repository it may write, which as `owner/name`; the root fills in the rest, as 4.3.1 says, and decides it as a `delegate` | | its task's number, as `delegate`'s; refused, and why: naming the project's repositories where none fits |
| `message` | message | words, a question, or the answer to one, to a task it references (jig's `domain/tasks.md`, 8.5) | the question, for an answer | sent, with the message's number; refused: no reference, the inbox full, too large for a run to read |
| `introduce` | message | gives two tasks it references a reference to each other | | done; refused, and why |
| `amend` | amend and cancel | changes a live delegate (jig's `domain/tasks.md`, section 7), naming only what changes: its tracking (a priority, or untracked), its spec, wake, dependencies (the whole list), authority (whole: a deadline is its budget's, the rest restated), instructions (whole; empty leaves none) or models; with a reason, which its live run hears unless new instructions replace it | its spec's revision, as the run last read it; a tracked delegate's priority alone needs no revision | done, with the spec's revision after the commit, the one its next amendment names, and what changed; refused, and why; moved, with what it is now |
| `release` | amend and cancel | lifts a delegate's hold; for an effect held uncertain, says whether to make it again (`again`) | its hold's revision | done, with its spec's revision, which a release leaves as it was; `again` missing for an effect held uncertain, or given for another; refused, and why; moved, with what it is now |
| `cancel` | amend and cancel | a live delegate and its tree, with a reason | | done; refused, and why |
| `decide` | decide | accepts, rejects with a reason, or passes up a proposal waiting for it (jig's `domain/tasks.md`, section 9) | the proposal, by its proposer and number | decided, and how; refused, and why |
| `decide_escalation` | decide | releases, rejects with a reason, or passes up a held descendant's escalation waiting for it | its hold's revision | decided, and how; beyond its authority, and passed up; moved, with what it is now; refused, and why |
| `withdraw` | decide | its own pending proposal | the proposal | withdrawn; already decided, and how |
| `propose` | propose | an action beyond its authority, with a reason, to the holder that may accept it: a batch, an amendment, a release, a forge effect, or a widening of its own authority (jig's `domain/authority.md`, section 9) | for an amendment, its spec's revision, as `amend` names it | proposed, as which number, and to whom; refused, when nobody may accept it; moved, for an amendment |
| `subscribe`, `unsubscribe` | subscribe | a referenced task's state, held or ended, or a timer, once or periodic (jig's `domain/tasks.md`, 8.4); removes a subscription by its number | | the subscription's number; removed; refused, and why |
| `subscribe_forge` | subscribe | a forge topic (forge.md, 7.1): landings on a branch, with the task's own change and the paths by which a landing overlaps; CI on a head; a pull request's state; the comments and reviews on a participating object | | the subscription's number; refused, and why |
| `effect_forge` | effect | one of the effects the forge offers to agents (forge.md, section 2) on an adopted repository: an issue created, a comment posted on an issue or a pull request, a commit status posted | | made or found, with what it made; committed and pending, past the call's deadline; uncertain, the connector finding it, or its task held for a person; raced; withdrawn; failed, and why; refused before it was committed; waiting for a verdict; unavailable, deciding nothing, while the forge has no room for another |
| `note` | note | writes a note in a scope, or revises one (jig's `domain/engine.md`, section 10) | for a revision, the note's revision it recalled | written, with its name and revision; refused: busy, too large, full, already there, missing, moved |
| `recall` | recall | reads notes by name, or a page of a search of their scoped descriptions | | the entries, and whether more remain |
| the forge's reads | read | one fresh read each, of an adopted repository (forge.md, section 2), below | | the answer, cut to the read's limit; failed, and why |

The forge's reads, each a host tool of its own: `read_items`, a page of
a repository's issues and pull requests; `read_item`, an issue or a pull
request with its comments; `read_pull`, a pull request; `read_pull_for`,
the pull request for a head and a base; `read_pull_files`, the files a
pull request changes; `read_reviews`, a pull request's reviews;
`read_remarks`, their remarks; `read_statuses`, a commit's statuses;
`read_checks`, a commit's combined checks; `read_job`, one CI job's log,
bounded, its failed step named before it; `read_branch`, a branch's head;
`read_branches`, the branches; `read_compare`, two commits compared;
`read_file`, a file at one head, bounded; `read_protection`, a branch's
protection; `read_settings`, the repository's settings;
`read_collaborators`, its collaborators; `read_permission`, a user's
permission on it.

- **Reads and writes:** the forge's reads and `recall` are reads; every
  other host tool is a write, which smith runs alone (smith's
  `domain/run.md`, 5.2).
- **Declared, decoded and rendered in one place,** `temper-engine-smith`:
  each tool's JSON Schema, description and effect, written once
  (`delegate`'s and `amend`'s schemas name every field of the spec and
  the authority, so that a run writes them from its tools alone, and
  both take a priority as `{priority}`, where jig's own `delegate` takes
  a bare number, so that a run learns one shape); a
  call's input decoded into the root's `Tool`, or answered at once as an
  error naming what is wrong, deciding nothing; an answer rendered as
  smith's text, marked an error or not (`../protocol/llm.md`, section 4).
- **Calls are named and decided once** (jig's `domain/engine.md`, 7.3):
  the engine answers a call it has seen before from its record, and smith
  asks again under the same name after busy or a lost answer (smith's
  `domain/run.md`, 5.2).
- **A call names what it saw** (jig's `domain/engine.md`, 7.3): the
  revisions above are those its brief, its messages and its earlier
  answers showed it. One whose object has moved since is answered moved,
  with what it is now, and the run decides again on that.
- **Not the engine's:** `wait`, `finish` and `deliver` (jig's
  `domain/engine.md`, 7.3). `deliver` is a push (section 6).
- **Jobs are smith's** (smith's `domain/tools.md`), not `read_job`'s,
  which reads a CI job's log. A `shell` command that runs past its
  yield goes on as a job in the agent's process, under the charter's
  `shell` (from `policy.shell`); the run reads or stops it with smith's
  `job`. Neither the engine nor the worker routes or answers a job.
  `temper-engine-smith`'s `turn` takes a turn request alone, and
  answers any other request, a job's included, as not a turn
  (`Problem::Type`).

#### 4.3.1 A change, in the run's words

A chat cannot know what a change's `delegate` needs: the procedure's
numbers, the repository's resource, its landing branch, the contract's
kind and the grants a change makes. `change` takes the words alone, and
the root builds the one delegate from what it knows, then decides it as
any `delegate` (jig's authority, named and decided once):

- **the repository:** among the project's adopted repositories it may
  write, on forges not ended, the one named, or with no name the only
  one; with several and no name, or a name none has, refused, naming
  them. Never the home by default: a project's home may be another
  forge's (the demo's);
- **the delegate:** the change procedure (forge.md, 8.1) with the words,
  the repository's resource and its default branch as the landing
  branch; a change's contract;
- **its authority, the run's own narrowed:** the run's grants of the
  kinds a change makes (forge.md, section 6: a push, open, merge, edit,
  update, close or reopen a pull request); the forge's charter as the
  only kind it may delegate, and a person's role where a landing rule
  covering the branch names a person gate; at most 4 tasks, one fewer
  than the run has left, and a depth of at most 2; no notes; half of
  what the run's budget has left, so that it can still read the result
  and answer, refused when that is no more than a run's minimum. That
  half is a default, open to tuning.

A `delegate` refused for an executor or a grant says, too, that a
repository change is `change`'s.

### 4.4 Result contracts

A task's result contract (jig's `domain/tasks.md`, section 3) becomes
smith's (smith's `domain/run.md`, 7.1):

| temper's | In smith |
|---|---|
| a report: words, bounded | a report, and a failure, within the same bound |
| a verdict from a closed list, each with the fields it requires and the follow-up tasks it may propose | a verdict for each choice, named by its number, with its fields and its words' bound; follow-ups as items, of the kinds of task the verdict may propose; a failure of at most 1,024 bytes |
| a connector's result: a change landed | a change, requiring the fields `title` and `body`, each within the contract's bound, its checks required; a failure within the same bound |
| an approval | a failure only, of at most 1,024 bytes: approvals are people's |

A change's result names the resource the worker's push and the forge
settled, never one from the LLM's fields, and its words are the `body`.
The core judges a result again as it commits it, against the contract
and the state as it is now (jig's `domain/tasks.md`, 5.6).

### 4.5 Conventions

Checks at `.temper/pre-pr`, guides at `AGENTS.md` (smith's
`domain/run.md`, 3.1 and 8.1).

### 4.6 Budget, models, waiting

- **Budget:** the run's turns, spend and time, with its reserve of turns
  and time for finishing (smith's `domain/run.md`, 9.1), from its charter
  in configuration (designed, not built: at the task's revision); its spend is what its task has left, capped per run,
  reserved at its claim, in the deployment's unit (jig's
  `domain/hosts.md`, section 8).
- **Models:** the main session's and those its sub-agents may use, each
  with its endpoint, window, output and prices (`../protocol/llm.md`,
  section 3): the task's LLM choice among those its charter allows,
  without one the charter's first whose LLM provider has an account
  usable for the payer (jig's `domain/tasks.md`, section 3),
  and the account the run's claim chose for each LLM provider (jig's
  `domain/accounts.md`, 4.3). Their credentials, or a program source's
  name, come to the agent as grants through the worker
  (`../protocol/credentials.md`, section 3). A run on a program has no
  sub-agents (smith's `protocol/programs.md`, section 2).
- **Waiting and resuming:** the waiting time is the charter's; a
  coordinator's is zero. A chat resumes its transcript, and a
  coordinator starts fresh from its brief (jig's `domain/engine.md`,
  7.4).
- **Its mode is its task's ending** (jig's `domain/tasks.md`, 5.8;
  `domain/hosts.md`, 8.1), the assignment's `ended_by`, whatever the
  charter: a task its result ends runs smith's `Finite` mode, and one
  its requester ends runs `Open`, granted the wait and neither `finish`
  nor `deliver`. temper's chats are Open (`../protocol/configuration.md`,
  section 2): ordinary text listens for the person's next words. Idle or
  wind-down parks the run, committing its reply with no result and no try
  spent. Close carries the grace's finish request live or in Start; its
  admission puts the Open run into wind-down, so its next text parks.
  Cancellation after the grace keeps its existing meaning; only the
  person's Close, or a cancel, ends the chat. Goals and every delegate
  run Finite and end by their results.

## 5. Messages

What a task hears while its run is live reaches smith as messages, as
jig's `domain/hosts.md`, section 9 says: each named, with its sender's
label and its words whole, the start carrying those waiting in the
task's inbox. Proposals and escalations waiting for the task's decision
are relayed the same way, each naming what `decide` or
`decide_escalation` takes.

## 6. Delivery is a push

smith's `deliver` is the worker's push (worker.md, section 5): the
worker commits exactly the tree the run checked, with the result's
`title` and `body`, and pushes it as a fast-forward, never forced, to the
branch the assignment names; a merge in progress is committed as the
merge, with both parents. Its outcomes, in smith's terms (smith's
`domain/run.md`, 8.2):

| The push | smith's delivery |
|---|---|
| landed in every repository with a change, or already landed by this hold's last push | delivered, naming each head |
| nothing to push | nothing |
| a branch moved, as a fast-forward cannot take it or its head is not the one expected | stale: the run ends, and the engine decides afresh |
| a conflicted file still holds a marker | refused, naming the file |
| refused by the forge, unreachable, timed out, broken, too large, missing the repository, a branch or a commit | failed, with the reason and the last 512 bytes of git's diagnostic output |

A push that lands is the result, even if the run was winding down. The
answer carries what the run left on the forge (worker.md, 4.2), and the
forge's change procedure goes on from it (forge.md, section 8).

## 7. Turns and transcripts

Turns are smith's bytes (smith's `domain/session.md`, section 3), which
the worker relays and the engine keeps opaque, by conversation and
window (jig's `domain/engine.md`, 7.2; jig's `domain/hosts.md`, 6.4).
temper's client decodes them as jig's client does (jig's
`client/domain/client.md`, section 10). A run that resumes is given its
transcript's current window and the calls committed after its last turn;
past the resume limit, or when smith refuses the transcript, the next
run starts fresh, its brief carrying the tail.

## 8. The agents temper runs

| Agent | Messages in | Tools its authority gives | Ends when |
|---|---|---|---|
| producing a change (forge.md, 8.2) | none | the workspace's, from the landing branch or saved work; `finish` with a change | its change passes its checks and is pushed |
| a repair (forge.md, 8.4) | none | the workspace's, from the change's branch, its brief carrying why; `finish` with a change | its new head passes its checks and is pushed |
| resolving a conflict (forge.md, 8.4) | none | the workspace's, from a merge in progress; `finish` with a change | the merge is resolved, checked and pushed |
| a review (forge.md, 8.3), a triage | none | reads; `finish` with a verdict | its verdict meets its contract |
| a chat | its person's words; its delegates' results | most; `deliver` mid-run; `wait` | its person closes it |
| a coordinator | questions, results, overlapping landings; proposals and escalations to decide | `delegate`, `amend`, `cancel`, `release`, `decide`, `decide_escalation`, `propose`, `subscribe_forge`, `note`; `wait` | its goal's report is written |

The change's agents run under the forge's charter; designed, not built,
the one the change was made with, kept on it whatever later changes of
settings make the forge's charter (`../protocol/configuration.md`,
13.2).

## 9. The world

smith's worlds test the agent itself (smith's documents, each section
"The world"). temper's test what it fills in, and the agent under its
host:

- **`temper-engine-smith`'s step tests:** the root's charter becomes
  smith's start, its brief in order and its budget exact; every declared
  tool decodes its least shape and refuses a wrong one; `amend` and
  `release` name their revision; every answer renders as smith's text
  within smith's limits, or as an error; a job's answer names its failed
  step before its log; smith's terminals keep their spend and the root's
  contract.
- **The routing's tests:** a call decoded into the root's `Tool`; one of
  another activation, or of an unknown tool, answered as an error with no
  commit; a result that does not decode failing its attempt as invalid;
  a change's result kept only with its settled resource; each settled
  delivery the hub's answer (`../protocol/llm.md`, section 6).
- **The worker's and the system worlds** (worker.md, section 10): smith's
  scripted agent for the agent child, and smith's real agent domain over
  skein's fake LLM, its tools working in the trees the worker prepared.
- **Stories that are temper's:** a change produced, checked and pushed,
  and its push found moved; a coordinator that delegates a batch, hears
  results, and proposes what is beyond its authority; a chat that waits,
  hears its person's words, parks, and resumes from its transcript; a
  host tool answered busy by the engine, asked again and decided once.

## 10. Open questions

- **Which host tools a charter offers.** jig's charter carries the tools
  its task's authority gives (jig's `domain/engine.md`, 7.1); temper's
  declares every host tool, and the authority refuses what it does not
  allow. Whether to declare only what it allows, and how a brief teaches
  an LLM when to delegate a task rather than a sub-agent.
- **A cancel's grace:** jig's `cancel` takes one, if the run gives it
  (jig's `domain/engine.md`, 7.3); temper's takes none.
- **A code graph** for a workspace (worker.md, section 12), as an MCP
  tool source once smith has one.
