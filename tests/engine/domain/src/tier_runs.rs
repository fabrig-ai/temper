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
fn park_owner(world: &mut World, run: &engine::Assignment) {
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
    let mut event = world.protocol.answer(OWNER, run.task, run.attempt, 0, tasks::End::Parked, engine::Parts::none());
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
