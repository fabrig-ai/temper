use jig_client_world::harness::{App, Duty, Front};
use skein_lib::{Env, Queue, Secret, Wall};
use skein_ui::Role;
use temper_client_domain::ChatsFilter;
use temper_client_domain::{
    self as d, Address, Place, Step,
    pages::{Act, connect_model::Act as A},
};
use temper_client_world::{app::Temper, fixtures, front::account_acts as accounts};

#[test]
fn a_first_run_welcome_without_a_demo() {
    let (mut front, mut root) = accounts::welcome_fixture(fixtures::SEED);
    let person = root.person();
    assert!(person.demo_answered());
    assert!(person.demo().is_none());
    assert!(person.pages().welcome().expect("welcome opened").decided);
    let document = d::pages::accounts::document(person.client()).expect("accounts delivered");
    assert!(!document.accounts.iter().any(|line| d::forge::demo::usable_account(line, None)));
    let limits = fixtures::domain_limits();
    let words = fixtures::phrasebooks(&limits);
    let bounds = fixtures::view_bounds(&limits, &words);
    let mut view = temper_client_view::View::new(&bounds, &limits, words, fixtures::FORGE);
    temper_client_view::build(&mut view, person, fixtures::NOW, &mut Queue::with_capacity(bounds.tree.patches));
    assert!(
        view.tree()
            .nodes()
            .iter()
            .any(|node| node.role == Role::Region && node.name.as_deref() == Some(b"Set up temper"))
    );
    assert!(!view.tree().nodes().iter().any(|node| node.name.as_deref() == Some(b"Try the playground")));

    accounts::event(
        &mut front,
        &mut root,
        d::Event::Went { place: Place::Address(Address::Welcome { step: Step::Model { way: None } }) }.into(),
    );
    accounts::event(
        &mut front,
        &mut root,
        d::Event::Went {
            place: Place::Address(Address::Welcome {
                step: Step::Model {
                    way: Some(d::ModelWay { source: None, provider: accounts::PROVIDER, kind: d::WayKind::Key }),
                },
            }),
        }
        .into(),
    );
    accounts::event(
        &mut front,
        &mut root,
        d::Event::AccountKey { value: Secret::new(b"test-private-key".as_slice().into()) }.into(),
    );
    accounts::event(
        &mut front,
        &mut root,
        d::Event::Act { action: d::Action::Pages(Act::ConnectModel(A::Submit { name: [41; 16] })) }.into(),
    );
    accounts::kept(&mut front);
    front.engine.send(temper_engine_domain::Event::Forge(Box::new(temper_engine_domain_forge::Event::Connection {
        forge: 1,
        state: temper_engine_domain_forge::Connection::Connected { id: 1, login: b"bot".as_slice().into() },
    })));
    accounts::settle(&mut front, &mut root);
    assert!(
        d::pages::accounts::document(root.person().client())
            .expect("current account facts")
            .accounts
            .iter()
            .any(|line| d::forge::demo::usable_account(line, None)),
        "key became usable"
    );
    let mut now = skein_lib::Time::ZERO.saturating_add(skein_lib::Duration::from_secs(1));
    let mut out = Queue::uncapped();
    Temper::step(
        &mut root,
        &Env { now, wall: Wall::EPOCH, limits },
        d::Event::Went { place: Place::Address(Address::Welcome { step: Step::Welcome }) }.into(),
        &mut out,
    );
    // Reopening the welcome decides from the session's current forges rows.
    // The root-only pump discards shell navigation, so retain it here through its actual callbacks.
    let mut moved = false;
    for _ in 0..32 {
        while let Some(request) = out.pop() {
            moved |= request
                == (d::Request::Show {
                    address: Address::Chats { filter: ChatsFilter::All, start: false },
                    replace: true,
                });
            if let Duty::Wire(request) = Temper::duty(request) {
                front.wire(jig_client_world::harness::ClientId(0), now, Wall::EPOCH, request);
            }
        }
        now = now.saturating_add(skein_lib::Duration::from_millis(3));
        let callbacks = front.poll(now, Wall::EPOCH);
        if callbacks.is_empty() {
            break;
        }
        for (_, event) in callbacks {
            Temper::step(&mut root, &Env { now, wall: Wall::EPOCH, limits }, event, &mut out);
        }
    }
    assert!(
        moved,
        "the second open replaces welcome with chats: {:?}, role {:?}, decision {:?}",
        root.person().pages().welcome(),
        d::pages::accounts::current_me(root.person().client()).expect("current account facts").deployment_role,
        d::pages::welcome::decision(root.person())
    );
}

#[test]
fn a_configured_model_hides_the_banner_and_the_strips_model_half() {
    let (mut front, mut root) = accounts::welcome_fixture(fixtures::SEED);
    let person = root.person();
    assert!(person.demo_answered());
    assert!(person.demo().is_none());
    let document = d::pages::accounts::document(person.client()).expect("accounts delivered");
    assert!(document.usable, "the configured model is usable");
    assert!(!document.accounts.iter().any(|line| d::forge::demo::usable_account(line, None)));
    assert_eq!(d::pages::welcome::decision(person), Some(false), "the forge is not connected");
    accounts::event(
        &mut front,
        &mut root,
        d::Event::Went { place: Place::Address(Address::Chats { filter: ChatsFilter::All, start: false }) }.into(),
    );
    let setup = d::forge::listed::setup(root.person()).expect("forge setup remains");
    assert!(setup.forge);
    assert!(!setup.model);
    let limits = fixtures::domain_limits();
    let words = fixtures::phrasebooks(&limits);
    let bounds = fixtures::view_bounds(&limits, &words);
    let mut view = temper_client_view::View::new(&bounds, &limits, words, fixtures::FORGE);
    temper_client_view::build(&mut view, root.person(), fixtures::NOW, &mut Queue::with_capacity(bounds.tree.patches));
    let nodes = view.tree().nodes();
    assert!(!nodes.iter().any(|node| node.name.as_deref() == Some(b"Agents need an LLM account to run.")));
    assert!(!nodes.iter().any(|node| node.name.as_deref() == Some(b"temper has no model yet.")));
    assert!(nodes.iter().any(|node| node.name.as_deref() == Some(b"temper has no forge yet.")));
    front.engine.send(temper_engine_domain::Event::Forge(Box::new(temper_engine_domain_forge::Event::Connection {
        forge: 1,
        state: temper_engine_domain_forge::Connection::Connected { id: 1, login: b"bot".as_slice().into() },
    })));
    accounts::settle(&mut front, &mut root);
    assert_eq!(d::pages::welcome::decision(root.person()), Some(true));
    assert!(d::forge::listed::setup(root.person()).is_none());
}

#[test]
fn later_after_a_real_forge_connects_retires_the_unstarted_playground_live() {
    use temper_engine_domain_forge as forge;
    for github in [false, true] {
        let (mut front, mut root) = if github {
            accounts::demo_github_fixture(fixtures::SEED)
        } else {
            accounts::demo_fixture(fixtures::SEED, &temper_client_protocol_wire::v1::DemoState::NotStarted)
        };
        let draw = |root: &temper_client_world::app::Root| {
            let (_, mut view) = Temper::new(fixtures::SEED);
            let mut patches = Queue::with_capacity(Temper::patch_limit(&view));
            Temper::build(&mut view, root, &mut patches);
            view
        };
        accounts::event(&mut front, &mut root, Temper::went(fixtures::chats_page()));
        let before = draw(&root);
        assert!(Temper::tree(&before).nodes().iter().any(|n| n.name.as_deref() == Some(b"Try the playground")));
        accounts::event(&mut front, &mut root, Temper::went(Address::Welcome { step: Step::Forge { forge: None } }));
        if github {
            front.engine.send(temper_engine_domain::Event::Forge(Box::new(forge::Event::AppRecorded {
                forge: 1,
                app: forge::AppWay::Pasted,
            })));
        }
        let mut demo = front.demo.take().expect("offered playground").into_parts();
        demo.state = temper_client_protocol_wire::v1::DemoState::Ended;
        front.demo = Some(
            temper_client_protocol_wire::v1::DemoAnswer::new(&temper_client_protocol_wire::v1::CEILINGS, demo)
                .expect("retired demo"),
        );
        front.engine.send(temper_engine_domain::Event::Forge(Box::new(forge::Event::Connection {
            forge: 1,
            state: forge::Connection::Connected { id: 1, login: b"bot".as_slice().into() },
        })));
        front.engine.settle();
        accounts::settle(&mut front, &mut root);
        let mut welcome = draw(&root);
        let later = Temper::tree(&welcome)
            .nodes()
            .iter()
            .find(|n| n.name.as_deref() == Some(b"Later"))
            .expect("Later control")
            .id;
        let event = Temper::act(&mut welcome, later, skein_ui::Act::Press).expect("bound Later");
        accounts::event(&mut front, &mut root, event);
        assert_eq!(root.person().pages().address(), &fixtures::chats_page(), "Later opens Chats in this live root");
        let after = draw(&root);
        let tree = Temper::tree(&after);
        assert!(tree.nodes().iter().any(|n| n.name.as_deref() == Some(b"No project yet")));
        for absent in [b"Playground".as_slice(), b"Project 1", b"Try the playground"] {
            assert!(
                !tree.nodes().iter().any(|n| n.name.as_deref() == Some(absent)),
                "retired navigation: {}",
                String::from_utf8_lossy(absent)
            );
        }
        assert_eq!(root.person().pages().project(), None);
        if github {
            let d::forge::forges::State::Rows(rows) = root.person().forges() else { panic!("forges") };
            assert!(rows[0].installations.is_empty(), "the pasted App is connected before installation");
        }
    }
}
