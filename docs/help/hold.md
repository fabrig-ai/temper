# Hold

Kind: concept

A hold keeps a task from running until someone with the right to lift it says so; a placed hold is one a party placed, and only its owner, or a role above theirs, lifts it.

## What it is

- **A held task** has no run. It keeps what it had: its turns, its
  inbox, its result while it was closing. Its document says why it is
  held and gives the hold's revision, which a release names.
- **Two kinds of hold.** temper holds a task by itself when something
  needs a decision: its tries ran out, its budget or deadline did, an
  effect failed, its executor stalled. Such a hold waits for a decision
  from the task above it or a person, and a release lifts it. A party
  holds a task by placing a hold: today, by stopping it.
- **A placed hold** is a record of its own, numbered in the deployment.
  It has an owner (the party who placed it), a mode (stop), a scope (a
  task and every task below it, those made later included), when it
  was placed, and a revision of its own. Every task it holds names it.
- **A stop** places a hold of the stopping party's over the task's
  subtree: the task and each of its delegates is held, and each live
  run among them is cancelled at once.

## How it changes

- **Placed:** a party stops a task. The hold's revision is 1.
- **A task released from it:** releasing the stopped task itself lifts
  the whole hold, and every task it holds that nothing else holds goes
  on. Releasing a task below it releases that task alone, which is
  then exempt from the hold; the others stay held.
- **Lifted:** the owner lifts the hold by its number and revision.
  Lifted alone, the tasks it held stay held, each released one at a
  time; lifted with its tasks, every task it holds that nothing else
  holds goes on. Lifting moves the hold's revision. A lifted hold is
  kept while a task it held is still held by it.
- **Ended with its task:** a stop whose task ends, cancelled, is erased.
- **Proposed:** a task above a stopped task cannot release it, since
  the hold is a party's. It proposes the release instead, naming the
  hold's revision. The proposal waits in the owner's inbox; accepted,
  it releases the task as the owner's own release would; lifting the
  hold, or a newer revision, withdraws it.

## What temper guarantees

- **Only its owner lifts it,** or a party whose role in the project
  ranks above the owner's: owner, then maintainer, then member, then
  observer; a role of the project's own policy ranks with member. A
  release or a lift by anyone else is refused, naming the hold and its
  owner, and changes nothing.
- **One placed hold at a time:** a task inside two stops is held by
  the first; released from it, it is judged afresh and held by the
  other.
- **A hold of temper's own stays its own.** A task that was already held
  for a decision keeps that hold and its escalation when a stop covers
  it; deciding it leaves the task held by the stop.
- **Nothing new under a stop:** a run in its scope that asks for a
  delegate is refused, naming the hold.
- **A stopped task resumes from its turns:** its next run reads the
  conversation it had.
- **Decisions name what they saw:** a release names the task's hold
  revision, a lift and a proposal the hold's. A stale one is refused as
  moved, with the revision now.

## Acting on it

- `temper ctl`: `task show` shows a held task's hold: its number,
  owner, mode, scope, revision, and whether you may lift it; its acts
  give the release when you may. `task stop` places a stop; `task
  release` releases one task; `hold lift` lifts a hold, alone or with
  its tasks; `inbox`, `proposal show` and `proposal decide` show and
  accept a proposed release.
- A run proposes a release with the host tool `propose`, the action
  `release`, naming the task, the hold and the hold's revision.

## Limits

- Placed holds in the deployment, in a project and per owning party,
  and the tasks one hold exempts, each bounded; one more is refused,
  naming the bound.
