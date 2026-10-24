# Credentials

Provisional, 2026-10-03, revised 2026-10-24. temper's credentials: which
secrets temper's processes hold, where each comes from, who may use it,
and how it reaches the process that does. LLM accounts, workers' secrets
and people's sign-ins are jig's (jig's `protocol/README.md`, section 8;
jig's `domain/accounts.md`; jig's `protocol/llm.md`, sections 5 and 6),
and temper adds `temper program-login` (section 3); temper's own are its
forges' credentials, records in jig's secrets' store under temper's tag
(jig's `protocol/store.md`, section 9), and the git identities made from
them. A forge's credentials come from the client, in a request carrying
a value (jig's `protocol/client.md`, section 5), or from the engine's
setup host, never from configuration (configuration.md, section 6); a
git identity's value on the channel is channel.md's (section 6); git's
use of it is git.md's (section 5); the forge's setup's own secrets are
setup.md's (section 9). What is open is listed in section 8.

## 1. In one page

- **Names through the domains, values beside them.** A domain names a
  credential by its number and a generation, and never holds its value,
  so no trace, snapshot, record or log of a domain can leak one. Values
  live in the protocol layer's tables, filled in as a record is encoded
  and taken out as it is decoded.
- **LLM accounts are jig's,** made from the client: a sign-in with the
  LLM provider, a key, or a program's login. temper writes no OAuth
  exchange and configures no account, and adds `temper program-login`,
  which signs a program's login in on a host (section 3).
- **Every forge credential is a record in jig's secrets' store,** under
  temper's tag, one row per forge and item: the forge's tokens, a GitHub
  App's id, slug and key, the bot's user, a Forgejo forge's webhooks'
  secret, the accepted installations (section 2.1). A connect from the
  client makes them, from the values pasted in its request or through
  the engine's setup host; nothing configures them, and `temper serve`
  reads no forge credential from a file but an owned Forgejo's handover,
  once. The main store keeps each forge's writer, which is not secret.
- **Kept before used, read once at start.** A new value is durable in the
  store before the forge component or a worker gets it; one load at start
  builds every forge's access before the forge component starts.
- **A pasted value never reaches a domain.** jig keeps a request's
  carried value under temper's tag before the core sees the request, and
  the core gets its number and its digest only (section 2.2).
- **A forge's access is the forge component's:** Forgejo's API token and
  its webhooks' secret, or a GitHub App's id and key and the
  installations' tokens it mints, with the forge's user; never granted,
  never sent to another forge or host (section 5). GitHub has no
  webhooks' secret: its changes are read from the App's delivery log.
- **Git identities are temper's application credentials,** one per
  forge: Forgejo's kept git token, or a GitHub App's installations'
  tokens, minted about hourly and never kept; each with a login and an
  author. Each change of value is a new generation, granted with each
  assignment that needs one (section 4).
- **Git gets its credentials per invocation,** in that process's
  environment only, never in an argument, a URL or a file.
- **Least power.** The forge component never holds an admin's token: the
  engine's setup host holds a managed or owned Forgejo's only while a
  plan runs, for reconcile, for access at an adoption and for the
  picker's reads (a `Reachable` plan), and Forget erases it, making the
  forge connected. The picker's read of any other forge takes its API
  token, or on GitHub a token minted for the page, held while the plan
  runs and dropped. A worker gets the git identity alone, never the
  forge's access.
- **A forge's credential failing stops that forge, never the engine.** A
  credential the forge refuses, or records that do not fit the configured
  forge, leave that forge stopped or not connected, named, with Replace
  or Connect offered; other forges and the engine run on (section 5).
- **Workers keep nothing between runs.** A worker's configuration holds
  no credential but its own secret; every other comes with assignments.

## 2. The credentials

| Credential | Owner | Kind | Source | Held by | Used by | Changes |
|---|---|---|---|---|---|---|
| an LLM account's source | jig | a signed-in record (refresh token, access token, expiry), a key, or a program's login | a sign-in or a key from the client; a program's login, `temper program-login` on each host (section 3) | jig's secrets' store, lent by skein's accounts driver, the access tokens and keys in jig's accounts' table; a program's login, the program, in the source's configuration directory | the worker's agent processes, through grants | the driver refreshes a signed-in record; a key's holder replaces it from the client; the program renews its own login |
| a git identity | temper | a token with its login, and an author: on Forgejo the `GitToken` record; on GitHub the accepted installations' tokens | the forge's records and tokens, and the identity's members (section 4) | the identities' table on the engine, and on the worker from grants | the git component's invocations | a new generation at each change of value (section 4) |
| a forge's API token | temper | a Forgejo token, the `ApiToken` record | pasted from the client at a connect, or made by the setup host's reconcile when the forge is managed or owned (setup.md, section 5) | jig's secrets' store; the forge component, in the forge's access | that forge's API requests | replaced from the client (section 5), or made again by reconcile when it does not answer as the bot |
| a GitHub App's key | temper | an RSA private key, PKCS#1 DER, 2048 bits, the `AppKey` record beside `AppId` | the manifest's conversion; or pasted from the client as GitHub gives it, PEM, which the setup host decodes to DER | jig's secrets' store; the forge component, in the forge's access; the setup host, while a plan runs | signing the App's own requests, and minting its installations' tokens | replaced from the client with a key of the same App (section 5) |
| an installation's token | temper | a GitHub token, expiring within the hour | minted by the forge component with the App's key, one per accepted installation (section 5); by the setup host, for a page of the picker | the forge component; the forge's git identity; the setup host, while a page is read; never kept | that installation's API requests; git's, through the identity; the picker's page | minted again `renew_before` ahead of its expiry; a picker's page's, dropped with the page |
| a worker's secret | jig | a shared secret | the worker's secret file; the engine's digest file | the worker; the engine, its digest | the link's opening | an operator replaces both |
| a sign-in's secret | jig | a bearer token | the printed link, then the sign-in | `jig-client` in `temper open`, which keeps it in its directory; the engine, its digest | each of the client's exchanges and its stream | the sign-in ends or expires |
| a Forgejo forge's webhooks' secret | temper | a shared secret, the `WebhookSecret` record; a GitHub forge has none | pasted at a connected Forgejo's connect; or drawn by the setup host and set on a managed or owned Forgejo's hooks by reconcile | jig's secrets' store; the forge component's webhooks' listener, on loopback; the forge's hooks, which sign each delivery with it | checking each delivery's signature (forge.md, section 5) | replaced from the client with the forge's tokens; reconcile sets a managed or owned Forgejo's hooks' secret to the record's |
| a site admin's token | temper | a Forgejo token, the `AdminToken` record | pasted from the client at a managed connect; an owned Forgejo's handover file, taken once and removed (setup.md, section 9) | jig's secrets' store; the setup host, only while a plan runs | reconcile, access at an adoption, and the picker's reads on a managed or owned Forgejo (a `Reachable` plan) | erased by Forget, after which the forge is connected |

- **Every forge credential the engine uses is in jig's secrets' store,**
  under temper's tag (`SecretOwner::Application`), beside jig's own:
  sign-ins' and services' digests, LLM accounts' records and keys (jig's
  `protocol/store.md`, section 9). No forge credential is a file
  `temper serve` reads, but an owned Forgejo's handover admin's token,
  read once, kept as a record, and removed.
- **A GitHub forge has no webhooks' secret.** The setup host points the
  App's hook at `https://<slug>.invalid/`, a name that never resolves; the
  conversion's webhook secret is never kept, and a pasted App's connect
  draws none. The forge component reads changes from the App's delivery
  log (forge.md, a GitHub App's delivery log).
- **The forge's setup's own secrets** (the bot's and temper's admin's
  passwords, the person's first password on an owned Forgejo, the
  runner's secret, Forgejo's own secrets) are setup.md's (section 9):
  kept in the forge's directory or nowhere, never in jig's secrets'
  store.

### 2.1 The records

- **Where:** jig's secrets' store, under temper's tag,
  `SecretOwner::Application`, one row per forge and item, written and
  read by the engine's protocol layer alone, through `Request::Secrets`
  (`Keep`, `Erase`, `Get`, `Load`). The items that are not secret (ids,
  the App's slug, the bot's user, the accepted installations, the
  origin) sit beside the secret ones, so that one load builds a forge's
  access, one durability answer (`Kept`) covers each of the setup host's
  writes, and one erase forgets a forge. The main store's application
  rows are the root's, and hold none of them.
- **The number** (`u64`): `family << 48 | forge << 16 | item`. Family 1 is
  a forge's records; other families, up to `0x7FFF`, are free for
  temper's later records. temper's own numbers keep the top bit clear,
  below 2^63, where jig numbers carried values under temper's tag itself
  (section 2.2). `forge` is the
  configured forge's `number` (`u16`; configuration.md, section 3), its
  explicit, stable id: reordering, adding or removing forges never moves
  one forge's records to another, and a forge given another number is a
  new forge, not connected.
- **The items** (`item`, `u16`):

  | Code | Item | Value | Forges |
  |---|---|---|---|
  | 1 | `AdminToken` | a site admin's token | Forgejo, managed or owned |
  | 2 | `ApiToken` | the bot's API token | Forgejo |
  | 3 | `GitToken` | the bot's git token | Forgejo |
  | 4 | `User` | the bot's user id: on Forgejo the bot's, on GitHub the App's bot user | both |
  | 5 | `WebhookSecret` | the hooks' shared secret | Forgejo alone; optional on a connected Forgejo |
  | 6 | `AppId` | the App's id | GitHub |
  | 7 | `AppSlug` | the App's slug | GitHub |
  | 8 | `AppKey` | the App's private key, PKCS#1 DER, 2048 bits | GitHub |
  | 9 | `Accepted` | the accepted installations' ids, at most `forges.installations` | GitHub |
  | 10 | `Origin` | the forge's kind and `api` address the records were made for | both |
  | 11 | `Pasted` | a pasted App key's PEM, in a pasted bundle alone (2.2), never a record: a row under 11 is unreadable | GitHub |

  Code 0 is never written, and a code never changes its meaning. temper
  maps skein's setup items to these codes; on GitHub, `User` is the
  App's bot user. `Origin` is temper's own, written at each connect.
- **The value:** a version byte, 1, then the item's bytes exactly as
  skein's setup host writes them, which skein's `forge-setup.md` (6.4)
  pins:
  - the tokens (`AdminToken`, `ApiToken`, `GitToken`, `WebhookSecret`):
    text, at most `token_bytes`;
  - `User` and `AppId`: a `u64`, little-endian, 8 bytes, never 0;
  - `AppSlug`: text, at most `name_bytes`;
  - `AppKey`: PKCS#1 DER, at most `key_bytes`;
  - `Accepted`: the installations' ids, each a `u64` little-endian,
    concatenated, ascending, distinct, at most `installations` of them,
    empty for none;
  - `Origin`, temper's: `[kind u8][api as text]`, the kind 1 for Forgejo
    and 2 for GitHub, and the `api` address as `<host>:<port>`: a socket
    address as Rust writes it (`[<ipv6>]:<port>` for IPv6), a host's
    name as configured, and `api.github.com:443` for GitHub's `null`.
    Equal bytes are the same forge; the text is shown as is.

  Each item has one total decoder, skein's (temper's for `Origin`); a
  change of an item's bytes is a new item, under a new code. temper keeps
  the values opaque, hands them back unchanged on the setup host's read,
  and decodes only what builds an access. A value is at most
  `records.value_bytes` (configuration.md, section 7), checked against
  jig's secrets' store's bound at start.
- **A PEM is never a record.** skein's setup plan decodes the
  conversion's key and a pasted key to DER before it writes; a pasted PEM
  sits only in its carried row, which jig numbered, until its run ends.
- **`Accepted`** is one row per GitHub forge, the whole list, rewritten
  at each installation accepted or deleted.
- **Replacement:** `Keep` under the same number replaces; the new value is
  durable before the forge component or a worker gets it.
- **Read at start, whole:** one `Load`, paged, grouped by forge, builds
  each configured forge's access before the forge component starts
  (forge.md, the forge component's start).
- **Records that do not fit the configured forge** (an `Origin` naming
  another kind or `api` address, an `AppKey` under a Forgejo forge, an
  `ApiToken` under a GitHub forge): that forge is not connected, named on
  the deployment's page, "its records were made for <kind> at <address>";
  nothing is erased, and connecting again overwrites them.
- **An unreadable row** (an unknown version, bytes skein's decoder
  refuses): that forge is not connected, named, "its records are
  unreadable"; other forges run on; nothing is erased.
- **An incomplete set** (a crash inside a conversion: an `AppId` kept, no
  `AppKey`): the forge is not connected; the next Connect overwrites it.
- **Rows of a forge not configured** are left as they are, and
  named once in the log at start (section 8).
- **A mode is not a record.** Owned is configuration's (`setup[]`'s owned
  entry); managed is an `AdminToken` row on a forge not owned; connected
  is neither. A GitHub App made from the manifest and one pasted end in
  the same rows.
- **Never kept:** installations' tokens; a GitHub forge's git tokens; the
  manifest's states and pages; the conversion's PEM; the App's OAuth
  client and its secret; any generation number.
- **The main store** keeps no credential. It keeps each forge's writer,
  which is not secret: the writer given at runtime (section 5), kept by
  skein's forge client as its own record (skein's `forge-client.md`,
  7.1), `Stored::Writer` under `Key::Writer(forge)`, temper's small
  store's tag 16, `STORE_VERSION` 3. It is restored at start, so effects
  restored with it never meet `Forbidden`.

### 2.2 Pasted values

- **One value per request.** A client request may carry one value (jig's
  `protocol/client.md`, section 5). jig's clients' component takes it out
  as it decodes the request, keeps it in the secrets' store under
  temper's tag before the core sees the request, and hands the core its
  number and its digest. The value never reaches the core, a root, a
  document or a log.
- **The requests that carry one** (`../client/README.md`, the FORGE
  requests): `Connect` with `ForgejoTokens` (one bundle: the API token,
  the git token, and the webhooks' secret when given), with
  `ForgejoAdmin` (the admin's token) and with `GitHubApp` (the App's key
  as GitHub gives it, PEM; the id rides in the request); and `Replace`
  (one token or key). A GitHub App has no webhooks' secret: a pasted
  App's connect writes `AppId`, `AppSlug`, `AppKey` and `User`, and sets
  the App's hook to `https://<slug>.invalid/` (section 2).
- **The number is jig's.** jig numbers each carried value itself, under
  temper's tag from 2^63 up, from a next number it keeps durably in a
  counter row of its own, which is not one of temper's rows and never
  reaches temper's `Load`, and never reuses. The request names no
  number and the client draws none; the core gets the number and the
  digest. A number already holding a row is refused at decode, never
  overwritten. A forge's records lie below 2^63 (section 2.1), so no
  request, refused or not, overwrites or erases them.
- **A carried value left unused** is named once: when the core leaves
  one unused, its request refused, replayed, joined to another or cut
  by a stop, it emits one output naming the number, and temper's root
  forwards it to jig's clients' component, which erases the row.
- **Only the core's decisions** replace or erase a kept value: a carried
  row's erase when it is left unused, at its run's end, and at start, is
  a decision of the core's; the routing carries out those at a run's end
  and at start.
- **The value:** temper's bundle, versioned: `[1][count u8]`, then per
  entry `[item u16][length u16][bytes]`, both `u16`s little-endian, the
  item one of 2.1's codes (`Pasted` for a pasted App key's PEM), at
  most 4 KiB in all, jig's bound on a carried value, and at most
  `records.value_bytes`.
  A pasted App's PEM key, under 2 KiB for GitHub's 2048-bit keys, fits.
  It lives until its run ends.
- **The core** keeps the number and the digest in the connect's own
  record, no secret, and checks the person's role, the forge's kind and
  mode, and that no run holds the forge. Refused, it answers so, and
  names the value left unused (above).
- **The run:** accepted, the root asks the routing to run the connect's
  plan with that number. The routing gets the row, checks its digest is
  the request's (else the run is refused, "the pasted value changed"),
  and serves the setup host's read of each pasted item from it before the
  forge's rows. When the run ends, done or failed, the core decides the
  row's erase, which the routing carries out; on done, the items the run
  wrote, and each pasted item but `Pasted` (the tokens and the secret
  the plan was given, which it keeps: skein's `forge-setup.md`, 4.6),
  are kept under the forge's family-1 numbers first, after the run's
  writes.
- **A restart** drops runs, whose states are in memory: the core decides
  the erase of each carried row under temper's tag found at start
  (loaded from 2^63), and the forge of a connect that was running reads
  as its records say: not connected when its paste wrote nothing, and
  nothing tells the person the connect was cut. Saying "interrupted;
  paste again" waits for a stored connect (section 8).

## 3. LLM accounts

jig's, as jig has them (jig's `domain/accounts.md`): each an LLM
provider's account, held by a person or by the deployment, with its
sources in its holder's order. temper configures the LLM providers
(configuration.md, section 2), shows and makes accounts from its client
(`../client/README.md`, 3.5; `../client/tools.md`, 5.5, area 5), and adds
`temper program-login`, below.

- **Made from the client, never configured.** Configuration names LLM
  providers, never an account (jig's `protocol/configuration.md`, section
  3). A fresh deployment has no account but a *none* source's, so no run
  on another LLM provider starts until a person adds one, and the client
  says so (`../client/README.md`, 5.1).
- **Kept by jig:** a signed-in record and a key in jig's secrets' store
  (jig's `protocol/store.md`, section 9), lent by skein's accounts driver
  through `jig-protocol-accounts` (jig's `protocol/llm.md`, section 5);
  never in a root, temper's records, a fact, a request or an answer. A
  key reaches the engine in a request the client sends once and never
  keeps (jig's `domain/accounts.md`, section 5).
- **Nothing handed in but a moved record.** No record is read from
  another program's login: a ChatGPT account comes from a sign-in (jig's
  `protocol/llm.md`, 5.4), and its grant is a bearer with no account's
  id (jig's `protocol/llm.md`, 5.3). The one record read from a file is
  a moved one: a signed-in account moved out of another temper
  deployment by `temper ctl export` and in by `temper ctl import`
  (`../client/tools.md`, 5.5, area 5), jig's move (jig's
  `domain/accounts.md`, section 6), in jig's form (jig's
  `protocol/llm.md`, 5.6) under the application's name `temper`,
  temper's root's constant. The file is the person's, written new and
  private by `ctl` and removed once moved in; the account has one
  holder at a time, the file or one deployment, and each side refreshes
  the record once it takes it, so a copy left behind is dead.
- **The sources** (jig's `domain/accounts.md`, 3.4):
  - **signed in:** the person's sign-in with the LLM provider, asked
    from the client. The engine binds a loopback listener for each
    sign-in, on an ephemeral port unless the LLM provider's
    configuration fixes one (jig's `protocol/llm.md`, section 5; skein's
    `oauth.md`, 6.5), so the browser that approves returns there: on
    the engine's machine, through a tunnel to it, or by the address it
    was sent to, pasted (below). A browser's page goes to the address in its own
    tab, and the listener sends it back to that page; the terminal
    prints the address and opens no browser (`../client/README.md`, 3.5
    and 6.5). temper detects no browser on another machine
    (`../client/README.md`, section 11).
    ChatGPT's registration fixes `127.0.0.1:1455` (setup.md, 8.1): a
    person whose browser is on another machine finishes the sign-in by
    pasting the address that browser was sent to, where nothing
    answers, into the page it started from, which hands it to the
    engine as jig's `FinishSignIn` (jig's `protocol/llm.md`, 5.4;
    `../client/README.md`, 3.5): the engine checks it against the
    sign-in's redirect and state and finishes the sign-in as its
    listener would. The address is a secret in transit, never kept by
    the page or the engine. Only the person who started the sign-in
    may paste, but nothing tells whose browser approved it: a person
    who handed the sign-in's address to another and pasted back where
    that browser landed would hold the other's ChatGPT account. Ready
    names the account signed in, and it is the paster's alone (jig's
    `domain/accounts.md`, 3.2). Or the person forwards that port to the
    engine's (`ssh -L 1455:127.0.0.1:1455 HOST`) for the sign-in. A
    plan's sign-in is the person's own, never the deployment's (jig's
    `domain/accounts.md`, 3.2);
  - **a key:** pasted on the accounts page, or read by `temper ctl` from
    a private file;
  - **a program's login:** declared from the client under its name, then
    signed in on each host by `temper program-login` (below). The login
    is the program's own, in the source's configuration directory,
    `<directory>/NAME` under the agent's programs directory
    (configuration.md, section 5), and temper never reads it;
  - **none:** an LLM provider whose endpoint takes no credential, such
    as a local model's on loopback; its one account is the deployment's.
- **The engine makes no LLM call** (llm.md, section 2): only agent
  processes use an account's values.
- **On the worker,** the routing puts each grant in smith's form (jig's
  `protocol/llm.md`, section 6) and hands it to smith's host half, which
  fills it into the agent's start and grants: a credential's value with
  its scheme, a bearer or a key; or a program's source by its name, by
  which the agent finds its configuration directory (smith's
  `protocol/programs.md`, section 3). Limited, refused and unauthorized
  travel back from the agent as notices, through smith's host half, the
  hub and the link, to the accounts child, fenced by attempt (jig's
  `domain/accounts.md`, 3.5). Unknown limit resets stay absent; an explicit
  zero remains supplied. A notice does not replace the run's terminal answer.
  At the current jig/smith pins, Bearer, Key and Withdrawn forms are checked
  against the lent value at start and renewal. Program names cross smith's
  codec without reading a credential table, but smith refuses installation;
  successful program runs and jig's production of Program grants still wait.
- **A host that keeps a person's program logins** runs only that
  person's work, until a run's process tree is contained from the
  programs directory (jig's `domain/accounts.md`, 3.4): a run can read
  every login kept on its host. A standalone deployment's one worker,
  on its person's machine, keeps that person's logins alone, and is
  unaffected.
- **`temper program-login [DIRECTORIES] NAME`:**
  - `NAME` is a program source's name, 1 to 32 of `a` to `z`, `0` to `9`
    and `-` (jig's `domain/accounts.md`, 3.4), as the accounts page and
    `temper ctl declare` give it with this command.
  - It reads and checks the document in its configuration directory
    (configuration.md, section 12) as `temper serve` does
    (configuration.md, section 8), and the agent's configuration it
    names, reading no secret file; it opens no store, starts no service
    and sends nothing to the engine, which need not run.
  - It refuses, exit 1, before it makes anything: a name out of its
    bounds; an agent's configuration with no program (smith's
    `protocol/agent.md`, section 4); a programs directory, or a
    `<directory>/NAME`, that exists and is not a directory of the
    user's own with no group or other bits, or is a link at its name
    (skein's `io.md`, 5.3). Linked parent directories are accepted.
  - It makes the programs directory and `<directory>/NAME`, each
    `0o700`, when missing, then replaces itself with the program's own
    sign-in, `claude auth login`, at the configured path. The
    environment is the caller's, so that the program's sign-in has the
    person's terminal and browser, with `CLAUDE_CONFIG_DIR` the source's
    directory and the program's credential variables removed, as every
    command of a program source's is (smith's `protocol/programs.md`,
    4.2): `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`,
    `CLAUDE_CODE_OAUTH_TOKEN` and the cloud providers' variables, which
    would win over the login (jig's `domain/providers.md`, 3.3). The
    forge's commands clear the environment before they exec (setup.md,
    6.4); a sign-in cannot.
  - Its exit status is the program's. A failure before the exec exits
    1, its reason on stderr: temper writes one line beginning
    `temper program-login:`. Smith's configuration reader may precede it with
    its own secret-free `smith: ` diagnostics, as under the other commands
    that read a smith configuration. No diagnostic is redirected or suppressed.
  - It reads nothing of the login. The host's half checks it (smith's
    `protocol/programs.md`, section 3; smith's `protocol/hosts.md`,
    section 2) at its hello, when told of a new source, on its holder's
    check and at its interval (jig's `domain/accounts.md`, 3.4), and the
    accounts page shows the state.
  - Its name says what it does, never a provider's product (jig's
    `domain/providers.md`, 3.2).
  - A worker on another host is signed in on that host, with that host's
    configuration.
- **A program's sign-out** is the host's half's, on each host, by the
  program's own (smith's `protocol/hosts.md`, section 2; jig's
  `domain/accounts.md`, 3.4), when its holder signs the source out or
  removes it from the client.
- **What temper does not do:** configure an account, sign one in at a
  terminal of its own, keep a record or a key in its own store or a
  file of its own (a moved record's file is the person's, above),
  refresh an account, or read a program's login.

## 4. Git identities

- **What one is:** the token git authenticates with, the login it goes
  with, and the author (a name and an email) of the commits made with
  it. It is an application credential (jig's `domain/engine.md`, 12.1),
  numbered by configuration (configuration.md, section 3).
- **One per forge.** Each identity serves exactly one forge, and each
  repository's runs use its forge's: configuration names the identity's
  number, its forge and its author; its value is the forge's. The root's
  `GitIdentity { forge, credential }` comes from configuration, and each
  repository of an assignment's workspace names its forge's.
- **On Forgejo:** the forge's `GitToken` record, with the bot's login,
  as the forge's `Connected` names it, unless configuration gives
  another. Its validity
  is unbounded (`Duration` at its largest, as jig's reference service
  gives its credential): Forgejo's tokens do not expire.
- **On GitHub:** a list of entries, one per installation that has a
  token, each the installation's account id and login and its token,
  built from the forge component's `Event::GitToken { forge,
  installation, account, token, expires }` and held in memory only, by
  installation. The component mints each token with the App's key, for
  contents alone, for every accepted installation that is present and
  not suspended, and again `renew_before` ahead of its expiry (skein's
  `forge-connection.md`). The login is `x-access-token`; the author
  defaults to the App's bot, `<slug>[bot]` with
  `<id>+<slug>[bot]@users.noreply.github.com`, `<slug>` the `AppSlug`
  record's and `<id>` the `User` record's. The value's validity is the
  earliest expiry among its tokens, minus now, and zero once that has
  passed; a list with no entry is valid without bound, as Forgejo's
  value is.
- **A GitHub forge's entries change** on its events, in the routing:
  a `GitToken` puts its installation's entry, replacing the one before;
  an `Installation` change `Deleted` or `Suspended`, or an installation
  no longer accepted, takes its entry out, when it has one. A forge that
  stops keeps its entries, which lapse with the value's validity; a
  forge connected again replaces them as its new tokens come.
- **Generations.** Each start gives each identity that has a value
  generation 1. Each change of value makes the next generation, strictly
  increasing per number: a Forgejo forge's `GitToken` replaced; on
  GitHub, each change of its entries. With N accepted installations, a
  GitHub forge's identity takes about N generations an hour, each one
  `Event::Credential` to the live attempts holding it. A generation is
  never kept.
- **The order:** the routing puts each value in the identities' table,
  then gives the root `Event::Credential { number, generation, valid }`
  (jig's `domain/engine.md`, 12.1; `../domain/root.md`, 7.1). At start,
  each Forgejo identity with a value is announced before the root's
  restart, so it is usable when decisions open. A GitHub forge's entries
  gathered before its first `Connected` since the start make no
  generation: that `Connected` announces generation 1, holding every
  token the component handed up before it, which is one per accepted
  installation it could mint for, so that no run starts on a partial
  list. Each change after it announces the next. Until generation 1, a
  run that needs the identity waits (jig's `domain/engine.md`, 12.1).
- **The identities' table, on the engine:** each identity's two newest
  values, by number and generation, each encoded once by
  `temper-protocol-worker-wire` and held as one secret; the value server
  answers either. A grant of an older generation is left out as it is
  encoded, and the newer one follows it (jig's `protocol/channel.md`,
  section 8). The table is temper's own: jig's accounts' table holds LLM
  credentials' schemes alone. No root, record of the main store, fact or
  log ever holds a value.
- **The value's codec,** engine to worker, is temper's, versioned
  (channel.md, section 6): a kind holding Forgejo's one token or GitHub's
  list, with the login and the author. It is never stored.
- **Granted with the assignment.** temper's assignment names the
  identity's number in each workspace item that names a repository, and
  on GitHub the account id of the entry whose account login is the
  repository's owner, compared in ASCII without case, as the routing
  last made the value (channel.md, sections 4 and 6); jig pushes an attempt only
  the credential numbers its workspace names. `jig-protocol-engine-workers`
  fills the value in from the identities' table, through the routing, as
  it encodes the assignment or a grant record, and grants follow their
  attempts as an account's do (jig's `protocol/channel.md`, section 8). A
  run with no repository is granted none.
- **On the worker,** `jig-protocol-worker-link` takes the value out into
  the worker's identities' table, by number, at most the two newest
  generations, so that a push in flight across a change ends on the
  value it began with; the hub hears the name only. temper's worker-side
  protocol layer hands the git component the identity's value with each
  operation that names it, for git's HTTPS transport, and the component
  holds it only while that operation is in flight; it never reads the
  table itself (git.md, sections 2 and 5). On GitHub the worker picks the
  entry whose account id is the item's; with none, the
  push is refused, named "temper's App is not installed on the account
  owning <name>" (channel.md, section 6).
  An operation whose identity has no value in the table fails `Broken`
  without running git. An entry goes as jig's do (jig's
  `protocol/channel.md`, section 8): a newer generation pushes it out, or
  its validity ends.
- **In git,** the environment only: an `http.extraHeader` of HTTP basic
  authentication from the login and the token, and the author's
  variables; never an argument, a URL, a repository's configuration or a
  file (git.md, section 5).
- **Agents never get one.** jig's hub never forwards an application's
  credential to an agent: an agent process's environment holds no
  credential (jig's `protocol/service.md`, section 6), and the hub hands
  an agent its LLM grants only.
- **A forge that refuses the identity** fails the workspace, never the
  credential: a preparation it refuses is `Preparation::Refused`
  (`../domain/worker.md`, 4.3), a push `Landing::Refused`
  (`../domain/worker.md`, section 5). Limited, refused and unauthorized
  are LLM accounts' notices only (jig's `domain/accounts.md`, 3.5).
- **Replaced** from the client: `Replace` on a Forgejo forge's git token
  (section 5) keeps the new record, then announces the next generation.
  A GitHub forge's tokens renew themselves. Generation 1 at each start is
  sound only because no worker outlives the engine: in standalone the
  worker is in the engine's process, and the cluster restarts whole,
  its workers stopped before its engine and never left under a new one
  (README, 15.4; section 8).

## 5. The forges' access

- **One access per forge,** built at start from its records (section
  2.1) and held by the forge component alone:
  - **Forgejo:** the `ApiToken`, in the `Authorization: token` header of
    each request to that forge's API (forge.md, 3.1); the `User`
    record's id; the `WebhookSecret`, when there is one.
  - **GitHub:** the App's id and key (`AppId`, `AppKey`), which sign the
    App's own requests, and by which the component mints a token for
    each installation the `Accepted` record names, expiring within the
    hour and minted again `renew_before` ahead of its expiry (skein's
    `forge-connection.md`); the `User` record's id, the App's bot's; no
    webhooks' secret. The App's hook points at `https://<slug>.invalid/`,
    and the component reads changes from the App's delivery log (forge.md,
    a GitHub App's delivery log).

  Each request goes over TLS unless the forge is on loopback
  (configuration.md, section 3).
- **Never granted, never stored elsewhere:** an access's values are in no
  grant, root, record of the main store, fact or log, and are never sent
  to another forge or another host. An installation's token is never
  kept: a restart mints new ones, and the one the setup host mints for a
  page of the picker is dropped with the page.
- **Its user, checked.** A connect's run finds the user its credentials
  answer as, and keeps its id (`User`) with the `Origin` the records
  were made for. A connected Forgejo's two pasted tokens must
  answer as one user; a replacement must answer as the forge's kept
  user, and on GitHub be a key of the same App (setup.md, the setup
  host's plans). The forge component's first exchange with each forge
  checks its access answers as that user (skein's `forge-connection.md`,
  section 3): until it passes, the forge's calls wait in the component,
  and a check that goes unanswered is made again (skein's
  `forge-connection.md`, 3.3).
- **A forge that refuses its access stops that forge, never the
  engine.** A `401`, a `403` for the credential, or an answer as another
  user, ends in `Stopped { forge, why }` from the forge component; the
  routing hands it to the root, the deployment's page names it, and the
  client offers Replace. That forge's calls fail as forge.md classifies a
  stopped forge; other forges, the engine and its runs go on. A wrong
  credential would make temper act as someone else, so the stop holds
  until a replacement answers as the forge's user: never a degraded mode.
  A failure of jig's secrets' store itself stops the engine, as the main
  store's does (jig's `protocol/store.md`, section 9).
- **The same user is the forge writer** in the domain (`Writer { forge,
  author: user }`), by which the forge connector tells temper's own
  writes from others': a key's provenance, and its own writes' echoes
  (`../domain/forge.md`, 5.2; forge.md, section 4). It comes from the
  forge's connect, never from configuration: on `Connected` the root is
  told the user (id and login), and the connector takes the writer.
  skein's forge client keeps it in the main store, not secret (section
  2.1).
- **The admin's token is the setup host's alone,** never the forge
  component's. The setup host holds a managed or owned Forgejo's while a
  plan runs (reconcile at each start and at a managed connect; access at
  an adoption; the picker's reads, a `Reachable` plan, skein's
  `forge-setup.md`, 4.8), from its record, and drops it when the plan ends (setup.md, the
  setup host in the engine; forge.md, the setup's host). The picker's
  page holds repositories only, never the token (`../domain/forge.md`,
  the picker). `Forget` erases the record: the forge is then connected,
  the bot's tokens its whole power.
- **Replaced from the client:** `Replace { forge, value }` carries the
  new token or key (section 2.2); the setup host checks it answers as
  the forge's user, the new record is kept, and the forge component is
  given the new access. On a managed or owned Forgejo, reconcile makes
  the bot's tokens again when they do not answer as the bot (setup.md,
  section 5).

## 6. Workers' secrets and sign-ins

- **A worker's secret is jig's** (jig's `protocol/channel.md`, 4.1). In
  standalone the link is in process, and still opens with the worker's
  name and secret; the files are jig's worker and engine parts'
  (configuration.md, section 6).
- **People's credentials are jig's** (jig's `protocol/client.md`,
  section 8): jig mints, verifies, expires and revokes them. Its printed
  link signs in the deployment's first owner: `temper serve` prints the
  link at start, `temper open` signs in by it; its client holds the
  bearer token it is given and sends it with every exchange, and the
  terminal keeps it in its private directory, to sign in with it again
  (`../client/README.md`, section 7). Every other way of signing in is open
  (section 8).
- **No TLS key** in standalone: nothing temper serves leaves the machine.

## 7. Testing

- **No value across a domain boundary:** the one-process world and the
  forge's protocol world search every root's events and requests, every
  fact and every record of the main store for the values their fakes
  issued (LLM access tokens and keys, forges' tokens, Apps' keys,
  installations' tokens, admins' tokens, webhooks' secrets), and fail on
  any (`../domain/testing.md`, 2.5). After each real-loop story, no
  secret is in a root, a fact, a record but the secrets' store's, a
  request or an answer.
- **No live forge:** every forge these tests reach is skein's fake, and
  every credential a placeholder its fake issued; no test calls
  github.com or uses a real credential. Live calls are skein's opt-in
  conformance's alone.
- **The codecs,** in step tests with golden bytes and fuzzing: the
  records' numbers and each item's value, the pasted bundle, and the git
  identity's value of each kind; an unknown version refused, and each
  item's bytes that skein's decoder refuses (an id of 0, `Accepted` out
  of order, repeated or past its bound) leaving the forge not connected.
- **The identities' table:** step tests of its filling at start, a value
  filled into a grant and taken out on the worker, two generations, a
  push across a change ending on the value it began with, and on GitHub
  the entry picked by the owner's account id, a push with none refused.
- **Git's environment:** step tests of its builder: the header's bytes,
  the author's variables, and no credential in an argument or a URL
  (git.md, section 9).
- **The routing,** in `temper-service`'s own tests (forge.md, section
  10): start's load, where a
  forge's unreadable row, records made for another forge, or an
  incomplete set leave it not connected, named, and the other forges
  running; a connect whose carried value is read by its run and erased
  at its end; carried rows erased at a restart; installations' tokens
  renewed, each a new generation; a credential of another user, or one
  refused, stopping that forge alone, named, while the engine and the
  other forges run on; calls waiting until the user check is answered,
  and an unanswered check made again.
- **Startup:** a secret file readable by another user, or reached
  through a link, refuses the start, naming it.
- **`temper program-login`,** step tests: each refusal (a name out of
  its bounds, no program configured, a directory not private or reached
  through a link), each before anything is made; the directory made
  `0o700`; and the exec's program, arguments and environment: the
  caller's kept, `CLAUDE_CONFIG_DIR` the source's directory, the
  program's credential variables removed. The process tier execs smith's
  fake `claude` (smith's `protocol/programs.md`, section 8) and finds its
  login in the source's directory (`../domain/testing.md`, 2.6).
- **LLM accounts' stories** in the real loop (`../domain/testing.md`, 2.4): a
  key added, a sign-in against skein's fake issuer with a redirect the
  test drives, and a program's source over smith's fake `claude`; after
  each, no value in temper's store, a fact, a request, an answer or the
  client's small store.
- **The live tier** runs on a program's login on its machine, which
  temper never reads, or on a sign-in or a key added per run
  (`../domain/testing.md`, 2.7).

## 8. Open questions

- **Git's environment, readable by agents.** An agent run as the same
  user could read a git process's environment. Default: accepted in
  standalone, where every process is the person's; between hosts,
  contained process trees give agents a process namespace of their own,
  which closes it (jig's `protocol/README.md`, section 10). Without
  them, the token would reach git through a pipe and a credential helper
  instead. An owned runner's jobs run as the same user too; their
  isolation, CI's, is setup.md's (section 15).
- **Generations across starts.** Generation 1 at every start names a new
  value once a record is replaced or a token minted, which is sound only
  while no worker outlives the engine; a GitHub forge's identity moves
  about one generation an hour per installation. Default: standalone,
  and the cluster restarted whole (README, 15.4); a fleet whose workers
  outlive the engine waits for generations kept across starts, with
  hosts apart (README, section 14).
- **A GitHub entry chosen by the owner's id.** The routing matches a
  repository to an entry by its owner's login, since an adopted
  repository keeps no owner's id, so an account renamed after adoption
  matches no entry, and its pushes are refused as not installed.
  Default: the login; keeping the owner's id at adoption (the store's
  `Adopted`, from the forge's read) would let the routing match the
  entry's `account` directly.
- **A connect cut by a restart.** Default: nothing kept; the carried
  row is erased at start and the forge reads as its records say, so a
  person whose connect a restart cut sees the forge not connected, with
  no line, and pastes again. Saying "interrupted; paste again" would
  take the connect's own record (its forge, number and digest) in the
  small store, a stored format (the store's version), read at start
  and erased once said; a carried row alone names no forge.
- **Rows of a forge not configured.** Default: left as they are and
  named once in the log at start; erasing them waits for a request that
  disconnects a forge.
- **Backups of the secrets' store.** Default: none; a deployment moved
  elsewhere takes its secrets' directory, or connects its forges again.
- **A second program.** `temper program-login` signs in the agent's
  one program, Claude Code. Default: it takes `--program ADAPTER` once
  smith's agent adapts a second (smith's `protocol/programs.md`).
- **The forge as people's sign-in provider,** for a deployment with more
  than one person: routes of temper's on its clients' listener run the
  forge's OAuth flow, read who the person is, and ask jig's clients'
  component to mint a credential for that identity, with a `Mint` from
  temper's server code alone (jig's `protocol/client.md`, section 8;
  jig's `protocol/service.md`, section 2; `../client/README.md`, section 11).
  The forge's OAuth client and its secret are then temper's records, in
  jig's secrets' store. Default: the printed link signs in the
  deployment's one owner.
