//! Supplied activity and run text; the host is the sole source of provisional words.
use super::{Text, lists, turn};
use crate::jig::{Limits, call::Call};
use alloc::boxed::Box;
use jig_client::{
    contract as c,
    thread::{Line, Thread},
};
pub(super) fn page(page: &c::ActivityPage, call: &Call, limits: &Limits, out: &mut Text) {
    let task = crate::jig::call::number(call, 0).expect("activity task");
    let rows = lists::activity_limit(call, limits.rows);
    let first = page.rows.len().saturating_sub(rows);
    for row in page.rows.as_slice().iter().skip(usize::try_from(first).expect("rows fit")) {
        line(row, task, call, limits, out);
    }
    match read_on(page, call, limits.rows) {
        Some(next) => {
            out.put(if call.tool == 3 { b"read with: " } else { b"read on with: " });
            out.put(&next);
            out.put(b"\n");
        }
        None if call.tool == 5 => out.put(b"no more\n"),
        None => {}
    }
}
pub(super) fn read_on(page: &c::ActivityPage, call: &Call, rows: u32) -> Option<Box<[u8]>> {
    if page.rows.len() > lists::activity_limit(call, rows) {
        let mut measure = Text::measure();
        replay(call, &mut measure);
        let mut out = Text::writing_exact(measure);
        replay(call, &mut out);
        return Some(out.finish());
    }
    if call.tool == 5
        && let Some(cursor) = &page.before
    {
        return Some(lists::read_on(call, 1, cursor));
    }
    None
}
fn replay(call: &Call, out: &mut Text) {
    if call.tool == 3 {
        out.put(b"task activity ");
        out.number(crate::jig::call::number(call, 0).expect("task number"));
    } else {
        let mut call = call.clone();
        *call.values.get_mut(2).expect("activity limit") = None;
        lists::write_call(&call, out);
    }
}

fn line(row: &c::ActivityLine, task: u64, call: &Call, limits: &Limits, out: &mut Text) {
    match &row.event {
        c::Activity::Turn { name, turn: value } => turn::text(task, *name, value, call, limits, out),
        c::Activity::Owner { source, decoded } => match decoded {
            Some(value) => turn::text(task, value.name, &value.turn, call, limits, out),
            None => owner(source, out),
        },
        c::Activity::Words { by, words, .. } => {
            out.number(row.position);
            out.put(b" \xc2\xb7 ");
            out.words(by.name.bytes());
            out.put(b": ");
            out.words(words.bytes());
            out.put(b"\n");
        }
        c::Activity::RunBegan { attempt, why, .. } => {
            out.number(row.position);
            out.put(b" \xc2\xb7 run ");
            out.number(*attempt);
            out.put(b" began: ");
            out.words(why.bytes());
            out.put(b"\n");
        }
        c::Activity::RunEnded { attempt, .. } => {
            out.number(row.position);
            out.put(b" \xc2\xb7 run ");
            out.number(*attempt);
            out.put(b" ended\n");
        }
        c::Activity::News { summary, .. } => {
            out.number(row.position);
            out.put(b" \xc2\xb7 news: ");
            out.words(summary.bytes());
            out.put(b"\n");
        }
        c::Activity::Result { result } => {
            out.number(row.position);
            out.put(b" \xc2\xb7 result of task ");
            out.number(result.task.task);
            out.put(b"\n");
        }
        c::Activity::Decision { .. } => {
            out.number(row.position);
            out.put(b" \xc2\xb7 decision\n");
        }
    }
}
fn owner(row: &c::OwnerActivityLine, out: &mut Text) {
    out.number(row.position);
    out.put(b" \xc2\xb7 ");
    match &row.line {
        c::OwnerActivity::Words { words, .. } => out.words(words.bytes()),
        c::OwnerActivity::Heard(word) | c::OwnerActivity::Notified { word, .. } => out.words(word.words.bytes()),
        c::OwnerActivity::RunBegan { attempt, .. } => {
            out.put(b"run ");
            out.number(*attempt);
            out.put(b" began");
        }
        c::OwnerActivity::RunEnded { attempt, .. } => {
            out.put(b"run ");
            out.number(*attempt);
            out.put(b" ended");
        }
        c::OwnerActivity::Turn { attempt, turn: number, .. } => {
            out.put(b"turn ");
            turn::name(c::TurnName { attempt: *attempt, number: *number }, out);
            out.put(b" unreadable");
        }
        c::OwnerActivity::Decided { revision } => {
            out.put(b"decision ");
            out.number(*revision);
        }
        c::OwnerActivity::Kept { .. } => out.put(b"parked"),
        c::OwnerActivity::Milestone { .. } => out.put(b"phase changed"),
    }
    out.put(b"\n");
}
pub(super) fn chat(thread: &Thread, call: &Call, limits: &Limits, out: &mut Text) {
    for entry in thread.lines().into_boxed() {
        match entry {
            Line::Turn { name, turn: value, .. } => {
                turn::text(thread.task, name, &c::TurnRead::Decoded(value.clone()), call, limits, out);
            }
            Line::Undecoded { name, why, .. } => {
                turn::text(thread.task, name, &c::TurnRead::Undecodable { why }, call, limits, out);
            }
            Line::Words { by, words, .. } => {
                if let Some(by) = by {
                    out.words(by.name.bytes());
                    out.put(b": ");
                }
                out.words(words);
                out.put(b"\n");
            }
            Line::Provisional { name, text, .. } => {
                out.put(b"provisional ");
                turn::name(name.turn(), out);
                out.put(b": ");
                out.words(text);
                out.put(b"\n");
            }
            Line::RunBegan { attempt } => {
                out.put(b"run ");
                out.number(attempt);
                out.put(b" began\n");
            }
            Line::RunEnded { attempt } => {
                out.put(b"run ");
                out.number(attempt);
                out.put(b" ended\n");
            }
            Line::Activity { line: row } => line(row, thread.task, call, limits, out),
            Line::Left { lines } => {
                out.number(u64::from(lines));
                out.put(b" lines left\n");
            }
        }
    }
    for part in thread.parts(limits.rows, thread.live.as_slice()).into_boxed() {
        turn::reference_text(&part, out);
    }
    match &thread.lines.edges.after {
        jig_client::thread::window::Edge::More(cursor) => lists::next(call, 1, Some(cursor), out),
        jig_client::thread::window::Edge::End => lists::next(call, 1, None, out),
    }
}
pub(super) fn run(delivery: &c::Delivery, text: Option<&c::RunText>, out: &mut Text) {
    out.put(b"run: ");
    match delivery {
        c::Delivery::RunState(run) => {
            match run.attempt {
                Some(attempt) => out.number(attempt),
                None => out.put(b"none"),
            }
            out.put(b"\n");
            if let Some(call) = &run.call {
                out.put(b"call: ");
                out.put(call.tool.bytes());
                out.put(b"\n");
                out.words(call.input.bytes());
                out.put(b"\n");
            }
        }
        c::Delivery::Run(run) => {
            out.number(run.attempt);
            out.put(b"\n");
        }
        c::Delivery::OwnerProject(_)
        | c::Delivery::Me(_)
        | c::Delivery::Task(_)
        | c::Delivery::OwnerProposal(_)
        | c::Delivery::Tree(_)
        | c::Delivery::OwnerTree(_)
        | c::Delivery::Chips(_)
        | c::Delivery::Inbox(_)
        | c::Delivery::OwnerInbox(_)
        | c::Delivery::Tasks(_)
        | c::Delivery::Settings(_)
        | c::Delivery::Deployment(_)
        | c::Delivery::OwnerDeployment(_)
        | c::Delivery::OwnerAccounts(_) => unreachable!("run watch delivers a run"),
    }
    if let Some(text) = text {
        out.put(b"provisional: ");
        out.words(text.text.bytes());
        out.put(b"\n");
    }
}
