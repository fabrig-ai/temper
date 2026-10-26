use super::fixtures;
use crate::{End, Stop, forge, limits};
use skein_forge_connection::{StartRead, Stopped};
use skein_lib::{Duration, Time, Token};
use temper_engine_domain as root;
#[test]
fn a_stopped_forge_is_named_and_only_its_calls_are_not_connected() {
    use skein_forge::api::{Answer, Error, Op, Read, Repository};
    let mut service = fixtures::started_with_two_forges();
    let mut face = fixtures::ForgeFace::new();
    face.connected(&mut service);
    face.answer(&mut service, b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n");
    assert_eq!(service.end(), None);
    assert!(
        service.engine.as_ref().expect("engine").drain.is_none(),
        "a stopped forge never starts the engine's drain"
    );
    let acts = temper_engine_domain_forge::demo_act_bound(&service.engine.as_ref().unwrap().env.limits.forge).unwrap();
    assert_eq!(service.notes().capacity(), 2 + acts);
    assert_eq!(
        service.notes().pop(),
        Some(crate::Note::ForgeStopped { number: 1, why: Stopped::Refused { read: StartRead::User, status: 401 } })
    );
    assert!(service.notes().is_empty());
    face.select(2);
    face.answer(&mut service, &forge_ok(br#"{"id":42,"login":"writer"}"#));
    face.answer(&mut service, &forge_ok(br#"{"max_response_items":2,"default_paging_num":30}"#));
    face.capture_answers(); // Scripted calls have no outstanding client token in the root.
    for (number, call) in [(1, Token::new(7)), (2, Token::new(8))] {
        crate::route::request(
            service.engine.as_mut().expect("engine"),
            root::Request::Forge {
                call,
                repository: Repository { forge: number, repository: 0 },
                op: Op::Read(Read::Locate {
                    owner: Box::from(b"fixture".as_slice()),
                    name: Box::from(b"specimen".as_slice()),
                }),
            },
        );
    }
    face.answer(&mut service, &forge_ok(br#"{"id":2,"owner":{"id":42,"login":"fixture"},"name":"specimen","object_format_name":"sha1","default_branch":"main","default_merge_style":"merge","allow_merge_commits":true,"allow_squash_merge":false,"allow_rebase":false,"html_url":"http://127.0.0.1:3001/fixture/specimen"}"#));
    let answers = face.take_answers();
    assert_eq!(
        answers.as_ref(),
        [
            skein_forge_connection::Answered {
                free: 0,
                rate: None,
                call: Token::new(7),
                cost: 0,
                result: Err(Error::NotConnected)
            },
            skein_forge_connection::Answered {
                free: 0,
                rate: None,
                call: Token::new(8),
                cost: 1,
                result: Ok(Answer::Located { repository: 2 })
            },
        ]
    );
    assert_eq!(service.end(), None);
    assert!(service.engine.as_ref().expect("engine").drain.is_none());
    assert!(service.notes().is_empty());
}
fn forge_ok(body: &[u8]) -> Box<[u8]> {
    const HEAD: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: ";
    let length = skein_lib::Decimal::of(u64::try_from(body.len()).unwrap());
    let size = HEAD
        .len()
        .checked_add(length.as_bytes().len())
        .unwrap()
        .checked_add(4)
        .unwrap()
        .checked_add(body.len())
        .unwrap();
    let mut response = skein_lib::Writer::new(size);
    response.put(HEAD).unwrap();
    response.put(length.as_bytes()).unwrap();
    response.put(b"\r\n\r\n").unwrap();
    response.put(body).unwrap();
    response.finish()
}
#[test]
fn a_forge_call_waiting_past_its_deadline_reaches_the_root_unavailable() {
    let mut service = fixtures::started();
    let call = Token::new(7);
    let mut face = fixtures::ForgeFace::new();
    face.at(&mut service, Time::from_nanos(1));
    face.connected(&mut service);
    face.refuse(&mut service);
    crate::route::request(
        service.engine.as_mut().expect("engine"),
        root::Request::Forge {
            call,
            repository: skein_forge::api::Repository { forge: 1, repository: 2 },
            op: skein_forge::api::Op::Read(skein_forge::api::Read::Settings),
        },
    );
    service.engine.as_mut().expect("engine").forge.env.now = Time::from_nanos(11_000_000_000);
    let engine = service.engine.as_mut().expect("engine");
    engine.forge.component.fire(&engine.forge.env, &mut engine.forge.above, &mut engine.forge.io);
    let Some(skein_forge_connection::Event::Answered(answer)) =
        service.engine.as_mut().expect("engine").forge.above.pop()
    else {
        panic!("call terminal")
    };
    assert_eq!(
        answer,
        skein_forge_connection::Answered {
            free: 0,
            rate: None,
            call,
            cost: 0,
            result: Err(skein_forge::api::Error::Unavailable)
        }
    );
    let root::Event::Forge(event) = forge::event(answer) else {
        panic!("forge event");
    };
    assert_eq!(forge::back(event).unwrap().call, call);
    assert_eq!(service.engine.as_ref().expect("engine").forge.above.len(), 0);
}
#[test]
fn an_answer_the_root_cannot_take_is_offered_again_next_pass() {
    let mut service = fixtures::ready();
    for number in 2..16 {
        let engine = service.engine.as_mut().expect("engine");
        if root::try_step(&mut engine.domain, &engine.env, fixtures::period(number)).is_err() {
            break;
        }
    }
    let answer = skein_forge_connection::Answered {
        free: 0,
        rate: None,
        call: Token::new(7),
        cost: 0,
        result: Err(skein_forge::api::Error::Unavailable),
    };
    let engine = service.engine.as_mut().expect("engine");
    assert!(root::try_step(&mut engine.domain, &engine.env, forge::event(answer.clone())).is_err());
    service.engine.as_mut().expect("engine").forge.above.push(skein_forge_connection::Event::Answered(answer.clone()));
    crate::engine::forge_pass(service.engine.as_mut().expect("engine"));
    assert_eq!(service.engine.as_ref().expect("engine").forge.above.len(), 0);
    service.engine.as_mut().expect("engine").forge.down(skein_forge_connection::Request::Offer);
    let Some(skein_forge_connection::Event::Answered(offered)) =
        service.engine.as_mut().expect("engine").forge.above.pop()
    else {
        panic!("offered answer")
    };
    assert_eq!(offered, answer);
}
#[test]
fn an_answer_returned_in_the_drain_is_never_offered_again() {
    let mut service = fixtures::ready();
    service.shutdown();
    fixtures::ForgeFace::returned(
        &mut service,
        skein_forge_connection::Answered {
            free: 0,
            rate: None,
            call: Token::new(7),
            cost: 0,
            result: Err(skein_forge::api::Error::Unavailable),
        },
    );
    for _ in 0_u32..4 {
        service.engine.as_mut().expect("engine").forge.down(skein_forge_connection::Request::Offer);
        assert_eq!(fixtures::ForgeFace::pending(&service), 0);
    }
    for _ in 0_u32..64 {
        fixtures::turn(&mut service);
    }
    assert_eq!(service.end(), Some(&End::Drained));
    assert!(service.engine.as_ref().expect("engine").forge.closed);
    assert_eq!(service.engine.as_ref().expect("engine").forge.above.len(), 0);
}
#[test]
fn routing_news_and_hints_returned_during_the_drain_are_dropped() {
    use temper_engine_domain_forge::{Connection, Event, SetUpAnswer};
    for event in [
        Event::Hint {
            hint: skein_forge::api::Hint {
                repository: skein_forge::api::Repository { forge: 1, repository: 2 },
                change: skein_forge::api::Change::Item(7),
                key: None,
            },
        },
        Event::Connection { forge: 1, state: Connection::Connected { id: 42, login: Box::from(b"temper".as_slice()) } },
        Event::BoxClosed,
        Event::SetUpAnswered { forge: 1, request: Token::new(91), answer: SetUpAnswer::SettingUp },
    ] {
        let mut service = fixtures::ready();
        service.shutdown();
        // The routing stops producing hints during a drain; exercise the
        // returned root boundary for every kind of volatile news here too.
        service.engine.as_mut().expect("engine").events.push(root::Event::Forge(Box::new(event)));
        for _ in 0_u32..256 {
            fixtures::turn(&mut service);
            if service.end().is_some() {
                break;
            }
        }
        assert_eq!(service.end(), Some(&End::Drained));
        assert_eq!(
            service.engine.as_ref().expect("engine").forge.returned,
            0,
            "no news or hint is returned to the component"
        );
        assert!(service.engine.as_ref().expect("engine").forge.closed);
    }
}
#[test]
fn forge_component_limits_that_disagree_are_problems_naming_both() {
    let rows: [(skein_forge_connection::Unusable, &[u8]); 3] = [
        (skein_forge_connection::Unusable::Connections { connections: 3, calls: 2 }, b"application.limits.overrides.forge_component.connections: past its bound\napplication.limits.overrides.forge.client.calls: past its bound\n"),
        (skein_forge_connection::Unusable::Deadline { deadline: Duration::from_secs(11), lifetime: Duration::from_secs(10) }, b"application.limits.overrides.forge_component.deadline_ms: past its bound\napplication.limits.overrides.forge.client.lifetime_ms: past its bound\n"),
        (skein_forge_connection::Unusable::BodyBytes { body_bytes: 1, needed: 1045 }, b"application.limits.overrides.forge_component.body_bytes: past its bound\napplication.limits.overrides.forge.client.op_bytes: past its bound\n"),
    ];
    for (refusal, expected) in rows {
        let mut found = jig_protocol_config::Problems::empty();
        forge::problems(refusal, &mut found);
        assert_eq!(
            crate::startup::lines(&crate::startup::Problems::jig(found, &jig_protocol_config::Path::root())).as_ref(),
            expected
        );
    }
}
#[test]
fn forge_component_overrides_take_dotted_names_and_ms() {
    type Row = (&'static [u8], u64, u64, fn(&limits::Limits) -> u64);
    let mut own = limits::profile(limits::Profile::Small);
    let rows: [Row; 2] = [
        (b"forge_component.idle_ms", 7000, 7_000_000_000, |limits| limits.forge_component.idle.as_nanos()),
        (b"forge_component.tls.read", 8192, 8192, |limits| u64::from(limits.forge_component.tls.read)),
    ];
    for (name, input, expected, read) in rows {
        assert_eq!(limits::apply(&mut own, name, input), Ok(()), "{name:?}");
        assert_eq!(read(&own), expected, "{name:?}");
    }
    assert_eq!(limits::apply(&mut own, b"forge_component.deadline_ms", u64::MAX), Err(limits::Refused::Range));
}
#[test]
fn a_hint_the_root_cannot_take_is_offered_again_next_pass() {
    let mut service = fixtures::ready();
    let hinted = fixtures::hook(&mut service);
    for number in 2..16 {
        let engine = service.engine.as_mut().expect("engine");
        if root::try_step(&mut engine.domain, &engine.env, fixtures::period(number)).is_err() {
            break;
        }
    }
    let engine = service.engine.as_mut().expect("engine");
    assert!(root::try_step(&mut engine.domain, &engine.env, forge::hinted(hinted.clone())).is_err());
    service.engine.as_mut().expect("engine").forge.above.push(skein_forge_connection::Event::Hint(hinted.clone()));
    crate::engine::forge_pass(service.engine.as_mut().expect("engine"));
    assert_eq!(service.engine.as_ref().expect("engine").forge.above.len(), 0);
    service.engine.as_mut().expect("engine").forge.down(skein_forge_connection::Request::Offer);
    let Some(skein_forge_connection::Event::Hint(offered)) = service.engine.as_mut().expect("engine").forge.above.pop()
    else {
        panic!("offered hint")
    };
    assert_eq!(offered, hinted);
}
#[test]
fn a_lost_webhooks_listener_drains_the_service_and_stops_it() {
    let mut service = fixtures::started();
    service.engine.as_mut().expect("engine").forge.above.push(skein_forge_connection::Event::ListenerLost);
    crate::engine::forge_pass(service.engine.as_mut().expect("engine"));
    let mut forge = fixtures::ForgeFace::new();
    for _ in 0_u32..64 {
        forge.turn(&mut service);
    }
    assert_eq!(service.end(), Some(&End::Stopped(Stop::Hooks)));
}
#[test]
fn a_shutdown_during_a_lost_webhooks_listeners_drain_ends_at_once_with_its_reason() {
    let mut service = fixtures::started();
    service.engine.as_mut().expect("engine").forge.above.push(skein_forge_connection::Event::ListenerLost);
    let mut forge = fixtures::ForgeFace::new();
    for _ in 0_u32..64 {
        if service.engine.as_ref().expect("engine").drain.is_some() {
            break;
        }
        forge.turn(&mut service);
    }
    assert!(service.engine.as_ref().expect("engine").drain.is_some(), "the lost listener drains");
    assert_eq!(service.end(), None, "the drain is under way");
    service.shutdown();
    assert_eq!(service.end(), Some(&End::Stopped(Stop::Hooks)));
}

#[test]
fn a_bound_webhooks_listener_announces_its_address_once() {
    let mut service = fixtures::started();
    let addr = ([127, 0, 0, 1], 54321).into();
    service
        .engine
        .as_mut()
        .expect("engine")
        .forge
        .above
        .push(skein_forge_connection::Event::Listening { addr: skein_io::Addr::Ip(addr) });
    let mut forge = fixtures::ForgeFace::new();
    for _ in 0_u32..64 {
        forge.turn(&mut service);
    }
    assert_eq!(service.listening().pop(), Some(crate::Note::Listening { listener: crate::Listening::Webhooks, addr }));
    assert!(service.listening().is_empty());
    assert_eq!(service.end(), None);
}

#[test]
fn a_hook_secret_bound_names_its_limit() {
    let mut found = jig_protocol_config::Problems::empty();
    forge::problems(skein_forge_connection::Unusable::SecretBytes { bytes: 16 }, &mut found);
    assert_eq!(
        crate::startup::lines(&crate::startup::Problems::jig(found, &jig_protocol_config::Path::root())).as_ref(),
        b"application.limits.overrides.forge_component.hook_secret_bytes: past its bound\n"
    );
}
#[test]
fn a_hook_server_over_io_names_both_limits() {
    let mut found = jig_protocol_config::Problems::empty();
    forge::problems(
        skein_forge_connection::Unusable::Stack {
            stacked: skein_forge_connection::limits::Stacked::ServerOverIo,
            demand: 8192,
            cap: 4096,
        },
        &mut found,
    );
    assert_eq!(crate::startup::lines(&crate::startup::Problems::jig(found, &jig_protocol_config::Path::root())).as_ref(),
        b"application.limits.overrides.forge_component.hook_head_bytes: past its bound\napplication.limits.overrides.io.intake: past its bound\napplication.limits.overrides.io.output: past its bound\n");
}

#[test]
fn temper_word_keeps_the_marker_bytes_and_length() {
    let config = fixtures::engine();
    let mut found = jig_protocol_config::Problems::empty();
    let settings =
        forge::settings(&config.forges, fixtures::forge_runtime(), None, &mut found).expect("fixture settings");
    let defaults = config.limits.forge_component;
    assert!(settings.forges.get(0).unwrap().log_origins.is_empty());
    assert_eq!((defaults.log_origins, defaults.log_pages, defaults.delivery_bytes), (4, 10, 1_048_576));
    assert_eq!(defaults.log_poll, Duration::from_secs(5));
    assert_eq!(defaults.log_catch_up, Duration::from_secs(86_400));
    assert_eq!(settings.word.as_ref(), b"temper");
    assert_eq!(skein_forge::marker::length(&settings.word, 2), Some(25));
    let encoded = skein_forge::marker::encode(&settings.word, &[0xab, 1], b"", 25);
    assert_eq!(encoded.as_deref(), Some(b"<!-- temper:key ab01 -->\n".as_slice()));
}

fn problem_lines(refusal: skein_forge_connection::Unusable) -> Box<[u8]> {
    let mut found = jig_protocol_config::Problems::empty();
    forge::problems(refusal, &mut found);
    crate::startup::lines(&crate::startup::Problems::jig(found, &jig_protocol_config::Path::root()))
}

#[test]
fn the_component_s_new_refusals_are_problems_naming_their_members() {
    let rows: [(skein_forge_connection::Unusable, &[u8]); 2] = [
        (skein_forge_connection::Unusable::Word, b"application.limits.overrides.forge_component: past its bound\n"),
        (skein_forge_connection::Unusable::PlaintextName { forge: 1 }, b"application.forges[1].api: past its bound\n"),
    ];
    for (refusal, expected) in rows {
        assert_eq!(problem_lines(refusal).as_ref(), expected);
    }
}

#[test]
fn the_forge_fixture_number_differs_from_its_index() {
    let config = fixtures::engine();
    assert_ne!(config.forges[0].number, 0);
}

#[test]
fn a_refusal_naming_a_forge_is_indexed_to_the_configured_forge() {
    use skein_forge_connection::Unusable;
    let config = fixtures::engine();
    let number = config.forges[0].number;
    let rows: [(Unusable, &[u8]); 3] = [
        (Unusable::PlaintextName { forge: number }, b"application.forges[0].api: past its bound\n"),
        (Unusable::Origin { forge: number, origin: skein_forge::github::redirect::Origin { tls: false, host: Box::from(b"logs.test".as_slice()), port: 80 } }, b"application.forges[0].log_origins: past its bound\n"),
        (Unusable::LogOrigins { forge: number, origins: 5, limit: 4 }, b"application.forges[0].log_origins: past its bound\napplication.limits.overrides.forge_component.log_origins: past its bound\n"),
    ];
    for (refusal, expected) in rows {
        assert_eq!(problem_lines(forge::indexed(refusal, &config.forges)).as_ref(), expected);
    }
}

#[test]
fn the_start_request_carries_the_user_agent_and_the_host() {
    let mut service = fixtures::started();
    let mut face = fixtures::ForgeFace::new();
    face.connected(&mut service);
    assert_eq!(
        face.sent(),
        b"GET /api/v1/user HTTP/1.1\r\nHost: forge.example:3000\r\n\
        Authorization: token api-token\r\nUser-Agent: temper/0.0.0\r\n\r\n"
    );
}

// Small operations fit the profile's default room. A larger key needs
// twice its bytes in hex, the six-byte word and the marker's 15-byte framing.
const SMALL_OP_BYTES: u32 = 100;
const LARGE_OP_BYTES: u32 = 10_000;
const PROFILE_BODY_BYTES: u32 = 16_384;
const LARGE_BODY_BYTES: u32 = 2 * LARGE_OP_BYTES + 6 + 15;

#[test]
fn the_small_profile_keeps_the_body_room_the_word_derives() {
    for (op_bytes, expected) in [(SMALL_OP_BYTES, PROFILE_BODY_BYTES), (LARGE_OP_BYTES, LARGE_BODY_BYTES)] {
        let mut limits = limits::profile(limits::Profile::Small).forge;
        limits.client.op_bytes = op_bytes;
        assert_eq!(forge::small(&limits).body_bytes, expected, "op_bytes={op_bytes}");
    }
}

#[test]
fn the_shared_numbers_are_the_clients_plain_numbers() {
    let config = fixtures::engine();
    let limits = &config.limits;
    let io = &config.jig.limits.io;
    assert_eq!(
        forge::shared(limits, io),
        skein_forge_connection::Shared {
            calls: limits.forge.client.calls,
            rows: limits.forge.client.rows,
            answer_bytes: limits.forge.client.answer_bytes,
            op_bytes: limits.forge.client.op_bytes,
            lifetime: limits.forge.client.lifetime,
            name_bytes: limits.forge.name_bytes,
            repositories: limits.forge.repositories,
            io: *io,
        }
    );
}

const HEAD_PROBLEMS: &[u8] = b"application.forges[0].api_token: past its bound\n\
    application.limits.overrides.forge_component.target_bytes: past its bound\n\
    application.limits.overrides.forge_component.token_bytes: past its bound\n\
    application.limits.overrides.forge_component.tls.send: past its bound\n\
    application.limits.overrides.io.output: past its bound\n";

#[test]
fn a_head_past_the_request_room_is_a_problem_naming_its_members_and_the_forge() {
    let forges = fixtures::forges();
    let refusal = skein_forge_connection::Unusable::Request { forge: forges[0].number };
    assert_eq!(problem_lines(forge::indexed(refusal, &forges)).as_ref(), HEAD_PROBLEMS);
}

// The output room is 1,048,576 bytes, less the fixed 192, Host's 18,
// token bound's 512 and user agent's 12 bytes: exactly 1,047,842 target bytes.
const FITTING_TARGET_BYTES: u32 = 1_047_842;
const REFUSED_TARGET_BYTES: u32 = 1_047_843;

#[test]
fn the_longest_head_of_the_shipped_forge_fits_its_room_to_the_byte() {
    let mut config = fixtures::engine();
    for (target_bytes, expected) in [(FITTING_TARGET_BYTES, None), (REFUSED_TARGET_BYTES, Some(HEAD_PROBLEMS))] {
        config.limits.forge_component.target_bytes = target_bytes;
        let problems = match forge::prepare(&config, fixtures::forge_runtime()) {
            Ok(_) => None,
            Err(found) => {
                Some(crate::startup::lines(&crate::startup::Problems::jig(found, &jig_protocol_config::Path::root())))
            }
        };
        assert_eq!(problems.as_deref(), expected, "target_bytes={target_bytes}");
    }
}

#[test]
fn a_full_notes_queue_keeps_its_first_diagnostic_and_the_engine_running() {
    let mut service = fixtures::ready();
    let occupied = service.notes().room() - 1;
    for _ in 0..occupied {
        service.notes().push(crate::Note::ForgeUnconfigured { number: 1 });
    }
    for user in [7, 8] {
        service
            .engine
            .as_mut()
            .expect("engine")
            .forge
            .above
            .push(skein_forge_connection::Event::Stopped { forge: 1, why: Stopped::OtherUser { user } });
    }
    crate::engine::forge_pass(service.engine.as_mut().expect("engine"));
    assert_eq!(service.end(), None);
    assert!(service.engine.as_ref().expect("engine").drain.is_none());
    for _ in 0..occupied {
        service.notes().pop().unwrap();
    }
    assert_eq!(
        service.notes().pop(),
        Some(crate::Note::ForgeStopped { number: 1, why: Stopped::OtherUser { user: 7 } })
    );
    assert!(service.notes().is_empty());
}

#[test]
fn connected_then_token_refused_keeps_the_note_and_engine_running() {
    let mut service = fixtures::ready();
    let why = Stopped::TokenRefused { installation: 7, status: 401 };
    service.engine.as_mut().expect("engine").forge.above.push(skein_forge_connection::Event::Connected { forge: 1 });
    service.engine.as_mut().expect("engine").forge.above.push(skein_forge_connection::Event::Stopped { forge: 1, why });
    crate::engine::forge_pass(service.engine.as_mut().expect("engine"));
    assert_eq!(service.notes().pop(), Some(crate::Note::ForgeStopped { number: 1, why }));
    assert_eq!(service.end(), None);
    assert!(service.engine.as_ref().expect("engine").drain.is_none());
}

fn records_service() -> (crate::Service, skein_lib::Secret) {
    use crate::settings::{Kind, Transport};
    use skein_lib::List;
    let mut settings = fixtures::engine();
    settings.limits.forge_component.forges = 9;
    settings.limits.forge_component.repositories = 9;
    settings.limits.forge.repositories = 9;
    settings.limits.forge.client.repositories = 9;
    settings.limits.forge.output = 256;
    let mut forges = List::with_capacity(9);
    let mut runtime = List::with_capacity(9);
    for number in 1..=9_u16 {
        let mut forge = fixtures::forges().into_iter().next().unwrap();
        forge.number = number;
        if number != 1 {
            forge.kind = Kind::GitHub;
            forge.api_token = None;
        }
        forge.api = jig_protocol_config::Address::Addr(skein_io::Addr::Ip(
            ([127, 0, 0, 1], 2999_u16.checked_add(number).unwrap()).into(),
        ));
        forge.transport = Transport::Plaintext;
        forges.push(forge).unwrap();
        runtime
            .push(crate::ForgeRuntime {
                number,
                address: ([127, 0, 0, 1], 2999_u16.checked_add(number).unwrap()).into(),
                transport: skein_forge_connection::settings::Transport::Plaintext { name: None },
                trust_roots: 0,
                trust_root_bytes: 0,
            })
            .unwrap();
    }
    settings.forges = forges.into_boxed();
    fixtures::fill_files(&mut settings);
    let file_token = settings.forges[0].api_token.clone().unwrap();
    let mut service = crate::Service::engine(settings, &fixtures::seeds(), None, runtime).unwrap();
    service.engine.as_mut().expect("engine").store = fixtures::records_store();
    (service, file_token)
}
#[expect(clippy::disallowed_macros, reason = "ordinary fixtures build versioned row bytes")]
fn startup_records() -> skein_lib::List<(u64, skein_lib::Secret)> {
    use skein_lib::{List, Secret};
    use temper_protocol_records::{self as records, Item, Origin};
    let mut kept = List::with_capacity(64);
    for number in [1, 2, 4, 5, 6, 7, 8, 9, 77] {
        let kind = if number == 1 || number == 7 {
            skein_forge_connection::settings::Kind::Forgejo
        } else {
            skein_forge_connection::settings::Kind::GitHub
        };
        let api = if number == 8 {
            "another.test:443".into()
        } else {
            alloc::format!("127.0.0.1:{}", 2999_u16.checked_add(number).unwrap()).into_boxed_str()
        };
        let mut values = List::with_capacity(7);
        values.push((Item::Origin, Origin { kind, api }.encode())).unwrap();
        if number != 4 {
            values.push((Item::User, Box::from(9_u64.to_le_bytes()))).unwrap();
            if number == 1 {
                let token = skein_lib::Decimal::of(skein_lib::Rng::new(512).next_u64());
                values.push((Item::ApiToken, Box::from(token.as_bytes()))).unwrap();
            } else {
                for (item, value) in [
                    (Item::AppId, Box::from(7_u64.to_le_bytes())),
                    (Item::AppSlug, Box::from(b"test-app".as_slice())),
                    (Item::AppKey, Box::from(skein_fake_forge_github::keys::KEYS[0])),
                    (Item::Accepted, Box::from(1_u64.to_le_bytes())),
                ] {
                    values.push((item, value)).unwrap();
                }
                if number == 9 {
                    values
                        .push((
                            Item::ApiToken,
                            Box::from(skein_lib::Decimal::of(skein_lib::Rng::new(513).next_u64()).as_bytes()),
                        ))
                        .unwrap();
                }
            }
        }
        for (item, bytes) in values.into_boxed() {
            let mut value = records::encode(&bytes);
            if number == 5 && item == Item::User {
                value[0] = 2;
            }
            if number == 6 && item == Item::AppKey {
                value.resize(1198, 0);
            }
            let number = records::number(number, item);
            kept.push((number, Secret::new(value.into_boxed_slice()))).unwrap();
        }
    }
    // A carried row and a later one: neither value decodes as any record.
    kept.push((records::CARRIED, Secret::new(Box::from([2, 255])))).unwrap();
    kept.push((records::CARRIED + 1, Secret::new(Box::from([2, 254])))).unwrap();
    kept
}
#[test]
#[expect(clippy::disallowed_methods, reason = "ordinary routing tests inspect store requests and HTTP fixture bytes")]
#[expect(
    clippy::too_many_lines,
    reason = "one paged startup proves routing and retention beside its existing handoffs"
)]
fn records_start_erases_carried_rows_before_connecting_without_decoding_them() {
    use jig_protocol_store::{SecretOwner, Secrets};
    use skein_lib::List;
    use temper_protocol_records::{self as records, NotConnected};
    let (mut service, file_token) = records_service();
    let kept = startup_records();
    for (number, value) in &kept {
        assert!(
            service
                .engine
                .as_mut()
                .expect("engine")
                .store
                .secrets(Token::new(123), fixtures::application_request(*number, value.clone()))
                .is_none()
        );
        match service.engine.as_mut().expect("engine").store.above.pop() {
            Some(jig_protocol_store::Event::Kept { .. }) if *number < records::CARRIED => (),
            Some(jig_protocol_store::Event::Carried { number: found, .. }) => assert_eq!(found, *number),
            other => panic!("row kept: {other:?}"),
        }
    }
    let mut face = fixtures::ForgeFace::new();
    for _ in 0_u32..40 {
        face.turn(&mut service);
    }
    assert!(service.engine.as_ref().expect("engine").forge_records.done());
    assert!(
        service.engine.as_ref().expect("engine").forge_records.rows.forge(2).is_some(),
        "setup retains the loaded rows"
    );
    assert!(
        service.engine.as_ref().expect("engine").forge_records.rows.forge(77).is_none(),
        "unconfigured values are not retained"
    );
    assert_eq!(service.engine.as_ref().expect("engine").store.forge_loads.len(), 4);
    assert_eq!(service.engine.as_ref().expect("engine").store.erases, 2);
    for number in [records::CARRIED, records::CARRIED + 1] {
        let Some(jig_protocol_store::Event::Secret { value: None, .. }) = service
            .engine
            .as_mut()
            .unwrap()
            .store
            .secrets(Token::new(125), Secrets::Get { owner: SecretOwner::Application, number })
        else {
            panic!("surviving carried row erased")
        };
    }
    for number in &service.engine.as_ref().expect("engine").forge_records.inspected {
        assert!(*number < records::CARRIED);
    }
    assert_eq!(
        service.engine.as_ref().expect("engine").forge_records.inspected.len(),
        kept.len().checked_sub(2).unwrap()
    );
    assert_eq!(service.engine.as_ref().expect("engine").forge.starts.len(), 1);
    let (number, access) = service.engine.as_mut().expect("engine").forge.starts.get(0).unwrap();
    assert_eq!((*number, access.user), (2, 9));
    let skein_forge::credential::Credential::App { id, key } = &access.credential else { panic!("App credential") };
    assert_eq!(*id, 7);
    assert!(key.expose() == skein_fake_forge_github::keys::KEYS[0]);
    assert_eq!(service.engine.as_ref().expect("engine").forge.accepted.get(0).unwrap(), &(2, Box::from([1_u64])));
    let mut notes = List::with_capacity(9);
    while let Some(note) = service.notes().pop() {
        notes.push(note).unwrap();
    }
    for (number, why) in [
        (3, NotConnected::NoRecords),
        (4, NotConnected::Incomplete),
        (5, NotConnected::Unreadable),
        (6, NotConnected::Unreadable),
        (
            7,
            NotConnected::OtherForge {
                kind: skein_forge_connection::settings::Kind::Forgejo,
                api: Box::from("127.0.0.1:3006"),
            },
        ),
        (
            8,
            NotConnected::OtherForge {
                kind: skein_forge_connection::settings::Kind::GitHub,
                api: Box::from("another.test:443"),
            },
        ),
        (
            9,
            NotConnected::OtherForge {
                kind: skein_forge_connection::settings::Kind::Forgejo,
                api: Box::from("127.0.0.1:3008"),
            },
        ),
    ] {
        assert!(notes.as_slice().contains(&crate::Note::ForgeNotConnected { number, why }));
    }
    assert_eq!(notes.len(), 8);
    assert!(notes.as_slice().contains(&crate::Note::ForgeUnconfigured { number: 77 }));
    let mut unconfigured = 0_u32;
    for note in &notes {
        if *note == (crate::Note::ForgeUnconfigured { number: 77 }) {
            unconfigured = unconfigured.checked_add(1).unwrap();
        }
    }
    assert_eq!(unconfigured, 1, "all of the unconfigured forge's rows name it once");
    face.select(1);
    assert!(
        face.sent().windows(b"GET /api/v1/user ".len()).any(|part| part == b"GET /api/v1/user "),
        "Forgejo starts with its files"
    );
    assert!(face.sent().windows(file_token.expose().len()).any(|part| part == file_token.expose()));
    face.capture_answers();
    for number in 3..=9 {
        service.engine.as_mut().expect("engine").forge.down(skein_forge_connection::Request::Call {
            call: Token::new(u64::from(number)),
            repository: skein_forge::api::Repository { forge: number, repository: 0 },
            op: skein_forge::api::Op::Read(skein_forge::api::Read::Settings),
        });
        face.turn(&mut service);
    }
    face.turn(&mut service);
    let answers = face.take_answers();
    assert_eq!(answers.len(), 7);
    for answer in answers {
        assert_eq!(answer.cost, 0);
        assert_eq!(answer.result, Err(skein_forge::api::Error::NotConnected));
    }
    assert_records_untouched(&mut service, &kept);
}
#[test]
fn an_unkept_installation_only_emits_its_diagnostic() {
    let mut service = fixtures::ready();
    service.engine.as_mut().expect("engine").forge.above.push(skein_forge_connection::Event::Fact {
        forge: 1,
        fact: skein_forge_connection::Fact::Unkept { installation: 93 },
    });
    crate::engine::forge_pass(service.engine.as_mut().expect("engine"));
    assert_eq!(service.notes().pop(), Some(crate::Note::ForgeUnkept { number: 1, installation: 93 }));
    assert!(service.engine.as_ref().expect("engine").events.is_empty());
    assert_eq!(service.end(), None);
    assert!(service.engine.as_ref().expect("engine").drain.is_none());
}

fn assert_records_untouched(service: &mut crate::Service, kept: &skein_lib::List<(u64, skein_lib::Secret)>) {
    use jig_protocol_store::{SecretOwner, Secrets};
    for (number, original) in kept {
        if *number >= temper_protocol_records::CARRIED {
            continue;
        }
        let Some(jig_protocol_store::Event::Secret {
            value: Some(jig_protocol_store::SecretValue::Application(found)),
            ..
        }) = service
            .engine
            .as_mut()
            .expect("engine")
            .store
            .secrets(Token::new(124), Secrets::Get { owner: SecretOwner::Application, number: *number })
        else {
            panic!("row preserved")
        };
        assert!(found.expose() == original.expose());
    }
}

#[test]
fn a_refused_or_corrupt_records_load_stops_the_engine_named() {
    for corrupt in [true, false] {
        let mut service = fixtures::started();
        if corrupt {
            service
                .engine
                .as_mut()
                .expect("engine")
                .store
                .above
                .push(jig_protocol_store::Event::Corrupt { owner: forge::records::LOAD });
        } else {
            let (root, _, profile) = fixtures::store_limits();
            let limits = crate::store::limits(&root, &root.forge, &profile).unwrap();
            service.engine.as_mut().expect("engine").store =
                crate::store::Store::new(&root, &limits, &[], fixtures::seeds().installation);
            service.engine.as_mut().expect("engine").store.opened = true; // The closed store refuses the loader's synchronous read.
        }
        crate::engine::iterate(service.engine.as_mut().expect("engine"), Time::from_nanos(1), skein_lib::Wall::EPOCH);
        let mut expected = jig_protocol_config::Problems::empty();
        expected.push(jig_protocol_config::Path::root().member(b"forge records"), jig_protocol_config::What::Refers);
        assert_eq!(service.end(), Some(&End::Stopped(Stop::Start(expected))));
    }
}

#[test]
fn a_records_key_that_cannot_sign_stops_only_its_forge() {
    let (mut service, _) = records_service();
    for (number, value) in startup_records().into_boxed() {
        let value = if number == temper_protocol_records::number(2, temper_protocol_records::Item::AppKey) {
            skein_lib::Secret::new(temper_protocol_records::encode(&[0x30, 0]).into_boxed_slice())
        } else {
            value
        };
        let _write = service
            .engine
            .as_mut()
            .expect("engine")
            .store
            .secrets(Token::new(123), fixtures::application_request(number, value));
        let _kept = service.engine.as_mut().expect("engine").store.above.pop();
    }
    let mut face = fixtures::ForgeFace::new();
    for _ in 0_u32..40 {
        face.turn(&mut service);
    }
    let mut stopped = 0_u32;
    while let Some(note) = service.notes().pop() {
        if note
            == (crate::Note::ForgeStopped {
                number: 2,
                why: Stopped::Invalid { value: skein_forge_connection::Value::Key },
            })
        {
            stopped = stopped.checked_add(1).unwrap();
        }
    }
    assert_eq!(stopped, 1);
    assert_eq!(service.end(), None);
    face.select(1);
    assert!(face.sent().starts_with(b"GET /api/v1/user "));
    assert_eq!(service.engine.as_ref().expect("engine").store.erases, 2);
}
