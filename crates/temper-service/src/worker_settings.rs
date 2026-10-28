//! The worker's checked construction bounds and payload sizes (protocol/README.md, 7).
use crate::{limits, settings};
use jig_protocol_config::{self as config, Path, Problems, What};
use jig_protocol_worker_link as link;
use skein_io as io;
use skein_lib::{List, Map, Queue, Token};
use smith_host_protocol as host;
use temper_protocol_worker_wire::{self as payload, v1};
use temper_worker_domain as root;

/// Git's operations and the worker root's close.
pub(crate) fn operations(git: &temper_worker_protocol_git::Limits) -> Option<u32> {
    temper_worker_protocol_git::limits::operations(git)?.checked_add(1)
}

/// Every queue and component bound established before constructing a worker.
#[derive(Debug)]
pub struct Prepared {
    pub root: root::Limits,
    pub link: link::Limits,
    pub payload: v1::Limits,
    pub prefix: u64,
    pub address: io::Addr,
}

pub(crate) fn problem(member: &[u8], what: What) -> Problems {
    let mut problems = Problems::empty();
    problems.push(Path::root().member(b"workers").child(config::Segment::Index(0)).member(member), what);
    problems
}

/// Check the root, host adapter and link before any state is allocated.
pub fn prepare(worker: &settings::Worker, agent: &settings::Agent, own: &limits::Limits) -> Result<Prepared, Problems> {
    io::worst_case(&worker.jig.limits.io).ok_or(problem(b"limits", What::Bound))?;
    let address = match worker.jig.engine {
        config::Address::Addr(address) => address,
        config::Address::Host { .. } => return Err(problem(b"engine", What::Form)),
    };
    programs_worst_case(&agent.programs).ok_or(problem(b"agent", What::Bound))?;
    let domain_limits = root_limits(worker, agent, own);
    root::worst_case(&domain_limits).ok_or(problem(b"limits", What::Bound))?;
    host::Process::new(Token::new(0), &agent.limits, agent.host_limits.detail_bytes, agent.endpoints.clone())
        .or(Err(problem(b"agent", What::Bound)))?;
    let payload = payload_limits(own).ok_or(problem(b"limits", What::Bound))?;
    let link = link_limits(&worker.jig, &payload).ok_or(problem(b"limits", What::Bound))?;
    link::worst_case(&link).ok_or(problem(b"limits", What::Bound))?;
    let prefix = crate::tokens::worker(0).ok_or(problem(b"slots", What::Bound))?;
    worker.jig.slots.checked_mul(4).ok_or(problem(b"slots", What::Bound))?;
    worker.jig.limits.core.accounts.accounts.checked_add(1).ok_or(problem(b"limits", What::Bound))?;
    let bodies = &agent.limits.bodies;
    if credential_bytes(worker.jig.limits.token) > bodies.credential_value_bytes
        || u32::try_from(smith_host_domain::SourceName::BYTES).expect("bounded source name") > bodies.program_value_name
    {
        return Err(problem(b"limits", What::Bound));
    }
    Ok(Prepared { root: domain_limits, link, payload, prefix, address })
}

#[must_use]
pub fn root_limits(worker: &settings::Worker, agent: &settings::Agent, own: &limits::Limits) -> root::Limits {
    let mut host = worker.jig.limits.hub;
    host.slots = worker.jig.slots;
    root::Limits {
        host,
        checkout: own.checkout,
        agent: agent.host_limits,
        reply_bytes: agent.limits.bodies.host_reply_text,
        bounds: agent.bounds.clone(),
        stop_bound: worker.jig.stop_bound,
        watermark: root::WATERMARK,
    }
}

/// The worker accepts repository and path bytes only up to its checkout limits.
#[must_use]
pub fn payload_limits(own: &limits::Limits) -> Option<v1::Limits> {
    let checkout = &own.checkout;
    payload::limits(&payload::Bounds {
        repositories: checkout.repositories,
        name_bytes: checkout.name_bytes,
        remote_bytes: checkout.name_bytes,
        conflicts: checkout.conflicts,
        path_bytes: checkout.path_bytes,
        token_bytes: v1::CEILINGS.token_token,
        entries: own.forges.installations,
        entry_token_bytes: own.forge_component.token_bytes,
        text_bytes: v1::CEILINGS.identity_name,
    })
}

/// Jig's link profile, with every temper part priced by the generated codec.
#[must_use]
pub fn link_limits(worker: &config::Worker, payload: &v1::Limits) -> Option<link::Limits> {
    let jig = &worker.limits;
    let core = jig.core;
    let hub = jig.hub;
    let profile = jig.link;
    let ceilings = jig_protocol_worker_wire::CEILINGS;
    let largest = payload::largest(payload)?;
    let sizes = jig_protocol_worker_wire::sizes::Sizes {
        program_sources: core.fleet.program_sources,
        piece: hub.piece_bytes,
        charter: u32::try_from(hub.charter_bytes).ok()?,
        transcript: u32::try_from(hub.transcript_bytes).ok()?,
        turn: u32::try_from(hub.turn_bytes).ok()?,
        message: core.tasks.message_bytes,
        sender: ceilings.message_sender,
        call_name: u32::try_from(core.fleet.call_name_bytes).ok()?,
        tool_name: ceilings.call_tool,
        call_input: core.call_answer_bytes,
        answer: core.call_answer_bytes,
        result: u32::try_from(hub.outcome_bytes).ok()?,
        detail: hub.detail_bytes,
        grant: grant_bytes(credential_bytes(jig.token), largest.grant)?,
        name: profile.name,
        secret: profile.secret,
        model_name: ceilings.model_bound_name,
        items: largest.items,
        left: largest.left,
        saved: largest.saved,
        evidence: u32::try_from(hub.delivery_evidence_bytes).ok()?,
        resource: largest.resource,
        slots: worker.slots,
        calls: hub.run_calls,
        turns: hub.turns,
        inbox: core.tasks.run_inbox,
        answered: core.call_records,
        models: ceilings.bounds_models,
        workstreams: core.fleet.workstreams,
        accounts: core.accounts.accounts,
        credentials: core.accounts.credentials,
        facts: profile.facts,
    };
    Some(link::Limits {
        sizes,
        chunk: profile.chunk,
        handshake: profile.handshake,
        ping: profile.ping,
        silence: profile.silence,
        stall: profile.stall,
        backoff: profile.backoff,
        ceiling: profile.ceiling,
        proved: profile.proved,
        grace: worker.graces.grace,
        busy: profile.busy,
    })
}

/// Credentials (including keys), a program source name, or a generated identity.
pub(crate) fn grant_bytes(credential: u32, identity: u32) -> Option<u32> {
    Some(credential.max(u32::try_from(smith_host_domain::SourceName::BYTES).ok()?).max(identity))
}

/// A lent credential's bound: a key's is jig's key bound, the token bound capped by the client wire's
/// (jig-protocol-config's `key_bytes`), so the token bound covers both. The wire's own ceiling also covers a
/// moved account's file, which no worker is lent (protocol/credentials.md, 3).
pub(crate) fn credential_bytes(token: u32) -> u32 {
    token
}

/// One retained declaration per recognized adapter, with each path at smith's bound.
fn programs_worst_case(programs: &[settings::Program]) -> Option<u64> {
    if programs.len() > 1 {
        return None;
    }
    let mut bytes =
        u64::try_from(programs.len()).ok()?.checked_mul(u64::try_from(size_of::<settings::Program>()).ok()?)?;
    for program in programs {
        for path in [&program.path, &program.directory] {
            if path.len() > settings::PROGRAM_PATH_BYTES || !path.starts_with(b"/") || path.contains(&0) {
                return None;
            }
            bytes = bytes.checked_add(u64::try_from(path.len()).ok()?)?;
        }
    }
    Some(bytes)
}

/// The layer's owned queues, processes, values, root and io, as the reference prices them.
#[must_use]
pub fn worker_worst_case(worker: &settings::Worker, agent: &settings::Agent, own: &limits::Limits) -> Option<u64> {
    crate::startup::memory_sum(&memory_terms(worker, agent, own)?)
}
/// Root, protocol machines, retained values and the worker's routing storage.
pub(crate) fn memory_terms(
    worker: &settings::Worker,
    agent: &settings::Agent,
    own: &limits::Limits,
) -> Option<[crate::startup::MemoryTerm; 11]> {
    let prepared = prepare(worker, agent, own).ok()?;
    let jig = &worker.jig;
    let mut command =
        u64::try_from(worker.root.len()).ok()?.checked_add(u64::try_from(jig.agent.program.len()).ok()?)?;
    for argument in &jig.agent.arguments {
        command = command.checked_add(u64::try_from(argument.len()).ok()?)?;
    }
    let remote = match prepared.address {
        io::Addr::InProcess(_) => 0,
        io::Addr::Ip(_) => crate::worker::remote_worst_case(u32::try_from(jig.name.len()).ok()?, crate::worker::NOTES)?,
    };
    let routing = Queue::<root::Event>::worst_case(crate::worker::ENGINE_EVENTS)?
        .checked_add(Queue::<link::Request>::worst_case(link::MAX_OUT_DOWN)?)?
        .checked_add(Map::<Token, host::Process>::worst_case(jig.slots)?)?
        .checked_add(Map::<Token, Token>::worst_case(jig.slots.checked_mul(5)?)?)?
        .checked_add(List::<Token>::worst_case(jig.slots)?.checked_mul(6)?)?
        .checked_add(Queue::<io::kernel::Submit>::worst_case(1)?)?
        .checked_add(command.checked_mul(u64::from(jig.slots).checked_add(1)?)?)?
        .checked_add(u64::try_from(crate::worker::WORKER_PATH_BYTES).ok()?)?;
    Some([
        ("worker", root::worst_case(&prepared.root)?),
        ("worker.remote", remote),
        ("worker_settings.programs", programs_worst_case(&agent.programs)?),
        ("program_sources", crate::program_sources::worst_case(jig.limits.hub.detail_bytes)?),
        (
            "host",
            host::process_worst_case(&agent.limits, agent.host_limits.detail_bytes)?
                .checked_mul(u64::from(jig.slots))?,
        ),
        ("worker.link", link::worst_case(&prepared.link)?),
        (
            "values",
            jig_protocol_accounts::table::worst_case(
                jig.limits.core.accounts.accounts.checked_add(1)?,
                credential_bytes(jig.limits.token),
            )?,
        ),
        (
            "identities",
            crate::identities::identities_worst_case(
                jig.limits.core.accounts.credentials,
                payload::largest(&prepared.payload)?.grant,
            )?,
        ),
        ("worker_settings", routing),
        ("io", skein_io::worst_case(&jig.limits.io)?),
        ("worker_git", crate::worker_git::worst_case(own)?),
    ])
}

/// The application's payload version is part of every worker's opening.
#[must_use]
pub fn credential(worker: &settings::Worker, secret: &[u8]) -> link::Credential {
    link::Credential {
        name: worker.jig.name.clone(),
        secret: config::secret(secret.into()),
        payload: payload::PAYLOAD_VERSION,
    }
}
