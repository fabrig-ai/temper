# landing

Kind: concept

A landing is how a project that lands by turn lands a change: its candidate is proved by each gate's receipt, then its main is fast-forwarded to it.

## What it is

- **A project lands by queue or by turn.** By queue, the default,
  each change is checked by CI on its pull request and merged in its
  landing branch's queue. By turn, a change lands through a landing,
  and this page is that way.
- **A landing** is a task of the project's, made when a change of a
  project that lands by turn becomes ready, with that change as its
  member. The deployment asks for it on the project's behalf, from the
  project's spend. The change waits on it.
- **A candidate** is what lands: the change's head when it holds its
  main's tip; otherwise the head the forge's rebase of the pull
  request onto main makes. The rebase rewrites the change's branch,
  and the pull request shows each candidate.
- **A gate** is one of a repository's test workflows, named in the
  project's configuration with the paths it ignores and whether it
  blocks. Its workflow runs only when temper dispatches it: it names
  `workflow_dispatch` alone among its triggers.
- **A key** is what a gate's result depends on: the candidate's files
  without the paths the gate ignores, and the gate's definition. Two
  candidates whose files differ only in ignored paths have one key.
- **A receipt** is a gate's result under a key: passed or failed, with
  its run. Receipts are kept per project, and a key that has one never
  runs again.
- **The turn** is the project's: one landing at a time holds it, for
  the seconds its merge takes.

## How it changes

A landing goes through these phases, each shown by `landing show`:

- **waiting:** its change is not ready: it is being repaired, or a
  conflict is being resolved.
- **preparing:** its candidate is being made: main's tip is read
  afresh, and the change asks the forge for the rebase when its head
  lacks it.
- **proving:** each gate's key is computed from the forge's trees. A
  key with a receipt passes or fails at once. A key with none
  dispatches the gate's workflow on the change's branch, with the
  inputs `temper_commit` and `temper_key`, and its run, followed to its
  end, makes the receipt.
- **proved:** every blocking gate passed at the candidate; it waits
  for the turn. The first proved takes it first.
- **in its turn:** main is read afresh. If it moved since the candidate
  was prepared, the landing prepares again; otherwise temper merges the
  pull request fast-forward only, at exactly the candidate.
- **landed:** main's tip is the candidate, and the change is landed.
- **held:** a gate's runs failed on their runner past the project's
  retries. It waits for a person.

A gate that fails in its workflow's own steps makes a failed receipt:
its change is repaired with the failing step's output, its new head
makes the change ready again, and the landing prepares again.

## What temper guarantees

- main moves only to a candidate every blocking gate passed at: the
  commit main holds is the commit the gates ran at, never a merge made
  after them.
- One key runs once at a time: two landings reaching one key wait on
  one run.
- A receipt comes only from a run temper dispatched for that key at
  that candidate. A status or a run anyone else made is never one.
- A runner's fault is retried, never repaired: a job that failed in
  its set-up or completion step alone, a run cancelled by no one of
  temper's, a run never found.
- Each gate shows on the candidate as the commit status
  `temper/gate/NAME`: pending with its key's first 16 hexadecimal
  digits once dispatched, then success or failure with its run as the
  link. A fast-forward makes main's commit the candidate, so main shows
  the statuses that proved it.

## Acting on it

- `landing show` reads a landing: its change, the candidate on its base,
  each gate's key with its receipt or run, and the turn. `landing list`
  lists a project's landings, the turn's holder first.
- `change show` reads the change's own way: its pull request and gates.
- A held landing is released or cancelled by the project's
  maintainers, as any held task is.
- On the forge, read the runs, logs and pull requests at the links ctl
  prints. What temper owns there changes only through temper.

## Limits

- A project lands by turn on Forgejo alone: GitHub cannot merge fast
  forward only, and a start whose configuration asks for the turn for
  a project with a repository on GitHub is refused, naming it.
- A landing has one member: the change that became ready.
- How a project lands, and each repository's gates, are set in the
  configuration document's `landing` member, read at the start.
- A gate's name is at most 32 bytes, a word a branch name could hold;
  its workflow's file name at most 255 bytes; its ignored paths each a
  path prefix of whole segments (`docs`, `crates/x/README.md`), never a
  pattern.
- The deployment's limits bound the live landings, the gates of a
  repository, the ignored paths of a gate and the receipts a gate
  keeps. A change whose landing would pass one waits, ready, until there is
  room.
- A runner's faults are retried twice per key, then the landing is
  held.
- A gate keeps its newest receipts, and always the receipt of its
  main's tip.
