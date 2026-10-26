# Connect GitHub

Kind: walk

Connect temper to GitHub through a GitHub App, temper's own or one you
make, and install it on the accounts whose repositories it works on.

temper works on GitHub through a GitHub App. temper makes the App from
its manifest in your browser, or you make one yourself and paste its id
and key; both end in the same records, and temper runs them alike. In
temper's web page, **Connect GitHub** takes the tab to GitHub and
GitHub brings it back; `temper ctl` and the terminal print each link
and never open one. For an App and installation that already exist,
sections 11 and 12 give the path through ctl without a browser.

## 1. What you need

- **A GitHub account.** temper's App is made under it; you may install
  it on your account and on any organization you own.
- **temper's clients' listener at a fixed address** your browser
  reaches, such as `127.0.0.1:7100`. GitHub sends your browser back to
  it, so open temper's links in a browser on the engine's machine, or
  forward that port yourself; temper makes no tunnel.
- **Who may connect:** the one whose role holds the deployment's
  connectors; in standalone, the deployment's owner.
- **The client,** or `temper ctl` signed in once with the link
  `temper serve` printed:

```sh
temper ctl me sign-in -     # paste the sign-in link, then Ctrl-D
```

## 2. Add the forge to the configuration

A configuration written by `temper init` already has GitHub as forge 2
and its identity; keep those entries. For another configuration, add a
forge of kind `github` to `application.forges`, and its identity to
`application.identities`. Its addresses are `null`, which are
github.com's; a value is given only to point at a fake, as temper's
tests do.

```json
{"number": 2, "kind": "github", "api": null, "transport": null,
 "git": null, "web": null, "log_origins": null}
```

```json
{"number": 2, "forge": 2, "login": null, "author": null}
```

- **A forge's `number` is its stable id,** never reused. temper keeps
  the forge's records under it, so give GitHub a number no other forge
  has had. A forge whose number changes is a new forge, not connected,
  and its old number's records are left as they are.
- **Records made for another kind or address** leave the forge "Not
  connected: its records were made for KIND at ADDRESS". Nothing is
  erased; connecting again overwrites them.
- **The identity's `null`s** are the App's: git signs in as
  `x-access-token`, and commits are the App's bot's, `<slug>[bot]`.
- **The clients' listener** needs a fixed port, as
  `"clients": {"listener": {"socket": "127.0.0.1:7100"}, ...}`; port 0
  refuses the start, naming `application.clients.listener`.
- **A landing rule** for one of its repositories names `github.com` as
  the forge's host:
  `"segments": ["forge", "github.com", "<owner>", "<name>", "branch"]`.
- **GitHub takes no `setup` entry.**

Restart `temper serve`. The forge's row on **Forges** says "Not
connected", with **Connect**.

## 3. Connect GitHub with temper's App

On **Forges**, choose **Add a forge**, then **Connect GitHub** on
GitHub's card; the welcome's **Set up temper** offers the same card.
The tab goes to GitHub, by a page of temper's on its clients' listener:

```text
http://127.0.0.1:7100/forges/2/github/manifest?state=<state>
```

Or, with `temper ctl`, which prints that link for you to open:

```sh
temper ctl me forge-connect 2 github --key github
temper ctl me wait forges --until settled
```

1. **temper's page** sends temper's manifest to GitHub's form for a new
   App as it opens; where it can't, it shows one button, **Continue to
   GitHub**.
2. **Create the App** on GitHub's form, under your account. You may
   rename it there; temper takes the name GitHub keeps.
3. **GitHub sends you back to temper,** which keeps the App's id, slug
   and key, then sends you on to the App's install page.
4. **Install it:** GitHub lists your account and your organizations;
   choose **Install** beside one, then **All repositories**, GitHub's
   default, or **Only select repositories**. GitHub brings you back to
   the page you started from, which lists the repositories to start
   in; a link printed by `temper ctl` ends on temper's page instead,
   "temper has the installation; you may close this tab."

- **An organization you're only a member of** offers **Request**, not
  **Install**: its owner must approve it. temper can't wait for that
  yet, so its page says so, "GitHub sent a request, not an
  installation": install it on your own account, or on an organization
  you own.

The row says "Connected as `<slug>[bot]` (`<id>`)", and lists the
installation: its account, and "selected repositories" or "all
repositories".

- **A link is good once,** and for a while. A used or expired link, or
  one from before a restart, answers "This link has ended. Start again
  from temper.": choose **Connect** again, or **Install on another
  account** once the App is made.
- **A tab closed before installing** leaves the App made and installed
  nowhere: install it as section 5 says.

## 4. Or connect an App of your own

Make the App on GitHub's page for a new GitHub App, under your account's
or your organization's developer settings:

- **Its name and homepage** are yours to choose.
- **Callback URL:** none. Leave **Request user authorization (OAuth)
  during installation** unchecked; temper asks for no user's sign-in.
- **Setup URL:** temper's setup route, with **Redirect on update**
  checked. GitHub brings your browser there after each install and each
  change of its repositories. Accept an installation that already exists
  through ctl as in section 12; otherwise one that never comes back
  through this route is not accepted, and may be uninstalled (section 12).

  ```text
  http://127.0.0.1:7100/forges/2/github/setup
  ```

- **Webhook:** keep **Active** checked, with any address that never
  resolves, such as `https://<slug>.invalid/`, and no secret. temper
  sets it to `https://<slug>.invalid/` and reads the App's deliveries
  from GitHub's log of them, so GitHub needs no address of yours. For a
  shared App, leave its hook to its owner and set read-only mode before
  connecting (section 11).
- **Repository permissions:** Contents, Pull requests, Issues and Merge
  queues, read and write; Checks, Commit statuses, Actions,
  Administration and Metadata, read-only; Workflows, no access.
- **Events:** Push; Pull request, Pull request review and Pull request
  review comment; Issue comment; Issues; Check run and Check suite;
  Status; Workflow job and Workflow run; Create and Delete; Merge group.
- **Where it installs:** **Any account** to install it on accounts
  other than its owner's; **Only on this account** keeps it to yours.

Create it. Note its App ID on its page, and generate a private key:
GitHub downloads it as a `.pem` file. On **Forges**, choose **Connect**,
then **Connect an App of your own**: its id, and the key's text pasted
as GitHub gives it. No webhook secret is asked. Or, with your App ID
in place of `12345`:

```sh
chmod 600 <slug>.private-key.pem
temper ctl me forge-connect 2 github-app 12345 <slug>.private-key.pem --key github-app
temper ctl me wait forges --until settled
```

- **temper checks the App with the key:** its id the one you gave, its
  bot user, its hook, its permissions and events. Then the row says
  "Connected as `<slug>[bot]` (`<id>`)".
- **An App lacking a permission or event** connects, and the row says
  "temper's App lacks WHAT: grant them in the App's settings, then
  paste its key again.", with a link to the App's settings. Grant them
  there; GitHub asks each installation's owner to approve them. This
  build cannot replace the key yet, through **Forges** or `temper ctl`.
- **Install it** as section 5 says, or accept an existing installation
  through ctl as in section 12.
- **Delete the downloaded file** once the row says connected. temper
  keeps the key; GitHub makes you a new one when you need it (section
  9).

## 5. Install on another account

On the forge's row, choose **Install on another account**, or:

```sh
temper ctl me forge-install 2
```

The tab goes to GitHub's install page (`temper ctl` prints its link).
Pick the account or organization, and **All repositories** or **Only
select repositories**. GitHub brings you back to temper, and the row
lists the installation.

- **Only accepted installations are used.** Install through temper's
  link, or accept an existing installation with `forge-accept` (section
  12). Others are listed as "not accepted"; read-only mode keeps them
  (section 11). Otherwise reconcile uninstalls them once older than an
  installation link's lifetime.
- **An installation deleted on GitHub** leaves the row; one GitHub
  suspends says "suspended", and its repositories are refused until it
  is resumed.
- **Adopting on an account with no installation** is refused: "temper's
  App is not installed on OWNER.", with **Install on another account**.

## 6. Add repositories later

An installation of selected repositories reaches only those. To add
one, open the installation's page on GitHub, linked from the forge's
row:

```text
https://github.com/settings/installations/<id>
https://github.com/organizations/<org>/settings/installations/<id>
```

Add the repository and save. GitHub brings your browser back to temper's
setup route, and the adoption form's picker lists the repository, under
that installation's account. Or list its pages with `temper ctl me
repositories 2 --installation <id>`, the installation's id as the
forge's row shows it.

- **A repository not granted** is said where you adopt it: "OWNER/NAME
  is not granted to temper's App.", linked to the installation's page;
  the picker's "Not listed" says "Grant OWNER/NAME to temper's App on
  GitHub's installation page."

## 7. What a repository's rules need

temper reads the landing branch's rules at adoption, rulesets and
classic protection alike, and says each with what it means for landing
and the one setting that lets temper land. temper never changes a
repository's rules.

- **A merge queue** is fine: temper's change enters it as its merge,
  and the queue merges by its own method, set in the branch's rules.
  The branch's required checks run on the queue's groups too, by
  GitHub's rule: a workflow that is required lists `merge_group` among
  its events.
- **Landing directly,** with no merge queue: temper merges the pull
  request once its landing rule is met.
- **Required approvals or code owners:** a person approves on GitHub, or
  you add temper's App to the rule's bypass list, for pull requests
  only or always.
- **Required checks** are CI, as the landing rule's.
- **A gate's verdict cannot be required:** temper's App reads commit
  statuses and writes none, so a gate's verdict stays temper's own and
  no rule of GitHub's can require it. Each adoption on GitHub says so.
- **Signed commits:** temper's workers do not sign their commits. Where
  GitHub refuses temper's merge for it, the change waits for a person's
  merge.
- **Linear history:** allow squash or rebase merging on the repository.
- **The merge style** is the first the repository allows of merge,
  squash and rebase: to have temper squash, allow squash and not merge
  commits.

## 8. Job logs

GitHub serves a job's log through a redirect to a storage host. temper
follows it only to an origin its forge's `log_origins` lists. `null` is
GitHub's default, `https://productionresultssa1.blob.core.windows.net`;
`[]` fetches no job log.

When **Forges** says "A job's log is on ORIGIN, which temper does not
read: add it to this forge's `log_origins` and restart.", list that
origin. A list replaces the default, so keep the default in it:

```json
"log_origins": ["https://productionresultssa1.blob.core.windows.net", "https://<host>"]
```

Then restart `temper serve`. The list holds at most `forges.log_origins`
origins (`docs/design/protocol/configuration.md`, the limits).

## 9. Replace the App's key

temper cannot replace an App's key yet; neither **Forges** nor
`temper ctl` offers it in this build. A key GitHub refuses stops the
forge: "Stopped: WHY". The engine and every other forge run on. This
holds for temper's App and for an App of your own alike.

## 10. What temper keeps, and where

- **In jig's secrets' store,** under the forge's number, written and
  read by the engine alone: the App's id, its slug, its key as DER, its
  bot user's id, the installations accepted through temper's links or
  `forge-accept`, and the kind and address the records were made for
  (`docs/design/protocol/credentials.md`, the records).
- **Never the PEM.** The key you paste is kept by jig until its run
  ends, the pasted value, then erased; temper keeps it decoded, and no
  page, log or answer shows it (`docs/design/protocol/credentials.md`,
  pasted values). A run cut by a restart says "interrupted; paste
  again".
- **Never kept at all:** the installations' tokens, minted from the key
  about hourly and held in memory; the App's OAuth client and its
  secret; the webhook secret GitHub gives temper's App at its making;
  the links' states.
- **No webhook secret for GitHub.** The App's hook points at an address
  that never resolves, and temper reads its deliveries from GitHub's
  log, so your deployment needs no address GitHub can reach.
- **`temper ctl` reads a key from a private file:** your own, with no
  group or other permission bits, not a link. It never prints the key,
  and sends it once.

## 11. An App another owns: read-only

For a shared App, such as a probe App or your team's App, set
`"app_read_only": true` on forge 2 in `C/temper.json` before starting
the box. Keep the rest of that entry:

```json
{"number": 2, "kind": "github", "api": null, "transport": null,
 "git": null, "web": null, "log_origins": null, "app_read_only": true}
```

temper reads the App's hook and leaves it, never uninstalls an
installation it did not accept, and changes nothing of the App. Its
key, id, permissions and events are still checked. This mode protects
the App; accepted installations still let temper work on repositories.

```sh
temper ctl DIR forges
```

An installation not yet accepted is listed by id with
"not accepted" and "kept: the App is read-only". Accept the one you
want as in section 12; leave other installations to their owners.

## 12. Accept an installation that exists already

Use this path when the App was installed on GitHub before temper knew
it. Sign in through ctl with the box's link as in `temper help box`,
section 3. DIR is that ctl directory. For a shared App, first set
read-only mode as in section 11. Have its App ID and private PEM key
file, FILE, owned by you, not a link, with mode 0600.

Run these in order, replacing 12345 with the App ID, 67890 with the
installation id `forges` prints as "not accepted". Replace `OWNER`
and `NAME` with the repository owner and name `repositories` prints,
and project 1 with your project. The key names identify
these requests; keep each name on a retry, and choose new names for
new work.

```sh
chmod 600 FILE
temper ctl DIR forge-connect 2 github-app 12345 FILE --key github-app
temper ctl DIR wait forges --until connected:2
temper ctl DIR forges
temper ctl DIR forge-accept 2 67890
temper ctl DIR repositories 2 --installation 67890
temper ctl DIR adopt --project 1 --forge 2 --owner OWNER --name NAME --landing main --key github-adopt
```

Connect submits the App's key; the wait reports it connected. That
alone does not accept installations. `forges` names each installation
and its id. Accept prints `accepted: installation 67890 on forge 2`,
then the repository-listing call. It works without a browser, including
when that installation is accepted already. The repository list prints
`OWNER/NAME`; follow its next-page call if needed. If the box's Forgejo
repository is project 1's home already, add `--no-home` to the `adopt`
call above, or choose a new project to make GitHub its home. Adoption
prints the adopted repository, with CI on.

Without read-only mode, a stray is uninstalled at the next reconcile
once older than an installation link's lifetime. Accept it before
that reconcile, or use `temper ctl DIR forge-install 2` instead and
complete the installation through its printed browser link. That
alternative needs a browser; accepting an existing installation does
not.

Give the model an account as in `temper help box`, section 5, then ask
for a change in `OWNER/NAME` using the adopted project's number. Follow
section 6's `chat`, `task --tree`, `wait change` and `change` calls to
landing. The change prints the pull request's number and address, CI
at its head, its step and its merge commit. Read the diff, reviews,
CI runs and logs, and branches there with your chosen tools. What
temper owns there, its branches, pull requests and landing, changes
only through temper. Export a moved-in login before stopping the box
(section 7 of that walk).
