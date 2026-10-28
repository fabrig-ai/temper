Read `~/src/rust/agents/roles/implementer.md` first; this task departs from it only where it says so.

Then `~/src/rust/tmp/owners/docs-reference/common.md`, the mission's standing rules.

# T1: jig's limits' names, a list beside its overrides' parser

## Goal

jig's engine part and worker part take `limits.overrides`, a map from a
limit's dotted name to a value, parsed by `jig-protocol-config`'s
`overrides::apply`, one long match. Give those names a list beside the
parser, `names::NAMES`, a row per name with its unit and a line, and make
the parser a lookup in it: a name with no row is refused before any
match, and every row is a name the parser takes. Add `overrides::value`,
which reads a named limit's value, so that an application can show each
name's value in a profile. temper renders its reference page `limits`
from this list in a later task. The rows' lines stay empty in this task:
the owner writes them later.

Read first: temper's `docs/design/protocol/documentation.md` 8, the row
`limits` and the bullet `limits:` (in
`~/src/rust/tmp/owners/docs-reference/clones/temper`); jig's
`docs/design/protocol/configuration.md` 6, first bullets;
`crates/jig-protocol-config/src/overrides.rs` whole;
`crates/jig-protocol-config/src/profile.rs` and `profile/small.rs`;
`crates/jig-protocol-config/src/tests/overrides.rs` and
`tests/provider.rs` `provider_limits_are_positive_named_overrides_with_profile_defaults`.

## Where

- Worktree `~/src/rust/tmp/owners/docs-reference/worktrees/jig-names`,
  jig, on branch `docs-reference/names` (made from `origin/main`,
  21cda10fe, temper's pin). Commit there; do not push.
- Files: `crates/jig-protocol-config/src/{lib.rs, names.rs (new),
  overrides.rs, tests.rs, tests/names.rs (new), tests/overrides.rs}`;
  `src/clients.rs` or `src/overrides.rs` `valid` only as the section
  "A row the document refuses" says.
- Log: `~/src/rust/tmp/owners/docs-reference/logs/t1.log`.
- Report: `~/src/rust/tmp/owners/docs-reference/reports/t1.md`.

## The outer layer

A new module `src/names.rs`, `pub mod names;` in `lib.rs`:

```rust
//! The limits' names `limits.overrides` takes, a row each, which the
//! overrides' parser looks up (configuration.md, 6).

/// What a limit's value counts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Unit {
    /// A number of things.
    Count,
    Bytes,
    /// A duration, its name ending `_ms`.
    Milliseconds,
}

/// One named limit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Name {
    /// The dotted name, as `limits.overrides` spells it: `tasks.inbox_messages`.
    pub name: &'static str,
    pub unit: Unit,
    /// What it bounds, one sentence; documentation text, the owner's.
    pub line: &'static str,
}

/// Every name the overrides take, grouped by owner prefix.
pub const NAMES: [Name; N] = [ /* ... */ ];

/// The row of a name, by its exact bytes.
#[must_use]
pub fn find(name: &[u8]) -> Option<&'static Name>;
```

`Name` and `Unit` are generic: an application with limits of its own
(temper) gives its names as rows of the same type, so name nothing of
jig's domains in their docs.

In `overrides.rs`:

```rust
/// Replace one whole scalar limit, named by a row of `names::NAMES`.
pub fn apply(limits: &mut Limits, name: &[u8], value: u64) -> Result<(), What>;   // unchanged signature

/// A named limit's value in `limits`, a duration's in milliseconds; none
/// for a name with no row.
#[must_use]
pub fn value(limits: &Limits, name: &[u8]) -> Option<u64>;
```

- **`apply`** first finds the name's row (`names::find`); with none, it
  returns `Err(What::Override)` as an unknown name does today. Then it
  sets the field as today, with every side effect it has today
  (`accounts.token` setting the driver's document token bytes,
  `authority.` copying the roles, `accounts.` recomputing the driver's
  listeners and places): no behaviour changes for a name that has a row.
- **`value`** reads the same fields. The mapping from a name to its
  field is written once: one match per owner, as today, that reads the
  field and, when given a value, sets it first (for example a private
  `fn field(limits: &mut Limits, name: &[u8], to: Option<u64>) ->
  Result<u64, What>`, with `count`/`millis` helpers that take the
  field by `&mut` and the optional value). `value` runs it on a copy
  of `limits` (`Limits` is `Copy`) with no value; reading has no side
  effects. Not two parallel matches.
- **No closure, trait, generic item or hash map.** A lookup over
  `NAMES` is a `for` loop or `iter().position` without a closure
  (a plain loop is fine).

## The table

- **Every name today's `apply` takes is a row, and no other.** Walk
  `overrides.rs`: the two names `apply` takes before its owner match
  (`accounts.token`, `accounts.sign_in_deadline_ms`), then each owner's
  match arms, the owner's prefix and a dot before each field name
  (`tasks.` + `retries.transient.base_ms`). Names whose fields are bound
  to `_` in the destructuring (`versions`, `revisions`, `act_grants`...)
  have no arm and no row.
- **Order:** by owner prefix in this order: `tasks`, `people`, `fleet`,
  `authority`, `accounts`, `notes`, `documents`, `views`, `brief`,
  `core`, `hub`, `link`, `clients`; within a prefix, the match arms'
  order, `accounts.token` and `accounts.sign_in_deadline_ms` after the
  accounts' own arms.
- **`unit`:** `Milliseconds` for every duration field (its name ends
  `_ms`, its setter `millis`), and only those; `Bytes` for every name
  ending `_bytes`, and for any other field whose own documentation (the
  field's doc comment in its domain crate, or `profile/small.rs`'
  comments) says it is a byte count (`hub.event_bytes` yes; a field
  like `token` that bounds a credential's bytes, check its doc);
  `Count` for the rest. The report lists every row that is `Bytes`
  without ending `_bytes`, and every row whose unit you were unsure of,
  with the doc that decided it.
- **`line`:** `""` in every row. Never draft one.

## A row the document refuses

`overrides::parse` checks each override after `apply` with `valid`,
which refuses a name whose prefix it does not list. Today `valid` lists
`hub`, `link` and the core's prefixes but not `clients`, so
`clients.provider_codes` and `clients.provider_tokens`, which `apply`
takes and jig's `configuration.md` 3 calls named limits, are refused in
a document. If the test `every_row_parses_at_its_own_small_value` below
confirms it:

- fix `valid` so that a `clients.` name is valid when
  `jig_protocol_engine_clients::worst_case(&limits.clients)` is some
  (the same family check the other prefixes have), in a commit of its
  own whose message says why; the provider test above already holds
  zero refused by `apply`;
- any other row the test refuses: stop and report it as a `Question:`
  with the name, the problem `parse` gives and its cause; do not change
  it.

## The tests

In a new `src/tests/names.rs` (registered in `src/tests.rs` as the
others are), in testing-strategy.md 10's shape: one behaviour each,
the setup its assertion reads.

- `every_row_is_a_name_apply_takes_at_its_small_value`: for each row,
  `value(&small, row.name)` is some `v`, and `apply` on a copy of
  `small` with `v` succeeds and leaves the copy equal to `small`.
- `every_row_parses_at_its_own_small_value`: for each row, `parse` of
  `{"profile":"small","overrides":{NAME: V}}`, V its `value` in small,
  is `Ok` and equal to `parse` of `{"profile":"small","overrides":{}}`.
  (Use the existing `fixtures::document` and `path`.)
- `a_name_with_no_row_is_refused`: a name the old match would not take
  (`tasks.absent`) and a name of a real field that has no row (pick
  one bound to `_`, such as `tasks.versions`) are each
  `Err(What::Override)` from `apply`, and `value` is none for each.
- `rows_are_unique_and_well_formed`: names unique; each `PREFIX.FIELD`
  with PREFIX one of the thirteen above, in the order above (the
  prefixes appear as contiguous groups in that order); each of
  `[a-z0-9_.]` only; a name ends `_ms` if and only if its unit is
  `Milliseconds`; a name ending `_bytes` is `Bytes`.
- `each_line_is_at_most_160_bytes_on_one_line`: every `line` within
  160 bytes, holding no line end. (Empty lines pass.)
- `value_reads_a_duration_in_milliseconds`: `value(&small,
  b"tasks.grace_ms")` equals small's `core.tasks.grace` in
  milliseconds.

Also in `src/tests/overrides.rs`: four functions there lack `#[test]`
(`an_override_of_an_unknown_name_is_refused`,
`an_override_past_its_type_is_refused`,
`an_override_that_breaks_a_worst_case_is_refused_naming_it`, and any
other `fn` there that reads as a test and has none): add `#[test]` to
each, in a commit of its own. If one fails, report it as a `Question:`
with its output; do not change its assertion.

## Acceptance

- `heavy cargo nextest run -p jig-protocol-config --lib` passes, the
  new tests among them; report the count and the time.
- `heavy cargo clippy -p jig-protocol-config --all-targets -- -D
  warnings` is clean.
- The focused suite is waived (common.md).
- The report lists: the commits; the row count by prefix; the unit
  choices as above; what `every_row_parses_at_its_own_small_value`
  found; the four tests' outcome.
- Remove the worktree's `target/` when done.
