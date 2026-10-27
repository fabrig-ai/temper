use crate::tests::fixtures as f;
use jig_client::contract as c;
use skein_lib::{List, Wall};
const FORGE: u16 = temper_client_protocol_wire::number(temper_client_protocol_wire::Owner::Forge);
const DELEGATE: u64 = 2;
const BRANCH_SEGMENTS: u32 = 3;
const ACTIVITY_ROWS: u32 = 3;
const MERGE: &[u8] = b"1d0b14e4db018fed000000000000000300000000";
const INSTRUCTIONS: &[u8] =
    b"Answer on port 8080 unless the store's file names another one, and keep the /health endpoint small.";
const REPORT: &[u8] = b"widget --csv prints the table\nand preserves its column order";
fn contract(connector: u16, kind: u16) -> c::TaskContractDocument {
    c::TaskContractDocument::Change { connector, kind, words: f::LONG_WORDS }
}
fn delegate() -> c::Task {
    let mut task = f::goal(DELEGATE, c::WaitsOn::Nothing);
    task.label = 0;
    task.goal = None;
    task.priority = None;
    task.result_contract = contract(FORGE, 1);
    task.requester = c::OwnerRequester::Task(1);
    task.spec.specification.words = f::document_text(b"Export widget's table as CSV");
    task.budget.numbers.spent = 0;
    task.budget.numbers.budget = 10;
    task.acts = List::with_capacity(0);
    task
}
fn part() -> c::OwnerPart {
    let mut path = List::with_capacity(BRANCH_SEGMENTS);
    for segment in [&1_u16.to_be_bytes()[..], &1_u32.to_be_bytes(), b"temper/1/c2"] {
        path.push(Box::from(segment)).expect("three segments");
    }
    c::OwnerPart { connector: FORGE, kind: c::OwnerPartKind::Resource, key: c::OwnerPartKey::Path(path) }
}
fn ended(mut task: c::Task, ending: c::Ending, value: Option<c::ResultValue>) -> c::ReadResult {
    task.chip.phase = c::Phase::Ended;
    let result =
        c::Result { task: task.chip.clone(), ending, value, at: None, details: None, acts: List::with_capacity(0) };
    task.result = Some(c::TaskResult::Document(Box::new(result.clone())));
    c::ReadResult::Value(c::ReadValue::Historical(Box::new(c::OwnerHistorical {
        key: c::ObjectKey::Task(task.chip.task),
        value: c::OwnerHistoricalValue::Task { document: Box::new(task), result: Box::new(result) },
        at: None,
        by: None,
    })))
}
fn assert_golden(name: &str, printed: crate::jig::Printed) {
    assert_eq!(printed.status, 0);
    f::check_golden(name, &printed.stdout);
}
fn delegate_answer(result: c::ReadResult) -> crate::jig::Printed {
    let (mut root, call) = f::read_call(&["task", "2"]);
    let mut me = f::me(f::PARTY);
    me.projects.get_mut(0).expect("project").name = jig_client::contract::DocumentText::complete(
        jig_client::contract::Text::new(b"project 1".as_slice().into(), 64).expect("project name"),
    );
    root.deliver(c::Delivery::Me(Box::new(me)));
    root.serve(call, result);
    root.answer(call).expect("delegate answered")
}
fn header(task: c::Task, expected: &[u8]) {
    let printed = f::read_answer(f::TASK_CALL, f::task_result(task));
    let end = skein_lib::bytes::find(&printed.stdout, b"\n").expect("header line");
    assert_eq!(&printed.stdout[..end], expected);
}
fn untracked(contract: c::TaskContractDocument) -> c::Task {
    let mut task = f::goal(f::TASK, c::WaitsOn::Nothing);
    task.goal = None;
    task.priority = None;
    task.result_contract = contract;
    task
}
#[test]
fn a_chat_header_names_the_conversation() {
    let mut task = f::goal(f::TASK, c::WaitsOn::Nothing);
    task.flags.conversation = true;
    header(task, b"task 41 \xc2\xb7 chat \xc2\xb7 Running \xc2\xb7 shop \xc2\xb7 priority 2");
}
#[test]
fn a_goal_header_names_its_own_goal() {
    header(
        f::goal(f::TASK, c::WaitsOn::Nothing),
        b"task 41 \xc2\xb7 goal \xc2\xb7 Running \xc2\xb7 shop \xc2\xb7 priority 2",
    );
}
#[test]
fn an_approval_header_names_the_contract() {
    header(
        untracked(c::TaskContractDocument::Approval { state: Box::from(*b"ready"), part: part() }),
        b"task 41 \xc2\xb7 approval \xc2\xb7 Running \xc2\xb7 shop \xc2\xb7 untracked",
    );
}
#[test]
fn a_report_header_names_the_contract() {
    header(
        untracked(c::TaskContractDocument::Report { words: f::LONG_WORDS }),
        b"task 41 \xc2\xb7 report \xc2\xb7 Running \xc2\xb7 shop \xc2\xb7 untracked",
    );
}
#[test]
fn a_verdict_header_names_the_contract() {
    header(
        untracked(c::TaskContractDocument::Verdict { choices: List::with_capacity(0), cut: false }),
        b"task 41 \xc2\xb7 verdict \xc2\xb7 Running \xc2\xb7 shop \xc2\xb7 untracked",
    );
}
#[test]
fn a_change_header_names_the_forge_change() {
    header(untracked(contract(FORGE, 1)), b"task 41 \xc2\xb7 change \xc2\xb7 Running \xc2\xb7 shop \xc2\xb7 untracked");
}
#[test]
fn another_change_kind_uses_the_forges_words() {
    for (kind, expected) in [
        (2, b"task 41 \xc2\xb7 change, push \xc2\xb7 Running \xc2\xb7 shop \xc2\xb7 untracked".as_slice()),
        (99, b"task 41 \xc2\xb7 change, kind 99 \xc2\xb7 Running \xc2\xb7 shop \xc2\xb7 untracked"),
    ] {
        header(untracked(contract(FORGE, kind)), expected);
    }
}
#[test]
fn another_connectors_change_keeps_its_connector_and_kind() {
    header(
        untracked(contract(7, 2)),
        b"task 41 \xc2\xb7 change, connector 7 kind 2 \xc2\xb7 Running \xc2\xb7 shop \xc2\xb7 untracked",
    );
}
#[test]
fn a_landed_delegate_names_the_pull_before_the_whole_merge_commit() {
    let line =
        b"\npull request #1 of qa/pilot-20261026 on localhost:3000: http://localhost:3000/qa/pilot-20261026/pulls/1";
    let mut words = skein_lib::Writer::new(MERGE.len() + line.len());
    words.put(MERGE).unwrap();
    words.put(line).unwrap();
    let mut task = delegate();
    let branch = part();
    task.resources = List::with_capacity(1);
    task.resources
        .push(c::TaskResource::Owner(Box::new(c::OwnerResourceLine {
            part: branch.clone(),
            kind: 1,
            taken: false,
            holder: None,
            writer: None,
            writer_lost: false,
        })))
        .expect("branch resource");
    let result = ended(
        task,
        c::Ending::Task(Box::new(c::OwnerChipEnding::Done(c::ResultValue::Change {
            kind: 1,
            part: c::OwnerPart {
                connector: FORGE,
                kind: c::OwnerPartKind::Resource,
                key: c::OwnerPartKey::Path({
                    let mut path = List::with_capacity(6);
                    for segment in [b"forge".as_slice(), b"localhost:3000", b"qa", b"pilot-20261026", b"pull", b"1"] {
                        path.push(Box::from(segment)).unwrap();
                    }
                    path
                }),
            },
            words: f::document_text(&words.finish()),
        }))),
        None,
    );
    assert_golden("task_change_landed", delegate_answer(result));
}
#[test]
fn a_landed_delegate_omits_a_pull_address_when_its_document_words_are_cut() {
    for line in [
        b"\npull request #1 of work/hello on forge.example: https://forge.example/work/hello/pulls/1".as_slice(),
        b"\npull request #1 of work/hello on forge.example: https://forge.example/work/hello/pulls".as_slice(),
    ] {
        let mut bytes = skein_lib::Writer::new(MERGE.len() + line.len());
        bytes.put(MERGE).unwrap();
        bytes.put(line).unwrap();
        let mut words = f::document_text(&bytes.finish());
        words.cut = true;
        result_text(
            c::Ending::Done,
            Some(c::ResultValue::Change {
                kind: 1,
                part: c::OwnerPart {
                    connector: FORGE,
                    kind: c::OwnerPartKind::Resource,
                    key: c::OwnerPartKey::Number(1),
                },
                words,
            }),
            b"result: Landed \xc2\xb7 pull request #1 \xc2\xb7 merge commit 1d0b14e4db018fed000000000000000300000000\n",
        );
    }
}

#[test]
fn an_ended_report_indents_every_line_of_its_words() {
    let mut task = delegate();
    task.result_contract = c::TaskContractDocument::Report { words: f::LONG_WORDS };
    assert_golden(
        "task_report_ended",
        delegate_answer(ended(task, c::Ending::Done, Some(c::ResultValue::Report(f::document_text(REPORT))))),
    );
}
#[test]
fn a_tracked_goal_shows_its_deadline_and_instruction_preview() {
    let mut task = f::goal(f::TASK, f::waiting_on_me());
    task.budget.deadline = Some(Wall::from_nanos(1_792_861_200_000_000_000));
    task.instructions = Some(f::document_text(INSTRUCTIONS));
    assert_golden("task_goal_tracked", f::read_answer(f::TASK_CALL, f::task_result(task)));
}
#[test]
fn a_tasks_dependencies_show_their_states_and_added_facts() {
    assert_golden("task_dependencies", f::read_answer(f::TASK_CALL, f::task_result(f::goal_with_dependencies())));
}
#[test]
fn a_tasks_json_keeps_dependency_facts_in_document_order() {
    let printed = f::read_answer(f::TASK_JSON_CALL, f::task_result(f::goal_with_dependencies()));
    assert_eq!(printed.status, 0);
    let from = skein_lib::bytes::find(&printed.stdout, b"\"dependency_facts\":").expect("dependency facts");
    assert_eq!(&printed.stdout[from - 2..from], b"],");
    let tail = &printed.stdout[from..];
    let to = skein_lib::bytes::find(tail, b",\"references\":").expect("next field");
    assert_eq!(
        &tail[..to],
        concat!(
            "\"dependency_facts\":[",
            "{\"number\":45,\"pending\":true,\"added\":{\"at\":1792645800000000000,\"by\":{\"task\":7}},\"keeps\":false,\"status\":null},",
            "{\"number\":43,\"pending\":false,\"added\":null,\"keeps\":false,\"status\":\"done\"},",
            "{\"number\":44,\"pending\":true,\"added\":{\"at\":1792645800000000000,\"by\":{\"person\":73}},\"keeps\":true,\"status\":null}]",
        )
        .as_bytes(),
    );
}
#[test]
fn a_task_without_dependencies_has_no_dependencies_line() {
    let printed = f::read_answer(f::TASK_CALL, f::task_result(f::goal(f::TASK, c::WaitsOn::Nothing)));
    assert_eq!(printed.status, 0);
    assert!(skein_lib::bytes::find(&printed.stdout, b"dependencies:").is_none());
}
#[test]
fn a_tasks_json_always_carries_dependency_facts() {
    let printed = f::read_answer(f::TASK_JSON_CALL, f::task_result(f::goal(f::TASK, c::WaitsOn::Nothing)));
    assert_eq!(printed.status, 0);
    assert!(skein_lib::bytes::find(&printed.stdout, b"\"dependencies\":[],\"dependency_facts\":[]").is_some());
}
fn dependency_text(task: c::Task, expected: &[u8]) {
    let printed = f::read_answer(f::TASK_CALL, f::task_result(task));
    assert_eq!(printed.status, 0);
    let from = skein_lib::bytes::find(&printed.stdout, b"dependencies:").expect("dependency lines");
    let tail = &printed.stdout[from..];
    let to = skein_lib::bytes::find(tail, b"\nwaiting on:").expect("next field");
    assert_eq!(&tail[..to], expected);
}
#[test]
fn stored_dependencies_use_only_their_supplied_status() {
    for (status, expected) in [
        (None, b"dependencies:\n  task 43".as_slice()),
        (Some(c::OwnerTaskStatus::Done), "dependencies:\n  task 43 · Completed".as_bytes()),
        (Some(c::OwnerTaskStatus::Failed), "dependencies:\n  task 43 · Failed".as_bytes()),
        (Some(c::OwnerTaskStatus::Cancelled), "dependencies:\n  task 43 · Cancelled".as_bytes()),
    ] {
        let mut task = f::goal_with_dependencies();
        task.dependencies = List::with_capacity(1);
        task.dependencies.push(c::OwnerTaskLink::Stored { number: 43, status }).expect("stored dependency");
        dependency_text(task, expected);
    }
}
#[test]
fn added_dependencies_use_requester_words() {
    for (by, expected) in [
        (c::OwnerRequester::Person(3), "dependencies:\n  task 45 · Waiting · added by party 3 at 2026-10-22 05:10 UTC"),
        (
            c::OwnerRequester::Deployment { project: 1 },
            "dependencies:\n  task 45 · Waiting · added by deployment 1 at 2026-10-22 05:10 UTC",
        ),
    ] {
        let mut task = f::goal_with_dependencies();
        let link = task.dependencies.get(2).expect("retained dependency").clone();
        task.dependencies = List::with_capacity(1);
        task.dependencies.push(link).expect("one dependency");
        task.dependency_facts.get_mut(0).expect("added fact").added.as_mut().expect("amendment").by = by;
        dependency_text(task, expected.as_bytes());
    }
}
#[test]
fn dependency_facts_without_a_link_print_nothing() {
    let mut task = f::goal_with_dependencies();
    task.dependency_facts.get_mut(0).expect("added fact").number = 99;
    dependency_text(
        task,
        concat!(
            "dependencies:\n  task 43 · Completed",
            "\n  task 44 · Running · added by you at 2026-10-22 05:10 UTC · keeps task 41 from ending",
            "\n  task 45 · Waiting",
        )
        .as_bytes(),
    );
}
#[test]
fn dependency_keeps_is_independent_of_added() {
    let mut task = f::goal_with_dependencies();
    task.dependency_facts.get_mut(2).expect("keeping fact").added = None;
    dependency_text(
        task,
        concat!(
            "dependencies:\n  task 43 · Completed",
            "\n  task 44 · Running · keeps task 41 from ending",
            "\n  task 45 · Waiting · added by task 7 at 2026-10-22 05:10 UTC",
        )
        .as_bytes(),
    );
}
#[test]
fn dependency_lines_share_the_tasks_text_budget() {
    let printed = f::read_answer(f::TASK_CALL, f::task_result(f::goal_with_dependencies_and_long_specification()));
    assert_eq!(printed.status, 0);
    assert!(printed.stdout.len() <= usize::try_from(crate::limits::TEXT).expect("text limit fits"));
    assert!(skein_lib::bytes::find(&printed.stdout, b"bytes left\n").is_some());
    assert!(
        skein_lib::bytes::find(
            &printed.stdout,
            "task 44 · Running · added by you at 2026-10-22 05:10 UTC · keeps task 41 from ending".as_bytes(),
        )
        .is_some()
    );
}
#[test]
fn report_activity_names_phase_changes_without_history_positions() {
    let mut rows = List::with_capacity(ACTIVITY_ROWS);
    for (position, line) in [
        (3, c::OwnerActivity::Milestone { position: 15 }),
        (
            2,
            c::OwnerActivity::Words {
                message: 1,
                by: c::OwnerRequester::Task(1),
                kind: c::OwnerMessageKind::Words,
                words: f::document_text(b"widget --csv prints the table").bytes,
            },
        ),
        (1, c::OwnerActivity::Milestone { position: 1 }),
    ] {
        rows.push(c::ActivityLine {
            position,
            at: Wall::EPOCH,
            event: c::Activity::Owner {
                source: Box::new(c::OwnerActivityLine { task: DELEGATE, position, at: Wall::EPOCH, line }),
                decoded: None,
            },
        })
        .expect("three activity rows");
    }
    let result = c::ReadResult::Value(c::ReadValue::Activity(Box::new(c::ActivityPage {
        rows,
        cut: false,
        before: None,
        after: None,
    })));
    assert_golden("activity_report", f::read_answer(&["activity", "2"], result));
}
fn result_text(ending: c::Ending, value: Option<c::ResultValue>, expected: &[u8]) {
    let printed = delegate_answer(ended(delegate(), ending, value));
    let from = skein_lib::bytes::find(&printed.stdout, b"result: ").expect("result line");
    let tail = &printed.stdout[from..];
    let to = match skein_lib::bytes::find(tail, b"next: ") {
        Some(to) => to,
        None => skein_lib::bytes::find(tail, b"digest: ").expect("digest line"),
    };
    assert_eq!(&tail[..to], expected);
}
#[test]
fn completed_results_print_each_values_words() {
    for (value, expected) in [
        (None, b"result: Completed\n".as_slice()),
        (Some(c::ResultValue::Report(f::document_text(b"ready"))), b"result: Completed \xc2\xb7 ready\n"),
        (
            Some(c::ResultValue::OwnerVerdict {
                verdict: 1,
                words: f::document_text(b"sound"),
                fields: List::with_capacity(0),
            }),
            b"result: Completed \xc2\xb7 sound\n",
        ),
        (
            Some(c::ResultValue::Verdict {
                verdict: 1,
                words: f::document_text(b"sound"),
                fields: List::with_capacity(0),
                tasks: List::with_capacity(0),
                proposal: None,
            }),
            b"result: Completed \xc2\xb7 sound\n",
        ),
        (
            Some(c::ResultValue::Approved { state: Box::from(*b"ready"), words: Some(f::document_text(b"checked")) }),
            b"result: Completed \xc2\xb7 ready \xc2\xb7 checked\n",
        ),
        (
            Some(c::ResultValue::Approved { state: Box::from(*b"ready"), words: None }),
            b"result: Completed \xc2\xb7 ready\n",
        ),
        (
            Some(c::ResultValue::Choice { option: 2, by: party(), reason: f::document_text(b"smaller") }),
            b"result: Completed \xc2\xb7 option 2 \xc2\xb7 smaller\n",
        ),
        (
            Some(c::ResultValue::Failure(f::document_text(b"could not finish"))),
            b"result: Completed \xc2\xb7 could not finish\n",
        ),
        (Some(c::ResultValue::Words(f::document_text(b""))), b"result: Completed\n"),
        (Some(c::ResultValue::Words(f::document_text(b"one\ntwo"))), b"result: Completed\n  one\n  two\n"),
    ] {
        result_text(c::Ending::Done, value, expected);
    }
}
#[test]
fn connector_results_use_the_shared_part_words() {
    let c::OwnerPartKey::Path(path) = part().key else { panic!("branch path") };
    result_text(
        c::Ending::Done,
        Some(c::ResultValue::Connector {
            summary: f::document_text(b"ready"),
            part: c::Part::Resource { connector: FORGE, path },
        }),
        b"result: Completed \xc2\xb7 ready \xc2\xb7 branch temper/1/c2 of repository 1 on forge 1\n",
    );
}
#[test]
fn changes_keep_non_commit_words_and_indent_multiline_words() {
    for (words, expected) in [
        (b"landed it".as_slice(), b"result: Landed \xc2\xb7 landed it\n".as_slice()),
        (b"one\ntwo", b"result: Landed\n  one\n  two\n"),
        (b"", b"result: Landed\n"),
    ] {
        result_text(
            c::Ending::Done,
            Some(c::ResultValue::Change { kind: 1, part: part(), words: f::document_text(words) }),
            expected,
        );
    }
}
#[test]
fn endings_print_their_states_and_supplied_reasons() {
    for (ending, expected) in [
        (c::Ending::Failed { reason: None }, b"result: Failed\n".as_slice()),
        (c::Ending::Failed { reason: Some(f::document_text(b"broken").bytes) }, b"result: Failed \xc2\xb7 broken\n"),
        (c::Ending::Cancelled { by: c::Actor::Task(1), reason: None, at: None }, b"result: Cancelled\n"),
        (
            c::Ending::Cancelled { by: c::Actor::Task(1), reason: Some(f::document_text(b"stopped").bytes), at: None },
            b"result: Cancelled \xc2\xb7 stopped\n",
        ),
        (c::Ending::Withdrawn { reason: f::document_text(b"one\ntwo").bytes }, b"result: Cancelled\n  one\n  two\n"),
        (c::Ending::Deleted, b"result: Ended\n"),
        (
            c::Ending::Decided {
                by: f::PARTY,
                party: None,
                how: Some(c::Decision::Rejected { reason: f::document_text(b"too large").bytes }),
                at: Wall::EPOCH,
            },
            b"result: Ended \xc2\xb7 rejected \xc2\xb7 too large\n",
        ),
        (
            c::Ending::Task(Box::new(c::OwnerChipEnding::Done(c::ResultValue::Report(f::document_text(b"ready"))))),
            b"result: Completed \xc2\xb7 ready\n",
        ),
        (
            c::Ending::Task(Box::new(c::OwnerChipEnding::Failed(f::document_text(b"broken")))),
            b"result: Failed \xc2\xb7 broken\n",
        ),
        (
            c::Ending::Task(Box::new(c::OwnerChipEnding::Cancelled {
                reason: f::document_text(b"stopped"),
                result: None,
                by: c::OwnerRequester::Task(1),
                at: Wall::EPOCH,
            })),
            b"result: Cancelled \xc2\xb7 stopped\n",
        ),
    ] {
        result_text(ending, None, expected);
    }
}
#[test]
fn multiline_results_stay_within_the_text_window() {
    let mut words = skein_lib::Writer::new(usize::try_from(f::LONG_WORDS).expect("size fits"));
    for _ in 0..f::LONG_WORDS {
        words.put(b"\n").expect("fixture size");
    }
    let printed = delegate_answer(ended(
        delegate(),
        c::Ending::Done,
        Some(c::ResultValue::Report(f::document_text(&words.finish()))),
    ));
    assert!(printed.stdout.len() <= usize::try_from(crate::limits::TEXT).expect("limit fits"));
    assert!(skein_lib::bytes::find(&printed.stdout, b"bytes left\n").is_some());
}

fn party() -> c::Party {
    let party = f::me(f::PARTY).party;
    c::Party { number: party.number, kind: party.kind, name: party.name.bytes }
}
#[test]
fn a_compound_task_read_and_an_ended_wait_share_result_words() {
    let task = f::goal(f::TASK, c::WaitsOn::Nothing);
    let read = ended(task, c::Ending::Done, Some(c::ResultValue::Report(f::document_text(REPORT))));
    let expected = f::read_answer(f::TASK_CALL, read.clone());
    let (mut root, call) = f::plain_wait(f::UNTIL_ENDED);
    root.end(call, &f::task_watch(), c::WatchEnd::SubjectEnded);
    root.serve(call, read.clone());
    assert_eq!(root.answer(call).expect("ended wait").stdout, f::appended(&expected.stdout, b"met\n"));
    let (mut root, call) = f::read_call(&["task", "41", "--activity", "1"]);
    root.serve(call, read);
    root.serve(call, f::activity_words());
    let printed = root.answer(call).expect("compound read");
    assert!(printed.stdout.starts_with(&expected.stdout));
}

fn instructions(words: &[u8], cut: bool, expected: &[u8]) {
    let mut task = f::goal(f::TASK, c::WaitsOn::Nothing);
    task.instructions = Some(c::DocumentText { bytes: f::document_text(words).bytes, cut });
    let printed = f::read_answer(f::TASK_CALL, f::task_result(task));
    let from = skein_lib::bytes::find(&printed.stdout, b"instructions: ").expect("instructions");
    let tail = &printed.stdout[from..];
    let to = skein_lib::bytes::find(tail, b"\n").expect("next line");
    assert_eq!(&tail[..to], expected);
}
#[test]
fn instructions_keep_short_words_and_name_held_cuts() {
    instructions(b"short", false, b"instructions: short");
    instructions(b"short", true, b"instructions: short (5 bytes held, cut; --json prints what is held)");
    instructions(b"first\nsecond", false, "instructions: first… (12 bytes; --json prints them whole)".as_bytes());
    instructions(
        b"first\nsecond",
        true,
        "instructions: first… (12 bytes held, cut; --json prints what is held)".as_bytes(),
    );
}
#[test]
fn instruction_previews_count_characters_and_keep_utf8_whole() {
    instructions(
        "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxé".as_bytes(),
        false,
        "instructions: xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxé".as_bytes(),
    );
    instructions("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxéZ".as_bytes(), false,
        "instructions: xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxé… (82 bytes; --json prints them whole)".as_bytes());
}

#[test]
fn a_held_tasks_text_names_its_reason_and_release_revision() {
    for (reason, words) in [
        (c::HoldCategory::Effects, b"Effects need a decision".as_slice()),
        (
            c::HoldCategory::Exceeded(c::Exceeded { bound: c::Bound::Items, value: 4 }),
            b"configured allowance of 4".as_slice(),
        ),
    ] {
        let task = f::held_task(f::goal(f::TASK, c::WaitsOn::Nothing), reason);
        let answer = f::read_answer(f::TASK_CALL, f::task_result(task));
        assert_eq!(answer.status, 0);
        assert!(skein_lib::bytes::find(&answer.stdout, words).is_some());
        assert!(skein_lib::bytes::find(&answer.stdout, b"release 41 --hold 9").is_some());
    }
}

#[test]
fn a_held_tasks_text_keeps_the_effect_source_supplied_by_the_read() {
    let mut task = f::held_task(f::goal(f::TASK, c::WaitsOn::Nothing), c::HoldCategory::EffectFailed);
    let Some(c::TaskHold::Owner(hold)) = task.hold.as_mut() else { panic!("owner hold") };
    hold.source = Some(part());
    let answer = f::read_answer(f::TASK_CALL, f::task_result(task));
    assert_eq!(answer.status, 0);
    assert!(skein_lib::bytes::find(&answer.stdout, b"source: ").is_some());
    assert!(skein_lib::bytes::find(&answer.stdout, b"temper/1/c2").is_some());
    assert!(
        skein_lib::bytes::find(&answer.stdout, b"review and resolve the effects, then release 41 --hold 9").is_some()
    );
}
