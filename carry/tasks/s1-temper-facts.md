Read `~/src/rust/agents/roles/implementer.md` first; this task departs from it only where it says so: it is a read-only survey. You change no file, build nothing and run no test; your final message is the report, whole.

Then `~/src/rust/tmp/owners/docs-reference/common.md`, the mission's standing rules.

# S1: the facts behind each of temper's limits' names

## Goal

The owner will write one line for each limit an operator may set in
`application.limits.overrides` of temper's configuration document,
saying what it bounds, for an agent who meets the name in a refused
start or wants to raise it. Gather, for every such name, the facts that
line is written from, true of the code at temper `main`, 9f399c6e6. Facts only: never a drafted line.

## Where

- Read in the worktree
  `~/src/rust/tmp/owners/docs-reference/worktrees/survey` (temper,
  detached at `origin/main`). jig, at temper's pin, is readable at
  `~/src/rust/tmp/owners/docs-reference/clones/jig`, and skein and smith
  at their pins under `~/.cargo/git/checkouts/` (read only).
- No log: the run is short. The report is your final message.

## The names

Every name these parsers take, and no other:

- `crates/temper-service/src/limits/names.rs` `apply` (its match arms,
  and `setup.` handed to `temper_forge_setup::limits::apply_host`);
- `crates/temper-forge-setup/src/limits.rs` `apply` and `apply_host`,
  and `setup.fetch_bytes` in `crates/temper-forge-setup/src/check.rs`;
- `crates/temper-box/src/limits.rs` `apply`;
- how `crates/temper-service/src/startup/application.rs`
  `configured_limits` and `crates/temper/src/setup.rs` `box_limits`
  route each name (`handed_on`, `host_setup`).

## For each name, one entry

```
### NAME
- unit: count | bytes | milliseconds (and the field's Rust type)
- small: the small profile's value, in that unit (`temper-service`
  `limits::profile`, the setup's `host()`/`small()`/`FETCH_BYTES`, the
  box's `small()`); with the constant's own comment, verbatim
- init: the value `temper init` writes for it in
  `crates/temper-init/template/temper.json`
  `application.limits.overrides`, or none
- bounds: what the field limits, from its doc comment and its uses (the
  struct field's documentation, the code that admits or refuses at it);
  `path:line` for each
- at the bound: what happens when the bound is reached at run time (a
  request refused and how it is named, a busy answer, a value cut, a
  wait, an item dropped), with `path:line`; or "sizes a table only"
- start checks: each check at start that names it against another limit
  or a figure (configuration.md 7's "checked against each other", the
  worst case, the channel's), with the other name and `path:line`
- refused when: the values its parser refuses (zero, above a maximum,
  past u32), with `path:line`
- design: the row or sentence of `docs/design/protocol/configuration.md`
  7 (or another design section) that names it, quoted, or none
```

Keep each entry short: the facts, not prose. Where the code and the
design disagree, say so in the entry under `differs:`.

## Also

- **The prefixes:** for each owner prefix (`root.`, `forge.`,
  `checkout.`, `forge_component.`, `git_component.`, `records.`,
  `forges.`, `setup.`, `box.`, `clients.`, `routes.`, `sign_in.`,
  `picker.`), one line: which process takes it (the engine, a worker,
  the box, `temper forge` commands), and in which configuration shape
  (standalone, an engine alone, a worker alone: configuration.md 2's
  table and 7 "In the cluster's shapes").
- **A refused name or value:** the exact line `temper serve` prints for
  an unknown name, a value past its type, and a failed check between
  two limits, for one name of each parser (service, setup, box), from
  the code that formats it, with `path:line`.
- **Names whose override changes nothing at run time** (a field read by
  nothing past the start), if any, with the evidence.

## The report

Your final message, Markdown: the prefixes, the refusal lines, then the
entries grouped by prefix in the parsers' order, then any names that
change nothing. Its first line: `S1: N names` with the count.
