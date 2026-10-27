# Projects

Kind: concept

A project is where work happens: the repositories adopted into it, one of them its home, the parties with a role in it, its own policy, and the period its budget comes from.

## What it is

- **Its number or its name** names it in ctl's calls: `1` or `Project
  1`. `me` lists your projects, each `N · NAME · ROLE`.
- **Its repositories,** adopted from the deployment's forges, each
  owned, a fork or context (temper help repository). A project may
  adopt several, on several forges.
- **Its home** is one of them, chosen at its adoption: the repository
  where temper keeps the project's goals, one issue each (temper help
  goal). A project has at most one home, and a context repository is
  never one. Every task of the project may read its home, whether or
  not it may write there.
- **Its members** are the parties with a role in it: owner,
  maintainer, member, observer, or a numbered role its policy defines
  (temper help party).
- **Its policy** is its own: what its work may do and spend, and what
  each role may start (temper help policy).
- **Its period** holds what its work may spend; each member's pool is
  drawn from it (temper help budget).

## How it changes

- **Made:** `temper init` makes project 1, `Project 1`. Setting up the
  box's Forgejo, or connecting a Forgejo with a first repository, makes
  a new project, named after the repository, unless you name one that
  exists. A new project's policy is a copy of policy 1 as it is then.
- **Given repositories:** `repository adopt` adopts one into a project
  that exists; the first one adopted becomes its home unless the
  adoption says otherwise.

## What temper guarantees

- **One home:** a second home is refused, and the refusal prints the
  adoption to make instead.
- **Its policy is its own:** a change of policy 1 reaches the projects
  made after it alone.
- **A goal shows in the home:** each tracked task of the project is an
  issue there, updated as the task changes (temper help goal).

## Acting on it

- **ctl:** `me` lists your projects; `forge setup` and `forge connect`
  can make one with its first repository (temper help forge);
  `repository adopt` adds a repository; `task list`, `inbox` and
  `model list` read one project's, and `chat start` starts a chat in
  one.
- **temper's runs** work in their task's project: `change` writes to
  one of its repositories, and the forge's reads name one by
  `owner/name`.

## Limits

- temper has no ctl group `project`: no call reads a project's home,
  members or period, renames it, or changes its roles.
- temper does not make a project from ctl but with its first
  repository.
- temper has no call that moves a task or a repository to another
  project.
- A project's period never renews: its budget is what its first period
  holds (temper help budget).
