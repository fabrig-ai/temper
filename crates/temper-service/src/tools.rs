//! Stateless host tools and fenced results (protocol/llm.md, 4 and 5).
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_domain_host as host;
use jig_domain_tasks as tasks;
use skein_lib::{ReplyTo, Token, Writer};
use smith_domain_run as run;
use temper_engine_domain as engine;
use temper_engine_smith::{Decoded, Problem};

#[cfg(test)]
pub(crate) fn decode(to: ReplyTo, task: u64, attempt: u64, call: &fleet::Call) -> engine::Event {
    decode_configured(to, task, attempt, call, &[])
}
pub(crate) fn decode_configured(
    to: ReplyTo,
    _task: u64,
    attempt: u64,
    call: &fleet::Call,
    models: &[engine::Model],
) -> engine::Event {
    let Some(name) = jig_protocol_charter::call_key(&call.name, attempt) else {
        return invalid(to, Problem::Malformed);
    };
    if call.input.len() > run::HostInput::CAPACITY {
        return invalid(to, Problem::TooLarge);
    }
    let Some(input) = run::HostInput::attested(call.input.clone()) else {
        return invalid(to, Problem::Malformed);
    };
    let name = run::CallName { activation: name.activation, completion: name.completion, position: name.position };
    match temper_engine_smith::call_configured(name, &call.tool, &input, models) {
        Ok(Decoded::Call(body)) => engine::Event::Worker(engine::WorkerInput::DecodedHostCall { to, body }),
        Ok(Decoded::Answered(answer)) => {
            let (body, error) = answer.into_parts();
            engine::Event::Worker(engine::WorkerInput::RenderedHostCall {
                to,
                answer: core::SettledAnswer::Host { error, body },
            })
        }
        Err(problem) => invalid(to, problem),
    }
}
fn invalid(to: ReplyTo, problem: Problem) -> engine::Event {
    let (body, member): (&[u8], Option<&[u8]>) = match problem {
        Problem::Malformed => (b"malformed host call", None),
        Problem::TooLarge => (b"host call too large", None),
        Problem::Missing(member) => (b"host call member missing", Some(member)),
        Problem::Type(member) => (b"host call member of wrong type", Some(member)),
        Problem::Range(member) => (b"host call out of range", Some(member)),
        Problem::UnknownTool => (b"unknown tool", None),
    };
    let body = match member {
        None => body.into(),
        Some(member) => {
            let mut writer = Writer::new(
                body.len()
                    .checked_add(2)
                    .expect("fixed error separator")
                    .checked_add(member.len())
                    .expect("static error member"),
            );
            writer.put(body).expect("measured error");
            writer.put(b": ").expect("measured separator");
            writer.put(member).expect("static member name");
            writer.finish()
        }
    };
    engine::Event::Worker(engine::WorkerInput::RenderedHostCall {
        to,
        answer: core::SettledAnswer::Host { error: true, body },
    })
}
pub(crate) fn render(to: ReplyTo, answer: &engine::CallAnswer) -> engine::Event {
    let (body, error) = temper_engine_smith::answer(answer).into_parts();
    engine::Event::Worker(engine::WorkerInput::RenderedHostCall {
        to,
        answer: core::SettledAnswer::Host { error, body },
    })
}
pub(crate) fn result(
    payload: Token,
    outcome: &[u8],
    change: Option<temper_engine_smith::ChangeResource>,
) -> engine::Event {
    let end = match smith_protocol_channel::decode_declared(outcome, &smith_charter::CEILINGS) {
        Ok(outcome) => {
            let spent = run::Spend {
                turns: 0,
                input: None,
                output: None,
                cache_read: None,
                cache_write: None,
                reasoning: None,
                units: 0,
            };
            match temper_engine_smith::result(run::Answer::Accepted { outcome, spent, turns: 0 }, change) {
                Ok(terminal) => terminal.end,
                Err(_) => tasks::End::Failed(tasks::Class::Invalid),
            }
        }
        Err(_) => tasks::End::Failed(tasks::Class::Invalid),
    };
    engine::Event::Worker(engine::WorkerInput::ResultRead { payload, end })
}
/// The hub carries a settled workspace delivery as host text.
pub(crate) fn settled(answer: core::SettledAnswer) -> host::Relayed {
    match answer {
        core::SettledAnswer::Host { error, body } => host::Relayed::Answer { error, text: body },
        core::SettledAnswer::Delivery { outcome, evidence } => {
            let (label, error): (&[u8], bool) = match outcome {
                core::DeliveryOutcome::Delivered => (b"delivered", false),
                core::DeliveryOutcome::Nothing => (b"nothing to deliver", false),
                core::DeliveryOutcome::Stale => (b"stale delivery", true),
                core::DeliveryOutcome::Refused => (b"delivery refused", true),
                core::DeliveryOutcome::Invalid => (b"invalid delivery", true),
                core::DeliveryOutcome::Failed => (b"delivery failed", true),
            };
            let bytes = label.len().checked_add(2).expect("bounded delivery label");
            let bytes = bytes.checked_add(evidence.len()).expect("bounded delivery text");
            let mut writer = Writer::new(bytes);
            writer.put(label).expect("measured delivery label");
            writer.put(b": ").expect("measured separator");
            writer.put(&evidence).expect("measured evidence");
            let Some(answer) = run::HostAnswer::new(writer.finish(), error) else {
                return host::Relayed::Answer { error: true, text: b"delivery answer too large".as_slice().into() };
            };
            let (text, error) = answer.into_parts();
            host::Relayed::Answer { error, text }
        }
    }
}
