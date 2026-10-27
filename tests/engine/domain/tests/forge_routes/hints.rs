//! A hint invalidates the procedures whose fresh facts it can concern.
use skein_forge::api::{Change, Hint, Op, Read};
use temper_engine_domain_forge as forge;
use temper_engine_domain_world::forge_routes::{hint_step, hint_world};
use temper_engine_forge_world::{REPO, World, translate};

fn hint(world: &mut World, change: Change) -> Vec<u64> {
    world.take_seen();
    world.event(forge::Event::Hint { hint: Hint { repository: REPO, change, key: None } });
    world.run_for(1);
    world
        .take_seen()
        .iter()
        .filter_map(|request| if let forge::Request::WakeProcedure { task } = request { Some(*task) } else { None })
        .collect()
}

fn reads_for(world: &World, task: u64) -> Vec<Op> {
    let Some(forge::Stored::Change(row)) = world.stored().get(&forge::Key::Change(task)) else {
        panic!("live change {task}")
    };
    let base = translate::commit(world.branch(&row.base));
    let head = translate::commit(world.branch(&row.branch));
    vec![
        Op::Read(Read::Branch { branch: row.branch.clone() }),
        Op::Read(Read::Branch { branch: row.base.clone() }),
        Op::Read(Read::PullFor { head: row.branch.clone(), base: row.base.clone() }),
        Op::Read(Read::Checks { commit: head }),
        Op::Read(Read::Checks { commit: base }),
        Op::Read(Read::Compare { before: base, after: head }),
        Op::Read(Read::Reviews { number: row.pull.expect("opened pull"), page: 1 }),
    ]
}

#[test]
fn a_repository_hint_wakes_only_the_changes_it_concerns() {
    let mut world = hint_world();
    for (change, expected) in [
        (Change::Branch(b"temper/a".as_slice().into()), vec![51]),
        (
            Change::BranchMoved {
                branch: b"release".as_slice().into(),
                head: translate::commit(world.branch(b"release")),
                actor: 2,
            },
            vec![52],
        ),
        (Change::Branch(b"unmatched-or-renamed".as_slice().into()), vec![51, 52]),
        (Change::Branch(Box::new([])), vec![51, 52]),
        (Change::Commit(translate::commit(world.branch(b"temper/a"))), vec![51]),
        (Change::Commit(translate::commit(world.branch(b"release"))), vec![52]),
        (Change::Item(1), vec![51]),
        (Change::Item(2), vec![52]),
    ] {
        let wakes = hint(&mut world, change.clone());
        assert_eq!(wakes, expected, "{change:?}");
        assert!(world.take_calls().is_empty(), "no resources held by this scripted parent");
        for task in wakes {
            let expected = reads_for(&world, task);
            hint_step(&mut world, task, 100 + task);
            assert_eq!(world.take_calls(), expected, "fresh reads for {task}, {change:?}");
        }
    }
}

#[test]
fn only_a_relevant_hint_restarts_an_unfinished_fresh_read_chain() {
    use skein_lib::Duration;
    use temper_engine_domain_forge_change as change;
    for (branch, expected_wake, passes) in [(b"temper/a".as_slice(), 51, 1), (b"release".as_slice(), 52, 2)] {
        let mut world = hint_world();
        let expected: Vec<_> = (0..passes).flat_map(|_| reads_for(&world, 52)).collect();
        world.slow_calls(Duration::from_millis(200));
        let step = |entry| forge::Event::StepChange {
            task: 52,
            entry,
            heard: change::Heard {
                lacking: None,
                spend: None,
                delegate: change::Status::Unknown,
                effect: change::EffectResult::None,
                released: false,
                cancelled: false,
            },
            gates: Box::new([]),
            queue_repair_active: false,
            kept: true,
        };
        world.event(step(150));
        assert_eq!(hint(&mut world, Change::Branch(branch.into())), [expected_wake]);
        if expected_wake == 52 {
            world.event(step(151));
        }
        world.run_for(3);
        assert_eq!(world.take_calls(), expected, "hint on {}", String::from_utf8_lossy(branch));
    }
}

#[test]
fn both_fresh_base_facts_remain_dependencies_when_the_pull_and_branch_disagree() {
    use skein_lib::{Duration, Wall};
    use temper_engine_domain_forge_change as change;
    let mut world = hint_world();
    let before = translate::commit(world.branch(b"release"));
    world.slow_calls(Duration::from_secs(1));
    world.event(forge::Event::StepChange {
        task: 52,
        entry: 152,
        heard: change::Heard {
            lacking: None,
            spend: None,
            delegate: change::Status::Unknown,
            effect: change::EffectResult::None,
            released: false,
            cancelled: false,
        },
        gates: Box::new([]),
        queue_repair_active: false,
        kept: true,
    });
    // The base branch's read has been sent and snapshots the old tip, but
    // the pull read has not been sent. Move the base between those reads.
    world.run_to(Wall::from_nanos(world.wall().as_nanos() + Duration::from_millis(1500).as_nanos()));
    world.push(b"release", b"base moved between reads");
    let after = translate::commit(world.branch(b"release"));
    assert_ne!(before, after);
    world.run_for(6);
    let calls = world.take_calls();
    assert!(calls.contains(&Op::Read(Read::Checks { commit: before })), "CI used the branch read's tip");
    assert_eq!(hint(&mut world, Change::Commit(before)), [52], "the decision's CI/compare base");
    assert_eq!(hint(&mut world, Change::Commit(after)), [52], "the pull's newer base");
}

#[test]
fn a_named_issue_still_gets_its_hint_reads_and_wakes_its_change() {
    let mut world = hint_world();
    let issue = world.issue(b"dependency");
    world.event(forge::Event::Names {
        task: 52,
        resources: Box::new([forge::Name {
            forge: REPO.forge,
            repository: REPO.repository,
            what: forge::What::Issue(issue),
        }]),
    });
    world.run_for(1);
    world.take_calls();
    assert_eq!(hint(&mut world, Change::Item(issue)), [52]);
    let calls = world.take_calls();
    assert!(
        calls.iter().any(|op| matches!(op, Op::Read(Read::Item { number, .. }) if *number == issue)),
        "hint keep reads: {calls:?}"
    );
    // An identical second hint must read the held item again too.
    world.run_for(1);
    world.take_calls();
    assert_eq!(hint(&mut world, Change::Item(issue)), [52]);
    let calls = world.take_calls();
    assert!(
        calls.iter().any(|op| matches!(op, Op::Read(Read::Item { number, .. }) if *number == issue)),
        "repeated hint keep reads: {calls:?}"
    );
}

#[test]
fn subscribed_ci_and_unknown_change_facts_keep_their_wakes() {
    let mut world = hint_world();
    let head = translate::commit(world.branch(b"temper/b"));
    world.event(forge::Event::Subscribe {
        subscription: forge::Subscriber {
            task: 51,
            number: 1,
            topic: forge::Topic::Ci { repository: REPO, head },
            own_change: None,
            goal_tasks: None,
            paths: Box::new([]),
        },
    });
    world.run_for(1);
    assert_eq!(hint(&mut world, Change::Commit(head)), [51, 52]);

    let Some(forge::Stored::Change(mut row)) = world.stored().get(&forge::Key::Change(51)).cloned() else {
        panic!("first live change")
    };
    row.task = 53;
    row.change.task = 53;
    row.branch = b"temper/not-yet-pushed".as_slice().into();
    row.pull = None;
    row.change.last_head = None;
    row.change.state = temper_engine_domain_forge_change::State::Producing { requested: false };
    world.event(forge::Event::Change { row });
    assert_eq!(hint(&mut world, Change::Item(2)), [52, 53], "unknown pull cannot exclude its review hint");
    assert_eq!(hint(&mut world, Change::Commit(head)), [51, 52, 53], "unknown head cannot exclude its CI hint");
    assert_eq!(hint(&mut world, Change::Item(0)), [51, 52, 53], "unnamed item is repository-wide");
    assert_eq!(hint(&mut world, Change::Commit([0; 32])), [51, 52, 53], "unnamed commit is repository-wide");
}

#[test]
#[ignore = "opt-in before/after request profile of two live change procedures"]
fn two_change_hint_request_profile() {
    use temper_engine_domain_forge_change as change;
    let mut world = hint_world();
    for change in [
        Change::Branch(b"temper/a".as_slice().into()),
        Change::Branch(b"release".as_slice().into()),
        Change::Branch(b"unmatched-or-renamed".as_slice().into()),
    ] {
        let wakes = hint(&mut world, change.clone());
        println!("{change:?}: wakes {wakes:?}");
        for task in wakes {
            hint_step(&mut world, task, 100 + task);
        }
        for call in world.take_calls() {
            println!("{call:?}");
        }
    }
    for task in [51, 52] {
        let Some(forge::Stored::Change(row)) = world.stored().get(&forge::Key::Change(task)) else {
            panic!("change {task}")
        };
        let gates = Box::new([change::GateReport {
            number: 9,
            head: row.change.last_head.expect("pushed head"),
            status: change::Status::Passed,
        }]);
        assert_eq!(
            temper_engine_forge_world::changes::change_step_task(
                &mut world,
                task,
                200 + task,
                change::Status::Unknown,
                gates.clone(),
                false,
                false,
            ),
            change::Decision::Ready
        );
        assert!(matches!(
            temper_engine_forge_world::changes::change_step_task(
                &mut world,
                task,
                300 + task,
                change::Status::Unknown,
                gates,
                false,
                false,
            ),
            change::Decision::Effect(change::Effect::Merge { .. })
        ));
        assert!(
            world
                .stored()
                .iter()
                .any(|(_, row)| matches!(row, forge::Stored::Landed { task: landed, .. } if *landed == task))
        );
        println!("landed {task}");
        for call in world.take_calls() {
            println!("{call:?}");
        }
    }
}
