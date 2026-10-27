//! Retained first-edition story (domain/testing.md, 5.3): a sub-owner's batch and two dependencies.
use temper_real_loop_world::{
    Exit,
    case::{
        self,
        oracle::{self, Observed},
    },
    run_cluster_forge,
};

#[test]
fn a_sub_owners_batch_runs_three_delegates_at_once_and_a_fourth_after_its_two_dependencies_without_a_pool_on_lent_slots()
 {
    // Leaves out the surveys; the Markdown design's landing and fast path
    // are held by fuzzy_case_4_1_4_3. Three one-slot workers stand for the
    // pool on lent slots, a design gap. The four reports leave out a1-a4's
    // landings (6.3/6.4). Departures and amendments are words, never judged;
    // A1's deadline is carried, with its effect left out as in 5.5
    // (domain/testing.md, 5.3).
    // Plain delegates expose writing through their task's observation, not
    // the chat-only run document. The four fake holds keep those attempts
    // open across the three reads. The reads before and after a1 ends prove
    // a4 has not begun; both successful endings precede its writing read,
    // and a3 still writes there. Ordinary waiting dependencies emit no Met
    // notice, so no positions from different tasks or wall times are compared.
    // Only zero-spend placement rechecks with no committed turn and a later
    // finished attempt are allowed, within each complete 32-row journal.
    // a3's ended read precedes a4's release, making a4's result A1's last cue.
    let ran = run_cluster_forge(
        case::PAUSE_WORKERS,
        &[],
        case::fixture_6_1(),
        case::scripts::step_6_1,
        case::scripts::step_6_1_rules,
        case::acts::step_6_1,
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
    let observed = Observed::from_ran(&ran, case::acts::READS_6_1);
    oracle::assert_met(&oracle::step_6_1(&observed));
}
