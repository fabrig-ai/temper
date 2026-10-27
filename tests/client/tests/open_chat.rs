//! Open chats through the person and the real engine, client/README.md, 3.5–3.6.
use jig_client_world::harness::{ClientId, Until, World};
use jig_domain_tasks as tasks;
use skein_fake_person::{Find, Step};
use skein_ui::Role;
use skein_world::domain::Verdict;
use temper_client_domain::Address;
use temper_client_world::{app::Temper, fixtures, front::Joint};
use temper_engine_domain as engine;

type Pages = World<Temper, Joint>;
const QUESTION: &[u8] = b"A chat that stays open";
const SECOND_MESSAGE: &[u8] = b"And then?";
const TWO_ANSWERS: u32 = 2;
const NO_ANSWERS: u32 = 0;
const NO_ATTEMPT: u64 = 0;
const REFUSAL: &[u8] = b"Next run refused \xc2\xb7 This model runs through a program. A chat can't stay open through a program yet. To chat now, choose a model that doesn't run through a program.";

fn see(find: Find) -> Step<Address> {
    Step::See { find, within: fixtures::WITHIN }
}

fn start_chat(world: &mut Pages) -> (ClientId, Vec<Step<Address>>) {
    world.front().answer_runs = true;
    world.front().reply_runs = true;
    let person = world.client(fixtures::PARTY, fixtures::chats_page());
    let steps = vec![
        see(Find::named(Role::TextArea, b"Start a chat")),
        Step::Type { find: Find::named(Role::TextArea, b"Start a chat"), words: QUESTION.into() },
        Step::Send { find: Find::named(Role::TextArea, b"Start a chat") },
    ];
    (person, steps)
}

#[test]
fn an_open_chat_stays_live_takes_a_second_message_and_is_closed() {
    let mut world = fixtures::open_world(fixtures::SEED);
    let (person, mut steps) = start_chat(&mut world);
    steps.extend([
        see(Find::text(b"Answered, run 1")),
        // client/browser.md, 4.6: the conversation's words and roles need no browser.
        see(Find::exactly_named(Role::Log, b"Conversation")),
        see(Find::exactly_named(Role::Paragraph, b"You")),
        see(Find::named(Role::Paragraph, "Today · ".as_bytes())),
        see(Find::named(Role::Time, b":")),
        see(Find::text(b"waiting on your words")),
        see(Find::exactly_named(Role::TextArea, b"Message")),
        see(Find::exactly_named(Role::Button, b"Send")),
        Step::Type { find: Find::named(Role::TextArea, b"Message"), words: SECOND_MESSAGE.into() },
        Step::Press { find: Find::named(Role::Button, b"Send") },
        see(Find::text(b"Answered, run 2")),
        Step::Press { find: Find::named(Role::Button, b"Close") },
        see(Find::named(Role::Dialog, b"Close")),
        see(Find::text(b"Close chat now")),
        see(Find::text(b"A closed chat can't be reopened.")),
        Step::Press { find: Find::named(Role::Button, b"Confirm") },
        // The world's epoch clock stays within its first minute throughout this story.
        see(Find::text(b"Closed 00:00")),
        Step::Gone { find: Find::named(Role::TextArea, b"Message"), within: fixtures::WITHIN },
        Step::Press { find: Find::exactly_named(Role::Link, b"Chats") },
        see(Find::named(Role::Heading { level: 2 }, b"Your chats")),
        see(Find::text(QUESTION)),
        see(Find::text(b"Closed").within(Box::new([skein_fake_person::Target::ExactRole {
            role: Role::List,
            name: b"Your chats".as_slice().into(),
        }]))),
    ]);
    world.person(person, steps);
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
    let front = world.front();
    let chat = front
        .engine
        .store
        .rows
        .values()
        .find_map(|row| {
            if let engine::Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Ended(chat))) = row {
                Some(chat)
            } else {
                None
            }
        })
        .expect("the engine holds the ended chat");
    assert!(front.engine.task(chat.number).is_none(), "the closed chat left live work");
    assert!(
        matches!(&chat.phase,
            tasks::Phase::Ended(tasks::Ending::Cancelled { by: tasks::Party::Person(fixtures::PARTY), reason, .. })
                if reason.as_ref() == b"Chat closed"
        ),
        "the requester closed the chat: {:?}",
        chat.phase
    );
    assert_eq!(front.runs_answered, TWO_ANSWERS);
}

#[test]
fn a_refused_chat_says_why_and_starts_nothing() {
    let mut world = fixtures::refusing_world(fixtures::SEED);
    let (person, mut steps) = start_chat(&mut world);
    steps.extend([
        see(Find::text(b"Cannot start yet")),
        see(Find::text(REFUSAL)),
        see(Find::named(Role::TextArea, b"Message")),
        see(Find::named(Role::Button, b"Send")),
    ]);
    world.person(person, steps);
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
    let front = world.front();
    assert_eq!(front.runs_answered, NO_ANSWERS);
    let chat = front
        .engine
        .store
        .rows
        .values()
        .find_map(|row| {
            if let engine::Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Live(chat))) = row {
                Some(chat)
            } else {
                None
            }
        })
        .expect("the refused chat stays live");
    assert_eq!(chat.attempt, NO_ATTEMPT, "no run was assigned");
    assert_eq!(chat.tries, tasks::Tries::NONE);
}
