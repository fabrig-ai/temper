# A weekly budget

Kind: recipe

A project spends at most AMOUNT units a week, every week.

## Goal

Give project PROJECT at most AMOUNT units of spend a week, renewed each
week from the moment its first period began.

## Settings

temper renews every project's period each week; the week is not a
setting. What a week may spend is the project's period spend, within
the rules' (`temper ctl DIR settings show rules`):

- **An existing project:** `temper ctl DIR project fund PROJECT AMOUNT`,
  then, for each role whose people should have more,
  `temper ctl DIR project fund PROJECT AMOUNT --role ROLE`. The open week
  takes it at once, and every later week starts at it.
- **Projects made from now on:** policy 1. Write FILE:
  `{"items": [{"setting": {"kind": "policy", "number": 1}, "set": VALUE}]}`,
  VALUE the `value` member that `temper ctl DIR settings show policy 1
  --json` printed, whole, with its `period_spend` and each role's set to
  AMOUNT. Apply it with
  `temper ctl DIR settings apply FILE --against REVISION`, REVISION the
  newest that `settings show` printed.

An AMOUNT past the rules' period spend is refused, exit 2, naming it.

## Enforced and followed

- **Enforced:** nothing in the project is funded past what the week has
  left. Each week starts full, at the period spend its policy holds when
  it opens; what an earlier week reserved stays its own and goes back to
  it. A person's chats are funded from their pool, at most their role's
  period spend a week.
- **Enforced:** a deployment stopped past a week's end opens one week
  when it starts again, from then, not one per week missed.
- **Followed:** nothing; agents need do nothing for the cap to hold.
- **Not:** an unspent week is not carried over.

## Checking it took

`temper ctl DIR project show PROJECT` prints the open week, its next
start a week after it began:

```
period 1, from 2026-10-27T06:00Z, next start 2026-11-03T06:00Z: budget AMOUNT, reserved 0, spent 0, left AMOUNT
```

`from` is when the week began and `next start` a week later, both UTC
to the minute. After the next start, `project show` prints `period 2`,
its budget the period spend.
