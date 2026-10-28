//! Reference case 4.4: scripted producers, a parked owner and a runner lost once.
use crate::forge_routes::{Change, Gate, StoryLimits, World, change_world_in, produce, produce_content};
use crate::protocol;
use jig_domain_host as host;
use jig_domain_tasks as tasks;
use skein_fake_forge_domain::api as raw;
use skein_lib::Token;
use std::collections::BTreeSet;
use temper_engine_domain as engine;
use temper_engine_domain::{Key, Record};
use temper_engine_domain_forge as forge;
use temper_engine_forge_world::REPO;

const SETTLE_TICKS: u32 = 600; // the forge routes' focused settling bound.
const OWNER: Token = Token::new(7);
const ALL_FAULTS: u32 = 1000; // the fake's probability scale.

fn change(world: &World, producer: &engine::Assignment) -> u64 {
    let Some(Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Live(row)))) =
        world.store.rows.get(&Key::Core(jig_domain_core::Key::Tasks(tasks::Key::Live(producer.task))))
    else {
        panic!("producer is live")
    };
    let tasks::Party::Task(change) = row.requester else { panic!("change owns its producer") };
    change
}
fn tier(world: &World) -> &forge::TierRow {
    let Some(Record::Forge { row }) =
        world.store.rows.get(&Key::Forge(forge::Key::Tier { repository: REPO, tier: b"browser".as_slice().into() }))
    else {
        panic!("browser tier exists")
    };
    let forge::Stored::Tier(row) = row.as_ref() else { panic!("tier row") };
    row
}
pub fn park_owner(world: &mut World, run: &engine::Assignment) {
    answer_owner(world, run, tasks::End::Parked);
}
fn answer_owner(world: &mut World, run: &engine::Assignment, end: tasks::End) {
    let read = run.messages.iter().map(|word| word.number).max();
    if read.is_some() {
        world.send(protocol::turn(
            OWNER,
            run.task,
            run.attempt,
            1,
            0,
            read,
            b"Read landing news; wait for the change result",
        ));
        for _ in 0..20 {
            world.tick();
        }
    }
    let mut event = world.protocol.answer(OWNER, run.task, run.attempt, 0, end, engine::Parts::none());
    let engine::Event::Worker(engine::WorkerInput::Link {
        link: engine::Link::Up { up: host::LinkUp::Answer { answer, .. }, .. },
        ..
    }) = &mut event
    else {
        panic!("scripted worker answer")
    };
    answer.read = read.map(Token::new);
    answer.turns = u32::from(read.is_some());
    world.send(event);
}
fn owner_tick(world: &mut World, o1: &engine::Assignment, k1: u64, parked: &mut BTreeSet<u64>) {
    world.tick();
    let wakes: Vec<_> = world
        .assigned
        .iter()
        .filter(|run| {
            run.task == o1.task
                && !parked.contains(&run.attempt)
                && !run.messages.iter().any(|word| {
                    word.from == tasks::Party::Task(k1) && matches!(word.kind, tasks::MessageKind::Result(_))
                })
        })
        .cloned()
        .collect();
    for run in wakes {
        parked.insert(run.attempt);
        park_owner(world, &run);
    }
}
fn ended(world: &World, task: u64) -> bool {
    matches!(world.store.rows.get(&Key::Core(jig_domain_core::Key::Tasks(tasks::Key::Ended(task)))), Some(Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Ended(row)))) if matches!(row.phase, tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Change { .. })) ))
}
/// Every agent judgement is an oracle: both producers return the checked Change,
/// and O1 parks until its child's terminal words. The independent forge decides runs.
#[expect(
    clippy::too_many_lines,
    reason = "one reference story follows both landings through the lost runner and O1's wake"
)]
pub fn case2_4_4() {
    let world = World::configured_limits(
        true,
        true,
        false,
        1000,
        Gate::default(),
        None,
        StoryLimits { tier: true, ..StoryLimits::default() },
    );
    let (mut world, o1, k1_producer, k1_branch) =
        change_world_in(world, Change { no_ci: false, owner_gate: false, tracked: true });
    let k1 = change(&world, &k1_producer);
    let mut parked = BTreeSet::from([o1.attempt]);
    park_owner(&mut world, &o1);
    produce(&mut world, &k1_producer, &k1_branch);
    for _ in 0..SETTLE_TICKS {
        owner_tick(&mut world, &o1, k1, &mut parked);
        if world.merges == 1 {
            break;
        }
    }
    let (mut world, _p, p1_producer, p1_branch) =
        change_world_in(world, Change { no_ci: false, owner_gate: false, tracked: false });
    let p1 = change(&world, &p1_producer);
    produce_content(&mut world, &p1_producer, &p1_branch, b"P changes the file again");
    for _ in 0..SETTLE_TICKS {
        owner_tick(&mut world, &o1, k1, &mut parked);
        if world.merges == 2 {
            break;
        }
    }
    assert_eq!(
        world.merges,
        2,
        "two merges at {:?}; changes {:?}; tasks {:?}",
        world.env.wall,
        world
            .store
            .rows
            .values()
            .filter_map(|stored| if let Record::Forge { row } = stored {
                if let forge::Stored::Change(row) = row.as_ref() {
                    Some((row.task, row.change.state.clone()))
                } else {
                    None
                }
            } else {
                None
            })
            .collect::<Vec<_>>(),
        world
            .store
            .rows
            .values()
            .filter_map(|stored| {
                if let Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Live(row))) = stored {
                    Some((row.number, row.phase.clone()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    );
    assert_eq!(tier(&world).record.run.as_ref().expect("baseline").number, 1);
    assert!(!ended(&world, k1));
    for _ in 0..SETTLE_TICKS * 3 {
        owner_tick(&mut world, &o1, k1, &mut parked);
        if tier(&world).record.run.as_ref().is_some_and(|run| run.number == 2) {
            break;
        }
    }
    assert_eq!(tier(&world).record.run.as_ref().expect("range run").landings.len(), 2);
    // The fake's lost draw represents its runner's restart, producing a no-step fault.
    world.fake_env.limits.lost = ALL_FAULTS;
    for _ in 0..SETTLE_TICKS * 3 {
        owner_tick(&mut world, &o1, k1, &mut parked);
        if tier(&world).record.run.as_ref().is_some_and(|run| run.faults == 1) {
            break;
        }
    }
    assert_eq!(tier(&world).record.run.as_ref().expect("retry").faults, 1, "tier progress: {:?}", tier(&world));
    assert!(!ended(&world, k1), "a runner fault gives no change result");
    world.fake_env.limits.lost = 0;
    for _ in 0..SETTLE_TICKS * 3 {
        owner_tick(&mut world, &o1, k1, &mut parked);
    }
    assert_eq!(tier(&world).record.sequence, 2);
    assert!(ended(&world, k1));
    assert!(ended(&world, p1));
    assert!(
        world.assigned.iter().any(|run| run.task == o1.task
            && run.attempt > o1.attempt
            && run
                .messages
                .iter()
                .any(|word| matches!(word.kind, tasks::MessageKind::Result(tasks::ResultKind::Change { .. }))
                    && word.from == tasks::Party::Task(k1))),
        "O1's parked wait ends with k1's result: {:?}",
        world.store.rows.get(&Key::Core(jig_domain_core::Key::Tasks(tasks::Key::Live(o1.task))))
    );
    let raw::Answer::Commit(tip) =
        world.external(1, raw::Op::Read(raw::Read::Branch { branch: b"main".as_slice().into() }))
    else {
        panic!("main tip")
    };
    let raw::Answer::Statuses(statuses) = world.external(1, raw::Op::Read(raw::Read::Statuses { commit: tip })) else {
        panic!("tip statuses")
    };
    assert!(
        statuses
            .iter()
            .any(|status| status.context.as_ref() == b"temper/verdict/browser" && status.state == raw::Check::Passed)
    );
    for branch in [b"temper/tier/browser.1".as_slice(), b"temper/tier/browser.2".as_slice()] {
        assert!(
            world
                .fake
                .inspect(&world.fake_env.limits, b"org/repo", &raw::Read::Branch { branch: branch.into() })
                .is_err()
        );
    }
}

/// The durable cuts T9 grows: time, member pulls, held progress, release and red.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BoundCut {
    Waiting,
    Uncleared,
    LogPulls,
    HeldStatus,
    Released,
    Red,
}
const BOUND: skein_lib::Duration = skein_lib::Duration::from_secs(360);
const RUN_TICKS: u32 = 1800; // The tier workflow's 90 s plus polling and effect admission.
const BOUND_TICKS: u32 = 7000; // 360 s bound + two 90 s runs and polling, at 100 ms/tick.
const BOUND_RESULT_BYTES: u32 = 100; // SHA-1 (40), newline (1), and the 59-byte uncleared reason.
const BOUND_FAILURE: &[u8] = b"suite.runner_returned\n";
fn change_row(world: &World, task: u64) -> &forge::ChangeRow {
    let Some(Record::Forge { row }) = world.store.rows.get(&Key::Forge(forge::Key::Change(task))) else {
        panic!("retained change")
    };
    let forge::Stored::Change(row) = row.as_ref() else { panic!("change row") };
    row
}
fn live(world: &World, task: u64) -> Option<&tasks::TaskRecord> {
    match world.store.rows.get(&Key::Core(jig_domain_core::Key::Tasks(tasks::Key::Live(task)))) {
        Some(Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Live(row)))) => Some(row),
        _ => None,
    }
}
fn bound_cold(world: &mut World, task: u64, cut: BoundCut) {
    let since = change_row(world, task).change.since;
    world.store.crash(71); // The root world's fixed deployment seed.
    world.restart(true, true);
    for _ in 0..SETTLE_TICKS {
        world.tick();
        assert!(
            world.root.restart_failure().is_none(),
            "cold {cut:?}: {:?}, {:?}",
            world.root.restart_failure(),
            world.root.forge_restore_failure()
        );
        if world.root.ready() {
            assert_eq!(change_row(world, task).change.since, since, "cold {cut:?} keeps the landing clock");
            return;
        }
    }
    panic!("cold {cut:?} opens: {:?}", world.root.restart_step());
}
/// Runner faults hold the tier; a bounded change ends and its goal hears it.
/// The goal finishes before the runner returns, so its late red awaits findings.
#[expect(
    clippy::too_many_lines,
    reason = "one bounded story follows the runner outage, result, person's release and late red"
)]
pub fn bound_story(cut: Option<BoundCut>) {
    let world = World::configured_limits(
        true,
        true,
        false,
        1000,
        Gate::default(),
        None,
        StoryLimits {
            tier: true,
            tier_reports: true,
            tier_bound: Some(BOUND),
            change_words: Some(BOUND_RESULT_BYTES),
            ..StoryLimits::default()
        },
    );
    let (mut world, owner, producer, branch) =
        change_world_in(world, Change { no_ci: false, owner_gate: false, tracked: true });
    let task = change(&world, &producer);
    let mut parked = BTreeSet::from([owner.attempt]);
    park_owner(&mut world, &owner);
    crate::forge_routes::produce_path_content(&mut world, &producer, &branch, b"failures", BOUND_FAILURE);
    for _ in 0..SETTLE_TICKS {
        owner_tick(&mut world, &owner, task, &mut parked);
        if world.merges == 1
            && matches!(change_row(&world, task).change.state, temper_engine_domain_forge_change::State::Landed { .. })
            && world.store.pending.is_empty()
        {
            break;
        }
    }
    assert_eq!(world.merges, 1);
    let temper_engine_domain_forge_change::State::Landed { merge } = change_row(&world, task).change.state else {
        panic!("verifying: {:?}", change_row(&world, task).change.state)
    };
    let pull = change_row(&world, task).pull.expect("landed pull");
    let Some(Record::Forge { row }) = world.store.rows.get(&Key::Forge(forge::Key::LandingLog(REPO))) else {
        panic!("log")
    };
    let forge::Stored::LandingLog(log) = row.as_ref() else { panic!("landing log") };
    assert!(log.landings.iter().any(|entry| entry.commit == merge && entry.changes.contains(&(task, pull))));
    if matches!(cut, Some(BoundCut::Waiting | BoundCut::LogPulls)) {
        bound_cold(&mut world, task, cut.expect("story state exists"));
    }
    // Let the original baseline finish green before faulting the runner.
    for _ in 0..RUN_TICKS {
        owner_tick(&mut world, &owner, task, &mut parked);
        if tier(&world).record.sequence >= 1 {
            break;
        }
    }
    assert_eq!(tier(&world).record.sequence, 1, "baseline: {:?}", tier(&world));
    world.fake_env.limits.lost = ALL_FAULTS;
    if cut == Some(BoundCut::HeldStatus) {
        world.hold_write_answers = Some(crate::forge_routes::HoldWrite::TierHold);
    }
    let mut held_cut = false;
    for _ in 0..BOUND_TICKS {
        owner_tick(&mut world, &owner, task, &mut parked);
        let row = tier(&world);
        if cut == Some(BoundCut::HeldStatus)
            && !held_cut
            && matches!(row.held.as_ref().map(|held| held.status), Some(forge::TierBranch::Pending { .. }))
            && world.store.pending.is_empty()
        {
            bound_cold(&mut world, task, BoundCut::HeldStatus);
            held_cut = true;
        }
        let row = tier(&world);
        if let Some(tier_task) = row.task
            && matches!(
                live(&world, tier_task).map(|task| &task.phase),
                Some(tasks::Phase::Held { why: tasks::Hold::Effects, .. })
            )
        {
            break;
        }
    }
    assert!(cut != Some(BoundCut::HeldStatus) || held_cut);
    let tier_task = tier(&world).task.expect("held task");
    assert!(
        matches!(
            live(&world, tier_task).map(|task| &task.phase),
            Some(tasks::Phase::Held { why: tasks::Hold::Effects, .. })
        ),
        "tier held: {:?}",
        tier(&world)
    );
    let words = &live(&world, tier_task).expect("story state exists").spec.words;
    assert_eq!(words.as_ref(), b"Tier browser run 2");
    assert_eq!(tier(&world).record.run.as_ref().expect("story state exists").faults, 3);
    for _ in 0..BOUND_TICKS {
        owner_tick(&mut world, &owner, task, &mut parked);
        if ended(&world, task) && world.store.pending.is_empty() {
            break;
        }
    }
    assert!(ended(&world, task));
    assert!(matches!(
        change_row(&world, task).change.state,
        temper_engine_domain_forge_change::State::Uncleared { .. }
    ));
    if cut == Some(BoundCut::Uncleared) {
        bound_cold(&mut world, task, BoundCut::Uncleared);
    }
    let mut result_run = None;
    for _ in 0..SETTLE_TICKS {
        owner_tick(&mut world, &owner, task, &mut parked);
        result_run = world
            .assigned
            .iter()
            .rev()
            .find(|run| {
                run.task == owner.task
                    && run.messages.iter().any(|word| {
                        word.from == tasks::Party::Task(task) && matches!(word.kind, tasks::MessageKind::Result(_))
                    })
            })
            .cloned();
        if result_run.is_some() {
            break;
        }
    }
    let result_run = result_run.expect("goal hears its change's result");
    assert!(result_run.messages.iter().any(|word| {
        word.words
            .windows(b"landed, uncleared: no verdict from browser within the bound".len())
            .any(|words| words == b"landed, uncleared: no verdict from browser within the bound")
    }));
    answer_owner(
        &mut world,
        &result_run,
        tasks::End::Finished {
            result: tasks::TaskResult::Report { words: b"landed and uncleared".as_slice().into() },
            cancel_delegates: false,
        },
    );
    for _ in 0..SETTLE_TICKS {
        world.tick();
        if live(&world, owner.task).is_none() && world.store.pending.is_empty() {
            break;
        }
    }
    assert!(live(&world, owner.task).is_none(), "goal ends after its result");
    world.fake_env.limits.lost = 0;
    world.signed_in = None; // The world's sign-in lasts 60 s; the outage lasted longer.
    world.sign_in();
    let revision = live(&world, tier_task).expect("held tier task").revisions.hold;
    world.send(engine::Event::Party(engine::PartyInput::Ask {
        models: Box::new([]),
        reply_to: skein_lib::ReplyTo::new(Token::new(950)),
        sign_in: world.signed_in.expect("signed-in person"),
        key: [95; 16],
        ask: jig_domain_people::Ask::Release { project: 1, task: tier_task, revision, again: None },
    }));
    for _ in 0..SETTLE_TICKS {
        world.tick();
        if tier(&world).held.is_none()
            && tier(&world).record.run.as_ref().is_some_and(|run| run.faults == 0)
            && world.store.pending.is_empty()
        {
            break;
        }
    }
    assert!(
        tier(&world).held.is_none(),
        "person releases: {:?}; core {:?}",
        tier(&world),
        live(&world, tier_task).map(|task| (&task.phase, task.revisions.hold, task.due_reason, &task.escalation))
    );
    assert_eq!(tier(&world).record.run.as_ref().expect("story state exists").faults, 0);
    if cut == Some(BoundCut::Released) {
        bound_cold(&mut world, task, BoundCut::Released);
    }
    for _ in 0..BOUND_TICKS {
        world.tick();
        if !tier(&world).reds.is_empty() && world.store.pending.is_empty() {
            break;
        }
    }
    assert_eq!(tier(&world).reds.len(), 1, "late red retained: {:?}", tier(&world));
    assert_eq!(tier(&world).reds[0].landing, merge);
    assert_eq!(
        tier(&world).reds[0].blame,
        temper_engine_domain_forge_tiers::Blame::Owns(temper_engine_domain_forge_tiers::Failures::Tests(Box::new([
            b"suite.runner_returned".as_slice().into()
        ])))
    );
    if cut == Some(BoundCut::Red) {
        bound_cold(&mut world, task, BoundCut::Red);
        assert_eq!(tier(&world).reds[0].landing, merge);
    }
}
pub fn case2_5_4() {
    bound_story(None);
}
