//! Shared ctl settings input and text, bounded for host tools (domain/agent.md, 4.3).
use crate::{Decoded, Problem, json::Value};
use alloc::boxed::Box;
use jig_client::contract as c;
use jig_client_protocol_link::translate::settings as client;
use jig_domain_core::{SettingsAnswer, SettingsRead, settings as s};
use jig_protocol_engine_clients::{self as protocol, translate::settings as wire};
use skein_lib::{Decimal, List, Writer};
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
    let limits = temper_client_tools::limits::tools();
    let items = match temper_client_tools::jig::settings_file::proposal_items(bytes, &limits) {
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
    let preset = temper_client_tools::limits::client().client;
    let link = jig_client_protocol_link::Limits::new(preset.domain, preset.exchanges).or(Err(Problem::TooLarge))?;
    let limits = temper_client_tools::limits::tools();
    let model = models(configured);
    let value = match answer {
        SettingsAnswer::Inventory(value) => c::ReadValue::Settings(Box::new(
            client::restore_settings_document(
                &wire::document(value, &limits.settings).or(Err(Problem::Range(b"items")))?,
                &link,
            )
            .or(Err(Problem::Range(b"items")))?,
        )),
        SettingsAnswer::Setting(value) => c::ReadValue::Setting(Box::new(
            client::restore_setting_document(
                &wire::setting_document(value, &model, &limits.settings).or(Err(Problem::Range(b"items")))?,
                &link,
            )
            .or(Err(Problem::Range(b"items")))?,
        )),
        SettingsAnswer::Revision(value) => c::ReadValue::Revision(Box::new(
            client::restore_revision_answer(
                &wire::revision(value, &model, &limits.settings).or(Err(Problem::Range(b"items")))?,
                &link,
            )
            .or(Err(Problem::Range(b"items")))?,
        )),
        SettingsAnswer::Revisions(value) => c::ReadValue::Revisions(Box::new(
            client::restore_revisions_page(
                &wire::revisions(value, &limits.settings).or(Err(Problem::Range(b"items")))?,
                &link,
            )
            .or(Err(Problem::Range(b"items")))?,
        )),
        SettingsAnswer::Unknown => return Ok(Box::from(b"unknown\n".as_slice())),
    };
    Ok(temper_client_tools::jig::render::settings_text(&value, requested, &limits))
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
        Ok(text) => match page(&text, from) {
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
pub(crate) fn page(text: &[u8], from: u32) -> Result<Box<[u8]>, Problem> {
    let start = usize::try_from(from).expect("u32 fits").min(text.len());
    let rest = text.get(start..).expect("text suffix");
    // The shared renderer produces validated UTF-8; an offset may not split it.
    if !boundary(text, start) {
        return Err(Problem::Range(b"from"));
    }
    let mut end = start;
    let mut line = start;
    let mut size = 0_u32;
    for byte in rest {
        let cost = if [b'"', b'\\', b'\n'].contains(byte) { 2 } else { 1 };
        if size.checked_add(cost).ok_or(Problem::TooLarge)? > 1536 {
            break;
        }
        size = size.checked_add(cost).expect("page bound");
        end = end.checked_add(1).expect("bounded text");
        if *byte == b'\n' {
            line = end;
        }
    }
    if end < text.len() && line > start {
        end = line;
    }
    while !boundary(text, end) {
        end = end.checked_sub(1).ok_or(Problem::Range(b"from"))?;
    }
    let body = text.get(start..end).expect("page range");
    let next = Decimal::of(u64::try_from(end).expect("text offset"));
    let marker = if end < text.len() { next.as_bytes().len().checked_add(7).ok_or(Problem::TooLarge)? } else { 4 };
    let separator = usize::from(!body.is_empty() && !body.ends_with(b"\n"));
    let size = body.len().checked_add(separator).ok_or(Problem::TooLarge)?;
    let mut out = Writer::new(size.checked_add(marker).ok_or(Problem::TooLarge)?);
    out.put(body).expect("page room");
    if !body.is_empty() && !body.ends_with(b"\n") {
        out.put(b"\n").expect("separator room");
    }
    if end < text.len() {
        out.put(b"next: ").expect("marker room");
        out.put(next.as_bytes()).expect("offset room");
        out.put(b"\n").expect("marker room");
    } else {
        out.put(b"end\n").expect("marker room");
    }
    Ok(out.finish())
}

fn boundary(text: &[u8], at: usize) -> bool {
    match text.get(at) {
        Some(byte) => byte & 0xc0 != 0x80,
        None => true,
    }
}

pub(crate) fn refusal(why: &jig_domain_people::Refusal) -> Box<[u8]> {
    use jig_domain_people::{Malformed, Refusal};
    let value = if let Refusal::Stale { newest } = why {
        c::Refusal::Stale(c::RefusalStale { newest: *newest })
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
        c::Refusal::Malformed { at: *at, why: c::Malformed::Settings(c::MalformedSettings { setting, fault }) }
    } else if let Refusal::NotWellFormed { at, why: Malformed::SettingsUnchanged } = why {
        c::Refusal::Malformed { at: *at, why: c::Malformed::SettingsUnchanged }
    } else if let Refusal::Authority { lacking: jig_domain_people::Lack::Approver, .. } = why {
        c::Refusal::Authority { lacking: c::Lacking::Approver, proposable: Some(false) }
    } else if let Refusal::Limit { which } = why {
        c::Refusal::Limit {
            which: c::Limit::Admission {
                which: if *which == jig_domain_people::LimitKind::SettingsProposals {
                    c::AdmissionLimit::SettingsProposals
                } else {
                    c::AdmissionLimit::Arithmetic
                },
            },
        }
    } else {
        c::Refusal::NotOffered
    };
    temper_client_tools::jig::render::settings_refusal(&value)
}
