# The protocol layer

Provisional, 2026-10-03, revised 2026-10-26. temper's protocol layer:
how temper composes jig's protocol layer with components of its own (its
forge's, its checkout's, its client's) into the processes of one binary.
jig owns the layer's shape (jig's `protocol/README.md`) and the services'
(jig's `protocol/service.md`); this document says what temper adds and
the crates it is made of. The mechanics are those of skein's
`programming-model.md` (sections 2, 4 and 9); the entities it carries are
those of `../domain/`. Each boundary of temper's own is designed in a
document of its own (section 2). What is open across them is listed in
section 14. The cluster, temper as several processes of one machine for
its tests and its lab, is section 15.

## 1. In one page

- **Translation, and nothing else.** The protocol layer turns bytes into
  the domains' entities and back, and decides nothing about the domain
  (programming-model.md, section 4). The domain decides deadlines,
  retries and what a failure means; the protocol layer runs the call, its
  timer and its one attempt.
- **jig's protocol layer, composed.** jig ships the components for its
  own vocabularies: the store, the workers' channel, LLM accounts,
  clients, configuration (jig's `protocol/README.md`, section 5).
  temper's services own them as children beside smith's, skein's and
  temper's own components, and route between them, io and the roots, as
  jig's reference service does (jig's `protocol/service.md`, section 2).
- **What temper adds:** the forge's protocol (skein's forge component,
  with its webhooks' listener, handed temper's values and routed to the
  root and the forge connector); the checkout's io
  (the git component); its parts' codecs on jig's channel; its families
  in jig's store; its git identities; its host tools' translation; its
  client (its wire, its client root and view on `jig-client`, its
  terminal); the clients' listener, on which it mounts jig's clients'
  component and serves its forge routes; the setup host, skein's, in the
  engine (forge.md, 2.6); and the forge's setup's commands outside the
  services (setup.md). Each is a crate of its own (section 5).
- **Foreign protocols are stacks of skein's machines.** Forgejo's REST
  API and GitHub's are skein's forge kit, each a dialect of it, over
  skein's HTTP client and JSON (skein's `forge.md`); git's arguments in and text out are temper's decoders
  over io's processes. temper's records on jig's channel, in jig's store
  and on the client's wire are generated from schemas by skein's
  generator.
- **Standalone first.** `temper serve` runs the engine and one worker in
  one process, on one loop, joined by jig's in-process address; the
  worker's agents are child processes of temper's own binary (`temper
  internal agent`); a person's client reaches the engine on loopback;
  the forge is polled, and hinted by its webhooks on loopback when they
  are configured (section 8; forge.md, section 5). Hosts apart,
  webhooks from another host, the forge as a sign-in provider and the
  forge's client documents come after (section 14). The browser is a
  person's client on loopback too (`../client/browser.md`).
- **The cluster, for tests and the lab.** The same `temper serve`, on a
  document of another shape, is an engine alone or a worker alone. One
  engine alone and its workers alone, each a process of its own on one
  machine, joined over loopback in plaintext, with the fakes as
  processes beside them, are the cluster that temper's process tier and
  the QA lab both run, from one description (section 15). It is no
  deployment's shape: a box keeps its one worker inside `temper serve`.
- **One binary:** `serve`, `internal agent`, `open`, `program-login`,
  the forge's setup's `forge prepare`, `forge run` and `box`, and the
  agents' `ctl`, which `../client/tools.md` designs (section 4).
- **No retries below the domain.** A call goes out once and ends once,
  with the failure the domain's vocabulary defines (section 5).
- **The forge's load follows its changes.** The calls a pass makes grow
  with what changed on the forge, never with its history, its labels or
  the items temper holds, and the worlds count them by HTTP request
  (forge.md, section 6).
- **Secrets never reach a domain.** A domain names a credential by number
  and generation; values live in the protocol layer's tables and in jig's
  secrets' store: LLM accounts' as jig keeps them, and each forge's
  records under temper's tag, from which its access and its git identity
  are made (section 10; credentials.md).
- **Tested at every tier:** codecs and parsers against golden bytes and
  transcripts of real peers; the forge component and the git component in
  worlds of their own; temper's client joined to the engine's root; the
  one-process world on skein's simulator; the real binary with an owned
  Forgejo, its runner and real git; a live tier with a real model
  (section 13; `../domain/testing.md`).

## 2. Reading order

1. **jig's `protocol/README.md` and `protocol/service.md`:** the layer
   temper composes, and the services temper copies.
2. **README.md:** this overview.
3. **configuration.md:** what `temper serve` and its other commands are
   told at start.
4. **credentials.md:** the secrets temper's processes hold: LLM accounts
   as jig's, the forges' records in jig's secrets' store, the forges'
   access, git identities, and the values clients' requests carry.
5. **channel.md:** temper's parts on jig's channel, and a git identity's
   value.
6. **store.md:** temper's families in jig's store.
7. **llm.md:** where LLM calls are made, host tools, the charter and a
   run's result.
8. **git.md:** the checkout's io: the git component.
9. **forge.md:** the forge's protocol layer on skein's forge kit: what
   temper hands the forge component, the setup host, its marks, its
   webhooks' address, and what it relies on of Forgejo and GitHub.
10. **setup.md:** the forge's setup: connected, managed and owned, the
    setup host in the engine, and `temper box`.
11. **sign-in.md:** people's sign-in with Google on a deployment's
    public address: its members, routes, endings and binding.
12. **`../client/README.md`:** temper's client: its client root on `jig-client`,
    its pages, the forge's client family, its view, the terminal, and the
    clients' listener of temper's server; then **`../client/tools.md`**, `temper
    ctl`, the agents' tools.
13. **`../domain/testing.md`:** the tiers below the domain.
14. **`../client/browser.md`:** the browser, the second shell of
    temper's client.
15. **documentation.md:** the user documentation compiled into the
    binary, `temper help`, and agents' `read_docs`.
16. **tooling.md:** the contract with a project's own `prepare` and
    `key` commands, run as jobs on a worker for a landing by turn.

## 3. Names

jig's names keep their meaning here (jig's `protocol/README.md`, section
3; jig's `domain/README.md`, section 4): the hub, the core, the fleet,
the worker's link, credential grant, an application's credential,
payload version, profile, the clients' component, the default HTTP
mapping, the reference service (`jig-test-service`), the reference
binary (`jig-test`).

| Name | What it is |
|---|---|
| standalone | one process on one person's machine: `temper serve` with the engine and one worker joined in process (section 8) |
| engine alone | `temper serve` on a document with an engine and no worker, its workers' listener on loopback (15.1) |
| worker alone | `temper serve` on a document with one worker and no engine, dialling an engine alone on loopback (15.1) |
| the cluster | one engine alone and its workers alone on one machine, with the fakes as processes, as temper's process tier and the QA lab run it (section 15) |
| the cluster's description | `lab/cluster/`: the cluster's processes and their documents in the lab's experiment format, read by the lab's runner and by the process tier's (15.5) |
| the engine's service, the worker's service | each `iterate` with its root and protocol layer, in `temper-service` (sections 6 and 7) |
| the routing | the protocol layer's code between io, components and the root (jig's `protocol/service.md`, section 2) |
| the forge component | skein's `skein-forge-connection` (forge.md; skein's `forge-connection.md`) |
| the git component | `temper-worker-protocol-git` (git.md) |
| temper's parts | the workspace items, the saved-work branch, the work, the preparation (channel.md) |
| payload version, store version, client version | temper's three own versions: `PAYLOAD_VERSION`, `STORE_VERSION`, `CLIENT_VERSION` (section 9) |
| temper's families | the forge connector's records in jig's main store (store.md) |
| a forge's records | its credentials, ids and accepted installations: rows of jig's secrets' store under temper's tag (credentials.md, 2.1) |
| git identity | one of temper's application credentials, one per forge: Forgejo's git token, or a GitHub App's installations' tokens, with a login and an author (credentials.md, section 4) |
| the identities' table | the protocol layer's table of git identities' values, on the engine and on each worker |
| a forge's access | what the forge component authenticates with: Forgejo's API token, or a GitHub App's id and key and its installations' tokens; the component's only (credentials.md, section 5) |
| the setup host | skein's in-process host for the setup's plans, a child of the engine's protocol layer (forge.md, 2.6) |
| the forge's client family | temper's client side of the forge connector: its words, and the adoption form (`../client/README.md`, section 4) |
| the terminal shell | `temper-client-shell-terminal` (`../client/README.md`, section 6) |
| the browser shell | `temper-client-shell-web` (`../client/browser.md`, 2.1) |
| the bundle | the browser shell's module, glue and stylesheets, skein's bundler's output, which the binary holds and the clients' listener serves (`../client/browser.md`, 2.2 and 2.6) |
| the tools | the agents' interface to temper, `temper ctl`: `temper-client-tools` and `temper-client-shell-cli` (`../client/tools.md`, section 4) |
| the clients' listener | `temper-service`'s listener for clients, on which jig's clients' component is mounted with jig's default HTTP mapping, under `/jig` (`../client/README.md`, section 8) |
| the fake git program | git as the worker runs it, faked: its core `crates/temper-fake-git`, `temper fake git`, and its hosted half `testing/temper-fake-git-hosted` (`../domain/testing.md`, 4.3) |
| the throwaway Forgejo | a Forgejo started by a test script on loopback, with SQLite in a scratch directory, removed after (`../domain/testing.md`, 2.6) |
| locate | the forge connector's read of a repository's id by owner and name (skein's `forge.md`, 5.2) |
| the webhooks' listener | the forge component's own listener for Forgejo's signed deliveries, plaintext on loopback, one path per forge (skein's `forge-connection.md`, section 6; forge.md, section 5) |

The forge's setup's names are setup.md's: connected, managed and owned
(section 2), the forge's directory (section 3), reconcile, a plan of the
setup host's (section 5),
the pin (6.1), the preparation (6.3), the box and its steps (section 7).

## 4. The processes and the binary

One binary, `temper` (`crates/temper`), ordinary Rust: a shell, copied
from jig's reference binary. Its commands, sorted by who runs them
(`../draft/agent-docs.md`, 7.1), `temper ctl`'s as `../client/tools.md`
designs them:

```
documentation  temper help [NAME | --search WORDS...]
deployment     temper init [DIRECTORIES]
               temper serve [DIRECTORIES]
               temper box [DIRECTORIES] [--echo none|steps] [--fake-backends [--bundle DIR]]
               temper versions
               temper upgrade --check DIRECTORY | --back
               temper program-login [DIRECTORIES] NAME
               temper forge fetch|prepare|sign-in [DIRECTORIES] FORGE
               temper forge run [DIRECTORIES] FORGE forgejo|runner
clients        temper ctl DIRECTORY GROUP VERB [ARGUMENT...] [--json]
               temper ctl DIRECTORY --call FILE [--json]
               temper ctl help [GROUP [VERB]]
               temper open DIRECTORY LINK_OR_ADDRESS
spawned        temper internal agent AGENT.json
               temper internal serve [DIRECTORIES] --box
               temper internal forge-child --parent PID --signal kill|terminate -- PROGRAM ARGS...
test doubles   temper fake llm|forge|oauth|git ...
```

- **Every command is a row of the binary's table,** as ctl's tools are
  (`../client/tools.md`, 5.1): its words, its usage, its line, its
  arguments with a line each, and its notes. The parser, `temper
  COMMAND --help`, `temper --help` (the rows by who runs them, a line
  each) and the reference page `commands` derive from it.
- **What temper spawns itself** is under `temper internal`, absent from
  `temper --help`'s first groups and from every page but `commands`; a
  person or an agent never runs it. `temper forge reconcile`, the
  managed and connected forms' transition, is a row marked so, until it
  goes (setup.md, 6.5).

`DIRECTORIES` is `[--config DIR] [--data DIR] [--cache DIR]`, given
anywhere among the command's arguments, each with its variable and its
default; the document is `temper.json` in the configuration directory
(configuration.md, section 12).

- **`temper serve`:** reads the configuration (configuration.md,
  section 2) and runs the services its document's shape names, in one
  process on skein's `Host` and `drive` (jig's `protocol/service.md`,
  sections 4 and 10): standalone, the engine's and the worker's, the
  store in the deployment's directory; an engine alone, the engine's
  with its store and no worker; a worker alone, the worker's, with no
  store (15.1).
  - Its stdout carries only sign-in links, one per line: each the
    secret jig's clients' component gives, in jig's default mapping's
    link route under temper's prefix (jig's `protocol/client.md`,
    section 8; `../client/README.md`, section 7); its stderr carries startup
    notes, the lines of 15.3 (each TCP listener once bound, and a worker
    alone's link at each opening) and the reason of a failure. A worker
    alone prints no sign-in link.
  - `--box`, given by `temper box` alone, as `temper internal serve
    --box`, adopts the box's pipe, the two fds after the listeners the
    box gives it (setup.md, 7.1). Given listeners, `LISTEN_FDS` of them
    from fd 3, its clients' and webhooks' listeners take them by their
    addresses (skein's `shell.md`, 6.6).
  - A first termination signal settles the engine's forge writes, then
    drains it (`../domain/root.md`, 9.2; jig's `protocol/service.md`,
    section 5), and a worker alone's runs its worker's ending (jig's
    `protocol/service.md`, section 6); a second turns either into an
    abort, which keeps the reason of a failure that started the drain
    (a lost webhooks listener's, forge.md, 2.3). A write the settling
    left without its outcome is printed as the drained engine ends, one
    line each, and the exit is still 0:
    `temper serve: stopped before task {task}'s forge write {entry} settled; the next start settles it`.
  - A start refused because its patches would reshape a write in flight
    (jig's `protocol/store.md`, 10.3, `InFlight`) prints jig's words,
    then a line for each write its keys name, from temper's `Flight`
    key, at most 64, then how many more, and exits 1:
    `temper serve: task {task}'s forge write {entry} is still in flight; start the build before this one and stop it to settle it`;
    `temper serve: and {n} more forge writes in flight`.
- **`temper init`:** writes a deployment that starts, the default
  document among it, into the three directories, refusing when the
  configuration directory holds a document (setup.md, 8.2).
- **`temper internal agent AGENT.json`:** smith's agent shell,
  `smith_agent_shell::run`, on smith's agent configuration
  (configuration.md, section 5). The worker's protocol layer spawns it,
  one process per run; a person never does.
- **`temper open DIRECTORY LINK_OR_ADDRESS`:** a person's terminal client
  (`../client/README.md`, 6.1). It reads no configuration file.
- **`temper ctl DIRECTORY ...`:** the agents' tools, one call per
  process, over the same native transport as `temper open`
  (`../client/tools.md`, section 8). It reads no configuration file.
- **`temper versions`:** what this binary speaks and keeps, one line
  each: every owner's patch level (none before the first release), its
  families' and the store's versions, and its wires, as jig's store
  keeps them (jig's `protocol/store.md`, 5.3). It reads no
  configuration and no store.
- **`temper upgrade --check DIRECTORY`:** a rehearsal (jig's
  `protocol/store.md`, 10.5) over DIRECTORY, a stopped deployment's
  data directory or a copy of one such as last night's backup, which it
  never writes: it works in a scratch copy under the cache directory,
  with the configuration `temper serve` would read; the open patches
  and checks as a start would, restores, and stops before its switch
  step. It prints a line per patch with its counts and time, the time
  of the whole, and the refusal a start would meet, in a start's words,
  or that the patches and the restores pass, saying that what follows
  the switch is not rehearsed; then whether the workers may stay: yes
  when the copy's wires equal this binary's and no pending patch is
  marked fresh, no saying which differs, unknown when the copy keeps
  none (`draft/upgrades.md`, sections 3.1 and 9). Exit 0 when the
  patches and restores pass, 1 when a start would be refused there.
- **`temper upgrade --back`:** makes the previous generation active
  again (jig's `protocol/store.md`, 10.5), for a binary that patched and
  failed before its first commit of its own: the deployment stopped,
  and the old binary started after it. Refused, changing nothing, once
  the patched generation has committed, with no previous generation, or
  while a process holds the deployment, each in words that say so; past
  the first commit, going back is restoring a backup
  (`draft/upgrades.md`, 3.2).
- **A start that patches** prints a line per patch as it ends:
  `temper serve: patched {owner} {patch}: {records} records in {commits} commits`.
- **`temper program-login NAME`:** a program's login for
  the program source NAME, on this host: it checks the document and the
  agent's configuration, makes the source's configuration directory, and
  replaces itself by the program's own sign-in (credentials.md, section
  3). The engine need not run.
- **The forge's setup** (setup.md), each command reading and checking
  the whole document as `temper serve` does (configuration.md, section
  8), FORGE a forge's `number`; reconcile is the engine setup host's
  plan (forge.md, 2.6), except for managed and connected old forms'
  transitional `temper forge reconcile` until temper-15 (setup.md, 6.5):
  - `temper forge prepare`: an owned forge's preparation, before its
    Forgejo runs; it ends when done (setup.md, 6.3);
  - `temper forge run`: checks the binary and the preparation, takes
    the forge's lock, and replaces itself by `forgejo web` or
    `forgejo-runner daemon`, whose process a host supervises (setup.md,
    6.4);
  - `temper box`: the configuration's steps as child processes, in
    order, restarted when they exit and stopped in reverse order; its
    stdout is `temper serve`'s sign-in links, each line whole; its
    stderr, a terminal's, the box's own lines alone, each step's lines
    in its log under the data directory, and on stderr too, prefixed by
    the step's name, with `--echo steps` (setup.md, 7.2).
- **Exit status:** 0 after a drained shutdown, a worker alone's ending
  or a finished command; 1 on
  any failure (a refused start, a usage error, a failed commit, an
  abort, a failed step of the setup, the box giving up), the reason on
  stderr. smith's agent shell exits 0 or 1 the same way; `temper ctl`'s
  statuses are `../client/tools.md`, 6.3; `temper forge run`'s is Forgejo's or
  the runner's own, and `temper program-login`'s the program's sign-in's
  once it has replaced itself.

Standalone, one person's machine:

```
temper serve                           one process, one loop: temper-service's system module
├── the engine's service               section 6
│   ├── listens: the in-process name "workers"; the clients' listener and, when configured, the webhooks' listener, TCP on loopback
│   └── connects: each configured forge's API (HTTP; TLS unless the forge is on loopback)
├── the worker's service               section 7
│   └── dials: the in-process name "workers"
├── temper internal agent AGENT.json   a child process per run, in a process group of its own
└── git ...                              a child process per git invocation, in a process group of its own (git.md, section 7)
temper open DIRECTORY LINK_OR_ADDRESS  a process of its own, reaching the engine on loopback
temper ctl DIRECTORY GROUP VERB ...    a process per call, reaching the engine on loopback
temper program-login NAME              run by hand on a host, replaced by the program's sign-in
temper versions                        prints this binary's levels, versions and wires
temper upgrade --check DIRECTORY       rehearses an upgrade over a copy; switches nothing
temper upgrade --back                  makes the previous generation active again, before the patched one's first commit
```

A box runs `temper serve` as one of its steps, after the forge's setup's
commands, each a child of its own (setup.md, 7.1). The cluster's
processes are 15.2's.

## 5. Components and crates

**jig's,** composed as they are (jig's `protocol/README.md`, section 5):

| Crate | In | What temper uses it for |
|---|---|---|
| `jig-protocol-config` | engine, worker | jig's parts of the configuration (configuration.md, section 2) |
| `jig-protocol-store` | engine | the store, temper's families beside jig's in the same commits (store.md) |
| `jig-protocol-worker-wire` | engine, worker | the channel's records; temper's parts ride in them as bytes (channel.md) |
| `jig-protocol-engine-workers` | engine | the worker's link, the engine's side: opening, the payload version, grants' values filled in |
| `jig-protocol-worker-link` | worker | the worker's link, the worker's side: dialling, grants' values taken out |
| `jig-protocol-accounts` | engine | LLM accounts on skein's accounts driver: signed-in records and keys from jig's secrets' store, a loopback listener for each sign-in's redirect, the access tokens and keys lent (credentials.md, section 3) |
| `jig-client-protocol-wire` | engine, client | jig's records for clients; the forge's client family's records ride in them as bytes (`../client/README.md`) |
| `jig-protocol-engine-clients` | engine | the clients' component: jig's messages with their credentials, the printed link's secret, sign-ins, credentials (`../client/README.md`, section 8) |
| `jig-protocol-engine-clients-http` | engine | jig's default HTTP mapping, its engine end, with its example guards, mounted on temper's clients' listener under `/jig` |
| `jig-client` | client | the client temper's client root holds: the link, watches by handle, the request table, kept work, families; jig's helpers (`../client/README.md`, section 3) |
| `jig-client-protocol-link` | client | `jig-client`'s half toward its transport: jig's messages and the small store's records as bytes (jig's `protocol/client.md`, 4.3) |
| `jig-client-protocol-http` | client | jig's default HTTP mapping, its client end, under `jig-client-shell-native` and `jig-client-shell-web` |
| `jig-client-shell-web` | client | the browser shell's adapter over skein's fetch, and the printed link's sign-in part (`../client/browser.md`, 2.1) |
| `jig-protocol-transcript` | client | turns' bytes, decoded below `jig-client` |
| `jig-client-shell-native` | client | `temper open`'s and `temper ctl`'s transport: HTTP on io, the credential `jig-client` holds sent as a bearer, the small store's file in its directory |

temper does not use `jig-protocol-charter` or `jig-protocol-tools`: its
root takes its own `Tool` vocabulary, translated by `temper-engine-smith`
(llm.md, sections 4 and 5). Nor does it use jig's reference UI
(`jig-client-view`, `jig-client-test-shell-web`, nor the look of
`jig-client-shell-web`): temper's pages are its own,
on skein's UI kit (`../client/README.md`, section 5).

**smith's:** `smith-host-domain`, the worker root's agent child;
`smith-host-protocol`, smith's host half, one channel per agent process
over its pipes (smith's `protocol/hosts.md`, sections 2 and 3);
`smith-protocol-channel`, `smith-channel`, `smith-charter` and
`smith-transcript`, smith's codecs, which the routing calls to decode and
encode smith's payloads; `smith-agent-shell`, the `agent` command; and
`smith-local-service`'s profile, from which `temper serve` reads the
agent's host limits at start, as jig's reference binary does.

**skein's:** `skein-lib` and `skein-io` everywhere, lib's HMAC-SHA256
checking the deliveries' signatures (skein's `lib.md`, 6.2), io's
processes under the box; `skein-shell`, the binary's startup kit
(`read_file`, `read_private_file`, seeds, `Host`, `drive`);
`skein-json`, configuration and the forges' documents; `skein-http`, the
forge component's client and its webhooks' listener's server, the setup
host's client, and the fake forge's server; the forge kit,
`skein-forge` (Forgejo's and GitHub's documents and call plans), `skein-forge-client`,
`skein-forge-connection` (the forge component) and `skein-forge-setup`
(the setup host and its plans), with the fake forge and the throwaway Forgejo in its worlds
(skein's `forge.md`, section 2);
`skein-tls`, a forge's TLS; `skein-codec` and its generator, every codec
crate; `skein-kv`, under jig's store and temper's key encoders;
`skein-channel`, under jig's channel; `skein-oauth-accounts`, under
jig's accounts; `skein-ui`, under temper's view, and
`skein-ui-text`, the terminal's text shell. In the worlds: skein's
simulator, fake machine, fake LLM peer, fake issuer and fake person.

**temper's own,** each step code under the subset but the binary, the
terminal's host, `temper ctl`'s host, `temper-forge-setup-shell`, the
box's host and the fake git program's core:

| Crate | Kind | Holds | Depends on |
|---|---|---|---|
| `crates/temper-service` | services | the engine's and the worker's services and their one-process composition, in an engine, a worker and a system module, as jig's reference service is: each `iterate`, the routing, the clients' listener with jig's clients' component and its default HTTP mapping mounted, temper's forge routes, and the browser's page and bundle served (`../client/browser.md`, 2.2), startup from configuration's bytes, start and end; temper's profile `small` | the roots, `temper-engine-smith`, jig's components, smith's host half and codecs, temper's codec crates and components, `skein-forge-connection`, `skein-io`, `skein-json` |
| `crates/temper` | binary | the commands of section 4, copied from jig's reference binary; for each that reads the document, `temper-service`'s startup, with `setup` and the setup's and the box's limits checked by their own crates (configuration.md, section 2) | `temper-service`, `skein-shell`, `smith-agent-shell`, `smith-local-service`'s profile, `skein-oauth-accounts`, `jig-client-shell-native`, the client crates, `temper-forge-setup-shell`, `temper-box`, `temper-client-bundle` |
| `crates/temper-forge-setup` | step code | the forge's setup, but for reconcile, which is skein's (`skein-forge-setup`): its types and checks, and the setup mapped onto reconcile's given values, with the kit's check of them; the preparation's machine; `app.ini` and the runner's files; the processes' specs; the configuration's steps, `steps` and `Step`. Names no host | setup.md, section 11 |
| `crates/temper-forge-setup-shell` | hosts | ordinary Rust: the hosts of `temper forge prepare` and `run`: commands, files, the forge's locks and the exec (setup.md, 6.4); HTTP on skein-http over skein-io | setup.md, section 11 |
| `crates/temper-box` | machine, host | the box's machine, step code, and its host, ordinary Rust, on skein-io's processes; nothing of the setup but its steps (setup.md, 7.3) | setup.md, section 11 |
| `crates/temper-fake-git` | program | the fake git program's core, ordinary Rust, which the binary runs as `temper fake git`: its checks, answers, step core, replay and two remotes (`../domain/testing.md`, 4.3) | setup.md, section 11 |
| `crates/temper-worker-protocol-git` | component | the git component: each `git::Op` of `temper-worker-domain-checkout` as a fixed sequence of git invocations, their arguments, environment, output and failures; the operations in flight, their processes, pipes and deadlines; `Make` as file operations (git.md) | `skein-lib`, `skein-io`, the checkout's vocabulary |
| `crates/temper-protocol-worker-wire` | codecs | temper's parts on jig's channel, and a git identity's value, from a schema; `PAYLOAD_VERSION` (channel.md) | `skein-lib`, `skein-codec` |
| `crates/temper-protocol-store` | codecs, keys | temper's families: key encoders, codecs from a schema, `STORE_VERSION`, the restart's interval (store.md) | `skein-lib`, `skein-codec`, `skein-kv`, `jig-protocol-store`, the forge's records |
| `crates/temper-client-protocol-wire` | codecs | the forge's client family's records: the adoption request and its outcome, the small store's record of a pending one; `FORGE`, the forge connector's number, 1; `Owner`, temper's owners; `CLIENT_VERSION` (`../client/README.md`, 4.6) | `skein-lib`, `skein-codec` |
| `crates/temper-engine-smith` | translation | smith's start from an assignment, host calls decoded into the root's `Tool` vocabulary, answers rendered, results and turns read; called by the engine's routing, keeping no state (llm.md) | the root's and its forge children's vocabularies, jig's and smith's domain vocabularies, `skein-lib`, `skein-json` |
| `crates/temper-client-domain` | client root | `jig-client` held, temper's pages, the forge's client side (`../client/README.md`, section 3) | `../client/README.md`, section 2 |
| `crates/temper-client-view` | view | temper's pages as skein-ui's tree, with the forge's view (`../client/README.md`, section 5) | `../client/README.md`, section 2 |
| `crates/temper-client-shell-terminal` | shell | the terminal: its host ordinary Rust, over skein's text shell and jig's native transport (`../client/README.md`, section 6) | `../client/README.md`, section 2 |
| `crates/temper-client-shell-web` | shell | the browser: a `cdylib` for wasm32, ordinary Rust, over skein's browser parts and jig's browser adapter (`../client/browser.md`, 2.1) | `../client/README.md`, section 2 |
| `crates/temper-client-bundle` | data | the browser's bundle in the binary: its build script builds `temper-client-shell-web` for wasm32 and skein's bundler in a nested cargo, runs the bundler and lists the bundle's files; its library includes them, or nothing under `TEMPER_BUNDLE=none` (`../client/browser.md`, 2.6) | nothing; its script, `std` alone |
| `crates/temper-client-tools` | client root, rendering | the agents' tools: the table, the runner, marks and conditions, the rendering, the tools' root (`../client/tools.md`, section 4) | `../client/tools.md`, section 4 |
| `crates/temper-client-shell-cli` | shell | `temper ctl`: its host ordinary Rust, one call per process over jig's native transport (`../client/tools.md`, section 8) | `../client/tools.md`, section 4 |

**temper's test crates, worlds and tiers** are `../domain/testing.md`'s
(sections 2, 4 and 7): the fake git program, the worlds below the
domain, the process and live tiers, and the tool that records git's
transcripts (git.md, section 9). The fake forge's protocol layer, the
throwaway Forgejo and the tool that records Forgejo's transcripts are
skein's (`fake-forge.md`, sections 4, 6 and 7).

**What temper's components share** is what every component of jig's
does (jig's `protocol/README.md`, section 1; programming-model.md,
sections 4 and 9): each a child of the protocol layer with its own
limits, worst case, deadlines and most outputs per entry point; its
connections or invocations its own entities, bound to calls by echoed
tokens; one exchange in flight per connection, with an output cap and a
room check before the next is parsed; a call with no room refused before
anything is set aside for it; each call's deadline the domain's, and
each connection's or invocation's its own, armed from its limits. Two
rules every temper component keeps:

- **No retries below the domain.** A call goes out once and ends once,
  with the failure the domain's vocabulary defines: for the forge,
  unavailable only when the call provably never left, and timed out
  otherwise (skein's `forge.md`, 5.4). A retry below the domain hides its cost from
  the budget, and adds load just when the peer is struggling.
- **Limits and the worst case.** Each component exports its `Limits` and
  `worst_case`, summed with jig's at start (configuration.md, section 7).

## 6. The engine's service

In the shape of jig's `protocol/service.md`, section 2:

```
temper-service, engine module           iterate: io ↔ components ↔ root
├── temper-engine-domain                the root, with the journal (../domain/root.md)
└── the protocol layer
    ├── jig-protocol-store              commits, loads, transcripts, secrets; temper's families (store.md)
    ├── jig-protocol-engine-workers     the worker's link; temper's parts as bytes (channel.md)
    ├── jig-protocol-accounts           LLM accounts (credentials.md, section 3)
    │   └── skein-oauth-accounts
    ├── the clients' listener           temper's: TCP on loopback (../client/README.md, section 8)
    │   └── jig-protocol-engine-clients-http   jig's default HTTP mapping, under /jig, with its example guards
    ├── jig-protocol-engine-clients     the clients' component: jig's messages; the forge's client family as bytes (../client/README.md, section 8)
    ├── skein-forge-connection          the forge component: every configured forge's API; the webhooks' listener, TCP on loopback (forge.md)
    ├── the setup host                  skein-forge-setup's in-process host: the setup's plans (forge.md, 2.6)
    ├── the identities' table           git identities' values (credentials.md, section 4)
    └── timers                          the root's and every component's deadlines
```

Starting, ending, `iterate`'s passes, room and timers are jig's
(jig's `protocol/service.md`, sections 3 to 5). What temper adds:

- **The forge component,** speaking each forge's API, Forgejo's REST API
  under `/api/v1` or GitHub's in skein's dialect, polled at the domain's
  cadences (skein's `forge.md`, section 5). The root's
  `Request::Forge { call, repository, op }` goes to it; it answers each
  call with exactly one `client::Event::Answered { call, cost, result }`,
  which the routing hands the root as a forge event
  (`../domain/root.md`, 5.1). An event the root cannot take yet comes
  back to the component, which holds it and offers it again (jig's
  `protocol/service.md`, section 2, room); one the root returns during
  its drain stays with the component until it closes (forge.md, section
  2; `../domain/root.md`, 9.2). Each forge's access is the component's
  alone (credentials.md, section 5).
- **A forge's start check** (credentials.md, section 5; skein's
  `forge-connection.md`, section 3): when a forge refuses its access, or
  it answers as a user other than the forge's own, that forge stops,
  never the engine: the component gives `Stopped { forge, why }`, the
  deployment's page names it with Replace (`../client/forge.md`, 5.4),
  and its calls are answered `NotConnected`; the engine and every other
  forge run on (forge.md, 2.3). Records that do not fit the configured
  forge leave it not connected, named the same way (credentials.md,
  2.1). A failure of jig's secrets' store itself stops the engine
  (credentials.md, section 5).
- **The setup host,** skein's in-process host component, beside the
  forge component: the routing runs its plans (reconcile at each start
  of a managed or owned Forgejo, a connect's, an adoption's access) and
  carries what passes between it, the root and the forge's records. It
  holds an admin's token or a GitHub App's key only while a plan runs
  (forge.md, 2.6; setup.md, the setup host in the engine). On close,
  the setup adapter retains closing requested separately from host
  `Close` sent; it drains host output independently of active notes,
  store, forge and events gates, sends `Close` once bounded host room
  permits, and completes only on actual `Closed` after io settlement
  (setup.md, 5.1 and 13.1). No application note-pop order is required;
  active-phase gates and skein's host/caller semantics remain in force.
- **The webhooks' listener,** the forge component's own, when
  `application.webhooks` names it: each delivery verified as it streams,
  answered, then made a hint, which the routing hands the root as a forge
  event (forge.md, sections 2 and 5). A hint is never a fact: polling
  finds what a lost delivery told of (forge.md, section 6).
- **temper's parts.** As it encodes an assignment, the routing encodes
  the root's `ForgeWorkspace` as temper's workspace items, with
  `temper-protocol-worker-wire`, making each repository's remote from its
  forge's `git` base and the repository's owner and name
  (configuration.md, section 3), with the owner's account id on a GitHub
  forge, so workers need no forge configuration.
  As it decodes an answer, it decodes the work and the preparation into
  the root's `Parts` (channel.md, sections 4, 5 and 7;
  `../domain/root.md`, 7.4).
- **temper's families.** The root's forge records go to the store under
  jig's application tag, encoded by `temper-protocol-store`, in the same
  commits as jig's; the forge's restart step loads them as one interval
  (store.md).
- **The identities' table,** filled from each Forgejo forge's records at
  start and from each GitHub forge's minted installation tokens as they
  come, keeping each identity's two newest generations; the routing puts
  each value in the table, then gives the root one `Event::Credential`
  for it, before its restart for a Forgejo forge's, and
  `jig-protocol-engine-workers` fills grants' values from the table
  through the routing (credentials.md, section 4; `../domain/root.md`,
  7.1).
- **Host tools, the charter and results through `temper-engine-smith`,**
  called in the routing, with smith's codecs, in the step that routes
  them, keeping no state (llm.md, sections 4 and 5; `../domain/root.md`,
  5.2).
- **The clients' listener.** The service owns it, and mounts jig's
  clients' component on it with jig's default HTTP mapping under `/jig`,
  with the mapping's example guards (`../client/README.md`, section 8; jig's
  `protocol/service.md`, section 2). A path jig's component does not claim
  goes to temper's forge routes, `/forges/...`, a GitHub connect's
  browser steps, each a setup host's run (`../client/forge.md`, 9.1), then
  to the browser's page and bundle; every other path, and a `GET` of the
  printed link's route, is answered not found.
- **The client's application wire.** The forge's client family's
  requests are decoded by `temper-client-protocol-wire` in the routing
  and answered by the root; every other part of the forge's is answered
  as unknown (`../client/README.md`, sections 4 and 8). An adoption names a
  forge, an owner and a name; the routing completes it with the forge's
  host, the host of its configured `git` base with `:port` when it names
  one (configuration.md, section 3), and hands the root the party's keyed
  request; the root locates the repository, then makes jig's adoption
  request on the party's behalf (`../domain/root.md`, 5.4;
  `../domain/forge.md`, section 4).
- **Periods,** from the smallest source, as jig's reference service gives
  them (configuration.md, section 8).

## 7. The worker's service

In the shape of jig's `protocol/service.md`, section 6:

```
temper-service, worker module           iterate
├── temper-worker-domain                the worker's root: jig's hub, smith's host domain, the checkout
└── the protocol layer
    ├── jig-protocol-worker-link        the link to the engine; temper's parts as bytes (channel.md)
    ├── smith-host-protocol             one channel per agent process, over its pipes; LLM grants filled into its start
    ├── agent processes                 temper internal agent, each in a process group of its own
    ├── the identities' table           git identities' values, from grants (credentials.md, section 4)
    └── temper-worker-protocol-git      the git component: the checkout's io (git.md)
```

No store; starting, ending and smith's payloads are jig's (jig's
`protocol/service.md`, section 6). What temper adds:

- **Agents run as `temper internal agent`:** the agent program is
  temper's own binary, with the arguments configuration gives
  (configuration.md, section 5), its environment holding no credential.
- **temper's parts.** The routing decodes the workspace items into the
  root's `wire::Workspace` before the root sees the assignment, and
  encodes the root's `wire::Work` and `wire::Preparation` beside an
  answer (channel.md, sections 4, 5 and 7).
- **Credentials.** `jig-protocol-worker-link` takes grants' values out:
  LLM grants go to smith's host half in its form, a credential's value
  with its scheme or a program source's name (jig's `protocol/llm.md`,
  section 6); git identities' go into the identities' table,
  from which the routing hands the git component an identity's value
  with each operation that names it; the component never reads the table
  (git.md, sections 2 and 5).
- **The checkout's io.** Each `git::Op` the root asks for goes to the git
  component, which answers it with exactly one terminal; `Make` is file
  operations under the worker's root directory (git.md, section 4).
- **Local channel refusals** use jig's public `u16` constants
  `REFUSED_LIMITS = 2` and `REFUSED_FRAMING = 3` from
  `jig-protocol-worker-link`, including diagnostic matches (jig's
  `protocol/channel.md`, 10.1; skein's `channel.md`, 5.1). These are
  channel reasons, separate from jig's application reasons at 256 and
  above. Existing worker diagnostic tests check those matches; the
  wire and `Closed` facts keep their values.
- **Starting:** configuration, the components, the worker's root
  directory opened, then the link dialled at once (jig's
  `protocol/service.md`, section 6).

## 8. One process

Standalone is jig's one process (jig's `protocol/service.md`, section 10):

- **One service, one loop.** `temper-service`'s system module owns the
  engine's protocol layer and the worker's as children, each with its own
  root and journal, sharing io and nothing else; nothing crosses between
  them but bytes.
- **Joined by the in-process name `workers`:** the engine's workers'
  listener and the worker's link (configuration.md, section 2). The
  link's opening, authentication, liveness, relays and grants run as
  between hosts, in plaintext, with no TLS.
- **A client on loopback,** over TCP to the clients' listener: `temper
  open`, signing in by the link `temper serve` prints at start.
- **Agents and git as child processes,** each in a process group of its
  own, which a stop signals whole (jig's `protocol/README.md`, section
  10); contained process trees are for hosts apart (section 14).
- **The forge outside,** reached over HTTP, and over TLS when it is not
  on loopback; its deliveries reach the webhooks' listener on loopback,
  so a forge on another host polls alone (forge.md, section 5).
- **The core knows its workers are in process:** the engine's workers'
  listener is the in-process name, from which jig's configuration tells
  the core so (jig's `protocol/configuration.md`, section 3;
  `../domain/root.md`, section 2).
- **Starting and ending together:** configuration for both, then the
  engine's start, then the worker's. A signal drains the engine; once it
  has drained, the service ends the worker at once, its agent processes'
  and git invocations' groups killed as io closes them. No worker
  outlives the engine, so the next start settles the worker's claims at
  once (jig's `domain/engine.md`, section 6; `../domain/root.md`, 9.1).
- **Its world** is `tests/one-process` (`../domain/testing.md`, 2.5).

## 9. Versions

jig's rules hold (jig's `protocol/README.md`, section 7): one version at a
time before release, a reader that refuses another and says so, and an
upgrade that stops the engine and its workers together. jig's versions
(its channel's, its store's format and families', its clients' per
owner) and smith's (its charter's, transcript's and result's) are as
jig's `protocol/README.md`, section 7 and smith's `protocol/README.md`,
section 8 give them. temper's sit beside them:

| Version | Owner | Covers | Checked |
|---|---|---|---|
| `PAYLOAD_VERSION` | `temper-protocol-worker-wire` | temper's parts: the workspace items, the saved-work branch, the work, the preparation; a git identity's value; and a delivery's evidence, smith's record as the smith temper links encodes it | the worker's credential at the link's opening: another is refused as *application* (channel.md, section 3) |
| `STORE_VERSION` | `temper-protocol-store` | temper's families | at the store's opening: a store written by another refuses to open (store.md, section 4) |
| `CLIENT_VERSION` | `temper-client-protocol-wire` | the forge's client family's records | per owner, as jig's: the engine serves exactly its own, and `temper open`, from the same binary, speaks it (`../client/README.md`, 4.6) |
| keys' markers | the forge connector | the keys temper writes into forge bodies | the key's version, its first byte; the marker has none of its own (forge.md, section 4) |
| configuration | none | — | unknown members refuse the start (configuration.md, section 2) |

- **The baseline is the first release** (skein's `codec.md`, section 5).
  Until it, each of temper's three versions keeps its current number.
  A change a reader would notice changes the schema, its generated code
  and its one set of golden bytes in place, including a repin of smith
  that changes a delivery's record (channel.md, section 3). Versions move
  at a release, not at a landing. A store from an older build is started
  afresh when its families change: its unchanged version does not tell
  the layouts apart. A reader still refuses another version.
- **Both channel components are given `PAYLOAD_VERSION`** when they are
  made; neither knows what it covers.
- **The store keeps the wires** the last start spoke (jig's
  `protocol/store.md`, 5.3, `Wires`): temper's shell hands jig's store
  `PAYLOAD_VERSION` as `temper-payload` beside jig's and smith's, so the
  next binary's rehearsal can say whether the workers may stay
  (`draft/upgrades.md`, section 9).
- **What the first release freezes.** From it, every byte of temper's a
  deployment keeps must stay readable by every later build
  (`draft/upgrades.md`, 4.1, 6.3, 10 and 11). Before it, in place, each
  in one change with its golden bytes:
  - **The forges' secrets from a schema:** temper's family
    `temper-secrets` (`temper-protocol-records`, `schema/secrets-v1.schema`),
    a versioned record per row of jig's secrets' store under temper's tag
    (credentials.md, 2.1): an enum of records, one per item (the three
    tokens and the webhook's secret each a `secret N`; the user and the
    app's id `u64`s, big-endian as every schema's; the app's slug a
    `text N`; its key a `secret N` holding its DER; the accepted
    installations a list; the origin its kind and address). The
    hand-written envelope and its little-endian numbers go. temper's set
    names the family at the baseline, `map: Secrets`, its keys temper's
    numbers below 2^63 (jig's `protocol/store.md`, section 9). A pasted
    bundle is a carried value, jig's from 2^63 up, kept only until the
    request that carries it is used (credentials.md, 2.2): its form is
    `temper-secrets`' `Pasted`, versioned, and one of another version is
    refused by name and pasted again, never patched.
  - **temper's store says what it holds** (store.md, section 4): a
    retained refusal `holds jig-store-people Refusal`; each list of
    answers' items, comments, reviews, statuses, remarks and files its
    `pieces`; each list of raw names (a branch's segments, protection
    contexts, labels, branch names) a declared item type and its
    `pieces`, its bytes moving in place where they must.
  - **An effect's key is an identity, never patched:** kept as written,
    in the store and in the forge's markers, compared byte for byte.
    Its first byte is its version (skein's `forge-client.md`, 7.1); from
    the first release the connector reads every version it has written,
    and a purpose's tag keeps its meaning for good, a new meaning taking
    a new tag (forge.md, section 4). The marker's wrapper and the bot's
    marker are fixed forms.
  - **Small files** (`draft/upgrades.md`, section 10): the forge's setup
    `state`, versioned JSON, is read in every version from the first
    release on and written in the current at its next write; the
    preparation writes it before the engine starts, so it follows the
    forge's own upgrade, whose going back is its dump (setup.md, 6.3).
    The worker's secret and digest (hex digits and a newline), the demo
    model's key, the forge's sign-in client secret (setup.md, 6.8), the
    box's started markers (an empty file each) and the admin token's
    handover are fixed forms, never versioned: a change is a new file
    name, read beside the old one for as long as a release may find it.
    The engine writes no small file before its start's wires are durable
    (jig's `protocol/store.md`, 5.3), so going back finds the engine's
    as they were; the preparation's `state` and an owned Forgejo's data
    are the forge's, and going back past a preparation that migrated it
    needs its dump (`draft/upgrades.md`, 3.2 and 11).
  - **Golden bytes** pin each fixed form, each schema's records and the
    key's frame and marker, so a change fails a test naming it.

  Outside it: what an external program reads (Forgejo's and the
  runner's files, which their own upgrade carries), the fake backends'
  sandbox, the standalone reconcile's files until it goes (setup.md,
  6.5), logs, caches and worker roots.
- **The baseline is v0.1.0's store** (`draft/v0.1.0.md`, section 6): its
  disk format (skein-kv's frames and snapshot, the transcripts'
  framing, the deployment's layout) every later build reads as it finds
  it, and its records every later build patches by itself as it starts
  (`draft/upgrades.md`, section 3). So the tag's commit makes, in one
  set across skein, smith, jig and temper:
  - **every stored family's text kept:** each owner's crate run once
    with `--baseline` (skein's `codec.md`, 12.1), its current texts its
    first kept versions, its level 0, no patch: skein-oauth's
    `skein-oauth`; smith-transcript's turn family; jig's store families
    (the core, tasks, people, notes, accounts, settings, the component's
    and `jig-store-secrets`) and `jig-accounts`; temper's `temper-store`
    and `temper-secrets`. No version moves at it;
  - **the real patch sets:** each owner's `PatchSet` from its crate's
    generated module, its families with their maps, key ranges, roots,
    the frames' families, smith's fresh word and temper's `Flight`
    range, handed by `temper-service`'s store shell in place of
    `PatchSets::NONE` (jig's `protocol/store.md`, 10.3). A store v0.1.0
    makes records every owner at level 0; one from a build before it,
    level none, is refused as older, made again (jig's
    `protocol/store.md`, 5.3);
  - **the gate's check from then on:** each repository's gate runs the
    patch command's check over each owner crate (skein's `codec.md`,
    12.7), so a stored change with no patch, an edited kept text or a
    landed patch changed fails it;
  - **the baseline's golden store,** made by the tag's binary from a
    story (a project, a person and an account, a goal's change landed,
    a chat paused, a task mid-run, a forge's records and secrets): the
    deployment's data directory whole, its store, secrets and
    transcripts, with its configuration and small files, kept for good
    beside the one-process world (`../domain/testing.md`, 2.5), never
    regenerated. Every later build starts on it and asserts what draft
    13 asks: identities and counters, policies, kept outcomes, old turns
    read in the current version (`draft/upgrades.md`, section 13).
- **Before the tag:** every stored record with its kept form, a
  delivery's evidence among them (jig's `protocol/store.md`, section
  16, still open); the golden store's story and its later builds'
  assertion, as a test; the rehearsal below. The patch command's
  modules and the sets built from them exist only once `--baseline` has
  run (skein's `codec.md`, 12.1 and 12.3), and its check reports every
  family unkept until then (12.7), so the sets, their handing and the
  gates' check are the tag's set's, with the kept texts.
- **The baseline rehearsed before the tag:** that set made on a branch
  that never lands, from the day's schemas, its sets handed to the
  one-process world over a store a story made: the plan accepts every
  real set (at most 64 families, names at most 64 bytes, key ranges
  disjoint and each family's own, holds acyclic, one frames' family per
  slot, the manifest within its bound), a start at level 0 opens, a
  start of the build before is refused as older, and nothing is
  patched. What it finds is fixed on `main` before the tag, so the
  tag's set only repeats it.

## 10. Secrets

The secrets temper's processes hold, where each comes from, who holds it
and what it reaches are credentials.md's (section 2); the files they are
read from are configuration.md's (section 6). Across the layer:

- **Names in the domain, values beside it** (jig's `protocol/README.md`,
  section 8): no domain state, fact, record of the main store or log
  holds a value. Values move between components through the routing,
  never through a root.
- **temper's forge secrets are in jig's secrets' store:** each forge's
  records (its credentials, ids and accepted installations) under
  temper's tag, read whole at start and written only by the engine's
  protocol layer (credentials.md, 2.1), and the values clients' requests
  carry, kept by jig until their run ends (credentials.md, 2.2). The
  files read at start are configuration.md's (section 6).
- **Nothing crosses a host** in standalone: the link is in process, the
  client and the webhooks on loopback, and a forge off the machine is
  reached over TLS only (configuration.md, section 3).
- **The forge's setup's secrets:** the engine's setup host holds an
  admin's token or a GitHub App's key only while a plan runs (setup.md,
  the setup host in the engine; credentials.md, section 5). The one setup
  file `temper serve` reads is an owned forge's handover `admin_token`,
  read once and removed after its `TakeOver` is done (forge.md, 2.6;
  configuration.md, section 6).

## 11. What temper uses from jig, smith and skein

temper's protocol layer uses jig, smith and skein as they are, at the
revisions temper's `Cargo.lock` pins.

- **jig:** the components of section 5; the reference service and binary,
  copied; `jig-client` and its native transport; jig's client test kit,
  its check, faults and attack stories, for temper's client tiers; the
  conformance kit, as the domain's worlds use it.
- **smith:** the agent shell, the host domain and host half, the codecs;
  the agent process over skein's LLM client, and its program backend,
  Claude Code (smith's `protocol/programs.md`); smith's fake `claude`, in
  the worlds.
- **skein:** io with its in-process names, processes and process groups;
  the startup kit and its private reads; lib's SHA-256 and its HMAC;
  JSON; HTTP's client and server; TLS's client; the codec generator; the
  key-value store; the framed channel; the accounts driver, under jig's
  accounts; the UI kit and its text shell; the simulator, the fake
  machine, the fake LLM peer, the fake issuer and the fake person.
- **What is temper's own:** its presentation, since presentation is the
  application's (jig's `client/domain/client.md`), on skein's UI kit and
  text shell (`../client/README.md`, sections 5 and 6); and, where something
  would be better upstream, as jig's design allows, the fake git
  program, on skein's fake checkout and fake forge; and `temper program-login`, the
  application's command a program source's holder runs on each host
  (jig's `domain/accounts.md`, section 5).
- **Not used:** `jig-protocol-charter` and `jig-protocol-tools` (llm.md);
  agents in a host's own process (`smith-inline-agent`; section 14);
  jig's reference UI (section 5).

## 13. Testing

`../domain/testing.md` applies skein's strategy (testing-strategy.md) to
temper: its tiers, fakes, layout and budgets. What checks this layer:

- **Step tests:** every codec against golden bytes, at its bounds and by
  fuzz targets; each translation both ways; git's argument and
  environment builders, and its output parsers against transcripts of
  real git (`tools/git-transcripts`); `skein-forge`'s decoders against
  transcripts of the throwaway Forgejo (skein's conformance,
  `fake-forge.md`, section 7);
  temper's pages as skein's text shell prints them (`../client/README.md`,
  section 10).
- **Machine worlds:** the forge component against a scripted HTTP peer
  (skein's `forge-connection.md`, section 10); the git component against
  scripted process ends and outputs; the terminal's host against a
  scripted stdin and stdout.
- **The forge's protocol world,** `tests/engine/protocol-forge`: the
  forge connector world's stories through bytes, over in-memory streams
  cut at random, with the cost checks by HTTP request (`../domain/testing.md`,
  2.2).
- **The client's joined world,** `tests/client`: temper's client root
  and view, with skein's fake person and jig's check, joined to temper's
  engine root (`../domain/testing.md`, 2.1 and 6.3).
- **The one-process world,** `tests/one-process`: `temper-service` on
  skein's simulator, killed and restarted at drawn moments, drained by a
  shutdown. Its referee: nothing a party, the worker or the forge saw
  depended on a lost commit; a drained shutdown ends with nothing
  waiting for a commit; no secret's value in a domain's state, a store
  record or an agent process's environment (`../domain/testing.md`, 2.5).
- **Reconcile's world** is skein's: reconcile against the fake forge's
  protocol layer, serving the setup's routes (skein's `forge-setup.md`,
  section 7; setup.md, 13.2).
- **The process tier and the live tier:** the real binary end to end,
  outside the two suites: a chat answered, a change landed on an owned
  Forgejo with CI and webhooks, the binary killed and restarted, `temper
  box`, `temper program-login` over smith's fake `claude`, the cluster
  with a task on each of two workers alone and a worker lost (15.6);
  then a real model (`../domain/testing.md`, 2.6 and 2.7; setup.md,
  13.4).

## 14. Open questions

Each document keeps its own; these cross them:

- **Names while running.** io connects to addresses, resolved at
  startup, while a forge's address may move behind its CDN, and a name
  that does not resolve at the start leaves its forge unreachable for
  that run, not the start refused (setup.md, 8.3). Default: a restart
  resolves names again; skein's DNS client, or skein-forge-connection
  resolving at its connect, would mend it (a skein addition).
- **HTTP/1.1 only.** Forgejo serves it; a forge that required HTTP/2
  would need skein to grow it. Default: HTTP/1.1.
- **Generating the routing** (jig's `protocol/service.md`, section 9).
  Default: copied from jig's reference service and filled.
- **Supervision** (jig's `protocol/service.md`, section 9): what
  restarts `temper serve` after a failed commit or a crash. Default:
  nothing of `temper serve`'s own; a box restarts it (setup.md, 7.2),
  and otherwise the person or the machine's
  service manager starts it again, and it recovers from the last
  durable commit.
- **Agents in the worker's process** (`smith-inline-agent`), with no
  containment, for one person's machine (jig's `protocol/service.md`,
  section 10). Default: agent processes, as `temper internal agent`.
- **After standalone,** each designed where it is named, and needed only
  beyond one person's machine: webhooks from another host, over TLS
  (forge.md, section 5); hosts apart: TLS on the engine's listeners,
  contained process trees, a worker alone on another host (jig's
  `protocol/README.md`, section 10; on one machine it is the cluster's,
  section 15); the forge as people's sign-in
  provider (credentials.md, section 8); the forge's client documents and
  panels (`../client/forge.md`); and what the forge's setup leaves for later
  (setup.md, section 15). Default: standalone.
- **An engine restarted under live workers.** The engine announces its
  identities' values at generation 1 at each start, sound only while no
  worker outlives it (credentials.md, section 4). Default: the cluster
  restarts whole, its workers stopped before its engine (15.4);
  generations kept across starts come with hosts apart (credentials.md,
  section 8).
- **`temper init` for the cluster's shapes.** Default: none; `temper
  init` writes standalone, and the cluster's documents are its
  description's templates (15.5). Hosts apart would want it.
- **Limits across processes.** Each process checks its own document
  (configuration.md, section 7); what holds only between an engine's
  limits and a worker's, the cluster's one template holds, writing the
  same `application.limits` into each of its documents, and jig's link
  holds the rest at its opening, by its terms and the hello's bounds
  (15.3). Default: so; a digest of the limits at the
  link's opening, beside the payload's version, comes with hosts apart.
- **The description's Rust reader's home.** Default: temper's process
  tier's support (15.5); skein-world's, once a second product runs a
  cluster of its own.

## 15. The cluster

The cluster is temper on several processes of one machine: one engine
alone, its workers alone, and the fakes, each a process of its own,
joined over loopback. It is a primitive of temper's tests and of the QA
lab, the retained reference-case tests' shape on fakes: an engine and its
workers, one of them added later (../domain/testing.md, 5.3;
`../draft/reference-case.md` is the second edition). It is no
deployment's: `temper init` writes standalone, and a box keeps its one
worker inside `temper serve` (setup.md, 7.1). TLS, containment and other
hosts are later (15.7).

### 15.1 Its shapes

`temper serve` takes its shape from its document's members, and from
nothing else: no flag, no command of its own.

| Shape | `engine` | `workers` | `application` |
|---|---|---|---|
| standalone | jig's engine part; `hosts.listener` `[{"in_process": "workers"}]` | one, whose `engine` is `{"in_process": "workers"}` | every member; `workers`, one entry |
| engine alone | jig's engine part; `hosts.listener` one socket address on loopback, in plaintext, its port 0 or fixed; `hosts.workers` the cluster's workers, each with its digest | `[]` | every member; `workers` `[]` |
| worker alone | absent | one, whose `engine` is a socket address on loopback, in plaintext | `workers`, one entry, and `limits`, alone |

- **Never mixed.** A document of none of these shapes is refused, naming
  the path that departs from the nearest: an engine with both a worker
  and a socket listener, a listener list with an in-process name and a
  socket, a worker alone with any other member. An in-process listener
  tells jig's core that no worker outlives the engine, so it settles the
  claims of every worker at once on a restart (jig's `domain/engine.md`,
  section 6): a socket's workers among them would lose their runs.
  The nearest is read from two members: a document without `engine` is
  nearest a worker alone; one whose `engine.hosts.listener`'s first
  entry is a socket address, an engine alone; any other, standalone.
- **Each shape's checks** (configuration.md, section 2): standalone's,
  as section 2 gives them; an engine alone's the same, but for its
  listener and its workers, by the table's rule, and the root's for
  every shape with an engine; a worker alone's, its worker part's, its
  entry of `application.workers` (configuration.md, section 4) and the
  limits' checks its side holds (configuration.md, section 7).
- **The service** has an optional engine and an optional worker:
  standalone both, an engine alone no worker, and a worker alone no
  engine, whose loop, ending and exit are its worker's alone, with no
  store (sections 6 and 7; jig's `protocol/service.md`, section 6).
- **Three directories for each process,** its own (configuration.md,
  section 12): an engine alone's data holds the deployment's directory,
  the store and the secrets' store among it, and each worker's digest; a
  worker alone's, its secret, its root, where its workspaces are, and
  its programs directory. No two processes share one.
- **Memory by its service:** an engine alone's worst case within
  `engine.memory_bytes`, a worker alone's within its worker part's
  `memory_bytes` (configuration.md, section 7); io's sockets, each its
  own service's.
- **`temper program-login`** takes a worker alone's directories as it
  takes standalone's, and reads only the worker's part: each worker
  signs its programs in on its own, and none is copied to another
  (credentials.md, section 3; `../draft/reference-case.md`, the second
  edition, leaves sign-ins out).

### 15.2 Its processes

```
fake forge   temper fake forge ... --ready PREFIX    (setup.md, 8.4)
fake llm     temper fake llm ... --ready PREFIX
engine       temper serve DIRS     after both fakes: an engine alone
w1, w2       temper serve DIRS     after the engine: each a worker alone
  agents, git                      its children, each a process group
```

- **Started in order, stopped in reverse:** the fakes; the engine, once
  both fakes are ready; each worker, once the engine is (15.3). A worker
  may start later than the others, as the reference case's s2 does.
- **Every address from a ready line:** each listener binds port 0 on
  127.0.0.1, and its process prints what it got; the description's
  templates carry it to the processes after it (the lab's `lab.md`, 4.4).
  The engine's document names the fakes' addresses as their ready lines
  gave them, and each worker's names the engine's workers' listener as
  its `listening workers` line gave it.
- **Workers are the engine's configured positions,**
  `engine.hosts.workers`, each with its own name and secret (jig's
  `domain/engine.md`, section 8). A worker started later is a position
  the engine knows from its start, empty until that worker links; a
  position added is a new engine document, so the cluster's restart
  (15.4).
- **The fakes are setup.md, 8.4's,** each a process of its own with its
  ready line. A worker's git is the program an assignment's repositories
  name, `temper fake git` for a fake forge (channel.md, section 4): the
  workers share the fake forge's machine, so its program's path and its
  loopback listener serve them as they serve standalone's worker.

### 15.3 Joining, and its lines

- **jig's link, in plaintext over loopback** (jig's
  `protocol/channel.md`, section 10): the worker dials the engine's
  workers' listener; the engine takes its name and its secret against
  that worker's digest, then its hello: its capability, its configured
  position, its capacity and a stop bound strictly below the engine's
  grace, `fleet.grace_ms`. The worker redials, jittered and doubling,
  until it links.
- **Secrets are the description's fixtures:** each worker's secret, and
  its digest in the engine's data, a fixed pair the description holds
  and renders into each process's directories (15.5); a test's, never a
  deployment's, and never copied from one.
- **Short graces:** the description's are seconds, each worker's stop
  bound below the engine's grace, so a lost worker's runs are placed
  again within a test's time.
- **Lines on stderr,** each one line, flushed once what it says holds;
  the cluster's contract, read by their literal prefixes (the lab's
  `lab.md`, 4.3):

  | Line | Printed by | When |
  |---|---|---|
  | `listening <listener> <address>` | each shape with an engine, for `clients`, `webhooks` and a socket's `workers` | once that listener is bound, with the address bound (`listening workers 127.0.0.1:41733`; an IPv6 one bracketed, `[::1]:41733`) |
  | `linked <worker> <address>` | a worker alone | at each opening of its link: the engine has taken its secret and its hello is sent, with the engine's address |

  A refused opening or hello is a note on the worker's stderr with jig's
  reason, and the worker redials, as a worker between hosts would; it
  never exits for one. The note reads `refused <worker> <address>
  <reason>`, the address the engine's as `linked` gives it, the reason
  the engine's refusal as jig's link names it (`unauthorized`,
  `application`, `busy`, `replaced`, `not-admitted`, `stopping`), a
  peer's code jig doesn't name as `code-<n>`, or the worker's own
  (`terms`, `malformed`); the engine's text, which jig does not keep,
  is not in it. A refusal is noted once until its reason changes or
  the worker links: jig's link backs the redials off, and a repeat of
  the same refusal prints nothing. The sign-in link stays on stdout
  (section 4).
- **Each line's address is its listener's bound one:** the clients'
  server keeps its own, as the sign-in link reads it; the workers'
  routing keeps the one io's `Listening` event gives; and the webhooks'
  comes up from skein-forge-connection as its `listening` event (skein's
  `forge-connection.md`, 6.4, as skein's change 14h brings it).
- **Ready** (the lab's `lab.md`, 4.3): the engine at its `listening` lines
  and its sign-in link; a worker at its `linked` line. A task created
  before a worker's hello has reached the fleet waits for a free slot,
  so no ready line need say more.

### 15.4 Stopping, and what fails

- **Stopped in reverse** (the lab's `lab.md`, 5.2): each worker first, by
  its first termination signal, which runs its worker's ending: its runs
  stopped, and done once the engine has every turn and answer, or past
  its grace (jig's `protocol/service.md`, section 6), exiting 0; then
  the engine, drained, exiting 0; then the fakes. A process not ended
  within its cost class's `stop_seconds` (15.5) is killed, its tree with it,
  and the stop fails.
- **No survivors:** each process in a cgroup of its own, its agents and
  git within it; a stop is done once its cgroup is empty, and a process
  left in it is a survivor, killed and reported.
- **A worker lost,** its process killed mid-run, then its tree (the
  worker first, so it cannot report its agent's end as an agent's
  failure): the engine's fleet keeps
  its runs for its grace, then they are lost and their tasks retry, on a
  worker with a free slot (jig's `domain/engine.md`, section 8). The
  lost attempt is fenced, so nothing it may still have sent counts, and
  nothing lands twice.
- **A worker hung,** its tree stopped by `SIGSTOP` past the engine's
  grace, then continued: its run has been placed again; it links again,
  and the attempt it held, fenced, lands nothing.
- **A worker restarted** on its own directories links again at its
  position and takes new runs.
- **The engine ends the cluster.** An engine that exits, or is killed,
  under live workers ends the cluster's run: it is never restarted under
  them (section 14). Its workers, past their grace, cancel their runs
  and are stopped; an agent whose worker has gone sees its pipes close,
  and ends.

### 15.5 Its description

- **One description, `lab/cluster/`** in temper: an experiment in the
  lab's format, `lab-experiment/1` (the lab's `lab.md`, section 4). Its
  `processes` are 15.2's, each with its `argv`, its whole `env`, its
  `ready` lines (15.3), its `after`, its `memory_bytes` (15.1) and its
  `files`: its document, rendered from a template, and its secret or the
  digests. Its templates name each address by `{proc.P.ready.C}` and
  each directory by `{proc.P.dir}` (the lab's `lab.md`, 4.2). A cost
  class's `stop_seconds` is above each worker's stop bound and the
  engine's drain.
  The experiment is the lab's canonical JSON (the lab's `lab.md`, 2.2),
  and each process's `memory_bytes` is the one its own document states,
  within the lab's integers.
- **Two readers.** The lab's runner runs it as any experiment (the lab's
  `lab.md`, section 5), its driver null until the lab's drivers come
  (15.7). temper's process tier reads it with
  `crates/temper/tests/support/cluster.rs`: the format, the directories
  and the processes; the templates rendered; each process started by
  skein-world's `Binary`, in a cgroup of its own, by `after`, ties in
  name order, with no seed; its ready lines awaited on its streams; and
  the processes stopped in reverse, a termination signal, then
  `stop_seconds`, then the tree killed. It reads none of the lab's own
  members (actions, faults, collection, checks), and refuses none.
- **The reader's bindings,** where the lab's runner has its own (the lab's
  `lab.md`, 4.2): `{artifact.temper}` is the absolute path of the
  `temper` binary the tier built, as Cargo names it to the test
  (`CARGO_BIN_EXE_temper`); `{experiment}` is the absolute
  `lab/cluster/`; `{proc.P.dir}` is P's own directory under the test's
  scratch, each made `0700` before any process starts; and
  `{proc.P.ready.C}` is the capture, known once P is ready. A name of any
  other kind (a seed's, a run's, a run directory's, the browser's) fails
  the test, naming it: the description uses none.
- **Its cost class,** `temper-m1-calibration`, is the binding's
  (`../draft/qa-lab.md`, 10.1), as every experiment's is (the lab's `lab.md`, 6.5),
  until the cluster has a class of its own: the reader holds its
  `ready_seconds` and `stop_seconds` as that table gives them, the
  first bounding each process's ready lines from its start, the second
  each process's stop.
- **The fake forge's setup** is `temper serve`'s handover and reconcile
  at start, from records (setup.md, 6.5; `../draft/qa-lab.md`, 4.3). So
  `lab/cluster/` holds the fake forge, the fake LLM, the engine and
  the workers, the engine's one forge connected and unreached, as the
  process tier's other documents have theirs, until a story makes it
  the fake's setup, as the reference case's do (`../domain/testing.md`,
  2.6); its tasks are chats, and its stories'
  referee the tasks' ends on their named workers and no survivor. A
  chat's run holds its worker's one slot until its charter's idle
  threshold or its time less its reserve, both of which the description
  sets past the story's deadline, so each chat takes a worker of its
  own, never by timing. The description starts the fake forge before
  the engine, whose setup host takes its handover and reconciles at
  start (setup.md, 6.5).
- **Why a description,** not one program both call: the lab's runner is
  Python and the process tier is Rust; the lab's format is literal so
  that readers in any language agree (the lab's `lab.md`, 4.3); and a
  change to the cluster is one file both run, so the tests and the lab
  cannot drift apart. The reader is temper's until a second product
  needs one (section 14).
- **Cgroups, or no cluster:** several processes observed by one test
  need skein-world's cgroup trees, where the user's service manager
  delegates them; walking refuses a second observer. Without them, the
  cluster's tests fail at their start, saying why.

### 15.6 Its tests

Each where `../domain/testing.md` puts it (2.4 to 2.6):

| Where | What it shows |
|---|---|
| step tests | each shape read and checked, a document of none refused, naming its path; each line of 15.3 as printed |
| the engine's domain | two workers at positions 0 and 1, one slot each: two tasks, one placed on each; one lost, its run placed on the other past the grace |
| the real loop | an engine alone and two workers alone, three services on one ring with their own directories, the workers dialling the engine's listener on loopback: a task on each runs to its end |
| the process tier | `lab/cluster/` on real processes, as below, and the reference case's steps on them (`../domain/testing.md`, 2.6 and 5.3) |

The process tier's first stories, one slot on each worker. A temper
worker takes one slot today: smith gives its agent one run, and the
worker's limits keep its slots within its agents. Two tasks on two
workers need no more; a worker of several slots is later (15.7).


- **A task on each worker:** two tasks on the fakes, one run on w1 and
  one on w2, each to its end; then the cluster stopped, each process
  exiting 0, no survivor.
- **A worker lost:** w1 killed mid-run; its task's run is placed on w2
  past the grace, and lands once. Its process is killed first, then
  its tree, so the engine sees its link close, not an agent's end, and
  no silence is waited on. A waiting
  run's turn is committed only when the run ends, so the killed
  attempt's reply never reaches the engine; the retried run resumes
  from the chat's committed turns and answers anew, the reply is in
  them once, and the lost attempt's run ends as lost in the task's
  activity. The killed process's end is the story's, not a failure.
- **A worker hung:** w1 stopped past the grace, then continued; its run,
  placed on w2, lands once, and w1's attempt lands nothing. First, a
  stop past the engine's silence alone, continued within the fleet's
  grace, heals: w1 links again and the same attempt answers there,
  nothing placed twice. Then a stop held until the run is seen on w2
  (never a length of time): once continued, w1 links again, its
  listing of the fenced attempt is cancelled and what it sends of it
  dropped, and w1 takes a new run on its one slot, which shows it
  acted on the cancel, whose end sent the fenced attempt's held turn;
  the fenced attempt's activity and the chat's turns are as they were
  before the continue, and the reply is in them once.
- **A worker started later:** w2 started once w1's slot is taken; the
  second task waits, due and unplaced, with no attempt (the engine
  admits a run only once a slot is free), then runs on w2 once it
  links. A restarted worker links again at its position: w1, its run
  ended, is stopped alone, exits 0, and is started again on its own
  directories; it prints `linked` again, is the engine's w1 again, and
  takes a new run.
- **Every story waits on what shows each step,** read through the
  client and the processes' lines, never on a clock, within one bound;
  w2 starts only once w1 holds its run, so which worker takes a chat is
  never left to placement's order; and each chat is closed and ended
  before the cluster stops, since a worker's ending under a waiting
  run fails it into a retry.
- **A mixed document,** each shape's departures, refuses its start,
  exiting 1 and naming the path.

These are the retired first edition's multi-worker steps at their smallest:
4.1's placement by free slots, 4.3's worker added later, 4.6's hung
worker (../domain/testing.md, 5.3; `../draft/reference-case.md` is the
second edition).

- **The retained first-edition steps,** each a story of its own on the same
  description, the case's workers named ws, s1 and s2, its fixture
  applied to the engine's and the workers' documents, its acts by real
  `temper ctl`, and its oracle over what the processes observed
  (`../domain/testing.md`, 2.6): 4.1's opener then 4.3's two-hour
  check on ws, s1 and s2, s2 started by its act, the check's turn held
  on the fake LLM until every worker holds a run; and 4.6's restarted
  workstation, ws stopped under the director's waiting chat and started
  again on its own directories.

### 15.7 Later

- **TLS on the workers' listener, contained process trees, and workers
  on other hosts,** with hosts apart (section 14): a worker alone on
  another machine is the same document, its `engine` in TLS (jig's
  `protocol/README.md`, section 10).
- **An engine restarted under live workers,** with generations kept
  across starts (section 14).
- **The reference case's multi-worker steps driven in the lab,** by a
  driver (the lab's `lab.md`, section 6), once temper's experiments have
  one; the cluster's description runs with none until then (15.5).
- **A worker of several slots,** as in the retained reference-case
  tests (../domain/testing.md, 5.3; `../draft/reference-case.md` is the
  second edition): a worker's change,
  in smith's runs and the worker's limits, and not the cluster's. Until
  then each of the cluster's workers takes one slot.
- **`temper init` for the cluster's shapes** (section 14).
