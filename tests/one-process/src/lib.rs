//! The actual service, native client and hosted programs on the crashing machine.
mod box_listener;
pub mod fixtures;
mod machine;
pub mod referee;
mod restart_control;
mod stories;
pub mod world;

mod ctl;
#[path = "../../real-loop/src/settings.rs"]
mod settings;
