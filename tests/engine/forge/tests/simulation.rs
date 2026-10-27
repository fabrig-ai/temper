use skein_lib::{Token, Wall};
use temper_engine_domain_forge as top;
use temper_engine_domain_forge_change as change;
use temper_engine_domain_forge_issues as issues;
use temper_engine_forge_world::changes::{
    change_row, change_step, change_step_task, change_step_with_release, reviewed_change,
};
use temper_engine_forge_world::{REPO, World, translate};

#[test]
fn a_merge_seen_by_the_base_read_before_the_pull_read_is_not_a_retarget() {
    let mut world = World::new(141);
    world.adopt();
    world.produce(b"temper/51");
    world.status(b"temper/51", skein_fake_forge_domain::api::Check::Passed);
    let pull = world.open_pull(b"temper/51", b"main");
    let head = translate::commit(world.branch(b"temper/51"));
    let mut row = change_row();
    row.pull = Some(pull);
    row.change.state = change::State::Landing { head };
    row.change.last_head = Some(head);
    row.change.since = world.wall();
    row.effect = change::EffectResult::Made;
    world.event(top::Event::Change { row });
    let merge = world.merge_before_pull_read(pull);

    let decision = change_step(&mut world, 100, change::Status::Passed, Box::new([]));
    assert!(matches!(decision, change::Decision::Wait { .. }), "await the merged pull: {decision:?}");
    let Some(top::Stored::Change(saved)) = world.stored().get(&top::Key::Change(51)) else { panic!("change retained") };
    assert_eq!(saved.change.state, change::State::Landing { head });
    assert_eq!(saved.base.as_ref(), b"main");

    world.restart();
    assert_eq!(change_step(&mut world, 101, change::Status::Passed, Box::new([])), change::Decision::Finish { merge });
    assert_eq!(world.writes(), 0, "the completed merge is never sent again");
}

#[test]
fn an_existing_repository_is_adopted_after_its_permission_and_collaborators_are_read() {
    let mut world = World::new(3);
    world.adopt();
    let row = world.stored().get(&top::Key::Repository(REPO));
    assert!(row.is_some());
    let mut adopted = false;
    for event in world.seen() {
        if let top::Request::Adopted { result: Ok(value), .. } = event {
            assert_eq!(value.repository.project, 5);
            assert!(!value.collaborators.is_empty());
            adopted = true;
        }
    }
    assert!(adopted);
    assert_eq!(world.writes(), 0);
}

#[test]
fn a_repository_with_another_deployments_branch_prefix_is_refused() {
    let mut world = World::with_branches(4, 222);
    world.preload_branches(220);
    world.produce(b"temper/other");
    world.adopt();
    assert!(
        world.seen().iter().any(|request| matches!(
            request,
            top::Request::Adopted { result: Err(skein_forge::api::Error::Refused), .. }
        ))
    );
    assert!(world.stored().get(&top::Key::Repository(REPO)).is_none());
}

#[test]
fn a_later_adoption_rechecks_the_prefix_and_accepts_only_recorded_writer_branches() {
    let mut world = World::new(4);
    world.adopt();
    world.produce(b"temper/own");
    let name = top::Name {
        forge: REPO.forge,
        repository: REPO.repository,
        what: top::What::Branch(Box::new([Box::from(&b"temper"[..]), Box::from(&b"own"[..])])),
    };
    world.event(top::Event::Names { task: 9, resources: Box::new([name.clone()]) });
    world.event(top::Event::Hold { task: 9, resource: name.clone(), from: None });
    world.event(top::Event::Claim { task: 9, attempt: 1, writes: Box::new([name.clone()]), holders: Box::new([9]) });
    world.event(top::Event::Answered {
        task: 9,
        attempt: 1,
        pushed: Box::new([(name, translate::commit(world.branch(b"temper/own")))]),
    });
    drop(world.take_seen());
    world.adopt();
    assert!(world.seen().iter().any(|request| matches!(request, top::Request::Adopted { result: Ok(_), .. })));
    world.produce(b"temper/foreign");
    drop(world.take_seen());
    world.adopt();
    assert!(
        world.seen().iter().any(|request| matches!(
            request,
            top::Request::Adopted { result: Err(skein_forge::api::Error::Refused), .. }
        ))
    );
}

#[test]
fn a_context_repository_cannot_receive_a_write() {
    let mut world = World::new(5);
    world.adopt_as(top::Role::Context);
    world.event(top::Event::Enqueue {
        entry: skein_forge_client::Entry {
            number: 1,
            task: 9,
            repository: REPO,
            effect: skein_forge_client::Effect {
                write: skein_forge::api::Write::CreateIssue {
                    key: Box::from(&b"context"[..]),
                    title: Box::from(&b"No"[..]),
                    body: Box::from(&b"write"[..]),
                },
                condition: skein_forge_client::Condition::None,
            },
            start: None,
            attempt: None,
            failures: 0,
            queued: false,
        },
    });
    world.event(top::Event::Committed { entry: 1 });
    world.run_for(2);
    assert!(world.seen().iter().any(|request| matches!(request, top::Request::Refused { task: 9 })));
    assert_eq!(world.writes(), 0);
}

#[test]
fn a_permission_refusal_rereads_capabilities_and_narrows_later_writes() {
    let mut world = World::new(5);
    world.adopt();
    world.permission(skein_fake_forge_domain::api::Permission::Read);
    let entry = skein_forge_client::Entry {
        number: 1,
        task: 9,
        repository: REPO,
        effect: skein_forge_client::Effect {
            write: skein_forge::api::Write::Status {
                commit: translate::commit(1),
                context: Box::from(&b"build"[..]),
                check: skein_forge::api::Check::Passed,
                description: Box::new([]),
                link: Box::new([]),
            },
            condition: skein_forge_client::Condition::None,
        },
        start: None,
        attempt: None,
        failures: 0,
        queued: false,
    };
    world.event(top::Event::Enqueue { entry: entry.clone() });
    world.event(top::Event::Committed { entry: 1 });
    world.run_for(5);
    assert!(world.seen().iter().any(|request| matches!(
        request,
        top::Request::Outcome { outcome: skein_forge_client::Outcome::Failed(skein_forge::api::Error::Forbidden), .. }
    )));
    let Some(top::Stored::Repository(repository)) = world.stored().get(&top::Key::Repository(REPO)) else {
        panic!("repository");
    };
    assert!(repository.kinds.read);
    assert!(!repository.kinds.issue && !repository.kinds.push && !repository.kinds.land);
    let before = world.calls();
    world.event(top::Event::Enqueue { entry: skein_forge_client::Entry { number: 2, ..entry } });
    world.run_for(1);
    assert_eq!(world.calls(), before, "later writes are refused before any provider call");
    assert!(world.seen().iter().any(|request| matches!(request, top::Request::Refused { task: 9 })));
}

#[test]
fn a_brief_section_refuses_a_head_that_moved_before_its_gather() {
    let mut world = World::new(84);
    world.adopt();
    let mut row = change_row();
    row.pull = Some(17);
    row.change.last_head = Some([1; 32]);
    world.event(top::Event::Change { row });
    world.take_seen();
    let before = world.calls();
    let owner = Token::new(93);
    world.event(top::Event::GatherBrief {
        owner,
        source: top::BriefSource::Reviews {
            item: top::BriefItem { repository: REPO.repository, number: 17 },
            head: top::BriefCommit([2; 32]),
        },
        parts: 1,
        bytes: 128,
        max_job_bytes: 0,
    });
    assert!(world.seen().contains(&top::Request::BriefReady { owner, read: top::BriefRead::Failed }));
    assert_eq!(world.calls(), before, "a stale pinned section does not read the provider");
    let section = Token::new(94);
    world.event(top::Event::GatherBriefHeld {
        section,
        source: top::BriefSource::Reviews {
            item: top::BriefItem { repository: REPO.repository, number: 17 },
            head: top::BriefCommit([2; 32]),
        },
        parts: 1,
        bytes: 128,
        max_job_bytes: 0,
    });
    assert!(world.seen().contains(&top::Request::BriefSized { section, size: None }));
    assert_eq!(world.calls(), before, "a stale held section does not read the provider");
}

#[test]
fn a_forge_brief_section_stays_with_its_connector_until_taken() {
    let mut world = World::new(85);
    world.adopt();
    world.produce(b"temper/51");
    let pull = world.open_pull(b"temper/51", b"main");
    let head = translate::commit(world.branch(b"temper/51"));
    let mut row = change_row();
    row.pull = Some(pull);
    row.change.last_head = Some(head);
    world.event(top::Event::Change { row });
    assert!(world.stored().get(&top::Key::Change(51)).is_some(), "change registered: {:?}", world.seen());
    world.take_seen();
    let section = Token::new(94);
    world.event(top::Event::GatherBriefHeld {
        section,
        source: top::BriefSource::Pull {
            item: top::BriefItem { repository: REPO.repository, number: pull },
            head: top::BriefCommit(head),
        },
        parts: 1,
        bytes: 512,
        max_job_bytes: 0,
    });
    assert!(world.seen().is_empty(), "gather startup: {:?}", world.seen());
    world.run_for(5);
    let size = world
        .seen()
        .iter()
        .find_map(|request| {
            if let top::Request::BriefSized { section: found, size } = request {
                if *found == section { *size } else { None }
            } else {
                None
            }
        })
        .unwrap_or_else(|| panic!("connector kept the gathered words: {:?}", world.seen()));
    assert!(size > 40);
    world.take_seen();
    world.event(top::Event::CutBrief { section, bytes: 40 });
    let cut = world
        .seen()
        .iter()
        .find_map(|request| {
            if let top::Request::BriefSized { section: found, size } = request {
                if *found == section { *size } else { None }
            } else {
                None
            }
        })
        .expect("connector cut its retained words");
    assert!(cut <= 40);
    world.take_seen();
    world.event(top::Event::TakeBrief { section });
    assert!(world.seen().iter().any(|request| matches!(request,
        top::Request::BriefTaken { section: found, bytes: Some(words) }
            if *found == section
                && words.len() == usize::try_from(cut).expect("bounded cut")
                && words.windows(b" bytes cut]".len()).any(|window| window == b" bytes cut]")
    )));
    world.take_seen();
    world.event(top::Event::TakeBrief { section });
    assert_eq!(world.seen(), [top::Request::BriefTaken { section, bytes: None }]);
}

#[test]
fn dropping_a_forge_section_while_it_gathers_releases_its_token() {
    let mut world = World::new(86);
    world.adopt();
    world.produce(b"temper/51");
    let pull = world.open_pull(b"temper/51", b"main");
    let head = translate::commit(world.branch(b"temper/51"));
    let mut row = change_row();
    row.pull = Some(pull);
    row.change.last_head = Some(head);
    world.event(top::Event::Change { row });
    world.take_seen();
    let section = Token::new(95);
    world.event(top::Event::GatherBriefHeld {
        section,
        source: top::BriefSource::Pull {
            item: top::BriefItem { repository: REPO.repository, number: pull },
            head: top::BriefCommit(head),
        },
        parts: 1,
        bytes: 512,
        max_job_bytes: 0,
    });
    world.event(top::Event::DropBrief { section });
    world.run_for(5);
    assert!(
        !world
            .seen()
            .iter()
            .any(|request| matches!(request, top::Request::BriefSized { section: found, .. } if *found == section))
    );
    world.take_seen();
    world.event(top::Event::TakeBrief { section });
    assert_eq!(world.seen(), [top::Request::BriefTaken { section, bytes: None }]);
}

#[test]
fn a_review_asking_for_changes_repeats_at_the_repaired_head() {
    let (mut world, head) = reviewed_change(false, change::Freshness::Exact);
    assert_eq!(
        change_step(&mut world, 1, change::Status::Unknown, Box::new([])),
        change::Decision::Delegate(change::Delegate::Gate { number: 9, head })
    );
    assert_eq!(
        change_step(
            &mut world,
            2,
            change::Status::Passed,
            Box::new([change::GateReport { number: 9, head, status: change::Status::Failed }])
        ),
        change::Decision::Delegate(change::Delegate::Repair(change::Repair::Gate(9)))
    );
    world.push(b"temper/51", b"review repair");
    world.status(b"temper/51", skein_fake_forge_domain::api::Check::Passed);
    let new_head = temper_engine_forge_world::translate::commit(world.branch(b"temper/51"));
    assert_eq!(
        change_step(&mut world, 3, change::Status::Passed, Box::new([])),
        change::Decision::Delegate(change::Delegate::Gate { number: 9, head: new_head })
    );
    let verdict = Box::new([change::GateReport { number: 9, head: new_head, status: change::Status::Passed }]);
    let mut finished = false;
    for entry in 4..18 {
        if let change::Decision::Finish { .. } = change_step(&mut world, entry, change::Status::Passed, verdict.clone())
        {
            finished = true;
            break;
        }
    }
    assert!(finished);
    assert_eq!(world.writes(), 1);
}

#[test]
fn a_gate_added_while_queued_takes_the_change_out_until_approved() {
    let (mut world, head) = reviewed_change(true, change::Freshness::Exact);
    assert_eq!(
        change_step(&mut world, 1, change::Status::Passed, Box::new([])),
        change::Decision::Delegate(change::Delegate::Gate { number: 9, head })
    );
    assert_eq!(world.writes(), 0);
    let verdict = Box::new([change::GateReport { number: 9, head, status: change::Status::Passed }]);
    let mut finished = false;
    for entry in 2..15 {
        if let change::Decision::Finish { .. } = change_step(&mut world, entry, change::Status::Passed, verdict.clone())
        {
            finished = true;
            break;
        }
    }
    assert!(finished);
    assert_eq!(world.writes(), 1);
}

#[test]
fn a_clean_update_keeps_its_review_and_lands_the_new_head() {
    let (mut world, head) = reviewed_change(true, change::Freshness::Clean);
    world.push_file(b"main", b"other", b"base moved");
    let verdict = Box::new([change::GateReport { number: 9, head, status: change::Status::Passed }]);
    assert!(matches!(
        change_step(&mut world, 1, change::Status::Passed, verdict.clone()),
        change::Decision::Effect(change::Effect::Update { .. })
    ));
    let updated = temper_engine_forge_world::translate::commit(world.branch(b"temper/51"));
    assert_ne!(updated, head);
    let mut finished = false;
    for entry in 2..16 {
        let decided = change_step(&mut world, entry, change::Status::Passed, verdict.clone());
        assert!(!matches!(decided, change::Decision::Delegate(change::Delegate::Gate { .. })));
        if let change::Decision::Finish { .. } = decided {
            finished = true;
            break;
        }
    }
    assert!(finished);
    assert_eq!(world.writes(), 2);
}

#[test]
fn an_exact_head_approval_is_asked_again_after_a_clean_update() {
    let (mut world, head) = reviewed_change(true, change::Freshness::Exact);
    world.push_file(b"main", b"other", b"base moved");
    let verdict = Box::new([change::GateReport { number: 9, head, status: change::Status::Passed }]);
    assert!(matches!(
        change_step(&mut world, 1, change::Status::Passed, verdict.clone()),
        change::Decision::Effect(change::Effect::Update { .. })
    ));
    let updated = temper_engine_forge_world::translate::commit(world.branch(b"temper/51"));
    assert_ne!(updated, head);
    assert_eq!(
        change_step(&mut world, 2, change::Status::Passed, verdict),
        change::Decision::Delegate(change::Delegate::Gate { number: 9, head: updated })
    );
    assert_eq!(world.writes(), 1);
}

#[test]
fn a_conflicting_update_is_resolved_from_a_merge_in_progress() {
    let (mut world, head) = reviewed_change(true, change::Freshness::Clean);
    world.push(b"main", b"conflicting base");
    let verdict = Box::new([change::GateReport { number: 9, head, status: change::Status::Passed }]);
    assert!(matches!(
        change_step(&mut world, 1, change::Status::Passed, verdict),
        change::Decision::Effect(change::Effect::Update { .. })
    ));
    let base = temper_engine_forge_world::translate::commit(world.branch(b"main"));
    assert_eq!(
        change_step(&mut world, 2, change::Status::Passed, Box::new([])),
        change::Decision::Delegate(change::Delegate::Resolve { base })
    );
    assert_eq!(world.writes(), 1);
    world.resolve(b"temper/51", b"resolved content");
    world.status(b"temper/51", skein_fake_forge_domain::api::Check::Passed);
    let resolved = temper_engine_forge_world::translate::commit(world.branch(b"temper/51"));
    assert_eq!(
        change_step(&mut world, 3, change::Status::Passed, Box::new([])),
        change::Decision::Delegate(change::Delegate::Gate { number: 9, head: resolved })
    );
    let verdict = Box::new([change::GateReport { number: 9, head: resolved, status: change::Status::Passed }]);
    let mut finished = false;
    for entry in 4..18 {
        if let change::Decision::Finish { .. } = change_step(&mut world, entry, change::Status::Passed, verdict.clone())
        {
            finished = true;
            break;
        }
    }
    assert!(finished);
    assert_eq!(world.writes(), 2);
}

#[test]
fn a_clean_update_with_broken_ci_is_repaired_as_a_semantic_conflict() {
    let (mut world, head) = reviewed_change(true, change::Freshness::Clean);
    world.push_file(b"main", b"other", b"base moved");
    let verdict = Box::new([change::GateReport { number: 9, head, status: change::Status::Passed }]);
    assert!(matches!(
        change_step(&mut world, 1, change::Status::Passed, verdict.clone()),
        change::Decision::Effect(change::Effect::Update { .. })
    ));
    world.status(b"temper/51", skein_fake_forge_domain::api::Check::Failed);
    assert_eq!(
        change_step(&mut world, 2, change::Status::Passed, verdict),
        change::Decision::Delegate(change::Delegate::Repair(change::Repair::Semantic))
    );
    assert_eq!(world.writes(), 1);
    world.push(b"temper/51", b"semantic repair");
    world.status(b"temper/51", skein_fake_forge_domain::api::Check::Passed);
    let repaired = temper_engine_forge_world::translate::commit(world.branch(b"temper/51"));
    assert_eq!(
        change_step(&mut world, 3, change::Status::Passed, Box::new([])),
        change::Decision::Delegate(change::Delegate::Gate { number: 9, head: repaired })
    );
}

#[test]
fn a_pull_closed_by_someone_else_is_held_then_reopened_on_release() {
    let (mut world, head) = reviewed_change(false, change::Freshness::Exact);
    let pull = match world.stored().get(&top::Key::Change(51)) {
        Some(top::Stored::Change(row)) => row.pull.expect("fixture pull"),
        Some(_) | None => panic!("fixture change"),
    };
    let name = top::Name {
        forge: REPO.forge,
        repository: REPO.repository,
        what: top::What::Branch(Box::new([Box::from(&b"temper"[..]), Box::from(&b"51"[..])])),
    };
    world.event(top::Event::Hold { task: 51, resource: name.clone(), from: None });
    world.close(pull);
    assert_eq!(
        change_step(&mut world, 1, change::Status::Passed, Box::new([])),
        change::Decision::Hold(change::Hold::PullClosed)
    );
    assert_eq!(world.writes(), 0);
    let Some(top::Stored::Hold(hold)) = world.stored().get(&top::Key::Hold(name)) else { panic!("held change") };
    assert_eq!(hold.drift.as_ref().expect("closed pull details").change, top::DriftChange::PullClosed { number: pull });
    assert!(hold.drift.as_ref().expect("drift timestamp").at > Wall::EPOCH);
    world.event(top::Event::ReleaseChange { task: 51 });
    assert_eq!(
        change_step_with_release(&mut world, 2, change::Status::Passed, Box::new([]), true),
        change::Decision::Effect(change::Effect::Reopen)
    );
    assert_eq!(world.writes(), 1);
    assert_eq!(
        change_step(&mut world, 3, change::Status::Passed, Box::new([])),
        change::Decision::Delegate(change::Delegate::Gate { number: 9, head })
    );
}

#[test]
fn a_broken_landing_branch_pauses_the_queue_for_one_repair() {
    let (mut world, head) = reviewed_change(true, change::Freshness::Clean);
    world.status(b"main", skein_fake_forge_domain::api::Check::Failed);
    let report = Box::new([change::GateReport { number: 9, head, status: change::Status::Passed }]);
    assert_eq!(change_step(&mut world, 1, change::Status::Passed, report.clone()), change::Decision::QueueRepair);
    assert_eq!(world.writes(), 0);
    world.take_seen();
    world.event(top::Event::StepChange {
        task: 51,
        entry: 2,
        heard: change::Heard {
            delegate: change::Status::Passed,
            effect: change::EffectResult::None,
            released: false,
            cancelled: false,
        },
        gates: report,
        queue_repair_active: true,
        kept: false,
    });
    world.run_for(1);
    assert!(world.seen().iter().any(|event| matches!(
        event,
        top::Request::ChangeDecision { task: 51, decision: change::Decision::Wait { .. }, .. }
    )));
    assert_eq!(world.writes(), 0);
}

#[test]
fn an_aged_low_priority_change_takes_its_turn_ahead_of_later_work() {
    let mut world = World::new(83);
    world.adopt();
    world.produce(b"temper/51");
    world.produce_file(b"temper/52", b"other", b"second change");
    world.status(b"temper/51", skein_fake_forge_domain::api::Check::Passed);
    world.status(b"temper/52", skein_fake_forge_domain::api::Check::Passed);
    let pull51 = world.open_pull(b"temper/51", b"main");
    let pull52 = world.open_pull(b"temper/52", b"main");
    let head51 = temper_engine_forge_world::translate::commit(world.branch(b"temper/51"));
    let head52 = temper_engine_forge_world::translate::commit(world.branch(b"temper/52"));
    let mut low = change_row();
    low.pull = Some(pull51);
    low.priority = 0;
    low.change.state = change::State::Queued { head: head51, ready_since: Wall::EPOCH };
    low.change.last_head = Some(head51);
    world.event(top::Event::Change { row: low });
    let mut high = change_row();
    high.task = 52;
    high.change.task = 52;
    high.branch = Box::from(&b"temper/52"[..]);
    high.pull = Some(pull52);
    high.priority = 10;
    high.change.state = change::State::Queued { head: head52, ready_since: Wall::from_nanos(100_000_000_000) };
    high.change.last_head = Some(head52);
    high.change.gates = Box::new([change::Gate {
        number: 7,
        kind: change::GateKind::Agent,
        blocking: true,
        freshness: change::Freshness::Clean,
        eager: false,
    }]);
    world.event(top::Event::Change { row: high });
    let review = Box::new([change::GateReport { number: 7, head: head52, status: change::Status::Passed }]);
    assert_eq!(
        change_step_task(&mut world, 52, 1, change::Status::Passed, review.clone(), false, false),
        change::Decision::Wait { until: None }
    );
    assert_eq!(world.writes(), 0);
    assert!(matches!(
        change_step(&mut world, 2, change::Status::Passed, Box::new([])),
        change::Decision::Effect(change::Effect::Merge { .. })
    ));
    assert_eq!(world.writes(), 1);
    assert!(matches!(
        change_step_task(&mut world, 52, 3, change::Status::Passed, review.clone(), false, false),
        change::Decision::Effect(change::Effect::Update { .. })
    ));
    let new_head = temper_engine_forge_world::translate::commit(world.branch(b"temper/52"));
    assert_ne!(new_head, head52);
    let mut finished = false;
    for entry in 4..16 {
        let decided = change_step_task(&mut world, 52, entry, change::Status::Passed, review.clone(), false, false);
        assert!(!matches!(decided, change::Decision::Delegate(change::Delegate::Gate { .. })));
        if let change::Decision::Finish { .. } = decided {
            finished = true;
            break;
        }
    }
    assert!(finished);
    assert_eq!(world.writes(), 3);
}

#[test]
fn a_kept_change_leaves_the_queue_and_comes_back_at_its_place() {
    let mut world = World::new(84);
    world.adopt();
    let earliest = Wall::EPOCH;
    for (task, branch, path, ready_since) in [
        (51, &b"temper/51"[..], &b"first"[..], earliest),
        (52, &b"temper/52"[..], &b"second"[..], Wall::from_nanos(1_000_000_000)),
        (53, &b"temper/53"[..], &b"third"[..], Wall::from_nanos(2_000_000_000)),
    ] {
        world.produce_file(branch, path, b"change");
        world.status(branch, skein_fake_forge_domain::api::Check::Passed);
        let pull = world.open_pull(branch, b"main");
        let head = translate::commit(world.branch(branch));
        let mut row = change_row();
        row.task = task;
        row.change.task = task;
        row.branch = Box::from(branch);
        row.pull = Some(pull);
        row.change.last_head = Some(head);
        row.change.ready_since = Some(ready_since);
        row.change.owns_turn = task == 51;
        row.change.state = if task == 51 {
            change::State::First { head, ready_since }
        } else {
            change::State::Queued { head, ready_since }
        };
        world.event(top::Event::Change { row });
    }
    world.take_seen();
    world.event(top::Event::StepChange {
        task: 51,
        entry: 1,
        heard: change::Heard {
            delegate: change::Status::Passed,
            effect: change::EffectResult::None,
            released: false,
            cancelled: false,
        },
        gates: Box::new([]),
        queue_repair_active: false,
        kept: true,
    });
    world.run_for(1);
    assert!(world.seen().iter().any(|event| matches!(
        event,
        top::Request::ChangeDecision { task: 51, decision: change::Decision::Wait { until: None }, .. }
    )));
    assert!(world.seen().iter().any(|event| matches!(event, top::Request::WakeProcedure { task: 52 })));
    assert_eq!(world.writes(), 0);
    let Some(top::Stored::Change(kept)) = world.stored().get(&top::Key::Change(51)) else { panic!("kept row") };
    assert!(matches!(kept.change.state, change::State::Gating { .. }));
    assert_eq!(kept.change.ready_since, Some(earliest));
    assert!(!kept.change.owns_turn);
    assert!(matches!(
        change_step_task(&mut world, 52, 2, change::Status::Passed, Box::new([]), false, false),
        change::Decision::Effect(change::Effect::Merge { .. })
    ));
    assert_eq!(world.writes(), 1);
    assert_eq!(change_step(&mut world, 3, change::Status::Passed, Box::new([])), change::Decision::Ready);
    let Some(top::Stored::Change(back)) = world.stored().get(&top::Key::Change(51)) else { panic!("ready row") };
    assert!(matches!(back.change.state, change::State::Queued { ready_since, .. } if ready_since == earliest));
    assert_eq!(
        change_step_task(&mut world, 53, 4, change::Status::Passed, Box::new([]), false, false),
        change::Decision::Wait { until: None }
    );
    for task in [51, 53] {
        let mut merged = false;
        let mut finished = false;
        for offset in 0..8 {
            let decision = change_step_task(
                &mut world,
                task,
                task * 100 + offset,
                change::Status::Passed,
                Box::new([]),
                false,
                false,
            );
            assert!(!matches!(decision, change::Decision::Delegate(_)));
            if matches!(decision, change::Decision::Effect(change::Effect::Merge { .. })) {
                merged = true;
            }
            if matches!(decision, change::Decision::Finish { .. }) {
                finished = true;
                break;
            }
        }
        assert!(merged && finished, "change {task} lands in order");
    }
    assert_eq!(world.writes(), 5); // three merges and two clean updates
}

#[test]
fn a_failed_ci_head_is_repaired_then_checked_again() {
    let mut world = World::new(13);
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
#[expect(clippy::too_many_lines, reason = "the full change story names each connector output explicitly")]
fn a_change_produced_opened_checked_queued_and_landed() {
    let mut world = World::new(11);
    world.adopt();
    world.take_seen();
    world.event(top::Event::Change { row: change_row() });
    world.event(top::Event::StepChange {
        task: 51,
        entry: 2,
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
    world.run_for(1);
    assert!(world.seen().iter().any(|request| matches!(
        request,
        top::Request::ChangeDecision { task: 51, decision: change::Decision::Delegate(change::Delegate::Produce), .. }
    )));
    world.produce(b"temper/51");
    world.take_seen();
    world.event(top::Event::StepChange {
        task: 51,
        entry: 3,
        heard: change::Heard {
            delegate: change::Status::Passed,
            effect: change::EffectResult::None,
            released: false,
            cancelled: false,
        },
        gates: Box::new([]),
        queue_repair_active: false,
        kept: false,
    });
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
    let mut finished = false;
    for entry in 4..24 {
        world.take_seen();
        world.event(top::Event::StepChange {
            task: 51,
            entry,
            heard: change::Heard {
                delegate: change::Status::Passed,
                effect: change::EffectResult::None,
                released: false,
                cancelled: false,
            },
            gates: Box::new([]),
            queue_repair_active: false,
            kept: false,
        });
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
                top::Request::Bound { .. }
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
                | top::Request::TierRun { .. }
                | top::Request::TierEffect { .. }
                | top::Request::TierDecision { .. }
                | top::Request::LandingDecision { .. }
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
                | top::Request::Settled { .. }
                | top::Request::RestartDone { .. }
                | top::Request::Call { .. } => {}
            }
        }
        if finished {
            break;
        }
    }
    assert!(finished, "change should land");
    assert_eq!(world.writes(), 2);
}

#[test]
fn a_change_landed_where_its_repository_has_a_tier_lives_verifying() {
    const PROJECT: u32 = 5;
    const CHANGE: u64 = 51;
    // The ordinary change story gives the procedure twenty entries to settle its merge.
    const LAST_ENTRY: u64 = 24;
    let mut world = World::new(11);
    world.adopt();
    assert!(world.project_tiers(
        PROJECT,
        top::TierSettings {
            verdict: temper_engine_domain_forge_tiers::Settings {
                retries: 2,
                suspects: temper_engine_domain_forge_tiers::Suspects::Bisect
            },
            bound: skein_lib::Duration::from_secs(7200),
            pause_on_red: false
        },
        Box::new([top::RepositoryTiers {
            repository: REPO,
            tiers: Box::new([top::Tier {
                name: b"browser".as_slice().into(),
                workflow: b"tier.yml".as_slice().into(),
                ignored: Box::new([]),
                artifact: b"report".as_slice().into(),
                format: temper_engine_domain_forge_tiers::Format::Lines
            }]),
        }])
    ));
    let branch = top::Name {
        forge: REPO.forge,
        repository: REPO.repository,
        what: top::What::Branch(Box::new([b"temper".as_slice().into(), b"51".as_slice().into()])),
    };
    world.event(top::Event::Hold { task: CHANGE, resource: branch.clone(), from: None });
    world.event(top::Event::Change { row: change_row() });
    let producing = change_step(&mut world, 2, change::Status::Unknown, Box::new([]));
    assert_eq!(producing, change::Decision::Delegate(change::Delegate::Produce));
    world.produce(b"temper/51");
    let mut merge = None;
    for entry in 3..LAST_ENTRY {
        let decision = change_step(&mut world, entry, change::Status::Passed, Box::new([]));
        assert!(!matches!(decision, change::Decision::Finish { .. }));
        if let Some(top::Stored::Change(row)) = world.stored().get(&top::Key::Change(CHANGE))
            && let change::State::Landed { merge: commit } = row.change.state
        {
            merge = Some(commit);
            break;
        }
    }
    let merge = merge.expect("the merge is seen");
    assert_eq!(change_step(&mut world, LAST_ENTRY, change::Status::Passed, Box::new([])), change::Decision::None);
    let expected = top::Verification {
        repository: REPO,
        commit: merge,
        changes: Box::new([CHANGE]),
        tiers: Box::new([top::TierVerification { tier: b"browser".as_slice().into(), verdict: None, repaired: None }]),
    };
    assert_eq!(
        world.stored().get(&top::Key::Verification { repository: REPO, commit: merge }),
        Some(&top::Stored::Verification(expected))
    );
    assert!(world.stored().contains_key(&top::Key::Hold(branch)));
    assert_eq!(translate::commit(world.branch(b"main")), merge);
    let _ = world.branch(b"temper/51");
}

#[test]
fn a_goal_issue_is_created_only_after_its_outbox_commit() {
    let mut world = World::new(7);
    world.adopt();
    let view = issues::GoalView {
        phase: issues::Phase::Waiting,
        goal: 42,
        repository: u64::from(REPO.repository),
        title: Box::from("Goal"),
        goal_text: Box::from("Plan"),
        plan: Box::new([]),
        milestones: Box::new([]),
        finished: None,
    };
    world.event(top::Event::Project { entry: 1, repository: REPO, view });
    world.run_for(1);
    assert_eq!(world.writes(), 0);
    world.event(top::Event::Committed { entry: 1 });
    world.run_for(2);
    assert_eq!(world.writes(), 1);
    let mut made = false;
    for event in world.seen() {
        if let top::Request::Outcome { entry: 1, outcome: skein_forge_client::Outcome::Made { .. }, .. } = event {
            made = true;
        }
    }
    assert!(made);
    assert!(world.stored().get(&top::Key::Entry(1)).is_none());
    match world.stored().get(&top::Key::Issue(42)) {
        Some(top::Stored::Issue(row)) => {
            assert!(row.number.is_some());
            assert_eq!(row.pending, None);
        }
        Some(_) | None => panic!("committed issue projection"),
    }
}

#[test]
fn a_goals_issue_is_revised_and_closed_as_the_goal_finishes() {
    let mut world = World::new(71);
    world.adopt();
    let mut view = issues::GoalView {
        phase: issues::Phase::Waiting,
        goal: 42,
        repository: u64::from(REPO.repository),
        title: Box::from("Goal"),
        goal_text: Box::from("Plan"),
        plan: Box::new([]),
        milestones: Box::new([]),
        finished: None,
    };
    world.event(top::Event::Project { entry: 1, repository: REPO, view: view.clone() });
    world.event(top::Event::Committed { entry: 1 });
    world.run_for(2);
    view.goal_text = Box::from("Revised plan");
    world.run_for(6);
    world.event(top::Event::Project { entry: 2, repository: REPO, view: view.clone() });
    world.event(top::Event::Committed { entry: 2 });
    world.run_for(2);
    assert_eq!(world.writes(), 2);
    view.finished = Some(Box::from("Accepted"));
    let mut closed = false;
    for entry in 3..10 {
        world.run_for(6);
        world.event(top::Event::Project { entry, repository: REPO, view: view.clone() });
        if world.stored().get(&top::Key::Entry(entry)).is_some() {
            world.event(top::Event::Committed { entry });
            world.run_for(2);
        }
        if let Some(top::Stored::Issue(row)) = world.stored().get(&top::Key::Issue(42))
            && row.state.closed
        {
            closed = true;
            break;
        }
    }
    assert!(closed);
    assert!(world.writes() >= 3);
}

#[test]
fn a_created_issue_with_its_answer_lost_is_found_after_restart() {
    let mut world = World::new(23);
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
fn a_restored_write_waits_for_the_fresh_repository_read() {
    let mut world = World::new(124);
    world.adopt();
    world.event(top::Event::Names {
        task: 51,
        resources: Box::new([top::Name {
            forge: REPO.forge,
            repository: REPO.repository,
            what: top::What::Branch(Box::new([Box::from(&b"main"[..])])),
        }]),
    });
    world.run_for(1);
    world.event(top::Event::Enqueue {
        entry: skein_forge_client::Entry {
            number: 1,
            task: 51,
            repository: REPO,
            effect: skein_forge_client::Effect {
                write: skein_forge::api::Write::CreateIssue {
                    key: Box::from(&b"restart-read"[..]),
                    title: Box::from(&b"Restart read"[..]),
                    body: Box::from(&b"after fresh repository state"[..]),
                },
                condition: skein_forge_client::Condition::None,
            },
            start: None,
            attempt: None,
            failures: 0,
            queued: false,
        },
    });
    assert!(world.stored().contains_key(&top::Key::Entry(1)));
    world.take_seen();
    world.slow_calls(skein_lib::Duration::from_secs(5));
    world.restart();
    world.run_for(2);
    assert_eq!(world.writes(), 0, "a restored entry cannot write before the fresh read");
    assert!(world.seen().contains(&top::Request::RestartDone { stage: top::RestartStage::Restored }));
    assert!(!world.seen().contains(&top::Request::RestartDone { stage: top::RestartStage::ReadAfresh }));
    world.run_for(20);
    assert_eq!(world.writes(), 1);
    let stages: Vec<_> = world
        .seen()
        .iter()
        .filter_map(|request| if let top::Request::RestartDone { stage } = request { Some(*stage) } else { None })
        .collect();
    assert_eq!(stages, [top::RestartStage::Restored, top::RestartStage::ReadAfresh, top::RestartStage::Settled]);
}

#[test]
fn a_merge_with_its_answer_lost_is_found_after_restart_without_merging_twice() {
    let mut world = World::new(89);
    world.adopt();
    world.produce(b"temper/51");
    let pull = world.open_pull(b"temper/51", b"main");
    let head = temper_engine_forge_world::translate::commit(world.branch(b"temper/51"));
    world.slow_calls(skein_lib::Duration::from_secs(5));
    world.event(top::Event::Enqueue {
        entry: skein_forge_client::Entry {
            number: 1,
            task: 51,
            repository: REPO,
            effect: skein_forge_client::Effect {
                write: skein_forge::api::Write::Merge { number: pull, head, how: skein_forge::api::MergeHow::Style },
                condition: skein_forge_client::Condition::Merge { base: Box::from(&b"main"[..]) },
            },
            start: None,
            attempt: None,
            failures: 0,
            queued: false,
        },
    });
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
fn a_merge_decided_for_an_old_head_cannot_land_the_new_head() {
    let mut world = World::new(90);
    world.adopt();
    world.produce(b"temper/51");
    let pull = world.open_pull(b"temper/51", b"main");
    let old_head = temper_engine_forge_world::translate::commit(world.branch(b"temper/51"));
    let landing = world.branch(b"main");
    world.push(b"temper/51", b"later work");
    world.event(top::Event::Enqueue {
        entry: skein_forge_client::Entry {
            number: 1,
            task: 51,
            repository: REPO,
            effect: skein_forge_client::Effect {
                write: skein_forge::api::Write::Merge {
                    number: pull,
                    head: old_head,
                    how: skein_forge::api::MergeHow::Style,
                },
                condition: skein_forge_client::Condition::Merge { base: Box::from(&b"main"[..]) },
            },
            start: None,
            attempt: None,
            failures: 0,
            queued: false,
        },
    });
    world.event(top::Event::Committed { entry: 1 });
    world.run_for(10);
    assert_eq!(world.branch(b"main"), landing);
    assert_eq!(world.writes(), 0);
}

#[test]
fn a_reported_worker_push_is_not_taken_for_outside_drift() {
    let mut world = World::new(91);
    world.adopt();
    let name = top::Name {
        forge: REPO.forge,
        repository: REPO.repository,
        what: top::What::Branch(Box::new([Box::from(&b"temper"[..]), Box::from(&b"51"[..])])),
    };
    world.event(top::Event::Names { task: 51, resources: Box::new([name.clone()]) });
    world.event(top::Event::Hold { task: 51, resource: name.clone(), from: None });
    world.event(top::Event::Claim { task: 51, attempt: 1, writes: Box::new([name.clone()]), holders: Box::new([]) });
    world.produce(b"temper/51");
    let pushed = temper_engine_forge_world::translate::commit(world.branch(b"temper/51"));
    world.event(top::Event::Answered { task: 51, attempt: 1, pushed: Box::new([(name, pushed)]) });
    world.run_for(1);
    world.take_seen();
    world.event(top::Event::Hint {
        hint: skein_forge::api::Hint {
            repository: REPO,
            change: skein_forge::api::Change::Branch(Box::from(&b"temper/51"[..])),
            key: None,
        },
    });
    world.run_for(5);
    assert!(!world.seen().iter().any(|event| matches!(event, top::Request::Drift { task: 51, .. })));
}

#[test]
fn closing_a_landed_change_deletes_its_branch_before_releasing_the_task() {
    let mut world = World::new(92);
    world.adopt();
    world.produce(b"temper/51");
    let name = top::Name {
        forge: REPO.forge,
        repository: REPO.repository,
        what: top::What::Branch(Box::new([Box::from(&b"temper"[..]), Box::from(&b"51"[..])])),
    };
    world.event(top::Event::Hold { task: 51, resource: name.clone(), from: None });
    world.take_seen();
    world.event(top::Event::Release { task: 51, root: 51, ending: top::ReleaseEnding::Done, reason: None, entry: 10 });
    assert!(world.stored().contains_key(&top::Key::Release(51)));
    assert!(world.stored().contains_key(&top::Key::Entry(10)));
    assert!(!world.seen().iter().any(|request| matches!(request, top::Request::Released { task: 51 })));
    world.event(top::Event::Committed { entry: 10 });
    world.run_for(5);
    assert!(world.seen().iter().any(|request| matches!(request, top::Request::ContinueRelease { task: 51 })));
    world.event(top::Event::ContinueRelease { task: 51, entry: 11 });
    assert!(world.seen().iter().any(|request| matches!(request, top::Request::Released { task: 51 })));
    assert!(!world.stored().contains_key(&top::Key::Hold(name)));
    assert!(!world.stored().contains_key(&top::Key::Release(51)));
    assert_eq!(world.writes(), 1);
}

#[test]
fn a_failed_childs_branch_is_kept_for_its_root_then_deleted_when_the_root_closes() {
    let mut world = World::new(124);
    world.adopt();
    world.produce(b"temper/51");
    let name = top::Name {
        forge: REPO.forge,
        repository: REPO.repository,
        what: top::What::Branch(Box::new([Box::from(&b"temper"[..]), Box::from(&b"51"[..])])),
    };
    world.event(top::Event::Hold { task: 51, resource: name.clone(), from: None });
    world.take_seen();
    world.event(top::Event::SettleEffects { task: 51, root: 50, ending: top::ReleaseEnding::Failed });
    world.event(top::Event::Release {
        task: 51,
        root: 50,
        ending: top::ReleaseEnding::Failed,
        reason: None,
        entry: 10,
    });
    assert!(world.seen().iter().any(|request| matches!(request,
        top::Request::Retained { task: 51, root: 50, resource } if *resource == name
    )));
    assert!(matches!(world.stored().get(&top::Key::Hold(name.clone())),
        Some(top::Stored::Hold(hold)) if hold.task == 50
    ));
    assert_eq!(world.writes(), 0);
    world.event(top::Event::SettleEffects { task: 50, root: 50, ending: top::ReleaseEnding::Done });
    world.event(top::Event::Release { task: 50, root: 50, ending: top::ReleaseEnding::Done, reason: None, entry: 11 });
    assert!(world.stored().contains_key(&top::Key::Entry(11)));
    world.event(top::Event::Committed { entry: 11 });
    world.run_for(5);
    world.event(top::Event::ContinueRelease { task: 50, entry: 12 });
    assert!(!world.stored().contains_key(&top::Key::Hold(name)));
    assert_eq!(world.writes(), 1);
}

#[test]
fn prior_effects_settle_before_cleanup_can_delete_a_held_resource() {
    let mut world = World::new(121);
    world.adopt();
    world.produce(b"temper/51");
    let name = top::Name {
        forge: REPO.forge,
        repository: REPO.repository,
        what: top::What::Branch(Box::new([Box::from(&b"temper"[..]), Box::from(&b"51"[..])])),
    };
    world.event(top::Event::Hold { task: 51, resource: name.clone(), from: None });
    world.event(top::Event::Enqueue {
        entry: skein_forge_client::Entry {
            number: 9,
            task: 51,
            repository: REPO,
            effect: skein_forge_client::Effect {
                write: skein_forge::api::Write::CreateIssue {
                    key: Box::from(&b"prior"[..]),
                    title: Box::from(&b"Prior effect"[..]),
                    body: Box::from(&b"settles first"[..]),
                },
                condition: skein_forge_client::Condition::None,
            },
            start: None,
            attempt: None,
            failures: 0,
            queued: false,
        },
    });
    world.take_seen();
    world.event(top::Event::SettleEffects { task: 51, root: 51, ending: top::ReleaseEnding::Done });
    assert!(!world.seen().iter().any(|request| matches!(request, top::Request::EffectsSettled { task: 51 })));
    assert!(world.stored().contains_key(&top::Key::Hold(name.clone())));
    world.event(top::Event::Committed { entry: 9 });
    world.run_for(5);
    world.event(top::Event::ContinueRelease { task: 51, entry: 10 });
    assert!(world.seen().iter().any(|request| matches!(request, top::Request::EffectsSettled { task: 51 })));
    assert!(world.stored().contains_key(&top::Key::Hold(name.clone())));
    assert!(!world.stored().contains_key(&top::Key::Entry(10)));
    world.event(top::Event::Release { task: 51, root: 51, ending: top::ReleaseEnding::Done, reason: None, entry: 11 });
    assert!(matches!(
        world.stored().get(&top::Key::Entry(11)),
        Some(top::Stored::Entry {
            entry: skein_forge_client::Entry {
                effect: skein_forge_client::Effect { write: skein_forge::api::Write::DeleteBranch { .. }, .. },
                ..
            },
            held: false,
        })
    ));
}

#[test]
fn cancelling_withdraws_a_prior_effect_that_never_went_out() {
    let mut world = World::new(122);
    world.adopt();
    world.event(top::Event::Enqueue {
        entry: skein_forge_client::Entry {
            number: 9,
            task: 51,
            repository: REPO,
            effect: skein_forge_client::Effect {
                write: skein_forge::api::Write::CreateIssue {
                    key: Box::from(&b"unsent"[..]),
                    title: Box::from(&b"Unsent"[..]),
                    body: Box::from(&b"no effect"[..]),
                },
                condition: skein_forge_client::Condition::None,
            },
            start: None,
            attempt: None,
            failures: 0,
            queued: false,
        },
    });
    world.take_seen();
    world.event(top::Event::SettleEffects { task: 51, root: 51, ending: top::ReleaseEnding::Cancelled });
    assert!(!world.stored().contains_key(&top::Key::Entry(9)));
    assert!(world.seen().iter().any(|request| matches!(
        request,
        top::Request::Outcome { entry: 9, outcome: skein_forge_client::Outcome::Withdrawn, .. }
    )));
    assert!(world.seen().iter().any(|request| matches!(request, top::Request::EffectsSettled { task: 51 })));
    assert_eq!(world.writes(), 0);
}

#[test]
fn cancellation_during_release_retargets_the_durable_cleanup() {
    let mut world = World::new(123);
    world.adopt();
    world.produce(b"temper/51");
    let name = top::Name {
        forge: REPO.forge,
        repository: REPO.repository,
        what: top::What::Branch(Box::new([Box::from(&b"temper"[..]), Box::from(&b"51"[..])])),
    };
    world.event(top::Event::Hold { task: 51, resource: name, from: None });
    world.event(top::Event::SettleEffects { task: 51, root: 51, ending: top::ReleaseEnding::Done });
    world.event(top::Event::Release { task: 51, root: 51, ending: top::ReleaseEnding::Done, reason: None, entry: 11 });
    assert!(world.stored().contains_key(&top::Key::Entry(11)));
    world.take_seen();
    world.event(top::Event::SettleEffects { task: 51, root: 51, ending: top::ReleaseEnding::Cancelled });
    world.event(top::Event::Release {
        task: 51,
        root: 51,
        ending: top::ReleaseEnding::Cancelled,
        reason: None,
        entry: 12,
    });
    assert!(
        matches!(world.stored().get(&top::Key::Release(51)), Some(top::Stored::Release(row)) if row.ending == top::ReleaseEnding::Cancelled)
    );
    assert!(!world.seen().iter().any(|request| matches!(request, top::Request::ReleaseFailed { task: 51 })));
}

#[test]
fn a_ci_subscription_reads_its_head_and_restarts_without_repeating_old_news() {
    let mut world = World::new(94);
    world.adopt();
    world.produce(b"temper/51");
    world.status(b"temper/51", skein_fake_forge_domain::api::Check::Passed);
    let head = temper_engine_forge_world::translate::commit(world.branch(b"temper/51"));
    let topic = top::Topic::Ci { repository: REPO, head };
    world.event(top::Event::Subscribe {
        subscription: top::Subscriber {
            task: 51,
            number: 1,
            topic,
            own_change: None,
            goal_tasks: None,
            paths: Box::new([]),
        },
    });
    world.run_for(2);
    assert!(world.seen().iter().any(|request| matches!(request, top::Request::News { task: 51, news: top::News::Ci { head: seen, .. }, .. } if *seen == head)));
    world.take_seen();
    world.restart();
    world.run_for(2);
    assert!(
        !world
            .seen()
            .iter()
            .any(|request| matches!(request, top::Request::News { task: 51, news: top::News::Ci { .. }, .. }))
    );
    world.status(b"temper/51", skein_fake_forge_domain::api::Check::Failed);
    world.event(top::Event::Hint {
        hint: skein_forge::api::Hint { repository: REPO, change: skein_forge::api::Change::Commit(head), key: None },
    });
    world.run_for(2);
    assert!(world.seen().iter().any(|request| matches!(
        request,
        top::Request::News { task: 51, news: top::News::Ci { status: skein_forge::api::Ci::Failed, .. }, .. }
    )));
    assert!(matches!(
        world.stored().get(&top::Key::Ci { repository: REPO, head }),
        Some(top::Stored::Ci(top::CiState { status: skein_forge::api::Ci::Failed, .. }))
    ));
}

const CANCEL_REASON: &[u8] = b"The new direction defers this work.";
const RELEASE_TASK: u64 = 51;

fn release_reason_key() -> Box<[u8]> {
    top::purpose::EffectPurpose::Release { task: RELEASE_TASK, resource: Box::from(b"reason".as_slice()) }
        .key(b"world", temper_engine_forge_world::LIMITS.client.op_bytes)
        .expect("reason key fits")
}

/// A cancelled change owns one pull and its pushed branch.
fn cancelled_pull(world: &mut World) -> u64 {
    world.adopt();
    world.produce(b"temper/51");
    let pull = world.open_pull(b"temper/51", b"main");
    let mut row = change_row();
    row.pull = Some(pull);
    world.event(top::Event::Change { row });
    let name = top::Name {
        forge: REPO.forge,
        repository: REPO.repository,
        what: top::What::Branch(Box::new([Box::from(&b"temper"[..]), Box::from(&b"51"[..])])),
    };
    world.event(top::Event::Hold { task: RELEASE_TASK, resource: name, from: None });
    world.take_seen();
    world.observations();
    pull
}

fn cancel_pull(world: &mut World, reason: &[u8]) {
    world.event(top::Event::Release {
        task: RELEASE_TASK,
        root: RELEASE_TASK,
        ending: top::ReleaseEnding::Cancelled,
        reason: Some(Box::from(reason)),
        entry: 10,
    });
    world.event(top::Event::Committed { entry: 10 });
}

fn finish_cancel(world: &mut World) {
    // The reason's terminal allows the close, then the branch deletion.
    for entry in [11, 12] {
        world.event(top::Event::ContinueRelease { task: RELEASE_TASK, entry });
        world.event(top::Event::Committed { entry });
        world.run_for(5);
    }
    world.event(top::Event::ContinueRelease { task: RELEASE_TASK, entry: 13 });
    assert!(world.seen().iter().any(|request| matches!(request, top::Request::Released { task: RELEASE_TASK })));
}

#[test]
fn cancelling_a_change_comments_its_reason_then_closes_its_pull_and_deletes_its_branch() {
    use skein_forge::api::{Op, Write};
    let mut world = World::new(93);
    let pull = cancelled_pull(&mut world);
    world.take_calls();
    cancel_pull(&mut world, CANCEL_REASON);
    world.run_for(5);
    finish_cancel(&mut world);
    let writes: Vec<_> = world
        .take_calls()
        .into_iter()
        .filter_map(|op| match op {
            Op::Write(write) => Some(write),
            Op::Read(_) => None,
        })
        .collect();
    let key = release_reason_key();
    assert_eq!(
        writes,
        vec![
            Write::Post { number: pull, key, body: Box::from(CANCEL_REASON) },
            Write::Close { number: pull },
            Write::DeleteBranch { branch: Box::from(b"temper/51".as_slice()) },
        ]
    );
    assert_eq!(world.writes(), 3);
}

#[test]
fn a_restart_after_the_reason_is_posted_posts_it_once() {
    use skein_fake_forge_domain::Observation;
    let mut world = World::new(93);
    let pull = cancelled_pull(&mut world);
    cancel_pull(&mut world, CANCEL_REASON);
    world.run_until_posted();
    assert!(matches!(world.stored().get(&top::Key::Release(RELEASE_TASK)),
        Some(top::Stored::Release(row)) if row.comment.as_deref() == Some(CANCEL_REASON) && row.pending.is_some()));
    world.restart();
    world.run_for(60);
    finish_cancel(&mut world);
    let comments: Vec<_> = world
        .observations()
        .into_iter()
        .filter_map(
            |record| if let Observation::Commented { number, body, .. } = record { Some((number, body)) } else { None },
        )
        .collect();
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].0, pull);
    // The independent forge retains the protocol's embedded identity marker.
    let key = release_reason_key();
    assert_eq!(comments[0].1, temper_engine_forge_world::translate::keyed(&key, CANCEL_REASON));
    assert_eq!(world.writes(), 3);
}

#[test]
fn a_permanently_failed_reason_post_still_closes_and_releases() {
    let mut world = World::new(93);
    cancelled_pull(&mut world);
    world.permission(skein_fake_forge_domain::api::Permission::Read);
    cancel_pull(&mut world, CANCEL_REASON);
    world.run_for(5);
    assert!(matches!(world.stored().get(&top::Key::Release(RELEASE_TASK)),
        Some(top::Stored::Release(row)) if row.comment.is_none() && !row.failed && row.close_pull.is_some()));
    world.permission(skein_fake_forge_domain::api::Permission::Write);
    finish_cancel(&mut world);
}

#[test]
fn releases_without_a_cancelled_pull_post_no_reason() {
    for ending in [top::ReleaseEnding::Cancelled, top::ReleaseEnding::Done, top::ReleaseEnding::Failed] {
        let mut world = World::new(93);
        world.adopt();
        if ending != top::ReleaseEnding::Cancelled {
            world.produce(b"temper/51");
            let pull = world.open_pull(b"temper/51", b"main");
            let mut row = change_row();
            row.pull = Some(pull);
            world.event(top::Event::Change { row });
        }
        world.take_calls();
        world.event(top::Event::Release {
            task: RELEASE_TASK,
            root: RELEASE_TASK,
            ending,
            reason: Some(Box::from(CANCEL_REASON)),
            entry: 10,
        });
        world.run_for(5);
        assert!(
            !world
                .take_calls()
                .iter()
                .any(|op| matches!(op, skein_forge::api::Op::Write(skein_forge::api::Write::Post { .. })))
        );
        assert!(world.seen().iter().any(|request| matches!(request, top::Request::Released { task: RELEASE_TASK })));
    }
}

/// A change watches its branch and its pull (domain/forge.md 5.1): the Closed
/// its own cancellation wrote is no drift (8.6), before or after its outcome.
fn watched_change(world: &mut World) -> (u64, top::Name) {
    world.adopt();
    world.produce(b"temper/51");
    let pull = world.open_pull(b"temper/51", b"main");
    let mut row = change_row();
    row.pull = Some(pull);
    world.event(top::Event::Change { row });
    let name = top::Name {
        forge: REPO.forge,
        repository: REPO.repository,
        what: top::What::Branch(Box::new([Box::from(&b"temper"[..]), Box::from(&b"51"[..])])),
    };
    let watched = top::Name { forge: REPO.forge, repository: REPO.repository, what: top::What::Pull(pull) };
    world.event(top::Event::Names { task: 51, resources: Box::new([name.clone(), watched]) });
    world.event(top::Event::Hold { task: 51, resource: name.clone(), from: None });
    world.run_for(60);
    world.take_seen();
    (pull, name)
}

#[test]
fn a_cancelled_change_watching_its_pull_deletes_its_branch_past_its_own_close() {
    let mut world = World::new(94);
    let (pull, _) = watched_change(&mut world);
    world.event(top::Event::Release {
        task: 51,
        root: 51,
        ending: top::ReleaseEnding::Cancelled,
        reason: None,
        entry: 10,
    });
    world.event(top::Event::Committed { entry: 10 });
    // Past the Close's outcome, the pull's keep reads it Closed.
    world.run_for(60);
    let Some(top::Stored::Change(row)) = world.stored().get(&top::Key::Change(51)) else { panic!("change kept") };
    assert_eq!(row.pull, Some(pull));
    assert!(!world.seen().iter().any(|request| matches!(request, top::Request::Drift { task: 51, .. })));
    world.event(top::Event::ContinueRelease { task: 51, entry: 11 });
    assert!(matches!(
        world.stored().get(&top::Key::Entry(11)),
        Some(top::Stored::Entry {
            entry: skein_forge_client::Entry {
                effect: skein_forge_client::Effect { write: skein_forge::api::Write::DeleteBranch { .. }, .. },
                ..
            },
            held: false,
        })
    ));
    world.event(top::Event::Committed { entry: 11 });
    world.run_for(5);
    world.event(top::Event::ContinueRelease { task: 51, entry: 12 });
    assert!(world.seen().iter().any(|request| matches!(request, top::Request::Released { task: 51 })));
    assert!(!world.seen().iter().any(|request| matches!(request, top::Request::Drift { task: 51, .. })));
    assert_eq!(world.writes(), 2);
}

#[test]
fn another_partys_close_during_a_cancel_is_taken_as_the_cancels_own() {
    let mut world = World::new(96);
    let (pull, _) = watched_change(&mut world);
    world.event(top::Event::Release {
        task: 51,
        root: 51,
        ending: top::ReleaseEnding::Cancelled,
        reason: None,
        entry: 10,
    });
    // The change is ending: a close before temper's own needs no hold.
    world.close(pull);
    world.run_for(60);
    world.event(top::Event::Committed { entry: 10 });
    world.run_for(60);
    assert!(!world.seen().iter().any(|request| matches!(request, top::Request::Drift { task: 51, .. })));
    world.event(top::Event::ContinueRelease { task: 51, entry: 11 });
    world.event(top::Event::Committed { entry: 11 });
    world.run_for(5);
    world.event(top::Event::ContinueRelease { task: 51, entry: 12 });
    assert!(world.seen().iter().any(|request| matches!(request, top::Request::Released { task: 51 })));
}

#[test]
fn a_live_change_watching_its_pull_reports_another_partys_close_as_drift() {
    let mut world = World::new(95);
    let (pull, name) = watched_change(&mut world);
    world.close(pull);
    world.run_for(60);
    assert!(world.seen().iter().any(|request| matches!(request,
        top::Request::Drift { task: 51, resource } if resource == &name)));
    let Some(top::Stored::Hold(hold)) = world.stored().get(&top::Key::Hold(name)) else { panic!("held change") };
    assert_eq!(hold.drift.as_ref().expect("closed pull details").change, top::DriftChange::PullClosed { number: pull });
    assert_eq!(world.writes(), 0);
}

fn wakes(world: &World) -> usize {
    world.seen().iter().filter(|request| matches!(request, top::Request::WakeProcedure { task: 51 })).count()
}

#[test]
fn a_watched_pulls_unchanged_polls_wake_its_change_no_more() {
    let mut world = World::new(97);
    watched_change(&mut world);
    let calls = world.calls();
    world.run_for(600);
    // The keep keeps polling, but the gather has nothing new to read (8.3).
    assert!(world.calls() > calls, "the pull is polled");
    assert_eq!(wakes(&world), 0);
}

#[test]
fn a_watched_pulls_ci_at_the_same_head_wakes_its_change() {
    let mut world = World::new(98);
    watched_change(&mut world);
    let head = world.branch(b"temper/51");
    // The fixture's head already passes: a failure is the change in CI.
    world.status(b"temper/51", skein_fake_forge_domain::api::Check::Failed);
    world.run_for(600);
    assert_eq!(world.branch(b"temper/51"), head);
    assert!(wakes(&world) >= 1, "CI without a delivery wakes: {:?}", world.seen());
}

#[test]
fn a_watched_pulls_mergeable_turning_true_at_the_same_head_wakes_its_change() {
    let mut world = World::new(99);
    let (pull, _) = watched_change(&mut world);
    let head = world.branch(b"temper/51");
    world.check_pull(pull, skein_lib::Duration::from_secs(120));
    world.run_for(60);
    world.take_seen();
    // The forge finishes checking: mergeable alone changes.
    world.run_for(120);
    assert_eq!(world.branch(b"temper/51"), head);
    assert!(wakes(&world) >= 1, "mergeable without a delivery wakes: {:?}", world.seen());
}

#[test]
fn an_external_move_of_a_held_branch_is_reported_as_drift() {
    let mut world = World::new(31);
    world.adopt();
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
    world.event(top::Event::StepChange {
        task: 51,
        entry: 10,
        heard: change::Heard {
            delegate: change::Status::Passed,
            effect: change::EffectResult::None,
            released: false,
            cancelled: false,
        },
        gates: Box::new([]),
        queue_repair_active: false,
        kept: false,
    });
    world.run_for(1);
    assert!(world.seen().iter().any(|event| matches!(
        event,
        top::Request::ChangeDecision { task: 51, decision: change::Decision::Hold(change::Hold::BranchMoved), .. }
    )));
    world.event(top::Event::ReleaseChange { task: 51 });
    world.take_seen();
    world.event(top::Event::StepChange {
        task: 51,
        entry: 11,
        heard: change::Heard {
            delegate: change::Status::Passed,
            effect: change::EffectResult::None,
            released: true,
            cancelled: false,
        },
        gates: Box::new([]),
        queue_repair_active: false,
        kept: false,
    });
    world.run_for(1);
    assert!(!world.seen().iter().any(|event| matches!(
        event,
        top::Request::ChangeDecision { task: 51, decision: change::Decision::Hold(change::Hold::BranchMoved), .. }
    )));
}

#[test]
fn idle_forge_call_cost_does_not_follow_old_issue_history() {
    let mut small = World::new(47);
    small.preload_history(2);
    small.adopt();
    small.event(top::Event::Names {
        task: 1,
        resources: Box::new([top::Name {
            forge: REPO.forge,
            repository: REPO.repository,
            what: top::What::Branch(Box::new([Box::from(&b"main"[..])])),
        }]),
    });
    small.run_for(60);
    let before = small.calls();
    small.run_for(60);
    let small_cost = small.calls() - before;

    let mut large = World::new(47);
    large.preload_history(20);
    large.adopt();
    large.event(top::Event::Names {
        task: 1,
        resources: Box::new([top::Name {
            forge: REPO.forge,
            repository: REPO.repository,
            what: top::What::Branch(Box::new([Box::from(&b"main"[..])])),
        }]),
    });
    large.run_for(60);
    let before = large.calls();
    large.run_for(60);
    assert_eq!(large.calls() - before, small_cost);
}

#[test]
fn a_landing_wakes_overlapping_work_and_keeps_unrelated_news() {
    let mut world = World::new(53);
    world.adopt();
    let topic = top::Topic::Landings { repository: REPO, branch: Box::from(&b"main"[..]) };
    let name = top::Name {
        forge: REPO.forge,
        repository: REPO.repository,
        what: top::What::Branch(Box::new([Box::from(&b"main"[..])])),
    };
    world.event(top::Event::Names { task: 1, resources: Box::new([name]) });
    world.event(top::Event::Subscribe {
        subscription: top::Subscriber {
            task: 1,
            number: 1,
            topic: topic.clone(),
            own_change: None,
            goal_tasks: None,
            paths: Box::new([Box::from(&b"file"[..])]),
        },
    });
    world.event(top::Event::Subscribe {
        subscription: top::Subscriber {
            task: 2,
            number: 2,
            topic,
            own_change: None,
            goal_tasks: None,
            paths: Box::new([Box::from(&b"elsewhere"[..])]),
        },
    });
    world.run_for(1);
    world.take_seen();
    world.push(b"main", b"landed");
    world.event(top::Event::Hint {
        hint: skein_forge::api::Hint {
            repository: REPO,
            change: skein_forge::api::Change::Branch(Box::from(&b"main"[..])),
            key: None,
        },
    });
    world.run_for(5);
    assert!(world.seen().iter().any(|request| matches!(
        request,
        top::Request::News { task: 1, news: top::News::Landing { .. }, class: top::Class::Wakes, .. }
    )));
    assert!(world.seen().iter().any(|request| matches!(
        request,
        top::Request::News { task: 2, news: top::News::Landing { .. }, class: top::Class::Kept, .. }
    )));
    for (task, class) in [(1, top::Class::Wakes), (2, top::Class::Kept)] {
        assert_eq!(
            world
                .seen()
                .iter()
                .filter(|request| matches!(request,
            top::Request::News { task: found, news: top::News::Landing { .. }, class: found_class, .. }
                if *found == task && *found_class == class))
                .count(),
            1
        );
    }
    world.take_seen();
    for _ in 0..2 {
        world.event(top::Event::Hint {
            hint: skein_forge::api::Hint {
                repository: REPO,
                change: skein_forge::api::Change::Branch(Box::from(&b"main"[..])),
                key: None,
            },
        });
    }
    world.run_for(5);
    assert!(!world.seen().iter().any(|request| matches!(request, top::Request::News { .. })));
}

#[test]
fn new_projection_feeds_retain_unwritten_milestones_across_a_pending_write_and_restart() {
    let mut world = World::new(81);
    world.adopt();
    let view = issues::GoalView {
        phase: issues::Phase::Waiting,
        goal: 42,
        repository: u64::from(REPO.repository),
        title: Box::from("Goal"),
        goal_text: Box::from("Plan"),
        plan: Box::new([]),
        milestones: Box::new([issues::Milestone {
            key: issues::MilestoneKey::Lifecycle { task: 43, position: 1 },
            text: Box::from("First child ended"),
        }]),
        finished: None,
    };
    world.event(top::Event::Project { entry: 1, repository: REPO, view: view.clone() });
    let next = issues::GoalView {
        milestones: Box::new([issues::Milestone {
            key: issues::MilestoneKey::TaskRevision { task: 44, revision: 1 },
            text: Box::from("Second child amended"),
        }]),
        ..view
    };
    world.event(top::Event::Project { entry: 2, repository: REPO, view: next });
    world.restart();
    world.run_for(3);
    for entry in 3..8 {
        world.event(top::Event::ProjectDesired { entry, goal: 42 });
        if world.stored().get(&top::Key::Entry(entry)).is_some() {
            world.event(top::Event::Committed { entry });
            world.run_for(3);
        }
    }
    let Some(top::Stored::Issue(row)) = world.stored().get(&top::Key::Issue(42)) else { panic!("projection kept") };
    assert_eq!(row.state.milestones.len(), 2);
    assert_eq!(world.writes(), 3, "one issue and each immutable milestone once");
    let empty = issues::GoalView {
        phase: issues::Phase::Waiting,
        goal: 42,
        repository: u64::from(REPO.repository),
        title: Box::from("Goal"),
        goal_text: Box::from("Plan"),
        plan: Box::new([]),
        milestones: Box::new([]),
        finished: None,
    };
    world.event(top::Event::Project { entry: 9, repository: REPO, view: empty });
    assert_eq!(world.writes(), 3);
    assert!(!world.seen().iter().any(|event| matches!(event, top::Request::ProjectionFailed { .. })));
}

#[test]
fn goals_classify_landings_from_their_subtree_heads_and_planned_paths_after_restart() {
    let mut world = World::new(82);
    world.adopt();
    world.produce_file(b"temper/51", b"file", b"own change");
    let pull = world.open_pull(b"temper/51", b"main");
    let head = translate::commit(world.branch(b"temper/51"));
    let mut row = change_row();
    row.pull = Some(pull);
    row.change.last_head = Some(head);
    world.event(top::Event::Change { row });
    let topic = top::Topic::Landings { repository: REPO, branch: Box::from(&b"main"[..]) };
    let name = top::Name {
        forge: REPO.forge,
        repository: REPO.repository,
        what: top::What::Branch(Box::new([Box::from(&b"main"[..])])),
    };
    world.event(top::Event::Names { task: 42, resources: Box::new([name]) });
    for (task, own, paths) in [(42, 51, Vec::new()), (43, 53, vec![Box::from(&b"elsewhere"[..])]), (44, 54, Vec::new())]
    {
        world.event(top::Event::Subscribe {
            subscription: top::Subscriber {
                task,
                number: task,
                topic: topic.clone(),
                own_change: None,
                goal_tasks: Some(Box::new([own])),
                paths: paths.into_boxed_slice(),
            },
        });
    }
    world.run_for(3);
    assert!(world.seen().iter().any(|request| matches!(request, top::Request::GoalTopic { goal: 42,
        topic: top::Topic::Ci { head: subscribed, .. } } if *subscribed == head)));
    world.restart();
    world.run_for(3);
    world.take_seen();
    world.push(b"main", b"landing");
    let hint = skein_forge::api::Hint {
        repository: REPO,
        change: skein_forge::api::Change::Branch(Box::from(&b"main"[..])),
        key: None,
    };
    world.event(top::Event::Hint { hint: hint.clone() });
    world.run_for(5);
    for (task, expected) in [(42, top::Class::Wakes), (43, top::Class::Kept), (44, top::Class::Wakes)] {
        let news: Vec<_> = world
            .seen()
            .iter()
            .filter_map(|request| {
                if let top::Request::News { task: goal, class, news: top::News::Landing { .. }, .. } = request
                    && *goal == task
                {
                    Some(*class)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(news, [expected], "each goal receives one classified landing");
    }
    world.take_seen();
    world.event(top::Event::Hint { hint: hint.clone() });
    world.run_for(3);
    assert!(
        !world
            .seen()
            .iter()
            .any(|request| matches!(request, top::Request::News { news: top::News::Landing { .. }, .. }))
    );
    world.push(b"main", b"own landing");
    let landed = translate::commit(world.branch(b"main"));
    world.event(top::Event::Restore { record: top::Stored::Landed { commit: landed, task: 51 } });
    world.event(top::Event::Hint { hint });
    world.run_for(5);
    assert!(world.seen().iter().any(|request| matches!(
        request,
        top::Request::News { task: 42, class: top::Class::Kept, news: top::News::Landing { .. }, .. }
    )));
}

#[test]
fn an_outside_push_during_a_worker_slot_holds_the_task_with_evidence() {
    let mut world = World::new(130);
    world.adopt();
    world.produce(b"temper/51");
    let name = top::Name {
        forge: REPO.forge,
        repository: REPO.repository,
        what: top::What::Branch(Box::new([Box::from(&b"temper"[..]), Box::from(&b"51"[..])])),
    };
    world.event(top::Event::Names { task: 51, resources: Box::new([name.clone()]) });
    world.event(top::Event::Hold { task: 51, resource: name.clone(), from: None });
    world.run_for(1);
    let before = temper_engine_forge_world::translate::commit(world.branch(b"temper/51"));
    world.event(top::Event::Claim { task: 51, attempt: 1, writes: Box::new([name.clone()]), holders: Box::new([]) });
    world.push_as(b"temper/51", b"file", b"outside", 2);
    let after = temper_engine_forge_world::translate::commit(world.branch(b"temper/51"));
    world.event(top::Event::Hint {
        hint: skein_forge::api::Hint {
            repository: REPO,
            change: skein_forge::api::Change::BranchMoved {
                branch: Box::from(&b"temper/51"[..]),
                head: after,
                actor: 2,
            },
            key: None,
        },
    });
    world.run_for(5);
    let Some(top::Stored::Hold(hold)) = world.stored().get(&top::Key::Hold(name)) else { panic!("affected hold") };
    assert_eq!(
        hold.drift.as_ref().expect("outside push holds task").change,
        top::DriftChange::Branch { expected: before, observed: Some(after) }
    );
    assert!(hold.drift.as_ref().expect("drift").at > Wall::EPOCH);
}

#[test]
fn a_waiting_procedure_stepped_twice_on_unchanged_facts_keeps_its_decision_and_state() {
    let (mut world, head) = reviewed_change(true, change::Freshness::Exact);
    world.produce(b"temper/50");
    let front_head = translate::commit(world.branch(b"temper/50"));
    let mut front = change_row();
    front.branch = Box::from(&b"temper/50"[..]);
    front.task = 50;
    front.change.task = 50;
    front.priority = 100;
    front.change.state = change::State::Queued { head: front_head, ready_since: Wall::EPOCH };
    world.event(top::Event::Change { row: front });
    let gates = Box::new([change::GateReport { number: 9, head, status: change::Status::Passed }]);
    let first = change_step(&mut world, 100, change::Status::Passed, gates.clone());
    let before = world.stored().get(&top::Key::Change(51)).expect("procedure state").clone();
    let second = change_step(&mut world, 101, change::Status::Passed, gates);
    assert_eq!(first, change::Decision::Wait { until: None });
    assert_eq!(second, first);
    assert_eq!(world.stored().get(&top::Key::Change(51)), Some(&before));
    assert_eq!(world.writes(), 0);
}

#[test]
fn dropping_a_completed_section_frees_it_and_its_token_can_be_reused() {
    let mut world = World::new(131);
    world.adopt();
    world.produce(b"temper/51");
    let pull = world.open_pull(b"temper/51", b"main");
    let head = translate::commit(world.branch(b"temper/51"));
    let mut row = change_row();
    row.pull = Some(pull);
    row.change.last_head = Some(head);
    world.event(top::Event::Change { row });
    let section = Token::new(96);
    let gather = || top::Event::GatherBriefHeld {
        section,
        source: top::BriefSource::Pull {
            item: top::BriefItem { repository: REPO.repository, number: pull },
            head: top::BriefCommit(head),
        },
        parts: 1,
        bytes: 512,
        max_job_bytes: 0,
    };
    world.take_seen();
    world.event(gather());
    world.run_for(5);
    assert!(world.seen().iter().any(|request| matches!(request,
        top::Request::BriefSized { section: found, size: Some(_) } if *found == section)));
    world.take_seen();
    world.event(top::Event::DropBrief { section });
    world.event(top::Event::TakeBrief { section });
    assert_eq!(world.seen(), &[top::Request::BriefTaken { section, bytes: None }]);
    world.take_seen();
    world.event(gather());
    world.run_for(5);
    assert!(world.seen().iter().any(|request| matches!(request,
        top::Request::BriefSized { section: found, size: Some(_) } if *found == section)));
    world.take_seen();
    world.event(top::Event::TakeBrief { section });
    assert!(world.seen().iter().any(|request| matches!(request,
        top::Request::BriefTaken { section: found, bytes: Some(_) } if *found == section)));
    world.take_seen();
    world.event(top::Event::TakeBrief { section });
    assert_eq!(world.seen(), &[top::Request::BriefTaken { section, bytes: None }]);
}

#[test]
fn a_lost_run_confirms_its_unreported_forward_head_by_fresh_reads() {
    let mut world = World::new(132);
    world.adopt();
    world.produce(b"temper/51");
    let name = top::Name {
        forge: REPO.forge,
        repository: REPO.repository,
        what: top::What::Branch(Box::new([Box::from(&b"temper"[..]), Box::from(&b"51"[..])])),
    };
    world.event(top::Event::Names { task: 51, resources: Box::new([name.clone()]) });
    world.event(top::Event::Hold { task: 51, resource: name.clone(), from: None });
    world.event(top::Event::Claim { task: 51, attempt: 1, writes: Box::new([name.clone()]), holders: Box::new([]) });
    world.event(top::Event::Answered {
        task: 51,
        attempt: 1,
        pushed: Box::new([(name.clone(), translate::commit(world.branch(b"temper/51")))]),
    });
    world.run_for(1);
    world.event(top::Event::Claim { task: 51, attempt: 2, writes: Box::new([name.clone()]), holders: Box::new([]) });
    world.push(b"temper/51", b"unreported work");
    let head = translate::commit(world.branch(b"temper/51"));
    let calls = world.calls();
    world.event(top::Event::Lost { task: 51, attempt: 2 });
    world.run_for(5);
    assert!(world.calls() >= calls + 2, "head read and ancestry comparison");
    let Some(top::Stored::Hold(hold)) = world.stored().get(&top::Key::Hold(name.clone())) else {
        panic!("hold retained")
    };
    assert_eq!(hold.writer, None);
    assert_eq!(hold.drift, None);
    let Some(top::Stored::BranchHead(recorded)) = world.stored().get(&top::Key::BranchHead(name)) else {
        panic!("head confirmed")
    };
    assert_eq!(recorded.commit, head);
    assert!(recorded.owned);
}

#[test]
fn a_lost_claim_before_its_first_push_releases_the_absent_branch_writer() {
    let mut world = World::new(133);
    world.adopt();
    let name = top::Name {
        forge: REPO.forge,
        repository: REPO.repository,
        what: top::What::Branch(Box::new([b"temper".as_slice().into(), b"51".as_slice().into()])),
    };
    world.event(top::Event::Names { task: 51, resources: Box::new([name.clone()]) });
    world.event(top::Event::Hold { task: 51, resource: name.clone(), from: None });
    world.event(top::Event::Claim { task: 51, attempt: 1, writes: Box::new([name.clone()]), holders: Box::new([]) });
    world.take_seen();
    world.event(top::Event::Lost { task: 51, attempt: 1 });
    world.run_for(5);
    let Some(top::Stored::Hold(hold)) = world.stored().get(&top::Key::Hold(name.clone())) else {
        panic!("hold retained")
    };
    assert_eq!(hold.writer, None);
    assert_eq!(hold.drift, None);
    assert!(!world.stored().contains_key(&top::Key::BranchHead(name)));
    assert!(!world.seen().iter().any(|request| matches!(request, top::Request::Drift { .. })));
}

#[test]
fn a_lost_writer_restarts_its_unfinished_fresh_read_before_releasing() {
    let mut world = World::new(137);
    world.adopt();
    let name = top::Name {
        forge: REPO.forge,
        repository: REPO.repository,
        what: top::What::Branch(Box::new([b"temper".as_slice().into(), b"53".as_slice().into()])),
    };
    world.event(top::Event::Names { task: 53, resources: Box::new([name.clone()]) });
    world.event(top::Event::Hold { task: 53, resource: name.clone(), from: None });
    world.event(top::Event::Claim { task: 53, attempt: 1, writes: Box::new([name.clone()]), holders: Box::new([]) });
    world.produce(b"temper/53");
    world.slow_calls(skein_lib::Duration::from_secs(5));
    world.event(top::Event::Lost { task: 53, attempt: 1 });
    let Some(top::Stored::Hold(hold)) = world.stored().get(&top::Key::Hold(name.clone())) else {
        panic!("hold retained")
    };
    assert_eq!(hold.writer, Some(top::Writer::Lost { task: 53, attempt: 1 }));
    world.restart();
    world.run_for(12);
    let Some(top::Stored::Hold(hold)) = world.stored().get(&top::Key::Hold(name.clone())) else {
        panic!("hold retained")
    };
    assert_eq!(hold.writer, None);
    let Some(top::Stored::BranchHead(head)) = world.stored().get(&top::Key::BranchHead(name)) else {
        panic!("fresh head retained")
    };
    assert!(head.owned);
}

#[test]
fn a_repository_with_222_branches_and_two_recorded_prefix_branches_is_adopted() {
    let mut world = World::with_branches(4, 222);
    world.adopt();
    for (task, suffix) in [(9, b"one".as_slice()), (10, b"two".as_slice())] {
        let branch = format!("temper/{}", std::str::from_utf8(suffix).expect("ASCII fixture suffix"));
        world.produce(branch.as_bytes());
        let name = top::Name {
            forge: REPO.forge,
            repository: REPO.repository,
            what: top::What::Branch(Box::new([Box::from(&b"temper"[..]), Box::from(suffix)])),
        };
        world.event(top::Event::Names { task, resources: Box::new([name.clone()]) });
        world.event(top::Event::Hold { task, resource: name.clone(), from: None });
        world.event(top::Event::Claim {
            task,
            attempt: 1,
            writes: Box::new([name.clone()]),
            holders: Box::new([task]),
        });
        world.event(top::Event::Answered {
            task,
            attempt: 1,
            pushed: Box::new([(name.clone(), translate::commit(world.branch(branch.as_bytes())))]),
        });
        world.run_for(1);
        let Some(top::Stored::BranchHead(head)) = world.stored().get(&top::Key::BranchHead(name)) else {
            panic!("writer branch tip retained")
        };
        assert!(head.owned);
    }
    world.preload_branches(219);
    drop(world.take_seen());
    world.adopt();
    assert!(
        world.seen().iter().any(|request| matches!(request, top::Request::Adopted { result: Ok(_), .. })),
        "{:?}",
        world.seen()
    );
    assert!(!world.seen().iter().any(|request| matches!(request, top::Request::Adopted { result: Err(_), .. })));
}
