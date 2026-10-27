//! Shared first-page reads and exact turn fixtures for the root harness.
use super::*;
/// A task's tree and run each close on their first delivery.
pub(crate) const RELATED_WATCHES: u32 = 2;
/// One row exercises the printed identity and the continuation that follows it.
pub(crate) const PAGE_ROWS: u32 = 1;
/// One byte past the part bound exercises an exact continuation offset.
pub(crate) const LONG_PART: u32 = crate::limits::PART + 1;
/// The engine/client turn bound is one MiB, with one byte making it too large.
pub(crate) const TOO_LARGE: u64 = crate::limits::DOCUMENT + 1;
/// The archive and run share the committed attempt number.
pub(crate) const ATTEMPT: u64 = 3;
/// Its turn number differs from the attempt number.
pub(crate) const TURN: u32 = 7;
/// One message exercises a cut; zero would leave the renderer without text.
pub(crate) const TURN_MESSAGES: u32 = 1;
/// One block exercises a part bound; zero would never reach that bound.
pub(crate) const TURN_BLOCKS: u32 = 1;
pub(crate) const TASKS_CALL: &[&str] = &["task", "list", "--project", "1"];
pub(crate) const ENDED_CALL: &[&str] =
    &[TASKS_CALL[0], TASKS_CALL[1], TASKS_CALL[2], "shop", "--ended", "--after", "1122"];
pub(crate) const TURN_CALL: &[&str] = &["task", "turn", TASK_CALL[2], "3.7"];
pub(crate) const ACTIVITY_CALL: &[&str] = &["task", "activity", TASK_CALL[2]];
pub(crate) const CHAT_CALL: &[&str] = &["chat", "show", TASK_CALL[2]];
/// Two distinct kept turns make losing either conversation entry observable.
pub(crate) const KEPT_TURNS: &[&[u8]] = &[b"First kept turn", b"Second kept turn"];
pub(crate) fn kept_conversation() -> c::ReadResult {
    let mut rows = List::with_capacity(u32::try_from(KEPT_TURNS.len()).expect("two turns"));
    for (index, words) in KEPT_TURNS.iter().enumerate() {
        let mut blocks = List::with_capacity(TURN_BLOCKS);
        blocks.push(c::Block::Text(turn_words(words))).expect("one text block");
        let c::ReadResult::Value(c::ReadValue::Turn(turn)) = turn_blocks(blocks) else {
            panic!("decoded turn fixture")
        };
        rows.push(c::ActivityLine {
            position: u64::try_from(index).expect("two positions").checked_add(1).expect("nonzero position"),
            at: Wall::EPOCH,
            event: c::Activity::Turn {
                name: c::TurnName {
                    attempt: ATTEMPT,
                    number: TURN.checked_add(u32::try_from(index).expect("two turns")).expect("next turn"),
                },
                turn: turn.turn,
            },
        })
        .expect("two kept turns");
    }
    c::ReadResult::Value(c::ReadValue::Activity(Box::new(c::ActivityPage {
        cut: false,
        rows,
        before: None,
        after: None,
    })))
}
pub(crate) const RUN_CALL: &[&str] = &["task", "run", TASK_CALL[2]];
pub(crate) const INBOX_CALL: &[&str] = &["inbox"];
pub(crate) const COMPOSITE_CALL: &[&str] =
    &[TASK_CALL[0], TASK_CALL[1], TASK_CALL[2], "--tree", "--activity", ONE_ROW, "--run"];
pub(crate) fn cursor() -> c::Cursor {
    c::Cursor::new(Box::from([0x11, 0x22]), WORDS).expect("opaque cursor")
}
pub(crate) fn task_line() -> c::TaskLine {
    let task = goal(TASK, waiting_on_me());
    c::TaskLine {
        chip: {
            let mut chip = task.chip;
            chip.title = document_text(b"goal");
            chip
        },
        project: PROJECT,
        requester: task.requester,
        tracked: None,
        label: task.label,
        flags: task.flags,
        changed: Wall::EPOCH,
        last_activity: Wall::EPOCH,
        ended_at: None,
        budget: task.budget,
        waits_on: task.waits_on,
        phases: c::OwnerPhaseCounts {
            waiting: 0,
            idle: 0,
            starting: 0,
            running: 0,
            backing_off: 0,
            closing: 0,
            held: 0,
            done: 0,
            failed: 0,
            cancelled: 0,
        },
        held_below: 0,
        conversation: None,
    }
}
pub(crate) fn task_page_with_cursor() -> c::ReadResult {
    let mut rows = List::with_capacity(PAGE_ROWS);
    rows.push(task_line()).expect("one task row");
    c::ReadResult::Value(c::ReadValue::Tasks(Box::new(c::TaskPage {
        live: rows,
        live_after: Some(cursor()),
        ended: c::EndedGroup::None,
    })))
}
pub(crate) fn text(size: u32) -> c::Text {
    let mut out = skein_lib::Writer::new(usize::try_from(size).expect("fixture length"));
    for _ in 0..size {
        out.put(b"x").expect("fixture text");
    }
    c::Text::new(out.finish(), size).expect("fixture bound")
}
pub(crate) fn archive(turn: c::TurnRead) -> c::ArchivedTurn {
    c::ArchivedTurn {
        task: TASK,
        name: c::TurnName { attempt: ATTEMPT, number: TURN },
        cumulative: 0,
        read: None,
        at: Wall::EPOCH,
        turn,
    }
}
pub(crate) fn turn_text(size: u32) -> c::ReadResult {
    let mut blocks = List::with_capacity(TURN_BLOCKS);
    blocks.push(c::Block::Text(text(size))).expect("one block");
    turn_blocks(blocks)
}
pub(crate) fn turn_blocks(blocks: List<c::Block>) -> c::ReadResult {
    let mut messages = List::with_capacity(TURN_MESSAGES);
    messages.push(c::Message { speaker: c::Speaker::Assistant, blocks }).expect("one message");
    c::ReadResult::Value(c::ReadValue::Turn(Box::new(archive(c::TurnRead::Decoded(c::Turn {
        messages,
        used: c::Usage { input: 0, output: 0, cached: 0, cache_written: 0 },
        spent: 0,
        read_through: None,
        cut: false,
    })))))
}
pub(crate) fn turn_too_large(bytes: u64) -> c::ReadResult {
    c::ReadResult::None(c::ReadNone::TooLarge { bytes })
}
pub(crate) fn activity_too_large() -> c::ReadResult {
    let mut rows = List::with_capacity(PAGE_ROWS);
    rows.push(c::ActivityLine {
        position: 1,
        at: Wall::EPOCH,
        event: c::Activity::Turn {
            name: c::TurnName { attempt: ATTEMPT, number: TURN },
            turn: c::TurnRead::Undecodable { why: c::TurnProblem::TooLarge { bytes: TOO_LARGE } },
        },
    })
    .expect("one turn row");
    c::ReadResult::Value(c::ReadValue::Activity(Box::new(c::ActivityPage {
        cut: false,
        rows,
        before: None,
        after: None,
    })))
}
pub(crate) fn no_provisional() -> c::Delivery {
    c::Delivery::RunState(Box::new(c::RunState {
        task: TASK,
        attempt: Some(ATTEMPT),
        committed_turn: TURN,
        writing: None,
        call: None,
    }))
}
impl Harness {
    pub fn deliver_subject(&mut self, wanted: &client::Watchable, delivery: c::Delivery) {
        let mut tokens = List::with_capacity(OUTPUTS);
        for (token, subject) in &self.watches {
            if subject == wanted {
                tokens.push(*token).expect("bounded watches");
            }
        }
        for token in tokens.into_boxed() {
            self.client(client::Event::Up {
                up: client::Up::Watched { watch: token, event: c::WatchEvent::Delivered(delivery.clone()) },
            });
        }
    }
    pub fn deliver_run(&mut self, delivery: c::Delivery) {
        self.deliver_subject(&client::Watchable::Jig(c::Subject::Run { task: TASK, turn: None, offset: 0 }), delivery);
    }
}

pub(crate) fn read_answer(argv: &[&str], value: c::ReadResult) -> crate::jig::Printed {
    let (mut root, call) = read_call(argv);
    root.serve(call, value);
    root.answer(call).expect("terminal read answer")
}
pub(crate) fn composite() -> (Harness, u32) {
    composite_call(COMPOSITE_CALL)
}
pub(crate) fn composite_call(argv: &[&str]) -> (Harness, u32) {
    let (mut root, call) = read_call(argv);
    root.serve(call, task_result(goal(TASK, waiting_on_me())));
    root.serve(call, activity_too_large());
    root.deliver_subject(
        &client::Watchable::Jig(c::Subject::Tree { task: TASK, window: crate::limits::ROWS }),
        c::Delivery::Tree(Box::new(composite_tree())),
    );
    root.deliver_run(no_provisional());
    (root, call)
}
/// A follow-up begins at the exact part bound, where the remaining byte starts.
const PART_OFFSET: &str = "16384";
pub(crate) const TURN_NEXT_CALL: &[&str] =
    &[TURN_CALL[0], TURN_CALL[1], TURN_CALL[2], TURN_CALL[3], "--part", "1", "--from", PART_OFFSET];
pub(crate) const WRONG_INBOX_GROUP: &[&str] = &[INBOX_CALL[0], "--kind", "questions", "--after", "011122"];

/// Two blocks retain the call input, its outcome, and a distinct result summary.
pub(crate) const CALL_RESULT_BLOCKS: u32 = 2;
/// One connector reference is enough to exceed the free-text window with a path.
pub(crate) const TURN_REFERENCES: u32 = 1;
/// At the part bound there is no continuation; below it would not exercise equality.
pub(crate) const PART_SIZE: u32 = crate::limits::PART;
pub(crate) const NEEDS_NEXT_CALL: &[&str] = &[INBOX_CALL[0], "--after", "001122"];
pub(crate) fn inbox_page(more: bool) -> c::ReadResult {
    inbox_rows(c::InboxReadGroup::Needs, PAGE_ROWS, more)
}
pub(crate) fn inbox_rows(group: c::InboxReadGroup, count: u32, more: bool) -> c::ReadResult {
    let mut rows = List::with_capacity(count);
    for index in 0..count {
        let task = TASK.checked_add(u64::from(index)).expect("fixture task number");
        let (object, kind) = match group {
            c::InboxReadGroup::Needs => (c::ObjectKey::Question { task, message: 2 }, c::InboxGroup::Questions),
            c::InboxReadGroup::Updates => {
                (c::ObjectKey::Result(task.checked_add(1).expect("different updated task")), c::InboxGroup::Results)
            }
        };
        rows.push(c::InboxEntry {
            reply: None,
            lineage: List::with_capacity(EMPTY),
            goal: None,
            recipient: None,
            stall: None,
            object,
            project: PROJECT,
            group: kind,
            position: u64::from(index),
            at: Wall::EPOCH,
        })
        .expect("bounded inbox rows");
    }
    c::ReadResult::Value(c::ReadValue::Inbox(Box::new(c::InboxPage {
        cut: false,
        rows,
        before: None,
        after: if more { Some(cursor()) } else { None },
    })))
}
pub(crate) fn inbox_answer(needs: u32, updates: u32, more: bool) -> crate::jig::Printed {
    let (mut root, call) = read_call(INBOX_CALL);
    root.serve(call, inbox_rows(c::InboxReadGroup::Needs, needs, false));
    root.serve(call, inbox_rows(c::InboxReadGroup::Updates, updates, more));
    root.answer(call).expect("inbox answered once")
}
/// A full needs page leaves no room for the single update; a smaller page fits.
pub(crate) const FULL_NEEDS: u32 = crate::limits::ROWS;
/// Two rows make a one-row activity limit omit the older supplied row.
pub(crate) const LIMITED_ROWS: u32 = 2;
pub(crate) const UPDATES_START_CALL: &[&str] = &[INBOX_CALL[0], "--after", "01"];
pub(crate) const LIMITED_ACTIVITY_CALL: &[&str] =
    &[ACTIVITY_CALL[0], ACTIVITY_CALL[1], ACTIVITY_CALL[2], "--before", "1122", "--limit", ONE_ROW];
pub(crate) const LIMITED_TASK_CALL: &[&str] = &[TASK_CALL[0], TASK_CALL[1], TASK_CALL[2], "--activity", ONE_ROW];
pub(crate) fn activity_words() -> c::ReadResult {
    let mut rows = List::with_capacity(LIMITED_ROWS);
    for index in 0..LIMITED_ROWS {
        rows.push(c::ActivityLine {
            position: u64::from(index).checked_add(1).expect("fixture position"),
            at: Wall::EPOCH,
            event: c::Activity::Words { by: activity_party(), message: u64::from(index), words: turn_words(b"words") },
        })
        .expect("two supplied activity rows");
    }
    c::ReadResult::Value(c::ReadValue::Activity(Box::new(c::ActivityPage {
        cut: false,
        rows,
        before: Some(cursor()),
        after: None,
    })))
}
pub(crate) fn turn_call(path: Option<List<Box<[u8]>>>) -> c::ReadResult {
    let mut parts = List::with_capacity(TURN_REFERENCES);
    if let Some(path) = path {
        parts.push(c::Part::Resource { connector: 4, path }).expect("one reference");
    }
    let mut blocks = List::with_capacity(CALL_RESULT_BLOCKS);
    blocks
        .push(c::Block::Call(c::Call {
            number: 2,
            tool: turn_words(b"read"),
            input: turn_words(b"request"),
            outcome: Some(turn_words(b"reply")),
            failed: false,
            objects: List::with_capacity(EMPTY),
            parts,
        }))
        .expect("one call");
    blocks.push(c::Block::Result { call: 2, summary: turn_words(b"summary"), failed: true }).expect("one result");
    turn_blocks(blocks)
}

fn turn_words(bytes: &[u8]) -> c::Text {
    c::Text::new(bytes.into(), WORDS).expect("bounded turn words")
}

pub(crate) const TURN_JSON_CALL: &[&str] = &[TURN_CALL[0], TURN_CALL[1], TURN_CALL[2], TURN_CALL[3], "--json"];

fn activity_party() -> c::Party {
    let party = me(PARTY).party;
    c::Party { number: party.number, kind: party.kind, name: turn_words(party.name.bytes()) }
}

pub(crate) fn provisional() -> c::Delivery {
    let c::Delivery::RunState(mut run) = no_provisional() else { unreachable!("run state fixture") };
    run.writing = Some(c::RunText {
        writing: None,
        attempt: ATTEMPT,
        began: None,
        turn: c::ProvisionalTurnName { attempt: ATTEMPT, number: TURN.checked_add(1).expect("next turn"), retry: 0 },
        from: 0,
        through: 5,
        text: turn_words(b"draft"),
        cut: false,
        call: None,
    });
    c::Delivery::RunState(run)
}
pub(crate) fn ended_page() -> c::ReadResult {
    let mut rows = List::with_capacity(PAGE_ROWS);
    rows.push(task_line()).expect("ended task");
    c::ReadResult::Value(c::ReadValue::Tasks(Box::new(c::TaskPage {
        live: List::with_capacity(EMPTY),
        live_after: None,
        ended: c::EndedGroup::Rows { rows, after: Some(cursor()), cut: false },
    })))
}

/// A single request needs one queue entry; zero would lose its call attribution.
pub(crate) const SINGLE_READ: u32 = 1;
/// The task plus its activity need two entries; one would lose the second read.
pub(crate) const COMPOSITE_READS: u32 = 2;
/// One displayed row omits the older row in `LIMITED_ROWS`; zero is invalid argv.
const ONE_ROW: &str = "1";
pub(crate) const TASK_LABELS: &[(&str, u32)] =
    &[("chat", jig_client::shapes::CHAT), ("goal", jig_client::shapes::GOAL)];
pub(crate) fn extended<'a>(base: &[&'a str], extra: &[&'a str]) -> List<&'a str> {
    let mut argv = List::with_capacity(WORDS);
    for word in base {
        argv.push(*word).expect("base fixture argv");
    }
    for word in extra {
        argv.push(*word).expect("extended fixture argv");
    }
    argv
}

pub(crate) fn composite_tree() -> c::Tree {
    c::Tree {
        task: TASK,
        nodes: List::with_capacity(EMPTY),
        folded: c::PhaseCounts {
            ended: 0,
            running: 0,
            waiting: 0,
            held: 0,
            closing: 0,
            done: 0,
            failed: 0,
            cancelled: 0,
        },
        held_below: 0,
    }
}

/// NUL is valid UTF-8 and takes six bytes as a JSON Unicode escape.
pub(crate) fn worst_case_json_text(size: u32) -> Box<[u8]> {
    let mut out = skein_lib::Writer::new(usize::try_from(size).expect("fixture length"));
    for _ in 0..size {
        out.put(&[0]).expect("fixture length");
    }
    out.finish()
}

/// Four nine-segment paths can fill DOCUMENT while each segment stays at TEXT.
const DOCUMENT_RESOURCES: u32 = 4;
pub(crate) fn document_at_byte_bound() -> c::Task {
    use jig_client_protocol_wire::v19 as w;
    let client = crate::limits::client().client;
    let link = jig_client_protocol_link::Limits::new(client.domain, client.exchanges).expect("shipped link limits");
    let mut parts = jig_client_test_records::task_record().into_parts();
    parts.instructions = Some(
        w::DocumentText::new(
            &link.wire,
            w::DocumentTextParts { bytes: worst_case_json_text(client.domain.text), cut: false },
        )
        .expect("worst-case instructions at text bound"),
    );
    parts.resources = resource_pieces(&link, 0);
    let empty = w::Task::new(&link.wire, parts).expect("task with empty paths");
    let payload = u32::try_from(crate::limits::DOCUMENT)
        .expect("document bound fits")
        .checked_sub(empty.measure())
        .expect("room after framing");
    let mut parts = empty.into_parts();
    parts.resources = resource_pieces(&link, payload);
    let record = w::Task::new(&link.wire, parts).expect("task at document bound");
    assert_eq!(u64::from(record.measure()), crate::limits::DOCUMENT, "whole encoded document at the bound");
    jig_client_protocol_link::translate::documents::task(record, &link).expect("document admitted by shipped limits")
}
fn resource_pieces(link: &jig_client_protocol_link::Limits, payload: u32) -> Box<[u8]> {
    use jig_client_protocol_wire::v19 as w;
    let mut rows = List::with_capacity(DOCUMENT_RESOURCES);
    let mut left = payload;
    let mut size = 0_u32;
    for _ in 0..DOCUMENT_RESOURCES {
        let mut path = List::with_capacity(link.client.segments);
        for _ in 0..link.client.segments {
            let bytes = left.min(link.client.text);
            path.push(worst_case_json_text(bytes)).expect("bounded path");
            left = left.checked_sub(bytes).expect("within payload");
        }
        let part = w::Part::Resource(
            w::PartResource::new(&link.wire, w::PartResourceParts { connector: 4, path }).expect("bounded resource"),
        );
        let row = w::TaskResource::Part(
            w::TaskResourcePart::new(&link.wire, w::TaskResourcePartParts { value: part }).expect("resource row"),
        );
        size = size.checked_add(4).expect("piece prefix").checked_add(row.measure()).expect("bounded pieces");
        rows.push(row).expect("bounded resources");
    }
    assert_eq!(left, 0, "paths contain the whole payload");
    let mut out = skein_lib::Writer::new(usize::try_from(size).expect("pieces fit"));
    for row in &rows {
        out.put(&row.measure().to_be_bytes()).expect("piece prefix");
        row.encode(&mut out).expect("measured row");
    }
    out.finish()
}

/// The owner read's exact hold facts, including its independent revision.
pub(crate) fn held_task(mut task: c::Task, reason: c::HoldCategory) -> c::Task {
    task.chip.phase = c::Phase::Held;
    task.hold = Some(c::TaskHold::Owner(Box::new(c::OwnerChipHold {
        circle: None,
        account: None,
        source: None,
        reason,
        revision: 9,
        before: c::OwnerPriorPhase::Waiting,
        escalation: None,
        tries: c::OwnerFailureTries { transient: 0, permanent: 0, run: 0, agent: 0, lost: 0, invalid: 0 },
        hold_wait_since: None,
    })));
    task
}
