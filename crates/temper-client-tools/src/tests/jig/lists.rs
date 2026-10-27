use crate::tests::fixtures;
use jig_client::contract as c;
#[test]
fn a_list_prints_its_rows_then_its_continuation() {
    let mut root = fixtures::signed_in(fixtures::PARTY);
    let call = root.submit(fixtures::TASKS_CALL);
    root.serve(call, fixtures::task_page_with_cursor());
    assert_eq!(root.answer(call).expect("page answered").stdout, fixtures::golden("tasks_next"));
}
#[test]
fn ended_tasks_use_only_the_ended_group() {
    let (root, call) = fixtures::read_call(fixtures::ENDED_CALL);
    assert_eq!(
        root.reads.get(&call).expect("task query").1,
        c::Read::Tasks {
            query: c::TaskQuery {
                project: Some(fixtures::PROJECT),
                requester: None,
                root: None,
                parent: None,
                label: None,
                phase: Some(c::PhaseClass::Ended),
                executor: None,
                tracked: None,
                order: c::QueryOrder::Number
            },
            live: None,
            ended: Some(fixtures::cursor()),
        }
    );
}
#[test]
fn a_composite_task_prints_one_answer() {
    let (mut root, call) = fixtures::composite();
    fixtures::check_golden("task_composite", &root.answer(call).expect("all parts answered").stdout);
}
#[test]
fn a_composite_task_closes_its_related_watches() {
    let (root, _) = fixtures::composite();
    assert_eq!(root.closed.len(), fixtures::RELATED_WATCHES);
}
#[test]
fn an_inbox_cursor_of_another_group_than_its_kind_exits_1() {
    assert_eq!(fixtures::parse(fixtures::WRONG_INBOX_GROUP), Err(crate::jig::argv::Usage::Kind { argument: 3 }));
}

#[test]
fn an_inbox_reads_on_in_needs_while_needs_has_more() {
    let (mut root, call) = fixtures::read_call(fixtures::INBOX_CALL);
    root.serve(call, fixtures::inbox_page(true));
    root.deliver_subject(
        &fixtures::inbox_watch(),
        c::Delivery::Inbox(Box::new(fixtures::inbox_window(fixtures::PAGE_ROWS, 0, true))),
    );
    let printed = root.answer(call).expect("read and window answered");
    assert_eq!(printed.stdout, fixtures::golden("inbox_needs_next"));
}
#[test]
fn an_inbox_cursor_selects_its_group() {
    let (root, call) = fixtures::read_call(fixtures::NEEDS_NEXT_CALL);
    assert_eq!(
        root.reads.get(&call).expect("inbox read").1,
        c::Read::Inbox {
            group: c::InboxReadGroup::Needs,
            project: None,
            kind: None,
            goal: None,
            after: Some(fixtures::cursor())
        }
    );
}

#[test]
fn inbox_prints_what_needs_the_party_then_updates() {
    let printed = fixtures::inbox_answer(fixtures::PAGE_ROWS, fixtures::PAGE_ROWS, false);
    assert_eq!(printed.stdout, fixtures::golden("inbox_both"));
}
#[test]
fn an_inbox_reads_on_in_updates_once_needs_ends() {
    let printed = fixtures::inbox_answer(fixtures::PAGE_ROWS, fixtures::PAGE_ROWS, true);
    assert_eq!(printed.stdout, fixtures::golden("inbox_updates_next"));
}
#[test]
fn an_inbox_page_whose_updates_do_not_fit_reads_on_at_the_start_of_updates() {
    let printed = fixtures::inbox_answer(fixtures::FULL_NEEDS, fixtures::PAGE_ROWS, true);
    assert_eq!(printed.stdout, fixtures::golden("inbox_updates_start"));
}
#[test]
fn an_inbox_group_byte_alone_starts_the_group() {
    let (root, call) = fixtures::read_call(fixtures::UPDATES_START_CALL);
    assert_eq!(
        root.reads.get(&call).expect("updates read").1,
        c::Read::Inbox { group: c::InboxReadGroup::Updates, project: None, kind: None, goal: None, after: None }
    );
}
#[test]
fn a_limited_activity_reads_on_with_its_page_whole() {
    let printed = fixtures::read_answer(fixtures::LIMITED_ACTIVITY_CALL, fixtures::activity_words());
    assert_eq!(printed.stdout, fixtures::golden("activity_limited"));
}
#[test]
fn a_limited_task_activity_names_its_whole_page() {
    let (mut root, call) = fixtures::read_call(fixtures::LIMITED_TASK_CALL);
    root.serve(call, fixtures::task_result(fixtures::goal(fixtures::TASK, fixtures::waiting_on_me())));
    root.serve(call, fixtures::activity_words());
    fixtures::check_golden("task_limited_activity", &root.answer(call).expect("task answered").stdout);
}

#[test]
fn ended_tasks_keep_their_flag_in_the_continuation() {
    let printed = fixtures::read_answer(fixtures::ENDED_CALL, fixtures::ended_page());
    assert_eq!(printed.stdout, fixtures::golden("tasks_ended_next"));
}

#[test]
fn task_labels_use_jigs_label_numbers() {
    for (word, label) in fixtures::TASK_LABELS {
        let argv = fixtures::extended(fixtures::TASKS_CALL, &["--label", word]);
        let (root, call) = fixtures::read_call(argv.as_slice());
        let c::Read::Tasks { query, .. } = &root.reads.get(&call).expect("task query").1 else { panic!("tasks query") };
        assert_eq!(query.label, Some(*label), "{word}");
    }
}

#[test]
fn a_kept_activity_is_parked_in_text_and_preserved_in_json() {
    for (argv, expected) in [
        (&["task", "activity", "41"][..], b"parked".as_slice()),
        (&["task", "activity", "41", "--json"][..], br#""kept":{"attempt":7,"pending":[]}"#),
    ] {
        let c::ReadResult::Value(c::ReadValue::Activity(mut page)) = fixtures::activity_words() else {
            panic!("activity")
        };
        page.rows.get_mut(0).expect("row").event = c::Activity::Owner {
            source: Box::new(c::OwnerActivityLine {
                task: fixtures::TASK,
                position: 1,
                at: skein_lib::Wall::EPOCH,
                line: c::OwnerActivity::Kept { attempt: 7, pending: skein_lib::List::with_capacity(0) },
            }),
            decoded: None,
        };
        let printed = fixtures::read_answer(argv, c::ReadResult::Value(c::ReadValue::Activity(page)));
        assert!(skein_lib::bytes::find(&printed.stdout, expected).is_some());
    }
}
#[test]
fn a_met_notice_keeps_its_dependency_and_removal_flag_in_json() {
    let c::ReadResult::Value(c::ReadValue::Activity(mut page)) = fixtures::activity_words() else { panic!("activity") };
    page.rows.get_mut(0).expect("row").event = c::Activity::Owner {
        source: Box::new(c::OwnerActivityLine {
            task: fixtures::TASK,
            position: 1,
            at: skein_lib::Wall::EPOCH,
            line: c::OwnerActivity::Words {
                message: 2,
                by: c::OwnerRequester::Task(12),
                kind: c::OwnerMessageKind::Met { dependency: 12, removed: true },
                words: fixtures::act_text(b""),
            },
        }),
        decoded: None,
    };
    let printed = fixtures::read_answer(
        &["task", "activity", "41", "--json"],
        c::ReadResult::Value(c::ReadValue::Activity(page)),
    );
    assert!(skein_lib::bytes::find(&printed.stdout, br#""met":{"dependency":12,"removed":true}"#).is_some());
}

#[test]
fn parent_tasks_keep_the_other_query_keys_and_continuation() {
    let argv = [
        "task",
        "list",
        "--project",
        "3",
        "--label",
        "goal",
        "--tracked",
        "--mine",
        "--ended",
        "--order",
        "priority",
        "--after",
        "1122",
        "--parent",
        "41",
    ];
    let (root, call) = fixtures::read_call(&argv);
    let c::Read::Tasks { query, live, ended } = &root.reads.get(&call).expect("task query").1 else { panic!("tasks") };
    assert_eq!(
        query,
        &c::TaskQuery {
            project: Some(3),
            requester: Some(c::OwnerRequester::Person(fixtures::PARTY)),
            root: None,
            parent: Some(41),
            label: Some(jig_client::shapes::GOAL),
            phase: Some(c::PhaseClass::Ended),
            executor: None,
            tracked: Some(true),
            order: c::QueryOrder::Priority,
        }
    );
    assert_eq!((live.as_ref(), ended.as_ref()), (None, Some(&fixtures::cursor())));
    assert_eq!(fixtures::back(&fixtures::call(&argv)), fixtures::owned(&argv));
    for bad in ["x", "-1", "18446744073709551616"] {
        assert_eq!(
            fixtures::parse(&["task", "list", "--project", "3", "--parent", bad]),
            Err(crate::jig::argv::Usage::Kind { argument: 7 })
        );
    }
}

#[test]
fn help_tasks_describes_parent() {
    let bytes = fixtures::help(Some(2));
    assert!(skein_lib::bytes::find(&bytes, b"[--parent TASK]").is_some());
    assert!(skein_lib::bytes::find(&bytes, b"Only this task's delegates").is_some());
}

fn tree_answer(folded: bool) -> crate::jig::Printed {
    use jig_client::{Latest, Watchable};
    use skein_lib::List;
    let (mut root, call) = fixtures::read_call(&["task", "show", "41", "--tree"]);
    let mut task = fixtures::goal(41, c::WaitsOn::Nothing);
    task.chip.title = fixtures::document_text(b"Root");
    let Latest::Jig(c::Delivery::OwnerTree(mut tree)) = fixtures::owner_tree_at(task.chip.clone()) else {
        panic!("owner tree")
    };
    tree.rows = List::with_capacity(2);
    for number in [42, 43] {
        tree.rows
            .push(c::OwnerTreeRow {
                chip: {
                    let mut chip = fixtures::goal(number, c::WaitsOn::Nothing).chip;
                    chip.title = fixtures::document_text(b"Delegate");
                    chip
                },
                parent: 41,
                depth: 1,
                proposals: List::with_capacity(0),
                spent: 0,
                dependencies: List::with_capacity(0),
                cut: false,
            })
            .expect("tree row");
    }
    tree.folded = List::with_capacity(2);
    // Independent parents ensure the summary adds every fold, not just the root's.
    for (parent, waiting, running) in [(41, 1, 1), (42, 0, 1)] {
        tree.folded
            .push(c::OwnerTreeFold {
                parent,
                phases: c::OwnerPhaseCounts {
                    waiting: if folded { waiting } else { 0 },
                    running: if folded { running } else { 0 },
                    ..c::OwnerPhaseCounts::ZERO
                },
                held_below: 0,
            })
            .expect("fold");
    }
    root.serve(call, fixtures::task_result(task));
    root.deliver_subject(
        &Watchable::Jig(c::Subject::Tree { task: 41, window: crate::limits::ROWS }),
        c::Delivery::OwnerTree(tree),
    );
    root.answer(call).expect("tree answer")
}

#[test]
fn a_tree_says_what_is_folded_and_how_to_read_on() {
    let answer = tree_answer(true);
    let start = skein_lib::bytes::find(&answer.stdout, b"tree:\n").expect("tree").checked_add(6).expect("tree start");
    let tree = answer.stdout.get(start..).expect("tree text");
    let end = skein_lib::bytes::find(tree, b"digest:").expect("digest");
    assert_eq!(tree.get(..end).expect("tree rows"), fixtures::golden("task_tree_cut").as_ref());
}

#[test]
fn a_tree_with_nothing_folded_has_no_cut_line() {
    assert!(skein_lib::bytes::find(&tree_answer(false).stdout, b"cut:").is_none());
}

#[test]
fn an_activity_page_cut_before_its_first_row_reads_on() {
    let c::ReadResult::Value(c::ReadValue::Activity(mut page)) = fixtures::activity_words() else { panic!("activity") };
    page.rows = skein_lib::List::with_capacity(0);
    page.cut = true;
    page.before = Some(fixtures::cursor());
    let printed =
        fixtures::read_answer(&["task", "activity", "41"], c::ReadResult::Value(c::ReadValue::Activity(page.clone())));
    assert_eq!(printed.stdout, fixtures::golden("activity_empty_cut"));
    let printed = fixtures::read_answer(
        &["task", "activity", "41", "--json"],
        c::ReadResult::Value(c::ReadValue::Activity(page)),
    );
    for field in
        [br#""cut":true"#.as_slice(), br#""before":{"value":"1122"}"#, br#""read_on":"task activity 41 --before 1122""#]
    {
        assert!(skein_lib::bytes::find(&printed.stdout, field).is_some());
    }
}

#[test]
fn a_cut_question_prints_its_marker_and_the_asking_tasks_activity() {
    use jig_client::contract as c;
    for json in [false, true] {
        let argv: &[&str] =
            if json { &["inbox", "--kind", "questions", "--json"] } else { &["inbox", "--kind", "questions"] };
        let (mut root, call) = fixtures::read_call(argv);
        root.serve(call, fixtures::inbox_rows(c::InboxReadGroup::Needs, 1, false));
        let task = fixtures::goal(fixtures::TASK, c::WaitsOn::Nothing);
        let mut words = fixtures::document_text(b"The plan continues");
        words.cut = true;
        root.serve(
            call,
            c::ReadResult::Value(c::ReadValue::Question(Box::new(c::Question {
                task: task.chip,
                message: 2,
                from: Some(c::OwnerRequester::Task(42)),
                words,
                since: skein_lib::Wall::EPOCH,
                acts: skein_lib::List::with_capacity(0),
                ending: None,
            }))),
        );
        let answer = root.answer(call).expect("question printed");
        assert_eq!(answer.status, 0);
        let printed = answer.stdout;
        if json {
            assert!(skein_lib::bytes::find(&printed, b"\"cut\":true").is_some());
            assert!(skein_lib::bytes::find(&printed, b"temper ctl DIR task activity 42").is_some());
        } else {
            assert!(
                skein_lib::bytes::find(
                    &printed,
                    b"The plan continues (cut)\nread whole: temper ctl DIR task activity 42\n"
                )
                .is_some()
            );
        }
    }
}
