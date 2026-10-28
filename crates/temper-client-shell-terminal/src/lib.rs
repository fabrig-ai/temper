//! temper's native client and terminal shell (client/README.md, section 2).
#![forbid(unsafe_code)]
#[cfg(test)]
extern crate self as temper_client_shell_terminal;
pub mod client;
pub mod diagnostics;
pub mod limits;
pub mod route;
pub mod terminal;
#[cfg(test)]
mod tests;
