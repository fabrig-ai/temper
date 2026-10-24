//! Shared adversarial actions bound to temper's forge and task vocabulary.
use super::{Config, Temper, send};
use jig_conformance::scenarios::{Action, Scenario, ScenarioApplication};
use jig_conformance::{
    Harness,
    referee::{Promise, Violation},
};
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_forge::api;
use skein_lib::{ReplyTo, Token};
use temper_engine_domain as engine;
use temper_engine_domain::{self as root};
use temper_engine_domain_forge as forge;
type World = Harness<Temper>;
const COMMITTED_CALLER: u8 = 47;
const NEW_DECISION_CALLER: u8 = 46;
mod conversations;
pub(crate) mod lifecycle;
pub(crate) mod queries;
mod revisions;
fn parent(world: &World) -> (u64, u64) {
    let a = &world.peers.assignments[0];
    (a.task, a.attempt)
}
fn message(world: &mut World, task: u64) {
    let attempt = world.peers.assignments.iter().rev().find(|a| a.task == task).expect("assigned host").attempt;
    world.peers.worker.message(task, attempt, 100, b"complete scripted work".as_slice().into());
}
fn check(holds: bool, why: &str) -> Result<(), Violation> {
    if holds { Ok(()) } else { Err(Violation { promise: Promise::Lost, why: why.into() }) }
}
fn check_lazy(holds: bool, why: impl FnOnce() -> String) -> Result<(), Violation> {
    if holds { Ok(()) } else { Err(Violation { promise: Promise::Lost, why: why() }) }
}
fn ci_repair(record: &root::Record) -> bool {
    let root::Record::Forge { row } = record else { return false };
    let forge::Stored::Change(change) = row.as_ref() else { return false };
    matches!(
        change.delegate,
        Some((_, temper_engine_domain_forge_change::Delegate::Repair(temper_engine_domain_forge_change::Repair::Ci)))
    )
}
pub(crate) fn ask(world: &mut World, key: u8, ask: people::Ask) {
    if let people::Ask::Amend { task, revision, .. } = &ask {
        world.peers.observed.push(jig_conformance::referee::Observed::Named {
            task: *task,
            revision: jig_conformance::referee::Revision::Spec(*revision),
        });
    }
    send(
        world,
        engine::Event::Party(engine::PartyInput::Ask {
            models: Box::new([]),
            reply_to: ReplyTo::new(Token::new(6_000 + u64::from(key))),
            sign_in: world.peers.sign_in.expect("owner signed in"),
            key: [key; 16],
            ask,
        }),
    );
}
fn authority(world: &World) -> tasks::Authority {
    let task = world.peers.assignments[0].task;
    match world.store.rows.get(&root::Key::Core(core::Key::Tasks(tasks::Key::Live(task)))) {
        Some(root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row)))) => row.authority.clone(),
        other => panic!("live parent {other:?}"),
    }
}
fn effect(world: &mut World, completion: u32, proposal: bool) {
    let assignment = if (completion == 1 || completion == 3) && world.peers.scenario == Some(Scenario::NarrowingPolicy)
    {
        world.peers.assignments.last().expect("new caller")
    } else {
        &world.peers.assignments[0]
    };
    let write = api::Write::Status {
        commit: world.systems.head,
        context: b"conformance".as_slice().into(),
        check: api::Check::Passed,
    };
    let tool = if proposal {
        engine::Tool::Propose {
            action: engine::ProposedAction::Effect {
                repository: temper_engine_forge_world::REPO,
                resource: forge::What::Repository,
                write: Box::new(write),
            },
            reason: b"a holder decides".as_slice().into(),
            as_holder: false,
        }
    } else {
        engine::Tool::EffectForge {
            repository: temper_engine_forge_world::REPO,
            resource: forge::What::Repository,
            write: Box::new(write),
        }
    };
    let (task, attempt) = (assignment.task, assignment.attempt);
    crate::link::call(
        world,
        task,
        attempt,
        7_000 + u64::from(completion),
        engine::Call { completion, position: 0, tool },
    );
}
pub(crate) fn wake(world: &mut World, task: u64) -> Result<(), Violation> {
    if !world.peers.worker.seen.iter().any(|run| run.task == task) {
        world.advance(6_000_000_000);
        world.drain()?;
        world.advance(1_000_000_000);
        world.drain()?;
    }
    let attempt = world
        .peers
        .assignments
        .iter()
        .rev()
        .find(|a| a.task == task)
        .unwrap_or_else(|| {
            panic!(
                "unassigned task {task} rows={:?} assignments={:?} trace={:?}",
                world.store.rows,
                world.peers.assignments,
                &world.trace[world.trace.len().saturating_sub(10)..]
            )
        })
        .attempt;
    world.peers.worker.message(task, attempt, 100, b"complete work".as_slice().into());
    Ok(())
}
fn ended(world: &World, task: u64) -> bool {
    world.store.rows.contains_key(&root::Key::Core(core::Key::Tasks(tasks::Key::Ended(task))))
}
impl ScenarioApplication for Temper {
    fn configuration(scenario: Scenario, seed: u64) -> Option<Config> {
        if scenario == Scenario::ShrinkingPool { None } else { Some(Config::new(seed, Some(scenario))) }
    }
    #[expect(
        clippy::too_many_lines,
        clippy::wildcard_enum_match_arm,
        clippy::match_wildcard_for_single_variants,
        reason = "the shared actions bind seven stories and select their durable task and outbox facts"
    )]
    fn action(world: &mut World, action: Action) -> Result<(), Violation> {
        match action {
            Action::Compound | Action::CompoundAgain | Action::CancelWithGrace | Action::Finish => {
                let mut phase = 0;
                while lifecycle::advance(world, action, &mut phase)? {
                    world.drain()?;
                }
            }
            Action::WatchQuery | Action::EndTask | Action::ReadEnded => {
                let mut phase = 0;
                while queries::advance(world, action, &mut phase)? {
                    world.drain()?;
                }
            }
            Action::Conversation { .. } | Action::Turn | Action::MarkTurns | Action::MarkResults => {
                conversations::action(world, action);
            }
            Action::EndConversations => {
                for index in 0..2 {
                    conversations::finish(world, index);
                    world.drain()?;
                }
            }
            Action::Ready => {
                for _ in 0..4 {
                    if !world.peers.assignments.is_empty() {
                        break;
                    }
                    world.advance(6_000_000_000);
                    world.drain()?;
                    world.advance(1_000_000_000);
                    world.drain()?;
                }
                check_lazy(!world.peers.assignments.is_empty(), || {
                    format!(
                        "no caller assigned signIn={:?} adopted={} tasks={:?} tail={:?} replies={:?}",
                        world.peers.sign_in,
                        world.peers.adopted,
                        world
                            .store
                            .rows
                            .values()
                            .filter(|r| matches!(r, root::Record::Core(core::Record::Tasks(_))))
                            .collect::<Vec<_>>(),
                        &world.trace[world.trace.len().saturating_sub(5)..],
                        world.trace.iter().filter(|event| event.contains("WebReply")).collect::<Vec<_>>()
                    )
                })?;
                if world.peers.scenario == Some(Scenario::Compound) {
                    let mut phase = 2;
                    while lifecycle::prepare(world, &mut phase)? {
                        world.drain()?;
                    }
                }
                world.peers.lifecycle.ready = world.outcome().commits;
                world.peers.initial_assignments = world.peers.assignments.len();
                if world.peers.scenario == Some(Scenario::NarrowingPolicy) {
                    ask(
                        world,
                        COMMITTED_CALLER,
                        crate::chat(&world.peers, &world.store, 1, b"committed flight caller"),
                    );
                    world.drain()?;
                    if world.peers.assignments.len() == 1 {
                        world.advance(7_000_000_000);
                        world.drain()?;
                    }
                }
                if world.peers.scenario == Some(Scenario::ChangedJudge) {
                    let pattern = people::Pattern {
                        segments: Box::new([
                            b"forge".as_slice().into(),
                            b"forge.example".as_slice().into(),
                            b"org".as_slice().into(),
                            b"repo".as_slice().into(),
                            b"branch".as_slice().into(),
                        ]),
                        last: people::Last::Exact(b"main".as_slice().into()),
                    };
                    ask(
                        world,
                        48,
                        people::Ask::ChangePolicy {
                            project: 1,
                            change: people::PolicyChange::Requirements {
                                requirements: Box::new([people::Requirement {
                                    connector: 0,
                                    kind: 4,
                                    pattern,
                                    judge: people::Judge { connector: 0, requirement: 1, parameters: 0x8000_0000 },
                                    guard: people::Guard::Observed { freshness: skein_lib::Duration::ZERO },
                                    must_be_guarded: false,
                                }]),
                            },
                        },
                    );
                    world.drain()?;
                    let mut base = super::observations::repository().path;
                    base.push(b"branch".to_vec());
                    world.peers.policy.requirements.push(jig_conformance::referee::Requirement {
                        project: Some(1),
                        connector: 0,
                        kind: 4,
                        pattern: jig_conformance::referee::Pattern { base, terminal: b"main".to_vec(), open: false },
                        judge: 0,
                        freshness: Some(1_000_000_000),
                    });
                    world.peers.policy_generation += 1;
                }
            }
            Action::Late => world.systems.late_next = true,
            Action::Effect(completion) => {
                if completion == 3 && world.peers.scenario == Some(Scenario::NarrowingPolicy) {
                    let flight = &world.peers.assignments[1];
                    world.peers.worker.cancel(flight.task, flight.attempt);
                    world.drain()?;
                    ask(world, NEW_DECISION_CALLER, crate::chat(&world.peers, &world.store, 1, b"new decision caller"));
                    world.drain()?;
                    for _ in 0..16 {
                        if world.peers.assignments.len() >= 3 {
                            break;
                        }
                        world.advance(1_000_000_000);
                        world.drain()?;
                    }
                    check(world.peers.assignments.len() >= 3, "new decision caller was not assigned")?;
                }
                if matches!(world.peers.scenario, Some(Scenario::OtherHand | Scenario::ChangedJudge)) {
                    change(world, completion)?;
                } else {
                    effect(world, completion, false);
                    if completion == 3 && world.peers.scenario == Some(Scenario::NarrowingPolicy) {
                        world.drain()?;
                        for _ in 0..4 {
                            if world
                                .peers
                                .answers
                                .iter()
                                .any(|answer| matches!(answer, root::CallAnswer::ForgeEffectDenied { .. }))
                            {
                                break;
                            }
                            world.advance(1_000_000_000);
                            world.drain()?;
                        }
                    }
                }
            }
            Action::DeliverLate => {
                if world.peers.scenario == Some(Scenario::LateCopy) {
                    for _ in 0..8 {
                        if !world.systems.effects.is_empty() {
                            break;
                        }
                        world.advance(2_000_000_000);
                        world.drain()?;
                    }
                }
                world.systems.release_late(world.clock());
            }
            Action::ReleaseWrites => {
                world.systems.release_late(world.clock());
                if world.peers.scenario == Some(Scenario::NarrowingPolicy) {
                    world.drain()?;
                    for _ in 0..3 {
                        if !world.systems.effects.is_empty() {
                            break;
                        }
                        world.advance(2_000_000_000);
                        world.drain()?;
                    }
                }
            }
            Action::HoldWrites => world.systems.hold = true,
            Action::CheckDeadline => {
                let deadline = world
                    .store
                    .rows
                    .values()
                    .find_map(|row| match row {
                        root::Record::Forge { row } => match row.as_ref() {
                            forge::Stored::Entry { entry, held: false } => entry.attempt.map(|a| a.expires),
                            _ => None,
                        },
                        _ => None,
                    })
                    .expect("uncertain entry");
                if let Some(saved) = world.peers.saved_deadline {
                    check(saved == deadline, "restart changed absolute attempt deadline")?;
                } else {
                    world.peers.saved_deadline = Some(deadline);
                }
            }
            Action::Propose => {
                effect(world, 2, true);
                world.drain()?;
                for _ in 0..4 {
                    if world.peers.pending_proposal.is_some() {
                        break;
                    }
                    world.advance(1_000_000_000);
                    world.drain()?;
                }
            }
            Action::AcceptProposal => {
                let proposal = world.peers.pending_proposal.unwrap_or_else(|| {
                    panic!(
                        "waiting proposal answers={:?} assignments={:?} tasks={:?} calls={:?}",
                        world.peers.answers,
                        world.peers.assignments.iter().map(|a| (a.task, a.attempt)).collect::<Vec<_>>(),
                        world
                            .store
                            .rows
                            .values()
                            .filter_map(|r| match r {
                                root::Record::Core(core::Record::Tasks(tasks::Stored::Live(t))) =>
                                    Some((t.number, t.phase.clone(), t.authority.tools, t.proposal.clone())),
                                _ => None,
                            })
                            .collect::<Vec<_>>(),
                        world
                            .store
                            .rows
                            .values()
                            .filter(|r| matches!(r, root::Record::Core(core::Record::Core(core::CoreRecord::Call(_)))))
                            .collect::<Vec<_>>()
                    )
                });
                ask(
                    world,
                    44,
                    people::Ask::DecideProposal {
                        project: 1,
                        proposer: people::Proposer::Task(world.peers.assignments[0].task),
                        proposal,
                        decision: people::ProposalDecision::Accept,
                        running: None,
                    },
                );
            }
            Action::StartStanding => {
                let parent = world.peers.assignments[0].clone();
                world.peers.worker.cancel(parent.task, parent.attempt);
                world.drain()?;
                let mut authority = authority(world);
                authority.budget.spend = 30;
                authority.delegation =
                    tasks::Delegation { kinds: Box::new([tasks::AuthorityExecutor::Charter(1)]), tasks: 1, depth: 1 };
                let mut child = authority.clone();
                child.grants = Box::new([]);
                child.budget.spend = 10;
                child.delegation = tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 };
                send(
                    world,
                    engine::Event::Party(engine::PartyInput::StartRecurring {
                        project: 1,
                        authority,
                        template: tasks::RecurringTemplate {
                            key: 1,
                            overlap: tasks::RecurringOverlap::Skip,
                            batch: Box::new([tasks::New {
                                label: 0,
                                deadline: tasks::Deadline::Named,
                                flags: tasks::Flags::PLAIN,
                                number: 1,
                                project: 1,
                                executor: tasks::Executor::Agent { charter: 1 },
                                spec: tasks::Spec {
                                    words: b"period work".as_slice().into(),
                                    parameters: Box::new([]),
                                    inputs: Box::new([]),
                                },
                                contract: tasks::Contract::Report { words: 128 },
                                llm: None,
                                authority: child,
                                numbers: tasks::Numbers { budget: 10, spent: 0, spent_below: 0, reserved: 0 },
                                funder: tasks::Funder::Period { project: 1, period: 0 },
                                dependencies: Box::new([]),
                                holdings: Box::new([]),
                                wake: tasks::WakePolicy::DEFAULT,
                                recurring: None,
                                tracked: None,
                            }]),
                        },
                    }),
                );
            }
            Action::Period(period) => {
                let task = world
                    .store
                    .rows
                    .values()
                    .filter_map(|row| match row {
                        root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row)))
                            if row.spec.words.as_ref() == b"period work" =>
                        {
                            Some(row.number)
                        }
                        _ => None,
                    })
                    .max()
                    .expect("standing batch");
                wake(world, task)?;
                world.drain()?;
                check(ended(world, task), "standing batch did not end")?;
                let before = world.peers.assignments.len();
                for _ in 0..2 {
                    send(
                        world,
                        engine::Event::Party(engine::PartyInput::Period {
                            project: 1,
                            period,
                            budget: 1000,
                            next: None,
                        }),
                    );
                    world.drain()?;
                }
                if world.peers.assignments.len() == before {
                    world.advance(6_000_000_000);
                    world.drain()?;
                    world.advance(1_000_000_000);
                    world.drain()?;
                }
                check_lazy(world.peers.assignments.len() == before + 1, || {
                    format!(
                        "period {period} before {before} assignments {:?} rows {:?}",
                        world.peers.assignments.iter().map(|a| (a.task, a.attempt)).collect::<Vec<_>>(),
                        world
                            .store
                            .rows
                            .values()
                            .filter_map(|r| match r {
                                root::Record::Core(core::Record::Tasks(
                                    tasks::Stored::Live(t) | tasks::Stored::Ended(t),
                                )) => Some((
                                    t.number,
                                    t.phase.clone(),
                                    t.numbers,
                                    t.funder,
                                    t.delegates.clone(),
                                    t.recurring.clone()
                                )),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                    )
                })?;
            }
            Action::Resume => wake(world, world.peers.assignments[0].task)?,
            Action::OtherHand => {
                let branch = world.peers.branches[0].clone();
                let head = world.systems.advance(world.clock(), &branch, b"other hand");
                send(
                    world,
                    engine::Event::Forge(Box::new(temper_engine_domain_forge::Event::Hint {
                        hint: api::Hint {
                            repository: temper_engine_forge_world::REPO,
                            change: api::Change::BranchMoved { branch, head, actor: 4 },
                            key: None,
                        },
                    })),
                );
            }
            Action::ChangeJudge => {
                world.systems.failed = true;
                let commit = world.systems.domain.branch(b"org/repo", &world.peers.branches[0]).expect("producer head");
                world.systems.fail(world.clock(), commit);
            }
            Action::NarrowPolicy => {
                let pattern = people::Pattern {
                    segments: Box::new([
                        b"forge".as_slice().into(),
                        b"forge.example".as_slice().into(),
                        b"org".as_slice().into(),
                    ]),
                    last: people::Last::Open(b"repo".as_slice().into()),
                };
                ask(
                    world,
                    45,
                    people::Ask::ChangePolicy {
                        project: 1,
                        change: people::PolicyChange::Requirements {
                            requirements: Box::new([people::Requirement {
                                connector: 0,
                                kind: 6,
                                pattern,
                                judge: people::Judge { connector: 0, requirement: 1, parameters: 0x8000_0000 },
                                guard: people::Guard::Observed { freshness: skein_lib::Duration::ZERO },
                                must_be_guarded: false,
                            }]),
                        },
                    },
                );
                world.drain()?;
                check(world.store.rows.values().any(|row|matches!(row,root::Record::Core(core::Record::People(people::Stored::Policy {value,..})) if value.requirements.iter().any(|r|r.kind==6))),"narrow requirement not committed")?;
                if !world.peers.policy.requirements.iter().any(|r| r.kind == 6) {
                    world.peers.policy.requirements.push(jig_conformance::referee::Requirement {
                        project: Some(1),
                        connector: 0,
                        kind: 6,
                        pattern: jig_conformance::referee::Pattern {
                            base: vec![b"forge".to_vec(), b"forge.example".to_vec(), b"org".to_vec()],
                            terminal: b"repo".to_vec(),
                            open: true,
                        },
                        judge: 0,
                        freshness: Some(0),
                    });
                    world.peers.policy_generation += 1;
                }
            }
            Action::PartyAmend => revisions::party_amend(world)?,
            Action::RunAmend => revisions::run_amend(world),
            Action::FillPool | Action::ShrinkPool | Action::FinishHolder(_) => {
                panic!("ShrinkingPool is not applicable: forge has no pooled holds")
            }
        }
        Ok(())
    }
    fn check(world: &World, scenario: Scenario) -> Result<(), Violation> {
        match scenario {
            Scenario::Compound | Scenario::Grace => lifecycle::check_ending(world, scenario),
            Scenario::ConversationEndsWhileRead => conversations::check_ending(world),
            Scenario::QueryWatchedWhileTasksEnd => queries::check_ending(world),
            Scenario::RacingDecisions => revisions::check(world),
            Scenario::LateCopy => {
                check_lazy(world.systems.effects.len() == 2 && world.systems.effects.iter().all(|x| x.applied), || {
                    format!("idempotent copies effects={:?} answers={:?}", world.systems.effects, world.peers.answers)
                })
            }
            Scenario::UncertainRestarts => check(
                !world.systems.effects.is_empty()
                    && world.systems.effects.iter().all(|x| x.applied)
                    && world
                        .systems
                        .effects
                        .iter()
                        .map(|effect| effect.key)
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                        == 1,
                "uncertain status set did not resolve one operation",
            ),
            Scenario::StandingTask => {
                check(world.peers.assignments.len() == 6, "standing task did not run five funding periods")
            }
            Scenario::RunBudget => check(
                world.peers.worker.actual_spent == 20 && world.peers.worker.turn_acks == 12,
                "sessions did not share one run allowance",
            ),
            Scenario::OtherHand => check(
                world.systems.effects.iter().filter(|x| x.kind == 4).all(|x| !x.applied)
                    && world.systems.effects.iter().any(|x| x.kind == 4),
                "a stale conditional merge was absent or overwrote the other hand",
            ),
            Scenario::ChangedJudge => check(
                world.systems.effects.iter().filter(|x| x.kind == 4 && x.applied).count() == 1
                    && world.systems.effects.iter().filter(|x| x.kind == 3 && x.applied).count() == 2
                    && world.store.rows.values().any(ci_repair),
                "observed judge did not grandfather the decided flight and stop a new merge",
            ),
            Scenario::NarrowingPolicy => check_lazy(
                world.systems.effects.iter().map(|effect| effect.key).collect::<std::collections::BTreeSet<_>>().len()
                    == 1
                    && world.peers.pending_proposal.is_some()
                    && world.peers.answers.iter().any(|a| matches!(a, root::CallAnswer::ForgeEffectDenied { .. })),
                || {
                    format!(
                        "narrow effects={:?} answers={:?} assignments={:?} calls={:?}",
                        world.systems.effects,
                        world.peers.answers,
                        world.peers.assignments.iter().map(|a| (a.task, a.attempt)).collect::<Vec<_>>(),
                        world
                            .store
                            .rows
                            .values()
                            .filter(|r| matches!(r, root::Record::Core(core::Record::Core(core::CoreRecord::Call(_)))))
                            .collect::<Vec<_>>()
                    )
                },
            ),
            Scenario::ShrinkingPool => panic!("no pooled holds"),
        }
    }
}

#[expect(
    clippy::too_many_lines,
    clippy::wildcard_enum_match_arm,
    reason = "one bounded producer fixture specifies its authority and reads task facts for diagnostics"
)]
pub(crate) fn change(world: &mut World, completion: u32) -> Result<(), Violation> {
    if world.peers.changes.contains(&u64::from(completion)) {
        return Ok(());
    }
    world.peers.changes.push(u64::from(completion));
    let parent = world.peers.assignments[0].clone();
    let grant = tasks::Grant {
        connector: 0,
        kind: 3,
        pattern: tasks::Pattern {
            segments: Box::new([
                b"forge".as_slice().into(),
                b"forge.example".as_slice().into(),
                b"org".as_slice().into(),
            ]),
            last: tasks::Last::Open(b"repo".as_slice().into()),
        },
    };
    let authority = tasks::Authority {
        tools: tasks::Tools(1023),
        grants: [3, 4, 15].map(|kind| tasks::Grant { kind, ..grant.clone() }).into(),
        delegation: tasks::Delegation { kinds: Box::new([tasks::AuthorityExecutor::Charter(1)]), tasks: 4, depth: 1 },
        budget: tasks::Budget { spend: 20, deadline: None },
        notes: tasks::Scopes(0),
        note_resources: Box::new([]),
        models: tasks::Models::NONE,
    };
    let before = world.peers.assignments.len();
    crate::link::call(
        world,
        parent.task,
        parent.attempt,
        8000 + u64::from(completion),
        engine::Call {
            completion: 100 + completion,
            position: 1,
            tool: engine::Tool::Delegate {
                batch: Box::new([engine::Delegate {
                    executor: tasks::Executor::Procedure { connector: 0, code: 2 },
                    spec: tasks::Spec {
                        words: b"Small fix".as_slice().into(),
                        parameters: Box::new([
                            tasks::Parameter::Resource {
                                name: 1,
                                connector: 0,
                                resource: temper_engine_domain_forge::tag(
                                    temper_engine_forge_world::REPO.forge,
                                    temper_engine_forge_world::REPO.repository,
                                ),
                            },
                            tasks::Parameter::Bytes { name: 2, value: b"main".as_slice().into() },
                            tasks::Parameter::Bytes { name: 6, value: b"file".as_slice().into() },
                        ]),
                        inputs: Box::new([]),
                    },
                    contract: tasks::Contract::Change { connector: 0, kind: 1, words: 32 },
                    authority,
                    symbolic_grants: Box::new([tasks::Grant {
                        connector: 0,
                        kind: 2,
                        pattern: tasks::Pattern {
                            segments: Box::new([
                                b"forge".as_slice().into(),
                                b"forge.example".as_slice().into(),
                                b"org".as_slice().into(),
                                b"repo".as_slice().into(),
                                b"branch".as_slice().into(),
                                b"temper".as_slice().into(),
                                parent.task.to_string().into_bytes().into(),
                            ]),
                            last: tasks::Last::Open(b"c".as_slice().into()),
                        },
                    }]),
                    dependencies: Box::new([]),
                    wake: tasks::WakePolicy::DEFAULT,
                    ..temper_engine_domain_world::fixtures::report_delegate(b"fixture", Box::new([]))
                }]),
            },
        },
    );
    world.drain()?;
    for _ in 0..4 {
        if world.peers.assignments.len() > before {
            break;
        }
        world.advance(7_000_000_000);
        world.drain()?;
    }
    let producer = world.peers.assignments.last().expect("producer").clone();
    check_lazy(producer.task != parent.task, || {
        format!(
            "change producer not assigned tasks={:?}",
            world
                .store
                .rows
                .values()
                .filter_map(|r| match r {
                    root::Record::Core(core::Record::Tasks(tasks::Stored::Live(t))) =>
                        Some((t.number, t.phase.clone(), t.spec.words.clone())),
                    _ => None,
                })
                .collect::<Vec<_>>()
        )
    })?;
    let branch = producer.workspace.repositories[0].push.clone().expect("change branch");
    world.systems.create(world.clock(), &branch);
    let pushed = world.systems.advance(world.clock(), &branch, format!("fixed {completion}").as_bytes());
    world.peers.branches.push(branch);
    world.peers.produced.insert(producer.task, pushed);
    world.peers.worker.message(producer.task, producer.attempt, 200, b"report push".as_slice().into());
    world.drain()?;
    for _ in 0..40 {
        if world.peers.story.is_some() && world.store.rows.values().any(|record|
            matches!(record,root::Record::Forge {row} if matches!(row.as_ref(),forge::Stored::Change(change) if change.pull.is_some()))) {
            break;
        }
        if world.systems.delayed_merge() {
            break;
        }
        if completion == 2 && world.systems.failed && world.store.rows.values().any(ci_repair) {
            break;
        }
        world.advance(if completion == 1 { 2_000_000_000 } else { 500_000_000 });
        world.drain()?;
    }
    Ok(())
}
