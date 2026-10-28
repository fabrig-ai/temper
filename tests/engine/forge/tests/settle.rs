use skein_fake_forge_domain::api::Check;
use skein_forge::api as client;
use skein_lib::{Duration, Wall};
use temper_engine_domain_forge as top;
use temper_engine_domain_forge_change as change;
use temper_engine_forge_world::changes::{change_row, change_step};
use temper_engine_forge_world::{REPO, World, translate};

/// First in its queue, a newly pushed head on a pull being checked now.
fn first_on_a_checked_pull(mut world: World, span: Duration) -> World {
    world.adopt();
    world.slow_calls(Duration::ZERO);
    world.produce(b"temper/51");
    world.status(b"temper/51", Check::Passed);
    let pull = world.open_pull(b"temper/51", b"main");
    world.check_pull(pull, span);
    let head = translate::commit(world.branch(b"temper/51"));
    let mut row = change_row();
    row.pull = Some(pull);
    row.change.since = world.wall();
    row.change.state = change::State::First { head, ready_since: world.wall() };
    row.change.last_head = Some(head);
    row.change.owns_turn = true;
    world.event(top::Event::Change { row });
    world
}

fn wait(world: &mut World, entry: u64) -> Wall {
    let decision = change_step(world, entry, change::Status::Passed, Box::new([]));
    let change::Decision::Wait { until: Some(until) } = decision else { panic!("{decision:?}, not a wait") };
    until
}

fn seconds(after: Wall, before: Wall) -> u64 {
    (after.as_nanos() - before.as_nanos()) / 1_000_000_000
}

#[test]
fn a_pull_forgejo_is_checking_waits_by_doubling_reads_then_merges() {
    let mut world = first_on_a_checked_pull(World::new(5), Duration::from_secs(5));
    let mut asked = Vec::new();
    for entry in 1..=4 {
        let at = world.wall();
        let until = wait(&mut world, entry);
        asked.push(seconds(until, at));
        world.run_to(until);
    }
    assert_eq!(asked, [1, 1, 2, 4]);
    let merged = change_step(&mut world, 5, change::Status::Passed, Box::new([]));
    assert!(matches!(merged, change::Decision::Effect(change::Effect::Merge { .. })), "{merged:?}");
    assert_eq!(world.writes(), 1, "never resolved");
}

#[test]
fn a_pull_checking_past_poll_is_read_every_poll_then_merges() {
    let mut world = first_on_a_checked_pull(World::new(5), Duration::from_secs(11));
    let t0 = world.wall();
    let calls = world.calls();
    let mut asked = Vec::new();
    for entry in 1..=5 {
        let at = world.wall();
        let until = wait(&mut world, entry);
        asked.push(seconds(until, at));
        world.run_to(until);
    }
    assert_eq!(asked, [1, 1, 2, 4, 5]);
    assert_eq!(world.calls() - calls, 35, "five steps of seven fresh reads");
    assert_eq!(seconds(world.wall(), t0), 13);
    let merged = change_step(&mut world, 6, change::Status::Passed, Box::new([]));
    assert!(matches!(merged, change::Decision::Effect(change::Effect::Merge { .. })), "{merged:?}");
    assert_eq!(world.calls() - calls, 46, "six steps of seven fresh reads and four calls to land the merge");
    assert_eq!(world.writes(), 1, "never resolved");
}

#[test]
fn a_pull_not_mergeable_past_settle_is_taken_as_conflicted() {
    let mut world = first_on_a_checked_pull(World::new(5), Duration::from_secs(40));
    let t0 = world.wall();
    let mut asked = Vec::new();
    for entry in 1..=9 {
        let at = world.wall();
        let until = wait(&mut world, entry);
        asked.push(seconds(until, at));
        world.run_to(until);
    }
    assert_eq!(asked, [1, 1, 2, 4, 5, 5, 5, 5, 2]);
    assert_eq!(seconds(world.wall(), t0), 30);
    assert_eq!(
        change_step(&mut world, 10, change::Status::Passed, Box::new([])),
        change::Decision::Delegate(change::Delegate::Resolve { base: translate::commit(world.branch(b"main")) })
    );
}

#[test]
fn a_hint_naming_the_base_opens_the_window_again() {
    let mut world = first_on_a_checked_pull(World::new(5), Duration::from_secs(40));
    let mut asked = Vec::new();
    for entry in 1..=3 {
        let at = world.wall();
        let until = wait(&mut world, entry);
        asked.push(seconds(until, at));
        if entry < 3 {
            world.run_to(until);
        }
    }
    world.event(top::Event::Hint {
        hint: client::Hint { repository: REPO, change: client::Change::Branch(Box::from(&b"main"[..])), key: None },
    });
    let at = world.wall();
    let until = wait(&mut world, 4);
    asked.push(seconds(until, at));
    assert_eq!(asked, [1, 1, 2, 1]);
}

#[test]
fn a_job_end_hint_makes_pending_ci_wait_a_hinted_read() {
    let mut world = World::new(5);
    world.adopt();
    world.slow_calls(Duration::ZERO);
    world.produce(b"temper/51");
    let pull = world.open_pull(b"temper/51", b"main");
    let head = translate::commit(world.branch(b"temper/51"));
    let mut row = change_row();
    row.pull = Some(pull);
    row.change.since = world.wall();
    row.change.state = change::State::Checking { head };
    row.change.last_head = Some(head);
    world.event(top::Event::Change { row });
    world.status(b"temper/51", Check::Pending);
    world.event(top::Event::Hint {
        hint: client::Hint { repository: REPO, change: client::Change::Commit(head), key: None },
    });
    let t0 = world.wall();
    let until = wait(&mut world, 1);
    assert_eq!(seconds(until, t0), 1);
    world.status(b"temper/51", Check::Passed);
    world.run_to(until);
    assert_eq!(change_step(&mut world, 2, change::Status::Passed, Box::new([])), change::Decision::Ready);
}

#[test]
fn a_head_pushed_long_before_leaves_no_window_open() {
    let mut world = first_on_a_checked_pull(World::new(5), Duration::from_secs(120));
    world.run_for(60);
    assert_eq!(
        change_step(&mut world, 1, change::Status::Passed, Box::new([])),
        change::Decision::Delegate(change::Delegate::Resolve { base: translate::commit(world.branch(b"main")) })
    );
}

#[test]
fn a_restart_keeps_the_window_of_the_changes_own_push() {
    let mut world = first_on_a_checked_pull(World::new(5), Duration::from_secs(40));
    let t0 = world.wall();
    for entry in 1..=2 {
        let until = wait(&mut world, entry);
        world.run_to(until);
    }
    world.restart();
    let at = world.wall();
    let until = wait(&mut world, 3);
    assert_eq!(seconds(until, at), seconds(at, t0).max(1));
    assert_eq!(seconds(until, at), 2);
}
