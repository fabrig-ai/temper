//! Reference case 6.7: deadline admission, escalation, split and release.
use temper_real_loop_world::{
    Exit,
    case::{self, oracle::Observed},
    run_cluster_forge,
};

#[test]
fn a_sub_owner_past_its_deadline_is_held_at_its_next_run_and_escalated_through_its_owner_to_the_director_who_splits_it_by_amendment_and_release_without_a_grace_for_its_delegates()
 {
    // Jig's rail wakes parked A1 just past its wall and admission holds it.
    // A running a5 finishes its one attempt: a grace for later admissions
    // is 6.8's conflict. A held A1 writes no escalation words; the split is
    // instructions amended by the director, not a new sub-owner (6.8).
    // Four hours stand for CAP_SECONDS from the decision making A1.
    // A gives A1 that span; a5 inherits A1's wall. A1 subscribes no timer.
    // A's own deadline and the director's amendment wall stay later.
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
    case::oracle::assert_met(&case::oracle::step_6_7(&observed));
}
