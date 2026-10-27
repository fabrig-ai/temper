# The reference case, second edition

Draft, 2026-10-27, from a session with the user. Nothing here is
adopted. One invented story that applies `process-model.md` to a
process like the one temper's own development runs. The story holds
every mechanic of the model that such a process uses, and none that it
doesn't. It is as small as that allows. temper must be able to run it.
The names are invented. `porting.md` maps the real process and its
episodes onto this case.

## 1. In one page

- **What it is:** the model at work in one project. Each step is
  something that happens, the model's sections that handle it, and what
  temper and jig lack for it.
- **The cast** (section 3):
  - the person, with the director as their chat;
  - D, a designer, and the owners under it: O1 and O2, then O3, O1's
    successor;
  - F, an owner of fixes;
  - P, an owner started before designers were the rule, and L, the
    designer it is moved under;
  - implementers, reviewers, an advisor and a tester, as delegates.

  There are two repositories, kit and app, which pins kit. There are two
  model plans, and one owned Forgejo.
- **Six acts, 20 steps, about 9 hours:**
  1. a feature designed and landed (section 4);
  2. after landing (section 5);
  3. landings that meet (section 6);
  4. the person (section 7);
  5. holds (section 8);
  6. the tree changes, and the ends (section 9).
- **What it asks** (section 11): every gap in `process-model.md` 12,
  except those the case leaves out (section 12).

## 2. How to read it

- **Each step** says what happens, then gives two lists:
  - **Model:** the sections of `process-model.md` that handle it;
  - **Needs:** what temper and jig must have, graded as that model's
    section 12 grades them: *exact*, *sugar*, *design* or *conflict*.
    Each item names its gap, for example J1 or T6.
- **Time** is in hours from the start. On fakes, timers and bounds are
  scaled down.
- **Run two ways:**
  - **On fakes,** with each judgement scripted as the oracle. The
    stories take the step's number: `fuzzy_case2_X_Y`.
  - **On real backends,** as v0.1.0's top rung. The person's words are
    a script that a QA driver types through `temper ctl`, and each step
    is checked by its effects.
- **The person acts directly or through the director.** Where both
  are possible, the step says which.

## 3. The cast and the start

- **The person** directs the work, sets budgets and priorities, and
  answers what is theirs to answer.
- **The director** is the person's chat, on Claude:
  - it coordinates under the coordinators' charter;
  - it is the project's triager and QA chooser;
  - a timer wakes it every 2 hours for its check.
- **D** is a coordinator goal on Claude: the designer of "Export",
  which spans kit and app. It designs, has each part reviewed once, and
  starts the owners.
- **O1** (Export's kit side) and **O2** (its app side) are goals on
  Claude under D. **O3** takes on what O1 leaves, in act 6.
- **F** is an owner of fixes, on Claude. The director starts it from a
  finding, with no designer, because a fix needs none.
- **P** owns "Themes" in app, on Claude. The director started it before
  the case, when features had no designer. **L** is Themes' designer,
  born in act 6.
- **The delegates:**
  - implementers, reviewers and the tester are agents on GPT;
  - an advisor is a one-shot agent on Claude, called once two tries
    have missed.
- **The repositories:**
  - **kit:** the upstream. Its gate is `check`. It has no tier.
  - **app:** pins kit's commit in its lock, and is the project's home.
    Its gate is `check`, and its tier `browser` runs after landing. Both
    ignore `docs/`. Its `STATUS.md` is generated from one file per part.
- **The tooling:**
  - `prepare` rebases each member onto its main, bumps app's lock to
    kit's candidate, and regenerates `STATUS.md`;
  - the key is the default one.
- **The forge:** an owned Forgejo with Actions. Every workflow is
  dispatch-only.
- **The settings** (`process-model.md`, 11):

  | Setting | Value |
  |---|---|
  | dependencies across repositories join a landing | yes |
  | app's pin kept at kit's main | yes |
  | stale proofs before a reservation | 2 |
  | several suspects | bisect |
  | a red pauses its repository | no |
  | verification bound | 2 hours |
  | the person's questions on the forge | on |

- **At hour 0:**
  - P is running on Themes;
  - nothing is held;
  - no finding is open;
  - app's `browser` tier is green at main.

## 4. Act 1: a feature designed and landed (hours 0 to 3)

### 4.1 The director starts D, and D's design lands

The person: "Export's draft is in; it's next." The director starts D at
priority 1, with the draft at its commit as its brief.

D writes the design of the boundary between kit and app, and a GPT
reviewer reads it once. D amends the design and asks for a change into
app's `docs/design/`.

The change touches only `docs/`, which both gates and the tier ignore.
So:
- its keys equal main's, whose receipts pass, and no run is dispatched;
- the turn fast-forwards app's main;
- the tier's key is unchanged too, so the landing is cleared at once.

D's change gives its result within a minute.

- **Model:** 3.1, 4.4, 4.5, 4.6, 5.1, 5.2.
- **Needs:**
  - a coordinator goal with a one-shot reviewer (exact);
  - receipts with ignored paths (T3);
  - dispatch-only gates (T4);
  - the turn (T6);
  - a tier judged by its receipt (T8);
  - a change that lives to cleared (T10).

### 4.2 D starts its owners

D starts O1 and O2, each with a brief: the design at its commit, and
the boundary where they meet. O1 delegates three implementers on GPT:
k1; k2, which depends on k1; and k3. k2 finds that its sketch doesn't
match main and asks O1, its requester. O1 answers, and k2 goes on.

- **Model:** 3.1.
- **Needs:** goals at depth three, dependencies between delegates, and
  a question to the requester (exact).

### 4.3 O1's first landing, across kit and app

k1, a change into kit, is ready. app pins kit, and the project keeps
the pin at kit's main, so prepare makes two candidates:
- kit's, which is k1 on main;
- app's, a pin-only member whose lock points at kit's candidate. It has
  a branch and a pull request of its own.

Both `check`s are dispatched. kit's fails on a real test.

A repair under O1's goal, an implementer given the failing output,
pushes a fix. Prepare runs again. Both keys changed (kit's tree, and
app's pin), so both gates run, and both pass.

The turn fast-forwards kit and then app. The forge marks both pull
requests merged, and both mains carry the gates' statuses.

k1's commit says `Taught: export ids must be stable across runs`. D's
wake policy names `Taught:`, so the landing wakes D, which amends its
design. That amendment lands as in 4.1.

- **Model:** 4.1 to 4.6, 8.
- **Needs:**
  - a landing with a pin-only member (T1);
  - prepare (T2);
  - receipts (T3), dispatch (T4);
  - the candidate on the member's branch (T5);
  - the turn (T6);
  - landing news with trailers (T13);
  - forge calls for dispatch, statuses and the fast-forward merge (B1,
    B4).

### 4.4 The verdict clears it

app's `browser` tier is running on an older main when k1 lands. When
that run ends, the next one takes the newest main, which holds k1's
landing and one of P's landings, p1. The run is lost when its runner
restarts. That is a fault, not a red: it is retried, and the retry is
green.

k1 and p1 are cleared. k1's change gives its result, and O1's wait for
it ends.

- **Model:** 5.1, 5.2, 5.3.
- **Needs:**
  - tiers following main, one run at a time on the newest tip (T8);
  - faults kept apart from reds (T9);
  - cleared as a change's result (T10).

## 5. Act 2: after landing (hours 3 to 5)

### 5.1 A red, bisected

One `browser` run covers three landings, in order: O2's a1, D's
second design landing, and P's p2. Two tests fail, and they fail again
on the rerun. temper bisects:
- the design landing changes nothing the tier reads, so it is skipped;
- the two tests fail at a1's commit, so they are a1's.

O2 hears it, with the tests and the log's end. Its repair:
1. an implementer tries twice and misses;
2. O2 calls a Claude advisor once, and it finds the cause;
3. the repair lands as a change that names a1.

Meanwhile P lands p3, because nothing is paused. The run after p3
shows the same two failures. They are inherited, so p3 is cleared.

The next run passes both tests. a1 is cleared, "repaired by" O2's
repair.

- **Model:** 5.3, 5.4.
- **Needs:**
  - the rerun, bisect over the landings, inherited failures, and
    per-landing verdicts (T9);
  - test ids from the run's artifacts (T8, B1);
  - a one-shot advisor on another model (exact).

### 5.2 A flake, and its fix

A `browser` run fails one test, and the rerun passes. The range is
cleared, and finding #7 is filed in app, labelled `finding`, `flake`,
`area:export` and `new`, with the test's signature.

At its 2-hour check, the director triages #7: it starts F, a goal
naming #7, with no designer.

Before F lands, O2's proof meets the same test failing at app's
`check`. The failure matches #7, so the failed job is rerun, and it
passes. O2 has no repair to make.

F's fix lands with `Fixes #7`. #7 stays open until that landing is
cleared (5.4).

- **Model:** 4.5, 5.3, 6.1 to 6.5.
- **Needs:**
  - findings as labelled issues, read back, and matched by signature
    (T11);
  - a rerun at a gate (T4, B1);
  - a goal made from a finding (exact).

### 5.3 QA after landing

k1 and a1 carry `QA:` trailers, and so does P's p2. At its check, the
director chooses k1's and a1's marks, and puts p2's in the backlog of
marks, since Themes comes after Export.

Once a cleared main holds both chosen marks, the QA procedure starts
one pass on the newest cleared main. The tester:
- works on GPT, with no forge effects;
- drives the product through its tool and reads the forge through its
  API;
- writes down what it expects before each case.

It files two findings:
- #9: Export's CSV header is wrong, on the happy path;
- #10: a page is slow, which is minor.

At the next check, the director routes #9 to O2, which hears it as
news, and #9 is labelled `routed`. #10 goes to the backlog.

- **Model:** 6.4, 7.
- **Needs:**
  - the QA procedure and the tester's charter (T12);
  - triage to a goal and to the backlog (T11).

### 5.4 A red after its goal has ended

`browser`'s runner goes down. Each run is a fault and is retried, and
after 15 minutes the director is told.

F's landing reaches the 2-hour bound with no verdict. F ends with its
change *landed, uncleared*, and says so in its result.

The runner comes back. The run is red, and bisect blames F's landing.
F has ended, so the red becomes finding #11, of kind `red`, naming the
landing and the tests.

The director routes #11 to O2, whose area it is. O2's repair lands
with `Fixes #11`. The next run is green:
- F's landing is cleared, repaired, and since it said `Fixes #7`, #7
  closes;
- O2's repair is cleared, and #11 closes.

- **Model:** 5.2, 5.4, 6.5.
- **Needs:**
  - the verification bound, and a late verdict (T10);
  - a red as a finding, and closing on clearing (T9, T11).

## 6. Act 3: landings that meet (hours 5 to 6)

### 6.1 One landing, two goals

O1's k4 renames a kit function that app calls, so app's build breaks
with it. O2 makes a3, the adaptation, and names k4 as `with`. O2 may
name k4 because D introduced its owners to each other.

One landing holds k4 in kit, and a3 in app with the pin stacked on it.
It proves, and lands kit, then app.

Both goals hear the landing. k4 is cleared as it lands, since kit has
no tier. a3 is cleared by app's next run.

- **Model:** 4.2, 4.3, 5.2.
- **Needs:**
  - `with` across goals (T1);
  - a repository with no tier clearing as it lands (T10).

### 6.2 Stale twice, then a reservation

O1's k5, with app's pin, takes 25 minutes to prove, because app's
`check` is slow.
1. Meanwhile P lands p4 into app. At the turn, app's base is stale.
   The landing prepares again. kit's main did not move, so kit's
   receipt stands, and only app's `check` runs.
2. P lands p5, and the proof is stale again. Now O1's landing holds the
   reservation for kit and app.
3. P's p6 proves in the meantime, but waits for the turn. O1's landing
   lands, and then p6 takes the turn, finds app moved, and prepares
   again.

Nobody holds anything by hand.

- **Model:** 4.6, 4.7, 4.8.
- **Needs:**
  - the turn's check of bases (T6);
  - the reservation (T7);
  - receipts kept across re-proofs (T3).

### 6.3 Conflicts, by rule and by resolution

O2's a4 and P's p7 each add a part to app, so each adds a row to
`STATUS.md`. p7 lands first. When a4 is prepared, the tooling
regenerates `STATUS.md`, and no conflict reaches anyone.

O2's a5 changes a function that p7 also changed. Prepare reports a
conflict in that file. A resolution task under O2's goal, on GPT,
settles it, and its result goes into a5's branch. Prepare runs again,
and only the keys that changed run.

- **Model:** 4.3.
- **Needs:** prepare's report of conflicts, and the resolution task
  under the member's goal (T2).

## 7. Act 4: the person (hours 6 to 7)

### 7.1 A question to the person, on the forge

O2's suite takes 70 s against its budget of 60 s. Budgets are the
person's, so O2 asks the person directly. The question is projected as
an issue in app, labelled `waiting-on-person`. The director is not
told.

The person comments on the issue: "don't raise it; make it cheaper."
The comment reaches O2 as the answer, and the issue closes with the
question. O2 has an implementer make the suite cheaper.

- **Model:** 3.2, 9.
- **Needs:**
  - any task asking the person (exact);
  - the question on the forge, answered by a comment (T14).

### 7.2 Questions to the chat stay with the chat

D asks the director, its requester: "Export's CSV first, or JSON?" The
question is the director's to answer, not the person's. The director
answers from the priorities in its notes.

Later D asks: "may kit's API break app's old callers?" The director
doesn't know, so it asks its own person, in the inbox. The person
answers. The director answers D in its own words.

- **Model:** 3.2.
- **Needs:**
  - a question to the chat staying with it (J7);
  - the chat asking its person (J8).

### 7.3 Parked until the person's draft

O1's next part needs the person's draft on Export's formats, which
doesn't exist yet. D parks O1 until a landing in app touches
`docs/design/drafts/formats.md`. The person lands the draft, a
documentation landing proved by receipts, and O1 wakes.

- **Model:** 3.3.
- **Needs:** a park lifted by an event (J4).

## 8. Act 5: holds (hours 7 to 8)

### 8.1 The person pauses P

The person: "pause Themes." The person stops P, with the person as the
hold's owner. P's run and its implementers' runs are cancelled.

Later the person says "use every free slot". The director reads that
as a resume and tries to release P, but it cannot, because the hold is
the person's. So it proposes the release. The person sees the proposal
in their inbox and says yes, and P resumes from its transcript.

- **Model:** 3.3.
- **Needs:** owned holds, and a proposal to lift one (J1).

### 8.2 A drain, to change the rules

The person wants to change the coordinators' rules while nothing is
half done, so they drain the project:
- running runs finish their turn;
- O2's landing, which was proving, lands;
- nothing new starts;
- each goal is held as it goes idle;
- the director stays live.

The person lands the rule change: a new revision of the coordinators'
charter, which live coordinators move to. Then the person lifts the
drain and releases the goals one at a time: D first, then O1 and O2
when D proposes them.

- **Model:** 3.1, 3.3.
- **Needs:**
  - drain (J2), over a project (J3);
  - a charter's new revision reaching live tasks (exact).

### 8.3 A stop over the project

The director relays a plan that the person hasn't approved yet. It
rewrites its priorities, starts a goal for the plan, and that goal
lands one change.

The person: "act on none." This stops the project: runs are cancelled
at once, nothing lands, and new tasks start held. The person cancels
the plan's goal, and asks the director for a revert of the landed
change. The revert's change starts held, so the person releases that
one task, and it lands.

Ninety minutes later the person approves the plan, and lifts the stop
with every hold it placed. The director starts the plan's goal again.

- **Model:** 3.3.
- **Needs:**
  - a stop over the project (J3), owned by the person (J1);
  - one task released under a project hold (J3).

## 9. Act 6: the tree changes, and the ends (hours 8 to 9)

### 9.1 A designer born, and a live owner moved

The rule changed in 8.2 says every new feature gets a designer. The
director starts L, Themes' designer, and moves P, still running, under
L:
- P's next result goes to L;
- its reservation is funded by L;
- the director keeps a reference to it.

- **Model:** 3.1, 3.4.
- **Needs:** moving a live task under another task (J5).

### 9.2 An owner ends, and its successor

O1's last landing is cleared, and O1 ends. Its result says what it did
and what is left: one part, and a question to the person about a
format's default, open for 2 hours.

D starts O3 with O1's result as its brief, and names O3 as O1's
successor. An hour later the person answers O1's question, and the
answer goes to O3.

- **Model:** 3.1, 3.2, 3.4.
- **Needs:** a successor that receives an ended task's answers (J6).

### 9.3 The ends

O2 and O3 each end once their last landing is cleared. D ends once its
owners have ended and its design carries what their `Taught:` lines
said. The director compares their results, and proposes a change to the
rules, which the person accepts.

- **Model:** 3.1, 5.2, 8.
- **Needs:** goals that end when their changes clear (T10); results
  and proposals (exact).

## 10. The index

Every mechanic of `process-model.md` that the case uses, with the steps
that hold it.

| Mechanic | Model | Steps |
|---|---|---|
| A chat as coordinator; coordinator goals; goals at depth three | 3.1 | 4.1, 4.2, 9.3 |
| Delegates by charter and model: implementers, reviewers, an advisor, a tester | 3.1 | 4.1, 4.2, 5.1, 5.3 |
| Periodic checks | 3.1 | 5.2, 5.3 |
| A standing rule as a charter's new revision | 3.1 | 8.2, 9.1 |
| Any task asking the person | 3.2 | 7.1 |
| A question to the chat stays with it; the chat asks its person | 3.2 | 7.2 |
| An answer after its asker ended | 3.2, 3.4 | 9.2 |
| Owned holds and proposals to lift them | 3.3 | 8.1, 8.3 |
| Stop and drain, over a subtree and over a project | 3.3 | 8.1, 8.2, 8.3 |
| Park until an event | 3.3 | 7.3 |
| A live task moved under another | 3.4 | 9.1 |
| A successor | 3.4 | 9.2 |
| A change's life to cleared | 4.1 | 4.3, 4.4, 9.3 |
| A landing across repositories, with a pin-only member | 4.2 | 4.3 |
| `with`, across goals | 4.2 | 6.1 |
| Prepare: rebase, pins, a regenerated file | 4.3 | 4.3, 6.3 |
| A conflict to a resolution task | 4.3 | 6.3 |
| Receipts; documentation landings with no run | 4.4 | 4.1, 7.3 |
| Dispatch-only gates; a repair; a flake-matching rerun | 4.5 | 4.3, 5.2 |
| The turn: bases checked, fast-forward upstream first | 4.6 | 4.3, 6.2 |
| The reservation | 4.7 | 6.2 |
| No test twice on a tree | 4.8 | 4.1, 4.3, 6.2 |
| Tiers on the newest main, one run at a time | 5.1 | 4.4, 5.1 |
| Verifying, cleared, the bound | 5.2 | 4.4, 5.4, 6.1 |
| Rerun, bisect, inherited failures, faults | 5.3 | 4.4, 5.1, 5.2 |
| A red to its goal; others go on; a red as a finding | 5.4 | 5.1, 5.4 |
| Findings: filed, matched, triaged, closed on clearing | 6 | 5.2, 5.3, 5.4 |
| QA after landing | 7 | 5.3 |
| Trailers in landing news | 8 | 4.3, 9.3 |
| The person's questions on the forge | 9 | 7.1 |

## 11. What the case asks of temper

Every gap of `process-model.md` 12, with its steps:

| Gap | Steps |
|---|---|
| J1 owned holds | 8.1, 8.3 |
| J2 drain | 8.2 |
| J3 project holds | 8.2, 8.3 |
| J4 park until an event | 7.3 |
| J5 move under a task | 9.1 |
| J6 successor | 9.2 |
| J7 questions to the chat stay | 7.2 |
| J8 the chat asks its person | 7.2 |
| T1 landings, `with`, pin-only members | 4.3, 6.1 |
| T2 prepare | 4.3, 6.3 |
| T3 receipts | 4.1, 4.3, 6.2 |
| T4 dispatch-only gates | 4.1, 4.3, 5.2 |
| T5 candidates on members' branches | 4.3 |
| T6 the turn | 4.1, 4.3, 6.2 |
| T7 the reservation | 6.2 |
| T8 tiers | 4.1, 4.4, 5.1 |
| T9 verdicts | 4.4, 5.1, 5.4 |
| T10 a change to cleared, the bound | 4.1, 4.4, 5.4, 6.1, 9.3 |
| T11 findings | 5.2, 5.3, 5.4 |
| T12 QA after landing | 5.3 |
| T13 trailers in landing news | 4.3 |
| T14 questions on the forge | 7.1 |
| T15 views and acts | every step, through `temper ctl` |
| B1, B4 the forge's calls and adoption | 4.3, 5.1, 5.2 |

B2 and B3, GitHub's, come when the case runs on GitHub. The first pair
of backends is an owned Forgejo.

## 12. Left out, and why

- **A partial turn** (`process-model.md`, 4.6): it needs the forge to
  fail between two members. It is a fault test on fakes, not a step.
- **"Tell all" and "a red pauses its repository"**: settings the
  process doesn't choose.
- **Judgement gates keyed by a diff:** reviews here are delegates
  inside goals, not gates on landings.
- **A review's follow-up as a finding:** the process files findings
  from QA and verdicts only.
- **Machines, caches, disks and sign-ins:** the plane's, not the
  model's. A runner's fault (4.4, 5.4) is as far as the case goes.
- **The change of shape** that the first edition's act 2 tells: here a
  rule changes (8.2, 9.1), but the shape of the tree does not.

## 13. Open questions

- **Names:** "designer" and "owner" are roles of this case's process.
  temper's charters may name them otherwise.
- **The oracle on fakes:** which judgements are scripted word for word
  (the director's triage, the chooser's picks), and which are checked
  by their effects.
- **Real time:** whether 9 hours fits one sitting on one held login,
  or whether the acts run apart, each from the state the act before it
  leaves.
