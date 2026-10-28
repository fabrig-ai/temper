use super::fixtures as f;
use crate::{jig, landing};
use jig_client::{self as client, contract as c};
use skein_lib::List;
use temper_client_protocol_wire::{self as wire, v1 as w};

fn panel(phase: w::LandingPhase) -> w::LandingPanel {
    let mut gates = List::with_capacity(1);
    gates
        .push(
            w::GateProof::new(
                &w::CEILINGS,
                w::GateProofParts {
                    name: b"gate".as_slice().into(),
                    blocking: true,
                    key: Some([0x33; 32]),
                    proof: w::Proof::Dispatched(
                        w::Dispatched::new(&w::CEILINGS, w::DispatchedParts { attempt: 1, run: Some(7) }).unwrap(),
                    ),
                    faults: 0,
                },
            )
            .unwrap(),
        )
        .unwrap();
    let mut members = List::with_capacity(1);
    members
        .push(
            w::Member::new(
                &w::CEILINGS,
                w::MemberParts {
                    change: 51,
                    forge: 1,
                    owner: b"work".as_slice().into(),
                    name: b"hello".as_slice().into(),
                    branch: b"temper/51".as_slice().into(),
                    landing: b"main".as_slice().into(),
                    pull: Some(
                        w::Pull::new(
                            &w::CEILINGS,
                            w::PullParts {
                                number: 3,
                                address: b"http://127.0.0.1:3000/work/hello/pulls/3".as_slice().into(),
                            },
                        )
                        .unwrap(),
                    ),
                    base: Some([0x22; 32]),
                    candidate: Some([0x11; 32]),
                    landed: if phase == w::LandingPhase::Landed { Some([0x11; 32]) } else { None },
                    gates,
                },
            )
            .unwrap(),
        )
        .unwrap();
    let hold = if phase == w::LandingPhase::Held {
        Some(w::LandingHold::Faults(
            w::HeldFaults::new(&w::CEILINGS, w::HeldFaultsParts { member: 0, gate: 0 }).unwrap(),
        ))
    } else {
        None
    };
    w::LandingPanel::new(
        &w::CEILINGS,
        w::LandingPanelParts {
            task: 61,
            project: 1,
            phase,
            hold,
            members,
            preparation: 1,
            stale_proofs: 0,
            turn: None,
        },
    )
    .unwrap()
}
fn deliver(root: &mut f::Harness, subject: client::Watchable, bytes: Box<[u8]>) {
    let token = root.watch_token(&subject);
    root.client(client::Event::Up {
        up: client::Up::FamilyDelivered {
            watch: token,
            record: client::Family { family: wire::FORGE, version: wire::CLIENT_VERSION, bytes },
        },
    });
}
fn read_panel(panel: &w::LandingPanel, json: bool) -> jig::Printed {
    let mut root = f::signed_in(f::PARTY);
    let call = root.submit(if json {
        &["landing", "show", "61", "--project", "1", "--json"]
    } else {
        &["landing", "show", "61", "--project", "1"]
    });
    let subject = landing::subject(Some(61), 1, false, &root.limits);
    deliver(&mut root, subject, wire::landing_panel(panel));
    let answer = root.answer(call).unwrap();
    assert_eq!(answer.status, 0);
    assert_eq!(root.watches.len(), 1, "only me remains watched");
    answer
}
#[test]
fn a_landing_prints_its_members_gates_and_turn() {
    f::check_golden("landing_proving", &read_panel(&panel(w::LandingPhase::Proving), false).stdout);
}
#[test]
fn a_landed_landing_prints_main_at_its_candidate() {
    f::check_golden("landing_landed", &read_panel(&panel(w::LandingPhase::Landed), false).stdout);
}
#[test]
fn a_held_landing_names_its_gate() {
    f::check_golden("landing_held", &read_panel(&panel(w::LandingPhase::Held), false).stdout);
}
fn with_gate(proof: w::Proof, key: Option<[u8; 32]>, blocking: bool, faults: u32) -> w::LandingPanel {
    let mut parts = panel(w::LandingPhase::Proving).into_parts();
    let mut member = parts.members.get(0).unwrap().clone().into_parts();
    let mut gate = member.gates.get(0).unwrap().clone().into_parts();
    gate.proof = proof;
    gate.key = key;
    gate.blocking = blocking;
    gate.faults = faults;
    member.gates = List::with_capacity(1);
    member.gates.push(w::GateProof::new(&w::CEILINGS, gate).unwrap()).unwrap();
    parts.members = List::with_capacity(1);
    parts.members.push(w::Member::new(&w::CEILINGS, member).unwrap()).unwrap();
    w::LandingPanel::new(&w::CEILINGS, parts).unwrap()
}
fn receipt() -> w::Receipted {
    w::Receipted::new(
        &w::CEILINGS,
        w::ReceiptedParts { run: 7, number: 3, address: b"http://127.0.0.1:3000/work/hello/pulls/3".as_slice().into() },
    )
    .unwrap()
}
#[test]
fn a_gate_passed_by_receipt_prints_its_run_and_address() {
    let panel = with_gate(w::Proof::Passed(receipt()), Some([0x33; 32]), true, 0);
    let answer = read_panel(&panel, false);
    assert!(contains(
        &answer.stdout,
        "  gate gate, blocking: key 3333333333333333, passed by run 7 · http://127.0.0.1:3000/work/hello/pulls/3\n"
    ));
    let json = read_panel(&panel, true);
    assert!(contains(
        &json.stdout,
        "\"proof\":{\"passed\":{\"run\":7,\"number\":3,\"address\":\"http://127.0.0.1:3000/work/hello/pulls/3\"}}"
    ));
}
#[test]
fn every_gate_proof_prints_its_state_and_runner_faults() {
    for (proof, key, expected) in [
        (w::Proof::Unkeyed, None, "no key yet"),
        (w::Proof::Keyed, Some([0x33; 32]), "key 3333333333333333, keyed"),
        (w::Proof::Waiting, Some([0x33; 32]), "key 3333333333333333, waiting on another landing's run"),
        (
            w::Proof::Dispatched(
                w::Dispatched::new(&w::CEILINGS, w::DispatchedParts { attempt: 1, run: None }).unwrap(),
            ),
            Some([0x33; 32]),
            "key 3333333333333333, dispatched (try 1)",
        ),
        (
            w::Proof::Failed(receipt()),
            Some([0x33; 32]),
            "key 3333333333333333, failed by run 7 · http://127.0.0.1:3000/work/hello/pulls/3",
        ),
    ] {
        let answer = read_panel(&with_gate(proof, key, false, 2), false);
        assert!(contains(&answer.stdout, "  gate gate, advisory: "));
        assert!(contains(&answer.stdout, ", 2 runner faults\n"));
        assert!(contains(&answer.stdout, expected));
    }
}
#[test]
fn a_landing_the_engine_does_not_know_fails_with_exit_2() {
    for end in [c::WatchEnd::Unknown, c::WatchEnd::SubjectEnded] {
        let mut root = f::signed_in(f::PARTY);
        let call = root.submit(&["landing", "show", "61", "--project", "1"]);
        root.end(call, &landing::subject(Some(61), 1, false, &root.limits), end);
        let answer = root.answer(call).unwrap();
        assert_eq!(answer.status, 2);
        assert_eq!(answer.stderr.as_ref(), b"no landing for task 61\n");
        assert_eq!(root.watches.len(), 1);
    }
}
fn list(turn: Option<u64>, tasks: &[u64]) -> w::LandingsPanel {
    let mut landings = List::with_capacity(u32::try_from(tasks.len()).unwrap());
    for task in tasks {
        let mut changes = List::with_capacity(1);
        changes.push(task.checked_sub(10).unwrap()).unwrap();
        let mut repositories = List::with_capacity(1);
        repositories.push(b"work/hello".as_slice().into()).unwrap();
        let mut landed = List::with_capacity(1);
        if *task != 61 {
            landed.push([0x11; 32]).unwrap();
        }
        landings
            .push(
                w::LandingLine::new(
                    &w::CEILINGS,
                    w::LandingLineParts {
                        task: *task,
                        phase: if *task == 61 { w::LandingPhase::Turn } else { w::LandingPhase::Landed },
                        changes,
                        repositories,
                        landed,
                    },
                )
                .unwrap(),
            )
            .unwrap();
    }
    w::LandingsPanel::new(
        &w::CEILINGS,
        w::LandingsPanelParts {
            newer_than: None,
            logged: List::with_capacity(0),
            after: None,
            project: 1,
            turn,
            landings,
        },
    )
    .unwrap()
}
fn read_list(panel: &w::LandingsPanel, json: bool) -> jig::Printed {
    let mut root = f::signed_in(f::PARTY);
    let call = root.submit(if json {
        &["landing", "list", "--project", "1", "--ended", "--json"]
    } else {
        &["landing", "list", "--project", "1", "--ended"]
    });
    let subject = landing::subject(None, 1, true, &root.limits);
    deliver(&mut root, subject, wire::landings_panel(panel));
    let answer = root.answer(call).unwrap();
    assert_eq!(answer.status, 0);
    assert_eq!(root.watches.len(), 1);
    answer
}
#[test]
fn landings_print_the_turns_holder_first_then_newest() {
    f::check_golden("landings_live", &read_list(&list(Some(61), &[60, 61]), false).stdout);
    let answer = read_list(&list(Some(61), &[62, 60, 61, 63]), false);
    let mut previous = 0;
    for prefix in ["\nlanding 61 ·", "\nlanding 63 ·", "\nlanding 62 ·", "\nlanding 60 ·"] {
        let at = skein_lib::bytes::find(&answer.stdout, prefix.as_bytes()).unwrap();
        assert!(at > previous);
        previous = at;
    }
}
#[test]
fn landings_with_none_print_no_landings() {
    assert_eq!(
        read_list(&list(None, &[]), false).stdout.as_ref(),
        "landings of project 1 · turn: free\nno landings\nno more\n".as_bytes()
    );
    for json in [false, true] {
        let mut root = f::signed_in(f::PARTY);
        let call = root.submit(if json {
            &["landing", "list", "--project", "1", "--json"]
        } else {
            &["landing", "list", "--project", "1"]
        });
        root.end(call, &landing::subject(None, 1, false, &root.limits), c::WatchEnd::Unknown);
        assert_eq!(root.answer(call).unwrap().stdout, read_list(&list(None, &[]), json).stdout);
    }
}
#[test]
fn landing_json_prints_the_panel_record() {
    f::check_golden("landing_json", &read_panel(&panel(w::LandingPhase::Proving), true).stdout);
    f::check_golden("landings_json", &read_list(&list(Some(61), &[60, 61]), true).stdout);
}
#[test]
fn an_absent_project_reads_the_task_and_a_project_name_resolves_through_me() {
    let mut root = f::signed_in(f::PARTY);
    root.pending_reads.push(1);
    let call = root.submit(&["landing", "show", "61"]);
    assert_eq!(root.reads[&call].1, c::Read::Task { task: 61 });
    let task = f::goal(61, c::WaitsOn::Nothing);
    root.serve(call, c::ReadResult::Value(c::ReadValue::Task(Box::new(task))));
    let subject = landing::subject(Some(61), 1, false, &root.limits);
    deliver(&mut root, subject, wire::landing_panel(&panel(w::LandingPhase::Proving)));
    assert_eq!(root.answer(call).unwrap().status, 0);
    let mut me = f::me(f::PARTY);
    me.projects.get_mut(0).unwrap().name = f::document_text(b"work");
    root.deliver(c::Delivery::Me(Box::new(me)));
    let call = root.submit(&["landing", "show", "61", "--project", "work"]);
    let subject = landing::subject(Some(61), 1, false, &root.limits);
    deliver(&mut root, subject, wire::landing_panel(&panel(w::LandingPhase::Proving)));
    assert_eq!(root.answer(call).unwrap().status, 0);
}
#[test]
fn landing_calls_round_trip_and_reject_waits_and_zero_tasks() {
    for argv in [
        &["landing", "show", "61"][..],
        &["landing", "show", "61", "--project", "work"],
        &["landing", "list", "--project", "1", "--ended"],
    ] {
        let call = f::call(argv);
        assert_eq!(f::printed(&call), jig::call::Parsed::Call { call: call.clone(), json: false });
        assert_eq!(f::from_json(&jig::json::write_call(crate::table::TOOLS, &call).unwrap()), call);
    }
    for argv in [
        &["landing", "show", "0"][..],
        &["wait", "landing", "61"],
        &["landings"],
        &["landing", "show", "61", "--key", ""],
    ] {
        f::parse(argv).unwrap_err();
    }
}
#[test]
fn help_landing() {
    f::check_golden("help_landing_show", &f::help(Some(55)));
}
#[test]
fn help_landings() {
    f::check_golden("help_landing_list", &f::help(Some(56)));
}

#[test]
fn phases_preparation_and_each_turn_holder_print_in_the_owners_words() {
    for (phase, word) in [
        (w::LandingPhase::Waiting, "waiting"),
        (w::LandingPhase::Preparing, "preparing"),
        (w::LandingPhase::Proving, "proving"),
        (w::LandingPhase::Proved, "proved"),
        (w::LandingPhase::Turn, "in its turn"),
        (w::LandingPhase::Landed, "landed"),
        (w::LandingPhase::Held, "held"),
        (w::LandingPhase::Cancelled, "cancelled"),
    ] {
        let answer = read_panel(&panel(phase), false);
        assert!(
            answer
                .stdout
                .strip_prefix("landing 61 · project 1 · ".as_bytes())
                .unwrap()
                .strip_prefix(word.as_bytes())
                .unwrap()
                .starts_with(b"\n")
        );
    }
    for (turn, words) in
        [(None, "turn: free\n"), (Some(61), "turn: this landing's\n"), (Some(60), "turn: landing 60's\n")]
    {
        let mut parts = panel(w::LandingPhase::Waiting).into_parts();
        parts.turn = turn;
        let mut member = parts.members.get(0).unwrap().clone().into_parts();
        member.candidate = None;
        member.base = None;
        member.pull = None;
        parts.members = List::with_capacity(1);
        parts.members.push(w::Member::new(&w::CEILINGS, member).unwrap()).unwrap();
        let answer = read_panel(&w::LandingPanel::new(&w::CEILINGS, parts).unwrap(), false);
        assert!(contains(&answer.stdout, "  candidate: not prepared\n"));
        assert!(contains(&answer.stdout, words));
        assert!(!contains(&answer.stdout, "pull request"));
    }
}
#[test]
fn a_line_can_name_several_changes_repositories_and_landed_commits() {
    let mut parts = list(None, &[61]).into_parts();
    let mut line = parts.landings.get(0).unwrap().clone().into_parts();
    line.changes = List::with_capacity(2);
    line.changes.push(51).unwrap();
    line.changes.push(52).unwrap();
    line.repositories = List::with_capacity(2);
    line.repositories.push(b"work/hello".as_slice().into()).unwrap();
    line.repositories.push(b"work/lib".as_slice().into()).unwrap();
    line.landed = List::with_capacity(2);
    line.landed.push([0x11; 32]).unwrap();
    line.landed.push([0x22; 32]).unwrap();
    parts.landings = List::with_capacity(1);
    parts.landings.push(w::LandingLine::new(&w::CEILINGS, line).unwrap()).unwrap();
    let answer = read_list(&w::LandingsPanel::new(&w::CEILINGS, parts).unwrap(), false);
    assert_eq!(answer.stdout.as_ref(), "landings of project 1 · turn: free\nlanding 61 · in its turn · change 51, 52 · work/hello, work/lib · main at 111111111111 · main at 222222222222\nno more\n".as_bytes());
}

#[test]
fn unknown_task_and_malformed_panels_finish_the_read_and_close_the_watch() {
    for why in [c::ReadNone::Unknown, c::ReadNone::Gone] {
        let mut root = f::signed_in(f::PARTY);
        root.pending_reads.push(1);
        let call = root.submit(&["landing", "show", "61"]);
        root.serve(call, c::ReadResult::None(why));
        let answer = root.answer(call).unwrap();
        assert_eq!(answer.status, 2);
        assert_eq!(answer.stderr.as_ref(), b"no landing for task 61\n");
    }
    let mut root = f::signed_in(f::PARTY);
    root.pending_reads.push(1);
    let call = root.submit(&["landing", "show", "61"]);
    root.serve(call, c::ReadResult::Value(c::ReadValue::Task(Box::new(f::goal(60, c::WaitsOn::Nothing)))));
    let answer = root.answer(call).unwrap();
    assert_eq!(answer.status, 1);
    assert_eq!(answer.stderr.as_ref(), b"unexpected task document\n");
    for wrong_task in [false, true] {
        let mut root = f::signed_in(f::PARTY);
        let call = root.submit(&["landing", "show", "61", "--project", "1"]);
        let mut parts = panel(w::LandingPhase::Proving).into_parts();
        if wrong_task {
            parts.task = 60;
        }
        let bytes = if wrong_task {
            wire::landing_panel(&w::LandingPanel::new(&w::CEILINGS, parts).unwrap())
        } else {
            Box::default()
        };
        let subject = landing::subject(Some(61), 1, false, &root.limits);
        deliver(&mut root, subject, bytes);
        let answer = root.answer(call).unwrap();
        assert_eq!(answer.status, 1);
        assert_eq!(answer.stderr.as_ref(), b"unexpected landing document\n");
        assert_eq!(root.watches.len(), 1);
    }
}
#[test]
fn panel_printing_refuses_one_byte_past_the_text_window() {
    let panel = panel(w::LandingPhase::Proving);
    for json in [false, true] {
        let mut limits = f::limits_root();
        let answer = landing::print(&panel, json, &limits);
        limits.tools.text = u32::try_from(answer.stdout.len()).unwrap();
        assert_eq!(landing::print(&panel, json, &limits).status, 0);
        limits.tools.text = limits.tools.text.checked_sub(1).unwrap();
        let answer = landing::print(&panel, json, &limits);
        assert_eq!(answer.status, 1);
        assert!(contains(&answer.stderr, "result limit"));
    }
}

fn contains(bytes: &[u8], part: &str) -> bool {
    skein_lib::bytes::find(bytes, part.as_bytes()).is_some()
}

#[test]
fn an_invalid_direct_call_prints_invalid_landing_call() {
    for json in [false, true] {
        let mut root = f::signed_in(f::PARTY);
        let mut call = f::call(&["landing", "show", "61"]);
        *call.values.get_mut(0).unwrap() = Some(jig::call::Values::One(jig::call::Value::Number(0)));
        let number = root.submit_prepared(call, json, None);
        let answer = root.answer(number).unwrap();
        assert_eq!(answer.status, 1);
        assert!(answer.stdout.is_empty());
        assert_eq!(
            answer.stderr.as_ref(),
            if json { b"{\"error\":\"invalid landing call\"}".as_slice() } else { b"invalid landing call\n" }
        );
        assert_eq!(root.watches.len(), 1, "only me remains watched");
    }
}

#[test]
fn a_refused_landing_or_landings_watch_prints_not_allowed() {
    for (argv, task) in [
        (&["landing", "show", "61", "--project", "1"][..], Some(61)),
        (&["landing", "list", "--project", "1"][..], None),
    ] {
        for json in [false, true] {
            let mut root = f::signed_in(f::PARTY);
            let call = root.submit_prepared(f::call(argv), json, None);
            let subject = landing::subject(task, 1, false, &root.limits);
            root.end(call, &subject, c::WatchEnd::NotAllowed);
            let answer = root.answer(call).unwrap();
            assert_eq!(answer.status, 2);
            assert!(answer.stdout.is_empty());
            assert_eq!(
                answer.stderr.as_ref(),
                if json { b"{\"error\":\"not_allowed\"}".as_slice() } else { b"not_allowed\n" }
            );
            assert_eq!(root.watches.len(), 1, "only me remains watched");
        }
    }
}

const CUTOFF: &str = "2026-10-27T21:00Z";
fn logged_page(count: u32, after: Option<Box<[u8]>>) -> w::LandingsPanel {
    let mut logged = List::with_capacity(count);
    for number in (1..=count).rev() {
        let mut changes = List::with_capacity(1);
        let mut verdicts = List::with_capacity(2);
        if number % 2 != 0 {
            changes.push(12).unwrap();
            for (tier, verdict) in [
                (b"fuzzy".as_slice(), w::LoggedVerdict::Cleared),
                (
                    b"browser".as_slice(),
                    w::LoggedVerdict::Red(
                        w::LoggedRed::new(&w::CEILINGS, w::LoggedRedParts { tests: Some(2) }).unwrap(),
                    ),
                ),
            ] {
                verdicts
                    .push(w::LoggedMark::new(&w::CEILINGS, w::LoggedMarkParts { tier: tier.into(), verdict }).unwrap())
                    .unwrap();
            }
        }
        logged
            .push(
                w::LoggedLine::new(
                    &w::CEILINGS,
                    w::LoggedLineParts {
                        commit: [u8::try_from(number).unwrap(); 32],
                        repository: b"o/app".as_slice().into(),
                        seen: jig::argv::deadline(CUTOFF.as_bytes()).unwrap().as_nanos()
                            + u64::from(number) * 60_000_000_000,
                        changes,
                        verdicts,
                    },
                )
                .unwrap(),
            )
            .unwrap();
    }
    w::LandingsPanel::new(
        &w::CEILINGS,
        w::LandingsPanelParts {
            project: 1,
            turn: None,
            landings: List::with_capacity(0),
            newer_than: Some(jig::argv::deadline(CUTOFF.as_bytes()).unwrap().as_nanos()),
            logged,
            after,
        },
    )
    .unwrap()
}
fn read_logged(panel: &w::LandingsPanel, json: bool, after: Option<&str>) -> jig::Printed {
    let mut root = f::signed_in(f::PARTY);
    let mut argv = vec!["landing", "list", "--project", "1", "--newer-than", CUTOFF];
    if json {
        argv.push("--json");
    }
    if let Some(after) = after {
        argv.extend(["--after", after]);
    }
    let call = root.submit(&argv);
    let subject =
        root.watches.values().find(|subject| matches!(subject, client::Watchable::Family { .. })).unwrap().clone();
    deliver(&mut root, subject, wire::landings_panel(panel));
    root.answer(call).unwrap()
}
#[test]
fn newer_than_takes_a_utc_minute() {
    let call = f::call(&["landing", "list", "--project", "1", "--newer-than", CUTOFF]);
    assert_eq!(f::from_json(&jig::json::write_call(crate::table::TOOLS, &call).unwrap()), call);
    for time in ["2026-10-27T21:00:01Z", "2026-10-27T21:00+00:00", "2026-10-27"] {
        assert_eq!(
            f::parse(&["landing", "list", "--project", "1", "--newer-than", time]),
            Err(jig::argv::Usage::Kind { argument: 2 })
        );
    }
}
#[test]
fn deadline_takes_the_same_kind() {
    let deadline = jig::rows::AMEND.forms[0].iter().find(|argument| argument.name == b"deadline").unwrap();
    let newer = landing::LIST.forms[0].iter().find(|argument| argument.name == b"newer-than").unwrap();
    assert_eq!((deadline.kind, newer.kind), (jig::table::Kind::Time, jig::table::Kind::Time));
    f::call(&["task", "amend", "41", "--rev", "7", "--deadline", CUTOFF, "because"]);
}
#[test]
fn logged_pages_print_their_changes_times_and_verdicts() {
    f::check_golden("logged_landings", &read_logged(&logged_page(2, None), false, None).stdout);
}
#[test]
fn logged_pages_json_names_each_tiers_mark() {
    f::check_golden("logged_landings_json", &read_logged(&logged_page(2, None), true, None).stdout);
}
#[test]
fn a_page_past_32_lines_ends_with_its_cursor_and_the_next_reads_on() {
    // Jig's Number document cursor: tag 0 and offset 32, little endian.
    const CURSOR: &str = "002000000000000000";
    let mut cursor = vec![0];
    cursor.extend(32_u64.to_le_bytes());
    let first = read_logged(&logged_page(32, Some(cursor.into_boxed_slice())), false, None);
    assert!(
        std::str::from_utf8(&first.stdout)
            .unwrap()
            .ends_with(&format!("read on with: landing list --project 1 --newer-than {CUTOFF} --after {CURSOR}\n"))
    );
    let second = read_logged(&logged_page(1, None), false, Some(CURSOR));
    assert!(second.stdout.ends_with(b"no more\n"));
    assert_eq!(second.status, 0);
}
