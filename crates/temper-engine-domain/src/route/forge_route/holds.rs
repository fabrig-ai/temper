//! The connector's retained account of a procedure decision (domain/forge.md, 8.4).
use super::{
    Domain, EffectDescription, Env, Limits, List, ProcedureStep, StepEnd, Work, append, append_change_replies,
    authority, decimal, forge, tasks,
};
use alloc::boxed::Box;

pub(super) fn source(domain: &Domain, entry: Option<u64>, words: Box<[u8]>) -> tasks::HoldSource {
    tasks::HoldSource::Account { connector: domain.config.forge_connector, entry, words }
}

pub(super) fn decision(domain: &Domain, env: &Env<Limits>, task: u64, words: Box<[u8]>) -> ProcedureStep {
    decision_as(domain, env, task, tasks::Hold::Procedure, words)
}

pub(super) fn decision_as(
    domain: &Domain,
    env: &Env<Limits>,
    task: u64,
    why: tasks::Hold,
    words: Box<[u8]>,
) -> ProcedureStep {
    let mut decision =
        ProcedureStep::new(StepEnd::HoldFrom { why, source: Box::new(source(domain, None, words.clone())) });
    if let Some(row) = domain.core.tasks.task(task)
        && let tasks::Party::Task(target) = row.requester
    {
        decision.messages = Box::new([jig_domain_core::StepMessage {
            target,
            kind: tasks::MessageKind::Words,
            words: forge::text::fit(
                &words,
                usize::try_from(env.limits.tasks.message_bytes).expect("bounded message"),
                b"...",
            ),
        }]);
    }
    decision
}

pub(super) fn findings(words: &mut List<u8>, findings: &[authority::Finding]) {
    for finding in findings {
        append(words, b"; ");
        append(
            words,
            match finding {
                authority::Finding::Executor { .. } => b"executor outside its delegation authority",
                authority::Finding::Tasks { .. } => b"no remaining task authority",
                authority::Finding::Oversized => b"request exceeds its configured bounds",
                authority::Finding::UnknownProject => b"project policy missing",
                authority::Finding::UnknownRole => b"role missing",
                authority::Finding::Authority { lacks, .. } => {
                    for (missing, name) in [
                        (lacks.tools, b"tool authority".as_slice()),
                        (lacks.grants, b"resource grants"),
                        (lacks.executors, b"executor authority"),
                        (lacks.tasks, b"task capacity"),
                        (lacks.depth, b"delegation depth"),
                        (lacks.spend, b"spend"),
                        (lacks.deadline, b"deadline authority"),
                        (lacks.notes, b"note scopes"),
                        (lacks.models, b"models"),
                    ] {
                        if missing {
                            append(words, b"lacks ");
                            append(words, name);
                            append(words, b"; ");
                        }
                    }
                    continue;
                }
                authority::Finding::Spend { .. } | authority::Finding::RunBudget => {
                    b"insufficient spend for the delegate"
                }
                authority::Finding::Grant { .. } => b"operation has no authority grant",
                authority::Finding::Required { .. } => b"required fact missing or pending",
                authority::Finding::Failed { .. } => b"required fact failed",
                authority::Finding::Unguarded { .. } => b"required fact cannot be guarded",
                authority::Finding::ResourceAccess => b"resource access refused",
                authority::Finding::ResourceUnavailable => b"resource unavailable",
                authority::Finding::Tool => b"tool family not granted",
                authority::Finding::Arithmetic => b"authority accounting overflow",
                authority::Finding::RunCap => b"run budget exceeds its cap",
                authority::Finding::Deadline => b"deadline passed",
                authority::Finding::Account => b"model account unusable",
                authority::Finding::Writer => b"resource has another writer",
                authority::Finding::Reference => b"task reference not granted",
                authority::Finding::Scope { .. } => b"note scope not granted",
                authority::Finding::Price { .. } => b"effect price exceeds its funding",
                authority::Finding::Unpermitted => b"request not permitted",
                authority::Finding::Undecidable => b"proposal decision not permitted",
                authority::Finding::PeriodSpend => b"period spend exceeds its ceiling",
            },
        );
    }
}

pub(super) fn effect(words: &mut List<u8>, description: &EffectDescription) {
    append(
        words,
        match description.effect.kind {
            3 => b"open pull request",
            4 => b"merge pull request",
            5 => b"post review",
            6 => b"set status",
            7 => b"post comment",
            8 => b"create issue",
            9 => b"create branch",
            10 => b"edit pull request",
            11 => b"update pull request",
            12 => b"close or reopen pull request",
            13 => b"edit issue",
            14 => b"set reviewers",
            15 => b"delete branch",
            _ => b"forge effect",
        },
    );
    append(words, b" on ");
    for (index, segment) in description.effect.name.segments.iter().enumerate() {
        if index > 0 {
            append(words, b"/");
        }
        append(words, segment);
    }
    if words.len() > 180 {
        words.truncate(177);
        append(words, b"...");
    }
}

pub(super) fn recovery(domain: &Domain, task: u64, words: &mut List<u8>) {
    let mut tail = List::with_capacity(tasks::HOLD_ACCOUNT_BYTES);
    append(&mut tail, b"; resolve this reason, then task release ");
    append(&mut tail, &decimal(task));
    append(&mut tail, b" --hold ");
    let revision = domain.core.tasks.task(task).expect("held procedure").revisions.hold.saturating_add(1);
    append(&mut tail, &decimal(revision));
    append(&mut tail, b"; or task cancel ");
    append(&mut tail, &decimal(task));
    append(&mut tail, b" REASON");
    let room = tasks::HOLD_ACCOUNT_BYTES.checked_sub(tail.len()).expect("recovery fits the account");
    if words.len() > room {
        words.truncate(room.saturating_sub(3));
        append(words, b"...");
    }
    append(words, tail.as_slice());
}

pub(crate) fn refused_step(
    domain: &mut Domain,
    env: &Env<Limits>,
    task: u64,
    step: u64,
    findings_: &[authority::Finding],
) {
    let Some((connector, code @ 2, current)) = domain.core.tasks.procedure_due(task) else { return };
    if current != step || connector != domain.config.forge_connector {
        return;
    }
    let mut words = List::with_capacity(tasks::HOLD_ACCOUNT_BYTES);
    append(&mut words, b"Delegate refused");
    findings(&mut words, findings_);
    recovery(domain, task, &mut words);
    let decision = decision(domain, env, task, words.into_boxed());
    domain.work.push(Work::Core(jig_domain_core::Event::ProcedureStep {
        task,
        step,
        connector,
        code,
        decision: append_change_replies(domain, env, task, decision),
    }));
}

pub(crate) fn hold(domain: &mut Domain, task: u64, reason: &[u8]) {
    hold_as(domain, task, tasks::Hold::Procedure, reason);
}

pub(crate) fn hold_as(domain: &mut Domain, task: u64, why: tasks::Hold, reason: &[u8]) {
    if domain.core.tasks.task(task).is_none() {
        return;
    }
    let mut words = List::with_capacity(tasks::HOLD_ACCOUNT_BYTES);
    append(&mut words, reason);
    if let Some(row) = domain.forge.change(task) {
        append(&mut words, b" for branch ");
        append(&mut words, &row.branch);
        if let Some(repository) = domain.forge.repository(row.repository) {
            append(&mut words, b" in ");
            append(&mut words, &repository.owner);
            append(&mut words, b"/");
            append(&mut words, &repository.name);
        }
    }
    recovery(domain, task, &mut words);
    domain.work.push(Work::Tasks(tasks::Event::HoldFrom {
        task,
        why,
        source: Box::new(source(domain, None, words.into_boxed())),
    }));
}

pub(crate) fn start_refused(domain: &mut Domain, task: u64) {
    if let Some(row) = domain.forge.change(task)
        && row.effect == super::forge_change::EffectResult::Failed
        && let Some(account) = &row.effect_account
    {
        domain.work.push(Work::Tasks(tasks::Event::HoldFrom {
            task,
            why: tasks::Hold::EffectFailed,
            source: Box::new(source(domain, Some(account.entry), account.words.clone())),
        }));
    } else {
        hold(domain, task, b"change procedure cannot start from its repository and branch parameters");
    }
}
