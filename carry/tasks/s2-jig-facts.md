Read `~/src/rust/agents/roles/implementer.md` first; this task departs from it only where it says so: it is a read-only survey. You change no file, build nothing and run no test; your final message is the report, whole.

Then `~/src/rust/tmp/owners/docs-reference/common.md`, the mission's standing rules.

# S2: the facts behind each of jig's limits' names, and what temper does with each

## Goal

The owner will write one line for each limit jig's `limits.overrides`
takes (in temper's document, `engine.limits.overrides` and
`workers[N].limits.overrides`), saying what it bounds, generic to any
application built on jig; and temper's page will say, for the names
temper's engine does not take from there, what it does instead. Gather
the facts those are written from, true of the code: jig at
`~/src/rust/tmp/owners/docs-reference/clones/jig` (main 21cda10fe,
temper's pin) and temper at the worktree's HEAD (9f399c6e6). Facts
only: never a drafted line.

## Where

- Read in the worktree
  `~/src/rust/tmp/owners/docs-reference/worktrees/survey` (temper,
  detached) and the jig clone above; skein and smith at temper's pins
  under `~/.cargo/git/checkouts/` (read only).
- The report is your final message.

## The names

Every name `crates/jig-protocol-config/src/overrides.rs` `apply` takes:
the two before its owner match, then each owner's arms, the prefix and
a dot before the field (`tasks.` + `retries.transient.base_ms`). Fields
bound to `_` have no name. Note which names `overrides::parse` then
refuses through `valid` whatever their value (`valid` lists no
`clients.` prefix).

## For each name, one entry

```
### NAME
- unit: count | bytes | milliseconds (and the field's Rust type)
- small: jig's small profile's value (`profile/small.rs`), with its
  constant's comment, verbatim if short
- init: the value temper's `temper init` writes for it, from
  `crates/temper-init/template/temper.json` in temper:
  `engine.limits.overrides` and `workers[0].limits.overrides`, each or
  none
- bounds: what the field limits, from its doc comment in its domain
  crate (`jig-domain-tasks`, `-people`, `-fleet`, `-authority`,
  `-brief`, `-accounts`, `-views`, `-notes`, `-host`, `-core`, the
  link, `jig-protocol-engine-clients`) and its uses; `path:line`
- at the bound: what happens when it is reached at run time (a request
  refused and the refusal's name, a busy answer, a cut, a wait), with
  `path:line`; or "sizes a table only"
- checks: each start check that names it against another limit or a
  figure (jig's `parse`, `valid`, `check_message`,
  `check_effective_limits`, `clients::check_settings`, the worst cases),
  with the other name and `path:line`
- side: engine, worker or both: which of jig's processes reads the
  field (the engine's core and children, the hub on either side, the
  link, the clients' component, the worker's own parts), with evidence
- temper: what temper's engine does with it, from temper's
  `crates/temper-service/src/limits.rs` `engine_candidate` and its
  callers, and temper's `docs/design/protocol/configuration.md` 7
  ("What the root takes of `engine.limits`"): `taken` (the root or a
  component uses the configured value), `from NAME` (temper's root
  takes the value from its own limit NAME instead: `root.brief.parts`
  for `brief.parts`), `fixed N` (temper holds a constant whatever the
  override), or `special` with one line (as `core.resume_bytes`); with
  `path:line`
- design: jig's `docs/design/protocol/configuration.md` 6 sentence (or
  another section) that names it, quoted, or none
```

Keep each entry short: the facts, not prose. Where code and design
disagree, add `differs:`.

## Also

- **The prefixes:** for each owner prefix (`tasks.`, `people.`,
  `fleet.`, `authority.`, `accounts.`, `notes.`, `documents.`, `views.`,
  `brief.`, `core.`, `hub.`, `link.`, `clients.`), one line: what it
  owns, and which side reads it.
- **The refusals:** the exact line `temper serve` prints for an unknown
  jig name, a value past its type, and a worst case broken by an
  override, under `engine.limits.overrides` and under
  `workers[0].limits.overrides`, from the code that formats each, with
  `path:line` (jig's `Problems`/`What` and temper's printing of them).
- **The standard profile:** whether temper can select it (temper's
  `application.limits.profile` takes `small` alone; does `engine.limits`
  take `standard`?), with evidence.

## The report

Your final message, Markdown: the prefixes, the refusals, the
standard profile, then the entries grouped by prefix in the parser's
order. Its first line: `S2: N names` with the count.
