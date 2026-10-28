#[global_allocator]
static HEAP: skein_world::domain::heap::Counting = skein_world::domain::heap::Counting;

use jig_conformance::{Application, Harness};
use temper_conformance_world::{Config, Temper};
#[test]
fn temper_starts_adopts_and_assigns_through_the_shared_referee() {
    let mut world = Harness::<Temper>::new(Config::new(71, None), 71);
    let result = world.drain();
    assert!(
        result.is_ok(),
        "{result:?} ready={} quiet={} trace={:?}",
        Temper::ready(&world.domain),
        Temper::quiescent(&world.domain),
        &world.trace[world.trace.len().saturating_sub(12)..]
    );
    assert!(world.peers.adopted);
    assert_eq!(world.peers.assignments.len(), 1);
}
#[test]
fn an_idempotent_forge_set_tolerates_its_late_copy() {
    jig_conformance::scenarios::run::<Temper>(
        jig_conformance::scenarios::Scenario::LateCopy,
        jig_conformance::Cut::None,
        71,
    )
    .expect("late copy")
    .expect("applicable");
}
#[test]
fn an_uncertain_status_set_keeps_its_deadline_across_restarts() {
    jig_conformance::scenarios::run::<Temper>(
        jig_conformance::scenarios::Scenario::UncertainRestarts,
        jig_conformance::Cut::None,
        71,
    )
    .expect("uncertain restart")
    .expect("applicable");
}
#[test]
fn a_standing_task_runs_once_per_funding_period() {
    jig_conformance::scenarios::run::<Temper>(
        jig_conformance::scenarios::Scenario::StandingTask,
        jig_conformance::Cut::None,
        71,
    )
    .expect("standing task")
    .expect("applicable");
}
#[test]
fn many_completions_use_one_run_allowance() {
    jig_conformance::scenarios::run::<Temper>(
        jig_conformance::scenarios::Scenario::RunBudget,
        jig_conformance::Cut::None,
        71,
    )
    .expect("run budget")
    .expect("applicable");
}

#[test]
fn remaining_shared_scenarios_use_real_forge_and_people_operations() {
    for scenario in [
        jig_conformance::scenarios::Scenario::Compound,
        jig_conformance::scenarios::Scenario::ConversationEndsWhileRead,
        jig_conformance::scenarios::Scenario::QueryWatchedWhileTasksEnd,
        jig_conformance::scenarios::Scenario::OtherHand,
        jig_conformance::scenarios::Scenario::ChangedJudge,
        jig_conformance::scenarios::Scenario::NarrowingPolicy,
        jig_conformance::scenarios::Scenario::Grace,
    ] {
        jig_conformance::scenarios::run::<Temper>(scenario, jig_conformance::Cut::None, 71)
            .unwrap_or_else(|error| panic!("{scenario:?}: {error:?}"))
            .expect("applicable");
    }
}

/// Crash one shared scenario at its configured entry, middle and closing
/// receipt cuts; its baseline runs in the focused cases above and in the
/// fuzzy sweep. Fixture startup runs the real first start, then preseeds restored keys;
/// offsets below count live receipts after that durable setup. The empty-store
/// cut at receipt one is covered by the engine world's first-start recovery story.
fn crashed_at(scenario: jig_conformance::scenarios::Scenario, cuts: [u64; 3]) {
    let setup = <Temper as Application>::store(&Config::new(91, Some(scenario))).applied;
    for cut in cuts {
        let cut = cut + setup - 1;
        let outcome = jig_conformance::scenarios::run::<Temper>(scenario, jig_conformance::Cut::AfterCommit(cut), 91)
            .unwrap_or_else(|error| panic!("{scenario:?} at {cut}: {error:?}"))
            .expect("applicable");
        assert!(outcome.crashes.contains(&cut), "{scenario:?} cut {cut} did not fire");
    }
}

#[test]
fn a_late_copy_keeps_its_promises_at_focused_crash_cuts() {
    crashed_at(jig_conformance::scenarios::Scenario::LateCopy, [2, 6, 12]);
}

#[test]
fn another_hand_keeps_its_promises_at_focused_crash_cuts() {
    crashed_at(jig_conformance::scenarios::Scenario::OtherHand, [2, 15, 30]);
}

#[test]
fn uncertain_restarts_keep_their_promises_at_focused_crash_cuts() {
    crashed_at(jig_conformance::scenarios::Scenario::UncertainRestarts, [2, 7, 15]);
}

#[test]
fn a_changed_judge_keeps_its_promises_at_focused_crash_cuts() {
    crashed_at(jig_conformance::scenarios::Scenario::ChangedJudge, [2, 22, 45]);
}

#[test]
fn a_narrowing_policy_keeps_its_promises_at_focused_crash_cuts() {
    crashed_at(jig_conformance::scenarios::Scenario::NarrowingPolicy, [2, 12, 24]);
}

#[test]
fn a_standing_task_keeps_its_promises_at_focused_crash_cuts() {
    crashed_at(jig_conformance::scenarios::Scenario::StandingTask, [2, 12, 24]);
}

#[test]
fn a_run_budget_keeps_its_promises_at_focused_crash_cuts() {
    crashed_at(jig_conformance::scenarios::Scenario::RunBudget, [2, 10, 21]);
}

#[test]
fn a_run_amendment_sees_the_partys_committed_spec_revision() {
    jig_conformance::scenarios::run::<Temper>(
        jig_conformance::scenarios::Scenario::RacingDecisions,
        jig_conformance::Cut::None,
        71,
    )
    .expect("revision race")
    .expect("applicable");
}
