//! Milestone QA from client README 3.5/5.5/5.6 and forge 5.4.
//! The real client and engine are driven only through the public Joint.
use jig_client_world::harness::App;
use skein_lib::Queue;
use skein_ui::{Act, Role, Tree};
use temper_client_domain::{Address, Choice, Step};
use temper_client_world::{
    app::{Root, Temper, View},
    fixtures,
    front::{Joint, account_acts as h},
};

fn draw(root: &Root) -> View {
    let (_, mut view) = Temper::new(fixtures::SEED);
    let mut patches = Queue::with_capacity(Temper::view_limits(&view).patches);
    Temper::build(&mut view, root, &mut patches);
    view
}
fn text(tree: &Tree) -> String {
    tree.nodes()
        .iter()
        .flat_map(|n| n.text.iter())
        .map(|r| String::from_utf8_lossy(&r.text))
        .collect::<Vec<_>>()
        .join("")
}
fn has(root: &Root, role: Role, name: &[u8]) -> bool {
    Temper::tree(&draw(root)).nodes().iter().any(|n| n.role == role && n.name.as_deref() == Some(name))
}
fn act(front: &mut Joint, root: &mut Root, role: Role, name: &[u8], action: Act) {
    let mut view = draw(root);
    let matches = Temper::tree(&view)
        .nodes()
        .iter()
        .filter(|n| n.role == role && n.name.as_deref() == Some(name))
        .map(|n| n.id)
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "unique {:?} {:?}\n{}",
        role,
        String::from_utf8_lossy(name),
        text(Temper::tree(&view))
    );
    let event = Temper::act(&mut view, matches[0], action).expect("offered act decodes");
    h::event(front, root, event);
}
fn press(front: &mut Joint, root: &mut Root, role: Role, name: &[u8]) {
    act(front, root, role, name, Act::Press);
}
fn go(front: &mut Joint, root: &mut Root, at: Address) {
    h::event(front, root, Temper::went(at));
}
fn welcome(step: Step) -> Address {
    Address::Welcome { step }
}

#[test]
fn brand_returns_to_chats_from_every_welcome_step_without_starting_the_playground() {
    for step in [Step::Welcome, Step::Forge { forge: None }, Step::Model { way: None }, Step::Ready] {
        let (mut front, mut root) =
            h::demo_fixture(fixtures::SEED, &temper_client_protocol_wire::v1::DemoState::NotStarted);
        go(&mut front, &mut root, welcome(step));
        assert!(has(&root, Role::Link, b"temper chats"));
        assert!(!has(&root, Role::Link, b"Inbox"));
        press(&mut front, &mut root, Role::Link, b"temper chats");
        assert!(has(&root, Role::Link, b"Accounts"), "{}", text(Temper::tree(&draw(&root))));
        assert!(has(&root, Role::Heading { level: 1 }, b"Chats"), "{}", text(Temper::tree(&draw(&root))));
        assert!(front.demo_starts.is_empty(), "brand must only navigate");
    }
}

#[test]
fn setup_model_sentence_names_supported_plans_for_different_providers() {
    for fixture in [h::empty_plan_fixture as fn(u64) -> (Joint, Root), h::program_fixture as fn(u64) -> (Joint, Root)] {
        let (mut front, mut root) = fixture(fixtures::SEED);
        go(&mut front, &mut root, welcome(Step::Welcome));
        let shown = text(Temper::tree(&draw(&root)));
        let expected = "Your model: ChatGPT or Claude";
        assert!(shown.contains(expected), "expected {expected:?}\n{shown}");
        assert_eq!(shown.matches("Your model:").count(), 1);
        assert!(!shown.contains("API key"), "{shown}");
        assert!(!shown.contains(&format!("{expected}.")), "{shown}");
    }
}

#[test]
fn setup_model_sentence_keeps_supported_plans_in_the_playground() {
    let (front, root) = h::demo_fixture(fixtures::SEED, &temper_client_protocol_wire::v1::DemoState::NotStarted);
    let shown = text(Temper::tree(&draw(&root)));
    assert!(shown.contains("Your model: ChatGPT or Claude"), "{shown}");
    assert!(!shown.contains("Other fake"), "{shown}");
    assert!(!shown.contains("Scripted"), "{shown}");
    assert!(front.demo_starts.is_empty());
}

#[test]
fn github_picker_marks_only_the_private_repository_and_keeps_names_plain() {
    let (front, root) = Joint::github_fixture(fixtures::SEED);
    let view = draw(&root);
    let tree = Temper::tree(&view);
    let shown = format!(
        "{}\n{}",
        text(tree),
        tree.nodes()
            .iter()
            .filter(|n| n.name.is_some())
            .map(|n| format!("{:?}: {}", n.role, String::from_utf8_lossy(n.name.as_deref().expect("named"))))
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(has(&root, Role::Choice, b"acme/shop"), "{shown}");
    assert!(has(&root, Role::Choice, b"acme/second"), "{shown}");
    assert!(!has(&root, Role::Choice, b"acme/second (private)"), "{shown}");
    assert_eq!(
        tree.nodes().iter().filter(|n| n.text.iter().any(|r| r.text.as_ref() == b"private")).count(),
        1,
        "{shown}"
    );
    let choices =
        tree.nodes().iter().enumerate().filter(|(_, n)| n.role == Role::Choice).map(|(at, _)| at).collect::<Vec<_>>();
    let private =
        tree.nodes().iter().position(|n| n.text.iter().any(|r| r.text.as_ref() == b"private")).expect("private tag");
    let project = tree.nodes().iter().position(|n| n.role == Role::TextField).expect("project field");
    assert_eq!(choices.len(), 2);
    assert!(choices[0] < choices[1] && choices[1] < private && private < project, "private belongs to the second row");
    assert_eq!(front.reachable_reads.len(), 1);
}

#[test]
fn github_not_listed_is_an_actionable_link_with_full_or_empty_picker_in_both_flows() {
    for empty in [false, true] {
        for at in [
            Address::ConnectForge { forge: Some(Choice::Forge(1)) },
            welcome(Step::Forge { forge: Some(Choice::Forge(1)) }),
        ] {
            let (mut front, mut root) = Joint::github_fixture_with(fixtures::SEED, true, empty);
            go(&mut front, &mut root, at);
            let shown = text(Temper::tree(&draw(&root)));
            assert!(shown.contains("Not listed?"), "empty={empty}: {shown}");
            press(&mut front, &mut root, Role::Link, b"Choose its repositories on GitHub");
            assert_eq!(front.installs.len(), 1, "link sends one Install");
        }
    }
}

fn adopt_private_in_welcome(front: &mut Joint, root: &mut Root) {
    h::reopen(front, root, welcome(Step::Forge { forge: Some(Choice::Forge(1)) }), false);
    act(front, root, Role::Choice, b"acme/second", Act::Toggle);
    press(front, root, Role::Button, b"Adopt acme/second");
    assert!(front.engine_adopted(1, 8));
}

#[test]
fn github_welcome_adoption_advances_to_the_model_step() {
    let (mut front, mut root) = Joint::github_fixture(fixtures::SEED);
    adopt_private_in_welcome(&mut front, &mut root);
    let shown = text(Temper::tree(&draw(&root)));
    assert!(has(&root, Role::Heading { level: 1 }, b"Connect a model"), "{shown}");
}

#[test]
fn private_github_adoption_supplies_model_and_ready_facts_live_and_after_reload() {
    let (mut front, mut root) = Joint::github_fixture(fixtures::SEED);
    adopt_private_in_welcome(&mut front, &mut root);
    go(&mut front, &mut root, welcome(Step::Model { way: None }));
    let shown = text(Temper::tree(&draw(&root)));
    assert!(shown.contains("acme/second, in the new project second"), "{shown}");
    assert!(!has(&root, Role::Disclosure, b"Each step"), "{shown}");
    go(&mut front, &mut root, welcome(Step::Ready));
    assert!(has(&root, Role::Heading { level: 1 }, b"acme/second is ready"));
    assert!(has(&root, Role::Link, b"Open second"));
    h::reopen(&mut front, &mut root, welcome(Step::Model { way: None }), false);
    let shown = text(Temper::tree(&draw(&root)));
    assert!(shown.contains("acme/second, in second"), "{shown}");
    assert!(!shown.contains("the new project second"), "{shown}");
    assert!(!has(&root, Role::Disclosure, b"Each step"));
}

#[test]
fn github_picker_preserves_an_edited_project_name_when_switching_repositories() {
    let (mut front, mut root) = Joint::github_fixture(fixtures::SEED);
    act(
        &mut front,
        &mut root,
        Role::TextField,
        b"Into a new project, named",
        Act::Input { value: b"My work".as_slice().into() },
    );
    act(&mut front, &mut root, Role::Choice, b"acme/second", Act::Toggle);
    press(&mut front, &mut root, Role::Button, b"Adopt acme/second");
    assert!(front.engine_adopted(1, 8));
    let shown = text(Temper::tree(&draw(&root)));
    assert!(shown.contains("In the new project My work, on GitHub."), "{shown}");
    press(&mut front, &mut root, Role::Link, b"Open My work");
    assert!(has(&root, Role::Heading { level: 1 }, b"Chats"));
    assert!(has(&root, Role::Button, b"My work"));
}

#[test]
fn github_picker_requires_a_project_name_before_adopting() {
    let (mut front, mut root) = Joint::github_fixture(fixtures::SEED);
    act(&mut front, &mut root, Role::TextField, b"Into a new project, named", Act::Input { value: Box::new([]) });
    press(&mut front, &mut root, Role::Button, b"Adopt acme/shop");
    let shown = text(Temper::tree(&draw(&root)));
    assert!(shown.contains("Required."), "{shown}");
    assert!(!front.engine_adopted(1, 7));
    assert_eq!(front.applications_admitted(), 0);
}
