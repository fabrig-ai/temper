Read `~/src/rust/agents/roles/implementer.md` first; this task departs from it only where it says so.

Then `~/src/rust/tmp/owners/docs-reference/common.md`, the mission's standing rules.

# T3: jig's lines, and the reference pages `limits` rendered from the names tables

## Goal

Render temper's reference pages for limits from the four names tables
(temper-service's, the setup's, the box's, and jig's) and temper's table
of where it reads jig's names (`temper_service::limits::JIG`), check them
in under `docs/help/reference/`, give each a row in `temper-docs`, and
hold them equal to their rendering with a step test of `temper`, as
`commands` and `host-tools` are. An agent that meets a limit's dotted
name in a refused start finds it with `temper help --search NAME`: its
unit, small's value, what `temper init` writes, where it is set, and
what it bounds. First, put the owner's lines into jig's table.

Read first: temper's `docs/design/protocol/documentation.md` 2.1, 2.2,
6 and 8; `crates/temper/src/commands.rs` `reference` and
`crates/temper/src/tests/commands.rs`
`the_page_commands_is_what_the_table_renders`; `crates/temper-docs/src/lib.rs`
(`PAGES`, the reference rows); the tables: jig's
`crates/jig-protocol-config/src/names.rs` and `overrides::value`,
temper's `limits::NAMES`/`value` in temper-service, temper-forge-setup
and temper-box, `temper_service::limits::{JIG, Place, Use}`.

## Where

- **jig:** worktree `~/src/rust/tmp/owners/docs-reference/worktrees/jig-names`,
  branch `docs-reference/names`. One commit there, first (below). Do
  not push.
- **temper:** worktree `~/src/rust/tmp/owners/docs-reference/worktrees/temper-pages`,
  branch `docs-reference/pages`, made from `docs-reference/names` (the
  three tables) with `docs-reference/uses` (the `JIG` table) merged in:
  `git merge --no-ff docs-reference/uses`; resolve the one-line
  conflict in `crates/temper-service/src/limits.rs`, if any, keeping
  both. Commit there; do not push.
- **jig through a local patch** to the jig worktree above, as
  `docs/development/local-jig.md` says; never commit
  `.cargo/config.toml` or a `Cargo.lock` naming jig by path; remove the
  patch and restore `Cargo.lock` at the end.
- Files: in jig, `crates/jig-protocol-config/src/{names.rs,
  tests/names.rs}`; in temper, `crates/temper/src/{lib.rs,
  limit_pages.rs (new), tests.rs, tests/limit_pages.rs (new)}`,
  `crates/temper/Cargo.toml` only if a dependency is missing,
  `crates/temper-docs/src/lib.rs` (16 rows), and the 16 generated pages
  `docs/help/reference/limits*.md`.
- Log: `~/src/rust/tmp/owners/docs-reference/logs/t3.log`.
- Report: `~/src/rust/tmp/owners/docs-reference/reports/t3.md`.

## 1. jig's lines

In jig's `names.rs`, set each row's `line` to the owner's line for its
name, copied byte for byte from
`~/src/rust/tmp/owners/docs-reference/pages/jig-lines.tsv` (name, a tab,
the line; the file's order is not the table's: match by name). Every row
gets one and every line is a row; a name missing on either side is a
`Question:`. In `tests/names.rs`, the test on lines now also requires
each line non-empty and ending with `.`. Run `heavy cargo nextest run
-p jig-protocol-config --lib -E 'test(names)'`; commit ("names: the
owner's line for each of jig's limits").

## 2. The outer layer, in temper

`crates/temper/src/limit_pages.rs`, `pub mod limit_pages;` in `lib.rs`
(the binary's shell: ordinary Rust, as `commands.rs`):

```rust
//! The reference pages `limits` and `limits-PREFIX`, rendered from the
//! limits' names tables (protocol/documentation.md, 8).

/// Each page's name and whole text, in the order `temper-docs` holds them.
#[must_use]
pub fn pages() -> Vec<(String, String)>;
```

with a private table of the 26 prefixes, each its source (temper's
`application.limits` or jig's), its prefix and its line, from
`~/src/rust/tmp/owners/docs-reference/pages/prefix-lines.tsv` (source,
prefix, line, tab-separated; `application` is temper's, `jig` is
jig's), in that file's order, byte for byte.

**The rows, by source:**
- temper's: `temper_service::limits::NAMES`, then
  `temper_forge_setup::limits::NAMES`, then `temper_box::limits::NAMES`;
  each with small's value from its crate's `value` on its small profile
  (`temper_service::limits::profile(Profile::Small)`; the setup's
  `value(&host(), FETCH_BYTES, name)`; the box's `small()`);
- jig's: `jig_protocol_config::names::NAMES`, each with small's value
  from `overrides::value(&profile(Profile::Small), name)` and its
  `Place` and `note` from `temper_service::limits::JIG` (same order).

A row's prefix is its name up to the first `.`. A prefix with 8 rows or
more has a page of its own; the others' rows are on `limits`.

**What `temper init` writes:** the overrides of the document
`temper_init::files` writes (its `temper.json`), given fixed facts as
temper-init's own tests give them: `application.limits.overrides`
for temper's rows, `engine.limits.overrides` and
`workers[0].limits.overrides` for jig's. Read it with the JSON reader
the crate already uses. Never copy the numbers by hand.

## 3. What it renders, byte for byte

Every fixed word below is the owner's: copy it exactly. `\n` is a line
end; no line has a trailing space; every page ends with one `\n`.

**`limits`:** the file
`~/src/rust/tmp/owners/docs-reference/pages/limits-head.md`, whole and
byte for byte (its head is the page's title, kind and summary; it ends
with a line end), then `\n`, then one line per prefix in the prefixes'
order:

```
- `PREFIX.`: LINE; `temper help PAGE`.\n      (a prefix with a page of its own)
- `PREFIX.`: LINE.\n                          (a prefix whose rows are on limits)
```

then the rows of every prefix without a page of its own, in the
prefixes' order, each prefix's rows in its table's order.

**`limits-PREFIX`** (PAGE: `limits-` and the prefix with each `_` as
`-`: `limits-forge-component`), for each prefix with 8 rows or more, in
the prefixes' order:

```
# The `PREFIX.` limits\n
\n
Kind: reference\n
\n
Every limit named `PREFIX.`, for LINE.\n
\n
Where a limit is set, and how a refused one reads: `temper help limits`.\n
```

then its rows, in its table's order.

**A row:**

```
\n
## NAME\n
\n
VALUE.\n
\n
ROW_LINE\n
```

and, for a jig row whose `note` is not empty, `\nNOTE\n` after it.

**VALUE** is PLACE, UNIT, `: `, SMALL, ` in small`, then INIT:

- PLACE, then `, `: temper's rows `Set in `application.limits.overrides``;
  a jig row by its `Place`: `Engine` `Set in `engine.limits.overrides``,
  `Worker` `Set in `workers[N].limits.overrides``, `Both` `Set in
  `engine.limits.overrides` and `workers[N].limits.overrides``; for
  `Neither`, no PLACE and the UNIT word capitalized (`A count`, `Bytes`,
  `Milliseconds`).
- UNIT: `Count` `a count`, `Bytes` `bytes`, `Milliseconds`
  `milliseconds`.
- SMALL: the value in plain decimal digits, no separators.
- INIT: what `temper init` writes for the name at each place it writes
  it (temper's: the application; jig's: the engine, the worker), the
  values equal to SMALL left out. None left: nothing. Otherwise
  `; `temper init` writes ` and either V alone, when every value left
  is V and the places written are the places the row is read from
  (temper's rows: always; `Engine`: the engine; `Worker`: the worker;
  `Both`: both), or else each, `V for the engine`, `W for the worker`,
  or `V for the engine and W for the worker`.

So a row of temper's and two of jig's read:

```
## forge.repositories

Set in `application.limits.overrides`, a count: 2 in small; `temper init` writes 3.

Repositories adopted across every forge, ...

## hub.turn_bytes

Set in `engine.limits.overrides` and `workers[N].limits.overrides`, bytes: 65536 in small; `temper init` writes 1039544.

One whole turn of a run's agent; a larger turn fails the run.

## views.watchers

A count: 4 in small; `temper init` writes 16 for the engine.

Watches open at once; another is answered busy.

temper does not read it: its engine holds a bound of its own.
```

(These illustrate the form; the values are whatever the tables give.)

## 4. temper-docs

In `crates/temper-docs/src/lib.rs`, 16 rows after the last reference
row there is on your base (the reference pages keep their order; new
ones go last), in `pages()`' order, each `Page { name, kind:
Kind::Reference, title, summary, text: include_str!(...) }` with the
title and summary as its file has them, and `PAGES`' length grown by
16. Generate the 16 files from `pages()` once the renderer passes its
tests (the step test below prints each page as it should be); never
edit them by hand.

## 5. The tests

testing-strategy.md 10's shape.

`crates/temper/src/tests/limit_pages.rs`:

- `the_limits_pages_are_what_the_tables_render`: for each page of
  `pages()`, `docs/help/reference/NAME.md` equals its text, failing with
  the page as it should be; the files named `limits*.md` there are
  exactly the pages; `temper-docs`' rows named `limits*` are the pages,
  in their order, consecutive.
- `every_prefix_has_rows_and_every_row_a_prefix`: each prefix of the
  table has at least one row of its source, and every row's prefix is
  one of its source's.
- `a_row_renders_its_place_unit_and_init_as_the_task_says`: render
  single rows through the function that writes a row (make it
  `pub(crate)` or private with the test beside it) for: a temper row
  with no init and one with an init; a jig row of each `Place`; a
  `Both` row written alike in both places; a `Both` row written in the
  engine only; a row whose init equals small; a note. Against inline
  golden text.

`temper-docs`' own step tests (bounds, headings, links) keep passing:
a page over 32,768 bytes, a title past 80 bytes or a summary past 200 is
a `Question:` naming it, never mended.

## Acceptance

- `heavy cargo nextest run -p temper --lib -E 'test(limit_pages)'`,
  `heavy cargo nextest run -p temper-docs`, and `heavy cargo nextest
  run -p temper --lib -E 'test(commands) | test(help)'` pass; report
  counts and times.
- `heavy cargo clippy -p temper -p temper-docs --all-targets -- -D
  warnings` and, in jig, `heavy cargo clippy -p jig-protocol-config
  --all-targets -- -D warnings` are clean.
- The focused suite is waived (common.md).
- The report lists the commits in each repository, each page's name
  and size in bytes, and anything in a page you would question.
- Remove the patch, restore `Cargo.lock`, remove both worktrees'
  `target/`; `git status` clean in each.
