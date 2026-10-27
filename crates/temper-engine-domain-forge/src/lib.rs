//! The forge connector's top (jig's domain/connectors.md, sections 2–8;
//! domain/forge.md, sections 3–12). It keeps adopted repositories, resource
//! holds, writer slots, subscriptions, procedure and projection rows, and
//! outbox entries. The client alone executes API calls; change and issues
//! decide policy without retained state.
//!
//! `step` accepts one parent or client event. Saves and erases join the
//! parent's decision. `Committed` is the only way a newly enqueued effect
//! reaches the client. `resume` and `fire` drive the client after earlier
//! saves have crossed that barrier. The top never knows tasks beyond numbers,
//! authorization grants or protocol wire formats.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
use alloc::boxed::Box;
mod address;
pub use address::{PULL_ADDRESS_BYTES, pull_address};
mod adoption;
pub mod boundary;
mod brief;
mod brief_ci;
mod capabilities;
mod ci;
mod commit;
mod connect;
mod demo;
mod domain;
mod gates;
mod held;
pub mod items;
mod judge;
mod landing;
mod limits;
mod panel;
pub mod purpose;
mod result;
pub use result::{ChangeResult, ResultText};
mod reconcile;
pub mod runs;
pub use reconcile::{RecordKind, Unresolved};
pub mod resources;
mod settle;
#[cfg(test)]
mod tests;
pub mod text;
mod topics;
pub use boundary::*;
pub use brief::{HOME_SECTION_BYTES, hex, home_access_reason};
pub use commit::commit_text;
pub use domain::{Domain, fire, max_out, resume, step, tag, untag, writer};
pub use gates::{GateCandidate, GateLast, GatePattern, GatePolicy, GateTemplate};
pub use judge::{Criterion, Freshness as JudgeFreshness, Judges, Reviewer, Verdict as JudgeVerdict};
pub use limits::{
    BOT_LOGIN_SUFFIX, CONNECT_WHY_BYTES, FIRST_NAME_BYTES, Limits, STOPPED_TEXT_BYTES, connection_worst_case,
    demo_act_bound, worst_case,
};
pub use panel::{MARKED, PANEL_AHEAD, PANEL_GATES, PANEL_NAME_BYTES, Panel, PanelGate, PanelStep, PanelUsed};
pub use temper_engine_domain_forge_landing::{CHECK_NAME_BYTES, WORKFLOW_BYTES};
/// Connector-owned access to one exact effect on an adopted resource.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Access {
    Owned,
    Participant,
    Context,
    Unavailable,
}

/// What a forge write does to its named target.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EffectForm {
    Creation,
    Transition,
    Set,
}

/// Recovery promise of one forge effect kind.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Recovery {
    Keyed,
    Conditional,
    Idempotent,
    Unrecoverable,
}

/// Effect form and recovery class declared by the forge connector.
///
/// The kinds follow domain/forge.md, section 6, one kind per row:
/// 3 opens pulls, 10 edits their title/body, 11 updates from the base,
/// 4 merges, 12 closes/reopens, 5 posts reviews, 7 posts comments,
/// 6 sets statuses, 8 creates issues, 13 edits/closes/reopens issues,
/// 14 sets reviewers, 9 creates branches, and 15 deletes branches.
/// Kind 1 reads and kind 2 grants workspace pushes. The order has no
/// implications between distinct kinds: each intended write needs a grant.
#[must_use]
pub const fn effect_shape(kind: u16) -> Option<(EffectForm, Recovery)> {
    match kind {
        2 | 11 | 17 => Some((EffectForm::Transition, Recovery::Unrecoverable)),
        4 => Some((EffectForm::Transition, Recovery::Conditional)),
        3 => Some((EffectForm::Creation, Recovery::Conditional)),
        5 | 7 | 8 | 9 | 16 => Some((EffectForm::Creation, Recovery::Unrecoverable)),
        6 | 10 | 12..=15 => Some((EffectForm::Set, Recovery::Idempotent)),
        0 | 1 | 18..=u16::MAX => None,
    }
}

/// A kept change's landing waits for dependencies (domain/forge.md, 8.2 item 7).
#[must_use]
pub const fn waits_for_dependencies(kind: u16) -> bool {
    kind == 4
}

/// Connector-owned request and the facts jig may inspect before admitting it.
#[derive(Debug)]
pub struct AgentEffect {
    pub effect: skein_forge_client::Effect,
    pub kind: u16,
    pub state: [u8; 32],
    pub form: EffectForm,
    pub recovery: Recovery,
}

/// Classify and bound an agent write in the connector's own vocabulary.
pub fn describe_agent_effect(
    repository: &Repository,
    resource: &What,
    write: skein_forge::api::Write,
    key: Box<[u8]>,
    op_bytes: u32,
) -> Result<AgentEffect, skein_forge::api::Error> {
    let (write, kind, state, form, valid_resource) = match write {
        skein_forge::api::Write::CreateIssue { title, body, .. } => (
            skein_forge::api::Write::CreateIssue { key, title, body },
            8,
            [0; 32],
            EffectForm::Creation,
            *resource == What::Repository,
        ),
        skein_forge::api::Write::Post { number, body, .. } => (
            skein_forge::api::Write::Post { number, key, body },
            7,
            [0; 32],
            EffectForm::Creation,
            *resource == What::Issue(number) || *resource == What::Pull(number),
        ),
        skein_forge::api::Write::Status { commit, context, check, description, link } => (
            skein_forge::api::Write::Status { commit, context, check, description, link },
            6,
            commit,
            EffectForm::Set,
            *resource == What::Repository,
        ),
        skein_forge::api::Write::OpenPull { .. }
        | skein_forge::api::Write::Review { .. }
        | skein_forge::api::Write::Edit { .. }
        | skein_forge::api::Write::SetReviewers { .. }
        | skein_forge::api::Write::Close { .. }
        | skein_forge::api::Write::Reopen { .. }
        | skein_forge::api::Write::Merge { .. }
        | skein_forge::api::Write::Update { .. }
        | skein_forge::api::Write::CreateBranch { .. }
        | skein_forge::api::Write::Dispatch { .. }
        | skein_forge::api::Write::Rerun { .. }
        | skein_forge::api::Write::DeleteBranch { .. } => return Err(skein_forge::api::Error::Forbidden),
    };
    let access = effect_access(repository, resource, kind);
    let available = match access {
        Access::Owned | Access::Participant => true,
        Access::Context | Access::Unavailable => false,
    };
    if !valid_resource || !available {
        return Err(skein_forge::api::Error::Forbidden);
    }
    let effect = skein_forge_client::Effect { write, condition: skein_forge_client::Condition::None };
    let recovery = match skein_forge_client::recovery(&effect.write) {
        skein_forge_client::Recovery::Keyed => Recovery::Keyed,
        skein_forge_client::Recovery::Conditional => Recovery::Conditional,
        skein_forge_client::Recovery::Idempotent => Recovery::Idempotent,
        skein_forge_client::Recovery::Unrecoverable => Recovery::Unrecoverable,
    };
    match skein_forge_client::measure::effect(&effect) {
        Some(bytes) if bytes <= u64::from(op_bytes) => {}
        Some(_) | None => return Err(skein_forge::api::Error::Forbidden),
    }
    Ok(AgentEffect { effect, kind, state, form, recovery })
}

/// Describe current access from the forge's adopted role and effect kinds.
#[must_use]
pub fn effect_access(repository: &Repository, what: &What, kind: u16) -> Access {
    let permitted = match kind {
        1 => repository.kinds.read,
        2 => repository.kinds.push,
        3 | 10..=12 => repository.kinds.open,
        4 => repository.kinds.land,
        5 | 14 => repository.kinds.review,
        6 => repository.kinds.status,
        7 => repository.kinds.comment,
        8 | 13 => repository.kinds.issue,
        9 | 15..=17 => repository.kinds.branch,
        _ => false,
    };
    let shared = match what {
        What::Issue(_) | What::Pull(_) => true,
        What::Repository | What::Branch(_) => false,
    };
    match repository.role {
        Role::Context => Access::Context,
        Role::Owned | Role::Adopted | Role::Fork if !permitted => Access::Unavailable,
        Role::Owned | Role::Adopted | Role::Fork if shared => Access::Participant,
        Role::Adopted => Access::Participant,
        Role::Fork if kind == 4 => Access::Participant,
        Role::Owned | Role::Fork => Access::Owned,
    }
}
/// Stable key of one durable connector row.
#[must_use]
pub fn stored_key(row: &Stored) -> Key {
    match row {
        Stored::Landing(row) => Key::Landing(row.landing.task),
        Stored::Receipt { key, .. } => Key::Receipt(key.clone()),
        Stored::Call { key, .. } => Key::Call(*key),
        Stored::Repository(row) => Key::Repository(row.provider),
        Stored::Hold(row) => Key::Hold(row.name.clone()),
        Stored::Names { task, .. } => Key::Names(*task),
        Stored::Subscription(row) => Key::Subscription { task: row.task, topic: row.topic.clone() },
        Stored::BranchHead(row) => Key::BranchHead(row.name.clone()),
        Stored::PullState(row) => Key::PullState(row.name.clone()),
        Stored::Ci(row) => Key::Ci { repository: row.repository, head: row.head },
        Stored::Landed { commit, .. } => Key::Landed(*commit),
        Stored::ProposedEffect(row) => Key::ProposedEffect(row.number),
        Stored::Entry { entry, .. } => Key::Entry(entry.number),
        Stored::Client(row) => Key::Client(match row {
            skein_forge_client::Stored::Live(row) => skein_forge_client::Key::Live(row.watch.resource.clone()),
            skein_forge_client::Stored::Repository(row) => skein_forge_client::Key::Repository(row.repository),
            skein_forge_client::Stored::Writer(_) => {
                unreachable!("the connector keeps the client's writer as Stored::Writer")
            }
        }),
        Stored::Change(row) => Key::Change(row.task),
        Stored::Issue(row) => Key::Issue(row.goal),
        Stored::Release(row) => Key::Release(row.task),
        Stored::Application { token, .. } => Key::Application(*token),
        Stored::Writer(row) => Key::Writer(row.forge),
        Stored::Demo(row) => Key::Demo(row.forge),
        Stored::Binding { address, .. } => Key::Binding(address.clone()),
        Stored::DemoAct(row) => Key::DemoAct(row.key),
        Stored::Flight { task, entry } => Key::Flight { task: *task, entry: *entry },
    }
}
/// Checked payload bytes of one connector record for its parent's store page.
#[must_use]
#[expect(clippy::too_many_lines, reason = "one exhaustive connector record payload sum")]
pub fn stored_bytes(record: &Stored) -> Option<u64> {
    use alloc::boxed::Box;
    use core::mem::size_of;
    use temper_engine_domain_forge_change as change;
    fn bytes(value: &[u8]) -> Option<u64> {
        u64::try_from(value.len()).ok()
    }
    fn remark_bytes(remarks: &[GateRemark]) -> Option<u64> {
        let mut total = 0_u64;
        for remark in remarks {
            total = total.checked_add(bytes(&remark.words)?)?;
        }
        Some(total)
    }
    fn name(value: &Name) -> Option<u64> {
        match &value.what {
            What::Branch(parts) => {
                let mut held =
                    u64::try_from(parts.len()).ok()?.checked_mul(u64::try_from(size_of::<Box<[u8]>>()).ok()?)?;
                for part in parts {
                    held = held.checked_add(bytes(part)?)?;
                }
                Some(held)
            }
            What::Repository | What::Pull(_) | What::Issue(_) => Some(0),
        }
    }
    fn topic(value: &Topic) -> Option<u64> {
        match value {
            Topic::Landings { branch, .. } => bytes(branch),
            Topic::Ci { .. } | Topic::Pull { .. } | Topic::Participation { .. } => Some(0),
        }
    }
    fn state(value: &change::State, depth: u32) -> Option<u64> {
        if depth > 16 {
            return None;
        }
        match value {
            change::State::Gating { asked, .. } => {
                u64::try_from(asked.len()).ok()?.checked_mul(u64::try_from(size_of::<u64>()).ok()?)
            }
            change::State::Held { was, .. } => {
                u64::try_from(size_of::<change::State>()).ok()?.checked_add(state(was, depth.checked_add(1)?)?)
            }
            change::State::Producing { .. }
            | change::State::Opening { .. }
            | change::State::Recreating { .. }
            | change::State::Reopening { .. }
            | change::State::Checking { .. }
            | change::State::CancellingGates { .. }
            | change::State::Queued { .. }
            | change::State::First { .. }
            | change::State::Ready { .. }
            | change::State::Preparing { .. }
            | change::State::Updating { .. }
            | change::State::Resolving { .. }
            | change::State::Repairing { .. }
            | change::State::Landing { .. }
            | change::State::Ended
            | change::State::Landed { .. } => Some(0),
        }
    }
    match record {
        Stored::Landing(row) => landing::row_bytes(row),
        Stored::Receipt { key, receipt } => landing::receipt_bytes(key, receipt),
        Stored::Call { answer, .. } => match answer {
            CallPayload::Effect { .. } | CallPayload::Refused(_) => Some(0),
            CallPayload::Read(answer) => match answer.as_ref() {
                Ok(answer) => skein_forge::measure::answer(answer),
                Err(_) => Some(0),
            },
        },
        Stored::Repository(row) => {
            let mut held = bytes(&row.host)?
                .checked_add(bytes(&row.owner)?)?
                .checked_add(bytes(&row.name)?)?
                .checked_add(bytes(&row.prefix)?)?
                .checked_add(bytes(&row.settings.default_branch)?)?;
            match &row.protection {
                Protection::Rule(rule) => {
                    held = held.checked_add(bytes(&rule.branch)?)?.checked_add(
                        u64::try_from(rule.contexts.len())
                            .ok()?
                            .checked_mul(u64::try_from(size_of::<Box<[u8]>>()).ok()?)?,
                    )?;
                    for context in &rule.contexts {
                        held = held.checked_add(bytes(context)?)?;
                    }
                }
                Protection::Unknown | Protection::Absent => {}
            }
            Some(held)
        }
        Stored::Application { outcome, .. } => match outcome {
            ApplicationOutcome::CoreRefused(_) | ApplicationOutcome::ForgeRefused(_) => Some(0),
            ApplicationOutcome::Adopted(row) => {
                let mut held = bytes(&row.owner)?.checked_add(bytes(&row.name)?)?;
                match &row.protection {
                    Protection::Unknown | Protection::Absent => {}
                    Protection::Rule(rule) => {
                        held = held.checked_add(bytes(&rule.branch)?)?.checked_add(
                            u64::try_from(rule.contexts.len())
                                .ok()?
                                .checked_mul(u64::try_from(size_of::<Box<[u8]>>()).ok()?)?,
                        )?;
                        for context in &rule.contexts {
                            held = held.checked_add(bytes(context)?)?;
                        }
                    }
                }
                Some(held)
            }
        },
        Stored::Hold(row) => {
            let mut bytes = name(&row.name)?;
            if let Some(drift) = &row.drift {
                match &drift.change {
                    DriftChange::Retargeted { before, after, .. } => {
                        bytes = bytes
                            .checked_add(u64::try_from(before.len()).ok()?)?
                            .checked_add(u64::try_from(after.len()).ok()?)?;
                    }
                    DriftChange::Branch { .. } | DriftChange::PullClosed { .. } => {}
                }
            }
            Some(bytes)
        }
        Stored::Names { resources, .. } => {
            let mut held = u64::try_from(resources.len()).ok()?.checked_mul(u64::try_from(size_of::<Name>()).ok()?)?;
            for resource in resources {
                held = held.checked_add(name(resource)?)?;
            }
            Some(held)
        }
        Stored::Subscription(row) => {
            let tasks = match &row.goal_tasks {
                Some(tasks) => u64::try_from(tasks.len()).ok()?.checked_mul(8)?,
                None => 0,
            };
            let mut held = topic(&row.topic)?.checked_add(tasks)?.checked_add(
                u64::try_from(row.paths.len()).ok()?.checked_mul(u64::try_from(size_of::<Box<[u8]>>()).ok()?)?,
            )?;
            for path in &row.paths {
                held = held.checked_add(bytes(path)?)?;
            }
            Some(held)
        }
        Stored::Binding { address, subject } => bytes(address)?.checked_add(bytes(subject)?),
        Stored::BranchHead(row) => name(&row.name),
        Stored::PullState(row) => name(&row.name),
        Stored::Ci(_)
        | Stored::Landed { .. }
        | Stored::Writer(_)
        | Stored::Demo(_)
        | Stored::DemoAct(_)
        | Stored::Flight { .. } => Some(0),
        Stored::ProposedEffect(row) => {
            let resource = match &row.resource {
                What::Branch(branch) => {
                    let mut total = 0_u64;
                    for segment in branch {
                        total = total.checked_add(bytes(segment)?)?;
                    }
                    total
                }
                What::Repository | What::Pull(_) | What::Issue(_) => 0,
            };
            skein_forge_client::measure::effect(&row.entry.effect)?.checked_add(resource)
        }
        Stored::Entry { entry, .. } => skein_forge_client::measure::effect(&entry.effect),
        Stored::Client(row) => skein_forge_client::measure::stored(row),
        Stored::Change(row) => bytes(&row.branch)?
            .checked_add(bytes(&row.base)?)?
            .checked_add(bytes(&row.title)?)?
            .checked_add(bytes(&row.body)?)?
            .checked_add(
                u64::try_from(row.change.gates.len())
                    .ok()?
                    .checked_mul(u64::try_from(size_of::<change::Gate>()).ok()?)?,
            )?
            .checked_add(u64::try_from(row.change.clean.len()).ok()?.checked_mul(32)?)?
            .checked_add(
                u64::try_from(row.change.gate_failures.len())
                    .ok()?
                    .checked_mul(u64::try_from(size_of::<change::GateFailure>()).ok()?)?,
            )?
            .checked_add(
                u64::try_from(row.verdicts.len())
                    .ok()?
                    .checked_mul(u64::try_from(size_of::<change::GateReport>()).ok()?)?,
            )?
            .checked_add(
                u64::try_from(row.gate_remarks.len())
                    .ok()?
                    .checked_mul(u64::try_from(size_of::<GateRemark>()).ok()?)?,
            )?
            .checked_add(remark_bytes(&row.gate_remarks)?)?
            .checked_add(bytes(&row.result.checks.bytes)?)?
            .checked_add(bytes(&row.result.failed.bytes)?)?
            .checked_add(bytes(&row.result.repairs.bytes)?)?
            .checked_add(bytes(&row.result.reviews.bytes)?)?
            .checked_add(state(&row.change.state, 0)?),
        Stored::Issue(row) => {
            let before = match &row.before {
                Some(before) => before.milestones.len(),
                None => 0,
            };
            let mut held = u64::try_from(row.state.milestones.len().checked_add(before)?)
                .ok()?
                .checked_mul(u64::try_from(size_of::<temper_engine_domain_forge_issues::MilestoneKey>()).ok()?)?;
            if let Some(view) = &row.desired {
                held =
                    held.checked_add(bytes(view.title.as_bytes())?)?
                        .checked_add(bytes(view.goal_text.as_bytes())?)?
                        .checked_add(u64::try_from(view.plan.len()).ok()?.checked_mul(
                            u64::try_from(size_of::<temper_engine_domain_forge_issues::PlanItem>()).ok()?,
                        )?)?
                        .checked_add(u64::try_from(view.milestones.len()).ok()?.checked_mul(
                            u64::try_from(size_of::<temper_engine_domain_forge_issues::Milestone>()).ok()?,
                        )?)?;
                for item in &view.plan {
                    held = held.checked_add(bytes(item.text.as_bytes())?)?;
                }
                for milestone in &view.milestones {
                    held = held.checked_add(bytes(milestone.text.as_bytes())?)?;
                }
                if let Some(finished) = &view.finished {
                    held = held.checked_add(bytes(finished.as_bytes())?)?;
                }
            }
            Some(held)
        }
        Stored::Release(row) => {
            let comment = match &row.comment {
                Some(comment) => bytes(comment)?,
                None => 0,
            };
            let pending = match &row.pending {
                Some((_, Some(resource))) => name(resource)?,
                Some((_, None)) | None => 0,
            };
            comment.checked_add(pending)
        }
    }
}

/// Connector-owned description of one goal projection write.
#[derive(PartialEq, Eq, Debug)]
pub struct ProjectionEffect {
    pub resource: What,
    pub kind: u16,
    pub form: EffectForm,
    pub recovery: Recovery,
    pub access: Access,
}

/// Describe the write selected by the issue projection mechanism.
#[must_use]
pub fn describe_projection_effect(
    repository: &Repository,
    write: &skein_forge::api::Write,
) -> Option<ProjectionEffect> {
    let (resource, kind, form) = match write {
        skein_forge::api::Write::CreateIssue { .. } => (What::Repository, 8, EffectForm::Creation),
        skein_forge::api::Write::Edit { number, .. } | skein_forge::api::Write::Close { number } => {
            (What::Issue(*number), 13, EffectForm::Set)
        }
        skein_forge::api::Write::Post { number, .. } => (What::Issue(*number), 7, EffectForm::Creation),
        skein_forge::api::Write::Status { .. }
        | skein_forge::api::Write::OpenPull { .. }
        | skein_forge::api::Write::Review { .. }
        | skein_forge::api::Write::SetReviewers { .. }
        | skein_forge::api::Write::Reopen { .. }
        | skein_forge::api::Write::Merge { .. }
        | skein_forge::api::Write::Update { .. }
        | skein_forge::api::Write::CreateBranch { .. }
        | skein_forge::api::Write::Dispatch { .. }
        | skein_forge::api::Write::Rerun { .. }
        | skein_forge::api::Write::DeleteBranch { .. } => return None,
    };
    let recovery = match skein_forge_client::recovery(write) {
        skein_forge_client::Recovery::Keyed => Recovery::Keyed,
        skein_forge_client::Recovery::Conditional => Recovery::Conditional,
        skein_forge_client::Recovery::Idempotent => Recovery::Idempotent,
        skein_forge_client::Recovery::Unrecoverable => Recovery::Unrecoverable,
    };
    Some(ProjectionEffect { access: effect_access(repository, &resource, kind), resource, kind, form, recovery })
}

/// A canonical member address, bounded as protocol/sign-in.md, 3.1 requires.
#[must_use]
pub fn binding_address(address: &[u8]) -> bool {
    if !(3..=254).contains(&address.len()) || address.first() == Some(&b'@') || address.last() == Some(&b'@') {
        return false;
    }
    let mut at = false;
    for byte in address {
        if !byte.is_ascii_graphic() || byte.is_ascii_uppercase() {
            return false;
        }
        if *byte == b'@' {
            if at {
                return false;
            }
            at = true;
        }
    }
    at
}
