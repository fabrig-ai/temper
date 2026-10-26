//! Typed host calls cross the protocol boundary before tool decisions and after
//! their answers (jig's domain/engine.md, 7.3; jig's domain/hosts.md, 2).
use super::{
    Call, CallAnswer, CallKey, Decision, Delivery, Domain, Env, Limits, Payload, Request, Work, emit, relay_payload,
};
use alloc::boxed::Box;
use jig_domain_core::{SettledAnswer, SettledCall};
use jig_domain_fleet as fleet;
use jig_domain_host as host;
use skein_lib::{ReplyTo, Token};

/// Root to protocol: a relayed call to decode, or one the fleet dropped.
#[derive(PartialEq, Eq, Debug)]
pub enum HostRequest {
    Decode { to: ReplyTo, task: u64, attempt: u64, call: fleet::Call },
    Dropped { call: fleet::Call },
}

/// Root to protocol after the decision commits: render a decided call's
/// answer, or carry a workspace delivery's settled answer to its host, which
/// the hub's link names only as text (jig's `domain/hosts.md`, 2).
#[derive(PartialEq, Eq, Debug)]
pub enum HostDelivery {
    /// Settings reads have no durable call part; the protocol renders the core document.
    Settings {
        to: ReplyTo,
        answer: Box<jig_domain_core::SettingsAnswer>,
        from: u32,
        requested: Option<u32>,
    },
    Render {
        to: ReplyTo,
        key: CallKey,
        name: Box<[u8]>,
        tool: Box<[u8]>,
        answer: CallAnswer,
    },
    Answer {
        channel: Token,
        task: u64,
        attempt: u64,
        name: Box<[u8]>,
        call: SettledCall,
    },
}

#[derive(Debug)]
pub(crate) struct Flight {
    pub task: u64,
    pub attempt: u64,
    pub key: Option<CallKey>,
    pub render: FlightRender,
    pub name: Box<[u8]>,
    pub tool: Box<[u8]>,
}

/// Only one tool's rendering context is kept per flight. Boxing the change
/// keeps the slot the same size as the existing amendment context.
#[derive(Debug)]
pub(crate) enum FlightRender {
    Plain,
    Settings { from: u32, requested: Option<u32> },
    Amended(crate::Changed),
    Change(Box<ChangeChoice>),
}

#[derive(Debug)]
pub(crate) struct ChangeChoice {
    named: Option<Box<[u8]>>,
    project: u32,
}

pub(crate) fn decode(domain: &mut Domain, to: ReplyTo, task: u64, attempt: u64, call: fleet::Call) {
    let right = to.into_token();
    let to = ReplyTo::new(right);
    let flight = Flight {
        task,
        attempt,
        key: None,
        render: FlightRender::Plain,
        name: call.name.clone(),
        tool: call.tool.clone(),
    };
    assert!(domain.host_calls.insert(right, flight).is_ok(), "typed flights fit fleet calls");
    super::now(
        domain,
        Request::Worker(crate::WorkerRequest::Host(Box::new(HostRequest::Decode { to, task, attempt, call }))),
    );
}

pub(crate) fn decoded(domain: &mut Domain, env: &Env<Limits>, decision: &mut Decision, to: ReplyTo, body: Call) {
    let right = to.into_token();
    let to = ReplyTo::new(right);
    if domain.settling.active() {
        drop(domain.host_calls.remove(&right));
        crate::stopping::call(domain, to);
        return;
    }
    let Some(flight) = domain.host_calls.get_mut(&right) else { return };
    if flight.key.is_some() {
        return;
    }
    let body = super::checked_call(body, &env.limits);
    let key =
        CallKey { task: flight.task, attempt: flight.attempt, completion: body.completion, position: body.position };
    // A call no retry can make valid is answered as an error; one that waits
    // for room is answered busy, and its run asks it again after a backoff
    // (jig's `domain/engine.md`, 5.1).
    let invalid = body.completion == 0 || !super::current_proof(domain, key.task, key.attempt);
    let reading = matches!(body.tool, crate::Tool::Settings { .. });
    let busy = !reading
        && (domain.core.pending_calls.contains_key(&key)
            || (!domain.core.call_parts.contains_key(&key)
                && domain.core.call_parts.len().saturating_add(domain.core.pending_calls.len())
                    >= env.limits.call_records)
            || (super::call_needs_input(&body.tool) && domain.archive_reads.len() >= domain.archive_reads.capacity()));
    if invalid || busy {
        let flight = domain.host_calls.remove(&right).expect("current typed flight");
        let answer = if invalid {
            Payload::SettledCall(SettledCall {
                serial: 0,
                tool: flight.tool,
                answer: SettledAnswer::Host { error: true, body: Box::from(&b"invalid call"[..]) },
            })
        } else {
            Payload::Busy
        };
        let id = domain.payloads.insert(Some(answer)).expect("fleet call reserved answer room");
        domain.work.push(Work::Fleet(fleet::Event::Relayed { to, answer: id.token() }));
        return;
    }
    let render = if let crate::Tool::Settings { from, read } = &body.tool {
        let requested = match read {
            jig_domain_core::SettingsRead::Revision { number } => Some(*number),
            jig_domain_core::SettingsRead::Inventory
            | jig_domain_core::SettingsRead::Setting { .. }
            | jig_domain_core::SettingsRead::Revisions { .. } => None,
        };
        FlightRender::Settings { from: *from, requested }
    } else if let crate::Tool::Change { repository, .. } = &body.tool {
        match domain.core.tasks.delegation(key.task) {
            Some(context) => {
                FlightRender::Change(Box::new(ChangeChoice { named: repository.clone(), project: context.project }))
            }
            None => FlightRender::Plain,
        }
    } else if let crate::Tool::Amend { amendment, .. } = &body.tool {
        FlightRender::Amended(crate::Changed::amendment(amendment))
    } else {
        FlightRender::Plain
    };
    let flight = domain.host_calls.get_mut(&right).expect("current typed flight");
    flight.render = render;
    flight.key = Some(key);
    let id = domain.payloads.insert(Some(Payload::Call { key, body })).expect("fleet call reserved payload room");
    relay_payload(domain, env, decision, to, Token::new(key.task), Token::new(key.attempt), id.token());
}

pub(crate) fn render(domain: &mut Domain, limits: &Limits, decision: &mut Decision, to: ReplyTo, answer: CallAnswer) {
    let right = to.into_token();
    let to = ReplyTo::new(right);
    // A flight dropped with its call has no host left to answer.
    let Some(flight) = domain.host_calls.get(&right) else { return };
    let key = flight.key.expect("decoded typed call");
    if let Some(call) = domain.core.call_settled.get(&key).cloned() {
        drop(domain.host_calls.remove(&right));
        settled(domain, limits, decision, to, call);
        return;
    }
    // Fresh and unsettled replay replies carry only the core's part.
    let answer = match &flight.render {
        FlightRender::Amended(changed) => {
            if let CallAnswer::Revised { task, revision } = answer {
                CallAnswer::Amended { task, revision, changed: *changed }
            } else {
                answer
            }
        }
        FlightRender::Change(change) => {
            if let CallAnswer::DelegationRefused(problem) = &answer
                && problem.why == jig_domain_tasks::Refusal::Unknown
            {
                CallAnswer::ChangeRefused {
                    problem: Box::new(problem.clone()),
                    named: change.named.clone(),
                    repositories: super::forge_route::change_repositories(domain, change.project),
                }
            } else {
                answer
            }
        }
        FlightRender::Plain | FlightRender::Settings { .. } => answer,
    };
    emit(
        decision,
        limits,
        Delivery::Host(Box::new(HostDelivery::Render {
            to,
            key,
            name: flight.name.clone(),
            tool: flight.tool.clone(),
            answer,
        })),
    );
}

pub(crate) fn rendered(domain: &mut Domain, to: ReplyTo, answer: SettledAnswer) {
    let right = to.into_token();
    let to = ReplyTo::new(right);
    let Some(flight) = domain.host_calls.remove(&right) else { return };
    let Some(key) = flight.key else {
        answered_at_decode(domain, to, flight, answer);
        return;
    };
    let mut call = SettledCall { serial: 0, tool: flight.tool, answer };
    let recorded = domain.core.call_parts.contains_key(&key);
    let answer_bytes = match super::call_answer(domain, key) {
        Some(answer) => crate::store::call_answer_bytes(&answer),
        None => Some(0),
    };
    let fits = call.valid(&super::core_limits(&domain.limits))
        && match answer_bytes {
            Some(answer) => match call.owned_bytes() {
                Some(bytes) => match bytes.checked_add(answer) {
                    Some(total) => total <= u64::from(domain.limits.journal.transcript_bytes),
                    None => false,
                },
                None => false,
            },
            None => false,
        };
    if !fits {
        call.answer = SettledAnswer::Host { error: true, body: Box::from(&b"busy"[..]) };
    }
    if fits && recorded {
        domain.work.push(Work::Core(jig_domain_core::Event::SettledCall { to, key, name: flight.name, call }));
    } else {
        let id = domain.payloads.insert(Some(Payload::SettledCall(call))).expect("fleet call reserved answer room");
        domain.work.push(Work::Fleet(fleet::Event::Relayed { to, answer: id.token() }));
    }
}

/// An answer made at decode decides nothing (domain/root.md, 5.2).
fn answered_at_decode(domain: &mut Domain, to: ReplyTo, flight: Flight, answer: SettledAnswer) {
    let mut call = SettledCall { serial: 0, tool: flight.tool, answer };
    call.answer = if call.valid(&super::core_limits(&domain.limits)) {
        match call.answer {
            answer @ SettledAnswer::Host { .. } => answer,
            SettledAnswer::Delivery { .. } => {
                SettledAnswer::Host { error: true, body: Box::from(&b"invalid call"[..]) }
            }
        }
    } else {
        SettledAnswer::Host { error: true, body: Box::from(&b"invalid call"[..]) }
    };
    let id = domain.payloads.insert(Some(Payload::SettledCall(call))).expect("fleet call reserved answer room");
    domain.work.push(Work::Fleet(fleet::Event::Relayed { to, answer: id.token() }));
}

pub(crate) fn settled(domain: &mut Domain, limits: &Limits, decision: &mut Decision, to: ReplyTo, call: SettledCall) {
    let id = domain.payloads.insert(Some(Payload::SettledCall(call))).expect("fleet call reserved answer room");
    emit(decision, limits, Delivery::Fleet(fleet::Event::Relayed { to, answer: id.token() }));
}

/// Core's refusal supplies its own live address; no settled record is kept.
pub(crate) fn refused(domain: &mut Domain, limits: &Limits, decision: &mut Decision, to: ReplyTo, name: Box<[u8]>) {
    let id = domain.payloads.insert(Some(Payload::RefusedCall { name })).expect("fleet call reserved answer room");
    emit(decision, limits, Delivery::Fleet(fleet::Event::Relayed { to, answer: id.token() }));
}

/// Capacity refusal already carries its validated original address; no flight is needed.
pub(crate) fn busy(channel: Token, run: Token, attempt: Token, name: Box<[u8]>) -> Delivery {
    Delivery::Link { channel, down: host::LinkDown::Relayed { run, attempt, call: name, answer: host::Relayed::Busy } }
}

/// Assemble the answer its owner kept; the core chooses its durability route.
pub(crate) fn relayed(
    domain: &mut Domain,
    channel: Token,
    run: Token,
    attempt: Token,
    name: Box<[u8]>,
    answer: Token,
) -> Delivery {
    if domain.settling.active() {
        drop(super::take_payload(domain, answer));
        return busy(channel, run, attempt, name);
    }
    let answer = match super::take_payload(domain, answer) {
        Some(Payload::Busy) => host::Relayed::Busy,
        Some(Payload::RefusedCall { name }) => {
            return Delivery::Link {
                channel,
                down: host::LinkDown::Relayed {
                    run,
                    attempt,
                    call: name,
                    answer: host::Relayed::Answer { error: true, text: Box::from(&b"busy"[..]) },
                },
            };
        }
        Some(Payload::SettledCall(call)) => match call.answer {
            SettledAnswer::Host { error, body } => host::Relayed::Answer { error, text: body },
            SettledAnswer::Delivery { .. } => {
                let (task, attempt) = (run.raw(), attempt.raw());
                return Delivery::Host(Box::new(HostDelivery::Answer { channel, task, attempt, name, call }));
            }
        },
        Some(Payload::Call { .. } | Payload::InboxWord(_) | Payload::Turn { .. } | Payload::Answer { .. }) | None => {
            unreachable!("fleet relays an owned answer")
        }
    };
    Delivery::Link { channel, down: host::LinkDown::Relayed { run, attempt, call: name, answer } }
}

/// A read's owned value crosses the journal door before any durable decision.
pub(crate) fn read(domain: &mut Domain, to: ReplyTo, answer: CallAnswer) {
    let right = to.into_token();
    let to = ReplyTo::new(right);
    let flight = domain.host_calls.get(&right).expect("a read answers its decoded flight");
    let request = crate::boundary::delivery_output(Delivery::Host(Box::new(HostDelivery::Render {
        to,
        key: flight.key.expect("decoded read"),
        name: flight.name.clone(),
        tool: flight.tool.clone(),
        answer,
    })));
    assert!(super::now(domain, request), "read render door reserved");
}

/// A settings read crosses the same journal door as other fresh reads.
pub(crate) fn settings(domain: &mut Domain, to: ReplyTo, answer: Box<jig_domain_core::SettingsAnswer>) {
    let right = to.into_token();
    let to = ReplyTo::new(right);
    let flight = domain.host_calls.get(&right).expect("settings read flight");
    let FlightRender::Settings { from, requested } = flight.render else { unreachable!("settings rendering context") };
    let request = crate::boundary::delivery_output(Delivery::Host(Box::new(HostDelivery::Settings {
        to,
        answer,
        from,
        requested,
    })));
    assert!(super::now(domain, request), "settings read render door reserved");
}
