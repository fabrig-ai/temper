//! Admission at the service boundary: an input the journal cannot take returns
//! to its owner, and store answers are always taken (jig's `domain/root.md`, 3;
//! `testing/jig-test-domain/src/admission.rs`).
use crate::boundary::{Event, Link, StoreInput, WorkerInput};
use crate::domain::{Domain, admits, step};
use crate::limits::Limits;
use skein_lib::Env;

/// Step one input if the journal can take its whole route; otherwise hand it
/// back unchanged. Store answers are always taken: a read load's answer waits
/// in the root's queue until the journal has room for its decision.
#[expect(clippy::result_large_err, reason = "the service admission boundary returns the original event by value")]
pub fn try_step(domain: &mut Domain, env: &Env<Limits>, event: Event) -> Result<(), Event> {
    domain.changed();
    crate::domain::invalidate_input(domain, &event);
    match event {
        event @ Event::Store(StoreInput::ReadLoaded { .. } | StoreInput::Switched) => {
            if domain.store_answers.is_empty() && admits(domain, &env.limits) {
                step(domain, env, event);
            } else {
                domain.store_answers.push(event);
            }
            Ok(())
        }
        event @ (Event::Store(
            StoreInput::Committed { .. }
            | StoreInput::Uncommitted { .. }
            | StoreInput::Loaded { .. }
            | StoreInput::Unloaded { .. },
        )
        | Event::Released(_)
        | Event::Resume) => {
            step(domain, env, event);
            Ok(())
        }
        event @ Event::Worker(WorkerInput::Link { link: Link::Up { .. }, .. }) => {
            // A worker's record keeps its bytes here until the fleet is done
            // with them; without room it is answered busy and sent again.
            if !admits(domain, &env.limits) || !crate::link::room(domain) {
                return Err(event);
            }
            step(domain, env, event);
            Ok(())
        }
        event @ (Event::Worker(_)
        | Event::Party(_)
        | Event::Account(_)
        | Event::Credential { .. }
        | Event::Forge(_)
        | Event::Setup { .. }
        | Event::Restart { .. }
        | Event::Stop) => {
            if !admits(domain, &env.limits) {
                return Err(event);
            }
            step(domain, env, event);
            Ok(())
        }
    }
}
