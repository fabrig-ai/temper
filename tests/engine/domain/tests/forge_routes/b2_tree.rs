//! Rung B2 under the box's stock funding and the init template's model.
use super::*;

use super::stock::box_model as model;
use jig_domain_core::{documents, reads};
use temper_engine_domain_world::forge_routes::box_limits;

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
const LOOKUP_JOINS: u32 = 1;

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
    started_world(World::configured_stock_limits(defaults, &model(), &box_limits()), defaults)
}

fn started_world(mut world: World, defaults: &jig_domain_core::settings::Defaults) -> (World, engine::Assignment) {
    // Two slots let a caller delegate before it parks; no actor needs a third.
    const SLOTS: u32 = 2;
    world.adopt();
    let mut bounds = protocol::bounds();
    bounds.models = Box::new([host::ModelBound { name: model().name, window: model().window, output: model().output }]);
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
fn charge(turns: u64, model: &engine::Model) -> u64 {
    let completion = (TOKENS_PER_TURN * model.input_price + OUTPUT_PER_TURN * model.output_price)
        .div_ceil(u64::from(model.price_unit))
        .max(1);
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
    let spent = charge(turns, &run.run.policy.model);
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

fn land_change(world: &mut World, work: &engine::Assignment) -> (u64, [u64; 3]) {
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
    (change, [producer_task, repair_task, review_task])
}

#[test]
fn b2s_tree_admits_every_run_under_the_stock() {
    // protocol/configuration.md, 13.4: W has its run and two 5K changes.
    const WORK_RUNS: u64 = 11;
    const CHANGE_RUNS: u64 = 5;
    let (mut world, chat) = stock_world();
    let tree = tree(&mut world, &chat);
    let defaults = defaults();
    let k = defaults.rules.maximum_run_spend;
    let c = defaults.rules.minimum_run_spend / u64::from(defaults.charters[0].policy.reserve_turns + 1);
    assert!(record(&world, tree.work).numbers.budget >= WORK_RUNS * k);
    for change in [tree.first, tree.second] {
        assert!(record(&world, change).numbers.budget >= CHANGE_RUNS * k);
    }
    println!("stock C={c}, K={k}");
    for (name, task) in
        [("chat", tree.chat), ("W", tree.work), ("V", tree.advisor), ("C1", tree.first), ("C2", tree.second)]
    {
        let numbers = &record(&world, task).numbers;
        println!("{name}: budget={}, spent={}, spent_below={}", numbers.budget, numbers.spent, numbers.spent_below);
    }
}

struct Tree {
    chat: u64,
    work: u64,
    advisor: u64,
    first: u64,
    second: u64,
    first_workers: [u64; 3],
    second_workers: [u64; 3],
}

fn tree(world: &mut World, chat: &engine::Assignment) -> Tree {
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
    let (first, first_workers) = land_change(world, &work);
    let work = admitted(world, work_task);
    let (second, second_workers) = land_change(world, &work);
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
    Tree { chat: chat.task, work: work_task, advisor: advisor_task, first, second, first_workers, second_workers }
}

#[test]
fn an_advisors_report_at_the_deployments_result_bound_reaches_the_work() {
    let (mut world, chat) = stock_world();
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

#[test]
fn b2s_tree_fits_the_boxs_limits() {
    let (mut world, chat) = stock_world();
    tree(&mut world, &chat);
}

struct ReadRow {
    name: &'static str,
    task: u64,
    ancestors: Vec<u64>,
    joins: u32,
    requester: Option<u64>,
}

fn read_rows(tree: &Tree) -> Vec<ReadRow> {
    // Each isolated row spends one lookup plus its zero to three task ancestors.
    const CHAT_JOINS: u32 = 1;
    const WORK_JOINS: u32 = 2;
    const CHANGE_JOINS: u32 = 3;
    const WORKER_JOINS: u32 = 4;
    let mut rows = vec![
        ReadRow { name: "chat", task: tree.chat, ancestors: vec![], joins: CHAT_JOINS, requester: None },
        ReadRow {
            name: "W",
            task: tree.work,
            ancestors: vec![tree.chat],
            joins: WORK_JOINS,
            requester: Some(tree.chat),
        },
        ReadRow {
            name: "V",
            task: tree.advisor,
            ancestors: vec![tree.work, tree.chat],
            joins: CHANGE_JOINS,
            requester: Some(tree.work),
        },
        ReadRow {
            name: "C1",
            task: tree.first,
            ancestors: vec![tree.work, tree.chat],
            joins: CHANGE_JOINS,
            requester: Some(tree.work),
        },
        ReadRow {
            name: "C2",
            task: tree.second,
            ancestors: vec![tree.work, tree.chat],
            joins: CHANGE_JOINS,
            requester: Some(tree.work),
        },
    ];
    for (change, workers, names) in [
        (tree.first, tree.first_workers, ["C1 producer", "C1 repairer", "C1 reviewer"]),
        (tree.second, tree.second_workers, ["C2 producer", "C2 repairer", "C2 reviewer"]),
    ] {
        for (task, name) in workers.into_iter().zip(names) {
            rows.push(ReadRow {
                name,
                task,
                ancestors: vec![change, tree.work, tree.chat],
                joins: WORKER_JOINS,
                requester: Some(change),
            });
        }
    }
    rows
}

fn stored_page(
    world: &mut World,
    person: u64,
    scope: reads::Scope,
    after: Option<reads::Position>,
) -> (Box<[reads::Row]>, Option<reads::Position>) {
    let result = world.party_load(person, scope, after);
    let reads::Loaded::Page { rows, next } = result else { panic!("served whole: {result:?}") };
    assert!(!rows.is_empty(), "each selected B2 task has a stored row");
    (rows, next)
}

fn row_joins(row: &reads::Row, expected: &ReadRow) -> u32 {
    assert_eq!(row.owner.as_ref().expect("owner").number, expected.task, "{}", expected.name);
    assert_eq!(row.ancestors.as_ref(), expected.ancestors, "{}", expected.name);
    assert_eq!(row.linked.iter().map(|task| task.number).collect::<Vec<_>>(), expected.ancestors, "{}", expected.name);
    // Task points join their terminal, and histories/indexes join their owner.
    let joins = LOOKUP_JOINS + u32::try_from(row.linked.len()).expect("bounded row links");
    assert_eq!(joins, expected.joins, "{}", expected.name);
    joins
}

fn task_document(answer: reads::Answer) -> Box<documents::Task> {
    match answer {
        reads::Answer::Document(value) => {
            let documents::Document::Task(task) = *value else { panic!("task document") };
            task
        }
        reads::Answer::Historical(value) => {
            let reads::Past::Task { document, .. } = value.value else { panic!("historical task") };
            document
        }
        other => panic!("served task: {other:?}"),
    }
}

fn ended_row(world: &mut World, person: u64, parent: u64, expected: &ReadRow) -> u32 {
    let scope = reads::Scope::EndedDelegates { task: parent };
    let mut after = None;
    for _ in 0..world.env.limits.tasks.tree_tasks {
        let from = after.clone().map(|after| reads::Cursor::Store { scope: scope.clone(), after: Some(after) });
        let answer = world.party_read(reads::Read::EndedDelegates { task: parent, from });
        let reads::Answer::Page(page) = answer else { panic!("ended delegates served: {answer:?}") };
        let (rows, next) = stored_page(world, person, scope.clone(), after);
        let numbers = page
            .rows
            .iter()
            .map(|row| {
                let reads::PageRow::EndedDelegate(row) = row else { panic!("ended delegate") };
                row.chip.number
            })
            .collect::<Vec<_>>();
        assert_eq!(numbers, rows.iter().map(|row| row.owner.as_ref().expect("owner").number).collect::<Vec<_>>());
        assert!(!page.cut);
        for row in &rows {
            if row.owner.as_ref().expect("owner").number == expected.task {
                return row_joins(row, expected);
            }
        }
        after = next;
        assert!(after.is_some(), "{} is in its requester's ended delegates", expected.name);
    }
    panic!("ended delegates within the tree bound");
}

#[test]
fn every_row_of_b2s_tree_reads_whole_on_small() {
    let (mut world, chat) = stock_world();
    let tree = tree(&mut world, &chat);
    let joins = temper_engine_domain_world::forge_routes::box_joins();
    let small = jig_protocol_config::profile(jig_protocol_config::Profile::Small).store.joins;
    assert_eq!(joins, small);
    assert_eq!(small, 4, "Small's stock join bound");
    assert_eq!(world.store.read_joins(), joins, "the real store uses the box allowance");
    let tasks::Party::Person(person) = record(&world, tree.chat).requester else { panic!("the person's chat") };
    for expected in read_rows(&tree) {
        // Owner membership permits the person to read all eleven tasks in this tree.
        let document = task_document(world.party_read(reads::Read::Task { task: expected.task }));
        assert_eq!(document.chip.number, expected.task);
        assert!(!document.cut, "{} task document", expected.name);
        assert_eq!(
            document.lineage.iter().map(|chip| chip.number).collect::<Vec<_>>(),
            expected.ancestors.iter().rev().copied().collect::<Vec<_>>()
        );
        let (rows, next) = stored_page(&mut world, person, reads::Scope::Task { task: expected.task }, None);
        assert!(next.is_none());
        let [row] = rows.as_ref() else { panic!("one task point") };
        let point = row_joins(row, &expected);
        let answer = world.party_read(reads::Read::History { task: expected.task, subtree: false, from: None });
        let reads::Answer::Page(page) = answer else { panic!("history served: {answer:?}") };
        assert!(!page.rows.is_empty());
        assert!(!page.cut);
        let (history, _) =
            stored_page(&mut world, person, reads::Scope::History { task: expected.task, subtree: false }, None);
        for row in &history {
            row_joins(row, &expected);
        }
        let history_page_joins =
            LOOKUP_JOINS + history.iter().map(|row| u32::try_from(row.linked.len()).expect("bounded row")).sum::<u32>();
        assert!(history_page_joins <= joins);
        let ended = expected.requester.map(|parent| ended_row(&mut world, person, parent, &expected));
        println!(
            "{}#{}: point={point}, history-first-page={history_page_joins}, ended-row={ended:?}; ancestors={:?}",
            expected.name, expected.task, expected.ancestors
        );
    }
}
