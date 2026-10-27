# Budgets

Kind: concept

A budget is what a task may spend, in units its models' prices make of their usage, and by when; each run draws a share of it, and a task without enough is held.

## What it is

- **Spend** is counted in whole units. A model's prices turn its
  usage, fresh input, output, cache reads and cache writes, into units;
  reasoning is output and counts once. A unit has no currency.
- **Two scales in ctl:** `me` prints units as they are, `budget 40`;
  a task prints them divided by 100, `spent 0.01 of 0.40`.
- **A task's budget** is the spend its authority allows, and its
  deadline, a wall time, where it has one.
- **Funding** comes from a person's pool in a project's period, from
  an ancestor task, or from a recurring task.
- **A run** draws a share of its task's budget, within the
  deployment's run cap; its charter (temper help charters) sets its
  turns, its time, a reserve to finish in, and its default spend. The
  stock rules' floor is 1 unit and their run cap 10; the stock
  charters' spend is 20 units, 8 turns and 120 seconds, with a reserve
  of 1 turn and 60 seconds; `setting` reads the current ones.

## How it changes

- **At the start:** a new task's template sets its budget rule: an
  amount, as much as is allowed, or the amount its maker gives.
- **Delegated:** a delegate's budget is reserved from its funder's.
  When the delegate settles, its spend counts below the funder, the
  reservation goes, and what it did not spend returns. A change task
  starts with half of what its parent has left.
- **What is left** is the budget, less the task's own spend, less
  what its delegates settled, less what is reserved for them; never
  below zero.
- **Spent out:** a run that runs out of its turns, spend or time
  fails as a run fails: retried with a growing pause, then held, out
  of tries. A task without enough for its next run is held,
  `Insufficient activation budget`; one past its deadline is held,
  `Deadline passed`, which comes first when both hold.

## What temper guarantees

- **No run without its floor:** a run starts only with more than the
  floor to spend, never with more than the run cap, and only with its
  reserve: enough for its reserve turns and one more at its model's
  costliest.
- **Reservations count:** what is reserved for a delegate is not
  spent elsewhere while it runs.
- **A deadline is checked when a run starts,** never as a timer that
  ends a run in progress; a run starting at its deadline is admitted.
- **Release grants nothing:** a released task faces the same checks
  at its next run.

## Acting on it

- **ctl:** `task` shows a task's spend, its budget and its funding;
  `setting` reads the rules' run cap and floor and a charter's spend
  and turns; `decide` accepts a proposal that widens a task's
  authority; `amend` changes a deadline; `release` lifts a budget's
  or a deadline's hold once its cause is mended.
- **temper's runs** give a delegate its budget with `delegate`, raise
  it with `amend` within their own, and ask for more with `propose`.

## Limits

- temper gives no task an amount on demand: more comes from an
  accepted proposal or an amendment within its funder's authority.
- temper does not refill a task when settings change: a new run cap
  or charter spend applies to later runs.
- A project's period is not a week: temper has no weekly budget.
