//! Shared ctl settings input and text, bounded for host tools (domain/agent.md, 4.3).
use crate::{Decoded, Problem, json::Value};
use alloc::boxed::Box;
use jig_domain_core::{SettingsAnswer, SettingsRead, settings as s};
use jig_protocol_engine_clients::{self as protocol, translate::settings as wire};
use settings_form::view as c;
use settings_form::wire as client;
use skein_lib::List;
use smith_domain_run as run;
use temper_engine_domain as engine;

fn only(value: &Value, fields: &[&[u8]], name: &'static [u8]) -> Result<(), Problem> {
    let Value::Object(items) = value else { return Err(Problem::Type(name)) };
    for (key, _) in items {
        if !fields.contains(&key.as_ref()) {
            return Err(Problem::Range(name));
        }
    }
    Ok(())
}
fn positive(value: &Value, name: &'static [u8]) -> Result<u32, Problem> {
    let n = value.small(name)?;
    if n == 0 { Err(Problem::Range(name)) } else { Ok(n) }
}
pub(crate) fn read(value: &Value) -> Result<engine::Tool, Problem> {
    only(value, &[b"setting", b"revision", b"history", b"from"], b"settings")?;
    let from = match value.field(b"from")? {
        Some(v) => v.small(b"from")?,
        None => 0,
    };
    let revision = match value.field(b"revision")? {
        Some(v) => Some(positive(v, b"revision")?),
        None => None,
    };
    let setting = value.field(b"setting")?;
    let history = value.field(b"history")?;
    let read = if let Some(history) = history {
        if setting.is_some() || revision.is_some() {
            return Err(Problem::Range(b"history"));
        }
        only(history, &[b"before"], b"history")?;
        let before = match history.field(b"before")? {
            Some(v) => Some(positive(v, b"before")?),
            None => None,
        };
        SettingsRead::Revisions { before }
    } else if let Some(setting) = setting {
        only(setting, &[b"kind", b"number"], b"setting")?;
        let kind = setting.required(b"kind")?.text(b"kind")?;
        let number = setting.field(b"number")?;
        let setting = if kind.as_ref() == b"rules" {
            if number.is_some() {
                return Err(Problem::Range(b"number"));
            }
            s::Setting::Rules
        } else {
            let number = positive(number.ok_or(Problem::Missing(b"number"))?, b"number")?;
            match kind.as_ref() {
                b"charter" => s::Setting::Charter(number),
                b"template" => s::Setting::Template(number),
                b"policy" => s::Setting::Policy(number),
                _ => return Err(Problem::Range(b"kind")),
            }
        };
        SettingsRead::Setting { setting, revision: revision.unwrap_or(0) }
    } else if let Some(number) = revision {
        SettingsRead::Revision { number }
    } else {
        SettingsRead::Inventory
    };
    Ok(engine::Tool::Settings { read, from })
}

fn models(values: &[engine::Model]) -> Box<[protocol::Model]> {
    let mut models = List::with_capacity(u32::try_from(values.len()).expect("configured models"));
    for value in values {
        models
            .push(protocol::Model {
                name: value.name.clone(),
                provider: None,
                window: value.window,
                output: value.output,
                efforts: value.efforts.clone(),
                prices: protocol::Prices {
                    input: value.input_price,
                    cached: value.cached_price,
                    cache_write: value.cache_write_price,
                    output: value.output_price,
                    unit: value.price_unit,
                },
                description: None,
            })
            .expect("one row per configured model");
    }
    models.into_boxed()
}
pub(crate) fn proposal(name: run::CallName, bytes: &[u8], configured: &[engine::Model]) -> Result<Decoded, Problem> {
    let value = crate::json::parse(bytes)?;
    only(&value, &[b"against", b"items", b"reason"], b"propose_settings")?;
    let against = positive(value.required(b"against")?, b"against")?;
    let reason = value.required(b"reason")?.text(b"reason")?;
    let limits = settings_form::Limits::standard();
    let items = match settings_form::file::proposal_items(bytes, &limits) {
        Ok(items) => items,
        Err(why) => return Ok(Decoded::Answered(run::HostAnswer::new(why, true).ok_or(Problem::TooLarge)?)),
    };
    let wire = client::wire_change_settings(
        &c::ChangeSettings { against, items, running: c::Running::Stays },
        &limits.settings,
    )
    .or(Err(Problem::Range(b"items")))?;
    let items = match wire::items(wire.into_parts().items, &models(configured), &limits.settings) {
        Ok(items) => items,
        Err(why) => {
            if why.field == jig_client_protocol_wire::v19::Path::TemplateLlm {
                return Err(Problem::Range(b"items.template.llm"));
            }
            return Err(Problem::Range(b"items"));
        }
    };
    Ok(Decoded::Call(engine::Call {
        completion: name.completion,
        position: name.position,
        tool: engine::Tool::ProposeSettings { against, items, reason },
    }))
}

/// Render the same settings document through the same text layer as ctl.
pub fn settings_text(
    answer: &SettingsAnswer,
    requested: Option<u64>,
    configured: &[engine::Model],
) -> Result<Box<[u8]>, Problem> {
    let limits = settings_form::Limits::standard();
    let model = models(configured);
    let value = match answer {
        SettingsAnswer::Inventory(value) => c::ReadValue::Settings(Box::new(
            client::restore_settings_document(
                &wire::document(value, &limits.settings).or(Err(Problem::Range(b"items")))?,
                &limits,
            )
            .or(Err(Problem::Range(b"items")))?,
        )),
        SettingsAnswer::Setting(value) => c::ReadValue::Setting(Box::new(
            client::restore_setting_document(
                &wire::setting_document(value, &model, &limits.settings).or(Err(Problem::Range(b"items")))?,
                &limits,
            )
            .or(Err(Problem::Range(b"items")))?,
        )),
        SettingsAnswer::Revision(value) => c::ReadValue::Revision(Box::new(
            client::restore_revision_answer(
                &wire::revision(value, &model, &limits.settings).or(Err(Problem::Range(b"items")))?,
                &limits,
            )
            .or(Err(Problem::Range(b"items")))?,
        )),
        SettingsAnswer::Revisions(value) => c::ReadValue::Revisions(Box::new(
            client::restore_revisions_page(
                &wire::revisions(value, &limits.settings).or(Err(Problem::Range(b"items")))?,
                &limits,
            )
            .or(Err(Problem::Range(b"items")))?,
        )),
        SettingsAnswer::Unknown => return Ok(Box::from(b"unknown\n".as_slice())),
    };
    Ok(settings_form::settings_text(&value, requested, &limits))
}

/// A bounded host page. Offsets count original text bytes; JSON escaping is
/// priced as `read_docs` prices it, with a line cut before a long-line cut.
#[must_use]
pub fn settings_answer(
    answer: &SettingsAnswer,
    from: u32,
    requested: Option<u64>,
    models: &[engine::Model],
) -> run::HostAnswer {
    match settings_text(answer, requested, models) {
        Ok(text) => match settings_form::page::page(&text, from) {
            Ok(body) => {
                let error = match answer {
                    SettingsAnswer::Unknown => true,
                    SettingsAnswer::Inventory(_)
                    | SettingsAnswer::Setting(_)
                    | SettingsAnswer::Revision(_)
                    | SettingsAnswer::Revisions(_) => false,
                };
                run::HostAnswer::new(body, error).expect("settings page bound")
            }
            Err(_) => run::HostAnswer::new(Box::from(b"from: not a UTF-8 text offset".as_slice()), true)
                .expect("fixed feedback"),
        },
        Err(_) => {
            run::HostAnswer::new(Box::from(b"settings: document limit".as_slice()), true).expect("fixed feedback")
        }
    }
}

pub(crate) fn refusal(why: &jig_domain_people::Refusal) -> Box<[u8]> {
    use jig_domain_people::{Malformed, Refusal};
    let view = if let Refusal::Stale { newest } = why {
        settings_form::Refusal::Stale(*newest)
    } else if let Refusal::NotWellFormed { at, why: Malformed::Settings { setting, fault } } = why {
        let setting = match setting {
            s::Setting::Charter(number) => c::Setting::Charter(c::SettingNumber { number: *number }),
            s::Setting::Template(number) => c::Setting::Template(c::SettingNumber { number: *number }),
            s::Setting::Policy(number) => c::Setting::Policy(c::SettingNumber { number: *number }),
            s::Setting::Rules => c::Setting::Rules,
        };
        let fault = match fault {
            jig_domain_people::SettingFault::Shape => c::SettingFault::Shape,
            jig_domain_people::SettingFault::Count => c::SettingFault::Count,
            jig_domain_people::SettingFault::Reference => c::SettingFault::Reference,
            jig_domain_people::SettingFault::Rules => c::SettingFault::Rules,
            jig_domain_people::SettingFault::Budget => c::SettingFault::Budget,
            jig_domain_people::SettingFault::Grace => c::SettingFault::Grace,
            jig_domain_people::SettingFault::Owner => c::SettingFault::Owner,
        };
        settings_form::Refusal::Malformed { at: *at, setting, fault }
    } else if let Refusal::NotWellFormed { why: Malformed::SettingsUnchanged, .. } = why {
        settings_form::Refusal::Unchanged
    } else if let Refusal::Authority { lacking: jig_domain_people::Lack::Approver, .. } = why {
        settings_form::Refusal::Authority
    } else if let Refusal::Limit { .. } = why {
        settings_form::Refusal::Limit
    } else {
        settings_form::Refusal::NotOffered
    };
    settings_form::refusal_text(&view)
}
