# o/docs-reference: the standing rules

Every task of o/docs-reference names this file. Where a task says
otherwise, the task wins.

## The mission

temper's reference pages `limits` and `configuration`, each rendered
from one table that its parser or decoder is driven by or held to, so
that every limit's name and every member of the configuration document
is on a page with its unit, bound, value and meaning, true of the code
by construction. Pages live in `docs/help/`, compiled into the binary
by `temper-docs`, read with `temper help` and the runs' `read_docs`,
and held to the code by step tests. The design is temper's
`docs/design/protocol/documentation.md` 8 and
`docs/design/protocol/configuration.md` 2 to 7.

## Documentation text is the owner's

Every word of documentation text is the owner's: the pages in
`docs/help/`, a page's row in `temper-docs` (title, summary), every
row's line in a names table or a members table, and every fixed word a
renderer prints. An implementer writes none of it and drafts none:

- it copies the text a task gives, byte for byte, from the task or from
  the file the task names (`~/src/rust/tmp/owners/docs-reference/pages/`);
- where a task gives no text, the text is the empty string `""`: never
  draft, paraphrase or "improve" one, not even as a placeholder;
- a check that fails on the owner's text (a line over its bound, a page
  over its bound, a heading out of its template) is reported, never
  mended: the report's first line is a `Question:` naming the row and
  the check.

Rust doc comments (`///`, `//!`, `//`) are code, not documentation
text: write them as the surrounding code does.

## Pages change with behaviour

A landing that makes a page of `docs/help/` untrue (a command, a
refusal's words, an order, a default) updates that page in the same
landing, its text the owner's. A task that finds its change makes a
page untrue stops with a `Question:` naming the page and the line.

## Repositories, branches and worktrees

- **Worktrees** are under `~/src/rust/tmp/owners/docs-reference/worktrees/`,
  as the task names them, made from the mission's clones
  `~/src/rust/tmp/owners/docs-reference/clones/{temper,jig}`. Never a
  primary checkout (`~/src/rust/<repo>`), and no git command there, not
  even `git fetch`. The clones are for worktrees alone: no build and no
  cargo command runs in a clone.
- **Branches** are `docs-reference/<name>`, as the task names them, in
  temper and in jig. Commit there; push only when the task says so,
  never `main`. A rebased branch gets a new name; nothing is ever
  force-pushed.
- **Commits** keep their history: no squash, each message saying what
  changed and why, ending with `Co-Authored-By: Codex
  <noreply@openai.com>`.
- **Not yours:** `docs/design/` in either repository (the designer's:
  what the code taught goes in the report) and `docs/design/draft/`
  (the user's).
- **Files other owners hold:** temper's settings form
  (`crates/temper-settings-form/`, if present), `temper-client-tools`'
  rows and renderers, the stock (`crates/temper-service/src/stock.rs`),
  `temper-engine-smith`'s `SPECS`. Read them; change none unless the
  task says so.

## Building and testing

- Through `heavy`, as `roles/implementer.md` 2 and 3 say, one command
  at a time. Each worktree builds into its own `target/`; never share
  or symlink another's. At most two build targets of this mission on
  the machine at once: remove your worktree's `target/` when the task
  is done.
- **The disk:** before a build, `df -h /home` shows at least 50 GB
  free; if not, stop and ask.
- **No value moves.** No limit's number, no default, no bound and no
  budget changes, and no test is moved or ignored to pass. A refusal's
  words stay as they are unless the task gives new ones.
- **No stored format, codec, wire or version moves** (decision 877).
  Generated files (codecs, goldens, memory figures) are regenerated only
  through their own switch, never by hand; a declared memory figure that
  moves is a `Question:`.
- **The workspace focused suite is waived** in every task of this
  mission unless the task says otherwise: `land`'s gate runs it. Run the
  tests the task names, and clippy on the changed packages
  (implementer.md 3, step 2).
- **No suite gets slower** but by milliseconds: no new story in the
  real loop or the process tier.
- **A page lands with `land`, never `land --docs`:** `docs/help/` is
  compiled in and held by step tests.

## The code

- skein's `docs/foundation/programming-model.md` and
  `testing-strategy.md` are the rules (section 1 of each, and
  testing-strategy.md 10, "A test's shape", for every test you write).
  Step code has no closures, traits, generics, trait objects or hash
  maps of its own; jig defines no traits and no generic items in step
  code (jig's `AGENTS.md`). `crates/temper` (the binary's shell) and
  test modules follow their files' existing idiom.
- **jig names no system** (no forge, repository, branch, pull request,
  issue or git) and never names temper.
- **`temper-docs`** is `#![cfg_attr(not(test), no_std)]` with `alloc`,
  depends on `skein-lib` alone, decides nothing and holds no state. Its
  `PAGES` table is in this order: `index`, then the concepts, walks,
  recipes, reference pages and guides; the reference pages keep their
  order and a new one goes last among them.

## Who else is near

- **o/docs-map** and **o/docs-settings** land pages in `docs/help/` and
  rows in `temper-docs`; other owners change `temper-service`. Whoever
  lands second rebases, and a conflict keeps both sides' meaning.
