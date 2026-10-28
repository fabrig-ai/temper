Read `~/src/rust/agents/roles/implementer.md` first; this task departs from it only where it says so: it is a read-only survey. You change no file, build nothing and run no test; your final message is the report, whole.

Then `~/src/rust/tmp/owners/docs-reference/common.md`, the mission's standing rules.

# S3: every member of temper's configuration document, and how each is decoded

## Goal

The owner will design one table of the configuration document's
members (`temper.json`, read by `temper serve`), beside its decoders,
each row a member with its path from the root, its type and bound, what
`temper init` writes, and a line; a step test will hold the table and
the decoders to each other (temper's `docs/design/protocol/documentation.md`
8, the row `configuration` and its bullet). Gather what that design
needs: every member every decoder takes, how each decoder is shaped,
and what `temper init` writes. Facts only, true of the code: temper at
the worktree's HEAD, jig at `~/src/rust/tmp/owners/docs-reference/clones/jig`
(temper's pin). Never a drafted line.

## Where

- Read in the worktree
  `~/src/rust/tmp/owners/docs-reference/worktrees/survey` (temper,
  detached; run `git -C` there only to read, never to move it), the jig
  clone above, and skein/smith at temper's pins under
  `~/.cargo/git/checkouts/` (read only).
- Start from temper's `docs/design/protocol/configuration.md` 2 to 6
  and 12, and jig's `docs/design/protocol/configuration.md` 3 and 4,
  for the members' intent; the code decides.
- The report is your final message.

## What to gather

**1. The decoders.** For each object of the document any decoder reads
(jig's engine part and its sections, jig's worker part, temper's
`application` and each of its members, the setup's entries, the demo,
the sign-in, the limits objects): the function and `path:line`; the
members it takes, each `required`, `optional` or `nullable`, in the
order the decoder reads them; how an unknown member is refused (the
reader's `finish`, or none); and the shape's choices (an object that is
one of several forms, such as an address `{"socket": ...}` or
`{"in_process": ...}`, a transport `"plaintext"` or `{"tls": ...}`).

**2. Each member, one line:**

```
PATH  TYPE  BOUND  NEED  INIT  DECODER
```

- PATH from the document's root, a list's element as `[]`:
  `engine.llm.models[].window`, `application.forges[].api`.
- TYPE as the decoder reads it: an integer (its width), milliseconds,
  bytes as a count, text (its byte bound), a path (configuration, data
  or cache directory, or absolute: configuration.md 12's typing), a
  boolean, one of words (list them), an object of a shape (name it), a
  list of a type (with its bound), a map (its key and value types).
- BOUND: the number or the limit's name the decoder checks it against
  (`text <= link.name`), or none.
- NEED: required, optional (its default when absent), nullable.
- INIT: what `crates/temper-init/template/temper.json` writes there
  (the value, or `placeholder NAME` for a `{{...}}`-style placeholder
  filled at init, or `—`).
- DECODER: `path:line`.

**3. Shapes that recur:** each object shape read at more than one path
(an authority, a pattern, an address, a transport, a budget, a
delegation...), with every path it is read at, and whether one decoder
function reads it everywhere.

**4. Members no decoder of `temper serve` takes,** but another command
does (`temper box`, `temper forge`, `temper open`, the setup shell), or
that `temper init` writes and nothing reads, each with evidence.

**5. The scripted demo:** `application.demo.llm`'s members (its
`asked`, `cued`, `otherwise` lists and their turns and lines): their
decoder, how deep the shape goes, and whether a call's `arguments` are
decoded by a schema or carried as JSON.

**6. How a table could drive or be held to them:** for each decoder,
whether its member names could be read from a table (the reader takes
a name per call: `object.member(document, b"name")`,
`read::required(...)`), and the existing tests that build or mutate a
whole document (fixtures, `temper init`'s document, the owned example
`docs/examples/owned.json`), with `path:line`, that a step test could
start from.

## The report

Your final message, Markdown: 1, 3, 4, 5 and 6 as above, then 2 grouped
by top-level member (`engine`, `workers`, `application`) in the
document's order. Its first line: `S3: N members` with the count of
section 2's lines.
