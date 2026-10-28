//! The root keeps these orders whatever admits work (skein's draft
//! `dynamic-memory.md`, 4.3): pending callbacks before ordinary entrances,
//! store answers in arrival order, older releases before later ones, each
//! worker channel's records in order, deferred calls fenced by their attempt,
//! and completion outputs after their commits.

use jig_domain_core as core;
use jig_domain_core::reads;
use jig_domain_host as host;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{Duration, ReplyTo, Token};
use std::collections::VecDeque;
use temper_engine_domain::{self as engine, Delivery, Record, Write};
use temper_engine_domain_world::{direct::Driver, protocol, roles, walking::config};

const HISTORY: Token = Token::new(950);
const ROSTER: Token = Token::new(951);
const READ_A: Token = Token::new(960);
const READ_B: Token = Token::new(961);
const WORKER: Token = Token::new(7);
const TURN_TEXT: &[u8] = b"kept turn";
const REPORT: &[u8] = b"finished work";
// The direct fixture's finite settlement budget.
const STEPS: usize = 200;

// Fill until admission itself witnesses the watermark. The unadmitted probe
// belongs to this pressure setup, so it does not join the story's input order.
fn fill_commits(driver: &mut Driver) {
    driver.fill_to_watermark();
}

/// O1: a load's callbacks, left in the root's work queue at the
/// backlog watermark, run before a later party change.
#[test]
fn completed_history_callbacks_precede_a_later_party_change() {
    let seed = 9204;
    // This fixture runs escalation's completed history, then gives the saved
    // policy permission for the later roster change.
    let world = roles::World::new(roles::Settings::calm(seed, roles::Base::Rejected));
    let archive = world
        .store
        .rows
        .values()
        .find_map(|row| {
            if let Record::Core(jig_domain_core::Record::Core(jig_domain_core::CoreRecord::EscalationDecision(
                archive,
            ))) = row
            {
                Some(archive.clone())
            } else {
                None
            }
        })
        .expect("actual immutable escalation decision");
    let session = world.store.header().sign_ins;
    let mut driver = Driver::configured(world.store, [config(seed), config(seed)], &roles::limits());
    driver.settle();
    driver.send(engine::Event::Party(engine::PartyInput::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(HISTORY),
        sign_in: session,
        key: [9; 16],
        ask: people::Ask::DecideEscalation {
            project: archive.project,
            task: archive.task,
            revision: archive.revision,
            decision: people::EscalationDecision::Release,
        },
    }));
    let loaded = take_history_load(&mut driver);
    fill_commits(&mut driver);
    driver.send(loaded);
    let returned = driver.returned;
    driver.send(engine::Event::Party(engine::PartyInput::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(ROSTER),
        sign_in: session,
        key: [10; 16],
        ask: people::Ask::SetRoles {
            project: archive.project,
            holdings: Box::new([people::Holding { person: archive.by, role: people::Role::Owner }]),
        },
    }));
    assert_eq!(driver.returned, returned + 1, "the later party change is handed back at the backlog watermark");
    assert!(keyed_replies(&driver).is_empty(), "neither request answers before callback durability");
    driver.settle();
    assert_eq!(
        keyed_replies(&driver),
        [
            (
                HISTORY,
                people::Reply::Outcome(people::Outcome::EscalationDecided {
                    task: archive.task,
                    revision: archive.revision,
                    by: archive.by,
                    at: archive.at,
                    choice: people::EscalationChoice::Rejected,
                })
            ),
            (ROSTER, people::Reply::Outcome(people::Outcome::RolesSet { project: archive.project })),
        ],
        "history callback answers before the later roster change, each exactly once"
    );
    assert!(driver.root.quiescent() && !driver.stopped);
}

fn take_history_load(driver: &mut Driver) -> engine::Event {
    for _ in 0..STEPS {
        if let Some(index) = driver
            .events
            .iter()
            .position(|event| matches!(event, engine::Event::Store(engine::StoreInput::Loaded { .. })))
        {
            return driver.events.remove(index).expect("historical load terminal");
        }
        driver.advance(true);
    }
    panic!("historical request issues its load within the direct world's budget");
}

fn keyed_replies(driver: &Driver) -> Vec<(Token, people::Reply)> {
    driver
        .delivered
        .iter()
        .filter_map(|delivery| {
            if let Delivery::WebReply { to, reply } = delivery {
                [HISTORY, ROSTER]
                    .into_iter()
                    .find(|token| *to == ReplyTo::new(*token))
                    .map(|token| (token, reply.clone()))
            } else {
                None
            }
        })
        .collect()
}

/// O2: two read answers, taken at the backlog watermark, answer in
/// the order they arrived.
#[test]
fn queued_read_answers_keep_their_arrival_order_under_load() {
    let (mut driver, assignment) = Driver::chat();
    driver.send(protocol::turn(WORKER, assignment.task, assignment.attempt, 1, 3, None, TURN_TEXT));
    driver.settle();
    let second = read_turn(&mut driver, &assignment, READ_B);
    let first = read_turn(&mut driver, &assignment, READ_A);
    // Completion order deliberately differs from request order.
    fill_commits(&mut driver);
    let before_probe = driver.returned;
    driver.sign_in();
    assert_eq!(
        driver.returned,
        before_probe + 1,
        "the backlog watermark binds admission before the two read answers arrive"
    );
    let returned = driver.returned;
    driver.send(first);
    driver.send(second);
    assert_eq!(driver.returned, returned, "both read terminals are taken while the watermark binds");
    assert!(read_replies(&driver).is_empty(), "neither answer has bypassed prior durability");
    driver.settle();
    assert_eq!(
        read_replies(&driver),
        [READ_A, READ_B],
        "read replies follow terminal arrival order, each exactly once"
    );
    for delivery in &driver.delivered {
        if let Delivery::Read { answer, .. } = delivery {
            let reads::Answer::Historical(past) = answer else { panic!("authenticated historical read: {answer:?}") };
            let reads::Past::Turn(turn) = &past.value else { panic!("the requested historical turn") };
            assert_eq!(turn.transcript.as_ref(), TURN_TEXT, "authenticated read returned the durable turn");
        }
    }
    assert!(driver.root.quiescent() && !driver.stopped);
}

fn read_turn(driver: &mut Driver, assignment: &engine::Assignment, reply: Token) -> engine::Event {
    driver.send(engine::Event::Party(engine::PartyInput::Read {
        reply_to: ReplyTo::new(reply),
        sign_in: driver.session(),
        read: reads::Read::Turn { task: assignment.task, attempt: assignment.attempt, turn: 1 },
    }));
    for _ in 0..STEPS {
        if let Some(index) = driver
            .events
            .iter()
            .position(|event| matches!(event, engine::Event::Store(engine::StoreInput::ReadLoaded { .. })))
        {
            return driver.events.remove(index).expect("real read terminal");
        }
        driver.advance(true);
    }
    panic!("authenticated read issues its load within the direct world's budget");
}

fn read_replies(driver: &Driver) -> Vec<Token> {
    driver
        .delivered
        .iter()
        .filter_map(|delivery| {
            if let Delivery::Read { reply, .. } = delivery {
                [READ_A, READ_B].into_iter().find(|token| *reply == ReplyTo::new(*token))
            } else {
                None
            }
        })
        .collect()
}

/// O3: a turn kept before its final answer is acknowledged before it,
/// even when new work waits at the backlog watermark.
#[test]
fn turn_kept_precedes_terminal_acknowledgement_under_callback_pressure() {
    let (mut driver, assignment) = Driver::chat();
    driver.send(protocol::turn(WORKER, assignment.task, assignment.attempt, 1, 3, None, TURN_TEXT));
    until_commits(&mut driver, 1);
    driver.answer(
        WORKER,
        assignment.task,
        assignment.attempt,
        3,
        tasks::End::Finished { result: tasks::TaskResult::Report { words: REPORT.into() }, cancel_delegates: false },
        engine::Parts::none(),
    );
    until_commits(&mut driver, 2);
    fill_commits(&mut driver);
    // Make both callbacks durable, refilling to the watermark after each
    // commit answer, before offering the callbacks.
    for _ in 0..2 {
        commit_one(&mut driver);
        fill_commits(&mut driver);
    }
    for _ in 0..STEPS {
        engine::release(&mut driver.root, &driver.env, &mut driver.out);
        if driver.out.is_empty() {
            break;
        }
        driver.collect();
    }
    let older = driver.events.pop_front().expect("durable turn callback");
    assert!(matches!(older, engine::Event::Released(_)), "the oldest pending input is a released callback");
    let returned = driver.returned;
    driver.send_retaining(older);
    assert_eq!(driver.returned, returned, "the older durable release is taken even at the watermark");
    assert!(worker_notices(&driver).is_empty(), "both worker acknowledgements still wait on their callbacks");
    settle_retaining(&mut driver);
    assert_eq!(
        worker_notices(&driver),
        [
            &host::LinkDown::TurnKept {
                run: Token::new(assignment.task),
                attempt: Token::new(assignment.attempt),
                turn: 1
            },
            &host::LinkDown::Acknowledge { run: Token::new(assignment.task), attempt: Token::new(assignment.attempt) },
        ],
        "the worker sees its kept turn before its final acknowledgement, each once"
    );
    let writes: Vec<_> = driver.transactions.iter().flatten().collect();
    assert_eq!(
        writes
            .iter()
            .filter(|write| matches!(write,
                Write::Save(Record::Core(jig_domain_core::Record::Core(jig_domain_core::CoreRecord::Turn(turn))))
                    if turn.task == assignment.task && turn.attempt == assignment.attempt && turn.turn == 1
            ))
            .count(),
        1,
        "turn is committed exactly once"
    );
    assert_eq!(writes.iter().filter(|write| matches!(write,
        Write::Save(Record::Core(jig_domain_core::Record::Core(jig_domain_core::CoreRecord::Terminal(terminal))))
            if terminal.task == assignment.task && terminal.attempt == assignment.attempt
    )).count(), 1, "final result is committed exactly once");
    assert_eq!(
        driver
            .delivered
            .iter()
            .filter(|delivery| matches!(delivery,
                Delivery::Result { task, words, .. } if *task == assignment.task && words.as_ref() == REPORT
            ))
            .count(),
        1,
        "durable result is delivered exactly once"
    );
    assert!(driver.root.quiescent() && !driver.stopped, "the root settles without stopping its journal");
}

/// O4: a worker channel's records apply in send order under load, including
/// its turns, terminal and subsequent channel loss.
#[test]
fn one_channels_records_apply_in_order_under_load() {
    let (mut driver, assignment) = Driver::chat();
    assert!(assignment.workspace.repositories.is_empty(), "this chat holds no forge writer");
    driver.fill_to_watermark();
    let answer = finished_answer(&mut driver, &assignment, 6);
    // The service holds one refused head; later channel records wait behind
    // it. settle_retaining offers each through send_retaining, in this order.
    driver.events.extend([
        protocol::turn(WORKER, assignment.task, assignment.attempt, 1, 3, None, TURN_TEXT),
        protocol::turn(WORKER, assignment.task, assignment.attempt, 2, 6, None, b"second turn"),
        answer,
        protocol::lost(WORKER),
    ]);
    settle_retaining(&mut driver);
    let mut records = Vec::new();
    let mut keys = Vec::new();
    for (commit, key) in attempt_records(&driver, &assignment) {
        if !matches!(key, core::CoreKey::RunProof(_)) {
            records.push((commit, key));
            keys.push(key);
        }
    }
    assert_eq!(
        keys,
        [
            core::CoreKey::Turn { task: assignment.task, attempt: assignment.attempt, turn: 1 },
            core::CoreKey::Turn { task: assignment.task, attempt: assignment.attempt, turn: 2 },
            core::CoreKey::Terminal { task: assignment.task, attempt: assignment.attempt },
        ],
        "each turn and the terminal commits exactly once, in channel order"
    );
    for pair in records.windows(2) {
        assert!(pair[0].0 < pair[1].0, "the three records have ordered commits");
    }
    let terminal = driver.store.rows.get(&engine::Key::Core(core::Key::Core(core::CoreKey::Terminal {
        task: assignment.task,
        attempt: assignment.attempt,
    })));
    assert!(
        matches!(terminal,
            Some(Record::Core(core::Record::Core(core::CoreRecord::Terminal(terminal))))
                if terminal.end == tasks::End::Finished {
                    result: tasks::TaskResult::Report { words: REPORT.into() }, cancel_delegates: false,
                }
        ),
        "the answer finishes the attempt before the channel loss can end it lost"
    );
    // Loss precedes the durability callbacks: the fleet sends neither kept
    // turns nor the final acknowledgement to an absent channel (jig's
    // turn.rs:116 and attempt.rs:511). O3 covers their order on a live channel.
    assert!(worker_notices(&driver).is_empty(), "the lost channel receives no worker acknowledgements");
    assert_eq!(result_count(&driver, &assignment), 1, "the result leaves exactly once");
    assert!(driver.env.now.as_nanos() >= driver.env.limits.fleet.grace.as_nanos(), "the lost channel's grace expires");
    assert!(driver.root.quiescent() && !driver.stopped, "the channel loss settles without stopping the root");
}

/// O5: a deferred new call cannot act for its attempt after that attempt's
/// terminal commits, whether the call or the terminal passes admission first.
#[test]
fn a_call_held_at_the_watermark_does_not_act_for_an_attempt_that_ended_behind_it() {
    const CALL: u64 = 970;
    let (mut driver, assignment) = Driver::chat();
    driver.fill_to_watermark();
    let returned = driver.returned;
    let call = driver.protocol.call(
        WORKER,
        assignment.task,
        assignment.attempt,
        CALL,
        engine::Call {
            completion: 1,
            position: 0,
            tool: engine::Tool::Delegate {
                batch: Box::new([temper_engine_domain_world::fixtures::report_delegate(b"investigate", Box::new([]))]),
            },
        },
    );
    driver.send(call);
    assert_eq!(driver.returned, returned + 1, "new work is handed back at the watermark");
    let answer = finished_answer(&mut driver, &assignment, 0);
    driver.send(answer);
    settle_worker_records(&mut driver);

    let mut relayed = Vec::new();
    for delivery in &driver.delivered {
        if let Delivery::Link { channel, down: host::LinkDown::Relayed { run, attempt, call, answer } } = delivery
            && *channel == WORKER
            && run.raw() == assignment.task
            && attempt.raw() == assignment.attempt
            && call.as_ref() == CALL.to_be_bytes()
        {
            relayed.push(answer);
        }
    }
    assert_eq!(relayed.len(), 1, "the held call gets exactly one answer at its host");
    assert!(
        matches!(relayed[0], host::Relayed::Answer { .. } | host::Relayed::Busy),
        "the call settles or is refused/busy"
    );
    let records = attempt_records(&driver, &assignment);
    let mut terminals = Vec::new();
    for record in &records {
        if matches!(record.1, core::CoreKey::Terminal { .. }) {
            terminals.push(record);
        }
    }
    assert_eq!(terminals.len(), 1, "one terminal commits for the run");
    let terminal_commit = terminals[0].0;
    for (commit, _) in records {
        assert!(commit <= terminal_commit, "no record follows its attempt's terminal");
    }
    for writes in driver.transactions.iter().skip(terminal_commit + 1) {
        for write in writes {
            assert!(
                !matches!(write,
                    Write::Save(Record::Core(core::Record::Tasks(tasks::Stored::Live(task))))
                        if task.requester == tasks::Party::Task(assignment.task)
                ),
                "no delegate is made for the ended attempt"
            );
            assert!(
                !matches!(write, Write::Save(Record::Forge { .. })),
                "no forge effect is made for the ended attempt"
            );
        }
    }
    assert_worker_notices(&driver, &assignment, &[]);
    assert!(driver.root.quiescent() && !driver.stopped, "the held call and terminal settle without stopping");
}

/// O6: completions offered at the watermark release their worker notices
/// and result only after the commits that make them durable.
#[test]
fn completions_taken_at_the_watermark_release_nothing_before_their_commit() {
    let (mut driver, assignment) = Driver::chat();
    driver.sign_in();
    driver.pressure();
    let applied = driver.store.applied;
    driver.send(protocol::turn(WORKER, assignment.task, assignment.attempt, 1, 3, None, TURN_TEXT));
    let answer = finished_answer(&mut driver, &assignment, 3);
    driver.send(answer);
    for _ in 0..STEPS {
        driver.advance(false);
    }
    assert_eq!(driver.store.applied, applied, "the story withholds every commit answer");
    assert!(worker_notices(&driver).is_empty(), "no kept turn or terminal acknowledgement bypasses durability");
    assert_eq!(result_count(&driver, &assignment), 0, "no result bypasses durability");
    for _ in 0..STEPS {
        if !driver.store.pending.is_empty() {
            commit_one(&mut driver);
        }
        driver.advance(false);
        assert_releases_durable(&driver, &assignment);
        if driver.root.quiescent() && driver.events.is_empty() && driver.store.pending.is_empty() {
            break;
        }
    }
    assert_worker_notices(&driver, &assignment, &[1]);
    assert_eq!(result_count(&driver, &assignment), 1, "the committed result leaves once");
    assert!(driver.root.quiescent() && !driver.stopped, "completion callbacks settle within the budget");
}

fn until_commits(driver: &mut Driver, count: usize) {
    for _ in 0..STEPS {
        if driver.store.pending.len() == count {
            return;
        }
        driver.advance(false);
    }
    panic!("the worker's decision issues its commit within the direct world's budget");
}

fn commit_one(driver: &mut Driver) {
    let number = driver.store.apply();
    driver.send_retaining(engine::Event::Store(engine::StoreInput::Committed { number, fill: 0 }));
}

fn worker_notices(driver: &Driver) -> Vec<&host::LinkDown> {
    driver
        .delivered
        .iter()
        .filter_map(|delivery| {
            if let Delivery::Link {
                channel,
                down: down @ (host::LinkDown::TurnKept { .. } | host::LinkDown::Acknowledge { .. }),
            } = delivery
                && *channel == WORKER
            {
                Some(down)
            } else {
                None
            }
        })
        .collect()
}

fn settle_retaining(driver: &mut Driver) {
    for _ in 0..STEPS {
        if let Some(event) = driver.events.pop_front() {
            driver.send_retaining(event);
        }
        if !driver.store.pending.is_empty() {
            commit_one(driver);
        }
        temper_engine_domain_world::poll(&mut driver.root, &driver.env, &mut driver.out);
        driver.collect();
        driver.root.reclaim();
        if driver.root.quiescent() && driver.events.is_empty() && driver.store.pending.is_empty() {
            return;
        }
        if driver.events.is_empty()
            && driver.store.pending.is_empty()
            && let Some(deadline) = driver.root.next_deadline(&driver.env)
        {
            driver.fire_at(Duration::from_nanos(deadline.as_nanos()));
        }
    }
    panic!("retained-head root inputs did not settle: {:?}", driver.root);
}

/// Edge, kept by the old watermark: the store's commits in flight
#[test]
fn the_root_sends_no_more_commits_than_the_store_takes_in_flight() {
    // Three store credits and 22 door outputs give the default mark of 25.
    let mut limits = temper_engine_domain_world::walking::limits();
    limits.journal.deliveries = 22;
    limits.watermark = engine::default_watermark(&limits.journal);
    let mut driver =
        Driver::configured(temper_engine_domain_world::store::Store::new(), [config(91), config(91)], &limits);
    driver.settle();
    assert_eq!(driver.env.limits.journal.commits, 3);
    assert_eq!(driver.env.limits.watermark, 25);
    driver.sign_in();
    driver.settle();
    let first = driver.store.applied + 1;
    let transactions = driver.transactions.len();
    let delivered = driver.delivered.len();
    let mut admitted = Vec::new();
    let mut refused = false;
    for _ in 0..=driver.env.limits.watermark {
        let returned = driver.returned;
        let probe_at = driver.events.len();
        driver.sign_in();
        if driver.returned > returned {
            driver.events.remove(probe_at).expect("new work handed back at the watermark");
            refused = true;
            break;
        }
        admitted.push(ReplyTo::new(Token::new(driver.serial)));
        assert!(driver.store.pending.len() <= 3);
        assert_eq!(driver.store.applied + 1, first, "the story has answered no commit");
    }
    assert!(refused, "the backlog's watermark hands new work back");
    assert!(admitted.len() > 3, "the uncapped journal holds admitted decisions past the store edge");
    assert_eq!(driver.store.pending.len(), 3, "the store's credit is fully used");
    let mut answered = Vec::new();
    for _ in 0..STEPS {
        if let Some((number, _)) = driver.store.pending.front() {
            assert_eq!(*number, first + u64::try_from(answered.len()).expect("small burst"));
            answered.push(*number);
            commit_one(&mut driver);
        }
        driver.advance(false);
        assert!(driver.store.pending.len() <= 3);
        if driver.root.quiescent() && driver.events.is_empty() && driver.store.pending.is_empty() {
            break;
        }
    }
    assert!(driver.root.quiescent() && !driver.stopped);
    assert_eq!(answered.len(), admitted.len(), "each admitted decision is sent and answered once, in order");
    assert_eq!(driver.transactions.len() - transactions, admitted.len(), "no duplicate commit is sent");
    let mut replies = Vec::new();
    for delivery in &driver.delivered[delivered..] {
        if let Delivery::WebReply { to, .. } = delivery {
            replies.push(to);
        }
    }
    assert_eq!(
        replies,
        admitted.iter().collect::<Vec<_>>(),
        "each admitted decision releases its answer once, in order"
    );
}
fn finished_answer(driver: &mut Driver, assignment: &engine::Assignment, spent: u64) -> engine::Event {
    driver.answer_event(
        WORKER,
        assignment.task,
        assignment.attempt,
        spent,
        tasks::End::Finished { result: tasks::TaskResult::Report { words: REPORT.into() }, cancel_delegates: false },
        engine::Parts::none(),
    )
}

fn attempt_records(driver: &Driver, assignment: &engine::Assignment) -> Vec<(usize, core::CoreKey)> {
    let mut records = Vec::new();
    for (commit, writes) in driver.transactions.iter().enumerate() {
        for write in writes {
            let Write::Save(Record::Core(core::Record::Core(row))) = write else { continue };
            let (task, attempt) = match row {
                core::CoreRecord::Call(call) => (call.key.task, call.key.attempt),
                core::CoreRecord::Turn(turn) => (turn.task, turn.attempt),
                core::CoreRecord::Terminal(terminal) => (terminal.task, terminal.attempt),
                core::CoreRecord::RunProof(proof) => (proof.task, proof.attempt),
                core::CoreRecord::Deployment(_)
                | core::CoreRecord::Settings(_)
                | core::CoreRecord::Projection(_)
                | core::CoreRecord::ProposalDecision(_)
                | core::CoreRecord::EscalationDecision(_) => continue,
            };
            if task == assignment.task && attempt == assignment.attempt {
                records.push((commit, row.key()));
            }
        }
    }
    records
}

fn assert_worker_notices(driver: &Driver, assignment: &engine::Assignment, turns: &[u32]) {
    let mut expected = Vec::new();
    for &turn in turns {
        expected.push(host::LinkDown::TurnKept {
            run: Token::new(assignment.task),
            attempt: Token::new(assignment.attempt),
            turn,
        });
    }
    expected.push(host::LinkDown::Acknowledge {
        run: Token::new(assignment.task),
        attempt: Token::new(assignment.attempt),
    });
    let notices = worker_notices(driver);
    assert_eq!(notices.len(), expected.len(), "each worker notice leaves once");
    for (notice, expected) in notices.into_iter().zip(&expected) {
        assert_eq!(notice, expected, "kept turns precede the acknowledgement");
    }
}

fn result_count(driver: &Driver, assignment: &engine::Assignment) -> usize {
    let mut count = 0;
    for delivery in &driver.delivered {
        if matches!(delivery,
                Delivery::Result { task, words, .. } if *task == assignment.task && words.as_ref() == REPORT
        ) {
            count += 1;
        }
    }
    count
}

// Complete one returned worker record's continuations before replaying the
// next. On main both records return, so the call finishes first; after the
// completion split the answer may already be taken and only the call remains.
fn settle_worker_records(driver: &mut Driver) {
    let mut deferred = VecDeque::new();
    for _ in 0..driver.events.len() {
        let event = driver.events.pop_front().expect("counted inputs");
        if matches!(
            event,
            engine::Event::Worker(engine::WorkerInput::Link {
                link: engine::Link::Up { up: host::LinkUp::Call { .. } | host::LinkUp::Answer { .. }, .. },
                ..
            })
        ) {
            deferred.push_back(event);
        } else {
            driver.events.push_back(event);
        }
    }
    driver.settle();
    for event in deferred {
        driver.send(event);
        driver.settle();
    }
}

fn assert_releases_durable(driver: &Driver, assignment: &engine::Assignment) {
    for notice in worker_notices(driver) {
        let key = match notice {
            host::LinkDown::TurnKept { turn, .. } => {
                core::CoreKey::Turn { task: assignment.task, attempt: assignment.attempt, turn: *turn }
            }
            host::LinkDown::Acknowledge { .. } => {
                core::CoreKey::Terminal { task: assignment.task, attempt: assignment.attempt }
            }
            other @ (host::LinkDown::Sources { .. }
            | host::LinkDown::Check { .. }
            | host::LinkDown::SignOut { .. }
            | host::LinkDown::Assign { .. }
            | host::LinkDown::Message { .. }
            | host::LinkDown::Relayed { .. }
            | host::LinkDown::Cancel { .. }
            | host::LinkDown::Grant { .. }) => panic!("worker_notices returned an unrelated notice: {other:?}"),
        };
        assert!(
            driver.store.rows.contains_key(&engine::Key::Core(core::Key::Core(key))),
            "the notice's record is durable"
        );
    }
    if result_count(driver, assignment) != 0 {
        let key = core::CoreKey::Terminal { task: assignment.task, attempt: assignment.attempt };
        assert!(
            driver.store.rows.contains_key(&engine::Key::Core(core::Key::Core(key))),
            "the result's terminal is durable"
        );
    }
}
