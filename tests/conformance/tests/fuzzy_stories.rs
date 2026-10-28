#[global_allocator]
static HEAP: skein_world::domain::heap::Counting = skein_world::domain::heap::Counting;

use jig_conformance::{Application, Cut};
use temper_conformance_world::stories::{self, Story};
use temper_conformance_world::{Config, Temper};

/// Crash one native story at every live durable commit after the two fixture receipts.
fn crashed_at_every_live_commit(story: Story) {
    let baseline = stories::run(story, Cut::None, 101).expect("baseline");
    eprintln!("{story:?}: {} durable commit cuts, {} baseline iterations", baseline.commits, baseline.iterations);
    // Setup's real first-start cut is exercised by the engine-domain world.
    let first_live = Temper::store(&Config::for_story(101, story)).applied + 1;
    for cut in first_live..=baseline.commits {
        let outcome = stories::run(story, Cut::AfterCommit(cut), 101)
            .unwrap_or_else(|error| panic!("{story:?}, cut {cut}: {error:?}"));
        assert!(outcome.crashes.contains(&cut), "{story:?} cut {cut} did not fire");
    }
}

#[test]
fn a_reviewed_change_is_crashed_at_every_live_commit() {
    crashed_at_every_live_commit(Story::ReviewedChange);
}

#[test]
fn a_ci_repair_is_crashed_at_every_live_commit() {
    crashed_at_every_live_commit(Story::RepairCi);
}

#[test]
fn a_goal_issue_is_crashed_at_every_live_commit() {
    crashed_at_every_live_commit(Story::GoalIssue);
}

#[test]
fn another_hand_is_crashed_at_every_live_commit() {
    crashed_at_every_live_commit(Story::OtherHand);
}

#[test]
fn a_lost_merge_is_crashed_at_every_live_commit() {
    crashed_at_every_live_commit(Story::LostMerge);
}

#[test]
fn person_words_are_crashed_at_every_live_commit() {
    crashed_at_every_live_commit(Story::PersonWords);
}

#[test]
fn an_issue_creation_is_crashed_at_every_live_commit() {
    crashed_at_every_live_commit(Story::IssueCreation);
}
