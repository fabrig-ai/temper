#![allow(clippy::disallowed_methods, reason = "view assertions inspect semantic trees and rendered UTF-8")]
use super::fixtures as f;
use jig_client::contract as c;
use skein_ui::Role;
use temper_client_domain::Address;
use temper_client_domain::ChatsFilter;

#[test]
fn the_start_draws_the_frame_banner_before_its_chatbar_context_and_invitation() {
    let root = f::demo_root(Address::Chats { filter: ChatsFilter::All, start: false }, f::PROJECT, true);
    let view = f::page_view(&root);
    let nodes = view.tree().nodes();
    let mut main = None;
    let mut chatbar = None;
    let mut context = None;
    for (index, node) in nodes.iter().enumerate() {
        if node.role == Role::Main {
            main = Some(index);
        }
        if node.styles.as_slice().contains(&crate::look::CHATBAR) {
            chatbar = Some(index);
        }
        if node.styles.as_slice().contains(&crate::look::CONTEXT) {
            context = Some(index);
        }
    }
    assert!(main.unwrap() < chatbar.unwrap());
    assert_eq!(nodes[context.unwrap()].role, Role::Slot);
    let context = context.unwrap();
    let end = context + usize::try_from(nodes[context].size).unwrap();
    let mut paragraphs = 0_u32;
    for node in &nodes[context..end] {
        if node.role == Role::Paragraph {
            paragraphs += 1;
        }
    }
    assert_eq!(paragraphs, 2, "project and sign-in");
    let mut after = 0;
    for (role, name) in [
        (Role::Banner, b"".as_slice()),
        (Role::Region, b"Playground"),
        (Role::Main, b""),
        (Role::Paragraph, b"New chat"),
        (Role::Status, b"Ready"),
        (Role::Paragraph, b"Scripted model"),
        (Role::Paragraph, "Playground · acme/shop on Playground forge".as_bytes()),
        (Role::Paragraph, b"Signed in as Pat"),
        (Role::Heading { level: 1 }, b"What shall we change?"),
        (Role::Paragraph, b"Try a request in acme/shop. Follow the task from its branch to a landed change."),
        (Role::List, b"Suggested requests"),
        (Role::Paragraph, b"The scripted model plays these requests from its script."),
        (Role::Paragraph, "Using Scripted model · Default".as_bytes()),
        (Role::TextArea, b"Start a chat"),
        (Role::Heading { level: 2 }, b"Your chats"),
        (Role::Paragraph, b"No chats yet."),
    ] {
        let mut found = None;
        for (index, node) in nodes.iter().enumerate() {
            let matches = if role == Role::Banner || role == Role::Main {
                true
            } else if role == Role::Paragraph {
                f::node_text(view.tree(), index).as_ref() == name
            } else {
                node.name.as_deref() == Some(name)
            };
            if node.role == role && matches {
                assert!(found.is_none(), "only one {name:?}");
                found = Some(index);
            }
        }
        let Some(at) = found else { panic!("missing {name:?}") };
        assert!(at > after, "{name:?} follows the previous part");
        after = at;
    }
    assert!(f::find(view.tree(), Role::Heading { level: 1 }, b"Chats").is_none());
    let ready = f::find(view.tree(), Role::Status, b"Ready").unwrap();
    for node in nodes {
        if node.id == ready {
            assert_eq!(node.styles.as_slice(), [crate::look::STATUS_QUIET]);
        }
    }
}

#[test]
fn other_chats_keep_their_heading_and_no_start() {
    for (row, project, model, demo) in [
        ("other project", f::SECOND_PROJECT, true, true),
        ("forge alone", f::PROJECT, false, true),
        ("no answer", f::PROJECT, true, false),
    ] {
        let root = if demo {
            f::demo_root(Address::Chats { filter: ChatsFilter::All, start: false }, project, model)
        } else {
            f::chats_root()
        };
        let view = f::page_view(&root);
        assert!(f::find(view.tree(), Role::Heading { level: 1 }, b"Chats").is_some(), "{row}");
        assert!(f::find(view.tree(), Role::Paragraph, b"New chat").is_none(), "{row}");
        assert!(f::find(view.tree(), Role::Heading { level: 1 }, b"What shall we change?").is_none(), "{row}");
        for node in view.tree().nodes() {
            assert!(!node.styles.as_slice().contains(&crate::look::CONTEXT), "{row}");
        }
    }
}

#[test]
fn the_context_names_the_deployment_owner_then_the_project_owner_then_the_person() {
    let template = f::demo_root(Address::Chats { filter: ChatsFilter::All, start: false }, f::PROJECT, true);
    for (row, deployment, role, named, expected, project) in [
        (
            "deployment owner",
            Some(c::HeldRole::Owner),
            c::HeldRole::Owner,
            true,
            "Signed in as the deployment’s owner".as_bytes(),
            "Playground · acme/shop on Playground forge",
        ),
        (
            "project owner",
            None,
            c::HeldRole::Owner,
            true,
            "Signed in as the project’s owner".as_bytes(),
            "Playground · acme/shop on Playground forge",
        ),
        (
            "another role",
            Some(c::HeldRole::Policy { role: 1 }),
            c::HeldRole::Policy { role: 1 },
            true,
            b"Signed in as Pat",
            "Playground · acme/shop on Playground forge",
        ),
        (
            "project fallback",
            None,
            c::HeldRole::Policy { role: 1 },
            false,
            b"Signed in as Pat",
            "Playground · acme/shop on Playground forge",
        ),
    ] {
        let mut me = template.pages().me(template.client()).unwrap().clone();
        me.deployment_role = deployment;
        me.projects.get_mut(0).unwrap().role = role;
        if !named {
            me.projects.get_mut(0).unwrap().name = jig_client::contract::DocumentText::complete(
                jig_client::contract::Text::new(b"Project 1".as_slice().into(), 64).expect("project name"),
            );
        }
        let root = f::demo_frame_root(
            Address::Chats { filter: ChatsFilter::All, start: false },
            f::PROJECT,
            true,
            None,
            Some(me),
        );
        let view = f::page_view(&root);
        assert!(f::find(view.tree(), Role::Paragraph, expected).is_some(), "{row}");
        let mut found = false;
        for (index, node) in view.tree().nodes().iter().enumerate() {
            if node.role == Role::Paragraph && f::node_text(view.tree(), index).as_ref() == project.as_bytes() {
                found = true;
            }
        }
        assert!(found, "{row}");
    }
}

#[test]
fn a_chats_context_follows_the_chatbar_only_in_the_demo_project_even_without_its_model() {
    for (row, project, model, demo, shown) in [
        ("demo", f::PROJECT, true, true, true),
        ("forge alone", f::PROJECT, false, true, true),
        ("other project", f::SECOND_PROJECT, true, true, false),
        ("no answer", f::PROJECT, true, false, false),
    ] {
        let root = if demo { f::demo_root(Address::Chat { task: 1 }, project, model) } else { f::chat::chat_root() };
        let view = f::page_view(&root);
        let mut chatbar = None;
        let mut context = None;
        for (index, node) in view.tree().nodes().iter().enumerate() {
            if node.styles.as_slice().contains(&crate::look::CHATBAR) {
                chatbar = Some(index);
            }
            if node.styles.as_slice().contains(&crate::look::CONTEXT) {
                context = Some(index);
            }
        }
        assert_eq!(context.is_some(), shown, "{row}");
        if shown {
            let chatbar = chatbar.unwrap();
            assert_eq!(
                context.unwrap(),
                chatbar + usize::try_from(view.tree().nodes()[chatbar].size).unwrap(),
                "{row}"
            );
        }
    }
}

#[test]
fn the_context_repository_has_its_own_node_and_style() {
    // Each page has exactly one demo repository in its context.
    const REPOSITORIES: u32 = 1;
    for address in [Address::Chats { filter: ChatsFilter::All, start: false }, Address::Chat { task: f::chat::TASK }] {
        let root = f::demo_root(address, f::PROJECT, true);
        let view = f::page_view(&root);
        let mut repositories = skein_lib::List::with_capacity(REPOSITORIES);
        for (index, node) in view.tree().nodes().iter().enumerate() {
            if node.styles.as_slice().contains(&crate::look::REPOSITORY) {
                repositories.push(f::node_text(view.tree(), index)).expect("one repository");
                assert_eq!(node.role, Role::Text);
            }
        }
        assert_eq!(repositories.as_slice(), [Box::from(b"acme/shop".as_slice())]);
        assert_eq!(crate::look::NAMES.styles[usize::from(crate::look::REPOSITORY.token)], b"temper-repository");
    }
}

#[test]
fn the_invitation_is_a_named_region_with_its_parts_in_order() {
    let root = f::demo_root(Address::Chats { filter: ChatsFilter::All, start: false }, f::PROJECT, true);
    let view = f::page_view(&root);
    let tree = view.tree();
    let region = f::find(tree, Role::Region, b"What shall we change?").expect("named invitation region");
    let nodes = tree.nodes();
    let mut at = 0;
    for (index, node) in nodes.iter().enumerate() {
        if node.id == region {
            at = index;
        }
    }
    assert_eq!(nodes[at].styles.as_slice(), [crate::look::INVITATION]);
    let end = at + usize::try_from(nodes[at].size).unwrap();
    let mut child = at + 1;
    for (role, name) in [
        (Role::Heading { level: 1 }, b"What shall we change?".as_slice()),
        (Role::Paragraph, b"Try a request in acme/shop. Follow the task from its branch to a landed change."),
        (Role::List, b"Suggested requests"),
        (Role::Paragraph, b"The scripted model plays these requests from its script."),
    ] {
        assert!(child < end, "part belongs to invitation");
        assert_eq!(nodes[child].role, role);
        assert_eq!(nodes[child].name.as_deref(), Some(name));
        child += usize::try_from(nodes[child].size).unwrap();
    }
    assert_eq!(child, end, "all four parts stay inside the region");
}

#[test]
fn starting_waits_and_each_failure_explains_why_with_try_again() {
    use temper_client_domain::forge::demo::DemoState;
    use temper_client_protocol_wire::v1 as w;
    for (why, expected) in [
        (
            w::StartFailure::Group(
                w::GroupFailed::new(&w::CEILINGS, w::GroupFailedParts { why: b"fetch failed".as_slice().into() })
                    .unwrap(),
            ),
            "The playground did not start: its fakes did not start (fetch failed).",
        ),
        (w::StartFailure::Account, "The playground did not start: the scripted model's account was refused."),
        (w::StartFailure::Adoption, "The playground did not start: acme/shop was not adopted."),
        (w::StartFailure::NotConnected, "The playground did not start: Playground forge did not connect."),
    ] {
        let mut root =
            f::demo_state_root(Address::Chats { filter: ChatsFilter::All, start: false }, w::DemoState::NotStarted);
        assert_eq!(root.forge().demo().unwrap().state, DemoState::NotStarted);
        let attempt = f::demo_start_attempt(&mut root);
        let view = f::page_view(&root);
        assert!(f::find(view.tree(), Role::Status, b"Starting").is_some());
        let button = f::find(view.tree(), Role::Button, f::DEMO_CUE).unwrap();
        assert!(
            view.tree()
                .nodes()
                .iter()
                .find(|node| node.id == button)
                .unwrap()
                .states
                .as_slice()
                .contains(&skein_ui::State::Disabled)
        );
        f::demo_start_answer(
            &mut root,
            attempt,
            w::Outcome::StartFailed(w::StartFailed::new(&w::CEILINGS, w::StartFailedParts { why }).unwrap()),
        );
        let mut view = f::page_view(&root);
        assert!(f::find(view.tree(), Role::Status, b"Not started").is_some());
        assert!(f::find(view.tree(), Role::Alert, expected.as_bytes()).is_some(), "{expected}");
        let again = f::find(view.tree(), Role::Button, b"Try again").unwrap();
        assert_eq!(
            crate::act(&mut view, again, skein_ui::Act::Press),
            Some(temper_client_domain::Action::Pages(temper_client_domain::pages::Act::RetryDemo))
        );
    }
}
#[test]
fn a_not_started_demo_has_the_setup_strip_and_no_context_or_demo_banner() {
    use temper_client_protocol_wire::v1 as w;
    for address in [Address::Chats { filter: ChatsFilter::All, start: false }, Address::Chat { task: 41 }] {
        let root = f::demo_state_root(address, w::DemoState::NotStarted);
        let view = f::page_view(&root);
        assert!(f::find(view.tree(), Role::Region, b"Playground").is_none());
        assert!(f::find(view.tree(), Role::Region, b"Setup").is_some());
        assert!(view.tree().nodes().iter().all(|node| !node.styles.as_slice().contains(&crate::look::CONTEXT)));
    }
}

#[test]
fn a_full_group_failure_keeps_each_utf8_replacement_as_the_forge_failure_does() {
    use temper_client_protocol_wire::v1 as w;
    let mut root =
        f::demo_state_root(Address::Chats { filter: ChatsFilter::All, start: false }, w::DemoState::NotStarted);
    let attempt = f::demo_start_attempt(&mut root);
    let why = w::StartFailure::Group(
        w::GroupFailed::new(&w::CEILINGS, w::GroupFailedParts { why: Box::from([255_u8; 256]) }).unwrap(),
    );
    f::demo_start_answer(
        &mut root,
        attempt,
        w::Outcome::StartFailed(w::StartFailed::new(&w::CEILINGS, w::StartFailedParts { why }).unwrap()),
    );
    let domain = f::domain_limits();
    let words = f::words();
    let limits = crate::Limits::of(&domain, &words).unwrap();
    let mut view = crate::View::new(&limits, &domain, words, f::FORGE);
    crate::build(&mut view, &root, f::NOW, &mut skein_lib::Queue::with_capacity(limits.tree.patches));
    let alert = view.tree().nodes().iter().find(|node| node.role == Role::Alert).unwrap();
    assert_eq!(core::str::from_utf8(alert.name.as_deref().unwrap()).unwrap().matches('�').count(), 256);
}

#[test]
fn a_retired_reload_removes_the_welcome_playground_card() {
    let root = f::demo_state_root(
        Address::Welcome { step: temper_client_domain::Step::Welcome },
        temper_client_protocol_wire::v1::DemoState::Ended,
    );
    for words in [f::words(), f::browser_words()] {
        let mut view = f::view_of(words);
        let tree = f::build(&mut view, &root);
        assert!(f::find(tree, Role::Button, b"Try the playground").is_none());
        assert!(f::find(tree, Role::Heading { level: 2 }, b"Try the playground").is_none());
        assert!(f::find(tree, Role::Link, b"Set up temper").is_some());
    }
}
