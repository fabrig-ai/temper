use crate::tests::fixtures;
use jig_client::contract::{Admission, ReadNone, ReadResult};
#[test]
fn a_task_read_prints_its_document() {
    let (mut root, call, task) = fixtures::task_read();
    root.serve(call, fixtures::task_result(task));
    let answer = root.answer(call).expect("answered once the read is");
    assert_eq!(answer.status, 0);
    fixtures::check_golden("task_goal_waiting", &answer.stdout);
}
#[test]
fn a_closed_chat_reads_its_kept_conversation() {
    use jig_client::contract as c;
    for json in [false, true] {
        for end_first in [false, true] {
            let argv = fixtures::extended(fixtures::CHAT_CALL, if json { &["--json"] } else { &[] });
            let (mut root, call) = fixtures::read_call(argv.as_slice());
            if end_first {
                root.end(call, &fixtures::run_watch(), c::WatchEnd::SubjectEnded);
                assert!(root.answer(call).is_none(), "activity still pending");
            }
            root.serve(call, fixtures::kept_conversation());
            if !end_first {
                assert!(root.answer(call).is_none(), "run slot still pending");
                root.end(call, &fixtures::run_watch(), c::WatchEnd::SubjectEnded);
            }
            let answer = root.answer(call).expect("kept conversation answered");
            assert_eq!(answer.status, 0);
            assert!(answer.stderr.is_empty());
            let mut previous = None;
            for words in fixtures::KEPT_TURNS {
                let at = skein_lib::bytes::find(&answer.stdout, words).expect("kept turn printed");
                if let Some(previous) = previous {
                    assert!(previous < at, "kept turns remain in order");
                }
                previous = Some(at);
            }
            if json {
                skein_json::tokenizer::validate(
                    &answer.stdout,
                    &skein_json::tokenizer::Limits {
                        depth: 16,
                        string: 0,
                        number: 20,
                        chunk: 256,
                        length: u32::try_from(answer.stdout.len()).expect("small conversation"),
                    },
                )
                .expect("one valid JSON document");
                assert!(skein_lib::bytes::find(&answer.stdout, br#""run":"#).is_none());
            }
        }
    }
}
#[test]
fn a_task_run_read_of_an_ended_task_prints_the_task() {
    use jig_client::contract as c;
    for json in [false, true] {
        let plain = fixtures::extended(fixtures::TASK_CALL, if json { &["--json"] } else { &[] });
        let argv = fixtures::extended(plain.as_slice(), &["--run"]);
        let task = fixtures::ended_task(c::Phase::Done);
        let expected = fixtures::read_answer(plain.as_slice(), fixtures::task_result(task.clone()));
        for end_first in [false, true] {
            let (mut root, call) = fixtures::read_call(argv.as_slice());
            if end_first {
                root.end(call, &fixtures::run_watch(), c::WatchEnd::SubjectEnded);
            }
            root.serve(call, fixtures::task_result(task.clone()));
            if !end_first {
                assert!(root.answer(call).is_none(), "run slot still pending");
                root.end(call, &fixtures::run_watch(), c::WatchEnd::SubjectEnded);
            }
            let answer = root.answer(call).expect("ended task answered");
            assert_eq!(answer.status, 0);
            assert!(answer.stderr.is_empty());
            assert_eq!(answer.stdout, expected.stdout);
        }
    }
}
#[test]
fn a_chat_read_whose_watch_is_refused_still_fails() {
    use jig_client::contract as c;
    for (json, expected) in [(false, b"watch ended\n".as_slice()), (true, b"{\"error\":\"watch ended\"}")] {
        let argv = fixtures::extended(fixtures::CHAT_CALL, if json { &["--json"] } else { &[] });
        let (mut root, call) = fixtures::read_call(argv.as_slice());
        root.serve(call, fixtures::kept_conversation());
        root.end(call, &fixtures::run_watch(), c::WatchEnd::NotAllowed);
        let answer = root.answer(call).expect("refusal answered");
        assert_eq!(answer.status, 2);
        assert!(answer.stdout.is_empty());
        assert_eq!(answer.stderr.as_ref(), expected);
    }
}
#[test]
fn a_proposable_act_says_to_whom() {
    let (mut root, call, mut task) = fixtures::task_read();
    task.acts.get_mut(0).expect("first offer").admission =
        Admission::Proposed { to: jig_client::contract::Recipient::Party(fixtures::PARTY) };
    root.serve(call, fixtures::task_result(task));
    fixtures::check_golden("task_proposed", &root.answer(call).expect("read answered").stdout);
}
#[test]
fn a_refused_act_says_why_without_a_call() {
    let (mut root, call, mut task) = fixtures::task_read();
    task.acts.get_mut(0).expect("first offer").admission =
        Admission::Refused(jig_client::contract::Refusal::NotOffered);
    root.serve(call, fixtures::task_result(task));
    fixtures::check_golden("task_refused", &root.answer(call).expect("read answered").stdout);
}
#[test]
fn me_prints_the_live_frame() {
    for (argv, golden) in [(fixtures::ME_CALL, "me"), (fixtures::ME_JSON_CALL, "me_json")] {
        let (mut root, call) = fixtures::read_call(argv);
        let answer = root.answer(call).expect("live frame answered");
        if std::env::var_os("SKEIN_CODEGEN_WRITE").is_some() {
            std::fs::write(std::path::Path::new("src/tests/golden").join(golden).with_extension("txt"), &answer.stdout)
                .expect("write me golden");
        }
        assert_eq!((answer.status, answer.stdout), (0, fixtures::golden(golden)), "{golden}");
    }
}
#[test]
fn no_call_runs_before_me_is_live() {
    let mut root = fixtures::started(skein_lib::List::with_capacity(fixtures::EMPTY));
    let call = root.submit(fixtures::TASK_CALL);
    assert_eq!(root.reads.len(), 0);
    root.deliver(jig_client::contract::Delivery::Me(Box::new(fixtures::me(fixtures::PARTY))));
    assert_eq!(
        root.reads.get(&call).expect("read after live me").1,
        jig_client::contract::Read::Task { task: fixtures::TASK }
    );
}
#[test]
fn each_read_none_exits_as_the_table_says() {
    for (why, status, message) in [
        (ReadNone::Unknown, 2, b"unknown\n".as_slice()),
        (ReadNone::Refused, 2, b"refused\n"),
        (ReadNone::Gone, 2, b"gone\n"),
        (ReadNone::Busy, 1, b"busy\n"),
        (ReadNone::SignedOut, 1, b"signed out\n"),
        (ReadNone::Unreachable, 1, b"unreachable\n"),
        (ReadNone::Malformed, 1, b"malformed document\n"),
        (ReadNone::TooLarge { bytes: fixtures::TOO_LARGE }, 1, b"document limit\n"),
    ] {
        let (mut root, call) = fixtures::read_call(fixtures::TASK_CALL);
        root.serve(call, ReadResult::None(why));
        let answer = root.answer(call).expect("none is terminal");
        assert_eq!((answer.status, answer.stderr.as_ref()), (status, message), "{why:?}");
    }
}
#[test]
fn a_result_past_the_text_limit_says_how_much_is_left() {
    let (mut root, call, mut task) = fixtures::task_read();
    let mut words = skein_lib::Writer::new(usize::try_from(fixtures::LONG_WORDS).expect("fixture size"));
    for _ in 0..fixtures::LONG_WORDS {
        words.put(b"x").expect("fixture length");
    }
    task.spec.specification.words = fixtures::document_text(&words.finish());
    root.serve(call, fixtures::task_result(task));
    let answer = root.answer(call).expect("cut result answered");
    fixtures::check_golden("task_cut", &answer.stdout);
}

#[test]
fn a_bounded_offer_prints_its_whole_bounds() {
    for (beyond, golden) in [
        (
            jig_client::contract::Beyond::Proposed { to: jig_client::contract::Recipient::Party(fixtures::PARTY) },
            "task_bounded_proposed",
        ),
        (jig_client::contract::Beyond::Refused(jig_client::contract::Refusal::NotOffered), "task_bounded_refused"),
    ] {
        let (mut root, call, mut task) = fixtures::task_read();
        task.acts.get_mut(0).expect("first offer").admission = fixtures::bounded_offer(beyond);
        root.serve(call, fixtures::task_result(task));
        fixtures::check_golden(golden, &root.answer(call).expect("read answered").stdout);
    }
}

#[test]
fn an_owner_resource_path_is_printed_whole() {
    let (mut root, call, mut task) = fixtures::task_read();
    fixtures::long_resource(&mut task);
    root.serve(call, fixtures::task_result(task));
    fixtures::check_golden("task_whole_path", &root.answer(call).expect("read answered").stdout);
}

#[test]
fn an_act_with_no_tool_prints_its_name() {
    let mut printed_by_admission = skein_lib::List::with_capacity(fixtures::NO_TOOL_CASES);
    let mut expected = skein_lib::List::with_capacity(fixtures::NO_TOOL_CASES);
    for (admission, golden) in [
        (Admission::Allowed, "task_no_tool"),
        (Admission::Proposed { to: jig_client::contract::Recipient::Party(fixtures::PARTY) }, "task_no_tool_proposed"),
        (Admission::Refused(jig_client::contract::Refusal::NotOffered), "task_no_tool_refused"),
        (
            fixtures::bounded_offer(jig_client::contract::Beyond::Refused(jig_client::contract::Refusal::NotOffered)),
            "task_no_tool_bounded",
        ),
    ] {
        let (mut root, call, mut task) = fixtures::task_read();
        let offer = task.acts.get_mut(0).expect("first offer");
        offer.act = jig_client::contract::ActKind::Take;
        offer.admission = admission;
        root.serve(call, fixtures::task_result(task));
        let printed = root.answer(call).expect("read answered");
        fixtures::check_golden(golden, &printed.stdout);
        printed_by_admission.push((golden, printed.stdout)).expect("every row");
        expected.push((golden, fixtures::golden(golden))).expect("every row");
    }
    assert_eq!(printed_by_admission, expected);
}
#[test]
fn task_json_names_the_nested_contract_fields() {
    let (mut root, call) = fixtures::read_call(fixtures::TASK_JSON_CALL);
    root.serve(call, fixtures::task_result(fixtures::goal(fixtures::TASK, fixtures::waiting_on_me())));
    let answer = root.answer(call).expect("JSON read answered");
    let field = br#""waits_on":{"party":{"whom":{"person":73},"entries":1,"task":42,"kind":"question"}}"#;
    let mut found = false;
    for bytes in answer.stdout.windows(field.len()) {
        if bytes == field {
            found = true;
            break;
        }
    }
    assert!(found);
}

#[test]
fn an_ended_task_json_has_one_result_member() {
    use jig_client::contract as c;
    let report = c::ResultValue::Report(fixtures::document_text(b"Final task report"));
    let ending = c::OwnerChipEnding::Done(report);
    let mut task = fixtures::ended_task(c::Phase::Done);
    task.result = Some(c::TaskResult::Ending(Box::new(ending.clone())));
    let historical = fixtures::historical_task_result(task.clone(), c::Ending::Task(Box::new(ending)));

    let direct = fixtures::read_answer(fixtures::TASK_JSON_CALL, historical.clone());
    let (mut root, call) = fixtures::waiting_call(fixtures::WAIT_TASK, fixtures::UNTIL_ENDED);
    root.end(call, &fixtures::task_watch(), c::WatchEnd::SubjectEnded);
    root.serve(call, historical);
    let waited = root.answer(call).expect("ended wait answered");
    assert_eq!(waited.stdout, fixtures::met_json(&direct.stdout));
    for answer in [direct, waited] {
        assert_eq!(answer.status, 0);
        fixtures::unique_keys(&answer.stdout).expect("every ended task object has unique keys");
        let fields = br#""ending":{"task":{"done":{"report":{"bytes":"Final task report","cut":false}}}},"value":{"report":{"bytes":"Final task report","cut":false}}"#;
        assert!(skein_lib::bytes::find(&answer.stdout, fields).is_some(), "whole record ending and value");
    }

    let live = fixtures::read_answer(fixtures::TASK_JSON_CALL, fixtures::task_result(task));
    let own = br#""result":{"ending":{"done":{"report":{"bytes":"Final task report","cut":false}}}}"#;
    assert!(skein_lib::bytes::find(&live.stdout, own).is_some(), "live task form retains its own result");
}

#[test]
fn a_source_wait_keeps_its_unknown_deadline() {
    use jig_client::contract as c;
    for (state, expected) in [
        (c::TaskSourceState::Later { until: None }, b"later until unknown".as_slice()),
        (c::TaskSourceState::Limited { until: None }, b"limited until unknown".as_slice()),
    ] {
        let (mut root, call, mut task) = fixtures::task_read();
        task.run_wait =
            Some(c::TaskRunWait::Account { account: Some(43), source: Some(17), why: c::TaskWaitWhy::Source(state) });
        root.serve(call, fixtures::task_result(task));
        let answer = root.answer(call).expect("read answered");
        assert!(skein_lib::bytes::find(&answer.stdout, expected).is_some());
    }
}

#[test]
fn a_task_read_retains_its_named_payer() {
    let (mut root, call) = fixtures::read_call(fixtures::TASK_JSON_CALL);
    let mut task = fixtures::goal(fixtures::TASK, fixtures::waiting_on_me());
    task.kept_account = Some(43);

    root.serve(call, fixtures::task_result(task));
    let answer = root.answer(call).expect("read answered");
    assert!(skein_lib::bytes::find(&answer.stdout, br#""kept_account":43"#).is_some());
}

#[test]
fn a_task_read_retains_its_account_wait_reason() {
    use jig_client::contract as c;
    let (mut root, call) = fixtures::read_call(fixtures::TASK_JSON_CALL);
    let mut task = fixtures::goal(fixtures::TASK, fixtures::waiting_on_me());
    task.run_wait = Some(c::TaskRunWait::Account {
        account: Some(43),
        source: Some(17),
        why: c::TaskWaitWhy::Invalid(c::AccountWhy::Limited),
    });
    root.serve(call, fixtures::task_result(task));
    let answer = root.answer(call).expect("read answered");
    let field = br#""run_wait":{"account":{"account":43,"source":17,"why":{"invalid":"limited"}}}"#;
    assert!(skein_lib::bytes::find(&answer.stdout, field).is_some());
}

#[test]
fn a_source_wait_prints_a_null_json_deadline() {
    use jig_client::contract as c;
    for state in [c::TaskSourceState::Later { until: None }, c::TaskSourceState::Limited { until: None }] {
        let (mut root, call) = fixtures::read_call(fixtures::TASK_JSON_CALL);
        let mut task = fixtures::goal(fixtures::TASK, fixtures::waiting_on_me());
        task.run_wait =
            Some(c::TaskRunWait::Account { account: Some(43), source: Some(17), why: c::TaskWaitWhy::Source(state) });
        root.serve(call, fixtures::task_result(task));
        let answer = root.answer(call).expect("read answered");
        assert!(skein_lib::bytes::find(&answer.stdout, br#""until":null"#).is_some());
    }
}

#[test]
fn dependency_waits_use_jigs_words_and_keep_their_counts() {
    for (argv, expected) in [
        (fixtures::TASK_CALL, b"waiting on: Kept by its dependencies".as_slice()),
        (fixtures::TASK_JSON_CALL, br#""dependencies":{"pending":[],"count":2}"#),
    ] {
        let task = fixtures::goal(
            fixtures::TASK,
            jig_client::contract::WaitsOn::Dependencies { pending: skein_lib::List::with_capacity(0), count: 2 },
        );
        let printed = fixtures::read_answer(argv, fixtures::task_result(task));
        assert!(skein_lib::bytes::find(&printed.stdout, expected).is_some());
    }
}
#[test]
fn a_circle_hold_keeps_its_json_category() {
    let mut latest = fixtures::chat_at(fixtures::TASK, jig_client::contract::RunStage::Idle, None);
    let task = fixtures::task_in(&mut latest);
    let jig_client::contract::ChipPhase::Owner { phase, .. } = task.chip.details.as_mut() else { panic!("owner chip") };
    *phase =
        jig_client::contract::OwnerChipPhase::Held { reason: jig_client::contract::HoldCategory::Circle, revision: 1 };
    let printed = fixtures::read_answer(fixtures::TASK_JSON_CALL, fixtures::task_result(task.clone()));
    assert!(skein_lib::bytes::find(&printed.stdout, br#""reason":"circle""#).is_some());
}
#[test]
fn a_refused_next_run_names_its_cause_in_text_and_json() {
    use jig_client::contract as c;
    // One minute after the epoch exercises the document's wall formatting.
    const AT: skein_lib::Wall = skein_lib::Wall::from_nanos(60_000_000_000);
    let rows = [
        (
            "model program",
            c::TaskWaitRefused::Program(c::OpenProgram::Model),
            "next run: refused, model runs through a program",
            r#"{"refused":{"program":"model"}}"#,
        ),
        (
            "source program",
            c::TaskWaitRefused::Program(c::OpenProgram::Source { account: 43, source: 17 }),
            "next run: refused, source 17 of account 43 runs through a program",
            r#"{"refused":{"program":{"source":{"account":43,"source":17}}}}"#,
        ),
        (
            "reserved engine channel",
            c::TaskWaitRefused::Hello {
                kind: c::OwnerHostKind::Engine,
                at: AT,
                why: c::OwnerHelloRefusal::ReservedChannel,
            },
            "next run: refused, engine refused at 00:01: channel zero belongs to the engine's own host",
            r#"{"refused":{"hello":{"kind":"engine","at":60000000000,"why":"reserved_channel"}}}"#,
        ),
        (
            "worker capacity",
            c::TaskWaitRefused::Hello {
                kind: c::OwnerHostKind::Worker,
                at: AT,
                why: c::OwnerHelloRefusal::Workers { limit: 4 },
            },
            "next run: refused, worker refused at 00:01: no room for another worker, limit 4",
            r#"{"refused":{"hello":{"kind":"worker","at":60000000000,"why":{"workers":{"limit":4}}}}}"#,
        ),
        (
            "worker position",
            c::TaskWaitRefused::Hello {
                kind: c::OwnerHostKind::Worker,
                at: AT,
                why: c::OwnerHelloRefusal::Position { worker: 3 },
            },
            "next run: refused, worker refused at 00:01: unconfigured or occupied worker position 3",
            r#"{"refused":{"hello":{"kind":"worker","at":60000000000,"why":{"position":{"worker":3}}}}}"#,
        ),
        (
            "worker stop bound",
            c::TaskWaitRefused::Hello {
                kind: c::OwnerHostKind::Worker,
                at: AT,
                why: c::OwnerHelloRefusal::StopBound { grace: skein_lib::Duration::from_nanos(2000) },
            },
            "next run: refused, worker refused at 00:01: would not stop before the engine's grace ends, allowed grace 2000 ns",
            r#"{"refused":{"hello":{"kind":"worker","at":60000000000,"why":{"stop_bound":{"grace":2000}}}}}"#,
        ),
        (
            "worker agent window",
            c::TaskWaitRefused::Hello {
                kind: c::OwnerHostKind::Worker,
                at: AT,
                why: c::OwnerHelloRefusal::Agent { why: c::OwnerAgentShortfall::Window { charter: 2, model: 5 } },
            },
            "next run: refused, worker refused at 00:01: agent window too small for charter 2, model position 5",
            r#"{"refused":{"hello":{"kind":"worker","at":60000000000,"why":{"agent":{"why":{"window":{"charter":2,"model":5}}}}}}}"#,
        ),
    ];
    for (name, cause, expected_text, expected_json) in rows {
        let mut task = fixtures::goal(fixtures::TASK, c::WaitsOn::Nothing);
        task.run_wait = Some(c::TaskRunWait::Refused(cause));
        let printed = fixtures::read_answer(fixtures::TASK_CALL, fixtures::task_result(task.clone()));
        let start = skein_lib::bytes::find(&printed.stdout, b"next run: ").expect("next-run line");
        let tail = printed.stdout.get(start..).expect("line within document");
        let end = skein_lib::bytes::find(tail, b"\n").expect("line ends");
        assert_eq!(tail.get(..end).expect("whole next-run line"), expected_text.as_bytes(), "{name}");
        let printed = fixtures::read_answer(fixtures::TASK_JSON_CALL, fixtures::task_result(task));
        let field = b"\"run_wait\":";
        let start = skein_lib::bytes::find(&printed.stdout, field).expect("run_wait field");
        let tail = printed.stdout.get(start.checked_add(field.len()).expect("field offset")..).expect("field value");
        let end = skein_lib::bytes::find(tail, b",\"spec_revision\":").expect("following task field");
        assert_eq!(tail.get(..end).expect("whole run_wait value"), expected_json.as_bytes(), "{name}");
    }
}

#[test]
fn a_document_at_its_byte_bound_prints_worst_case_json_escapes() {
    use crate::jig::render::document::{self, Document};
    let task = fixtures::document_at_byte_bound();
    let printed = document::print(Document::Task(&task, None), true, &crate::limits::tools(), &crate::root::Parts);

    assert_eq!(printed.status, 0, "{:?}", printed.stderr);
    assert!(u64::try_from(printed.stdout.len()).expect("output length fits") > crate::limits::DOCUMENT);
    assert!(skein_lib::bytes::find(&printed.stdout, br"\u0000").is_some(), "control text is JSON escaped");
}

#[test]
fn a_budget_held_task_explains_how_to_run_or_cancel_it() {
    use jig_client::contract as c;
    let (mut root, call, mut task) = fixtures::task_read();
    task.hold = Some(c::TaskHold::Document(Box::new(c::Hold {
        revision: c::Revision { value: fixtures::HOLD },
        reason: c::HoldReason::Budget { spent: 3, limit: 4 },
        before: c::Phase::Idle,
        since: skein_lib::Wall::EPOCH,
        result: None,
        spend: skein_lib::List::with_capacity(0),
    })));
    task.chip.phase = c::Phase::Held;
    root.serve(call, fixtures::task_result(task));
    let answer = root.answer(call).expect("held task answered");
    assert_eq!(answer.status, 0);
    assert!(skein_lib::bytes::find(&answer.stdout,
        b"Insufficient activation budget. A release alone holds it again: give the task more budget (amend 41 with a larger budget, or decide its budget proposal), then release it; or cancel it.").is_some(), "{:?}", answer.stdout);
}

#[test]
fn budget_held_owner_tasks_and_inbox_rows_share_the_recovery_words() {
    use jig_client::contract as c;
    let mut latest = fixtures::chat_at(fixtures::TASK, c::RunStage::Idle, None);
    let mut task = fixtures::task_in(&mut latest).clone();
    task.chip.phase = c::Phase::Held;
    let c::ChipPhase::Owner { phase, .. } = task.chip.details.as_mut() else { panic!("owner chip") };
    *phase = c::OwnerChipPhase::Held { reason: c::HoldCategory::Budget, revision: fixtures::HOLD };
    task.hold = Some(c::TaskHold::Owner(Box::new(c::OwnerChipHold {
        circle: None,
        account: None,
        source: None,
        reason: c::HoldCategory::Budget,
        revision: fixtures::HOLD,
        before: c::OwnerPriorPhase::Active(c::RunStage::Idle),
        escalation: None,
        tries: c::OwnerFailureTries { transient: 0, permanent: 0, run: 0, agent: 0, lost: 0, invalid: 0 },
        hold_wait_since: None,
    })));
    let mut page = fixtures::owner_first_window(1, false).0;
    page.rows.get_mut(0).unwrap().task = c::OwnerTaskLink::Live(task.chip.clone());
    for (argv, result) in [
        (fixtures::TASK_CALL, fixtures::task_result(task)),
        (fixtures::INBOX_CALL, ReadResult::Value(c::ReadValue::OwnerInbox(Box::new(page)))),
    ] {
        let (mut root, call) = fixtures::read_call(argv);
        root.serve(call, result);
        if argv == fixtures::INBOX_CALL {
            root.serve(call, fixtures::inbox_rows(c::InboxReadGroup::Updates, 0, false));
        }
        let answer = root.answer(call).expect("held task or inbox answered");
        assert_eq!(answer.status, 0);
        assert!(skein_lib::bytes::find(&answer.stdout,
            b"Insufficient activation budget. A release alone holds it again: give the task more budget (amend 41 with a larger budget, or decide its budget proposal), then release it; or cancel it.").is_some(), "{:?}", answer.stdout);
    }
}
