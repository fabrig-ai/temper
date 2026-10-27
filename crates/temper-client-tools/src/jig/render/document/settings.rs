//! Deployment settings and retained revision documents (client/tools.md, 5.2).
use super::{Text, settings_values as values};
use crate::jig::Limits;
use jig_client::contract as c;
use skein_json::writer::Encoder;

pub(crate) fn setting_text(setting: c::Setting, out: &mut Text) {
    let (kind, number) = match setting {
        c::Setting::Charter(n) => (b"charter".as_slice(), Some(n.number)),
        c::Setting::Template(n) => (b"template".as_slice(), Some(n.number)),
        c::Setting::Policy(n) => (b"policy".as_slice(), Some(n.number)),
        c::Setting::Rules => (b"rules".as_slice(), None),
    };
    out.put(kind);
    if let Some(number) = number {
        out.put(b" ");
        out.number(u64::from(number));
    }
}
pub(super) fn encode_setting(setting: c::Setting, out: &mut Encoder) {
    let (kind, number) = match setting {
        c::Setting::Charter(n) => (b"charter".as_slice(), Some(n.number)),
        c::Setting::Template(n) => (b"template".as_slice(), Some(n.number)),
        c::Setting::Policy(n) => (b"policy".as_slice(), Some(n.number)),
        c::Setting::Rules => (b"rules".as_slice(), None),
    };
    out.object_start();
    out.key(b"kind");
    out.string(kind);
    if let Some(n) = number {
        out.key(b"number");
        out.unsigned(u64::from(n));
    }
    out.object_end();
}
pub(super) fn encode_value(value: &c::SettingValue, out: &mut Encoder) {
    match value {
        c::SettingValue::Charter(v) => values::encode_charter(v, out),
        c::SettingValue::Template(v) => values::encode_template(v, out),
        c::SettingValue::Policy(v) => values::encode_setting_policy(v, out),
        c::SettingValue::Rules(v) => values::encode_rules(v, out),
    }
}
pub(super) fn value_inline(value: &c::SettingValue, limits: &Limits, out: &mut Text) {
    let bounds =
        skein_json::writer::Limits { depth: 32, length: u32::try_from(limits.result_bytes).expect("result bound") };
    let mut measured = Encoder::measure(&bounds);
    encode_value(value, &mut measured);
    // The document entrance has already measured the same bounded value.
    let length = measured.measured().expect("received settings value fits result envelope");
    let mut write = Encoder::write(length, &bounds);
    encode_value(value, &mut write);
    out.put(&write.finish());
}
fn value_text(value: &c::SettingValue, limits: &Limits, out: &mut Text) {
    value_inline(value, limits, out);
    out.put(b"\n");
}
pub(crate) fn proposal_text(proposal: c::ProposalKey, out: &mut Text) {
    let bytes = crate::jig::argv::value::encode(
        &[],
        crate::jig::table::Kind::Proposal,
        &crate::jig::call::Value::Proposal(proposal),
    )
    .expect("proposal identity");
    out.put(&bytes);
}
fn head_text(head: &c::RevisionHead, out: &mut Text) {
    out.put(b"revision ");
    out.number(u64::from(head.number));
    out.put(b", against revision ");
    out.number(u64::from(head.against));
    out.put(b", at ");
    super::utc(head.at, out);
    out.put(b" UTC");
    out.put(b", by ");
    match &head.by {
        c::RevisionDecider::FirstStart => out.put(b"the first start"),
        c::RevisionDecider::Release => out.put(b"the release"),
        c::RevisionDecider::Party(p) => {
            out.put(b"party ");
            out.number(p.number);
        }
    }
    if let Some(p) = head.proposal {
        out.put(b", from ");
        proposal_text(p, out);
    }
    out.put(if head.moved { b", running work moves\n" } else { b", running work stays\n" });
}
fn encode_head(head: &c::RevisionHead, out: &mut Encoder) {
    out.object_start();
    out.key(b"number");
    out.unsigned(u64::from(head.number));
    out.key(b"against");
    out.unsigned(u64::from(head.against));
    out.key(b"at");
    out.unsigned(head.at.as_nanos());
    out.key(b"by");
    match &head.by {
        c::RevisionDecider::FirstStart => out.string(b"first_start"),
        c::RevisionDecider::Release => out.string(b"release"),
        c::RevisionDecider::Party(p) => {
            out.object_start();
            out.key(b"party");
            out.unsigned(p.number);
            out.object_end();
        }
    }
    out.key(b"proposal");
    match head.proposal {
        Some(p) => {
            let bytes = crate::jig::argv::value::encode(
                &[],
                crate::jig::table::Kind::Proposal,
                &crate::jig::call::Value::Proposal(p),
            )
            .expect("proposal identity");
            out.string(&bytes);
        }
        None => out.null(),
    }
    out.key(b"moved");
    out.boolean(head.moved);
    out.object_end();
}
pub(super) fn text(value: &c::ReadValue, requested: Option<u64>, limits: &Limits, out: &mut Text) {
    match value {
        c::ReadValue::Settings(document) => inventory_text(document, out),
        c::ReadValue::Setting(document) => {
            setting_text(document.setting, out);
            out.put(b", revision ");
            out.number(u64::from(document.revision));
            out.put(b"\n");
            value_text(&document.value, limits, out);
        }
        c::ReadValue::Revision(answer) => revision_text(answer, requested.expect("revision call"), limits, out),
        c::ReadValue::Revisions(page) => {
            for row in &page.rows {
                head_text(&row.head, out);
                for item in &row.items {
                    out.put(b"  ");
                    setting_text(item.setting, out);
                    out.put(b": ");
                    out.put(edit_word(&item.edit));
                    out.put(b"\n");
                }
                if row.more != 0 {
                    out.put(b"  ");
                    out.number(u64::from(row.more));
                    out.put(b" more items\n");
                }
            }
            match page.next {
                Some(next) => {
                    out.put(b"read on: revision list --before ");
                    out.number(u64::from(next));
                    out.put(b"\n");
                }
                None => {
                    out.put(if page.forgotten { b"older revisions were forgotten\n" } else { b"no older revisions\n" });
                }
            }
        }
        c::ReadValue::Tasks(_)
        | c::ReadValue::OwnerProject(_)
        | c::ReadValue::Historical(_)
        | c::ReadValue::Me(_)
        | c::ReadValue::Turn(_)
        | c::ReadValue::Task(_)
        | c::ReadValue::Proposal(_)
        | c::ReadValue::OwnerProposal(_)
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
        | c::ReadValue::OwnerDeployment(_) => unreachable!("selected settings read"),
    }
}
pub(super) fn edit_word(edit: &c::RevisionItemEdit) -> &'static [u8] {
    match edit {
        c::RevisionItemEdit::Set => b"set",
        c::RevisionItemEdit::Removed => b"removed",
        c::RevisionItemEdit::Reset => b"reset",
    }
}
pub(super) fn encode(value: &c::ReadValue, out: &mut Encoder) {
    out.object_start();
    match value {
        c::ReadValue::Settings(document) => encode_inventory(document, out),
        c::ReadValue::Setting(document) => {
            out.key(b"setting");
            encode_setting(document.setting, out);
            out.key(b"revision");
            out.unsigned(u64::from(document.revision));
            out.key(b"value");
            encode_value(&document.value, out);
        }
        c::ReadValue::Revision(answer) => encode_revision(answer, out),
        c::ReadValue::Revisions(page) => {
            out.key(b"rows");
            out.array_start();
            for row in &page.rows {
                out.object_start();
                out.key(b"head");
                encode_head(&row.head, out);
                out.key(b"items");
                out.array_start();
                for item in &row.items {
                    out.object_start();
                    out.key(b"setting");
                    encode_setting(item.setting, out);
                    out.key(b"edit");
                    out.string(edit_word(&item.edit));
                    out.object_end();
                }
                out.array_end();
                out.key(b"more");
                out.unsigned(u64::from(row.more));
                out.object_end();
            }
            out.array_end();
            out.key(b"next");
            match page.next {
                Some(n) => out.unsigned(u64::from(n)),
                None => out.null(),
            }
            out.key(b"forgotten");
            out.boolean(page.forgotten);
        }
        c::ReadValue::Tasks(_)
        | c::ReadValue::OwnerProject(_)
        | c::ReadValue::Historical(_)
        | c::ReadValue::Me(_)
        | c::ReadValue::Turn(_)
        | c::ReadValue::Task(_)
        | c::ReadValue::Proposal(_)
        | c::ReadValue::OwnerProposal(_)
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
        | c::ReadValue::OwnerDeployment(_) => unreachable!("selected settings read"),
    }
    out.object_end();
}
pub(crate) const fn fault_word(fault: c::SettingFault) -> &'static [u8] {
    match fault {
        c::SettingFault::Shape => b"shape",
        c::SettingFault::Count => b"count",
        c::SettingFault::Reference => b"reference",
        c::SettingFault::Rules => b"rules",
        c::SettingFault::Budget => b"budget",
        c::SettingFault::Grace => b"grace",
        c::SettingFault::Owner => b"owner",
    }
}

fn inventory_text(document: &c::SettingsDocument, out: &mut Text) {
    out.put(b"newest revision ");
    out.number(u64::from(document.newest));
    out.put(b"\nsettings:\n");
    for line in &document.settings {
        out.put(b"  ");
        setting_text(line.setting, out);
        if let Some(name) = &line.name {
            out.put(b" \"");
            out.words(name.bytes());
            out.put(b"\"");
        }
        out.put(b": ");
        match line.follows {
            c::SettingFollows::Default => {
                out.put(b"default");
                if line.last != 0 {
                    out.put(b", last changed in revision ");
                    out.number(u64::from(line.last));
                }
            }
            c::SettingFollows::Change => {
                out.put(b"changed in revision ");
                out.number(u64::from(line.last));
            }
        }
        out.put(b"\n");
    }
    out.put(b"held revisions:\n");
    for held in &document.held {
        out.put(b"  ");
        head_text(&held.head, out);
        out.put(b"    live tasks: ");
        out.number(u64::from(held.holders));
        out.put(b"\n");
    }
    out.put(b"pending proposals:\n");
    for p in &document.pending {
        out.put(b"  ");
        proposal_text(c::ProposalKey { proposer: p.proposer, number: u64::from(p.number) }, out);
        out.put(b"\n");
    }
}

fn revision_text(answer: &c::RevisionAnswer, requested: u64, limits: &Limits, out: &mut Text) {
    match answer {
        c::RevisionAnswer::Revision(document) => {
            head_text(&document.head, out);
            for item in &document.items {
                out.put(b"  ");
                setting_text(item.setting, out);
                out.put(b": ");
                match &item.edit {
                    c::RevisionEdit::Set(set) => {
                        out.put(b"set\n");
                        value_text(&set.value, limits, out);
                    }
                    c::RevisionEdit::TooLarge(size) => {
                        out.put(b"too large, ");
                        out.number(size.bytes);
                        match item.setting {
                            c::Setting::Charter(_) => out.put(b" bytes; read with settings show "),
                            c::Setting::Template(_) | c::Setting::Policy(_) | c::Setting::Rules => {
                                out.put(b" bytes; read current value with settings show ");
                            }
                        }
                        setting_text(item.setting, out);
                        if let c::Setting::Charter(_) = item.setting {
                            out.put(b" --revision ");
                            out.number(u64::from(document.head.number));
                        }
                        out.put(b"\n");
                    }
                    c::RevisionEdit::Removed => out.put(b"removed\n"),
                    c::RevisionEdit::Reset => out.put(b"reset\n"),
                }
            }
        }
        c::RevisionAnswer::Forgotten(f) => {
            out.put(b"revision ");
            out.number(requested);
            out.put(b" is forgotten; the oldest kept is revision ");
            out.number(u64::from(f.oldest));
            out.put(b"\n");
        }
        c::RevisionAnswer::Unknown => out.put(b"unknown revision\n"),
    }
}

fn encode_inventory(document: &c::SettingsDocument, out: &mut Encoder) {
    out.key(b"newest");
    out.unsigned(u64::from(document.newest));
    out.key(b"settings");
    out.array_start();
    for line in &document.settings {
        out.object_start();
        out.key(b"setting");
        encode_setting(line.setting, out);
        out.key(b"name");
        match &line.name {
            Some(name) => values::encode_document_text(name, out),
            None => out.null(),
        }
        out.key(b"follows");
        out.string(match line.follows {
            c::SettingFollows::Default => b"default",
            c::SettingFollows::Change => b"change",
        });
        out.key(b"last");
        out.unsigned(u64::from(line.last));
        out.object_end();
    }
    out.array_end();
    out.key(b"held");
    out.array_start();
    for held in &document.held {
        out.object_start();
        out.key(b"head");
        encode_head(&held.head, out);
        out.key(b"holders");
        out.unsigned(u64::from(held.holders));
        out.object_end();
    }
    out.array_end();
    out.key(b"pending");
    out.array_start();
    for p in &document.pending {
        out.object_start();
        out.key(b"proposal");
        let mut measured = Text::measure();
        proposal_text(c::ProposalKey { proposer: p.proposer, number: u64::from(p.number) }, &mut measured);
        let mut write = Text::writing_exact(measured);
        proposal_text(c::ProposalKey { proposer: p.proposer, number: u64::from(p.number) }, &mut write);
        out.string(&write.finish());
        out.object_end();
    }
    out.array_end();
}

fn encode_revision(answer: &c::RevisionAnswer, out: &mut Encoder) {
    match answer {
        c::RevisionAnswer::Revision(document) => {
            out.key(b"head");
            encode_head(&document.head, out);
            out.key(b"items");
            out.array_start();
            for item in &document.items {
                out.object_start();
                out.key(b"setting");
                encode_setting(item.setting, out);
                match &item.edit {
                    c::RevisionEdit::Set(set) => {
                        out.key(b"set");
                        encode_value(&set.value, out);
                    }
                    c::RevisionEdit::TooLarge(size) => {
                        out.key(b"too_large");
                        out.unsigned(size.bytes);
                    }
                    c::RevisionEdit::Removed => {
                        out.key(b"edit");
                        out.string(b"removed");
                    }
                    c::RevisionEdit::Reset => {
                        out.key(b"edit");
                        out.string(b"reset");
                    }
                }
                out.object_end();
            }
            out.array_end();
        }
        c::RevisionAnswer::Forgotten(f) => {
            out.key(b"forgotten");
            out.object_start();
            out.key(b"oldest");
            out.unsigned(u64::from(f.oldest));
            out.object_end();
        }
        c::RevisionAnswer::Unknown => {
            out.key(b"unknown");
            out.boolean(true);
        }
    }
}
