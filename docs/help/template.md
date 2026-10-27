# Template

Kind: concept

How a new task of one shape is filled: who runs it, the result it gives, what it may spend, by when it ends, and how it is kept.

## What it is

A template is a setting, `template N`, numbered from 1. It is a recipe,
never an executor: a client fills it, with what its creator gives, into
a request for a new task, which temper checks as it checks any request.
A chat is filled from template 1 and a goal from template 2.

A template holds:

- its name, as clients show it;
- its executor: an agent under a charter it names, or the creator's
  choice;
- its result: a report, a verdict among choices, a connector's result
  such as a change, or an approval;
- the task's authority: its creator's role, as many levels of delegation
  shallower as it says;
- its budget: an amount, as much as the creator has left (with a cap or
  none), or the creator's to give;
- its deadline: none, a span from the task's making, or the creator's to
  give;
- whether it is tracked, with a default priority;
- its flags: whether the task is a conversation, whether it is listed
  once ended, how long it is kept, and what ends it;
- its label, a number that groups ended tasks in lists;
- the grace a chat's close gives its runs;
- the models the task asks for, or none for its charter's default.

Each member, with its type and bound, is in
`temper help settings-fields`.

## How it changes

A change of a template reaches the tasks made from it after the change.
A task keeps the values it was made with. A project's policy may
override, for its own tasks, a template's executor, budget, deadline and
default priority (`temper help policy`).

A removed template is no longer offered, and a change that leaves a
policy's override naming it is refused.

## What temper guarantees

- A template grants nothing: a task made from it is checked against its
  creator's authority, as any request is.
- A template's charter is offered, and its models are ones that charter
  offers, each with an effort the model takes.
- A task that its requester alone ends is a conversation, under a
  charter the template names.
- A template's grace is at most the deployment's limit for one.

## Acting on it

- `temper ctl`: `settings show template` reads one; `settings apply`
  and `settings propose` change it; `chat start` starts a chat from
  the project's chat template.
- `me` gives each project's templates, with its policy's overrides
  applied.
- A run whose charter and authority have the settings family reads
  templates with the host tool `settings`.

## Limits

- temper does not let ctl pick a template: `chat start` fills the chat
  template, and no ctl tool starts a goal.
- A template whose result is an approval is accepted, but an agent given
  one can only fail.
- A template with `shallower` 0 is accepted, but no task can be made
  from it.
