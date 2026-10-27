#![allow(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    reason = "test assertions inspect semantic trees and their bound controls"
)]
use super::connect_forge as forge;
use super::fixtures as f;
use skein_ui::{Act, Role, State};
use temper_client_domain::ChatsFilter;
use temper_client_domain::{self as d, Address, Step, pages};
use temper_client_protocol_wire::v1 as w;

const HERE: u16 = 3;
/// The fake box's Forgejo elsewhere (temper-fake's `managed()`):
/// configured as its admin, not connected, with no `web`.
const ADMIN: u16 = 4;
fn rows_with_an_admin_forgejo() -> skein_lib::List<w::ForgeRow> {
    let mut rows = skein_lib::List::with_capacity(3);
    for row in &forge::rows(w::Connection::NotSetUp, w::Offer::Offered) {
        rows.push(row.clone()).unwrap();
    }
    let mut parts =
        forge::row(ADMIN, w::Kind::Forgejo, w::Mode::Managed, w::Connection::NotConnected, w::Offer::NotOffered)
            .into_parts();
    parts.admin = true;
    rows.push(w::ForgeRow::new(&w::CEILINGS, parts).unwrap()).unwrap();
    rows
}
fn choice_cards(tree: &skein_ui::Tree) -> Vec<&[u8]> {
    let choices = tree.nodes().iter().find(|n| n.styles.as_slice().contains(&crate::look::FLOW_CHOICES)).unwrap();
    tree.nodes()
        .iter()
        .filter(|n| forge::parent(tree, n.id) == Some(choices.id))
        .map(|n| n.name.as_deref().unwrap())
        .collect()
}

#[test]
fn the_welcome_leaves_a_configured_admin_forgejo_to_connect_a_forge() {
    for (address, cards) in [
        (
            Address::Welcome { step: Step::Forge { forge: None } },
            ["GitHub", "Forgejo on this machine", "A Forgejo elsewhere"].as_slice(),
        ),
        (
            Address::ConnectForge { forge: None },
            ["GitHub", "Forgejo on this machine", "Forgejo at forge 4", "A Forgejo elsewhere"].as_slice(),
        ),
    ] {
        let view = f::page_view(&f::forge_root(address.clone(), rows_with_an_admin_forgejo()));
        assert_eq!(
            choice_cards(view.tree()),
            cards.iter().map(|name| name.as_bytes()).collect::<Vec<_>>(),
            "{address:?}"
        );
    }
}

fn model_view(rows: skein_lib::List<w::ForgeRow>) -> crate::View {
    f::page_view(&f::forge_root(Address::Welcome { step: Step::Model { way: None } }, rows))
}
fn before_model(tree: &skein_ui::Tree) -> &[skein_ui::Node] {
    let main = tree.nodes().iter().position(|n| n.role == Role::Main).expect("welcome's main");
    let heading = node(tree, Role::Heading { level: 1 }, "Connect a model");
    let heading = tree.nodes().iter().position(|n| n.id == heading).unwrap();
    &tree.nodes()[main..heading]
}
fn model_forge_line(tree: &skein_ui::Tree) -> Option<&skein_ui::Node> {
    before_model(tree).iter().find(|n| n.role == Role::Paragraph)
}
fn line_before_model(tree: &skein_ui::Tree, expected: &str) -> skein_ui::NodeId {
    let line = model_forge_line(tree).expect("forge line before Model's heading");
    assert_eq!(line.name.as_deref(), Some(expected.as_bytes()));
    assert_eq!(before_model(tree).iter().filter(|n| n.role == Role::Paragraph).count(), 1);
    line.id
}
fn each_step(tree: &skein_ui::Tree, shown: bool) {
    assert_eq!(f::find(tree, Role::Disclosure, b"Each step").is_some(), shown);
}

#[test]
fn model_without_a_persons_forge_has_no_line() {
    let view = model_view(skein_lib::List::with_capacity(0));
    assert!(model_forge_line(view.tree()).is_none());
    each_step(view.tree(), false);
}

#[test]
fn model_names_the_new_project_while_forgejo_sets_up() {
    let (mut root, watch) = f::forge_root_watch(
        Address::Welcome { step: Step::Forge { forge: None } },
        forge::rows(w::Connection::NotSetUp, w::Offer::Offered),
    );
    f::root_event(&mut root, d::Event::Act { action: d::Action::Forge(d::forge::Act::SetUp { forge: HERE }) });
    // The setup request supplies the names until the watch's row carries them.
    forge::deliver(
        &mut root,
        watch,
        forge::rows(
            w::Connection::SettingUp(w::SettingUp::new(&w::CEILINGS, w::SettingUpParts { step: None }).unwrap()),
            w::Offer::Offered,
        ),
    );
    f::root_event(
        &mut root,
        d::Event::Went { place: d::Place::Address(Address::Welcome { step: Step::Model { way: None } }) },
    );
    let view = f::page_view(&root);
    line_before_model(view.tree(), "Forgejo on this machine · Setting up · work/hello, into the new project hello");
    each_step(view.tree(), true);
}

#[test]
fn model_ticks_the_first_repository_made_in_its_project() {
    let view = model_view(forge::first_rows(forge::connected(), forge::made()));
    let line = line_before_model(view.tree(), "Forgejo on this machine · work/hello, in Shop");
    let line = view.tree().nodes().iter().find(|n| n.id == line).unwrap();
    assert!(line.styles.as_slice().contains(&crate::look::FLOW_DONE));
    let index = view.tree().nodes().iter().position(|n| n.id == line.id).unwrap();
    assert!(f::node_text(view.tree(), index).starts_with("✓ ".as_bytes()));
    each_step(view.tree(), false);
}

#[test]
fn model_says_github_has_no_repository_chosen_yet() {
    let view = model_view(forge::github_rows(true));
    line_before_model(view.tree(), "GitHub · no repository chosen yet");
    each_step(view.tree(), true);
}

#[test]
fn model_keeps_github_connecting_until_an_installation_is_accepted() {
    let view = model_view(forge::github_rows(false));
    line_before_model(view.tree(), "GitHub · Connecting: no installation accepted yet");
    each_step(view.tree(), true);
}

#[test]
fn model_says_which_setup_failed_and_keeps_its_steps() {
    let view = model_view(forge::rows(
        w::Connection::SetUpFailed(
            w::SetUpFailed::new(&w::CEILINGS, w::SetUpFailedParts { why: b"fetch failed".as_slice().into(), at: None })
                .unwrap(),
        ),
        w::Offer::Offered,
    ));
    let line = line_before_model(view.tree(), "Forgejo on this machine · Set up failed: fetch failed");
    let line = view.tree().nodes().iter().find(|n| n.id == line).unwrap();
    assert!(!line.styles.as_slice().contains(&crate::look::FLOW_DONE));
    each_step(view.tree(), true);
}

#[test]
fn model_names_the_failed_first_repository_step() {
    for (step, connection, failed) in [
        (
            w::CreateStep::Admin,
            "Forgejo on this machine · Set up failed: checking the admin's token · work/hello, into Shop",
            "Making work/hello failed: checking the admin's token",
        ),
        (
            w::CreateStep::Org,
            "Forgejo on this machine · Set up failed: making the organization · work/hello, into Shop",
            "Making work/hello failed: making the organization",
        ),
        (
            w::CreateStep::Repository,
            "Forgejo on this machine · Set up failed: making the repository · work/hello, into Shop",
            "Making work/hello failed: making the repository",
        ),
        (
            w::CreateStep::File,
            "Forgejo on this machine · Set up failed: adding the starter workflow · work/hello, into Shop",
            "Making work/hello failed: adding the starter workflow",
        ),
    ] {
        let view = model_view(forge::first_rows(
            forge::connected(),
            w::FirstState::Failed(w::CreateFailed::new(&w::CEILINGS, w::CreateFailedParts { step }).unwrap()),
        ));
        line_before_model(view.tree(), connection);
        each_step(view.tree(), true);
        forge::words_in_order(view.tree(), &["failed", failed]);
        node(view.tree(), Role::Status, "failed");
    }
}

#[test]
fn model_keeps_steps_while_the_first_repository_is_making() {
    let view = model_view(forge::first_rows(forge::connected(), w::FirstState::Making));
    line_before_model(view.tree(), "Forgejo on this machine · Connected as temper (42) · work/hello, into Shop");
    each_step(view.tree(), true);
    node(view.tree(), Role::Status, "under way");
}

fn node(tree: &skein_ui::Tree, role: Role, name: &str) -> skein_ui::NodeId {
    f::find(tree, role, name.as_bytes()).unwrap_or_else(|| panic!("missing {role:?}: {name}"))
}
fn press(root: &d::Domain, role: Role, name: &str) -> d::Action {
    let mut view = f::page_view(root);
    let id = node(view.tree(), role, name);
    crate::act(&mut view, id, Act::Press).expect("bound control")
}
#[test]
fn the_welcome_has_a_bare_frame_and_no_banner() {
    let root = f::welcome_root(Step::Welcome, true);
    let view = f::page_view(&root);
    let tree = view.tree();
    assert!(
        tree.nodes()
            .iter()
            .find(|n| n.role == Role::Heading { level: 1 })
            .unwrap()
            .styles
            .as_slice()
            .contains(&crate::look::WELCOME_INTRO)
    );
    node(tree, Role::Button, "Sign out");
    assert_eq!(f::banner_statuses(tree).as_ref(), &[Box::from(b"Pat".as_slice())]);
    for role in [Role::Disclosure, Role::Navigation] {
        assert!(!tree.nodes().iter().any(|node| node.role == role));
    }
    assert!(f::find(tree, Role::Region, b"Playground").is_none());
    assert!(!String::from_utf8_lossy(&f::text(tree)).contains("No model yet"));
    assert_eq!(tree.nodes().iter().filter(|node| node.role == Role::Region).count(), 2);
}
#[test]
fn the_welcome_step_says_its_words_and_offers_both_cards() {
    let root = f::welcome_root(Step::Welcome, true);
    assert!(root.pages().welcome().unwrap().decided);
    let view = f::page_view(&root);
    let mut bytes = Vec::new();
    for node in view.tree().nodes() {
        if node.role == Role::Button && node.text.is_empty() {
            bytes.extend_from_slice(node.name.as_deref().unwrap());
        }
        for run in &node.text {
            bytes.extend_from_slice(&run.text);
        }
    }
    let words = String::from_utf8(bytes).unwrap();
    let mut remaining = words.as_str();
    for phrase in [
        "Welcome",
        "Forge",
        "Model",
        "Ready",
        "Welcome to temper",
        "Reliable, scalable and flexible software factory.",
        "You describe what you want and how you want it done, and it happens.",
        "Try the playground",
        "A built-in scripted model and forge. Ask one of three requests and follow its change from a branch to main.",
        "Nothing leaves this machine, and nothing costs. · Ready at once",
        "Try the playground",
        "Set up temper",
        "Your forge: GitHub or Forgejo",
        "About five minutes",
        "Set up temper",
        "Started in the playground? Connect your own forge and model whenever you like; the playground then steps aside.",
    ] {
        let at = remaining.find(phrase).unwrap_or_else(|| panic!("missing or out of order: {phrase}"));
        remaining = &remaining[at + phrase.len()..];
    }
    let primary: Vec<_> = view
        .tree()
        .nodes()
        .iter()
        .filter(|n| n.styles.as_slice().contains(&crate::look::FLOW_PRIMARY))
        .map(|n| n.name.as_deref().unwrap())
        .collect();
    assert_eq!(primary, [b"Try the playground".as_slice(), b"Set up temper"]);
    assert_eq!(press(&root, Role::Button, "Try the playground"), d::Action::Pages(pages::Act::Playground));
    assert_eq!(
        press(&root, Role::Link, "Set up temper"),
        d::Action::Pages(pages::Act::Go { address: Address::Welcome { step: Step::Forge { forge: None } } })
    );
}
#[test]
fn the_welcome_lead_has_two_named_paragraphs_and_strong_what_and_how() {
    let view = f::page_view(&f::welcome_root(Step::Welcome, true));
    let tree = view.tree();
    let first = node(tree, Role::Paragraph, "Reliable, scalable and flexible software factory.");
    let second = node(tree, Role::Paragraph, "You describe what you want and how you want it done, and it happens.");
    for id in [first, second] {
        let paragraph = tree.nodes().iter().find(|n| n.id == id).unwrap();
        assert!(paragraph.styles.as_slice().contains(&crate::look::WELCOME_LEAD));
    }
    let start = tree.nodes().iter().position(|n| n.id == first).unwrap();
    assert_eq!(tree.nodes()[start + usize::try_from(tree.nodes()[start].size).unwrap()].id, second);
    let start = tree.nodes().iter().position(|n| n.id == second).unwrap();
    let text = &tree.nodes()[start + 1];
    assert_eq!(text.role, Role::Text);
    let runs: Vec<_> = text.text.iter().map(|r| (r.run, r.text.as_ref())).collect();
    assert_eq!(
        runs,
        [
            (skein_ui::Run::Plain, b"You describe ".as_slice()),
            (skein_ui::Run::Strong, b"what".as_slice()),
            (skein_ui::Run::Plain, b" you want and ".as_slice()),
            (skein_ui::Run::Strong, b"how".as_slice()),
            (skein_ui::Run::Plain, b" you want it done, and it happens.".as_slice()),
        ]
    );
}
#[test]
fn a_document_with_no_demo_has_neither_the_playground_card_nor_the_closing_line() {
    let root = f::welcome_root(Step::Welcome, false);
    assert!(root.pages().welcome().unwrap().decided);
    let view = f::page_view(&root);
    node(view.tree(), Role::Region, "Set up temper");
    let words = String::from_utf8_lossy(&f::text(view.tree())).into_owned();
    assert!(!words.contains("playground"));
}
#[test]
fn the_welcome_shows_its_heading_alone_until_it_decides() {
    let (mut root, _, _) = f::root_at(Address::Chats { filter: ChatsFilter::All, start: false }, &f::limits());
    f::root_event(&mut root, d::Event::Went { place: d::Place::Address(Address::Welcome { step: Step::Welcome }) });
    assert!(!root.pages().welcome().unwrap().decided);
    let view = f::page_view(&root);
    node(view.tree(), Role::Heading { level: 1 }, "Welcome to temper");
    assert!(!view.tree().nodes().iter().any(|node| node.role == Role::Region));
    assert!(
        !view
            .tree()
            .nodes()
            .iter()
            .any(|node| node.role == Role::Paragraph && node.styles.as_slice().contains(&crate::look::WELCOME_LEAD))
    );
}
#[test]
fn the_steps_mark_the_current_one() {
    for (step, name) in [
        (Step::Welcome, "Welcome"),
        (Step::Forge { forge: None }, "Forge"),
        (Step::Model { way: None }, "Model"),
        (Step::Ready, "Ready"),
    ] {
        let view = f::page_view(&f::welcome_root(step, true));
        let heading = view.tree().nodes().iter().find(|n| n.role == Role::Heading { level: 1 }).unwrap();
        assert_eq!(heading.styles.as_slice().contains(&crate::look::WELCOME_INTRO), step == Step::Welcome);
        node(view.tree(), Role::List, "Steps");
        let current: Vec<_> =
            view.tree().nodes().iter().filter(|node| node.states.as_slice().contains(&State::Current)).collect();
        assert_eq!(current.len(), 1);
        assert_eq!(current[0].role, Role::Item);
        assert_eq!(
            f::nodes_text(
                &view.tree().nodes()[view.tree().nodes().iter().position(|node| node.id == current[0].id).unwrap()..]
                    [..usize::try_from(current[0].size).unwrap()]
            )
            .as_ref(),
            name.as_bytes()
        );
    }
}
#[test]
fn the_model_step_offers_the_accounts_ways() {
    let mut root = super::connect_model::offered_root();
    f::root_event(
        &mut root,
        d::Event::Went { place: d::Place::Address(Address::Welcome { step: Step::Model { way: None } }) },
    );
    let view = f::page_view(&root);
    node(view.tree(), Role::Heading { level: 1 }, "Connect a model");
    node(view.tree(), Role::Paragraph, "temper’s agents run on it: on a plan you sign in to, or on a key.");
    node(view.tree(), Role::Button, "Continue with ChatGPT");
    let action = press(&root, Role::Link, "Use an API key");
    let d::Action::Pages(pages::Act::Go { address: Address::Welcome { step: Step::Model { way: Some(_) } } }) = &action
    else {
        panic!("shared account binding")
    };
    let d::Action::Pages(pages::Act::Go { address }) = action else { unreachable!() };
    f::root_event(&mut root, d::Event::Went { place: d::Place::Address(address) });
    let view = f::page_view(&root);
    assert!(view.tree().nodes().iter().any(|node| node.role == Role::TextField));
}
#[test]
fn each_later_step_offers_back_later_and_the_playground_while_the_demo_is_answered() {
    for (step, back) in
        [(Step::Forge { forge: None }, Step::Welcome), (Step::Model { way: None }, Step::Forge { forge: None })]
    {
        for demo in [false, true] {
            let root = f::welcome_root(step, demo);
            assert_eq!(
                press(&root, Role::Link, "‹ Back"),
                d::Action::Pages(pages::Act::Go { address: Address::Welcome { step: back } })
            );
            assert_eq!(
                press(&root, Role::Link, "Later"),
                d::Action::Pages(pages::Act::Go { address: Address::Chats { filter: ChatsFilter::All, start: false } })
            );
            let view = f::page_view(&root);
            for name in ["‹ Back", "Later"] {
                let id = node(view.tree(), Role::Link, name);
                assert!(
                    view.tree()
                        .nodes()
                        .iter()
                        .find(|n| n.id == id)
                        .unwrap()
                        .styles
                        .as_slice()
                        .contains(&crate::look::FLOW_QUIET)
                );
            }
            assert_eq!(f::find(view.tree(), Role::Button, b"Try the playground instead").is_some(), demo);
            if demo {
                let id = node(view.tree(), Role::Button, "Try the playground instead");
                assert!(
                    !view
                        .tree()
                        .nodes()
                        .iter()
                        .find(|n| n.id == id)
                        .unwrap()
                        .styles
                        .as_slice()
                        .contains(&crate::look::FLOW_PRIMARY)
                );
                assert_eq!(
                    press(&root, Role::Button, "Try the playground instead"),
                    d::Action::Pages(pages::Act::Playground)
                );
            }
        }
    }
}
#[test]
fn an_unavailable_welcome_keeps_the_signed_out_and_out_of_date_frame() {
    for event in [
        d::Event::SignedOut,
        d::Event::Client {
            event: jig_client::Event::Up { up: jig_client::Up::Served { served: false, versions: None } },
        },
    ] {
        let mut root = f::welcome_root(Step::Welcome, true);
        f::root_event(&mut root, event);
        let view = f::page_view(&root);
        node(view.tree(), Role::Disclosure, "Menu");
        assert!(f::find(view.tree(), Role::Heading { level: 1 }, b"Welcome to temper").is_none());
    }
}

fn ready_model_fixture() -> (d::Domain, skein_lib::Token) {
    let (mut root, watch) = super::connect_model::flow_fixture(d::WayKind::SignIn);
    f::root_event(&mut root, d::Event::Went { place: d::Place::Address(Address::Welcome { step: Step::Ready }) });
    (root, watch)
}
fn signing_document() -> jig_client::contract::OwnerAccounts {
    let mut doc =
        super::accounts::card_document(b"openai", b"alice@example.com", jig_client::contract::OwnerAccountKind::SignIn);
    doc.accounts.clear();
    doc.usable = false;
    doc
}
#[test]
fn ready_waits_for_the_accounts_document_before_drawing_a_model_line() {
    let (root, _) = ready_model_fixture();
    let view = f::page_view(&root);
    let words = String::from_utf8_lossy(&f::text(view.tree())).into_owned();
    assert!(!words.contains("Model:"));
    assert!(f::find(view.tree(), Role::Link, b"Connect a model").is_none());
}
#[test]
fn ready_names_an_own_sign_in_under_way_without_a_connect_act() {
    let (mut root, watch) = ready_model_fixture();
    super::accounts::deliver(&mut root, watch, signing_document());
    let view = f::page_view(&root);
    node(view.tree(), Role::Paragraph, "Model: ChatGPT · signing in");
    assert!(f::find(view.tree(), Role::Link, b"Connect a model").is_none());
}
#[test]
fn ready_does_not_name_another_holders_sign_in() {
    let (mut root, watch) = ready_model_fixture();
    let mut doc = signing_document();
    doc.signing.get_mut(0).unwrap().holder = jig_client::contract::OwnerAccountHolder::Deployment;
    super::accounts::deliver(&mut root, watch, doc);
    let view = f::page_view(&root);
    node(view.tree(), Role::Slot, "Model: none yet");
    node(view.tree(), Role::Link, "Connect a model");
}
#[test]
fn ready_names_a_usable_account_before_a_sign_in_under_way() {
    let (mut root, watch) = ready_model_fixture();
    super::accounts::deliver(
        &mut root,
        watch,
        super::accounts::card_document(b"openai", b"alice@example.com", jig_client::contract::OwnerAccountKind::SignIn),
    );
    let view = f::page_view(&root);
    node(view.tree(), Role::Paragraph, "Model: ChatGPT · alice@example.com · OpenAI · held by Pat");
    assert!(f::find(view.tree(), Role::Link, b"Connect a model").is_none());
}

fn configured_model_document() -> jig_client::contract::OwnerAccounts {
    let mut document = signing_document();
    document.signing.clear();
    document.usable = true;
    document
}
fn configured_model_fixture(address: Address) -> d::Domain {
    let (mut root, watch) = f::forge_root_accounts(
        address,
        super::connect_forge::rows(
            temper_client_protocol_wire::v1::Connection::Connecting,
            temper_client_protocol_wire::v1::Offer::Offered,
        ),
    );
    let document = configured_model_document();
    super::accounts::deliver(&mut root, watch, document);
    root
}
#[test]
fn ready_names_the_configured_model_without_an_account_row() {
    let root = configured_model_fixture(Address::Welcome { step: Step::Ready });
    let view = f::page_view(&root);
    node(view.tree(), Role::Paragraph, "Model: the default model");
    assert!(f::find(view.tree(), Role::Link, b"Connect a model").is_none());
}
#[test]
fn the_setup_strip_continues_to_ready_with_a_configured_model() {
    let root = configured_model_fixture(Address::Chats { filter: ChatsFilter::All, start: false });
    assert_eq!(
        press(&root, Role::Link, "Continue setting up"),
        d::Action::Pages(pages::Act::Go { address: Address::Welcome { step: Step::Ready } })
    );
}

#[test]
fn ready_names_the_new_chat_templates_model_and_effort() {
    use jig_client::contract as c;
    let text_bytes = f::limits().client.domain.text;
    let (mut root, me_watch, _, _, accounts_watch) =
        f::root_at_full(Address::Welcome { step: Step::Ready }, &f::limits(), b"Shop");
    let template_root = f::chat::messaging_root();
    let mut me = pages::accounts::current_me(template_root.client()).unwrap().clone();
    let project = me.projects.get_mut(0).unwrap();
    let template = project.templates.get_mut(0).unwrap();
    template.llm = Some(c::LlmChoice {
        account: None,
        main: c::ModelEffort {
            model: c::Text::new(b"configured-model".as_slice().into(), text_bytes).unwrap(),
            effort: Some(c::Text::new(b"high".as_slice().into(), text_bytes).unwrap()),
        },
        subagents: skein_lib::List::with_capacity(0),
    });
    f::root_event(
        &mut root,
        d::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched {
                    watch: me_watch,
                    event: c::WatchEvent::Delivered(c::Delivery::Me(Box::new(me))),
                },
            },
        },
    );
    let document = configured_model_document();
    super::accounts::deliver(&mut root, accounts_watch.unwrap(), document);
    let view = f::page_view(&root);
    node(view.tree(), Role::Paragraph, "Model: configured-model · high");
}

#[test]
fn selected_forge_choices_have_one_back_to_choose() {
    for choice in [d::Choice::Forge(2), d::Choice::Forge(3), d::Choice::Elsewhere] {
        let root = f::forge_root(
            Address::Welcome { step: Step::Forge { forge: Some(choice) } },
            forge::rows(w::Connection::NotSetUp, w::Offer::Offered),
        );
        let view = f::browser_page_view(&root);
        assert!(f::find(view.tree(), Role::Link, "‹ All forges".as_bytes()).is_none());
        assert_eq!(view.tree().nodes().iter().filter(|n| n.name.as_deref() == Some("‹ Back".as_bytes())).count(), 1);
        assert_eq!(
            press(&root, Role::Link, "‹ Back"),
            d::Action::Pages(pages::Act::Go { address: Address::Welcome { step: Step::Forge { forge: None } } })
        );
        if choice == d::Choice::Forge(2) {
            node(view.tree(), Role::Heading { level: 2 }, "An App of your own");
            assert!(f::find(view.tree(), Role::Heading { level: 2 }, b"GitHub").is_none());
        }
    }
}

#[test]
fn ready_has_no_foot_controls() {
    for demo in [false, true] {
        let root = f::welcome_root(Step::Ready, demo);
        let view = f::page_view(&root);
        for (role, name) in
            [(Role::Link, "‹ Back"), (Role::Link, "Later"), (Role::Button, "Try the playground instead")]
        {
            assert!(f::find(view.tree(), role, name.as_bytes()).is_none());
        }
        assert!(!view.tree().nodes().iter().any(|n| n.styles.as_slice().contains(&crate::look::WELCOME_FOOT)));
    }
}

#[test]
fn a_connected_real_forge_hides_the_playground_before_its_end_is_read() {
    let mut cases = Vec::new();
    cases.push(forge::rows(forge::connected(), w::Offer::Offered));
    for app in [w::AppWay::Manifest, w::AppWay::Pasted] {
        let mut rows = forge::github_rows(false);
        let mut parts = rows.get(0).unwrap().clone().into_parts();
        parts.app = Some(app);
        *rows.get_mut(0).unwrap() = w::ForgeRow::new(&w::CEILINGS, parts).unwrap();
        cases.push(rows);
    }
    for rows in cases {
        for step in [Step::Forge { forge: None }, Step::Model { way: None }] {
            let root = f::demo_forges_root(Address::Welcome { step }, rows.clone());
            assert!(root.forge().demo().is_some(), "the cached answer has not been reread yet");
            let view = f::page_view(&root);
            assert!(f::find(view.tree(), Role::Button, b"Try the playground instead").is_none());
        }
    }
}
#[test]
fn ready_under_way_lists_model_forge_and_project_with_folded_progress() {
    let (mut root, account) = f::forge_root_accounts(
        Address::Welcome { step: Step::Ready },
        forge::first_rows(w::Connection::Connecting, w::FirstState::Making),
    );
    f::welcome_account(&mut root, account);
    let view = f::page_view(&root);
    node(view.tree(), Role::Heading { level: 1 }, "Almost ready");
    forge::words_in_order(
        view.tree(),
        &[
            "Model:",
            "Forge: Forgejo on this machine",
            "Project Shop, with work/hello",
            "Open Shop",
            "It opens once work/hello is made.",
        ],
    );
    let after = node(view.tree(), Role::Paragraph, "It opens once work/hello is made.");
    let after = view.tree().nodes().iter().find(|n| n.id == after).unwrap();
    assert!(after.styles.as_slice().contains(&crate::look::WELCOME_AFTER));
    assert!(after.styles.as_slice().contains(&crate::look::WELCOME_TINY));
    let disclosure = node(view.tree(), Role::Disclosure, "Each step");
    assert!(
        !view.tree().nodes().iter().find(|n| n.id == disclosure).unwrap().states.as_slice().contains(&State::Expanded)
    );
    assert!(!view.tree().nodes().iter().any(|n| n.styles.as_slice().contains(&crate::look::FLOW_CARD)));
    let open = node(view.tree(), Role::Button, "Open Shop");
    assert!(view.tree().nodes().iter().find(|n| n.id == open).unwrap().states.as_slice().contains(&State::Disabled));
}

#[test]
fn model_choose_excludes_the_playgrounds_provider_before_and_after_retirement() {
    let address = Address::Welcome { step: Step::Model { way: None } };
    let (mut root, watch) = super::choice::playground_offers_root(address, true);
    let check = |root: &d::Domain| {
        let mut view = f::page_view(root);
        let tree = view.tree();
        assert!(f::find(tree, Role::Heading { level: 2 }, b"A demo API key").is_none());
        node(tree, Role::Heading { level: 2 }, "ChatGPT");
        node(tree, Role::Heading { level: 2 }, "An OpenAI API key");
        let headings: Vec<_> = tree
            .nodes()
            .iter()
            .filter(|n| n.role == (Role::Heading { level: 2 }))
            .map(|n| n.name.as_deref().unwrap())
            .collect();
        assert_eq!(headings, [b"ChatGPT".as_slice(), b"An OpenAI API key"]);
        let key = node(tree, Role::Link, "Use an API key");
        assert_eq!(
            crate::act(&mut view, key, Act::Press),
            Some(d::Action::Pages(pages::Act::Go {
                address: Address::Welcome {
                    step: Step::Model { way: Some(d::ModelWay { provider: 18, source: None, kind: d::WayKind::Key }) }
                }
            }))
        );
    };
    check(&root);
    let read = super::choice::retire_playground(&mut root, watch);
    check(&root);
    super::choice::answer_retired_playground(&mut root, read);
    check(&root);
}

#[test]
fn welcome_keeps_failed_setup_steps_at_model_forge_and_ready() {
    for step in [Step::Model { way: None }, Step::Forge { forge: Some(d::Choice::Forge(HERE)) }, Step::Ready] {
        for (rows, failed, why) in forge::failure_cases() {
            let view = f::page_view(&f::forge_root(Address::Welcome { step }, rows));
            forge::failed_progress(view.tree(), failed, why);
        }
    }
}

fn model_address(kind: d::WayKind) -> Address {
    Address::Welcome { step: Step::Model { way: Some(d::ModelWay { provider: 17, kind, source: None }) } }
}
fn in_welcome(root: &mut d::Domain, kind: d::WayKind) {
    f::root_event(root, d::Event::Went { place: d::Place::Address(model_address(kind)) });
}
fn model_back(root: &d::Domain) {
    let view = f::page_view(root);
    let tree = view.tree();
    assert_eq!(tree.nodes().iter().filter(|n| n.name.as_deref() == Some("‹ Back".as_bytes())).count(), 1);
    for back in ["‹ Back", "‹ All ways"] {
        assert_eq!(
            press(root, Role::Link, back),
            d::Action::Pages(pages::Act::Go { address: Address::Welcome { step: Step::Model { way: None } } })
        );
    }
}
#[test]
fn model_choose_has_direct_ways_and_the_persons_default_holder() {
    use jig_client::contract as c;
    let mut root = super::connect_model::offered_root();
    f::root_event(
        &mut root,
        d::Event::Went { place: d::Place::Address(Address::Welcome { step: Step::Model { way: None } }) },
    );
    let view = f::page_view(&root);
    let tree = view.tree();
    for name in ["Use your accounts", "Use deployment accounts"] {
        assert!(f::find(tree, Role::Button, name.as_bytes()).is_none());
    }
    assert!(f::find(tree, Role::Region, b"Choose").is_none());
    assert_eq!(root.pages().connect_model().unwrap().holder, c::AccountHolder::Own);
    assert_eq!(tree.nodes().iter().filter(|n| n.styles.as_slice().contains(&crate::look::FLOW_CHOICE)).count(), 2);
    assert_eq!(
        press(&root, Role::Link, "‹ Back"),
        d::Action::Pages(pages::Act::Go { address: Address::Welcome { step: Step::Forge { forge: None } } })
    );
}
#[test]
fn model_details_and_signing_use_one_back_to_their_choose() {
    for kind in [d::WayKind::Key, d::WayKind::Program, d::WayKind::SignIn] {
        let (mut root, _) = super::connect_model::flow_fixture(kind);
        in_welcome(&mut root, kind);
        model_back(&root);
        let view = f::page_view(&root);
        let tree = view.tree();
        if kind == d::WayKind::Key {
            node(tree, Role::Heading { level: 2 }, "An OpenAI API key");
            let field = node(tree, Role::TextField, "Key (pasted once)");
            let field = tree.nodes().iter().find(|n| n.id == field).unwrap();
            assert_eq!(field.placeholder, None);
            assert_eq!(field.entry, Some(skein_ui::EntryKind::Secret));
            assert!(f::find(tree, Role::TextField, b"Name").is_none());
            node(tree, Role::Button, "Add key");
            let words = String::from_utf8(f::text(tree).into_vec()).unwrap();
            assert!(words.contains(
                "From OpenAI’s platform, under API keys. What temper’s agents use is billed to the key’s organization."
            ));
            for absent in ["A pasted value is sent once", "A key reads Ready until a run is refused"] {
                assert!(!words.contains(absent), "drew {absent}");
            }
        } else if kind == d::WayKind::SignIn {
            node(tree, Role::Heading { level: 2 }, "Signing in to ChatGPT");
        }
    }
}
#[test]
fn model_endings_have_their_heading_and_one_back() {
    use jig_client::contract::{OwnerAccountEnd as E, OwnerAccountFailure as F, OwnerAccountRefusal as R};
    for (end, heading) in [
        (E::Made { account: 1, declined: false }, "Signed in to ChatGPT as alice@example.com"),
        (E::Renewed { account: 1, declined: false }, "Signing in to ChatGPT"),
        (E::Made { account: 1, declined: true }, "Signing in to ChatGPT"),
        (E::Renewed { account: 1, declined: true }, "Signing in to ChatGPT"),
        (E::Refused { refusal: R::Held { provider: 17 } }, "Signing in to ChatGPT"),
        (E::Failed { failure: F::Misconfigured }, "Signing in to ChatGPT"),
        (E::Failed { failure: F::SignInAgain }, "Signing in to ChatGPT"),
        (E::Failed { failure: F::MissingScope { scope: 19 } }, "Signing in to ChatGPT"),
    ] {
        let mut root = super::connect_model::ending(end, b"alice@example.com");
        let standalone = f::page_view(&root);
        node(standalone.tree(), Role::Heading { level: 2 }, heading);
        in_welcome(&mut root, d::WayKind::SignIn);
        model_back(&root);
        let view = f::page_view(&root);
        node(view.tree(), Role::Heading { level: 2 }, heading);
        if heading.starts_with("Signed in") {
            node(view.tree(), Role::Slot, "ChatGPT · alice@example.com is on the Accounts page, with its details.");
        }
    }
}

fn github_adopted_rows() -> skein_lib::List<w::ForgeRow> {
    let mut rows = forge::github_rows(true);
    let mut parts = rows.get(0).unwrap().clone().into_parts();
    parts.repositories = skein_lib::List::with_capacity(1);
    parts
        .repositories
        .push(
            w::Adoption::new(
                &w::CEILINGS,
                w::AdoptionParts {
                    owner: b"sam".as_slice().into(),
                    name: b"widget".as_slice().into(),
                    project: f::PROJECT,
                },
            )
            .unwrap(),
        )
        .unwrap();
    *rows.get_mut(0).unwrap() = w::ForgeRow::new(&w::CEILINGS, parts).unwrap();
    rows
}
#[test]
fn model_ticks_githubs_durable_adoption_with_its_named_project() {
    let view = model_view(github_adopted_rows());
    let id = line_before_model(view.tree(), "GitHub · sam/widget, in Shop");
    assert!(
        view.tree().nodes().iter().find(|n| n.id == id).unwrap().styles.as_slice().contains(&crate::look::FLOW_DONE)
    );
    each_step(view.tree(), false);
}
fn ready_root(rows: skein_lib::List<w::ForgeRow>) -> d::Domain {
    let (mut root, account) = f::forge_root_accounts(Address::Welcome { step: Step::Ready }, rows);
    f::welcome_account(&mut root, account);
    root
}
#[test]
fn managed_forgejo_ready_names_its_adopted_repository_and_opens_its_project() {
    // Forge 4 is Forgejo elsewhere in the standalone configuration.
    const ELSEWHERE: u16 = 4;
    let mut rows = owned_adopted_rows();
    let mut parts = rows.get(0).unwrap().clone().into_parts();
    parts.number = ELSEWHERE;
    parts.mode = w::Mode::Managed;
    parts.admin = true;
    *rows.get_mut(0).unwrap() = w::ForgeRow::new(&w::CEILINGS, parts).unwrap();
    let root = ready_root(rows);
    let view = f::page_view(&root);
    node(view.tree(), Role::Heading { level: 1 }, "work/hello is ready");
    forge::words_in_order(
        view.tree(),
        &["Forge: Forgejo, connected as temper", "Model:", "Project Shop, with work/hello", "Open Shop"],
    );
    assert_eq!(
        press(&root, Role::Link, "Open Shop"),
        d::Action::Pages(pages::Act::OpenProject { project: f::PROJECT })
    );
}
#[test]
fn github_ready_has_the_same_checklist_and_project_control_as_owned_forgejo() {
    for (rows, heading, forge_line, project_line) in [
        (
            github_adopted_rows(),
            "sam/widget is ready",
            "Forge: GitHub, connected as my-app[bot]",
            "Project Shop, with sam/widget",
        ),
        (
            owned_adopted_rows(),
            "work/hello is ready",
            "Forge: Forgejo on this machine, connected as temper",
            "Project Shop, with work/hello",
        ),
    ] {
        let root = ready_root(rows);
        let view = f::page_view(&root);
        node(view.tree(), Role::Heading { level: 1 }, heading);
        forge::words_in_order(view.tree(), &[forge_line, "Model:", project_line, "Open Shop"]);
        assert_eq!(
            press(&root, Role::Link, "Open Shop"),
            d::Action::Pages(pages::Act::OpenProject { project: f::PROJECT })
        );
        assert!(
            f::find(view.tree(), Role::Link, "Open on GitHub ↗".as_bytes()).is_none(),
            "no repository web address in the row"
        );
    }
}
fn ready_ending(end: jig_client::contract::OwnerAccountEnd, message: &str, key: bool) {
    use jig_client::contract as c;
    let (mut root, watch) = ready_model_fixture();
    let mut doc = signing_document();
    doc.signing.clear();
    doc.last_end = Some(c::OwnerAccountLastEnd { source: 43, end });
    super::accounts::deliver(&mut root, watch, doc);
    let view = f::page_view(&root);
    forge::words_in_order(view.tree(), &["Model: none yet", "Connect a model", message, "Continue with ChatGPT"]);
    let tree = view.tree();
    let nodes = tree.nodes();
    let line = nodes.iter().position(|n| n.name.as_deref() == Some(b"Model: none yet")).unwrap();
    let line_end = line.checked_add(usize::try_from(nodes[line].size).unwrap()).unwrap();
    assert!(
        nodes[line.checked_add(1).unwrap()..line_end]
            .iter()
            .any(|n| n.role == Role::Link && n.name.as_deref() == Some(b"Connect a model"))
    );
    assert!(f::node_text(tree, line).starts_with("Model: none yet · ".as_bytes()));
    let ending = line_end;
    assert_eq!(nodes[ending].role, Role::Slot);
    assert!(nodes[ending].styles.as_slice().contains(&crate::look::WELCOME_END));
    let end = ending.checked_add(usize::try_from(nodes[ending].size).unwrap()).unwrap();
    assert!(nodes[ending..end].iter().any(|n| n.role == Role::Alert && n.name.as_deref() == Some(message.as_bytes())));
    assert!(
        nodes[ending..end]
            .iter()
            .any(|n| n.role == Role::Button && n.name.as_deref() == Some(b"Continue with ChatGPT"))
    );
    node(tree, Role::Alert, message);
    assert_eq!(f::find(view.tree(), Role::Link, b"Use an API key").is_some(), key);
    if key {
        assert_eq!(
            press(&root, Role::Link, "Use an API key"),
            d::Action::Pages(pages::Act::Go { address: model_address(d::WayKind::Key) })
        );
    }
    let action = press(&root, Role::Button, "Continue with ChatGPT");
    assert_eq!(
        action,
        d::Action::Pages(pages::Act::ConnectModel(pages::connect_model::Act::SignIn {
            name: root.pages().account_name(),
            way: pages::accounts::Way::SignIn { provider: 17, plan: true }
        }))
    );
    let limits = f::limits();
    let mut out = skein_lib::Queue::with_capacity(d::max_out(&limits));
    d::step(
        &mut root,
        &skein_lib::Env { now: skein_lib::Time::ZERO, wall: f::NOW, limits },
        d::Event::Act { action },
        &mut out,
    );
    let mut started = false;
    for request in &out {
        if let d::Request::Show { address: Address::Welcome { step: Step::Model { way: Some(_) } }, .. } = request {
            started = true;
        }
    }
    assert!(started, "Ready's retry starts the actual model flow");
}
#[test]
fn ready_draws_a_declined_sign_in_with_retry_and_key() {
    ready_ending(
        jig_client::contract::OwnerAccountEnd::Made { account: 1, declined: true },
        "You signed in without plan usage.",
        true,
    );
}
#[test]
fn ready_draws_a_held_sign_in_with_retry() {
    ready_ending(
        jig_client::contract::OwnerAccountEnd::Refused {
            refusal: jig_client::contract::OwnerAccountRefusal::Held { provider: 17 },
        },
        "Another person here holds that ChatGPT account. To use one of yours, sign out of ChatGPT in your browser first, then Continue with ChatGPT.",
        false,
    );
}
#[test]
fn ready_draws_a_misconfigured_sign_in_with_retry() {
    ready_ending(
        jig_client::contract::OwnerAccountEnd::Failed {
            failure: jig_client::contract::OwnerAccountFailure::Misconfigured,
        },
        "OpenAI refused temper’s sign-in. Check the configuration’s registration.",
        false,
    );
}

pub(super) fn owned_adopted_rows() -> skein_lib::List<w::ForgeRow> {
    let mut rows = forge::first_rows(forge::connected(), forge::made());
    let mut parts = rows.get(0).unwrap().clone().into_parts();
    parts.repositories = skein_lib::List::with_capacity(1);
    parts
        .repositories
        .push(
            w::Adoption::new(
                &w::CEILINGS,
                w::AdoptionParts {
                    owner: b"work".as_slice().into(),
                    name: b"hello".as_slice().into(),
                    project: f::PROJECT,
                },
            )
            .unwrap(),
        )
        .unwrap();
    *rows.get_mut(0).unwrap() = w::ForgeRow::new(&w::CEILINGS, parts).unwrap();
    rows
}

#[test]
fn the_setup_card_names_supported_models_for_every_provider_catalogue() {
    use jig_client::contract as c;
    for (sign_in, program, key) in
        [(true, false, false), (false, true, false), (false, false, true), (false, false, false)]
    {
        let (mut root, watch, _) = f::root_at(Address::Welcome { step: Step::Welcome }, &f::domain_limits());
        let mut me = root.pages().me(root.client()).unwrap().clone();
        me.providers = skein_lib::List::with_capacity(1);
        if sign_in || program || key {
            let mut programs = skein_lib::List::with_capacity(1);
            if program {
                programs.push(c::Text::new(Box::from(b"claude_code".as_slice()), 32).unwrap()).unwrap();
            }
            me.providers
                .push(c::ProviderOffer {
                    provider: 20,
                    name: super::connect_model::text(b"Other model"),
                    key_bytes: 4096,
                    sign_in,
                    signed_in_plan: true,
                    key,
                    programs,
                    programs_cut: false,
                    none: false,
                    manage_usage: None,
                })
                .unwrap();
        }
        f::root_event(
            &mut root,
            d::Event::Client {
                event: jig_client::Event::Up {
                    up: jig_client::Up::Watched {
                        watch,
                        event: c::WatchEvent::Delivered(c::Delivery::Me(Box::new(me))),
                    },
                },
            },
        );
        for view in [f::page_view(&root), f::browser_page_view(&root)] {
            let block = view
                .tree()
                .nodes()
                .iter()
                .position(|n| n.styles.as_slice().contains(&crate::look::WELCOME_SETUP_DESCRIPTION))
                .unwrap();
            let lines: Vec<_> = view
                .tree()
                .nodes()
                .iter()
                .enumerate()
                .skip(block + 1)
                .take(usize::try_from(view.tree().nodes()[block].size).unwrap() - 1)
                .filter(|(_, n)| n.role == Role::Paragraph)
                .map(|(at, _)| f::node_text(view.tree(), at))
                .collect();
            assert_eq!(lines.len(), 2);
            assert_eq!(lines[0].as_ref(), b"Your forge: GitHub or Forgejo");
            assert_eq!(lines[1].as_ref(), b"Your model: ChatGPT or Claude");
        }
    }
}

#[test]
fn welcomes_five_steps_use_the_owned_pins_without_a_future_login() {
    for (pins, runner, fetching, fetched) in [
        (true, true, "Fetching Forgejo 17.0.0 and its runner 11.2.1", "Fetched Forgejo 17.0.0 and its runner 11.2.1"),
        (true, false, "Fetching Forgejo and its runner", "Fetched Forgejo and its runner"),
        (false, false, "Fetching Forgejo and its runner", "Fetched Forgejo and its runner"),
    ] {
        for (step, expected) in [(w::SetUpStep::Fetch, fetching), (w::SetUpStep::Prepare, fetched)] {
            let connection = w::Connection::SettingUp(
                w::SettingUp::new(&w::CEILINGS, w::SettingUpParts { step: Some(step) }).unwrap(),
            );
            let root =
                f::forge_root(Address::Welcome { step: Step::Ready }, forge::pinned_rows(connection, pins, runner));
            for view in [f::page_view(&root), f::browser_page_view(&root)] {
                forge::words_in_order(
                    view.tree(),
                    &[expected, "Preparing", "Starting Forgejo and its runner", "Connecting", "Making"],
                );
                assert!(!String::from_utf8_lossy(&f::main_text(view.tree())).contains(" sam"));
            }
        }
    }
}

#[test]
fn ready_offers_forgejo_sign_in_only_after_its_owned_repository_is_adopted() {
    for (rows, offered) in [
        (owned_adopted_rows(), true),
        (forge::first_rows(forge::connected(), w::FirstState::Making), false),
        (
            forge::pinned_rows(
                w::Connection::SettingUp(
                    w::SettingUp::new(&w::CEILINGS, w::SettingUpParts { step: Some(w::SetUpStep::Fetch) }).unwrap(),
                ),
                true,
                true,
            ),
            false,
        ),
        (github_adopted_rows(), false),
    ] {
        let (mut root, accounts) = f::forge_root_accounts(Address::Welcome { step: Step::Ready }, rows);
        f::welcome_account(&mut root, accounts);
        for facts_present in [true, false] {
            let mut view = f::view_of(f::browser_words());
            if facts_present {
                let mut facts = crate::forge_sign_in::Forges::new();
                facts.read("3 forgejo http://127.0.0.1:3000 owned");
                facts.read("2 github http://127.0.0.1:3001 other");
                view.forge_sign_in(facts);
            }
            f::build(&mut view, &root);
            let link = f::find(view.tree(), Role::Link, "Open Forgejo ↗".as_bytes());
            assert_eq!(link.is_some(), offered && facts_present);
            if let Some(link) = link {
                assert_eq!(
                    crate::href(&mut view, link).as_deref(),
                    Some(b"http://127.0.0.1:3000/user/oauth2/temper".as_slice())
                );
                forge::words_in_order(view.tree(), &["Open Shop", "Open Forgejo ↗"]);
            }
        }
    }
}

#[test]
fn ready_offers_connect_a_model_as_a_terminal_command() {
    let (mut root, watch) = ready_model_fixture();
    let mut doc = signing_document();
    doc.signing.clear();
    super::accounts::deliver(&mut root, watch, doc);
    let mut view = f::page_view(&root);
    let control = node(view.tree(), Role::Link, "Connect a model");
    let (node, first) = f::terminal_command(
        view.tree(),
        &view.limits().tree,
        control,
        b"Connect a model",
        "Model: none yet · ".as_bytes(),
    );
    assert_eq!(
        crate::act(&mut view, node, first),
        Some(d::Action::Pages(pages::Act::Go { address: Address::Welcome { step: Step::Model { way: None } } }))
    );
}
