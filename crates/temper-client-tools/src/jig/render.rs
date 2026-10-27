//! Bounded terminal bytes and integer JSON (tools contract, 6).
mod activity;
pub mod acts;
pub mod document;
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
    fn phrasebook(&self, text: u32) -> skein_ui::phrases::Phrasebook;
    fn task_next(&self, task: &contract::Task, out: &mut Text);
    fn owner_part(&self, part: &contract::OwnerPart, out: &mut Text);
    fn part(&self, part: &contract::Part, out: &mut Text);
    fn change_kind(&self, connector: u16, kind: u16, out: &mut Text);
    fn change(&self, kind: u16, part: &contract::OwnerPart, words: &contract::DocumentText, out: &mut Text);
}
/// A bounded text result; only free text is cut, never identities or authority.
#[derive(Debug)]
pub struct Text {
    writer: Option<Writer>,
    room: usize,
    written: usize,
    left: usize,
}
impl Text {
    pub(crate) fn measure() -> Self {
        Self { writer: None, room: 0, written: 0, left: 0 }
    }
    pub(crate) fn structural(&self) -> usize {
        self.written
    }
    pub(crate) fn bounded(limits: &Limits, structural: usize) -> Self {
        let limit = usize::try_from(limits.text).expect("text bound fits");
        let room = limit.saturating_sub(structural).saturating_sub(64);
        Self { writer: None, room, written: 0, left: 0 }
    }
    pub(crate) fn writing(mut self, limits: &Limits, structural: usize) -> Self {
        self.cut_line();
        let mut output = Self::bounded(limits, structural);
        output.writer = Some(Writer::new(self.written));
        output
    }
    pub(crate) fn writing_exact(measured: Self) -> Self {
        let mut out = Self::measure();
        out.writer = Some(Writer::new(measured.written));
        out
    }
    pub fn put(&mut self, bytes: &[u8]) {
        self.written = self.written.checked_add(bytes.len()).expect("document bound");
        if let Some(writer) = &mut self.writer {
            writer.put(bytes).expect("measured document capacity");
        }
    }
    pub fn number(&mut self, mut number: u64) {
        let mut bytes = [0u8; 20];
        let mut start = bytes.len();
        for _ in 0..20u8 {
            start = start.checked_sub(1).expect("decimal digit slot");
            *bytes.get_mut(start).expect("decimal digit slot") =
                b'0'.checked_add(u8::try_from(number % 10).expect("digit fits")).expect("ASCII digit");
            number /= 10;
            if number == 0 {
                break;
            }
        }
        self.put(bytes.get(start..).expect("decimal range"));
    }
    pub fn words(&mut self, bytes: &[u8]) {
        let end = text_end(bytes, self.room);
        self.room = self.room.checked_sub(end).expect("within text budget");
        self.put(bytes.get(..end).expect("text range"));
        self.left = self
            .left
            .checked_add(bytes.len().checked_sub(end).expect("text cut within value"))
            .expect("document bound");
    }
    /// A result's words follow its state, with multiline words indented within the text budget.
    pub fn result_words(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        if !bytes.contains(&b'\n') {
            self.put(b" \xc2\xb7 ");
            self.words(bytes);
            return;
        }
        self.put(b"\n");
        let mut from = 0;
        for (at, byte) in bytes.iter().chain(core::iter::once(&b'\n')).enumerate() {
            if *byte != b'\n' {
                continue;
            }
            if from != 0 {
                self.words(b"\n");
            }
            let line = bytes.get(from..at).expect("line within words");
            from = at.checked_add(1).expect("document bound");
            if self.room < 2 {
                self.left = self.left.checked_add(line.len()).expect("document bound");
                continue;
            }
            self.room = self.room.checked_sub(2).expect("indent fits text budget");
            self.put(b"  ");
            self.words(line);
        }
    }
    #[must_use]
    pub fn finish(mut self) -> Box<[u8]> {
        self.cut_line();
        self.writer.expect("only written output is finished").finish()
    }
    fn cut_line(&mut self) {
        if self.left > 0 {
            self.put(b"\n");
            self.number(u64::try_from(self.left).expect("byte count fits"));
            self.put(b" bytes left\n");
        }
    }
}
pub(crate) fn text_end(bytes: &[u8], most: usize) -> usize {
    let mut end = bytes.len().min(most);
    for _ in 0..4u8 {
        match bytes.get(end) {
            Some(byte) if byte & 0xc0 == 0x80 => end = end.checked_sub(1).expect("continuation follows a leading byte"),
            _ => break,
        }
    }
    end
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
