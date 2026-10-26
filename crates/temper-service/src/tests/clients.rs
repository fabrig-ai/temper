use crate::clients::translation;
use jig_domain_core as core;
use jig_domain_people as people;
use jig_protocol_engine_clients as kit;
use skein_lib::{ReplyTo, Token};
use temper_engine_domain as root;

const LISTENER: Token = Token::new(77);
const STARTING_LISTENER: Token = Token::new(99);
const REPLY: ReplyTo = ReplyTo::new(Token::new(9));
const PROCESS_UPTIME: skein_lib::Time = skein_lib::Time::from_nanos(600_000_000_000);

#[test]
fn a_link_printed_on_the_first_clock_tick_is_live() {
    let mut service = super::fixtures::started();
    let engine = service.engine.as_mut().expect("engine");
    engine.clients.iterate(PROCESS_UPTIME, skein_lib::Wall::EPOCH, &mut engine.events, &[], None, &mut engine.accounts);
    let Some(skein_io::Request::Listen { owner, addr }) = engine.clients.io.pop() else { panic!("listen") };
    assert!(engine.clients.up(skein_io::Event::Listening { owner, listener: LISTENER, addr }));
    engine.clients.iterate(PROCESS_UPTIME, skein_lib::Wall::EPOCH, &mut engine.events, &[], None, &mut engine.accounts);
    let link = engine.clients.links.pop().expect("printed link");
    let body = peer::link_body(engine, &link);
    let mut peer = peer::Peer::new(engine, b"POST", b"/jig/v1/sign-in/link", &body, None);
    peer.pump_at(engine, false, PROCESS_UPTIME, skein_lib::Wall::EPOCH);
    assert_eq!(engine.clients.early.len(), 1, "the printed link reaches the root");
}
#[test]
fn a_chat_ask_reaches_the_root_as_a_party_ask() {
    let ask = core::Event::People(people::Event::Ask {
        models: Box::new([]),
        reply_to: REPLY,
        sign_in: 3,
        key: [5; 16],
        ask: super::fixtures::chat(1, b"question"),
    });
    let Ok(root::PartyInput::Ask { models: _, reply_to, sign_in, key, ask }) = translation::to_root(ask) else {
        panic!("party ask")
    };
    assert_eq!(reply_to, REPLY);
    assert_eq!(sign_in, 3);
    assert_eq!(key, [5; 16]);
    assert_eq!(ask, super::fixtures::chat(1, b"question"));
}
#[test]
fn whose_preserves_its_reply_right_and_sign_in() {
    let event = core::Event::People(people::Event::Whose { reply_to: REPLY, sign_in: 3 });
    let Ok(root::PartyInput::Whose { reply_to, sign_in }) = translation::to_root(event) else { panic!("party whose") };
    assert_eq!(reply_to, REPLY);
    assert_eq!(sign_in, 3);
}
#[test]
fn sign_out_preserves_its_reply_right_and_sign_in() {
    let event = core::Event::People(people::Event::SignOut { reply_to: REPLY, sign_in: 3 });
    let Ok(root::PartyInput::SignOut { reply_to, sign_in }) = translation::to_root(event) else {
        panic!("party sign out")
    };
    assert_eq!(reply_to, REPLY);
    assert_eq!(sign_in, 3);
}
#[test]
fn not_ready_uses_the_clients_reply_vocabulary() {
    for (row, input, expected) in [
        (
            "whose",
            root::PartyInput::Whose { reply_to: REPLY, sign_in: 3 },
            people::Reply::Again(people::Again::NotReady),
        ),
        (
            "sign out",
            root::PartyInput::SignOut { reply_to: REPLY, sign_in: 3 },
            people::Reply::Again(people::Again::NotReady),
        ),
        (
            "ask",
            root::PartyInput::Ask {
                models: Box::new([]),
                reply_to: REPLY,
                sign_in: 3,
                key: [5; 16],
                ask: super::fixtures::chat(1, b"question"),
            },
            people::Reply::Again(people::Again::NotReady),
        ),
    ] {
        let Some(kit::Event::Reply { to, reply }) = translation::not_ready(input) else {
            panic!("not-ready reply {row}")
        };
        assert_eq!(to, REPLY, "{row}");
        assert_eq!(reply, expected, "{row}");
    }
}
#[test]
fn a_released_reply_keeps_its_terminal_and_destination() {
    let request = root::PartyRequest::Deliver(root::Delivery::WebReply { to: REPLY, reply: people::Reply::SignedOut });
    let Some(kit::Event::Reply { to, reply }) = translation::to_component(request) else { panic!("clients reply") };
    assert_eq!(to, REPLY);
    assert_eq!(reply, people::Reply::SignedOut);
}

#[test]
fn the_component_prints_one_sign_in_link_at_start() {
    let mut clients = super::fixtures::clients();
    let mut events = skein_lib::Queue::with_capacity(64);
    clients.iterate(skein_lib::Time::ZERO, skein_lib::Wall::EPOCH, &mut events, &[], None, &mut None);
    let Some(skein_io::Request::Listen { owner, addr }) = clients.io.pop() else { panic!("listen") };
    assert!(clients.up(skein_io::Event::Listening { owner, listener: LISTENER, addr }));
    clients.iterate(skein_lib::Time::ZERO, skein_lib::Wall::EPOCH, &mut events, &[], None, &mut None);
    let link = clients.links.pop().unwrap();
    assert!(link.starts_with(b"http://127.0.0.1:7100/jig/v1/sign-in/link?secret="));
    assert!(clients.links.is_empty());
    clients.iterate(skein_lib::Time::ZERO, skein_lib::Wall::EPOCH, &mut events, &[], None, &mut None);
    assert!(clients.links.is_empty());
}
#[test]
fn closing_a_listener_during_start_closes_the_component() {
    let mut clients = super::fixtures::clients();
    let mut events = skein_lib::Queue::with_capacity(64);
    clients.iterate(skein_lib::Time::ZERO, skein_lib::Wall::EPOCH, &mut events, &[], None, &mut None);
    let Some(skein_io::Request::Listen { owner, addr }) = clients.io.pop() else { panic!("listen") };
    clients.down(kit::Event::Close);
    assert!(clients.up(skein_io::Event::Listening { owner, listener: LISTENER, addr }));
    clients.iterate(skein_lib::Time::ZERO, skein_lib::Wall::EPOCH, &mut events, &[], None, &mut None);
    let Some(skein_io::Request::Close { entity }) = clients.io.pop() else { panic!("late listener close") };
    assert_eq!(entity, LISTENER);
    assert!(clients.up(skein_io::Event::Closed { owner }));
    clients.iterate(skein_lib::Time::ZERO, skein_lib::Wall::EPOCH, &mut events, &[], None, &mut None);
    assert!(clients.closed);
}

#[test]
fn a_sign_in_digest_and_expiry_are_kept_in_secrets() {
    let mut clients = super::fixtures::clients();
    let mut store = super::fixtures::marked_store();
    clients.secrets.push(kit::SecretsRequest::Keep {
        owner: kit::SecretOwner::SignIn(3),
        value: kit::Kept {
            digest: skein_lib::Secret::new(Box::new([5; 32])),
            expires: skein_lib::Wall::from_nanos(12),
        },
    });
    clients.secrets_step(&mut store);
    store.iterate();
    let Some(jig_protocol_store::Event::Kept { owner }) = store.above.pop() else { panic!("digest kept") };
    assert_eq!(owner, Token::new(crate::tokens::CLIENTS + 1));
    let Some(jig_protocol_store::Event::Secret { value: Some(value), .. }) = store.secrets(
        Token::new(9),
        jig_protocol_store::Secrets::Get { owner: jig_protocol_store::SecretOwner::SignIn, number: 3 },
    ) else {
        panic!("secret read")
    };
    assert_eq!(value.expose().get(..8), Some([0, 0, 0, 0, 0, 0, 0, 12].as_slice()));
    assert_eq!(value.expose().get(8..), Some([5; 32].as_slice()));
}
#[test]
fn a_sign_in_ending_waits_for_its_erase_to_commit() {
    let config = super::fixtures::engine().jig;
    let mut journal = super::fixtures::engine().limits.root.journal;
    journal.commits = 1;
    let mut clients = crate::clients::Clients::new(
        &config,
        &super::fixtures::engine().clients,
        &super::fixtures::seeds(),
        64,
        &journal,
        None,
        &[],
    )
    .unwrap();
    clients.commit(3, &[root::Write::Erase(root::Key::Core(core::Key::People(people::Key::SignIn(7))))]);
    assert!(!clients.commit_room());
    let mut events = skein_lib::Queue::with_capacity(64);
    clients.iterate(skein_lib::Time::ZERO, skein_lib::Wall::EPOCH, &mut events, &[], None, &mut None);
    assert!(!clients.commit_room());
    clients.committed(3);
    clients.iterate(skein_lib::Time::ZERO, skein_lib::Wall::EPOCH, &mut events, &[], None, &mut None);
    assert!(clients.commit_room());
}

use super::fixtures::http as peer;
#[test]
fn the_next_clients_stream_opens_once_a_peer_ends_its_own() {
    // The small profile admits two streams; a third needs a predecessor freed.
    const CLIENTS: u32 = 3;
    let mut service = super::fixtures::ready();
    let link = service.links().pop().unwrap();
    let body = peer::link_body(service.engine.as_ref().expect("engine"), &link);
    let mut sign_in =
        peer::Peer::new(service.engine.as_mut().expect("engine"), b"POST", b"/jig/v1/sign-in/link", &body, None);
    sign_in.pump(service.engine.as_mut().expect("engine"), true);
    let jig_client_protocol_wire::v19::SignInAnswer::SignedIn(answer) =
        sign_in.sign_in_answer(service.engine.as_ref().expect("engine"))
    else {
        panic!("signed in")
    };
    let bearer = peer::bearer(answer.token().as_ref().expect("native credential").expose());
    let mut third_opened = false;
    for _ in 0..CLIENTS {
        let mut stream =
            peer::Peer::new(service.engine.as_mut().expect("engine"), b"GET", b"/jig/v1/stream", &[], Some(&bearer));
        third_opened = stream.read_hello(service.engine.as_mut().expect("engine"));
        stream.end(service.engine.as_mut().expect("engine"));
    }
    assert!(third_opened, "the third client receives its hello before a heartbeat");
}
#[test]
fn the_engine_prints_one_sign_in_link_at_start() {
    let mut service = super::fixtures::ready();
    let link = service.links().pop().expect("sign-in link");
    assert!(link.starts_with(b"http://127.0.0.1:7100/jig/v1/sign-in/link?secret="));
    assert!(service.links().is_empty());
    super::fixtures::turn(&mut service);
    assert!(service.links().is_empty());
}
#[test]
fn a_link_sign_in_reaches_the_root_and_answers_a_token() {
    let mut service = super::fixtures::ready();
    let link = service.links().pop().unwrap();
    let body = peer::link_body(service.engine.as_ref().expect("engine"), &link);
    let mut peer =
        peer::Peer::new(service.engine.as_mut().expect("engine"), b"POST", b"/jig/v1/sign-in/link", &body, None);
    peer.pump(service.engine.as_mut().expect("engine"), true);
    let jig_client_protocol_wire::v19::SignInAnswer::SignedIn(answer) =
        peer.sign_in_answer(service.engine.as_ref().expect("engine"))
    else {
        panic!("signed in")
    };
    assert_eq!(answer.party(), 1);
    assert!(super::fixtures::get(&mut service, core::Key::People(people::Key::SignIn(1))).is_some());
    let engine = service.engine.as_mut().expect("engine");
    let Some(jig_protocol_store::Event::Secret { value: Some(secret), .. }) = engine.store.secrets(
        Token::new(1),
        jig_protocol_store::Secrets::Get { owner: jig_protocol_store::SecretOwner::SignIn, number: 1 },
    ) else {
        panic!("digest durable in real secrets store")
    };
    assert_eq!(secret.expose().len(), 40);
    let bearer = peer::bearer(answer.token().as_ref().expect("native credential").expose());
    let mut stream = peer::Peer::new(engine, b"GET", b"/jig/v1/stream", &[], Some(&bearer));
    stream.pump(engine, true);
    assert!(stream.streaming(), "the issued bearer authenticates through the stored digest");
    assert_eq!(engine.end, None);
}
#[test]
fn a_request_before_decisions_open_is_answered_not_ready() {
    let mut service = super::fixtures::started();
    let engine = service.engine.as_mut().expect("engine");
    engine.clients.iterate(
        skein_lib::Time::ZERO,
        skein_lib::Wall::EPOCH,
        &mut engine.events,
        &engine.workers.forges,
        engine.demo.as_ref(),
        &mut engine.accounts,
    );
    let Some(skein_io::Request::Listen { owner, addr }) = engine.clients.io.pop() else { panic!("listener") };
    assert!(engine.clients.up(skein_io::Event::Listening { owner, listener: STARTING_LISTENER, addr }));
    engine.clients.iterate(
        skein_lib::Time::ZERO,
        skein_lib::Wall::EPOCH,
        &mut engine.events,
        &engine.workers.forges,
        engine.demo.as_ref(),
        &mut engine.accounts,
    );
    let link = engine.clients.links.pop().unwrap();
    let body = peer::link_body(engine, &link);
    let mut peer = peer::Peer::new(engine, b"POST", b"/jig/v1/sign-in/link", &body, None);
    peer.pump(engine, false);
    assert_eq!(engine.clients.early.len(), 1, "request staged during restoration");
    crate::engine::iterate(engine, skein_lib::Time::ZERO, skein_lib::Wall::EPOCH);
    peer.pump(engine, false);
    assert_eq!(peer.sign_in_answer(engine), jig_client_protocol_wire::v19::SignInAnswer::NotReady);
    assert!(!engine.domain.ready());
}
#[test]
fn a_clients_listener_failure_stops_the_engine() {
    let mut service = super::fixtures::started();
    service.told.push(skein_io::Event::Closed { owner: Token::new(crate::tokens::SERVER) });
    super::fixtures::turn(&mut service);
    assert_eq!(service.end(), Some(&crate::End::Stopped(crate::Stop::Listener)));
}

#[test]
fn a_failed_clients_bind_leaves_the_service_quiet() {
    failed_clients_bind(true, false);
}

#[test]
fn a_failed_clients_bind_before_store_open_leaves_the_service_quiet() {
    failed_clients_bind(false, false);
}

#[test]
fn a_closed_clients_bind_leaves_the_service_quiet() {
    failed_clients_bind(true, true);
}

fn failed_clients_bind(opened: bool, closed: bool) {
    use skein_lib::{Time, Wall};
    let mut service = super::fixtures::started();
    let engine = service.engine.as_mut().expect("engine");
    engine.store.opened = opened;
    engine.clients.iterate(Time::ZERO, Wall::EPOCH, &mut engine.events, &[], None, &mut engine.accounts);
    let Some(skein_io::Request::Listen { owner, .. }) = engine.clients.io.pop() else { panic!("clients listen") };
    service.told.push(if closed {
        skein_io::Event::Closed { owner }
    } else {
        skein_io::Event::Failed { owner, error: skein_io::Error::Refused }
    });
    for _ in 0_u32..64 {
        super::fixtures::turn(&mut service);
        if service.is_empty() {
            break;
        }
    }
    assert_eq!(service.end(), Some(&crate::End::Stopped(crate::Stop::Listener)));
    assert!(service.is_empty(), "every child and kernel route closes");
    assert!(!service.work_pending(Time::from_nanos(1)), "the loop needs no further turn");
    assert_eq!(service.next_deadline(), None, "no timer remains armed");
}

#[test]
fn original_model_names_reach_the_root() {
    let mut ask = super::fixtures::chat(1, b"question");
    let people::Ask::Create { batch, .. } = &mut ask else { panic!("fixture creates a chat") };
    batch[0].authority.models = Some(people::Models::Named(1));
    let event = core::Event::People(people::Event::Ask {
        reply_to: REPLY,
        sign_in: 3,
        key: [5; 16],
        ask,
        models: Box::new([Box::from(b"small".as_slice())]),
    });
    let Ok(root::PartyInput::Ask { models, .. }) = translation::to_root(event) else { panic!("party ask") };
    assert_eq!(models.as_ref(), &[Box::<[u8]>::from(b"small".as_slice())]);
}

#[test]
#[expect(clippy::disallowed_macros, reason = "ordinary test formats an HTTP request")]
fn clients_public_admits_the_public_and_listener_hosts_and_refuses_an_unknown_host() {
    for (host, status) in [
        ("temper.example.com:8443", "HTTP/1.1 404"),
        ("127.0.0.1:7100", "HTTP/1.1 404"),
        ("unknown.example.com", "HTTP/1.1 421"),
    ] {
        let mut config = super::fixtures::engine();
        config.clients.public = Some(crate::startup::application::Origin {
            scheme: crate::startup::application::Scheme::Https,
            host: Box::from(b"temper.example.com".as_slice()),
            port: Some(8443),
            name: Box::from(b"temper.example.com:8443".as_slice()),
        });
        let mut service = super::fixtures::started();
        let engine = service.engine.as_mut().unwrap();
        engine.clients = crate::clients::Clients::new(
            &config.jig,
            &config.clients,
            &super::fixtures::seeds(),
            64,
            &config.limits.root.journal,
            None,
            &[],
        )
        .unwrap();
        engine.clients.iterate(
            skein_lib::Time::ZERO,
            skein_lib::Wall::EPOCH,
            &mut engine.events,
            &[],
            None,
            &mut engine.accounts,
        );
        let Some(skein_io::Request::Listen { owner, addr }) = engine.clients.io.pop() else { panic!("listen") };
        assert!(engine.clients.up(skein_io::Event::Listening { owner, listener: LISTENER, addr }));
        let request = format!("GET /jig/unknown HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
        let mut peer = peer::Peer::raw(engine, request.as_bytes());
        peer.pump(engine, false);
        assert!(peer.received().starts_with(status.as_bytes()), "{host}: {:?}", peer.received());
    }
}

#[test]
fn a_moved_file_at_its_forms_bound_passes_and_one_byte_past_is_refused() {
    use jig_client_protocol_wire::v19 as w;
    use skein_lib::{Reader, Secret, Writer, bytes};

    let mut service = super::fixtures::ready();
    let link = service.links().pop().expect("sign-in link");
    let engine = service.engine.as_mut().expect("engine");
    let body = peer::link_body(engine, &link);
    let mut login = peer::Peer::new(engine, b"POST", b"/jig/v1/sign-in/link", &body, None);
    login.pump(engine, true);
    let w::SignInAnswer::SignedIn(answer) = login.sign_in_answer(engine) else { panic!("signed in") };
    let bearer = peer::bearer(answer.token().as_ref().expect("credential").expose());
    let settings = super::fixtures::engine();
    let bound = jig_protocol_accounts::moved::bound(&settings.jig.limits.driver.client.document);
    assert_eq!(bound, 68_252);
    assert_eq!(u64::from(engine.clients.env.limits.wire.keyed_value_some), bound);
    for extra in 0_u8..2 {
        let value = Secret::new(bytes::zeroed(usize::try_from(bound + u64::from(extra)).unwrap()));
        let digest = skein_lib::sha256::digest(value.expose());
        // Encode under the published ceiling to put the extra byte on the wire;
        // the service decodes under temper's configured, smaller value cap.
        let limits = w::CEILINGS;
        let ask = w::Ask::AccountChange(
            w::AskAccountChange::new(
                &limits,
                w::AskAccountChangeParts {
                    holder: w::AccountHolder::Own,
                    change: w::AccountChange::MoveIn(
                        w::AccountChangeMoveIn::new(&limits, w::AccountChangeMoveInParts { digest }).unwrap(),
                    ),
                },
            )
            .unwrap(),
        );
        let mut bytes = Writer::new(usize::try_from(ask.measure()).unwrap());
        ask.encode(&mut bytes).unwrap();
        let keyed = w::Keyed::new(
            &limits,
            w::KeyedParts {
                key: [extra; 16],
                owner: w::Owner::Jig,
                version: 19,
                request: bytes.finish(),
                value: Some(value),
            },
        )
        .unwrap();
        let mut bytes = Writer::new(usize::try_from(keyed.measure()).unwrap());
        keyed.encode(&mut bytes).unwrap();
        let mut call = peer::Peer::new(engine, b"POST", b"/jig/v1/request", &bytes.finish(), Some(&bearer));
        for _ in 0_u32..128 {
            call.pump(engine, true);
            if call.is_closed() {
                break;
            }
        }
        if extra == 0 {
            assert!(call.received().starts_with(b"HTTP/1.1 200"), "response: {:?}", call.received());
            let at = bytes::find(call.received(), b"\r\n\r\n").unwrap().checked_add(4).unwrap();
            let w::KeyedAnswer::FinalAnswer(answer) =
                w::KeyedAnswer::decode(&engine.clients.env.limits.wire, &mut Reader::new(&call.received()[at..]))
                    .unwrap()
            else {
                panic!("terminal move-in answer")
            };
            let w::Final::Refused(refused) =
                w::Final::decode(&engine.clients.env.limits.wire, &mut Reader::new(answer.record())).unwrap()
            else {
                panic!("the accounts refuse the opaque, invalid file")
            };
            let w::Refusal::AccountMove(why) = refused.refusal() else {
                panic!("the value passes both caps and reaches the accounts: {:?}", refused.refusal())
            };
            assert_eq!(why.why(), &w::AccountMoveWhy::NotOurs);
        } else {
            assert!(call.received().starts_with(b"HTTP/1.1 400"), "the wire refuses the extra byte");
            assert!(engine.events.is_empty(), "refused before reaching the root");
        }
    }
}
