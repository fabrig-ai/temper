//! A change's agents ask no one, and its inbox explains where to send direction.
use super::*;

use super::delegates::fixtures::{BASE_COMMIT, INITIAL_COMMIT, OWNER, PUSHER, answer_producer, next_delegate};

const MAX_TICKS: u32 = 200;
const PASSES: u32 = 1000;
const MESSAGE_CALL: u64 = 700;
const MESSAGE: &[u8] = b"Can we discuss the change?";

fn change_task(world: &World, producer: u64) -> u64 {
    let tasks::Party::Task(change) = live_task(world, producer).requester else {
        panic!("producer belongs to a change")
    };
    change
}

fn holds_no_message(world: &World, task: u64) {
    let change = change_task(world, task);
    let tasks::Party::Task(chat) = live_task(world, change).requester else { panic!("change belongs to the chat") };
    let message = jig_domain_core::ToolFamilies::JIG.message.0;
    let parent_tools = live_task(world, chat).authority.tools.0;
    assert_ne!(parent_tools & message, 0, "the chat holds Message");
    assert_eq!(live_task(world, task).authority.tools.0, parent_tools & !message);
}

fn push(world: &mut World, branch: &[u8]) {
    assert_eq!(
        world.external(PUSHER, raw::Op::Git(raw::Git::Create { branch: branch.into(), commit: INITIAL_COMMIT })),
        raw::Answer::Branch(raw::Created::Created)
    );
    fake::advance(&mut world.fake, &world.fake_env, b"org/repo", branch, b"file", b"branch change", OWNER)
        .expect("producer pushed branch");
}

#[test]
fn a_changes_delegates_hold_no_message_family() {
    let (mut world, _, producer, branch) = change_world(false, PASSES, None);
    holds_no_message(&world, producer.task);
    push(&mut world, &branch);
    assert_eq!(
        fake::advance(&mut world.fake, &world.fake_env, b"org/repo", b"main", b"file", b"base change", OWNER),
        Ok(BASE_COMMIT)
    );
    answer_producer(&mut world, &producer);
    let resolution = next_delegate(&mut world);
    holds_no_message(&world, resolution.task);
    assert!(live_task(&world, resolution.task).spec.words.starts_with(b"Resolve"));

    let (mut world, _, producer, branch) = change_world_with_gate(false, PASSES, None, true);
    push(&mut world, &branch);
    answer_producer(&mut world, &producer);
    let review = next_delegate(&mut world);
    holds_no_message(&world, review.task);
    assert!(live_task(&world, review.task).spec.words.starts_with(b"Review"));

    let (mut world, _, _) = failed_ci_with_delayed_brief();
    let mut repair = None;
    for _ in 0..MAX_TICKS {
        world.tick();
        repair = world.store.rows.values().find_map(|stored| match stored {
            Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Live(row)))
                if row.spec.words.starts_with(b"Repair") =>
            {
                Some(row.number)
            }
            _ => None,
        });
        if repair.is_some() {
            break;
        }
    }
    holds_no_message(&world, repair.expect("repair made before its delayed brief"));
}

fn send_words(world: &mut World, chat: &engine::Assignment, change: u64) {
    let event = world.protocol.call(
        Token::new(7),
        chat.task,
        chat.attempt,
        MESSAGE_CALL,
        engine::Call {
            completion: 1,
            position: 2,
            tool: engine::Tool::Message { target: change, form: engine::MessageForm::Words, words: MESSAGE.into() },
        },
    );
    world.send(event);
}

fn replies(world: &World, target: u64, change: u64) -> Vec<&tasks::Word> {
    live_task(world, target).inbox.iter().filter(|word| word.from == tasks::Party::Task(change)).collect()
}

fn assert_reply(world: &World, target: u64, change: u64, chat: u64, kind: &tasks::MessageKind) {
    let found = replies(world, target, change);
    assert_eq!(found.len(), 1, "exactly one reply: {found:?}");
    assert_eq!(&found[0].kind, kind);
    assert_eq!(
        found[0].words.as_ref(),
        format!("A change takes no questions, answers or words; its requester is task {chat}.").as_bytes()
    );
    assert!(!live_task(world, change).inbox.iter().any(|word| word.words.as_ref() == MESSAGE));
}

#[test]
fn words_to_a_change_are_answered_naming_its_requester() {
    let (mut world, chat, producer, _) = change_world(true, PASSES, None);
    let change = change_task(&world, producer.task);
    let forge = world.store.rows.get(&Key::Forge(forge_top::Key::Change(change))).cloned();
    let assigned = world.assigned.len();
    send_words(&mut world, &chat, change);
    for _ in 0..MAX_TICKS {
        world.tick();
        if !replies(&world, chat.task, change).is_empty() {
            break;
        }
    }
    assert_reply(&world, chat.task, change, chat.task, &tasks::MessageKind::Words);
    assert_eq!(world.assigned.len(), assigned, "no duplicate delegate");
    assert_eq!(world.merges, 0, "no effect from the extra step");
    assert_eq!(world.store.rows.get(&Key::Forge(forge_top::Key::Change(change))), forge.as_ref());
}

#[test]
fn a_question_to_a_change_is_answered_in_the_answer_form() {
    let (mut world, chat, producer, _) = change_world(true, PASSES, None);
    let change = change_task(&world, producer.task);
    let question = world.restore_delegate_question(producer.task, MESSAGE);
    for _ in 0..MAX_TICKS {
        world.tick();
        if !replies(&world, producer.task, change).is_empty() {
            break;
        }
    }
    assert_reply(&world, producer.task, change, chat.task, &tasks::MessageKind::Answer { question });
    assert!(live_task(&world, producer.task).questions.is_empty(), "the reserved answer was used");
    assert_eq!(world.assigned.len(), 2, "no delegate made twice");
    assert_eq!(world.merges, 0);
}

#[test]
fn a_retried_step_answers_once() {
    // Lose the acknowledgement of the atomic step commit, then restore it.
    let (mut world, chat, producer, _) = change_world(true, PASSES, None);
    let change = change_task(&world, producer.task);
    send_words(&mut world, &chat, change);
    let mut cut = false;
    for _ in 0..MAX_TICKS {
        if world.tick_losing_reply_commit(chat.task) {
            cut = true;
            break;
        }
    }
    assert!(cut, "cut the step that consumes the words and saves its reply");
    world.restart(true, true);
    for _ in 0..MAX_TICKS {
        world.tick();
    }
    assert_reply(&world, chat.task, change, chat.task, &tasks::MessageKind::Words);
    assert_eq!(world.assigned.len(), 2);
    assert_eq!(world.merges, 0);
}

#[test]
fn replies_past_the_batch_are_capped() {
    let (mut world, chat, producer, _) = change_world(true, PASSES, None);
    let change = change_task(&world, producer.task);
    let batch = world.env.limits.tasks.batch;
    assert!(batch + 1 <= world.env.limits.tasks.inbox_messages, "the tiny inbox fits one past the batch");
    assert!(world.store.pending.is_empty());
    let mut header = world.store.header();
    let mut row = Box::new(live_task(&world, change).clone());
    assert!(row.inbox.is_empty());
    let mut inbox = Vec::new();
    for index in 0..=batch {
        header.messages += 1;
        inbox.push(tasks::Word {
            number: header.messages,
            from: tasks::Party::Task(if index < batch { chat.task } else { producer.task }),
            kind: tasks::MessageKind::Words,
            words: MESSAGE.into(),
            at: world.env.wall,
            hits: 1,
            eligible: true,
        });
    }
    row.inbox = inbox.into_boxed_slice();
    row.last_message = header.messages;
    row.phase = tasks::Phase::Active(tasks::Active::Due);
    header.commits += 1;
    world.store.pending.push_back((
        header.commits,
        Box::new([
            engine::Write::Save(Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Live(row)))),
            engine::Write::Save(Record::Core(jig_domain_core::Record::Core(jig_domain_core::CoreRecord::Deployment(
                header,
            )))),
        ]),
    ));
    world.store.apply();
    world.restart(true, true);
    for _ in 0..MAX_TICKS {
        world.tick();
        if live_task(&world, change).inbox.is_empty() {
            break;
        }
    }
    assert!(live_task(&world, change).inbox.is_empty(), "one step takes all offered words");
    assert_eq!(replies(&world, chat.task, change).len(), batch as usize, "the oldest words get batch replies");
    assert!(replies(&world, producer.task, change).is_empty(), "the newest word is taken unanswered");
    while let Some(fact) = world.root.pop_fact() {
        assert!(
            !matches!(fact, engine::Fact::StepRefused { task, .. } if task == change),
            "the step is admitted: {fact:?}"
        );
    }
}
