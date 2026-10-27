# Charter

Kind: concept

How an agent's run works: the instructions its agent reads, the models it may use, its tools in the workspace, the hosts that may run it, and its limits.

## What it is

A charter is a setting, `charter N`, numbered from 1. An agent task runs
under one charter, which its template or its creator names. Each run of
the task reads its charter, whole:

- its instructions, which the run's agent reads before its task's brief;
- its models: `model`, the default, then `alternatives`, in order;
- its tools in the workspace: to inspect, to modify, to run a shell, to
  open sub-agents;
- the hosts that may run it: the deployment's workers, the engine, or
  both;
- a run's limits: its turns and time, a reserve of each for finishing,
  how long it waits idle, and the timeout of its calls to temper's
  tools.

The forge's procedures run a change's agent work under two charters,
which the change keeps from its making to its end: its producing and
repairs under the forge's charter, the one the configuration's
`application.charter` names, `charter 2` as `temper init` writes it;
its reviews under the reviewer's charter, `charter 4`, which reads the
change and gives its verdict, and edits, pushes and runs nothing.

## How it changes

A change of a charter reaches the tasks made after it. A task made
before it keeps reading the charter of its own revision until it moves
to the newest (`temper help revision`). A run already going keeps the
instructions it started with.

A removed charter is no longer offered: no new task may name it, and a
change that leaves a template or a delegation naming it is refused.
Tasks made before keep it, on their revision.

## What temper guarantees

- A task names no model a charter does not offer. With none named, a
  task takes, when it is made, the first of the charter's models with an
  account that can pay for it, else `model`; its runs keep the model it
  took.
- A charter's models are the deployment's: each must equal a configured
  model in every member but its effort, and its effort must be one the
  model takes.
- A run has more turns and time than its reserve, and keeps the reserve
  to finish and report.
- A charter's tools in the workspace are its own: a sub-agent has no
  more than its run. What a run may do beyond its workspace (its
  effects, its reads of the forge, its delegates) is its task's
  authority, within the project's policy and the rules
  (`temper help policy`).

## Acting on it

- `temper ctl`: `settings show` reads one, as the newest revision or a
  held one has it; `settings apply` and `settings propose` change it.

## Limits

- temper does not give a charter's `spend` to its runs: a run is given
  what its task has left, at most the rules' `maximum_run_spend`. A
  charter's spend must still hold its reserve.
- temper does not hold which charter the forge uses as a setting: naming
  another in `application.charter` needs a restart. The charter it names
  is a setting like any other.
- temper does not check, when a charter changes, that a connected host
  can run it.
- A charter's instructions and models are within the deployment's bytes
  for a run; past them, the change is refused.
