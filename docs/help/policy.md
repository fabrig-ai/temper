# Policy

Kind: concept

What work in a project may do and spend: a policy a project is made from, the project's own copy, and the deployment's rules above every project.

## What it is

Authority is what a task, a role or a ceiling may do: the engine's tool
families its runs may call, the models they may use, its grants (each a
connector's kind of effect or read on a pattern of resources), the
delegates it may make, how much its tree may spend and by when it must
end.

Three layers bound it, each within the one above:

- **The rules,** the setting `rules`: the deployment's ceiling, the most
  any project may have; the most a project may spend in a period; the
  least and the most a run is given; which kinds' grants grant other
  kinds; and the requirements every effect meets.
- **A project's policy:** its ceiling, the most any task in the project
  may have; its spend in a period; its roles; its escalation role; its
  requirements; the grants its goals' projections are written under; the
  role a forge collaborator's permission gives at adoption; and its
  overrides of templates.
- **A role:** what a party in it may give a task, may allot in a period,
  may ask and may decide.

A numbered policy, the setting `policy N`, is what a project made from
it starts with. The project keeps a copy, its own policy, which is not a
setting.

A requirement is what one kind of effect needs on some resources,
whatever any authority says: a connector's criterion, judged by that
connector. The forge's criteria are CI passed at the head, the head up
to date with its base, a gate, and approvals.

## How it changes

- A change of the rules reaches every decision after it. A narrowing
  reaches existing tasks: a grant beyond the new ceiling cannot be used.
  A widening never widens an existing task.
- A change of a numbered policy reaches the projects made from it after
  the change, never an existing project's own policy.
- A project's own policy is changed by the project's owners, within the
  rules.

## What temper guarantees

- No task exceeds its project's ceiling, and no project's policy exceeds
  the rules: a change that would is refused.
- A task's delegate fits under its creator: its authority within the
  creator's, with the creator's depth less one and what the creator has
  left to spend and to make.
- An effect is made only when a grant covers it and every requirement on
  it is met. A guarded requirement is checked by the forge as it makes
  the effect; an observed one is judged from facts read within its
  freshness.
- A run starts only with more than the rules' least run spend, and is
  given at most their most.
- Role 0 of a policy can create tasks, and an escalation role accepts
  proposals and decides escalations.

## Acting on it

- `temper ctl`: `settings show policy` and `settings show rules` read
  them; `settings apply` and `settings propose` change them.
- A project's own policy: `project show PROJECT` reads its spends, its
  open period and its pools; `project fund PROJECT AMOUNT` raises its
  period spend, and with `--role ROLE` a role's.
- A task's authority is in `task show`'s result; a proposal asks a holder for
  more than a task has (`temper help proposal`).

## Limits

- temper may spend, for a project, its period spend a week: each week
  a new period opens, full, at the spend its policy holds then
  (`temper help weekly-budget`). `project fund PROJECT AMOUNT` raises it, the open
  period's budget at once; with `--role ROLE` it raises a role's, which
  each of its parties' pools takes, and a role's raise the period has no
  room for is refused, naming what the period has left. When it is
  spent, nothing more in the project is funded until the next week, or
  until it is raised. A
  project made after a change of `policy 1` starts with the new period
  spend.
- temper does not change a project's own policy through settings; ctl
  changes its spends alone (`project fund`).
- temper does not check, when settings change, that a grant's kind, a
  requirement's criterion or a procedure's number is one the forge
  defines.
- The landing rules become requirements of the rules at each start, from
  the configuration: `temper help landing-rule`.
