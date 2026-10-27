use crate::{
    jig::{
        argv::Usage,
        call::{Value, Values},
    },
    tests::fixtures as f,
};
use jig_client::contract as c;
use skein_lib::{List, Wall};

const REVISION: Option<c::Revision> = Some(c::Revision { value: 7 });
const DEADLINE: Wall = Wall::from_nanos(1_893_553_440_000_000_000); // 2030-01-02T03:04Z.
const DEADLINE_CALL: &[&str] = &["task", "amend", "41", "--rev", "7", "--deadline", "2030-01-02T03:04Z", "because"];
fn task() -> c::Task {
    let mut task = f::choice_task(c::Admission::Allowed);
    task.authority.cut = false;
    task.authority.authority.deadline = None;
    task.spec.cut = false;
    task.spec.specification.words.cut = false;
    task.dependencies = List::with_capacity(1);
    task.dependencies.push(c::OwnerTaskLink::Stored { number: 43, status: None }).unwrap();
    task
}
fn no_change() -> c::Amendment {
    c::Amendment {
        addressed: None,
        spec: None,
        wake: None,
        dependencies: None,
        authority: None,
        instructions: None,
        tracked: None,
        llm: None,
    }
}
fn expected(change: c::Amendment, revision: Option<c::Revision>) -> c::Ask {
    c::Ask::Amend {
        project: f::PROJECT,
        task: f::TASK,
        revision,
        change: Box::new(change),
        reason: f::act_text(b"because"),
    }
}
fn sends(argv: &[&str], task: c::Task, ask: c::Ask) {
    let (root, call) = f::send_choice_amend(argv, task);
    assert_eq!(f::act_ask(&root, call), &ask);
}
fn refuses(argv: &[&str], task: c::Task, error: &[u8]) {
    let (mut root, call) = f::send_choice_amend(argv, task);
    assert!(root.sends.is_empty());
    let answer = root.answer(call).expect("local refusal");
    assert_eq!((answer.status, answer.stderr.as_ref()), (1, error));
}
fn deadline_change(task: &c::Task, deadline: Option<Wall>) -> c::Amendment {
    let mut authority = task.authority.whole().expect("whole authority");
    authority.deadline = deadline;
    c::Amendment { authority: Some(authority), ..no_change() }
}
#[test]
fn amend_sends_a_priority_alone_without_a_revision() {
    sends(
        &["task", "amend", "41", "--priority", "3", "because"],
        task(),
        expected(c::Amendment { tracked: Some(c::Tracking::Tracked { priority: 3 }), ..no_change() }, None),
    );
}
#[test]
fn amend_sends_a_priority_with_an_explicit_revision() {
    sends(
        &["task", "amend", "41", "--rev", "7", "--priority", "3", "because"],
        task(),
        expected(c::Amendment { tracked: Some(c::Tracking::Tracked { priority: 3 }), ..no_change() }, REVISION),
    );
}
#[test]
fn amend_needs_a_revision_to_track_an_untracked_task() {
    let mut task = task();
    task.priority = None;
    refuses(&["task", "amend", "41", "--priority", "3", "because"], task, b"task amend needs --rev REV\n");
}
#[test]
fn amend_untracks_at_the_shown_revision() {
    sends(
        &["task", "amend", "41", "--rev", "7", "--untracked", "because"],
        task(),
        expected(c::Amendment { tracked: Some(c::Tracking::Untracked), ..no_change() }, REVISION),
    );
}
#[test]
fn amend_refuses_a_moved_revision_locally() {
    let mut task = task();
    task.spec_revision = c::Revision { value: 9 };
    refuses(DEADLINE_CALL, task, b"task 41 moved: its spec revision is 9, not 7; read again: task show 41\n");
}
#[test]
fn amend_restates_the_authority_with_its_deadline() {
    let task = task();
    let change = deadline_change(&task, Some(DEADLINE));
    sends(DEADLINE_CALL, task, expected(change, REVISION));
}
#[test]
fn amend_removes_the_deadline() {
    let mut task = task();
    task.authority.authority.deadline = Some(DEADLINE);
    let change = deadline_change(&task, None);
    sends(&["task", "amend", "41", "--rev", "7", "--no-deadline", "because"], task, expected(change, REVISION));
}
#[test]
fn amend_refuses_a_deadline_on_a_cut_authority() {
    let mut task = task();
    task.authority.cut = true;
    refuses(
        DEADLINE_CALL,
        task,
        b"task 41's authority is cut in this read; amend its deadline from a client with wider bounds\n",
    );
}
fn loaded(argv: &[&str], bytes: &[u8], index: usize, task: c::Task, ask: c::Ask) {
    let mut call = f::call(argv);
    call.values[index] = Some(Values::One(Value::Text(bytes.into())));
    let mut root = f::choice_root(f::choice_me(f::CHAT_BUDGET));
    let call = root.submit_prepared(call, false, None);
    root.serve(call, f::task_result(task));
    assert_eq!(f::act_ask(&root, call), &ask);
}
#[test]
fn amend_reads_instructions_from_a_file() {
    // The host supplies the loaded bytes; its file IO is tested by the shell's world.
    loaded(
        &["task", "amend", "41", "--rev", "7", "--instructions", "instructions.txt", "because"],
        b"Run checks",
        6,
        task(),
        expected(c::Amendment { instructions: Some(f::act_text(b"Run checks")), ..no_change() }, REVISION),
    );
}
#[test]
fn amend_reads_instructions_from_stdin() {
    loaded(
        &["task", "amend", "41", "--rev", "7", "--instructions", "-", "because"],
        b"Run checks",
        6,
        task(),
        expected(c::Amendment { instructions: Some(f::act_text(b"Run checks")), ..no_change() }, REVISION),
    );
}
#[test]
fn amend_sends_empty_instructions() {
    let mut task = task();
    task.instructions = Some(f::document_text(b"Run checks"));
    loaded(
        &["task", "amend", "41", "--rev", "7", "--instructions", "-", "because"],
        b"",
        6,
        task,
        expected(c::Amendment { instructions: Some(f::act_text(b"")), ..no_change() }, REVISION),
    );
}
#[test]
fn amend_sets_the_spec_words_keeping_its_parameters() {
    let mut task = task();
    task.spec.specification.parameters = List::with_capacity(1);
    task.spec
        .specification
        .parameters
        .push(c::ParameterDocument::Bytes { name: 1, value: f::document_text(b"debug") })
        .unwrap();
    task.spec.specification.inputs = List::with_capacity(1);
    task.spec.specification.inputs.push(43).unwrap();
    let mut spec = task.spec.whole().expect("whole spec");
    spec.words = f::act_text(b"Build and check");
    loaded(
        &["task", "amend", "41", "--rev", "7", "--spec", "-", "because"],
        b"Build and check",
        7,
        task,
        expected(c::Amendment { spec: Some(spec), ..no_change() }, REVISION),
    );
}
#[test]
fn amend_refuses_words_on_a_cut_spec() {
    let mut task = task();
    task.spec.cut = true;
    let mut call = f::call(&["task", "amend", "41", "--rev", "7", "--spec", "-", "because"]);
    call.values[7] = Some(Values::One(Value::Text(b"Build".as_slice().into())));
    let mut root = f::choice_root(f::choice_me(f::CHAT_BUDGET));
    let call = root.submit_prepared(call, false, None);
    root.serve(call, f::task_result(task));
    assert!(root.sends.is_empty());
    assert_eq!(
        root.answer(call).expect("cut spec").stderr.as_ref(),
        b"task 41's spec is cut in this read; amend its words from a client with wider bounds\n"
    );
}
#[test]
fn amend_sends_the_whole_dependency_list() {
    let mut dependencies = List::with_capacity(2);
    dependencies.push(47).unwrap();
    dependencies.push(43).unwrap();
    sends(
        &["task", "amend", "41", "--rev", "7", "--dependency", "47", "--dependency", "43", "because"],
        task(),
        expected(c::Amendment { dependencies: Some(dependencies), ..no_change() }, REVISION),
    );
}
#[test]
fn amend_removes_every_dependency() {
    sends(
        &["task", "amend", "41", "--rev", "7", "--no-dependencies", "because"],
        task(),
        expected(c::Amendment { dependencies: Some(List::with_capacity(0)), ..no_change() }, REVISION),
    );
}
#[test]
fn amend_proposes_a_deadline_past_the_ceiling() {
    let mut task = task();
    task.authority.authority.deadline = Some(Wall::from_nanos(1));
    let mut ceiling = task.authority.clone();
    ceiling.authority.deadline = Some(Wall::from_nanos(1_893_553_441_000_000_000));
    task.acts.get_mut(0).unwrap().admission = c::Admission::Bounded {
        within: c::Bounds::Amend {
            budget_left: 0,
            grants: List::with_capacity(0),
            implications: List::with_capacity(0),
            authority: Box::new(task.authority.clone()),
            ceiling: Box::new(ceiling),
        },
        beyond: c::Beyond::Proposed { to: c::Recipient::Party(f::RECIPIENT) },
    };
    let change = Box::new(deadline_change(&task, Some(DEADLINE)));
    sends(
        DEADLINE_CALL,
        task,
        c::Ask::Propose {
            project: f::PROJECT,
            action: c::ProposeAction::Amendment { task: f::TASK, revision: c::Revision { value: 7 }, change },
            reason: f::act_text(b"because"),
        },
    );
}
#[test]
fn amend_proposes_a_priority_at_the_base_revision() {
    let mut task = task();
    task.acts.get_mut(0).unwrap().admission = c::Admission::Proposed { to: c::Recipient::Party(f::RECIPIENT) };
    sends(
        &["task", "amend", "41", "--priority", "3", "because"],
        task,
        c::Ask::Propose {
            project: f::PROJECT,
            action: c::ProposeAction::Amendment {
                task: f::TASK,
                revision: c::Revision { value: 7 },
                change: Box::new(c::Amendment { tracked: Some(c::Tracking::Tracked { priority: 3 }), ..no_change() }),
            },
            reason: f::act_text(b"because"),
        },
    );
}
#[test]
fn amend_refuses_both_of_a_pair() {
    for (flags, argument) in [
        (["--priority", "3", "--untracked"].as_slice(), 3),
        (["--deadline", "2030-01-02T03:04Z", "--no-deadline"].as_slice(), 5),
        (["--dependency", "43", "--no-dependencies"].as_slice(), 9),
    ] {
        let argv = f::extended(&["task", "amend", "41", "--rev", "7", "because"], flags);
        assert_eq!(f::parse(argv.as_slice()), Err(Usage::Twice { argument }), "{flags:?}");
    }
}
#[test]
fn amend_refuses_a_malformed_deadline() {
    for deadline in [
        "2030-1-02T03:04Z",
        "2030-02-29T03:04Z",
        "2100-02-29T03:04Z",
        "2030-01-02T24:00Z",
        "2030-01-02T03:60Z",
        "2030-01-02T03:04:00Z",
        "2030-01-02T03:04+00:00",
        "1969-01-01T00:00Z",
        "9999-01-01T00:00Z",
    ] {
        assert_eq!(
            f::parse(&["task", "amend", "41", "--rev", "7", "--deadline", deadline, "because"]),
            Err(Usage::Kind { argument: 4 })
        );
    }
}
#[test]
fn amend_changes_nothing_is_refused() {
    refuses(&["task", "amend", "41", "--priority", "2", "because"], task(), b"amend changes nothing on task 41\n");
}
#[test]
fn the_old_spec_revision_form_asks_for_rev() {
    assert_eq!(
        f::parse(&["task", "amend", "41", "--spec", "8", "--model", "chosen-model", "because"]),
        Err(Usage::AmendmentRevision)
    );
}
#[test]
fn amend_refuses_a_repeated_flag() {
    for (flags, argument) in [
        (["--priority", "3", "--priority", "4"].as_slice(), 2),
        (["--untracked", "--untracked"].as_slice(), 3),
        (["--instructions", "one", "--instructions", "two"].as_slice(), 6),
        (["--model", "chosen-model", "--model", "plain-model"].as_slice(), 10),
    ] {
        let argv = f::extended(&["task", "amend", "41", "--rev", "7", "because"], flags);
        assert_eq!(f::parse(argv.as_slice()), Err(Usage::Twice { argument }), "{flags:?}");
    }
}
#[test]
fn amend_uses_stdin_only_once() {
    assert_eq!(
        f::parse(&["task", "amend", "41", "--rev", "7", "--instructions", "-", "--spec", "-", "because"]),
        Err(Usage::Stdin)
    );
}
#[test]
fn amend_requires_a_model_for_an_effort() {
    assert_eq!(
        f::parse(&["task", "amend", "41", "--rev", "7", "--effort", "high", "because"]),
        Err(Usage::Missing { argument: 10 })
    );
}
#[test]
fn amend_json_carries_texts_inline_including_a_dash() {
    let call = f::from_json(
        br#"{"tool":"task amend","arguments":{"task":41,"rev":7,"instructions":"-","spec":"Build","reason":"because"}}"#,
    );
    assert_eq!(call.values[6], Some(Values::One(Value::Text(b"-".as_slice().into()))));
    assert_eq!(f::from_json(&crate::jig::json::write_call(crate::table::TOOLS, &call).unwrap()), call);
}

#[test]
fn amend_deadlines_parse_epoch_and_leap_dates() {
    for (text, nanos) in [(b"1970-01-01T00:00Z".as_slice(), 0), (b"2000-02-29T00:00Z", 951_782_400_000_000_000)] {
        assert_eq!(crate::jig::argv::deadline(text), Some(Wall::from_nanos(nanos)));
    }
}

fn dependencies_at(count: u32) -> List<&'static str> {
    let mut argv = f::extended(&["task", "amend", "41", "--rev", "7", "because"], &[]);
    for _ in 0..count {
        argv.push("--dependency").expect("fixture word bound");
        argv.push("43").expect("fixture word bound");
    }
    argv
}
#[test]
fn amend_accepts_dependencies_at_the_argument_bound() {
    f::parse(dependencies_at(crate::limits::ROWS).as_slice()).expect("dependencies at the argument bound");
}
#[test]
fn amend_refuses_dependencies_past_the_argument_bound() {
    assert_eq!(f::parse(dependencies_at(crate::limits::ROWS + 1).as_slice()), Err(Usage::Many { argument: 8 }));
}

#[test]
fn amend_needs_a_revision_for_any_other_change() {
    for flags in [
        ["--untracked"].as_slice(),
        &["--deadline", "2030-01-02T03:04Z"],
        &["--no-deadline"],
        &["--instructions", "missing-file"],
        &["--spec", "8"],
        &["--dependency", "43"],
        &["--no-dependencies"],
        &["--model", "chosen-model"],
    ] {
        let argv = f::extended(&["task", "amend", "41", "because"], flags);
        assert_eq!(f::parse(argv.as_slice()), Err(Usage::AmendmentRevision), "{flags:?}");
    }
}
