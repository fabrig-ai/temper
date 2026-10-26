//! Owned Forgejo's welcome facts, read through the existing Joint.
use jig_client_world::harness::{App, ClientId, Front};
use skein_lib::Queue;
use temper_client_domain::{Address, Choice, Step};
use temper_client_world::{
    app::{Root, Temper, View},
    fixtures,
    front::{Joint, account_acts as h, first_run_projects},
};
use temper_engine_domain as engine;
use temper_engine_domain_forge as forge;
use temper_engine_domain_world::{direct::Driver, store::Store, walking};

fn owned_fixture(runner: Option<&[u8]>) -> (Joint, Root) {
    let seed = fixtures::SEED;
    let limits = fixtures::domain_limits();
    let mut bounds = temper_engine_domain_world::fixtures::forge_limits();
    bounds.people.projects = 3;
    bounds.people.holdings = 3;
    bounds.people.requests = 5;
    let configs = std::array::from_fn(|_| {
        let mut config = walking::config(seed);
        first_run_projects(&mut config, &mut bounds);
        config.under_box = true;
        config.core.connectors = Box::new([fixtures::FORGE]);
        config.forges = Box::new([forge::Configured {
            number: 1,
            kind: forge::Kind::Forgejo,
            mode: forge::Mode::Owned,
            admin_connect: false,
            bot: Box::new([]),
            read_only: false,
            asked: true,
            git: Box::new([]),
            web: Some(b"http://forge.test:3000".as_slice().into()),
            owned: Some(forge::Owned {
                forgejo: b"71.2.3".as_slice().into(),
                runner: runner.map(Into::into),
                password: b"fixture/person_password".as_slice().into(),
            }),
        }]);
        config
    });
    let mut front = Joint::new(seed, limits);
    front.engine = Driver::with_repositories(Store::new(), configs, &bounds, &[(b"work", b"hello", 42)]);
    front.engine.send(engine::Event::Forge(Box::new(forge::Event::Connection {
        forge: 1,
        state: forge::Connection::NotSetUp,
    })));
    front.engine.settle();
    front.bind(ClientId(0), fixtures::PARTY);
    let mut root = Root::Person(Box::new(temper_client_domain::unlinked(&limits, seed)));
    h::event(
        &mut front,
        &mut root,
        Temper::start(Address::Welcome { step: Step::Forge { forge: Some(Choice::Forge(1)) } }, Vec::new(), 0),
    );
    (front, root)
}
fn draw(root: &Root) -> View {
    let (_, mut view) = Temper::new(fixtures::SEED);
    let mut patches = Queue::with_capacity(Temper::view_limits(&view).patches);
    Temper::build(&mut view, root, &mut patches);
    view
}
fn text(root: &Root) -> String {
    let view = draw(root);
    Temper::tree(&view)
        .nodes()
        .iter()
        .flat_map(|n| n.text.iter())
        .map(|r| String::from_utf8_lossy(&r.text))
        .collect::<Vec<_>>()
        .join("")
}
#[test]
fn owned_details_names_its_configured_forgejo_version() {
    let (_, root) = owned_fixture(Some(b"64.5.6"));
    let view = draw(&root);
    let shown = Temper::tree(&view)
        .nodes()
        .iter()
        .flat_map(|n| n.text.iter())
        .map(|r| String::from_utf8_lossy(&r.text))
        .collect::<Vec<_>>()
        .join("");
    assert!(shown.contains("downloads Forgejo 71.2.3 and its runner"), "{shown}");
}
fn act(front: &mut Joint, root: &mut Root, role: skein_ui::Role, name: &[u8], action: skein_ui::Act) {
    let mut view = draw(root);
    let node = Temper::tree(&view)
        .nodes()
        .iter()
        .find(|n| n.role == role && n.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("missing {}", String::from_utf8_lossy(name)))
        .id;
    let event = Temper::act(&mut view, node, action).expect("offered act");
    h::event(front, root, event);
}
#[test]
fn owned_setup_progress_names_both_pins_on_welcome_and_standalone() {
    let (mut front, mut root) = owned_fixture(Some(b"64.5.6"));
    act(&mut front, &mut root, skein_ui::Role::Button, b"Set up a Forgejo here", skein_ui::Act::Press);
    let (forge, request) = front.set_ups()[0];
    front.engine.send(engine::Event::Forge(Box::new(forge::Event::Connection {
        forge,
        state: forge::Connection::SettingUp { step: Some(forge::SetUpStep::Fetch) },
    })));
    front.engine.send(engine::Event::Forge(Box::new(forge::Event::SetUpAnswered {
        forge,
        request,
        answer: forge::SetUpAnswer::SettingUp,
    })));
    front.engine.settle();
    h::settle(&mut front, &mut root);
    let shown = text(&root);
    assert!(shown.contains("Fetching Forgejo 71.2.3 and its runner 64.5.6"), "{shown}");
    assert!(
        !Temper::tree(&draw(&root)).nodes().iter().any(|n| n.name.as_deref() == Some("Open Forgejo ↗".as_bytes())),
        "not offered before adoption"
    );
    for step in [forge::SetUpStep::Prepare, forge::SetUpStep::Run] {
        front.engine.send(engine::Event::Forge(Box::new(forge::Event::Connection {
            forge,
            state: forge::Connection::SettingUp { step: Some(step) },
        })));
        front.engine.settle();
        h::settle(&mut front, &mut root);
        let shown = text(&root);
        assert!(shown.contains("Fetched Forgejo 71.2.3 and its runner 64.5.6"), "{shown}");
    }
    h::event(&mut front, &mut root, Temper::went(Address::ConnectForge { forge: Some(Choice::Forge(1)) }));
    let shown = text(&root);
    assert!(shown.contains("Fetched Forgejo 71.2.3 and its runner 64.5.6"), "{shown}");
    assert!(shown.contains("Preparing: its database, temper’s admin, your account"), "{shown}");
}
#[test]
fn owned_missing_runner_pin_keeps_the_progress_unversioned() {
    let (mut front, mut root) = owned_fixture(None);
    assert!(text(&root).contains("downloads Forgejo 71.2.3 and its runner"));
    act(&mut front, &mut root, skein_ui::Role::Button, b"Set up a Forgejo here", skein_ui::Act::Press);
    let (forge, request) = front.set_ups()[0];
    front.engine.send(engine::Event::Forge(Box::new(forge::Event::Connection {
        forge,
        state: forge::Connection::SettingUp { step: Some(forge::SetUpStep::Fetch) },
    })));
    front.engine.send(engine::Event::Forge(Box::new(forge::Event::SetUpAnswered {
        forge,
        request,
        answer: forge::SetUpAnswer::SettingUp,
    })));
    front.engine.settle();
    h::settle(&mut front, &mut root);
    let shown = text(&root);
    assert!(shown.contains("Fetching Forgejo and its runner"), "{shown}");
    assert!(!shown.contains("Fetching Forgejo 71.2.3"), "{shown}");
}

#[test]
fn owned_details_without_pins_use_the_same_fallback_in_both_flows() {
    let (mut front, mut root) = h::forge_fixture(fixtures::SEED);
    for at in [
        Address::Welcome { step: Step::Forge { forge: Some(Choice::Forge(1)) } },
        Address::ConnectForge { forge: Some(Choice::Forge(1)) },
    ] {
        h::event(&mut front, &mut root, Temper::went(at));
        let shown = text(&root);
        assert!(shown.contains("downloads Forgejo and its runner"), "{shown}");
        assert!(!shown.contains("71.2.3"), "{shown}");
    }
}

#[test]
#[ignore = "backlog: the Model step drops the requested target until the first box status (a delivery order)"]
fn accepted_owned_setup_keeps_its_target_on_model_before_box_status_arrives() {
    let (mut front, mut root) = h::forge_fixture(fixtures::SEED);
    h::event(
        &mut front,
        &mut root,
        Temper::went(Address::Welcome { step: Step::Forge { forge: Some(Choice::Forge(1)) } }),
    );
    act(&mut front, &mut root, skein_ui::Role::Button, b"Set up a Forgejo here", skein_ui::Act::Press);
    let (forge, request) = front.set_ups()[0];
    // Admission precedes the box's first status: the request still supplies the target.
    front.engine.send(engine::Event::Forge(Box::new(forge::Event::SetUpAnswered {
        forge,
        request,
        answer: forge::SetUpAnswer::SettingUp,
    })));
    front.engine.settle();
    h::settle(&mut front, &mut root);
    let shown = text(&root);
    assert!(
        Temper::tree(&draw(&root)).nodes().iter().any(|n| {
            n.role == skein_ui::Role::Heading { level: 1 } && n.name.as_deref() == Some(b"Connect a model")
        }),
        "{shown}"
    );
    assert!(shown.contains("work/hello, into the new project hello"), "{shown}");
}
