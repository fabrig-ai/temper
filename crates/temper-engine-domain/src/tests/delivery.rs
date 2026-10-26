use super::resume_fixture;
use crate::{CallKey, DeliveryRecord, DeliveryReply, Key, Record, Write};
use jig_domain_core as core;
use skein_lib::{Queue, ReplyTo, Token};

const KEY: CallKey = CallKey { task: 1, attempt: 1, completion: 2, position: 0 };
const RIGHT: Token = Token::new(7);

fn root() -> (crate::Domain, skein_lib::Env<crate::Limits>) {
    let (mut root, env) = resume_fixture::root();
    root.core.call_parts.insert(KEY, core::CallPart::Unavailable).unwrap();
    (root, env)
}
fn settle(root: &mut crate::Domain, env: &skein_lib::Env<crate::Limits>, reply: DeliveryReply) -> crate::Decision {
    use crate::route::host_route::{self, Flight, FlightRender};
    root.host_calls
        .insert(
            RIGHT,
            Flight {
                task: KEY.task,
                attempt: KEY.attempt,
                key: Some(KEY),
                render: FlightRender::Plain,
                name: b"call-name".as_slice().into(),
                tool: b"deliver".as_slice().into(),
            },
        )
        .unwrap();
    let answer = core::SettledAnswer::Delivery { outcome: reply.outcome() };
    host_route::rendered(root, ReplyTo::new(RIGHT), answer, Some(reply));
    let mut decision = crate::Decision::new(&env.limits.journal);
    crate::route::route_into(root, env, &mut decision);
    decision
}
fn writes(root: &mut crate::Domain, env: &skein_lib::Env<crate::Limits>, decision: crate::Decision) -> Box<[Write]> {
    let mut out = Queue::with_capacity(1);
    crate::accept(&mut root.journal, &mut root.core.counters, &env.limits.journal, decision, &mut out).unwrap();
    match out.pop() {
        Some(crate::JournalOutput::Commit { number, writes }) => {
            crate::committed(&mut root.journal, number);
            writes
        }
        Some(crate::JournalOutput::Deliver(_) | crate::JournalOutput::Stop | crate::JournalOutput::Now(_)) | None => {
            Box::new([])
        }
    }
}
#[test]
fn delivery_evidence_and_core_settlement_share_one_decision() {
    let (mut root, env) = root();
    let decision = settle(&mut root, &env, DeliveryReply::Nothing);
    let writes = writes(&mut root, &env, decision);
    let row = DeliveryRecord { key: KEY, reply: DeliveryReply::Nothing };
    assert_eq!(root.delivery_replies.get(&KEY), Some(&row.reply));
    assert!(writes.contains(&Write::Save(Record::Delivery { row: Box::new(row) })));
    let mut core_row = None;
    for write in writes {
        if let Write::Save(Record::Core(core::Record::Core(core::CoreRecord::Call(row)))) = write {
            core_row = Some(row);
        }
    }
    assert_eq!(
        core_row,
        Some(core::CallRecord {
            key: KEY,
            part: core::CallPart::Unavailable,
            settled: Some(core::SettledCall {
                serial: 1,
                tool: b"deliver".as_slice().into(),
                answer: core::SettledAnswer::Delivery { outcome: core::DeliveryOutcome::Nothing },
            })
        })
    );
}
#[test]
fn a_second_delivery_settlement_keeps_the_first_evidence() {
    let (mut root, env) = root();
    let first = settle(&mut root, &env, DeliveryReply::Nothing);
    writes(&mut root, &env, first);
    let second = settle(&mut root, &env, DeliveryReply::Stale);
    let writes = writes(&mut root, &env, second);
    assert_eq!(root.delivery_replies.get(&KEY), Some(&DeliveryReply::Nothing));
    let mut saved = None;
    for write in writes {
        if let Write::Save(Record::Delivery { row }) = write {
            saved = Some(row);
        }
    }
    assert_eq!(saved, None);
}
#[test]
fn retiring_a_core_call_erases_its_delivery_evidence_together() {
    let (mut root, env) = root();
    let first = settle(&mut root, &env, DeliveryReply::Nothing);
    writes(&mut root, &env, first);
    let mut output = Queue::with_capacity(1);
    for key in root.core.retire_calls(KEY.task, KEY.attempt + 1, 1, env.limits.call_records) {
        output.push(core::Request::Write(core::Write::Erase(core::Key::Core(core::CoreKey::Call(key)))));
    }
    let mut decision = crate::Decision::new(&env.limits.journal);
    crate::route::route_core_requests(&mut root, &env, &mut decision, core::Requests::Out(output));
    let writes = writes(&mut root, &env, decision);
    assert_eq!(root.delivery_replies.get(&KEY), None);
    assert!(writes.contains(&Write::Erase(Key::Delivery(KEY))));
    assert!(writes.contains(&Write::Erase(Key::Core(core::Key::Core(core::CoreKey::Call(KEY))))));
}
#[test]
fn restored_delivery_evidence_is_available_by_its_call_key() {
    let (mut root, env) = root();
    root.core
        .call_settled
        .insert(
            KEY,
            core::SettledCall {
                serial: 1,
                tool: b"deliver".as_slice().into(),
                answer: core::SettledAnswer::Delivery { outcome: core::DeliveryOutcome::Stale },
            },
        )
        .unwrap();
    crate::restart::restore_page_row(
        &mut root,
        &env,
        Record::Delivery { row: Box::new(DeliveryRecord { key: KEY, reply: DeliveryReply::Stale }) },
    );
    assert_eq!(root.delivery_replies.get(&KEY), Some(&DeliveryReply::Stale));
}
