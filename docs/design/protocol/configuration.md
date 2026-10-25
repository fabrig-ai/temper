# Configuration

Provisional, 2026-10-14, revised 2026-10-23. What temper's processes
are told when they start: the one JSON document `temper serve` reads,
and the forge's setup's commands with it, with jig's parts as jig
defines them and temper's own members beside them; the private files it
names; the agent's configuration; what `temper open` and `temper
program-login` are given; limits; what holds at a first start and at
every start; and the formats an operator writes. A forge's credentials
are not configuration: they are records in jig's secrets' store
(credentials.md, section 2).
jig's shape and rules are jig's `protocol/configuration.md`; this
document says what temper adds. What is open is listed in section 11.

## 1. In one page

- **One document per `temper serve`,** in jig's shape: three top-level
  members, `engine` (jig's engine part), `workers` (jig's worker parts)
  and `application` (temper's members). Nothing else at the top level.
  The forge's setup's commands read the same document (section 8). Its
  members give its shape: standalone, or the cluster's engine alone or
  worker alone (section 2).
- **Three directories,** configuration, data and cache, by skein's
  arguments' rule: the document is always `temper.json` in the
  configuration directory, each path in it is typed by the directory it
  is relative to, and no secret sits in the configuration directory
  (section 12).
- **Read once, before the loop, and strict.** Unknown, missing,
  duplicate or out-of-bound members refuse the start, naming the path
  (jig's `protocol/configuration.md`, section 1). A change is a restart.
- **jig's parts are jig's:** `jig-protocol-config` parses them; temper
  writes no parser of its own for them, and adds only the checks its
  shapes need (section 2).
- **temper's members,** under `application`: `forges`, `identities`,
  `landing`, `charter`, `clients`, `webhooks`, `setup`, `demo` and
  `limits` for the engine (section 3), and `workers`, one entry per worker (section 4).
  `webhooks` is the forge component's webhooks' listener, on loopback
  (forge.md, section 5).
- **Forges of two kinds,** Forgejo and GitHub, each by its `number`: its
  explicit, stable id, never a list position, never reused for another
  forge. A forge's records are kept under it, so a changed number is a
  new forge, not connected (section 3). A GitHub forge's addresses
  default to github.com's, and are given only to point at a fake.
- **`setup`, for the forge's setup** (setup.md): how each set-up forge is
  run and set up, which the `temper forge` commands and `temper box` act
  on, and the engine's setup host reconciles at each start, and at the
  box's `started` for an owned forge set up from the client (setup.md,
  7.1). `temper serve` reads one file of it, an owned or fake forge's
  handover (sections 3 and 6).
- **`demo`, a first run's demo** (setup.md, 8.4): the fake LLM the box
  runs, its scripts and its key, and what the seed adopts; null for
  none. `temper serve` reads none of its files.
- **Not configuration:** repositories, which people adopt at runtime;
  the branch namespace, which the root derives from the deployment's id;
  the forge writer's identity, which comes from each forge's connect and
  which skein's forge client keeps in the main store (credentials.md,
  2.1); a forge's credentials, which come from the client or the engine's
  setup host (credentials.md, section 2).
- **No secret in the document,** and no forge credential in a file the
  document names. Each forge's credentials are records in jig's secrets'
  store under temper's tag (credentials.md, 2.1). The private files the
  document names are jig's, a worker's secret and its digest, an owned
  or fake forge's handover, each read at start, and the demo model's
  key, which `temper serve` never reads (section 6).
- **LLM providers, never accounts.** jig's `engine.llm` names the LLM
  providers, written out whole: their endpoints, models and prices, and
  the sources each offers. Accounts are made at runtime from the client,
  and jig keeps them (section 2; credentials.md, section 3).
- **One format rule** for temper's typed members: member names are the
  Rust field names, and the parser follows the types (section 10).
- **Limits by name,** their numbers in `temper-service`'s limits module,
  overridden by dotted name. The forge component's (`renew_before`,
  `log_poll`, `log_catch_up`, `log_pages` among them) are given once for
  every forge, never a member of `forges` (section 7).
- **Standalone shape:** exactly one worker, joined to the engine by the
  in-process name `workers`; the clients' listener, and the webhooks'
  listener when configured, on loopback, the clients' at a fixed port
  when a GitHub forge is configured; the agent program is temper's own
  binary. `temper init` writes it, and every deployment is it.
- **The cluster's two shapes,** for tests and the lab on one machine
  (README, section 15): an engine alone, with no worker and a workers'
  listener on loopback in plaintext, and a worker alone, with no engine,
  dialling it. A document of none of the three is refused (section 2).
- **The agent's configuration is smith's** (section 5), and `temper open`
  takes no file at all (section 9).
- **Settings leave the document** (section 13; designed, not built):
  jig's charters, templates, policies and rules, and temper's landing
  rules and forge's charter, become settings in the store, changed by a
  decision an owner accepts, with the binary's stock values as their
  defaults. The document keeps what a start needs before it reads its
  store, and what belongs to the machine.

## 2. The document

```
{
  "engine":      jig's engine part
  "workers":     [ jig's worker part ]
  "application": temper's members
}
```

- **`engine`** is jig's engine part (jig's `protocol/configuration.md`,
  section 3), in jig's formats (its section 10). Its `clients` member is
  what jig's clients' component takes; the clients' listener is temper's
  (section 3).
- **`engine.llm`** is jig's LLM providers (jig's
  `protocol/configuration.md`, section 3), written out whole, never
  implied: each with its endpoints and the models they serve, each
  model's window, output, efforts, prices and what it is good at; the
  sources it offers, a sign-in with its public registration, keys, a
  program, or none; and the facts its terms ask temper to show. It holds
  no account, key or record: accounts are made from the client
  (credentials.md, section 3). temper adds no member; the registration's
  name hint is `temper` in every deployment (jig's `domain/providers.md`,
  2.2). The owned example writes out OpenAI with keys and ChatGPT's
  sign-in, a plan's, using its public Responses API (setup.md, 8.1); a
  provider temper does
  not ship is another entry, such as a local model's endpoint on
  loopback with no source to hold.
- **A sign-in's redirect** binds a loopback listener on an ephemeral
  port for each sign-in, unless the LLM provider's registration fixes a
  port (jig's `protocol/configuration.md`, section 3; skein's
  `oauth.md`, 6.5). A fixed port is checked at start against the
  deployment's own listeners: the clients' and the webhooks', an owned
  or fake forge's `listen` (setup.md, section 8) and the demo's fake
  LLM's (setup.md, 8.4), whose ports it must not share on loopback.
- **`workers`** is a list of jig's worker parts (jig's
  `protocol/configuration.md`, section 4).
- **`application`** holds temper's members (sections 3 and 4).
- **jig's reference fixtures** are a complete example of jig's parts: the
  reference binary's test configuration (`jig-test`'s
  `tests/support/mod.rs`, made from jig's channel world's `fixtures.rs`).
  temper's process tier writes its own the same way (`../domain/testing.md`,
  2.6), and the owned example is a whole document (setup.md, 8.1).
- **What standalone requires of jig's parts,** each checked at start, a
  failure naming its path:
  - `workers` has exactly one entry, whose `engine` is `{"in_process":
    "workers"}`;
  - `engine.hosts.listener` is `[{"in_process": "workers"}]`, and
    `engine.hosts.workers` names that one worker;
  - `engine.connectors` is exactly `[1]`: the forge connector, temper's
    one connector, whose number both ends of the client's wire fix as
    `temper-client-protocol-wire`'s `FORGE` (`../client/README.md`, 4.6);
  - the worker's `agent` paths are absolute (section 5).

  The worker's `agent` names temper's own binary (section 5), which no
  check can tell from another program.
- **The shape is read from the members,** before any other check, and
  each shape's checks are its own (README, 15.1):

  | Shape | `engine` | `workers` | `application` |
  |---|---|---|---|
  | standalone | present, `hosts.listener` `[{"in_process": "workers"}]` | one, its `engine` `{"in_process": "workers"}` | every member, `workers` one entry |
  | engine alone | present, `hosts.listener` one socket address on loopback (in `127.0.0.0/8`, or `::1`), plaintext, its port fixed or 0; `hosts.workers` one or more | `[]` | every member, `workers` `[]` |
  | worker alone | absent | one, its `engine` a socket address on loopback, plaintext | `workers`, one entry, and `limits`; no other member |

  - an engine alone is checked as standalone is, but for its listener
    and its workers, by the table, each worker's `digest` a data path
    of its own;
  - a worker alone is checked for its worker part, its entry of
    `application.workers` (section 4), its agent's configuration
    (section 5) and the limits its side takes (section 7), and for
    nothing of an engine's;
  - any other document is refused, naming the member that departs from
    the nearest shape: an in-process listener beside a socket, an
    engine with both a worker and a socket listener, a worker alone
    with another member. An in-process listener tells jig's core that
    no worker outlives the engine (jig's `domain/engine.md`, section 6),
    which a socket's workers would.
- **What temper's root requires of jig's parts,** in every shape with an
  engine, each checked at start, a failure naming its path:
  `engine.limits`' `fleet.engine_slots` is 0, and every charter's `hosts` is `"workers"`.
  The root has no hub child, so the fleet must place every run on a
  worker (`../domain/root.md`, section 2).
- **Templates are jig's two defaults,** chat (template 1, label 1) and
  goal (template 2, label 2), as jig's `protocol/configuration.md`,
  3.1 gives them, the chat's executor the charter chats run under;
  temper defines no template of its own. One flag departs from jig's
  defaults: the chat is ended by its requester, `"ended_by":
  "requester"` (jig's `protocol/configuration.md`, 3.2), so that a chat
  stays open past its run's turn until its person closes it
  (`../domain/agent.md`, 4.6); the goal is ended by its result, as
  every delegation is. Its client's lists filter on
  those labels (`../client/README.md`, 3.5), and a project's policy may override
  their defaults as jig's allows.
- **A project's permission mapping,** jig's `roles` of each of
  `engine.projects` (jig's `domain/people.md`, section 4), names the
  forge's permissions by number under the forge connector, 1: 1 read, 2
  write, 3 admin. An adoption gives a repository's collaborators the
  roles their permissions map to (`../domain/forge.md`, section 4).
- **Reading,** by `temper serve`, in this order:
  1. the three directories, resolved and checked (section 12); then
     the document, `temper.json` in the configuration directory, whole,
     with skein's startup kit, within the bound jig's reference binary
     sets on a configuration's bytes;
  2. jig's parts, by `jig-protocol-config`, and the shape they give;
     `application`, with the members its shape takes, by
     `temper-service`'s startup, with `jig-protocol-config`'s readers, so
     every problem names its path the same way
     (`application.forges[0].api`); `setup`, `demo`, and the setup's and
     the box's limits, by their own crates' checks (setup.md, sections 8,
     8.4 and 11);
  3. the checks across the whole that its shape holds: the references
     of sections 2 to 5,
     the clients' listener's fixed port with a GitHub forge and each
     identity's largest value against the channel's bound on a
     credential's value (section 3), the limits against each other and
     the worst case (section 7), and the channel's start checks on
     temper's parts (channel.md, section 8);
  4. every private file the document names, each within its bound
     (section 6);
  5. the agent's configuration (section 5).

  Every problem found is reported, one line each on stderr, and `temper
  serve` exits 1 before io starts. A forge's records are not read here:
  the engine's routing loads them from jig's secrets' store once io
  starts, before the forge component starts, and a forge whose records
  do not serve is not connected, never a refused start (section 8;
  credentials.md, 2.1).
- **Paths are typed:** each is a configuration, data or cache path,
  relative to its directory or absolute, as jig's parts' are (jig's
  `protocol/configuration.md`, section 10), or a path that must be
  absolute; section 12 gives each member's. None is resolved against
  the working directory.

## 3. temper's members for the engine

**`forges`:** a list of the forges temper works on, at least one, and at
most the forge client's `repositories` limit (`forge.client.repositories`),
which bounds its writers. Each:

| Member | Type | Bounds | Feeds |
|---|---|---|---|
| `number` | u16 | unique among `forges` | the domain's forge number: a repository's `forge`, a writer's, a git identity's; and the `forge` of the numbers its records are kept under in jig's secrets' store (credentials.md, 2.1) |
| `kind` | string | `"forgejo"` or `"github"` | the documents the forge component speaks (forge.md, section 3); the records the forge takes (credentials.md, 2.1); the ways it is connected (setup.md) |
| `api` | an address, jig's format, or `null` | a socket address, or a host's name and port; never an in-process name. `null` on GitHub alone, for github.com's: `api.github.com`, port 443 | where the forge component connects for the forge's REST API: Forgejo's under `/api/v1`, GitHub's at its root |
| `transport` | `"plaintext"`, `{"tls": {"server_name": string, "trust": a configuration path or null}}`, or `null` | plaintext only when `api` is a socket address on loopback; `trust` null is the machine's roots. `null` on GitHub alone, for TLS to github.com's API with the machine's roots | the forge component's connections |
| `git` | string, or `null` | an `http` or `https` URL: a host, an optional port and an optional path; no user, query, fragment or final `/`; `http` only to a loopback host. `null` on GitHub alone, for `https://github.com` | the base of each repository's remote, `<git>/<owner>/<name>.git`, which the engine's routing makes (channel.md, section 4); and the host segment of the forge's resource paths: `git`'s host, with `:port` when it names a port |
| `git_program` | a data path or an absolute path, or absent | within 4096 bytes, resolved under data as setup's `fake.git`; refused together with that member, naming both | the git executable workers run for this forge's repositories, for a custom git build or wrapper; absent keeps `workers[].git` (section 4), or the demo's existing `fake.git` (setup.md, 8.4) |
| `web` | string, or `null` | on GitHub, a URL by `git`'s rule, `null` for `https://github.com`; on Forgejo, `null` | GitHub's browser pages: the manifest's form, the install link and an installation's pages (setup.md, connecting GitHub; `../client/forge.md`) |
| `log_origins` | list of strings, or `null` | each an origin: `"https://<host>[:<port>]"`, TLS with `transport`'s trust (the machine's roots when it names none), or `"http://<host>[:<port>]"` to a loopback host alone, as skein's fake serves its logs; no path, user, query, fragment or final `/`; the port 443 or 80 when absent. Each host is resolved at start, and one that does not resolve refuses the start, naming the forge and the host; at most `forges.log_origins` (section 7). `null` is the kind's default: GitHub's `["https://productionresultssa1.blob.core.windows.net"]`, Forgejo's none | the origins to which a job log's redirect is followed (forge.md, GitHub's API as temper uses it); `[]` fetches no job log |
| `app_read_only` | boolean, or absent | on GitHub alone, refused on Forgejo naming its path; absent is `false` | `true`: the pasted App (setup.md, 17.3), its hook and its installations are another's, and the engine's setup host changes none of them (setup.md, 17.7): the paste check and reconcile leave the App's hook as they read it, and a stray installation is shown and never uninstalled; installing, accepting and the forge component's reads and mints are unchanged |

- **A forge's number is its stable id,** never its place in the list:
  its records in jig's secrets' store and its repositories in the main
  store are kept under it. Reordering, adding or removing forges moves
  no forge's records to another. A number is never reused for another
  forge, and a forge whose number changes is a new forge, not connected,
  whose old number's rows are left (credentials.md, 2.1).
- **A GitHub forge's addresses** (`api`, `transport`, `git`, `web`) are
  `null` in a deployment: a value is given only to point at a fake, as
  temper's tests do, or at the live tier's relay to github.com
  (`../domain/testing.md`, 2.7). A Forgejo forge gives
  `api`, `transport` and `git`.
- **No credential is configured:** a forge's API token, git token,
  webhooks' secret, App's key and user are records, kept from the
  client or by the engine's setup host (credentials.md, section 2). A
  forge's `kind` and `api` are checked against its `Origin` record at
  start: records made for another kind or address leave the forge not
  connected, named, and erase nothing (credentials.md, 2.1).

**`identities`:** a list of git identities, temper's application
credentials (credentials.md, section 4), one per forge, and at most
jig's limit on an engine's application credentials (the accounts child's
`credentials`). Each:

| Member | Type | Bounds | Feeds |
|---|---|---|---|
| `number` | u32 | unique among `identities`; from 1 to the accounts child's `credentials`, in `engine.limits` and in the worker's `limits`, since the worker's identities' table holds those numbers alone | the credential's number, as the root names it in `Event::Credential` and in grants |
| `forge` | u16 | the `number` of one entry of `forges`; each forge named by exactly one identity | the forge whose value the identity takes, and whose repositories' runs use it (credentials.md, section 4) |
| `login` | string, or `null` | not empty; no `:` and no control byte. `null` is the forge's: on Forgejo the bot's login, as the forge's connect names it; on GitHub `x-access-token` | the user of git's HTTP basic authentication |
| `author` | `{"name": string, "email": string}`, or `null` | each not empty; no NUL byte, newline, `<` or `>`. `null` on GitHub alone, for the App's bot: `<slug>[bot]`, with `<id>+<slug>[bot]@users.noreply.github.com` (credentials.md, section 4) | the author and committer of the commits made with it (git.md, section 5) |

An identity's value is its forge's, never configured: a Forgejo forge's
`GitToken` record, or a GitHub forge's installations' tokens, minted at
runtime (credentials.md, section 4). The largest value an identity can
take, encoded (channel.md, section 6), must fit the channel's bound on a
credential's value, or the start is refused, naming the identity: its
login and author, with a Forgejo forge's token at `records.value_bytes`,
or a GitHub forge's list at `forges.installations` entries, each at its
largest (section 7).

**`landing`:** the deployment's and the projects' landing rules, the
root's `LandingPolicy`. Leaving the document: section 13 makes them
settings in the store, temper's kinds 1 and 2; until mission 2 builds
it, they are configured here.

```
"landing": {
  "deployment": [ rule ],
  "projects": [ {"project": u32, "rules": [ rule ]} ]
}
```

A rule is a `LandingRule`, member for member:

| Member | Type | Meaning, and bounds |
|---|---|---|
| `connector` | u16 | the forge connector's number, the one `engine.connectors` names |
| `kind` | u16 | the effect kind the rule governs: 4, the forge's merge, the only kind a landing rule takes |
| `pattern` | jig's pattern, as jig's `rules` and `projects` write one: `{"segments": [string], "last": "exact" or "open", "value": string}` | the resources the rule covers, as jig's authority matches them: the exact base segments, then one more segment equal to `value` (`exact`), or beginning with it, with all below it (`open`). The forge's resource path is `forge`, the forge's host (as `git` gives it), the repository's owner and name, then `branch` and the branch's segments |
| `enforce` | boolean | true makes each criterion below a requirement every landing it covers must meet (jig's `domain/authority.md`, section 10; `../domain/forge.md`, 8.7); false leaves them to the change procedure alone |
| `ci` | boolean | CI must have passed at the change's head |
| `up_to_date` | boolean | the change's head must contain its base's tip |
| `gates` | list of `{"number": u32, "blocking": boolean, "freshness": "exact" or "clean"}` | the gates every change it covers asks for (`../domain/forge.md`, 8.3): an agent's review, or one of a repository's checks when it has no CI; at most the forge's `change_policy.gates` |
| `approvals` | list of `{"role": u32, "people": u32, "freshness": "exact" or "clean"}` | `people` distinct people holding `role` in the project approve the change, each by a review on the forge (`../domain/forge.md`, 8.7) |

- **`freshness`,** a gate's or an approval's, says at which later heads
  a verdict still counts: `exact`, the head it was given at; `clean`,
  every head reached from it by updates temper made that merged cleanly
  (`../domain/forge.md`, 8.3). It is not jig's freshness of an observed
  requirement, the age within which its facts must have been read: CI,
  gates and approvals are guarded, and up to date is observed with a
  freshness of 0 (`../domain/forge.md`, 8.7).
- **Bounds:** each list of rules holds at most jig's authority limit on
  requirements; a project appears in `projects` at most once, and must
  be one of `engine.projects`; each approval's `role` must be a role of
  its project's policy, and, for a deployment rule, of every configured
  project's policy and every configured policy a project is made from
  (jig's `engine.policies`).
- **Feeds** `Config.landing`: `deployment` and `projects` as they are.

**`charter`:** u32, the charter the forge's procedures start agent work
under; it must be the `number` of one of `engine.charters`. Feeds
`Config.forge_charter`. Leaving the document: temper's kind 3 (section
13).

**`clients`:** the clients' listener, which `temper-service` owns
(`../client/README.md`, section 8), and the browser's bundle it serves
(`../client/browser.md`, 2.2):

| Member | Type | Bounds | Feeds |
|---|---|---|---|
| `listener` | an address, jig's format | a socket address; on loopback (in `127.0.0.0/8`, or `::1`) in standalone, and wherever `engine.clients` enables the printed link (jig's `protocol/service.md`, section 3); a fixed port, never 0, when a forge of kind GitHub is configured | the listener on which jig's clients' component is mounted with jig's default HTTP mapping, under `/jig`; the mapping's Host allow-list; the address of the sign-in links `temper serve` prints (`../client/README.md`, section 7); and temper's forge routes beside it, whose addresses a GitHub App's redirect and setup URLs name (`../client/README.md`, the routes) |
| `bundle` | an absolute directory, or `null` | a bundle of skein's bundler, built apart (`scripts/bundle.sh`): its page and its digest's directory; its files, and the binary's own, within `clients.bundle_files` and `clients.bundle_bytes` (section 7) | the browser's page and bundle, made at start and served on the clients' listener (`../client/browser.md`, 2.2): `null` serves the bundle the binary holds (2.6), or, from a binary built without one, the page for no bundle; a directory serves its bundle in place of the binary's. A document naming an old `share/temper/bundle` keeps serving that directory, older than the binary once it is rebuilt: set it to `null` |
| `public` | an origin, or `null` | `https://HOST[:PORT]`: HOST a lower-case DNS name, the port not 443 when written, no user, path, query, fragment or final `/`; `http://` only to a loopback host, for a test; within 253 bytes | the address people's browsers use, behind a TLS proxy (`sign-in.md`, 2.2): the mapping's Host allow-list holds its host beside the listener's (jig's `Settings.public`, the host with its port where not 443), jig's cookie is `Secure` by it, and every address temper gives out is the origin's: Google's return, a GitHub App's redirect and setup URLs. `null`: the listener's address alone, `public: None` |

- **A fixed port with a GitHub forge:** a GitHub App's `redirect_url`
  and `setup_url` name the clients' listener's address, and GitHub's API
  cannot change them once the App is made. A port of 0 with a GitHub
  forge configured refuses the start, naming
  `application.clients.listener`.

**`sign_in`:** the people's sign-in with Google (`sign-in.md`), or
`null` for none, never missing: with `null`, the browser's signed-out
page keeps its printed link's line, and `/sign-in/google` and its return
answer not found.

```
"sign_in": {
  "google": {"provider": 2, "client": "1234-abc.apps.googleusercontent.com",
             "secret": "google-client.secret", "endpoints": null},
  "members": [{"address": "sam@example.com", "login": "sam"}]
}
```

| Member | Type | Bounds | Feeds |
|---|---|---|---|
| `google.provider` | u16 | a provider of jig's `engine.providers` of kind application | the identity's provider, in `Mint` and in jig's `owners` (`sign-in.md`, section 3) |
| `google.client` | a string | Google's client id: 1 to 256 bytes of visible ASCII | the client's `client_id` |
| `google.secret` | a data path | a secrets' file (section 6), its bytes 1 to `sign_in.secret_bytes`, visible ASCII | the client secret, in the token request's body |
| `google.endpoints` | an object, or `null` | `null` for Google's own (`sign-in.md`, 3.1); otherwise `{"issuer", "authorization", "token", "keys", "transport"}`, `null` in a deployment; a value is for a test, or any issuer that is not Google, and names no deployment: each an `https` URL, or `http` to a loopback host, within `sign_in.url_bytes`; `transport` as a forge's, `"plaintext"` to loopback alone or `{"tls": {"server_name", "trust"}}`. Each host resolved at start; one that does not resolve refuses the start, naming it | the registration's endpoints and the fetcher's origins (`sign-in.md`, 6.3) |
| `members` | a list | 1 to `sign_in.members` | who may sign in, each `{"address", "login"}` |
| `members[].address` | a string | 3 to 254 bytes of visible ASCII, one `@` with bytes on both sides; unique among members with ASCII letters folded to lower case | the identity's subject, exactly as written; compared with Google's verified `email`, folded |
| `members[].login` | a string | 1 to 40 bytes of ASCII letters, digits, `-`, `_` and `.`, its first and last a letter or a digit; unique, folded | the login in `Mint`, and the name where Google gives none |
| `forges` | a list, or absent for none | each `{"forge", "client"}`: `forge` the number of a Forgejo of `forges`, `client` the id of one of jig's `engine.clients.provider.clients`, in visible ASCII (the page's metas carry it), whose `redirect_uri` is exactly the forge's `git` URL's `/user/oauth2/temper/callback`; each forge and each client at most once; with no provider, refused naming `engine.clients.provider` | the forges that sign their people in through temper (`sign-in.md`, section 9): the page's `temper-forge-sign-in` metas, "Open Forgejo ↗", an owned Forgejo's `app.ini` (setup.md, 6.2) and `temper forge sign-in` (setup.md, 6.8) |

- **jig's owners of Google's provider are members:** each of
  `engine.owners` whose provider is `google.provider` has a member's
  address as its subject, or the start is refused, naming the owner; and
  one of them at least, the first, signs in by the printed link
  (`sign-in.md`, 2.3).
- **Google's provider is not the link's:** `google.provider` differs
  from `engine.clients`' link provider, or the start is refused, naming
  both.

**`limits`:** temper's profile and overrides (section 7).

**`webhooks`:** the webhooks' listener, the forge component's own
(forge.md, section 5), or `null` for none:

| Member | Type | Bounds | Feeds |
|---|---|---|---|
| `listener` | an address, jig's format | a socket address on loopback (in `127.0.0.0/8`, or `::1`), as the clients' listener's in standalone; unique among the deployment's listeners: neither the clients' listener's nor an owned forge's `listen` (setup.md, section 8) | the forge component's listener for Forgejo's deliveries, plaintext HTTP on skein's HTTP server, one path per Forgejo forge whose records hold a webhooks' secret (credentials.md, 2.1), `/hooks/<number>` by the forge's `number`; the URL of each hook the setup's plans make (setup.md, section 5); an owned Forgejo's `[webhook] ALLOWED_HOST_LIST` (setup.md, 6.2) |

- **A delivery is a hint,** never a fact: polling is the same with
  webhooks or without (forge.md, section 6), and finds what a delivery
  lost, refused or never sent told of.
- **A forge on another host polls:** its deliveries would need TLS on
  the listener (README.md, section 14; setup.md, section 15).
- **A GitHub forge has no path:** its hints come from its App's
  delivery log, which the forge component reads, with no address GitHub
  must reach (forge.md, section 5).

**`setup`:** the forges' setup, a list of at most one entry per forge,
`[]` for none, and never missing (section 10). Its members, their bounds
and the checks of the forge's own members against each entry are
setup.md, section 8's. An owned entry names no org: its org is its
first repository's, which the set up makes (setup.md, 6.7), and
reconcile makes none.
`temper serve` checks it whole, and the engine's setup host reconciles
each managed and owned forge at each start, and an owned forge whose setup says `"start": "asked"` once the box has
started it (setup.md, 7.1; the setup host in the engine). A fake forge
runs start-time adoption instead (setup.md, 8.4). Of the files `setup`
names, `temper serve` reads one: the handover, `admin_token`, taken
once for an owned forge (section 6), read and left for a fake's later
starts. A fake forge's `git` it names and never reads: the program the
engine's routing names in each of that forge's repositories' records,
for the worker to spawn (channel.md, section 4).

**`demo`:** a first run's demo, or `null` for none, never missing
(section 10): the fake LLM the box runs, its address, its LLM provider,
its key's file and its scripts, and the demo's forge and the project
its start makes. Its members, their bounds and
their checks against the document's LLM providers, `setup` and
policies are setup.md, 8.4's,
`temper-fake`'s. `temper serve` checks it whole and reads none of its
files; the engine takes nothing of it.

**What the engine root's `Config` takes,** field by field:

| Field | From |
|---|---|
| `core` | `engine`, by `jig-protocol-config`, with the deployment's id and seed |
| `landing` | `application.landing` |
| `forge.writers` | not configured: each forge's writer comes from its connect, its user as the routing tells the root (credentials.md, section 5) |
| `forge.namespace` | not configured: the root derives it from the deployment's id |
| `identities` | one `GitIdentity { forge, credential: number }` per entry of `identities` |
| `forge_charter` | `application.charter` |

Beside the root: the forge component takes each forge's `number`,
`kind`, `api`, `transport` and `log_origins`, with the access the
engine's routing builds from the forge's records (credentials.md,
section 5), and the webhooks' listener, `webhooks.listener`; the
engine's setup host takes each forge's `kind`, `api`, `transport` and
`web`, and `setup`; the identities' table takes each identity's number,
forge, login and author, its value from its forge (credentials.md,
section 4); the engine's routing takes each forge's `git`, and each fake
forge's setup's `git`; the clients' listener takes `clients.listener`,
and its forge routes each GitHub forge's `web`.

## 4. temper's members for a worker

**`application.workers`:** a list with one entry per entry of the
top-level `workers`, in the same order; standalone and a worker alone
have one, an engine alone none. Each:

| Member | Type | Bounds | Feeds |
|---|---|---|---|
| `root` | a directory, a data path | made private (0o700) if absent; an existing one must be the process's user's, private, and not reached through a link | the worker's workspace directory: every checkout's workspace is made under it (git.md, section 2; `../domain/worker.md`, section 5) |
| `git` | string, or `null` | an absolute path to an executable file, checked at start, whose `git version` is at least the one git.md, section 3 names; `null` for none, as `temper init` writes it on a machine with no git (setup.md, 8.2) | the program the git component runs for each invocation in a repository whose record names none (channel.md, section 4), in the environment git.md, section 5 gives it; with `null`, each such invocation fails, naming its forge's host, and the row of each connected forge but a fake one warns (git.md, section 3; `../client/forge.md`, 5.4). The demo's git is the fake forge's setup's `git`, never this (setup.md, 8.2) |

- **The worker's root takes the rest from jig's and smith's:** the hub's
  limits from jig's worker part; smith's host limits and the bounds its
  hello states from the agent's configuration (section 5), as jig's
  reference binary derives them; its stop bound from jig's graces; the
  checkout's limits from temper's profile (section 7).
- **Workers are alike** (jig's `protocol/configuration.md`, section 4):
  no credential but the worker's secret, and no forge configuration.
  Remotes, git identities and a fake forge's git program come with each
  assignment, in its repositories' records (channel.md, section 4).
- **A worker on another host** takes its own machine's git as its `git`:
  a fake forge is on loopback alone (setup.md, 8.4), and serves no
  worker on another host. The cluster's workers are on its machine, and
  it serves them (README, 15.2).

## 5. The agent's configuration

- **smith's agent configuration** (smith's `protocol/agent.md`, section
  4), a file of smith's, named by jig's worker part's `agent` member:
  - `program`: the absolute path of temper's binary;
  - `configuration`: the absolute path of smith's agent configuration;
  - `arguments`: `["agent", <that same path>]`,

  so each agent process is `temper agent AGENT.json` (README.md,
  section 4). They are absolute because the worker spawns the agent
  with its arguments as written (jig's `protocol/configuration.md`,
  section 10); `temper init` writes them so, with smith's agent
  configuration as `agent.json` in the configuration directory
  (setup.md, 8.2).
- **Read twice:** by `temper serve` at start, for smith's host limits,
  the channel's limits and the agent's bounds, as jig's reference binary
  reads it; and by each agent process as it starts.
- **It holds no credential:** an agent's grants come over smith's
  channel (credentials.md, section 3).
- **A model's reasoning allowance,** its `reasoning_item` (smith's
  `protocol/agent.md`, section 4), bounds each reasoning item the model
  sends; past it, with smith's default `oversized_reasoning` of `fail`,
  the call fails and the run holds at once, naming the bound and its
  value (`../domain/worker.md`, 4.3). smith derives its largest turn
  from it, so the allowance and the hub's bounds go together: the start
  refuses a document whose bounds do not hold that turn (section 7).
  temper's documents give a plan's model an allowance its reasoning
  does not reach in ordinary use (setup.md, 8.1), a default a person
  may lower, with the bounds it derives.
- **`programs`** (smith's `protocol/agent.md`, section 4): each program
  the agent may run as a backend, by its adapter, with the program's
  absolute path and its programs directory, which holds a configuration
  directory for each program source, `<directory>/NAME`
  (credentials.md, section 3). Its paths are smith's, absolute; the
  programs directory `temper init` writes is `programs` in the data
  directory (section 12). The owned example's agent configures
  Claude Code's (setup.md, 8.1). A programs directory need not exist
  until `temper program-login` makes it; one that exists must be a
  directory of the user's own with no group or other bits, not reached
  through a link at its name, or the start is refused, naming it
  (skein's `io.md`, 5.3). A linked parent is accepted. temper reads
  nothing inside it and creates no missing programs directory at serve.
- **The same names as the engine's** (jig's `protocol/configuration.md`,
  section 4), checked at start: every endpoint of `engine.llm`'s
  providers must be in the agent's configuration under the same name,
  every model a charter allows must be declared there, and every
  program an LLM provider offers must be among the agent's `programs`,
  or the start is refused, naming it. One process reads both, so a
  mismatch that would fail every run's charter is caught before the
  loop.
- **The worker must fit the configured charters.** After those name checks,
  startup builds jig's core configuration once, expands its instruction files,
  and projects its fleet needs with jig's `fleet_needs::configured`. The
  worker's bounds pass through the same `hello_bounds` conversion as its
  hello and jig's `shortfall` selects the first unmet requirement. Startup
  names that requirement's configuration path, required and offered amounts,
  with its dimension's units; the same core configuration then checks the
  root. Admission policy stays in jig, including checks fitting within a
  modifying charter's reserve.

## 6. Secrets' files

| File | Named by | Holds | Read by |
|---|---|---|---|
| an owned or fake forge's handover: its admin's token | `admin_token` in the forge's setup directory, `application.setup[].directory` (setup.md, section 3) | the token's bytes | the engine's setup host, once, at start or the box's `started` (setup.md, 6.5); a fake forge, before binding (setup.md, 8.4) |
| a worker's secret, its digest | jig's parts, data paths: `worker.secret` and `worker.digest` in the data directory as `temper init` writes them (setup.md, 8.2) | jig's formats (jig's `protocol/configuration.md`, sections 7 and 10) | jig's components |
| Google's client secret | `application.sign_in.google.secret`, a data path | the secret's bytes | `temper serve`, once, at start, within `sign_in.secret_bytes` |
| the OpenID provider's signing key, and each of its clients' secrets | jig's `engine.clients.provider` (jig's `protocol/configuration.md`, section 3) | jig's formats: the key PKCS#8 PEM, made by `temper forge sign-in` (setup.md, 6.8) | `temper serve`, once, at start, for jig's helper that builds the issuer (`sign-in.md`, 9.2) |
| the demo model's key | `application.demo.llm.key`, a data path: `demo/llm.key` as `temper init` writes it (setup.md, 8.2) | the key's bytes | the fake LLM, at its start, and the seed (setup.md, 8.4); never `temper serve` |

- **A secret file holds its bytes and nothing else** but a final
  newline, which is not part of the secret (jig's
  `protocol/configuration.md`, section 10).
- **Private reads,** each within its bound, with skein's
  `read_private_file` (skein's `shell.md`, 6.1): a file readable by
  anyone but the process's user, or reached through a link, is refused,
  naming it. For the handover, the forge stays not connected, or set up
  failed at the box's `started`; the engine and other forges run on
  (setup.md, 6.5). Its bound is `records.value_bytes` (section 7).
- **A present handover is always taken over.** At start or the box's
  `started`, the setup host reads it once, even with a kept
  `AdminToken`, runs `TakeOver`, and removes the file only after its
  `Done` and the record kept in jig's secrets' store (setup.md, 6.5).
  Removal must succeed before reconcile; its failure leaves the file
  for the next start or ask to take over again. With no file, the host
  reconciles from the kept record; with neither, the forge is not set
  up.
- **A set-up forge's directory** (`application.setup[].directory`) and
  its other files are setup.md, section 3's, made by the setup's
  commands; `temper serve` reads none of them but the handover.
- **Never written by `temper serve`,** but for the handover it removes
  once taken. An operator replaces a worker's secret and restarts; a
  forge's credentials are replaced from the client, with no restart
  (credentials.md, section 5).
- **Every forge credential in jig's secrets' store,** under temper's
  tag, and in no file but an owned or fake forge's handover until it is
  taken:
  a forge's tokens, its App's key, its webhooks' secret, its user, from
  the client or from the engine's setup host (credentials.md, sections
  2 and 2.1).
- **No LLM account's file.** LLM accounts' records and keys are in
  jig's secrets' store, and a program's login is in its configuration
  directory, which temper never reads (credentials.md, section 3).

## 7. Limits

```
"limits": {"profile": "small", "overrides": {"<name>": value}}
```

- **Every limit temper's own domains and components take:** the forge's
  and its change and issue policies, the queue window, the forge
  client's, the checkout's, the root's own beyond jig's core, the forge
  component's, its webhooks' listener's and its GitHub App's among them,
  the git component's, the engine's routing's records and forges, the
  setup host's, the clients' listener's forge routes' and the picker's;
  and the box's, which `temper serve` checks and takes none of but
  `box.line_bytes`, its pipe's (setup.md, section 7).
- **One profile, `small`,** for one person's machine; another name is
  refused. Its numbers are constants in `temper-service`'s limits module,
  and the setup's and the box's in their own crates, each documented
  with what it bounds; the design names limits and what they bound, and
  never their numbers (jig's `protocol/configuration.md`, section 6).
  Where a limit of temper's is a quantity jig's profile also sets, its
  constant cites jig's small profile's.
- **`small` starts as it stands,** with no override, against the agent
  temper runs: smith's, whose run bounds (its directories, their names,
  its accounts) are smith's own and no configuration changes (smith's
  `smith-domain-run`, `derive`; `smith-agent-service`), and whose largest
  call, its run's `outcome_bytes`, follows from its agent configuration,
  the owned example's (setup.md, 8.1). Every constant passes the
  checks below with that agent: a task's resources and the checkout's
  repositories within the agent's directories, the checkout's names
  within theirs, and a run's push message, which smith bounds as a call,
  within the checkout's commit message. So `small` takes a forge's `git`
  base of at most 122 bytes, the longest remote's bound (below). A test
  whose agent has other bounds states the overrides it needs.
- **Overrides by dotted name:** the path of the field in its owner's
  `Limits` type, a duration's last segment with `_ms`. A name temper does
  not know, or a value outside its type, refuses the start, naming it.

  | Prefix | Owner |
  |---|---|
  | `root.` | the engine root's own limits, beyond those jig's `engine.limits` sets |
  | `forge.` | `temper-engine-domain-forge`'s `Limits`: `forge.client.`, `forge.change_policy.` and `forge.issue_policy.` for its children, `forge.queue_window_ms` |
  | `checkout.` | `temper-worker-domain-checkout`'s `Limits` |
  | `forge_component.` | the forge component's, its webhooks' listener's and its GitHub App's among them (forge.md, section 7): the owner's, given once for every forge as the component is made (skein's `forge-connection.md`, 7.2), so none is a member of `forges` |
  | `git_component.` | the git component's (git.md, section 8) |
  | `records.` | the engine's routing's: a forge's records and carried values in jig's secrets' store (credentials.md, 2.1) |
  | `forges.` | the engine's routing's, per forge: a GitHub forge's accepted installations and every forge's log origins (forge.md, section 7) |
  | `setup.` | `temper-forge-setup`'s `Limits`: reconcile's (setup.md, section 5), the fetch's (setup.md, 6.6), and the engine's setup host's states and runs (setup.md, the setup host in the engine) |
  | `box.` | `temper-box`'s `Limits`: the box's restarts and stop, its pipe's lines, its logs and tails (setup.md, 7.1 and 7.2) |
  | `clients.` | `temper-service`'s clients' server's `Limits`: the browser's bundle (`../client/browser.md`, 2.2) |
  | `routes.` | `temper-service`'s clients' listener's forge routes (`../client/README.md`, the routes) |
  | `sign_in.` | `temper-service`'s Google sign-ins on the clients' listener (`sign-in.md`, section 6) |
  | `picker.` | the picker's reads of a forge's repositories (`../domain/forge.md`, section 4) |

  For example `forge.repositories`, `forge.client.calls`,
  `checkout.remote_timeout_ms`.
- **The webhooks', the keep's settling, the records', the forges', the
  GitHub App's, the setup's, the routes', the picker's, the box's and
  the bundle's limits,** whose numbers are `small`'s constants, as every
  other's:

  | Name | Bounds |
  |---|---|
  | `forge.client.settle_ms` | after a move of a pull request's head or base, how long a not mergeable answer, or a hinted job's pending CI, is read again at `hinted` and not believed, by the change procedure (`../domain/forge.md`, 5.2); above 0, as `hinted` is |
  | `forge_component.hooks` | the webhooks' listener's connections in all |
  | `forge_component.hook_head_bytes` | a delivery's request head |
  | `forge_component.hook_body_bytes` | a delivery's body, hashed and decoded as it streams, so time and bytes, not memory; one past it is answered `413` and hints nothing |
  | `forge_component.hook_deadline_ms` | `hook_deadline`: a delivery's read and answer, from its connection's accept or its last answer |
  | `forge_component.hook_secret_bytes` | a Forgejo forge's webhooks' secret; a GitHub forge has none (credentials.md, section 2) |
  | `records.value_bytes` | a forge's record or a carried value of temper's, the largest of any item; a pasted value past it is refused, named (credentials.md, 2.1 and 2.2). The client wire's value ceiling is the larger of it and a moved record's bound (jig's `protocol/client.md`, section 5; `../client/tools.md`, 12), so a moved record's file reaches the engine and temper's own values stay within this bound |
  | `forges.installations` | the installations a GitHub forge accepts, its `Accepted` record's ids (credentials.md, 2.1) |
  | `forges.log_origins` | a forge's `log_origins`; at most 4 (section 3) |
  | `forge_component.installations` | the installations the component holds tokens for, per forge |
  | `forge_component.log_origins` | the origins a job log's redirect may reach, per forge (default 4); as many origins said of other redirects are kept per forge (`Fact::Origin`, forge.md, 3.6) |
  | `forge_component.token_bytes` | a forge's token: an API token, a git token or a site admin's token (skein's `forge-connection.md`, 7.2) |
  | `forge_component.key_bytes` | a GitHub App's key |
  | `forge_component.renew_before_ms` | `renew_before`: how long before an installation's token expires it is minted again (credentials.md, section 5) |
  | `forge_component.log_poll_ms` | `log_poll`: between two reads of a GitHub App's delivery log (default 5 s) |
  | `forge_component.log_catch_up_ms` | `log_catch_up` (default 24 h): how far back the delivery log is read when a forge starts again within one process (skein's `forge-connection.md`, 6.5); at most a day, the log's own reach. temper keeps no `guid` across process starts: a process's first read of a forge's log keeps the newest and reads nothing back, and skein's forge client reads each adopted repository afresh at restart |
  | `forge_component.log_pages` | the pages one read of the delivery log takes (default 10) |
  | `forge_component.delivery_bytes` | a delivery read from the log (default 1 MiB) |
  | `setup.orgs` | a managed entry's `orgs` (until temper-15); an owned entry names none |
  | `setup.deadline_ms` | a setup plan's run, its wait for the forge among it |
  | `setup.backoff_ms`, `setup.backoff_max_ms` | a setup plan's first request made again, doubling |
  | `setup.states` | the setup host's states, held in memory, for the manifest's and the installation's returns |
  | `setup.state_life_ms` | a state's life; at most an hour, the life of the manifest's conversion code |
  | `setup.runs` | the setup host's runs at once, a forge's one writing run and its one `Reachable` beside it both counted; a request past them, or a second of either kind on one forge, is refused busy |
  | `setup.fetch_bytes` | a fetched binary's body, written to its part file as it streams, never held; one past it removes the part file and is refused, naming the URL (setup.md, 6.6) |
  | `routes.query_bytes` | a forge route's query |
  | `picker.page` | the repositories one read of the picker takes, the setup host's `Reachable` plan's `reachable` on every kind (skein's `forge-setup.md`, 4.8), within the forge's own page cap: GitHub's page, a connected Forgejo's; at most 100, GitHub's largest page |
  | `box.backoff_ms`, `box.backoff_max_ms` | a long step restarted, doubling |
  | `box.restarts`, `box.window_ms` | a long step's exits within the window past which the box gives up; `box.restarts` at most 64; the backoff counted from the exits within the window (setup.md, 7.2) |
  | `box.grace_ms` | a long step's grace, after each of its stop's two signals (setup.md, 7.2) |
  | `box.line_bytes` | a line of the box's pipe, either way, its line feed among them; the box and `temper serve` each read within it (setup.md, 7.1) |
  | `box.log_bytes` | a step's log file, past which it moves to `<name>.log.1` and starts afresh (setup.md, 7.2) |
  | `box.tail_lines` | the lines of a step's log the box holds, and writes under a failure's terminal line (setup.md, 7.2) |
  | `clients.bundle_files` | the bundle's files, its page among them |
  | `clients.bundle_bytes` | the bundle's files' bytes in all, held in memory while `temper serve` runs |
  | `sign_in.members` | the configured members, and the forge connector's bindings held (`forge.bindings`, the members' count) |
  | `sign_in.pending` | the sign-ins under way at once; a start past them is answered the busy page |
  | `sign_in.deadline_ms` | a sign-in's life, from its start to its return's answer; the sign-in cookie's `Max-Age` |
  | `sign_in.fetches` | the fetcher's exchanges with Google at once |
  | `sign_in.body_bytes` | an answer's body from Google: the key set or the token |
  | `sign_in.url_bytes` | a configured endpoint, and the authorization URL given to the browser |
  | `sign_in.secret_bytes` | the client secret |
- **Limits checked against each other at start,** each failure naming
  both limits:
  - the forge component's: its connections equal the forge client's
    calls in flight; its deadline is at most the forge client's
    lifetime; its body bound holds the forge client's largest write and
    a key's marker; every listing's page size is at least 1 (forge.md,
    section 7);
  - the git component's: each rule of git.md, section 8 (its operations
    at least the checkout's workspaces, its output caps at least what
    it parses and the diagnostic it keeps);
  - the records': `records.value_bytes` within the largest value jig's
    secrets' store keeps (jig's `protocol/store.md`, section 9), and
    large enough for the maximum encoded `AppKey`, `Accepted`, `AppSlug`
    and every configured GitHub forge's `Origin`: the component's
    `key_bytes`, 8 bytes per `forges.installations`, or `forge.name_bytes`,
    each with its version byte; `Origin` counts both its kind and version
    prefixes and the configured API address with its port (credentials.md, 2.1);
  - the forges': `forges.installations` at most
    `forge_component.installations`; `forges.log_origins` at most
    `forge_component.log_origins`, and each forge's `log_origins` within
    it; `forge_component.renew_before_ms` below the hour an
    installation's token lives; `forge_component.log_catch_up_ms` at
    least `forge_component.log_poll_ms`; each of these limits, and
    `routes.query_bytes` and `picker.page`, above 0;
  - the setup's and the box's: each backoff at most its maximum;
    `setup.backoff_max_ms` below `setup.deadline_ms`; `box.backoff_max_ms`
    below `box.window_ms`; `setup.states`, `setup.state_life_ms`,
    `setup.runs` and `setup.fetch_bytes` above 0; `box.log_bytes` at least
    one whole record (the time, the box's longest piece and a line feed);
    `box.tail_lines` above 0 and at most 100; `box.line_bytes` at
    least 64, which holds each line but a `failed` one's `WHY`, which
    the box cuts to fit (setup.md, 7.1);
  - the bundle's: `clients.bundle_files` and `clients.bundle_bytes`
    above 0;
  - the sign-ins': each above 0; `sign_in.body_bytes` within skein-oauth's response bound;
    `sign_in.members` within the forge's bindings' bound;
  - the webhooks' listener's: each of its limits above 0;
    `hook_secret_bytes` at least 32; `hook_head_bytes` at most the
    HTTP server's largest read within io's intake; `hooks` within the
    sockets left once the clients' listener and the forge's connections
    are counted. The sockets are temper's own, a constant of
    `temper-service`'s limits module that its io check takes in place
    of the sum of jig's engine's and worker's, and `small`'s hold a
    document with both listeners and a sign-in, as the owned example has (setup.md,
    8.1);
  - the channel's (channel.md, section 8): each of temper's largest
    parts within the ceiling jig's schema gives its field; the largest
    part the engine can send within what the worker's limits take; the
    forge's `resources_per_task` equal to the checkout's `repositories`,
    since the engine's terms take the smaller and a worker's parts past
    them never open the link; the longest remote the routing can make (the longest `git` base, an
    owner and a name of the forge's `name_bytes`, two `/` and `.git`),
    and the forge's names and branches, within the checkout's
    `name_bytes`;
  - the worker's, against the agent's configuration
    (`temper-worker-domain`'s `worst_case`), among its rules: the
    checkout's repositories, names, conflicts and paths within the
    agent's directories, their names, conflicts and paths; the agent's
    largest call within the checkout's commit message; and, of the hub's
    (jig's, in the worker's `limits`), its accounts within the agent's,
    its largest event with smith's 2-byte separator within the agent's
    message, and the agent's message less that separator within the
    hub's event. So `hub.event_bytes` is the agent's `message_bytes` less
    2, and no other value starts;
  - the turn's, against the agent's configuration: smith's largest
    turn, the `turn_body` its configuration derives (smith's
    `smith-agent-service`, `profile`; its models' reasoning allowances
    among what it derives from, section 5), is a whole conversation
    turn's body as the hub measures it, and each bound on its way holds
    it. Within each hub's `turn_bytes`, the engine's and the worker's,
    since each side of the link takes a turn of its own hub's bound;
    with its 4-byte length, within `core.resume_bytes`, so that a chat's
    largest turn resumes rather than starting afresh, and so within
    `root.journal.transcript_bytes`, a journal row's (the resume
    setting's check below); within `root.loads.transcript_bytes`, a
    loaded row's, and with a row's slot, within `root.loads.bytes`, a
    loaded page's, so that the engine loads it back; and with a read's
    row, within `root.loads.read_bytes`, so that a person's read
    carries it whole. Then the engine's resume window, as each hub
    counts it in an assignment's transcript (jig's `domain/hosts.md`):
    `core.resume_bytes` and, for each turn it can hold, a box past the
    4 bytes it already counts, within each hub's `transcript_bytes`.
    Each failure names the bound, its bytes, and the figure it must
    hold: `workers[0].limits.overrides.hub.turn_bytes: 65536 bytes,
    short of the agent's largest turn, 1039544 bytes`.
- **In the cluster's shapes** (README, 15.1), each process checks what
  its document holds: an engine alone, every check above but the
  worker's and those of the turn's that need the agent; a worker alone,
  the worker's, the checkout's and the git component's, the channel's
  for what its side receives, and the agent's largest turn within its
  hub's `turn_bytes`. What only an engine's document and a worker's
  hold together, the engine's largest part against the worker's
  `limits` among it and the agent's largest turn against the engine's
  bounds, is held by the cluster's description, whose one template
  writes the same `application.limits` into every document (README,
  15.5), and at the link's opening by jig's hello and terms (jig's
  `protocol/channel.md`, 4.3).
- **jig's limits stay in jig's parts:** `engine.limits` and each worker's
  `limits` (jig's `protocol/configuration.md`, section 6).
- **What the root takes of `engine.limits`:** its core children's
  (`tasks.`, `authority.`, `people.`, `fleet.`, `accounts.`, `notes.`
  and `documents.`), with `core.templates`, `core.call_records`, `core.policy_bytes` and
  `core.call_answer_bytes`, each configured byte bound taken unchanged. The
  rest of the core's it gives the core itself (`../domain/root.md`,
  section 10): from its own `root.` limits, in place of `brief.`,
  `core.brief_parts`, `core.brief_core_budgets.`, `core.resume_bytes`,
  `core.run_bytes`, `core.escalation_reason_bytes`,
  `core.load_slots` and `core.load_bytes`, a person's read of the store,
  from `root.loads.read_bytes`; as constants, in place of `views.`,
  `core.fill_mark` and `core.connectors`. An
  override of one of these changes nothing the root holds, but for
  `core.resume_bytes`: the core's resume setting is still taken from it,
  above 0 and within `root.journal.transcript_bytes`.
- **The engine's documents fit temper's client.** A document past the
  bounds a client decodes is refused there whole, as too large (jig's
  `protocol/client.md`, section 3), so the documents the engine serves
  must be within what temper's client takes: its client takes
  `jig-client`'s limits from jig's small profile with the same
  `documents.` overrides the owned example's `engine.limits` names
  (`../client/README.md`, section 2; setup.md, 8.1). The engine cuts each
  text to its bound and says so (jig's `client/domain/contract.md`,
  5.3). It checks none of this at start: it serves whatever client the
  deployment lets in, whose limits are the client's. A test holds the
  owned example's and the process tier's configuration to temper's
  client's limits, so the two cannot drift apart unseen.
- **The engine's watch room fits temper's client.** A watch past the
  room the engine allows a party, or a stream, is refused busy, and each
  holds its place until the engine has ended it, so a deployment allows
  each party, and each stream, at least the watches one client opens at
  its limit, `me`'s included (jig's `client/domain/contract.md`, 7.2;
  jig's `protocol/client.md`, section 13). temper's client opens 16, so
  its engine allows 16 a party, 16 a stream and 16 in all, temper's own
  numbers above jig's profiles' 4, which the root holds whatever
  `engine.limits` says (`../domain/root.md`, section 10). It checks none
  of this at start, as for documents (above); the same test holds these
  rooms to temper's client's watches.
- **The forge's paths fit a document.** jig's documents carry a
  resource's path and a pattern whole, with as many segments as the
  engine admits in a name (jig's `client/domain/contract.md`, 5.3), and
  `documents.lines` bounds a document's short lists alone, at most the
  client's rows. So `engine.limits` must name the bounds on a name's
  segments, `authority.segments`, `tasks.hold_segments` and
  `tasks.authority_segments`, at least the segments of the deepest path
  the forge names: a branch's, the forge's five segments then the
  branch's own (`../domain/forge.md`, section 2), 8 as temper's branches
  name them. temper's client takes a path's segments, 9, apart from its
  rows (jig's `client/domain/client.md`, section 14), so a page of the
  forge's work is never refused for a path, and a long path shows cut,
  with jig's mark (`../client/README.md`, 5.4).
- **The forge's authorities fit a document.** jig's documents carry an
  authority's lists whole, with as many entries as the engine admits in
  each, and neither cut nor refuse a document or an offer for one (jig's
  `client/domain/contract.md`, 5.3), so a party who may amend a task
  reads its document whatever `documents.lines` is. temper's client
  takes an authority's entries, 12, apart from its rows (jig's
  `client/domain/client.md`, section 14): the most the process tier and
  the owned example admit in a list, their `authority.grants` and
  `tasks.authority_grants` (setup.md, 8.1). A long authority shows
  in part, with jig's mark (`../client/README.md`, 5.4). The test above holds
  the engine's bounds on an authority's lists to temper's client's.
- **One worst case per service:** jig's sum (the root's, its components'
  and the journal's) and temper's (its domains', its components', the
  identities' table, the routing's records, forges' accesses and origins
  said, the setup host's states and runs) together must fit the memory
  jig's part states, `engine.memory_bytes` for the engine's service and
  `memory_bytes` of the worker's part for the worker's, each in the
  process that holds it (section 2), or the start is refused, saying by
  how much. The owned example's `engine.limits`, worker `limits` and
  `memory_bytes`, and why each holds, are setup.md, 8.1's.

- **A worker alone's ring** holds its io's and its worker's kernel
  operations, staged and in flight, independently of its domain's room.
  With the owned example's worker it fits the kernel's 32,768 entries.

## 8. First start and every start

- **Every command that reads the document checks it,** once its three
  directories are resolved and checked (section 12). `temper serve`
  checks it whole, as section 2 says. `temper program-login`, every
  `temper forge` command and `temper box` check its structure, its
  members, the limits and `setup`, and none of the checks that need a
  private file read, since a set-up forge's files do not exist before
  its commands make them (setup.md, section 8). Each reads the private
  files its own part needs alone: a `temper forge` command its forge's,
  by setup.md, section 6; `temper box` and `temper program-login`
  none, the latter reading the agent's configuration besides (section
  5); `temper serve` every one the document names, and of `setup`'s an
  owned forge's handover alone (section 6).
- **A deployment's first start** is jig's (jig's
  `protocol/configuration.md`, section 8): the first with an empty store,
  whose owners and projects the core seeds from `engine`. temper adds:
  - **periods from the smallest source,** as jig's reference service
    gives them (jig's `protocol/configuration.md`, section 5): before the
    core admits a party's request, the engine's routing gives the root
    one period for each configured project, its budget the project's
    spend per period, with no next start;
  - **each project's landing rules** join its policy as it is seeded;
  - **a project made after the first start** (jig's `MakeProject`, its
    `domain/core.md`, 6.1) has the deployment's landing rules, which
    every start applies (`../domain/root.md`, section 6), and none of
    `landing.projects`, which names seeded projects alone; its first
    period is made with it (jig's `domain/people.md`, 5.2). The default
    document still seeds project 1 and offers policy 1, equal to its seeded
    policy; the green-CI landing rule is in `landing.deployment` for every
    project (setup.md, 8.3).
- **Every start:**
  - jig's parts apply as jig says (jig's `protocol/configuration.md`,
    section 8);
  - temper's members apply as configured: the forges, the identities,
    the deployment's landing rules, the forge's judges for every rule,
    the charter, the limits and the workers;
  - every limit is checked again, temper's and jig's together, with the
    channel's start checks on temper's parts (section 7; channel.md,
    section 8), so a `git` base or a limit changed since the last start
    is refused before io starts when a remote or a part could pass its
    bound;
  - the root's configuration is held to the root's limits before the
    root is made (`../domain/root.md`, section 6): a start that breaks
    one is refused, naming it;
  - each provider's fixed `sources.sign_in.redirect_port` differs from
    the clients', webhooks' and each owned forge's listener port, even
    between IPv4 and IPv6. An absent or ephemeral listener reserves no
    fixed port. A connected forge's remote API is no local listener;
  - a forge of kind GitHub with a clients' listener on port 0 refuses
    the start, naming `application.clients.listener` (section 3);
  - once io starts, the engine's routing loads every forge's records
    from jig's secrets' store before the forge component opens a forge.
    A forge whose records are missing, incomplete, unreadable, or made
    for another kind or `api` (their `Origin`) is not connected, named,
    and the other forges run on; nothing is erased. Rows of a forge not
    configured are left, and named once in the log (credentials.md,
    2.1);
  - each forge's writer is restored from the main store, where skein's
    forge client keeps it (`Key::Writer(forge)`; credentials.md, 2.1);
  - a GitHub forge's delivery log is read from its newest delivery on:
    temper keeps no `guid` across starts, and skein's forge client
    reads each adopted repository afresh (forge.md, a GitHub App's
    delivery log); `forge_component.log_catch_up_ms` bounds only a later
    start of a forge within the same process (section 7);
  - carried rows that a run left at a stop are erased, a decision of
    the core that the routing carries out; a connect they belonged to
    ends interrupted, and the client asks to paste again
    (credentials.md, 2.2);
  - an owned forge's handover is taken, when its file exists (section
    6), at the start, and at the box's `started` for a forge set up
    from the client, while the engine serves (setup.md, 6.5 and 7.1);
    each managed and owned forge is reconciled by the engine's setup
    host, and shows "connecting" until reconcile is done, while the
    engine serves (setup.md, the setup host in the engine); an owned
    forge whose setup says `"start": "asked"`, with neither a handover
    nor a record of one, is not set up until the client asks the box
    for it (setup.md, 6.5);
  - under the box, `temper serve` takes the listeners the box gives it,
    from fd 3, `LISTEN_FDS` of them, its clients' and webhooks' listeners
    claiming them by their addresses, and `--box` adopts the box's pipe,
    the two fds after them, refusing its start, naming the fd, when
    either is not an open pipe (setup.md, 7.1); a start without the flag
    has no pipe, and leaves those fds alone;
  - every identity whose forge has a value is announced at generation 1
    before the root's restart; a GitHub forge's, once its first tokens
    are minted (credentials.md, section 4). Every connected forge's user
    is checked by the forge component's first exchange (credentials.md,
    section 5);
  - the branch namespace is derived from the deployment's id, the same
    at every start;
  - a later start gives no period: the store keeps each project's
    current one (jig's `domain/engine.md`, 5.4).
- **Repositories are never configured.** A person adopts each at runtime
  through the client, by the forge's number, the owner and the name, and
  the store keeps it (jig's `protocol/configuration.md`, section 3;
  `../client/README.md`, section 4).
- **What a change reaches,** after its restart: a forge's `git`, every
  remote from the next assignment on; its `kind` or `api`, the forge not
  connected until it is connected again, since its records' `Origin`
  names the old ones (credentials.md, 2.1); its `log_origins`, from the
  next job log on; the webhooks' listener, from the first delivery on;
  an identity's `login` or `author`, granted again at generation 1; a
  forge's `number`, a new forge, not connected (section 3); a forge's
  setup, at the engine's next reconcile, or the next run of its
  commands (setup.md, 6.3 and section 5); its `start`, at the box's next
  start, but that a forge with the box's record runs at every start
  (setup.md, 7.1); its `mirror`, at the next fetch; a fake forge's
  `git`, every repository's from the next assignment on; a worker's
  `git`, every invocation, checked at the start.
- **A forge's credentials are no change of configuration:** they are
  connected, replaced and forgotten from the client, with no restart
  (credentials.md, section 5).

## 9. The client's arguments

- **`temper open DIRECTORY LINK_OR_ADDRESS`** takes no configuration
  file: its two arguments are `../client/README.md`, 6.1's, and its limits are
  constants of temper's client (`../client/README.md`, section 2).
- **`temper ctl DIRECTORY ...`** takes none either (`../client/tools.md`,
  section 8).
- **`temper program-login [DIRECTORIES] NAME`** reads the same document
  as `temper serve`, and the agent's configuration it names
  (credentials.md, section 3). `DIRECTORIES` is `[--config DIR] [--data
  DIR] [--cache DIR]`, each with its variable and its default (section
  12), given anywhere among the command's arguments.
- **`temper agent AGENT.json`** reads smith's agent configuration
  (section 5).
- **`temper forge prepare|run [DIRECTORIES] FORGE`** and **`temper box
  [DIRECTORIES]`** read the same document as `temper serve` (section 8;
  setup.md, sections 6 and 7); **`temper init [DIRECTORIES]`** writes
  it (setup.md, 8.2). Reconcile is no command: the
  engine's setup host runs it, at each start and at a connect (setup.md,
  the setup host in the engine).

## 10. Formats

**The format rule** for temper's typed members, from the Rust types they
feed:

- **member names** are the field names;
- **an enum's variant without fields** is a string of its name in
  snake_case (`"clean"`);
- **a variant with fields** is an object with one member named after the
  variant in snake_case, whose value is the field's for one unnamed field,
  or an object of the fields by name (a forge's `transport`:
  `{"tls": {"server_name": ..., "trust": ...}}`);
- **byte strings** are JSON strings, UTF-8;
- **durations** are integers of milliseconds, the member's name suffixed
  `_ms`;
- **an absent option** is `null`, never a missing member;
- **a map keyed by a number** is a list of objects, each holding its key
  under the key's name (`landing.projects`: `project`);
- **integers** are within their type's range; **booleans** are `true` or
  `false`;
- **a value of one of jig's types** is written as jig's configuration
  writes it, so one document spells one type one way: a landing rule's
  `pattern`, jig's authority pattern, is
  `{"segments": [...], "last": "exact" or "open", "value": ...}`, as in
  jig's `rules` and `projects`.

**jig's formats,** by citation (jig's `protocol/configuration.md`, section
10): an address (`{"socket": ...}`, `{"host": ..., "port": ...}`,
`{"in_process": ...}`); a secret file; a digest file. A forge's
`transport` offers the choices jig's LLM endpoints offer (plaintext to
a loopback socket address
only; TLS with a server name and a trust file, or the machine's roots),
written by the format rule as one member of temper's own type, where
jig's endpoints spell them as sibling members (`"transport": "tls"`,
`"server_name"`, `"trust"`).

**The `application` member for standalone,** one forge, one identity and
one worker, complete:

```json
"application": {
  "forges": [{
    "number": 1,
    "kind": "forgejo",
    "api": {"host": "forge.example", "port": 443},
    "transport": {"tls": {"server_name": "forge.example", "trust": null}},
    "git": "https://forge.example",
    "web": null,
    "log_origins": null
  }],
  "identities": [{
    "number": 1,
    "forge": 1,
    "login": null,
    "author": {"name": "temper", "email": "temper-bot@forge.example"}
  }],
  "landing": {
    "deployment": [],
    "projects": [{
      "project": 1,
      "rules": [{
        "connector": 1,
        "kind": 4,
        "pattern": {
          "segments": ["forge", "forge.example", "acme", "widget", "branch"],
          "last": "exact",
          "value": "main"
        },
        "enforce": true,
        "ci": true,
        "up_to_date": true,
        "gates": [{"number": 1, "blocking": true, "freshness": "clean"}],
        "approvals": []
      }]
    }]
  },
  "charter": 2,
  "clients": {"listener": {"socket": "127.0.0.1:7100"}, "bundle": null, "public": null},
  "webhooks": null,
  "setup": [],
  "demo": null,
  "limits": {"profile": "small", "overrides": {}},
  "workers": [{"root": "workspaces", "git": "/usr/bin/git"}]
}
```

A landing rule may name a repository no project has adopted yet: it
covers nothing until a person adopts `acme/widget`. The forge is reached
over TLS from another host, so it polls, with no webhooks. It names no
credential: the person connects it from the client, pasting the bot's
two tokens (credentials.md, 2.2), and its git identity takes the bot's
login and the `GitToken` record.

**The whole document,** jig's members elided as `...` (this skeleton is
not itself valid JSON):

```
{
  "engine": {
    "deployment": ..., "owners": ..., "providers": ..., "rules": ...,
    "projects": [{"number": 1, ...}],
    "charters": [{"number": 1, ...}, {"number": 2, ...}],
    "templates": [{"number": 1, ...}, {"number": 2, ...}],
    "hosts": {
      "workers": [{"name": "worker", "digest": "worker.digest"}],
      "listener": [{"in_process": "workers"}]
    },
    "llm": ...,
    "clients": {"link": true, ...},
    "connectors": [1],
    "limits": {"profile": "small", "overrides": {"authority.segments": 8, ...}},
    "memory_bytes": ...
  },
  "workers": [{
    "engine": {"in_process": "workers"},
    "name": "worker",
    "secret": "worker.secret",
    "slots": ..., "graces": ...,
    "programs": {"check_interval_ms": ...},
    "agent": {
      "program": "/usr/local/bin/temper",
      "configuration": "/home/me/.config/temper/agent.json",
      "arguments": ["agent", "/home/me/.config/temper/agent.json"]
    },
    "limits": {"profile": "small", "overrides": {"hub.event_bytes": ..., ...}},
    "memory_bytes": ...
  }],
  "application": { ... as above ... }
}
```

Its `engine.limits` names the overrides section 7 asks, and its worker's
`limits` those the worker's check against the agent asks (section 7).
The owned example, `docs/examples/owned.json`, is a whole document for
one owned forge, with its own values (setup.md, 8.1).

## 11. Open questions

- **A calendar for periods.** The smallest source opens one period per
  configured project at the first start and none after, so a project's
  budget is not renewed. Default: the smallest source; a calendar child
  of temper's root, as `ops`'s (jig's `protocol/configuration.md`,
  section 5; jig's `examples.md`, section 4), would renew it.
- **A made project's own landing rules.** `landing.projects` names
  seeded projects, so a project made after the first start has the
  deployment's rules alone. Default: the deployment's. Designed in
  section 13, not built: a project's rules are a setting its owners
  change, kind 2, for any project, made or seeded.
- **A forge removed while the store holds its repositories.** Default:
  the start is refused at the forge's restart step, naming the forge
  number (jig's `protocol/configuration.md`, section 9, removed parts).
- **A project's landing rules changed after its first start.** They
  reach the forge's judges at the next start, but the project's policy
  keeps the requirements seeded at its first start. Default: a change
  reaches the policy only through a policy change by a person. Designed
  in section 13, not built: the rules leave the policies for two
  standing requirements the forge judges over the rules it holds, so a
  change reaches the next merge.
- **A charter an application's setting names** (13.2). The forge's
  charter is a number in bytes jig never reads, so jig's check of
  references cannot hold it, and a change that removes that charter is
  accepted. Default, asked of jig in the configuring plan's mission 2
  (jig's hook for an application's kinds): an application's kind may
  declare that its values name a charter, beside the bytes, and jig's
  one check holds the reference as it holds a template's. Until then,
  the forge's next change is held, saying its charter is not offered.
- **Requirements an application fixes** (13.3). The landing rules'
  two requirements are in jig's rules, which the deployment's owners may
  change, so landing can be left unenforced by a change of rules.
  Default: changeable, and said plainly on the review page; jig could
  let an application mark requirements no change removes.
- **A charter's documents** (`../draft/configuring.md`, section 2: rules
  reach the runs). A charter of jig's is its instructions as text, with
  no documents (jig's `domain/engine.md`, 17.1), so a chat, which has
  no workspace, cannot read the rules a repository holds. Default,
  asked of jig in the configuring plan's mission 3, with the configuring
  agent: a charter may name documents, each a repository, a path
  and a branch, carried as references jig never reads and pinned with
  the charter; the forge reads each at its branch's head into the
  run's brief, as a section of its own (`../domain/agent.md`, 4.1).
  Until then the configuring agent puts a rule in the charter's own
  instructions, as the draft does for a rule typed in the chat, and
  says that a change to the repository's text will not reach it.
- **A repository renamed or moved after adoption.** Default: its
  resource path, which landing patterns match, and its remote keep the
  owner and name of its adoption; Forgejo redirects the old path, and
  git follows the redirect (skein's conformance, `fake-forge.md`,
  section 7: `moved-repository-is-a-redirect`;
  `tools/git-transcripts/facts.txt`,
  `git-follows-a-renamed-repository-redirect`).
- **Profiles' numbers,** from the first deployments' measurements
  (jig's `protocol/configuration.md`, section 9). Default: `small` as
  written.
- **`small`'s worst cases against a person's machine.** With the owned
  example's strings at their bounds, the engine's and the worker's worst
  cases are past any machine's memory (setup.md, 8.1 and section 16), so
  the start's check (section 7, one worst case per service) bounds
  nothing a person's machine holds. Which terms make them so large is
  not known. Default: the check as it stands.
- **A configuration's format version** (jig's `protocol/configuration.md`,
  section 9). Default: none; unknown members refuse the start.
- **A configuration directory moved or copied.** The agent's paths are
  absolute (section 5), so they still name the old place: the agent's
  configuration is read there, or the start is refused, naming the
  member, when it is gone. Default: the person edits the two members;
  standalone could derive them from its own binary and its
  configuration directory, as no check can tell temper's binary from
  another program today (section 2).
- **The rest of an assignment's transcript and of a person's read.**
  The turn's check (section 7) holds the resume window's share of each
  hub's `transcript_bytes`, and the turn's share of
  `root.loads.read_bytes`. The answered calls an assignment carries
  beside its window (`core.call_records`, each within
  `core.call_answer_bytes`), and the owner's task record and ancestors
  a read carries with its row, have no bound the start can name: jig
  gives neither's worst case. Default: the hub refuses an assignment
  past its bound at run time, and the documents leave room beside the
  turn for the read's evidence (setup.md, 8.1).
- **A turn past what temper's client takes.** The engine carries a
  turn of smith's largest size whole, but temper's clients take a
  document within their own bound, jig-client's `document_bytes` as
  each shell sets it (`../client/README.md`, section 2), which a turn
  of the agent's largest size with its wrapper can pass. Default: such a
  turn, which no plan's model reaches in ordinary use, shows as too
  large to read (jig's `protocol/store.md`, 7.3).

## 12. The three directories

Every command that reads the document takes three directories, by
skein's arguments' rule (skein's `shell.md`, 6.5): a flag, else its
variable, else its default.

| Directory | Flag, variable | Default | Holds | Kept |
|---|---|---|---|---|
| configuration | `--config DIR`, `TEMPER_CONFIG` | `$XDG_CONFIG_HOME/temper` | the document, `temper.json`; smith's agent configuration, `agent.json`; the charters' instructions, until section 13 moves them into the binary; trusted roots' files | as a person's own files are: copied, synced or published, so it holds no secret |
| data | `--data DIR`, `TEMPER_DATA` | `$XDG_DATA_HOME/temper` | the deployment's directory, with the store, transcripts and jig's secrets' store; the worker's secret and its digest; the worker's root; each set-up forge's directory (setup.md, section 3); the demo's key and its git's link, in `demo/` (setup.md, 8.4); the box's records of the forges it has started, in `box/started/` (setup.md, 7.1); the box's logs of its last two runs, in `box/logs/` and `box/logs.previous/` (setup.md, 7.2); the programs directory | the deployment: backed up whole |
| cache | `--cache DIR`, `TEMPER_CACHE` | `$XDG_CACHE_HOME/temper` | binaries fetched again at will: an owned forge's Forgejo and runner, `temper forge fetch`'s in `forgejo/VERSION/` and `forgejo-runner/RUNNER_VERSION/` (setup.md, 6.6) | never backed up |

- **The document is `temper.json` in the configuration directory,**
  always: no command takes a document's path. A command that finds none
  there is refused, naming the path, and `temper init`, which writes one
  (setup.md, 8.2); `temper box` alone, on an empty configuration
  directory, runs `temper init` first (setup.md, 7.1).
- **Resolved once, at start,** before the document is read: each
  directory, from its flag, variable or default, is made absolute
  against the working directory, by joining names, and every parser is
  given the three (section 2; jig's `protocol/configuration.md`, section
  10). Nothing later is resolved against the working directory.
- **Checked at start:** the data and cache directories, and their
  missing parents, are made private (0o700) when absent; an existing one
  must be the process's user's, private, and not reached through a link
  at its name, or the start is refused, naming it (skein's `io.md`,
  5.3). A linked parent is accepted, as the programs directory's is
  (section 5). The configuration directory must hold the document; only
  `temper init` makes it.
- **Each path member's kind:**
  - *configuration paths:* jig's charters' instructions and trusted
    roots' files, and each forge's `transport.tls.trust` (section 3); the
    worker's agent's `configuration` is one in jig's parts, which
    standalone requires absolute (below);
  - *data paths:* jig's `engine.deployment.directory`, each of
    `engine.hosts.workers`' `digest` and the worker's `secret`; each of
    `application.workers`' `root` (section 4) and of
    `application.setup`'s `directory` and a fake forge's `git` (setup.md,
    section 8); and `application.demo`'s `llm.key` (setup.md, 8.4);
  - *cache paths:* an owned forge's `forgejo` and `runner.binary`, or
    `"fetched"` for `temper forge fetch`'s (setup.md, section 8 and 6.6);
  - *absolute paths,* refused when relative: each of
    `application.workers`' `git` but a null one,
    `application.clients.bundle`, and the agent's `program`,
    `configuration` and `arguments` (section 5), which are installed
    programs and files, or the spawn's own; every path in smith's agent
    configuration, which is smith's format.
- **No secret in the configuration directory:** a worker's secret and
  its digest, an owned or fake forge's handover, every set-up forge's
  file and the demo model's key are data paths, and jig's secrets' store is in the deployment's directory.
- **The box passes its three on** to each step, as flags, each absolute
  (setup.md, 7.1).
- **Each of the cluster's processes has its own three** (README, 15.1):
  an engine alone's data holds the deployment's directory and each
  worker's digest; a worker alone's, its secret and its root. No two
  processes share a data or cache directory.
- **Tests give all three,** each a directory of the test's own: no test
  resolves a default under `$HOME` (`../domain/testing.md`).

### 12.1 The fake-backend sandbox

temper box --fake-backends prepares effective configuration for the
welcome's local peers (setup.md, 7.5). The flag belongs to configuration
preparation and box supervision alone; the client, service and engine use
their ordinary configuration and paths.

For caller directories C, D and K, its private sandbox is
E = D/box/fake-backends, with configuration C' = E/config, data
D' = E/data and cache K. All child commands receive C', D' and K,
including the worker's agent configuration and its argument. No child is
given the original deployment's data. Original configuration and records
are never overwritten; no credential, account, worker secret or program
login is imported.

- **First preparation:** absent or empty C runs ordinary init in
  C'/D'/K', retaining init's listeners. Give --bundle DIR to name the
  separately built loopback bundle; C stays untouched. A nonempty C is
  accepted only for the current init-shaped standalone configuration,
  its public agent document and two public charters. --bundle overrides
  its application.clients.bundle; otherwise that configured directory is
  used. Unsupported shapes are refused with the member named before any
  child starts. Fresh worker/deployment resources are created in D'.
  Source paths resolve against C before public assets are copied; data
  paths are rebased to D', never followed into the source store.
- **Lifetime:** the box holds an exclusive nonblocking file lock on
  E/lock. It publishes E/prepared last, after effective documents and
  resources pass the ordinary deployment and agent checks. The marker
  identifies source directory names and public-document digests. Partial
  preparation, incompatible source identity or changed source documents
  is refused with instructions to reset the sandbox.
- **Restart:** effective configuration and data survive box shutdown.
  Reuse keeps edits to C' and the state in D', so an ordinary managed
  Forgejo configuration/restart uses the same mechanism as elsewhere.
  An explicit --bundle changes only the retained effective bundle selection,
  after validating it; other effective edits and data remain. Without that
  override, the retained bundle is authoritative. The box prints these
  effective directories. Compatible v1 peer documents are revalidated and
  retained; the seeded output files follow setup.md, 8.4. Peer in-memory
  state lasts one launch, independently of retained configuration and data.
  A later required document or adapter schema that cannot safely reuse an
  older sandbox is refused by name with explicit reset instructions;
  startup never migrates or deletes E or silently omits a required peer.
  Removing E resets this scratch deployment; a
  normal start never reads it as the real one.
- **Roots:** GitHub forge 2's API, web, git and transport name one
  plaintext loopback fake. Provider 1 keeps its shipped client, scopes,
  must-grant, resource and callback; its five sign-in origins name the
  local issuer. The openai endpoint in both effective documents names
  the same plaintext fake model, with stale TLS members removed. The
  pure transformation is temper-fake's, with bounded bytes and explicit
  paths/sockets as inputs; files and listeners belong to temper's shell.
  Its rewrite table holds the init-shaped git/program edits plus all four
  removable TLS members on each of forge, sign-in and model endpoint: 42
  slots. The agent document needs eight of the same bounded slots.
- **Browser:** --bundle DIR, or source application.clients.bundle,
  names a provisioned loopback-navigation bundle (../client/browser.md,
  2.6). Build it once with scripts/bundle.sh --loopback DIR, then run
  temper box --fake-backends --bundle DIR. Refuse before sandbox mutation
  only when no bundle is known or its shape/types/size are invalid. The
  refusal names that bundle command and --bundle; no hand-edited setting
  is required. The ordinary embedded bundle cannot replace it. Startup
  neither builds a bundle nor accesses a remote service.


Before starting its supervised processes, the box prints these fixed
prefixes on stderr, for people and lab readiness captures. Addresses are
loopback IPs with ports; each origin belongs to a peer the box runs, so
there are no per-peer commands to run:

```text
box: effective configuration C'
box: effective data D'
box: effective cache K
box: fake github http://ADDR
box: fake issuer http://ADDR
box: fake model http://ADDR
box: fake forgejo http://ADDR
box: model key FILE
box: Forgejo elsewhere admin token file FILE
box: program login: COMMAND
box: GitHub own App id 1, key file FILE
```

The model key line names the effective `llm.json`'s `key` file, which a
person pastes on the Model step's key way. Key and token lines print
paths only. The program-login command names the effective directories
and ends `NAME (the name the Model step shows)`: the person substitutes
the name declared by the welcome's Model step. The own-App line names
App 1's matching private fixture PEM.


The effective configuration keeps forge 1 as the playground, forge 2 as
GitHub and forge 3 as the owned Forgejo. Forge 4 is the ordinary managed
Forgejo elsewhere: its API and git roots name a separate loopback
fake. Its web member is null, as for ordinary Forgejo (section 3);
readiness prints the fake peer's origin separately. Its retained peer document is C'/forgejo.json, and its dummy admin
token is D'/forgejo-admin.token, private with mode 0600. Readiness names
only that file's path; its contents belong in no log, report or record.
The sandbox's own configured counts admit all four forges and their four
identities; these explicit local overrides are checked through ordinary
admission and worst-case sizing. Repository defaults, policy bounds and
time budgets remain the ordinary ones. The managed entry carries the
ordinary admin-connect contract, not an automatic account login or a
product fake branch. A retained sandbox
missing this required document refuses by name with reset instructions;
its marker stays v1 and no file is silently generated on reuse.

Forgejo here keeps the ordinary owned steps and cache (setup.md, 6.6);
an empty cache may need its ordinary one-time pinned download, while a
disconnected walk uses a populated cache. Forgejo elsewhere is an
ordinary managed entry backed by its separate fake. Program login is the
ordinary command with its configured fake adapter, not an automatic login.
The fake program way uses the pinned `fake-claude` executable provisioned
beside `temper`, resolved to that exact absolute sibling path, and its
private adapter directory D'/programs. Preparation supplies that path as
init's program fact; source import checks an existing adapter's bounded
absolute shape without looking for Claude on PATH. The pure rewrite writes
the exact sibling and private directory even when the source has no adapter,
and an ordinary Anthropic provider (number 2, `signed_in_plan: false`,
with its required `manage_usage` address `https://claude.ai/settings/usage`)
offers that adapter and the `fake-claude` model. Init ships no Claude
provider, so the rewrite adds it with only the program source; both
program endpoints name `anthropic` with its dialect 2, while OpenAI keeps
its key and ChatGPT sign-in sources. The fake overrides `documents.lines` to 3 so
OpenAI, demo and Anthropic fit in the Model step’s provider offers.
The view therefore shows “Use your Claude subscription” and “Declared your Claude subscription”. No startup
path discovers or falls back to a real `claude` from PATH.

Reuse requires agent.json/programs/claude_code/path to equal that sibling.
A different retained path refuses with that member named and explicit reset
instructions; no credential or login state is imported. A correct path with
a missing or nonexecutable sidecar instead refuses with the executable path
and the provisioning command `cargo build -p temper-fake-claude`, without
asking for a reset. Startup builds or fetches nothing. A walk that needs a
fake program story provisions it outside startup, then uses ordinary
program-login; the product links no fake-adapter story machinery.

The required retained documents are temper.json, agent.json, github.json,
oauth.json, llm.json and forgejo.json. One validation path checks every
required file before any child starts. A missing required file refuses by
name with reset instructions, without generating a replacement on reuse.

## 13. Settings in the store

Designed, not built: mission 2 of the configuring plan builds it
(`../STATUS.md`). How work is run leaves the document for the store,
by the rule of `../draft/configuring.md`, 3.1: the document keeps
what a start needs before it can read its store, and what belongs to
the machine. The mechanism is jig's (jig's `domain/engine.md`, section
17; `domain/connectors.md`, section 17): a setting is a kind, a scope
and a number; the current settings are the binary's defaults with the
stored changes over them; a change is one decision, checked whole and
refused whole; charters are pinned per revision, everything else is
read at its newest. This section says what temper adds: its own kinds,
how the forge judges them, its stock defaults, and the members that go.
The pages a person meets it on are `../client/README.md`, 5.13.

### 13.1 What leaves the document

| Member today | Becomes | Its default (13.4) |
|---|---|---|
| jig's `engine.charters` | jig's charters | temper's stock charters: a chat's (1), a coding agent's (2) and the configuring agent's (3) |
| jig's `engine.templates` | jig's templates | jig's chat (1) and goal (2), and temper's configuring chat (3) |
| jig's `engine.policies` | jig's policies projects are made from | the stock policy (1) |
| jig's `engine.rules` | jig's rules | temper's stock rules, the landing's two requirements among them (13.3) |
| `application.landing` | temper's kinds 1 and 2: the deployment's landing rules, and each project's (13.2) | one deployment rule: CI passed and up to date, everywhere |
| `application.charter` | temper's kind 3: the forge's charter (13.2) | charter 2 |

- **What stays** is section 3's other members and jig's other parts:
  the directories, the listeners, the workers and their secrets' files,
  the limits, the first owners and the sign-in providers, the forges,
  the identities, `clients`, `sign_in`, `webhooks`, `setup` and `demo`,
  and, for now, the LLM providers (`engine.llm`), each its own design
  later (`../draft/configuring.md`, section 10). jig's `engine.projects`,
  the projects a first start seeds, stays jig's member.
- **A project's own policy** is not a setting: it is in the store
  already, and its owners change it (jig's `domain/people.md`, 5.2).
  What a project adds to how its changes land is its landing rules, a
  setting of kind 2, decided by its owners.

### 13.2 temper's kinds

temper declares three kinds of its own beside jig's, each owned by the
forge's connector, which judges them (jig's `domain/engine.md`, 17.5):

| Kind | Scope | Numbers | Value | Decided by |
|---|---|---|---|---|
| 1, the deployment's landing rules | the deployment | a rule's, from 1; at most `forge.landing_rules` | a landing rule (below) | the deployment's owners |
| 2, a project's landing rules | each project | a rule's, from 1; at most `forge.landing_rules` per project | a landing rule | that project's owners |
| 3, the forge's charter | the deployment | 1 alone | a charter's number, a u32 | the deployment's owners |

- **A landing rule's value** is a `LandingRule` record in temper's
  codec, section 3's rule member for member but `connector` and `kind`,
  which are always the forge's and its merge: `pattern`, `enforce`,
  `ci`, `up_to_date`, `gates` and `approvals`, each as section 3 says.
  Its largest bytes, the kind's bound, are its encoding with a pattern
  at jig's pattern bounds, `forge.change_policy.gates` gates and
  `authority.roles` approvals; `forge.landing_rules`, a limit of the
  forge's (section 7), is 4 in `small`, as jig's requirements per scope
  bound the rules today.
- **Judged by the forge, in the change's decision.** The root hands the
  forge each item of its kinds with the value it would leave (jig's
  `domain/connectors.md`, section 17); the forge reads only its own
  state, and refuses an item, naming it by kind, scope and number and
  saying what fails:
  - *a rule* that does not decode; whose pattern's first segment is not
    `forge`, or passes jig's pattern bounds; with more gates than
    `forge.change_policy.gates`, or a gate's number twice; with an
    approval of no people, or a role twice, or one past
    `authority.roles`;
  - *the forge's charter* outside 1 to jig's `tasks.charters`.
  Whether a project's policy has an approval's role is not the forge's
  to know: a rule naming a role nobody holds is accepted, and the
  review page says so (`../client/README.md`, 5.13); a change it covers
  waits, as one whose approvers have not yet approved. Whether the
  forge's charter names a charter the change leaves offered is jig's
  check of references, once jig's kinds may name a charter (section
  11); until then a change of charters that would leave it unoffered is
  accepted, and the forge's next change is held, saying so.
- **Told in the same decision.** Once a change is committed, the forge
  hears its kinds' new values and replaces its rules, the deployment's
  or the project's, whole; each live change into a repository it serves
  is stepped after the commit, so a gate a rule added is asked for and
  one it dropped no longer waited on (`../domain/forge.md`, 8.3). At a
  start the forge hears the current values before its records
  (`../domain/forge.md`, section 16).
- **Never pinned.** Every merge is judged by the rules as they are
  then, whatever revision its change was made in: what work may do is
  always current (`../draft/configuring.md`, section 4).
- **The forge's charter is read when a change is made.** The root reads
  it through the core, by kind, scope and number, as it reads a charter
  (jig's `domain/engine.md`, 17.5), and grants the change's procedure
  delegation to that charter. The change keeps it: its producing, its
  repairs, its resolutions and its agents' and checks' gates run under
  the charter it was made with, and a newer forge's charter reaches the
  changes made after it (`../domain/root.md`, section 6).

### 13.3 How the landing rules are enforced

- **Two standing requirements** in temper's stock rules, on the forge's
  merge (kind 4) over `{"segments": ["forge"], "last": "open", "value":
  ""}`, every landing branch of every forge: the forge's requirement 5,
  *the landing rules*, guarded and marked as one that must be guarded;
  and its requirement 6, *up to date by the landing rules*, observed
  with a freshness of 0. Their parameters are 0. They replace the
  requirements 1 to 4 that each rule's criteria became, one per
  criterion, in the rules and in each seeded project's policy
  (`../domain/forge.md`, 8.7).
- **Judged over the rules the forge holds then.** At a merge, the forge
  takes every rule covering its branch that enforces, the deployment's
  and the project's: requirement 5 meets when each one's CI, gates and
  approvals are met, waits when one waits and none refuses, and refuses
  when one refuses; requirement 6 likewise for each one's up to date. A
  branch no enforcing rule covers meets both. The forge keeps on the
  change the rule and the criterion it last waited on or refused, which
  the change's panel shows and its hold says (`../domain/forge.md`,
  8.7): the words are the forge's, kept with its change, whatever a
  verdict carries through jig.
- **So a change of landing rules touches nothing of jig's:** no rule,
  no project's policy. A project made after the first start is covered
  by the deployment's rules at once, and by its own once its owners
  set them; a project's rules changed reach its next merge.
- **jig's rules are the deployment owners' to change,** the two
  requirements among them. A change that removes one leaves landing to
  the change procedure's waits alone; the review page says so under
  "temper enforces" (`../client/README.md`, 5.13).

### 13.4 temper's stock defaults

The binary holds them, and gives them to jig's builder as `Config`'s
defaults at each start (jig's `domain/engine.md`, 17.1): the values
`temper init` writes into the document today (`setup.md`, 8.3), moved
into the binary.

| Setting | Its value |
|---|---|
| charter 1, a chat's | the instructions of today's `charters/chat.txt`; models: every model `engine.llm` configures, in the document's order, the first its default; hosts `workers`; waiting 10 s; resume; a run of 8 turns and 120 s, a reserve of 1 turn and 60 s; a default spend of 20; the inspect, modify and agents tool families, no shell; a call's timeout 10 s |
| charter 2, a coding agent's | as charter 1, with today's `charters/coding.txt` |
| template 1, a chat | jig's chat, run by charter 1, as `temper init` writes it today |
| charter 3, the configuring agent's | as charter 1, with new instructions held in the binary (`../draft/configuring.md`, sections 2 and 5.2: split what temper enforces from what agents follow, offer a rule as structure where temper has it, ask until it can propose); the inspect family and jig's `settings` family (jig's `domain/engine.md`, 7.3), no modify, no agents, no shell |
| template 2, a goal | jig's goal, its executor chosen |
| template 3, the configuring chat | a chat, flagged conversation and labelled as jig's chat is, run by charter 3 |
| policy 1 | today's seeded project's policy, with no requirements: role 0 requests and decides everything, a period's spend of 100 for the project and for role 0, an authority's spend of 40, with the rules' grants and delegation |
| the rules | today's: tools, the forge's grants under `forge`, delegation to charters 1, 2 and 3 and the change procedure, 8 tasks 4 deep, a spend of 100, a run's spend from 1 to 10; and 13.3's two requirements |
| kind 1, rule 1 | everywhere (`forge`, open), enforced: CI passed, up to date; no gates, no approvals |
| kind 3 | charter 2 |

- **The configuring agent is a setting** like any other (`../draft/configuring.md`, 5.1): "Tell temper how you work" starts template 3,
  which an owner may change or remove as any template. Its runs read
  and propose settings; they never change one, since jig's `settings`
  family proposes and an owner accepts (jig's `domain/engine.md`, 17.6).
- **Built at each start** by `temper-service`'s `stock` module, as
  typed values, the instructions held in the binary; `Service` passes
  them, with the three kinds' declarations (number, scope, the forge's
  connector, the bounds above), to jig's builder in place of the
  document's members, and makes the engine as today.
- **The stock charters' models are the document's:** a charter names
  configured models, and the binary knows none, so it gives every model
  `engine.llm` configures, the demo's among them where the demo is
  configured. A document whose models change between starts gives new
  defaults, which make a revision recorded as the release's (jig's
  `domain/engine.md`, 17.7).
- **A release that changes a default** reaches every deployment that
  kept it, at its next start, by a revision recorded as the release's,
  and its release notes say so (`../draft/configuring.md`, 3.2).

### 13.5 The members gone

- **Refused.** temper passes its defaults, so `jig-protocol-config`
  reads no `engine.charters`, `templates`, `policies` or `rules` (jig's
  bridge, `domain/engine.md`, 17.7, serves an application that passes
  none), and temper reads no `application.landing` or `charter`. A
  document naming one is refused as any unknown member is (section 1),
  naming its path and saying where it went: "settings live in the
  store: How work runs". Section 3's `landing` and `charter`, and
  `Config`'s `landing` and `forge_charter` fields, go with them.
- **Before the baseline** (`../draft/upgrades.md`, 4.1), when mission 2
  is planned to land: the settings change the store's families, so a
  deployment is made again, and nothing is imported. A document `temper
  init` wrote carried the stock values, now the binary's; one an
  operator edited is told again, after the start, as a proposal ("Tell
  temper how you work").
- **From the baseline,** were the members to go after it
  (`../draft/upgrades.md`, section 10): the release that removes them
  accepts each for one release with a warning naming its setting, and
  at that start takes its values as stored changes where the store has
  none, in a revision recorded as the release's and as from the
  document, so nothing configured is lost; the next release refuses it.
- **Across jig and temper:** jig reads the four members while temper
  passes no defaults (the bridge); temper's repin onto the jig that
  takes defaults and application kinds lands with temper's side whole
  (the `stock` module, the kinds, the members refused, `temper init`'s
  document shortened), and jig's bridge goes after.
- **Who writes them today, and loses them:** `temper init`'s document
  and its `charters/` files (`setup.md`, 8.2 and 8.3); the box, which
  has no document of its own but `temper init`'s (`setup.md`, section
  7); `docs/examples/owned.json` and its instructions; the lab's engine
  documents; the tests' documents and builders (`../domain/testing.md`,
  6.4). A generated document elsewhere, fabrig's, drops the six members;
  what it set differently from the stock values becomes a proposal after
  the deployment's first start.
