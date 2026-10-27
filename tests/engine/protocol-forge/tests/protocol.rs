use skein_fake_forge_domain::{Observation, api::Kind};
use skein_fake_forge_protocol::Route;
use skein_forge_machine_world::joint::REPO;
use skein_lib::Wall;
use temper_engine_domain_forge as top;
use temper_engine_domain_forge_change as change;
use temper_engine_domain_forge_issues as issues;
use temper_engine_protocol_forge_world::{fixtures, protocol::Protocol};
fn change_row() -> top::ChangeRow {
    top::ChangeRow {
        task: 51,
        repository: REPO,
        branch: Box::from(&b"temper/51"[..]),
        base: Box::from(&b"main"[..]),
        title: Box::from(&b"Small fix"[..]),
        body: Box::from(&b"Fix the file"[..]),
        priority: 1,
        change: change::Change {
            task: 51,
            state: change::State::Producing { requested: false },
            gates: Box::new([]),
            clean: Box::new([]),
            gate_failures: Box::new([]),
            repairs: 0,
            resolutions: 0,
            updates: 0,
            since: Wall::from_nanos(100_000_000_000),
            ready_since: None,
            owns_turn: false,
            last_head: None,
        },
        pull: None,
        pending: None,
        effect: change::EffectResult::None,
        delegate: None,
        delegate_status: change::Status::Unknown,
        verdicts: Box::new([]),
        gate_remarks: Box::new([]),
        drift: None,
        base_repair: false,
        queue_repair: None,
        queue_repairs: 0,
    }
}

fn change_step(
    world: &mut Protocol,
    entry: u64,
    heard: change::Status,
    gates: Box<[change::GateReport]>,
) -> change::Decision {
    change_step_with_release(world, entry, heard, gates, false)
}

fn change_step_with_release(
    world: &mut Protocol,
    entry: u64,
    heard: change::Status,
    gates: Box<[change::GateReport]>,
    released: bool,
) -> change::Decision {
    change_step_task(world, 51, entry, heard, gates, released)
}

fn change_step_task(
    world: &mut Protocol,
    task: u64,
    entry: u64,
    heard: change::Status,
    gates: Box<[change::GateReport]>,
    released: bool,
) -> change::Decision {
    world.take_seen();
    world.event(top::Event::StepChange {
        task,
        entry,
        heard: change::Heard { delegate: heard, effect: change::EffectResult::None, released, cancelled: false },
        gates,
        queue_repair_active: false,
        kept: false,
    });
    world.run_for(1);
    let mut decision = None;
    let mut committed = None;
    for event in world.take_seen() {
        if let top::Request::ChangeDecision { task: decided, decision: current, entry, .. } = event
            && decided == task
        {
            decision = Some(current);
            committed = entry;
        }
    }
    if let Some(entry) = committed {
        world.event(top::Event::Committed { entry });
        world.run_for(1);
    }
    decision.expect("change step made a decision")
}
#[test]
fn a_change_produced_opened_checked_queued_and_landed() {
    let mut world = opened_change();
    let finished = land_change(&mut world);
    assert!(finished, "change should land");
    assert_eq!(world.writes(), 2);
}

#[test]
fn a_github_change_is_produced_and_opened() {
    let mut world = Protocol::github(11);
    world.run_for(1);
    world.restart();
    let world = opened_change_in(world);
    assert!(matches!(
        world.stored().get(&top::Key::Repository(REPO)),
        Some(top::Stored::Repository(row)) if matches!(row.protection, top::Protection::Absent)
    ));
    assert_eq!(world.tally().routes.get(&Route::OpenPull), Some(&1));
    let opened: Vec<_> = world.observations().iter().filter(|o| matches!(o, Observation::Opened { .. })).collect();
    assert!(matches!(opened.as_slice(), [Observation::Opened {
        repository, number, kind: Kind::Pull, branches: Some(branches), by: 9, ..
    }] if repository.as_ref() == b"acme/widgets" && *number > 0
        && branches.head.as_ref() == b"temper/51" && branches.base.as_ref() == b"main"
        && branches.commit == world.branch(b"temper/51")));
}

fn commit_read(world: &mut Protocol, read: skein_forge::api::Read) -> skein_forge::api::Answer {
    let owner = skein_lib::Token::new(1_u64 << 61);
    world.take_seen();
    world.event(top::Event::Client(skein_forge_client::Event::Read { owner, repository: REPO, read }));
    world.run_for(1);
    for request in world.take_seen() {
        if let top::Request::Read { owner: answered, result } = request {
            assert_eq!(answered, owner);
            return result.expect("commit read through bytes");
        }
    }
    panic!("commit read answered");
}

fn github_head_checks(world: &mut Protocol, expected: skein_forge::api::Check) {
    use skein_forge::api::{Answer, Ci, Read};
    let commit = temper_engine_forge_world::translate::commit(world.branch(b"temper/51"));
    assert!(
        matches!(commit_read(world, Read::Statuses { commit, page: 1 }),
            Answer::Statuses { ci: Ci::None, statuses, more: false } if statuses.is_empty()
        ),
        "GitHub CI has no classic statuses"
    );
    let Answer::Checks(rows) = commit_read(world, Read::Checks { commit }) else { panic!("check rows") };
    assert!(matches!(rows.as_ref(), [row] if row.context.as_ref() == b"build" && row.check == expected));
}

#[test]
fn a_github_change_is_checked_by_its_check_runs_and_lands() {
    use skein_fake_forge_domain::api::Check;
    use skein_forge_machine_world::joint;
    use skein_lib::Duration;

    let mut checks = joint::checks(1000);
    checks.latency_min = Duration::from_secs(20);
    checks.latency_max = Duration::from_secs(20);
    let mut world = opened_change_in(Protocol::github_with_checks(71, checks));
    let head = world.branch(b"temper/51");
    github_head_checks(&mut world, skein_forge::api::Check::Pending);
    assert!(world.observations().iter().any(|observation| matches!(observation,
        Observation::Reported { commit, state: Check::Pending, .. } if *commit == head
    )));
    // The delegate result is unknown: CI must come from the forge through bytes.
    let mut waiting = false;
    for entry in 4..7 {
        waiting |= matches!(
            change_step(&mut world, entry, change::Status::Unknown, Box::new([])),
            change::Decision::Wait { .. }
        );
    }
    assert!(waiting, "the pending check run holds the change");
    assert_eq!(world.tally().routes.get(&Route::Merge), None);
    world.run_for(20);
    github_head_checks(&mut world, skein_forge::api::Check::Passed);
    assert!(world.observations().iter().any(|observation| matches!(observation,
        Observation::Reported { commit, state: Check::Passed, .. } if *commit == head
    )));
    let mut finished = false;
    for entry in 7..24 {
        if matches!(
            change_step(&mut world, entry, change::Status::Unknown, Box::new([])),
            change::Decision::Finish { .. }
        ) {
            finished = true;
            break;
        }
    }
    assert!(finished, "passed check runs land the change");
    assert_eq!(world.tally().routes.get(&Route::Merge), Some(&1));
    let merges: Vec<_> =
        world.observations().iter().filter(|observation| matches!(observation, Observation::Merged { .. })).collect();
    assert!(matches!(merges.as_slice(), [Observation::Merged { head: checked, .. }] if *checked == head));
    assert_eq!(world.tally().routes.get(&Route::PostStatus), None);
}

#[test]
fn a_github_change_whose_check_run_fails_is_repaired() {
    use skein_fake_forge_domain::api::Check;
    use skein_forge_machine_world::joint;

    let mut world = opened_change_in(Protocol::github_with_checks(73, joint::checks(0)));
    let head = world.branch(b"temper/51");
    github_head_checks(&mut world, skein_forge::api::Check::Failed);
    assert!(world.observations().iter().any(|observation| matches!(observation,
        Observation::Reported { commit, state: Check::Failed, .. } if *commit == head
    )));
    let mut repaired = false;
    for entry in 4..8 {
        let decision = change_step(&mut world, entry, change::Status::Unknown, Box::new([]));
        if decision == change::Decision::Delegate(change::Delegate::Repair(change::Repair::Ci)) {
            repaired = true;
            break;
        }
    }
    assert!(repaired, "the failed check run requests a CI repair");
    assert_eq!(world.tally().routes.get(&Route::Merge), None);
    assert_eq!(world.tally().routes.get(&Route::PostStatus), None);
}

#[test]
fn a_created_issue_with_its_answer_lost_is_found_after_restart() {
    let mut world = Protocol::new(23);
    world.adopt();
    world.slow_calls(skein_lib::Duration::from_secs(1));
    let view = issues::GoalView {
        phase: issues::Phase::Waiting,
        goal: 77,
        repository: u64::from(REPO.repository),
        title: Box::from("Goal"),
        goal_text: Box::from("Plan"),
        plan: Box::new([]),
        milestones: Box::new([]),
        finished: None,
    };
    world.event(top::Event::Project { entry: 1, repository: REPO, view });
    world.event(top::Event::Committed { entry: 1 });
    world.run_for(1);
    assert_eq!(world.writes(), 1);
    world.restart();
    world.run_for(20);
    assert_eq!(world.writes(), 1);
    assert!(world.stored().get(&top::Key::Entry(1)).is_none());
    match world.stored().get(&top::Key::Issue(77)) {
        Some(top::Stored::Issue(row)) => assert!(row.number.is_some()),
        Some(_) | None => panic!("recovered issue"),
    }
}

#[test]
fn a_merge_with_its_answer_lost_is_found_after_restart_without_merging_twice() {
    let mut world = Protocol::new(89);
    world.adopt();
    world.produce(b"temper/51");
    let pull = world.open_pull(b"temper/51", b"main");
    let head = temper_engine_forge_world::translate::commit(world.branch(b"temper/51"));
    world.slow_calls(skein_lib::Duration::from_secs(3));
    enqueue_merge(&mut world, pull, head);
    world.event(top::Event::Committed { entry: 1 });
    world.run_for(11);
    assert_eq!(world.writes(), 1);
    assert!(world.stored().get(&top::Key::Entry(1)).is_some());
    let landed = world.branch(b"main");
    world.restart();
    world.run_for(30);
    assert_eq!(world.writes(), 1);
    assert_eq!(world.branch(b"main"), landed);
    assert!(world.stored().get(&top::Key::Entry(1)).is_none());
}

#[test]
fn a_failed_ci_head_is_repaired_then_checked_again() {
    let mut world = Protocol::new(13);
    world.adopt();
    world.event(top::Event::Change { row: change_row() });
    assert_eq!(
        change_step(&mut world, 1, change::Status::Unknown, Box::new([])),
        change::Decision::Delegate(change::Delegate::Produce)
    );
    world.produce(b"temper/51");
    assert_eq!(
        change_step(&mut world, 2, change::Status::Passed, Box::new([])),
        change::Decision::Effect(change::Effect::Open)
    );
    world.status(b"temper/51", skein_fake_forge_domain::api::Check::Failed);
    assert_eq!(
        change_step(&mut world, 3, change::Status::Passed, Box::new([])),
        change::Decision::Delegate(change::Delegate::Repair(change::Repair::Ci))
    );
    let before = world.writes();
    world.push(b"temper/51", b"repaired");
    world.status(b"temper/51", skein_fake_forge_domain::api::Check::Passed);
    let mut finished = false;
    for entry in 4..20 {
        if let change::Decision::Finish { .. } = change_step(&mut world, entry, change::Status::Passed, Box::new([])) {
            finished = true;
            break;
        }
    }
    assert!(finished);
    assert_eq!(world.writes(), before + 1); // only the merge is a connector write
}

#[test]
fn an_external_move_of_a_held_branch_is_reported_as_drift() {
    let mut world = Protocol::new(31);
    world.adopt();
    let (name, pushed) = held_branch(&mut world);
    world.push(b"temper/51", b"outside");
    world.event(top::Event::Hint {
        hint: skein_forge::api::Hint {
            repository: REPO,
            change: skein_forge::api::Change::Branch(Box::from(&b"temper/51"[..])),
            key: None,
        },
    });
    world.run_for(5);
    assert!(world.seen().iter().any(|event| matches!(event,
        top::Request::Drift { task: 51, resource } if resource == &name)));
    let Some(top::Stored::Hold(hold)) = world.stored().get(&top::Key::Hold(name.clone())) else {
        panic!("affected hold");
    };
    assert!(
        matches!(hold.drift.as_ref(), Some(top::Drift { at, change: top::DriftChange::Branch { expected, observed: Some(_) } }) if *expected == pushed && *at > Wall::EPOCH)
    );
    world.take_seen();
    step_change(&mut world, 10, change::Status::Passed, false);
    world.run_for(1);
    assert!(world.seen().iter().any(|event| matches!(
        event,
        top::Request::ChangeDecision { task: 51, decision: change::Decision::Hold(change::Hold::BranchMoved), .. }
    )));
    world.event(top::Event::ReleaseChange { task: 51 });
    world.take_seen();
    step_change(&mut world, 11, change::Status::Passed, true);
    world.run_for(1);
    assert!(!world.seen().iter().any(|event| matches!(
        event,
        top::Request::ChangeDecision { task: 51, decision: change::Decision::Hold(change::Hold::BranchMoved), .. }
    )));
}

#[test]
fn idle_forge_call_cost_does_not_follow_old_issue_history() {
    let mut small = Protocol::new(47);
    small.preload_history(2);
    small.adopt();
    small.event(top::Event::Names { task: 1, resources: Box::new([main_name()]) });
    small.run_for(60);
    let before = small.tally().total;
    small.run_for(60);
    let small_cost = small.tally().total - before;

    let mut large = Protocol::new(47);
    large.preload_history(20);
    large.adopt();
    large.event(top::Event::Names { task: 1, resources: Box::new([main_name()]) });
    large.run_for(60);
    let before = large.tally().total;
    large.run_for(60);
    assert_eq!(large.tally().total - before, small_cost);
}

#[test]
fn a_repository_with_more_branches_than_rows_is_adopted_with_one_branches_read() {
    use temper_engine_forge_world::LIMITS;

    let limits = top::Limits { client: skein_forge_client::Limits { rows: 3, ..LIMITS.client }, ..LIMITS };
    let mut world = Protocol::forgejo_with_limits(11, &limits);
    let main = world.branch(b"main");
    for number in 1..=3 {
        let branch = format!("work/{number}");
        world.push(branch.as_bytes(), b"outside temper's prefix");
        assert_ne!(world.branch(branch.as_bytes()), main, "the fake holds {branch} besides main");
    }
    assert_eq!(world.branch(b"main"), main);
    let before = world.tally().routes.get(&Route::Branches).copied().unwrap_or(0);
    world.adopt();
    let reads = world.tally().routes.get(&Route::Branches).copied().unwrap_or(0) - before;
    assert_eq!(reads, 1, "one read under the prefix: no page of the four, and no refused attempt retried");
}

#[test]
fn honest_costs_equal_the_requests_the_fake_served() {
    let mut world = Protocol::new(11);
    world.adopt();
    world.push(b"work", b"one");
    world.open_pull(b"work", b"main");
    world.run_for(60);
    let tally = world.tally();
    let starts = tally.routes.get(&Route::User).copied().unwrap_or(0)
        + tally.routes.get(&Route::ApiSettings).copied().unwrap_or(0);
    assert!(world.costs() > 0, "the story made calls");
    assert_eq!(world.costs(), tally.total - starts);
}

#[test]
fn a_busy_pass_lists_at_most_changed_items_over_the_page_plus_one() {
    let mut world = Protocol::new(12);
    world.adopt();
    world.event(top::Event::Names { task: 1, resources: Box::new([main_name()]) });
    world.run_for(61);
    for _ in 0..6 {
        world.issue(b"Changed", b"body");
    }
    let before = world.tally().routes.get(&Route::Issues).copied().unwrap_or(0);
    let first = world.listing_passes().len();
    world.event(top::Event::Hint {
        hint: skein_forge::api::Hint { repository: REPO, change: skein_forge::api::Change::Item(1), key: None },
    });
    world.run_for(2);
    let pages = world.tally().routes.get(&Route::Issues).copied().unwrap_or(0) - before;
    assert!(pages > 0, "the pass listed the changed items");
    let passes = &world.listing_passes()[first..];
    assert_eq!(passes.iter().sum::<u64>(), pages, "every HTTP listing is in a pass");
    for (pass, pages) in passes.iter().enumerate() {
        assert!(*pages <= 7, "pass {pass}: six changed items, page size one, plus one: {pages}");
    }
}

#[test]
fn the_api_token_reaches_no_record_request_or_fact() {
    let mut world = Protocol::new(11);
    world.adopt();
    world.event(top::Event::Project { entry: 1, repository: REPO, view: fixtures::goal_view() });
    world.event(top::Event::Committed { entry: 1 });
    world.run_for(60);
    assert_eq!(world.issues_made(), 1);
    assert!(!fixtures::token_absent(&fixtures::TOKEN), "the check recognizes byte secrets");
    assert!(
        !fixtures::token_absent(&core::str::from_utf8(fixtures::TOKEN).expect("ASCII token")),
        "the check recognizes text secrets"
    );
    assert!(world.token_kept());
}

fn main_name() -> top::Name {
    top::Name {
        forge: REPO.forge,
        repository: REPO.repository,
        what: top::What::Branch(Box::new([Box::from(b"main".as_slice())])),
    }
}

fn step_change(world: &mut Protocol, entry: u64, delegate: change::Status, released: bool) {
    world.event(top::Event::StepChange {
        task: 51,
        entry,
        heard: change::Heard { delegate, effect: change::EffectResult::None, released, cancelled: false },
        gates: Box::new([]),
        queue_repair_active: false,
        kept: false,
    });
}

fn land_change(world: &mut Protocol) -> bool {
    let mut finished = false;
    for entry in 4..24 {
        world.take_seen();
        step_change(world, entry, change::Status::Passed, false);
        world.run_for(1);
        let replies = world.take_seen();
        for reply in replies {
            match reply {
                top::Request::ChangeDecision { decision: change::Decision::Effect(_), entry: Some(number), .. } => {
                    world.event(top::Event::Committed { entry: number });
                    world.run_for(1);
                }
                top::Request::ChangeDecision { decision: change::Decision::Finish { .. }, .. } => finished = true,
                top::Request::ChangeDecision { decision: change::Decision::Delegate(_), .. } => {
                    panic!("unexpected delegate")
                }
                top::Request::Settled { .. }
                | top::Request::Bound { .. }
                | top::Request::ListDemoActs
                | top::Request::SendDemoAct(_)
                | top::Request::DemoStart { .. }
                | top::Request::StopGroup { .. }
                | top::Request::SetUpGroup { .. }
                | top::Request::Connect { .. }
                | top::Request::Manifest { .. }
                | top::Request::Reachable { .. }
                | top::Request::Install { .. }
                | top::Request::Accept { .. }
                | top::Request::CarriedUnused { .. }
                | top::Request::EraseCarried { .. }
                | top::Request::SetUp { .. }
                | top::Request::ChangeDecision { .. }
                | top::Request::Resource { .. }
                | top::Request::Adopted { .. }
                | top::Request::Save { .. }
                | top::Request::MarkedForges
                | top::Request::Marked { .. }
                | top::Request::Erase { .. }
                | top::Request::Taken { .. }
                | top::Request::Refused { .. }
                | top::Request::Outcome { .. }
                | top::Request::NotMade { .. }
                | top::Request::ContinueRelease { .. }
                | top::Request::EffectsSettled { .. }
                | top::Request::Released { .. }
                | top::Request::Retained { .. }
                | top::Request::ReleaseFailed { .. }
                | top::Request::ProjectionEffect { .. }
                | top::Request::WakeProcedure { .. }
                | top::Request::ProjectAfter { .. }
                | top::Request::ProjectionSettled { .. }
                | top::Request::GoalTopic { .. }
                | top::Request::GoalUntopic { .. }
                | top::Request::ProjectionFailed { .. }
                | top::Request::News { .. }
                | top::Request::Drift { .. }
                | top::Request::Read { .. }
                | top::Request::BriefClient { .. }
                | top::Request::BriefReady { .. }
                | top::Request::BriefSized { .. }
                | top::Request::BriefTaken { .. }
                | top::Request::ApplicationPart { .. }
                | top::Request::ApplicationDone { .. }
                | top::Request::ApplicationAgain { .. }
                | top::Request::RestartDone { .. }
                | top::Request::Call { .. } => {}
            }
        }
        if finished {
            break;
        }
    }
    finished
}

fn opened_change() -> Protocol {
    opened_change_in(Protocol::new(11))
}

fn opened_change_in(mut world: Protocol) -> Protocol {
    world.adopt();
    world.take_seen();
    let mut row = change_row();
    row.change.since = world.wall();
    world.event(top::Event::Change { row });
    step_change(&mut world, 2, change::Status::Unknown, false);
    world.run_for(1);
    assert!(world.seen().iter().any(|request| matches!(
        request,
        top::Request::ChangeDecision { task: 51, decision: change::Decision::Delegate(change::Delegate::Produce), .. }
    )));
    world.produce(b"temper/51");
    world.take_seen();
    step_change(&mut world, 3, change::Status::Passed, false);
    world.run_for(1);
    assert!(world.seen().iter().any(|request| matches!(
        request,
        top::Request::ChangeDecision {
            task: 51,
            decision: change::Decision::Effect(change::Effect::Open),
            entry: Some(3),
            ..
        }
    )));
    assert_eq!(world.writes(), 0);
    world.event(top::Event::Committed { entry: 3 });
    world.run_for(2);
    assert_eq!(world.writes(), 1);

    world
}

fn enqueue_merge(world: &mut Protocol, pull: u64, head: skein_forge::api::Commit) {
    world.event(top::Event::Enqueue {
        entry: skein_forge_client::Entry {
            number: 1,
            task: 51,
            repository: REPO,
            effect: skein_forge_client::Effect {
                write: skein_forge::api::Write::Merge { number: pull, head },
                condition: skein_forge_client::Condition::Merge { base: Box::from(&b"main"[..]) },
            },
            start: None,
            attempt: None,
            failures: 0,
            queued: false,
        },
    });
}

fn held_branch(world: &mut Protocol) -> (top::Name, skein_forge::api::Commit) {
    let name = top::Name {
        forge: REPO.forge,
        repository: REPO.repository,
        what: top::What::Branch(Box::new([Box::from(&b"temper"[..]), Box::from(&b"51"[..])])),
    };
    world.event(top::Event::Change { row: change_row() });
    world.event(top::Event::Names { task: 51, resources: Box::new([name.clone()]) });
    world.event(top::Event::Hold { task: 51, resource: name.clone(), from: None });
    world.run_for(1);
    world.event(top::Event::Claim { task: 51, attempt: 1, writes: Box::new([name.clone()]), holders: Box::new([]) });
    world.produce(b"temper/51");
    let pushed = temper_engine_forge_world::translate::commit(world.branch(b"temper/51"));
    world.event(top::Event::Answered { task: 51, attempt: 1, pushed: Box::new([(name.clone(), pushed)]) });
    world.run_for(1);
    world.take_seen();

    (name, pushed)
}

fn connected_github_with_two_installations() -> skein_forge_machine_world::joint::Joint {
    use skein_forge_connection::Request;
    use skein_forge_machine_world::joint::{self, Joint};
    use skein_lib::Duration;

    let mut world = Joint::github(
        11,
        0,
        fixtures::limits(),
        fixtures::github_settings(),
        fixtures::protocol_shared(),
        joint::checks(1000),
    );
    world.action(Request::Installations { forge: fixtures::FORGE, accepted: Box::from([1, 2]) });
    for _ in 0..10_000 {
        world.tick();
        if world.connected != 0 {
            break;
        }
        world.time(world.now().saturating_add(Duration::from_nanos(100_000)));
    }
    assert_eq!(world.connected, 1);
    assert!(world.stopped.is_empty());
    assert_eq!(world.git_tokens.len(), 2);
    world
}

#[test]
fn github_git_tokens_of_every_accepted_installation_come_before_connected() {
    use skein_forge_connection::Event;

    let world = connected_github_with_two_installations();
    let connected = world.events.iter().position(|event| event.starts_with("Connected {")).expect("Connected");
    assert_eq!(world.events[..connected].iter().filter(|event| event.starts_with("GitToken {")).count(), 2);

    // Provenance is asserted from events alone: the joint's fake service is private.
    for (installation_id, account_id, login) in [(1, 22, b"acme".as_slice()), (2, 23, b"example".as_slice())] {
        let mut matching = world
            .git_tokens
            .iter()
            .filter(|event| matches!(event, Event::GitToken { installation, .. } if *installation == installation_id));
        let Some(Event::GitToken { forge, account, token, expires, .. }) = matching.next() else {
            panic!("accepted installation's git token");
        };
        assert!(matching.next().is_none());
        assert_eq!(*forge, fixtures::FORGE);
        assert_eq!(account.id, account_id);
        assert_eq!(account.login.as_ref(), login);
        assert!(!token.expose().is_empty());
        assert!(*expires > world.env.wall);
    }
    let [Event::GitToken { token: first, .. }, Event::GitToken { token: second, .. }] = world.git_tokens.as_slice()
    else {
        panic!("exactly two git tokens");
    };
    assert!(first.expose() != second.expose());
}

#[test]
fn github_git_tokens_renew_before_they_expire() {
    use skein_forge_connection::Event;
    use skein_lib::Duration;

    let mut world = connected_github_with_two_installations();
    let life = skein_fake_forge_github::installations::TOKEN_LIFE;
    let renew_before = fixtures::limits().renew_before;
    // Run the ordinary polls and renewal timers through ten minutes before expiry,
    // with a second for both installations' mint exchanges to finish.
    world.run_for(Duration::from_nanos(life.as_nanos() - renew_before.as_nanos() + Duration::from_secs(1).as_nanos()));
    assert_eq!(world.git_tokens.len(), 4);

    for (installation_id, account_id, login) in [(1, 22, b"acme".as_slice()), (2, 23, b"example".as_slice())] {
        let mut matching = world
            .git_tokens
            .iter()
            .filter(|event| matches!(event, Event::GitToken { installation, .. } if *installation == installation_id));
        let Some(Event::GitToken { forge, account, token, expires, .. }) = matching.next() else {
            panic!("accepted installation's first git token");
        };
        let Some(Event::GitToken {
            forge: renewed_forge, account: renewed_account, token: next, expires: later, ..
        }) = matching.next()
        else {
            panic!("accepted installation's renewed git token");
        };
        assert!(matching.next().is_none());
        assert_eq!(*forge, fixtures::FORGE);
        assert_eq!(renewed_forge, forge);
        assert_eq!(account.id, account_id);
        assert_eq!(account.login.as_ref(), login);
        assert_eq!(renewed_account.id, account.id);
        assert_eq!(renewed_account.login, account.login);
        assert!(!token.expose().is_empty());
        assert!(!next.expose().is_empty());
        assert!(next.expose() != token.expose(), "renewal mints a different token");
        assert!(later > expires, "renewal extends the token's life");
        assert!(world.env.wall < *expires, "renewal completes before the first token expires");
    }
    assert_eq!(world.connected, 1);
    assert!(world.stopped.is_empty());
}

#[test]
fn github_reaches_ready_through_its_accepted_installations() {
    use skein_forge_machine_world::machine::{Answer, Machine};
    let mut world = Machine::github(
        7,
        fixtures::limits(),
        fixtures::github_settings(),
        [
            "GET /app/installations?per_page=100&page=1",
            "GET /app/installations?per_page=100&page=2",
            "POST /app/installations/1/access_tokens",
            "POST /app/installations/1/access_tokens",
            "POST /graphql",
        ]
        .into_iter()
        .map(Answer::served)
        .collect(),
    );
    world.action(skein_forge_connection::Request::Installations { forge: fixtures::FORGE, accepted: Box::from([1]) });
    world.log_requests(vec!["GET /app/hook/deliveries?per_page=8".into()]);
    world.run_for(skein_lib::Duration::from_secs(6));
    assert_eq!(world.connected_forges(), &[fixtures::FORGE]);
    assert_eq!(world.start(fixtures::FORGE), Some(skein_forge_connection::start::Start::Ready { cap: 100 }));
    assert_eq!(world.requests().len(), 6);
    assert_eq!(world.git_tokens().len(), 1);
    assert!(world.script_done());

    // Empty origins refuse a pinned Job before metadata, redirect or blob requests.
    let requests = world.requests().to_vec();
    let head = skein_fake_forge_protocol::routes::object(1).bytes;
    world.call(
        1,
        skein_forge::api::Repository { forge: fixtures::FORGE, repository: 2 },
        skein_forge::api::Op::Read(skein_forge::api::Read::Job {
            attempt: skein_forge::api::JobAttempt { head, run: 1, job: 1, attempt: 1 },
            max_bytes: 1_024,
        }),
    );
    // Settle the terminal in virtual time before the next delivery-log poll.
    world.run_for(skein_lib::Duration::from_millis(1));
    let answer = world.answered(1).expect("refused Job terminal");
    assert_eq!(answer.result, Err(skein_forge::api::Error::Refused));
    assert_eq!(answer.cost, 0);
    assert_eq!(world.deliveries().iter().filter(|answer| answer.call.raw() == 1).count(), 1);
    assert_eq!(world.requests(), requests);
    assert!(world.blob_records().is_empty());
    assert!(world.script_done());
    assert!(world.close());
}

fn branch_name(branch: &[u8]) -> top::Name {
    top::Name {
        forge: REPO.forge,
        repository: REPO.repository,
        what: top::What::Branch(branch.split(|b| *b == b'/').map(Box::from).collect()),
    }
}

fn request_routes(world: &Protocol, since: Wall, until: Wall) -> std::collections::BTreeMap<String, u64> {
    let mut routes = std::collections::BTreeMap::new();
    for (_, line) in world.requests().filter(|(at, _)| *at >= since && *at < until) {
        let line = String::from_utf8_lossy(line);
        let route = line.split(" HTTP/").next().expect("HTTP request").split('?').next().expect("target");
        *routes.entry(route.to_owned()).or_default() += 1;
    }
    routes
}

// A minute at the fastest shipped paces, plus the finite opening reads:
// listing: 60/5 + 1; three branch watches: 3*(60/40 + 1);
// pull passes (Item/comments + Pull/CI/GraphQL = 6 HTTP):
// 6*(60/5 + 60/40 + 2), including a slow repair and boundary passes;
// two fresh procedure reads: 2*(2 branches + lookup + 4 pull/CI +
// 2 base CI + compare + 2 files + reviews), plus opening the pull.
const PENDING_INSTALLATION_REQUESTS_PER_MINUTE: u64 =
    (60 / 5 + 1) + 3 * (60 / 40 + 1) + 6 * (60 / 5 + 60 / 40 + 2) + 2 * (2 + 1 + 4 + 2 + 1 + 2 + 1) + 1;

#[test]
#[expect(clippy::too_many_lines, reason = "one no-hint CI story with its clock, root names and cost referee")]
fn a_github_change_waits_out_a_ninety_second_ci_and_merges_on_its_verdict() {
    use skein_fake_forge_domain::api::Check;
    use skein_forge_machine_world::joint;
    use skein_lib::Duration;
    use temper_service::limits::{Profile, profile};

    let shipped = profile(Profile::Small).forge;
    assert_eq!(shipped.change_policy.stall, Duration::from_secs(30 * 60));
    let mut limits = temper_engine_forge_world::LIMITS;
    limits.client = shipped.client;
    limits.change_policy.stall = shipped.change_policy.stall;
    limits.resources_per_task = shipped.resources_per_task;
    let mut checks = joint::checks(1000);
    checks.latency_min = Duration::from_secs(90);
    checks.latency_max = Duration::from_secs(90);
    let mut world = Protocol::github_with_limits(71, checks, &limits);
    world.adopt();
    let run = branch_name(b"temper/1/r1");
    world.event(top::Event::Names { task: 1, resources: Box::new([run.clone()]) });
    world.event(top::Event::Hold { task: 1, resource: run, from: None });
    let mut row = change_row();
    row.change.since = world.wall();
    world.event(top::Event::Change { row });
    let branch = branch_name(b"temper/51");
    world.event(top::Event::Names { task: 51, resources: Box::new([branch.clone(), branch_name(b"main")]) });
    world.event(top::Event::Hold { task: 51, resource: branch.clone(), from: None });
    world.run_for(0);
    assert_eq!(
        change_step(&mut world, 2, change::Status::Unknown, Box::new([])),
        change::Decision::Delegate(change::Delegate::Produce)
    );
    let pushed = world.wall();
    world.produce(b"temper/51");
    let head = world.branch(b"temper/51");
    let verdict = Wall::from_nanos(pushed.as_nanos() + Duration::from_secs(90).as_nanos());
    let deadline =
        Wall::from_nanos(verdict.as_nanos() + shipped.client.poll_max.as_nanos() + Duration::from_secs(1).as_nanos());
    assert_eq!(
        change_step(&mut world, 3, change::Status::Passed, Box::new([])),
        change::Decision::Effect(change::Effect::Open)
    );
    let mut named_pull = false;
    let mut due = true;
    let mut until = None;
    let mut entry = 4;
    let mut hold_reason = None;
    let mut finished = false;
    // Play the root's procedure scheduler: wakes and the procedure's actual wait,
    // never a periodic StepChange that could discover CI on the keep's behalf.
    while world.wall() <= deadline {
        if !named_pull
            && let Some(top::Stored::Change(row)) = world.stored().get(&top::Key::Change(51))
            && let Some(number) = row.pull
        {
            world.event(top::Event::Rename {
                task: 51,
                resources: Box::new([
                    branch.clone(),
                    top::Name { forge: REPO.forge, repository: REPO.repository, what: top::What::Pull(number) },
                ]),
            });
            named_pull = true;
        }
        for request in world.take_seen() {
            #[expect(clippy::wildcard_enum_match_arm, reason = "only this change's scheduler requests drive the story")]
            match request {
                top::Request::WakeProcedure { task: 51 } => due = true,
                top::Request::ChangeDecision { task: 51, decision, entry: committed, .. } => {
                    if let Some(entry) = committed {
                        world.event(top::Event::Committed { entry });
                    }
                    match decision {
                        change::Decision::Wait { until: at } => until = at,
                        change::Decision::Hold(why) => hold_reason = Some(why),
                        change::Decision::Finish { .. } => finished = true,
                        change::Decision::None
                        | change::Decision::Delegate(_)
                        | change::Decision::CancelGates { .. }
                        | change::Decision::Effect(_)
                        | change::Decision::Ready
                        | change::Decision::QueueRepair
                        | change::Decision::Cancel => due = true,
                    }
                }
                _ => {}
            }
        }
        if hold_reason.is_some() || finished || world.tally().routes.get(&Route::Merge) == Some(&1) {
            break;
        }
        if until.is_some_and(|at| world.wall() >= at) {
            due = true;
            until = None;
        }
        if due {
            due = false;
            world.event(top::Event::StepChange {
                task: 51,
                entry,
                heard: change::Heard {
                    delegate: change::Status::Unknown,
                    effect: change::EffectResult::None,
                    released: false,
                    cancelled: false,
                },
                gates: Box::new([]),
                queue_repair_active: false,
                kept: false,
            });
            entry += 1;
            world.run_for(0);
        } else {
            world.run_for(1);
        }
    }
    let end = Wall::from_nanos(world.wall().as_nanos() + 1);
    let routes = request_routes(&world, pushed, end);
    let all = request_routes(&world, Wall::from_nanos(0), end);
    eprintln!(
        "90s CI: elapsed={}s, pending routes={:?}, all routes={all:?}",
        (world.wall().as_nanos() - pushed.as_nanos()) / 1_000_000_000,
        request_routes(&world, pushed, verdict)
    );
    let opening = world
        .requests()
        .find_map(|(at, line)| line.starts_with(b"POST /repos/acme/widgets/pulls HTTP/").then_some(at))
        .expect("pull opened");
    eprintln!(
        "phases: before pull={:?}; verdict through merge={:?}",
        request_routes(&world, Wall::from_nanos(0), opening),
        request_routes(&world, verdict, end)
    );
    for offset in 0..=30 {
        let since = Wall::from_nanos(pushed.as_nanos() + Duration::from_secs(offset).as_nanos());
        let until = Wall::from_nanos(since.as_nanos() + Duration::from_secs(60).as_nanos());
        let minute = request_routes(&world, since, until);
        let app = minute.get("GET /app/hook/deliveries").copied().unwrap_or(0);
        let installation: u64 = minute.values().sum::<u64>() - app;
        assert!(
            installation <= PENDING_INSTALLATION_REQUESTS_PER_MINUTE,
            "pending minute +{offset}s: {installation} installation requests, {app} App log reads; \
             bound {PENDING_INSTALLATION_REQUESTS_PER_MINUTE}; routes={minute:?}; all={all:?}"
        );
    }
    assert_eq!(world.requests().count() as u64, world.tally().total, "count every served request; {all:?}");
    assert!(named_pull, "pull must be named; {routes:?}");
    assert!(hold_reason.is_none(), "healthy CI held: {hold_reason:?}; {routes:?}");
    assert_eq!(world.delivery_hints(), 0, "no delivery hints; {routes:?}");
    assert!(
        world
            .observations()
            .iter()
            .any(|o| matches!(o, Observation::Reported { commit, state: Check::Passed, .. } if *commit == head)),
        "CI verdict; {routes:?}"
    );
    assert_eq!(world.tally().routes.get(&Route::Merge), Some(&1), "merge by poll_max + 1s; {routes:?}");
    assert!(world.wall() <= deadline, "merge by CI verdict + poll_max + 1s; {routes:?}");
    assert!(
        world.observations().iter().any(|o| matches!(o, Observation::Merged { head: merged, .. } if *merged == head)),
        "merge checked head; {routes:?}"
    );
    let resolutions: u64 =
        all.iter().filter(|(route, _)| route.starts_with("GET /repositories/")).map(|(_, count)| count).sum();
    assert!(resolutions <= 1, "resolve repository once; {all:?}");
    let seconds = (world.wall().as_nanos() - pushed.as_nanos()) / 1_000_000_000;
    assert!(
        routes.get("GET /repos/acme/widgets/branches/temper/1/r1").copied().unwrap_or(0) <= seconds / 40 + 1,
        "missing run branch at slow; {routes:?}"
    );
}

#[test]
fn a_pull_watch_swap_reads_through_bytes_but_cannot_rename_a_released_task() {
    let mut world = Protocol::github(93);
    world.adopt();
    world.produce(b"temper/51");
    let number = world.open_pull(b"temper/51", b"main");
    let branch = branch_name(b"temper/51");
    let pull = top::Name { forge: REPO.forge, repository: REPO.repository, what: top::What::Pull(number) };
    world.event(top::Event::Names { task: 51, resources: Box::new([branch.clone(), branch_name(b"main")]) });
    world.event(top::Event::Rename { task: 51, resources: Box::new([branch.clone(), pull.clone()]) });
    world.run_for(1);
    assert!(matches!(world.stored().get(&top::Key::Names(51)),
        Some(top::Stored::Names { resources, .. }) if resources.as_ref() == [branch.clone(), pull.clone()]));
    let pull_route = format!("GET /repos/acme/widgets/pulls/{number}");
    let reads = request_routes(&world, Wall::EPOCH, world.wall()).get(&pull_route).copied().unwrap_or(0);
    assert!(reads > 0, "the swapped pull read through bytes");
    world.event(top::Event::Unnamed { task: 51 });
    world.run_for(1);
    let reads = request_routes(&world, Wall::EPOCH, world.wall()).get(&pull_route).copied().unwrap_or(0);
    world.event(top::Event::Rename { task: 51, resources: Box::new([branch, pull]) });
    world.run_for(60);
    assert!(world.stored().get(&top::Key::Names(51)).is_none(), "the queued swap cannot recreate names");
    assert_eq!(
        request_routes(&world, Wall::EPOCH, world.wall()).get(&pull_route).copied().unwrap_or(0),
        reads,
        "no pull watch recreated"
    );
    world.finish();
}

#[test]
fn a_sha1_pull_files_answer_matches_the_requested_head_and_fills_the_review_brief() {
    use skein_forge::api::{Answer, Read};
    use temper_engine_forge_world::translate;
    let mut world = Protocol::new(1352);
    world.adopt();
    world.produce_file(b"temper/51", b"review.txt", b"the reviewed contents");
    let pull = world.open_pull(b"temper/51", b"main");
    let head = translate::commit(world.branch(b"temper/51"));
    assert_eq!(&head[20..], &[0; 12], "the fake Forgejo serves SHA-1 heads");
    let Answer::Pull(observed) = commit_read(&mut world, Read::Pull { number: pull }) else {
        panic!("pull through bytes")
    };
    assert_eq!(observed.commit, head);
    let Answer::PullFiles { head: observed, files, more } =
        commit_read(&mut world, Read::PullFiles { number: pull, head, page: 1 })
    else {
        panic!("pull files matched by the client")
    };
    assert_eq!(observed, head);
    assert!(!more);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path.as_ref(), b"review.txt");
    assert_eq!(files[0].after.as_deref(), Some(b"the reviewed contents".as_slice()));
    let mut row = change_row();
    row.pull = Some(pull);
    row.change.last_head = Some(head);
    row.change.state = change::State::Gating { head, asked: Box::new([1]) };
    world.event(top::Event::Change { row });
    world.take_seen();
    let owner = skein_lib::Token::new(1352);
    world.event(top::Event::GatherBrief {
        owner,
        source: top::BriefSource::Pull {
            item: top::BriefItem { repository: REPO.repository, number: pull },
            head: top::BriefCommit(head),
        },
        parts: 8,
        bytes: 4096,
        max_job_bytes: 0,
    });
    world.run_for(5);
    let words = world
        .take_seen()
        .into_vec()
        .into_iter()
        .find_map(|request| {
            if let top::Request::BriefReady { owner: found, read: top::BriefRead::Got { bytes, left: 0 } } = request
                && found == owner
            {
                Some(bytes)
            } else {
                None
            }
        })
        .expect("the review's complete pull section came through");
    let words = core::str::from_utf8(&words).expect("review text");
    assert!(words.contains("Files and diff:"), "{words}");
    assert!(words.contains("File: review.txt"), "{words}");
    assert!(words.contains("After:\nthe reviewed contents"), "{words}");
}
