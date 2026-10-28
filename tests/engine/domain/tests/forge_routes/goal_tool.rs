//! A goal uses ordinary delegation admission and can plan before making a change.
use super::*;

const WORDS: &[u8] = b"Plan the fix, ask me, then make it";
const ADVISOR_PREFIX: &[u8] = b"You are an advisor: you read what your brief asks and report what you found; you change nothing and ask no one.\n\n";
const GOAL_PREFIX: &[u8] = b"You are a goal: the work your person asked for, yours to plan and to change; you start no goal but an advisor.\n\n";
const AGREED_PREFIX: &[u8] = b"You are a goal: the work your person asked for, yours to plan and to change; you start no goal but an advisor.\nThe work is to be agreed: before anything is changed, ask your requester your plan with message, as a question, and wait for the answer.\n\n";
const PERSON_HEADING: &[u8] = b"\n\nYour person's words, whole:\n";
const PERSON_REQUEST: &[u8] = b"First: read the code and ask me.\nThen: keep the second paragraph too.";
const PERSON_BYTES: usize = 128; // The root world's existing people.words bound and adoption request size.
const GOAL_CALL: u64 = 93; // The fixture's first call name, after chat key 92.
const AT_BOUND: usize = 0;
const ONE_PAST_BOUND: usize = 1; // One additional byte crosses the spec's bound.
const NESTED_GOAL_CALL: u64 = GOAL_CALL + 1;
const CONTINUED_CALL: u64 = NESTED_GOAL_CALL + 1;

fn chat_world(cap: u64, tasks: u32, depth: u32) -> (World, engine::Assignment) {
    start_chat(World::configured(true, true), cap, tasks, depth, None)
}

fn start_chat(
    mut world: World,
    cap: u64,
    tasks: u32,
    depth: u32,
    person_words: Option<&[u8]>,
) -> (World, engine::Assignment) {
    world.adopt();
    world.send(protocol::hello_slots(Token::new(7), 2, Box::new([])));
    let people::Ask::Create { project, mut batch } = adoption_chat(&world) else { panic!("create") };
    if let Some(words) = person_words {
        batch[0].words = words.into();
    }
    batch[0].budget = people::Budget::AsAllowed { cap: Some(cap) };
    batch[0].contract = people::Contract::Report { words: 32 };
    batch[0].authority.tools = 1 | 2 | 512;
    batch[0].authority.delegation.tasks = tasks;
    batch[0].authority.delegation.depth = depth;
    let chat = assigned_chat(&mut world, 92, people::Ask::Create { project, batch });
    (world, chat)
}

fn call(world: &mut World, run: &engine::Assignment, name: u64, tool: engine::Tool) -> engine::CallAnswer {
    let before = world.answers.len();
    send_call(world, run, name, tool);
    for _ in 0..200 {
        world.tick();
        if world.answers.len() > before {
            return world.answers[before].clone();
        }
    }
    panic!("call was not answered");
}

fn send_call(world: &mut World, run: &engine::Assignment, name: u64, tool: engine::Tool) {
    let event = world.protocol.call(
        Token::new(7),
        run.task,
        run.attempt,
        name,
        engine::Call { completion: u32::try_from(name).expect("bounded call name"), position: 1, tool },
    );
    world.send(event);
}

fn goal(world: &mut World, run: &engine::Assignment, name: u64, advisor: bool) -> engine::CallAnswer {
    call(world, run, name, engine::Tool::Goal { agree: false, words: WORDS.into(), advisor })
}

fn live(world: &World, task: u64) -> &tasks::TaskRecord {
    let Some(Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Live(row)))) =
        world.store.rows.get(&Key::Core(jig_domain_core::Key::Tasks(tasks::Key::Live(task))))
    else {
        panic!("live task {task}")
    };
    row
}

fn delegated(answer: engine::CallAnswer) -> u64 {
    let engine::CallAnswer::Delegated(made) = answer else { panic!("delegated: {answer:?}") };
    assert_eq!(made.len(), 1);
    made[0].task
}

#[test]
fn a_chats_goal_is_unchanged() {
    let (mut world, chat) = chat_world(100, 11, 3);
    let task = delegated(goal(&mut world, &chat, 93, false));
    let row = live(&world, task);
    assert_eq!(row.executor, tasks::Executor::Agent { charter: 1 });
    assert_eq!(row.requester, tasks::Party::Task(chat.task));
    assert_eq!(row.project, live(&world, chat.task).project);
    assert_eq!(row.tracked, Some(0));
    assert!(matches!(row.contract, tasks::Contract::Report { .. }));
    assert_eq!(row.spec.words.as_ref(), [GOAL_PREFIX, WORDS].concat());
    assert!(row.spec.parameters.is_empty() && row.spec.inputs.is_empty());
    assert_eq!(delegated(goal(&mut world, &chat, 93, false)), task, "named retry makes no second goal");
}

#[test]
fn a_goals_report_is_bounded_by_the_deployments_result_bytes() {
    let (mut world, chat, run) = running_goal();
    let tasks::Contract::Report { words } = live(&world, chat.task).contract else { panic!("chat report") };
    let result_bytes = world.env.limits.tasks.result_bytes;
    assert!(words < result_bytes, "the chat's own contract is smaller");
    let advisor = delegated(goal(&mut world, &run, NESTED_GOAL_CALL, true));
    assert_eq!(live(&world, run.task).contract, tasks::Contract::Report { words: result_bytes });
    assert_eq!(live(&world, advisor).contract, tasks::Contract::Report { words: result_bytes });
}

#[test]
fn a_goals_authority_is_the_runs_narrowed() {
    let (mut world, chat) = chat_world(100, 11, 3);
    let parent = live(&world, chat.task).clone();
    let task = delegated(goal(&mut world, &chat, 93, false));
    let row = live(&world, task);
    assert_eq!(row.authority.tools, parent.authority.tools);
    assert_eq!(
        row.authority.grants,
        parent
            .authority
            .grants
            .iter()
            .filter(|g| g.connector == 0 && [1, 2, 3, 4, 10, 11, 12].contains(&g.kind))
            .cloned()
            .collect::<Box<[_]>>()
    );
    assert_eq!(row.authority.delegation.kinds, parent.authority.delegation.kinds);
    assert_eq!(row.authority.delegation.tasks, 10);
    assert_eq!(row.authority.delegation.depth, 2);
    assert_eq!(row.authority.notes, tasks::Scopes(0));
    assert!(row.authority.note_resources.is_empty());
    assert_eq!(
        row.authority.budget.spend,
        (parent.numbers.budget - parent.numbers.spent - parent.numbers.spent_below - parent.numbers.reserved) / 2
    );
    assert_eq!(row.authority.budget.deadline, parent.authority.budget.deadline);
    assert_eq!(row.authority.models, parent.authority.models);
}

#[test]
fn an_untracked_chats_goal_comes_last_among_the_projects_goals() {
    for (existing, expected) in [(8, 9), (u32::MAX, u32::MAX)] {
        let (mut world, chat) = chat_world(400, 11, 3);
        let mut first = temper_engine_domain_world::fixtures::report_delegate(b"existing goal", Box::new([]));
        first.tracked = Some(existing);
        first.authority.tools = tasks::Tools(1 | 2 | 512);
        let earlier = delegated(call(&mut world, &chat, 93, engine::Tool::Delegate { batch: Box::new([first]) }));
        let task = delegated(goal(&mut world, &chat, 94, false));
        assert_eq!(live(&world, task).tracked, Some(expected));
        assert!(task > earlier, "equal priorities are ordered by task number");
    }
}

#[test]
fn an_advisor_reads_alone_untracked() {
    let (mut world, _, run) = running_goal();
    let task = delegated(goal(&mut world, &run, NESTED_GOAL_CALL, true));
    let row = live(&world, task);
    assert_eq!(row.tracked, None);
    assert_eq!(row.authority.tools, tasks::Tools(jig_domain_core::ToolFamilies::JIG.read.0));
    assert!(!row.authority.grants.is_empty());
    assert!(row.authority.grants.iter().all(|g| g.connector == 0 && g.kind == 1));
    assert!(row.authority.delegation.kinds.is_empty());
    assert_eq!(row.authority.delegation.tasks, 0);
    assert_eq!(row.authority.delegation.depth, 0);
    assert_eq!(row.authority.notes, tasks::Scopes(0));
    assert!(row.authority.note_resources.is_empty());
}

#[test]
fn a_goal_with_one_task_left_is_refused_as_the_tree_limit() {
    let (mut world, chat) = chat_world(100, 1, 3);
    assert!(
        matches!(goal(&mut world, &chat, 93, false), engine::CallAnswer::DelegationRefused(p) if p.why == tasks::Refusal::Tree)
    );
}

#[test]
fn a_goal_with_one_level_left_is_refused_as_the_depth_limit() {
    let (mut world, chat) = chat_world(100, 11, 1);
    assert!(
        matches!(goal(&mut world, &chat, 93, false), engine::CallAnswer::DelegationRefused(p) if p.why == tasks::Refusal::Depth)
    );
}

#[test]
fn a_goal_with_half_the_remainder_at_minimum_is_refused_as_funding() {
    let (mut world, chat) = chat_world(62, 11, 3);
    assert!(
        matches!(goal(&mut world, &chat, 93, false), engine::CallAnswer::DelegationRefused(p) if p.why == tasks::Refusal::Funding)
    );
}

fn running_goal() -> (World, engine::Assignment, engine::Assignment) {
    running_goal_with_agreement(false)
}

fn running_goal_with_agreement(agree: bool) -> (World, engine::Assignment, engine::Assignment) {
    let (mut world, chat) = start_chat(World::configured(true, true), 400, 11, 3, Some(PERSON_REQUEST));
    let task = delegated(call(
        &mut world,
        &chat,
        GOAL_CALL,
        engine::Tool::Goal { words: WORDS.into(), advisor: false, agree },
    ));
    world.until(Until::SecondAssignment);
    let run = world.assigned.iter().find(|a| a.task == task).expect("goal assigned").clone();
    (world, chat, run)
}

#[test]
fn a_goal_calls_change_and_its_change_is_made() {
    let (mut world, _, run) = running_goal();
    let change = delegated(call(
        &mut world,
        &run,
        94,
        engine::Tool::Change { words: b"Small fix".as_slice().into(), repository: None },
    ));
    let row = live(&world, change);
    assert_eq!(row.requester, tasks::Party::Task(run.task));
    assert_eq!(row.executor, tasks::Executor::Procedure { connector: 0, code: 2 });
    for _ in 0..200 {
        world.tick();
        if world.store.rows.values().any(|r| {
            matches!(r, Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Live(row)))
            if row.requester == tasks::Party::Task(change) && matches!(row.executor, tasks::Executor::Agent { .. }))
        }) {
            return;
        }
    }
    panic!("change producer not delegated");
}

#[test]
fn a_goals_advisor_is_admitted() {
    let (mut world, _, run) = running_goal();
    let advisor = delegated(goal(&mut world, &run, 94, true));
    assert_eq!(live(&world, advisor).requester, tasks::Party::Task(run.task));
    assert_eq!(live(&world, advisor).tracked, None);
}

#[test]
fn a_goal_asks_its_requester_and_the_person_sees_the_question() {
    use jig_domain_core::{documents, reads};
    let (mut world, chat, run) = running_goal();
    let engine::CallAnswer::Sent { message } = call(
        &mut world,
        &run,
        94,
        engine::Tool::Message {
            target: chat.task,
            form: engine::MessageForm::Question,
            words: b"Shall I make this plan?".as_slice().into(),
        },
    ) else {
        panic!("question sent")
    };
    let reply_to = ReplyTo::new(Token::new(95));
    world.send(engine::Event::Party(engine::PartyInput::Read {
        reply_to,
        sign_in: world.signed_in.expect("owner session"),
        read: reads::Read::Inbox {
            group: reads::Group::Questions,
            filter: reads::Filter { project: Some(1), goal: None },
            from: None,
        },
    }));
    for _ in 0..200 {
        world.tick();
        if let Some((_, reads::Answer::Page(page))) =
            world.reads.iter().find(|(to, _)| *to == ReplyTo::new(Token::new(95)))
        {
            assert!(page.rows.iter().any(|r| matches!(r, reads::PageRow::Inbox(entry)
                if matches!(&entry.task, documents::TaskLink::Live(chip) if chip.number == chat.task)
                && entry.object == documents::InboxObject::Question { message })));
            return;
        }
    }
    panic!("person's inbox not read");
}

fn refused_goal(world: &mut World, run: &engine::Assignment, advisor: bool, expected: &[u8]) {
    refused_tool(
        world,
        run,
        NESTED_GOAL_CALL,
        engine::Tool::Goal { agree: false, words: WORDS.into(), advisor },
        expected,
    );
}

fn refused_tool(world: &mut World, run: &engine::Assignment, name: u64, tool: engine::Tool, expected: &[u8]) {
    world.until(Until::Ready);
    let (applied, rows) = (world.store.applied, world.store.rows.clone());
    send_call(world, run, name, tool);
    world.until(Until::Ready);
    assert_eq!(
        world.call_errors,
        [temper_engine_domain_world::forge_routes::CallError {
            call: name.to_be_bytes().into(),
            text: expected.into()
        }]
    );
    assert_eq!(world.store.applied, applied, "the refusal commits nothing");
    assert_eq!(world.store.rows, rows, "the refusal makes no task or call record");
    assert!(
        matches!(
            call(world, run, name + 1, engine::Tool::ReadForge {
                repository: forge_world::REPO,
                read: skein_forge::api::Read::Branch { branch: b"main".as_slice().into() },
            }),
            engine::CallAnswer::ForgeRead(answer) if answer.is_ok()
        ),
        "the same run's next call is served"
    );
}

#[test]
fn a_goals_goal_without_advisor_is_refused_by_name() {
    let (mut world, _, run) = running_goal();
    refused_goal(
        &mut world,
        &run,
        false,
        b"a goal starts an advisor alone: set advisor, or make the change with change",
    );
}

#[test]
fn a_chats_advisor_is_refused_by_name() {
    let (mut world, chat) = chat_world(100, 11, 3);
    refused_goal(
        &mut world,
        &chat,
        true,
        b"to change a repository, call change; a goal is for work they want to agree first or a survey, and its goal asks for an advisor",
    );
}

#[test]
fn a_goal_uses_the_subtrees_remaining_task_allotment() {
    let (mut world, chat) = chat_world(400, 11, 3);
    delegated(goal(&mut world, &chat, 93, false));
    let task = delegated(goal(&mut world, &chat, 94, false));
    assert_eq!(live(&world, task).authority.delegation.tasks, 9);
}

#[test]
fn oversized_goal_words_are_refused_as_a_delegate_spec() {
    let (mut world, chat) = chat_world(100, 11, 3);
    let words = vec![b'x'; usize::try_from(world.env.limits.tasks.spec_bytes).expect("u32 fits usize") + 1];
    assert!(
        matches!(call(&mut world, &chat, 93, engine::Tool::Goal { agree: false, words: words.into(), advisor: false }),
        engine::CallAnswer::DelegationRefused(p) if p.why == tasks::Refusal::Spec)
    );
}

#[test]
fn an_advisors_brief_opens_with_the_roots_line() {
    let (mut world, _, run) = running_goal();
    let task = delegated(goal(&mut world, &run, NESTED_GOAL_CALL, true));
    assert_eq!(live(&world, task).spec.words.as_ref(), [ADVISOR_PREFIX, WORDS].concat());
}

fn advisor_at_spec_edge(extra: usize) -> (World, engine::CallAnswer) {
    let (mut world, _, run) = running_goal();
    let bound = usize::try_from(world.env.limits.tasks.spec_bytes).expect("u32 fits usize");
    let words = vec![b'x'; bound - ADVISOR_PREFIX.len() + extra];
    let answer = call(
        &mut world,
        &run,
        NESTED_GOAL_CALL,
        engine::Tool::Goal { agree: false, words: words.into(), advisor: true },
    );
    (world, answer)
}

#[test]
fn an_advisors_brief_fits_at_the_spec_bound() {
    let (world, answer) = advisor_at_spec_edge(AT_BOUND);
    let task = delegated(answer);
    assert_eq!(
        live(&world, task).spec.words.len(),
        usize::try_from(world.env.limits.tasks.spec_bytes).expect("u32 fits usize")
    );
}

#[test]
fn an_advisors_brief_past_the_spec_bound_is_refused() {
    let (_, answer) = advisor_at_spec_edge(ONE_PAST_BOUND);
    assert!(matches!(answer, engine::CallAnswer::DelegationRefused(p) if p.why == tasks::Refusal::Spec));
}

#[test]
fn agree_with_advisor_is_refused_by_name() {
    let (mut world, chat) = chat_world(400, 11, 3);
    refused_tool(
        &mut world,
        &chat,
        NESTED_GOAL_CALL,
        engine::Tool::Goal { words: WORDS.into(), advisor: true, agree: true },
        b"an advisor agrees nothing: set agree or advisor, not both",
    );
}

#[test]
fn a_goals_goal_with_agree_is_refused_by_f3() {
    let (mut world, _, run) = running_goal();
    refused_tool(
        &mut world,
        &run,
        NESTED_GOAL_CALL,
        engine::Tool::Goal { words: WORDS.into(), advisor: false, agree: true },
        b"a goal starts an advisor alone: set advisor, or make the change with change",
    );
}

#[test]
fn an_agreed_goals_brief_carries_its_persons_words_whole() {
    let (world, _, run) = running_goal_with_agreement(true);
    let row = live(&world, run.task);
    assert!(row.spec.asks);
    assert_eq!(row.spec.words.as_ref(), [AGREED_PREFIX, WORDS, PERSON_HEADING, PERSON_REQUEST].concat());
}

#[test]
fn every_goals_brief_opens_with_the_roots_goal_line() {
    for (agree, prefix) in [(false, GOAL_PREFIX), (true, AGREED_PREFIX)] {
        let (world, _, run) = running_goal_with_agreement(agree);
        assert!(live(&world, run.task).spec.words.starts_with(prefix));
    }
}

fn agreed_at_spec_edge(extra: usize, person_words: &[u8]) -> (World, u64, Box<[u8]>) {
    let world = World::configured(true, true);
    let bound = usize::try_from(world.env.limits.tasks.spec_bytes).expect("u32 fits usize");
    let fixed = AGREED_PREFIX.len() + PERSON_HEADING.len() + PERSON_BYTES;
    let words = vec![b's'; bound - fixed + extra].into_boxed_slice();
    let (mut world, chat) = start_chat(world, 400, 11, 3, Some(person_words));
    let task = delegated(call(
        &mut world,
        &chat,
        GOAL_CALL,
        engine::Tool::Goal { words: words.clone(), advisor: false, agree: true },
    ));
    (world, task, words)
}

#[test]
fn an_agreed_brief_fits_its_persons_words_at_the_spec_bound() {
    let person = [b'x'; PERSON_BYTES];
    let (world, task, words) = agreed_at_spec_edge(AT_BOUND, &person);
    assert_eq!(live(&world, task).spec.words.as_ref(), [AGREED_PREFIX, &words, PERSON_HEADING, &person].concat());
    assert_eq!(
        live(&world, task).spec.words.len(),
        usize::try_from(world.env.limits.tasks.spec_bytes).expect("u32 fits usize")
    );
}

#[test]
fn an_agreed_brief_cuts_its_persons_words_at_the_spec_bound() {
    // One excess byte and the 16-byte root cut marker omit 17 original bytes.
    const OMITTED: usize = 17;
    let (world, task, words) = agreed_at_spec_edge(ONE_PAST_BOUND, &[b'x'; PERSON_BYTES]);
    let kept = [b'x'; PERSON_BYTES - OMITTED];
    assert_eq!(
        live(&world, task).spec.words.as_ref(),
        [AGREED_PREFIX, &words, PERSON_HEADING, &kept, b"\n[17 bytes cut]\n"].concat()
    );
    assert_eq!(
        live(&world, task).spec.words.len(),
        usize::try_from(world.env.limits.tasks.spec_bytes).expect("u32 fits usize")
    );
}

#[test]
fn an_agreed_brief_cuts_at_a_whole_character() {
    // Cutting the 17 bytes above would split é: the root omits 18 instead.
    const OMITTED: usize = 18;
    let person = "é".repeat(PERSON_BYTES / "é".len());
    let (world, task, words) = agreed_at_spec_edge(ONE_PAST_BOUND, person.as_bytes());
    let kept = "é".repeat((PERSON_BYTES - OMITTED) / "é".len());
    assert_eq!(
        live(&world, task).spec.words.as_ref(),
        [AGREED_PREFIX, &words, PERSON_HEADING, kept.as_bytes(), b"\n[18 bytes cut]\n"].concat()
    );
    assert!(core::str::from_utf8(&live(&world, task).spec.words).is_ok());
}

#[test]
fn an_agreed_goals_change_before_the_answer_is_refused_by_name() {
    let (mut world, chat, run) = running_goal_with_agreement(true);
    let engine::CallAnswer::Sent { message } = call(
        &mut world,
        &run,
        NESTED_GOAL_CALL,
        engine::Tool::Message {
            target: chat.task,
            form: engine::MessageForm::Question,
            words: b"May I go ahead?".as_slice().into(),
        },
    ) else {
        panic!("question sent")
    };
    let expected = format!("wait for the answer to question {message}");
    refused_tool(
        &mut world,
        &run,
        CONTINUED_CALL,
        engine::Tool::Change { words: b"Small fix".as_slice().into(), repository: None },
        expected.as_bytes(),
    );
}

#[test]
fn an_agreed_goals_change_before_asking_is_refused_by_name() {
    let (mut world, _, run) = running_goal_with_agreement(true);
    refused_tool(
        &mut world,
        &run,
        NESTED_GOAL_CALL,
        engine::Tool::Change { words: b"Small fix".as_slice().into(), repository: None },
        b"this work is to be agreed: ask your requester your plan with message, as a question, and wait",
    );
}

#[test]
fn an_agreed_brief_uses_the_persons_newest_consumed_chat_words() {
    const MESSAGE_KEY: u8 = 96; // Distinct from the chat creation and agent call keys.
    const STEPS: u32 = 200; // The root world's durable round-trip bound.
    const NEWEST: &[u8] = b"Newest request: read both files, then ask me before changing either.";
    let (mut world, chat) = start_chat(World::configured(true, true), 400, 11, 3, Some(PERSON_REQUEST));
    world.send(engine::Event::Party(engine::PartyInput::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(u64::from(MESSAGE_KEY))),
        sign_in: world.signed_in.expect("owner session"),
        key: [MESSAGE_KEY; 16],
        ask: people::Ask::Say { project: 1, task: chat.task, words: NEWEST.into() },
    }));
    for _ in 0..STEPS {
        world.tick();
    }
    let read = Some(live(&world, chat.task).last_message);
    world.send(protocol::turn(Token::new(7), chat.task, chat.attempt, 1, 0, read, b"Read the latest request"));
    world.until(Until::Ready);
    let task = delegated(call(
        &mut world,
        &chat,
        GOAL_CALL,
        engine::Tool::Goal { words: WORDS.into(), advisor: false, agree: true },
    ));
    assert_eq!(live(&world, task).spec.words.as_ref(), [AGREED_PREFIX, WORDS, PERSON_HEADING, NEWEST].concat());
}
