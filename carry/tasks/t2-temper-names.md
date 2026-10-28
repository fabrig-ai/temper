Read `~/src/rust/agents/roles/implementer.md` first; this task departs from it only where it says so.

Then `~/src/rust/tmp/owners/docs-reference/common.md`, the mission's standing rules.

# T2: temper's limits' names, a table beside each of its three parsers

## Goal

temper's configuration takes `application.limits.overrides`, a map
from a limit's dotted name to a value, parsed by three parsers:
`temper-service`'s (`limits::apply`, `limits/names.rs`), the setup's
(`temper-forge-setup`'s `limits::apply`, `apply_host`, and
`setup.fetch_bytes` in `check.rs`) and the box's (`temper-box`'s
`limits::apply`). Give each parser a table of its names beside it,
`NAMES`, a row per name with its unit and its line, in jig's row type
(`jig_protocol_config::names::{Name, Unit}`, built by the previous task
on jig's branch `docs-reference/names`), and make each parser a lookup
in its table: a name with no row is refused before any match, and every
row is a name its parser takes. Add a `value` reader to each, through
the same mapping its parser sets. temper renders the reference pages
`limits` from these tables and jig's in the next task.

Read first: temper's `docs/design/protocol/documentation.md` 8 (row
`limits`, bullet `limits:`) and `docs/design/protocol/configuration.md`
7 (the prefixes table); jig's branch's
`crates/jig-protocol-config/src/names.rs` and `overrides.rs` (`apply`,
`value`, `field`: the pattern to follow); in temper
`crates/temper-service/src/limits.rs`, `limits/names.rs`,
`startup/application.rs` `configured_limits`;
`crates/temper-forge-setup/src/limits.rs` and `check.rs` `checked`;
`crates/temper-box/src/limits.rs`; `crates/temper/src/setup.rs`
`box_limits`; `crates/temper-service/src/tests/limits.rs`.

## Where

- Worktree `~/src/rust/tmp/owners/docs-reference/worktrees/temper-names`,
  temper, on branch `docs-reference/names` (made from `origin/main`).
  Commit there; do not push.
- **jig through a local patch:** write the untracked
  `.cargo/config.toml` of temper's `docs/development/local-jig.md`
  with `jig_checkout=~/src/rust/tmp/owners/docs-reference/worktrees/jig-names`
  (jig's branch `docs-reference/names`, which has `names`). Never commit
  `.cargo/config.toml` or a `Cargo.lock` that names jig by path:
  before each commit, `git diff --cached --name-only` shows neither;
  keep the patch until the task is done, then remove it and restore
  `Cargo.lock` (`git checkout Cargo.lock`). Do not change jig.
- Files: `crates/temper-service/src/{limits.rs, limits/names.rs,
  tests/limits.rs or tests/names.rs (new)}`;
  `crates/temper-forge-setup/src/{limits.rs, check.rs, tests.rs or
  tests/names.rs}`; `crates/temper-box/{Cargo.toml, src/limits.rs,
  src/tests/limits.rs}`; `crates/temper/src/tests/` (one new test file,
  registered as the others); a `Cargo.toml` dependency line where a
  crate now names `jig-protocol-config` (temper-box).
- Log: `~/src/rust/tmp/owners/docs-reference/logs/t2.log`.
- Report: `~/src/rust/tmp/owners/docs-reference/reports/t2.md`.

## The outer layer

Each of the three crates:

```rust
use jig_protocol_config::names::{Name, Unit};

/// Every name this parser takes, in the order below.
pub const NAMES: [Name; N] = [ /* ... */ ];

/// A named limit's value, a duration's in milliseconds; none for a name
/// with no row.
#[must_use]
pub fn value(limits: &Limits, name: &[u8]) -> Option<u64>;
```

- **temper-service:** `limits::NAMES` (in `limits/names.rs`,
  re-exported from `limits` as `apply` is) holds every name its
  `names::apply` match takes today, `setup.` excepted: the prefixes
  `sign_in`, `picker`, `routes`, `root`, `forge`, `checkout`,
  `git_component`, `clients`, `records`, `forges`, `forge_component`.
  `limits::value(&Limits, name)`. Its `apply` keeps handing `setup.`
  to the setup's `apply_host` as today.
- **temper-forge-setup:** `limits::NAMES` holds the eight `setup.`
  names: `setup.orgs`, `setup.deadline_ms`, `setup.backoff_ms`,
  `setup.backoff_max_ms`, `setup.runs`, `setup.states`,
  `setup.state_life_ms`, `setup.fetch_bytes`. `apply`, `apply_host`
  and `checked`'s `setup.fetch_bytes` branch each refuse a `setup.`
  name with no row before their match. `forge.client.repositories`,
  which `checked` receives from the service's start as a resolved
  input (`startup/application.rs` pushes it into `handed`), is not a
  setup row: keep it working exactly as today, its exception named in a
  comment. Its value reader is
  `pub fn value(host: &Host, fetch_bytes: u64, name: &[u8]) -> Option<u64>`
  (`fetch_bytes` is not in `Host`); `value(&host(), FETCH_BYTES, name)`
  gives small's.
- **temper-box:** `limits::NAMES` holds the eight `box.` names, its
  `apply` a lookup in it in place of its list, and
  `limits::value(&Limits, name)`. temper-box gains a dependency on
  `jig-protocol-config` for the row type (already in its graph through
  temper-forge-setup).

Inside each crate, the mapping from a name to its field is written
once: one match that reads the field and, when given a value, sets it
first (jig's `field(limits, name, to: Option<u64>)`), `apply` and
`value` both through it; `value` on a copy (`Limits` is `Copy`) with no
value. Not two parallel matches. Every refusal keeps its kind and its
words (`Refused::Unknown`, `Refused::Range`, and the setup's zero and
maximum rules): no behaviour changes for a name that has a row. No
closure, trait, generic item or hash map in the step crates
(temper-service, temper-forge-setup, temper-box follow their files'
idiom; a lookup is a `for` loop).

## The rows

- **Order:** temper-service's by prefix in the order above, each
  prefix's names in its match arms' order; the setup's and the box's in
  the order of the lists above and of today's box list.
- **`unit`:** `Milliseconds` for every duration (its name ends `_ms`),
  and only those; `Bytes` for every name ending `_bytes` and for
  `forge_component.chunk`, `forge_component.tls.read`,
  `forge_component.tls.send` and `forge_component.tls.records`; `Count`
  for the rest. If a field's own documentation contradicts one of
  these, it is a `Question:`.
- **`line`:** copied byte for byte from
  `~/src/rust/tmp/owners/docs-reference/pages/temper-lines.tsv`: one row
  per line, the name, a tab, the line. Every row of the three tables
  has its line there and every line there is a row; a name missing on
  either side is a `Question:`. Backticks and apostrophes are part of
  the text. Never edit a line.

## The tests

testing-strategy.md 10's shape: one behaviour each, the setup its
assertion reads.

In each of the three crates (its tests module, as its other limits
tests are):

- `every_row_is_a_name_apply_takes_at_its_small_value`: for each row,
  `value` in small is some `v`; applying `v` to a copy of small
  succeeds and leaves it equal to small (the setup: `apply_host` on
  `host()` for the host's seven, `apply` on `small()` for the plan's
  four, `checked`'s branch for `setup.fetch_bytes` through whatever
  function you give it).
- `a_name_with_no_row_is_refused`: an unknown name of the crate's prefix
  (`forge.unknown`, `setup.unknown`, `box.unknown`) and, in
  temper-service, a real field that no override names
  (`forge.bindings`, `forge.authority_segments`): `Refused::Unknown`,
  `value` none, the limits unchanged.
- `rows_are_unique_and_well_formed`: names unique; each starts with
  one of the crate's prefixes, the prefixes in contiguous groups in the
  order above; only `[a-z0-9_.]`; ends `_ms` if and only if
  `Milliseconds`; ending `_bytes` is `Bytes`.
- `each_line_is_one_sentence_of_at_most_160_bytes`: every line
  non-empty, at most 160 bytes, holding no line end, ending with `.`.

In `crates/temper/src/tests/` (the crate that sees all three), one new
file:

- `every_name_the_start_takes_is_a_row_routed_to_its_parser`: the three
  tables' names are disjoint; for each service row,
  `temper_service::limits::handed_on` is false; for each setup and box
  row it is true, and `host_setup` is true for exactly the seven setup
  rows `apply_host` takes; each row is taken by the parser the start
  routes it to (as `every_row_is_a…` above, through the crates' public
  functions).

Existing tests that list names (`tests/limits.rs`) keep passing
unchanged; one whose list a table now makes redundant stays.

## Acceptance

- `heavy cargo nextest run -p temper-service --lib -E
  'test(limits) | test(names)'`, `heavy cargo nextest run -p
  temper-forge-setup --lib`, `heavy cargo nextest run -p temper-box
  --lib`, and the new `temper` test by name: all pass; report counts
  and times.
- `heavy cargo clippy -p temper-service -p temper-forge-setup -p
  temper-box -p temper --all-targets -- -D warnings` is clean.
- The focused suite is waived (common.md).
- The report lists the commits, the row count of each table, and any
  name whose routing or unit surprised you.
- Remove `.cargo/config.toml`, restore `Cargo.lock`, and remove the
  worktree's `target/` when done; `git status` shows the tree clean.
