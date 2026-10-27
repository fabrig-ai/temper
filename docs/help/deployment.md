# Deployments

Kind: concept

A deployment is one temper: its engine, the workers that run its agents, the forges it works on, and the three directories that hold its configuration, its data and its cache.

## What it is

- **The engine** holds the deployment's state and decides every act:
  ctl's calls, the person's client's, and those of temper's own runs.
- **A worker** checks repositories out, runs the agents' processes and
  carries their calls to the engine. `temper init` writes one worker,
  in the engine's process, with one slot.
- **`temper serve`** runs what the configuration's shape names: the
  engine and its workers in one process, the engine alone, or a worker
  alone, on another host.
- **A box** is a deployment on one machine: `temper box` runs the
  forge's setup steps, then `temper serve`, each a child process
  (temper help box).
- **A server** is a long-lived deployment: the engine behind a proxy,
  its workers on another host (temper help server).

**The three directories:**

- `CONFIG` holds the configuration: `temper.json` and `agent.json`,
  which `temper init` writes.
- `DATA` holds what the deployment keeps: its store, under
  `DATA/deployment`; the workers' workspaces; an owned Forgejo's data;
  the worker's secret; a box's logs, under `DATA/box/logs`.
- `CACHE` holds what can be fetched again: Forgejo and its runner, at
  the versions the binary pins.

Each comes from its flag (temper help commands), else its variable,
else its default: `TEMPER_CONFIG`, else `$XDG_CONFIG_HOME/temper` or
`$HOME/.config/temper`; `TEMPER_DATA`, else `$XDG_DATA_HOME/temper` or
`$HOME/.local/share/temper`; `TEMPER_CACHE`, else
`$XDG_CACHE_HOME/temper` or `$HOME/.cache/temper`. A relative path is
taken from the working directory. `DATA` and `CACHE`
belong to the user who runs temper, with no permission for group or
others; temper makes a missing one with mode 0700.

**The configuration document,** `CONFIG/temper.json`, has three
members:

- `engine`: its limits; the LLM providers and their models; the
  workers it admits and its listener; the owners; the ways to sign in;
  the first project and its policy;
- `workers`: each worker's engine address, name, secret file, slots
  and agent program;
- `application`: the forges; the landing rules and the forge's
  charter; the client and webhook listeners; the owned forge's setup;
  sign-in with Google; the playground; temper's own limits.

The settings (charters, templates, policies, rules) are not in it:
they live in the store and change while the deployment runs (temper
help settings).

## How it changes

- **Started:** `temper serve`, or `temper box`, reads the
  configuration and opens the store in `DATA`. With a store already
  there, the engine restores its tasks, accounts, parties, settings and
  forge records, and reads each forge afresh.
- **Stopped:** the first `SIGINT` or `SIGTERM` stops admitting calls,
  lets the engine settle its work and its writes to the forge, closes
  the store, and exits 0. A second signal aborts, and the exit is 1.
- **Restarted:** stop it, change `CONFIG/temper.json` if you must,
  start it again with the same three directories.

## What temper guarantees

- **A drained stop keeps everything:** a start with the same `DATA`
  resumes from it.
- **A run outlives a restart where its worker does:** the engine takes
  its claim back. A claim no worker reports within its grace is lost,
  and its task retries or holds as its charter says (temper help
  task).
- **ctl rides a restart:** a wait reconnects within the link's
  deadline, and a kept request is sent again under its key (temper
  help ctl).
- **A refused start changes nothing,** and its line names the member or
  file at fault, such as a secret file readable by others.

## Acting on it

- **The binary:** `temper init` writes a deployment; `temper box` runs
  one on this machine; `temper serve` runs one, or a worker alone;
  `temper program-login` signs a program in on this host. Each
  command's arguments are its row: temper help commands.
- **ctl:** `me` reads your party in the deployment; `settings show`
  reads how its work runs.

## Limits

- temper reads its configuration at a start alone: a change to the
  document takes effect at the next start.
- temper has no command that upgrades a deployment in place or prints
  its versions.
- temper has no ctl group `deployment`: `me` and `settings show` are
  what ctl reads of one.
- A box on fake backends keeps a deployment of its own under
  `DATA/box/fake-backends`, and refuses a `CONFIG` changed after
  `temper init`.
