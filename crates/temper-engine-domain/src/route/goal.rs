//! Goals in the run's words (domain/agent.md, 4.3.2).
use super::{Delegate, Domain, Env, Limits};
use alloc::boxed::Box;
use jig_domain_tasks as tasks;
use skein_lib::{Decimal, List, Writer};
use temper_engine_domain_forge as forge;

/// The root names an advisor before the requester's words (domain/agent.md, 4.3.2).
const ADVISOR_SCOPE: &[u8] =
    b"You are an advisor: you read what your brief asks and report what you found; you change nothing and ask no one.";

const GOAL_SCOPE: &[u8] =
    b"You are a goal: the work your person asked for, yours to plan and to change; you start no goal but an advisor.";
const GOAL_AGREED: &[u8] =
    b"The work is to be agreed: before anything is changed, ask your requester your plan with message, as a question, and wait for the answer.";
const PERSON_WORDS: &[u8] = b"Your person's words, whole:";
const ASK_PLAN: &[u8] =
    b"this work is to be agreed: ask your requester your plan with message, as a question, and wait";

/// Work goals carry a tracked priority; advisors and chats are untracked.
/// The caller owns its plan regardless of who requested it (domain/agent.md, 4.3.2).
pub(crate) fn refusal(domain: &Domain, task: u64, advisor: bool, agree: bool) -> Option<&'static [u8]> {
    let row = domain.core.tasks.task(task)?;
    if advisor && agree {
        Some(b"an advisor agrees nothing: set agree or advisor, not both")
    } else if !advisor && row.tracked.is_some() {
        Some(b"a goal starts an advisor alone: set advisor, or make the change with change")
    } else if advisor && row.tracked.is_none() {
        Some(b"to change a repository, call change; a goal is for work they want to agree first or a survey, and its goal asks for an advisor")
    } else {
        None
    }
}

pub(crate) fn goal(
    domain: &Domain,
    env: &Env<Limits>,
    task: u64,
    context: &tasks::DelegationContext,
    words: Box<[u8]>,
    advisor: bool,
    person_words: Option<Box<[u8]>>,
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
    let agree = person_words.is_some();
    let words = brief(env.limits.tasks.spec_bytes, words, advisor, person_words)?;
    Ok(Delegate {
        executor: tasks::Executor::Agent { charter },
        spec: tasks::Spec { asks: agree, words, parameters: Box::new([]), inputs: Box::new([]) },
        contract: tasks::Contract::Report { words: env.limits.tasks.result_bytes },
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

/// Root lines and caller words are whole; only the person's archived words may be cut.
fn brief(
    bound: u32,
    words: Box<[u8]>,
    advisor: bool,
    person_words: Option<Box<[u8]>>,
) -> Result<Box<[u8]>, tasks::Refusal> {
    let scope = if advisor { ADVISOR_SCOPE } else { GOAL_SCOPE };
    let agreed = if person_words.is_some() { GOAL_AGREED } else { b"" };
    let separator = if agreed.is_empty() { b"".as_slice() } else { b"\n".as_slice() };
    let heading = if person_words.is_some() { PERSON_WORDS } else { b"" };
    let before_heading = if heading.is_empty() { b"".as_slice() } else { b"\n\n".as_slice() };
    let after_heading = if heading.is_empty() { b"".as_slice() } else { b"\n".as_slice() };
    let parts = [scope, separator, agreed, b"\n\n".as_slice(), &words, before_heading, heading, after_heading];
    let mut length = 0_usize;
    for part in parts {
        length = length.checked_add(part.len()).ok_or(tasks::Refusal::Spec)?;
    }
    let room = usize::try_from(bound).expect("u32 fits usize").checked_sub(length).ok_or(tasks::Refusal::Spec)?;
    let person = match person_words {
        Some(bytes) => forge::ResultText { bytes, cut: 0 }
            .fitted(u32::try_from(room).expect("room within spec bound"))
            .ok_or(tasks::Refusal::Spec)?,
        None => Box::new([]),
    };
    length = length.checked_add(person.len()).ok_or(tasks::Refusal::Spec)?;
    let mut writer = Writer::new(length);
    for part in parts {
        writer.put(part).expect("measured goal brief");
    }
    writer.put(&person).expect("measured person's words");
    Ok(writer.finish())
}

pub(crate) fn change_refusal(domain: &Domain, task: u64) -> Option<Box<[u8]>> {
    let row = domain.core.tasks.task(task)?;
    if !row.spec.asks {
        return None;
    }
    if !row.asked_requester {
        return Some(ASK_PLAN.into());
    }
    let mut pending = None;
    for question in &row.questions {
        if question.asked == row.requester {
            pending = Some(match pending {
                Some(old) => core::cmp::min(old, question.number),
                None => question.number,
            });
        }
    }
    let number = Decimal::of(pending?);
    let label = b"wait for the answer to question ";
    let mut writer = Writer::new(label.len().checked_add(number.as_bytes().len()).expect("bounded question refusal"));
    writer.put(label).expect("measured question refusal");
    writer.put(number.as_bytes()).expect("measured question number");
    Some(writer.finish())
}

/// The root reads the chat's own immutable activity, newest first, independently of its caller's summary.
pub(crate) fn request_words(
    domain: &mut Domain,
    env: &Env<Limits>,
    decision: &mut super::Decision,
    to: skein_lib::ReplyTo,
    key: crate::CallKey,
    words: Box<[u8]>,
) {
    let Some(row) = domain.core.tasks.task(key.task) else {
        domain.work.push(super::Work::Core(jig_domain_core::Event::NamedAnswer {
            to,
            key,
            part: jig_domain_core::CallPart::DelegationRefused(tasks::Problem::new(None, tasks::Refusal::Unknown)),
        }));
        return;
    };
    let tasks::Party::Person(person) = row.requester else {
        domain.work.push(super::Work::Core(jig_domain_core::Event::NamedAnswer {
            to,
            key,
            part: jig_domain_core::CallPart::DelegationRefused(tasks::Problem::new(None, tasks::Refusal::Executor)),
        }));
        return;
    };
    let read = Box::new(crate::boundary::GoalWords {
        to: to.into_token(),
        key,
        words,
        person,
        position: row.activity,
        person_words: row.spec.words.clone(),
    });
    if read.position == 0 {
        ready(domain, env, decision, *read);
        return;
    }
    let range = crate::Range::Core(crate::CoreRange::RequesterWord { task: key.task, position: read.position });
    let waiter = domain
        .archive_reads
        .insert(Some(crate::boundary::Read::GoalWords(read)))
        .expect("goal read slot preflighted")
        .token();
    assert!(domain.core.reserve_connector_call(key), "one root-owned agreed-goal read per call");
    super::emit(decision, &env.limits, crate::Delivery::Load { waiter, range, after: None });
}

pub(crate) fn ready(
    domain: &mut Domain,
    env: &Env<Limits>,
    decision: &mut super::Decision,
    read: crate::boundary::GoalWords,
) {
    let to = skein_lib::ReplyTo::new(read.to);
    let result = match domain.core.tasks.delegation(read.key.task) {
        Some(context) => goal(domain, env, read.key.task, &context, read.words, false, Some(read.person_words)),
        None => Err(tasks::Refusal::Unknown),
    };
    match result {
        Ok(member) => {
            super::delegate_call(domain, env, decision, to, read.key, Box::new([member]), false, Box::new([]));
        }
        Err(why) => domain.work.push(super::Work::Core(jig_domain_core::Event::NamedAnswer {
            to,
            key: read.key,
            part: jig_domain_core::CallPart::DelegationRefused(tasks::Problem::new(None, why)),
        })),
    }
}
