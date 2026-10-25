use super::fixtures;
use jig_domain_core as core;
use jig_domain_host as host;
use skein_lib::{ReplyTo, Token, Writer};
use temper_engine_domain as root;

const TO: ReplyTo = ReplyTo::new(Token::new(9));
fn call(activation: u64, tool: &[u8], input: &[u8]) -> jig_domain_fleet::Call {
    let name = smith_channel::CallName::new(
        &smith_channel::CEILINGS,
        smith_channel::CallNameParts { activation, completion: 2, position: 1 },
    )
    .unwrap();
    let mut writer = Writer::new(16);
    name.encode(&mut writer).unwrap();
    jig_domain_fleet::Call {
        name: writer.finish(),
        tool: tool.into(),
        input: input.into(),
        writes: false,
        deadline: skein_lib::Duration::from_secs(1),
    }
}
#[test]
fn a_call_is_decoded_into_the_roots_tool() {
    let event = crate::tools::decode(TO, 3, 7, &call(7, b"release", br#"{"target":5,"revision":1}"#));
    let root::Event::Worker(root::WorkerInput::DecodedHostCall { to, body }) = event else { panic!("decoded call") };
    assert_eq!(to, TO);
    assert_eq!(
        body,
        root::Call { completion: 2, position: 1, tool: root::Tool::Release { target: 5, revision: 1, again: None } }
    );
}
#[test]
fn a_call_of_another_activation_is_answered_an_error() {
    let event = crate::tools::decode(TO, 3, 8, &call(7, b"release", br#"{"target":5,"revision":1}"#));
    let root::Event::Worker(root::WorkerInput::RenderedHostCall { to, answer }) = event else {
        panic!("rendered error")
    };
    assert_eq!(to, TO);
    assert_eq!(answer, core::SettledAnswer::Host { error: true, body: b"host call out of range".as_slice().into() });
}
#[test]
fn an_unknown_tool_is_answered_an_error_with_no_commit() {
    let (mut service, run) = fixtures::claimed_chat();
    let commits = service.engine.as_mut().expect("engine").domain.deployment().commits;
    let call = call(run.attempt, b"no_such_tool", b"{}");
    service.engine.as_mut().expect("engine").step(root::Event::Worker(root::WorkerInput::Link {
        channel: Token::new(7),
        link: root::Link::Up {
            parts: root::Parts::none(),
            up: host::LinkUp::Call {
                run: Token::new(run.task),
                attempt: Token::new(run.attempt),
                call: call.name.clone(),
                tool: call.tool,
                input: call.input,
                writes: false,
                deadline: call.deadline,
            },
        },
    }));
    let mut delivered = skein_lib::Queue::with_capacity(32);
    fixtures::settle_root(&mut service, &mut delivered);
    let mut answer = None;
    for _ in 0..delivered.len() {
        let root::Delivery::Link { down: host::LinkDown::Relayed { call: name, answer: relayed, .. }, .. } =
            delivered.pop().unwrap()
        else {
            continue;
        };
        assert_eq!(name, call.name);
        assert!(answer.replace(relayed).is_none(), "one answer");
    }
    assert_eq!(answer, Some(host::Relayed::Answer { error: true, text: b"unknown tool".as_slice().into() }));
    assert_eq!(service.engine.as_ref().expect("engine").domain.deployment().commits, commits);
    assert_eq!(service.end(), None);
}
#[test]
fn a_decided_answer_is_rendered_as_smiths_text() {
    let event = crate::tools::render(TO, &root::CallAnswer::NoteWritten { name: 4, revision: 2 });
    let root::Event::Worker(root::WorkerInput::RenderedHostCall { to, answer }) = event else {
        panic!("rendered answer")
    };
    assert_eq!(to, TO);
    assert_eq!(
        answer,
        core::SettledAnswer::Host { error: false, body: b"Note 4 revision 2 written".as_slice().into() }
    );
}
#[test]
fn a_result_that_does_not_decode_fails_its_attempt_invalid() {
    for (bytes, expected) in [
        (b"".as_slice(), jig_domain_tasks::End::Failed(jig_domain_tasks::Class::Invalid)),
        (b"not a result", jig_domain_tasks::End::Failed(jig_domain_tasks::Class::Invalid)),
        (&[0, 0, 0, 99], jig_domain_tasks::End::Failed(jig_domain_tasks::Class::Invalid)),
    ] {
        let event = crate::tools::result(Token::new(3), bytes, None);
        let root::Event::Worker(root::WorkerInput::ResultRead { payload, end }) = event else {
            panic!("read result {bytes:?}")
        };
        assert_eq!(payload, Token::new(3), "{bytes:?}");
        assert_eq!(end, expected, "{bytes:?}");
    }
}

#[test]
fn each_invalid_call_names_its_problem() {
    let oversized = skein_lib::bytes::zeroed(smith_domain_run::HostInput::CAPACITY + 1);
    for (input, expected) in [
        (b"[]".as_slice(), b"malformed host call".as_slice()),
        (b"{broken}", b"malformed host call"),
        (b"{}", b"host call member missing"),
        (br#"{"target":"five"}"#, b"host call member of wrong type"),
        (br#"{"target":0}"#, b"host call out of range"),
        (oversized.as_ref(), b"host call too large"),
    ] {
        let event = crate::tools::decode(TO, 3, 7, &call(7, b"release", input));
        let root::Event::Worker(root::WorkerInput::RenderedHostCall { answer, .. }) = event else {
            panic!("invalid input {input:?}")
        };
        assert_eq!(answer, core::SettledAnswer::Host { error: true, body: expected.into() }, "{input:?}");
    }
}
#[test]
fn a_report_result_keeps_its_words() {
    let report = smith_domain_run::outcome::Declared::Report(smith_domain_run::outcome::Report {
        text: b"finished".as_slice().into(),
        fields: Box::new([]),
    });
    let bytes = smith_protocol_channel::encode_result(&report, &smith_charter::CEILINGS).unwrap();
    let event = crate::tools::result(Token::new(3), &bytes, None);
    let root::Event::Worker(root::WorkerInput::ResultRead { payload, end }) = event else { panic!("report read") };
    assert_eq!(payload, Token::new(3));
    assert_eq!(
        end,
        jig_domain_tasks::End::Finished {
            result: jig_domain_tasks::TaskResult::Report { words: b"finished".as_slice().into() },
            cancel_delegates: false,
        }
    );
}
#[test]
fn a_change_result_without_settled_resource_is_invalid() {
    let change =
        smith_domain_run::outcome::Declared::Change(smith_domain_run::outcome::Change { fields: Box::new([]) });
    let bytes = smith_protocol_channel::encode_result(&change, &smith_charter::CEILINGS).unwrap();
    let event = crate::tools::result(Token::new(3), &bytes, None);
    let root::Event::Worker(root::WorkerInput::ResultRead { end, .. }) = event else { panic!("change read") };
    assert_eq!(end, jig_domain_tasks::End::Failed(jig_domain_tasks::Class::Invalid));
}

#[test]
fn a_change_result_keeps_the_settled_resource() {
    let change = smith_domain_run::outcome::Declared::Change(smith_domain_run::outcome::Change {
        fields: Box::new([smith_domain_run::outcome::Field {
            name: b"body".as_slice().into(),
            value: b"Changed".as_slice().into(),
        }]),
    });
    let bytes = smith_protocol_channel::encode_result(&change, &smith_charter::CEILINGS).unwrap();
    let resource = temper_engine_smith::ChangeResource { connector: 1, kind: 2, resource: 42 };
    let event = crate::tools::result(Token::new(3), &bytes, Some(resource));
    let root::Event::Worker(root::WorkerInput::ResultRead { payload, end }) = event else { panic!("change read") };
    assert_eq!(payload, Token::new(3));
    assert_eq!(
        end,
        jig_domain_tasks::End::Finished {
            result: jig_domain_tasks::TaskResult::Change {
                connector: 1,
                kind: 2,
                resource: 42,
                words: b"Changed".as_slice().into(),
            },
            cancel_delegates: false,
        }
    );
}
#[test]
fn each_settled_delivery_becomes_the_hubs_answer() {
    for (outcome, error, text) in [
        (core::DeliveryOutcome::Delivered, false, b"delivered: evidence".as_slice()),
        (core::DeliveryOutcome::Nothing, false, b"nothing to deliver: evidence"),
        (core::DeliveryOutcome::Stale, true, b"stale delivery: evidence"),
        (core::DeliveryOutcome::Refused, true, b"delivery refused: evidence"),
        (core::DeliveryOutcome::Invalid, true, b"invalid delivery: evidence"),
        (core::DeliveryOutcome::Failed, true, b"delivery failed: evidence"),
    ] {
        let answer =
            crate::tools::settled(core::SettledAnswer::Delivery { outcome, evidence: b"evidence".as_slice().into() });
        assert_eq!(answer, host::Relayed::Answer { error, text: text.into() }, "{outcome:?}");
    }
}

#[test]
#[expect(clippy::wildcard_enum_match_arm, reason = "unexpected test outputs fail immediately")]
fn owned_capacity_refusal_preserves_the_opaque_address_without_a_tool_effect() {
    let (mut service, run) = fixtures::claimed_chat();
    let engine = service.engine.as_mut().expect("engine");
    let commits = engine.domain.deployment().commits;
    let mut decoded = 0_u32;
    let mut busy = 0_u32;
    for (channel, name) in [(7, b"first".as_slice()), (7, b"second"), (8, b"foreign"), (7, b"opaque capacity name")] {
        engine.step(root::Event::Worker(root::WorkerInput::Link {
            channel: Token::new(channel),
            link: root::Link::Up {
                parts: root::Parts::none(),
                up: host::LinkUp::Call {
                    run: Token::new(run.task),
                    attempt: Token::new(run.attempt),
                    call: name.into(),
                    tool: b"no_such_tool".as_slice().into(),
                    input: b"{}".as_slice().into(),
                    writes: true,
                    deadline: skein_lib::Duration::from_secs(1),
                },
            },
        }));
        assert!(engine.pending.is_none(), "inputs enter only after the preceding route settles");
        let mut observed = false;
        for _ in 0_u32..256 {
            root::release(&mut engine.domain, &engine.env, &mut engine.requests);
            if engine.requests.is_empty() {
                engine.step(root::Event::Resume);
                root::release(&mut engine.domain, &engine.env, &mut engine.requests);
            }
            while let Some(request) = engine.requests.pop() {
                if let root::Request::Worker(worker) = request {
                    observed = true;
                    match worker {
                        root::WorkerRequest::Host(host) => {
                            if let root::HostRequest::Decode { .. } = *host {
                                decoded += 1;
                            }
                        }
                        root::WorkerRequest::Deliver(root::Delivery::Link {
                            channel,
                            down: host::LinkDown::Relayed { run: task, attempt, call, answer },
                        }) => {
                            assert_eq!(
                                (channel, task, attempt),
                                (Token::new(7), Token::new(run.task), Token::new(run.attempt))
                            );
                            assert_eq!(call.as_ref(), b"opaque capacity name");
                            assert_eq!(answer, host::Relayed::Busy);
                            busy += 1;
                        }
                        _ => panic!("unexpected worker output: {worker:?}"),
                    }
                }
            }
            if observed {
                break;
            }
        }
        assert!(observed, "every input produces its own bounded route output");
    }
    assert_eq!((decoded, busy), (2, 1), "foreign calls stay discarded; capacity cannot decode a tool");
    assert_eq!(engine.domain.deployment().commits, commits, "no effect or commit from refusal");
}
