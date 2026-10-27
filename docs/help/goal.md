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

- **Made a goal:** an agent's run delegates a task tracked at a
  priority, with the host tool `delegate`; or a party accepts a
  proposal that makes one; or an amendment tracks a task.
- **Reprioritised:** an amendment of its priority alone needs no
  spec revision; tracking an untracked task, or untracking a goal,
  names the spec revision the amender saw.
- **Ended** as any task ends (temper help task).

## What temper guarantees

- **The listing's order:** `task list` ordered by priority lists the lower
  number first, then by task number.
- **No interruption:** a new priority does not stop or restart a run
  in progress.

## Acting on it

- **ctl:** `task list` lists a project's goals, tracked alone and
  ordered by priority; `task show` reads one; `task amend` changes its
  priority or its tracking; `proposal decide` accepts a proposal that
  makes one.
- **temper's runs** make goals with `delegate` and reprioritise
  their delegates with `amend`.

## Limits

- temper does not let ctl start a goal: one comes from a run's
  delegation, an accepted proposal, or an amendment of a task.
- temper has no board to reorder goals: each changes by its own
  amendment.
- Among tasks waiting for the same resources, temper starts the one
  under the higher priority number first, each task taking its nearest
  goal's priority: the reverse of the listing's order.
