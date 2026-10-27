//! Bounded terminal bytes and integer JSON (tools contract, 6).
mod activity;
pub mod acts;
pub mod document;
pub(crate) mod holds;
pub(crate) mod lists;
pub(crate) mod outcome;
pub mod pending;
mod turn;
use crate::jig::{Limits, runner::Printed};
use alloc::boxed::Box;
use jig_client::{
    Client, Handle, Latest, Readout,
    contract::{self, Delivery, ReadNone, ReadResult, ReadValue},
};
use skein_json::writer::{Encoder, Limits as JsonLimits};
use skein_lib::Writer;
/// A subject's supplied facts, borrowed only during this step.
#[derive(Clone, Copy, Debug)]
pub enum Part<'a> {
    Me(&'a contract::Me),
    Inbox(&'a contract::Inbox),
    OwnerInbox(&'a contract::OwnerInbox),
    InboxPage(&'a contract::InboxPage),
    OwnerInboxPage(&'a contract::OwnerInboxPage),
    Tasks(&'a contract::Tasks),
    TaskPage(&'a contract::TaskPage),
    Task(&'a contract::Task),
    Tree(&'a contract::Tree),
    OwnerTree(&'a contract::OwnerTree),
    Run(&'a contract::RunText),
    RunState(&'a contract::RunState),
}
impl<'a> Part<'a> {
    #[must_use]
    pub fn read(value: &'a ReadValue) -> Option<Self> {
        match value {
            ReadValue::Inbox(value) => Some(Self::InboxPage(value)),
            ReadValue::OwnerInbox(value) => Some(Self::OwnerInboxPage(value)),
            ReadValue::Tasks(value) => Some(Self::TaskPage(value)),
            ReadValue::Task(value) => Some(Self::Task(value)),
            ReadValue::Historical(value) => Some(Self::Task(historical_task(value)?.0)),
            ReadValue::Settings(_)
            | ReadValue::Setting(_)
            | ReadValue::Revision(_)
            | ReadValue::Revisions(_)
            | ReadValue::OwnerProject(_)
            | ReadValue::Me(_)
            | ReadValue::Turn(_)
            | ReadValue::Proposal(_)
            | ReadValue::OwnerProposal(_)
            | ReadValue::Escalation(_)
            | ReadValue::OwnerEscalation(_)
            | ReadValue::Question(_)
            | ReadValue::PersonTask(_)
            | ReadValue::OwnerPersonTask(_)
            | ReadValue::Result(_)
            | ReadValue::Ended(_)
            | ReadValue::OwnerEnded(_)
            | ReadValue::History(_)
            | ReadValue::Activity(_)
            | ReadValue::Spend(_)
            | ReadValue::OwnerSpend(_)
            | ReadValue::Project(_)
            | ReadValue::Notes(_)
            | ReadValue::Deployment(_)
            | ReadValue::OwnerDeployment(_) => None,
        }
    }
    #[must_use]
    pub fn delivery(delivery: &'a Delivery) -> Option<Self> {
        match delivery {
            Delivery::Me(value) => Some(Self::Me(value)),
            Delivery::Inbox(value) => Some(Self::Inbox(value)),
            Delivery::OwnerInbox(value) => Some(Self::OwnerInbox(value)),
            Delivery::Tasks(value) => Some(Self::Tasks(value)),
            Delivery::Task(value) => Some(Self::Task(value)),
            Delivery::Tree(value) => Some(Self::Tree(value)),
            Delivery::OwnerTree(value) => Some(Self::OwnerTree(value)),
            Delivery::Run(value) => Some(Self::Run(value)),
            Delivery::RunState(value) => Some(Self::RunState(value)),
            Delivery::OwnerProject(_)
            | Delivery::OwnerProposal(_)
            | Delivery::Chips(_)
            | Delivery::Settings(_)
            | Delivery::Deployment(_)
            | Delivery::OwnerDeployment(_)
            | Delivery::OwnerAccounts(_) => None,
        }
    }
}
pub(crate) fn terminal_phase(phase: contract::Phase) -> bool {
    match phase {
        contract::Phase::Ended | contract::Phase::Done | contract::Phase::Failed | contract::Phase::Cancelled => true,
        contract::Phase::Waiting
        | contract::Phase::Idle
        | contract::Phase::Due
        | contract::Phase::Preparing
        | contract::Phase::Claimed
        | contract::Phase::Running
        | contract::Phase::BackingOff
        | contract::Phase::Active
        | contract::Phase::Closing
        | contract::Phase::Held => false,
    }
}
pub(crate) fn read_task(value: &ReadValue) -> Option<(&contract::Task, Option<&contract::Result>)> {
    if let ReadValue::Task(task) = value {
        return Some((task, None));
    }
    if let ReadValue::Historical(value) = value {
        return match historical_task(value) {
            Some((task, result)) => Some((task, Some(result))),
            None => None,
        };
    }
    None
}
pub(crate) fn historical_proposal(value: &contract::OwnerHistorical) -> Option<contract::ProposalKey> {
    match &value.value {
        contract::OwnerHistoricalValue::Proposal(_) | contract::OwnerHistoricalValue::PersonProposal { .. } => {}
        contract::OwnerHistoricalValue::Task { .. }
        | contract::OwnerHistoricalValue::Escalation { .. }
        | contract::OwnerHistoricalValue::Question { .. }
        | contract::OwnerHistoricalValue::PersonTask { .. } => return None,
    }
    if let contract::ObjectKey::Proposal(key) = value.key
        && value.matches(value.key)
    {
        return Some(key);
    }
    None
}
fn proposal_matches_call(value: &contract::OwnerHistorical, call: &crate::jig::call::Call) -> bool {
    if let Some(crate::jig::call::Value::Proposal(key)) = crate::jig::call::value(call, 0) {
        return value.matches(contract::ObjectKey::Proposal(*key));
    }
    false
}
pub(crate) fn historical_task(value: &contract::OwnerHistorical) -> Option<(&contract::Task, &contract::Result)> {
    if let contract::OwnerHistoricalValue::Task { document, result } = &value.value
        && value.matches(contract::ObjectKey::Task(document.chip.task))
    {
        return Some((document, result));
    }
    None
}
impl Part<'_> {
    #[must_use]
    pub fn rows(&self) -> usize {
        let rows = match self {
            Self::Inbox(value) => value.entries.len(),
            Self::OwnerInbox(value) => value.needs.len().checked_add(value.updates.len()).expect("bounded inbox"),
            Self::InboxPage(value) => value.rows.len(),
            Self::OwnerInboxPage(value) => value.rows.len(),
            Self::Tasks(value) => value.rows.len(),
            Self::TaskPage(value) => value.live.len(),
            Self::Tree(value) => value.nodes.len(),
            Self::OwnerTree(value) => value.rows.len(),
            Self::Me(_) | Self::Task(_) | Self::Run(_) | Self::RunState(_) => 1,
        };
        usize::try_from(rows).expect("row count fits")
    }
    #[must_use]
    pub fn more(&self) -> bool {
        match self {
            Self::Inbox(value) => value.more,
            Self::OwnerInbox(value) => value.next.is_some(),
            Self::InboxPage(value) => value.after.is_some(),
            Self::OwnerInboxPage(value) => value.after.is_some(),
            Self::Tasks(value) => value.more,
            Self::TaskPage(value) => value.live_after.is_some(),
            Self::OwnerTree(value) => value.next.is_some(),
            Self::Me(_) | Self::Task(_) | Self::Tree(_) | Self::Run(_) | Self::RunState(_) => false,
        }
    }
}
/// Application vocabulary for enum phrases, parts and changes, borrowed for this step alone.
pub trait Parts: core::fmt::Debug {
    fn diagnostic_prefix(&self) -> &'static [u8];
    fn command_prefix(&self) -> &'static [u8];
    fn phrasebook(&self, text: u32) -> skein_ui::phrases::Phrasebook;
    fn task_next(&self, task: &contract::Task, out: &mut Text);
    fn owner_part(&self, part: &contract::OwnerPart, out: &mut Text);
    fn part(&self, part: &contract::Part, out: &mut Text);
    fn change_kind(&self, connector: u16, kind: u16, out: &mut Text);
    fn change(&self, kind: u16, part: &contract::OwnerPart, words: &contract::DocumentText, out: &mut Text);
}
/// A bounded text result; only free text is cut, never identities or authority.
pub use settings_form::{Text, text_end};
fn join_limit(joins: u32) -> Box<[u8]> {
    const PREFIX: &[u8] = b"join limit: more than ";
    const SUFFIX: &[u8] = b" per read";
    let number = skein_lib::Decimal::of(u64::from(joins));
    let size = PREFIX
        .len()
        .checked_add(number.as_bytes().len())
        .expect("join reason size")
        .checked_add(SUFFIX.len())
        .expect("join reason size");
    let mut reason = Writer::new(size);
    reason.put(PREFIX).expect("measured reason");
    reason.put(number.as_bytes()).expect("measured reason");
    reason.put(SUFFIX).expect("measured reason");
    reason.finish()
}
#[must_use]
pub fn failure(reason: &[u8], json: bool, status: u8) -> Printed {
    let stderr = if json {
        let limits = JsonLimits { depth: 2, length: 4096 };
        let mut measure = Encoder::measure(&limits);
        error(&mut measure, reason);
        let mut write = Encoder::write(measure.measured().expect("bounded error"), &limits);
        error(&mut write, reason);
        write.finish()
    } else {
        let mut out = Writer::new(reason.len().checked_add(1).expect("bounded error"));
        out.put(reason).expect("measured reason");
        out.put(b"\n").expect("measured newline");
        out.finish()
    };
    Printed { file: crate::jig::FileEnd::Keep, status, stdout: Box::default(), stderr }
}
fn error(out: &mut Encoder, reason: &[u8]) {
    out.object_start();
    out.key(b"error");
    out.string(reason);
    out.object_end();
}
#[must_use]
pub fn me(me: &contract::Me, json: bool, limits: &Limits, vocabulary: &dyn Parts) -> Printed {
    document::print(document::Document::Me(me), json, limits, vocabulary)
}
#[must_use]
pub fn read(
    page: &Readout,
    client: &Client,
    me: Handle,
    json: bool,
    limits: &Limits,
    vocabulary: &dyn Parts,
) -> Printed {
    match page {
        Readout::Jig(ReadResult::Value(ReadValue::Task(task))) => {
            let party = match client.document(me) {
                Some(document) => match document.latest {
                    Some(Latest::Jig(Delivery::Me(me))) => Some(me.as_ref()),
                    _ => None,
                },
                None => None,
            };
            document::print(document::Document::Task(task, party), json, limits, vocabulary)
        }
        Readout::Jig(ReadResult::Value(ReadValue::Historical(value))) if historical_task(value).is_some() => {
            let (task, result) = historical_task(value).expect("matched historical task");
            let party = match client.document(me) {
                Some(document) => match document.latest {
                    Some(Latest::Jig(Delivery::Me(me))) => Some(me.as_ref()),
                    _ => None,
                },
                None => None,
            };
            document::print(document::Document::HistoricalTask(task, result, party), json, limits, vocabulary)
        }
        Readout::Jig(ReadResult::None(why)) => {
            let (reason, status): (&[u8], u8) = match why {
                ReadNone::Refused => (b"refused", 2),
                ReadNone::Unknown => (b"unknown", 2),
                ReadNone::Gone => (b"gone", 2),
                ReadNone::Busy => (b"busy", 1),
                ReadNone::SignedOut => (b"signed out", 1),
                ReadNone::Unreachable => (b"unreachable", 1),
                ReadNone::Malformed => (b"malformed document", 1),
                ReadNone::TooLinked { joins } => return failure(&join_limit(*joins), json, 1),
                ReadNone::TooLarge { .. } => (b"document limit", 1),
            };
            failure(reason, json, status)
        }
        Readout::Limit => failure(b"read table limit", json, 1),
        Readout::NotServed { .. } => failure(b"version not served", json, 1),
        Readout::Family { .. } | Readout::Jig(ReadResult::Value(_)) => {
            failure(b"tools' bug; unexpected document; report it as a finding", json, 1)
        }
    }
}

#[must_use]
pub(crate) fn reads(
    reading: &crate::jig::runner::reads::Reading,
    client: &Client,
    me: Handle,
    limits: &Limits,
    vocabulary: &dyn Parts,
) -> Printed {
    let party = match client.document(me) {
        Some(document) => match document.latest {
            Some(Latest::Jig(Delivery::Me(me))) => Some(me.as_ref()),
            _ => None,
        },
        None => None,
    };
    for (local, page) in &reading.pages {
        if reading.called.call.tool == crate::jig::rows::PROPOSAL_AT && *local >= crate::jig::runner::proposal::FIRST {
            continue;
        }
        match page {
            Readout::Jig(ReadResult::Value(ReadValue::Revision(answer)))
                if reading.called.call.tool == crate::jig::rows::REVISION_AT
                    && *answer.as_ref() == contract::RevisionAnswer::Unknown =>
            {
                return failure(b"unknown revision", reading.called.json, 2);
            }
            Readout::Jig(ReadResult::Value(value)) => {
                if check_selected(&reading.called.call, *local, value).is_err() {
                    return failure(b"tools' bug; unexpected document; report it as a finding", reading.called.json, 1);
                }
            }
            Readout::Jig(ReadResult::None(ReadNone::TooLarge { bytes })) if reading.called.call.tool == 6 => {
                let Some(crate::jig::call::Value::Turn(name)) = crate::jig::call::value(&reading.called.call, 1) else {
                    unreachable!("turn call")
                };
                let task = crate::jig::call::number(&reading.called.call, 0).expect("turn task");
                return document::print(
                    document::Document::SizedTurn { task, name: *name, bytes: *bytes },
                    reading.called.json,
                    limits,
                    vocabulary,
                );
            }
            Readout::Limit | Readout::NotServed { .. } | Readout::Jig(_) | Readout::Family { .. } => {
                return read(page, client, me, reading.called.json, limits, vocabulary);
            }
        }
    }
    if reading.called.call.tool == 22 || reading.called.call.tool == 23 {
        let current = match client.document(me) {
            Some(document) => document.state == jig_client::State::Live,
            None => false,
        };
        if !current {
            return failure(b"me is not current", reading.called.json, 1);
        }
        let Some(Delivery::OwnerAccounts(accounts)) = reading.documents.get(&3) else {
            return failure(b"accounts document missing", reading.called.json, 1);
        };
        let deployment = match reading.documents.get(&4) {
            Some(Delivery::OwnerDeployment(value)) => Some(value.as_ref()),
            _ => None,
        };
        let models = if reading.called.call.tool == 23 {
            let Some(party) = party else { return failure(b"me document missing", reading.called.json, 1) };
            let Ok(project) = crate::jig::runner::reads::model_project(&reading.called.call, party) else {
                return failure(b"unknown project", reading.called.json, 1);
            };
            Some((party, project))
        } else {
            None
        };
        return document::print(
            document::Document::Accounts(document::accounts::Accounts {
                party: Some(accounts),
                deployment,
                models,
                outcome: None,
                current: true,
            }),
            reading.called.json,
            limits,
            vocabulary,
        );
    }
    if reading.called.call.tool == 3 && reading.pages.len() == 1 && reading.documents.is_empty() {
        return read(reading.pages.get(&1).expect("task read"), client, me, reading.called.json, limits, vocabulary);
    }
    document::print(document::Document::ReadSet(reading, party), reading.called.json, limits, vocabulary)
}

fn check_selected(call: &crate::jig::call::Call, local: u32, value: &ReadValue) -> Result<(), ()> {
    match (call.tool, local, value) {
        (crate::jig::rows::PROJECT_SHOW_AT | crate::jig::rows::PROJECT_FUND_AT, 1, ReadValue::OwnerProject(_)) => {
            Ok(())
        }
        (1, _, ReadValue::Inbox(_) | ReadValue::OwnerInbox(_))
        | (2, 1, ReadValue::Tasks(_))
        | (3, 1, ReadValue::Task(_))
        | (3, 2, ReadValue::Activity(_))
        | (4 | 5, 1, ReadValue::Activity(_))
        | (6, 1, ReadValue::Turn(_))
        | (crate::jig::rows::SETTINGS_AT, 1, ReadValue::Settings(_))
        | (crate::jig::rows::SETTING_AT, 1, ReadValue::Setting(_))
        | (crate::jig::rows::REVISION_AT, 1, ReadValue::Revision(_))
        | (crate::jig::rows::REVISIONS_AT, 1, ReadValue::Revisions(_))
        | (crate::jig::rows::PROPOSAL_AT, 1, ReadValue::Proposal(_) | ReadValue::OwnerProposal(_)) => Ok(()),
        (3, 1, ReadValue::Historical(value)) if historical_task(value).is_some() => Ok(()),
        (crate::jig::rows::PROPOSAL_AT, 1, ReadValue::Historical(value))
            if historical_proposal(value).is_some() && proposal_matches_call(value, call) =>
        {
            Ok(())
        }
        _ => Err(()),
    }
}

#[must_use]
pub(crate) fn waiting(
    mut wait: document::waiting::Waiting<'_>,
    json: bool,
    limits: &Limits,
    vocabulary: &dyn Parts,
) -> Printed {
    if wait.digest.is_none()
        && let Some(delivery) = wait.delivery
    {
        wait.digest = Some(crate::jig::digest::part(&Part::delivery(delivery).expect("subject form"), limits));
    }
    let met = wait.met;
    let mut printed = document::print(document::Document::Wait(wait), json, limits, vocabulary);
    if printed.status == 0 && !met {
        printed.status = 3;
    }
    printed
}

/// The engine's unsaved admission refusal (client/tools.md, 6.3).
pub(crate) fn answers_full(prefix: &[u8], held: u32, room_at: skein_lib::Wall) -> Printed {
    fn line(prefix: &[u8], held: u32, room_at: skein_lib::Wall, out: &mut Text) {
        out.put(prefix);
        out.put(b": the engine holds its ");
        out.number(u64::from(held));
        out.put(b" answers of the last day; room again at ");
        document::clock::deadline(room_at, out);
        out.put(b"\n");
    }
    let mut measured = Text::measure();
    line(prefix, held, room_at, &mut measured);
    let mut write = Text::writing_exact(measured);
    line(prefix, held, room_at, &mut write);
    Printed { file: crate::jig::FileEnd::Keep, status: 2, stdout: Box::default(), stderr: write.finish() }
}

/// Render a settings read with ctl's words and layout, before host pagination.
#[must_use]
pub fn settings_text(value: &ReadValue, requested: Option<u64>, limits: &Limits) -> Box<[u8]> {
    let mut measure = Text::measure();
    document::settings::text(value, requested, limits, &mut measure);
    let structural = measure.structural();
    let mut measure = Text::bounded(limits.text, structural);
    document::settings::text(value, requested, limits, &mut measure);
    let mut out = measure.writing(limits.text, structural);
    document::settings::text(value, requested, limits, &mut out);
    out.finish()
}

/// Render a proposal refusal with the same words as ctl.
#[must_use]
pub fn settings_refusal(why: &contract::Refusal, text: u32) -> Box<[u8]> {
    let book = document::phrasebook(text);
    let mut measure = Text::measure();
    acts::refusal(why, &book, &mut measure);
    let mut out = Text::writing_exact(measure);
    acts::refusal(why, &book, &mut out);
    out.finish()
}
