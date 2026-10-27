# Budgets

Kind: guide

How temper counts what work spends, where a task's budget comes from,
what happens when it runs out, and what you can do then.

## What temper counts

A task's budget counts spend in deployment units. It can also carry a
wall deadline. Each run has separate ceilings for turns and spend,
and its own time limit. A task can wait, have delegates and take more
than one run, so a run's limits do not describe the whole task's
lifetime. Read `temper help tasks` for tasks, delegates and chats.

Model usage becomes spend through the model's prices. temper measures
fresh input, output, cache reads and cache writes. Reasoning counts
already belong to output and are not charged twice. The price rates
for those categories, with their denominator, turn token usage into
units of spend. A spend figure is an amount in those units, not a
count of tokens.

The client's spend numbers have no built-in currency label. Read
**Budget and authority** as permission and accounting for the task,
not as a statement of an invoiced amount.

## Where a task's budget comes from

A template chooses a new task's budget rule: an amount, as much as is
allowed, or an amount given by its creator. A delegate's authority
gives its allotment. The run then draws a capped share from the task.
A charter's spend (`temper help charters`) is a default and an
admission value; it does not
promise that every run gets that much.

Funding can come from a person's pool in a project period, an
ancestor task or a recurring task. Projects have separate periods
and pools. Changing a project's role budget affects a later pool;
it does not reset the old period's pool. A project-policy setting
copied into future projects is another limit, not a refill of an
existing project's pool.

Giving a delegate a budget reserves that amount against its funder.
While the reservation is outstanding, the funder cannot treat that
amount as available for other work. When the delegate settles, its
actual spend is counted below the funder and its full reservation is
removed. Any unspent part then becomes available through settlement.

What is left is the task's budget minus its direct spend, settled
spend below it and current reservations, with a minimum of zero. A
reservation therefore matters even before it becomes settled spend.
Reading just the parent's direct spend misses both settled delegate
spend and outstanding reservations.

## What one run may spend

A run gets at most the task's available amount and the deployment's
run cap. It must also have enough reserve and fit task and project
spend ceilings. The deployment's minimum is strict: the amount must
be greater than the floor, not equal to it.

The default floor is 1 unit and the default run cap is 10 units.
With those defaults, 1 unit left does not admit a run. A task allowed
40 units can still receive only a 10-unit run. Read the current
rules rather than assuming those defaults apply to your deployment:

```sh
temper ctl me settings show rules --json
```

A charter supplies turns, time, finishing reserve and default spend.
The default charter spend is 20 units, subject to the task's funds
and the deployment cap. Its default turn ceiling is 8 turns. Read
charter 1, or the charter used by your work, for its current values:

```sh
temper ctl me settings show charter 1 --json
```

A run protects a finishing reserve of turns and time. Ordinary work
can enter wind-down before the total ceiling so the run has room to
finish. Zero turns, spend or time refuses admission. The turn ceiling
belongs to one run; it is not a counter shared across all runs of
the task. The agent also has a hard turn ceiling, so raising a
charter's turns can require raising that ceiling too.

## Deadlines

A task deadline is checked against wall time when a run is admitted.
It is not a timer that promises to kill work already running at that
instant. A parked task becomes due just past its deadline and can
then be held for the expired deadline when admission is attempted.

The run's own time limit is separate. It limits that run, not the
whole task's elapsed time across waits, delegates and later runs.
Changing a task deadline does not give it more spend, and increasing
spend does not remove a deadline.

## Seeing what is spent

On a tracked task, **Budget and authority** shows direct spend and
what is left of the total. It also shows **Delegates spent**,
**Reserved for delegates**, **Funded by** and **Deadline**. A deadline
can say **None**. Follow these figures together when deciding what
is still available.

Funding labels include **Your pool**, **Person 2's pool**,
**Task 42's recurring budget** and **Project 1's budget**, with the
numbers for your work. **May spend** describes the permitted amount;
**May delegate** describes delegation permission. Use the task's
number to inspect its budget, tree and run:

```sh
temper ctl me task show 41 --tree --run
```

The chat header shows direct spend and its total. A chat row shows
direct spend too. Neither displayed direct-spend figure is an
all-inclusive total of the whole tree: settled delegate spend and
reserved delegate budgets are separate on the tracked-task card.
For example, a low direct-spend figure can coexist with a substantial
reservation for delegates.

## When a budget runs out

Running out of a run's turns, spend or time need not instantly end
the task. A budget failure from the agent is treated as a run failure,
with the task's retry and backoff policy, and eventually a hold when
its retry allowance is passed. An open chat normally parks on idle
or wind-down; a parked chat is not by itself a budget failure.

Insufficient spend for the next run can hold the task for Budget.
An expired deadline can hold it for Deadline. Read the hold's reason
and revision, fix the cause, then release it using that current hold
revision. Here the task is 41 and its hold revision is 2:

```sh
temper ctl me task release 41 --hold 2 --key release-budget-41
```

Release does not grant funds or a later deadline. The next run faces
the same checks, so releasing without resolving the cause does not
solve exhaustion. `temper help tasks` explains holds and the separate
choice required for an uncertain effect.

## Giving work more

You can accept a concrete proposal that widens a task's authority.
Review what the proposal permits before accepting it. An authorized
amendment can raise a live task's or delegate's spend when funding
and policy permit it; an agent with enough authority can amend its
delegate. The proposal path lets you decide an offered change, not
enter an arbitrary new allowance. For task 41's proposal 3:

```sh
temper ctl me proposal decide t41/3 accept --key allow-budget-3
```

You can also change settings (`temper help settings`) for later runs:
the deployment's run
cap or a charter's spend, for example. A raised run cap can let the
next run draw more from the task's existing allotment. A charter
change can change later run behavior. These settings changes do not
refill a particular task's allotment and do not extend the budget
already sent to a running agent.

## What it cannot do yet

There is no calendar-aware weekly budget recipe. Project periods
and pools exist, but a period amount does not by itself mean an
amount per week.

There is no amount you can type, in the client or with `temper ctl`, to
give a task more budget: you can accept a proposal that offers more, as
"Giving work more" says.

Spend has no currency. A model's prices turn its usage into units, and
temper does not say what a unit costs you.
