# Settings

Kind: concept

How work runs in a deployment: its charters, templates, policies and rules, which its owners change while it runs, one checked decision at a time.

## What it is

The settings are a deployment's charters, templates and policies, and
its rules. A setting is named by its kind and, but for the rules, its
number: `charter 2`, `template 1`, `policy 1`, `rules`.

- A charter is how an agent's run works: `temper help charter`.
- A template is how a new task is filled: `temper help template`.
- A policy is what a project made from it starts with, and the rules are
  the ceilings of every project: `temper help policy`.

The binary holds the defaults, and the store holds the changes over
them. The current settings are the defaults with the stored changes over
them: a setting no change names follows its default. The settings live
in the store, never in the configuration document.

A setting's value is whole: one JSON object, as `settings show` prints
it in JSON. Its members, with their types and bounds, are in
`temper help settings-fields`.

## How it changes

A change is a set of items, at most one per setting. Each item does one
of three things to its setting:

- sets it to a whole value, which replaces the value before it;
- removes it, so that it is no longer offered, its default included;
- resets it, erasing its change, so that it follows its default again.

The rules are set or reset, never removed.

A change names the newest revision its maker read. It makes one revision
(`temper help revision`), or it is refused whole and changes nothing.
Only the deployment's owners make a change. A party with a role in the
deployment or in a project proposes one instead, for an owner to decide
(`temper help proposal`).

A start whose binary has a new default for a setting that follows its
default makes a revision of its own. A stored change still wins over a
default that changed.

## What temper guarantees

- A change is checked whole before it commits: each value in its shape
  and within its bounds, each kind within its count, and every reference
  between settings, such as a template's charter or a charter's models.
- A refused change commits nothing. Its refusal names the item, its
  setting and its fault.
- A change against a revision that is no longer the newest is refused as
  stale, naming the newest. Read again, and decide again on what you
  read.
- A change that leaves every setting as it was is refused as unchanged.
- An accepted change takes effect without a restart.
- A change asked again under the same key gives its first outcome and
  makes no second revision.
- Each kind is read when it is used: a charter's at each run, from its
  task's revision (`temper help revision`); a template's when a task is
  made from it; a policy's when a project is made from it; the rules at
  every decision.

## Acting on it

- `temper ctl`: `settings show` reads the inventory (the newest
  revision, each setting and where its value comes from, the held
  revisions and the pending proposals) and, given a setting's kind and
  number, its whole value; `settings apply` makes a change; `settings
  propose` proposes one. Each verb's arguments are its help: `temper
  ctl help settings apply`.
- A settings file holds a change's items: its form is
  `temper help settings-fields`.
- A run whose charter names the settings tool family, and whose
  authority holds it, reads with the host tool `settings` and proposes
  with `propose_settings` (`temper help host-tools`).

## Limits

- temper does not patch a member: an item sets a whole value.
- temper does not hold its landing rules or the forge's charter as
  settings: `temper help landing-rule`.
- The person's client has no page to change settings.
- A value's bound in `temper help settings-fields` is what ctl sends;
  the deployment may refuse a value within it, past one of its own
  limits.
- The providers, models, forges, listeners, workers and limits are not
  settings: they are the configuration document's, read at a start, and
  a change to them needs a restart.
