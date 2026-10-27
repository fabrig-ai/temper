//! The walking root with real children, an ordered paged fake store, scripted
//! person and worker, independent durable-delivery referee and restart cuts.
/// The protocol layer the world plays: link records, tool decode and render, outcomes, accounts.
pub mod protocol;
pub mod store;
pub mod walking;
pub mod walking_referee;

/// Real held-chat escalation driver.
pub mod escalation;
/// Independent escalation obligations.
pub mod escalation_referee;

/// Authenticated role-change driver over real held-chat records.
pub mod roles;
/// Independent role-cohort and recipient evidence.
pub mod roles_referee;

/// Shared direct root route driver for focused and randomized scenarios.
pub mod direct;
pub mod fixtures;
/// Requester-backed forge change fixture and delayed required-read control.
pub mod forge_routes;
pub mod forge_stand;

/// Take ready journal output, then admit one owner continuation if none was ready.
pub fn poll(
    root: &mut temper_engine_domain::Domain,
    env: &skein_lib::Env<temper_engine_domain::Limits>,
    out: &mut skein_lib::Queue<temper_engine_domain::Request>,
) {
    temper_engine_domain::release(root, env, out);
    if out.is_empty() {
        temper_engine_domain::release(root, env, out);
    }
    if out.is_empty() {
        temper_engine_domain::step(root, env, temper_engine_domain::Event::Resume);
        temper_engine_domain::release(root, env, out);
    }
}

pub mod tier_runs;
