#![expect(clippy::disallowed_methods, reason = "ordinary test assertions inspect captured requests")]
use super::*;
use crate::setup::lifecycle;
use skein_forge_connection::{self as connection, Change};
use temper_engine_domain_forge as domain;

fn keep_next(service: &mut crate::Service, item: Item) {
    let owner = *service.engine.as_mut().expect("engine").setup.writes.first().unwrap().0;
    assert_eq!(
        service.engine.as_ref().expect("engine").setup.writes.get(&owner).unwrap().number,
        records::number(1, item)
    );
    answer(service.engine.as_mut().expect("engine"), StoreEvent::Kept { owner });
    advance(service.engine.as_mut().expect("engine"));
}
fn converted() -> crate::Service {
    let (mut service, mut face) = writing();
    for item in [Item::AppId, Item::AppSlug, Item::AppKey, Item::User] {
        keep_next(&mut service, item);
        assert_eq!(
            service.engine.as_mut().expect("engine").setup.answers.pop(),
            Some((Token::new(2), host::Answer::Written))
        );
        face.turn(&mut service);
    }
    service
}
pub(super) fn app(service: &mut crate::Service) {
    for (item, value) in [
        (Item::AppId, Box::from(7_u64.to_le_bytes())),
        (Item::AppSlug, Box::from(b"app-fixture".as_slice())),
        (Item::AppKey, Box::from(skein_fake_forge_github::keys::KEYS[0])),
        (Item::User, Box::from(9_u64.to_le_bytes())),
    ] {
        row(service, item, &value);
    }
    let origin = records::origin(
        connection::settings::Kind::GitHub,
        &service.engine.as_ref().expect("engine").workers.forges[0].api,
    )
    .encode();
    row(service, Item::Origin, &origin);
}
pub(super) fn accepted(service: &mut crate::Service, ids: &[u64]) {
    row(service, Item::Accepted, skein_forge_setup::item::accepted_item(ids).expose());
}
fn values(service: &crate::Service) -> Box<[u64]> {
    lifecycle::accepted(service.engine.as_ref().expect("engine"), 1)
}
fn installation(service: &mut crate::Service, id: u64, change: Change) {
    service.engine.as_mut().expect("engine").forge.above.push(connection::Event::Installation {
        forge: 1,
        installation: id,
        account: Some(connection::Account {
            id: 100_u64.checked_add(id).unwrap(),
            login: Box::from(b"account-fixture".as_slice()),
        }),
        change,
    });
    crate::engine::forge_pass(service.engine.as_mut().expect("engine"));
}
fn run_events(service: &mut crate::Service) {
    for _ in 0..service.engine.as_mut().expect("engine").events.len() {
        let event = service.engine.as_mut().expect("engine").events.pop().unwrap();
        if let root::Event::Forge(_) = &event {
            service.engine.as_mut().expect("engine").step(event);
        }
    }
    for _ in 0_u32..8 {
        service.engine.as_mut().expect("engine").step(root::Event::Resume);
    }
}
#[test]
fn conversion_keeps_origin_then_installs_answers_the_browser_and_connects_from_records() {
    let mut service = converted();
    assert!(service.engine.as_ref().expect("engine").forge.starts.is_empty(), "no Connect before Origin Kept");
    for event in &service.engine.as_ref().expect("engine").events {
        if let root::Event::Setup { .. } = event {
            panic!("browser still waiting");
        }
    }
    let owner = *service.engine.as_mut().expect("engine").setup.writes.first().unwrap().0;
    let write = service.engine.as_mut().expect("engine").setup.writes.get(&owner).unwrap();
    assert_eq!(write.number, records::number(1, Item::Origin));
    assert!(service.engine.as_ref().expect("engine").setup.answers.is_empty(), "Origin is the routing's write");
    // The records, rather than the conversion outcome, determine access.
    row(&mut service, Item::User, &19_u64.to_le_bytes());
    keep_next(&mut service, Item::Origin);
    assert!(service.engine.as_ref().expect("engine").forge.starts.is_empty(), "Install runs before Connect");
    let root::SetupEnd::Done { outcome: root::SetupOutcome::Install { state, url }, .. } = terminal(&mut service)
    else {
        panic!("browser install link")
    };
    assert!(!state.is_empty());
    assert!(url.starts_with(b"http://127.0.0.1:3000/apps/app-fixture/"));
    let (forge, access) = service.engine.as_mut().expect("engine").forge.starts.get(0).unwrap();
    assert_eq!((*forge, access.user), (1, 19));
    let skein_forge::credential::Credential::App { id, key } = &access.credential else { panic!("App access") };
    assert_eq!(*id, 7);
    assert_eq!(key.expose().len(), skein_fake_forge_github::keys::KEYS[0].len());
    assert_eq!(service.engine.as_ref().expect("engine").forge.accepted.get(0).unwrap().1.as_ref(), []);
}
#[test]
fn an_install_failure_answers_the_conversion_but_the_kept_app_still_connects() {
    let mut service = converted();
    let owner = *service.engine.as_mut().expect("engine").setup.writes.first().unwrap().0;
    answer(service.engine.as_mut().expect("engine"), StoreEvent::Kept { owner });
    // Exercise the routing boundary's terminal independently of the local Install machine.
    for _ in 0..service.engine.as_mut().expect("engine").setup.above.len() {
        let _discard = service.engine.as_mut().expect("engine").setup.above.pop();
    }
    service.engine.as_mut().expect("engine").setup.above.push(host::Event::Stop {
        run: Token::new(2),
        failure: reconcile::Failure {
            forge: 1,
            step: reconcile::Stage::State,
            subject: Box::from(b"install".as_slice()),
            why: reconcile::Why::Crowded,
        },
    });
    let root::SetupEnd::Stopped { reason, .. } = terminal(&mut service) else { panic!("install failure") };
    assert!(reason.ends_with(b"Crowded"));
    assert_eq!(service.engine.as_ref().expect("engine").forge.starts.len(), 1);
}
#[test]
fn a_spent_conversion_code_names_the_possibly_orphaned_app_without_connecting() {
    let mut service = service(8, Duration::from_secs(3600));
    let state = manifest(&mut service, 1);
    start(&mut service, 2, host::Plan::Convert { forge: 1, code: code(), state });
    for _ in 0..service.engine.as_mut().expect("engine").setup.above.len() {
        let _discard = service.engine.as_mut().expect("engine").setup.above.pop();
    }
    service.engine.as_mut().expect("engine").setup.above.push(host::Event::Stop {
        run: Token::new(2),
        failure: reconcile::Failure {
            forge: 1,
            step: reconcile::Stage::App,
            subject: Box::from(b"convert".as_slice()),
            why: reconcile::Why::Refused(404),
        },
    });
    let root::SetupEnd::Stopped { reason, .. } = terminal(&mut service) else { panic!("spent code") };
    assert!(contains(&reason, b"App may exist on GitHub without temper holding its key"));
    assert!(service.engine.as_ref().expect("engine").forge.starts.is_empty());
}
#[test]
fn manifest_and_convert_refuse_even_partial_app_records_and_preserve_the_accepted_set() {
    for item in [Item::AppId, Item::AppSlug, Item::AppKey] {
        let mut service = service(8, Duration::from_secs(3600));
        let state = manifest(&mut service, 1);
        row(&mut service, item, b"partial-record");
        accepted(&mut service, &[11, 22]);
        for (run, plan) in [
            (2, host::Plan::Manifest { forge: 1, target: Target::Personal }),
            (3, host::Plan::Convert { forge: 1, code: code(), state }),
        ] {
            start(&mut service, run, plan);
            let root::SetupEnd::Stopped { reason, .. } = terminal(&mut service) else { panic!("existing App refused") };
            assert!(contains(&reason, b"Forget"));
        }
        assert_eq!(values(&service).as_ref(), [11, 22]);
        assert!(service.engine.as_ref().expect("engine").setup.io.is_empty());
    }
}
#[test]
fn connected_delivers_the_records_identity_to_the_root_then_reconciles_once() {
    let mut service = service(8, Duration::from_secs(3600));
    app(&mut service);
    service.engine.as_mut().expect("engine").forge.above.push(connection::Event::Connected { forge: 1 });
    crate::engine::forge_pass(service.engine.as_mut().expect("engine"));
    let Some(root::Event::Forge(event)) = service.engine.as_mut().expect("engine").events.pop() else {
        panic!("connected reaches root")
    };
    assert_eq!(
        *event,
        domain::Event::Connection {
            forge: 1,
            state: domain::Connection::Connected { id: 9, login: Box::from(b"app-fixture[bot]".as_slice()) }
        }
    );
    assert!(service.engine.as_ref().expect("engine").setup.runs.is_empty(), "root learns Connected first");
    advance(service.engine.as_mut().expect("engine"));
    assert!(
        service.engine.as_ref().expect("engine").setup.runs.is_empty(),
        "reconcile waits for the root to give the writer"
    );
    service.engine.as_mut().expect("engine").step(root::Event::Forge(event));
    advance(service.engine.as_mut().expect("engine"));
    assert_eq!(service.engine.as_ref().expect("engine").setup.runs.len(), 1);
    let (_, run) = service.engine.as_mut().expect("engine").setup.runs.first().unwrap();
    assert_eq!(run.kind, Kind::Reconcile);
    assert!(run.internal);
    advance(service.engine.as_mut().expect("engine"));
    assert_eq!(service.engine.as_ref().expect("engine").setup.runs.len(), 1, "one pending reconcile per forge");
}
#[test]
fn connected_holds_one_reconcile_until_the_writing_run_ends() {
    let mut service = service(8, Duration::from_secs(3600));
    app(&mut service);
    start(&mut service, 1, host::Plan::Install { forge: 1 });
    for _ in 0_u32..2 {
        service.engine.as_mut().expect("engine").forge.above.push(connection::Event::Connected { forge: 1 });
        crate::engine::forge_pass(service.engine.as_mut().expect("engine"));
    }
    run_events(&mut service);
    assert_eq!(service.engine.as_ref().expect("engine").setup.runs.len(), 1);
    assert!(!service.engine.as_ref().expect("engine").setup.runs.first().unwrap().1.internal);
    advance(service.engine.as_mut().expect("engine"));
    assert_eq!(service.engine.as_ref().expect("engine").setup.runs.len(), 1, "Install still reading");
    for _ in 0_u32..8 {
        advance(service.engine.as_mut().expect("engine"));
    }
    assert_eq!(service.engine.as_ref().expect("engine").setup.runs.len(), 1);
    assert!(service.engine.as_ref().expect("engine").setup.runs.first().unwrap().1.internal);
}
fn accepting(ids: &[u64], installation: u64) -> (crate::Service, Face) {
    let mut service = service(8, Duration::from_secs(3600));
    app(&mut service);
    accepted(&mut service, ids);
    start(&mut service, 1, host::Plan::Install { forge: 1 });
    let root::SetupEnd::Done { outcome: root::SetupOutcome::Install { state, .. }, .. } = terminal(&mut service) else {
        panic!("install state")
    };
    start(
        &mut service,
        2,
        host::Plan::Accept { forge: 1, installation: core::num::NonZeroU64::new(installation).unwrap(), state },
    );
    let mut face = Face::new();
    face.sent(&mut service);
    (service, face)
}
fn accept_reply(service: &mut crate::Service, face: &mut Face) {
    face.reply(service, br#"{"id":22,"app_id":7,"account":{"id":122,"login":"account-fixture","type":"Organization"},"target_type":"Organization","created_at":"1970-01-01T00:00:00Z","suspended_at":null}"#);
    assert!(
        !service.engine.as_ref().expect("engine").setup.writes.is_empty(),
        "Accept ended before its write: {:?}",
        terminal(service)
    );
}
#[test]
fn accept_hands_the_kept_set_to_the_component_only_after_done() {
    let (mut service, mut face) = accepting(&[11], 22);
    installation(&mut service, 22, Change::Created);
    accept_reply(&mut service, &mut face);
    assert!(service.engine.as_ref().expect("engine").forge.accepted.is_empty(), "no Installations before Kept");
    assert_eq!(values(&service).as_ref(), [11]);
    keep_next(&mut service, Item::Accepted);
    assert!(let_done(terminal(&mut service)));
    assert!(!service.engine.as_ref().expect("engine").forge.accepted.is_empty(), "Accept Done must send Installations");
    assert_eq!(service.engine.as_ref().expect("engine").forge.accepted.get(0).unwrap().1.as_ref(), [11, 22]);
    let info = service.engine.as_mut().expect("engine").setup.lifecycle.installations.get(&(1, 22)).unwrap();
    assert!(!info.stray, "Accept ends the stray mark");
    assert!(service.engine.as_ref().expect("engine").forge.starts.is_empty(), "Accept never reconnects");
}
#[test]
fn deletion_keeps_the_rewritten_set_before_handing_it_down() {
    let mut service = service(8, Duration::from_secs(3600));
    accepted(&mut service, &[11, 22]);
    installation(&mut service, 11, Change::Deleted);
    advance(service.engine.as_mut().expect("engine"));
    assert_eq!(values(&service).as_ref(), [11, 22], "old set until Kept");
    assert!(service.engine.as_ref().expect("engine").forge.accepted.is_empty());
    keep_next(&mut service, Item::Accepted);
    assert_eq!(values(&service).as_ref(), [22]);
    assert_eq!(service.engine.as_ref().expect("engine").forge.accepted.get(0).unwrap().1.as_ref(), [22]);
    installation(&mut service, 11, Change::Deleted);
    advance(service.engine.as_mut().expect("engine"));
    assert!(service.engine.as_ref().expect("engine").setup.writes.is_empty(), "same deletion writes nothing twice");
}
#[test]
fn deletion_during_accept_waits_then_rewrites_from_the_newly_kept_set() {
    let (mut service, mut face) = accepting(&[11], 22);
    installation(&mut service, 11, Change::Deleted);
    advance(service.engine.as_mut().expect("engine"));
    assert!(service.engine.as_ref().expect("engine").setup.writes.is_empty(), "deletion must wait for Accept");
    accept_reply(&mut service, &mut face);
    assert_eq!(service.engine.as_ref().expect("engine").setup.writes.len(), 1);
    keep_next(&mut service, Item::Accepted);
    for _ in 0_u32..4 {
        advance(service.engine.as_mut().expect("engine"));
    }
    assert_eq!(values(&service).as_ref(), [11, 22], "Accept's kept set is the rewrite's source");
    assert_eq!(service.engine.as_ref().expect("engine").setup.writes.len(), 1, "deferred deletion now writes");
    keep_next(&mut service, Item::Accepted);
    assert_eq!(values(&service).as_ref(), [22], "neither Accept nor deletion is lost");
    assert_eq!(service.engine.as_ref().expect("engine").forge.accepted.get(1).unwrap().1.as_ref(), [22]);
}
#[test]
fn a_first_listing_deletion_suspension_and_stray_are_idempotent() {
    let mut service = service(8, Duration::from_secs(3600));
    accepted(&mut service, &[11, 22]);
    for _ in 0_u32..2 {
        installation(&mut service, 11, Change::Deleted);
        installation(&mut service, 22, Change::Suspended);
        installation(&mut service, 33, Change::Created);
    }
    assert_eq!(
        service.engine.as_ref().expect("engine").events.len(),
        3,
        "same deleted, suspended and stray states reach root only once"
    );
    assert!(service.engine.as_ref().expect("engine").setup.lifecycle.installations.get(&(1, 22)).unwrap().suspended);
    assert!(service.engine.as_ref().expect("engine").setup.lifecycle.installations.get(&(1, 33)).unwrap().stray);
    installation(&mut service, 22, Change::Repositories);
    assert_eq!(service.engine.as_ref().expect("engine").events.len(), 3, "same repositories data is silent");
    assert!(service.engine.as_ref().expect("engine").setup.lifecycle.installations.get(&(1, 22)).unwrap().suspended);
    for _ in 0_u32..3 {
        let _discard = service.engine.as_mut().expect("engine").events.pop().unwrap();
    }
    let mut updated =
        service.engine.as_mut().expect("engine").setup.lifecycle.installations.get(&(1, 22)).unwrap().clone();
    updated.account = Some((222, Box::from(b"changed-account".as_slice())));
    for _ in 0_u32..2 {
        service.engine.as_mut().expect("engine").forge.above.push(connection::Event::Installation {
            forge: 1,
            installation: 22,
            account: Some(connection::Account { id: 222, login: Box::from(b"changed-account".as_slice()) }),
            change: Change::Repositories,
        });
        crate::engine::forge_pass(service.engine.as_mut().expect("engine"));
    }
    assert_eq!(service.engine.as_ref().expect("engine").events.len(), 1, "changed account is emitted once");
    let event = domain::Event::Installation { forge: 1, installation: 22, state: Some(updated) };
    let root::Event::Forge(emitted) = service.engine.as_mut().expect("engine").events.pop().unwrap() else {
        panic!("installation event")
    };
    assert_eq!(*emitted, event);
    advance(service.engine.as_mut().expect("engine"));
    assert_eq!(service.engine.as_ref().expect("engine").setup.writes.len(), 1);
    keep_next(&mut service, Item::Accepted);
    assert_eq!(values(&service).as_ref(), [22]);
    installation(&mut service, 33, Change::Deleted);
    assert!(!service.engine.as_ref().expect("engine").setup.lifecycle.installations.contains_key(&(1, 33)));
}
#[test]
fn a_stopped_event_reaches_the_root_and_a_later_connected_clears_it() {
    let mut service = fixtures::ready();
    let why = connection::Stopped::KeyRefused;
    service.engine.as_mut().expect("engine").forge.above.push(connection::Event::Stopped { forge: 1, why });
    crate::engine::forge_pass(service.engine.as_mut().expect("engine"));
    run_events(&mut service);
    assert_eq!(
        service.engine.as_ref().expect("engine").domain.forge_connection(1),
        domain::Connection::Stopped { reason: Box::from(b"KeyRefused".as_slice()) }
    );
    service.engine.as_mut().expect("engine").forge.above.push(connection::Event::Connected { forge: 1 });
    crate::engine::forge_pass(service.engine.as_mut().expect("engine"));
    run_events(&mut service);
    assert_eq!(
        service.engine.as_ref().expect("engine").domain.forge_connection(1),
        domain::Connection::Connected { id: 42, login: Box::from(b"temper".as_slice()) }
    );
}
#[test]
fn a_reconcile_failure_names_its_stop_without_connecting_or_changing_the_connection() {
    let mut service = service(8, Duration::from_secs(3600));
    app(&mut service);
    start(&mut service, 1, host::Plan::Reconcile { forge: 1, adopted: Box::new([]) });
    for _ in 0..service.engine.as_mut().expect("engine").setup.above.len() {
        let _discard = service.engine.as_mut().expect("engine").setup.above.pop();
    }
    service.engine.as_mut().expect("engine").setup.above.push(host::Event::Stop {
        run: Token::new(1),
        failure: reconcile::Failure {
            forge: 1,
            step: reconcile::Stage::App,
            subject: Box::from(b"app".as_slice()),
            why: reconcile::Why::Refused(401),
        },
    });
    let root::SetupEnd::Stopped { reason, .. } = terminal(&mut service) else { panic!("reconcile stop") };
    assert!(contains(&reason, b"Refused(401)"));
    assert!(service.engine.as_ref().expect("engine").forge.starts.is_empty());
    assert_eq!(service.engine.as_ref().expect("engine").domain.forge_connection(1), domain::Connection::NotConnected);
}

fn connection_turn(service: &mut crate::Service, face: &mut Face) {
    service.engine.as_mut().expect("engine").forge.iterate(Time::ZERO, Wall::EPOCH);
    for _ in 0..service.engine.as_mut().expect("engine").forge.io.len() {
        match service.engine.as_mut().expect("engine").forge.io.pop().unwrap() {
            skein_io::Request::Connect { owner, .. } => {
                face.owner = Some(owner);
                face.wire = skein_world::stream::Wire::new(7);
                service
                    .engine
                    .as_mut()
                    .expect("engine")
                    .forge
                    .up(skein_io::Event::Connecting { owner, socket: Token::new(321) });
                service.engine.as_mut().expect("engine").forge.up(skein_io::Event::Connected { owner });
            }
            skein_io::Request::Stream { down, .. } => face.wire.take(down),
            skein_io::Request::Close { .. } | skein_io::Request::Abort { .. } => {
                service
                    .engine
                    .as_mut()
                    .expect("engine")
                    .forge
                    .up(skein_io::Event::Closed { owner: face.owner.take().unwrap() });
            }
            skein_io::Request::Listen { .. }
            | skein_io::Request::Bind { .. }
            | skein_io::Request::Reject { .. }
            | skein_io::Request::Output { .. }
            | skein_io::Request::Spawn { .. }
            | skein_io::Request::Signal { .. }
            | skein_io::Request::Usage { .. }
            | skein_io::Request::Window { .. } => panic!("connection socket request"),
        }
    }
    if let Some(owner) = face.owner
        && let Some(up) = face.wire.answer()
    {
        service.engine.as_mut().expect("engine").forge.up(skein_io::Event::Stream { owner, up });
    }
}
fn locate(service: &mut crate::Service, call: u64) {
    service.engine.as_mut().expect("engine").forge.down(connection::Request::Call {
        call: Token::new(call),
        repository: skein_forge::api::Repository { forge: 1, repository: 0 },
        op: skein_forge::api::Op::Read(skein_forge::api::Read::Locate {
            owner: Box::from(b"account-fixture".as_slice()),
            name: Box::from(b"repository-fixture".as_slice()),
        }),
    });
}
#[test]
fn calls_are_not_connected_before_connect_then_wait_until_connected() {
    let mut service = converted();
    locate(&mut service, 1);
    let Some(connection::Event::Answered(answer)) = service.engine.as_mut().expect("engine").forge.above.pop() else {
        panic!("NotConnected before Connect")
    };
    assert_eq!((answer.cost, answer.result), (0, Err(skein_forge::api::Error::NotConnected)));
    keep_next(&mut service, Item::Origin);
    assert!(let_done(terminal(&mut service)));
    locate(&mut service, 2);
    assert!(
        service.engine.as_ref().expect("engine").forge.above.is_empty(),
        "a call after Connect waits for the start"
    );
    let mut face = Face::new();
    for _ in 0_u32..1024 {
        connection_turn(&mut service, &mut face);
    }
    assert!(contains(&face.wire.received, b"GET /app/installations"));
    face.wire.write(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n[]");
    for _ in 0_u32..1024 {
        connection_turn(&mut service, &mut face);
    }
    let mut connected = false;
    let mut answered = false;
    for _ in 0..service.engine.as_mut().expect("engine").forge.above.len() {
        match service.engine.as_mut().expect("engine").forge.above.pop().unwrap() {
            connection::Event::Connected { forge: 1 } => connected = true,
            connection::Event::Answered(answer) => {
                assert!(connected, "the waiting call is answered after Connected");
                assert_eq!(
                    (answer.call, answer.cost, answer.result),
                    (Token::new(2), 0, Err(skein_forge::api::Error::NotInstalled))
                );
                answered = true;
            }
            connection::Event::Installation { .. }
            | connection::Event::Fact { .. }
            | connection::Event::GitToken { .. }
            | connection::Event::Hint(_)
            | connection::Event::ListenerLost
            | connection::Event::Listening { .. }
            | connection::Event::Connected { .. }
            | connection::Event::Stopped { .. }
            | connection::Event::Closed => panic!("connection start event"),
        }
    }
    assert!(connected && answered);
}

#[test]
fn a_real_reconcile_prunes_accepted_after_kept_and_never_connects_or_writes_origin() {
    let mut service = service(8, Duration::from_secs(3600));
    app(&mut service);
    accepted(&mut service, &[11]);
    start(&mut service, 1, host::Plan::Reconcile { forge: 1, adopted: Box::new([]) });
    let mut face = Face::new();
    face.sent(&mut service);
    face.reply(&mut service, br#"{"id":7,"slug":"app-fixture","permissions":{},"events":[]}"#);
    face.sent(&mut service);
    face.reply(&mut service, br#"{"url":"https://app-fixture.invalid/"}"#);
    face.sent(&mut service);
    face.reply(&mut service, b"[]");
    assert_eq!(values(&service).as_ref(), [11]);
    assert_eq!(service.engine.as_ref().expect("engine").setup.writes.len(), 1);
    keep_next(&mut service, Item::Accepted);
    assert!(let_done(terminal(&mut service)), "Lacking is a successful reconcile");
    let Some(crate::Note::ForgeLacking { number, names }) = service.engine.as_mut().expect("engine").notes.pop() else {
        panic!("Lacking names logged")
    };
    assert_eq!(number, 1);
    assert!(contains(&names, b"permission contents=write;"));
    assert!(contains(&names, b"event push;"));
    assert_eq!(u64::try_from(names.len()).unwrap(), diagnostics::worst_case().unwrap());
    assert!(values(&service).is_empty());
    assert!(service.engine.as_ref().expect("engine").setup.writes.is_empty(), "reconcile writes no Origin");
    assert!(service.engine.as_ref().expect("engine").forge.starts.is_empty(), "reconcile connects nothing");
    assert!(service.engine.as_ref().expect("engine").forge.accepted.get(0).unwrap().1.is_empty());
}
fn connection_reply(service: &mut crate::Service, face: &mut Face, body: &[u8]) {
    let length = skein_lib::Decimal::of(u64::try_from(body.len()).unwrap());
    let head = b"HTTP/1.1 200 OK\r\nContent-Length: ";
    let mut writer = skein_lib::Writer::new(
        head.len()
            .checked_add(length.as_bytes().len())
            .unwrap()
            .checked_add(4)
            .unwrap()
            .checked_add(body.len())
            .unwrap(),
    );
    writer.put(head).unwrap();
    writer.put(length.as_bytes()).unwrap();
    writer.put(b"\r\n\r\n").unwrap();
    writer.put(body).unwrap();
    face.wire.write(&writer.finish());
    for _ in 0_u32..1024 {
        connection_turn(service, face);
    }
}
#[test]
fn the_components_first_listing_routes_gone_suspended_and_stray_installations() {
    let mut service = service(8, Duration::from_secs(3600));
    app(&mut service);
    accepted(&mut service, &[11, 22]);
    let expected = records::origin(
        connection::settings::Kind::GitHub,
        &service.engine.as_ref().expect("engine").workers.forges[0].api,
    );
    let engine = service.engine.as_mut().expect("engine");
    let start = records::access(
        connection::settings::Kind::GitHub,
        &expected,
        engine.forge_records.rows.forge(1).unwrap(),
        &engine.forge_records.bounds,
        false,
    )
    .unwrap();
    service
        .engine
        .as_mut()
        .expect("engine")
        .forge
        .down(connection::Request::Connect { forge: 1, access: start.access });
    service
        .engine
        .as_mut()
        .expect("engine")
        .forge
        .down(connection::Request::Installations { forge: 1, accepted: start.accepted });
    let mut face = Face::new();
    for _ in 0_u32..1024 {
        connection_turn(&mut service, &mut face);
    }
    connection_reply(&mut service, &mut face, br#"[{"id":22,"account":{"id":122,"login":"suspended-account","type":"Organization"},"created_at":"1970-01-01T00:00:00Z","suspended_at":"2026-01-01T00:00:00Z"},{"id":33,"account":{"id":133,"login":"stray-account","type":"User"},"created_at":"1970-01-01T00:00:00Z","suspended_at":null}]"#);
    crate::engine::forge_pass(service.engine.as_mut().expect("engine"));
    assert!(service.engine.as_ref().expect("engine").setup.lifecycle.installations.get(&(1, 22)).unwrap().suspended);
    assert!(service.engine.as_ref().expect("engine").setup.lifecycle.installations.get(&(1, 33)).unwrap().stray);
    assert_eq!(values(&service).as_ref(), [11, 22]);
    advance(service.engine.as_mut().expect("engine"));
    keep_next(&mut service, Item::Accepted);
    assert_eq!(values(&service).as_ref(), [22], "the missing accepted id is pruned exactly once");
    assert_eq!(service.engine.as_ref().expect("engine").forge.accepted.get(1).unwrap().1.as_ref(), [22]);
}

#[test]
fn a_manifest_already_started_without_records_is_unaffected_by_records_arriving() {
    let mut service = service(8, Duration::from_secs(3600));
    start(&mut service, 1, host::Plan::Manifest { forge: 1, target: Target::Personal });
    row(&mut service, Item::AppId, &7_u64.to_le_bytes());
    let root::SetupEnd::Done { outcome: root::SetupOutcome::Manifest { state }, .. } = terminal(&mut service) else {
        panic!("admitted Manifest finishes")
    };
    assert!(service.setup_page(1, &state, Time::ZERO).is_some());
    start(&mut service, 2, host::Plan::Manifest { forge: 1, target: Target::Personal });
    let root::SetupEnd::Stopped { reason, .. } = terminal(&mut service) else { panic!("later Manifest refused") };
    assert!(contains(&reason, b"Forget"));
}
#[test]
fn new_app_admission_changes_neither_a_connected_nor_a_stopped_forges_state() {
    for state in [
        domain::Connection::Connected { id: 42, login: Box::from(b"temper".as_slice()) },
        domain::Connection::Stopped { reason: Box::from(b"key refused".as_slice()) },
    ] {
        let mut service = fixtures::ready();
        // Use the real startup host for this forge's loopback GitHub setup boundary.
        let mut setup = super::service(8, Duration::from_secs(3600));
        app(&mut setup);
        let setup = setup.engine.expect("engine");
        service.engine.as_mut().expect("engine").setup = setup.setup;
        service.engine.as_mut().expect("engine").forge_records.rows = setup.forge_records.rows;
        service
            .engine
            .as_mut()
            .expect("engine")
            .step(root::Event::Forge(Box::new(domain::Event::Connection { forge: 1, state: state.clone() })));
        start(&mut service, 1, host::Plan::Manifest { forge: 1, target: Target::Personal });
        let root::SetupEnd::Stopped { reason, .. } = terminal(&mut service) else { panic!("new App refused") };
        assert!(contains(&reason, b"Forget"));
        assert_eq!(service.engine.as_ref().expect("engine").domain.forge_connection(1), state);
        assert!(service.engine.as_ref().expect("engine").forge.starts.is_empty());
    }
}
#[test]
fn pending_reconcile_wakes_the_loop_only_when_a_run_slot_is_available() {
    let mut service = service(8, Duration::from_secs(3600));
    app(&mut service);
    lifecycle::connected(service.engine.as_mut().expect("engine"), 1);
    run_events(&mut service);
    assert!(
        service.engine.as_ref().expect("engine").setup.lifecycle_ready(service.engine.as_ref().expect("engine")),
        "Connected after the setup pass needs a new pass"
    );
    service.engine.as_mut().expect("engine").setup.env.limits.runs = 1;
    let token = Token::new(99);
    service
        .engine
        .as_mut()
        .expect("engine")
        .setup
        .runs
        .insert(
            token,
            Run {
                first: None,
                forge: 2,
                kind: Kind::Install,
                internal: false,
                previous: domain::Connection::NotConnected,
            },
        )
        .unwrap();
    assert!(
        !service.engine.as_ref().expect("engine").setup.lifecycle_ready(service.engine.as_ref().expect("engine")),
        "no busy loop while all run slots wait for IO"
    );
    service.engine.as_mut().expect("engine").setup.runs.remove(&token).unwrap();
    assert!(service.engine.as_ref().expect("engine").setup.lifecycle_ready(service.engine.as_ref().expect("engine")));
    advance(service.engine.as_mut().expect("engine"));
    assert!(
        !service.engine.as_ref().expect("engine").setup.lifecycle_ready(service.engine.as_ref().expect("engine")),
        "the reconcile now owns the forge"
    );
}

fn carried_pem() -> (Secret, Secret) {
    let der = skein_fake_forge_github::keys::KEYS[0];
    let size = skein_lib::base64::encoded_len(skein_lib::base64::Alphabet::Standard, u32::try_from(der.len()).unwrap())
        .unwrap();
    let mut writer = skein_lib::Writer::new(
        usize::try_from(size)
            .unwrap()
            .checked_add(b"-----BEGIN RSA PRIVATE KEY-----\n".len())
            .unwrap()
            .checked_add(b"\n-----END RSA PRIVATE KEY-----\n".len())
            .unwrap(),
    );
    writer.put(b"-----BEGIN RSA PRIVATE KEY-----\n").unwrap();
    skein_lib::base64::encode(skein_lib::base64::Alphabet::Standard, der, &mut writer).unwrap();
    writer.put(b"\n-----END RSA PRIVATE KEY-----\n").unwrap();
    let pem = Secret::new(writer.finish());
    let mut bundle = skein_lib::Writer::new(pem.expose().len().checked_add(6).unwrap());
    bundle.put(&[1, 1, 11, 0]).unwrap();
    bundle.put(&u16::try_from(pem.expose().len()).unwrap().to_le_bytes()).unwrap();
    bundle.put(pem.expose()).unwrap();
    (pem, Secret::new(bundle.finish()))
}
fn paste_start(service: &mut crate::Service, bundle: Secret, digest: [u8; 32]) {
    let engine = service.engine.as_mut().unwrap();
    engine.store.secrets(Token::new(901), fixtures::application_request(records::CARRIED, bundle));
    let Some(StoreEvent::Carried { number, .. }) = engine.store.above.pop() else { panic!("carried seed kept") };
    assert_eq!(number, records::CARRIED);
    public::connect(
        engine,
        1,
        Token::new(90),
        core::num::NonZeroU64::new(7).unwrap(),
        jig_domain_core::Carried { number: records::CARRIED, digest },
    );
}
// Script only the parent's already-admitted request, then feed the service's real
// terminal into the real connector. Authentication is the root world's contract.
fn paste_row(limits: &domain::Limits, request: Token, connected: bool, why: Option<Box<[u8]>>) -> domain::Forges {
    let mut forge = domain::Domain::new(
        limits,
        7,
        skein_forge_client::Config { namespace: Box::from(b"fixture".as_slice()), writers: Box::new([]) },
        &[],
        &[domain::Configured {
            bot: Box::new([]),
            admin_connect: false,
            owned: None,
            web: None,
            read_only: false,
            number: 1,
            kind: domain::Kind::GitHub,
            mode: domain::Mode::Connected,
            asked: false,
        }],
        false,
    )
    .unwrap();
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: *limits };
    let mut out = Queue::with_capacity(domain::max_out(limits));
    domain::step(
        &mut forge,
        &env,
        domain::Event::Application {
            request,
            token: request,
            retention: Duration::from_secs(3600),
            ask: domain::Ask::Connect {
                forge: 1,
                how: domain::How::GitHubApp { app: core::num::NonZeroU64::new(7).unwrap() },
            },
            role: Some(jig_domain_people::Role::Owner),
            carried: Some(jig_domain_core::Carried { number: records::CARRIED, digest: [0; 32] }),
        },
        &mut out,
    );
    while out.pop().is_some() {}
    domain::step(
        &mut forge,
        &env,
        domain::Event::ConnectEnded { stop: None, forge: 1, request, connected, why },
        &mut out,
    );
    forge.forges()
}
#[expect(clippy::wildcard_enum_match_arm, reason = "test rejects all unrelated forge events")]
#[expect(clippy::disallowed_macros, reason = "test formats captured records to scan for PEM")]
fn paste_ended(service: &mut crate::Service, connected: bool, pem: &[u8]) -> Option<Box<[u8]>> {
    let engine = service.engine.as_mut().unwrap();
    assert!(engine.setup.public.is_empty(), "run releases the PEM");
    assert_eq!(engine.store.erases, 0, "routing waits for the root's erase decision");
    let mut ended = 0_u32;
    let mut last_end = None;
    while let Some(event) = engine.events.pop() {
        let text = alloc::format!("{event:?}");
        assert!(!contains(text.as_bytes(), pem));
        if let root::Event::Forge(event) = event {
            match *event {
                domain::Event::ConnectEnded { stop: None, why, forge: 1, request, connected: actual } => {
                    assert_eq!((request, actual), (Token::new(90), connected));
                    let rows = paste_row(&engine.env.limits.forge, request, actual, why.clone());
                    assert_eq!(rows.rows[0].ended, why);
                    let bytes = crate::family::forges(rows, &temper_client_protocol_wire::v1::CEILINGS).unwrap();
                    assert!(!contains(&bytes, pem));
                    last_end = why;
                    ended = ended.checked_add(1).unwrap();
                }
                domain::Event::ConnectAnswered { answer: domain::ConnectAnswer::Connecting, .. } => {}
                _ => panic!("unexpected paste event"),
            }
        }
    }
    assert_eq!(ended, 1);
    for note in &engine.notes {
        if let crate::Note::BoxLink { line } = note {
            assert!(!contains(line, pem));
        }
        assert!(!contains(alloc::format!("{note:?}").as_bytes(), pem));
    }
    while engine.notes.pop().is_some() {}
    if connected {
        let rows = engine.forge_records.rows.forge(1).unwrap();
        for item in [Item::AppId, Item::AppSlug, Item::AppKey, Item::User, Item::Origin] {
            assert!(!contains(rows.get(item).unwrap(), pem), "only decoded items are kept");
        }
        assert_eq!(rows.get(Item::AppKey).unwrap(), skein_fake_forge_github::keys::KEYS[0]);
    }
    for (_, write) in &engine.setup.writes {
        assert!(!contains(write.value.expose(), pem));
    }
    // The root's decision is covered through its real admitted application in domain tests.
    crate::route::request(engine, root::Request::EraseCarried { number: records::CARRIED });
    assert_eq!(engine.store.erases, 1);
    while let Some(event) = engine.store.above.pop() {
        if let StoreEvent::Erased { .. } = event {
            answer(engine, event);
        }
    }
    assert_eq!(engine.forge.starts.len(), u32::from(connected));
    let Some(StoreEvent::Secret { value: None, .. }) = engine.store.secrets(
        Token::new(902),
        jig_protocol_store::Secrets::Get {
            owner: jig_protocol_store::SecretOwner::Application,
            number: records::CARRIED,
        },
    ) else {
        panic!("carried row erased")
    };
    last_end
}
#[test]
fn paste_reads_the_carried_pem_keeps_identity_then_origin_connects_and_erases_after_root_decides() {
    let (pem, bundle) = carried_pem();
    paste_connects(pem, bundle);
}
#[test]
fn paste_reads_the_fakes_key_as_a_file_or_as_the_web_fields_restored_armour() {
    let (pem, _) = carried_pem();
    let restored = pem.expose().strip_suffix(b"\n").unwrap();
    let body = restored
        .strip_prefix(b"-----BEGIN RSA PRIVATE KEY-----\n")
        .unwrap()
        .strip_suffix(b"\n-----END RSA PRIVATE KEY-----")
        .unwrap();
    assert!(!body.contains(&b'\n'), "the web field's restored body is one line");
    let lines = body.len().checked_add(63).unwrap().checked_div(64).unwrap();
    let mut file = skein_lib::Writer::new(restored.len().checked_add(lines).unwrap());
    file.put(b"-----BEGIN RSA PRIVATE KEY-----\n").unwrap();
    for line in body.chunks(64) {
        file.put(line).unwrap();
        file.put(b"\n").unwrap();
    }
    file.put(b"-----END RSA PRIVATE KEY-----\n").unwrap();
    let file = file.finish();
    for bytes in [restored, pem.expose(), file.as_ref()] {
        let mut bundle = skein_lib::Writer::new(bytes.len().checked_add(6).unwrap());
        bundle.put(&[1, 1, 11, 0]).unwrap();
        bundle.put(&u16::try_from(bytes.len()).unwrap().to_le_bytes()).unwrap();
        bundle.put(bytes).unwrap();
        paste_connects(Secret::new(Box::from(bytes)), Secret::new(bundle.finish()));
    }
}
fn paste_connects(pem: Secret, bundle: Secret) {
    let mut service = service(8, Duration::from_secs(3600));
    let digest = skein_lib::sha256::digest(bundle.expose());
    paste_start(&mut service, bundle, digest);
    let mut face = Face::new();
    face.sent(&mut service);
    let (_, host::Answer::Read(Some(read))) = service.engine.as_mut().unwrap().setup.answers.pop().unwrap() else {
        panic!("Pasted read")
    };
    assert_eq!(read.expose(), pem.expose());
    assert!(!contains(&face.wire.received, pem.expose()));
    face.reply(&mut service, br#"{"id":7,"slug":"app-fixture","permissions":{},"events":[]}"#);
    face.sent(&mut service);
    face.reply(&mut service, br#"{"id":9,"login":"app-fixture[bot]"}"#);
    face.sent(&mut service);
    face.reply(&mut service, br#"{"url":"https://app-fixture.invalid/","content_type":"json","insecure_ssl":"0"}"#);
    for item in [Item::AppId, Item::AppSlug, Item::AppKey, Item::User] {
        assert!(service.engine.as_ref().unwrap().forge.starts.is_empty());
        keep_next(&mut service, item);
        face.turn(&mut service);
    }
    assert!(service.engine.as_ref().unwrap().forge.starts.is_empty());
    keep_next(&mut service, Item::Origin);
    assert!(service.engine.as_ref().unwrap().forge.starts.is_empty(), "Connect waits for root-directed durable erase");
    assert_eq!(paste_ended(&mut service, true, pem.expose()), None);
    service.engine.as_mut().unwrap().forge.above.push(connection::Event::Connected { forge: 1 });
    crate::engine::forge_pass(service.engine.as_mut().unwrap());
    let event = service.engine.as_mut().unwrap().events.pop().expect("Connected tells the root");
    let root::Event::Forge(connection) = &event else { panic!("root connection event") };
    let domain::Event::Connection { state: domain::Connection::Connected { .. }, .. } = connection.as_ref() else {
        panic!("Connected event")
    };
    service.engine.as_mut().unwrap().step(event);
    advance(service.engine.as_mut().unwrap());
    assert_eq!(service.engine.as_ref().unwrap().setup.runs.first().unwrap().1.kind, Kind::Reconcile);
}
#[test]
fn a_changed_digest_writes_nothing_logs_the_reason_and_erases_after_root_decides() {
    let mut service = service(8, Duration::from_secs(3600));
    let (pem, bundle) = carried_pem();
    paste_start(&mut service, bundle, [0; 32]);
    assert!(service.engine.as_ref().unwrap().setup.writes.is_empty());
    assert!(service.engine.as_ref().unwrap().setup.io.is_empty());
    assert!(service.engine.as_ref().unwrap().notes.iter().any(|note| if let crate::Note::BoxLink { line } = note {
        line.as_ref() == b"the pasted value changed"
    } else {
        false
    }));
    assert_eq!(paste_ended(&mut service, false, pem.expose()).as_deref(), Some(b"the pasted value changed".as_slice()));
}
#[test]
fn a_stopped_paste_erases_its_carried_row_and_keeps_no_pem_in_its_terminal() {
    let mut service = service(8, Duration::from_secs(3600));
    let (pem, bundle) = carried_pem();
    let digest = skein_lib::sha256::digest(bundle.expose());
    paste_start(&mut service, bundle, digest);
    let mut face = Face::new();
    face.sent(&mut service);
    // Valid JSON with a different App proves that a host stop uses the same cleanup path.
    face.reply(&mut service, br#"{"id":8,"slug":"other-app","permissions":{},"events":[]}"#);
    assert!(service.engine.as_ref().unwrap().setup.writes.is_empty());
    assert!(paste_ended(&mut service, false, pem.expose()).is_some());
}
#[test]
fn a_carried_bundle_of_the_wrong_item_is_not_a_key() {
    let mut service = service(8, Duration::from_secs(3600));
    let bundle = Secret::new(Box::from([1, 1, 1, 0, 1, 0, 42]));
    let digest = skein_lib::sha256::digest(bundle.expose());
    paste_start(&mut service, bundle, digest);
    assert!(service.engine.as_ref().unwrap().notes.iter().any(|note| if let crate::Note::BoxLink { line } = note {
        line.as_ref() == b"the pasted value is not a key"
    } else {
        false
    }));
    assert!(service.engine.as_ref().unwrap().setup.writes.is_empty());
}

fn install_link(service: &mut crate::Service, back: Option<&[u8]>) -> Box<[u8]> {
    public::install(service.engine.as_mut().unwrap(), 1, Token::new(91), back.map(Box::from));
    assert!(service.engine.as_ref().unwrap().events.is_empty(), "the link waits for Done");
    for _ in 0_u32..8 {
        advance(service.engine.as_mut().unwrap());
    }
    let engine = service.engine.as_mut().unwrap();
    let mut link = None;
    while let Some(event) = engine.events.pop() {
        if let root::Event::Forge(event) = event {
            let domain::Event::InstallAnswered { request, answer: domain::InstallAnswer::Link(url), .. } = *event
            else {
                panic!("install link")
            };
            assert_eq!(request, Token::new(91));
            link = Some(url);
        }
    }
    let link = link.expect("Install Done answers the link");
    assert!(engine.setup.public.is_empty());
    let at = skein_lib::bytes::find(&link, b"state=").unwrap().checked_add(6).unwrap();
    Box::from(&link[at..])
}
fn setup_target(state: &[u8]) -> Box<[u8]> {
    let mut writer = skein_lib::Writer::new(
        b"/forges/1/github/setup?installation_id=22&setup_action=install&state="
            .len()
            .checked_add(state.len())
            .unwrap(),
    );
    writer.put(b"/forges/1/github/setup?installation_id=22&setup_action=install&state=").unwrap();
    writer.put(state).unwrap();
    writer.finish()
}
fn visit(service: &mut crate::Service, method: &[u8], target: &[u8]) -> fixtures::http::Peer {
    let mut peer = fixtures::http::Peer::new(service.engine.as_mut().unwrap(), method, target, &[], None);
    pump_visit(service, &mut peer);
    peer
}
fn pump_visit(service: &mut crate::Service, peer: &mut fixtures::http::Peer) {
    for _ in 0_u32..4 {
        peer.pump(service.engine.as_mut().unwrap(), false);
        advance(service.engine.as_mut().unwrap());
    }
    peer.pump(service.engine.as_mut().unwrap(), false);
}
fn status(peer: &fixtures::http::Peer, code: &[u8]) {
    assert!(contains(peer.received(), code), "response: {}", core::str::from_utf8(peer.received()).unwrap());
    for header in [b"Cache-Control: no-store".as_slice(), b"Referrer-Policy: no-referrer"] {
        assert!(contains(peer.received(), header));
    }
    if code != b"302" && code != b"303" {
        for part in [
            b"<!doctype html><html lang=\"en\">".as_slice(),
            b"<meta charset=\"utf-8\">",
            b"name=\"viewport\"",
            b"<style>",
            b"Back to temper</a>",
            b"Content-Security-Policy: default-src 'none';",
        ] {
            assert!(contains(peer.received(), part));
        }
    }
}
fn complete_accept(service: &mut crate::Service, peer: &mut fixtures::http::Peer) {
    assert!(peer.received().is_empty(), "HTTP waits for Accept");
    let mut face = Face::new();
    face.sent(service);
    face.reply(service, br#"{"id":22,"app_id":7,"account":{"id":122,"login":"account-fixture","type":"Organization"},"target_type":"Organization","created_at":"1970-01-01T00:00:00Z","suspended_at":null}"#);
    assert!(peer.received().is_empty(), "HTTP waits for Accepted's durable keep");
    keep_next(service, Item::Accepted);
    pump_visit(service, peer);
}
#[test]
fn install_stop_answers_connecting_without_a_link() {
    let mut service = service(8, Duration::from_secs(3600));
    public::install(service.engine.as_mut().unwrap(), 1, Token::new(91), None);
    assert!(service.engine.as_ref().unwrap().events.is_empty());
    for _ in 0_u32..8 {
        advance(service.engine.as_mut().unwrap());
    }
    let Some(root::Event::Forge(event)) = service.engine.as_mut().unwrap().events.pop() else {
        panic!("Install stop answer")
    };
    assert_eq!(
        *event,
        domain::Event::InstallAnswered {
            stopped: Some(Box::from(b"setup App slug: Missing".as_slice())),
            forge: 1,
            request: Token::new(91),
            answer: domain::InstallAnswer::Connecting
        }
    );
}
#[test]
fn setup_accept_redirects_to_back_after_done_and_a_refresh_is_ended() {
    let mut service = service(8, Duration::from_secs(3600));
    app(&mut service);
    let state = install_link(&mut service, Some(b"/forges?from=connect"));
    let target = setup_target(&state);
    let mut peer = visit(&mut service, b"GET", &target);
    complete_accept(&mut service, &mut peer);
    status(&peer, b"303");
    assert!(contains(peer.received(), b"Location: /forges?from=connect"));
    let again = visit(&mut service, b"GET", &target);
    status(&again, b"410");
    assert!(contains(again.received(), b"This link has ended"));
}
#[test]
fn setup_accept_without_back_answers_its_close_tab_page() {
    let mut service = service(8, Duration::from_secs(3600));
    app(&mut service);
    let state = install_link(&mut service, None);
    let mut peer = visit(&mut service, b"GET", &setup_target(&state));
    complete_accept(&mut service, &mut peer);
    status(&peer, b"200");
    assert!(contains(peer.received(), b"<h2>GitHub is connected</h2>"));
    assert!(contains(peer.received(), b"temper has the installation; you may close this tab."));
    assert!(contains(peer.received(), b"href=\"/forges\""));
}
#[test]
fn setup_unknown_state_is_ended_without_calling_github() {
    let mut service = service(8, Duration::from_secs(3600));
    app(&mut service);
    let peer = visit(&mut service, b"GET", &setup_target(b"unknown"));
    status(&peer, b"410");
    assert!(service.engine.as_ref().unwrap().setup.io.is_empty());
}
#[test]
fn setup_expired_state_is_ended() {
    let mut service = service(8, Duration::from_secs(3));
    app(&mut service);
    let state = install_link(&mut service, Some(b"/forges"));
    service.engine.as_mut().unwrap().env.now = Time::ZERO.saturating_add(Duration::from_secs(3));
    let peer = visit(&mut service, b"GET", &setup_target(&state));
    status(&peer, b"410");
}
#[test]
fn setup_accept_stop_answers_the_ended_page() {
    let mut service = service(8, Duration::from_secs(3600));
    app(&mut service);
    let state = install_link(&mut service, Some(b"/forges"));
    let mut peer = visit(&mut service, b"GET", &setup_target(&state));
    let mut face = Face::new();
    face.sent(&mut service);
    face.reply(&mut service, br#"{"id":23,"app_id":7,"account":{"id":122,"login":"account-fixture","type":"Organization"},"target_type":"Organization","created_at":"1970-01-01T00:00:00Z","suspended_at":null}"#);
    pump_visit(&mut service, &mut peer);
    status(&peer, b"410");
}
#[test]
fn setup_request_uses_and_removes_the_back_entry_without_accepting() {
    let mut service = service(8, Duration::from_secs(3600));
    app(&mut service);
    let state = install_link(&mut service, Some(b"/forges?first=1&next=2"));
    let mut target = skein_lib::Writer::new(
        b"/forges/1/github/setup?setup_action=request&state=".len().checked_add(state.len()).unwrap(),
    );
    target.put(b"/forges/1/github/setup?setup_action=request&state=").unwrap();
    target.put(&state).unwrap();
    let target = target.finish();
    let peer = visit(&mut service, b"GET", &target);
    status(&peer, b"200");
    assert!(contains(peer.received(), b"GitHub sent a request, not an installation"));
    assert!(contains(peer.received(), b"href=\"/forges?first=1&amp;next=2\""));
    assert!(service.engine.as_ref().unwrap().setup.io.is_empty());
    assert!(service.engine.as_ref().unwrap().setup.backs.is_empty());
    let peer = visit(&mut service, b"GET", &target);
    status(&peer, b"410");
}
#[test]
fn setup_without_state_redirects_an_already_accepted_installation_to_forges() {
    let mut service = service(8, Duration::from_secs(3600));
    accepted(&mut service, &[22]);
    let peer = visit(&mut service, b"GET", b"/forges/1/github/setup?installation_id=22&setup_action=update");
    status(&peer, b"303");
    assert!(contains(peer.received(), b"Location: /forges"));
}
#[test]
fn setup_without_state_refuses_a_stray_installation() {
    let mut service = service(8, Duration::from_secs(3600));
    let peer = visit(&mut service, b"GET", b"/forges/1/github/setup?installation_id=22&setup_action=install");
    status(&peer, b"410");
}
#[test]
fn setup_malformed_queries_are_bad_requests() {
    for (path, query) in [
        (b"setup".as_slice(), b"".as_slice()),
        (b"setup", b"installation_id=0&setup_action=install"),
        (b"setup", b"installation_id=22&setup_action=other"),
        (b"setup", b"state=%GG&setup_action=request"),
        (b"setup", b"state=s&state=s&setup_action=request"),
        (b"manifest", b""),
        (b"manifest", b"state="),
        (b"manifest", b"state=%GG"),
        (b"manifest", b"state=s&state=s"),
        (b"manifest", b"state=s&code=c"),
        (b"redirect", b"state=s"),
        (b"redirect", b"code=c"),
        (b"redirect", b"code=&state=s"),
        (b"redirect", b"code=c&state="),
        (b"redirect", b"code=c&state=%00"),
        (b"redirect", b"code=c&code=d&state=s"),
    ] {
        let mut service = service(8, Duration::from_secs(3600));
        let target = joined(&[b"/forges/1/github/", path, b"?", query]);
        let peer = visit(&mut service, b"GET", &target);
        status(&peer, b"400");
    }
}
#[test]
fn setup_queries_past_the_configured_bound_are_bad_requests() {
    for path in [b"setup".as_slice(), b"manifest", b"redirect"] {
        let mut service = service(8, Duration::from_secs(3600));
        service.engine.as_mut().unwrap().clients.query_bytes = 1;
        let target = joined(&[b"/forges/1/github/", path, b"?setup_action=request&state=s"]);
        let peer = visit(&mut service, b"GET", &target);
        status(&peer, b"400");
    }
}
#[test]
fn setup_non_get_is_method_not_allowed() {
    for path in [b"setup".as_slice(), b"manifest", b"redirect"] {
        let mut service = service(8, Duration::from_secs(3600));
        let peer = visit(&mut service, b"POST", &joined(&[b"/forges/1/github/", path]));
        status(&peer, b"405");
    }
}
#[test]
fn setup_unconfigured_forge_is_not_found() {
    for path in [b"setup".as_slice(), b"manifest", b"redirect"] {
        let mut service = service(8, Duration::from_secs(3600));
        let peer = visit(&mut service, b"GET", &joined(&[b"/forges/2/github/", path]));
        status(&peer, b"404");
    }
}
#[test]
fn setup_forgejo_is_not_found() {
    for path in [b"setup".as_slice(), b"manifest", b"redirect"] {
        let mut service = service(8, Duration::from_secs(3600));
        service.engine.as_mut().unwrap().workers.forges[0].kind = crate::settings::Kind::Forgejo;
        let peer = visit(&mut service, b"GET", &joined(&[b"/forges/1/github/", path]));
        status(&peer, b"404");
    }
}

#[test]
fn restored_accepted_installations_reach_the_row_before_any_component_reply() {
    let mut service = routing_service(8, Duration::from_secs(3600));
    app(&mut service);
    accepted(&mut service, &[11, 22]);
    lifecycle::connect(service.engine.as_mut().unwrap(), 1);
    run_events(&mut service);
    let expected = domain::Installation { account: None, suspended: false, stray: false };
    assert_eq!(
        service.engine.as_ref().unwrap().domain.forges().rows[0].installations.as_ref(),
        [(11, expected.clone()), (22, expected.clone())]
    );
    assert_eq!(service.engine.as_ref().unwrap().setup.lifecycle.installations.get(&(1, 11)), Some(&expected));
}

#[test]
#[expect(clippy::disallowed_macros, reason = "scan captured public events and notes for the fake token")]
fn git_token_fills_a_restored_installations_account_without_exposing_its_value() {
    const TOKEN: &[u8] = b"fixture-secret-git-token";
    let mut service = routing_service(8, Duration::from_secs(3600));
    app(&mut service);
    accepted(&mut service, &[11]);
    lifecycle::connect(service.engine.as_mut().unwrap(), 1);
    run_events(&mut service);
    for login in [b"account-fixture".as_slice(), b"renamed-fixture".as_slice()] {
        service.engine.as_mut().unwrap().forge.above.push(connection::Event::GitToken {
            forge: 1,
            installation: 11,
            account: connection::Account { id: 111, login: Box::from(login) },
            token: Secret::new(Box::from(TOKEN)),
            expires: Wall::from_nanos(3_600_000_000_000),
        });
        crate::engine::forge_pass(service.engine.as_mut().unwrap());
        for event in &service.engine.as_ref().unwrap().events {
            assert!(!contains(alloc::format!("{event:?}").as_bytes(), TOKEN));
        }
        run_events(&mut service);
        let rows = service.engine.as_ref().unwrap().domain.forges().rows;
        assert_eq!(rows[0].installations[0].1.account, Some((111, Box::from(login))));
        assert!(!contains(alloc::format!("{rows:?}").as_bytes(), TOKEN));
    }
    for note in &service.engine.as_ref().unwrap().notes {
        assert!(!contains(alloc::format!("{note:?}").as_bytes(), TOKEN));
    }
    installation(&mut service, 11, Change::Suspended);
    run_events(&mut service);
    let rows = service.engine.as_ref().unwrap().domain.forges().rows;
    assert_eq!(rows[0].installations[0].1.account, Some((111, Box::from(b"account-fixture".as_slice()))));
    assert!(rows[0].installations[0].1.suspended);
}

fn manifest_link(service: &mut crate::Service, back: Option<&[u8]>) -> Box<[u8]> {
    public::manifest(service.engine.as_mut().unwrap(), 1, Token::new(92), back.map(Box::from));
    for _ in 0_u32..8 {
        advance(service.engine.as_mut().unwrap());
    }
    let mut link = None;
    while let Some(event) = service.engine.as_mut().unwrap().events.pop() {
        if let root::Event::Forge(event) = event {
            let domain::Event::ConnectAnswered { request, answer: domain::ConnectAnswer::Link(address), .. } = *event
            else {
                panic!("manifest link")
            };
            assert_eq!(request, Token::new(92));
            link = Some(address);
        }
    }
    link.expect("Manifest Done answers its page link")
}
fn joined(parts: &[&[u8]]) -> Box<[u8]> {
    let mut size = 0_usize;
    for part in parts {
        size = size.checked_add(part.len()).unwrap();
    }
    let mut writer = skein_lib::Writer::new(size);
    for part in parts {
        writer.put(part).unwrap();
    }
    writer.finish()
}
fn manifest_target(state: &[u8]) -> Box<[u8]> {
    joined(&[b"/forges/1/github/manifest?state=", state])
}
fn manifest_state(link: &[u8]) -> &[u8] {
    let start = skein_lib::bytes::find(link, b"state=").unwrap().checked_add(b"state=".len()).unwrap();
    link.get(start..).unwrap()
}
fn redirect_target(state: &[u8]) -> Box<[u8]> {
    let prefix = b"/forges/1/github/redirect?code=fixture-code&state=";
    let mut writer = skein_lib::Writer::new(prefix.len().checked_add(state.len()).unwrap());
    writer.put(prefix).unwrap();
    writer.put(state).unwrap();
    writer.finish()
}
#[test]
fn manifest_connect_answers_its_listener_page_with_policy_and_personal_form() {
    // fixtures::http::address is loopback:7100; the manifest's form is the forge's 3000.
    const PREFIX: &[u8] = b"http://127.0.0.1:7100/forges/1/github/manifest?state=";
    for (back, escaped) in [
        (
            Some(b"/forges?from=manifest&next=\"<>".as_slice()),
            b"href=\"/forges?from=manifest&amp;next=&quot;&lt;&gt;\"".as_slice(),
        ),
        (None, b"href=\"/forges/connect?forge=1\""),
    ] {
        let mut service = service(8, Duration::from_secs(3600));
        let link = manifest_link(&mut service, back);
        let state = manifest_state(&link);
        // skein-ui-web admits paths in every bundle; plaintext loopback needs loopback-navigation, absent in the shipped bundle.
        if back.is_some() {
            assert_eq!(link, manifest_target(state));
        } else {
            assert!(link.starts_with(PREFIX));
        }
        let peer = visit(&mut service, b"GET", &manifest_target(state));
        status(&peer, b"200");
        assert!(contains(peer.received(), b"Content-Type: text/html; charset=utf-8"));
        let policy = joined(&[
            b"Content-Security-Policy: default-src 'none'; form-action http://127.0.0.1:3000; script-src 'sha256-",
            MANIFEST_SCRIPT_HASH,
            b"'; style-src 'sha256-",
            MANIFEST_STYLE_HASH,
            b"'\r\n",
        ]);
        assert!(contains(peer.received(), &policy));
        assert!(contains(peer.received(), b"<button type=\"submit\">Continue to GitHub</button>"));
        assert!(contains(peer.received(), escaped));
        let target = joined(&[b"action=\"http://127.0.0.1:3000/settings/apps/new?state=", state, b"\""]);
        assert!(contains(peer.received(), &target));
        assert!(contains(peer.received(), b"method=\"post\""));
        assert!(service.engine.as_ref().unwrap().setup.runs.is_empty());
        let page = service.setup_page(1, state, Time::ZERO).unwrap();
        assert!(page.len() <= usize::try_from(service.engine.as_ref().unwrap().setup.env.limits.body_bytes).unwrap());
        measure_page(if back.is_some() { "manifest-held-back" } else { "manifest-fallback" }, page.len());
    }
}
#[test]
fn manifest_unknown_used_and_expired_pages_are_ended() {
    let mut service = service(8, Duration::from_secs(3));
    let link = manifest_link(&mut service, None);
    let state = manifest_state(&link);
    service.engine.as_mut().unwrap().env.now = Time::ZERO.saturating_add(Duration::from_secs(3));
    for state in [state, b"unknown"] {
        status(&visit(&mut service, b"GET", &manifest_target(state)), b"410");
    }
    assert!(service.engine.as_ref().unwrap().setup.io.is_empty());
}
fn redirect_writes() -> (crate::Service, Face, fixtures::http::Peer, Box<[u8]>) {
    let mut service = service(8, Duration::from_secs(3600));
    let link = manifest_link(&mut service, Some(b"/forges?from=manifest"));
    let target = redirect_target(manifest_state(&link));
    let peer = visit(&mut service, b"GET", &target);
    let face = conversion_replies(&mut service);
    (service, face, peer, target)
}
fn converted_redirect() -> (crate::Service, fixtures::http::Peer, Box<[u8]>) {
    let (mut service, mut face, peer, target) = redirect_writes();
    for item in [Item::AppId, Item::AppSlug, Item::AppKey, Item::User] {
        assert!(peer.received().is_empty(), "HTTP waits for durable records");
        keep_next(&mut service, item);
        face.turn(&mut service);
    }
    (service, peer, target)
}
#[test]
fn redirect_keeps_records_then_installs_and_hands_back_to_setup() {
    let (mut service, mut peer, target) = converted_redirect();
    assert!(peer.received().is_empty(), "HTTP waits for Origin");
    keep_next(&mut service, Item::Origin);
    pump_visit(&mut service, &mut peer);
    status(&peer, b"302");
    let prefix = b"Location: http://127.0.0.1:3000/apps/app-fixture/installations/new?state=";
    let start = skein_lib::bytes::find(peer.received(), prefix).unwrap().checked_add(prefix.len()).unwrap();
    let rest = &peer.received()[start..];
    let end = skein_lib::bytes::find(rest, b"\r\n").unwrap();
    let state = Box::<[u8]>::from(&rest[..end]);
    assert_ne!(state.as_ref(), manifest_state(&target));
    assert_eq!(service.engine.as_ref().unwrap().forge.starts.len(), 1);
    assert!(service.engine.as_ref().unwrap().events.iter().any(|event| if let root::Event::Forge(event) = event {
        event.as_ref() == &domain::Event::AppRecorded { forge: 1, app: domain::AppWay::Manifest }
    } else {
        false
    }));
    let again = visit(&mut service, b"GET", &target);
    status(&again, b"410");
    status(&visit(&mut service, b"GET", &manifest_target(manifest_state(&target))), b"410");
    let mut setup = visit(&mut service, b"GET", &setup_target(&state));
    complete_accept(&mut service, &mut setup);
    status(&setup, b"303");
    assert!(contains(setup.received(), b"Location: /forges?from=manifest"));
}
#[test]
fn redirect_conversion_stop_and_repeat_answer_ended() {
    let mut service = service(8, Duration::from_secs(3600));
    let link = manifest_link(&mut service, None);
    let target = redirect_target(manifest_state(&link));
    let mut peer = visit(&mut service, b"GET", &target);
    let mut face = Face::new();
    face.sent(&mut service);
    face.reply_status(&mut service, b"404 Not Found", b"{}");
    pump_visit(&mut service, &mut peer);
    status(&peer, b"410");
    // The shell takes diagnostic notes between calls; this routing fixture drives it by hand.
    while service.engine.as_mut().unwrap().notes.pop().is_some() {}
    status(&visit(&mut service, b"GET", &target), b"410");
    assert!(service.engine.as_ref().unwrap().forge.starts.is_empty());
}
#[test]
fn redirect_for_a_busy_forge_answers_ended() {
    let mut service = service(8, Duration::from_secs(3600));
    let link = manifest_link(&mut service, None);
    start(&mut service, 19, host::Plan::Install { forge: 1 });
    let peer = visit(&mut service, b"GET", &redirect_target(manifest_state(&link)));
    status(&peer, b"410");
}

#[test]
fn redirect_install_stop_answers_ended_with_the_app_kept() {
    let (mut service, mut peer, _) = converted_redirect();
    let engine = service.engine.as_mut().unwrap();
    let owner = *engine.setup.writes.first().unwrap().0;
    answer(engine, StoreEvent::Kept { owner });
    while engine.setup.above.pop().is_some() {}
    let run = *engine.setup.runs.first().unwrap().0;
    engine.setup.above.push(host::Event::Stop {
        run,
        failure: reconcile::Failure {
            forge: 1,
            step: reconcile::Stage::State,
            subject: Box::from(b"install".as_slice()),
            why: reconcile::Why::Crowded,
        },
    });
    advance(engine);
    pump_visit(&mut service, &mut peer);
    status(&peer, b"410");
    assert_eq!(service.engine.as_ref().unwrap().forge.starts.len(), 1);
}
#[test]
fn a_setup_visit_for_a_busy_forge_waits_for_its_run_then_accepts() {
    let mut service = service(8, Duration::from_secs(3600));
    app(&mut service);
    let state = install_link(&mut service, Some(b"/forges?from=manifest"));
    // The forge's one run, as the reconcile after a manifest's Connected.
    service.engine.as_mut().unwrap().setup.lifecycle.deleting.insert(1, ()).unwrap();
    let mut peer = visit(&mut service, b"GET", &setup_target(&state));
    assert!(peer.received().is_empty(), "the visit waits for the forge's run");
    assert!(!service.engine.as_ref().unwrap().setup.work_pending(Time::ZERO));
    service.engine.as_mut().unwrap().setup.lifecycle.deleting.remove(&1).unwrap();
    assert!(service.engine.as_ref().unwrap().setup.work_pending(Time::ZERO));
    pump_visit(&mut service, &mut peer);
    complete_accept(&mut service, &mut peer);
    status(&peer, b"303");
    assert!(contains(peer.received(), b"Location: /forges?from=manifest"));
}
#[test]
fn a_setup_visit_held_past_the_states_life_is_ended() {
    let mut service = service(8, Duration::from_secs(3));
    app(&mut service);
    let state = install_link(&mut service, None);
    service.engine.as_mut().unwrap().setup.lifecycle.deleting.insert(1, ()).unwrap();
    let mut peer = visit(&mut service, b"GET", &setup_target(&state));
    assert!(peer.received().is_empty(), "the visit waits for the forge's run");
    let until = service.engine.as_ref().unwrap().setup.next_deadline().unwrap();
    service.engine.as_mut().unwrap().env.now = until;
    pump_visit(&mut service, &mut peer);
    status(&peer, b"410");
}
// The forge's one run, as the engine's own reconcile (internal) or a run a person asked.
fn running(service: &mut crate::Service, internal: bool) -> Token {
    let token = Token::new(99);
    let run =
        Run { first: None, forge: 1, kind: Kind::Reconcile, internal, previous: domain::Connection::NotConnected };
    service.engine.as_mut().unwrap().setup.runs.insert(token, run).unwrap();
    token
}
fn install_answer(service: &mut crate::Service) -> Option<(domain::InstallAnswer, Option<Box<[u8]>>)> {
    let engine = service.engine.as_mut().unwrap();
    let mut found = None;
    while let Some(event) = engine.events.pop() {
        if let root::Event::Forge(event) = event
            && let domain::Event::InstallAnswered { request, answer, stopped, .. } = *event
        {
            assert_eq!(request, Token::new(91));
            found = Some((answer, stopped));
        }
    }
    found
}
#[test]
fn an_install_during_the_engines_own_run_answers_its_link_once_the_run_ends() {
    let mut service = service(8, Duration::from_secs(3600));
    app(&mut service);
    let reconcile = running(&mut service, true);
    public::install(service.engine.as_mut().unwrap(), 1, Token::new(91), None);
    for _ in 0_u32..8 {
        advance(service.engine.as_mut().unwrap());
    }
    assert_eq!(install_answer(&mut service), None, "the Install waits for the engine's run");
    assert_eq!(service.engine.as_ref().unwrap().setup.held.len(), 1);
    assert!(!service.engine.as_ref().unwrap().setup.work_pending(Time::ZERO));
    service.engine.as_mut().unwrap().setup.runs.remove(&reconcile).unwrap();
    assert!(service.engine.as_ref().unwrap().setup.work_pending(Time::ZERO));
    for _ in 0_u32..8 {
        advance(service.engine.as_mut().unwrap());
    }
    let Some((domain::InstallAnswer::Link(_), None)) = install_answer(&mut service) else { panic!("install link") };
    assert!(service.engine.as_ref().unwrap().setup.held.is_empty());
}
#[test]
fn an_install_during_a_persons_run_answers_busy() {
    let mut service = service(8, Duration::from_secs(3600));
    app(&mut service);
    running(&mut service, false);
    public::install(service.engine.as_mut().unwrap(), 1, Token::new(91), None);
    assert_eq!(
        install_answer(&mut service),
        Some((domain::InstallAnswer::Refused(domain::ForgeRefusal::Busy), None)),
        "a person's run refuses an Install at once"
    );
    assert!(service.engine.as_ref().unwrap().setup.held.is_empty());
}
#[test]
fn an_install_held_past_the_states_life_answers_busy() {
    let mut service = service(8, Duration::from_secs(3));
    app(&mut service);
    running(&mut service, true);
    public::install(service.engine.as_mut().unwrap(), 1, Token::new(91), None);
    advance(service.engine.as_mut().unwrap());
    assert_eq!(install_answer(&mut service), None, "the Install waits for the engine's run");
    service.engine.as_mut().unwrap().env.now = Time::ZERO.saturating_add(Duration::from_secs(3));
    advance(service.engine.as_mut().unwrap());
    let Some((domain::InstallAnswer::Connecting, Some(why))) = install_answer(&mut service) else { panic!("busy") };
    assert_eq!(why.as_ref(), b"setup host: busy");
    assert!(service.engine.as_ref().unwrap().setup.held.is_empty());
}

#[test]
fn pasted_app_key_form_and_401_failures_use_the_persons_words() {
    let mut service = service(8, Duration::from_secs(3600));
    let pem = b"not a PEM key";
    let mut bundle = skein_lib::Writer::new(pem.len() + 6);
    bundle.put(&[1, 1, 11, 0]).unwrap();
    bundle.put(&u16::try_from(pem.len()).unwrap().to_le_bytes()).unwrap();
    bundle.put(pem).unwrap();
    let bundle = Secret::new(bundle.finish());
    let digest = skein_lib::sha256::digest(bundle.expose());
    paste_start(&mut service, bundle, digest);
    for _ in 0_u32..4 {
        advance(service.engine.as_mut().unwrap());
    }
    assert_eq!(
        paste_ended(&mut service, false, pem).as_deref(),
        Some(b"The key isn't a GitHub App private key: paste its .pem file whole.".as_slice())
    );
    let mut service = super::service(8, Duration::from_secs(3600));
    let (pem, bundle) = carried_pem();
    let digest = skein_lib::sha256::digest(bundle.expose());
    paste_start(&mut service, bundle, digest);
    let mut face = Face::new();
    face.sent(&mut service);
    face.reply_status(&mut service, b"401 Unauthorized", b"{}");
    assert_eq!(
        paste_ended(&mut service, false, pem.expose()).as_deref(),
        Some(b"GitHub refused the App's id and key (401): check both are the same App's.".as_slice())
    );
}

// client/forge.md 9.1: QA observes the browser's status, shell and destination.
fn qa_ended(peer: &fixtures::http::Peer) {
    status(peer, b"410");
    assert!(contains(peer.received(), b"<h2>This link has ended</h2>"));
    assert!(contains(peer.received(), b"href=\"/forges/connect?forge=1\""));
    assert!(contains(peer.received(), b"style-src 'sha256-"));
    assert!(!contains(peer.received(), b"/welcome?"));
}

#[test]
fn qa_a_members_request_explains_ownership_and_consumes_its_back() {
    // F-m3-1: the fake has no member-request outcome; stage GitHub's GET.
    let mut service = service(8, Duration::from_secs(3600));
    app(&mut service);
    let state = install_link(&mut service, Some(b"/welcome?at=forge&forge=1"));
    let target = joined(&[b"/forges/1/github/setup?setup_action=request&state=", &state]);
    let peer = visit(&mut service, b"GET", &target);
    status(&peer, b"200");
    for words in [
        b"<h2>GitHub sent a request, not an installation</h2>".as_slice(),
        b"Only an owner of an organization can install",
        b"Install it on your own account, or on an organization you own.",
        b"href=\"/welcome?at=forge&amp;forge=1\"",
        b"style-src 'sha256-",
    ] {
        assert!(contains(peer.received(), words));
    }
    let again = visit(&mut service, b"GET", &target);
    qa_ended(&again);
    // The routing back is consumed, while the host's install state lives until
    // expiry (9.1). A later real installation has only the close-tab destination.
    let mut accepted = visit(&mut service, b"GET", &setup_target(&state));
    complete_accept(&mut service, &mut accepted);
    status(&accepted, b"200");
    assert!(contains(accepted.received(), b"<h2>GitHub is connected</h2>"));
    assert!(contains(accepted.received(), b"href=\"/forges\""));
}

#[test]
fn qa_an_accepted_link_ends_with_the_fallback_back() {
    let mut service = service(8, Duration::from_secs(3600));
    app(&mut service);
    let state = install_link(&mut service, Some(b"/welcome?at=forge&forge=1"));
    let target = setup_target(&state);
    let mut peer = visit(&mut service, b"GET", &target);
    complete_accept(&mut service, &mut peer);
    status(&peer, b"303");
    assert!(contains(peer.received(), b"Location: /welcome?at=forge&forge=1\r\n"));
    qa_ended(&visit(&mut service, b"GET", &target));
}

#[test]
fn qa_an_expired_link_ends_with_the_fallback_back() {
    let mut service = service(8, Duration::from_secs(3));
    app(&mut service);
    let state = install_link(&mut service, Some(b"/welcome?at=forge&forge=1"));
    service.engine.as_mut().unwrap().env.now = Time::ZERO.saturating_add(Duration::from_secs(3));
    qa_ended(&visit(&mut service, b"GET", &setup_target(&state)));
}

fn qa_uninstalled_store() -> crate::store::Store {
    let mut store = fixtures::records_store();
    for (item, value) in [
        (Item::AppId, Box::from(7_u64.to_le_bytes())),
        (Item::AppSlug, Box::from(b"app-fixture".as_slice())),
        (Item::AppKey, Box::from(skein_fake_forge_github::keys::KEYS[0])),
        (Item::User, Box::from(9_u64.to_le_bytes())),
    ] {
        kept_record(&mut store, 1, item, &value);
    }
    let config = github_config(8, Duration::from_secs(3600), None);
    let origin = records::origin(connection::settings::Kind::GitHub, &config.forges[0].api).encode();
    kept_record(&mut store, 1, Item::Origin, &origin);
    store
}
fn qa_restore(store: crate::store::Store) -> (crate::Service, Face) {
    let config = github_config(8, Duration::from_secs(3600), None);
    let mut service = local_service(config, store);
    load_routing(&mut service);
    let mut face = Face::new();
    // Hold the connection's fake socket before the shell restores the root.
    for _ in 0_u32..1024 {
        connection_turn(&mut service, &mut face);
    }
    restore_local(&mut service);
    (service, face)
}

#[test]
fn qa_a_link_forgotten_by_restart_ends_with_the_fallback_back() {
    let (mut before, _before_face) = qa_restore(qa_uninstalled_store());
    let state = install_link(&mut before, Some(b"/welcome?at=forge&forge=1"));
    let store = core::mem::replace(&mut before.engine.as_mut().unwrap().store, fixtures::records_store());
    drop(before);
    let (mut after, _after_face) = qa_restore(store);
    qa_ended(&visit(&mut after, b"GET", &setup_target(&state)));
}

#[test]
fn qa_restart_before_acceptance_restores_connecting_then_install_accept_and_reachable_work() {
    let (mut before, _before_face) = qa_restore(qa_uninstalled_store());
    let _abandoned = install_link(&mut before, Some(b"/welcome?at=forge&forge=1"));
    let store = core::mem::replace(&mut before.engine.as_mut().unwrap().store, fixtures::records_store());
    drop(before);
    let (mut after, mut connection) = qa_restore(store);
    assert!(contains(&connection.wire.received, b"GET /app/installations"));
    connection_reply(&mut after, &mut connection, b"[]");
    crate::engine::forge_pass(after.engine.as_mut().unwrap());
    run_events(&mut after);
    let rows = after.engine.as_ref().unwrap().domain.forges().rows;
    assert_eq!(rows.len(), 1);
    assert!(rows[0].installations.is_empty(), "no Ready installation survives an abandoned tab");
    // App connectivity with no accepted installation is the client's Connecting
    // step (client/forge.md 5.4), exercised by the joint client's abandoned-tab QA.
    assert_eq!(
        rows[0].connection,
        domain::Connection::Connected { id: 9, login: Box::from(b"app-fixture[bot]".as_slice()) }
    );
    // Let the restart's ordinary App reconcile finish before following Install.
    for _ in 0_u32..8 {
        advance(after.engine.as_mut().unwrap());
    }
    let mut reconcile = Face::new();
    reconcile.sent(&mut after);
    assert!(contains(&reconcile.wire.received, b"GET /app "));
    reconcile.reply(&mut after, br#"{"id":7,"slug":"app-fixture","permissions":{},"events":[]}"#);
    reconcile.sent(&mut after);
    reconcile.reply(&mut after, br#"{"url":"https://app-fixture.invalid/"}"#);
    reconcile.sent(&mut after);
    reconcile.reply(&mut after, b"[]");
    if !after.engine.as_ref().unwrap().setup.writes.is_empty() {
        keep_next(&mut after, Item::Accepted);
    }
    for _ in 0_u32..8 {
        advance(after.engine.as_mut().unwrap());
    }
    run_events(&mut after);
    let state = install_link(&mut after, Some(b"/welcome?at=forge&forge=1"));
    let mut peer = visit(&mut after, b"GET", &setup_target(&state));
    complete_accept(&mut after, &mut peer);
    status(&peer, b"303");
    assert!(contains(peer.received(), b"Location: /welcome?at=forge&forge=1\r\n"));
    public::reachable(after.engine.as_mut().unwrap(), 1, Token::new(93), Some(22), 1);
    let mut face = Face::new();
    face.sent(&mut after);
    face.reply(&mut after, br#"{"token":"ghs-fixture","expires_at":"2099-01-01T00:00:00Z"}"#);
    face.sent(&mut after);
    face.reply(&mut after, br#"{"total_count":1,"repositories":[{"id":31,"owner":{"id":122,"login":"account-fixture"},"name":"first","private":false}]}"#);
    let mut page = None;
    while let Some(event) = after.engine.as_mut().unwrap().events.pop() {
        if let root::Event::Forge(event) = event
            && let domain::Event::ReachableAnswered { answer: domain::ReachableAnswer::Page(answer), .. } = *event
        {
            page = Some(answer);
        }
    }
    let page = page.expect("Reachable after restart and acceptance");
    assert!(!page.more);
    assert_eq!(page.rows.len(), 1);
    assert_eq!(page.rows[0].name.as_ref(), b"first");
}

#[test]
fn qa_install_again_after_an_abandoned_tab_offers_a_fresh_state_and_accepts() {
    let mut service = service(8, Duration::from_secs(3600));
    app(&mut service);
    let abandoned = install_link(&mut service, Some(b"/welcome?at=forge&forge=1"));
    let retry = install_link(&mut service, Some(b"/welcome?at=forge&forge=1"));
    assert_ne!(abandoned, retry, "Install again gives a fresh browser link");
    let mut peer = visit(&mut service, b"GET", &setup_target(&retry));
    complete_accept(&mut service, &mut peer);
    status(&peer, b"303");
    assert!(contains(peer.received(), b"Location: /welcome?at=forge&forge=1\r\n"));
}

#[test]
fn an_empty_github_redirect_releases_the_connection_for_its_next_visit() {
    let mut service = service(8, Duration::from_secs(3600));
    accepted(&mut service, &[22]);
    // Chromium returns from GitHub on the connection that served its empty
    // redirect. Pipeline the next visit so no new connection can hide a stuck response.
    let requests = b"GET /forges/1/github/setup?installation_id=22&setup_action=update HTTP/1.1\r\nHost: temper.test\r\n\r\nGET /forges/no/github/setup HTTP/1.1\r\nHost: temper.test\r\nConnection: close\r\n\r\n";
    let mut peer = fixtures::http::Peer::raw(service.engine.as_mut().expect("engine"), requests);
    pump_visit(&mut service, &mut peer);
    status(&peer, b"404");
    assert!(contains(peer.received(), b"Forge not found"));
}
