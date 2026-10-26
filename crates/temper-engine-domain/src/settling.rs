//! Hold the core’s stop while sent forge entries finish (domain/root.md, 9.2).
use crate::boundary::{Request, Unsettled, Work};
use crate::domain::{Domain, now};
use alloc::boxed::Box;
use jig_domain_core as core;
use temper_engine_domain_forge as forge;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Phase {
    Running,
    Waiting,
}
impl Phase {
    pub(crate) fn active(self) -> bool {
        self == Self::Waiting
    }
}

pub(crate) fn start(domain: &mut Domain) {
    if domain.settling.active() || domain.core.stopping() {
        return;
    }
    domain.settling = Phase::Waiting;
    domain.work.push(Work::Forge(forge::Event::Settle));
}
pub(crate) fn finished(domain: &mut Domain, unfinished: Box<[forge::Unfinished]>) {
    if !domain.settling.active() {
        return;
    }
    for write in unfinished {
        assert!(
            now(domain, Request::Unsettled(Unsettled { entry: write.entry, task: write.task })),
            "stop report door reserved"
        );
    }
    domain.settling = Phase::Running;
    domain.work.push(Work::Core(core::Event::Stopping));
}
