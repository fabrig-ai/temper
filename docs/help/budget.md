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
- **A project's period** funds all of its work: temper may spend up
  to N units in it, N the project's period spend. Its first period
  opens at the project's making, and each period lasts a week: at its
  next start the next opens, full, at the period spend its policy holds
  then. `project fund PROJECT AMOUNT` sets N, and resizes the open
  period with it at once.
- **Funding** comes from a person's pool, carved from the project's
  period up to their role's period spend, from an ancestor task, or
  from a recurring task.
- **A run** draws what its task has left, up to the deployment's run
  cap; its charter (temper help charter) sets its turns, its time and
  a reserve to finish in. The stock sizes its spends at each start
  from C, one costliest completion of the first configured model at
  its prices: its rules' floor is 2C and their run cap K, 4C; a
  person's chat may spend 24K, and the ceilings and a project's period
  48K. On `temper init`'s box C is 1,057 units, so a run draws 2,114 to
  4,228. The stock charters give 8 turns and 120 seconds, with a
  reserve of 1 turn and 60 seconds; `settings show` reads the current ones.

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
  of tries. A task without enough for its next run is held for its
  budget, one past its deadline for its deadline, which comes first
  when both hold; `task show` prints `Held`, and its record names the
  reason.
- **The period spent:** nothing more in the project is funded until
  the next period opens, or until it is raised. `project fund PROJECT AMOUNT` raises the project's
  period spend, and the open period's budget with it. A person's chats
  are funded from their pool, which their role's period spend sizes and
  which reserves from the period, so raise the role's next: `project
  fund PROJECT AMOUNT --role ROLE`. A cut keeps each budget at what it
  has used: nothing is taken back. A project made after a change of
  policy 1 starts with the new period spend.

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

- **ctl:** `task show` shows a task's spend, its budget and its
  funding; `me` prints what is left of your pool in each project;
  `project show` prints a project's open period and each pool in it,
  with what each has left; `project fund` raises the project's period
  spend or a role's; `settings show` reads the rules' run cap and floor, a charter's
  turns, and policy 1's period spend, which a new project takes;
  `settings apply` changes policy 1; `proposal decide` accepts a
  proposal that widens a task's authority; `task amend` changes a
  deadline; `task release` lifts a budget's or a deadline's hold once
  its cause is mended.
- **temper's runs** give a delegate its budget with `delegate`, raise
  it with `amend` within their own, and ask for more with `propose`.

## Limits

- temper gives no task an amount on demand: more comes from an
  accepted proposal or an amendment within its funder's authority.
- temper does not refill a task when settings change: a new run cap
  applies to later runs.
- A period lasts a week, from the project's first; the week is not a
  setting. An unspent period is not carried over, and what an old one
  reserved stays its own. A deployment stopped past a next start opens
  one period when it starts, from then: `temper help weekly-budget`.
- ctl changes a project's own policy in its spends alone (`project
  fund`); `settings show` reads policy 1, which only new projects
  copy.
