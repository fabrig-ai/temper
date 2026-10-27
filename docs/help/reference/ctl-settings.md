# temper ctl settings

Kind: reference

The settings: read them, change them, or propose a change to their owners.

## settings show

```
settings show
Read the settings, the revisions running work holds, and pending proposals
Prints: `newest revision R`, each setting and its last revision, the held revisions and their live tasks, and the pending proposals
--json: the record settings.
Key: none.
settings show KIND NUMBER --json prints one value to edit.
settings apply and settings propose take the newest revision this
prints as `--against`.
settings show charter|template|policy NUMBER [--revision REVISION]
  charter|template|policy (kind): one of charter, template or policy; required. The setting's kind
  NUMBER (number): a number from 0 to 4294967295; required. The setting's number
  --revision REVISION: a number from 0 to 4294967295; optional. Its value at this revision; the newest when absent
settings show rules
  rules (kind): one of rules; required. The deployment's rules
Read one setting's value whole
Prints: `KIND N, revision R`, or `rules, revision R`, then the value as JSON on one line
--json: the record setting.
Key: none.
The JSON value is what a settings file's `set` takes.
```

## settings apply

```
settings apply FILE --against REVISION [--running stays|moves] --key KEY
  FILE (items): a file, at most 1048576 bytes; required. The settings file, JSON, or -
  --against REVISION: a number from 0 to 4294967295; required. The newest revision you read
  --running stays|moves: one of stays or moves; optional. Running work stays on its revision, or moves to the new one; stays when absent
Change the deployment's settings in one decision
Prints: `revision R`, the new revision
--json: the record change_settings.
Key: --key KEY, required.
The file is `{"items": [ITEM, ...]}`, at most 1024 items; an item
is `{"setting": {"kind": KIND, "number": NUMBER}, "set": VALUE}`, or
`"edit": "removed"` or `"reset"` in place of `set`; rules takes no
number. A file out of form names the item, member and rule (exit
1). `stale; newest revision R; read settings again` (exit 2): read
the settings, then decide again.
```

## settings propose

```
settings propose FILE [REASON] --against REVISION --key KEY
  FILE (items): a file, at most 1048576 bytes; required. The settings file, JSON, or -
  REASON (reason): text, at most 1048576 bytes, or - to read stdin; optional. Why; empty when absent
  --against REVISION: a number from 0 to 4294967295; required. The newest revision you read
Propose a change of the settings to the deployment's owners
Prints: `proposed: PROPOSAL, to RECIPIENT`
--json: the record propose_settings.
Key: --key KEY, required.
The file is settings apply's. Read the proposal with proposal show
PROPOSAL.
```
