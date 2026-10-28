//! A producer's no-change explanation is the change's hold account and the chat's news.
use super::*;
use jig_domain_core::reads;
use temper_engine_domain_forge_change::{Hold, State};

const CHANNEL: Token = Token::new(7);
const STEPS: u32 = 200;
const REASON: &[u8] = b"the repository has no search UI: it is a CSV command-line program; no files changed";

fn failed_production(reason: &[u8]) -> (World, engine::Assignment, u64) {
    let mut world = World::configured_limits(
        true,
        true,
        false,
        1000,
        Gate::default(),
        None,
        StoryLimits { result_bytes: Some(tasks::HOLD_ACCOUNT_BYTES), ..StoryLimits::default() },
    );
    world.adopt();
    world.send(protocol::hello_slots(CHANNEL, 2, Box::new([])));
    let ask = world.chat(1, b"Fix widget's empty search state");
    let chat = assigned_chat(&mut world, 92, ask);
    let engine::CallAnswer::Delegated(made) =
        super::change_tool::change(&mut world, &chat, 93, None, b"Fix widget's empty search state")
    else {
        panic!("change delegated")
    };
    let producer = super::reviews::next_assignment(&mut world, 1);
    let event = world.protocol.answer(CHANNEL, chat.task, chat.attempt, 1, tasks::End::Parked, engine::Parts::none());
    world.send(event);
    let event = world.protocol.answer(
        CHANNEL,
        producer.task,
        producer.attempt,
        5,
        tasks::End::Finished { result: tasks::TaskResult::Failure { reason: reason.into() }, cancel_delegates: false },
        engine::Parts::none(),
    );
    world.send(event);
    let change = made[0].task;
    for _ in 0..STEPS {
        world.tick();
        if matches!(live_task(&world, change).phase, tasks::Phase::Held { .. }) {
            return (world, chat, change);
        }
    }
    panic!("failed production did not hold its change");
}

fn account(world: &World, change: u64) -> &[u8] {
    let tasks::Phase::Held { why: tasks::Hold::EffectFailed, source: Some(source), .. } =
        &live_task(world, change).phase
    else {
        panic!("retained failure hold")
    };
    let tasks::HoldSource::Account { words, .. } = source.as_ref() else { panic!("producer's account") };
    words
}

#[test]
fn an_inapplicable_producers_words_reach_its_hold_and_its_chats_reply() {
    let (mut world, chat, change) = failed_production(REASON);
    assert_eq!(account(&world, change), REASON);
    let Some(Record::Forge { row }) = world.store.rows.get(&Key::Forge(forge_top::Key::Change(change))) else {
        panic!("durable change")
    };
    let forge_top::Stored::Change(row) = row.as_ref() else { panic!("durable change") };
    assert!(matches!(&row.change.state, State::Held { was, why: Hold::Failed }
        if matches!(was.as_ref(), State::Producing { .. })));
    assert!(row.pull.is_none());
    assert_eq!(world.merges, 0);
    let resumed = super::reviews::next_assignment(&mut world, 2);
    assert_eq!(resumed.task, chat.task);
    let notice = resumed
        .messages
        .iter()
        .find(|word| word.from == tasks::Party::Task(change) && word.kind == tasks::MessageKind::Words)
        .expect("chat hears its change's reason");
    assert_eq!(notice.words.as_ref(), REASON);
    // The scripted worker plays the chat's response to the words it actually received.
    let reply = format!("I could not make the change: {}", core::str::from_utf8(&notice.words).expect("explanation"));
    let read = resumed.messages.iter().map(|word| word.number).max();
    world.send(protocol::turn(CHANNEL, resumed.task, resumed.attempt, 1, 1, read, reply.as_bytes()));
    for _ in 0..STEPS {
        world.tick();
    }
    let reads::Answer::Historical(past) =
        world.party_read(reads::Read::Turn { task: resumed.task, attempt: resumed.attempt, turn: 1 })
    else {
        panic!("person reads the chat's reply")
    };
    let reads::Past::Turn(turn) = past.value else { panic!("chat turn") };
    assert_eq!(turn.transcript.as_ref(), reply.as_bytes());
    assert!(turn.transcript.windows(REASON.len()).any(|bytes| bytes == REASON));
}

#[test]
fn a_producer_failure_uses_the_hold_account_bound_even_with_a_smaller_message_bound() {
    let bound = usize::try_from(tasks::HOLD_ACCOUNT_BYTES).expect("hold bound");
    let mut reason = vec![b'x'; bound];
    reason[..REASON.len()].copy_from_slice(REASON);
    let (world, chat, change) = failed_production(&reason);
    assert!(usize::try_from(world.env.limits.tasks.message_bytes).expect("message bound") < bound);
    assert_eq!(account(&world, change), reason, "a whole hold account is never cut to the inbox bound");
    let notice = live_task(&world, chat.task)
        .inbox
        .iter()
        .find(|word| word.from == tasks::Party::Task(change) && word.kind == tasks::MessageKind::Words)
        .expect("requester notified");
    assert_eq!(notice.words.len(), usize::try_from(world.env.limits.tasks.message_bytes).expect("message bound"));
    assert!(notice.words.starts_with(REASON));
    assert!(notice.words.ends_with(b"..."));
}
