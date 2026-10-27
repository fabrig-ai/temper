//! A host's typed bound crosses the engine and holds without a retry.
use super::*;

pub(super) fn meets_bound(driver: &mut Driver, run: &engine::Assignment, bound: host::Bound) {
    let answer = host::Answer {
        read: None,
        refused: Box::new([]),
        turns: 0,
        spent: 0,
        ending: host::Ending::Failed {
            detail: Box::new([]),
            failure: host::Failure::Run(host::RunFailure::Exceeded(host::Exceeded { bound, value: 2048 })),
            work: host::Work { left: None, saved: None },
        },
    };
    driver.send(protocol::up(
        Token::new(7),
        host::LinkUp::Answer { run: Token::new(run.task), attempt: Token::new(run.attempt), answer },
        engine::Parts::none(),
    ));
    driver.settle();
}

pub(super) fn assert_held(driver: &Driver, run: &engine::Assignment, bound: tasks::Bound) {
    let row = live(driver, run.task);
    assert!(
        matches!(row.phase, tasks::Phase::Held {
        was: tasks::Was::Active(tasks::Active::Due),
        why: tasks::Hold::Exceeded(tasks::Exceeded { bound: held, value: 2048 }), ..
    } if held == bound),
        "{:?}",
        row.phase
    );
    assert_eq!(row.tries, tasks::Tries::NONE);
    assert_eq!(row.retry_notice, None);
    assert_eq!(row.attempt, run.attempt);
    assert_eq!(driver.delivered.iter().filter(|delivery| matches!(delivery, Delivery::Assigned { .. })).count(), 1);
}

#[test]
fn a_run_that_meets_a_bound_is_held_after_one_activation() {
    for (host, task) in [
        (host::Bound::Reasoning, tasks::Bound::Reasoning),
        (host::Bound::Metadata, tasks::Bound::Metadata),
        (host::Bound::Items, tasks::Bound::Items),
        (host::Bound::Answer, tasks::Bound::Answer),
        (host::Bound::Tools, tasks::Bound::Tools),
        (host::Bound::HistoryItems, tasks::Bound::HistoryItems),
        (host::Bound::History, tasks::Bound::History),
        (host::Bound::Window, tasks::Bound::Window),
        (host::Bound::Output, tasks::Bound::Output),
    ] {
        let (mut driver, run) = Driver::chat();
        meets_bound(&mut driver, &run, host);
        assert_held(&driver, &run, task);
        driver.fire_at(Duration::from_secs(60));
        driver.settle();
        assert_held(&driver, &run, task);
    }
}

#[test]
fn a_host_part_start_refusal_is_held_with_its_bound_without_a_try() {
    let (mut driver, run) = Driver::chat();
    let answer = host::Answer {
        read: None,
        refused: Box::new([]),
        turns: 0,
        spent: 0,
        ending: host::Ending::Refused(host::Refusal::Invalid(host::Invalid::Start {
            cause: host::StartCause::HostPart { bound: 33419 },
            why: b"invalid".as_slice().into(),
        })),
    };
    driver.send(protocol::up(
        Token::new(7),
        host::LinkUp::Answer { run: Token::new(run.task), attempt: Token::new(run.attempt), answer },
        engine::Parts::none(),
    ));
    driver.settle();
    let row = live(&driver, run.task);
    assert!(
        matches!(
            row.phase,
            tasks::Phase::Held {
                why: tasks::Hold::Exceeded(tasks::Exceeded { bound: tasks::Bound::HostPart, value: 33419 }),
                ..
            }
        ),
        "{:?}",
        row.phase
    );
    assert_eq!(row.tries, tasks::Tries::NONE);
    assert_eq!(row.retry_notice, None);
}
