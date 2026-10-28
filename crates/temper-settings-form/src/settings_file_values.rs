//! Concrete whole-value decoders; the schema has fixed depth, with no recursive calls.
use super::{Decoder, Error, error, field, optional};
use crate::fields::Shape;
use crate::view as c;
use skein_json::Kind;
use skein_lib::{Duration, List, Wall};
pub(super) fn charter(d: &Decoder, at: u32) -> Result<c::Charter, Error> {
    let fields = d.object(at, Shape::Charter)?;
    Ok(c::Charter {
        switches: [
            d.boolean(field(&fields, b"resume")?)?,
            d.boolean(field(&fields, b"inspect")?)?,
            d.boolean(field(&fields, b"modify")?)?,
            d.boolean(field(&fields, b"shell")?)?,
            d.boolean(field(&fields, b"agents")?)?,
        ],
        number: u32::try_from(d.number(field(&fields, b"number")?, fields.shape, b"number")?)
            .or(Err(error(b"charter.number", b"integer bound")))?,
        instructions: d.words(
            field(&fields, b"instructions")?,
            d.limits.charter_instructions,
            b"charter.instructions",
        )?,
        waiting: Duration::from_nanos(d.number(field(&fields, b"waiting")?, fields.shape, b"waiting")?),
        turns: u32::try_from(d.number(field(&fields, b"turns")?, fields.shape, b"turns")?)
            .or(Err(error(b"charter.turns", b"integer bound")))?,
        time: Duration::from_nanos(d.number(field(&fields, b"time")?, fields.shape, b"time")?),
        reserve_turns: u32::try_from(d.number(field(&fields, b"reserve_turns")?, fields.shape, b"reserve_turns")?)
            .or(Err(error(b"charter.reserve_turns", b"integer bound")))?,
        reserve_time: Duration::from_nanos(d.number(
            field(&fields, b"reserve_time")?,
            fields.shape,
            b"reserve_time",
        )?),
        model: setting_model(d, field(&fields, b"model")?)?,
        alternatives: {
            let positions =
                d.array(field(&fields, b"alternatives")?, d.limits.charter_alternatives, b"charter.alternatives")?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values.push(setting_model(d, *at)?).expect("checked list length");
            }
            values
        },
        call_timeout: Duration::from_nanos(d.number(
            field(&fields, b"call_timeout")?,
            fields.shape,
            b"call_timeout",
        )?),
        hosts: setting_hosts(d, field(&fields, b"hosts")?)?,
        spend: d.number(field(&fields, b"spend")?, fields.shape, b"spend")?,
    })
}
pub(super) fn template(d: &Decoder, at: u32) -> Result<c::Template, Error> {
    let fields = d.object(at, Shape::Template)?;
    Ok(c::Template {
        number: u32::try_from(d.number(field(&fields, b"number")?, fields.shape, b"number")?)
            .or(Err(error(b"template.number", b"integer bound")))?,
        name: d.document_text(field(&fields, b"name")?)?,
        executor: template_executor(d, field(&fields, b"executor")?)?,
        contract: task_contract_document(d, field(&fields, b"contract")?)?,
        shallower: u32::try_from(d.number(field(&fields, b"shallower")?, fields.shape, b"shallower")?)
            .or(Err(error(b"template.shallower", b"integer bound")))?,
        budget: template_budget(d, field(&fields, b"budget")?)?,
        deadline: template_deadline(d, field(&fields, b"deadline")?)?,
        tracked: if d.kind(field(&fields, b"tracked")?)? == Kind::Null {
            None
        } else {
            Some(
                u32::try_from(d.number(field(&fields, b"tracked")?, fields.shape, b"tracked")?)
                    .or(Err(error(b"template.tracked", b"integer bound")))?,
            )
        },
        flags: task_flags(d, field(&fields, b"flags")?)?,
        label: u32::try_from(d.number(field(&fields, b"label")?, fields.shape, b"label")?)
            .or(Err(error(b"template.label", b"integer bound")))?,
        grace: if d.kind(field(&fields, b"grace")?)? == Kind::Null {
            None
        } else {
            Some(Duration::from_nanos(d.number(field(&fields, b"grace")?, fields.shape, b"grace")?))
        },
        llm: if d.kind(field(&fields, b"llm")?)? == Kind::Null {
            None
        } else {
            Some(llm_choice(d, field(&fields, b"llm")?)?)
        },
    })
}
pub(super) fn setting_policy(d: &Decoder, at: u32) -> Result<c::SettingPolicy, Error> {
    let fields = d.object(at, Shape::Policy)?;
    Ok(c::SettingPolicy {
        escalation_role: if d.kind(field(&fields, b"escalation_role")?)? == Kind::Null {
            None
        } else {
            Some(
                u32::try_from(d.number(field(&fields, b"escalation_role")?, fields.shape, b"escalation_role")?)
                    .or(Err(error(b"policy.escalation_role", b"integer bound")))?,
            )
        },
        ceiling: owner_authority(d, field(&fields, b"ceiling")?)?,
        projections: {
            let positions =
                d.array(field(&fields, b"projections")?, d.limits.setting_policy_projections, b"policy.projections")?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values.push(owner_grant(d, *at)?).expect("checked list length");
            }
            values
        },
        period_spend: d.number(field(&fields, b"period_spend")?, fields.shape, b"period_spend")?,
        roles: {
            let positions = d.array(field(&fields, b"roles")?, d.limits.setting_policy_roles, b"policy.roles")?;
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
                b"policy.requirements",
            )?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values.push(policy_requirement(d, *at)?).expect("checked list length");
            }
            values
        },
        permissions: {
            let positions =
                d.array(field(&fields, b"permissions")?, d.limits.setting_policy_permissions, b"policy.permissions")?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values.push(policy_permission(d, *at)?).expect("checked list length");
            }
            values
        },
        templates: {
            let positions =
                d.array(field(&fields, b"templates")?, d.limits.setting_policy_templates, b"policy.templates")?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values.push(template_override(d, *at)?).expect("checked list length");
            }
            values
        },
    })
}
pub(super) fn rules(d: &Decoder, at: u32) -> Result<c::Rules, Error> {
    let fields = d.object(at, Shape::Rules)?;
    Ok(c::Rules {
        ceiling: owner_authority(d, field(&fields, b"ceiling")?)?,
        period_spend: d.number(field(&fields, b"period_spend")?, fields.shape, b"period_spend")?,
        minimum_run_spend: d.number(field(&fields, b"minimum_run_spend")?, fields.shape, b"minimum_run_spend")?,
        maximum_run_spend: d.number(field(&fields, b"maximum_run_spend")?, fields.shape, b"maximum_run_spend")?,
        implies: {
            let positions = d.array(field(&fields, b"implies")?, d.limits.rules_implies, b"rules.implies")?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values.push(owner_kind_implication(d, *at)?).expect("checked list length");
            }
            values
        },
        requirements: {
            let positions =
                d.array(field(&fields, b"requirements")?, d.limits.rules_requirements, b"rules.requirements")?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values.push(policy_requirement(d, *at)?).expect("checked list length");
            }
            values
        },
    })
}
fn setting_model(d: &Decoder, at: u32) -> Result<c::SettingModel, Error> {
    let fields = d.object(at, Shape::Model)?;
    Ok(c::SettingModel {
        dialect: u32::try_from(d.number(field(&fields, b"dialect")?, fields.shape, b"dialect")?)
            .or(Err(error(b"model.dialect", b"integer bound")))?,
        endpoint: u32::try_from(d.number(field(&fields, b"endpoint")?, fields.shape, b"endpoint")?)
            .or(Err(error(b"model.endpoint", b"integer bound")))?,
        program: if d.kind(field(&fields, b"program")?)? == Kind::Null {
            None
        } else {
            Some(d.words(field(&fields, b"program")?, d.limits.setting_model_program_some, b"model.program")?)
        },
        name: d.words(field(&fields, b"name")?, d.limits.setting_model_name, b"model.name")?,
        window: u32::try_from(d.number(field(&fields, b"window")?, fields.shape, b"window")?)
            .or(Err(error(b"model.window", b"integer bound")))?,
        output: u32::try_from(d.number(field(&fields, b"output")?, fields.shape, b"output")?)
            .or(Err(error(b"model.output", b"integer bound")))?,
        input_price: d.number(field(&fields, b"input_price")?, fields.shape, b"input_price")?,
        cached_price: d.number(field(&fields, b"cached_price")?, fields.shape, b"cached_price")?,
        cache_write_price: if d.kind(field(&fields, b"cache_write_price")?)? == Kind::Null {
            None
        } else {
            Some(d.number(field(&fields, b"cache_write_price")?, fields.shape, b"cache_write_price")?)
        },
        output_price: d.number(field(&fields, b"output_price")?, fields.shape, b"output_price")?,
        price_unit: u32::try_from(d.number(field(&fields, b"price_unit")?, fields.shape, b"price_unit")?)
            .or(Err(error(b"model.price_unit", b"integer bound")))?,
        efforts: {
            let positions = d.array(field(&fields, b"efforts")?, d.limits.setting_model_efforts, b"model.efforts")?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values
                    .push(d.words(*at, d.limits.setting_model_efforts_item, b"model.efforts")?)
                    .expect("checked list length");
            }
            values
        },
        effort: if d.kind(field(&fields, b"effort")?)? == Kind::Null {
            None
        } else {
            Some(d.words(field(&fields, b"effort")?, d.limits.setting_model_effort_some, b"model.effort")?)
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
            _ => Err(error(b"template.executor", b"unknown word")),
        };
    }
    let fields = d.object(at, Shape::Executor)?;
    if fields.members.len() != 1 {
        return Err(error(b"template.executor", b"one variant required"));
    }
    if let Some(at) = optional(&fields, b"charter") {
        return Ok(c::TemplateExecutor::Charter(
            u32::try_from(d.number(at, fields.shape, b"charter")?)
                .or(Err(error(b"executor.charter", b"integer bound")))?,
        ));
    }
    Err(error(b"template.executor", b"unknown variant"))
}
fn task_contract_document(d: &Decoder, at: u32) -> Result<c::TaskContractDocument, Error> {
    let fields = d.object(at, Shape::Contract)?;
    if fields.members.len() != 1 {
        return Err(error(b"task_contract_document", b"one variant required"));
    }
    if let Some(at) = optional(&fields, b"approval") {
        let fields = d.object(at, Shape::Approval)?;
        return Ok(c::TaskContractDocument::Approval {
            state: d.hex(
                field(&fields, b"state")?,
                d.limits.task_contract_document_approval_state,
                b"approval.state",
            )?,
            part: owner_part(d, field(&fields, b"part")?)?,
        });
    }
    if let Some(at) = optional(&fields, b"report") {
        let fields = d.object(at, Shape::Report)?;
        return Ok(c::TaskContractDocument::Report {
            words: u32::try_from(d.number(field(&fields, b"words")?, fields.shape, b"words")?)
                .or(Err(error(b"report.words", b"integer bound")))?,
        });
    }
    if let Some(at) = optional(&fields, b"verdict") {
        let fields = d.object(at, Shape::Verdict)?;
        return Ok(c::TaskContractDocument::Verdict {
            choices: {
                let positions = d.array(
                    field(&fields, b"choices")?,
                    d.limits.task_contract_document_verdict_choices,
                    b"verdict.choices",
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
        let fields = d.object(at, Shape::Change)?;
        return Ok(c::TaskContractDocument::Change {
            connector: u16::try_from(d.number(field(&fields, b"connector")?, fields.shape, b"connector")?)
                .or(Err(error(b"change.connector", b"integer bound")))?,
            kind: u16::try_from(d.number(field(&fields, b"kind")?, fields.shape, b"kind")?)
                .or(Err(error(b"change.kind", b"integer bound")))?,
            words: u32::try_from(d.number(field(&fields, b"words")?, fields.shape, b"words")?)
                .or(Err(error(b"change.words", b"integer bound")))?,
        });
    }
    Err(error(b"task_contract_document", b"unknown variant"))
}
fn template_budget(d: &Decoder, at: u32) -> Result<c::TemplateBudget, Error> {
    if d.kind(at)? == Kind::String {
        return match d.string(at)? {
            b"given" => Ok(c::TemplateBudget::Given),
            _ => Err(error(b"template.budget", b"unknown word")),
        };
    }
    let fields = d.object(at, Shape::Budget)?;
    if fields.members.len() != 1 {
        return Err(error(b"template.budget", b"one variant required"));
    }
    if let Some(at) = optional(&fields, b"amount") {
        return Ok(c::TemplateBudget::Amount(d.number(at, fields.shape, b"amount")?));
    }
    if let Some(at) = optional(&fields, b"as_allowed") {
        let fields = d.object(at, Shape::AsAllowed)?;
        return Ok(c::TemplateBudget::AsAllowed {
            cap: if d.kind(field(&fields, b"cap")?)? == Kind::Null {
                None
            } else {
                Some(d.number(field(&fields, b"cap")?, fields.shape, b"cap")?)
            },
        });
    }
    Err(error(b"template.budget", b"unknown variant"))
}
fn template_deadline(d: &Decoder, at: u32) -> Result<c::TemplateDeadline, Error> {
    if d.kind(at)? == Kind::String {
        return match d.string(at)? {
            b"none" => Ok(c::TemplateDeadline::None),
            b"given" => Ok(c::TemplateDeadline::Given),
            _ => Err(error(b"template.deadline", b"unknown word")),
        };
    }
    let fields = d.object(at, Shape::Deadline)?;
    if fields.members.len() != 1 {
        return Err(error(b"template.deadline", b"one variant required"));
    }
    if let Some(at) = optional(&fields, b"span") {
        return Ok(c::TemplateDeadline::Span(Duration::from_nanos(d.number(at, fields.shape, b"span")?)));
    }
    Err(error(b"template.deadline", b"unknown variant"))
}
fn task_flags(d: &Decoder, at: u32) -> Result<c::TaskFlags, Error> {
    let fields = d.object(at, Shape::Flags)?;
    Ok(c::TaskFlags {
        conversation: d.boolean(field(&fields, b"conversation")?)?,
        listed: d.boolean(field(&fields, b"listed")?)?,
        retention: retention(d, field(&fields, b"retention")?)?,
        ended_by: ended_by(d, field(&fields, b"ended_by")?)?,
    })
}
fn llm_choice(d: &Decoder, at: u32) -> Result<c::LlmChoice, Error> {
    let fields = d.object(at, Shape::Llm)?;
    Ok(c::LlmChoice {
        account: if d.kind(field(&fields, b"account")?)? == Kind::Null {
            None
        } else {
            Some(
                u32::try_from(d.number(field(&fields, b"account")?, fields.shape, b"account")?)
                    .or(Err(error(b"llm.account", b"integer bound")))?,
            )
        },
        main: model_effort(d, field(&fields, b"main")?)?,
        subagents: {
            let positions = d.array(field(&fields, b"subagents")?, d.limits.llm_choice_subagents, b"llm.subagents")?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values.push(model_effort(d, *at)?).expect("checked list length");
            }
            values
        },
    })
}
fn owner_authority(d: &Decoder, at: u32) -> Result<c::OwnerAuthority, Error> {
    let fields = d.object(at, Shape::Authority)?;
    Ok(c::OwnerAuthority {
        models: Some(authority_models(d, field(&fields, b"models")?)?),
        tools: d.number(field(&fields, b"tools")?, fields.shape, b"tools")?,
        grants: {
            let positions =
                d.array(field(&fields, b"grants")?, d.limits.owner_authority_grants, b"authority.grants")?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values.push(owner_grant(d, *at)?).expect("checked list length");
            }
            values
        },
        delegation: owner_delegation(d, field(&fields, b"delegation")?)?,
        spend: d.number(field(&fields, b"spend")?, fields.shape, b"spend")?,
        deadline: if d.kind(field(&fields, b"deadline")?)? == Kind::Null {
            None
        } else {
            Some(Wall::from_nanos(d.number(field(&fields, b"deadline")?, fields.shape, b"deadline")?))
        },
        notes: u8::try_from(d.number(field(&fields, b"notes")?, fields.shape, b"notes")?)
            .or(Err(error(b"authority.notes", b"integer bound")))?,
        note_resources: {
            let positions = d.array(
                field(&fields, b"note_resources")?,
                d.limits.owner_authority_note_resources,
                b"authority.note_resources",
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
    let fields = d.object(at, Shape::Grant)?;
    Ok(c::OwnerGrant {
        connector: u16::try_from(d.number(field(&fields, b"connector")?, fields.shape, b"connector")?)
            .or(Err(error(b"grant.connector", b"integer bound")))?,
        kind: u16::try_from(d.number(field(&fields, b"kind")?, fields.shape, b"kind")?)
            .or(Err(error(b"grant.kind", b"integer bound")))?,
        pattern: owner_pattern(d, field(&fields, b"pattern")?)?,
    })
}
fn policy_edit_role(d: &Decoder, at: u32) -> Result<c::PolicyEditRole, Error> {
    let fields = d.object(at, Shape::Role)?;
    Ok(c::PolicyEditRole {
        number: u32::try_from(d.number(field(&fields, b"number")?, fields.shape, b"number")?)
            .or(Err(error(b"role.number", b"integer bound")))?,
        authority: owner_authority(d, field(&fields, b"authority")?)?,
        period_spend: d.number(field(&fields, b"period_spend")?, fields.shape, b"period_spend")?,
        requests: u16::try_from(d.number(field(&fields, b"requests")?, fields.shape, b"requests")?)
            .or(Err(error(b"role.requests", b"integer bound")))?,
        decides: u8::try_from(d.number(field(&fields, b"decides")?, fields.shape, b"decides")?)
            .or(Err(error(b"role.decides", b"integer bound")))?,
    })
}
fn policy_requirement(d: &Decoder, at: u32) -> Result<c::PolicyRequirement, Error> {
    let fields = d.object(at, Shape::Requirement)?;
    Ok(c::PolicyRequirement {
        connector: u16::try_from(d.number(field(&fields, b"connector")?, fields.shape, b"connector")?)
            .or(Err(error(b"requirement.connector", b"integer bound")))?,
        kind: u16::try_from(d.number(field(&fields, b"kind")?, fields.shape, b"kind")?)
            .or(Err(error(b"requirement.kind", b"integer bound")))?,
        pattern: owner_pattern(d, field(&fields, b"pattern")?)?,
        judge: policy_judge(d, field(&fields, b"judge")?)?,
        guard: policy_guard(d, field(&fields, b"guard")?)?,
        must_be_guarded: d.boolean(field(&fields, b"must_be_guarded")?)?,
    })
}
fn policy_permission(d: &Decoder, at: u32) -> Result<c::PolicyPermission, Error> {
    let fields = d.object(at, Shape::Permission)?;
    Ok(c::PolicyPermission {
        connector: u16::try_from(d.number(field(&fields, b"connector")?, fields.shape, b"connector")?)
            .or(Err(error(b"permission.connector", b"integer bound")))?,
        permission: u16::try_from(d.number(field(&fields, b"permission")?, fields.shape, b"permission")?)
            .or(Err(error(b"permission.permission", b"integer bound")))?,
        role: u32::try_from(d.number(field(&fields, b"role")?, fields.shape, b"role")?)
            .or(Err(error(b"permission.role", b"integer bound")))?,
    })
}
fn template_override(d: &Decoder, at: u32) -> Result<c::TemplateOverride, Error> {
    let fields = d.object(at, Shape::TemplateOverride)?;
    Ok(c::TemplateOverride {
        template: u32::try_from(d.number(field(&fields, b"template")?, fields.shape, b"template")?)
            .or(Err(error(b"template_override.template", b"integer bound")))?,
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
                u32::try_from(d.number(field(&fields, b"priority")?, fields.shape, b"priority")?)
                    .or(Err(error(b"template_override.priority", b"integer bound")))?,
            )
        },
    })
}
fn owner_kind_implication(d: &Decoder, at: u32) -> Result<c::OwnerKindImplication, Error> {
    let fields = d.object(at, Shape::Implication)?;
    Ok(c::OwnerKindImplication {
        connector: u16::try_from(d.number(field(&fields, b"connector")?, fields.shape, b"connector")?)
            .or(Err(error(b"implication.connector", b"integer bound")))?,
        kind: u16::try_from(d.number(field(&fields, b"kind")?, fields.shape, b"kind")?)
            .or(Err(error(b"implication.kind", b"integer bound")))?,
        implies: u16::try_from(d.number(field(&fields, b"implies")?, fields.shape, b"implies")?)
            .or(Err(error(b"implication.implies", b"integer bound")))?,
    })
}
fn owner_part(d: &Decoder, at: u32) -> Result<c::OwnerPart, Error> {
    let fields = d.object(at, Shape::Part)?;
    Ok(c::OwnerPart {
        connector: u16::try_from(d.number(field(&fields, b"connector")?, fields.shape, b"connector")?)
            .or(Err(error(b"part.connector", b"integer bound")))?,
        kind: owner_part_kind(d, field(&fields, b"kind")?)?,
        key: owner_part_key(d, field(&fields, b"key")?)?,
    })
}
fn verdict_choice(d: &Decoder, at: u32) -> Result<c::VerdictChoice, Error> {
    let fields = d.object(at, Shape::Choice)?;
    Ok(c::VerdictChoice {
        code: u32::try_from(d.number(field(&fields, b"code")?, fields.shape, b"code")?)
            .or(Err(error(b"choice.code", b"integer bound")))?,
        words: u32::try_from(d.number(field(&fields, b"words")?, fields.shape, b"words")?)
            .or(Err(error(b"choice.words", b"integer bound")))?,
        followups: u32::try_from(d.number(field(&fields, b"followups")?, fields.shape, b"followups")?)
            .or(Err(error(b"choice.followups", b"integer bound")))?,
        fields: {
            let positions = d.array(field(&fields, b"fields")?, d.limits.verdict_choice_fields, b"choice.fields")?;
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
    let fields = d.object(at, Shape::ModelEffort)?;
    Ok(c::ModelEffort {
        model: d.words(field(&fields, b"model")?, d.limits.model_effort_model, b"model_effort.model")?,
        effort: if d.kind(field(&fields, b"effort")?)? == Kind::Null {
            None
        } else {
            Some(d.words(field(&fields, b"effort")?, d.limits.model_effort_effort_some, b"model_effort.effort")?)
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
    let fields = d.object(at, Shape::Models)?;
    if fields.members.len() != 1 {
        return Err(error(b"authority_models", b"one variant required"));
    }
    if let Some(at) = optional(&fields, b"named") {
        return Ok(c::AuthorityModels::Named({
            let positions = d.array(at, d.limits.model_set_names_names, b"models.named")?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values
                    .push(d.hex(*at, d.limits.model_set_names_names_item, b"models.named")?)
                    .expect("checked list length");
            }
            values
        }));
    }
    Err(error(b"authority_models", b"unknown variant"))
}
fn owner_delegation(d: &Decoder, at: u32) -> Result<c::OwnerDelegation, Error> {
    let fields = d.object(at, Shape::Delegation)?;
    Ok(c::OwnerDelegation {
        kinds: {
            let positions = d.array(field(&fields, b"kinds")?, d.limits.owner_delegation_kinds, b"delegation.kinds")?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values.push(owner_authority_executor(d, *at)?).expect("checked list length");
            }
            values
        },
        tasks: u32::try_from(d.number(field(&fields, b"tasks")?, fields.shape, b"tasks")?)
            .or(Err(error(b"delegation.tasks", b"integer bound")))?,
        depth: u32::try_from(d.number(field(&fields, b"depth")?, fields.shape, b"depth")?)
            .or(Err(error(b"delegation.depth", b"integer bound")))?,
    })
}
fn owner_resource_scope(d: &Decoder, at: u32) -> Result<c::OwnerResourceScope, Error> {
    let fields = d.object(at, Shape::Resource)?;
    Ok(c::OwnerResourceScope {
        connector: u16::try_from(d.number(field(&fields, b"connector")?, fields.shape, b"connector")?)
            .or(Err(error(b"resource.connector", b"integer bound")))?,
        pattern: owner_pattern(d, field(&fields, b"pattern")?)?,
    })
}
fn owner_pattern(d: &Decoder, at: u32) -> Result<c::OwnerPattern, Error> {
    let fields = d.object(at, Shape::Pattern)?;
    Ok(c::OwnerPattern {
        segments: {
            let positions =
                d.array(field(&fields, b"segments")?, d.limits.owner_pattern_segments, b"pattern.segments")?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values
                    .push(d.hex(*at, d.limits.owner_pattern_segments_item, b"pattern.segments")?)
                    .expect("checked list length");
            }
            values
        },
        last: owner_pattern_end(d, field(&fields, b"last")?)?,
    })
}
fn policy_judge(d: &Decoder, at: u32) -> Result<c::PolicyJudge, Error> {
    let fields = d.object(at, Shape::Judge)?;
    Ok(c::PolicyJudge {
        connector: u16::try_from(d.number(field(&fields, b"connector")?, fields.shape, b"connector")?)
            .or(Err(error(b"judge.connector", b"integer bound")))?,
        requirement: u16::try_from(d.number(field(&fields, b"requirement")?, fields.shape, b"requirement")?)
            .or(Err(error(b"judge.requirement", b"integer bound")))?,
        parameters: u32::try_from(d.number(field(&fields, b"parameters")?, fields.shape, b"parameters")?)
            .or(Err(error(b"judge.parameters", b"integer bound")))?,
    })
}
fn policy_guard(d: &Decoder, at: u32) -> Result<c::PolicyGuard, Error> {
    if d.kind(at)? == Kind::String {
        return match d.string(at)? {
            b"guarded" => Ok(c::PolicyGuard::Guarded),
            _ => Err(error(b"policy_guard", b"unknown word")),
        };
    }
    let fields = d.object(at, Shape::Guard)?;
    if fields.members.len() != 1 {
        return Err(error(b"policy_guard", b"one variant required"));
    }
    if let Some(at) = optional(&fields, b"observed") {
        let fields = d.object(at, Shape::Observed)?;
        return Ok(c::PolicyGuard::Observed {
            freshness: Duration::from_nanos(d.number(field(&fields, b"freshness")?, fields.shape, b"freshness")?),
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
            _ => Err(error(b"part.kind", b"unknown word")),
        };
    }
    Err(error(b"part.kind", b"expected word"))
}
fn owner_part_key(d: &Decoder, at: u32) -> Result<c::OwnerPartKey, Error> {
    let fields = d.object(at, Shape::Key)?;
    if fields.members.len() != 1 {
        return Err(error(b"part.key", b"one variant required"));
    }
    if let Some(at) = optional(&fields, b"path") {
        return Ok(c::OwnerPartKey::Path({
            let positions = d.array(at, d.limits.owner_part_key_path_value, b"key.path")?;
            let mut values = List::with_capacity(positions.len());
            for at in &positions {
                values
                    .push(d.hex(*at, d.limits.owner_part_key_path_value_item, b"key.path")?)
                    .expect("checked list length");
            }
            values
        }));
    }
    if let Some(at) = optional(&fields, b"number") {
        return Ok(c::OwnerPartKey::Number(d.number(at, fields.shape, b"number")?));
    }
    Err(error(b"part.key", b"unknown variant"))
}
fn field_rule(d: &Decoder, at: u32) -> Result<c::FieldRule, Error> {
    let fields = d.object(at, Shape::ChoiceField)?;
    Ok(c::FieldRule {
        name: d.hex(field(&fields, b"name")?, d.limits.field_rule_name, b"choice_field.name")?,
        max: u32::try_from(d.number(field(&fields, b"max")?, fields.shape, b"max")?)
            .or(Err(error(b"choice_field.max", b"integer bound")))?,
    })
}
fn owner_authority_executor(d: &Decoder, at: u32) -> Result<c::OwnerAuthorityExecutor, Error> {
    let fields = d.object(at, Shape::Delegate)?;
    if fields.members.len() != 1 {
        return Err(error(b"authority.executor", b"one variant required"));
    }
    if let Some(at) = optional(&fields, b"charter") {
        return Ok(c::OwnerAuthorityExecutor::Charter(
            u32::try_from(d.number(at, fields.shape, b"charter")?)
                .or(Err(error(b"delegate.charter", b"integer bound")))?,
        ));
    }
    if let Some(at) = optional(&fields, b"procedure") {
        return Ok(c::OwnerAuthorityExecutor::Procedure(
            u32::try_from(d.number(at, fields.shape, b"procedure")?)
                .or(Err(error(b"delegate.procedure", b"integer bound")))?,
        ));
    }
    if let Some(at) = optional(&fields, b"role") {
        return Ok(c::OwnerAuthorityExecutor::Role(
            u32::try_from(d.number(at, fields.shape, b"role")?).or(Err(error(b"delegate.role", b"integer bound")))?,
        ));
    }
    Err(error(b"authority.executor", b"unknown variant"))
}
fn owner_pattern_end(d: &Decoder, at: u32) -> Result<c::OwnerPatternEnd, Error> {
    let fields = d.object(at, Shape::Last)?;
    if fields.members.len() != 1 {
        return Err(error(b"owner_pattern_end", b"one variant required"));
    }
    if let Some(at) = optional(&fields, b"exact") {
        return Ok(c::OwnerPatternEnd::Exact(d.hex(at, d.limits.owner_pattern_end_exact_value, b"last.exact")?));
    }
    if let Some(at) = optional(&fields, b"open") {
        return Ok(c::OwnerPatternEnd::Open(d.hex(at, d.limits.owner_pattern_end_open_value, b"last.open")?));
    }
    Err(error(b"owner_pattern_end", b"unknown variant"))
}

#[cfg(test)]
mod tests {
    use super::{Decoder, llm_choice};

    #[test]
    fn a_short_model_choice_takes_the_tables_defaults() {
        let d = Decoder {
            doc: crate::json::document_depth(br#"{"main":{"model":"opus","effort":"high"}}"#, 32).expect("JSON"),
            limits: crate::Limits::standard().settings,
        };
        let Ok(choice) = llm_choice(&d, 0) else { panic!("short model choice") };
        assert_eq!(choice.account, None);
        assert!(choice.subagents.is_empty());
        assert_eq!(choice.main.model.bytes(), b"opus");
        assert_eq!(choice.main.effort.expect("effort").bytes(), b"high");
    }

    #[test]
    fn a_missing_required_member_names_its_shape_path() {
        let d = Decoder {
            doc: crate::json::document_depth(br#"{"account":null}"#, 32).expect("JSON"),
            limits: crate::Limits::standard().settings,
        };
        let Err(why) = llm_choice(&d, 0) else { panic!("main is required") };
        assert_eq!(super::super::message(Some(1), why).as_ref(), b"settings file item 1: llm.main: missing member");
    }
    #[test]
    fn an_integer_overflow_names_its_member_path() {
        let d = Decoder {
            doc: crate::json::document_depth(br#"{"account":18446744073709551616,"main":{"model":"opus"}}"#, 32)
                .expect("JSON"),
            limits: crate::Limits::standard().settings,
        };
        let Err(why) = llm_choice(&d, 0) else { panic!("account overflows u64") };
        assert_eq!(super::super::message(Some(1), why).as_ref(), b"settings file item 1: llm.account: integer bound");
    }
}
