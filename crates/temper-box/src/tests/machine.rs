use super::fixtures::{FORGEJO, ONCE_1, ONCE_2, PREPARE, RUNNER, Rig, SERVE, effects_empty, pop_effect, take_effects};
use crate::{
    limits,
    machine::{Input, Order, Outcome},
};
use skein_io::kernel::Exit;
use skein_lib::Duration;
use temper_forge_setup::Kind;
#[test]
fn a_once_step_must_exit_zero_before_the_next_starts() {
    let mut rig = Rig::new(&limits::small());
    assert_eq!(&*rig.effects(), &[Order::Spawn { step: PREPARE }]);
    rig.started(PREPARE);
    assert_eq!(&*rig.effects(), &[]);
    rig.exited(PREPARE, Exit::Code(0));
    assert_eq!(&*rig.effects(), &[Order::Close { step: PREPARE }]);
    rig.closed(PREPARE);
    assert_eq!(&*rig.effects(), &[Order::Spawn { step: FORGEJO }]);
}
#[test]
fn a_long_step_lets_the_next_start_once_it_has_started() {
    let mut rig = Rig::new(&limits::small());
    rig.completed(PREPARE);
    rig.effects();
    rig.started(FORGEJO);
    assert_eq!(&*rig.effects(), &[Order::Spawn { step: RUNNER }]);
}
#[test]
fn a_long_step_that_exits_runs_its_once_steps_again_before_it() {
    let mut rig = Rig::running();
    rig.exited(FORGEJO, Exit::Code(1));
    assert_eq!(&*rig.effects(), &[Order::Close { step: FORGEJO }]);
    rig.closed(FORGEJO);
    rig.advance(Duration::from_millis(500));
    assert_eq!(&*rig.effects(), &[Order::Spawn { step: PREPARE }]);
    rig.completed(PREPARE);
    assert_eq!(&*rig.effects(), &[Order::Close { step: PREPARE }, Order::Spawn { step: FORGEJO }]);
    rig.started(FORGEJO);
    assert_eq!(&*rig.effects(), &[]);
}
#[test]
fn a_long_step_that_exits_runs_every_once_step_just_before_it_again() {
    let mut rig = Rig::running();
    rig.failed_long(SERVE);
    rig.advance(Duration::from_millis(500));
    assert_eq!(&*rig.effects(), &[Order::Spawn { step: ONCE_1 }]);
    rig.completed(ONCE_1);
    assert_eq!(&*rig.effects(), &[Order::Close { step: ONCE_1 }, Order::Spawn { step: ONCE_2 }]);
    rig.completed(ONCE_2);
    assert_eq!(&*rig.effects(), &[Order::Close { step: ONCE_2 }, Order::Spawn { step: SERVE }]);
}
#[test]
fn a_long_step_after_a_long_step_restarts_alone() {
    let mut rig = Rig::running();
    rig.failed_long(RUNNER);
    rig.advance(Duration::from_millis(500));
    assert_eq!(&*rig.effects(), &[Order::Spawn { step: RUNNER }]);
    rig.started(RUNNER);
    assert_eq!(&*rig.effects(), &[]);
}
#[test]
fn the_backoff_doubles_and_stops_doubling_at_its_maximum() {
    // Eight retries reach the cap; the shipped five stop before it.
    let mut rig = Rig::new(&limits::Limits { restarts: 8, ..limits::small() });
    rig.completed(PREPARE);
    rig.started(FORGEJO);
    rig.effects();
    for millis in [500, 1_000, 2_000, 4_000, 8_000, 16_000, 30_000, 30_000] {
        rig.failed_long(FORGEJO);
        assert_eq!(
            rig.machine.next_deadline(),
            Some(rig.now.saturating_add(Duration::from_millis(millis))),
            "gap {millis}"
        );
        rig.advance(Duration::from_millis(millis));
        rig.effects();
        rig.restart_forgejo();
    }
}
#[test]
fn the_backoff_starts_again_after_a_window_without_an_exit() {
    let mut rig = Rig::running();
    rig.failed_long(FORGEJO);
    rig.advance(Duration::from_millis(500));
    rig.effects();
    rig.restart_forgejo();
    rig.advance(Duration::from_secs(301));
    rig.failed_long(FORGEJO);
    assert_eq!(rig.machine.next_deadline(), Some(rig.now.saturating_add(Duration::from_millis(500))));
}
#[test]
fn more_exits_than_restarts_within_the_window_give_up_naming_the_step() {
    let mut rig = Rig::running();
    for millis in [500, 1_000, 2_000, 4_000, 8_000] {
        rig.failed_long(FORGEJO);
        rig.advance(Duration::from_millis(millis));
        rig.effects();
        rig.restart_forgejo();
    }
    rig.exited(FORGEJO, Exit::Code(1));
    rig.closed(FORGEJO);
    assert_eq!(rig.machine.outcome(), Some(Outcome::GaveUp { step: FORGEJO, restarts: 5 }));
    assert_eq!(&*rig.effects(), &[Order::Close { step: FORGEJO }, Order::Signal { step: SERVE }]);
}
#[test]
fn a_once_step_that_fails_gives_up_and_stops_what_runs() {
    for not_started in [false, true] {
        let mut rig = Rig::running();
        rig.failed_long(SERVE);
        rig.advance(Duration::from_millis(500));
        rig.effects();
        if not_started {
            rig.input(Input::NotStarted { step: ONCE_1 });
        } else {
            rig.started(ONCE_1);
            rig.exited(ONCE_1, Exit::Code(1));
            rig.closed(ONCE_1);
        }
        assert_eq!(
            rig.machine.outcome(),
            Some(Outcome::GaveUp { step: ONCE_1, restarts: 0 }),
            "not started {not_started}"
        );
        let expected: &[Order] = if not_started {
            &[Order::Signal { step: RUNNER }]
        } else {
            &[Order::Close { step: ONCE_1 }, Order::Signal { step: RUNNER }]
        };
        assert_eq!(&*rig.effects(), expected, "not started {not_started}");
    }
}
#[test]
fn a_stop_signals_the_long_steps_last_first_one_at_a_time() {
    let mut rig = Rig::running();
    rig.terminate();
    for (step, next) in [(SERVE, Some(RUNNER)), (RUNNER, Some(FORGEJO)), (FORGEJO, None)] {
        assert_eq!(&*rig.effects(), &[Order::Signal { step }]);
        rig.exited(step, Exit::Code(0));
        assert_eq!(&*rig.effects(), &[Order::Close { step }]);
        rig.closed(step);
        if next.is_none() {
            assert_eq!(&*rig.effects(), &[]);
        }
    }
    assert!(rig.machine.is_idle(), "all children closed");
    assert_eq!(rig.machine.outcome(), Some(Outcome::Stopped));
}
#[test]
fn every_long_step_is_signalled_twice_with_a_grace_after_each() {
    let mut rig = Rig::running();
    rig.terminate();
    for step in [SERVE, RUNNER, FORGEJO] {
        assert_eq!(&*rig.effects(), &[Order::Signal { step }]);
        rig.advance(Duration::from_secs(10));
        assert_eq!(&*rig.effects(), &[Order::Signal { step }]);
        rig.advance(Duration::from_secs(10));
        assert_eq!(&*rig.effects(), &[Order::Close { step }]);
        rig.closed(step);
    }
    assert!(rig.machine.is_idle(), "all children closed");
}
#[test]
fn a_stop_during_a_backoff_starts_nothing() {
    let mut rig = Rig::running();
    rig.failed_long(FORGEJO);
    rig.terminate();
    assert_eq!(&*rig.effects(), &[Order::Signal { step: SERVE }]);
    rig.advance(Duration::from_millis(500));
    assert_eq!(&*rig.effects(), &[]);
}
#[test]
fn a_step_still_starting_is_stopped_once_started() {
    let mut rig = Rig::new(&limits::small());
    rig.completed(PREPARE);
    rig.effects();
    rig.terminate();
    assert_eq!(&*rig.effects(), &[Order::Withdraw { step: FORGEJO }]);
    rig.started(FORGEJO);
    assert_eq!(&*rig.effects(), &[Order::Signal { step: FORGEJO }]);
}
#[test]
fn a_second_termination_signal_closes_every_child_at_once() {
    let mut rig = Rig::running();
    rig.terminate();
    rig.effects();
    rig.terminate();
    assert_eq!(
        &*rig.effects(),
        &[Order::Close { step: FORGEJO }, Order::Close { step: RUNNER }, Order::Close { step: SERVE }]
    );
    for step in [FORGEJO, RUNNER, SERVE] {
        rig.closed(step);
    }
    assert!(rig.machine.is_idle(), "forced stop leaves no child");
}

#[test]
fn a_long_step_refused_before_start_retries_and_continues_the_order() {
    let mut rig = Rig::new(&limits::small());
    rig.completed(PREPARE);
    rig.effects();
    rig.input(Input::NotStarted { step: FORGEJO });
    assert_eq!(&*rig.effects(), &[]);
    rig.advance(Duration::from_millis(500));
    assert_eq!(&*rig.effects(), &[Order::Spawn { step: PREPARE }]);
    rig.completed(PREPARE);
    assert_eq!(&*rig.effects(), &[Order::Close { step: PREPARE }, Order::Spawn { step: FORGEJO }]);
    rig.started(FORGEJO);
    assert_eq!(&*rig.effects(), &[Order::Spawn { step: RUNNER }]);
}

#[test]
#[expect(clippy::too_many_lines, reason = "protocol/setup.md 13.1 group behaviours share one transition table")]
fn asked_groups_follow_their_chains_records_failures_and_stop() {
    use super::fixtures::step;
    use crate::layout::{self, Asked, Placed};
    use crate::machine::{Input, Machine, Order, Said, Why};
    use skein_lib::{Queue, Time};
    fn append(prefix: &[(Action, Box<[Order]>)], rows: &[(Action, &[Order])]) -> Box<[(Action, Box<[Order]>)]> {
        let mut script = skein_lib::List::with_capacity(u32::try_from(prefix.len() + rows.len()).unwrap());
        for row in prefix {
            script.push(row.clone()).unwrap();
        }
        for (action, expected) in rows {
            script.push((*action, Box::from(*expected))).unwrap();
        }
        script.into_boxed()
    }
    use temper_forge_setup::{Group, Stage, Steps};
    #[derive(Clone, Copy, Debug)]
    enum Action {
        Input(Input),
        Fire(u64),
    }
    use Action::{Fire as F, Input as I};
    use Input::{Ask, Closed, Exited, NotStarted, NotWritten, Started, Terminate, Written};
    use Order::{Close, Record, Say, Signal, Spawn};
    fn placed() -> Box<[Placed]> {
        layout::flatten(Steps {
            always: Box::new([step(b"anchor", Kind::Long), step(b"serve", Kind::Long)]),
            asked: Box::new([
                Group {
                    label: skein_lib::bytes::copy_of(b"Forge 3 (Forgejo)"),
                    recorded: true,
                    forge: 3,
                    before: 1,
                    steps: Box::new([
                        (Stage::Fetch, step(b"fetch 3", Kind::Once)),
                        (Stage::Prepare, step(b"prepare 3", Kind::Once)),
                        (Stage::Run, step(b"forgejo 3", Kind::Long)),
                        (Stage::Run, step(b"runner 3", Kind::Long)),
                    ]),
                },
                Group {
                    label: skein_lib::bytes::copy_of(b"other label"),
                    recorded: true,
                    forge: 4,
                    before: 2,
                    steps: Box::new([
                        (Stage::Prepare, step(b"prepare 4", Kind::Once)),
                        (Stage::Run, step(b"forgejo 4", Kind::Long)),
                    ]),
                },
            ]),
        })
    }
    fn progress(step: usize) -> Order {
        Say { said: Said::Step { step } }
    }
    fn failed(forge: u16, why: Why) -> Order {
        Say { said: Said::Failed { forge, why } }
    }
    let begin = append(&[], &[(I(Ask { forge: 3 }), &[Spawn { step: 1 }, progress(1)]), (I(Started { step: 1 }), &[])]);
    let mut prepared = begin.clone();
    prepared = append(
        &prepared,
        &[
            (I(Exited { step: 1, exit: Exit::Code(0) }), &[Close { step: 1 }]),
            (I(Closed { step: 1 }), &[Spawn { step: 2 }, progress(2)]),
            (I(Started { step: 2 }), &[]),
            (I(Exited { step: 2, exit: Exit::Code(0) }), &[Close { step: 2 }]),
            (I(Closed { step: 2 }), &[Spawn { step: 3 }, progress(3)]),
        ],
    );
    let mut running = prepared.clone();
    running = append(
        &running,
        &[
            (I(Started { step: 3 }), &[Spawn { step: 4 }, progress(4)]),
            (I(Started { step: 4 }), &[Record { forge: 3 }]),
            (I(Written { forge: 3 }), &[Say { said: Said::Started { forge: 3 } }]),
        ],
    );
    let mut cases = skein_lib::List::with_capacity(16);
    cases
        .push((
            "dormant, unknown and restart across it",
            append(
                &[],
                &[
                    (I(Ask { forge: 99 }), &[failed(99, Why::NotAsked)]),
                    (I(Exited { step: 5, exit: Exit::Code(1) }), &[Close { step: 5 }]),
                    (I(Closed { step: 5 }), &[]),
                    (F(500), &[Spawn { step: 5 }]),
                ],
            ),
        ))
        .unwrap();
    let mut busy = begin.clone();
    busy = append(&busy, &[(I(Ask { forge: 3 }), &[]), (I(Ask { forge: 4 }), &[failed(4, Why::Busy)])]);
    cases.push(("busy and same starting forge", busy)).unwrap();
    let mut recorded = prepared.clone();
    recorded = append(
        &recorded,
        &[
            (I(Ask { forge: 3 }), &[]),
            (I(Ask { forge: 4 }), &[failed(4, Why::Busy)]),
            (I(Started { step: 3 }), &[Spawn { step: 4 }, progress(4)]),
            (I(Started { step: 4 }), &[Record { forge: 3 }]),
            (I(NotWritten { forge: 3 }), &[Close { step: 3 }, Close { step: 4 }]),
            (I(Closed { step: 3 }), &[]),
            (I(Closed { step: 4 }), &[failed(3, Why::NotWritten)]),
            (I(Ask { forge: 3 }), &[Spawn { step: 1 }, progress(1)]),
        ],
    );
    cases.push(("record failure and reask", recorded)).unwrap();
    let mut once_failed = begin.clone();
    once_failed = append(
        &once_failed,
        &[
            (I(Exited { step: 1, exit: Exit::Code(7) }), &[Close { step: 1 }]),
            (I(Ask { forge: 4 }), &[failed(4, Why::Busy)]),
            (I(Ask { forge: 3 }), &[]),
            (I(Closed { step: 1 }), &[failed(3, Why::Exited { step: 1, exit: Exit::Code(7) })]),
            (I(Ask { forge: 3 }), &[Spawn { step: 1 }, progress(1)]),
        ],
    );
    cases.push(("once failure closes then dormant", once_failed)).unwrap();
    cases
        .push((
            "spawn refused",
            append(
                &[],
                &[
                    (I(Ask { forge: 3 }), &[Spawn { step: 1 }, progress(1)]),
                    (I(NotStarted { step: 1 }), &[failed(3, Why::NotStarted { step: 1 })]),
                ],
            ),
        ))
        .unwrap();
    let mut long_failed = prepared.clone();
    long_failed = append(
        &long_failed,
        &[
            (I(Started { step: 3 }), &[Spawn { step: 4 }, progress(4)]),
            (I(Exited { step: 3, exit: Exit::Signal(9) }), &[Close { step: 3 }, Order::Withdraw { step: 4 }]),
            (I(Closed { step: 3 }), &[]),
            (I(Started { step: 4 }), &[Close { step: 4 }]),
            (I(Closed { step: 4 }), &[failed(3, Why::Exited { step: 3, exit: Exit::Signal(9) })]),
            (F(1000), &[]),
        ],
    );
    cases.push(("long failure closes spawn in flight", long_failed)).unwrap();
    let mut restart = running.clone();
    restart = append(
        &restart,
        &[
            (I(Ask { forge: 3 }), &[Say { said: Said::Started { forge: 3 } }]),
            (I(Exited { step: 3, exit: Exit::Code(0) }), &[Close { step: 3 }]),
            (I(Closed { step: 3 }), &[]),
            (F(500), &[Spawn { step: 1 }]),
        ],
    );
    cases.push(("started answers and restarts its own preparation", restart)).unwrap();
    let mut stop = begin.clone();
    stop = append(
        &stop,
        &[
            (I(Terminate), &[Signal { step: 5 }]),
            (I(Ask { forge: 3 }), &[failed(3, Why::Stopping)]),
            (I(Exited { step: 1, exit: Exit::Code(1) }), &[Close { step: 1 }]),
            (I(Closed { step: 1 }), &[]),
        ],
    );
    cases.push(("stop suppresses group replies", stop)).unwrap();
    let mut give_up = running.clone();
    give_up = append(
        &give_up,
        &[(I(Exited { step: 5, exit: Exit::Code(1) }), &[Close { step: 5 }]), (I(Closed { step: 5 }), &[])],
    );
    cases.push(("started long is subject to restart policy", give_up)).unwrap();
    for (name, script) in cases.into_boxed() {
        let mut machine = Machine::new(&limits::small(), &placed()).unwrap();
        let mut out = Queue::with_capacity(128);
        machine.start(Time::ZERO, &[], &mut out);
        assert_eq!(pop_effect(&mut out), Some(Spawn { step: 0 }), "{name}");
        assert!(effects_empty(&out));
        machine.input(Time::ZERO, Started { step: 0 }, &mut out);
        assert_eq!(pop_effect(&mut out), Some(Spawn { step: 5 }), "{name}: dormant skipped");
        machine.input(Time::ZERO, Started { step: 5 }, &mut out);
        for (action, expected) in script {
            match action {
                I(input) => machine.input(Time::ZERO, input, &mut out),
                F(ms) => machine.fire(Time::ZERO.saturating_add(Duration::from_millis(ms)), &mut out),
            }
            let got = take_effects(&mut out);
            assert_eq!(got.as_ref(), expected.as_ref(), "{name}");
        }
    }
    // The record barrier also works when there is no preparation, or no long step.
    for long in [false, true] {
        let placed = layout::flatten(Steps {
            always: Box::new([]),
            asked: Box::new([Group {
                label: skein_lib::bytes::copy_of(b"Forge 3 (Forgejo)"),
                recorded: true,
                forge: 9,
                before: 0,
                steps: Box::new([(
                    if long { Stage::Run } else { Stage::Fetch },
                    step(b"only", if long { Kind::Long } else { Kind::Once }),
                )]),
            }]),
        });
        let mut machine = Machine::new(&limits::small(), &placed).unwrap();
        let mut out = Queue::with_capacity(32);
        machine.start(Time::ZERO, &[], &mut out);
        assert!(effects_empty(&out));
        machine.input(Time::ZERO, Ask { forge: 9 }, &mut out);
        assert_eq!(pop_effect(&mut out), Some(Spawn { step: 0 }));
        assert_eq!(pop_effect(&mut out), Some(progress(0)));
        machine.input(Time::ZERO, Started { step: 0 }, &mut out);
        if !long {
            machine.input(Time::ZERO, Exited { step: 0, exit: Exit::Code(0) }, &mut out);
            assert_eq!(pop_effect(&mut out), Some(Close { step: 0 }));
            machine.input(Time::ZERO, Closed { step: 0 }, &mut out);
        }
        assert_eq!(pop_effect(&mut out), Some(Record { forge: 9 }));
        assert!(effects_empty(&out));
        machine.input(Time::ZERO, Written { forge: 9 }, &mut out);
        assert_eq!(pop_effect(&mut out), Some(Say { said: Said::Started { forge: 9 } }));
        assert!(effects_empty(&out));
    }
    // A record joins the startup chain, with no ask or record barrier.
    let mut machine = Machine::new(&limits::small(), &placed()).unwrap();
    let mut out = Queue::with_capacity(128);
    machine.start(Time::ZERO, &[3], &mut out);
    pop_effect(&mut out);
    machine.input(Time::ZERO, Started { step: 0 }, &mut out);
    assert_eq!(pop_effect(&mut out), Some(Spawn { step: 1 }));
    for (step, next) in [(1, 2), (2, 3)] {
        machine.input(Time::ZERO, Started { step }, &mut out);
        machine.input(Time::ZERO, Exited { step, exit: Exit::Code(0) }, &mut out);
        pop_effect(&mut out);
        machine.input(Time::ZERO, Closed { step }, &mut out);
        assert_eq!(pop_effect(&mut out), Some(Spawn { step: next }));
    }
    machine.input(Time::ZERO, Started { step: 3 }, &mut out);
    assert_eq!(pop_effect(&mut out), Some(Spawn { step: 4 }));
    machine.input(Time::ZERO, Started { step: 4 }, &mut out);
    assert_eq!(pop_effect(&mut out), Some(Spawn { step: 5 }));
    assert!(effects_empty(&out));
    // Flatten includes end placements, and preserves the stage on every item.
    let placed = placed();
    assert_eq!(placed.len(), 8);
    assert_eq!(placed[6].asked, Some(Asked { recorded: true, forge: 4, stage: Stage::Prepare }));
    assert_eq!(layout::label(&placed, 3), Some(b"Forge 3 (Forgejo)".as_slice()));
    assert_eq!(layout::label(&placed, 4), Some(b"other label".as_slice()));
    assert_eq!(layout::label(&placed, 99), None);
    for (index, item) in placed.iter().enumerate() {
        assert_eq!(item.label.is_some(), [1, 6].contains(&index), "each label moves once to its first placed step");
    }
}

#[test]
#[expect(clippy::too_many_lines, reason = "protocol/setup.md 13.1 link behaviours share one transition table")]
fn a_link_takes_the_first_line_once_across_restarts_and_stop() {
    use super::fixtures::step;
    use crate::{
        layout,
        machine::{Machine, Said},
    };
    use skein_lib::{Queue, Time};
    use temper_forge_setup::{Group, Stage, Steps};
    #[derive(Clone, Copy, Debug)]
    enum Action {
        Input(Input),
        Fire(u64),
    }
    use Action::{Fire as F, Input as I};
    use Input::{Ask, Closed, Exited, Line, Started, Terminate};
    use Order::{Close, Hold, Say, Signal, Spawn};
    const PREPARE: usize = 0;
    const FORGE: usize = 1;
    const SERVE: usize = 2;
    const SEED: usize = 3;
    type Rows = Box<[(Action, Box<[Order]>)]>;
    fn append(prefix: &Rows, rows: &[(Action, Box<[Order]>)]) -> Rows {
        let mut result = skein_lib::List::with_capacity(u32::try_from(prefix.len() + rows.len()).unwrap());
        for row in prefix {
            result.push(row.clone()).unwrap();
        }
        for row in rows {
            result.push(row.clone()).unwrap();
        }
        result.into_boxed()
    }
    let startup: Rows = Box::new([
        (I(Started { step: PREPARE }), Box::new([])),
        (I(Exited { step: PREPARE, exit: Exit::Code(0) }), Box::new([Close { step: PREPARE }])),
        (I(Closed { step: PREPARE }), Box::new([Spawn { step: FORGE }])),
        (I(Started { step: FORGE }), Box::new([Hold { step: SERVE }, Spawn { step: SERVE }])),
        (I(Started { step: SERVE }), Box::new([])),
    ]);
    let linked = append(
        &startup,
        &[
            (I(Line { step: SERVE }), Box::new([Spawn { step: SEED }])),
            (I(Started { step: SEED }), Box::new([])),
            (I(Line { step: SERVE }), Box::new([])),
        ],
    );
    let mut stopping: Rows = Box::new([(I(Terminate), Box::new([Signal { step: SEED }]))]);
    for (index, step) in [SEED, SERVE, FORGE].into_iter().enumerate() {
        let first = u64::try_from(index).unwrap() * 20_000;
        stopping = append(
            &stopping,
            &[
                (F(first + 10_000), Box::new([Signal { step }])),
                (F(first + 20_000), Box::new([Close { step }])),
                (I(Closed { step }), if step == FORGE { Box::new([]) } else { Box::new([Signal { step: step - 1 }]) }),
            ],
        );
    }
    let cases = [
        (
            "success then ask",
            append(
                &linked,
                &[
                    (I(Exited { step: SEED, exit: Exit::Code(0) }), Box::new([Close { step: SEED }])),
                    (I(Closed { step: SEED }), Box::new([])),
                    (I(Ask { forge: 3 }), Box::new([Spawn { step: 4 }, Say { said: Said::Step { step: 4 } }])),
                ],
            ),
            None,
        ),
        (
            "seed failure",
            append(
                &linked,
                &[
                    (I(Exited { step: SEED, exit: Exit::Code(1) }), Box::new([Close { step: SEED }])),
                    (I(Closed { step: SEED }), Box::new([Signal { step: SERVE }])),
                ],
            ),
            Some(Outcome::GaveUp { step: SEED, restarts: 0 }),
        ),
        (
            "exit before line",
            append(
                &startup,
                &[
                    (I(Exited { step: SERVE, exit: Exit::Code(1) }), Box::new([Close { step: SERVE }])),
                    (I(Closed { step: SERVE }), Box::new([])),
                    (F(500), Box::new([Hold { step: SERVE }, Spawn { step: SERVE }])),
                    (I(Started { step: SERVE }), Box::new([])),
                    (I(Line { step: SERVE }), Box::new([Spawn { step: SEED }])),
                ],
            ),
            None,
        ),
        (
            "exit after line",
            append(
                &linked,
                &[
                    (I(Exited { step: SERVE, exit: Exit::Code(1) }), Box::new([Close { step: SERVE }])),
                    (I(Closed { step: SERVE }), Box::new([])),
                    (F(500), Box::new([Spawn { step: SERVE }])),
                    (I(Started { step: SERVE }), Box::new([])),
                    (I(Line { step: SERVE }), Box::new([])),
                ],
            ),
            None,
        ),
        (
            "line after exit",
            append(
                &startup,
                &[
                    (I(Exited { step: SERVE, exit: Exit::Code(1) }), Box::new([Close { step: SERVE }])),
                    (I(Line { step: SERVE }), Box::new([Spawn { step: SEED }])),
                    (I(Closed { step: SERVE }), Box::new([])),
                    (F(500), Box::new([Spawn { step: SERVE }])),
                    (I(Started { step: SERVE }), Box::new([])),
                    (I(Line { step: SERVE }), Box::new([])),
                ],
            ),
            None,
        ),
        (
            "line after closed",
            append(
                &startup,
                &[
                    (I(Exited { step: SERVE, exit: Exit::Code(1) }), Box::new([Close { step: SERVE }])),
                    (I(Closed { step: SERVE }), Box::new([])),
                    (I(Line { step: SERVE }), Box::new([Spawn { step: SEED }])),
                    (F(500), Box::new([Spawn { step: SERVE }])),
                ],
            ),
            None,
        ),
        (
            "forge restart while seed runs",
            append(
                &linked,
                &[
                    (I(Exited { step: FORGE, exit: Exit::Code(1) }), Box::new([Close { step: FORGE }])),
                    (I(Closed { step: FORGE }), Box::new([])),
                    (F(500), Box::new([Spawn { step: PREPARE }])),
                    (I(Started { step: PREPARE }), Box::new([])),
                    (I(Exited { step: PREPARE, exit: Exit::Code(0) }), Box::new([Close { step: PREPARE }])),
                    (I(Closed { step: PREPARE }), Box::new([Spawn { step: FORGE }])),
                    (I(Started { step: FORGE }), Box::new([])),
                ],
            ),
            None,
        ),
        ("stop running seed", append(&linked, &stopping), Some(Outcome::Stopped)),
        (
            "stop before line",
            append(
                &startup,
                &[(I(Terminate), Box::new([Signal { step: SERVE }])), (I(Line { step: SERVE }), Box::new([]))],
            ),
            Some(Outcome::Stopped),
        ),
    ];
    for (name, script, outcome) in cases {
        let steps = layout::flatten(Steps {
            always: Box::new([
                step(b"prepare", Kind::Once),
                step(b"forge", Kind::Long),
                step(b"serve", Kind::Long),
                step(b"seed", Kind::Link),
            ]),
            asked: Box::new([Group {
                label: skein_lib::bytes::copy_of(b"Forge 3 (Forgejo)"),
                recorded: true,
                forge: 3,
                before: 4,
                steps: Box::new([
                    (Stage::Prepare, step(b"prepare 3", Kind::Once)),
                    (Stage::Run, step(b"forge 3", Kind::Long)),
                ]),
            }]),
        });
        let mut machine = Machine::new(&limits::small(), &steps).unwrap();
        let mut out = Queue::with_capacity(machine.max_out());
        let mut now = Time::ZERO;
        machine.start(now, &[], &mut out);
        assert_eq!(pop_effect(&mut out), Some(Spawn { step: PREPARE }), "{name}");
        assert!(effects_empty(&out));
        for (action, expected) in script {
            match action {
                I(input) => machine.input(now, input, &mut out),
                F(ms) => {
                    now = Time::ZERO.saturating_add(Duration::from_millis(ms));
                    machine.fire(now, &mut out);
                }
            }
            let got = take_effects(&mut out);
            assert_eq!(got.as_ref(), expected.as_ref(), "{name}: {action:?}");
        }
        assert_eq!(machine.outcome(), outcome, "{name}");
    }
}

fn check_link_placement(steps: &[crate::layout::Placed]) {
    crate::machine::Machine::new(&limits::small(), steps).unwrap();
}
#[test]
#[should_panic(expected = "a link is not first")]
fn a_link_cannot_be_first() {
    check_link_placement(&[crate::layout::Placed {
        label: None,
        step: super::fixtures::step(b"seed", Kind::Link),
        asked: None,
    }]);
}
#[test]
#[should_panic(expected = "a link is outside groups")]
fn a_link_cannot_belong_to_a_group() {
    use crate::layout::{Asked, Placed};
    check_link_placement(&[
        Placed { label: None, step: super::fixtures::step(b"serve", Kind::Long), asked: None },
        Placed {
            label: None,
            step: super::fixtures::step(b"seed", Kind::Link),
            asked: Some(Asked { recorded: true, forge: 3, stage: temper_forge_setup::Stage::Run }),
        },
    ]);
}
#[test]
#[should_panic(expected = "a link follows a long step")]
fn a_link_cannot_follow_a_once_step() {
    use crate::layout::Placed;
    check_link_placement(&[
        Placed { label: None, step: super::fixtures::step(b"prepare", Kind::Once), asked: None },
        Placed { label: None, step: super::fixtures::step(b"seed", Kind::Link), asked: None },
    ]);
}
#[test]
#[should_panic(expected = "a link's long step is outside groups")]
fn a_link_cannot_follow_a_group_step() {
    use crate::layout::{Asked, Placed};
    check_link_placement(&[
        Placed {
            label: None,
            step: super::fixtures::step(b"serve", Kind::Long),
            asked: Some(Asked { recorded: true, forge: 3, stage: temper_forge_setup::Stage::Run }),
        },
        Placed { label: None, step: super::fixtures::step(b"seed", Kind::Link), asked: None },
    ]);
}

#[test]
fn an_asked_groups_final_once_step_gates_started_and_failure_allows_another_ask() {
    use super::fixtures::step;
    use crate::{
        layout,
        machine::{Machine, Said, Why},
    };
    use skein_lib::{Queue, Time};
    use temper_forge_setup::{Group, Stage, Steps};
    let placed = layout::flatten(Steps {
        always: Box::new([]),
        asked: Box::new([Group {
            label: skein_lib::bytes::copy_of(b"Forge 3 (Forgejo)"),
            recorded: true,
            forge: 3,
            before: 0,
            steps: Box::new([
                (Stage::Run, step(b"forgejo 3", Kind::Long)),
                (Stage::Run, step(b"runner 3", Kind::Long)),
                (Stage::Run, step(b"once 3", Kind::Once)),
            ]),
        }]),
    });
    for code in [0, 1] {
        let mut machine = Machine::new(&limits::small(), &placed).unwrap();
        let mut out = Queue::with_capacity(32);
        machine.start(Time::ZERO, &[], &mut out);
        machine.input(Time::ZERO, Input::Ask { forge: 3 }, &mut out);
        for step in 0..3 {
            assert_eq!(pop_effect(&mut out), Some(Order::Spawn { step }));
            assert_eq!(pop_effect(&mut out), Some(Order::Say { said: Said::Step { step } }));
            machine.input(Time::ZERO, Input::Started { step }, &mut out);
        }
        assert!(effects_empty(&out), "the once step must exit before started");
        machine.input(Time::ZERO, Input::Exited { step: 2, exit: Exit::Code(code) }, &mut out);
        if code == 0 {
            assert_eq!(pop_effect(&mut out), Some(Order::Close { step: 2 }));
            machine.input(Time::ZERO, Input::Closed { step: 2 }, &mut out);
            assert_eq!(pop_effect(&mut out), Some(Order::Record { forge: 3 }));
            machine.input(Time::ZERO, Input::Written { forge: 3 }, &mut out);
            assert_eq!(pop_effect(&mut out), Some(Order::Say { said: Said::Started { forge: 3 } }));
        } else {
            for step in 0..3 {
                assert_eq!(pop_effect(&mut out), Some(Order::Close { step }));
                machine.input(Time::ZERO, Input::Closed { step }, &mut out);
            }
            assert_eq!(
                pop_effect(&mut out),
                Some(Order::Say { said: Said::Failed { forge: 3, why: Why::Exited { step: 2, exit: Exit::Code(1) } } })
            );
            machine.input(Time::ZERO, Input::Ask { forge: 3 }, &mut out);
            assert_eq!(pop_effect(&mut out), Some(Order::Spawn { step: 0 }));
            assert_eq!(pop_effect(&mut out), Some(Order::Say { said: Said::Step { step: 0 } }));
        }
        assert!(effects_empty(&out));
    }
}

#[test]
fn an_owned_version_gates_the_runner_and_record_on_ask_and_restart() {
    use super::fixtures::step;
    use crate::layout;
    use crate::machine::{Machine, Said, Why};
    use skein_lib::{Queue, Time};
    use temper_forge_setup::{Group, Stage, Steps};
    let placed = layout::flatten(Steps {
        always: Box::new([step(b"serve", Kind::Long)]),
        asked: Box::new([Group {
            label: skein_lib::bytes::copy_of(b"Forge 3 (Forgejo)"),
            recorded: true,
            forge: 3,
            before: 0,
            steps: Box::new([
                (Stage::Run, step(b"forgejo 3", Kind::Long).with_ready(([127, 0, 0, 1], 3000).into())),
                (Stage::Run, step(b"runner 3", Kind::Long)),
                (Stage::Run, step(b"once 3", Kind::Once)),
            ]),
        }]),
    });
    for recorded in [false, true] {
        for ending in ["ready", "timeout", "stop", "exit"] {
            let mut machine = Machine::new(&limits::small(), &placed).unwrap();
            let mut out = Queue::with_capacity(32);
            let now = Time::ZERO;
            machine.start(now, if recorded { &[3] } else { &[] }, &mut out);
            if !recorded {
                assert_eq!(pop_effect(&mut out), Some(Order::Spawn { step: 3 }));
                machine.input(now, Input::Started { step: 3 }, &mut out);
                machine.input(now, Input::Ask { forge: 3 }, &mut out);
            }
            assert_eq!(pop_effect(&mut out), Some(Order::Spawn { step: 0 }));
            if !recorded {
                assert_eq!(pop_effect(&mut out), Some(Order::Say { said: Said::Step { step: 0 } }));
            }
            machine.input(now, Input::Started { step: 0 }, &mut out);
            assert_eq!(pop_effect(&mut out), Some(Order::Wait { step: 0 }));
            assert!(effects_empty(&out), "{recorded}/{ending}: no runner or record before version");
            match ending {
                "ready" => {
                    machine.input(now, Input::Ready { step: 0 }, &mut out);
                    assert_eq!(pop_effect(&mut out), Some(Order::Spawn { step: 1 }));
                    if !recorded {
                        assert_eq!(pop_effect(&mut out), Some(Order::Say { said: Said::Step { step: 1 } }));
                    }
                    machine.input(now, Input::Ready { step: 0 }, &mut out);
                    assert!(effects_empty(&out), "a stale ready cannot skip the once step");
                    machine.input(now, Input::Started { step: 1 }, &mut out);
                    assert_eq!(pop_effect(&mut out), Some(Order::Spawn { step: 2 }));
                    if !recorded {
                        assert_eq!(pop_effect(&mut out), Some(Order::Say { said: Said::Step { step: 2 } }));
                    }
                    machine.input(now, Input::Started { step: 2 }, &mut out);
                    machine.input(now, Input::Exited { step: 2, exit: Exit::Code(0) }, &mut out);
                    assert_eq!(pop_effect(&mut out), Some(Order::Close { step: 2 }));
                    machine.input(now, Input::Closed { step: 2 }, &mut out);
                    if recorded {
                        assert_eq!(pop_effect(&mut out), Some(Order::Spawn { step: 3 }));
                    } else {
                        assert_eq!(pop_effect(&mut out), Some(Order::Record { forge: 3 }));
                        machine.input(now, Input::Written { forge: 3 }, &mut out);
                        assert_eq!(pop_effect(&mut out), Some(Order::Say { said: Said::Started { forge: 3 } }));
                    }
                }
                "timeout" => {
                    machine.input(now, Input::NotReady { step: 0 }, &mut out);
                    if recorded {
                        assert_eq!(machine.outcome(), Some(Outcome::GaveUp { step: 0, restarts: 0 }));
                    } else {
                        assert_eq!(pop_effect(&mut out), Some(Order::Close { step: 0 }));
                        machine.input(now, Input::Closed { step: 0 }, &mut out);
                        assert_eq!(
                            pop_effect(&mut out),
                            Some(Order::Say { said: Said::Failed { forge: 3, why: Why::NotReady { step: 0 } } })
                        );
                    }
                }
                "stop" => {
                    machine.input(now, Input::Terminate, &mut out);
                    machine.input(now, Input::Ready { step: 0 }, &mut out);
                    assert_eq!(pop_effect(&mut out), Some(Order::Signal { step: if recorded { 0 } else { 3 } }));
                    assert!(effects_empty(&out), "ready during stop launches nothing");
                }
                "exit" => {
                    machine.input(now, Input::Exited { step: 0, exit: Exit::Code(1) }, &mut out);
                    assert_eq!(pop_effect(&mut out), Some(Order::Close { step: 0 }));
                    machine.input(now, Input::Closed { step: 0 }, &mut out);
                    if !recorded {
                        assert_eq!(
                            pop_effect(&mut out),
                            Some(Order::Say {
                                said: Said::Failed { forge: 3, why: Why::Exited { step: 0, exit: Exit::Code(1) } }
                            })
                        );
                    }
                    assert!(effects_empty(&out), "exit while waiting starts nothing");
                }
                _ => unreachable!(),
            }
        }
    }
}

#[test]
fn an_unrecorded_group_runs_only_when_asked_even_with_a_stale_record() {
    use super::fixtures::step;
    use crate::{
        layout,
        machine::{Machine, Said},
    };
    use skein_lib::{Queue, Time};
    use temper_forge_setup::{Group, Stage, Steps};
    let placed = layout::flatten(Steps {
        always: Box::new([step(b"serve", Kind::Long)]),
        asked: Box::new([Group {
            label: skein_lib::bytes::copy_of(b"Forge 3 (Forgejo)"),
            forge: 1,
            before: 0,
            recorded: false,
            steps: Box::new([
                (Stage::Run, step(b"fake llm", Kind::Long)),
                (Stage::Prepare, step(b"prepare 1", Kind::Once)),
                (Stage::Run, step(b"fake forge 1", Kind::Long)),
            ]),
        }]),
    });
    for records in [&[][..], &[1][..]] {
        let mut machine = Machine::new(&limits::small(), &placed).unwrap();
        let mut out = Queue::with_capacity(machine.max_out());
        machine.start(Time::ZERO, records, &mut out);
        assert_eq!(pop_effect(&mut out), Some(Order::Spawn { step: 3 }));
        machine.input(Time::ZERO, Input::Started { step: 3 }, &mut out);
        assert!(effects_empty(&out), "only serve starts, also on a second box");
        machine.input(Time::ZERO, Input::Ask { forge: 1 }, &mut out);
        for step in 0..3 {
            assert_eq!(pop_effect(&mut out), Some(Order::Spawn { step }));
            assert_eq!(pop_effect(&mut out), Some(Order::Say { said: Said::Step { step } }));
            machine.input(Time::ZERO, Input::Started { step }, &mut out);
            if step == 1 {
                machine.input(Time::ZERO, Input::Exited { step, exit: Exit::Code(0) }, &mut out);
                assert_eq!(pop_effect(&mut out), Some(Order::Close { step }));
                machine.input(Time::ZERO, Input::Closed { step }, &mut out);
            }
        }
        assert_eq!(pop_effect(&mut out), Some(Order::Say { said: Said::Started { forge: 1 } }));
        assert!(effects_empty(&out), "no record order");
        machine.input(Time::ZERO, Input::Ask { forge: 1 }, &mut out);
        assert_eq!(pop_effect(&mut out), Some(Order::Say { said: Said::Started { forge: 1 } }));
        assert!(effects_empty(&out));
    }
}

// A running serve outside the asked group makes an accidental whole-box stop visible.
fn stoppable_group(recorded: bool) -> Rig {
    use crate::layout;
    use temper_forge_setup::{Group, Stage, Steps};
    let limits = limits::small();
    let machine = crate::machine::Machine::new(
        &limits,
        &layout::flatten(Steps {
            always: Box::new([super::fixtures::step(b"serve", Kind::Long)]),
            asked: Box::new([Group {
                label: skein_lib::bytes::copy_of(b"Forge 3 (Forgejo)"),
                forge: 1,
                recorded,
                before: 0,
                steps: Box::new([
                    (Stage::Run, super::fixtures::step(b"fake llm", Kind::Long)),
                    (Stage::Prepare, super::fixtures::step(b"prepare", Kind::Once)),
                    (Stage::Run, super::fixtures::step(b"fake forge", Kind::Long)),
                ]),
            }]),
        }),
    )
    .unwrap();
    let out = skein_lib::Queue::with_capacity(machine.max_out());
    let mut rig = Rig { machine, out, now: skein_lib::Time::ZERO };
    rig.machine.start(rig.now, &[], &mut rig.out);
    assert_eq!(&*rig.effects(), &[Order::Spawn { step: 3 }]);
    rig.started(3);
    rig
}
fn start_stoppable(rig: &mut Rig, recorded: bool) {
    use crate::machine::Said;
    rig.input(Input::Ask { forge: 1 });
    for step in 0..3 {
        assert_eq!(&*rig.effects(), &[Order::Spawn { step }, Order::Say { said: Said::Step { step } }]);
        rig.started(step);
        if step == 1 {
            rig.exited(step, Exit::Code(0));
            assert_eq!(&*rig.effects(), &[Order::Close { step }]);
            rig.closed(step);
        }
    }
    if recorded {
        assert_eq!(&*rig.effects(), &[Order::Record { forge: 1 }]);
        rig.input(Input::Written { forge: 1 });
    }
    assert_eq!(&*rig.effects(), &[Order::Say { said: Said::Started { forge: 1 } }]);
}
#[test]
fn a_group_stop_signals_in_reverse_and_answers_only_after_all_closes_then_can_start_again() {
    use crate::machine::Said;
    for recorded in [false, true] {
        let mut rig = stoppable_group(recorded);
        start_stoppable(&mut rig, recorded);
        rig.input(Input::Stop { forge: 1 });
        for step in [2, 0] {
            assert_eq!(&*rig.effects(), &[Order::Signal { step }]);
            rig.advance(limits::small().grace);
            assert_eq!(&*rig.effects(), &[Order::Signal { step }]);
            rig.advance(limits::small().grace);
            assert_eq!(&*rig.effects(), &[Order::Close { step }]);
            rig.closed(step);
        }
        assert_eq!(&*rig.effects(), &[Order::Say { said: Said::Stopped { forge: 1 } }]);
        assert_eq!(rig.machine.outcome(), None, "serve keeps running");
        rig.advance(Duration::from_secs(100));
        assert!(rig.effects().is_empty(), "nothing restarts");
        start_stoppable(&mut rig, recorded);
    }
}
#[test]
fn group_stop_refusals_cover_dormant_starting_failing_closing_unknown_and_box_stopping() {
    use crate::machine::{Said, Why};
    fn failed(rig: &mut Rig, input: Input, forge: u16, why: Why) {
        rig.input(input);
        assert_eq!(&*rig.effects(), &[Order::Say { said: Said::Failed { forge, why } }]);
    }
    let mut rig = stoppable_group(false);
    rig.input(Input::Stop { forge: 1 });
    assert_eq!(&*rig.effects(), &[Order::Say { said: Said::Stopped { forge: 1 } }]);
    failed(&mut rig, Input::Stop { forge: 99 }, 99, Why::NotAsked);
    rig.input(Input::Ask { forge: 1 });
    rig.effects();
    failed(&mut rig, Input::Stop { forge: 1 }, 1, Why::Busy);
    rig.started(0);
    rig.effects();
    rig.input(Input::NotStarted { step: 1 });
    assert_eq!(&*rig.effects(), &[Order::Close { step: 0 }]);
    failed(&mut rig, Input::Stop { forge: 1 }, 1, Why::Busy);
    rig.input(Input::Ask { forge: 1 });
    assert!(rig.effects().is_empty(), "the failing forge waits for its own failure line");
    rig.closed(0);
    rig.effects();
    start_stoppable(&mut rig, false);
    rig.input(Input::Stop { forge: 1 });
    assert_eq!(&*rig.effects(), &[Order::Signal { step: 2 }]);
    for forge in [1, 99] {
        failed(&mut rig, Input::Ask { forge }, forge, Why::Busy);
        failed(&mut rig, Input::Stop { forge }, forge, Why::Busy);
    }
    rig.terminate();
    rig.effects();
    for forge in [1, 99] {
        failed(&mut rig, Input::Ask { forge }, forge, Why::Stopping);
        failed(&mut rig, Input::Stop { forge }, forge, Why::Stopping);
    }
}
#[test]
fn a_group_stop_disarms_backoff_and_closes_a_live_or_starting_restart_preparation() {
    use crate::machine::Said;
    for preparation in ["backoff", "starting", "live", "not started"] {
        let mut rig = stoppable_group(false);
        start_stoppable(&mut rig, false);
        rig.failed_long(2);
        if preparation != "backoff" {
            rig.advance(limits::small().backoff);
            assert_eq!(&*rig.effects(), &[Order::Spawn { step: 1 }]);
            if preparation == "live" {
                rig.started(1);
            }
        }
        rig.input(Input::Stop { forge: 1 });
        if preparation == "starting" {
            assert_eq!(&*rig.effects(), &[Order::Withdraw { step: 1 }]);
            rig.started(1);
        } else if preparation == "not started" {
            assert_eq!(&*rig.effects(), &[Order::Withdraw { step: 1 }]);
            rig.input(Input::NotStarted { step: 1 });
        }
        if preparation == "starting" || preparation == "live" {
            assert_eq!(&*rig.effects(), &[Order::Signal { step: 1 }]);
            rig.exited(1, Exit::Code(1));
            assert_eq!(&*rig.effects(), &[Order::Close { step: 1 }]);
            rig.closed(1);
        }
        assert_eq!(&*rig.effects(), &[Order::Signal { step: 0 }]);
        rig.exited(0, Exit::Code(1));
        assert_eq!(&*rig.effects(), &[Order::Close { step: 0 }]);
        rig.closed(0);
        assert_eq!(&*rig.effects(), &[Order::Say { said: Said::Stopped { forge: 1 } }]);
        rig.advance(Duration::from_secs(100));
        assert!(rig.effects().is_empty(), "{preparation}: no restart or continuation");
    }
}
#[test]
fn box_termination_during_a_group_stop_closes_everything_without_a_stopped_reply() {
    let mut rig = stoppable_group(false);
    start_stoppable(&mut rig, false);
    rig.input(Input::Stop { forge: 1 });
    assert_eq!(&*rig.effects(), &[Order::Signal { step: 2 }]);
    rig.terminate();
    assert_eq!(&*rig.effects(), &[Order::Signal { step: 3 }]);
    rig.terminate();
    assert_eq!(&*rig.effects(), &[Order::Close { step: 0 }, Order::Close { step: 2 }, Order::Close { step: 3 }]);
    for step in [0, 2, 3] {
        rig.closed(step);
    }
    assert!(rig.effects().is_empty());
    assert!(rig.machine.is_idle());
    assert_eq!(rig.machine.outcome(), Some(Outcome::Stopped));
}

#[test]
fn lifecycle_facts_interleave_with_spawn_and_actual_stop_settlement() {
    use crate::{layout, machine::Machine, terminal::Fact};
    use skein_lib::{Queue, Time};
    use temper_forge_setup::Steps;
    let placed =
        layout::flatten(Steps { always: Box::new([super::fixtures::step(b"serve", Kind::Long)]), asked: Box::new([]) });
    let mut machine = Machine::new(&limits::small(), &placed).unwrap();
    let mut out = Queue::with_capacity(machine.max_out());
    machine.start(Time::ZERO, &[], &mut out);
    assert_eq!(out.pop(), Some(Order::Terminal { fact: Fact::Starting { step: 0 } }));
    assert_eq!(out.pop(), Some(Order::Spawn { step: 0 }));
    machine.input(Time::ZERO, Input::Started { step: 0 }, &mut out);
    machine.input(Time::ZERO, Input::Terminate, &mut out);
    assert_eq!(out.pop(), Some(Order::Terminal { fact: Fact::Stopping }));
    assert_eq!(out.pop(), Some(Order::Signal { step: 0 }));
    assert!(out.is_empty(), "assigning Outcome::Stopped is not settlement");
    machine.input(Time::ZERO, Input::Terminate, &mut out);
    assert_eq!(out.pop(), Some(Order::Close { step: 0 }));
    assert!(out.is_empty(), "a second termination never repeats Stopping");
    machine.input(Time::ZERO, Input::Closed { step: 0 }, &mut out);
    assert_eq!(out.pop(), Some(Order::Terminal { fact: Fact::Stopped }));
    machine.input(Time::ZERO, Input::Closed { step: 0 }, &mut out);
    machine.input(Time::ZERO, Input::Terminate, &mut out);
    assert!(out.is_empty(), "Stopped is emitted once");
}
#[test]
fn failure_and_a_requested_stop_keep_independent_outcomes() {
    use super::fixtures::take_facts;
    use crate::terminal::Fact;
    let mut rig = Rig::new(&limits::small());
    rig.effects();
    rig.started(PREPARE);
    rig.exited(PREPARE, Exit::Code(1));
    rig.closed(PREPARE);
    assert_eq!(
        take_facts(&mut rig.out).as_ref(),
        &[Fact::GivingUp { step: PREPARE, why: crate::machine::Why::Exited { step: PREPARE, exit: Exit::Code(1) } }]
    );
    rig.terminate();
    assert_eq!(take_facts(&mut rig.out).as_ref(), &[Fact::Stopping, Fact::Stopped]);
    assert_eq!(rig.machine.outcome(), Some(Outcome::GaveUp { step: PREPARE, restarts: 0 }));
}

#[test]
fn group_facts_follow_transitions_without_reask_duplicates() {
    use super::fixtures::take_facts;
    use crate::terminal::Fact;
    for recorded in [false, true] {
        let mut rig = stoppable_group(recorded);
        rig.input(Input::Ask { forge: 1 });
        assert_eq!(take_facts(&mut rig.out).as_ref(), &[Fact::GroupStarting { forge: 1 }, Fact::Starting { step: 0 }]);
        rig.input(Input::Ask { forge: 1 });
        assert!(rig.out.is_empty());
        rig.started(0);
        assert_eq!(take_facts(&mut rig.out).as_ref(), &[Fact::Starting { step: 1 }]);
        rig.completed(1);
        assert_eq!(take_facts(&mut rig.out).as_ref(), &[Fact::Starting { step: 2 }]);
        rig.started(2);
        if recorded {
            assert!(take_facts(&mut rig.out).is_empty(), "the record gates started");
            rig.input(Input::Ask { forge: 1 });
            assert!(rig.out.is_empty());
            rig.input(Input::Written { forge: 1 });
        }
        assert_eq!(take_facts(&mut rig.out).as_ref(), &[Fact::GroupStarted { forge: 1 }]);
        rig.input(Input::Ask { forge: 1 });
        assert!(take_facts(&mut rig.out).is_empty(), "started reask is wire-only");
        rig.input(Input::Stop { forge: 1 });
        assert!(take_facts(&mut rig.out).is_empty(), "stop waits for closure");
        for step in [2, 0] {
            rig.exited(step, Exit::Signal(15));
            assert!(take_facts(&mut rig.out).is_empty(), "intentional exits do not restart");
            rig.closed(step);
            let facts = take_facts(&mut rig.out);
            if step == 0 {
                assert_eq!(facts.as_ref(), &[Fact::GroupStopped { forge: 1 }]);
            } else {
                assert!(facts.is_empty());
            }
        }
        rig.input(Input::Stop { forge: 1 });
        assert!(take_facts(&mut rig.out).is_empty(), "dormant stop is wire-only");
        assert_eq!(rig.machine.outcome(), None, "serve remains live");
    }
}

#[test]
fn group_failure_facts_keep_exit_spawn_readiness_and_record_reasons_until_closure() {
    use super::fixtures::{step, take_facts};
    use crate::{
        layout,
        machine::{Machine, Why},
        terminal::Fact,
    };
    use skein_lib::{Queue, Time, bytes};
    use temper_forge_setup::{Group, Stage, Steps};
    let placed = layout::flatten(Steps {
        always: Box::new([]),
        asked: Box::new([Group {
            forge: 7,
            before: 0,
            recorded: true,
            label: bytes::copy_of(b"unrelated label"),
            steps: Box::new([(Stage::Run, step(b"child", Kind::Long).with_ready(([127, 0, 0, 1], 3000).into()))]),
        }]),
    });
    for (input, why) in [
        (Input::NotStarted { step: 0 }, Why::NotStarted { step: 0 }),
        (Input::NotReady { step: 0 }, Why::NotReady { step: 0 }),
        (Input::NotWritten { forge: 7 }, Why::NotWritten),
        (Input::Exited { step: 0, exit: Exit::Code(0) }, Why::Exited { step: 0, exit: Exit::Code(0) }),
        (Input::Exited { step: 0, exit: Exit::Code(9) }, Why::Exited { step: 0, exit: Exit::Code(9) }),
        (Input::Exited { step: 0, exit: Exit::Signal(9) }, Why::Exited { step: 0, exit: Exit::Signal(9) }),
    ] {
        let machine = Machine::new(&limits::small(), &placed).unwrap();
        let mut rig = Rig { out: Queue::with_capacity(machine.max_out()), machine, now: Time::ZERO };
        rig.machine.start(rig.now, &[], &mut rig.out);
        rig.input(Input::Ask { forge: 7 });
        take_facts(&mut rig.out);
        if input != (Input::NotStarted { step: 0 }) {
            rig.started(0);
            take_facts(&mut rig.out);
            if why == Why::NotWritten {
                rig.input(Input::Ready { step: 0 });
                take_facts(&mut rig.out);
            }
        }
        rig.input(input);
        if input != (Input::NotStarted { step: 0 }) {
            assert!(take_facts(&mut rig.out).is_empty(), "{why:?}: failure waits for closure");
            rig.closed(0);
        }
        assert_eq!(take_facts(&mut rig.out).as_ref(), &[Fact::GroupFailed { forge: 7, why }]);
        rig.closed(0);
        rig.input(Input::Ask { forge: 99 });
        assert!(take_facts(&mut rig.out).is_empty(), "no duplicate failure or notices for refusals");
        assert_eq!(rig.machine.outcome(), None);
    }
}

#[test]
fn restart_and_give_up_facts_keep_exact_reasons_at_the_decision() {
    use super::fixtures::take_facts;
    use crate::{machine::Why, terminal::Fact};
    for exit in [Exit::Code(0), Exit::Code(17), Exit::Signal(31)] {
        let mut rig = Rig::running();
        let why = Why::Exited { step: FORGEJO, exit };
        for millis in [500, 1_000, 2_000, 4_000, 8_000] {
            rig.exited(FORGEJO, exit);
            assert!(take_facts(&mut rig.out).is_empty(), "no retry before Closed");
            rig.closed(FORGEJO);
            let delay = Duration::from_millis(millis);
            assert_eq!(take_facts(&mut rig.out).as_ref(), &[Fact::Restarting { step: FORGEJO, why, delay }]);
            assert_eq!(rig.machine.next_deadline(), Some(rig.now.saturating_add(delay)));
            rig.advance(delay);
            rig.effects();
            rig.restart_forgejo();
        }
        rig.exited(FORGEJO, exit);
        take_facts(&mut rig.out);
        rig.closed(FORGEJO);
        assert_eq!(rig.out.pop(), Some(Order::Terminal { fact: Fact::GivingUp { step: FORGEJO, why } }));
        assert_eq!(
            rig.out.pop(),
            Some(Order::Signal { step: SERVE }),
            "give-up precedes teardown of another live child"
        );
        assert!(!rig.machine.is_idle());
        assert_eq!(rig.machine.outcome(), Some(Outcome::GaveUp { step: FORGEJO, restarts: 5 }));
        for step in [SERVE, RUNNER] {
            rig.exited(step, Exit::Signal(15));
            rig.closed(step);
        }
        assert!(take_facts(&mut rig.out).is_empty(), "settlement does not repeat failure");
        assert!(rig.machine.is_idle());
    }
}

#[test]
fn retry_preparation_and_readiness_give_up_keep_the_actual_step_reason() {
    use super::fixtures::{step, take_facts};
    use crate::{
        layout,
        machine::{Machine, Why},
        terminal::Fact,
    };
    use skein_lib::{Queue, Time};
    use temper_forge_setup::Steps;
    for not_started in [false, true] {
        let mut rig = Rig::running();
        rig.failed_long(SERVE);
        rig.advance(limits::small().backoff);
        rig.effects();
        let why = if not_started {
            rig.input(Input::NotStarted { step: ONCE_1 });
            Why::NotStarted { step: ONCE_1 }
        } else {
            rig.started(ONCE_1);
            rig.exited(ONCE_1, Exit::Signal(12));
            assert!(take_facts(&mut rig.out).is_empty());
            rig.closed(ONCE_1);
            Why::Exited { step: ONCE_1, exit: Exit::Signal(12) }
        };
        assert_eq!(take_facts(&mut rig.out).as_ref(), &[Fact::GivingUp { step: ONCE_1, why }]);
        assert!(!rig.machine.is_idle());
    }
    let placed = layout::flatten(Steps {
        always: Box::new([step(b"forge", Kind::Long).with_ready(([127, 0, 0, 1], 3000).into())]),
        asked: Box::new([]),
    });
    let mut machine = Machine::new(&limits::small(), &placed).unwrap();
    let mut out = Queue::with_capacity(machine.max_out());
    machine.start(Time::ZERO, &[], &mut out);
    machine.input(Time::ZERO, Input::Started { step: 0 }, &mut out);
    take_facts(&mut out);
    machine.input(Time::ZERO, Input::NotReady { step: 0 }, &mut out);
    assert_eq!(out.pop(), Some(Order::Terminal { fact: Fact::GivingUp { step: 0, why: Why::NotReady { step: 0 } } }));
    assert_eq!(out.pop(), Some(Order::Signal { step: 0 }));
    machine.input(Time::ZERO, Input::Exited { step: 0, exit: Exit::Code(0) }, &mut out);
    machine.input(Time::ZERO, Input::Closed { step: 0 }, &mut out);
    assert!(take_facts(&mut out).is_empty());
}

#[test]
fn spawn_refusal_retry_fact_and_inclusive_window_reset_use_the_selected_delay() {
    use super::fixtures::take_facts;
    use crate::{machine::Why, terminal::Fact};
    let limits = limits::small();
    let mut rig = Rig::new(&limits);
    rig.completed(PREPARE);
    rig.effects();
    rig.input(Input::NotStarted { step: FORGEJO });
    assert_eq!(
        take_facts(&mut rig.out).as_ref(),
        &[Fact::Restarting { step: FORGEJO, why: Why::NotStarted { step: FORGEJO }, delay: limits.backoff }]
    );
    rig.advance(limits.backoff);
    rig.effects();
    rig.restart_forgejo();
    // Exactly the window retains the first exit; one nanosecond beyond the
    // window from the latest exit removes both and starts again at 500 ms.
    rig.now = skein_lib::Time::ZERO.saturating_add(limits.window);
    for delay in [Duration::from_secs(1), limits.backoff] {
        let why = Why::Exited { step: FORGEJO, exit: Exit::Code(0) };
        rig.exited(FORGEJO, Exit::Code(0));
        rig.closed(FORGEJO);
        assert_eq!(take_facts(&mut rig.out).as_ref(), &[Fact::Restarting { step: FORGEJO, why, delay }]);
        let exit_at = rig.now;
        rig.advance(delay);
        rig.effects();
        rig.restart_forgejo();
        rig.now = exit_at.saturating_add(limits.window).saturating_add(Duration::from_nanos(1));
    }
}

#[test]
fn intentional_stop_exits_and_spawn_refusals_emit_no_restart_or_give_up() {
    use super::fixtures::take_facts;
    use crate::terminal::Fact;
    for exit in [Exit::Code(0), Exit::Code(1), Exit::Signal(15)] {
        let mut rig = Rig::running();
        rig.terminate();
        assert_eq!(take_facts(&mut rig.out).as_ref(), &[Fact::Stopping]);
        for step in [SERVE, RUNNER, FORGEJO] {
            rig.exited(step, exit);
            rig.closed(step);
            let facts = take_facts(&mut rig.out);
            if step == FORGEJO {
                assert_eq!(facts.as_ref(), &[Fact::Stopped]);
            } else {
                assert!(facts.is_empty());
            }
        }
        assert_eq!(rig.machine.outcome(), Some(Outcome::Stopped));
    }
    let mut rig = Rig::new(&limits::small());
    rig.effects();
    rig.terminate();
    take_facts(&mut rig.out);
    rig.input(Input::NotStarted { step: PREPARE });
    assert_eq!(take_facts(&mut rig.out).as_ref(), &[Fact::Stopped]);
    assert_eq!(rig.machine.outcome(), Some(Outcome::Stopped));
}

#[test]
fn max_out_covers_simultaneous_retry_and_group_failure_emissions() {
    use super::fixtures::step;
    use crate::{layout, machine::Machine};
    use skein_lib::{List, Queue, Time, bytes};
    use temper_forge_setup::{Group, Stage, Steps};
    const COUNT: u32 = 64;
    let mut longs = List::with_capacity(COUNT);
    let mut group_steps = List::with_capacity(COUNT);
    for _ in 0..COUNT {
        longs.push(step(b"long", Kind::Long)).unwrap();
        group_steps.push((Stage::Run, step(b"long", Kind::Long))).unwrap();
    }
    for grouped in [false, true] {
        let placed = layout::flatten(if grouped {
            Steps {
                always: Box::new([]),
                asked: Box::new([Group {
                    forge: 7,
                    before: 0,
                    recorded: true,
                    label: bytes::copy_of(b"many"),
                    steps: Box::from(group_steps.as_slice()),
                }]),
            }
        } else {
            Steps { always: Box::from(longs.as_slice()), asked: Box::new([]) }
        });
        let mut machine = Machine::new(&limits::small(), &placed).unwrap();
        let mut out = Queue::with_capacity(machine.max_out());
        machine.start(Time::ZERO, &[], &mut out);
        while out.pop().is_some() {}
        if grouped {
            machine.input(Time::ZERO, Input::Ask { forge: 7 }, &mut out);
            while out.pop().is_some() {}
        }
        for step in 0..usize::try_from(COUNT).unwrap() {
            machine.input(Time::ZERO, Input::Started { step }, &mut out);
            assert!(out.len() <= machine.max_out());
            while out.pop().is_some() {}
        }
        if grouped {
            machine.input(Time::ZERO, Input::NotWritten { forge: 7 }, &mut out);
            assert_eq!(out.len(), COUNT, "failure closes every live group member within the reservation");
            while out.pop().is_some() {}
        } else {
            for step in 0..usize::try_from(COUNT).unwrap() {
                machine.input(Time::ZERO, Input::Exited { step, exit: Exit::Signal(9) }, &mut out);
                while out.pop().is_some() {}
            }
        }
        for step in 0..usize::try_from(COUNT).unwrap() {
            machine.input(Time::ZERO, Input::Closed { step }, &mut out);
            assert!(out.len() <= machine.max_out());
            while out.pop().is_some() {}
        }
        if !grouped {
            machine.fire(Time::ZERO.saturating_add(limits::small().backoff), &mut out);
            assert_eq!(out.len(), COUNT.checked_mul(2).unwrap(), "one Starting and Spawn per simultaneous retry");
            while out.pop().is_some() {}
            machine.input(Time::ZERO, Input::Terminate, &mut out);
            assert!(out.len() <= machine.max_out());
            let effects = take_effects(&mut out);
            assert_eq!(effects.len(), usize::try_from(COUNT).unwrap(), "one Withdraw per starting retry");
            for (step, effect) in effects.iter().enumerate() {
                assert_eq!(*effect, Order::Withdraw { step });
            }
            machine.input(Time::ZERO, Input::Terminate, &mut out);
            assert!(out.is_empty(), "a second termination withdraws nothing twice");
        }
    }
}

#[test]
fn an_unsent_spawn_is_withdrawn_at_the_first_stop() {
    use crate::terminal::Fact;
    let mut rig = Rig::new(&limits::small());
    assert_eq!(&*rig.effects(), &[Order::Spawn { step: PREPARE }]);
    rig.completed(PREPARE);
    assert_eq!(&*rig.effects(), &[Order::Close { step: PREPARE }, Order::Spawn { step: FORGEJO }]);
    rig.terminate();
    let mut effects = skein_lib::Queue::with_capacity(8);
    let mut facts = skein_lib::Queue::with_capacity(8);
    while let Some(order) = rig.out.pop() {
        effects.push(order);
        facts.push(order);
    }
    assert_eq!(&*take_effects(&mut effects), &[Order::Withdraw { step: FORGEJO }]);
    assert_eq!(&*super::fixtures::take_facts(&mut facts), &[Fact::Stopping]);
    rig.input(Input::Withdrawn { step: FORGEJO });
    assert_eq!(rig.out.pop(), Some(Order::Terminal { fact: Fact::Stopped }));
    assert!(rig.out.is_empty());
    assert!(rig.machine.is_idle());
    assert_eq!(rig.machine.outcome(), Some(Outcome::Stopped));
}

#[test]
fn an_owned_forge_starting_with_the_box_announces_its_hands_only_after_readiness() {
    use super::fixtures::{step, take_facts};
    use crate::{layout, machine::Machine, terminal::Fact};
    use skein_lib::{Queue, Time};
    use temper_forge_setup::{Group, Kind, Stage, Steps};
    for restored in [false, true] {
        let mut forge = step(b"forge", Kind::Long).with_ready(([127, 0, 0, 1], 3000).into());
        forge.set_started_notice(b"owned: started\nowned: api http://127.0.0.1:3000/api/v1\n".as_slice().into());
        let steps = if restored {
            Steps {
                always: Box::new([]),
                asked: Box::new([Group {
                    forge: 37,
                    label: b"owned".as_slice().into(),
                    recorded: true,
                    before: 0,
                    steps: Box::new([(Stage::Run, forge)]),
                }]),
            }
        } else {
            Steps { always: Box::new([forge]), asked: Box::new([]) }
        };
        let placed = layout::flatten(steps);
        let mut machine = Machine::new(&limits::small(), &placed).unwrap();
        let mut out = Queue::with_capacity(machine.max_out());
        machine.start(Time::ZERO, &[37], &mut out);
        take_facts(&mut out);
        machine.input(Time::ZERO, Input::Started { step: 0 }, &mut out);
        assert!(take_facts(&mut out).is_empty());
        machine.input(Time::ZERO, Input::Ready { step: 0 }, &mut out);
        assert_eq!(take_facts(&mut out).as_ref(), &[Fact::Started { step: 0 }]);
        machine.input(Time::ZERO, Input::Ready { step: 0 }, &mut out);
        assert!(take_facts(&mut out).is_empty(), "readiness is announced once");
    }
}

fn stopping_forgejo_spawn() -> Rig {
    let mut rig = Rig::new(&limits::small());
    rig.effects();
    rig.completed(PREPARE);
    rig.effects();
    rig.terminate();
    assert_eq!(&*rig.effects(), &[Order::Withdraw { step: FORGEJO }]);
    assert_eq!(rig.machine.next_deadline(), None, "a starting step has no deadline");
    rig
}
#[test]
fn an_admitted_spawn_after_a_second_termination_is_closed_without_another_withdraw() {
    let mut rig = stopping_forgejo_spawn();
    rig.terminate();
    assert!(rig.out.is_empty());
    rig.started(FORGEJO);
    assert_eq!(&*rig.effects(), &[Order::Close { step: FORGEJO }]);
    rig.closed(FORGEJO);
    assert!(rig.machine.is_idle());
    assert_eq!(rig.machine.outcome(), Some(Outcome::Stopped));
}
#[test]
fn a_late_spawn_failure_ends_as_a_stop_without_a_failure_fact() {
    let mut rig = stopping_forgejo_spawn();
    rig.input(Input::NotStarted { step: FORGEJO });
    assert_eq!(rig.out.pop(), Some(Order::Terminal { fact: crate::terminal::Fact::Stopped }));
    assert!(rig.out.is_empty(), "no restart, give-up or continuation");
    assert!(rig.machine.is_idle());
    assert_eq!(rig.machine.outcome(), Some(Outcome::Stopped));
}
#[test]
fn giving_up_withdraws_a_starting_preparation_and_continues_the_stop() {
    let mut rig = Rig::running();
    for millis in [500, 1000, 2000, 4000, 8000] {
        rig.failed_long(RUNNER);
        rig.advance(Duration::from_millis(millis));
        rig.started(RUNNER);
        rig.effects();
    }
    rig.failed_long(FORGEJO);
    rig.advance(Duration::from_millis(500));
    assert_eq!(&*rig.effects(), &[Order::Spawn { step: PREPARE }]);
    rig.exited(RUNNER, Exit::Code(1));
    rig.closed(RUNNER);
    assert_eq!(
        &*rig.effects(),
        &[Order::Close { step: RUNNER }, Order::Withdraw { step: PREPARE }, Order::Signal { step: SERVE }]
    );
    rig.input(Input::Withdrawn { step: PREPARE });
    assert!(rig.out.is_empty());
    rig.exited(SERVE, Exit::Code(0));
    rig.closed(SERVE);
    assert_eq!(&*rig.effects(), &[Order::Close { step: SERVE }]);
    assert!(rig.machine.is_idle());
    assert_eq!(rig.machine.outcome(), Some(Outcome::GaveUp { step: RUNNER, restarts: 5 }));
}
#[test]
fn a_group_stop_withdraws_its_preparation_before_signalling_the_earlier_child() {
    use crate::machine::Said;
    let mut rig = stoppable_group(false);
    start_stoppable(&mut rig, false);
    rig.failed_long(2);
    rig.advance(limits::small().backoff);
    assert_eq!(&*rig.effects(), &[Order::Spawn { step: 1 }]);
    rig.input(Input::Stop { forge: 1 });
    assert_eq!(&*rig.effects(), &[Order::Withdraw { step: 1 }]);
    assert_eq!(rig.machine.next_deadline(), None, "reverse stop waits on the starting step");
    rig.input(Input::Withdrawn { step: 1 });
    assert_eq!(&*rig.effects(), &[Order::Signal { step: 0 }]);
    rig.exited(0, Exit::Code(0));
    assert_eq!(&*rig.effects(), &[Order::Close { step: 0 }]);
    rig.closed(0);
    assert_eq!(&*rig.effects(), &[Order::Say { said: Said::Stopped { forge: 1 } }]);
    rig.advance(Duration::from_secs(100));
    assert!(rig.out.is_empty());
    start_stoppable(&mut rig, false);
}
#[test]
fn a_group_failure_withdraws_its_starting_step_and_can_be_asked_again() {
    use crate::machine::{Said, Why};
    let mut rig = stoppable_group(false);
    rig.input(Input::Ask { forge: 1 });
    rig.effects();
    rig.started(0);
    assert_eq!(&*rig.effects(), &[Order::Spawn { step: 1 }, Order::Say { said: Said::Step { step: 1 } }]);
    rig.exited(0, Exit::Code(1));
    assert_eq!(&*rig.effects(), &[Order::Close { step: 0 }, Order::Withdraw { step: 1 }]);
    rig.closed(0);
    assert!(rig.out.is_empty());
    rig.input(Input::Withdrawn { step: 1 });
    assert_eq!(
        &*rig.effects(),
        &[Order::Say { said: Said::Failed { forge: 1, why: Why::Exited { step: 0, exit: Exit::Code(1) } } }]
    );
    start_stoppable(&mut rig, false);
}
#[test]
fn a_box_stop_does_not_withdraw_a_group_stops_spawn_twice() {
    let mut rig = stoppable_group(false);
    start_stoppable(&mut rig, false);
    rig.failed_long(2);
    rig.advance(limits::small().backoff);
    rig.effects();
    rig.input(Input::Stop { forge: 1 });
    assert_eq!(&*rig.effects(), &[Order::Withdraw { step: 1 }]);
    rig.terminate();
    assert_eq!(&*rig.effects(), &[Order::Signal { step: 3 }]);
}
