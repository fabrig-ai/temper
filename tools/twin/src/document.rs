//! The fake document's strict validation remains temper-fake's.
use crate::{Refusal, Replace, Take};
use serde_json::value::RawValue;
use smith_transcript::{Block, Role};
use std::{collections::BTreeMap, fmt::Write};

type Base = BTreeMap<String, Box<RawValue>>;

pub(crate) fn base(bytes: &[u8]) -> Result<Base, Refusal> {
    if temper_fake::llm::Document::decode(bytes).is_err() {
        return Err(Refusal::Base);
    }
    match serde_json::from_slice(bytes) {
        Ok(base) => Ok(base),
        Err(_) => Err(Refusal::Base),
    }
}

fn quoted(bytes: &[u8]) -> Result<String, Refusal> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Err(Refusal::Bounds { what: "UTF-8 text" });
    };
    Ok(serde_json::to_string(text).expect("text JSON"))
}

pub(crate) fn script(
    take: &Take,
    turns: &[smith_transcript::Turn],
    replaces: &[Replace],
    counts: &mut [u32],
) -> Result<String, Refusal> {
    let mut script = format!("{{\"cue\":{},\"turns\":[", quoted(&take.cue)?);
    for (i, turn) in turns.iter().enumerate() {
        if i != 0 {
            script.push(',');
        }
        script.push_str("{\"lines\":[");
        let mut lines = 0;
        let mut calls = false;
        for message in turn.messages() {
            if *message.role() != Role::Assistant {
                continue;
            }
            for block in message.blocks() {
                if lines == temper_fake::ENTRIES {
                    return Err(Refusal::Bounds { what: "lines" });
                }
                if lines != 0 {
                    script.push(',');
                }
                match block {
                    Block::Text(text) | Block::Refusal(text) => {
                        write!(script, "{{\"text\":{}}}", quoted(text.text())?).expect("writing to String");
                    }
                    Block::Call(call) => {
                        calls = true;
                        let arguments = replace(call.input(), replaces, counts)?;
                        // Raw arguments are embedded without a JSON parse/serialize
                        // round trip, preserving their recorded order and spacing.
                        let Ok(text) = std::str::from_utf8(&arguments) else {
                            return Err(Refusal::Bounds { what: "arguments" });
                        };
                        write!(script, "{{\"call\":{},\"arguments\":{text}}}", quoted(call.name())?)
                            .expect("writing to String");
                    }
                    Block::Opaque(_) | Block::Result(_) => return Err(Refusal::Bounds { what: "assistant block" }),
                }
                lines += 1;
            }
        }
        let finish = if calls { "tool_calls" } else { "stop" };
        let tokens = turn.usage().output().unwrap_or(1);
        write!(script, "],\"finish\":\"{finish}\",\"tokens\":{tokens}}}").expect("writing to String");
        if script.len() > temper_fake::DOCUMENT_BYTES as usize {
            return Err(Refusal::Bounds { what: "document bytes" });
        }
    }
    script.push_str("]}");
    Ok(script)
}

fn replace(input: &[u8], replaces: &[Replace], counts: &mut [u32]) -> Result<Vec<u8>, Refusal> {
    let mut arguments = input.to_vec();
    for (i, replace) in replaces.iter().enumerate() {
        let mut out = Vec::new();
        let mut at = 0;
        while at < arguments.len() {
            if arguments[at..].starts_with(&replace.old) {
                out.extend_from_slice(&replace.new);
                at += replace.old.len();
                counts[i] = counts[i].checked_add(1).ok_or(Refusal::Bounds { what: "replace count" })?;
            } else {
                out.push(arguments[at]);
                at += 1;
            }
            if out.len() > temper_fake::DOCUMENT_BYTES as usize {
                return Err(Refusal::Bounds { what: "arguments" });
            }
        }
        arguments = out;
    }
    if !matches!(serde_json::from_slice::<serde_json::Value>(&arguments), Ok(serde_json::Value::Object(_))) {
        return Err(Refusal::Bounds { what: "replacement arguments" });
    }
    Ok(arguments)
}

pub(crate) fn finish(base: &mut Base, scripts: &[String]) -> Result<Box<[u8]>, Refusal> {
    let scripts = format!("[{}]", scripts.join(","));
    let Ok(scripts) = RawValue::from_string(scripts) else {
        return Err(Refusal::Bounds { what: "scripts" });
    };
    base.insert(String::from("cued"), scripts);
    let bytes = serde_json::to_vec(base).expect("raw JSON members");
    if temper_fake::llm::Document::decode(&bytes).is_err() {
        return Err(Refusal::Bounds { what: "fake document" });
    }
    Ok(bytes.into_boxed_slice())
}
