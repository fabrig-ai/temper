//! Procedure cells, queue order and landing-rule agreement.
#![expect(clippy::trivially_copy_pass_by_ref, reason = "test helper borrows the standing snapshot")]
use crate::{
    Change, Ci, Decision, Delegate, Effect, EffectResult, Facts, Freshness, Gate, GateFailure, GateKind, GateReport,
    Heard, Hold, Limits, Pull, Ready, Repair, State, Status, first, released, step,
};
use alloc::boxed::Box;
use skein_lib::{Duration, Wall};

const HEAD: [u8; 32] = [1; 32];
const BASE: [u8; 32] = [2; 32];
const NEW: [u8; 32] = [3; 32];
const MERGE: [u8; 32] = [4; 32];
const LIMITS: Limits =
    Limits { repairs: 2, resolutions: 2, updates: 3, stall: Duration::from_secs(30), gates: 4, clean_heads: 4 };
fn at(seconds: u64) -> Wall {
    Wall::from_nanos(seconds.saturating_mul(1_000_000_000))
}
fn change(state: State) -> Change {
    Change {
        task: 7,
        state,
        gates: Box::new([]),
        clean: Box::new([]),
        gate_failures: Box::new([]),
        released: None,
        repairs: 0,
        resolutions: 0,
        updates: 0,
        since: at(0),
        ready_since: None,
        owns_turn: false,
        last_head: Some(HEAD),
    }
}
fn facts() -> Facts {
    Facts {
        branch: Some(HEAD),
        expected_base: BASE,
        pull: Pull::Open { head: HEAD, base: BASE },
        ci: Ci { head: HEAD, status: Status::Passed },
        base_ci: Ci { head: BASE, status: Status::Passed },
        base_tip: BASE,
        contains_base: Status::Passed,
        mergeable: Status::Passed,
        gates: Box::new([]),
        writer_taken: false,
        drift: None,
        first: false,
        may_merge: true,
        queue_repair_active: false,
        kept: false,
        settling: None,
    }
}
fn heard() -> Heard {
    Heard { delegate: Status::Pending, effect: EffectResult::None, released: false, cancelled: false }
}
fn decide(change: &Change, facts: &Facts, heard: &Heard) -> crate::Stepped {
    step(change, facts, heard, at(1), &LIMITS)
}

#[test]
fn a_kept_queued_or_first_change_leaves_the_queue_without_losing_its_ready_time() {
    let mut f = facts();
    f.kept = true;
    f.first = true;
    for state in [State::Queued { head: HEAD, ready_since: at(0) }, State::First { head: HEAD, ready_since: at(0) }] {
        let mut c = change(state);
        c.ready_since = Some(at(0));
        c.owns_turn = true;
        let next = decide(&c, &f, &heard());
        assert_eq!(next.change.state, State::Gating { head: HEAD, asked: Box::new([]) });
        assert_eq!(next.change.ready_since, Some(at(0)));
        assert!(!next.change.owns_turn);
        assert_eq!(next.decision, Decision::None);
    }
}

#[test]
fn a_kept_gating_change_waits_without_a_stall_deadline_or_a_ready_time() {
    let mut f = facts();
    f.kept = true;
    let c = change(State::Gating { head: HEAD, asked: Box::new([]) });
    let next = step(&c, &f, &heard(), at(60), &LIMITS);
    assert_eq!(next.decision, Decision::Wait { until: None });
    assert_eq!(next.change.state, c.state);
    assert_eq!(next.change.ready_since, None);
}

#[test]
fn an_unkept_change_is_ready_again_at_its_old_time_with_its_gate_verdict() {
    let mut c = change(State::First { head: HEAD, ready_since: at(0) });
    c.ready_since = Some(at(0));
    c.owns_turn = true;
    c.gates = Box::new([Gate {
        number: 9,
        kind: GateKind::Agent,
        blocking: true,
        freshness: Freshness::Exact,
        eager: false,
    }]);
    let mut f = facts();
    f.kept = true;
    f.gates = Box::new([GateReport { number: 9, head: HEAD, status: Status::Passed }]);
    let left = decide(&c, &f, &heard());
    let waiting = step(&left.change, &f, &heard(), at(60), &LIMITS);
    assert_eq!(waiting.decision, Decision::Wait { until: None });
    f.kept = false;
    let ready = step(&waiting.change, &f, &heard(), at(61), &LIMITS);
    assert_eq!(ready.decision, Decision::Ready);
    assert_eq!(ready.change.state, State::Queued { head: HEAD, ready_since: at(0) });
    assert_eq!(ready.change.ready_since, Some(at(0)));
    assert!(!ready.change.owns_turn);
}
#[test]
fn a_change_is_produced_opened_checked_queued_and_landed() {
    let mut f = facts();
    f.branch = None;
    f.pull = Pull::Missing;
    let one = decide(&change(State::Producing { requested: false }), &f, &heard());
    assert_eq!(one.decision, Decision::Delegate(Delegate::Produce));
    assert_eq!(one.change.state, State::Producing { requested: true });
    assert_eq!(decide(&one.change, &f, &heard()).decision, Decision::Wait { until: Some(at(30)) });
    f.branch = Some(HEAD);
    let two = decide(&one.change, &f, &heard());
    assert_eq!(two.change.state, State::Opening { requested: false });
    let three = decide(&two.change, &f, &heard());
    assert_eq!(three.decision, Decision::Effect(Effect::Open));
    assert_eq!(decide(&three.change, &f, &heard()).decision, Decision::Wait { until: Some(at(31)) });
    f.pull = Pull::Open { head: HEAD, base: BASE };
    let four = decide(&three.change, &f, &heard());
    assert_eq!(four.change.state, State::Checking { head: HEAD });
    let five = decide(&four.change, &f, &heard());
    assert_eq!(five.change.state, State::Gating { head: HEAD, asked: Box::new([]) });
    let six = decide(&five.change, &f, &heard());
    assert_eq!(six.decision, Decision::Ready);
    assert_eq!(six.change.state, State::Queued { head: HEAD, ready_since: at(1) });
    assert_eq!(decide(&six.change, &f, &heard()).decision, Decision::Wait { until: None });
    f.first = true;
    let seven = decide(&six.change, &f, &heard());
    assert_eq!(seven.change.state, State::First { head: HEAD, ready_since: at(1) });
    let eight = decide(&seven.change, &f, &heard());
    assert_eq!(eight.decision, Decision::Effect(Effect::Merge { head: HEAD, base: BASE }));
    assert_eq!(decide(&eight.change, &f, &heard()).decision, Decision::Wait { until: Some(at(31)) });
    f.pull = Pull::Merged { commit: MERGE };
    let nine = decide(&eight.change, &f, &heard());
    assert_eq!(nine.change.state, State::Landed { merge: MERGE });
    assert_eq!(nine.decision, Decision::Finish { merge: MERGE });
}
#[test]
fn a_clean_update_preserves_the_turn_and_carries_a_review() {
    let gate = Gate { number: 9, kind: GateKind::Agent, blocking: true, freshness: Freshness::Clean, eager: false };
    let mut c = change(State::First { head: HEAD, ready_since: at(0) });
    c.gates = Box::new([gate]);
    c.ready_since = Some(at(0));
    c.owns_turn = true;
    let mut f = facts();
    f.contains_base = Status::Failed;
    f.gates = Box::new([GateReport { number: 9, head: HEAD, status: Status::Passed }]);
    let one = decide(&c, &f, &heard());
    assert_eq!(one.decision, Decision::Effect(Effect::Update { head: HEAD, base: BASE }));
    assert_eq!(one.change.state, State::Updating { from: HEAD, base: BASE, ready_since: at(0) });
    let mut h = heard();
    h.effect = EffectResult::Made;
    f.branch = Some(NEW);
    f.pull = Pull::Open { head: NEW, base: BASE };
    f.ci = Ci { head: NEW, status: Status::Pending };
    let two = decide(&one.change, &f, &h);
    assert_eq!(two.change.state, State::Checking { head: NEW });
    assert_eq!(two.change.clean.as_ref(), &[HEAD]);
    f.ci.status = Status::Passed;
    let three = decide(&two.change, &f, &heard());
    let four = decide(&three.change, &f, &heard());
    assert_eq!(four.change.state, State::First { head: NEW, ready_since: at(0) });
    assert_eq!(four.decision, Decision::Ready);
}
#[test]
fn a_conflict_leaves_the_queue_and_reasks_gates_after_resolution() {
    let mut c = change(State::Updating { from: HEAD, base: BASE, ready_since: at(0) });
    c.owns_turn = true;
    c.ready_since = Some(at(0));
    let mut h = heard();
    h.effect = EffectResult::Conflict;
    let one = decide(&c, &facts(), &h);
    assert_eq!(one.decision, Decision::Delegate(Delegate::Resolve { base: BASE }));
    assert_eq!(one.change.state, State::Resolving { requested: true, base: BASE });
    assert!(!one.change.owns_turn);
    h.effect = EffectResult::None;
    h.delegate = Status::Passed;
    let mut f = facts();
    f.branch = Some(NEW);
    let two = decide(&one.change, &f, &h);
    assert_eq!(two.change.state, State::Checking { head: NEW });
    assert!(two.change.clean.is_empty());
}
#[test]
fn a_failed_check_takes_the_base_before_a_repair_and_both_bounds_hold() {
    let mut f = facts();
    f.ci.status = Status::Failed;
    f.contains_base = Status::Failed;
    let one = decide(&change(State::Checking { head: HEAD }), &f, &heard());
    assert_eq!(one.decision, Decision::Effect(Effect::Update { head: HEAD, base: BASE }));
    f.contains_base = Status::Passed;
    let two = decide(&change(State::Checking { head: HEAD }), &f, &heard());
    assert_eq!(two.decision, Decision::Delegate(Delegate::Repair(Repair::Ci)));
    let mut capped = change(State::Checking { head: HEAD });
    capped.repairs = LIMITS.repairs;
    assert_eq!(decide(&capped, &f, &heard()).decision, Decision::Hold(Hold::Repairs));
    capped.state = State::Updating { from: HEAD, base: BASE, ready_since: at(0) };
    capped.resolutions = LIMITS.resolutions;
    let mut h = heard();
    h.effect = EffectResult::Conflict;
    assert_eq!(decide(&capped, &f, &h).decision, Decision::Hold(Hold::Resolutions));
}
#[test]
fn a_failed_clean_update_is_a_semantic_repair_and_a_broken_base_gets_one_queue_repair() {
    let mut c = change(State::Checking { head: HEAD });
    c.clean = Box::new([NEW]);
    let mut f = facts();
    f.ci.status = Status::Failed;
    assert_eq!(decide(&c, &f, &heard()).decision, Decision::Delegate(Delegate::Repair(Repair::Semantic)));
    c.state = State::First { head: HEAD, ready_since: at(0) };
    f.ci.status = Status::Passed;
    f.base_ci.status = Status::Failed;
    assert_eq!(decide(&c, &f, &heard()).decision, Decision::QueueRepair);
    f.queue_repair_active = true;
    assert_eq!(decide(&c, &f, &heard()).decision, Decision::Wait { until: None });
}
#[test]
fn release_reopens_a_closed_pull_and_recreates_a_deleted_branch() {
    let mut c = change(State::Held { was: Box::new(State::Checking { head: HEAD }), why: Hold::PullClosed });
    let mut h = heard();
    h.released = true;
    let one = decide(&c, &facts(), &h);
    assert_eq!(one.decision, Decision::Effect(Effect::Reopen));
    assert_eq!(one.change.state, State::Reopening { requested: true, retarget: false });
    let mut f = facts();
    f.pull = Pull::Closed;
    assert_eq!(decide(&one.change, &f, &heard()).decision, Decision::Wait { until: Some(at(31)) });
    c.state = State::Held { was: Box::new(State::Checking { head: HEAD }), why: Hold::BranchMissing };
    let two = decide(&c, &f, &h);
    assert_eq!(two.decision, Decision::Effect(Effect::CreateBranch { head: HEAD }));
    assert_eq!(two.change.state, State::Recreating { head: HEAD });
}
#[test]
fn drift_writer_slots_and_stalls_do_not_write_over_a_branch() {
    let mut f = facts();
    f.writer_taken = true;
    assert_eq!(
        decide(&change(State::First { head: HEAD, ready_since: at(0) }), &f, &heard()).decision,
        Decision::Wait { until: Some(at(30)) }
    );
    f.writer_taken = false;
    f.drift = Some(Hold::BranchMoved);
    assert_eq!(
        decide(&change(State::First { head: HEAD, ready_since: at(0) }), &f, &heard()).decision,
        Decision::Hold(Hold::BranchMoved)
    );
    f.drift = None;
    let held = step(&change(State::Checking { head: HEAD }), &f, &heard(), at(31), &LIMITS);
    assert_eq!(held.change.state, State::Gating { head: HEAD, asked: Box::new([]) });
    f.ci.status = Status::Pending;
    assert_eq!(
        step(&change(State::Checking { head: HEAD }), &f, &heard(), at(31), &LIMITS).decision,
        Decision::Hold(Hold::Stalled)
    );
}
#[test]
fn generated_phase_cells_decide_the_same_from_the_same_snapshot() {
    let states = [
        State::Producing { requested: false },
        State::Producing { requested: true },
        State::Opening { requested: false },
        State::Opening { requested: true },
        State::Recreating { head: HEAD },
        State::Reopening { requested: true, retarget: false },
        State::Checking { head: HEAD },
        State::Gating { head: HEAD, asked: Box::new([]) },
        State::Queued { head: HEAD, ready_since: at(0) },
        State::First { head: HEAD, ready_since: at(0) },
        State::Updating { from: HEAD, base: BASE, ready_since: at(0) },
        State::Resolving { requested: true, base: BASE },
        State::Repairing { requested: true, why: Repair::Ci },
        State::Landing { head: HEAD },
        State::Landed { merge: MERGE },
        State::Held { was: Box::new(State::Checking { head: HEAD }), why: Hold::Stalled },
    ];
    let statuses = [Status::Unknown, Status::Pending, Status::Passed, Status::Failed];
    let effects =
        [EffectResult::None, EffectResult::Pending, EffectResult::Made, EffectResult::Conflict, EffectResult::Failed];
    for state in states {
        for status in statuses {
            for effect in effects {
                let mut f = facts();
                f.ci.status = status;
                f.mergeable = status;
                let mut h = heard();
                h.delegate = status;
                h.effect = effect;
                let c = change(state.clone());
                assert_eq!(decide(&c, &f, &h), decide(&c, &f, &h));
            }
        }
    }
}
#[test]
fn queue_age_precedes_later_priority_and_priority_breaks_young_ties() {
    let ready = [
        Ready { task: 1, priority: 1, since: at(1) },
        Ready { task: 2, priority: 10, since: at(15) },
        Ready { task: 3, priority: 20, since: at(17) },
    ];
    assert_eq!(first(&ready, Duration::from_secs(10), at(20)), Some(1));
    assert_eq!(first(&ready, Duration::from_secs(10), at(18)), Some(1));
    assert_eq!(first(&ready[1..], Duration::from_secs(10), at(18)), Some(3));
}
#[test]
fn generated_queue_order_agrees_with_an_independent_rank_statement() {
    for seed in 0..200_u64 {
        let now = at(100);
        let window = Duration::from_secs(20);
        let mut ready = [Ready { task: 0, priority: 0, since: at(0) }; 4];
        for (index, item) in ready.iter_mut().enumerate() {
            let number = u64::try_from(index).expect("index fits");
            *item = Ready {
                task: number + 1,
                priority: i32::try_from((seed.wrapping_mul(17).wrapping_add(number * 13)) % 9).expect("rank fits"),
                since: at(60 + ((seed.wrapping_mul(31).wrapping_add(number * 11)) % 39)),
            };
        }
        let mut expected = ready[0];
        for item in &ready[1..] {
            let item_aged = now.as_nanos() - item.since.as_nanos() >= window.as_nanos();
            let old_aged = now.as_nanos() - expected.since.as_nanos() >= window.as_nanos();
            let item_wins = if item_aged != old_aged {
                item_aged
            } else if item_aged && item.since != expected.since {
                item.since < expected.since
            } else if item.priority != expected.priority {
                item.priority > expected.priority
            } else if item.since != expected.since {
                item.since < expected.since
            } else {
                item.task < expected.task
            };
            if item_wins {
                expected = *item;
            }
        }
        assert_eq!(first(&ready, window, now), Some(expected.task), "seed {seed}");
    }
}

#[test]
fn a_pending_wait_inside_the_settle_window_ends_with_it_and_never_stalls() {
    let mut f = facts();
    f.mergeable = Status::Pending;
    f.settling = Some(at(50));
    let decision =
        step(&change(State::First { head: HEAD, ready_since: at(0) }), &f, &heard(), at(40), &LIMITS).decision;
    assert_eq!(decision, Decision::Wait { until: Some(at(50)) });
}

#[test]
fn a_pending_wait_past_the_settle_window_stalls_as_any_wait() {
    let mut f = facts();
    f.mergeable = Status::Pending;
    f.settling = Some(at(40));
    let decision =
        step(&change(State::First { head: HEAD, ready_since: at(0) }), &f, &heard(), at(40), &LIMITS).decision;
    assert_eq!(decision, Decision::Hold(Hold::Stalled));
}

fn reviewing() -> Change {
    let mut c = change(State::Gating { head: HEAD, asked: Box::new([9]) });
    c.gates = Box::new([Gate {
        number: 9,
        kind: GateKind::Agent,
        blocking: true,
        freshness: Freshness::Exact,
        eager: false,
    }]);
    c
}

#[test]
fn a_failed_review_is_asked_again_at_the_same_head() {
    let failed = crate::review_failed(&reviewing(), 9, HEAD, at(2), &LIMITS);
    assert_eq!(failed.gate_failures.as_ref(), &[crate::GateFailure { number: 9, count: 1 }]);
    let next = step(&failed, &facts(), &heard(), at(3), &LIMITS);
    assert_eq!(next.decision, Decision::Delegate(Delegate::Gate { number: 9, head: HEAD }));
    assert_eq!(next.change.repairs, 0);
    assert_eq!(next.change.state, State::Gating { head: HEAD, asked: Box::new([9]) });
    assert!(
        decide(&next.change, &facts(), &heard()).decision == Decision::Wait { until: Some(at(32)) },
        "a repeated snapshot does not request a third review"
    );
}

#[test]
fn a_second_failed_review_holds_the_change() {
    let failed = crate::review_failed(&reviewing(), 9, HEAD, at(2), &LIMITS);
    let retry = step(&failed, &facts(), &heard(), at(3), &LIMITS);
    let twice = crate::review_failed(&retry.change, 9, HEAD, at(4), &LIMITS);
    let next = step(&twice, &facts(), &heard(), at(5), &LIMITS);
    assert_eq!(next.decision, Decision::Hold(Hold::Failed));
    assert_eq!(next.change.repairs, 0);
    assert_eq!(next.change.gate_failures.as_ref(), &[crate::GateFailure { number: 9, count: 2 }]);
    assert_eq!(decide(&next.change, &facts(), &heard()).decision, Decision::None);
}

#[test]
fn a_new_head_starts_its_review_failures_at_zero() {
    let mut c = reviewing();
    c.gate_failures = Box::new([crate::GateFailure { number: 9, count: 2 }]);
    c.state = State::Checking { head: NEW };
    let mut f = facts();
    f.branch = Some(NEW);
    f.pull = Pull::Open { head: NEW, base: BASE };
    f.ci.head = NEW;
    let checked = decide(&c, &f, &heard());
    assert!(checked.change.gate_failures.is_empty());
    let review = decide(&checked.change, &f, &heard());
    let failed = crate::review_failed(&review.change, 9, NEW, at(2), &LIMITS);
    assert_eq!(failed.gate_failures.as_ref(), &[crate::GateFailure { number: 9, count: 1 }]);
    assert_eq!(decide(&failed, &f, &heard()).decision, Decision::Delegate(Delegate::Gate { number: 9, head: NEW }));
}

#[test]
fn each_gate_has_its_own_failure_count() {
    let mut c = reviewing();
    c.gates = Box::new([c.gates[0], Gate { number: 10, ..c.gates[0] }]);
    c.state = State::Gating { head: HEAD, asked: Box::new([9, 10]) };
    let one = crate::review_failed(&c, 9, HEAD, at(2), &LIMITS);
    let two = crate::review_failed(&one, 10, HEAD, at(3), &LIMITS);
    assert_eq!(
        two.gate_failures.as_ref(),
        &[crate::GateFailure { number: 9, count: 1 }, crate::GateFailure { number: 10, count: 1 }]
    );
    assert_eq!(decide(&two, &facts(), &heard()).decision, Decision::Delegate(Delegate::Gate { number: 9, head: HEAD }));
}

#[test]
fn a_release_resets_failed_review_tries_before_any_forge_read() {
    let mut c =
        change(State::Held { was: Box::new(State::Gating { head: HEAD, asked: Box::new([]) }), why: Hold::Failed });
    c.gate_failures = Box::new([GateFailure { number: 1, count: 2 }]);
    c.repairs = LIMITS.repairs;
    let lifted = released(&c, at(5));
    assert!(!matches!(lifted.state, State::Held { .. }));
    assert!(lifted.gate_failures.is_empty());
    assert_eq!(lifted.released, Some(Hold::Failed));
    assert_eq!(lifted.repairs, 0);
    assert_eq!(lifted.since, at(5));
    let decided = step(&lifted, &facts(), &heard(), at(10), &LIMITS);
    assert_eq!(decided.change.released, None);
    assert_eq!(decided.change.since, at(5), "a slow read does not restart the release's stall clock");
}

#[test]
fn a_cancel_ends_every_held_cause_unless_the_change_landed() {
    for why in [
        Hold::Failed,
        Hold::Repairs,
        Hold::Rejected,
        Hold::BranchMoved,
        Hold::Stalled,
        Hold::Resolutions,
        Hold::Updates,
        Hold::BranchMissing,
        Hold::PullClosed,
        Hold::Retargeted,
    ] {
        let c = change(State::Held { was: Box::new(State::Gating { head: HEAD, asked: Box::new([]) }), why });
        let mut h = heard();
        h.cancelled = true;
        let ended = decide(&c, &facts(), &h);
        assert_eq!(ended.decision, Decision::Cancel, "{why:?}");
        assert_eq!(ended.change.state, State::Ended, "{why:?}");
        let mut f = facts();
        f.pull = Pull::Merged { commit: MERGE };
        let landed = decide(&c, &f, &h);
        assert_eq!(landed.change.state, State::Landed { merge: MERGE });
        assert_eq!(landed.decision, Decision::Finish { merge: MERGE });
    }
}
