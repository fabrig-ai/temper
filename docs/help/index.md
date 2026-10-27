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

- temper help ctl: the command line through which an agent works in a
  deployment for a person: calls, keys, kept requests, waits, exit
  statuses.
- temper help deployment: one temper: its engine and workers, a box or
  a server, its three directories, its configuration, and a restart.
- temper help party: who acts in a deployment, a person or a service:
  its roles, its pool, signing in, and secrets read from files.
- temper help project: where work happens: its repositories and its
  home, its members, its policy and its period.
- temper help forge: GitHub or Forgejo, connected, managed or owned:
  GitHub's App and installations, and what temper writes there.
- temper help repository: a forge's repository adopted into a project:
  its role, temper's branch prefix, CI or checks, and the home.
- temper help account: how a deployment pays an LLM provider: accounts
  and their sources, a move between deployments, and the models.
- temper help task: a numbered piece of work, its delegates, phases,
  holds and parts.
- temper help hold: what holds a task, who may lift a hold a party
  placed, and how its release is proposed to its owner.
- temper help chat: a conversation with an agent, parked between
  messages, closed by its party.
- temper help goal: a tracked task, its priority and its plan.
- temper help change: a change's way to landing: its pull request, CI,
  gates, queue, repairs, holds and merge.
- temper help inbox: what waits on your party, questions, proposals,
  escalations and person tasks, then its updates.
- temper help proposal: an act beyond its proposer's authority, which
  waits for one who may decide it; a change of settings among them.
- temper help settings: the charters, templates, policies and rules
  a deployment's owners change while it runs, one checked decision at
  a time.
- temper help revision: each settings decision as a numbered record,
  their history, and the revision a task's runs read their charter
  from.
- temper help charter: how an agent's run works: its instructions,
  models, workspace tools, hosts and limits.
- temper help template: how a new task of one shape is filled.
- temper help policy: what work in a project may do and spend: the
  policies, roles and rules, and the requirements on effects.
- temper help landing-rule: what a change must meet before temper
  merges it.
- temper help budget: what work spends, where it comes from, and what
  happens when it runs out.
- temper help playground: the demo: a fake forge and a scripted model,
  started with one call.

## The walks

- temper help box: a deployment on one machine, from its
  configuration to a change landed, and its account kept at the stop.
- temper help github: GitHub through a GitHub App, an installation
  accepted, a repository adopted.
- temper help forgejo: a Forgejo temper does not run, connected as
  its site admin.
- temper help server: a long-lived deployment on a server, its
  workers on another host, and its redeploy.
- temper help configure: the settings changed while the deployment
  runs: applied or proposed, running work moved, a value restored.

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
