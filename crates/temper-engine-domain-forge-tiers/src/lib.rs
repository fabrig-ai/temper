//! Pure tier verdict policy (domain/after-landing.md, sections 2–5).
//!
//! This crate keeps no state and knows no forge call: the connector top owns
//! the records, reads reports and receipts, and saves and dispatches each part.
//! The policy is level-triggered: the same record and observations give the
//! same decision. The top commits a decision before dispatching its part and
//! passes each completion with that committed run.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

mod boundary;
mod report;
#[cfg(test)]
mod tests;
mod verdict;

pub use boundary::{
    Blame, Commit, Failures, Finding, Flake, Format, Judged, Key, Landing, Limits, Next, Outcome, Part, Receipt,
    Record, Run, Selection, Settings, Suspects, TestId, Unreadable, Verdict,
};
pub use report::{failed, report};
pub use verdict::{due, judged, repaired, start, step};
