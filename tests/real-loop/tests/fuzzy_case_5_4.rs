//! Retained first-edition story (domain/testing.md, 5.3): new direction and overlap news wakes B once released.
use temper_real_loop_world::{
    Exit,
    case::{
        self,
        oracle::{self, Observed},
    },
    run_cluster_forge,
};

#[test]
fn a_new_direction_cancels_x3_under_its_closed_pull_and_bs_landing_news_waits_for_its_release_with_an_overlap_wake_and_a_closing_comment()
 {
    // B's planned change makes a1's overlapping landing wake it, while its
    // hold still defers the next run until Release, with the news told once.
    // A follows delivered input: its current-head CI wakes wait for the
    // amendment, and its extra parked run is included in the full journal.
    // Leaves out the kept branch: domain/forge.md 8.6 deletes it; its closed
    // pull keeps the commits, with A's reason posted once before it closes.
    // Leaves out x3 past its cap: a running
    // task's deadline never fires (jig's crates/jig-domain-tasks/src/domain.rs:698
    // and crates/jig-domain-core/src/run.rs:121; case_5_3.rs).
    // Leaves out enforced grants (draft/temper-on-temper.md 6.7): A's grant is
    // its instructions' convention; the footprint detects overlap at landing.
    // Leaves out the hunks (news names commits), B's rebase (its planned h1
    // is cancelled to go again from new main), and the twenty minutes.
    // Three one-slot workers: x3's review and h1's coder can remain held
    // together, with one slot for A/a1 or B. Waiting owner runs park.
    let ran = run_cluster_forge(
        case::PAUSE_WORKERS,
        &[],
        case::fixture_5_4(),
        case::scripts::step_5_4,
        case::scripts::step_5_4_rules,
        case::acts::step_5_4,
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
    let observed = Observed::from_ran(&ran, case::acts::READS_5_4);
    oracle::assert_met(&oracle::step_5_4(&observed));
}
