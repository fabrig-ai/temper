//! A fresh-read decision and its jig admission share the procedure's state.
use super::*;
use temper_engine_domain_forge_change as policy;
use temper_engine_domain_world::forge_routes::ReadPhase;

fn change_row(world: &World, task: u64) -> &forge_top::ChangeRow {
    world
        .store
        .rows
        .values()
        .find_map(|stored| match stored {
            Record::Forge { row, .. } => match row.as_ref() {
                forge_top::Stored::Change(row) if row.task == task => Some(row),
                _ => None,
            },
            Record::Core(_) => None,
        })
        .expect("durable change")
}

fn checking() -> (World, u64, u64) {
    let (mut world, _chat, producer, branch) = change_world_with_policy(true, 1000, None, true, false, false, false);
    let tasks::Party::Task(change) = live_task(&world, producer.task).requester else {
        panic!("producer belongs to change")
    };
    world.external(2, raw::Op::Git(raw::Git::Create { branch: branch.clone(), commit: 1 }));
    let head = fake::advance(&mut world.fake, &world.fake_env, b"org/repo", &branch, b"file", b"review me", 1)
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
    for _ in 0..200 {
        world.tick();
        if world.store.rows.values().any(|stored| {
            matches!(stored,
            Record::Forge { row, .. } if matches!(row.as_ref(), forge_top::Stored::Change(row)
                if row.task == change && matches!(row.change.state, policy::State::Checking { .. })))
        }) {
            break;
        }
    }
    assert!(matches!(change_row(&world, change).change.state, policy::State::Checking { .. }));
    assert_eq!(world.assigned.len(), 2, "no reviewer before CI passes");
    (world, change, head)
}

fn pause_checking(world: &mut World) {
    world.held_read_phase = ReadPhase::Checking;
    world.held_reads = Some(Vec::new());
    world.env.now = world.env.now.saturating_add(Duration::from_secs(30));
    for _ in 0..100 {
        world.tick();
        if world.held_reads.as_ref().is_some_and(|reads| !reads.is_empty()) {
            break;
        }
    }
    assert!(world.held_reads.as_ref().is_some_and(|reads| !reads.is_empty()), "checking read paused");
}

fn finish_reads(world: &mut World) {
    for read in world.held_reads.take().expect("paused reads") {
        world.out.push(read);
    }
    world.held_read_phase = ReadPhase::Queued;
    world.collect();
    for _ in 0..80 {
        world.tick();
    }
}

fn pass_ci(world: &mut World, head: u64) {
    for commit in [1, head] {
        world.external(
            1,
            raw::Op::Write(raw::Write::Status {
                commit,
                context: b"build".as_slice().into(),
                state: raw::Check::Passed,
            }),
        );
    }
}

fn owner(world: &mut World, key: u8, ask: people::Ask) {
    world.send(engine::Event::Party(engine::PartyInput::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(u64::from(key))),
        sign_in: world.signed_in.expect("owner session"),
        key: [key; 16],
        ask,
    }));
    for _ in 0..40 {
        world.tick();
    }
}

fn release(world: &mut World, change: u64) {
    let revision = live_task(world, change).revisions.hold;
    owner(world, 182, people::Ask::Release { project: 1, task: change, revision, again: None });
}

fn reviewer(world: &mut World, change: u64) -> engine::Assignment {
    for _ in 0..200 {
        world.tick();
        if world.assigned.iter().any(|run| run.charter == 4) {
            break;
        }
    }
    let reviewers: Vec<_> = world.assigned.iter().filter(|run| run.charter == 4).collect();
    assert_eq!(
        reviewers.len(),
        1,
        "exactly one reviewer: phase={:?}, change={:?}, assignments={:?}",
        live_task(world, change).phase,
        change_row(world, change).change,
        world.assigned.iter().map(|run| (run.task, run.charter)).collect::<Vec<_>>()
    );
    let gate = reviewers[0].clone();
    assert!(matches!(change_row(world, change).delegate,
        Some((child, policy::Delegate::Gate { head, .. })) if child == gate.task
            && Some(head) == change_row(world, change).change.last_head));
    gate
}

fn approve(world: &mut World, change: u64, gate: &engine::Assignment) {
    let event = world.protocol.answer(
        Token::new(7),
        gate.task,
        gate.attempt,
        5,
        tasks::End::Finished {
            result: tasks::TaskResult::Verdict { code: 1, fields: Box::new([]), words: b"approved".as_slice().into() },
            cancel_delegates: false,
        },
        engine::Parts::none(),
    );
    world.send(event);
    for _ in 0..200 {
        world.tick();
        if matches!(change_row(world, change).change.state, policy::State::Landed { .. }) {
            break;
        }
    }
    assert!(matches!(change_row(world, change).change.state, policy::State::Landed { .. }));
    assert_eq!(world.merges, 1);
}

#[test]
fn a_change_stopped_mid_read_asks_its_review_once_released() {
    let (mut world, change, head) = checking();
    pause_checking(&mut world);
    let prior = change_row(&world, change).change.clone();
    owner(&mut world, 181, people::Ask::Stop { project: 1, task: change });
    assert!(matches!(live_task(&world, change).phase, tasks::Phase::Held { .. }));
    pass_ci(&mut world, head);
    finish_reads(&mut world);
    assert_eq!(world.assigned.len(), 2, "held decision makes no reviewer");
    assert_eq!(change_row(&world, change).change, prior, "the whole procedure state stays put");
    release(&mut world, change);
    let gate = reviewer(&mut world, change);
    approve(&mut world, change, &gate);
}

#[test]
fn a_change_whose_review_step_is_refused_asks_again_at_its_next_step() {
    let (mut world, change, head) = checking();
    pause_checking(&mut world);
    let prior = change_row(&world, change).change.clone();
    // Fill the change's actual delegate room through jig, without changing
    // configured limits. These report tasks occupy no forge delegate slot.
    let chat = world.assigned[0].clone();
    for position in 0..world.env.limits.tasks.delegates {
        let mut member = temper_engine_domain_world::fixtures::report_delegate(b"occupy child room", Box::new([]));
        member.executor = tasks::Executor::Agent { charter: 2 };
        member.authority.tools = tasks::Tools(0);
        member.authority.budget.spend = 6;
        let step = live_task(&world, change).attempt + 1;
        world.send(engine::Event::Party(engine::PartyInput::ProcedureStep {
            task: change,
            step,
            connector: 0,
            code: 2,
            decision: jig_domain_core::ProcedureStep {
                batch: Box::new([member]),
                ..jig_domain_core::ProcedureStep::new(jig_domain_core::StepEnd::Wait)
            },
        }));
        for _ in 0..20 {
            world.tick();
        }
        assert_eq!(
            live_task(&world, change).delegates.len(),
            position as usize + 1,
            "occupant admitted: {:?}",
            std::iter::from_fn(|| world.root.pop_fact()).collect::<Vec<_>>()
        );
        // Wake a fresh step while the read chain is still paused.
        let event = world.protocol.call(
            Token::new(7),
            chat.task,
            chat.attempt,
            94 + u64::from(position),
            engine::Call {
                completion: 1,
                position: 2 + position,
                tool: engine::Tool::Message {
                    target: change,
                    form: engine::MessageForm::Words,
                    words: b"check again".as_slice().into(),
                },
            },
        );
        world.send(event);
        for _ in 0..20 {
            world.tick();
        }
    }
    let occupants = live_task(&world, change).delegates.to_vec();
    assert_eq!(occupants.len(), world.env.limits.tasks.delegates as usize, "delegate room full");
    pass_ci(&mut world, head);
    finish_reads(&mut world);
    assert!(world.assigned.iter().all(|run| run.charter != 4));
    assert_eq!(change_row(&world, change).change, prior);
    assert!(
        std::iter::from_fn(|| world.root.pop_fact()).any(|fact| matches!(fact,
        engine::Fact::StepRefused { task, problem, .. } if task == change
            && problem.why == tasks::Refusal::Delegates)),
        "jig refused the review batch"
    );
    let run = world
        .assigned
        .iter()
        .find(|run| occupants.contains(&run.task))
        .expect("one occupant uses the worker slot")
        .clone();
    let event = world.protocol.answer(
        Token::new(7),
        run.task,
        run.attempt,
        5,
        tasks::End::Finished {
            result: tasks::TaskResult::Report { words: b"room free".as_slice().into() },
            cancel_delegates: false,
        },
        engine::Parts::none(),
    );
    world.send(event);
    for _ in 0..100 {
        world.tick();
        if change_row(&world, change).delegate.is_some() {
            break;
        }
    }
    let Some((child, policy::Delegate::Gate { head: reviewed, .. })) = change_row(&world, change).delegate else {
        panic!("next step makes its reviewer");
    };
    assert_eq!(Some(reviewed), change_row(&world, change).change.last_head);
    assert_eq!(live_task(&world, child).executor, tasks::Executor::Agent { charter: 4 });
    assert_eq!(
        live_task(&world, change)
            .delegates
            .iter()
            .filter(|task| live_task(&world, **task).executor == tasks::Executor::Agent { charter: 4 })
            .count(),
        1,
        "exactly one reviewer task, even while other occupants use the worker"
    );
}

#[test]
fn a_reviewer_made_before_the_stop_is_kept_and_not_asked_again() {
    let (mut world, change, head) = checking();
    pass_ci(&mut world, head);
    world.env.now = world.env.now.saturating_add(Duration::from_secs(30));
    let gate = reviewer(&mut world, change);
    let delegate = change_row(&world, change).delegate;
    owner(&mut world, 181, people::Ask::Stop { project: 1, task: change });
    assert!(matches!(live_task(&world, gate.task).phase, tasks::Phase::Held { .. }));
    release(&mut world, change);
    for _ in 0..80 {
        world.tick();
    }
    assert_eq!(change_row(&world, change).delegate, delegate);
    assert!(world.assigned.iter().all(|assignment| assignment.task != change));
    let reviewers: Vec<_> = world.assigned.iter().skip(2).map(|assignment| assignment.task).collect();
    assert!(reviewers.iter().all(|task| *task == gate.task), "only the same reviewer resumes");
}

#[test]
fn a_review_and_its_change_survive_a_restart_at_the_admission_commit() {
    let (mut world, change, head) = checking();
    pass_ci(&mut world, head);
    world.env.now = world.env.now.saturating_add(Duration::from_secs(30));
    let mut admitted = None;
    for _ in 0..200 {
        if let Some(event) = world.events.pop_front() {
            world.send(event);
        } else {
            world.tick();
        }
        for (_, writes) in &world.store.pending {
            let child = writes.iter().find_map(|write| match write {
                engine::Write::Save(Record::Forge { row, .. }) => match row.as_ref() {
                    forge_top::Stored::Change(row) if row.task == change => match row.delegate {
                        Some((child, policy::Delegate::Gate { .. })) => Some(child),
                        _ => None,
                    },
                    _ => None,
                },
                _ => None,
            });
            if let Some(child) = child {
                assert!(
                    writes.iter().any(|write| matches!(write,
                    engine::Write::Save(Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Live(row))))
                        if row.number == change && row.delegates.contains(&child))),
                    "parent admission shares the change save"
                );
                assert!(
                    writes.iter().any(|write| matches!(write,
                    engine::Write::Save(Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Live(row))))
                        if row.number == child)),
                    "new reviewer shares the change save"
                );
                admitted = Some(child);
            }
        }
        if admitted.is_some() {
            break;
        }
    }
    let child = admitted.expect("review admission commit");
    // Persist the decision and lose its terminal before any worker sees the
    // new reviewer. Only the durable transaction crosses this process cut.
    while !world.store.pending.is_empty() {
        world.store.apply();
    }
    assert_eq!(world.assigned.len(), 2, "reviewer has not been delivered");
    world.restart(true, true);
    // Only the chat is still hosted: the producer finished, and the new
    // reviewer never crossed the lost commit terminal.
    world.events.pop_back().expect("fixture host snapshot");
    let chat = &world.assigned[0];
    world.events.push_back(protocol::hello_slots(
        Token::new(7),
        2,
        Box::new([protocol::hosting(chat.task, chat.attempt, host::Phase::Active)]),
    ));
    let gate = reviewer(&mut world, change);
    assert_eq!(gate.task, child, "restart keeps the admitted reviewer identity");
    assert!(world.root.ready(), "cold restore admitted both records");
}
