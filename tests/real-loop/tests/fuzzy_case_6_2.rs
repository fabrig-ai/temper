//! Retained first-edition story (domain/testing.md, 5.3): two levels of questions and a live landing dependency.
use temper_real_loop_world::{
    Exit,
    case::{self, oracle::Observed},
    run_cluster_forge,
};

#[test]
fn a_blocker_goes_up_two_levels_as_questions_and_its_fix_lands_before_the_live_task_given_it_as_a_dependency_goes_on_without_apps_repin()
 {
    // App's repin rides on 6.6's second repository; a3 waits on e1 alone.
    // The six signatures are e1's words. B's stop is the question's reason;
    // stopped-owner news is 5.2's. The ancestor's gate is A1's convention:
    // delegate e1 only after the answer. Dependency circles, reach and
    // refusals belong to jig's tasks world and qa_added_deps.
    // A has depth 4 and A1 depth 3 (departing from act 3's depth one),
    // allowing e1 depth 2 for its coder/reviewer; a3 keeps depth 0.
    // Singleton batches need standard for five store joins, beyond small's
    // fixed four. Only a3 keeps its wait on charter 2 (60000 ms);
    // the owners park on charter 1 (1 ms). A alone is tracked; A1 inherits
    // its goal. Owners and e1 carry six forge grants, as 5.4 does.
    // Input cue rules tolerate extra news.
    // The held review keeps e1 pending across a3's dependency-wait and
    // kept activity reads; the first orders the task snapshot before the
    // saved-journal check. Release follows the reads, ordering the Met wake
    // without wall time.
    // A wait on the coder's ending avoids polling e1 throughout coding;
    // the held review still has its own exact writing read.
    let ran = run_cluster_forge(
        case::PAUSE_WORKERS,
        &[],
        case::fixture_6_2(),
        case::scripts::step_6_2,
        case::scripts::step_6_2_rules,
        case::acts::step_6_2,
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
    let observed = Observed::from_ran(&ran, case::acts::READS_6_2);
    case::oracle::assert_met(&case::oracle::step_6_2(&observed));
}
