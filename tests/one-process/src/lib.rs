//! The actual service, native client and hosted programs on the crashing machine.
pub mod fixtures;
mod machine;
pub mod referee;
mod stories;
pub mod world;

mod ctl;
#[path = "../../real-loop/src/settings.rs"]
mod settings;

pub mod deployment;
