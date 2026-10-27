//! Checked wait conditions over live subject facts (tools contract, 5.4).
use crate::jig::subject::Condition;
use jig_client::{
    Latest, contract as c,
    facts::{self, ChatStatus},
};
/// Documents borrowed for this evaluation; the caller supplies live values only.
#[derive(Debug)]
pub struct Facts<'a> {
    pub subject: &'a Latest,
    pub me: Option<&'a c::Me>,
}
/// None means that a document required by this condition is not live yet.
#[must_use]
pub fn holds(condition: &Condition, facts: &Facts<'_>) -> Option<bool> {
    let Latest::Jig(subject) = facts.subject else { return None };
    match subject {
        c::Delivery::Me(me) => match condition {
            Condition::Waiting => Some(me.needs_total > 0),
            Condition::Usable
            | Condition::Settled
            | Condition::Updates
            | Condition::Ended
            | Condition::WaitingOnMe
            | Condition::Held
            | Condition::Replied
            | Condition::Changed
            | Condition::Phase(_) => None,
        },
        c::Delivery::Inbox(inbox) => match condition {
            Condition::Waiting => Some(
                inbox.needs.questions > 0
                    || inbox.needs.proposals > 0
                    || inbox.needs.escalations > 0
                    || inbox.needs.person_tasks > 0,
            ),
            Condition::Updates => Some(inbox.updates.value > 0),
            Condition::Usable
            | Condition::Settled
            | Condition::Ended
            | Condition::WaitingOnMe
            | Condition::Held
            | Condition::Replied
            | Condition::Changed
            | Condition::Phase(_) => None,
        },
        c::Delivery::OwnerInbox(inbox) => match condition {
            Condition::Waiting => Some(inbox.cached_needs > 0),
            Condition::Updates => Some(inbox.cached_updates.value > 0),
            Condition::Usable
            | Condition::Settled
            | Condition::Ended
            | Condition::WaitingOnMe
            | Condition::Held
            | Condition::Replied
            | Condition::Changed
            | Condition::Phase(_) => None,
        },
        c::Delivery::Task(task) => task_holds(*condition, task, facts.me),
        c::Delivery::Tree(tree) => {
            if *condition != Condition::Ended {
                return None;
            }
            for node in &tree.nodes {
                if node.chip.task == tree.task {
                    return Some(ended(node.chip.phase));
                }
            }
            None
        }
        c::Delivery::OwnerTree(tree) => {
            if *condition == Condition::Ended {
                Some(ended(tree.root.phase))
            } else {
                None
            }
        }
        c::Delivery::OwnerAccounts(accounts) => match condition {
            Condition::Usable => Some(accounts.usable),
            Condition::Settled => Some(accounts.settled),
            Condition::Waiting
            | Condition::Updates
            | Condition::Ended
            | Condition::WaitingOnMe
            | Condition::Held
            | Condition::Replied
            | Condition::Changed
            | Condition::Phase(_) => None,
        },
        c::Delivery::OwnerProject(_)
        | c::Delivery::OwnerProposal(_)
        | c::Delivery::Chips(_)
        | c::Delivery::Tasks(_)
        | c::Delivery::Run(_)
        | c::Delivery::RunState(_)
        | c::Delivery::Settings(_)
        | c::Delivery::Deployment(_)
        | c::Delivery::OwnerDeployment(_) => None,
    }
}
fn task_holds(condition: Condition, task: &c::Task, me: Option<&c::Me>) -> Option<bool> {
    Some(match condition {
        Condition::Ended => ended(task.chip.phase),
        Condition::Held => task.chip.phase == c::Phase::Held,
        Condition::Phase(phase) => task.chip.phase == phase,
        Condition::WaitingOnMe => waits_on_me(&task.waits_on, task.project, me?),
        Condition::Replied => {
            match facts::chat_status(&task.chip, &task.waits_on, task.run_wait.as_ref(), task.project, me?) {
                ChatStatus::HostRefused(_)
                | ChatStatus::Parked
                | ChatStatus::WaitingForYou { .. }
                | ChatStatus::WaitingFor(_)
                | ChatStatus::Held(_)
                | ChatStatus::Closing
                | ChatStatus::Ended { .. } => true,
                ChatStatus::Starting
                | ChatStatus::Writing
                | ChatStatus::Tool { .. }
                | ChatStatus::Working { .. }
                | ChatStatus::Retrying { .. } => false,
            }
        }
        Condition::Changed | Condition::Usable | Condition::Settled | Condition::Waiting | Condition::Updates => {
            return None;
        }
    })
}
/// Terminal chip facts can arrive before a watch's terminal.
#[must_use]
pub fn ended(phase: c::Phase) -> bool {
    match phase {
        c::Phase::Ended | c::Phase::Done | c::Phase::Failed | c::Phase::Cancelled => true,
        c::Phase::Waiting
        | c::Phase::Idle
        | c::Phase::Due
        | c::Phase::Preparing
        | c::Phase::Claimed
        | c::Phase::Running
        | c::Phase::BackingOff
        | c::Phase::Active
        | c::Phase::Closing
        | c::Phase::Held => false,
    }
}
/// A task's supplied waits-on already covers its whole tree (client contract, 5.1).
#[must_use]
pub fn waits_on_me(waits_on: &c::WaitsOn, project: u32, me: &c::Me) -> bool {
    let c::WaitsOn::Party { whom, .. } = waits_on else { return false };
    match whom {
        c::Whom::Person(party) => *party == me.party.number,
        c::Whom::Role { project: held_project, role } => {
            if *held_project != project {
                return false;
            }
            for held in &me.projects {
                let number = match held.role {
                    c::HeldRole::Owner => 0,
                    c::HeldRole::Maintainer => 1,
                    c::HeldRole::Member => 2,
                    c::HeldRole::Observer => 3,
                    c::HeldRole::Policy { role } => role,
                };
                if held.project == project && number == *role {
                    return true;
                }
            }
            false
        }
    }
}
