//! Pure landing-by-turn policy (domain/landing.md, sections 1–10).
//!
//! This crate keeps nothing. The connector top owns each landing, receipts,
//! keys in flight and effects. A step returns one decision and the state to
//! commit with it. Keys, run verdicts and turn order are pure functions too.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

mod boundary;
mod domain;
mod key;
#[cfg(test)]
mod tests;
mod turn;
mod verdict;

pub use boundary::{
    CHECK_NAME_BYTES, Commit, Decision, Facts, Flight, Gate, GateFacts, GateProof, Hold, Key, Landing, Limits, Member,
    MemberFacts, Phase, Posted, Posting, Project, Proof, Repository, Status, Stepped, WORKFLOW_BYTES,
};
pub use domain::step;
pub use key::{Entry, EntryKind, KeyLimits, Keyed, Tree, definition, key};
pub use turn::{Proved, first};
pub use verdict::{Conclusion, Fault, Job, Run, RunState, Verdict, verdict};
