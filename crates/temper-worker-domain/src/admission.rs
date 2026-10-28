//! New work waits at the watermark; what finishes or advances admitted
//! work is always taken (skein's draft `dynamic-memory.md`, 4.2, 4.3).
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

/// Whether the worker takes new work with `backlog` entries waiting.
#[must_use]
pub fn admits(limits: &Limits, backlog: u32) -> bool {
    backlog < limits.watermark
}
