# Read a task's settings revision

Use a signed-in client directory to read a task. Replace `41` with the
number of the task you want to inspect:

```sh
temper ctl DIRECTORY task 41
temper ctl DIRECTORY task 41 --json
```

For an agent task, the text includes a line after its instructions:

```text
settings: revision 7
```

This is the settings revision recorded by the task. When its live run
was prepared under another revision, the line includes that run's
revision. When the newest revision differs from the task's, it includes
the newest number too:

```text
settings: revision 7 · its run on 6 · newest 9
```

Between runs, the same task would print `settings: revision 7 · newest 9`.
An ended agent task retains its settings line. A procedure's or a person's
task prints no settings line.

With `--json`, the task object includes
`"settings":{"task":7,"run":6,"newest":9}`. The `run` member is null
when no different live run revision is supplied; `settings` is null
for a procedure's or a person's task.
