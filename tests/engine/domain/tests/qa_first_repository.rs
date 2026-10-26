//! Setup admission and repository identity at the real engine root.
use skein_lib::{ReplyTo, Token};
use temper_engine_domain as engine;
use temper_engine_domain_forge as forge;
use temper_engine_domain_world::{direct::Driver, fixtures, store::Store, walking};

fn driver(under_box: bool) -> Driver {
    driver_with_name_bytes(under_box, fixtures::forge_limits().forge.name_bytes)
}
fn driver_with_name_bytes(under_box: bool, name_bytes: u32) -> Driver {
    let mut limits = fixtures::forge_limits();
    limits.forge.name_bytes = name_bytes;
    let configs = std::array::from_fn(|_| {
        let mut config = walking::config(701);
        config.under_box = under_box;
        config.forges = Box::new([forge::Configured {
            admin_connect: false,
            bot: Box::new([]),
            owned: None,
            git: Box::new([]),
            web: None,
            read_only: false,
            number: 1,
            kind: forge::Kind::Forgejo,
            mode: forge::Mode::Owned,
            asked: true,
        }]);
        config
    });
    let mut driver = Driver::with_repositories(Store::new(), configs, &limits, &[(b"org", b"repo", 2)]);
    driver.hello(Token::new(7));
    driver.settle();
    driver.sign_in();
    driver.settle();
    event(&mut driver, forge::Event::Connection { forge: 1, state: forge::Connection::NotSetUp });
    driver
}
fn event(driver: &mut Driver, event: forge::Event) {
    driver.send(engine::Event::Forge(Box::new(event)));
    driver.settle();
}
fn setup(driver: &mut Driver, serial: u8, org: &[u8], repository: &[u8]) -> Option<engine::ApplicationAnswer> {
    let reply = ReplyTo::new(Token::new(u64::from(serial)));
    driver.send(engine::Event::Party(engine::PartyInput::Application {
        scope: jig_domain_people::ApplicationScope::Deployment,
        reply_to: ReplyTo::new(Token::new(u64::from(serial))),
        sign_in: driver.session(),
        key: [serial; 16],
        digest: [serial; 32],
        carried: None,
        request: engine::Application::Forge(forge::Ask::SetUp {
            playground: false,
            forge: 1,
            org: org.into(),
            repository: repository.into(),
            project: forge::SetUpProject::Existing(1),
        }),
    }));
    driver.settle();
    driver.delivered.iter().rev().find_map(|value| {
        if let engine::Delivery::Application { to, outcome } = value
            && *to == reply
        {
            return outcome.clone();
        }
        None
    })
}
#[test]
fn setup_refused_without_the_box() {
    // domain/forge.md, 4.1: without the box, setup is refused before routing.
    let mut driver = driver(false);
    let answer = setup(&mut driver, 51, b"work", b"hello");
    assert_eq!(
        answer,
        Some(engine::ApplicationAnswer::SetUp(forge::SetUpAnswer::Refused(forge::SetUpRefusal::NotUnderTheBox)))
    );
}

#[test]
fn invalid_names_are_refused_before_any_box_request() {
    // client/README.md, 4.6; protocol/setup.md, 6.7: both grammars checked at admission.
    let cases: &[(&[u8], &[u8])] = &[
        (b"", b"hello"),
        (b"work", b""),
        (b"bad/org", b"hello"),
        (b"bad org", b"hello"),
        (b"work", b"."),
        (b"work", b".."),
        (b"work", b"bad/repo"),
        (b"work", b"bad repo"),
        (b"work", b"hello\n"),
        (b"work", b"hello\0"),
        (b"w\xc3\xb6rk", b"hello"),
    ];
    for (org, name) in cases {
        let mut driver = driver(true);
        let answer = setup(&mut driver, 52, org, name);
        assert_eq!(
            answer,
            Some(engine::ApplicationAnswer::SetUp(forge::SetUpAnswer::Refused(forge::SetUpRefusal::Invalid))),
            "org {org:?}, repository {name:?}"
        );
        assert!(
            !driver.immediate.iter().any(|request| matches!(request, engine::Request::SetUp { .. })),
            "invalid names must not reach the box"
        );
    }
}

#[test]
fn names_one_byte_past_each_forge_limit_are_refused() {
    // client/README.md, 4.6: 40-byte organization and 100-byte repository maxima.
    for (org, name) in [(vec![b'o'; 41], b"hello".to_vec()), (b"work".to_vec(), vec![b'r'; 101])] {
        let mut driver = driver(true);
        assert_eq!(
            setup(&mut driver, 53, &org, &name),
            Some(engine::ApplicationAnswer::SetUp(forge::SetUpAnswer::Refused(forge::SetUpRefusal::Invalid)))
        );
    }
}

#[test]
fn exact_grammar_limits_and_repository_punctuation_are_admitted() {
    // client/README.md, 4.6: accept the other side of both name boundaries.
    for (org, name) in [(vec![b'o'; 40], vec![b'r'; 100]), (b"work".to_vec(), b".hello_World-9.".to_vec())] {
        let mut driver = driver(true);
        assert_eq!(setup(&mut driver, 54, &org, &name), None, "an admitted setup waits for the box's answer");
        assert!(driver.immediate.iter().any(|request| matches!(request, engine::Request::SetUp { forge: 1, org: actual_org, repository: actual_name, .. } if actual_org.as_ref() == org && actual_name.as_ref() == name)), "admitted names must reach the box whole");
    }
}

#[test]
fn a_second_setup_is_busy_and_cannot_replace_the_first_intent() {
    // domain/forge.md, 4.1; protocol/setup.md, 6.7: one setup at a time, intent only replaced on admission.
    let mut driver = driver(true);
    assert_eq!(setup(&mut driver, 55, b"work", b"hello"), None);
    event(&mut driver, forge::Event::Connection { forge: 1, state: forge::Connection::SettingUp { step: None } });
    let original = forges(&mut driver).rows[0].first.clone();
    driver.immediate.clear();
    assert_eq!(
        setup(&mut driver, 56, b"elsewhere", b"replacement"),
        Some(engine::ApplicationAnswer::SetUp(forge::SetUpAnswer::Refused(forge::SetUpRefusal::Busy)))
    );
    assert!(
        !driver.immediate.iter().any(|request| matches!(request, engine::Request::SetUp { .. })),
        "busy must not start or replace a setup"
    );
    assert_eq!(forges(&mut driver).rows[0].first, original, "busy must preserve the admitted intent");
}

#[test]
fn connected_forge_does_not_offer_setup() {
    // domain/forge.md, 4.1: a forge already connected does not offer SetUp.
    let mut driver = driver(true);
    event(
        &mut driver,
        forge::Event::Connection {
            forge: 1,
            state: forge::Connection::Connected { id: 1, login: b"temper".as_slice().into() },
        },
    );
    assert_eq!(
        setup(&mut driver, 57, b"work", b"hello"),
        Some(engine::ApplicationAnswer::SetUp(forge::SetUpAnswer::Refused(forge::SetUpRefusal::NotOffered)))
    );
}

fn forges(driver: &mut Driver) -> forge::Forges {
    driver.serial += 1;
    let watcher = Token::new(driver.serial + 900);
    driver.send(engine::Event::Party(engine::PartyInput::Watch {
        watcher,
        sign_in: driver.session(),
        subject: jig_domain_core::documents::Watched::Application {
            scope: jig_domain_people::ApplicationScope::Deployment,
            connector: walking::config(701).core.connectors[0],
            subject: Token::new(0),
        },
        window: jig_domain_core::documents::Window { rows: 4, after: None },
    }));
    driver.settle();
    driver
        .delivered
        .iter()
        .rev()
        .find_map(|delivery| {
            if let engine::Delivery::Forges { watcher: actual, forges } = delivery
                && *actual == watcher
            {
                return Some(forges.clone());
            }
            None
        })
        .unwrap_or_else(|| panic!("owner forges missing: {:?}", driver.delivered))
}

#[test]
fn first_repository_row_is_volatile_across_restart() {
    // protocol/setup.md, 6.7: even made(id) is volatile, never recovered as an adoption.
    let mut driver = driver(true);
    let _ = setup(&mut driver, 58, b"work", b"hello");
    event(&mut driver, forge::Event::FirstMade { forge: 1, id: 2 });
    let rows = forges(&mut driver);
    assert!(rows.rows[0].first.is_some());
    let store = driver.store.fork();
    let mut restarted = Driver::configured(
        store,
        std::array::from_fn(|_| {
            let mut config = walking::config(701);
            config.under_box = true;
            config.forges = Box::new([forge::Configured {
                admin_connect: false,
                bot: Box::new([]),
                owned: None,
                git: Box::new([]),
                web: None,
                read_only: false,
                number: 1,
                kind: forge::Kind::Forgejo,
                mode: forge::Mode::Owned,
                asked: true,
            }]);
            config
        }),
        &fixtures::forge_limits(),
    );
    restarted.hello(Token::new(7));
    restarted.settle();
    restarted.sign_in();
    restarted.settle();
    assert!(forges(&mut restarted).rows[0].first.is_none(), "restart must forget the first repository's made row");
    assert!(
        !restarted.immediate.iter().any(|request| matches!(request, engine::Request::SetUp { .. })),
        "a restart must not replay volatile intent"
    );
}

#[test]
fn every_create_failure_names_its_step_and_allows_a_fresh_setup() {
    // protocol/setup.md, 6.7: admin/org/repository/file failures are named and offered again.
    for step in
        [forge::CreateStep::Admin, forge::CreateStep::Org, forge::CreateStep::Repository, forge::CreateStep::File]
    {
        let mut driver = driver(true);
        let _ = setup(&mut driver, 60, b"work", b"hello");
        event(&mut driver, forge::Event::FirstFailed { forge: 1, step });
        event(
            &mut driver,
            forge::Event::Connection {
                forge: 1,
                state: forge::Connection::SetUpFailed { why: b"Create stopped".as_slice().into() },
            },
        );
        let rows = forges(&mut driver);
        let first = rows.rows[0].first.as_ref().expect("failed intent remains visible");
        assert_eq!(first.state, forge::FirstState::Failed { step }, "failure must retain its exact step");
        driver.immediate.clear();
        assert_eq!(
            setup(&mut driver, 61, b"work", b"hello"),
            None,
            "a failed setup must be admitted again at {step:?}"
        );
        assert!(
            driver.immediate.iter().any(|request| matches!(request, engine::Request::SetUp { forge: 1, .. })),
            "retry must reach the box at {step:?}"
        );
    }
}

#[test]
fn replay_of_an_answered_setup_does_not_start_the_box_twice() {
    // domain/forge.md, 4.1; client/README.md, 4.6: SetUp is keyed like adoption.
    let mut driver = driver(true);
    let _ = setup(&mut driver, 62, b"work", b"hello");
    let request = driver
        .immediate
        .iter()
        .find_map(|request| if let engine::Request::SetUp { request, .. } = request { Some(*request) } else { None })
        .expect("first box request");
    event(&mut driver, forge::Event::SetUpAnswered { forge: 1, request, answer: forge::SetUpAnswer::SettingUp });
    event(&mut driver, forge::Event::Connection { forge: 1, state: forge::Connection::SettingUp { step: None } });
    driver.immediate.clear();
    assert_eq!(
        setup(&mut driver, 62, b"work", b"hello"),
        Some(engine::ApplicationAnswer::SetUp(forge::SetUpAnswer::SettingUp))
    );
    assert!(
        !driver.immediate.iter().any(|request| matches!(request, engine::Request::SetUp { .. })),
        "same key and request must replay the answer, never start again"
    );
}

#[test]
fn first_row_moves_from_none_to_making_to_the_created_id() {
    // protocol/setup.md, 6.7; client/README.md, 4.6: row transitions preserve the created id.
    let mut driver = driver(true);
    assert!(forges(&mut driver).rows[0].first.is_none());
    let _ = setup(&mut driver, 63, b"work", b"hello");
    assert_eq!(forges(&mut driver).rows[0].first.as_ref().expect("admitted intent").state, forge::FirstState::Making);
    event(&mut driver, forge::Event::FirstMade { forge: 1, id: 4_294_967_295 });
    assert_eq!(
        forges(&mut driver).rows[0].first.as_ref().expect("made intent").state,
        forge::FirstState::Made { id: 4_294_967_295 },
        "made must carry the forge's full repository id"
    );
}

#[test]
fn engine_name_limit_is_checked_even_when_forge_grammar_allows_the_names() {
    // client/README.md, 4.6: valid forge names must also fit engine name_bytes.
    for (org, name, admitted) in [
        (b"work".as_slice(), b"repo".as_slice(), true),
        (b"works".as_slice(), b"repo".as_slice(), false),
        (b"work".as_slice(), b"repos".as_slice(), false),
    ] {
        let mut driver = driver_with_name_bytes(true, 4);
        let answer = setup(&mut driver, 64, org, name);
        let expected = if admitted {
            None
        } else {
            Some(engine::ApplicationAnswer::SetUp(forge::SetUpAnswer::Refused(forge::SetUpRefusal::Invalid)))
        };
        assert_eq!(answer, expected, "org {org:?}, repository {name:?}, name_bytes = 4");
    }
}
