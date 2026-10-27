# Tasks, goals and chats

Kind: guide

What temper's work is made of, tasks with their delegates, goals and
chats, and how you follow, answer, steer and stop them.

## Tasks

A task is a numbered piece of work. It has an executor, what was
asked, permission to act, funding and a required kind of result. Its
executor can be an agent, a procedure or a person. In the client,
**Task details** shows **What was asked** and the kind of result:
a report, change, verdict or approval.

You start a chat; an agent can create delegate tasks to carry out
parts of its work. Delegates form a tree. Follow **Its tasks** from
the parent to see live work and ended delegates. The page of ended
work is bounded: use the offered next controls when there is more.

These examples use `me` as your client directory and task 41 as the
work you want to follow. Replace the project and task numbers with
yours. The first command lists tracked work in project 1 by priority;
the second reads one task and its tree.

```sh
temper ctl me task list --project 1 --tracked --order priority
temper ctl me task show 41 --tree
```

The state tells you where work is: **waiting**, **due to run**,
**preparing a host**, **claimed by a host**, **executor active**,
**running**, **retrying**, **closing** or **held**. The running detail
identifies writing or a tool. Ended work distinguishes
**Done**, **Failed** and **Cancelled**.

A wait is more specific than a pause with no explanation. You may see
**waiting on task 42**, **waiting on its resources**,
**waiting on 2 delegates**, **waiting on an answer from you** or
**waiting on news**. A wait does not by itself mean the agent is
stuck: it may need another task, resources, delegates, your answer or
news before it can go on.

## Settings revision

Read a task to see the settings revision its runs use for their charter.
These examples use task 41 and the signed-in client directory `me`:

```sh
temper ctl me task show 41
temper ctl me task show 41 --json
```

For an agent task, the text includes a line after its instructions:

```text
settings: revision 7
```

When a live run was prepared under another revision, the line adds
`its run on 6`. When the newest revision differs, it adds `newest 9`.
Each suffix follows a middle dot separator. Between runs, the line
omits the run number and keeps the newest number when it differs.
An ended agent task retains its settings line. A procedure's or a
person's task prints no settings line. Read `temper help settings` for
held revisions and the current limits of `revision advance`.

With `--json`, the task object includes
`"settings":{"task":7,"run":6,"newest":9}`. The `run` member is null
when no different live run revision is supplied; `settings` is null
for a procedure's or a person's task.

## Goals

A goal is a tracked task with a numeric priority. Tracking is
optional, including for delegates. The client identifies a goal by
its priority, and **Plan** lets you follow its work. Lower numbers
order work that has not started first; changing priority does not
promise to interrupt work already running.

An agent can make a goal by creating a tracked delegate. You can also
accept a proposed goal in **Inbox**. You can read existing goals and
change their priority, even though there is no goal-creation command.

For task 41, already tracked, set priority 2 with a reason and a key
you choose for this request:

```sh
temper ctl me task amend 41 --priority 2 urgent --key priority-41
```

Changing only an already tracked task's priority does not need its
specification revision. Tracking an untracked task, untracking a goal
or changing other controls needs the specification revision shown
for the task. Read the task before making those changes: an old
revision is refused rather than applied to a different specification.

## Chats

Start a chat in a project to talk with temper. It uses that project's
template 1, with its charter (`temper help charters`), budget rule
and model choice; the command does not offer a template picker. The supplied chat template
is an untracked conversation that you end. It has no task deadline
and keeps its transcript.

```sh
temper ctl me chat start --project 1 hello --key first-chat
```

The creation outcome names the task; it is not the agent's answer.
Use the returned number in place of 41 to read the conversation or
send more words:

```sh
temper ctl me chat show 41
temper ctl me task message 41 investigate --key investigate-41
```

An ordinary reply leaves the chat open. A run can park, showing
**waiting on your words**, and a later message can start another run. A
park is not a failure, and an ordinary reply is not the task's result.
Follow the offered **Older** control to read earlier text when the
conversation window does not include it all.

You can send words while work runs. They reach the agent at a turn
boundary, so a message steers its next turn rather than changing the
turn already in progress. In the client, use **Message** and **Send**.
A closed chat appears under **Closed** with its transcript kept. It
cannot be reopened.

## Closing a chat

Choose **Close** when you want to end the conversation. The
confirmation says **Close chat now**, or **Close chat: allow 5 seconds
to finish, then cancel** when that is the current grace. Five seconds
is the default chat-template grace; read template 1 to see the
current value before relying on it:

```sh
temper ctl me settings show template 1 --json
```

Close settles what should happen to each live direct delegate, then
closes the chat with that grace. Delegates default to cancellation.
The confirmation says **Delegate 42 will be cancelled** or
**Delegate 42 will be moved to you**. It also says **A closed chat
can't be reopened.** Check those choices before confirming.

To take live delegate 42 into your care while closing chat 41:

```sh
temper ctl me chat close 41 --move 42 --reason finished --key close-41
```

Use the numbers of the chat and its live direct delegate. The move
choice keeps that delegate in your care instead of cancelling it as
part of closing the chat.

## Asking for a change

Ask the chat for the repository change you want. The chat asks temper
for the change, a delegate task makes and lands it, and the chat
follows that task and its result. You can follow the delegated work
from the chat rather than treating a conversational reply as the
landed change.

## What needs you

**Inbox** separates **Needs you** from **Updates**. Needs include
proposals, questions, escalations and person tasks; updates include
results and replies. The kind filter calls escalations **Held**.
Read the item to learn which action it asks of you.

A question needs **Answer**, which names both the task and the
particular question. An ordinary `task message` sends words but is a
separate operation. For question 2 on task 41:

```sh
temper ctl me task answer 41 2 proceed --key answer-41-2
```

A proposal asks someone with more authority to authorize a concrete
action. Review the proposed work and, for an authority widening or
amendment, the before and after authority. You can **Accept**, or
**Reject with reason**. To accept task 41's proposal 3:

```sh
temper ctl me proposal decide t41/3 accept --key accept-41-3
```

A hold has a reason and a revision. Read both before **Release task**;
you can also **Leave held** or pass an escalation up. Use the current
hold revision, here 2:

```sh
temper ctl me task release 41 --hold 2 --key release-41
```

An uncertain effect additionally needs your decision about repeating
it. Only for that case, `--again` chooses **Repeat the effect**. The
client also offers **Do not repeat the effect**; omitting `--again`
in ctl does not explicitly choose that option. Plain release is for
an ordinary hold. For budget-related holds, read `temper help budgets`
to understand the cause before releasing the task.

## Stop and cancel

**Stop** stops the current run and holds its task. **Cancel** cancels
the task's tree. Leaving the chat page only stops watching it; it is
neither of these actions.

```sh
temper ctl me task stop 41 --key stop-41
temper ctl me task cancel 41 --grace 5s finished --key cancel-41
```

These are separate choices: use the command for the action you want.
The cancellation example explicitly allows five seconds of grace.
For a chat, the default closing grace is five seconds; read the
current template as shown under Closing a chat.

## What it cannot do yet

There is no board for reordering goals and no ctl goal-creation
command. Agents can create tracked delegates, and you can read and
amend goals that exist.

A chat does not deliver a repository change itself: ask it for the
change and follow the delegated task. There is no guarantee that a
final summary arrives before Close cancels the run after its grace.

Read a closed chat in the client: `temper ctl me chat show 41` may not show
a chat that has ended.
