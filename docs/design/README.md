# temper's design

Provisional, 2026-10-18, revised 2026-10-19. What temper is, what it is
built from, and where each part of its design lives. This document
comes first; the documents it lists in section 6 go deeper into each
part. temper is an application
of jig, the kit for agentic applications: jig's design is jig's own
(jig's `README.md`), and temper's documents say what temper adds,
decides or chooses, citing jig for the rest. The mechanics are those of
skein's `programming-model.md`. What is still open is listed in
section 8.

## 1. In one page

- **Software made by agents, directed by people.** temper is where
  people direct the development of software and agents do it: they
  write, check, review and land the changes, on a forge. People decide
  at the level of goals, designs and plans, and steer where agents
  cannot decide. Feedback on code is the agents' business.
- **An application of jig.** jig's core runs the work: tasks, their
  executors (agents, procedures, people), messages, authority, and
  connectors to external systems (jig's `domain/core.md`). temper brings
  what is its own: the forge as a connector, its agents' charters and
  tools, the checkout its workers prepare, its client, and the forge's
  setup.
- **The forge is the one external system** (`domain/forge.md`).
  Repositories are adopted as they are. A change has a procedure of its
  own to landing: produced, checked by CI, reviewed by gates, repaired,
  brought up to date, landed through a queue per branch. A goal shows on
  the forge as an issue, a projection temper writes and never reads back.
- **What temper does on the forge is temper's.** It writes only to what
  it owns, lands into shared branches, and takes part as one participant
  in what others own. It never forces a push, and merges exactly the head
  its decision saw. A change it did not make to what it owns is drift:
  the task relying on it is held, and nothing is written over.
- **Mechanics are code, judgement is an LLM's.** Procedures carry out
  what needs no judgement: a change's way to landing, a branch brought
  up to date. An agent is woken for what does: making a change,
  repairing a failure, reviewing, resolving a conflict, revising a plan.
- **The agents are smith's** (`domain/agent.md`). temper writes their
  charters and serves their tools; it builds no agent. Its worker hosts
  smith's agent processes, each run in a checkout of the repositories its
  task names (`domain/worker.md`).
- **The root is temper's, the rails are jig's.** temper's root composes
  jig's core with the forge connector, speaks jig's hub's link to its
  workers, and routes and translates between them; it decides nothing
  jig's core decides (`domain/root.md`). jig's rails hold temper to
  jig's promises: skein-lib's journal, jig's restart script, its
  conformance world and its referee, with temper's own wiring and its
  fake forge (jig's `README.md`, section 6).
- **One binary, standalone first** (`protocol/README.md`). `temper
  serve` runs the engine and one worker in one process on one person's
  machine; `temper open` is the person's terminal; `temper agent` is
  smith's agent, one process per run; `temper forge` and `temper box`
  set a forge up and run it (`protocol/setup.md`).
- **Tested at every tier** (`domain/testing.md`): domains and fakes in worlds,
  then through bytes, then the standalone service, hosted agents and
  fake peers on one real ring, then the real binary with an owned
  Forgejo, its runner and real git, then a real model.

## 2. The first milestone

The smallest product that works end to end, for real: one person, on
their own machine, with a terminal, a real Forgejo repository and a real
model. They adopt the repository, ask a chat for a change, follow the
work at the chat's level, and see what landed. The change is checked by
CI and an agent's review, and lands with no person in between.

One story:

1. `temper serve` runs on a deployment's directories,
   configured for one Forgejo (with Actions, or with project checks), one
   git identity, one worker and its LLM providers, never an account
   (`protocol/configuration.md`; `protocol/credentials.md`, sections 3
   and 4). The forge is connected, managed, or owned on a box
   (`protocol/setup.md`).
2. `temper open DIRECTORY LINK` signs the person in by the printed link
   and shows their inbox (`client/README.md`, sections 6 and 7). With
   no LLM account yet, a banner offers the ways to add one: a sign-in
   with an LLM provider, a Claude Code login made with `temper
   program-login`, or a key; the person adds one on the accounts page
   (`client/README.md`, 3.5 and 5.5).
3. The person adopts a repository into the configured project, through
   the adoption form (`client/README.md`, 4.4).
4. The person starts a chat and asks for a change; the chat makes a
   small one itself and hands its branch to a change task, or delegates
   the change whole (`domain/forge.md`, 8.1).
5. Agents do the rest: the change is produced, its pull request opened,
   checked by CI and by an agent's review (a gate the usual landing rule
   asks for, `domain/forge.md`, 8.3 and 8.7), repaired when either
   fails, and landed. No person acts on it.
6. The person follows it from the chat: its delegates and their phases
   in words, and, if the change is held, why, with what may be done
   (`client/forge.md`, 2.4 and 2.5).
7. The person sees what landed: the pull request and the merge commit,
   in the forge's words, with a link to each on the forge
   (`client/forge.md`, 2.5).
8. `temper serve` is stopped and started again; nothing the person saw
   is lost, and work in flight goes on.

Not part of it: a person's approval of changes (a choice a person makes
when giving direction, never temper's default), the browser, a run's
live text, the forge's panels in full, GitHub, several people.

## 3. The system

```
person    temper open: temper's terminal, its pages around jig's client (client/README.md)
engine    temper serve: temper's root around jig's core and the forge connector (domain/root.md)
store     jig's store in the engine's process: jig's records and temper's, committed together (protocol/store.md)
worker    standalone, in the engine's process: temper's worker root on jig's hub, the checkout (domain/worker.md)
agents    smith's, one `temper agent` process per run, on the worker (domain/agent.md)
forge     Forgejo and its runner: polled, hinted by its webhooks (domain/forge.md; protocol/forge.md)
```

- **One engine per deployment.** It is the store's only writer and the
  forge's only client (jig's `domain/core.md`, section 2).
- **The engine decides, the worker hosts, agents think.** The worker
  prepares checkouts, runs agents and pushes what they made; agents reach
  the engine only through the tools their host relays (jig's
  `domain/hosts.md`).
- **People reach temper through its client:** temper's pages on jig's
  client, `jig-client`, in a terminal and in a browser
  (`client/README.md`; `client/browser.md`).
- **Standalone first.** Hosts apart, webhooks from another host, TLS on
  the engine's listeners and contained process trees come with a
  deployment beyond one person's machine (`protocol/README.md`,
  section 14).

## 4. On skein, smith and jig

temper builds on three repositories, each pinned by `Cargo.lock`, and
follows their design:

| Repository | What temper takes from it |
|---|---|
| skein | the programming model and the testing strategy; lib and its journal; io, its machines and protocols (HTTP, JSON, TLS, the key-value store, channels, accounts); the UI kit and its text shell; the simulator, the fake machine and the fake person |
| smith | the agent: its runs, sessions and tools' families, its agent shell, its host domain and host protocol |
| jig | the core and its children (tasks, authority, people, the fleet, accounts, briefs, notes, views); the hub; the root's shape and its rails; the store, the workers' channel, LLM accounts, configuration, the clients' component; `jig-client`; the conformance world and its referee |

What temper adds is listed in `domain/README.md` (the domain) and
`protocol/README.md`, section 1 (the protocol layer). Where temper needs
something jig lacks, it is added to jig, with temper as its first user,
never built around jig in temper.

## 5. What temper promises

jig's promises (jig's `domain/core.md`, section 11) hold for temper,
checked by jig's referee in temper's worlds with temper's wiring. On the
forge, temper adds:

- **Nothing is forced.** temper never forces a push, never writes to an
  object it does not own but as a participant, and never overwrites a
  change it did not make (`domain/forge.md`, 8.5 and 8.7).
- **A merge lands what was checked:** exactly the head its decision saw,
  with every requirement its branch's rules enforce met: CI, gates and
  approvals at that head, up to date on the decision's reads
  (`domain/forge.md`, 8.7).
- **A landing temper did not make is news,** like any other
  (`domain/forge.md`, 7.2).
- **A change temper did not make to what it owns holds the task that
  relies on it,** and the client says why (`domain/forge.md`, 8.5).

## 6. Documents

- **`domain/`:** temper's domain layer on jig's: its root, the forge
  connector, the worker, and what temper fills in of smith's agent; and
  `testing.md`, temper's tiers, its fakes, its scenarios and its layout,
  on skein's testing strategy and jig's worlds. Its `README.md` gives the
  reading order, the structure, the names and the decisions.
- **`protocol/`:** temper's protocol layer on jig's: the processes and
  the binary, the configuration, the credentials, temper's parts on the
  workers' channel, its families in the store, the host tools and LLM
  accounts, the forge's protocol on skein's forge kit (skein's
  `forge.md`), git, and the forge's setup. Its `README.md` gives the
  reading order and the names.
- **`client/`:** temper's client on `jig-client`. Its `README.md` is the
  client: its client root and pages, the forge's client family, its view,
  the terminal, signing in, and the clients' listener; `forge.md` is the
  forge's part of the client, `browser.md` the browser shell, `tools.md`
  `temper ctl`, temper for agents (reads, jig's operations and waits,
  from a command line), and `mockups/` the pages' design: their look,
  parts, words and acts, binding, with a `STATUS.md` of their own.
- **`qa.md`:** temper's binding to skein's lab (skein's `lab.md`): its
  experiments in `lab/`, their charter, tool and checks, and M1's
  campaign.
- **`welcome-walk.md`:** the welcome's every path walked on `temper box
  --fake-backends`: the fake box, its process stories, its lab
  experiments and what the walk found, under `draft/fake-backends.md`.
- **`draft/`:** drafts, not contract.
- **`STATUS.md`:** how far the code is from this design, and which plans
  take it there.

## 7. Conventions

- **A bare file name** names a document of the same directory: in
  `domain/`, `forge.md` is `domain/forge.md`; in `protocol/`, it is
  `protocol/forge.md`. temper's other documents are named by their path
  from there: `../protocol/forge.md`, `../client/README.md`; from this
  directory, `domain/root.md`.
- **Code cites a section** by the document's path from this directory,
  at module or type level: `domain/forge.md`, 8.2; `protocol/forge.md`,
  3.2; `client/README.md`, 3.5.
- **jig's documents** are named with jig's name and their path in jig's
  `docs/design/`: jig's `domain/connectors.md`, jig's
  `protocol/channel.md`, jig's `client/domain/client.md`. jig has two
  `client.md` and two `testing.md`, so a bare jig file name is never
  enough.
- **smith's documents** likewise: smith's `domain/run.md`. skein's
  foundation documents by their own names: `programming-model.md`,
  `testing-strategy.md`, `notes.md`; skein's design documents with
  skein's name: skein's `lib.md`.
- **Each document** says in one page what it is, then its structure, its
  parts, what it owes and is owed below the domain, its world, and what
  is open. It states the contract: what is built and what is not is
  `STATUS.md`'s.

## 8. Open questions

- **A second connector:** test environments, which an agent provisions,
  explores and tears down. It would check that jig's connector contract
  is not the forge's in disguise; it waits for continuous QA
  (`draft/qa.md`) or the factory (`draft/factory.md`).
- **Where continuous QA lives:** in a temper deployment of its own
  (`draft/qa.md`, 6.1), or in the factory's lab, as an application of
  its own (`draft/factory.md`, section 13).
- **A person's approval as a rule:** whether temper alone enforces it,
  or posts the approval as the person, with a forge credential they
  signed in with, so that the forge's protection requires it too
  (`domain/forge.md`, section 20).
- **Contributions in and out:** outside contributions to a project
  temper works on, and temper's own contributions upstream
  (`domain/forge.md`, sections 13 and 14). Neither is needed first.

## 9. Crates' names

temper's crates follow jig's rule (jig's `README.md`, section 15), with
temper's side before the layer:

- `temper-engine-domain` (the root) and its forge children
  `temper-engine-domain-forge`, `-forge-change` and `-forge-issues`,
  which own skein's forge client, `skein-forge-client`;
  `temper-worker-domain` (the worker's root) and
  `temper-worker-domain-checkout`;
- the protocol layer's: `temper-worker-protocol-git`,
  `temper-protocol-store`, `temper-protocol-worker-wire`, and
  `temper-engine-smith`, the translation to smith; the forge component
  is skein's, `skein-forge-connection`;
- the forge's, of no side: skein's, `skein-forge` (its documents and
  call plans, which the forge component and the setup share) and
  `skein-forge-setup` (reconcile); its setup's rest, outside the
  services, `temper-forge-setup`, `temper-forge-setup-shell` and
  `temper-box`;
- the client's, under `temper-client` (`client/README.md`, section 2);
- the services, `temper-service`, and the binary, `temper`.

`protocol/README.md`, section 5 says what each holds and depends on. Test
kits are named by their kind (`testing/temper-fake-git-hosted`); the forge's are
skein's (`testing/skein-fake-forge-domain`,
`testing/skein-throwaway-forgejo`), and the worlds live in `tests/`
(`domain/testing.md`, section 7).
