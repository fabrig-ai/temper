//! Connected GitHub QA, confined to the world's public harness and cited recipes.
use skein_fake_forge_domain::{Observation, api as raw};
use skein_fake_forge_protocol::Route;
use skein_forge_machine_world::joint::{self, REPO};
use skein_lib::{Duration, Wall};
use temper_engine_domain_forge as forge;
use temper_engine_domain_forge_change as change;
use temper_engine_forge_world::{changes::change_row, translate::commit};
use temper_engine_protocol_forge_world::protocol::Protocol;

fn step(world: &mut Protocol, entry: u64, delegate: change::Status) -> change::Decision {
    // Public admission recipe: protocol.rs's change_step / opened_change_in.
    world.take_seen();
    world.event(forge::Event::StepChange {
        task: 51,
        entry,
        heard: change::Heard {
            lacking: None,
            spend: None,
            delegate,
            effect: change::EffectResult::None,
            released: false,
            cancelled: false,
        },
        gates: Box::new([]),
        queue_repair_active: false,
        kept: false,
    });
    world.run_for(1);
    let mut decided = None;
    let mut admission = None;
    for request in world.take_seen() {
        if let forge::Request::ChangeDecision { task: 51, decision, entry, .. } = request {
            assert!(decided.replace(decision).is_none(), "one decision per step");
            admission = entry;
        }
    }
    if let Some(entry) = admission {
        world.event(forge::Event::Committed { entry });
        world.run_for(1);
    }
    decided.expect("change step answered")
}

fn branch_name(branch: &[u8]) -> forge::Name {
    forge::Name {
        forge: REPO.forge,
        repository: REPO.repository,
        what: forge::What::Branch(branch.split(|byte| *byte == b'/').map(Box::from).collect()),
    }
}

fn enroll(world: &mut Protocol) {
    let branch = branch_name(b"temper/51");
    world.event(forge::Event::Names { task: 51, resources: Box::new([branch.clone(), branch_name(b"main")]) });
    world.event(forge::Event::Hold { task: 51, resource: branch, from: None });
    // Let enrollment's fresh resource reads finish before stepping the procedure.
    world.run_for(0);
}

fn name_pull(world: &mut Protocol) {
    let Some(forge::Stored::Change(row)) = world.stored().get(&forge::Key::Change(51)) else {
        panic!("durable change")
    };
    // QA: domain/forge.md 6, 8.2: post-pull cases require the confirmed opening.
    let number = row.pull.unwrap_or_else(|| {
        panic!("admitted opening has its pull: state {:?}, outputs {:?}", row.change.state, world.seen())
    });
    world.event(forge::Event::Rename {
        task: 51,
        resources: Box::new([
            branch_name(b"temper/51"),
            forge::Name { forge: REPO.forge, repository: REPO.repository, what: forge::What::Pull(number) },
        ]),
    });
    world.run_for(0);
}

fn prepared(world: &mut Protocol) {
    world.adopt();
    world.produce(b"temper/51");
    let mut row = change_row();
    row.change.since = world.wall();
    row.change.gates = Box::new([]);
    world.event(forge::Event::Change { row });
}

fn opened(world: &mut Protocol) {
    prepared(world);
    enroll(world);
    assert_eq!(step(world, 1, change::Status::Passed), change::Decision::Effect(change::Effect::Open));
    assert_eq!(world.writes(), 1);
    // Outcome wakes the parent; its next StepChange discovers the pull.
    // Callers supply that continuation before inspecting the durable pull.
}

fn checking(world: &mut Protocol) {
    world.adopt();
    world.produce(b"temper/51");
    let pull = world.open_pull(b"temper/51", b"main");
    let head = commit(world.branch(b"temper/51"));
    let mut row = change_row();
    row.pull = Some(pull);
    row.change.since = world.wall();
    row.change.last_head = Some(head);
    row.change.state = change::State::Checking { head };
    world.event(forge::Event::Change { row });
    enroll(world);
    name_pull(world);
    assert!(matches!(step(world, 1, change::Status::Unknown), change::Decision::Wait { .. }));
}

fn delayed_checks(seconds: u64) -> raw::Checks {
    let mut checks = joint::checks(1000);
    checks.latency_min = Duration::from_secs(seconds);
    checks.latency_max = checks.latency_min;
    checks
}

fn assert_forge_passed_and_change_woke(world: &Protocol) {
    let head = world.branch(b"temper/51");
    assert!(
        world.observations().iter().any(|observation| matches!(observation,
            Observation::Reported { commit, state: raw::Check::Passed, .. } if *commit == head
        )),
        "precondition: independent CI must actually pass this head"
    );
    assert!(
        world.seen().iter().any(|request| matches!(request, forge::Request::WakeProcedure { task: 51 })),
        "QA: protocol/forge.md 10: CI completion must wake its enrolled change"
    );
}

// The scheduler is the public recipe in protocol.rs:787-879: act only on a
// WakeProcedure or Wait.until. Never poll StepChange to discover CI ourselves.
fn scheduled_landing(world: &mut Protocol, first_entry: u64, seconds: u64) -> Vec<[u8; 32]> {
    let deadline = Wall::from_nanos(world.wall().as_nanos() + Duration::from_secs(seconds).as_nanos());
    let mut entry = first_entry;
    let mut due = true;
    let mut until = None;
    let mut finishes = Vec::new();
    let mut named_pull = false;
    while world.wall() <= deadline {
        if !named_pull
            && let Some(forge::Stored::Change(row)) = world.stored().get(&forge::Key::Change(51))
            && row.pull.is_some()
        {
            name_pull(world);
            named_pull = true;
        }
        for request in world.take_seen() {
            if let forge::Request::WakeProcedure { task: 51 } = request {
                due = true;
            } else if let forge::Request::ChangeDecision { task: 51, decision, entry: admission, .. } = request {
                if let Some(entry) = admission {
                    world.event(forge::Event::Committed { entry });
                }
                match decision {
                    change::Decision::Wait { until: at } => until = at,
                    change::Decision::Hold(why) => panic!("QA: healthy path held: {why:?}"),
                    change::Decision::Finish { merge } => finishes.push(merge),
                    change::Decision::Delegate(delegate) => panic!("unexpected delegate: {delegate:?}"),
                    change::Decision::CancelGates { .. } => panic!("unexpected gate cancellation"),
                    change::Decision::None
                    | change::Decision::Effect(_)
                    | change::Decision::Ready
                    | change::Decision::QueueRepair
                    | change::Decision::Cancel => due = true,
                }
            }
        }
        if !finishes.is_empty() {
            break;
        }
        if until.is_some_and(|at| world.wall() >= at) {
            due = true;
        }
        if due {
            due = false;
            until = None;
            world.event(forge::Event::StepChange {
                task: 51,
                entry,
                heard: change::Heard {
                    lacking: None,
                    spend: None,
                    delegate: change::Status::Unknown,
                    effect: change::EffectResult::None,
                    released: false,
                    cancelled: false,
                },
                gates: Box::new([]),
                queue_repair_active: false,
                kept: false,
            });
            entry += 1;
            world.run_for(0);
        } else {
            world.run_for(1);
        }
    }
    assert_eq!(finishes.len(), 1, "QA: domain/forge.md 8.2: confirming read must Finish by {deadline:?}");
    finishes
}

fn landed_once(world: &Protocol, finishes: &[[u8; 32]], expected_writes: u32) {
    assert_eq!(
        finishes,
        [commit(world.branch(b"main"))],
        "QA: domain/forge.md 8.2: Finish names main's actual landing commit"
    );
    assert_eq!(world.writes(), expected_writes, "QA: domain/forge.md 6: no effect lost or repeated");
    let merges: Vec<_> = world.observations().iter().filter(|o| matches!(o, Observation::Merged { .. })).collect();
    assert!(
        matches!(merges.as_slice(), [Observation::Merged { head, commit: merge, .. }]
        if *head == world.branch(b"temper/51") && *merge == world.branch(b"main")),
        "QA: merge uses the checked head, and Finish uses the resulting commit: {merges:?}"
    );
    let checked = world
        .observations()
        .iter()
        .position(|o| {
            matches!(o,
                Observation::Reported { commit, state: raw::Check::Passed, .. }
                    if *commit == world.branch(b"temper/51")
            )
        })
        .expect("CI passed the merged head");
    let landing_at =
        world.observations().iter().position(|o| matches!(o, Observation::Merged { .. })).expect("merge observed");
    assert!(checked < landing_at, "QA: domain/forge.md 8.2: CI must pass before the merge");
    assert_eq!(world.tally().routes.get(&Route::Merge), Some(&1));
    assert!(!world.observations().iter().any(|o| matches!(o, Observation::Enqueued { .. })));
    assert!(
        !world.requests().any(|(_, line)| line.windows(5).any(|s| s == b"/jobs")),
        "QA: domain/forge.md 8.4: passing CI needs no Job or JobLog"
    );
    assert!(world.token_kept());
}

#[test]
fn failed_ci_requests_repair_without_merge() {
    // QA: domain/forge.md 8.2-8.4: a failed head is repaired before landing.
    let mut world = Protocol::github_with_checks(749, joint::checks(0));
    opened(&mut world);
    let failed_head = world.branch(b"temper/51");
    assert!(world.observations().iter().any(|o| matches!(o,
        Observation::Reported { commit, state: raw::Check::Failed, .. } if *commit == failed_head
    )));
    let mut repaired = false;
    for entry in 2..10 {
        if step(&mut world, entry, change::Status::Unknown)
            == change::Decision::Delegate(change::Delegate::Repair(change::Repair::Ci))
        {
            repaired = true;
            break;
        }
    }
    assert!(repaired, "QA: failed CI must delegate a CI repair");
    let Some(forge::Stored::Change(row)) = world.stored().get(&forge::Key::Change(51)) else {
        panic!("durable change")
    };
    assert_eq!(row.change.repairs, 1);
    assert_eq!(world.writes(), 1, "only the admitted opening; no merge before repair");
    assert_eq!(world.tally().routes.get(&Route::Merge), None);
    assert_eq!(world.tally().routes.get(&Route::PostStatus), None);
    world.finish();
}

#[test]
fn pending_pull_wakes_after_ci_verdict() {
    // QA: protocol/forge.md 10; draft/github.md 7: changed CI wakes its waiter.
    let mut limits = temper_engine_forge_world::LIMITS;
    limits.change_policy.stall = Duration::from_secs(1800);
    let mut world = Protocol::github_with_limits(747, delayed_checks(90), &limits);
    checking(&mut world);
    world.take_seen();
    world.run_for(120);
    assert_forge_passed_and_change_woke(&world);
    assert_eq!(world.writes(), 0, "watching must never write to the connector");
    world.finish();
}

#[test]
fn pending_pull_survives_token_renewal() {
    // QA: domain/testing.md 2.2; draft/github.md 11 P1, 17: watch through the hour.
    let mut limits = temper_engine_forge_world::LIMITS;
    // Deliberate long-stall story; shipped silent-CI stall remains 1800 s.
    limits.change_policy.stall = Duration::from_secs(7200);
    let mut world = Protocol::github_with_limits(748, delayed_checks(3601), &limits);
    checking(&mut world);
    world.take_seen();
    world.run_for(3720);
    assert!(mints(&world) >= 4, "both API and git tokens must renew");
    assert_forge_passed_and_change_woke(&world);
    assert_eq!(world.writes(), 0);
    assert!(world.token_kept());
    world.finish();
}

#[test]
fn pushed_change_requests_its_pull_once() {
    // QA: domain/forge.md 6, 8.2: opening waits for parent admission.
    let mut world = Protocol::github(741);
    prepared(&mut world);
    world.take_seen();
    world.event(forge::Event::StepChange {
        task: 51,
        entry: 1,
        heard: change::Heard {
            lacking: None,
            spend: None,
            delegate: change::Status::Passed,
            effect: change::EffectResult::None,
            released: false,
            cancelled: false,
        },
        gates: Box::new([]),
        queue_repair_active: false,
        kept: false,
    });
    world.run_for(1);
    let decisions: Vec<_> =
        world
            .seen()
            .iter()
            .filter_map(|request| {
                if let forge::Request::ChangeDecision { decision, .. } = request { Some(decision) } else { None }
            })
            .collect();
    assert_eq!(decisions, [&change::Decision::Effect(change::Effect::Open)]);
    assert_eq!(world.writes(), 0, "unadmitted effects cannot write");
    world.finish();
}

#[test]
fn direct_landing_finish_and_two_writes() {
    // QA: domain/forge.md 6, 8.2, 8.7; protocol/forge.md 10; Q8.
    let mut world = Protocol::github(742);
    opened(&mut world);
    let finishes = scheduled_landing(&mut world, 2, 30);
    landed_once(&world, &finishes, 2);
    world.finish();
}

#[test]
#[ignore = "needs: ruleset knob on the GitHub joint"]
fn ruleset_refuses_merge_without_landing() {
    // QA: skein forge-github.md 7 (405 rule violation -> Protected); Q3.
    // Missing Setup.protection must expose a required approval on main,
    // no bot-9 bypass, and enforce it at merge with the real 405 mapping.
    let mut world = Protocol::github(743);
    opened(&mut world);
    let mut held = false;
    for entry in 2..20 {
        match step(&mut world, entry, change::Status::Unknown) {
            change::Decision::Hold(_) => {
                held = true;
                break;
            }
            change::Decision::Finish { .. } => panic!("QA: protected merge cannot Finish"),
            change::Decision::None
            | change::Decision::Wait { .. }
            | change::Decision::Delegate(_)
            | change::Decision::CancelGates { .. }
            | change::Decision::Effect(_)
            | change::Decision::Ready
            | change::Decision::QueueRepair
            | change::Decision::Cancel => {}
        }
    }
    assert!(held, "QA: ruleset refusal must hold the change");
    assert!(!world.observations().iter().any(|o| matches!(o, Observation::Merged { .. })));
    world.finish();
}

fn conflicting_update(world: &mut Protocol) -> u64 {
    opened(world);
    world.push(b"main", b"incompatible base edit");
    let base = commit(world.branch(b"main"));
    let mut resolution_entry = None;
    let mut updated = false;
    for entry in 2..20 {
        let decision = step(world, entry, change::Status::Unknown);
        updated |= matches!(decision, change::Decision::Effect(change::Effect::Update { .. }));
        if decision == change::Decision::Delegate(change::Delegate::Resolve { base }) {
            resolution_entry = Some(entry);
            break;
        }
        assert!(!matches!(decision, change::Decision::Finish { .. }), "QA: unresolved conflict cannot land");
    }
    assert!(updated, "QA: update must be admitted before resolution");
    assert!(world.requests().any(|(_, line)| line.starts_with(b"PUT /repos/acme/widgets/pulls/1/update-branch ")));
    assert_eq!(world.writes(), 2, "opening and refused update only");
    assert_eq!(world.tally().routes.get(&Route::Merge), None);
    let Some(forge::Stored::Change(row)) = world.stored().get(&forge::Key::Change(51)) else {
        panic!("durable conflicting change")
    };
    assert_eq!(row.change.resolutions, 1);
    resolution_entry.expect("QA: update-conflict must delegate resolution")
}

#[test]
fn conflicting_update_requests_resolution_without_merge() {
    // QA: domain/forge.md 8.2, 8.4, 10; skein forge-github.md 7; Q3 prefix.
    let mut world = Protocol::github(750);
    conflicting_update(&mut world);
    world.finish();
}

#[test]
fn update_conflict_resolved_then_merged() {
    // QA: domain/forge.md 8.2, 8.4, 10; Q3. Resolve as the fixture writer.
    let mut world = Protocol::github(744);
    let entry = conflicting_update(&mut world);
    world.resolve(b"temper/51", b"resolved both edits");
    assert!(!matches!(step(&mut world, entry + 1, change::Status::Passed), change::Decision::Hold(_)));
    let finishes = scheduled_landing(&mut world, entry + 2, 30);
    landed_once(&world, &finishes, 3);
    world.finish();
}

fn mints(world: &Protocol) -> usize {
    world.requests().filter(|(_, line)| line.starts_with(b"POST /app/installations/1/access_tokens ")).count()
}

#[test]
fn pending_ci_across_token_hour_completes_on_fresh_token() {
    // QA: domain/testing.md 2.2; draft/github.md 11 P1, 17; Q4.
    let mut limits = temper_engine_forge_world::LIMITS;
    // Explicitly extend this story's stall; do not claim silent CI may wait
    // out an hour under the shipped 1800 s policy.
    limits.change_policy.stall = Duration::from_secs(7200);
    let mut world = Protocol::github_with_limits(745, delayed_checks(3601), &limits);
    opened(&mut world);
    let before = mints(&world);
    let opened_at = world.wall();
    assert!(
        world.observations().iter().any(|o| matches!(o,
            Observation::Reported { commit, state: raw::Check::Pending, .. }
                if *commit == world.branch(b"temper/51")
        )),
        "precondition: CI is pending before renewal"
    );
    let finishes = scheduled_landing(&mut world, 2, 3720);
    assert!(
        world.wall().as_nanos() - opened_at.as_nanos() >= Duration::from_secs(3600).as_nanos(),
        "QA: the landing must cross the initial tokens' hour"
    );
    assert!(mints(&world) >= before + 2, "API and git credentials renewed while CI was pending");
    assert!(world.observations().iter().any(|o| matches!(o,
        Observation::Reported { commit, state: raw::Check::Passed, .. } if *commit == world.branch(b"temper/51")
    )));
    landed_once(&world, &finishes, 2);
    world.finish();
}

#[test]
#[ignore = "needs: cut after applied merge before confirming read on the GitHub joint"]
fn restart_after_merge_before_read_never_merges_twice() {
    // QA: domain/forge.md 6, 16; protocol/forge.md 10; Q5.
    let mut world = Protocol::github(746);
    opened(&mut world);
    // Missing joint control must be armed here, to stop an admitted merge
    // after apply and before answer/confirming read, preserving its entry.
    let mut requested = false;
    for entry in 2..20 {
        if matches!(
            step(&mut world, entry, change::Status::Unknown),
            change::Decision::Effect(change::Effect::Merge { .. })
        ) {
            requested = true;
            break;
        }
    }
    assert!(requested, "precondition: the merge was admitted");
    assert_eq!(
        world.observations().iter().filter(|o| matches!(o, Observation::Merged { .. })).count(),
        1,
        "cut must follow an applied merge"
    );
    assert!(
        world.stored().iter().any(|(_, row)| matches!(row, forge::Stored::Entry { .. })),
        "cut must preserve the outstanding merge entry"
    );
    let writes = world.writes();
    world.restart();
    let finishes = scheduled_landing(&mut world, 20, 30);
    assert_eq!(world.writes(), writes, "QA: recovery must find the merge without another write");
    landed_once(&world, &finishes, 2);
    world.finish();
}
