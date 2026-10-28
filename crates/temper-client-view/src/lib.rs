//! Temper's own pages on skein-ui and jig-client's documents.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
mod accounts;
pub mod cards;
mod choice;
mod confirm;
mod flow;
pub mod forge;
pub mod forge_sign_in;
pub mod frame;
pub mod limits;
pub mod look;
pub mod pages;
mod root;
pub mod sign_in;
pub mod words;
pub use limits::{Limits, valid};
pub use root::{View, act, build, event, href};

/// Open a bound link with its address, leaving its contents to the caller.
pub(crate) fn link(b: &mut skein_ui::Builder, name: &[u8], to: skein_ui::Binding) {
    b.open(skein_ui::Role::Link);
    b.name(name);
    b.bind(to);
    b.link(to);
}

#[cfg(test)]
mod tests;
