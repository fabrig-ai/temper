# QA of temper

Provisional, 2026-10-26, with the user. What a pass of QA after landing
(agents' `docs/design/qa-flow.md`) needs of temper: whom it plays, the
deployment it drives, the hands it holds, how it reads the forge and how
it checks an outcome. Who marks, chooses, starts and triages is
agents' `qa-flow.md`; the pass's steps and report are agents'
`roles/qa.md`. This document says only what those leave to the product,
and a pass reads it after its brief. The lab's binding it replaces is
parked as `draft/qa-lab.md`. What is still open is section 8.

## 1. In one page

- **A pass is one of temper's users:** an agent working with temper for
  a person. It has what any such agent has: temper's guide (`temper
  help`, `temper ctl help`), `temper ctl`, and the tools it chooses for
  the systems temper works with (section 2).
- **temper is agnostic about those tools.** It serves its own part, and
  names the rest by id and link; how a user reaches the forge is the
  user's choice (the user, 10-26). Our passes choose `curl` (section 5).
- **temper says what it is doing, why, and where; the forge shows what
  is there.** A change's step, hold and landing are temper's; its diff,
  reviews, CI logs and branches are the forge's (`client/forge.md`,
  section 1).
- **The deployment is a box,** on its fake backends first, then on real
  ones a rung at a time, as the brief names (section 3).
- **Three hands:** ctl as the signed-in person; the forge read as an
  observer, with reads alone; and the person on the forge, for what
  people do there. Never temper's bot or its tokens, its store, its HTTP
  API or a fake's internals (section 4).
- **The box hands each hand over,** printed as its other files are, so
  that a pass reads no design to set up (section 4).
- **A check compares two accounts,** temper's through ctl and the
  forge's through its API. A finding is temper not saying where to
  look, the two disagreeing, temper stale past its bound, a guide that
  does not get the pass there, or temper's words wrong; reading the
  forge is none (section 6).
- **What the design has and the code does not yet** is named once as
  blocked, never found again each pass (section 6).

## 2. Whom a pass plays

- **An agent working for a person,** with the person's credential,
  signed in by the box's printed link (`client/tools.md`, section 9).
  ctl is its tool for temper; what it uses for anything else is its
  own, as for any of temper's users.
- **temper's part, through ctl:** tasks, chats and their activity; a
  change's way to landing, its step in words, its hold, its place in a
  queue, its gates' verdicts and, landed, its merge commit, read with
  `change N`; the forges and their repositories. Each comes with the ids
  and links that find it on the forge (`client/tools.md`, 5.5, area 6).
- **The forge's part, on the forge:** a pull request's diff, files and
  reviews; CI's runs, jobs and logs; branches and their heads. A part
  temper does not serve says "Shown on the forge.", with the resource
  in the forge's words (`client/tools.md`, 5.2).
- **What temper owns on the forge changes through temper:** its
  branches, its pull requests and their landing. What a person does on
  the forge (a review, an approval at a head, a comment, a push to a
  branch of their own) they do there as themselves, and temper reads it
  (`client/forge.md`, sections 4 and 6).
- **temper's own agents are another user.** A chat or a coder reads the
  forge only through its host's read tools, within its authority, since
  a worker holds no forge credential (`domain/agent.md`, 4.3). A pass
  tests that as one of temper's features, as any other.

## 3. The deployment

- **A box in the pass's scratch directory,** started as
  `docs/guide/box.md` says, with the flags of the brief's rung, and
  stopped by the pass before it ends.
- **The rungs:**

  | Rung | Forges | Model | Accounts held |
  |---|---|---|---|
  | fakes | `--fake-backends` (`protocol/setup.md`, 7.5): GitHub and a Forgejo elsewhere faked; Forgejo here real, from the box's cache | the fake model, by the key file the box prints | none |
  | a real model | Forgejo here, set up through ctl (`../guide/box.md` 4); the box walked as `../guide/box.md` 2 to 7 | ChatGPT, a held login imported and exported back, or a key read from a private file (`client/tools.md`, 5.5, area 5) | the model's |
  | GitHub | GitHub, through an App kept for QA, on a repository kept for it | as the rung below | the App's key and its installation |

- **Down before up:** the director opens a rung once the rung below has
  run clean at a `main` no older, and a rung's open findings hold the
  rungs above it (`draft/qa-live.md`, 3.2, without the lab's verdicts).
- **Real accounts are the user's,** held on the machine `qa-flow.md`
  names, used by path and never read, and given back before the box
  stops: an imported login exported to its file. Their spend is
  `qa-flow.md`'s open question (its section 9).

## 4. The hands

| Hand | Who | For | Given as |
|---|---|---|---|
| ctl | the deployment's first owner, the person | every act on temper, and temper's account of it | the box's printed sign-in link, on stdin to `ctl sign-in -` |
| the observer | the forge's person, with reads alone: on Forgejo, `read:repository`, `read:issue`, `read:organization` and `read:user` | section 5's reads | a header file the box names |
| the person on the forge | the forge's person, with a token that writes | what a person does there, as a case's step written in its plan: a review, an approval at a head, a comment, a push, a branch moved under temper | a header file the box names |

- **Never:** temper's bot or its tokens, the preparation's
  `admin_token`, temper's store or its HTTP API, a fake's internals or
  its observations. A value the box prints for a person to paste (a
  model key, a fake's admin token, a PEM) reaches temper through ctl,
  as the welcome's paste does.
- **What the box prints** today: its sign-in link, and on fake backends
  the fakes' addresses and the files a person pastes
  (`docs/guide/box.md`, its last section). For the forge's two hands it
  is to print, for each forge it sets up or fakes, the forge's API and
  the person (section 7):

  ```text
  HH:MM:SSZ Forge 3 (Forgejo): api http://ADDR/api/v1
  HH:MM:SSZ Forge 3 (Forgejo): person LOGIN, read header FILE, write header FILE
  ```

  Each header file holds one line, `Authorization: token TOKEN`, private
  to the box's user, so that a pass passes a token by its path and never
  reads it.
- **Until it does,** on the box's Forgejo here, the pass signs in on
  Forgejo's web as the person with `person_password`, changes it as
  Forgejo asks (`protocol/setup.md`, 6.5), and makes the two tokens as a
  person makes them; on the fake Forgejo elsewhere, the admin token file
  the box prints serves both. It reports the minutes under Automation.

## 5. Reading the forge

Our passes read Forgejo with `curl`, the observer's header by its path:

```sh
curl -fsS -H @READ "$API/repos/OWNER/NAME/pulls/7"
```

| What | Read, under `API` |
|---|---|
| a change's pull request, by the number ctl prints | `GET /repos/O/R/pulls/N` |
| the pull request from a branch | `GET /repos/O/R/pulls/BASE/HEAD` |
| its reviews; its files | `GET /repos/O/R/pulls/N/reviews`; `GET /repos/O/R/pulls/N/files` |
| CI at a head, combined | `GET /repos/O/R/commits/SHA/status` |
| the Actions runs at a head | `GET /repos/O/R/actions/runs?head_sha=SHA` |
| a run's jobs; a job's log | `GET /repos/O/R/actions/runs/ID/jobs`; `GET /repos/O/R/actions/jobs/ID/logs` |
| a branch and its head | `GET /repos/O/R/branches/B` |
| a file at a commit | `GET /repos/O/R/raw/PATH?ref=SHA` |

- **Each read is one skein's fake forge serves,** decoded by
  skein-forge's Forgejo requests, so that a pass reads a fake as it
  reads Forgejo 17. A read the fake refuses is a finding against the
  fake, routed to skein's owner.
- **The person's writes** are Forgejo's API with the write header, as
  the case's plan names them; a pass writes only what a case needs.
- **GitHub's reads** come with its rung: GitHub's REST API by `curl`,
  this table in GitHub's terms, written then.

## 6. Checking an outcome

- **Each case, one loop:** act through ctl and wait on it
  (`client/tools.md`, 5.4); read temper's account; follow its ids and
  links to the forge; compare.
- **A case's line says both,** what temper said and what the forge
  showed, in agents' `roles/qa.md`, section 3's shape.
- **A finding is one of:**
  - **No way to the forge:** an outcome temper reports with no id or
    link to its place there, such as "Landed" with no pull request.
  - **The two disagree:** landed in temper while `main`'s head is not
    its merge commit; CI green in temper and red on the forge.
  - **temper stale past its bound:** the forge shows what a `wait` on
    temper does not see within the wait's timeout.
  - **The guide does not get there:** a step the pass could find only
    in the design or the source, or a refusal that does not name what
    is missing.
  - **temper's words wrong:** a state said that is not so.
- **Not a finding:** that the pass read the forge, or how.
- **Known gaps:** what the design has and `STATUS.md` lists as not yet
  built goes under Blocked, once, with what the pass did instead.

## 7. What it asks of temper

- **The box's forge hands** (section 4): for each forge it sets up or
  fakes, its API's address, the person's login, and a read and a write
  token as header files, printed with its other lines
  (`protocol/setup.md`, 7.3).
- **ctl's guide** in these terms: the forge read as the agent chooses,
  and what temper owns there changed only through temper
  (`client/tools.md`, 11.5).

## 8. Open questions

- **The GitHub rung's App:** made by the user for QA, or from temper's
  manifest. An installation that exists already is accepted with no
  browser, by `forge-accept F I` (`client/tools.md`, 5.5, area 6).
- **Real accounts' holding and spend:** `qa-flow.md`, section 9.
- **Holding section 5 to the fake:** a step test that decodes each read
  with skein-forge's Forgejo requests, as the guide's ctl lines are held
  to ctl's parser (`protocol/documentation.md`, section 1). Default:
  none, until a pass meets a read the fake refuses.
- **The browser:** a pass drives ctl; driving temper's pages waits with
  the lab (`draft/qa-web.md`).
