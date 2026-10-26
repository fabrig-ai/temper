//! Hold the core’s stop while sent forge entries finish (domain/root.md, 9.2).
use crate::boundary::{Request, Unsettled, Work};
use crate::domain::{Domain, now};
use alloc::boxed::Box;
use jig_domain_core as core;
use temper_engine_domain_forge as forge;

pub(crate) fn start(domain: &mut Domain) {
    if domain.settling || domain.core.stopping() {
        return;
    }
    domain.settling = true;
    domain.work.push(Work::Forge(forge::Event::Settle));
}
pub(crate) fn finished(domain: &mut Domain, unfinished: Box<[forge::Unfinished]>) {
    if !domain.settling {
        return;
    }
    for write in unfinished {
        assert!(now(domain, Request::Unsettled(Unsettled { entry: write.entry, task: write.task })), "stop report door reserved");
    }
    domain.settling = false;
    domain.work.push(Work::Core(core::Event::Stopping));
}
