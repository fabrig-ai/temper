//! The calendar says when; the core's policy supplies the budget (domain/root.md, 5.7).
use crate::{Domain, Limits, boundary::Work};
use skein_lib::{Env, Queue};
use temper_engine_domain_calendar as calendar;

/// A first source already supplies its budget; decorate that same event.
/// Calendar renewals always take the policy's spend at this decision.
pub(crate) fn route(domain: &mut Domain, env: &Env<Limits>, event: calendar::Event, source_budget: Option<u64>) {
    let limits = calendar::Limits { projects: env.limits.authority.projects };
    let mut out = Queue::with_capacity(calendar::max_out(&limits));
    calendar::step(&mut domain.calendar, &Env { now: env.now, wall: env.wall, limits }, event, &mut out);
    while let Some(request) = out.pop() {
        let (project, period, next, supplied) = match request {
            calendar::Request::Next { project, period, next } => (project, period, next, source_budget),
            calendar::Request::Open { project, period, next } => (project, period, next, None),
        };
        if let Some(policy) = domain.core.authority.policy(project) {
            let budget = supplied.unwrap_or(policy.period_spend);
            domain.work.push(Work::Core(jig_domain_core::Event::Period { project, period, budget, next: Some(next) }));
        }
    }
}
