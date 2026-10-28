use super::fixtures;
use crate::look;
use jig_client::contract as c;
use skein_fake_person::{Face, Find, Found, Target, TreeFace};
use skein_lib::{Env, List, Queue, Time, Wall};
use skein_ui::Role;
use temper_client_domain::{Address, Domain, Event, Request};
use temper_client_domain::{ChatsFilter, PlanFilter};
const CHANGE: u64 = 2;
const CHAT: u64 = 1;
const EXCHANGES: u32 = 64;
const STEPS: u32 = 4;
const TITLE: &[u8] = b"Export widget's table as CSV";
const MERGE: &[u8] = b"1a2b3c4abcdef0123456789abcdef0123456789abcd";
const WAKE_SUMMARY: &[u8] =
    b"Begin the work your brief describes.\n\n2: Result from task 2:\n1d0b14e4db018fed000000000000000300000000";
const LATER: &[u8] = b"The change is running.";
fn text(bytes: &[u8]) -> c::Text {
    c::Text::new(Box::from(bytes), 256).expect("fixture text")
}
fn document(bytes: &[u8]) -> c::DocumentText {
    c::DocumentText::complete(text(bytes))
}
fn change() -> c::Task {
    let mut task = fixtures::task::document();
    task.chip.task = CHANGE;
    task.chip.title = document(TITLE);
    task.chip.phase = c::Phase::Running;
    task.hold = None;
    task.result = None;
    task.result_contract = c::TaskContractDocument::Change { connector: fixtures::FORGE, kind: 1, words: 256 };
    let c::ChipPhase::Owner { details, .. } = task.chip.details.as_mut() else { panic!("owner chip") };
    details.ending = None;
    task
}
fn landed() -> c::Task {
    let mut task = change();
    task.requester = c::OwnerRequester::Task(CHAT);
    task.result = Some(c::TaskResult::Ending(Box::new(c::OwnerChipEnding::Done(c::ResultValue::Change {
        kind: 1,
        part: c::OwnerPart {
            connector: fixtures::FORGE,
            kind: c::OwnerPartKind::Result,
            key: c::OwnerPartKey::Number(CHANGE),
        },
        words: document(MERGE),
    }))));
    task
}
fn limits() -> temper_client_domain::Limits {
    let mut limits = fixtures::domain_limits();
    limits.client.domain.rows = 8;
    limits.client.domain.text = 256;
    limits.client.domain.turn_text = 256;
    limits
}
fn up(root: &mut Domain, up: jig_client::Up, receiving: &temper_client_domain::Limits, out: &mut Queue<Request>) {
    temper_client_domain::step(
        root,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: *receiving },
        Event::Client { event: jig_client::Event::Up { up } },
        out,
    );
}

fn task_delivery(watch: skein_lib::Token, task: c::Task) -> jig_client::Up {
    jig_client::Up::Watched { watch, event: c::WatchEvent::Delivered(c::Delivery::Task(Box::new(task))) }
}

fn root(documents: &[c::Task], called: &[u64], heard: bool) -> Domain {
    root_activity(documents, activity(called, heard), &[])
}
fn root_activity(documents: &[c::Task], activity: c::ActivityPage, delegates: &[u64]) -> Domain {
    root_chat(None, documents, activity, delegates)
}
fn root_chat(
    chat_document: Option<&c::Task>,
    documents: &[c::Task],
    activity: c::ActivityPage,
    delegates: &[u64],
) -> Domain {
    root_chat_limits(chat_document, documents, activity, delegates, &limits())
}
fn root_chat_limits(
    chat_document: Option<&c::Task>,
    documents: &[c::Task],
    activity: c::ActivityPage,
    delegates: &[u64],
    receiving: &temper_client_domain::Limits,
) -> Domain {
    root_chat_with_reads(chat_document, documents, activity, delegates, receiving).0
}
fn root_chat_with_reads(
    chat_document: Option<&c::Task>,
    documents: &[c::Task],
    activity: c::ActivityPage,
    delegates: &[u64],
    receiving: &temper_client_domain::Limits,
) -> (Domain, Queue<Request>) {
    let mut reads = Queue::uncapped();
    let chat_number = match chat_document {
        Some(chat) => chat.chip.task,
        None => CHAT,
    };
    let (mut root, _, _) = fixtures::root_at(Address::Chats { filter: ChatsFilter::All, start: false }, receiving);
    let mut out = Queue::uncapped();
    temper_client_domain::step(
        &mut root,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits: *receiving },
        Event::Went { place: temper_client_domain::Place::Address(Address::Chat { task: chat_number }) },
        &mut out,
    );
    for _ in 0..EXCHANGES {
        let Some(request) = out.pop() else { break };
        match request {
            Request::Client {
                request:
                    jig_client::Request::Down {
                        down:
                            jig_client::Down::Read {
                                read,
                                what: jig_client::Readable::Jig(c::Read::Activity { before, .. }),
                            },
                    },
            } => {
                let page = fixtures::first_run::page(&activity, before.as_ref(), receiving.client.domain.rows);
                up(
                    &mut root,
                    jig_client::Up::ReadDone {
                        read,
                        result: jig_client::Readout::Jig(c::ReadResult::Value(c::ReadValue::Activity(Box::new(page)))),
                    },
                    receiving,
                    &mut out,
                );
            }
            Request::Client {
                request:
                    jig_client::Request::Down {
                        down:
                            jig_client::Down::Open {
                                watch,
                                subject: jig_client::Watchable::Jig(c::Subject::Task { task: number }),
                            },
                    },
            } if number == chat_number => {
                let mut chat = match chat_document {
                    Some(chat) => chat.clone(),
                    None => change(),
                };
                chat.chip.task = chat_number;
                chat.delegates = List::with_capacity(u32::try_from(delegates.len()).expect("delegates fit"));
                for task in delegates {
                    let mut chip = change().chip;
                    chip.task = *task;
                    chat.delegates.push(c::LiveDelegate { chip, takeover_budget: 0 }).expect("delegate");
                }
                chat.activity = activity.rows.get(0).expect("newest activity").position;
                up(&mut root, task_delivery(watch, chat), receiving, &mut out);
            }
            Request::Client {
                request:
                    jig_client::Request::Down {
                        down:
                            jig_client::Down::Open {
                                watch,
                                subject: jig_client::Watchable::Jig(c::Subject::Task { task: number }),
                            },
                    },
            } => {
                for task in documents {
                    if task.chip.task != number {
                        continue;
                    }
                    up(&mut root, task_delivery(watch, task.clone()), receiving, &mut out);
                }
            }
            request @ Request::Client {
                request:
                    jig_client::Request::Down {
                        down: jig_client::Down::Read { what: jig_client::Readable::Jig(c::Read::Task { .. }), .. },
                    },
            } => reads.push(request),
            Request::Client { .. }
            | Request::Show { .. }
            | Request::Leave { .. }
            | Request::SignOut
            | Request::Dropped { .. }
            | Request::Fact { .. } => {}
        }
    }
    assert!(out.is_empty(), "delegation fixture settles");
    (root, reads)
}

fn activity(called: &[u64], heard: bool) -> c::ActivityPage {
    let mut objects = List::with_capacity(u32::try_from(called.len()).expect("tasks fit"));
    for task in called {
        objects.push(c::ObjectKey::Task(*task)).expect("task");
    }
    let mut blocks = List::with_capacity(3);
    blocks.push(c::Block::Text(text(b"I'll delegate this change in playground/widget."))).expect("agent words");
    blocks
        .push(c::Block::Call(c::Call {
            number: 7,
            tool: text(b"delegate"),
            input: text(b"{}"),
            outcome: Some(text(b"Delegated tasks: 2")),
            failed: false,
            objects,
            parts: List::with_capacity(0),
        }))
        .expect("call");
    blocks.push(c::Block::Result { call: 7, summary: text(b"Delegated tasks: 2"), failed: false }).expect("result");
    let mut messages = List::with_capacity(1);
    messages.push(c::Message { speaker: c::Speaker::Assistant, blocks }).expect("message");
    let mut rows = List::with_capacity(4);
    if heard {
        for (position, from, words) in [(5, CHANGE, b"Carded result".as_slice()), (4, 3, b"Another task's result")] {
            rows.push(c::ActivityLine {
                position,
                at: Wall::EPOCH,
                event: c::Activity::Owner {
                    source: Box::new(c::OwnerActivityLine {
                        task: CHAT,
                        position,
                        at: Wall::EPOCH,
                        line: c::OwnerActivity::Heard(c::OwnerWord {
                            number: position,
                            from: c::OwnerRequester::Task(from),
                            kind: c::OwnerMessageKind::Result(c::OwnerWordResult::Report),
                            words: text(words),
                            at: Wall::EPOCH,
                            hits: 0,
                            eligible: false,
                        }),
                    }),
                    decoded: None,
                },
            })
            .expect("heard line");
        }
        let task = change();
        rows.push(c::ActivityLine {
            position: 3,
            at: Wall::EPOCH,
            event: c::Activity::Result {
                result: c::Result {
                    task: task.chip,
                    ending: c::Ending::Done,
                    value: Some(c::ResultValue::Report(document(b"Duplicate result"))),
                    at: Some(Wall::EPOCH),
                    details: None,
                    acts: List::with_capacity(0),
                },
            },
        })
        .expect("result line");
    }
    rows.push(c::ActivityLine {
        position: 2,
        at: Wall::EPOCH,
        event: c::Activity::Turn {
            name: c::TurnName { attempt: 1, number: 1 },
            turn: c::TurnRead::Decoded(c::Turn {
                messages,
                used: c::Usage { input: 0, output: 0, cached: 0, cache_written: 0 },
                spent: 0,
                read_through: None,
                cut: false,
            }),
        },
    })
    .expect("turn");
    c::ActivityPage { rows, before: None, after: None, cut: false }
}
fn view(task: Option<c::Task>, called: &[u64], heard: bool) -> crate::View {
    view_documents(task.as_slice(), called, heard)
}
fn view_documents(documents: &[c::Task], called: &[u64], heard: bool) -> crate::View {
    let root = root(documents, called, heard);
    view_root(&root)
}
fn view_root(root: &Domain) -> crate::View {
    let mut view = crate::View::new(
        &crate::Limits { tree: fixtures::tree_limits() },
        &limits(),
        fixtures::words(),
        fixtures::FORGE,
    );
    fixtures::build(&mut view, root);
    view
}
fn find(view: &crate::View, target: Find) -> Option<skein_ui::NodeId> {
    let find = target.within(Box::new([
        Target::Role { role: Role::Log, name: Box::from(b"Conversation".as_slice()) },
        Target::Role { role: Role::Region, name: Box::from(b"Task 2".as_slice()) },
    ]));
    match TreeFace::new(view.tree()).find(&find) {
        Found::Node(id) => Some(id),
        Found::Absent | Found::Pending => None,
    }
}
fn status(view: &crate::View, name: &[u8], token: skein_ui::Style) {
    let id = find(view, Find::exactly_named(Role::Status, name)).expect("card status");
    assert_eq!(view.tree().find(id).expect("status").styles.as_slice(), [token]);
}
fn steps(view: &crate::View, done: bool) {
    let id = find(view, Find::exactly_named(Role::List, b"Steps")).expect("steps");
    let mut face = TreeFace::subtree(view.tree(), id).expect("list");
    let mut count = 0_u32;
    for node in view.tree().nodes() {
        if node.styles.as_slice().contains(&look::STEP) {
            count = count.checked_add(1).expect("step count");
            assert_eq!(node.styles.as_slice().contains(&look::STEP_DONE), done);
        }
    }
    assert_eq!(count, STEPS);
    for name in [b"Branch pushed".as_slice(), b"Pull request opened", b"CI passed", b"Landed"] {
        if let Found::Node(_) = face.find(&Find::exactly_named(Role::Item, name)) {
        } else {
            panic!("step absent");
        }
    }
}
fn after_call() -> c::ActivityPage {
    let mut rows = List::with_capacity(1);
    rows.push(c::ActivityLine {
        position: 3,
        at: Wall::EPOCH,
        event: c::Activity::News { topic: 0, summary: text(LATER), part: None },
    })
    .expect("later activity");
    c::ActivityPage { rows, before: None, after: None, cut: false }
}
fn last_turn(view: &crate::View) -> &[skein_ui::Node] {
    let nodes = view.tree().nodes();
    for (index, node) in nodes.iter().enumerate() {
        if node.role != Role::Log {
            continue;
        }
        let end = index.checked_add(usize::try_from(node.size).expect("log size")).expect("log end");
        let mut child = index.checked_add(1).expect("first child");
        let mut last = child;
        while child < end {
            assert_eq!(nodes[child].role, Role::Region);
            last = child;
            child = child.checked_add(usize::try_from(nodes[child].size).expect("turn size")).expect("next turn");
        }
        return &nodes[last..end];
    }
    panic!("conversation log absent");
}
fn change_cards(view: &crate::View) -> u32 {
    let mut cards = 0_u32;
    for node in view.tree().nodes() {
        if node.role == Role::Region && node.name.as_deref() == Some(b"Task 2") {
            cards = cards.checked_add(1).expect("card count");
        }
    }
    cards
}
#[test]
fn a_running_change_shows_its_card_after_the_conversation_once_its_call_has_left() {
    let view = view_root(&root_activity(&[change()], after_call(), &[CHANGE]));
    assert_eq!(change_cards(&view), 1);
    let region = fixtures::find(view.tree(), Role::Region, b"Task 2").expect("task card");
    assert!(find(&view, Find::text("T2 · CHANGE".as_bytes())).is_some());
    status(&view, b"Running", look::STATUS_ACTIVE);
    let last = last_turn(&view);
    assert_eq!(last[0].styles.as_slice(), [look::TURN]);
    assert_eq!(last[1].id, region, "the card follows the conversation's last line");
}
#[test]
fn a_live_delegate_carded_at_its_call_shows_once() {
    let view = view_root(&root_activity(&[change()], activity(&[CHANGE], false), &[CHANGE]));
    assert_eq!(change_cards(&view), 1);
}
#[test]
fn a_live_delegate_without_its_document_shows_nothing_after_the_conversation() {
    let view = view_root(&root_activity(&[], after_call(), &[CHANGE]));
    assert_eq!(change_cards(&view), 0);
    let last = last_turn(&view);
    assert_eq!(last[1].role, Role::Paragraph);
    assert_eq!(last[1].name.as_deref(), Some(LATER), "the last region is still the conversation's line");
}
#[test]
fn a_running_change_shows_its_card_at_its_call() {
    let mut view = view(Some(change()), &[CHANGE], false);
    let region = fixtures::find(view.tree(), Role::Region, b"Task 2").expect("task card");
    assert_eq!(view.tree().find(region).expect("card").styles.as_slice(), [look::CARD, look::TASK_CARD]);
    assert!(find(&view, Find::text("T2 · CHANGE".as_bytes())).is_some());
    status(&view, b"Running", look::STATUS_ACTIVE);
    assert!(find(&view, Find::exactly_named(Role::Heading { level: 3 }, TITLE)).is_some());
    steps(&view, false);
    let link = find(&view, Find::exactly_named(Role::Link, b"Task 2")).expect("task link");
    let mut link_text = None;
    for (index, node) in view.tree().nodes().iter().enumerate() {
        if node.id == link {
            link_text = Some(fixtures::node_text(view.tree(), index));
            break;
        }
    }
    assert_eq!(link_text.as_deref(), Some("Task 2 ↗".as_bytes()));
    assert_eq!(
        crate::act(&mut view, link, skein_ui::Act::Press),
        Some(temper_client_domain::Action::Pages(temper_client_domain::pages::Act::Go {
            address: Address::Task { task: CHANGE, filter: PlanFilter::All }
        }))
    );
    assert_eq!(work_slots(&view), 0);
    assert!(skein_lib::bytes::find(&fixtures::text(view.tree()), b"Delegated tasks: 2").is_none());
    let said = fixtures::text(view.tree());
    assert!(
        skein_lib::bytes::find(&said, b"I'll delegate").expect("agent")
            < skein_lib::bytes::find(&said, "T2 · CHANGE".as_bytes()).expect("card")
    );
}
#[test]
fn a_landed_change_ticks_its_steps_and_says_its_merge_commit() {
    let view = view(Some(landed()), &[CHANGE], false);
    status(&view, b"Landed", look::STATUS_DONE);
    assert!(find(&view, Find::text("CI passed. Landed · merge commit 1a2b3c4".as_bytes())).is_some());
    steps(&view, true);
}
fn completed_steps(task: c::Task) -> u32 {
    let view = view(Some(task), &[CHANGE], false);
    let mut count = 0_u32;
    for node in view.tree().nodes() {
        if node.states.as_slice().contains(&skein_ui::State::Completed) {
            assert_eq!(node.role, Role::Item);
            assert!(node.styles.as_slice().contains(&look::STEP));
            assert!(node.styles.as_slice().contains(&look::STEP_DONE));
            count = count.checked_add(1).expect("completed step count");
        }
    }
    count
}
#[test]
fn a_landed_cards_four_steps_are_completed_items() {
    assert_eq!(completed_steps(landed()), STEPS);
}
#[test]
fn a_running_cards_steps_are_not_completed() {
    assert_eq!(completed_steps(change()), 0);
}
fn wake(called: bool, summary: &[u8]) -> c::ActivityPage {
    let mut blocks = List::with_capacity(1);
    blocks.push(c::Block::Result { call: 8, summary: text(summary), failed: false }).expect("wake result");
    let mut messages = List::with_capacity(1);
    messages.push(c::Message { speaker: c::Speaker::Person, blocks }).expect("wake message");
    let mut rows = List::with_capacity(2);
    rows.push(c::ActivityLine {
        position: 3,
        at: Wall::EPOCH,
        event: c::Activity::Turn {
            name: c::TurnName { attempt: 2, number: 1 },
            turn: c::TurnRead::Decoded(c::Turn {
                messages,
                used: c::Usage { input: 0, output: 0, cached: 0, cache_written: 0 },
                spent: 0,
                read_through: None,
                cut: false,
            }),
        },
    })
    .expect("wake turn");
    if called {
        for line in &activity(&[CHANGE], false).rows {
            rows.push(line.clone()).expect("call turn");
        }
    }
    c::ActivityPage { rows, before: None, after: None, cut: false }
}
#[test]
fn a_landed_change_shows_its_card_at_its_result_once_its_call_has_left() {
    let view = view_root(&root_activity(&[landed()], wake(false, WAKE_SUMMARY), &[]));
    assert!(fixtures::find(view.tree(), Role::Region, b"Task 2").is_some());
    assert!(find(&view, Find::text("T2 · CHANGE".as_bytes())).is_some());
    status(&view, b"Landed", look::STATUS_DONE);
    assert_eq!(work_slots(&view), 0);
    assert!(skein_lib::bytes::find(&fixtures::text(view.tree()), b"Begin the work your brief describes.").is_none());
}
#[test]
fn a_card_shows_once_at_its_call_when_its_result_shares_the_window() {
    let view = view_root(&root_activity(&[landed()], wake(true, WAKE_SUMMARY), &[]));
    let mut cards = 0_u32;
    for node in view.tree().nodes() {
        if node.role == Role::Region && node.name.as_deref() == Some(b"Task 2") {
            cards = cards.checked_add(1).expect("card count");
        }
    }
    assert_eq!(cards, 1);
    let said = fixtures::text(view.tree());
    let call = skein_lib::bytes::find(&said, b"I'll delegate").expect("call turn");
    let card = skein_lib::bytes::find(&said, "T2 · CHANGE".as_bytes()).expect("card");
    assert!(call < card, "the card stays at its call");
    let log = view.tree().nodes();
    let mut regions = 0_u32;
    for (at, node) in log.iter().enumerate() {
        if node.role == Role::Region && node.styles.as_slice().contains(&look::TURN) {
            regions = regions.checked_add(1).expect("region count");
            if regions > 1 {
                let end = at.checked_add(usize::try_from(node.size).expect("region size")).expect("region end");
                for child in &log[at..end] {
                    if child.role == Role::Paragraph {
                        assert!(
                            child.name.as_deref() != Some(b"Person") && child.name.as_deref() != Some(b"You"),
                            "the result region has no speaker shell"
                        );
                    }
                }
            }
        }
    }
    assert!(regions > 1, "request and turn regions");
    assert_eq!(work_slots(&view), 0);
}
#[test]
fn a_result_without_its_document_draws_no_input_work() {
    let view = view_root(&root_activity(&[], wake(false, WAKE_SUMMARY), &[]));
    assert!(fixtures::find(view.tree(), Role::Region, b"Task 2").is_none());
    assert_eq!(work_slots(&view), 0);
    assert!(fixtures::find(view.tree(), Role::Paragraph, WAKE_SUMMARY).is_none());
}
#[test]
fn a_persons_result_draws_no_input_work() {
    let summary = b"...\n2: Result from task 2:\nwords";
    let mut task = change();
    task.requester = c::OwnerRequester::Person(CHANGE);
    let view = view_root(&root_activity(&[task], wake(false, summary), &[]));
    assert!(fixtures::find(view.tree(), Role::Region, b"Task 2").is_none());
    assert_eq!(work_slots(&view), 0);
    assert!(fixtures::find(view.tree(), Role::Paragraph, b"Tool completed").is_none());
    assert!(fixtures::find(view.tree(), Role::Paragraph, summary).is_none());
}
#[test]
fn a_partly_carded_result_draws_no_input_work() {
    let summary = b"Result from task 2:\nlanded\n3: Result from task 3:\nwaiting";
    let view = view_root(&root_activity(&[landed()], wake(false, summary), &[]));
    assert_eq!(work_slots(&view), 0);
    assert!(fixtures::find(view.tree(), Role::Paragraph, summary).is_none());
}
#[test]
fn each_state_says_its_words_in_its_status_look() {
    for (ending, held, name, token) in [
        (None, true, b"Held".as_slice(), look::STATUS_HELD),
        (Some(c::Ending::Done), false, b"Completed", look::STATUS_DONE),
        (Some(c::Ending::Failed { reason: None }), false, b"Failed", look::STATUS_HELD),
        (
            Some(c::Ending::Cancelled { reason: None, by: c::Actor::Party(1), at: None }),
            false,
            b"Cancelled",
            look::STATUS_QUIET,
        ),
        (Some(c::Ending::Deleted), false, b"Ended", look::STATUS_QUIET),
    ] {
        let mut task = change();
        if held {
            task.chip.phase = c::Phase::Held;
        }
        if let Some(ending) = ending {
            task.chip.details = Box::new(c::ChipPhase::Ended { how: ending, at: Wall::EPOCH });
        }
        let view = view(Some(task), &[CHANGE], false);
        status(&view, name, token);
        steps(&view, false);
    }
}
#[test]
fn a_report_has_no_steps() {
    let mut task = change();
    task.result_contract = c::TaskContractDocument::Report { words: 256 };
    task.result = Some(c::TaskResult::Ending(Box::new(c::OwnerChipEnding::Done(c::ResultValue::Report(document(
        b"A **useful** report",
    ))))));
    let view = view(Some(task), &[CHANGE], false);
    assert!(find(&view, Find::text("T2 · REPORT".as_bytes())).is_some());
    assert!(find(&view, Find::exactly_named(Role::Paragraph, b"A useful report")).is_some());
    assert!(find(&view, Find::named(Role::List, b"Steps")).is_none());
}
#[test]
fn a_failed_change_says_its_reason() {
    let mut task = change();
    task.result = Some(c::TaskResult::Ending(Box::new(c::OwnerChipEnding::Failed(document(b"Checks failed")))));
    let view = view(Some(task), &[CHANGE], false);
    status(&view, b"Failed", look::STATUS_HELD);
    assert!(find(&view, Find::text(b"Checks failed")).is_some());
}
#[test]
fn a_delegation_without_its_document_shows_its_call_work() {
    let view = view(None, &[CHANGE], false);
    assert!(fixtures::find(view.tree(), Role::Region, b"Task 2").is_none());
    assert!(skein_lib::bytes::find(&fixtures::text(view.tree()), b"delegate").is_some());
    super::chat::assert_call_fold(view.tree(), b"1 tool call", b"delegate", &[b"delegateDelegated tasks: 2"]);
    assert!(fixtures::find(view.tree(), Role::Paragraph, b"Delegated tasks: 2").is_some());
    assert_eq!(work_slots(&view), 1);
}
#[test]
fn a_carded_task_hides_its_heard_result() {
    let view = view(Some(landed()), &[CHANGE], true);
    let text = fixtures::text(view.tree());
    assert!(skein_lib::bytes::find(&text, b"Carded result").is_none());
    assert!(skein_lib::bytes::find(&text, b"Duplicate result").is_none());
    assert!(skein_lib::bytes::find(&text, b"Another task's result").is_some());
}
#[test]
fn a_partly_carded_call_keeps_its_work() {
    let view = view(Some(change()), &[CHANGE, 3], false);
    assert!(find(&view, Find::text("T2 · CHANGE".as_bytes())).is_some());
    assert_eq!(work_slots(&view), 1);
}

pub(super) fn step_roles() {
    let view = view(Some(landed()), &[CHANGE], false);
    for (token, role) in [(look::STEPS, Role::List), (look::STEP, Role::Item), (look::STEP_DONE, Role::Item)] {
        let mut found = false;
        for node in view.tree().nodes() {
            if node.styles.as_slice().contains(&token) {
                assert_eq!(node.role, role);
                found = true;
            }
        }
        assert!(found);
    }
}

#[test]
fn a_calls_cards_keep_its_task_order() {
    let mut other = change();
    other.chip.task = 3;
    let view = view_documents(&[change(), other], &[3, CHANGE], false);
    let mut cards = List::with_capacity(2);
    for node in view.tree().nodes() {
        if node.styles.as_slice().contains(&look::TASK_CARD) {
            cards.push(node.name.as_deref().expect("card name")).expect("two cards");
        }
        assert!(!node.styles.as_slice().contains(&look::WORK));
    }
    assert_eq!(cards.as_slice(), [b"Task 3".as_slice(), b"Task 2"]);
}

fn work_slots(view: &crate::View) -> u32 {
    let mut count = 0_u32;
    for node in view.tree().nodes() {
        if node.role == Role::Slot && node.styles.as_slice().contains(&look::WORK) {
            count = count.checked_add(1).expect("work slot count");
        }
    }
    count
}

fn first_run_view(dark: bool, landed_change: bool) -> crate::View {
    use fixtures::first_run as run;
    let mut receiving = limits();
    receiving.client.domain.rows = 3;
    receiving.client.domain.text = 512;
    receiving.client.domain.turn_text = 2048;
    let mut chat = change();
    chat.chip.task = if dark { 4 } else { CHAT };
    chat.requester = c::OwnerRequester::Person(fixtures::chat::PARTY);
    chat.chip.title = c::DocumentText::complete(run::text(if dark { run::DARK } else { run::CSV }));
    chat.spec.specification.words = chat.chip.title.clone();
    chat.chip.phase = c::Phase::Idle;
    chat.flags.ended_by = c::EndedBy::Requester;
    chat.acts = List::with_capacity(3);
    for act in [c::ActKind::Message, c::ActKind::Stop, c::ActKind::Cancel] {
        chat.acts.push(c::Offered { act, admission: c::Admission::Allowed, effect: None }).expect("chat act");
    }
    chat.activity = if dark {
        4
    } else if landed_change {
        9
    } else {
        5
    };
    let document = if landed_change { landed() } else { change() };
    let root = root_chat_limits(
        Some(&chat),
        &[document],
        run::activity(dark, landed_change, receiving.client.domain.turn_text),
        if dark || landed_change { &[] } else { &[CHANGE] },
        &receiving,
    );
    let mut view = crate::View::new(
        &crate::Limits { tree: fixtures::tree_limits() },
        &receiving,
        fixtures::words(),
        fixtures::FORGE,
    );
    fixtures::build(&mut view, &root);
    view
}
fn in_order(said: &[u8], expected: &[&[u8]]) {
    let mut rest = said;
    for value in expected {
        let at = skein_lib::bytes::find(rest, value).expect("next transcript text");
        rest = &rest[at.checked_add(value.len()).expect("matched text end")..];
    }
}
#[test]
fn the_csv_chat_receives_the_whole_turn_and_anchors_the_landed_card_at_its_call() {
    use fixtures::first_run as run;
    let view = first_run_view(false, true);
    super::chat::assert_call_fold(view.tree(), b"1 tool call", b"wait", &[b"wait"]);
    let said = fixtures::text(view.tree());
    in_order(
        &said,
        &[
            b"You",
            run::CSV,
            b"Temper",
            run::DELEGATING,
            "T2 · CHANGE".as_bytes(),
            b"wait",
            b"Temper",
            run::LANDED,
            b"wait",
        ],
    );
    for hidden in [
        b"Run 1 began".as_slice(),
        b"Earlier lines omitted",
        b"Earlier text omitted",
        b"Run 1 ended",
        b"Begin the work your brief describes.",
        b"Milestone",
        b"Delegated tasks",
        run::MERGE,
        b"Instructions",
        b"Tool completed",
    ] {
        assert!(skein_lib::bytes::find(&said, hidden).is_none(), "{hidden:?}");
    }
    assert_eq!(TreeFace::new(view.tree()).find(&Find::exactly_named(Role::Paragraph, b"delegate")), Found::Absent);
    assert!(fixtures::find(view.tree(), Role::Button, b"Older").is_none());
    assert!(fixtures::find(view.tree(), Role::Status, b"Read").is_none());
}
// QA frame 004: the change's result reached the parked chat, before the
// resumed turn and before its independently watched task document caught up.
// Whole, the first turn's delegate call anchors the card; cut, the heard
// result does, and it is never drawn as bare words (client/README.md, 5.7).
#[test]
fn the_csv_chat_keeps_its_card_and_header_together_as_the_result_arrives() {
    for turn_text in [2048, 512] {
        csv_chat_hears_its_result(turn_text);
    }
}
fn csv_chat_hears_its_result(turn_text: u32) {
    use fixtures::first_run as run;
    let mut receiving = limits();
    receiving.client.domain.rows = 3;
    receiving.client.domain.text = 512;
    receiving.client.domain.turn_text = turn_text;
    let mut activity = run::activity(false, true, receiving.client.domain.turn_text);
    let mut rows = List::with_capacity(6);
    for line in &activity.rows {
        if line.position <= 6 {
            let mut line = line.clone();
            if let c::Activity::Owner { source, .. } = &mut line.event
                && let c::OwnerActivity::Heard(word) = &mut source.line
            {
                word.words = run::text(b"1d0b14e4db018fed000000000000000300000000");
            }
            rows.push(line).expect("before resumed turn");
        }
    }
    activity.rows = rows;
    let mut chat = change();
    chat.chip.task = CHAT;
    chat.requester = c::OwnerRequester::Person(fixtures::chat::PARTY);
    chat.waits_on = c::WaitsOn::News;
    chat.wake.words = c::WakeRule::Immediate;
    let c::ChipPhase::Owner { details, .. } = chat.chip.details.as_mut() else { panic!("owner") };
    details.live_delegates = 0;
    let mut task = change();
    task.requester = c::OwnerRequester::Task(CHAT);
    let (mut root, mut reads) = root_chat_with_reads(Some(&chat), &[task], activity, &[], &receiving);
    let mut view = crate::View::new(
        &crate::Limits { tree: fixtures::tree_limits() },
        &receiving,
        fixtures::words(),
        fixtures::FORGE,
    );
    fixtures::build(&mut view, &root);
    let cut = fixtures::find(view.tree(), Role::Status, b"Earlier text omitted").is_some();
    assert_eq!(cut, turn_text < 2048, "the first turn is cut only below its 1.2 KB");
    assert_csv_card(&view, false);
    let Some(Request::Client {
        request:
            jig_client::Request::Down {
                down: jig_client::Down::Read { read, what: jig_client::Readable::Jig(c::Read::Task { task: CHANGE }) },
            },
    }) = reads.pop()
    else {
        panic!("hearing T2's result refreshes its document")
    };
    assert!(reads.is_empty(), "one final document read");
    let mut out = Queue::uncapped();
    up(
        &mut root,
        jig_client::Up::ReadDone {
            read,
            result: jig_client::Readout::Jig(c::ReadResult::Value(c::ReadValue::Task(Box::new(landed())))),
        },
        &receiving,
        &mut out,
    );
    fixtures::build(&mut view, &root);
    assert_csv_card(&view, true);
}
fn assert_csv_card(view: &crate::View, landed: bool) {
    assert_eq!(change_cards(view), 1, "exactly one card for the change, at its call or its heard result");
    let mut bare_hex = false;
    for (index, _) in view.tree().nodes().iter().enumerate() {
        let text = fixtures::node_text(view.tree(), index);
        if text.len() == 40 {
            let mut hex = true;
            for byte in &text {
                if !byte.is_ascii_hexdigit() {
                    hex = false;
                }
            }
            bare_hex |= hex;
        }
    }
    assert!(!bare_hex, "the card owns the result; no bare merge commit text node");
    status(
        view,
        if landed { b"Landed" } else { b"Running" },
        if landed { look::STATUS_DONE } else { look::STATUS_ACTIVE },
    );
    assert_eq!(fixtures::find(view.tree(), Role::Status, b"waiting on 1 delegate").is_some(), !landed);
    assert_eq!(fixtures::find(view.tree(), Role::Status, b"waiting on your words").is_some(), landed);
}
#[test]
fn the_csv_chat_stops_waiting_when_its_card_lands_before_the_chat_document() {
    use fixtures::first_run as run;
    let mut receiving = limits();
    receiving.client.domain.rows = 3;
    receiving.client.domain.text = 512;
    receiving.client.domain.turn_text = 2048;
    let mut chat = change();
    chat.chip.task = CHAT;
    chat.requester = c::OwnerRequester::Person(fixtures::chat::PARTY);
    chat.waits_on = c::WaitsOn::Delegates;
    chat.wake.words = c::WakeRule::Immediate;
    let c::ChipPhase::Owner { details, .. } = chat.chip.details.as_mut() else { panic!("owner") };
    details.live_delegates = 1;
    let mut task = change();
    task.requester = c::OwnerRequester::Task(CHAT);
    for landed_change in [false, true] {
        let document = if landed_change { landed() } else { task.clone() };
        let root = root_chat_limits(
            Some(&chat),
            &[document],
            run::activity(false, false, receiving.client.domain.turn_text),
            &[CHANGE],
            &receiving,
        );
        let mut view = crate::View::new(
            &crate::Limits { tree: fixtures::tree_limits() },
            &receiving,
            fixtures::words(),
            fixtures::FORGE,
        );
        fixtures::build(&mut view, &root);
        assert_csv_card(&view, landed_change);
    }
}
#[test]
fn the_dark_theme_chat_has_its_whole_answer_and_waits_with_its_composer() {
    use fixtures::first_run as run;
    let view = first_run_view(true, false);
    let said = fixtures::text(view.tree());
    in_order(&said, &[b"You", run::DARK, b"Temper", run::DARK_ANSWER]);
    for hidden in [
        b"Done".as_slice(),
        b"Ended",
        b"finish",
        b"Milestone",
        b"Earlier text omitted",
        b"Tool completed",
        b"Instructions",
    ] {
        assert!(skein_lib::bytes::find(&said, hidden).is_none(), "{hidden:?}");
    }
    let mut composer = false;
    for node in view.tree().nodes() {
        composer |= node.styles.as_slice().contains(&look::COMPOSER_AREA);
    }
    assert!(composer);
}

#[test]
fn a_failed_chat_has_one_end_line_with_its_reason() {
    let mut chat = change();
    chat.chip.task = CHAT;
    chat.result = Some(c::TaskResult::Ending(Box::new(c::OwnerChipEnding::Failed(document(b"No capacity")))));
    let view = view_root(&root_chat(Some(&chat), &[], after_call(), &[]));
    assert!(fixtures::find(view.tree(), Role::Paragraph, "Failed · No capacity".as_bytes()).is_some());
}

#[test]
fn an_ended_chat_has_no_composer_line_or_chooser() {
    let mut chat = change();
    chat.chip.task = CHAT;
    chat.result = Some(c::TaskResult::Ending(Box::new(c::OwnerChipEnding::Failed(document(b"No capacity")))));
    let view = view_root(&root_chat(Some(&chat), &[], after_call(), &[]));
    for node in view.tree().nodes() {
        for token in [look::COMPOSER_AREA, look::CHOICE, look::ACCOUNT_LINE] {
            assert!(!node.styles.as_slice().contains(&token));
        }
        assert!(!(node.role == Role::Disclosure && node.name.as_deref() == Some(b"Change model")));
    }
}
