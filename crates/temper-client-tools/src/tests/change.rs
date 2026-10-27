use super::fixtures as f;
use crate::{
    change,
    jig::{self, call::Parsed, digest::Digest},
};
use jig_client::{self as client, contract as c};
use skein_lib::{Duration, List};
use temper_client_protocol_wire::{self as wire, v1 as w};

pub(super) fn panel(step: w::Step) -> w::ChangePanel {
    let used = w::Used::new(&w::CEILINGS, w::UsedParts { used: 0, limit: 3 }).unwrap();
    w::ChangePanel::new(
        &w::CEILINGS,
        w::ChangePanelParts {
            task: 2,
            forge: 3,
            owner: b"qa".as_slice().into(),
            name: b"pilot-20261026".as_slice().into(),
            branch: b"temper/2-readme".as_slice().into(),
            landing: b"main".as_slice().into(),
            head: Some([17; 32]),
            pull: Some(
                w::Pull::new(
                    &w::CEILINGS,
                    w::PullParts {
                        number: 1,
                        address: b"http://127.0.0.1:3000/qa/pilot-20261026/pulls/1".as_slice().into(),
                    },
                )
                .unwrap(),
            ),
            step,
            hold: None,
            runs: None,
            ci: w::Ci::Pending,
            gates: List::with_capacity(0),
            repairs: used.clone(),
            resolutions: used,
            updates: w::Used::new(&w::CEILINGS, w::UsedParts { used: 0, limit: 5 }).unwrap(),
        },
    )
    .unwrap()
}
fn landed() -> w::Step {
    w::Step::Landed(w::Landed::new(&w::CEILINGS, w::LandedParts { merge: [34; 32] }).unwrap())
}
fn deliver(root: &mut f::Harness, panel: w::ChangePanel) {
    let subject = change::subject(2, 1, &root.limits);
    let token = root.watch_token(&subject);
    root.client(client::Event::Up {
        up: client::Up::FamilyDelivered {
            watch: token,
            record: client::Family {
                family: wire::FORGE,
                version: wire::CLIENT_VERSION,
                bytes: wire::change_panel(&panel),
            },
        },
    });
}
fn task(ended: bool, change_result: bool) -> c::Task {
    let mut task = f::goal(2, c::WaitsOn::Nothing);
    task.chip.executor = c::Executor::Procedure { connector: wire::FORGE, kind: 2 };
    task.result_contract = c::TaskContractDocument::Change { connector: wire::FORGE, kind: 1, words: f::LONG_WORDS };
    if ended {
        task.chip.phase = c::Phase::Ended;
        task.result = Some(c::TaskResult::Document(Box::new(c::Result {
            task: task.chip.clone(),
            ending: c::Ending::Done,
            value: if change_result {
                Some(c::ResultValue::Change {
                    kind: 1,
                    part: c::OwnerPart {
                        connector: wire::FORGE,
                        kind: c::OwnerPartKind::Resource,
                        key: c::OwnerPartKey::Number(1),
                    },
                    words: f::document_text(b"2222222222222222222222222222222222222222"),
                })
            } else {
                None
            },
            at: None,
            details: None,
            acts: List::with_capacity(0),
        })));
    }
    task
}
fn serve(root: &mut f::Harness, call: u32, ended: bool, change_result: bool) {
    root.serve(call, c::ReadResult::Value(c::ReadValue::Task(Box::new(task(ended, change_result)))));
}
fn contains(bytes: &[u8], part: &[u8]) {
    assert!(skein_lib::bytes::find(bytes, part).is_some(), "{bytes:?} missing {part:?}");
}
fn end_panel(root: &mut f::Harness, call: u32, end: c::WatchEnd) {
    let subject = change::subject(2, 1, &root.limits);
    root.end(call, &subject, end);
}
#[test]
fn change_calls_round_trip_and_reject_other_conditions() {
    for argv in [
        &["change", "show", "2"][..],
        &["change", "show", "2", "--project", "shop"],
        &[
            "change",
            "show",
            "2",
            "--project",
            "1",
            "--until",
            "landed",
            "--since",
            "0123456789abcdef",
            "--timeout",
            "1s",
        ],
        &["change", "show", "2", "--until", "ci:failed"],
    ] {
        let call = f::call(argv);
        assert_eq!(f::printed(&call), Parsed::Call { call: call.clone(), json: false });
        assert_eq!(f::from_json(&jig::json::write_call(crate::table::TOOLS, &call).unwrap()), call);
    }
    for argv in [
        &["change", "show", "0"][..],
        &["change", "show", "2", "--until", "ended"],
        &["change", "show", "2", "--until", "ci:unknown"],
        &["change", "show", "2", "--key", "no"],
    ] {
        f::parse(argv).unwrap_err();
    }
}
#[test]
fn checking_text_and_json_name_pull_head_gates_bounds_and_same_digest() {
    let mut root = f::signed_in(f::PARTY);
    let call = root.submit(&["change", "show", "2", "--project", "1"]);
    deliver(&mut root, panel(w::Step::Checking));
    let text = root.answer(call).unwrap();
    assert_eq!(text.status, 0);
    contains(&text.stdout, b"change 2 \xc2\xb7 qa/pilot-20261026 on forge 3 \xc2\xb7 temper/2-readme into main\n");
    contains(&text.stdout, b"pull request #1 \xc2\xb7 http://127.0.0.1:3000/qa/pilot-20261026/pulls/1\nstep: checking\nhead 1111111111111111111111111111111111111111 \xc2\xb7 CI pending\ngates: none\n");
    contains(&text.stdout, b"repairs 0 of 3 \xc2\xb7 resolutions 0 of 3 \xc2\xb7 updates 0 of 5\n");
    contains(&text.stdout, b"next: temper ctl DIR change show 2 --until landed\n");
    let call = root.submit(&["change", "show", "2", "--project", "1", "--json"]);
    deliver(&mut root, panel(w::Step::Checking));
    let json = root.answer(call).unwrap();
    let expected = br#"{"task":2,"forge":3,"owner":"qa","name":"pilot-20261026","branch":"temper/2-readme","landing":"main","head":"1111111111111111111111111111111111111111","pull":{"number":1,"address":"http://127.0.0.1:3000/qa/pilot-20261026/pulls/1"},"step":"checking","merge":null,"queue":null,"repair":null,"hold":null,"runs":null,"ci":"pending","gates":[],"repairs":{"used":0,"limit":3},"resolutions":{"used":0,"limit":3},"updates":{"used":0,"limit":5}"#;
    assert!(json.stdout.starts_with(expected));
    let mut document = skein_lib::Writer::new(expected.len() + 1);
    document.put(expected).unwrap();
    document.put(b"}").unwrap();
    let digest = Digest::of(&document.finish()).hex();
    contains(&json.stdout, &digest);
    contains(&text.stdout, &digest);
    assert_eq!(root.watches.len(), 1, "only me remains watched");
}
#[test]
fn landed_and_held_repair_panels_keep_whole_facts() {
    for json in [false, true] {
        let limits = f::limits_root();
        let answer = change::print(Some(&panel(landed())), json, None, b"", &limits);
        assert_eq!(answer.status, 0);
        contains(&answer.stdout, b"2222222222222222222222222222222222222222");
        assert!(skein_lib::bytes::find(&answer.stdout, b"next:").is_none());
        let mut parts = panel(w::Step::Repairing(
            w::Repairing::new(&w::CEILINGS, w::RepairingParts { reason: w::Reason::Ci }).unwrap(),
        ))
        .into_parts();
        parts.hold = Some(w::Hold::Stalled);
        parts.runs = Some(7);
        parts.gates = List::with_capacity(1);
        parts
            .gates
            .push(
                w::Gate::new(
                    &w::CEILINGS,
                    w::GateParts { number: 1, kind: w::GateKind::Review, blocks: true, verdict: w::Verdict::Pending },
                )
                .unwrap(),
            )
            .unwrap();
        let answer = change::print(Some(&w::ChangePanel::new(&w::CEILINGS, parts).unwrap()), json, None, b"", &limits);
        if json {
            contains(&answer.stdout, br#""repair":"ci","hold":"stalled","runs":7"#);
            contains(&answer.stdout, br#""gates":[{"number":1,"kind":"review","blocks":true,"verdict":"pending"}]"#);
        } else {
            contains(&answer.stdout, b"held: its wait stalled at repairing\nrunning: task 7\n");
            contains(&answer.stdout, b"repair: Repairing: CI failed at its head.");
        }
    }
}
#[test]
fn absent_project_reads_the_task_project_before_opening_the_panel() {
    let mut root = f::signed_in(f::PARTY);
    root.pending_reads.push(1);
    let call = root.submit(&["change", "show", "2"]);
    assert_eq!(root.reads[&call].1, c::Read::Task { task: 2 });
    assert_eq!(root.watches.len(), 1);
    serve(&mut root, call, false, false);
    deliver(&mut root, panel(w::Step::Checking));
    assert_eq!(root.answer(call).unwrap().status, 0);
}
#[test]
fn a_read_of_an_unknown_panel_exits_two() {
    let mut root = f::signed_in(f::PARTY);
    let call = root.submit(&["change", "show", "2", "--project", "1"]);
    end_panel(&mut root, call, c::WatchEnd::Unknown);
    let answer = root.answer(call).unwrap();
    assert_eq!(answer.status, 2);
    contains(&answer.stderr, b"no change panel for task 2");
}
#[test]
fn waits_test_each_delivery_including_the_first() {
    for (condition, ci) in [
        ("pull", w::Ci::Pending),
        ("ci:passed", w::Ci::Passed),
        ("ci:pending", w::Ci::Pending),
        ("ci:failed", w::Ci::Failed),
    ] {
        let mut root = f::signed_in(f::PARTY);
        let call = root.submit(&["change", "show", "2", "--project", "1", "--until", condition, "--json"]);
        let mut parts = panel(w::Step::Checking).into_parts();
        parts.ci = ci;
        deliver(&mut root, w::ChangePanel::new(&w::CEILINGS, parts).unwrap());
        let answer = root.answer(call).unwrap();
        assert_eq!(answer.status, 0);
        contains(&answer.stdout, br#""met":true"#);
    }
    let mut root = f::signed_in(f::PARTY);
    let call = root.submit(&["change", "show", "2", "--project", "1", "--until", "landed"]);
    deliver(&mut root, panel(w::Step::Checking));
    assert!(root.answer(call).is_none());
    deliver(&mut root, panel(landed()));
    assert_eq!(root.answer(call).unwrap().status, 0);
}
#[test]
fn a_wait_reopens_the_last_panel_after_the_watch_ends_at_landing() {
    for end in [c::WatchEnd::Unknown, c::WatchEnd::SubjectEnded] {
        let mut root = f::signed_in(f::PARTY);
        let call = root.submit(&["change", "show", "2", "--project", "1", "--until", "landed", "--json"]);
        deliver(&mut root, panel(w::Step::Checking));
        end_panel(&mut root, call, end);
        assert!(root.answer(call).is_none());
        serve(&mut root, call, true, true);
        deliver(&mut root, panel(landed()));
        let answer = root.answer(call).unwrap();
        assert_eq!(answer.status, 0);
        contains(&answer.stdout, br#""step":"landed"#);
    }
}
#[test]
fn live_task_waits_for_the_next_document_then_reopens_the_panel() {
    let mut root = f::signed_in(f::PARTY);
    let call = root.submit(&["change", "show", "2", "--project", "1", "--until", "landed"]);
    end_panel(&mut root, call, c::WatchEnd::Unknown);
    serve(&mut root, call, false, false);
    let subject = client::Watchable::Jig(c::Subject::Task { task: 2 });
    let token = root.watch_token(&subject);
    root.client(client::Event::Up {
        up: client::Up::Watched {
            watch: token,
            event: c::WatchEvent::Delivered(c::Delivery::Task(Box::new(task(false, false)))),
        },
    });
    assert_eq!(root.watches.len(), 2, "me and task; no premature panel retry");
    let mut next = task(false, false);
    next.activity += 1;
    root.client(client::Event::Up {
        up: client::Up::Watched { watch: token, event: c::WatchEvent::Delivered(c::Delivery::Task(Box::new(next))) },
    });
    deliver(&mut root, panel(landed()));
    assert_eq!(root.answer(call).unwrap().status, 0);
}
#[test]
fn unmet_final_panel_exits_three_and_no_change_result_exits_two() {
    for change_result in [false, true] {
        let mut root = f::signed_in(f::PARTY);
        let call = root.submit(&["change", "show", "2", "--project", "1", "--until", "ci:passed", "--json"]);
        deliver(&mut root, panel(w::Step::Checking));
        end_panel(&mut root, call, c::WatchEnd::SubjectEnded);
        serve(&mut root, call, true, change_result);
        if change_result {
            deliver(&mut root, panel(landed()));
        }
        let answer = root.answer(call).unwrap();
        assert_eq!(answer.status, if change_result { 3 } else { 2 });
        if change_result {
            contains(&answer.stdout, br#""met":false,"why":"ended"#);
        } else {
            contains(&answer.stderr, b"no change panel for task 2");
        }
    }
}
#[test]
fn since_and_condition_both_must_hold_and_timeout_keeps_last_digest() {
    let mut root = f::signed_in(f::PARTY);
    let answer = change::print(Some(&panel(w::Step::Checking)), true, None, b"", &root.limits);
    let marker = b"\"digest\":\"";
    let start = skein_lib::bytes::find(&answer.stdout, marker).unwrap() + marker.len();
    let digest = &answer.stdout[start..start + 16];
    let mut call = f::call(&[
        "change",
        "show",
        "2",
        "--project",
        "1",
        "--since",
        "0000000000000000",
        "--until",
        "pull",
        "--timeout",
        "1s",
    ]);
    *call.values.get_mut(2).unwrap() = Some(jig::call::Values::One(
        jig::argv::value::decode(crate::table::TOOLS, jig::table::Kind::Digest, jig::table::Bound::None, digest)
            .unwrap(),
    ));
    let call = root.submit_prepared(call, true, None);
    deliver(&mut root, panel(w::Step::Checking));
    assert!(root.answer(call).is_none());
    root.now = root.now.checked_add(Duration::from_secs(1)).unwrap();
    root.fire();
    let answer = root.answer(call).unwrap();
    assert_eq!(answer.status, 3);
    contains(&answer.stdout, digest);
    contains(&answer.stdout, b"timeout");
}

#[test]
fn a_held_wait_and_a_wait_without_a_condition_use_the_panel_facts() {
    let mut root = f::signed_in(f::PARTY);
    let call = root.submit(&["change", "show", "2", "--project", "1", "--until", "held"]);
    let mut parts = panel(w::Step::Checking).into_parts();
    parts.hold = Some(w::Hold::Stalled);
    deliver(&mut root, w::ChangePanel::new(&w::CEILINGS, parts).unwrap());
    assert_eq!(root.answer(call).unwrap().status, 0);
    let call = root.submit(&["change", "show", "2", "--project", "1", "--until", "changed"]);
    deliver(&mut root, panel(w::Step::Checking));
    assert!(root.answer(call).is_none(), "first delivery only establishes the digest");
    deliver(&mut root, panel(w::Step::Checking));
    assert!(root.answer(call).is_none(), "repeated digest is no change");
    deliver(&mut root, panel(w::Step::Gating));
    assert_eq!(root.answer(call).unwrap().status, 0);
}

#[test]
fn changed_with_since_waits_for_a_panel_change_after_the_baseline() {
    let mut root = f::signed_in(f::PARTY);
    let read = root.submit(&["change", "show", "2", "--project", "1", "--json"]);
    deliver(&mut root, panel(w::Step::Gating));
    let digest = f::last_digest(&root.answer(read).unwrap().stdout);
    let call = root.wait_since(&["change", "show", "2", "--project", "1"], &["--until", "changed"], &digest);
    for step in [w::Step::Checking, w::Step::Gating] {
        deliver(&mut root, panel(step));
        assert!(root.answer(call).is_none());
    }
    deliver(&mut root, panel(w::Step::Checking));
    assert_eq!(root.answer(call).unwrap().status, 0);
}
