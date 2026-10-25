use super::fixtures;
use jig_client::contract::*;
use skein_lib::{Env, List, Queue, Time, Token, Wall};
use skein_ui::{Builder, Role};
use temper_client_domain::ChatsFilter;
use temper_client_domain::{Domain, Request};
const TASK: u64 = 9;
const TEXT: u32 = 32;
const SPENT: u64 = 7;
const BELOW: u64 = 2;
const BUDGET: u64 = 100;
const RESERVED: u64 = 0;
const LEFT: u64 = 91;
const LABEL: u32 = 1;
const DELEGATES: u32 = 3;
const EMPTY: u32 = 0;
const ACTIVITY: u64 = 1;
pub(super) fn line() -> TaskLine {
    let budget = OwnerBudget {
        numbers: OwnerNumbers { budget: BUDGET, spent: SPENT, spent_below: BELOW, reserved: RESERVED },
        left: LEFT,
        funder: Funder::Task(TASK),
        deadline: None,
    };
    let details = OwnerChipDetails {
        dependencies: List::with_capacity(EMPTY),
        project: fixtures::PROJECT,
        executor: OwnerExecutor::Person(PersonAddress::Party(73)),
        phase_since: Wall::EPOCH,
        run_since: None,
        attempt: None,
        retry: None,
        wake: OwnerWakePolicy {
            words: WakeRule::Never,
            notices: WakeRule::Never,
            news: WakeRule::Never,
            results: ResultsWake::Never,
            questions: false,
            answers: false,
            timers: false,
        },
        person_wait: None,
        hold: None,
        budget: budget.clone(),
        activity: ACTIVITY,
        resources: List::with_capacity(EMPTY),
        resources_cut: false,
        resources_observed: false,
        procedure: None,
        live_delegates: DELEGATES,
        closing_connectors: None,
        ending: None,
        ended_at: None,
        observation: None,
    };
    TaskLine {
        chip: Chip {
            task: TASK,
            title: DocumentText::complete(Text::new(Box::from(*b"Hello"), TEXT).expect("title")),
            executor: Executor::Party { party: 73 },
            phase: Phase::Idle,
            details: Box::new(ChipPhase::Owner {
                phase: OwnerChipPhase::Active(RunStage::Idle),
                details: Box::new(details),
            }),
        },
        project: fixtures::PROJECT,
        requester: OwnerRequester::Person(73),
        tracked: None,
        label: LABEL,
        flags: TaskFlags { ended_by: EndedBy::Result, conversation: true, listed: false, retention: Retention::Kept },
        changed: Wall::EPOCH,
        last_activity: Wall::EPOCH,
        ended_at: None,
        budget,
        waits_on: WaitsOn::Nothing,
        phases: OwnerPhaseCounts { running: DELEGATES, ..OwnerPhaseCounts::ZERO },
        held_below: EMPTY,
        conversation: None,
    }
}
pub(super) fn tree(line: &TaskLine) -> (skein_ui::Tree, crate::frame::Bindings) {
    tree_at(line, fixtures::NOW)
}
fn tree_at(line: &TaskLine, now: Wall) -> (skein_ui::Tree, crate::frame::Bindings) {
    let mut b = Builder::new(&fixtures::tree_limits());
    let mut bindings = crate::frame::Bindings::new(fixtures::tree_limits().bindings);
    crate::pages::chats::row(
        &mut b,
        &fixtures::root_in_project(Some(fixtures::PROJECT)),
        line,
        &fixtures::words(),
        &mut bindings,
        &mut crate::words::Times::new(now),
    );
    (b.finish(), bindings)
}
#[test]
fn a_chats_row_says_its_status_from_jigs_facts() {
    let (tree, _) = tree(&line());
    assert!(fixtures::find(&tree, Role::Status, b"Parked").is_some());
}
pub(super) const HELD_STATUSES: [(HoldCategory, &[u8]); 3] = [
    (
        HoldCategory::Exceeded(Exceeded { bound: Bound::Reasoning, value: 2048 }),
        b"Held: the model's reasoning went past its configured allowance of 2048 bytes",
    ),
    (
        HoldCategory::Exceeded(Exceeded { bound: Bound::History, value: 2048 }),
        b"Held: the run's history no longer fits its configured allowance, even compacted",
    ),
    (HoldCategory::Failures { class: HoldFailure::Run }, b"Held"),
];

#[test]
fn a_held_chats_row_names_only_its_exceeded_bound() {
    for (reason, expected) in HELD_STATUSES {
        let mut line = line();
        let ChipPhase::Owner { phase, .. } = line.chip.details.as_mut() else { panic!("owner") };
        *phase = OwnerChipPhase::Held { reason, revision: 1 };
        let (tree, _) = tree(&line);
        let mut status = None;
        for node in tree.nodes() {
            if node.role == Role::Status && node.name.as_deref() == Some(expected) {
                status = Some(node);
                break;
            }
        }
        let status = status.expect("held status");
        assert_eq!(status.styles.as_slice(), [crate::look::STATUS_HELD]);
        assert!(skein_lib::bytes::find(&fixtures::text(&tree), expected).is_some());
    }
}

#[test]
fn a_chats_row_says_its_spend_and_delegates() {
    let (tree, _) = tree(&line());
    assert!(fixtures::find(&tree, Role::Paragraph, "7 spent".as_bytes()).is_some());
    assert!(skein_lib::bytes::find(&fixtures::text(&tree), "7 spent · 3 delegates".as_bytes()).is_some());
}
#[test]
fn a_chats_row_links_to_its_chat_by_number() {
    let (tree, bindings) = tree(&line());
    let mut bound = None;
    for node in tree.nodes() {
        if node.role == Role::Link && node.name.as_deref() == Some(b"T9") {
            bound = node.binding;
        }
    }
    let binding = bound.expect("bound link");
    assert_eq!(
        bindings.decode(binding.token, skein_ui::Act::Press),
        Some(temper_client_domain::Action::Pages(temper_client_domain::pages::Act::Go {
            address: temper_client_domain::Address::Chat { task: TASK }
        }))
    );
}
#[test]
fn the_chat_composer_waits_for_the_person_and_project() {
    let mut root = temper_client_domain::unlinked(&fixtures::domain_limits(), fixtures::SEED);
    fixtures::root_event(
        &mut root,
        temper_client_domain::Event::Went {
            place: temper_client_domain::Place::Address(temper_client_domain::Address::Chats {
                filter: ChatsFilter::All,
                start: false,
            }),
        },
    );
    let mut view = fixtures::view();
    assert_eq!(fixtures::find(fixtures::build(&mut view, &root), Role::TextArea, b"Start a chat"), None);
}

#[test]
fn enter_in_the_start_composer_starts_a_chat() {
    let mut view = fixtures::page_view(&fixtures::chats_root());
    let composer = fixtures::find(view.tree(), Role::TextArea, b"Start a chat").expect("composer");
    assert_eq!(
        crate::act(&mut view, composer, skein_ui::Act::Submit),
        Some(temper_client_domain::Action::Pages(temper_client_domain::pages::Act::StartChat)),
    );
}
#[test]
fn a_line_break_in_the_start_composer_is_kept() {
    let mut root = fixtures::chats_root();
    assert_eq!(&*fixtures::type_into(&mut root, b"Start a chat", b"Hello\nthere"), b"Hello\nthere");
}
#[test]
fn a_chats_row_carries_the_row_token() {
    let (tree, _) = tree(&line());
    assert_eq!(fixtures::styles(&tree, Role::Item).as_ref(), [Box::from([crate::look::ROW])]);
}

pub(super) fn status_rows() -> [(TaskLine, skein_ui::Style); 11] {
    use crate::look;
    let mut rows = [
        (line(), look::STATUS_YOURS),
        (line(), look::STATUS_HELD),
        (line(), look::STATUS_RETRYING),
        (line(), look::STATUS_ACTIVE),
        (line(), look::STATUS_ACTIVE),
        (line(), look::STATUS_ACTIVE),
        (line(), look::STATUS_ACTIVE),
        (line(), look::STATUS_WAITING),
        (line(), look::STATUS_QUIET),
        (line(), look::STATUS_QUIET),
        (line(), look::STATUS_QUIET),
    ];
    for (index, (line, _)) in rows.iter_mut().enumerate() {
        let ChipPhase::Owner { phase, details } = line.chip.details.as_mut() else { panic!("owner") };
        details.ended_at = Some(Wall::EPOCH);
        details.retry = Some(RetryNotice::OwnerFailed { class: HoldFailure::Lost, count: 2, allowance: 3 });
        *phase = match index {
            1 => OwnerChipPhase::Held { reason: HoldCategory::Stopped, revision: 1 },
            2 => OwnerChipPhase::Active(RunStage::BackingOff { until: Wall::EPOCH }),
            3 => OwnerChipPhase::Active(RunStage::Due),
            4..=6 => OwnerChipPhase::Active(RunStage::Running { attempt: 1 }),
            9 => OwnerChipPhase::Closing(ClosingStage::Delegates),
            10 => OwnerChipPhase::Ended,
            _ => OwnerChipPhase::Active(RunStage::Idle),
        };
        if index == 0 || index == 7 {
            line.waits_on = WaitsOn::Party {
                whom: Whom::Person(if index == 0 { 73 } else { 74 }),
                entries: 1,
                task: TASK,
                kind: WaitedFor::Question,
            };
        }
        if (4..=6).contains(&index) {
            details.observation = Some(OwnerChipObservation {
                channel: Token::new(1),
                work: match index {
                    4 => OwnerChipWork::Writing,
                    5 => OwnerChipWork::Tool { call: Box::from(*b"call"), tool: Box::from(*b"search") },
                    _ => OwnerChipWork::Active,
                },
                since: Wall::EPOCH,
                reported_at: Wall::EPOCH,
                started: None,
            });
        }
    }
    rows
}
#[test]
fn each_status_is_styled_by_its_meaning() {
    for (line, token) in status_rows() {
        let (tree, _) = tree(&line);
        assert_eq!(fixtures::styles(&tree, Role::Status).as_ref(), [Box::from([token])]);
    }
}
#[test]
fn the_entry_contains_the_composer_hint_and_start_in_order() {
    let view = fixtures::page_view(&fixtures::chats_root());
    let nodes = view.tree().nodes();
    let mut entry = None;
    for (index, node) in nodes.iter().enumerate() {
        if node.styles.as_slice().contains(&crate::look::ENTRY) {
            entry = Some(index);
        }
    }
    let entry = entry.expect("entry");
    let children = &nodes[entry + 1..entry + usize::try_from(nodes[entry].size).expect("size")];
    assert_eq!(nodes[entry].role, Role::Slot);
    assert_eq!(children[0].role, Role::TextArea);
    assert!(children[0].states.as_slice().contains(&skein_ui::State::SubmitsOnEnter));
    let mut hint = None;
    let mut start = None;
    for (index, node) in children.iter().enumerate() {
        if node.styles.as_slice().contains(&crate::look::HINT) {
            hint = Some(index);
        }
        if node.role == Role::Button && node.name.as_deref() == Some(b"Start chat") {
            start = Some(index);
        }
    }
    let hint = hint.expect("hint");
    let start = start.expect("start");
    assert!(hint < start);
    assert!(skein_lib::bytes::find(&fixtures::text(view.tree()), b"Enter to start").is_some());
}
#[test]
fn a_row_shows_the_real_activity_or_end_time() {
    let mut live = line();
    let now = Wall::from_nanos(720_000_000_000);
    let (shown, _) = tree_at(&live, now);
    assert!(skein_lib::bytes::find(&fixtures::text(&shown), b"12 min ago").is_some());
    live.ended_at = Some(Wall::from_nanos(360_000_000_000));
    let (shown, _) = tree_at(&live, now);
    assert!(skein_lib::bytes::find(&fixtures::text(&shown), b"6 min ago").is_some());
}

fn open_chats(root: &mut Domain, env: &Env<temper_client_domain::Limits>, out: &mut Queue<Request>) -> (Token, Token) {
    use temper_client_domain::{Address, Event};
    temper_client_domain::step(
        root,
        env,
        Event::Went {
            place: temper_client_domain::Place::Address(Address::Chats { filter: ChatsFilter::All, start: false }),
        },
        out,
    );
    let mut watch = None;
    let mut read = None;
    while let Some(request) = out.pop() {
        if let Request::Client {
            request:
                jig_client::Request::Down {
                    down:
                        jig_client::Down::Open { watch: token, subject: jig_client::Watchable::Jig(Subject::Tasks { .. }) },
                },
        } = request
        {
            watch = Some(token);
        } else if let Request::Client {
            request:
                jig_client::Request::Down {
                    down: jig_client::Down::Read { read: token, what: jig_client::Readable::Jig(Read::Tasks { .. }) },
                },
        } = request
        {
            read = Some(token);
        }
    }
    (watch.expect("live watch"), read.expect("closed read"))
}
fn deliver_live(
    root: &mut Domain,
    env: &Env<temper_client_domain::Limits>,
    out: &mut Queue<Request>,
    watch: Token,
    rows: List<TaskLine>,
) {
    use temper_client_domain::Event;
    temper_client_domain::step(
        root,
        env,
        Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched {
                    watch,
                    event: WatchEvent::Delivered(Delivery::Tasks(Box::new(Tasks { rows, more: false }))),
                },
            },
        },
        out,
    );
}

pub(super) fn listed_root() -> Domain {
    let mut ended = line();
    ended.chip.task = 8;
    ended.ended_at = Some(Wall::EPOCH);
    listed(&[line()], &[ended], false)
}
fn listed(live: &[TaskLine], closed: &[TaskLine], more: bool) -> Domain {
    use temper_client_domain::{Address, Event};
    let limits = fixtures::domain_limits();
    let (mut root, me_watch, _) =
        fixtures::root_at(Address::Inbox { filter: temper_client_domain::address::Filter::ALL }, &limits);
    let template = fixtures::chat::messaging_root();
    let me = template.pages().me(template.client()).expect("me").clone();
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    let mut out = Queue::with_capacity(temper_client_domain::max_out(&limits));
    temper_client_domain::step(
        &mut root,
        &env,
        Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched {
                    watch: me_watch,
                    event: WatchEvent::Delivered(Delivery::Me(Box::new(me))),
                },
            },
        },
        &mut out,
    );
    let (watch, read) = open_chats(&mut root, &env, &mut out);

    let mut live_rows = List::with_capacity(u32::try_from(live.len()).unwrap());
    for line in live {
        live_rows.push(line.clone()).expect("live row");
    }
    deliver_live(&mut root, &env, &mut out, watch, live_rows);
    let mut closed_rows = List::with_capacity(u32::try_from(closed.len()).unwrap());
    for line in closed {
        closed_rows.push(line.clone()).expect("closed row");
    }
    // A live update refreshes the first ended page; answer the latest read.
    let mut read = read;
    while let Some(request) = out.pop() {
        if let Request::Client {
            request:
                jig_client::Request::Down {
                    down: jig_client::Down::Read { read: token, what: jig_client::Readable::Jig(Read::Tasks { .. }) },
                },
        } = request
        {
            read = token;
        }
    }
    temper_client_domain::step(
        &mut root,
        &env,
        Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::ReadDone {
                    read,
                    result: jig_client::Readout::Jig(ReadResult::Value(ReadValue::Tasks(Box::new(TaskPage {
                        live: List::with_capacity(0),
                        live_after: None,
                        ended: EndedGroup::Rows {
                            rows: closed_rows,
                            after: if more { Some(Cursor::new(Box::from(*b"next"), 8).unwrap()) } else { None },
                            cut: false,
                        },
                    })))),
                },
            },
        },
        &mut out,
    );
    root
}
#[test]
fn chats_have_one_list_or_the_empty_sentence() {
    let empty = fixtures::page_view(&fixtures::chats_root());
    assert!(fixtures::find(empty.tree(), Role::Heading { level: 2 }, b"Your chats").is_some());
    assert!(fixtures::styles(empty.tree(), Role::List).is_empty());
    assert!(fixtures::find(empty.tree(), Role::Paragraph, b"No chats yet.").is_some());
    let view = fixtures::page_view(&listed_root());
    assert_eq!(fixtures::styles(view.tree(), Role::List).as_ref(), [Box::from([crate::look::ROWS])]);
    let list = fixtures::find(view.tree(), Role::List, b"Your chats").expect("named chats list");
    let nodes = view.tree().nodes();
    for (index, node) in nodes.iter().enumerate() {
        if node.id == list {
            assert_eq!(node.role, Role::List);
            let end = index.checked_add(usize::try_from(node.size).expect("list size")).expect("list end");
            let mut child = index + 1;
            for task in [8, 9] {
                assert_eq!(nodes[child].role, Role::Item);
                assert_eq!(nodes[child].key.expect("chat line key").a, task);
                child = child.checked_add(usize::try_from(nodes[child].size).expect("line size")).expect("next line");
            }
            assert_eq!(child, end, "both lines belong to the named list");
        }
    }
}

#[test]
fn an_empty_demo_start_has_its_invitation_and_your_chats() {
    let root = fixtures::demo_root(
        temper_client_domain::Address::Chats { filter: ChatsFilter::All, start: false },
        fixtures::PROJECT,
        true,
    );
    let view = fixtures::page_view(&root);
    assert!(fixtures::find(view.tree(), Role::Heading { level: 2 }, b"Live chats").is_none());
    assert!(fixtures::find(view.tree(), Role::Heading { level: 2 }, b"Closed chats").is_none());
    assert!(fixtures::find(view.tree(), Role::Heading { level: 2 }, b"Your chats").is_some());
    assert!(fixtures::find(view.tree(), Role::Paragraph, b"No chats yet.").is_some());
    let mut heading = None;
    for node in view.tree().nodes() {
        if node.role == (Role::Heading { level: 1 }) {
            assert!(heading.is_none(), "one level-1 heading");
            heading = node.name.as_deref();
        }
    }
    assert_eq!(heading, Some(b"What shall we change?".as_slice()));
}

#[test]
fn a_live_demo_chat_is_listed_under_your_chats() {
    use temper_client_domain::Address;
    let mut root = fixtures::demo_root(
        Address::Inbox { filter: temper_client_domain::address::Filter::ALL },
        fixtures::PROJECT,
        true,
    );
    let limits = fixtures::domain_limits();
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    let mut out = Queue::with_capacity(temper_client_domain::max_out(&limits));
    let (watch, _) = open_chats(&mut root, &env, &mut out);
    let mut rows = List::with_capacity(1);
    rows.push(line()).expect("live row");
    deliver_live(&mut root, &env, &mut out, watch, rows);
    let view = fixtures::page_view(&root);
    assert!(fixtures::find(view.tree(), Role::Heading { level: 2 }, b"Your chats").is_some());
    assert!(fixtures::find(view.tree(), Role::Link, b"T9").is_some());
    assert!(fixtures::find(view.tree(), Role::Paragraph, b"Hello").is_some());
    assert!(fixtures::find(view.tree(), Role::Heading { level: 2 }, b"Closed chats").is_none());
    assert!(fixtures::find(view.tree(), Role::Button, b"More closed chats").is_none());
}

#[test]
fn a_chats_line_counts_the_delegates_it_waits_on() {
    let mut line = line();
    line.waits_on = WaitsOn::Delegates;
    let (tree, _) = tree(&line);
    assert!(fixtures::find(&tree, Role::Status, b"waiting on 3 delegates").is_some());
    assert_eq!(fixtures::styles(&tree, Role::Status).as_ref(), [Box::from([crate::look::STATUS_WAITING])]);
}

#[test]
fn a_chats_line_reads_its_owner_wake_policy_for_news() {
    let mut line = line();
    line.waits_on = WaitsOn::News;
    let ChipPhase::Owner { details, .. } = line.chip.details.as_mut() else { panic!("owner") };
    details.wake.words = WakeRule::Immediate;
    details.live_delegates = 0;
    let (tree, _) = tree(&line);
    assert!(fixtures::find(&tree, Role::Status, b"waiting on your words").is_some());
}

#[test]
fn an_open_chat_at_rest_says_waiting_on_your_words_on_its_line() {
    let mut line = line();
    line.flags.ended_by = EndedBy::Requester;
    line.waits_on = WaitsOn::News;
    let ChipPhase::Owner { phase, details } = line.chip.details.as_mut() else { panic!("owner") };
    *phase = OwnerChipPhase::Active(RunStage::Idle);
    details.wake.words = WakeRule::Immediate;
    details.live_delegates = 0;
    let (tree, _) = tree(&line);
    assert!(fixtures::find(&tree, Role::Status, b"waiting on your words").is_some());
}
#[test]
fn a_closed_chat_says_closed_on_its_line() {
    for (by, expected) in [
        (Some(OwnerRequester::Person(73)), b"Closed".as_slice()),
        (Some(OwnerRequester::Person(74)), b"Closed"),
        (None, b"Closed"),
    ] {
        let mut line = line();
        let ChipPhase::Owner { phase, details } = line.chip.details.as_mut() else { panic!("owner") };
        *phase = OwnerChipPhase::Ended;
        details.ended_at = Some(Wall::EPOCH);
        details.ending = Some(match by {
            Some(by) => OwnerChipEnding::Cancelled {
                by,
                reason: DocumentText::complete(fixtures::first_run::text(b"Chat closed")),
                result: None,
                at: Wall::EPOCH,
            },
            None => OwnerChipEnding::Done(ResultValue::Report(DocumentText::complete(fixtures::first_run::text(
                b"A report",
            )))),
        });
        let (tree, _) = tree(&line);
        assert!(fixtures::find(&tree, Role::Status, expected).is_some());
        assert_eq!(fixtures::styles(&tree, Role::Status).as_ref(), [Box::from([crate::look::STATUS_QUIET])]);
    }
}

fn filter(root: &mut Domain, filter: ChatsFilter) {
    fixtures::root_event(
        root,
        temper_client_domain::Event::Went {
            place: temper_client_domain::Place::Address(temper_client_domain::Address::Chats { filter, start: false }),
        },
    );
}

fn numbers(tree: &skein_ui::Tree) -> Box<[u64]> {
    let mut numbers = List::with_capacity(u32::try_from(tree.nodes().len()).expect("tree nodes fit"));
    for node in tree.nodes() {
        if node.role == Role::Item
            && let Some(key) = node.key
        {
            numbers.push(key.a).expect("one key per node");
        }
    }
    numbers.into_boxed()
}
fn timed(task: u64, activity: u64, ended: Option<u64>) -> TaskLine {
    let mut line = line();
    line.chip.task = task;
    line.last_activity = Wall::from_nanos(activity);
    if let Some(ended) = ended {
        line.ended_at = Some(Wall::from_nanos(ended));
    }
    line
}
#[test]
fn one_list_merges_activity_and_end_times_with_lower_numbers_first_on_a_tie() {
    // Closed T8's activity is newer than everything, but its end is the tie.
    let root =
        listed(&[timed(9, 90, None), timed(6, 40, None)], &[timed(8, 200, Some(40)), timed(7, 0, Some(100))], false);
    let view = fixtures::page_view(&root);
    assert_eq!(numbers(view.tree()).as_ref(), [7, 9, 6, 8]);
    assert_eq!(fixtures::styles(view.tree(), Role::List).as_ref(), [Box::from([crate::look::ROWS])]);
}
#[test]
fn each_filter_selects_its_lines_and_presses_its_bound_button() {
    use temper_client_domain::{Action, address::ChatsFilter as Filter, pages::Act};
    for (selected, name, expected) in [
        (Filter::All, b"All".as_slice(), [8, 9].as_slice()),
        (Filter::Live, b"Live", [9].as_slice()),
        (Filter::Closed, b"Closed", [8].as_slice()),
    ] {
        let mut root = listed_root();
        filter(&mut root, selected);
        let mut view = fixtures::page_view(&root);
        assert_eq!(numbers(view.tree()).as_ref(), expected);
        for (filter, label) in [(Filter::All, b"All".as_slice()), (Filter::Live, b"Live"), (Filter::Closed, b"Closed")]
        {
            let id = fixtures::find(view.tree(), Role::Button, label).expect("filter button");
            for node in view.tree().nodes() {
                if node.id == id {
                    assert_eq!(node.states.as_slice().contains(&skein_ui::State::Pressed), label == name);
                }
            }
            assert_eq!(
                crate::act(&mut view, id, skein_ui::Act::Press),
                Some(Action::Pages(Act::ChatFilter { filter }))
            );
        }
        for task in expected {
            let label = match *task {
                8 => b"T8",
                9 => b"T9",
                _ => panic!("fixture task"),
            };
            let id = fixtures::find(view.tree(), Role::Link, label).expect("TN link");
            assert_eq!(
                crate::act(&mut view, id, skein_ui::Act::Press),
                Some(Action::Pages(Act::Go { address: temper_client_domain::Address::Chat { task: *task } }))
            );
        }
    }
}
#[test]
fn empty_filters_say_which_chats_are_missing() {
    use temper_client_domain::address::ChatsFilter as Filter;
    for (selected, words) in [
        (Filter::All, b"No chats yet.".as_slice()),
        (Filter::Live, b"No live chats."),
        (Filter::Closed, b"No closed chats."),
    ] {
        let mut root = listed(&[], &[], false);
        filter(&mut root, selected);
        let view = fixtures::page_view(&root);
        assert!(fixtures::find(view.tree(), Role::Paragraph, words).is_some());
        assert!(fixtures::styles(view.tree(), Role::List).is_empty());
    }
    for (selected, live, ended, words) in
        [(Filter::Live, false, true, b"No live chats.".as_slice()), (Filter::Closed, true, false, b"No closed chats.")]
    {
        let active = [line()];
        let closed = [timed(8, 0, Some(0))];
        let mut root = listed(if live { &active } else { &[] }, if ended { &closed } else { &[] }, false);
        filter(&mut root, selected);
        assert!(fixtures::find(fixtures::page_view(&root).tree(), Role::Paragraph, words).is_some());
    }
}
#[test]
fn more_closed_chats_is_bound_only_under_all_or_closed_with_an_older_page() {
    use temper_client_domain::{Action, address::ChatsFilter as Filter, pages::Act};
    for more in [false, true] {
        for selected in [Filter::All, Filter::Live, Filter::Closed] {
            let mut root = listed(&[line()], &[timed(8, 0, Some(0))], more);
            filter(&mut root, selected);
            let mut view = fixtures::page_view(&root);
            let button = fixtures::find(view.tree(), Role::Button, b"More closed chats");
            assert_eq!(button.is_some(), more && selected != Filter::Live);
            if let Some(id) = button {
                assert_eq!(crate::act(&mut view, id, skein_ui::Act::Press), Some(Action::Pages(Act::ClosedChats)));
            }
        }
    }
}
#[test]
fn delegate_counts_are_plural_and_absent_at_zero() {
    for (count, expected) in [(0, None), (1, Some(b"1 delegate".as_slice())), (2, Some(b"2 delegates".as_slice()))] {
        let mut line = line();
        let ChipPhase::Owner { details, .. } = line.chip.details.as_mut() else { panic!("owner") };
        details.live_delegates = count;
        let (tree, _) = tree(&line);
        if let Some(words) = expected {
            assert!(skein_lib::bytes::find(&fixtures::text(&tree), words).is_some());
        } else {
            let mut paragraphs = 0_u32;
            for node in tree.nodes() {
                if node.role == Role::Paragraph {
                    paragraphs += 1;
                }
            }
            assert_eq!(paragraphs, 4, "project/number, title, spend and time");
        }
    }
}

#[test]
fn start_a_chat_heads_the_composer_except_at_the_demos_start() {
    for demo in [false, true] {
        let root = if demo {
            fixtures::demo_root(
                temper_client_domain::Address::Chats { filter: ChatsFilter::All, start: false },
                fixtures::PROJECT,
                true,
            )
        } else {
            // A template makes the ordinary chats composer available.
            super::choice::ordinary_chats()
        };
        let view = fixtures::page_view(&root);
        let nodes = view.tree().nodes();
        let (mut heading, mut slot, mut hint) = (None, None, None);
        for (at, node) in nodes.iter().enumerate() {
            if node.role == (Role::Heading { level: 2 }) && node.name.as_deref() == Some(b"Start a chat") {
                heading = Some(at);
            }
            if node.styles.as_slice().contains(&crate::look::CHOICE) {
                slot = Some(at);
            }
            if node.styles.as_slice().contains(&crate::look::HINT) {
                hint = Some(at);
            }
        }
        assert_eq!(heading.is_some(), !demo);
        if let Some(heading) = heading {
            assert!(heading < slot.unwrap());
        }
        let hint = hint.unwrap();
        assert_eq!(
            fixtures::node_text(view.tree(), hint).as_ref(),
            "temper · Enter to start · Shift+Enter for newline".as_bytes()
        );
    }
}

#[test]
fn an_accepted_start_draws_no_applied_status() {
    use temper_client_domain::{self as d, Action, Event, pages::Act};
    let mut root =
        fixtures::demo_root(d::Address::Chats { filter: ChatsFilter::All, start: false }, fixtures::PROJECT, true);
    let limits = fixtures::domain_limits();
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    let mut out = Queue::with_capacity(d::max_out(&limits));
    for act in [Act::ChatWords { words: Box::from(b"Hello".as_slice()) }, Act::StartChat] {
        d::step(&mut root, &env, Event::Act { action: Action::Pages(act) }, &mut out);
    }
    assert!(root.pages().chats().unwrap().start.is_some());
    d::step(
        &mut root,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
        Event::Client { event: jig_client::Event::Stored },
        &mut out,
    );
    let mut attempt = None;
    for request in &out {
        if let Request::Client {
            request: jig_client::Request::Down { down: jig_client::Down::Send { attempt: token, .. } },
        } = request
        {
            attempt = Some(*token);
        }
    }
    let attempt = attempt.unwrap();
    fixtures::root_event(
        &mut root,
        Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Answer {
                    attempt,
                    envelope: skein_lib::RequestEnvelope::Final,
                    answer: Some(jig_client::Answered::Jig(Answer::Done(Outcome::Owner(OwnerOutcome::Created {
                        first: 1,
                        last: 1,
                        given: None,
                    })))),
                    why: None,
                },
            },
        },
    );
    assert!(root.pages().chats().unwrap().outcome.is_some());
    let view = fixtures::page_view(&root);
    assert!(fixtures::find(view.tree(), Role::Status, b"Applied").is_none());
}

fn index_of(nodes: &[skein_ui::Node], id: skein_ui::NodeId) -> usize {
    let mut at = None;
    for (index, node) in nodes.iter().enumerate() {
        if node.id == id {
            at = Some(index);
        }
    }
    at.expect("node in the tree")
}
fn main_of(nodes: &[skein_ui::Node]) -> usize {
    let mut main = None;
    for (index, node) in nodes.iter().enumerate() {
        if node.role == Role::Main {
            main = Some(index);
        }
    }
    main.expect("a main")
}
#[test]
fn the_chats_page_opens_with_its_bar_and_draws_its_list_as_the_mockup() {
    let chats = fixtures::page_view(&fixtures::chats_root());
    let nodes = chats.tree().nodes();
    let main = main_of(nodes);
    assert!(nodes[main].styles.as_slice().contains(&crate::look::PAGE));
    let bar = main + 1;
    assert_eq!(nodes[bar].styles.as_slice(), [crate::look::PAGEBAR]);
    assert_eq!(nodes[bar + 1].role, Role::Heading { level: 1 });
    assert_eq!(nodes[bar + 1].name.as_deref(), Some(b"Chats".as_slice()));
    let after = bar + usize::try_from(nodes[bar].size).unwrap();
    assert_eq!(nodes[after].role, Role::Heading { level: 2 });
    assert_eq!(nodes[after].name.as_deref(), Some(b"Start a chat".as_slice()));
    let demo = fixtures::page_view(&fixtures::demo_root(
        temper_client_domain::Address::Chats { filter: ChatsFilter::All, start: false },
        fixtures::PROJECT,
        true,
    ));
    let demo_nodes = demo.tree().nodes();
    assert!(!demo_nodes[main_of(demo_nodes)].styles.as_slice().contains(&crate::look::PAGE));
    for node in demo_nodes {
        assert!(!node.styles.as_slice().contains(&crate::look::PAGEBAR), "the demo's start keeps no bar");
    }
    for (page, view) in [("chats", &chats), ("demo", &demo)] {
        let nodes = view.tree().nodes();
        let heading = index_of(nodes, fixtures::find(view.tree(), Role::Heading { level: 2 }, b"Your chats").unwrap());
        let row = heading - 1;
        assert_eq!(nodes[row].role, Role::Slot, "{page}");
        assert_eq!(nodes[row].styles.as_slice(), [crate::look::HEADING], "{page}");
        let end = row + usize::try_from(nodes[row].size).unwrap();
        for name in [b"All".as_slice(), b"Live", b"Closed"] {
            let button = index_of(nodes, fixtures::find(view.tree(), Role::Button, name).unwrap());
            assert!(row < button && button < end, "{page}: {} in the heading row", name.escape_ascii());
        }
        let empty = index_of(nodes, fixtures::find(view.tree(), Role::Paragraph, b"No chats yet.").unwrap());
        assert_eq!(empty, end, "{page}: the empty words follow the heading row");
        assert_eq!(nodes[empty].styles.as_slice(), [crate::look::EMPTY], "{page}");
    }
}

#[test]
fn done_and_failed_chats_say_closed_in_the_list_and_bar() {
    for ending in [
        OwnerChipEnding::Done(ResultValue::Report(DocumentText::complete(fixtures::first_run::text(b"Done")))),
        OwnerChipEnding::Failed(DocumentText::complete(fixtures::first_run::text(b"Failed"))),
    ] {
        let mut line = line();
        let ChipPhase::Owner { phase, details } = line.chip.details.as_mut() else { panic!("owner") };
        *phase = OwnerChipPhase::Ended;
        details.live_delegates = 0;
        details.ended_at = Some(Wall::EPOCH);
        details.ending = Some(ending);
        let (tree, _) = tree(&line);
        assert!(fixtures::find(&tree, Role::Status, b"Closed").is_some());
        let root = fixtures::root_in_project(None);
        let me = root.pages().me(root.client()).unwrap();
        let mut bar = Builder::new(&fixtures::tree_limits());
        crate::pages::chats::status(
            &mut bar,
            &line.chip,
            (&line.waits_on, Some(0), None),
            (line.project, line.requester),
            None,
            me,
            &fixtures::words(),
        );
        assert!(fixtures::find(&bar.finish(), Role::Status, b"Closed").is_some());
    }
}

#[test]
fn only_the_starts_address_focuses_its_composer_including_the_demo() {
    for demo in [false, true] {
        for start in [false, true] {
            let address = temper_client_domain::Address::Chats { filter: ChatsFilter::All, start };
            let root = if demo {
                fixtures::demo_root(address, fixtures::PROJECT, true)
            } else {
                fixtures::root_at(address, &fixtures::domain_limits()).0
            };
            let view = fixtures::page_view(&root);
            let mut focused = 0_usize;
            for node in view.tree().nodes() {
                if node.focus.is_some() {
                    assert_eq!(node.role, Role::TextArea);
                    assert_eq!(node.name.as_deref(), Some(b"Start a chat".as_slice()));
                    focused = focused.checked_add(1).expect("focus requests fit");
                }
            }
            assert_eq!(focused, usize::from(start), "demo={demo}, start={start}");
        }
    }
}

#[test]
fn unavailable_models_leave_an_editable_draft_but_no_submission() {
    let root = super::choice::no_model_root(false);
    let mut view = fixtures::page_view(&root);
    assert!(
        skein_lib::bytes::find(&fixtures::text(view.tree()), "No model yet · Connect a model".as_bytes()).is_some()
    );
    assert!(fixtures::find(view.tree(), Role::Disclosure, b"Change model").is_none());
    let connect = fixtures::find(view.tree(), Role::Link, b"Connect a model").expect("connect link");
    assert_eq!(
        crate::act(&mut view, connect, skein_ui::Act::Press),
        Some(temper_client_domain::Action::Pages(temper_client_domain::pages::Act::Go {
            address: temper_client_domain::Address::ConnectModel { way: None }
        }))
    );
    for (role, name, event) in [
        (Role::Button, b"Start chat".as_slice(), skein_ui::Act::Press),
        (Role::TextArea, b"Start a chat".as_slice(), skein_ui::Act::Submit),
    ] {
        let node = fixtures::find(view.tree(), role, name).expect("composer control");
        assert_eq!(crate::act(&mut view, node, event), None);
    }
    let field = fixtures::find(view.tree(), Role::TextArea, b"Start a chat").expect("draft field");
    assert!(
        crate::act(&mut view, field, skein_ui::Act::Input { value: Box::from(b"still editable".as_slice()) }).is_some()
    );
}
#[test]
fn rows_name_their_project_and_only_kept_ended_transcripts() {
    for ended in [false, true] {
        for kept in [false, true] {
            let mut line = line();
            if ended {
                line.ended_at = Some(Wall::EPOCH);
            }
            line.flags.retention = if kept { Retention::Kept } else { Retention::ForgottenOnceRead };
            let (tree, _) = tree(&line);
            assert!(skein_lib::bytes::find(&fixtures::text(&tree), "Shop · T9".as_bytes()).is_some());
            assert_eq!(
                fixtures::find(&tree, Role::Paragraph, b"Closed. Its transcript is kept.").is_some(),
                ended && kept
            );
        }
    }
}

#[test]
fn a_kept_ended_row_without_a_clock_is_closed_even_with_stale_wait_metadata() {
    let mut line = line();
    line.chip.phase = Phase::Ended;
    line.waits_on = WaitsOn::Delegates;
    let (tree, _) = tree(&line);
    assert!(fixtures::find(&tree, Role::Status, b"Closed").is_some());
    assert!(fixtures::find(&tree, Role::Paragraph, b"Closed. Its transcript is kept.").is_some());
    assert!(skein_lib::bytes::find(&fixtures::text(&tree), b"3 delegates").is_none());
}
