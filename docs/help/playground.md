# The playground

Kind: concept

The playground is temper's demo: a repository on a fake forge and a scripted model, started with one call, so that a chat and its change run through real tasks with no account of yours.

## What it is

- **Its parts:** forge 1, a fake Forgejo, with the repository
  `playground/widget`; provider 3, a scripted model, with a key the
  deployment holds; project 1, which adopts the repository. `temper
  init` writes them all into the configuration; nothing runs until the
  playground starts.
- **What is real:** the engine, the worker and the agents' processes;
  the forge, its git and the model are fakes.
- **The scripted model** answers three requests, as typed: `Add CSV
  export to widget` delegates a change to the repository;
  `Add a dark theme to widget` and `Fix widget's empty search state`
  each explain that nothing changes. Any other request is answered that
  it is not one of these, and that other work needs a connected model.
- **Under a box,** the fakes run as one of its groups of child
  processes; elsewhere, the deployment must reach fakes of its own.

## How it changes

- **Started:** `playground start` starts the fakes, adds the model's
  key as a deployment account, waits for the forge, and adopts
  `playground/widget` into project 1. It prints `running` once the
  playground runs.
- **Running:** a chat in project 1 asks for one of the three requests
  (temper help chat). A start of the deployment starts the fakes again
  and keeps the account and the adoption.
- **Ended:** connecting another forge ends the playground for good: its
  fakes stop, its chats close, and its account goes; its tasks stay
  readable.

## What temper guarantees

- **One call, again until done:** a start that fails prints the step
  that failed, and the same call again finishes what is not done,
  never repeating what is.
- **No account of yours is used:** its model is the scripted one, on
  the deployment's own key.
- **Its work is real work:** its chats, tasks and change have the
  states, acts and reads of any other (temper help task).

## Acting on it

- **ctl:** `playground start` starts it, as a party with a role in the
  deployment; `chat start` in project 1 asks it for a change; `task
  show` and `change show` follow the work.
- `forge setup` can make the same repository, `playground/widget`, on
  the owned Forgejo of a box (temper help forge).

## Limits

- `refused: not offered` (exit 2): the deployment has no playground,
  or it ended.
- A failed start exits 1, naming its step: `the fakes did not start
  (REASON)`, `the account refused`, `the adoption refused` or `the
  forge not connected in time`; make the same call again.
- temper does not restart an ended playground.
- The scripted model does no work beyond its three requests.
