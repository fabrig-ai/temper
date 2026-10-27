# Documentation for agents

Draft, 2026-10-27. temper's documentation and the command line it
documents, as they should end up, taken green field from the user's
words on temper#9: the readers are agents, since temper's command line
is for automated, unattended delivery; people read the site, which is
the user's and outside this draft; the text stays concise, with one
consistent structure that never grows by accretion; and the tools may
be renamed and regrouped where that serves. For agents it replaces
`configuring.md` 7 (one source, three readers, written for people) and
`../protocol/documentation.md` 1, 2, 4 and 7 (the register, the
directory, the kinds and their bounds, `--help`, the site); it reshapes
`../client/tools.md`'s names, its 5.4 (`wait`), 8.1 and 11.5, and
`../qa.md` 3's citations of the box walk. Approved by the user on
temper#13 (10-27), the command line regrouped as section 7 has it; each
part becomes contract in the documents it names as it is built.

## 1. In one page

- **Agents are the readers,** three of them: the operator's agent,
  which sets a deployment up and keeps it with the binary's commands;
  the driver, which works through a deployment for a person with
  `temper ctl`; and temper's own runs, which read through their host
  tools. QA's passes play the first two with nothing more than they
  have (section 2).
- **Every word temper shows an agent is documentation:** a result and
  its acts, an error, the help, the pages, a host tool's description, a
  stock setting's text. One voice, one vocabulary, one set of rules
  (section 6).
- **One map.** The nouns of temper's model are at once the concept
  pages, the groups of `temper ctl` and the glossary: `task` is a page,
  a group (`temper ctl DIR task show 41`) and a word. Walks and recipes
  reach a goal across nouns. Each page is a row of one table, and a row
  is added only by a change of this design (section 4).
- **Read cheapest first:** what a call prints (its acts, an error's
  remedy), then five kinds of page: the reference, generated; the
  concepts; the walks and the recipes; the index, one screen that
  orients (section 3).
- **The reference is generated, never written twice.** Each command,
  tool, argument, setting field and member of the configuration is a row
  of a table in the code with its text; the parser, the help, the JSON
  schema and the reference page derive from it (8.1).
- **Checked against the code at the gate:** every command line in a
  page parses, every address resolves, every named setting or member
  exists, every page keeps its kind's template and bound, and the
  checked-in reference equals what the tables generate (8.2).
- **The command line, regrouped:** `temper ctl DIR GROUP VERB`, the
  groups the map's nouns and the verbs a closed vocabulary; a wait
  becomes a read with `--until`. The binary's own commands are sorted by
  who runs them, and what temper spawns itself is set apart (section 7).
- **One address per page,** the same for `temper help`, `temper ctl
  help`, `--help` and the runs' `read_docs` (7.3).
- **Built a part at a time** (section 10): the issue's asks first, in
  today's names; the renames as slices of their own, announced first.

## 2. The readers

| Reader | Who | Reaches the text by | Needs first |
|---|---|---|---|
| The operator's agent | sets up and keeps a deployment for a person: `temper init`, `box` or `serve`, a forge, accounts, an upgrade | a shell, often before anything is configured: `temper help` works before `temper init` | the index, a walk, the binary's reference |
| The driver | works through a deployment for a person: adopts, asks, follows, answers, decides, configures | `temper ctl`, a process per call | the page `ctl`, then each result's acts; a group's reference; a concept when the model surprises it |
| temper's own runs | a chat, a coder, the configuring agent | its charter's instructions, its host tools' descriptions, and `read_docs`; no shell into temper | the settings' concepts and reference, the recipes, the limits |

- **The moments:** first contact (the index, then a walk); every call
  (its result and acts); a failure (the error, its remedy, an address);
  a surprise in the model (the concept); configuring (the settings'
  concepts, their reference and the recipes).
- **QA reads what a user reads.** A pass is given a walk and its rung
  (`../qa.md`, section 3), never a list of commands, so a rename
  reaches it through the walk. A step it finds only in the design or
  the source is a finding (`../qa.md`, section 6).
- **People read the site,** the user's: it may render any page of
  section 9's directory as it is; nothing here is written for it.
  `temper open`, a person's terminal, keeps its usage line alone.

## 3. The kinds of page

| Kind | What it holds | Source | Bound |
|---|---|---|---|
| reference | a group of rows: commands, ctl verbs, host tools, setting fields, configuration members, limits | the tables in code, generated into pages (8.1) | 32 KB a page; a row's notes 1 KB |
| concept | one noun of the map: what it is, how it changes, what temper guarantees, how agents act on it, its limits | written | 8 KB |
| walk | a goal reached by commands, as a runbook | written | 12 KB |
| recipe | a goal reached by a change of settings | written | 3 KB |
| index | what temper is, the map with a line per noun, the walks and recipes, the placeholders | written, its lists checked | 4 KB |

- **At the point of use,** every result prints what can follow as calls
  (`../client/tools.md`, 6.1), and every cut says how to read on. An
  error is one line of at most 240 bytes: what is wrong, the value or
  bound that made it so, and the act that mends it or the page that
  explains. A refusal is said in its concept's words. A host tool's
  description is at most 160 bytes, each member's at most 120. The
  tables' and renderers' step tests hold these bounds. A stock setting's
  text, such as a stock charter's instructions, follows the voice
  within its setting's bounds.
- **A concept** has its headings in this order: *What it is*; *How it
  changes* (its states and its parts, where it has any); *What temper
  guarantees*; *Acting on it*, which names its groups and host tools and
  gives no verb's arguments or flags; *Limits*, said as facts of `main`
  ("temper does not …"), linking `configuration` for what needs a
  restart. A concept describes behaviour, a move of an account between
  deployments among it; a verb's syntax is the reference's.
- **A walk** has *Before you start* (what the agent needs, what it
  decides first), numbered steps, each its commands, then *Expect* (the
  output's first line, verbatim) and *If not* (each other outcome a
  sound temper gives, such as an expired link, a busy forge or a
  timeout, and its act), then *Clean up* and *Next*. Its order is the
  only order that works: a choice that must come before a start is a
  step before it. A bug is fixed in the code, never walked around: a
  fix changes the walk only where the walk was wrong.
- **A recipe** has its goal in one sentence, the settings file that
  reaches it, what temper enforces apart from what agents follow, and
  how to see that it took.
- **A reference page** holds rows, each a `## ` heading of its usage's
  words (`## task show`), so that `help GROUP VERB`, `--help` and a
  search find it: its usage line, its line of text, its arguments
  (name, type, bound, required, a line each), its result (a line of
  text, and its record under `--json`), whether it is keyed, its
  refusals beyond the exit classes `ctl` gives, and its notes, a
  paragraph of what this verb alone does beyond its line.

## 4. The map

The map's single copy is `temper-docs`' table, a row per page; this
section is its design. It lists the end state: a page joins the table
with the part that makes it true of `main`.

| Noun | Concept | ctl groups | Covers |
|---|---|---|---|
| ctl | `ctl` | the client's: `sign-in`, `sign-out`, `pending`, `help` | a call's loop (read, act, wait), the wait conditions, keys and kept requests, acts and the revisions they name, the exit classes, JSON; the forge read with tools of the agent's choosing, and what temper owns there changed only through temper |
| deployment | `deployment` | `deployment` | the engine, the worker, a box, a server, the three directories, the configuration document |
| party | `party` | `me`, `party` | people and services, roles, pools, signing in to temper, credentials as files |
| project | `project` | `project` | projects, their home, their members |
| forge | `forge` | `forge` | kinds, modes (connected, managed, owned), GitHub's App and installations, the repositories a forge reaches |
| repository | `repository` | `repository` | adoption, its role, the home, the landing branch, CI and checks |
| account | `account` | `account`, `model` | LLM providers, accounts, sources, keys, a provider's sign-in, a program's login, a move between deployments, models |
| task | `task` | `task` | tasks and their parts (tree, activity, turns, run), delegates, phases, a task's own holds and their release, stop and cancel, amendments, a task's settings revision |
| hold | `hold` | `hold` | a placed hold: its owner, its mode (stop or drain), its scope (a task's subtree or a project), what lifts it; placed by `task hold` and `project hold` |
| chat | `chat` | `chat` | a conversation, its template, following it as a task, closing it |
| goal | `goal` | `goal` | tracked work, priority, plans, advisors |
| change | `change` | `change` | a change's way to landing: its pull request, CI, gates, queue, repairs, holds, merge; what the forge shows and temper does not |
| landing | `landing` | `landing` | a landing task: its members, each candidate with its base, its gates' receipts and runs, its proof, the turn; a repository's receipts (`repository receipts`) |
| finding | `finding` | `finding` | something found that is not its finder's to fix now: a labelled issue in the project's home, its kind, severity, area and state; triage (route, backlog, drop) |
| qa | `qa` | `qa` | QA after landing: the marks of what a user can now do, the chooser, the passes on the newest cleared `main` |
| inbox | `inbox` | `inbox` | what waits on the party: questions, proposals, escalations; updates |
| proposal | `proposal` | `proposal` | proposals of every kind, deciding, withdrawing |
| settings | `settings` | `settings` | the settings and their kinds, applying, proposing, running work |
| revision | `revision` | `revision` | revisions, the history, restoring, moving running work to the newest |
| charter | `charter` | (settings) | a charter's fields and defaults; the forge's charter |
| template | `template` | (settings) | a template's fields and defaults |
| policy | `policy` | (settings) | a policy's fields and defaults; the rules |
| landing rule | `landing-rule` | (settings) | a landing rule, the deployment's and a project's |
| budget | `budget` | (none) | what work spends, budgets, deadlines |
| playground | `playground` | `playground` | the demo, its fakes, starting it |

- **Walks:** `box` (a local box from nothing to a landed change, and
  back), `server` (a long-lived deployment), `forgejo` and `github`
  (connecting each, GitHub's shared App read-only among them),
  `configure` (changing how work runs), `upgrade`.
- **Recipes:** "review before landing", "a tester that cannot land", "a
  weekly budget". Each comes with the part that makes it true.
- **Reference pages:** `ctl-GROUP` for each group, `commands` for the
  binary's, `host-tools`, `settings-fields`, `configuration` (its
  members, and which need a restart) and `limits` (the bounds).
- **The rules:** every group names its concept; a concept may cover
  several groups, or none; a row's name, kind and noun are the table's,
  and the gate holds the directory to the table. A noun, walk or recipe
  the design does not name is no row.

## 5. The placeholders

One set, defined once in the index, and no other in a written page:
`DIR` (ctl's directory), `CONFIG`, `DATA` and `CACHE` (the three
directories), `FILE`, `TEXT`, `KEY`, `TASK`, `PROJECT`, `FORGE`,
`INSTALLATION`, `ACCOUNT`, `REVISION`, `PROPOSAL`, `PROVIDER`. A walk
that needs another value names it in *Before you start*. Reference
pages print the tables' argument names, which the regrouping (7.2)
renames to the same set.

## 6. The voice

- **For an agent about to act:** the present of `main`; steps in the
  imperative; one fact a sentence.
- **temper's words,** each defined once, in its concept; never a word of
  the design's: no crate, no section number, no kit's name.
- **Exact:** commands as typed, values with their units and bounds,
  outputs verbatim, a limit as a fact.
- **Nothing twice:** a fact lives on one page and others give its
  address. The exit classes and the wait conditions are `ctl`'s; what
  needs a restart is `configuration`'s; a verb's syntax is its row's.
- **True of `main`:** no page describes what `main` does not do. A
  behaviour and the text that tells it land in one change.

## 7. The command line

### 7.1 The binary

```
documentation   temper help [NAME | --search WORDS...]
deployment      temper init | serve | box | upgrade | versions | program-login
                temper forge fetch | prepare | run | sign-in
clients         temper ctl DIR ...            for agents
                temper open DIR LINK          for people
spawned         temper internal ...           never run by hand
test doubles    temper fake ... ; temper box --fake-backends
```

- **Every command is a row of the binary's table,** as ctl's tools are:
  its parser, `temper COMMAND --help` and its reference derive from it.
- **What temper spawns itself** (`temper agent`, `temper forge child`,
  `serve --box`) moves under `temper internal`, out of the index and
  the walks, with its usage line alone; `temper forge reconcile`, a
  transition, stays out of the index until it goes.

### 7.2 `temper ctl`

```
temper ctl DIR GROUP VERB [ARGUMENT...] [--json]
temper ctl DIR me | inbox | sign-in | sign-out | pending
temper ctl DIR --call FILE [--json]
temper ctl help [GROUP [VERB]]
```

- **DIR stays explicit:** an unattended agent never acts on a
  deployment it did not name.
- **The only one-word commands** are the client's and the party's own:
  `me`, `inbox`, `sign-in`, `sign-out`, `pending`, `help`.
- **The groups are the map's nouns; the verbs a closed vocabulary,**
  each with one meaning in every group:
  - reads: `show` (one), `list` (many), and a part of the group's noun
    by the part's name, as its concept names it: `task tree`, `task
    activity`, `task turn`, `task run`, `forge repositories`,
    `repository receipts`, `qa marks`, `qa passes`;
  - acts: `start`, `close`, `message`, `answer`, `amend`, `stop`,
    `cancel`, `release`, `hold` (a placed hold, over the group's object),
    `lift` (a placed hold, ended), `decide`, `withdraw`, `take`, `hand-back`,
    `succeed` (a successor named), `choose` (what QA tries next), `route`
    (a finding to a goal), `backlog` (a finding set aside), `drop` (a
    finding dismissed, with a reason),
    `move` (a task to another party), `seen`, `apply`, `propose`,
    `restore`, `advance` (running work to the newest revision), `adopt`,
    `setup`, `connect`, `disconnect`, `install`, `accept`, `add-key`,
    `replace-key`, `replace` (a pasted value), `forget` (a credential
    temper holds, erased), `declare`, `verify`, `rank`, `retry`,
    `export`, `import`, `make`.

  A new verb is a change of this list; `../client/tools.md` 5.5's
  area 4, jig's plumbing, takes its group and verbs when designed.
- **A wait is a read:** a read of one watched subject takes `--since
  DIGEST`, `--until CONDITION` and `--timeout DURATION`, and with either
  of the first two blocks as `wait` does today: `change show 2 --until
  landed`, `task show 1 --until replied`. `--until changed` is a bare
  wait's: the subject's next change. A read that joins several subjects
  (`task show N --tree --run`) takes none. A chat is followed as its
  task. `wait` goes.
- **`--call`'s `tool`** is the command's words: `{"tool": "task show",
  "arguments": {...}}`.
- **Today's tools, regrouped:**

  | Today | Then |
  |---|---|
  | `tasks`, `task N`, `activity N`, `turn N A.T`, `run N` | `task list`, `task show N`, `task activity N`, `task turn N A.T`, `task run N`; `task tree N`, from `task N --tree` |
  | `message`, `answer`, `amend`, `stop`, `cancel`, `release N --hold R`, `decide N --hold R` | `task` and the same verb |
  | `chat --project P TEXT`, `chat N`, `close N` | `chat start`, `chat show N`, `chat close N` |
  | `proposal P/K`, `decide P/K` | `proposal show`, `proposal decide` |
  | `settings`, `setting KIND N`, `change-settings`, `propose-settings` | `settings show [KIND N]`, `settings apply`, `settings propose` |
  | `revision R`, `revisions`, `restore R`, `move-to-newest R` | `revision show R`, `revision list`, `revision restore R`, `revision advance R` |
  | `accounts`, `models`, `connect`, `disconnect`, `add-key`, `replace-key`, `forget`, `declare`, `verify`, `rank`, `retry`, `export`, `import` | `account list`, `model list`, then `account` and the same verb |
  | `forges`, `repositories F`, `forge-setup`, `forge-connect`, `forge-install`, `forge-accept`, `forge-replace`, `forge-forget` | `forge list`, `forge repositories F`, then `forge` and the verb |
  | `adopt`, `change N`, `demo-start` | `repository adopt`, `change show N`, `playground start` |
  | `wait SUBJECT` | the subject's read with `--until` or `--since` |

  The named tools of `../client/tools.md` 5.5's areas 2 and 3 take
  their group: `goal start`, `proposal withdraw`, `inbox seen`, `task
  take`, `task hand-back`, `task move`, `party make` (a service),
  `project show`, `deployment show`.
- **The host tools keep their names,** smith's form for a run's tools
  (`propose_settings`, `decide_escalation`), on the map's nouns: the
  run's `change` makes the change that ctl's `change show` reads.

### 7.3 One address for each page

- **An address is a page's name** in the table: a noun, a walk, a
  recipe, or a reference page.
- **The same page by every road:** `temper help NAME`; `temper ctl help
  GROUP` is `ctl-GROUP`, and `help GROUP VERB` its row; `temper COMMAND
  --help` and `temper ctl DIR GROUP VERB --help` print their row; a run
  reads any page with `read_docs`, as today: a page from an offset, or
  the sections matching some words, read-only, bounded, with no
  authority (`../protocol/documentation.md`, section 5).
- **`temper help` prints the index;** `temper --help` the binary's part
  of it; `temper ctl help` the page `ctl`, then the groups, a line each.

## 8. Staying true, staying small

### 8.1 The reference, generated

- **The tables:** ctl's tools (`temper-client-tools`, as today), the
  binary's commands (new), the host tools (`temper-engine-smith`, as
  today), the settings file's fields, the configuration's members
  (where `temper.json` is decoded), and the limits (the profiles'
  bounds).
- **The settings file's form is decoded in one place,** by one table of
  its fields (name, type, bound, default, a line), the decoder of
  ctl's `change-settings` and of the runs' `propose_settings` alike;
  today's hand-written decoder becomes that table's.
- **Each table renders its own rows** as reference pages; the pages are
  checked in beside the written ones, and a step test of `temper`,
  which depends on every table, fails when one differs from what its
  table renders, printing the page as it should be. A table's change and
  its page land together, a reviewer reads the page's diff, the site may
  render it, and `temper-docs` stays a crate that depends on nothing.

### 8.2 Checked at the gate

`temper-docs`' and `temper`'s step tests, as
`../protocol/documentation.md` 6's are, extended:

- the directory equals the table, each page's kind its row's;
- each page within its kind's bound, its headings its kind's template;
- every fenced line that starts `temper ctl ` or `temper ` parses with
  its table's parser;
- every `temper help NAME` names a page;
- every setting field a written page names in a code span, as
  `KIND.FIELD` (`charter.models`), and every configuration member, as
  its path from the document's root (`engine.limits`), exists in its
  table;
- in a written page, placeholders from section 5 alone;
- outside code spans and fenced blocks, none of a short list of the
  design's words: the kits' names, `.md`, a crate's prefix.

### 8.3 Walked

- **QA walks the walks,** on each rung (`../qa.md`, section 3): a step
  that does not get a pass through is a finding, fixed in the code or,
  where the walk was wrong, in the walk.
- **Later, named:** a walk run as a story of the process tier on fake
  backends, its commands read from the walk itself; when a walk drifts
  in a way the parse misses.

### 8.4 Small

- The bounds of section 3; at most 96 pages, a bound only this design
  moves.
- One fact in one place (section 6), which the kinds' templates make
  the default.

## 9. Where it lives

- **`docs/help/`,** beside the design, holds every page, the generated
  ones under `docs/help/reference/`. Today's `docs/guide/` moves there;
  whatever the site renders of it is the site's.
- **`temper-docs`** holds the table (each page's name, kind, noun, title,
  summary and text), and finds, pages and searches, as today.
- **`temper help`, `temper ctl help`, `--help` and `read_docs`** print
  and answer from it.

## 10. The parts, in order

1. **This vision,** to the user. Nothing is built on it before their
   yes.
2. **The issue's asks, in today's names:**
   - `docs/help/`, the table with its kinds, bounds and templates, and
     8.2's checks; the index; the page `ctl` in place of ctl's guide.
     Today's eight guides move in under a transitional kind, `guide`,
     with today's bound and checks, which the gate allows for those
     eight names alone and which ends empty with this part;
   - the walks `box` and `github`: the order of `temper init`, `temper
     box`, `forge-setup` and an account's import, and its export before
     the stop, made the walk's order; GitHub's shared App read-only, a
     step before the start;
   - ctl's per-tool help whole enough to drive a rung without the
     source: every argument's line, the result's line, the refusals,
     the notes, rendered at run time as today;
   - the configuring agent's pages: the concepts `settings`,
     `revision`, `proposal`, `charter`, `template`, `policy` and
     `landing-rule`, the settings file's table and its generated
     `settings-fields`, and each recipe with its part;
   - the rest of today's guides rewritten into the map's pages (`server`,
     `forgejo`; `task`, `chat`, `goal`; `budget`), so that `guide` ends.
3. **The command line regrouped** (section 7), as slices of their own,
   each announced to every owner beforehand and never landed mid-walk
   of a rung: ctl's groups and verbs and the reference pages
   `ctl-GROUP`, the waits as reads, the binary's table and `temper
   internal`. The parse check names every page line a rename leaves
   stale, so the pages change in the same slice.
4. **The rest of the map:** the remaining concepts and walks, and the
   generated pages of the binary, the host tools, the configuration and
   the limits.
5. **Errors' remedies,** area by area: each error names its act or its
   page, within its bound.

## 11. Open questions

- **Whether the site renders `docs/help/`:** the user's.
- **A default DIR** for ctl: default none (7.2).
- **JSON results' fields** in the reference: default, a row gives its
  result's line and record's name, and `--json` keys are
  self-describing, until a result's record has a schema to generate
  from.
- **The settings file's table, jig's or temper's:** the fields are of
  jig's settings, the JSON form ctl's. Default: temper's, beside the
  decoder; jig's once a second application documents the same form.
- **The host tools' names** on the groups' words (`settings_propose`
  for `propose_settings`): default, unchanged (7.2).
- **The MCP endpoint** (`../client/tools.md`, section 14): its tools'
  descriptions from the same table, when it comes.
- **Translations:** English only.
