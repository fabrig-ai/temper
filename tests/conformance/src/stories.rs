//! Temper's task and forge stories, judged outside the engine after every cut.
use crate::{Config, Temper, observations::Store, scenarios, send};
use jig_conformance::{
    Application, Cut, Harness, Outcome,
    referee::{Promise, Violation},
    scenarios::{Action, ScenarioApplication},
};
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_fake_forge_domain::Observation;
use skein_fake_forge_domain::api as raw;
use skein_forge::api;
use temper_engine_domain as engine;
use temper_engine_domain::{self as root};
use temper_engine_domain_forge as forge;
use temper_engine_domain_forge_change as change;

type World = Harness<Temper>;
/// Stories from temper session 05, plus uncertainty for native issue creation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Story {
    ReviewedChange,
    RepairCi,
    GoalIssue,
    OtherHand,
    LostMerge,
    PersonWords,
    IssueCreation,
}
pub const ALL: [Story; 7] = [
    Story::ReviewedChange,
    Story::RepairCi,
    Story::GoalIssue,
    Story::OtherHand,
    Story::LostMerge,
    Story::PersonWords,
    Story::IssueCreation,
];

fn require(holds: bool, why: &str) -> Result<(), Violation> {
    if holds { Ok(()) } else { Err(Violation { promise: Promise::Lost, why: why.into() }) }
}
fn change_row(store: &Store) -> Option<&forge::ChangeRow> {
    store.rows.values().find_map(|record| {
        let root::Record::Forge { row } = record else { return None };
        let forge::Stored::Change(change) = row.as_ref() else { return None };
        Some(change)
    })
}
fn wait(world: &mut World, done: impl Fn(&World) -> bool) -> Result<(), Violation> {
    for _ in 0..30 {
        world.drain()?;
        if Temper::ready(&world.domain) && done(world) {
            return Ok(());
        }
        if world.peers.story == Some(Story::GoalIssue) && projection_held(world) {
            return Err(Violation {
                promise: Promise::Lost,
                why: format!(
                    "native projection is held for a person: issue={:?}; uncertain={:?}",
                    issue_row(world),
                    world.peers.uncertain
                ),
            });
        }
        world.advance(2_000_000_000);
    }
    Err(Violation {
        promise: Promise::Lost,
        why: format!(
            "story did not settle; change={:?}, assignments={:?}, tail={:?}; issue={:?}; calls={:?}; tasks={:?}; forge={:?}",
            change_row(&world.store),
            world.peers.assignments.iter().map(|run| (run.task, run.attempt)).collect::<Vec<_>>(),
            &world.trace[world.trace.len().saturating_sub(5)..], issue_row(world),
            world.store.rows.values().filter(|row| matches!(row,root::Record::Core(core::Record::Core(core::CoreRecord::Call(_))))).collect::<Vec<_>>(),
            world.store.rows.values().filter(|row| matches!(row,root::Record::Core(core::Record::Tasks(_)))).collect::<Vec<_>>(),
            world.store.rows.values().filter(|row| matches!(row,root::Record::Forge {row} if matches!(row.as_ref(),forge::Stored::Release(_)|forge::Stored::Entry { .. }))).collect::<Vec<_>>()
        ),
    })
}
fn landed(world: &World) -> bool {
    change_row(&world.store).is_some_and(|row| matches!(row.change.state, change::State::Landed { .. }))
}
fn review(world: &mut World) -> Result<(), Violation> {
    wait(world, |world| change_row(&world.store).is_some_and(|row| row.pull.is_some()))?;
    let number = change_row(&world.store).and_then(|row| row.pull).expect("opened pull");
    let answer = world.systems.external_as(
        world.clock(),
        2,
        raw::Op::Write(raw::Write::Review {
            number,
            verdict: Some(raw::Verdict::Approve),
            body: b"Reviewed the current head".as_slice().into(),
        }),
    );
    require(matches!(answer, raw::Answer::Reviewed(_)), "human review was refused")
}
fn positive_change(world: &mut World, repair: bool) -> Result<(), Violation> {
    if repair {
        // Only the producer's change fails; the deployment starts healthy.
        world.systems.external(
            world.clock(),
            raw::Op::Write(raw::Write::Status {
                commit: world.systems.domain.branch(b"org/repo", b"main").expect("initial main"),
                context: b"build".as_slice().into(),
                state: raw::Check::Passed,
                description: Box::new([]),
                link: Box::new([]),
            }),
        );
    }
    world.systems.failed = repair;
    scenarios::change(world, 1)?;
    if repair {
        wait(world, |world| {
            change_row(&world.store)
                .is_some_and(|row| matches!(row.delegate, Some((_, change::Delegate::Repair(change::Repair::Ci)))))
        })?;
        let child = change_row(&world.store).and_then(|row| row.delegate).expect("repair requested").0;
        wait(world, |world| world.peers.assignments.iter().any(|run| run.task == child))?;
        let branch = change_row(&world.store).expect("change").branch.clone();
        world.systems.failed = false;
        let repaired = world.systems.advance(world.clock(), &branch, b"repaired head");
        world.drain()?;
        world.systems.external(
            world.clock(),
            raw::Op::Write(raw::Write::Status {
                commit: world.systems.domain.branch(b"org/repo", &branch).expect("repaired branch"),
                context: b"build".as_slice().into(),
                state: raw::Check::Passed,
                description: Box::new([]),
                link: Box::new([]),
            }),
        );
        world.peers.produced.insert(child, repaired);
        scenarios::wake(world, child)?;
        world.drain()?;
    }
    require(!landed(world), "change merged before its head was reviewed")?;
    review(world)?;
    wait(world, landed)?;
    require(
        world.systems.effects.iter().filter(|effect| effect.kind == 4 && effect.applied).count() == 1,
        "change was not merged exactly once",
    )?;
    require(change_row(&world.store).is_some_and(|row| row.change.repairs == u32::from(repair)), "wrong repair count")?;
    let row = change_row(&world.store).expect("landed change");
    let task = row.task;
    let raw::Answer::Pull(pull) =
        world.systems.inspect(&raw::Read::Pull { number: row.pull.expect("pull") }).expect("landed pull")
    else {
        panic!("native pull answer");
    };
    require(
        !pull.statuses.is_empty() && pull.statuses.iter().all(|status| status.state == raw::Check::Passed),
        "landed head had failing CI",
    )?;
    require(
        pull.reviews.iter().any(|review| {
            review.author == 2
                && review.official
                && review.commit == pull.commit
                && review.verdict == raw::Verdict::Approve
        }),
        "landing lacked a human approval of that exact head",
    )?;
    wait(world, |world| {
        world.store.rows.values().any(|record| {
            matches!(record,
        root::Record::Core(core::Record::Tasks(tasks::Stored::Ended(row))) if row.number == task
        && matches!(row.phase,tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Change {..}))))
        })
    })
}
fn other_hand(world: &mut World) -> Result<(), Violation> {
    world.systems.hold = true;
    scenarios::change(world, 1)?;
    wait(world, |world| world.systems.delayed_merge())?;
    let row = change_row(&world.store).expect("the affected change");
    let task = row.task;
    let branch = row.branch.clone();
    Temper::action(world, Action::OtherHand)?;
    let foreign_head = world.systems.domain.branch(b"org/repo", &branch).expect("foreign branch head");
    require(
        world.systems.native_observations.iter().any(|observation| {
            matches!(observation,
            Observation::Moved { branch: moved, to, by: 4, .. } if moved == &branch && *to == foreign_head)
        }),
        "the native forge did not observe another actor's movement of that branch",
    )?;
    world.systems.hold = false;
    world.systems.release_late(world.clock());
    wait(world, |world| {
        world.store.rows.values().any(|record|matches!(record,root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row))) if row.number == task && matches!(row.phase,tasks::Phase::Held {..})))
    })?;
    require(
        world.systems.effects.iter().filter(|effect| effect.kind == 4).all(|effect| !effect.applied),
        "stale merge overwrote the foreign push",
    )?;
    require(
        world.systems.domain.branch(b"org/repo", &branch) == Some(foreign_head),
        "holding the affected task did not preserve the foreign branch head",
    )
}
fn lost_merge(world: &mut World) -> Result<(), Violation> {
    scenarios::change(world, 1)?;
    wait(world, |world| world.systems.effects.iter().any(|effect| effect.kind == 4 && effect.applied))?;
    require(!world.systems.uncertain.is_empty(), "lost merge supplied no outside uncertainty evidence")?;
    let head = world.systems.domain.branch(b"org/repo", b"main").expect("landed main");
    world.crash()?;
    wait(world, landed)?;
    require(world.systems.domain.branch(b"org/repo", b"main") == Some(head), "restart changed the merged head")?;
    require(
        world.systems.effects.iter().filter(|effect| effect.kind == 4 && effect.applied).count() == 1,
        "lost merge answer caused a second merge",
    )
}
fn person_words(world: &mut World) -> Result<(), Violation> {
    let task = world.peers.assignments[0].task;
    scenarios::ask(world, 81, people::Ask::Say { project: 1, task, words: b"Check the edge case".as_slice().into() });
    world.drain()?;
    if !world
        .peers
        .worker
        .messages
        .iter()
        .any(|(to, _, _, words)| *to == task && words.as_ref() == b"Check the edge case")
    {
        // The durable inbox survives a lost relay. After this activation parks,
        // the client's continuation starts a new run carrying the unread words.
        world.advance(2_000_000_000);
        world.drain()?;
        scenarios::ask(
            world,
            83,
            people::Ask::Say { project: 1, task, words: b"Continue with the edge case".as_slice().into() },
        );
    }

    wait(world, |world| {
        world
            .peers
            .worker
            .messages
            .iter()
            .any(|(to, _, _, words)| *to == task && words.as_ref() == b"Check the edge case")
            || world.peers.assignments.iter().any(|run| {
                run.task == task && run.messages.iter().any(|word| word.words.as_ref() == b"Check the edge case")
            })
    })?;
    require(world.store.rows.values().any(|record|matches!(record,root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row)|tasks::Stored::Ended(row))) if row.number==task && row.inbox.iter().any(|word|word.from==tasks::Party::Person(1) && word.words.as_ref()==b"Check the edge case"))),"person's words were not durably retained")
}
fn issue_row(world: &World) -> Option<&forge::IssueRow> {
    world.store.rows.values().find_map(|record| {
        let root::Record::Forge { row } = record else { return None };
        let forge::Stored::Issue(issue) = row.as_ref() else { return None };
        Some(issue)
    })
}
fn projection_held(world: &World) -> bool {
    issue_row(world).is_some_and(|issue| {
        issue.failed
            && world.store.rows.values().any(|record| {
                matches!(record,
        root::Record::Core(core::Record::Tasks(tasks::Stored::Live(task)))
        if task.number == issue.goal && matches!(task.phase,tasks::Phase::Held {..}))
            })
    })
}

fn goal_run(world: &mut World, goal: u64) -> Result<engine::Assignment, Violation> {
    wait(world, |world| world.peers.assignments.iter().any(|run| run.task == goal))?;
    let run = world.peers.assignments.iter().rev().find(|run| run.task == goal).expect("goal run").clone();
    let branch = run.workspace.repositories[0].push.as_ref().expect("goal workspace branch");
    world.systems.create(world.clock(), branch);
    world.peers.reported.insert(goal, world.systems.head);
    Ok(run)
}

fn lost_open_settles(world: &mut World, goal: u64) -> Result<(), Violation> {
    scenarios::wake(world, goal)?;
    wait(world, |world| {
        world.store.rows.contains_key(&root::Key::Core(core::Key::Tasks(tasks::Key::Ended(goal))))
            && issue_row(world).is_some_and(|issue| issue.failed && issue.pending.is_none() && issue.number.is_none())
    })?;
    require(
        !world
            .systems
            .native_observations
            .iter()
            .any(|event| matches!(event, Observation::Opened { kind: raw::Kind::Issue, by: 1, .. })),
        "a failed lost Open must have made no issue",
    )
}
#[expect(
    clippy::too_many_lines,
    reason = "one complete goal story tracks its plan, subtree milestones and final closure"
)]
fn goal_issue(world: &mut World) -> Result<(), Violation> {
    let chat = world.peers.assignments[0].clone();
    world.peers.worker.cancel(chat.task, chat.attempt);
    world.drain()?;
    scenarios::ask(
        world,
        82,
        temper_engine_domain_world::walking::goal_from(
            crate::chat(&world.peers, &world.store, 1, b"Fix the small bug through a milestone"),
            people::TaskExecutor::Agent { charter: 1 },
            jig_core_world::fixtures::Goal { budget: 100, priority: 3, deadline: None },
        ),
    );
    wait(world, |world| {
        issue_row(world).is_some_and(|issue| issue.pending.is_none() && (issue.number.is_some() || issue.failed))
    })?;
    let issue = issue_row(world).expect("goal issue");
    let goal = issue.goal;
    let number = issue.number;
    let run = goal_run(world, goal)?;
    let Some(number) = number else { return lost_open_settles(world, goal) };
    let mut authority = run.run.authority.clone();
    authority.budget.spend = 10;
    authority.delegation = tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 };
    world.systems.external_as(
        world.clock(),
        2,
        raw::Op::Write(raw::Write::Comment {
            number,
            body: b"A human issue comment is not task news".as_slice().into(),
        }),
    );
    send(
        world,
        engine::Event::Forge(Box::new(temper_engine_domain_forge::Event::Hint {
            hint: api::Hint {
                repository: temper_engine_forge_world::REPO,
                change: api::Change::Item(number),
                key: None,
            },
        })),
    );
    world.drain()?;
    require(
        !world.store.rows.values().any(|record| {
            matches!(record,
            root::Record::Forge {row} if matches!(row.as_ref(),forge::Stored::Subscription(subscription)
            if subscription.task == goal && subscription.topic == forge::Topic::Participation {
                repository: temper_engine_forge_world::REPO, number
            }))
        }),
        "owned goal issue acquired a participant subscription",
    )?;
    require(!world.store.rows.values().any(|record| matches!(record,
        root::Record::Core(core::Record::Tasks(tasks::Stored::Live(task))) if task.number == goal
        && task.inbox.iter().any(|word| matches!(word.kind,tasks::MessageKind::News {class: tasks::NewsClass::Wakes,..})))),
        "human issue comments woke the projected goal")?;
    let milestone = engine::Delegate {
        executor: tasks::Executor::Agent { charter: 1 },
        spec: tasks::Spec {
            words: b"First milestone".as_slice().into(),
            parameters: Box::new([]),
            inputs: Box::new([]),
        },
        contract: tasks::Contract::Report { words: 32 },
        authority,
        symbolic_grants: Box::new([]),
        dependencies: Box::new([]),
        wake: tasks::WakePolicy::DEFAULT,
        ..temper_engine_domain_world::fixtures::report_delegate(b"fixture", Box::new([]))
    };
    let tool = engine::Tool::Delegate { batch: Box::new([milestone]) };
    crate::link::call(world, goal, run.attempt, 8301, engine::Call { completion: 301, position: 0, tool });
    wait(world, |world| world.peers.assignments.iter().any(|run| run.task != chat.task && run.task != goal))?;
    let child =
        world.peers.assignments.iter().find(|run| run.task != chat.task && run.task != goal).expect("milestone").task;
    let branch =
        world.peers.assignments.iter().find(|run| run.task == child).expect("milestone run").workspace.repositories[0]
            .push
            .clone()
            .expect("milestone workspace branch");
    world.systems.create(world.clock(), &branch);
    world.peers.reported.insert(child, world.systems.head);
    scenarios::wake(world, child)?;
    wait(world, |world| world.store.rows.contains_key(&root::Key::Core(core::Key::Tasks(tasks::Key::Ended(child)))))?;
    scenarios::wake(world, goal)?;
    wait(world, |world| {
        world.store.rows.contains_key(&root::Key::Core(core::Key::Tasks(tasks::Key::Ended(goal))))
            && world.systems.native_observations.iter().any(|event| {
                matches!(event,
            Observation::Closed {number: n,by: 1,..} if *n == number)
            })
    })?;
    require(world.systems.native_observations.iter().any(|event| matches!(event,
        Observation::Revised {number: n,body,..} if *n == number && body.windows(b"First milestone".len()).any(|part| part == b"First milestone"))), "goal projection omitted its milestone")?;
    require(
        world.systems.native_observations.iter().any(|event| {
            matches!(event,
        Observation::Closed {number: n,by: 1,..} if *n == number)
        }),
        "ended goal did not close its issue",
    )?;
    require(
        world
            .systems
            .native_observations
            .iter()
            .filter(|event| matches!(event, Observation::Opened { kind: raw::Kind::Issue, by: 1, .. }))
            .count()
            == 1,
        "goal issue creation was duplicated",
    )
}
fn issue_creation(world: &mut World) -> Result<(), Violation> {
    let run = world.peers.assignments[0].clone();
    let tool = engine::Tool::EffectForge {
        repository: temper_engine_forge_world::REPO,
        resource: forge::What::Repository,
        write: Box::new(api::Write::CreateIssue {
            title: b"Native uncertainty".as_slice().into(),
            body: b"A retained creation marker".as_slice().into(),
            key: Box::new([]),
        }),
    };
    crate::link::call(world, run.task, run.attempt, 8401, engine::Call { completion: 401, position: 0, tool });
    wait(world, |world| {
        world.systems.effects.iter().any(|effect| effect.kind == 8 && effect.applied) || creation_held(world)
    })?;
    if creation_held(world) {
        return require(
            world.systems.effects.iter().filter(|effect| effect.kind == 8 && effect.applied).count() == 0,
            "held creation was repeated",
        );
    }
    require(!world.systems.uncertain.is_empty(), "lost issue supplied no outside uncertainty evidence")?;
    world.crash()?;
    wait(world, |world| {
        world.store.rows.values().any(|record| matches!(record,
        root::Record::Core(core::Record::Core(core::CoreRecord::Call(call)))
        if call.key.completion == 401 && matches!(call.part, core::CallPart::Effect {outcome: Some(core::connector::OutboxOutcome::Made),..})))
    })?;
    require(
        world
            .systems
            .native_observations
            .iter()
            .filter(|event| matches!(event, Observation::Opened { kind: raw::Kind::Issue, by: 1, .. }))
            .count()
            == 1,
        "lost creation answer caused another issue",
    )
}
fn creation_held(world: &World) -> bool {
    world.store.rows.values().any(|record| {
        matches!(record,
        root::Record::Core(core::Record::Core(core::CoreRecord::Call(call)))
        if call.key.completion == 401 && matches!(call.part,core::CallPart::Effect {
            outcome: Some(core::connector::OutboxOutcome::Held {..}),..
        }))
    })
}
/// Run one complete story with the selected additional cold cut.
///
/// # Errors
/// Returns the first broken outside promise or unmet story ending.
pub fn run(story: Story, cut: Cut, seed: u64) -> Result<Outcome, Violation> {
    let mut world = Harness::<Temper>::new(Config::for_story(seed, story), seed);
    world.cut(cut);
    world.drain()?;
    Temper::action(&mut world, Action::Ready)?;
    let chat = world.peers.assignments[0].clone();
    let branch = chat.workspace.repositories[0].push.as_ref().expect("chat workspace branch");
    world.systems.create(world.clock(), branch);
    world.peers.reported.insert(chat.task, world.systems.head);
    match story {
        Story::ReviewedChange => positive_change(&mut world, false)?,
        Story::RepairCi => positive_change(&mut world, true)?,
        Story::OtherHand => other_hand(&mut world)?,
        Story::LostMerge => lost_merge(&mut world)?,
        Story::PersonWords => person_words(&mut world)?,
        Story::GoalIssue => {
            if let Err(error) = goal_issue(&mut world) {
                if !projection_held(&world)
                    || !world.peers.held.iter().any(|(key, kind)| {
                        issue_row(&world).is_some_and(|issue| issue.goal == key.task)
                            && matches!(kind, 7 | 8)
                            && world.peers.uncertain.contains_key(key)
                    })
                {
                    return Err(error);
                }
                require(
                    world
                        .systems
                        .native_observations
                        .iter()
                        .filter(|event| matches!(event, Observation::Opened { kind: raw::Kind::Issue, by: 1, .. }))
                        .count()
                        <= 1,
                    "held goal projection created duplicate issues",
                )?;
            }
        }
        Story::IssueCreation => issue_creation(&mut world)?,
    }
    world.drain()
}
