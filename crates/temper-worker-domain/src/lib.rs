//! The domain layer of the temper worker (programming-model.md, section 4;
//! `domain/worker.md`, section 3): the worker loop's entry point.
//!
//! Sans-io: [`step`], [`fire`] and [`resume`] turn events into requests and
//! change nothing but the [`Domain`] they are given. Time and randomness are
//! inputs; every effect, from spawning an agent to running git, is a
//! [`Request`] the layers below carry out, and its outcome comes back later
//! as an [`Event`].
//!
//! It is the root domain over a tree of child domains (4.5), and the only one
//! that faces the protocol layer, shaped as jig's reference worker root
//! (jig's `domain/hosts.md`, section 4):
//!
//! ```text
//! temper-worker-domain                 faces the protocol; the hub's link; routes; translates
//! ├── jig-domain-host                  jig's hub: hosted runs, admit, prepare, start, relay, park or end
//! ├── temper-worker-domain-checkout    workspaces: prepare, commit, push, save; the cache
//! └── smith-host-domain                agent processes: spawn, channel, watchdog, cancel then kill
//! ```
//!
//! It owns its children's state and routes each event to the child it is for
//! and each child's requests out: the engine's records to and from the hub,
//! the git and file io to and from the checkout, the agent processes' io to
//! and from smith's host domain. The hub owns the engine link's vocabulary:
//! the hello, assignments, messages, relays, turns and answers, which it
//! keeps until the engine acknowledges them; the link's mechanics (dialling,
//! the grace, relays across channels) are jig's worker link, in the protocol
//! layer, which tells the domain what it notices. The checkout and smith's
//! host domain are the hub's capabilities; the three share no types, so the
//! root translates between them through small total functions, keeping for
//! each workspace what each side names it by, and temper's own parts: the
//! workspace items down, what a run left on the forge up, beside jig's
//! records. smith's payloads (the charter, the transcript, turns, call names,
//! results, facts) are the protocol layer's to decode and encode, by
//! hand-out (6.8); the root links no codec of smith's. The hand-offs are
//! short and acyclic, so each entry point completes them before it returns
//! (the `domain` module).
//!
//! What happens is also told as content-free [`Fact`]s, the child domains',
//! gathered into one bounded queue the loop drains ([`Domain::pop_fact`]).
//! What a run tells of itself goes to the engine as jig's activity, through
//! the hub.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

mod admission;
mod agents;
mod assignment;
mod boundary;
mod domain;
mod facts;
mod hand_out;
mod limits;
mod push;
mod route;
mod smith_delivery;
#[cfg(test)]
mod tests;
mod translate;
pub mod wire;
mod workspace;

pub use admission::{admits, is_new_work, passes};
pub use boundary::{Encodable, Encoded, Event, Invalid, Request};
pub use domain::{Domain, fire, resume, step};
pub use facts::Fact;
pub use limits::{Limits, SEPARATOR, Unfit, WATERMARK, unfit, valid};
// The payloads are the children's: a parent may use its children's types.
pub use jig_domain_host as host;
pub use smith_host_domain as agent;
pub use temper_worker_domain_checkout as checkout;
