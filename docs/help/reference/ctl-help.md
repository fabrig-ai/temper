# temper ctl help

Kind: reference

Read the page ctl and the groups, a group's tools, or one tool's record.

## help

```
help [GROUP [VERB]]
  GROUP (group): text, at most 1048576 bytes; optional. A group, as help lists them
  VERB (verb): text, at most 1048576 bytes; optional. One of the group's tools, as help GROUP lists them
Read the page ctl and the groups, a group's tools, or one tool's record
Prints: The page ctl, then a line per group; with GROUP, its tools; with GROUP VERB, the tool's record
--json: the record help.
Key: none.
It takes no DIR. With `--json`, it prints the tool's first record.
```
