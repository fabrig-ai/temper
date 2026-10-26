# Connect Forgejo

Kind: walk

Connect temper to a Forgejo as its bot, in one of three modes:
connected, managed or owned.

temper works on a Forgejo in one of three modes. Each ends with temper
connected as its bot, a Forgejo user of its own; they differ in who
makes that bot and what temper keeps.

| Mode | You give temper | temper makes |
|---|---|---|
| Connected | the bot's API token and git token, pasted | nothing on the forge |
| Managed | a site admin's token, pasted | its bot and the bot's tokens; at each adoption, the bot's access and the repository's hook |
| Owned | a setup entry naming Forgejo's binaries | the forge itself, its runner and admin; then all of managed's |

The design is `docs/design/protocol/setup.md`; the page you use is the
client's **Forges** (`docs/design/client/forge.md`, the deployment's
page). A local owned forge, from a checkout to a first change, is the
box's walk, `temper help box`.

## 1. Add the forge to the configuration

Add a forge of kind `forgejo` to `application.forges`, and its identity
to `application.identities`:

```json
{"number": 1, "kind": "forgejo",
 "api": {"host": "forge.example", "port": 443},
 "transport": {"tls": {"server_name": "forge.example", "trust": null}},
 "git": "https://forge.example", "web": null, "log_origins": null}
```

```json
{"number": 1, "forge": 1, "login": null,
 "author": {"name": "temper", "email": "temper-bot@forge.example"}}
```

- **A forge's `number` is its stable id,** never reused. temper keeps
  the forge's records under it, so give a forge a number no other forge
  has had. A forge whose number changes is a new forge, not connected,
  and its old number's records are left as they are.
- **Records made for another kind or address** leave the forge "Not
  connected: its records were made for KIND at ADDRESS". Nothing is
  erased; connecting again overwrites them.
- **The identity's `login`,** `null`, is the bot's; its `author` names
  temper's commits.
- **No credential is configured.** Every token is pasted in the client
  and kept as the forge's records (section 5).
- **Who may connect:** the one whose role holds the deployment's
  connectors; in standalone, the deployment's owner. `temper ctl` is
  signed in once with the link `temper serve` printed:
  `temper ctl me sign-in -`.

Restart `temper serve`. The forge's row on **Forges** says "Not
connected", with **Connect**.

## 2. Connected: paste the bot's tokens

### Make the bot on Forgejo

1. **A user for temper,** not a site admin. Put `temper:bot` in its full
   name if you may later give temper a site admin's token (section 3):
   temper then takes this user over rather than making another.
2. **Two tokens of that user,** under its settings' applications:
   - the API token, with the scopes `read:user`, `write:issue` and
     `write:repository`;
   - the git token, with `write:repository` and `read:user`.
3. **Its write access** to each repository temper will adopt: a
   collaborator with write, or a member of a team that has it.
4. **Webhooks, if you want them.** Without them temper polls, and
   keeps up all the same, more slowly. With them, configure
   `application.webhooks`, as `{"listener": {"socket":
   "127.0.0.1:7101"}}`, and on each repository, or its organization,
   add a Forgejo webhook:
   - target `http://127.0.0.1:7101/hooks/1`, the listener's address and
     the forge's number; content type JSON; a secret of your own
     drawing;
   - the events skein's `forge.md`, section 7 lists: issues, and their
     assignments, labels and milestones; pull requests, and theirs,
     their pushes and their review requests; issue and pull request
     comments; reviews; pushes; branch creation and deletion; workflow
     jobs;
   - Forgejo's `[webhook] ALLOWED_HOST_LIST` in its `app.ini` naming
     the listener's IP address: Forgejo's default refuses loopback.
5. **CI,** as each repository has it.

### Paste them

On **Forges**, choose **Connect** on the forge's row: paste the API
token, the git token and, when you made webhooks, their secret,
together. Or write them into a private tokens file, one `NAME VALUE` a
line:

```text
api <api-token>
git <git-token>
webhook_secret <secret>
```

```sh
chmod 600 tokens
temper ctl me forge-connect 1 forgejo tokens --key forgejo
temper ctl me wait forges --until settled
rm tokens
```

temper checks that both tokens answer as one user, not a site admin,
and the row says "Connected as LOGIN (ID)". Otherwise it says why, and
the forge stays as it was:

- "the API token and the git token answer as LOGIN and LOGIN": two
  users' tokens;
- a site admin's token, a token Forgejo refuses, or the forge
  unreachable, each named.

### Adopt, and add repositories later

Adopting grants nothing on a connected forge. The adoption form's
picker lists the repositories the bot may write; for one it does not
list, "Not listed" says "Make LOGIN a collaborator with write on
OWNER/NAME." Do so on Forgejo, and add its webhook if you use them.

### Replace a token

temper cannot replace a forge's token yet; neither **Forges** nor
`temper ctl` offers it in this build. A token Forgejo refuses under a
running engine stops the forge, "Stopped: WHY"; the engine and every
other forge run on.

## 3. Managed: paste a site admin's token

temper keeps a site admin's token and makes, repairs and grants what it
needs itself.

- **Forgejo on the same machine.** A managed forge's `api` is a socket
  address on loopback, its `transport` `"plaintext"` and its `git`
  `http://<address>`; a TLS transport refuses the start, naming it.

  ```json
  {"number": 1, "kind": "forgejo",
   "api": {"socket": "127.0.0.1:3000"}, "transport": "plaintext",
   "git": "http://127.0.0.1:3000", "web": null, "log_origins": null}
  ```

- **A setup entry** in `application.setup` naming its bot:

  ```json
  {"forge": 1,
   "bot": {"login": "temper", "email": "temper@example.invalid"},
   "mode": "managed"}
  ```

  A Forgejo with no entry refuses a connect as its admin, naming
  `application.setup`.
- **A site admin's token,** made in that admin's settings, with the
  scopes temper's own preparation gives its admin: `write:admin`,
  `read:user` and `write:repository`. Without `write:repository`,
  Forgejo refuses temper the bot's access when you adopt a repository,
  and temper says that scope is missing.

On **Forges**, choose **Connect as its admin** and paste the token, or:

```sh
chmod 600 admin-token
temper ctl me forge-connect 1 forgejo-admin admin-token --key forgejo-admin
temper ctl me wait forges --until settled
rm admin-token
```

temper makes its bot, the setup entry's login, unless the forge was
connected before: then it takes over the connected user when its full
name carries `temper:bot`, or says "LOGIN is not temper's bot: add
`temper:bot` to its full name on Forgejo, then connect again.", and the
forge stays connected. Done, the row says "Connected as LOGIN (ID)",
managed, "admin's token kept".

- **The admin's token is the setup host's alone,** the part of the
  engine that runs temper's setup: it reads the token to reconcile the
  forge at each start, to grant access at an adoption, and to list the
  picker's repositories. The forge's component, which does temper's
  work, never holds it.
- **The picker lists every repository the admin can see.** Adopting one
  makes the bot a collaborator with write on it, and adds its webhook
  when `application.webhooks` is configured. Nothing is granted
  organization-wide: the bot reaches the repositories you adopted, and
  no other.
- **Each start repairs** the bot, its tokens and each adopted
  repository's access and hook, where the forge differs. A start whose
  repair stops leaves the forge not connected, named; connect as its
  admin again to mend it.
- **Forgejo's own settings** are yours: temper does not change
  `ALLOWED_HOST_LIST` or whether Actions are enabled. A forge whose host
  list refuses temper's listener delivers no webhook, and polling keeps
  up.
- **To stop keeping the token,** choose **Forget the admin's token**, or
  `temper ctl me forge-forget 1`. temper erases it, and the forge is
  connected, its bot, tokens and hooks kept. Adopting then grants
  nothing: make the bot a collaborator yourself (section 2). Then delete
  the token on Forgejo.

## 4. Owned: temper runs the forge

temper prepares and runs a Forgejo of its own on loopback, with a
runner, and manages it. The setup entry's `mode` is `owned`, naming
Forgejo's and the runner's binaries, the forge's directory, its address
and your account (`docs/examples/owned.json` is a whole example). `temper box --config C --data D --cache K`
runs every step below in order; the box's walk is `temper help box`.

For a fresh box, follow `temper help box`: `temper ctl DIR forge-setup
3` starts its owned Forgejo and adopts the first repository without a
browser. Its CI jobs run on the box's runner, on this machine as the
box's user, with label `temper-host`, without a container. The starter
workflow only echoes success on pushes to `main` and pull requests;
your repository's real checks are its own workflow's. With CI on, a
change waits for a reported pass at its head. Read `temper ctl DIR
change N` for that CI state, the step, pull request and merge commit.

For an already configured owned forge, the box runs these commands:

1. **`temper forge prepare --config C --data D --cache K 1`** makes the forge's private
   directory and Forgejo's settings; temper's admin, `temper-admin`;
   your account, a site admin, with its first password in
   `person_password`, private, with no forced password change; your read and
   write API headers, `person_read.header` and `person_write.header`; and
   the runner's registration. It leaves one file
   for the engine: `admin_token`, the handover, temper's admin's token.
   A later preparation makes it again only when it is missing.
2. **`temper forge run --config C --data D --cache K 1 forgejo`**, and `... 1 runner`,
   run Forgejo and its runner.
3. **`temper serve --config C --data D --cache K`** takes the handover at its start: it
   reads `admin_token` once, keeps it as the forge's admin's token,
   deletes temper's admin's other tokens, and removes the file. Then it
   reconciles as managed, and makes each of your organizations that is
   missing.

- **A forge not yet prepared** says "Not connected: prepare it with
  `temper forge prepare`".
- **Files of an older temper,** `api_token`, `git_token`, `user` or
  `webhook_secret` in the forge's directory, are left unread, and
  nothing is converted: temper makes the forge's records afresh from
  the handover.
- **Sign in on Forgejo** as yourself with `person_password`. Change it
  on Forgejo when you like. The box prints the API address and your read
  and write header paths after the forge starts; pass a header by path
  with `curl -H @FILE`, keeping its token private. temper's admin
  is temper's; deleting it stops the next start's repair and the next
  preparation, naming it.

## 5. What temper keeps, and where

- **In jig's secrets' store,** under the forge's number, written by the
  engine's setup alone: the bot's API token and git token, its id, the
  webhooks' secret, the kind and address the records were made for, and
  on a managed or owned forge, the admin's token
  (`docs/design/protocol/credentials.md`, the records).
- **The webhooks' secret,** on a managed or owned forge, is drawn by
  temper when `application.webhooks` is configured, and set on each hook
  it makes; on a connected forge it is the one you pasted, or none.
- **What you paste** is kept by jig until its run ends, the pasted
  value, then erased (`docs/design/protocol/credentials.md`, pasted
  values). No page, log or answer shows it: **Forges** says that a value
  was given, never what. A run cut by a restart says "interrupted; paste
  again".
- **`temper ctl` reads each value from a private file:** your own, with
  no group or other permission bits, not a link. It never prints the
  value; delete the file once the row says connected.
