# Settings, revisions and proposals

Kind: concept

How you read and change temper's settings with temper ctl, what a revision
is, how a proposal asks the deployment's owners, and what still needs a restart.

## What the settings are

Temper supplies stock defaults for its settings in the binary. The
store keeps changes over those defaults. A changed value
wins until reset. Removing a default hides its value while keeping
its origin in the inventory. The kinds are numbered charters,
templates and policies, and unnumbered deployment rules. Read
`temper help charters` for what each controls. These values do not go in
`temper.json`, and charter instructions are part of the setting rather
than separate files.

Providers and model catalogues, listener addresses, workers, slots,
secrets and capacity limits are startup inputs rather than settings.
So are sign-in and bootstrap details, forge identities and setup,
demo setup, landing rules and the forge's charter selection. They
are read at start; changing them needs a restart.

## Reading them

Use `me` for your client directory in these examples. The numbers,
including revision 5 and proposal p73/7, are examples: replace them
with the setting, revision, project and proposal you actually read.

```sh
temper ctl me settings
temper ctl me setting charter 1 --json
```

The inventory shows the newest revision, each setting and its
origin, held revisions and holder counts, and pending proposals.
It is not a read of every value. The JSON setting read gives you
`setting`, `revision` and the whole `value` for that one setting.

You need a deployment role to read the inventory and values. Having
only a project role lets you propose a change, but does not let you
read deployment settings or a pending full proposal review.

Read history and an individual decision with:

```sh
temper ctl me revisions
temper ctl me revisions --before 5
temper ctl me revision 5
```

History comes newest first in bounded pages. `--before 5` asks for
entries strictly before revision 5. Follow the offered continuation.
Old entries are eventually forgotten; a forgotten-revision answer
names the oldest revision still kept. A history entry can report a
size or a cut when its contents are too large to show.

Whole charter values can also be read at a held revision, as shown
under Running work. Other whole setting values are read at the newest
revision. A revision record is history of a decision, not a promise
that you can retrieve every older whole value.

## Revisions

A revision records one settings decision: its number, the revision
it was against, time, decider, originating proposal if any, running
choice and changed items. It is separate from a task's specification
revision or the revision of a hold. Use the settings revision for a
settings request.

A change must be against the newest settings revision you read.
If another decision has moved it on, your older number is stale and
changes nothing. Read the settings again, reconsider the whole
change against the new values, and make a fresh request with a new
key and the new `--against` number.

A successful settings revision withdraws other pending settings
proposals as stale and sends their proposers updates. This includes
proposals about different settings: they were all based on the old
whole settings revision.

## Changing settings

Only the deployment's owners can directly change settings. An items
file asks for one whole decision. Each item sets a whole replacement
value, removes a setting, or resets it to its default. It does not
patch individual members. A bad set is refused without partially
changing the settings.

For example, save this JSON as `settings.json` to request a reset of
template 1 to its default:

```json
{"items":[{"setting":{"kind":"template","number":1},"edit":"reset"}]}
```

For a value change, first save `setting ... --json`, as under Reading
them. Keep its `setting` selector, put its `value` under `set` beside
that selector in an item, and edit the whole value there. Put the
item in the file's `items` array. Do not put the read's entire
`setting`, `revision`, `value` response into the array. Use the JSON
returned by the setting read, not a value copied from the
configuration document: their formats and duration units differ.

Rules cannot be removed. Duplicate selectors and unknown JSON
members are refused. An unchanged request is refused too: the reset
example parses but does not guarantee a new revision when the value
is already at its default.

After reading revision 5 and preparing your file, choose a key for
this request:

```sh
temper ctl me change-settings settings.json --against 5 --key reset-chat-template-5
```

A key makes the same request safe to repeat. The same key and the
same ask recover its outcome without making a second revision.
Reusing the key with a changed ask can conflict; choose a fresh key
when you change what you are asking for.

A change that would strand live tasks can be refused **full**, naming
affected tasks. Removing a live task's charter or removing models
its live choices need does not silently migrate that work. The
list of affected tasks can be bounded rather than naming them all.

## Running work

`--running stays` or `--running moves` is recorded with the revision;
a direct change defaults to `stays`. A run already going keeps what
it was given. An agent task records the settings revision in which it
was created, and its later runs read that revision's charter. A new
delegate records the newest revision, even when its parent uses an
older one. Host placement follows that same charter version: changing its
host kinds does not change where an older task's next run may run. Tasks
created after the change use the new host kinds.

A held revision is the settings revision from which a live task's runs
read their charter. It is kept while a task holds it. The settings
inventory shows these revisions and their live holder counts. While
a revision is held, you can read its whole charter value:

```sh
temper ctl me setting charter 3 --revision 5 --json
```

An owner can move eligible live agent tasks from a held revision to
the newest. For example, if revision 5 has two live tasks and the
newest is 6:

```sh
temper ctl me move-to-newest 5 --key move-revision-5
```

The answer is `moved 2 tasks to revision 6`. If some cannot move, the
answer also counts those that stay on 5. A task can move only if the
newest charter permits its model choices. A run already going keeps
its instructions; the task reads its new revision at its next run.
A conversation moves to the newest compatible revision at its next run.

Other setting kinds are read only at the newest revision. A held
revision leaves memory when its last live holder ends or moves away;
its revision record remains readable only within the history window.

The recorded `--running moves` choice moves eligible existing tasks
with the change; `stays` keeps their recorded revisions. Conversations
still move at their next run.

## Proposals

If you have a project or deployment role, you can propose settings
for the deployment's owners to decide. Preparing the same items file
and proposing it does not apply the settings. Give your reason and
a key for the request:

```sh
temper ctl me propose-settings settings.json because --against 5 --key propose-chat-template-5
```

The result names a proposal. `p73/7` means party 73's proposal 7.
With a deployment role, read the proposal before deciding it:

```sh
temper ctl me proposal p73/7
```

The review can show the revision it was against, what is newest,
the reason, and current and proposed values and origins. Look for
**now** and **would be**. Large facts can report their size or
**cut: more not shown** instead of all their contents.

Only deployment owners can accept. Acceptance needs an explicit
running choice, even when choosing `stays`, and records the proposal
identity together with the made revision:

```sh
temper ctl me decide p73/7 accept --running stays --key accept-settings-7
```

To reject with a reason instead:

```sh
temper ctl me decide p73/7 reject because --key reject-settings-7
```

Rejection keeps the reason and notifies the proposer. Do not supply
`--running` when rejecting or passing a proposal up: it is invalid
for those decisions. Ordinary task proposals are described in
`temper help tasks`.

Pending settings proposals survive an unchanged restart. A start
that makes a new revision for changed defaults withdraws proposals
that have become stale, just as a settings change does.

The client's **Inbox** shows a settings **Proposal**, but it does
not compare its values or accept it. Its generic **Accept** lacks
the running choice settings acceptance requires. Review and decide
settings proposals with `temper ctl` using the commands above.

## What needs a restart

An admitted stored settings change takes effect without restarting
the service. Changes to startup inputs need a restart, including
adding models or providers, changing workers or host capability,
and increasing capacity limits. These are not extra setting kinds.

The first start records revision 1. A later start whose defaults
changed records a revision of its own. A stored change still wins over
a default changed at that later start. Editing the configuration does
not undo an accepted stored change; reset the setting when you want to
return it to its default. A reset uses the default at the reset's
revision; work holding an older revision keeps that revision's charter.

## What it cannot do yet

Landing rules and the forge's charter selection are not settings
yet. There is no settings page in the client, nor a page that
compares and accepts settings proposals.

## Reading and proposing from a run

A run whose task authority includes the settings family can use the
`settings` and `propose_settings` host tools. `settings` with `{}` reads
the inventory. `{"setting":{"kind":"charter","number":1},"revision":5}`
reads charter 1 as held revision 5 sees it. With `{"revision":5}` it
reads the decision record, and with `{"history":{}}` it reads history;
`{"history":{"before":5}}` continues before revision 5.

These reads use the same text as temper ctl. Follow `next: OFFSET` by
repeating the read with `"from": OFFSET`; `end` marks the last text page.
The reads make no call record and may be repeated to read settings afresh.

`propose_settings` takes `against`, the newest revision the run read,
`items` in the settings file form above, and a `reason`. Only the
deployment's owners can accept its proposal. A stale answer names the
newest revision so the run can read again and propose afresh. The run
hears the owner's outcome in its messages and next starting inbox:
`accepted: revision R made`, `rejected: REASON`, or
`withdrawn as stale: the newest is R`.
