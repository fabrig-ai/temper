# The welcome's walk on fake backends

Built 2026-10-25, under the user's `draft/fake-backends.md`, which holds
the purpose and the rules: the clicks work and the welcome reaches its
end, skein ships primitives and temper wires them, no fake branches in
product code, and Forgejo here, Forgejo elsewhere and programs keep
their real mechanism. This document is how the walk is built: what
`temper box --fake-backends` gives a person (section 1), how each
welcome path is proved, below the browser and clicked (sections 2 and
3), and what the walk found (section 4). Each path ends with a first
chat whose committed reply is the fake's (the user's brief, 10-24).

## 1. The fake box

`protocol/configuration.md`, 12.1, and `protocol/setup.md`, 7.3, 7.5 and
8.4, are the contract; in short:

- **The flag.** `temper box --fake-backends [--bundle DIR]` keeps a
  private sandbox, `DATA/box/fake-backends`, whose effective `temper.json`
  and `agent.json` are rewritten onto the fakes. The source configuration
  is left untouched, and init runs in the sandbox when it is empty. The
  bundle allows loopback navigation (`scripts/bundle.sh --loopback`).
- **Four supervised fakes,** always running, ahead of the playground's
  asked group, each on a listener the box holds before spawning it:
  - a GitHub (`temper fake forge`), with App 1 seeded and installed on
    `fake-person`, and repository `widget`;
  - the issuer (`temper fake oauth`), which grants every sign-in;
  - the model (`temper fake llm`), which accepts the key the box leaves
    and the issuer's token, and answers "Hello from the local fake model.";
  - a Forgejo elsewhere, forge 4, managed, its admin token in a private
    file.
- **A program,** Claude Code's, is the real mechanism: `agent.json`'s
  `programs.claude_code.path` is the `fake-claude` sibling of `temper`.
- **A forge's git** is its `git_program` (`protocol/configuration.md`):
  the box sets the fake git adapter for forges 2 and 4, since skein's
  fake forges speak a fake git protocol. Forgejo here keeps the
  machine's git.
- **What the box tells the person,** on stderr at start, each line a
  fixed prefix that the lab captures:
  - the effective directories;
  - each fake's origin (`box: fake github http://…`, `issuer`, `model`,
    `forgejo`);
  - the model key's file and Forgejo's admin token file;
  - the program-login command with the effective directories, ending
    `NAME (the name the Model step shows)`.

## 2. Below the browser: process stories

`crates/temper/tests/fake_backends/walk.rs`. A `Walk` is one fake box on
an empty configuration, as a person starts it. Its `Act`s are what the
welcome's buttons send, through `temper ctl`. `Act::Visit` is the tab:
it follows the address an act printed through redirects and a page's
self-posting form, as a browser would, back to temper. Each story pairs
one new path with one already proved, so that a failure names one thing:

| Story | Forge | Model |
|---|---|---|
| `forgejo_elsewhere_and_a_key_…` | Forgejo elsewhere, by its admin token, its first repository made | a key |
| `github_one_click_and_a_key_…` | GitHub, temper's App by manifest | a key |
| `githubs_own_app_and_a_key_…` | GitHub, an App of the person's own | a key |
| `chatgpt_sign_in_and_github_one_click_…` | GitHub one-click | ChatGPT sign-in |
| `forgejo_here_and_a_key_…` | Forgejo here | a key |
| `a_program_and_forgejo_here_reach_ready_…` | Forgejo here | a program's login, to Ready |

Each story asserts what the welcome shows before its new path. It then
asserts the end: the forge connected with its first repository adopted,
the account usable, and the first chat's committed assistant reply. The
stories run in the process tier. The ones that start a real Forgejo take
the owned stories' allowance.

## 3. Clicked: lab experiments

The proof that the clicks work is skein's lab: hand drives Chromium
(skein's `lab-browser.md`, 8) against an ordinary box. hand reaches
temper's origin and the fakes the box names as its peers (`--peer`,
8.5). The ChatGPT walk also names the app-owned fixed callback origin,
`http://127.0.0.1:1455`, which its shipped registration preserves. Every
other destination stays refused. Four experiments cover
the seven paths, all in the browser:

| Experiment | Forge | Model |
|---|---|---|
| `lab/welcome-chatgpt` | Forgejo elsewhere | ChatGPT sign-in |
| `lab/welcome-github` | GitHub one-click | a key |
| `lab/welcome-own-app` | GitHub, an App of the person's own | a program's login, to Ready |
| `lab/welcome-forgejo-here` | Forgejo here | a key |

After opening the printed link, the hand waits for the visible Sign out
button before navigating: the link's page must finish signing in. Each
walks `/welcome` to Ready and opens the project. It types a first chat
and presses Start chat, waits for the reply, and finds it again after a
reload. Its
checks require hand's trouble to be empty, and ctl's chat to hold the
expected reply as its first committed assistant message. Each welcome
step is its own phase of the walk, so the look's live scenes
(`draft/qa-web.md`, 3.3) can capture between them.

## 4. What the walk found

| Found | Cause | Fix |
|---|---|---|
| GitHub's Create → install → return never completed in Chromium (10-23, 10-24, 10-25) | first, the browser fixture's fake GitHub admitted two connections for a chain of three: Chromium's keep-alive, temper's forge client and its setup client. Then temper answered GitHub's empty redirect as a body of length zero and never finished it, so the setup request Chromium sent on that kept-alive connection was never read | the fixture sized to the chain's clients; temper-service's GitHub client sends an empty answer with no body (a service regression pipelines the next visit). The box's fake GitHub already admits eight |
| Forgejo elsewhere's first repository refused at adoption (`no_access`) | skein's fake Forgejo listed the setup admin among a new repository's collaborators. A real Forgejo 17, probed for the record, lists none until a grant | skein's fake, as Forgejo answers |
| the first chat's worker never replied on a fake forge | workers ran the machine's git against a fake git protocol | a forge's `git_program`, a general setting |
| the printed program-login line named a source `source` | the welcome declares `claude-` + the person's name, which the box can't know when it prints | the line ends with the placeholder |
| Ready on a Forgejo elsewhere said "Almost ready" after adoption | `ready_target` admitted GitHub and owned Forgejo only; the client README's Ready names no such limit | the guard removed |
| a manifest's new App inherited the seeded App's installation, so setup answered 410 | skein's fake GitHub kept the old App's installations and tokens when a conversion replaced the App | skein's fake: a conversion replaces the App whole |
| the own App's installation stayed stray | a seeded installation isn't an accepted one, so the own App takes GitHub's install hop, and the fake's install page refused an App with no setup URL | skein's fake: a seeded App takes a setup URL; the box sets it, and leaves the App's private key in a file it names |
| a program's account was usable but had no model | the box offered `claude_code` with no model endpoint behind it | the box's documents route model `fake-claude` to the program |
| after "Connect as its admin" made and adopted the first repository, the welcome stayed on Forge | its advance knew only owned set-up's answer; the client README moves on to Model once any first repository is adopted | the advance follows the adoption, whatever the forge |
| the own App's pasted key was refused as malformed | hand typed the key's line endings into the one-line field as spaces, past the parser's 1196-byte bound; a person's paste drops them | hand drops line endings typed into a one-line field |
| a key walk never saw the welcome's Ready checklist | as designed: only a sign-in returns to it; an account made on the Model step shows `ConnectModel`'s Ready, whose "Start a chat" is the way on | none: the walks follow it |

On main since 10-25 22:33Z (set 1: skein `ab7b536c`, temper `3064e597`):
the fake box's wiring, every process story, `lab/welcome-chatgpt`, and
every fix above but these, which land with set 2: the welcome's advance
to Model after any first adoption (a Forgejo elsewhere's stayed on
Forge), and hand's typing into a one-line field, which turned a pasted
key's line endings into spaces where a person's paste drops them.

Open, for their owners:
- **A program can't take a first chat.** jig refuses a program model for
  a requester-ended task (`jig-domain-core/src/run_choice.rs`, a
  deliberate gap), and ctl has no goal tool yet (`client/tools.md`, area
  2). So a person who connects their subscription reaches Ready, and
  their first chat is refused.
- jig's `accounts.usable` is true once a program's source is declared,
  before its login, while the worker's program is still not signed in.
- temper's setup response waits on a GitHub API call through refused
  connections with doubling backoff, 11.4 s in one record, so a
  person's tab hangs with no answer while GitHub's API is unreachable.
- skein-http treats a zero-length body as a stream that must be
  finished. Any empty answer sent that way stalls a kept-alive
  connection, as temper's GitHub client did.
