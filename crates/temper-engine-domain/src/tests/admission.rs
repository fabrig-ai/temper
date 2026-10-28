use super::resume_fixture::root;
use crate::{Call, Event, Link, Parts, Tool, WorkerInput};
use alloc::boxed::Box;
use jig_domain_host as host;
use skein_lib::{ReplyTo, Token};

#[test]
fn a_full_payload_slab_returns_payload_records_but_takes_metadata() {
    let (mut domain, env) = root();
    while crate::link::room(&domain) {
        domain.payloads.insert(None).unwrap();
    }
    let full = domain.payloads.len();
    for input in [
        WorkerInput::Link {
            channel: Token::new(7),
            link: Link::Up {
                up: host::LinkUp::Turn {
                    run: Token::new(1),
                    attempt: Token::new(1),
                    turn: host::Turn {
                        opens_window: true,
                        refused: Box::new([]),
                        turn: 1,
                        spent: 0,
                        read: None,
                        body: Box::new([]),
                    },
                },
                parts: Parts::none(),
            },
        },
        WorkerInput::Link {
            channel: Token::new(7),
            link: Link::Up {
                up: host::LinkUp::Answer {
                    run: Token::new(1),
                    attempt: Token::new(1),
                    answer: host::Answer {
                        read: None,
                        refused: Box::new([]),
                        turns: 0,
                        spent: 0,
                        ending: host::Ending::Refused(host::Refusal::Busy),
                    },
                },
                parts: Parts::none(),
            },
        },
        WorkerInput::DecodedHostCall {
            to: ReplyTo::new(Token::new(8)),
            body: Call { completion: 1, position: 0, tool: Tool::Unavailable },
        },
        WorkerInput::RenderedHostCall {
            to: ReplyTo::new(Token::new(8)),
            answer: jig_domain_core::SettledAnswer::Host { error: true, body: Box::new([]) },
        },
    ] {
        assert!(
            crate::try_step(&mut domain, &env, Event::Worker(input)).is_err(),
            "payload ownership stays with the caller"
        );
        assert_eq!(domain.payloads.len(), full, "the slab cap stays");
    }
    for up in [
        host::LinkUp::Logins { logins: Box::new([]) },
        host::LinkUp::Call {
            run: Token::new(999),
            attempt: Token::new(999),
            call: Box::new([]),
            tool: Box::new([]),
            writes: false,
            input: Box::new([]),
            deadline: skein_lib::Duration::from_secs(1),
        },
    ] {
        crate::try_step(
            &mut domain,
            &env,
            Event::Worker(WorkerInput::Link { channel: Token::new(7), link: Link::Up { up, parts: Parts::none() } }),
        )
        .expect("non-retaining records need no payload slot");
    }
    assert_eq!(domain.payloads.len(), full);
}

#[test]
fn completions_wait_for_earlier_handoffs_and_store_answers() {
    let (mut domain, env) = root();
    domain.work.push(crate::boundary::Work::Forge(temper_engine_domain_forge::Event::BoxClosed));
    let event = Event::Worker(WorkerInput::Link { channel: Token::new(99), link: Link::Lost });
    let event = crate::try_step(&mut domain, &env, event).expect_err("earlier hand-off goes first");
    domain.work.pop().unwrap();
    domain.store_answers.push(Event::Store(crate::StoreInput::Switched));
    let event = crate::try_step(&mut domain, &env, event).expect_err("earlier store answer goes first");
    domain.store_answers.pop().unwrap();
    crate::try_step(&mut domain, &env, event).expect("completion follows the drained prefix");
}
