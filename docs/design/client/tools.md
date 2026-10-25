# Tools: temper for agents

Provisional, 2026-10-17, revised 2026-10-23. How agents drive temper:
they read what it holds, act through jig's operations and wait for what
changes, with `temper ctl`, a command line. It serves the agents
(smith's) that QA and develop temper's core: they drive a throwaway
deployment on coding tasks of growing size, then on temper's features,
until the first milestone (`../README.md`, section 2) is usable for real
tasks. Their QA loop extends the tools area by area (5.5). Their bounds
are section 12, and what they take of jig section 13; an MCP endpoint
and the tools' extraction into jig are later (section 14). What is
still open is listed in section 15.

## 1. In one page

- **Agents drive the plumbing, not the pages.** The tools are jig's
  reads, watches and operations (jig's `client/domain/contract.md`),
  jig's helpers where they fit (`shapes`), and temper's forge family.
  An agent never reads a page, a position on a screen or a layout
  (section 3).
- **Each result says what can be done next.** jig's documents carry the
  acts the party may make (made, proposable, refused); every result
  prints them as the calls that make them, with the revisions they name
  (section 6).
- **One table of tools.** Each tool is defined once: its name, its
  arguments, its result, its line of help. The parser, the help, the
  JSON and the agents' guide are derived from it (5.1).
- **Waiting is a tool.** `wait` blocks on a watch until a subject
  changes from what the agent saw, or meets a condition, or a timeout
  passes (5.4).
- **Each request once.** A tool that makes something requires a key the
  agent names; asked again under that name, it gets the first answer,
  never a second chat. Acts that decide name the revision the agent saw
  (section 7). The named key is jig's (section 13).
- **A turn too large for a page is read alone,** by jig's read of one
  turn (5.2, section 13).
- **Thin command line.** `temper ctl DIRECTORY TOOL ...` makes one call
  per process, in `jig-client`'s one-shot mode, over jig's native
  transport, and prints its result (section 8).
- **The deployment under test is a box** (`../protocol/setup.md`, section
  7): the agent's harness starts it, signs in by its printed link, adds
  the deployment's LLM key, and throws it away (section 10).
- **LLM accounts are an area of their own** (5.5, area 5): a sign-in
  prints the address to open and returns, and the agent follows it with
  `wait accounts`; a key is read from a private file and sent in a
  request that is never kept (section 7).
- **Forges are an area of their own** (5.5, area 6): `forges` reads
  each configured forge as the `Forges` page shows it; a connect prints
  its link as text, never opened, or reads its pasted value from a
  private file, and returns, and the agent follows it with `wait
  forges`; `repositories` reads the picker's page, whose rows ride
  `adopt`.
- **Grown by areas,** the first, the fifth and the sixth designed in
  full, the others named (5.5); **later, named:** the MCP endpoint and
  the extraction into `jig-client-tools` (section 14).

## 2. What it is for, and what not

- **The first QA loop.** Agents use temper to do coding work of growing
  size, through growing parts of its surface: a chat that changes one
  file; then larger changes, held changes released, questions answered;
  then goals with plans, decisions, several parties. What breaks is
  fixed, and what is slow made faster, each fix with a story in the
  worlds (11.6).
- **Production later.** An agent driving temper for a person, or a
  service of a project acting as a party, uses the same tools; MCP gives
  them to agents that have no shell (section 14).
- **Not the person's pages.** temper's pages are the person's, proven by
  the fake person's stories in `tests/client` (`README.md`, section 10).
  No tool waits for a page: a tool reads jig's documents, so no page is
  a tool's prerequisite.
- **Not the browser.** Once temper has one, its QA is agents driving the
  browser as a person does. These tools are no way around it.
- **Not the deployment.** Starting temper with a forge, a runner and a
  model is `../protocol/setup.md`'s; resetting it is the harness's, never a
  tool's (section 10).

## 3. Why the plumbing, not the pages

- **The pages are for people.** The text view's numbers are positions
  that change between prints, the banner and navigation cost an agent's
  context on every read, and a page shows only what its mockup chose. So
  the text view is no agent's driver; these tools are, for temper
  (`../draft/qa.md`, 4.2).
- **jig's client was made for this.** It is presentation-agnostic: "a
  browser, a terminal, a chat bot, a one-shot command and a service with
  no presentation hold the same client" (jig's `client/domain/client.md`,
  section 1). Its documents are typed, with stable numbers, revisions
  and offered acts (jig's `client/domain/contract.md`, 5.1 to 5.3).
- **Offered acts replace discovery.** An agent learns what it may do
  from the document, as a person learns it from buttons: each act made,
  proposable (to whom) or refused (why), with the bounds a draft is
  compared with.
- **The engine judges again.** Whatever an agent sends, the engine
  checks as it checks a person's request: its role, its pool, the
  revision it names. The tools add no authority and remove no check.

## 4. The crates

```
temper-client-tools          the table, a call, the runner, digests and conditions, the rendering (text,
│                            JSON), the argv parser and help, the guide, the limits; the tools' root
├── jig-client               jig's client and its helpers (shapes, facts, thread, words)
├── jig-client-protocol-link the configured settings wire limits
├── jig-client-protocol-wire bounded whole settings values
├── temper-client-domain     the forge's client side and its words (README.md, 3.1, section 4, 4.1 and 4.7)
├── temper-client-protocol-wire  the forge family's records (README.md, 4.6)
├── skein-ui                 phrases: jig's words and the tools' own, rendered as text
└── skein-json               the JSON writer and tokenizer
temper-client-shell-cli      the host: argv to a call, the one-shot host over jig-client-shell-native,
│                            the directory's lock and address, printing, exit status
├── temper-client-tools
└── jig-client-shell-native  jig's native transport: HTTP, the small store's file, the token's
temper                       the binary: `temper ctl` runs the host
```

- **Named by the client naming rule** (jig's `README.md`, section 15):
  the tools are to agents what `temper-client-view` is to people.
- **jig's part apart from temper's.** `temper-client-tools`' module
  `jig` holds what is generic: jig's part of the table, the call, the
  runner, digests and conditions, the rendering. It names no temper item:
  a boundary test fails the gate when a file under `src/jig/` names
  `temper`, or an item of the crate outside `crate::jig`. Extraction is then
  that module moved into a crate of jig's (section 14).
- **The forge's side is reused, not copied.** The tools' root holds the
  forge's client side of `temper-client-domain`, its adoption's ask,
  owner and words, and none of its pages.
- **The tools' root** is a client root in jig's one shape (jig's
  `client/domain/root.md`, section 3): `jig-client` held; two children,
  each an owner and an asker, the runner and the forge's side; routing
  by asker. It holds no page and no view, as a service's root holds
  none (jig's `client/domain/root.md`, section 11). The runner is one
  owner, `Runner`, 3 (`README.md`, 4.6), and so one asker (jig's
  `client/domain/root.md`, section 15). Each handle and key it gives
  the client carries its call's number in the high 32 bits of its
  number, calls numbered from one past the highest among the keys
  kept at `Start`: several calls in flight need nothing new of the
  root, and a new key never meets a kept one.
- **Words:** jig's for jig's enums (`jig-client`'s `words`) and the
  forge's of `README.md`, 4.1 and 4.7, so an agent and a person name
  things alike. The view's overrides for its mockups stay the view's.
- **Step code under the subset:** the runner, the rendering, the table,
  the tools' root. **Ordinary Rust:** the host.

## 5. The tools

### 5.1 One table

- **A tool is a record of the table:** its name; a line saying what it
  does; its arguments, each a name, a type (a number, a word from a set,
  a text, a file, a duration), whether it is required, and a line; its
  result, a typed record; whether it is keyed (section 7); its area
  (5.5).
- **Everything else is derived from it:** the parser and `temper ctl
  help [TOOL]`, the JSON of arguments and results, and the tool names
  the guide may use (11.5). A test checks that each derivation names
  every tool.
- **A call is a value:** a tool's name and its arguments, decoded from
  argv or from JSON and checked against the table before anything is
  sent. The runner takes a call and gives back its result; the host
  interprets neither.
- **Projects by name or number;** a name is resolved through `me`. A
  name is never decimal digits alone, and no two projects share one
  (jig's `domain/core.md`, 6.1), so an argument is a number or a name,
  never both, and names one project. Until jig gives projects names
  (jig's `STATUS.md`, section 3), `me` gives none, and a project is
  taken by its number.

### 5.2 Reads

A read answers with a document, or a page and its cursor, and, for a
watch's subject, its digest (5.4). It needs no key.

| Tool | What it answers | jig's |
|---|---|---|
| `me` | the party, its projects with their roles and pools, what waits for it counted, the bounds of what it may create, configured provider offers and key-byte ceilings, the retention | `me` |
| `inbox [--project P] [--kind K] [--root N] [--after C]` | what needs the party, then updates, each with its acts | `inbox` |
| `tasks --project P [--parent N] [--label chat\|goal] [--tracked] [--mine] [--ended] [--order activity\|priority] [--after C]` | task lines of one group: the live tasks, or with `--ended` the ended; with `--parent`, task N's delegates alone | `tasks{query}` (jig's `client/domain/contract.md`, 6.1, its parent key) |
| `task N [--tree] [--activity K] [--run]` | a task's document, with its acts; with its live tree, its K latest activity lines, its live run, in one answer | `task{n}`, `tree{root}`, `activity{n}`, `run{n}` |
| `chat N [--before C]` | a conversation: turns, words and, where the run's host gives it, provisional text, in order, a page from its end | `activity{n}`, `run{n}`, through `thread` |
| `activity N [--before C] [--limit K]` | a task's activity, a page from its end | `activity{n}` |
| `turn N A.T [--part K] [--from B]` | one turn of task N, attempt A, turn T, whole: its text, each call and its result | `turn{n, attempt, turn}` (jig's `client/domain/contract.md`, section 6) |
| `run N` | a task's live run: its attempt, its provisional text where its host gives it, its current call | `run{n}` |
| `settings` | the settings: the newest revision's number; each setting, by kind and number with its name, following its default or a change and the revision that last changed it; each revision held, with who decided it, when, the proposal it came from and how many live tasks hold it; the pending proposals of settings | `settings` |
| `setting charter N [--revision R]`, `setting template\|policy N`, `setting rules` | one setting's value whole, as the newest reads it, or a charter as held revision R reads it; with `--json`, the value in the form `change-settings` takes | `setting{kind, number, revision}` |
| `revision R` | a revision: the revision it was made against, when, who decided it (a party, the release or the first start), the proposal it came from, whether it moved running work, and its items, each set with its value whole (or its size, too large, to read with `setting`), removed or reset; a revision forgotten past the history names the oldest kept | `revision{n}` |
| `proposal tN/K\|pN/K` | a proposal of any kind, including its settings review: current values, origins, proposed values and the newest revision; a decided or withdrawn proposal retains its state | `proposal{proposer, number}` |
| `revisions [--before R]` | the history a page, newest first: each revision's record and its items without values, cut at the page's bound with how many more; the last page says whether older ones were forgotten | `revisions` |

- **A revision's value too large to print** says `too large, N bytes; read
  with setting charter N --revision R` for a charter, and `too large, N
  bytes; read current value with setting KIND [N]` for a template, policy
  or rules. Only a charter takes `--revision`; the other reads serve the
  current value, including when the hint appears in a historical revision.
- **`tasks` reads one group a call.** jig answers a task query in two
  groups, live then ended, each with its own cursor (jig's
  `client/domain/contract.md`, 6.1), and a printed cursor is one
  group's. Without `--ended`, `tasks` reads the live group, `--after C`
  its cursor; with `--ended`, the ended group alone, as jig's view reads
  it, `--after C` the ended group's. The call that reads on keeps
  `--ended`, whose help reads "Only ended tasks". An agent that wants
  both groups makes two calls; the printed cursor is jig's, with nothing
  added.
- **`tasks`' order:** by number, ascending, without `--order`, with
  `--ended` or without; by activity or priority with it. The tools never
  send jig's other orders, `Changed` and `Ended`: no command line makes
  a call with either, and such a call is refused a command line, never
  given a guessed one, so that a call's command line parses back to
  that call (6.1).
- **`inbox` reads needs, then updates,** in the one call. jig reads the
  inbox a group a call, needs or updates, each with its own cursor, and
  each kind in one group. A call reads needs first; while needs has
  more, its page is needs alone. Once needs ends on a page, updates'
  first page follows it when both fit in a page's rows (section 12);
  when they do not, the call that reads on starts updates. No page
  passes the rows, and no update is read twice.
- **The inbox's printed cursor** is one group byte, 0 for needs and 1
  for updates, then jig's cursor, whole, or none when the call that
  reads on starts updates; to the agent it stays opaque. The call that
  reads on stays in needs while needs has more, then moves to updates.
  `--after C` reads C's group; with `--kind K`, a C of the other group
  is a usage error, exit 1.
- **A page cut by `--limit K`** reads on with the same read without
  `--limit`, `activity N`, or `activity N --before C` with the C it was
  given, which prints the page whole, its K rows again, then reads on
  at jig's `before`. `task N --activity K` that cuts a page prints
  `read with: activity N`. The printed cursor stays jig's: at most a
  page's rows repeat, and no row is skipped.
- **Provisional text is the run's host's.** An agent process gives none
  (`../protocol/llm.md`, section 7), and `chat` and `run` then print none:
  a run shows by its activity, and its words as each turn commits.
- **A turn too large for a page** comes in a page by its size in place
  of its bytes (jig's `client/domain/contract.md`, 5.1).
  `activity`, `chat` and `task --activity` print it as one line, its
  number, its size and the call that reads it: `turn 3.7 · 412 KB ·
  read with: turn 41 3.7`.
- **`turn`** prints each part of the turn cut to the tools' window, each
  cut with the call that reads on (`--part K --from B`). A turn past the
  engine's bound on one read, or the client's limit for one document, is
  answered with its size and that it is past the limit, exit 0; the
  agent reports it as a finding (section 12).
- **The forge's changes are read on the forge,** with its API or `git`,
  as a person reads them there. A part of the forge's that the engine
  does not serve prints "Shown on the forge.", with the resource in the
  forge's words (`README.md`, 4.1 and 4.2). Of the forge's read family
  (`forge.md`, section 9), the deployment's part is a tool, `forges`
  (5.5, area 6); the others join the table as tools of their own
  (section 14).

### 5.3 Acts

Each act is one keyed request through `jig-client`'s request table,
answered once durable (section 7). `K` marks the tools whose key the
agent must name.

| Tool | What it does | jig's | K |
|---|---|---|---|
| `chat --project P TEXT` | starts a chat | `shapes::chat`, a `Create` | yes |
| `message N TEXT` | words to a task | `Message` | yes |
| `close N [--move D]... [--reason TEXT]` | closes a chat: its live delegates cancelled, or moved to the party where named; the reason empty unless given, as jig's reference UI sends it | `shapes::close_chat`, a compound request | yes |
| `adopt --project P [--forge F] --owner O --name N [--ids R/U] --landing B [--role owned\|fork\|context] [--prefix X] [--no-home] [--no-ci] [--check G]...` | adopts a repository | the forge family's request (`README.md`, 4.4) | yes |
| `answer N M TEXT` | answers question M of task N | `Answer` | |
| `decide tN/K\|pN/K accept\|reject\|pass-up [REASON] [--running stays\|moves]` | decides a proposal: P is `tN` for task N's, `pN` for party N's | `Decide` | |
| `decide N --hold R release\|leave\|pass-up [REASON]` | decides an escalation | `Decide` | |
| `propose-settings FILE [REASON] --against R` | proposes the whole settings set to the deployment owners; FILE or `-` shares the change-settings format and bounds; the optional reason is forwarded unchanged | `Propose` settings | yes |
| `change-settings FILE --against R [--running stays\|moves]` | changes the settings in one decision: FILE (or `-`) holds the items; R is the newest revision the caller read; running work stays unless `--running moves`; answered the new revision's number | `ChangeSettings` | yes |
| `release N --hold R [--again]` | lifts a hold | `Release` | |
| `stop N` | cancels a task's run, the task held | `Stop` | |
| `cancel N [--grace D] REASON` | cancels a task and its tree | `Cancel` | |

- **Settings FILE is JSON:** `{"items": [ITEM, ...]}`, at most one item per
  setting, each `{"setting": SETTING, "set": VALUE}`,
  `{"setting": SETTING, "edit": "removed"}` or
  `{"setting": SETTING, "edit": "reset"}`. SETTING is
  `{"kind": "charter"|"template"|"policy", "number": N}` or
  `{"kind": "rules"}`; VALUE is exactly the `value` member `setting … --json`
  prints. Decoded bounded (6.4): the file's size, the item count and every
  value's bounds are jig's (`ChangeSettings`' own, and the limits the engine's
  `me` gives where it gives them). Past a bound it is refused before anything
  is sent, naming the item and the bound. Unknown members are refused,
  naming them. These tools select deployment settings only.
- **The arguments are jig's,** named as its operations name them, with
  the revisions they need (jig's `client/domain/contract.md`, 5.2): a
  proposal by its proposer and number, `t41/3` for task 41's third,
  `p3/17` for party 3's seventeenth, as every result prints it, an
  escalation and a release by the hold's revision. An agent copies them
  from the acts a result printed (6.1); it never has to know which an
  act needs.
- **`adopt` takes the adoption form's fields and defaults** (`README.md`,
  4.4): forge 1, role `owned`, prefix `temper/`, the home and CI on.
  `--landing` names the landing branch. A context repository is never
  the home: `--role context` turns the home off. `--no-ci` needs at
  least one `--check`, a gate number, each number once. `--ids R/U` is
  a picker row's repository id and owner id, as `repositories` prints
  them (5.5, area 6), carried with that row's `--owner` and `--name`;
  without it the request carries no ids (`README.md`, 4.6). The form's
  checks are the parser's: a call that fails one exits 1 before
  anything is sent.
- **No confirmation.** The person's client says what an act will do
  before sending it (`README.md`, 3.6); an agent reads it in the
  document's acts and their effects, and the revision is its promise.
- **A draft beyond the party's bounds** is compared with the bounds the
  document gives, as the person's client does (jig's
  `client/domain/contract.md`, 5.3): within, sent; between the bounds,
  sent as `Propose` where it is proposable, and the result says so; past
  them, refused by the client, nothing sent. Where `me` gives no bound
  for a draft, it is sent and the engine's answer printed.

### 5.4 Waiting

```
wait SUBJECT [--since DIGEST] [--until CONDITION] [--timeout DURATION]
```

- **The subject** is a watch's (jig's `client/domain/contract.md`, 7.1):
  `me`, `inbox`, `task N`, `tree N`, `run N`, `accounts` (area 5 of
  5.5), `forges` (area 6), or a task query as `tasks` takes it, with
  what its watch takes alone: `inbox` with no flag, the inbox's watch
  being the party's whole inbox; `tasks` without
  `--after` or `--ended`, `tasks{query}` following live tasks. Any
  other is a usage error, exit 1.
- **A digest** is the first 8 bytes of the SHA-256 (skein's `lib.md`,
  6.2) of the subject's document as `--json` prints it (6.2), without
  its `digest` field, printed as 16 hex digits. A read and a wait digest
  one form, the watch's document: for `inbox` and `tasks`, whose read is
  a page and whose watch a window, the rows the watch's first window
  holds, in its order, and whether more follow, within the tools' page
  of rows (section 12). The read makes that form from its own rows and
  prints its cursor beside it, outside the digest. A read's digest is
  therefore a wait's first delivery's while nothing moved between them,
  so a wait `--since` a read's digest never answers at once. Two calls
  of the same tools, under the same limits, print the same digest for
  the same document. jig's revisions move only with what a party decides
  about (jig's `client/domain/contract.md`, 5.2), so they cannot say
  that a document changed; the digest can, and needs nothing of jig. It
  is not the mark the engine keeps per watch (jig's
  `client/domain/contract.md`, 7.2), which no tool prints.
- **Which reads print one:** a read whose arguments are a watch's
  subject, with every wait's answer: `me`; `inbox` and `tasks` with the
  arguments a wait takes (above); `task N`; `run N`; `accounts`;
  `forges`.
  `task N --tree --run` prints one for each subject, `task N`, `tree N`
  and `run N`, each over its own part of the result. `--activity`'s
  lines, `chat`, `activity` and `turn` are pages, not subjects, and
  print none.
- **`--since DIGEST`:** answered at the first delivery whose digest
  differs; without it, at the first change after the watch's first
  delivery.
- **`--until CONDITION`:** answered at the first delivery that meets it,
  the first delivery included; with `--since`, at the first delivery
  whose digest differs and that meets it. Conditions are named facts
  over the document:

  | Subject | Conditions |
  |---|---|
  | `me` | `waiting` (something waits for the party) |
  | `inbox` | `waiting`; `updates` (an unread update) |
  | `task N` | `ended`; `waiting-on-me`; `held`; `replied` (a chat's run done for now); `phase:<phase>` |
  | `tree N` | `ended` |
  | `run N` | `ended` |
  | `accounts` | `usable` (an account the party's work may use has a source usable on a host); `settled` (no sign-in or check the party asked is in progress) |
  | `forges` | `settled` (no forge setting up or connecting: no set up, connect or install the deployment asked is in progress); `connected:F` (forge F is connected); `made:F` (forge F's first repository is made, `../protocol/setup.md`, 6.7) |

- **`replied`** holds while a chat's status (jig's
  `client/domain/client.md`, section 19, its facts) is parked, waiting
  for you, waiting for them, held, closing or ended. A chat wakes on its
  person's words (jig's `domain/tasks.md`, 8.3), so a wait begun once
  `chat` or `message` has answered is not met by the reply before them:
  `message 41 TEXT --key k`, then `wait task 41 --until replied`.
- **`waiting-on-me`** covers task N's tree: what a task waits on is
  computed over its tree (jig's `domain/tasks.md`, section 3), so a
  question or a proposal below for the party meets it on `task N`, and
  `tree N` needs no condition of its own.
- **`phase:<phase>`** names the client's phase in lower case, hyphenated:
  `waiting`, `idle`, `due`, `preparing`, `claimed`, `running`,
  `backing-off`, `active`, `closing`, `held`. The ended phases, `ended`,
  `done`, `failed` and `cancelled`, are never conditions: the engine
  ends a task's watch as the task ends (jig's
  `client/domain/contract.md`, 7.2), so a delivery may never show one.
  `--until phase:done` and its kin are usage errors, exit 1, naming
  `--until ended`.
- **`settled`** follows a sign-in or a check, or a forge's connect or
  install, to its end. A sign-in is in
  progress from its request's answer until it ends: made, renewed,
  refused, or past the sign-in's deadline (skein's `oauth.md`, section
  3); a check, until each host it asks has reported (jig's
  `domain/accounts.md`, 3.4). So a wait begun once `connect` or `verify`
  has answered is not met before them: `connect 1 --key chatgpt`, then
  `wait accounts --until settled`, then the account's state in its
  answer. On `forges`, a connect or an install is in progress from its
  request's answer until its run ends: connected, refused, lacking, or
  failed, or its link's state past its life (`setup.state_life_ms`,
  `../protocol/configuration.md`); `forge-connect 1 github --key gh`,
  then `wait forges --until settled`, then the forge's row in its
  answer. A set up is in progress from its request's answer, setting
  up, until the forge's row leaves setting up and connecting: set up
  failed, connected, stopped, or not connected; `forge-setup 3`, then
  `wait forges --until settled`. `connected:F` is met by a delivery where forge F is
  connected, whoever connected it.
- **`usable`** counts the party's own accounts, those whose uses name
  the party, and the deployment's (jig's `domain/accounts.md`, 4.3), on
  any host: the box's harness waits on it once it has added the
  deployment's key (section 10).
- **`ended`** is met by a delivery that shows the subject ended, or by
  its watch's end because it ended. For `task N`, completion is checked
  by the task's number and terminal phase as below; its answer says
  how it ended.
- **A task wait whose condition a terminal task can hold watches first.**
  These conditions are `ended` and `replied`; terminal `phase:<phase>`
  conditions are usage errors as above. A watch on a subject
  that has ended is refused `Unknown` (jig's
  `client/domain/contract.md`, sections 6 and 7.2). On that opening
  refusal or `SubjectEnded`, the tool reads N through the existing
  authorized task read. On both paths, a `Historical` record or a
  live-form task establishes completion only when its returned number
  is N and its phase is terminal. `Unknown` alone never means success.
  A nonterminal or wrong-number result fails visibly; it cannot succeed or
  leave the wait hanging. A valid terminal uses the ordinary result
  renderer and meets the wait when its condition holds on that task;
  otherwise it answers not met, `ended`, exit 3, with the final document
  and its digest. Unknown, hidden or forgotten history keeps status 2;
  Busy, sign-out and read failures keep their ordinary statuses (6.3).
  The read follows the watch, so the engine's retained-load durability
  barrier leaves no gap at live retirement. Conditions no terminal task
  holds (`held`, `waiting-on-me` and live phases) take no terminal read:
  `SubjectEnded` answers not met with the last document and its digest,
  and an opening `Unknown` keeps status 2. Run/tree waits retain their
  watch-based completion; there is no additional retry or timer policy.
- **`--timeout`:** 300 seconds by default, at most 3600; answered with
  the current document, its digest, and that the condition is not met
  (6.3).
- **A watch that ends under a wait** (jig's `client/domain/contract.md`,
  7.2) answers it: met where the condition is `ended` and the subject
  ended. For `task N`, `ended` and `replied` first require the matching
  terminal read above and answer according to the condition on that task;
  refused where the party may no longer see the subject; failed
  where the sign-in ended; otherwise, not met, with the last document
  and its digest (6.3). An engine that stops ends no watch: jig's
  client opens again a watch the engine let go as it stopped (jig's
  `client/domain/client.md`, 8.1), so the wait goes on while the
  engine comes back, and fails as a link down past the link's deadline
  (6.3, section 12).
- **Level-triggered, as the watch is** (jig's
  `client/domain/contract.md`, 7.2): deliveries between are coalesced,
  and the answer is the document as it is when answered. A change before
  the wait began is in its first delivery, which `--since` compares.
- **The loop:** `wait me --until waiting`; `inbox`; act; again. An agent
  following one chat: `wait task N --until waiting-on-me`, or
  `wait task N --until ended`.
- **On a box,** the forge's webhooks make the engine's documents move
  soon after the forge does; with polling alone, the same waits answer
  later (`../protocol/setup.md`, section 10).

### 5.5 Areas

The table grows in the order the QA loop reaches temper's surface. Each
area lands with its stories (section 11).

1. **A chat to a landed change** (the first QA loop, the milestone's
   story, `../README.md`, section 2): the host's tools of 5.6; every read
   of 5.2 other than the general proposal read of area 2; `wait`; every act of 5.3.
2. **Goals and plans:** `goal --project P [--priority I] [--budget A |
   --as-allowed] [--deadline T] TEXT` (`shapes::set_goal`, keyed),
   `proposal P/K`, `amend N [--rev R] CHANGE... REASON`, `withdraw P/K`,
   `seen`.

   The general `proposal tN/K|pN/K` reads every proposal kind. For settings it
   prints the engine's current/origin/would-be review and its newest revision;
   ended and withdrawn records retain their state. It uses the same read
   vocabulary independently of the first area's settings proposal act.

   - **`amend`'s changes,** one or more, each a control of the task,
     the rest kept as the task's document shows them (jig's
     `domain/tasks.md`, section 7), drafted by the client domain's
     amendment editor (`README.md`, 3.7):

     | Change | Sets |
     |---|---|
     | `--priority P` \| `--untracked` | its tracking: tracked at priority P, or untracked |
     | `--deadline T` \| `--no-deadline` | its authority's deadline, T as `2026-10-24T17:00Z` (UTC, to the minute), the rest of its authority restated as shown |
     | `--instructions FILE` | its instructions, whole: FILE's bytes, less a final newline; empty is none, said explicitly |
     | `--spec FILE` | its spec's words, whole; its parameters and inputs kept |
     | `--dependency M`... \| `--no-dependencies` | its dependencies, whole: those named, in order; what it names anew is added, what it leaves out removed |
     | `--model M [--effort E]` | its main model and effort (area 5) |

     FILE is a path, or `-` for stdin, used once, as TEXT is.
   - **`--rev R` is the spec's revision** that `task N` prints, as the
     task's offered `amend` act carries it: `task N`'s acts print it as
     `amend 41 --rev 8 CHANGE... REASON --key NAME`. A change of a
     tracked task's priority alone names none (jig's `domain/tasks.md`, section 7;
     `../domain/agent.md`, 4.3); every other amendment does, and one
     without it exits 1, `temper ctl: amend needs --rev R`, nothing
     sent. The tool reads the task's document, drafts the change on
     it, and sends nothing when the document's revision is no longer R:
     `temper ctl: task 41 moved: its spec revision is 9, not 7; read
     again: task 41`, exit 1.
   - **What the document must hold whole:** a deadline restates the
     task's authority, a spec's words its spec. Where the document cut
     it to the client's bounds, the tool sends nothing and exits 1,
     `temper ctl: task 41's authority is cut in this read; amend its
     deadline from a client with wider bounds` (or `spec`, `words`).
   - **Beyond the party's bounds,** an amendment follows 5.3, compared
     by jig's admission for the task's offered `amend` act: within,
     sent; past what the party holds but proposable, a deadline later
     than its funder allows among them, sent as `Propose`, and
     `proposed: t41/3, to …`; past the bounds, refused, nothing sent.
     Each change is proposable (jig's `domain/tasks.md`, 4 and 7); what
     fails another check than authority (a dependency out of reach, a
     circle, a goal past the project's goals) is the engine's refusal,
     exit 2.
   - **The same flag twice, or both of a pair,** exits 1 as usage;
     `--dependency` alone may repeat.
3. **Several parties:** `service --project P --role R --out FILE`
   (jig's `MakeService`, keyed; section 9), `sign-in --service FILE`,
   `project P`, `deployment`, `move N REASON`, a person task's `take`,
   `hand-back` and `answer N --rev R`, and the project's own operations,
   `policy`, `role`, `pool` (jig's `domain/people.md`, 5.2).
4. **jig's plumbing whole:** `create`, `propose`, `compound --project P
   FILE`, a batch, a proposal or a compound request as JSON (keyed).
5. **LLM accounts** (jig's `domain/accounts.md`, section 5): the party's
   accounts and, for an owner, the deployment's, as temper's accounts
   page shows them (`README.md`, 3.5). Each act is one of jig's
   requests for accounts (jig's `client/domain/contract.md`, 3.2).

   | Tool | What it does | jig's | K |
   |---|---|---|---|
   | `accounts` | the party's accounts, then, for an owner, the deployment's: each with its number, LLM provider, display name and holder; its sources in order, each with its kind, its name for a program's, and its state, per host for a program's, with the reset where the provider gave one; the runs on it and the tasks waiting for it; the sign-ins and checks in progress; its acts | the `accounts` watch (5.4) | |
   | `models [--project P]` | the models the party may use, by LLM provider: each with its efforts, what it is good at and its prices, and "needs an account" beside a provider no account of the party's work serves | `me`'s models, with the `accounts` watch's states | |
   | `connect PROVIDER` | starts a sign-in with an LLM provider: prints the address to open in a browser on the engine's machine, and the call that follows it, `wait accounts --until settled`; exits 0 once the request is durable | sign in | yes |
   | `disconnect ACCOUNT [--source S]` | signs a source out: a signed-in source's record revoked at the provider, a program's login signed out on each host; the source removed. Unconfirmed, the answer says to disconnect temper in the provider's settings, or names the hosts that did not sign out | sign out | |
   | `add-key PROVIDER FILE [--account A] [--deployment]` | adds a key read from FILE: to account A, or as a new key-only account; the deployment's with `--deployment`, by an owner | add a key | yes |
   | `replace-key ACCOUNT FILE` | replaces the account's key with the one read from FILE | replace a key | |
   | `forget ACCOUNT [--source S]` | removes a source, or the account with every source, each signed out as `disconnect` does | remove | |
   | `declare PROVIDER ADAPTER NAME [--account A]` | declares a program's login under the source's NAME (1 to 32 of `a` to `z`, `0` to `9` and `-`): a new account, or a source of account A; the answer names the command to run on each host, `temper program-login NAME` (`../protocol/credentials.md`, section 3) | declare a program's login | yes |
   | `verify ACCOUNT [--source S]` | has each host check a program source's login now; answered once asked, then `wait accounts --until settled` | check | |
   | `rank ACCOUNT POSITION [--source S]` | the account's place among its holder's accounts of its LLM provider, from 1; with `--source`, the source's place in the account's order | order | |
   | `retry ACCOUNT [--source S]` | resumes a limited source, so that the runs it holds start again | resume | |
   | `export ACCOUNT FILE` | moves a signed-in account out, by its holder: FILE made new and private before anything is sent; the signed-in source withdrawn, so that no run takes it and no refresh follows; its record written to FILE and synced; then *written*, and the engine erases it. Prints `moved out: account N to FILE`. FILE already there, exit 1, nothing sent. Refused, exit 2, FILE removed: "not signed in" (no signed-in source: a key is added again, a program's login is per host), "moving", "cancelled" (*written* past the move's deadline). A record not written, or lost on its way, is not erased: exit 1, FILE removed, and the engine cancels the move at its deadline | move out, then *written* | |
   | `import FILE` | moves a record in from FILE, read as a key's file is: taken into the secrets' store, checked as this application's and its subject held by no other holder, the account made or the one of its subject renewed; prints `moved in: account N`, `, renewed` after it for a renewal, then removes FILE. FILE refused (not private, a link, empty, past the bound), exit 1, nothing sent. Refused, exit 2, FILE kept: "not ours" (another application's or LLM provider's record), "held" (its subject held by another holder), "moving" (its subject's account moving out here) | move in | yes |

   - **The names are plain verbs, and reads nouns,** as the other
     tools' are. Eight are not jig's words: `sign-in` and `sign-out` are
     the host's (5.6), so a sign-in with a provider is `connect` and its
     end `disconnect`; `forget`, `rank`, `verify` and `retry` are
     jig's remove, order, check and resume; `export` and `import`, its
     move out and in.
   - **Offers and readiness are supplied facts.** `me` prints `providers`,
     `providers_cut` and `key_bytes`. `accounts` prints the party's
     `readiness` and `readiness_cut`; for an owner its `deployment_facts`
     holds the deployment's offers and readiness beside the existing
     `deployment` account rows. Nested loss contributes to the document's
     `cut`. Each readiness list has its own window, preserving both supplied
     observations. These fields enter the same digest for reads and waits.
   - **One result per call.** `connect` prints the address and returns;
     the sign-in goes on at the engine, and `wait accounts --until
     settled` follows it (5.4). It never opens a browser, and the
     provider's redirect reaches the engine's loopback, so the browser
     that approves runs on the engine's machine
     (`../protocol/credentials.md`, section 3). `verify` alike: asked, then
     waited for.
   - **A key is read from FILE,** never an argument and never printed:
     a private file, the user's own with no group or other bits and not
     reached through a link, as the engine reads a secret's file
     (`../protocol/configuration.md`, section 6); its bytes, less a final
     newline, within the bound of section 12. The tool reads it, sends
     it once, and keeps no copy (section 7).
   - **`PROVIDER` is an LLM provider's number,** as `models` and
     `accounts` print it (jig's `protocol/configuration.md`, section 3);
     `ADAPTER` is the program's name in smith's agent configuration,
     `claude_code` (smith's `protocol/agent.md`, section 4).
   - **`S` names one of the account's sources** by its kind, `sign_in`,
     `key` or `program`, as `accounts` prints it (section 15).
   - **A program's login is never the tools'.** `declare` makes the
     source; the person, or the harness, runs `temper program-login` on
     each host, outside `temper ctl`, since it hands the terminal to the
     program's own sign-in (`../protocol/credentials.md`, section 3).
   - **The task's model** is chosen with this area: `chat` and area 2's
     `goal` take `--model M [--effort E]`, the task's LLM choice, the
     charter's default without them (jig's `domain/tasks.md`, section
     3), and area 2's `amend` changes it. The bracket applies only to a model
     with no declared efforts: when a model declares efforts, `--effort E` is
     required, naming one of them. An amendment replaces the main choice whole;
     its effort is the caller's, never inherited from the task's old main.
   - **An account's uses** have no tool: each takes jig's default (jig's
     `domain/accounts.md`, 4.2) until the QA loop asks for one (section
     15).
   - **An account moved,** `export` and `import`, is jig's move (jig's
     `domain/accounts.md`, section 6) through `jig-client`'s `move_out`,
     `written` and `move_in` (jig's `client/domain/client.md`, section
     6). The QA loop on real backends needs it first: each run's fresh
     deployment takes the one signed-in account in and gives it back
     (`../draft/qa-live.md`, 4.3), as a host with no browser takes a
     record signed in on a computer with one. The record is never
     printed, never an argument, and in no request or answer kept
     (section 7). `move` and `take` are area 3's names.
   - **The file's form is jig's,** opaque to temper: a moved record,
     `JIGM` and its version, then the application's name, the issuer,
     the client id, the subject, the display name and skein's record
     (jig's `protocol/llm.md`, 5.6). The application's name is
     `temper`, temper's root's constant, the same in every deployment
     and never configured, so a file moves between any two temper
     deployments whose LLM provider has the same registration (issuer
     and client id, as ChatGPT's is: `../protocol/setup.md`, 8.1), and
     is refused "not ours" by another application built on jig.
   - **The file's versions** (`../draft/upgrades.md`, 4.2 to 4.4): it
     carries two families, each with its version, a version naming one
     layout: jig's moved form (`JIGM`, form 1) and, inside it, skein's
     OAuth record (its `RECORD_VERSION`). It holds one value, not a
     store, so no store's levels; the nesting fixes a patch's order,
     the form's patches first, then skein's on the record inside it.
     `export` writes the form the engine's jig makes. A newer `import`
     reads every older form and patches it, the form's patches and then
     the record's, before the engine's checks (section 14). An older
     `import` refuses a newer form in its own words before anything is
     sent (below), since the older one is whatever is deployed: the
     words must be in form 1's `import`.
   - **`export`, step by step,** in the shell
     (`temper-client-shell-cli`), the tools' root deciding each step:
     1. FILE is made new before anything is sent: an exclusive create,
        so that no name already there, a link among them, is opened,
        with no group or other bits (0600). FILE there: `temper ctl:
        FILE exists`, exit 1.
     2. `move_out` is sent once, under a name the table draws and never
        keeps (section 7).
     3. Answered *moving out*, the record is written to FILE whole,
        FILE synced and then its directory, and only then is `written`
        sent with the move's number, an ordinary request, kept in
        DIRECTORY until answered (8.2). Answered done, the tool prints
        `moved out: account N to FILE`, FILE as given, and exits 0:
        FILE is the account's only holder.
     4. Until `written` is sent, every failure removes FILE, which
        then holds no record the engine has let go: a refusal ("not
        signed in", "moving"), exit 2; the record not written (a write
        or a sync failed), `temper ctl: FILE not written: WHY; the
        engine keeps the account and cancels the move at its
        deadline`, exit 1; the answer lost (jig-client's *lost*: the
        exchange ended with no whole answer, or the answer came with no
        record), `temper ctl: the record of the move was lost on its
        way; the engine keeps the account and cancels the move at its
        deadline; export again after it`, exit 1; *unsent*, as every
        call's, exit 1; a signal, 128 plus its number.
     5. `written` refused "cancelled" (the move's deadline passed, or
        the engine restarted, and its record was refreshed once at the
        engine, so FILE's is dead): FILE is removed, exit 2.
     6. Once `written` is sent and its answer has not come (the link
        down past its deadline, a signal), FILE is kept: it is the only
        holder if the engine took `written`, and a dead record if the
        move was cancelled. The call exits as 6.3 says, and the next
        call sends the kept `written` (section 15).
   - **`import`, step by step:** FILE is read as a key's file is
     (above), its bytes whole, nothing trimmed, within the bound of
     section 12; refused, exit 1, nothing sent, the line naming FILE's
     fault and never its bytes. Its magic and form version are read
     against the form this temper's jig reads (`jig-protocol-accounts`'
     `moved::VERSION`): `JIGM` and a newer form, `temper ctl: FILE holds
     a moved record of form vN; this temper reads up to vM: upgrade
     temper`, exit 1, nothing sent, FILE kept. Any other start is sent,
     and the engine refuses it "not ours", as it does a form newer than
     the engine's jig reads when ctl's and the engine's jig differ
     (section 15). `move_in` carries the bytes once under
     `--key NAME`, never kept (section 7). Answered *moved in*, the tool
     prints the account, then removes FILE, whose record is dead once
     the engine refreshes it; a removal that fails exits 1, `temper
     ctl: FILE not removed: WHY; its record is spent`, after the
     account's line. Every other end keeps FILE, the only holder of the
     record: a refusal, exit 2; *unsent*, *lost* or the link down,
     exit 1, and the agent asks again with the same `--key` and FILE,
     answered with the first answer within the engine's retention.
6. **Forges** (`forge.md`, 5.1, 5.4 and 5.5): each configured forge as
   temper's `Forges` page shows it, its connects, and the adoption
   form's picker. Each act is one of the FORGE requests (`README.md`,
   section 8), made through `jig-client` as the forge family's bytes,
   as `adopt` is.

   | Tool | What it does | temper's | K |
   |---|---|---|---|
   | `forges` | each configured forge by its number, kind and `web` address: its connection, "connected as LOGIN (ID)", connecting and what it waits for, stopped and why, not set up, setting up and at which step, set up failed and why, or not connected and why; how its last connect ended, a GitHub App's lacking permissions and events each named; its mode, and "admin's token kept" for managed and owned; on GitHub, the App's slug and its installations, each with its id, its account's login, all or selected repositories, suspended, or stray, "uninstalled at the next reconcile"; a job's log on a host it does not read, with how to add it; its load; its acts | the forge's deployment part, read and watched (`forge.md`, section 9) | |
   | `forge-connect F github` | starts GitHub's manifest flow, the App made under the person's own account and installed on any account GitHub offers them: prints the link to open in a browser, the manifest's page on the clients' listener, and the call that follows it, `wait forges --until settled`; exits 0 once the request is durable | `Connect { forge, how: GitHubManifest { back: none } }` | yes |
   | `forge-connect F github-app ID FILE` | connects an App of the person's own: ID its id, FILE its private key, PEM as GitHub gives it; no webhook secret, since temper points the App's hook at `https://SLUG.invalid/`; then `wait forges --until settled` | `Connect { forge, how: GitHubApp { id, key } }` | yes |
   | `forge-connect F forgejo FILE` (not offered) | connects a Forgejo through its bot's tokens, FILE a tokens file (below) naming `api`, `git` and, where the person made hooks to temper's listener, `webhook_secret`; then `wait forges --until settled` | `Connect { forge, how: ForgejoTokens { api, git, webhook_secret? } }` | yes |
   | `forge-connect F forgejo-admin FILE [--first ORG/NAME [--project P] [--label L]]` | connects a Forgejo as its site admin, FILE the admin's token; the string-managed setup entry is required; Reconcile makes or finds its marked bot, never TakeOver; with `--first`, makes ORG/NAME with the starter workflow, project P a number or name as for `forge-setup`, default a new project named NAME, runner label L default `docker`; the token is kept only after Reconcile and Create are Done, and every end erases the carried value; without `--first`, prints `connecting` and `wait forges --until connected:F`; with `--first`, prints `connecting`, waits for Made and Connected, then shares `forge-setup`'s MakeProject and adoption continuation, as P's home unless it has one, with key `first-repository-F`; prints the project number and adoption (JSON: `project` and `outcome`) and exits 0; a stop or failed Create exits 1 naming the reason or step; an ORG equal to the bot's login, case-folded, is refused before any write and exits 2, `first_is_bot`, "the organization is the bot's login: name another organization" | `Connect { forge, how: ForgejoAdmin { first? } }`, token carried as AdminToken | yes |
   | `forge-install F` | prints a fresh link to install temper's App on another account, GitHub alone; the installation shows on `forges` once the person's browser comes back | `Install { forge, back: none }` | |
   | `forge-replace F FILE` | replaces what a person pasted: on GitHub, FILE the App's new key; on a connected Forgejo, a tokens file naming `api`, `git` or both; it must answer as the forge's user, and the old is erased once the new is kept | `Replace { forge, value }` | |
   | `forge-forget F` | erases a managed forge's admin's token; the forge is connected, its bot and tokens kept | `Forget { forge }` | |
   | `forge-setup F [--org O] [--repository R] [--project P]` | sets up an owned Forgejo here, under the box, with its first repository: the box fetches its binaries, prepares it and starts Forgejo and its runner, then temper takes its handover, makes `O/R` (default `work/hello`) with the starter workflow, and connects (`../protocol/setup.md`, 6.5 and 6.7); prints `setting up`, waits as `wait forges --until made:F` does, then adopts `O/R` into project P, a project's number or name, a name no project holds made a new project first by jig's `MakeProject`, and with no `--project` a new project named R (`../draft/first-run.md`, 2.4), as `adopt --project P --forge F --owner O --name R --landing main` would, as P's home unless P already has one, under the same sign-in and with the key `first-repository-F`, so that the call run again adopts nothing twice; prints the adoption and exits 0. Refused, exit 2: "not offered" (the forge is not an owned one whose setup says `"start": "asked"`, or it is set up, setting up or connecting), "busy" (the box is setting up another forge), "a new project" (a new one, by no `--project` or a P no project holds, waits on jig's projects at runtime), "invalid" (O or R not a name Forgejo takes, or O, R or P's name empty or too long), or, without the box, "not under the box", with the commands to run (`../client/forge.md`, 5.4). Set up failed, or its first repository failed, exit 1, naming the step | `SetUp { forge, org, repository, project }`; then `Adopt` | |
   | `demo-start` | starts the playground, the demo (`../protocol/setup.md`, 8.4): the box starts its fakes, and the engine adds the demo model's account and adopts its repository into its project, in the caller's name; waits for the answer, then prints `running` and exits 0, the same when it already runs. Refused, exit 2: "not offered" (no demo, or the demo ended). Start failed, exit 1, naming why: the fakes did not start (the box's why), the account refused, the adoption refused, or the forge not connected in time; run again, it makes what is not done | `StartDemo` | |
   | `repositories F [--installation I] [--page N]` | the picker's page N, from 1 (`README.md`, 4.4): a line per row, `OWNER/NAME`, `private` where it is, and `R/U`, its repository id and owner id for `adopt --ids`; then the call that reads page N plus one while more follow, or that none do. `--installation I` is an accepted installation's id, as `forges` prints it, GitHub alone | `Reachable { forge, installation?, page }` | |

   - **`forges`' text,** one line per forge, in number order: `forge N
     · KIND · MODE · STATE`, the state in the row's words
     (`../client/forge.md`, 5.4), such as `forge 3 · Forgejo · owned ·
     Setting up: preparing`, then, when it has one, its first
     repository in the row's words, such as `· work/hello made, in
     Default`; then the digest (5.4). Its JSON is the
     rows' fields, the state by its name with its step, its why, or its
     user.
   - **The names are the forge's and the page's.** A `forge-` prefix
     keeps them apart from area 5's `connect`, `forget` and
     `replace-key`, which are an LLM account's; `forges` and
     `repositories` are reads, nouns as the others are.
   - **Who may:** a party whose role holds the deployment's connectors;
     in standalone, the deployment's owner (`README.md`, section 8). Any
     other call of this area is refused by the engine, exit 2, in its
     words, before a forge is called; `adopt` without `--ids` stays the
     way of a party that may adopt and not read the picker.
   - **One result per call.** A connect or an install prints the link
     as text and returns; a pasted connect returns once the request is
     durable. The run goes on at the engine, and `wait forges --until
     settled` follows it (5.4); its end, connected or why not, is the
     forge's row in `forges`, never the call's status. No tool opens a
     link: the person opens it in a browser where the clients' listener
     reaches (`forge.md`, 9.1).
   - **A pasted value is read from FILE,** never an argument, never
     typed and never printed: a private file, as area 5's key is (the
     user's own, no group or other bits, not reached through a link),
     within the bound of section 12. Group or other permissions are refused
     before reading, naming the file and its mode in octal. An admin's
     token is item 1, with six bytes of framing within 4096 bytes; it
     is never restored as an RSA key or replayed from a carried request.
     `--project` and `--label` require `--first`; ORG and NAME are
     nonempty within the wire's bounds, and L is 1 to 64 letters,
     digits, dots, dashes or underscores. A key's file and an admin's
     token's are their bytes, less a final newline. A tokens file is
     one item a line, `NAME VALUE`, NAME one of `api`, `git` and
     `webhook_secret`, each at most once, VALUE its bytes to the line's
     end, and no other line; a file that breaks it is a usage error,
     exit 1, nothing sent, the line's number named and never its value.
     The tool makes temper's bundle of the items
     (`../protocol/credentials.md`, pasted values), sends it once and
     keeps no copy (section 7).
   - **No pasted value is echoed:** not in a result, a `kept:` line,
     `--json`, an error or the help. `forges` says that a value was
     given, and what the forge answered of it, never the value; an act
     it offers that takes one prints its call with `FILE`, the agent's
     own path to give.
   - **The refusals** are the engine's, exit 2, in `forge.md`, 5.5's
     words: no such forge; the forge's kind or mode does not take the
     act (an install on Forgejo, a forget on a forge not managed, a
     replacement on a managed or owned forge, a connect of an owned
     one); busy, since a forge runs one writing plan, a connect, an
     install or a reconcile, and one picker's read beside it, a second
     of either kind refused, as is one past `setup.runs`, which counts
     both; and, for `repositories`, the forge's reason (not connected,
     not installed, unreachable) and, on a managed or owned Forgejo, the
     admin's token's, missing or not a site admin's (skein's
     `forge-setup.md`, 4.3 and 4.8).
   - **A page** is at most `picker.page` rows
     (`../protocol/configuration.md`, its limits), clipped to the
     forge's own page cap, and says more follow when it is full; so a
     page past the last, the one after a full last page among them,
     answers no rows and no more, exit 0.
   - **"Not listed"** is `adopt` by `--owner` and `--name` without
     `--ids`; a repository temper cannot reach is refused, exit 2, with
     what would make it reachable, the installation's page among it
     (`README.md`, 4.7; `forge.md`, 5.5).

- **Areas 1, 5 and 6 are designed here in full.** Areas 2 to 4 name their
  tools and jig's operations; each area's arguments are checked against
  jig's operations as they stand when the QA loop reaches it, by a
  design pass (skein's `development.md`, section 4), and land with it.
- **The QA loop may ask for a tool no area names.** It goes to the
  designer as a design question, and joins the table as a row of an
  area.

### 5.6 The host's tools

| Tool | What it does |
|---|---|
| `sign-in FILE\|-` | signs in by the link `temper serve` printed, read from FILE, or from stdin for `-`; writes `token` and `address` (8.1) |
| `sign-out` | ends the sign-in and leaves `token` empty, zero bytes, read as signed out (8.1) |
| `pending` | lists what DIRECTORY's store keeps: each request's operation, its party and when it was first sent, and how many records these tools cannot read |
| `help [TOOL]` | the guide, then the tools (11.5); with TOOL, its record of the table |

- **`pending` reads the store's file only:** it takes no lock, starts
  no client and sends nothing (8.3).
- **A `pending` row** is a line each: the request's operation, as a
  `kept:` line names it (6.1); its party, `party N`; and when it was
  first sent, `first sent HH:MM UTC`, the documents' words for a wall
  time (skein's `lib.md`, section 12), or `not sent`. No name and no
  key, as a `kept:` line prints none. Under `--json`: `pending`, rows
  of `operation`, `party` and `first_sent`, a wall time as jig's
  integer, or `null`; and `unread`.
- **A record these tools cannot read,** of a store version, or of a
  family's owner or version, that they do not declare, is counted, never
  listed: jig's client reads nothing of such a record, drops it at start
  and says how many (jig's `protocol/client.md`, section 10), and
  `pending` says what a call's client will. Its line, `N kept records
  these tools cannot read; a call that sends drops them`, follows the
  rows, and the call exits 0. A file that is not a small store's at all
  exits 1, "DIRECTORY's store cannot be read".
- **A TEXT of `-`** is read from stdin, to its end. A call takes at
  most one `-`, and none with `--call -`; otherwise it is a usage
  error.

## 6. What a result looks like

### 6.1 Text

A new owner proposal outcome prints the original returned role in the request's
project. A restored kept line prints its identity alone; read the proposal for
its present recipient. Replaying the same key retains the original outcome;
`propose-settings` forwards an optional reason, and `decide --running` forwards
only an explicitly supplied choice. The engine keeps its default when omitted.

The default: plain UTF-8 lines, no colour, no wrapping. A document
prints its header line, its fields, its acts and, for a watch's
subject, its digest (5.4). A goal waiting on the agent:

```
task 41 · goal · Running · shop · priority 2
asked: Add a /health endpoint that reports the store's state
requester: you · spent 1.20 of 5.00 · deadline 2026-10-24 17:00 UTC
instructions: Answer on port 8080 unless the store's file names another one, and keep the /hea… (412 bytes; --json prints them whole)
waiting on: you: question 3 of task 44, "Which port should it listen on?"
plan: 4 live (2 running, 1 waiting, 1 held below)
acts:
  answer 44 3 TEXT
  stop 41
  cancel 41 REASON
digest: task 7c1e09a2b44f0d13
```

- **The header** is the task's number, its kind, its phase in jig's
  words, its project and its tracking. Its kind is `chat` for a
  conversation, `goal` for a goal (the task its own goal), and
  otherwise its result contract's kind: `approval`, `report`, `verdict`,
  or, for Change, `change` when its connector is the forge's (1) and
  its kind is 1. Another kind on the forge's connector is `change, `
  followed by that kind's words (`README.md`, 4.1); any other connector
  is `change, connector C kind K`. These are never jig's label as a
  number, which jig gives no meaning (jig's `domain/tasks.md`, section
  3) and temper numbers its delegates by.
  Its tracking is `priority P` while it is tracked, and `untracked`
  otherwise. A change delegate, ended: `task 2 · change · Ended ·
  project 1 · untracked`.
- **The deadline** has its date and its zone: `deadline 2026-10-24
  17:00 UTC`, or `no deadline`.
- **Its instructions,** when the document carries them, on a line of
  their own: whole when they are one line of at most 80 characters;
  otherwise their first line cut at 80 characters, then `…` and how
  many bytes they hold and where to read them whole, `(N bytes; --json
  prints them whole)`. They are the task's instructions in force, its
  charter's or an amendment's, and the document does not say which
  (section 15). With none, no line.
- **Its dependencies,** when it has any, after its instructions: a
  line `dependencies:`, then one indented line per dependency, in the
  document's order: `task M`, its state as a tree's chip says it, or an
  ended one's supplied status (`Completed`, `Failed`, `Cancelled`), or
  none; then, for one added by an amendment (its `dependency_facts`,
  jig's `Dependency.added`), ` · added by WHO at 2026-10-22 05:10 UTC`,
  WHO as the requester is said (`you`, `party 3`, `task 7`,
  `deployment 1`); then, while it keeps the task (jig's `keeps`, jig's
  `domain/tasks.md`, 5.7), ` · keeps task N from ending`:

  ```
  dependencies:
    task 43 · Completed
    task 44 · Running · added by you at 2026-10-22 05:10 UTC · keeps task 41 from ending
  ```
- **A resource** in the forge's words (`README.md`, 4.1): `resource:
  branch temper/1/c2 of repository 1 on forge 1`, never its bytes.
- **A result** in temper's words, never its record: `result:`, its
  state as the card says it (`README.md`, 5.7: `Landed`, `Completed`,
  `Failed`, `Cancelled` (cancelled or withdrawn), `Ended`), then ` · ` and its
  words. A landed change's words are its merge commit, whole, as the
  engine's landing words give it (the card shows seven characters; an
  agent may need the id): `result: Landed · merge commit 3f2a9c1…`
  (forty characters); a change's words that are not an id print as they
  are. A report's words, a verdict's, an approval's and a choice's print
  as they are; a connector's its summary and its part in the forge's
  words; a failed or cancelled task's its reason, if it gave one. Words
  of more than one line go on the lines after it, each indented two
  spaces. The result's details and acts are under `--json` only. An
  ended task's document, a read of an ended task and a wait's ended
  task print it alike.
- **An activity line** is its position, then what happened. A
  milestone's, `phase changed`: the activity names only the position of
  the task's history row that says which phase, and no tool reads that
  history (section 15).
- **Acts as calls,** without the `temper ctl DIRECTORY` prefix; a keyed
  act's call ends with `--key NAME`. A proposable act says to whom; a
  refused one says why, and prints no call. The host gives the
  rendering how a call is printed.
- **An offered act with no tool** (5.3 has none for it) prints its name
  in the tools' words and ` (no tool)`, in the document's order, with no
  call and no admission: the agent learns the act is there and may hand
  it to a person, and no call names a tool that does not exist. Under
  `--json`, the document's offers print as they are.
- **jig's words and the forge's** (section 4): phases, holds, what a
  task waits on and refusals in the words a person reads.
- **Lists** print one line per row, its number first, then the cursor
  for the next page, or that there is none.
- **A tree** prints `tree:`, then its root and each row its window
  holds, a chip a line. The engine answers the tools' window within its
  own rows (jig's `client/domain/contract.md`, 7.2), so a tree larger
  than either folds the rest, and its text says so on one line after
  the rows: `cut: 3 more not shown (1 Waiting, 2 Running); read on
  with: tasks --project 1 --parent 41`, the folded tasks' count, then
  each phase that has any, in a chip's words and jig's order of phases,
  then the call that lists the root's delegates, the project's number
  from the root's document. A tree's window opens at its start only
  (jig's wire carries no cursor for it), so the rest is read as the
  root's delegates, a page at a time by `--after`, and each one's tree
  by `task M --tree`. With nothing folded, no line. Under `--json`,
  `more` and `folded` as the document carries them.
- **An activity page** cut before its first row (jig's
  `protocol/store.md`, 7.3) prints no rows and its `read on with:` call,
  never `no more`; under `--json`, `"cut":true` and the position to read
  on from.
- **An act's outcome** prints what was made or decided and its numbers:
  `done: task 41, chat`; `proposed: p3/17, to maintainers`; `decided
  first: by ada, accepted, 14:02`; for `adopt`, `README.md`, 4.7's
  words.
- **What a call sends first** (8.2, step 4) prints on stderr, a line
  each: `kept: `, the request's operation, jig's or the forge's (`stop`,
  `adopt`), then its outcome as an act's prints, which names what it
  made or decided. No name and no key: a request's record keeps the
  engine's key, drawn or a name's digest, and the client's, the
  runner's (jig's `protocol/client.md`, section 10), never the name,
  and neither key is of use to an agent.
- **A restored proposal's `kept:` line** prints its identity alone,
  `kept: chat, proposed: t3/17`, with no recipient: jig keeps none with
  the request, so the line is never stale, and the agent reads the
  proposal for its present one. A new call's outcome keeps `, to
  <recipient>`.
- **`adopt`'s `kept:` line** has two forms: `kept: adopt WORDS`, with no
  comma, for an answer in `README.md`, 4.7's words, unknown among them,
  or an answer that could not be decoded; and `kept: adopt, REASON`, as
  every other operation's, for the client's own answers: held, the
  limit, unconfirmed, not kept.
- **What it does not send** prints a line on stderr too: another
  party's kept requests, `kept: N for party P, unsent while it is not
  signed in here` (8.2); the records the client dropped at start,
  `dropped: N kept records these tools cannot read` (5.6).

### 6.2 JSON

`--json` prints the result record instead: one JSON object on stdout,
written with `skein-json`'s writer, the result's fields by their names
in the table, in snake case, numbers as jig's integers, no floats. The
JSON is the contract for scripts; the text follows temper's words, and
may change with them.

- **A task's document** prints its fields as jig's client gives them:
  its `dependencies` as links, and beside them its `dependency_facts`,
  one object per dependency, `number`, `pending`, `added` (`null`, or
  `at`, a wall in nanoseconds, and `by`, the requester as `requester`
  prints it), `keeps` and `status`.
- **`help TOOL --json`** prints the tool's record of the table.
- **`--call`'s arguments** are named as the table names them:
  `decide`'s are `proposal`, or `task` and `hold`, then `outcome` and
  `reason`.
- **The `kept:` lines,** under `--json`, are one JSON object a line on
  stderr: `operation` and `outcome`, as the act's; another party's,
  `party` and `unsent`; the dropped, `dropped`.

### 6.3 Exit status

| Status | Meaning | Prints |
|---|---|---|
| 0 | a read answered, an act done or proposed, a wait's condition met | the result |
| 1 | the call failed: usage, the directory refused, its store unreadable (5.6), or busy past the lock's wait (8.3), the engine's address not on loopback, the engine unreachable or the link down past its deadline, signed out, a request past the client's limits, a name the client holds already (`Held`, section 7), unknown past the retention, not kept (DIRECTORY's store not written); the client's own answers alike, a read past its limits among them; a wait's watch ended by the sign-in's end | one line on stderr, `temper ctl: ...` |
| 2 | refused by the engine, and only by it: a key conflict among them (section 7); a wait's subject no longer visible to the party | jig's refusal, typed in JSON, in words in text; the act's alternatives where the refusal names them (read again, propose instead) |
| 3 | a wait's timeout, or its subject's watch ended, its condition not met | the current document, or the last, and its digest |
| 128 + N | ended by signal N, `SIGINT` or `SIGTERM`: 130 or 143 (8.2) | nothing more |

- **2 is the engine's answer alone:** what it decided about the
  request, with the alternatives it names. The client's own answers
  are 1. Two of them never reach a call: unconfirmed, since the runner
  sends nothing before `me` is live (8.2, step 3), and the client's key
  conflict, since each call's keys are its own (section 4). Either is
  the tools' bug: it exits 1, says so, and the guide says to report it
  as a finding (11.5).
- **A task wait's final read** follows 5.4 on both an `Unknown`
  opening and `SubjectEnded`: status 0 requires N and a terminal phase,
  for both `Historical` and live-form records. Unknown or hidden
  history remains status 2; a nonterminal or wrong-number result fails
  visibly, never reports completion. Busy, sign-out and other read
  failures keep their ordinary statuses.
- **A call cut by a signal** ends at once and exits 128 plus the
  signal's number, never 0 (8.2).
- **A call whose link stays down** past its deadline exits 1, and what
  it sent stays in the small store, for the next call that sends (8.2).
- **An account's move** (5.5, area 5) keeps the rule: 2 is the
  engine's refusal alone, "not signed in", "not ours", "held",
  "moving" and "cancelled"; FILE's faults are the tool's own, exit 1,
  nothing sent: `export`'s FILE already there, `import`'s FILE not
  private, a link, empty, past its bound or of a newer form than it
  reads; so are a record not
  written and an answer lost or unsent, exit 1, each line saying what
  the engine does with the account.
- **Only `sign-in` creates DIRECTORY.** Three of 1's lines, word for
  word: `temper ctl: DIRECTORY does not exist`, for any other call on a
  DIRECTORY that is not there; `temper ctl: DIRECTORY is not signed
  in`, for a call that needs the token when DIRECTORY holds none; and
  `temper ctl: the engine at ADDRESS did not answer within 30 s`, the
  link down past its deadline (section 12), ADDRESS as `address` holds
  it.
- **`adopt`:** adopted exits 0, with branch protection unknown or not
  (`README.md`, 4.7, prints "Branch protection could not be read."
  then); unknown past the retention exits 1; each other outcome there,
  not found, no access and refused among them, exits 2, in its words.

### 6.4 Bounded

- **Every result has a ceiling,** in rows and bytes: a list's page, the
  activity lines `task` adds, a conversation's page, a turn's part. A
  text cut prints how much was left and the call that reads the rest.
- **`me` prints a `cut:` line** in text and a `cut` field in JSON. It is
  true for supplied document loss, omitted provider/program rows, a cut
  provider name, and any free text lost earlier in the document. The
  provider offers include their own `providers_cut`, `name.cut` and
  `programs_cut` facts; the document flag combines these losses.
- **Program identifiers and manage URLs are whole or omitted.** If one
  cannot fit the text budget, JSON prints null and text omits its bytes;
  the document's cut flag is true. A displayed prefix is never a target.
- **Long texts are paged, never whole by default:** `chat`, `activity`
  and `turn` take a cursor or an offset. An agent's context is the
  budget the defaults are chosen for.
- **The numbers** are section 12's. A path is never cut: the tools
  print it whole, within the client's bound on a path's segments
  (`jig-client-domain`'s `Limits`, `segments`); nor is an authority,
  within its bound on an authority's entries.

## 7. Requests: once, and named

- **Through jig's request table.** Every act but one that carries a
  value (below) is a keyed request: `jig-client` keeps it before sending
  it, sends it again under its key until its answer is durable, and
  answers it once (jig's `client/domain/client.md`, section 6).
- **The agent names the key** of every tool marked `K`: `--key NAME`, a
  word of 1 to 64 bytes of `a` to `z`, `0` to `9` and `-`, such as
  `health-chat`. A call without it is refused as a usage error, before
  anything is sent.
- **A named key is the engine's key:** the first 16 bytes of the SHA-256
  of `temper-tools`, a zero byte and the name. jig scopes it to the party
  (jig's `client/domain/contract.md`, 3.1). The same name asked again by
  the same party, within the engine's retention, is the same request: a
  rerun after a lost answer, an agent's own timeout or a lost context
  gets the first answer, never a second chat. The same name with another
  ask is refused by the engine as a key conflict. The name, not
  DIRECTORY, makes a request the same: a rerun from any directory signed
  in as the party is the same request, and DIRECTORY's store keeps a
  request only until it is answered (8.2). Past the engine's retention,
  which `me` prints, the name is free again, and asking under it is a
  new request.
- **A named request is built from the call and `me` alone,** never from
  the clock, so a rerun builds the same ask: a deadline a template gives
  as a span is sent as that span, which the engine makes a wall when it
  makes the task (jig's `client/domain/contract.md`, 3.2). A rerun
  after `me` changed what the request is built from (a template, a
  bound) is another ask, refused as a key conflict.
- **A name the client holds already** is answered `Held`, nothing sent
  (jig's `client/domain/client.md`, section 6). A call sends what
  DIRECTORY kept, and waits for each answer, before it sends its own
  (8.2), so it meets `Held` only past that, or for a name another
  party's parked request holds (8.2); it exits 1.
- **Names are the party's.** Agents sharing a party give their names a
  prefix of their own; the guide says so (11.5).
- **Acts that decide name the revision the agent saw;** a moved one is
  refused, and the agent reads again. A repeat of a decision is answered
  as decided first, by the party itself.
- **The other acts** (`stop`, `cancel`, `decide`, ...) take `--key`
  optionally; without it the table draws one, which the small store
  keeps until answered (8.2).
- **A request carrying a value,** `add-key`'s and `replace-key`'s (5.5,
  area 5), and area 6's `forge-connect` with a FILE and `forge-replace`,
  is sent once and never kept: `jig-client` writes no record of
  it to DIRECTORY's store and never sends it again (jig's
  `client/domain/client.md`, section 6; jig's `protocol/client.md`,
  sections 5 and 10), and the engine takes the value out as it decodes
  the request, so no committed request or answer holds it (jig's
  `domain/accounts.md`, section 5). A forge's pasted value is kept by
  jig's clients' component in the secrets' store under temper's tag, at
  a number jig gives it, before temper's core sees the request; a
  request refused, or a duplicate, has jig erase it
  (`../protocol/credentials.md`, pasted values). The tool names no
  number. The client's own answers *unsent*
  (the engine did not make it) and *lost* (it may have) exit 1 with
  nothing kept, as a call cut or a link down before its answer does, and
  `pending` lists nothing of it. The agent asks again with the same
  `--key` and the same FILE: within the engine's retention, `add-key`
  gets the first answer, never a second account, and `forge-connect`
  never a second connect. `replace-key` and `forge-replace` take
  `--key` optionally, as the other acts do, but the name drawn is not
  kept, so only a name the agent gives can be asked again; a rerun
  without one replaces the key or the tokens again.
- **An account's move** (5.5, area 5): `import`'s `move_in` is a
  request carrying a value, as `add-key`'s is, under its `--key`.
  `export`'s `move_out` is a request whose answer carries one, the
  record: sent once under a name the table draws, never kept and never
  sent again, since the record comes with the first answer alone and
  the name asked again is answered with none, *lost* (jig's
  `client/domain/contract.md`, 3.1). So `export` takes no `--key`: an
  export asked again is a new request, refused "moving" until the
  first move is cancelled at its deadline. Its `written` is an
  ordinary request under a drawn name, kept in DIRECTORY until
  answered (8.2).

## 8. The command line: `temper ctl`

### 8.1 Its shape

```
temper ctl DIRECTORY TOOL [ARGUMENT...] [--json]
temper ctl DIRECTORY --call FILE [--json]
temper ctl help [TOOL]
```

- **DIRECTORY** is the client's private directory, as for `temper open`
  (`README.md`, 6.1): its credential, `token`, its small store, `store`,
  and the engine's address, `address`, all `jig-client-shell-native`'s
  or beside them, `0o700` and refused unless the user's own. A
  DIRECTORY is `temper ctl`'s or `temper open`'s, never both's (section 15).
- **`address`** is `temper-client-shell-cli`'s: one line,
  `http://HOST:PORT`, `0o600`, written by `sign-in` and read by every
  other call.
- **`--call FILE`** reads one call as JSON, `{"tool": ..., "arguments":
  {...}}`, from FILE, or from stdin for `-`.
- **A call `--call` reads gives no argument of the type a file** (5.1),
  a path or `-`: one that does is a usage error, exit 1, once the call
  is decoded and before DIRECTORY opens. So `sign-in`, and each tool
  of areas 3 to 6 that reads or writes a FILE, is called from its
  arguments. A call opens a FILE only beneath a root its start opened
  before the loop, the directory of the FILE its arguments name, and
  `--call`'s call is decoded on the loop, from FILE or from stdin
  alike.
- **`sign-in FILE|-`** signs in by the link `temper serve` printed, read
  from FILE or stdin and never an argument (section 9), on loopback, and
  writes the token and the address the link names, as `temper open` does
  (`README.md`, section 7). It offers the token DIRECTORY holds, if any,
  beside the link: a sign-in kept leaves `token` as it was. Its
  answers, each exit 1 with one line, and none writing `token` or
  `address`:
  - *refused* (the link spent, expired or unknown): `temper ctl: the
    sign-in link was refused`; `temper serve` prints a fresh link;
  - *full* (jig's `domain/people.md`, section 3): `temper ctl: all N
    sign-ins are in use; the earliest ends at T; this link stays usable
    until U; after that, temper serve prints a fresh one when it is
    tried`, N the deployment's room, T the earliest saved expiry among
    its sign-ins and U the link's own, in universal time to the second,
    `YYYY-MM-DDTHH:MM:SSZ`. The link is not spent: the same `sign-in`
    signs in once a sign-in has ended, before U;
  - *not ready* and *busy* pass: the call sends the same link again,
    with the same token, after 100 ms, doubling to 2 s, within its
    deadline (8.2); past it, after busy, `temper ctl: temper is busy;
    this link stays usable until it expires`, and after not ready, the
    deadline's own line.

  `sign-out`
  ends the sign-in, and jig's native shell, keeping none, leaves `token`
  a zero-byte file, which a call reads as no token. Every other tool
  signs in with the token DIRECTORY holds (`SignIn { with: Held }`).
- **Loopback only,** as `temper open`, until the clients' listener
  serves TLS: a call checks that `address` is on loopback, `127.0.0.0/8`
  or `[::1]`, before it sends the token, and otherwise exits 1, "the
  engine's address must be on loopback".
- **No configuration file;** its inputs are its arguments and DIRECTORY.

### 8.2 One call, one process

The host of `jig-client`'s one-shot mode (jig's
`client/domain/client.md`, section 20), ordinary Rust on skein's loop,
as the terminal's host is:

1. parse the arguments against the table; a usage error exits 1 before
   anything is opened;
2. open DIRECTORY and check its address (8.1); for a call that sends,
   take its lock (8.3) and load the small store's records;
3. start the tools' root, sign in with the token, wait for `me`;
4. send what the store kept for `me`'s party, each answered as a line
   on stderr, `kept: ...`, a restored proposal's without its recipient
   (6.1); the step ends once each is answered and its record erased, so
   that the store keeps nothing of the party's when the call is sent;
5. run the call: a read answered, an act's answer durable, a wait's
   condition met or its timeout;
6. print the result on stdout, close the stream, exit with the call's
   status.

- **A call's stream ends with its connection.** Step 6's close ends the
  stream's connection, and so does the kernel when the process ends any
  other way. The clients' listener takes the peer's end as the call's
  end (`README.md`, section 8), and the engine ends the stream then, not
  at its next heartbeat. So a call holds one of the engine's streams,
  which its profile bounds (jig's `protocol/client.md`, section 13),
  only while its process runs, and calls that run one after another
  never wait on their predecessors' streams. A read or a wait sends no
  stream gone itself (8.3).
- **A termination signal,** `SIGINT` or `SIGTERM`, arrives as skein's
  `Shutdown` (skein's `io.md`, section 7), and ends the call at once:
  it sends nothing more, leaves DIRECTORY's kept requests as they are,
  prints nothing more, and exits 128 plus the signal's number, 130 or
  143. What it sent stays in the small store, the next call that sends
  sends it again under its key, and a rerun under its name gets its
  first answer (section 7).
- **A link down** past the link's deadline (section 12), at step 3, 4
  or 5, exits 1; what was sent stays in the small store, as after a
  signal.
- **Another party's kept requests,** in a DIRECTORY signed in again as
  another party, stay parked and unsent, until their party signs in
  there again or the retention passes and they are answered unknown
  (jig's `client/domain/client.md`, section 11). Step 4 does not wait
  for them; the call says how many (6.1) and goes on.
- **`pending`** lists what the small store keeps (5.6).

### 8.3 Several calls at once

An agent runs a `wait` while it acts, or several agents share a party.

- **A call that sends** takes an exclusive lock on DIRECTORY before it
  loads the small store's records (8.2, step 2), so two never write the
  small store over each other: skein's `Lock` on a root (skein's
  `io.md`, section 5), on DIRECTORY itself, since `store` is replaced by
  a rename on each write. A second tries again every 100 ms, and past
  the lock's wait (section 12) exits 1, "DIRECTORY is busy with another
  call".
- **The lock ends when the call is done with DIRECTORY,** before its
  process exits: once the call has its status and its reads and writes
  of DIRECTORY have completed, a store write in flight at a signal
  included, it closes the descriptor it locked, and the lock ends with
  that `Close` (skein's `io.md`, section 5: "The lock lasts as long as
  the root's descriptor: until its `Close`"). Its output drains and its
  process exits after, touching nothing of DIRECTORY, so the next call
  never waits on a slow reader of this one's output. A process that ends
  sooner, however it ends, ends the lock with its descriptor.
- **Which take it:** every act, `sign-in` and `sign-out`; never
  `pending`, `help`, a read or a wait.
- **A read and a wait** take no lock, and hold their client's store in
  memory, empty, never DIRECTORY's file: they write no record and send
  nothing but reads and watches. A client loaded from DIRECTORY's
  records would send the stream they name gone (jig's
  `client/domain/client.md`, section 11), ending the watches of the
  call that opened it.
- **Several parties** are several directories, each signed in as its
  own.

## 9. Who the agent is

- **Any credential.** An agent acting for a person may hold the person's
  credential or one of its own: the tools, their calls and their
  results are the same either way, and jig judges each request as the
  party whose credential it is (jig's `domain/people.md`, section 3).
- **The person's own:** the printed link's sign-in. The QA loop's first
  identity is the deployment's first owner.
- **Its own, a service's** (area 3): an owner makes a service in a
  project, with a role (`service`), and jig's clients' component answers
  its credential once; the tool writes it to the file `--out` names,
  `0o600`, refused if the file exists, and never prints it. The agent
  signs in with it (`sign-in --service FILE`) and acts within that
  role's authority and pool. QA plays several parties this way.
- **Credentials are files,** private, never arguments, never printed;
  the printed link too, which `sign-in` reads from a file or stdin.
- **The agent's LLM is not the deployment's.** The LLM the agent itself
  runs on, to drive `temper ctl`, is its harness's. The deployment's
  runs are paid by jig's LLM accounts, added from the client
  (`../protocol/credentials.md`, section 3): on a box, by the deployment's
  key the harness adds (section 10), which pays for every party's runs
  (jig's `domain/accounts.md`, 4.2). An agent acting for a person
  connects that person's own accounts with area 5's tools, as the
  person would on the accounts page.

## 10. The box

The deployment an agent drives is an owned setup run by `temper box`
(`../protocol/setup.md`, section 7), with no demo, in a scratch
directory of the QA harness's:

- **Started:** `temper box`, given the harness's three directories
  (`../protocol/configuration.md`, section 12); its stdout carries `temper
  serve`'s sign-in links, and the harness passes the first to `temper
  ctl DIRECTORY sign-in -` on stdin.
- **The deployment's LLM key:** a fresh box has no LLM account, so no
  run starts (`../protocol/credentials.md`, section 3). Once signed in, the
  harness adds one before the agent's first act: `add-key PROVIDER FILE
  --deployment --key box-llm`, from a private file of its own, then
  `wait accounts --until usable` (5.4). The key is QA's own, with limits
  of its own (`../draft/qa.md`, section 7); the agent is never handed its
  file, and the harness removes the file with the box. On the process
  tier and in the real loop, it is a key skein's fake LLM peer accepts
  (`../domain/testing.md`, 2.4 and 2.6).
- **The person on Forgejo:** the harness plays the person of
  `../protocol/setup.md`, 6.5: it signs in on Forgejo's web with
  `person_password` and changes it, and makes the repository to adopt
  in an org, which the agent adopts with `adopt` as the first act of
  area 1.
- **The forge read directly:** its API and `git` on `owned.listen`, with
  a token the harness makes for the person, as a person makes one
  (`../protocol/setup.md`, section 16), whose scopes are reads alone:
  `read:repository`, `read:issue`, `read:organization` and `read:user`.
  The agent is handed that token, never the preparation's
  `admin_token`, which the engine's setup host alone takes, once
  (`../protocol/setup.md`, sections 3 and 9). Forgejo refuses the
  agent's writes with it; the agent changes the forge only through
  temper, and the guide says so (11.5).
- **The forge through temper:** the box's Forgejo is owned, connected by
  the box's own preparation and the setup host's plans
  (`../protocol/setup.md`, owned), so it takes no connect, and the agent is never handed a value
  to paste. With area 6, the agent reads it with `forges` and the
  repositories to adopt with `repositories`, every one the site admin
  can see, and adopts one with `adopt --ids`.
- **Logs:** each step's, the engine's and the worker's among them, in
  its file under `<data>/box/logs/`, and on the box's stderr prefixed
  by its step with `--echo steps` (`../protocol/setup.md`, 7.2);
  Forgejo's in its directory. Outside the tools.
- **Thrown away** by stopping the box with a termination signal and
  removing its directory: the harness's, never a tool's.

A person's first run is the box's demo (`../protocol/setup.md`, 8.4),
its fakes dormant until the person chooses the playground; the engine
then adds the demo model's account and adopts its repository itself. A
harness does as the person does: `sign-in -` with serve's first link,
then `demo-start` (5.5, area 6), which answers once the playground is
ready.

## 11. Testing

### 11.1 Step tests

- **The table:** each tool's argv parsed into its call and back, and its
  JSON; usage errors; the help, as golden text.
- **The rendering:** each result kind in text and JSON, as golden text,
  with jig's and the forge's words; a turn by its size; a cut and its
  call.
- **Digests and conditions:** a digest moves with a document's change
  and only then; each condition over documents that meet and miss it.
- **Task waits,** in the existing tools step fixture: `Unknown` and
  `SubjectEnded` each followed by real `Historical` and live-form
  terminal records, nonterminal and wrong-number records, unknown or
  hidden history, Busy, sign-out and read failure. Only matching
  terminal records meet `ended`; each path emits exactly one result
  and releases its handles and timer. No new harness (5.4).
- **The boundary:** `src/jig/` names no temper item (section 4).
- **The bounds:** each of the tools' ceilings within the client's limit
  it reads from, and a picker's page within a result's text (section
  12).
- **DIRECTORY's records,** in `temper-client-shell-cli`'s tests: a
  record of a store version the tools do not read counted by `pending`,
  and the next call that sends saying it dropped it.
- **An account's move,** in `temper-client-shell-cli`'s tests, the
  engine's answers scripted (5.5, area 5; 6.3): `export`'s FILE already
  there, a link among them, exit 1 with nothing sent; FILE made with
  mode 0600, the record written whole and synced, then its directory,
  before `written` is sent; each of the engine's refusals, exit 2, FILE
  removed; `written` refused "cancelled", exit 2, FILE removed; a write
  that fails, and an answer lost, exit 1, FILE removed and no `written`
  sent; `written`'s answer not come, FILE kept. `import`'s FILE not
  private, a link, empty and past its bound, exit 1 with nothing sent;
  *moved in*, the account printed and FILE removed; each refusal, exit
  2, and *lost*, exit 1, FILE kept. No record's byte on stdout, stderr
  or in DIRECTORY's store.

### 11.2 `tests/client`

`temper-client-world` runs the tools' root against temper's engine root,
beside the fake person, as a scripted agent making calls (`README.md`,
section 10; `../domain/testing.md`, 6.3). Area 1's stories: a chat started, its
reply waited for, a message, a turn too large for a page read whole,
the chat closed; a repository adopted; a held task released. The
existing agent story also waits for a task whose end was observed before
`wait task N --until ended` begins, checking the matching terminal
answer (5.4). A turn too large for a page exists only where the engine's
bound on a turn passes its page bound, so the world's profile sets it
so. The world's
faults hold: a call cut by a link drop or an engine restart, asked
again under its named key, is made once. Each area adds its
stories. Area 5's, over the engine root's accounts child with each
source's mechanism scripted: a key added, then the deployment's,
`accounts` showing both with no value in DIRECTORY's store or in any
document the tools print; an `add-key` cut before its answer and asked
again with its key and file, one account made; a sign-in started, its
answer scripted made, refused and past its deadline, each meeting
`wait accounts --until settled`; a limited notice, the next task
waiting for its account and `task N` saying why, then `retry` and the
run started; `rank` and
`forget`; a program source declared, and `verify` answered as each host
reports. Area 6's, over the engine root's forge connector and the
setup's host with skein's fake forge (`../domain/testing.md`, 2.4): a
GitHub connect printing its link, never opening it, the browser's
steps driven by the world, then `wait forges --until settled` and the
forge connected; a pasted connect cut before its answer and asked
again with its key and file, one connect; an App lacking a permission
and an event, each named on the forge's row; a tokens file with a
line broken, exit 1 naming the line and never its value;
`repositories` by page number, a full last page's next and a page past
the last answering no rows, and one row adopted with `--ids`; a second
picker's read while one runs, busy; a call by a party whose role does
not hold the deployment's connectors, exit 2; `forge-forget` on a
managed Forgejo; `forge-setup` without the box, refused naming the
commands; `forge-setup` on an asked owned forge, answered `setting up`,
then `wait forges --until settled` following the routing's told states,
setting up at each step to not connected, and to set up failed with its
why; a `forge-setup` of the same forge while it sets up, not offered,
and of another asked forge while one sets up, busy; `demo-start`
answered `running`, run again answered the same, adding and adopting
nothing twice, and refused "not offered" with no demo. After each, no pasted value in DIRECTORY's store or
in any document the tools print.

### 11.3 The real loop

`tests/real-loop` hosts the tools' root over jig's native transport,
the standalone engine and worker, smith's agents on skein's fake LLM
peer's scripted turns, skein's forge peer hosting the fake forge, and
the fake git program over that forge, on one thread and one real ring
(`../domain/testing.md`, 2.4). Agents are hosted in that loop and
joined to their worker by real pipes. The store and client credentials
use a scratch directory on the real filesystem.

Its stories sign in, start a chat and wait for its reply, and satisfy one
client's wait by another client's act; each drains with everything
settled. Then a service restart retains the work. A call whose answer was
lost is made once: the tools' root is dropped once the engine holds its
request, before its answer, and a fresh one, on another DIRECTORY signed
in as the same party, asks under the same name and is answered the first
answer; the party has one chat. One signed in again as another party:
the first party's kept request stays unsent, and the call goes on. The
forge's stories adopt a repository, and clone, push and land through the
fake git program over the forge peer's git route, with CI by the fake
forge's content marker and its webhooks delivered to the engine
(`../domain/testing.md`, 2.4). Real git meets a forge in the process
tier (11.4). The forge's stories run on `skein-world`'s
`World::new_controlled`, unchecked, while the fake git program shares
the fake forge in memory: its memory there outlives the git program
that made it, which the checked world fails (skein's
`testing-strategy.md`, section 6; skein's `fake-forge.md`, section 5).
Over the git route the program shares nothing, and they run checked.
Every other story here is checked.

Every story whose agents run first adds a key the fake LLM peer accepts, `add-key` from a scratch
file, as section 10's harness does, and waits for `usable`. Area 5's
stories (`../domain/testing.md`, 2.4):

- **A sign-in:** `connect` against skein's fake issuer, the browser's
  visit and the redirect to the engine's loopback listener driven by the
  test, then `wait accounts --until settled`, the account usable, and a
  run on it whose request the peer observes with its bearer.
- **A program's login:** `declare`, with smith's fake `claude` as the
  agent's program, its login signed in in the source's directory under
  the agent's programs directory; `verify`, then `wait accounts --until
  settled`, the source usable on the worker's host, and a run on it
  through the program backend.
- **No value kept:** after each, neither DIRECTORY's store nor the
  engine's store holds a key's or a record's value in a request, an
  answer or a fact; the secrets' store alone does.

Area 6's stories reach the forge through the tools, on skein's fake
forge, whose browser pages are plain HTTP: its manifest post answers
`200` with a page holding one link, `Create GitHub App` (id `create`),
and its install page `302` (skein's `fake-forge.md`, 10.1), so the test
drives a browser's steps as plain requests, following the link as a
person clicks it, and no story calls github.com:

- **GitHub through the manifest:** `forge-connect F github`, the printed
  link followed as plain requests on the clients' listener and the
  fake's pages, then `wait forges --until settled`, and `forges`
  showing the forge connected as the App's user.
- **A repository added later,** from the picker: `repositories F`, then
  `adopt --ids` of one of its rows into a project holding another.
- **A repository granted on GitHub:** the fake grants one to an
  installation of selected repositories, and the next `repositories F`
  lists it.
- **Forgejo connected, managed and owned** from the tools:
  `forge-connect F forgejo-admin FILE --key NAME`, with and without
  `--first work/hello --project P --label docker`: with the first repository,
  waits for creation and connection, makes a missing project, adopts once
  under `first-repository-F`, and prints its project and adoption; existing
  projects by name or number, reruns, stops and failed Create are covered.
  Without `--first`, prints `wait forges --until connected:F`.
  Also an owned forge's `repositories`.
  Plain `forgejo FILE` stays unoffered.
- **No secret kept:** after each, no pasted value, key, token or secret
  in a root, a fact, a record but the secrets' store's, a request or an
  answer, nor in DIRECTORY's store.

Each call of `temper ctl` in these stories is its host,
`temper-client-shell-cli`'s, hosted as a program of the real world, one
per call, on its DIRECTORY: the lock, DIRECTORY's files and the call's
exit run on the real kernel. Two acts at once on one DIRECTORY are
serialized by the lock: skein's `Lock` is `flock`, held per open
descriptor (skein's `kernel.md`), so two programs of one process contend
as two processes do.

These are smoke, one end-to-end path per feature, at the gate
(`../domain/testing.md`, 2.4). A tool's behaviour and corner cases are
`tests/client`'s (11.2) and the lower worlds' (11.1): a sign-in refused
or past its deadline, a limit and `retry`, a record the tools do not
read. These stories reuse lower worlds' scripts and outside expectations
where applicable, and add checks of the full transport, persistence and
teardown on the real kernel; the harness checks memory ownership,
outstanding operations and release between iterations.
A real-loop failure is kept with its trace and reproduced lower down
where possible; its seed does not replay the kernel's schedule.

One story reads at `temper init`'s template's bounds, its documents'
rows below the tools' 32-row window: a goal with more delegates than
those rows, one of them ended with an ended dependency, and `task N
--tree` answering within a second, cut and saying so, and `activity N
--json` of the ended one answering its rows, newest first, and reading
on to its first; the tree's `cut:` line's call followed, page by page,
to the last delegate. Its fixture sets that bound itself, whatever the
others' fixtures raise.

### 11.4 The process tier

Settings inventory, a setting, a revision and revision history each have
one named ctl process story in text and JSON. An owner's change has one
story: read the current revision, change a charter, read its full value
and revision back, and refuse a stale request naming the newest revision.

`temper serve` and `temper ctl` as processes over skein's fake LLM peer
(`../domain/testing.md`, 2.6), on a scratch directory of their own, for
what needs the executable: argv through `main`, printed output, the OS's
exit statuses, the printed link read on stdin (`sign-in -`), the
deployment's credential (its key added from a private file as the first act,
section 10, and a key's file refused when it is not private, as a
forge's pasted value's file is), an act
cut by a termination signal, its status 128 plus the signal's number
(8.2), then asked again under its key, made once, and agent process
launch; an account moved between two deployments (below). Under `temper box`, `forge-setup` of the default document's
forge 3 and `wait forges --until settled` against the real Forgejo and
runner (`../domain/testing.md`, 2.6). One landing on the owned Forgejo proves a change against real
git and CI (`../domain/testing.md`, 2.6). The lock, DIRECTORY's files
and a call's exit are the real loop's (11.3).

**An account moved,** one story (5.5, area 5): two `temper serve`
processes, deployments A and B on directories of their own, and one
fake issuer. A person signs in to A with the LLM provider through
`connect`, the fake browser approving; `export ACCOUNT FILE` exits 0
and A's `accounts` no longer has the account; the owner of B, signed in
to B by its printed link, runs `import FILE --key NAME`, which prints
the account and exits 0, FILE gone; B, stopped and started again on its
directories, holds the account, and a grant is lent on it after its one
refresh at the issuer; the issuer refuses the refresh token FILE held.
What jig's worlds show of the move (a late *written*, a restart while
moving, the refusals' decisions) is not repeated here.

### 11.5 The agents' guide

`temper ctl help` prints a short guide, then the tools from the table:
how to sign in, the loop of 5.4, naming keys and their prefix, reading
acts, the exit statuses, reading the forge and never changing it but
through temper (section 10), and an LLM account's key read from a
private file, never typed, with a sign-in followed by `wait accounts`
(5.5, area 5), and a forge's pasted value read from a private file,
never typed, with a connect's link printed as text, never opened, and
followed by `wait forges` (5.5, area 6). The guide is text of
`temper-client-tools`; a
test fails when it names a tool the table lacks.

### 11.6 Findings into stories

Each bug an agent finds through the tools ends as a story in
the lowest tier that shows it, before its fix lands: a domain world,
`tests/client`, the simulated world, the real loop or the process tier
(`../draft/qa.md`, section 1). A story's calls are the tools' calls, so an
agent's history of `temper ctl` commands transcribes into one.

## 12. The bounds

settings FILE (or stdin):1MiB, tools.ask_bytes; item/member/value
bounds are checked before the request is sent.

Four bounds decide how much of a document a call prints:

| Bound | Whose | It decides | Past it |
|---|---|---|---|
| a page's bytes | the engine's (jig's `client/domain/contract.md`, 5.1) | whether a turn comes in a page by its bytes or by its size | the turn's line, its size and the call that reads it (5.2) |
| one read's bytes | the engine's (the read of one turn, jig's `client/domain/contract.md`, section 6) | whether `turn` is answered whole | its size, and that it is past the limit |
| one document's bytes | the client's, `document_bytes` | whether the client takes a document whole | a turn: its size, and that it is past the limit; any other document: the client's `Limit`, exit 1 |
| the tools' window | the tools' | how much of a result prints | the rest by its cursor, or `--part` and `--from` (6.4) |

- **A turn past either read bound** answers exit 0, with its size and
  "past the limit; report it": the agent reports it as a finding, and
  never works around it.
- **The tools' window is within the client's limit:** a step test fails
  when one of the tools' ceilings passes the client limit it reads from
  (11.1).

The numbers are constants of `temper-client-tools`' `limits` module. The
client's are the tools' own preset of `jig-client`'s `Limits`, not the
terminal's, whose window shows two rows:

| What | Bound |
|---|---|
| rows: a list's page, the inbox's needs and updates together, the activity lines `task` adds, a conversation's page; the client's rows per window | 32, the most jig's client wire takes in a limit of rows (its schemas' ceilings; jig's `protocol/client.md`, section 3) |
| a result's text | 32 KiB |
| a turn's part | 16 KiB |
| one document, the client's `document_bytes` | 1 MiB |
| a path's segments; an authority's entries | 9; 12, as the terminal's (`README.md`, 5.4) |
| the link's deadline: the engine unreachable, or a call's request or answer unheard | 30 s |
| the lock's wait | 60 s, tried every 100 ms |
| a key's file, less a final newline (5.5, area 5) | 4 KiB; past it, or empty, exit 1 with nothing sent; a key the engine refuses is its refusal, exit 2 |
| a moved record's file, whole (5.5, area 5) | 128 KiB, jig's bound on a keyed request's value (jig's `protocol/client.md`, section 5); past it, or empty, exit 1 with nothing sent; a file past its form's bound at the engine is its refusal, a limit, exit 2 |
| a forge's pasted value: its file, and the bundle the tool makes of it (5.5, area 6) | 4 KiB, jig's bound on a carried value; past it, or empty, exit 1 with nothing sent; a value the engine refuses, one past the deployment's `records.value_bytes` among them, is its refusal, exit 2 |
| a picker's page, `repositories` (5.5, area 6) | 100 rows, the most `picker.page` takes, printed whole: temper's own family carries them, not jig's rows; a step test fails when 100 rows at the client's bound on a field's bytes pass a result's text (11.1) |
| a wait's timeout | 300 s by default, at most 3600 (5.4) |

The client's other limits are the terminal's (`README.md`, section 2).

## 13. What it takes of jig

The tools work around nothing of jig's and skein's. Besides the
contract's reads, watches and requests, they take:

- **A named key:** `jig-client`'s `Send` under a key the application
  names, refused `Held` when the client holds it already (jig's
  `client/domain/client.md`, section 6), on skein's request table's
  given keys (skein's `lib.md`, section 12); everything else of keyed
  requests is the contract's (jig's `client/domain/contract.md`, 3.1).
- **A read of one turn,** `turn{n, attempt, turn}` (jig's
  `client/domain/contract.md`, section 6), answered from the store's
  `Scope::Turn` (jig's `protocol/store.md`, 7.3); on a page, a turn past
  the page's bound keeps its size (jig's `client/domain/contract.md`,
  5.1).
- **A deadline as a span** in a request a template gives one (jig's
  `client/domain/contract.md`, 3.2), so that a named request is built
  without the clock (section 7).
- **For area 5,** jig's accounts (jig's `domain/accounts.md`, section 5)
  and their move (its section 6, for `export` and `import`):
  their requests (jig's `client/domain/contract.md`, 3.2), each under a
  key as the others, a key's bytes held to the tools' own bound
  (section 12) and the engine's refusal past its own exit 2; the
  `accounts` watch (jig's `client/domain/contract.md`, 7.1); the models
  a party may use, in `me` (jig's `client/domain/contract.md`, 5.1),
  and the LLM choice in a task's request (jig's `domain/tasks.md`,
  section 3); and a request carrying a value, and one whose answer
  carries one (the moved record, `jig-client`'s `move_out`),
  sent once and never kept nor resent (jig's `client/domain/client.md`,
  section 6; jig's `protocol/client.md`, section 10).
- **For area 6,** temper's FORGE requests (`README.md`, section 8), each
  under a key as the others, through `jig-client` as the forge family's
  bytes; jig's request carrying a value (jig's `protocol/client.md`,
  section 5) for a pasted connect and a replacement, the value numbered
  and kept by jig's clients' component (`../protocol/credentials.md`,
  pasted values), sent once and never kept nor resent; and the forge's
  deployment part, read and watched (`forge.md`, section 9).
- **The rest as jig designs it:** its helpers (`shapes`, `facts`,
  `thread`, `words`; jig's `client/domain/client.md`, section 19), the
  documents' acts and `me`'s bounds (jig's `client/domain/contract.md`,
  5.3), the client's bound on a path's segments, a service's sign-in,
  and the one-shot mode, whose modes "differ in the host alone" (jig's
  `client/domain/client.md`, section 20).

## 14. Later

Each is named here and designed when it comes.

- **An MCP endpoint of the engine,** for agents with no shell:
  `temper-service` serves the same tools at `/mcp` on the clients'
  listener, each session a tools' root holding `jig-client` over an
  in-process transport to jig's clients' component (jig's
  `protocol/service.md`, section 10), signed in by its bearer, a wait a
  long call. It needs jig's client in the engine's process, and an
  agent other than a shell agent to drive temper. Area 1 leaves it
  wiring: a call decoded from JSON (`--call`), the rendering told how to
  print a call, a call per asker.
- **Extraction into jig,** `temper-client-tools`' module `jig` moved to
  `jig-client-tools`, when a second application wants the tools.
- **The forge's read family's tools,** beside `forges` (area 6), as the
  engine serves each of the forge's other parts (`forge.md`, section 9).
- **A service's credential reissued, or a service removed** (jig's
  `domain/people.md`, section 3): tools of area 3.
- **An account's uses** set, as a tool of area 5, when the QA loop asks
  for one (section 15).
- **An older moved form patched at `import`** (5.5, area 5, the file's
  versions): jig's form patches, then skein's record patches, before
  the engine's checks, when a form 2 exists.

## 15. Open questions

- **`temper open` and `temper ctl` on one DIRECTORY:** not supported and
  not detected (`README.md`, section 11). Default: the lock of 8.3 is
  `temper ctl`'s alone, until `temper open` takes it.
- **The one-shot host's place:** temper's, in `temper-client-shell-cli`.
  Default: it moves into `jig-client-shell-native` with the extraction
  (section 14), if jig wants one.
- **Naming a source** in area 5's `--source S`: by its kind, which
  names one source while an account holds one of each kind. Default:
  by kind, and by a program source's name once an account may hold two
  of a kind.
- **An account's uses:** no tool, each account taking jig's defaults
  (jig's `domain/accounts.md`, 4.2). Default: a tool of area 5 once the
  QA loop plays a service paid by a person's account (section 14).
- **An export's `written` answered to a later call** (5.5, area 5): when
  `export` ends before `written` is answered, the next call that sends
  it reports its answer as any kept request's, and nothing removes
  FILE when it is refused "cancelled", though FILE's record is then
  dead (an import of it makes an account needing sign-in at its first
  refresh). Default: so, until the QA loop meets it; the alternative
  keeps FILE's path with the kept request, a stored format. Until then
  the agent tells which by `accounts`, which sends the kept `written`
  first: the account's `sign_in` source gone (or the account), FILE is
  its only holder; the source still listed, signed in or moving, the
  move was cancelled and FILE's record is dead.
- **Why a moved record is "not ours"** (5.5, area 5): jig's refusal
  carries no reason, so a form newer than the engine's jig reads, another
  application's record and another registration's read alike. Default:
  `import`'s own check of the form (5.5) covers ctl's jig; a reason on
  jig's refusal (form, application, registration) is jig's, when a form
  2 exists.
- **A tokens file's layout** (5.5, area 6): temper's bundle is items by
  number (`../protocol/credentials.md`, pasted values), which no person
  writes. Default: `NAME VALUE` lines, as 5.5 has them; JSON, or a file
  per token, if the QA loop asks.
- **An agent and GitHub's browser steps:** a manifest's or an install's
  link needs a person's browser signed in on GitHub. Default: the tools
  print the link for a person and open nothing; tests drive its steps as
  plain requests on skein's fake forge (11.2, 11.3); an agent alone
  connects a Forgejo, or an App of its own with `github-app`.
- **The picker's read as a keyed request:** `repositories` draws a key
  as the other unmarked acts do, and the engine keeps its answer for the
  retention (`README.md`, section 11, the picker's page as a read).
  Default: kept as any keyed request; a read of the forge's family once
  the engine serves one.
- **Instructions, the charter's or an amendment's:** a task's document
  carries the instructions in force, and nothing says whether an
  amendment set them (6.1). Default: `task N` prints them as they are;
  a word for their source once jig's document says it.
- **A milestone's phase in `activity`:** the activity line holds only
  the position of the task's history row (jig's `OwnerActivity::
  Milestone`), and the phase is on that row (`OwnerMilestoneLine`).
  Printing it costs a second read per page, a history read of the task
  after the page's first position (jig's `Read::History`), whose page
  is bounded and holds other rows too, so it may not reach every
  milestone the page names, and a cut to print where it does not.
  Default: `phase changed` (6.1); the history read with a `history`
  tool, if the QA loop asks.
- **Raw values left in the text** (6.1's words do not reach them yet):
  - a role, as its number, in acts and in `me`'s held policy (`role N`);
  - a grant's connector and kind, as numbers, not 4.1's words;
  - a grant's resource pattern, as bytes and `*`, not 4.1's words and
    `…`;
  - an amendment's implications, as numbers;
  - a turn's resource references and part references, as bytes and
    JSON;
  - an account's lines in `me`, as the record's fields;
  - charter and procedure numbers in creation bounds and delegation
    authority, and an authority's tool and note numbers;
  - a bounded authority's walls, as nanoseconds;
  - a change contract on a connector other than the forge's, in the
    header, as `change, connector C kind K`;
  - a choice's option in a result, as `option N`, not the option's
    words, which the contract holds.
- **Waiting on one forge:** `wait forges` watches every forge's row, so
  another forge's change answers a wait with no condition. Default:
  `--until settled` or `--until connected:F` names what is waited for; a
  subject per forge, `forge F`, if the QA loop asks.
