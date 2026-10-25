# The forge connector

Provisional, 2026-10-04, revised 2026-10-23. How temper drives a forge,
Forgejo or GitHub: the forge as a connector on jig's contract (jig's
`domain/connectors.md`), with its resources, effects, topics, procedures,
requirements and reads; a forge connected or not; adopting repositories,
and what the picker offers; keeping up; a change's way to landing and
the requirements its merge meets; landing queues, a forge's own merge
queue among them; updates and conflicts; issues; and the ways temper
takes part in what others own. Its protocol layer, the component that
makes each call to a forge and hears its deliveries, is
`../protocol/forge.md`. What is still open is listed in section 20.

## 1. In one page

- **What the forge holds:** git; pull requests as the unit of a change;
  CI; branch protection, as a second net; and a record people can read.
  From what temper owns there, only CI, people's approvals and drift
  come back: no comment on it is an input, and no label marks what
  temper tracks.
- **Live work comes from the store.** The connector holds a working set
  of what the forge owns of the resources live tasks name (pull
  requests' heads and states, CI on those heads, landing branches'
  tips), kept up by listing what changed, sooner on a webhook's hint.
- **The change procedure** takes a change from produced to landed:
  produce, open, CI, gates, its turn in its landing branch's queue, an
  update from its base when it lacks the base's tip, and a merge at
  exactly its head. Agents are woken for what needs judgement: producing,
  repairs, reviews, conflicts.
- **Landing rules are requirements.** A project's rules say what a merge
  into a branch needs: CI passed, the head up to date with the branch,
  gates, people's approvals. The forge judges each from its own facts at
  the merge's exact head, which the forge's merge is conditional on (8.7).
- **Landing queues.** Changes ready to land in a branch wait in line, by
  their goal's priority then by when each became ready; only the first
  is brought up to date and merged. Where the forge keeps a merge queue
  of its own on the branch, temper's queue decides only what enters it
  and when; the forge lands it, or ejects it, which is a repair (9.1).
- **Connected or not.** A forge is connected while temper runs, or not:
  the root knows which from the routing's events, rebuilt at each start.
  A forge not connected answers every call `NotConnected` at once, and
  adopts nothing. Its writer, given when it connects, is the forge
  client's own record, restored at start (4.1).
- **Adoption picks from what temper can reach:** a GitHub App's
  installations, what a connected Forgejo's API token can push to, or
  any repository of a managed or owned Forgejo, where adoption first
  gives temper's user its access (4.2, 4.3). What it reads of the landing
  branch's rules, and the merge style, it says in words (4.4).
- **Updates are lazy and mechanical.** A change takes its base when it
  is first in line, or about to get a run, through the forge's own
  update; an LLM only resolves a conflict, in a workspace where the
  merge is in progress.
- **Every landing is news** for every goal working in that repository;
  it wakes a goal's coordinator only when it overlaps the files of that
  goal's changes.
- **Issues stay, one per goal,** as a projection people can read: the
  goal, its plan as a task list, and its milestones. Pull requests refer
  to it.
- **Plugged in as it is.** temper adopts repositories with whatever
  access it is given; webhooks, labels, protection and CI are all
  optional. Nothing is forced: pushes are fast-forwards, and merges are
  made at exactly the head decided on.

## 2. What it gives

- **Resources,** named `forge` `<host>` `<owner>` `<repo>`, then:
  - `branch` and the branch's own segments: `… branch temper 7 c42`;
  - `pull` and a number; `issue` and a number;
  - nothing beyond, for the repository itself.

  The host, the owner and the name are those the repository was adopted
  with (section 4).
- **Holds** (jig's `domain/connectors.md`, 3.3): a branch under temper's
  prefix takes an exclusive hold, and a hold already taken makes its
  task wait, never refuses it (jig's `domain/tasks.md`, 6.2). Landing
  branches, pull requests, issues and the repository itself are shared
  and take none: merging into a landing branch is ordered by its queue
  (section 9).
- **Effects:** open, edit, update, merge, close and reopen a pull
  request; post a review, a comment or a commit status; create, edit,
  close and reopen an issue; set reviewers; create and delete a branch
  (section 6). A run's push is the worker's, over git, under its task's
  hold (`worker.md`, section 5).
- **Kinds,** for authority (jig's `domain/authority.md`, section 4),
  numbered: 1, reads; 2, a run's push; 3 to 15, the effects, one per row
  of section 6's table. No kind implies another: each write a task means
  to make needs a grant of its own kind. Landing rules govern kind 4,
  the merge (8.7). None forces a push (8.7).
- **Topics:** landings on a branch; CI on a head; a pull request's
  state; the comments and reviews on a participating object (section 7).
- **Procedures:** the change (section 8).
- **Requirements:** CI passed, up to date, a gate, an approval, which
  landing rules name as judges of the merge (8.7).
- **Reads an agent may call:** each of the forge client's reads but
  locating a repository: a page of the issues and pull requests changed
  since a time; an issue or a pull request and its comments; a pull
  request, or the one for a head and a base, its reviews, a review's
  remarks, and its files; a branch's head, and the branches; the
  statuses and checks on a commit, and a failed job's log; a comparison
  of two commits; a file at a head; a repository's protection, settings
  and collaborators, and a user's permission. Each answer is cut to its
  limit. The host tools that serve them, and those for effects and
  topics, are `agent.md`, 4.3.
- **Effects offered to agents** (jig's `domain/connectors.md`, 4.5):
  creating an issue, posting a comment on an issue or a pull request,
  posting a commit status. What the change procedure owns (opening,
  updating, merging, reviewing as a gate) is not offered: a run that
  wants a change landed delegates it.
- **Brief sections,** of three kinds, each at the change's head: the pull
  request, its files and what differs from its base, or, for a
  resolution and a semantic repair, what landed in the base, with the
  files in conflict for a resolution; CI's failures, each with its
  failed step and the end of that step's output; reviews' remarks.
- **Workspace items:** for each repository a run works in (root.md,
  7.1), a checkout: where it starts (a landing branch or the
  repository's default, a branch, saved work, or a merge in progress),
  whether it may be written and the branch it pushes, and the git
  identity it is fetched and pushed with (`worker.md`, 4.1;
  `../protocol/credentials.md`, section 4).
- **Documents and parts** (jig's `domain/connectors.md`, 5.5): what
  temper's client shows of the forge, made from the connector's records
  and working set: a change's way to landing, its result and its hold,
  the queues, the repositories and their landing rules, goals' issues,
  findings and the forge itself (`../client/forge.md`, section 9).
- **What temper owns there** (jig's `domain/core.md`, 8.1): what it
  creates, recorded in the store by key. On the forge, ownership shows
  as authorship, by temper's own user (on GitHub, its App's bot user),
  and as temper's branch prefix, which branch protection can reserve to
  it.

## 3. Structure

```
temper-engine-domain-forge              the connector's top: resources and holds, adoption, topics, judges, briefs, routing; the effects and their identities
├── skein-forge-client                  the working set, keeping up, fresh reads, the request budget, effects' keys, write lanes and recovery (skein's forge-client.md)
├── skein-forge                         the vocabulary the top and the client share: calls, answers, errors, hints (skein's forge.md, section 3)
├── temper-engine-domain-forge-change   the change procedure and the landing queues: decisions over facts
└── temper-engine-domain-forge-issues   the goals' issues: what each projects, when to write it
```

- **The top** keeps the forge's resources as the project knows them
  (roles, what temper may do there), the holds on branches, the topics
  and their subscribers, the changes' procedure states, the issues'
  projection states, and the landing rules' gates and judges' parameters.
  It routes between its children and translates to and from the root.
  It names the client's live resources, owns the effects and their
  identities (section 6), and hands the client their bytes and its
  namespace, the deployment's id, before any restored entry or live
  resource, by which time the client has restored each forge's writer
  from its own record (section 16). It gives the client a forge's
  writer when the forge connects, and keeps each forge's connection as
  the root tells it (4.1).
- **`skein-forge-client` is a capability:** it knows the forge's API as
  `skein-forge`'s vocabulary states it, the working set, the request
  budget, and how a write is made once. Its owner, how it is driven and
  what it never knows are skein's `forge-client.md`, sections 2 and 9.
- **`change` and `issues` are policy:** given a change's state and the
  facts, or a goal's state, they say what is due. They keep nothing
  between calls; their states are the top's, committed in the store.

## 4. Adopting repositories

- **A repository joins a project with a role** (jig's `domain/core.md`,
  6.1): *owned*, where temper pushes its branches, opens pull requests
  and lands; a *fork* of an upstream (section 14); or *context*, read
  only. An owner of the project adopts it in the client, through the
  adoption form (`../client/README.md`, 4.4). A repository temper writes
  belongs to one project; others may adopt it as context.
- **A repository is named by its owner and name, and kept by the forge's
  id for it.** The adoption names the forge, the repository's owner and
  its name; the connector first locates it, a read whose answer is the
  forge's own id for the repository, and names the repository by that id
  from then on (`Repository { forge, repository }`), so a rename or a
  transfer on the forge keeps it the same repository. A repository the
  forge does not have refuses the adoption as not found.
- **A task names a repository by its tag, the pair packed.** jig's
  resource number, opaque to jig, is temper's tag: `(forge << 32) |
  repository`, the forge's number (16 bits) in the high half and the
  forge's id for the repository (32 bits) in the low, packed and read
  back by one pair of functions in the forge's domain; a tag with a bit
  set above the forge's names none. A delegation's resource,
  a workspace's repository, what a run left and a pushed head carry it,
  and a saved resource's path is the pair itself (`root.md`, section
  6), so two forges' repositories with the same id in one project stay
  two. A tag whose pair the task's project has not adopted names none.
- **The adoption request is the forge's own,** keyed as any party's
  request (`../client/README.md`, 4.4): the root takes it from the protocol
  layer typed, and the connector refuses a forge the deployment does not
  configure; then locates the repository (`Read::Locate { owner, name }`,
  answered `Answer::Located { repository }`); then the root makes jig's
  adoption request on the person's behalf (jig's `domain/people.md`,
  5.1), in which the connector reads what temper may do there (below)
  before the core commits the adoption (jig's `domain/connectors.md`,
  12). A forge not connected refuses it first, as such (4.1); a managed
  or owned Forgejo's access is asked for before the reads (4.2). Its
  outcome is the adoption's: adopted, refused by the core with jig's
  reasons, or refused by the forge with the connector's. The
  connector keeps it under the people child's token until its retention
  ends, to give again when the request is sent again (jig's
  `domain/connectors.md`, 5.5; `../protocol/store.md`, section 2).
- **The forge's host** in a repository's resource path, and in what a
  person reads of it, is the forge's as configured: the host of its `git`
  base, with `:port` when it names one (`../protocol/configuration.md`,
  section 3). The adoption does not ask a person for it.
- **What temper may do is read at adoption:** its user's permission on
  the repository; the repository's settings (the merge styles it allows,
  its default branch); the branch protection on the landing branch; and,
  but for context, the branches already under temper's prefix. Write
  permission, or admin, gives every kind of effect on an owned or fork
  repository, merging only where the repository allows a merge style;
  read gives reads alone; no permission refuses the adoption as
  forbidden. Forgejo refuses protection reads to a writer who is not an
  admin (skein's `forge.md`, section 9): adoption records
  protection as unknown, never as absent. On GitHub temper's user is
  its App's bot, its permission the installation's on that repository,
  and protection the rules GitHub applies to the branch (4.4). A write
  the forge refuses narrows what temper may do there, for every later
  decision (jig's `domain/connectors.md`, 12).
- **The project's ceiling follows** (jig's `domain/authority.md`,
  section 6) from each repository's role and what was read: on an owned
  repository, pushing under temper's prefix, opening pull requests and
  landing into its landing branches; on a fork, pushing into the fork;
  on a context repository, reading. Protection that keeps temper from
  merging makes landing a wait for someone else's merge (section 13).
- **One deployment's prefix.** Adoption refuses a repository where
  branches under temper's prefix exist that the store records no making
  of: they are another deployment's (jig's `domain/connectors.md`, 12).
  A prefix whose branches' names would pass the authority's segments
  (`authority.segments`, counting forge, host, owner, repository and
  `branch` before the branch's own parts) is refused as too large.
- **People the forge knows.** Adoption reads the repository's
  collaborators, each with its permission (read, write or admin), and
  gives them to the core as parties known by their forge user's id. The
  project's configuration maps each permission to a role, given to those
  with no role in the project yet (jig's `domain/people.md`, section 4):
  temper's usual mapping is admin to maintainer, write to member, read to
  observer. After that, roles are the core's: a permission that changes
  on the forge changes no role.
- **Access is given per repository, at adoption.** On a managed or
  owned Forgejo, the adoption asks for it first: temper's user made a
  collaborator with write, and the repository's hook made when the
  deployment has a webhooks' listener; a refusal refuses the adoption,
  named (4.2). On a connected Forgejo a person gives temper's user its
  access, as a collaborator or through a team, and adds its hook or
  leaves the repository to poll. On GitHub the App's installation on the
  repository's account grants it, and its hints come from the App's
  delivery log, with no hook per repository (`../protocol/forge.md`,
  5.1). The adoption then reads what it was given as any other.
- **The adoption names, per repository:** its landing branch; temper's
  branch prefix, `temper/` by default, which protection can reserve to
  temper's user; whether it is the project's home, where its goals'
  issues are kept (section 12), one per project and never a context
  repository; and whether it runs CI, and if not, the checks its changes
  are gated on (8.3). The merge style is the forge's for the repository,
  and the adoption shows it (4.4).
- **Nothing is required:**
  - no webhook: polling is the backstop, webhooks only cut the latency,
    and the client shows a repository whose hints never arrive;
  - no label: temper writes none, and decides nothing by one;
  - no protection: temper's rules are its enforcement, protection a second
    net;
  - no CI: a repository without it gates its changes on checks its
    adoption names (8.3).
- **Provisioning** (creating a repository, its webhook, its protection)
  is an optional set of effects a person may ask for, never a step of
  adoption (jig's `domain/connectors.md`, 12). The access a managed or
  owned Forgejo's adoption asks for is the setup host's run, not an
  effect of the connector's (4.2).

### 4.1 Connected or not

- **The root knows each forge's connection** from the routing's events
  (`../protocol/forge.md`, 8.1): not connected; not set up, an owned
  forge the box starts when asked, whose files are absent; setting up,
  while the box runs its group, at the step it last named, if any; set
  up failed, and why; connecting, while a setup's run that connects it
  holds it; connected, as its user, by id and login; or stopped, and
  why. The connector keeps it as the root
  tells it, in memory: nothing of it is stored, and each start rebuilds
  it from the routing's events, every forge not connected until its
  `Connected`.
- **A failed setup names the step it held,** alongside the connection in
  `Row.failed`: setting up at fetch, prepare or run names that step;
  connecting names connect; already set up failed keeps its first step;
  every other held state names none. This rule also applies when the
  box pipe closes during setup. Every nonfailure state clears the step,
  so a retry that reaches setting up or connecting fails at its own
  step. Both fields are volatile; no store or record version changes.
  Making the first repository has its own `first.state` failure.
- **A forge not connected answers `NotConnected`,** at once, at cost 0,
  and so does a connecting or stopped one (`../protocol/forge.md`, 2.7).
  A read it answers is final; a write's entry fails at once, as a typed
  refusal does, having reached no forge (skein's `forge-client.md`,
  sections 5 and 7). The connector takes it as the forge's access
  lacking, never as a fact of a repository: nothing is narrowed, no
  change is held, and no effect is found again for it.
- **What waits on it.** The connector steps no change on a forge not
  connected, and a stall does not count that wait (8.4); an effect whose
  entry failed `NotConnected` is committed again once the forge
  connects, since it reached no forge. The client shows why on the
  forge's row (`../client/forge.md`, the deployment's page).
- **The offer to set a forge up,** decided here alone. The host gives
  the connector, at construction as it gives the configuration, each
  configured forge's number, kind, mode and setup start, and whether
  serve runs under the box (`--box`, `../protocol/setup.md`, 7.1). One
  function gives a forge's offer from those, the box and the forge's
  connection: *offered* for an owned forge whose setup says `"start":
  "asked"`, under the box with its pipe open, its connection not set up,
  setting up or set up failed (setting up so that its row keeps showing
  the set up under way); *not under the box* for such a forge, in such a
  state, without the box or once its pipe closed; *not offered*
  otherwise, and for a forge not configured. The row and the answer read
  the same function, so they never disagree. The same function gives
  each action's offer from the forge's kind, mode, setup and connection:
  the engine domain decides every action a row offers, including
  "Connect as its admin", which needs a managed setup entry
  (`../protocol/setup.md`, 5.5).
- **Setting up a forge,** `SetUp { forge }` (`../client/README.md`,
  section 8): the root gives it to the connector, which refuses it by
  the forge's offer when not offered, "not offered" or "not under the
  box"; "busy" while any forge is setting up, itself included; and
  otherwise asks the routing's box link to ask the box
  (`../protocol/forge.md`, 2.8). The link decides nothing: it carries
  the ask out and the box's lines in, which the routing tells the
  connector as connections, and the box's answer back. The pipe's close
  comes to the connector as the root's `BoxClosed`: each offer becomes
  not under the box, and a forge setting up becomes set up failed, "the
  box closed the pipe". The connector changes no state at the ask: the
  routing tells it each, as it does the others: at the box's `started`,
  connecting, then connected, or set up failed with the engine's step
  that stopped (`../protocol/forge.md`, 2.8), offered again. `SetUp` is
  a keyed request, admitted by the core first (`../client/README.md`,
  section 8): the connector keeps its answer as its outcome, in memory
  alone, the latest per configured forge under its request's token and
  one more for any forge not configured, and answers unknown for a token
  it no longer holds.
- **The deployment's forges,** the subject `forges`
  (`../client/forge.md`, 5.4): the connector makes its document, one
  row per configured forge in number order, but an ended one (below),
  each its number, kind, mode, connection as the routing last told it,
  offer, first repository, on GitHub its installations as the routing
  last told each (`Event::Installation`: its id, its account when
  heard, suspended, stray), in id order, and its last connect's end:
  none, or the reason a `Connect`'s or an `Install`'s run stopped with
  (`ConnectEnded`, `InstallAnswered`), held until the forge's next
  connect or install is admitted or it connects; all its own
  (nothing is added where it is encoded), and
  marks the subject at each connection, installation and connect's end
  it is told and at `BoxClosed`, so that a watch
  follows a set up through its steps and a connect to its end. Nothing
  of it is kept. A restart starts every row with no connect's end, and
  with the installations the forge's records accept (its `Accepted`
  ids), each told by the setup host at the restored connect, its
  account none, neither suspended nor stray, since the component's
  first read tells only those not accepted or changed (skein's
  `forge-connection.md`, 4.4); the account is filled from the first
  `GitToken` minted for it (5.3 there), and changes follow as before.
- **The demo's forge, ended** (`../protocol/setup.md`, 8.4): the
  connector's own state, not the routing's. It holds the demo's `Demo`
  record as it restores it and writes it, and the demo's forge is ended
  from a start whose record reads ended, as from the end's decision
  on. The end is decided here, in the step that takes a connection: the
  first time the routing tells a configured forge other than the
  demo's connected, while the record is none or running, the connector
  saves ended, ends the demo's forge, asks the box link for `stop
  FORGE` when the demo ran and the pipe is open (`../protocol/forge.md`,
  2.8), and answers a start held for its acts *not offered*. An ended
  forge is in no list: no row of `forges`, so that `temper ctl wait
  forges` names it no more, no offer, so that `SetUp` is refused "not
  offered", nothing in the picker (4.3), and nothing in the setup host's
  scans, which read the same state; the routing's events for it change
  nothing (`../protocol/forge.md`, 8.1), so that a read waiting on its
  connection (16) is never asked again. Its writer stays the forge
  client's record, and its rows stay, so that what it did reads as
  before.
- **A repository ended** is one adopted from an ended forge. Nothing
  is stored for it: it ends with its forge. It leaves every list of a
  project's repositories: the project's home, so that the project's
  next adoption may name a home, the one-home check skipping it; the
  context repositories a workspace is given (`items`); and the adopted
  ids the setup host reconciles. Its row stays, and a lookup by its
  name or tag answers it, so that its tasks, chats and landings read as
  before.
- **An adoption on a forge not connected** is refused as such, before
  any read, and the client offers Connect; the picker offers none of
  that forge's repositories (4.3).
- **The writer given at runtime.** The forge client refuses every effect
  on a forge with no writer, `Forbidden` (skein's `forge-client.md`,
  7.1). The configuration names none (`../protocol/configuration.md`,
  the forges): the connector gives the client a forge's writer, its
  user's id (the Forgejo user, or the GitHub App's bot user), when the
  routing says the forge is connected (`../protocol/forge.md`, 2.2),
  through the client's `writer(d, env, Writer { forge, author }, out)`.
  The same writer given again changes nothing. A forge's writer never
  changes: a replacement that answers as another user is refused before
  it reaches the forge (`../protocol/forge.md`, 2.2), since the keys and
  echoes already on the forge are the old user's.
- **The forge client keeps the writer as its own record** (skein's
  `forge-client.md`, 7.1): `Stored::Writer` under `Key::Writer(forge)`,
  in temper's small store under its tag 16 (`STORE_VERSION` 3), and
  restored at start among the forge's rows, in any order: the client
  takes them all before `Restored`, and runs no restored effect before
  it, so a restored effect never meets `Forbidden` (skein's
  `forge-client.md`, section 8; section 16). The connector keeps no
  copy.

### 4.2 Access at adoption

- **On a managed or owned Forgejo** the connector asks for access
  before the adoption's reads: `Adopt { forge, repository }`, through
  the root and the routing, which runs it on the setup's host with the
  admin's token (`../protocol/forge.md`, 2.6). It makes temper's user a
  collaborator with write on the repository, and its hook when the
  deployment has a webhooks' listener. Done, the adoption reads as any
  other; refused, the adoption is refused, with the run's failure named.
- **The connector never holds the admin's token,** and the forge
  component never does: the setup's host alone uses it, for reconcile,
  for access at adoption and for the picker's reads (4.3)
  (`../protocol/credentials.md`, least power).
- **At each start** the repositories adopted on a managed or owned
  Forgejo are named to the routing for its reconcile, which checks and
  repairs each one's collaborator and hook (`../protocol/forge.md`,
  2.1).
- **Elsewhere there is nothing to ask:** on a connected Forgejo a person
  gives the access, and on GitHub the App's installation does (4.3).

### 4.3 What the picker offers

The adoption form lists, per connected forge, the repositories temper
can reach, a page at a time, by number (`../client/README.md`, 4.4; the
FORGE family's `Reachable { forge, installation?, page }`, answered
`{ rows, more }`). The request names no project, so the project's own
repositories are among the rows, and adopting one it holds is refused
as any adoption of it is. Each row is a repository's id, its owner's id
and login, its name and whether it is private.

Every kind is read the same way: the connector asks the root, which has
the routing run the setup host's plan `Reachable { forge,
installation?, page }` (skein's `forge-setup.md`, 4.8;
`../protocol/forge.md`, 2.6; pending in skein's code), pages counted
from 1, at most `reachable` rows (`picker.page`), ending
`Done(Reachable { rows, more })`. The plan writes nothing, so it runs
beside the forge's one writing plan, and no call of the forge client
reads the picker:

- **GitHub:** one accepted installation's repositories (`GET
  /installation/repositories`), `installation` its id, read on a token
  the host mints for the page and drops. An id the forge has not
  accepted is refused. The document is skein's (`forge-github.md`, 8.3).
- **A connected Forgejo:** the repositories temper's API token can push
  to, read on that token (skein's `forge-setup.md`, 4.6).
- **A managed or owned Forgejo:** every repository the admin can see, by
  id ascending, since adoption gives temper's user its access (4.2). The
  admin's token stays the setup host's. Forgetting it makes the forge
  connected, and its picker with it.
- **"Not listed":** any repository by owner and name, located as any
  adoption is (section 4), and, when it cannot be reached, what would
  make it reachable, in words.

What a refusal says:

- **`NotConnected`:** the forge is offered for Connect, not listed.
- **`NotInstalled`** (GitHub: no accepted installation on the owner's
  account): "temper's App is not installed on <owner>", with "Install
  on another account".
- **`Missing`** on a repository whose account has an installation that
  selects repositories: "not granted to temper's App", linked to
  GitHub's installation page; on a connected Forgejo, "make <login> a
  collaborator". Once the forge says so, the repository is listed,
  with no restart.

An installation's repositories changing, or its account renamed
(`Repositories`, `Renamed`; `../protocol/forge.md`, 2.4), reaches the
connector with no names: it lists that installation again, shows the
account's login as that listing gives it, and marks each adoption whose
repository left the installation; a later listing that has it unmarks
it. Each `Connected` lists again every accepted installation that holds
an adoption, so a change made while temper was away is found. An
installation deleted marks every adoption on its account, and one
suspended marks them until it is unsuspended. The marks are held in
memory, never stored. A marked repository is shown with what would make
it reachable again (`../client/forge.md`, a repository's health). Its
calls meet `Missing`, `NotInstalled` or `Forbidden`, which the connector
takes there as access lacking, as `NotConnected` is (4.1): nothing is
narrowed, and its changes wait.

### 4.4 What adoption says of the branch's rules

- **The rules on the landing branch** are read at adoption and said in
  words, each with what it means for landing, and the one setting that
  would let temper land where a rule keeps it from landing. temper
  never changes a repository's rules.
- **On GitHub,** the rules GitHub applies to the branch, from rulesets
  and classic protection alike, neither's absence taken for the
  other's: required approvals, required checks, code owners, a merge
  queue (9.1), signed commits, linear history, and whether temper's App
  may bypass them, `Never`, `PullsOnly` or `Always` (the App's own,
  never the ruleset's list). skein's `Protection` carries them as
  `queue`, `signatures`, `linear`, `code_owners` and `bypass` (skein's
  `forge.md`, 3.6; `forge-github.md`, 5.3).
  Forgejo answers each false, and `Never`.
- **What each means:**
  - a merge queue: temper's changes enter it (9.1);
  - required approvals or code owners: a person reviews, or the App
    bypasses;
  - required checks: CI, as the landing rule's;
  - signed commits: temper's workers do not sign their commits, so
    where the forge refuses temper's merge for it, landing is a wait for
    someone else's merge (section 13; section 20);
  - linear history: a merge style that keeps it;
  - temper's prefix reserved to temper's user: welcome.
- **The merge style** is shown at adoption. On Forgejo it is the
  repository's default (skein's `forge.md`, section 12). GitHub keeps
  none: it is the first the repository allows of merge, squash and
  rebase (skein's `forge-github.md`, 5.1). On a branch a merge queue
  guards, the queue merges by its own method, set in the branch's rules,
  and the adoption says so.
- **Commit statuses on GitHub:** temper's App reads statuses and writes
  none: `status` is false in the kinds there from adoption on, and
  nothing is held for it. Every adoption on GitHub says that the gates'
  verdicts (8.3) stay temper's own, and that GitHub's rules cannot
  require them.

## 5. What it holds, and keeping up

The working set, the keep, fresh reads and the budget are the client's
(skein's `forge-client.md`, sections 3 to 6). This section keeps what
temper puts in them and the members that set them: the client's limits
are `forge.client.<limit>`, with durations in `_ms`
(`../protocol/configuration.md`, section 7).

### 5.1 The working set

Held by skein's `forge-client.md`, section 3. temper's part:

- **The live resources, and nothing else** (jig's
  `domain/connectors.md`, 3.4). A resource enters when a live task names
  it, and leaves when none does. The top names them to the client.
- **What temper watches:**
  - per repository: the landing branches' tips, and the CI on each tip;
  - per pull request temper owns that a live task names: its head and
    base, its state (open, closed, merged), its mergeability, CI on its
    head (each context's latest status, and the combined state), and,
    when a decision wants them, its files at its head;
  - per branch a live task holds: its tip as temper last pushed or
    updated it, to tell drift from temper's own moves;
  - per participating object: its comments and reviews not yet given to
    its task (section 13);
  - per issue of a live goal: what temper last wrote into it, so an
    unchanged projection is not written again.

  A change watches its branch and its base until its pull is known, then
  its branch and its pull. A change of the pull it owns wakes the change,
  as a hint does.
- **Its capacity:** `forge.client.resources` live resources and
  `forge.client.repositories` repositories. A batch naming resources past
  it is refused.

### 5.2 Keeping up

Held by skein's `forge-client.md`, section 4: the listing per
repository since the forge's time, a pull request's backoff, the hints,
the landing branches' tips, the slow pass and the forge's clock. temper's
part:

- **Its values:** `forge.client.poll_ms`, `poll_max_ms`, `hinted_ms`,
  `slow_ms` and `settle_ms`. The paces are what they are with no
  webhook, so a delivery lost, refused or never sent costs latency,
  never liveness (skein's `forge-client.md`, section 4).
- **Hints** come from the forge's webhook deliveries, or a GitHub App's
  delivery log (`../protocol/forge.md`, section 5 and 5.1). The client
  reads on every hint it holds a resource for. temper, the owner, drops
  its own echoes, by key and by pusher: a creation's key for an item,
  the pusher for a push (skein's `forge-client.md`, section 4).
- **Which change a hint wakes:** within its repository, a named branch
  wakes changes with that head branch or landing base; a commit wakes
  changes using it as their last head or observed base tip, including
  facts already gathered by an unfinished fresh-read step. If the branch
  and pull reads saw different base tips, both remain dependencies until
  the next step reads them again. An item wakes its pull's change (including reviews on that pull), or a change naming
  that issue or pull. Named resources and subscriptions also retain their
  dependencies. Before a change knows its pull or commit dependencies,
  it cannot exclude those hints. A resource matching no change, including
  an unnamed push or a name no longer recognised after a rename, wakes
  every change in the repository. The client still receives every hint:
  filtering procedure wakes never filters its applicable keep reads.
  A relevant wake during a fresh-read step still restarts that step before
  deciding; an irrelevant hint adds no restart. Gate results, task
  dependencies and queue changes keep their existing wake routes. Each
  decision still uses the rules held then (8.7–8.9); hints do not replace
  polling, settling or stall deadlines (8.3–8.4, 9).
- **What Forgejo settles after it delivers** is read again sooner, by
  the change procedure, which reads its pull request afresh (5.3), not
  by the keep. Forgejo checks a pull request's mergeability after a push
  moves its head or base, and answers it not mergeable while it checks;
  and temper relies on no order between a job's end being delivered and
  its commit status being written, only on both settling within `settle`
  (skein's `forge.md`, section 9). So, within `settle` of a move of
  its head or base (known by a hint, a read or temper's own push), a
  pull request that reads not mergeable is read again at `hinted`,
  doubling up to `poll` apart, until it reads mergeable or `settle`
  has passed; and a commit a job's hint named whose CI reads pending
  is read again the same way. These rereads are temper's fresh reads. `settle` is a limit
  of the forge client's, which checks only that it is above 0 and runs
  no window (skein's `forge-client.md`, sections 4 and 5).
- **The window is the change procedure's own wait.** Until `settle` has
  passed since the move, not mergeable is not known: the change waits
  until the window's end at most, never stalls on it, and is never taken
  as conflicted on it; once the window has passed, not mergeable is a
  conflict (8.2). A move long before, such as a head pushed before a
  long CI, leaves no window open. The window's start is the later of the
  change's own last push, which it keeps, and the last move a hint or a
  read told of, which it does not: after a restart, only its own push
  opens a window.
- **GitHub settles too.** It answers mergeability unknown until it has
  checked, and may refuse a merge as not mergeable just after a head
  moves (skein's `forge-github.md`): within the window both are not
  known, and read again, as Forgejo's not mergeable is. An update GitHub
  accepts it makes after it answers: the update is done once the head,
  read afresh, contains the base's tip it asked for (section 10).

### 5.3 Fresh reads

Held by skein's `forge-client.md`, section 5: a fresh read goes first
in the request budget (5.4). temper's part is what it asks for. Before a
decision depends on a fact, the change child domain asks for it afresh:
the branch's head, the base's tip, the pull request, CI on the exact
head and on the base's tip, the comparison of the head with the base's
tip, the pull request's reviews, and the files of the pull requests an
overlap compares. These are also the facts the forge's requirements are
judged on (8.7).

- **CI on a commit** is read as its checks, skein's `Read::Checks`: the
  commit statuses and, on GitHub, the check runs, each a row with its
  context and verdict (skein's `forge.md`, 5.2; `forge-github.md`, 4.4),
  never the combined status alone, which on GitHub stays pending with no
  status at all while Actions' check runs end. Which rows are CI is the
  adoption's: where the landing branch's rules name required checks
  (4.4, `Protection`'s contexts), those contexts, each of which must
  have reported; otherwise every row. They fold to one verdict: failed
  when any of them failed; else pending when any is pending or a
  required context has no row; else passed when any is there; else
  unknown, which the change waits on as pending (8.4, stalls). The
  head's, the base tip's and the CI a topic publishes (7.1) are read
  and folded alike, on every forge: on Forgejo the checks are
  its statuses, each context's latest, so its CI reads as before.
  Where no required checks are named, the pull request's own read,
  which folds every row of its head the same way (skein's
  `forge-github.md`, 4.4), is the head's CI when it is at the head, and
  no second read is made.

### 5.4 The request budget

Held by skein's `forge-client.md`, section 6: the four queues in their
order, the charge of each call and the wait after a refusal for the
rate. The order is the one jig's `domain/connectors.md`, section 13
gives: fresh reads first, a write's own checks and finds among them;
then writes; then keeping up; then the slow pass. temper's values:
`forge.client.rate` requests per `forge.client.window_ms`, with
`forge.client.reserve` of each window kept for keeping up and the slow
pass.

## 6. Effects

Every effect is committed into the outbox and made by `client`, in
commit order per lane (a pull request, an issue, a repository) and one
at a time on each (jig's `domain/connectors.md`, 4.2). The top owns the
effects and their identities; the client's keys, its write lanes and its
recovery are skein's `forge-client.md`, section 7. temper's effects:

| Kind | Effect | What it is | Recovery class (jig's `domain/connectors.md`, 4.3) | Found again after an uncertain failure |
|---|---|---|---|---|
| 3 | open a pull request | a creation: from a branch into its landing branch, with a title, a body and a key | conditional: only while no pull request for its branches is open; Forgejo keeps it, closed or not | by its branches: the pull request open for them, else the oldest |
| 10 | edit a pull request's title, body or base | a set | idempotent | written again |
| 11 | update a pull request from its base | a transition: the base's tip merged into its branch, by the forge | unrecoverable | its head contains the base tip it asked for |
| 4 | merge a pull request | a transition, conditional on its exact head; or, where the forge keeps a merge queue on the base, the pull request queued there (9.1) | conditional | merged, and at which commit; or in the forge's merge queue |
| 12 | close, reopen a pull request | a set | idempotent | written again |
| 5 | post a review | a creation, with a key | unrecoverable | by its key, among the reviews after the entry's start |
| 7 | post a comment | a creation, with a key | unrecoverable | by its key, among the comments after the entry's start |
| 6 | post a commit status | a set: a context on a commit | idempotent | written again |
| 8 | create an issue | a creation, with a key | unrecoverable | by its key, among the issues changed since the entry's start |
| 13 | edit, close, reopen an issue | a set | idempotent | written again |
| 14 | set a pull request's reviewers | a set | idempotent | written again |
| 9 | create a branch at a commit | a creation, named | unrecoverable: another hand can delete it, and nothing then shows it was made | the branch exists at that commit |
| 15 | delete a branch | a set: temper's own branches only | idempotent | it is gone |

- **Each row is a kind of its own** (jig's `domain/connectors.md`,
  3.1): a policy that grants writing issues grants each kind it means.
- **Classes follow what the forges offer** (skein's `forge-client.md`,
  section 7.3; the facts, skein's `forge.md`, section 9, and
  `forge-github.md`, section 13). Neither Forgejo nor GitHub keeps a
  key it would refuse a second creation by, so no kind is keyed. A pull
  request is opened only while none is open for its branches, and
  each forge keeps every pull request, closed or not, so the
  state it leads to is always found, closed by another hand included,
  which is drift (8.5). A change opens one only when none exists for its
  branches, and reopens its own when it was closed (8.5), so the oldest
  for its branches is the one it follows; one another hand opened while
  that one is closed is the open one, found first. A branch is created
  only while none of its name exists, but another hand can delete it, and
  nothing then shows it was made: its creation is unrecoverable.
- **An unrecoverable creation is looked for first:** an uncertain review,
  comment, issue, update or branch found by its key, or by the state it
  leads to, is made; one not found by its deadline holds its task for a
  person, and is never sent again by temper on its own. The client makes
  the find (skein's `forge-client.md`, section 7).
- **Keys are carried** in what temper creates: a marker at the head of a
  body, a comment or a review, hidden where the forge renders it (skein's
  `forge.md`, section 6; temper's word and namespace,
  `../protocol/forge.md`, section 4). A pull request's key is its
  branches, which temper alone names. An entry's start, and the find that
  reads only what came after it, are the client's (skein's
  `forge-client.md`, section 7).
- **An effect's identity** is its purpose, and the top encodes it. The
  client takes the bytes whole and never inspects them (skein's
  `forge-client.md`, section 7). A key is the client's frame (version 1, the namespace's
  length as two bytes big-endian, the namespace) and then the purpose's
  bytes: a tag, and its fields in order, each number big-endian. The
  namespace is the deployment's id, 16 bytes. A purpose is derived from
  what the effect is for, never from when it was asked or its entry's
  number, so asking again for the same thing gives the same bytes (jig's
  `domain/connectors.md`, 4.4):

  | Tag | Purpose | Fields, with their bytes | Bytes with the tag |
  |---|---|---|---|
  | 1 | a call: one agent's call in one task attempt | task (8), attempt (8), completion (4), position (4) | 25 |
  | 2 | a step: one purpose within a task's procedure | task (8), purpose (2) | 11 |
  | 3 | a projection: one part of a goal's issue | goal (8), part (1), number (8) | 18 |
  | 4 | a release: cleanup of a task's named resource | task (8), the resource's length (2), the resource | 11 and the resource |
  | 5 | a history milestone: one task's milestone in a goal's issue | family (1), goal (8), task (8), position (8) | 26 |

  A part of a projection is the issue's open (1), its body at a local
  revision (2), the plan accepted (3), a plan revision by its history
  number (4), a change landed (5), a change held (6), a report kept (7),
  the goal finished (8) and the issue's close (9); the number is the
  revision, the history number or the change's, and 0 for the others. A
  history milestone's family is a task's lifecycle position (1) or a
  task's semantic revision (2, whose position is the revision). So a
  call's key is 44 bytes, and a key is refused when it passes the
  client's `op_bytes` or the namespace is empty.
- **Conditions travel with the effect:** a merge names the head its
  decision saw, which the forge checks, and a stale head is refused, not
  merged. It names the base too: the pull request is read afresh first,
  and one retargeted since the decision is not merged.
- **A merge may be queued:** on a base the forge's merge queue guards,
  the merge answers `Answer::Queued`, final for the call and not for
  the entry: neither a merge nor a refusal. The entry is the client's
  until a read finds the pull request merged or out of the queue
  (skein's `forge.md`, 3.3; `forge-client.md`, 7.7, pending in skein's
  code): it is never sent
  again, and it holds its lane, so a comment or a review on that pull
  request waits until the queue decides. What follows is 9.1's.
  Forgejo never answers it.
- **Failures:** a merge refused for a conflict, an update refused for a
  conflict, a pull request with nothing to merge, a protected branch and
  a forbidden write are each typed, and go to the procedure that asked,
  never retried blind. A forge not connected is not a failure of the
  procedure's: the entry fails at once, `NotConnected`, and is committed
  again once the forge connects (4.1).

## 7. Topics, and landings as news

### 7.1 Topics

| Topic | Published when | Read by |
|---|---|---|
| landings on a repository's branch | a landing branch's tip moves | goals working in that repository; landing queues |
| CI on a head | a head's CI, as 5.3 reads it, changes | changes; tasks that subscribe |
| a pull request's state | opened, closed, merged, its head moved, its mergeability known | changes; tasks that subscribe |
| comments and reviews on a participating object | another party writes there | the task dealing with it (section 13) |

The change child domain reads its facts directly and needs no
subscription; topics are for tasks whose executors are agents, and for
goals.

### 7.2 Every landing is news

- **For two kinds of goal:** the goal whose change landed, whose step is
  done; and every other active goal with changes open or planned in the
  same repository, whose base has moved. Concurrent plans meeting this
  way is the ordinary case, not an exception for outside activity.
- **A goal hears of landings** through the forge's topic for goals
  (jig's `domain/connectors.md`, 5.4): the landings on the branches its
  subtree's changes target, which the core subscribes it to as the first
  change into each is made, until the goal ends.
- **A landing temper did not make** is news like any other: what landed
  is read, a pull request or a range of commits.

### 7.3 Which landings wake

Not every piece of news wakes. The connector classifies each landing for
each subscribed goal (jig's `domain/connectors.md`, 5.3), without an
LLM:

- **its own change landed:** kept; what comes after it is due
  mechanically, through the goal's dependencies;
- **another landing overlaps its work:** wakes. Overlap is the files the
  landing touched against those of the goal's own open pull requests at
  their heads, and, for changes not produced yet, the paths their specs
  name, as a hint;
- **no overlap:** kept, for its next run to read;
- **overlap unknown** (a landing too large to list, a read that failed):
  wakes, since a wasted turn is cheaper than a missed conflict.

The files are read from the forge (a pull request's files, or the
comparison of the branch's old and new tips), cached per head and cut at
a limit past which the overlap is unknown. Forgejo pages a pull
request's files but answers a comparison whole
(skein's `forge.md`, section 9): the protocol layer bounds the
comparison's bytes, files and commits before the domain holds them, and
a bound reached, an unusable answer or an uncertain completeness makes
the overlap unknown and wakes every affected subscriber. A pull
request's pages count only while fresh reads before and after them name
the same head; a head that moved voids the listing.

## 8. The change procedure

### 8.1 What a change is

A change task asks for one change to one repository, landed into one
branch. Its parameters:

- **the repository and the landing branch;**
- **the branch,** held by the task: one it is handed, already pushed (a
  chat's small fix, handed over by the chat that made it: jig's
  `domain/tasks.md`, 6.4), or its own, named after its tree
  (section 11), which a producing task pushes;
- **what to produce,** when nothing is pushed yet: the spec and charter
  of its producing task;
- **its gates** (8.3);
- **the pull request's title and body,** or what to make them from, and
  the goal it is part of;
- **its bounds:** repairs, resolutions, updates, and the stall of each
  wait.

A chat asks for one with `change`, giving the words alone, and the root
fills in the rest from the project (`agent.md`, 4.3.1).

A goal across repositories is several change tasks, ordered by their
dependencies: skein's change lands before temper's that bumps its lock.

### 8.2 Its way to landing

```
producing ─► opening ─► checking ─► gating ─► queued ─► first ─► landing ─► landed
                          ▲   │        │                  │
                          │   └────────┴──► repairing ────┤
                          │                               ├─ lacks the base's tip ─► updating
                          └───────────────────────────────┤                          │
                                                          └──── a conflict ◄─────────┘
                                                                      │
                                                                  resolving
```

1. **Producing:** a task for an agent, with the charter the change
   names, whose workspace starts from its landing branch (or its saved
   work) and pushes the change's branch (`agent.md`, section 6). Its
   result is the head it pushed. An attempt whose answer was lost may
   have pushed all the same: its writer slot is freed only once the
   branch has been read afresh (jig's `domain/tasks.md`, 6.3), and a
   head found there then is the change's own (the branch is held by this
   change alone), so the change cancels its producing task, whose work
   is done, and goes on to open it, in the same commit that frees the
   slot, before any retry could claim it. A change handed a pushed
   branch starts at the next step.
2. **Opening:** its pull request into its landing branch, keyed by its
   branches, its body naming its goal's issue ("Part of
   `ai/temper#12`").
3. **Checking:** CI runs on its head. A failure is a repair (8.4): a task
   given the failing output. A repository without CI is checked by its
   checks (8.3).
4. **Gating:** each gate without a verdict valid at this head runs: a
   task at the exact head. A blocking review or check that asks for
   changes is a repair, given its remarks; a blocking approval a person
   declines holds the change (8.3).
5. **Queued:** ready (CI passed and every blocking gate holds, at its
   head, and its task is not kept by a dependency) and waiting its turn
   in its landing branch's queue (section 9).
6. **First:** its turn, which it keeps through what follows unless it
   leaves the queue (section 9). If its head lacks the base's tip, it is
   **updated** (section 10): cleanly, and its new head is checked again,
   its gates carried over where they count clean updates (8.3); or in
   conflict, and it leaves the queue to be
   **resolved**, its new head checked again and its gates asked again.
   A pull request whose mergeability has not settled (5.2) is neither:
   the change waits for it, until the window's end at most, and the
   stall does not count that wait.
7. **Landing:** merged at exactly its head, in the repository's merge
   style (4.4), automatically: what more a landing needs is a landing
   rule's, which the core checks at the merge (8.7). Where the forge's
   own merge queue guards the branch, the merge puts the pull request in
   that queue at exactly its head, and the change lands or is ejected
   later (9.1). The merge waits for dependencies (jig's
   `domain/connectors.md`, 4.1): the core refuses it while the change's
   task is kept, naming what it waits for, a backstop to the queue's own
   rule (section 9). Merged, the change is **landed**: its result is the
   landing (the pull request, the merge commit, the queue's group's
   where the forge's queue landed it, whose id in hexadecimal is the
   result's words: `root.md`, section 6), and it closes, its branch
   deleted as its hold is released (section 11).

Lazy, too, for runs: before a repair or a resolution is created, the
change takes its base if its head lacks the base's tip, so a repair
starts from current code. It never updates only because its base moved.

### 8.3 Gates

- **A gate** is a verdict a change asks for before it lands, at its exact
  head:
  - *a review:* a task for an agent with a lens (memory safety,
    performance, a design), whose verdict is approve or request changes,
    with remarks;
  - *a person's approval,* through a person task addressed to the
    project's maintainers and answered in the client (jig's
    `domain/people.md`, section 8): the change approved at the head, or
    declined as "cannot do it", with a reason;
  - *a check:* one of the checks a repository without CI is adopted
    with, a task at exactly the head.
- **Blocking or advisory.** A blocking gate's verdict holds the landing;
  an advisory one's is read, and reported, and holds nothing.
- **A blocking gate that says no:** a review that asks for changes, or a
  check that fails, is a repair, given its remarks (8.4); a declined
  approval holds the change, with the person's reason. The verdict
  stands at that head, so a release alone holds the change again there:
  a person amends the change to drop the gate, then releases it, or
  cancels it.
- **Asked for by the change, the project and the plan.** A change's own
  gates are its parameters, amended while it is live (a coordinator
  adds a memory-safety review to one change); the landing rules add
  theirs to every change into a branch they cover (8.7). A gate added
  while a change is queued takes it out of the queue until it has its
  verdict. Designed, not built (8.9): the rules' gates are not copied
  into a change when it is made, but taken at each step from the rules
  the forge holds then, every rule covering the branch adding its own,
  enforced or not; a gate a changed rule drops is no longer waited on,
  and a review already running for it reports as an advisory one.
- **Valid at a head, and carried over clean updates.** A verdict is given
  at a head. Each gate says which later heads it still counts at
  (`freshness`): *exact*, the head itself; *clean*, every head reached
  from it by updates temper made that merged cleanly, since the change's
  own diff is unchanged. A check is always exact. Any other new head (a
  repair, a resolved conflict) asks for every gate again. The change
  keeps, in its state, the heads its clean updates led through, which is
  the fact the requirements read (8.7).
- **Posted, if the project wants:** a gate's verdict as a commit status,
  such as `temper/review/memory-safety`, which protection can require
  independently of temper. On GitHub, temper's App has `statuses: read`
  and no more, so temper writes none: `status` is false in the kinds
  there (4.4), the post, `Write::Status`, is refused at the connector,
  and nothing waits for it (skein's `forge-github.md`, 4.2). Every
  adoption there says so, and the verdict stays temper's own; the App
  asks for no more (`../protocol/setup.md`, 17.1).
- **When they run:** once CI passes on the head, by default, so a head
  CI fails costs no review; the change may ask to run them at once.

### 8.4 Repairs, resolutions and stalls

- **A repair** is a task for an agent whose workspace starts from the
  change's branch, its brief carrying why: CI's failing output on the
  head; a gate's remarks; for a clean update whose CI then fails (a
  semantic conflict), the base merged in and what landed in it; or, for
  a change the forge's merge queue ejected, the forge's reason, the
  pull request's `dequeued` (9.1). It pushes a new head, and the
  change is checked again.
  For each failed check, the brief carries its context, description and
  link, then the step that failed, by name, and the end of that step's
  log, where a step tells its failure; the job's whole log's end when no
  step failed. The log is read at the job attempt that failed on this
  head, within the brief's byte budget, and its truncation is said; a
  read that fails is said in its place (skein's `forge.md`, 5.2).
- **A resolution** is a task for an agent whose workspace starts from a
  merge in progress: the base merged into the branch, the conflicting
  files marked. The agent edits them and runs the checks, with no git
  writes of its own; the worker commits the tree as the merge, with both
  parents, and pushes it (`worker.md`, section 5).
- **A verdict whose post is refused** (8.3) is no repair and no stall:
  the gate counts as temper's verdict says, nothing is held for the
  post, and its `Forbidden` narrows what temper may do in that
  repository, as any refused write does (section 4), so no later
  verdict is posted there.
- **Bounded apart:** repairs past their limit, resolutions past theirs,
  updates past theirs, hold the change, saying why, and its requester
  hears.
- **Stalls:** each wait on the forge (CI that never reports, a
  mergeability never known, a forge's merge queue that neither lands
  nor ejects it, a gate's task stuck) is bounded by a stall counted from
  the head's push or the last release; past it, the change is held. The
  default, 30 minutes, outlasts an ordinary CI run so a healthy change
  is never held just for waiting, yet tells the person of a check that
  never reports within the same sitting. A wait for access is not one:
  while its forge is not connected, or its repository not reachable
  (4.1, 4.3), the stall does not count.
- **A release** lifts what it releases: repairs, resolutions and updates
  count again from it, its stall is counted from it, and the change
  decides afresh.

### 8.5 Drift

While its branch's writer slot is taken (jig's `domain/tasks.md`, 6.3),
by a run of one of its delegates, by a run that handed it the branch and
has not yet answered, or by an update it asked for, the change decides
nothing on its branch's head: forward moves by temper's identity are
taken as that writer's (jig's `domain/connectors.md`, section 11), and
the change reads the head afresh once the slot is freed. Otherwise:

- **Its pull request closed** by someone else: held, saying who and
  when. A release reopens it; a cancel closes the change.
- **Its branch moved** while nothing of temper's writes it, or by another
  identity, or not forward: held, the head kept as it is. A release
  takes the new head as the change's and checks it, gates asked again.
- **Its branch deleted:** held. A release creates the branch again at
  the head temper last knew, which the forge keeps under the pull
  request, and reopens the pull request.
- **Its pull request retargeted** to another base: held. A release
  retargets it back, or takes the new base as its landing branch if its
  requester amends the change so.
- **Merged by someone else:** landed, like any other landing.

Nothing is written over: a held change makes no effect until it is
released.

### 8.6 Cancelled

A cancelled change closes as every task does (jig's `domain/tasks.md`,
5.1): its live delegates close first, their runs ended; an entry already
sent, such as a merge, settles, and what landed counts; then its hold is
released (jig's `domain/tasks.md`, 6.5): its pull request closed with a
comment saying why, and its branch deleted, its commits still readable
under the closed pull request. A change whose merge landed meanwhile
ends landed, not cancelled.

### 8.7 The forge's requirements

The forge judges four requirements on its own facts (jig's
`domain/connectors.md`, section 7), which landing rules attach to its
merge, kind 4, on a pattern of landing branches (jig's
`domain/authority.md`, section 10):

| Requirement | Its parameters | Met | Wait | Refuse | Guarded or observed |
|---|---|---|---|---|---|
| 1, CI passed | none | CI passed at the head; for a repository without CI, every one of its checks passed at exactly the head; no CI verdict is needed for the explicit adoption choice to land without CI described below | CI pending or unknown; a check with no verdict at the head | CI failed; a check failed; a repository with neither CI nor checks unless explicitly adopted to land without CI as described below | guarded |
| 2, up to date | none | the head contains the base's tip | the comparison unread or failed | the head lacks the base's tip | observed, freshness 0 |
| 3, a gate | its number; `exact` or `clean` (8.3) | its verdict, at the head or, where `clean`, at a head the change's clean updates led through, approves | no such verdict yet, or one pending | such a verdict asks for changes | guarded |
| 4, an approval | a role; how many people; `exact` or `clean` | that many distinct people holding the role in the project approved the pull request at the head, or where `clean` at a head its clean updates led through | fewer have; the reviews too many to read whole | one of them asked for changes at such a head | guarded |

- **The explicit choice to land without CI** is offered at GitHub
  adoption only when Actions is on, there is no workflow and the landing
  branch has no required checks (`../draft/github.md`, 10.3). Choosing it
  adopts with CI off and allows adoption without replacement checks. Otherwise a
  repository without CI names the checks that stand for it. Neither this
  choice nor replacement checks waive a landing rule or required checks;
  CI pending or unknown on a repository adopted with CI on remains a wait.
- **Judged at the merge's head.** The core asks the forge for each
  verdict as it checks the merge (jig's `domain/root.md`, section 9),
  and the forge answers from the fresh reads its step made for that
  decision (5.3). Facts read for another head are wait.
- **Guarded or observed.** Each forge applies a merge only at the head
  it names, and a forge's merge queue takes a pull request in only at
  the head it names (section 6; skein's `forge.md`, section 9;
  `forge-github.md`, 5.1), so a fact pinned to that head still holds as
  the merge lands: CI, gates and approvals are guarded, and are marked
  as requirements that must be guarded. Up to date is observed: the base
  may move between the decision and the merge, and the forge does not
  check it then. Its freshness is zero, so only the reads of the step
  that decides the merge count. A base that moves in between lands the
  change on a tip it was not checked against, which the branch's own CI
  then checks (section 9). Through a forge's merge queue, the forge
  itself merges the head onto the queue's base and checks the group
  before it lands (9.1).
- **An approval,** the requirement, is a review a person posts on the
  pull request on the forge, as themselves, apart from a person's gate
  answered in the client (8.3). The forge reads the pull request's
  reviews, keeps each author's latest, and counts the distinct people
  among them whom the core knows by their forge user's id with the role
  in the project (section 4); a review that only comments counts for
  nothing.
  It is the one thing temper reads of what people write on its pull
  requests (section 13). A project has approvals only if its rules ask
  for them: temper's own default has none (`../README.md`, section 2).
- **Landing rules** come from the configuration, the deployment's and
  each project's (`../protocol/configuration.md`, section 3, `landing`),
  each covering a pattern of landing branches with CI, up to date, its
  gates and its approvals. Designed, not built: they are settings, the
  forge's own kinds (8.9). A merge meets every rule that covers its
  branch: a project's rules add to the deployment's, and never replace
  them; gates a plan asks for on one change are the change's own (8.3),
  and add to the rules', never replace them.
- **The usual rule** for `main`: CI passed at the head (or the
  repository's checks, without CI), the head up to date with the branch,
  and an agent's review as a gate. A project adds what it wants on every
  landing (a review with a lens, people's approvals) to its rule.
- **Enforced, and waited for.** A rule marked `enforce` makes each of its
  criteria a requirement the core checks at the merge, whatever any
  task's authority says: the rule. Every change also waits, on its own,
  for CI, its blocking gates and its base's tip (8.2): the mechanics. A
  rule not marked `enforce` leaves its criteria to those waits alone. A
  requirement that waits keeps the change at its turn until it is asked
  again; one that refuses holds the change, saying which, and never
  lands it. Approvals are waited for at the merge alone; for what the
  procedure waits for itself, a refusal at the merge is a bug in the
  procedure, counted (jig's `domain/authority.md`, section 10).
- **Force-pushing does not exist.** No kind forces a push or moves a
  branch backwards: a run's push is a fast-forward
  (`../protocol/git.md`, 4.8), an update merges the base in
  (section 10), and a branch found moved backwards is drift (8.5).

- **Designed, not built: two standing requirements** replace the four
  above as landing rules' attachments (`../protocol/configuration.md`,
  13.3). temper's stock rules attach both to the merge on every landing
  branch, and the forge judges each over every enforcing rule covering
  the merge's branch, as it holds the rules then:

  | Requirement | Its parameters | Met | Wait | Refuse | Guarded or observed |
  |---|---|---|---|---|---|
  | 5, the landing rules | none | each such rule's CI, gates and approvals are met, as 1, 3 and 4 above; or no rule enforces there | one waits, and none refuses | one refuses | guarded |
  | 6, up to date by the landing rules | none | each such rule asking for it is met, as 2 above; or none asks | the comparison unread or failed | the head lacks the base's tip | observed, freshness 0 |

  The change keeps the rule and the criterion of its last wait or
  refusal, by the rule's scope and number and the criterion's kind
  (a gate's number, an approval's role); its panel shows it (8.8), and
  its hold says it, in the forge's words.

### 8.8 Its panel

What the connector makes of a change for the client's panel
(`../client/forge.md`, 9.2), as the core asks for the document
(`root.md`, 5.3), from its record and its working set:

- **From its record:** its repository, by owner and name from the
  repository's record; its branch, landing branch and head; its pull
  request's number; its state, with the step it was held at; the task
  its one delegate runs; its gates and their verdicts, each judged at
  the current head with its freshness (8.3), a gate whose review runs at
  that head pending until it reports; and its repairs, resolutions and
  updates used, with their limits from `change_policy`.
- **CI at its head** is what its last step read (5.3), kept in memory
  beside its record, one verdict per change: a restart forgets it, and
  the panel says unknown until the change's next step reads it again.
- **Its place in its queue** is computed as the document is made,
  in section 9's order among the changes queued or first in its
  landing branch at the core's time: its place from 1, and the tasks
  ahead of it, nearest first.
- **Marked** as 9.2 of the client's says: its own subject each time its
  record is saved and each time a step reads a CI verdict at its head
  other than the one kept; and, when it enters its landing branch's
  queue, leaves it or moves in it, the changes behind it, nearest first,
  within the views' subjects marked per change (`root.md`, section 10).
  One past that bound shows its new place at its own next mark.
- **In its project's scope:** a change into a repository the project
  holds. Any other task, or a scope that is not that project, is
  unknown. Before the change's record exists, the engine still answers
  Gone; the client's bounded reopen and its limit are in
  `../client/forge.md`, 9.2, **Its subject**.
- **Kept as long as its record:** the record outlives the task's end, so
  a landed or cancelled change still has its panel.
- **Designed, not built:** what the landing rules wait on at its head,
  the rule and criterion its last verdict named (8.7), and the forge's
  charter it was made with (8.9).

### 8.9 The landing rules as settings

Designed, not built (`../protocol/configuration.md`, section 13). The
forge owns three of temper's kinds of settings: the deployment's
landing rules (kind 1), each project's (kind 2), and the forge's charter
(kind 3), whose values are bytes jig keeps and never reads (jig's
`domain/connectors.md`, section 17).

- **It judges them in the change's decision,** from its own state: a
  rule decodes, its pattern is a landing branch's within jig's pattern
  bounds, its gates and approvals within their bounds, and the forge's
  charter a charter's number (`../protocol/configuration.md`, 13.2). A
  refusal names the item and what fails, and the whole change is
  refused with it.
- **It holds them** decoded, the deployment's rules and each project's,
  within `landing_rules` per scope and `judge_projects` projects, and
  replaces a scope's whole when told its new values, in the decision
  that committed them. Then each live change into a repository a
  changed scope covers is stepped: a gate a rule added is asked for, a
  gate it dropped is no longer waited on, and a change queued for a
  gate it now needs leaves its queue until it has its verdict (8.3).
- **It reads the newest:** every merge is judged by the rules held at
  its decision, whatever revision its change was made in.
- **A change keeps its charter.** The forge's charter is read when the
  change is made, and the change's producing, repairs, resolutions and
  agents' and checks' gates run under it: its procedure's authority
  grants that charter's delegation alone, and is never rewritten. A
  change whose charter the newest settings no longer offer is held when
  it next needs a delegate, saying so; it is cancelled, or made again.

## 9. Landing queues

One per landing branch that has changes ready. A repository belongs to
one project (another may read it as context), so its goals' priorities
are comparable.

- **In the queue:** a change is in the queue once it is ready (CI passed
  and every blocking gate holds, at its head, and its task is not kept:
  no dependency it was given is pending, jig's `domain/tasks.md`, 5.7).
  It keeps the time it
  first became ready while it stays live, so leaving and coming back
  does not send it to the back.
- **Ordered by priority,** its goal's (jig's `domain/core.md`, 6.2), a
  change outside any goal taking its project's default, **then by that
  time.** Priorities move with the goals'. **Aging bounds starvation:**
  a change ready for longer than the deployment's queue window
  (`forge.queue_window_ms`, `../protocol/configuration.md`, section 7)
  goes before every change that became ready after it, whatever their
  priorities.
- **The first owns the queue.** Only the first is brought up to date and
  merged; the rest wait. It keeps its turn through a clean update, CI on
  its new head and its merge, so no CI run is wasted by a change
  overtaking it. It **leaves the queue** on a conflict, on a check that
  fails (CI, a gate asked again), on a gate added, when held or
  closing, when its task is kept by a dependency added to it, or once
  the forge's own merge queue holds it (9.1), and the next becomes
  first; it comes back when it is ready again, at its place by its
  first time. A kept change keeps its head, its checks and its gates'
  verdicts: once its last dependency is met, it is ready again at once
  if they still hold. A resolution never holds the queue.
- **Derived, not kept.** A queue is the ready changes into a branch,
  their priorities and their times; nothing of it is stored beyond the
  changes' own states, so a restart rebuilds it from them.
- **The branch's own CI.** A landing temper did not make may break the
  branch. Once CI fails on the branch's tip, the queue pauses and one
  repair is made: a change task into that branch, first in its queue,
  requested by the deployment on the project's behalf with the
  authority its policy gives such repairs, funded from the project's
  spend (jig's `domain/authority.md`, section 6), and news for every goal
  with changes waiting there. Every change in the queue waits for it
  rather than repairing a failure that is not its own. The pause ends
  when CI passes on the tip; repairs are made one at a time, up to a
  bound, past which the queue is held for a person, who may also release
  it as it is (a flaky check).
- **Where temper may not merge,** the queue still orders what is ready,
  for people to see; each change's landing waits for someone else's
  merge (section 13).
- **Where the forge keeps its own merge queue** on the branch, temper's
  queue decides only what enters it and when (9.1).

### 9.1 A forge's own merge queue

A branch whose rules require the forge's merge queue (GitHub's; the
adoption says so, 4.4) lands through that queue. Forgejo keeps none.

- **Entering is the merge.** The first's merge (kind 4) is made as any
  merge is: at exactly its head, its requirements judged at that head
  (8.7). The forge answers `Answer::Queued`, final for the call and not
  for the entry: the pull request is in the forge's queue at that head,
  neither landed nor refused. temper names no other call for it: the
  forge's rules, not temper, send the merge into its queue (skein's
  `forge.md`, 3.3).
- **temper's queue decides only what enters and when.** A change in
  the forge's queue leaves temper's turn, and the next ready change
  becomes first, and enters behind it when its own turn comes; the
  forge's queue groups and orders what it holds, and runs the branch's
  CI on each group. While the branch's own CI fails, nothing enters
  (section 9).
- **Landed or ejected comes later,** heard as any hint is, from a
  delivery or the delivery log (`../protocol/forge.md`, section 5 and
  5.1), and found by the client's read of the pull request: the merge's
  entry stays open in the client until that read finds the pull request
  merged or out of the queue (skein's `forge.md`, 3.3;
  `forge-client.md`, 7.7, pending in skein's code).
- **Landed:** the change is landed (8.2), and its landing's commit is
  the queue's group's (`Pull.merged`), not the head temper saw: the
  forge merged that head onto the queue's base with the changes ahead
  of it.
- **Ejected,** for any reason but its landing (its group's CI failed, a
  conflict with what entered before it, a person took it out): a
  repair, as a failed CI is (8.4), its brief carrying the forge's reason
  alone, the pull request's `dequeued` (section 20). The change takes
  its base before the repair starts (8.2), so a conflict there is
  resolved. A head another hand moved is drift, as ever (8.5).
- **Its place there,** `Pull.queued`, is read with the pull request and
  shown in the change's panel ("in GitHub's merge queue, n";
  `../client/forge.md`, a change's panel). The wait there is bounded by
  the stall (8.4).
- **Cancelled while there,** the change waits for its merge's entry to
  settle, landed or ejected, and what landed counts (8.6): temper never
  takes a pull request out of the forge's queue itself (section 20).

## 10. Updates: lazy and mechanical

- **Lazy:** a change takes its base when it is first in its queue, or
  about to get a run for another reason (8.2). It never does only
  because its base moved.
- **Mechanical:** the update merges the base into the change's branch,
  with no LLM. The forge makes it itself (its pull request's update, by
  a merge), so it is an effect, as a merge is. A merge rather than a
  rebase: the branch only moves forward, so it is never forced, and the
  heads before the update stay the change's own.
- **Made when the forge says, read afresh.** Forgejo has made the
  update when it answers; GitHub accepts it and makes it after (skein's
  `forge-github.md`, 5.2). Either way the update is done once the head,
  read afresh, contains the base's tip it asked for, which is also how
  its recovery finds it (section 6), and the new head is heard as any
  push.
- **A conflict** makes the forge refuse the update, and the change is
  resolved (8.4).
- **A semantic conflict,** a clean update whose CI fails while CI passes
  on the base's tip, is a repair, whose brief names the base merged in
  and what landed in it. While CI fails on the base's tip, the failure is
  not the change's: its queue is paused (section 9), and the change
  waits for the branch's repair instead.
- **Gates carry over** a clean update temper made, where their
  `freshness` is clean (8.3); after a conflict an LLM resolved, they are
  asked for again.

## 11. Branches

- **Named after the tree.** Every branch temper makes is under the
  repository's prefix (`temper/` by default, one deployment's on a
  repository: section 4) and its tree's root task:
  - a change's own branch: `temper/<root>/c<task>`;
  - a task's saved work: `temper/<root>/s<task>`, never the change's
    branch, so it starts no CI and moves no pull request;
  - an agent's task's own branch, for runs that push outside a change,
    as a chat making a small fix does: `temper/<root>/r<task>`, in each
    repository its runs may write, the project's home among them, which
    the task holds from the commit that makes it and hands to the change
    task that lands it.

  A task made before its project's home was adopted holds no run branch
  there: its runs read the home without pushing, and its brief says why.
  A new task gets the branch.

  They are siblings under `temper/<root>/`, so no name is the prefix of
  another as git refs forbid, and one grant covers a tree's branches;
  a task's own branches are named in its batch symbolically and resolved
  once it has its number (jig's `domain/authority.md`, section 4).
- **Held by the change** that lands it, and written by its delegates one
  at a time (jig's `domain/tasks.md`, 6.3).
- **Landing branches are never made by a push.** A landing branch must
  exist when it is adopted or named; a goal's own branch (11.1) is
  created at its base's tip by the change that lands it, as an effect.
- **Deleted as their tasks close,** as the release of their holds (jig's
  `domain/tasks.md`, 6.5): a landed change's branch, a cancelled one's,
  and a task's saved work. A failed change's branch is kept for a person
  to read, and deleted as its tree's root closes. A release waits for a lost
  writer's reread, and fails only if that reread fails.

### 11.1 A branch per goal

Whether a goal's changes land into a branch of its own, which then lands
whole, or each into the shared base, is the plan's choice like any
other: a change's landing branch is its parameter. Landing each into the
base is the usual answer: plans meet early and in small pieces; a goal
that needs another's work has it once that lands; and repositories that
pin each other's commits (temper locks skein's in `Cargo.lock`) pin a
commit of the base, which no later squash of a goal's branch replaces.
A goal's own branch suits work likely to be dropped; a goal
checked whole before it lands (by an end-to-end run, by a review of the
whole); intermediate states that would break the base; and upstream
contributions (section 14). Such a branch is short-lived: it is a change
of its own, landed by the change procedure like any other, and it takes
the base's landings that touch its area as they come, by an update. A
project's rule may require or forbid it.

## 12. Issues

The forge stays readable: issues show goals and their milestones, pull
requests their changes, for people who prefer to read there and for the
history they keep if temper is unplugged. Issues are projections (jig's
`domain/core.md`, 8.1), written and never read back:

- **One per goal,** in its project's home (jig's `domain/core.md`, 6.1),
  created when the task becomes tracked. Its body is the goal and its
  plan as a task list, which the forge renders with its progress,
  rewritten as the plan changes, at most once per `interval` and only
  when what it projects changed.
- **Milestones as comments,** each once: the plan accepted; each revision
  and its reason, from the tree's history; each change landed or held;
  each report the goal's agents mark as worth keeping ("why not library
  X"), and the design agreed, in their words; the goal finished, after
  which the issue is closed.
- **Pull requests refer to it** ("Part of `ai/temper#12`"), so the forge
  links each change to its goal, across repositories too.
- **Findings as issues:** a bug found while exploring, a follow-up a
  review raised, a flaky test, created by an agent's `effect_forge` tool
  (`agent.md`, 4.3; jig's `domain/connectors.md`, 4.5), keyed. Their
  state (open, taken up by a goal, dropped) is the store's, shown in the
  client (`../client/forge.md`, 7.3); taking one up is a goal that
  names it.
- **A projection, not a procedure** (jig's `domain/connectors.md`,
  section 8). The `issues` child domain is fed each goal's state, its
  plan and its milestones by the root: it is part of tracking, not work
  anyone asked for, so it has no task of its own. Its effects are keyed
  by the goal and what they write, made with the authority the project's
  policy gives its goals' projections (jig's `domain/authority.md`,
  section 6), in its home; the last ones, the closing milestone and
  closing the issue, are made as the goal closes.
- **Issues others open** in an adopted repository are participating
  objects, which may become intake later.

### 12.1 Accepted desired generations

The forge accepts a final core feed synchronously in the decision that
marks it fed, saving the accepted desired view and its generation in the
same IssueRow write as the core's final header. If it cannot accept the
view within its bounds, it refuses within that decision. Acceptance is
durable across restart; it does not depend on delivery after commit.

`IssueRow.closing_generation: Option<u64>` follows `desired`. It is Some
only for an accepted final desired view and is cleared when a non-final
desired view replaces it. Restart restores it with desired. A late write
completion rechecks the whole current desired: its body, milestones and
closing write must all be complete, with no pending write. Every issue
row for that goal and connector must agree on the same saved Some
generation before `ProjectionSettled { goal, generation }` is emitted.
The root echoes that saved generation; it never stamps a callback with
the core's current generation. A refused newer desired view cannot make
an old completed desired settle its cohort, including when a timer later
asks the connector to project the old view again.

### 12.2 Failure episodes and recovery

Before admitting projection work or refusing it without a write, the
connector obtains a source through jig's ProjectionBegin. That source
survives the flight, entry and retries. Permanent projection refusal
reaches jig's durable failure receipt; Busy and Wait remain transient.
Projection outbox outcomes only settle the core's resource writer.
The receipt delivers once-per-episode quiet news to the original goal,
independently of task-effect holds (jig's `domain/connectors.md`, 8).

Explicit recovery stages an infallible reset with reserved room. The
root routes CommitProjectionRecovery synchronously, so the connector's
reset and the core's source fence commit together. Pending receipts
refuse recovery as Busy; missing or final state answers Missing. Armed
recovery preserves its existing fence without writing; only Reported
rearms at the current connector counter. Retained entries covered by the
new fence are abandoned or obtain a new Begin. Recovery of projection
work and ordinary task-effect settlement have separate ownership.

The synchronous connector hand-off is
`Event::CommitProjectionRecovery { goal, after }`. The core's keyed held
answer becomes the root's internal
`Delivery::ProjectionRecovery { connector, owner, result }` after commit.
The root routes it to its owner, preserving all three fields; it neither
discards the result nor translates it to an unavailable service response.

## 13. Participating objects

Not needed first, and kept possible with one mechanism (jig's
`domain/core.md`, 8.2), for three cases: outside contributions to a
project temper works on; a repository where temper may not merge; and
temper's own contributions upstream (section 14).

- **What people write on what temper owns is no input.** A comment on
  temper's pull request or issue reaches no task; the pull request's
  body says where to reach temper instead. The one fact temper reads
  there is a review's verdict by a person a landing rule's approval
  counts (8.7).
- **Outside contributions** to a project temper works on: their issues
  and pull requests, which a task may triage and review. A task given
  one subscribes to its comments and reviews; what others write reaches
  its inbox as news, in order, from a position the connector keeps.
  Outside contributors are not the project's parties unless adoption
  seeded them (section 4) or they sign in: what they write is a
  participant's words.
- **A repository where temper may not merge:** a change there is ready
  when its CI and gates hold, and then waits for someone else's merge,
  which is a landing like any other. People's reviews on its pull request
  are news for the change; a request for changes is a repair, given its
  remarks.
- **What temper writes** there (a reply, a review) is a participant's:
  drafted by an agent's task, and, where the project's policy says so,
  proposed to a person before it is posted.

## 14. Upstream contributions

Not needed first, and kept possible:

- **A fork,** on the upstream's forge where it can be, so that upstream's
  CI runs on it unchanged and the pull request is native there. Its main
  follows upstream's, fast-forward only, and temper's own changes land
  into a work branch in the fork: a goal's own branch (11.1).
- **The submission is a task:** brought up to date with upstream, its
  commits shaped to upstream's conventions, described and signed off,
  and accepted by a person before anything is posted, often under that
  person's own identity.
- **Once posted,** its pull request is a participating object
  (section 13): a task reads the maintainers' reviews, updates the
  branch, and drafts replies a person approves.

## 15. A goal and its landings, step by step

### 15.1 A goal, as tasks

A person, P, reaches a plan for adding OAuth login to temper:

```
P, a person
├─ T0  a chat with P about temper          agent, a chat
└─ T1  add OAuth login                     agent, a coordinator's charter; tracked;
   │                                       T0 proposed it, P accepted it
   ├─ T2  spike: library X                 agent, read-only: a report
   ├─ T3  spike: a hand-rolled flow        agent, read-only: a report
   ├─ T4  choose       after T2, T3        person P: a choice
   ├─ T5  design       after T4            the forge's change, a design review as its gate
   ├─ T6…T9  changes   T5 as their input   the forge's change, made by T1
   │                                       once it read T5's result
   │     T7 amended while live: a memory-safety review as a gate
   └─ T10 try it       after T6…T9         agent: a report on what landed
```

- **T1 is proposed, explicitly.** Its authority (a budget, landing into
  main) is beyond T0's, so T0's `delegate` is refused, naming what it
  lacked, and T0 proposes it (jig's `domain/tasks.md`, section 9). P
  accepts it and funds it from P's pool. A goal a chat proposes is
  requested by the person who accepts it (jig's `domain/core.md`, 6.2),
  so T1 is P's, and outlives T0, which keeps a reference to it. Within
  T1's authority, its own delegations need nobody's acceptance.
- **T1 is a goal.** Its issue shows the plan as T1 revises it
  (section 12); its changes queue for main at its priority (section 9);
  landings in temper are news for it (section 7).
- **Results flow along dependencies:** T2's and T3's reports reach T4,
  which P answers in the client; T4's choice reaches T5's brief.
- **T6…T9 are a revision,** made by T1 once T5 landed, within its
  authority; T5 has ended, so its result reaches them as an input, not a
  dependency.
- **The memory-safety review is one amendment:** T1 adds a gate to T7
  (8.3). The change procedure runs a review task at each head of T7 its
  verdict does not carry over to, and holds the merge until it passes.
- **Under each change,** the procedure makes its own tasks: producing,
  repairs, reviews, resolving conflicts (section 8).
- **Had P asked for a small fix,** T0 would have made it itself and
  handed its branch to one change task (8.1), and there would be no T1.
- **Had P closed T0** while T1 ran, T1 would go on: it is P's.

How T6…T9 land while another goal lands into the same branch is 15.2.

### 15.2 Landing, step by step

One repository, temper, with main at M0, and two goals in flight:

- **goal A:** change A1 is ready to land (CI green on its head a1,
  reviewed), and A has steps still to come;
- **goal B:** change B1 is ready to land (CI green on b1, reviewed), and
  change B2 is being produced by a run that started from M0.

A1 and B2 both change `limits.rs`; nothing else overlaps. A has the
higher priority.

| When | Who | Does | Afterwards |
|---|---|---|---|
| t0 | Engine | A1 is first in main's queue, and its head holds M0: merges A1 at exactly a1, through Forgejo | main is M1 |
| t0 | Engine | news for A (its own step landed) and B (A1 landed in temper, overlapping neither B1 nor anything B has pushed): neither wakes | B1 and B2 untouched |
| t1 | Engine | B1 is first, and its head lacks M1: asks Forgejo to update it | |
| t1 | Forgejo | merges M1 into B1's branch, cleanly | B1's head is b1′ |
| t1–t2 | CI | passes on b1′ | |
| t2 | Engine | B1's review carries over the clean update; merges B1 at exactly b1′ | main is M2 |
| t2 | Engine | news for B (its own step) and A (B1 overlaps nothing of A's): neither wakes | |
| t3 | Agent, worker | B2's producing run finishes; the worker pushes it | B2's head is b2, built on M0 |
| t3 | Engine, CI, agent | opens B2's pull request; CI passes on b2; a review task approves it | B2 is ready |
| t4 | Engine | B2 is first, and its head lacks M2: asks Forgejo to update it | |
| t4 | Forgejo | refuses: B2 and A1 both changed `limits.rs` | a conflict |
| t4 | Engine | B2 is resolving: a task for an agent; news for B, which does not wake it while the task resolves it | |
| t4 | Worker, agent | the worker prepares the merge in progress; the agent resolves `limits.rs` and runs the checks; the worker commits the merge, with both parents, and pushes it | B2's head is b2′ |
| t5 | CI, agent | CI passes on b2′; a review, asked again since an LLM resolved a conflict, approves it | |
| t5 | Engine | merges B2 at exactly b2′ | main is M3 |
| t5 | Engine | news for B (its last change landed: its coordinator's policy wakes on the last of its delegates done) and A (no overlap: kept) | |
| t5 | Coordinator B | its one run: reads its inbox (A1 and B1 landed, the conflict resolved) and finishes goal B with its report, which the issue's last milestone carries | goal B is done |

## 16. Restart

At a restart (jig's `domain/engine.md`, section 6; jig's
`domain/connectors.md`, section 14) the connector is given its records
from the store: the projects' repositories and what temper may do there,
the live tasks' resources and holds, the changes' and issues' states, and
its outbox's entries with their attempts. The deployment's id is given
to the client first, as its namespace, before any restored entry or live
resource; the client restores each forge's writer from its own record
before any entry too (4.1). The client's part is skein's
`forge-client.md`, section 8: what it reads afresh before anything else
steps, how it settles an outbox entry as its class says, and where its
listings start. That fresh read, each adopted repository's, covers what
changed while the engine was down on GitHub too, since no delivery
log's guid is kept across starts (`../protocol/forge.md`, 5.1).

Each forge starts not connected, whatever it was before the restart, and
is connected as the routing tells the root (`../protocol/forge.md`, 2.1
and 8.1); the adopted repositories of each managed or owned Forgejo are
named to the routing for its reconcile (4.2). Until a forge connects, its
reads and entries answer `NotConnected`, which narrows nothing (4.1), so
the steps below run on each forge as it connects. Designed, not built:
before its records, the connector hears its kinds' current values
(8.9), so a restored change is judged by the rules as they are. temper's
steps:

1. it reads afresh each repository's landing branches' tips and each
   live pull request (head, state, mergeability, CI, its place in a
   forge's merge queue), before any change steps;
2. it settles its outbox, each entry as its class says (section 6): a
   pull request found by its branches, a merge by its pull request's
   state (merged, in the forge's merge queue, or neither), a creation by
   its key after the entry's start, a set written again; one not found
   waits for the retry deadline its last attempt committed, and an
   unrecoverable one holds its task for a person;
3. it steps every change once, and every goal's issue.

Its listings start from the newest time the store kept for each
repository, inclusive, so what changed while the engine was down is
read once.

## 18. The world

The connector's world, `tests/engine/forge`, runs its subtree with the
fake forge below it (`testing.md`, 4.2) and the root scripted above
it; the tiers that run it through bytes and on a real Forgejo are
`testing.md`, section 2. Its stories: a change produced, opened,
checked, gated, queued and landed; CI failing, repaired; a gate asking
for changes, repaired, its verdict given again; a gate added while
queued; two changes into one branch, the second updated cleanly and its
review carried over; an approval carried over a clean update and asked
again after a repair; a change that leaves the queue for a conflict and
comes back at its place; a low-priority change aging past the window
ahead of later ones; a conflict resolved from a merge in progress; a
semantic conflict repaired; the branch's own CI broken by a landing
temper did not make, the queue paused and repaired; its pull request
closed by someone else, held, released and reopened; a landing that
overlaps a goal's change, and one that does not; a goal's issue
written, revised and closed; a merge whose answer is lost, found merged
after a restart; a mergeability unsettled within the window, then
settled; a forge preloaded with ten times the history, idle at the same
cost. Designed, not built (8.9): a landing rule changed between two
merges, the second judged by the new rule; a gate a rule adds asked for
on a queued change, which leaves the queue, and a gate a rule drops no
longer waited on; a project's rule adding an approval; a rule refused
in its change's decision; a change kept on the charter it was made
with, and one held when that charter is no longer offered; a restart
that hands the values back before the records.

Its stories of access and of GitHub's ways, the root and routing
scripted: a forge connected while temper runs, an adoption refused as
not connected before it and made after it, the writer given then and
restored at the next start; an owned forge set up from the client,
each state's row and `SetUp`'s answer checked together, with and
without the box: asked when offered and none sets up, "busy" while
any sets up, "not offered" connecting, connected, stopped, not owned
or asked, or not configured, "not under the box" without the box and
after `BoxClosed`, each state the routing tells kept as told and none
restored; a forge stopped while a change waits on it,
nothing narrowed, its entry committed again once it connects; a managed
Forgejo's adoption whose access is refused, refused and named, and one
given, then read; the picker's listing on each kind of forge, a managed
one's admin's view read while another run writes, one not listed, an
installation's repositories changed and an adoption marked,
then unmarked when it is listed again; an adoption on a branch whose
rules want a merge queue and signatures, said in words with the one
setting; a merge queued, landed at the group's commit, and another
ejected and repaired; an update GitHub makes after it answers, found by
the head read afresh; no commit status written on GitHub, and nothing
held for it. The forge below is skein's fake, its GitHub layer
included: no example calls github.com or uses a real credential.

Its referee, beyond jig's for every connector (jig's
`domain/connectors.md`, section 15): nothing lands without every
requirement its branch's enforced rules name, guarded ones met at the
exact head merged and up to date met on the reads of the decision that
merged it; merges land exactly the heads decided; temper never writes to
what it does not own, nor forces a push; a held change makes no effect;
a landing wakes a goal exactly when it overlaps; no change waits in a
queue past the aging window behind changes that became ready after it;
temper's own pushes are never taken for drift; the calls made while
idle are bounded by the limits; nothing is narrowed, and no change held,
for a forge not connected; a change whose merge was answered queued is
not merged again until the forge has landed or ejected it, and lands at
the group's commit.

## 20. Open questions

- **A queue's throughput:** landings into one branch wait on CI one after
  another. If that binds, landings are batched: tested together, and
  bisected when they fail.
- **Conflicts found before either side lands:** temper sees every open
  change, and could try them against each other and warn both goals.
- **Checks without an LLM:** a repository's checks run as agents' tasks
  at the exact head (8.3); a checks-only run on a worker, running the
  project's commands with no model, would cost no turns (`worker.md`,
  section 12).
- **A person's approval as a rule:** temper takes it two ways, a person
  gate answered in the client (8.3) and a review the person posts on the
  forge, which a landing rule counts (8.7). Whether an approval given in
  the client should also be posted on the forge as the person, so that
  the forge's protection requires it independently, needs a forge
  credential of the person's (jig's `domain/people.md`, section 12).
- **A failing check's repair brief:** a check gate that says no is
  repaired as a review that says no is, and its repair's brief carries
  the pull request's reviews section (8.4). A check's verdict is not a
  review on the forge: whether the brief should carry the check's own
  output instead is open.
- **A failure that is the runner's, not the change's:** a job whose
  set-up step failed, or whose runner was lost, is a repair like any
  other. Running the job again first, through the API, is
  `../draft/ci-on-forgejo-17.md`, with the stalls' reasons and the
  agents' log tools.
- **Two Forgejo behaviours no fact records yet** (`../protocol/forge.md`,
  section 14): that CI after a clean update runs on exactly the head the
  update made (section 10); and where a merge lands when its pull
  request is retargeted after the fresh read before it, since Forgejo's
  merge is conditional on its head alone (section 6).
- **Intake:** issues others open as a source of goals.
- **A resource refused as forbidden:** whether the connector should hear
  when it is readable again, rather than read it on a backoff.
- **A merge style per repository,** named by the project rather than the
  forge's (4.4). Default: on Forgejo the repository's default (skein's
  `forge.md`, section 12); on GitHub the first the repository allows of
  merge, squash and rebase; either shown at adoption.
- **A change whose forge is not connected,** or whose repository is not
  reachable (an installation suspended, a repository it does not select):
  it waits, its stall not counting the wait, and nothing is narrowed
  (4.1, 4.3). The alternative is holding it for a person past a bound.
  Default: it waits, and the client shows why.
- **`Forbidden` and `Missing` taken as access lacking** on a repository
  an installation suspended or stopped selecting (4.3), rather than as
  facts that narrow what temper may do there. Default: access lacking,
  while the installation's events say so; any other refusal narrows, as
  ever.
- **Several changes in a forge's merge queue** (9.1): a change there
  leaves temper's turn, and the next enters behind it. The alternative
  is the first keeping its turn until it lands or is ejected, one at a
  time, as temper's own queue does. Default: several, the forge's queue
  grouping them.
- **The update before entering a forge's merge queue** (9.1): the first
  takes its base as any first does (section 10), though the queue merges
  the base itself. The alternative enters it as its head is, saving a CI
  run. Default: as any first.
- **What failed on a queue's group** (8.4, 9.1): an ejection's repair
  brief carries the forge's reason alone, the pull request's `dequeued`,
  since skein's `Pull` names no commit of the group. The alternative
  reads the group's CI and gives the repair its failing output.
  Default: the reason alone.
- **Taking a change out of a forge's merge queue** when it is cancelled:
  the vocabulary has no call for it. Default: none; the cancel waits for
  the landing or the ejection (9.1). The alternative is closing the
  pull request, which the forge's queue drops.
- **Signed commits required** on a landing branch (4.4): temper's
  workers do not sign. Default: landing there is a wait for someone
  else's merge, said at adoption; signing is later.
- **Connecting a forge as another user** (4.1): refused, since the keys
  and echoes already on the forge are the old user's. Moving a
  deployment to another App or user, and disconnecting a forge, are
  later.
- **Which checks are CI on GitHub, without required checks:** temper's
  draft (`../draft/github.md`, section 9) takes the repository's Actions
  workflows' check runs there, so that another App's check run (a
  Codecov's) is not CI. That needs each row's App, which skein's
  `Status` carries only for a failed Actions run; until skein's dialect
  carries it on every row, every row is CI (5.3), another App's check
  runs among them, and its suites with no run list nothing.
