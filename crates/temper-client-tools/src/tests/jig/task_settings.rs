//! Settings lines and JSON use the revisions carried by the task document.
use crate::tests::fixtures as f;
use jig_client::contract as c;

fn golden(name: &str, printed: crate::jig::Printed) {
    assert_eq!(printed.status, 0);
    f::check_golden(name, &printed.stdout);
}

#[test]
fn task_settings_text_and_json_show_the_recorded_run_and_newest_revisions() {
    for (name, settings, json) in [
        (
            "task_settings_current",
            c::TaskSettings { task: 7, run: None, newest: 7 },
            b"{\"task\":7,\"run\":null,\"newest\":7}".as_slice(),
        ),
        (
            "task_settings_run",
            c::TaskSettings { task: 7, run: Some(6), newest: 9 },
            b"{\"task\":7,\"run\":6,\"newest\":9}",
        ),
        (
            "task_settings_newest",
            c::TaskSettings { task: 7, run: None, newest: 9 },
            b"{\"task\":7,\"run\":null,\"newest\":9}",
        ),
    ] {
        let mut task = f::goal_with_dependencies();
        task.instructions = Some(f::document_text(b"Read the task's charter."));
        task.settings = Some(settings);
        golden(name, f::read_answer(f::TASK_CALL, f::task_result(task.clone())));
        let printed = f::read_answer(f::TASK_JSON_CALL, f::task_result(task));
        assert_eq!(printed.status, 0);
        let from = skein_lib::bytes::find(&printed.stdout, b"\"settings\":").expect("settings member")
            + b"\"settings\":".len();
        let end = from + json.len();
        assert_eq!(&printed.stdout[from..end], json);
        assert_eq!(printed.stdout[end], b',');
    }
}

#[test]
fn procedure_task_text_and_json_carry_no_settings_revision() {
    let mut task = f::goal(f::TASK, c::WaitsOn::Nothing);
    task.chip.executor = c::Executor::Procedure { connector: 1, kind: 1 };
    task.settings = None;
    task.instructions = None;
    golden("task_settings_procedure", f::read_answer(f::TASK_CALL, f::task_result(task.clone())));
    let printed = f::read_answer(f::TASK_JSON_CALL, f::task_result(task));
    assert_eq!(printed.status, 0);
    assert!(skein_lib::bytes::find(&printed.stdout, b"\"settings\":null,").is_some());
}
