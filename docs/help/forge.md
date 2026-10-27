# Forges

Kind: concept

A forge is where repositories live, GitHub or Forgejo, numbered in the configuration; temper connects to one or runs its own, and works only in the repositories adopted from it.

## What it is

- **Its kind:** GitHub or Forgejo.
- **Its number** is its identity, from the configuration: another
  number is another forge. `temper init` writes forge 1, the
  playground's fake Forgejo; forge 2, GitHub; forge 3, a Forgejo temper
  runs, started only when set up.
- **Its mode,** as `forge list` prints it:
  - `connected`: a forge someone else runs, reached with a credential
    temper holds, such as GitHub through an App;
  - `managed`: a Forgejo someone else runs, connected as its site
    admin: temper keeps the admin's token and makes its own user there
    (temper help forgejo);
  - `owned`: a Forgejo temper runs on its box, with a runner for its CI
    (temper help box);
  - `fake`: the playground's (temper help playground).
- **GitHub's App:** temper reaches GitHub through an App: one of your
  own, by its id and private key; temper's own, made from its manifest
  in a browser; or one another owns, made read-only in the
  configuration before the start (temper help github).
- **An installation** of the App on an account reaches the repositories
  that account chose. temper works through the installations it has
  accepted.
- **The repositories it reaches:** an accepted installation's on
  GitHub; what the site admin sees on a managed Forgejo. Reaching one
  is not adopting it (temper help repository).

## How it changes

- **Its connection,** as `forge list` prints it: `Not connected`, `Not
  set up`, `Setting up` (then `: fetching`, `: preparing` or `:
  starting`), `Set up failed: REASON`, `Connecting`, `Connected as
  LOGIN (ID)`, or `Stopped: REASON`. GitHub's App connected with no
  installation accepted prints `Connecting: installed nowhere yet` or
  `Connecting: no installation accepted yet`.
- **Set up:** a box's owned Forgejo is fetched, prepared and started
  with its runner, then makes its first repository with a starter CI
  workflow and adopts it.
- **Connected:** GitHub by its App, a Forgejo by its admin's token; a
  managed Forgejo can make and adopt a first repository as it
  connects.
- **Its installations,** on GitHub: one made through temper's install
  link is accepted at once; one made on GitHub waits to be accepted. An
  installation not accepted prints ` · not accepted · uninstalled at
  the next reconcile`, or ` · not accepted · kept: the App is
  read-only`; a suspended one ` · suspended`.

## What temper guarantees

- **temper writes only what it owns:** in an adopted repository, its
  branches under the adoption's prefix, the pull requests of its
  changes, and its goals' issues in the home; elsewhere it reads, and
  comments or posts statuses where its tools ask.
- **No branch is overwritten:** a push extends the branch temper last
  wrote; a branch moved by someone else holds its change instead.
- **A merge names its head:** temper merges a pull request at the head
  its checks passed, and a moved head or base is checked again first.
- **A read-only App is left as it is:** temper reads its hook,
  changes nothing of the App, and uninstalls no installation it did
  not accept.

## Acting on it

- **ctl:** `forge list` reads the forges, their modes, connections and
  installations; `forge setup` sets up a box's Forgejo; `forge
  connect` connects GitHub or a managed Forgejo; `forge install`
  prints a link that installs the App; `forge accept` accepts an
  installation made already; `forge repositories` lists what a forge
  reaches. Each verb's arguments are its help: `temper ctl help forge`.
- **temper's runs** read an adopted repository's issues, pull
  requests, reviews, checks, branches, files and CI logs with the forge
  reads (`read_items`, `read_pull`, `read_job`...), write with
  `effect_forge`, and hear its news with `subscribe_forge`.
- **Yourself:** read diffs, reviews, CI runs and logs on the forge,
  at the numbers and links ctl prints (temper help ctl).

## Limits

- temper cannot replace a forge's key or forget it: a second connect
  is refused with `connected already` (exit 2).
- temper does not connect a Forgejo by a user's token: only as its
  site admin.
- temper sets up an owned Forgejo under a box alone: elsewhere, `forge
  setup` is refused with `refused: not under the box` (exit 2).
- Connecting another forge ends the playground.
- ctl opens no browser: it prints the address a person opens on the
  engine's machine.
