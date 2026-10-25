# The welcome on fake backends

Draft, 2026-10-24, from a session with the user; reshaped 10-25 as the
walk is built. `temper box --fake-backends` starts temper with every
backend the welcome needs pointed at fakes on loopback: a GitHub, the
ChatGPT sign-in's issuer, the model and a Forgejo elsewhere. The user or
a QA agent can then click through the welcome (Welcome, Forge, Model,
Ready) and send a first chat, with no real backend and no one's account.
It validates the welcome's flow and its look against
`client/mockups/welcome.html` and `connect-model.html`. Section 2 says
what is built, section 3 how the walk proves each path, section 4 what
the walk found, section 5 what stays real, and section 6 what is open.

## 1. In one page

- **Why.** The welcome's GitHub and ChatGPT steps reach real services,
  so walking them took the user, and audits went around them: gallery
  fixtures, hand-written issuers, split requests. Nobody had clicked one
  welcome from start to Ready against the shipped configuration.
- **The minimum, no more** (the user, 10-24): "the clicks work and the
  welcome reaches its end. Pre-canned choices are fine. No login
  pages." One fake person, one seeded repository, a sign-in that grants
  at once; no consent page, no fidelity beyond what the welcome's pages
  need. The fakes' own looks don't matter.
- **skein ships primitives, temper wires them** (the user, 10-24). The
  fakes are skein's libraries; the flag, the configuration, the
  processes and their supervision are temper's. `draft/qa-loop.md` 11.1
  holds: skein ships no fake executables. Where a fake answers unlike
  the real service, its fix is skein's, as the primitive's fidelity.
- **No fake branches in product code** (the user, 10-24). The flag
  changes configuration and the processes `temper box` supervises,
  nothing in the client, service or engine. What the product needs
  to be pointed at a fake is a general setting a person could use too
  (`git_program`, section 4).
- **The end is a first chat.** Each path reaches Ready (a connected
  forge with its first repository adopted, a usable model account),
  then a first chat whose committed reply is the fake's.

## 2. What is built

Steps 1 to 3 of 10-24's plan are on main (`protocol/configuration.md`,
12.1, the fake sandbox; `protocol/setup.md`, 7.3, 7.5 and 8.4):

- **The flag.** `temper box --fake-backends [--bundle DIR]` keeps a
  private sandbox, `DATA/box/fake-backends`, with effective `temper.json`
  and `agent.json` rewritten onto the fakes, a source configuration left
  untouched, and init run in the sandbox when the source is empty. It
  names a bundle that allows loopback navigation (`scripts/bundle.sh
  --loopback`).
- **Four supervised fakes,** always running, ahead of the playground's
  asked group, each on a listener the box holds before spawning it: a
  GitHub (`temper fake forge`; App 1 seeded and installed on
  `fake-person`, repository `widget`), the issuer (`temper fake oauth`;
  grants every sign-in), the model (`temper fake llm`; accepts the key
  the box leaves and the issuer's token, and answers "Hello from the
  local fake model."), and a Forgejo elsewhere (forge 4, managed, its
  admin token in a private file).
- **A program,** Claude Code's, is the real mechanism: `agent.json`'s
  `programs.claude_code.path` is the `fake-claude` sibling of the
  `temper` binary.
- **What the box tells the person,** on stderr at start, each line a
  fixed prefix the lab captures: the effective directories, each
  fake's origin (`box: fake github http://…`, `issuer`, `model`,
  `forgejo`), the model key's file, Forgejo's admin token file, and the
  program-login command with the effective directories. Being built:
  the own App's id and key file (section 6).

## 3. The walk

Each welcome path is proved twice: once below the browser, where its
wiring is cheapest to test, and once clicked, in the lab.

### 3.1 Below the browser: process stories

`crates/temper/tests/fake_backends/walk.rs`: a `Walk` is one fake box on
an empty configuration, as a person starts it. Its `Act`s are what the
welcome's buttons send, through `temper ctl`, and `Act::Visit` is the
tab: it follows the address an act printed through redirects and a
page's self-posting form, as a browser would, back to temper. Each
story pairs one new path with one already proved, so that a failure
names one thing:

| Story | Forge | Model |
|---|---|---|
| `forgejo_elsewhere_and_a_key_…` | Forgejo elsewhere, by its admin token, its first repository made | a key |
| `github_one_click_and_a_key_…` | GitHub, temper's App by manifest | a key |
| `githubs_own_app_and_a_key_…` | GitHub, an App of the person's own | a key |
| `chatgpt_sign_in_and_forgejo_elsewhere_…` | Forgejo elsewhere | ChatGPT sign-in |
| `forgejo_here_and_a_key_…` | Forgejo here | a key |
| `a_program_and_forgejo_here_…` | Forgejo here | a program's login |

Each asserts what the welcome shows before its new path, then the end:
the forge connected with its first repository adopted, the account
usable, and the first chat's committed assistant reply. They run in the
process tier. The owned stories take the owned stories' allowance, a real
Forgejo's start (section 5).

### 3.2 Clicked: lab experiments

The proof that the clicks work is skein's lab, with hand driving
Chromium (skein's `lab-browser.md`, 8) against an ordinary box. hand
reaches temper's origin and the fakes the box names as its peers
(`--peer`, 8.5); every other destination stays refused. Four
experiments cover the seven paths, all in the browser:

| Experiment | Forge | Model |
|---|---|---|
| `lab/welcome-chatgpt` | Forgejo elsewhere | ChatGPT sign-in |
| `lab/welcome-github` | GitHub one-click | a key |
| `lab/welcome-own-app` | GitHub, an App of the person's own | a program's login |
| `lab/welcome-forgejo-here` | Forgejo here | a key |

Each walks `/welcome` to Ready, opens the project, sends a first chat,
waits for the reply and finds it again after a reload; its checks
require hand's trouble to be empty and ctl's chat to hold the fake's
reply as its first committed assistant message. Each welcome step is
its own step of the walk, so the look's live scenes (`draft/qa-web.md`,
3.3) can capture between them.

## 4. What the walk found

As of 10-25 21:30Z the fixes are on their branches, landing with the
walk's first set.

| Found | Cause | Fix |
|---|---|---|
| GitHub's Create → install → return never completed in Chromium (10-23, 10-24, 10-25) | the browser fixture's fake GitHub admitted two connections. The chain has three: Chromium's keep-alive, temper's forge client and temper's setup client. Setup waited on a refused connection, so the navigation never ended | the fixture's admission sized to the chain's clients; the box's fake GitHub checked for the same |
| Forgejo elsewhere's first repository refused at adoption (`no_access`) | skein's fake Forgejo listed the setup admin among a new repository's collaborators; a real Forgejo 17 lists none until a grant | skein's fake, as Forgejo answers |
| the first chat's worker never replied on a fake forge | workers ran the machine's git; skein's fake forges speak a fake git protocol | `application.forges[].git_program`, a forge's git, general; the box sets the fake git adapter for forges 2 and 4 |
| the printed program-login line named a source `source` | the welcome declares `claude-` + the person's name, which the box can't know | the line ends `NAME (the name the Model step shows)` |
| Ready on a Forgejo elsewhere said "Almost ready" after adoption | `ready_target` admitted GitHub and owned Forgejo only; the client README's Ready names no such limit | the guard removed |

## 5. What stays real

These need neither a remote service nor the user, so they keep their
real mechanism rather than gaining fake modes (the user, 10-24):

- **Forgejo here** runs a real Forgejo on the machine, fetched once from
  Codeberg and cached at temper's pins. A walk with no network has the
  pinned binaries in its cache already, as a person's has after a first
  run; its start costs seconds, not the 2 s a process story aims at.
- **Forgejo elsewhere** reaches a fake Forgejo through the generated
  configuration: a managed forge on `temper fake forge --document`, its
  admin token in a file the box names. The welcome does what it does
  for a forge configured by hand.
- **A program** (Claude Code) is the real mechanism: whoever walks runs
  the printed `temper program-login` once, as a person would.

## 6. Open

- **The own App.** The fake's install page refuses an App with no setup
  URL, and only a manifest sets one. If temper's own-App flow sends
  the tab to GitHub's install page for an App already installed, the
  seeded App needs its setup URL, as its owner set it on GitHub, a
  skein primitive. Being settled.
- **Setup waits on GitHub.** temper's setup response waits on a GitHub
  API call through refused connections with backoff, so a person's tab
  hangs with no answer while GitHub's API is unreachable. A finding for
  the setup's owner, not this walk's.
- **Whether QA's look adopts these experiments** for its live scenes in
  place of the gallery's welcome fixtures: `draft/qa-web.md` decides.
