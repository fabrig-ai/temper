use super::fixtures;
use jig_domain_core as core;
use jig_domain_host as host;
use jig_domain_tasks as tasks;
use jig_protocol_engine_workers::GrantValue;
use skein_lib::Time;
use skein_lib::Token;
#[test]
fn each_repositorys_grant_carries_its_identitys_value() {
    let values = fixtures::values_with_identity(1, b"forge-token-1234");
    let filled = values.grants(&[fixtures::application_grant(1, 1)], Time::ZERO);
    let [Some(GrantValue::Application { secret })] = filled.as_ref() else { panic!("one application value") };
    let identity = fixtures::decoded_identity(secret);
    let temper_protocol_worker_wire::v1::Kind::Forgejo(token) = identity.kind() else { panic!("Forgejo identity") };
    assert_eq!(token.token().expose(), b"forge-token-1234");
    assert_eq!(identity.login(), b"temper");
    assert_eq!(identity.name(), b"temper");
    assert_eq!(identity.email(), b"temper@forge.example");
}
#[test]
fn a_missing_identity_or_generation_keeps_its_grant_slot_empty() {
    let values = fixtures::values_with_identity(1, b"forge-token-1234");
    let grants =
        [fixtures::application_grant(1, 2), fixtures::application_grant(2, 1), fixtures::application_grant(1, 1)];
    let filled = values.grants(&grants, Time::ZERO);
    assert_eq!(filled.len(), 3);
    for (index, present) in [(0, false), (1, false), (2, true)] {
        assert_eq!(filled[index].is_some(), present, "grant {index}");
    }
}
#[test]
fn application_grants_answer_either_retained_generation() {
    let mut values = fixtures::values_with_identity(1, b"first");
    let next = fixtures::values_with_identity(1, b"second");
    let (_, value) = next.identities.newest(1).unwrap();
    assert_eq!(values.identities.put(1, value.clone()), 2);
    let filled = values.grants(
        &[
            fixtures::application_grant(1, 1),
            fixtures::application_grant(1, 2),
            fixtures::application_grant(1, 0),
            fixtures::application_grant(1, 3),
        ],
        Time::ZERO,
    );
    for (index, token) in [(0, b"first".as_slice()), (1, b"second".as_slice())] {
        let Some(GrantValue::Application { secret }) = &filled[index] else { panic!("retained generation") };
        let identity = fixtures::decoded_identity(secret);
        let temper_protocol_worker_wire::v1::Kind::Forgejo(value) = identity.kind() else { panic!("Forgejo") };
        assert_eq!(value.token().expose(), token);
    }
    assert!(filled[2].is_none());
    assert!(filled[3].is_none());
}
#[test]
fn identities_are_announced_before_the_restart() {
    let mut service = fixtures::started();
    service.engine.as_mut().expect("engine").store.opened = true;
    let _listen = service.engine.as_mut().expect("engine").workers.io.pop().unwrap();
    assert_eq!(
        service.engine.as_ref().expect("engine").domain.next_deadline(&service.engine.as_ref().expect("engine").env),
        None
    );
    for _ in 0_u8..8 {
        crate::engine::iterate(service.engine.as_mut().expect("engine"), Time::ZERO, skein_lib::Wall::EPOCH);
        if service.engine.as_mut().expect("engine").credentials.is_empty() {
            break;
        }
        assert!(service.engine.as_ref().expect("engine").store.routed.is_empty(), "records precede Restart");
    }
    assert!(service.engine.as_ref().expect("engine").forge_records.done());
    assert!(service.engine.as_ref().expect("engine").credentials.is_empty());
    assert_eq!(
        service.engine.as_ref().expect("engine").domain.next_deadline(&service.engine.as_ref().expect("engine").env),
        Some(Time::from_nanos(u64::MAX))
    );
    assert!(service.engine.as_ref().expect("engine").store.routed.is_empty(), "the restart has not asked for a load");
    fixtures::restart(&mut service);
}

#[test]
fn an_accounts_grant_uses_the_lent_generation_until_it_expires() {
    let mut values = fixtures::values_with_identity(1, b"forge-token-1234");
    values
        .accounts
        .granted(
            1,
            2,
            jig_protocol_accounts::table::Value {
                token: skein_lib::Secret::new(b"account-token".as_slice().into()),
                scheme: jig_protocol_accounts::table::Scheme::Bearer,
            },
            Time::from_nanos(9),
        )
        .unwrap();
    let mut grant = fixtures::application_grant(1, 2);
    grant.form = jig_domain_host::Form::Credential(jig_domain_host::Scheme::Bearer);
    for (now, generation, expected) in [(8, 2, true), (9, 2, false), (8, 1, false)] {
        grant.generation = generation;
        let filled = values.grants(&[grant], Time::from_nanos(now));
        assert_eq!(filled[0].is_some(), expected, "{now} {generation}");
        if let Some(value) = &filled[0] {
            let GrantValue::Credential { scheme: jig_protocol_engine_workers::Scheme::Bearer, token } = value else {
                panic!("a bearer grant")
            };
            assert_eq!(token.expose(), b"account-token", "{now} {generation}");
        }
    }
}

fn assignment_with_identity() -> temper_engine_domain::Assignment {
    let mut assignment = fixtures::assignment_with(fixtures::repository(7, b"acme", b"shop"));
    assignment.grants = Box::new([jig_domain_accounts::Grant {
        form: jig_domain_accounts::Form::Application,
        account: 1,
        generation: 1,
        valid: skein_lib::Duration::from_secs(30),
    }]);
    assignment.transcript = Box::new([b"turn".as_slice().into()]);
    assignment
}
#[test]
fn an_assignment_carries_its_items_encoded_charter_and_identity_value() {
    let settings = fixtures::engine();
    let encoding = fixtures::encoding(&settings);
    let assignment = assignment_with_identity();
    let values = fixtures::values_with_identity(1, b"forge-token-1234");
    let event =
        crate::assignment::down(assignment, &settings.forges, &encoding, &values, Token::new(8), Time::ZERO).unwrap();
    let jig_protocol_engine_workers::Event::Down {
        channel,
        down: jig_domain_host::LinkDown::Assign { assignment },
        parts,
        values,
    } = event
    else {
        panic!("assignment event")
    };
    assert_eq!(channel, Token::new(8));
    assert_eq!(assignment.assignment.run, Token::new(7));
    assert_eq!(assignment.assignment.attempt, Token::new(2));
    assert_eq!(
        assignment.assignment.identity,
        [118, 213, 170, 119, 186, 221, 240, 223, 121, 163, 222, 185, 198, 157, 28, 125]
    );
    assert!(!assignment.assignment.save);
    let workspace = assignment.assignment.workspace.unwrap();
    assert_eq!(workspace.workstream, 7);
    assert_eq!(workspace.items, parts[0].token);
    let items =
        temper_protocol_worker_wire::v1::Items::decode(&encoding.limits, &mut skein_lib::Reader::new(&parts[0].bytes))
            .unwrap();
    assert_eq!(items.repositories().get(0).unwrap().remote(), b"https://forge.example/acme/shop.git");
    let charter = decoded_charter(&assignment.assignment.charter);
    assert!(charter.resume());
    let Some(GrantValue::Application { secret }) = &values[0] else { panic!("an application value") };
    let value = fixtures::decoded_identity(secret);
    let temper_protocol_worker_wire::v1::Kind::Forgejo(token) = value.kind() else { panic!("Forgejo identity") };
    assert_eq!(token.token().expose(), b"forge-token-1234");
}
#[test]
#[expect(clippy::disallowed_methods, reason = "ordinary tests assemble byte fixtures and inspect text")]
fn a_fresh_start_whose_tail_reaches_the_turns_headers_still_encodes() {
    let settings = fixtures::engine();
    let encoding = fixtures::encoding(&settings);
    let mut assignment = assignment_with_identity();
    assignment.run.policy.resume = false;
    // jig's tail copies the retained turns' encoded bytes, headers among them.
    let mut tail = b"\x00\x00\x01\x2aearlier words of a turn that came before this one, ".to_vec();
    tail.extend_from_slice(&[0xff, 0xfe, 0x00, 0x00, 0x00, 0x05]);
    tail.extend_from_slice(b"The README's last line is in.");
    let mut sections = assignment.sections.into_vec();
    sections.push(temper_engine_domain::BriefSection {
        kind: temper_engine_domain::BriefKind::Core(jig_domain_brief::Core::TranscriptTail),
        body: temper_engine_domain::BriefBody::Text(tail.into()),
    });
    assignment.sections = sections.into_boxed_slice();
    let values = fixtures::values_with_identity(1, b"forge-token-1234");
    let event =
        crate::assignment::down(assignment, &settings.forges, &encoding, &values, Token::new(8), Time::ZERO).unwrap();
    let jig_protocol_engine_workers::Event::Down { down: jig_domain_host::LinkDown::Assign { assignment }, .. } = event
    else {
        panic!("assignment event")
    };
    let charter = &assignment.assignment.charter;
    assert!(!decoded_charter(charter).resume());
    assert!(skein_lib::bytes::find(charter, b"[earlier bytes not text, cut]\nThe README's last line is in.").is_some());
}
#[test]
#[expect(clippy::disallowed_methods, reason = "ordinary tests assemble byte fixtures and inspect text")]
fn a_tail_keeps_its_text_marked_and_never_grows() {
    use crate::assignment::text_tail;
    const MARKER: &[u8] = b"[earlier bytes not text, cut]\n";
    // A text tail, its newlines and tabs among it, is unchanged.
    assert_eq!(text_tail(b"I appended the line.\n\tDone.").as_ref(), b"I appended the line.\n\tDone.");
    // A tail with no text after its last header is the marker alone, never its bytes.
    assert_eq!(text_tail(&[0xff; 40]).as_ref(), MARKER);
    assert_eq!(text_tail(&[0x00; 40]).as_ref(), MARKER);
    // The marker's room is cut from the text's start, at a character's edge.
    let mut long = b"\x00\x05".to_vec();
    long.extend_from_slice("it’s done: the README’s line’s in, all of it".as_bytes());
    let text = text_tail(&long);
    assert!(text.starts_with(MARKER), "{text:?}");
    assert!(::core::str::from_utf8(&text).is_ok(), "{text:?}");
    assert!(text.ends_with(b"all of it"));
    assert!(text.len() <= long.len());
    // A tail too short for the marker keeps its text alone.
    assert_eq!(text_tail(b"\x00\x05done").as_ref(), b"done");
}
#[test]
fn an_assignment_that_cannot_encode_its_items_is_refused() {
    let settings = fixtures::engine();
    let encoding = fixtures::encoding(&settings);
    let assignment = fixtures::assignment_with(fixtures::repository(7, b"acme", b"shop"));
    let values = fixtures::values_with_identity(1, b"forge-token-1234");
    assert!(crate::assignment::down(assignment, &[], &encoding, &values, Token::new(8), Time::ZERO).is_none());
}
#[test]
fn a_bad_or_missing_answer_part_discards_all_of_its_parts() {
    use jig_domain_host as host;
    use skein_lib::Token;
    let limits = fixtures::wire_limits();
    let resource = temper_protocol_worker_wire::v1::Preparation::Refused(
        temper_protocol_worker_wire::v1::Tag::new(&limits, temper_protocol_worker_wire::v1::TagParts { tag: 7 })
            .unwrap(),
    );
    let up = fixtures::answer(host::Ending::Failed {
        failure: host::Failure::Unprepared(host::Preparation::Permanent { resource: Some(Token::new(2)) }),
        detail: Box::new([]),
        work: host::Work { left: Some(Token::new(1)), saved: None },
    });
    for bytes in [b"".as_slice(), &[0, 0, 0, 0, 3], &[0, 0, 0, 1]] {
        let parts = [
            jig_protocol_worker_wire::Part { token: Token::new(1), bytes: bytes.into() },
            jig_protocol_worker_wire::Part {
                token: Token::new(2),
                bytes: temper_protocol_worker_wire::preparation(&resource),
            },
        ];
        assert_eq!(crate::workers::parts(&up, &parts, &limits), None, "{bytes:?}");
    }
    assert_eq!(crate::workers::parts(&up, &[], &limits), None, "missing part");
}
#[test]
fn a_named_preparation_part_reaches_the_roots_parts() {
    use jig_domain_host as host;
    use skein_lib::Token;
    let limits = fixtures::wire_limits();
    let resource = temper_protocol_worker_wire::v1::Preparation::Refused(
        temper_protocol_worker_wire::v1::Tag::new(&limits, temper_protocol_worker_wire::v1::TagParts { tag: 7 })
            .unwrap(),
    );
    let up = fixtures::answer(host::Ending::Failed {
        failure: host::Failure::Unprepared(host::Preparation::Permanent { resource: Some(Token::new(2)) }),
        detail: Box::new([]),
        work: host::Work { left: None, saved: None },
    });
    let parts = [jig_protocol_worker_wire::Part {
        token: Token::new(2),
        bytes: temper_protocol_worker_wire::preparation(&resource),
    }];
    assert_eq!(
        crate::workers::parts(&up, &parts, &limits).unwrap().preparation,
        Some(temper_engine_domain::Preparation::Refused { tag: 7 })
    );
}

#[test]
fn an_assignment_preserves_its_ordered_messages_and_answered_calls() {
    let settings = fixtures::engine();
    let encoding = fixtures::encoding(&settings);
    let mut assignment = assignment_with_identity();
    assignment.messages = Box::new([
        word(3, tasks::Party::Person(8), b"first"),
        word(5, tasks::Party::Deployment { project: 1 }, b"second"),
    ]);
    assignment.settled = Box::new([core::Settled {
        key: core::CallKey { task: assignment.task, attempt: 1, completion: 2, position: 1 },
        call: core::SettledCall {
            serial: 6,
            tool: b"note".as_slice().into(),
            answer: core::SettledAnswer::Host { error: false, body: b"done".as_slice().into() },
        },
    }]);
    let values = fixtures::values_with_identity(1, b"forge-token-1234");
    let event =
        crate::assignment::down(assignment, &settings.forges, &encoding, &values, Token::new(8), Time::ZERO).unwrap();
    let jig_protocol_engine_workers::Event::Down { down: host::LinkDown::Assign { assignment }, .. } = event else {
        panic!("assignment")
    };
    assert_eq!(assignment.messages.len(), 2);
    for (index, number, sender, words) in
        [(0, 3, b"8".as_slice(), b"first".as_slice()), (1, 5, b"deployment".as_slice(), b"second".as_slice())]
    {
        assert_eq!(assignment.messages[index].name, Token::new(number), "message {index}");
        assert_eq!(assignment.messages[index].sender.as_ref(), sender, "message {index}");
        assert_eq!(assignment.messages[index].words.as_ref(), words, "message {index}");
    }
    assert_eq!(
        assignment.answered.as_ref(),
        &[host::AnsweredCall {
            name: jig_protocol_charter::call_name(&jig_protocol_charter::CallKey {
                activation: 1,
                completion: 2,
                position: 1,
            }),
            tool: b"note".as_slice().into(),
            answer: host::SettledAnswer::Host { error: false, body: b"done".as_slice().into() }
        }]
    );
    assert_eq!(assignment.turns.as_ref(), &[Box::from(b"turn".as_slice())]);
}

fn word(number: u64, from: jig_domain_tasks::Party, words: &[u8]) -> jig_domain_tasks::Word {
    jig_domain_tasks::Word {
        number,
        from,
        words: words.into(),
        kind: jig_domain_tasks::MessageKind::Words,
        at: skein_lib::Wall::EPOCH,
        hits: 0,
        eligible: true,
    }
}

fn decoded_charter(bytes: &[u8]) -> smith_charter::Charter {
    smith_charter::Charter::decode(&smith_charter::CEILINGS, &mut skein_lib::Reader::new(bytes)).unwrap()
}

#[test]
fn a_part_that_does_not_decode_gives_no_parts_and_is_counted() {
    let mut workers = fixtures::workers();
    let mut events = skein_lib::Queue::with_capacity(32);
    for (bytes, expected) in [(b"".as_slice(), 1), (&[0, 0, 0, 1], 2)] {
        let up = fixtures::answer(host::Ending::Ended {
            outcome: Box::new([]),
            work: host::Work { left: Some(Token::new(1)), saved: None },
        });
        workers.received(
            Token::new(8),
            up,
            &[jig_protocol_worker_wire::Part { token: Token::new(1), bytes: bytes.into() }],
            &mut events,
        );
        let Some(temper_engine_domain::Event::Worker(temper_engine_domain::WorkerInput::Link {
            channel,
            link: temper_engine_domain::Link::Up { parts, up },
        })) = events.pop()
        else {
            panic!("answer reaches root")
        };
        assert_eq!(channel, Token::new(8), "{bytes:?}");
        assert_eq!(parts, temper_engine_domain::Parts::none(), "{bytes:?}");
        assert_eq!(
            up,
            fixtures::answer(host::Ending::Ended {
                outcome: Box::new([]),
                work: host::Work { left: Some(Token::new(1)), saved: None },
            }),
            "{bytes:?}"
        );
        assert_eq!(workers.bad_parts, expected, "{bytes:?}");
    }
}
#[test]
fn the_workers_listener_and_accepted_socket_have_their_own_prefixes() {
    let mut workers = fixtures::workers();
    let owner = Token::new(crate::tokens::WORKER_LISTENERS);
    let mut events = skein_lib::Queue::with_capacity(32);
    let Some(skein_io::Request::Listen { owner: requested, addr: addr @ skein_io::Addr::InProcess(_) }) =
        workers.io.pop()
    else {
        panic!("configured in-process listener")
    };
    assert_eq!(requested, owner);
    assert!(workers.up(Time::ZERO, skein_io::Event::Listening { owner, listener: Token::new(3), addr }, &mut events));
    assert!(workers.opened());
    assert!(workers.up(
        Time::ZERO,
        skein_io::Event::Accepted { owner, socket: Token::new(4), peer: addr },
        &mut events
    ));
    let Some(skein_io::Request::Bind { socket, owner: connection }) = workers.io.pop() else {
        panic!("accepted socket bound")
    };
    assert_eq!(socket, Token::new(4));
    assert_eq!(connection, Token::new(crate::tokens::WORKER_CONNECTIONS + 1));
    assert_eq!(workers.sockets.get(&connection), Some(&socket));
}
#[test]
fn a_listener_that_opens_during_closing_is_closed_immediately() {
    let mut workers = fixtures::workers();
    let _listen = workers.io.pop().unwrap();
    let mut events = skein_lib::Queue::with_capacity(32);
    workers.close(Time::ZERO, &mut events);
    assert!(workers.up(
        Time::ZERO,
        skein_io::Event::Listening {
            owner: Token::new(crate::tokens::WORKER_LISTENERS),
            listener: Token::new(3),
            addr: skein_io::Addr::InProcess(skein_io::Name::new(b"workers").unwrap())
        },
        &mut events
    ));
    let Some(skein_io::Request::Close { entity }) = workers.io.pop() else { panic!("late listener close") };
    assert_eq!(entity, Token::new(3));
    assert!(!workers.closed());
    assert!(workers.up(
        Time::ZERO,
        skein_io::Event::Closed { owner: Token::new(crate::tokens::WORKER_LISTENERS) },
        &mut events
    ));
    assert!(workers.closed());
}
#[test]
fn an_assignment_that_cannot_encode_is_answered_invalid_through_the_link() {
    let mut workers = fixtures::workers();
    let mut events = skein_lib::Queue::with_capacity(32);
    let mut assignment = fixtures::assignment_with(fixtures::repository(7, b"acme", b"shop"));
    assignment.workspace.repositories[0].provider.forge = 99;
    workers.assignment(
        Time::ZERO,
        Token::new(8),
        assignment,
        &fixtures::values_with_identity(1, b"forge-token-1234"),
        &mut events,
    );
    let Some(temper_engine_domain::Event::Worker(temper_engine_domain::WorkerInput::Link {
        channel,
        link: temper_engine_domain::Link::Up { up: host::LinkUp::Answer { run, attempt, answer }, parts },
    })) = events.pop()
    else {
        panic!("invalid assignment answer")
    };
    assert_eq!(channel, Token::new(8));
    assert_eq!((run, attempt), (Token::new(7), Token::new(2)));
    assert_eq!(parts, temper_engine_domain::Parts::none());
    assert_eq!(
        answer.ending,
        host::Ending::Refused(host::Refusal::Invalid(host::Invalid::Start {
            cause: host::StartCause::Other,
            why: b"assignment cannot encode".as_slice().into(),
        }))
    );
}

#[test]
fn the_roots_facts_are_drained_and_counted_after_each_step() {
    use temper_engine_domain as root;
    let (mut service, run) = fixtures::claimed_chat();
    let preparation = root::Preparation::Missing { tag: 3, missing: root::Missing::Branch };
    let up = host::LinkUp::Answer {
        run: Token::new(run.task),
        attempt: Token::new(run.attempt),
        answer: host::Answer {
            read: None,
            refused: Box::new([]),
            turns: 0,
            spent: 1,
            ending: host::Ending::Failed {
                failure: host::Failure::Unprepared(host::Preparation::Permanent { resource: None }),
                detail: Box::new([]),
                work: host::Work { left: None, saved: None },
            },
        },
    };
    service.engine.as_mut().expect("engine").step(root::Event::Worker(root::WorkerInput::Link {
        channel: Token::new(7),
        link: root::Link::Up {
            up,
            parts: root::Parts { saved: None, pushed: Box::new([]), preparation: Some(preparation) },
        },
    }));
    fixtures::settle_root(&mut service, &mut skein_lib::Queue::with_capacity(32));
    assert_eq!(service.engine.as_ref().expect("engine").facts, 1);
    assert_eq!(service.engine.as_mut().expect("engine").domain.pop_fact(), None);
    assert_eq!(service.engine.as_ref().expect("engine").domain.facts_lost(), 0);
    service.engine.as_mut().expect("engine").step(root::Event::Resume);
    assert_eq!(service.engine.as_ref().expect("engine").facts, 1);
}

#[test]
fn a_worker_assignment_uses_the_services_configured_forges() {
    use temper_engine_domain as root;
    let mut service = fixtures::started();
    let mut assignment = fixtures::assignment_with(fixtures::repository(7, b"acme", b"shop"));
    assignment.workspace.repositories[0].provider.forge = 99;
    crate::route::request(
        service.engine.as_mut().expect("engine"),
        root::Request::Worker(root::WorkerRequest::Deliver(root::Delivery::Assigned {
            channel: Token::new(8),
            assignment,
        })),
    );
    let Some(root::Event::Worker(root::WorkerInput::Link {
        channel,
        link: root::Link::Up { up: host::LinkUp::Answer { run, attempt, answer }, parts },
    })) = service.engine.as_mut().expect("engine").events.pop()
    else {
        panic!("worker assignment refused through its root input")
    };
    assert_eq!(channel, Token::new(8));
    assert_eq!((run, attempt), (Token::new(7), Token::new(2)));
    assert_eq!(
        answer.ending,
        host::Ending::Refused(host::Refusal::Invalid(host::Invalid::Start {
            cause: host::StartCause::Other,
            why: b"assignment cannot encode".as_slice().into(),
        }))
    );
    assert_eq!(parts, root::Parts::none());
    assert_eq!(service.end(), None);
}

#[test]
fn engine_values_preserve_slots_and_refuse_a_different_declared_form() {
    use jig_protocol_accounts::table::{Scheme, Value};
    let forms = [host::Form::Credential(host::Scheme::Bearer), host::Form::Credential(host::Scheme::Key)];
    for (scheme, actual) in [(Scheme::Bearer, forms[0]), (Scheme::Key, forms[1])] {
        let mut values = fixtures::values_with_identity(1, b"forge-token");
        values
            .accounts
            .granted(
                2,
                3,
                Value { scheme, token: skein_lib::Secret::new(b"llm-token".as_slice().into()) },
                Time::from_nanos(9),
            )
            .unwrap();
        let grants =
            forms.map(|form| host::Grant { form, account: 2, generation: 3, valid: skein_lib::Duration::from_secs(5) });
        let filled = values.grants(&grants, Time::from_nanos(8));
        for (grant, value) in grants.iter().zip(filled.iter()) {
            assert_eq!(value.is_some(), grant.form == actual);
        }
        for value in values.grants(&grants, Time::from_nanos(9)) {
            assert!(value.is_none());
        }
    }
}

#[test]
fn a_program_grant_carries_its_name_without_a_credential_table_entry() {
    let values = crate::values::Values::new(0, 0);
    let name = host::SourceName::new(b"claude").unwrap();
    let grant = host::Grant {
        form: host::Form::Program(name),
        account: 1,
        generation: 2,
        valid: skein_lib::Duration::from_secs(60),
    };
    let filled = values.grants(&[grant], Time::ZERO);
    let [Some(GrantValue::Program { name: held })] = filled.as_ref() else { panic!("one named program value") };
    assert_eq!(*held, name);
}

#[test]
fn a_program_grants_charter_keeps_its_source_account() {
    let settings = fixtures::engine();
    let encoding = fixtures::encoding(&settings);
    let mut assignment = fixtures::assignment_with(fixtures::repository(7, b"acme", b"shop"));
    assignment.run.sources = Box::new([Some(17)]);
    assignment.grants = Box::new([jig_domain_accounts::Grant {
        form: jig_domain_accounts::Form::Program(jig_domain_accounts::SourceName::new(b"alice").unwrap()),
        account: 17,
        generation: 2,
        valid: skein_lib::Duration::from_secs(60),
    }]);
    let empty = crate::values::Values::new(0, 0);
    let jig_protocol_engine_workers::Event::Down { down: host::LinkDown::Assign { assignment: sent }, values, .. } =
        crate::assignment::down(assignment, &settings.forges, &encoding, &empty, Token::new(8), Time::ZERO).unwrap()
    else {
        panic!("assignment down")
    };
    let [grant] = sent.assignment.grants.as_ref() else { panic!("one program grant") };
    assert_eq!(grant.form, host::Form::Program(host::SourceName::new(b"alice").unwrap()));
    let [Some(GrantValue::Program { name })] = values.as_ref() else { panic!("one name without a table entry") };
    assert_eq!(name.as_bytes(), b"alice");
    let charter = decoded_charter(&sent.assignment.charter);
    assert_eq!(charter.main().account(), &Some(17));
    assert_eq!(charter.main().endpoint(), b"fake");
}
