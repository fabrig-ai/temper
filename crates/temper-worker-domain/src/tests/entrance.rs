//! What the root refuses or answers at the entrance, and how a start comes
//! back decoded.

use super::fixtures::{ACCOUNT, ACCOUNT_RESET, assignment, fields, items, refused};
use super::harness::Harness;
use crate::{Event, Invalid, Request, agent, host, translate};
use alloc::boxed::Box;
use skein_lib::Duration;

#[test]
fn workspace_items_that_did_not_decode_are_refused_naming_them() {
    let mut h = Harness::new();
    let assign =
        Event::Assign { assignment: assignment(true, Box::new([])), items: Err(crate::wire::Undecodable), save: None };
    assert_eq!(&*h.step(assign), [refused(b"workspace items")]);
    assert_eq!(h.domain.host().hosted(), 0);
}

#[test]
#[expect(clippy::manual_map, reason = "explicit option match follows the step subset")]
fn workspace_items_are_refused_naming_the_item_or_carry_their_program_to_checkout() {
    let mut bound = Box::new([b'x'; 4096]);
    *bound.first_mut().expect("nonempty program") = b'/';
    let mut long = Box::new([b'x'; 4097]);
    *long.first_mut().expect("nonempty program") = b'/';
    for (row, program, duplicate, why) in [
        ("duplicate", None, true, Some(b"repository named twice".as_slice())),
        ("relative", Some(b"git".as_slice()), false, Some(b"repository git".as_slice())),
        ("past bound", Some(long.as_slice()), false, Some(b"repository git".as_slice())),
        ("NUL", Some(b"/bin/g\0it".as_slice()), false, Some(b"repository git".as_slice())),
        ("absolute", Some(b"/opt/fake/git".as_slice()), false, None),
        ("at bound", Some(bound.as_slice()), false, None),
        ("none", None, false, None),
    ] {
        let mut h = Harness::new();
        let mut workspace = if duplicate { items(&[b"app", b"app"]) } else { items(&[b"app"]) };
        workspace.repositories.first_mut().expect("one repository").git = match program {
            Some(program) => Some(Box::from(program)),
            None => None,
        };
        if let Some(why) = why {
            let assign = Event::Assign { assignment: assignment(true, Box::new([])), items: Ok(workspace), save: None };
            assert_eq!(&*h.step(assign), [refused(why)], "{row}");
            assert_eq!(h.domain.host().hosted(), 0, "{row}");
        } else {
            assert_eq!(crate::assignment::check(&workspace, None, &h.env.limits), Ok(()), "{row}");
            let spec = translate::spec(7, workspace);
            assert_eq!(
                spec.repositories.first().expect("one repository").git.as_deref(),
                program,
                "{row}: program reaches checkout"
            );
        }
    }
}

#[test]
fn workspace_items_without_a_workstream_are_refused_as_an_invalid_start() {
    let mut h = Harness::new();
    let assign = Event::Assign { assignment: assignment(false, Box::new([])), items: Ok(items(&[b"app"])), save: None };
    assert_eq!(&*h.step(assign), [refused(b"workstream")]);
}

#[test]
fn an_undecodable_call_name_refuses_the_start_as_invalid_naming_it() {
    let mut h = Harness::new();
    let (agent, _) = h.decode(Box::new([]));
    let rows = h.step(Event::Decoded { agent, start: Err(Invalid::CallName) });
    assert!(rows.contains(&refused(b"call name")));
}

#[test]
fn undecodable_delivery_evidence_refuses_the_start_as_invalid_naming_it() {
    let mut h = Harness::new();
    let (agent, _) = h.decode(Box::new([]));
    let rows = h.step(Event::Decoded { agent, start: Err(Invalid::Evidence) });
    assert!(rows.contains(&refused(b"delivery evidence")));
}

#[test]
fn a_start_refused_as_invalid_frees_its_slot_unacknowledged() {
    let mut h = Harness::new();
    let (agent, _) = h.decode(Box::new([]));
    h.step(Event::Decoded { agent, start: Err(Invalid::Charter) });
    assert_eq!(h.domain.host().hosted(), 0);
}

#[test]
fn an_assignment_hosted_already_is_a_wire_retry_and_dropped() {
    let mut h = Harness::new();
    h.decode(Box::new([]));
    let again = Event::Link(host::LinkDown::Assign { assignment: assignment(false, Box::new([])) });
    assert!(h.step(again).is_empty());
}

#[test]
fn a_worker_shutting_down_refuses_assignments_as_busy() {
    let mut h = Harness::new();
    h.step(Event::Shutdown);
    let rows = h.step(Event::Link(host::LinkDown::Assign { assignment: assignment(false, Box::new([])) }));
    let [Request::Answer { answer, .. }] = &*rows else { panic!("one answer: {rows:?}") };
    assert_eq!(answer.ending, host::Ending::Refused(host::Refusal::Busy));
}

#[test]
fn a_worker_shutting_down_still_says_its_slots() {
    let mut h = Harness::new();
    h.step(Event::Shutdown);
    let rows = h.step(Event::Ready);
    let [Request::Hello(hello)] = &*rows else { panic!("one hello: {rows:?}") };
    assert_eq!(hello.slots, 2);
}

#[test]
fn the_hello_states_the_configured_stop_bound_whole() {
    let mut h = Harness::new();
    let rows = h.step(Event::Ready);
    let [Request::Hello(hello)] = &*rows else { panic!("one hello: {rows:?}") };
    assert_eq!(hello.stop_bound, Duration::from_secs(20));
}

#[test]
fn a_worker_told_to_shut_down_with_nothing_left_says_stop_once() {
    let mut h = Harness::new();
    assert_eq!(&*h.step(Event::Shutdown), [Request::Stop]);
    assert!(h.step(Event::Lost).is_empty());
}

#[test]
fn an_exhausted_account_stays_a_failure_of_its_own() {
    let exhausted = agent::ModelFault::Limited { account: ACCOUNT, reset: Some(ACCOUNT_RESET) };
    assert_eq!(
        translate::run_failure(&agent::RunFailure::Model(exhausted)),
        host::RunFailure::Limited { account: ACCOUNT, reset: Some(ACCOUNT_RESET) }
    );
}

#[test]
fn a_delivery_is_exactly_a_title_and_a_body() {
    let both = fields(&[(b"title", b"a title"), (b"body", b"a body")]);
    let delivered = crate::smith_delivery::fields(both, 512).expect("a commit message");
    assert_eq!((&*delivered.title, &*delivered.body), (&b"a title"[..], &b"a body"[..]));
}

#[test]
fn a_delivery_with_a_field_beyond_its_title_and_body_is_refused() {
    let extra = fields(&[(b"title", b"t"), (b"body", b"b"), (b"tag", b"x")]);
    assert!(crate::smith_delivery::fields(extra, 512).is_none());
}

#[test]
fn a_delivery_with_an_empty_title_is_refused() {
    let empty = fields(&[(b"title", b""), (b"body", b"b")]);
    assert!(crate::smith_delivery::fields(empty, 512).is_none());
}

#[test]
fn landing_branch_is_bounded_and_translated_to_checkout() {
    for length in [0, 32, 33] {
        let mut h = Harness::new();
        let mut workspace = items(&[b"app"]);
        workspace.repositories[0].access = crate::wire::Access::Writable {
            push: Box::from(&b"topic"[..]),
            expected: Some([1; 32]),
            landing: Some(skein_lib::bytes::zeroed(length)),
        };
        if length == 32 {
            assert_eq!(crate::assignment::check(&workspace, None, &h.env.limits), Ok(()));
            let spec = translate::spec(7, workspace);
            assert_eq!(spec.repositories[0].landing.as_ref().unwrap().len(), length);
            assert_eq!(spec.repositories[0].expected.unwrap().raw(), [1; 32]);
        } else {
            let assign = Event::Assign { assignment: assignment(true, Box::new([])), items: Ok(workspace), save: None };
            assert_eq!(&*h.step(assign), [refused(b"landing branch")]);
        }
    }
}
