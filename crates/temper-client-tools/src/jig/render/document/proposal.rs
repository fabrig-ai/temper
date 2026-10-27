//! Proposal reads and the settings review supplied by wire 19 (client/tools.md, 5.2).
use super::{Cut, Limits, Text, settings};
use crate::jig::runner::{proposal::slot, reads::Reading};
use alloc::boxed::Box;
use jig_client::{Readout, contract as c};
use skein_json::writer::Encoder;

#[derive(Clone, Copy)]
enum Proposal<'a> {
    Owner(&'a c::OwnerProposal),
    Legacy(&'a c::Proposal),
}
fn selected(value: &c::ReadValue) -> Proposal<'_> {
    match value {
        c::ReadValue::OwnerProposal(p) => Proposal::Owner(p),
        c::ReadValue::Proposal(p) => Proposal::Legacy(p),
        c::ReadValue::Settings(_)
        | c::ReadValue::Setting(_)
        | c::ReadValue::Revision(_)
        | c::ReadValue::Revisions(_)
        | c::ReadValue::Tasks(_)
        | c::ReadValue::OwnerProject(_)
        | c::ReadValue::Historical(_)
        | c::ReadValue::Me(_)
        | c::ReadValue::Turn(_)
        | c::ReadValue::Task(_)
        | c::ReadValue::Escalation(_)
        | c::ReadValue::OwnerEscalation(_)
        | c::ReadValue::Question(_)
        | c::ReadValue::PersonTask(_)
        | c::ReadValue::OwnerPersonTask(_)
        | c::ReadValue::Result(_)
        | c::ReadValue::Ended(_)
        | c::ReadValue::OwnerEnded(_)
        | c::ReadValue::History(_)
        | c::ReadValue::Activity(_)
        | c::ReadValue::Inbox(_)
        | c::ReadValue::OwnerInbox(_)
        | c::ReadValue::Spend(_)
        | c::ReadValue::OwnerSpend(_)
        | c::ReadValue::Project(_)
        | c::ReadValue::Notes(_)
        | c::ReadValue::Deployment(_)
        | c::ReadValue::OwnerDeployment(_) => unreachable!("selected proposal read"),
    }
}

pub(super) fn text(value: &c::ReadValue, reading: &Reading, limits: &Limits, out: &mut Text) {
    let (key, reason, action) = match selected(value) {
        Proposal::Owner(p) => (p.key, p.reason.bytes.bytes(), owner_action(&p.action)),
        Proposal::Legacy(p) => (p.key, p.why.bytes(), legacy_action(&p.action)),
    };
    out.put(b"proposal ");
    settings::proposal_text(key, out);
    out.put(b" \xc2\xb7 ");
    out.put(action);
    if let c::ReadValue::OwnerProposal(p) = value
        && let c::OwnerProposalAction::Settings(s) = &p.action
    {
        out.put(b" \xc2\xb7 against revision ");
        out.number(u64::from(s.against));
        if let Some(review) = &s.review {
            out.put(b", newest ");
            out.number(u64::from(review.newest));
        }
    }
    out.put(b"\n");
    out.words(reason);
    out.put(b"\n");
    if let c::ReadValue::OwnerProposal(p) = value
        && let c::OwnerProposalAction::Settings(s) = &p.action
        && let Some(review) = &s.review
    {
        for (index, item) in review.items.iter().enumerate() {
            settings::setting_text(item.setting, out);
            out.put(b" \xc2\xb7 ");
            out.put(review_change(item));
            out.put(b" \xc2\xb7 now ");
            review_value_text(&item.now, item.setting, reading.pages.get(&slot(index, false)), limits, out);
            out.put(b" (");
            match &item.origin {
                c::SettingOrigin::Change(r) => {
                    out.put(b"revision ");
                    out.number(u64::from(r.revision));
                }
                c::SettingOrigin::Default(_) => out.put(b"default"),
                c::SettingOrigin::NotOffered => out.put(b"not offered"),
            }
            out.put(b") \xc2\xb7 would be ");
            review_value_text(&item.would_be, item.setting, reading.pages.get(&slot(index, true)), limits, out);
            out.put(b"\n");
        }
        if review.cut {
            out.put(b"cut: more not shown\n");
        }
    }
}
fn review_change(item: &c::ReviewItem) -> &'static [u8] {
    match item.edit {
        c::RevisionItemEdit::Set => b"set",
        c::RevisionItemEdit::Removed => b"removed",
        c::RevisionItemEdit::Reset => {
            if item.now == item.would_be {
                b"kept"
            } else {
                b"changed"
            }
        }
    }
}
fn resolved<'a>(
    value: &'a c::ReviewValue,
    setting: c::Setting,
    page: Option<&'a Readout>,
) -> Result<Option<&'a c::SettingValue>, Box<[u8]>> {
    match value {
        c::ReviewValue::Value(set) => Ok(Some(&set.value)),
        c::ReviewValue::None => Ok(None),
        c::ReviewValue::TooLarge(_) => match page {
            Some(Readout::Jig(c::ReadResult::Value(c::ReadValue::Setting(document))))
                if document.setting == setting =>
            {
                Ok(Some(&document.value))
            }
            Some(Readout::Jig(c::ReadResult::None(why))) => {
                let reason: &[u8] = match why {
                    c::ReadNone::Refused => b"refused",
                    c::ReadNone::Unknown => b"unknown",
                    c::ReadNone::Gone => b"gone",
                    c::ReadNone::Busy => b"busy",
                    c::ReadNone::SignedOut => b"signed out",
                    c::ReadNone::Unreachable => b"unreachable",
                    c::ReadNone::Malformed => b"malformed document",
                    c::ReadNone::TooLarge { .. } => b"document limit",
                    c::ReadNone::TooLinked { joins } => return Err(super::super::join_limit(*joins)),
                };
                Err(Box::from(reason))
            }
            Some(Readout::Limit) => Err(Box::from(b"read table limit".as_slice())),
            Some(Readout::NotServed { .. }) => Err(Box::from(b"version not served".as_slice())),
            Some(Readout::Jig(c::ReadResult::Value(_)) | Readout::Family { .. }) | None => {
                Err(Box::from(b"tools' bug; unexpected setting document; report it as a finding".as_slice()))
            }
        },
    }
}
fn review_value_text(
    value: &c::ReviewValue,
    setting: c::Setting,
    page: Option<&Readout>,
    limits: &Limits,
    out: &mut Text,
) {
    match resolved(value, setting, page) {
        Ok(Some(value)) => settings::value_inline(value, limits, out),
        Ok(None) => out.put(b"none"),
        Err(why) => out.put(&why),
    }
}
fn owner_action(action: &c::OwnerProposalAction) -> &'static [u8] {
    match action {
        c::OwnerProposalAction::Settings(_) => b"settings",
        c::OwnerProposalAction::Batch(_) => b"batch",
        c::OwnerProposalAction::Effect { .. } => b"effect",
        c::OwnerProposalAction::ProcedureEffect { .. } => b"procedure_effect",
        c::OwnerProposalAction::Amend { .. } => b"amend",
        c::OwnerProposalAction::Widen { .. } => b"widen",
        c::OwnerProposalAction::Release { .. } => b"release",
    }
}
fn legacy_action(action: &c::ProposalAction) -> &'static [u8] {
    match action {
        c::ProposalAction::Batch { .. } => b"batch",
        c::ProposalAction::Effect { .. } => b"effect",
        c::ProposalAction::Widen { .. } => b"widen",
        c::ProposalAction::Amend { .. } => b"amend",
    }
}
pub(super) fn encode(value: &c::ReadValue, reading: &Reading, out: &mut Encoder, budget: &mut Cut) {
    out.object_start();
    let proposal = selected(value);
    let (key, project, acts) = match proposal {
        Proposal::Owner(p) => (p.key, p.project, &p.acts),
        Proposal::Legacy(p) => (p.key, p.project, &p.acts),
    };
    out.key(b"key");
    super::encode_proposal_key(&key, out, budget);
    out.key(b"project");
    out.unsigned(u64::from(project));
    match proposal {
        Proposal::Owner(p) => {
            out.key(b"reason");
            super::encode_document_text(&p.reason, out, budget);
            out.key(b"action");
            match &p.action {
                c::OwnerProposalAction::Settings(s) => encode_settings(s, reading, out),
                action @ (c::OwnerProposalAction::Batch(_)
                | c::OwnerProposalAction::Effect { .. }
                | c::OwnerProposalAction::ProcedureEffect { .. }
                | c::OwnerProposalAction::Amend { .. }
                | c::OwnerProposalAction::Widen { .. }
                | c::OwnerProposalAction::Release { .. }) => out.string(owner_action(action)),
            }
            out.key(b"as_holder");
            out.boolean(p.as_holder);
            out.key(b"waiting");
            out.object_start();
            out.key(b"holder");
            holder(p.waiting.holder, out, budget);
            out.key(b"since");
            out.unsigned(p.waiting.since.as_nanos());
            out.object_end();
            out.key(b"funding");
            out.array_start();
            for offer in &p.funding {
                out.object_start();
                out.key(b"source");
                super::encode_funder(&offer.source, out, budget);
                out.key(b"needed");
                out.unsigned(offer.needed);
                for (key, value) in [(b"left".as_slice(), offer.left), (b"after".as_slice(), offer.after)] {
                    out.key(key);
                    match value {
                        Some(n) => out.unsigned(n),
                        None => out.null(),
                    }
                }
                out.object_end();
            }
            out.array_end();
        }
        Proposal::Legacy(p) => {
            out.key(b"why");
            budget.words(p.why.bytes(), out);
            out.key(b"action");
            out.string(legacy_action(&p.action));
            out.key(b"waiting");
            super::encode_recipient(&p.waiting, out, budget);
            out.key(b"since");
            out.unsigned(p.since.as_nanos());
        }
    }
    out.key(b"acts");
    out.array_start();
    for act in acts {
        super::encode_offered(act, out, budget);
    }
    out.array_end();
    out.object_end();
}
fn holder(holder: c::OwnerHistoryHolder, out: &mut Encoder, budget: &mut Cut) {
    out.object_start();
    match holder {
        c::OwnerHistoryHolder::Task(n) => holder_number(b"task", n, out),
        c::OwnerHistoryHolder::Person(n) => holder_number(b"person", n, out),
        c::OwnerHistoryHolder::Policy { project, kind } => {
            out.key(b"policy");
            out.object_start();
            out.key(b"project");
            out.unsigned(u64::from(project));
            out.key(b"kind");
            super::read_owner_words::encode_owner_message_proposal_kind(kind, out, budget);
            out.object_end();
        }
    }
    out.object_end();
}
fn holder_number(kind: &[u8], n: u64, out: &mut Encoder) {
    out.key(kind);
    out.object_start();
    out.key(b"value");
    out.unsigned(n);
    out.object_end();
}

fn encode_settings(s: &c::OwnerProposalActionSettings, reading: &Reading, out: &mut Encoder) {
    encode_settings_fields(s.against, &s.items, s.review.as_ref(), s.made, Some(reading), out);
}
fn encode_settings_fields(
    against: u32,
    items: &skein_lib::List<c::RevisionItemHead>,
    review: Option<&c::SettingsReview>,
    made: Option<u32>,
    reading: Option<&Reading>,
    out: &mut Encoder,
) {
    out.object_start();
    out.key(b"settings");
    out.object_start();
    out.key(b"against");
    out.unsigned(u64::from(against));
    out.key(b"items");
    out.array_start();
    for item in items {
        out.object_start();
        out.key(b"setting");
        settings::encode_setting(item.setting, out);
        out.key(b"edit");
        out.string(settings::edit_word(&item.edit));
        out.object_end();
    }
    out.array_end();
    out.key(b"review");
    match review {
        Some(review) => encode_review(review, reading.expect("pending review reads"), out),
        None => out.null(),
    }
    out.key(b"made");
    match made {
        Some(n) => out.unsigned(u64::from(n)),
        None => out.null(),
    }
    out.object_end();
    out.object_end();
}
fn encode_review(review: &c::SettingsReview, reading: &Reading, out: &mut Encoder) {
    out.object_start();
    out.key(b"newest");
    out.unsigned(u64::from(review.newest));
    out.key(b"items");
    out.array_start();
    for (index, item) in review.items.iter().enumerate() {
        out.object_start();
        out.key(b"setting");
        settings::encode_setting(item.setting, out);
        out.key(b"edit");
        out.string(settings::edit_word(&item.edit));
        out.key(b"now");
        encode_review_value(&item.now, item.setting, reading.pages.get(&slot(index, false)), out);
        out.key(b"origin");
        match &item.origin {
            c::SettingOrigin::Default(r) => origin_revision(b"default", r.revision, out),
            c::SettingOrigin::Change(r) => origin_revision(b"change", r.revision, out),
            c::SettingOrigin::NotOffered => out.string(b"not_offered"),
        }
        out.key(b"would_be");
        encode_review_value(&item.would_be, item.setting, reading.pages.get(&slot(index, true)), out);
        out.object_end();
    }
    out.array_end();
    out.key(b"cut");
    out.boolean(review.cut);
    out.object_end();
}
fn origin_revision(kind: &[u8], revision: u32, out: &mut Encoder) {
    out.object_start();
    out.key(kind);
    out.object_start();
    out.key(b"revision");
    out.unsigned(u64::from(revision));
    out.object_end();
    out.object_end();
}

fn encode_review_value(value: &c::ReviewValue, setting: c::Setting, page: Option<&Readout>, out: &mut Encoder) {
    match resolved(value, setting, page) {
        Ok(Some(value)) => {
            out.object_start();
            out.key(b"value");
            out.object_start();
            out.key(b"value");
            out.object_start();
            out.key(match value {
                c::SettingValue::Charter(_) => b"charter",
                c::SettingValue::Template(_) => b"template",
                c::SettingValue::Policy(_) => b"policy",
                c::SettingValue::Rules(_) => b"rules",
            });
            settings::encode_value(value, out);
            out.object_end();
            out.object_end();
            out.object_end();
        }
        Ok(None) => out.string(b"none"),
        Err(why) => {
            out.object_start();
            out.key(b"refused");
            out.string(&why);
            out.object_end();
        }
    }
}

fn history_state(state: &c::OwnerProposalHistoryState) -> &'static [u8] {
    match state {
        c::OwnerProposalHistoryState::Pending { .. } => b"pending",
        c::OwnerProposalHistoryState::Accepted { .. } => b"accepted",
        c::OwnerProposalHistoryState::Rejected { .. } => b"rejected",
        c::OwnerProposalHistoryState::Withdrawn => b"withdrawn",
    }
}
fn retained_kind(action: &c::OwnerRetainedAction) -> &'static [u8] {
    match action {
        c::OwnerRetainedAction::Settings(_) => b"settings",
        c::OwnerRetainedAction::Batch(_) => b"batch",
        c::OwnerRetainedAction::Effect { .. } => b"effect",
        c::OwnerRetainedAction::ProcedureEffect { .. } => b"procedure_effect",
        c::OwnerRetainedAction::Amend { .. } => b"amend",
        c::OwnerRetainedAction::Widen { .. } => b"widen",
        c::OwnerRetainedAction::Release { .. } => b"release",
    }
}
fn person_state(state: &c::OwnerPersonProposalState) -> &'static [u8] {
    match state {
        c::OwnerPersonProposalState::Pending { .. } => b"pending",
        c::OwnerPersonProposalState::Accepted { .. } => b"accepted",
        c::OwnerPersonProposalState::Rejected { .. } => b"rejected",
    }
}
pub(super) fn history_text(value: &c::OwnerHistorical, out: &mut Text) {
    let key = super::super::historical_proposal(value).expect("validated proposal history");
    out.put(b"proposal ");
    settings::proposal_text(key, out);
    out.put(b" \xc2\xb7 ");
    match &value.value {
        c::OwnerHistoricalValue::Proposal(history) => {
            out.put(retained_kind(&history.action));
            out.put(b" \xc2\xb7 ");
            out.put(history_state(&history.state));
            out.put(b"\n");
            if let c::OwnerRetainedAction::Settings(settings) = &history.action {
                out.put(b"against revision ");
                out.number(u64::from(settings.against));
                if let Some(made) = settings.made {
                    out.put(b", made revision ");
                    out.number(u64::from(made));
                }
                out.put(b"\n");
            }
        }
        c::OwnerHistoricalValue::PersonProposal { proposal, .. } => {
            out.put(b"batch \xc2\xb7 ");
            out.put(person_state(&proposal.state));
            out.put(b"\n");
        }
        c::OwnerHistoricalValue::Task { .. }
        | c::OwnerHistoricalValue::Escalation { .. }
        | c::OwnerHistoricalValue::Question { .. }
        | c::OwnerHistoricalValue::PersonTask { .. } => unreachable!("validated proposal history"),
    }
}
pub(super) fn encode_history(value: &c::OwnerHistorical, out: &mut Encoder, budget: &mut Cut) {
    let key = super::super::historical_proposal(value).expect("validated proposal history");
    out.object_start();
    out.key(b"key");
    super::encode_proposal_key(&key, out, budget);
    out.key(b"action");
    match &value.value {
        c::OwnerHistoricalValue::Proposal(history) => {
            match &history.action {
                c::OwnerRetainedAction::Settings(s) => {
                    encode_settings_fields(s.against, &s.items, None, s.made, None, out);
                }
                other @ (c::OwnerRetainedAction::Batch(_)
                | c::OwnerRetainedAction::Effect { .. }
                | c::OwnerRetainedAction::ProcedureEffect { .. }
                | c::OwnerRetainedAction::Amend { .. }
                | c::OwnerRetainedAction::Widen { .. }
                | c::OwnerRetainedAction::Release { .. }) => out.string(retained_kind(other)),
            }
            out.key(b"state");
            encode_history_state(&history.state, out, budget);
        }
        c::OwnerHistoricalValue::PersonProposal { proposal, .. } => {
            out.string(b"batch");
            out.key(b"state");
            encode_person_state(&proposal.state, out, budget);
        }
        c::OwnerHistoricalValue::Task { .. }
        | c::OwnerHistoricalValue::Escalation { .. }
        | c::OwnerHistoricalValue::Question { .. }
        | c::OwnerHistoricalValue::PersonTask { .. } => unreachable!("validated proposal history"),
    }
    out.object_end();
}
fn encode_person_state(state: &c::OwnerPersonProposalState, out: &mut Encoder, budget: &mut Cut) {
    out.object_start();
    out.key(person_state(state));
    out.object_start();
    match state {
        c::OwnerPersonProposalState::Pending { since } => {
            out.key(b"since");
            out.unsigned(since.as_nanos());
        }
        c::OwnerPersonProposalState::Accepted { by } => {
            out.key(b"by");
            super::encode_owner_requester(by, out, budget);
        }
        c::OwnerPersonProposalState::Rejected { by, reason, .. } => {
            out.key(b"by");
            super::encode_owner_requester(by, out, budget);
            out.key(b"reason");
            budget.words(reason.bytes(), out);
        }
    }
    out.object_end();
    out.object_end();
}
fn encode_history_state(state: &c::OwnerProposalHistoryState, out: &mut Encoder, budget: &mut Cut) {
    out.object_start();
    out.key(history_state(state));
    out.object_start();
    match state {
        c::OwnerProposalHistoryState::Pending { holder: held, since } => {
            out.key(b"holder");
            holder(*held, out, budget);
            out.key(b"since");
            out.unsigned(since.as_nanos());
        }
        c::OwnerProposalHistoryState::Accepted { by } => {
            out.key(b"by");
            super::encode_owner_requester(by, out, budget);
        }
        c::OwnerProposalHistoryState::Rejected { by, reason } => {
            out.key(b"by");
            super::encode_owner_requester(by, out, budget);
            out.key(b"reason");
            super::encode_document_text(reason, out, budget);
        }
        c::OwnerProposalHistoryState::Withdrawn => {}
    }
    out.object_end();
    out.object_end();
}
