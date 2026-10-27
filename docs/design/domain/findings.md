# Findings and QA after landing

Provisional, 2026-10-27. Designed, not built. Something found that is
not its finder's to fix now (a QA pass's finding, a flake, a red whose
goal has ended, a review's follow-up) is a finding: a labelled issue in
the project's home, whose labels temper reads back, so that it can be
triaged on the forge or in temper. And QA after landing: landings mark
what a user can now do, a chooser picks which marks to test, and one
tester at a time tries them on the newest cleared main. It replaces
`forge.md`, 12's findings, kept in the store and never read back. What
is open is section 10.

## 1. In one page

- **A finding is a forge issue,** in the project's home, labelled
  `finding`, its kind (`qa`, `flake`, `red` or `review`), its severity,
  its area and its state (`new`, `routed`, `backlog` or `dropped`). Its
  body carries its reproduction, its commit and its signature: the
  failing test ids and the failure's first line, where it has one.
- **Labels are read back** (section 3): a person may triage on the
  forge, by its labels, or in temper; either way the finding's state is
  its labels. A narrow exception to `forge.md` 12's "written and never
  read back", for findings alone.
- **temper keeps the mechanics** (section 4): filing, matching by
  signature (a flake seen again is counted, not filed twice), routing,
  closing. The forge keeps the record.
- **A flake at a gate** (section 5): a gate's failure whose every
  failing test matches an open flake finding has its failed jobs rerun
  once rather than repaired.
- **Triage** (section 6): the project's triager, a task (often the
  person's chat) or a party, is woken by new findings; it routes each
  to a goal, to a new goal, to the backlog, or drops it.
- **Closed when its fix clears** (section 7): a landing whose change
  says `Fixes #n` closes finding n once the landing is cleared, not
  when it lands. A finding a person closes on the forge is closed in
  temper.
- **QA after landing** (section 8): marks (`QA:` trailers, a `qa`
  label), a chooser, one pass at a time per project on the newest
  cleared main, a tester with no forge effects whose findings are filed.

## 2. What a finding is

- **Its issue,** in the project's home (jig's `domain/core.md`, 6.1),
  created by temper (skein's `Write::CreateIssue` with its labels),
  keyed by its purpose (section 4), its title the finder's.
- **Its labels:** `finding`; `kind:qa`, `kind:flake`, `kind:red` or
  `kind:review`; `severity:low`, `severity:medium` or `severity:high`;
  `area:<name>`, from the project's areas; `state:new`, `state:routed`,
  `state:backlog` or `state:dropped`. A project that turns findings on
  has temper create the labels in its home, an effect its owners ask
  for once (`forge.md`, 4: provisioning).
- **Its body:** the key's marker; the reproduction, from a fresh start;
  the commit it was seen at; its signature, a fenced block of the test
  ids and the failure's first line; and, for a routed one, the goal's
  issue it was routed to.
- **Its record** (family `Finding`): its home's issue, kind, state, the
  signature's ids, the times a flake was seen, the goal it was routed
  to, the landing that fixes it, and the forge's labels as last read.

## 3. Read back

- **The keep reads the home's issues** as ever (`forge.md`, 5.2); each
  finding's labels, as the change feed's row carries them, are compared
  with its record. A `state:` label a person changed is the finding's
  new state; a `severity:` or `area:` label is its new severity or
  area; two `state:` labels, or none, is the newest the record knew,
  and temper writes it back.
- **Nothing else is read:** not its body, not its comments, not who
  changed the label. An issue without the marker of temper's key is no
  finding, whatever its labels.
- **A finding closed by another hand,** an issue found closed that
  temper's record keeps open and no close of temper's made, is dropped
  in temper, with "closed on the forge" as its reason; one reopened is
  `new` again. temper reads no actor: what it closed itself, it knows
  from its record. The forge's own closing by a keyword is section 7's.

## 4. Filing and matching

- **Who files:** the QA procedure, from its tester's report (section
  8); verdicts, a flake (after-landing.md, 5.2) and a red of no goal's
  (after-landing.md, section 7); an agent's tool, outside its task
  (`agent.md`, 4.3), as `forge.md` 12's findings were.
- **What a verdict files:** its reproduction is the tier's or gate's
  workflow at the run's commit with its failing tests alone; its first
  line, the failed job's failed step's last line (skein's `Read::Job`);
  its severity `low` for a flake and `high` for a red; its area the
  project's area for the repository (section 9's settings), or none.
- **A signature** is the repository, the tier or gate, a set of test
  ids in byte order, and the failure's first line. A failure is matched
  against the open findings of its kind with the same repository and
  tier or gate, by test id: a flake whose ids are all an open flake
  finding's is counted on it (its record's count, and a comment once
  per day at most), never filed again; a red's or a QA finding's is
  filed anew, naming the open ones it overlaps.
- **Purposes,** tag 9 in `forge.md`'s table: a finding's filing, by its
  source (a verdict's repository, tier and run; a QA pass and its
  case's number; a task's call) and, where it has one, its signature's
  first id in byte order, else 0, so that the same thing asked twice
  files once.

## 5. A flake at a gate

- **When a gate fails** at a candidate (`landing.md`, 7.3) and every
  failing test of the run (its artifact, as a tier's: after-landing.md,
  section 4) matches an open flake finding, its failed jobs are rerun
  once (skein's `Write::Rerun`). Passing, the receipt is made as a pass
  and the flake counted; failing again, it is a real failure, and a
  repair.
- **A gate that reports no tests** is never treated so: its failure is
  a repair, as ever.

## 6. Triage

- **The triager** is named by the project's settings: a task, by its
  number (usually the person's chat), or a role of the project's. New
  findings wake it, at once or on its period: news on the forge's topic
  of a project's findings for a task (`forge.md`, 7.1), a notification
  for a party.
- **Its acts,** a task's through its tools and a party's through
  `temper ctl` (section 9), each writing the finding's labels and
  record in one decision:
  - **to a goal:** news to that goal with the finding, `state:routed`,
    and the goal's issue named in its body;
  - **to a new goal:** the triager makes one, naming the finding, as
    any goal is made, then routes it there;
  - **to the backlog:** `state:backlog`, until a later triage;
  - **dropped,** with a reason: `state:dropped`, the issue closed with
    the reason as a comment.
- **A goal hears a finding** as news it may act on: the fix is a change
  like any, which says `Fixes #n`.

## 7. Closing

- **`Fixes #n`,** in a landed commit's trailers or its pull request's
  body, names finding n in the project's home. The finding is closed,
  with a comment naming the landing, once that landing is cleared
  (after-landing.md, 5.3), not when it lands; a red landing closes
  nothing until it is repaired.
- **The forge's own keywords** would close it at the merge: an owned
  Forgejo's setup turns its closing keywords off (`../protocol/setup.md`,
  `CLOSE_KEYWORDS` empty); on any other forge, a finding closed in the
  minute after a landing whose commits name it is the keyword's, and
  temper reopens it with a comment saying it closes once its landing is
  cleared.
- **A finding closed on the forge** is closed in temper (section 3).

## 8. QA after landing

- **Marks:** a landing marks what a user can now do, by a `QA:` trailer
  in a commit it brought, or a `qa` label on a member's pull request.
  Only marked landings are tested.
- **The chooser,** named by the project's settings (usually the
  triager), hears each new mark once its landing is cleared, and
  chooses it or leaves it in the backlog of marks, from priorities and
  spend: `temper ctl qa choose` or its tool.
- **The pass:** a standing procedure of the forge's, `qa`, one per
  project, which starts a pass once a chosen, untested mark is on a
  cleared main, never two at once. A pass runs on the newest cleared
  main, and covers every chosen mark not yet tested; marks chosen while
  it runs wait for the next.
- **The tester** is the pass's delegate: an agent under the project's
  tester charter, untracked, with no forge effect and no grant to push,
  whose workspace is the cleared main, read-only. It uses the product as
  its users do, and writes down what it expects before trying each
  case. Its result is a report in the verdict contract: each case's
  outcome, and each finding with its reproduction, severity and area,
  in the QA report contract (`agent.md`, 4.4): `cases`, each a mark's
  landing, what it expected, its outcome (`passed`, `failed`,
  `blocked`) and notes; and `findings`, each a title, a reproduction,
  a severity, an area and the mark it came from, each field within the
  contract's bounds.
- **Its findings** are filed (section 4) by the pass from the report,
  kind `qa`, and the triager routes them. The marks it covered are
  tested.

## 9. Effects, views and the store

- **Effects:** an issue created with labels (kind 8), an issue's labels
  set (kind 13, an edit), a comment (kind 7), and, once per project,
  its labels created (a new kind 18, provisioning: a creation on the
  repository's lane, recovered as conditional, since the forge refuses
  a second label of a name, `Exists` counting as made; purpose tag 11,
  the project and the label's name). skein's
  `CreateIssue` takes labels, and `Write::Labels { number, labels }`
  sets an issue's whole set of labels by name, `Write::CreateLabel {
  name, color }` makes one.
- **Views and acts** (`../client/tools.md`, a group `finding` and a
  group `qa`): `finding list --project PROJECT [--state STATE]`,
  `finding show FINDING`, `finding route FINDING --goal TASK`, `finding
  backlog FINDING`, `finding drop FINDING REASON`; `qa marks --project
  PROJECT`, `qa choose LANDING`, `qa passes --project PROJECT`.
- **In the store:** `Finding` (by the home's issue), `Mark` (by
  landing), `QaPass` (by its task), through the baseline's rules.
- **Settings,** with kind 4: findings on or off; the triager; the
  chooser; the tester charter; the project's areas.

## 10. Open questions

- **Severity and area by whom:** the finder's (the tester, the verdict)
  or only the triager's.
- **A review's follow-up as a finding** (the draft's kind `review`):
  filed by a review's verdict contract's follow-ups, or only by an
  agent's tool.
