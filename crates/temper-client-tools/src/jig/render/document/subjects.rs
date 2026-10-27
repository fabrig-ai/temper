//! The common subject form uses the same field encoders as ordinary reads.
use super::{Cut, Limits};
use crate::jig::render::Part;
use alloc::boxed::Box;
use skein_json::writer::{Encoder, Limits as JsonLimits};

#[must_use]
pub(crate) fn part_bytes(part: &Part<'_>, limits: &Limits) -> Box<[u8]> {
    let bounds = JsonLimits { depth: 32, length: u32::try_from(limits.result_bytes).expect("output bound fits") };
    let mut measure = Encoder::measure(&bounds);
    encode(part, limits, false, &mut measure);
    let mut out = Encoder::write(measure.measured().expect("bounded subject"), &bounds);
    encode(part, limits, false, &mut out);
    out.finish()
}
pub(crate) fn encode(part: &Part<'_>, limits: &Limits, printed: bool, out: &mut Encoder) {
    let mut digest = None;
    if printed {
        digest = Some(crate::jig::digest::part(part, limits));
    }
    encode_as(part, limits, digest, out);
}
pub(crate) fn encode_as(
    part: &Part<'_>,
    limits: &Limits,
    digest: Option<crate::jig::digest::Digest>,
    out: &mut Encoder,
) {
    let mut cut =
        Cut { room: usize::try_from(limits.text).expect("text fits"), left: 0, turn: None, subject: true, digest };
    match part {
        Part::Me(value) => super::encode_me(value, out, &mut cut, limits),
        Part::Task(value) => super::encode_task(value, out, &mut cut),
        Part::Tree(value) => super::encode_tree(value, out, &mut cut),
        Part::OwnerTree(value) => super::encode_owner_tree(value, out, &mut cut),
        Part::Run(value) => super::encode_run_text(value, out, &mut cut),
        Part::RunState(value) => super::encode_run_state(value, out, &mut cut),
        Part::Inbox(_)
        | Part::OwnerInbox(_)
        | Part::InboxPage(_)
        | Part::OwnerInboxPage(_)
        | Part::Tasks(_)
        | Part::TaskPage(_) => rows(part, limits, out, &mut cut),
    }
}
fn rows(part: &Part<'_>, limits: &Limits, out: &mut Encoder, cut: &mut Cut) {
    let most = usize::try_from(limits.rows).expect("rows fit");
    let digest = cut.digest.take();
    out.object_start();
    out.key(b"rows");
    out.array_start();
    match part {
        Part::Inbox(value) => {
            for row in value.entries.as_slice().iter().take(most) {
                super::read_inbox::encode_inbox_entry(row, out, cut);
            }
        }
        Part::OwnerInbox(value) => {
            for row in value.needs.as_slice().iter().chain(value.updates.as_slice()).take(most) {
                super::read_inbox::encode_owner_inbox_entry(row, out, cut);
            }
        }
        Part::InboxPage(value) => {
            for row in value.rows.as_slice().iter().take(most) {
                super::read_inbox::encode_inbox_entry(row, out, cut);
            }
        }
        Part::OwnerInboxPage(value) => {
            for row in value.rows.as_slice().iter().take(most) {
                super::read_inbox::encode_owner_inbox_entry(row, out, cut);
            }
        }
        Part::Tasks(value) => {
            for row in value.rows.as_slice().iter().take(most) {
                super::encode_task_line(row, out, cut);
            }
        }
        Part::TaskPage(value) => {
            for row in value.live.as_slice().iter().take(most) {
                super::encode_task_line(row, out, cut);
            }
        }
        Part::Me(_) | Part::Task(_) | Part::Tree(_) | Part::OwnerTree(_) | Part::Run(_) | Part::RunState(_) => {
            unreachable!("list subject")
        }
    }
    out.array_end();
    out.key(b"more");
    out.boolean(part.more() || part.rows() > most);
    super::subject_end(digest, out);
}

pub(crate) fn text_digest(name: &[u8], part: &Part<'_>, limits: &Limits, out: &mut crate::jig::render::Text) {
    out.put(b"digest: ");
    out.put(name);
    out.put(b" ");
    out.put(&crate::jig::digest::part(part, limits).hex());
    out.put(b"\n");
}

#[must_use]
pub(crate) fn inbox(reading: &crate::jig::runner::reads::Reading, limits: &Limits) -> jig_client::contract::ReadValue {
    use jig_client::{Readout, contract as c};
    let capacity = limits.rows.checked_mul(2).expect("two bounded pages");
    let mut entries = skein_lib::List::with_capacity(capacity);
    let mut owners = skein_lib::List::with_capacity(capacity);
    let mut owner = false;
    let fit = super::super::lists::updates_fit(reading, limits.rows);
    for (local, page) in &reading.pages {
        if *local == 5 && !fit {
            continue;
        }
        match page {
            Readout::Jig(c::ReadResult::Value(c::ReadValue::Inbox(page))) => {
                for row in &page.rows {
                    entries.push(row.clone()).expect("two bounded pages");
                }
            }
            Readout::Jig(c::ReadResult::Value(c::ReadValue::OwnerInbox(page))) => {
                owner = true;
                for row in &page.rows {
                    owners.push(row.clone()).expect("two bounded pages");
                }
            }
            Readout::Limit | Readout::NotServed { .. } | Readout::Jig(_) | Readout::Family { .. } => {
                unreachable!("validated inbox pages")
            }
        }
    }
    let after = super::super::lists::inbox_cursor(reading, limits.rows);
    if owner {
        c::ReadValue::OwnerInbox(Box::new(c::OwnerInboxPage {
            rows: owners,
            after,
            before: None,
            cut: false,
            selection: None,
        }))
    } else {
        c::ReadValue::Inbox(Box::new(c::InboxPage { rows: entries, after, before: None, cut: false }))
    }
}

#[must_use]
pub(crate) fn inbox_eligible(call: &crate::jig::call::Call) -> bool {
    for value in &call.values {
        if value.is_some() {
            return false;
        }
    }
    true
}

#[must_use]
pub(crate) fn tasks_eligible(call: &crate::jig::call::Call) -> bool {
    crate::jig::call::value(call, 4).is_none() && crate::jig::call::value(call, 6).is_none()
}

pub(crate) fn encode_inbox_read(reading: &crate::jig::runner::reads::Reading, limits: &Limits, out: &mut Encoder) {
    let normalized = crate::jig::digest::inbox(reading.documents.get(&3).expect("inbox watch"));
    let digest = crate::jig::digest::part(&normalized, limits);
    let displayed = inbox(reading, limits);
    let displayed = Part::read(&displayed).expect("inbox form");
    out.object_start();
    out.key(b"inbox");
    encode_as(&displayed, limits, Some(digest), out);
    super::question::encode(reading, out);
    out.key(b"read_on");
    match super::super::lists::inbox_cursor(reading, limits.rows) {
        Some(cursor) => out.string(&super::super::lists::read_on(&reading.called.call, 3, &cursor)),
        None => out.null(),
    }
    out.object_end();
}
