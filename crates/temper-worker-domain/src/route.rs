//! Routing (4.5): each of the protocol's events to the child domain it is
//! for, and each child domain's requests to the protocol layer or,
//! translated, to a sibling.
//!
//! Every match is exhaustive, so a variant added to either side's vocabulary
//! breaks the build here.

use alloc::boxed::Box;

use crate::wire;
use jig_domain_host as host;
use skein_lib::{Env, List, Queue, Token};
use smith_host_domain as agent;
use temper_worker_domain_checkout as checkout;

use crate::boundary::{Event, Request};
use crate::domain::Domain;
use crate::limits::Limits;
use crate::workspace::{self, Write};
use crate::{agents, translate};

/// What the host reads: this iteration's times, and its own limits.
pub(crate) const fn host_env(env: &Env<Limits>) -> Env<host::Limits> {
    Env { now: env.now, wall: env.wall, limits: env.limits.host }
}

/// What the checkout reads.
pub(crate) const fn checkout_env(env: &Env<Limits>) -> Env<checkout::Limits> {
    Env { now: env.now, wall: env.wall, limits: env.limits.checkout }
}

/// What the agent child domain reads.
pub(crate) const fn agent_env(env: &Env<Limits>) -> Env<agent::Limits> {
    Env { now: env.now, wall: env.wall, limits: env.limits.agent }
}

/// Hands one of the protocol's events to the child domain it is for.
pub(crate) fn event(domain: &mut Domain, env: &Env<Limits>, event: Event, out: &mut Queue<Request>) {
    match event {
        Event::SourceAnswered { owner, answer } => {
            host_step(domain, env, host::Event::SourceAnswered { owner, answer });
        }
        Event::Assign { assignment, items, save } => assign(domain, env, assignment, items, save, out),
        Event::Link(host::LinkDown::Assign { assignment }) => {
            let items = wire::Workspace { repositories: Box::new([]) };
            assign(domain, env, assignment, Ok(items), None, out);
        }
        Event::Link(host::LinkDown::Acknowledge { run, attempt }) => {
            host_step(domain, env, host::Event::Down(host::LinkDown::Acknowledge { run, attempt }));
            if !domain.host.is_hosting(run, attempt) {
                workspace::forget(domain, run, attempt);
            }
        }
        Event::Link(down) => host_step(domain, env, host::Event::Down(down)),
        Event::Ready => {
            domain.past = false;
            let opened = host::Event::Opened {
                stop_bound: env.limits.stop_bound,
                workstreams: workstreams(&domain.checkout),
                bounds: env.limits.bounds.clone(),
            };
            host_step(domain, env, opened);
        }
        Event::Lost => host_step(domain, env, host::Event::SourceLost),
        Event::GracePassed => {
            domain.past = true;
            host_step(domain, env, host::Event::CancelAll { reason: host::Reason::Contact });
        }
        Event::Shutdown => {
            domain.stopping = true;
            host_step(domain, env, host::Event::CancelAll { reason: host::Reason::Shutdown });
        }
        Event::Resend { run, attempt, turn } => {
            if let Some(turn) = domain.host.turn(run, attempt, turn) {
                out.push(Request::Link(host::LinkUp::Turn { run, attempt, turn }));
            }
        }
        Event::RelayCancelled { run, attempt, call } => {
            host_step(domain, env, host::Event::RelayCancelled { run, attempt, call });
        }
        Event::Process(event) => agent_step(domain, env, agent::Input::Process(*event)),
        Event::Live { agent, piece } => live(domain, env, agent, piece),
        Event::Decoded { agent, start } => agents::decoded(domain, env, agent, start),
        Event::Encoded { owner, encoded } => agents::encoded(domain, env, owner, encoded),
        Event::Activity { owner, activity } => {
            domain.facts_out = domain.facts_out.saturating_sub(1);
            if let Some(activity) = activity {
                host_step(domain, env, host::Event::Activity { owner, activity });
            }
        }
        Event::Done { owner, done } => checkout_step(domain, env, checkout::Event::Done { owner, done }),
    }
}

/// A piece of a run's live content: progress for the agent's watchdog, and
/// the hub's to send up or drop. An agent the root does not know gives
/// nothing.
fn live(domain: &mut Domain, env: &Env<Limits>, source: Token, piece: host::Piece) {
    let mut owner = None;
    for (client, bound) in &domain.agents {
        if bound.agent == source {
            owner = Some(*client);
            break;
        }
    }
    let Some(owner) = owner else { return };
    agent_step(domain, env, agent::Input::Process(agent::process::Event::Progress { owner: source }));
    host_step(domain, env, host::Event::Piece { owner, piece });
}

/// An assignment from the engine. One the hub hosts already, or whose answer
/// it keeps, is a wire retry: dropped. Workspace items the checkout cannot
/// take are refused as an invalid start, in words that name them; the rest
/// goes to the hub, which admits or refuses it.
fn assign(
    domain: &mut Domain,
    env: &Env<Limits>,
    assignment: host::Assignment,
    items: Result<wire::Workspace, wire::Undecodable>,
    save: Option<Box<[u8]>>,
    out: &mut Queue<Request>,
) {
    let (run, attempt) = (assignment.assignment.run, assignment.assignment.attempt);
    if domain.host.is_hosting(run, attempt) {
        return;
    }
    let items = match items {
        Ok(items) => items,
        Err(wire::Undecodable) => {
            let why = words(b"workspace items", env.limits.host.detail_bytes);
            return refuse(
                run,
                attempt,
                host::Refusal::Invalid(host::Invalid::Start { cause: host::StartCause::Other, why }),
                out,
            );
        }
    };
    let checked = if !items.repositories.is_empty() && assignment.assignment.workspace.is_none() {
        Err(&b"workstream"[..])
    } else {
        crate::assignment::check(&items, save.as_deref(), &env.limits)
    };
    if let Err(why) = checked {
        let why = words(why, env.limits.host.detail_bytes);
        return refuse(
            run,
            attempt,
            host::Refusal::Invalid(host::Invalid::Start { cause: host::StartCause::Other, why }),
            out,
        );
    }
    match workspace::stage(domain, assignment, items, save) {
        Some(assignment) => host_step(domain, env, host::Event::Down(host::LinkDown::Assign { assignment })),
        None => refuse(run, attempt, host::Refusal::Busy, out),
    }
}

/// At most `limit` of the bytes `why`, as an answer's detail is.
fn words(why: &[u8], limit: u32) -> Box<[u8]> {
    let keep = why.len().min(usize::try_from(limit).expect("a u32 fits a usize"));
    Box::from(why.get(..keep).expect("within the words"))
}

/// The root's own refusal: it goes once, and holds nothing.
fn refuse(run: Token, attempt: Token, refusal: host::Refusal, out: &mut Queue<Request>) {
    let answer =
        host::Answer { read: None, refused: Box::new([]), turns: 0, spent: 0, ending: host::Ending::Refused(refusal) };
    let work = wire::Work { landed: Box::new([]), saved: None };
    out.push(Request::Answer { run, attempt, answer, work, preparation: None });
}

/// The workstreams the checkout holds workspaces for, in its order.
fn workstreams(checkout: &checkout::Domain) -> Box<[u64]> {
    let mut workstreams = List::with_capacity(checkout.workspaces());
    for nth in 0..checkout.workspaces() {
        let Some(key) = checkout.workstream(nth) else {
            break;
        };
        workstreams.push(translate::workstream(key)).expect("room for every workspace");
    }
    workstreams.into_boxed()
}

/// Routes what the child domains emitted, and what that leads to, until all
/// three have emitted all they will in this entry point: what smith emitted
/// first, then the checkout's, then the hub's, so that each capability's
/// requests are routed before the hub asks it anything more. The finite
/// hand-off chain described in `domain.rs` ensures this drain ends.
pub(crate) fn hand_off(domain: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    loop {
        if let Some(request) = domain.agent_out.pop() {
            match request {
                agent::Output::Process(request) => out.push(Request::Process(Box::new(request))),
                agent::Output::Parent(request) => agents::up(domain, env, request, out),
            }
        } else if let Some(request) = domain.checkout_out.pop() {
            from_checkout(domain, env, request, out);
        } else if let Some(request) = domain.host_out.pop() {
            from_host(domain, env, request, out);
        } else {
            return;
        }
    }
}

pub(crate) fn host_step(domain: &mut Domain, env: &Env<Limits>, event: host::Event) {
    host::step(&mut domain.host, &host_env(env), event, &mut domain.host_out);
}

pub(crate) fn checkout_step(domain: &mut Domain, env: &Env<Limits>, event: checkout::Event) {
    checkout::step(&mut domain.checkout, &checkout_env(env), event, &mut domain.checkout_out);
}

pub(crate) fn agent_step(domain: &mut Domain, env: &Env<Limits>, event: agent::Input) {
    agent::step(&mut domain.agent, &agent_env(env), event, &mut domain.agent_out);
}

/// One of the host's requests: to the engine, or to a capability.
fn from_host(domain: &mut Domain, env: &Env<Limits>, request: host::Request, out: &mut Queue<Request>) {
    let request = match request.to_agent() {
        Ok(request) => return agents::to_agent(domain, env, request, out),
        Err(request) => request,
    };
    match request {
        host::Request::Source { owner, name, operation, deadline } => {
            out.push(Request::Source { owner, name, operation, deadline });
        }
        host::Request::SourceFault => out.push(Request::SourceFault),
        host::Request::Up(host::LinkUp::Answer { run, attempt, answer }) => answered(domain, run, attempt, answer, out),
        host::Request::Up(up) => out.push(Request::Link(up)),
        host::Request::Hello { hello } => {
            out.push(Request::Hello(hello));
            // Bounded: the hub keeps no more turns and answers than its slots.
            for index in 0..domain.host.retained_turns() {
                if let Some(turn) = domain.host.turn_at(index) {
                    out.push(Request::Link(turn));
                }
            }
            for index in 0..domain.host.kept_answers() {
                if let Some(host::LinkUp::Answer { run, attempt, answer }) = domain.host.answer_at(index) {
                    answered(domain, run, attempt, answer, out);
                }
            }
        }
        host::Request::CancelRelay { run, attempt, call } => out.push(Request::CancelRelay { run, attempt, call }),
        host::Request::Prepare { owner, workspace } => workspace::prepare(domain, env, owner, workspace),
        host::Request::Abort { owner } => workspace::abort(domain, env, owner),
        host::Request::Deliver { owner, workspace, title, body } => {
            workspace::write(domain, env, owner, workspace, Write::Push { title, body });
        }
        host::Request::Save { owner, workspace } => workspace::save(domain, env, owner, workspace),
        host::Request::Release { workspace } => workspace::release(domain, env, workspace),
        host::Request::Message { .. }
        | host::Request::Reply { .. }
        | host::Request::Start { .. }
        | host::Request::Grant { .. }
        | host::Request::AcknowledgeAgentTurn { .. }
        | host::Request::Stop { .. } => unreachable!("the agent capability's requests were taken above"),
    }
}

/// A run's answer, from the hub, with temper's parts beside it. A refusal
/// keeps nothing: its items go.
fn answered(domain: &mut Domain, run: Token, attempt: Token, answer: host::Answer, out: &mut Queue<Request>) {
    let (work, preparation) = workspace::parts(domain, &answer);
    let refused = match answer.ending {
        host::Ending::Refused(_) => true,
        host::Ending::Ended { .. } | host::Ending::Parked { .. } | host::Ending::Failed { .. } => false,
    };
    if refused {
        workspace::forget(domain, run, attempt);
    }
    out.push(Request::Answer { run, attempt, answer, work, preparation });
}

/// One of the checkout's requests: out to io, or to the host.
fn from_checkout(domain: &mut Domain, env: &Env<Limits>, request: checkout::Request, out: &mut Queue<Request>) {
    match request {
        checkout::Request::Held { client, hold } => workspace::held(domain, env, client, hold),
        checkout::Request::Prepared { client, prepared } => workspace::prepared(domain, env, client, prepared),
        checkout::Request::Pushed { client, outcome } => workspace::wrote(domain, env, client, outcome, true),
        checkout::Request::Saved { client, outcome } => workspace::wrote(domain, env, client, outcome, false),
        checkout::Request::Released { client } => workspace::released(domain, env, client),
        checkout::Request::Io { owner, op, deadline } => out.push(Request::Io { owner, op, deadline }),
        checkout::Request::Cancel { owner } => out.push(Request::CancelIo { owner }),
    }
}
