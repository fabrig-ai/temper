//! Retained first-edition story (domain/testing.md, 5.3): an informative direction, cancelled helper and persistent timer.
//! Fuzzy, though within the 2 s guide (testing.md 6.1): at about 1.56 s it would
//! take the real loop's default suite past its 13 s ceiling.
use temper_real_loop_world::{
    Exit,
    case::{
        self,
        oracle::{self, Observed},
    },
    run_cluster_forge,
};

#[test]
fn a_misread_helper_is_cancelled_and_the_rule_noted_while_the_directors_timer_leaves_its_fresh_run_free_without_preferred_hosts()
 {
    // Leaves out preferred host kinds: delegate has no host field
    // (crates/temper-engine-smith/src/calls.rs:43); neither does jig's Spec
    // (crates/jig-domain-tasks/src/boundary.rs:393), and its host kinds are
    // Worker and Engine (crates/jig-domain-fleet/src/boundary.rs:57).
    // s1 and s2 preferred for priority 1 remain the person's words.
    // Leaves out a helper's change in flight, covered by 5.4 and 5.2's h1:
    // this helper's first LLM turn is held before any work can land.
    // The timer's wake is 4.3's; here it stays far ahead. The two failed
    // fresh starts stay out: subscriptions belong to the task, outside its
    // run (jig's crates/jig-domain-tasks/src/subscriptions.rs:1-4, 76-84).
    let ran = run_cluster_forge(
        &["ws", "s1"],
        &[],
        case::fixture_5_6(),
        case::scripts::step_5_6,
        case::scripts::step_5_6_rules,
        case::acts::step_5_6,
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
    let observed = Observed::from_ran(&ran, case::acts::READS_5_6);
    oracle::assert_met(&oracle::step_5_6(&observed));
}
