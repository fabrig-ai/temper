//! New work waits at the watermark; what finishes or advances admitted
//! work is always taken (skein's `programming-model.md`, 7.1;
//! `domain/worker.md`, 3.1). Passing held work preserves W5 (skein's
//! `programming-model.md`, 7.2).
use crate::{Event, Limits, host};

/// Whether `event` starts new work from the engine: an assignment, a
/// person's message to a run, a source check or a sign-out. Everything
/// else advances admitted work, including source reconciliation.
#[must_use]
pub fn is_new_work(event: &Event) -> bool {
    match event {
        Event::Assign { .. }
        | Event::Link(
            host::LinkDown::Assign { .. }
            | host::LinkDown::Message { .. }
            | host::LinkDown::Check { .. }
            | host::LinkDown::SignOut { .. },
        ) => true,
        Event::Link(
            host::LinkDown::Sources { .. }
            | host::LinkDown::Relayed { .. }
            | host::LinkDown::TurnKept { .. }
            | host::LinkDown::Acknowledge { .. }
            | host::LinkDown::Cancel { .. }
            | host::LinkDown::Grant { .. },
        )
        | Event::SourceAnswered { .. }
        | Event::Ready
        | Event::Lost
        | Event::GracePassed
        | Event::Resend { .. }
        | Event::RelayCancelled { .. }
        | Event::Process(_)
        | Event::Live { .. }
        | Event::Decoded { .. }
        | Event::Encoded { .. }
        | Event::Activity { .. }
        | Event::Done { .. }
        | Event::Shutdown => false,
    }
}

/// Whether `event`, from the engine, passes new work waiting before it:
/// the engine's answers about admitted runs' turns and calls. A cancellation
/// or a grant may name the waiting run, and the link's state follows what
/// came before it, so those wait (W5; `domain/worker.md`, 3.1).
#[must_use]
pub fn passes(event: &Event) -> bool {
    match event {
        Event::Link(
            host::LinkDown::TurnKept { .. } | host::LinkDown::Acknowledge { .. } | host::LinkDown::Relayed { .. },
        )
        | Event::RelayCancelled { .. }
        | Event::Resend { .. } => true,
        Event::Assign { .. }
        | Event::Link(
            host::LinkDown::Assign { .. }
            | host::LinkDown::Message { .. }
            | host::LinkDown::Check { .. }
            | host::LinkDown::SignOut { .. }
            | host::LinkDown::Sources { .. }
            | host::LinkDown::Cancel { .. }
            | host::LinkDown::Grant { .. },
        )
        | Event::SourceAnswered { .. }
        | Event::Ready
        | Event::Lost
        | Event::GracePassed
        | Event::Process(_)
        | Event::Live { .. }
        | Event::Decoded { .. }
        | Event::Encoded { .. }
        | Event::Activity { .. }
        | Event::Done { .. }
        | Event::Shutdown => false,
    }
}

/// Whether the worker takes new work with `backlog` entries waiting.
#[must_use]
pub fn admits(limits: &Limits, backlog: u32) -> bool {
    backlog < limits.watermark
}
