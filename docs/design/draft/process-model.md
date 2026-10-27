# The process model

Draft, 2026-10-27, from a session with the user. Nothing here is
adopted. How temper runs a development process in which agents, under a
person's direction, change several repositories at once. It covers who
decides, how changes land together and are proved, how a branch is
verified after landing, how findings flow, and how the person steers
and holds the work. It names no real process. `reference-case.md`
applies it to an invented one, and `porting.md` maps a real one onto
it. Its gaps against jig's and temper's designs are section 12.
Unqualified, forge.md is temper's `../domain/forge.md`.

## 1. In one page

- **Four layers, each with its own job** (section 2):
  - **The forge** keeps the record and runs CI. It holds branches, pull
    requests, CI runs, commit statuses, and issues with labels.
  - **The project's tooling** makes candidate trees and says what a
    gate depends on.
  - **temper** owns the mechanics: landings, proofs, receipts, the
    landing turn, verdicts, findings and QA.
  - **jig** owns the tree: who works for whom, who is woken, and who
    decides.

  Nothing is forced into the forge, and nothing specific to one
  project is built into temper.
- **The tree** (section 3): the person talks to a chat. The chat
  coordinates goals, which can coordinate goals of their own at any
  depth. A goal's delegates are agents, procedures or people. Any task
  may ask the person directly. A hold has an owner, so only that owner
  may lift it. A hold either stops work at once or drains it, and it
  covers one task's subtree or a whole project. A live task can be
  moved under another task, and a task that ends can name a successor.
- **A landing** (section 4) is one or more changes that land together,
  in one or more repositories and from one or more goals. A single
  change is a landing of one.
  - **Prepare.** The project's tooling makes each repository's
    candidate: its main's tip with the landing's changes on top, and
    downstream pins bumped.
  - **Proof.** temper dispatches only the gates that have no receipt
    for their key. The key is what a gate depends on: the tree, minus
    the paths the gate ignores, with the gate's definition and the
    policy.
  - **The turn.** Once every gate holds, temper takes the project's
    landing turn for a few seconds. It fast-forwards each main to its
    proved candidate, upstream first.
  - **No test runs twice on the same tree.**
- **After landing** (section 5): the slower tiers run on the newest
  main, one run at a time per branch.
  - **A green run** clears every landing up to its commit.
  - **A red run** is rerun once. If the same tests fail again, temper
    bisects over the landings in the range and blames only new
    failures. The verdict goes to the landing's goal, and other
    landings go on.
  - **A change's task** lives until its landing is cleared, so a goal
    ends only when its work is verified.
- **Findings** (section 6) are forge issues, classified by labels.
  temper reads them back, matches failures against them, and has them
  triaged to a goal, to the backlog, or dropped. A landing that fixes
  one closes it once that landing clears.
- **QA after landing** (section 7): a landing marks what a user can now
  do. A chooser picks which marks to test, and one tester at a time
  tries them on the newest cleared main. The tester has no forge
  effects, and what it finds becomes findings.
- **Landing news carries the commit's trailers** (section 8), so what
  a landing taught reaches its coordinators with no message of its own.
- **The person's questions can wait on the forge** (section 9,
  optional): a question becomes an issue, and the person answers it in
  a comment.

## 2. Layers: who owns what

| Layer | Owns | Never |
|---|---|---|
| The forge | branches and pull requests, as the record and the place people review; CI runs, as one place gates and tiers run; commit statuses; issues and labels, as the store of goals' records, findings and, optionally, waits | decides when anything lands, or which landings go together |
| The project's tooling | `prepare`: from a landing's changes and the mains, a candidate commit per repository; `key`: what a gate's result depends on, when the default is not enough | holds a model or forge credentials; pushes |
| temper | changes, landings, receipts, proofs, the turn, tiers' runs and verdicts, findings' mechanics, QA passes, projections onto the forge | a project's build, its pins or its conflict rules |
| jig | tasks, goals, coordinators, messages, holds, proposals, authority, spend | knows a forge |

- **The forge as a primitive.** A pull request is a member's record:
  it shows the candidate that landed and the statuses that proved it,
  and it is where people read and review. A CI run is a gate or a
  tier, dispatched by temper. An issue is a goal's record, a finding,
  or a wait. Merge queues, auto-merge, protection's required checks and
  push-triggered CI are not used for landing. Protection can still
  require temper's statuses (4.4).
- **The tooling as commands.** temper runs them on a worker with no
  model and no forge credentials, at a pinned revision of the project's
  home, with a timeout. They read the checkouts temper gives them and
  write what section 4.3 says. A project with no tooling gets temper's
  default prepare, a rebase of each change onto its main, and the
  default key (4.4).

## 3. The tree of work

### 3.1 Coordinators, goals and delegates

- **The person's chat** is a coordinator: an agent conversation whose
  charter lets it make goals and steer them (jig's `domain/people.md`,
  section 7).
- **A coordinator goal** coordinates goals of its own: it designs, then
  delegates the parts. Its delegates can be coordinators too, so the
  tree goes as deep as the work needs. Depth is policy (authority's
  depth), not a fixed set of layers.
- **A goal's delegates** are agents (implementers, reviewers,
  advisors, testers), procedures (a change, a landing, a QA pass), or
  people (a question, an approval).
- **Models are per charter:** a coordinator's charter can name one
  model, and its implementers' charter another. A delegate's charter is
  one its parent's authority allows.
- **Briefs, notes and results.** A goal starts from its brief and keeps
  notes while it works. It ends with a result that says what it did and
  what is left. That result can be the next goal's brief.
- **Checks on a period.** A coordinator wakes on its delegates' results
  and on news. A timer or a recurring task wakes it on a period as well
  (jig's `domain/tasks.md`, section 10).
- **Standing rules are instructions.** A rule such as "every new
  feature gets a designer" lives in the coordinators' charter. When the
  rule changes, the charter gets a new revision, and live tasks are
  moved onto it (jig's `MoveToNewest`, `domain/people.md`, 5.1). Rules
  are judged by the agents that read them, not enforced by the engine.

### 3.2 Asking the person

- **Any task may ask the person directly** if its authority lets it
  make a person task addressed to them. The answer goes back to the
  task that asked, and nobody in between is told.
- **The chat may ask its own person,** with a question in the inbox,
  like any other task (gap J8).
- **Questions to the chat stay with the chat.** A delegate that asks
  its requester, the chat, is asking the chat. The chat answers itself,
  or asks the person in its own words. The question is not placed in
  the person's inbox on the chat's behalf (gap J7).
- **An answer after its asker ended** goes to the asker's successor
  (3.4). With no successor, it goes to the asker's requester, as news
  naming the question.
- **On the forge, optionally:** section 9.

### 3.3 Holds with owners

- **A hold has an owner:** the party, or the task, that placed it.
  - Only the owner can lift it. So can a party whose role in the project
    stands above the owner (a maintainer over a member).
  - A task above a party's hold cannot lift it. It can only propose to
    lift it, and the proposal waits in the owner's inbox (jig's
    `domain/tasks.md`, section 9).
  - A hold a task placed (a coordinator parking a delegate) can be
    lifted by that task, by any task above it, and by the parties over
    its tree.

  This is gap J1.
- **Two modes:**
  - **Stop.** Runs are cancelled at once, and the tasks are held. This
    is jig's `Stop`, now with an owner.
  - **Drain.** Running runs finish their turn, and running changes
    finish, landing included. Nothing new starts: no delegates are
    made, no due task gets a run, and no new landing is proved. Each
    task is held as it goes idle (gap J2).
- **Two scopes** (gap J3):
  - **A task's subtree.** For example, the person pausing one goal, or
    a coordinator parking a delegate.
  - **A project.** "Finish what runs and start nothing" is a project
    drain. "Act on nothing" is a project stop. Tasks made under a
    project hold start held. The person's chat is exempt, so the person
    can still talk to it.
- **Park until an event** (gap J4): a task-owned hold that lifts itself
  when a named event comes: a landing on a branch, a task's end, or a
  time. A coordinator uses it to park a delegate until what it waits
  for exists.
- **Lifting a project hold** lifts it from the project. A task that
  went idle under it is released only if the owner lifts it as well,
  or lifts every hold at once. This keeps the person able to resume
  goals one at a time.

### 3.4 Moving and handing on

- **A live task moved under another task** (gap J5): anyone who may
  amend both tasks can make the second one the first one's requester.
  The moved task's reservation moves to the new parent's funding. Its
  results, questions and escalations go to the new parent from then
  on, and the old parent keeps a reference to it. jig today moves a
  task only to a party.
- **A successor** (gap J6): a task that ends can name a live task as
  its successor. These then go to the successor:
  - answers to its open questions;
  - its subscriptions and kept news;
  - its notes, by reference.

  Its requester still hears its result.

## 4. Changes and landings

### 4.1 A change

- **One change to one repository's branch,** as forge.md 8.1 has it:
  - it has a branch of its own and a pull request;
  - it is produced by an agent and repaired by agents;
  - its gates are its own plus the project's.
- **It lives past its landing** (gap T10). Its phases are:

  ```
  producing ─► ready ─► preparing ─► proving ─► turn ─► landed ─► verifying ─► cleared
                            ▲           │                             │
                            └─repairing─┘                             └─► red ─► cleared
  ```

  Preparing, proving and the turn are its landing's (4.3 to 4.6).
  Its result is given at *cleared*, so the goal that asked for it waits
  for verification. Once it is *landed*, its landing is news to the
  repository's subscribers, as today (forge.md, 7.2).

### 4.2 A landing

- **Its members** are changes, at most one stack per repository, from
  any goals. Changes into one repository in the same landing are
  applied in dependency order.
- **How changes join one landing** (gap T1):
  - **`with`:** a change names the changes it must land with. The
    change can say so when it is made, or it can be amended later, by
    a task that may name the other change (jig's `domain/tasks.md`,
    section 7's rule for dependencies). `with` is symmetric and
    transitive.
  - **A dependency across repositories** joins the two changes, unless
    the project's policy says that such dependencies only order them.
  - **Pin-only members.** Prepare adds a member in a downstream
    repository whose only change is the bumped pin, when the policy
    keeps downstream pins at their upstream's main.
- **Ready** when every member is ready: produced, with its reviews and
  approvals in hand.
- **A landing is a procedure task,** the landing task, made by temper
  when its first member is ready. Its members' change tasks wait on it.
  It answers to no one: its members' goals hear of it as news, and
  their repairs are made under them (4.5).
- **Several goals, one landing.** Each member stays its own goal's. The
  landing's news reaches every member's goal. A repair goes to the goal
  of the member that failed.

### 4.3 Prepare

- **What it is given:** the members (repository, branch, head), each
  main's tip, and the project's dependency graph between repositories.
- **What it makes:** a candidate commit per repository, at most one,
  whose parent chain starts at that main's tip. For example, it can:
  - rebase or merge each member onto main;
  - bump a downstream's pin to its upstream's candidate;
  - regenerate files that are derived from others;
  - settle conflicts that its rules know how to settle (a table row
    added on both sides, a lock file).
- **What it says:** for each repository, the candidate or a conflict it
  could not settle, with the files. For each gate, the key of its
  candidate when the project's `key` command is part of prepare (4.4).
- **A conflict it cannot settle** becomes a resolution task under the
  goal of the conflicting member, whose workspace is the merge in
  progress (forge.md, 8.4). Its result goes back into the member's
  branch, and the landing prepares again.
- **Where candidates go:** temper pushes each candidate to its member's
  own branch, so the pull request shows what will land (gap T5):
  - **a rebase is a forced update** of that branch, guarded by the head
    temper last read, and allowed only on branches the change holds;
  - **a pin-only member** gets a branch and a pull request of its own;
  - **a repository with several members** gets the stack on its last
    member's branch.

### 4.4 Receipts

- **A receipt** is a gate's result under a key: pass or fail, the run's
  link, when it ran and where (gap T3).
- **The key** is what the result depends on:
  - **a test gate** (a CI workflow, or a check on a worker): the
    candidate's tree, without the paths the gate declares it ignores,
    plus the gate's definition and the policy revision it ran under
    (the runner's class, its timeout);
  - **a judgement gate** (a review, a person's approval): the member's
    diff from its base, as a patch id, so a rebase that leaves the diff
    unchanged keeps it. This generalises forge.md 8.3's freshness:
    *clean* becomes "the same diff", and *exact* becomes "the same
    tree".

  The project's `key` command replaces the default key when a gate
  reads more than the tree, for example an image it pulls by tag.
- **Kept by temper,** per project, in its store, and reused across
  members, landings and workers. A receipt is never trusted from a run
  temper did not dispatch.
- **Mirrored on the forge** as a commit status on the candidate,
  `temper/gate/<name>`, with the key in its description and the run as
  its link. A fast-forward makes main's commit the candidate, so main
  shows the statuses that proved it, and protection can require them.

### 4.5 Proof

- **Dispatch-only gates.** Each gate workflow runs only when temper
  dispatches it, never on a push or a pull request event (gap T4).
  temper dispatches a gate when its key has no passing receipt, with
  the candidate's ref and the key as the run's inputs. The run reports
  to its commit. A check on a worker is a task at the candidate, as
  forge.md 8.3's checks are.
- **Proved** when every member's gates hold at exactly its candidate,
  by receipt. A landing whose members change nothing a gate reads
  (documentation, when every gate ignores it) is proved without a run.
- **A failure:**
  - **A real one** is a repair, as forge.md 8.4 makes it. It is a task
    under the failing member's goal, given the failing output. The
    landing then prepares again, and only the keys that changed run.
  - **A fault of the runner** is retried, and is not a repair: the run
    was lost, the runner was refused, or a tool was missing. A fault
    that lasts past its bound holds the landing and tells its goals.
  - **A failure that only matches open flake findings** (6.3) has its
    failed jobs rerun once. Only if the rerun fails is it a repair.
- **Proofs run side by side.** Any number of landings can prove at
  once. Only the turn is exclusive.

### 4.6 The turn

- **One per project,** held for seconds (gap T6). Any landing whose
  proof holds can take it.
- **First, it checks the bases.** If a member's main is not its
  candidate's parent any more, the proof is stale. The landing goes
  back to prepare, and its other members keep their receipts.
- **Then it lands each member, upstream first:**
  - on Forgejo, by merging the pull request with `fast-forward-only` at
    `head_commit_id`, the candidate;
  - elsewhere, by a fast-forward update of the branch's ref, refused if
    it is not a fast-forward;
  - where neither is open to temper, by the forge's merge commit at the
    guarded head. Its tree is the candidate's, because the candidate's
    parent is main's tip (gap B2).

  Either way, temper reads main's new tree and compares it to the
  candidate's. A rebase or squash merge is never used for a repository
  that others pin, because the pinned commit has to stay on its main.
- **The pull request is marked merged** by the merge itself, or, after
  a ref update, by the forge once main holds its head. In a stack, the
  members below the top are marked the second way, which on Forgejo
  needs the repository's "autodetect manual merge". temper never pushes
  main from a checkout, and never runs a gate inside the turn.
- **Partial:** if the turn ends with some members landed (a forge
  error, or a main moved by someone outside temper), the landing is
  partly landed. The landed members stay landed. A downstream that has
  not landed still pins its upstream's previous commit, which is a
  consistent state. The rest of the landing prepares and proves again,
  and takes the turn again.

### 4.7 Fairness

- **First proved, first landed:** the turn goes to the landing whose
  proof completed first. Priorities order the dispatch of gates when
  runners are scarce, not the turn.
- **Reservation** (gap T7): a landing whose proof went stale twice gets
  the project's next turn for its repositories. While it re-proves, any
  other landing that touches those repositories can prove but cannot
  take the turn. The reservation lapses after one more proof, or when
  the landing fails. It replaces both forge.md 9's aging and a person
  holding landings to let one through.

### 4.8 No test runs twice on the same tree

- The fast-forward makes main's commit the gated commit, so main is
  never gated again.
- Nothing is gated inside the turn.
- A re-prepare runs only the keys that changed. A member whose
  repository did not move keeps its receipts.
- A tier (5.1) whose key has a receipt is not run again.
- Documentation-only landings run nothing, because every gate's key
  ignores their paths.
- **The exceptions are deliberate:**
  - one rerun of failed jobs, for a flake;
  - bisect runs, which are on different trees;
  - a downstream whose pin changed, which is a different tree.

### 4.9 What this replaces in forge.md

- Per-branch landing queues and the forge's merge queue (forge.md,
  section 9 and 9.1) give way to landings, side-by-side proofs and the
  turn.
- The merge at a change's head in the repository's merge style (8.2,
  step 7) gives way to the fast-forward.
- Updates by the forge's merge (section 10) give way to prepare.
- CI on every pushed head (8.2, step 3) gives way to dispatch.
- The paused queue and its one repair (section 9's "branch's own CI")
  give way to section 5's verdicts.

## 5. After landing

### 5.1 Tiers

- **A tier** is a check too slow or too broad for a gate, such as a
  browser suite or an exhaustive profile. It is defined like a gate,
  with its key.
- **Tiers follow the mains, not the landings.** When a branch's tip
  moves, by temper or by anyone else, its tiers run on it.
  - Only one run per tier and branch is in flight at a time.
  - When the run ends, the next one takes the newest tip, so one run
    covers every landing since the last.
  - A tip whose key has a receipt is judged by that receipt, with no
    run.
- **What a tier reports:** pass or fail, and the failing tests by id,
  in a declared format (a JUnit file, or one id per line) read from the
  run's artifacts. A run that names no tests is a fault.

### 5.2 Verifying and cleared

- **Landed means verifying.** A change is *cleared* once each of its
  repository's tiers has a run whose range holds its commit and blames
  it nothing (5.3). A repository with no tiers clears its landings as
  they land.
- **A bound:** past the project's verification bound (two hours by
  default), with no verdict because the runner was faulty or down, the
  change ends *landed, uncleared*, and its goal is told. A verdict that
  comes later still counts. A red then becomes a finding (5.4), because
  the goal may have ended.
- **A goal ends when its changes have cleared,** because its change
  tasks give their results only then. A coordinator that wants to go on
  sooner does not wait for the result, and still hears the verdict.

### 5.3 The verdict

- **Green:** every landing in the run's range is cleared. The range is
  from the last run's commit, exclusive, to this one.
- **Red:**
  1. **Rerun once.** The failed tests run alone at the same commit. If
     they pass, the run was a flake: the range is cleared, and a flake
     finding is filed, or the matching one is counted again (section
     6).
  2. **Inherited failures** are tests that already failed in the
     previous run. They are not blamed on this range: they belong to
     the landing that first failed them, or to their finding.
  3. **New failures, one landing in range:** they belong to that
     landing.
  4. **New failures, several landings:** temper bisects over the
     landings' commits in the range, running the failing tests alone,
     since each landing's commit was proved. The first landing whose
     commit fails them owns them. The project's policy can say "tell
     all" instead: every landing in the range is a suspect, and each
     suspect's goal is told.
  5. **A fault** is not a red: a runner refused, a disk full, a missing
     tool. It is retried, and if it lasts, the people the project names
     are told.
- **A landing's verdict is its own,** never read off the tip:
  - it is red only by failures blamed on it;
  - it is cleared by the first run whose range holds it and blames it
    nothing;
  - once red, it stays red until a later run passes its failures.

  So a green tip never hides a red landing behind it, and a red run of
  inherited failures never reddens the landings that came after.

### 5.4 Where a red goes

- **To the landing's goals:** news to each member's goal, with the
  failing tests and the log's end. The goal makes the repair, as it
  makes any change, and that change names the red one. The red change
  ends *cleared* at the first run where its new failures pass, naming
  the landing that repaired them.
- **Others go on** by default. Landings keep landing, and their own
  verdicts do not count the inherited failures. A project's policy can
  pause new turns into that repository until it is cleared instead.
- **A goal that has ended:** the red becomes a finding of kind `red`,
  and is triaged (6.4).
- **On the forge:** the verdict is a commit status on the run's commit,
  `temper/verdict/<tier>`, and a comment on each member's pull request
  ("cleared by run N" or "red: tests X, Y; repair in #M").

## 6. Findings

### 6.1 What a finding is

Something found that is not the finder's to fix now: a QA pass's
finding, a flake, a red whose goal has ended, or a review's follow-up.

- **Stored as forge issues** in the project's home, labelled:
  - `finding`;
  - its kind: `qa`, `flake`, `red` or `review`;
  - its severity and its area;
  - its state: `new`, `routed`, `backlog` or `dropped`.

  Its body carries its reproduction, the commit, and its signature (the
  test ids and the failure's first line) where it has one.
- **Read back** (gap T11): labels are part of the finding's state.
  Labels that a person changes on the forge are an input temper reads,
  so a finding can be triaged in either place. This is a narrow
  exception to forge.md 12's "written and never read back".
- **temper keeps the mechanics:** filing, matching, routing and
  closing. The forge keeps the record.

### 6.2 Sources

- QA passes (section 7) file what they find.
- Verdicts file flakes (5.3) and reds whose goals have ended (5.4).
- An agent's tool files what it finds outside its task, as forge.md 12
  has it.

### 6.3 Matching

- **A failure's signature** is matched against open findings by test
  id.
- **A flake that matches** is counted on its finding, not filed again.
- **A failure at a gate that only matches open flake findings** has its
  failed jobs rerun once rather than repaired (4.5).

### 6.4 Triage

- **The triager** is named by the project's policy: a coordinator task
  (often the person's chat) or a person. New findings wake it, at once
  or on its period.
- **It routes each finding:**
  - **to a goal:** the goal hears it as news, the label becomes
    `routed`, and the finding names the goal;
  - **to a new goal:** the triager makes one, naming the finding;
  - **to the backlog,** until a later triage;
  - **or drops it,** with a reason, and the issue is closed.

### 6.5 Closing

- **A landing whose change says `Fixes #n`** (in its commit or its pull
  request) closes finding *n* once the landing is **cleared**, not when
  it lands.
- **A finding a person closes** on the forge is closed in temper as
  well.

## 7. QA after landing

- **Marks:** a landing marks what a user can now do, as a `QA:` trailer
  in its commit or as a `qa` label on its pull request. Only marked
  landings are tested.
- **The chooser:** the project names one, usually the triager. It
  decides which marks get a pass and which wait in a backlog of marks,
  from priorities and spend.
- **The pass:** a procedure (gap T12) that starts one pass at a time
  per project once a chosen mark is on a cleared main. It runs on the
  newest cleared main, and covers every chosen mark that has not been
  tested yet. Marks chosen while a pass runs go into the next one.
- **The tester:** an agent under the project's tester charter. It has
  no forge effects and cannot push, and it uses the product as its
  users do. It writes down what it expects before trying each case.
  Its result is a report: each case's outcome, and each finding with
  its reproduction.
- **Its findings** are filed (section 6), and the triager routes them.

## 8. What a landing tells

- **Landing news carries the trailers** of each commit it brought to
  main (gap T13): `Taught:`, what the code taught that the design does
  not say; `QA:`; `Fixes:`.
- **A coordinator's wake policy can name trailers.** For example, a
  designer wakes on `Taught:` in its subtree's landings, and keeps the
  rest of the news for its next run.
- **A read of landings since a time,** per project, with their
  verdicts and trailers, for a coordinator catching up.

## 9. The person on the forge (optional)

- **A question to a person** can be projected as an issue in the
  repository it concerns, labelled `waiting-on-person`. The issue
  carries the question and the asking task (gap T14).
- **The person answers in a comment.** The comment comes from the
  forge account that adoption mapped to that party, and temper reads it
  as the person task's answer. The inbox shows the same question, and
  whichever answer comes first wins.
- **Closed when the question ends:** answered, cancelled, or ended
  with its asker.
- **Off by default.** It is the one exception to forge.md 13's rule
  that what people write on temper's objects reaches no task. The
  exception is narrow: one issue per question, the mapped account
  only, and nothing else read.

## 10. What people see

- **On the forge:**
  - each member's pull request, with its candidates in its timeline,
    its gates' statuses, its verdict and its landing;
  - the findings and the waits, as labelled issues;
  - main's statuses.
- **In the client:**
  - a landing's panel: its members, candidates, receipts, the proof's
    state, the turn, and the verdict;
  - a repository's tiers: the run in flight, and the last green and
    first red commits;
  - the findings, by state;
  - the holds, with their owners and modes.
- **Through `temper ctl`:** the same reads and acts, so an agent
  driving temper sees what a person sees.

## 11. Settings

| Setting | Scope | Default |
|---|---|---|
| prepare command, key command | project | temper's rebase; the tree-based key |
| gates: workflow or check, ignored paths, blocking or advisory | project, repository | the repository's adoption |
| tiers: workflow, ignored paths, test report format | repository | none |
| a dependency across repositories joins a landing | project | yes |
| downstream pins kept at upstream's main | project | no |
| stale proofs before a reservation | project | 2 |
| verdict on several suspects: bisect or tell all | project | bisect |
| a red pauses its repository's turns | project | no |
| verification bound | project | 2 hours |
| triager, QA chooser, tester charter | project | the person's chat; the chat; none (no QA) |
| person's questions on the forge | project | off |

These join the landing rules as settings (forge.md, 8.9).

## 12. Gaps

Grades as temper-on-temper.md section 2 has them: *exact* (there
today), *sugar* (composed of what is there), *design* (to design, and
it fits), *conflict* (to design, and it changes something designed).
Each gap has an id that the case and the porting guide cite.

### 12.1 jig

Its paths are jig's, unless marked temper's.

| Id | Gap | Grade | Today |
|---|---|---|---|
| J1 | A hold's owner: a party's hold is lifted only by that party, or a role above it; a task above it may only propose | conflict | a hold escalates to the nearest task or party above, which may release it (`domain/tasks.md`, 5.5; `domain/people.md`, 5.1 `Stop`) |
| J2 | Drain: running runs and changes finish, nothing new starts, each task is held as it goes idle | design | `Stop` cancels the run at once; `domain/engine.md` 6.1 stops only the process |
| J3 | Holds over a project, which tasks made later inherit; the person's chat exempt | design | holds are per task; a compound request stops the tasks that exist now (`domain/tasks.md`, section 7) |
| J4 | Park until an event: a task-owned hold lifted by a landing, a task's end or a time | design | a hold is lifted by a decision; only a party stops a task (`domain/people.md`, 5.1); timers and subscriptions exist |
| J5 | Move a live task under another task | design | `Move` makes only a party the requester (`domain/tasks.md`, section 7) |
| J6 | A successor: answers, subscriptions and kept news of an ended task go to it | design | none; an answer after its asker ended has no rule |
| J7 | A question to a chat stays with the chat | conflict | jig puts it in the chat's person's inbox (`domain/tasks.md`, 8.1; temper's `../domain/agent.md`, 8.2) |
| J8 | A chat asks its own person | design | the chat "says in its reply that a question waits" (temper's `../domain/agent.md`, 8.2) |
| — | Coordinators at any depth, delegates per charter and model, briefs and results, periodic checks, any task asking a person, proposals, a charter's new revision reaching live tasks | exact | `domain/core.md`, 3.2, 6.2, 7.2; `domain/tasks.md`, 9, 10; `domain/people.md`, 5.1, 8 |

### 12.2 temper

| Id | Gap | Grade | Today |
|---|---|---|---|
| T1 | The landing: changes across repositories and goals, joined by `with`, dependencies and pin-only members; a landing task | design | "several change tasks, ordered by their dependencies" (`../domain/forge.md`, 8.1) |
| T2 | Prepare by the project's tooling, on a worker with no model and no credentials; conflicts settled by rule, else a resolution task | design, replaces | updates by the forge's merge (`../domain/forge.md`, section 10); every conflict an LLM's (8.4) |
| T3 | Receipts by key: tree without ignored paths, gate, policy; a diff's patch id for judgements; per project; mirrored as statuses | design | freshness *exact* or *clean* per head (`../domain/forge.md`, 8.3) |
| T4 | Dispatch-only gates: dispatched for keys with no receipt; failed jobs rerun; results read from statuses and artifacts | design | CI runs on every pushed head (`../domain/forge.md`, 8.2, step 3) |
| T5 | A candidate as a forced update of the change's own branch, guarded by the head last read | conflict, small | a push is a fast-forward, never forced (`../domain/worker.md`, section 5) |
| T6 | The turn: one per project, bases checked, fast-forward upstream first, trees read back, a partial landing resumed | conflict, replaces | a merge at the head in the merge style, per-branch queues, the forge's merge queue (`../domain/forge.md`, 8.2, section 9, 9.1); pushing several is not atomic (`../domain/worker.md`, section 5) |
| T7 | A reservation after stale proofs | design, replaces | aging by the queue window (`../domain/forge.md`, section 9) |
| T8 | Tiers following the mains, one run at a time, newest tip, by receipt | conflict | CI failing on the tip pauses the queue for one repair (`../domain/forge.md`, section 9) |
| T9 | Verdicts: rerun once, new failures by test id, bisect or tell all, inherited failures unblamed, faults apart, worst over the range | design | none |
| T10 | A change lives to *cleared*; a verification bound; a goal ends after its changes clear | design | a change's result is its landing, and it closes (`../domain/forge.md`, 8.2, step 7) |
| T11 | Findings: labelled issues read back, matched by signature, triaged, closed when a fix clears | conflict, narrow | issues "written and never read back"; findings keyed, their state in the store (`../domain/forge.md`, section 12) |
| T12 | QA after landing: marks, a chooser, one pass at a time on the newest cleared main, a tester charter with no forge effects | design | none in the product (`qa-lab.md` is the lab's) |
| T13 | Landing news with trailers; wake policies on them; landings since a time | sugar | landing news carries before, after and files (`../domain/forge.md`, 7.2, 7.3) |
| T14 | A question projected as an issue, answered by the mapped account's comment (opt-in) | conflict, narrow | "a comment on temper's pull request or issue reaches no task" (`../domain/forge.md`, section 13) |
| T15 | The client's and `ctl`'s views and acts for landings, receipts, tiers, findings and holds | design | a change's panel (`../domain/forge.md`, 8.8) |

### 12.3 The forges

| Id | Gap | Grade | Today |
|---|---|---|---|
| B1 | Calls in skein's forge protocol: dispatch a workflow with inputs, rerun failed jobs, read a run's artifacts, write a commit status, merge fast-forward only at a head | design | skein's `forge.md` reads CI and statuses; Forgejo 17's API has every call (`actions/workflows/{file}/dispatches`, `actions/runs/{id}/rerun`, `actions/runs/{id}/artifacts`, `statuses/{sha}`, merge `Do: fast-forward-only` with `head_commit_id`) |
| B2 | GitHub: no fast-forward merge; a ref update needs a bypass of the pull-request rule, or the member lands by a merge commit whose tree is checked | to check | temper's App has `statuses: read` and no more (`../domain/forge.md`, 8.3) |
| B3 | GitHub App permissions: statuses and actions written | design | `../protocol/setup.md`, 17.1 |
| B4 | Adoption checks a repository's workflows are dispatch-only, and Forgejo's "autodetect manual merge" is on where landings stack | design | adoption reads the branch's rules (`../domain/forge.md`, 4.4) |

## 13. Open questions

- **Who makes the landing task:** temper on the first ready member, as
  above, or a goal that asks for it explicitly ("land these together").
- **Reviews in a landing:** whether a member's agent review runs before
  its landing prepares, as above, or on the candidate.
- **Prepare's rebase or merge:** a rebase keeps main linear but forces
  the member's branch, and a merge never forces but leaves merge
  commits on main. The project chooses. Which should be the default?
- **The bound and the goal:** whether a goal can close at *landed,
  uncleared* on its own judgement before the bound.
- **Tiers on the forge's runners or on workers:** both are allowed
  here. Which is the default for a box with an owned Forgejo?
