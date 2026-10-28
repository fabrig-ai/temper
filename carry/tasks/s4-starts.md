Read `~/src/rust/agents/roles/implementer.md` first; this task departs from it only where it says so: it is a read-only survey. You change no file, build nothing and run no test; your final message is the report, whole.

Then `~/src/rust/tmp/owners/docs-reference/common.md`, the mission's standing rules.

# S4: what a later start does with each member of temper's configuration document

## Goal

The page `configuration` will say what a change to each member does.
The design says every member is read at a start, so a change needs a
restart; but some members seed the store at the first start (projects,
owners, the settings' document defaults), and a later start may apply,
ignore or refuse a change to them. Find, true of the code, what a start
after the first does when each member below differs from the first
start's. Facts only, with `path:line`: temper at the worktree's HEAD,
jig at `~/src/rust/tmp/owners/docs-reference/clones/jig`.

## Where

- Read in `~/src/rust/tmp/owners/docs-reference/worktrees/survey`
  (temper, detached), the jig clone, skein/smith pins under
  `~/.cargo/git/checkouts/` (read only). The member inventory is
  `~/src/rust/tmp/owners/docs-reference/reports/s3.md` (its decoders'
  aliases at its top).
- Start from temper's `docs/design/protocol/configuration.md` 8 (first
  start and every start) and 13 (settings in the store), and jig's
  `docs/design/protocol/configuration.md` 8.
- The report is your final message.

## For each member below

`engine.deployment` (`directory`, `seed`), `engine.providers`,
`engine.owners`, `engine.acts`, `engine.connectors`, `engine.projects`
(each seed: `number`, `name`, `policy`, `roles`), `engine.policies`,
`engine.charters`, `engine.templates`, `engine.rules`, `engine.llm`
(providers, endpoints, models), `engine.hosts`, `engine.clients`,
`engine.memory_bytes`, `engine.limits`; `workers[]` (each member);
`application.forges`, `application.identities`, `application.landing`,
`application.charter`, `application.clients`, `application.webhooks`,
`application.setup`, `application.sign_in`, `application.demo`,
`application.limits`, `application.workers`.

One entry each:

```
### MEMBER
- first start: what the start does with it (seeds the store, configures
  a component, checks only), path:line
- a later start, changed: applied / ignored (the store's value wins) /
  refused (and the refusal's exact words) / partly (say which part),
  path:line
- with settings in the store: how the document's value and the store's
  settings relate (for the settings' document defaults and the seed
  projects' policies), path:line
- note: anything an operator changing it must know (a member whose
  change orphans records, renumbers, or needs another command first)
```

## Also

- **The settings' document defaults:** when `engine.charters`,
  `templates`, `policies` or `rules` are present, what the first start
  and a later start do with them, and whether the four must be all
  present or none; with the exact refusal when one is partial.
- **How the document's forms differ** from the settings file's (the
  page `settings-fields` on temper's branch `origin/docs-settings/fields`,
  `docs/help/reference/settings-fields.md`, in the clone
  `~/src/rust/tmp/owners/docs-reference/clones/temper`, read with
  `git -C ... show`): for each of charter, template, policy, rules,
  authority, grant, pattern, requirement, role, the members that differ
  in name, unit or form, one line each.

## The report

Your final message, Markdown: the entries in the order above, then the
two "Also" sections. Its first line: `S4: N members`.
