Read `~/src/rust/agents/roles/implementer.md` first; this task departs from it only where it says so.

Then `~/src/rust/tmp/owners/docs-reference/common.md`, the mission's standing rules.

# T5: jig's configuration members, a table by shape beside its decoder

## Goal

temper's reference page `configuration` will show every member of the
configuration document with its type, bound, need and a line, rendered
from tables held to the decoders. jig decodes its engine part (temper's
`engine`) and its worker part (each of `workers`). Give jig a table of
the shapes its decoders read, a row per shape and a row per member of
each, in a row type temper's own table will share, and step tests that
hold it to the decoders: each shape's object generated from its rows
decodes with that shape's decoder, a required member left out is
refused, an unknown one is refused, and jig's whole fixtures walk
through the table. The lines stay empty here: the owner writes them.

Read first: temper's `docs/design/protocol/documentation.md` 8 (row
`configuration` and its bullet); jig's
`docs/design/protocol/configuration.md` 3 and 4; the survey
`~/src/rust/tmp/owners/docs-reference/reports/s3.md` sections 1 (the
decoders' members and their order), 3 (recurring shapes) and 2's
`engine` and `workers` rows (types, bounds, needs; its aliases `j`,
`jo`, `jl`, `jv`, `jh`, `jad`, `jc`, `jcp`, `jd`, `ja`, `jp`, `jt`, `jw`
are jig's `crates/jig-protocol-config/src/` files); jig's
`crates/jig-protocol-config/src/read.rs` and `tests/fixtures.rs`.

## Where

- Worktree `~/src/rust/tmp/owners/docs-reference/worktrees/jig-members`,
  jig, on branch `docs-reference/members`, made from
  `docs-reference/names` (it builds on `names`). Commit there; do not
  push.
- Files: `crates/jig-protocol-config/src/{lib.rs, members.rs (new),
  tests.rs, tests/members.rs (new)}`; a decoder function made
  `pub(crate)` where the test needs to call it, nothing more.
- Log: `~/src/rust/tmp/owners/docs-reference/logs/t5.log`.
- Report: `~/src/rust/tmp/owners/docs-reference/reports/t5.md`.

## The outer layer

`crates/jig-protocol-config/src/members.rs`, `pub mod members;`:

```rust
//! The configuration document's members, a row per shape the decoders
//! read and a row per member of each (configuration.md, 3 and 4).

/// An object the decoders read, or a choice among forms.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Shape {
    /// Its heading on the page: `address`, `llm_provider`.
    pub name: &'static str,
    pub form: Form,
    /// What it is, one sentence; documentation text, the owner's.
    pub line: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Form {
    /// An object holding its members.
    Object,
    /// Exactly one of its members, or one of these words in its place.
    One { words: &'static [&'static str] },
}

/// A member of a shape.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Member {
    pub shape: &'static str,
    pub name: &'static str,
    pub value: Value,
    /// Some: a JSON list of `value`, at most this many.
    pub list: Option<Bound>,
    pub need: Need,
    /// What it does, one sentence; documentation text, the owner's.
    pub line: &'static str,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Need {
    /// Present, never null.
    Required,
    /// Present, null allowed.
    Nullable,
    /// May be left out; its line says what is taken then.
    Optional,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Value {
    /// An unsigned integer of this many bits.
    Integer { bits: u8 },
    /// A duration in whole milliseconds, its name ending `_ms`.
    Milliseconds,
    Boolean,
    /// UTF-8 text, at most this many bytes.
    Text(Bound),
    /// A path, relative to this directory or absolute.
    Path(Directory),
    /// One of these words.
    Words(&'static [&'static str]),
    /// An object of this shape, by its name, in this table or the application's.
    Shape(&'static str),
    /// The overrides' map, a limit's dotted name to its value, whose
    /// names the limits' pages hold.
    Overrides,
    /// Its form is documented elsewhere; its line says where.
    Elsewhere,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bound {
    Most(u32),
    /// A named limit's value: `link.name`.
    Limit(&'static str),
    /// The document's own size.
    Document,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Directory { Config, Data, Cache, Absolute }

/// jig's shapes, in the order the page gives them.
pub const SHAPES: [Shape; N];
/// Their members, grouped by shape in `SHAPES`' order, each shape's in
/// its decoder's order.
pub const MEMBERS: [Member; M];

/// The members of one shape.
#[must_use]
pub fn members(shape: &str) -> &'static [Member];
```

These types are generic: an application's own table (temper's) uses
them too, and its `Value::Shape` may name one of jig's shapes. No
closure, trait, generic item or hash map.

## The shapes

Names, forms and decoders, in this order (the page's); members as the
decoder reads them, in its order (S3 section 1):

| Shape | Form | Decoder |
|---|---|---|
| `engine` | Object | `lib.rs` `engine` (with `defaults::offers`' `charters`, `templates`) |
| `limits` | Object | `overrides::parse` |
| `deployment` | Object | `domain::deployment` |
| `identity_provider` | Object | `domain::providers`' element |
| `owner` | Object | `domain::owners`' element |
| `act` | Object | `acts::grant` |
| `llm` | Object | `llm::parse` |
| `endpoint` | Object | `llm::endpoint`, its flat transport members (`transport`, `server_name`, `trust`) among its own |
| `header` | Object | `llm::headers`' element |
| `model` | Object | `llm::model` |
| `prices` | Object | `llm::price` |
| `llm_provider` | Object | `llm/providers.rs` `provider` |
| `sources` | Object | `sources` |
| `registration` | Object | `registration`, its flat transport members among its own |
| `hosts` | Object | `hosts::parse` |
| `host` | Object | `hosts::worker` |
| `address` | One, no words | `address::read` (`socket`, `host` with `port`, `in_process`) |
| `clients` | Object | `clients::parse` |
| `openid_provider` | Object | `clients/provider.rs` `parse` |
| `openid_client` | Object | `clients/provider.rs` `client` |
| `project` | Object | `projects::projects`' element |
| `permission` | Object | `projects::permissions`' element |
| `policy` | Object | `projects::policy` |
| `role` | Object | `projects::roles`' element |
| `authority` | Object | `projects::value` |
| `budget` | Object | `projects::budget` |
| `delegation` | Object | `projects::delegation` |
| `delegate` | Object | `delegation`'s kinds' element |
| `grant` | Object | `projects::grants`' element |
| `resource` | Object | `projects::scopes`' element |
| `pattern` | Object | `projects::pattern` |
| `requirement` | Object | `projects::requirement` |
| `judge` | Object | `projects::judge` |
| `template_override` | Object | `templates::overrides`' element |
| `executor` | One, words `chosen` | `templates::executor` |
| `template_budget` | One, words `given` | `templates::budget` |
| `deadline` | One, words `given`, `none` | `templates::deadline` |
| `priority` | Object | `templates::priority` |
| `worker` | Object | `worker::worker` |
| `graces` | Object | `worker::parse_graces` |
| `programs` | Object | `worker`'s `programs` |
| `agent` | Object | `worker::parse_agent` |

A member whose value is a further object not listed (for example
authority's `models`, a requirement's `guard`, a source's `programs`)
is a shape of its own, named after the member, after its parent in the
order: add it and say so in the report. A member read through a
shared function (an address, a pattern) names that shape.

**The engine's four settings members** `rules`, `policies`, `charters`
and `templates` (the document's form of the settings, which the owner's
page documents elsewhere): `Value::Elsewhere`, `Need::Optional`, no
shape of their own. Their inner shapes (`charter`, `template`, `rules`,
an entry of `policies`) get no rows; `policy`, `role`, `authority` and
the rest that `project` reaches do.

**A member's row** follows the decoder: `u8`/`u16`/`u32`/`u64` reads
are `Integer { bits }`; `_ms` durations `Milliseconds`; booleans
`Boolean`; text reads `Text(Bound::Most(n))` or `Text(Bound::Limit(name))`
where the decoder checks it against a named limit's value; typed paths
`Path(...)` by their directory (configuration.md 12); a match on strings
`Words` (each as the decoder spells it); a nested object `Shape`; a list
sets `list` with its bound (`Most`, `Limit`, or `Document` where only
the document's size bounds it) and `value` the element's; the limits'
`overrides` map `Value::Overrides`. `need`: `Required`; `Nullable` where the decoder
takes `null`; `Optional` where it may be left out (S3's `O(...)`); a
need that depends on another member's form is the base need, its rule
the owner's line. Every `line`, a shape's and a member's, is `""`.

## The tests

`crates/jig-protocol-config/src/tests/members.rs`, testing-strategy.md
10's shape:

- `every_shape_has_its_members_and_every_reference_resolves`: shape
  names unique; members grouped in `SHAPES`' order, no member name twice
  in a shape; every `Value::Shape` names one of `SHAPES`; every shape
  reached from `engine` or `worker`; every `Bound::Limit` is a row of
  `names::NAMES`; a `One` form has at least two alternatives, members
  and words together.
- `every_generated_object_decodes`: for each shape, generate its object
  from the table alone (an integer `1`, milliseconds `1`, `false`, text
  `"x"`, a path `"x"`, the first word, a list of one, a nested shape's
  generated object, an overrides map `{}`, an
  `Elsewhere` member left out; a `One` form's first member, then one
  object per other member and per word), decode it with that shape's
  decoder (a match in the test from the shape's name to its decoder
  call, given jig's small profile and the fixtures' values where a
  decoder needs more), and assert that no problem is `Unknown`,
  `Missing`, `Duplicate` or `Kind`. Value problems (`Bound`, `Form`,
  `Refers`, ...) are a generated value's, not the table's: ignore them.
  This proves every row a member the decoder takes, of the type the row
  says.
- `a_required_member_left_out_is_refused_naming_it`: for every
  `Required` and `Nullable` member of an `Object` shape, its shape's
  generated object without it gives `Missing` at its path.
- `an_unknown_member_is_refused`: each `Object` shape's generated
  object with one more member gives `Unknown` at its path.
- `the_fixtures_walk_through_the_table`: `ENGINE_SMALL` and
  `WORKER_SMALL` (`tests/fixtures.rs`), walked from `engine` and
  `worker`: every member they hold is a row of the shape at that point,
  its JSON kind the row's (an object for a shape, a list where `list` is
  some, a number, a string, a boolean, null only where `Nullable`).
- `each_line_is_one_line_within_its_bound`: a shape's line at most 200
  bytes, a member's at most 160, no line end (empty passes here).

A decoder that cannot be called on one object alone (it needs a whole
engine, say) is a `Question:` naming it and what it needs; do not
change a decoder's behaviour to make it callable.

## Acceptance

- `heavy cargo nextest run -p jig-protocol-config --lib` passes; report
  the count, the new tests' time (well under a second) and the row
  counts by shape.
- `heavy cargo clippy -p jig-protocol-config --all-targets -- -D
  warnings` is clean.
- The focused suite is waived (common.md).
- The report lists the commits, every shape you added beyond the table
  above, every decoder made `pub(crate)`, and every row whose type or
  need you were unsure of, with the decoder line that decided it.
- Remove the worktree's `target/` when done.
