# tier

Kind: concept

A tier is a slow check a repository runs on its main after landing, one run at a time at the newest tip; a change lives until every tier of its repository has cleared its landing.

## What it is

- **A tier** is one of a repository's workflows, too slow or too broad
  for a gate: a browser suite, an exhaustive profile. It is named in
  the configuration's `landing` member beside the repository's gates,
  with its workflow's file, the paths it ignores, and its report: the
  artifact its run uploads and that file's format, `junit` or `lines`.
  Its workflow runs only when temper dispatches it: it names
  `workflow_dispatch` alone among its triggers.
- **A run** is the workflow dispatched by temper on a branch of its
  own, `temper/tier/NAME.N`, made at the commit it checks, with the
  inputs `temper_commit` and `temper_key`. N counts the tier's runs.
- **A landing** is each move of the repository's main: a change temper
  merged, by queue or by turn, or a push temper did not make.
- **Its range** is the landings a run covers: those since the tier's
  last judged tip, up to the run's.
- **A key** is what a run's result depends on, as a gate's is (temper
  help landing): the files at its commit without the paths the tier
  ignores, and the tier's definition. A run's tip whose key has a
  receipt is judged by it, with no run.
- **A run is a task** of the project's, made when the run starts and
  asked for by the deployment on the project's behalf, from the
  project's spend: it owns the run's branch, its dispatch and its
  status, and ends with the verdict.

## How it changes

- **The baseline:** once a tier is configured and its repository
  adopted, it runs at its main's tip first.
- **One run at a time:** a tier with no run in flight starts one at
  the newest tip when main moves, once each landing's key is read from
  the forge's trees; one in flight is followed to its
  end, and the next starts at the newest tip then, covering every
  landing since.
- **Green:** every landing of the run's range is cleared for that
  tier. A landing that changes only paths every tier ignores keeps the
  judged tip's key, so it is cleared on that tip's receipt at once. A landing is cleared once every tier of its repository has
  cleared it.
- **A fault:** a run lost with its runner, one whose set-up failed, a
  run never found, is retried, its failed jobs first, then dispatched
  again, up to the project's `tier_retries` (2); past them the run's
  task is held for a person, its landings still verifying.

## What temper guarantees

- **A change lives to cleared:** in a repository with tiers, a landed
  change verifies: its task lives on, its branch kept, until every tier
  has cleared its landing; then it gives its result. In a repository
  with no tier, it is cleared as it lands.
- **A run never runs on main,** which may move under it: on its own
  branch, at the commit it checks.
- **Only temper's runs count:** a run anyone else triggered, or one at
  another commit, is never a verdict.
- **The forge shows it:** each verdict as the commit status
  `temper/verdict/NAME` on the run's tip, success for green, its link
  the run.

## Acting on it

- **The configuration:** each repository's `tiers` in the `landing`
  member's `gates` list, and the project's `tier_retries` beside its
  `by`, read at the start (temper help configure).
- **ctl:** `change show` prints a verifying change's step as `landed`,
  as a cleared one's; `task show` tells them apart, the verifying
  change's task live, and shows a tier run's task, live, held or done,
  its result naming the run's verdict.
- **`landing list --project PROJECT --newer-than TIME`** lists every
  landing temper saw on the project's repositories after TIME, newest
  first, by
  queue, by turn or from outside temper, with each tier's verdict
  (temper help landing).
- **A held run** is released by the project's maintainers, as any held
  task is.
- **The forge** holds each run, its jobs and their logs.

## Limits

- A failed run is a fault for now: it is retried, then held. Reading
  its failing tests, rerunning them alone, and blaming one landing come
  later.
- A run's branch stays on the forge once the run is judged.
- The deployment's limits bound the tiers of a repository (2 in the
  small profile) and a tier's name, at most 32 bytes of letters,
  digits, `-` and `_`.
