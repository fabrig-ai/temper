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
  temper's own binary run as `temper internal agent`, smith's agent shell
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
  `Pull request`, `CI`, `Reviews` and, for each document the charter
  names, `Document` followed by its repository, path and heading
  (`../protocol/configuration.md`, 13.7). A section its gather could not fill
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
| `goal` | delegate | one tracked goal, in the run's words: the run gives the words, and whether it is an advisor, which changes nothing; the root fills in the rest, as 4.3.2 says, and decides it as a `delegate` | | its task's number, as `delegate`'s; refused, and why |
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
| `settings` | settings | reads the settings (jig's `domain/engine.md`, 7.3): with `{}` the inventory; with `{"setting": SETTING}` one setting's value, SETTING as a settings file names it (`../client/tools.md`, 5.3), a charter's of a held revision with `"revision"`; with `{"revision": R}` a revision's record; with `{"history": {}}` the newest page of the history, and `{"history": {"before": R}}` the page before R. Every form takes `"from"`, an offset into its answer's text. temper's kinds' values come from the forge (`../protocol/configuration.md`, 13.2) | | its text as `temper ctl` prints the same read (`../client/tools.md`, 5.2), from `from`, cut as `read_docs` cuts a page (`../protocol/documentation.md`, 3) within the answer's bound, with `next: OFFSET` or `end`; refused, and why |
| `propose_settings` | settings | proposes a change of settings to the owners who may accept it, with a reason (jig's `domain/engine.md`, 17.6): its items in a settings file's form (`../client/tools.md`, 5.3) | the newest revision it read | proposed, as which proposal and to whom; stale, naming the newest; refused, when nobody may accept it or an item is out of shape, naming it |
| `read_docs` | none | a page of one of temper's documentation topics from an offset, or the sections matching some words (`../protocol/documentation.md`, section 5); answered where the call is decoded, with no authority, commit or record | | the page and the next offset, or the end; the hits; no such topic, with the hits of a search for its words |

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

Their input is flat, each member declared by name in the read's schema,
so that a run writes a call from its tools alone:

- **`repository`,** one of the project's repositories as `owner/name`:
  among its adopted repositories of any role, on forges not ended, the
  one named, or with no name the only one; with several and no name, or
  a name none has, refused, naming them. The root resolves it to the
  repository's resource as it builds a change's delegate (4.3.1),
  before it decides the call, so that the call's authority is checked
  on that resource (`root.md`, 5.2) and the run never names a number.
- **The read's own members,** beside it: `read_branch` `branch`;
  `read_branches` `prefix`; `read_pull` `number`; `read_pull_for` `head`
  and `base`, branch names; `read_pull_files` `number`, `head` and
  `page`; `read_reviews` `number` and `page`; `read_remarks` `number`,
  `review` and `page`; `read_statuses` `commit` and `page`;
  `read_checks` `commit`; `read_compare` `before` and `after`;
  `read_file` `head`, `path` and `max_bytes`; `read_job` `head`, `run`,
  `job`, `attempt` and `max_bytes`; `read_items` `since`, `page` and
  `item_kind` (`issue` or `pull`); `read_item` `number` and `after`;
  `read_protection` `branch`; `read_settings` none;
  `read_collaborators` `page`; `read_permission` `user`, a forge user's
  id. A commit or a head is its id, 40 or 64 lowercase hexadecimal
  digits; `page` is from 1, and 1 when left out; `since` is a time in
  nanoseconds, 0 when left out; `after` a comment's id, 0 when left
  out; `max_bytes`, the read's limit when left out.

- **Reads and writes:** the forge's reads, `recall`, `settings` and `read_docs` are reads; every
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
- **An answer fits what the agent takes** (`domain/worker.md`, 4.3):
  a call past the worker's input bound, `core.call_answer_bytes`, is
  answered `call too large` under its name, and the run goes on (jig's
  worker link). An answer past the agent's `host_reply_bytes` (smith's,
  its channel's `host_reply_text`, the agent's `tool_payload`) reaches
  the run as smith's `TooLarge`, which says the answer was too large
  and not shown, and the run goes on. The engine keeps the answer it
  decided, so a call asked again gets the same reply.
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
  forge's reads (kind 1, forge.md, section 2) and of the kinds a change
  makes (forge.md, section 6: a push, open, merge, edit, update, close
  or reopen a pull request), on the change's repository, which every
  agent of the change holds, so that its reviewer and its repairs read
  the pull request, its files and CI's logs; the forge's charter as the
  only kind it may delegate, and a person's role where a landing rule
  covering the branch names a person gate; at most 4 tasks, one fewer
  than the run has left, and a depth of at most 2; no notes; half of
  what the run's budget has left, so that it can still read the result
  and answer, refused when that is no more than a run's minimum. That
  half is a default, open to tuning.

A `delegate` refused for an executor or a grant says, too, that a
repository change is `change`'s.

#### 4.3.2 A goal, in the run's words

A chat cannot know what a goal's `delegate` needs either: its own
authority's tool families, the grants' vocabulary on the forge's
resources, the delegation's kinds and its funding, none of which a run
sees. `goal` takes the words alone, and whether the goal is an advisor,
and the root builds the one delegate from the run's own, then decides
it as any `delegate`:

- **the delegate:** under the run's charter (a chat's, charter 1),
  whose instructions stay its instructions, the words its brief; tracked
  at the run's own priority, or last among the project's goals where the
  run is untracked: one past the largest tracked priority in the
  project, saturating, or 0 with none (the lower first, jig's
  `domain/tasks.md`, section 3), with a report's contract bounded by
  the deployment's largest result, jig's `tasks.result_bytes` (512 on
  the small profile a box runs), whatever the run's own contract's
  bound (a chat's is a line), since jig refuses a batch whose contract
  is above it (4.4): the work a person asked for, which plans, asks its
  requester with `message` where its words say to (8.2), calls `change`
  for each change, and reports. The contract bounds the result, not
  the work: asking first is the instructions', and the rail is
  authority's;
- **its authority, the run's own narrowed,** never wider in any part
  (jig's `domain/authority.md`, sections 3 and 5): the run's models and
  deadline; its tool families (delegation, messages and reads, so that
  it may call `change`, ask and read the forge); its grants of the kinds
  a change makes and its reads, so that `change` can narrow them again
  (4.3.1); its delegation kinds, with the tasks its subtree may still
  make, as jig counts them, less one for the goal itself, and a depth
  one less, refused when either is spent; no notes; half of what its
  budget has left, refused when that is no more than a run's minimum, as
  `change`'s. A change the goal then makes has half of the goal's,
  so at most a quarter of the chat's: defaults, open to tuning;
- **an advisor,** with `advisor` set, its report bounded as a goal's:
  untracked, so that no goal of its
  is projected on the forge; reads alone, no grant of a forge effect, no
  delegation (no kinds, tasks or depth) and no notes, so that it reads
  and reports and can change nothing beyond its own workspace, which it
  has no grant to push (the retired first edition's tester,
  testing.md, 5.3: a rail, not an instruction).

The root builds it where it builds `change`'s delegate, in the engine's
route of host tools, and decides it as any `delegate`; the smith
translation only decodes the call.

The goal's own run takes `goal` too: an advisor it asks for, before it
asks its requester, is how the retired first edition's survey before a plan
goes (testing.md, 5.3).

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
| a chat | its person's words; its delegates' results and questions, and its person's answers to those questions (8.2) | most; `deliver` mid-run; `message`; `wait` | its person closes it |
| a coordinator | questions, results, overlapping landings, the answers to its own questions (8.2); proposals and escalations to decide | `delegate`, `amend`, `cancel`, `release`, `decide`, `decide_escalation`, `propose`, `subscribe_forge`, `note`, `message`; `wait` | its goal's report is written |
| the configuring agent (8.1) | its person's words; its proposals' outcomes: accepted, rejected with a reason, withdrawn as stale | `settings`, `propose_settings`, `withdraw`, the forge's reads, `read_docs`; `wait` | its person closes it |

The change's agents run under the forge's charters, the ones the change
was made with, kept on it whatever later changes of settings make them
(`../protocol/configuration.md`, 13.2): its producing, repairs and
resolutions under the forge's charter, a coding agent's; its reviews
under the forge's review charter, a reviewer's, whose instructions are
to judge the change through its brief and the forge's reads and give a
verdict, or fail saying why it cannot (forge.md, 8.3), and whose tools
are the workspace's inspection and the forge's reads, never
modification or the shell: a reviewer changes nothing.

The change's agents ask no one, and take no questions, answers or words
(8.2).

### 8.1 The configuring agent

A chat from template 3, run by charter 3, both stock and both settings
like any other (`../protocol/configuration.md`, 13.4), which turns how
a person says they work into a proposal of settings
(`../draft/configuring.md`, sections 2, 5 and 6).

- **Its instructions** are held in the binary, in the `stock` module,
  and say, in the person's register:
  - the test for each sentence of a description: would temper's engine
    act differently without it? What changes what temper wakes,
    schedules, permits, pays, lands or holds is structure, and becomes
    settings; what only changes how an agent behaves is a rule, and
    stays text that runs read, in a charter's instructions;
  - to read the settings first, and the documents the person points to
    (a path in an adopted repository, with `read_file`; a pasted text);
  - to ask until it can propose, one question at a time, and to offer a
    rule as structure where temper's model has it, saying what it
    guarantees and what it does not, as the draft's 4-hour cap does;
  - to propose once, against the newest revision it read, with a reason
    whose first line is the proposal's title and whose next lines say,
    under "temper enforces", each change of structure in the person's
    words, under "your agents follow", each rule and where it comes
    from, and what the change does to running work;
  - to say what temper cannot model, offering the nearest thing, and
    what needs a restart (the document's members: a provider, a forge,
    a listener, a worker, a limit), never proposing it;
  - to explain from the settings and the documentation (`read_docs`),
    naming its pages, never their sections or offsets: the concepts
    `settings`, `revision` (moving running work and restoring a version
    among it), `proposal`, `charter`, `template`, `policy` and
    `landing-rule`, each with what it cannot do and what needs a
    restart; `settings-fields`, each setting's fields, bounds and
    defaults; `configuration`, the document's members; and the recipes.
    What happens when work starts, which role runs where, why a task's
    run had what it had, given the task's `settings:` line the person
    reads to it;
  - on its proposal's outcome, heard in its inbox (jig's
    `domain/engine.md`, 17.6): accepted, to say so in one line with the
    revision it made; rejected, to take the
    reason as the person's words and revise; withdrawn as stale, to
    read the settings again and propose afresh, saying what changed.
- **It changes nothing itself.** Its charter's `settings` family reads
  and proposes; only an owner's decision changes settings (jig's
  `domain/engine.md`, 17.6). A member's configuring chat proposes as
  an owner's does, and its proposal waits for an owner.
- **Started from ctl** with `chat start --project PROJECT --template 3
  TEXT` (`../client/tools.md`, 5.3), and, once the user's pages land,
  from "Tell temper how you work".
- **Its tests** are `testing.md`, 6.4's: scripted runs in the engine's
  domain world for its flows, and one process story through ctl.

### 8.2 Asking before the work

A person who wants to agree a plan before anything is changed: the
reference case's question to a requester and to the chat
(`../draft/reference-case.md`, 4.2 and 7.2),
and rung B0 of `../draft/v0.1.0.md`. Where the second edition keeps a
question to the chat with the chat, which answers it or asks its person
itself (`../draft/process-model.md`, J7 and J8), the route below is
today's, until those land.

- **The work that waits asks; a change never does.** A change is
  decided work, asked for once what to change is agreed. Its agents,
  the table's first four rows, ask no one: the change procedure
  delegates each with the Message family taken out of the tool families
  it passes on, which a child's authority otherwise keeps (jig's
  `domain/authority.md`, section 3). The change itself takes no
  questions, answers or words (forge.md, 8.1); its amendments and its
  cancel reach it as forge.md says.
- **The chat delegates the work as a goal** when its person asks to
  agree it first, with `goal` (4.3.2) and the person's words: a
  delegate under charter 1 holding the authority to call `change` and
  to ask; never `change` itself, which would start the work at once. The stock
  chat's instructions (charter 1, `../protocol/configuration.md`, 13.4)
  carry the rule: work the person wants to agree before anything is
  changed goes to such a delegate, which tells its plan, asks, and calls
  `change` only after the answer; the chat relays the person's answer to
  it.
- **Designed, not built (J7, J8):** the goal's question stays with the
  chat (jig's `domain/tasks.md`, 8.5): the chat, woken by it, answers
  from what it knows (its person's priorities, its notes), or asks its
  person a question of its own in its own words, which is an entry of
  the person's inbox, and answers the goal once it has the person's
  answer. The rest of this section is today's route, which this
  replaces once jig's rule is built.
- **The goal asks its requester,** the chat, with `message` in the
  question form, before it calls `change`, and waits. jig puts the
  question in the inbox of the chat's person, since the person requested
  the chat (jig's `domain/tasks.md`, 8.1; jig's `domain/people.md`,
  section 6). The chat cannot ask its own person: it says in its reply
  that a question waits for them.
- **The person answers in the inbox,** `temper ctl task answer TASK
  QUESTION TEXT` (`../client/tools.md`, 5.3) or the inbox's page. The
  chat, woken by the answer, relays it to the goal as the answer to the
  question's number, and says so. The goal then calls `change` with what
  was agreed, waits for its result and reports; the chat says what
  landed.
- **Nothing is changed before the answer:** no change task, branch or
  pull request exists until the goal calls `change`.
- **Its stories:** `fuzzy_green_light` on fakes (testing.md, 2.4), and
  rung B0 walked on real backends (`../draft/v0.1.0.md`, 2.1).

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
