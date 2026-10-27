//! Retained first-edition story (domain/testing.md, 5.3): two landings, with a fresh waiting sibling dependency.
use temper_real_loop_world::{Exit, case, run_cluster_forge};

#[test]
fn a_kit_change_releases_the_apps_waiting_repin_without_relay_without_the_a3_a4_epilogue() {
    // Start with the person's permission. Omit the preceding question/hour
    // (6.5's director-origin gap), A between director and A1, the successor
    // notice (7.1), a3/a4's later landings, and 6.4's red-tip/interim narrative.
    // Kit's final tree is not observed by the shared single-main carrier;
    // exact head, CI, review, merge and result are checked from records.
    // Repin is initially Waiting, never active-kept: it has no coding,
    // attempt, delegate or app pull before e1 ends. No Met is expected.
    let ran = run_cluster_forge(
        case::PAUSE_WORKERS,
        &[],
        case::fixture_6_6(),
        case::scripts::step_6_6,
        case::scripts::step_6_6_rules,
        case::acts::step_6_6,
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
    let observed = case::oracle::Observed::from_ran(&ran, case::acts::READS_6_6).with_commands(&case::acts::step_6_6());
    case::oracle::assert_met(&case::oracle::step_6_6(&observed));
}
