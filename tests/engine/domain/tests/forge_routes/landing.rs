//! A chat's change runs through the root's deployment-funded landing task.
use super::*;
use skein_forge::api;

fn task_row(world: &World, task: u64) -> Option<&tasks::TaskRecord> {
    world.store.rows.values().find_map(|record| match record {
        Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Live(row) | tasks::Stored::Ended(row)))
            if row.number == task =>
        {
            Some(row.as_ref())
        }
        _ => None,
    })
}

fn produced_turn_change(landing_members: Option<u32>) -> (World, u64) {
    let mut world = World::configured_limits(
        true,
        true,
        false,
        1000,
        Gate::default(),
        None,
        StoryLimits { turn: true, landing_members, ..StoryLimits::default() },
    );
    world.adopt();
    world.send(protocol::hello_slots(Token::new(7), 2, Box::new([])));
    let ask = adoption_chat(&world);
    let chat = assigned_chat(&mut world, 92, ask);
    let engine::CallAnswer::Delegated(made) =
        change_tool::change(&mut world, &chat, 93, None, b"Small fix\nwith detail")
    else {
        panic!("change delegated")
    };
    let change = made[0].task;
    world.until(Until::SecondAssignment);
    let producer = world.assigned[1].clone();
    let branch = producer.workspace.repositories[0].push.as_ref().expect("producer branch").clone();
    assert!(branch.starts_with(b"temper/"));
    world.external(1, raw::Op::Git(raw::Git::Create { branch: branch.clone(), commit: 1 }));
    fake::advance(&mut world.fake, &world.fake_env, b"org/repo", &branch, b"file", b"fixed", 1)
        .expect("producer pushes");
    fake::advance(&mut world.fake, &world.fake_env, b"org/repo", b"main", b"other", b"main moved", 2)
        .expect("main moves under change");
    let answer = world.protocol.answer(
        Token::new(7),
        producer.task,
        producer.attempt,
        5,
        tasks::End::Finished {
            result: tasks::TaskResult::Change {
                connector: 0,
                kind: 2,
                resource: forge_top::tag(forge_world::REPO.forge, forge_world::REPO.repository),
                words: b"pushed".as_slice().into(),
            },
            cancel_delegates: false,
        },
        engine::Parts::none(),
    );
    world.send(answer);
    (world, change)
}

#[test]
fn a_chats_change_lands_by_turn() {
    let (mut world, change) = produced_turn_change(None);
    let mut landing = None;
    for _ in 0..600 {
        world.tick();
        for record in world.store.rows.values() {
            if let Record::Forge { row } = record
                && let forge_top::Stored::Landing(row) = row.as_ref()
            {
                landing = Some(row.landing.task);
            }
        }
        if landing.is_some_and(|landing| {
            task_row(&world, landing).is_some_and(|row| matches!(row.phase, tasks::Phase::Ended(_)))
        }) && task_row(&world, change).is_some_and(|row| matches!(row.phase, tasks::Phase::Ended(_)))
        {
            break;
        }
    }
    let landing = landing.unwrap_or_else(|| {
        panic!(
            "landing made: {:?}",
            world.store.rows.values().filter(|record| matches!(record, Record::Forge { .. })).collect::<Vec<_>>()
        )
    });
    let row = task_row(&world, landing).expect("landing task");
    assert_eq!(row.executor, tasks::Executor::Procedure { connector: 0, code: 3 });
    assert_eq!(row.requester, tasks::Party::Deployment { project: 1 });
    assert_eq!(row.spec.words.as_ref(), b"Land Small fix");
    assert!(row.dependencies.is_empty());
    for kind in [3, 4, 6, 9, 12, 16, 17] {
        assert!(row.authority.grants.iter().any(|grant| grant.kind == kind), "landing granted kind {kind}");
    }
    assert!(!row.authority.grants.iter().any(|grant| grant.kind == 5 || grant.kind == 15));
    let tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Report { words })) = &row.phase else {
        panic!("landing done: {:?}; writes={:?}", row.phase, world.writes);
    };
    let report = words.clone();
    let change_row = task_row(&world, change).expect("change task");
    let tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Change { words, .. })) = &change_row.phase else {
        panic!("change done: {:?}", change_row.phase)
    };
    let change_result = words.clone();
    let raw::Answer::Commit(main) = world.external(1, raw::Op::Git(raw::Git::Fetch { want: raw::Want::Default }))
    else {
        panic!("main tip")
    };
    let main = translate::commit(main);
    let mut hex = String::new();
    for byte in &main[..20] {
        write!(&mut hex, "{byte:02x}").expect("commit hex");
    }
    assert_eq!(report.as_ref(), format!("org/repo {hex}\n").as_bytes());
    assert!(std::str::from_utf8(&change_result).expect("change result text").contains(&hex));
    assert_eq!(
        world
            .writes
            .iter()
            .filter(|write| matches!(write, api::Write::Update { style: api::UpdateStyle::Rebase, .. }))
            .count(),
        1
    );
    assert_eq!(
        world
            .writes
            .iter()
            .filter(|write| matches!(write, api::Write::Dispatch { workflow, .. } if workflow.as_ref() == b"gate.yml"))
            .count(),
        1
    );
    assert_eq!(world.writes.iter().filter(|write| matches!(write, api::Write::Status { commit, context, .. } if *commit == main && context.as_ref() == b"temper/gate/gate")).count(), 2);
    assert_eq!(world.writes.iter().filter(|write| matches!(write, api::Write::Merge { head, how: api::MergeHow::FastForwardOnly, .. } if *head == main)).count(), 1);
    assert_eq!(world.merges, 1);
}

#[test]
fn a_goal_is_kept_on_its_own_changes_landing_by_turn() {
    let world = World::configured_limits(
        true,
        true,
        false,
        1000,
        Gate::default(),
        None,
        StoryLimits { turn: true, ..StoryLimits::default() },
    );
    let (mut world, goal, producer, branch) = change_world_in(world, Change { tracked: true, ..Change::default() });
    let owner = live_task(&world, goal.task);
    assert_ne!(owner.root, goal.task, "goal is delegated");
    let tasks::Party::Task(change) = live_task(&world, producer.task).requester else {
        panic!("producer belongs to change")
    };
    assert_eq!(live_task(&world, change).requester, tasks::Party::Task(goal.task));
    let subscription = world.store.rows.values().find_map(|record| match record {
        Record::Forge { row } => match row.as_ref() {
            forge_top::Stored::Subscription(row) if row.task == goal.task
                && matches!(&row.topic, forge_top::Topic::Landings { branch, .. } if branch.as_ref() == b"main") => Some(row.number),
            _ => None,
        },
        Record::Core(_) => None,
    }).expect("goal's landing subscription");
    let event =
        world.protocol.answer(Token::new(7), goal.task, goal.attempt, 0, tasks::End::Parked, engine::Parts::none());
    world.send(event);
    for _ in 0..30 {
        world.tick();
    }
    world.external(1, raw::Op::Git(raw::Git::Create { branch: branch.clone(), commit: 1 }));
    fake::advance(&mut world.fake, &world.fake_env, b"org/repo", &branch, b"file", b"fixed", 1)
        .expect("producer pushes");
    let event = world.protocol.answer(
        Token::new(7),
        producer.task,
        producer.attempt,
        5,
        tasks::End::Finished {
            result: tasks::TaskResult::Change {
                connector: 0,
                kind: 2,
                resource: forge_top::tag(forge_world::REPO.forge, forge_world::REPO.repository),
                words: b"pushed".as_slice().into(),
            },
            cancel_delegates: false,
        },
        engine::Parts::none(),
    );
    world.send(event);
    for _ in 0..600 {
        world.tick();
    }
    assert_eq!(world.merges, 1);
    let inbox = &live_task(&world, goal.task).inbox;
    let news: Vec<_> = inbox
        .iter()
        .filter_map(|word| match word.kind {
            tasks::MessageKind::News { subscription: number, class, .. } if number == subscription => Some(class),
            _ => None,
        })
        .collect();
    assert_eq!(news, [tasks::NewsClass::Kept], "own landing never wakes goal");
    assert!(
        inbox
            .iter()
            .any(|word| word.from == tasks::Party::Task(change) && matches!(word.kind, tasks::MessageKind::Result(_))),
        "change result reaches goal"
    );
    assert!(task_row(&world, change).is_some_and(|row| matches!(
        row.phase,
        tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Change { .. }))
    )));
}

#[test]
fn a_ready_change_waits_when_landing_room_is_full() {
    let (mut world, change) = produced_turn_change(Some(0));
    for _ in 0..300 {
        world.tick();
    }
    assert!(task_row(&world, change).is_some_and(|row| matches!(row.phase, tasks::Phase::Active(_))));
    let row = world
        .store
        .rows
        .values()
        .find_map(|record| match record {
            Record::Forge { row } => match row.as_ref() {
                forge_top::Stored::Change(row) if row.task == change => Some(row),
                _ => None,
            },
            Record::Core(_) => None,
        })
        .expect("change remains");
    assert_eq!(row.change.landing, None);
    assert!(matches!(row.change.state, temper_engine_domain_forge_change::State::Ready { .. }));
    assert!(world.store.rows.values().all(
        |record| !matches!(record, Record::Forge { row } if matches!(row.as_ref(), forge_top::Stored::Landing(_)))
    ));
    assert_eq!(world.merges, 0);
}
