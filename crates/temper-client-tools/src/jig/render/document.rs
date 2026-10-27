//! Step-local document rendering; no borrowed document survives a step.
mod account_lines;
mod account_programs;
mod proposal;
pub(crate) mod settings;
use super::{Text, acts};
use crate::jig::{Limits, runner::Printed};
use alloc::boxed::Box;
use jig_client::{
    contract::{HeldRole, Me, OwnerRequester, Recipient, Task, WaitedFor, WaitsOn, Whom},
    words::{self, Word},
};
use skein_json::writer::Encoder;
use skein_ui::phrases::{PhraseLimits, Phrasebook};
#[derive(Debug)]
pub(crate) enum Document<'a> {
    Me(&'a Me),
    Accounts(accounts::Accounts<'a>),
    Task(&'a Task, Option<&'a Me>),
    HistoricalTask(&'a Task, &'a jig_client::contract::Result, Option<&'a Me>),
    ReadSet(&'a crate::jig::runner::reads::Reading, Option<&'a Me>),
    Wait(waiting::Waiting<'a>),
    SizedTurn { task: u64, name: jig_client::contract::TurnName, bytes: u64 },
}
#[must_use]
pub(crate) fn print(
    document: Document<'_>,
    json: bool,
    limits: &Limits,
    vocabulary: &dyn crate::jig::render::Parts,
) -> Printed {
    let accounts = match document {
        Document::Accounts(_) => true,
        Document::Me(_)
        | Document::Task(..)
        | Document::HistoricalTask(..)
        | Document::ReadSet(..)
        | Document::Wait(_)
        | Document::SizedTurn { .. } => false,
    };
    let settings = match &document {
        Document::ReadSet(reading, _) => {
            reading.called.call.tool >= crate::jig::rows::SETTINGS_AT
                && reading.called.call.tool <= crate::jig::rows::REVISIONS_AT
                || reading.called.call.tool == crate::jig::rows::PROPOSAL_AT
        }
        Document::Me(_)
        | Document::Accounts(_)
        | Document::Task(..)
        | Document::HistoricalTask(..)
        | Document::Wait(_)
        | Document::SizedTurn { .. } => false,
    };
    if settings {
        let bounds = skein_json::writer::Limits {
            depth: 32,
            length: u32::try_from(limits.result_bytes).expect("output bound fits"),
        };
        let mut measured = Encoder::measure(&bounds);
        encode(&document, limits, subject_digest(&document, limits), &mut measured);
        if measured.measured().is_err() {
            return super::failure(b"result limit", json, 1);
        }
    }
    let stdout = if json {
        let digest = subject_digest(&document, limits);
        let bounds = skein_json::writer::Limits {
            depth: 32,
            length: if accounts { limits.text } else { u32::try_from(limits.result_bytes).expect("output bound fits") },
        };
        let mut measure = Encoder::measure(&bounds);
        encode(&document, limits, digest, &mut measure);
        let Ok(size) = measure.measured() else { return super::failure(b"result limit", true, 1) };
        let mut write = Encoder::write(size, &bounds);
        encode(&document, limits, digest, &mut write);
        write.finish()
    } else {
        let book = phrasebook(limits.text);
        let mut measure = Text::measure();
        text(&document, &book, limits, vocabulary, &mut measure);
        let structural = measure.structural();
        if settings && u64::try_from(structural).unwrap_or(u64::MAX) > limits.result_bytes {
            return super::failure(b"result limit", false, 1);
        }
        if accounts && structural > usize::try_from(limits.text).expect("text fits") {
            return super::failure(b"result limit", false, 1);
        }
        let mut measure = Text::bounded(limits.text, structural);
        text(&document, &book, limits, vocabulary, &mut measure);
        let mut out = measure.writing(limits.text, structural);
        text(&document, &book, limits, vocabulary, &mut out);
        out.finish()
    };
    Printed { file: crate::jig::FileEnd::Keep, status: 0, stdout, stderr: Box::default() }
}
fn text(
    document: &Document<'_>,
    book: &Phrasebook,
    limits: &Limits,
    vocabulary: &dyn crate::jig::render::Parts,
    out: &mut Text,
) {
    match document {
        Document::Wait(wait) => waiting::text(wait, book, limits, vocabulary, out),
        Document::Accounts(value) => accounts::text(*value, limits, out),
        Document::Me(me) => me_text(me, book, limits, out),
        Document::Task(task, me) => {
            task_text(task, *me, book, vocabulary, out);
            vocabulary.task_next(task, out);
        }
        Document::HistoricalTask(task, result, me) => {
            task_text(task, *me, book, vocabulary, out);
            result_text(result, vocabulary, out);
            vocabulary.task_next(task, out);
        }
        Document::ReadSet(reading, me) => read_sets::text(reading, *me, book, limits, vocabulary, out),
        Document::SizedTurn { task, name, bytes } => super::turn::sized(*task, *name, *bytes, true, out),
    }
    match document {
        Document::Me(me) => subjects::text_digest(b"me", &super::Part::Me(me), limits, out),
        Document::Task(task, _) | Document::HistoricalTask(task, _, _) => {
            subjects::text_digest(b"task", &super::Part::Task(task), limits, out);
        }
        Document::Accounts(_) | Document::ReadSet(..) | Document::SizedTurn { .. } | Document::Wait(_) => {}
    }
}
pub(crate) fn phrasebook(text: u32) -> Phrasebook {
    let shapes = words::phrase_shapes(text);
    let limits = PhraseLimits {
        languages: 1,
        phrases: words::PHRASES,
        holes: 3,
        segments: words::PHRASES.checked_mul(8).expect("phrase segments"),
        text_bytes: 16_384,
        hole_bytes: text,
    };
    Phrasebook::load(shapes, Box::new([words::english()]), &limits).expect("checked jig phrases")
}
fn me_text(me: &Me, book: &Phrasebook, limits: &Limits, out: &mut Text) {
    out.put(b"party ");
    out.number(me.party.number);
    out.put(b" \xc2\xb7 ");
    out.words(me.party.login.bytes());
    out.put(b" \xc2\xb7 ");
    out.words(me.party.name.bytes());
    out.put(b"\nprojects:\n");
    for project in &me.projects {
        out.put(b"  ");
        out.number(u64::from(project.project));
        out.put(b" \xc2\xb7 ");
        out.words(project.name.bytes());
        out.put(b" \xc2\xb7 ");
        role(project.role, out);
        if let Some(pool) = &project.pool {
            out.put(b" \xc2\xb7 pool left ");
            out.number(pool.left);
        }
        out.put(b"\n");
        if let Some(creating) = &project.creating {
            out.put(b"    creating within ");
            acts::create_bounds(&creating.within, out);
            match &creating.beyond {
                jig_client::contract::Beyond::Proposed { to } => acts::proposed(to, Some(me), out),
                jig_client::contract::Beyond::Refused(why) => {
                    out.put(b"; refused beyond them: ");
                    acts::refusal(why, book, out);
                }
            }
            out.put(b"\n");
        }
    }
    out.put(b"needs: ");
    out.number(u64::from(me.needs_total));
    out.put(b" \xc2\xb7 questions ");
    out.number(u64::from(me.needs.questions));
    out.put(b" \xc2\xb7 proposals ");
    out.number(u64::from(me.needs.proposals));
    out.put(b" \xc2\xb7 escalations ");
    out.number(u64::from(me.needs.escalations));
    out.put(b" \xc2\xb7 person tasks ");
    out.number(u64::from(me.needs.person_tasks));
    out.put(b"\nupdates: ");
    out.number(u64::from(me.updates.value));
    if me.updates.limited {
        out.put(b"+");
    }
    out.put(b"\nretention: ");
    out.number(me.retention.as_nanos());
    out.put(b" ns\n");
    offers::text(me, limits, out);
}
fn role(role: HeldRole, out: &mut Text) {
    match role {
        HeldRole::Owner => out.put(b"owner"),
        HeldRole::Maintainer => out.put(b"maintainer"),
        HeldRole::Member => out.put(b"member"),
        HeldRole::Observer => out.put(b"observer"),
        HeldRole::Policy { role } => {
            out.put(b"role ");
            out.number(u64::from(role));
        }
    }
}
fn task_text(
    task: &Task,
    me: Option<&Me>,
    book: &Phrasebook,
    vocabulary: &dyn crate::jig::render::Parts,
    out: &mut Text,
) {
    out.put(b"task ");
    out.number(task.chip.task);
    out.put(b" \xc2\xb7 ");
    match task.flags.conversation {
        true => out.put(b"chat"),
        false if task.goal == Some(task.chip.task) => out.put(b"goal"),
        false => match task.result_contract {
            jig_client::contract::TaskContractDocument::Approval { .. } => out.put(b"approval"),
            jig_client::contract::TaskContractDocument::Report { .. } => out.put(b"report"),
            jig_client::contract::TaskContractDocument::Verdict { .. } => out.put(b"verdict"),
            jig_client::contract::TaskContractDocument::Change { connector, kind, .. } => {
                vocabulary.change_kind(connector, kind, out);
            }
        },
    }
    out.put(b" \xc2\xb7 ");
    out.put(&book.say(0, Word::Phase(task.chip.phase).id(), &[]).expect("phase phrase").text);
    out.put(b" \xc2\xb7 ");
    project(task.project, me, out);
    if let Some(priority) = task.priority {
        out.put(b" \xc2\xb7 priority ");
        out.number(u64::from(priority));
    } else {
        out.put(b" \xc2\xb7 untracked");
    }
    out.put(b"\nasked: ");
    out.words(task.spec.specification.words.bytes());
    out.put(b"\nrequester: ");
    requester(task.requester, me, out);
    out.put(b" \xc2\xb7 spent ");
    amount(task.budget.numbers.spent, out);
    out.put(b" of ");
    amount(task.budget.numbers.budget, out);
    match task.budget.deadline {
        None => out.put(b" \xc2\xb7 no deadline"),
        Some(wall) => {
            out.put(b" \xc2\xb7 deadline ");
            clock::deadline(wall, out);
        }
    }
    if let Some(instructions) = &task.instructions {
        instruction_words::text(instructions, out);
    }
    task_settings::text(task.settings.as_ref(), out);
    dependencies::text(task, me, book, out);
    llm::text_wait(task.run_wait, out);
    out.put(b"\nwaiting on: ");
    waiting(&task.waits_on, me, book, out);
    out.put(b"\n");
    if let Some(context) = &task.context {
        out.put(b"plan: ");
        out.number(
            u64::from(context.impact.tasks.running)
                .checked_add(u64::from(context.impact.tasks.waiting))
                .expect("plan counts fit")
                .checked_add(u64::from(context.impact.tasks.held))
                .expect("plan counts fit"),
        );
        out.put(b" live (");
        out.number(u64::from(context.impact.tasks.running));
        out.put(b" running, ");
        out.number(u64::from(context.impact.tasks.waiting));
        out.put(b" waiting, ");
        out.number(u64::from(context.impact.tasks.held));
        out.put(b" held below)\n");
    }
    for resource in &task.resources {
        if let jig_client::contract::TaskResource::Owner(resource) = resource {
            out.put(b"resource: ");
            vocabulary.owner_part(&resource.part, out);
            out.put(b"\n");
        }
    }
    acts::text(task, me, book, out);
}
fn requester(value: OwnerRequester, me: Option<&Me>, out: &mut Text) {
    match value {
        OwnerRequester::Person(party) => acts::party(party, me, out),
        OwnerRequester::Task(task) => {
            out.put(b"task ");
            out.number(task);
        }
        OwnerRequester::Deployment { project } => {
            out.put(b"deployment ");
            out.number(u64::from(project));
        }
    }
}
fn project(number: u32, me: Option<&Me>, out: &mut Text) {
    if let Some(me) = me {
        for project in &me.projects {
            if project.project == number {
                out.words(project.name.bytes());
                return;
            }
        }
    }
    out.put(b"project ");
    out.number(u64::from(number));
}
fn waiting(value: &WaitsOn, me: Option<&Me>, book: &Phrasebook, out: &mut Text) {
    match value {
        WaitsOn::Party { whom, task, kind, entries } => {
            match whom {
                Whom::Person(party) => acts::party(*party, me, out),
                Whom::Role { project, role } => {
                    acts::recipient(&Recipient::Role { project: *project, role: *role }, me, out);
                }
            }
            out.put(b": ");
            match kind {
                WaitedFor::Question => out.put(b"question"),
                WaitedFor::Proposal => out.put(b"proposal"),
                WaitedFor::Escalation => out.put(b"escalation"),
                WaitedFor::PersonTask => out.put(b"person task"),
            }
            out.put(b" of task ");
            out.number(*task);
            out.put(b" \xc2\xb7 ");
            out.number(u64::from(*entries));
            out.put(b" waiting");
        }
        WaitsOn::Dependencies { .. } | WaitsOn::Delegates | WaitsOn::News | WaitsOn::Nothing => {
            out.put(&book.say(0, Word::waiting(value).id(), &[]).expect("waiting phrase").text);
        }
    }
}
fn amount(value: u64, out: &mut Text) {
    out.number(value / 100);
    out.put(b".");
    if value % 100 < 10 {
        out.put(b"0");
    }
    out.number(value % 100);
}
pub(super) fn utc(wall: skein_lib::Wall, out: &mut Text) {
    let minute = wall.as_secs() / 60;
    let hour = (minute / 60) % 24;
    if hour < 10 {
        out.put(b"0");
    }
    out.number(hour);
    out.put(b":");
    if minute % 60 < 10 {
        out.put(b"0");
    }
    out.number(minute % 60);
}
struct Cut {
    room: usize,
    left: u64,
    turn: Option<turn_window::Window>,
    subject: bool,
    digest: Option<crate::jig::digest::Digest>,
}
impl Cut {
    fn words(&mut self, bytes: &[u8], out: &mut Encoder) {
        let end = super::text_end(bytes, self.room);
        self.room = self.room.checked_sub(end).expect("within text budget");
        self.left = self
            .left
            .checked_add(u64::try_from(bytes.len().checked_sub(end).expect("within value")).expect("byte count fits"))
            .expect("within document budget");
        out.string(bytes.get(..end).expect("UTF-8 cut"));
    }
}
fn hex(bytes: &[u8], out: &mut Encoder) {
    let mut hex = skein_lib::Writer::new(bytes.len().checked_mul(2).expect("document bound fits"));
    for byte in bytes {
        for digit in [byte >> 4u32, byte & 15] {
            hex.put(&[if digit < 10 {
                b'0'.checked_add(digit).expect("digit")
            } else {
                b'a'.checked_add(digit.checked_sub(10).expect("hex letter")).expect("letter")
            }])
            .expect("measured hex");
        }
    }
    out.string(&hex.finish());
}
fn subject_digest(document: &Document<'_>, limits: &Limits) -> Option<crate::jig::digest::Digest> {
    match document {
        Document::Me(me) => Some(crate::jig::digest::part(&super::Part::Me(me), limits)),
        Document::Task(task, _) | Document::HistoricalTask(task, _, _) => {
            Some(crate::jig::digest::part(&super::Part::Task(task), limits))
        }
        Document::Accounts(_) | Document::ReadSet(..) | Document::Wait(_) | Document::SizedTurn { .. } => None,
    }
}
fn encode(document: &Document<'_>, limits: &Limits, digest: Option<crate::jig::digest::Digest>, out: &mut Encoder) {
    let mut cut = Cut {
        room: usize::try_from(limits.text).expect("text limit fits"),
        left: 0,
        turn: None,
        subject: false,
        digest: None,
    };
    match document {
        Document::Wait(wait) => waiting::encode(wait, limits, out),
        Document::Accounts(value) => accounts::encode(*value, limits, true, out),
        Document::Me(me) => subjects::encode_as(&super::Part::Me(me), limits, digest, out),
        Document::Task(task, _) => subjects::encode_as(&super::Part::Task(task), limits, digest, out),
        Document::HistoricalTask(task, result, _) => {
            cut.digest = digest;
            tasks::encode_historical_task(task, result, out, &mut cut);
        }
        Document::ReadSet(reading, _) => read_sets::encode(reading, limits, out, &mut cut),
        Document::SizedTurn { task, name, bytes } => {
            out.object_start();
            out.key(b"task");
            out.unsigned(*task);
            out.key(b"name");
            encode_turn_name(name, out, &mut cut);
            out.key(b"bytes");
            out.unsigned(*bytes);
            out.key(b"error");
            out.string(b"past the limit; report it");
            out.object_end();
        }
    }
}
mod admissions;
mod bounds;
mod chips;
mod creation;
mod endings;
mod hold_reasons;
mod hold_states;
mod holds;
mod limits;
mod llm;
mod me;
mod offers;
mod outcomes;
mod owner_authority;
mod owner_chips;
mod owner_links;
mod owner_resources;
mod parties;
mod parts;
mod results;
mod runs;
mod runtime;
mod task_fields;
mod tasks;
mod templates;
use admissions::{encode_act_kind, encode_active_run, encode_actor, encode_admission};
use bounds::{
    encode_admission_limit, encode_attempt_failure, encode_beyond, encode_bounds, encode_budget, encode_call_identity,
};
use chips::{encode_chip, encode_closing, encode_closing_stage};
use creation::{
    encode_count, encode_create_bounds, encode_creating, encode_current_hold, encode_decision,
    encode_document_specification, encode_document_text, encode_effect,
};
use endings::{encode_ending, encode_escalation, encode_executor, encode_failure_class, encode_field_rule};
use hold_reasons::{encode_hold_reason, encode_impact};
use hold_states::{encode_hold_category, encode_hold_failure};
use holds::{encode_funder, encode_grant, encode_held_role, encode_held_runtime, encode_hold};
use limits::{encode_lacking, encode_limit};
use me::{encode_live_delegate, encode_malformed, encode_me, encode_moved, encode_offered, encode_owner_authority};
use outcomes::{encode_proposer, encode_recipient, encode_refusal, encode_release_effect, encode_result};
use owner_authority::{
    encode_owner_authority_document, encode_owner_authority_executor, encode_owner_budget, encode_owner_chip_details,
    encode_owner_chip_ending, encode_owner_chip_hold,
};
use owner_chips::{
    encode_owner_chip_phase, encode_owner_chip_work, encode_owner_delegation, encode_owner_escalation_holder,
    encode_owner_executor, encode_owner_failure_tries,
};
use owner_links::{
    encode_owner_person_wait, encode_owner_prior_phase, encode_owner_requester, encode_owner_resource_line,
    encode_owner_resource_scope, encode_owner_task_link,
};
use owner_resources::{
    encode_owner_grant, encode_owner_hold_escalation, encode_owner_kind_implication, encode_owner_numbers,
    encode_owner_part, encode_owner_pattern, encode_owner_person_stall_notice,
};
use parties::{
    encode_part_scope, encode_party, encode_party_kind, encode_passed, encode_person_address, encode_phase,
    encode_phase_counts, encode_pool_ledger, encode_proposal_key,
};
use parts::{encode_owner_wake_policy, encode_parameter_document, encode_part};
use results::{encode_result_details, encode_result_field, encode_result_value, encode_results_wake, encode_retention};
use runs::{encode_retry_notice, encode_revision, encode_run_observation, encode_run_stage};
use runtime::{encode_runtime, encode_specification_document, encode_spend_line, encode_steering, encode_subscription};
use task_fields::{
    encode_task_contract_document, encode_task_flags, encode_task_hold, encode_task_resource, encode_task_result,
    encode_template,
};
use tasks::{encode_subscription_kind, encode_taken_by, encode_task};
use templates::{
    encode_template_deadline, encode_template_executor, encode_verdict_choice, encode_waits_on, encode_wake_policy,
    encode_wake_rule, encode_writer_slot,
};

mod read_activity;
use read_activity::{encode_activity_page, encode_archived_turn};

mod read_turns;
use read_turns::{
    encode_current_call, encode_live_call, encode_provisional_turn_name, encode_turn_name, encode_turn_read,
};

mod read_queries;
use read_queries::{encode_cursor, encode_owner_phase_counts, encode_task_line, encode_task_page};

mod read_inbox;
use read_inbox::{encode_inbox_page, encode_owner_inbox_page};

mod read_owner_activity;
use read_owner_activity::encode_owner_activity_line;

mod read_owner_words;
use read_owner_words::encode_owner_message_kind;

mod read_watches;
use read_watches::{
    encode_object_key, encode_owner_tree, encode_run_beginning, encode_run_ending, encode_run_state, encode_run_text,
    encode_tree,
};

mod read_sets;

mod turn_window;

pub(super) fn part(part: &jig_client::contract::Part, out: &mut Text) {
    let limits = skein_json::writer::Limits { depth: 32, length: u32::MAX };
    let mut budget = Cut { room: 0, left: 0, turn: None, subject: false, digest: None };
    let mut measure = Encoder::measure(&limits);
    encode_part(part, &mut measure, &mut budget);
    let size = measure.measured().expect("bounded part identity");
    let mut write = Encoder::write(size, &limits);
    encode_part(part, &mut write, &mut budget);
    out.put(&write.finish());
}

mod act_answers;
use act_answers::encode_answer;
pub(super) fn answer_json(answer: &jig_client::contract::Answer, out: &mut Encoder) {
    let mut budget = Cut { room: usize::MAX, left: 0, turn: None, subject: false, digest: None };
    encode_answer(answer, out, &mut budget);
}
pub(super) fn recipient_json(to: &Recipient, out: &mut Encoder) {
    let mut budget = Cut { room: usize::MAX, left: 0, turn: None, subject: false, digest: None };
    encode_recipient(to, out, &mut budget);
}

pub(crate) fn refusal_json(refusal: &jig_client::contract::Refusal, out: &mut Encoder) {
    let mut budget = Cut { room: usize::MAX, left: 0, turn: None, subject: false, digest: None };
    encode_refusal(refusal, out, &mut budget);
}

pub(crate) mod subjects;
pub(crate) use subjects::part_bytes;

fn subject_end(digest: Option<crate::jig::digest::Digest>, out: &mut Encoder) {
    if let Some(digest) = digest {
        out.key(b"digest");
        out.string(&digest.hex());
    }
    out.object_end();
}

pub(crate) mod waiting;

pub(crate) mod accounts;

mod clock;
mod dependencies;
mod instruction_words;
mod result_words;
mod task_settings;
use result_words::text as result_text;
