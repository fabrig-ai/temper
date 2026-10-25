use super::fixtures;
use fixtures::chat::{
    ATTEMPT, BYTES, CLOSE_GRACE, COMMITTED, GRACES, LINES, MESSAGE, PARTY, TASK, TEXT, TURN, chat_header, chat_root,
    closing, messaging_root, projected_tree as tree, proposal_root, provisional, provisional_with_text, rejecting_chat,
};
use jig_client::{contract as c, thread::Thread};
use skein_lib::{List, Wall};
use skein_ui::Role;

#[test]
fn the_demo_chat_context_names_its_project_playground() {
    let root = fixtures::demo_root(temper_client_domain::Address::Chat { task: TASK }, fixtures::PROJECT, true);
    for words in [fixtures::words(), fixtures::browser_words()] {
        let mut view = fixtures::view_of(words);
        let tree = fixtures::build(&mut view, &root);
        let mut context = None;
        for (index, node) in tree.nodes().iter().enumerate() {
            if node.styles.as_slice().contains(&crate::look::CONTEXT) {
                context = Some(index);
                break;
            }
        }
        let context = context.expect("chat context");
        assert!(
            skein_lib::bytes::find(&fixtures::node_text(tree, context), "Playground · acme/shop".as_bytes()).is_some()
        );
    }
}

#[test]
fn the_chatbar_region_is_named_by_its_heading() {
    use skein_fake_person::{Face, Find, Found};

    let root = fixtures::demo_root(temper_client_domain::Address::Chat { task: TASK }, fixtures::PROJECT, true);
    let view = fixtures::page_view(&root);
    let heading =
        named_position(view.tree().nodes(), Role::Heading { level: 1 }, Some("T9 · Ship the update".as_bytes()))
            .expect("heading");
    let children = subtree(view.tree(), heading);
    assert_eq!(children.len(), 3);
    for (child, name) in children.iter().zip([b"T9".as_slice(), " · ".as_bytes(), b"Ship the update"]) {
        assert_eq!(child.role, Role::Text);
        assert_eq!(child.name.as_deref(), Some(name));
    }
    let id = fixtures::person_find(view.tree(), Role::Region, "T9 · Ship the update".as_bytes()).expect("chatbar");
    let mut face = skein_fake_person::TreeFace::subtree(view.tree(), id).expect("chatbar subtree");
    match face.find(&Find::exactly_named(Role::Heading { level: 1 }, "T9 · Ship the update".as_bytes())) {
        Found::Node(_) => {}
        Found::Absent | Found::Pending => panic!("chatbar heading"),
    }
}

#[test]
fn a_chat_proposal_is_named_from_its_source_task() {
    let view = fixtures::page_view(&proposal_root());
    assert!(fixtures::person_find(view.tree(), Role::Item, b"Proposal from T12").is_some());
}

#[test]
fn a_chats_header_says_its_status_and_spend() {
    let tree = chat_header();
    assert!(fixtures::find(&tree, Role::Status, b"Parked").is_some());
}
#[test]
fn a_held_chatbar_names_only_its_exceeded_bound() {
    use temper_client_domain::{Address, Event};
    for (reason, expected) in super::chats::HELD_STATUSES {
        let (mut root, _, watch) = fixtures::root_at(Address::Chat { task: TASK }, &fixtures::domain_limits());
        let mut task = fixtures::task::document();
        task.chip.task = TASK;
        let c::ChipPhase::Owner { phase, .. } = task.chip.details.as_mut() else { panic!("owner") };
        *phase = c::OwnerChipPhase::Held { reason, revision: 1 };
        fixtures::root_event(
            &mut root,
            Event::Client {
                event: jig_client::Event::Up {
                    up: jig_client::Up::Watched {
                        watch: watch.expect("task watch"),
                        event: c::WatchEvent::Delivered(c::Delivery::Task(Box::new(task))),
                    },
                },
            },
        );
        let view = fixtures::page_view(&root);
        let mut status = None;
        for node in view.tree().nodes() {
            if node.role == Role::Status && node.name.as_deref() == Some(expected) {
                status = Some(node);
                break;
            }
        }
        let status = status.expect("held status");
        assert_eq!(status.styles.as_slice(), [crate::look::STATUS_HELD]);
        assert!(skein_lib::bytes::find(&fixtures::text(view.tree()), expected).is_some());
    }
}

#[test]
fn a_chats_header_says_its_spend_including_delegates() {
    let tree = chat_header();
    assert!(fixtures::find(&tree, Role::Paragraph, "7 spent · of 7".as_bytes()).is_some());
}

#[test]
fn provisional_text_is_marked_until_its_turn_commits() {
    let mut thread = provisional();
    assert!(fixtures::find(&tree(&thread), Role::Status, b"Writing...").is_some());
    thread.reading = Some(jig_client::thread::Reading::Latest { known: COMMITTED.into() });
    let mut rows = List::with_capacity(TURN);
    rows.push(c::ActivityLine {
        position: TURN.into(),
        at: Wall::EPOCH,
        event: c::Activity::Turn {
            name: c::TurnName { attempt: ATTEMPT, number: TURN },
            turn: c::TurnRead::Undecodable { why: c::TurnProblem::Malformed },
        },
    })
    .expect("turn");
    thread.page(c::ActivityPage { rows, before: None, after: None, cut: false });
    assert_eq!(fixtures::find(&tree(&thread), Role::Status, b"Writing..."), None);
}

#[test]
fn an_owner_activity_message_is_shown_in_the_chat() {
    let thread = owner_words_thread();
    assert!(skein_lib::bytes::find(&fixtures::text(&tree(&thread)), b"An owner message").is_some());
}

// The chat gallery's T10: a chat started with no spec, its request a words line.
#[test]
fn a_chat_whose_spec_is_empty_draws_no_opening_turn() {
    let thread = owner_words_thread();
    let spec = |words: &[u8]| {
        c::SpecificationDocument::complete(c::Specification {
            words: c::Text::new(Box::from(words), TEXT).expect("spec words"),
            parameters: List::with_capacity(0),
            inputs: List::with_capacity(0),
        })
    };
    let person_turns = |tree: &skein_ui::Tree| {
        let mut count = 0_u32;
        for node in tree.nodes() {
            if node.styles.as_slice().contains(&crate::look::BY_PERSON) {
                count = count.checked_add(1).expect("person turn count");
            }
        }
        count
    };
    assert_eq!(person_turns(&fixtures::chat::requested_tree(&thread, &spec(b"A request"))), 2, "the request opens");
    let tree = fixtures::chat::requested_tree(&thread, &spec(b""));
    assert_eq!(person_turns(&tree), 1, "the words line alone");
    assert!(skein_lib::bytes::find(&fixtures::text(&tree), b"An owner message").is_some());
}

fn owner_words_thread() -> Thread {
    let mut thread = Thread::new(TASK, &jig_client::thread::Limits { lines: LINES, bytes: BYTES, text: TEXT });
    thread.reading = Some(jig_client::thread::Reading::Latest { known: COMMITTED.into() });
    let mut rows = List::with_capacity(TURN);
    rows.push(c::ActivityLine {
        position: TURN.into(),
        at: Wall::EPOCH,
        event: c::Activity::Owner {
            source: Box::new(c::OwnerActivityLine {
                task: TASK,
                position: TURN.into(),
                at: Wall::EPOCH,
                line: c::OwnerActivity::Words {
                    message: MESSAGE,
                    by: c::OwnerRequester::Person(PARTY),
                    kind: c::OwnerMessageKind::Words,
                    words: c::Text::new(Box::from(*b"An owner message"), TEXT).expect("words"),
                },
            }),
            decoded: None,
        },
    })
    .expect("line");
    thread.page(c::ActivityPage { rows, before: None, after: None, cut: false });
    thread
}

#[test]
fn the_close_confirmation_names_each_live_delegates_fate() {
    let tree = closing(Some(CLOSE_GRACE));
    let text = fixtures::text(&tree);
    assert!(skein_lib::bytes::find(&text, b"Delegate 12 will be cancelled").is_some());
    assert!(skein_lib::bytes::find(&text, b"Delegate 13 will be moved to you").is_some());
}
#[test]
fn the_close_confirmation_offers_each_live_delegates_choice() {
    let tree = closing(Some(CLOSE_GRACE));
    assert!(fixtures::find(&tree, Role::Button, b"Cancel delegate 12").is_some());
    assert!(fixtures::find(&tree, Role::Button, b"Take delegate 13").is_some());
}
#[test]
fn the_close_confirmation_says_how_the_chat_ends() {
    use c::RunStage as S;
    use skein_lib::Duration;
    let grace = Some(Duration::from_secs(30));
    for (name, stage, delegate, grace, expected) in [
        ("at rest", S::Idle, false, grace, "Close chat now"),
        ("at rest with delegate", S::Idle, true, grace, "Close chat now"),
        ("preparing", S::Preparing, false, grace, "Close chat now"),
        (
            "claimed",
            S::Claimed { attempt: ATTEMPT },
            false,
            grace,
            "Close chat: allow 30 seconds to finish, then cancel",
        ),
        (
            "running",
            S::Running { attempt: ATTEMPT },
            false,
            grace,
            "Close chat: allow 30 seconds to finish, then cancel",
        ),
        ("running without grace", S::Running { attempt: ATTEMPT }, false, None, "Close chat now"),
    ] {
        let source = chat_root();
        let mut me = source.pages().me(source.client()).unwrap().clone();
        me.projects.get_mut(0).unwrap().templates.get_mut(0).unwrap().grace = grace;
        let mut task = source.pages().chat().unwrap().task(source.client()).unwrap().clone();
        let c::ChipPhase::Owner { phase, .. } = task.chip.details.as_mut() else { panic!("owner") };
        *phase = c::OwnerChipPhase::Active(stage);
        task.delegates = List::with_capacity(1);
        if delegate {
            let mut chip = task.chip.clone();
            chip.task = 7;
            task.delegates.push(c::LiveDelegate { chip, takeover_budget: 0 }).unwrap();
        }
        let (mut root, me_watch, watch) =
            fixtures::root_at(temper_client_domain::Address::Chat { task: TASK }, &fixtures::domain_limits());
        for (watch, delivery) in
            [(me_watch, c::Delivery::Me(Box::new(me))), (watch.unwrap(), c::Delivery::Task(Box::new(task)))]
        {
            deliver_chat(&mut root, watch, delivery);
        }
        fixtures::root_event(
            &mut root,
            temper_client_domain::Event::Act {
                action: temper_client_domain::Action::Pages(temper_client_domain::pages::Act::Close),
            },
        );
        let view = fixtures::page_view(&root);
        let text = fixtures::text(view.tree());
        if expected == "Close chat now" {
            assert!(skein_lib::bytes::find(&text, b"Close chat: allow").is_none(), "{name}");
        }
        let mut expected_text = List::with_capacity(3);
        if delegate {
            expected_text.push(b"Delegate 7 will be cancelled".as_slice()).unwrap();
        }
        expected_text.push(expected.as_bytes()).unwrap();
        expected_text.push(b"A closed chat can't be reopened.".as_slice()).unwrap();
        let mut remaining = text.as_ref();
        for line in &expected_text {
            let Some(at) = skein_lib::bytes::find(remaining, line) else { panic!("{name}: missing {line:?}") };
            remaining = &remaining[at.checked_add(line.len()).unwrap()..];
        }
    }
    for (grace, expected) in GRACES {
        assert!(skein_lib::bytes::find(&fixtures::text(&closing(grace)), expected).is_some(), "{grace:?}");
    }
}
fn deliver_chat(root: &mut temper_client_domain::Domain, watch: skein_lib::Token, delivery: c::Delivery) {
    fixtures::root_event(
        root,
        temper_client_domain::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched { watch, event: c::WatchEvent::Delivered(delivery) },
            },
        },
    );
}
fn messaging_task(task: c::Task) -> temper_client_domain::Domain {
    let (mut root, _, watch) =
        fixtures::root_at(temper_client_domain::Address::Chat { task: TASK }, &fixtures::domain_limits());
    deliver_chat(&mut root, watch.unwrap(), c::Delivery::Task(Box::new(task)));
    root
}
#[test]
fn an_open_chat_at_rest_says_waiting_on_your_words_and_keeps_its_composer() {
    let source = messaging_root();
    let mut task = source.pages().chat().unwrap().task(source.client()).unwrap().clone();
    task.flags.ended_by = c::EndedBy::Requester;
    task.waits_on = c::WaitsOn::News;
    task.wake.words = c::WakeRule::Immediate;
    task.requester = c::OwnerRequester::Person(PARTY);
    let c::ChipPhase::Owner { phase, details } = task.chip.details.as_mut() else { panic!("owner") };
    *phase = c::OwnerChipPhase::Active(c::RunStage::Idle);
    details.live_delegates = 0;
    let view = fixtures::page_view(&messaging_task(task));
    assert!(fixtures::find(view.tree(), Role::Status, b"waiting on your words").is_some());
    assert!(fixtures::find(view.tree(), Role::TextArea, b"Message").is_some());
    assert!(fixtures::find(view.tree(), Role::Button, b"Send").is_some());
}

#[test]
fn pressing_close_opens_the_chat_confirmation() {
    let mut root = chat_root();
    fixtures::root_event(
        &mut root,
        temper_client_domain::Event::Act {
            action: temper_client_domain::Action::Pages(temper_client_domain::pages::Act::Close),
        },
    );
    assert!(root.pages().confirmation().is_some(), "Close prepared the compound request");
    let mut view = fixtures::view();
    assert!(fixtures::find(fixtures::build(&mut view, &root), Role::Dialog, b"Close").is_some());
}

#[test]
fn the_close_confirmation_says_the_cancellations_facts() {
    let mut root = chat_root();
    fixtures::root_event(
        &mut root,
        temper_client_domain::Event::Act {
            action: temper_client_domain::Action::Pages(temper_client_domain::pages::Act::Close),
        },
    );
    let mut view = fixtures::view();
    let said = fixtures::text(fixtures::build(&mut view, &root));
    let dangerous =
        fixtures::words().say_word(jig_client::words::Word::Decision(jig_client::words::DecisionWord::Dangerous), &[]);
    assert!(skein_lib::bytes::find(&said, &dangerous).is_some(), "irreversible cancellation");
    let impact = fixtures::words().say_word(
        jig_client::words::Word::Decision(jig_client::words::DecisionWord::TasksRunning),
        &[skein_ui::phrases::Hole::Number(1)],
    );
    assert!(skein_lib::bytes::find(&said, &impact).is_some(), "supplied running-task impact");
}

#[test]
fn an_unanchored_proposal_is_shown_once_after_its_conversation() {
    let root = proposal_root();
    let mut view = fixtures::view();
    let tree = fixtures::build(&mut view, &root);
    let mut conversation_end = None;
    let mut accept = None;
    for (index, node) in tree.nodes().iter().enumerate() {
        if node.role == Role::Log {
            conversation_end = Some(index + usize::try_from(node.size).expect("log size"));
        }
        if node.role == Role::Button && node.name.as_deref() == Some(b"Accept".as_slice()) {
            accept = Some(index);
        }
    }
    assert_eq!(named_count(tree, Role::Button, b"Accept"), 1);
    assert!(
        accept.expect("proposal's accept act") >= conversation_end.expect("conversation"),
        "card after conversation"
    );
}
#[test]
fn a_chat_proposal_offers_rejection_with_a_reason() {
    let root = proposal_root();
    let mut view = fixtures::view();
    let tree = fixtures::build(&mut view, &root);
    assert!(fixtures::find(tree, Role::Button, b"Reject").is_some());
}
#[test]
fn rejecting_in_the_chat_shows_the_reason_in_its_confirmation() {
    let root = rejecting_chat();
    let mut view = fixtures::view();
    let tree = fixtures::build(&mut view, &root);
    assert!(fixtures::find(tree, Role::Dialog, b"Reject with reason").is_some());
}
#[test]
fn a_chat_rejection_confirmation_keeps_the_persons_reason() {
    let root = rejecting_chat();
    let mut view = fixtures::view();
    let tree = fixtures::build(&mut view, &root);
    assert!(skein_lib::bytes::find(&fixtures::text(tree), b"Too costly").is_some());
}

const LONG_PROVISIONAL: &[u8] = b"1234567890123456789012345678901\xc3\xa9 and more";
#[test]
fn long_provisional_text_says_that_earlier_text_was_omitted() {
    let thread = provisional_with_text(LONG_PROVISIONAL);
    let tree = tree(&thread);
    assert!(fixtures::find(&tree, Role::Status, b"Earlier text omitted").is_some());
    assert!(skein_lib::bytes::find(&fixtures::text(&tree), b"\xc3\xa9 and more").is_some());
}

#[test]
fn the_message_composer_keeps_its_name_and_has_a_numbered_placeholder() {
    let view = fixtures::page_view(&messaging_root());
    let id = fixtures::person_find(view.tree(), Role::TextArea, b"Message").expect("composer named Message");
    let composer = view.tree().find(id).expect("composer node");
    assert_eq!(composer.name.as_deref(), Some(b"Message".as_slice()));
    // messaging_root's chat is TASK, T9.
    assert_eq!(composer.placeholder.as_deref(), Some("Message T9…".as_bytes()));
}

#[test]
fn enter_in_the_message_composer_sends() {
    let mut view = fixtures::page_view(&messaging_root());
    let composer = fixtures::find(view.tree(), Role::TextArea, b"Message").expect("composer");
    assert_eq!(
        crate::act(&mut view, composer, skein_ui::Act::Submit),
        Some(temper_client_domain::Action::Pages(temper_client_domain::pages::Act::Send)),
    );
}
#[test]
fn a_line_break_counts_within_the_message_bound() {
    let mut root = messaging_root();
    let bound = usize::try_from(temper_client_protocol_wire::CUE_BYTES).expect("bound fits");
    let mut over = skein_lib::bytes::zeroed(bound.checked_add(1).unwrap());
    over.fill(b'a');
    over[3] = b'\n';
    let words = &over[..bound];
    assert_eq!(&*fixtures::type_into(&mut root, b"Message", words), words);
    assert_eq!(&*fixtures::type_into(&mut root, b"Message", &over), words);
}
#[test]
fn enter_in_a_cards_reason_does_nothing() {
    let mut view = fixtures::page_view(&proposal_root());
    let reason = fixtures::find(view.tree(), Role::TextArea, b"Reason").expect("reason");
    assert_eq!(crate::act(&mut view, reason, skein_ui::Act::Submit), None);
}
#[test]
fn a_chats_card_carries_the_card_token() {
    let view = fixtures::page_view(&proposal_root());
    let cards = fixtures::styles(view.tree(), Role::Item);
    assert!(!cards.is_empty());
    for card in &cards {
        assert!(card.as_ref().contains(&crate::look::CARD));
        assert!(card.as_ref().contains(&crate::look::KIND_PROPOSAL));
    }
}
#[test]
fn each_turn_carries_the_turn_token() {
    let tree = tree(&provisional());
    let turns = fixtures::styles(&tree, Role::Region);
    assert_eq!(turns.len(), 1);
    assert_eq!(turns.as_ref(), [Box::from([crate::look::TURN])]);
}

fn style_position(nodes: &[skein_ui::Node], token: skein_ui::Style) -> Option<usize> {
    for (index, node) in nodes.iter().enumerate() {
        if node.styles.as_slice().contains(&token) {
            return Some(index);
        }
    }
    None
}
pub(super) fn named_position(nodes: &[skein_ui::Node], role: Role, name: Option<&[u8]>) -> Option<usize> {
    for (index, node) in nodes.iter().enumerate() {
        if node.role == role && (name.is_none() || node.name.as_deref() == name) {
            return Some(index);
        }
    }
    None
}
fn styled(tree: &skein_ui::Tree, token: skein_ui::Style) -> usize {
    style_position(tree.nodes(), token).expect("styled node")
}
fn subtree(tree: &skein_ui::Tree, at: usize) -> &[skein_ui::Node] {
    let from = at.checked_add(1).expect("child index");
    let through = at.checked_add(usize::try_from(tree.nodes()[at].size).expect("size")).expect("subtree end");
    &tree.nodes()[from..through]
}
#[test]
fn the_chat_bar_contains_identity_spend_and_acts() {
    use temper_client_domain::Event;
    let root = messaging_root();
    // The standard task fixture offers Close; a checked task delivery adds Stop.
    let mut task = root.pages().chat().expect("chat").task(root.client()).expect("task").clone();
    let mut offers = List::with_capacity(3);
    for act in [c::ActKind::Message, c::ActKind::Stop, c::ActKind::Cancel] {
        offers.push(c::Offered { act, admission: c::Admission::Allowed, effect: None }).expect("offer");
    }
    task.acts = offers;
    // Re-enter with a task watch whose token is supplied by the fixture.
    let (mut fresh, _, watch) =
        fixtures::root_at(temper_client_domain::Address::Chat { task: TASK }, &fixtures::domain_limits());
    fixtures::root_event(
        &mut fresh,
        Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched {
                    watch: watch.expect("task watch"),
                    event: c::WatchEvent::Delivered(c::Delivery::Task(Box::new(task))),
                },
            },
        },
    );
    let view = fixtures::page_view(&fresh);
    let bar = styled(view.tree(), crate::look::CHATBAR);
    assert_eq!(view.tree().nodes()[bar].role, Role::Region);
    let nodes = subtree(view.tree(), bar);
    let heading = fixtures::words().say(
        crate::words::Word::ChatTitle,
        &[
            skein_ui::phrases::Hole::Number(9),
            skein_ui::phrases::Hole::Name(skein_lib::bytes::copy_of(fixtures::task::document().chip.title.bytes())),
        ],
    );
    for (role, name) in [
        (Role::Heading { level: 1 }, heading.as_ref()),
        (Role::Paragraph, "7 spent · of 7".as_bytes()),
        (Role::Status, b"Parked"),
        (Role::Button, b"Stop"),
        (Role::Button, b"Close"),
    ] {
        assert!(named_position(nodes, role, Some(name)).is_some(), "{name:?}");
    }
    assert!(named_position(nodes, Role::Paragraph, Some(fixtures::task::document().chip.title.bytes())).is_none());
    assert!(fixtures::find(view.tree(), Role::Button, b"Mark seen").is_none());
}
#[test]
fn the_chat_entry_has_the_true_hint_between_message_and_send() {
    let view = fixtures::page_view(&messaging_root());
    let tree = view.tree();
    let area = styled(tree, crate::look::COMPOSER_AREA);
    let entry = styled(tree, crate::look::ENTRY);
    assert_eq!(tree.nodes()[area].role, Role::Slot);
    assert!(entry > area);
    let nodes = subtree(tree, entry);
    assert_eq!(nodes[0].role, Role::TextArea);
    assert!(nodes[0].states.as_slice().contains(&skein_ui::State::SubmitsOnEnter));
    assert_eq!(nodes[1].role, Role::Paragraph);
    assert!(nodes[1].styles.as_slice().contains(&crate::look::HINT));
    let send = 1_usize.checked_add(usize::try_from(nodes[1].size).expect("hint size")).expect("send index");
    assert_eq!(nodes[send].role, Role::Button);
    assert_eq!(nodes[send].name.as_deref(), Some(b"Send".as_slice()));
    assert_eq!(
        fixtures::words().say(crate::words::Word::ChatSendHint, &[]).as_ref(),
        b"Enter to send \xc2\xb7 Shift+Enter for newline"
    );
}
#[test]
fn a_chat_without_message_has_no_composer_area() {
    let view = fixtures::page_view(&chat_root());
    assert!(style_position(view.tree().nodes(), crate::look::COMPOSER_AREA).is_none());
}
#[test]
fn chat_cards_are_items_of_the_cards_list_after_the_log() {
    let view = fixtures::page_view(&proposal_root());
    let tree = view.tree();
    let cards = styled(tree, crate::look::CARDS);
    assert_eq!(tree.nodes()[cards].role, Role::List);
    let card = style_position(subtree(tree, cards), crate::look::CARD).expect("card");
    assert_eq!(subtree(tree, cards)[card].role, Role::Item);
    assert!(named_position(tree.nodes(), Role::Log, None).expect("log") < cards);
}

pub(super) fn structured_thread() -> Thread {
    let mut thread = Thread::new(TASK, &jig_client::thread::Limits { lines: 12, bytes: BYTES, text: 128 });
    thread.reading = Some(jig_client::thread::Reading::Latest { known: 0 });
    let mut messages = List::with_capacity(4);
    for speaker in [c::Speaker::Person, c::Speaker::Assistant, c::Speaker::Tool, c::Speaker::System] {
        let mut blocks = List::with_capacity(2);
        blocks
            .push(c::Block::Call(c::Call {
                number: 1,
                tool: c::Text::new(Box::from(*b"Read"), TEXT).expect("tool"),
                input: c::Text::new(Box::from([]), TEXT).expect("input"),
                outcome: None,
                failed: false,
                objects: List::with_capacity(0),
                parts: List::with_capacity(0),
            }))
            .expect("call");
        blocks
            .push(c::Block::Result {
                call: 1,
                summary: c::Text::new(Box::from(*b"Done"), TEXT).expect("summary"),
                failed: false,
            })
            .expect("result");
        messages.push(c::Message { speaker, blocks }).expect("message");
    }
    let mut rows = List::with_capacity(5);
    for (position, event) in [
        (5, c::Activity::RunEnded { attempt: ATTEMPT, how: c::RunEnding::Parked, spent: 0 }),
        (
            4,
            c::Activity::Turn {
                name: c::TurnName { attempt: ATTEMPT, number: 1 },
                turn: c::TurnRead::Decoded(c::Turn {
                    messages,
                    used: c::Usage { input: 0, output: 0, cached: 0, cache_written: 0 },
                    spent: 0,
                    read_through: None,
                    cut: false,
                }),
            },
        ),
        (
            3,
            c::Activity::Words {
                by: c::Party {
                    number: PARTY,
                    kind: c::PartyKind::Person,
                    name: c::Text::new(Box::from(*b"Pat"), TEXT).expect("person"),
                },
                message: 1,
                words: c::Text::new(Box::from(*b"Hello"), TEXT).expect("words"),
            },
        ),
        (
            2,
            c::Activity::RunBegan {
                attempt: ATTEMPT,
                how: c::RunBeginning::Fresh { tail: 0 },
                why: c::Text::new(Box::from(*b"Hello"), TEXT).expect("why"),
            },
        ),
    ] {
        rows.push(c::ActivityLine { position, at: Wall::EPOCH, event }).expect("line");
    }
    thread.page(c::ActivityPage {
        rows,
        after: None,
        before: Some(c::Cursor::new(Box::from(*b"older"), TEXT).expect("cursor")),
        cut: true,
    });
    thread.left = 1;
    thread
}
#[test]
fn person_words_keep_their_shell_and_only_assistant_calls_keep_work() {
    use crate::look;
    let tree = tree(&structured_thread());
    {
        let at = styled(&tree, look::BY_PERSON);
        assert_eq!(tree.nodes()[at].role, Role::Slot);
        assert!(tree.nodes()[at].styles.as_slice().contains(&look::MESSAGE));
        let nodes = subtree(&tree, at);
        let who = style_position(nodes, look::WHO).expect("who");
        let says = style_position(nodes, look::SAYS).expect("says");
        assert!(who < says);
        assert_eq!(nodes[who].role, Role::Slot);
        assert_eq!(nodes[says].role, Role::Slot);
    }
    let mut works = 0_u32;
    for (at, node) in tree.nodes().iter().enumerate() {
        if node.styles.as_slice().contains(&look::WORK) {
            if node.role == Role::Slot {
                assert!(named_position(subtree(&tree, at), Role::Paragraph, None).is_some());
            } else {
                assert_eq!(node.role, Role::Disclosure);
            }
            works = works.checked_add(1).expect("work count");
        }
    }
    assert_eq!(works, 2);
    for hidden in [look::BY_TOOL, look::BY_SYSTEM, look::BY_AGENT] {
        assert!(style_position(tree.nodes(), hidden).is_none(), "inputs and call-only assistant have no shell");
    }
    assert_call_fold(&tree, b"1 tool call", b"Read", &[b"Read"]);
}
#[test]
fn only_omission_markers_keep_the_turn_region() {
    let tree = tree(&structured_thread());
    let mut markers = 0_u32;
    for node in tree.nodes() {
        if node.styles.as_slice().contains(&crate::look::MARKER) {
            assert_eq!(node.role, Role::Region);
            assert!(node.styles.as_slice().contains(&crate::look::TURN));
            markers = markers.checked_add(1).expect("marker count");
        }
    }
    assert_eq!(markers, 1);
}

fn chat_event(
    root: &mut temper_client_domain::Domain,
    event: temper_client_domain::Event,
    out: &mut skein_lib::Queue<temper_client_domain::Request>,
) {
    temper_client_domain::step(
        root,
        &skein_lib::Env { now: skein_lib::Time::ZERO, wall: Wall::EPOCH, limits: fixtures::domain_limits() },
        event,
        out,
    );
}
fn chat_with_history() -> temper_client_domain::Domain {
    history().0
}
fn history() -> (temper_client_domain::Domain, skein_lib::Queue<temper_client_domain::Request>) {
    use temper_client_domain::{Address, Event, Place, Request};
    let (mut root, _, _) = fixtures::root_at(
        Address::Inbox { filter: temper_client_domain::address::Filter::ALL },
        &fixtures::domain_limits(),
    );
    let mut out = skein_lib::Queue::with_capacity(temper_client_domain::max_out(&fixtures::domain_limits()));
    chat_event(&mut root, Event::Went { place: Place::Address(Address::Chat { task: TASK }) }, &mut out);
    let mut watch = None;
    let mut activity = None;
    while let Some(request) = out.pop() {
        match request {
            Request::Client {
                request:
                    jig_client::Request::Down {
                        down:
                            jig_client::Down::Open {
                                watch: token,
                                subject: jig_client::Watchable::Jig(c::Subject::Task { .. }),
                            },
                    },
            } => watch = Some(token),
            Request::Client {
                request:
                    jig_client::Request::Down {
                        down: jig_client::Down::Read { read, what: jig_client::Readable::Jig(c::Read::Activity { .. }) },
                    },
            } => activity = Some(read),
            Request::Client { .. }
            | Request::Show { .. }
            | Request::Leave { .. }
            | Request::SignOut
            | Request::Dropped { .. }
            | Request::Fact { .. } => {}
        }
    }
    let template = messaging_root();
    let mut task = template.pages().chat().expect("chat").task(template.client()).expect("task").clone();
    task.activity = 4;
    chat_event(
        &mut root,
        Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched {
                    watch: watch.expect("watch"),
                    event: c::WatchEvent::Delivered(c::Delivery::Task(Box::new(task))),
                },
            },
        },
        &mut out,
    );
    let mut rows = List::with_capacity(2);
    rows.push(c::ActivityLine {
        position: 3,
        at: Wall::EPOCH,
        event: c::Activity::Owner {
            source: Box::new(c::OwnerActivityLine {
                task: TASK,
                position: 3,
                at: Wall::EPOCH,
                line: c::OwnerActivity::Turn { reply: 1, attempt: ATTEMPT, turn: TURN, read: None },
            }),
            decoded: None,
        },
    })
    .expect("owner turn");
    rows.push(c::ActivityLine {
        position: 4,
        at: Wall::EPOCH,
        event: c::Activity::Words {
            by: c::Party {
                number: PARTY,
                kind: c::PartyKind::Person,
                name: c::Text::new(Box::from(*b"Pat"), TEXT).expect("name"),
            },
            message: 1,
            words: c::Text::new(Box::from(*b"Hello"), TEXT).expect("words"),
        },
    })
    .expect("line");
    chat_event(
        &mut root,
        Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::ReadDone {
                    read: activity.expect("activity"),
                    result: jig_client::Readout::Jig(c::ReadResult::Value(c::ReadValue::Activity(Box::new(
                        c::ActivityPage {
                            rows,
                            before: Some(c::Cursor::new(Box::from(*b"older"), TEXT).expect("cursor")),
                            after: None,
                            cut: false,
                        },
                    )))),
                },
            },
        },
        &mut out,
    );
    (root, out)
}
#[test]
fn older_is_not_offered_while_the_window_is_filling() {
    let view = fixtures::page_view(&chat_with_history());
    let nodes = subtree(view.tree(), styled(view.tree(), crate::look::THREAD));
    assert_eq!(nodes[0].role, Role::Log);
    assert!(fixtures::find(view.tree(), Role::Button, b"Older").is_none());
}
fn answered_message(answer: c::Answer) -> temper_client_domain::Domain {
    use temper_client_domain::{Action, Event, Request, pages::Act};
    let (mut root, mut out) = history();
    let mut seen = None;
    while let Some(request) = out.pop() {
        if let Request::Client {
            request:
                jig_client::Request::Down {
                    down: jig_client::Down::Send { attempt, request: jig_client::Ask::Jig(ask), .. },
                },
        } = request
            && let c::Ask::Seen { .. } = *ask
        {
            seen = Some(attempt);
        }
    }
    let seen = seen.expect("history already emitted automatic Seen");
    for act in [Act::MessageWords { words: Box::from(*b"Hello") }, Act::Send] {
        chat_event(&mut root, Event::Act { action: Action::Pages(act) }, &mut out);
    }
    assert_eq!(root.pages().chat().expect("chat").pending.len(), 2);
    let pending = fixtures::page_view(&root);
    assert!(fixtures::find(pending.tree(), Role::Status, b"Being sent").is_some());
    let thread = styled(pending.tree(), crate::look::THREAD);
    let area = styled(pending.tree(), crate::look::COMPOSER_AREA);
    let log = named_position(pending.tree().nodes(), Role::Log, None).expect("log");
    let end = log.checked_add(usize::try_from(pending.tree().nodes()[log].size).expect("log size")).expect("log end");
    assert!(named_position(&pending.tree().nodes()[end..area], Role::Status, None).is_none());
    assert!(thread < log && end <= area, "thread={thread} log={log} end={end} area={area}");
    chat_event(&mut root, Event::Client { event: jig_client::Event::Stored }, &mut out);
    let mut attempt = None;
    for request in &out {
        if let Request::Client {
            request: jig_client::Request::Down { down: jig_client::Down::Send { attempt: token, .. } },
        } = request
        {
            attempt = Some(*token);
        }
    }
    let attempt = attempt.expect("attempt");
    chat_event(
        &mut root,
        Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Answer {
                    attempt,
                    envelope: skein_lib::RequestEnvelope::Final,
                    answer: Some(jig_client::Answered::Jig(answer)),
                    why: None,
                },
            },
        },
        &mut out,
    );
    chat_event(
        &mut root,
        Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Answer {
                    attempt: seen,
                    envelope: skein_lib::RequestEnvelope::Final,
                    answer: Some(jig_client::Answered::Jig(c::Answer::Done(c::Outcome::Changed))),
                    why: None,
                },
            },
        },
        &mut out,
    );
    root
}
#[test]
fn a_messages_progress_and_acceptance_stay_in_the_conversation() {
    let root =
        answered_message(c::Answer::Done(c::Outcome::Owner(c::OwnerOutcome::Said { task: TASK, message: MESSAGE })));
    let answered = fixtures::page_view(&root);
    assert!(fixtures::find(answered.tree(), Role::Status, b"Sent").is_some());
    let applied = fixtures::words().say_card(crate::cards::P::Applied, &[]);
    assert!(named_position(answered.tree().nodes(), Role::Status, Some(&applied)).is_none());
}
#[test]
fn a_refused_message_draws_its_refusal_after_send() {
    let root = answered_message(c::Answer::Refused { refusal: c::Refusal::NotOffered, at: None });
    let view = fixtures::page_view(&root);
    let nodes = view.tree().nodes();
    let send = named_position(nodes, Role::Button, Some(b"Send")).unwrap();
    let status = named_position(nodes, Role::Status, Some(b"This act is not offered")).unwrap();
    assert!(send < status);
    let area = styled(view.tree(), crate::look::COMPOSER_AREA);
    assert!(status < area + usize::try_from(nodes[area].size).unwrap());
}

fn header_with_status(waits_on: c::WaitsOn, live: u32, wake: c::WakeRule, requester: c::OwnerRequester) -> crate::View {
    use temper_client_domain::{Address, Event};
    let (mut root, _, watch) = fixtures::root_at(Address::Chat { task: TASK }, &fixtures::domain_limits());
    let mut task = fixtures::task::document();
    task.chip.task = TASK;
    task.waits_on = waits_on;
    task.requester = requester;
    task.wake.words = wake;
    let c::ChipPhase::Owner { details, .. } = task.chip.details.as_mut() else { panic!("owner") };
    details.live_delegates = live;
    fixtures::root_event(
        &mut root,
        Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched {
                    watch: watch.expect("task watch"),
                    event: c::WatchEvent::Delivered(c::Delivery::Task(Box::new(task))),
                },
            },
        },
    );
    fixtures::page_view(&root)
}

#[test]
fn the_chatbar_counts_the_delegates_it_waits_on() {
    // One and two distinguish the shared goal phrase's plural forms.
    for (live, expected) in [(1, b"waiting on 1 delegate".as_slice()), (2, b"waiting on 2 delegates")] {
        let view =
            header_with_status(c::WaitsOn::Delegates, live, c::WakeRule::Never, c::OwnerRequester::Person(PARTY));
        let status = fixtures::find(view.tree(), Role::Status, expected).expect("delegate status");
        assert_eq!(view.tree().find(status).unwrap().styles.as_slice(), [crate::look::STATUS_WAITING]);
    }
}

#[test]
fn the_chatbar_waits_on_the_requesting_persons_words() {
    // Batching also wakes on words, after its configured count or age.
    const BATCH_COUNT: u32 = 2;
    let person = fixtures::root_in_project(None);
    let requester = person.pages().me(person.client()).unwrap().party.number;
    for rule in
        [c::WakeRule::Immediate, c::WakeRule::Batch { count: BATCH_COUNT, age: skein_lib::Duration::from_secs(1) }]
    {
        let view = header_with_status(c::WaitsOn::News, 0, rule, c::OwnerRequester::Person(requester));
        let status = fixtures::find(view.tree(), Role::Status, b"waiting on your words").expect("words status");
        assert_eq!(view.tree().find(status).unwrap().styles.as_slice(), [crate::look::STATUS_WAITING]);
    }
}

#[test]
fn the_chatbar_keeps_jigs_word_without_the_requesters_words_wake() {
    const OTHER_PERSON: u64 = 74;
    let person = fixtures::root_in_project(None);
    let requester = person.pages().me(person.client()).unwrap().party.number;
    for (rule, by) in [
        (c::WakeRule::Never, c::OwnerRequester::Person(requester)),
        (c::WakeRule::Immediate, c::OwnerRequester::Person(OTHER_PERSON)),
        (c::WakeRule::Immediate, c::OwnerRequester::Task(TASK)),
    ] {
        let view = header_with_status(c::WaitsOn::News, 0, rule, by);
        assert!(fixtures::find(view.tree(), Role::Status, b"Parked").is_some());
    }
}

fn assistant_blocks(blocks: &[c::Block]) -> skein_ui::Tree {
    use fixtures::first_run as run;
    let mut thread = Thread::new(TASK, &jig_client::thread::Limits { lines: LINES, bytes: BYTES, text: 512 });
    thread.reading = Some(jig_client::thread::Reading::Latest { known: 0 });
    let mut rows = List::with_capacity(1);
    rows.push(run::turn(1, ATTEMPT, 1, &[run::message(c::Speaker::Assistant, blocks)], 512)).expect("turn");
    thread.page(c::ActivityPage { rows, before: None, after: None, cut: false });
    tree(&thread)
}
#[test]
fn an_agents_pipe_table_keeps_its_rows_cells_and_empty_header() {
    let tree =
        assistant_blocks(&[c::Block::Text(fixtures::first_run::text(b"| | |\n|---|---|\n| Harness | Running |"))]);
    let message = styled(&tree, crate::look::BY_AGENT);
    let says = style_position(subtree(&tree, message), crate::look::SAYS).expect("agent's words");
    let words = subtree(&tree, message + 1 + says);
    let table = named_position(words, Role::Table, None).expect("table in agent's turn");
    let table_at = message + 1 + says + 1 + table;
    let nodes = subtree(&tree, table_at);
    let mut rows = 0_u32;
    let mut headers = 0_u32;
    let mut cells = List::with_capacity(2);
    for (offset, node) in nodes.iter().enumerate() {
        if node.role == Role::Row {
            rows += 1;
        }
        if node.role == Role::ColumnHeader {
            headers += 1;
            assert_eq!(node.size, 1, "empty header has no child");
            if let Some(name) = &node.name {
                assert!(name.is_empty());
            }
            assert!(node.text.is_empty());
        }
        if node.role == Role::Cell {
            cells.push(offset).expect("two body cells");
        }
    }
    assert_eq!(rows, 2);
    assert_eq!(headers, 2);
    assert_eq!(cells.len(), 2);
    for (offset, expected) in cells.iter().zip([b"Harness".as_slice(), b"Running"]) {
        assert_eq!(fixtures::node_text(&tree, table_at + 1 + offset).as_ref(), expected);
    }
}
#[test]
fn repeated_calls_fold_in_first_name_order_with_each_outcome_inside() {
    use fixtures::first_run as run;
    let tree = assistant_blocks(&[
        run::call(1, b"read", b"{}", Some(b"Read the first file")),
        run::call(2, b"wait", b"{}", None),
        run::call(3, b"read", b"{}", Some(b"Read the second file")),
    ]);
    assert_call_fold(
        &tree,
        b"3 tool calls",
        "read ×2 · wait".as_bytes(),
        &[b"readRead the first file", b"wait", b"readRead the second file"],
    );
    assert!(skein_lib::bytes::find(&fixtures::text(&tree), b"Temper").is_none());
}
#[test]
fn a_fold_names_two_repeated_calls_and_two_distinct_calls() {
    use fixtures::first_run as run;
    for (tool, description) in [(b"read".as_slice(), "read ×2"), (b"edit".as_slice(), "read · edit")] {
        let tree = assistant_blocks(&[run::call(1, b"read", b"{}", None), run::call(2, tool, b"{}", None)]);
        assert_call_fold(&tree, b"2 tool calls", description.as_bytes(), &[b"read", tool]);
    }
}
#[test]
fn a_tool_name_using_the_whole_turn_budget_fits_its_fold_description() {
    use fixtures::first_run as run;
    const TURN_BYTES: u32 = 4096;
    let mut receiving = fixtures::domain_limits();
    receiving.client.domain.turn_text = TURN_BYTES;
    let words = fixtures::words();
    let bounds = crate::Limits::of(&receiving, &words).expect("view bounds");
    let mut tool = skein_lib::bytes::zeroed(usize::try_from(TURN_BYTES).unwrap());
    tool.fill(b'x');
    let mut call = run::call(1, b"read", b"", None);
    let c::Block::Call(source) = &mut call else { panic!("call") };
    source.tool = c::Text::new(Box::from(tool.as_ref()), TURN_BYTES).unwrap();
    let mut thread = Thread::new(TASK, &jig_client::thread::Limits { lines: LINES, bytes: BYTES, text: TURN_BYTES });
    thread.reading = Some(jig_client::thread::Reading::Latest { known: 0 });
    let mut rows = List::with_capacity(1);
    rows.push(run::turn(1, ATTEMPT, 1, &[run::message(c::Speaker::Assistant, &[call])], TURN_BYTES)).unwrap();
    thread.page(c::ActivityPage { rows, before: None, after: None, cut: false });
    let mut b = skein_ui::Builder::new(&bounds.tree);
    crate::pages::chat::conversation(
        &mut b,
        &thread,
        &words,
        (receiving.client.domain.rows, TURN_BYTES),
        crate::pages::chat::Speakers { person: None, requester: false },
        crate::pages::chat::Drawn::Chat { request: None, delegations: None },
        &mut crate::frame::Bindings::new(bounds.tree.bindings),
    );
    let tree = b.finish();
    assert_call_fold(&tree, b"1 tool call", &tool, &[&tool]);
}
// The fake person sees the fold and all its content, without an Expanded state.
pub(super) fn assert_call_fold(tree: &skein_ui::Tree, name: &[u8], description: &[u8], lines: &[&[u8]]) {
    assert!(fixtures::person_find(tree, Role::Disclosure, name).is_some());
    let at = named_position(tree.nodes(), Role::Disclosure, Some(name)).expect("disclosure node");
    let fold = &tree.nodes()[at];
    assert_eq!(fold.description.as_deref(), Some(description));
    assert!(!fold.states.as_slice().contains(&skein_ui::State::Expanded));
    let mut child = at.checked_add(1).unwrap();
    for line in lines {
        let node = &tree.nodes()[child];
        assert_eq!(node.role, Role::Slot);
        assert!(node.styles.as_slice().contains(&crate::look::WORK));
        assert_eq!(fixtures::node_text(tree, child).as_ref(), *line);
        child = child.checked_add(usize::try_from(node.size).expect("work line size")).unwrap();
    }
    assert_eq!(child, at.checked_add(usize::try_from(fold.size).expect("fold size")).unwrap());
}
#[test]
fn an_assistants_opaque_block_stays_inside_its_temper_message() {
    let tree = assistant_blocks(&[c::Block::Opaque { dialect: fixtures::first_run::text(b"opaque-v1"), size: 42 }]);
    let message = styled(&tree, crate::look::BY_AGENT);
    let said = fixtures::node_text(&tree, message);
    assert!(skein_lib::bytes::find(&said, b"TemperContent in opaque-v142 bytes").is_some());
}
#[test]
fn a_persons_words_are_paragraphs_without_a_read_label() {
    let mut thread = structured_thread();
    let mut rows = List::with_capacity(1);
    rows.push(c::ActivityLine {
        position: 6,
        at: Wall::EPOCH,
        event: c::Activity::Words {
            by: c::Party { number: PARTY, kind: c::PartyKind::Person, name: fixtures::first_run::text(b"Pat") },
            message: 2,
            words: fixtures::first_run::text(b"First line\nSecond line"),
        },
    })
    .expect("words");
    thread.reading = Some(jig_client::thread::Reading::Latest { known: 5 });
    thread.page(c::ActivityPage { rows, before: None, after: None, cut: false });
    let tree = tree(&thread);
    assert!(fixtures::find(&tree, Role::Paragraph, b"First line").is_some());
    assert!(fixtures::find(&tree, Role::Paragraph, b"Second line").is_some());
    assert!(fixtures::find(&tree, Role::Status, b"Read").is_none());
}

#[test]
fn an_owners_person_words_have_a_person_shell_and_one_paragraph_per_line() {
    let mut thread = Thread::new(TASK, &jig_client::thread::Limits { lines: LINES, bytes: BYTES, text: 512 });
    thread.reading = Some(jig_client::thread::Reading::Latest { known: 0 });
    let mut rows = List::with_capacity(1);
    rows.push(c::ActivityLine {
        position: 1,
        at: Wall::EPOCH,
        event: c::Activity::Owner {
            source: Box::new(c::OwnerActivityLine {
                task: TASK,
                position: 1,
                at: Wall::EPOCH,
                line: c::OwnerActivity::Words {
                    message: MESSAGE,
                    by: c::OwnerRequester::Person(PARTY),
                    kind: c::OwnerMessageKind::Words,
                    words: fixtures::first_run::text(b"First line\nSecond line"),
                },
            }),
            decoded: None,
        },
    })
    .expect("owner words");
    thread.page(c::ActivityPage { rows, before: None, after: None, cut: false });
    let tree = tree(&thread);
    let person = styled(&tree, crate::look::BY_PERSON);
    let said = fixtures::node_text(&tree, person);
    assert!(skein_lib::bytes::find(&said, b"PersonFirst lineSecond line").is_some());
    assert!(fixtures::find(&tree, Role::Paragraph, b"First line").is_some());
    assert!(fixtures::find(&tree, Role::Paragraph, b"Second line").is_some());
    assert!(fixtures::find(&tree, Role::Status, b"Read").is_none());
}

#[test]
fn a_tasks_words_have_an_agent_shell_named_by_the_task() {
    let mut thread = Thread::new(TASK, &jig_client::thread::Limits { lines: LINES, bytes: BYTES, text: TEXT });
    thread.reading = Some(jig_client::thread::Reading::Latest { known: 0 });
    let mut rows = List::with_capacity(1);
    rows.push(c::ActivityLine {
        position: 1,
        at: Wall::EPOCH,
        event: c::Activity::Owner {
            source: Box::new(c::OwnerActivityLine {
                task: TASK,
                position: 1,
                at: Wall::EPOCH,
                line: c::OwnerActivity::Words {
                    message: MESSAGE,
                    by: c::OwnerRequester::Task(2),
                    kind: c::OwnerMessageKind::Question,
                    words: fixtures::first_run::text(b"First line\nSecond line"),
                },
            }),
            decoded: None,
        },
    })
    .expect("owner words");
    thread.page(c::ActivityPage { rows, before: None, after: None, cut: false });
    let mut b = skein_ui::Builder::new(&fixtures::tree_limits());
    crate::pages::chat::conversation(
        &mut b,
        &thread,
        &fixtures::words(),
        (LINES, TEXT),
        crate::pages::chat::Speakers { person: Some(PARTY), requester: true },
        crate::pages::chat::Drawn::Chat { request: None, delegations: None },
        &mut crate::frame::Bindings::new(fixtures::tree_limits().bindings),
    );
    let tree = b.finish();
    let turn = styled(&tree, crate::look::TURN);
    let message = styled(&tree, crate::look::BY_AGENT);
    #[expect(clippy::disallowed_methods, reason = "the view assertion searches the bounded turn's nodes")]
    {
        assert!(subtree(&tree, turn).iter().any(|node| node.styles.as_slice().contains(&crate::look::BY_AGENT)));
    }
    let mut paragraphs = List::with_capacity(3);
    for node in subtree(&tree, message) {
        assert_ne!(node.role, Role::Status);
        if node.role == Role::Paragraph {
            paragraphs.push(node.name.as_deref().expect("named paragraph")).expect("three paragraphs");
        }
    }
    assert_eq!(paragraphs.as_slice(), [b"T2".as_slice(), b"First line", b"Second line"]);
}

#[test]
fn committed_text_exceeds_field_text_on_both_conversation_pages() {
    use fixtures::first_run as run;
    let value = [b'x'; 256];
    let mut thread = Thread::new(TASK, &jig_client::thread::Limits { lines: LINES, bytes: BYTES, text: TEXT });
    thread.reading = Some(jig_client::thread::Reading::Latest { known: 0 });
    let mut rows = List::with_capacity(1);
    rows.push(run::turn(
        1,
        ATTEMPT,
        1,
        &[run::message(c::Speaker::Assistant, &[c::Block::Text(run::text(&value))])],
        2048,
    ))
    .expect("committed turn");
    thread.page(c::ActivityPage { rows, before: None, after: None, cut: false });
    for projected in [true, false] {
        let drawn = if projected { fixtures::chat::projected_tree(&thread) } else { fixtures::chat::tree(&thread) };
        assert!(skein_lib::bytes::find(&fixtures::text(&drawn), &value).is_some(), "chat projection {projected}");
        assert!(fixtures::find(&drawn, Role::Status, b"Earlier text omitted").is_none());
    }
}

fn result_view(result: c::TaskResult) -> crate::View {
    result_view_at(result, None)
}
fn result_view_at(result: c::TaskResult, at: Option<Wall>) -> crate::View {
    use temper_client_domain::{Address, Event};
    let (mut root, _, watch) = fixtures::root_at(Address::Chat { task: TASK }, &fixtures::domain_limits());
    let live = messaging_root();
    let mut task = live.pages().chat().expect("chat").task(live.client()).expect("task").clone();
    // Keep Message offered: the same result which draws the end must suppress the whole foot.
    task.requester = c::OwnerRequester::Person(PARTY);
    task.result = Some(result);
    if let c::ChipPhase::Owner { details, .. } = task.chip.details.as_mut() {
        details.ended_at = at;
    }
    fixtures::root_event(
        &mut root,
        Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched {
                    watch: watch.expect("task watch"),
                    event: c::WatchEvent::Delivered(c::Delivery::Task(Box::new(task))),
                },
            },
        },
    );
    fixtures::page_view(&root)
}

fn result_document(ending: c::Ending) -> c::TaskResult {
    c::TaskResult::Document(Box::new(c::Result {
        task: fixtures::task::document().chip,
        ending,
        value: None,
        at: None,
        details: None,
        acts: List::with_capacity(0),
    }))
}

#[test]
fn every_chat_end_line_carries_ended_and_omits_the_whole_composer() {
    use fixtures::first_run::text;
    let report = c::ResultValue::Report(c::DocumentText::complete(text(b"A report")));
    let cancelled = c::OwnerChipEnding::Cancelled {
        reason: c::DocumentText::complete(text(b"No longer needed")),
        result: None,
        by: c::OwnerRequester::Person(74),
        at: Wall::EPOCH,
    };
    for (result, expected) in [
        (c::TaskResult::Ending(Box::new(c::OwnerChipEnding::Done(report.clone()))), "Closed · its report, “A report”"),
        (
            c::TaskResult::Ending(Box::new(c::OwnerChipEnding::Failed(c::DocumentText::complete(text(
                b"Checks failed",
            ))))),
            "Failed · Checks failed",
        ),
        (c::TaskResult::Ending(Box::new(cancelled.clone())), "Cancelled · No longer needed"),
        (
            result_document(c::Ending::Task(Box::new(c::OwnerChipEnding::Done(report)))),
            "Closed · its report, “A report”",
        ),
        (result_document(c::Ending::Task(Box::new(cancelled))), "Cancelled · No longer needed"),
        (result_document(c::Ending::Failed { reason: Some(text(b"Checks failed")) }), "Failed · Checks failed"),
        (result_document(c::Ending::Failed { reason: None }), "Failed · "),
        (
            result_document(c::Ending::Cancelled {
                reason: Some(text(b"No longer needed")),
                by: c::Actor::Party(74),
                at: None,
            }),
            "Cancelled · No longer needed",
        ),
        (result_document(c::Ending::Withdrawn { reason: text(b"Withdrawn") }), "Cancelled · Withdrawn"),
        (result_document(c::Ending::Done), "Closed"),
        (result_document(c::Ending::Deleted), "Closed"),
        (result_document(c::Ending::Decided { by: PARTY, party: None, how: None, at: Wall::EPOCH }), "Closed"),
    ] {
        let view = result_view(result);
        let end = fixtures::find(view.tree(), Role::Paragraph, expected.as_bytes()).expect("end line");
        assert_eq!(view.tree().find(end).expect("end").styles.as_slice(), [crate::look::ENDED]);
        for token in [crate::look::CHOICE, crate::look::ACCOUNT_LINE, crate::look::COMPOSER_AREA, crate::look::COMPOSER]
        {
            assert!(style_position(view.tree().nodes(), token).is_none(), "ended foot: {token:?}");
        }
        assert!(fixtures::find(view.tree(), Role::TextArea, b"Message").is_none());
    }
}

fn cancelled_by(by: c::OwnerRequester) -> c::OwnerChipEnding {
    c::OwnerChipEnding::Cancelled {
        by,
        reason: c::DocumentText::complete(fixtures::first_run::text(b"No longer needed")),
        result: None,
        at: Wall::EPOCH,
    }
}
fn cancelled_document(by: c::Actor) -> c::TaskResult {
    result_document(c::Ending::Cancelled { by, reason: Some(fixtures::first_run::text(b"No longer needed")), at: None })
}
#[test]
fn a_chats_end_line_says_who_ended_it() {
    use fixtures::first_run::text;
    let report = c::ResultValue::Report(c::DocumentText::complete(text(b"A report")));
    let mut reported = result_document(c::Ending::Done);
    let c::TaskResult::Document(doc) = &mut reported else { panic!("document") };
    doc.value = Some(report.clone());
    for (name, owner, document, clock, expected) in [
        (
            "requester",
            cancelled_by(c::OwnerRequester::Person(PARTY)),
            cancelled_document(c::Actor::Party(PARTY)),
            Some(fixtures::NOW),
            "Closed 00:00",
        ),
        (
            "requester without clock",
            cancelled_by(c::OwnerRequester::Person(PARTY)),
            cancelled_document(c::Actor::Party(PARTY)),
            None,
            "Closed",
        ),
        (
            "another person",
            cancelled_by(c::OwnerRequester::Person(74)),
            cancelled_document(c::Actor::Party(74)),
            Some(fixtures::NOW),
            "Cancelled 00:00 · No longer needed",
        ),
        (
            "a task",
            cancelled_by(c::OwnerRequester::Task(7)),
            cancelled_document(c::Actor::Task(7)),
            Some(fixtures::NOW),
            "Cancelled 00:00 · No longer needed",
        ),
        (
            "report",
            c::OwnerChipEnding::Done(report),
            reported,
            Some(fixtures::NOW),
            "Closed 00:00 · its report, “A report”",
        ),
        (
            "failure",
            c::OwnerChipEnding::Failed(c::DocumentText::complete(text(b"Checks failed"))),
            result_document(c::Ending::Failed { reason: Some(text(b"Checks failed")) }),
            Some(fixtures::NOW),
            "Failed 00:00 · Checks failed",
        ),
    ] {
        for result in [
            c::TaskResult::Ending(Box::new(owner.clone())),
            result_document(c::Ending::Task(Box::new(owner))),
            document,
        ] {
            let view = result_view_at(result, clock);
            assert!(fixtures::find(view.tree(), Role::Paragraph, expected.as_bytes()).is_some(), "{name}");
            for token in
                [crate::look::COMPOSER_AREA, crate::look::CHOICE, crate::look::ACCOUNT_LINE, crate::look::COMPOSER]
            {
                assert!(style_position(view.tree().nodes(), token).is_none(), "{name}");
            }
            assert!(fixtures::find(view.tree(), Role::TextArea, b"Message").is_none());
            assert!(fixtures::find(view.tree(), Role::Button, b"Send").is_none());
        }
    }
}

#[test]
fn a_live_chat_keeps_its_choice_line_and_composer() {
    let view = fixtures::page_view(&messaging_root());
    assert!(style_position(view.tree().nodes(), crate::look::CHOICE).is_some());
    assert!(style_position(view.tree().nodes(), crate::look::ACCOUNT_LINE).is_some());
    assert!(fixtures::find(view.tree(), Role::TextArea, b"Message").is_some());
}

#[test]
fn a_chats_end_and_its_phase_keep_the_supplied_clock() {
    use fixtures::first_run::text;
    for (ending, expected) in [
        (
            c::OwnerChipEnding::Done(c::ResultValue::Report(c::DocumentText::complete(text(b"A report")))),
            "Closed 00:00 · its report, “A report”",
        ),
        (c::OwnerChipEnding::Failed(c::DocumentText::complete(text(b"Checks failed"))), "Failed 00:00 · Checks failed"),
        (
            c::OwnerChipEnding::Cancelled {
                reason: c::DocumentText::complete(text(b"No longer needed")),
                result: None,
                by: c::OwnerRequester::Person(74),
                at: Wall::EPOCH,
            },
            "Cancelled 00:00 · No longer needed",
        ),
    ] {
        let view = result_view_at(c::TaskResult::Ending(Box::new(ending)), Some(fixtures::NOW));
        assert!(skein_lib::bytes::find(&fixtures::text(view.tree()), expected.as_bytes()).is_some());
        assert!(fixtures::find(view.tree(), Role::Status, b"Closed 00:00").is_some());
        assert_eq!(view.next_change(), Some(Wall::from_nanos(86_400_000_000_000)));
    }
}

#[test]
fn an_automatic_seen_draws_no_progress() {
    let root = chat_with_history();
    let chat = root.pages().chat().expect("chat");
    let mut seeing = false;
    for (key, kind) in &chat.pending {
        if *kind == temper_client_domain::pages::chat::Sending::Seen {
            assert!(root.client().pending(*key).is_some(), "request table has Seen progress");
            seeing = true;
        }
    }
    assert!(seeing, "automatic Seen pending");
    let view = fixtures::page_view(&root);
    let words = fixtures::words();
    for phrase in
        [crate::cards::P::InFlight, crate::cards::P::Retrying, crate::cards::P::Parked, crate::cards::P::Unknown]
    {
        let name = words.say_card(phrase, &[]);
        assert!(fixtures::find(view.tree(), Role::Status, &name).is_none(), "automatic request progress: {name:?}");
    }
    assert!(fixtures::find(view.tree(), Role::Button, b"Mark seen").is_none());
}

// Thursday 22 October 2026, noon UTC, and its neighbouring days.
const TURN_NOON: u64 = 1_792_670_400;
const TURN_DAY: u64 = 86_400;
const TURN_MIDNIGHT: u64 = TURN_NOON - TURN_DAY / 2;
fn turn_wall(seconds: u64) -> Wall {
    Wall::from_nanos(seconds.checked_mul(1_000_000_000).expect("fixture wall fits"))
}
fn timed_thread(events: &[(u64, c::Activity)]) -> Thread {
    let mut thread = Thread::new(TASK, &jig_client::thread::Limits { lines: 12, bytes: BYTES, text: 512 });
    thread.reading = Some(jig_client::thread::Reading::Latest { known: 0 });
    let mut rows = List::with_capacity(u32::try_from(events.len()).unwrap());
    for (index, (at, event)) in events.iter().enumerate().rev() {
        rows.push(c::ActivityLine {
            position: u64::try_from(index).unwrap().checked_add(1).expect("position fits"),
            at: turn_wall(*at),
            event: event.clone(),
        })
        .unwrap();
    }
    thread.page(c::ActivityPage { rows, before: None, after: None, cut: false });
    thread
}
fn timed_words() -> c::Activity {
    c::Activity::Words {
        by: c::Party { number: PARTY, kind: c::PartyKind::Person, name: fixtures::first_run::text(b"Pat") },
        message: MESSAGE,
        words: fixtures::first_run::text(b"A message"),
    }
}
fn timed_turn() -> c::Activity {
    fixtures::first_run::turn(
        1,
        ATTEMPT,
        TURN,
        &[fixtures::first_run::message(
            c::Speaker::Assistant,
            &[c::Block::Text(fixtures::first_run::text(b"An answer"))],
        )],
        512,
    )
    .event
}
fn timed_tree(
    thread: &Thread,
    drawn: crate::pages::chat::Drawn<'_>,
    person: Option<u64>,
    now: u64,
) -> (skein_ui::Tree, Option<Wall>) {
    let mut b = skein_ui::Builder::new(&fixtures::tree_limits());
    let mut times = crate::words::Times::new(turn_wall(now));
    crate::pages::chat::conversation_with_times(
        &mut b,
        thread,
        &fixtures::words(),
        (LINES, TEXT),
        crate::pages::chat::Speakers { person, requester: true },
        (drawn, Some(&mut times)),
        &mut crate::frame::Bindings::new(fixtures::tree_limits().bindings),
    );
    (b.finish(), times.next)
}
fn timed_drawing(projected: bool) -> crate::pages::chat::Drawn<'static> {
    if projected {
        crate::pages::chat::Drawn::Chat { request: None, delegations: None }
    } else {
        crate::pages::chat::Drawn::Lines
    }
}
fn timed_style_positions(tree: &skein_ui::Tree, style: skein_ui::Style) -> List<usize> {
    let mut positions = List::with_capacity(fixtures::tree_limits().nodes);
    for (index, node) in tree.nodes().iter().enumerate() {
        if node.styles.as_slice().contains(&style) {
            positions.push(index).expect("positions fit tree nodes");
        }
    }
    positions
}
#[test]
fn turn_clocks_follow_separately_named_speakers_on_both_paths() {
    let thread = timed_thread(&[(TURN_NOON, timed_words()), (TURN_NOON + 60, timed_turn())]);
    for projected in [true, false] {
        for (person, label) in [(Some(PARTY), b"You".as_slice()), (None, b"Pat".as_slice())] {
            let (tree, _) = timed_tree(&thread, timed_drawing(projected), person, TURN_NOON);
            let roles = timed_style_positions(&tree, crate::look::WHO);
            for (index, (name, clock)) in roles.iter().zip([(label, b"12:00"), (b"Temper".as_slice(), b"12:01")]) {
                let children = subtree(&tree, *index);
                assert_eq!(children[0].name.as_deref(), Some(name));
                let clock_at = usize::try_from(children[0].size).unwrap();
                assert_eq!(children[clock_at].role, Role::Time);
                assert_eq!(children[clock_at].name.as_deref(), Some(clock.as_slice()));
                assert_eq!(children[clock_at].size, 2, "the time holds only its text leaf");
                assert_eq!(children[clock_at + 1].role, Role::Text);
            }
            assert_eq!(roles.len(), 2);
        }
    }
}
#[test]
fn pending_words_and_provisional_text_have_no_clock_or_day() {
    let mut pending = Thread::new(TASK, &jig_client::thread::Limits { lines: LINES, bytes: BYTES, text: TEXT });
    assert!(pending.words.begin(
        jig_client::Key { asker: jig_client::Asker::JIG, number: 1 },
        None,
        fixtures::first_run::text(b"Pending")
    ));
    for thread in [&pending, &provisional()] {
        for projected in [true, false] {
            let (tree, next) = timed_tree(thread, timed_drawing(projected), None, TURN_NOON);
            for node in tree.nodes() {
                assert_ne!(node.role, Role::Time);
                assert!(!node.styles.as_slice().contains(&crate::look::DAY));
            }
            assert_eq!(next, None);
        }
    }
}
#[test]
fn the_request_uses_the_first_timed_line_and_shares_its_day() {
    let began = c::Activity::RunBegan {
        attempt: ATTEMPT,
        how: c::RunBeginning::Fresh { tail: 0 },
        why: fixtures::first_run::text(b"Run"),
    };
    let thread = timed_thread(&[(TURN_NOON - 60, began), (TURN_NOON, timed_words())]);
    let request = c::SpecificationDocument::complete(c::Specification {
        words: fixtures::first_run::text(b"The request"),
        parameters: List::with_capacity(0),
        inputs: List::with_capacity(0),
    });
    for (thread, expected) in [(&thread, Some(b"12:00".as_slice())), (&provisional(), None)] {
        let (tree, _) = timed_tree(
            thread,
            crate::pages::chat::Drawn::Chat { request: Some(&request), delegations: None },
            None,
            TURN_NOON,
        );
        let who = styled(&tree, crate::look::WHO);
        let children = subtree(&tree, who);
        assert_eq!(children[0].name.as_deref(), Some(b"You".as_slice()));
        let mut clock = None;
        for child in children {
            if child.role == Role::Time {
                clock = child.name.as_deref();
            }
        }
        assert_eq!(clock, expected);
        assert_eq!(timed_style_positions(&tree, crate::look::DAY).len(), u32::from(expected.is_some()));
    }
}
#[test]
fn each_days_first_turn_has_one_day_line_with_the_calendar_words() {
    // Wednesday 31 December 2025: a year must be said before this year's October.
    let last_year = 1_767_139_200;
    let thread = timed_thread(&[
        (last_year, timed_words()),
        (TURN_NOON - 2 * TURN_DAY, timed_words()),
        (TURN_NOON - TURN_DAY, timed_words()),
        (TURN_NOON, timed_words()),
        (TURN_NOON + 60, timed_turn()),
    ]);
    for projected in [true, false] {
        let (tree, next) = timed_tree(&thread, timed_drawing(projected), None, TURN_NOON);
        let days = timed_style_positions(&tree, crate::look::DAY);
        for (index, said) in days.iter().zip([
            "Wednesday 31 December 2025",
            "Tuesday 20 October",
            "Yesterday · Wednesday 21 October",
            "Today · Thursday 22 October",
        ]) {
            let node = &tree.nodes()[*index];
            assert_eq!(node.name.as_deref(), Some(said.as_bytes()));
            assert!(
                tree.nodes()[index + usize::try_from(node.size).unwrap()]
                    .styles
                    .as_slice()
                    .contains(&crate::look::TURN),
                "day precedes its first turn"
            );
        }
        assert_eq!(days.len(), 4);
        assert_eq!(next, Some(turn_wall(TURN_MIDNIGHT + TURN_DAY)));
    }
}
#[test]
fn an_undecodable_turn_keeps_its_clock_and_day_on_both_paths() {
    let thread = timed_thread(&[(
        TURN_NOON,
        c::Activity::Turn {
            name: c::TurnName { attempt: ATTEMPT, number: TURN },
            turn: c::TurnRead::Undecodable { why: c::TurnProblem::Malformed },
        },
    )]);
    for projected in [true, false] {
        let (tree, _) = timed_tree(&thread, timed_drawing(projected), None, TURN_NOON);
        assert!(fixtures::find(&tree, Role::Time, b"12:00").is_some());
        assert!(fixtures::find(&tree, Role::Paragraph, "Today · Thursday 22 October".as_bytes()).is_some());
    }
}
#[test]
fn a_chat_rebuilds_its_day_at_midnight_and_keeps_the_turn_clock() {
    let root = chat_with_history();
    let shown = temper_client_domain::shown(&root);
    let mut view = fixtures::view();
    let mut patches = skein_lib::Queue::with_capacity(view.limits().tree.patches);
    for (now, day, next) in [
        (TURN_DAY - 1, "Today · Thursday 1 January", TURN_DAY),
        (TURN_DAY, "Yesterday · Thursday 1 January", 2 * TURN_DAY),
    ] {
        crate::build(&mut view, &root, turn_wall(now), &mut patches);
        assert!(fixtures::find(view.tree(), Role::Paragraph, day.as_bytes()).is_some());
        assert!(fixtures::find(view.tree(), Role::Time, b"00:00").is_some());
        assert_eq!(view.next_change(), Some(turn_wall(next)));
        assert_eq!(temper_client_domain::shown(&root), shown);
        while patches.pop().is_some() {}
    }
}

#[test]
fn the_chatbar_and_chats_line_share_every_status_phrase() {
    const STATES: u32 = 18;
    let mut rows = List::with_capacity(STATES);
    for (line, _) in super::chats::status_rows() {
        rows.push(line).unwrap();
    }
    for (waits_on, live, wake) in [
        (c::WaitsOn::Delegates, 1, c::WakeRule::Never),
        (c::WaitsOn::Delegates, 2, c::WakeRule::Never),
        (c::WaitsOn::News, 0, c::WakeRule::Immediate),
        (c::WaitsOn::News, 0, c::WakeRule::Never),
        (c::WaitsOn::News, 2, c::WakeRule::Never),
        (c::WaitsOn::Delegates, 0, c::WakeRule::Immediate),
    ] {
        let mut line = super::chats::line();
        line.waits_on = waits_on;
        let c::ChipPhase::Owner { details, .. } = line.chip.details.as_mut() else { panic!("owner") };
        details.live_delegates = live;
        details.wake.words = wake;
        rows.push(line).unwrap();
    }
    let mut closed = super::chats::line();
    let c::ChipPhase::Owner { phase, details } = closed.chip.details.as_mut() else { panic!("owner") };
    *phase = c::OwnerChipPhase::Ended;
    details.ended_at = Some(Wall::EPOCH);
    details.ending = Some(c::OwnerChipEnding::Cancelled {
        by: closed.requester,
        reason: c::DocumentText::complete(fixtures::first_run::text(b"Any reason")),
        result: None,
        at: Wall::EPOCH,
    });
    rows.push(closed).unwrap();
    let expected = [
        b"waiting on an answer from you".as_slice(),
        b"Held",
        b"Retrying",
        b"Starting",
        b"Writing",
        b"Using tool search",
        b"Working",
        b"Waiting for another party",
        b"Parked",
        b"Closing",
        b"Closed",
        b"waiting on 1 delegate",
        b"waiting on 2 delegates",
        b"waiting on your words",
        b"Parked",
        b"waiting on 2 delegates",
        b"waiting on your words",
        b"Closed",
    ];
    for (line, expected) in rows.iter().zip(expected) {
        let (line_tree, _) = super::chats::tree(line);
        assert!(fixtures::find(&line_tree, Role::Status, expected).is_some(), "chats line: {expected:?}");
        let c::ChipPhase::Owner { details, .. } = line.chip.details.as_ref() else { panic!("owner") };
        let root = fixtures::root_in_project(None);
        let me = root.pages().me(root.client()).unwrap();
        let mut b = skein_ui::Builder::new(&fixtures::tree_limits());
        crate::pages::chats::status(
            &mut b,
            &line.chip,
            (&line.waits_on, Some(details.live_delegates), None),
            (line.project, line.requester),
            Some(&details.wake),
            me,
            &fixtures::words(),
        );
        assert!(fixtures::find(&b.finish(), Role::Status, expected).is_some(), "chatbar: {expected:?}");
    }
}

#[test]
fn the_chatbar_draws_the_supplied_budget_including_zero() {
    use temper_client_domain::{Address, Event};
    for (budget, phrase) in [(0, "7 spent · of 0".as_bytes()), (100, "7 spent · of 100".as_bytes())] {
        let (mut root, _, watch) = fixtures::root_at(Address::Chat { task: TASK }, &fixtures::domain_limits());
        let mut task = fixtures::task::document();
        task.chip.task = TASK;
        task.budget.numbers.budget = budget;
        fixtures::root_event(
            &mut root,
            Event::Client {
                event: jig_client::Event::Up {
                    up: jig_client::Up::Watched {
                        watch: watch.unwrap(),
                        event: c::WatchEvent::Delivered(c::Delivery::Task(Box::new(task))),
                    },
                },
            },
        );
        let view = fixtures::page_view(&root);
        assert!(fixtures::find(view.tree(), Role::Paragraph, phrase).is_some());
    }
}

#[test]
fn a_proposals_fold_precedes_its_card_after_the_message() {
    use fixtures::first_run as run;
    let object = c::ObjectKey::Proposal(fixtures::chat::PROPOSAL);
    let mut proposal = run::call(3, b"propose", b"{}", None);
    let c::Block::Call(call) = &mut proposal else { panic!("call") };
    call.objects = List::with_capacity(1);
    call.objects.push(object).unwrap();
    let mut message = run::message(
        c::Speaker::Assistant,
        &[
            c::Block::Text(run::text(b"A proposal follows")),
            run::call(1, b"read", b"{}", None),
            run::call(2, b"read", b"{}", None),
        ],
    );
    let mut blocks = List::with_capacity(4);
    for block in &message.blocks {
        blocks.push(block.clone()).unwrap();
    }
    blocks.push(proposal.clone()).unwrap();
    message.blocks = blocks;
    let mut rows = List::with_capacity(2);
    let mut first = run::turn(1, ATTEMPT, 1, &[], 512);
    let c::Activity::Turn { turn: c::TurnRead::Decoded(turn), .. } = &mut first.event else { panic!("decoded turn") };
    turn.messages = List::with_capacity(1);
    turn.messages.push(message).unwrap();
    rows.push(first).unwrap();
    rows.push(run::turn(2, ATTEMPT, 2, &[run::message(c::Speaker::Assistant, &[proposal])], 512)).unwrap();
    let root = fixtures::chat::proposal_root_with_activity(Some(c::ActivityPage {
        rows,
        before: None,
        after: None,
        cut: false,
    }));
    let mut builder = skein_ui::Builder::new(&fixtures::tree_limits());
    crate::pages::chat::main(
        &mut builder,
        &root,
        &fixtures::words(),
        &mut crate::frame::Bindings::new(fixtures::tree_limits().bindings),
        4,
        TEXT,
        &mut crate::words::Times::new(fixtures::NOW),
    );
    let tree = builder.finish();
    let mut turn = None;
    for (at, node) in tree.nodes().iter().enumerate() {
        if node.styles.as_slice().contains(&crate::look::TURN)
            && skein_lib::bytes::find(&fixtures::node_text(&tree, at), b"A proposal follows").is_some()
        {
            turn = Some(at);
            break;
        }
    }
    assert!(
        turn.is_some(),
        "assistant turn: {:?}; rows {:?}",
        fixtures::text(&tree),
        root.pages().chat().unwrap().thread.lines.rows
    );
    let turn = turn.unwrap();
    let children = subtree(&tree, turn);
    let card = style_position(children, crate::look::CARD).expect("proposal inside first turn");
    let message = style_position(children, crate::look::MESSAGE).expect("message");
    assert!(card >= message + usize::try_from(children[message].size).unwrap());
    let fold = style_position(children, crate::look::WORK).expect("fold in the same turn");
    assert!(fold >= message + usize::try_from(children[message].size).unwrap());
    assert!(fold + usize::try_from(children[fold].size).unwrap() <= card, "fold before proposal card");
    assert_eq!(named_count(&tree, Role::Button, b"Accept"), 1);
    assert_call_fold(&tree, b"2 tool calls", "read ×2".as_bytes(), &[b"read", b"read"]);
    let said = fixtures::text(&tree);
    assert!(skein_lib::bytes::find(&said, b"propose").is_none());
}

#[test]
fn a_chat_proposal_links_to_the_inbox_filtered_to_proposals() {
    let mut view = fixtures::page_view(&proposal_root());
    let link = fixtures::find(view.tree(), Role::Link, "Open in inbox ↗".as_bytes()).expect("inbox link");
    assert_eq!(
        crate::act(&mut view, link, skein_ui::Act::Press),
        Some(temper_client_domain::Action::Pages(temper_client_domain::pages::Act::Go {
            address: temper_client_domain::Address::Inbox {
                filter: temper_client_domain::address::Filter {
                    kind: Some(temper_client_domain::address::Kind::Proposal),
                    ..temper_client_domain::address::Filter::ALL
                }
            }
        }))
    );
}

fn named_count(tree: &skein_ui::Tree, role: Role, name: &[u8]) -> u32 {
    let mut count = 0_u32;
    for node in tree.nodes() {
        if node.role == role && node.name.as_deref() == Some(name) {
            count = count.checked_add(1).unwrap();
        }
    }
    count
}

#[test]
fn the_demo_chats_bar_has_no_model_word() {
    let root = fixtures::demo_root(temper_client_domain::Address::Chat { task: TASK }, fixtures::PROJECT, true);
    let view = fixtures::page_view(&root);
    let bar = styled(view.tree(), crate::look::CHATBAR);
    for node in subtree(view.tree(), bar) {
        assert_ne!(node.name.as_deref(), Some(b"Scripted model".as_slice()));
    }
}

#[test]
fn unavailable_models_block_send_and_enter_in_the_view() {
    let root = super::choice::no_model_root(true);
    let mut view = fixtures::page_view(&root);
    for (role, name, event) in [
        (Role::Button, b"Send".as_slice(), skein_ui::Act::Press),
        (Role::TextArea, b"Message".as_slice(), skein_ui::Act::Submit),
    ] {
        let node = fixtures::find(view.tree(), role, name).expect("composer control");
        assert_eq!(crate::act(&mut view, node, event), None);
    }
}
