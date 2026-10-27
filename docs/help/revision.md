# Revision

Kind: concept

A numbered record of one settings decision, the history of such records, and the revision each agent task's runs read their charter from.

## What it is

A revision is one settings decision, numbered from 1. It records the
revision it was made against, when it was made, who decided it, the
proposal it came from if any, its running choice, and each item it
changed: a set item with its whole value. The first start makes
revision 1.

A revision is not a task's specification revision or a hold's revision:
a settings change names a settings revision.

The history is the revisions, newest first, read in bounded pages. The
store keeps a bounded number of them: past it, the oldest are forgotten.

## How it changes

Every accepted change makes the next revision, and so does a start with
a new default for a setting that follows its default. A revision never
changes once made.

An agent task records the revision it was made in, and each of its runs
reads its charter from that revision. A delegate records the newest
revision, whatever its parent's. A revision a live task reads from is
held: kept, with its charters, while a task holds it.

A task moves to the newest revision in three ways:

- an owner moves the live agent tasks of a held revision, each at its
  next run;
- a change accepted with its running choice `moves` moves them with it,
  and one with `stays`, the default, keeps them;
- a conversation moves by itself at its next run.

A task moves only when the newest revision offers its charter with the
models it chose; else it stays on its own. A held revision is released
when its last task ends or moves.

## What temper guarantees

- A run keeps the instructions it started with, whatever changes while
  it runs.
- A task's later runs read the charter of the task's revision until it
  moves: its instructions, its models and the hosts that may run it.
- A held revision's charters stay readable while it is held.

## Acting on it

- `temper ctl`: `revision list` reads the history, a page at a time;
  `revision show` reads one revision's record; `settings show` reads a
  charter as a held revision has it; `revision advance` moves a held
  revision's tasks.
- `task show` prints an agent task's `settings:` line: its revision,
  its live run's when that differs, and the newest when that differs.
- A run whose charter and authority have the settings family reads
  revisions with the host tool `settings`.

## Limits

- A revision records what it changed, not every setting's value:
  `revision show` prints a set item's whole value, and `settings show`
  reads a past value of a held revision's charter alone.
- temper keeps a bounded number of held revisions and of charter
  versions. A change that would hold more is refused as full, naming the
  oldest held revision's tasks: move them to go on.
- A forgotten revision cannot be read. The refusal names the oldest
  revision kept.
