//! Checks across standalone's parts before any service is made.
use super::{Problems, What};
use crate::{limits, settings};
use alloc::boxed::Box;
use jig_domain_fleet as fleet;
use jig_protocol_config::{self as config, Path, Segment};
use skein_io::Addr;
use skein_lib::{Duration, List, Map};
use temper_engine_domain as domain;
use temper_protocol_worker_wire as payload;
use temper_worker_domain as worker_domain;

fn at(index: usize, path: Path) -> Path {
    path.child(Segment::Index(u32::try_from(index).expect("bounded configuration")))
}
fn own(name: &[u8]) -> Path {
    Path::root().member(b"application").member(b"limits").member(b"overrides").member(name)
}
fn against(problems: &mut Problems, name: &[u8], other: &'static [u8]) {
    problems.push(own(name), What::Against { other });
}
/// Named compatibility checks on the envelope runtime and store actually use.
pub fn engine_envelope(root: &domain::Limits, problems: &mut Problems) {
    if let Err(found) = config::check_effective_limits(&domain::core_limits(root)) {
        let mut path = Path::root().member(b"engine");
        for segment in &found.path.segments {
            path = path.child(segment.clone());
        }
        problems.push(path, What::Jig(found.what));
    }
    let fits = match domain::policy_row_bound(root) {
        Some(bytes) => bytes <= u64::from(root.journal.transcript_bytes),
        None => false,
    };
    if !fits {
        against(
            problems,
            b"root.journal.transcript_bytes",
            b"engine.limits.core.policy_bytes plus its policy row carrier",
        );
    }
}
fn in_process(address: &config::Address) -> bool {
    match address {
        config::Address::Addr(Addr::InProcess(name)) => name.as_bytes() == b"workers",
        config::Address::Addr(Addr::Ip(_)) | config::Address::Host { .. } => false,
    }
}
pub(super) fn loopback(address: &config::Address) -> bool {
    match address {
        config::Address::Addr(Addr::Ip(socket)) => socket.ip().is_loopback(),
        config::Address::Addr(Addr::InProcess(_)) | config::Address::Host { .. } => false,
    }
}
fn engine_alone(engine: &config::Engine, workers: &[config::Worker], problems: &mut Problems) {
    let hosts = Path::root().member(b"engine").member(b"hosts");
    if !workers.is_empty() {
        problems.push(Path::root().member(b"workers"), What::EngineAlone);
    }
    if engine.hosts.listener.len() != 1 {
        problems.push(hosts.member(b"listener"), What::EngineAlone);
    }
    for (index, address) in engine.hosts.listener.iter().enumerate() {
        if !loopback(address) {
            problems.push(at(index, hosts.member(b"listener")), What::EngineAlone);
        }
    }
    if engine.hosts.workers.is_empty() {
        problems.push(hosts.member(b"workers"), What::EngineAlone);
    }
    for (index, worker) in engine.hosts.workers.iter().enumerate() {
        for previous in engine.hosts.workers.iter().take(index) {
            if previous.digest == worker.digest {
                problems.push(at(index, hosts.member(b"workers")).member(b"digest"), What::EngineAlone);
                break;
            }
        }
    }
}
/// Standalone's requirements on jig's parts, including absolute agent paths.
pub fn standalone(
    engine: &config::Engine,
    clients: &super::application::Clients,
    workers: &[config::Worker],
    problems: &mut Problems,
) {
    let root = Path::root();
    let alone = match engine.hosts.listener.first() {
        Some(config::Address::Addr(Addr::Ip(_))) => true,
        Some(config::Address::Addr(Addr::InProcess(_)) | config::Address::Host { .. }) | None => false,
    };
    if alone {
        engine_alone(engine, workers, problems);
    } else {
        if workers.len() != 1 {
            problems.push(root.member(b"workers"), What::Standalone);
        }
        if engine.hosts.listener.len() != 1 || !in_process(engine.hosts.listener.first().expect("parsed listeners")) {
            problems.push(root.member(b"engine").member(b"hosts").member(b"listener"), What::Standalone);
        }
        let hosts_match = match (engine.hosts.workers.first(), workers.first()) {
            (Some(host), Some(worker)) => host.name == worker.name,
            (None, _) | (_, None) => false,
        };
        if engine.hosts.workers.len() != 1 || !hosts_match {
            problems.push(root.member(b"engine").member(b"hosts").member(b"workers"), What::Standalone);
        }
    }
    match clients.listener {
        config::Address::Addr(Addr::Ip(socket)) if socket.ip().is_loopback() => {}
        config::Address::Addr(Addr::Ip(_) | Addr::InProcess(_)) | config::Address::Host { .. } => {
            problems.push(root.member(b"application").member(b"clients").member(b"listener"), What::Loopback);
        }
    }
    if engine.connectors.as_ref() != [1] {
        problems.push(root.member(b"engine").member(b"connectors"), What::Connectors);
    }
    if alone {
        return;
    }
    for (index, worker) in workers.iter().enumerate() {
        let path = at(index, root.member(b"workers"));
        if !in_process(&worker.engine) {
            problems.push(path.member(b"engine"), What::Standalone);
        }
        let command = match worker.agent.arguments.first() {
            Some(value) => value.as_ref() == b"agent",
            None => false,
        };
        if command {
            let absolute = match worker.agent.arguments.get(1) {
                Some(value) => value.starts_with(b"/"),
                None => false,
            };
            if worker.agent.arguments.len() != 2 || !absolute {
                problems.push(path.member(b"agent").member(b"arguments"), What::Absolute);
            }
        } else {
            for (index, value) in worker.agent.arguments.iter().enumerate() {
                if !value.starts_with(b"/") {
                    problems.push(at(index, path.member(b"agent").member(b"arguments")), What::Absolute);
                }
            }
        }
        for (name, value) in
            [(b"program".as_slice(), &worker.agent.program), (b"configuration", &worker.agent.configuration)]
        {
            if !value.starts_with(b"/") {
                problems.push(path.member(b"agent").member(name), What::Absolute);
            }
        }
    }
}
/// The root has no hub child: the engine runs no slot, and every charter's
/// runs are placed on the workers (domain/root.md, 2).
pub fn hub(engine: &config::Engine, problems: &mut Problems) {
    let path = Path::root().member(b"engine");
    if engine.limits.core.fleet.engine_slots != 0 {
        problems.push(path.member(b"limits").member(b"overrides").member(b"fleet.engine_slots"), What::NoHub);
    }
    for (index, charter) in engine.charters.iter().enumerate() {
        match charter.hosts {
            fleet::Kinds::Workers => {}
            fleet::Kinds::Engine | fleet::Kinds::Both => {
                problems.push(at(index, path.member(b"charters")).member(b"hosts"), What::NoHub);
            }
        }
    }
}
/// The deployment's provider is not 0, which the root keeps for the forge's
/// sign-in (domain/root.md, 6).
pub fn provider(engine: &config::Engine, problems: &mut Problems) {
    for (index, provider) in engine.providers.iter().enumerate() {
        match provider.kind {
            config::ProviderKind::Deployment if provider.number == 0 => {
                problems.push(
                    at(index, Path::root().member(b"engine").member(b"providers")).member(b"number"),
                    What::Provider,
                );
            }
            config::ProviderKind::Deployment | config::ProviderKind::Link | config::ProviderKind::Application => {}
        }
    }
}
/// Each git identity's number is one both the engine's and the worker's
/// accounts tables hold.
pub fn credentials(
    engine: &config::Engine,
    workers: &[config::Worker],
    identities: &[super::Identity],
    problems: &mut Problems,
) {
    let mut bound = engine.limits.core.accounts.credentials;
    for worker in workers {
        bound = bound.min(worker.limits.core.accounts.credentials);
    }
    for (index, identity) in identities.iter().enumerate() {
        if identity.number == 0 || identity.number > bound {
            let path = at(index, Path::root().member(b"application").member(b"identities"));
            problems.push(path.member(b"number"), What::Credentials);
        }
    }
}
/// Check the optional listener and the secret paths that require it.
pub fn webhooks(members: &super::application::Members, problems: &mut Problems) {
    let path = Path::root().member(b"application");
    if let Some(webhooks) = &members.webhooks {
        match &webhooks.listener {
            config::Address::Addr(Addr::Ip(socket)) if socket.ip().is_loopback() => {
                if socket.port() != 0 && webhooks.listener == members.clients.listener {
                    problems.push(path.member(b"webhooks").member(b"listener"), What::Jig(config::What::Unique));
                }
            }
            config::Address::Addr(_) | config::Address::Host { .. } => {
                problems.push(path.member(b"webhooks").member(b"listener"), What::Loopback);
            }
        }
    } else {
        for (index, forge) in members.forges.iter().enumerate() {
            if forge.webhook_secret.is_some() {
                problems.push(at(index, path.member(b"forges")).member(b"webhook_secret"), What::NoWebhooks);
            }
        }
    }
}
/// Every git rule is checked independently, so one start reports them all.
pub fn limits(limits: &limits::Limits, git: &temper_worker_protocol_git::Limits, problems: &mut Problems) {
    use temper_worker_protocol_git::{Unfit, check};
    let mut checked = *git;
    // Repair just the reported rule in the local copy, then ask for the next.
    for _ in 0_u8..7 {
        match check(&checked, &limits.checkout) {
            Ok(()) => break,
            Err(Unfit::Operations) => {
                against(problems, b"git_component.operations", b"checkout.workspaces");
                checked.operations = limits.checkout.workspaces;
            }
            Err(Unfit::StdoutBytes) => {
                against(
                    problems,
                    b"git_component.stdout_bytes",
                    b"checkout.name_bytes and checkout.conflicts * (checkout.path_bytes + 1)",
                );
                let Some(bytes) = temper_worker_protocol_git::limits::largest_output(&limits.checkout) else { break };
                checked.stdout_bytes = bytes;
            }
            Err(Unfit::StderrBytes) => {
                against(problems, b"git_component.stderr_bytes", b"checkout.push_diagnostic");
                checked.stderr_bytes = u32::try_from(temper_worker_domain_checkout::git::PushDiagnostic::CAPACITY)
                    .expect("diagnostic bound");
            }
            Err(Unfit::ReadBytes) => {
                against(problems, b"git_component.read_bytes", b"a nonzero read");
                checked.read_bytes = 1;
            }
            Err(Unfit::SweepEntries) => {
                against(problems, b"git_component.sweep_entries", b"a nonempty sweep page");
                checked.sweep_entries = 1;
            }
            Err(Unfit::SweepBytes) => {
                against(problems, b"git_component.sweep_bytes", b"the kernel's longest name");
                checked.sweep_bytes = u32::try_from(skein_io::kernel::LONGEST_NAME).expect("kernel name bound");
            }
            Err(Unfit::Memory) => {
                problems.push(own(b"git_component.operations"), What::Jig(config::What::Bound));
                break;
            }
        }
    }
    let sign_in = limits.sign_in;
    for (name, value) in [
        (b"sign_in.members".as_slice(), sign_in.members),
        (b"sign_in.pending", sign_in.pending),
        (b"sign_in.fetches", sign_in.fetches),
        (b"sign_in.body_bytes", sign_in.body_bytes),
        (b"sign_in.url_bytes", sign_in.url_bytes),
        (b"sign_in.secret_bytes", sign_in.secret_bytes),
    ] {
        if value == 0 {
            against(problems, name, b"a nonzero sign-in bound");
        }
    }
    if sign_in.deadline == Duration::ZERO {
        against(problems, b"sign_in.deadline_ms", b"a nonzero sign-in deadline");
    }
    if sign_in.members > limits.forge.bindings {
        against(problems, b"sign_in.members", b"forge.bindings");
    }
    if sign_in.body_bytes > 65_536 {
        against(problems, b"sign_in.body_bytes", b"skein-oauth's response bound");
    }
    if limits.clients.bundle_files == 0 {
        against(problems, b"clients.bundle_files", b"a nonempty bundle");
    }
    if limits.clients.bundle_bytes == 0 {
        against(problems, b"clients.bundle_bytes", b"a nonempty bundle");
    }
}
/// The longest routed remote: base, two separators, owner, name and `.git`.
pub fn longest_remote(forges: &[settings::Forge], name_bytes: u32) -> Option<u64> {
    let mut base = 0;
    for forge in forges {
        base = base.max(u64::try_from(forge.git.len()).ok()?);
    }
    remote(base, name_bytes)
}
pub(super) fn remote(base: u64, name_bytes: u32) -> Option<u64> {
    base.checked_add(u64::from(name_bytes).checked_mul(2)?)?.checked_add(6)
}
pub(super) fn names(remote: Option<u64>, limits: &limits::Limits, problems: &mut Problems) {
    if remote.unwrap_or(u64::MAX) > u64::from(limits.checkout.name_bytes) {
        against(problems, b"checkout.name_bytes", b"the longest remote");
    }
    if limits.forge.name_bytes > limits.checkout.name_bytes {
        against(problems, b"checkout.name_bytes", b"forge.name_bytes");
    }
}
fn bounded(bounds: &payload::Bounds, engine: bool, problems: &mut Problems) -> Option<payload::v1::Limits> {
    let ceilings = payload::v1::CEILINGS;
    let repositories: &[u8] = if engine { b"forge.resources_per_task" } else { b"checkout.repositories" };
    let names: &[u8] = if engine { b"forge.name_bytes" } else { b"checkout.name_bytes" };
    for (name, bytes, bound) in [
        (
            repositories,
            bounds.repositories,
            ceilings.items_repositories.min(ceilings.left_landed).min(ceilings.saved_repositories),
        ),
        (
            names,
            bounds.name_bytes,
            ceilings
                .repository_name
                .min(ceilings.merge_branch)
                .min(ceilings.branch_branch)
                .min(ceilings.writable_push)
                .min(ceilings.writable_landing_some)
                .min(ceilings.items_save_some),
        ),
        (b"checkout.name_bytes", bounds.remote_bytes, ceilings.repository_remote),
        (b"checkout.conflicts", bounds.conflicts, ceilings.files_files),
        (b"checkout.path_bytes", bounds.path_bytes, ceilings.files_files_item),
        (b"identity.token_bytes", bounds.token_bytes, ceilings.token_token),
        (b"forges.installations", bounds.entries, ceilings.entries_entries),
        (b"forge_component.token_bytes", bounds.entry_token_bytes, ceilings.entry_token),
        (
            b"identity.text_bytes",
            bounds.text_bytes,
            ceilings
                .identity_name
                .min(ceilings.identity_login)
                .min(ceilings.identity_email)
                .min(ceilings.entry_account_login),
        ),
    ] {
        if bytes > bound {
            problems.push(own(name), What::Past { bytes: u64::from(bytes), bound: u64::from(bound) });
        }
    }
    payload::limits(bounds)
}
/// Each side's application records fit jig's fields and the worker's admission.
pub fn channel(
    engine: Option<&payload::Bounds>,
    worker: Option<&payload::Bounds>,
    forges: &[settings::Forge],
    limits: &limits::Limits,
    problems: &mut Problems,
) {
    if engine.is_some() {
        names(longest_remote(forges, limits.forge.name_bytes), limits, problems);
    }
    // The engine's terms take the smaller of the two repository bounds
    // (translate::limits), so a checkout's larger one makes the worker's
    // parts past those terms and its hello never opens the link.
    let repository_shape = match (engine, worker) {
        (Some(engine), Some(worker)) => engine.repositories == worker.repositories,
        _ => true,
    };
    let engine = match engine {
        Some(engine) => bounded(engine, true, problems),
        None => None,
    };
    let worker = match worker {
        Some(worker) => bounded(worker, false, problems),
        None => None,
    };
    let ceilings = jig_protocol_worker_wire::CEILINGS;
    let mut sizes = [None, None];
    for (index, side) in [engine, worker].iter().enumerate() {
        if let Some(side) = side {
            *sizes.get_mut(index).expect("two sides") = payload::largest(side);
            if let Some(largest) = *sizes.get(index).expect("two sides") {
                for (name, bytes, bound) in [
                    (b"workspace_items".as_slice(), largest.items, ceilings.workspace_items),
                    (b"work_left_some", largest.left, ceilings.work_left_some),
                    (b"work_saved_some", largest.saved, ceilings.work_saved_some),
                    (b"resource_resource_some", largest.resource, ceilings.resource_resource_some),
                    (b"application_value_secret", largest.grant, ceilings.application_value_secret),
                ] {
                    if bytes > bound {
                        let prefix: &[u8] = if index == 0 { b"engine" } else { b"workers" };
                        problems.push(
                            Path::root().member(prefix).member(b"limits").member(name),
                            What::Past { bytes: u64::from(bytes), bound: u64::from(bound) },
                        );
                    }
                }
            }
        }
    }
    if let [Some(engine), Some(worker)] = sizes
        && (!repository_shape || engine.items > worker.items)
    {
        against(problems, b"forge.resources_per_task", b"checkout.repositories and checkout.name_bytes");
    }
}
/// The identity's complete encoded value must fit one grant.
pub fn identities(identities: &[settings::Identity], limits: &payload::v1::Limits, problems: &mut Problems) {
    for (index, identity) in identities.iter().enumerate() {
        let path = at(index, Path::root().member(b"application").member(b"identities"));
        match crate::translate::identity(identity, limits) {
            Ok(value) => {
                let bytes = u64::try_from(value.expose().len()).expect("bounded identity");
                let bound = u64::from(jig_protocol_worker_wire::CEILINGS.application_value_secret);
                if bytes > bound {
                    problems.push(path, What::Past { bytes, bound });
                }
            }
            Err(_) => problems.push(path, What::Jig(config::What::Bound)),
        }
    }
}
/// Each sender's credential fits the receiving table and smith's bodies.
pub fn grants(engine: &config::Engine, worker: &config::Limits, agent: &settings::Agent, problems: &mut Problems) {
    let credential = crate::worker_settings::credential_bytes(worker.token);
    if crate::values::credential_bytes(engine) > credential {
        problems.push(worker_limit(b"accounts.token"), What::Against { other: b"engine.limits accounts.token" });
    }
    let bodies = &agent.limits.bodies;
    if u32::try_from(smith_host_domain::SourceName::BYTES).expect("bounded source name") > bodies.program_value_name {
        problems.push(
            at(0, Path::root().member(b"workers")).member(b"agent").member(b"configuration"),
            What::Against { other: b"the agent's program_value_name" },
        );
    }
    if credential > bodies.credential_value_bytes {
        problems.push(worker_limit(b"accounts.token"), What::Against { other: b"the agent's credential_value_bytes" });
    }
}
fn worker_limit(member: &[u8]) -> Path {
    at(0, Path::root().member(b"workers")).member(b"limits").member(b"overrides").member(member)
}
/// The resume window as jig's hub counts it, excluding answered calls
/// (jig's `domain/hosts.md`; hosted.rs, `conversation_fits`).
fn resume_window(bytes: u32) -> Option<u64> {
    let bytes = u64::from(bytes);
    let carrier = u64::try_from(size_of::<Box<[u8]>>()).ok()?;
    bytes.checked_add(carrier.checked_sub(4)?.checked_mul(bytes / 4)?)
}
fn short(problems: &mut Problems, path: Path, bytes: u64, of: &'static [u8], needs: Option<u64>) {
    match needs {
        Some(needs) if bytes < needs => problems.push(path, What::Short { bytes, of, needs }),
        Some(_) => {}
        None => problems.push(path, What::Jig(config::What::Bound)),
    }
}
/// Each process checks the turn bounds its own document can know
/// (protocol/configuration.md, 7). A worker alone knows only its turn cap.
pub fn turns(
    engine: Option<&config::Limits>,
    workers: &[config::Worker],
    own_limits: &limits::Limits,
    agent: Option<&settings::Agent>,
    problems: &mut Problems,
) {
    let engine_path = Path::root().member(b"engine").member(b"limits").member(b"overrides");
    if let Some(agent) = agent {
        let turn = u64::from(agent.limits.bodies.turn_body);
        if let Some(engine) = engine {
            short(
                problems,
                engine_path.member(b"hub.turn_bytes"),
                engine.hub.turn_bytes,
                b"the agent's largest turn",
                Some(turn),
            );
            short(
                problems,
                engine_path.member(b"core.resume_bytes"),
                u64::from(engine.core.resume_bytes),
                b"the agent's largest turn with its length",
                turn.checked_add(4),
            );
            let loads = &own_limits.root.loads;
            short(
                problems,
                own(b"root.loads.transcript_bytes"),
                u64::from(loads.transcript_bytes),
                b"the agent's largest turn",
                Some(turn),
            );
            short(
                problems,
                own(b"root.loads.bytes"),
                u64::from(loads.bytes),
                b"the agent's largest turn with a loaded row",
                turn.checked_add(u64::try_from(size_of::<domain::Record>()).expect("record slot")),
            );
            // jig's reads::vocabulary::row_bytes charges size_of::<Row>() and
            // record_bytes charges size_of::<Record>() even for a Turn. Both
            // types are public; owner, ancestor and linked evidence is extra.
            let row = u64::try_from(size_of::<jig_domain_core::reads::Row>()).expect("read row slot");
            let record = u64::try_from(size_of::<jig_domain_core::Record>()).expect("read record carrier");
            short(
                problems,
                own(b"root.loads.read_bytes"),
                loads.read_bytes,
                b"the agent's largest turn with a read's row",
                turn.checked_add(row.checked_add(record).expect("read row carriers")),
            );
        }
        for (index, worker) in workers.iter().enumerate() {
            let path = at(index, Path::root().member(b"workers")).member(b"limits").member(b"overrides");
            short(
                problems,
                path.member(b"hub.turn_bytes"),
                worker.limits.hub.turn_bytes,
                b"the agent's largest turn",
                Some(turn),
            );
        }
    }
    if let Some(engine) = engine {
        let window = resume_window(engine.core.resume_bytes);
        short(
            problems,
            engine_path.member(b"hub.transcript_bytes"),
            engine.hub.transcript_bytes,
            b"the engine's resume window",
            window,
        );
        for (index, worker) in workers.iter().enumerate() {
            let path = at(index, Path::root().member(b"workers")).member(b"limits").member(b"overrides");
            short(
                problems,
                path.member(b"hub.transcript_bytes"),
                worker.limits.hub.transcript_bytes,
                b"the engine's resume window",
                window,
            );
        }
    }
}
/// The worker's rules against the agent's configuration, each naming both
/// limits (temper-worker-domain's `worst_case`).
pub fn worker(limits: &worker_domain::Limits, problems: &mut Problems) {
    use worker_domain::Unfit;
    let worker = at(0, Path::root().member(b"workers"));
    for unfit in &worker_domain::unfit(limits) {
        let (path, other): (Path, &'static [u8]) = match unfit {
            Unfit::Agent | Unfit::Unacknowledged => {
                problems.push(worker.member(b"agent").member(b"configuration"), What::Jig(config::What::Bound));
                continue;
            }
            Unfit::Directories => (own(b"checkout.repositories"), b"the agent's directories"),
            Unfit::Conflicts => (own(b"checkout.conflicts"), b"the agent's conflicts"),
            Unfit::Paths => (own(b"checkout.path_bytes"), b"the agent's path_bytes"),
            Unfit::Names => (own(b"checkout.name_bytes"), b"the agent's name_bytes"),
            Unfit::Key => (own(b"checkout.name_bytes"), b"a workstream's 8-byte key"),
            Unfit::Accounts => (worker_limit(b"hub.accounts"), b"the agent's accounts"),
            Unfit::Workspaces => (worker.member(b"slots"), b"checkout.workspaces"),
            Unfit::Agents => (worker.member(b"slots"), b"the agent's agents"),
            Unfit::Turns => (worker_limit(b"hub.turns"), b"the agent's turns"),
            Unfit::TurnQueue => (worker_limit(b"hub.turn_queue_bytes"), b"the agent's turns * hub.turn_bytes"),
            Unfit::Calls => (worker_limit(b"hub.run_calls"), b"the agent's calls"),
            Unfit::Events => {
                (worker_limit(b"hub.event_bytes"), b"the agent's message_bytes less smith's 2-byte separator")
            }
            Unfit::Held => (worker_limit(b"hub.held"), b"the agent's messages"),
            Unfit::Inbox => (worker_limit(b"hub.inbox"), b"the agent's messages"),
            Unfit::CallBytes => (own(b"checkout.message_bytes"), b"the agent's call_bytes"),
            Unfit::Saved => (own(b"checkout.message_bytes"), b"the save's commit message"),
            Unfit::BoundsInbox => (worker_limit(b"hub.inbox"), b"the agent's hello's inbox"),
            Unfit::BoundsMessage => (worker_limit(b"hub.event_bytes"), b"the agent's hello's message_bytes"),
        };
        problems.push(path, What::Against { other });
    }
}
/// The landing rules' limits, one of which a refused rule breaks.
const LANDING: &[u8] =
    b"root.journal.transcript_bytes, authority.requirements, forge.judge_criteria and forge.change_policy.gates";
/// The index of `number` in a list of numbered entries, which a check found there.
fn index_of(numbers: &[u32], number: u32) -> usize {
    let mut index = 0;
    for (at, listed) in numbers.iter().enumerate() {
        if *listed == number {
            index = at;
        }
    }
    index
}
/// What the root's construction asserts, refused before it is made, each
/// naming the configuration that breaks it (domain/root.md, 6).
#[expect(clippy::manual_map, reason = "the strict subset uses explicit matches")]
pub fn root(
    core: jig_domain_core::Config,
    engine: &settings::Engine,
    seeds: &settings::Seeds,
    landing_projects: &[u32],
    limits: &domain::Limits,
    problems: &mut Problems,
) {
    let mut projects = Map::with_capacity(engine.landing.projects.capacity());
    for (&project, rules) in &engine.landing.projects {
        projects.insert(project, rules.clone()).expect("the same capacity");
    }
    let landing = domain::LandingPolicy { deployment: engine.landing.deployment.clone(), projects };
    let (forge, identities) = crate::forge::config(engine, seeds.deployment);
    let config = domain::Config {
        demo: match &engine.demo {
            Some(demo) => Some(demo.domain(&engine.forges, engine.limits.setup.deadline)),
            None => None,
        },
        core,
        landing,
        forge,
        identities,
        forge_charter: engine.charter,
        forges: super::application::configured(engine),
        under_box: false,
    };
    let mut numbers = List::with_capacity(engine.jig.limits.core.authority.projects);
    for project in &engine.jig.projects {
        numbers.push(project.number).expect("bounded projects");
    }
    let numbers = numbers.into_boxed();
    let engine_path = Path::root().member(b"engine");
    let landing_path = Path::root().member(b"application").member(b"landing");
    for refused in domain::refused(config, limits) {
        let (path, what) = match refused {
            domain::Refused::Workers(problem) => {
                let (path, what) = membership(problem);
                (path, What::Jig(what))
            }
            domain::Refused::Run { charter, bytes } => (
                at(charter, engine_path.member(b"charters")),
                match bytes {
                    Some(bytes) => What::Within {
                        bytes,
                        limit: b"root.journal.run_bytes",
                        bound: u64::from(limits.journal.run_bytes),
                    },
                    None => What::Jig(config::What::Bound),
                },
            ),
            domain::Refused::Resume => (
                engine_path.member(b"limits").member(b"overrides").member(b"core.resume_bytes"),
                What::Against { other: b"root.journal.transcript_bytes, above 0" },
            ),
            domain::Refused::JudgeProjects => {
                (own(b"forge.judge_projects"), What::Against { other: b"authority.projects in engine.limits" })
            }
            domain::Refused::JudgeCriteria => {
                (own(b"forge.judge_criteria"), What::Against { other: b"authority.requirements in engine.limits" })
            }
            domain::Refused::Escalation { project } => (
                at(index_of(&numbers, project), engine_path.member(b"projects"))
                    .member(b"policy")
                    .member(b"escalation_role"),
                What::Escalation,
            ),
            domain::Refused::Permissions { project, bytes } => (
                at(index_of(&numbers, project), engine_path.member(b"projects")).member(b"roles"),
                match bytes {
                    Some(bytes) => What::Within {
                        bytes,
                        limit: b"root.journal.transcript_bytes",
                        bound: u64::from(limits.journal.transcript_bytes),
                    },
                    None => What::Jig(config::What::Bound),
                },
            ),
            domain::Refused::Policy { project, bytes } => (
                at(index_of(&numbers, project), engine_path.member(b"projects")).member(b"policy").member(b"bytes"),
                match bytes {
                    Some(bytes) => What::Within { bytes, limit: b"core.policy_bytes", bound: limits.policy_bytes },
                    None => What::Jig(config::What::Bound),
                },
            ),
            domain::Refused::Landing { project: None } => {
                (landing_path.member(b"deployment"), What::Against { other: LANDING })
            }
            domain::Refused::Landing { project: Some(project) } => (
                at(index_of(landing_projects, project), landing_path.member(b"projects")).member(b"rules"),
                What::Against { other: LANDING },
            ),
            domain::Refused::Forges => (
                Path::root().member(b"application").member(b"forges"),
                What::Against { other: b"forge.repositories and forge.client.op_bytes" },
            ),
        };
        problems.push(path, what);
    }
}
/// Endpoint names, and every charter model, agree with the agent.
pub fn agent(engine: &config::Engine, agent: &settings::Agent, problems: &mut Problems) {
    for (index, endpoint) in engine.llm.endpoints.iter().enumerate() {
        let agrees = agent.endpoints.resolve(&endpoint.name).is_some();
        if !agrees {
            problems.push(at(index, Path::root().member(b"engine").member(b"llm").member(b"endpoints")), What::Agent);
        }
    }
    for (index, provider) in engine.llm.providers.iter().enumerate() {
        for (offered, name) in provider.programs.iter().enumerate() {
            let mut declared = false;
            for program in &agent.programs {
                declared |= program.adapter.name() == name.as_ref();
            }
            if !declared {
                problems.push(
                    at(
                        offered,
                        at(index, Path::root().member(b"engine").member(b"llm").member(b"providers"))
                            .member(b"sources")
                            .member(b"programs"),
                    ),
                    What::Adapter,
                );
            }
        }
    }
    for (index, charter) in engine.charters.iter().enumerate() {
        let path = at(index, Path::root().member(b"engine").member(b"charters"));
        model(&charter.policy.model, &agent.bounds, path.member(b"model"), problems);
        for (index, alternative) in charter.policy.alternatives.iter().enumerate() {
            model(alternative, &agent.bounds, at(index, path.member(b"alternatives")), problems);
        }
    }
}
fn model(model: &jig_domain_core::Model, bounds: &jig_domain_host::Bounds, path: Path, problems: &mut Problems) {
    let mut found = false;
    for bound in &bounds.models {
        found |= bound.name == model.name && bound.window >= model.window && bound.output >= model.output;
    }
    if !found {
        problems.push(path, What::Agent);
    }
}
/// Report the fleet's selected admission failure; amounts only project that decision.
pub fn admission(
    core: &jig_domain_core::Config,
    engine: &config::Engine,
    agent: &settings::Agent,
    problems: &mut Problems,
) {
    use fleet::AgentShortfall;
    let needs = jig_domain_core::fleet_needs::configured(core, &engine.limits.core);
    let bounds = domain::hello_bounds(agent.bounds.clone());
    let Some(why) = fleet::shortfall(&needs, &bounds, fleet::HostKind::Worker) else { return };
    let engine_path = Path::root().member(b"engine");
    let (path, offered, required) = match why {
        AgentShortfall::Model { charter, model }
        | AgentShortfall::Window { charter, model }
        | AgentShortfall::Output { charter, model } => {
            let index = charter_index(engine, charter);
            let need = needs
                .charters
                .get(index)
                .expect("selected charter")
                .models
                .get(usize::try_from(model).expect("configured model index"))
                .expect("selected model");
            let path = model_path(index, model);
            let mut offered = None;
            for bound in &bounds.models {
                if bound.name == need.name {
                    offered = Some(bound);
                    break;
                }
            }
            let Some(bound) = offered else {
                problems.push(path, What::Agent);
                return;
            };
            match why {
                AgentShortfall::Model { .. } => {
                    problems.push(path, What::Agent);
                    return;
                }
                AgentShortfall::Window { .. } => (path, u64::from(bound.window), u64::from(need.window)),
                AgentShortfall::Output { .. } => (path, u64::from(bound.output), u64::from(need.output)),
                AgentShortfall::Turns { .. }
                | AgentShortfall::Time { .. }
                | AgentShortfall::Waiting { .. }
                | AgentShortfall::Checks { .. }
                | AgentShortfall::Spend
                | AgentShortfall::Message
                | AgentShortfall::Inbox
                | AgentShortfall::Unacknowledged => unreachable!("selected model dimension"),
            }
        }
        AgentShortfall::Turns { charter }
        | AgentShortfall::Time { charter }
        | AgentShortfall::Waiting { charter }
        | AgentShortfall::Checks { charter } => {
            let index = charter_index(engine, charter);
            let need = needs.charters.get(index).expect("selected charter");
            let path = at(index, engine_path.member(b"charters"));
            match why {
                AgentShortfall::Turns { .. } => (path.member(b"turns"), u64::from(bounds.turns), u64::from(need.turns)),
                AgentShortfall::Time { .. } => (path.member(b"time_ms"), millis(bounds.time), millis(need.time)),
                AgentShortfall::Waiting { .. } => {
                    (path.member(b"waiting_ms"), millis(bounds.waiting), millis(need.waiting))
                }
                AgentShortfall::Checks { .. } => (
                    path.member(b"reserve_time_ms"),
                    millis(bounds.checks),
                    millis(need.change_reserve.expect("selected change reserve")),
                ),
                AgentShortfall::Model { .. }
                | AgentShortfall::Window { .. }
                | AgentShortfall::Output { .. }
                | AgentShortfall::Spend
                | AgentShortfall::Message
                | AgentShortfall::Inbox
                | AgentShortfall::Unacknowledged => unreachable!("selected charter dimension"),
            }
        }
        AgentShortfall::Spend => (engine_path.member(b"rules").member(b"maximum_run_spend"), bounds.spend, needs.spend),
        AgentShortfall::Message => {
            (engine_path.member(b"limits"), u64::from(bounds.message_bytes), u64::from(needs.message_bytes))
        }
        AgentShortfall::Inbox => {
            (engine_path.member(b"limits"), u64::from(bounds.inbox), u64::from(needs.start_messages))
        }
        AgentShortfall::Unacknowledged => {
            (engine_path.member(b"limits"), u64::from(bounds.unacknowledged), u64::from(needs.unacknowledged))
        }
    };
    problems.push(path, What::Admission { why, offered, required });
}
fn model_path(index: usize, model: u32) -> Path {
    let path = at(index, Path::root().member(b"engine").member(b"charters"));
    match model.checked_sub(1) {
        None => path.member(b"model"),
        Some(alternative) => {
            at(usize::try_from(alternative).expect("configured alternative index"), path.member(b"alternatives"))
        }
    }
}
fn charter_index(engine: &config::Engine, number: u32) -> usize {
    for (index, charter) in engine.charters.iter().enumerate() {
        if charter.number == number {
            return index;
        }
    }
    unreachable!("canonical shortfall names a configured charter")
}
fn millis(duration: Duration) -> u64 {
    duration.as_nanos() / 1_000_000
}
/// One io holds engine links, the worker's link, and four entities per child.
pub(super) fn sockets(
    engine: &config::Engine,
    worker: &config::Worker,
    git: &temper_worker_protocol_git::Limits,
    members: &super::application::Members,
    problems: &mut Problems,
) {
    match (socket_count(engine, worker, git, members), process_io(&engine.limits.io, Some(&worker.limits.io))) {
        (Some(bytes), Some(io)) if bytes <= u64::from(io.sockets) => {}
        (Some(bytes), Some(io)) => problems.push(
            Path::root().member(b"engine").member(b"limits").member(b"io").member(b"sockets"),
            What::Past { bytes, bound: u64::from(io.sockets) },
        ),
        (None, _) | (_, None) => problems.push(
            Path::root().member(b"engine").member(b"limits").member(b"io").member(b"sockets"),
            What::Jig(config::What::Bound),
        ),
    }
}
fn socket_count(
    engine: &config::Engine,
    worker: &config::Worker,
    git: &temper_worker_protocol_git::Limits,
    members: &super::application::Members,
) -> Option<u64> {
    let mut required = u64::from(limits::server(&engine.limits).connections).checked_add(1)?.checked_add(2)?;
    if members.webhooks.is_some() {
        required = required.checked_add(1)?.checked_add(u64::from(members.limits.forge_component.hooks))?;
    }
    if members.sign_in.is_some() {
        required = required.checked_add(u64::from(members.limits.sign_in.fetches))?;
    }
    let mut sign_in = false;
    for provider in &engine.llm.providers {
        sign_in |= provider.sign_in.is_some();
    }
    if sign_in {
        required = required
            .checked_add(u64::from(engine.limits.driver.accounts))?
            .checked_add(u64::from(engine.limits.driver.exchanges))?
            .checked_add(u64::from(engine.limits.driver.listeners))?;
    }
    required
        .checked_add(1)?
        .checked_add(u64::from(worker.slots).checked_mul(4)?)?
        .checked_add(u64::from(git.operations).checked_mul(4)?)
}
/// Compose engine and worker io exactly as jig's reference startup does.
pub(crate) fn process_io(engine: &skein_io::Limits, worker: Option<&skein_io::Limits>) -> Option<skein_io::Limits> {
    let mut io = *engine;
    if let Some(worker) = worker {
        io.names = io.names.checked_add(worker.names)?;
        io.intake = io.intake.max(worker.intake);
        io.receive = io.receive.max(worker.receive);
        io.output = io.output.max(worker.output);
        io.sends = io.sends.max(worker.sends);
    }
    io.sockets = limits::SOCKETS;
    Some(io)
}
/// Each service's sum is checked against its own memory admission.
pub fn memory(engine: u64, engine_memory: u64, worker: u64, worker_memory: u64, problems: &mut Problems) {
    for (path, bytes, bound) in [
        (Path::root().member(b"engine"), engine, engine_memory),
        (at(0, Path::root().member(b"workers")), worker, worker_memory),
    ] {
        if let Err(what) = config::fits(bound, &[config::Share { name: b"service", bytes }]) {
            problems.push(path.member(b"memory_bytes"), What::Jig(what));
        }
    }
}

/// Compare port numbers, also across IPv4/IPv6. Absent or ephemeral listeners reserve none.
pub fn redirect_ports(
    redirects: &[super::RedirectPort],
    port: Option<u16>,
    listener: super::Listener,
    problems: &mut Problems,
) {
    for redirect in redirects {
        if let Some(port) = port
            && port != 0
            && port == redirect.port
        {
            problems.push(redirect.path.clone(), What::Redirect(listener));
        }
    }
}
pub(super) fn port(address: &config::Address) -> Option<u16> {
    match address {
        config::Address::Addr(address) => Some(address.socket()?.port()),
        config::Address::Host { port, .. } => Some(*port),
    }
}

/// Check standalone's raw agent path before jig joins a configuration path.
pub(super) fn raw_worker(document: &skein_json::Document, at: u32, path: &Path, problems: &mut Problems) {
    if let Ok(mut worker) = config::read::Object::new(document, at)
        && let Some(agent_at) = worker.member(document, b"agent")
        && let Ok(mut agent) = config::read::Object::new(document, agent_at)
        && let Some(at) = agent.member(document, b"configuration")
        && let Ok(value) = config::read::text(document, at, 4096)
        && !value.starts_with(b"/")
    {
        problems.push(path.member(b"agent").member(b"configuration"), What::Absolute);
    }
}

/// Encoded active writes; `AppId`/`User` are 9 bytes, dominated by admitted `AppKey`.
pub(crate) fn record_writes(
    limits: &limits::Limits,
    forges: &[super::application::Forge],
) -> List<(Option<u64>, &'static [u8], Option<usize>)> {
    use temper_protocol_records as records;
    let capacity =
        u32::try_from(forges.len()).expect("bounded configured forges").checked_add(3).expect("three scalars");
    let mut writes = List::with_capacity(capacity);
    let accepted = match u64::from(limits.forges.installations).checked_mul(8) {
        Some(bytes) => bytes.checked_add(1),
        None => None,
    };
    for (bytes, source) in [
        (u64::from(limits.forge_component.key_bytes).checked_add(1), b"AppKey (forge_component.key_bytes)".as_slice()),
        (accepted, b"Accepted (forges.installations)"),
        (u64::from(limits.forge.name_bytes).checked_add(1), b"AppSlug (forge.name_bytes)"),
    ] {
        writes.push((bytes, source, None)).expect("three scalar writes");
    }
    for (index, forge) in forges.iter().enumerate() {
        if forge.kind != settings::Kind::GitHub {
            continue;
        }
        // Already refused by application::forge_endpoint; never encode its socket.
        if let config::Address::Addr(Addr::InProcess(_)) = &forge.api {
            continue;
        }
        let origin = records::origin(skein_forge_connection::settings::Kind::GitHub, &forge.api).encode();
        writes
            .push((u64::try_from(records::encode(&origin).len()).ok(), b"Origin", Some(index)))
            .expect("configured origins");
    }
    writes
}
/// Application records fit the store and the largest encoded active write.
pub fn records(
    jig: &config::Engine,
    limits: &limits::Limits,
    forges: &[super::application::Forge],
    problems: &mut Problems,
) {
    let component = &limits.forge_component;
    for (name, positive, other) in [
        (b"forges.log_origins".as_slice(), limits.forges.log_origins > 0, b"forges.log_origins (above 0)".as_slice()),
        (b"forge_component.log_origins", component.log_origins > 0, b"forge_component.log_origins (above 0)"),
        (b"forge_component.log_poll_ms", component.log_poll > Duration::ZERO, b"forge_component.log_poll_ms (above 0)"),
        (
            b"forge_component.log_catch_up_ms",
            component.log_catch_up > Duration::ZERO,
            b"forge_component.log_catch_up_ms (above 0)",
        ),
        (b"forge_component.log_pages", component.log_pages > 0, b"forge_component.log_pages (above 0)"),
        (b"forge_component.delivery_bytes", component.delivery_bytes > 0, b"forge_component.delivery_bytes (above 0)"),
    ] {
        if !positive {
            against(problems, name, other);
        }
    }
    if limits.forges.log_origins > 4 {
        against(problems, b"forges.log_origins", b"forges.log_origins (at most 4)");
    }
    if limits.forges.log_origins > component.log_origins {
        against(problems, b"forges.log_origins", b"forge_component.log_origins");
    }
    if component.log_catch_up < component.log_poll {
        against(problems, b"forge_component.log_catch_up_ms", b"forge_component.log_poll_ms");
    }
    if component.log_catch_up > Duration::from_secs(86_400) {
        against(
            problems,
            b"forge_component.log_catch_up_ms",
            b"forge_component.log_catch_up_ms (at most 86400000 ms / 24 h)",
        );
    }
    if limits.picker.page == 0 || limits.picker.page > 100 {
        against(problems, b"picker.page", b"picker.page (1 to 100)");
    }
    if limits.routes.query_bytes == 0 {
        against(problems, b"routes.query_bytes", b"positive query bound");
    }
    let value = limits.records.value_bytes;
    if value == 0 || value > secret_value_bound(jig, limits).unwrap_or(0) {
        against(problems, b"records.value_bytes", b"engine.limits.store.value");
    }
    let mut largest = (0, b"AppKey (forge_component.key_bytes)".as_slice(), None);
    for (bytes, source, index) in record_writes(limits, forges).into_boxed() {
        if bytes.unwrap_or(u64::MAX) > largest.0 {
            largest = (bytes.unwrap_or(u64::MAX), source, index);
        }
    }
    if u64::from(value) < largest.0 {
        against(problems, b"records.value_bytes", largest.1);
        if let Some(index) = largest.2 {
            problems.push(
                at(index, Path::root().member(b"application").member(b"forges")).member(b"api"),
                What::Against { other: b"Origin (records.value_bytes)" },
            );
        }
    }
    // Every observed installation must fit the deployment's client document (client/README.md, 4.6).
    let bound = temper_client_protocol_wire::INSTALLATIONS;
    if limits.forge_component.installations > bound {
        problems.push(
            own(b"forge_component.installations"),
            What::Past { bytes: u64::from(limits.forge_component.installations), bound: u64::from(bound) },
        );
    }
    if limits.forges.installations == 0 || limits.forges.installations > limits.forge_component.installations {
        against(problems, b"forges.installations", b"forge_component.installations");
    }
}

pub(crate) fn secret_value_bound(jig: &config::Engine, own: &limits::Limits) -> Option<u32> {
    let root = limits::engine(&jig.limits, own)?;
    Some(crate::store::limits(&root, &root.forge, &jig.limits.store)?.secrets.value)
}

/// Translate the kit's configured membership refusal into its startup path.
pub(crate) fn membership(problem: fleet::WorkerProblem) -> (Path, config::What) {
    let hosts = Path::root().member(b"engine").member(b"hosts").member(b"workers");
    match problem {
        fleet::WorkerProblem::Workers { .. } => (hosts, config::What::Bound),
        fleet::WorkerProblem::WorkerName { worker, .. } => {
            (hosts.child(Segment::Index(worker)).member(b"name"), config::What::Bound)
        }
    }
}
