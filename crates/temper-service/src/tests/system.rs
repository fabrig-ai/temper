use super::fixtures;
use crate::{End, Stop};
use skein_lib::{Time, Token, Wall};

#[test]
fn the_worker_dials_workers_and_its_hello_reaches_the_engine() {
    let mut stand = fixtures::one_process();
    fixtures::until_hosting(&mut stand);
    let hello = fixtures::hello(&stand.service).unwrap();
    assert_eq!(hello.0, Token::new(1));
    assert_eq!(hello.1.slots, 1);
    assert_eq!(hello.1.hosting.as_ref(), &[]);
    assert_eq!(stand.kernel.spawned, 1);
}
#[test]
fn the_worker_iterates_only_once_the_engine_listens() {
    let mut stand = fixtures::one_process();
    stand.service.engine.as_mut().expect("engine").store.opened = false;
    crate::iterate(&mut stand.service, Time::ZERO, Wall::EPOCH);
    assert!(!stand.service.worker.as_ref().unwrap().started, "store still closed");
    stand.service.engine.as_mut().expect("engine").store.opened = true;
    crate::iterate(&mut stand.service, Time::ZERO, Wall::EPOCH);
    assert!(!stand.service.worker.as_ref().unwrap().started, "listener still opening");
    for _ in 0_u8..4 {
        stand.turn();
    }
    assert!(stand.service.engine.as_ref().expect("engine").workers.opened());
    assert!(stand.service.worker.as_ref().unwrap().started);
}
#[test]
fn a_signal_drains_the_engine_then_ends_the_worker_at_once() {
    let mut stand = fixtures::one_process();
    fixtures::until_hosting(&mut stand);
    stand.service.shutdown();
    assert!(stand.service.worker.as_ref().unwrap().aborted.is_none());
    for _ in 0_u16..256 {
        stand.turn();
        if stand.service.is_empty() {
            break;
        }
    }
    assert_eq!(stand.service.end(), Some(&End::Drained));
    assert!(stand.service.is_empty(), "the worker ended with the engine");
    assert!(stand.kernel.root_closed);
}
#[test]
fn a_second_signal_aborts_both_layers() {
    let mut stand = fixtures::one_process();
    fixtures::until_hosting(&mut stand);
    stand.service.shutdown();
    stand.service.shutdown();
    assert_eq!(stand.service.end(), Some(&End::Aborted));
    assert!(stand.service.worker.as_ref().unwrap().aborted.is_some());
    for _ in 0_u16..256 {
        stand.turn();
        if stand.service.is_empty() {
            break;
        }
    }
    assert!(stand.service.is_empty());
}
#[test]
fn io_events_and_completions_reach_the_layer_whose_tokens_they_carry() {
    let mut stand = fixtures::one_process();
    fixtures::until_hosting(&mut stand);
    assert!(stand.service.worker.as_ref().unwrap().git.ready, "git completions reached the worker");
    assert_eq!(stand.kernel.files, 6);
    stand.service.told.push(skein_io::Event::Closed { owner: Token::new(crate::tokens::SERVER) });
    crate::iterate(&mut stand.service, Time::ZERO, Wall::EPOCH);
    assert_eq!(stand.service.end(), Some(&End::Stopped(Stop::Listener)), "clients event reached engine");
    assert!(stand.service.worker.as_ref().unwrap().aborted.is_some());
}
#[test]
fn an_unusable_git_stops_the_service_naming_its_program() {
    let mut stand = fixtures::one_process();
    stand.kernel.unusable = true;
    for _ in 0_u16..256 {
        stand.turn();
        if stand.service.is_empty() {
            break;
        }
    }
    let Some(End::Stopped(Stop::Start(problems))) = stand.service.end() else { panic!("git refuses the start") };
    assert_eq!(problems.problems.len(), 1);
    assert_eq!(
        problems.problems.get(0).unwrap().path,
        jig_protocol_config::Path::root()
            .member(b"application")
            .member(b"workers")
            .child(jig_protocol_config::Segment::Index(0))
            .member(b"git")
    );
    assert!(stand.service.is_empty());
}

#[test]
fn a_socket_workers_bind_announces_its_bound_address_once() {
    let mut service = fixtures::started();
    for _ in 0..service.notes().capacity() {
        service.notes().push(crate::Note::ForgeUnconfigured { number: 1 });
    }
    assert_eq!(service.notes().room(), 0, "forge diagnostics occupy their whole queue");
    let addr = ([127, 0, 0, 1], 41733).into();
    service.told.push(skein_io::Event::Listening {
        owner: Token::new(crate::tokens::WORKER_LISTENERS),
        listener: Token::new(77),
        addr: skein_io::Addr::Ip(addr),
    });
    crate::iterate(&mut service, Time::ZERO, Wall::EPOCH);
    assert_eq!(service.listening().pop(), Some(crate::Note::Listening { listener: crate::Listening::Workers, addr }));
    assert!(service.listening().is_empty());
    crate::iterate(&mut service, Time::ZERO, Wall::EPOCH);
    assert!(service.listening().is_empty());
    assert_eq!(service.notes().pop(), Some(crate::Note::ForgeUnconfigured { number: 1 }));
}

#[test]
fn an_in_process_workers_bind_has_no_listening_note() {
    let mut service = fixtures::started();
    service.told.push(skein_io::Event::Listening {
        owner: Token::new(crate::tokens::WORKER_LISTENERS),
        listener: Token::new(77),
        addr: skein_io::Addr::InProcess(skein_io::Name::new(b"workers").unwrap()),
    });
    crate::iterate(&mut service, Time::ZERO, Wall::EPOCH);
    assert!(service.listening().is_empty());
    crate::iterate(&mut service, Time::ZERO, Wall::EPOCH);
    assert!(service.listening().is_empty());
}

#[test]
fn a_clients_bind_announces_the_servers_bound_address_once() {
    let mut service = fixtures::started();
    let addr = ([127, 0, 0, 1], 41734).into();
    service.told.push(skein_io::Event::Listening {
        owner: Token::new(crate::tokens::SERVER),
        listener: Token::new(77),
        addr: skein_io::Addr::Ip(addr),
    });
    crate::iterate(&mut service, Time::ZERO, Wall::EPOCH);
    assert_eq!(service.listening().pop(), Some(crate::Note::Listening { listener: crate::Listening::Clients, addr }));
    assert!(service.listening().is_empty());
    crate::iterate(&mut service, Time::ZERO, Wall::EPOCH);
    assert!(service.listening().is_empty());
}

#[test]
fn standalone_announces_clients_without_announcing_in_process_workers() {
    let mut stand = fixtures::one_process();
    fixtures::until_hosting(&mut stand);
    let addr = fixtures::engine().clients.listener;
    let jig_protocol_config::Address::Addr(addr) = addr else { panic!("clients socket") };
    let addr = addr.socket().unwrap();
    assert_eq!(
        stand.service.listening().pop(),
        Some(crate::Note::Listening { listener: crate::Listening::Clients, addr })
    );
    assert!(stand.service.listening().is_empty());
    stand.turn();
    assert!(stand.service.listening().is_empty());
}

#[test]
fn the_box_pipes_carry_no_service_data_and_close_on_drain_or_abort() {
    use skein_io::kernel::Fd;
    for abort in [false, true] {
        let mut stand = fixtures::one_process();
        stand.service.adopt_box(Fd::new(3), Fd::new(4), 128).unwrap();
        assert_eq!(stand.service.adopt_box(Fd::new(5), Fd::new(6), 128), Err(Fd::new(5)));
        fixtures::until_hosting(&mut stand);
        let mut pending_reads = 0_u32;
        for submit in &stand.service.submissions {
            if let skein_io::kernel::Op::PipeRead { fd, .. } = &submit.kind
                && *fd == Fd::new(4)
            {
                pending_reads = pending_reads.checked_add(1).unwrap();
            }
        }
        assert_eq!(pending_reads, 1, "io's read is still pending; nothing reached the service");
        assert!(!stand.kernel.box_read_cancelled);
        assert_eq!(stand.kernel.box_writes, 0);
        assert!(!stand.kernel.closed.as_slice().contains(&Fd::new(3)));
        assert!(!stand.kernel.closed.as_slice().contains(&Fd::new(4)));
        stand.service.shutdown();
        if abort {
            stand.service.shutdown();
        }
        for _ in 0_u16..256 {
            stand.turn();
            if stand.service.is_empty() {
                break;
            }
        }
        assert_eq!(stand.service.end(), Some(if abort { &End::Aborted } else { &End::Drained }));
        assert!(stand.service.is_empty());
        assert!(stand.kernel.box_read_cancelled, "close cancelled io's pending read");
        for note in stand.service.notes().iter() {
            if let crate::Note::BoxLink { line } = note {
                panic!("our close is not the box closing: {line:?}");
            }
        }
        assert_eq!(stand.kernel.box_writes, 0);
        for fd in [Fd::new(3), Fd::new(4)] {
            let mut count = 0_u32;
            for closed in &stand.kernel.closed {
                if *closed == fd {
                    count = count.checked_add(1).unwrap();
                }
            }
            assert_eq!(count, 1);
        }
    }
}

#[test]
fn a_scripted_box_answers_keyed_setup_requests_through_the_clients_family() {
    use skein_io::kernel::Fd;
    use temper_client_protocol_wire::v1 as w;
    use temper_engine_domain_forge::{Connection, SetUpStep};
    const OWNED: u16 = 1;
    const OTHER: u16 = 2;
    // Allow the root continuations and memory-store handoffs, then output credit and IO.
    fn turns(stand: &mut fixtures::system::SystemStand) {
        const TURNS: u8 = 64;
        for _ in 0..TURNS {
            stand.turn();
        }
    }
    const LINE_BYTES: u32 = fixtures::system::BOX_BYTES;
    let mut stand = fixtures::system::one_process_under_box(true);
    stand.service.adopt_box(Fd::new(3), Fd::new(4), LINE_BYTES).unwrap();
    fixtures::until_hosting(&mut stand);
    // SystemStand has fixed membership and an in-memory store. Startup's
    // absent-file state is tested separately; inject its root inputs here.
    for forge in [OWNED, OTHER] {
        crate::setup::lifecycle::state(stand.service.engine.as_mut().unwrap(), forge, Connection::NotSetUp);
    }
    turns(&mut stand);
    let bearer = box_sign_in(&mut stand);
    assert_eq!(box_setup(&mut stand, &bearer, OWNED, 1), w::Outcome::SettingUp);
    turns(&mut stand);
    assert_eq!(stand.kernel.box_written.as_slice(), b"start 1\n");
    assert_eq!(
        stand.service.engine.as_ref().unwrap().domain.forge_connection(OWNED),
        Connection::SettingUp { step: None }
    );

    for byte in b"step 1 prepare\n" {
        stand.kernel.box_answers.push(*byte);
    }
    turns(&mut stand);
    assert_eq!(
        stand.service.engine.as_ref().unwrap().domain.forge_connection(OWNED),
        Connection::SettingUp { step: Some(SetUpStep::Prepare) }
    );
    assert_eq!(box_setup(&mut stand, &bearer, OWNED, 1), w::Outcome::SettingUp);
    assert_eq!(stand.kernel.box_written.as_slice(), b"start 1\n");

    for byte in b"failed 1 prepare 1 exited 1\n" {
        stand.kernel.box_answers.push(*byte);
    }
    turns(&mut stand);
    assert_eq!(
        stand.service.engine.as_ref().unwrap().domain.forge_connection(OWNED),
        Connection::SetUpFailed { why: b"prepare 1 exited 1".as_slice().into() }
    );
    assert_eq!(stand.kernel.box_written.as_slice(), b"start 1\n");

    assert_eq!(box_setup(&mut stand, &bearer, OWNED, 2), w::Outcome::SettingUp);
    turns(&mut stand);
    assert_eq!(stand.kernel.box_written.as_slice(), b"start 1\nstart 1\n");
    assert_eq!(
        stand.service.engine.as_ref().unwrap().domain.forge_connection(OWNED),
        Connection::SettingUp { step: None }
    );

    let w::Outcome::SetUpRefused(refused) = box_setup(&mut stand, &bearer, OTHER, 3) else { panic!("busy answer") };
    assert_eq!(*refused.why(), w::SetUpRefusal::Busy);
    turns(&mut stand);
    assert_eq!(stand.kernel.box_written.as_slice(), b"start 1\nstart 1\n");
    assert_eq!(
        stand.service.engine.as_ref().unwrap().domain.forge_connection(OWNED),
        Connection::SettingUp { step: None }
    );
    assert_eq!(stand.service.engine.as_ref().unwrap().domain.forge_connection(OTHER), Connection::NotSetUp);
    assert!(stand.service.notes().is_empty());

    for byte in b"started 9\n" {
        stand.kernel.box_answers.push(*byte);
    }
    turns(&mut stand);
    assert_eq!(stand.kernel.box_written.as_slice(), b"start 1\nstart 1\n");
    assert_eq!(
        stand.service.engine.as_ref().unwrap().domain.forge_connection(OWNED),
        Connection::SettingUp { step: None }
    );
    assert_eq!(
        stand.service.notes().pop(),
        Some(crate::Note::BoxLink { line: b"box link: dropped started for forge 9, not asked".as_slice().into() })
    );
    assert!(stand.service.notes().is_empty());
}

// The system story uses the actual clients component: an HTTP family request
// crosses the core's key admission, and FromApplication::Answered returns its bytes.
fn box_sign_in(stand: &mut fixtures::system::SystemStand) -> Box<[u8]> {
    use fixtures::http as peer;
    let link = stand.service.links().pop().expect("sign-in link");
    let engine = stand.service.engine.as_mut().unwrap();
    let body = peer::link_body(engine, &link);
    let mut sign_in = peer::Peer::new(engine, b"POST", b"/jig/v1/sign-in/link", &body, None);
    sign_in.pump(engine, true);
    let jig_client_protocol_wire::v19::SignInAnswer::SignedIn(answer) = sign_in.sign_in_answer(engine) else {
        panic!("deployment owner signed in")
    };
    peer::bearer(answer.token().as_ref().expect("native credential").expose())
}
fn box_setup(
    stand: &mut fixtures::system::SystemStand,
    bearer: &[u8],
    forge: u16,
    key: u8,
) -> temper_client_protocol_wire::v1::Outcome {
    use fixtures::http as peer;
    use jig_client_protocol_wire::v19 as j;
    use skein_lib::{Reader, Writer};
    use temper_client_protocol_wire::{self as wire, v1 as w};
    let engine = stand.service.engine.as_mut().unwrap();
    let limits = &engine.clients.env.limits.wire;
    let request = w::Requesting::new(
        &w::CEILINGS,
        w::RequestingParts {
            request: w::Request::SetUp(
                w::SetUp::new(
                    &w::CEILINGS,
                    w::SetUpParts {
                        playground: false,
                        forge,
                        org: b"work".as_slice().into(),
                        repository: b"hello".as_slice().into(),
                        project: w::SetUpProject::Existing(
                            w::ExistingProject::new(&w::CEILINGS, w::ExistingProjectParts { project: 1 })
                                .expect("project"),
                        ),
                    },
                )
                .unwrap(),
            ),
        },
    )
    .unwrap();
    let family = j::FamilyRequest::new(
        limits,
        j::FamilyRequestParts { scope: j::Scope::Deployment, record: wire::requesting(&request) },
    )
    .unwrap();
    let mut bytes = Writer::new(usize::try_from(family.measure()).unwrap());
    family.encode(&mut bytes).unwrap();
    let request = j::Keyed::new(
        limits,
        j::KeyedParts {
            key: [key; 16],
            owner: j::Owner::Family(j::Family::new(limits, j::FamilyParts { number: wire::FORGE }).unwrap()),
            version: wire::CLIENT_VERSION,
            request: bytes.finish(),
            value: None,
        },
    )
    .unwrap();
    let mut bytes = Writer::new(usize::try_from(request.measure()).unwrap());
    request.encode(&mut bytes).unwrap();
    let mut client = peer::Peer::new(engine, b"POST", b"/jig/v1/request", &bytes.finish(), Some(bearer));
    client.pump(engine, true);
    let at = skein_lib::bytes::find(client.received(), b"\r\n\r\n").unwrap().checked_add(4).unwrap();
    let mut reader = Reader::new(client.received().get(at..).unwrap());
    let j::KeyedAnswer::FinalAnswer(answer) =
        j::KeyedAnswer::decode(&engine.clients.env.limits.wire, &mut reader).unwrap()
    else {
        panic!("one terminal family answer")
    };
    assert!(reader.is_empty());
    assert_eq!(
        answer.version(),
        wire::CLIENT_VERSION,
        "core terminal for setup: {:?}",
        j::Final::decode(&engine.clients.env.limits.wire, &mut Reader::new(answer.record()))
    );
    assert_eq!(
        *answer.owner(),
        j::Owner::Family(
            j::Family::new(&engine.clients.env.limits.wire, j::FamilyParts { number: wire::FORGE }).unwrap()
        )
    );
    let mut reader = Reader::new(answer.record());
    let answer = w::Answer::decode(&w::CEILINGS, &mut reader).unwrap();
    assert!(reader.is_empty());
    answer.into_parts().outcome
}

#[test]
fn a_new_client_after_idle_gets_its_whole_stall_period() {
    use skein_lib::Duration;
    const STALL: Duration = Duration::from_secs(30);
    const BEGAN: Time = Time::from_nanos(1_000_000_000);
    for idle in [29, 30, 60, 100] {
        let mut service = fixtures::started();
        fixtures::listening(&mut service);
        crate::iterate(&mut service, BEGAN, Wall::EPOCH);
        let accepted = BEGAN.saturating_add(Duration::from_secs(idle));
        service.told.push(skein_io::Event::Accepted {
            owner: Token::new(crate::tokens::SERVER),
            socket: fixtures::http::SOCKET,
            peer: fixtures::http::address(),
        });
        crate::iterate(&mut service, accepted, Wall::EPOCH);
        assert_eq!(
            service.engine.as_ref().unwrap().clients.next_deadline(),
            Some(accepted.saturating_add(STALL)),
            "new connection after {idle} seconds idle gets its deadline at accept"
        );
    }
}

#[test]
fn a_new_webhook_after_idle_gets_its_whole_delivery_period() {
    use skein_lib::Duration;
    const BEGAN: Time = Time::from_nanos(1_000_000_000);
    const IDLE: Duration = Duration::from_secs(60);
    const LISTENER: Token = Token::new(1_u64 << 59);
    let mut service = fixtures::ready();
    let config = fixtures::engine();
    let mut found = jig_protocol_config::Problems::empty();
    let mut settings = crate::forge::settings(
        &config.forges,
        fixtures::forge_runtime(),
        Some(skein_forge_connection::hooks::Webhooks { listener: fixtures::http::address() }),
        &mut found,
    )
    .unwrap();
    for at in 0..settings.forges.len() {
        settings.forges.get_mut(at).unwrap().access = None;
    }
    let layer = &mut service.engine.as_mut().unwrap().forge;
    layer.component = skein_forge_connection::Component::new(
        settings,
        &layer.env.limits,
        &crate::forge::shared(&config.limits, &config.jig.limits.io),
    )
    .unwrap();
    layer.component.begin(&layer.env, &mut layer.above, &mut layer.io);
    layer.up(skein_io::Event::Listening { owner: LISTENER, listener: Token::new(88), addr: fixtures::http::address() });
    while layer.io.pop().is_some() {}
    let delivery = layer.env.limits.hook_deadline;
    crate::iterate(&mut service, BEGAN, Wall::EPOCH);
    let accepted = BEGAN.saturating_add(IDLE);
    service.told.push(skein_io::Event::Accepted {
        owner: Token::new(crate::tokens::FORGE | LISTENER.raw()),
        socket: Token::new(89),
        peer: fixtures::http::address(),
    });
    crate::iterate(&mut service, accepted, Wall::EPOCH);
    assert_eq!(
        service.engine.as_ref().unwrap().forge.component.next_deadline(),
        Some(accepted.saturating_add(delivery)),
        "the webhook starts its delivery period at accept"
    );
}
#[test]
fn clients_claim_a_listener_adopted_before_the_first_down() {
    use skein_io::kernel::{Fd, Op};
    let mut stand = fixtures::one_process();
    let jig_protocol_config::Address::Addr(addr) = fixtures::engine().clients.listener else { panic!("clients IP") };
    let addr = addr.socket().unwrap();
    let fd = Fd::new(8);
    stand.service.adopt_listener(fd, addr).unwrap();
    assert_eq!(stand.service.adopt_listener(Fd::new(7), addr), Err(Fd::new(7)), "duplicate stays the caller's");
    let mut accepted = false;
    for _ in 0_u16..256 {
        crate::iterate(&mut stand.service, Time::ZERO, Wall::EPOCH);
        for submit in &stand.service.submissions {
            if let Op::Bind { addr: bound, .. } = &submit.kind {
                assert_ne!(*bound, addr, "clients must not bind again");
            }
            if let Op::Listen { fd: listening, .. } = &submit.kind {
                assert_ne!(*listening, fd, "already listening");
            }
            if let Op::Accept { fd: listening, .. } = &submit.kind {
                accepted |= *listening == fd;
            }
        }
        stand.kernel.answer(&mut stand.service);
        if accepted {
            break;
        }
    }
    assert!(accepted, "clients accept on the inherited descriptor");
    assert_eq!(
        stand.service.listening().pop(),
        Some(crate::Note::Listening { listener: crate::Listening::Clients, addr })
    );
    stand.service.shutdown();
    for _ in 0_u16..256 {
        stand.turn();
        if stand.service.is_empty() {
            break;
        }
    }
    assert!(stand.service.is_empty());
    let mut closed = 0_u32;
    for descriptor in &stand.kernel.closed {
        if *descriptor == fd {
            closed = closed.checked_add(1).unwrap();
        }
    }
    assert_eq!(closed, 1);
}
