# Repositories

Kind: concept

A repository is a forge's repository adopted into a project: owned, a fork or context, with temper's own branches under a prefix, its CI or its checks, and one of them the project's home.

## What it is

- **An adoption** records, for one repository on one forge: its
  project; whether it is the project's home; its owner and name; its
  role; the prefix of temper's branches; whether changes wait for the
  forge's CI or for checks of temper's; and what temper may do there,
  read from temper's permission on it when it was adopted.
- **Its role:**
  - `owned`: temper writes changes there and lands them;
  - `fork`: written as an owned one, its branches under the prefix
    temper's;
  - `context`: read, never written, and never the home.
- **Its prefix,** `temper/` unless the adoption names another: every
  branch under it is temper's, and temper writes no other.
- **Its landing branch** is the branch its changes merge into: the
  repository's default branch, as the forge had it at the adoption.
- **CI or checks:** a change waits for the forge's CI at its head,
  or, adopted without CI, for its checks: gates by their numbers, each
  run by an agent under the forge's charter (temper help landing-rule).
- **The home** is the one repository where the project's goals show as
  issues (temper help project).
- **A run names it** by `owner/name`, and may leave it out where only
  one fits.

## How it changes

- **Adopted:** `repository adopt` adopts one into a project; `forge
  setup`, and `forge connect` with a first repository, make one and
  adopt it (temper help forge).
- **Adopted again:** the same repository adopted again into the same
  project replaces its record.
- **Ended with its forge:** the playground's repository leaves the
  project's listings when the playground ends.

## What temper guarantees

- **A context repository is never written,** and no change targets
  it.
- **temper's branches alone:** an adoption is refused while a branch
  under the prefix is one temper does not know, such as another
  deployment's; choose another prefix. A branch of temper's moved by
  someone else holds its change, never overwritten.
- **One home a project:** a second is refused, and the refusal prints
  the adoption to make instead.
- **What it may do is the forge's:** temper writes only where its
  permission on the repository allows, and lands only by a merge style
  the repository permits.

## Acting on it

- **ctl:** `forge repositories` lists what a forge reaches; `repository
  adopt` adopts one; `change show` reads a change to it (temper help
  change). Each verb's arguments are its help: `temper ctl help
  repository adopt`.
- **temper's runs:** `change` asks for a change to one; the forge
  reads read it, by `owner/name`.

## Limits

- temper lands every change on the repository's default branch, read
  at the adoption, whatever landing branch the adoption names.
- temper has no call that ends an adoption or changes one but by
  adopting it again.
- A repository belongs to one project at a time: adopting it into
  another, even as context, moves it there.
- An adoption without CI names at least one check.
- A fork is written as an owned repository: temper opens no pull
  request to the repository it was forked from.
