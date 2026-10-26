//! Goals in the run's words (domain/agent.md, 4.3.2).
use super::{Delegate, Domain, Env, Limits};
use alloc::boxed::Box;
use jig_domain_tasks as tasks;
use skein_lib::List;
use temper_engine_domain_forge as forge;

pub(crate) fn goal(
    domain: &Domain,
    env: &Env<Limits>,
    task: u64,
    context: &tasks::DelegationContext,
    words: Box<[u8]>,
    advisor: bool,
) -> Result<Delegate, tasks::Refusal> {
    let row = domain.core.tasks.task(task).ok_or(tasks::Refusal::Unknown)?;
    let tasks::Executor::Agent { charter } = row.executor else { return Err(tasks::Refusal::Executor) };
    if context.tasks_left == 1 {
        return Err(tasks::Refusal::Tree);
    }
    if context.authority.delegation.depth == 1 {
        return Err(tasks::Refusal::Depth);
    }
    let spend = context
        .numbers
        .budget
        .checked_sub(context.numbers.spent)
        .ok_or(tasks::Refusal::Funding)?
        .checked_sub(context.numbers.spent_below)
        .ok_or(tasks::Refusal::Funding)?
        .checked_sub(context.numbers.reserved)
        .ok_or(tasks::Refusal::Funding)?
        / 2;
    if spend <= domain.core.authority.rules().minimum_run_spend {
        return Err(tasks::Refusal::Funding);
    }
    let mut authority = context.authority.clone();
    let mut grants = List::with_capacity(u32::try_from(authority.grants.len()).expect("bounded grants"));
    for grant in &authority.grants {
        if grant.connector == domain.config.forge_connector
            && (forge::READ_KINDS.contains(&grant.kind) || (!advisor && forge::CHANGE_KINDS.contains(&grant.kind)))
        {
            grants.push(grant.clone()).expect("narrowed grants fit");
        }
    }
    authority.grants = grants.into_boxed();
    authority.delegation.tasks = context.tasks_left.saturating_sub(1);
    authority.delegation.depth = authority.delegation.depth.saturating_sub(1);
    authority.budget.spend = spend;
    authority.notes = tasks::Scopes(0);
    authority.note_resources = Box::new([]);
    if advisor {
        authority.tools.0 &= domain.core.settings.tools.read.0;
        authority.delegation.kinds = Box::new([]);
        authority.delegation.tasks = 0;
        authority.delegation.depth = 0;
    }
    let tracked = if advisor {
        None
    } else if let Some(priority) = row.tracked {
        Some(priority)
    } else {
        // Project queries order lower priorities first, then task number. At the
        // numeric ceiling, the new number follows existing goals of that priority.
        let mut priority = 0;
        for goal in &domain.core.tasks.view_tasks() {
            if goal.project == context.project
                && let Some(existing) = goal.tracked
            {
                priority = priority.max(existing.saturating_add(1));
            }
        }
        Some(priority)
    };
    let bound = match &row.contract {
        tasks::Contract::Report { words } | tasks::Contract::Change { words, .. } => *words,
        tasks::Contract::Verdict { choices } => {
            let mut bound = 0;
            for choice in choices {
                bound = bound.max(choice.words);
            }
            bound
        }
        tasks::Contract::Approval { .. } => 1024,
    }
    .min(env.limits.tasks.result_bytes);
    Ok(Delegate {
        executor: tasks::Executor::Agent { charter },
        spec: tasks::Spec { words, parameters: Box::new([]), inputs: Box::new([]) },
        contract: tasks::Contract::Report { words: bound },
        authority,
        deadline: tasks::Deadline::Named,
        llm: None,
        symbolic_grants: Box::new([]),
        dependencies: Box::new([]),
        wake: tasks::WakePolicy::DEFAULT,
        tracked,
        label: 0,
        flags: tasks::Flags::PLAIN,
        budget: jig_domain_people::Budget::Named,
    })
}
