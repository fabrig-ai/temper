//! Retained first-edition story (domain/testing.md, 5.3): an answered plan, fresh owner and depth-zero sub-owner.
use temper_real_loop_world::{
    Exit,
    case::{
        self,
        oracle::{self, Observed},
    },
    run_cluster_forge,
};

#[test]
fn an_owners_answered_plan_starts_a_sub_owner_under_a_deadline_that_cannot_delegate_without_the_deadline_firing_or_a_fresh_run_only_at_the_milestones_end()
 {
    // Leaves out the deadline's effect: jig checks it on admission, never on
    // a running task (jig's crates/jig-domain-tasks/src/domain.rs:698 and
    // crates/jig-domain-core/src/run.rs:121). Both deadlines stay far ahead.
    // Milestone counts and lengths are plan words, never judged. Leaves out
    // a fresh run only at a milestone's end: charter resume policy applies
    // to every next run, with no fresh-next-run call (temper's
    // crates/temper-engine-smith/src/calls.rs:42-59 and jig's
    // crates/jig-domain-core/src/routing/run_preparation.rs:123-138). A1's milestone work,
    // the four hours and the sixteen stay out (domain/testing.md, 5.3).
    let ran = run_cluster_forge(
        &["ws", "s1"],
        &[],
        case::fixture_5_5(),
        case::scripts::step_5_5,
        case::scripts::step_5_5_rules,
        case::acts::step_5_5,
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
    let observed = Observed::from_ran(&ran, case::acts::READS_5_5);
    oracle::assert_met(&oracle::step_5_5(&observed));
}
