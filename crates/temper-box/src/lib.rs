//! The box's steps, restarts and ordered shutdown (protocol/setup.md, 7).
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
pub mod host;
pub mod layout;
pub mod limits;
pub mod lines;
mod logs;
pub mod machine;
pub mod pipe;
mod ready;
mod records;
pub mod sink;
pub mod terminal;
#[cfg(test)]
mod tests;
