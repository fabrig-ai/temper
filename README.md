# temper

*Agent-driven software development on a forge.*

People decide what to build and why; agents do the work, from first
draft to landed change.

## The idea

- **People direct, agents develop.** People decide at the level of
  goals, designs and plans, and step in where agents cannot decide.
  Agents write, check, review and land the changes; feedback on code is
  their business.
- **Mechanics are code, judgement is an LLM's.** A change's way to
  landing is a procedure: checked by CI, reviewed by gates, repaired,
  brought up to date, landed through a queue per branch. An agent is
  woken only for what needs judgement: making a change, repairing a
  failure, reviewing, resolving a conflict, revising a plan.
- **A careful guest on the forge.** Repositories are adopted as they
  are. temper writes only to what it owns, never forces a push, and
  merges exactly the head its decision saw. A change it did not make to
  what it owns is drift: the work relying on it is held, and nothing is
  written over. Forgejo is the forge today.
- **Built from the family.** jig's engine runs the work, smith's agents
  do it under the charters temper writes and with the tools it serves,
  and skein carries the bytes, the forge's calls among them.
- **One binary, standalone first.** `temper serve` runs the engine and a
  worker on one person's machine, `temper open` is that person's
  terminal, and `temper forge` and `temper box` set up and run a forge
  beside it.
- **Tested as it runs.** Worlds with a fake forge and a fake LLM, then
  the real loop, then the binary against an owned Forgejo with its
  runner and real git, then a real model.

## Start here

- [`docs/design/README.md`](docs/design/README.md): what temper is, what
  it is built from, and where each part of its design lives.
- [`docs/guide/box.md`](docs/guide/box.md): a local temper box, with its
  own Forgejo, on loopback; also [trying the welcome with nothing
  real](docs/guide/box.md#trying-the-welcome-with-nothing-real).
- [`docs/guide/github.md`](docs/guide/github.md): GitHub connected from
  temper's client, through temper's own App.
- [`docs/guide/forgejo.md`](docs/guide/forgejo.md): a Forgejo connected
  from temper's client, managed, owned or connected.
- [`docs/guide/server.md`](docs/guide/server.md): temper on a server,
  behind a TLS proxy, with Google's sign-in and a worker on another
  host, redeployed from `main`.
- [`docs/design/STATUS.md`](docs/design/STATUS.md): how far the code is
  from the design.
- skein's foundation, which temper follows: `programming-model.md`,
  `testing-strategy.md` and `notes.md`.

Before changing code, read [`AGENTS.md`](AGENTS.md) and
[`docs/development/workflow.md`](docs/development/workflow.md). `main`
moves only through agents' `land`, which runs the gate,
`scripts/check.sh`.

## The family

These are the repositories of [Fabrig](https://github.com/fabrica-works), an
agent-driven software factory: write, prove, run.

| Repository | | Builds on |
|---|---|---|
| [skein](https://github.com/fabrica-works/skein) | One thread, one loop, no async | |
| [smith](https://github.com/fabrica-works/smith) | Protocol-driven agent loop for AI applications | skein |
| [jig](https://github.com/fabrica-works/jig) | Engine for agentic applications | skein, smith |
| [temper](https://github.com/fabrica-works/temper) | Agent-driven software development on a forge | skein, smith, jig |
| [hearth](https://github.com/fabrica-works/hearth) | Immutable machines, built from a revision | skein |

Commands that read the deployment use `<config>/temper.json` and accept
`--config DIR --data DIR --cache DIR` after their command words. Each flag
falls back to `TEMPER_CONFIG`, `TEMPER_DATA` or `TEMPER_CACHE`, then the
corresponding XDG directory under `temper`. Relative flags are joined to
the working directory once at startup. Data and cache must be private.

`temper init --config ./config --data ./data --cache ./cache` writes the
initial configuration, worker secret and charters. It refuses existing
`temper.json`, `worker.secret` or `worker.digest` files.

After starting temper, open the accounts page and add an OpenAI API key.
The initial configuration uses OpenAI's public Responses API. ChatGPT
sign-in will be available later.
