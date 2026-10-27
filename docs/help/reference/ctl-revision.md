# temper ctl revision

Kind: reference

Settings revisions: read one or the history, move running work to the newest.

## revision show

```
revision show REVISION
  REVISION (revision): a number from 0 to 4294967295; required. The revision's number
Read one revision of the settings and its items
Prints: `revision R, against revision A`, when, by whom, from which proposal, whether running work moved, then its items
--json: the record revision.
Key: none.
A forgotten revision prints `revision R is forgotten; the oldest
kept is revision O`, with exit 0. An item too large to print names
the settings show call that reads it.
```

## revision list

```
revision list [--before REVISION]
  --before REVISION: a number from 0 to 4294967295; optional. Older than this revision
Read the settings' history, newest first, a page at a time
Prints: Each revision's head and its items' edits, then `read on: revision list --before REVISION`, `no older revisions` or `older revisions were forgotten`
--json: the record revisions.
Key: none.
```

## revision advance

```
revision advance REVISION --key KEY
  REVISION (revision): a number from 0 to 4294967295; required. The held revision
Move the live tasks on held REVISION to the newest revision
Prints: `moved N tasks to revision R`, then how many could not, which finish on their revision
--json: the record move_to_newest.
Key: --key KEY, required.
```
