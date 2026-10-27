# Porting our process to temper

Draft, 2026-10-27, from a session with the user. Nothing here is
adopted. It covers three things:

- how the process that runs temper's own development maps onto
  `process-model.md`. The process is agents' roles and designs as of
  2026-10-26.
- what stays outside temper;
- the order in which the process can move onto temper, step by step.

`reference-case.md` is the same process, invented and condensed.
Section 7 maps the real episodes onto its steps. Once temper has the
model's gaps, this draft becomes the migration guide. It names
agents' documents, which are private, but no machine, account or
budget.

## 1. In one page

- **Almost everything has a home in the model.** Each role, tool and
  convention of 10-26 maps onto it (section 3):
  - **roles:** the director is the person's chat, designers are
    coordinator goals, owners are goals, and implementers are delegated
    agents;
  - **landing tools:** `land-set` becomes landings with prepare and
    receipts, and the after-landing runner becomes tiers and verdicts;
  - **qa:** QA after landing and findings;
  - **the user's issues:** questions to the person, shown on the forge;
  - **pauses and holds:** holds with owners.
- **Some things stay outside** (section 4): building and its queue,
  caches, disks, machines, the custody of logins, and the public
  mirrors. temper's workers and the forge's runners sit on top of them,
  as gates sit on top of them today.
- **Some behaviour changes** (section 5):
  - no gate runs while landings are locked out;
  - receipts are shared by the whole project, not kept per machine;
  - a reservation replaces the director's hand-made hold;
  - any goal may ask the person;
  - holds are owned, so only the person lifts the person's pause.
- **Seven steps, from the bottom of the tree up** (section 6). Each
  step works alone and can be undone:
  0. gates as dispatch-only workflows;
  1. temper lands, for owners still outside it;
  2. findings and QA;
  3. implementers;
  4. owners;
  5. designers;
  6. the director.

  The mains and the forge are shared ground at every step, so the old
  tools and temper can run side by side.

## 2. The process, 10-26

Pointers, not a restatement:
- **The roles** are agents' `roles/` (director, designer, owners,
  implementer, qa).
- **Landing** is `docs/design/landing-flow.md`:
  - sets, and the set lock as the landing turn;
  - after landing as its step 8;
  - what a landing taught as its section 12.
- **Proof** is `docs/design/landing-proof.md` (prove, then land, with
  the gate cache as receipts), together with `gate-policy.md`.
- **QA and findings** are `docs/design/qa-flow.md`.
- **The user's waits** are `docs/design/user-issues.md`.
- **Messages** are `docs/design/post.md`, and **the record** is
  `docs/design/records.md`.

## 3. The encoding

| Today | In temper | Model | Gaps |
|---|---|---|---|
| The director, the user's chat | the person's chat, under the coordinators' charter; a timer for its 2-hour check | 3.1 | exact |
| A designer | a coordinator goal; its design parts as changes into `docs/` | 3.1 | exact |
| An owner and its mission | a goal: the brief is the mission's, its notes are the mission's notes, its result is `closing.md` | 3.1 | exact |
| An owner without a designer (a fix) | a goal the chat makes, from a finding or not | 3.1, 6.4 | exact |
| Implementers through `codex-run`; reviews; advisors | agent delegates under GPT charters; a one-shot under a Claude charter | 3.1 | exact |
| "Codex first"; "every new feature gets a designer" | instructions in the coordinators' charter | 3.1 | exact |
| `roles/*.md`, changed often | charters' instructions; a change is a new revision that live tasks move to | 3.1 | exact |
| `post` between parents and children | messages, results and news | 3.1 | exact |
| `records/`: missions, decisions | tasks' notes, results and histories; the chat's transcript | 3.1 | sugar for an ordered decision log |
| A `waiting-on-user` issue, opened and closed by the asker | a question to the person, projected as an issue, answered by comment | 3.2, 9 | T14 |
| The director triaging questions that are the user's | gone: any goal asks the person; questions to the chat stay with it | 3.2 | J7, J8 |
| An answer after its owner closed, routed to the parent | to the successor, or to the requester | 3.2, 3.4 | J6 |
| A mission paused, resumed only on an explicit yes | the person's stop; the chat proposes its release | 3.3 | J1 |
| An owner parked until the user's draft | park until a landing touches the draft | 3.3 | J4 |
| A global pause, a graceful shutdown, "HOLD, act on none" | project drain and project stop | 3.3 | J2, J3 |
| A running owner put under a new designer | move under a task | 3.4 | J5 |
| A closing note as the next mission's brief | a result, and a successor | 3.4 | J6 |
| A set (`set.json`) | a landing: `with`, dependencies, pin-only members | 4.2 | T1 |
| `land-set`'s rebase, repin and regenerated files | the project's `prepare` | 4.3 | T2 |
| The gate cache, keyed by tree, gate and policy, per machine | receipts, per project, mirrored as statuses | 4.4 | T3 |
| `scripts/check.sh` under `land` | a dispatch-only gate workflow that runs it | 4.5 | T4, B1, B4 |
| The set lock, held for seconds where no `main` moved | the turn, with no gate in it | 4.6 | T6 |
| `Gate-Tree:` on every landing | none needed: main's commit is the gated commit, and its statuses say so | 4.4, 4.6 | — |
| The director holding landings so that one set gets through | the reservation | 4.7 | T7 |
| `land --docs` | a landing whose gates ignore its paths: receipts, no run | 4.4 | T3 |
| `scripts/after-landing.sh` and its runner | tiers, dispatched on the newest main | 5.1 | T8 |
| The runner's rerun, bisect, "first failed it" | the verdict | 5.3 | T9 |
| `landings wait --cleared`, and its 2-hour exit | a change's result at cleared; the verification bound | 5.2 | T10 |
| A red on a landing whose owner is gone, to the director | a red finding, triaged | 5.4 | T9, T11 |
| `QA:` trailers; `qa decide`; the qa service's passes | marks; the chooser; the QA procedure and its tester | 7 | T12 |
| `records/qa/` findings, triaged by the director | findings as labelled issues, triaged by the chat | 6 | T11 |
| `Taught:` trailers in the feed | trailers in landing news; designers wake on them | 8 | T13 |
| `landings`, the status page | the client and `temper ctl` | 10 | T15 |

## 4. What stays outside temper

- **Building:** the build queue and its lanes, caches and their store,
  disk floors and cleanup. The forge's runners and temper's workers run
  `scripts/check.sh` as `land` does today, under the same queue.
  Whether a gate runs is temper's to say. How it runs is the plane's.
- **Machines:** setting them up, redeploying them and retiring them.
  temper sees a runner or a worker go away, and treats it as a fault
  (`process-model.md`, 4.5 and 5.3).
- **Logins:** one holder for each login that refreshes, and moving a
  login into a box and back out. temper reads the accounts it is given,
  as it does today.
- **The public mirrors:** keeping them in sync stays a job of its own.
  temper lands on the forge that adopts the repositories.
- **agents' designs:** they stay documents. Only `roles/` becomes
  charters.

## 5. What changes in how we work

- **No gate runs while landings are locked out.** Today, when a `main`
  moves under the set lock, the set gates again from the most upstream
  repository that moved (`landing-proof.md`, 1). Instead, the landing
  goes back to prepare, and the turn stays free for others.
- **Receipts are shared by the whole project.** A set proved anywhere
  lands from anywhere, and no landing is gated twice on the same tree.
- **The reservation is mechanical.** After two stale proofs a set gets
  the next turn, so the director no longer holds landings by hand.
- **Owners no longer write `set.json`.** A change says what it lands
  with, and prepare adds the pins.
- **Any goal asks the person.** The director no longer triages
  questions, and a question to the director stays with it.
- **Holds are owned.** A pause the user made is lifted only by the
  user. The director proposes, and the user says yes in the inbox.
- **Flake findings are filed by temper** at the verdict, rather than by
  the landing's owner.

## 6. The migration

Each step lists:
- **Moves:** what moves into temper;
- **Needs:** the gaps it needs, from `process-model.md` 12;
- **Retires:** what it retires;
- **Check:** how we know it holds;
- **Back:** how to undo it.

From step 1 on, temper runs as one deployment whose project adopts the
repositories on our forge, with workers on the build machines.

### 6.0 Gates as dispatch-only workflows

- **Moves:** nothing yet. Each repository gets dispatch-only workflows
  for `check.sh` and `after-landing.sh`, run on runners on the build
  machines under the build queue. `land-set` dispatches them, and reads
  their statuses as receipts.
- **Needs:** none of temper's. It needs runners on the build machines.
- **Retires:** the per-machine gate cache.
- **Check:** a set proved on one machine lands from another with no
  gate run.
- **Back:** `land-set`'s local gate path, kept behind a switch.

### 6.1 temper lands, for owners outside it

- **Moves:** the landing and after landing:
  - owners, still sessions, push their branches and hand them to
    temper as changes (`temper ctl`), naming what each lands with;
  - temper prepares with `scripts/devel/prepare`, which is `land-set`'s
    rebase, repin and regeneration, taken out of it;
  - temper proves by dispatch, takes the turn, runs the tiers, and
    gives the verdicts;
  - owners wait for a change's result at cleared.
- **Needs:** T1 to T10, T13, T15, B1, B4.
- **Retires:** `land-set` and `land`, the set lock, the after-landing
  runner, and the feed. `landings` becomes `temper ctl`.
- **Check:** a day of landings through temper, compared with 10-26.
  The things to compare are stops per landed set, verdict latency (46
  minutes at the median for temper's tier that day), and gates run per
  landing.
- **Back:** a project stop on temper's turns, and `land-set` again. The
  mains are plain git, so nothing is lost.

### 6.2 Findings and QA

- **Moves:**
  - `qa decide` becomes the chooser, which is the director, through
    `temper ctl`;
  - the qa service's passes become the QA procedure, with a GPT tester
    briefed by each product's `docs/design/qa.md`;
  - findings become labelled issues in the project's home.
- **Needs:** T11, T12.
- **Retires:** `qa watch`, `qa decide`, and `records/qa/`, which is
  kept as history.
- **Check:** a pass that temper starts on a cleared main, whose
  findings the director routes.
- **Back:** the qa tool reads the open finding issues back into its
  queue.

### 6.3 Implementers

- **Moves:** owners delegate implementers, reviews and advisors as
  temper tasks, under GPT and Claude charters. Each runs in a worker's
  checkout, and pushes to its change's branch.
- **Needs:** agent tasks requested by a party, with an owner outside
  temper acting as that party (exact). Spend is per task.
- **Retires:** `codex-run` and its run log.
- **Check:** one mission's implementers all run in temper, and its
  spend shows in temper.
- **Back:** `codex-run`, still installed.

### 6.4 Owners

- **Moves:** a mission becomes a goal under the owners' charter:
  - designers and the director, still outside, start it as parties;
  - its questions to the user become questions to the person, shown on
    the forge;
  - its closing note becomes its result.
- **Needs:** T14, J4, J6, and T10 for closing on clearing.
- **Retires:** owners' sessions, missions in `records/`, and post for
  owners.
- **Check:** one mission end to end as a goal, then every new one.
- **Back:** new missions start outside again, and goals in flight
  finish.

### 6.5 Designers

- **Moves:** designers become coordinator goals, with owners as their
  delegates. They wake on `Taught:` lines, park owners, and hand on to
  successors.
- **Needs:** J4, J5, J6, T13.
- **Retires:** designers' sessions, and post between designers and
  owners.
- **Check:** a feature from draft to its owners' ends inside temper.
- **Back:** as 6.4.

### 6.6 The director

- **Moves:**
  - the user's chat becomes the director;
  - it triages findings and chooses QA;
  - `roles/` become the coordinators' charter;
  - the user's pauses, drains and stops become owned holds.
- **Needs:** J1, J2, J3, J7, J8.
- **Retires:** `post`, the director's session, `records/` (frozen and
  readable), and the status page.
- **Check:** a day with the user in temper's client alone.
- **Back:** a fresh director session outside, briefed from the chat's
  notes.

## 7. Episodes and the case

The real episodes behind each step of `reference-case.md`, by
decision number, mission or design.

| Step | Episodes |
|---|---|
| 4.1 a design landed as Markdown | 1207, 1276, 1334; configuring-draft |
| 4.2 owners and their implementers | configuring-revisions; tier-flakes |
| 4.3 a landing across repositories, repaired, with `Taught:` | real-loop-speed k1; upgrades-6 m1; 1259; `landing-flow.md`, 12 |
| 4.4 the verdict, and a runner's fault | 1213, 1218; `landing-flow.md`, 4, step 8 |
| 5.1 a red, bisected, repaired with an advisor; inherited failures | 1213, 1226, 1231; upgrades-6 R1, R2; the false clearance, 1326 to 1328 |
| 5.2 a flake, and a fix owner without a designer | tier-flakes; 1334 |
| 5.3 QA after landing | 1233 to 1238; the passes of 10-26 |
| 5.4 a red after its owner ended | `landing-flow.md`, 4, step 8; 1316 |
| 6.1 upstream alone broke downstream | 1310, 1311 |
| 6.2 a chased set, and the director's hold | 1332, 1338; upgrades-5; playground-page |
| 6.3 conflicts in generated rows and in code | the sets of 10-24 to 10-26 |
| 7.1 a budget asked directly | agents#5; 1197, 1331 to 1333 |
| 7.2 the director answering, and asking the user | `user-issues.md`, 1 |
| 7.3 an owner parked until the user's draft | 1334; configuring-docs |
| 8.1 resumed only on an explicit yes | 1147 to 1151, 1247, 1251 |
| 8.2 a pause to roll out rules; a graceful shutdown | 1069, 1135, 1136, 1150 |
| 8.3 a plan acted on before its approval; "HOLD" | 1317 to 1319, 1325 |
| 9.1 every new feature gets a designer; an owner moved under one | 1334, 1335; github-pilot under design/ladder |
| 9.2 an answer after the close; a closing note as the next brief | 1260, 1279, 1283; github-land-2 to 3; upgrades-4 to 5 |
| 9.3 a mission closes when its last landing clears | 1193, 1213, 1218; `roles/owners.md`, 5 |

## 8. Open questions

- **Where this draft lives:** here, beside the model and the case, or
  in agents, beside the process it ports. It cites agents' private
  documents and the record's decisions.
- **Step 0 or not:** temper's workers can run the gates as checks
  instead, so step 0 is needed only if the forge's runners are
  preferred.
- **The order of steps 2 and 3:** findings and QA need little of the
  tree, and implementers need little of the landing, so either order
  works.
- **One project or several:** agents and qa are private, and the rest
  are public. Do they form one project with one turn, or two projects?
  Two projects would mean no landing spans both.
