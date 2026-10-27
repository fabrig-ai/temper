//! Common supplied rows for page/window comparisons.
use super::*;
/// Below the window bound, no row should be omitted; smaller misses its edge.
pub(crate) const SHORT_WINDOW: u32 = crate::limits::ROWS - 1;
/// A full window distinguishes its last row from a supplied row beyond the bound.
pub(crate) const FULL_WINDOW: u32 = crate::limits::ROWS;
/// One extra supplied row must set more without entering the hashed rows.
pub(crate) const OVER_WINDOW: u32 = crate::limits::ROWS + 1;
/// Each pair includes every nibble value, checking fixed width and lower case.
pub(crate) const DIGEST_BYTES: [u8; 8] = [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef];
pub(crate) fn tasks_first_window(rows: u32, more: bool) -> (c::TaskPage, c::Tasks) {
    let mut live = List::with_capacity(rows);
    let mut supplied = task_line();
    supplied.chip.phase = c::Phase::Preparing;
    *supplied.chip.details = c::ChipPhase::Starting;
    for index in 0..rows {
        let mut row = supplied.clone();
        row.chip.task = TASK.checked_add(u64::from(index)).expect("bounded task identity");
        live.push(row).expect("bounded rows");
    }
    let page = c::TaskPage {
        live: live.clone(),
        live_after: if more { Some(cursor()) } else { None },
        ended: c::EndedGroup::None,
    };
    (page, c::Tasks { rows: live, more })
}
pub(crate) fn json_without_digest(bytes: &[u8]) -> Box<[u8]> {
    let at = skein_lib::bytes::find(bytes, b",\"digest\":").expect("digest last");
    let mut out = skein_lib::Writer::new(at.checked_add(b"}".len()).expect("bounded JSON"));
    out.put(bytes.get(..at).expect("subject fields")).expect("measured fields");
    out.put(b"}").expect("closing object");
    out.finish()
}
pub(crate) fn printed_digest(bytes: &[u8]) -> [u8; 16] {
    let marker = b",\"digest\":\"";
    let mut at = None;
    for (index, window) in bytes.windows(marker.len()).enumerate().rev() {
        if window == marker {
            at = Some(index);
            break;
        }
    }
    let start = at.expect("printed digest").checked_add(marker.len()).expect("bounded JSON");
    let end = start.checked_add(16).expect("bounded digest");
    let digest: [u8; 16] = bytes.get(start..end).expect("printed digest digits").try_into().expect("sixteen digits");
    for digit in digest {
        assert!(digit.is_ascii_digit() || (b'a'..=b'f').contains(&digit), "lowercase hex digit");
    }
    assert_eq!(bytes.get(end..).expect("digest closure"), b"\"}", "digest is last");
    digest
}

pub(crate) fn limits() -> crate::jig::Limits {
    crate::limits::tools()
}

/// Two update rows leave one outside a 31-needs first window; one misses truncation.
pub(crate) const WINDOW_UPDATES: u32 = 2;
/// The next opaque cursor must differ without changing whether more rows follow.
pub(crate) const OTHER_CURSOR: &[u8] = &[0x33, 0x44];
pub(crate) fn inbox_first_window(total: u32) -> (c::InboxPage, c::Inbox) {
    let needs = total.checked_sub(WINDOW_UPDATES).expect("window includes both groups");
    let c::ReadResult::Value(c::ReadValue::Inbox(needs)) = inbox_rows(c::InboxReadGroup::Needs, needs, false) else {
        panic!("needs page")
    };
    let c::ReadResult::Value(c::ReadValue::Inbox(updates)) =
        inbox_rows(c::InboxReadGroup::Updates, WINDOW_UPDATES, false)
    else {
        panic!("updates page")
    };
    let mut rows = List::with_capacity(crate::limits::ROWS);
    for row in needs
        .rows
        .as_slice()
        .iter()
        .chain(updates.rows.as_slice())
        .take(usize::try_from(crate::limits::ROWS).expect("window bound"))
    {
        rows.push(row.clone()).expect("bounded first window");
    }
    let more = total > crate::limits::ROWS;
    let page =
        c::InboxPage { rows: rows.clone(), before: None, after: if more { Some(cursor()) } else { None }, cut: false };
    let watched = c::Inbox {
        entries: rows,
        more,
        needs: c::Needs { questions: needs.rows.len(), proposals: 0, escalations: 0, person_tasks: 0 },
        updates: c::Count { value: WINDOW_UPDATES, limited: false },
        results_seen: 0,
        replies_seen: List::with_capacity(EMPTY),
    };
    (page, watched)
}
/// Print a first inbox page while preserving its real group continuations.
pub(crate) fn inbox_json_answer(needs: u32, updates: u32, more: bool) -> crate::jig::Printed {
    let argv = extended(INBOX_CALL, &["--json"]);
    let (mut root, call) = read_call(argv.as_slice());
    root.serve(call, inbox_rows(c::InboxReadGroup::Needs, needs, false));
    root.serve(call, inbox_rows(c::InboxReadGroup::Updates, updates, more));
    root.answer(call).expect("inbox answered")
}

/// Different clocks make an accidental environment-dependent hash observable.
pub(crate) const OTHER_CLOCK: u64 = 17;
/// Another live me changes the text's identity lookup, while task facts stay fixed.
pub(crate) const DIGEST_OTHER_PARTY: u64 = PARTY + 1;
/// A composite task has exactly these three independently watchable subjects.
pub(crate) const SUBJECT_PARTS: usize = 3;
pub(crate) fn subject_json(part: &crate::jig::render::Part<'_>) -> Box<[u8]> {
    let limits = limits();
    let bounds = skein_json::writer::Limits { depth: JSON_DEPTH, length: crate::limits::TEXT };
    let mut measure = skein_json::writer::Encoder::measure(&bounds);
    crate::jig::render::document::subjects::encode(part, &limits, true, &mut measure);
    let mut out = skein_json::writer::Encoder::write(measure.measured().expect("subject fits"), &bounds);
    crate::jig::render::document::subjects::encode(part, &limits, true, &mut out);
    out.finish()
}
/// The shipped renderer uses this nesting ceiling; less could reject owner chips.
const JSON_DEPTH: u32 = 32;
pub(crate) fn task_under(party: u64, clock: u64) -> crate::jig::Printed {
    let mut root = signed_in(party);
    root.now = Time::from_nanos(clock);
    root.wall = Wall::from_nanos(clock);
    let call = root.submit(TASK_JSON_CALL);
    root.serve(call, task_result(goal(TASK, waiting_on_me())));
    root.answer(call).expect("task answered")
}
pub(crate) fn non_subject_reads() -> [(List<&'static str>, c::ReadResult); NON_SUBJECT_READS] {
    [
        (extended(INBOX_CALL, &["--project", "shop"]), inbox_page(true)),
        (extended(INBOX_CALL, &["--kind", "questions"]), inbox_page(true)),
        (extended(INBOX_CALL, &["--root", TASK_CALL[2]]), inbox_page(true)),
        (extended(INBOX_CALL, &["--after", "001122"]), inbox_page(true)),
        (extended(TASKS_CALL, &["--after", "1122"]), task_page_with_cursor()),
        (extended(TASKS_CALL, &["--ended"]), ended_page()),
        (extended(ACTIVITY_CALL, &[]), activity_too_large()),
        (extended(TURN_CALL, &[]), turn_call(None)),
    ]
}
/// Four inbox flags, two tasks exclusions, activity and turn cover non-subject reads.
const NON_SUBJECT_READS: usize = 8;
pub(crate) const WATCH_QUERY_FILTERS: &[&[&str]] =
    &[&["--label", "chat"], &["--tracked"], &["--mine"], &["--order", "activity"]];
pub(crate) fn owner_first_window(rows: u32, more: bool) -> (c::OwnerInboxPage, c::OwnerInbox) {
    let mut entries = List::with_capacity(rows);
    for index in 0..rows {
        let task = TASK.checked_add(u64::from(index)).expect("fixture identity");
        let object = c::OwnerInboxObject::Question { message: u64::from(index) };
        entries
            .push(c::OwnerInboxEntry {
                reply: None,
                task: c::OwnerTaskLink::Stored { number: task, status: None },
                project: Some(PROJECT),
                object,
                activity: None,
                at: Some(Wall::EPOCH),
                unread: Some(false),
                cursor: c::OwnerInboxCursor::Inbox { task, object, at: Some(Wall::EPOCH) },
            })
            .expect("bounded owner rows");
    }
    let page = c::OwnerInboxPage {
        rows: entries.clone(),
        after: if more { Some(cursor()) } else { None },
        before: None,
        cut: false,
        selection: None,
    };
    let client::Latest::Jig(c::Delivery::OwnerInbox(mut watched)) = owner_inbox_counts(ONE_NEED, ONE_NEED) else {
        panic!("owner inbox fixture")
    };
    if more {
        watched.next = Some(c::OwnerInboxCursor::Number { number: TASK });
    }
    watched.needs = entries;
    (page, *watched)
}
/// Two revisions of one escalation share a card address but have distinct raw identities.
pub(crate) const ROW_REVISION: u64 = 1;
pub(crate) const OTHER_ROW_REVISION: u64 = ROW_REVISION + 1;

/// Allowed, proposed, refused and bounded are distinct supplied admission documents.
pub(crate) const NO_TOOL_CASES: u32 = 4;
pub(crate) const WINDOW_CASES: &[(u32, bool, u32, &[u8])] = &[
    (SHORT_WINDOW, false, SHORT_WINDOW, b"\"more\":false"),
    (FULL_WINDOW, false, FULL_WINDOW, b"\"more\":false"),
    (FULL_WINDOW, true, FULL_WINDOW, b"\"more\":true"),
    (OVER_WINDOW, false, FULL_WINDOW, b"\"more\":true"),
];
pub(crate) fn occurrences(bytes: &[u8], needle: &[u8]) -> usize {
    let mut count = 0usize;
    for window in bytes.windows(needle.len()) {
        if window == needle {
            count = count.checked_add(1).expect("bounded byte occurrences");
        }
    }
    count
}
