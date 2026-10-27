//! Watch subjects and their conditions (tools contract, 5.4).
use super::{argv::Usage, call::Call};
use jig_client::contract::Phase;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Subject {
    Me,
    Accounts,
    Inbox,
    Task(u64),
    Tree(u64),
    Run(u64),
    Tasks,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Condition {
    Changed,
    Waiting,
    Usable,
    Settled,
    Updates,
    Ended,
    WaitingOnMe,
    Held,
    Replied,
    Phase(Phase),
}
/// Parse a condition for its subject; terminal phases name `--until ended`.
pub fn condition(subject: &Subject, word: &[u8]) -> Result<Condition, Usage> {
    let value = parse(word).ok_or(Usage::Condition)?;
    check(subject, value)?;
    Ok(value)
}
pub(crate) fn check(subject: &Subject, condition: Condition) -> Result<(), Usage> {
    if takes(subject, &condition) {
        return Ok(());
    }
    if let (Subject::Task(_), Condition::Phase(phase)) = (subject, condition) {
        return Err(Usage::TerminalPhase { phase });
    }
    Err(Usage::Condition)
}
/// Tree/run: `Ended` only; `WaitingOnMe`: `Task` only; `Phase`: ten live phases only.
#[must_use]
pub fn takes(subject: &Subject, condition: &Condition) -> bool {
    match (subject, condition) {
        (_, Condition::Changed)
        | (Subject::Accounts, Condition::Usable | Condition::Settled)
        | (Subject::Me, Condition::Waiting)
        | (Subject::Inbox, Condition::Waiting | Condition::Updates)
        | (Subject::Task(_), Condition::Ended | Condition::WaitingOnMe | Condition::Held | Condition::Replied)
        | (Subject::Tree(_) | Subject::Run(_), Condition::Ended) => true,
        (Subject::Task(_), Condition::Phase(phase)) => live(*phase),
        _ => false,
    }
}
pub(super) fn parse(word: &[u8]) -> Option<Condition> {
    Some(match word {
        b"changed" => Condition::Changed,
        b"waiting" => Condition::Waiting,
        b"usable" => Condition::Usable,
        b"settled" => Condition::Settled,
        b"updates" => Condition::Updates,
        b"ended" => Condition::Ended,
        b"waiting-on-me" => Condition::WaitingOnMe,
        b"held" => Condition::Held,
        b"replied" => Condition::Replied,
        _ => {
            let phase = phase(word.strip_prefix(b"phase:")?)?;
            Condition::Phase(phase)
        }
    })
}
#[must_use]
pub fn phase(word: &[u8]) -> Option<Phase> {
    Some(match word {
        b"waiting" => Phase::Waiting,
        b"idle" => Phase::Idle,
        b"due" => Phase::Due,
        b"preparing" => Phase::Preparing,
        b"claimed" => Phase::Claimed,
        b"running" => Phase::Running,
        b"backing-off" => Phase::BackingOff,
        b"active" => Phase::Active,
        b"closing" => Phase::Closing,
        b"held" => Phase::Held,
        b"ended" => Phase::Ended,
        b"done" => Phase::Done,
        b"failed" => Phase::Failed,
        b"cancelled" => Phase::Cancelled,
        _ => return None,
    })
}
#[must_use]
pub fn phase_word(phase: Phase) -> &'static [u8] {
    match phase {
        Phase::Waiting => b"waiting",
        Phase::Idle => b"idle",
        Phase::Due => b"due",
        Phase::Preparing => b"preparing",
        Phase::Claimed => b"claimed",
        Phase::Running => b"running",
        Phase::BackingOff => b"backing-off",
        Phase::Active => b"active",
        Phase::Closing => b"closing",
        Phase::Held => b"held",
        Phase::Ended => b"ended",
        Phase::Done => b"done",
        Phase::Failed => b"failed",
        Phase::Cancelled => b"cancelled",
    }
}
pub(crate) fn live(phase: Phase) -> bool {
    match phase {
        Phase::Waiting
        | Phase::Idle
        | Phase::Due
        | Phase::Preparing
        | Phase::Claimed
        | Phase::Running
        | Phase::BackingOff
        | Phase::Active
        | Phase::Closing
        | Phase::Held => true,
        Phase::Ended | Phase::Done | Phase::Failed | Phase::Cancelled => false,
    }
}

pub(crate) fn of(call: &Call) -> Option<Subject> {
    use super::{call, rows};
    Some(match call.tool {
        rows::ME_AT => Subject::Me,
        rows::ACCOUNTS_AT => Subject::Accounts,
        rows::INBOX_AT => Subject::Inbox,
        rows::TASK_AT => Subject::Task(call::number(call, 0)?),
        rows::TREE_AT => Subject::Tree(call::number(call, 0)?),
        rows::RUN_AT => Subject::Run(call::number(call, 0)?),
        rows::TASKS_AT => Subject::Tasks,
        _ => return None,
    })
}
