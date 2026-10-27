# Landings

Provisional, 2026-10-27. How a project that lands by turn lands its
changes: changes joined into landings across repositories and goals,
candidates prepared by the project's tooling or by temper's default,
gates proved by receipts kept by key, and the project's landing turn,
which fast-forwards each main to its proved candidate. It replaces, for
such a project, the change's CI, its queue, its updates and its merge
in the repository's style (`forge.md`, 8.2 steps 3 and 5 to 7,
sections 9 and 10). What follows a landing, its tiers and its verdict,
is `after-landing.md`. The forge's calls it makes are skein's
`forge.md`, 3.2. What is still open is section 17.

## 1. In one page

- **A project lands by queue or by turn,** its own setting (section
  13). `queue` is `forge.md`'s change procedure, sections 8 to 10, and
  stays the default until a project's turn has been walked on real
  backends. `turn` is this document. A change follows the way its
  project names when it becomes ready.
- **A landing** is one or more changes that land together, a landing
  of one included: at most one stack per repository, from any goals
  (section 4). It is a procedure task of the forge's, made when its
  first member is ready, requested by the deployment on the project's
  behalf. Its members' change tasks wait on it, and their goals hear
  it as news.
- **Prepare** makes each repository's candidate: main's tip with the
  members on top (section 5). temper's default asks the forge to rebase
  each member's pull request onto its main, which puts the candidate on
  the member's own branch. A project's own tooling, run on a worker with
  no model and no forge credentials, does more: pins, regenerated
  files, conflicts it knows how to settle.
- **Receipts by key** (section 6). A key is what a gate's result
  depends on: the candidate's tree without the paths the gate ignores,
  with the gate's definition. A receipt is a gate's result under a key,
  kept per project and reused by every member, landing and worker. A
  gate whose key has a receipt never runs again.
- **Proof by dispatch** (section 7). Gate workflows run only when
  temper dispatches them, for a key with no receipt, at the candidate.
  A real failure is a repair under the failing member's change; a
  runner's fault is retried; a key that changed nothing a gate reads is
  proved without a run.
- **The turn** (section 8): one per project, held for the seconds its
  merges take. It checks each member's main is still its candidate's
  parent, then fast-forwards each main to its candidate, upstream
  first. A stale base sends the landing back to prepare, keeping what
  still holds. No gate runs inside the turn, and main is never gated
  again: its commit is the gated one.
- **First proved, first landed** (section 9), and a landing whose proof
  went stale twice reserves the next turn for its repositories.
- **The forge shows it** (section 10): each candidate on its member's
  pull request, each gate's receipt as a commit status,
  `temper/gate/<name>`, which a fast-forward carries onto main.

## 2. Structure

```
temper-engine-domain-forge              the top: landings' and receipts' states, the turn, routing; the effects
├── temper-engine-domain-forge-change   the change procedure: under turn, up to ready, its repairs and resolutions
└── temper-engine-domain-forge-landing  the landing: decisions over its state and the facts; keeps nothing
```

- **The top keeps** each landing's state (family `Landing`), the
  receipts (family `Receipt`), the keys in flight, and who holds each
  project's turn, as it keeps the changes' states (`forge.md`, section
  3). The turn is derived, never stored: the landing whose phase is
  `turn` holds it.
- **`landing` is policy,** as `change` is: given a landing's state, its
  members' states, the receipts and the facts the top read, it says
  what is due (a prepare, a key to compute, a dispatch, a read, a
  repair, the turn, a merge) and the landing's next state.
- **Keys are computed in the domain** from what the forge answers of a
  tree (skein's `Read::Tree`), or taken from a prepare's report: the
  engine never reads a repository (`worker.md`, section 2).
- **The worker** runs a project's prepare (section 5.3) as a job, never
  an agent's run.

## 3. A change under the turn

```
producing ─► opening ─► ready ─► preparing ─► proving ─► turn ─► landed
                          ▲          │            │
                          │      resolving    repairing
                          └──────────┴────────────┘
```

- **Producing and opening** are `forge.md`'s 8.2, steps 1 and 2. No CI
  runs on a pushed head: the project's gate workflows are dispatch-only
  (section 7), and the change has no checking step.
- **Ready:** its head pushed and its pull request open; every blocking
  judgement gate the landing rules and the change ask for (a review, a
  person's approval: `forge.md`, 8.3) holds at its head; and its task
  is not kept by a dependency. A review runs as soon as the head is
  pushed.
- **Then its landing's phases:** preparing, proving and the turn are
  the landing's (sections 5 to 8). The change steps on its landing's
  news, and makes what its landing asks of it on its own branch, under
  its own hold: the forge's rebase (5.2), or a prepare job delegated
  (5.3). The turn's merge of its pull request is the landing's (8).
- **Repairing** (`forge.md`, 8.4): a gate's real failure at the
  candidate, given its failing output. Its new head makes the change
  ready again, and the landing prepares again.
- **Resolving** (`forge.md`, 8.4): a conflict prepare could not settle,
  in a workspace where main's tip is merged into the change's branch
  with the files in conflict marked. Its pushed merge contains main's
  tip, so the next prepare takes the head as it is (5.2).
- **Landed** when its member's main is its candidate. Its result is the
  landing: the candidate, which is main's commit, and its pull request.
  It then verifies, and ends at cleared (`after-landing.md`, section 6).
- **Judgement gates carry over clean prepares,** where their freshness
  is `clean` (`forge.md`, 8.3): a verdict given at a head counts at the
  candidates temper's prepare made from it without settling a conflict,
  as `clean` counts clean updates, since the member's diff is unchanged.
  An `exact` verdict counts at its head alone, so a candidate that is
  not that head asks for it again. A candidate whose prepare settled a
  conflict, a resolution, or a repair asks for every judgement gate
  again. The change keeps the heads its clean prepares led through,
  which the landing rules read (`forge.md`, 8.7).

## 4. A landing

### 4.1 Its members

- **Changes, at most one stack per repository.** Changes into one
  repository in the same landing are applied in their dependency order,
  the last one's branch carrying the stack.
- **From any goals.** Each member stays its own goal's: its repairs and
  resolutions are its change's delegates, under that goal.
- **How changes join one:**
  - **`with`:** a change names the changes it must land with, a
    parameter of its own (`forge.md`, 8.1), when it is made or by an
    amendment of a task that may name the other one (jig's
    `domain/tasks.md`, section 7's rule for dependencies). `with` is
    symmetric and transitive: the connector joins the closure into one
    landing, and a change is ready to land only when every change of its
    closure is.
  - **A dependency across repositories** joins its two changes, unless
    the project's setting says such dependencies only order them.
    Joined, it orders their preparation and their merges, upstream
    first, and never keeps the dependent from its landing: the merges
    are the landing's, which has no dependency (section 8). The
    dependent's result still waits for its dependency's, as jig's
    dependencies order results (jig's `domain/tasks.md`, 5.7).
  - **Pin-only members:** where the project keeps downstream pins at
    their upstream's main, a landing with a member upstream gains a
    member in each downstream repository the project's pins name that
    has none of its own, whose only change is the bumped pin, on a
    branch the landing makes and holds, `temper/<root>/l<task>`, with a
    pull request of its own that the landing opens (5.3; `forge.md`,
    11). It has no change task: its prepare job is the landing's
    delegate, its gates are its repository's test gates, with no
    judgement gate, its merge is the landing's, and its branch is
    deleted as the landing closes.
- **Ready** when every member is ready (section 3). A member that goes
  back to repairing or resolving takes its landing back to preparing
  once it is ready again; the other members keep what holds.

### 4.2 The landing task

- **A procedure task of the forge's,** `landing`, made in the decision
  in which its first member becomes ready, with that member and every
  change joined to it. Members joined later are added by the connector
  in the decision that joins them.
- **Requested by the deployment on the project's behalf,** funded from
  the project's spend, with the authority the project's policy gives
  landings (jig's `domain/authority.md`, section 6), as a branch's
  repair under `queue` is (`forge.md`, section 9). It answers to no one:
  its escalations go to the project's escalation role, and its members'
  goals hear of it as news.
- **Its members' change tasks wait on it:** each steps only on its
  landing's news, while its landing lives.
- **It ends** done once every member has landed, its result naming each
  repository's landed commit; cancelled once every member is cancelled.
  A member cancelled while its landing lives leaves it, and the landing
  prepares again without it, unless a member it is joined to with
  `with` is still live, which holds the landing for its goals to decide.
- **Held** for a fault past its bound (7.4), or a member's bound of
  repairs or resolutions past, saying which; its members' goals hear
  it. A release lifts it, and it prepares again.
- **Drained** (jig's `domain/connectors.md`, 6.7): a landing in a
  draining hold's scope that has begun its proof goes on, through its
  wait for the turn and the turn, to land; one still preparing is idle,
  and held. A repair, a stale base or a fault that would send it back
  to prepare makes it idle there.

## 5. Prepare

### 5.1 What it is given and what it makes

- **Given:** the members (repository, branch, head), each repository's
  main and its tip, and the project's graph of pins between
  repositories.
- **Makes:** a candidate commit per repository, at most one, whose
  history contains that main's tip, so that a fast-forward lands it.
- **Says,** per repository, the candidate, or the conflict it could not
  settle with its files; and per gate, the candidate's key where the
  project's key command is part of its prepare (6.2).
- **Prepares again** when a member's head moves (a repair, a
  resolution), when a member joins or leaves, and when the turn finds a
  base stale. A repository none of whose members and none of whose
  upstreams moved keeps its candidate and its receipts.

### 5.2 temper's default

For a project with no prepare command, one member per repository:

- **Read afresh** the member's head and its main's tip.
- **A head that contains main's tip** is the candidate as it is.
- **Otherwise the forge rebases it:** the pull request updated from its
  base by a rebase (skein's `Write::Update { style: Rebase }`), which
  moves the member's branch to main's tip with the member's commits on
  top. Once the head, read afresh, contains the tip it asked for, the
  head is the candidate. The branch is the change's, held by it, and
  the update is made in its name, so no run of temper's writes it
  meanwhile (jig's `domain/tasks.md`, 6.3); a head moved by another
  identity is drift, as ever (`forge.md`, 8.5).
- **A conflict** makes the forge refuse the rebase: the member resolves
  it (section 3), and its merged head is then the candidate as it is.
  main keeps the resolution's merge commit.
- **A forced update, and only here:** the rebase rewrites the member's
  branch, so the heads before it are kept in the change's state, and
  the pull request's timeline shows each candidate (`worker.md`,
  section 5's rule that no push is forced is temper's runs'; the
  forge's rebase is the prepare's).
- **Not drift.** The update is asked while the change's writer slot is
  its own (jig's `domain/tasks.md`, 6.3), after a fresh read finds the
  head temper last knew; the head it leads to is the change's, though
  it does not contain the one before, once it contains main's tip it
  asked for. This is the one move `forge.md`, 8.5 takes as the change's
  that is not forward. Another hand's push between that read and the
  rebase is carried into the candidate, as a merge update carries it
  under `queue` (skein's `forge-client.md`, 7.3: the forge's update
  checks no head); a head moved by another identity at any other time
  is drift.

### 5.3 The project's tooling, and stacks

- **Per repository, upstream first.** A landing whose project names a
  prepare command, or with a stack of several members in a repository,
  prepares each repository by a job (`worker.md`, 4.4), in the order of
  the project's pins, upstream first: a downstream's job starts once
  its upstreams' candidates are known, and is given them.
- **Delegated by the task that holds the branch it writes,** so the job
  writes under its holder's hold (jig's `domain/tasks.md`, 6.1): the
  last member's change, for a repository with members, asked by its
  landing as it asks for the forge's rebase (5.2); the landing itself,
  for a pin-only member's branch, which the landing makes and holds
  (`forge.md`, 11).
- **The job runs the project's command,** at the home's pinned
  revision, with no model and no forge credentials, or, with none
  named, temper's own rebase of the stack in dependency order. It may
  rebase or merge, bump a downstream's pin to its upstream's candidate,
  regenerate derived files and settle conflicts its rules know.
- **Its result** is the candidate, pushed by the worker to the branch
  with a lease on the head temper last read (`worker.md`, 4.4), or the
  conflict: the member, its files, and the main's tip, kept in the
  landing's state. The conflict makes a resolution under that member
  (section 3), whose workspace is main's tip merged into the member's
  head with those files marked, made afresh from the forge, never the
  job's discarded one; its pushed merge contains main's tip, and the
  next prepare takes it. And each gate's key when the project's key
  command is named (6.1).
- **A job's own failure** (a command that fails, times out or writes
  what the contract refuses) is the project's, not a member's: it is
  tried again within the job's tries, then holds the landing, saying
  so, and its members' goals hear it.
- **Its contract** (the arguments, the files, their formats, the
  environment, the exit codes) is `../protocol/tooling.md`.

## 6. Keys and receipts

### 6.1 A key

- **What a gate's result depends on,** as 32 bytes: SHA-256 of a
  canonical encoding of version 1, the gate (its repository and name),
  the gate's definition (its setting's value, section 13: its workflow,
  its ignored paths, whether it blocks), and the candidate's tree
  filtered by the gate's ignored paths.
- **The filtered tree:** the entries of the candidate's root tree, in
  path order, where an entry is listed whole unless an ignored path
  lies at or below it: an ignored entry is left out, and a directory
  above one is replaced by its own entries, recursively. An ignored
  path is a path prefix of whole segments (`docs`,
  `crates/x/README.md`), never a pattern.
- **The encoding,** each number big-endian: the version, one byte; the
  repository's forge and id, eight bytes each; the gate's name and its
  definition's bytes, each after its length in two bytes; then each
  entry of the filtered tree: its path's length in two bytes and its
  path from the root, its mode as four bytes, and its object id as 32
  bytes, as skein's vocabulary carries every id, a SHA-1 one padded.
- **Read from the forge:** the top reads the trees it needs (the root,
  and each directory above an ignored path) with skein's `Read::Tree`,
  and the landing computes the key.
- **A project's `key` command** gives its own instead: in its prepare
  job's result when the project names a prepare command too, and
  otherwise by a key job (`worker.md`, 4.4) per repository and gate at
  the candidate, delegated by the landing; the engine reads only the
  job's result.
- **A judgement gate's key** is not a tree: it is the gate and the head
  its verdict was given at, carried over clean prepares (section 3).

### 6.2 A receipt

- **A gate's result under a key:** passed or failed, the run (the
  forge's run, its number and its link), the candidate it ran at, and
  when it ended.
- **Made only from a run temper dispatched** for that key and saw end
  at exactly that candidate. A status someone else wrote, or a run
  temper did not dispatch, is never a receipt.
- **Kept per project,** in the family `Receipt` (`../protocol/store.md`,
  section 2), and reused across members, landings, re-prepares and
  workers. A gate keeps its newest receipts up to `receipts_per_gate`,
  oldest first out, but never the receipt of the key at its
  repository's main tip, which stays while it is main's.
- **A baseline at main's tip.** When a repository first lands by turn,
  or a gate is added or changed, and the key at main's tip has no
  receipt, the gate is dispatched once at main's tip, on a branch of
  temper's own made at that commit (`after-landing.md`, section 2), so
  that a landing whose keys equal main's (a documentation landing) has a
  receipt to land on.
- **A failed receipt** makes a key that fails at once, with its run's
  output, until the tree changes. A failure whose tests all match open
  flake findings is rerun once first (`findings.md`, section 5).

## 7. Proof

### 7.1 Dispatch

- **Each gate workflow runs only when dispatched** (skein's
  `Write::Dispatch`), never on a push or a pull request event:
  adoption says when a gate's workflow names another trigger (13.2).
- **For each member and each test gate,** at its candidate: a key with
  a passed receipt holds; one with a failed receipt fails (7.3); one in
  flight for another landing is waited for; otherwise the landing
  dispatches the gate's workflow on the member's branch, whose head is
  the candidate, with the inputs `temper_commit` (the candidate) and
  `temper_key` (the key in hexadecimal).
- **The run is found and followed:** among the runs of that workflow
  at the candidate (skein's `Read::Runs`), one made by a dispatch,
  triggered by temper's own user on that forge, after the dispatch's
  start; then read as it goes (`Read::Run`), on the job deliveries'
  hints and the keep's polling (`../protocol/forge.md`, section 5). A
  run another user triggered is never taken. Two that match are two of
  temper's own dispatches of one key at one commit, either of which
  proves it: the first to complete makes the receipt. A run whose
  commit is not the candidate is a fault (7.4): the branch moved under
  it.
- **Its status:** `temper/gate/<name>` on the candidate, pending once
  dispatched, then the receipt's verdict (section 10).

### 7.2 Proved

- **When every member's blocking gates hold at exactly its candidate,**
  test gates by receipt and judgement gates by their verdicts. An
  advisory gate is dispatched, read and reported, and holds nothing.
- **A landing whose keys all have passed receipts** (documentation,
  when every gate ignores it) is proved with no run.
- **A repository with no test gate** is proved by its judgement gates
  alone, and its adoption says so.
- **Proofs run side by side:** any number of landings prove at once,
  and a key in flight is dispatched once for all of them.

### 7.3 A real failure

- **A run whose workflow's own step failed** makes a failed receipt,
  and the failing member's change is repaired (`forge.md`, 8.4), its
  brief carrying each failed job's failed step and the end of its log
  (skein's `Read::Job`). The landing waits for the member to be ready
  again, then prepares again, and only the keys that changed run.

### 7.4 A fault of the runner

- **The runner's, not the change's:** a job whose set-up step failed, or
  whose completion step alone failed, or that failed with no step
  failed; a run cancelled by no one of temper's; a run never found, or
  found at another commit; a dispatch refused for a missing workflow or
  runner. Never read from a log's words (`../draft/ci-on-forgejo-17.md`,
  2.1).
- **Retried, never repaired:** the failed jobs rerun (skein's
  `Write::Rerun`), or the gate dispatched again, up to the project's
  `gate_retries` per key. No receipt is made.
- **Past its bound** the landing is held, saying which gate and why, and
  its members' goals hear it.

## 8. The turn

- **One per project, held for seconds.** At most one landing of a
  project is in its turn. It is given, in the decision that frees it or
  that proves a landing, to the proved landing section 9 puts first.
- **First, the bases.** Each member's main is read afresh. One whose
  tip is not the tip its candidate was prepared on makes the proof
  stale: the landing goes back to preparing and frees the turn; its
  members whose repositories did not move, and none of whose upstreams
  did, keep their candidates and receipts.
- **Then each member lands, upstream first,** by the forge's merge of
  its pull request, fast-forward only, at exactly its candidate
  (skein's `Write::Merge { how: FastForwardOnly }`), made by the
  landing task and judged by the landing rules (`forge.md`, 8.7, where
  `ci` is the test gates by receipt at the candidate), a pin-only
  member's pull request as any other. Each merge's outcome is read
  back: main's tip is the candidate, and its member is landed.
- **Nothing runs inside the turn:** no gate, no prepare, no run of an
  agent's. A merge refused or uncertain is settled as `forge.md`,
  section 6 settles any merge.
- **Partial:** if the turn
  ends with some members landed (a forge's error, a main moved by
  someone outside temper), the landed members stay landed, each
  repository's landed commit kept in the landing's state; a downstream
  not landed still pins its upstream's previous commit, a consistent
  state. The rest prepares and proves again, each landed upstream given
  as its main and its candidate at once, its landed commit, so that a
  downstream's pin bump points at what landed; then takes the turn
  again for the repositories not landed.
- **Never a rebase or squash merge** where others pin the repository:
  the pinned commit must stay on its main.
- **No turn without a fast-forward:** a project whose forge cannot
  merge fast-forward only (GitHub, until its gap B2 is designed) cannot
  choose `turn`, and the setting is refused saying so.

## 9. Fairness

- **First proved, first landed:** the turn goes to the landing whose
  proof completed first. Priorities order the dispatch of gates when
  runners are scarce, never the turn.
- **The reservation** (designed; built with conflicts): a landing whose
  proof went stale `stale_proofs` times (2 by default) reserves the
  project's next turn for its repositories. While it proves again,
  another landing touching them may prove but not take the turn. The
  reservation lapses after one more proof, or when the landing fails.

## 10. What the forge shows

- **Each candidate on its member's pull request:** the rebased head on
  the member's branch, the pull request's timeline showing each.
- **Each gate as a commit status on the candidate,**
  `temper/gate/<name>`: pending with its key's first 16 hexadecimal
  digits once dispatched; success or failure with the run as its link
  once its receipt is made; success with the receipt's run as its link
  when a receipt held with no run. A fast-forward makes main's commit
  the candidate, so main shows the statuses that proved it, and
  protection may require them.
- **The landed pull request is marked merged** by the merge itself.

## 11. Effects and kinds

Two kinds join `forge.md`'s section 6, each a row of its own:

| Kind | Effect | What it is | Recovery class | Found again after an uncertain failure |
|---|---|---|---|---|
| 16 | dispatch a workflow | a creation: a run of a workflow at a branch, with inputs | unrecoverable: the forge keeps no key for it | among the runs of its workflow at its commit made after the entry's start |
| 17 | rerun a job | a transition: the job's next attempt | unrecoverable: the forge checks no attempt | the job's attempt moved past the one named |

- **Two change their parameters:** the merge (kind 4) carries how it
  merges, the repository's style or fast-forward only; the update
  (kind 11), merge or rebase.
- **The landing's own effects** are its dispatches, reruns, statuses and
  merges (kinds 16, 17, 6 and 4), on the repositories of its members,
  and, for a pin-only member, its branch's creation and deletion and
  its pull request's opening and closing (kinds 9, 15, 3 and 12); its
  members' rebases are theirs (kind 11, which a change holds already),
  under their holds.
- **The grant:** the project's policy gives the landing procedure kinds
  3, 4, 6, 9, 12, 15, 16 and 17 on the project's repositories, as it
  gives a queue's repair its kinds (`forge.md`, section 9); temper's
  root grants them to each landing task it makes.
- **A purpose of its own,** tag 6 in `forge.md`'s table: a landing's
  step, its task (8 bytes), the member (2), the gate (2), the
  operation (1: a dispatch, a rerun, a pending status, a verdict's
  status), the candidate's preparation, counted from 1 in the landing
  (2), and the try within it (2), 18 bytes with the tag. A baseline's
  dispatch is its gate's, tag 7: the repository (16), the gate (2) and
  the first eight bytes of the key (8), 27 bytes with the tag.

## 12. In the store

- **`Landing`,** keyed by its task: its project, phase, members (each
  repository, change task or none for a pin-only member, pull request,
  the head prepared from, the main tip, the candidate, and each gate's
  key and state), its jobs in flight, a conflict (member, files, main's
  tip), each repository's landed commit once landed, when its proof
  completed, its stale proofs and its faults per key.
- **`Receipt`,** keyed by the project, the repository, the gate and the
  key: the verdict, the run, the candidate and the time.
- **A change's state** grows its landing, its clean prepares' heads and
  its way (`queue` or `turn`).
- New families at the next bytes, in place until the first release
  (`../protocol/store.md`, section 2), through the baseline's goldens
  (`../draft/upgrades.md`).
- **Bounded** by the forge's limits (`../protocol/configuration.md`,
  section 7), designed, not built: `landings`, live landings per
  deployment; `landing_members`, members per landing; `gates`, test
  gates per repository; `receipts_per_gate`; `ignored_paths`, per gate,
  each within a branch name's bounds; and `inputs`, within skein's
  `INPUTS`. A landing that would pass one is refused at its making, or
  a member at its joining, naming the limit; its change stays ready and
  waits for room, as a change waits for a hold. Each enters the
  engine's worst case.

## 13. Settings

### 13.1 Kinds

Two kinds of temper's join `../protocol/configuration.md`, 13.2, each
the forge's, judged in the change's decision:

| Kind | Scope | Numbers | Value |
|---|---|---|---|
| 4, a project's landing | each project | 1 | `by`: `queue` or `turn`; `gate_retries` (2); and, as their parts are built, the prepare and key commands, the home's pinned revision, whether a dependency across repositories joins a landing (yes), whether downstream pins follow their upstream's main (no), `stale_proofs` (2) |
| 5, a repository's gates | each project | the repository's adoption, unique in the deployment (its forge's number shifted 16 bits left, with the forge's repository number below) | the repository (its forge and id); its test gates, each a name, the workflow's file, its ignored paths, and whether it blocks |

- **Until both kinds are built** on jig's application settings (jig's
  `domain/engine.md`, 17.5), a project's `by` and its repositories'
  gates are configured in the document's `landing` member
  (`../protocol/configuration.md`, section 3), as the landing rules
  are, and read at the start.
- **`turn` is refused** for a project with a repository on a forge that
  cannot merge fast-forward only, saying which (section 8).
- **A gate's name** is a branch-name-safe word, unique in its
  repository, and its status context's last segment.

### 13.2 What adoption says

- **Under `turn`,** for each repository: its gates, each workflow
  checked to exist at the landing branch's tip and to name
  `workflow_dispatch` alone among its triggers, or said otherwise in
  words; and, where landings stack, whether the forge marks a pull
  request merged once main holds its head (Forgejo's "autodetect manual
  merge").

## 14. Views and acts

Under `../client/tools.md`, 5.1's names (a group `landing`):

| Tool | What it answers or does |
|---|---|
| `landing show LANDING` | a landing: its members, each candidate with its base, each gate's key, receipt or run, the proof's state, the turn, its stale proofs and faults |
| `landing list --project PROJECT [--ended]` | the project's landings, newest first, with their phases; the turn's holder first |
| `repository receipts REPOSITORY [--gate GATE]` | a repository's receipts, newest first: gate, key, verdict, run, candidate, time |

- **A change's panel** (`forge.md`, 8.8) names its landing and its way.
- **Acts** are jig's on the landing task: release and cancel, by the
  project's maintainers.

## 15. Restart

- **Restored whole** with the forge's other families (`forge.md`,
  section 16).
- **Before any landing steps,** each member's main and head are read
  afresh, and each run in flight is read (`Read::Run`); a dispatch
  whose outcome was uncertain is found by its runs, as section 11 says.
- **A landing in its turn** settles its merges as `forge.md`'s merges
  settle; its members found landed are landed, and the rest prepare
  again if their bases moved.

## 16. The world

Stories in the forge's world (`forge.md`, section 18; `testing.md`,
4.2), the fake forge running dispatched workflows (skein's
`fake-forge.md`), then at the process tier on a real Forgejo with its
runner (`testing.md`, 2.7):

- **a landing of one** is prepared by the forge's rebase, proved by one
  dispatched run, and fast-forwarded; main carries its gate's status;
- **a documentation change** whose every gate ignores its paths lands
  on its receipts, with no run;
- **a tree is never gated twice:** a second landing reaching a key
  already received runs nothing; a re-prepare after main moved in
  another repository runs only that repository's keys;
- **a real failure** makes a repair, and only its changed keys run;
- **a runner's fault** is rerun, never repaired, and past its bound
  holds the landing;
- **a stale base at the turn** sends the landing back to prepare, and
  the turn to the next proved landing;
- **a conflict** is resolved by a merge of main into the member, which
  lands as it is;
- **a restart** mid-proof finds its run, and mid-turn settles its merge;
- **under `queue`,** every story of `forge.md`'s world passes unchanged.

## 17. Open questions

- **Who makes the landing task:** temper, on the first ready member
  (the default here), or a goal that asks for it ("land these
  together").
- **Reviews in a landing:** before prepare (the default here), or on
  the candidate.
- **Prepare's rebase or merge:** a rebase keeps main linear and forces
  the member's branch; a merge never forces and leaves merge commits.
  Default: the rebase.
- **A judgement gate's key** as a patch id of the member's diff, which a
  rebase over conflicting lines would keep and a clean prepare (the
  mechanism here) may not; kept as clean prepares until a project needs
  more.
