//! The protocol layer this world plays for its scripted worker: the hub's
//! link records, engine tools called by name and their relayed answers
//! (jig's `domain/hosts.md`, 2 and 6.4). The engine world's protocol stand
//! decodes, renders and reads outcomes, as jig's worlds do.
use crate::{Peers, Temper, send};
use jig_conformance::{Harness, referee as r};
use jig_domain_host as host;
use jig_domain_tasks as tasks;
use jig_fake_workers as workers;
use skein_lib::{Duration, Time, Token};
use temper_engine_domain as engine;
use temper_engine_domain_forge as forge;
use temper_engine_domain_world::protocol;

/// The one scripted worker's channel.
pub const CHANNEL: u64 = 7;
/// A worker asks a call again when its answer has not come within this long.
const RETRY: u64 = 1_000_000_000;

/// A run's call of one engine tool, named `name`; the worker keeps it until
/// its answer is relayed down and asks it again after a busy answer or a
/// lost engine.
pub fn call(world: &mut Harness<Temper>, task: u64, attempt: u64, name: u64, body: engine::Call) {
    if matches!(body.tool, engine::Tool::Propose { .. }) {
        world.peers.observed.push(r::Observed::ProposeAsked { proposer: r::Source::Task(task) });
    }
    if let engine::Tool::Amend { target, revision, .. } = &body.tool {
        world.peers.observed.push(r::Observed::Named { task: *target, revision: r::Revision::Spec(*revision) });
    }
    let now = world.clock().now;
    world.peers.calls.insert(name, (task, attempt, body.clone()));
    world.peers.retries.insert(name, now + RETRY);
    let event = world.peers.protocol.call(Token::new(CHANNEL), task, attempt, name, body);
    send(world, event);
}

/// The calls whose answers are overdue, asked again.
pub(crate) fn retried(peers: &mut Peers, now: u64) -> Vec<engine::Event> {
    let due: Vec<_> = peers.retries.iter().filter_map(|(name, at)| (*at <= now).then_some(*name)).collect();
    let mut events = Vec::new();
    for name in due {
        peers.retries.insert(name, now + RETRY);
        if let Some((task, attempt, body)) = peers.calls.get(&name) {
            events.push(peers.protocol.call(Token::new(CHANNEL), *task, *attempt, name, body.clone()));
        }
    }
    events
}

/// Translate one scripted worker report into the hub's link vocabulary.
/// temper's parts ride beside an answer: the heads the scenario pushed for
/// that task, and a produced change's result.
pub(crate) fn worker_event(peers: &mut Peers, up: workers::Up) -> engine::Event {
    match up {
        workers::Up::Hello { channel, slots, stop_bound, hosting } => {
            assert_eq!(stop_bound, Duration::from_millis(100), "the stand's hello keeps the worker's stop bound");
            let hosting = hosting
                .into_iter()
                .map(|(task, attempt, answered)| {
                    protocol::hosting(task, attempt, if answered { host::Phase::Answered } else { host::Phase::Active })
                })
                .collect();
            protocol::hello_slots(Token::new(channel), slots, hosting)
        }
        workers::Up::Writing { .. } => panic!("conformance scripts report no provisional writing"),
        workers::Up::Lost { channel } => protocol::lost(Token::new(channel)),
        workers::Up::Turn { channel, task, attempt, turn, spent, read, body, opens_window, refused } => {
            assert!(refused.is_empty(), "conformance scripts refuse no message");
            let turn =
                host::Turn { opens_window, refused: Box::new([]), turn, spent, read: read.map(Token::new), body };
            let up = host::LinkUp::Turn { run: Token::new(task), attempt: Token::new(attempt), turn };
            protocol::up(Token::new(channel), up, engine::Parts::none())
        }
        workers::Up::Answer { channel, task, attempt, spent, end, read, refused } => {
            assert!(read.is_none() && refused.is_empty(), "conformance scripts read and refuse no message");
            let mut end = match end {
                workers::End::Finished(words) => {
                    tasks::End::Finished { result: tasks::TaskResult::Report { words }, cancel_delegates: false }
                }
                workers::End::Parked => tasks::End::Parked,
                workers::End::Failed(code) => panic!("unbound host failure {code:?}"),
            };
            let mut pushed: Box<[forge::Pushed]> = Box::new([]);
            if let Some(commit) = peers.reported.get(&task) {
                pushed = Box::new([forge::Pushed {
                    tag: forge::tag(temper_engine_forge_world::REPO.forge, temper_engine_forge_world::REPO.repository),
                    commit: *commit,
                }]);
            }
            if matches!(end, tasks::End::Finished { .. })
                && let Some(commit) = peers.produced.get(&task)
            {
                end = tasks::End::Finished {
                    result: tasks::TaskResult::Change {
                        connector: 0,
                        kind: 2,
                        resource: temper_engine_domain_forge::tag(
                            temper_engine_forge_world::REPO.forge,
                            temper_engine_forge_world::REPO.repository,
                        ),
                        words: b"pushed".as_slice().into(),
                    },
                    cancel_delegates: false,
                };
                pushed = Box::new([forge::Pushed {
                    tag: forge::tag(temper_engine_forge_world::REPO.forge, temper_engine_forge_world::REPO.repository),
                    commit: *commit,
                }]);
            }
            let parts = engine::Parts { saved: None, pushed, preparation: None };
            peers.protocol.answer(Token::new(channel), task, attempt, spent, end, parts)
        }
        workers::Up::Call { .. } => panic!("scenarios send typed calls"),
        workers::Up::Text { .. } => panic!("conformance scripts write no provisional text"),
    }
}

/// The worker's evidence of one assignment: its slot, the referee's view of
/// the reservation and conversation, and the words the start carried.
pub(crate) fn assigned(peers: &mut Peers, channel: Token, assignment: engine::Assignment) {
    assert_eq!(channel.raw(), CHANNEL);
    peers.observed.push(r::Observed::Assigned {
        task: assignment.task,
        attempt: assignment.attempt,
        conversation: assignment.conversation,
        resumed: !assignment.transcript.is_empty(),
        budget: assignment.run.budget,
        settings: assignment.run.revision,
    });
    for word in &assignment.messages {
        peers.observed.push(r::Observed::Reached(r::Receipt::Word(assignment.task, word.number)));
    }
    peers.worker.assign(workers::Assignment {
        conversation: assignment.conversation,
        task: assignment.task,
        attempt: assignment.attempt,
        budget: assignment.run.budget,
        transcript: assignment.transcript.clone(),
        messages: assignment.messages.iter().map(|word| word.number).collect(),
    });
    if !peers.assignments.iter().any(|a| a.task == assignment.task && a.attempt == assignment.attempt) {
        peers.assignments.push(assignment);
    }
}

/// One record of the hub's link down to the worker, and the durable
/// receipts its release requires.
pub(crate) fn down(peers: &mut Peers, now: Time, down: host::LinkDown) -> Vec<r::Receipt> {
    match down {
        host::LinkDown::Sources { sources } => {
            assert!(sources.is_empty(), "this fixture declares no program sources");
            Vec::new()
        }
        host::LinkDown::Check { .. } | host::LinkDown::SignOut { .. } => {
            panic!("this fixture declares no program sources")
        }
        host::LinkDown::TurnKept { run, attempt, turn } => {
            peers.worker.acknowledge_turn(run.raw(), attempt.raw(), turn);
            vec![r::Receipt::Turn(run.raw(), attempt.raw(), turn)]
        }
        host::LinkDown::Acknowledge { run, attempt } => {
            peers.worker.acknowledge_answer(run.raw(), attempt.raw());
            vec![r::Receipt::Terminal(run.raw(), attempt.raw())]
        }
        host::LinkDown::Cancel { run, attempt } => {
            peers.worker.cancel(run.raw(), attempt.raw());
            Vec::new()
        }
        host::LinkDown::Message { run, attempt, name, words, .. } => {
            let receipt = r::Receipt::Word(run.raw(), name.raw());
            peers.observed.push(r::Observed::Reached(receipt.clone()));
            peers.worker.message(run.raw(), attempt.raw(), name.raw(), words);
            vec![receipt]
        }
        host::LinkDown::Relayed { call, answer, .. } => relayed(peers, now, &call, answer),
        // Grants carry secret-free generations the scripted worker does not use.
        host::LinkDown::Grant { .. } => Vec::new(),
        host::LinkDown::Assign { .. } => panic!("temper's root hands assignments as typed deliveries"),
    }
}

/// A relayed answer ends its call; a busy one is asked again later. The
/// stand renders each answer as its call's name, so a replayed call maps
/// back to the typed answer the root decided.
fn relayed(peers: &mut Peers, now: Time, call: &[u8], answer: host::Relayed) -> Vec<r::Receipt> {
    let name = u64::from_be_bytes(call.try_into().expect("the world names calls by eight bytes"));
    let text = match answer {
        host::Relayed::Answer { error: false, text } => text,
        host::Relayed::Answer { error: true, .. } | host::Relayed::Busy => {
            peers.retries.insert(name, now.as_nanos() + RETRY);
            return Vec::new();
        }
    };
    // A render lost with its engine is made again; the settled one is the last.
    let (_, answer) =
        peers.protocol.answers.iter().rev().find(|(named, _)| *named == text).expect("the stand rendered every answer");
    let answer = answer.clone();
    // A call asked again may be answered twice; the worker took the first.
    let Some((task, attempt, body)) = peers.calls.remove(&name) else { return Vec::new() };
    peers.retries.remove(&name);
    if let engine::CallAnswer::Proposed { proposal, .. } = &answer {
        peers.pending_proposal = Some(*proposal);
    }
    let mut requires = Vec::new();
    if !matches!(answer, engine::CallAnswer::ForgeEffectDenied { answer: jig_domain_authority::Answer::Wait, .. }) {
        requires.push(r::Receipt::Call(task, attempt, body.completion, body.position));
    }
    peers.answers.push(answer);
    requires
}
