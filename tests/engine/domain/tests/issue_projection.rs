//! A tracked goal's bounded issue is independent of its work and its close.
use jig_domain_host as host;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_fake_forge_domain::api as raw;
use skein_lib::{Duration, ReplyTo, Token};
use temper_engine_domain::{self as engine, Key, Record};
use temper_engine_domain_forge as forge;
use temper_engine_domain_world::forge_routes::{World, box_limits};
use temper_engine_domain_world::protocol;

const CHANNEL: Token = Token::new(7);
const PROJECT: u32 = 1;
const ASK_KEY: u8 = 92;
const FIRST_CALL: u64 = 100;
const STEPS: u32 = 200; // The root world's existing durable-round-trip allowance.
const CHILDREN: usize = 9;
const GOAL_BYTES: usize = 300;
const CHILD_BYTES: usize = 200;
const BODY_BYTES: usize = 512;
const COMMENT_SLOTS: usize = 8;
const REPORT: &[u8] = b"The work is complete";
const CHILD_REPORT: &[u8] = b"Task complete";
const GOAL_BUDGET: u64 = 100_000;
const CHILD_BUDGET: u64 = 10;
const SPEND: u64 = 1_000_000; // Funding is incidental to this issue-bound story.
const DELEGATION_TASKS: u32 = 13; // configuring's 13.4 sizing, not yet landed.
const INBOX_BYTES: u32 = 12_288; // configuring, director 1353: a tracked goal can delegate.
const MODEL_WINDOW: u32 = 1_048_576; // The init template's scripted stock model.
const MODEL_OUTPUT: u32 = 8192;
const PRICE_UNIT: u32 = 1_000_000;
const WORKER_SLOTS: u32 = 2; // The running goal and one report delegate.

fn model() -> engine::Model {
    engine::Model {
        dialect: 3,
        endpoint: 1,
        program: None,
        name: b"gpt-6-luna".as_slice().into(),
        window: MODEL_WINDOW,
        output: MODEL_OUTPUT,
        input_price: 1,
        cached_price: 1,
        cache_write_price: None,
        output_price: 1,
        price_unit: PRICE_UNIT,
        efforts: Box::new([]),
        effort: None,
    }
}

fn defaults() -> jig_domain_core::settings::Defaults {
    let llm = jig_protocol_config::Llm {
        endpoints: Box::new([]),
        providers: Box::new([]),
        descriptions: Box::new([None]),
        models: Box::new([model()]),
    };
    let mut defaults = temper_service::stock::defaults(&llm);
    defaults.rules.period_spend = SPEND;
    defaults.rules.ceiling.budget.spend = SPEND;
    defaults.rules.ceiling.delegation.tasks = DELEGATION_TASKS;
    for charter in &mut defaults.charters {
        charter.spend = SPEND;
    }
    let policy = &mut defaults.policies.get_mut(&PROJECT).expect("stock policy").policy;
    policy.period_spend = SPEND;
    policy.ceiling.budget.spend = SPEND;
    policy.ceiling.delegation.tasks = DELEGATION_TASKS;
    for role in &mut policy.roles {
        role.period_spend = SPEND;
        role.authority.budget.spend = SPEND;
        role.authority.delegation.tasks = DELEGATION_TASKS;
    }
    defaults
}

fn record(world: &World, task: u64) -> &tasks::TaskRecord {
    for key in [tasks::Key::Live(task), tasks::Key::Ended(task)] {
        if let Some(Record::Core(jig_domain_core::Record::Tasks(
            tasks::Stored::Live(row) | tasks::Stored::Ended(row),
        ))) = world.store.rows.get(&Key::Core(jig_domain_core::Key::Tasks(key)))
        {
            return row;
        }
    }
    panic!("task {task} has no durable row");
}

fn issue(world: &World, task: u64) -> &forge::IssueRow {
    let Some(Record::Forge { row, .. }) = world.store.rows.get(&Key::Forge(forge::Key::Issue(task))) else {
        panic!("goal {task} has no durable issue row");
    };
    let forge::Stored::Issue(row) = row.as_ref() else {
        panic!("issue key");
    };
    row
}

fn advance(world: &mut World) {
    for _ in 0..STEPS {
        world.tick();
    }
}

fn assigned(world: &World, task: u64) -> engine::Assignment {
    world
        .assigned
        .iter()
        .rev()
        .find(|run| run.task == task)
        .unwrap_or_else(|| panic!("task {task} not assigned: {:?}", record(world, task)))
        .clone()
}

fn call(world: &mut World, run: &engine::Assignment, tool: engine::Tool) -> engine::CallAnswer {
    let before = world.answers.len();
    let event = world.protocol.call(
        CHANNEL,
        run.task,
        run.attempt,
        FIRST_CALL + before as u64,
        engine::Call { completion: 1, position: u32::try_from(before + 1).expect("bounded calls"), tool },
    );
    world.send(event);
    for _ in 0..STEPS {
        world.tick();
        if world.answers.len() > before {
            return world.answers[before].clone();
        }
    }
    panic!("task {} call unanswered", run.task);
}

fn delegate(world: &mut World, run: &engine::Assignment, words: &[u8], tracked: bool) -> u64 {
    let mut authority = record(world, run.task).authority.clone();
    authority.delegation.tasks = if tracked { u32::try_from(CHILDREN).expect("bounded children") } else { 0 };
    authority.delegation.depth = u32::from(tracked);
    authority.budget.spend = if tracked { GOAL_BUDGET } else { CHILD_BUDGET };
    let member = engine::Delegate {
        deadline: tasks::Deadline::Named,
        tracked: if tracked { Some(1) } else { None },
        label: 0,
        flags: tasks::Flags::PLAIN,
        budget: people::Budget::Named,
        executor: tasks::Executor::Agent { charter: 1 },
        spec: tasks::Spec { words: words.into(), parameters: Box::new([]), inputs: Box::new([]) },
        contract: tasks::Contract::Report { words: 32 },
        authority,
        llm: None,
        symbolic_grants: Box::new([]),
        dependencies: Box::new([]),
        wake: tasks::WakePolicy::DEFAULT,
    };
    let answer = call(world, run, engine::Tool::Delegate { batch: Box::new([member]) });
    let engine::CallAnswer::Delegated(made) = answer else {
        panic!("delegation: {answer:?}");
    };
    made[0].task
}

fn inbox_read(world: &World, run: &engine::Assignment) -> Option<u64> {
    run.messages
        .iter()
        .map(|word| word.number)
        .chain(world.delivered_messages.get(&(run.task, run.attempt)).into_iter().flatten().copied())
        .max()
}

fn consume_inbox(world: &mut World, run: &engine::Assignment) {
    let read = inbox_read(world, run);
    let turn = record(world, run.task).turn + 1;
    world.send(protocol::turn(CHANNEL, run.task, run.attempt, turn, 0, read, b"Read the delegates' reports"));
    advance(world);
}

fn answer(world: &mut World, run: &engine::Assignment, end: tasks::End) {
    let read = inbox_read(world, run);
    let turn = record(world, run.task).turn + 1;
    world.send(protocol::turn(CHANNEL, run.task, run.attempt, turn, 0, read, b"Read the inbox"));
    let mut event = world.protocol.answer(CHANNEL, run.task, run.attempt, 0, end, engine::Parts::none());
    let engine::Event::Worker(engine::WorkerInput::Link {
        link: engine::Link::Up { up: host::LinkUp::Answer { answer, .. }, .. },
        ..
    }) = &mut event
    else {
        unreachable!("scripted worker answer");
    };
    answer.read = read.map(Token::new);
    answer.turns = turn;
    world.send(event);
    advance(world);
}

fn report(world: &mut World, run: &engine::Assignment, words: &[u8]) {
    answer(
        world,
        run,
        tasks::End::Finished { result: tasks::TaskResult::Report { words: words.into() }, cancel_delegates: false },
    );
}

fn world(failing: bool) -> (World, engine::Assignment, engine::Assignment) {
    configured_world(failing, &defaults())
}

fn configured_world(
    failing: bool,
    defaults: &jig_domain_core::settings::Defaults,
) -> (World, engine::Assignment, engine::Assignment) {
    let mut limits = box_limits();
    limits.tasks.inbox_bytes = INBOX_BYTES;
    let mut world = World::configured_stock_limits(defaults, &model(), &limits);
    world.adopt();
    world.fail_issue_writes = failing;
    let mut bounds = protocol::bounds();
    bounds.models = Box::new([host::ModelBound { name: model().name, window: MODEL_WINDOW, output: MODEL_OUTPUT }]);
    bounds.spend = defaults.rules.maximum_run_spend;
    bounds.time = defaults.charters[0].policy.time;
    bounds.message_bytes = limits.tasks.message_bytes.max(limits.tasks.result_bytes);
    bounds.inbox = limits.tasks.run_inbox;
    world.send(engine::Event::Worker(engine::WorkerInput::Link {
        channel: CHANNEL,
        link: engine::Link::Hello {
            worker: 0,
            hello: host::Hello {
                slots: WORKER_SLOTS,
                stop_bound: Duration::from_millis(100),
                workstreams: Box::new([]),
                hosting: Box::new([]),
                bounds,
            },
        },
    }));
    let people::Ask::Create { project, mut batch } = world.chat(PROJECT, b"Complete the tracked work") else {
        panic!("chat batch");
    };
    batch[0].contract = people::Contract::Report { words: 32 };
    batch[0].budget = people::Budget::AsAllowed { cap: None };
    batch[0].flags.ended_by = people::EndedBy::Requester;
    world.send(engine::Event::Party(engine::PartyInput::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(u64::from(ASK_KEY))),
        sign_in: world.signed_in.expect("owner session"),
        key: [ASK_KEY; 16],
        ask: people::Ask::Create { project, batch },
    }));
    advance(&mut world);
    let chat = world.assigned.first().expect("chat assigned").clone();
    let goal_task = delegate(&mut world, &chat, &[b'g'; GOAL_BYTES], true);
    answer(&mut world, &chat, tasks::End::Parked);
    let goal = assigned(&world, goal_task);
    (world, chat, goal)
}

fn assert_reported(world: &World, chat: u64, goal: u64) {
    let goal_row = record(world, goal);
    assert_eq!(goal_row.hold, 0, "goal was never held");
    assert!(
        matches!(&goal_row.phase, tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Report { words }))
        if words.as_ref() == REPORT),
        "goal closed: {:?}",
        goal_row.phase
    );
    assert!(
        record(world, chat).inbox.iter().any(|word| word.from == tasks::Party::Task(goal)
            && word.kind == tasks::MessageKind::Result(tasks::ResultKind::Report)
            && word.words.as_ref() == REPORT),
        "requester hears the goal's report"
    );
    assert!(
        !world.store.rows.contains_key(&Key::Core(jig_domain_core::Key::Core(jig_domain_core::CoreKey::Projection(
            jig_domain_core::ProjectionKey::Goal(goal)
        ),))),
        "final projection settled"
    );
}

#[test]
fn a_goal_with_a_long_request_and_nine_tasks_reports_and_its_issue_fits() {
    const REPORTS_PER_READ: usize = 3; // Three results plus one reserved slot fit the box's eight messages.
    let (mut world, chat, goal) = world(false);
    for index in 0..CHILDREN {
        let child = delegate(&mut world, &goal, &[b't'; CHILD_BYTES], false);
        advance(&mut world);
        let child = assigned(&world, child);
        report(&mut world, &child, CHILD_REPORT);
        if (index + 1).is_multiple_of(REPORTS_PER_READ) {
            consume_inbox(&mut world, &goal);
        }
    }
    let number = issue(&world, goal.task).number.expect("issue created");
    report(&mut world, &goal, REPORT);
    assert_reported(&world, chat.task, goal.task);
    assert_eq!(record(&world, goal.task).made, u32::try_from(CHILDREN).expect("bounded children") + 1);
    let raw::Answer::Item { item, .. } = world
        .fake
        .inspect(&world.fake_env.limits, b"org/repo", &raw::Read::Item { number, after: 0 })
        .expect("fake issue")
    else {
        panic!("item read");
    };
    let body = std::str::from_utf8(&item.body).expect("UTF-8 body");
    assert!(body.len() <= BODY_BYTES);
    assert!(item.title.len() + body.len() <= BODY_BYTES, "edit fits client operation");
    assert!(body.contains('…'));
    assert!(body.contains("2 more tasks"), "{body}");
    assert_eq!(item.state, raw::State::Closed);
    let raw::Answer::Comments(comments) = world
        .fake
        .inspect(&world.fake_env.limits, b"org/repo", &raw::Read::Comments { number })
        .expect("fake comments")
    else {
        panic!("comments read");
    };
    assert!(comments.len() <= COMMENT_SLOTS);
    assert!(
        comments
            .iter()
            .any(|comment| std::str::from_utf8(&comment.body).expect("comment").contains("more milestones; the last:"))
    );
    assert!(comments.last().expect("Finished").body.ends_with(b"Goal finished"));
}

#[test]
fn a_projection_that_keeps_failing_holds_neither_the_goal_nor_its_close() {
    let (mut world, chat, goal) = world(true);
    // Keep the same goal feed alive for multiple policy intervals.
    let interval = world.env.limits.forge.issue_policy.interval;
    let initial = world.env.wall;
    while world.env.wall.as_nanos() < initial.as_nanos() + interval.as_nanos() * 2 {
        world.tick();
    }
    report(&mut world, &goal, REPORT);
    assert_reported(&world, chat.task, goal.task);
    assert!(issue(&world, goal.task).failed);
    assert!(
        world
            .issue_write_times
            .windows(2)
            .any(|times| times[1].as_nanos() >= times[0].as_nanos() + interval.as_nanos()),
        "projection retried after interval"
    );
    assert!(world.issue_write_times.len() >= 2);
}

#[test]
fn a_refused_projection_holds_neither_the_goal_nor_its_close() {
    let mut defaults = defaults();
    defaults.policies.get_mut(&PROJECT).expect("stock policy").policy.projections = Box::new([]);
    let (mut world, chat, goal) = configured_world(false, &defaults);
    report(&mut world, &goal, REPORT);
    assert_reported(&world, chat.task, goal.task);
    assert!(issue(&world, goal.task).failed);
    assert!(world.issue_write_times.is_empty(), "policy refused before any backend write");
}
