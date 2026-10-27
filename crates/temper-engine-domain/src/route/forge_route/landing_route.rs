//! Deployment-funded landing tasks and their authorized steps (domain/landing.md, 4.2, 11).
use super::{
    Box, Domain, EffectDescription, EffectForm, Env, Family, LANDING_CODE, Limits, List, ProcedureStep, Recovery,
    StepEnd, Work, append, authority, effect_access, forge, forge_change, landing, resource_name, tasks,
};

pub(super) fn start_landing(domain: &mut Domain, env: &Env<Limits>, task: u64) {
    let Some(change) = domain.forge.change(task) else { return };
    if change.change.way != forge_change::Way::Turn {
        return;
    }
    let Some(repository) = domain.forge.repository(change.repository) else { return };
    let project = repository.project;
    if domain.forge.landing_admits(project, &[task]).is_err() {
        return;
    }
    let pull = change.pull.expect("preflighted pull");
    let provider = change.repository;
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
    let mut gates = List::with_capacity(env.limits.forge.gates);
    if let Some(config) = domain.config.landings.get(&project) {
        for repository in &config.repositories {
            if repository.repository == provider {
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
                members: Box::new([landing::Member {
                    change: task,
                    repository: landing::Repository { forge: provider.forge, repository: provider.repository },
                    pull,
                    head: None,
                    base: None,
                    candidate: None,
                    gates: gates.into_boxed(),
                    asked: None,
                    landed: None,
                }]),
            },
            asks: Box::new([None]),
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

#[expect(clippy::match_like_matches_macro, reason = "the foundation requires explicit matching")]
pub(crate) fn start_landing_step(domain: &mut Domain, _env: &Env<Limits>, context: &tasks::RunContext) {
    let task = context.task;
    let Some(row) = domain.forge.landing(task) else { return };
    let released = if let landing::Phase::Held { .. } = row.landing.phase { true } else { false };
    let Some(entry) = crate::fresh(&mut domain.core.counters, Family::ConnectorRow) else { return };
    domain.forge.offered_landing(task);
    domain.work.push(Work::Forge(forge::Event::StepLanding {
        task,
        entry,
        heard: forge::LandingHeard { released, cancelled: false },
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
