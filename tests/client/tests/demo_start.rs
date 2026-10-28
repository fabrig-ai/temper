//! The first-run choice through the client's scripted demo peer.
use jig_client_world::harness::{App, ClientId, Duty, Front};
use skein_lib::{Duration, Env, Queue, Time, Wall};
use skein_ui::Role;
use temper_client_domain::ChatsFilter;
use temper_client_domain::{self as d, Address, Place, Step, pages::Act};
use temper_client_protocol_wire::v1 as w;
use temper_client_world::{
    app::{Root, Temper},
    fixtures as f,
    front::{Joint, account_acts as accounts},
};

fn pump(front: &mut Joint, root: &mut Root, event: d::Event) -> usize {
    pump_holding_start(front, root, event, None)
}
fn pump_holding_start(
    front: &mut Joint,
    root: &mut Root,
    event: d::Event,
    mut held: Option<&mut Option<d::Request>>,
) -> usize {
    let limits = f::domain_limits();
    let mut now = Time::ZERO.saturating_add(Duration::from_secs(1));
    let mut out = Queue::uncapped();
    Temper::step(root, &Env { now, wall: Wall::EPOCH, limits }, event.into(), &mut out);
    let mut replaced = 0;
    for _ in 0..64 {
        Temper::take(root, &Env { now, wall: Wall::EPOCH, limits }, &mut out);
        while let Some(request) = out.pop() {
            if let Some(held) = &mut held
                && let d::Request::Client {
                    request:
                        jig_client::Request::Down {
                            down: jig_client::Down::Send { request: jig_client::Ask::Family { record, .. }, .. },
                        },
                } = &request
                && *temper_client_protocol_wire::decode_requesting(&record.bytes, &w::CEILINGS)
                    .expect("demo request")
                    .request()
                    == w::Request::StartDemo
            {
                assert!(held.replace(request).is_none(), "one held playground start");
                continue;
            }
            if let d::Request::Show { address, replace } = &request {
                replaced += usize::from(*replace);
                Temper::step(
                    root,
                    &Env { now, wall: Wall::EPOCH, limits },
                    d::Event::Went { place: Place::Address(address.clone()) }.into(),
                    &mut out,
                );
            }
            if let Duty::Wire(request) = Temper::duty(request) {
                front.wire(ClientId(0), now, Wall::EPOCH, request);
            }
        }
        now = now.saturating_add(Duration::from_millis(3));
        let callbacks = front.poll(now, Wall::EPOCH);
        if callbacks.is_empty() {
            return replaced;
        }
        for (_, event) in callbacks {
            front.received(ClientId(0), &event);
            Temper::step(root, &Env { now, wall: Wall::EPOCH, limits }, event, &mut out);
        }
    }
    panic!("demo story did not settle");
}
fn go(front: &mut Joint, root: &mut Root, address: Address) -> usize {
    pump(front, root, d::Event::Went { place: Place::Address(address) })
}
fn view(root: &Root) -> temper_client_view::View {
    let limits = f::domain_limits();
    let words = f::phrasebooks(&limits);
    let bounds = f::view_bounds(&limits, &words);
    let mut view = temper_client_view::View::new(&bounds, &limits, words, f::FORGE);
    temper_client_view::build(&mut view, root.person(), Wall::EPOCH, &mut Queue::with_capacity(bounds.tree.patches));
    view
}
fn has(root: &Root, role: Role, name: &[u8]) -> bool {
    view(root).tree().nodes().iter().any(|node| node.role == role && node.name.as_deref() == Some(name))
}
fn press(front: &mut Joint, root: &mut Root, name: &[u8]) {
    let mut view = view(root);
    let id = view
        .tree()
        .nodes()
        .iter()
        .find(|node| node.role == Role::Button && node.name.as_deref() == Some(name))
        .expect("button")
        .id;
    let action = temper_client_view::act(&mut view, id, skein_ui::Act::Press).expect("bound start");
    pump(front, root, d::Event::Act { action });
}
#[test]
fn not_started_welcome_and_strip_start_the_playground_in_its_project() {
    for strip in [false, true] {
        let (mut front, mut root) = accounts::demo_fixture(f::SEED, &w::DemoState::NotStarted);
        assert!(has(&root, Role::Heading { level: 1 }, b"Welcome to temper"));
        assert!(has(&root, Role::Button, b"Try the playground"));
        assert!(!has(&root, Role::Region, b"Playground"));
        assert!(front.demo_starts.is_empty());
        if strip {
            go(&mut front, &mut root, Address::Chats { filter: ChatsFilter::All, start: false });
            assert!(has(&root, Role::Region, b"Setup"));
        }
        press(&mut front, &mut root, b"Try the playground");
        assert_eq!(front.demo_starts.len(), 1);
        assert_eq!(root.person().pages().address(), &Address::Chats { filter: ChatsFilter::All, start: false });
        assert_eq!(root.person().pages().project(), Some(1));
        assert!(has(&root, Role::Region, b"Playground"));
        assert!(!has(&root, Role::Region, b"Setup"));
        assert!(has(&root, Role::Status, b"Ready"));
        pump(&mut front, &mut root, d::Event::Act { action: d::Action::Pages(Act::Playground) });
        assert_eq!(front.demo_starts.len(), 1);
    }
}
#[test]
fn failed_start_survives_navigation_and_try_again_uses_a_new_key() {
    let (mut front, mut root) = accounts::demo_fixture(f::SEED, &w::DemoState::NotStarted);
    front.demo_outcomes.push_back(w::Outcome::StartFailed(
        w::StartFailed::new(&w::CEILINGS, w::StartFailedParts { why: w::StartFailure::Account })
            .expect("account failure"),
    ));
    press(&mut front, &mut root, b"Try the playground");
    assert!(has(&root, Role::Status, b"Not started"));
    assert!(has(&root, Role::Alert, b"The playground did not start: the scripted model's account was refused."));
    go(&mut front, &mut root, Address::Accounts);
    go(&mut front, &mut root, Address::Chats { filter: ChatsFilter::All, start: false });
    assert!(has(&root, Role::Button, b"Try again"));
    press(&mut front, &mut root, b"Try again");
    assert_eq!(front.demo_starts.len(), 2);
    assert_ne!(front.demo_starts[0], front.demo_starts[1]);
    assert!(has(&root, Role::Status, b"Ready"));
    assert!(!has(&root, Role::Button, b"Try again"));
}
#[test]
fn running_read_sends_no_start_and_welcome_moves_once() {
    let (mut front, mut root) = accounts::demo_fixture(f::SEED, &w::DemoState::Running);
    assert_eq!(front.demo_starts.len(), 0);
    assert_eq!(root.person().pages().address(), &Address::Chats { filter: ChatsFilter::All, start: false });
    assert!(!has(&root, Role::Status, b"Starting"));
    let replaces = go(&mut front, &mut root, Address::Welcome { step: Step::Welcome });
    assert_eq!(replaces, 1);
    assert_eq!(front.demo_starts.len(), 0);
    go(&mut front, &mut root, Address::Welcome { step: Step::Ready });
    assert!(!has(&root, Role::Region, b"Playground"));
    assert!(!has(&root, Role::Region, b"Setup"));
}
#[test]
fn absent_demo_still_has_the_setup_strip() {
    let (mut front, mut root) = accounts::forge_fixture(f::SEED);
    go(&mut front, &mut root, Address::Chats { filter: ChatsFilter::All, start: false });
    assert!(has(&root, Role::Region, b"Setup"));
    assert!(!has(&root, Role::Region, b"Playground"));
}

fn add_model(front: &mut Joint, root: &mut Root) {
    go(
        front,
        root,
        Address::ConnectModel {
            way: Some(d::ModelWay { source: None, provider: accounts::PROVIDER + 1, kind: d::WayKind::Key }),
        },
    );
    pump(front, root, d::Event::AccountKey { value: skein_lib::Secret::new(b"test-private-key".as_slice().into()) });
    press(front, root, b"Add key");
    accounts::kept(front);
    accounts::settle(front, root);
}
fn connected(front: &mut Joint, root: &mut Root) {
    // The scripted engine half has durably ended before delivering the connection.
    front.demo = None;
    front.engine.send(temper_engine_domain::Event::Forge(Box::new(temper_engine_domain_forge::Event::Connection {
        forge: 1,
        state: temper_engine_domain_forge::Connection::Connected { id: 1, login: b"bot".as_slice().into() },
    })));
    accounts::settle(front, root);
}
fn demo_account(front: &mut Joint, root: &mut Root) -> bool {
    let address = root.person().pages().address().clone();
    go(front, root, Address::Accounts);
    let found = root
        .person()
        .pages()
        .accounts()
        .expect("accounts page")
        .deployment(root.person().client())
        .expect("deployment delivered by the real engine")
        .accounts
        .iter()
        .any(|line| {
            line.provider == Some(accounts::PROVIDER)
                && line.holder == jig_client::contract::OwnerAccountHolder::Deployment
        });
    go(front, root, address);
    found
}
#[test]
fn ready_names_an_owners_deployment_model_never_the_demos() {
    let (mut front, mut root) = accounts::demo_fixture(f::SEED, &w::DemoState::Running);
    go(&mut front, &mut root, Address::ConnectModel { way: None });
    press(&mut front, &mut root, b"Use deployment accounts");
    add_model(&mut front, &mut root);

    let domain = root.person();
    let me = d::pages::accounts::current_me(domain.client()).expect("owner identity");
    assert_eq!(me.deployment_role, Some(jig_client::contract::HeldRole::Owner));
    let document = d::pages::accounts::document(domain.client()).expect("accounts delivered");
    assert!(!document.accounts.iter().any(|row| d::forge::demo::usable_account(row, Some(accounts::PROVIDER))));
    let deployment = d::forge::demo::owner_deployment(domain.forge(), domain.client()).expect("owner deployment");
    assert!(deployment.accounts.iter().any(|account| {
        account.provider == Some(accounts::PROVIDER + 1)
            && account.holder == jig_client::contract::OwnerAccountHolder::Deployment
            && !account.cut
            && account.sources.iter().any(|source| source.usable)
    }));
    assert_eq!(d::pages::welcome::model_usable(domain), Some(true));
    assert!(!domain.demo().expect("forge half still running").1.model);

    go(&mut front, &mut root, Address::Accounts);
    let heading = "Other fake API key · held by the deployment".as_bytes();
    assert!(has(&root, Role::Heading { level: 3 }, heading));
    go(&mut front, &mut root, Address::Welcome { step: Step::Ready });
    assert_eq!(root.person().pages().address(), &Address::Welcome { step: Step::Ready });
    assert!(has(&root, Role::Paragraph, &[b"Model: ".as_slice(), heading].concat()));
    let ready = view(&root);
    for node in ready.tree().nodes() {
        if let Some(name) = &node.name {
            assert!(!name.windows(b"scripted".len()).any(|part| part.eq_ignore_ascii_case(b"scripted")));
        }
    }
}
#[test]
fn a_model_connected_removes_the_scripted_account_and_leaves_the_playground_forge_banner() {
    let (mut front, mut root) = accounts::demo_fixture(f::SEED, &w::DemoState::Running);
    assert!(demo_account(&mut front, &mut root));
    assert!(root.person().demo().expect("running playground").1.model);
    add_model(&mut front, &mut root);
    assert!(!demo_account(&mut front, &mut root));
    assert_eq!(front.demo_removals.len(), 1);
    go(&mut front, &mut root, Address::Accounts);
    assert!(!has(&root, Role::Heading { level: 2 }, b"Scripted"));
    let halves = root.person().demo().expect("running playground").1;
    assert_eq!(halves, d::forge::demo::Halves { model: false, forge: true });
    go(&mut front, &mut root, Address::Chats { filter: ChatsFilter::All, start: false });
    assert!(has(&root, Role::Region, b"Playground"));
    assert!(has(&root, Role::Paragraph, b"This is the playground: a playground forge."));
    assert!(!has(&root, Role::Button, b"Connect a model"));
    assert!(!has(&root, Role::Status, b"Ready"), "no demo start");
    go(&mut front, &mut root, Address::Forges);
    assert!(d::forge::listed::playground(root.person()).is_some());
    assert!(has(&root, Role::Heading { level: 2 }, b"The playground"));
    connected(&mut front, &mut root);
    assert_eq!(front.demo_removals.len(), 1, "forge end never repeats removal");
    assert!(root.person().forge().demo().is_none());
}
#[test]
fn a_forge_connected_clears_the_demo_everywhere_and_welcome_moves_when_a_model_is_usable() {
    let (mut front, mut root) = accounts::demo_fixture(f::SEED, &w::DemoState::Running);
    assert!(demo_account(&mut front, &mut root));
    go(&mut front, &mut root, Address::Forges);
    assert!(d::forge::listed::playground(root.person()).is_some());
    assert!(has(&root, Role::Heading { level: 2 }, b"The playground"));
    let reads = front.demo_reads;
    connected(&mut front, &mut root);
    assert_eq!(front.demo_reads, reads + 1);
    assert_eq!(front.demo_removals.len(), 0);
    assert!(root.person().forge().demo().is_none());
    assert!(d::forge::listed::playground(root.person()).is_none());
    assert!(!has(&root, Role::Region, b"Playground"));
    assert!(!has(&root, Role::Heading { level: 2 }, b"The playground"));
    go(&mut front, &mut root, Address::Chats { filter: ChatsFilter::All, start: false });
    assert!(!has(&root, Role::Region, b"Playground"));
    assert!(!has(&root, Role::Button, b"Try the playground"));
    assert!(!has(&root, Role::Status, b"Ready"));
    // The fixture also has a usable configured model; connect the other fake
    // account before reopening Welcome, exercising both sources of usable facts.
    add_model(&mut front, &mut root);
    assert_eq!(go(&mut front, &mut root, Address::Welcome { step: Step::Welcome }), 1);
    assert_eq!(root.person().pages().address(), &Address::Chats { filter: ChatsFilter::All, start: false });
    assert_eq!(front.demo_starts.len(), 0);
    assert_eq!(front.demo_removals.len(), 0);
}

fn submit_chat(front: &mut Joint, root: &mut Root) -> bool {
    let mut view = view(root);
    let field = view
        .tree()
        .nodes()
        .iter()
        .find(|node| node.role == Role::TextArea && node.name.as_deref() == Some(b"Start a chat"))
        .expect("chat composer")
        .id;
    let Some(action) = temper_client_view::act(&mut view, field, skein_ui::Act::Submit) else { return false };
    pump(front, root, d::Event::Act { action });
    true
}

fn delegate_to_repository(front: &mut Joint, root: &Root) -> temper_engine_domain::CallAnswer {
    use jig_domain_tasks as tasks;
    use temper_engine_domain as engine;
    let Address::Chat { task } = *root.person().pages().address() else { panic!("chat opened") };
    let attempt = front.hosted_attempt(task).expect("chat assigned to the scripted worker");
    let mut delegate = temper_engine_domain_world::fixtures::report_delegate(b"Change the title", Box::new([]));
    delegate.authority.tools = tasks::Tools(0);
    delegate.spec.parameters = Box::new([tasks::Parameter::Resource {
        name: 1,
        connector: f::FORGE,
        resource: temper_engine_domain_forge::tag(1, 7),
    }]);
    front.engine.call(
        skein_lib::Token::new(7),
        task,
        attempt,
        99,
        engine::Call { completion: 1, position: 1, tool: engine::Tool::Delegate { batch: Box::new([delegate]) } },
    );
    // The parent occupies the fixture's worker; an admitted child can wait for
    // placement. Drain immediate work without requiring that deadline to clear.
    front.engine.pump();
    front.engine.answered(99).expect("real delegation answer").clone()
}

#[test]
fn enter_waits_for_adoption_then_the_chat_can_delegate() {
    let (mut front, mut root) = accounts::demo_adoption_fixture(f::SEED);
    let mut held = None;
    pump_holding_start(
        &mut front,
        &mut root,
        d::Event::Act { action: d::Action::Pages(Act::Playground) },
        Some(&mut held),
    );
    assert!(held.is_some(), "startup's adoption has not completed");
    assert!(root.person().forge().demo_starting());
    assert!(root.person().demo().expect("playground").1.model, "model is usable before adoption");
    assert!(!front.engine_adopted(1, 7));
    pump(
        &mut front,
        &mut root,
        d::Event::Act { action: d::Action::Pages(Act::ChatWords { words: b"Change the title".as_slice().into() }) },
    );
    if submit_chat(&mut front, &mut root) {
        assert_eq!(f::chats(&front.engine), 1, "Enter started a chat while adoption was held");
        let answer = delegate_to_repository(&mut front, &root);
        assert!(
            matches!(&answer, temper_engine_domain::CallAnswer::DelegationRefused(problem)
            if problem.why == jig_domain_tasks::Refusal::Holds),
            "missing repository: {answer:?}"
        );
        panic!("Enter started the chat before adoption; its delegation was refused: {answer:?}");
    }
    assert_eq!(f::chats(&front.engine), 0, "Enter waits without creating a chat");
    front.engine.send(temper_engine_domain::Event::Forge(Box::new(temper_engine_domain_forge::Event::Connection {
        forge: 1,
        state: temper_engine_domain_forge::Connection::Connected { id: 1, login: b"bot".as_slice().into() },
    })));
    f::adopt_change_repository(&mut front);
    assert!(front.engine_adopted(1, 7), "the held adoption completes");
    let Duty::Wire(request) = Temper::duty(held.take().expect("held start")) else { panic!("start wire") };
    front.wire(ClientId(0), Time::from_nanos(1_000_000_000), Wall::EPOCH, request);
    accounts::settle(&mut front, &mut root);
    assert!(!root.person().forge().demo_starting());
    assert!(has(&root, Role::Status, b"Ready"));
    assert!(submit_chat(&mut front, &mut root), "Enter now starts the kept draft");
    assert_eq!(f::chats(&front.engine), 1);
    assert!(matches!(delegate_to_repository(&mut front, &root), temper_engine_domain::CallAnswer::Delegated(_)));
}
