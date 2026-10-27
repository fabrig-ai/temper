use crate::{
    Decision, Facts, Fault, Flight, GateFacts, GateProof, Hold, Landing, Limits, Member, MemberFacts, Phase, Posted,
    Posting, Project, Proof, Repository, Status, Stepped, Verdict, step,
};
use alloc::boxed::Box;
use skein_lib::{Duration, Wall};

const HEAD: [u8; 32] = [1; 32];
const MAIN: [u8; 32] = [2; 32];
const NEW: [u8; 32] = [3; 32];
const KEY: [u8; 32] = [4; 32];
const LIMITS: Limits = Limits { members: 4, gates: 4, poll: Duration::from_secs(5) };
const PROJECT: Project = Project { gate_retries: 2 };
fn at(seconds: u64) -> Wall {
    Wall::from_nanos(seconds.saturating_mul(1_000_000_000))
}
fn landing_of_one() -> Landing {
    Landing {
        task: 7,
        project: 9,
        phase: Phase::Waiting,
        preparation: 0,
        stale_proofs: 0,
        members: Box::new([Member {
            change: 8,
            repository: Repository { forge: 1, repository: 2 },
            pull: 10,
            head: None,
            base: None,
            candidate: None,
            gates: Box::new([GateProof { proof: Proof::Unkeyed, posted: Posted::Nothing, faults: 0 }]),
            asked: None,
            landed: None,
        }]),
    }
}
fn facts() -> Facts {
    Facts {
        members: Box::new([MemberFacts {
            ready: true,
            cancelled: false,
            head: HEAD,
            main: MAIN,
            contains_main: Status::Passed,
            asked: false,
            merge: Status::Unknown,
            merged: None,
            gates: Box::new([GateFacts {
                blocking: true,
                key: None,
                receipt: Status::Unknown,
                flight: Flight::None,
                posting: Status::Unknown,
            }]),
        }]),
        turn_free: false,
        first: false,
        released: false,
    }
}
fn decide(landing: &Landing, facts: &Facts) -> Stepped {
    step(landing, facts, &PROJECT, &LIMITS, at(10))
}
fn preparing() -> Landing {
    Landing { phase: Phase::Preparing, ..landing_of_one() }
}
fn proving(proof: Proof) -> Landing {
    let mut landing = decide(&preparing(), &facts()).landing;
    landing.members[0].gates[0].proof = proof;
    landing
}
fn keyed() -> Landing {
    proving(Proof::Keyed { key: KEY })
}
fn dispatched() -> Landing {
    let mut landing = proving(Proof::Dispatched { key: KEY, try_: 1 });
    landing.members[0].gates[0].posted = Posted::Pending;
    landing
}
fn turn() -> Landing {
    Landing { phase: Phase::Turn { at: at(4) }, ..proving(Proof::Passed { key: KEY }) }
}

#[test]
fn a_member_not_ready_waits() {
    let mut f = facts();
    f.members[0].ready = false;
    let next = decide(&landing_of_one(), &f);
    assert_eq!(next.decision, Decision::Wait { until: None });
    assert_eq!(next.landing.phase, Phase::Waiting);
}
#[test]
fn a_head_that_contains_main_is_its_candidate() {
    let next = decide(&preparing(), &facts());
    assert_eq!(next.decision, Decision::None);
    assert_eq!(next.landing.phase, Phase::Proving);
    assert_eq!(next.landing.preparation, 1);
    let member = &next.landing.members[0];
    assert_eq!((member.head, member.base, member.candidate), (Some(HEAD), Some(MAIN), Some(HEAD)));
    assert_eq!(member.gates[0].proof, Proof::Unkeyed);
}
#[test]
fn a_head_behind_main_asks_its_change_to_prepare_once() {
    let mut f = facts();
    f.members[0].contains_main = Status::Failed;
    let one = decide(&preparing(), &f);
    assert_eq!(one.decision, Decision::Prepare { member: 0, head: HEAD, main: MAIN });
    assert_eq!(one.landing.members[0].asked, Some(1));
    assert_eq!(one.landing.preparation, 1);
    assert_eq!(decide(&one.landing, &f).decision, Decision::Wait { until: None });
    f.members[0].asked = true;
    assert_eq!(decide(&one.landing, &f).decision, Decision::Wait { until: None });
}
#[test]
fn a_rebased_head_that_contains_main_is_its_candidate() {
    let mut f = facts();
    f.members[0].contains_main = Status::Failed;
    let one = decide(&preparing(), &f);
    f.members[0].head = NEW;
    f.members[0].contains_main = Status::Passed;
    let two = decide(&one.landing, &f);
    assert_eq!(two.landing.members[0].candidate, Some(NEW));
    assert_eq!(two.landing.members[0].base, Some(MAIN));
    assert_eq!(two.landing.members[0].asked, None);
    assert_eq!(two.landing.preparation, 1);
    assert_eq!(two.landing.phase, Phase::Proving);
}
#[test]
fn an_unkeyed_gate_asks_for_its_key() {
    let next = decide(&proving(Proof::Unkeyed), &facts());
    assert_eq!(next.decision, Decision::Key { member: 0, gate: 0, candidate: HEAD });
    assert_eq!(next.landing.members[0].gates[0].proof, Proof::Unkeyed);
}
#[test]
fn a_passed_receipt_holds_with_no_dispatch_and_posts_received() {
    let mut f = facts();
    f.members[0].gates[0].key = Some(KEY);
    f.members[0].gates[0].receipt = Status::Passed;
    let next = decide(&proving(Proof::Unkeyed), &f);
    assert_eq!(next.decision, Decision::Post { member: 0, gate: 0, posting: Posting::Received });
    assert_eq!(next.landing.members[0].gates[0].proof, Proof::Passed { key: KEY });
    assert_eq!(next.landing.members[0].gates[0].posted, Posted::Verdict);
    assert_eq!(decide(&next.landing, &f).decision, Decision::Proved);
}
#[test]
fn a_key_with_no_receipt_is_dispatched_then_posted_pending() {
    let one = decide(&keyed(), &facts());
    assert_eq!(one.decision, Decision::Dispatch { member: 0, gate: 0, key: KEY, try_: 1 });
    assert_eq!(one.landing.members[0].gates[0].proof, Proof::Dispatched { key: KEY, try_: 1 });
    let two = decide(&one.landing, &facts());
    assert_eq!(two.decision, Decision::Post { member: 0, gate: 0, posting: Posting::Pending });
    assert_eq!(two.landing.members[0].gates[0].posted, Posted::Pending);
    assert_eq!(decide(&two.landing, &facts()).decision, Decision::Wait { until: None });
}
#[test]
fn a_key_in_flight_for_another_landing_waits() {
    let mut f = facts();
    f.members[0].gates[0].flight = Flight::Other;
    let next = decide(&keyed(), &f);
    assert_eq!(next.decision, Decision::Wait { until: None });
    assert_eq!(next.landing.members[0].gates[0].proof, Proof::Keyed { key: KEY });
}
#[test]
fn a_runners_fault_retries_up_to_gate_retries_then_holds() {
    let mut f = facts();
    f.members[0].gates[0].flight = Flight::Mine { run: Some(Verdict::Fault(Fault::SetUp)) };
    let one = decide(&dispatched(), &f);
    assert_eq!(one.decision, Decision::Retry { member: 0, gate: 0, key: KEY, try_: 2 });
    assert_eq!(one.landing.members[0].gates[0].faults, 1);
    f.members[0].gates[0].flight = Flight::Mine { run: None };
    assert_eq!(decide(&one.landing, &f).decision, Decision::Wait { until: Some(at(15)) });
    f.members[0].gates[0].flight = Flight::Mine { run: Some(Verdict::Fault(Fault::Completion)) };
    let two = decide(&one.landing, &f);
    assert_eq!(two.decision, Decision::Retry { member: 0, gate: 0, key: KEY, try_: 3 });
    assert_eq!(two.landing.members[0].gates[0].faults, 2);
    f.members[0].gates[0].flight = Flight::Mine { run: Some(Verdict::Fault(Fault::Lost)) };
    let three = decide(&two.landing, &f);
    assert_eq!(three.decision, Decision::Hold(Hold::Faults { member: 0, gate: 0 }));
    assert_eq!(
        three.landing.phase,
        Phase::Held { was: Box::new(Phase::Proving), why: Hold::Faults { member: 0, gate: 0 } }
    );
}
#[test]
fn a_failed_receipt_repairs_its_member() {
    let mut f = facts();
    f.members[0].gates[0].receipt = Status::Failed;
    let one = decide(&dispatched(), &f);
    assert_eq!(one.decision, Decision::Repair { member: 0, gate: 0 });
    assert_eq!(one.landing.members[0].gates[0].proof, Proof::Failed { key: KEY });
    let two = decide(&one.landing, &f);
    assert_eq!(two.decision, Decision::Post { member: 0, gate: 0, posting: Posting::Ran { passed: false } });
    assert_eq!(decide(&two.landing, &f).decision, Decision::Wait { until: None });
}

#[test]
fn a_failed_gate_posts_its_verdict_even_while_its_member_repairs() {
    let mut f = facts();
    f.members[0].gates[0].receipt = Status::Failed;
    let repair = decide(&dispatched(), &f);
    assert_eq!(repair.decision, Decision::Repair { member: 0, gate: 0 });
    f.members[0].ready = false;
    f.members[0].gates[0].posting = Status::Pending;
    let waiting = decide(&repair.landing, &f);
    assert_eq!(waiting.landing.phase, Phase::Waiting);
    assert_eq!(waiting.decision, Decision::Wait { until: None });
    f.members[0].gates[0].posting = Status::Passed;
    let posted = decide(&waiting.landing, &f);
    assert_eq!(posted.decision, Decision::Post { member: 0, gate: 0, posting: Posting::Ran { passed: false } });
    assert_eq!(posted.landing.phase, Phase::Waiting);
    assert_eq!(posted.landing.members[0].candidate, Some(HEAD));
    assert_eq!(decide(&posted.landing, &f).decision, Decision::Wait { until: None });
}

#[test]
fn a_main_that_moves_during_proof_is_checked_in_the_turn() {
    let mut f = facts();
    f.members[0].main = NEW;
    let proved = decide(&proving(Proof::Passed { key: KEY }), &f);
    assert_eq!(proved.decision, Decision::Proved);
    assert_eq!(proved.landing.members[0].base, Some(MAIN));
    f.turn_free = true;
    f.first = true;
    let owned = decide(&proved.landing, &f);
    assert_eq!(owned.landing.phase, Phase::Turn { at: at(10) });
    let stale = decide(&owned.landing, &f);
    assert_eq!(stale.decision, Decision::Stale);
    assert_eq!(stale.landing.stale_proofs, 1);
}

#[test]
fn a_repaired_head_does_not_erase_the_old_candidates_unposted_failure() {
    let mut f = facts();
    f.members[0].gates[0].receipt = Status::Failed;
    let repair = decide(&dispatched(), &f);
    f.members[0].head = NEW;
    f.members[0].gates[0].posting = Status::Pending;
    let pending = decide(&repair.landing, &f);
    assert_eq!(pending.decision, Decision::Wait { until: None });
    assert_eq!(pending.landing.members[0].candidate, Some(HEAD));
    f.members[0].gates[0].posting = Status::Passed;
    let posted = decide(&pending.landing, &f);
    assert_eq!(posted.decision, Decision::Post { member: 0, gate: 0, posting: Posting::Ran { passed: false } });
    assert_eq!(posted.landing.members[0].candidate, Some(HEAD));
    assert_eq!(decide(&posted.landing, &f).landing.phase, Phase::Preparing);
}
#[test]
fn a_member_back_to_repairing_takes_the_landing_back_to_waiting() {
    for phase in [Phase::Preparing, Phase::Proving] {
        let mut landing = keyed();
        landing.phase = phase;
        let mut f = facts();
        f.members[0].ready = false;
        let next = decide(&landing, &f);
        assert_eq!(next.landing.phase, Phase::Waiting);
        assert_eq!(next.landing.members, landing.members);
        f.members[0].ready = true;
        f.members[0].head = NEW;
        let ready = decide(&next.landing, &f);
        assert_eq!(ready.landing.phase, Phase::Preparing);
        let prepared = decide(&ready.landing, &f);
        assert_eq!(prepared.landing.members[0].candidate, Some(NEW));
        assert_eq!(prepared.landing.members[0].gates[0].proof, Proof::Unkeyed);
    }
}
#[test]
fn an_advisory_gate_holds_nothing() {
    let mut f = facts();
    f.members[0].gates[0].blocking = false;
    let one = decide(&keyed(), &f);
    assert_eq!(one.decision, Decision::Dispatch { member: 0, gate: 0, key: KEY, try_: 1 });
    let two = decide(&one.landing, &f);
    assert_eq!(two.decision, Decision::Post { member: 0, gate: 0, posting: Posting::Pending });
    f.members[0].gates[0].flight = Flight::Mine { run: Some(Verdict::Running) };
    assert_eq!(decide(&two.landing, &f).decision, Decision::Proved);
    f.members[0].gates[0].receipt = Status::Failed;
    let failed = decide(&dispatched(), &f);
    assert_eq!(failed.decision, Decision::Post { member: 0, gate: 0, posting: Posting::Ran { passed: false } });
    assert_eq!(decide(&failed.landing, &f).decision, Decision::Proved);
    f.members[0].gates[0].receipt = Status::Unknown;
    f.members[0].gates[0].flight = Flight::Mine { run: Some(Verdict::Fault(Fault::Lost)) };
    let mut exhausted = dispatched();
    exhausted.members[0].gates[0].faults = PROJECT.gate_retries;
    assert_eq!(decide(&exhausted, &f).decision, Decision::Proved);
}
#[test]
fn every_blocking_gate_passed_proves_the_landing() {
    let next = decide(&proving(Proof::Passed { key: KEY }), &facts());
    assert_eq!(next.decision, Decision::Proved);
    assert_eq!(next.landing.phase, Phase::Proved { at: at(10) });
}
#[test]
fn a_proved_landing_takes_a_free_turn_only_when_first() {
    let landing = Landing { phase: Phase::Proved { at: at(4) }, ..keyed() };
    for (free, first) in [(false, false), (false, true), (true, false), (true, true)] {
        let mut f = facts();
        f.turn_free = free;
        f.first = first;
        let next = decide(&landing, &f);
        if free && first {
            assert_eq!(next.landing.phase, Phase::Turn { at: at(4) });
            assert_eq!(next.decision, Decision::None);
        } else {
            assert_eq!(next.landing.phase, landing.phase);
            assert_eq!(next.decision, Decision::Wait { until: None });
        }
    }
}
#[test]
fn the_turn_merges_each_member_at_its_candidate_once() {
    let mut landing = turn();
    landing.members = Box::new([landing.members[0].clone(), landing.members[0].clone()]);
    let mut f = facts();
    f.members = Box::new([f.members[0].clone(), f.members[0].clone()]);
    let one = decide(&landing, &f);
    assert_eq!(one.decision, Decision::Merge { member: 0, candidate: HEAD });
    f.members[0].merge = Status::Pending;
    assert_eq!(decide(&one.landing, &f).decision, Decision::Wait { until: None });
    f.members[0].merge = Status::Passed;
    f.members[0].merged = Some(HEAD);
    f.members[0].main = HEAD;
    let two = decide(&one.landing, &f);
    assert_eq!(two.landing.members[0].landed, Some(HEAD));
    assert_eq!(two.decision, Decision::Merge { member: 1, candidate: HEAD });
    f.members[1].merge = Status::Pending;
    assert_eq!(decide(&two.landing, &f).decision, Decision::Wait { until: None });
}
#[test]
fn a_main_moved_before_the_turns_merge_is_stale() {
    let mut landing = turn();
    landing.members = Box::new([landing.members[0].clone(), landing.members[0].clone()]);
    let mut f = facts();
    f.members = Box::new([f.members[0].clone(), f.members[0].clone()]);
    f.members[0].main = NEW;
    let next = decide(&landing, &f);
    assert_eq!(next.decision, Decision::Stale);
    assert_eq!(next.landing.phase, Phase::Preparing);
    assert_eq!(next.landing.stale_proofs, 1);
    assert_eq!(next.landing.members[0].candidate, None);
    assert_eq!(next.landing.members[0].gates[0].proof, Proof::Unkeyed);
    assert_eq!(next.landing.members[1], landing.members[1]);
}
#[test]
fn a_merged_member_lands_the_landing() {
    let mut f = facts();
    f.members[0].merged = Some(HEAD);
    f.members[0].main = HEAD;
    let next = decide(&turn(), &f);
    assert_eq!(next.decision, Decision::Finish);
    assert_eq!(next.landing.phase, Phase::Landed);
    assert_eq!(next.landing.members[0].landed, Some(HEAD));
    assert_eq!(decide(&next.landing, &f).decision, Decision::None);
}
#[test]
fn a_cancelled_member_cancels_a_landing_of_one() {
    let mut f = facts();
    f.members[0].cancelled = true;
    let next = decide(&landing_of_one(), &f);
    assert_eq!(next.decision, Decision::Cancel);
    assert_eq!(next.landing.phase, Phase::Cancelled);
    assert!(next.landing.members.is_empty(), "cancelled member left");
}
#[test]
fn a_held_landing_waits_until_released() {
    let mut landing = dispatched();
    landing.members[0].gates[0].faults = 2;
    landing.phase = Phase::Held { was: Box::new(Phase::Proving), why: Hold::Faults { member: 0, gate: 0 } };
    assert_eq!(decide(&landing, &facts()).decision, Decision::Wait { until: None });
    let mut f = facts();
    f.released = true;
    let next = decide(&landing, &f);
    assert_eq!(next.decision, Decision::None);
    assert_eq!(next.landing.phase, Phase::Preparing);
    assert_eq!(next.landing.members[0].gates[0].faults, 0);
}
#[test]
fn current_runs_poll_and_wait_for_a_receipt_before_proving() {
    for run in [None, Some(Verdict::Running), Some(Verdict::Passed), Some(Verdict::Failed)] {
        let mut f = facts();
        f.members[0].gates[0].flight = Flight::Mine { run };
        let next = decide(&dispatched(), &f);
        assert_eq!(next.decision, Decision::Wait { until: Some(at(15)) });
        assert_eq!(next.landing.phase, Phase::Proving);
    }
}
#[test]
fn a_passed_run_posts_ran_after_the_pending_post_completes() {
    let mut f = facts();
    f.members[0].gates[0].receipt = Status::Passed;
    f.members[0].gates[0].posting = Status::Pending;
    let one = decide(&dispatched(), &f);
    assert_eq!(one.decision, Decision::Wait { until: None });
    f.members[0].gates[0].posting = Status::Passed;
    let two = decide(&one.landing, &f);
    assert_eq!(two.decision, Decision::Post { member: 0, gate: 0, posting: Posting::Ran { passed: true } });
    assert_eq!(decide(&two.landing, &f).decision, Decision::Proved);
}
#[test]
fn a_cancelled_member_leaves_live_members_and_their_proofs() {
    let mut landing = keyed();
    landing.members = Box::new([landing.members[0].clone(), landing.members[0].clone()]);
    let mut f = facts();
    f.members = Box::new([f.members[0].clone(), f.members[0].clone()]);
    f.members[0].cancelled = true;
    let next = decide(&landing, &f);
    assert_eq!(next.decision, Decision::None);
    assert_eq!(next.landing.phase, Phase::Preparing);
    assert_eq!(&*next.landing.members, &[landing.members[1].clone()]);
}
#[test]
fn a_refused_merge_makes_its_candidate_stale() {
    let mut f = facts();
    f.members[0].merge = Status::Failed;
    let next = decide(&turn(), &f);
    assert_eq!(next.decision, Decision::Stale);
    assert_eq!(next.landing.members[0].candidate, None);
    assert_eq!(next.landing.stale_proofs, 1);
}
#[test]
fn an_unchanged_candidate_keeps_its_proofs_and_preparation() {
    let mut landing = keyed();
    landing.phase = Phase::Preparing;
    let next = decide(&landing, &facts());
    assert_eq!(next.landing.members, landing.members);
    assert_eq!(next.landing.preparation, landing.preparation);
    assert_eq!(next.landing.phase, Phase::Proving);
}
#[test]
fn the_same_snapshot_always_gives_the_same_decision() {
    for landing in [landing_of_one(), preparing(), keyed(), dispatched(), turn()] {
        assert_eq!(decide(&landing, &facts()), decide(&landing, &facts()));
    }
}
