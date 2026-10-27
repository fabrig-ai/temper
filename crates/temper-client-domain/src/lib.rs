//! Keep: the child/table routing shape and exhaustive owner variants.
//! Replace: Ops connector sides with the forge's side and temper's navigation.
//! The children keep visibility and pending acts. The root retains only
//! those children and the shared client (jig's client/domain/root.md, 3–10).
//! Each step routes one event; take reclaims after routing every tell.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
pub mod acts;
pub mod address;
pub mod amend;
pub mod authorize;
pub mod boundary;
mod departure;
pub mod domain;
pub mod forge;
pub mod key;
pub mod limits;
pub mod owner;
pub mod pages;
mod route;
pub mod sign_in;
pub mod words;
pub use address::{Address, ChatsFilter, Choice, ModelWay, Place, PlanFilter, Step, WayKind};
pub use boundary::{Action, Event, Fact, Request};
pub use domain::{Domain, fire, new, new_departing, next_deadline, shown, step, take, unlinked, unlinked_departing};
pub use limits::{Limits, max_out, worst_case};
#[cfg(test)]
mod tests;
