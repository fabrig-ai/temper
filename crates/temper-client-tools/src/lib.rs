//! Agent tools as bounded values (client/tools.md, 4).
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
pub mod adopt;
mod amend;
mod change;
mod demo;
mod forge;
mod forges;
pub mod jig;
pub mod key;
mod landing;
pub mod limits;
pub mod root;
pub mod table;
#[cfg(test)]
mod tests;

pub mod help;
