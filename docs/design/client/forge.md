# The forge in temper's client

Provisional, 2026-10-05, revised 2026-10-23. What the forge connector
adds to temper's client: a change's way to landing and its result, a
project's landing queues, approvals at a head, the forge's holds in
words, repositories with their health and landing rules, the picker of
repositories to adopt, goals, findings and landings as the forge shows
them, and the forge's part of the deployment, where each configured
forge is connected. jig's pages and cards (jig's `client/domain/pages.md`
and jig's `client/domain/objects.md`) leave a slot for each part a
connector shows (jig's `client/domain/root.md`, section 6), and
temper's pages and cards, its own after them, show the same parts
(`README.md`, section 1); this document says what the forge shows in
those slots and on the pages only temper has, `Changes`, `Adopt` and
`Forges` (`README.md`, 3.5), in temper's words. The forge's client
family, its words for resources and kinds, its adoption form and its
requests are `README.md`, sections 4 and 8. Each part shows what `../domain/forge.md` keeps or reads,
and links to the forge for code. What is still open is listed in
section 10.

## 1. In one page

- **In jig's slots, in the forge's words.** jig's pages show tasks,
  cards and holds; the forge fills the parts they name (a panel, a
  result, an effect, an outbox entry, a hold, a project's part, the
  deployment's part) on a task's page, in a chat, in the inbox, on a
  project's pages and on the deployment's (section 9). A part the
  forge's side does not read says "Shown on the forge." (`README.md`,
  4.2).
- **The first milestone needs two of them** (`../README.md`, section
  2): a change's result and its hold, in the forge's words, with a link
  to each on the forge (2.5).
- **A change is shown as its way to landing,** one step at a time, each
  with its reason in words: CI running, a review asked for, second in
  the queue, updating from main, resolving a conflict.
- **Queues are shown, not managed.** A landing branch's queue is its
  ready changes in order, on temper's `Changes` page; a person changes
  the order through goals' priorities on the board, never by moving
  changes about. Where a branch lands through GitHub's merge queue,
  temper's queue decides only what enters it and when, and the panel
  says "in GitHub's merge queue, n" (2.1).
- **Code stays on the forge.** Parts link to a pull request, its diff,
  its files and its failing jobs; the client shows states and verdicts,
  never code, and has no code review of its own.
- **Approvals are at a head,** given in the client as a person task or
  on the forge as a review (section 4): each counts at the head it was
  given at, and the panel says when a later head still carries it.
- **A hold says what happened on the forge,** and what each act would
  do; nothing on the forge is written over until the person decides
  (section 6).
- **What the forge did not tell temper is said:** a repository with no
  webhooks, protection temper could not read, a branch someone else
  moved, a job's log on a host temper does not read (5.4).
- **Forges are connected from the client.** Configuration names each
  forge, never its access; on the deployment's `Forges` page (5.4) a
  person whose role holds the deployment's connectors connects it:
  GitHub through an App made from temper's manifest, or one of their
  own, its key pasted; Forgejo with its bot's tokens pasted, or a site
  admin's token from which temper makes its bot. The page says what
  temper is there, what stopped it, and offers Replace.
- **Adoption picks from what temper can reach** (5.1): per connected
  forge, the repositories temper can reach there, a page at a time by
  number, and "Not listed" for one named by owner and name, with what
  would make it reachable. A forge not connected offers none.
- **The forge's refusals are said with their remedy** (5.5): not
  connected, not installed on an account, not granted to temper's App
  (linked to the installation's page on GitHub), stopped and why.
- **The browser's steps of a connect are temper's routes** on the
  clients' listener (9.1): the manifest's page, GitHub's redirect and
  its setup, each authenticated by a one-use state alone. A browser's
  tab goes to a connect's link and comes back to the page that sent
  it; the terminal prints the link and never opens it.

## 2. A change's panel

On a change task's page, as its panel: a procedure's activity is its
connector's panel (jig's `client/domain/pages.md`, 5.2). Folded to its
step in words, it is the change's phase wherever its chip shows (jig's
`client/domain/objects.md`, section 3): a chat's conversation, a goal's
plan, the inbox, the `Changes` page.

### 2.1 Its way to landing

The steps of `../domain/forge.md`, 8.2, with the one it is at and
why:

```
producing ─ opening ─ checking ─ gating ─ queued ─ first ─ landing ─ landed
                 repairing · updating · resolving · recreating · reopening
```

| Step | It says |
|---|---|
| producing | T21 producing it, as a chip |
| opening | opening its pull request |
| checking | CI on its head, and for how long |
| gating | which gates are running, as chips, and which requirements wait |
| queued | its place in its landing branch's queue, and who is ahead; handed to GitHub's merge queue, "in GitHub's merge queue, n", n its place there |
| first | first in the queue: updating from its base, CI on the updated head, or merging |
| repairing | why (CI's failure, a gate's remarks, a semantic conflict, taken out of GitHub's merge queue and GitHub's reason), the repair as a chip, and how many repairs are left |
| updating | the base it takes in |
| resolving | the conflicting files, the resolution as a chip, and how many are left |
| recreating, reopening | after drift released (6.2): its branch made again at the head temper last knew, or its pull request reopened or retargeted back |
| landing | merging at its head |
| landed | its merge commit, and when |

On the change's task page, the steps its panel carries (9.2) and their
words are `README.md`'s, 5.10: the steps as the mockup's six, each side
step at the one it serves, and a line for the step it is at, with the
task it runs as a link. What this table names beyond the panel (how
long CI has run, which requirements wait, when it landed) waits for
the panel to carry it (section 10).

Where the landing branch requires GitHub's merge queue
(`../domain/forge.md`, on merging through it), temper's own queue
decides only which change enters GitHub's and when: the first change's
merge hands it to GitHub's queue at the head its decision saw, and
until it lands or leaves, the panel says "in GitHub's merge queue, n",
n the position the forge reads for its pull request (`Pull.queued`) as
each read gives it. GitHub builds the merge group, runs CI on it and lands
it; the landed step names the commit the group landed (`Pull.merged`),
which is not the head temper saw. A pull request GitHub takes out of
its queue for any reason but its merge is repaired, as a failed CI is,
the repairing step naming GitHub's reason. Forgejo has no such queue.

### 2.2 What it shows

- **Its pull request:** number, title, head and base, state,
  mergeability, or that it is not known yet while the forge settles it
  (`../domain/forge.md`, 5.2); links to it, its diff and its files on
  the forge.
- **CI on its head:** the combined state and each context's, with a link
  to a failing job, and the failed step and the end of its log a repair
  was given, cut as the repair got it (`../domain/forge.md`, 8.4).
- **Its gates** (`../domain/forge.md`, 8.3): each with its kind (a
  review by a lens, a person's approval, checks), blocking or advisory,
  who asked for it (the change, the project's landing rule, its plan),
  its verdict and the head it was given at, and whether it carries over
  to the current head; the task running it, as a chip. An advisory gate
  that failed is reported and holds nothing, and the panel says so.
- **Its requirements:** the landing rule's, each met, waiting (and for
  what) or refused at the current head: CI at the head, up to date with
  the branch, its blocking gates, approvals on the forge by so many
  people of a role (section 4; `../domain/forge.md`, 8.7).
- **Its heads,** in order, each with what made it (produced, a repair, a
  clean update, a resolution) and its CI and verdicts, so the person can
  see why a gate is asked again: a repair's head asks every gate again,
  a clean update's carries them.
- **Its bounds:** repairs, resolutions and updates used of their limits;
  each wait against its stall: "CI has not reported for 25 minutes; held
  at 60".
- **Its delegates:** producing, repairs, reviews, resolutions, as the
  task page's plan (jig's `client/domain/pages.md`, 5.2).

### 2.3 What may be done

- **Amend its gates:** add a review by a lens or a person's approval, or
  drop one the change or its plan asked for; a landing rule's gate
  stays. A change's gates are its parameters, amended
  through the forge's form in the panel, the forge's own request (jig's
  `client/domain/root.md`, section 6). A gate added while it is queued
  takes it out of the queue until its verdict, which the form says.
- **Release** it, held (section 6; jig's `client/domain/objects.md`,
  section 5).
- **Cancel** it (jig's `client/domain/objects.md`, section 11): the
  panel says what the forge will do: its pull request closed with a
  comment saying why, its branch deleted, its commits still readable
  under the closed pull request; a merge already sent may still land,
  and then it ends landed (`../domain/forge.md`, 8.6).

### 2.4 A chat's small change

A chat that makes a small change itself delivers it to its run's own
branch, and delegates a change task handed that branch
(`../domain/forge.md`, 8.1 and section 11). In the conversation the
delegation shows as the change's chip (jig's `client/domain/objects.md`,
section 10), its phase the panel's step in words, following it through
CI, its gates and its queue to landing while the chat carries on. The
change's result reaches the chat as its delegate's result (2.5), and the
chat tells the person.

### 2.5 Its result, and why it stopped

What the first milestone needs of the forge's parts (`../README.md`,
section 2, steps 6 and 7):

- **Its result,** a connector's result (jig's
  `client/domain/objects.md`, section 8): landed, with its pull
  request's number and title, its merge commit (through GitHub's merge
  queue, the commit its group landed), and the address of each on the
  forge; or ended otherwise, with what became of its pull request
  and its branch. It shows on the change's page, in the chat that
  delegated it, as its delegate's result, and in the update of the
  person who asked for it.
- **Its hold,** the forge's hold part (jig's `client/domain/objects.md`,
  section 5): its reason in section 6's words, and the head and the
  failure it is about, each with a link on the forge, on the held card
  wherever it shows: the change's page, the chat, the inbox's
  escalation.
- **A repository named by owner and name,** in place of its numbers
  (`README.md`, 4.1), from the same reads.

Beyond it: the way to landing step by step (2.1 to 2.3), the queues
(section 3), health (5.2), what each act on drift would do (6.2), the
outbox and the deployment's part (5.4).

## 3. Changes and landing queues

temper's `Changes { project }` page (`README.md`, 3.5): a
project's changes, by landing branch (`../domain/forge.md`, section
9):

- **The queue:** the ready changes in order, each with its goal and
  priority and when it became ready; the first says what it is doing
  (updating, CI on its updated head, merging); a change past the queue's
  window is marked as going ahead of those ready after it.
- **The order's rule,** in a line: "by goal priority, then by when each
  became ready; ready for over WINDOW goes first", WINDOW the forge's
  queue window (`../protocol/configuration.md`, section 7).
- **Not ready yet,** by step: producing, checking, gating, repairing,
  resolving.
- **Held,** each with its reason (section 6).
- **The branch:** its tip, CI on it, and the landings recently made,
  paged.
- **Paused:** when CI fails on the branch's tip, "main fails at its tip
  since 10:02; T88 repairs it, first in the queue; the rest wait", with
  the repair as a chip. Past its repairs' bound, the queue is held for a
  person, whose card offers to release it as it is (a flaky check) or to
  cancel the repair.
- **Where temper may not merge,** the queue is still ordered for people
  to see, each ready change "waiting for someone else's merge".

## 4. Approvals

A person approves a change in one of two ways, as its gates and its
project's landing rule ask (`../domain/forge.md`, 8.3 and 8.7).
Neither is temper's default: a person's approval is a gate a plan or a
change is given when a person asks for it, or a project's rule
(`../README.md`, section 2).

- **In the client:** a person task addressed to the project's
  maintainers, one of whom takes it, whose contract is an approval of
  the change at a head (jig's `client/domain/objects.md`, section 7;
  jig's `domain/people.md`, section 8). Its card, in the inbox and on
  the change's page, says:
  - *what is asked:* approve this change at this head, and who asked
    for the approval: the change, its plan or a landing rule;
  - *the change at that head,* the forge's part: its pull request, CI at
    the head, the other gates' verdicts, what changed since an earlier
    approval if there was one, and a link to its diff on the forge;
  - *the answer:* approve, naming the head; or cannot do it, with a
    reason, which holds the change when the approval is blocking, the
    reason in the person's words (6.1).
- **On the forge:** reviews by people of a role, counted by the landing
  rule at the head or, where the rule allows, a clean update of it. The
  panel shows each approval counted, by whom and at which head, and how
  many more the rule asks; the person reviews through the panel's link.

If the change's head moves while the card is open, the card says so. A
clean update temper made carries the approval where its gate counts
clean updates (its freshness, `../domain/forge.md`, 8.3); any other
new head asks again, as a new person task at that head, and the old card
says it has been replaced.

## 5. Repositories, and the forge itself

### 5.1 Adopting one

The adoption form is `README.md`, 4.4, on temper's `Adopt` page,
and its outcome 4.7. With the forge's documents served, the form chooses
among the configured forges by name, says whether the project has a
home, and, before the adoption is confirmed, shows what temper found
there (`../domain/forge.md`, section 4): temper's permission, the
merge styles the repository allows and so whether it may merge, the
protection on the landing branch (or that it could not read it), its
webhooks or none, CI or none, the people it knows there with the role
each would be given, and the ceiling that follows, in words.

What the adoption names there: the repository's role (owned, where
temper pushes and lands; a fork of an upstream; or context, read only),
its landing branch, temper's branch prefix (`temper/` by default),
whether it is the project's home, and whether it runs CI or the checks
its changes are gated on without it. The merge style is the forge's for
the repository, which the form shows before the adoption is confirmed:
on GitHub the first the repository allows of merge, squash and rebase,
on Forgejo its default (`../domain/forge.md`, what adoption says of the
branch's rules). A repository where another
deployment's branches already live under the prefix is refused, saying
so (jig's `domain/connectors.md`, section 12).

**The picker.** Below the forge's choice, for the chosen forge, the form
lists the repositories temper can reach there (the `Reachable` request,
`README.md`, section 8), a page at a time by number from 1, each page
at most `picker.page` rows (`../protocol/configuration.md`, its limits)
clipped to the forge's own page cap, with "Next" while the answer says
more follow, which it says when the page is full. The request names no
project, so the project's own repositories are among the rows; adopting
one again is refused as any adoption is (section 10). Each row is
OWNER/NAME, marked "private" where it is; the repository's id and its
owner's id the row carries ride in the adoption request and are never
shown. Choosing a row fills the owner and the name. What temper can
reach:

| The forge | Listed |
|---|---|
| GitHub | one accepted installation's repositories of temper's App (GitHub's `GET /installation/repositories`, on a token the setup's host mints for the page and drops), the installation shown by its account's login and asked for by its id, which the picker's request carries, the installations as the deployment's page reads them (5.4), marked "selected repositories" where its installation has chosen repositories and "suspended" where GitHub suspended it; read by the setup's host's `Reachable { forge, installation, page }` plan (skein's `forge-setup.md`, 4.8) |
| Forgejo, connected | the repositories its API token can push to, read by the same plan, with no installation |
| Forgejo, managed or owned | every repository the site admin can see, read by the setup's host's `Reachable { forge, installation?, page }` plan on the admin's token (skein's `forge-setup.md`, 4.8), by id ascending, its `reachable` rows given from `picker.page`, since adopting one grants the bot its access (`../protocol/forge.md`, the setup's host's `Reachable` and `Adopt`); the admin's token stays the setup's host's, and the picker gets the rows alone. The plan runs beside the forge's one writing run; a second read of the picker while one runs is refused busy, and so is one past `setup.runs`, which counts both. Its own refusals are the admin's token's, as the reconcile's first step has them (`forge-setup.md`, 4.3): "the admin's token is missing" and "NAME is not a site admin" |

A forge not connected offers none: the picker says "Forge F is not
connected." and links to the deployment's page (5.4), and the "Adopt"
button stays disabled for it. A suspended installation's repositories are
listed and marked; adopting one is refused as no access until GitHub
unsuspends it.

**Not listed.** Below the list, "Not listed" takes a repository's owner
and name, and the form says what would make it reachable, by the forge
and what its read answered:

| Where | The form says |
|---|---|
| GitHub, an account whose installation has selected repositories | "Grant OWNER/NAME to temper's App on GitHub's installation page.", linked to that installation's page (5.5) |
| GitHub, an account with no installation of temper's App | "temper's App is not installed on OWNER.", with "Install on another account" (5.4) |
| Forgejo, connected | "Make LOGIN a collaborator with write on OWNER/NAME.", LOGIN the bot's |
| Forgejo, managed or owned | nothing: adopting it grants the bot its access |
| any forge | a repository its credential can read may be adopted as context |

Once the forge says so (an installation's repositories changed, a
collaborator added), the repository is listed on the picker's next
page, with no restart.

**The rules, in words** (`../domain/forge.md`, on what adoption reads):
the landing branch's rules as the forge applies them, each with what it
means for landing: a merge queue (temper enters it, 2.1), required
approvals (a person approves, or temper's App may bypass), required
status checks (CI as the landing rule's), code owners (a person
reviews), signed commits (temper's commits are unsigned, so where the
forge refuses its merge for it, its landings wait), linear history (a
merge style that keeps it), and whether
temper's App may bypass the rules. Where a rule keeps temper from
landing, the form names the one setting on the forge that would let it
land, for the person to choose; until it is changed, its ready changes
wait for someone else's merge (section 3). temper never changes a
repository's rules. On GitHub temper writes no commit status
(`../domain/forge.md`, 4.4), so every GitHub adoption says "Commit
statuses stay temper's own: GitHub's rules cannot require them."
Forgejo's reads answer none of the merge queue,
signed commits, linear history and code owners, and no bypass, so on
Forgejo the form says its branch protection alone.

### 5.2 Health

Wherever a repository is shown (the project's page, its settings, the
deployment's page):

- **what temper may do** there, and whether that narrowed since adoption
  ("a merge was refused: temper may no longer land into main");
- **its webhooks:** when they were last heard, on GitHub the App's
  deliveries as its delivery log records them (`../protocol/forge.md`,
  a GitHub App's delivery log); none, or none for days, means changes
  are seen by polling alone, more slowly;
- **its landing branches:** their tips and CI on them;
- **on GitHub, its installation:** the account's, marked suspended
  while GitHub suspends it; a repository that left the installation is
  marked "not granted to temper's App", linked to the installation's
  page (5.5);
- **what is held** on it for drift (6.2).

### 5.3 In a project's settings

- **Resources:** each repository of the project, as the forge's part in
  the settings' resources (jig's `client/domain/pages.md`, 6.5): its
  role, whether it is the project's home, its landing branch, temper's
  branch prefix there, its merge style, CI or the checks that stand for
  it, what temper may do there in `README.md`, 4.7's words, and its
  health (5.2). Adopting one is the form of 5.1; where the form lives is
  `README.md`, section 11's.
- **The landing rules,** in the policy's requirements (jig's
  `client/domain/pages.md`, 6.5), per pattern of landing branches, the
  deployment's and the project's, in words: "into main in acme/shop: CI
  passed at the head; up to date with main; a review for memory safety,
  at the head or a clean update of it; two maintainers' approvals on the
  forge" (`../domain/forge.md`, 8.7).

### 5.4 The deployment's page

The forge's part of the deployment's connectors (jig's
`client/domain/pages.md`, section 7), on two pages of temper's
(`README.md`, 3.5; with the user, 2026-10-23): `Forges`, which manages
the forges the person has, and `ConnectForge`, a flow that connects one
more. Each is read for a person whose role on project 0 is `Owner`, in
standalone the deployment's owner; any other's read is refused, in
jig's words (`README.md`, section 8). Their mockups are `forges.html`
and `connect-forge.html`.

- **The Forges page** lists the person's forges: each configured forge,
  fakes aside, whose connection is setting up, set up failed, connecting,
  connected or stopped, or not connected where its records say why. A
  forge not connected or not set up, with nothing to say, is not
  listed: it is a choice of the flow. After the page's level-1 heading
  "Forges", the level-2 heading "Your forges", with their count, the
  line "Each forge temper is connected to, or setting up. Add one to
  bring its repositories to temper." and "+ Add a forge", a link to
  `ConnectForge`; with none, "No forge of yours yet." and "Connect a
  forge", the same link. Each row is a disclosure, folded to its
  summary: its name, "forge N · KIND · WEB" (its number, its kind and
  its `web` address), its connection's words (below) and its line:
  connected, its repositories, "OWNER/NAME · project PROJECT" each;
  setting up, its set up's first repository, "ORG/NAME, into PROJECT";
  set up failed or stopped, why. Opened, it holds the rest of the row's
  parts below and its actions. Every row starts folded.
- **"Open Forgejo ↗"** on a connected Forgejo that signs its people in
  through temper (`../protocol/sign-in.md`, section 9): a link to the
  forge's `ROOT/user/oauth2/temper`, `ROOT` its `git` URL, in a new tab,
  and under the actions the line "Open Forgejo signs you in as LOGIN,
  through temper: Forgejo’s sign-in is temper’s.", LOGIN the member's
  login. Forgejo starts its sign-in with the temper source, temper
  answers at once, and Forgejo opens signed in.
- **A forge's name:** "Forgejo on this machine" for an owned Forgejo,
  "GitHub" for GitHub, "Forgejo at HOST" for another Forgejo, HOST its
  `web` address's host, and "Playground forge" for the demo's
  (`README.md`, 5.6). PROJECT is a project as the banner names it
  (`README.md`, 5.1).
- **What the row carries today** (`README.md`, 4.6): its number, kind,
  mode, connection, offer, its first repository, on GitHub its
  installations, and its last connect's end. Until it carries
  its `web` address and the repositories adopted on it, each with its
  project (a growth of the `forges` subject's row, and `CLIENT_VERSION`
  with it), the summary is "forge N · KIND", another Forgejo is named
  "Forgejo", and a connected row's line is its first repository once
  made, "ORG/NAME · project PROJECT", or none. Opened, a row holds its
  Account line, connected ("LOGIN · user ID"), and its actions as
  offered today: "Set up again" on set up failed, "Open its set-up ›",
  a link to `ConnectForge` at it, while it sets up or connects, an
  owned forge's restart words when stopped, and, connected, "Adopt a
  repository", a link to `Adopt { project }` for the project the person
  is in. On GitHub it holds, connected, the App's line (below, "On
  GitHub, temper's App"), each installation by its account's login, or
  "installation ID" while the forge has not heard its account, with
  "suspended" and "uninstalled at the next reconcile" where they hold,
  never yet "all repositories" or "selected repositories" nor a link to
  the installation's page, which wait on the forge hearing an
  installation's selection (skein's installation events carry none);
  and "Install on another account", which sends `Install` with `back`,
  the Forges page's address, once an installation is accepted, or
  "Install on GitHub" before (the actions' table, below). Every row
  holds its last connect's line where it has one (the row's table,
  below). Its Access and Runs lines, its load, the App's setup URL, the
  `Lacking` line, and the actions that carry a pasted value but
  Connect wait on their own work (`../STATUS.md`).
- **The playground's forge,** the fake the demo names, follows under
  the level-2 heading "The playground", folded, while the demo's forge
  half holds (`README.md`, 4.8): "Playground forge · the playground’s,
  until a forge of yours connects · OWNER/NAME"; opened, its Account
  line. Once the demo has ended (`../draft/first-run.md`, 2.3), it
  shows nowhere.
- **Connecting a forge,** `ConnectForge { forge }`, is four steps,
  named along its top: Choose, Details, Setting up ("Connecting" for a
  Forgejo elsewhere) and Ready. GitHub's, through temper's App, is
  three, Choose, Connecting and Ready, with no step of temper's
  between Choose and GitHub; an App of the person's own adds Details
  (with the user, 2026-10-23). `forge` is none at Choose,
  or the choice: a forge's number, or "a Forgejo elsewhere"
  (`browser.md`, 2.3). The step shown is the choice's: none, Choose;
  a forge whose row offers its connect or set up, or a Forgejo
  elsewhere, Details (on GitHub, an App of the person's own); a forge
  setting up, set up failed or connecting, Setting up, or Connecting;
  a forge connected, Ready, but a GitHub forge with no accepted
  installation, Connecting (below). A number that names no row, or
  the demo's forge, is Choose. The page's level-1 heading is "Connect a
  forge", above the steps, and each step's content is in one card,
  under its level-2 heading.
  Leaving the flow leaves what runs running; the banner then says it.
- **Choose** is a card for each forge offered to connect, in this
  order, each with its level-2 heading, its words, a muted line and
  its link, "Choose" (a Forgejo elsewhere's "Show how"):
  - *GitHub,* a GitHub forge not connected: "Your repositories on
    GitHub. temper makes its own App there; you choose the account and
    the repositories it may use." · "In this tab, on GitHub · about two
    minutes"; in its link's place the button "Connect GitHub", which
    sends `Connect` (below) and takes the tab to GitHub, and beside it
    "Use an App of your own ›", a link to its Details. Nothing asks for
    an organization: the App is made under the person's own account,
    and GitHub's install page offers every account they may install it
    on;
  - *Forgejo on this machine,* an owned Forgejo not set up whose offer
    is offered or not under the box (`README.md`, 4.6): "A forge of
    your own, run by temper box. No account needed: temper makes your
    first repository, and you start in it." · "Here · about a minute",
    or, not under the box, "Needs four commands and a restart";
  - *Forgejo at HOST,* a configured Forgejo whose `admin` is true and
    which is not connected: "temper connects as its site admin, makes
    its bot and your first repository." and "Connect as its admin".
    HOST is `row.web` when present, otherwise "forge N". The legacy
    managed object configuration has `admin` false and offers no card;
  - *A Forgejo elsewhere,* always: "Your own Forgejo server. temper
    connects as its admin, or with its bot’s tokens." · "Needs a change
    to temper’s configuration and a restart".
- **Details** are the choice's, under its level-2 heading, after "‹ All
  forges", a link back to Choose:
  - *GitHub,* an App of the person's own, by "Use an App of your own
    ›": "An App of your own", "Its id, and its private key as GitHub
    gives it.", "On GitHub, set the App’s setup URL to temper’s address
    followed by /forges/N/github/setup, and give it the permissions and
    events temper’s guide lists." (the client knows no address of its
    own to print), its fields "App id" and "Private key" and its
    button, "Connect the App"; then "A pasted value is sent once and
    shown nowhere, then or after." "App id" is a line of digits, the
    App's id as GitHub shows it; "Private key" is a secret text field,
    one line, where the person pastes the PEM file's text as GitHub
    gives it. A browser draws it as a password input, masked and never
    spellchecked, and strips its line feeds; skein's builder marks only
    a text field secret, and a text area would show the key and offer
    it to the browser's spellcheck. So the client restores the armour
    alone: the text trimmed of spaces, tabs, CRs and LFs at both ends,
    one LF put after `-----BEGIN RSA PRIVATE KEY-----` and one before
    `-----END RSA PRIVATE KEY-----` where none is, nothing else changed
    (the body's line breaks matter to no one: skein's parse ignores
    them); a text that is not that armour goes as it is, for `Paste` to
    refuse. "Connect the App" sends `Connect { forge, how: GitHubApp {
    id } }`, the key its carried value, a bundle of one `Pasted` item
    of that text (`../protocol/credentials.md`, 2.2), and empties the
    key's field as it leaves (below, a pasted value). The bundle holds
    4 KiB; the setup host takes an RS256 key of at most 1196 DER bytes
    (skein-tls's `RS256_KEY_BYTES`: 2048 bits, the size GitHub gives),
    about 1.7 KB of PEM, and refuses a larger one at `Paste`, in words
    that never repeat the key. A secret text area, its line feeds kept,
    is skein's to offer later (10). The
    page's own refusals, before anything is sent, the fields kept: a
    field empty, "Required."; an id that is not a number from 1 to
    2^64 − 1, "An App's id is a number, as GitHub shows it on the App's
    page."; the bundle past 4 KiB, as below. Answered connecting, the
    flow follows the row: Connecting while the run holds the forge,
    then, connected, Connecting's install (below); a run that stopped
    leaves the row not connected with its last connect's line (the
    row's table, below) shown under the button, the key asked for
    again. Refused, the refusal shows under the button, the id kept:
    `role`, "Only the deployment's owner connects a forge."; `no such
    forge`, "No forge N is configured."; `not taken`, "Forge N does not
    take an App."; `connected`, "GitHub is connected already."; `busy`,
    "temper is busy with forge N: try again when it ends."; `no key`,
    "The key did not reach temper: paste it again."; unknown past the
    retention, "It is not known whether the App was connected: see
    Forges.". Its install is Connecting's, as for temper's App.
    The terminal shell has no secret field of several lines: its
    Details for an App of your own say, in place of the fields and the
    button, "Paste the key on this page in a browser, or run temper
    ctl forge-connect N github-app ID FILE, FILE the key's file,
    readable by you alone." (`tools.md`, 5.5);
  - *Forgejo on this machine,* under the box: "temper box downloads
    Forgejo and its runner, starts them, and makes your first
    repository, with a README and a CI workflow. No restart."; the set
    up's fields, its organization (default `work`), its repository
    (default `hello`) and its project, a new one named as the
    repository; "Its CI jobs run on this machine, as you, with no
    isolation." and "Forgejo stops when temper box stops. Its
    repositories are kept in temper’s data directory."; and "Set up a
    Forgejo here", which sends `SetUp` and, once accepted, shows
    Setting up. The fields: "Organization", `work` until changed;
    "Repository", `hello`; and "Into a new project, named", the
    repository's name until changed: the first run offers no existing
    project (`../draft/first-run.md`, 2.4). An organization, repository
    or project's name left empty is the page's refusal,
    "Required.". The set up's refusals show under the button in its
    words (`README.md`, 4.6): "a new project", until jig makes projects
    at runtime, and `invalid`, which also marks the organization and
    repository fields invalid, the fields kept for the person to fix.
    "Set up again" sends the fields the last `SetUp` sent. Not under
    the box: "Set it up from a terminal", "temper was not started by
    temper box, so it does not start a Forgejo itself. Run these, keep
    the last two running (a service manager is best), then restart
    temper:", the four commands of the row's not set up (below), and
    no action;
  - *A Forgejo elsewhere:* "Add its address to the configuration and
    restart temper box. Then connect as its admin, and temper makes and
    keeps its bot, or paste its bot’s tokens."; the entries to add to
    the document, each in a block of code: its forge in
    `application.forges` (`number` the next one unused, `kind`
    `forgejo`, its `api`, `transport`, `git`, `web` and `log_origins`
    for a host `forge.example`), its git identity in
    `application.identities`, and, to connect as its admin, its setup
    entry in `application.setup` (`mode` `managed`, its bot); "Without
    it, temper takes its bot’s tokens only."; then "Restart temper box,
    then connect it from this page." These words stay: a Forgejo
    elsewhere is added by hand and a restart (`../draft/first-run.md`,
    section 4).
  - *A configured admin Forgejo:* "Connect as its admin", then "Paste a
    site admin’s token with the scopes write:admin, read:user,
    write:organization and write:repository. temper keeps it, and uses it
    only to make its bot and your first repository, and to give the bot
    access to it. To stop temper using it, delete the token on Forgejo."
    "The site admin’s token" is a password field, passed straight to a
    `Secret`, outside text actions. "Your first repository" has
    "Organization" (`work`), "Repository" (`hello`), "Into a new project,
    named" (following Repository until edited), and "Runner label"
    (`docker`). Its hint is "Its CI runs on your forge’s runners with this
    label. Forgejo’s runner registers as docker unless told otherwise."
    The button "Connect as its admin" sends `Connect { forge, how:
    ForgejoAdmin { first: Some(FirstAsk) } }`, with the four fields and
    the token carried as item 1, a single-item bundle with its six-byte
    header, at most 4 KiB. No RSA armour restoration applies. Sending
    empties the token field; a stop empties it again and keeps the other
    fields. Before sending: an empty token or name, "Required."; a bundle
    past its bound, "The pasted values are past 4 KiB, the most a request
    carries."; a label outside `[A-Za-z0-9._-]{1,64}`, "Letters, digits,
    dots, dashes and underscores, at most 64." Sign-out erases the draft.
- **Connecting, as a Forgejo's admin,** says "Connecting forge F as its
  admin." and "Making its bot, then ORG/NAME." The welcome keeps Forge
  highlighted. A stop returns to Details with its token empty and the
  other fields kept. `ended_stop`, the row's optional `AdminStop`, selects
  the words; a refused stop carries its status and `SetupStage`. `ended` is a
  diagnostic, never parsed to decide the stop:
  - `NotAdmin`: "The token you gave is not a site admin’s. Paste one a
    site admin made.";
  - refused 401: "Forgejo refused the token. Check that it was copied
    whole, and that it hasn’t been deleted.";
  - refused 403: "The token lacks a scope: STAGE was refused. Make one
    with write:admin, read:user, write:organization and write:repository."
    STAGE names the actual operation, for example "making the bot";
  - `Unmarked`: "A user temper didn’t make holds the bot’s login, without
    temper’s mark: add temper:bot to its full name on Forgejo, or change
    the bot’s login in the configuration.";
  - another stop: the `ended` line.
  A failed `first` names ORG/NAME and its failed step (admin, organization,
  repository or starter workflow), with the diagnostic and "Paste the
  token again to finish it." Every stop says "temper kept nothing of the
  token." No "Set up again" action is offered.
- **Ready, on that Forgejo,** says "Connected as LOGIN (id ID), managed."
  and "Made ORG/NAME, and its project PROJECT. Its CI runs on your forge’s
  runners labelled LABEL; with none, a change’s checks wait." LABEL is
  `first.label`, including after reload. "Open PROJECT" waits for the
  existing keyed project creation and adoption, once; a repeated row
  starts neither twice. With `row.web`, "Open ORG/NAME on Forgejo ↗"
  links to the repository. After reload, the durable adoption supplies
  the project for Open.
- **Setting up** shows the set up as its row says it: the level-2
  heading "Setting up Forgejo on this machine", its first repository's
  line, "ORG/NAME, into PROJECT", then its steps as a list, each with
  its state in words, "done", "under way" or none for one to come:
  "Fetching Forgejo and its runner" ("Fetched Forgejo and its runner"
  once done), "Preparing: its database, temper’s admin, your account",
  "Starting Forgejo and its runner", "Connecting as temper", and
  "Making ORG/NAME, and adopting it into PROJECT". The box's step
  names the one under way (fetch, prepare or run); none named yet, no
  individual step is marked. Connecting is the fourth, making and
  adopting the fifth, done only once the first repository is adopted
  into its project. Before the target is known, the fifth reads
  "Making the first repository, and adopting it into its project".
  GitHub rows never list Forgejo's steps. The versions and the
  person's login stay off until the row carries them. Then "You may
  leave this page: the steps go on, and the banner says where they
  are." A failed setup keeps all five steps: those before the failed
  step are done, that step is marked failed (✕), and those after it
  are to come. After the list, "STEP failed: WHY", STEP one of
  "Fetching", "Preparing", "Starting", "Connecting" or "Making
  ORG/NAME". The first four read the row's `at`; the fifth reads
  `first.state`'s failed creation step. With no step named, the list
  has nothing marked and the line is "Set up failed: WHY". The welcome
  keeps the same list and failure line. Then "Set up again", which
  sends `SetUp` with the same fields. While a `SetUp` is pending its button is disabled;
  refused, its refusal shows under it in the set up's words
  (`README.md`, 4.6).
- **Connecting, on GitHub,** is the App made and installed nowhere:
  a GitHub forge connected with no accepted installation, which the
  row also says as connecting (below), since it reaches no repository
  yet. The level-2 heading "Install temper’s App", then "SLUG is made
  on GitHub, under your account, and installed nowhere yet. Install
  it, and choose the repositories temper may use." and "Install on
  GitHub", which sends `Install` and takes the tab to GitHub's install
  page, as Connect GitHub does; with `back`, the flow's own address,
  so that the setup route sends the tab back to it, where the accepted
  installation shows Ready. The person meets it only by coming
  back to temper before installing: GitHub's redirect takes the tab
  from the App's form straight to that page. An App of the person's
  own meets it always, since its connect has no redirect. The row's
  `app` says which (`README.md`, 4.6): `Manifest`, the words above;
  otherwise, an App pasted or one connected before a restart, since
  the two end in the same records (`../protocol/credentials.md`, 2.1),
  "SLUG is connected to temper, and installed nowhere yet. Install it,
  and choose the repositories temper may use.". SLUG is the
  connected login without its `[bot]` (the App's bot user's login is
  its slug and `[bot]`). The link `Install` answers is where the tab
  goes, as 5.4's last paragraph says; refused, its refusal shows under
  the button in `ForgeRefusal`'s words (Details, above, and `not
  connected`, "Connect the App first."). A connect whose App is
  not made yet shows nowhere: its tab is GitHub's, no run holds the
  forge, and Connect GitHub again draws a fresh state, the first
  ending unused (9.1).
- **Ready** ends on the work: "ORG/NAME is ready", the level-2 heading,
  then "In PROJECT, on FORGE.", FORGE its name, the steps all done,
  "Open PROJECT", a link to the chats page in that project, and "FORGE
  is on the Forges page, with its details.", "Forges" a link to it. Its repository is
  adopted by the client that asked the set up: once the row says the
  first repository made, the forge's side, which keeps the `SetUp` it
  sent, sends `Adopt` of it into its project, once, on whatever page
  the person is (`../protocol/setup.md`, 6.7), as `temper ctl
  forge-setup` does (`tools.md`, 5.5): keyed `first-repository-F`, F
  the forge's number, as the project's home unless it has one, so that
  the two never adopt it twice; and "Open PROJECT"
  waits for its outcome; refused, the adoption's outcome shows in its
  words (`README.md`, 4.7). A client that did not send it (another
  tab, or after a reload) shows Ready on the row's made alone. With
  no first repository on the row (a set up before first-repo's wire,
  or one a restart has dropped): "FORGE is
  connected", "Connected as LOGIN.", "Adopt a repository", a link to
  `Adopt { project }`, and the Forges line.
- **Ready, on GitHub,** until the picker's `Reachable` is built, is
  Ready's with no first repository (above): "GitHub is connected",
  "Connected as LOGIN.", the App's line of the row (below), "Adopt a
  repository", a link to `Adopt { project }`, and the Forges line.
  With the picker, it starts with the repository to start in, while
  none of the forge's is adopted: "Choose a repository to start in",
  "SLUG is installed on ACCOUNT, with all its repositories, N." or
  "…, with N repositories you chose.", then the repositories of its
  accepted installations as one choice, each "OWNER/NAME", marked
  "private" where it is, the first chosen (`Reachable`, each
  installation's, as the picker reads them, 4.3); "Into a new project,
  named", a field, the chosen repository's name until changed;
  "Adopt OWNER/NAME", which sends jig's `MakeProject` of that name,
  then `Adopt` of it into that new project, as its home (`README.md`, 4.7), and refused "a new project" until
  jig makes projects at runtime, as Set up's is; and "Not listed?
  Choose its repositories on GitHub, and GitHub sends you back here.",
  whose link sends `Install` and takes the tab to GitHub's install
  page, where an account installed already offers Configure. Once one
  is adopted: "OWNER/NAME is ready", "In the new project PROJECT, on
  GitHub.", what its adoption found of its CI and its branch's rules
  (`../domain/forge.md`, section 4), "Open PROJECT", "Open on GitHub
  ↗", the repository's page in a tab of its own, and the Forges
  line.
- **Elsewhere:** while a forge sets up or connects, the banner says it,
  "FORGE · CONNECTION", its name and its connection's words, with
  "View", a link to the flow at it (`README.md`, 5.1); the navigation's
  "Forges" carries a dot, amber while a forge sets up or connects, red
  while one needs the person: set up failed, stopped, or not connected
  with a reason (`README.md`, 5.3). The dot's words, the entry's state
  in words: "a forge is setting up" or "a forge needs you". The
  demo's forge is none of them.

A row for each forge, by its number, its kind and its `web` address,
says:

- **Its connection,** one of:

  | State | The row says |
  |---|---|
  | not connected | "Not connected", with Connect; where its records say why, "Not connected: its records are unreadable" or "Not connected: its records were made for KIND at ADDRESS", and connecting again overwrites them (`../protocol/forge.md`, at start) |
  | not set up | an owned forge not prepared yet (`../protocol/setup.md`, 6.5): under the box, "Not set up", with "Set up a Forgejo here"; without it, "Not set up: run `temper forge fetch N`, `temper forge prepare N`, `temper forge run N forgejo` and `temper forge run N runner`, then restart `temper serve`", N its number, with no action |
  | setting up | the box runs its steps, asked: "Setting up: fetching", "Setting up: preparing" or "Setting up: starting", by the box's last word of the step it runs, else "Setting up" (`../protocol/setup.md`, 7.1); then "Connecting", as its handover is taken and reconciled (`../protocol/setup.md`, 5.1) |
  | set up failed | "Set up failed: STEP", naming the box's step, or, after the box's `started`, the engine's step that stopped (taking the handover, reconciling, removing the handover file; `../protocol/setup.md`, 6.5), with `temper box`'s output for the rest until its `failed` line carries a reason; with "Set up again"; the engine and every other forge run on |
  | connecting | "Connecting": a run holds the forge, and what it waits for: a managed or owned Forgejo's reconcile, the forge's answers to a check, or GitHub's conversion of its App; on GitHub, also connected with no accepted installation, "Connecting: installed nowhere yet", with "Install on GitHub". The person's browser on GitHub is no run, and shows nowhere |
  | connected | "Connected as LOGIN", the user the records hold: the bot's on Forgejo, the App's on GitHub; its id on an Account line |
  | stopped | "Stopped: WHY", the component's reason in words (its key or token refused, a user other than its own, an access it finds invalid, a refusal that asking again would not mend), with Replace where offered; an owned forge says "Restart temper box: its preparation mends it"; the engine and every other forge run on |

  A connect that ends otherwise is said on the row until the next one,
  its last connect's line, "The last connect stopped: WHY": a run of a
  `Connect` or an `Install` that stopped, WHY the setup host's reason
  in words, at most 256 bytes, kept by the forge domain in memory per
  forge until that forge's next connect or install starts or it
  connects, and dropped by a restart. A refusal before any run (the
  person's role, the forge's kind or mode, "busy": the forge's one
  writing run is taken, or `setup.runs` runs are running) is its
  request's outcome, shown by the page or the tool that sent it, and
  never on the row. The reasons a run stops with include
  "the pasted value changed", "the API token and the git token answer as
  LOGIN and LOGIN", "LOGIN is not temper's bot: add `temper:bot` to its
  full name on Forgejo" (5.5), the forge unreachable, a `Manifest` or an
  `Install` that stopped, its failure said here and never in its answer
  (`README.md`, section 8), or "interrupted; paste again" after a
  restart, once the connect's own record is kept; until then a restart
  drops a running connect unsaid, its carried value erased at start
  (`../protocol/credentials.md`, 2.2), and the forge reads as its
  records say. A GitHub App lacking what temper needs connects all the same
  (the setup's host's `Lacking` is done): the row says it beside
  "Connected as LOGIN", each permission and event named (5.5), until the
  forge's next check of the App ends: a paste's (a connect or a Replace)
  or the reconcile after each connect, the one at each start among them
  (`../protocol/setup.md`, 17.5), which clears the line or names what
  the App still lacks. The line is kept in memory alone: a restart drops
  it, and the start's reconcile names it again while the App still lacks
  it. A forge other than a fake, connected while a worker's `git` is
  null (`../protocol/configuration.md`, section 4), says beside
  "Connected as LOGIN": "No git on this machine: install git 2.41 or
  later and set workers[].git, then restart"; its repositories' commands
  fail on that worker, naming the forge's host, until it has one
  (`../protocol/git.md`, section 3).
- **Its first repository,** when its set up asked for one
  (`../protocol/setup.md`, 6.7): "Making ORG/NAME" from the ask until
  it is made, "ORG/NAME made, in PROJECT" once it is, or "Making
  ORG/NAME failed: STEP", the step one of "checking the admin's
  token", "making the organization", "making the repository" (its
  bot's access with it) or "adding the starter workflow", beside "Set
  up failed". It is the engine's memory alone: a restart drops the
  line. The set up flow's Ready step and `temper ctl forge-setup`
  adopt the made repository into its project (`tools.md`, 5.5).
- **Its mode** (`../protocol/setup.md`, the modes): owned, where the
  configuration's setup owns the forge; managed, where temper keeps a
  site admin's token for it; connected otherwise.
- **Its Access line,** on Forgejo: "Site admin, with temper's own token"
  for owned; "Site admin, with a token you gave" for managed; "Its bot's
  tokens, which you gave" for connected. The setup host alone reads an
  admin's token, never the forge component.
- **Owned's Runs line:** Forgejo and its runner on this machine, their
  versions, and CI jobs on this machine as the person. Before set up,
  the card says CI jobs run here unisolated, Forgejo stops with `temper
  box`, and it is kept in temper's data directory. Until signing in
  through temper, the card names the `person_password` file's path,
  never the password (`../protocol/setup.md`, 6.5).
- **On GitHub, temper's App:** a line labelled "temper’s App", "SLUG ·
  INSTALLATIONS", INSTALLATIONS each installation's words joined by
  ", ", or "installed nowhere yet" with none: its slug, and its installations, each
  with its account's login, "all repositories" or "selected
  repositories" (linked to the installation's page, 5.5), "suspended"
  while GitHub suspends it, and a stray, an installation never accepted
  through temper's link, "uninstalled at the next reconcile". When
  connected, it shows the setup URL the person must set on GitHub:
  `http://<listener>/forges/<n>/github/setup`, with redirect on update,
  no callback URL and no user's authorization at install
  (`../protocol/setup.md`, 17.3).
- **A job's log on an origin temper does not read:** "A job's log is on
  ORIGIN, which temper does not read: add it to this forge's
  `log_origins` and restart." (`../protocol/configuration.md`, the
  forges' members), until a start names it: the forge's `Fact::Origin`
  (`../protocol/forge.md`, 3.6), kept per forge in memory, at most
  `log_origins` of them, and forgotten when the forge next connects. The
  same line is in the engine's log, once until a start names the
  origin.
- **Its load:** its request budget and what uses it (fresh reads,
  writes, keeping up, the slow pass; `../domain/forge.md`, 5.4); a
  pause for its rate, and until when; its outbox, the effects waiting
  and those uncertain, being looked for; and when each repository's
  webhooks were last heard.

Its actions, each a request of the forge's family (`README.md`, section
8), offered by kind, mode, setup and state. The engine domain decides
every offer; the client reads it (`../domain/forge.md`, 4.1). "A Forgejo
elsewhere" names the managed setup entry needed for "Connect as its
admin", with its forge number and bot (`../protocol/setup.md`, section
8). The bot-token paste is not offered by the current client:

| Action | Offered | What it does |
|---|---|---|
| Connect | GitHub, not connected or stopped | "Connect GitHub", with `back`, the page's address to come back to: the answer is a link to the manifest's page on the clients' listener (9.1), and the browser goes to it in its own tab; the page posts temper's manifest to GitHub, the person creates the App under their own account and installs it on an account GitHub offers, and the setup route sends the tab back to `back`. Or "Connect an App of your own": its id, and its private key pasted as GitHub gives it; no webhook secret is asked, since temper points the App's hook at `https://SLUG.invalid/` and hears changes from its delivery log |
| Connect (not offered) | Forgejo through its bot's tokens | its bot's API token, its git token and, where the person made hooks to temper's listener, their secret, pasted together; both tokens must answer as one user |
| Connect | Forgejo with `admin` true, not connected or connecting | "Connect as its admin": a site admin's token pasted, with the first repository, project and runner label (default `docker`); Reconcile makes or finds the marked bot, then Create makes the repository; only after Done is the admin token kept and the forge connected; never `TakeOver` (`../protocol/setup.md`, 5.5) |
| Replace | GitHub, connected or stopped; a Forgejo of the connected mode, connected or stopped | a new App key, or a new API token, git token or both, pasted, which must answer as the forge's user (on GitHub, the same App); the old is erased once the new is kept |
| Install the App; Install on another account | GitHub, connected: "Install on GitHub" before any accepted installation (Connecting), "Install on another account" after one, and Ready's "Choose its repositories on GitHub" | a fresh link to temper's App's install page, with `back`, where the browser goes in its own tab; the person picks the account and its repositories on GitHub, keeping GitHub's default, all of them, or choosing some, and the setup route sends the tab back to `back` |
| New repository | Forgejo owned or managed, connected | the set up's same fields: organization (default `work`), repository (default `hello`) and a project, one of the person's or a new one; the general form of the first repository (`../protocol/setup.md`, section 15) |
| Forget the admin's token | managed | the admin's token erased; the forge is connected, its bot and tokens kept, and adopting a repository grants nothing more: the person makes the bot a collaborator |
| Set up a Forgejo here | the row's offer offered, not set up (`README.md`, 4.6) | its organization (default `work`), its repository (default `hello`) and its project, a new one named as the repository (`../draft/first-run.md`, 2.4; a new project waits on jig's projects at runtime); then the box asked to run the forge's steps (`../protocol/setup.md`, 7.1): its binaries fetched, its preparation, Forgejo and its runner started, then its handover taken and reconciled, the repository made with the starter workflow (`../protocol/setup.md`, 6.7), and, once the row says it made, the client's `MakeProject` and `Adopt` into it; the card names the path of `person_password`, never its password (`../protocol/setup.md`, 6.5) |
| Set up again | the row's offer offered, set up failed | the same ask again: what the last one left is kept, a fetched binary hashed and not fetched again |

Connect, but on a stopped forge or one whose records say why it is not
connected, "Connect as its admin" and "Set up a Forgejo here" are the
flow's, at its Details; every other action is a row's, in its opened
part.

A pasted value is a field of its own (`README.md`, 3.6 and section 8),
emptied as the request leaves and shown nowhere, then or after; the
page shows that it was given, never what. Two refusals are the page's
own, before anything is sent, the fields kept for the person to fix: a
field the action needs left empty is marked invalid, "Required." (a
Replace of a Forgejo's tokens with both empty: "Paste a new API token,
git token or both."); and the bundle of the action's values past 4 KiB,
jig's bound on a carried value (`tools.md`, section 12), shows "The
pasted values are past 4 KiB, the most a request carries." under the
action's button. The link a connect or an install answers is where
the browser's tab goes, as a sign-in's address is (`README.md`, 3.5,
`ConnectModel`, The tab's departure): the root asks its shell to leave
for it once, from the answer, and the page shows it as an external
link meanwhile. The terminal prints it and opens nothing (`README.md`,
6.5).

### 5.5 How the forge's refusals show

What the forge's protocol layer answers a call, or tells the root
(`../protocol/forge.md`, a forge stopped, GitHub's installations,
what the vocabulary adds and the setup's host's outcomes), wherever a
part shows it: the deployment's
page, the adoption form and its outcome, a change's panel and its hold.

| The forge answers or tells | It shows |
|---|---|
| `NotConnected` | on the forge's row, "Not connected", with Connect (5.4); the adoption form lists none of its repositories, and an adoption on it is refused, "Forge F is not connected." |
| `NotInstalled`, no accepted installation on the owner | "temper's App is not installed on OWNER.", with "Install on another account"; a push to it is refused, as "temper's App is not installed on the account owning NAME." (`../protocol/channel.md`, section 6) |
| `Missing`, on a repository whose account has an installation of selected repositories | "OWNER/NAME is not granted to temper's App.", linked to the installation's page on GitHub: `WEB/settings/installations/ID` for a person's account, `WEB/organizations/ORG/settings/installations/ID` for an organization's, WEB the forge's configured `web` address and ID the installation's |
| `Missing` otherwise | not found, as `README.md`, 4.7 says |
| `Stopped { why }`, told to the root | on the forge's row, "Stopped: WHY", with Replace (5.4); its calls answer `NotConnected` until its next connect, shown as that row says |
| an installation suspended | "suspended" on the installation, on the forge's row and on each repository of it (5.2); its calls are refused as no access |
| a stray installation | on the forge's row, "uninstalled at the next reconcile" |
| `Lacking`, a GitHub App's `Paste` (a connect of an App of the person's own, or a Replace) whose App lacks permissions or events temper needs | on the forge's row, connected, "temper's App lacks WHAT: grant them in the App's settings, then paste its key again.", WHAT each permission, with its level, and each event, as GitHub names them, until the forge's next check of the App ends (5.4); meanwhile a call that needs what the App lacks is refused by GitHub, as no access; with Replace where the forge is connected, Connect where it is not, and a link to the App's settings, `WEB/settings/apps/SLUG`, or `WEB/organizations/ORG/settings/apps/SLUG` for an organization's App |
| `Unmarked`, connecting as the admin a forge whose user lacks temper's mark | "LOGIN is not temper's bot: add `temper:bot` to its full name on Forgejo, then connect again." |

The installation's page and the App's settings are built by the engine
from the configured `web` address, never from an address a forge's
answer gave, so a link the client shows points at the configured forge
alone. SLUG is the App's slug, as the forge's row shows it (5.4).

## 6. Holds, in the forge's words

A held change's card (jig's `client/domain/objects.md`, section 5)
shows the forge's hold part: what happened, the head and the failure it
is about, by whom and when where another hand did it, a link to each on
the forge, and what each act will do. A release resets the change's
repairs, resolutions and updates, and counts its stall from the release.
Nothing on the forge is written over until the person decides
(`../domain/forge.md`, 8.4 and 8.5).

### 6.1 The change's own

| Held because | The card says | A release |
|---|---|---|
| stalled | the wait, and since when: CI that never reported on its head, a mergeability never known, a gate's task stuck | takes the change back to the step it was at |
| repairs spent | the repairs made of their limit, and the last failure: CI's failed step, a gate's remarks | takes it back, its repairs counted again; amending its gates or its producing charter's instructions first, where the failure is not the change's |
| resolutions spent | the conflicts resolved of their limit, and the base it conflicts with | takes it back, its resolutions counted again |
| updates spent | the updates made of their limit | takes it back, its updates counted again |
| an approval declined | who declined it, at which head, and why | amended to drop the approval and released, in one request; released alone, it is held again at the same head |
| a requirement refused at the merge (`../domain/forge.md`, 8.7) | which of its landing rule's requirements refused, at which head: CI failed, a gate asked for changes, a person's review on the forge asked for changes, by whom | takes it back, judged again at its next merge: held again while the refusal stands |
| a delegate or a write failed for good | which: its producing task's failure, or the forge's refusal of a write (a protected branch, a forbidden write) | takes it back, to decide afresh |
| a write uncertain, of a kind that cannot be recovered (`../domain/forge.md`, section 6) | the write, that it may or may not have been made, and where to look on the forge: the branch, the pull request's reviews or comments, the issue | makes it again, or goes on without it, as the person says (jig's `client/domain/objects.md`, section 5) |

### 6.2 Drift

A held change's card for drift (`../domain/forge.md`, 8.5; jig's
`domain/connectors.md`, section 11): what changed on the forge, by whom
and when, and a link to it there.

| What changed | Release | Otherwise |
|---|---|---|
| its pull request closed by someone else | reopens it | cancel: the change closes |
| its branch moved by another identity, backwards, or while nothing of temper's wrote it | takes the new head as the change's, checks it and asks every gate again | cancel |
| its branch deleted | creates it again at the head temper last knew, and reopens the pull request | cancel |
| its pull request retargeted | retargets it back | amend its landing branch to the new base, then release; cancel |
| merged by someone else | not held: it is landed, like any landing | |

## 7. Goals, findings and news on the forge

### 7.1 A goal's issue

- **In its project's home repository** (`../domain/forge.md`, section
  12): the goal's page links to it, says when it was last written, and
  that it is a projection: what people write there is not read, and the
  client is where temper is reached.
- **Pull requests name their goal's issue,** so a person reading the
  forge finds the goal, and from it temper's client.

### 7.2 Landings as news

On a goal's page, with what it hears (jig's `client/domain/pages.md`,
5.2): each landing in its repositories, marked as its own, overlapping
its work (it woke the goal), not overlapping (kept), or unknown, as for
a landing too large to compare (it woke the goal)
(`../domain/forge.md`, 7.3).

### 7.3 Findings

The issues agents filed (`../domain/forge.md`, section 12): a bug
found while exploring, a follow-up a review raised, a flaky test. They
are the forge's part of a project's page (jig's
`client/domain/pages.md`, 6.1):

- **Each** shows its title, its repository and issue, the task that
  filed it as a chip, when, and its state, which the store keeps: open,
  taken up by a goal, dropped.
- **Taking one up** sets a goal that names it, the board's form with its
  words filled in (jig's `client/domain/pages.md`, 6.2), or starts a
  chat about it, with the finding named in the chat's first words, so
  its brief carries it (jig's `client/domain/pages.md`, section 4).
- **Dropping one** gives a reason, the forge's own request (jig's
  `client/domain/root.md`, section 6).

## 8. Taking part

What the forge shows of work on what others own
(`../domain/forge.md`, sections 13 and 14), possible with the parts
above:

- **A participating object** (an outside contribution, an issue others
  opened) shows on the page of the task dealing with it: what others
  wrote there, as the news it heard, and what temper would write, a
  reply or a review, as a proposal card with its exact text, when the
  project's policy has a person accept it first.
- **Where temper may not merge,** a ready change waits for a maintainer's
  merge, and people's reviews on its pull request show as its news.
- **An upstream submission** is a proposal a person accepts before
  anything is posted, showing exactly what will be.

## 9. What each part is served from

Each part is the forge's family's (`README.md`, section 4): read
or watched by the forge's client side through the family's own reads
and subjects, under the forge's owner number, each naming its scope, a
project or the deployment (jig's `client/domain/contract.md`, 7.5; jig's
`client/domain/root.md`, section 7), their records in
`temper-client-protocol-wire` beside `Adopt` (`README.md`, 4.6).
The first read served is the demo's (`README.md`, 4.6 and 4.8), from
the configuration alone; the first watch, a change's panel (9.2); then
the deployment's part, its forges (5.4), as the subject `forges`, whose
rows carry each forge's number, kind, mode and connection (`README.md`,
4.6); the rest of a forge's row (its `web` address, its last connect's
end, its installations, its load and its actions) and a change's result
and its hold (2.5) follow.

| The client | jig's part and slot | The domain |
|---|---|---|
| a change's way to landing, its gates, requirements and heads | a panel, on its task's page | `../domain/forge.md`, section 8 |
| a change's result | a result, on its task's page, in a chat, in an update | `../domain/forge.md`, 8.2 and 8.6 |
| its holds | a hold, on the held card | `../domain/forge.md`, 8.4 and 8.5; jig's `domain/connectors.md`, 4.3 and section 11 |
| what a proposed write would do; the outbox | an effect, on a proposal card; an outbox entry | `../domain/forge.md`, section 6 |
| queues, paused and held | temper's `Changes` page | `../domain/forge.md`, section 9 |
| approvals | a person task's card; a panel | `../domain/forge.md`, 8.3 and 8.7; jig's `domain/people.md`, section 8 |
| adoption, repositories, health, landing rules | temper's `Adopt` page; a project's part | `../domain/forge.md`, sections 4 and 5, and 8.7; jig's `domain/connectors.md`, section 12; jig's `domain/authority.md`, section 10 |
| goals' issues, landings as news, findings | a project's part; a goal's page | `../domain/forge.md`, sections 7 and 12 |
| the forge itself | the deployment's part | `../domain/forge.md`, 5.4 |
| the picker (5.1) | temper's `Adopt` page, through the `Reachable` request (`README.md`, section 8) | `../protocol/forge.md`, the setup's host's `Reachable`; `../domain/forge.md`, section 4 |
| each forge's connection, its mode, installations and actions (5.4, 5.5) | temper's `Forges` page; the deployment's part | `../protocol/forge.md`, at start, connecting at runtime, a forge stopped, GitHub's installations and the setup's host; `../protocol/credentials.md`, the forges' records |
| the browser's steps of a connect (9.1) | none of jig's: temper's routes on the clients' listener | `../protocol/forge.md`, the setup's host |
| the demo: its project, forge, repository and suggested requests | the deployment's read, `Demo` (`README.md`, 4.6), for the demo's banner and the chats page | `../protocol/setup.md`, 8.4 |

### 9.1 The browser's steps, on the clients' listener

A GitHub connect's browser steps are temper's routes on the clients'
listener (`README.md`, section 8): temper's server asks jig's clients'
component first whether it claims a path (jig claims `/jig/...`), then
routes `/forges/N/github/...`, N a number in decimal, then serves the
browser's bundle. GitHub sends the person's browser to them, and
nothing of temper's client calls them. Every other path, the client's
`/forges` and `/forges/connect` among them, is the bundle's
(`browser.md`, 2.3).

| Route | What it does | Answers |
|---|---|---|
| `GET /forges/N/github/manifest?state=S` | temper's page for that state, rendered from what the setup's host's `Manifest` run answered, the form's target and the manifest, and held with it, at most `page_bytes`: one form, the manifest its hidden `manifest` field, escaped as an attribute's value, posting temper's manifest to `WEB/settings/apps/new?state=S`, the person's own account's, which posts itself as the page loads; a browser that runs no script, or one come back to the page through its history, shows instead "temper’s App, on GitHub", its button "Continue to GitHub" and "Back to temper", a link to the connect's `back`, else `/forges/connect?forge=N`; the manifest's redirect and setup addresses are whole URLs on the clients' listener, `http://LISTENER/forges/N/github/redirect` and `http://LISTENER/forges/N/github/setup`, and its `url`, the App's homepage, `http://LISTENER/` | `200`, the page |
| `GET /forges/N/github/redirect?code=C&state=S` | `Convert`: the code exchanged for the App's id, slug and key, each kept before the browser is answered, and its user; the App's hook points at `https://SLUG.invalid/`, so no webhook secret is kept on GitHub; then `Install` for a fresh state, held with the connect's `back`, whose link answers the browser, and the forge connects | `302` to the install page, `Install`'s link for that state |
| `GET /forges/N/github/setup?installation_id=I&setup_action=A&state=S` | `Accept`: the installation checked and accepted, then the forge's installations handed to its component again | `303` to the state's `back`; with none, `200`, a short page: "temper has the installation; you may close this tab." |

N is the forge's `number`, WEB its configured `web` address and
LISTENER the clients' listener's address, as temper builds the
manifest from it (`../protocol/forge.md`, the setup's host).

- **The tab goes and comes back** (with the user, 2026-10-23): the
  page that sends `Connect` or `Install` names `back`, its own address,
  which the setup's host holds with the state (a connect's passes to
  the install `Convert` starts), and the browser goes to the link in
  the page's own tab, as a sign-in's does (`README.md`, 3.5,
  `ConnectModel`, The tab's departure). The setup route sends it back
  there, so the page that started it shows how it ended, as a new
  client: the welcome's Forge step, `ConnectForge` at the forge, or the
  Forges page. A connect from the terminal names none, and its routes
  answer their pages as before.
- **The manifest's page's headers:** `Content-Security-Policy:
  default-src 'none'; form-action WEB; script-src 'sha256-H';
  style-src 'sha256-S'`, H the hash of its one inline script and S
  its one inline style. The script submits the form unless the
  page was reached through the browser's history (its navigation's
  type `back_forward`), so that back from GitHub's form stops on the
  page rather than posting again; `Cache-Control: no-store` and
  `Referrer-Policy: no-referrer`. It loads nothing, runs nothing but
  that script, and may post only to the configured forge.
- **The state is the only authenticator.** A route takes no cookie and
  no credential of jig's: the state, 128 bits or more drawn by the
  setup's host for one connect or one install, is good once, for its
  life (`setup.state_life_ms`), for that forge alone. The host keeps at
  most `setup.states` of them, in memory, so a restart forgets them
  (`../protocol/forge.md`, the setup's host).
- **Where GitHub's way can't go on,** the route answers a short page
  of its own, temper's name above one card: its heading, why, and
  "Back to temper", a link, in place of sending the tab back; each with
  `Content-Security-Policy: default-src 'none'; style-src 'sha256-S'`,
  S the hash of its one inline style, `Cache-Control: no-store` and
  `Referrer-Policy: no-referrer`, and nothing is kept. The pages:
  - *Ended:* a state used, unknown or past its life, or a code GitHub
    refuses, is answered `410`: "This link has ended", "It was used
    already, it expired, or temper restarted since. Start again from
    temper.", its link to `/forges/connect?forge=N`, since the state's
    `back` is gone with it. So is a run that stops otherwise:
    `Convert`'s `Stop`, whatever its failure, an `Install` that stops,
    and `Accept`'s `Stop`.
  - *A request:* GitHub's visit with `setup_action=request` and no
    installation, after a member of an organization who is not its
    owner asked for the App there. Nothing is accepted: the routing
    takes the state's `back` from what it holds (removing it), and the
    setup's host's state is left to the end of its life, since `Accept`
    needs an installation to use it; a later visit with that state and
    an installation is accepted as any other, and answered as one with
    no `back`. The route answers `200`: "GitHub sent a request, not
    an installation", "Only an owner of an organization can install
    temper’s App there, and temper can’t wait for an owner to approve
    it yet: an installation approved later isn’t used. Install it on
    your own account, or on an organization you own.", its link to the
    state's `back`, else `/forges/connect?forge=N`. An owner's approval
    later makes an installation with no state of temper's, a stray
    (`../protocol/setup.md`, 17.4); waiting for it is for later
    (`../draft/github.md`, section 18).
- **A busy forge holds the visit.** GitHub's setup visit with an
  installation and a state, for a forge whose one run is live, as the
  reconcile after the App's `Connected` (`../protocol/setup.md`, 17.5),
  which an install straight after a manifest's conversion meets, is held,
  not refused: the routing keeps its call, installation, state and
  `back`, at most `setup.states` of them, and starts its `Accept` once
  the forge's run ends. One held past the state's life
  (`setup.state_life_ms`), or with no room to hold it, is answered `410`
  as ended.
- **A visit with no state** to the setup route is GitHub's, after a
  person changed the installation's repositories on GitHub: for an
  installation accepted already it is a refresh, answered `303` to
  `/forges`, the routing handing the root `Repositories` for it so the
  picker lists them again, and nothing reaches the forge's component
  or the store; for any other installation it is refused, `410`, as
  ended. An update's visit with a live state is the same refresh, sent
  on to the state's `back`: GitHub may keep an install link's state
  through Configure to the update it saves, which Ready's "Choose its
  repositories on GitHub" counts on, and which the probe App is to
  show; where it doesn't, the tab lands on Forges.
- **Bounds and others:** a query malformed or past `routes.query_bytes`
  is answered `400`, in the refused page's words and headers; a method
  other than `GET` is answered `405`; a forge not configured, or one
  that is not GitHub, `404`; any other path under
  `/forges/N/github/`, not found.
- **A fixed address with a GitHub forge.** The App's `redirect_url` and
  `setup_url` are those whole URLs on the clients' listener, and
  GitHub's API cannot change them, so a deployment configuring a GitHub
  forge gives its clients' listener a fixed address, never port 0; the
  engine
  checks it at start and refuses to start, naming it
  (`../protocol/configuration.md`, the clients' listener). GitHub takes
  `http://127.0.0.1:PORT` for both, so a standalone deployment needs no
  public address: the browser that follows the link runs where that
  address reaches (section 10).
- **Tested without GitHub.** The fake forge's manifest post answers
  `200` with a small page holding one link, `Create GitHub App` (id
  `create`), to the manifest's `redirect_url?code=C&state=S`, which a
  test follows as a person clicks GitHub's form (a link, not a
  redirect, so the page's `form-action` is not crossed), and its install
  page answers `302` to the `setup_url` (skein's `fake-forge.md`, 10.1);
  so the three steps run as plain requests on the clients' listener.
  The routes' tests are `temper-service`'s own, requests through
  `fixtures::http::Peer`; the bytes through the fake's GitHub layer are
  the real loop's (`../domain/testing.md`, 2.4), whose browser's tier
  opens the manifest's page once to see it post its form, by itself, to
  the configured `web`; no test calls github.com.

### 9.2 A change's panel, watched

A change's panel (section 2) is the family's first subject, `change`,
watched by the forge's client side while the change's task page is open
(`README.md`, 3.5). Its document, `ChangePanel` (`README.md`, 4.6), is
made by the forge connector from what it keeps of the change
(`../domain/forge.md`, 8.8).

- **Its subject** names the change task by its number, in the scope of
  the task's project. jig admits the watch on the party's role in that
  project (jig's `client/domain/contract.md`, 7.2). The connector
  answers unknown for a task that is not a change into a repository the
  project holds, and the page's slot says the panel is unknown (jig's
  `client/domain/root.md`, section 6). The watch's window is unused:
  the document is whole within its bounds.

  A panel whose watch the engine ended may be unknown before the forge
  registers its change. For the same live change task, or its ended
  document with a change result, the page reopens that panel once per
  later task document delivered, with a fresh watch; an end alone never
  reopens it. An unsolicited Closed follows SubjectEnded because jig
  gives the client the same reason for both; a local unwatch's
  acknowledgement is ignored. Malformed or mismatched deliveries,
  another task, every other end reason, and an ended task without a
  change result remain terminal. Between the procedure's start and the
  next task document the page may say "no change"; it recovers on that
  next document, at the latest at the landing.
- **Level-triggered,** as every watch: each delivery is the whole panel
  as it is when made, and a slow client sees fewer of them. The
  connector marks the subject when the change's record changes (its
  step, its pull request, a gate's verdict, a bound used, its hold),
  and when a step reads CI at its head and finds a verdict other than
  the one kept. When a change enters its landing branch's queue, leaves
  it or moves in it, the changes behind it are marked too.
- **What it carries:**
  - the repository, by its forge's number and its owner and name
    (2.5); the change's branch and its landing branch; its head;
  - its pull request: its number once opened, and its address on the
    forge;
  - its step, the one 2.1 names, with what the step says: its place in
    its queue and the tasks ahead of it, a repair's reason, or its merge
    commit once landed;
  - its hold, with the step it was held at;
  - the task its step runs (its producer, a repair, a resolution or a
    gate's review), at most one at a time;
  - CI at its head;
  - its gates (2.2), each with its number, its kind, whether it is
    blocking or advisory, and its verdict at the current head, a clean
    update's carry counted (`../domain/forge.md`, 8.3);
  - its repairs, resolutions and updates, each used of its limit (8.4).
- **The address on the forge is the engine's.** The routing builds it as
  it encodes the document, from the forge's configured `web` and its
  kind: `WEB/OWNER/NAME/pull/N` on GitHub, `WEB/OWNER/NAME/pulls/N` on
  Forgejo. A forge with no `web` gives no address, and the page offers
  no "View on forge" (`README.md`, 5.10); so does an address longer
  than its bound (`README.md`, 4.6), which never fails the delivery.
- **Not in it,** since the connector's records do not keep them: the
  requirements at its head, judged only at the merge
  (`../domain/forge.md`, 8.7); its heads in order; each CI context and
  its failing job; the pull request's title and mergeability as the forge has them; its
  stall's times; its place in GitHub's merge queue (section 10).
- **As long as the change's record:** a change that has landed or ended
  keeps its last panel, delivered when its page opens, and nothing marks
  it again.

## 10. Open questions

- **A change's requirements, heads and CI contexts in its panel** (2.2;
  9.2). Default: left out until the connector keeps them beside the
  change's record (a judgment of each requirement at the current head,
  its heads with what made each, each context's state and its failing
  job, and a step's and a stall's times, which 2.1's "for how long"
  and 6.1's "since when" need), each a field of `ChangePanel` added in
  the version move that first needs it.
- **A change's files in the client:** whether the panel lists the files a
  change touches, from the connector's read, short of showing the diff.
- **Conflicts seen early** (`../domain/forge.md`, section 20):
  warning two goals whose open changes conflict before either lands, as
  a panel on both.
- **A queue's throughput:** if landings are ever batched, how a batch and
  its bisection show.
- **A person's approval as a rule,** posted on the forge as the person
  (`../README.md`, section 8), which the approval's card would then
  say.
- **A secret text area for the pasted key** (5.4). Default: the key is
  a masked one-line field, its armour's line feeds restored by the
  client; a text area skein can mark secret (spellcheck and
  autocomplete off, its text masked) would keep the PEM's lines as
  GitHub gives them and retire the restore. A change to skein's
  builder and page, not temper's.
- **A browser on another machine** for GitHub's connect. Default: the
  link names the clients' listener's fixed address, and the person
  opens it where that address reaches (in standalone, the engine's
  machine) or forwards the port themselves; temper makes no tunnel.
- **Mending a stopped managed or owned forge.** Default: Replace is
  offered for what a person pasted (a GitHub App's key, a connected
  Forgejo's tokens); a managed forge is mended by connecting as its
  admin again, reconcile making the bot's tokens anew for the same
  user; an owned one by its preparation and a restart.
- **Disconnecting a forge** from the client. Default: later; a forge's
  records are overwritten by connecting again, and those of a forge the
  configuration stops naming are named once in the engine's log.
- **The picker's pages kept.** Default: none; each page is a fresh read
  of the forge through the setup host's `Reachable` plan, asked by
  number, so a
  repository added or removed between two pages can move a row across
  them, seen twice or missed until the list is read again from its
  first page, and a page past the last answers no rows, as "Next" after
  a last page that came back full does.
- **The project's own repositories in the picker:** `Reachable` names no
  project, so neither the engine nor the client can leave out the
  repositories the project holds, as `../domain/forge.md`, what the
  picker offers, says the list does. Default: every reachable
  repository is listed, and adopting one the project holds is refused
  by the engine as any adoption; the request naming the project is the
  candidate, and the domain's page is asked to say which.
