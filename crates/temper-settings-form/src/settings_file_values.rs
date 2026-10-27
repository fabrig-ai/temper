//! Concrete whole-value decoders; the schema has fixed depth, with no recursive calls.
use super::{Decoder, Error, error, field, optional};
use crate::view as c;
use skein_json::Kind;
use skein_lib::{Duration, List, Wall};
pub(super) fn charter(d: &Decoder, at: u32) -> Result<c::Charter, Error> {
    let fields = d.object(
        at,
        &[
            b"number",
            b"instructions",
            b"waiting",
            b"resume",
            b"turns",
            b"time",
            b"reserve_turns",
            b"reserve_time",
            b"model",
            b"alternatives",
            b"inspect",
            b"modify",
            b"shell",
            b"agents",
            b"call_timeout",
            b"hosts",
            b"spend",
        ],
    )?;
    Ok(c::Charter {
        switches: [
            d.boolean(field(&fields, b"resume")?)?,
            d.boolean(field(&fields, b"inspect")?)?,
            d.boolean(field(&fields, b"modify")?)?,
            d.boolean(field(&fields, b"shell")?)?,
            d.boolean(field(&fields, b"agents")?)?,
        ],
        number: u32::try_from(d.number(field(&fields, b"number")?)?)
            .or(Err(error(b"charter_number", b"integer bound")))?,
        instructions: d.words(
            field(&fields, b"instructions")?,
            d.limits.charter_instructions,
            b"charter_instructions",
        )?,
        waiting: Duration::from_nanos(d.number(field(&fields, b"waiting")?)?),
        turns: u32::try_from(d.number(field(&fields, b"turns")?)?)
            .or(Err(error(b"charter_turns", b"integer bound")))?,
        time: Duration::from_nanos(d.number(field(&fields, b"time")?)?),
        reserve_turns: u32::try_from(d.number(field(&fields, b"reserve_turns")?)?)
            .or(Err(error(b"charter_reserve_turns", b"integer bound")))?,
        reserve_time: Duration::from_nanos(d.number(field(&fields, b"reserve_time")?)?),
        model: setting_model(d, field(&fields, b"model")?)?,
        alternatives: {
            let positions =
                d.array(field(&fields, b"alternatives")?, d.limits.charter_alternatives, b"charter_alternatives")?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values.push(setting_model(d, *at)?).expect("checked list length");
            }
            values
        },
        call_timeout: Duration::from_nanos(d.number(field(&fields, b"call_timeout")?)?),
        hosts: setting_hosts(d, field(&fields, b"hosts")?)?,
        spend: d.number(field(&fields, b"spend")?)?,
    })
}
pub(super) fn template(d: &Decoder, at: u32) -> Result<c::Template, Error> {
    let fields = d.object(
        at,
        &[
            b"number",
            b"name",
            b"executor",
            b"contract",
            b"shallower",
            b"budget",
            b"deadline",
            b"tracked",
            b"flags",
            b"label",
            b"grace",
            b"llm",
        ],
    )?;
    Ok(c::Template {
        number: u32::try_from(d.number(field(&fields, b"number")?)?)
            .or(Err(error(b"template_number", b"integer bound")))?,
        name: d.document_text(field(&fields, b"name")?)?,
        executor: template_executor(d, field(&fields, b"executor")?)?,
        contract: task_contract_document(d, field(&fields, b"contract")?)?,
        shallower: u32::try_from(d.number(field(&fields, b"shallower")?)?)
            .or(Err(error(b"template_shallower", b"integer bound")))?,
        budget: template_budget(d, field(&fields, b"budget")?)?,
        deadline: template_deadline(d, field(&fields, b"deadline")?)?,
        tracked: if d.kind(field(&fields, b"tracked")?)? == Kind::Null {
            None
        } else {
            Some(
                u32::try_from(d.number(field(&fields, b"tracked")?)?)
                    .or(Err(error(b"template_tracked_some", b"integer bound")))?,
            )
        },
        flags: task_flags(d, field(&fields, b"flags")?)?,
        label: u32::try_from(d.number(field(&fields, b"label")?)?)
            .or(Err(error(b"template_label", b"integer bound")))?,
        grace: if d.kind(field(&fields, b"grace")?)? == Kind::Null {
            None
        } else {
            Some(Duration::from_nanos(d.number(field(&fields, b"grace")?)?))
        },
        llm: if d.kind(field(&fields, b"llm")?)? == Kind::Null {
            None
        } else {
            Some(llm_choice(d, field(&fields, b"llm")?)?)
        },
    })
}
pub(super) fn setting_policy(d: &Decoder, at: u32) -> Result<c::SettingPolicy, Error> {
    let fields = d.object(
        at,
        &[
            b"escalation_role",
            b"ceiling",
            b"projections",
            b"period_spend",
            b"roles",
            b"requirements",
            b"permissions",
            b"templates",
        ],
    )?;
    Ok(c::SettingPolicy {
        escalation_role: if d.kind(field(&fields, b"escalation_role")?)? == Kind::Null {
            None
        } else {
            Some(
                u32::try_from(d.number(field(&fields, b"escalation_role")?)?)
                    .or(Err(error(b"setting_policy_escalation_role_some", b"integer bound")))?,
            )
        },
        ceiling: owner_authority(d, field(&fields, b"ceiling")?)?,
        projections: {
            let positions = d.array(
                field(&fields, b"projections")?,
                d.limits.setting_policy_projections,
                b"setting_policy_projections",
            )?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values.push(owner_grant(d, *at)?).expect("checked list length");
            }
            values
        },
        period_spend: d.number(field(&fields, b"period_spend")?)?,
        roles: {
            let positions =
                d.array(field(&fields, b"roles")?, d.limits.setting_policy_roles, b"setting_policy_roles")?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values.push(policy_edit_role(d, *at)?).expect("checked list length");
            }
            values
        },
        requirements: {
            let positions = d.array(
                field(&fields, b"requirements")?,
                d.limits.setting_policy_requirements,
                b"setting_policy_requirements",
            )?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values.push(policy_requirement(d, *at)?).expect("checked list length");
            }
            values
        },
        permissions: {
            let positions = d.array(
                field(&fields, b"permissions")?,
                d.limits.setting_policy_permissions,
                b"setting_policy_permissions",
            )?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values.push(policy_permission(d, *at)?).expect("checked list length");
            }
            values
        },
        templates: {
            let positions =
                d.array(field(&fields, b"templates")?, d.limits.setting_policy_templates, b"setting_policy_templates")?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values.push(template_override(d, *at)?).expect("checked list length");
            }
            values
        },
    })
}
pub(super) fn rules(d: &Decoder, at: u32) -> Result<c::Rules, Error> {
    let fields = d.object(
        at,
        &[b"ceiling", b"period_spend", b"minimum_run_spend", b"maximum_run_spend", b"implies", b"requirements"],
    )?;
    Ok(c::Rules {
        ceiling: owner_authority(d, field(&fields, b"ceiling")?)?,
        period_spend: d.number(field(&fields, b"period_spend")?)?,
        minimum_run_spend: d.number(field(&fields, b"minimum_run_spend")?)?,
        maximum_run_spend: d.number(field(&fields, b"maximum_run_spend")?)?,
        implies: {
            let positions = d.array(field(&fields, b"implies")?, d.limits.rules_implies, b"rules_implies")?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values.push(owner_kind_implication(d, *at)?).expect("checked list length");
            }
            values
        },
        requirements: {
            let positions =
                d.array(field(&fields, b"requirements")?, d.limits.rules_requirements, b"rules_requirements")?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values.push(policy_requirement(d, *at)?).expect("checked list length");
            }
            values
        },
    })
}
fn setting_model(d: &Decoder, at: u32) -> Result<c::SettingModel, Error> {
    let fields = d.object(
        at,
        &[
            b"dialect",
            b"endpoint",
            b"program",
            b"name",
            b"window",
            b"output",
            b"input_price",
            b"cached_price",
            b"cache_write_price",
            b"output_price",
            b"price_unit",
            b"efforts",
            b"effort",
        ],
    )?;
    Ok(c::SettingModel {
        dialect: u32::try_from(d.number(field(&fields, b"dialect")?)?)
            .or(Err(error(b"setting_model_dialect", b"integer bound")))?,
        endpoint: u32::try_from(d.number(field(&fields, b"endpoint")?)?)
            .or(Err(error(b"setting_model_endpoint", b"integer bound")))?,
        program: if d.kind(field(&fields, b"program")?)? == Kind::Null {
            None
        } else {
            Some(d.words(
                field(&fields, b"program")?,
                d.limits.setting_model_program_some,
                b"setting_model_program_some",
            )?)
        },
        name: d.words(field(&fields, b"name")?, d.limits.setting_model_name, b"setting_model_name")?,
        window: u32::try_from(d.number(field(&fields, b"window")?)?)
            .or(Err(error(b"setting_model_window", b"integer bound")))?,
        output: u32::try_from(d.number(field(&fields, b"output")?)?)
            .or(Err(error(b"setting_model_output", b"integer bound")))?,
        input_price: d.number(field(&fields, b"input_price")?)?,
        cached_price: d.number(field(&fields, b"cached_price")?)?,
        cache_write_price: if d.kind(field(&fields, b"cache_write_price")?)? == Kind::Null {
            None
        } else {
            Some(d.number(field(&fields, b"cache_write_price")?)?)
        },
        output_price: d.number(field(&fields, b"output_price")?)?,
        price_unit: u32::try_from(d.number(field(&fields, b"price_unit")?)?)
            .or(Err(error(b"setting_model_price_unit", b"integer bound")))?,
        efforts: {
            let positions =
                d.array(field(&fields, b"efforts")?, d.limits.setting_model_efforts, b"setting_model_efforts")?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values
                    .push(d.words(*at, d.limits.setting_model_efforts_item, b"setting_model_efforts_item")?)
                    .expect("checked list length");
            }
            values
        },
        effort: if d.kind(field(&fields, b"effort")?)? == Kind::Null {
            None
        } else {
            Some(d.words(
                field(&fields, b"effort")?,
                d.limits.setting_model_effort_some,
                b"setting_model_effort_some",
            )?)
        },
    })
}
fn setting_hosts(d: &Decoder, at: u32) -> Result<c::SettingHosts, Error> {
    if d.kind(at)? == Kind::String {
        return match d.string(at)? {
            b"workers" => Ok(c::SettingHosts::Workers),
            b"engine" => Ok(c::SettingHosts::Engine),
            b"both" => Ok(c::SettingHosts::Both),
            _ => Err(error(b"setting_hosts", b"unknown word")),
        };
    }
    Err(error(b"setting_hosts", b"expected word"))
}
fn template_executor(d: &Decoder, at: u32) -> Result<c::TemplateExecutor, Error> {
    if d.kind(at)? == Kind::String {
        return match d.string(at)? {
            b"chosen" => Ok(c::TemplateExecutor::Chosen),
            _ => Err(error(b"template_executor", b"unknown word")),
        };
    }
    let fields = d.object(at, &[b"charter"])?;
    if fields.len() != 1 {
        return Err(error(b"template_executor", b"one variant required"));
    }
    if let Some(at) = optional(&fields, b"charter") {
        return Ok(c::TemplateExecutor::Charter(
            u32::try_from(d.number(at)?).or(Err(error(b"template_executor_charter", b"integer bound")))?,
        ));
    }
    Err(error(b"template_executor", b"unknown variant"))
}
fn task_contract_document(d: &Decoder, at: u32) -> Result<c::TaskContractDocument, Error> {
    let fields = d.object(at, &[b"approval", b"report", b"verdict", b"change"])?;
    if fields.len() != 1 {
        return Err(error(b"task_contract_document", b"one variant required"));
    }
    if let Some(at) = optional(&fields, b"approval") {
        let fields = d.object(at, &[b"state", b"part"])?;
        return Ok(c::TaskContractDocument::Approval {
            state: d.hex(
                field(&fields, b"state")?,
                d.limits.task_contract_document_approval_state,
                b"task_contract_document_approval_state",
            )?,
            part: owner_part(d, field(&fields, b"part")?)?,
        });
    }
    if let Some(at) = optional(&fields, b"report") {
        let fields = d.object(at, &[b"words"])?;
        return Ok(c::TaskContractDocument::Report {
            words: u32::try_from(d.number(field(&fields, b"words")?)?)
                .or(Err(error(b"task_contract_document_report_words", b"integer bound")))?,
        });
    }
    if let Some(at) = optional(&fields, b"verdict") {
        let fields = d.object(at, &[b"choices", b"cut"])?;
        return Ok(c::TaskContractDocument::Verdict {
            choices: {
                let positions = d.array(
                    field(&fields, b"choices")?,
                    d.limits.task_contract_document_verdict_choices,
                    b"task_contract_document_verdict_choices",
                )?;
                let mut values = List::with_capacity(positions.len());
                for at in &positions {
                    values.push(verdict_choice(d, *at)?).expect("checked list length");
                }
                super::verdict_bound(&values, &d.limits)?;
                values
            },
            cut: {
                if d.boolean(field(&fields, b"cut")?)? {
                    return Err(error(b"cut", b"whole value required"));
                }
                false
            },
        });
    }
    if let Some(at) = optional(&fields, b"change") {
        let fields = d.object(at, &[b"connector", b"kind", b"words"])?;
        return Ok(c::TaskContractDocument::Change {
            connector: u16::try_from(d.number(field(&fields, b"connector")?)?)
                .or(Err(error(b"task_contract_document_change_connector", b"integer bound")))?,
            kind: u16::try_from(d.number(field(&fields, b"kind")?)?)
                .or(Err(error(b"task_contract_document_change_kind", b"integer bound")))?,
            words: u32::try_from(d.number(field(&fields, b"words")?)?)
                .or(Err(error(b"task_contract_document_change_words", b"integer bound")))?,
        });
    }
    Err(error(b"task_contract_document", b"unknown variant"))
}
fn template_budget(d: &Decoder, at: u32) -> Result<c::TemplateBudget, Error> {
    if d.kind(at)? == Kind::String {
        return match d.string(at)? {
            b"given" => Ok(c::TemplateBudget::Given),
            _ => Err(error(b"template_budget", b"unknown word")),
        };
    }
    let fields = d.object(at, &[b"amount", b"as_allowed"])?;
    if fields.len() != 1 {
        return Err(error(b"template_budget", b"one variant required"));
    }
    if let Some(at) = optional(&fields, b"amount") {
        return Ok(c::TemplateBudget::Amount(d.number(at)?));
    }
    if let Some(at) = optional(&fields, b"as_allowed") {
        let fields = d.object(at, &[b"cap"])?;
        return Ok(c::TemplateBudget::AsAllowed {
            cap: if d.kind(field(&fields, b"cap")?)? == Kind::Null {
                None
            } else {
                Some(d.number(field(&fields, b"cap")?)?)
            },
        });
    }
    Err(error(b"template_budget", b"unknown variant"))
}
fn template_deadline(d: &Decoder, at: u32) -> Result<c::TemplateDeadline, Error> {
    if d.kind(at)? == Kind::String {
        return match d.string(at)? {
            b"none" => Ok(c::TemplateDeadline::None),
            b"given" => Ok(c::TemplateDeadline::Given),
            _ => Err(error(b"template_deadline", b"unknown word")),
        };
    }
    let fields = d.object(at, &[b"span"])?;
    if fields.len() != 1 {
        return Err(error(b"template_deadline", b"one variant required"));
    }
    if let Some(at) = optional(&fields, b"span") {
        return Ok(c::TemplateDeadline::Span(Duration::from_nanos(d.number(at)?)));
    }
    Err(error(b"template_deadline", b"unknown variant"))
}
fn task_flags(d: &Decoder, at: u32) -> Result<c::TaskFlags, Error> {
    let fields = d.object(at, &[b"conversation", b"listed", b"retention", b"ended_by"])?;
    Ok(c::TaskFlags {
        conversation: d.boolean(field(&fields, b"conversation")?)?,
        listed: d.boolean(field(&fields, b"listed")?)?,
        retention: retention(d, field(&fields, b"retention")?)?,
        ended_by: ended_by(d, field(&fields, b"ended_by")?)?,
    })
}
fn llm_choice(d: &Decoder, at: u32) -> Result<c::LlmChoice, Error> {
    let fields = d.object(at, &[b"account", b"main", b"subagents"])?;
    Ok(c::LlmChoice {
        account: if d.kind(field(&fields, b"account")?)? == Kind::Null {
            None
        } else {
            Some(
                u32::try_from(d.number(field(&fields, b"account")?)?)
                    .or(Err(error(b"llm_choice_account_some", b"integer bound")))?,
            )
        },
        main: model_effort(d, field(&fields, b"main")?)?,
        subagents: {
            let positions =
                d.array(field(&fields, b"subagents")?, d.limits.llm_choice_subagents, b"llm_choice_subagents")?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values.push(model_effort(d, *at)?).expect("checked list length");
            }
            values
        },
    })
}
fn owner_authority(d: &Decoder, at: u32) -> Result<c::OwnerAuthority, Error> {
    let fields = d.object(
        at,
        &[b"models", b"tools", b"grants", b"delegation", b"spend", b"deadline", b"notes", b"note_resources"],
    )?;
    Ok(c::OwnerAuthority {
        models: if d.kind(field(&fields, b"models")?)? == Kind::Null {
            None
        } else {
            Some(authority_models(d, field(&fields, b"models")?)?)
        },
        tools: d.number(field(&fields, b"tools")?)?,
        grants: {
            let positions =
                d.array(field(&fields, b"grants")?, d.limits.owner_authority_grants, b"owner_authority_grants")?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values.push(owner_grant(d, *at)?).expect("checked list length");
            }
            values
        },
        delegation: owner_delegation(d, field(&fields, b"delegation")?)?,
        spend: d.number(field(&fields, b"spend")?)?,
        deadline: if d.kind(field(&fields, b"deadline")?)? == Kind::Null {
            None
        } else {
            Some(Wall::from_nanos(d.number(field(&fields, b"deadline")?)?))
        },
        notes: u8::try_from(d.number(field(&fields, b"notes")?)?)
            .or(Err(error(b"owner_authority_notes", b"integer bound")))?,
        note_resources: {
            let positions = d.array(
                field(&fields, b"note_resources")?,
                d.limits.owner_authority_note_resources,
                b"owner_authority_note_resources",
            )?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values.push(owner_resource_scope(d, *at)?).expect("checked list length");
            }
            values
        },
    })
}
fn owner_grant(d: &Decoder, at: u32) -> Result<c::OwnerGrant, Error> {
    let fields = d.object(at, &[b"connector", b"kind", b"pattern"])?;
    Ok(c::OwnerGrant {
        connector: u16::try_from(d.number(field(&fields, b"connector")?)?)
            .or(Err(error(b"owner_grant_connector", b"integer bound")))?,
        kind: u16::try_from(d.number(field(&fields, b"kind")?)?)
            .or(Err(error(b"owner_grant_kind", b"integer bound")))?,
        pattern: owner_pattern(d, field(&fields, b"pattern")?)?,
    })
}
fn policy_edit_role(d: &Decoder, at: u32) -> Result<c::PolicyEditRole, Error> {
    let fields = d.object(at, &[b"number", b"authority", b"period_spend", b"requests", b"decides"])?;
    Ok(c::PolicyEditRole {
        number: u32::try_from(d.number(field(&fields, b"number")?)?)
            .or(Err(error(b"policy_edit_role_number", b"integer bound")))?,
        authority: owner_authority(d, field(&fields, b"authority")?)?,
        period_spend: d.number(field(&fields, b"period_spend")?)?,
        requests: u16::try_from(d.number(field(&fields, b"requests")?)?)
            .or(Err(error(b"policy_edit_role_requests", b"integer bound")))?,
        decides: u8::try_from(d.number(field(&fields, b"decides")?)?)
            .or(Err(error(b"policy_edit_role_decides", b"integer bound")))?,
    })
}
fn policy_requirement(d: &Decoder, at: u32) -> Result<c::PolicyRequirement, Error> {
    let fields = d.object(at, &[b"connector", b"kind", b"pattern", b"judge", b"guard", b"must_be_guarded"])?;
    Ok(c::PolicyRequirement {
        connector: u16::try_from(d.number(field(&fields, b"connector")?)?)
            .or(Err(error(b"policy_requirement_connector", b"integer bound")))?,
        kind: u16::try_from(d.number(field(&fields, b"kind")?)?)
            .or(Err(error(b"policy_requirement_kind", b"integer bound")))?,
        pattern: owner_pattern(d, field(&fields, b"pattern")?)?,
        judge: policy_judge(d, field(&fields, b"judge")?)?,
        guard: policy_guard(d, field(&fields, b"guard")?)?,
        must_be_guarded: d.boolean(field(&fields, b"must_be_guarded")?)?,
    })
}
fn policy_permission(d: &Decoder, at: u32) -> Result<c::PolicyPermission, Error> {
    let fields = d.object(at, &[b"connector", b"permission", b"role"])?;
    Ok(c::PolicyPermission {
        connector: u16::try_from(d.number(field(&fields, b"connector")?)?)
            .or(Err(error(b"policy_permission_connector", b"integer bound")))?,
        permission: u16::try_from(d.number(field(&fields, b"permission")?)?)
            .or(Err(error(b"policy_permission_permission", b"integer bound")))?,
        role: u32::try_from(d.number(field(&fields, b"role")?)?)
            .or(Err(error(b"policy_permission_role", b"integer bound")))?,
    })
}
fn template_override(d: &Decoder, at: u32) -> Result<c::TemplateOverride, Error> {
    let fields = d.object(at, &[b"template", b"executor", b"budget", b"deadline", b"priority"])?;
    Ok(c::TemplateOverride {
        template: u32::try_from(d.number(field(&fields, b"template")?)?)
            .or(Err(error(b"template_override_template", b"integer bound")))?,
        executor: if d.kind(field(&fields, b"executor")?)? == Kind::Null {
            None
        } else {
            Some(template_executor(d, field(&fields, b"executor")?)?)
        },
        budget: if d.kind(field(&fields, b"budget")?)? == Kind::Null {
            None
        } else {
            Some(template_budget(d, field(&fields, b"budget")?)?)
        },
        deadline: if d.kind(field(&fields, b"deadline")?)? == Kind::Null {
            None
        } else {
            Some(template_deadline(d, field(&fields, b"deadline")?)?)
        },
        priority: if d.kind(field(&fields, b"priority")?)? == Kind::Null {
            None
        } else {
            Some(
                u32::try_from(d.number(field(&fields, b"priority")?)?)
                    .or(Err(error(b"template_override_priority_some", b"integer bound")))?,
            )
        },
    })
}
fn owner_kind_implication(d: &Decoder, at: u32) -> Result<c::OwnerKindImplication, Error> {
    let fields = d.object(at, &[b"connector", b"kind", b"implies"])?;
    Ok(c::OwnerKindImplication {
        connector: u16::try_from(d.number(field(&fields, b"connector")?)?)
            .or(Err(error(b"owner_kind_implication_connector", b"integer bound")))?,
        kind: u16::try_from(d.number(field(&fields, b"kind")?)?)
            .or(Err(error(b"owner_kind_implication_kind", b"integer bound")))?,
        implies: u16::try_from(d.number(field(&fields, b"implies")?)?)
            .or(Err(error(b"owner_kind_implication_implies", b"integer bound")))?,
    })
}
fn owner_part(d: &Decoder, at: u32) -> Result<c::OwnerPart, Error> {
    let fields = d.object(at, &[b"connector", b"kind", b"key"])?;
    Ok(c::OwnerPart {
        connector: u16::try_from(d.number(field(&fields, b"connector")?)?)
            .or(Err(error(b"owner_part_connector", b"integer bound")))?,
        kind: owner_part_kind(d, field(&fields, b"kind")?)?,
        key: owner_part_key(d, field(&fields, b"key")?)?,
    })
}
fn verdict_choice(d: &Decoder, at: u32) -> Result<c::VerdictChoice, Error> {
    let fields = d.object(at, &[b"code", b"words", b"followups", b"fields"])?;
    Ok(c::VerdictChoice {
        code: u32::try_from(d.number(field(&fields, b"code")?)?)
            .or(Err(error(b"verdict_choice_code", b"integer bound")))?,
        words: u32::try_from(d.number(field(&fields, b"words")?)?)
            .or(Err(error(b"verdict_choice_words", b"integer bound")))?,
        followups: u32::try_from(d.number(field(&fields, b"followups")?)?)
            .or(Err(error(b"verdict_choice_followups", b"integer bound")))?,
        fields: {
            let positions =
                d.array(field(&fields, b"fields")?, d.limits.verdict_choice_fields, b"verdict_choice_fields")?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values.push(field_rule(d, *at)?).expect("checked list length");
            }
            values
        },
    })
}
fn retention(d: &Decoder, at: u32) -> Result<c::Retention, Error> {
    if d.kind(at)? == Kind::String {
        return match d.string(at)? {
            b"kept" => Ok(c::Retention::Kept),
            b"forgotten_once_read" => Ok(c::Retention::ForgottenOnceRead),
            _ => Err(error(b"retention", b"unknown word")),
        };
    }
    Err(error(b"retention", b"expected word"))
}
fn ended_by(d: &Decoder, at: u32) -> Result<c::EndedBy, Error> {
    if d.kind(at)? == Kind::String {
        return match d.string(at)? {
            b"result" => Ok(c::EndedBy::Result),
            b"requester" => Ok(c::EndedBy::Requester),
            _ => Err(error(b"ended_by", b"unknown word")),
        };
    }
    Err(error(b"ended_by", b"expected word"))
}
fn model_effort(d: &Decoder, at: u32) -> Result<c::ModelEffort, Error> {
    let fields = d.object(at, &[b"model", b"effort"])?;
    Ok(c::ModelEffort {
        model: d.words(field(&fields, b"model")?, d.limits.model_effort_model, b"model_effort_model")?,
        effort: if d.kind(field(&fields, b"effort")?)? == Kind::Null {
            None
        } else {
            Some(d.words(field(&fields, b"effort")?, d.limits.model_effort_effort_some, b"model_effort_effort_some")?)
        },
    })
}
fn authority_models(d: &Decoder, at: u32) -> Result<c::AuthorityModels, Error> {
    if d.kind(at)? == Kind::String {
        return match d.string(at)? {
            b"all" => Ok(c::AuthorityModels::All),
            _ => Err(error(b"authority_models", b"unknown word")),
        };
    }
    let fields = d.object(at, &[b"named"])?;
    if fields.len() != 1 {
        return Err(error(b"authority_models", b"one variant required"));
    }
    if let Some(at) = optional(&fields, b"named") {
        return Ok(c::AuthorityModels::Named({
            let positions = d.array(at, d.limits.model_set_names_names, b"model_set_names_names")?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values
                    .push(d.hex(*at, d.limits.model_set_names_names_item, b"model_set_names_names_item")?)
                    .expect("checked list length");
            }
            values
        }));
    }
    Err(error(b"authority_models", b"unknown variant"))
}
fn owner_delegation(d: &Decoder, at: u32) -> Result<c::OwnerDelegation, Error> {
    let fields = d.object(at, &[b"kinds", b"tasks", b"depth"])?;
    Ok(c::OwnerDelegation {
        kinds: {
            let positions =
                d.array(field(&fields, b"kinds")?, d.limits.owner_delegation_kinds, b"owner_delegation_kinds")?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values.push(owner_authority_executor(d, *at)?).expect("checked list length");
            }
            values
        },
        tasks: u32::try_from(d.number(field(&fields, b"tasks")?)?)
            .or(Err(error(b"owner_delegation_tasks", b"integer bound")))?,
        depth: u32::try_from(d.number(field(&fields, b"depth")?)?)
            .or(Err(error(b"owner_delegation_depth", b"integer bound")))?,
    })
}
fn owner_resource_scope(d: &Decoder, at: u32) -> Result<c::OwnerResourceScope, Error> {
    let fields = d.object(at, &[b"connector", b"pattern"])?;
    Ok(c::OwnerResourceScope {
        connector: u16::try_from(d.number(field(&fields, b"connector")?)?)
            .or(Err(error(b"owner_resource_scope_connector", b"integer bound")))?,
        pattern: owner_pattern(d, field(&fields, b"pattern")?)?,
    })
}
fn owner_pattern(d: &Decoder, at: u32) -> Result<c::OwnerPattern, Error> {
    let fields = d.object(at, &[b"segments", b"last"])?;
    Ok(c::OwnerPattern {
        segments: {
            let positions =
                d.array(field(&fields, b"segments")?, d.limits.owner_pattern_segments, b"owner_pattern_segments")?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values
                    .push(d.hex(*at, d.limits.owner_pattern_segments_item, b"owner_pattern_segments_item")?)
                    .expect("checked list length");
            }
            values
        },
        last: owner_pattern_end(d, field(&fields, b"last")?)?,
    })
}
fn policy_judge(d: &Decoder, at: u32) -> Result<c::PolicyJudge, Error> {
    let fields = d.object(at, &[b"connector", b"requirement", b"parameters"])?;
    Ok(c::PolicyJudge {
        connector: u16::try_from(d.number(field(&fields, b"connector")?)?)
            .or(Err(error(b"policy_judge_connector", b"integer bound")))?,
        requirement: u16::try_from(d.number(field(&fields, b"requirement")?)?)
            .or(Err(error(b"policy_judge_requirement", b"integer bound")))?,
        parameters: u32::try_from(d.number(field(&fields, b"parameters")?)?)
            .or(Err(error(b"policy_judge_parameters", b"integer bound")))?,
    })
}
fn policy_guard(d: &Decoder, at: u32) -> Result<c::PolicyGuard, Error> {
    if d.kind(at)? == Kind::String {
        return match d.string(at)? {
            b"guarded" => Ok(c::PolicyGuard::Guarded),
            _ => Err(error(b"policy_guard", b"unknown word")),
        };
    }
    let fields = d.object(at, &[b"observed"])?;
    if fields.len() != 1 {
        return Err(error(b"policy_guard", b"one variant required"));
    }
    if let Some(at) = optional(&fields, b"observed") {
        let fields = d.object(at, &[b"freshness"])?;
        return Ok(c::PolicyGuard::Observed {
            freshness: Duration::from_nanos(d.number(field(&fields, b"freshness")?)?),
        });
    }
    Err(error(b"policy_guard", b"unknown variant"))
}
fn owner_part_kind(d: &Decoder, at: u32) -> Result<c::OwnerPartKind, Error> {
    if d.kind(at)? == Kind::String {
        return match d.string(at)? {
            b"resource" => Ok(c::OwnerPartKind::Resource),
            b"pool" => Ok(c::OwnerPartKind::Pool),
            b"result" => Ok(c::OwnerPartKind::Result),
            b"procedure" => Ok(c::OwnerPartKind::Procedure),
            b"effect" => Ok(c::OwnerPartKind::Effect),
            b"outbox" => Ok(c::OwnerPartKind::Outbox),
            b"hold" => Ok(c::OwnerPartKind::Hold),
            _ => Err(error(b"owner_part_kind", b"unknown word")),
        };
    }
    Err(error(b"owner_part_kind", b"expected word"))
}
fn owner_part_key(d: &Decoder, at: u32) -> Result<c::OwnerPartKey, Error> {
    let fields = d.object(at, &[b"path", b"number"])?;
    if fields.len() != 1 {
        return Err(error(b"owner_part_key", b"one variant required"));
    }
    if let Some(at) = optional(&fields, b"path") {
        return Ok(c::OwnerPartKey::Path({
            let positions = d.array(at, d.limits.owner_part_key_path_value, b"owner_part_key_path_value")?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values
                    .push(d.hex(*at, d.limits.owner_part_key_path_value_item, b"owner_part_key_path_value_item")?)
                    .expect("checked list length");
            }
            values
        }));
    }
    if let Some(at) = optional(&fields, b"number") {
        return Ok(c::OwnerPartKey::Number(d.number(at)?));
    }
    Err(error(b"owner_part_key", b"unknown variant"))
}
fn field_rule(d: &Decoder, at: u32) -> Result<c::FieldRule, Error> {
    let fields = d.object(at, &[b"name", b"max"])?;
    Ok(c::FieldRule {
        name: d.hex(field(&fields, b"name")?, d.limits.field_rule_name, b"field_rule_name")?,
        max: u32::try_from(d.number(field(&fields, b"max")?)?).or(Err(error(b"field_rule_max", b"integer bound")))?,
    })
}
fn owner_authority_executor(d: &Decoder, at: u32) -> Result<c::OwnerAuthorityExecutor, Error> {
    let fields = d.object(at, &[b"charter", b"procedure", b"role"])?;
    if fields.len() != 1 {
        return Err(error(b"owner_authority_executor", b"one variant required"));
    }
    if let Some(at) = optional(&fields, b"charter") {
        return Ok(c::OwnerAuthorityExecutor::Charter(
            u32::try_from(d.number(at)?).or(Err(error(b"owner_authority_executor_charter", b"integer bound")))?,
        ));
    }
    if let Some(at) = optional(&fields, b"procedure") {
        return Ok(c::OwnerAuthorityExecutor::Procedure(
            u32::try_from(d.number(at)?).or(Err(error(b"owner_authority_executor_procedure", b"integer bound")))?,
        ));
    }
    if let Some(at) = optional(&fields, b"role") {
        return Ok(c::OwnerAuthorityExecutor::Role(
            u32::try_from(d.number(at)?).or(Err(error(b"owner_authority_executor_role", b"integer bound")))?,
        ));
    }
    Err(error(b"owner_authority_executor", b"unknown variant"))
}
fn owner_pattern_end(d: &Decoder, at: u32) -> Result<c::OwnerPatternEnd, Error> {
    let fields = d.object(at, &[b"exact", b"open"])?;
    if fields.len() != 1 {
        return Err(error(b"owner_pattern_end", b"one variant required"));
    }
    if let Some(at) = optional(&fields, b"exact") {
        return Ok(c::OwnerPatternEnd::Exact(d.hex(
            at,
            d.limits.owner_pattern_end_exact_value,
            b"owner_pattern_end_exact_value",
        )?));
    }
    if let Some(at) = optional(&fields, b"open") {
        return Ok(c::OwnerPatternEnd::Open(d.hex(
            at,
            d.limits.owner_pattern_end_open_value,
            b"owner_pattern_end_open_value",
        )?));
    }
    Err(error(b"owner_pattern_end", b"unknown variant"))
}
