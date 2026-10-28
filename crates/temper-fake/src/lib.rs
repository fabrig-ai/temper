//! The QA loop's two documents and their seeds (protocol/setup.md, 8.4).
//! Files and processes belong to the binary; these checks and plans do no io.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;

pub mod backends;
pub mod check;
pub mod credential;
pub mod demo;
pub mod forge;
pub mod limits;
pub mod llm;
pub mod oauth;
mod read;
pub mod steps;
pub mod turn;
mod wire;
pub use read::Refusal;
pub use steps::steps;

/// The whole JSON input bound, checked before decoding or creating any output.
pub const DOCUMENT_BYTES: u32 = 1 << 20;
/// Joint script payload and arrays, including eleven scripts each way and fallback.
pub const SCRIPT_BYTES: u32 = 1 << 18;
/// One answer's owned parts and bytes, before the provider encodes it.
pub const ANSWER_BYTES: u32 = 1 << 16;
/// Scripts per cue list, sized for rung A's first passing walk's eleven agents.
pub const ENTRIES: u32 = 11;
/// Turns per script, matching the stock coding agent and reviewer runs.
pub const SCRIPT_TURNS: u32 = 16;
/// Other document lists: models, lines and repositories.
pub const LIST_ENTRIES: u32 = 8;

#[cfg(test)]
mod tests;
