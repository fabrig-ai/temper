//! A goal uses ordinary delegation admission and can plan before making a change.
use super::*;

const WORDS: &[u8] = b"Plan the fix, ask me, then make it";
const ADVISOR_PREFIX: &[u8] = b"You are an advisor: you read what your brief asks and report what you found; you change nothing and ask no one.\n\n";
const GOAL_CALL: u64 = 93; // The fixture's first call name, after chat key 92.
const AT_BOUND: usize = 0;
const ONE_PAST_BOUND: usize = 1; // One additional byte crosses the spec's bound.

fn chat_world(cap: u64, tasks: u32, depth: u32) -> (World, engine::Assignment) {
    let mut world = World::configured(true, true);
    world.adopt();
    world.send(protocol::hello_slots(Token::new(7), 2, Box::new([])));
    let people::Ask::Create { project, mut batch } = adoption_chat(&world) else { panic!("create") };
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
    let event = world.protocol.call(
        Token::new(7),
        run.task,
        run.attempt,
        name,
        engine::Call { completion: u32::try_from(name).expect("bounded call name"), position: 1, tool },
    );
    world.send(event);
    for _ in 0..200 {
        world.tick();
        if world.answers.len() > before {
            return world.answers[before].clone();
        }
    }
    panic!("call was not answered");
}

fn goal(world: &mut World, run: &engine::Assignment, name: u64, advisor: bool) -> engine::CallAnswer {
    call(world, run, name, engine::Tool::Goal { words: WORDS.into(), advisor })
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
fn goal_delegates_one_tracked_report_under_the_runs_charter() {
    let (mut world, chat) = chat_world(100, 11, 3);
    let task = delegated(goal(&mut world, &chat, 93, false));
    let row = live(&world, task);
    assert_eq!(row.executor, tasks::Executor::Agent { charter: 1 });
    assert_eq!(row.requester, tasks::Party::Task(chat.task));
    assert_eq!(row.project, live(&world, chat.task).project);
    assert_eq!(row.tracked, Some(0));
    assert!(matches!(row.contract, tasks::Contract::Report { .. }));
    assert_eq!(row.spec.words.as_ref(), WORDS);
    assert!(row.spec.parameters.is_empty() && row.spec.inputs.is_empty());
    assert_eq!(delegated(goal(&mut world, &chat, 93, false)), task, "named retry makes no second goal");
}

#[test]
fn a_goals_report_is_bounded_by_the_deployments_result_bytes() {
    for advisor in [false, true] {
        let (mut world, chat) = chat_world(100, 11, 3);
        let tasks::Contract::Report { words } = live(&world, chat.task).contract else { panic!("chat report") };
        let result_bytes = world.env.limits.tasks.result_bytes;
        assert!(words < result_bytes, "the chat's own contract is smaller");
        let task = delegated(goal(&mut world, &chat, 93, advisor));
        assert_eq!(live(&world, task).contract, tasks::Contract::Report { words: result_bytes });
    }
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
    let (mut world, chat) = chat_world(100, 11, 3);
    let task = delegated(goal(&mut world, &chat, 93, true));
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
    let (mut world, chat) = chat_world(400, 11, 3);
    let task = delegated(goal(&mut world, &chat, 93, false));
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
fn a_goals_run_takes_goal_too() {
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

#[test]
fn a_tracked_runs_goal_keeps_its_priority() {
    let (mut world, _, run) = running_goal();
    let priority = live(&world, run.task).tracked.expect("tracked parent");
    let task = delegated(goal(&mut world, &run, 94, false));
    assert_eq!(live(&world, task).tracked, Some(priority));
}

#[test]
fn a_goal_uses_the_subtrees_remaining_task_allotment() {
    let (mut world, chat) = chat_world(400, 11, 3);
    delegated(goal(&mut world, &chat, 93, true));
    let task = delegated(goal(&mut world, &chat, 94, false));
    assert_eq!(live(&world, task).authority.delegation.tasks, 9);
}

#[test]
fn oversized_goal_words_are_refused_as_a_delegate_spec() {
    let (mut world, chat) = chat_world(100, 11, 3);
    let words = vec![b'x'; usize::try_from(world.env.limits.tasks.spec_bytes).expect("u32 fits usize") + 1];
    assert!(matches!(call(&mut world, &chat, 93, engine::Tool::Goal { words: words.into(), advisor: false }),
        engine::CallAnswer::DelegationRefused(p) if p.why == tasks::Refusal::Spec));
}

fn words_world() -> (World, engine::Assignment) {
    // The ordinary goal fixture's funding and tree capacity; only its words differ.
    const CHAT_CAP: u64 = 100;
    const TREE_TASKS: u32 = 11;
    const TREE_DEPTH: u32 = 3;
    chat_world(CHAT_CAP, TREE_TASKS, TREE_DEPTH)
}

#[test]
fn an_advisors_brief_opens_with_the_roots_line() {
    for (advisor, expected) in [(true, [ADVISOR_PREFIX, WORDS].concat()), (false, WORDS.to_vec())] {
        let (mut world, chat) = words_world();
        let task = delegated(goal(&mut world, &chat, GOAL_CALL, advisor));
        assert_eq!(live(&world, task).spec.words.as_ref(), expected.as_slice());
    }
}

fn advisor_at_spec_edge(extra: usize) -> (World, engine::CallAnswer) {
    let (mut world, chat) = words_world();
    let bound = usize::try_from(world.env.limits.tasks.spec_bytes).expect("u32 fits usize");
    let words = vec![b'x'; bound - ADVISOR_PREFIX.len() + extra];
    let answer = call(&mut world, &chat, GOAL_CALL, engine::Tool::Goal { words: words.into(), advisor: true });
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
