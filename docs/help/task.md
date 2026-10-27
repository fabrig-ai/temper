# Tasks

Kind: concept

A task is one numbered piece of work in a project, done by an agent, a procedure or a person, within an authority, toward a result of a declared kind; tasks make trees of delegates.

## What it is

- **A task** has what was asked (its spec, words and inputs), its
  executor (an agent under a charter, a procedure, a person, or a
  role), its authority (what it may do, delegate and spend, and by
  when), and its result contract: a report, a change, a verdict or an
  approval. `task show` prints them; its number is its name.
- **Delegates:** an agent's run makes child tasks of its own; each
  is a delegate of its parent, and together they make a tree. A
  delegate's result reaches its parent, which its run hears as its
  rules for waking say. A change task is a delegate run by the forge,
  whose result is a change landed (temper help box); it starts with
  half of what its parent has left to spend. A change asks no one,
  and neither do the agents it delegates: a question or words sent to
  it by a task are answered that a change takes none, naming its
  requester; a party's words to it are taken unanswered.
  A change whose authority no longer covers its next step (its tasks all
  made, its depth used, too little left to spend) holds, saying what it
  lacks; released with nothing more, it holds again, and its way out is
  to cancel it, or ask for the change afresh.
- **Its parts,** each read on its own: its tree (its live
  delegates), its activity (what happened, a line each, newest last),
  its turns (each step of a run: the model's words, its calls and
  their results), and its run (the attempt in progress).
- **Its settings revision:** an agent task records the settings
  revision its runs use; `task show` prints it as `settings: revision R`,
  with the run's own and the newest where they differ.

## How it changes

- **Live phases,** as `task show` prints them, then as a wait names them:
  Waiting (`waiting`), Waiting for news (`idle`), Due to run (`due`),
  Preparing a host (`preparing`), Claimed by a host (`claimed`),
  Running (`running`), Retrying (`backing-off`), Executor active
  (`active`), Closing (`closing`), Held (`held`).
- **Ended:** Completed, Failed, Cancelled, or Ended; a wait names
  them all `ended`. An ended task keeps its result, and `task show` prints
  it.
- **What it waits on** prints after `waiting on:`: `Kept by its
  dependencies`, `Waiting for delegates`, `Waiting for news`, `Nothing
  awaited`, or a party and what it must answer: `you: question 3 of
  task 44 · 1 waiting`.
- **Held:** a held task runs no more until someone decides. Its hold
  has a reason (out of tries, stopped, its deadline passed, an
  insufficient budget, an effect whose outcome is uncertain, among
  others) and a revision of its own; a hold a party placed is theirs to
  release (temper help hold). Releasing it runs it again;
  leaving it held, or passing it up to the next holder of authority,
  are the other decisions. Only an uncertain effect asks whether to
  repeat it. `task show` prints `held:` and its reason, then `way
  out:`, what to mend and the call that releases it: `task release
  TASK --hold REV`.
- **Stopped:** stopping places a hold of yours over the task and every
  task below it, those made later included, and ends their runs;
  releasing the stopped task runs them all again (temper help hold).
- **Cancelled:** cancelling ends the task and its tree: at once, or
  after a grace the canceller gives, in which its run may finish. A
  task that has ended refuses a cancel, saying how it ended; the same
  cancel again, under its key, gets its first answer.
- **Amended:** an amendment replaces the controls it names (priority
  and tracking, deadline, instructions, spec, dependencies, model) and
  keeps the rest; all but a tracked task's priority name the spec
  revision the amender saw. Words a read cut short are replaced whole
  at that revision.

## What temper guarantees

- **One result.** A task ends once, with one result of its contract's
  kind, or failed or cancelled; a delegate's result reaches its
  parent.
- **Words between turns:** a message reaches the agent at a turn's
  boundary and steers its next turn, never the one in progress.
- **Decisions on what was seen:** a release names the hold's
  revision and an amendment the spec's; a moved one is refused, and a
  read shows the new one.
- **A task's runs keep their settings revision** until it is moved to
  the newest (temper help settings).
- **Spend counts up the tree:** a delegate's budget is reserved from
  its parent's (temper help budget).

## Acting on it

- **Reading:** `task list` lists a project's tasks; `task show` reads
  one, and its tree, activity or run beside it; `task tree`, `task
  activity`, `task turn` and `task run` read its parts; `task show`,
  `task tree` and `task run` wait with `--until` or `--since`.
- **Acting:** the acts `task show` lists under `acts:`, among them
  `task message`, `task answer`, `task amend`, `task stop`, `task
  cancel`, `task release`, `hold lift`, `task decide` and `proposal
  decide`; an act marked `(no tool)` has no ctl tool yet.
- **temper's runs** act on their delegates with the host tools
  `delegate`, `change`, `goal`, `amend`, `message`, `cancel`, `release`,
  `subscribe`, `introduce`, `decide_escalation` and `propose`.
  `message` is offered only to a run whose authority holds the message
  family: a change's coder, repairer and reviewer are not offered it.

## Limits

- temper does not let ctl take a task, hand it back, move it to
  another party, reorder it or mark it seen: those acts print `(no
  tool)`.
- temper does not interrupt a turn in progress for a message, an
  amendment or a priority.
- A task's deadline is checked when a run starts, not while it runs
  (temper help budget).
- `task activity` and `task list` read 32 lines a page; a turn's part prints
  16384 bytes at a time.
