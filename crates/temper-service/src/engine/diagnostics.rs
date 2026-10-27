//! Read-only readiness detail; normal engine passes never construct this snapshot.
use super::{Engine, root};
use crate::diagnostics::{self, EngineEvent};
use skein_lib::{List, Time};

impl Engine {
    #[expect(clippy::manual_map, reason = "the step subset uses explicit matches")]
    pub(crate) fn pending(&self, now: Time) -> diagnostics::Engine {
        let active = self.store.opened || self.end.is_some();
        let next_deadline = self.next_deadline();
        let mut events = List::with_capacity(self.events.len());
        for event in &self.events {
            events.push(event_tag(event)).expect("one tag per queued event");
        }
        diagnostics::Engine {
            work_pending: self.work_pending(now),
            clients: active && self.clients.work_pending(now),
            setup: active && self.setup.work_pending(now),
            setup_lifecycle: self.setup.lifecycle_ready(self)
                && self.setup.room()
                && self.store.room()
                && self.forge.room(2)
                && self.events.room() >= 4,
            store: self.store.work_pending(now),
            forge: active && self.forge.work_pending(now),
            workers: active && self.workers.work_pending(now),
            accounts: active
                && match &self.accounts {
                    Some(accounts) => accounts.work_pending(now),
                    None => false,
                },
            store_opened: self.store.opened,
            ended: self.end.is_some(),
            room: self.room(),
            begin: self.begin,
            ready: self.ready,
            domain_ready: self.domain.ready(),
            release_pending: self.release_pending(),
            blocked: self.blocked,
            pending: match &self.pending {
                Some(event) => Some(event_tag(event)),
                None => None,
            },
            events,
            due: match next_deadline {
                Some(at) => at <= now,
                None => false,
            },
            first_periods: self.first_periods.len(),
            settled_revision: self.settled_revision,
            revision: self.domain.revision(),
            next_deadline,
            requests: self.requests.len(),
            workers_opened: self.workers.opened(),
            clients_opened: self.clients.opened(),
        }
    }
}

fn event_tag(event: &root::Event) -> EngineEvent {
    match event {
        root::Event::Setup { .. } => EngineEvent::Setup,
        root::Event::Store(_) => EngineEvent::Store,
        root::Event::Worker(_) => EngineEvent::Worker,
        root::Event::Party(_) => EngineEvent::Party,
        root::Event::Account(_) => EngineEvent::Account,
        root::Event::Credential { .. } => EngineEvent::Credential,
        root::Event::Forge(_) => EngineEvent::Forge,
        root::Event::Released(_) => EngineEvent::Released,
        root::Event::Restart => EngineEvent::Restart,
        root::Event::Resume => EngineEvent::Resume,
        root::Event::Stop => EngineEvent::Stop,
    }
}
