//! A call joins its selected documents before answering once.
use super::{Cut, Limits, Text};
use crate::jig::runner::reads::Reading;
use jig_client::{Readout, contract as c};
use skein_json::writer::Encoder;
use skein_ui::phrases::Phrasebook;
pub(super) fn text(
    reading: &Reading,
    me: Option<&c::Me>,
    book: &Phrasebook,
    limits: &Limits,
    vocabulary: &dyn crate::jig::render::Parts,
    out: &mut Text,
) {
    let call = &reading.called.call;
    if call.tool == 1 {
        super::super::lists::inboxes(reading, limits, book, out);
        return;
    }
    if let Some(thread) = &reading.thread {
        super::super::activity::chat(thread, call, limits, out);
        return;
    }
    for page in reading.pages.values() {
        match page {
            Readout::Jig(c::ReadResult::Value(c::ReadValue::OwnerProject(project))) => {
                super::projects::text(project, call, limits, vocabulary, out);
            }
            Readout::Jig(c::ReadResult::Value(
                value @ (c::ReadValue::Settings(_)
                | c::ReadValue::Setting(_)
                | c::ReadValue::Revision(_)
                | c::ReadValue::Revisions(_)),
            )) => super::settings::text(value, crate::jig::call::number(call, 0), limits, out),
            Readout::Jig(c::ReadResult::Value(
                value @ (c::ReadValue::Proposal(_) | c::ReadValue::OwnerProposal(_)),
            )) => super::proposal::text(value, limits, out),
            Readout::Jig(c::ReadResult::Value(c::ReadValue::Task(task))) => {
                super::task_text(task, me, book, vocabulary, out);
                vocabulary.task_next(task, out);
                super::subjects::text_digest(b"task", &super::super::Part::Task(task), limits, out);
            }
            Readout::Jig(c::ReadResult::Value(c::ReadValue::Historical(value)))
                if super::super::historical_proposal(value).is_some() =>
            {
                super::proposal::history_text(value, out);
            }
            Readout::Jig(c::ReadResult::Value(c::ReadValue::Historical(value))) => {
                let (task, result) = super::super::historical_task(value).expect("validated historical task");
                super::task_text(task, me, book, vocabulary, out);
                super::result_text(result, vocabulary, out);
                vocabulary.task_next(task, out);
                super::subjects::text_digest(b"task", &super::super::Part::Task(task), limits, out);
            }
            Readout::Jig(c::ReadResult::Value(c::ReadValue::Tasks(page))) => {
                super::super::lists::tasks(page, call, limits, book, out);
            }
            Readout::Jig(c::ReadResult::Value(c::ReadValue::Activity(page))) => {
                super::super::activity::page(page, call, limits, out);
            }
            Readout::Jig(c::ReadResult::Value(c::ReadValue::Turn(turn))) => {
                super::super::turn::text(turn.task, turn.name, &turn.turn, call, limits, out);
            }
            Readout::Jig(c::ReadResult::Value(c::ReadValue::Inbox(_) | c::ReadValue::OwnerInbox(_))) => {
                unreachable!("inbox handled above")
            }
            Readout::Limit | Readout::NotServed { .. } | Readout::Jig(_) | Readout::Family { .. } => {
                unreachable!("validated read value")
            }
        }
    }
    for (local, document) in &reading.documents {
        match local {
            3 => {
                let mut project = None;
                for page in reading.pages.values() {
                    if let Readout::Jig(c::ReadResult::Value(value)) = page
                        && let Some((task, _)) = super::super::read_task(value)
                    {
                        project = Some(task.project);
                        break;
                    }
                }
                super::super::lists::tree(document, project, book, out);
                super::subjects::text_digest(
                    b"tree",
                    &super::super::Part::delivery(document).expect("tree form"),
                    limits,
                    out,
                );
            }
            4 => {
                super::super::activity::run(document, reading.text.as_ref(), out);
                if call.tool != 4 {
                    super::subjects::text_digest(
                        b"run",
                        &super::super::Part::delivery(document).expect("run form"),
                        limits,
                        out,
                    );
                }
            }
            _ => unreachable!("selected watch"),
        }
    }
}
pub(super) fn encode(reading: &Reading, limits: &Limits, out: &mut Encoder, budget: &mut Cut) {
    let call = &reading.called.call;
    if call.tool == 1 && super::subjects::inbox_eligible(call) {
        super::subjects::encode_inbox_read(reading, limits, out);
        return;
    }
    let compound = reading.called.call.tool == 1 || reading.called.call.tool == 3 || reading.called.call.tool == 4;
    if compound {
        out.object_start();
    }
    for (local, page) in &reading.pages {
        if reading.called.call.tool == 1 && *local == 5 && !super::super::lists::updates_fit(reading, limits.rows) {
            continue;
        }
        if reading.called.call.tool == 1 {
            out.key(if *local == 1 { b"needs" } else { b"updates" });
        }
        match page {
            Readout::Jig(c::ReadResult::Value(c::ReadValue::OwnerProject(project))) => {
                super::projects::encode(project, call, limits, out, budget);
            }
            Readout::Jig(c::ReadResult::Value(
                value @ (c::ReadValue::Settings(_)
                | c::ReadValue::Setting(_)
                | c::ReadValue::Revision(_)
                | c::ReadValue::Revisions(_)),
            )) => super::settings::encode(value, out),
            Readout::Jig(c::ReadResult::Value(
                value @ (c::ReadValue::Proposal(_) | c::ReadValue::OwnerProposal(_)),
            )) => super::proposal::encode(value, out, budget),
            Readout::Jig(c::ReadResult::Value(c::ReadValue::Task(task))) => {
                if compound {
                    out.key(b"task");
                }
                super::subjects::encode(&super::super::Part::Task(task), limits, true, out);
            }
            Readout::Jig(c::ReadResult::Value(c::ReadValue::Historical(value)))
                if super::super::historical_proposal(value).is_some() =>
            {
                super::proposal::encode_history(value, out, budget);
            }
            Readout::Jig(c::ReadResult::Value(c::ReadValue::Historical(value))) => {
                let (task, result) = super::super::historical_task(value).expect("validated historical task");
                if compound {
                    out.key(b"task");
                }
                budget.digest = Some(crate::jig::digest::part(&super::super::Part::Task(task), limits));
                super::tasks::encode_historical_task(task, result, out, budget);
            }
            Readout::Jig(c::ReadResult::Value(c::ReadValue::Tasks(page))) => {
                encode_tasks(reading, page, limits, out, budget);
            }
            Readout::Jig(c::ReadResult::Value(c::ReadValue::Activity(page))) => {
                if compound {
                    out.key(b"activity");
                }
                let call = &reading.called.call;
                let task = crate::jig::call::number(call, 0).expect("activity task");
                budget.turn = Some(super::turn_window::Window::new(
                    call,
                    task,
                    c::TurnName { attempt: 0, number: 0 },
                    limits.part,
                ));
                let next = super::super::activity::read_on(page, call, limits.rows);
                super::encode_activity_page(
                    page,
                    super::super::lists::activity_limit(&reading.called.call, limits.rows),
                    next.as_deref(),
                    out,
                    budget,
                );
                budget.turn = None;
            }
            Readout::Jig(c::ReadResult::Value(c::ReadValue::Turn(turn))) => {
                encode_turn(reading, turn, limits, out, budget);
            }
            Readout::Jig(c::ReadResult::Value(c::ReadValue::Inbox(page))) => {
                super::encode_inbox_page(page, out, budget);
            }
            Readout::Jig(c::ReadResult::Value(c::ReadValue::OwnerInbox(page))) => {
                super::encode_owner_inbox_page(page, out, budget);
            }
            Readout::Limit | Readout::NotServed { .. } | Readout::Jig(_) | Readout::Family { .. } => {
                unreachable!("validated read value")
            }
        }
    }
    encode_documents(reading, limits, compound, out);
    if reading.called.call.tool == 1 {
        super::question::encode(reading, out);
        out.key(b"read_on");
        match super::super::lists::inbox_cursor(reading, limits.rows) {
            Some(cursor) => out.string(&super::super::lists::read_on(&reading.called.call, 3, &cursor)),
            None => out.null(),
        }
    }
    if compound {
        out.object_end();
    }
}

fn encode_documents(reading: &Reading, limits: &Limits, compound: bool, out: &mut Encoder) {
    for (local, document) in &reading.documents {
        if compound {
            out.key(if *local == 3 { b"tree" } else { b"run" });
        }
        match document {
            c::Delivery::Tree(tree) => super::subjects::encode(&super::super::Part::Tree(tree), limits, true, out),
            c::Delivery::OwnerTree(tree) => {
                super::subjects::encode(&super::super::Part::OwnerTree(tree), limits, true, out);
            }
            c::Delivery::RunState(_) | c::Delivery::Run(_) => {
                super::subjects::encode(
                    &super::super::Part::delivery(document).expect("run form"),
                    limits,
                    reading.called.call.tool != 4,
                    out,
                );
            }
            c::Delivery::OwnerProject(_)
            | c::Delivery::Me(_)
            | c::Delivery::Task(_)
            | c::Delivery::OwnerProposal(_)
            | c::Delivery::Chips(_)
            | c::Delivery::Inbox(_)
            | c::Delivery::OwnerInbox(_)
            | c::Delivery::Tasks(_)
            | c::Delivery::Settings(_)
            | c::Delivery::Deployment(_)
            | c::Delivery::OwnerDeployment(_)
            | c::Delivery::OwnerAccounts(_) => unreachable!("selected watch value"),
        }
    }
}

fn encode_tasks(reading: &Reading, page: &c::TaskPage, limits: &Limits, out: &mut Encoder, budget: &mut Cut) {
    let call = &reading.called.call;
    let next = super::super::lists::task_read_on(page, call);
    if super::subjects::tasks_eligible(call) {
        out.object_start();
        out.key(b"tasks");
        super::subjects::encode(&super::super::Part::TaskPage(page), limits, true, out);
        out.key(b"read_on");
        match next {
            Some(next) => out.string(&next),
            None => out.null(),
        }
        out.object_end();
    } else {
        super::encode_task_page(page, crate::jig::call::value(call, 4).is_some(), next.as_deref(), out, budget);
    }
}

fn encode_turn(reading: &Reading, turn: &c::ArchivedTurn, limits: &Limits, out: &mut Encoder, budget: &mut Cut) {
    budget.turn = Some(super::turn_window::Window::new(&reading.called.call, turn.task, turn.name, limits.part));
    super::encode_archived_turn(turn, out, budget);
    budget.turn = None;
}
