# The welcome on fake backends

Draft, 2026-10-24, from a session with the user. `temper box
--fake-backends` starts temper with its GitHub, its ChatGPT sign-in and
its model pointed at fakes on loopback, so that the user or a QA agent
can click through the welcome (Welcome, Forge, Model, Ready) with no
real backend and no one's account. It serves one purpose: validating the
welcome's flow and its look against `client/mockups/welcome.html` and
`connect-model.html`. Section 2 says what exists, section 3 the work,
section 4 what stays real.

## 1. In one page

- **Why.** The welcome's GitHub and ChatGPT steps reach real services,
  so walking them takes the user. The mockup audits so far went around
  them: gallery fixtures, hand-written issuers, split requests
  (o/first-run-ux-3's walk, 10-23 and 10-24, 26 rows still differing).
  Nobody has clicked one welcome from start to Ready against the shipped
  configuration.
- **The minimum, no more** (the user, 10-24). The goal is clicks that
  work and a welcome that reaches its end. Pre-canned choices are fine:
  one fake account, one repository, a sign-in that grants at once. No
  login or consent pages, no device flow, no fidelity beyond what the
  welcome's pages need. Looks of the fakes don't matter.
- **skein ships primitives, temper wires them** (the user, 10-24). The
  protocol work is skein's and is already there; the flag, the
  configuration, the processes and their supervision are temper's.
  Predictable boilerplate in temper is fine. A piece moves to skein only
  once proven generic and wanted by another consumer, such as the
  factory's applications (`draft/factory.md`). `draft/qa-loop.md` 11.1
  holds: skein ships no fake executables.
- **No fake branches in product code.** The flag changes configuration
  and the processes `temper box` supervises, nothing in the client,
  service or engine. What the welcome does against the fakes is what it
  does against the real services.
- **Ready is the end.** The welcome completes on a connected forge and a
  usable model account (`temper-client-domain`'s `pages/welcome.rs`); a
  first chat's reply is outside this draft, though the fake model is
  wired so that one works.

## 2. What exists

- **skein's fakes, as libraries** (`skein-fake-peers`): the OAuth issuer
  (`skein-fake-oauth`: authorization code with PKCE, the issued client
  id the shipped ChatGPT registration uses, OpenID ID tokens and keys;
  its authorization answers with a redirect at once), the GitHub fake
  (`skein-fake-forge-github`: App manifest, creation and install pages,
  defaulting to the configured account and all repositories, and the API
  the setup uses), Forgejo's dialect, and the model's (public OpenAI
  Responses, which checks a bearer's bytes only).
- **temper's process wrappers:** `temper fake forge` and `temper fake
  llm` (`crates/temper-fake`, `crates/temper/src/fake.rs`), each run
  from a deployment or a `--document`. `temper box` supervises them
  today for the playground only (`temper-fake`'s `steps.rs`).
- **The configuration seam is proven, not shipped.** Every root the
  welcome reaches is a configuration value: a GitHub forge's `api`,
  `transport`, `git` and `web`; the sign-in's issuer, authorization,
  token, revocation and keys; the engine's and `agent.json`'s model
  endpoints, which are separate. `crates/temper/tests/support/sign_in.rs`
  rewrites the shipped ChatGPT registration onto a fake issuer and model
  this way; it is the recipe.
- **Missing:** the flag (`temper box` takes no arguments,
  `crates/temper/src/lib.rs`), an issuer process (no `temper fake
  oauth`), supervision of the fakes outside the playground, and one
  browser click from GitHub's Create to its install: the walkers' run
  timed out after Create while each request alone passed, undiagnosed
  (o/first-run-ux-3's `shots-gh/report.md` and `reports/m4-walk.md`, on
  hetzner5).

## 3. The work

1. **The flag and its configuration.** `temper box --fake-backends`
   writes an effective configuration, both `temper.json` and
   `agent.json`, beside the deployment's, and runs every child on it:
   children reopen their directory's documents, so rewriting the
   parent's bytes alone is not enough. It points GitHub forge 2 at the
   fake GitHub (plaintext, `git` and `web` on the same fake), the ChatGPT
   sign-in's five addresses at the fake issuer (plaintext, the shipped
   client, scopes, `must_grant`, resource and callback kept), and the
   OpenAI endpoint, in both documents, at the fake model. It names a
   bundle that allows loopback navigation (`scripts/bundle.sh`). With an
   empty configuration it runs `init` first, as `box` does.
2. **`temper fake oauth`.** A wrapper like its two siblings around
   skein's issuer peer, taking `--document` and `--ready`, granting
   every sign-in: the shipped registration's terms and scopes, an ID
   token for one fixed person, a fixed access token. The fake model
   accepts the same token bytes, so a chat after Ready works.
3. **Supervision.** `box` runs the fake GitHub (`temper fake forge
   --document`, seeded with one personal account and one repository,
   and with its App's id and key printed for the own-App path), the
   issuer and the model as always-running steps, not in the playground's
   asked group, which the first connected forge retires. Their prefix
   precedes every asked group's insertion point. The box holds each
   listener before spawn and hands it over as for the playground's fakes;
   the issuer requires the same held-listener support. Compatible retained
   documents and exact private seed outputs survive restart; peer memory
   lasts one launch. There is no second supervisor or self-bound readiness
   substitute.
4. **The walk.** Reproduce GitHub's Create, install and return as one
   browser click chain against the separate processes, and fix what
   breaks. Then walk each path to Ready in a browser: GitHub one-click,
   GitHub's own App, Forgejo here, Forgejo elsewhere; ChatGPT sign-in,
   a key, a program. The walk's proof is a lab experiment the look's
   scenes can use (`draft/qa-web.md`), and a test in the lowest world
   that owns each fix (`domain/testing.md`).

Each step lands alone; the first three are small, the fourth is the
risk.

## 4. What stays real

These need neither a remote service nor the user, so they keep their
real mechanism rather than gaining fake modes:

- **Forgejo here** runs a real Forgejo on the machine, fetched once from
  Codeberg and cached. Faking it means replacing the owned forge's
  steps while keeping its choice in the welcome; not worth it now.
- **Forgejo elsewhere** reaches a fake Forgejo through the generated
  configuration: a managed forge on `temper fake forge --document`,
  with its admin token printed. The welcome does what it does for a
  forge configured by hand, restart included; any shortcut would be a
  fake branch in product code.
- **A program** (Claude Code) is the real mechanism: `agent.json`'s
  `programs.claude_code.path` points at `tools/fake-claude`, and whoever
  walks runs `temper program-login NAME` once, as a person would.

## 5. Open

- Whether the GitHub fake's pages need anything for the own-App path
  beyond a seeded App with its setup callback (`skein-fake-forge-
  github`'s `pages.rs` refuses an install without one). If they do, that
  is skein's, and the one place this draft may touch it.
- Whether QA's look adopts this box for its live scenes in place of the
  gallery's welcome fixtures; `draft/qa-web.md` decides.
