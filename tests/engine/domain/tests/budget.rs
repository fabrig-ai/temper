//! Activation alone waits on the root's transcript and turn budget (domain/root.md, 4.1).
use jig_domain_core::{self as core, reads};
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{ReplyTo, Token};
use temper_engine_domain::{self as engine, Delivery, Record};
use temper_engine_domain_world::{
    direct::Driver,
    protocol,
    store::Store,
    walking::{config, limits},
};

const WORKER: Token = Token::new(7);
const READ: Token = Token::new(900);
const BODY: &[u8] = b"committed history";
const STEPS: usize = 200;
// The fixture's configured resume and text-tail allowances, per preparation.
const ALLOWANCE: u64 = 256 + 128;

fn bounds(budget: u64) -> engine::Limits {
    let mut bounds = limits();
    bounds.memory_bytes = budget;
    bounds.tasks.tasks = 4;
    bounds.forge.tasks = 4;
    bounds.tasks.project_tasks = 4;
    bounds.tasks.project_goals = 4;
    bounds.people.requests = 16;
    bounds.people.inbox_tasks = 8;
    bounds.people.inbox_recipients = 32;
    bounds.fleet.slots = 3;
    bounds.fleet.attempts = 4;
    bounds.fleet.grants = 4;
    bounds.fleet.turns = 4;
    bounds.loads.loads = 4;
    bounds.brief.briefs = 4;
    bounds.forge.brief_sections = 4 * bounds.brief.sections * 2;
    bounds
}

// Three real parked conversations with committed history, ready to wake again.
fn parked() -> (Store, Vec<engine::Assignment>) {
    let mut driver = Driver::configured(Store::new(), [config(91), config(91)], &bounds(u64::MAX));
    driver.send(protocol::hello_slots(WORKER, 3, Box::new([])));
    driver.settle();
    driver.sign_in();
    driver.settle();
    let mut runs = Vec::new();
    for key in 1..=3 {
        let people::Ask::Create { project, mut batch } = driver.chat_ask(1, b"budget conversation") else {
            unreachable!()
        };
        batch[0].budget = people::Budget::Named;
        batch[0].authority.spend = 100;
        driver.send(engine::Event::Party(engine::PartyInput::Ask {
            models: Box::new([]),
            reply_to: ReplyTo::new(Token::new(1000 + u64::from(key))),
            sign_in: driver.session(),
            key: [key; 16],
            ask: people::Ask::Create { project, batch },
        }));
        driver.settle();
        let run = driver.last_assignment();
        assert!(
            runs.iter().all(|previous: &engine::Assignment| previous.task != run.task),
            "each ask created its own chat"
        );
        driver.send(protocol::turn(WORKER, run.task, run.attempt, 1, 1, None, BODY));
        driver.settle();
        driver.answer(WORKER, run.task, run.attempt, 1, tasks::End::Parked, engine::Parts::none());
        driver.settle();
        runs.push(run);
    }
    (driver.store.fork(), runs)
}

fn wake(driver: &mut Driver, runs: &[engine::Assignment]) {
    for (index, run) in runs.iter().enumerate() {
        let key = u8::try_from(index).expect("three tasks") + 20;
        driver.send(engine::Event::Party(engine::PartyInput::Ask {
            models: Box::new([]),
            reply_to: ReplyTo::new(Token::new(2000 + u64::from(key))),
            sign_in: driver.store.header().sign_ins,
            key: [key; 16],
            ask: people::Ask::Say { project: 1, task: run.task, words: b"wake".as_slice().into() },
        }));
    }
}

// Delay only preparation transcript pages; store commits, reads and fleet progress continue.
fn hold_transcripts(driver: &mut Driver, held: &mut Vec<engine::Event>) {
    for _ in 0..STEPS {
        let mut other = std::collections::VecDeque::new();
        while let Some(event) = driver.events.pop_front() {
            if matches!(event, engine::Event::Store(engine::StoreInput::Loaded { texts: Some(_), .. })) {
                held.push(event);
            } else {
                other.push_back(event);
            }
        }
        driver.events = other;
        driver.advance(true);
    }
}

fn assigned(driver: &Driver) -> usize {
    driver.delivered.iter().filter(|out| matches!(out, Delivery::Assigned { .. })).count()
}

#[test]
fn due_tasks_activate_within_the_byte_budget_as_earlier_runs_drain() {
    let (store, runs) = parked();
    let mut driver = Driver::cold(store, config(91), &bounds(2 * ALLOWANCE));
    driver.settle();
    driver.send(protocol::hello_slots(WORKER, 3, Box::new([])));
    driver.settle();
    wake(&mut driver, &runs);
    let mut held = Vec::new();
    hold_transcripts(&mut driver, &mut held);
    assert_eq!(held.len(), 2, "only two preparations start while their full allowances are held");
    assert_eq!(driver.root.held_bytes(), 2 * ALLOWANCE);
    driver.send(held.remove(0));
    hold_transcripts(&mut driver, &mut held);
    assert!(driver.root.held_bytes() >= driver.env.limits.memory_bytes);
    assert!(assigned(&driver) >= 1, "fleet places a completed preparation while bytes bind");
    assert_eq!(held.len(), 2, "the third activation follows the first preparation's drain");
    for event in held {
        driver.send(event);
    }
    driver.settle();
    assert_eq!(assigned(&driver), 3);
    let runs: Vec<_> = driver
        .delivered
        .iter()
        .filter_map(|out| {
            if let Delivery::Assigned { assignment, .. } = out {
                Some((assignment.task, assignment.attempt))
            } else {
                None
            }
        })
        .collect();
    for (task, attempt) in runs {
        driver.send(protocol::turn(WORKER, task, attempt, 1, 1, None, BODY));
    }
    driver.settle();
    assert_eq!(driver.root.held_bytes(), 0);
}

#[test]
fn a_restart_with_three_due_tasks_activates_them_within_the_budget() {
    let (mut store, runs) = parked();
    let mut saves = Vec::new();
    for (index, run) in runs.iter().enumerate() {
        let key = engine::Key::Core(core::Key::Tasks(tasks::Key::Live(run.task)));
        let Record::Core(core::Record::Tasks(tasks::Stored::Live(mut row))) = store.rows[&key].clone() else {
            panic!("live parked conversation")
        };
        row.phase = tasks::Phase::Active(if index == 0 { tasks::Active::Preparing } else { tasks::Active::Due });
        saves.push(Record::Core(core::Record::Tasks(tasks::Stored::Live(row))));
    }
    store.seed(saves.into_boxed_slice(), Box::new([]));
    let mut driver = Driver::cold(store, config(91), &bounds(2 * ALLOWANCE));
    driver.send(protocol::hello_slots(WORKER, 3, Box::new([])));
    // Drive restart inputs without poll/Resume so Open's own behaviour is visible.
    for _ in 0..STEPS {
        if driver.root.ready() {
            break;
        }
        if let Some(event) = driver.events.pop_front() {
            driver.send(event);
        }
        if !driver.store.pending.is_empty() {
            let number = driver.store.apply();
            driver.send(engine::Event::Store(engine::StoreInput::Committed { number, fill: 0 }));
        }
        engine::release(&mut driver.root, &driver.env, &mut driver.out);
        driver.collect();
    }
    assert!(driver.root.ready());
    assert_eq!(driver.root.held_bytes(), 0, "Open starts no preparations even with a worker connected");
    let mut held = Vec::new();
    hold_transcripts(&mut driver, &mut held);
    assert_eq!(held.len(), 2);
    driver.send(held.remove(0));
    hold_transcripts(&mut driver, &mut held);
    assert_eq!(held.len(), 2);
    for event in held {
        driver.send(event);
    }
    driver.settle();
    assert_eq!(assigned(&driver), 3);
}

fn read_history(driver: &mut Driver, run: &engine::Assignment) {
    driver.send(engine::Event::Party(engine::PartyInput::Read {
        reply_to: ReplyTo::new(READ),
        sign_in: driver.store.header().sign_ins,
        read: reads::Read::Turn { task: run.task, attempt: run.attempt, turn: 1 },
    }));
    for _ in 0..STEPS {
        driver.advance(true);
    }
    let answer = driver
        .delivered
        .iter()
        .find_map(|out| {
            if let Delivery::Read { reply, answer, .. } = out {
                (*reply == ReplyTo::new(READ)).then_some(answer)
            } else {
                None
            }
        })
        .expect("the person's read is answered while activation waits");
    let reads::Answer::Historical(past) = answer else { panic!("historical turn: {answer:?}") };
    let reads::Past::Turn(turn) = &past.value else { panic!("requested turn") };
    assert_eq!(turn.transcript.as_ref(), BODY);
}

#[test]
fn a_persons_read_is_answered_while_the_budget_binds() {
    let (store, runs) = parked();
    let mut driver = Driver::cold(store, config(91), &bounds(0));
    driver.settle();
    driver.send(protocol::hello_slots(WORKER, 3, Box::new([])));
    driver.settle();
    wake(&mut driver, &runs);
    read_history(&mut driver, &runs[0]);
    assert_eq!(assigned(&driver), 0);
    assert_eq!(driver.root.pending_assignments().len(), 0, "no preparation starts at the byte boundary");
}

#[test]
fn with_no_worker_the_budget_stops_preparations_and_nothing_else() {
    let (store, runs) = parked();
    let mut driver = Driver::cold(store, config(91), &bounds(2 * ALLOWANCE));
    driver.settle();
    wake(&mut driver, &runs);
    read_history(&mut driver, &runs[0]);
    assert_eq!(driver.root.pending_assignments().len(), 0, "without a worker the tasks remain due");
    assert_eq!(driver.root.held_bytes(), 0);
    assert_eq!(assigned(&driver), 0);
    driver.send(protocol::hello_slots(WORKER, 3, Box::new([])));
    let mut held = Vec::new();
    hold_transcripts(&mut driver, &mut held);
    assert_eq!(held.len(), 2, "on connection, the byte budget paces the previously due tasks");
    driver.send(held.remove(0));
    hold_transcripts(&mut driver, &mut held);
    assert_eq!(held.len(), 2);
    for event in held {
        driver.send(event);
    }
    driver.settle();
    assert_eq!(assigned(&driver), 3);
    assert_eq!(driver.root.held_bytes(), 0);
}
