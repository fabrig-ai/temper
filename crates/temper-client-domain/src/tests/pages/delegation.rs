use super::fixtures::{self, chat as f};
use crate::pages::delegation::{self, Facts, Kind, Said, State};
use jig_client::contract as c;
use skein_lib::{List, Wall};
const MERGE: &[u8] = b"abcdef0123456789abcdef0123456789abcdef0123";
const TASKS: u32 = 2;

#[test]
fn a_result_names_its_tasks_by_temper_words() {
    for (summary, expected) in [
        (f::WAKE_SUMMARY, [2].as_slice()),
        ("Begin…\n\n3: Result from task 7:\nok\n4: Result from task 9:\nwords".as_bytes(), &[7, 9]),
        (b"Result from task 5:\nx", &[5]),
        (b"she said Result from task 2:", &[]),
        (b"Result from task x:", &[]),
        (b"9: Result from task 18446744073709551616:", &[]),
        (b"Result from task 2:\n3: Result from task 2:\nResult from task 7:", &[2, 7]),
        (b"x: Result from task 2:\nResult from task  2:\nResult from task 2 :", &[]),
    ] {
        let mut tasks = List::with_capacity(TASKS);
        delegation::results(summary, &mut tasks);
        assert_eq!(tasks.as_slice(), expected);
    }
    let mut tasks = List::with_capacity(1);
    tasks.push(7).expect("full receiver");
    delegation::results(f::WAKE_SUMMARY, &mut tasks);
    assert_eq!(tasks.as_slice(), [7]);
}

#[test]
fn a_delegate_call_names_its_tasks_by_its_objects() {
    let mut call = f::delegate_call(2);
    call.objects = List::with_capacity(3);
    for object in [c::ObjectKey::Task(7), c::ObjectKey::Proposal(fixtures::acts::PROPOSAL), c::ObjectKey::Task(5)] {
        call.objects.push(object).expect("object");
    }
    let mut tasks = List::with_capacity(TASKS);
    delegation::delegated(&call, &mut tasks);
    assert_eq!(tasks.as_slice(), [7, 5], "objects take precedence over the acknowledgement");
}
#[test]
fn a_delegate_call_names_its_tasks_by_its_acknowledgement() {
    let mut call = f::delegate_call(2);
    call.objects.clear();
    let mut tasks = List::with_capacity(TASKS);
    delegation::delegated(&call, &mut tasks);
    assert_eq!(tasks.as_slice(), [2, 3]);
}
#[test]
fn a_failed_delegate_call_names_no_task() {
    let mut call = f::delegate_call(2);
    call.failed = true;
    let mut tasks = List::with_capacity(TASKS);
    delegation::delegated(&call, &mut tasks);
    assert!(tasks.is_empty());
}
#[test]
fn another_tool_names_no_task() {
    let mut call = f::delegate_call(2);
    call.tool = fixtures::acts::text(b"message");
    let mut tasks = List::with_capacity(TASKS);
    delegation::delegated(&call, &mut tasks);
    assert!(tasks.is_empty());
}
#[test]
fn a_delegates_bad_acknowledgement_numbers_are_skipped() {
    let mut call = f::delegate_call(2);
    call.objects.clear();
    call.outcome =
        Some(c::Text::new(Box::from(*b"Delegated tasks: x, 2, 18446744073709551616, 3x, , 3"), 128).expect("ack"));
    let mut tasks = List::with_capacity(TASKS);
    delegation::delegated(&call, &mut tasks);
    assert_eq!(tasks.as_slice(), [2, 3]);
}
#[test]
fn a_delegates_tasks_stop_at_the_receivers_bound() {
    for objects in [true, false] {
        let mut call = f::delegate_call(2);
        if !objects {
            call.objects.clear();
        }
        let mut tasks = List::with_capacity(1);
        delegation::delegated(&call, &mut tasks);
        delegation::delegated(&call, &mut tasks);
        assert_eq!(tasks.as_slice(), [2]);
    }
}
#[test]
fn delegation_watches_leave_room_for_the_chats_other_watches() {
    let mut limits = fixtures::limits();
    for (reads, watches, expected) in [(16, 16, 12), (2, 16, 2), (16, 4, 0), (16, 3, 0)] {
        limits.client.domain.reads = reads;
        limits.client.domain.watches = watches;
        assert_eq!(delegation::bound(&limits), expected);
    }
}
#[test]
fn a_running_change_has_no_steps_done() {
    let task = f::change();
    assert_eq!(
        delegation::facts(&task),
        Facts {
            task: f::TASK,
            kind: Kind::Change,
            state: State::Running,
            title: task.chip.title.bytes(),
            said: None,
            steps: Some(0)
        }
    );
}
#[test]
fn a_held_change_is_held() {
    let mut task = f::change();
    f::phase(&mut task, c::OwnerChipPhase::Held { reason: c::HoldCategory::Stopped, revision: 1 });
    let facts = delegation::facts(&task);
    assert_eq!(facts.state, State::Held);
    assert_eq!(facts.steps, Some(0));
    assert_eq!(facts.said, None);
}
#[test]
fn a_landed_change_says_its_merge_commit() {
    let mut task = f::change();
    task.result = Some(c::TaskResult::Ending(Box::new(c::OwnerChipEnding::Done(f::change_value(MERGE)))));
    let facts = delegation::facts(&task);
    assert_eq!(facts.state, State::Landed);
    assert_eq!(facts.said, Some(Said::Merge(b"abcdef0")));
    assert_eq!(facts.steps, Some(4));
}
#[test]
fn a_failed_change_says_its_reason() {
    let mut task = f::change();
    task.result = Some(c::TaskResult::Ending(Box::new(c::OwnerChipEnding::Failed(f::document_text(b"CI failed")))));
    let facts = delegation::facts(&task);
    assert_eq!(facts.state, State::Failed);
    assert_eq!(facts.said, Some(Said::Words(b"CI failed")));
    assert_eq!(facts.steps, Some(0));
}
#[test]
fn a_cancelled_task_is_cancelled() {
    let mut task = f::change();
    task.result = Some(c::TaskResult::Ending(Box::new(c::OwnerChipEnding::Cancelled {
        reason: f::document_text(b"No longer needed"),
        result: None,
        by: c::OwnerRequester::Person(fixtures::PARTY),
        at: Wall::EPOCH,
    })));
    let facts = delegation::facts(&task);
    assert_eq!(facts.state, State::Cancelled);
    assert_eq!(facts.said, Some(Said::Words(b"No longer needed")));
    assert_eq!(facts.steps, Some(0));
}
#[test]
fn a_done_report_says_its_words() {
    let mut task = f::change();
    task.result_contract = c::TaskContractDocument::Report { words: 64 };
    task.result = Some(c::TaskResult::Document(Box::new(f::result(
        &task,
        c::Ending::Done,
        Some(c::ResultValue::Report(f::document_text(b"Found it"))),
    ))));
    let facts = delegation::facts(&task);
    assert_eq!(facts.kind, Kind::Report);
    assert_eq!(facts.state, State::Done);
    assert_eq!(facts.said, Some(Said::Words(b"Found it")));
    assert_eq!(facts.steps, None);
}
#[test]
fn a_landed_change_without_a_commit_id_says_its_words() {
    for words in [b"Landed".as_slice(), b"ABCDEF0123456789", b"This change landed"] {
        let mut task = f::change();
        task.result = Some(c::TaskResult::Ending(Box::new(c::OwnerChipEnding::Done(f::change_value(words)))));
        assert_eq!(delegation::facts(&task).said, Some(Said::Words(words)));
    }
}
#[test]
fn the_tasks_result_precedes_its_chip_ending_and_hold() {
    let mut task = f::change();
    f::phase(&mut task, c::OwnerChipPhase::Held { reason: c::HoldCategory::Stopped, revision: 1 });
    let c::ChipPhase::Owner { details, .. } = task.chip.details.as_mut() else { panic!("owner") };
    details.ending = Some(c::OwnerChipEnding::Failed(f::document_text(b"Old failure")));
    task.result =
        Some(c::TaskResult::Document(Box::new(f::result(&task, c::Ending::Done, Some(f::change_value(MERGE))))));
    assert_eq!(delegation::facts(&task).state, State::Landed);
    assert_eq!(delegation::facts(&task).said, Some(Said::Merge(b"abcdef0")));
}
#[test]
fn a_chips_owner_ending_supplies_the_result() {
    let mut task = f::change();
    let c::ChipPhase::Owner { details, .. } = task.chip.details.as_mut() else { panic!("owner") };
    details.ending = Some(c::OwnerChipEnding::Done(f::change_value(MERGE)));
    assert_eq!(delegation::facts(&task).state, State::Landed);
}
#[test]
fn an_ended_chip_supplies_its_state_and_reason() {
    for (how, state, said) in [
        (c::Ending::Done, State::Done, None),
        (
            c::Ending::Failed { reason: Some(fixtures::acts::text(b"Failed")) },
            State::Failed,
            Some(Said::Words(b"Failed")),
        ),
        (c::Ending::Cancelled { by: c::Actor::Party(fixtures::PARTY), reason: None, at: None }, State::Cancelled, None),
        (
            c::Ending::Withdrawn { reason: fixtures::acts::text(b"Withdrawn") },
            State::Cancelled,
            Some(Said::Words(b"Withdrawn")),
        ),
        (c::Ending::Deleted, State::Ended, None),
    ] {
        let mut task = f::change();
        *task.chip.details = c::ChipPhase::Ended { how, at: Wall::EPOCH };
        let facts = delegation::facts(&task);
        assert_eq!(facts.state, state);
        assert_eq!(facts.said, said);
        assert_eq!(facts.steps, Some(0));
    }
}

#[test]
fn merge_commits_are_whole_lowercase_hex_ids_of_at_least_seven_bytes() {
    for (words, expected) in [
        (MERGE, Some(MERGE)),
        (
            b"abc1234\npull request #7 of acme/app on github.com: https://github.com/acme/app/pull/7",
            Some(b"abc1234".as_slice()),
        ),
        (b"abc1234".as_slice(), Some(b"abc1234".as_slice())),
        (b"abc123", None),
        (b"ABC1234", None),
        (b"abc123g", None),
        (b"landed it", None),
        (b"", None),
    ] {
        assert_eq!(delegation::merge_commit(words), expected);
    }
}
