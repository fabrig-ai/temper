# temper ctl hold

Kind: reference

Placed holds: lift one you may lift.

## hold lift

```
hold lift HOLD --rev REV [--with-tasks] [--key KEY]
  HOLD (hold): a number; required. Hold number, as task show prints it
  --rev REV: a number; required. Hold revision, as task show prints it
  --with-tasks: a switch. Also release every task it holds that nothing else holds
Lift a placed hold you may lift
Prints: `done: hold N lifted, COUNT released`
--json: the record lift.
Key: --key KEY, optional.
Only the hold's owner, or a party whose role ranks above theirs,
lifts it. Lifted alone, the tasks it holds stay held, each
released with task release; with --with-tasks, every task it
holds that nothing else holds runs again. A lift moves the hold's
revision.
```
