# Charters, templates and policies

Kind: concept

What decides how temper's agents work: a charter for each run, a template
for each new task, and the policies and rules that bound what work may do.

## Charters

A charter supplies instructions for an agent's run. It names the
models the run may use, its local tool families and the kinds of host
that can run it. It also supplies resuming and waiting behavior,
turn and time limits, a finishing reserve and default spend.

The task adds the particular brief, the required result, how it ends
and the spend admitted for this run. Whether work is an open chat or
a finite task comes from the task, not the charter's number. Read
`temper help tasks` for that distinction and how you follow the work.

The default chat charter is 1 and the default coding charter is 2.
Both default to 8 turns and 120 seconds, with a finishing reserve of
1 turn and 60 seconds. Their default waiting and call timeout are
10 seconds, with resuming enabled and default spend of 20 deployment
units. They use workers and enable inspection, modification and
agents; local shell use is off by default. Read the current values:

```sh
temper ctl me setting charter 1 --json
```

Here and below, `me` is your client directory. Use the charter number
for your work in place of 1. These are defaults, not a promise that
a run receives that much spend: `temper help budgets` explains how
its allowance depends on the task's funds and deployment limits.

## Models

A task can choose a main model and effort, and choices for subagents,
from the models its charter permits. With no named task choice, the
engine selects through the charter and available accounts. The
client calls this **Using the default model**.

Read the authorized model names for your project, here project 1:

```sh
temper ctl me models --project 1
```

A charter can name only configured models. Permitting a model in a
charter does not add it to the deployment's model catalogue. The
models available to a task must fit both the configured catalogue
and the charter's permitted choices.

## Templates

A template is a recipe the client fills to make a new task. It is
not another executor. It supplies the required result, executor
choice, budget and deadline selection, tracking, flags, label,
closing grace and default model choice.

The default chat template is 1. It fixes the executor to charter 1.
A new chat from the client or `temper ctl` uses the project's chat
template; the chat command has no template picker. Read its current
whole value:

```sh
temper ctl me setting template 1 --json
```

The default goal template is 2. It asks its creator for an executor,
budget and deadline, and defaults to tracked priority 1. Its model
choice is optional. Read the current value before relying on those
defaults:

```sh
temper ctl me setting template 2 --json
```

The goal template's existence does not give ctl a goal-creation
command. `temper help tasks` explains what you can do with goals
that already exist. `temper help budgets` explains the difference
between a task's budget rule and a run's limits.

## What work may do

A charter's local tool families are only part of what permits work.
Each tool call must fit the task's authority and the project and
deployment ceilings. A charter does not by itself give an agent
unrestricted permission to act on a repository.

A task's authority includes tool permissions, resource grants and
delegation permission. A resource grant names a connector, a kind
of action and a resource pattern. An exact ending names one
resource. An open ending covers a prefix and its descendants. Read
the grant as permission for that action on those resources, not
permission to act everywhere the connector can reach.

Delegation permission names the executors a task may use, a lifetime
count of descendants and a maximum depth. The count is not the
number of children running at once. Work down a tree must still fit
the authority and ceilings that bound it; starting another delegate
does not give that work a fresh unrestricted allowance.

In the client, **Budget and authority** shows **May spend** and
**May delegate**, including **Up to 8 tasks, 4 deep** when those are
the allowed values. The default deployment delegation ceiling allows
charters 1 and 2 and procedure 2, with 8 lifetime descendants and
depth 4. Read the current ceiling:

```sh
temper ctl me setting rules --json
```

A role or task can have narrower permission. Local shell use also
needs the charter's explicit shell permission, which is off by
default. Tool permissions and resource grants remain additional
controls; enabling shell does not replace them.

## Policies and rules

A project's own policy includes role permissions, spend, resource
grants, requirements, where escalations go and template overrides.
It is separate from a reusable numbered policy setting.

A numbered policy is copied when a later project is made. Changing
that numbered policy does not change existing projects. In
particular, policy 1 is not a way to read or change project 1's own
policy. Read the reusable policy, if your deployment offers it:

```sh
temper ctl me setting policy 1 --json
```

The deployment rules supply ceilings, period spend, per-run spend
floor and cap, and requirements. Read them with `setting rules` as
shown above. The setting kinds you can name are `charter N`,
`template N`, `policy N` and unnumbered `rules`; use a real setting
number in place of `N`.

## Changing them

Charters, templates, numbered policies and rules are settings. Use
`temper help settings` for whole-value changes, revisions and
proposals. An admitted stored change does not need a restart.

An agent task's later runs keep the charter from its recorded settings
revision. New tasks and new delegates use the newest revision. While
older work holds a revision, read that charter with `setting charter`
and `--revision`, as shown in `temper help settings`.

A changed charter must name configured models, leave turns and time
above its finishing reserve, and supply enough spend for that
reserve. A larger charter must still fit the host that will run it;
acceptance alone does not prove an existing host can satisfy a
larger request.

Save the current whole value with `setting ... --json` before editing
it. Setting JSON and the configuration document are different
formats: the setting contains inline instructions, model objects
and durations in nanoseconds. Never copy a value from the
configuration document into a settings change, or copy a setting
value back into that document. Follow the items-file construction
in `temper help settings`.

## What it cannot do yet

There is no page in the client to edit a charter, template or policy.
Use `temper ctl` to read and change settings.

There is no "Tell temper how you work" configuring chat. There is
also no separate charter for each repository change.
