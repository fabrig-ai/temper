use jig_client_world::harness::{ClientId, Until, World};
use skein_fake_person::{Find, Step};
use skein_world::domain::Verdict;
use temper_client_tools::jig::Printed;
use temper_client_world::{app::Temper, fixtures, front::Joint};

#[test]
fn a_chat_the_agent_starts_is_seen_by_the_person() {
    let (mut world, _agent, task, started) = chat(false);
    let person = world.client(fixtures::PARTY, fixtures::chat_page(task));
    world.person(person, vec![Step::See { find: Find::text(b"Hello"), within: fixtures::WITHIN }]);
    assert_eq!(world.run(Until::Done).verdict, Verdict::Passed);
    assert_chat(&started, task);
    assert_eq!(fixtures::chats(&world.front().engine), 1);
}

fn call(world: &mut World<Temper, Joint>, agent: ClientId, words: &[&[u8]]) -> Printed {
    let number = fixtures::give(world, agent, words);
    fixtures::answered(world, agent, number)
}

fn chat(park_runs: bool) -> (World<Temper, Joint>, ClientId, u64, Printed) {
    let mut world = fixtures::world(fixtures::SEED);
    if park_runs {
        world.front().engine.hello(skein_lib::Token::new(7));
        world.front().park_runs = true;
    }
    let agent = fixtures::agent(&mut world);
    let printed = call(&mut world, agent, &[b"chat", b"start", b"--project", b"1", b"--key", b"hello", b"Hello"]);
    let task = fixtures::done_task(&printed);
    (world, agent, task, printed)
}

fn assert_chat(printed: &Printed, task: u64) {
    assert_eq!((printed.status, printed.stdout.as_ref()), (0, format!("done: task {task}, chat\n").as_bytes()));
}

fn wait_for_reply(world: &mut World<Temper, Joint>, agent: ClientId, task: u64) -> Printed {
    let task = task.to_string();
    call(world, agent, &[b"task", b"show", task.as_bytes(), b"--until", b"replied"])
}

#[test]
fn the_agent_waits_for_its_chats_reply() {
    let (mut world, agent, task, started) = chat(true);
    let printed = wait_for_reply(&mut world, agent, task);
    assert_chat(&started, task);
    assert_eq!(printed.status, 0, "{printed:?}");
    assert!(printed.stdout.ends_with(b"met\n"), "{printed:?}");
    assert_eq!(
        world.front().engine.task(task).expect("parked chat").phase,
        jig_domain_tasks::Phase::Active(jig_domain_tasks::Active::Idle)
    );
}

#[test]
fn a_message_is_followed_by_its_reply_waited_for() {
    let (mut world, agent, task, started) = chat(true);
    wait_for_reply(&mut world, agent, task);
    let first_attempt = world.front().engine.task(task).expect("live chat").attempt;
    let printed =
        call(&mut world, agent, &[b"task", b"message", task.to_string().as_bytes(), b"More", b"--key", b"more"]);
    let reply = wait_for_reply(&mut world, agent, task);
    assert_chat(&started, task);
    assert_eq!((printed.status, printed.stdout.as_ref()), (0, format!("done: task {task}, message\n").as_bytes()));
    assert_eq!(reply.status, 0, "{reply:?}");
    assert!(reply.stdout.ends_with(b"met\n"), "{reply:?}");
    assert_eq!(world.front().engine.task(task).expect("live chat").attempt, first_attempt + 1);
    assert_eq!(
        world.front().engine.task(task).expect("chat parked after More").phase,
        jig_domain_tasks::Phase::Active(jig_domain_tasks::Active::Idle)
    );
}

#[test]
fn a_chat_the_agent_closes_ends() {
    let (mut world, agent, task, started) = chat(true);
    wait_for_reply(&mut world, agent, task);
    world.front().park_runs = false;
    let closed = call(&mut world, agent, &[b"chat", b"close", task.to_string().as_bytes(), b"--key", b"bye"]);
    let number =
        fixtures::give(&mut world, agent, &[b"task", b"show", task.to_string().as_bytes(), b"--until", b"ended"]);
    // The later point read crosses the same client's wire after its watch.
    // Its answer is an event barrier while the closing run is still held here.
    let current = call(&mut world, agent, &[b"task", b"show", task.to_string().as_bytes()]);
    assert_eq!(current.status, 0, "{current:?}");
    assert!(world.root(agent).agent().printed(number).is_none(), "watch waits before the run ends");
    let attempt = world.front().engine.task(task).expect("closing chat").attempt;
    world.front().engine.answer(
        skein_lib::Token::new(7),
        task,
        attempt,
        0,
        jig_domain_tasks::End::Finished {
            result: jig_domain_tasks::TaskResult::Report { words: b"Closed by the run".as_slice().into() },
            cancel_delegates: false,
        },
        temper_engine_domain::Parts::none(),
    );
    let printed = fixtures::answered(&mut world, agent, number);
    assert_chat(&started, task);
    assert_eq!((closed.status, closed.stdout.as_ref()), (0, format!("done: task {task}, close\n").as_bytes()));
    assert!(printed.stdout.starts_with(format!("task {task} ").as_bytes()), "{printed:?}");
    assert!(
        printed
            .stdout
            .windows(b"result: Completed \xc2\xb7 Closed by the run\n".len())
            .any(|w| w == b"result: Completed \xc2\xb7 Closed by the run\n"),
        "{printed:?}"
    );
    assert_eq!(printed.status, 0, "{printed:?}");
    assert!(printed.stdout.ends_with(b"met\n"), "{printed:?}");

    // The committed ended row must exist before opening another wait; this
    // exercises real watch refusal and the authorized historical read path.
    let key = temper_engine_domain::Key::Core(jig_domain_core::Key::Tasks(jig_domain_tasks::Key::Ended(task)));
    let Some(temper_engine_domain::Record::Core(jig_domain_core::Record::Tasks(jig_domain_tasks::Stored::Ended(row)))) =
        world.front().engine.store.rows.get(&key)
    else {
        panic!("task end was committed before the late wait");
    };
    let jig_domain_tasks::Phase::Ended(jig_domain_tasks::Ending::Done(jig_domain_tasks::TaskResult::Report { words })) =
        &row.phase
    else {
        panic!("the committed task has its completed report");
    };
    assert_eq!(words.as_ref(), b"Closed by the run");
    assert!(world.front().engine.task(task).is_none(), "live task name was retired");
    let late = call(&mut world, agent, &[b"task", b"show", task.to_string().as_bytes(), b"--until", b"ended"]);
    assert_eq!(late.status, 0, "{late:?}");
    assert_eq!(late.stdout, printed.stdout, "late wait prints the same real final report");
    assert!(late.stderr.is_empty(), "{late:?}");
}

// The fixture forge has repository acme/shop as 7 on forge 1.
const FORGE: u16 = 1;
const REPOSITORY: u32 = 7;
// A successful adoption is one admitted application, per protocol/forge.md.
const ADOPTIONS: usize = 1;
#[test]
fn a_repository_the_agent_adopts_is_held_by_the_engine() {
    let mut world = fixtures::world(fixtures::SEED);
    let agent = fixtures::agent(&mut world);
    let printed = call(
        &mut world,
        agent,
        &[
            b"repository",
            b"adopt",
            b"--project",
            b"1",
            b"--owner",
            b"acme",
            b"--name",
            b"shop",
            b"--landing",
            b"main",
            b"--key",
            b"shop",
        ],
    );
    assert_eq!(printed.status, 0, "{printed:?}");
    assert!(world.front().engine_adopted(FORGE, REPOSITORY));
    assert_eq!(world.front().applications_admitted(), ADOPTIONS);
}

#[test]
fn a_held_task_the_agent_releases_from_the_printed_act_runs_again() {
    let mut world = fixtures::world(fixtures::SEED);
    let (_goal, held) = fixtures::goal_with_held_delegate(world.front());
    let agent = fixtures::agent(&mut world);
    let printed = call(&mut world, agent, &[b"task", b"show", held.to_string().as_bytes()]);
    let mut words = fixtures::printed_act(&printed.stdout, b"task release");
    for word in &mut words {
        if word.as_ref() == b"KEY" {
            *word = Box::from(*b"release");
        }
    }
    let words: Vec<&[u8]> = words.iter().map(AsRef::as_ref).collect();
    let printed = call(&mut world, agent, &words);
    assert_eq!((printed.status, printed.stdout.as_ref()), (0, format!("done: task {held}, release\n").as_bytes()));
    assert_eq!(world.front().engine.task(held).expect("released delegate").tries, jig_domain_tasks::Tries::NONE);
}

#[test]
fn a_cut_question_is_answered_through_the_tools_and_reaches_the_chat() {
    use jig_domain_tasks::{MessageKind, Party};
    const TEXT_BOUND: u32 = 60;
    const QUESTION_BYTES: usize = 61;
    let mut world = fixtures::world(fixtures::SEED);
    let words = vec![b'q'; QUESTION_BYTES];
    let chat = fixtures::question_with_text_bound(world.front(), &words, Some(TEXT_BOUND));
    let (question, asking) = {
        let task = world.front().engine.task(chat).expect("asked chat");
        let question = task.inbox.iter().find(|word| word.kind == MessageKind::Question).expect("question");
        let Party::Task(asking) = question.from else { panic!("asking delegate") };
        (question.number, asking)
    };
    let agent = fixtures::agent(&mut world);
    let inbox = call(&mut world, agent, &[b"inbox", b"--kind", b"questions"]);
    assert_eq!(inbox.status, 0, "{inbox:?}");
    let shown = std::str::from_utf8(&inbox.stdout).expect("UTF-8");
    assert!(shown.contains(&format!("{} (cut)\n", "q".repeat(usize::try_from(TEXT_BOUND).expect("bound")))), "{shown}");
    assert!(shown.contains(&format!("read whole: temper ctl DIR task activity {asking}")), "{shown}");
    let answer = call(
        &mut world,
        agent,
        &[
            b"task",
            b"answer",
            chat.to_string().as_bytes(),
            question.to_string().as_bytes(),
            b"Go ahead.",
            b"--key",
            b"long-question-answer",
        ],
    );
    assert_eq!(answer.status, 0, "{answer:?}");
    assert!(world.front().engine.task(chat).expect("answered chat").inbox.iter().any(|word| {
        word.from == Party::Person(fixtures::PARTY)
            && word.kind == MessageKind::Answer { question }
            && word.words.as_ref() == b"Go ahead."
    }));
}
