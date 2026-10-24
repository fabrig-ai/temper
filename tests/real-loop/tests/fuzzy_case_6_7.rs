//! Reference case 6.7: deadline admission, escalation, split and release.
use temper_real_loop_world::{
    Exit,
    case::{self, oracle::Observed},
    run_cluster_forge,
};

#[test]
fn a_sub_owner_past_its_deadline_is_held_at_its_next_run_and_escalated_through_its_owner_to_the_director_who_splits_it_by_amendment_and_release_without_a_grace_for_its_delegates()
 {
    // A1 parks before the cap; jig's B1 does not hold running tasks. a5
    // finishes its one running attempt: a grace for later admissions is
    // 6.8's conflict. A held A1 writes no escalation words; the split is
    // instructions amended by the director, not a new sub-owner (6.8).
    // Four hours stand for CAP_SPAN from script construction. F11:
    // delegate accepts only an absolute wall (temper-engine-smith/src/nested.rs,
    // 206-209), while jig at 1d93fa987 has Deadline::Span (jig-domain-tasks/src/boundary.rs:857).
    // The timer is deadline + 1 ms: jig's strict wall > deadline (1d93fa987,
    // jig-domain-authority/src/check.rs:525). A's and a5's delegations
    // carry the same constructed deadline; A's own deadline stays later.
    // Singleton batches and input cue rules tolerate parked extra runs.
    // A's pass and the director's amendment are held for the checkpoint
    // reads; amendment then release are separate calls.
    // Earlier sections stay unchanged; batch_member, batch_held and the
    // shared tool, delivery and attempt checks are reused by calls.
    let ran = run_cluster_forge(
        case::PAUSE_WORKERS,
        &[],
        case::fixture_6_7(),
        case::scripts::step_6_7,
        case::scripts::step_6_7_rules,
        case::acts::step_6_7,
    );
    for (at, called) in ran.calls().iter().enumerate() {
        assert_eq!(
            called.exit,
            Some(Exit::Code(0)),
            "call {at}: {}\n{}",
            String::from_utf8_lossy(&called.stderr),
            String::from_utf8_lossy(&called.stdout)
        );
    }
    let observed = Observed::from_ran(&ran, case::acts::READS_6_7);
    eprintln!("CAP opening {:?} ns", case::oracle::cap_opening_nanos(&observed));
    case::oracle::assert_met(&case::oracle::step_6_7(&observed));
}
