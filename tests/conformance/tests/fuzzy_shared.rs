#[global_allocator]
static HEAP: skein_world::domain::heap::Counting = skein_world::domain::heap::Counting;

use jig_conformance::{
    Application, Cut,
    scenarios::{self, Scenario},
};
use temper_conformance_world::{Config, Temper};

/// Crash one shared scenario at every live durable commit after the two fixture receipts.
fn crashed_at_every_live_commit(scenario: Scenario) {
    let baseline = scenarios::run::<Temper>(scenario, Cut::None, 101).expect("baseline").expect("applicable");
    eprintln!("{scenario:?}: {} durable commit cuts", baseline.commits);
    // Setup's real first-start cut is exercised by the engine-domain world.
    let first_live = Temper::store(&Config::new(101, Some(scenario))).applied + 1;
    for cut in first_live..=baseline.commits {
        let outcome = scenarios::run::<Temper>(scenario, Cut::AfterCommit(cut), 101)
            .unwrap_or_else(|error| panic!("{scenario:?}, cut {cut}: {error:?}"))
            .expect("applicable");
        assert!(outcome.crashes.contains(&cut), "{scenario:?} cut {cut} did not fire");
    }
}

#[test]
fn a_late_copy_is_crashed_at_every_live_commit() {
    crashed_at_every_live_commit(Scenario::LateCopy);
}

#[test]
fn another_hand_is_crashed_at_every_live_commit() {
    crashed_at_every_live_commit(Scenario::OtherHand);
}

#[test]
fn uncertain_restarts_are_crashed_at_every_live_commit() {
    crashed_at_every_live_commit(Scenario::UncertainRestarts);
}

#[test]
fn a_changed_judge_is_crashed_at_every_live_commit() {
    crashed_at_every_live_commit(Scenario::ChangedJudge);
}

#[test]
fn a_narrowing_policy_is_crashed_at_every_live_commit() {
    crashed_at_every_live_commit(Scenario::NarrowingPolicy);
}

#[test]
fn a_standing_task_is_crashed_at_every_live_commit() {
    crashed_at_every_live_commit(Scenario::StandingTask);
}

#[test]
fn a_run_budget_is_crashed_at_every_live_commit() {
    crashed_at_every_live_commit(Scenario::RunBudget);
}

#[test]
fn a_shrinking_pool_is_not_applicable() {
    let outcome = scenarios::run::<Temper>(Scenario::ShrinkingPool, Cut::None, 101).expect("not applicable");
    assert!(outcome.is_none(), "temper's forge has no pooled holds");
}
