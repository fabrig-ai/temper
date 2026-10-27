//! Supplied hold facts in the same words for task reads and change panels.
use super::{Parts, Text, document};
use alloc::boxed::Box;
use jig_client::{contract as c, words::Word};
use skein_ui::phrases::Hole;

pub(crate) fn summary(task: &c::Task, vocabulary: &dyn Parts) -> Box<[u8]> {
    if budget(task.hold.as_ref()) {
        return skein_lib::bytes::copy_of(b"Insufficient activation budget");
    }
    let mut measure = Text::measure();
    reason(task, vocabulary, &mut measure);
    let mut out = Text::writing_exact(measure);
    reason(task, vocabulary, &mut out);
    out.finish()
}

pub(crate) fn text(task: &c::Task, vocabulary: &dyn Parts, out: &mut Text) {
    if task.hold.is_none() {
        return;
    }
    out.put(b"held: ");
    reason(task, vocabulary, out);
    out.put(b"\nway out: ");
    let uncertain = task.hold.as_ref().expect("supplied hold").uncertain();
    if uncertain {
        out.put(b"check whether the effect happened before deciding to repeat it; ");
    } else {
        out.put(recovery(task.hold.as_ref().expect("supplied hold")));
    }
    out.put(b"task release ");
    out.number(task.chip.task);
    out.put(b" --hold ");
    out.number(task.hold.as_ref().expect("supplied hold").revision().value);
    if uncertain {
        out.put(b" --again only to repeat the effect");
    }
    out.put(b"\n");
}

fn recovery(hold: &c::TaskHold) -> &'static [u8] {
    match hold {
        c::TaskHold::Owner(hold) => match hold.reason {
            c::HoldCategory::Exceeded(_) => b"reduce the request or raise its configured allowance, then ",
            c::HoldCategory::Budget => b"",
            c::HoldCategory::Deadline => b"amend the task's deadline, then ",
            c::HoldCategory::Effects | c::HoldCategory::EffectFailed => b"review and resolve the effects, then ",
            c::HoldCategory::Failures { .. }
            | c::HoldCategory::Stalled
            | c::HoldCategory::Dependency { .. }
            | c::HoldCategory::Stopped
            | c::HoldCategory::Placed { .. }
            | c::HoldCategory::Drift
            | c::HoldCategory::Uncertain { .. }
            | c::HoldCategory::HoldsWaited
            | c::HoldCategory::Circle
            | c::HoldCategory::ResourceUnavailable
            | c::HoldCategory::ResourceAccess
            | c::HoldCategory::PoolLost { .. }
            | c::HoldCategory::Procedure => b"resolve the reason, then ",
        },
        c::TaskHold::Document(hold) => match hold.reason {
            c::HoldReason::Budget { .. } => b"",
            c::HoldReason::Deadline { .. } => b"amend the task's deadline, then ",
            c::HoldReason::EffectFailed { .. } => b"review and resolve the effects, then ",
            c::HoldReason::Tries { .. }
            | c::HoldReason::Stalled { .. }
            | c::HoldReason::Dependency { .. }
            | c::HoldReason::Drift { .. }
            | c::HoldReason::EffectUncertain { .. }
            | c::HoldReason::Stopped { .. }
            | c::HoldReason::Resource { .. }
            | c::HoldReason::Pool { .. }
            | c::HoldReason::Procedure { .. } => b"resolve the reason, then ",
        },
    }
}

fn budget(hold: Option<&c::TaskHold>) -> bool {
    if let Some(c::TaskHold::Owner(hold)) = hold {
        return hold.reason == c::HoldCategory::Budget;
    }
    if let Some(c::TaskHold::Document(hold)) = hold
        && let c::HoldReason::Budget { .. } = hold.reason
    {
        return true;
    }
    false
}

fn budget_reason(task: &c::Task, vocabulary: &dyn Parts, out: &mut Text) {
    let book = vocabulary.phrasebook(4096);
    let said = book
        .say(0, Word::hold(&c::HoldCategory::Budget).id(), &[Hole::Number(task.chip.task)])
        .expect("budget hold phrase");
    out.put(&said.text);
}

fn reason(task: &c::Task, vocabulary: &dyn Parts, out: &mut Text) {
    match task.hold.as_ref() {
        None => out.put(b"reason unavailable"),
        Some(c::TaskHold::Owner(hold)) => {
            if budget(task.hold.as_ref()) {
                budget_reason(task, vocabulary, out);
            } else {
                owner(&hold.reason, out);
            }
            if let Some(source) = &hold.source {
                out.put(b"; source: ");
                vocabulary.owner_part(source, out);
            }
        }
        Some(c::TaskHold::Document(hold)) => {
            if budget(task.hold.as_ref()) {
                budget_reason(task, vocabulary, out);
            } else {
                detailed(&hold.reason, out);
            }
        }
    }
}

fn owner(reason: &c::HoldCategory, out: &mut Text) {
    let number = match reason {
        c::HoldCategory::Exceeded(value) if value.bound != c::Bound::History => Some(value.value),
        c::HoldCategory::Dependency { task } => Some(*task),
        c::HoldCategory::Uncertain { entry } => Some(*entry),
        c::HoldCategory::PoolLost { pool } => Some(*pool),
        c::HoldCategory::Exceeded(_)
        | c::HoldCategory::Failures { .. }
        | c::HoldCategory::Stalled
        | c::HoldCategory::Stopped
        | c::HoldCategory::Placed { .. }
        | c::HoldCategory::Drift
        | c::HoldCategory::Effects
        | c::HoldCategory::EffectFailed
        | c::HoldCategory::Budget
        | c::HoldCategory::Deadline
        | c::HoldCategory::HoldsWaited
        | c::HoldCategory::Circle
        | c::HoldCategory::ResourceUnavailable
        | c::HoldCategory::ResourceAccess
        | c::HoldCategory::Procedure => None,
    };
    let holes: Box<[_]> = match number {
        Some(n) => Box::new([Hole::Number(n)]),
        None => Box::new([]),
    };
    let book = document::phrasebook(4096);
    let said = book.say(0, Word::hold(reason).id(), &holes).expect("hold phrase");
    out.put(said.text.strip_prefix(b"Held: ").unwrap_or(&said.text));
    match reason {
        c::HoldCategory::Placed { hold } => {
            out.put(b" ");
            out.number(*hold);
        }
        c::HoldCategory::Failures { class } => {
            out.put(b": ");
            out.put(match class {
                c::HoldFailure::Transient => b"transient failures",
                c::HoldFailure::Permanent => b"permanent failures",
                c::HoldFailure::Run => b"run failures",
                c::HoldFailure::Agent => b"agent failures",
                c::HoldFailure::Lost => b"lost runs",
                c::HoldFailure::Invalid => b"invalid results",
            });
        }
        c::HoldCategory::Exceeded(_)
        | c::HoldCategory::Stalled
        | c::HoldCategory::Dependency { .. }
        | c::HoldCategory::Stopped
        | c::HoldCategory::Drift
        | c::HoldCategory::Effects
        | c::HoldCategory::EffectFailed
        | c::HoldCategory::Uncertain { .. }
        | c::HoldCategory::Budget
        | c::HoldCategory::Deadline
        | c::HoldCategory::HoldsWaited
        | c::HoldCategory::Circle
        | c::HoldCategory::ResourceUnavailable
        | c::HoldCategory::ResourceAccess
        | c::HoldCategory::PoolLost { .. }
        | c::HoldCategory::Procedure => {}
    }
}

fn detailed(reason: &c::HoldReason, out: &mut Text) {
    match reason {
        c::HoldReason::Budget { spent, limit } => {
            out.put(b"Insufficient activation budget: spent ");
            out.number(*spent);
            out.put(b" of ");
            out.number(*limit);
        }
        c::HoldReason::Deadline { at } => {
            out.put(b"Deadline passed: ");
            document::clock::deadline(*at, out);
        }
        c::HoldReason::Stalled { waiting, .. } => {
            out.put(b"Stalled waiting for ");
            out.words(waiting.bytes());
        }
        c::HoldReason::Dependency { task, .. } => {
            out.put(b"Dependency task ");
            out.number(*task);
            out.put(b" ended unsuccessfully");
        }
        c::HoldReason::Tries { failures } => {
            out.put(b"Out of tries");
            for failure in failures {
                out.put(b"; ");
                out.words(failure.reason.bytes());
            }
        }
        c::HoldReason::Stopped { by, .. } => {
            out.put(b"Task stopped by party ");
            out.number(by.number);
        }
        c::HoldReason::Drift { part } => {
            out.put(b"Required resource drifted: ");
            document::part(part, out);
        }
        c::HoldReason::EffectFailed { part } => {
            out.put(b"An effect failed permanently: ");
            document::part(part, out);
        }
        c::HoldReason::EffectUncertain { part } => {
            out.put(b"An effect may have happened; its outcome is uncertain: ");
            document::part(part, out);
        }
        c::HoldReason::Resource { .. } => out.put(b"Resource holds waited too long"),
        c::HoldReason::Pool { .. } => out.put(b"Allocation lost from pool"),
        c::HoldReason::Procedure { .. } => out.put(b"Procedure needs a decision"),
    }
}
