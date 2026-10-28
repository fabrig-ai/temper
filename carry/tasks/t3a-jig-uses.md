Read `~/src/rust/agents/roles/implementer.md` first; this task departs from it only where it says so.

Then `~/src/rust/tmp/owners/docs-reference/common.md`, the mission's standing rules.

# T3a: where temper reads each of jig's limits' names

## Goal

jig's limits' names (`jig_protocol_config::names::NAMES`, 210 rows on
jig's branch `docs-reference/names`) are taken by both
`engine.limits.overrides` and `workers[N].limits.overrides` of temper's
document, since both parse a whole jig profile. Which of them temper
actually reads is temper's fact: the engine's root takes some of jig's
limits and holds its own for others (`brief.`, `views.`, some `core.`),
the engine's channel to its workers takes some `hub.` and `link.`
names, and each worker takes its hub's and link's. Give temper a table,
a row per jig name, saying where temper reads it, with a note where
temper does something of its own; and hold the table to temper's code
with a step test that changes each name and sees which side changes.
The reference pages `limits` render it in a later task.

Read first: temper's `docs/design/protocol/configuration.md` 7 (the
bullets "What the root takes of `engine.limits`" and "Call input
derivation for a local agent"); the survey
`~/src/rust/tmp/owners/docs-reference/reports/s2.md`, "Prefixes" and
each entry's `side:` and `temper:` lines (its abbreviations: `TL`
`crates/temper-service/src/limits.rs`, `TP`
`crates/temper-engine-domain/src/limits.rs`, `TW`
`crates/temper-service/src/workers.rs`, `TS`
`crates/temper-service/src/worker_settings.rs`); in temper
`crates/temper-service/src/limits.rs` `engine_candidate`.

## Where

- Worktree `~/src/rust/tmp/owners/docs-reference/worktrees/temper-uses`,
  temper, on branch `docs-reference/uses` (made from `origin/main`).
  Commit there; do not push. Another task changes
  `crates/temper-service/src/limits.rs` and `limits/names.rs` on another
  branch: touch `limits.rs` only to add `mod jig;` and its `pub use`.
- **jig through a local patch,** as `docs/development/local-jig.md`
  says, `jig_checkout=~/src/rust/tmp/owners/docs-reference/worktrees/jig-names`;
  never commit `.cargo/config.toml` or a `Cargo.lock` naming jig by
  path (check `git diff --cached --name-only` before each commit);
  remove the patch and restore `Cargo.lock` when done. Do not change
  jig.
- Files: `crates/temper-service/src/limits/jig.rs` (new), one line or
  two in `limits.rs`, a new test file under
  `crates/temper-service/src/tests/` registered as the others; a
  function made `pub(crate)` where the test needs a projection that is
  private today, nothing more.
- Log: `~/src/rust/tmp/owners/docs-reference/logs/t3a.log`.
- Report: `~/src/rust/tmp/owners/docs-reference/reports/t3a.md`.

## The outer layer

`crates/temper-service/src/limits/jig.rs`, re-exported from `limits`
as `limits::{JIG, Place, Use}`:

```rust
//! Where temper reads each of jig's limits' names (protocol/configuration.md, 7).

/// Where a jig name of a deployment's document is read.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Place {
    /// From `engine.limits`: the engine reads it.
    Engine,
    /// From `workers[N].limits`: each worker reads its own.
    Worker,
    /// From both: the engine and each worker read their own.
    Both,
    /// From neither: the parsers take it and temper reads it nowhere.
    Neither,
}

/// How temper uses one of jig's names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Use {
    pub name: &'static str,
    pub place: Place,
    /// What temper does beyond the name's own line; documentation text,
    /// the owner's; empty where there is nothing to add.
    pub note: &'static str,
}

/// A row per row of jig's `names::NAMES`, in its order.
pub const JIG: [Use; 210] = [ /* ... */ ];
```

`place` is data the test below decides; `note` is the owner's text,
copied byte for byte from the list below, `""` for every other row.
Never draft a note.

## The notes

Name, a tab, the note (backticks are part of the text):

```
brief.briefs	temper does not read it: its engine takes `root.brief.briefs`.
brief.sections	temper does not read it: its engine takes `root.brief.sections`.
brief.read_bytes	temper does not read it: its engine takes `root.brief.read_bytes`.
brief.brief_bytes	temper does not read it: its engine takes `root.brief.brief_bytes`.
views.runs	temper does not read it: its engine holds a bound of its own.
views.watchers	temper does not read it: its engine holds a bound of its own.
views.watchers_per_party	temper does not read it: its engine holds a bound of its own.
views.subjects_per_change	temper does not read it: its engine holds a bound of its own.
views.text_bytes	temper does not read it: its engine holds a bound of its own.
views.facts	temper does not read it: its engine holds a bound of its own.
core.fill_mark	temper does not read it: its engine holds a bound of its own.
core.connectors	temper does not read it: its engine has one connector, the forge's.
core.run_bytes	temper does not read it: its engine takes `root.journal.run_bytes`.
core.escalation_reason_bytes	temper does not read it: its engine takes the least of `tasks.result_bytes`, `root.journal.result_bytes` and `root.journal.transcript_bytes`.
core.load_slots	temper does not read it: its engine takes `root.loads.loads`.
core.load_bytes	temper does not read it: its engine takes `root.loads.read_bytes`.
core.brief_parts	temper does not read it: its engine takes `root.brief.parts`.
core.brief_core_budgets.tail	temper does not read it: its engine holds a bound of its own.
core.brief_core_budgets.task	temper does not read it: its engine takes `root.brief.budgets.task`.
core.brief_core_budgets.dependencies	temper does not read it: its engine takes `root.brief.budgets.dependencies`.
core.brief_core_budgets.attempts	temper does not read it: its engine takes `root.brief.budgets.attempts`.
core.brief_core_budgets.plan	temper does not read it: its engine takes `root.brief.budgets.plan`.
core.brief_core_budgets.notes	temper does not read it: its engine takes `root.brief.budgets.notes`.
core.resume_bytes	temper's engine holds it above 0 and within `root.journal.transcript_bytes`.
core.call_answer_bytes	Left out, temper takes the larger of small's value and the agent's call input; a value below the agent's call input refuses the start.
fleet.engine_slots	temper places every run on a worker: any value but 0 refuses the start.
hub.slots	temper's worker runs `workers[N].slots` at once, which must be within it.
```

## The test

In a new test file of temper-service, testing-strategy.md 10's shape:

- `jigs_names_each_have_one_row_in_order`: `JIG`'s names equal
  `jig_protocol_config::names::NAMES`' names, in order.
- `each_place_is_where_temper_reads_the_name`: for each row, take jig's
  small profile (`jig_protocol_config::profile(Profile::Small)`), read
  the name's value with `jig_protocol_config::overrides::value`, and
  apply a different value with `overrides::apply` (one more, or one less
  where one more is refused; a row whose change no value can show is a
  `Question:`). Then compare, before and after, what temper derives
  from jig's limits on each side, as its start does:
  - **the engine's side:** `limits::engine_candidate(&jig, &own)` (own:
    temper's small profile), the engine's channel limits to its workers
    (`workers.rs`), the accounts' driver and the clients' component
    limits the engine's start builds from jig's limits;
  - **a worker's side:** what `worker_settings.rs` builds for a worker
    from its jig limits (its hub, its link, its credentials).

  Each projection is called through temper's own functions, never
  re-derived in the test; make one `pub(crate)` where needed. Where a
  projection needs inputs beyond jig's limits (an agent's profile, a
  worker's slots), use the small profile's and the test fixtures'
  values the existing tests use (`tests/fixtures`). The row's `place`
  is `Engine` when only the engine's side changes, `Worker` when only
  a worker's, `Both`, or `Neither`. Fill each row's `place` from this
  test's findings and S2's `side:`/`temper:` lines; the assertion
  holds every row. A row where the test and S2 disagree: follow the
  test, and list it in the report with both.
- `a_name_read_nowhere_says_what_temper_does`: a `Neither` row has a
  note; every note is one line of at most 200 bytes ending with `.`.
  A `Neither` row with no note in the list above is a `Question:`
  naming it.

## Acceptance

- `heavy cargo nextest run -p temper-service --lib -E 'test(jig)'`
  (or the new file's module) passes; report the count and the time,
  which stays well under a second.
- `heavy cargo clippy -p temper-service --all-targets -- -D warnings`
  is clean.
- The focused suite is waived (common.md).
- The report lists the commits, the count of each `Place`, every row
  where the test and S2 differ, and every projection function you made
  `pub(crate)`.
- Remove the patch, restore `Cargo.lock`, remove the worktree's
  `target/`; `git status` clean.
