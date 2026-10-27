use super::*;
use jig_domain_people::ApplicationScope;
use temper_engine_domain_forge_change as change;

fn panel(d: &Domain) -> Panel {
    d.panel(fixtures::CHANGE, ApplicationScope::Project(5), env().wall, &LIMITS).expect("known change")
}
fn with_row(row: ChangeRow) -> Domain {
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    outputs(&mut d, Event::Restore { record: Stored::Change(row) });
    d
}
#[test]
fn each_step_and_held_step_comes_from_the_retained_record() {
    let head = fixtures::HEAD;
    let since = env().wall;
    let rows = [
        (change::State::Producing { requested: true }, PanelStep::Producing),
        (change::State::Opening { requested: false }, PanelStep::Opening),
        (change::State::Recreating { head }, PanelStep::Recreating),
        (change::State::Reopening { requested: true, retarget: true }, PanelStep::Reopening),
        (change::State::Checking { head }, PanelStep::Checking),
        (change::State::Gating { head, asked: Box::new([]) }, PanelStep::Gating),
        (change::State::Queued { head, ready_since: since }, PanelStep::Queued { place: 1, ahead: Box::new([]) }),
        (change::State::First { head, ready_since: since }, PanelStep::First),
        (change::State::Updating { from: head, base: fixtures::BASE, ready_since: since }, PanelStep::Updating),
        (change::State::Resolving { requested: true, base: fixtures::BASE }, PanelStep::Resolving),
        (
            change::State::Repairing { requested: true, why: change::Repair::Gate(7) },
            PanelStep::Repairing(change::Repair::Gate(7)),
        ),
        (change::State::Landing { head }, PanelStep::Landing),
        (change::State::Landed { merge: fixtures::BASE }, PanelStep::Landed { merge: fixtures::BASE }),
    ];
    for (state, step) in rows {
        for hold in [None, Some(change::Hold::Stalled)] {
            let mut row = fixtures::opening_row();
            row.change.last_head = Some(head);
            row.change.ready_since = Some(since);
            row.change.state = match hold {
                Some(why) => change::State::Held { was: Box::new(state.clone()), why },
                None => state.clone(),
            };
            let d = with_row(row.clone());
            let got = panel(&d);
            assert_eq!(got.step, step);
            assert_eq!(got.hold, hold);
            assert_eq!(got.task, row.task);
            assert_eq!(got.forge, REPO.forge);
            assert_eq!(got.owner.as_ref(), b"org");
            assert_eq!(got.name.as_ref(), b"repo");
            assert_eq!(got.branch, row.branch);
            assert_eq!(got.landing, row.base);
            assert_eq!(got.pull, row.pull);
            assert_eq!(got.head, Some(head));
            assert_eq!(got.repairs, PanelUsed { used: row.change.repairs, limit: LIMITS.change_policy.repairs });
            assert!(got.within());
        }
    }
}
#[test]
fn unknown_subjects_and_other_scopes_have_no_panel() {
    let d = with_row(fixtures::opening_row());
    for (task, scope) in [
        (0, ApplicationScope::Project(5)),
        (999, ApplicationScope::Project(5)),
        (fixtures::CHANGE, ApplicationScope::Project(6)),
        (fixtures::CHANGE, ApplicationScope::Deployment),
        (fixtures::CHANGE, ApplicationScope::Own),
    ] {
        assert_eq!(d.panel(task, scope, env().wall, &LIMITS), None);
    }
}
#[test]
fn every_record_save_marks_its_change_after_the_save() {
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    let row = fixtures::opening_row();
    assert_eq!(
        outputs(&mut d, Event::Change { row: row.clone() }).as_ref(),
        &[
            Request::Save { record: Stored::Change(row.clone()), release: None, read_afresh: None },
            Request::Marked { tasks: Box::new([row.task]) },
        ]
    );
}

fn queued(task: u64, priority: i32, seconds: u64) -> ChangeRow {
    let mut row = fixtures::opening_row();
    row.task = task;
    row.change.task = task;
    row.priority = priority;
    let since = Wall::from_nanos(seconds.checked_mul(1_000_000_000).expect("test time"));
    row.change.state = change::State::Queued { head: fixtures::HEAD, ready_since: since };
    row.change.ready_since = Some(since);
    row.pending = None;
    row
}
#[test]
fn queue_place_uses_priority_time_aging_and_the_base_repairs_precedence() {
    type QueueCase = ([(u64, i32, u64); 3], bool, [u64; 3]);
    let cases: [QueueCase; 3] = [
        ([(51, 9, 95), (52, 1, 96), (53, 9, 90)], false, [52, 53, 51]),
        ([(51, 9, 60), (52, 1, 96), (53, 9, 90)], false, [51, 52, 53]),
        ([(51, 9, 95), (52, 1, 96), (53, 9, 90)], true, [51, 52, 53]),
    ];
    for (rows, repair, expected) in cases {
        let mut d = domain();
        outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
        for (task, priority, seconds) in rows {
            let mut row = queued(task, priority, seconds);
            row.base_repair = repair && task == 51;
            outputs(&mut d, Event::Restore { record: Stored::Change(row) });
        }
        for (index, task) in expected.iter().enumerate() {
            let got = d.panel(*task, ApplicationScope::Project(5), env().wall, &LIMITS).expect("queued panel");
            let mut ahead = skein_lib::List::with_capacity(PANEL_AHEAD);
            for predecessor in expected.get(..index).expect("predecessors").iter().rev() {
                ahead.push(*predecessor).expect("ahead room");
            }
            assert_eq!(
                got.step,
                PanelStep::Queued { place: u32::try_from(index + 1).expect("place"), ahead: ahead.into_boxed() }
            );
        }
    }
}
#[test]
fn a_long_queue_counts_every_place_but_carries_only_the_nearest_ahead() {
    const CHANGES: u32 = PANEL_AHEAD + 3;
    let limits = Limits { changes: CHANGES, ..LIMITS };
    let mut d = Domain::new(
        &limits,
        4,
        skein_forge_client::Config { namespace: Box::from(&b"test"[..]), writers: Box::new([]) },
        &[],
        &[],
        false,
    )
    .expect("long queue bounds");
    let environment = Env { limits, ..env() };
    let mut out = Queue::with_capacity(max_out(&limits));
    step(&mut d, &environment, Event::Restore { record: Stored::Repository(adopted()) }, &mut out);
    for task in 1..=u64::from(CHANGES) {
        step(&mut d, &environment, Event::Restore { record: Stored::Change(queued(task, 0, 95)) }, &mut out);
    }
    let got = d.panel(u64::from(CHANGES), ApplicationScope::Project(5), env().wall, &limits).expect("last panel");
    let mut ahead = skein_lib::List::with_capacity(PANEL_AHEAD);
    for task in (u64::from(CHANGES - PANEL_AHEAD)..u64::from(CHANGES)).rev() {
        ahead.push(task).expect("ahead room");
    }
    assert_eq!(got.step, PanelStep::Queued { place: CHANGES, ahead: ahead.into_boxed() });
}
#[test]
fn queue_entry_and_departure_mark_the_nearest_followers_within_the_subject_bound() {
    let mut d = with_row(queued(52, 2, 90));
    let expected = queued(51, 1, 90);
    let got = outputs(&mut d, Event::Change { row: expected.clone() });
    assert_eq!(
        got.as_ref(),
        &[
            Request::Save { record: Stored::Change(expected), release: None, read_afresh: None },
            Request::Marked { tasks: Box::new([51, 52]) },
        ]
    );
    // Refusing an outstanding delegate holds the queued change, taking it out.
    outputs(&mut d, Event::Delegated { task: 51, child: 90, kind: change::Delegate::Produce });
    let got = outputs(&mut d, Event::DelegateRefused { task: 51, child: 90 });
    let [Request::Save { .. }, Request::Marked { tasks }] = got.as_ref() else { panic!("save and mark: {got:?}") };
    assert_eq!(tasks.as_ref(), &[51, 52]);
}
#[test]
fn gates_are_judged_at_the_head_with_failed_and_pending_verdicts_taking_precedence() {
    let mut row = fixtures::opening_row();
    row.change.gates = Box::new([
        change::Gate {
            number: 1,
            kind: change::GateKind::Agent,
            blocking: true,
            freshness: change::Freshness::Exact,
            eager: false,
        },
        change::Gate {
            number: 2,
            kind: change::GateKind::Person,
            blocking: false,
            freshness: change::Freshness::Clean,
            eager: false,
        },
        change::Gate {
            number: 3,
            kind: change::GateKind::Check,
            blocking: true,
            freshness: change::Freshness::Exact,
            eager: false,
        },
    ]);
    row.change.clean = Box::new([fixtures::BASE]);
    row.verdicts = Box::new([
        change::GateReport { number: 1, head: fixtures::BASE, status: change::Status::Passed },
        change::GateReport { number: 2, head: fixtures::BASE, status: change::Status::Passed },
        change::GateReport { number: 3, head: fixtures::HEAD, status: change::Status::Failed },
    ]);
    let got = panel(&with_row(row));
    assert_eq!(
        got.gates.as_ref(),
        &[
            PanelGate { number: 1, kind: change::GateKind::Agent, blocking: true, verdict: change::Status::Unknown },
            PanelGate { number: 2, kind: change::GateKind::Person, blocking: false, verdict: change::Status::Passed },
            PanelGate { number: 3, kind: change::GateKind::Check, blocking: true, verdict: change::Status::Failed },
        ]
    );
}
#[test]
fn a_gate_whose_review_runs_at_the_head_is_pending() {
    let gate = |number| change::Gate {
        number,
        kind: change::GateKind::Agent,
        blocking: true,
        freshness: change::Freshness::Exact,
        eager: false,
    };
    for (head, verdict) in [(fixtures::HEAD, change::Status::Pending), (fixtures::BASE, change::Status::Unknown)] {
        let mut row = fixtures::opening_row();
        row.change.gates = Box::new([gate(1), gate(2)]);
        row.change.state = change::State::Gating { head: fixtures::HEAD, asked: Box::new([1]) };
        row.delegate = Some((90, change::Delegate::Gate { number: 1, head }));
        let got = panel(&with_row(row));
        assert_eq!(got.runs, Some(90));
        assert_eq!(
            got.gates.as_ref(),
            &[
                PanelGate { number: 1, kind: change::GateKind::Agent, blocking: true, verdict },
                PanelGate {
                    number: 2,
                    kind: change::GateKind::Agent,
                    blocking: true,
                    verdict: change::Status::Unknown
                },
            ]
        );
    }
}
#[test]
fn a_fresh_ci_read_marks_a_changed_verdict_and_restart_forgets_it() {
    let (mut d, _) = fixtures::opening();
    fixtures::missing_pull(&mut d);
    let got = fixtures::answer_next(&mut d, Ok(fixtures::checks(skein_forge::api::Check::Pending)));
    assert_eq!(got.as_ref(), &[Request::Marked { tasks: Box::new([fixtures::CHANGE]) }]);
    assert_eq!(panel(&d).ci, change::Status::Pending);
    let row = d.change(fixtures::CHANGE).expect("kept change").clone();
    let mut restarted = with_row(row);
    ready(&mut restarted);
    assert_eq!(panel(&restarted).ci, change::Status::Unknown);
}
#[test]
fn output_bounds_include_marks_and_shared_demo_answers() {
    assert_eq!(
        max_out(&LIMITS),
        LIMITS.output * 2
            + LIMITS.applications
            + demo_act_bound(&LIMITS).unwrap() * 2
            + LIMITS.landings
            + LIMITS.landing_members
            + LIMITS.entries
    );
    assert_eq!(worst_case(&Limits { output: u32::MAX, ..LIMITS }), None);
    assert!(worst_case(&LIMITS).is_some());
}

#[test]
fn priority_moves_mark_the_nearest_followers_and_stop_at_the_mark_bound() {
    const CHANGES: u32 = MARKED + 3;
    let limits = Limits { changes: CHANGES, ..LIMITS };
    let mut d = Domain::new(
        &limits,
        4,
        skein_forge_client::Config { namespace: Box::from(&b"test"[..]), writers: Box::new([]) },
        &[],
        &[],
        false,
    )
    .expect("queue bounds");
    let environment = Env { limits, ..env() };
    let mut out = Queue::with_capacity(max_out(&limits));
    step(&mut d, &environment, Event::Restore { record: Stored::Repository(adopted()) }, &mut out);
    for task in 1..=u64::from(CHANGES) {
        step(&mut d, &environment, Event::Restore { record: Stored::Change(queued(task, 0, 95)) }, &mut out);
    }
    d.priority(&environment, u64::from(CHANGES), -10, &mut out);
    let Some(Request::Save { record: Stored::Change(row), .. }) = out.pop() else { panic!("priority saved") };
    assert_eq!(row.priority, -10_i32);
    let Some(Request::Marked { tasks }) = out.pop() else { panic!("priority marked") };
    let mut expected = skein_lib::List::with_capacity(MARKED);
    expected.push(u64::from(CHANGES)).expect("own task");
    for task in 1..u64::from(MARKED) {
        expected.push(task).expect("nearest followers");
    }
    assert_eq!(tasks, expected.into_boxed());
    assert!(out.is_empty());
    // Moving it behind the same followers also invalidates them in their new order.
    d.priority(&environment, u64::from(CHANGES), 10, &mut out);
    let Some(Request::Save { .. }) = out.pop() else { panic!("priority saved") };
    let Some(Request::Marked { tasks: moved }) = out.pop() else { panic!("move marked") };
    assert_eq!(moved, tasks);
    assert!(out.is_empty());
}
#[test]
fn ci_replacements_mark_once_and_repeated_reads_are_inert() {
    let mut d = with_row(fixtures::opening_row());
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    for status in [change::Status::Pending, change::Status::Passed, change::Status::Failed] {
        let read = change::Ci { head: fixtures::HEAD, status };
        domain::keep_panel_ci(&mut d, fixtures::CHANGE, read, &mut out);
        assert_eq!(out.pop(), Some(Request::Marked { tasks: Box::new([fixtures::CHANGE]) }));
        assert_eq!(panel(&d).ci, status);
        domain::keep_panel_ci(&mut d, fixtures::CHANGE, read, &mut out);
        assert!(out.is_empty());
    }
    domain::keep_panel_ci(
        &mut d,
        fixtures::CHANGE,
        change::Ci { head: fixtures::BASE, status: change::Status::Passed },
        &mut out,
    );
    assert_eq!(out.pop(), Some(Request::Marked { tasks: Box::new([fixtures::CHANGE]) }));
    assert_eq!(panel(&d).ci, change::Status::Unknown);
}

#[test]
fn terminal_panel_rejects_a_saved_verdict_at_another_head() {
    for state in [change::State::Landed { merge: fixtures::BASE }, change::State::Ended] {
        let mut row = fixtures::opening_row();
        row.change.state = state;
        row.ci = Some(change::Ci { head: fixtures::BASE, status: change::Status::Passed });
        assert_eq!(panel(&with_row(row)).ci, change::Status::Unknown);
    }
}

#[test]
fn ending_a_change_saves_its_last_ci_without_an_extra_write() {
    let mut row = fixtures::opening_row();
    row.pending = None;
    let mut d = with_row(row);
    outputs(&mut d, Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic });
    outputs(&mut d, Event::ReadAfresh);
    outputs(&mut d, Event::SettleOutbox);
    let read = change::Ci { head: fixtures::HEAD, status: change::Status::Failed };
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    domain::keep_panel_ci(&mut d, fixtures::CHANGE, read, &mut out);
    let requests = outputs(
        &mut d,
        Event::Release {
            task: fixtures::CHANGE,
            root: fixtures::CHANGE,
            ending: ReleaseEnding::Cancelled,
            reason: None,
            entry: fixtures::OPEN,
        },
    );
    let mut records = skein_lib::List::with_capacity(LIMITS.changes);
    for request in requests {
        if let Request::Save { record: Stored::Change(row), .. } = request {
            records.push(row).expect("fixture changes fit");
        }
    }
    let [saved] = records.as_slice() else { panic!("the finish's one change save") };
    assert_eq!(saved.change.state, change::State::Ended);
    assert_eq!(saved.ci, Some(read));
    assert_eq!(panel(&with_row(saved.clone())).ci, change::Status::Failed);
}

/// The parent commits every save; replaying them in order restores each key's
/// last value without copying any of the connector's volatile working set.
fn commit_panel_records(records: &mut skein_lib::List<Stored>, requests: Box<[Request]>) {
    for request in requests {
        if let Request::Save { record, .. } = request {
            records.push(record).expect("fixture's durable records fit");
        }
    }
}

#[test]
fn landed_panel_keeps_passed_ci_across_a_restart() {
    use skein_forge::api::{Answer, State};
    use skein_forge_client::RecoveryClock;

    // Repository, initial change, head-CI update, and landed change.
    const RESTART_RECORDS: u32 = 4;
    let repository = Stored::Repository(adopted());
    let mut records = skein_lib::List::with_capacity(RESTART_RECORDS);
    records.push(repository.clone()).expect("repository fits");
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: repository });
    outputs(&mut d, Event::Restored { clock: RecoveryClock::Monotonic });
    outputs(&mut d, Event::ReadAfresh);
    outputs(&mut d, Event::SettleOutbox);
    let mut row = fixtures::opening_row();
    row.pending = None;
    row.effect = change::EffectResult::None;
    row.pull = Some(fixtures::open_pull().number);
    row.change.state = change::State::Landing { head: fixtures::HEAD };
    commit_panel_records(&mut records, outputs(&mut d, Event::Change { row }));

    fixtures::step_change(&mut d);
    let mut merged = fixtures::open_pull();
    merged.state = State::Closed;
    merged.merged = Some(fixtures::BASE);
    for answer in [
        Answer::Commit(fixtures::HEAD),
        Answer::Commit(fixtures::BASE),
        Answer::Pull(merged),
        fixtures::checks(skein_forge::api::Check::Passed),
        Answer::Compare {
            before: fixtures::BASE,
            after: fixtures::HEAD,
            contains_before: true,
            files: Box::new([]),
            commits: Box::new([]),
        },
        Answer::Reviews { reviews: Box::new([]), more: false },
    ] {
        commit_panel_records(&mut records, fixtures::answer_next(&mut d, Ok(answer)));
    }
    let before = panel(&d);
    commit_panel_records(&mut records, outputs(&mut d, Event::Settle));
    drop(d);

    let mut restarted = domain();
    for record in records.into_boxed() {
        outputs(&mut restarted, Event::Restore { record });
    }
    outputs(&mut restarted, Event::Restored { clock: RecoveryClock::Monotonic });
    outputs(&mut restarted, Event::ReadAfresh);
    outputs(&mut restarted, Event::SettleOutbox);
    let after = panel(&restarted);
    assert_eq!(before.step, PanelStep::Landed { merge: fixtures::BASE });
    assert_eq!(after.step, PanelStep::Landed { merge: fixtures::BASE });
    assert_eq!(after.head, Some(fixtures::HEAD));
    assert_eq!((before.ci, after.ci), (change::Status::Passed, change::Status::Passed));
}
