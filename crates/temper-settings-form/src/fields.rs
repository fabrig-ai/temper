//! The settings file's form, a row per shape and member (client/tools.md, 5.3).
mod jig;
pub mod render;
#[cfg(test)]
mod tests;

/// Each object the decoder reads, by the name the page gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    File,
    Item,
    Setting,
    Charter,
    Model,
    Template,
    Text,
    Executor,
    Contract,
    Approval,
    Part,
    Key,
    Report,
    Verdict,
    Choice,
    ChoiceField,
    Change,
    Budget,
    AsAllowed,
    Deadline,
    Flags,
    Llm,
    ModelEffort,
    Policy,
    Role,
    TemplateOverride,
    Permission,
    Rules,
    Implication,
    Authority,
    Models,
    Grant,
    Pattern,
    Last,
    Delegation,
    Delegate,
    Resource,
    Requirement,
    Judge,
    Guard,
    Observed,
}
/// A shape's own row.
#[derive(Debug)]
pub struct Form {
    pub shape: Shape,
    pub name: &'static [u8],
    pub choice: Choice,
    pub line: &'static [u8],
}
#[derive(Debug)]
pub enum Choice {
    Every,
    One { words: &'static [Word] },
}
#[derive(Debug)]
pub struct Word {
    pub word: &'static [u8],
    pub line: &'static [u8],
}
/// A member of a shape.
#[derive(Debug)]
pub struct Field {
    pub shape: Shape,
    pub name: &'static [u8],
    pub value: Value,
    pub list: Option<Bound>,
    pub null: bool,
    pub presence: Presence,
    pub line: &'static [u8],
}
/// Whether an absent member is refused, or takes a JSON default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presence {
    Required,
    Optional(Default),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Default {
    Null,
    EmptyList,
}
#[derive(Debug)]
pub enum Value {
    Integer(Width),
    Nanoseconds,
    Time,
    Boolean,
    Text(Bound),
    Hex(Bound),
    Words(&'static [Word]),
    Shape(Shape),
    Setting,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Width {
    U8,
    U16,
    U32,
    U64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bound {
    Most(u32),
    Limit(Limit),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Limit {
    CharterInstructions,
    CharterAlternatives,
    SettingModelProgramSome,
    SettingModelName,
    SettingModelEffortsItem,
    SettingModelEfforts,
    SettingModelEffortSome,
    DocumentTextBytes,
    TaskContractDocumentApprovalState,
    OwnerPartKeyPathValueItem,
    OwnerPartKeyPathValue,
    TaskContractDocumentVerdictChoices,
    VerdictChoiceFields,
    FieldRuleName,
    LlmChoiceSubagents,
    ModelEffortModel,
    ModelEffortEffortSome,
    SettingPolicyProjections,
    SettingPolicyRoles,
    SettingPolicyRequirements,
    SettingPolicyPermissions,
    SettingPolicyTemplates,
    RulesImplies,
    RulesRequirements,
    OwnerAuthorityGrants,
    OwnerAuthorityNoteResources,
    ModelSetNamesNamesItem,
    ModelSetNamesNames,
    OwnerPatternSegmentsItem,
    OwnerPatternSegments,
    OwnerPatternEndExactValue,
    OwnerPatternEndOpenValue,
    OwnerDelegationKinds,
}

pub use jig::{FIELDS, FORMS};
pub use render::page;
#[must_use]
pub fn most(bound: Bound, limits: &crate::Limits) -> u64 {
    u64::from(match bound {
        Bound::Most(n) => n,
        Bound::Limit(limit) => match limit {
            Limit::CharterInstructions => limits.settings.charter_instructions,
            Limit::CharterAlternatives => limits.settings.charter_alternatives,
            Limit::SettingModelProgramSome => limits.settings.setting_model_program_some,
            Limit::SettingModelName => limits.settings.setting_model_name,
            Limit::SettingModelEffortsItem => limits.settings.setting_model_efforts_item,
            Limit::SettingModelEfforts => limits.settings.setting_model_efforts,
            Limit::SettingModelEffortSome => limits.settings.setting_model_effort_some,
            Limit::DocumentTextBytes => limits.settings.document_text_bytes,
            Limit::TaskContractDocumentApprovalState => limits.settings.task_contract_document_approval_state,
            Limit::OwnerPartKeyPathValueItem => limits.settings.owner_part_key_path_value_item,
            Limit::OwnerPartKeyPathValue => limits.settings.owner_part_key_path_value,
            Limit::TaskContractDocumentVerdictChoices => limits.settings.task_contract_document_verdict_choices,
            Limit::VerdictChoiceFields => limits.settings.verdict_choice_fields,
            Limit::FieldRuleName => limits.settings.field_rule_name,
            Limit::LlmChoiceSubagents => limits.settings.llm_choice_subagents,
            Limit::ModelEffortModel => limits.settings.model_effort_model,
            Limit::ModelEffortEffortSome => limits.settings.model_effort_effort_some,
            Limit::SettingPolicyProjections => limits.settings.setting_policy_projections,
            Limit::SettingPolicyRoles => limits.settings.setting_policy_roles,
            Limit::SettingPolicyRequirements => limits.settings.setting_policy_requirements,
            Limit::SettingPolicyPermissions => limits.settings.setting_policy_permissions,
            Limit::SettingPolicyTemplates => limits.settings.setting_policy_templates,
            Limit::RulesImplies => limits.settings.rules_implies,
            Limit::RulesRequirements => limits.settings.rules_requirements,
            Limit::OwnerAuthorityGrants => limits.settings.owner_authority_grants,
            Limit::OwnerAuthorityNoteResources => limits.settings.owner_authority_note_resources,
            Limit::ModelSetNamesNamesItem => limits.settings.model_set_names_names_item,
            Limit::ModelSetNamesNames => limits.settings.model_set_names_names,
            Limit::OwnerPatternSegmentsItem => limits.settings.owner_pattern_segments_item,
            Limit::OwnerPatternSegments => limits.settings.owner_pattern_segments,
            Limit::OwnerPatternEndExactValue => limits.settings.owner_pattern_end_exact_value,
            Limit::OwnerPatternEndOpenValue => limits.settings.owner_pattern_end_open_value,
            Limit::OwnerDelegationKinds => limits.settings.owner_delegation_kinds,
        },
    })
}
/// A setting's kind word (`charter`, `template`, `policy`, `rules`) to its value's shape.
#[must_use]
pub fn shape_of_kind(kind: &[u8]) -> Option<Shape> {
    match kind {
        b"charter" => Some(Shape::Charter),
        b"template" => Some(Shape::Template),
        b"policy" => Some(Shape::Policy),
        b"rules" => Some(Shape::Rules),
        _ => None,
    }
}
#[must_use]
pub fn form(shape: Shape) -> &'static Form {
    for row in &FORMS {
        if row.shape == shape {
            return row;
        }
    }
    unreachable!("each shape has a form")
}
#[must_use]
pub fn members(shape: Shape) -> &'static [Field] {
    let mut start = 0;
    let mut end = FIELDS.len();
    for (index, row) in FIELDS.iter().enumerate() {
        if row.shape == shape {
            start = index;
            break;
        }
    }
    for (index, row) in FIELDS.iter().enumerate().skip(start) {
        if row.shape != shape {
            end = index;
            break;
        }
    }
    FIELDS.get(start..end).expect("grouped fields")
}
/// `charter.model.name`, or from any shape, `model.name`: each word after
/// the first a member reached through `Value::Shape`, a list's member
/// with or without `[]` (`policy.roles[].number`); for the pages' check.
#[must_use]
pub fn resolve(path: &[u8]) -> Option<&'static Field> {
    let mut shape = None;
    let mut row = None;
    let mut start = 0;
    for end in 0..=path.len() {
        if end < path.len() && path.get(end) != Some(&b'.') {
            continue;
        }
        let word = path.get(start..end)?;
        if start == 0 {
            for form in &FORMS {
                if form.name == word {
                    shape = Some(form.shape);
                    break;
                }
            }
            shape?;
        } else {
            let (name, listed) = match word.strip_suffix(b"[]") {
                Some(name) => (name, true),
                None => (word, false),
            };
            let mut found = None;
            for member in members(shape?) {
                if member.name == name && (!listed || member.list.is_some()) {
                    found = Some(member);
                    break;
                }
            }
            let member = found?;
            row = Some(member);
            shape = match member.value {
                Value::Shape(s) => Some(s),
                Value::Integer(_)
                | Value::Nanoseconds
                | Value::Time
                | Value::Boolean
                | Value::Text(_)
                | Value::Hex(_)
                | Value::Words(_)
                | Value::Setting => None,
            };
        }
        start = end.checked_add(1)?;
    }
    row
}
