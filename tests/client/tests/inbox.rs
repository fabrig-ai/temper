use jig_client_world::harness::Until;
use skein_fake_person::{Face, Find, Found, Step, TreeFace};
use skein_ui::Role;
use skein_world::domain::Verdict;
use temper_client_world::fixtures;

const QUESTION: &[u8] = b"Export widget's table as CSV? Go ahead?";
const ANSWER: &[u8] = b"Go ahead.";

#[test]
fn a_delegates_question_answered_in_the_inbox_reaches_the_delegate() {
    let mut world = fixtures::world(fixtures::SEED);
    let chat = fixtures::question(world.front(), QUESTION);
    let (question, delegate) = {
        let task = world.front().engine.task(chat).expect("asked chat");
        let question = task
            .inbox
            .iter()
            .find(|message| {
                message.kind == jig_domain_tasks::MessageKind::Question && message.words.as_ref() == QUESTION
            })
            .expect("delegate's question");
        let jig_domain_tasks::Party::Task(delegate) = question.from else { panic!("question from a task") };
        (question.number, delegate)
    };
    let person = world.client(fixtures::PARTY, fixtures::inbox());
    world.person(
        person,
        vec![
            Step::See { find: Find::named(Role::Heading { level: 2 }, b"Needs you"), within: fixtures::WITHIN },
            Step::See { find: Find::exactly_named(Role::Paragraph, b"Question"), within: fixtures::WITHIN },
            Step::See { find: Find::text(QUESTION), within: fixtures::WITHIN },
            Step::See { find: Find::exactly_named(Role::Link, b"Inbox 1"), within: fixtures::WITHIN },
            Step::Type { find: Find::named(Role::TextArea, b"Answer words"), words: ANSWER.into() },
            Step::Press { find: Find::named(Role::Button, b"Answer") },
            Step::See { find: Find::exactly_named(Role::Status, b"Answered"), within: fixtures::WITHIN },
            Step::See { find: Find::exactly_named(Role::Link, b"Inbox"), within: fixtures::WITHIN },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
    assert!(world.front().engine.task(delegate).expect("answered delegate").inbox.iter().any(|message| {
        message.from == jig_domain_tasks::Party::Task(chat)
            && message.kind == jig_domain_tasks::MessageKind::Answer { question }
            && message.words.as_ref() == ANSWER
    }));

    let notice = format!("Person answered question in inbox: {question}");
    assert!(
        world
            .front()
            .engine
            .task(chat)
            .expect("chat notice")
            .inbox
            .iter()
            .any(|word| word.kind == jig_domain_tasks::MessageKind::QuestionAnswered { question })
    );
    world.go(person, fixtures::chat_page(chat));
    world.person(
        person,
        vec![
            Step::See { find: Find::text(QUESTION), within: fixtures::WITHIN },
            Step::See { find: Find::text(notice.as_bytes()), within: fixtures::WITHIN },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
    let tree = world.tree(person);
    assert!(
        turn_holds(tree, format!("T{delegate}").as_bytes(), QUESTION),
        "delegate T{delegate}'s question turn missing"
    );
    assert!(!turn_holds(tree, b"You", ANSWER), "the answer goes directly to its asker");

    world.go(person, fixtures::inbox());
    world.person(
        person,
        vec![
            Step::See { find: Find::named(Role::Heading { level: 2 }, b"Needs you"), within: fixtures::WITHIN },
            Step::Gone { find: Find::text(QUESTION), within: fixtures::WITHIN },
            Step::Gone { find: Find::exactly_named(Role::Paragraph, b"Question"), within: fixtures::WITHIN },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
}

fn turn_holds(tree: &skein_ui::Tree, label: &[u8], words: &[u8]) -> bool {
    let Some((start, log)) = tree
        .nodes()
        .iter()
        .enumerate()
        .find(|(_, node)| node.role == Role::Log && node.name.as_deref() == Some(b"Conversation"))
    else {
        return false;
    };
    let conversation = &tree.nodes()[start..start + usize::try_from(log.size).expect("log size")];
    conversation.iter().filter(|node| node.role == Role::Region).any(|turn| {
        let mut face = TreeFace::subtree(tree, turn.id).expect("conversation turn");
        matches!(face.find(&Find::exactly_named(Role::Paragraph, label)), Found::Node(_))
            && matches!(face.find(&Find::exactly_named(Role::Paragraph, words)), Found::Node(_))
    })
}

#[test]
fn a_held_chats_escalation_released_from_the_inbox() {
    let mut world = fixtures::world(fixtures::SEED);
    let task = fixtures::held_chat(world.front());
    let address = fixtures::inbox();
    let person = world.client(fixtures::PARTY, address);
    world.person(
        person,
        vec![
            Step::See { find: Find::exactly_named(Role::Button, b"Release task"), within: fixtures::WITHIN },
            Step::Press { find: Find::exactly_named(Role::Button, b"Release task") },
            Step::Press { find: Find::named(Role::Button, b"Confirm") },
            Step::See { find: Find::text(b"Released"), within: fixtures::WITHIN },
        ],
    );
    assert_eq!(world.run(Until::Done).verdict, Verdict::Passed);
    assert_eq!(world.front().engine.task(task).expect("released chat").tries, jig_domain_tasks::Tries::NONE);
}

#[test]
fn releasing_says_what_releasing_does_before_it_is_sent() {
    let mut world = fixtures::world(fixtures::SEED);
    fixtures::held_chat(world.front());
    let person = world.client(fixtures::PARTY, fixtures::inbox());
    world.person(
        person,
        vec![
            Step::See { find: Find::exactly_named(Role::Button, b"Release task"), within: fixtures::WITHIN },
            Step::Press { find: Find::exactly_named(Role::Button, b"Release task") },
            Step::See {
                find: Find::text(b"The next run resumes its conversation; tries count again from zero"),
                within: fixtures::WITHIN,
            },
        ],
    );
    assert_eq!(world.run(Until::Done).verdict, Verdict::Passed);
}
