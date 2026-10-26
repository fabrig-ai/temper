//! Root assignments as the hub's records, with application items beside them.
use alloc::boxed::Box;
use jig_domain_accounts as accounts;
use jig_domain_core as core;
use jig_domain_host as host;
use jig_domain_tasks as tasks;
use jig_protocol_engine_workers as kit;
use skein_lib::{Decimal, List, Time, Token, Writer};
use temper_engine_domain as root;
use temper_protocol_worker_wire as wire;

/// The checked channel limits, endpoint names and deployment identity.
#[derive(Debug)]
pub struct Encoding {
    pub limits: wire::v1::Limits,
    pub endpoints: smith_protocol_channel::Endpoints,
    pub deployment: [u8; 16],
}

/// A transcript tail as text alone. jig's tail copies the retained turns'
/// encoded bytes, so a long one reaches their binary headers, which Smith's
/// brief refuses as not UTF-8: keep what follows the last byte that is not
/// text, behind a cut marker, never longer than the tail given. jig's own fix
/// renders the turns as text.
#[expect(clippy::disallowed_methods, reason = "the worker protocol checks a tail's text before Smith's brief")]
pub(crate) fn text_tail(tail: &[u8]) -> Box<[u8]> {
    const MARKER: &[u8] = b"[earlier bytes not text, cut]\n";
    let mut from = 0_usize;
    let mut at = 0_usize;
    while let Some(rest) = tail.get(at..)
        && !rest.is_empty()
    {
        let (valid, bad) = match ::core::str::from_utf8(rest) {
            Ok(valid) => (valid, 0),
            Err(problem) => {
                let (valid, rest) = rest.split_at_checked(problem.valid_up_to()).expect("valid prefix inside the rest");
                let valid = ::core::str::from_utf8(valid).expect("valid prefix");
                (valid, problem.error_len().unwrap_or(rest.len()))
            }
        };
        for (offset, char) in valid.char_indices() {
            let text = match char {
                '\n' | '\t' => true,
                other => !other.is_control(),
            };
            if !text {
                from = at.saturating_add(offset).saturating_add(char.len_utf8());
            }
        }
        at = at.saturating_add(valid.len());
        if bad != 0 {
            at = at.saturating_add(bad);
            from = at;
        }
    }
    let kept = tail.get(from..).expect("text tail");
    if from == 0 || tail.len() <= MARKER.len() {
        return kept.into();
    }
    let kept = ::core::str::from_utf8(kept).expect("text after the last non-text byte");
    let mut skip = kept.len().saturating_sub(tail.len().saturating_sub(MARKER.len()));
    while !kept.is_char_boundary(skip) {
        skip = skip.saturating_add(1);
    }
    let kept = kept.get(skip..).expect("a char boundary");
    let mut text = Writer::new(MARKER.len().saturating_add(kept.len()));
    text.put(MARKER).expect("marker within the tail's room");
    text.put(kept.as_bytes()).expect("tail within its room");
    text.finish()
}

/// An assignment and its application parts, or none when it cannot encode.
#[must_use]
pub fn down(
    assignment: root::Assignment,
    forges: &[crate::settings::Forge],
    encoding: &Encoding,
    values: &crate::values::Values,
    channel: Token,
    now: Time,
) -> Option<kit::Event> {
    let items = crate::translate::items(&assignment, forges, &values.tokens, &encoding.limits).ok()?;
    let mut charter = temper_engine_smith::charter(&assignment);
    for (section, from) in charter.brief.sections.iter_mut().zip(&assignment.sections) {
        if from.kind == root::BriefKind::Core(jig_domain_brief::Core::TranscriptTail) {
            section.text = text_tail(&section.text);
        }
    }
    let charter =
        smith_protocol_channel::encode_charter(&charter, &smith_charter::CEILINGS, &encoding.endpoints).ok()?;
    let identity = identity(&encoding.deployment, assignment.task, assignment.conversation);
    let mut parts = List::with_capacity(1);
    let workspace = match items {
        Some(items) => {
            let token = Token::new(0);
            parts
                .push(jig_protocol_worker_wire::Part { token, bytes: wire::items(&items) })
                .expect("one workspace part");
            Some(host::Workspace { workstream: assignment.workstream, items: token })
        }
        None => None,
    };
    let grants = grants(&assignment.grants);
    let filled = values.grants(&grants, now);
    Some(kit::Event::Down {
        channel,
        down: host::LinkDown::Assign {
            assignment: host::Assignment {
                messages: messages(&assignment.messages),
                assignment: host::RunAssignment {
                    identity,
                    live: assignment.live,
                    run: Token::new(assignment.task),
                    attempt: Token::new(assignment.attempt),
                    workspace,
                    save: false,
                    charter,
                    grants,
                },
                turns: assignment.transcript,
                answered: answered(&assignment.settled),
            },
        },
        parts: parts.into_boxed(),
        values: filled,
    })
}
fn identity(deployment: &[u8; 16], task: u64, conversation: u64) -> [u8; 16] {
    let mut source = Writer::new(32);
    source.put(deployment).expect("deployment identity");
    source.put(&task.to_be_bytes()).expect("task identity");
    source.put(&conversation.to_be_bytes()).expect("conversation identity");
    let digest = skein_io::digest::digest(&source.finish());
    let mut identity = Writer::new(16);
    for word in digest.0.get(..2).expect("first sixteen digest bytes") {
        identity.put(&word.to_be_bytes()).expect("two identity words");
    }
    identity.finish().as_ref().try_into().expect("sixteen identity bytes")
}
fn messages(words: &[tasks::Word]) -> Box<[host::StartMessage]> {
    let mut messages = List::with_capacity(u32::try_from(words.len()).expect("bounded messages"));
    for word in words {
        let sender = match word.from {
            tasks::Party::Task(number) | tasks::Party::Person(number) => Decimal::of(number).as_bytes().into(),
            tasks::Party::Deployment { .. } => b"deployment".as_slice().into(),
        };
        messages
            .push(host::StartMessage {
                name: Token::new(word.number),
                sender,
                words: root::inbox_words(word),
                finish_requested: word.finish_requested(),
            })
            .expect("bounded messages");
    }
    messages.into_boxed()
}
fn grants(grants: &[accounts::Grant]) -> Box<[host::Grant]> {
    let mut filled = List::with_capacity(u32::try_from(grants.len()).expect("bounded grants"));
    for grant in grants {
        let form = match grant.form {
            accounts::Form::Credential(scheme) => host::Form::Credential(match scheme {
                accounts::Scheme::Bearer => host::Scheme::Bearer,
                accounts::Scheme::Key => host::Scheme::Key,
            }),
            accounts::Form::Application => host::Form::Application,
            accounts::Form::Program(name) => {
                host::Form::Program(host::SourceName::new(name.as_bytes()).expect("same source name grammar"))
            }
        };
        filled
            .push(host::Grant { form, account: grant.account, generation: grant.generation, valid: grant.valid })
            .expect("bounded grants");
    }
    filled.into_boxed()
}
fn answered(calls: &[core::Settled]) -> Box<[host::AnsweredCall]> {
    let mut answered = List::with_capacity(u32::try_from(calls.len()).expect("bounded answered calls"));
    for core::Settled { key, call } in calls {
        let answer = match &call.answer {
            core::SettledAnswer::Host { error, body } => {
                host::SettledAnswer::Host { error: *error, body: body.clone() }
            }
            core::SettledAnswer::Delivery { outcome, evidence } => host::SettledAnswer::Delivery {
                outcome: match outcome {
                    core::DeliveryOutcome::Delivered => host::DeliveryOutcome::Delivered,
                    core::DeliveryOutcome::Nothing => host::DeliveryOutcome::Nothing,
                    core::DeliveryOutcome::Stale => host::DeliveryOutcome::Stale,
                    core::DeliveryOutcome::Refused | core::DeliveryOutcome::Invalid => host::DeliveryOutcome::Refused,
                    core::DeliveryOutcome::Failed => host::DeliveryOutcome::Failed,
                },
                evidence: evidence.clone(),
            },
        };
        answered
            .push(host::AnsweredCall {
                name: jig_protocol_charter::call_name(&jig_protocol_charter::CallKey {
                    activation: key.attempt,
                    completion: key.completion,
                    position: key.position,
                }),
                tool: call.tool.clone(),
                answer,
            })
            .expect("bounded answered calls");
    }
    answered.into_boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_starting_run_hears_its_met_dependency_by_name() {
        // A run resumed by its last dependency's end hears the Met notice that woke it (jig's
        // domain/tasks.md, section 8.1), as a live run does.
        let met = tasks::Word {
            number: 2,
            from: tasks::Party::Task(2),
            kind: tasks::MessageKind::Met { dependency: 2, removed: false },
            words: Box::new([]),
            at: skein_lib::Wall::EPOCH,
            hits: 1,
            eligible: true,
        };
        let started = messages(&[met]);
        assert_eq!(started[0].sender.as_ref(), b"2");
        assert_eq!(started[0].words.as_ref(), b"T2 is done; the task can go on");
    }
}
