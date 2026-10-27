//! A sent attempt's own row, through the real root/store commit gate.
use super::*;
use skein_forge_client::{Condition, Effect, Entry};

// At 100ms a turn, 200 turns cover the client's 10s lifetime plus retry and store routing.
const SETTLEMENT_TURNS: u32 = 200;
// One second covers a decoded answer's decision and its commit.
const COMMIT_TURNS: u32 = 10;
const STATUS_ENTRY: u64 = 100; // Beyond the setup's one named issue entry.

pub(super) fn rows(world: &World) -> Vec<(u64, u64)> {
    world
        .store
        .rows
        .values()
        .filter_map(|record| match record {
            Record::Forge { row } => match row.as_ref() {
                forge_top::Stored::Flight { task, entry } => Some((*task, *entry)),
                _ => None,
            },
            Record::Core(_) => None,
        })
        .collect()
}
fn entry(world: &World, number: u64) -> Option<(&Entry, bool)> {
    match world.store.rows.get(&Key::Forge(forge_top::Key::Entry(number))) {
        Some(Record::Forge { row }) => match row.as_ref() {
            forge_top::Stored::Entry { entry, held } => Some((entry, *held)),
            _ => panic!("entry key names another family"),
        },
        _ => None,
    }
}
fn wait_for_answer(world: &mut World) {
    for _ in 0..SETTLEMENT_TURNS {
        world.tick();
        if !world.held_write_answers.is_empty() {
            return;
        }
    }
    panic!("write answer never reached the held cut");
}
fn commit_answer(world: &mut World) {
    world.release_write_answers();
    for _ in 0..COMMIT_TURNS {
        world.tick();
    }
}
fn assert_flight(world: &World, task: u64, number: u64) {
    assert_eq!(rows(world), [(task, number)]);
    assert!(entry(world, number).expect("flight's entry remains").0.attempt.is_some());
}
#[test]
fn a_forge_write_in_flight_keeps_its_row_until_settled() {
    let (mut world, assignment) = pending_issue_effect();
    world.lose_writes = 1;
    // A lost unrecoverable creation remains in flight until the deadline holds it.
    let mut issue = None;
    for _ in 0..SETTLEMENT_TURNS {
        world.tick();
        issue = rows(&world).first().map(|(_, number)| *number);
        if issue.and_then(|number| entry(&world, number)).is_some_and(|(row, held)| row.failures > 0 && !held) {
            break;
        }
    }
    let issue = issue.expect("attempt's Flight row committed before uncertain outcome");
    assert_flight(&world, assignment.task, issue);
    for _ in 0..SETTLEMENT_TURNS {
        world.tick();
        if entry(&world, issue).is_some_and(|(_, held)| held) {
            break;
        }
    }
    assert!(entry(&world, issue).expect("held entry remains").1);
    assert!(rows(&world).is_empty(), "holding settles the flight for a person");
    world.hold_write_answers = Some(HoldWrite::All);
    world.send(engine::Event::Forge(Box::new(forge_top::Event::Resolve { entry: issue, again: true })));
    wait_for_answer(&mut world);
    assert_flight(&world, assignment.task, issue);
    commit_answer(&mut world);
    assert!(rows(&world).is_empty(), "the remade creation settled");

    // An idempotent write can be retried after an uncertain attempt.
    world.lose_writes = 1;
    world.hold_write_answers = Some(HoldWrite::All);
    world.send(engine::Event::Forge(Box::new(forge_top::Event::Enqueue {
        entry: Entry {
            number: STATUS_ENTRY,
            task: assignment.task,
            repository: forge_world::REPO,
            effect: Effect {
                write: skein_forge::api::Write::Status {
                    commit: translate::commit(1),
                    context: Box::from(b"flight-test".as_slice()),
                    check: skein_forge::api::Check::Passed,
                    description: Box::new([]),
                    link: Box::new([]),
                },
                condition: Condition::None,
            },
            start: None,
            attempt: None,
            failures: 0,
            queued: false,
        },
    })));
    let mut first_attempt = None;
    for _ in 0..SETTLEMENT_TURNS {
        world.tick();
        if let Some((row, false)) = entry(&world, STATUS_ENTRY)
            && row.failures > 0
        {
            first_attempt = row.attempt;
            break;
        }
    }
    assert_flight(&world, assignment.task, STATUS_ENTRY);
    wait_for_answer(&mut world);
    assert_flight(&world, assignment.task, STATUS_ENTRY);
    let retried = entry(&world, STATUS_ENTRY).expect("retry entry").0;
    assert!(retried.attempt.expect("retry attempt").sent > first_attempt.expect("uncertain attempt").sent);
    commit_answer(&mut world);
    assert!(rows(&world).is_empty(), "the settling decision erased the retry's flight");
}

#[test]
fn a_crash_keeps_the_flight_until_restart_settles_it() {
    let (mut world, assignment, number) = held_issue_write();
    assert_flight(&world, assignment.task, number);
    // The fake made the write; its answer belongs to the crashed process.
    world.restart(true, false);
    assert_eq!(rows(&world), [(assignment.task, number)], "the crash erased no row");
    for _ in 0..SETTLEMENT_TURNS {
        world.tick();
        if rows(&world).is_empty() {
            break;
        }
    }
    assert!(rows(&world).is_empty());
    assert!(entry(&world, number).is_none());
    assert_eq!(issue_count(&mut world), 1, "restart found the original creation");
}
