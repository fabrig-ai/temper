# temper's commands

Kind: reference

Every command of the temper binary, by who runs it: its usage, what it does, its arguments and its notes.

## temper help

`temper help [NAME | --search WORDS...]`

Print the index of temper's pages, one page whole, or the sections that hold every word.

- `NAME`: A page's name, as the index and the pages give it.
- `WORDS`: Words that a section must hold every one of, case folded: at most 8, each at most 32 bytes.

Reads no directory and no configuration, so it works before temper init. A name that is no page, or a search that matches nothing, exits 1. A search prints at most 20 hits, a line each, NAME: HEADING: LINE, then "and more; narrow the words" when more matched.

## temper init

`temper init [DIRECTORIES]`

Write a deployment that starts, with temper's default configuration, into the three directories.

- `DIRECTORIES`: [--config CONFIG] [--data DATA] [--cache CACHE], anywhere among the arguments: the deployment's configuration, data and cache directories, each from its flag, else its variable, else its default.

Refused, changing nothing, when the configuration directory already holds temper.json.

## temper serve

`temper serve [DIRECTORIES]`

Run the deployment's services, as its configuration's shape names them, until stopped.

- `DIRECTORIES`: [--config CONFIG] [--data DATA] [--cache CACHE], anywhere among the arguments: the deployment's configuration, data and cache directories, each from its flag, else its variable, else its default.

Its stdout carries only sign-in links, one a line; its stderr, its startup notes and the reason of a failure. A first termination signal settles the engine's forge writes and drains it; a second aborts. Exit 0 after a drained stop, 1 on a refused start or any failure.

## temper box

`temper box [DIRECTORIES] [--echo none|steps]`

`temper box [DIRECTORIES] --fake-backends [--bundle BUNDLE]`

Run a deployment on this machine as one box: the forge's setup steps, then temper serve, each a child process.

- `DIRECTORIES`: [--config CONFIG] [--data DATA] [--cache CACHE], anywhere among the arguments: the deployment's configuration, data and cache directories, each from its flag, else its variable, else its default.
- `--echo`: none prints the box's own lines and a failed step's log; steps prints every step's lines too, prefixed by its name.
- `--fake-backends`: Run against temper's fake LLM provider, forge and OAuth issuer, as a deployment of its own under DATA/box/fake-backends that shares the cache.
- `BUNDLE`: The web client's bundle directory, in place of the configured one; a relative path is taken from the working directory.

Steps run in order, each restarted when it exits, and stop in reverse order. Its stdout is temper serve's sign-in links, each line whole; its stderr, what --echo chooses; each step logs under the data directory.

## temper program-login

`temper program-login [DIRECTORIES] NAME`

Sign a program source in on this host, through the program's own sign-in.

- `DIRECTORIES`: [--config CONFIG] [--data DATA] [--cache CACHE], anywhere among the arguments: the deployment's configuration, data and cache directories, each from its flag, else its variable, else its default.
- `NAME`: The program source's name: 1 to 32 of a to z, 0 to 9 and -.

Checks the configuration and the agent's, makes the source's directory, then replaces itself by the program's sign-in, whose exit status is then its own. The engine need not run.

## temper forge fetch

`temper forge fetch [DIRECTORIES] FORGE`

Download an owned forge's Forgejo and runner binaries into the cache.

- `DIRECTORIES`: [--config CONFIG] [--data DATA] [--cache CACHE], anywhere among the arguments: the deployment's configuration, data and cache directories, each from its flag, else its variable, else its default.
- `FORGE`: A forge's number in the configuration.

Fetches only the binaries the forge's setup gives as "fetched", at temper's pinned versions, for amd64 or arm64; another machine is refused, naming its architecture.

## temper forge prepare

`temper forge prepare [DIRECTORIES] FORGE`

Prepare an owned forge before its Forgejo first runs.

- `DIRECTORIES`: [--config CONFIG] [--data DATA] [--cache CACHE], anywhere among the arguments: the deployment's configuration, data and cache directories, each from its flag, else its variable, else its default.
- `FORGE`: A forge's number in the configuration.

## temper forge run

`temper forge run [DIRECTORIES] FORGE forgejo|runner`

Run an owned forge's Forgejo or its runner.

- `DIRECTORIES`: [--config CONFIG] [--data DATA] [--cache CACHE], anywhere among the arguments: the deployment's configuration, data and cache directories, each from its flag, else its variable, else its default.
- `FORGE`: A forge's number in the configuration.

Checks the binary and the preparation, and takes the forge's lock, first. Run alone, it replaces itself by the program, whose exit status is then its own; given the forge's listener, as the box gives it, it runs the program as its child.

## temper forge sign-in

`temper forge sign-in [DIRECTORIES] FORGE`

Make what people need to sign in to temper through a Forgejo, and print the Forgejo command that registers temper.

- `DIRECTORIES`: [--config CONFIG] [--data DATA] [--cache CACHE], anywhere among the arguments: the deployment's configuration, data and cache directories, each from its flag, else its variable, else its default.
- `FORGE`: A forge's number in the configuration.

For a Forgejo of application.sign_in.forges, on temper's host, before temper serve. Writes the provider's key and the forge's client secret where they are missing, private, and leaves any already there; prints Forgejo's add-oauth command, never running it.

## temper forge reconcile

`temper forge reconcile [DIRECTORIES] FORGE`

Reconcile a managed or connected Forgejo still in its old form. A transition: it goes once the forms it serves do.

- `DIRECTORIES`: [--config CONFIG] [--data DATA] [--cache CACHE], anywhere among the arguments: the deployment's configuration, data and cache directories, each from its flag, else its variable, else its default.
- `FORGE`: A forge's number in the configuration.

A forge that starts from its records is refused: temper serve takes its handover.

## temper ctl

`temper ctl DIR GROUP VERB [ARGUMENT...] [--json]`

`temper ctl DIR me|inbox|sign-in|sign-out|pending [ARGUMENT...] [--json]`

`temper ctl DIR --call FILE [--json]`

`temper ctl help [GROUP [VERB]]`

Act in a deployment as one party, one call a process: the agents' command line.

- `DIR`: ctl's private directory for one deployment and one party: its sign-in, its kept requests and the engine's address.
- `GROUP`: A group of tools, a noun of temper's or one of the client's own names; temper ctl help lists them.
- `VERB`: One of the group's tools; temper ctl help GROUP lists them.
- `FILE`: One call as JSON, {"tool": "GROUP VERB", "arguments": {...}}, or - for stdin.

temper ctl help prints how to work through it, then the groups.

## temper open

`temper open DIR LINK_OR_ADDRESS`

Open a person's terminal client on a deployment.

- `DIR`: The client's private directory, never one that ctl uses.
- `LINK_OR_ADDRESS`: A sign-in link temper serve printed, or the address of an engine the directory is signed in to.

For people; an agent works through temper ctl.

## temper internal agent

`temper internal agent AGENT.json`

Run one agent for a worker's run, on smith's agent configuration. Spawned by temper; never run by hand.

- `AGENT.json`: The agent's configuration, as the worker's agent.configuration names it.

## temper internal serve

`temper internal serve [DIRECTORIES] --box`

Run temper serve as the box's step, on the listeners and the pipe the box gives it. Spawned by temper; never run by hand.

- `DIRECTORIES`: [--config CONFIG] [--data DATA] [--cache CACHE], anywhere among the arguments: the deployment's configuration, data and cache directories, each from its flag, else its variable, else its default.

## temper internal forge-child

`temper internal forge-child --parent PID --signal kill|terminate -- PROGRAM ARGS...`

Run an owned forge's program, sent SIGNAL when its parent PID ends. Spawned by temper; never run by hand.

- `PID`: The process whose end ends the program.
- `PROGRAM`: The program to run, with its ARGS.

## temper fake llm

`temper fake llm [DIRECTORIES] [--ready PREFIX] [--seed N] [--observations FILE]`

`temper fake llm --document FILE [--ready PREFIX] [--seed N] [--observations FILE]`

Serve a fake LLM provider: the demo's, from the configuration, or one from a JSON document.

- `DIRECTORIES`: [--config CONFIG] [--data DATA] [--cache CACHE], anywhere among the arguments: the deployment's configuration, data and cache directories, each from its flag, else its variable, else its default.

## temper fake forge

`temper fake forge [DIRECTORIES] FORGE [--ready PREFIX] [--seed N] [--observations FILE]`

`temper fake forge --document FILE [--ready PREFIX] [--seed N] [--observations FILE]`

Serve a fake forge: forge FORGE of the configuration, or one from a JSON document.

- `DIRECTORIES`: [--config CONFIG] [--data DATA] [--cache CACHE], anywhere among the arguments: the deployment's configuration, data and cache directories, each from its flag, else its variable, else its default.
- `FORGE`: A forge's number in the configuration.

## temper fake forge check

`temper fake forge check [DIRECTORIES] FORGE OBSERVATIONS STORE`

Judge a fake forge's observations against a stopped deployment's store.

- `DIRECTORIES`: [--config CONFIG] [--data DATA] [--cache CACHE], anywhere among the arguments: the deployment's configuration, data and cache directories, each from its flag, else its variable, else its default.
- `FORGE`: A forge's number in the configuration.
- `OBSERVATIONS`: The observations file the fake forge wrote.
- `STORE`: A copy of the deployment's store.

## temper fake oauth

`temper fake oauth --document FILE [--ready PREFIX] [--seed N]`

Serve a fake OAuth issuer from a JSON document.

## temper fake git

`temper fake git ARGS...`

Run temper's fake git with git's arguments, as the demo's git does.
