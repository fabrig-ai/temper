# temper's client

Provisional, 2026-10-14, revised 2026-10-24. temper's client: temper's own
presentation on jig's client, `jig-client`. Its crates; its client root,
which holds `jig-client` and temper's pages, the LLM accounts' and the
forges' among them; the forge's client family, its words, its one form
and its requests; its view, on
skein's UI kit; the terminal that `temper open` runs, on skein's text
shell; signing in and the small store; what temper's server does for it;
and how it is tested. jig's client is jig's `client/domain/client.md`, its
API jig's `client/domain/contract.md` and its protocol jig's
`protocol/client.md`: this document takes them as given and cites them.
The forge's parts of the client are `forge.md`, the browser `browser.md`,
and the agents' tools, `temper ctl`, `tools.md`. The mockups (`mockups/`)
are reference material for the pages' look and the order of their parts,
not contract. What is still open is listed in section 11.

## 1. In one page

- **temper's presentation, on jig's client.** temper's client root
  holds `jig-client` as a component (jig's `client/domain/root.md`):
  the link, watches by handle, keyed requests until durable, the work
  kept across a restart, and the forge's family carried as bytes. Its
  pages, words, confirmations and navigation are temper's own. jig's
  reference UI (jig's `client/domain/pages.md` and jig's
  `client/domain/view.md`) is not temper's: temper arranges no page of
  jig's, and copies no client root, view root, native client or card of
  jig's.
- **Its pages,** on jig's (jig's `client/domain/pages.md`): the inbox
  (home), the chats, a chat, a project's board, a task's page, the
  changes, the forge's adoption form, the forges, and the LLM accounts
  (3.5); designed, not built, "How work runs" and a proposal of
  settings' review (5.13). The
  mockups give their look and the order of their parts. What no page
  shows is open (section 11).
- **Its cards are its own.** A proposal, a question, a held task, a
  person task, a result, an entry of activity, an outcome and each
  confirmation are temper's nodes, drawn from the object's document as
  `jig-client` delivers it and from `jig-client`'s `facts` (jig's
  `client/domain/client.md`, section 19), as jig's
  `client/domain/objects.md`, section 2 allows an application. temper
  calls no renderer of jig's reference UI, and copies none of its
  modules, functions or phrase tables.
- **On jig's API alone.** What a page shows, it watches through jig's
  subjects (`me`, `inbox`, `tasks{query}`, `task`, `tree`, `run`,
  `accounts`) or
  reads (`activity`, `proposal`; jig's `client/domain/contract.md`,
  sections 6 and 7); what a person does is jig's operations (jig's
  `client/domain/contract.md`, section 3), through jig's helpers where
  they fit (3.6).
- **The confirmation is temper's.** An act that cannot be undone says
  what it will do before it is sent, from the facts jig's documents
  give, and every deciding act names the revision the person was shown
  (3.6).
- **LLM accounts, made from the client.** On the accounts pages,
  `Accounts` and its flow `ConnectModel`, a person signs in with an LLM
  provider, declares a program's login, or adds a key, pasted once and
  never kept (3.5, 3.6); while no account their work may use is usable,
  a banner on every page says so and offers the three (5.5). A chat's
  composer and a task's page name the account and model the work runs
  on. A browser goes to a sign-in's address in its own tab and comes
  back; the terminal prints the address and opens no browser (6.5).
  The accounts are jig's, kept by its engine
  (`../protocol/credentials.md`, section 3).
- **A first run's demo.** When the deployment runs the demo
  (`../protocol/setup.md`, 8.4), the client reads it from the engine
  (4.8): a banner on every page says the deployment is a demo and offers
  to connect a real model and a real forge, and the demo's project's
  chats page suggests the requests its model plays (5.6). The printed
  link's sign-in in a browser lands on the chats page (`browser.md`,
  2.4).
- **The forge's family.** Its words for the connector, a repository, a
  branch, a pull request and each kind of write need no document (4.1).
  Its other parts are documents its client side reads, with requests of
  their own (`forge.md`); a part it does not read says "Shown on the
  forge." (4.2). Its form is the adoption, with a picker of the
  repositories temper can reach on the chosen forge and "Not listed"
  for the others, sent through `jig-client`'s request table as the
  forge's bytes, like any other keyed request (4.3 to 4.7).
- **Forges connected from the client.** On the forges' pages, `Forges`
  and its flow `ConnectForge`, a person whose role holds the
  deployment's connectors (in standalone, the owner) connects each
  configured forge, installs GitHub's App on
  another account, replaces a forge's key or tokens and forgets a
  managed forge's admin's token: the FORGE requests (section 8;
  `forge.md`, 5.4). A pasted value rides its request as jig's carried
  value, sent once and kept by neither the client nor the core (3.6).
  A connect's or an install's link is printed, never opened (6.5).
- **One crate of records.** `temper-client-protocol-wire` carries the
  forge family's records: the adoption, its outcome and the small
  store's record of a pending one (4.6), the FORGE requests and their
  outcomes (section 8), and those of `forge.md`, section 9. temper's
  server decodes them before the engine's root, which locates the
  repository and adopts it on the person's behalf, or runs the connect
  (section 8).
- **One view, every shell.** `temper-client-view` builds temper's pages
  as skein-ui's tree (skein's `ui.md`); the terminal prints that tree
  with skein's text shell (skein's `ui-text.md`); the browser, its
  second shell, maps it with skein's browser parts (`browser.md`).
- **A terminal first.** `temper open` runs temper's client, its view and
  skein's text shell over `jig-client-shell-native`, jig's native
  transport (section 6).
- **Signed in by jig's printed link,** on loopback, and kept signed in
  across restarts by its bearer credential, which the terminal keeps in
  the client's private directory (section 7). The forge as people's
  sign-in provider is open (section 11).
- **temper's server owns the clients' listener,** and mounts jig's
  clients' component on it with jig's default HTTP mapping, under
  `/jig`, with its example guards, and routes `/forges/...` for a
  GitHub connect's browser steps (section 8; `forge.md`, 9.1). The
  mapping's engine end
  owns its calls in flight: the server asks for its room before it
  hands it a call, and answers a call under `/jig` that finds none
  itself, as it answers a path no one claims: `503`, with no body,
  closing the connection (jig's `protocol/client.md`, 17.4). The
  server's connections are its own bound, never the engine end's.
- **Joined to the engine from the start.** `tests/client`
  (`temper-client-world`) runs temper's client and view, with skein's
  fake person, against temper's engine root (section 10;
  `../domain/testing.md`, 6.3).

## 2. The client's crates

```
temper's client
temper-client-domain            the client root: jig-client held, temper's pages, the forge's side (sections 3 and 4)
├── jig-client                  jig's client: the link, watches by handle, the request table, kept work, families; jig's helpers
└── temper-client-protocol-wire the forge family's records, for both ends (4.6)
temper-client-view              the view: temper's pages as skein-ui's tree, temper's words (section 5)
└── skein-ui                    skein's UI kit: the tree, the builder, bindings, the diff, phrasebooks

below the client
jig-client-protocol-wire        jig's records, for both ends, under jig-client
jig-protocol-transcript         turns decoded with smith's transcript crate, under jig-client

the shell
temper-client-shell-terminal    the terminal: temper's host, its settings and lines (section 6)
├── skein-ui-text               skein's text shell: the renderer, the command reader, the terminal
└── jig-client-shell-native     jig's native transport: skein's HTTP client with jig-client-protocol-http; the small store's file and the credential's
temper                          the binary; `temper open` runs the terminal (../protocol/README.md, section 4)
temper-client-shell-web         the browser: temper's shell, built for wasm32, its host ordinary Rust (browser.md, 2.1)
├── skein-ui-web                skein's browser parts: the page, the DOM, history, fetch, session storage
└── jig-client-shell-web        jig's client adapter over skein's fetch, and the printed link's sign-in part

for agents
temper-client-tools             the agents' tools on jig-client, beside the view (tools.md, section 4)
temper-client-shell-cli         `temper ctl`, over jig-client-shell-native (tools.md, section 8)

for tests
skein-fake-person               the person, on skein-ui's tree (../domain/testing.md, 4.4)
jig-client-test-kit             jig's check, and the faults of jig's client worlds (jig's `client/domain/testing.md`, 6.2 and section 7)
jig-client-fake-person-browser  jig's attack stories (jig's `protocol/browser.md`, section 15)
temper-client-world             tests/client: temper's client joined to temper's engine root (section 10)
skein-fake-person-browser       the person in Chromium, through skein-browser (browser.md, section 4)
temper-client-browser-world     tests/client-browser: the browser tier (browser.md, section 4)
temper-client-gallery           tools/gallery: the browser's pages in named states, as static documents (browser.md, section 7)
```

- **The crate graph.** `temper-client-domain` depends on skein-lib,
  `jig-client`, jig's records (`jig-client-protocol-wire`,
  `jig-client-protocol-link`) and `temper-client-protocol-wire`;
  `temper-client-view` on skein-lib, `skein-ui`, `jig-client` (for jig's
  facts and words) and `temper-client-domain`, and on no crate of jig's
  reference UI. Neither links io, HTTP, smith, or an engine crate of
  temper's: `jig-client` emits jig's messages, never HTTP, and turns
  reach it decoded (jig's `client/domain/client.md`).
- **`temper-client-protocol-wire`** is a codec crate generated by
  skein's generator from a schema, depending on skein-lib and
  skein-codec alone, no crate of jig's (jig's `protocol/client.md`,
  sections 3 and 11). temper's server and the client both use it.
- **`temper-client-shell-terminal`** holds, as modules:
  - `client`: the native client, `NativeClient`: temper's client root,
    its view and `jig-client-shell-native`'s transport, and when
    `jig-client-shell-native` keeps the credential's file (section 7);
  - `route`: each of the root's requests to the native shell, the host
    or one of temper's lines;
  - `terminal`: skein's text shell's terminal with temper's settings,
    its own lines and the sign-in's retries (6.2, 6.6, 6.7); and
    `Terminal`, the one host of skein's loop, composing the native
    client with stdin, stdout and the termination signals (6.2). The
    worlds put skein's fake person where `temper open` puts the
    terminal (section 10);
  - `limits`: the client's limits and their sum.
- **Step code under the subset:** the domain, the forge's side, the
  view and the wire. **Ordinary Rust:** the terminal's host and the
  browser's shell, as a shell is (skein's `ui.md`, section 2).
- **The client's limits** are constants of
  `temper-client-shell-terminal`: `jig-client`'s, from jig's small
  profile with the overrides temper's configuration gives the engine's
  documents (`../protocol/configuration.md`, section 7); the pages' and
  the forge side's; skein-ui's, derived from the view's (5.4); and their
  sum. The client reads no configuration: its only inputs are its two
  arguments (`../protocol/configuration.md`, section 9).

## 3. The client root

`temper-client-domain` is temper's client root, in jig's one shape
(jig's `client/domain/root.md`, section 3): `new`, `step`, `fire`,
`next_deadline` and `take`, and `shown`, a count of what is shown,
which the view builds from. It holds `jig_client::Client` as a
component and calls it; the client never calls it back, and the root
routes nothing of jig's (jig's `client/domain/client.md`, section 4).

### 3.1 What it holds

- **`jig-client`,** given its limits and a seed at construction.
- **Two children, each an owner** (jig's `client/domain/root.md`,
  section 15): temper's pages, owner `Pages`, which ask the client for
  jig's operations, reads and subjects only; and the forge's client
  side, owner `Forge` (section 4); numbered in 4.6. Each is the asker
  of every handle and key it gives `jig-client`, and draws its keys'
  numbers from the seed the shell gives the root.
- **`me`,** watched from start to sign-out under a handle of its own:
  the signed-in party, their projects with their roles and pools, what
  waits for them counted, and each project's merged templates (jig's
  `client/domain/contract.md`, 5.1). Every page shows the link's state,
  `jig-client`'s `Link`.
- **The page at its address** (3.5): its handles, its windows, the
  composer's words, and the open confirmation (3.6), at most one.
- **The project the person is in,** chosen among `me`'s projects; the
  first at start.
- **The pages' acts in flight,** each by its key: which page's card or
  composer sent it, so its answer reaches what shows it (3.3).

Everything is bounded by the domain's limits: the handles a page opens,
the rows of each window, the composer's bytes, the acts in flight.

### 3.2 Its modules

```
temper-client-domain/src/
├── lib.rs          the tree, re-exports
├── boundary.rs     events in (the start, a place gone to, the person's acts, jig-client's events), requests out (jig-client's, a place shown, signing out, kept work dropped, facts)
├── owner.rs        temper's two owners, Pages and Forge, as askers (3.3)
├── domain.rs       Domain: new, step, fire, next_deadline, take, shown; jig-client held
├── route.rs        each event to its child; what jig-client tells, to the child its asker names (3.3)
├── address.rs      temper's addresses and the inbox's filters (3.5)
├── pages.rs        the pages: their handles, the project the person is in, navigation (3.1)
├── pages/          one module per page: what it watches and reads, its state, its acts
├── acts.rs         acts as jig's operations, through jig's helpers; the confirmation (3.6)
├── amend.rs        the amendment editor: a draft of a task's controls, to jig's Amendment (3.7)
├── forge.rs        the forge's client side (section 4), with its modules below forge/
├── limits.rs       jig-client's limits, the pages', the forge side's, and their sum
└── tests.rs
```

### 3.3 Routing

As jig's client root routes (jig's `client/domain/root.md`, section 5),
with temper's two children:

- **Out and in:** `jig-client`'s requests to the transport leave the
  domain as they are, for the shell to carry; the transport's events
  reach `jig-client` as they are.
- **By asker:** what `jig-client` says of a handle or of a key goes to
  `Pages` or `Forge`, matched whole; an asker that names neither is a
  fact (`UnknownAsker`), and reaches no child. The pages hand it to the
  page that opened the handle, or the card or composer that sent the
  key, and drop one for a handle they closed; the forge's side to its
  form, its picker or the forges' page that sent it.
- **The link** goes to both children, and changes what every page
  shows; a page reopens nothing of its own, since `jig-client` keeps the
  watches. Kept work dropped goes to the shell (3.4).
- **A place gone to** goes to the pages, and to the forge's side when
  it is `Adopt { project }` or `Forges`; any other tells the forge's
  side it left.

### 3.4 At start

The shell starts the domain with the small store's records and the
inbox's address, then signs in (section 7), holding back neither. The
domain's children ask their watches from their start, `me` among them,
and `jig-client` opens none on the engine until it is signed in (jig's
`client/domain/client.md`, section 12), so no stream leaves without the
credential.

- **Another party's kept work** stays parked and is not shown (jig's
  `client/domain/client.md`, section 11); the terminal says so in a line
  of its own (6.6).
- **Pending reads at sign-in** end visibly in jig's existing forms,
  on successful installation even before the first `me`, and on
  sign-out (jig's `client/domain/client.md`, sections 7 and 12).
  Whoever asked must ask again; no read is replayed automatically.
- **Kept work dropped:** the domain hands `jig-client`'s `Dropped {
  count }` to the shell (jig's `client/domain/root.md`, section 8), and
  the terminal says so in a line of its own (6.6).
- **temper's pages keep no record of their own:** a client closed on
  any page opens at the inbox, and a composer's words not sent, or a
  form's values, are not kept across a restart (section 11). A browser
  opens at its address (`browser.md`, 2.3), and after the printed
  link's sign-in at `Welcome` (`browser.md`, 2.4;
  `../draft/first-run.md`, 2.3), which shows its first step while the
  first run is not over, and otherwise moves on to the chats page
  (3.5, The welcome).

### 3.5 Its pages

Each page has an address, and watches what it shows while it is open;
leaving it closes its handles. Its mockup gives its look and the order
of its parts (reference material, not contract). Lists of tasks are
task queries (jig's `client/domain/contract.md`, 6.1): the chat and
goal templates' labels are jig's defaults, which temper's configuration
keeps (`../protocol/configuration.md`, section 2).

| Address | Mockup | Shows | Watches and reads | Acts |
|---|---|---|---|---|
| `Inbox` (home) | `inbox.html` | what needs the person, across projects, filtered by project, kind and goal (its root): proposals, questions, held tasks, person tasks; then updates (results, replies); "+ New chat", a link to the Chats page's start (`/chats?at=start`) | `inbox` with its filters; the `proposal` or `task` an entry's card needs | accept ("Accept"), reject with a reason, answer, release, leave held, take, hand back |
| `Chats` | `chats.html`; the demo's start at the first run's first step, which `first-run.html` opens | a composer to start a chat, under the level-2 heading "Start a chat", its line and its chooser (5.5) above its text area "Start a chat" and its button "Start chat", with the hint "temper · Enter to start · Shift+Enter for newline"; in the demo's project while the demo's model half holds, the demo's start in a chat's frame, with its suggested requests (4.8, 5.6); the person's chats as one list, "Your chats", live and ended together, newest first, under All, Live or Closed (below), each with its number and title, its status, spend and live delegates | `tasks{query}`: requester the person, the chat label, its live group by last activity, its ended pages by when each ended; the demo, from the forge's side (4.8) | start a chat, from the composer or a suggested request; choose the list's filter; more closed chats |
| `Chat { task }` | `chat.html` | its chatbar: its number and title, its status, its spend and acts; after it, the context line, only in the demo's project while the demo is running (4.8, 5.6); the conversation (below): its request, the agent's words with its calls folded, the person's words, provisional text, and its end once it has ended; each task it delegated as that task's card, at the call that made it (a `delegate` or a `change`), or else at the result it sent back, or else after the conversation while it runs (5.7); the proposals waiting in its subtree, each at the call that made it where its turn names it, and after the conversation otherwise (jig's `client/domain/objects.md`, section 10); its status and spend; Stop and Close; the composer, while the chat lives: its line and its chooser (5.5), its text area "Message" with the placeholder "Message T6…", T6 the chat's number, shown while it is empty and never its name, and the hint "Enter to send · Shift+Enter for newline", and "Send", then a message's refusal or failure, if any: a message or the chat's start accepted draws no status of its own, neither at the conversation's end nor in the composer, since the conversation shows its words, "Being sent", then "Sent" (below); while Message is not offered, its line and chooser alone; an ended chat draws no composer and no composer's line | `task`, `activity` a page at a time, older pages while its window has room (5.4), `run` while one is live; each `proposal` its `task` names; each delegated `task` its calls, results and live delegates name, up to the domain's `reads` (5.7) | message, stop, close, accept or reject a proposal in place |
| `Board { project }` | `board.html` | the project's goals in priority order, live, waiting and recently ended, with their progress, spend and deadlines; what needs the person; "Set a goal" | `tasks{query}`: the project, tracked, by priority; ended, by ended at | set a goal, reorder goals |
| `Task { task }` | `task.html` | a goal: its header, what was asked and the result it gives, its result once ended, its plan as a tree of its delegates, live and ended, with their counts and its filter, each task's state, spend and dependencies, its latest activity and history, its budget and authority (5.8), with Amend, Stop and Cancel (5.11); a change: its header and "View on forge ↗", its steps, its step's line, its hold, its tasks live and ended, its gates at its head and its bounds (5.10); an agent's task: its lineage, its brief, its run's heading, its conversation as a chat's, its result once ended and its plan's held cards (5.9), with Amend, Stop and its composer (5.11); either's own held card while it is held | `task`, `tree`, `activity` a page at a time, older pages on an agent's task's "Older"; `run` while one is live; a change's panel, through the forge's side (`forge.md`, 9.2); a goal's or a procedure's ended delegates, a page read once and again on a change | a goal's plan's filter (5.8); as its document offers them (5.11): on a goal amend, stop, cancel; on an agent's task message, amend, stop; release its own held task or one of its plan |
| `Changes { project }` | `changes.html` | the forge's changes and landing queues (`forge.md`, section 3) | the forge's, through the forge's side (4.2) | the forge's |
| `Adopt { project }` | none: 4.4 | the adoption form, into `project`, or, with none, into a new project (4.4), with the picker of the repositories temper can reach on the chosen forge | `me`; the chosen forge's deployment part, for its kind, mode, user and installations (`forge.md`, 5.4); the picker's page, a `Reachable` request (section 8) | adopt a repository (4.4) |
| `Forges` | `forges.html` | the person's forges, each row folded until opened: its connection, its mode and access, on GitHub temper's App and its installations, a connect under way and how the last ended, its load; then the playground's forge, folded, while the demo's forge half holds; "+ Add a forge", to `ConnectForge` (`forge.md`, 5.4) | the forge's, through the forge's side (4.2) | connect a stopped forge; replace a key or tokens; install on another account; new repository; forget the admin's token; set up again (section 8) |
| `ConnectForge { forge }` | `connect-forge.html` | connecting one more forge, `forge` the choice it is at: Choose, Details, Setting up and Ready, which ends on the work it adopted (`forge.md`, 5.4) | the forge's, through the forge's side (4.2) | connect; connect as its admin; set up a Forgejo here (section 8) |
| `Welcome { step }` | `welcome.html`; the first run's step 0 | the first run's welcome, until the person has chosen: "Try the playground" or "Set up temper", the latter a wizard of Welcome, Forge, Model and Ready, its `step` (5.6; `../draft/first-run.md`, 2.3) | the demo's state (4.8); `accounts`; the `forges` watch, for a person whose role holds the deployment's forges; the forge's, as `ConnectForge` reads it | start the playground; `ConnectForge`'s and `ConnectModel`'s acts |
| `Accounts` | `accounts.html` | the person's LLM accounts, then, for an owner, the deployment's, each row folded until opened: its LLM provider and display name, its holder, its deciding source's state and its line (5.5, An account's line); opened, that state's words and acts, its sources in order, each with its state in words and, for a program's, on which hosts (5.5), what it pays for, the runs using it and the tasks waiting for it (facts jig gives the deployment's accounts alone for now, so a person's own leave them out: 5.5) and its acts; the checks under way, and how the last ended; then the playground's account, folded, while the demo's model half holds; "+ Connect a model", to `ConnectModel` | `accounts` (jig's `client/domain/contract.md`, 7.1); `me`'s LLM providers | sign in again or add a key from a state's acts, as `ConnectModel`'s ways; sign a source out; add a key to an account; replace a key; remove a source or an account; check a program's login again; move an account, or a source, up or down; resume a limited source |
| `ConnectModel { way }` | `connect-model.html` | connecting one more LLM account, `way` the way it is at, or none at Choose: Choose, a card for each way its providers offer (5.5, The ways); Signing in, at the provider, for a sign-in, or Details, a way's field, for a key or a program's login; and Ready, the account made, or how the sign-in ended (5.5, A sign-in) | `accounts`; `me`'s LLM providers | sign in with an LLM provider; add a key as an account of its own; declare a program's login |
| `HowWorkRuns` (designed, not built) | none yet: the user's, from the branch `configuring-mockups` (5.13) | the settings in the person's words: the newest revision and who decided it; "Tell temper how you work"; pending proposals of settings; roles, ways to start work, rules, running work and history, each row tagged Default, Changed or Added (5.13) | `settings`; each setting shown, `setting` (jig's `client/domain/contract.md`, 5.1 and 6) | start the configuring chat; move a revision's tasks to the newest; propose a reset, a removal or a restore |
| `Proposal { proposal }` (designed, not built) | none yet (5.13) | a proposal of settings: temper enforces and your agents follow, its effect on running work, the settings folded; withdrawn when stale; accepted (5.13) | the `proposal`; `settings`; each item's setting before | accept, choosing whether running work moves; reject with a reason; change something, in the chat |

- **Words and facts:** a chat's status from jig's facts (jig's
  `client/domain/client.md`, section 19), in jig's words, with temper's
  own in places ("Waiting for you", "Parked"); a conversation from
  jig's thread model; jig's enums and a decision's facts in jig's
  words, under temper's phrasebook (5.1);
  each card temper's own (section 1).
- **A chat's chatbar** (`chat.html`): its level-1
  heading "TN · TITLE", the chat's task number and its title (a
  chat's first message); its status; its spend; and its acts. Nothing
  of the composer's line (5.5) is repeated there, neither its model nor
  its effort, nor the mockup's template, charter, host kind and budget:
  the composer's line says what the next run uses (the user's pick,
  10-23).
- **A conversation's turns** are named "You" when the turn is the
  person's own, "Temper" when it is the agent's, and by the party's
  name otherwise (`chat.html`), on every page that draws a
  conversation. A chip's executor keeps jig's "Agent". On a chat's
  page each turn's time follows its name, "HH:MM", under the day's
  rule (5.12): a line's time for the agent's words and the person's,
  and for the request the conversation's first line's; words not yet
  read by the engine have none.
- **A chat's status,** in its chatbar and on its line of the chats
  page, is jig's, narrowed as the mockups' "Where each value comes
  from" says: "waiting on 1 delegate", "waiting on N delegates" while
  it waits on its delegates, N its chip's live delegates, as a goal's
  header says it (5.8). On the chat page, the delegated tasks' documents
  reconcile that count while deliveries catch up: a card still running
  counts until its final document arrives, and an ended card no longer
  counts even if the chat's document still lists it. It says "waiting on
  your words" while it waits on news and its wake policy wakes it on
  words (its document's `wake`), from
  the person signed in, who requested it; jig's word otherwise. While
  a question a delegate asked it waits in the person's inbox, jig's wait
  is on the person, before its delegates, and it says "Waiting for
  you". A line of the chats page reads the same wake policy, from its
  chip's (`OwnerChipDetails`' `wake`), and says "waiting on your words"
  as the chatbar does: a chat at rest after its reply, which only its
  person's words or Close move on (jig's `domain/tasks.md`, 5.8). A
  chat whose host refuses its start says jig's "Cannot start yet", and
  its composer's line says why (below); a line of the chats page holds
  no run wait, so it is never refused, and says jig's word for its
  stage (jig's `client/domain/client.md`, 19.1). A chat held on a
  configured bound (jig's `Exceeded` hold) says jig's phrase for that
  hold, as its hold card does: "Held: the model's reasoning went past
  its configured allowance of 2048 bytes". Any other hold says jig's
  "Held", and the chat's page says why.
- **A chat's conversation** is the chat's own, drawn from jig's thread
  (jig's `client/domain/objects.md`, section 10), never its runs'
  mechanics (`chat.html`, the mockups' first run):
  - *its request,* first, under "You" (or the requester's name): the
    chat's specification's words (its document's `spec`), a paragraph
    for each of its lines, where the window holds the conversation's
    start, its first line with no older page; a chat whose spec is
    empty has no such turn, and opens on its first words line;
  - *the agent's words,* each assistant message's text under "Temper",
    whole within the receiving bounds (5.4), read as Markdown by
    `skein-ui`'s reader (skein's `ui.md`, section 10): paragraphs, lists,
    quotes, code, and tables at the top level. A browser draws a table
    as the mockups' (`md-table`), and leaves out a header row whose
    cells are all empty, as they do;
  - *its calls,* after its message's text: each delegation as its
    task's card (5.7), then the message's other calls in one
    disclosure (skein's `ui.md`, its disclosure), named by its count,
    "1 tool call" or "N tool calls", and described by their tools'
    names in order, a name called again given once with "×N", joined by
    " · " ("read ×2", "read · edit"). A browser shows it folded to its
    summary, the name and then its description in a lighter tone, as
    the mockups' two tones, until the person opens it; the terminal
    prints the description on its own line under the name (skein's
    `ui-text.md`),
    while the terminal and the fake person find its content whole: each
    of those calls in order, as a work line, its tool's name and its
    outcome where the call has one, as an agent's task's page draws a
    call (5.9). A call that a card stands for is neither counted, named
    nor listed;
  - *the person's words,* the thread's words and each words line a
    person sent (jig's `OwnerActivity::Words`, by a person), under
    "You" or the party's name, a paragraph for each line, with "Being
    sent" or "Sent" until the engine has it, and no word once read;
    an answer the person gave in the inbox to a question on the chat
    is among them;
  - *a delegate's words,* each words line a task sent the chat (jig's
    `OwnerActivity::Words`, by a task), a delegate's question among
    them, under the task's name, "TN", on the agent's side, a paragraph
    for each line;
  - *not drawn:* a run's beginning and end, the task's milestones, and
    every message a turn's person, tool or system speaks, which are
    the run's inputs: its opening, its calls' results, and what woke
    it, a result delivered among them. A result block still anchors
    its task's card where the call that made it is not in the window
    (5.7), drawn on the agent's side, never under "You";
  - *its end,* once the chat has ended, after the conversation: one
    line, "Closed" and the time it ended ("Closed 21:05", 5.12's
    clock), the status's word (3.5's chats), then, for a report,
    " · its report, “REPORT”", the
    report's words whole; a failed chat's line is "Failed", its time
    and its reason ("Failed 21:05 · REASON"); a chat cancelled by the
    party that requested it was closed, and its line is "Closed" and its
    time ("Closed 21:05"), with no reason; one cancelled by anyone else
    is "Cancelled", its time and its reason ("Cancelled 21:05 ·
    REASON"). Who closed it is the ending's `by` against the chat's
    requester, both in its document, never the reason's words. The
    result is not drawn again.
- **The chats list** (`chats.html`), under the heading "Your chats",
  under the demo's start too (5.6), is one list: the live group's
  lines by last activity and the ended page's by when each ended,
  merged newest first, a tie by the lower number first. "All", "Live"
  and "Closed", three buttons, the chosen one pressed, filter it in
  the page, reading nothing; the filter is in the page's address
  (`browser.md`, 2.3), "/chats?state=live", a filter chosen a move, as
  the inbox's. Under "All" or
  "Closed", while an older page of ended chats remains, "More closed
  chats" reads it, in place of the last. With no line to show it says
  "No chats yet.", or under a filter "No live chats." or "No closed
  chats.". A line is the chat's number, "TN", a link to its page, then
  its title, its status (the chatbar's: one word for one state in
  both places, and "Closed" for every ended chat, as the filter says
  it; the user's pick, 10-23), its
  spend as its chatbar says it, and "1
  delegate" or "N delegates" while it has live ones, and when it
  ended or was last active, as 5.12's ago ("1 h 28 min ago").
- **The inbox** (`inbox.html`) shows what needs the person, then the
  updates, each entry its card (section 1), under two headings, each
  with how many entries it lists beside it, the updates' as new ones
  since what is read leaves them (below): "Needs you 2", "Updates 2
  new", or "0"; "2+" and "2+ new" where the inbox's document says its
  list was cut (jig's `more`, an owner inbox's next page, a filtered
  page's `cut`), so that a count never says less than there is; with
  none, "Nothing needs you." or "No updates.". Its first heading's
  row holds "+ New chat", a link to the Chats page's start
  (`/chats?at=start`, its text area focused: no act of its own), then
  its filters: the projects, each a button, while the person has two
  projects or more (with one, the banner's project says it; the
  user's pick, 10-23, until skein-ui has a menu and the three filters
  become menus); the kinds in one
  disclosure, folded until the person opens it, holding "All kinds"
  and each kind, the chosen one pressed, and named "Kind" while every
  kind is shown, "Kind · Question" once one is chosen, in the word its
  pressed button says, so that the folded menu says what the list is
  filtered by; then "All goals" and "Goal". The filter is in the
  inbox's address (`browser.md`, 2.3), "/?kind=question": a filter
  chosen is a move to the inbox at that address, as a link pressed is,
  so that a reload, back and a shared link keep it, and back undoes
  the choice. The move folds the Kind menu, as a menu closes on its
  choice, and its name says what was chosen.
- **A card that needs the person** begins with a line: its kind as a
  tag, "Proposal", its project and the task's link, "Task N", and at
  its right how long it waits, "Waiting 12 min" (5.12's span), from
  its object's time: a proposal's wait, a question's, a hold's, a
  person task's; on a chat's page too, a proposal's card says "waiting
  12 min". Its title, where it has one, is its heading: a proposal's,
  the goal it proposes; a question's, a held task's or a person
  task's, the task's. A proposal's card is the chat's, its title, its
  text, its rows and its acts, without the chat's "Open in inbox ↗".
- **An update's card** begins with its project and the task's link,
  "Task N", which opens it and so marks it seen (below). Its heading
  says what came: "T10 Add a --json flag to tally report replied" for
  a reply, "T10 … is done" for a result done, "T10 … ended" for
  another, its ending in words after it. Then a result's first line;
  and last, how many of the chat's turns are unread, where the inbox's
  entry counts them, and when it came (5.12's ago), a result's time, a
  reply's last activity: "1 new · 12 min ago", "5+ new · …" where the
  count is limited, "12 min ago" alone for a result or a count not
  given. A reply draws neither its request nor its words: no field
  carries a reply's first line (mockups README, Where each value comes
  from).
- **A question** in the inbox is a delegate's, asked of a task the
  person requested, their chat among them (jig's `domain/people.md`,
  section 6). Its card, under "Needs you": "Question", its project, the
  asked task's link, "Task N", whose conversation holds the question
  (above), and the question's words as written. Its act is an answer in
  words, "Answer words" and "Answer", one request (3.6): "Pending" while
  it is sent, "Answered" once the engine has it. The answer goes to the
  asked task, which relays it to its delegate as its own judgement. An
  answered card keeps its outcome while the page stays open; the inbox
  read again holds it no more, and the navigation's count no longer
  counts it. A question carries words alone, so its answer is words
  (choices, section 11), and temper has no hold of its own for one: the
  person answers in words, or stops the asking task from its page,
  which holds it until it is released (5.11).
- **Seen** is how far the person has read: jig's read positions (jig's
  `client/domain/pages.md`, section 3), one over their results and one
  over each chat's turns. An update is seen when it is opened, and no
  page offers "Mark seen":
  - *a chat's turns,* once its page shows them: as its window loads,
    and as turns arrive while it is open, through the newest its window
    holds;
  - *a result,* when the person follows its entry's link from the
    inbox: through that result, and so through every result before it,
    since jig keeps one position over them. A result opened from
    another page, or its entry left unopened, stays unread.

  A position only moves forward, with one `Seen` in flight for each;
  one refused or failed is sent again at the next move. What is read
  leaves the inbox's updates, and a task forgotten once read is erased
  then (jig's `domain/people.md`, section 6). The pages draw no mark of
  their own for what is unread, beyond the inbox's counts of it
  (above).
- **A task's page** is of its kind, as `task.html` lays each out: a
  tracked task's, a goal's (5.8); an agent's task's, for an agent's
  task with no priority (5.9). A task with the chat label opens as
  `Chat { task }`. A procedure's task shows a tracked task's header
  and plan, with its goal and its plan as "Its tasks", its ended tasks
  among them, and a change's task its panel (5.10); a person
  task shows a tracked task's header and plan only (section 11).
- **The forge's parts** of a page (a goal's issue and changes on its
  page; changes in flight and the project's health on the board) are
  `forge.md`'s, and "Shown on the forge." while the forge's side does
  not read them (4.2).
- **Links are by number:** a chip, an entry or a line naming a task
  opens its page, from any page (jig's `client/domain/pages.md`,
  section 2).
- **The accounts page** is jig's reference UI's accounts, on a page of
  temper's own (jig's `client/domain/pages.md`, sections 7 and 11),
  with temper's words (5.5), in the forges' page's shape (with the
  user, 2026-10-23). It shows no value. Its acts are jig's accounts'
  requests (jig's `client/domain/contract.md`, 3.2), each sent as 3.6
  says. Its parts, in order:
  - *"Your accounts",* level 2, with their count and "+ Connect a
    model", to `ConnectModel`; then each of the person's accounts as a
    disclosure, folded, named by the account's heading (5.5, An
    account), its summary its deciding source's state in words and the
    account's line (5.5, An account's line). Opened: the state's words
    and acts (5.5, Each state); its sources in order, each with its kind
    and state in words; "Pays for", and on the deployment's accounts
    "Used by" and "Waiting for it", whose facts jig gives the
    deployment's accounts alone for now (a person's own leave them
    out, and its line its use: the user's pick, 10-23); and
    the account's acts: sign a source out, add a key to the account,
    replace or remove a key, remove the account, resume a source, move
    the account or a source, check a program's login again;
  - *the deployment's accounts,* for an owner, the same way under
    "Deployment accounts";
  - *the playground's account,* in a section of its own, folded, while
    the demo's model half holds (4.8).

  A state's "Continue with ChatGPT" and "Use an API key" (5.5, Each
  state) are `ConnectModel`'s ways, sent from the row, a sign-in coming
  back to the page; the page adds no account of its own: that is
  `ConnectModel`'s.
  - *Its acts are the rows':* each names what it acts on, its holder,
    account and source, from the row it is pressed in, so that no act
    asks for a number. "Add an API key" and "Replace" open a secret
    field in their row, with "Add key" or "Replace key"; a value
    written in one row is dropped when another row's field is written.
    "Use an API key" in a state's acts is the row's "Add an API key".
    "Continue with ChatGPT" in a row is a sign-in for the account's
    provider, held by the account's holder: its address, then its end,
    are drawn in the row (5.5, A sign-in), as `ConnectModel`'s Signing
    in draws them, its paste among them (`ConnectModel`, The paste). "Use the API key first" moves the account's first
    key after its deciding source to that source's place. "Check
    again" checks the program's login of the source it names. "Sign
    out", "Remove" and "Remove account" ask to confirm first (3.6).
    The page has one request out at a time, as each page has: while it
    is out, every row's acts are disabled, and its outcome is drawn
    under the row that sent it.
  - *The playground's account,* under the level-2 heading "The
    playground", is one disclosure, "Scripted model · the playground’s,
    until a model of yours connects · held by the deployment", holding
    "It plays the playground’s scripts: it answers the suggested
    requests, and nothing else. It costs nothing, and nothing leaves
    this machine.", from the demo's state alone (4.8). An account of
    the demo's LLM provider (`Demo.provider`) is in neither "Your
    accounts" nor "Deployment accounts", nor in their counts.
  - *With none of the person's own,* "Your accounts" holds "No model
    of yours yet." and "Connect a model", to `ConnectModel`.
- **`ConnectModel`** (`connect-model.html`) connects one more LLM
  account, in `ConnectForge`'s shape: Choose; then Signing in, for a
  sign-in, or Details, for a key or a program's login; then Ready.
  - *Choose* is a card for each way the configured LLM providers offer
    (5.5, The ways), each with its words and its button. A way adds a
    new account to the person's own accounts, a key's to the
    deployment's when an owner has chosen "Use deployment accounts"; a
    plan's sign-in and a program's login are always the person's own,
    since only a person holds a plan (jig's `domain/accounts.md`, 3.2),
    and their ways are not offered while the deployment's are chosen.
    An owner chooses above the cards, "Use your accounts" or "Use
    deployment accounts", the one chosen pressed, yours at the flow's
    start; the choice is the flow's state, kept across its steps, not
    its address.
  - *Signing in* has no page of temper's: the way's button sends the
    sign-in, whose answer is the address to open, and a browser goes to
    it in the page's own tab. The provider sends the browser back to
    the engine's listener, which sends it on to the page it left, and
    the page shows the sign-in's end, read on the `accounts` watch:
    Ready, or the ending's words and acts (5.5, A sign-in). The
    terminal prints the address and opens no browser (6.5). temper
    detects no browser elsewhere (section 11, A browser on another
    machine): the person says so, below.
  - *From another computer,* for a browser not on the engine's
    machine, whose provider sends it to a loopback address where
    nothing answers. The sign-in's card on Choose offers, beside its
    button, "This browser isn't on temper's computer ›", which sends
    the same sign-in with no departure (no `Return`, no
    `Request::Leave`) and keeps the page at Signing in: once answered,
    it draws "Open ChatGPT's sign-in ›", the sign-in's address as an
    external link, a tab of its own (the person's own click, which no
    browser blocks as a pop-up), and *The paste* (below) under it. A
    sign-in under way that the page draws offers the paste too, folded
    under "Signed in, but the page couldn't be reached?"; unfolded,
    with "Go forward to that page, or find it in your history, and
    copy its address." first, at a departed sign-in's address come
    back to by hand, not by the listener's return, while it still
    waits. The tunnel still works, and needs neither.
  - *The paste:* "When ChatGPT is done, its tab shows a page that
    can't be reached. Copy that page's whole address from the address
    bar, come back here, and paste it. If a page opens instead, such
    as another program's sign-in, close that program's sign-in and
    start again." A field, "Address of the page that couldn't be
    reached", and "Finish connecting", which sends jig's `FinishSignIn`
    for the sign-in's source, carrying the address, its surrounding
    spaces and line breaks trimmed, as its value (jig's
    `client/domain/contract.md`, 3.1 and 3.2). The
    address is held as a key's value is: the page's state alone, never
    in its address, the small store or a draft, dropped once sent, or
    when the page or the way is left; its field asks the browser to
    keep nothing (`autocomplete="off"`, no spelling check), and is
    disabled while empty or past the bytes `me` states. While the
    request is out, the button reads "Finishing…"; answered
    *finishing*, the field goes and the card reads "Finishing your
    sign-in…" until the `accounts` watch shows the end, Ready or the
    ending's words (5.5, A sign-in). Refused, the field stays empty
    with the refusal's words above it (5.5, The paste), and the sign-in
    still waits; refused *not waiting*, the card follows the watch: an
    end it shows, or, with the sign-in gone, "This sign-in has ended.",
    and the way's button again. *Lost* or *unsent* asks for the paste
    again: the client never sends it twice by itself.
  - *The tab's departure,* in a browser, in this order. The sign-in's
    answer, `AccountSignInStarted`, names its source S and the address.
    The page then sends `Return { source: S, path }` (4.6), `path` the
    address it returns to: `ConnectModel`'s at the way with the source,
    `/accounts/connect?provider=N&way=sign-in&source=S` (`browser.md`,
    2.3), or the welcome's Ready, `/welcome?at=ready`, from the
    welcome's Model step. Once `Return` is answered done, the root asks
    its shell to leave for the address (`Request::Leave`), once, from
    that answer: never from its state, so that no build, reload or
    return asks again, and a page come back knows no departure of its
    own. A `Return` refused, failed or lost leaves the page where it is.
    Signing in shows the address as an external link throughout, a tab
    of its own, so that a page that stays, a shell that cannot leave
    (the terminal's root is made with no departure: it sends no
    `Return`) and a browser that refuses the target (skein's `ui-web.md`,
    section 10) still offer it. The engine keeps a return as long as
    the sign-in, and never across a restart: after one, its listener
    answers with its fixed page, "Sign-in received. You may close this
    page.", and the person goes back to the page by hand (skein's
    `oauth.md`, 6.5).
  - *Come back,* the page is a new client at `ConnectModel`'s address
    with the source (`browser.md`, 2.3): it takes the source's sign-in
    as its own and draws it as Signing in did, while the `accounts`
    watch lists it under way, then its end: Ready, or the ending's words
    and acts, when the document's last ending is the source's. Until
    the watch has delivered, Signing in's card alone; a source the watch
    shows neither under way nor ended (another's, or one whose end a
    later ending has replaced) is no sign-in of the page's: Signing in
    shows the way's card again, with its button, and the address drops
    the source.
  - *Details* is the way's field: a key's secret field and "Add key"; a
    program's name field and "Declare". Declaring a program's login
    asks its name, 1 to 32 of `a` to `z`, `0` to `9` and `-`, `claude-`
    and the person's first name by default (`claude-alice`), and shows
    the command to run on each host, `temper program-login NAME`
    (`../protocol/credentials.md`, section 3).
  - *Ready* says the account is made, in 5.5's words, with "Start a
    chat", to the chats page, and a plan's "Manage usage".
  - *Its steps,* the flow's list (5.2, A flow's parts): "Choose",
    "Signing in" and "Ready" for a sign-in; "Choose", "Details" and
    "Ready" for a key or a program's login, and at Choose.
  - *Its address* is its way (`browser.md`, 2.3): none at Choose; a
    way's provider and kind once one is chosen, Details, Signing in
    and Ready all at the way's address. A key's and a program's card
    on Choose is a link to the way's address, Details; a sign-in's is a
    button that sends the sign-in and moves to the way's address,
    Signing in. Each step but Choose offers "‹ All ways", to Choose. A
    way the configured providers do not offer, or the demo's
    provider's, shows Choose.
  - *At a sign-in's address with none under way,* as a link to it, a
    reload or the banner's way opens it, Signing in shows the way's
    card again, with its button, which sends the sign-in. A sign-in
    started from another client is not this page's, unless its address
    names it by its source (above): its end is the accounts page's.
  - *Its state and acts* are a page of the domain's own, its way, its
    key's value, its program's name and its request, built from the
    accounts' requests (jig's `client/domain/contract.md`, 3.2); the
    welcome's Model step holds the same state (below). A move between
    its steps keeps them; leaving the flow drops them, the key's value
    first.

  The first run reaches it by the demo's banner's "Connect a model"
  (5.6), and its "Continue with ChatGPT" signs in with the default
  document's OpenAI (`../protocol/setup.md`, 8.3).
- **The forges' page** is the forge's part of the deployment's
  connectors (jig's `client/domain/pages.md`, section 7), on a page of
  temper's own (`forge.md`, 5.4). It is read for a person whose role
  on project 0 is `Owner`, in standalone the deployment's owner; any
  other's read is refused, and the page says so in jig's words, and a
  request the root refuses for the role says "Your role does not hold
  the deployment's forges." Its acts are the FORGE
  requests (section 8), each sent as 3.6
  says; a pasted key or token is a secret field (3.6). A connect's or
  an install's link is where the page's own tab goes, as a sign-in's
  address is (`ConnectModel`, The tab's departure), the request naming
  the page's address as its `back` (section 8); GitHub's last step
  sends the tab back there (`forge.md`, 9.1), and the page shows the
  link as an external link meanwhile (6.5). How a
  connect ends is read on the page, never assumed from its answer.
- **The welcome** (`welcome.html`) is one page, `Welcome { step }`, its
  step Welcome, Forge, Model or Ready, at `/welcome` and
  `/welcome?at=forge`, `model` or `ready` (`browser.md`, 2.3); its
  words are 5.6's.
  - *Its frame* is temper's name, the person's name and "Sign out"
    alone: no project, navigation or link state, and neither the
    accounts' banner nor the demo's nor the setup strip (5.1). Each
    step's heading is the page's level-1 heading.
  - *Whether it shows:* at its Welcome step, the page decides once each
    time it opens, when its facts are in. The first run is over
    when a model is usable, and a forge other than the
    demo's (`application.demo.forge`) is connected in the `forges`
    watch's rows; for a person whose role does not hold the
    deployment's forges (above), the forges are not theirs to set up,
    and the model alone decides. A model is usable, with no demo, when the
    `accounts` document says so, its `usable`, which counts the
    configured model too (jig's `client/domain/contract.md`, 5.1); and
    with a demo, when an account of that document is usable whose LLM
    provider is not the demo's (4.8), and, for an owner, as 4.8's
    model's half counts the deployment's. Its facts are in once the demo's
    read has answered, a demo or none, the `accounts` document is
    delivered, its accounts as far as it shows them, and, where the
    person's role holds the forges, the `forges` watch has delivered
    its rows. A read refused, failed, ended or not served is in, with
    nothing in it: no demo, no account, no forge, so that the page
    never waits on a read that will not answer. A demo ended reads
    none, as no demo does (4.8): its end was a forge of the person's
    connected, so the page moves on once a model is usable.
    Over, the page moves on to the chats page, its address replacing
    the welcome's, so that back does not return to it; a running demo
    (4.8) is the person's choice made, and the page moves on the same
    way. Otherwise it shows its step, and until its facts are in,
    its heading alone. Its other steps never move on of their own.
  - *"Try the playground"* sends `StartDemo` while the demo has not
    started (4.8) and opens the chats page, in the demo's project once
    the demo's read names it, which the start's first act makes
    (`../draft/first-run.md`, 2.4); until then the chats page shows the
    demo's start under way, as in that project.
    *"Set up temper"* opens its Forge step.
  - *Forge* is `ConnectForge`'s Choose and Details (`forge.md`, 5.4),
    its choice in the welcome's address (`browser.md`, 2.3); inside a
    choice, the foot's one "‹ Back" returns to Forge's Choose. The
    welcome has no "‹ All forges"; `ConnectForge` keeps it. A forge
    already setting up, connecting or connected is no choice: the
    step says it, "FORGE · CONNECTION",
    with "View", to `ConnectForge` at it; but a GitHub forge with no
    accepted installation, or with none of its repositories adopted,
    shows `ConnectForge`'s Connecting or Ready in the step, its `back`
    the welcome's Forge step, `/welcome?at=forge&forge=N`, so that the
    tab GitHub sends back finds its choice there. Once a repository is
    adopted, the welcome moves on to Model. Once "Set up a Forgejo here"
    is accepted, the welcome moves on to Model, the set up running on.
    *Model* is `ConnectModel`'s Choose and Details, its state and acts
    (above), its way in the welcome's address (`browser.md`, 2.3), "‹
    All ways" back to its Choose, so that an account added there is the
    accounts page's too; a sign-in sends the browser back to the
    welcome at its Ready, `/welcome?at=ready` (`ConnectModel`, The
    tab's departure). Until it does (`ConnectModel`, Signing in),
    the step's body draws the sign-in as `ConnectModel`'s does, its
    address and then its end, and an account made there `ConnectModel`'s
    Ready. Above
    them, once a forge is acted on, the forge's line, so that Model
    says what Forge did: while it sets up or connects, "FORGE ·
    CONNECTION · ORG/NAME, into PROJECT", and a disclosure, "Each
    step", holding `ConnectForge`'s Setting up steps; once connected
    with its first repository adopted, ticked, "✓ FORGE · ORG/NAME, in
    PROJECT", on GitHub the repository Ready adopted; PROJECT reads
    "the new project NAME" for a new one. A GitHub forge with none
    adopted yet reads "GitHub · no repository chosen yet", as a forge
    connected with no first repository reads "FORGE · no repository
    chosen yet"; a set up or a first repository that failed keeps all
    five steps, with its failed step marked ✕, preceding steps done
    and later steps to come, followed by "STEP failed: WHY"
    (`forge.md`, 5.4). With no failed step named, nothing is marked and
    the line is "Set up failed: WHY". The line is one
    paragraph, from the facts Ready reads on the forge's row (the
    person's forge, its connection and its first repository, or the
    set up's request until the row names it), and every line but the
    ticked one keeps "Each step". *The seam:* the Model step's
    body is `ConnectModel`'s, the step's frame, heading, lead, set up
    line and foot the welcome's.
    *Ready* is a checklist, under "Almost ready" while work is under
    way: Model, Forge, then Project. Forge reads
    "Forge: FORGE, connected as LOGIN" or
    "Forge: FORGE · CONNECTION" (with its "Each step" folded), or
    "Forge: none yet" with "Connect a forge", to the Forge step; while
    a model is usable, as *Whether it shows* says (the one test Ready and the
    setup strip share), "Model: ACCOUNT", the first usable account's
    heading (5.5, An account), followed by " · MODEL" where the model
    and its account are known: the `accounts` document's first usable
    row, else, for an owner, the first of the deployment's accounts
    that counts for the demo's model half (4.8), never the demo's
    own; or "Model: MODEL" where no such account
    is usable (the configured model), MODEL a new chat's as the
    composer's line names it (5.5); or, while
    a sign-in of the person's is under way on the `accounts` document
    and no model is usable, "Model: PROVIDER · signing in", PROVIDER
    its LLM provider's name (5.5), with no act, so that a person come
    back from the sign-in reads it as under way; or "Model: none yet"
    with "Connect a model", to the Model step; and no model line until
    the `accounts` document is delivered; and "Project
    PROJECT, with ORG/NAME" once the forge's first repository is on its
    row. Until adopted, "Open PROJECT" is disabled, with "It opens once
    ORG/NAME is made." Once that repository is adopted (`forge.md`,
    5.4, Ready), its heading is "ORG/NAME is ready", its checklist
    done, with "Open PROJECT", to the chats page in that project; while the forge is connected with no first
    repository, "Adopt a repository", to `Adopt` into a new project
    (4.4). Ready is
    done, not current, along the top once its forge and model both
    are.
- **With no project yet** (`../draft/first-run.md`, 2.4): until the
  playground's start or the person's first repository makes one, the
  chats page's composer and "Your chats" give way to the level-1
  heading "No project yet" and "A project starts with its first
  repository.", with "Set up temper", to the welcome's Forge step,
  while no forge of the person's is connected, else "Adopt a
  repository", to `Adopt` into a new project (4.4); and, while the
  demo is offered and not started, "Try the playground". The frame
  keeps the setup strip (5.1); the navigation offers no project's
  entries (5.3), and the project's place in it reads "No project
  yet".
- **A suggested request** (`chats.html`) starts a chat with the
  cue's words as its first message, as the composer's send does: one
  act, one request (3.6), the suggestions and the send disabled while
  it is pending. The engine puts that message in the run's brief, and
  the demo model finds the cue there (`../protocol/setup.md`, 8.4), so
  the chat plays the cue's script; the composer's text is left as it
  was.
- **The work's account and model.** A chat's composer names the
  account and model its next run takes in one line, the composer's line
  (5.5), and under it offers the models `me`
  serves for the project (jig's `client/domain/contract.md`, 5.3), each
  with its efforts, and "needs an account" beside an LLM provider no
  usable account of the person's work serves; a choice is the task's
  LLM choice, from the chat's next run (jig's `domain/tasks.md`, section
  3). The offer is the chooser, a disclosure named "Change model"
  (skein's `ui.md`, its disclosure): a browser shows it folded to its
  name until the person opens it, while the terminal and the fake
  person find its content whole. In it, in order: each model's LLM
  provider, its readiness and its button, the chosen model's efforts,
  "Reset choice", and, on a task's composer while the draft differs
  from its choice, "Next run if applied: MODEL" (or "…: engine
  default") and "Apply to next run". A request in flight and its
  outcome, a task's wait for its account, and a refused next run stay
  outside it, after the line. A refused next run says "Next run refused
  · WHY", WHY the refusal's cause in the words of jig's reference view
  (jig's `client/domain/client.md`, 19.1), each sentence ending with a
  full stop: for the model, "This model runs through a program. A chat
  can't stay open through a program yet. To chat now, choose a model
  that doesn't run through a program."; for an account's source,
  "Account A would use its program source S. A chat can't stay open
  through a program yet. This chat does not switch to another source on
  its own."; for a host's hello, "An engine host was refused at HH:MM"
  or "A worker was refused at HH:MM" (5.12's clock), then its cause, in
  the words jig's reference view gives a hello's refusal. A refusal is not a reply: the chat
  starts nothing, and keeps its composer, so the person can choose
  another model. The demo's model stays offered as configured once another
  account serves the work. Setting a goal names its planner's model and
  effort, the template's by default. A goal's page shows its model, effort and
  account, changed by amending the task, and an agent's task's page
  shows them in its composer's line, as a chat's (5.11); a task waiting
  for its account
  says so, with the account's acts (5.5; jig's `domain/authority.md`,
  8.3).

### 3.6 Requests and confirmations

- **One act, one request,** keyed by the domain and sent through
  `jig-client`, which keeps it until its answer is durable (jig's
  `client/domain/client.md`, section 6): a double press joins the
  pending one, and nothing is shown done before its answer.
- **jig's operations, through jig's helpers** (jig's
  `client/domain/client.md`, section 19; jig's
  `client/domain/contract.md`, 3.2 and 3.4): starting a chat is
  `shapes::chat`; setting a goal is `shapes::set_goal`, a proposal where
  the person's authority does not cover it; closing a chat is
  `shapes::close_chat`, one compound request, with what becomes of each
  live delegate; reordering goals is one compound request of priority
  changes; amending and releasing a held task is one compound request.
  Every other act is one operation.
- **Said before it is sent.** An act that cannot be undone (cancelling,
  closing a chat, accepting, rejecting, releasing) says what it will do
  before it is sent, from the facts the document gives (a decision's
  facts, as `jig-client`'s `facts` reads them): what accepting grants
  and costs and the pool after it, what cancelling ends, what releasing
  does (jig's `client/domain/objects.md`, section 1; jig's
  `client/domain/contract.md`, 5.4). Closing a chat says what becomes
  of each live delegate ("Delegate N will be cancelled" or "Delegate N
  will be moved to you", with "Cancel delegate N" and "Take delegate N"
  to choose), then how the chat ends: "Close chat: allow N seconds to
  finish, then cancel", the template's grace, while a run of it is
  live, and "Close chat now" while none is, since a chat at rest closes
  at once (jig's `domain/tasks.md`, 5.8); then "A closed chat can't be
  reopened.". On a task's page, stopping is
  confirmed too, since it holds the task until it is released (5.11);
  a chat's Stop is sent at once. Where the document offers an act
  as proposable (jig's `client/domain/contract.md`, 5.3), the card says
  so, and to whom; where it offers it as refused, the card says why and
  sends nothing.
- **It names what the person saw.** Each deciding act names the
  revision of the document the card showed; when the engine refuses it
  as moved, the card asks the person to look again (jig's
  `client/domain/contract.md`, 5.2). jig's check holds it in temper's
  worlds (section 10).
- **Answered:** the card shows the outcome, or the refusal in jig's
  words with what fits (look again, change what was asked); unknown,
  past the engine's retention, it says so.
- **A key is pasted once, never kept.** Its field is a secret (skein's
  `ui.md`, 3.2 and 3.4), and adding or replacing a key is a request
  carrying a value (jig's `client/domain/client.md`, section 6; jig's
  `protocol/client.md`, section 10): sent once under a name, never in
  the small store, never parked, never sent again by the client. The
  field is emptied as the request leaves, and the key is shown nowhere,
  then or after. Answered unsent or lost, the card says so and asks for
  the key again; the person's second send keeps the first's name, so
  that a request the engine made answers with its first answer, and no
  second account is made. A key empty or past 4 KiB (the bound of
  `tools.md`, section 12) is refused before it is sent; one the
  engine refuses is answered with its refusal.
- **A forge's pasted value is sent the same way.** A GitHub App's
  private key (with no webhook secret: GitHub has none of temper's),
  a Forgejo bot's tokens and their webhook secret, a site
  admin's token, and a replacement of any of them are secret fields,
  and the request is a request carrying a value (jig's
  `protocol/client.md`, section 5): the value is sent once, never in
  the small store, never parked, never sent again by the client, and
  the fields are emptied as the request leaves. The request names no
  number for it and the client draws none: jig numbers the value as it
  keeps it (`../protocol/credentials.md`, pasted values). Answered
  unsent or lost, the page asks for the value again, and the second
  send keeps the first's key. A value empty, or a bundle past 4 KiB,
  jig's bound on a carried value (the bound of `tools.md`, section 12),
  is refused before it is sent, in the words of `forge.md`, 5.4; one
  the engine refuses, past the
  deployment's `records.value_bytes` among them, shows its refusal
  (section 8). The client reads no configuration, so its bound is jig's.

### 3.7 The amendment editor

One editor drafts a person's amendment of a task, for every page and
command that amends one: the task's pages (5.8, 5.9) and `temper ctl
amend` (`tools.md`, 5.5). It is `temper-client-domain`'s `amend`
module, beside `acts.rs`, and pure: it reads a document and builds
jig's `Amendment`; it sends nothing, judges no bounds and keeps no
clock. Each caller lays out its own fields, compares the change with
the task's offered `amend` act by jig's admission (`c::admission::
amendment`), and sends it, as `Amend` or `Propose`, as 3.6 says.

- **A draft is made from the task's document** as read,
  `amend::Draft::of(&c::Task)`: its base is the document's
  `spec_revision`, and the controls the amendment names, as the
  document shows them, the same as jig's `OwnerAmendmentState` holds
  them, with the tracking and the LLM choice: its tracking (`priority`,
  tracked when supplied), its spec (`SpecificationDocument::whole()`),
  its authority (`OwnerAuthorityDocument::whole()`), its instructions,
  its dependencies (its links' numbers, in order) and its `llm`.
- **An edit sets one control,** whole, and leaves the rest as the
  base holds them:

  | Edit | Sets in the `Amendment` |
  |---|---|
  | `track(Tracking)` | `tracked`: `Tracked { priority }` or `Untracked` |
  | `deadline(Option<Wall>)` | `authority`: the base's, with its `deadline` replaced |
  | `instructions(Text)` | `instructions`: whole; empty is none, explicitly |
  | `words(Text)` | `spec`: the base's, with its `words` replaced |
  | `dependencies(List<u64>)` | `dependencies`: the whole list |
  | `llm(LlmChoice)` | `llm`: the whole choice |

  An edit equal to the base is no change. A deadline needs the base's
  whole authority and words its whole spec: where the document cut it,
  the edit answers `Cut::Authority` or `Cut::Spec`, and the draft is
  unchanged, so a page shows the field read only and `ctl` says so.
- **Its revision.** `Draft::revision()` is the base's. `Draft::moved
  (&c::Task)` says whether a document read since has another
  `spec_revision`: a page's draft is then stale, and says so before
  sending; `ctl` compares it with its `--rev R`.
- **The change,** `Draft::amendment() -> Option<Drafted>`: none
  without a change; else `Drafted { revision: Option<Revision>,
  change: c::Amendment }`, the revision absent exactly when the change
  is a tracked task's priority alone (jig's `domain/tasks.md`, section
  7), and the base's otherwise.
- **What is whose.** The editor holds the rules of a draft (what
  each edit keeps, what is cut, when a revision is named). The words
  and fields are the caller's: a page's form, `ctl`'s flags and its
  lines on stderr.

## 4. The forge's client family

The forge's client side is the client root's child for the forge
connector, an owner numbered as its connector (jig's
`client/domain/root.md`, sections 2 and 15). It shows the forge's parts
that need no document, reads the documents of the others (`forge.md`),
holds one form, the adoption, with its picker, sends the forges'
page's requests (section 8), and reads the demo (4.8).

### 4.1 Words that need no document

A connector, a resource or a kind needs no document: its words come from
its number or its path.

- **The connector:** "forge".
- **A resource,** by its path (`../domain/forge.md`, section 2):

  | Path | Words |
  |---|---|
  | `forge` HOST OWNER NAME | OWNER/NAME on HOST |
  | … `branch` S1 … Sn | branch S1/…/Sn of OWNER/NAME on HOST |
  | … `pull` N | pull request #N of OWNER/NAME on HOST |
  | … `issue` N | issue #N of OWNER/NAME on HOST |
  | two segments: a forge's number (2 bytes) and a repository's id (4 bytes), big-endian, as an adoption names it | repository ID on forge F |
  | three segments: a forge's number (2 bytes) and a repository's id (4 bytes), big-endian, then a branch's name, as the hub names a delegate's branch (`temper-engine-domain`'s forge route) | branch B of repository ID on forge F |
  | anything else | its segments joined by `/`, as they are |

  jig's documents name an adopted repository by its forge's number and
  its id (`../domain/forge.md`, section 4), and the client shows those;
  the forge's documents name it by owner and name (`forge.md`, 2.5).
- **A resource pattern** (a grant's): the words of its segments as
  above, its last segment as it is; an open end is its prefix followed
  by `…` ("branch temper/7/… of acme/shop on forge.example").
- **A kind,** by the forge's numbering (`temper-engine-domain-forge`'s
  `effect_shape`; `../domain/forge.md`, section 6):

  | Kind | Words | Kind | Words |
  |---|---|---|---|
  | 1 | read | 9 | create a branch |
  | 2 | push | 10 | edit a pull request |
  | 3 | open a pull request | 11 | update a pull request from its base |
  | 4 | merge a pull request | 12 | close or reopen a pull request |
  | 5 | post a review | 13 | edit, close or reopen an issue |
  | 6 | set a commit status | 14 | set reviewers |
  | 7 | post a comment | 15 | delete a branch |
  | 8 | create an issue | any other N | kind N |

  These are the forge client's writes in words (its `Write`
  vocabulary), with reading and pushing. A new kind is a new row here
  in the same change that numbers it.

### 4.2 The forge's other parts

Every other part of the forge's that jig's documents name (a panel, a
result, an effect, an outbox entry, a hold, a project's part, the
deployment's part: jig's `client/domain/root.md`, section 6) needs a
document, which the forge's side reads or watches through the family's
own reads and subjects (`forge.md`, section 9). Wherever a page would
show a part the forge's side does not read, temper's view shows one
paragraph:

> Shown on the forge.

Such a part never shows loading, and never an error. A change's
procedure part on its task's page is not such a part: the forge's side
watches its panel (`forge.md`, 9.2), and the page draws it, or says it
is loading, unknown or cannot be shown (5.10).

### 4.3 Where the form is

The form is a page of temper's, `Adopt { project }`, reached from the
navigation's "Adopt a repository" (5.3); its place among a project's
settings is open (section 11). The outcome's sentence (4.7) is what
tells the person a repository is adopted: no page of temper's lists a
project's resources (section 11).

### 4.4 The adoption form

The page's title is "Adopt a repository into PROJECT", PROJECT as `me`
names the project. For a project `me` does not name (before `me`'s
first delivery, or one no longer among the party's projects), it is
"Adopt a repository into project N", N the address's project number,
until `me` names it. The form is the same either way: the engine
judges the adoption, and a refusal at admission (the party's role in
the project) is shown in jig's words (4.6). Its fields are
`temper-engine-domain-forge`'s `Adoption`'s but the repository's id,
which the forge connector locates by owner and name
(`../domain/forge.md`, section 4), and the forge's host, which temper's
server fills from the forge's configuration (section 8). A row chosen in
the picker, below the forge's field, fills the owner and the name (the
picker, below). In order:

| Field | Node, named | Default | Rule |
|---|---|---|---|
| forge | text field "Forge number" | `1` | a number from 1 to 65535; one the engine has not configured is refused as no such forge |
| owner | text field "Owner" | empty | required |
| name | text field "Name" | empty | required |
| role | choice group "Role": "Owned: temper pushes, opens pull requests and lands", "Fork: temper contributes through a fork", "Context: temper only reads" | owned | one of the three |
| home | check "Make it the project's home" | checked | a context repository cannot be the home |
| prefix | text field "Branch prefix" | `temper/` | required |
| landing | text field "Landing branch" | empty | required: the forge connector refuses an adoption without one |
| ci | check "It runs CI" | checked | without CI, at least one check |
| checks | text field "Checks: gate numbers, separated by spaces or commas" | empty: none | numbers from 1, each once, at most the client's limit on checks |

Then a button "Adopt", and below it the request's state or outcome.

**Into a new project** (`../draft/first-run.md`, 2.4): `Adopt` with
no project, from the welcome's Ready and the chats page with no
project yet (3.5), is titled "Adopt a repository into a new project",
and adds a last field, "Into a new project, named", the name's field
following the picker's chosen repository until the person changes it,
required. "Adopt" sends jig's `MakeProject` of that name and policy 1
(`../protocol/setup.md`, 8.3), then `Adopt` into it, as its home; a
name taken is jig's refusal, shown under the field. Until jig makes
projects at runtime, the form is refused "a new project", as `SetUp`'s
is (4.6).

**The picker,** between "Forge number" and "Owner": the repositories
temper can reach on that forge (`forge.md`, 5.1), the project's own
among them (section 11), a page at a time by number, each page the
answer to a `Reachable` request (section 8) the form sends when the
forge's number is written and at each of the picker's buttons, beside
the forge's deployment part, read as the forges' page reads it (3.5),
for its kind, its user's login and its installations. Its nodes:

| Node, named | Shows | Acts |
|---|---|---|
| choice group "Account", GitHub alone | a choice per accepted installation the forge's read names, its account's login, followed by ", selected repositories" where its installation selects them and ", suspended" where GitHub suspends it; the first is chosen when the forge's number is written | a choice reads that installation's first page, the request's `installation` its id |
| list "Repositories" | an item per row holding a button named "OWNER/NAME", or "OWNER/NAME, private" where the forge says it is private; at most the answer's rows, `picker.page` (`../protocol/configuration.md`, its limits) clipped to the forge's own page cap, within the client's limit on a page's rows | a row fills "Owner" and "Name" and keeps the row's ids for the request (4.6) |
| buttons "Next" and "First page" | "Next" while the answer says more follow (`more`, set when the page is full); "First page" past the first | the page after the one shown, by its number, or page 1 again |
| a status under the list | "Reading forge F's repositories…" while the request is pending; "None reachable." for an empty first page; a refusal, below | — |
| "Not listed" | under "Owner" and "Name" typed by hand, of no row read, what would make the repository reachable, from the forge's read and its installations, in `forge.md`, 5.1's words: on GitHub "Grant OWNER/NAME to temper's App on GitHub's installation page." linked to that installation's page, or "temper's App is not installed on OWNER." with "Install on another account"; on a connected Forgejo "Make LOGIN a collaborator with write on OWNER/NAME."; on a managed or owned Forgejo, nothing | "Install on another account" goes to the forges' page (3.5) |

- **The ids ride, never shown.** A row carries the forge's id for the
  repository and its owner's id, besides the owner's login, the name and
  whether it is private; the form keeps the two ids for the `Adopt`
  request while "Owner" and "Name" are the row's, and drops them when
  either is edited by hand (4.6).
- **Refused, in words.** A forge not connected (`NotConnected`):
  "Forge F is not connected.", linked to the forges' page, and "Adopt"
  disabled for that forge until a later page lists its repositories. A
  forge not configured: "No forge F is configured." Refused for the
  person's role, which does not hold the deployment's connectors:
  "Your role does not hold the deployment's forges: give the owner and
  the name.", and the person adopts by owner and name, the
  engine judging the adoption as any other (section 11). A refusal at
  admission: jig's words. A forge that did not answer: "Forge F did not
  answer.", with "Try again". Busy, another read of the forge's picker
  running or the setup's host full: "Forge F is busy.", with "Try
  again". On a managed or owned Forgejo, the admin's token missing or
  not a site admin's: "Forge F's admin's token is missing." or "Forge
  F's admin, NAME, is not a site admin.", linked to the forges' page.
  None of these empties the fields.
- **One page at a time.** The form holds the last page read, its
  number and whether more follow, never earlier pages; a page answered
  after the person chose another forge, installation or page is dropped. A
  page past the last answers no rows, as "Next" after a last page that
  came back full does, or one asked after the list shrank, and the form
  says "No more repositories." with "First page".

- **Every field is bounded** by the client's limit on a field's bytes,
  set no larger than the forge connector's name bound in temper's small
  profile (`temper-engine-domain-forge`'s `name_bytes`). A value past
  it is not taken: the field keeps its last value and is marked
  invalid, with the bound in words ("At most N bytes.").
- **Checked before sending:** each rule of the table that the draft
  alone decides (all but whether the forge is configured) is the
  form's own check, never one of what the person may do. A field that
  fails is marked invalid, with one status line under it saying the
  rule in words ("Required.", "A forge number is from 1 to 65535.",
  "Checks are numbers from 1, each once.", "A context repository
  cannot be the project's home.", "Without CI, name at least one
  check."), and nothing is sent. The engine checks everything again.
- **Adopt** makes one keyed request through `jig-client` (4.6), as the
  forge's family's bytes, made once (jig's `client/domain/client.md`,
  section 6). While it is pending the button is disabled and a status
  says its progress as `jig-client` gives it, in jig's words.
- **The forge asked by number, home checked.** `me` names neither the
  configured forges nor whether the project has a home, so the form
  asks for the forge by number and checks home by default; the engine
  refuses a forge it has not configured and a second home, saying
  which. With the forge's project part read, the form chooses among the
  configured forges by name and says whether the project has a home
  (`forge.md`, 5.1).
- **After the outcome,** the outcome stays shown until the person edits
  a field or leaves the page. Adopted, the fields go back to their
  defaults (written by the domain, so the view writes them).
- **Out of date:** when the engine does not serve this client's version
  of the forge's records (jig's `protocol/client.md`, section 9), the
  page says "This temper is out of date for adopting repositories: run
  the temper the engine runs.", the button is disabled, the picker
  sends nothing, and a pending adoption stays in the small store.

### 4.5 Its address and state

- **Address:** `Adopt { project }`, one of temper's addresses (3.5).
- **State:** each field's text and the domain's count of its own
  writes; the pending request's key and progress, at most one; the last
  outcome; the version range the engine serves for the forge's records;
  the picker's installation chosen, its last page (its number, its rows
  and whether more follow) or refusal, its pending `Reachable` key, at
  most one, and the chosen row's ids. Bounded by its limits: the field
  bytes, the checks, a page's rows.
- **Records:** none of its own; its pending adoption is `jig-client`'s
  kept request (4.6).

### 4.6 The records

`temper-client-protocol-wire`, generated from one schema (jig's
`protocol/client.md`, sections 3 and 11):

- **`FORGE`:** the forge connector's number, 1, which is the family's
  owner number on the wire, under which its records travel as bytes
  (jig's `protocol/client.md`, section 11), and the engine's one
  configured connector (`engine.connectors`,
  `../protocol/configuration.md`, section 2).
- **`Owner`:** temper's owners, one enum the engine's root and the
  client root share, with the functions from an owner to its number and
  back (jig's `client/domain/root.md`, section 15): `Forge`, `FORGE`'s
  number; `Pages`, temper's pages, 2; and `Runner`, the agents' tools'
  runner (`tools.md`, section 4), 3: numbers no connector takes, never
  reused. `Pages` and `Runner` ask only for jig's operations, reads and
  subjects, so no request, read or subject of their own reaches the
  engine.
- **`CLIENT_VERSION`:** the version of every record below, 1 at first,
  2 since the demo's read, 3 since a change's panel, 4 since a forge's
  set up and the deployment's forges, 5 since jig's client wire 13
  carried in its refusals, 6 since the first repository (`SetUp`'s
  fields and the row's `first`), 7 since the demo's state (the demo's
  read's `state` and `StartDemo`), 8 since a sign-in's return
  (`Return`, section 8), 9 since connecting a GitHub App (`Connect`,
  `Install`, `ForgeRefusal`), 10 since the forges row's installations
  and last connect's end, 11 since GitHub's manifest (`Connect`'s
  `GitHubManifest` and the row's `app`). It stays at 11 until the first
  release, as `../protocol/README.md`, section 9 says (skein's
  `codec.md`, section 5). A request, an outcome, a read, a subject or a
  field added to the family (the FORGE requests of section 8, the
  picker's ids in `Adopt`, the forge's kind in the adopted outcome, the
  demo's read, a change's panel) changes its schema, generated code and
  one set of golden bytes in place. The version moves at a release.
  The engine serves exactly one version (jig's `protocol/client.md`, section 9): a client of another
  is out of date for the forge (4.4), and its kept adoption is dropped
  as kept work at start (6.6; jig's `protocol/client.md`, section 10).
- **The request, `Requesting`:** which of the family's requests it is,
  `Request`, as `Reading` names a read: `adopt`, `Adopt`, `set_up`,
  `SetUp`, `start_demo`, `StartDemo`, `connect`, `Connect`, `install`,
  `Install` (below), or `return`, `Return`, answered `returning` (section 8). Each is answered by the family's one record,
  `Answer`, whose outcome is one of either request's (below); the client
  refuses as undecodable an outcome that does not fit the request it
  sent.
- **`Adopt`:** the project (u32); the forge's number
  (u16); the owner, the name, the prefix and the landing branch (byte
  strings); the role (owned, fork or context); home and ci (booleans);
  the checks (a list of u32); and, when the owner and name are a picker
  row's, the forge's id for the repository and its owner's id (u64
  each, both or neither).
- **Its outcome,** `Answer`'s:
  - *adopted,* with the repository as the forge connector recorded it:
    its forge's number, its kind and its id; its owner and name; its role;
    whether it is the project's home; what temper may do there (the
    forge's `Kinds`: read, push, open, land, review, status, comment,
    issue, branch); whether branch protection was read (unknown,
    absent, or a rule); its merge style; and the landing branch's rules
    as read, each with the setting that would let temper land where it
    keeps temper from landing (`forge.md`, 5.1);
  - *refused by the core,* when the core refused the adoption the root
    made on the party's behalf: jig's refusal facts, carried whole as
    `jig-client-protocol-wire` encodes them, and shown in jig's words
    for its refusals;
  - *refused by the forge,* with the forge connector's reason: no such
    forge; not connected; not installed; not granted, with the
    installation's page; not found; no access; refused; in flight; too
    large; unreachable; invalid answer (4.7).

  A refusal at admission (signed out, the party's role in the project,
  a key sent again with another request, a limit) is not an outcome:
  it is jig's answer, which never reaches the connector (jig's
  `client/domain/contract.md`, 3.3), and the form shows it in jig's
  words.
- **`SetUp`:** of the deployment's scope (section 8): the forge's
  number (u16); its first repository's org and name (byte strings,
  bounded as `Adopt`'s owner and name); and its project, `SetUpProject`:
  `existing`, a project's number (u32), or `new`, a new project's name
  (a byte string bounded as `Adopt`'s name) (`../protocol/setup.md`,
  6.7). Its outcome, `Answer`'s: *setting up*, the box asked
  for the forge's group; *set up refused*, with why: not offered (the
  forge's connection does not offer it, or no such forge), not under
  the box, busy (`../protocol/forge.md`, 2.8), a new project, which
  waits on jig's projects at runtime, or invalid, a name Forgejo
  would refuse (the org a login of at most 40 bytes, the repository
  1 to 100 bytes of letters, digits, `.`, `_` or `-`, neither `.` nor
  `..`; skein `forge-setup.md`, 4.9) or past the engine's
  `name_bytes`, or a new project's name empty or past it; or *unknown*, its
  outcome no longer held (section 8), the same unknown as an
  adoption's. A refusal at admission is jig's answer, as for `Adopt`.
- **`StartDemo`:** of the deployment's scope (section 8), with no
  fields: the demo is the document's (`../protocol/setup.md`, 8.4).
  Its outcome, `Answer`'s: *running*, the demo's record running, its
  model's account kept and its repository adopted; *start refused*,
  not offered (no demo, or the demo ended); *start failed*, with why,
  `StartFailure`: the group's failure, with the box's `WHY` (a byte
  string of at most `box.line_bytes`), the account refused, the
  adoption refused, the acts not granted (`not_granted`, an older
  document without `engine.acts`' grant), or the forge not connected in
  time; or *unknown*,
  its outcome no longer held (section 8), as `SetUp`'s. A refusal at
  admission is jig's answer, as for `Adopt`.
- **`Connect`:** of the deployment's scope (section 8): the forge's
  number (u16) and how it connects, `How`: `github_app`, `GitHubApp`,
  the App's id (u64, never 0). The manifest's way and the Forgejo ways
  join `How` with their work (section 8's table). The App's key is no
  field: it rides beside the record as jig's carried value, a bundle of
  one `Pasted` item (`../protocol/credentials.md`, 2.2). Its outcome,
  `Answer`'s: *connecting*, the run started; *forge refused*, with why,
  `ForgeRefusal` (below); or *unknown*, as `SetUp`'s.
- **`Install`:** of the deployment's scope: the forge's number (u16)
  and `back`, an optional byte string checked as `Return`'s `path`
  (section 8). Its outcome: *a link*, `Link`, the install page's
  address (a byte string of at most 2048 bytes); *connecting*, an
  `Install` that stopped, its failure said on the forge's row; *forge
  refused*, `ForgeRefusal`; or *unknown*.
- **`ForgeRefusal`,** a `Connect`'s or an `Install`'s, by the root
  before any run: `role`, the sender not the deployment's owner; `no
  such forge`; `not taken`, the forge's kind or mode does not take the
  request (a Forgejo, the demo's fake); `connected`, a `Connect` on a
  forge connected or connecting already, whose key `Replace` replaces;
  `not connected`, an `Install` on a forge with no App; `busy`, a run
  holding the forge, or `setup.runs` full; `no key`, a `Connect` of an
  App with no carried value. Its carried value, if any, is named unused
  (section 8).
- **The small store's record of a pending request** is `jig-client`'s
  kept request, its request encoded by this crate as the family's bytes
  at `CLIENT_VERSION` (jig's `protocol/client.md`, section 10).
- **The FORGE requests and their outcomes:** `Connect`, `Install`,
  `Replace`, `Forget`, `SetUp`, `StartDemo` and `Reachable`, beside
  `Adopt` (section 8).
- **The demo's read, `Demo`:** of the deployment's scope, with no
  fields, and no subject: what it answers changes with the
  configuration, at a restart, and with the demo's state, which moves
  to running only by a start (4.8). Its answer is none, for a
  deployment without the demo, or the demo: its
  project (u32), once its start has made it, else none
  (`../protocol/setup.md`, 8.4); its forge's number (u16); its model's LLM provider
  (u32); its repository's owner and name (byte strings, bounded as
  `Adopt`'s); its state, `DemoState`, not started, running or ended
  (`../protocol/setup.md`, 8.4); and its cues, at most 8, each 1 to
  256 bytes of text, in the configuration's order (section 8). On the wire it is `Reading`'s
  `demo`, `Reading` naming which of the family's reads a read is, and
  its answer `DemoAnswer`; none is the read's empty answer.
- **A change's panel, the subject `change`** (`forge.md`, 9.2): of a
  project's scope, its window unused, naming the change task's number
  (u64). On the wire it is `Watching`'s `change`, `Watching` naming
  which of the family's subjects a watch is, as `Reading` names a read.
  Its document, `ChangePanel`:
  - the task (u64); the repository's forge's number (u16), its owner
    and name; the change's branch and its landing branch (byte strings,
    each bounded as `Adopt`'s); its head: none before its branch is
    pushed, or a commit (32 bytes, as the forge connector keeps one);
  - its pull request: none yet, or its number (u64) and its address on
    the forge (a byte string of at most 2048 bytes, empty for a forge
    with no `web`);
  - its step: producing, opening, recreating, reopening, checking,
    gating, queued (its place, from 1 (u32), and the tasks ahead of it,
    nearest first, at most 64 (u64 each)), first, updating, resolving,
    repairing (its reason: CI, a gate by its number (u64), or a semantic
    conflict), landing, or landed (its merge commit, 32 bytes);
  - its hold: none, or why: stalled; past its repairs, its resolutions
    or its updates; its branch moved; its branch missing; its pull
    request closed; its pull request retargeted; a delegate or effect
    failed; a person rejected a gate. A held change's step is the one it
    was held at;
  - the task its step runs: none, or its number (u64);
  - CI at its head: unknown, pending, passed or failed;
  - its gates, at most 64, each with its number (u64), its kind (review,
    approval or check), whether it blocks (bool), and its verdict at the
    head: none, pending, approved, or changes asked;
  - its repairs, its resolutions and its updates, each used of its
    limit (u32 each).
- **The deployment's forges, the subject `forges`** (`forge.md`, 5.4
  and section 9): of the deployment's scope, with no fields, its window
  unused; on the wire `Watching`'s `forges`. Its document, `Forges`, a
  `ForgeRow` for each configured forge, at most `FORGES` (16), in number
  order: its number (u16); its kind (Forgejo or GitHub); its mode
  (connected, managed, owned or fake); and its connection: not
  connected; not set up; setting up, with its step when the box has
  named one (fetch, prepare or run); set up failed, with its optional step (fetch, prepare, run or connect)
  and why (text of
  at most 256 bytes, the box's line bound); connecting; connected, with
  the user's id (u64) and login (a byte string bounded as `Adopt`'s);
  or stopped, with the component's reason;
  `admin`, whether the configured string-managed form takes a site
  admin's token (the legacy managed object does not); `ended_stop`, none
  or `AdminStop`: not admin, refused with status (u16) and `SetupStage`
  (admin, bot, token, org, repository, file, app, installation or state),
  unmarked, or other. The diagnostic `ended` stays separate; the client
  selects its stop's words from this typed fact, never its text;
  its first repository, `first`: none, or the set up's org, name, runner
  label (at most 64 bytes) and `SetUpProject` with its state, `FirstState`:
  making; made, with the repository's id (u64);
  or failed, with its step, `CreateStep`: admin, org, repository (its
  bot's access with it) or file (`../protocol/setup.md`, 6.7); its
  installations, `installations`, on GitHub, at most
  `INSTALLATIONS` (64, the configuration's `forges.installations` at
  most it) in id order, empty on Forgejo, each
  `Installation`: its id (u64), its account, none until the forge has
  heard it or its id (u64) and login (a byte string bounded as
  `Adopt`'s), whether it is suspended and whether it is a stray, never
  accepted through temper's link (booleans); its last connect's end,
  `ended`: none, or `ConnectStopped`, why the run of a `Connect` or an
  `Install` stopped, text of at most 256 bytes, in the setup host's
  words (`forge.md`, 5.4); on GitHub, its App's way, `app`: none,
  `Manifest` or `Pasted`, how the App its records hold was connected
  since the engine started, the forge domain's memory alone, so none
  after a restart and while no App is connected (`forge.md`, 5.4,
  Connecting); and its
  offer, the forge domain's (`../domain/forge.md`, 4.1): offered, not
  under the box, or not offered, which is what "Set up a Forgejo here", "Set up again"
  and the commands to run by hand follow (`forge.md`, 5.4). The read of the forges, the
  tools' `forges` (`tools.md`, 5.5, area 6), is the watch's first
  document. *The client's forge side watches it once,* for the
  session, from when `me` is first delivered while the link is live,
  for every person, so that the engine judges the role: a watch
  refused or ended is kept as such, with no rows, and not opened again
  until the next sign-in. Its rows feed the Forges page,
  `ConnectForge`, the welcome, the Forges entry's dot, the setup strip,
  a set up under way in the banners and the demo's forge half (4.8).
- **The forge's documents' records:** its other reads, subjects and
  documents, and its other requests, beside `Adopt` and `SetUp`
  (`forge.md`, section 9).
- **Bounds:** the schema's ceilings bound every record at both ends,
  and the generated worst cases price each. Within them, the client
  bounds its fields by its own limits (4.4), and the engine's forge
  connector refuses what is past its own as too large (4.7), so a
  record a client can make always decodes.

### 4.7 The outcome in words

| Outcome | The form says |
|---|---|
| adopted | "Adopted OWNER/NAME on forge F as ROLE." then "It is the project's home." if so; "temper may: KINDS." in the forge's words; on GitHub, "Commit statuses stay temper's own: GitHub's rules cannot require them."; "Branch protection could not be read." if unknown; "It merges by STYLE."; then each rule as `forge.md`, 5.1 words it, on Forgejo its branch protection alone |
| refused by the core | jig's words for its refusal |
| no such forge | "No forge F is configured." |
| not connected | "Forge F is not connected.", linked to the forges' page (3.5) |
| not installed | "temper's App is not installed on OWNER.", with "Install on another account" (`forge.md`, 5.5) |
| not granted | "OWNER/NAME is not granted to temper's App.", linked to the installation's page on GitHub (`forge.md`, 5.5) |
| not found | "No repository OWNER/NAME on forge F that temper's forge user can see." |
| no access | "temper's forge user may not read OWNER/NAME on forge F." |
| refused | "The forge connector refused it: another project writes OWNER/NAME, the project has a home already, or branches under PREFIX are not this deployment's." |
| in flight | "OWNER/NAME is being adopted already; try again when that ends." |
| too large | "A field is past the forge's bounds, or the forge's repositories are full." |
| unreachable | "Forge F did not answer; nothing was adopted. Try again." |
| invalid answer | "Forge F answered what temper did not expect; nothing was adopted." |
| unknown past the retention | "It is not known whether OWNER/NAME was adopted." |

KINDS lists what temper may do, in this order and these words: read;
push; open pull requests; land; review; set statuses; comment; create
issues; create and delete branches. STYLE is "merge", "squash" or
"rebase". The forge connector maps the forge client's errors to these
reasons (section 8). The links are the forge's configured addresses,
shown as external links and never opened by the client (6.5).

### 4.8 The demo

A first run's deployment is a demo, on a demo model and a demo forge
(`../protocol/setup.md`, 8.4), until the person connects a real one
of each (`../draft/first-run.md`, 3.6).

- **Read once a link is live.** The forge's side reads `Demo` (4.6)
  when the link first becomes live, again each time it becomes live
  after it was not, again once a start it sent is answered, and again
  when the forge's half ends (below), so that the end's none replaces
  a running answer within the session. It
  keeps the last answer, and drops the answer of a read it has
  replaced. None, and no answer yet, both mean no demo is shown.
- **Started when chosen** (`../protocol/setup.md`, 8.4): the demo's
  state is the engine's, not started or running, and ended answers
  none. "Try the playground", the welcome's and the setup strip's,
  sends `StartDemo` while the demo has not started, one start at a
  time, held by the forge's side across pages. While it is out, or
  once it failed, the side holds the demo as running, the demo's start
  saying the start is under way, its suggestions waiting, or why it
  failed, with "Try again", which sends a start under a new key (5.6).
  Answered running, or unknown, the side reads the demo again. It
  sends no start of its own beyond the person's: a start a restart cut
  short is the engine's to mend, with no client
  (`../protocol/setup.md`, 8.4).
- **The model's half** holds while the demo is running (above) and no account
  of the `accounts` document, the one the accounts' banner keeps (5.1),
  of an LLM provider other than the demo's is usable. While that
  document is not delivered, or is cut, the half does not hold. For a
  person whose deployment role is owner, the deployment's own accounts
  count too: the half also needs that no account of the `deployment`
  document held by the deployment, of a provider other than the demo's,
  is usable, a source of it usable as an `accounts` line's is; the
  demo model's own account, the deployment's of the demo's provider,
  never counts. That document counts only by such an account: while it
  is not delivered, or is cut, it ends nothing, and the half reads the
  `accounts` document alone, as for any other person, for whom the
  `deployment` document plays no part. Ready and the setup strip judge
  a model usable by the same accounts (3.5).
- **The forge's half** holds while the demo is running (above) and the
  deployment's part (`forge.md`, 5.4) shows no forge connected but the
  demo's. That part is read only for a person whose role holds the
  deployment's forges (3.5); for any other person, and while the part
  is not served, the half does not hold. The part is the `forges`
  watch's rows (4.6).
- **Ending for good,** with the welcome (`../draft/first-run.md`, 2.3,
  its step 5): the model's half ends when an account of another LLM
  provider that counts for it (above) is first usable, the person's own
  or, for an owner, the deployment's, and does not hold again; the demo ends
  when a forge of the person's first connects, and the read then
  answers none (`../protocol/setup.md`, 8.4). Each act has one owner.
  At the demo's end the engine removes the demo model's account
  itself, as it closes the demo's chats, whether or not a client saw
  the end (`../protocol/setup.md`, 8.4), and the side sends nothing.
  At the model's half's end, which only the client judges, the side
  removes it, when it sees that end while its last answer is running
  and before any demo's end: the deployment's account of the demo's
  provider, which only the `deployment` document lists (jig's
  `client/domain/contract.md`, 7.1: the `accounts` document lists the
  person's own), by jig's `Remove` (jig's `client/domain/contract.md`),
  as the accounts page's "Remove account" sends it, unconfirmed (3.5,
  `Accounts`), once a session, for a person whose deployment role is
  owner; for any other person it sends nothing. The answer shows
  nowhere: refused, the account stays, and Accounts offers its Remove.
  Gone, it supplies no run: the model's half has ended.
- **One `deployment` watch,** for a person whose deployment role is
  owner alone, for both of its uses, the accounts that count for the
  model (above) and the removal: the side holds it open while `me`
  says the role and either the demo is running (above: answered
  running, or its start out or failed), or the side has seen the
  model's half end and the removal is not settled: the `Remove` not
  sent, and the document, at or after that end, still listing such an
  account. The end it saw holds the watch, whatever the demo's read
  answers since.
  Once neither holds, it drops the watch. For any other person it
  opens none.
- **What shows:** the demo's banner, while either half holds (5.1); the
  demo's start, on the chats page, while the model's half holds and
  the person is in the demo's project, or in none while its start is
  out (3.5); the context line, on the
  demo's start and on a chat's page in the demo's project, while the
  demo is running. Each is worded in 5.6. Nothing hides them but
  their halves (section 11).
- **State:** the last answer, the pending read's key, at most one, the
  start's key while it is out, at most one, and its failure, whether
  the link was live at the last pass, whether the demo model's
  account's removal was settled this session, sent or with none to
  send, and the `deployment` watch's key and the demo's provider while
  it is open, at most one.
  Bounded by the record's bounds (4.6).

## 5. The view

### 5.1 Its shape

`temper-client-view` builds temper's pages as skein-ui's tree (skein's
`ui.md`), one owner, temper's:

- **Its state:** skein-ui's view (the last tree, a spare, node ids never
  reused); temper's phrasebook, English, with jig's words for jig's
  enums and a decision's facts (`jig-client`'s `words`) under it,
  overridden phrase by phrase where temper words them otherwise
  ("Funds 40 from your pool · 120 left, 80 after"), and
  temper's own phrase for what `jig-client`'s `words` does not say (an
  executor, an outcome); temper's bindings for the last tree; its
  limits; the instant its words next change (5.12).
- **`build`:** at the shell's time, when the domain's shown count moved
  or that time has reached the instant its words next change (5.12),
  it builds the page in one outline (skein's `ui.md`, section 14), in
  the mockups' order:
  - *a banner:* temper's name; the project the person is in, and the
    others to choose from, each by the name `me` gives it, which no
    other project of the deployment shares (jig's `domain/core.md`,
    6.1), its visible and accessible name the same words. Until jig
    gives projects names (jig's `STATUS.md`, section 3, "A project's
    name"), `me` gives none, and the banner says "Project N", N the
    project's number: the name jig gives a project configuration
    leaves unnamed, so nothing a person sees changes when names come;
    the navigation (5.3) after the projects, as in
    the mockups; the link's state, one word for each of
    `jig-client`'s `Link` states ("Signed out", "Connecting" while
    starting, "Live", "Offline", "Out of date"), and "Behind" while a
    page's watch reopens; the person's name and "Sign out"; all but
    temper's name (text, not a link: the navigation's Inbox and Chats
    lead home; the user's pick, 10-23) in one disclosure, "Menu",
    which a browser folds
    behind its ☰ at phone width alone (`browser.md`, 2.5) and the
    terminal and the fake person find whole; on `Welcome`, temper's
    name, the person's name and "Sign out" alone (3.5, The welcome);
  - *the accounts' banner,* while no account the person's work may use
    is usable (the `accounts` watch; jig's
    `client/domain/contract.md`, 7.1): a region named "LLM account",
    its words and three buttons
    (5.5), on every page but the accounts page, `ConnectModel` and
    `Welcome`;
  - *the demo's banner,* while either of the demo's halves holds
    (4.8): a region named "Playground", its words and its links (5.6), on
    every page but `Welcome`, the accounts and the forges pages,
    `ConnectModel` and `ConnectForge`, a chat's page and the demo's
    start among them, above their chatbars;
  - *the setup strip,* in the demo's banner's place, while the demo has
    not started and no model is usable (3.5, The welcome) or
    no forge of the person's is connected (`../draft/first-run.md`,
    2.3), the welcome's facts in and
    its first run not over (3.5, The welcome): a region named "Setup",
    its words and links (5.6), on the same pages. "Not started" is the
    demo's read answering not started, or none (4.8);
  - in either, *a set up under way:* while a forge of the person's sets
    up or connects, "FORGE · CONNECTION", its name and its connection's
    words, and "View", a link to `ConnectForge` at it (`forge.md`,
    5.4), in the demo's banner in place of "Connect a forge";
  - *the main:* the page at the address (3.5), its title its level-1
    heading.

  Then skein-ui diffs it and keeps the tree.
- **The pages not about the work** (jig's `client/domain/pages.md`,
  section 8). While the link is signed out or out of date, `me` has
  ended (jig's `client/domain/client.md`, 8.2). The banner is then
  temper's name and the link's state alone, no navigation is offered,
  and the main, in place of the page at the address, is one paragraph
  of temper's words with no heading:
  - *signed out:* in the shell's words, which the view takes at
    construction: the terminal's "Signed out." (6.6); the browser's
    "Signed out. To sign in, open a sign-in link `temper serve`
    printed." (`browser.md`, 2.4);
  - *signed out, with a way of signing in:* in the browser alone, in
    place of the paragraph, the sign-in page: temper's mark, the
    heading "Sign in", and a link for each way the view was given at
    construction, "Continue with Google" to `/sign-in/google?then=PATH`,
    `PATH` the page's printed address (`../protocol/sign-in.md`,
    section 7);
  - *signed out by Sign out, with a forge that signs in through
    temper:* the sign-in page, under its heading, for each such forge,
    "You signed out. FORGE keeps its own sign-in: sign out there too.",
    FORGE the forge's name (`forge.md`, 5.4), read from the page's facts
    (`../protocol/sign-in.md`, 9.3 and 9.6);
  - *signed out at the authorization:* the `Authorize` place's sign-in
    page, "to continue to FORGE" under its heading, its link's `then`
    the authorization's address (`../protocol/sign-in.md`, 9.3);
  - *signed out, the link's sign-in not taken:* while the link's view
    says why its last sign-in installed nothing and the link stays
    usable (jig's `client/domain/client.md`, section 12,
    `not_signed_in`), the paragraph says so in place of the shell's
    words: full, "All N sign-ins are in use; the earliest ends MOMENT
    UTC. This link stays usable until LINK UTC; after that, opening it
    has `temper serve` print a fresh one." (N the deployment's room,
    MOMENT the earliest saved expiry and LINK the link's own, each with
    its date beside its clock, "Oct 24, 09:30", as the accounts page says
    its moments, 5.5: a sign-in's life is a day or more, so a bare clock
    would name tomorrow's as today's; the minute is the expiry's own, so
    the link serves a little past what it says); busy, "temper is busy. This
    link stays usable until it expires: open it again in a moment."; not
    ready, "temper is starting or stopping. This link stays usable until
    it expires: open it again in a moment." A link opened again past its
    expiry is refused, and `temper serve` prints a fresh one;
  - *out of date,* the engine serving no version of jig's records that
    this client speaks: "This temper is out of date: run the temper the
    engine runs." What is pending stays in the small store (jig's
    `client/domain/client.md`, 8.2).

  Both are phrases of temper's phrasebook: `jig-client`'s `words` has
  none for the link's states, and temper arranges no page of jig's
  (section 1). The page at the address is shown again once the link is
  starting, live or offline. When jig's records are served and the
  forge's are not, the link stays live, and the form says its own out
  of date (4.4).
- **`act`:** a shell's act on a node is resolved by skein-ui in the
  last tree to its binding, and decoded by temper's bindings into the
  domain's action. A node gone, or a token of an older tree, decodes to
  nothing (skein's `ui.md`, section 6).
- **Every node a person acts on has a role and a name,** since skein's
  fake person finds nodes only by them; nothing a person must know is
  said by style alone (skein's `ui.md`, section 8).
- **The look:** the mockups' (`mockups/style.css`, reference material),
  as style tokens each shell interprets or ignores; the terminal prints
  none, and the browser's stylesheet draws them (`browser.md`, 2.5).
  The composers are text areas that submit on Enter (skein-ui's
  `SubmitsOnEnter`), for the browser, and whose words may hold line
  breaks, counted within their bounds (5.4; `browser.md`, 2.5).

### 5.2 The forge's view

- **Words:** the forge's phrases in temper's phrasebook: the connector,
  resources, patterns and kinds (4.1); "Shown on the forge." (4.2);
  a change's panel's: its tag, its pull request's line, its steps,
  its step's lines, its holds, its gates' rows, its bounds and its
  panel's slot words (5.10); the form's title, field names, roles,
  rules and button (4.4), its title by the project's number included;
  the picker's nodes, "Not listed" and its refusals (4.4); the outcome
  (4.7); the form's out-of-date sentence (4.4); the forges' page's
  (`forge.md`, 5.4 and 5.5); the navigation entries. Names (owners,
  repositories, branches, logins) are copied in, never translated.
- **The parts:** a connector, resource, pattern or kind is one text
  node of its words; a part the forge's side does not read is the
  paragraph of 4.2; a part it reads is shown as `forge.md` says.
- **The form's page:** a level-1 heading (the title), then the form's
  nodes in the order of 4.4: the forge's field, the picker's nodes,
  text fields with their values and write counts, the role as a choice
  group of three choices, home and ci as checks, the button, and after
  them a status for progress and an outcome, or an alert for a refusal.
- **The forges' page:** a level-1 heading, "Forges", then "Your
  forges" with their count and "+ Add a forge", then a disclosure per
  listed forge, folded, named by the forge's name, its summary the
  forge's number, kind and `web` address, its connection and its line,
  holding its rows and its actions (`forge.md`, 5.4); the playground's
  forge in a section of its own; a pasted value a secret field (3.6), a
  link an external link (6.5), and a connect's or an action's status
  under its button. While the `forges` watch is refused, the page says
  so in jig's words; until it has delivered, its headings alone.
- **`ConnectForge`:** its level-1 heading, "Connect a forge"; its steps
  as a list, the current one marked; the step's card, under its level-2
  heading: Choose's cards as a list of links, each with its words;
  Details' fields, as the forges' page's; Setting up's steps as a list,
  each with its state in words; Ready's link to the project.
- **The accounts page:** a level-1 heading, "Accounts", then "Your
  accounts" with their count and "+ Connect a model", then a
  disclosure per account, folded, named by its card's heading (5.5, An
  account), its summary its deciding source's state and the account's
  line, holding its state's words and acts, its sources as a list in
  order, each its kind and its state in words, "Pays for", "Used by"
  and "Waiting for it" as name and value, and its acts as buttons;
  "Deployment accounts" the same way; the playground's in a section of
  its own (3.5). A secret field is the row's (3.6), a sign-in's
  address an external link (6.5), and an act's status under its row.
  While the `accounts` document is refused, the page says so in jig's
  words; until it has delivered, its headings alone.
- **`ConnectModel`:** its level-1 heading, "Connect a model"; its
  steps (below); the step's card, under its level-2 heading: Choose's
  cards, a choice for each way (5.5, The ways); Details' field and its
  button, and a program's command as code; Signing in's sign-in, its
  address as an external link (6.5), then its end; Ready's words, its
  account's name and model and what it pays for as name and value,
  "Start a chat" and a plan's "Manage usage".
- **A flow's parts,** shared by `ConnectForge`, `ConnectModel` and the
  welcome, are the view's `flow` module, each drawing one part from
  words already said and the bindings it is given, so that each flow
  keeps its own state and words:
  - *its steps,* the list along the top, named "Steps", each item a
    step's name, the current one `State::Current`, those before it
    styled done;
  - *a choice,* one of Choose's cards: a region named by its level-2
    heading, its words, its muted line and its link or button;
  - *a progress,* a list of lines, each with its state in words
    ("done", "under way", "failed", or none for one to come);
  - *a card,* the step's one card, holding the rest.
  A flow's page builds its main from them; the welcome's own steps are
  the same list.
- **Bindings:** writing a field, choosing a role, toggling home or ci,
  choosing a picker's installation or row, its "Next" and "First page",
  pressing Adopt, each of the forges' page's actions, and going to
  either page from its navigation entry.
  Typed input is made here: the forge number's digits become a number,
  the checks' words a list, or nothing when they are not one, which the
  form marks invalid.
- **No drawings.**

### 5.3 Navigation

In the mockups' top bar's order: "Inbox", with the count of what needs
the person from `me`, omitted when zero; "Chats"; then, for the project
the person is in, "Board" and "Changes"; and after them "Adopt a
repository", to `Adopt { project }`; then "Forges", to `Forges`, with
a dot, amber while a forge sets up or connects and red while one needs
the person (`forge.md`, 5.4), its state also in words, "a forge is
setting up" or "a forge needs you", a text node of its own after the
entry's name; current on `Forges` and `ConnectForge`; and last,
"Accounts", to `Accounts`, with a dot, amber while an account is
limited and red while one needs the person (5.5, Each state), its
state also in words, "an account is limited" or "an account needs
you", a text node of its own after the entry's name, read from the
person's own accounts, the demo's provider's left out: red for a
deciding source refused, needing a sign-in, misconfigured or
unloaded, before amber for one limited; current on `Accounts` and
`ConnectModel`; both whatever the project; and, designed, not built,
"How work runs", to `HowWorkRuns`, current there and on `Proposal`
(5.13), whatever the project. With no project chosen, the
project's entries are not offered. The mockups' "Tasks", a list of a
project's tasks, and "Workers", the deployment's hosts, are not in the
navigation (the user's pick, 10-23): the Board covers what a task list
is for, and the hosts are the operator's, through the engine's
configuration. "Adopt
a repository" and "Forges" are offered whatever the person's role: the
engine judges each, and a refusal says why, since the client never
computes what a party may do.

### 5.4 Limits

The view's limits are skein-ui's (skein's `ui.md`, section 15), derived
from the domain's: each page's nodes at its windows' most rows, the
forge's form and its outcome's lines, the kinds' list at its longest,
and one paragraph per part not read. Their text across a tree,
`text_bytes`, is the nodes at a node's most text: a lower cap would be
a guess, which skein-ui's builder turns into a panic on the page past
it. Their patches are four a node, the most a diff emits (skein's
`ui.md`, section 7). skein's text shell counts its worst-case print from these limits,
and reserves for each print only what that page's tree may print
(6.8).

temper's client takes every document within the bounds the engine's
configuration gives its documents. A path's segments have a bound of
their own, 9, apart from the windows' rows (jig's
`client/domain/client.md`, section 14), at least the most the engine
admits in a name (`../protocol/configuration.md`, section 7), so a page
of the forge's work is never refused for a path. A path longer than a
node's text shows its last segments after the mark `…` (as jig's
`client/domain/view.md`, section 7 cuts one), which adds nothing to the
view's limits. An authority's entries have a bound of their own, 12,
apart from the windows' rows (jig's `client/domain/client.md`, section
14), at least the most the engine admits in an authority's list
(`../protocol/configuration.md`, section 7), so a task's page is never
refused for its authority, nor an amendment drafted from part of one.
Each of an authority's lists shows at most the rows per window, then
jig's mark (jig's `client/domain/view.md`, section 7), which adds
nothing to the view's limits.

Both presets receive the dark theme's turn and the CSV chat's turns whole:
three rows (`rows`, which bounds a turn's messages and a message's blocks
as well as a page's lines), 512 bytes of field and provisional text (`text`),
and 2048 bytes of a committed turn's shared text (`turn_text`, which jig's
transcript shares among its texts, its calls' names, inputs and outcomes,
and its results' summaries). The CSV chat's first turn fits with its
`delegate` input of about 1.2 KB, so its card appears at that call (5.7).
A chat's window holds 16 lines (the domain's chat page), and the page
reads the page before its oldest while an older one remains and the
window has room for a page, `rows` lines, so a conversation of a dozen
lines shows whole, with no "Older". Past that, "Older" reads the page
before, and the lines the window lets go are said ("Earlier lines omitted N", jig's
`Left`). The engine's pages keep their own rows (its
`documents.rows`, at most the client's). Committed turns on both the chat
and agent task pages are priced by `turn_text`; cards and other fields
retain `text`.

### 5.5 LLM accounts' words

temper's phrasebook words what jig's accounts document holds (jig's
`client/domain/contract.md`, 7.1; jig's `domain/accounts.md`, 3.5 and
section 5); the providers' names and their plans' are the
configuration's. The owned example and the default document offer
OpenAI's ChatGPT sign-in and API keys (`../protocol/setup.md`, 8.1).
The words for each offered source are:

- **The providers' words.** A provider is named by its configured name,
  but for the names the owned example ships, which temper's phrasebook
  words as their makers do: `openai` reads "OpenAI", its signed-in plan
  "ChatGPT"; `anthropic` reads "Anthropic", its program's plan "Claude".
  Any other provider's plan reads as its name. jig configures no plan's
  display name yet (jig's `protocol/configuration.md`, section 3), so
  the plan is branded by its provider until it does.
- **The ways,** on `ConnectModel`'s Choose (3.5) and in the banner, one for
  each source a configured provider offers, in provider number order,
  but the demo's provider (`Demo.provider`, 4.8), whose account is the
  deployment's: a sign-in to a plan, "Continue with PLAN" ("Continue
  with ChatGPT"), and to no plan, "Sign in to PROVIDER"; a program's
  login, "Use your PLAN subscription" ("Use your Claude subscription");
  a key, "Use an API key", or "Use a PROVIDER API key" ("Use an OpenAI
  API key") when more than one shown provider offers keys. Each way
  sends what the account form's act for it would: a sign-in is jig's
  `SignIn` for that provider, held by the person (`Own`); a key's,
  `AddKey` joining no account; a program's, its declaration. On
  Choose, each way is a card: a plan's sign-in, the plan's name,
  "Your ChatGPT plan. Sign in with your ChatGPT account, and temper’s
  agents use your plan’s included usage." and "In a browser · about a
  minute", and, beside its button, "This browser isn't on temper's
  computer ›" (3.5, `ConnectModel`, From another computer); a key, "An OpenAI API key", "A
  key from OpenAI’s platform. What temper’s agents use is billed to the
  key’s organization." and "Pasted here · at once"; then the way's
  button.

- **The banner:** "Agents need an LLM account to run." and "Continue
  with ChatGPT", "Use your Claude subscription", "Use an API key", one
  for each way the configured providers offer (The ways, above), each
  a link to `ConnectModel` at its way. A chat may be started
  meanwhile; its page says "Starting: waiting for an account".
- **Ready,** for a key: "Added your OpenAI API key", PROVIDER's
  configured name ("Added your PROVIDER API key"), and "temper’s
  agents now run on it."; for a program's login: "Declared your Claude
  subscription · claude-alice", then "Run this on each host temper’s
  agents run on, in a terminal there:", the command, `temper
  program-login claude-alice`, and "Claude Code asks you to sign in.
  temper never reads its login."; for a sign-in, A sign-in's (below).
  Then, for a key and a sign-in, its account, the model a new chat's
  first run takes and what it pays for, as name and value ("Account",
  "Model", "Pays for"), and, while the demo's model half held (5.6),
  "The playground’s scripted model stepped aside.".
- **An account** is its provider and display name, "ChatGPT ·
  alice@example.com"; a key-only account, "OpenAI API key"; a program's,
  "Claude subscription · claude-alice". Its card's heading follows with
  its provider and holder: "ChatGPT · alice@example.com · OpenAI · held
  by Alice", the holder the person's name from `me` for their own, and
  "held by the deployment" for the deployment's; a key-only account's
  name already holds its provider, so its heading doesn't repeat it:
  "OpenAI API key · held by Alice". The card's region is
  named by that heading, so a person and a screen reader find an
  account by its name. Each source is its kind in words, "ChatGPT
  sign-in", "API key", "Claude Code login NAME", then its state.
- **An account's line,** its folded row's summary: its models, what it
  pays for, then "2 tasks waiting", "used by T4, T9" or "no task uses
  it now", a waiting task never also a user. What it pays for, its
  "Pays for": a plan's, "Your own work, and projects where you’re the
  only member"; a key's or a program's login's with no uses given,
  "Your own work" (jig's `domain/accounts.md`, 4.2).
- **The composer's line** names the account and model the work's next
  run takes, one line above the chooser (3.5), from what jig gives
  every page: `me`, the accounts document and its readiness:
  - *MODEL* is the draft's model (a task's LLM choice; a new chat's,
    its template's), then " · EFFORT" where the choice names an effort.
    Where the template names none, it is the supplied
    `resolved_default` beside that template in `me` (jig's
    `client/domain/contract.md`, 5.1), using Create's resolver, charter
    order, effective authority and current receiving-party payer.
    The composer never infers a default from the project catalogue's
    order, including a clipped catalogue. With no preview it uses
    "Using the default model". The preview is an observation, not a
    reservation: the draft keeps configured intent, and Create resolves
    omission once (`../protocol/setup.md`, 8.3).
  - *The account* is named only when the payer is you: your new chat,
    or a task funded by your pool. The accounts document lists only
    your own accounts, in `(place, number)` order. In this slice the
    view translates jig's account selection and coverage (jig's
    `selection.rs` and `coverage.rs`): an eligible account preference,
    then an eligible kept account, wins, even while held; else the
    first account of MODEL's provider whose coverage serves you and
    whose deciding source is usable. Explicit uses replace defaults;
    absent uses serve the holder. A plan serves its holder. Program
    endpoints skip direct sources, and direct endpoints skip program
    sources; a held deciding source stops that account, and a waiting
    direct source does too. A "later" source names no account where
    jig's Later and Limited would choose differently: a usable one, and
    an unusable program source before one that would be chosen. A
    program source without usable evidence may pass to the next program
    source. Mixed provider offers do not establish a model's endpoint
    kind, so name no account. Another payer's work, or a cut document or
    account, names no account.
    The source words the line, with the configured provider name from
    `me`: "Using your OpenAI API key · MODEL" for a key; "Using ChatGPT
    plan · MODEL · Manage usage" for a signed-in plan, the last an
    external link where the document gives a safe manage-usage address;
    "Using your Claude subscription, through Claude Code · MODEL" for
    a program's login.
  - *The readiness cross-check:* the payer's row must read `Usable`
    before any account is named. The document's source `usable` is a
    fact, while jig's selection reads its grant at the moment it
    chooses: these may differ across expiry. When the translation
    picks an account but readiness does not read `Usable`, the line
    falls back to "Using MODEL". An eligible preferred or kept account
    whose deciding source is held or waiting reads "MODEL · needs an
    account" when readiness passes that cross-check.
  - *The demo:* its line is keyed by MODEL's provider, the delivered
    `Demo.provider`, because its account is the deployment's and is
    outside the viewer's accounts document. Unless an eligible own
    preferred or kept account wins, use 5.6's "Using Scripted model ·
    Default". Only `Unavailable` readiness changes it to "MODEL ·
    needs an account"; an unknown or missing row keeps the demo line,
    including on another member's chat.
  - *Unsettled:* with no model settled, "Using the default model".
    Otherwise, when no account is named, `Usable` or `Unknown`
    readiness reads "Using MODEL", and `Unavailable` reads "MODEL ·
    needs an account", subject to the expiry and demo rules above.
- **Each state,** with its acts:

  | State | Words | Acts |
  |---|---|---|
  | usable | "Ready" | — |
  | limited, a plan's | "temper reached its usage limit on your ChatGPT plan. Tasks on it wait." | "Manage usage", "Resume", "Use the API key first" |
  | limited, with the reset the provider gave | "Limited by OpenAI until 14:00. Tasks on it wait." | "Resume", "Use the API key first" |
  | refused: plan usage declined | "You signed in without plan usage." | "Continue with ChatGPT", "Use an API key" |
  | refused: not eligible | "This ChatGPT account can't be used for plan usage in temper. Plan usage needs an eligible ChatGPT plan." | "Use an API key", "Sign in to another account" |
  | needs sign-in | "Signed out of ChatGPT." | "Continue with ChatGPT" |
  | needs sign-in, a program's, on a host | "Claude Code isn't signed in for 'claude-alice' on HOST." and the command to run there | "Check again" |
  | misconfigured | "OpenAI refused temper's sign-in. Check the configuration's registration." | — |
  | unloaded | "Can't be used on HOST: WHY." | — |

  Never a reset the provider did not give, and never "your plan is
  empty" (jig's `domain/providers.md`, 2.5). A limit may be the
  person's own cap for temper, which "Manage usage" reaches. A limited
  account's tasks wait for it, and temper never moves them to another
  account (jig's `domain/accounts.md`, 4.3). "Use the API key first" is
  offered where the account holds a key after the limited source: it
  moves the key up, so that the account's waiting tasks go on on it,
  and moves no task. A plan's row shows its limit's state and "Manage
  usage" alone: its usage is ChatGPT's settings' to show (jig's
  `domain/providers.md`, 2.6), and temper reads no plan's windows
  (`../draft/temper-on-temper.md`, 6.3).
- **A sign-in:** no words while it waits in a browser, which is at the
  provider (3.5, `ConnectModel`); in the terminal, "Open this address in
  a browser:" and the address, then the paste (below), as an external
  link (6.5). Then "Signed in to ChatGPT as alice@example.com", or
  "Signed in to ChatGPT" while the account has no display (jig carries
  none yet: its card's heading then drops that part), "temper’s agents
  now run on your ChatGPT plan.", its account, the model a new chat's
  first run takes and what it pays for, then, while the demo's model
  half held (5.6), "The playground’s scripted model stepped aside."; or,
  for a subject the person already has, "That's alice@example.com
  again. To add another account, sign out of ChatGPT in your browser
  first, then Continue with ChatGPT."; for a subject another person
  holds (jig's `domain/accounts.md`, 3.3), "Another person here holds
  that ChatGPT account. To use one of yours, sign out of ChatGPT in
  your browser first, then Continue with ChatGPT."; for a listener's
  port another program holds, before the browser leaves, "temper can’t
  listen on 127.0.0.1:1455: another program has it, such as Codex’s own
  sign-in. Finish or close that sign-in, then Continue with ChatGPT.";
  or why it was refused, in Each state's words. Each ending offers
  "Continue with ChatGPT" again, and a refusal Each state's acts.
  Each reads jig's ending on the `accounts` document (jig's
  `client/domain/contract.md`, 7.1): signed in, `Made`; the same
  account again, `Renewed`, named by its display where it has one,
  and "your ChatGPT account" otherwise ("That's your ChatGPT account
  again. To add another account, …"); signed in without
  plan usage, `Made` or `Renewed` with `declined`, in "refused: plan
  usage declined"'s words and acts, the account made all the same;
  another holder's subject, `Refused` `Held`; misconfigured, unloaded
  and signed out, `Failed` `Misconfigured`, `Unloaded` and
  `SignInAgain`; any other refusal or failure in jig's words.
- **The paste's refusals,** above its field, the sign-in still
  waiting (3.5, `ConnectModel`, The paste): *address*, "That isn't
  this sign-in's address. Copy the whole address of the page that
  couldn't be reached, from the address bar."; *too many*, "That's
  too many addresses from other sign-ins. Continue with ChatGPT to
  start again." and its button; *state*,
  "That address is from an earlier sign-in. Paste the one from this
  sign-in's tab, or start again."; a value past the bytes `me` states,
  before sending, "That's longer than a sign-in's address. Copy the
  address bar alone."; *not waiting* with the sign-in gone, "This
  sign-in has ended." and "Continue with ChatGPT"; a sign-in that
  refused too many ends as a misconfigured one does, in its words. Four have
  no fact of jig's yet, and are not drawn until jig gives one: the
  listener's port another program holds, "not eligible", plan usage
  declined at the provider rather than at its consent, and a missing
  scope in words of its own rather than its number (section 11).
- **A plan's first use:** a card in the conversation of the run that
  first used it, from the document's fact (jig's `domain/accounts.md`,
  section 5): "You're using your ChatGPT plan. Eligible requests from
  temper use your plan's included usage. You can see it, and set a
  weekly cap for temper, in ChatGPT's settings." and "Manage usage". It
  is part of the conversation, so nothing is kept to dismiss it
  (section 11).
- **Signing out:** "Signed out of ChatGPT." once the revocation is
  confirmed; "temper removed its tokens, but ChatGPT didn't confirm.
  Disconnect temper in ChatGPT's settings, under Apps." otherwise; for a
  program's login, "Signed out on every machine.", or the hosts that did
  not.
- **Plain words:** temper says that it runs Claude Code, and names no
  page, button or feature after it (jig's `domain/providers.md`, 3.2).

### 5.6 The first run's words: the demo and the welcome

temper's phrasebook words the demo (4.8; the mockups' first run) as
the playground, the person's name for it (with the user, 2026-10-23;
`../draft/first-run.md`, 2.3); OWNER/NAME is the demo's repository, and
the cues the demo's, copied in. An ended demo read retains its provider
and project after a reload: the Model step excludes that provider, even
if its account cleanup is still pending, and the navigation and default
project pick skip that project. With no other project, none is selected
and the navigation says "No project yet". An unavailable read supplies
no retirement fact; a running playground keeps its project and words:

- **The banner:** "This is the playground: a scripted model and a
  playground forge." while both halves hold, "This is the playground: a
  scripted model." or "This is the playground: a playground forge."
  while one does; then "Connect a model", a link to `ConnectModel`, while the model's
  half holds, and "Connect a forge", a link to `ConnectForge`, while the
  forge's half holds, or, while a forge of the person's sets up or
  connects, its set up under way (5.1).
- **The demo's start,** on the chats page, in place of its level-1
  heading "Chats" (`first-run.html`, the first run's first step): a
  chatbar, "New
  chat", the status "Ready", and "Scripted model", under the frame's
  banner (5.1); while its start is out (4.8), the status "Starting"
  and its suggestions disabled; once the start failed, the status "Not
  started", and under its lead "The playground did not start: WHY."
  with "Try again", WHY "its fakes did not start (BOX'S WHY)", "the
  scripted model's account was refused", "OWNER/NAME was not adopted",
  "the playground's project was not made", "temper's configuration does
  not grant it its acts: add {"connector": 1, "may": ["close",
  "add_key", "remove_account", "adopt"]} to engine.acts, as temper init
  writes it" or "Playground forge did not connect"; the context line; the level-1 heading "What shall we change?"; "Try a request in
  OWNER/NAME. Follow the task from its branch to a landed change."; a
  list named "Suggested requests" of one button per suggested request,
  the demo's `llm.asked` cues in order and never its `llm.cued`
  (`../protocol/setup.md`, 8.4), each named by its cue's words; "The
  scripted model plays these requests from its script."; then the page's
  composer, with no heading of its own, and "Your chats", as on any
  chats page (3.5). A chat starts with its first message, so the start has
  no task and no number; once a suggestion or the composer starts the
  chat, its page keeps the frame (3.5).
- **The context line,** after the chatbar: "PROJECT · OWNER/NAME on
  Playground forge", PROJECT the project as the banner names it; then "Signed in
  as the deployment’s owner" while `me`'s role on the deployment is
  `Owner`, else "Signed in as the project’s owner" while the person's
  role on the project is `Owner`, else "Signed in as NAME", the
  person's name as the banner shows it. OWNER/NAME is a node of its
  own, for the look.
- **The composer's line,** for the demo's LLM provider: "Using Scripted
  model · Default" (5.5). The demo's LLM provider serves one model, so the
  line names no other. While the model's half holds, a new chat's line
  is the demo's, since its model is the one with a usable account; a
  chat the demo's model ran keeps the demo's line after it, since jig
  records the model at Create (`../protocol/setup.md`, 8.3).
- **Its names:** the configuration names the demo's provider, model
  and forge by identifiers ("demo"), so the phrasebook names them
  "Scripted model" and "Playground forge", and its project by its name,
  "Playground" as `temper init` writes it (`../protocol/setup.md`,
  8.4), as the demo's scripted words
  do once `../protocol/setup.md` 8.4 takes the playground's words
  (`../draft/first-run.md`, 2.3).
- **The welcome** (`welcome.html`; `../draft/first-run.md`, 2.3): the
  wizard's steps, Welcome, Forge, Model and Ready, along its top, under
  a frame of temper's name, the person's and "Sign out" alone. Welcome:
  the level-1 heading "Welcome to temper"; "Ask for a change in a chat.
  temper’s agents make it on a branch, open a pull request, and land it
  once CI passes."; two cards, "Try the playground" ("A built-in
  scripted model and forge. Ask one of three requests and follow its
  change from a branch to main."; "Nothing leaves this machine, and
  nothing costs. · Ready at once") and "Set up temper" ("Your forge:
  GitHub, or a Forgejo temper runs on this machine. Your model: your
  ChatGPT plan, or an OpenAI API key.", by the default document's
  forges and its providers' ways;
  "About five minutes"), each with its button, the set up card's a
  link to the Forge step drawn as one (a move is a link: `browser.md`,
  2.3); then
  "Started in the playground? Connect your own forge and model whenever
  you like; the playground then steps aside." Forge is `ConnectForge`'s
  Choose and Details; Model is `ConnectModel`'s Choose (5.5, The ways);
  Ready names each as set up or under way, under "Almost ready", then
  "Open PROJECT" under "OWNER/NAME is ready" once the forge's work is
  adopted, whichever a sign-in comes back to. Forge's heading is
  "Connect a forge", then
  "Where your repositories live: temper’s agents push branches and
  open pull requests there."; Model's "Connect a model", then "temper’s
  agents run on it: on a plan you sign in to, or on a key.". Each
  step between Welcome and Ready offers a foot: "‹ Back", to its
  Choose inside a choice, otherwise to the step before; "Later", to
  the chats page; and "Try the playground instead" while the demo is
  answered (4.8) and no real forge is connected, including before the
  demo's end is read. Ready has no foot. A document with no demo
  (`application.demo` null, the demo's read answering none) has neither
  the playground's card nor the closing line, and no step offers the playground.
- **The setup strip:** "temper has no forge and no model yet.", or the
  one missing alone ("temper has no forge yet.", "temper has no model
  yet."; for a person whose role does not hold the deployment's forges,
  the model alone); a set up under way (5.1); then "Continue setting
  up", to `Welcome` at its step: Forge while no forge of the person's
  sets up, connects or is connected, else Model while no model is
  usable (3.5, The welcome, Whether it shows), else Ready; and "Try the playground" while no forge of the
  person's is connected and the demo is answered. With no usable account, a composer's
  line is "No model yet · Connect a model", and it does not send (5.5).
- **Not drawn:** the mockup's placeholder, "Try a suggested request…",
  and "Reset demo" (section 11).

### 5.7 A delegation's card

A chat's delegation shows as the card of the task it made, at the call
that made it, or else at the result it sent back: a delegation as the
chips of the tasks it made, and their results (jig's
`client/domain/objects.md`, section 10), in one card per task, as
the mockups' first run shows it. For a change, it is the first form of
the change's chip in a chat (`forge.md`, 2.4 and 2.5).

- **Which tasks.** A `delegate` call that did not fail names the tasks
  it made: by the call's `objects`, each `Task(N)`, when jig's thread
  keys them there, and otherwise by its acknowledgement, temper's own
  words ("Delegated tasks: 2, 3", `temper-engine-smith`'s answers),
  which the domain reads N from (section 11). The chat's task document
  is not enough: it lists the chat's delegates while they live only.
- **Where it shows.** Once, at the first of its anchors the
  conversation's window holds: its call, or else a result block that
  carries its result, whose summary has a line "Result from task N:"
  after the inbox entry's "M: " (temper's inbox words,
  `temper-engine-domain`'s `inbox_words`); or else the chat's heard
  result from the task (its activity's `Heard`, a result from task N),
  which comes before the chat's resumed turn carries a result block and
  is never drawn as words of its own; or else, while the task
  lives, after the window's last line, as one of the chat's live
  delegates (its document's `delegates`). The chat's window holds a
  small conversation whole, its turns with their calls'
  acknowledgements (5.4), so the first run's change shows at the call
  that made it; in a longer conversation the call may leave the
  window, and the card shows at its result, or after the window while
  the task lives. Those words name a person's number as well as a task's: a
  result shows a card only once its task's document says this chat
  requested it (its `requester`), and keeps its work summary
  otherwise; such a number still takes one of the watches while the
  thread names it.
- **What it reads.** The chat page watches each such task's document
  (`Subject::Task`), as the task's own page does: its chip, its result
  contract, its hold and its result, which stay readable once it has
  ended. When the chat hears a delegate's result, it reads that task's
  document again once, so a cached running card takes its durable result
  without waiting for the chat's resumed turn or the watch's end.
  It watches those its thread's calls, results and heard results name, the
  newest first, then its live delegates, up to the domain's `reads`;
  one past them, or that neither names any longer, is unwatched, and
  leaving the page closes them all. A delegation whose document has
  not arrived, or past the bound, shows as its anchor's work summary,
  as any call does.
- **Its parts,** in the mockup's order, a region named "Task N":
  - a line with its number and kind, "TN · CHANGE", and its state; the
    kind is its result contract's: CHANGE, REPORT, VERDICT, APPROVAL;
  - its title, the chip's (the spec's first line), as a heading;
  - its result's words, once it has ended;
  - for a change, its steps, a list named "Steps": "Branch pushed",
    "Pull request opened", "CI passed", "Landed";
  - a link to its page, named "Task N" and reading "Task N ↗" (3.5,
    "Links are by number").
- **Its state,** in temper's words, from its document: "Running" while
  it lives, "Held" while held (why, on its page and in the inbox);
  once ended, "Landed" for a change done with a change's result,
  "Completed" for any other done (jig's word, as the task's page says
  it), "Failed", "Cancelled" (cancelled or withdrawn), and "Ended"
  otherwise.
- **Its result's words:** a landed change's, "CI passed. Landed ·
  merge commit ABCDEFG", ABCDEFG the first seven characters of the
  result's words, which are the merge commit's id in hex (the engine's
  landing words); any other result's words as they are; a failed
  task's reason.
- **The steps are a stand-in:** none ticked while the change lives, all
  four once it has landed, none otherwise. A ticked step is skein-ui's
  `completed` item, its tick the look's (STEP_DONE): a screen reader
  reads "Branch pushed completed" and the terminal prints "- Branch
  pushed (completed)" (skein's `ui.md`, 3.2), so nothing is said by
  style alone (5.1). The change's live stages are later (section 11).
- **What it stands for:** the call's work summary, its result's, and
  the line that heard the task's result, whose words are the card's: a
  chat that shows a task's card shows none of them.
- **The look:** the card's tokens (CARD and TASK_CARD, META, TITLE,
  DETAILS, a status's) and three of the steps: STEPS, STEP and
  STEP_DONE, which the browser's stylesheet draws as the mockup's
  `.steps`, `.step` and `.step.done` (`browser.md`, 2.5).
- **Where it is decided:** the domain's chat page holds each delegation
  (its task and its handle), and a pure function of the domain derives
  a card's facts from a task's document: its kind, state, title, words
  and steps ticked. The view lays them out with the phrasebook's words.
- **The first run:** the chat is task 1 and the change it delegates
  task 2, so the person reads "T2 · CHANGE", Running, then Landed
  (`../draft/first-run.md`, 2.1), and after the card the chat's next
  call, folded, "1 tool call", described "wait" (3.5).

### 5.8 A goal's page

A goal's page is a tracked task's (3.5): an agent's task with a
priority. It shows its task document, its owner tree and its latest
activity, as `mockups/task.html` lays them out, and draws each amount
and each state as the documents supply it: it computes no sum, share
or difference of their amounts (jig's `client/domain/contract.md`,
5.1).

- **Its parts, in order:** the bar; the header, "Task details"; its
  acts' toolbar, then their outcome, and its own held card while it is
  held (5.11); the model choice (3.5); "What was asked", with the result
  the goal gives;
  "Result", once it has ended; "Plan"; "Latest activity"; "Budget and
  authority". The confirmation follows while one is open (3.6). The
  mockup's tabs, and its cards for findings (a gap,
  `mockups/README.md`), notes, history, placement and changes, are not
  drawn.
- **The header** begins "Goal · priority N", then the task's number,
  its executor and its project, as before; its state follows the
  title (below), then who asked. The deadline is the budget's card's.
- **The result it gives:** "What was asked" names it after its heading,
  from the task's result contract, as the mockup's tag: "Result ·
  report", "Result · change", "Result · verdict" or "Result ·
  approval".
- **Its result,** once the goal has ended: a region "Result" after
  "What was asked", holding its state and its words as a delegation's
  card says them (5.7), from the same function: "Completed",
  "Landed", "Failed", "Cancelled" or "Ended"; the result's words, a
  landed change's merge commit, or a failed task's reason. A goal that
  has not ended has no such region. The page reads an ended goal's
  document once more when its watch ends, so its result stays
  readable. The mockup ends no goal: this region is temper's.
- **A task's state,** in the mockup's words, in lower case:
  - in the header, from what the engine computes the task waits on
    (`Task.waits_on`): on its dependencies, "waiting on task M" for
    one, "waiting on N tasks" for several, N their supplied `count`;
    on its delegates, "waiting on N delegates" ("1 delegate"), N its
    chip's `live_delegates`; on a party, "waiting on an answer from
    you" when the person is that party or holds that role, "waiting
    on an answer from person N" or "from role N" otherwise; on news,
    "waiting on news". Waiting on nothing, it says what a row would;
  - on a plan row, which has no `waits_on`, from its chip's owner phase
    (jig's `client/domain/objects.md`, section 3): waiting on its
    dependencies (the phase's `dependencies`, those not yet ended),
    "waiting on task M" on one, "waiting on N tasks" on several, N
    those listed, and "waiting on more than N tasks" when the list was
    cut (`more`); waiting on no dependency but on its holdings
    (`holds`), "waiting on its resources";
  - running, in both, with the work its owner observed
    (`observation`): "running · writing", or "running · a tool ·
    NAME", the tool's name as supplied; "running" alone when no
    observation is supplied, which jig allows, or for any other work;
  - ended, with its owner's ending: "Completed", "Failed" or
    "Cancelled", as 5.7 words them;
  - otherwise, jig's word for its phase in lower case: "held",
    "retrying", "waiting for news". A held task's card and a
    delegation's card keep theirs (3.6, 5.7).

  Each carries its phase's status token: a waiting state waiting
  (`STATUS_WAITING`, the mockup's amber), a cancelled one quiet, a
  running one active, a completed one done, a failed one held.
- **The plan:** the goal is not a row of it; its state is the
  header's, and its numbers the budget's card's. Its delegates are the
  plan's roots: its live ones, the owner tree's root's children, with
  their own live delegates under them; and its ended ones, one page
  read from the engine as a procedure's are (5.10): `Read::Tasks`, a
  `TaskQuery` by `parent` the goal and the ended phase, in the order
  they were made (`QueryOrder::Ended`, by number), `rows` to a page.
  The page is read once the goal's document has arrived, and again
  when its tree or its document changes, one read out at a time; the
  last page answered stays drawn until the next one answers.
- **Its rows' order:** the roots by number, the order they were made.
  An ended delegate is drawn before the first live root with a higher
  number, or after the last live root when none has one; a live root's
  own delegates stay under it, in the tree's order. A delegate in both
  the tree and the page, one that ended after its tree was delivered,
  is drawn once, as the tree's row. An ended delegate's own delegates
  are not read: its page shows them (section 11).
- **An ended row** is a plan row (below) whose state is its ending,
  "Completed", "Failed" or "Cancelled", with its spend and with no
  dependency line or held card; its "Task N" link is muted, as the
  mockup's done row.
- **Its counts:** a paragraph after the heading, once the read has
  answered: "Its delegates: N live, M ended.", N the goal's chip's
  `live_delegates`, as the header's "waiting on N delegates" has it,
  and M the ended rows in the page; "more than M ended" when the page
  has more after it (`after`) or was cut (`cut`), as a row's "waiting
  on more than N tasks" says a cut list. They count the goal's own
  delegates, not theirs, and the filter does not change them.
- **Its filter:** after the counts, on a line of its own, "All" and
  "Live only", two buttons, the chosen one pressed, as the chats
  list's (3.5); they filter the plan in the page, reading nothing, and
  are in the goal's address (`browser.md`, 2.3), "/tasks/9?state=live",
  a filter chosen a move, as the chats list's. "Live only" leaves out the ended
  rows and the more line; the live rows, their folds and held cards
  stay, and the read goes on under either. The filter is drawn from the
  page's start, before the read answers.
- **Its words:** until the read answers, a plan with no row, fold or
  held count says "No live tasks."; once it has answered, under "All",
  a plan with no row says "None yet." while the goal lives and "None."
  once it has ended, and under "Live only", a plan with no live row,
  fold or held count says "No live tasks." Under "All", while the page
  has more after it or was cut, a last line follows the tree: "More of
  its ended tasks are not shown." (section 11).
- **A plan row's cells:** the "Task N" link and the task's title; its
  executor; its state; its spend, "N spent", what the task spent
  itself (its chip's `budget.spent`; not the row's `spent`, which adds
  what the ended tasks it funded spent), in the deployment's unit, a
  whole number. The cells have no heads, since tree items are not a
  grid's cells; the mockup heads its columns "Task", "Kind", "Model",
  "On", "State" and "Spent" (a default the user may change, section
  11). A procedure's part follows on a line of its own, as before. At
  phone width the name is on a first line, and the executor, state
  and spend on a second; no cell is hidden.
- **The dependency line:** under a row whose task has dependencies
  (its `dependencies`), a list named "Dependencies of task N", one item
  per dependency in the supplied order, keyed by its task as an object,
  holding its "Task M" link and its state: a live dependency's as
  above, an ended one's supplied status ("Completed", "Failed",
  "Cancelled"), and none when none was supplied. When the row's list
  was cut (its `cut`), a last item, with no key: "More dependencies
  not shown." The browser shows the list's name before its items. In a
  row's tree item come the row, its dependency line, its held task's
  card (3.6), then its children. The mockup's worlds declare no
  dependency, so it draws none: this line is temper's, for B2.
- **An added dependency's mark:** in its item of the dependency line,
  after its link and its state, read only, for a dependency an
  amendment added (the row's chip's `dependencies`, jig's
  `Dependency.added`, matched by number): a paragraph, "Added by you"
  when the person signed in added it, "Added by party P", "Added by
  task M" or "Added by the deployment" otherwise; then, while it keeps
  the row's task (jig's `keeps`, jig's `domain/tasks.md`, 5.7), " ·
  keeps task N from ending". No time is drawn: `ctl task N` prints
  when (`tools.md`, 6.1). A dependency the facts
  don't name, or one not added, has no mark; the line keeps its shape
  (section 11).
- **Latest activity,** a region after "Plan": the latest page of the
  goal's activity (`activity`), an item per line, its time, then what
  happened. A result the goal heard begins with its task's "Task N"
  link: a heard word of a result's kind from a task (jig's
  `OwnerActivity::Heard`, its word's `kind` a `Result` and its `from`
  a task), then its words; a result line (`Activity::Result`), then
  its task's title. A person reaches an ended delegate by its plan
  row while it is in the plan's page, and by its result's line while
  that is in the latest page (section 11). The page reads no older
  page of a goal's activity (section 11). The chat's page draws
  its activity as before.
- **Budget and authority,** a region after "Latest activity", as the
  mockup's side card: "S spent · L left of B", what the goal spent
  itself and what its budget has left of itself, both as supplied;
  then each fact as a term and its description:
  - "Delegates spent": what the ended tasks it funded spent;
  - "Reserved for delegates": what it holds for the live tasks it
    funds;
  - "Funded by": a task, as its "Task N" link; a pool, "Your pool"
    when it is the person's, "Person N's pool" otherwise, since a pool
    names its person by number where the mockup names them; a
    recurring task's, "Task N's recurring budget"; a project's,
    "Project N's budget";
  - "Deadline": its date, or its date and time in universal time when
    it is not at midnight, or "None" (a default the user may change,
    section 11);
  - from its authority, which the mockup's card leaves out: "May
    spend", what its subtree may spend; "May delegate", "Up to N
    tasks, M deep"; "Models", "All", their names, or "None", and no
    such term when the authority leaves them out.

  A cut authority (its `cut`) ends the card with "Some of its
  authority is not shown." The mockup's bar is not drawn: the words
  say what it would.
- **Not drawn,** besides the mockup's tabs and cards above: what is
  not supplied, the mockup's charter, model, host, account and pool
  names; the mockup's "made yesterday", a time this page does not
  say (5.12); and, as defaults the user may change (section 11), the
  heading's "N tasks" and the counts' "N running, M in queues", where
  the counts say "N live, M ended" of its own delegates (above): the
  queues are the forge's stage, and the tree is a window of a goal's
  subtree that counts by phase would mix with rows not drawn; the
  ended delegates' own delegates; the filter at the heading's right,
  where it is on its own line after the counts;
  "Wakes on", "Input", "Writes", "Run N" and the pool's line.
- **Where it is decided:** pure functions of the domain read the
  documents: a task's state from its document's `waits_on` and chip
  in the header and from its chip on a row (`pages::task::state`), a
  dependency's from its link, and the result's from the delegation
  card's (`pages::delegation::facts`). The view lays them out with the
  phrasebook's words. The domain's task page holds the ended page, its
  one read and whether it is stale, as a procedure's (5.10), and the
  plan's filter, reset at each open; the view merges the ended rows
  into the tree by number, leaves out a delegate the tree holds,
  counts the page's rows and draws by the filter.
- **The look:** TASK_CONTRACT on the result it gives, PLAN_SPEND on a
  row's spend, DEPENDENCIES on its dependency line, PLAN_DONE on an
  ended row, FILTERS on its filter, as the chats list's, TASK_BUDGET on
  the budget's card, besides the task page's tokens; `task-pages.css` draws
  them (`browser.md`, 2.5).

### 5.9 An agent's task's page

An agent's task's page (3.5) is that of an agent's task with no
priority and not the chat label: a goal's delegate, the agent a
change's procedure makes, a task an agent's task delegated. It shows
its task document, its conversation as a chat's page holds one, and its
owner tree's held cards, as `mockups/task.html` draws its agent kind,
in the chat's look. It draws each amount and each state as the
documents supply them (jig's `client/domain/contract.md`, 5.1), and
acts on it as its document offers (5.11).

- **Its parts, in order:** the lineage bar; the taskbar, with its acts
  (5.11); "Task brief"; the run's heading, then its own held card while
  it is held (5.11); "Older" while an older page remains; the
  conversation; its acts' outcome; "Result", once it has ended; "Plan",
  while its tree holds a delegate or a held task's card; the composer
  while Message is offered (5.11); then the confirmation while one is
  open (3.6). At phone width the order is
  the same: the lineage bar keeps one line, its titles cut short, and
  the taskbar is a chat's chatbar at that width.
- **The lineage bar:** its parent, the last of its lineage
  (`lineage_chips`, root first), as its "Task N" link and its title,
  after an arrow; then the task itself, "Task M" and its title. With no
  lineage supplied, the parent is its requester's "Task N" link when a
  task asked it, and there is none otherwise. The arrow and the slash
  between them are the look's.
- **The taskbar,** a chat's chatbar (3.5): its level-1 heading "TN ·
  TITLE", as a chat's; its state, in 5.8's words, from the same
  function (`pages::task::header_state`), with its status token; then
  its executor, "Agent", and its own spend, "N spent", its `budget.spent`
  as a plan row's (5.8).
- **"Task brief":** a region, the mockup's "TASK BRIEF", whose capitals
  are the look's: its specification as written (`spec`, read as
  "What was asked" reads it, 5.8); who asked, in 5.8's words ("Asked by
  task N"); then its dependency line, as a plan row's (5.8): the list
  "Dependencies of task N", one item per dependency of its document
  (`dependencies`), in order, keyed by its task, with its "Task M"
  link and its state, and an added one's mark (5.8), from the
  document's `dependency_facts`. The document's `cut` does not say
  which of its lists was cut, so the line has no last item (section
  11). The mockup's worlds declare no dependency: the line is
  temper's, for B2.
- **The run's heading,** one, for its current or its last run, from its
  chip's owner phase and its `attempt` (jig's `client/domain/objects.md`,
  section 3):
  - "Run N · live now" while a run is out: a claimed or a running
    run, or a run closing, N its attempt;
  - "Run N · waiting" while the task lives and no run is out, N its
    last run's attempt;
  - "Run N · ended" once the task has ended, N its last run's attempt;
  - "No run now" while it lives and has had no run, and no heading
    once it has ended without one.

  Under a live or a waiting run's heading, "Started MM-DD HH:MMZ" when
  its chip supplies `run_since`, when the run was placed, in universal
  time. A run's earlier runs are the
  conversation's "Run N began" and "Run N ended": the page has no run
  groups.
- **The conversation,** from the task's bounded thread (jig-client's
  `thread`), read as the chat's page reads it, is drawn as work, not as
  a chat's (3.5): its latest window; committed turns, the agent's
  "Temper"; a message's calls folded as a chat's (3.5), in one
  disclosure after its text, named and described as the chat's and
  listing each call as a work line, a `delegate` call's too, since a
  delegation's card is the chat's (5.7); each result as a work line,
  where its message has it; "Run N began" and
  "Run N ended"; the lines omitted before the window; and a live run's
  provisional text, as the chat's provisional turn.
- **Its result,** once it has ended: a region "Result" after the
  conversation, its state and its words as a goal's "Result" says them
  (5.8), from the same function (`pages::delegation::facts`):
  "Completed", "Landed" (done with a change's result, as 5.8 says),
  "Failed", "Cancelled" or "Ended", and the report's words, or a
  failed task's reason. The mockup's "RESULT" capitals are
  the look's. A task that has not ended has no such region.
- **Its plan,** while its owner tree holds a delegate, or a held task's
  card, its own or a delegate's: a goal's "Plan" (5.8), its rows, their
  dependency lines and held tasks' cards. The mockup draws none on an
  agent's page: its held cards are what this page may act on.
- **Against its mockup,** the page's way:
  - links and the lineage name a task "Task N", the mockup's "T12"
    (3.5, "Links are by number"); the executor is "Agent", where the
    mockup names the charter (3.5);
  - the demo's banner stays where the frame draws it, before the page,
    where the mockup draws it under the taskbar (5.1);
  - a message's calls folded as the mockup folds them ("7 tool calls",
    then "read ×6 · recall") and as a chat does (3.5), its results still
    work lines (section 11);
  - not drawn, since no document supplies them: the charter's, model's,
    account's, worker's and pool's names ("Implementer · Sonnet ·
    medium", "on s1", "holds a slot of T12's pool"); whose
    specification it took ("from T3's spec"); a run's own spend ("N
    spent so far"); and a turn's time, which a chat's page alone says
    (5.12). Nor its context (`Task.context`), which
    is optional;
  - its acts are 5.11's, with their own departures there.
- **Where it is decided:** the domain's task page has the shape
  `Agent`: an agent's executor, no priority, not the chat label, which
  opens as `Chat { task }` (3.5). It holds the task's thread as the
  chat's page holds one: made with the chat's bounds, its latest page
  read as the page opens, read again on each change of the document,
  given the run's live text while the `run` watch is open, and its run
  ended as the chat's is when no run is out. A pure function of the
  domain derives the run's heading from the chip (`pages::task::run`);
  the state and the result are 5.8's. The view lays them out with the
  phrasebook's words.
- **The look:** the chat's, MAIN and CHAT on its main, CHATBAR on its
  taskbar, THREAD on what follows it and the conversation's tokens, so
  `temper.css`'s rules for a chat draw them; and its own: AGENT_LINEAGE
  on the lineage bar, which tells the page from a chat's (a node
  carries two tokens at most, and its main has MAIN and CHAT),
  AGENT_BRIEF on the brief, AGENT_RUN on the run's heading and its
  start, AGENT_RESULT on its result. `task-pages.css` draws them as the
  mockup's
  `.agent-lineage`, `.agent-brief`, `.agent-run-heading`,
  `.agent-run-meta` and `.agent-result` (`browser.md`, 2.5).

### 5.10 A procedure's task's page

A procedure's task, a change among them (`../domain/forge.md`, section
8), shows a tracked task's header and plan (5.8), as a person task
does, with its goal and "Its tasks". A forge procedure's task, whose
panel the forge's side watches while its page is open (`forge.md`,
9.2), draws its panel as `task.html`'s change does: its steps, its
hold, its gates and its bounds. Each fact is drawn as the panel
supplies it: the page computes none (jig's `client/domain/contract.md`,
5.1).

- **Its parts, in order:** the bar; the header; "View on forge ↗"; the
  change's card, holding its steps, its step's line, its hold, "Its
  tasks", "Gates at its head" and its bounds. At phone width the order
  is the same, and the steps take three columns. Until its panel is
  shown, and for any other procedure, the page is the bar, the header
  and "Its tasks".
- **Its goal:** in the header's first line, after its project and its
  procedure's part, a slot of "Goal" and the "Task G" link to the goal
  it serves, the document's `goal` (the nearest tracked task at or
  above it, itself included: jig's `domain/core.md`, 6.2), when that is
  another task. A tracked procedure's task serves itself and one under
  a chat serves none; neither shows the slot.
- **The header, its panel shown:** its first line begins with the tag
  "Change · Task N" where 5.8's says "Task N" and its executor, then
  its project, then its goal's slot; the procedure's part is the panel
  itself, so the line has no "Shown on the forge.". Its title follows,
  its state and who asked, as 5.8's; then a line naming its pull
  request, "PR #N · OWNER/NAME · BRANCH → LANDING", or "No pull
  request yet · OWNER/NAME · BRANCH → LANDING" before one is opened,
  the repository, its branch and its landing branch as supplied.
- **The header, its panel not shown:** 5.8's, its procedure's part
  saying where the panel is (jig's `client/domain/root.md`, section 6):
  "Loading the change from the forge." until its first delivery; "The
  forge knows no change for this task." when the forge answers unknown
  (a task that is no change, or one whose subject left the engine);
  "The forge cannot show this change." when its watch ends for any
  other reason. A procedure the forge does not run says "Shown on the
  forge." as before (4.2).
- **"View on forge ↗":** an external link (6.5) to its pull request's
  address on the forge, in a toolbar after the header, only when the
  panel supplies an address (`forge.md`, 9.2); a forge with no `web`
  gives none, as `temper box`'s demo does.
- **Its steps:** a list of six, as the mockup's: "Producing",
  "Opening", "Checking", "Gating", "Queued", "Landed". The one its step
  is at is current, those before it done, and all six done once it has
  landed. The steps 2.1 draws beside the way show at the one they
  serve: recreating and reopening at "Opening"; a repair of CI at
  "Checking", of a gate at "Gating", of a semantic conflict at
  "Queued"; first, updating and resolving at "Queued"; landing at
  "Landed", current. "Queued" reads "Queued · Nth" while it has a place
  in its queue, "Queued · 1st" when first, the place an English
  ordinal ("1st", "2nd", "3rd", "4th", "11th", "12th", "13th",
  "21st").
- **Its step's line,** after the steps, then the "Task N" link of the
  task its step runs, when the panel names one (`runs`):

  | Step | Its line |
  |---|---|
  | producing | "Producing its branch." |
  | opening | "Opening its pull request." |
  | recreating | "Making its branch again at the head temper last knew." |
  | reopening | "Reopening its pull request." |
  | checking | "Waiting for CI at its head." |
  | gating | "Waiting for its gates at its head." |
  | queued | "Nth in the queue for LANDING, after Task A, Task B and N more.": the tasks ahead it names, nearest first, as links; "and N more" for those it does not name; "after N tasks" when it names none; and no "after" when nothing is ahead |
  | first | "First in the queue for LANDING." |
  | updating | "Updating from LANDING." |
  | resolving | "Resolving its conflicts with LANDING." |
  | repairing | "Repairing: CI failed at its head.", "Repairing: gate N asked for changes." or "Repairing: a semantic conflict with LANDING." |
  | landing | "Merging at its head." |
  | landed | "Landed · merge commit ABCDEFG", its first seven digits, as a delegation's card says it (5.7) |

- **Its hold,** while it is held, a notice after its step's line: "Held
  while STEP: REASON.", STEP the step it was held at in lower case as
  the table names it ("first in the queue" for first), and REASON:

  | Hold | Its reason |
  |---|---|
  | stalled | "its wait stalled" |
  | repairs | "its repairs are spent" |
  | resolutions | "its resolutions are spent" |
  | updates | "its updates are spent" |
  | moved | "its branch was moved on the forge" |
  | missing | "its branch is missing on the forge" |
  | closed | "its pull request was closed on the forge" |
  | retargeted | "its pull request was retargeted on the forge" |
  | failed | "a delegate or a write failed for good" |
  | rejected | "a person rejected a gate" |

  The reasons are 6.1's and 6.2's, in the panel's measure (`forge.md`,
  section 6): the held card says the rest.
- **Its tasks:** a region and tree named "Its tasks", the tasks its
  procedure made (the agent producing a change's branch, its repairs
  and reviews), where a goal's and a person task's say "Plan". Its
  live tasks are 5.8's plan rows. Its ended ones are 5.8's ended
  rows, read once from the engine, its children in the order they
  were made (`Read::Tasks`, a `TaskQuery` by `parent` and the ended
  phase, `QueryOrder::Ended`, by number), and read again when its tree
  or its document changes, one read out at a time; they are drawn in
  5.8's rows' order. It has no filter and no counts, as its mockup
  draws none. Until that read answers, an empty tree says
  "No live tasks." as 5.8's; after it, a region with no row says "None
  yet." while the task lives, "None." once it has ended. When the
  read's page holds less than all of them, a last line: "More of its
  ended tasks are not shown." The page reads no older page (section
  11). Every procedure's task reads them, its panel shown or not.
- **Gates at its head:** a region of rows, each a name and its state.
  First "CI at its head": "Not yet" while CI is unknown, "Running"
  while pending, "Passed", "Failed". Then each gate, in the supplied
  order: "Gate N · review", "· approval" or "· check", then "·
  blocking" or "· advisory", and its verdict: "Not yet" with none,
  "Waiting" pending, "Approved", "Changes asked". With no gate, a last
  row, "No other gate."
- **Its bounds,** the card's last line: "Repairs N of M · resolutions N
  of M · updates N of M", each used of its limit (2.2).
- **Against its mockup,** the page's way:
  - links name a task "Task N", the tag "Change · Task N", the mockup's
    "T28" and "Change · T28" (3.5, "Links are by number");
  - not drawn, since the panel does not carry them (`forge.md`, 9.2):
    the "Fixes finding" notice (findings are a gap,
    `mockups/README.md`); the head's requirements, its heads in order,
    CI's contexts, the pull request's title and mergeability, how long
    a step has waited and when a stall began;
  - not drawn: the mockup's case name beside the tag, a reference
    world's;
  - not drawn: the "Changes" breadcrumb, the bar naming its way as
    5.1's does (the `Changes` page is not built); "Its tasks"'s
    subtitle ("made by the change procedure…"); and "its landing rules
    add none" after "No other gate.", which the panel cannot tell from
    a rule not read;
  - its tasks' rows are 5.8's plan rows, with their executor, state
    and spend, where the mockup draws each as a gate's row with its
    kind and phase; a gate is named by its number and kind, where the
    mockup names it by the landing rule that adds it;
  - its hold, the step's line and its bounds are temper's: the mockup
    draws none.
- **Where it is decided:** the domain's task page holds the forge's
  side's panel (`forge::panel`, its state loading, shown, unknown or
  ended) and its ended children's page, its one read and whether it is
  stale; pure functions of the domain's forge side read the panel: the
  step's slot and the steps done (`forge::change::slot`), the line's
  facts and the hold's step. The view lays them out with the forge's
  words (4.2).
- **Its words and look:** "Goal", the existing word; "Its tasks", "None
  yet.", "None." and the more line, the page's; the change's words, a
  forge table beside its parts' (`forge::words`). CHANGE on the card,
  CHANGE_TAG on the header's tag, STEPS, STEP and STEP_DONE as a
  delegation's card's (5.7) and STEP_CURRENT on the current step,
  CHANGE_HOLD on the hold, GATE on each row of "Gates at its head",
  META on the pull request's line and the bounds, TASK_ACTS on the
  toolbar; `task-pages.css` draws them as the mockup's `.tag.accent`,
  `.steps`, `.step.current`, `.notice` and `.gate` (`browser.md`, 2.5),
  the steps in six columns, three at phone width.

### 5.11 A task's page's acts

A goal's page (5.8) and an agent's task's page (5.9) act on their task
as a chat's page acts on its chat (3.5): through the acts its document
offers (`Task.acts`, read through `acts::offers`), each sent as 3.6
says. An act the document does not offer is not drawn, not even
disabled. One it offers is drawn by its admission (jig's
`client/domain/contract.md`, 5.3), as a held task's card draws its own
(3.6):

- *allowed* or *bounded:* its button;
- *proposable:* its name, then the card's "This needs a proposal to"
  and to whom, and no button: the page proposes none of these acts;
- *refused:* its name, then the refusal in jig's words, and no button:
  the page sends nothing.

Its rules:

- **Where they are.** On a goal's page, a toolbar after the header, as
  the mockup's: "Amend", "Stop", "Cancel", in that order. On an agent's
  task's page, in its taskbar after its spend, as a chat's chatbar
  holds Stop and Close: "Amend", "Stop"; and its composer last (below).
  At phone width the goal's toolbar is a line of its own under the
  header, in the same order, and the taskbar's acts wrap as a chat's
  do. While the page's confirmation is open, the toolbar and the
  taskbar draw no act: the dialog is the one place to answer, and its
  "Cancel" the only one on the page.
- **Its own held card.** While the page's task is held, stopped among
  the reasons, its held task's card (3.6), with Release and its
  confirmation, follows the goal's toolbar, or the agent's run heading.
  The task is no row of its own plan, so its plan never draws it; a
  delegate's held card stays in its row (5.8). Release names the
  hold's revision, and a moved one's refusal asks the person to look
  again (3.6).
- **The composer,** on an agent's task's page only, while Message is
  offered: the chat's composer (3.5, 5.1), with its model choice line,
  its text area "Message", its hint and "Send", preceded by the note
  "Steer this task · words reach its live run at the next turn." A
  message sent shows in the conversation as a chat's person's words
  do, "Being sent", then "Sent". The engine offers Message on every
  live agent's task and refuses it while the task waits to start
  (`NotWaiting`): the composer's place then holds "Message", then a
  note in the steer note's look, temper's "Messages reach it once it
  runs." (any other refusal: its supplied words, in the same look,
  since why Message isn't offered is no outcome), and its model choice
  line alone. A goal's page has no
  composer, as the mockup draws none; its model choice is 5.8's.
- **Stop,** confirmed (3.6): a dialog "Stop" saying "Its run stops, and
  the task is held until it is released.", then "Confirm" and "Cancel".
  It is sent as `Ask::Stop`. The engine holds the task, stopped by the
  person, so Release is offered next, its effect said as 3.6 says ("The
  next run resumes its conversation; tries count again from zero").
- **Cancel,** on a goal's page, confirmed (3.6): a dialog "Cancel"
  saying what cancelling ends, from the offer's effect
  (`Effect::Cancel`, jig's `Impact`, over its tasks below it), in jig's
  words, as closing a chat says it: "Dangerous action: review its
  consequences", its waiting, running, held and closing tasks, its
  live runs and pending proposals, and that their effects settle;
  then "Confirm" and "Cancel". It is sent as `Ask::Cancel`, with no
  grace and the reason "Cancelled from its page"; its delegates end
  with it.
- **Amend** opens the amendment's editor, laid out after the goal's
  toolbar or the agent's taskbar: a region "Amend task N". The draft
  is the amendment editor's (3.7), `amend::Draft::of` the task's
  document as read when it opens, and the page lays out two of its
  edits, no other:
  - "Specification", a text area holding the spec's words, its edit
    `words`; where the read cut the spec (`Cut::Spec`), the words read
    only and "This task's specification is too long to edit here: use
    `temper ctl amend`";
  - on a goal, "Tracked", a check, and "Priority", a number, its edit
    `track`: `Tracked { priority }` or, unchecked, `Untracked`.

  Then "Send amendment" and "Discard". "Discard" closes the editor and
  drops its draft. "Send amendment" with no change (`amendment()` is
  none) sends nothing and says "No change". While a document read since
  has another spec revision (`Draft::moved`), the editor says "This task
  changed since the editor opened" and draws no "Send amendment": the
  person discards and opens it again on the new document. Otherwise it
  sends `Ask::Amend` with the drafted change, the drafted revision
  (none for a tracked task's priority alone, 3.7) and the reason
  "Amended from its page"; a refusal as moved in between says so with
  "Look again" (below). Its outcome is the editor's: pending, then the
  answer in jig's words in the region, a refusal as moved with "Look
  again". A done amendment leaves the region holding its outcome alone,
  where it was made, until "Amend" opens another draft. Amend is drawn
  by its admission as every act is: proposable, the page proposes
  nothing. The model choice for the next runs stays the choice line's
  (3.5), its own amendment, not repeated in the editor. Instructions,
  dependencies, the deadline, wake and an addressee are `temper ctl
  amend`'s alone (`tools.md`, 5.5).
- **Their outcome.** One request per kind of act at a time; a double
  press joins the pending one (3.6). Pending, its progress as a card
  says it; answered, the outcome in jig's words, or the refusal in
  jig's words, with "Look again" after a moved one; past the engine's
  retention, "Unknown". On a goal's page it follows the toolbar; on an
  agent's task's page it follows the conversation, as a chat's.
- **"Older,"** on an agent's task's page, the chat's (5.4): before the
  conversation while an older page remains, reading the page before
  through the same read of its activity, no other.
- **Against the mockup,** the page's way:
  - the composer's text area is named "Message", the chat's, where the
    mockup's is "Write to agent task" with a placeholder "Write to
    T12…" (section 11, the agent's placeholder); its hint is the
    chat's;
  - the composer carries the chat's model choice line, which the
    mockup's agent page does not draw (3.5);
  - the composer shows while Message is offered, the mockup's while the
    task lives, and a refused Message says why in its place (3.6);
  - each confirmation, and an act's outcome and refusal, are 3.6's: the
    mockup's buttons do nothing;
  - its own held card, which no mockup world draws;
  - the acts hidden while the confirmation is open.
- **Where it is decided:** the domain's task page holds the composer's
  words, its pending requests by kind and the last outcome, as a chat's
  page does, and the chat's acts (`Act::MessageWords`, `Send`, `Stop`,
  `Older`) are routed to whichever of the two pages is open; a task
  page's Stop and Cancel open its confirmation first, as `Confirm` with
  the page's task as its object, and `Act::Confirm` sends them. The
  page reads a card for its own task when its document is held, as it
  reads one for a held row. The view draws each act by
  `acts::offers` and `acts::allowed`, and words what cancelling ends
  through the cards' decision facts, as a chat's close does.
- **The look:** TASK_ACTS on the goal's toolbar, the mockup's
  `.task-toolbar`, which draws its Cancel as `.button.danger`;
  AGENT_STEER on the composer's note, the mockup's
  `.agent-steer-note`; TASK_AMEND on the editor's region. The taskbar's
  acts and the composer keep the chat's tokens. `task-pages.css` draws
  them (`browser.md`, 2.5).

### 5.12 Times

- **The view's clock is the shell's.** The view builds at the wall
  time of the shell's iteration (6.2; `browser.md`, 2.1) and says each
  time the domain's documents carry against it, both nanoseconds since
  the epoch. The same documents at the same time give the same words.
- **Four forms, the mockups' own** (`mockups/pages.js`). For a time T,
  E is the whole minutes from T to now, and 0 when T is ahead of now,
  as a machine's clock behind the engine's makes it:
  - *ago,* when something last happened: "just now" while E is 0;
    under 12 hours, E as a span with " ago" ("12 min ago", "1 h 28 min
    ago"); then by the calendar's day, "at HH:MM" on now's day,
    "yesterday" on the day before, the weekday ("Tue") within the six
    days before that, and earlier its date ("20 October"), with its
    year when not now's;
  - *span,* how long something has waited: "under 1 min" while E is
    0; "M min" under an hour; "H h M min" under a day, "H h" on the
    hour; then "N d", in whole days;
  - *clock,* when a moment of a conversation was: "HH:MM" on now's
    day, the weekday and "HH:MM" ("Tue 07:20") within the six days
    before it, and earlier its date and "HH:MM" ("20 October 07:20"),
    with its year when not now's;
  - *the day's rule,* a line before the first turn of each day a
    conversation's window holds, whose turns then say "HH:MM" alone:
    "Today · Thursday 22 October", "Yesterday · Wednesday 21 October",
    or the day's name and date ("Tuesday 20 October"), with its year
    when not now's.
- **The person's zone** sets the calendar's days and the hours the
  forms say: universal time, by section 11's default. The words name
  no zone, as the mockups' don't.
- **The tick.** With each build, the view keeps the instant at which a
  word it drew next changes: the next whole minute of an ago under 12
  hours or of a span under a day, a longer span's next whole day, and
  the next midnight while it drew a word of the calendar's (a later
  ago, a clock, a day's rule). The shell wakes at that instant, and the
  view builds again although the domain's shown count has not moved
  (6.2; `browser.md`, 2.1). A page that draws no time asks for no wake.
- **Where they are said:** a chats line's last activity or end, an ago
  (3.5); a chat's turns and its end, clocks under the day's rules
  (3.5); a card's wait, a span, in the inbox and on a chat's page; an
  update's arrival in the inbox, an ago (3.5). The task pages and the
  accounts page say their moments as their sections do (5.5, 5.8,
  5.9), in universal time, and so does a full deployment's earliest end
  (5.1, the pages not about the work).
- **Fixed in the tests.** The view world, the terminal's goldens and
  the gallery build at a fixed time: the fixtures' `NOW`, and each of
  the gallery's worlds its own (`browser.md`, 7.2). A test of a form
  steps that time to either side of the bound it reads (section 10).

### 5.13 How work runs, and a proposal of settings

Designed, not built (`../protocol/configuration.md`, section 13; jig's
`domain/engine.md`, section 17, for the settings, their revisions and
their proposals; the draft `../draft/configuring.md`, section 5). These
pages are the user's to make: the user works on them from the mockups
drawn ahead on temper's branch `configuring-mockups`, and the option
pages once put to the user are withdrawn. Until the user's pages land,
this section is the draft they start from; the configuring plan builds
the settings first for `temper ctl` (its mission 2).

- **Two pages and a card.** `HowWorkRuns`, "How work runs", lists the
  settings in the person's words; `Proposal { proposal }`, under it,
  reviews one proposal of settings; and a proposal of settings in a
  chat is a card of its own (below). They read jig's `settings`
  document, and each setting they show whole (`setting`, jig's
  `client/domain/contract.md`, 5.1 and 6), again as the document says
  it changed. The settings are bounded by jig's limits and temper's
  (`../protocol/configuration.md`, 13.2), so the page reads them all
  as it opens. A party sees the deployment's settings with a role in
  the deployment, and a project's with a role in that project, as the
  document gives them; the client hides nothing of its own.
- **In the navigation** after "Accounts", whatever the project
  (5.3); current on both pages.
- **How work runs:** the level-1 heading "How work runs"; the line "The
  roles temper's agents take, the ways work starts, and the rules temper
  holds it to."; the newest revision, "Revision 8, accepted by Pat today
  at 14:03" (who decided it and when, 5.12; "temper's defaults" for the
  first start's and "temper 0.4's defaults" for a release's); the
  offer "Tell temper how you work", which starts the configuring chat
  (template 3, `../protocol/configuration.md`, 13.4) and opens it; each
  pending proposal of settings as a line, "Proposal 13 waits for an
  owner. Lee asked for a Reviewer role, from "Reviews"", with "Review".
  Then five sections, each a level-2 heading with its count and a line
  of what it holds, its rows folded until opened, as the accounts' are
  (5.5):
  - *Roles,* the charters: each its name, "a role · temper's stock" or
    "added in revision 6", a tag, and its line, a sentence of its
    instructions' first paragraph; opened, its instructions (temper's
    for a stock one, its own otherwise), its models, where it runs, how
    it waits and its budget, and how many tasks run it, by revision;
    the forge's charter says "Made by temper when a change needs a fix,
    a rebase or a gate".
  - *Ways to start work,* the templates: each its name and line;
    opened, its role, its budget and deadline, and how it closes.
  - *Rules,* what temper holds work to: the deployment's landing rules
    and, for the project chosen, its own, each its branches and what
    it needs ("Needs CI passed · up to date · the gate heavy --gate · 1
    approval by an owner") and who decides it ("Decided by acme/app's
    owners: Pat, Kim"); the policy new projects start with; and jig's
    rules, "Everywhere, always". A project's people and its own policy
    are its own page's, not this one's.
  - *Running work,* the revisions held: the newest, "Newest · 14 tasks
    · new work takes it", and each older one, its tasks' count, and
    "Move them" (jig's `MoveToNewest`), confirmed (3.6) as "Move 9
    tasks to revision 8 at their next run? A task whose role revision 8
    removed, or whose model it no longer allows, finishes on revision
    7."; the answer's count of those that could not move is said after
    it.
  - *History,* the revision records the store keeps, newest first:
    each its number, what it did in a line (a proposal's title, or "a
    release's", "the first start's"), who decided it and when, "Show",
    and "Restore…", which proposes its items again against the newest
    (jig's `domain/engine.md`, 17.7) and opens that proposal's review.
- **Its tags:** "Default", a setting no change is over; "Changed", one
  a change is over, with "changed in revision N"; "Added", one no
  default has.
- **Proposals only, at first** (`../draft/configuring.md`, section 10;
  jig keeps the owner's direct change in its API, and the pages never
  send `ChangeSettings`). A row's act is "Change it in a chat", which
  starts the configuring chat with the row named in its first message.
  "Reset to default…" on a changed default, "Remove…" on an added
  setting and "Restore…" in the history are the person's own proposals
  (jig's `Propose`), each confirmed (3.6) and opening its review page.
- **The card in a chat:** a proposal of settings the chat's subtree made
  is a card at the call that made it, as every proposal is (3.5): the
  tag "Proposal 12" and "against revision 7", its title, one line per
  change in the person's words, before and after ("Testers GPT-5 high
  → GPT-5 medium", "Sub-owners new: temper stops one 4 h after its
  start"), at most five and "and N more", then "Review", to its review
  page, and "Nothing changes until an owner accepts it." Nothing is
  accepted from the card. A person who may not accept it sees "Waiting
  for an owner" in its tag's place and "Pat or Kim decides: it is in
  their inbox.", the parties it waits for as jig's proposal names them.
  In the inbox, its card is the chat's (3.5), with "Review".
- **The review page:** the level-1 heading "Proposal 12 · from "How we
  work" · against revision 7"; "Made by T21 How we work, for Pat, today
  at 13:56"; then two blocks:
  - *temper enforces,* "temper holds work to these, whatever an agent
    does.": the changes of every setting but a charter's instructions,
    grouped as How work runs groups them (roles, ways to start work,
    rules), each before and after in words, with "new" and "removed";
  - *your agents follow,* "Agents read these at each run and are
    trusted to follow them; temper does not check.": the changes of a
    charter's instructions, each as the role and what it now reads.
  Then *Effect*: "New work follows this from the moment it is accepted.
  Nothing restarts."; "9 running tasks are on revision 7:" with two
  choices, "Let them finish on revision 7", chosen, and "Move them at
  their next run"; and "A task whose role this removes, or whose model
  it no longer allows, finishes on revision 7." Then "Show the
  settings", folded, the items as the settings name them (charter 4,
  its models, before and after), and the acts: "Reject…", a reason,
  sent as jig's `Decide`, which the agent hears and revises from;
  "Change something", which returns to the chat with the person's
  words; and "Accept", jig's `Decide` with the
  choice of running work, naming the proposal by its proposer and
  number (jig's `client/domain/contract.md`, 3.2 and 5.2); the revision
  it was made against is its own, and one made against an older is
  withdrawn, never accepted (jig's `domain/engine.md`, 17.6).
- **What the review page says plainly:** a landing rule naming a role
  nobody holds, "Nobody holds this role yet: changes wait for an
  approval nobody can give."; a change of jig's rules that removes the
  landing's requirements (`../protocol/configuration.md`, 13.3),
  "temper no longer holds landing to its rules: the change procedure
  still waits for them."; a role removed while tasks run it, "2 tasks
  hold it on revision 7".
- **Stale:** a proposal made against an older revision is withdrawn by
  the decision that made the newer (jig's `domain/engine.md`, 17.6). Its
  review page keeps its items, dimmed, under the banner "Withdrawn:
  made against revision 7; now 8.", what revision 8 changed in a line
  and who made it, and "T21 How we work is redoing it against revision
  8; its new proposal comes to your inbox."; it offers no Accept, only
  "Open the chat". In the chat the card says "Withdrawn: made against
  revision 7; now 8", and the agent, told so, proposes again against
  the newest, a new card after it.
- **Accepted:** the review page says "Accepted by Pat at 14:03:
  revision 8. New work takes it now.", with what became of running
  work ("9 tasks finish on revision 7", or "moved"), and "How work
  runs"; in the chat, the card's line "Accepted at 14:03: new work
  takes it now." A refusal of Accept (out of shape, judged, full) is
  said under the acts in jig's words, the forge's for a landing rule
  it refused (`../protocol/configuration.md`, 13.2), and the proposal
  stays pending; full names the oldest revision held and offers "Move
  them" there.
- **Drawn ahead of jig:** the names of a revision's tasks, "Show them"
  and how many of them could not move are not in jig's facts (section
  11); the page draws counts alone until they are. A charter's
  documents, "Owners also read acme/handbook process.md", wait on jig
  (`../protocol/configuration.md`, section 11).

## 6. The terminal shell

### 6.1 `temper open DIRECTORY LINK_OR_ADDRESS`

- **DIRECTORY** is the client's private directory. The terminal opens
  its parent as a root (skein-shell's `open_root`) and gives its last
  component to `jig-client-shell-native`, whose files make it `0o700` if
  it is absent and refuse it unless it is a directory of this user's
  with no group or other bits (skein's `io.md`, 5.3). It holds the
  bearer credential's file and the small store's, both the native
  shell's (section 7).
- **LINK_OR_ADDRESS** is one of:
  - the link `temper serve` printed,
    `http://HOST:PORT/jig/v1/sign-in/link?secret=SECRET`: the client
    signs in by it first, offering the credential DIRECTORY holds, if
    any, beside it, so that a person still signed in keeps their sign-in
    (section 7);
  - the engine's address, its clients' listener's `http://HOST:PORT`,
    with or without a final `/`: the client signs in with the credential
    DIRECTORY holds. Before its loop, `jig-client-shell-native` reads
    `DIRECTORY/token` once, as it loads the small store, and the host
    gives it to `jig-client` as held (section 7).

  HOST is an IP address, and must be on loopback (in `127.0.0.0/8`, or
  `[::1]`).
- **Exit status:** 0 once the client closed and drained (6.6); 1 when
  it cannot start, with one line on stderr:

  | Cause | stderr |
  |---|---|
  | not two arguments | `temper open: usage: temper open DIRECTORY LINK_OR_ADDRESS` |
  | not an `http://` link or address, or HOST not an IP address and port | `temper open: not a link or an address: ARG` |
  | HOST not on loopback | `temper open: the engine's address must be on loopback` |
  | DIRECTORY refused | `temper open: DIRECTORY: ` and what skein's io found unsafe or failed |
  | an address given and no token in DIRECTORY | `temper open: not signed in: give the link temper serve printed` |
  | the link refused by the engine | `temper open: the link was refused: use the fresh link temper serve printed` |
  | the engine's sign-ins full (6.7) | `temper open: all N sign-ins are in use; the earliest ends at T; this link stays usable until U; after that, temper serve prints a fresh one when it is tried` |

  A panic aborts the process (fail-stop); the small store and the keys
  make what was pending safe to send again at the next `temper open`.

### 6.2 The loop

- **One host on skein's loop** (skein-shell's `drive`; skein's
  `shell.md`, section 12): temper's client root, its view,
  `jig-client-shell-native`'s transport, and skein's text shell's
  terminal (skein's `ui-text.md`, section 7), with SIGINT and SIGTERM
  from skein-shell's `open_termination_signals`, adopted by io.
- **The text shell's settings** (skein's `ui-text.md`, section 3): the
  name `temper`; the longest line, 4096 bytes; a print interval of one
  second; the status region, the banner's link state (5.1); temper's
  lines, 32 (6.3); and their line bytes, DIRECTORY's bytes and 160:
  6.6's signed out holds DIRECTORY and 84 bytes, and a service's
  credential's line its credential's base64 and at most 57.
- **Time and the seed** are skein-shell's: its clock once per
  iteration, and a seed from the kernel at each start (`seed`), from
  which the client root's children draw their keys' numbers (3.1).
  The view builds at the iteration's wall time, and the loop's
  deadline is the earlier of the domain's and the instant the view's
  words next change (5.12).
- **Each iteration:** the transport iterates; its loaded records start
  the client (3.4); signing in, by the link or with the token DIRECTORY
  holds (section 7); the token written once signed in by a link; the
  transport's events into the domain; the domain's `fire`; the
  transport's requests carried; the view built; each whole line made
  into one command and done; a print, when one is due; the output
  written.

### 6.3 When it prints

As skein's text shell prints (skein's `ui-text.md`, section 4), at the
interval of 6.2. Before the first tree (signing in), only temper's own
lines, each starting `temper: `.

- **temper's lines wait for room.** The host says each of temper's
  lines only within the room skein's terminal gives (`Terminal::room`).
  A line with none waits in the host, in the order they came; a line
  equal to one already waiting adds none (a sign-in tried again, the
  small store refused again).
- **While a line waits, the person's commands wait.** The host takes no
  command from the terminal while one of temper's lines waits in it, so
  no act of the person's makes a new line; SIGINT and SIGTERM still
  close. What waits is then at most one of each line but a service's
  credential, and one credential for each request already sent, at most
  the request table's requests.
- **Notices are taken whatever the room.** The host takes each of its
  client's notices at every iteration, so a held stdout delays temper's
  lines and the person's commands, never the client: the sign-in's
  retries keep their backoff (6.7). Lines that wait when the terminal
  closes are not printed.

### 6.4 How it prints

As skein's text shell prints (skein's `ui-text.md`, section 5). The
first line is `== TITLE == STATUS`: TITLE the page's level-1 heading,
STATUS the banner's link state, with "Behind" after it while the page
is behind. For example, the adoption page, its banner and navigation
left out:

```
== Adopt a repository into shop == Live
…
[9] Forge number: 1
Account:
  [10] (*) acme, selected repositories
  [11] ( ) acme-labs
Repositories:
  - [12] acme/shop, private
  - [13] acme/docs
[14] Next
[15] Owner: acme
[16] Name: (empty) (invalid)
* Required.
Role:
  [17] (*) Owned: temper pushes, opens pull requests and lands
  [18] ( ) Fork: temper contributes through a fork
  [19] ( ) Context: temper only reads
[20] [x] Make it the project's home
```

and so on to `[25] Adopt`. `acme` is an example account on the fake
forge of the tests (section 10), as every name here is.

### 6.5 Numbers and commands

skein's text shell's (skein's `ui-text.md`, section 6). temper adds no
command.

- **A sign-in's address** is an external link (skein's `ui.md`, 6.2),
  which the terminal prints as `TEXT <URL>`, with no number (skein's
  `ui-text.md`, section 5), and never opens: it starts no browser and
  no program. The person opens the address in a browser on the
  engine's machine, where the provider's redirect reaches the engine
  (`../protocol/credentials.md`, section 3), or in one elsewhere, and
  pastes the address it ends at into the page's field, which the
  terminal draws as the browser does (3.5, `ConnectModel`, The paste);
  the page shows the sign-in's end when the `accounts` watch tells it. A browser goes to the address
  in its own tab instead (3.5, `ConnectModel`).
- **A forge's connect or install link** is printed the same way, as
  `TEXT <URL>` with no number, and never opened; the terminal names no
  `back`, so the setup route ends on its own page. A connect link's
  address is on the clients' listener (`forge.md`, 9.1), for example
  `http://127.0.0.1:PORT/forges/1/github/manifest?state=STATE`; an
  install link's is GitHub's,
  `https://github.com/apps/SLUG/installations/new?state=STATE`, whose
  end GitHub sends back to the clients' listener. The person opens
  either in a browser that reaches the clients' listener, on the
  engine's machine in standalone, and the forges' page shows the
  connect's end when the engine tells it (`forge.md`, 5.4).
- **A key's field,** a secret, prints `(hidden)` once written, and the
  command `n KEY` writes it (skein's `ui-text.md`, sections 5 and 6);
  the terminal's line editing echoes the key as it is typed, which
  skein's text shell cannot turn off (skein's `ui-text.md`, section 9).
  `temper ctl add-key` reads a key from a file instead (`tools.md`,
  5.5, area 5).
- **A forge's pasted value** prints the same way: each of the forges'
  page's secret fields (3.6) prints `(hidden)` once written, and the
  terminal echoes it as it is typed. A PEM key spans lines, which one
  command cannot write, so from a terminal an App's key is given with
  `temper ctl`, which reads every pasted value from a private file and
  never from an argument (`tools.md`, 5.5, area 6).

### 6.6 The link, signing out and closing

- **The link's state** is the first line's (6.4). Its change changes
  the tree, so the page is printed again.
- **Signing out** is the banner's "Sign out": the domain asks it, and
  the host gives `jig-client` `SignOut`, which sends jig's sign-out.
  Signed out: the host has the native shell keep none, which leaves
  DIRECTORY's token a zero-byte file, read as none; the main says
  "Signed out." (5.1), the first line reads `== temper == Signed out`
  (skein's `ui-text.md`, section 5: TITLE is "the shell's name when
  there is none"), and temper prints `temper: signed out; to sign in
  again, run temper open DIRECTORY LINK with a link temper serve
  printed`. A sign-out that did not reach the engine leaves the person
  signed in (jig's
  `client/domain/client.md`, section 12): the banner says "Sign out did
  not reach the engine." and offers "Sign out" again.
- **Another party's kept work:** while the small store holds requests
  of a party other than the one `me` confirms, temper prints `temper:
  work of another party is kept in DIRECTORY, and waits for them` once.
- **Out of date** (5.1): the first line reads `== temper == Out of
  date`, and the main says "This temper is out of date: run the temper
  the engine runs."; temper prints no line of its own. What is pending
  stays in DIRECTORY's small store, closed as below.
- **Kept work dropped** (3.4): when `jig-client` drops kept requests at
  start, temper prints `temper: N requests kept in DIRECTORY were not
  sent: another version of temper made them` once, N the count it told.
- **A service's credential,** when an answer carries one (jig's
  `protocol/client.md`, section 8), is printed once: `temper:
  credential for service N, shown once: CREDENTIAL`, the credential in
  base64url. The client keeps no copy of it.
- **Closing:** `q`, the end of stdin, SIGINT or SIGTERM. The client does
  not sign out. skein's text shell closes (`temper: closed`); no new
  exchange starts; `jig-client-shell-native` finishes the small store's
  last write, then closes its connections and files; the process exits
  0. What is pending stays in DIRECTORY's small store, and the next
  `temper open` on the same DIRECTORY sends it with its keys, within
  the engine's retention.

### 6.7 When the engine is unreachable

- **While signing in by the link,** an exchange with no answer, or
  answered not ready (the engine stopping), prints `temper: signing in:
  no answer from the engine; trying again`; one answered busy prints
  `temper: signing in: the engine is busy; trying again`. Either is
  made again, the same link with the same held credential, after a
  backoff of 250 ms, doubling to 8 s. `q` gives up and exits 0. A
  refusal (the link spent or expired) exits 1 (6.1); `temper serve`
  prints a fresh link when a spent one is tried (jig's
  `protocol/client.md`, section 8).
- **The engine's sign-ins full** exits 1 with `temper open: all N
  sign-ins are in use; the earliest ends at T; this link stays usable
  until U; after that, temper serve prints a fresh one when it is tried`
  (6.1), N the deployment's room, T the earliest saved expiry among its
  sign-ins and U the link's own, in universal time to the second,
  `YYYY-MM-DDTHH:MM:SSZ`. Nothing is written to DIRECTORY, and the link
  is not spent: the same `temper open DIRECTORY LINK`, once a sign-in
  has ended and before U, signs in. Full waits for a sign-in to end,
  hours away, so the terminal does not wait for it.
- **Once signed in,** the link is `jig-client`'s to judge (jig's
  `client/domain/client.md`, 8.2). The first line says its state, and
  the page is printed again at each change. Requests made meanwhile are
  kept in the small store and shown as parked; nothing the person did
  is lost, and `q` keeps it in DIRECTORY.
- **A small store that cannot be written** stops that step's requests,
  whose cards say they were not sent (`NotStored`), and prints `temper:
  cannot write the small store in DIRECTORY`.

### 6.8 Pure where it decides

The renderer and the command reader are skein's text shell's, step code
under the subset, bounded by its `worst_case` (skein's `ui-text.md`,
section 3). The host adds the terminal's worst case
(`Terminal::worst_case`) to its memory, and the terminal's operations
(`Terminal::operations`) to its ring's, beside its client's and its
own; it copies none of the terminal's count. temper's view limits put
that worst case past a u32 of bytes, and the terminal starts from them,
as `temper open` makes it, in a unit test (section 10). What temper
decides is in its domain and its view, both step code; golden text
fixes how temper's pages print (section 10).

### 6.9 While a run's text streams

The terminal is chiefly the real loop's test surface (`../domain/testing.md`,
2.4): people use the browser, or a later full-screen terminal. While a
run's text streams, the terminal reprints the whole page at each print
interval (6.3), as skein's text shell does, with the chat's provisional
turn as the page draws it (section 3.5); the print interval is the only
pace it keeps. No rule of skein's text shell is added for streaming.
A command typed while text streams is done as at any time (6.5).

### 6.10 A page's address

The terminal opens at the inbox (3.4) and reaches each page by its
numbered link (6.5), never by a page's address: `temper open` takes the
engine's address or the sign-in link (6.1). A browser opens a page by
its address: a reload, or `/tasks/N` typed or opened, is a new client
at that address (`browser.md`, 2.3), and an inbox's address holds its
filter, so that `/?kind=question` opens the inbox filtered. Of that
open, the path and its load are the browser's; the rest is the client
root's, the same under both shells: a root started at a page's address,
its pages opening that page, asking its reads and drawing it once they
answer. The client's joint world, `tests/client`, starts its clients
so, with no shell (`fixtures::task_page`), and proves the root's part:
in roadmap B2's story a fresh client started at a delegate's address
draws its heading and its "Task brief", as the browser tier's task
pages story does at `/tasks/3` (section 10).

## 7. Signing in and the small store

How jig signs a client in, and how a native client keeps its credential
and its small store, are jig's (jig's `protocol/client.md`, sections 8,
10 and 12; jig's `client/domain/client.md`, sections 11 and 12).
temper's:

- **The printed link** (jig's `protocol/client.md`, section 8): at
  start, `temper serve` prints a sign-in link for the deployment's
  first owner (`../protocol/README.md`, section 4), in the default
  mapping's link route under temper's prefix,
  `http://HOST:PORT/jig/v1/sign-in/link?secret=SECRET`. `temper open
  DIRECTORY LINK` checks that the link names the engine's address on
  loopback and that route, and gives the secret to `jig-client` with the
  credential DIRECTORY holds, if any (`SignIn { with: Link { secret,
  held } }`). The secret is in the command line only until the sign-in
  spends it; the client root passes it on and keeps nothing of it.
- **A sign-in held is kept** (jig's `domain/people.md`, section 3): a
  person still signed in who opens a fresh link with the same DIRECTORY
  keeps their sign-in, its number and its expiry, and the deployment's
  sign-ins don't grow. The engine checks the credential offered as any
  exchange's and answers with none; `jig-client` goes on holding the
  one offered, and `DIRECTORY/token` is not written. A credential the
  engine does not know, or one ended or expired, is no refusal: the
  link signs the person in afresh, and its credential is written. A
  link refused, answered not ready, busy or full, or an engine not
  reached, leaves `DIRECTORY/token` as it was.
- **The bearer credential,** the sign-in's number beside 32 random
  bytes, is `DIRECTORY/token`, a private file (`0o600`), which
  `jig-client-shell-native` writes where the host says, when it says.
  Once signed in by a link, the host reads the credential from
  `jig-client`'s `link()`, through the client root, and has the native
  shell keep it, before anything is told of the sign-in; a new sign-in
  by a link has it written anew, and a sign-in kept writes nothing,
  its credential unchanged; signed out, the host has the native
  shell keep none, which erases it. A later `temper open DIRECTORY
  ADDRESS` has the native shell read it before its loop (6.1), and the
  host gives it back as held (`SignIn { with: Held(credential) }`). It
  is never printed and never an argument, and neither temper's root
  nor its view keeps it.
- **The small store** is `DIRECTORY/store`, `jig-client-shell-native`'s
  file, holding `jig-client`'s records alone: the requests it keeps,
  jig's and the forge's adoptions, pending. temper's pages keep none
  (3.4). It never holds an LLM account's key, token or record, nor a
  forge's pasted value: a request carrying a value is never kept (3.6),
  and a sign-in's record and a forge's credentials are the engine's
  alone (`../protocol/credentials.md`). A pending FORGE request that
  carries no value (a manifest's `Connect`, `Install`, `Forget`,
  `SetUp`, `Reachable`) is kept as any keyed request; one carrying a
  value is not.
- **Loopback only** in standalone (`../protocol/configuration.md`,
  section 2; 6.1), so a bearer token never crosses a network in
  plaintext. Clients between hosts, over TLS, come after standalone.
- **One person.** The deployment's link signs in its first owner;
  signing in through the forge is open (section 11).

## 8. On the engine's side

The engine's service is `../protocol/README.md`'s (section 6). What it
does for the client:

- **The clients' listener** is `temper-service`'s: TCP on the loopback
  address temper's configuration names
  (`../protocol/configuration.md`, section 3), at a fixed port, never 0,
  when a forge of kind GitHub is configured, since a GitHub App's
  redirect and setup addresses name it and the engine refuses to start
  otherwise (`../protocol/configuration.md`, the clients' listener). On
  it, it mounts jig's clients' component (`jig-protocol-engine-clients`)
  with jig's default HTTP mapping's engine end
  (`jig-protocol-engine-clients-http`) under the prefix `/jig` (jig's
  `protocol/service.md`, section 2). A request's path is offered first
  to jig's component, which says whether it claims it (jig's clients'
  listener claims `/jig/...`); then to temper's forge routes,
  `/forges/...` (the routes, below); then to the browser's page and
  bundle, the binary's own or the one the configuration names, or else,
  from a binary built without its own and a configuration naming none,
  to the page for no bundle (`browser.md`, 2.2 and 2.6). Every other
  path is answered not found.
- **The routes,** temper's beside jig's: a GitHub connect's browser
  steps, `GET /forges/N/github/manifest`, `redirect` and `setup`, N the
  forge's number, each the setup's host's run for the state its query
  names (`../protocol/forge.md`, the setup's host). Their answers, their
  headers, their `302`s and their refused pages are `forge.md`, 9.1's.
  The state is their only authenticator: 128 bits or more, good once,
  for its life (`setup.state_life_ms`), for that forge alone; they take no
  cookie and no credential of jig's. A query malformed or past
  `routes.query_bytes` is answered `400`, a method other than `GET`
  `405`, and a forge not configured or not GitHub `404`. No client of
  temper's calls them: GitHub sends the person's browser there.
- **A client's leaving, on the listener.** `temper-service` reads each
  connection's events below skein's server. When the peer ends a
  connection before its call is finished, it aborts the connection, as
  it does one that fails, and the call's owner hears the connection
  closed: for a call under `/jig`, jig's engine end, which takes it as
  the call's end (jig's `protocol/client.md`, 17.4: "A peer's end is
  the call's end"). So a stream ends when the client closes it or its
  process ends, and its room among the profile's streams is free for
  the next call at once (`tools.md`, 8.2).
- **The printed link opened in a browser:** a `GET` of the link's route
  spends nothing, and the mapping leaves it to the application's page
  (jig's `protocol/browser.md`, section 11). With a bundle, temper's
  server answers it with the page, which signs the browser in by it
  (`browser.md`, 2.4); with none, the page for no bundle, which says
  so and names `temper open` (`browser.md`, 2.2), with
  `Referrer-Policy: no-referrer`, and the link is `temper open`'s alone
  (6.1).
- **jig's example guards, as they are** (jig's `protocol/browser.md`,
  12.2), the Host allow-list holding the listener's own address. The
  terminal sends its credential as a bearer token; the cookie is the
  browser's (`browser.md`).
- **jig's clients' component** (jig's `protocol/client.md`) serves jig's
  wire's versions and, for the family `FORGE`, `CLIENT_VERSION` alone
  (4.6). temper gives it no `Mint` in standalone: the printed link and
  services' credentials are jig's own ways of signing in (jig's
  `protocol/client.md`, section 8).
- **The forge's request, decoded before the root** (jig's
  `protocol/client.md`, section 11). An application request of the
  family `FORGE` at `CLIENT_VERSION` is decoded by
  `temper-client-protocol-wire`; one that does not decode, or of
  another family or version, is answered undecodable, as jig's
  reference service answers it. The routing fills the adoption's host
  from that forge's configured `git` base: its host, with `:port` when
  it names one, as the forge's resource paths have it
  (`../protocol/configuration.md`, section 3). It leaves the host empty
  for a forge number not configured, and hands the root the party's
  keyed request: its reply, its sign-in, its key, the digest of the
  record's bytes, and the adoption, or another of the FORGE requests
  (below) with the number and digest of the value it carried.
- **The FORGE requests** beside `Adopt` (`forge.md`, 5.1 and 5.4), each
  keyed, each naming the deployment as its scope (jig's
  `protocol/client.md`, section 11):

  | Request | The value it carries | What the engine does | Its outcome |
  |---|---|---|---|
  | `Connect { forge, how: GitHubManifest { back? } }` | none | the setup's host's `Manifest` run, for the person's own account, always (`forge.md`, 9.1), ending `Done(Manifest { state, target, manifest })`, from which temper renders the manifest's page (`forge.md`, 9.1); the page and `back` held with the state | a link, the manifest's page on the clients' listener for a fresh state (the routes, above) |
  | `Connect { forge, how: GitHubApp { id, key } }` | `key`, the App's private key, PEM as GitHub gives it; `id` rides in the request | `Paste`: the key decoded and matched to the App `id` names, the App's user found, its hook pointed at `https://SLUG.invalid/` (no webhook secret on GitHub: changes are heard from the App's delivery log), its permissions and events checked | connecting |
  | `Connect { forge, how: ForgejoAdmin { first? } }` | the site admin's token, item 1 of the carried bundle (at most 4 KiB including its six-byte header) | `Reconcile` with the paste, its `AdminOrigin` write kept, then `Create` of `first` when given; only after Done is `AdminToken` kept, then the forge connects; every end erases the carried value; never `TakeOver` (`../protocol/setup.md`, 5.5); the configured string-managed form (`ForgeRow.admin`) offers the client's admin Details, with `first`'s org, repository, new project and runner label | connecting; then the watched row, its typed `ended_stop` on a stop, or `first` made and its adoption on Ready |
  | `Install { forge, back? }` | none | `Install`, GitHub alone; `back` held with the state | a link, the install page for a fresh state |
  | `Replace { forge, value }` | a new App key, or a new API token, git token or both | `Paste` with the kept App id, or `Check` against the kept user: it must answer as the forge's user; the old is erased once the new is kept | connecting |
  | `Forget { forge }` | none | the admin's token erased; a managed forge becomes connected (`../protocol/credentials.md`) | forgotten |
  | `SetUp { forge }` | none | an owned forge not set up, or whose set up failed, whose setup says `"start": "asked"`: the routing asks the box for the forge's group, `start FORGE` on its pipe (`../protocol/setup.md`, 7.1), refused "not under the box" without a pipe and "busy" while a group starts; at the box's `started`, its handover taken, then reconciled and connected (`../protocol/forge.md`, 2.2) | setting up |
  | `StartDemo` | the demo model's key, `llm.key`'s bytes, which the routing reads from the file at each start and carries as a pasted value | the demo's start (`../protocol/setup.md`, 8.4): the record written running when none, the box asked for the demo's group, then the demo model's account added and its repository adopted in the person's name, each unless done; refused "not offered" with no demo or one ended | running, once both are done; start failed, naming why |
  | `Reachable { forge, installation?, page }` | none | on every kind, the setup's host's `Reachable { forge, installation?, page }` plan (skein's `forge-setup.md`, 4.8; `../domain/forge.md`, what the picker offers): on GitHub one accepted installation's repositories, `GET /installation/repositories` paged by number on a token minted for the page; on a connected Forgejo what the API token can push to; on a managed or owned Forgejo every repository the site admin can see by id ascending, read on the admin's token, which stays the setup's host's (`../protocol/forge.md`, the setup's host) | a page, `{ rows, more }`: at most `picker.page` rows on every forge, clipped to the forge's own page cap, each the repository's id, its owner's id and login, its name and whether it is private; `more` set when the page is full |

  `first` is a `FirstAsk { org, repository, project, label }`: the
  first repository's fields, as `SetUp`'s, and `label`, the runner label
  its starter workflow runs on (default `docker` in the client and ctl;
  1 to 64 of letters, digits, `.`, `_` and `-`).

  Names in a carried value's braces are its bundle's items
  (`../protocol/credentials.md`, pasted values), never fields of the
  record, which holds the forge, the kind of connect, and a GitHub App's
  id. `installation` is an accepted installation's id, GitHub alone: one
  the forge has not accepted, or one given for a Forgejo, stops the plan
  (`Installation`, `Missing` or `Unexpected`); `page` a page's
  number, from 1, and the picker's "Next" asks for the shown page's
  number plus one; after a last page that came back full, it answers no
  rows. The plan's `reachable` limit is given from `picker.page`. The
  request names no project (section 11).
  The forge's kind, mode, user and installations are the deployment
  part's, never the page's; an installation's page there is built by the
  engine from the forge's configured `web` address (`forge.md`, 5.5).
- **A carried value, as jig carries it** (jig's `protocol/client.md`,
  section 5). jig's clients' component strips the value at decode and
  keeps it in the secrets' store under temper's tag, at a number it
  gives it from 2^63 up (`../protocol/credentials.md`, pasted values),
  before the core sees the request; the core gets the number and the
  value's digest, and the value never reaches the core, a document or a
  log. The request names no number and the client draws none. A value
  is at most 4 KiB, jig's bound on a carried value. A carried value the
  core leaves unused, its request refused, replayed, joined to another
  or cut by a stop, is named in one output of the core's, which temper's
  root forwards to jig's clients' component to erase.
- **`back`,** on a GitHub connect and an install, is the page's address
  the setup route sends the browser back to once GitHub is done
  (`forge.md`, 9.1), checked at decode as `Return`'s `path` is (below);
  none, from the terminal, leaves the route's own page.
- **`Return { source, path }`,** a sign-in's return (3.5,
  `ConnectModel`, The tab's departure), is no forge's: the family
  carries it as temper's one request of the client's own. `source` is
  jig's source of the sign-in (`AccountSignInStarted`), `path` the
  page's address to come back to. The service checks `path` at decode,
  before the root: one leading `/`, not `//`, no `\`, no scheme, of
  visible ASCII, at most 512 bytes; one that is not is answered
  undecodable. Any signed-in person may send it, in their own scope,
  with no role: jig admits it by key, and the root answers it
  `returning` and asks the routing to give the return. The routing
  composes the clients' listener's own address, as the printed link
  names it (`http://HOST:PORT`, the bound socket, never a request's
  Host), with `path`, and gives it to skein's accounts component for
  that source (skein's `oauth.md`, 6.5: `Request::Return`), which
  drops it when no sign-in of that source is under way. The root
  checks no holder: the return only ever sends a browser to a page of
  this engine's own clients' listener, so a source of another
  person's, given a path, sends that person to one of temper's pages,
  where the cookie decides what they see. A return lives as long as
  its sign-in, in skein's component alone: never kept, so a restart
  loses it and the listener answers its fixed page.
- **Who may:** a person whose role on project 0 is `Owner`; in
  standalone, the deployment's owner, for every request above but
  `Return`. jig admits each by key and
  refuses at admission as for any request, in jig's words; the root
  refuses any other party's, before a forge is called or a run starts,
  as the outcome refused (role).
- **The root runs it** (`../domain/root.md`; `../protocol/forge.md`,
  the setup's host): it keeps, in memory, a connect's or a replacement's number and
  digest in the connect's own record, refuses it or asks the routing to
  run its plan, and answers. A connect's end, connected or why not, is
  the forges' page's to read (`forge.md`, 5.4), never the answer's: a
  run's failure, "the pasted value changed", a GitHub App lacking
  permissions or events, each named (the setup's host's `Lacking`;
  `forge.md`, 5.5), are said there; a connect a restart cut is said
  nowhere until a connect is kept (`../protocol/credentials.md`, 2.2 and
  section 8). The outcomes:
  - *connecting:* the run started; a `Manifest` or an `Install` that
    stops, other than busy, answers it too, and its failure is said on
    the forge's row as the last connect's end, never in the answer;
  - *a link:* the address to open, printed and never opened (6.5);
  - *forgotten:* the admin's token erased;
  - *a page:* the picker's (4.4);
  - *refused:* `Connect`'s and `Install`'s as `ForgeRefusal` (4.6) has
    them: the role; no such forge; the forge's kind or mode does not
    take it (a Forgejo's install, a forget on a forge not managed, a
    replacement on a managed or owned forge); busy, since a forge runs
    one writing plan, a connect, an install or a reconcile, and one
    `Reachable` beside it, and a second of either kind is refused, as is
    one past `setup.runs`, which counts both; or, for `Reachable`, the
    forge's reason below, an installation the forge has not accepted or
    one given for a Forgejo (the plan's `Installation` stop), and on a
    managed or owned Forgejo the admin's token's, missing (`Missing`) or
    not a site admin's (`NotAdmin`), as the plan's first step has them
    (skein's `forge-setup.md`, 4.3 and 4.8).
- **The root routes an adoption** (`../domain/forge.md`, section 4;
  `../domain/root.md`, 5.4): jig's people child admits it by key, and
  refuses at admission as jig's answer; admitted, it goes to the forge
  connector, which refuses a forge not configured (no such forge) or not
  connected (not connected), locates the repository by owner and name
  (not found when the forge has none), and makes jig's adoption request
  on the party's behalf
  (`adopt_repository_ask`), which the core judges as the party's own.
  The forge connector's adoption then reads what temper may do there.
  The outcome is kept under the request's key for the retention and
  answered once durable (jig's `client/domain/contract.md`, 3.3).
- **The forge's errors as reasons,** by the forge connector
  (`temper-engine-domain-forge`'s `reason`, from `skein-forge`'s
  `Error`, skein's `forge.md`, section 3.4): `NotConnected` is not
  connected; `NotInstalled` is not installed; `Missing` is not granted
  on a repository whose account has an installation of selected
  repositories, with that installation's page, and not found
  otherwise; `Forbidden` is no access; `Refused` is refused; `Busy` is
  in flight; `TooLarge` and `Full` are too large; `Unavailable`,
  `Timeout` and `RateLimited` are unreachable; any other is an invalid
  answer. A `Reachable` whose plan stops on the forge's error carries the
  same reasons.
- **The outcome, encoded** by `temper-client-protocol-wire`, goes back
  as the family's answer to jig's clients' component, an adoption's and
  every other FORGE request's alike, kept under the request's key for
  the retention, as an adoption's is, at the store's tag 15
  (`../protocol/store.md`), a refusal among them. An outcome that
  carries a link or a page is kept in memory alone, until its state's life ends
  (`setup.state_life_ms` after `Done`): the same key sent again while it
  lives answers the same link and starts no run; after its end, or a
  restart, the key is unknown, as one past the retention is. A
  `SetUp`'s outcome is kept in memory alone too, as the ask it answers
  is (`../protocol/forge.md`, 2.8): the forge connector holds the
  latest per configured forge, and one more for a forge not configured,
  under its request's token, and the same key sent
  again while it is held answers the same outcome and asks the box
  nothing; once a later `SetUp` of that forge replaces it, or after a
  restart, the key answers unknown.
- **A forge's set up, answered** (`../protocol/forge.md`, 2.8): the
  root admits `SetUp` by key, of the deployment's scope, and the forge
  connector by the forge's connection; the routing's box link then
  answers it, the connector's own refusal among what it answers: the
  box asked, *setting up*, or refused, *not under the box* or *busy*,
  or the connector's *not offered*. The answer is the request's
  outcome, and the set up's course, its steps, its `started` and its
  `failed`, is the forge's row's (`forge.md`, 5.4), never the answer's.
- **The deployment's forges, watched** (4.6; `forge.md`, 5.4): the
  routing hands the root the watch of `forges`, with the deployment's
  scope; the core admits it on the party's deployment role and asks the
  forge for the document, and again each time the forge marks it: at
  every connection the routing tells it and at the box's close
  (`../domain/forge.md`, 4.1). Every field of a row is the forge
  domain's: the routing encodes the document as it is, adding nothing.
- **The forge's reads and watches** are decoded by
  `temper-client-protocol-wire` the same way and go to the root
  (`../domain/root.md`, 5.3; `forge.md`, section 9). One the wire has
  no record for does not decode: the routing answers the read with no
  record (unknown) and refuses the watch as unknown, without the root,
  as jig's reference service answers an undecodable one.
- **A change's panel** (4.6; `forge.md`, 9.2): the routing hands the
  root the watch, with its scope and the change's number; the core
  admits it on the party's role in the project and asks the forge for
  the document (`../domain/root.md`, 5.3), and again each time the
  forge marks its subject. Each document leaves held in the decision
  that made it, released once that decision's commit is durable, and
  the routing encodes it, building the pull request's address from
  that forge's configured `web` and its kind (`forge.md`, 9.2).
- **The demo's read** (4.6, 4.8) is answered by the root's forge
  connector, to any signed-in party, from the configuration and the
  demo's record (`../protocol/store.md`, section 2): none while
  `application.demo` is null or the record says ended, and otherwise
  its `project`, its `forge`, its `llm.provider`, that fake forge's
  `fake.owner` and its first of `fake.repositories`, its state, not
  started with no record and running with one, and its `llm.asked`
  cues in order (`../protocol/setup.md`, 8.4).
- **The demo's start, answered** (`../protocol/setup.md`, 8.4): the
  root admits `StartDemo` by key, of the deployment's scope, as
  `SetUp`; the forge connector answers once the start's acts are done,
  or at the first that fails, and holds its answer in memory alone, as
  `SetUp`'s: the latest start's, under its request's token, the same
  key sent again while it is held answering the same outcome; after a
  restart, the key answers unknown. A start while another is under
  way takes that one's answer. The course is the forge's row and the
  `accounts` document's; the answer says only how it ended.

## 10. Testing

The tiers, the fakes and the budgets are `../domain/testing.md`'s (sections 2,
4 and 6.1); the client's joint world is its 2.1 and 6.3. The client's
own tests:

- **How work runs and a proposal of settings** (5.13), designed, not
  built: step tests of their rows, card and review from jig's
  documents, and the gallery's and goldens' pages
  (`../domain/testing.md`, 6.4).
- **Credential-boundary consumption** is checked in the existing
  domain and composed-client fixtures, including the terminal shell's
  startup `Held` sign-in after its children have asked reads (3.4).
  Every cancelled read is consumed visibly and its asker must ask again;
  no new retry policy is implied. The preview's existing view tests
  render jig's supplied `resolved_default`, with catalogue and charter
  orders opposed, explicit choice and no preview, preserving omitted
  request intent (5.5).
- **Step tests:** the domain's pages (what each watches and reads as it
  opens and closes, each act as the operations it sends, the
  confirmation's rule of 3.6, routing by asker; a chat's delegations,
  5.7: a call's tasks by its objects and by its acknowledgement, a
  failed call's and another tool's none, the newest watched up to
  `reads`, each unwatched as it leaves, and a card's facts from a
  task's document in each state; a goal's, 5.8: a task's state from
  what it waits on and from its chip, in each waiting shape, running
  with and without an observation, and in each ending, and a
  dependency's from each kind of link; an agent's task's, 5.9: its
  shape from its executor, priority and label, its thread as a chat's
  (opened, a page, a document's change, a run's text and its end), and
  its run's heading in each phase, with an attempt and none; a
  goal's and a procedure's task's, 5.8 and 5.10: its ended delegates
  read as it opens, by its parent and the ended phase, read again on
  its tree's or its document's change, one read out at a time, one
  asked while another is out read once that answers, and the read's
  page cut; a goal's plan's filter chosen, and "All" again at its next
  open; an agent's task with no priority reading none; the panel's
  slot in each state; each step's slot and the steps done; a task's
  page's acts, 5.11: Message, Stop, Cancel and Release each as its
  request, Stop and Cancel only once confirmed, one request per kind
  at a time, an act not offered, proposable or refused sending
  nothing, a card read for its own task once held, and "Older" as the
  chat's read); the forge side's steps
  (the form's rules, its request, every outcome, the version check; the
  picker's pages by number, "Next" asking the shown page's number plus
  one, a full last page's "Next" and a page past the last answering no
  rows, its ids kept and dropped, a page answered late dropped, busy
  and the admin's token's refusals in words, "Not listed"'s words from
  each forge's read and installations; each FORGE request
  and outcome of section 8, a carried value's fields emptied as it
  leaves and asked again when lost, under the first key; a change's
  panel watched as a change's task page opens and closed as it leaves,
  in the task's project's scope, each delivery decoded and kept whole,
  a document of another version or one that does not decode ending the
  watch as unknown, and unknown and ended kept apart from loading);
  the view (each page from fixture states, read by skein's fake
  person's queries; a delegation's card in each state, in place of its
  call's summaries and its heard result; a goal's page, 5.8: its
  parts in order, the result it gives of each kind, its result in each
  state and none before it ends, no row for the goal, a row's state,
  own spend and dependency line, cut and not, its budget's card
  with each funder, with a deadline and none, with and without models,
  cut and not, and its latest activity's heard result and result line
  with their task's link, a person's words with none, and the chat's
  activity unchanged; an agent's task's page, 5.9: its parts in
  order, its lineage bar with a parent's chip, a requester alone and
  neither, its brief with dependencies and none, its run's heading in
  each phase with a start and none, its conversation as a chat's, its
  result in each state and none before it ends, and its plan only with
  a delegate or a card; a task's page's acts, 5.11: each act by its
  admission (a button, proposable, refused, and not offered: not
  drawn), the goal's toolbar in order, the taskbar's acts, both gone
  while the confirmation is open, the composer with its note and a
  refused Message in its place, the Stop dialog's words, the Cancel
  dialog's from each count of the effect, its own held card where 5.11
  puts it, and each outcome: pending, done, refused, moved; the
  amendment's editor on a goal and on an agent's task, a cut spec read
  only, "No change", a moved draft with no "Send amendment", and its
  request's revision named or not as 3.7 says; a procedure's task's
  page, 5.10: its goal's link when it serves another task and none
  when it serves itself or none, and "Its tasks" over its plan where a
  person task's says "Plan"; its header with its panel shown and its
  part's words loading, unknown and ended; "View on forge ↗" with an
  address and none; each step at its slot with its line, the task it
  runs and none, a queue's place with the tasks ahead named, more and
  none; each hold's notice; CI and each verdict at its head, blocking
  and advisory, and no gate; its bounds; and its ended tasks among its
  live ones by number, "No live tasks." before the read, "None yet.",
  "None." and the more line; a goal's plan, 5.8: its ended rows among
  its live roots by number, before the first, between two and after
  the last, a delegate in both drawn once as the tree's, each ended
  row muted with its ending and its spend and no dependency line; its
  counts, none before the read, live and ended, "more than M ended"
  with a page after and with a cut page, the same under either
  filter; its filter, "All" pressed at the start, "Live only" leaving
  out the ended rows and the more line and keeping the live rows, the
  folds and the held cards; "No live tasks." before the read and under
  "Live only", "None yet." and "None." under "All"; the forge's words
  for every
  path shape and every kind, the form and its picker in each state,
  every outcome, the forges' page with a forge in each state of
  `forge.md`, 5.4);
  `temper-client-protocol-wire` (golden bytes, bounds and fuzz targets,
  as skein's `codec.md`, section 6 has them); and the engine's routing
  of the forge's family both ways, a change's panel's watch to the root
  with its scope, and its document encoded with the pull request's
  address on a GitHub forge, on a Forgejo and with no `web`.
- **The demo's halves** (4.8), in the domain: the model's half held
  on the demo model alone and ended by a usable account of another
  provider, the person's own, and for an owner the deployment's too;
  the demo provider's deployment account ending nothing; a member's
  half reading `accounts` alone, with no `deployment` watch; an
  owner's undelivered or cut `deployment` ending nothing; the owner's
  one watch, open while the demo runs, its start out or failed among
  it, held to the removal, and dropped once neither holds; Ready and
  the setup strip judging the model by the same accounts.
- **The accounts pages' steps and views:** in the domain, the
  accounts page's acts each as its request from the row's names, one
  out at a time, those that confirm only once confirmed, a key's value
  dropped when another row's field is written and as it leaves, "Use
  the API key first" as its move, a row's sign-in followed to its end;
  `ConnectModel`'s address read and printed for each way, a way not
  offered or the demo's showing Choose, its state kept across its steps
  and dropped as it leaves, each way as its request, a sign-in's start,
  its watched end and each of jig's endings, and the welcome's Model
  on the same state. In the view, the accounts page in each state of
  5.5's table, folded and opened, with each act and its outcome under
  its row, the deployment's and the playground's sections, and none of
  the person's own; `ConnectModel` at each step for each way, Ready's
  words for each, and each ending's words and acts; the welcome's Model
  step with no forge's line, and with each (3.5, The welcome): under
  way with its steps, ticked, none adopted and failed; the navigation's
  dot in each colour and none; the banners' ways.
- **Golden text:** temper's pages as skein's text shell prints them,
  kept as files beside the tests and changed only by hand: the inbox
  with entries of each kind, the chats, a chat with its conversation and
  a proposal in place, a chat with a delegation's card running and one
  landed (5.7), a board, a goal's page with its plan, a row's spend
  and dependency line, an ended row, its counts and its filter, and its
  budget (5.8), an agent's
  task with its brief, run, conversation and result (5.9), the
  changes, the adoption form in each state of 4.4 and 4.7,
  its picker on a GitHub and on a Forgejo forge, a forge not connected
  and "Not listed", signed out, the accounts' banner, `ConnectModel`
  with a sign-in's address, as the terminal shows it, a key's field
  written, and an account in
  each state of 5.5, and the forges' page with a forge in each state of
  `forge.md`, 5.4, a connect's link, and a pasted value's field written
  (`(hidden)`).
- **The terminal's machine world,** in `temper-client-shell-terminal`'s
  own tests (`../domain/testing.md`, section 7): the host against a scripted
  stdin and stdout and a scripted transport: signing in by the link and
  by a kept token, a page printed after an act, the end of stdin, an
  interrupt, `q` drained. And, in a unit test, the terminal made from
  `limits::native()`'s limits, as `temper open` makes it, so the gate
  meets a refusal of temper's own limits before the process tier does.
- **`tests/client`** (`temper-client-world`), the joint world
  (`../domain/testing.md`, 2.1 and 6.3): temper's client root and view, with
  skein's fake person acting on the tree, joined to temper's engine
  root through jig's clients' component, with no transport between,
  with jig's client worlds' faults and jig's check (jig's
  `client/domain/testing.md`, sections 5, 6 and 7). The forge's request
  and outcome are decoded and encoded by the engine's routing (section
  8), exhaustive both ways. Stories: signing in and seeing the inbox; a
  chat started and answered; a goal proposed in a chat, accepted in the
  inbox, and gone from the chat's card; goals reordered on the board; a
  held task released from a goal's plan; a goal's plan saying which
  task a delegate waits for, the rows' spend and the goal's budget,
  then, the first delegate ended, its dependant's line saying
  "Completed" and the goal's budget counting what it spent; an ended
  goal's result; a goal's delegate opened from its plan, showing its
  brief, its run's heading and its committed turn, the goal reached
  again by its lineage bar, and, once the delegate has reported, its
  run ended and its result; from a running delegate's page, a message
  sent and read by its scripted run's next turn; that delegate stopped,
  confirmed, its own held card shown, and released, running again; a
  goal cancelled from its page, confirmed with what it ends, and its
  delegates ended; a waiting delegate's Message refused, its page
  saying why and sending nothing; an amendment of a goal's
  specification from its page, and one refused as moved asking to look
  again; roadmap B2 whole, on fixtures shaped on the reference case
  (`../draft/reference-case.md`, 6.1, 6.8 and 7.3): a goal's advisor
  reports, the goal's latest activity names it by its link, and its
  page, opened from there, shows its result and reaches the goal by
  its lineage bar; the goal declares two delegates, the second
  depending on the first in a later call: the second's row and its
  page's state say "waiting on task M", and its dependency line names
  the first with its state; the first reports, the line says
  "Completed" and the second runs; the second reports, then the
  goal, and its page shows its "Result" and its budget's card as
  supplied, what its delegates spent among it; along the way, each
  delegate stays on the goal's plan once it has ended, an ended row
  saying its ending, the advisor's from the story's first step, its
  counts saying how many are live and how many ended, and "Live only"
  leaving the ended rows out, saying "No live tasks." once all have
  ended; and a fresh
  client started at a delegate's address draws its heading and its
  "Task brief", the client root's part of the browser's open of a
  page by its address (6.10); a repository adopted
  from the navigation's link, its outcome read and the engine holding it;
  refused: not found, a second home, a field the form refuses; a part
  of the forge's saying "Shown on the forge."; a change into the fake
  forge's repository, handed its pushed branch, its panel watched from
  its task's page as it goes from opening to landed: its deliveries
  following its steps in order, never going back, its pull request's
  number, CI passed at its head,
  its gate's verdict, and at its end its merge commit, the watch kept
  open with nothing moving it; that page read at each step, its steps,
  its step's line and its pull request's line as 5.10 draws them, to
  "Landed · merge commit" with all six done; an ended procedure's "Its
  tasks" listing its ended tasks with their endings; a task that is no
  change watched as unknown, its part saying so; the banner with no
  account, its key way followed to `ConnectModel`'s Details, a key
  added there, Ready, and the banner gone, and the key in no record of
  the small store; a key's request lost and sent again by the person,
  making one account; a person on `ConnectModel`'s Choose pressing
  "Continue with ChatGPT", Signing in showing its address, the
  watched sign-in made, and Ready; in a browser's root, the answer's
  source followed by one `Return` with the way's address and the
  source, then one `Leave` for the address once `Return` is answered,
  none when it is refused, none from the terminal's root, and none
  again on a rebuild or from a root made at the returned address,
  which draws the source's sign-in, then its end; from the welcome's
  Model step, the same with `Return` naming `/welcome?at=ready`, and a
  root made there drawing Ready with no model line until the
  `accounts` document is delivered, then "Model: ChatGPT · signing
  in" while the sign-in is under way, then the account made; a program's login declared from
  Details and its command shown on Ready; the accounts page's rows
  folded, one opened and a key added to its account from the row; a
  limited account's words, "Use the API key first" moving its key
  up, and its task waiting; on the fake forge's GitHub, the picker's pages by
  installation, a row chosen and adopted, its ids in the request, a
  repository of a selected installation not granted and "Not listed"
  saying so with the installation's page, and a forge not connected
  offering none; on the fake forge's Forgejo, a forge connected from the
  forges' page by its pasted tokens, the value in no record of the small
  store, no document and no answer, a pasted request lost and sent again
  making one connect, a replacement answering as another user refused,
  and a person whose role does not hold the deployment's connectors
  refused; the welcome's Model step reading its forge's line from the
  `forges` watch's row, under way while the forge connects, then
  ticked with ORG/NAME and its project once its first repository is
  adopted; with jig's faults (drops,
  the engine restarting, a double press, a reload with an adoption
  pending, kept work), each request made once. jig's check holds in
  every story, said in temper's phrasebook, which temper's world gives
  jig's referee (jig's `client/domain/testing.md`, 6.2).
- **The routes** (section 8; `forge.md`, 9.1), in temper's clients'
  world (`../domain/testing.md`, 2.1): requests to the service's
  routes against the fake forge's GitHub, each answer, its headers and
  its `302`; a state used, unknown or past its life refused; a query
  past `routes.query_bytes`, a method other than `GET` and a forge not
  GitHub refused; a path jig claims never reaching them. No test calls
  github.com or uses a real credential.
- **The one-process world** (`../domain/testing.md`, 2.5): skein's fake person
  in front of temper's terminal client (its client root, view and
  transport), joined through bytes to `temper-service` on skein's
  simulator: the same stories through jig's default HTTP mapping and
  `temper-client-protocol-wire`, killed and restarted with an adoption
  pending.
- **The real loop's terminal** (`../domain/testing.md`, 2.4): temper's client
  root and view under skein's text shell, over jig's native transport,
  in the same loop as the service, driven by a typist that types
  commands and reads printed pages, with a fake LLM, forge and git. Its
  live-text story: a reply held after its first piece is read printed
  under "Writing…", then replaced by the committed turn.
- **The process tier** (`../domain/testing.md`, 2.6): the real `temper`
  binary; skein's fake person through temper's client signs in by the
  printed link, adds the deployment's key on `ConnectModel`, and
  adopts a repository on an owned Forgejo
  (`../protocol/setup.md`, section 2), with "It runs CI" checked, since
  its runner runs Actions. ChatGPT's sign-in from the welcome's Model
  step, against the fake issuer, the person's root a browser's: its
  `Leave` followed through the issuer and the listener's `303` to
  `/welcome?at=ready`, Ready there naming the account, and the account
  still signed in on the accounts page after a restart. And `temper open` itself, stdin and stdout
  piped: it signs in by the printed link, prints the inbox, adopts a
  repository by commands, and exits 0 on `q` after a drained close.
  The first run (`../draft/first-run.md`, 2.1): the chat's card for the
  change it delegated read landed, with its words (5.7). And
  jig's attack story that needs no browser, a rebound Host, against
  `temper serve`'s clients' listener: a request naming a host off the
  list is answered `421`, with nothing done (jig's
  `protocol/browser.md`, section 15). The cross-site request and the
  framed page need a browser, and come with temper's browser tier
  (`browser.md`, section 4).

## 11. Open questions

- **The mockups' question card:** the asker's lineage and its link
  ("Open T3 ↗"), and choices as buttons beside "Answer in words"
  (`world-solo.js`). jig's question carries words alone, so choices
  need its question form to carry them, an upstream change; the lineage
  needs the asker's documents on the inbox page. Default: the asked
  task's link and an answer in words (3.5).

- **Pages the mockups do not show:** a project's page with its
  resources, findings, notes and settings (policy, people and roles,
  spend; jig's `client/domain/pages.md`, section 6, and the forge's
  parts in `forge.md`, 5.3 and 7.3); the deployment's page but its LLM
  accounts, which the accounts page shows an owner, and its forges,
  which the forges' page shows (3.5); the "Tasks" list of a project
  and the deployment's hosts, which the mockups draw ("Tasks",
  "Workers"), after jig's pages (`client/domain/pages.md`, 6.3 and 7);
  a procedure's task beyond 5.10's page, and a person task beyond a
  tracked task's header and plan; making a service and showing its
  credential once; another party's kept work. Default: not in temper's
  client; the person acts on them through the engine's configuration,
  or not at all.
- **The form's place:** among a project's settings, once temper has
  that page (`forge.md`, 5.3). Default: a page of its own, from the
  navigation.
- **The forge's `Refused`, split** into its causes (another project
  writes the repository; the project has a home; another deployment's
  branches under the prefix), so the form names the one. Default: the
  sentence of 4.7 naming the three.
- **The forge as people's sign-in provider:** temper's own routes on its
  clients' listener run the forge's OAuth flow, read who the person is,
  and ask jig's clients' component to mint a credential for that
  identity (`Mint`, with the forge's provider number, a provider of
  kind application in jig's `providers` member, the person's id there,
  their login and name; jig's `protocol/client.md`, section 8).
  A browser then holds it as the default mapping's cookie; a terminal
  needs a flow of its own, such as a device code. Default: the
  deployment's link only.
- **What a page keeps across a restart:** where the client was, the
  composers' words and the form's values, as records of temper's own,
  which the terminal keeps beside `jig-client`'s small store, never in
  it (jig's `protocol/client.md`, section 10). Default: none; the client
  opens at the inbox.
- **Clients that are not a person's:** a command run once and a
  service acting as a party are the agents' tools, `temper ctl`
  (`tools.md`). A bot (`../draft/factory.md`): none, until the
  factory's lab or run side needs one.
- **Several terminals:** each `temper open` with its own DIRECTORY is a
  client of its own. Two on one DIRECTORY share its token and write its
  small store whole over each other. Default: not supported and not
  detected; a lock on DIRECTORY is the candidate, which `temper ctl`
  takes for its own calls (`tools.md`, 8.3).
- **A fresh link from the client:** default: the person runs `temper
  open` with the last link, whose refusal makes `temper serve` print a
  fresh one.
- **The person's time zone:** default universal time (an offset of 0)
  for the calendar's days and the hours 5.12's forms say, which name no
  zone, as the mockups' do; the browser's zone, an input of the shell's,
  is the candidate.
- **A browser on another machine, detected.** A sign-in's redirect
  reaches the engine on loopback; a browser elsewhere lands on its own
  error page, and its person finishes by pasting that page's address
  (3.5, `ConnectModel`, From another computer), or through a tunnel
  (`../../guide/box.md`, step 4). The person says which (with the
  user, 2026-10-23, and decision 971). Candidates for telling by
  itself: before the browser leaves, the page asks the listener for a
  one-time code the engine gave it for this sign-in, and only a browser
  that gets it goes on in its own tab, the others in a tab of their own
  with the paste; or a connection from an address not the engine's
  machine's goes so at once. Later: a client relaying the redirect
  (jig's `protocol/llm.md`, section 10). Default: undetected, the
  person's choice.
- **The first use of a plan,** in the terminal: a card in the
  conversation, or a card that waits to be dismissed, which needs a
  dismissal kept by the engine, as jig's seen is. Default: a card in
  the conversation (5.5), with nothing to dismiss.
- **A key typed in the terminal** is echoed by its line editing (6.5).
  Default: accepted on one person's machine; `temper ctl add-key` reads
  one from a file.
- **The picker for a person who may adopt but not connect:** the
  `Reachable` request's who-may is the deployment's connectors', so a
  project's maintainer without that role sees no list. Default: refused
  for the role (4.4), and the person adopts by owner and name, which
  the engine judges by the project's role as any adoption; a read the
  project's role may make is the candidate.
- **The picker's page as a read:** a page could be a read of the forge's
  family rather than a keyed request. Default: a request, since its
  answer is a fresh read of the forge by the setup's host's plan; keyed
  as any request, and its answer to a
  form that has closed dropped.
- **What the picker's ids do at adoption:** `Adopt` carries the row's
  repository id and owner id (4.6), while the forge connector locates a
  repository by owner and name (`../domain/forge.md`, section 4).
  Default: the connector locates by owner and name as any adoption, and
  refuses as not found one whose id is not the carried one, so a
  repository renamed or replaced between the page and the adoption is
  never adopted in its stead; the domain's page is asked to say so.
- **The project's own repositories in the picker:** `Reachable` names
  no project, so the engine cannot leave out what the project holds,
  as `../domain/forge.md`, what the picker offers, says the list does,
  and the client holds no list of a project's repositories
  to leave them out itself (4.3). Default: every reachable repository
  is listed, and adopting one the project holds is refused by the
  engine as any adoption; the request naming the project, the engine
  leaving those out, is the candidate (`forge.md`, section 10).
- **A connect's end told:** the forges' page reads how a connect ended
  on the deployment's part. Default: that read, watched while the page
  is open; no notice in the inbox.
- **Hiding the demo** (`../draft/first-run.md`, 2.2): the banner and
  the suggestions show while their halves hold (4.8). Default: nothing
  hides them, and nothing is kept to dismiss them.
- **"Reset demo"** (the first-run mockup's earlier draft): the demo's
  chats and their work set aside for a fresh start. Default: not
  offered.
- **"Signed in as the deployment’s owner"** (5.6) reads `me`'s role on
  the deployment, which the engine gives only once it seeds the
  deployment's owner (jig's t31a). Default: the project's role
  meanwhile ("Signed in as the project’s owner").
- **The demo's and the agent's placeholders** ("Try a suggested
  request…", the first-run mockup's earlier draft; "Write to T12…", the
  agent's task's composer, 5.11): skein-ui's text area takes a
  placeholder, which a chat's composer draws (3.5). Default: neither
  drawn, the agent's waiting for its page's owner.
- **The composer's selected account and source** (5.5): jig's
  `PayerReadiness` could carry the account and source its selection
  chose (`jig-domain-core`'s `catalogue.rs` already calls `select`).
  The view could then read jig's choice for every payer without
  translating selection over the viewer's own account slice. Default:
  name an account only in that slice, with the readiness cross-check
  and expiry fallback in 5.5; other work names the model alone, with
  the demo provider's exception.
- **A time's moment on demand:** an ago ("12 min ago") has no title
  or hover saying its moment, since skein-ui's nodes carry no title.
  Default: none; a time node with its moment, skein-ui's, is the
  candidate.
- **The terminal's reprint:** skein's text shell prints the whole page
  when it changes (skein's `ui-text.md`, section 4), so a page with a
  time under 12 hours old prints again each minute while it is shown.
  Default: as printed; a terminal shell that says every time as its
  moment, with no wake, is the candidate.
- **An agent's conversation drawn as a chat's** (5.9): the agent's
  page folds a message's calls as a chat does, and keeps each result
  and the run markers as work lines, which a chat hides (3.5).
  Default: as work; the chat's drawing is the candidate, the task
  pages'.
- **Which person messages are the person's** (3.5): jig's turns keep no
  provenance, so a chat draws none of a turn's person messages, and
  the person's words from the thread's words alone. A charter whose
  person's words reach the chat only inside a turn would show none of
  them. Default: as drawn; a turn's message kind from smith's record
  is the candidate, jig's.
- **Seen while no one looks:** a chat's page open in a tab the person
  is not looking at, or in a terminal left open, marks the turns that
  arrive as seen (3.5). Default: open is seen; the page's visibility,
  an input of the browser's shell, is the candidate.
- **A delegation's tasks, keyed** (5.7): jig's thread keeps a
  `delegate` call's `objects` empty, so the domain reads the tasks
  from temper's acknowledgement and, at a result, from temper's inbox
  words: words of the engine's that the client depends on, each marked
  so where it is written and where it is read. Default: the objects
  when jig's thread keys them (a jig change and its repin), the words
  meanwhile, both ends temper's and joined by the process tier's first
  run (section 10).
- **A change's live stages** on its card (5.7; `forge.md`, 2.1): the
  steps are a stand-in, ticked from the result. The engine publishing
  the change's step as its task's part, which the forge's side reads,
  is the candidate, about 1.5 to 3 thousand lines. Default: the
  stand-in. The change's task page draws its live step from its panel
  (5.10); the card does not watch it.
- **"Landed into main"** (the first-run mockup's earlier draft): the
  branch a change lands into is a parameter of the forge's procedure,
  which jig's contract does not type, and the landing words name the
  merge commit alone. Default: "Landed · merge commit …"; the result's words
  naming the branch are the candidate.
- **"CI passed" on a card** says what the stand-in's steps say: a pull
  request merged by someone else is landed like any landing
  (`forge.md`, 6.2), CI or not. Default: the words, until the live
  stages.
- **The pull request's link on the card** (the first-run mockup's
  earlier draft, "OWNER/NAME#N on Demo forge", now Playground forge): the forge's part, which the forge's
  side does not read yet (4.2). Default: the link to the task's page.
- **An added dependency** on a task's page (5.8, 5.9):
  `Dependency.added` says when and by whom a dependency was added.
  Default: a mark in its item of the dependency line, who added it and
  whether it keeps the task, which keeps the line's shape; when, in
  `ctl task N` alone.
- **The rest of a goal's authority:** its tools, grants, executor kinds
  and note scopes, which jig supplies as codes and patterns, not
  words. Default: not on the goal's page.
- **A goal's page against its mockup** (5.8), each a default the user
  may change; the plan's columns, "Wakes on", "Input" and "Writes" the
  user's picks (10-23), settled for now:
  - the deadline: the budget's card's "Deadline", "None" when it has
    none, and not the header's;
  - the plan's columns: no heads, the spend "N spent" in its row;
  - "Wakes on" under "What was asked": supplied (`Task.wake`), not
    drawn yet; later, in jig's words;
  - "Input", "Writes", "Run N" and the pool's line under the plan:
    not drawn; the first two and the pool's name are not supplied;
  - the plan's counts: "Its delegates: N live, M ended." of its own
    delegates, not the heading's "N tasks" and the counts by phase
    ("3 running, 3 in queues, 7 ended") of its subtree, which a window
    of the tree would count wrong and whose queues are the forge's;
  - its ended delegates' own delegates: not read, each on its parent's
    page;
  - its filter: on its own line after the counts, where the mockup
    puts it at the heading's right.
- **An agent's task's page against its mockup** (5.9), each a default
  the user may change:
  - its dependencies, which the mockup draws on no agent's page: the
    dependency line under its brief;
  - a cut document's dependency line: no last item, since the
    document's `cut` does not say which list was cut; a cut of its own
    on the dependencies, as a plan row's, is the candidate (a jig
    change);
  - a message's calls: folded as a chat's, in the disclosure named
    "N tool calls" and described by their names (3.5), each call a work
    line inside it;
    its results stay work lines where their message has them, which
    the mockup doesn't draw;
  - its plan: drawn while its tree holds a delegate or a held card,
    where the mockup draws none;
  - its acts are 5.11's.
- **A task's page's acts** (5.11), each a default the user may change:
  - Stop confirmed on a task's page, where a chat's Stop is sent at
    once: stopping holds the task until it is released, and the task's
    page acts on work the person may not be watching. The chat
    confirming its Stop too is the candidate;
  - Cancel on a goal's page only, as the mockup draws it; an agent's
    task's Cancel, which the engine offers the person its requesters
    lead to, is reached from its goal or `temper ctl`;
  - the amendment's editor on the pages: the specification's words
    and, on a goal, its tracking and priority, the model choice being
    the choice line's; the rest of an amendment is `temper ctl
    amend`'s. A draft whose task moved is discarded and opened again,
    not carried onto the new revision: the person rereads what
    changed;
  - a goal's page has no composer, as the mockup, though the engine
    offers Message on a goal.
- **A change's page against its mockup** (5.10), each a default the
  user may change:
  - "Its tasks"'s rows: 5.8's plan rows, with their executor, state and
    spend, where the mockup draws each as a gate's row, its kind and
    its phase, and heads the region "made by the change procedure…";
  - a gate named "Gate N · kind", where the mockup names the landing
    rule that adds it, and "No other gate." with no "its landing rules
    add none", since the panel carries the gates, not the rules;
  - its step's line, its hold and its bounds, which the mockup does
    not draw;
  - the "Changes" breadcrumb, not drawn until the `Changes` page is.
- **Older ended delegates and a goal's older activity** (5.8, 5.10):
  a goal's plan and a procedure's "Its tasks" read one page of their
  ended delegates, the first made, and say "More of its ended tasks
  are not shown." past it; a goal's page reads its latest activity's
  page, not the older ones. Default: one page of each, since a
  procedure makes a handful and a goal's plan is read for what is
  under way. An "Older" read is the candidate, as an agent's task's
  conversation has: the ended page's `after` as the next query's
  start, and the activity's older pages by their `before`; with the
  view's words and tests and the client world's story, about two hours
  of work.
- **A sign-in's endings jig gives no fact for** (5.5, A sign-in): the
  listener's port another program holds, before the browser leaves; a
  ChatGPT account not eligible for plan usage; plan usage declined at
  the provider, apart from its consent's `declined`; and a missing
  scope said in its own words, where jig gives its number. Each waits
  for a fact of jig's ending (jig's `client/domain/contract.md`, 7.1;
  the providers' facts, jig's `domain/providers.md`, 2.5); until then,
  each reads as the ending jig gives, in its words. Default: none drawn
  before jig's fact.
- **A sign-in's end after the welcome's return** (3.5, The welcome):
  a sign-in started at the welcome's Model step that ends refused or
  failed comes back to Ready, which then says "Model: none yet" with
  "Connect a model", and draws none of the ending's words, which the
  accounts page and `ConnectModel` at the source still show.
  Candidate: Ready draws the person's last ending under the model's
  line while no account is usable. Default: as above.
- **A sign-in started from another client** (3.5, `ConnectModel`): a
  page opened at a sign-in's way, by a reload or a link, knows no
  sign-in of its own, and offers the way's button again; the accounts
  document lists the sign-ins under way, but not which client started
  each. The return names its sign-in by its source in the page's
  address (`ConnectModel`, Come back), so a page come back, or a link
  copied from it, shows that sign-in; one opened at the way without a
  source does not. Candidate: the page shows the person's sign-in under
  way for that provider, with its end. Default: the way's button
  again.
- **Which tasks hold a revision, and which could move** (5.13). jig's
  `settings` document counts the live tasks holding each revision, but
  a task query has no revision among its keys (jig's
  `client/domain/contract.md`, 6.1), and nothing says before a move or
  an acceptance how many tasks could not move. Default, asked of jig in
  the configuring plan's mission 2 (revisions held by tasks, with moves):
  a task query keyed by revision, its live group only (a scan, which
  costs live tasks no more than any key), and a proposal's or a move's
  count of tasks that could not move. Until then "Running work" and the
  review page draw counts alone and say in words which tasks stay.
- **How work runs' place and look** (5.13). The user's to make, from
  the mockups on the branch `configuring-mockups`; the option pages put
  to the user are withdrawn. Default: 5.13 as drawn, until the user's
  pages land.
