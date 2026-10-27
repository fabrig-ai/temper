//! Rung B2 under the box's stock funding and the init template's model.
use super::*;

// crates/temper-init/template/temper.json, engine.llm.models: gpt-6-luna.
const MODEL_WINDOW: u32 = 1_048_576;
const MODEL_OUTPUT: u32 = 8192;
const INPUT_PRICE: u64 = 1;
const OUTPUT_PRICE: u64 = 1;
const PRICE_UNIT: u32 = 1_000_000;
const TOKENS_PER_TURN: u64 = 30_000;
const OUTPUT_PER_TURN: u64 = 1000;
const CHAT_TURNS: u64 = 2;
const WORK_TURNS: u64 = 3;
const ADVISOR_TURNS: u64 = 4;
const PRODUCER_TURNS: u64 = 6;
const REPAIR_TURNS: u64 = 4;
const REVIEW_TURNS: u64 = 3;
const STEPS: u32 = 200; // The root world's existing bound for a durable round trip.
const CHANNEL: Token = Token::new(7);
const PROJECT: u32 = 1;
const FORGE: u16 = 1;
const CHAT_KEY: u8 = 92;
const FIRST_CALL: u64 = 93; // Global fixture call names must be unique across actors.
const WORK_WORDS: &[u8] = b"Survey, then land two changes in order";
const ADVISOR_REPORT: &[u8] = b"Survey complete";
const WORK_REPORT: &[u8] = b"Both changes landed";
const CHAT_REPORT: &[u8] = b"Done: both landed";

fn model() -> engine::Model {
    engine::Model {
        dialect: 3,
        endpoint: 1,
        program: None,
        name: b"gpt-6-luna".as_slice().into(),
        window: MODEL_WINDOW,
        output: MODEL_OUTPUT,
        input_price: INPUT_PRICE,
        cached_price: INPUT_PRICE,
        cache_write_price: None,
        output_price: OUTPUT_PRICE,
        price_unit: PRICE_UNIT,
        efforts: Box::new([]),
        effort: None,
    }
}

fn defaults() -> jig_domain_core::settings::Defaults {
    // This world scripts the worker; it never connects to an endpoint or real account.
    let llm = jig_protocol_config::Llm {
        endpoints: Box::new([]),
        providers: Box::new([]),
        descriptions: Box::new([None]),
        models: Box::new([model()]),
    };
    temper_service::stock::defaults(&llm)
}

fn world_with(defaults: &jig_domain_core::settings::Defaults) -> (World, engine::Assignment) {
    started_world(World::configured_stock(defaults, &model()), defaults)
}

fn started_world(mut world: World, defaults: &jig_domain_core::settings::Defaults) -> (World, engine::Assignment) {
    // Two slots let a caller delegate before it parks; no actor needs a third.
    const SLOTS: u32 = 2;
    world.adopt();
    let mut bounds = protocol::bounds();
    bounds.models = Box::new([host::ModelBound { name: model().name, window: MODEL_WINDOW, output: MODEL_OUTPUT }]);
    bounds.spend = defaults.rules.maximum_run_spend;
    bounds.time = defaults.charters[0].policy.time;
    bounds.message_bytes = world.env.limits.tasks.message_bytes.max(world.env.limits.tasks.result_bytes);
    bounds.inbox = world.env.limits.tasks.run_inbox;
    world.send(engine::Event::Worker(engine::WorkerInput::Link {
        channel: CHANNEL,
        link: engine::Link::Hello {
            worker: 0,
            hello: host::Hello {
                slots: SLOTS,
                stop_bound: Duration::from_millis(100),
                workstreams: Box::new([]),
                hosting: Box::new([]),
                bounds,
            },
        },
    }));
    // jig's Creating fixture supplies authority but hard-codes its own chat contract
    // and ending. Apply the stock template's fields instead of inheriting those.
    let people::Ask::Create { project, mut batch } = world.chat(PROJECT, WORK_WORDS) else { panic!("chat batch") };
    let template = &defaults.templates[0];
    let jig_domain_people::TemplateExecutor::Charter(charter) = template.executor else { panic!("stock charter") };
    let tasks::Contract::Report { words } = template.contract else { panic!("stock report") };
    let jig_domain_people::TemplateBudget::AsAllowed { cap } = template.budget else { panic!("stock budget") };
    batch[0].executor = people::TaskExecutor::Agent { charter };
    batch[0].contract = people::Contract::Report { words };
    batch[0].budget = people::Budget::AsAllowed { cap };
    batch[0].flags = people::Flags {
        ended_by: people::EndedBy::Requester,
        conversation: template.flags.conversation,
        listed: template.flags.listed,
        retention: people::Retention::Kept,
    };
    batch[0].label = template.label;
    batch[0].tracked = template.tracked;
    batch[0].llm = match template.llm {
        None => None,
        Some(_) => panic!("stock takes its charter model"),
    };
    let chat = assigned_chat(&mut world, CHAT_KEY, people::Ask::Create { project, batch });
    (world, chat)
}

fn stock_world() -> (World, engine::Assignment) {
    world_with(&defaults())
}

/// Smith rounds each completion's combined input/output charge upward once
/// (smith's domain/session.md, 6; smith-domain-session's record.rs).
fn charge(turns: u64) -> u64 {
    let completion = (TOKENS_PER_TURN * INPUT_PRICE + OUTPUT_PER_TURN * OUTPUT_PRICE).div_ceil(u64::from(PRICE_UNIT));
    turns * completion
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

fn admitted(world: &World, task: u64) -> engine::Assignment {
    let row = record(world, task);
    assert!(
        !matches!(row.phase, tasks::Phase::Held { .. }),
        "task {task} run held: {:?}; budget={}, spent={}, spent_below={}, reserved={}",
        row.phase,
        row.numbers.budget,
        row.numbers.spent,
        row.numbers.spent_below,
        row.numbers.reserved
    );
    let run = world
        .assigned
        .iter()
        .rev()
        .find(|run| run.task == task)
        .unwrap_or_else(|| panic!("task {task} run not assigned: {:?}; numbers={:?}", row.phase, row.numbers))
        .clone();
    assert!(
        matches!(row.phase, tasks::Phase::Active(tasks::Active::Running { attempt }) if attempt == run.attempt),
        "task {task} has no running attempt: {:?}; numbers={:?}",
        row.phase,
        row.numbers
    );
    run
}

fn advance(world: &mut World) {
    for _ in 0..STEPS {
        world.tick();
    }
}

fn call(world: &mut World, run: &engine::Assignment, tool: engine::Tool) -> engine::CallAnswer {
    let before = world.answers.len();
    let event = world.protocol.call(
        CHANNEL,
        run.task,
        run.attempt,
        FIRST_CALL + u64::try_from(before).expect("bounded calls"),
        engine::Call { completion: 1, position: 1, tool },
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

fn delegated(world: &mut World, run: &engine::Assignment, tool: engine::Tool) -> u64 {
    let answer = call(world, run, tool);
    let engine::CallAnswer::Delegated(made) = answer else {
        panic!("task {} delegation: {answer:?}; numbers={:?}", run.task, record(world, run.task).numbers)
    };
    assert_eq!(made.len(), 1);
    made[0].task
}

fn goal(world: &mut World, run: &engine::Assignment, advisor: bool) -> u64 {
    delegated(world, run, engine::Tool::Goal { words: WORK_WORDS.into(), advisor })
}

fn answer(world: &mut World, run: &engine::Assignment, turns: u64, end: tasks::End) {
    answer_parts(world, run, turns, end, engine::Parts::none());
}

fn answer_parts(world: &mut World, run: &engine::Assignment, turns: u64, end: tasks::End, parts: engine::Parts) {
    // Like park_chat, consume the assignment's inbox in a recorded turn before
    // answering. An unread result otherwise wakes W while its old writer is Lost.
    let read = run
        .messages
        .iter()
        .map(|word| word.number)
        .chain(world.delivered_messages.get(&(run.task, run.attempt)).into_iter().flatten().copied())
        .max();
    let spent = charge(turns).min(run.run.budget);
    let body = match &end {
        tasks::End::Finished { result: tasks::TaskResult::Report { words }, .. } => words.as_ref(),
        _ => b"Read messages",
    };
    world.send(protocol::turn(CHANNEL, run.task, run.attempt, 1, spent, read, body));
    world.until(Until::Ready);
    let mut event = world.protocol.answer(CHANNEL, run.task, run.attempt, spent, end, parts);
    let engine::Event::Worker(engine::WorkerInput::Link {
        link: engine::Link::Up { up: host::LinkUp::Answer { answer, .. }, .. },
        ..
    }) = &mut event
    else {
        unreachable!("scripted worker answer")
    };
    answer.read = read.map(Token::new);
    answer.turns = 1;
    world.send(event);
    advance(world);
}

fn report(world: &mut World, run: &engine::Assignment, turns: u64, words: &[u8]) {
    answer(
        world,
        run,
        turns,
        tasks::End::Finished { result: tasks::TaskResult::Report { words: words.into() }, cancel_delegates: false },
    );
}

fn has_report(world: &World, task: u64, from: u64, words: &[u8]) -> bool {
    record(world, task).inbox.iter().any(|word| {
        word.from == tasks::Party::Task(from)
            && word.kind == tasks::MessageKind::Result(tasks::ResultKind::Report)
            && word.words.as_ref() == words
    })
}

fn next_child(world: &World, parent: u64, after: u64) -> u64 {
    world
        .store
        .rows
        .values()
        .filter_map(|stored| match stored {
            Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Live(row)))
                if row.requester == tasks::Party::Task(parent) && row.number > after =>
            {
                Some(row.number)
            }
            _ => None,
        })
        .min()
        .unwrap_or_else(|| {
            panic!(
                "task {parent} has no next child after {after}; phase={:?}; numbers={:?}",
                record(world, parent).phase,
                record(world, parent).numbers
            )
        })
}

fn pushed(world: &mut World, run: &engine::Assignment, turns: u64, head: u64) {
    answer_parts(
        world,
        run,
        turns,
        tasks::End::Finished {
            result: tasks::TaskResult::Change {
                connector: FORGE,
                kind: 2,
                resource: forge_top::tag(forge_world::REPO.forge, forge_world::REPO.repository),
                words: b"pushed".as_slice().into(),
            },
            cancel_delegates: false,
        },
        engine::Parts {
            saved: None,
            pushed: Box::new([forge_top::Pushed {
                tag: forge_top::tag(forge_world::REPO.forge, forge_world::REPO.repository),
                commit: translate::commit(head),
            }]),
            preparation: None,
        },
    );
}

fn landed(world: &World, task: u64) -> bool {
    world.store.rows.values().any(|stored| {
        matches!(stored,
        Record::Forge { row, .. } if matches!(row.as_ref(), forge_top::Stored::Change(row)
            if row.task == task && matches!(row.change.state, temper_engine_domain_forge_change::State::Landed { .. })))
    })
}

fn land_change(world: &mut World, work: &engine::Assignment) -> u64 {
    let change =
        delegated(world, work, engine::Tool::Change { words: b"Small fix".as_slice().into(), repository: None });
    answer(world, work, WORK_TURNS, tasks::End::Parked);
    let producer_task = next_child(world, change, change);
    let producer = admitted(world, producer_task);
    let branch = producer.workspace.repositories[0].push.clone().expect("producer branch");
    let raw::Answer::Commit(base) =
        world.external(1, raw::Op::Read(raw::Read::Branch { branch: b"main".as_slice().into() }))
    else {
        panic!("landing branch head")
    };
    assert!(matches!(
        world.external(2, raw::Op::Git(raw::Git::Create { branch: branch.clone(), commit: base })),
        raw::Answer::Branch(raw::Created::Created)
    ));
    let head = fake::advance(&mut world.fake, &world.fake_env, b"org/repo", &branch, b"file", b"first version", 1)
        .expect("producer pushes");
    world.external(
        1,
        raw::Op::Write(raw::Write::Status {
            commit: head,
            context: b"build".as_slice().into(),
            state: raw::Check::Failed,
        }),
    );
    pushed(world, &producer, PRODUCER_TURNS, head);
    let repair_task = next_child(world, change, producer_task);
    let repair = admitted(world, repair_task);
    let head = fake::advance(&mut world.fake, &world.fake_env, b"org/repo", &branch, b"file", b"repaired", 1)
        .expect("repair pushes");
    world.external(
        1,
        raw::Op::Write(raw::Write::Status {
            commit: head,
            context: b"build".as_slice().into(),
            state: raw::Check::Passed,
        }),
    );
    pushed(world, &repair, REPAIR_TURNS, head);
    let review_task = next_child(world, change, repair_task);
    let review = admitted(world, review_task);
    answer(
        world,
        &review,
        REVIEW_TURNS,
        tasks::End::Finished {
            result: tasks::TaskResult::Verdict { code: 1, fields: Box::new([]), words: b"Approved".as_slice().into() },
            cancel_delegates: false,
        },
    );
    assert!(landed(world, change), "change {change} lands after CI repair and agent approval");
    change
}

#[test]
fn b2s_tree_admits_every_run_under_the_stock() {
    let (mut world, chat) = stock_world();
    tree(&mut world, &chat);
}

fn tree(world: &mut World, chat: &engine::Assignment) {
    let chat = admitted(world, chat.task);
    let work_task = goal(world, &chat, false);
    answer(world, &chat, CHAT_TURNS, tasks::End::Parked);
    let work = admitted(world, work_task);
    let advisor_task = goal(world, &work, true);
    answer(world, &work, WORK_TURNS, tasks::End::Parked);
    let advisor = admitted(world, advisor_task);
    assert!(matches!(call(world, &advisor, engine::Tool::ReadForge {
        repository: forge_world::REPO, read: skein_forge::api::Read::Branch { branch: b"main".as_slice().into() },
    }), engine::CallAnswer::ForgeRead(result) if result.is_ok()));
    report(world, &advisor, ADVISOR_TURNS, ADVISOR_REPORT);
    assert!(has_report(world, work_task, advisor_task, ADVISOR_REPORT), "advisor report reaches work");
    let work = admitted(world, work_task);
    let first = land_change(world, &work);
    let work = admitted(world, work_task);
    let second = land_change(world, &work);
    assert!(second > first, "C2 is created after C1's Landed was asserted");
    let work = admitted(world, work_task);
    report(world, &work, WORK_TURNS, WORK_REPORT);
    assert!(has_report(world, chat.task, work_task, WORK_REPORT));
    let chat = admitted(world, chat.task);
    report(world, &chat, CHAT_TURNS, CHAT_REPORT);
    assert!(matches!(&record(world, work_task).phase,
        tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Report { words })) if words.as_ref() == WORK_REPORT));
    // The stock chat is a conversation ended by its requester; its report is a reply.
    assert!(record(world, chat.task).last_reply > 0);
}

#[test]
fn an_advisors_report_at_the_deployments_result_bound_reaches_the_work() {
    let mut defaults = defaults();
    // Funding is incidental to the report contract: use the period's full authority.
    defaults.policies.get_mut(&PROJECT).expect("stock policy").policy.roles[0].authority.budget.spend = 100;
    let (mut world, chat) = world_with(&defaults);
    let work_task = goal(&mut world, &chat, false);
    answer(&mut world, &chat, CHAT_TURNS, tasks::End::Parked);
    let work = admitted(&world, work_task);
    let advisor_task = goal(&mut world, &work, true);
    answer(&mut world, &work, WORK_TURNS, tasks::End::Parked);
    let advisor = admitted(&world, advisor_task);
    let report_bytes = world.env.limits.tasks.result_bytes;
    let words = vec![b'x'; usize::try_from(report_bytes).expect("result byte bound")];
    report(&mut world, &advisor, ADVISOR_TURNS, &words);
    assert!(
        has_report(&world, work_task, advisor_task, &words),
        "advisor {advisor_task}'s {report_bytes}-byte report must reach work {work_task} whole; contract={:?}; phase={:?}; numbers={:?}",
        record(&world, advisor_task).contract,
        record(&world, advisor_task).phase,
        record(&world, advisor_task).numbers
    );
}

/// Build the effective engine limits through the same profile/override readers
/// and final composition as startup, reading the init template itself.
fn box_limits() -> engine::Limits {
    use jig_protocol_config as config;
    let document = config::document(
        include_bytes!("../../../../../crates/temper-init/template/temper.json"),
        &config::DocumentLimits { bytes: 1_048_576, tokens: 16_384, depth: 32 },
    )
    .expect("init template JSON");
    let mut root = config::read::Object::new(&document, 0).expect("template object");
    let engine = root.member(&document, b"engine").expect("engine member");
    let mut engine = config::read::Object::new(&document, engine).expect("engine object");
    let at = engine.member(&document, b"limits").expect("engine limits");
    let jig = config::overrides::parse(&document, at, &config::Path::root().member(b"engine").member(b"limits"))
        .expect("init limits");
    let temper = temper_service::limits::profile(temper_service::limits::Profile::Small);
    temper_service::limits::engine(&jig, &temper).expect("coherent box limits")
}

fn stock_box_world() -> (World, engine::Assignment) {
    // This test isolates capacities: even repeated halvings leave abundant funding.
    const SPEND: u64 = 1_000_000;
    let mut defaults = defaults();
    defaults.rules.period_spend = SPEND;
    defaults.rules.ceiling.budget.spend = SPEND;
    for charter in &mut defaults.charters {
        charter.spend = SPEND;
    }
    let policy = &mut defaults.policies.get_mut(&PROJECT).expect("stock policy").policy;
    policy.ceiling.budget.spend = SPEND;
    policy.period_spend = SPEND;
    for role in &mut policy.roles {
        role.authority.budget.spend = SPEND;
        role.period_spend = SPEND;
    }
    started_world(World::configured_stock_limits(&defaults, &model(), &box_limits()), &defaults)
}

#[test]
fn b2s_tree_fits_the_boxs_limits() {
    let (mut world, chat) = stock_box_world();
    tree(&mut world, &chat);
}
