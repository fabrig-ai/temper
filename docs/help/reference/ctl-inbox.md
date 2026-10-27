# temper ctl inbox

Kind: reference

Read what waits on your party, then its updates.

## inbox

```
inbox [--project PROJECT] [--kind questions|proposals|escalations|person-tasks|results|replies] [--root TASK] [--after CURSOR] [--since DIGEST] [--until CONDITION] [--timeout DURATION]
  --project PROJECT: a project's name or number; optional. Only this project
  --kind questions|proposals|escalations|person-tasks|results|replies: one of questions, proposals, escalations, person-tasks, results or replies; optional. Only this group
  --root TASK: a number; optional. Only what waits under this goal, by its task number
  --after CURSOR: a cursor, as a read prints it; optional. The cursor a page printed, to read on
  --since DIGEST: a digest, as a read prints it; optional. Wait until it differs from the read that printed DIGEST
  --until CONDITION: a condition; optional. Wait until it holds: waiting, updates, or changed
  --timeout DURATION: a duration: seconds, or a number with s, m or h, at most 3600 seconds; optional. How long a wait lasts; 300 seconds when absent, at most 3600
Read what waits on your party, then its updates
Prints: A line per item, its task and what waits, then `read on with: inbox ... --after CURSOR` or `no more`, and, unfiltered, `digest: inbox`
--json: the record inbox.
Key: none.
Questions, proposals, escalations and person tasks come first;
updates, results and replies, follow once those are read whole.
A page holds at most 32 lines. A filtered or continued read has
no digest: wait on the inbox unfiltered.
```
