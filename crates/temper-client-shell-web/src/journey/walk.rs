//! The native decisions for recording a journey again (client/journeys.md, 8.1).
use super::{Line, Unreadable, line::clone_act, replay::read};
use jig_client_shell_web::adapter::Heard;
use skein_lib::{Duration, Time};
use skein_ui::Act;
use std::collections::BTreeSet;

/// One step of a recorded walk (client/journeys.md, 8.1).
#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    Act { act: Act, event: Box<[u8]> },
    Mark,
    End,
}
/// Why a recording's walk cannot be given again; `step` starts at one.
#[derive(Debug, PartialEq, Eq)]
pub enum Unwalkable {
    Unreadable(Unreadable),
    Drawing { step: u32 },
    Moving { step: u32 },
}
/// Read the acts, key marks and end without replaying possibly stale duties.
/// # Errors
/// Refuses unreadable recordings, drawings and key marks caught moving.
pub fn walk(recording: &[u8]) -> Result<Vec<Step>, Unwalkable> {
    let (lines, _, _) = read(recording).map_err(Unwalkable::Unreadable)?;
    let mut steps = Vec::new();
    for (at, line) in lines.iter().enumerate() {
        let step = number(steps.len());
        if let Line::Act { act, event, .. } = line {
            if matches!(act, Act::Drawing { .. }) {
                return Err(Unwalkable::Drawing { step });
            }
            steps.push(Step::Act { act: clone_act(act), event: event.clone() });
        } else if matches!(line, Line::Mark(_)) && matches!(lines.get(at + 1), Some(Line::Deadline(_))) {
            if let Some(Line::Turn { now, .. }) = at.checked_sub(1).and_then(|at| lines.get(at)) {
                if moving(&lines[at + 2..], *now) {
                    return Err(Unwalkable::Moving { step });
                }
                steps.push(Step::Mark);
            }
        } else if matches!(line, Line::End(_)) {
            steps.push(Step::End);
        }
    }
    Ok(steps)
}
fn number(index: usize) -> u32 {
    u32::try_from(index + 1).expect("walk step count fits u32")
}
fn moving(lines: &[Line], mark: Time) -> bool {
    let until = mark.saturating_add(Duration::from_millis(500));
    let mut now = mark;
    for line in lines {
        if let Line::Turn { now: turn, .. } = line {
            now = *turn;
        } else if now >= mark && now <= until && report(line) {
            return true;
        }
    }
    false
}
// Dues alone do not mean that a page moved.
fn report(line: &Line) -> bool {
    matches!(line, Line::Start | Line::Place(_) | Line::Heard(_) | Line::Carried { .. } | Line::Act { .. })
}

/// Whether the later save holds no new reports but dues, no exchange in flight,
/// and no write awaiting carrying (client/journeys.md, 8.1).
/// The caller measures the half second between the saves.
/// # Errors
/// Refuses an unreadable recording.
pub fn still(earlier: &[u8], later: &[u8]) -> Result<bool, Unreadable> {
    let (earlier, _, _) = read(earlier)?;
    let (later, _, _) = read(later)?;
    let prefix = &earlier[..earlier.len() - 1];
    if !later.starts_with(prefix) || later[prefix.len()..].iter().any(report) {
        return Ok(false);
    }
    let mut exchanges = BTreeSet::new();
    let mut writes = 0_u64;
    for line in later {
        if let Line::Exchange { id, .. } = line {
            exchanges.insert(id);
        } else if let Line::Heard(Heard::Answered { exchange, .. } | Heard::Failed { exchange })
        | Line::Lost(exchange) = line
        {
            exchanges.remove(&exchange);
        } else if let Line::Writes(count) = line {
            writes += u64::from(count);
        } else if let Line::Carried { writes: count } = line {
            writes = writes.saturating_sub(u64::from(count));
        }
    }
    Ok(exchanges.is_empty() && writes == 0)
}

/// The first step, from one, that differs in act, value, event, mark or length.
#[must_use]
pub fn differs(old: &[Step], new: &[Step]) -> Option<u32> {
    old.iter()
        .zip(new)
        .position(|(old, new)| old != new)
        .map(number)
        .or_else(|| (old.len() != new.len()).then(|| number(old.len().min(new.len()))))
}
