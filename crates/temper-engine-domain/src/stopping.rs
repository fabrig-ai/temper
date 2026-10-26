//! Routing the engine's drain without deciding connector work (jig's
//! `domain/engine.md`, 6.1; `testing/jig-test-domain/src/stopping.rs`).
use crate::boundary::{Event, Payload, Work};
use crate::domain::Domain;
use alloc::boxed::Box;
use jig_domain_fleet as fleet;
use skein_lib::ReplyTo;
use temper_engine_domain_forge as forge;

/// While the core drains, a forge input returns to its owner undecided.
pub(crate) fn defer(domain: &Domain, event: Event) -> Result<Event, Box<forge::Event>> {
    if !domain.core.stopping() {
        return Ok(event);
    }
    match event {
        Event::Forge(event) => Err(event),
        event @ (Event::Store(_)
        | Event::Worker(_)
        | Event::Party(_)
        | Event::Account(_)
        | Event::Credential { .. }
        | Event::Released(_)
        | Event::Setup { .. }
        | Event::Restart { .. }
        | Event::Resume
        | Event::Stop) => Ok(event),
    }
}

/// The drain answers a relayed call busy; the host retries it after the restart.
pub(crate) fn call(domain: &mut Domain, to: ReplyTo) {
    let answer = domain.payloads.insert(Some(Payload::Busy)).expect("fleet call reserved answer room").token();
    domain.work.push(Work::Fleet(fleet::Event::Relayed { to, answer }));
}
