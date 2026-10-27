# Parties

Kind: concept

A party is who acts in a deployment, a person or a service, numbered, with a role in the deployment or in a project; ctl acts as the party its directory signed in as.

## What it is

- **A person** is a party who signs in: by a link the deployment
  prints, or with Google where the configuration allows it.
- **A service** is a party a project's owner makes for a program,
  signed in by the deployment.
- **Its number** names it: `party 1` is the first owner of a box.
  `temper ctl DIR me` prints your party, `party N`, its login and
  name, then its projects, each `N · NAME · ROLE`.
- **Its roles:** owner, maintainer, member and observer, or a
  project's numbered role, `role N`, which the project's policy
  defines (temper help policy). A party holds a role in the deployment,
  in a project, or both.
- **Its pool:** in a project, a party's work spends from a pool, drawn
  from the project's period up to what its role may spend; `me` prints
  `pool left` beside the project (temper help budget).
- **Its bounds:** what it may start, and with what, as `me` prints
  them: the tools, models and spend a task of its may have, the
  charters and procedures it may name, the deadline it may give.

**What each role may do:**

- the deployment's owners change its settings and move running work
  (temper help settings), and hold the deployment's accounts (temper
  help account);
- a project's owner adopts repositories into it, changes its policy
  and roles, makes its services and sizes its pools;
- owners and maintainers act on every task of the project;
- an owner, a maintainer, a member or a numbered role starts chats,
  sends messages and answers, and amends tasks, within the policy and
  what its pool can fund;
- an observer reads, and starts nothing;
- a party with a role proposes what its role cannot do (temper help
  proposal).

## How it changes

- **Made:** the configuration's owners become parties, and the
  deployment's and the first project's owners, at the first start. On
  a deployment that signs in with Google, each person its configuration
  lists as a member signs in as a party of their own.
- **Signed in:** `temper serve` prints a sign-in link for the first
  owner on its stdout, and `temper box` passes it on. A link signs in
  once, within a short while. A spent or expired link, given again,
  makes the deployment print a fresh one; the box prints it after `A
  fresh sign-in link:`.
- **Signed out:** `sign-out` ends the sign-in in `DIR` (temper help
  ctl).

## What temper guarantees

- **Every act is a party's:** the engine decides it under that party's
  roles and bounds.
- **A secret is read from a file, never from an argument:** a model's
  key, a forge's key or token, and an account's record are each read
  from a regular file owned by you, with no permission for group or
  others, and not a link. A file that is not refuses the call (exit 1),
  naming its fault: `key file mode 0o644 permits group or other
  access`, `key file must not be a link`, or `key file must be a
  private regular file owned by this user`; `account import` says
  `record file`.
- **A secret's bounds:** a model's key holds 1 to 4096 bytes and a
  forge's 1 to 4089, once one final newline is trimmed; an account's
  record at most 131072.
- **The deployment's own secrets** (its workers' secrets, Google's
  client secret) are private files too: a start refuses one readable by
  others or reached through a link, naming its member and file.

## Acting on it

- **ctl:** `sign-in` signs `DIR` in as a party, `sign-out` ends it, and
  `me` reads it; temper help ctl gives the loop.
- **The secrets' tools:** `account add-key`, `account replace-key` and
  `account import` read a model's key or an account's record (temper
  help account); `forge connect` reads a forge's key or token (temper
  help forge).
- **People:** a person joins a deployment that signs in with Google by
  a change of its configuration and a restart (temper help server).

## Limits

- temper does not let ctl make a party, invite a person, give a role
  or make a service.
- A box made by `temper init` has one person, its owner, and signs in
  no other.
- A `DIR` holds one party's sign-in; several parties take several
  directories.
