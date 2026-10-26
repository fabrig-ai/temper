//! Structural adapters between neutral forms and the shared client wire.
use crate::{Limits, view as c};
use alloc::boxed::Box;
use jig_client_protocol_wire::v19 as w;
use skein_lib::{List, Reader, Wall, Writer, bytes};
pub fn restore_revisions_page(value: &w::RevisionsPage, limits: &Limits) -> Result<c::RevisionsPage, w::Problem> {
    let mut rows = List::with_capacity(value.rows().len());
    for item in value.rows() {
        rows.push(settings_restore_revision_row(item, limits)).expect("same capacity");
    }
    Ok(c::RevisionsPage { rows, next: *value.next(), forgotten: value.forgotten() })
}

pub(crate) fn settings_restore_revision_row(value: &w::RevisionRow, limits: &Limits) -> c::RevisionRow {
    let mut items = List::with_capacity(value.items().len());
    for item in value.items() {
        items.push(settings_restore_revision_item_head(item, limits)).expect("same capacity");
    }
    c::RevisionRow { head: settings_restore_revision_head(value.head(), limits), items, more: value.more() }
}

pub(crate) fn settings_restore_revision_head(value: &w::RevisionHead, limits: &Limits) -> c::RevisionHead {
    c::RevisionHead {
        number: value.number(),
        against: value.against(),
        at: Wall::from_nanos(value.at()),
        by: settings_restore_revision_decider(value.by(), limits),
        proposal: optional_proposal(value.proposal().as_ref(), limits),
        moved: value.moved(),
    }
}

pub(crate) fn answers_proposal_key(value: &w::ProposalKey, limits: &Limits) -> c::ProposalKey {
    c::ProposalKey { proposer: answers_proposer(value.proposer(), limits), number: value.number() }
}

pub(crate) fn answers_proposer(value: &w::Proposer, _limits: &Limits) -> c::Proposer {
    match value {
        w::Proposer::Task(row) => c::Proposer::Task(row.value()),
        w::Proposer::Party(row) => c::Proposer::Party(row.value()),
    }
}

pub(crate) fn settings_restore_revision_decider(value: &w::RevisionDecider, limits: &Limits) -> c::RevisionDecider {
    match value {
        w::RevisionDecider::FirstStart => c::RevisionDecider::FirstStart,
        w::RevisionDecider::Release => c::RevisionDecider::Release,
        w::RevisionDecider::Party(item) => c::RevisionDecider::Party(settings_restore_revision_party(item, limits)),
    }
}

pub(crate) fn settings_restore_revision_party(value: &w::RevisionParty, _limits: &Limits) -> c::RevisionParty {
    c::RevisionParty { number: value.number() }
}

pub(crate) fn settings_restore_revision_item_head(value: &w::RevisionItemHead, limits: &Limits) -> c::RevisionItemHead {
    c::RevisionItemHead {
        setting: settings_restore_setting(value.setting(), limits),
        edit: settings_restore_revision_item_edit(value.edit(), limits),
    }
}

pub(crate) fn settings_restore_revision_item_edit(
    value: &w::RevisionItemEdit,
    _limits: &Limits,
) -> c::RevisionItemEdit {
    match value {
        w::RevisionItemEdit::Set => c::RevisionItemEdit::Set,
        w::RevisionItemEdit::Removed => c::RevisionItemEdit::Removed,
        w::RevisionItemEdit::Reset => c::RevisionItemEdit::Reset,
    }
}

pub(crate) fn settings_restore_setting(value: &w::Setting, limits: &Limits) -> c::Setting {
    match value {
        w::Setting::Charter(item) => c::Setting::Charter(settings_restore_setting_number(item, limits)),
        w::Setting::Template(item) => c::Setting::Template(settings_restore_setting_number(item, limits)),
        w::Setting::Policy(item) => c::Setting::Policy(settings_restore_setting_number(item, limits)),
        w::Setting::Rules => c::Setting::Rules,
    }
}

pub(crate) fn settings_restore_setting_number(value: &w::SettingNumber, _limits: &Limits) -> c::SettingNumber {
    c::SettingNumber { number: value.number() }
}

pub fn restore_revision_answer(value: &w::RevisionAnswer, limits: &Limits) -> Result<c::RevisionAnswer, w::Problem> {
    Ok(match value {
        w::RevisionAnswer::Revision(item) => {
            c::RevisionAnswer::Revision(settings_restore_revision_document(item, limits)?)
        }
        w::RevisionAnswer::Forgotten(item) => {
            c::RevisionAnswer::Forgotten(settings_restore_revision_forgotten(item, limits))
        }
        w::RevisionAnswer::Unknown => c::RevisionAnswer::Unknown,
    })
}

pub(crate) fn settings_restore_revision_forgotten(
    value: &w::RevisionForgotten,
    _limits: &Limits,
) -> c::RevisionForgotten {
    c::RevisionForgotten { oldest: value.oldest() }
}

pub(crate) fn settings_restore_revision_document(
    value: &w::RevisionDocument,
    limits: &Limits,
) -> Result<c::RevisionDocument, w::Problem> {
    Ok(c::RevisionDocument {
        head: settings_restore_revision_head(value.head(), limits),
        items: settings_decode_revision_item_pieces(value.items(), limits)?,
    })
}

pub(crate) fn settings_decode_revision_item_pieces(
    values: &[u8],
    limits: &Limits,
) -> Result<List<c::RevisionItem>, w::Problem> {
    let count = settings_item_count(values, limits)?;
    let mut reader = Reader::new(values);
    let mut rows = List::with_capacity(count);
    for _ in 0_u32..=1024 {
        let Some(piece) = (match skein_codec::piece::split(&mut reader) {
            Ok(value) => value,
            Err(reason) => return Err(root_piece_problem(reason)),
        }) else {
            return Ok(rows);
        };
        let mut input = Reader::new(&piece);
        let row = w::RevisionItem::decode(&limits.settings, &mut input)?;
        if !input.is_empty() {
            return Err(root_too_large());
        }
        if rows.push(settings_restore_revision_item(&row, limits)?).is_err() {
            return Err(root_too_large());
        }
    }
    Err(root_too_large())
}

pub(crate) fn root_too_large() -> w::Problem {
    w::Problem { path: w::Path::OwnedRecord, reason: skein_codec::Reason::Bound }
}

pub(crate) fn settings_restore_revision_item(
    value: &w::RevisionItem,
    limits: &Limits,
) -> Result<c::RevisionItem, w::Problem> {
    Ok(c::RevisionItem {
        setting: settings_restore_setting(value.setting(), limits),
        edit: settings_restore_revision_edit(value.edit(), limits)?,
        replaced: settings_restore_revision_replaced(value.replaced(), limits)?,
    })
}

pub(crate) fn settings_restore_revision_edit(
    value: &w::RevisionEdit,
    limits: &Limits,
) -> Result<c::RevisionEdit, w::Problem> {
    Ok(match value {
        w::RevisionEdit::Set(item) => c::RevisionEdit::Set(settings_restore_setting_set(item, limits)?),
        w::RevisionEdit::TooLarge(item) => c::RevisionEdit::TooLarge(settings_restore_size(item, limits)),
        w::RevisionEdit::Removed => c::RevisionEdit::Removed,
        w::RevisionEdit::Reset => c::RevisionEdit::Reset,
    })
}

pub(crate) fn settings_restore_size(value: &w::Size, _limits: &Limits) -> c::Size {
    c::Size { bytes: value.bytes() }
}

pub(crate) fn settings_restore_setting_set(
    value: &w::SettingSet,
    limits: &Limits,
) -> Result<c::SettingSet, w::Problem> {
    Ok(c::SettingSet { value: settings_restore_setting_value(value.value(), limits)? })
}

pub(crate) fn settings_restore_setting_value(
    value: &w::SettingValue,
    limits: &Limits,
) -> Result<c::SettingValue, w::Problem> {
    Ok(match value {
        w::SettingValue::Charter(item) => c::SettingValue::Charter(Box::new(settings_restore_charter(item, limits)?)),
        w::SettingValue::Template(item) => c::SettingValue::Template(Box::new(documents_me_template(item, limits)?)),
        w::SettingValue::Policy(item) => {
            c::SettingValue::Policy(Box::new(settings_restore_setting_policy(item, limits)?))
        }
        w::SettingValue::Rules(item) => c::SettingValue::Rules(Box::new(settings_restore_rules(item, limits)?)),
    })
}

pub(crate) fn settings_restore_rules(value: &w::Rules, limits: &Limits) -> Result<c::Rules, w::Problem> {
    let mut implies = List::with_capacity(value.implies().len());
    for item in value.implies() {
        implies
            .push(c::OwnerKindImplication { connector: item.connector(), kind: item.kind(), implies: item.implies() })
            .expect("same capacity");
    }
    let mut requirements = List::with_capacity(value.requirements().len());
    for item in value.requirements() {
        requirements.push(documents_policy_requirement_fields(item, limits)).expect("same capacity");
    }
    Ok(c::Rules {
        ceiling: documents_owner_authority_fields(value.ceiling(), limits)?,
        period_spend: value.period_spend(),
        minimum_run_spend: value.minimum_run_spend(),
        maximum_run_spend: value.maximum_run_spend(),
        implies,
        requirements,
    })
}

pub(crate) fn documents_owner_authority_fields(
    value: &w::OwnerAuthority,
    limits: &Limits,
) -> Result<c::OwnerAuthority, w::Problem> {
    let opt0 = optional_wall(value.deadline().as_ref());
    Ok(c::OwnerAuthority {
        models: model_sets_decode(value.models().as_ref())?,
        tools: value.tools(),
        grants: documents_owner_grant_piece(value.grants(), limits, limits.authority)?,
        delegation: documents_owner_delegation_fields(value.delegation(), limits)?,
        spend: value.spend(),
        deadline: opt0,
        notes: value.notes(),
        note_resources: documents_owner_resource_scope_piece(value.note_resources(), limits, limits.authority)?,
    })
}

pub(crate) fn documents_owner_resource_scope_piece(
    bytes: &[u8],
    limits: &Limits,
    bound: u32,
) -> Result<List<c::OwnerResourceScope>, w::Problem> {
    let mut reader = Reader::new(bytes);
    let mut rows = List::with_capacity(bound);
    let mut sum = 0_u64;
    for _ in 0..=bound {
        let item = match skein_codec::piece::split(&mut reader) {
            Ok(value) => value,
            Err(reason) => return Err(root_piece_problem(reason)),
        };
        let Some(item) = item else {
            return Ok(rows);
        };
        sum = sum
            .checked_add(u64::try_from(item.len()).expect("piece fits u64"))
            .ok_or(root_too_large())?
            .checked_add(4)
            .ok_or(root_too_large())?;
        if sum > limits.ask_bytes {
            return Err(root_too_large());
        }
        let row = w::OwnerResourceScope::decode(&limits.settings, &mut Reader::new(&item))?;
        let row = documents_owner_resource_scope_fields(&row, limits);
        if rows.push(row).is_err() {
            return Err(root_too_large());
        }
    }
    Err(root_too_large())
}

pub(crate) fn documents_owner_resource_scope_fields(
    value: &w::OwnerResourceScope,
    limits: &Limits,
) -> c::OwnerResourceScope {
    c::OwnerResourceScope { connector: value.connector(), pattern: answers_owner_pattern(value.pattern(), limits) }
}

pub(crate) fn answers_owner_pattern(value: &w::OwnerPattern, limits: &Limits) -> c::OwnerPattern {
    let mut list0 = List::with_capacity(value.segments().len());
    for item in value.segments() {
        list0.push(bytes::copy_of(item)).expect("reserved supplied list");
    }
    c::OwnerPattern { segments: list0, last: answers_owner_pattern_end(value.last(), limits) }
}

pub(crate) fn answers_owner_pattern_end(value: &w::OwnerPatternEnd, _limits: &Limits) -> c::OwnerPatternEnd {
    match value {
        w::OwnerPatternEnd::Exact(row) => c::OwnerPatternEnd::Exact(bytes::copy_of(row.value())),
        w::OwnerPatternEnd::Open(row) => c::OwnerPatternEnd::Open(bytes::copy_of(row.value())),
    }
}

pub(crate) fn root_piece_problem(reason: skein_codec::Reason) -> w::Problem {
    w::Problem { path: w::Path::OwnedRecord, reason }
}

pub(crate) fn documents_owner_delegation_fields(
    value: &w::OwnerDelegation,
    limits: &Limits,
) -> Result<c::OwnerDelegation, w::Problem> {
    if value.kinds().len() > limits.authority {
        return Err(root_too_large());
    }
    let mut list0 = List::with_capacity(value.kinds().len());
    for item in value.kinds() {
        list0.push(documents_owner_authority_executor_fields(item, limits)).expect("reserved supplied list");
    }
    Ok(c::OwnerDelegation { kinds: list0, tasks: value.tasks(), depth: value.depth() })
}

pub(crate) fn documents_owner_authority_executor_fields(
    value: &w::OwnerAuthorityExecutor,
    _limits: &Limits,
) -> c::OwnerAuthorityExecutor {
    match value {
        w::OwnerAuthorityExecutor::Charter(row) => c::OwnerAuthorityExecutor::Charter(row.value()),
        w::OwnerAuthorityExecutor::Procedure(row) => c::OwnerAuthorityExecutor::Procedure(row.value()),
        w::OwnerAuthorityExecutor::Role(row) => c::OwnerAuthorityExecutor::Role(row.value()),
    }
}

pub(crate) fn documents_owner_grant_piece(
    bytes: &[u8],
    limits: &Limits,
    bound: u32,
) -> Result<List<c::OwnerGrant>, w::Problem> {
    let mut reader = Reader::new(bytes);
    let mut rows = List::with_capacity(bound);
    let mut sum = 0_u64;
    for _ in 0..=bound {
        let item = match skein_codec::piece::split(&mut reader) {
            Ok(value) => value,
            Err(reason) => return Err(root_piece_problem(reason)),
        };
        let Some(item) = item else {
            return Ok(rows);
        };
        sum = sum
            .checked_add(u64::try_from(item.len()).expect("piece fits u64"))
            .ok_or(root_too_large())?
            .checked_add(4)
            .ok_or(root_too_large())?;
        if sum > limits.ask_bytes {
            return Err(root_too_large());
        }
        let row = w::OwnerGrant::decode(&limits.settings, &mut Reader::new(&item))?;
        let row = documents_owner_grant_fields(&row, limits);
        if rows.push(row).is_err() {
            return Err(root_too_large());
        }
    }
    Err(root_too_large())
}

pub(crate) fn documents_owner_grant_fields(value: &w::OwnerGrant, limits: &Limits) -> c::OwnerGrant {
    c::OwnerGrant {
        connector: value.connector(),
        kind: value.kind(),
        pattern: answers_owner_pattern(value.pattern(), limits),
    }
}

pub(crate) fn model_sets_decode(value: Option<&w::ModelSet>) -> Result<Option<c::AuthorityModels>, w::Problem> {
    Ok(match value {
        None => None,
        Some(w::ModelSet::All) => Some(c::AuthorityModels::All),
        Some(w::ModelSet::Names(value)) => {
            model_sets_validate(value.names())?;
            Some(c::AuthorityModels::Named(value.names().clone()))
        }
    })
}

pub(crate) fn model_sets_validate(names: &List<Box<[u8]>>) -> Result<(), w::Problem> {
    let bad = w::Problem { path: w::Path::ModelSetNamesNames, reason: skein_codec::Reason::Bound };
    if names.len() > c::AUTHORITY_MODELS {
        return Err(bad);
    }
    for (at, name) in names.iter().enumerate() {
        if name.is_empty()
            || name.len() > usize::try_from(c::AUTHORITY_MODEL_BYTES).expect("name bound")
            || !skein_codec::text_is_valid(name)
        {
            return Err(bad);
        }
        for earlier in names.iter().take(at) {
            if earlier == name {
                return Err(bad);
            }
        }
    }
    Ok(())
}

pub(crate) fn documents_policy_requirement_fields(
    value: &w::PolicyRequirement,
    limits: &Limits,
) -> c::PolicyRequirement {
    c::PolicyRequirement {
        connector: value.connector(),
        kind: value.kind(),
        pattern: answers_owner_pattern(value.pattern(), limits),
        judge: documents_policy_judge_fields(value.judge(), limits),
        guard: documents_policy_guard_fields(value.guard(), limits),
        must_be_guarded: value.must_be_guarded(),
    }
}

pub(crate) fn documents_policy_guard_fields(value: &w::PolicyGuard, _limits: &Limits) -> c::PolicyGuard {
    match value {
        w::PolicyGuard::Guarded => c::PolicyGuard::Guarded,
        w::PolicyGuard::Observed(row) => c::PolicyGuard::Observed { freshness: row.freshness() },
    }
}

pub(crate) fn documents_policy_judge_fields(value: &w::PolicyJudge, _limits: &Limits) -> c::PolicyJudge {
    c::PolicyJudge { connector: value.connector(), requirement: value.requirement(), parameters: value.parameters() }
}

pub(crate) fn settings_restore_setting_policy(
    value: &w::SettingPolicy,
    limits: &Limits,
) -> Result<c::SettingPolicy, w::Problem> {
    let mut projections = List::with_capacity(value.projections().len());
    for item in value.projections() {
        projections.push(documents_owner_grant_fields(item, limits)).expect("same capacity");
    }
    let mut roles = List::with_capacity(value.roles().len());
    for item in value.roles() {
        roles.push(settings_restore_policy_edit_role(item, limits)?).expect("same capacity");
    }
    let mut requirements = List::with_capacity(value.requirements().len());
    for item in value.requirements() {
        requirements.push(documents_policy_requirement_fields(item, limits)).expect("same capacity");
    }
    let mut permissions = List::with_capacity(value.permissions().len());
    for item in value.permissions() {
        permissions.push(documents_policy_permission_fields(item, limits)).expect("same capacity");
    }
    let mut templates = List::with_capacity(value.templates().len());
    for item in value.templates() {
        templates.push(settings_restore_template_override(item)).expect("same capacity");
    }
    Ok(c::SettingPolicy {
        escalation_role: *value.escalation_role(),
        ceiling: documents_owner_authority_fields(value.ceiling(), limits)?,
        projections,
        period_spend: value.period_spend(),
        roles,
        requirements,
        permissions,
        templates,
    })
}

pub(crate) fn settings_restore_template_override(value: &w::TemplateOverride) -> c::TemplateOverride {
    c::TemplateOverride {
        template: value.template(),
        executor: optional_executor(value.executor().as_ref()),
        budget: optional_budget(value.budget().as_ref()),
        deadline: optional_deadline(value.deadline().as_ref()),
        priority: *value.priority(),
    }
}

pub(crate) fn operations_restore_deadline(value: &w::TemplateDeadline) -> c::TemplateDeadline {
    match value {
        w::TemplateDeadline::None => c::TemplateDeadline::None,
        w::TemplateDeadline::Span(v) => c::TemplateDeadline::Span(v.value()),
        w::TemplateDeadline::Given => c::TemplateDeadline::Given,
    }
}

pub(crate) fn operations_restore_budget(value: &w::TemplateBudget) -> c::TemplateBudget {
    match value {
        w::TemplateBudget::Amount(v) => c::TemplateBudget::Amount(v.value()),
        w::TemplateBudget::AsAllowed(v) => c::TemplateBudget::AsAllowed { cap: *v.cap() },
        w::TemplateBudget::Given => c::TemplateBudget::Given,
    }
}

pub(crate) fn operations_restore_executor(value: &w::TemplateExecutor) -> c::TemplateExecutor {
    match value {
        w::TemplateExecutor::Charter(v) => c::TemplateExecutor::Charter(v.value()),
        w::TemplateExecutor::Chosen => c::TemplateExecutor::Chosen,
    }
}

pub(crate) fn documents_policy_permission_fields(value: &w::PolicyPermission, _limits: &Limits) -> c::PolicyPermission {
    c::PolicyPermission { connector: value.connector(), permission: value.permission(), role: value.role() }
}

pub(crate) fn settings_restore_policy_edit_role(
    value: &w::PolicyEditRole,
    limits: &Limits,
) -> Result<c::PolicyEditRole, w::Problem> {
    Ok(c::PolicyEditRole {
        number: value.number(),
        authority: documents_owner_authority_fields(value.authority(), limits)?,
        period_spend: value.period_spend(),
        requests: value.requests(),
        decides: value.decides(),
    })
}

pub(crate) fn documents_me_template(value: &w::Template, limits: &Limits) -> Result<c::Template, w::Problem> {
    Ok(c::Template {
        number: value.number(),
        name: facts_document_text(value.name(), limits)?,
        executor: operations_restore_executor(value.executor()),
        contract: documents_task_contract_document_fields(value.contract(), limits)?,
        shallower: value.shallower(),
        budget: operations_restore_budget(value.budget()),
        deadline: operations_restore_deadline(value.deadline()),
        tracked: *value.tracked(),
        flags: c::TaskFlags {
            ended_by: match value.flags().ended_by() {
                w::EndedBy::Result => c::EndedBy::Result,
                w::EndedBy::Requester => c::EndedBy::Requester,
            },
            conversation: value.flags().conversation(),
            listed: value.flags().listed(),
            retention: match value.flags().retention() {
                w::Retention::Kept => c::Retention::Kept,
                w::Retention::ForgottenOnceRead => c::Retention::ForgottenOnceRead,
            },
        },
        label: value.label(),
        grace: *value.grace(),
        llm: llm_choice(value.llm().as_ref(), limits)?,
    })
}

pub(crate) fn llm_choice(value: Option<&w::LlmChoice>, limits: &Limits) -> Result<Option<c::LlmChoice>, w::Problem> {
    let Some(value) = value else {
        return Ok(None);
    };
    let mut subagents = List::with_capacity(value.subagents().len());
    for item in value.subagents() {
        subagents.push(llm_model_effort(item, limits)?).expect("same capacity");
    }
    Ok(Some(c::LlmChoice { account: *value.account(), main: llm_model_effort(value.main(), limits)?, subagents }))
}

pub(crate) fn llm_model_effort(value: &w::ModelEffort, limits: &Limits) -> Result<c::ModelEffort, w::Problem> {
    Ok(c::ModelEffort {
        model: llm_name(value.model(), limits.settings.model_effort_model)?,
        effort: match value.effort() {
            Some(effort) => Some(llm_name(effort, limits.settings.model_effort_effort_some)?),
            None => None,
        },
    })
}

pub(crate) fn llm_name(value: &[u8], limit: u32) -> Result<c::Text, w::Problem> {
    match c::Text::new(bytes::copy_of(value), limit) {
        Ok(text) => Ok(text),
        Err(_) => Err(root_too_large()),
    }
}

pub(crate) fn documents_task_contract_document_fields(
    value: &w::TaskContractDocument,
    limits: &Limits,
) -> Result<c::TaskContractDocument, w::Problem> {
    Ok(match value {
        w::TaskContractDocument::Approval(row) => c::TaskContractDocument::Approval {
            state: documents_raw(row.state(), limits)?,
            part: facts_owner_part(row.part(), limits),
        },
        w::TaskContractDocument::Report(row) => c::TaskContractDocument::Report { words: row.words() },
        w::TaskContractDocument::Verdict(row) => c::TaskContractDocument::Verdict {
            choices: documents_verdict_choice_piece(row.choices(), limits, limits.rows)?,
            cut: row.cut(),
        },
        w::TaskContractDocument::Change(row) => {
            c::TaskContractDocument::Change { connector: row.connector(), kind: row.kind(), words: row.words() }
        }
    })
}

pub(crate) fn documents_verdict_choice_piece(
    bytes: &[u8],
    limits: &Limits,
    bound: u32,
) -> Result<List<c::VerdictChoice>, w::Problem> {
    let mut reader = Reader::new(bytes);
    let mut rows = List::with_capacity(bound);
    let mut sum = 0_u64;
    for _ in 0..=bound {
        let item = match skein_codec::piece::split(&mut reader) {
            Ok(value) => value,
            Err(reason) => return Err(root_piece_problem(reason)),
        };
        let Some(item) = item else {
            return Ok(rows);
        };
        sum = sum
            .checked_add(u64::try_from(item.len()).expect("piece fits u64"))
            .ok_or(root_too_large())?
            .checked_add(4)
            .ok_or(root_too_large())?;
        if sum > limits.ask_bytes {
            return Err(root_too_large());
        }
        let row = w::VerdictChoice::decode(&limits.settings, &mut Reader::new(&item))?;
        let row = documents_verdict_choice_fields(&row, limits)?;
        if rows.push(row).is_err() {
            return Err(root_too_large());
        }
    }
    Err(root_too_large())
}

pub(crate) fn documents_verdict_choice_fields(
    value: &w::VerdictChoice,
    limits: &Limits,
) -> Result<c::VerdictChoice, w::Problem> {
    if value.fields().len() > limits.fields {
        return Err(root_too_large());
    }
    let mut list0 = List::with_capacity(value.fields().len());
    for item in value.fields() {
        list0.push(documents_field_rule_fields(item, limits)?).expect("reserved supplied list");
    }
    Ok(c::VerdictChoice { code: value.code(), words: value.words(), followups: value.followups(), fields: list0 })
}

pub(crate) fn documents_field_rule_fields(value: &w::FieldRule, limits: &Limits) -> Result<c::FieldRule, w::Problem> {
    Ok(c::FieldRule { name: documents_raw(value.name(), limits)?, max: value.max() })
}

pub(crate) fn documents_raw(value: &[u8], limits: &Limits) -> Result<Box<[u8]>, w::Problem> {
    if u64::try_from(value.len()).expect("usize fits u64") > u64::from(limits.text) {
        return Err(root_too_large());
    }
    Ok(bytes::copy_of(value))
}

pub(crate) fn facts_owner_part(value: &w::OwnerPart, limits: &Limits) -> c::OwnerPart {
    c::OwnerPart {
        connector: value.connector(),
        kind: facts_owner_part_kind(value.kind(), limits),
        key: facts_owner_part_key(value.key(), limits),
    }
}

pub(crate) fn facts_owner_part_key(value: &w::OwnerPartKey, _limits: &Limits) -> c::OwnerPartKey {
    match value {
        w::OwnerPartKey::Path(row) => {
            let mut list0 = List::with_capacity(row.value().len());
            for item in row.value() {
                list0.push(bytes::copy_of(item)).expect("reserved supplied list");
            }
            c::OwnerPartKey::Path(list0)
        }
        w::OwnerPartKey::Number(row) => c::OwnerPartKey::Number(row.value()),
    }
}

pub(crate) fn facts_owner_part_kind(value: &w::OwnerPartKind, _limits: &Limits) -> c::OwnerPartKind {
    match value {
        w::OwnerPartKind::Resource => c::OwnerPartKind::Resource,
        w::OwnerPartKind::Pool => c::OwnerPartKind::Pool,
        w::OwnerPartKind::Result => c::OwnerPartKind::Result,
        w::OwnerPartKind::Procedure => c::OwnerPartKind::Procedure,
        w::OwnerPartKind::Effect => c::OwnerPartKind::Effect,
        w::OwnerPartKind::Outbox => c::OwnerPartKind::Outbox,
        w::OwnerPartKind::Hold => c::OwnerPartKind::Hold,
    }
}

pub(crate) fn facts_document_text(value: &w::DocumentText, limits: &Limits) -> Result<c::DocumentText, w::Problem> {
    Ok(c::DocumentText {
        bytes: match c::Text::new(bytes::copy_of(value.bytes()), limits.text) {
            Ok(value) => value,
            Err(problem) => return Err(root_text_problem(problem)),
        },
        cut: value.cut(),
    })
}

pub(crate) fn root_text_problem(_: Box<[u8]>) -> w::Problem {
    root_too_large()
}

pub(crate) fn settings_restore_charter(value: &w::Charter, limits: &Limits) -> Result<c::Charter, w::Problem> {
    let mut alternatives = List::with_capacity(value.alternatives().len());
    for item in value.alternatives() {
        alternatives.push(settings_restore_setting_model(item, limits)?).expect("same capacity");
    }
    Ok(c::Charter {
        switches: [value.resume(), value.inspect(), value.modify(), value.shell(), value.agents()],
        number: value.number(),
        instructions: settings_text(value.instructions(), 1_114_240)?,
        waiting: value.waiting(),
        turns: value.turns(),
        time: value.time(),
        reserve_turns: value.reserve_turns(),
        reserve_time: value.reserve_time(),
        model: settings_restore_setting_model(value.model(), limits)?,
        alternatives,
        call_timeout: value.call_timeout(),
        hosts: settings_restore_setting_hosts(value.hosts(), limits),
        spend: value.spend(),
    })
}

pub(crate) fn settings_restore_setting_hosts(value: &w::SettingHosts, _limits: &Limits) -> c::SettingHosts {
    match value {
        w::SettingHosts::Workers => c::SettingHosts::Workers,
        w::SettingHosts::Engine => c::SettingHosts::Engine,
        w::SettingHosts::Both => c::SettingHosts::Both,
    }
}

pub(crate) fn settings_restore_setting_model(
    value: &w::SettingModel,
    _limits: &Limits,
) -> Result<c::SettingModel, w::Problem> {
    let mut efforts = List::with_capacity(value.efforts().len());
    for item in value.efforts() {
        efforts.push(settings_text(item, 32)?).expect("same capacity");
    }
    Ok(c::SettingModel {
        dialect: value.dialect(),
        endpoint: value.endpoint(),
        program: match value.program() {
            Some(item) => Some(settings_text(item, 32)?),
            None => None,
        },
        name: settings_text(value.name(), 4096)?,
        window: value.window(),
        output: value.output(),
        input_price: value.input_price(),
        cached_price: value.cached_price(),
        cache_write_price: *value.cache_write_price(),
        output_price: value.output_price(),
        price_unit: value.price_unit(),
        efforts,
        effort: match value.effort() {
            Some(item) => Some(settings_text(item, 32)?),
            None => None,
        },
    })
}

pub(crate) fn settings_text(value: &[u8], bound: u32) -> Result<c::Text, w::Problem> {
    match c::Text::new(bytes::copy_of(value), bound) {
        Ok(value) => Ok(value),
        Err(_) => Err(root_too_large()),
    }
}

pub(crate) fn settings_item_count(values: &[u8], limits: &Limits) -> Result<u32, w::Problem> {
    if u64::try_from(values.len()).expect("piece bytes fit u64") > limits.ask_bytes {
        return Err(root_too_large());
    }
    let mut reader = Reader::new(values);
    for count in 0_u32..=1024 {
        let piece = match skein_codec::piece::split(&mut reader) {
            Ok(value) => value,
            Err(reason) => return Err(root_piece_problem(reason)),
        };
        if piece.is_none() {
            return Ok(count);
        }
    }
    Err(root_too_large())
}

pub fn restore_setting_document(value: &w::SettingDocument, limits: &Limits) -> Result<c::SettingDocument, w::Problem> {
    Ok(c::SettingDocument {
        setting: settings_restore_setting(value.setting(), limits),
        revision: value.revision(),
        value: settings_restore_setting_value(value.value(), limits)?,
    })
}

pub fn restore_settings_document(
    value: &w::SettingsDocument,
    limits: &Limits,
) -> Result<c::SettingsDocument, w::Problem> {
    let mut settings = List::with_capacity(value.settings().len());
    for item in value.settings() {
        settings.push(settings_restore_setting_line(item, limits)?).expect("same capacity");
    }
    let mut held = List::with_capacity(value.held().len());
    for item in value.held() {
        held.push(settings_restore_held_revision(item, limits)).expect("same capacity");
    }
    let mut pending = List::with_capacity(value.pending().len());
    for item in value.pending() {
        pending.push(settings_restore_settings_proposal(item, limits)).expect("same capacity");
    }
    Ok(c::SettingsDocument { newest: value.newest(), settings, held, pending })
}

pub(crate) fn settings_restore_settings_proposal(value: &w::SettingsProposal, limits: &Limits) -> c::SettingsProposal {
    c::SettingsProposal { proposer: answers_proposer(value.proposer(), limits), number: value.number() }
}

pub(crate) fn settings_restore_held_revision(value: &w::HeldRevision, limits: &Limits) -> c::HeldRevision {
    c::HeldRevision { head: settings_restore_revision_head(value.head(), limits), holders: value.holders() }
}

pub(crate) fn settings_restore_setting_line(
    value: &w::SettingLine,
    limits: &Limits,
) -> Result<c::SettingLine, w::Problem> {
    Ok(c::SettingLine {
        setting: settings_restore_setting(value.setting(), limits),
        name: match value.name() {
            Some(item) => Some(facts_document_text(item, limits)?),
            None => None,
        },
        follows: settings_restore_setting_follows(value.follows(), limits),
        last: value.last(),
    })
}

pub(crate) fn settings_restore_setting_follows(value: &w::SettingFollows, _limits: &Limits) -> c::SettingFollows {
    match value {
        w::SettingFollows::Default => c::SettingFollows::Default,
        w::SettingFollows::Change => c::SettingFollows::Change,
    }
}

pub fn wire_change_settings(value: &c::ChangeSettings, limits: &w::Limits) -> Result<w::ChangeSettings, w::Problem> {
    w::ChangeSettings::new(
        limits,
        w::ChangeSettingsParts {
            against: value.against,
            items: settings_encode_setting_item_pieces(&value.items, limits)?,
            running: settings_wire_running(&value.running, limits),
        },
    )
}

pub(crate) fn settings_wire_running(value: &c::Running, _limits: &w::Limits) -> w::Running {
    match value {
        c::Running::Stays => w::Running::Stays,
        c::Running::Moves => w::Running::Moves,
    }
}

pub(crate) fn settings_encode_setting_item_pieces(
    values: &List<c::SettingItem>,
    limits: &w::Limits,
) -> Result<Box<[u8]>, w::Problem> {
    if values.len() > 1024 {
        return Err(root_too_large());
    }
    let mut rows = List::with_capacity(values.len());
    let mut size = 0_u32;
    for value in values {
        let row = settings_wire_setting_item(value, limits)?;
        size = size.checked_add(row.measure()).ok_or(root_too_large())?.checked_add(4).ok_or(root_too_large())?;
        if size > limits.change_settings_items {
            return Err(root_too_large());
        }
        rows.push(row).expect("same capacity");
    }
    let mut writer = Writer::new(usize::try_from(size).expect("wire size fits"));
    for row in rows.into_boxed() {
        writer.put(&row.measure().to_be_bytes()).expect("measured");
        row.encode(&mut writer).expect("measured");
    }
    Ok(writer.finish())
}

pub(crate) fn settings_wire_setting_item(
    value: &c::SettingItem,
    limits: &w::Limits,
) -> Result<w::SettingItem, w::Problem> {
    w::SettingItem::new(
        limits,
        w::SettingItemParts {
            setting: settings_wire_setting(value.setting, limits)?,
            edit: settings_wire_setting_edit(&value.edit, limits)?,
        },
    )
}

pub(crate) fn settings_wire_setting_edit(
    value: &c::SettingEdit,
    limits: &w::Limits,
) -> Result<w::SettingEdit, w::Problem> {
    Ok(match value {
        c::SettingEdit::Set(item) => w::SettingEdit::Set(settings_wire_setting_set(item, limits)?),
        c::SettingEdit::Removed => w::SettingEdit::Removed,
        c::SettingEdit::Reset => w::SettingEdit::Reset,
    })
}

pub(crate) fn settings_wire_setting_set(
    value: &c::SettingSet,
    limits: &w::Limits,
) -> Result<w::SettingSet, w::Problem> {
    w::SettingSet::new(limits, w::SettingSetParts { value: settings_wire_setting_value(&value.value, limits)? })
}

pub(crate) fn settings_wire_setting_value(
    value: &c::SettingValue,
    limits: &w::Limits,
) -> Result<w::SettingValue, w::Problem> {
    Ok(match value {
        c::SettingValue::Charter(item) => w::SettingValue::Charter(Box::new(settings_wire_charter(item, limits)?)),
        c::SettingValue::Template(item) => w::SettingValue::Template(Box::new(settings_wire_template(item, limits)?)),
        c::SettingValue::Policy(item) => w::SettingValue::Policy(Box::new(settings_wire_setting_policy(item, limits)?)),
        c::SettingValue::Rules(item) => w::SettingValue::Rules(Box::new(settings_wire_rules(item, limits)?)),
    })
}

pub(crate) fn settings_wire_rules(value: &c::Rules, limits: &w::Limits) -> Result<w::Rules, w::Problem> {
    let mut implies = List::with_capacity(value.implies.len());
    for item in &value.implies {
        implies
            .push(w::OwnerKindImplication::new(
                limits,
                w::OwnerKindImplicationParts { connector: item.connector, kind: item.kind, implies: item.implies },
            )?)
            .expect("same capacity");
    }
    let mut requirements = List::with_capacity(value.requirements.len());
    for item in &value.requirements {
        requirements.push(asks_wire_policy_requirement(item, limits)?).expect("same capacity");
    }
    w::Rules::new(
        limits,
        w::RulesParts {
            ceiling: asks_wire_owner_authority(&value.ceiling, limits)?,
            period_spend: value.period_spend,
            minimum_run_spend: value.minimum_run_spend,
            maximum_run_spend: value.maximum_run_spend,
            implies,
            requirements,
        },
    )
}

pub(crate) fn asks_wire_owner_authority(
    value: &c::OwnerAuthority,
    limits: &w::Limits,
) -> Result<w::OwnerAuthority, w::Problem> {
    let mut list0 = List::with_capacity(value.grants.len());
    for item in &value.grants {
        list0.push(asks_wire_owner_grant(item, limits)?).expect("reserved supplied list");
    }
    let opt1 = optional_nanos(value.deadline);
    let mut list2 = List::with_capacity(value.note_resources.len());
    for item in &value.note_resources {
        list2.push(asks_wire_owner_resource_scope(item, limits)?).expect("reserved supplied list");
    }
    w::OwnerAuthority::new(
        limits,
        w::OwnerAuthorityParts {
            models: model_sets_encode(value.models.as_ref(), limits)?,
            tools: value.tools,
            grants: asks_wire_owner_grant_piece(&list0, limits)?,
            delegation: asks_wire_owner_delegation(&value.delegation, limits)?,
            spend: value.spend,
            deadline: opt1,
            notes: value.notes,
            note_resources: asks_wire_owner_resource_scope_piece(&list2, limits)?,
        },
    )
}

pub(crate) fn asks_wire_owner_resource_scope_piece(
    rows: &List<w::OwnerResourceScope>,
    limits: &w::Limits,
) -> Result<Box<[u8]>, w::Problem> {
    let mut length = 0_u32;
    for row in rows {
        length = length.checked_add(row.measure()).ok_or(root_too_large())?;
        length = length.checked_add(4).ok_or(root_too_large())?;
    }
    if length > limits.owned_record {
        return Err(root_too_large());
    }
    let mut writer = Writer::new(usize::try_from(length).expect("u32 fits usize"));
    for row in rows {
        let mut item = Writer::new(usize::try_from(row.measure()).expect("u32 fits usize"));
        row.encode(&mut item).expect("measured row");
        let item = item.finish();
        skein_codec::piece::write(&mut writer, &item).expect("measured piece");
    }
    Ok(writer.finish())
}

pub(crate) fn asks_wire_owner_delegation(
    value: &c::OwnerDelegation,
    limits: &w::Limits,
) -> Result<w::OwnerDelegation, w::Problem> {
    let mut list0 = List::with_capacity(value.kinds.len());
    for item in &value.kinds {
        list0.push(asks_wire_owner_authority_executor(*item, limits)?).expect("reserved supplied list");
    }
    w::OwnerDelegation::new(limits, w::OwnerDelegationParts { kinds: list0, tasks: value.tasks, depth: value.depth })
}

pub(crate) fn asks_wire_owner_authority_executor(
    value: c::OwnerAuthorityExecutor,
    limits: &w::Limits,
) -> Result<w::OwnerAuthorityExecutor, w::Problem> {
    Ok(match value {
        c::OwnerAuthorityExecutor::Charter(value) => w::OwnerAuthorityExecutor::Charter(
            w::OwnerAuthorityExecutorCharter::new(limits, w::OwnerAuthorityExecutorCharterParts { value })?,
        ),
        c::OwnerAuthorityExecutor::Procedure(value) => w::OwnerAuthorityExecutor::Procedure(
            w::OwnerAuthorityExecutorProcedure::new(limits, w::OwnerAuthorityExecutorProcedureParts { value })?,
        ),
        c::OwnerAuthorityExecutor::Role(value) => w::OwnerAuthorityExecutor::Role(w::OwnerAuthorityExecutorRole::new(
            limits,
            w::OwnerAuthorityExecutorRoleParts { value },
        )?),
    })
}

pub(crate) fn asks_wire_owner_grant_piece(
    rows: &List<w::OwnerGrant>,
    limits: &w::Limits,
) -> Result<Box<[u8]>, w::Problem> {
    let mut length = 0_u32;
    for row in rows {
        length = length.checked_add(row.measure()).ok_or(root_too_large())?;
        length = length.checked_add(4).ok_or(root_too_large())?;
    }
    if length > limits.owned_record {
        return Err(root_too_large());
    }
    let mut writer = Writer::new(usize::try_from(length).expect("u32 fits usize"));
    for row in rows {
        let mut item = Writer::new(usize::try_from(row.measure()).expect("u32 fits usize"));
        row.encode(&mut item).expect("measured row");
        let item = item.finish();
        skein_codec::piece::write(&mut writer, &item).expect("measured piece");
    }
    Ok(writer.finish())
}

pub(crate) fn model_sets_encode(
    value: Option<&c::AuthorityModels>,
    limits: &w::Limits,
) -> Result<Option<w::ModelSet>, w::Problem> {
    Ok(match value {
        None => None,
        Some(c::AuthorityModels::All) => Some(w::ModelSet::All),
        Some(c::AuthorityModels::Named(names)) => {
            model_sets_validate(names)?;
            Some(w::ModelSet::Names(w::ModelSetNames::new(limits, w::ModelSetNamesParts { names: names.clone() })?))
        }
    })
}

pub(crate) fn asks_wire_owner_resource_scope(
    value: &c::OwnerResourceScope,
    limits: &w::Limits,
) -> Result<w::OwnerResourceScope, w::Problem> {
    w::OwnerResourceScope::new(
        limits,
        w::OwnerResourceScopeParts {
            connector: value.connector,
            pattern: asks_wire_owner_pattern(&value.pattern, limits)?,
        },
    )
}

pub(crate) fn asks_wire_owner_pattern(
    value: &c::OwnerPattern,
    limits: &w::Limits,
) -> Result<w::OwnerPattern, w::Problem> {
    let mut list0 = List::with_capacity(value.segments.len());
    for item in &value.segments {
        list0.push(bytes::copy_of(item)).expect("reserved supplied list");
    }
    w::OwnerPattern::new(
        limits,
        w::OwnerPatternParts { segments: list0, last: asks_wire_owner_pattern_end(&value.last, limits)? },
    )
}

pub(crate) fn asks_wire_owner_pattern_end(
    value: &c::OwnerPatternEnd,
    limits: &w::Limits,
) -> Result<w::OwnerPatternEnd, w::Problem> {
    Ok(match value {
        c::OwnerPatternEnd::Exact(value) => w::OwnerPatternEnd::Exact(w::OwnerPatternEndExact::new(
            limits,
            w::OwnerPatternEndExactParts { value: bytes::copy_of(value) },
        )?),
        c::OwnerPatternEnd::Open(value) => w::OwnerPatternEnd::Open(w::OwnerPatternEndOpen::new(
            limits,
            w::OwnerPatternEndOpenParts { value: bytes::copy_of(value) },
        )?),
    })
}

pub(crate) fn asks_wire_owner_grant(value: &c::OwnerGrant, limits: &w::Limits) -> Result<w::OwnerGrant, w::Problem> {
    w::OwnerGrant::new(
        limits,
        w::OwnerGrantParts {
            connector: value.connector,
            kind: value.kind,
            pattern: asks_wire_owner_pattern(&value.pattern, limits)?,
        },
    )
}

pub(crate) fn asks_wire_policy_requirement(
    value: &c::PolicyRequirement,
    limits: &w::Limits,
) -> Result<w::PolicyRequirement, w::Problem> {
    w::PolicyRequirement::new(
        limits,
        w::PolicyRequirementParts {
            connector: value.connector,
            kind: value.kind,
            pattern: asks_wire_owner_pattern(&value.pattern, limits)?,
            judge: asks_wire_policy_judge(value.judge, limits)?,
            guard: asks_wire_policy_guard(&value.guard, limits)?,
            must_be_guarded: value.must_be_guarded,
        },
    )
}

pub(crate) fn asks_wire_policy_guard(value: &c::PolicyGuard, limits: &w::Limits) -> Result<w::PolicyGuard, w::Problem> {
    Ok(match value {
        c::PolicyGuard::Guarded => w::PolicyGuard::Guarded,
        c::PolicyGuard::Observed { freshness } => w::PolicyGuard::Observed(w::PolicyGuardObserved::new(
            limits,
            w::PolicyGuardObservedParts { freshness: *freshness },
        )?),
    })
}

pub(crate) fn asks_wire_policy_judge(value: c::PolicyJudge, limits: &w::Limits) -> Result<w::PolicyJudge, w::Problem> {
    w::PolicyJudge::new(
        limits,
        w::PolicyJudgeParts {
            connector: value.connector,
            requirement: value.requirement,
            parameters: value.parameters,
        },
    )
}

pub(crate) fn settings_wire_setting_policy(
    value: &c::SettingPolicy,
    limits: &w::Limits,
) -> Result<w::SettingPolicy, w::Problem> {
    let mut projections = List::with_capacity(value.projections.len());
    for item in &value.projections {
        projections.push(asks_wire_owner_grant(item, limits)?).expect("same capacity");
    }
    let mut roles = List::with_capacity(value.roles.len());
    for item in &value.roles {
        roles.push(asks_wire_policy_edit_role(item, limits)?).expect("same capacity");
    }
    let mut requirements = List::with_capacity(value.requirements.len());
    for item in &value.requirements {
        requirements.push(asks_wire_policy_requirement(item, limits)?).expect("same capacity");
    }
    let mut permissions = List::with_capacity(value.permissions.len());
    for item in &value.permissions {
        permissions.push(asks_wire_policy_permission(*item, limits)?).expect("same capacity");
    }
    let mut templates = List::with_capacity(value.templates.len());
    for item in &value.templates {
        templates.push(settings_wire_template_override(item, limits)?).expect("same capacity");
    }
    w::SettingPolicy::new(
        limits,
        w::SettingPolicyParts {
            escalation_role: value.escalation_role,
            ceiling: asks_wire_owner_authority(&value.ceiling, limits)?,
            projections,
            period_spend: value.period_spend,
            roles,
            requirements,
            permissions,
            templates,
        },
    )
}

pub(crate) fn settings_wire_template_override(
    value: &c::TemplateOverride,
    limits: &w::Limits,
) -> Result<w::TemplateOverride, w::Problem> {
    w::TemplateOverride::new(
        limits,
        w::TemplateOverrideParts {
            template: value.template,
            executor: match value.executor {
                Some(value) => Some(operations_executor(value, limits)?),
                None => None,
            },
            budget: match value.budget {
                Some(value) => Some(operations_budget(value, limits)?),
                None => None,
            },
            deadline: match value.deadline {
                Some(value) => Some(operations_deadline(value, limits)?),
                None => None,
            },
            priority: value.priority,
        },
    )
}

pub(crate) fn operations_deadline(
    value: c::TemplateDeadline,
    limits: &w::Limits,
) -> Result<w::TemplateDeadline, w::Problem> {
    Ok(match value {
        c::TemplateDeadline::None => w::TemplateDeadline::None,
        c::TemplateDeadline::Span(value) => {
            w::TemplateDeadline::Span(w::TemplateSpan::new(limits, w::TemplateSpanParts { value })?)
        }
        c::TemplateDeadline::Given => w::TemplateDeadline::Given,
    })
}

pub(crate) fn operations_budget(value: c::TemplateBudget, limits: &w::Limits) -> Result<w::TemplateBudget, w::Problem> {
    Ok(match value {
        c::TemplateBudget::Amount(value) => {
            w::TemplateBudget::Amount(w::TemplateAmount::new(limits, w::TemplateAmountParts { value })?)
        }
        c::TemplateBudget::AsAllowed { cap } => {
            w::TemplateBudget::AsAllowed(w::TemplateAsAllowed::new(limits, w::TemplateAsAllowedParts { cap })?)
        }
        c::TemplateBudget::Given => w::TemplateBudget::Given,
    })
}

pub(crate) fn operations_executor(
    value: c::TemplateExecutor,
    limits: &w::Limits,
) -> Result<w::TemplateExecutor, w::Problem> {
    Ok(match value {
        c::TemplateExecutor::Charter(value) => {
            w::TemplateExecutor::Charter(w::TemplateCharter::new(limits, w::TemplateCharterParts { value })?)
        }
        c::TemplateExecutor::Chosen => w::TemplateExecutor::Chosen,
    })
}

pub(crate) fn asks_wire_policy_permission(
    value: c::PolicyPermission,
    limits: &w::Limits,
) -> Result<w::PolicyPermission, w::Problem> {
    w::PolicyPermission::new(
        limits,
        w::PolicyPermissionParts { connector: value.connector, permission: value.permission, role: value.role },
    )
}

pub(crate) fn asks_wire_policy_edit_role(
    value: &c::PolicyEditRole,
    limits: &w::Limits,
) -> Result<w::PolicyEditRole, w::Problem> {
    w::PolicyEditRole::new(
        limits,
        w::PolicyEditRoleParts {
            number: value.number,
            authority: asks_wire_owner_authority(&value.authority, limits)?,
            period_spend: value.period_spend,
            requests: value.requests,
            decides: value.decides,
        },
    )
}

pub(crate) fn settings_wire_template(value: &c::Template, limits: &w::Limits) -> Result<w::Template, w::Problem> {
    w::Template::new(
        limits,
        w::TemplateParts {
            number: value.number,
            name: settings_wire_document_text(&value.name, limits)?,
            executor: operations_executor(value.executor, limits)?,
            contract: settings_wire_task_contract(&value.contract, limits)?,
            shallower: value.shallower,
            budget: operations_budget(value.budget, limits)?,
            deadline: operations_deadline(value.deadline, limits)?,
            tracked: value.tracked,
            flags: w::TaskFlags::new(
                limits,
                w::TaskFlagsParts {
                    ended_by: match value.flags.ended_by {
                        c::EndedBy::Result => w::EndedBy::Result,
                        c::EndedBy::Requester => w::EndedBy::Requester,
                    },
                    conversation: value.flags.conversation,
                    listed: value.flags.listed,
                    retention: match value.flags.retention {
                        c::Retention::Kept => w::Retention::Kept,
                        c::Retention::ForgottenOnceRead => w::Retention::ForgottenOnceRead,
                    },
                },
            )?,
            label: value.label,
            grace: value.grace,
            llm: llm_wire_choice(value.llm.as_ref(), limits)?,
        },
    )
}

pub(crate) fn llm_wire_choice(
    value: Option<&c::LlmChoice>,
    limits: &w::Limits,
) -> Result<Option<w::LlmChoice>, w::Problem> {
    let Some(value) = value else {
        return Ok(None);
    };
    let mut subagents = List::with_capacity(value.subagents.len());
    for item in &value.subagents {
        subagents.push(llm_wire_model_effort(item, limits)?).expect("same capacity");
    }
    Ok(Some(w::LlmChoice::new(
        limits,
        w::LlmChoiceParts { account: value.account, main: llm_wire_model_effort(&value.main, limits)?, subagents },
    )?))
}

pub(crate) fn llm_wire_model_effort(value: &c::ModelEffort, limits: &w::Limits) -> Result<w::ModelEffort, w::Problem> {
    w::ModelEffort::new(
        limits,
        w::ModelEffortParts {
            model: bytes::copy_of(value.model.bytes()),
            effort: optional_text(value.effort.as_ref()),
        },
    )
}

pub(crate) fn settings_wire_task_contract(
    value: &c::TaskContractDocument,
    limits: &w::Limits,
) -> Result<w::TaskContractDocument, w::Problem> {
    Ok(match value {
        c::TaskContractDocument::Approval { state, part } => {
            w::TaskContractDocument::Approval(w::TaskContractDocumentApproval::new(
                limits,
                w::TaskContractDocumentApprovalParts {
                    state: bytes::copy_of(state),
                    part: settings_wire_owner_part(part, limits)?,
                },
            )?)
        }
        c::TaskContractDocument::Report { words } => w::TaskContractDocument::Report(
            w::TaskContractDocumentReport::new(limits, w::TaskContractDocumentReportParts { words: *words })?,
        ),
        c::TaskContractDocument::Change { connector, kind, words } => {
            w::TaskContractDocument::Change(w::TaskContractDocumentChange::new(
                limits,
                w::TaskContractDocumentChangeParts { connector: *connector, kind: *kind, words: *words },
            )?)
        }
        c::TaskContractDocument::Verdict { choices, cut } => {
            let mut rows = List::with_capacity(choices.len());
            let mut size = 0_u32;
            for choice in choices {
                let mut fields = List::with_capacity(choice.fields.len());
                for field in &choice.fields {
                    fields
                        .push(w::FieldRule::new(
                            limits,
                            w::FieldRuleParts { name: bytes::copy_of(&field.name), max: field.max },
                        )?)
                        .expect("same capacity");
                }
                let row = w::VerdictChoice::new(
                    limits,
                    w::VerdictChoiceParts {
                        code: choice.code,
                        words: choice.words,
                        followups: choice.followups,
                        fields,
                    },
                )?;
                size =
                    size.checked_add(row.measure()).ok_or(root_too_large())?.checked_add(4).ok_or(root_too_large())?;
                rows.push(row).expect("same capacity");
            }
            if size > limits.task_contract_document_verdict_choices {
                return Err(root_too_large());
            }
            let mut writer = Writer::new(usize::try_from(size).expect("wire size fits"));
            for row in &rows {
                writer.put(&row.measure().to_be_bytes()).expect("measured");
                row.encode(&mut writer).expect("measured");
            }
            w::TaskContractDocument::Verdict(w::TaskContractDocumentVerdict::new(
                limits,
                w::TaskContractDocumentVerdictParts { choices: writer.finish(), cut: *cut },
            )?)
        }
    })
}

pub(crate) fn settings_wire_owner_part(value: &c::OwnerPart, limits: &w::Limits) -> Result<w::OwnerPart, w::Problem> {
    let kind = match value.kind {
        c::OwnerPartKind::Resource => w::OwnerPartKind::Resource,
        c::OwnerPartKind::Pool => w::OwnerPartKind::Pool,
        c::OwnerPartKind::Result => w::OwnerPartKind::Result,
        c::OwnerPartKind::Procedure => w::OwnerPartKind::Procedure,
        c::OwnerPartKind::Effect => w::OwnerPartKind::Effect,
        c::OwnerPartKind::Outbox => w::OwnerPartKind::Outbox,
        c::OwnerPartKind::Hold => w::OwnerPartKind::Hold,
    };
    let key = match &value.key {
        c::OwnerPartKey::Number(value) => {
            w::OwnerPartKey::Number(w::OwnerPartKeyNumber::new(limits, w::OwnerPartKeyNumberParts { value: *value })?)
        }
        c::OwnerPartKey::Path(values) => {
            let mut rows = List::with_capacity(values.len());
            for value in values {
                rows.push(bytes::copy_of(value)).expect("same capacity");
            }
            w::OwnerPartKey::Path(w::OwnerPartKeyPath::new(limits, w::OwnerPartKeyPathParts { value: rows })?)
        }
    };
    w::OwnerPart::new(limits, w::OwnerPartParts { connector: value.connector, kind, key })
}

pub(crate) fn settings_wire_document_text(
    value: &c::DocumentText,
    limits: &w::Limits,
) -> Result<w::DocumentText, w::Problem> {
    w::DocumentText::new(limits, w::DocumentTextParts { bytes: bytes::copy_of(value.bytes()), cut: value.cut })
}

pub(crate) fn settings_wire_charter(value: &c::Charter, limits: &w::Limits) -> Result<w::Charter, w::Problem> {
    let mut alternatives = List::with_capacity(value.alternatives.len());
    for item in &value.alternatives {
        alternatives.push(settings_wire_setting_model(item, limits)?).expect("same capacity");
    }
    w::Charter::new(
        limits,
        w::CharterParts {
            skein_bools: [value.resume(), value.inspect(), value.modify(), value.shell(), value.agents()],
            number: value.number,
            instructions: bytes::copy_of(value.instructions.bytes()),
            waiting: value.waiting,
            turns: value.turns,
            time: value.time,
            reserve_turns: value.reserve_turns,
            reserve_time: value.reserve_time,
            model: settings_wire_setting_model(&value.model, limits)?,
            alternatives,
            call_timeout: value.call_timeout,
            hosts: settings_wire_setting_hosts(&value.hosts, limits),
            spend: value.spend,
        },
    )
}

pub(crate) fn settings_wire_setting_hosts(value: &c::SettingHosts, _limits: &w::Limits) -> w::SettingHosts {
    match value {
        c::SettingHosts::Workers => w::SettingHosts::Workers,
        c::SettingHosts::Engine => w::SettingHosts::Engine,
        c::SettingHosts::Both => w::SettingHosts::Both,
    }
}

pub(crate) fn settings_wire_setting_model(
    value: &c::SettingModel,
    limits: &w::Limits,
) -> Result<w::SettingModel, w::Problem> {
    let mut efforts = List::with_capacity(value.efforts.len());
    for item in &value.efforts {
        efforts.push(bytes::copy_of(item.bytes())).expect("same capacity");
    }
    w::SettingModel::new(
        limits,
        w::SettingModelParts {
            dialect: value.dialect,
            endpoint: value.endpoint,
            program: optional_text(value.program.as_ref()),
            name: bytes::copy_of(value.name.bytes()),
            window: value.window,
            output: value.output,
            input_price: value.input_price,
            cached_price: value.cached_price,
            cache_write_price: value.cache_write_price,
            output_price: value.output_price,
            price_unit: value.price_unit,
            efforts,
            effort: optional_text(value.effort.as_ref()),
        },
    )
}

pub(crate) fn settings_wire_setting(value: c::Setting, limits: &w::Limits) -> Result<w::Setting, w::Problem> {
    Ok(match value {
        c::Setting::Charter(item) => w::Setting::Charter(settings_wire_setting_number(item, limits)?),
        c::Setting::Template(item) => w::Setting::Template(settings_wire_setting_number(item, limits)?),
        c::Setting::Policy(item) => w::Setting::Policy(settings_wire_setting_number(item, limits)?),
        c::Setting::Rules => w::Setting::Rules,
    })
}

pub(crate) fn settings_wire_setting_number(
    value: c::SettingNumber,
    limits: &w::Limits,
) -> Result<w::SettingNumber, w::Problem> {
    w::SettingNumber::new(limits, w::SettingNumberParts { number: value.number })
}

fn optional_proposal(value: Option<&w::ProposalKey>, limits: &Limits) -> Option<c::ProposalKey> {
    let value = value?;
    Some(answers_proposal_key(value, limits))
}
fn optional_wall(value: Option<&u64>) -> Option<Wall> {
    let value = value?;
    Some(Wall::from_nanos(*value))
}
fn optional_nanos(value: Option<Wall>) -> Option<u64> {
    let value = value?;
    Some(value.as_nanos())
}
fn optional_executor(value: Option<&w::TemplateExecutor>) -> Option<c::TemplateExecutor> {
    let value = value?;
    Some(operations_restore_executor(value))
}
fn optional_budget(value: Option<&w::TemplateBudget>) -> Option<c::TemplateBudget> {
    let value = value?;
    Some(operations_restore_budget(value))
}
fn optional_deadline(value: Option<&w::TemplateDeadline>) -> Option<c::TemplateDeadline> {
    let value = value?;
    Some(operations_restore_deadline(value))
}
fn optional_text(value: Option<&c::Text>) -> Option<Box<[u8]>> {
    let value = value?;
    Some(bytes::copy_of(value.bytes()))
}

fn settings_restore_revision_replaced(
    value: &w::RevisionReplaced,
    limits: &Limits,
) -> Result<c::RevisionReplaced, w::Problem> {
    Ok(match value {
        w::RevisionReplaced::Value(item) => c::RevisionReplaced::Value(settings_restore_setting_set(item, limits)?),
        w::RevisionReplaced::TooLarge(item) => c::RevisionReplaced::TooLarge(settings_restore_size(item, limits)),
        w::RevisionReplaced::Default => c::RevisionReplaced::Default,
        w::RevisionReplaced::Removed => c::RevisionReplaced::Removed,
        w::RevisionReplaced::NotOffered => c::RevisionReplaced::NotOffered,
    })
}
