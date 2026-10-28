//! Deployment-funded landing tasks and their authorized steps (domain/landing.md, 4.2, 11).
use super::{
    Box, Domain, EffectDescription, EffectForm, Env, Family, LANDING_CODE, Limits, List, ProcedureStep, Recovery,
    StepEnd, Work, append, authority, effect_access, forge, forge_change, landing, resource_name, tasks,
};

fn changes(domain: &Domain, env: &Env<Limits>, task: u64) -> Box<[u64]> {
    let Some(change) = domain.forge.change(task) else { return Box::new([task]) };
    let Some(repository) = domain.forge.repository(change.repository) else { return Box::new([task]) };
    let mut graph = List::with_capacity(env.limits.forge.changes);
    for repository in domain.forge.writable_repositories(repository.project) {
        for change in domain.forge.changes_for(repository.provider, env.limits.forge.changes) {
            if let Some(row) = domain.core.tasks.task(change) {
                graph
                    .push(forge::ChangeDependencies { change, pending: row.waiting_on.clone() })
                    .expect("bounded changes");
            }
        }
    }
    domain.forge.joined_changes(repository.project, task, graph.as_slice())
}

/// Joined dependencies and dependencies between live cross-repository landings
/// permit readiness; the latter keep only the dependent landing's turn.
/// Jig still orders the change tasks' successful results (domain/landing.md, 4.1).
pub(super) fn kept(domain: &Domain, env: &Env<Limits>, task: u64) -> bool {
    let Some(row) = domain.core.tasks.task(task) else { return false };
    if row.waiting_on.is_empty() {
        return false;
    }
    let joined = changes(domain, env, task);
    let fits = joined.len() <= usize::try_from(env.limits.forge.landing_members).expect("member bound");
    for dependency in &row.waiting_on {
        if (fits && joined.contains(dependency)) || landing_dependency(domain, task, *dependency) {
            continue;
        }
        return true;
    }
    false
}

fn landing_dependency(domain: &Domain, task: u64, dependency: u64) -> bool {
    let Some(change) = domain.forge.change(task) else { return false };
    let Some(upstream) = domain.forge.change(dependency) else { return false };
    if change.repository == upstream.repository {
        return false;
    }
    let Some(landing) = change.change.landing else { return false };
    let Some(other) = upstream.change.landing else { return false };
    if landing == other {
        return true;
    }
    let Some(row) = domain.forge.landing(landing) else { return false };
    let Some(other) = domain.forge.landing(other) else { return false };
    if row.landing.project != other.landing.project {
        return false;
    }
    match other.landing.phase {
        landing::Phase::Landed | landing::Phase::Cancelled => false,
        landing::Phase::Waiting
        | landing::Phase::Preparing
        | landing::Phase::Proving
        | landing::Phase::Proved { .. }
        | landing::Phase::Turn { .. }
        | landing::Phase::Held { .. } => true,
    }
}

pub(super) fn landed_step_due(domain: &Domain, task: u64) -> bool {
    let Some(change) = domain.forge.change(task) else { return false };
    if let forge_change::State::Ready { .. } = change.change.state
        && let Some(task) = change.change.landing
        && let Some(row) = domain.forge.landing(task)
    {
        for member in &row.landing.members {
            if member.change == change.task {
                return member.landed.is_some();
            }
        }
    }
    false
}

fn member(domain: &Domain, env: &Env<Limits>, task: u64) -> Option<landing::Member> {
    let row = domain.forge.change(task)?;
    let project = domain.forge.repository(row.repository)?.project;
    let mut gates = List::with_capacity(env.limits.forge.gates);
    if let Some(config) = domain.config.landings.get(&project) {
        for repository in &config.repositories {
            if repository.repository == row.repository {
                for _ in &repository.gates {
                    gates
                        .push(landing::GateProof {
                            proof: landing::Proof::Unkeyed,
                            posted: landing::Posted::Nothing,
                            faults: 0,
                        })
                        .expect("configured gates");
                }
            }
        }
    }
    Some(landing::Member {
        change: task,
        repository: landing::Repository { forge: row.repository.forge, repository: row.repository.repository },
        // A producing member has no pull yet; the top fills it before reading.
        pull: row.pull.unwrap_or(0),
        head: None,
        base: None,
        candidate: None,
        gates: gates.into_boxed(),
        asked: None,
        landed: None,
    })
}

/// A creation or amendment joins its closure before its next change step.
pub(super) fn join_landing(domain: &mut Domain, env: &Env<Limits>, task: u64) {
    let joined = changes(domain, env, task);
    let mut targets = List::with_capacity(env.limits.forge.landings);
    for change in &joined {
        if let Some(row) = domain.forge.change(*change)
            && let Some(landing) = row.change.landing
            && !targets.as_slice().contains(&landing)
        {
            targets.push(landing).expect("bounded landing closure");
        }
    }
    let Some(target) = targets.as_slice().first().copied() else { return };
    if targets.len() > 1 {
        for landing in &targets {
            domain.work.push(Work::Tasks(tasks::Event::WakeProcedure { task: *landing }));
        }
    }
    let mut after = List::with_capacity(env.limits.forge.landing_members);
    for change in &joined {
        let unjoined = match domain.forge.change(*change) {
            Some(row) => row.change.landing.is_none(),
            None => false,
        };
        if unjoined && let Some(member) = member(domain, env, *change) {
            domain.work.push(Work::Forge(forge::Event::JoinLanding {
                task: target,
                member,
                after: Box::from(after.as_slice()),
            }));
        }
        if after.push(*change).is_err() {
            return;
        }
    }
    domain.work.push(Work::Forge(forge::Event::OrderLanding { task: target, changes: joined }));
}

pub(super) fn start_landing(domain: &mut Domain, env: &Env<Limits>, task: u64) {
    let Some(change) = domain.forge.change(task) else { return };
    if change.change.way != forge_change::Way::Turn || kept(domain, env, task) {
        return;
    }
    let Some(repository) = domain.forge.repository(change.repository) else { return };
    let project = repository.project;
    let joined = changes(domain, env, task);
    for task in &joined {
        let joined = match domain.forge.change(*task) {
            Some(row) => row.change.landing.is_some(),
            None => false,
        };
        if joined {
            join_landing(domain, env, *task);
            return;
        }
    }
    if domain.forge.landing_admits(project, &joined).is_err() {
        return;
    }
    let mut words = List::with_capacity(env.limits.tasks.spec_bytes.saturating_sub(32));
    append(&mut words, b"Land ");
    for byte in &change.title {
        if *byte == b'\n' {
            break;
        }
        if words.push(*byte).is_err() {
            break;
        }
    }
    let mut members = List::with_capacity(env.limits.forge.landing_members);
    let mut asks = List::with_capacity(env.limits.forge.landing_members);
    for task in &joined {
        let Some(member) = member(domain, env, *task) else { return };
        members.push(member).expect("admitted closure");
        asks.push(None).expect("admitted closure");
    }
    let Some(seed) = domain.core.repair_seed(project) else { return };
    let number = seed.number();
    domain.work.push(Work::Forge(forge::Event::Landing {
        row: forge::LandingRow {
            landing: landing::Landing {
                task: number,
                project,
                phase: landing::Phase::Waiting,
                preparation: 0,
                stale_proofs: 0,
                members: members.into_boxed(),
            },
            asks: asks.into_boxed(),
            entries: Box::new([]),
        },
    }));
    domain.work.push(Work::Core(jig_domain_core::Event::ConnectorRepair {
        seed: Box::new(seed),
        executor: tasks::Executor::Procedure { connector: domain.config.forge_connector, code: LANDING_CODE },
        spec: tasks::Spec {
            words: words.into_boxed(),
            parameters: Box::new([
                tasks::Parameter::Number { name: 1, value: u64::from(project) },
                tasks::Parameter::Number { name: 2, value: task },
            ]),
            inputs: Box::new([]),
        },
        contract: tasks::Contract::Report { words: env.limits.tasks.result_bytes },
        holdings: Box::new([]),
    }));
}

// Dependencies joining two existing landings order their turns, not their proofs.
fn kept_landings(domain: &Domain, env: &Env<Limits>, project: u32) -> Box<[u64]> {
    let mut kept = List::with_capacity(env.limits.forge.landings);
    for repository in domain.forge.writable_repositories(project) {
        for task in domain.forge.changes_for(repository.provider, env.limits.forge.changes) {
            let Some(change) = domain.forge.change(task) else { continue };
            let Some(landing) = change.change.landing else { continue };
            let Some(row) = domain.core.tasks.task(task) else { continue };
            for dependency in &row.waiting_on {
                let Some(upstream) = domain.forge.change(*dependency) else { continue };
                let Some(other) = upstream.change.landing else { continue };
                if other == landing {
                    continue;
                }
                let Some(other) = domain.forge.landing(other) else { continue };
                match other.landing.phase {
                    landing::Phase::Landed | landing::Phase::Cancelled => continue,
                    landing::Phase::Waiting
                    | landing::Phase::Preparing
                    | landing::Phase::Proving
                    | landing::Phase::Proved { .. }
                    | landing::Phase::Turn { .. }
                    | landing::Phase::Held { .. } => {}
                }
                if !kept.as_slice().contains(&landing) {
                    kept.push(landing).expect("bounded landings");
                }
            }
        }
    }
    kept.into_boxed()
}

#[expect(clippy::match_like_matches_macro, reason = "the foundation requires explicit matching")]
pub(crate) fn start_landing_step(domain: &mut Domain, env: &Env<Limits>, context: &tasks::RunContext) {
    let task = context.task;
    let Some(row) = domain.forge.landing(task) else { return };
    let kept = kept_landings(domain, env, row.landing.project);
    let released = if let landing::Phase::Held { .. } = row.landing.phase { true } else { false };
    let Some(entry) = crate::fresh(&mut domain.core.counters, Family::ConnectorRow) else { return };
    domain.forge.offered_landing(task);
    domain.work.push(Work::Forge(forge::Event::StepLanding {
        task,
        entry,
        heard: forge::LandingHeard { released, cancelled: false, kept },
    }));
}

pub(super) fn describe_landing_effect(
    domain: &Domain,
    env: &Env<Limits>,
    task: u64,
    entry: u64,
) -> Option<EffectDescription> {
    let (provider, what, described) = domain.forge.describe_landing_effect(task, entry)?;
    let repository = domain.forge.repository(provider)?;
    let context = domain.core.tasks.delegation(task)?;
    if repository.project != context.project {
        return None;
    }
    Some(EffectDescription {
        waits_for_dependencies: false,
        connector: domain.config.forge_connector,
        purpose: entry,
        effect: authority::Effect {
            connector: domain.config.forge_connector,
            kind: described.kind,
            name: resource_name(repository, &what, env.limits.authority.segments)?,
            state: described.state,
            price: None,
            access: effect_access(repository, &what, described.kind),
            additional: Box::new([]),
            guards: Box::new([]),
        },
        form: match described.form {
            forge::EffectForm::Creation => EffectForm::Creation,
            forge::EffectForm::Transition => EffectForm::Transition,
            forge::EffectForm::Set => EffectForm::Set,
        },
        recovery: match described.recovery {
            forge::Recovery::Keyed => Recovery::Keyed,
            forge::Recovery::Conditional => Recovery::Conditional,
            forge::Recovery::Idempotent => Recovery::Idempotent,
            forge::Recovery::Unrecoverable => Recovery::Unrecoverable,
        },
    })
}

pub(super) fn landing_decision(domain: &mut Domain, env: &Env<Limits>, task: u64, choice: landing::Decision) {
    let Some((connector, code, step)) = domain.core.tasks.procedure_due(task) else { return };
    if connector != domain.config.forge_connector || code != LANDING_CODE {
        return;
    }
    domain.forge.offered_landing(task);
    let end = match choice {
        landing::Decision::Wait { until } => {
            domain.forge.wait_landing(&super::super::environment_forge(env), task, until);
            StepEnd::Wait
        }
        landing::Decision::Finish => {
            StepEnd::Result(tasks::TaskResult::Report { words: landing_result_words(domain, env, task) })
        }
        landing::Decision::Cancel => {
            domain.work.push(Work::Tasks(tasks::Event::Control {
                reply_to: super::super::internal(u64::MAX),
                by: tasks::Party::Deployment { project: domain.forge.landing(task).expect("landing").landing.project },
                task,
                control: tasks::Control::Cancel { reason: Box::new([]), grace: None, message: None },
            }));
            StepEnd::Wait
        }
        landing::Decision::Hold(landing::Hold::Faults { member, gate }) => {
            let row = domain.forge.landing(task).expect("landing");
            let member = row.landing.members.get(usize::from(member)).expect("member");
            let provider = skein_forge::api::Repository {
                forge: member.repository.forge,
                repository: member.repository.repository,
            };
            let mut words = List::with_capacity(env.limits.tasks.message_bytes);
            append(&mut words, b"gate ");
            if let Some(config) = domain.config.landings.get(&row.landing.project) {
                for repository in &config.repositories {
                    if repository.repository == provider
                        && let Some(check) = repository.gates.get(usize::from(gate))
                    {
                        append(&mut words, &check.name);
                    }
                }
            }
            append(&mut words, b" of ");
            if let Some(repository) = domain.forge.repository(provider) {
                append(&mut words, &repository.owner);
                append(&mut words, b"/");
                append(&mut words, &repository.name);
            }
            append(&mut words, b" failed on its runner past its retries");
            let decision = ProcedureStep {
                messages: Box::new([jig_domain_core::StepMessage {
                    target: task,
                    kind: tasks::MessageKind::Words,
                    words: words.into_boxed(),
                }]),
                ..ProcedureStep::new(StepEnd::Hold(tasks::Hold::Procedure))
            };
            domain.work.push(Work::Core(jig_domain_core::Event::ProcedureStep {
                task,
                step,
                connector,
                code,
                decision,
            }));
            return;
        }
        landing::Decision::None
        | landing::Decision::Prepare { .. }
        | landing::Decision::Key { .. }
        | landing::Decision::Dispatch { .. }
        | landing::Decision::Retry { .. }
        | landing::Decision::Post { .. }
        | landing::Decision::Repair { .. }
        | landing::Decision::Proved
        | landing::Decision::Merge { .. }
        | landing::Decision::Stale => StepEnd::Wait,
    };
    domain.work.push(Work::Core(jig_domain_core::Event::ProcedureStep {
        task,
        step,
        connector,
        code,
        decision: ProcedureStep::new(end),
    }));
    if choice == landing::Decision::Finish || choice == landing::Decision::Cancel {
        wake_ready(domain, env, task);
    } else if let landing::Decision::Wait { .. } = choice {
    } else {
        domain.work.push(Work::Tasks(tasks::Event::WakeProcedure { task }));
    }
}

fn wake_ready(domain: &mut Domain, env: &Env<Limits>, task: u64) {
    let Some(row) = domain.forge.landing(task) else { return };
    let project = row.landing.project;
    for repository in domain.forge.writable_repositories(project) {
        for change in domain.forge.changes_for(repository.provider, env.limits.forge.changes) {
            if let Some(row) = domain.forge.change(change)
                && row.change.landing.is_none()
                && let forge_change::State::Ready { .. } = row.change.state
            {
                domain.work.push(Work::Tasks(tasks::Event::WakeProcedure { task: change }));
            }
        }
    }
}
/// Aggregate report contract: owner/name, a space, then the landed commit in hex, one line each.
fn landing_result_words(domain: &Domain, env: &Env<Limits>, task: u64) -> Box<[u8]> {
    let mut words = List::with_capacity(env.limits.tasks.result_bytes);
    for member in &domain.forge.landing(task).expect("finished landing").landing.members {
        let provider =
            skein_forge::api::Repository { forge: member.repository.forge, repository: member.repository.repository };
        if let Some(repository) = domain.forge.repository(provider) {
            append(&mut words, &repository.owner);
            append(&mut words, b"/");
            append(&mut words, &repository.name);
        }
        append(&mut words, b" ");
        let commit = member.landed.expect("finished member landed");
        let width = if commit[20..] == [0; 12] { 20 } else { 32 };
        for byte in commit.get(..width).expect("commit width") {
            append(&mut words, &forge::hex(*byte));
        }
        append(&mut words, b"\n");
    }
    words.into_boxed()
}
