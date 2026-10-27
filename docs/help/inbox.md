# The inbox

Kind: concept

A party's inbox is what waits on it, questions, proposals, escalations and person tasks, then its updates, the results and replies it has not read; one read lists both, needs first.

## What it is

- **Needs** wait for your party to act, a line each:
  - a question a task asked you, `TASK · question N`, its task and the
    question's number;
  - a proposal waiting for your decision, `PROPOSAL · proposal`, such
    as `t41/3 · proposal` (temper help proposal);
  - an escalation, a held task waiting for you to decide its hold,
    `TASK · escalation`;
  - a person task, a task whose executor is you, `TASK · person task`.
- **Updates** are news your party has not read: a result,
  `TASK · result`, and a reply in a conversation, `TASK · task`.
- **Its order:** every need, then the updates, in pages of at most 32
  lines; a page ends with the call that reads on, or `no more`.
- **Its counts:** `me` prints how many of each need wait on you, and
  how many updates.

## How it changes

- **A need leaves** once it is met: a question once answered, a
  proposal once decided or withdrawn, an escalation once decided, a
  person task once its person no longer has it.
- **An update's unread state** is the engine's: reading the inbox
  with ctl does not change it.

## What temper guarantees

- **Needs first:** no update is listed before every need has been.
- **One wait for all of it:** the inbox read whole prints a digest,
  and a wait on the inbox covers needs and updates alike (temper help
  ctl). `me` waits on needs alone: an unread update does not end a
  wait of `me` for `waiting`.
- **Each need is one subject's:** its line names the task or proposal
  whose read lists the acts that meet it.
- **Every question is answerable:** a question's words past what a read
  holds are cut at a character and marked as cut; read them whole in
  the asking task's activity (`task activity TASK`), and answer it with
  `task answer` as any other.

## Acting on it

- **ctl:** `inbox` lists it, whole or narrowed to one project, one kind
  or one goal's tree; `me` counts it. Act on each line through its
  subject: `task show` on a task lists its acts, among them `task
  answer` for a question and `task decide` for an escalation; `proposal
  show` and `proposal decide` for a proposal.
- **temper's runs** have no inbox: what waits on a run reaches it at
  its turns, as its rules for waking say (temper help task).

## Limits

- temper does not let ctl mark an update seen: `inbox seen` is no
  tool, and a seen act prints `(no tool)`.
- A narrowed or continued read prints no digest: wait on the inbox
  read whole.
- A reply prints as its task, `TASK · task`, without the message it
  holds: read it with `chat show` or `task show`.
