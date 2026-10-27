# temper

Kind: index

temper runs software work for people: agents take tasks on the repositories of a forge and land their changes there, and this page is where an agent starts.

## What temper is

- **A deployment** is temper's engine, its workers and the forges it
  works on. `temper init` writes its configuration, `temper box` runs
  it on one machine, `temper serve` runs its engine.
- **An agent works in a deployment** for a person with `temper ctl`,
  one call a process.
- **temper's own runs** read these pages with their host tool
  `read_docs`.

## Reading

- `temper help NAME` prints a page whole; `temper help --search
  WORDS...` finds the sections holding every word.
- `temper ctl help` prints the page `ctl`, then the groups;
  `temper ctl help GROUP VERB` prints one tool.
- Read what a call prints first: its acts, an error's remedy. A page
  explains what a call does not.

## The map

- temper help ctl: calls, keys, kept requests, waits, exit statuses.
- temper help deployment: the engine, its workers, its directories
  and its configuration.
- temper help party: a person or a service, its roles, signing in.
- temper help project: its repositories, its home, its members.
- temper help forge: GitHub or Forgejo, and how temper reaches one.
- temper help repository: a forge's repository adopted into a project.
- temper help account: an LLM provider's account, its sources, models.
- temper help task: a piece of work, its delegates, phases and holds.
- temper help chat: a conversation with an agent, closed by its party.
- temper help goal: a tracked task, its priority and its plan.
- temper help change: a change's way from a branch to its landing.
- temper help inbox: what waits on your party, then its updates.
- temper help proposal: an act that waits for one who may decide it.
- temper help settings: charters, templates, policies and rules,
  changed while the deployment runs.
- temper help revision: each settings decision, numbered, and the
  revision a task's runs read.
- temper help charter: how an agent's run works.
- temper help template: how a new task of one shape is filled.
- temper help policy: what a project's work may do and spend.
- temper help landing-rule: what a change must meet to merge.
- temper help budget: what work spends, and what happens at the end.
- temper help playground: the demo, on a fake forge and model.

## The walks

- temper help box: a deployment on one machine, from its
  configuration to a change landed, and its account kept at the stop.
- temper help github: GitHub through a GitHub App, an installation
  accepted, a repository adopted.
- temper help forgejo: a Forgejo temper does not run, connected as
  its site admin.
- temper help server: a long-lived deployment on a server, its
  workers on another host, and its redeploy.

## Placeholders

In a page's commands, each of these words stands for a value you
supply; a walk names any other in its `Before you start`.

- `DIR`: ctl's directory for one deployment and one party, private
  to you.
- `CONFIG`, `DATA`, `CACHE`: the deployment's configuration, data and
  cache directories.
- `FILE`: a file's path; `-` reads stdin where the tool allows it.
- `TEXT`: one argument of words, quoted for the shell; `-` reads
  stdin.
- `KEY`: a request's name, 1 to 64 lower-case letters, digits or
  hyphens.
- `TASK`, `PROJECT`, `FORGE`, `INSTALLATION`, `ACCOUNT`, `REVISION`,
  `PROVIDER`: numbers, as a read prints them.
- `PROPOSAL`: a proposal, as a read prints it: `t41/3`, task 41's
  third, or `p2/1`, party 2's first.
