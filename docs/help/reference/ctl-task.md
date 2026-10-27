# temper ctl task

Kind: reference

Tasks: read one and its parts, and act on it.

## task list

```
task list --project PROJECT [--label chat|goal] [--tracked] [--mine] [--ended] [--order activity|priority] [--after CURSOR] [--parent TASK] [--since DIGEST] [--until CONDITION] [--timeout DURATION]
  --project PROJECT: a project's name or number; required. The project
  --label chat|goal: one of chat or goal; optional. Only chats, or only goals
  --tracked: a switch. Only tracked tasks
  --mine: a switch. Only the tasks your party asked for
  --ended: a switch. Ended tasks, not live ones
  --order activity|priority: one of activity or priority; optional. By last activity or by priority; by number when absent
  --after CURSOR: a cursor, as a read prints it; optional. The cursor a page printed, to read on
  --parent TASK: a number; optional. Only this task's delegates
  --since DIGEST: a digest, as a read prints it; optional. Wait until it differs from the read that printed DIGEST
  --until CONDITION: a condition; optional. Wait until it holds: changed
  --timeout DURATION: a duration: seconds, or a number with s, m or h, at most 3600 seconds; optional. How long a wait lasts; 300 seconds when absent, at most 3600
Read a project's tasks, a line each
Prints: A line per task, its number, kind and phase, then `read on with: task list ... --after CURSOR` or `no more`; the first page of live tasks ends with a digest
--json: the record tasks.
Key: none.
A page holds at most 32 tasks. Live and ended tasks are read apart.
Only the first page of live tasks has a digest to wait on.
```

## task show

```
task show TASK [--tree] [--activity COUNT] [--run] [--since DIGEST] [--until CONDITION] [--timeout DURATION]
  TASK (task): a number; required. The task's number
  --tree: a switch. Add its live tree of delegates
  --activity COUNT: a count from 1 to 32; optional. Add its latest COUNT lines of activity
  --run: a switch. Add its live run
  --since DIGEST: a digest, as a read prints it; optional. Wait until it differs from the read that printed DIGEST
  --until CONDITION: a condition; optional. Wait until it holds: ended, waiting-on-me, held, replied, phase:PHASE, or changed
  --timeout DURATION: a duration: seconds, or a number with s, m or h, at most 3600 seconds; optional. How long a wait lasts; 300 seconds when absent, at most 3600
Read a task: its state, what it waits on, and the acts you may take
Prints: `task N`, its kind, phase, project and tracking, its fields, `acts:` and a digest; each part asked for after it, with its own digest
--json: the record task.
Key: none.
`acts:` lists the calls the engine offers your party, ready to copy.
An act marked `(no tool)` has no tool here yet; one followed by a
colon and a reason is refused as it stands; one marked `proposed
to` asks another party first. A change task adds `next:` and the
call that reads its change. An ended task prints its result.
```

## task activity

```
task activity TASK [--before CURSOR] [--limit COUNT]
  TASK (task): a number; required. The task's number
  --before CURSOR: a cursor, as a read prints it; optional. The cursor a page printed, to read further back
  --limit COUNT: a count from 1 to 32; optional. At most COUNT lines
Read a task's activity from its end
Prints: A line per event, oldest first, then `read on with: task activity TASK --before CURSOR` or `no more`
--json: the record activity.
Key: none.
A turn prints as its size and `read with: task turn TASK TURN`.
With `--limit`, the page keeps its last COUNT lines, and `read on
with:` repeats the read without the limit.
```

## task turn

```
task turn TASK TURN [--part PART] [--from OFFSET]
  TASK (task): a number; required. The task's number
  TURN (turn): an attempt and a turn, A.T; required. The attempt and turn, as A.T, as task activity prints them
  --part PART: a number; optional. Only this part
  --from OFFSET: a number; optional. From this byte of the part
Read one turn of a task's run, part by part
Prints: A turn's header, then its words, calls and results in order; a cut part ends with `N bytes left; read on with: task turn TASK TURN --part PART --from OFFSET`
--json: the record turn.
Key: none.
A part prints at most 16384 bytes at a time. A turn past ctl's
bound prints its size and `past the limit; report it`, and one it
cannot decode `unreadable; report it`, both with exit 0: report
either as a finding.
```

## task run

```
task run TASK [--since DIGEST] [--until CONDITION] [--timeout DURATION]
  TASK (task): a number; required. The task's number
  --since DIGEST: a digest, as a read prints it; optional. Wait until it differs from the read that printed DIGEST
  --until CONDITION: a condition; optional. Wait until it holds: ended, or changed
  --timeout DURATION: a duration: seconds, or a number with s, m or h, at most 3600 seconds; optional. How long a wait lasts; 300 seconds when absent, at most 3600
Read a task's live run: its attempt, its call in flight, its words so far
Prints: `run: A` or `run: none`, the call in flight, the words so far, and `digest: run`
--json: the record run.
Key: none.
With `--until ended`, it returns when the run ends.
```

## task message

```
task message TASK TEXT --key KEY
  TASK (task): a number; required. The task's number
  TEXT (text): text, at most 1048576 bytes, or - to read stdin; required. The words
Send words to a task
Prints: `done: task N, message`
--json: the record message.
Key: --key KEY, required.
```

## task answer

```
task answer TASK QUESTION TEXT [--key KEY]
  TASK (task): a number; required. The task's number
  QUESTION (question): a number; required. The question's number, as the inbox prints it
  TEXT (text): text, at most 1048576 bytes, or - to read stdin; required. The answer
Answer a task's question
Prints: `done: task N, answer`
--json: the record answer.
Key: --key KEY, optional.
```

## task release

```
task release TASK --hold REV [--again] [--key KEY]
  TASK (task): a number; required. The task's number
  --hold REV: a number; required. The hold's revision, as the task prints it
  --again: a switch. Run the task again
Lift a task's hold
Prints: `done: task N, release`
--json: the record release.
Key: --key KEY, optional.
```

## task stop

```
task stop TASK [--key KEY]
  TASK (task): a number; required. The task's number
Stop a task's run and hold it
Prints: `done: task N, stop`
--json: the record stop.
Key: --key KEY, optional.
The task stays held until task release lifts the hold or task
cancel ends it.
```

## task cancel

```
task cancel TASK [--grace DURATION] REASON [--key KEY]
  TASK (task): a number; required. The task's number
  --grace DURATION: a duration: seconds, or a number with s, m or h; optional. How long its run may finish first; the engine's default when absent
  REASON (reason): text, at most 1048576 bytes, or - to read stdin; required. Why
Cancel a task and its tree
Prints: `done: task N, cancel`
--json: the record cancel.
Key: --key KEY, optional.
```

## task tree

```
task tree TASK [--since DIGEST] [--until CONDITION] [--timeout DURATION]
  TASK (task): a number; required. The task's number
  --since DIGEST: a digest, as a read prints it; optional. Wait until it differs from the read that printed DIGEST
  --until CONDITION: a condition; optional. Wait until it holds: ended, or changed
  --timeout DURATION: a duration: seconds, or a number with s, m or h, at most 3600 seconds; optional. How long a wait lasts; 300 seconds when absent, at most 3600
Read a task's live tree: the task and each delegate its window holds, a line each
Prints: `tree:`, the task, then a line per delegate; `cut:` and the call that reads the rest when it folds some; and `digest: tree`
--json: the record tree.
Key: none.
A large tree folds what does not fit: read the rest with task list
--parent TASK, a page at a time.
```

## task amend

```
task amend TASK [--rev REV] [--priority PRIORITY | --untracked] [--deadline TIME | --no-deadline] [--instructions FILE] [--spec FILE] [--dependency TASK]... [--no-dependencies] [--model MODEL [--effort EFFORT]] REASON [--key KEY]
  TASK (task): a number; required. The task's number
  --rev REV: a number; optional. The spec revision the task prints; not needed for its priority alone
  --priority PRIORITY: a number; optional. Track it at this priority
  --untracked: a switch. Stop tracking it
  --deadline TIME: text, at most 1048576 bytes; optional. Its new deadline, as 2026-10-24T17:00Z
  --no-deadline: a switch. Remove its deadline
  --instructions FILE: a file, at most 1048576 bytes; optional. A file of its whole instructions, or -; an empty file removes them
  --spec FILE: a file, at most 1048576 bytes; optional. A file of its spec's words, or -
  --dependency TASK: a number; repeated, at most 32. A task it waits on; repeated, the whole list
  --no-dependencies: a switch. Remove every dependency
  --model MODEL: text, at most 1048576 bytes, or - to read stdin; optional. A model the task may use, as models lists it
  --effort EFFORT: text, at most 1048576 bytes, or - to read stdin; optional. An effort that model offers
  REASON (reason): text, at most 1048576 bytes, or - to read stdin; required. Why
Amend a task: each control you name is replaced, the rest kept
Prints: `done: task N, amend`, or `proposed: PROPOSAL, to RECIPIENT` when another party must agree
--json: the record amend.
Key: --key KEY, optional.
A task whose spec moved since your read fails with `read again:
task show TASK`. Each file is read before anything is sent, one
final newline trimmed, at most 32768 bytes of UTF-8. An amendment
that changes nothing fails (exit 1).
```

## task decide

```
task decide TASK --hold REV release|leave|pass-up [REASON] [--key KEY]
  TASK (task): a number; required. The held task's number
  --hold REV: a number; required. The hold's revision, as the task prints it
  release|leave|pass-up (outcome): one of release, leave or pass-up; required. The decision
  REASON (reason): text, at most 1048576 bytes, or - to read stdin; optional. Why; sent with leave alone
Decide the escalation of a held task
Prints: `done: decide`, or `decided first:`, by whom, how and when, if it was decided already
--json: the record decide.
Key: --key KEY, optional.
A decision made already, by your party or another, answers
`decided first:` with exit 0 and changes nothing.
```
