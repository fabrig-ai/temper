# Goals

Kind: concept

A goal is a tracked task: work given a numeric priority, which orders the goals' listing and the work waiting for resources, and whose tree is its plan.

## What it is

- **A goal is a task that is tracked,** at a priority, a number;
  any task can be one, a delegate among them. `task show` prints its
  tracking as `priority P`, and an untracked task's as `untracked`.
- **Its plan** is its tree of delegates: `task show` prints it in one line,
  `plan:` and how many live delegates run, wait or are held below.
- **The goal template,** template 2 in the stock settings, makes a
  tracked task at priority 1.

## How it changes

- **Made a goal:** an agent's run calls `goal` with the work in your
  words to make a tracked delegate that plans, asks and calls `change`.
  With `advisor` set, it reads and reports without changing the forge,
  and stays untracked. A run can also delegate a task tracked at a
  priority with `delegate`; or a party accepts a
  proposal that makes one; or an amendment tracks a task.
- **Reprioritised:** an amendment of its priority alone needs no
  spec revision; tracking an untracked task, or untracking a goal,
  names the spec revision the amender saw.
- **Ended** as any task ends (temper help task).

## What temper guarantees

- **The listing's order:** `task list` ordered by priority lists the lower
  number first, then by task number.
- **Waiting work in the same order:** among tasks waiting for the same
  write hold, or for a slot to run in, temper starts the one under the
  lower priority number first, each task taking its nearest goal's
  priority; a task under no goal waits behind every goal's, then the
  earlier wait goes first.
- **No interruption:** a new priority does not stop or restart a run
  in progress.

## Acting on it

- **ctl:** `task list` lists a project's goals, tracked alone and
  ordered by priority; `task show` reads one; `task amend` changes its
  priority or its tracking; `proposal decide` accepts a proposal that
  makes one.
- **temper's runs** make goals with `goal` or `delegate` and reprioritise
  their delegates with `amend`.

## Limits

- temper does not let ctl start a goal: one comes from a run's
  delegation, an accepted proposal, or an amendment of a task.
- temper has no board to reorder goals: each changes by its own
  amendment.
