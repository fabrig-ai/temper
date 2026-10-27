# Temper on temper

Draft, 2026-10-21. Can the owners' process (agents'
`roles/owners.md`) be encoded in temper, so that from the same
inputs and the same environment temper runs the same execution, or a
better one? This draft records a session with the user on 2026-10-20
and 2026-10-21, the session `qa-loop.md` leaves to one of its own. The
old process of designs, plans and runs is out of scope. The evidence is
a trace of 25 hours of the real execution, kept in
`~/src/rust/tmp/temper-on-temper/`. What falls short is recorded here
(section 6); drafts in jig, smith or temper follow when the user agrees.
On 2026-10-21 owners.md and director.md moved to a new shape: owners
reach their drafts through milestones under a cap, each on a fresh
session. Since that evening each owner takes one mission of at most
four milestones, and then stops. Sections 4.2, 4.4 and 6 take it in.
The retired first edition (superseded by `reference-case.md`) condenses this analysis, the change of shape and
the new shape's first milestone into one story.

## 1. In one page

- **The bar is the execution, not the concepts:** the same inputs (the
  user's operational chat with the director, and drafts from design
  sessions) and the same environment (machines, accounts, budgets) give
  the same agents, scheduling, landings and escalations. Equal or better
  is the aim: the reference execution delivers, holds its throughput and
  handles problems, but it is artisanal (section 2).
- **The reference:** 10-19 16:15Z to 10-20 17:20Z. It has 286 landings
  in five repositories, up to ten owner sessions on five machines, 72
  messages from the user and about 36 incidents. It has a quality
  baseline and 88 moves no rule prescribed (section 3).
- **The encoding:** one deployment with a worker per machine, and one
  project with the five repositories.
  - The director is the user's chat. Owners are goals; implementers,
    testers and advisors are delegated tasks.
  - `land` is the change procedure on the forge's CI. Repins are
    dependent changes.
  - The process documents are conventions, which every run reads
    (section 4).
- **Six replays** of real episodes, each judgement replayed as an
  oracle, check that temper would wake the same agent (the trigger),
  show it what the director saw (the information), let it act (the
  power), and do what followed (the mechanics). Every judgement can be
  expressed, except two powers (section 5).
- **The retired first edition** (superseded by `reference-case.md`) is one story that holds
  every mechanic here once, the change of shape included. The mockups,
  the lab and the director work from it.
- **What falls short needs design:**
  - adding a dependency to a live task, since designed in jig;
  - parking an owner;
  - plan usage in accounts;
  - worker health;
  - a failure at the landing tip that matches an open finding.

  Whether an owner resumes its session or starts fresh is the charter's
  choice already; which works better is an experiment (section 6). The
  new shape adds seven (6.8):
  - an open finding at a change's head;
  - a repair that adopts work in flight;
  - the cap against delegates' deadlines;
  - a fresh run at a milestone's end;
  - a stopped goal's delegates;
  - retiring a charter;
  - a goal's successor.
- **What temper does better:**
  - Sessions can't be orphaned or duplicated.
  - Releases need no relay, and landing conflicts get repair tasks.
  - x1 is no longer a single point.
  - Spend per owner can be computed.
  - Rails: a tester cannot land (section 7).
- **Outside temper:** disk, caches, build queues and machines belong to
  the ground and to hearth; benchmarks, flakes and findings to the lab;
  worker health to a later ops application (section 8).
- **The path:**
  1. Design the gaps.
  2. Replay the reference in the lab, with the judgements scripted.
  3. Move one owner into temper while the director stays outside.
  4. Move the director last (section 9).
- **Landing throughput stays as designed:** one queue per landing
  branch, a fast path for documents and fast runners. Batching or the
  forge's merge queue comes when the waits grow (section 10).

## 2. The question and its bar

- **The inputs:**
  - the user's operational chat with the director: direction, rules,
    answers, approvals, steers and reports;
  - the drafts that design sessions land;
  - the environment: the machines and their resources, the accounts and
    their plans, and the budgets.
- **What must match, the execution:**
  - the agents: each one's role, who started it, its engine, model and
    effort, its machine, and when it starts, parks, resumes, moves and
    ends;
  - the dependency graph, as it changes;
  - the landings and repins per repository, in a compatible order, with
    their gates;
  - the escalations to the user;
  - the actions that resources forced.
- **What need not match:** clock times (the causal order counts), the
  LLMs' text, identifiers, transport, and the concepts themselves.
- **Equal or better:** quality is measured on five things:
  - delivery;
  - throughput;
  - cost;
  - incidents and how they were handled;
  - the user's load.

  The moves no rule prescribed test whether temper is flexible enough
  to make them too.
- **The method:** each judgement in the reference is an oracle, and
  each is checked for four parities:
  - the trigger: temper wakes the agent at that point;
  - the information: its tools show it what the director saw;
  - the power: the decision can be expressed as temper's operations;
  - the mechanics: temper produces what followed.

  The rails are checked too: where temper would refuse or delay
  something that happened, it is either an improvement or a conflict.
- **The grades** of a practice:
  - **exact:** existing operations, the same result;
  - **sugar:** expressible, but a small convenience would help;
  - **design:** needs a capability that is not designed;
  - **conflict:** a rail of temper's makes the outcome worse unless a
    policy changes;
  - **plane:** it belongs to the ground, hearth, the lab or the ops
    application (`factory.md`, sections 3 to 8).

## 3. The reference execution

- **The window:** 25 hours, from 10-19 16:15Z to 10-20 17:20Z.
- **The sources:**
  - the director's log, decisions, notes, briefs and inbox;
  - the director's transcripts and two owners' (o/first-run,
    o/accounts-2);
  - every owner's notes and ledger.

  They were reduced into a trace of 8,414 events of a fixed schema.
  The trace has a quality baseline, an hourly timeline per machine,
  and a catalogue of improvisations (`out/` in the evidence
  directory).
- **The agents:**
  - the director, an Opus session on x1;
  - owners per draft: browser-1, hearth (with its sub-owners files and
    trees), smith-perf, accounts-2, smith-sources, github (with dialect,
    runtime and temper), first-run, and two Codex owners, hygiene and
    qa;
  - implementers, mostly Codex, with Claude kept for taste; Opus xhigh
    advisors; testers.
- **Delivery:**

  | Repository | Landings | By kind |
  |---|---|---|
  | skein | 115 | 87 code, 28 docs |
  | temper | 53 | 24 code, 18 docs, 11 repins |
  | jig | 52 | 42 code, 8 docs, 2 repins |
  | smith | 46 | 33 code, 9 docs, 4 repins |
  | hearth | 20 | 12 code, 7 docs, 1 repin |

  That is 286 in all, 11.4 an hour. By machine: hetzner2 114,
  hetzner 73, hel8 48, x1 40, carbon 11. There were 37 records of
  failed landings.
- **Dependencies:** 23, of which 12 were known up front and 11 were
  found while the work went on.
  - Of the 11 that ran to a release, the wait had a median of 103
    minutes and a maximum of 364.
  - 6 were released by a landing, 5 by a judgement. A landing's
    release often still needed the director to relay it.
- **The director:**
  - It answered in a median of 8 minutes (nine sampled pairs), at most
    239.
  - Work waited on it for 4.4 wall hours, mostly through x1's freeze
    from about 02:46Z to 06:43Z, which held three owners' green lights.
- **Incidents,** about 36:
  - x1 hung and rebooted;
  - carbon became unreachable;
  - disk fell below the floor on three machines;
  - logouts and a weekly limit stopped sessions;
  - resume copies acted beside their originals;
  - a completion checker was orphaned and a process hung on stdin;
  - sessions' tokens reached an agent through a build queue's
    environment;
  - flakes, and gates that failed on slow or loaded machines.
- **The user:**
  - 72 messages: 23 direction, 27 questions, 9 rules, 3 approvals, 10
    reports;
  - at least 7 escalations;
  - many steers nobody asked for, and about 15 amendments to the
    process documents landed mid-flight;
  - the user is also the environment's operator: servers, sign-ins,
    subscription switches, remounts, the organisation's rename.
- **Cost** is read as plans' percentages, not money, so no total per
  owner or per landing can be computed.
  - At 13:43Z, Claude burned about 3.6% of the week an hour, against
    about 0.45% that the week could sustain.
  - In the sampled windows, the director's share of the spend was
    about 9%.
- **The improvisations,** 88 kinds of move:

  | Class | Moves | Examples |
  |---|---|---|
  | placement | 6 | moving owners between machines |
  | budget | 7 | throttling, parking, Claude and Codex |
  | resources | 15 | disk, caches, lanes, memory |
  | ordering | 18 | sequences, quiet windows, deadlines, splits |
  | recovery | 21 | stuck sessions, orphans, failed gates, reboots |
  | relay | 7 | passing news between owners |
  | process change | 7 | rules changed mid-flight |
  | other | 7 | scope cuts, interpretations of rules |

  The five that mattered most:
  - stopping a stalled experiment to free x1's disk for the chain of
    repins;
  - landing hygiene's branches from hetzner2 rather than a loaded x1;
  - recovering the originals in place after the duplicate resumes;
  - folding two repins into one (t4r);
  - moving work that needs no taste to Codex.

## 4. The encoding

### 4.1 The deployment

- **One deployment.** The engine runs on a host that stays up, not x1.
- **A worker per machine:** x1, hel8, hetzner, hetzner2 and carbon.
  Each is a host kind of its own, and a charter names one machine's
  kind or several (jig's `domain/hosts.md`, section 3).
- **One project, `factory`,** adopting skein, smith, jig, hearth and
  temper as owned repositories, on GitHub.
- **The user is its owner.** The accounts are the user's (Claude plans,
  ChatGPT plans), in the user's order of sources.
  - Smith signs in with ChatGPT itself (jig's `domain/providers.md`,
    section 1), and is on par with Codex (smith's `benchmarks.md`,
    section 16).
  - Claude on a plan runs through Claude Code, as smith's program
    backend.
- **CI is the forge's,** with self-hosted runners on the machines:
  - the gates need wasm32, the private Chromium and heavy;
  - runner labels choose the machine;
  - each repository's CI runs `scripts/check.sh`.

  This is configuration and environment, not temper.

### 4.2 The roles

- **The director is the user's chat:** a conversation task with a
  coordinator's charter, which a coordinator may be (jig's
  `domain/core.md`, 7.2).
  - Its charter: `check.md`, the standing direction, and `owners.md`'s
    part for the director.
  - It wakes on the user's words; on owners' questions, proposals,
    results and failures; and on a timer every two hours.
  - Landing news is kept for its next run, not woken on.
  - Its authority is the project's.
  - Its compactions, capped since 10-21, and its fresh context at every
    third check are its charter's waiting and the deployment's resume
    limit (6.6). Its timers are the task's, so they can't keep a run
    busy past a fresh start.
- **An owner is a goal,** a tracked task with a priority (jig's
  `domain/core.md`, 6.2), which the director delegates per draft.
  - Since 10-21 there is one goal per mission: a draft, or part of one,
    with a defined end, reached in at most four milestones. The mission
    ends with its closing note as the goal's result, and the goal
    completes. No goal is standing: recurring work, such as hygiene,
    becomes missions too. The next mission is a new goal, and its input
    carries the last one's notes and closing note (6.8). A goal's
    deadline at about 16 hours would make a mission's size a rail, where
    today it is a count of milestones.
  - Its charter: `owners.md`'s part for owners.
  - Its instructions: the brief, with the draft as an input
    (repository, path, commit). The user's order of drafts becomes the
    goals' priorities.
  - Its model and effort: Opus xhigh, or GPT through smith for a Codex
    owner.
  - Its host kind is its machine, and its implementer slots are a pool
    (jig's `domain/tasks.md`, section 6).
  - Its notes are its ledger.
  - It wakes on its delegates' questions and results, on failures, on
    landings that overlap its work, and on the director's words.
  - Its charter resumes the transcript and waits with a long threshold,
    as today's long sessions do (6.6). Since 10-21 an owner starts
    fresh at each milestone's end, and today's handover routine goes:
    one run per task leaves no old session to stop (6.8).
- **A sub-owner** is the same, delegated by its owner, narrower. Since
  10-21 there is one per milestone, under a deadline at the 4-hour cap.
  It starts no sub-owner of its own, which authority's depth makes a
  rail.
- **An implementer** is an agent task on its own r-branch.
  - Its instructions are the task file. Its model is GPT at medium or
    high by the task, or Claude for taste.
  - Its result reaches the owner, who checks the diff and hands the
    branch down to a change (`../domain/forge.md`, section 11). Or the
    implementer's task produces the change, and the owner's check is a
    gate on it.
- **A tester** is an agent task without forge effects, so it cannot
  land. Since 10-21 a milestone's QA pass is one: a Codex task that
  reads the design only. It runs beside the next milestone, and is
  delegated by the sub-owner that ended.
- **An advisor** is a one-shot agent task, Opus xhigh, with a result
  contract.

### 4.3 Landing

- **`land` is the change procedure** (`../domain/forge.md`, sections 8
  and 9):
  1. CI runs on the change's head. A failure is a repair, given the
     failing output.
  2. A ready change joins its landing branch's queue.
  3. The first in the queue is brought up to date and lands once CI
     passes at its exact head.

  An update that conflicts becomes a conflict resolution, and CI that
  fails a repair. A change of Markdown alone takes the
  CI's fast path, as `land --docs` does now.
- **A repin** is a change in the downstream repository that depends on
  the upstream change, as forge.md's own example has it: skein lands
  before temper bumps its lock.
- **The after-landing lane** is CI at the landing tip: the slower tiers
  run on main after each landing. A failure there pauses the queue and
  asks for a repair (6.5).

### 4.4 Coordination

- **A design report and its green light** are a question and its
  answer. A change that breaks another owner's interface is a proposal
  the director decides (jig's `domain/tasks.md`, section 9).
- **A checkpoint** ("bring me its trace before it lands") is a
  convention: the owner hands its branch down only after the answer.
- **Milestones** (owners.md since 10-21):
  - The plan is the owner's first report, a question its parent
    answers.
  - The cap is the sub-owner's deadline. At the deadline jig holds the
    sub-owner and tells its requester, and the escalation goes up as
    questions. The answer (a split, more time, a change of course) is
    an amendment or a new sub-owner. Where jig and the cap differ is
    6.8.
  - A milestone's end is a result to the owner. The owner's next run
    starts the next sub-owner on it.
- **Pausing:** the user's pause is the user's Stop on each goal, and
  their resume a Release with an amended brief. While a goal is held,
  overlaps on its files are kept for it as news, which replaces the
  ledger of hunks another owner writes for its rebase. Parking by the
  director stays a gap (6.2).
- **Footprints** are the paths a change's spec names. A landing that
  overlaps them wakes the owner (`../domain/forge.md`, section 7). They
  are not enforced: authority is per repository, not per path.
- **Dependencies:**
  - Those known up front go in the batch.
  - A dependency found later is a new task that depends on the live
    one, or a subscription to its landing (6.1).
  - A landing starts what depends on it, with no relay.
- **Ordering:**
  - priorities: the queue and the pools take goals by priority;
  - holding a branch back;
  - a timer, for "reorder if it is more than three hours out";
  - removing a dependency that is still waiting, which amend allows.
- **Slots:**
  - Throttling and lending slots are pools resized by amend.
  - A pool that shrinks keeps its holders and admits nobody until it
    is below its size.
- **Relays:**
  - Landing news and subscriptions carry most news.
  - The director introduces owners who must talk directly.
  - The wake policy batches arrivals, as the watcher's hashes and its
    second look do now.
- **The two-hour check** is a timer, and status recaps are client
  reads.
- **Rules:** the process documents are conventions in the repositories.
  A rule that lands reaches every later run, with no message to
  re-read it.

### 4.5 Accounts and budgets

- **Engine choice is the charter's models.** Moving work that needs no
  taste from Claude to GPT is an amendment, effective at the next run.
- **A logout or a limit** makes the tasks it pays for wait, with the
  reason. When the user signs in again or the limit resets, they go on
  (jig's `domain/accounts.md`, 3.4 to 4).
- **Switching subscriptions** is the holder reordering their sources, as
  the user does now.
- **Spend** rolls up per task, so per owner (jig's `domain/authority.md`,
  section 7).
- **Pacing** against a plan's window needs the plan's usage (6.3).

### 4.6 The practices, graded

| Practice | In temper | Grade |
|---|---|---|
| Director as the operational chat | a kept chat with a coordinator's charter, off x1 | exact |
| An owner per draft, sub-owners | goals and delegated coordinators | exact |
| Codex owners and implementers | GPT through smith on the ChatGPT plan | exact |
| Implementers, advisors, testers | delegated tasks, models and effort in the charter | exact |
| A tester never lands | no forge effects in its authority | exact, a rail |
| Design report, green light | question and answer; a proposal for a break | exact |
| Footprints | overlap news on the spec's paths | exact as detection |
| Checkpoint before landing | a convention | exact; sugar for a gate an ancestor judges |
| Dependencies known up front | in the batch | exact |
| Dependencies found later | a new task or a subscription | design (6.1) |
| A landing releasing work | dependencies and landing news | exact |
| Orders, windows, deadlines | priorities, a held branch, timers, amend | exact |
| Slots, throttling, lending | pools | exact |
| Relays between owners | news, subscriptions, introductions | exact |
| Two-hour check, status | a timer, client reads | exact |
| Escalation to the user | the user's inbox | exact |
| Rules changed mid-flight | conventions read by every run | exact |
| `land` | the change procedure on the forge's CI | exact |
| Repins | dependent changes | exact |
| After-landing lane | CI at the landing tip | exact; conflict on a known flake (6.5) |
| Claude for taste, Codex for the rest | the charter's models | exact |
| Logouts, limits, switches | accounts' states, the holder's sources | exact |
| Pacing against a plan | the plan's usage | design (6.3) |
| Parking an owner | Stop, the user's alone | design (6.2) |
| Placing an owner on a machine | host kinds | exact; sugar to amend one |
| Placement by load and disk | worker health | design (6.4), plane |
| Carrying data on a move | notes and branches go with it; other data doesn't | plane (hearth, lab) |
| Gate valid on every machine | runners, the lab | plane |
| heavy, disk floors, caches, memory caps | the machines' environment | plane |
| Live comparisons, benchmarks | experiments | plane (lab) |
| Orphans, duplicates, stale copies | one run per task, fenced | gone |

## 5. Replays

Each replay names what happened, then T (trigger), I (information), P
(power), M (mechanics) and R (rails) in temper. The retired first edition
(superseded by `reference-case.md`) puts all six into one story, with the change of shape on 10-21 and the
new shape's first milestone.

### 5.1 o/qa starts (decision 242, 16:52Z)

The user: "qa-loop draft ready… please schedule it when possible", and
earlier "qa loop will be mostly codex". The director started a Codex
owner on hetzner, which real-loop's end had freed:
- taste went to an Opus advisor;
- the fakes as processes went to o/first-run;
- the check on the forge's promises went through the director;
- two implementers on hetzner, shared with o/accounts-2, and one
  cluster run at a time.

- **T:** the user's words wake the chat.
- **I:** the drafts come from forge reads. That hetzner is free is the
  fleet's to know, but no tool reads it yet (sugar).
- **P:**
  - delegate a goal with GPT through smith, host kind hetzner, and a
    pool of two;
  - amend o/first-run's instructions;
  - "one cluster run at a time" is a hold on the cluster as a resource,
    or a convention;
  - "through me" for o/github's crate is a convention (4.4).
- **M:** the goal waits for its slot and is placed when real-loop ends.
- **R:** nothing that happened would be refused.

### 5.2 Temper's repin order (decision 238, 15:22Z) and t4r (17:03Z)

After jig d21ab188, three owners had temper repins pending. The
director ruled:
- t4b first;
- then o/accounts-2's t4r to jig's main, taking o/first-run's
  Directories with it, so one repin instead of two;
- reorder if t4r was more than three hours out.

t4r landed at 17:03Z, about an hour early, and the director relayed:
o/first-run could start, and o/github could start its skein repin, 01b.

- **T:** the owners' reports, or jig's landing news.
- **I:** the tree's change tasks, their bases and the owners' estimates.
- **P:**
  - amend o/accounts-2's instructions and cancel o/first-run's repin;
  - order by holding t4r's branch back until t4b lands, since t4r's
    change exists and a dependency cannot be added to it (6.1);
  - the three hours are a timer.
- **M:** t4r's landing starts o/first-run's directories and o/github's
  01b by dependency or by landing news. The relay goes. A conflict in
  `Cargo.lock` is a conflict resolution.
- **R:** CI at the exact head, as `land` does now.

### 5.3 o/smith-perf moves (decisions 235 and 236)

o/smith-perf moved from x1 to hetzner2 when that server arrived. At
13:43Z the two-hour check found hetzner2 at capacity:
- 16 builds queued;
- a load of 1.45 per core.

Under the user's rule for that case, the director moved the owner back
to x1 after its round 4. Moving it meant:
- stopping its session after it wrote its RESUME;
- copying its notes, pairs and dogfood (218 MB);
- leaving its worktrees behind, every branch pushed;
- rewriting its brief for x1's disk;
- landing jig for it from hetzner2.

- **T:** the timer.
- **I:** the queue and the load are not temper's to read (6.4). Spend
  per owner is, and better than the burn scripts.
- **P:** amend the owner's host kind. It takes effect at its next run,
  so "after round 4" comes free. Amending a host kind is not among
  amend's named fields (sugar).
- **M:**
  - Notes and branches follow the task. The pairs and the dogfood do
    not: they need a store (hearth's outputs, the lab's evidence).
  - x1's rule of one build at a time is the worker's slots.
  - Landing jig for it goes: the gate runs on the runners, not on the
    owner's machine.
- **R:** worktrees left behind become the worker's cache and are
  evicted.

### 5.4 The 'Again' flake at the landing tip (11:19Z to 11:33Z)

1. **11:19Z:** hetzner's after-landing lane failed on temper 787649ae4,
   real-loop's landing, with a flake o/hygiene already owned, its fix
   (1t) queued. The director did not route it to the landing's owner.
2. **11:21Z:** the same flake failed on 83dcd2e72, o/accounts-2's t3a.
   o/hygiene was told to land 1t first.
3. **11:33Z:** 1t landed, and both stories passed.

All along, temper's main kept taking landings.

- **T:** CI at the tip fails.
- **I:** that this is a known flake with an owner and a fix needs the
  lab's findings.
- **P:** route it to o/hygiene and raise 1t's priority.
- **M:** a conflict. Temper pauses the queue and asks for a repair. 1t
  is itself a change in that queue, so the fix waits behind the pause,
  unless the repair adopts it or a rerun passes (6.5).
- **R:** stronger in general, since main is never quietly red. Too
  strict for a known flake.

### 5.5 o/hearth parked and resumed (decisions 184 and 207)

- **Parked at 00:00Z** on the user's word: "park further hearth work
  for a bit". Its stacks stayed unlanded, pushed as branches, with
  notes to resume from, and the director stopped its sessions.
- **Resumed at 07:44Z** on carbon, spare capacity the user allowed,
  from those notes.
- **Parked again at 09:34Z** "to save credit".

- **T:** the user's words.
- **I:** nothing missing.
- **P:** Stop is the party's alone (jig's `domain/people.md`, 5.1). The
  user can stop the goal in the client, or the director can propose
  Stop and the user accept it. The director cannot park it itself
  (6.2).
- **M:**
  - Stop cancels the run and leaves the task held. Delivery in flight
    settles, saved work pushes what is unfinished, and the notes stay.
  - Release resumes it wherever carbon has a slot, from the brief or
    the transcript. There are no RESUME files and no copying.
- **R:** "save credit" would be a budget, if budgets knew plans (6.3).

### 5.6 The pace, escalated (decision 235, answered in 247)

- **13:43Z:** at the two-hour check, Claude was burning about 3.6% of
  the week an hour against a sustainable 0.45%, and would run out at
  about 08:00Z the next day. The pace question went to the user.
- **15:42Z:** it was still with the user at the next check.
- **17:46Z:** the user answered: "keep the pace as long as hardware
  resources and cross-plan churn permits".

- **T:** the timer.
- **I:** the plan's usage and its resets are not temper's to read (6.3).
  Spend per owner and per machine is, in full.
- **P:** a question to the user. It waits in the user's inbox rather
  than in two checks' reports.
- **M:** with budgets in the plan's terms, the budget itself would
  hold and escalate.
- **R:** an exhausted budget holds the work it pays for.

### 5.7 What the replays show

- **Every judgement can be expressed,** with two exceptions:
  - the director cannot park an owner (6.2);
  - a dependency found later cannot be added to a live task (6.1).
- **Information falls short twice:**
  - the plan's usage (6.3);
  - the machines' health (6.4).
- **Mechanics differ once, for the worse,** on a known flake at the
  landing tip (6.5).
- **The rest of the mechanics replace hand work:**
  - relays;
  - RESUME files and copies;
  - landing for an owner from another machine;
  - conflicts repaired by hand.

## 6. Where it falls short

Each of these needs assessment and design, in a draft of its own.

### 6.1 Adding a dependency to a live task

- **What exists.** The task tree changes freely at runtime:
  - new tasks, depending on any live task their creator can reach;
  - amendments: instructions, authority, models, wake policy,
    priority;
  - cancel, move, release;
  - removing a dependency that is still waiting.

  What is refused is adding a dependency to a task that exists (jig's
  `domain/tasks.md`, sections 4 and 7). jig forbids it so that the
  cycle check stays local to a batch, and lists it as open (section
  13).
- **Why it matters here.** 11 of the reference's 23 dependencies were
  found while the work went on, mostly on tasks already running:
  - a repin that turned out to need another owner's landing;
  - the directories work that needed o/github's 01b;
  - browser's repin that met accounts' older pin.

  A new task with the dependency, or a subscription, can stand in.
  The user's ruling: design the feature, not the workaround.
- **To design:**
  - the cycle check across the live graph, and its cost;
  - what a dependency means for a task that is running: park until the
    other closes, or a step that waits at its next turn;
  - who may add one: the requester, an ancestor, a party;
  - how it shows in views and the tree's history.
- **Since designed:** jig's `domain/tasks.md`, section 7, "Adding and
  removing dependencies", which is being built.

### 6.2 Parking an owner

- **What exists.** A party's Stop ends a task's run and holds it for
  that party, and Release resumes it (jig's `domain/people.md`, 5.1).
  An agent cannot stop a run. A coordinator can cancel, amend or
  shrink a delegate's budget, none of which is a park.
- **Why it matters here.** The director parked o/hearth twice and
  resumed it once, each time on the user's words. It also stopped and
  restarted owners to move them, and to recover after duplicates. On
  10-21 the user paused every owner but one. That pause is the user's
  own Stop, which the user can do in the client. What stays a gap is
  the director doing it for them.
- **To design:**
  - whether an ancestor may park a delegate's subtree, and whom the
    hold escalates to;
  - what a park settles: delegates, effects in flight, saved work;
  - parking for budget, as distinct from a hold for an exhausted
    budget;
  - resuming elsewhere.

### 6.3 Plan usage

- **What exists.** An account knows it is limited, and the reset the
  provider gives (jig's `domain/accounts.md`, 3.4 to 3.6). Budgets are
  in a deployment's unit of spend, and temper's periods have no
  calendar yet (`../protocol/configuration.md`, section 11).
- **Why it matters here.** The director paced the work by the plans'
  windows:
  - the five-hour and weekly percentages, their resets, and the rate
    the week can sustain;
  - parking, throttling and moving work to the other provider by them;
  - escalating the pace to the user.

  It read them by hand, from each provider.
- **To design:** reading a plan's usage per window as an account's
  facts; budgets and pacing in the plan's terms; how that meets
  the holder's order of sources across subscriptions.

### 6.4 Worker health

- **What exists.** Workers declare slots. Placement goes by host kind,
  model and source, and is withheld only on a failed check of a
  program's login.
- **Why it matters here.** Placement followed each machine's:
  - load per core;
  - build queue;
  - free disk against its floor;
  - memory.

  The director sampled them on every machine to move owners and to
  decline work. Gates failed on loaded or slow machines.
- **To design**, likely as a later ops application feeding temper:
  - a worker's health as facts a coordinator can read;
  - placement withheld when a worker fails a health check, as for a
    program's login;
  - amending a task's host kind.

### 6.5 An open finding at the landing tip

- **What exists.** CI failing at a landing branch's tip pauses its
  queue and asks for a repair (`../domain/forge.md`, section 9). A
  rerun before a repair is open (section 20).
- **Why it matters here.** A known flake with an owner and a fix in
  flight would stop every landing in its repository, the fix's
  included (5.4).
- **To design:** a failure at the tip that matches an open finding (the
  lab's, `factory.md`, section 8) does not pause the queue, and is
  attached to the finding; one that matches nothing pauses it as now.

### 6.6 A long session or a fresh start

- **Not a gap.** A parked task's charter says whether its next run
  resumes the transcript or starts fresh from a brief, and how long a
  waiting run keeps its slot (jig's `domain/engine.md`, 7.4). The
  defaults are a coordinator fresh with no waiting, and a conversation
  resumed. Past the deployment's resume limit, a run starts fresh with
  the transcript's tail (7.2).
- **For owners on Claude's plan,** resuming rests on Claude Code's own
  sessions under smith's program backend, which smith is building.
- **Which is better is an experiment.** The old process's experience
  favours compaction. The sampled owners compacted often (dozens of
  times in a few hours, to be checked). The encoding starts with
  resume (4.2).
- **Since 10-21 owners.md settles it for owners:** resume within a
  milestone, and start fresh at its end. Expressing that is 6.8's.

### 6.7 Smaller

- **Fleet occupancy:** a coordinator reading which workers have free
  slots (5.1).
- **A gate an ancestor judges,** for checkpoints as rails rather than
  conventions (4.4).
- **Footprints enforced,** by grants on paths within a repository,
  if detection proves not enough (section 11).
- **Data that is neither git nor notes,** carried on a move: hearth's
  outputs or the lab's evidence (5.3).

### 6.8 From the new shape

The change of shape on 10-21 and the new shape's first milestone
(o/first-run's D1) showed seven more. The retired first edition
(superseded by `reference-case.md`) names the
step each comes from.

- **An open finding at a change's head.**
  - CI failing at a change's head starts a repair (4.3). In D1, 8 of 9
    failed gates came from the runner (its CPU clock under load, a
    global timeout) or from a known flake, and one was real. In temper
    each of the eight would have been a repair with nothing to fix.
  - To design: a failure that matches an open finding, or comes from a
    runner flagged unhealthy (6.4), runs CI again rather than starting
    a repair. This is 6.5's rule, at the head.
- **A repair that adopts work in flight.**
  - D1's first landing turned main red: the after-landing tier, which
    the gate doesn't run, failed. The fix was already in flight in the
    milestone's next task, and the director gave it a deadline.
  - To design: the tip's repair adopting a change that is already open,
    rather than starting one.
- **The cap against delegates' deadlines.**
  - jig holds a task past its deadline and tells its requester. Its
    delegates' deadlines are no later than its own
    (`domain/authority.md`, section 7), so they are held with it.
  - owners.md's cap is different. A milestone at its cap starts nothing
    new, but what is running finishes.
  - To design: a grace for delegates at a parent's deadline, or a
    deadline that holds new starts only.
- **A fresh run at a milestone's end.**
  - A charter says resume or fresh for every next run (6.6), while
    owners.md wants both, by phase.
  - To design: a coordinator asking for a fresh next run, or a charter
    policy keyed on a delegate's result. It is likely sugar.
- **A stopped goal's delegates.**
  - When the user paused owners, their running implementers finished
    into their worktrees, and their changes already queued landed.
  - To check: whether a party's Stop (`domain/people.md`, 5.1) leaves a
    goal's delegates and queued changes going on, and if not, whether
    a pause needs it to.
- **Retiring a charter.**
  - The user had the old process's leftovers stopped, and kept from
    starting again. A new shape also reaches owners mid-flight.
  - To check: withdrawing a charter that still has tasks, and whether
    a charter's changed policy reaches tasks already running. An
    amendment replaces a task's instructions whole (`domain/tasks.md`,
    section 7), but nothing says what a charter change does.
- **A goal's successor.**
  - Owners take missions now, so a draft passes from goal to goal. Some
    owners paused under the old shape were released only to wrap up:
    land what was ready, write a closing note with what was left, and
    stop. Fresh missions took their work on, with briefs built from
    their notes.
  - Work other owners owed the ended owner went to it by name. Hashes
    o/github owed o/qa had to be re-addressed by hand to o/qa-a, its
    successor.
  - In jig, a goal's notes are scoped to its subtree (`domain/core.md`,
    6.2). Its kept news, footprint, grants, and waits on other tasks
    (dependencies and subscriptions) are its own too. So a successor
    goal starts with none of them.
  - To design: what passes from a goal that ended to the goal that
    takes its work on: notes, kept news, grants on another owner's
    files, parked branches, and its waits on other tasks, so that what
    others owe it reaches the successor. And whether that takes a new
    operation, or an input and a note scope that the director passes on.

## 7. Where it does better

- **Bookkeeping incidents go.** About 12 of the 36 incidents need no
  handling in temper:
  - missing processes;
  - resume copies acting beside their originals;
  - stale copies after a reboot;
  - an orphaned checker;
  - recreating agents after a logout;
  - a hang on stdin;
  - the hand repairs of conflicting landings (ten records in o/github's
    runtime alone).

  The engine keeps one run per task, fences a lost attempt before the
  next, and repairs a conflict with a task.
- **No relays.** A landing starts what depends on it, and wakes the
  goals it overlaps. In the reference, a release by a landing often
  still needed the director.
- **No single point on x1.** The 4.4 hours that work waited on the
  director were mostly x1's freeze. In temper the engine is elsewhere,
  and the chat's run is placed again after its worker's grace.
- **Moving and parking cost nothing by hand:** no RESUME files, no
  copied notes, no rewritten briefs, no landing for an owner from
  another machine.
- **Spend per owner can be computed,** which the baseline could not do.
- **Rails:**
  - a tester cannot land;
  - a change lands only at a head CI passed;
  - authority bounds what an owner's delegates may do;
  - git credentials never sit on a worker's disk.
- **Rules reach every run** as they land.
- **The user's questions wait in one inbox,** visibly, not in a
  session on a machine that may be frozen.

## 8. What stays outside temper

- **The ground:** the machines and their health, disk floors, memory
  caps, crash capture, reboots and remounts, the runners
  (`factory.md`, section 5).
- **hearth:** build caches (kache today), build queues and lanes (heavy
  today), and outputs addressed by content, which can carry data
  between machines (`factory.md`, section 7).
- **The lab:**
  - live comparisons and benchmarks;
  - flakiness judged as a verdict;
  - findings with signatures;
  - whether a gate holds on every machine (`factory.md`, section 8).
- **An ops application:** worker health fed back to temper (6.4).
- **The user as operator:** sign-ins, servers, switching subscriptions.
  These stay the user's acts; temper only shows when one is needed.

Until those planes exist, they are the machines' environment and the
conventions that say to use it, as now.

## 9. The path

1. **Design what falls short** (section 6), as drafts in jig and temper,
   when the user agrees.
2. **GitHub and CI.** o/github's work, then CI on self-hosted runners
   with labels per machine.
3. **Replays in the lab.** The reference case comes first, as one
   story on fakes. Then each episode of section 5 is an experiment:
   - the fake LLM is scripted with the reference's judgements, the
     oracle;
   - the fake forge holds the five repositories at the reference's
     commits;
   - the checks compare triggers and mechanics with the trace.

   It extends the QA loop's experiments (`qa-loop.md`) and costs no
   LLM.
4. **The session experiment:** owners resuming against owners starting
   fresh, on the same draft (6.6).
5. **One owner moves.** A new draft's owner runs as a goal in temper.
   The director on x1 talks to it through `temper ctl`, and its trace
   is compared with the baseline. Then more owners.
6. **The director moves last,** when the chat is temper's.

The deployment that builds temper runs a release of its own, which the
run plane upgrades (`factory.md`, section 4).

## 10. Landing throughput

The design stays as it is:
- one queue per landing branch;
- a fast path for documents;
- fast runners.

Temper measures each change's wait, which today is hidden in `land`'s
retries. When skein's waits pass a bound, batching or the forge's merge
queue is turned on.

### 10.1 Today and in temper

- **Today:** `land` rebases onto main, runs the gate and pushes, and
  starts again if main moved. So landings in a repository are one at a
  time, at its exact head, but optimistically: gates run at once on
  several machines, and the losers run theirs again.
- **In temper:** CI runs on each change's own head at once, which
  catches most failures before the queue. What stays serial is one CI
  run per landing, at the head brought up to date.

  So a repository lands at most about 3600 s ÷ (gate + overhead) an
  hour, as today, without the wasted gates.

### 10.2 The numbers

| Repository | Gate, median | Gate, p90 | Landings in 25 h | Peak hour |
|---|---|---|---|---|
| skein | 96 s | 164 s | 115 | 18 |
| temper | 49 s | 81 s | 53 | 7 |
| smith | 37 s | 75 s | 46 | 7 |
| jig | about 216 s (two samples) | | 52 | 6 |
| hearth | | | 20 | 4 |

- **skein is the hot spot.** With an overhead of 30 to 60 seconds, its
  serial capacity is about 22 to 30 landings an hour at the median gate,
  and about 18 at p90.
- **The mean is easy** (4.6 an hour). The peaks run at 60 to 100% of
  capacity, so waits grow in bursts.
- **About one landing in eight failed** (37 records against 286),
  which matters for batching.

### 10.3 Approaches, cheapest first

1. **A shorter serial step:**
   - faster runners with warm caches;
   - a fast path for changes of documents alone (46 landings in skein
     and temper).

   The full gate stays at the exact head. Two changes each within a
   memory fixture's ratchet can together exceed it, and a lighter
   recheck would miss that.
2. **Batching:**
   1. The queue takes the first k ready changes onto the tip and runs
      CI once.
   2. If CI passes, it lands all k. If it fails, it bisects.

   With one failure in eight, k from 2 to 4 pays. Open in
   `../domain/forge.md`, section 20.
3. **Speculative runs, a merge train:** CI runs at once on tip+A,
   tip+A+B and tip+A+B+C, and each lands when it and everything ahead
   of it pass. A failure drops its change and restarts what is behind
   it.
   - Throughput is up to N changes per CI time, at the cost of N
     runners.
   - GitHub's merge queue does this itself, and temper already
     enqueues the exact head there (`../domain/forge.md`, section 9;
     `github.md`).
   - Handing the forge several changes at once is open (forge.md,
     section 20).
4. **Fewer, larger landings:** an owner stacks consecutive small changes
   into one. It is the coordinator's judgement, not a mechanism, and it
   trades feedback and fine reverts for queue time.

## 11. Open questions

- **The owner's charter:** its waiting threshold, its wake policy, and
  which landing news wakes it. Tuned from the replays.
- **Wake volume:** 286 landings a day in five repositories, filtered by
  overlap. Whether that holds the director and owners to the wakes they
  had.
- **Footprints:** whether detection by overlap is enough, or owners
  need grants on paths within a repository.
- **Checkpoints:** whether a convention is enough, or they need gates
  that an ancestor judges.
- **The owner's check of a diff:** whether before the branch is handed
  down, or as a gate on the change.
- **Estimates:** the director orders work by owners' estimates. Whether
  temper needs them as data, or messages and timers are enough.
- **The bound** at which batching or the forge's merge queue turns on.
- **The operator's acts:** how the user learns that a sign-in, a
  switch or a machine is needed. Today it is the director's
  escalation; in temper it would be an account's or a worker's state.
