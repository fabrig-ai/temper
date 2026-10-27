//! The stock funds B2 at init's default model prices (protocol/configuration.md, 13.4).
use super::*;
use jig_domain_tasks as tasks;
use temper_engine_domain_forge as forge;
use temper_engine_domain_world::{fixtures, forge_stand::ForgeStand, protocol};

const K: u64 = 4228;
const CHANNEL: Token = Token::new(7);

fn box_model() -> engine::Model {
    let document: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../../../../crates/temper-init/template/temper.json"))
            .expect("init document");
    let model = &document["engine"]["llm"]["models"][0];
    let mut value = walking::models()[0].clone();
    value.dialect = u32::try_from(document["engine"]["llm"]["endpoints"][0]["dialect"].as_u64().expect("dialect"))
        .expect("bounded dialect");
    value.name = model["name"].as_str().expect("model name").as_bytes().into();
    value.window = u32::try_from(model["window"].as_u64().expect("window")).expect("bounded window");
    value.output = u32::try_from(model["output"].as_u64().expect("output")).expect("bounded output");
    value.input_price = model["prices"]["input"].as_u64().expect("input price");
    value.cached_price = model["prices"]["cached"].as_u64().expect("cache price");
    value.cache_write_price = model["prices"]["cache_write"].as_u64();
    value.output_price = model["prices"]["output"].as_u64().expect("output price");
    value.price_unit =
        u32::try_from(model["prices"]["unit"].as_u64().expect("price unit")).expect("bounded price unit");
    value
}

fn configuration() -> engine::Config {
    let mut config = stock_config();
    let model = box_model();
    let llm = jig_protocol_config::Llm {
        endpoints: Box::new([]),
        models: Box::new([model.clone()]),
        descriptions: Box::new([None]),
        providers: Box::new([]),
    };
    config.core.defaults = temper_service::stock::defaults(&llm);
    config.core.settings.llm_providers.providers[0].models = Box::new([model.name.clone()]);
    config.core.settings.models = Box::new([model]);
    config.core.seed_projects = walking::config(SEED).core.seed_projects;
    config.core.seed_projects[0].policy =
        config.core.defaults.policies.get(&POLICY_NUMBER).expect("stock policy 1").clone();
    config.core.owners[0].project = 1;
    config.forge_charter = 2;
    config.landing.deployment = Box::new([engine::LandingRule {
        connector: 1,
        kind: 4,
        enforce: true,
        pattern: authority::Pattern {
            segments: Box::new([b"forge".as_slice().into()]),
            last: authority::Last::Open(Box::new([])),
        },
        ci: true,
        up_to_date: true,
        gates: Box::new([engine::Gate { number: 1, blocking: true, freshness: engine::Freshness::Exact }]),
        approvals: Box::new([]),
    }]);
    config
}

fn limits() -> engine::Limits {
    let mut limits = stock_limits();
    limits.tasks.tasks = 16;
    limits.tasks.project_tasks = 16;
    limits.tasks.tree_tasks = 16;
    limits.tasks.funders = 8;
    limits.tasks.depth = 4;
    limits.tasks.delegates = 4;
    limits.tasks.references = 4;
    limits.tasks.batch = 2;
    limits.tasks.parameters = 8;
    limits.tasks.spec_bytes = 512;
    limits.tasks.result_bytes = 512;
    limits.tasks.message_bytes = 4096;
    limits.tasks.inbox_bytes = 8192;
    limits.tasks.inbox_messages = 8;
    limits.tasks.run_inbox = 8;
    limits.tasks.authority_grants = 16;
    limits.tasks.executor_kinds = 8;
    // The stock's read plus six change grants narrow to eight repository patterns (200 bytes).
    limits.tasks.authority_bytes = 256;
    limits.tasks.authority_segments = 8;
    limits.tasks.contract_choices = 2;
    limits.authority.segments = 8;
    limits.authority.batch = 2;
    limits.authority.requirements = 4;
    limits.authority.facts = 4;
    limits.authority.writes = 2;
    limits.people.amendment_bytes = 1024;
    limits.people.inbox_tasks = 32;
    limits.people.inbox_recipients = 32;
    limits.fleet.slots = 4;
    limits.fleet.attempts = 4;
    limits.fleet.workstreams = 4;
    limits.forge.tasks = 16;
    limits.journal.deliveries = 128;
    limits.journal.held = 512;
    limits.journal.run_bytes = 16384;
    limits.journal.result_bytes = 1024;
    limits.journal.transcript_bytes = 32768;
    limits.loads.transcript_bytes = 32768;
    limits.loads.reply_bytes = 32768;
    limits.brief.sections = 8;
    limits.brief.read_bytes = 512;
    limits.brief.budgets.task = 384;
    limits.brief.budgets.pull = 384;
    limits.brief.budgets.ci = 384;
    limits.brief.brief_bytes = 1024;
    limits.call_records = 8;
    limits.journal.writes = walking::writes(&limits);
    limits
}

fn assigned(driver: &Driver, task: u64) -> Option<engine::Assignment> {
    for delivery in driver.delivered.iter().rev() {
        if let Delivery::Assigned { assignment, .. } = delivery
            && assignment.task == task
        {
            return Some(assignment.clone());
        }
    }
    None
}

fn child_run(driver: &mut Driver, parent: u64) -> engine::Assignment {
    for _ in 0..30 {
        for delivery in driver.delivered.iter().rev() {
            if let Delivery::Assigned { assignment, .. } = delivery
                && let Some(task) = driver.task(assignment.task)
                && task.requester == tasks::Party::Task(parent)
                && task.ended_at.is_none()
            {
                return assignment.clone();
            }
        }
        // The finite forge rate window and evidence polls use simulated time.
        driver.fire_at(Duration::from_nanos(driver.env.now.as_nanos() + 1_000_000_000));
        driver.pump();
    }
    panic!("child of {parent} was not admitted; parent: {:?}", driver.task(parent));
}

fn delegate(driver: &Driver, chat: u64, words: &[u8], descendants: u32, spend: u64) -> engine::Delegate {
    let mut value = fixtures::report_delegate(words, Box::new([]));
    value.contract = tasks::Contract::Report { words: 512 };
    value.authority = driver.task(chat).expect("chat").authority.clone();
    value.authority.budget.spend = spend;
    value.authority.delegation.tasks = descendants;
    value.authority.delegation.depth = if descendants == 0 { 0 } else { 2 };
    value
}

fn call(driver: &mut Driver, run: &engine::Assignment, number: u64, tool: engine::Tool) -> engine::CallAnswer {
    fixtures::tool_call(driver, run, number, tool);
    driver.pump();
    driver.answered(number).expect("tool answered").clone()
}

fn finish(driver: &mut Driver, run: &engine::Assignment, result: tasks::TaskResult) {
    assert_eq!(run.run.budget, K, "each agent has a full K run");
    driver.answer(
        CHANNEL,
        run.task,
        run.attempt,
        K,
        tasks::End::Finished { result, cancel_delegates: false },
        engine::Parts::none(),
    );
    driver.pump();
}

fn inbox(driver: &Driver, target: u64, from: u64, kind: &tasks::MessageKind, words: &[u8]) -> bool {
    for word in &driver.task(target).expect("inbox target").inbox {
        if word.from == tasks::Party::Task(from) && &word.kind == kind && word.words.as_ref() == words {
            return true;
        }
    }
    false
}

fn chat() -> (Driver, engine::Assignment) {
    let limits = limits();
    let mut driver = Driver::configured(Store::with_limits(&limits), [configuration(), configuration()], &limits);
    driver.period_sent = true;
    driver.forge = Some(ForgeStand::with_checks(&[(b"org", b"repo", 2)], &[b"build"]));
    let mut greeting = protocol::hello_slots(CHANNEL, limits.fleet.slots, Box::new([]));
    let Event::Worker(engine::WorkerInput::Link { link: engine::Link::Hello { hello, .. }, .. }) = &mut greeting else {
        unreachable!("hello")
    };
    let model = box_model();
    hello.bounds.models[0].name = model.name;
    hello.bounds.models[0].window = model.window;
    hello.bounds.models[0].output = model.output;
    hello.bounds.time = Duration::from_secs(120);
    hello.bounds.spend = K;
    hello.bounds.message_bytes = limits.tasks.message_bytes;
    hello.bounds.inbox = limits.tasks.run_inbox;
    driver.send(greeting);
    driver.settle();
    driver.send(Event::Party(PartyInput::Period { project: 1, period: 1, budget: 48 * K, next: None }));
    driver.settle();
    driver.send(Event::Credential { number: 1, generation: 1, valid: Duration::from_secs(3600) });
    driver.sign_in();
    driver.settle();
    driver.send(Event::Party(PartyInput::Application {
        reply_to: ReplyTo::new(Token::new(40)),
        sign_in: driver.session(),
        key: [40; 16],
        digest: [40; 32],
        scope: people::ApplicationScope::Project(1),
        carried: None,
        request: engine::Application::Forge(forge::Ask::Adopt(fixtures::adoption(1))),
    }));
    driver.settle();
    let mut ask = driver.chat_ask(1, b"Advise, then land two dependent changes");
    let template = configuration().core.defaults.templates[0].clone();
    let people::Ask::Create { batch, .. } = &mut ask else { unreachable!("chat creation") };
    let tasks::Contract::Report { words } = template.contract else { unreachable!("stock chat report") };
    batch[0].contract = people::Contract::Report { words };
    batch[0].flags = people::Flags {
        ended_by: people::EndedBy::Requester,
        conversation: template.flags.conversation,
        listed: template.flags.listed,
        retention: people::Retention::Kept,
    };
    batch[0].label = template.label;
    batch[0].llm = None;
    driver.send(Event::Party(PartyInput::Ask {
        reply_to: ReplyTo::new(Token::new(41)),
        sign_in: driver.session(),
        key: [41; 16],
        models: Box::new([]),
        ask,
    }));
    driver.pump();
    let chat = assigned(&driver, 1).expect("stock chat assigned");
    assert_eq!(chat.run.policy.model, box_model());
    assert_eq!(chat.run.budget, K);
    assert_eq!(driver.task(chat.task).expect("chat").numbers.budget, 24 * K);
    (driver, chat)
}

fn repair_change(driver: &mut Driver, coordinator: &engine::Assignment, number: u64) {
    let answer = call(
        driver,
        coordinator,
        number,
        engine::Tool::Change { repository: None, words: b"Repair twice, then land".as_slice().into() },
    );
    let engine::CallAnswer::Delegated(made) = answer else {
        panic!("change admitted: {answer:?}");
    };
    let change = made[0].task;
    assert_eq!(driver.task(change).expect("procedure").numbers.budget, 5 * K);
    let mut worker = child_run(driver, change);
    let tasks::Parameter::Bytes { value: branch, .. } = &driver.task(worker.task).expect("producer").spec.parameters[2]
    else {
        panic!("branch");
    };
    let branch = branch.clone();
    let mut workers = Vec::new();
    for round in 0..3 {
        assert_eq!(worker.charter, 2);
        assert_eq!(worker.run.policy.model, box_model());
        workers.push(worker.task);
        driver.forge.as_mut().expect("peer").push_checked(2, &branch, round == 0, round == 2);
        finish(
            driver,
            &worker,
            tasks::TaskResult::Change {
                connector: 1,
                kind: 2,
                resource: forge::tag(1, 2),
                words: b"pushed".as_slice().into(),
            },
        );
        worker = child_run(driver, change);
    }
    assert_eq!(worker.charter, 2);
    assert!(matches!(worker.run.contract, tasks::Contract::Verdict { .. }), "one review follows both repairs");
    workers.push(worker.task);
    workers.sort_unstable();
    workers.dedup();
    assert_eq!(workers.len(), 4, "producer, two distinct repairs and one review");
    finish(
        driver,
        &worker,
        tasks::TaskResult::Verdict { code: 1, fields: Box::new([]), words: b"Approved".as_slice().into() },
    );
    for _ in 0..30 {
        for stored in driver.store.rows.values() {
            if let Record::Forge { row } = stored
                && let forge::Stored::Change(row) = row.as_ref()
                && row.task == change
            {
                assert_eq!(row.change.repairs, 2);
                if matches!(row.change.state, temper_engine_domain_forge_change::State::Landed { .. }) {
                    return;
                }
            }
        }
        // Pull evidence is polled; a push does not rewrite the engine's cached CI.
        driver.fire_at(Duration::from_nanos(driver.env.now.as_nanos() + 1_000_000_000));
        driver.pump();
    }
    panic!(
        "the change lands after two repairs and its review: {:?}; task {:?}",
        driver.store.rows.get(&Key::Forge(forge::Key::Change(change))),
        driver.task(change)
    );
}

pub(super) fn story() {
    let (mut driver, chat) = chat();
    let advisor = delegate(&driver, chat.task, b"Advise", 0, K);
    let engine::CallAnswer::Delegated(made) =
        call(&mut driver, &chat, 42, engine::Tool::Delegate { batch: Box::new([advisor]) })
    else {
        panic!("advisor admitted");
    };
    let advisor = assigned(&driver, made[0].task).expect("advisor run");
    let report = [b'a'; 512];
    finish(&mut driver, &advisor, tasks::TaskResult::Report { words: report.as_slice().into() });
    assert!(inbox(&driver, chat.task, advisor.task, &tasks::MessageKind::Result(tasks::ResultKind::Report), &report));
    let first = delegate(&driver, chat.task, b"D1", 5, 11 * K);
    let mut second = delegate(&driver, chat.task, b"D2", 5, 11 * K);
    second.dependencies = Box::new([engine::Dependency::Batch(0)]);
    let engine::CallAnswer::Delegated(made) =
        call(&mut driver, &chat, 43, engine::Tool::Delegate { batch: Box::new([first, second]) })
    else {
        panic!("coordinators admitted");
    };
    let first = assigned(&driver, made[0].task).expect("D1");
    let second = made[1].task;
    assert!(assigned(&driver, second).is_none(), "D2 waits with worker capacity free");
    let engine::CallAnswer::Sent { message: question } = call(
        &mut driver,
        &first,
        44,
        engine::Tool::Message {
            target: chat.task,
            form: engine::MessageForm::Question,
            words: b"Green light?".as_slice().into(),
        },
    ) else {
        panic!("question admitted");
    };
    assert!(inbox(&driver, chat.task, first.task, &tasks::MessageKind::Question, b"Green light?"));
    assert!(matches!(
        call(
            &mut driver,
            &chat,
            45,
            engine::Tool::Message {
                target: first.task,
                form: engine::MessageForm::Answer { question },
                words: b"Go ahead".as_slice().into(),
            }
        ),
        engine::CallAnswer::Sent { .. }
    ));
    assert!(inbox(&driver, first.task, chat.task, &tasks::MessageKind::Answer { question }, b"Go ahead"));
    repair_change(&mut driver, &first, 46);
    assert!(assigned(&driver, second).is_none(), "D2 still waits until its coordinator ends");
    finish(&mut driver, &first, tasks::TaskResult::Report { words: b"D1 landed".as_slice().into() });
    let second = assigned(&driver, second).expect("D2 follows D1");
    repair_change(&mut driver, &second, 47);
    finish(&mut driver, &second, tasks::TaskResult::Report { words: b"D2 landed".as_slice().into() });
    assert_eq!(driver.store.header().tasks, 14, "chat, advisor, coordinators, procedures and eight workers");
    let chat = driver.task(chat.task).expect("chat remains open");
    assert_eq!(chat.numbers.spent_below, 11 * K, "advisor and both five-agent coordinator trees settle full K charges");
    assert_eq!(chat.run_reserved, K, "the chat keeps its own full run allowance");
}

/// A review task supplies its verdict and narrower authority; the charter supplies
/// its workspace families (jig's domain/engine.md, 7.1).
pub(super) fn reviewer() {
    let (mut driver, chat) = chat();
    let mut review = delegate(&driver, chat.task, b"Review the named head", 0, K);
    review.executor = tasks::Executor::Agent { charter: 4 };
    review.contract = tasks::Contract::Verdict {
        choices: Box::new([
            tasks::Verdict { code: 1, words: 512, fields: Box::new([]), followups: 0 },
            tasks::Verdict { code: 2, words: 512, fields: Box::new([]), followups: 0 },
        ]),
    };
    review.authority.tools = tasks::Tools(jig_domain_core::ToolFamilies::JIG.read.0);
    review.authority.delegation.kinds = Box::new([]);
    let answer = call(&mut driver, &chat, 42, engine::Tool::Delegate { batch: Box::new([review]) });
    let engine::CallAnswer::Delegated(made) = answer else { panic!("review admitted: {answer:?}") };
    let review = assigned(&driver, made[0].task).expect("stock charter 4 offered and assigned");
    assert_eq!(review.charter, 4);
    assert_eq!(review.run.policy.model, box_model());
    assert_eq!(review.run.authority.tools, tasks::Tools(jig_domain_core::ToolFamilies::JIG.read.0));
    assert!(review.run.authority.delegation.kinds.is_empty());
    assert_eq!(review.run.authority.delegation.tasks, 0);
    let smith = temper_engine_smith::charter(&review);
    assert!(smith.grants.tools.inspect);
    assert!(!smith.grants.tools.modify);
    assert!(!smith.grants.tools.shell);
    assert!(!smith.grants.agents);
    assert!(smith.outcome.change.is_none());
    assert!(smith.outcome.report.is_none());
    assert_eq!(smith.outcome.verdicts.len(), 2);
    let answer = call(
        &mut driver,
        &review,
        43,
        engine::Tool::ReadNamed {
            repository: None,
            read: skein_forge::api::Read::Branch { branch: b"main".as_slice().into() },
        },
    );
    assert!(matches!(answer, engine::CallAnswer::ForgeRead(ref result) if result.is_ok()), "{answer:?}");
    let child = delegate(&driver, review.task, b"Forbidden delegation", 0, K);
    let answer = call(&mut driver, &review, 44, engine::Tool::Delegate { batch: Box::new([child]) });
    assert!(
        matches!(&answer, engine::CallAnswer::ToolDenied { findings, .. }
        if findings.contains(&authority::Finding::Tool)),
        "{answer:?}"
    );
    assert_eq!(driver.store.header().tasks, 2, "refused delegation creates no task");
    finish(
        &mut driver,
        &review,
        tasks::TaskResult::Verdict { code: 1, fields: Box::new([]), words: b"Approved".as_slice().into() },
    );
    assert!(inbox(
        &driver,
        chat.task,
        review.task,
        &tasks::MessageKind::Result(tasks::ResultKind::Verdict { code: 1, fields: Box::new([]) }),
        b"Approved"
    ));
}
