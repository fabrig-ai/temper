//! Rows keep whole identities and opaque cursors in calls that can be replayed.
use super::Text;
use crate::jig::runner::reads::Reading;
use crate::jig::{
    call::{Call, Value, Values},
    call::{number, value},
};
use alloc::boxed::Box;
use jig_client::{Readout, contract as c, words::Word};
use skein_lib::List;
use skein_ui::phrases::Phrasebook;
pub(super) fn tasks(page: &c::TaskPage, call: &Call, limits: &crate::jig::Limits, book: &Phrasebook, out: &mut Text) {
    let (rows, after) = if value(call, 4).is_some() {
        match &page.ended {
            c::EndedGroup::Rows { rows, after, .. } => (rows, after.as_ref()),
            c::EndedGroup::None => {
                out.put(b"no more\n");
                return;
            }
        }
    } else {
        (&page.live, page.live_after.as_ref())
    };
    for row in rows.as_slice().iter().take(usize::try_from(limits.rows).expect("rows fit")) {
        chip(&row.chip, book, out);
    }
    if super::document::subjects::tasks_eligible(call) {
        super::document::subjects::text_digest(b"tasks", &super::Part::TaskPage(page), limits, out);
    }
    next(call, 6, after, out);
}
pub(super) fn task_read_on(page: &c::TaskPage, call: &Call) -> Option<Box<[u8]>> {
    Some(read_on(call, 6, task_after(page, call)?))
}
pub(super) fn task_after<'a>(page: &'a c::TaskPage, call: &Call) -> Option<&'a c::Cursor> {
    if value(call, 4).is_some() {
        match &page.ended {
            c::EndedGroup::Rows { after, .. } => after.as_ref(),
            c::EndedGroup::None => None,
        }
    } else {
        page.live_after.as_ref()
    }
}
pub(super) fn chip(chip: &c::Chip, book: &Phrasebook, out: &mut Text) {
    out.number(chip.task);
    out.put(b" \xc2\xb7 ");
    out.words(chip.title.bytes());
    out.put(b" \xc2\xb7 ");
    out.put(&book.say(0, Word::Phase(chip.phase).id(), &[]).expect("phase phrase").text);
    out.put(b"\n");
}
pub(super) fn tree(delivery: &c::Delivery, project: Option<u32>, book: &Phrasebook, out: &mut Text) {
    out.put(b"tree:\n");
    match delivery {
        c::Delivery::Tree(tree) => {
            for row in &tree.nodes {
                chip(&row.chip, book, out);
            }
        }
        c::Delivery::OwnerTree(tree) => {
            chip(&tree.root, book, out);
            for row in &tree.rows {
                chip(&row.chip, book, out);
            }
            tree_cut(tree, project, book, out);
        }
        c::Delivery::OwnerProject(_)
        | c::Delivery::Me(_)
        | c::Delivery::Task(_)
        | c::Delivery::OwnerProposal(_)
        | c::Delivery::Chips(_)
        | c::Delivery::Inbox(_)
        | c::Delivery::OwnerInbox(_)
        | c::Delivery::Tasks(_)
        | c::Delivery::Run(_)
        | c::Delivery::RunState(_)
        | c::Delivery::Settings(_)
        | c::Delivery::Deployment(_)
        | c::Delivery::OwnerDeployment(_)
        | c::Delivery::OwnerAccounts(_) => unreachable!("tree watch delivers a tree"),
    }
}
// The owner's counters retain their field order, independently of visible rows.
fn tree_cut(tree: &c::OwnerTree, project: Option<u32>, book: &Phrasebook, out: &mut Text) {
    let mut counts = [0u64; 10];
    for fold in &tree.folded {
        let p = fold.phases;
        for (count, supplied) in counts.iter_mut().zip([
            p.waiting,
            p.idle,
            p.starting,
            p.running,
            p.backing_off,
            p.closing,
            p.held,
            p.done,
            p.failed,
            p.cancelled,
        ]) {
            *count = count.checked_add(u64::from(supplied)).expect("bounded folded tasks");
        }
    }
    let mut total = 0u64;
    for count in counts {
        total = total.checked_add(count).expect("bounded folded tasks");
    }
    if total == 0 {
        return;
    }
    out.put(b"cut: ");
    out.number(total);
    out.put(b" more not shown (");
    let mut separator = b"".as_slice();
    for (count, phase) in counts.into_iter().zip([
        c::Phase::Waiting,
        c::Phase::Idle,
        c::Phase::Preparing,
        c::Phase::Running,
        c::Phase::BackingOff,
        c::Phase::Closing,
        c::Phase::Held,
        c::Phase::Done,
        c::Phase::Failed,
        c::Phase::Cancelled,
    ]) {
        if count > 0 {
            out.put(separator);
            out.number(count);
            out.put(b" ");
            out.put(&book.say(0, Word::Phase(phase).id(), &[]).expect("phase phrase").text);
            separator = b", ";
        }
    }
    out.put(b"); read on with: ");
    let project = if let c::ChipPhase::Owner { details, .. } = tree.root.details.as_ref() {
        details.project
    } else {
        project.expect("the tree call read its root's project")
    };
    let values = Box::new([
        Some(Values::One(Value::Project(crate::jig::call::Project::Number(project)))),
        None,
        None,
        None,
        None,
        None,
        None,
        Some(Values::One(Value::Number(tree.root.task))),
    ]);
    write_call(&Call { tool: 2, form: 0, values, key: None }, out);
    out.put(b"\n");
}
pub(super) fn next(call: &Call, index: usize, cursor: Option<&c::Cursor>, out: &mut Text) {
    let Some(cursor) = cursor else {
        out.put(b"no more\n");
        return;
    };
    out.put(b"read on with: ");
    continuation(call, index, cursor, out);
    out.put(b"\n");
}
pub(super) fn read_on(call: &Call, index: usize, cursor: &c::Cursor) -> Box<[u8]> {
    let mut measure = Text::measure();
    continuation(call, index, cursor, &mut measure);
    let mut out = Text::writing_exact(measure);
    continuation(call, index, cursor, &mut out);
    out.finish()
}
fn continuation(call: &Call, index: usize, cursor: &c::Cursor, out: &mut Text) {
    let mut call = call.clone();
    *call.values.get_mut(index).expect("continuation argument") =
        Some(Values::One(Value::Cursor(cursor.value.bytes().into())));
    write_call(&call, out);
}
pub(super) fn write_call(call: &Call, out: &mut Text) {
    let mut call = call.clone();
    let mut argv = List::with_capacity(64);
    let tool = match call.tool {
        1 => &crate::jig::rows::INBOX,
        2 => &crate::jig::rows::TASKS,
        4 => &crate::jig::rows::CHAT,
        5 => &crate::jig::rows::ACTIVITY,
        _ => unreachable!("paged read"),
    };
    // This slice preserves printed follow-up calls; their rename follows separately.
    let tool = crate::jig::table::Tool { name: tool.result, ..*tool };
    call.tool = 0;
    crate::jig::argv::back(&[&tool], &call, &mut argv).expect("a parsed continuation call");
    for (index, word) in argv.as_slice().iter().enumerate() {
        if index > 0 {
            out.put(b" ");
        }
        argument(word, out);
    }
}
pub(crate) fn argument(word: &[u8], out: &mut Text) {
    let mut quoted = word.is_empty();
    for byte in word {
        if !byte.is_ascii_alphanumeric() && !b"-_./:".contains(byte) {
            quoted = true;
        }
    }
    if !quoted {
        out.put(word);
        return;
    }
    out.put(b"'");
    for byte in word {
        if *byte == b'\'' {
            out.put(b"'\\''");
        } else {
            out.put(&[*byte]);
        }
    }
    out.put(b"'");
}
pub(super) fn activity_limit(call: &Call, rows: u32) -> u32 {
    match number(call, 2) {
        Some(limit) => u32::try_from(limit).expect("parsed row bound").min(rows),
        None => rows,
    }
}

pub(super) fn inboxes(reading: &Reading, limits: &crate::jig::Limits, out: &mut Text) {
    let rows = limits.rows;
    let updates_fit = updates_fit(reading, rows);
    for (local, page) in &reading.pages {
        if *local == 5 && !updates_fit {
            continue;
        }
        match page {
            Readout::Jig(c::ReadResult::Value(c::ReadValue::Inbox(page))) => {
                for row in page.rows.as_slice().iter().take(usize::try_from(rows).expect("rows fit")) {
                    object(row.object, out);
                }
            }
            Readout::Jig(c::ReadResult::Value(c::ReadValue::OwnerInbox(page))) => {
                for row in page.rows.as_slice().iter().take(usize::try_from(rows).expect("rows fit")) {
                    object(row.card_key(), out);
                }
            }
            Readout::Limit | Readout::NotServed { .. } | Readout::Jig(_) | Readout::Family { .. } => {
                unreachable!("validated inbox page")
            }
        }
    }
    if super::document::subjects::inbox_eligible(&reading.called.call) {
        let normalized = super::document::subjects::inbox(reading, limits, false);
        super::document::subjects::text_digest(
            b"inbox",
            &super::Part::read(&normalized).expect("inbox form"),
            limits,
            out,
        );
    }
    next(&reading.called.call, 3, inbox_cursor(reading, rows).as_ref(), out);
}
pub(super) fn updates_fit(reading: &Reading, rows: u32) -> bool {
    let needs = match reading.pages.get(&1) {
        Some(page) => inbox_page(page).0,
        None => 0,
    };
    let updates = match reading.pages.get(&5) {
        Some(page) => inbox_page(page).0,
        None => 0,
    };
    needs.checked_add(updates).expect("two bounded pages") <= rows
}
pub(super) fn inbox_cursor(reading: &Reading, rows: u32) -> Option<c::Cursor> {
    if let Some(page) = reading.pages.get(&1)
        && let Some(after) = inbox_page(page).1
    {
        return Some(group_cursor(0, Some(after)));
    }
    if !updates_fit(reading, rows) {
        return Some(group_cursor(1, None));
    }
    let page = reading.pages.get(&5)?;
    let after = inbox_page(page).1?;
    Some(group_cursor(1, Some(after)))
}
fn inbox_page(page: &Readout) -> (u32, Option<&c::Cursor>) {
    match page {
        Readout::Jig(c::ReadResult::Value(c::ReadValue::Inbox(page))) => (page.rows.len(), page.after.as_ref()),
        Readout::Jig(c::ReadResult::Value(c::ReadValue::OwnerInbox(page))) => (page.rows.len(), page.after.as_ref()),
        Readout::Limit | Readout::NotServed { .. } | Readout::Jig(_) | Readout::Family { .. } => {
            unreachable!("validated inbox page")
        }
    }
}
fn group_cursor(group: u8, after: Option<&c::Cursor>) -> c::Cursor {
    let after: &[u8] = match after {
        Some(after) => after.value.bytes(),
        None => &[],
    };
    let mut bytes = skein_lib::Writer::new(after.len().checked_add(1).expect("bounded cursor"));
    bytes.put(&[group]).expect("group byte");
    bytes.put(after).expect("whole cursor");
    c::Cursor::new(bytes.finish(), u32::MAX).expect("bounded cursor")
}
pub(super) fn object(key: c::ObjectKey, out: &mut Text) {
    match key {
        c::ObjectKey::Task(task) => {
            out.number(task);
            out.put(b" \xc2\xb7 task");
        }
        c::ObjectKey::Proposal(proposal) => {
            super::document::settings::proposal_text(proposal, out);
            out.put(b" \xc2\xb7 proposal");
        }
        c::ObjectKey::Escalation(task) => {
            out.number(task);
            out.put(b" \xc2\xb7 escalation");
        }
        c::ObjectKey::Question { task, message } => {
            out.number(task);
            out.put(b" \xc2\xb7 question ");
            out.number(message);
        }
        c::ObjectKey::PersonTask(task) => {
            out.number(task);
            out.put(b" \xc2\xb7 person task");
        }
        c::ObjectKey::Result(task) => {
            out.number(task);
            out.put(b" \xc2\xb7 result");
        }
        c::ObjectKey::Note { project, note } => {
            out.number(note);
            out.put(b" \xc2\xb7 note of project ");
            out.number(u64::from(project));
        }
    }
    out.put(b"\n");
}
