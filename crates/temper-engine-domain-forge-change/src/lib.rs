//! Pure change procedure and landing queue policy (domain/forge.md, sections 8–10).
//!
//! This crate keeps no state: the connector top owns and commits every
//! `Change`. It never knows a task or a forge API call. `step` reads a coherent
//! snapshot and returns the next state and one decision; the top checks a
//! requested effect with authority and saves its state with the request.
//! The procedure is level-triggered: a repeated snapshot yields the same
//! decision, while a committed pending state asks for no duplicate action.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

mod boundary;
mod domain;
mod queue;
#[cfg(test)]
mod tests;

pub use boundary::{
    Ask, AuthorityHold, AuthorityLack, AuthorityRecovery, Change, Ci, Decision, Delegate, DelegateSpend, Effect,
    EffectResult, Facts, Freshness, Gate, GateFailure, GateKind, GateReport, Heard, Hold, Limits, Pull, Ready, Repair,
    State, Status, Stepped, Way,
};
pub use domain::{authority_words, released, review_failed, step};
pub use queue::first;
