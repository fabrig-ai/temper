//! Pure, typed translation from the engine's owned run and call vocabulary
//! into Smith's generic agent boundary (jig's domain/hosts.md, sections 5 and 8;
//! jig's domain/engine.md, section 7; smith's domain/run.md, sections 3, 5 and 10).
//!
//! Jig's core keeps task policy and numbered calls; the root wraps records and
//! orders their commits. This crate keeps no state and performs no IO. Its `start` takes
//! already decoded concrete history and already prepared workspace mounts from
//! the composing world; a later protocol adapter supplies the same values from
//! bytes and worker preparation. It never chooses authority or commits a call.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;

mod answers;
mod calls;
mod docs;
mod finish;
mod forge;
mod json;
mod nested;
mod notes;
mod reference;
mod settings;
mod start;

pub use answers::answer;
pub use calls::{call, call_configured, tools};
pub use finish::{ChangeResource, Terminal, result, turn};
pub use reference::reference;
pub use settings::{settings_answer, settings_text};
pub use start::{charter, model, section, start};

fn decimal(bytes: &[u8], member: &'static [u8]) -> Result<u64, Problem> {
    if bytes.is_empty() {
        return Err(Problem::Range(member));
    }
    let mut value = 0_u64;
    for byte in bytes {
        if !byte.is_ascii_digit() {
            return Err(Problem::Range(member));
        }
        value = value.checked_mul(10).ok_or(Problem::Range(member))?;
        value = value.checked_add(u64::from(byte.saturating_sub(b'0'))).ok_or(Problem::Range(member))?;
    }
    Ok(value)
}

/// What a host call's input decodes to: a call for the root to decide,
/// or its answer, made here at decode, deciding nothing
/// (protocol/documentation.md, section 5).
#[derive(Debug)]
#[expect(clippy::large_enum_variant, reason = "the stateless decode boundary moves the engine's bounded call whole")]
pub enum Decoded {
    Call(temper_engine_domain::Call),
    Answered(smith_domain_run::HostAnswer),
}

/// The most a call answered at decode holds, as the root counts a
/// settled call (jig's `SettledCall::owned_bytes`: its tool and body);
/// the start refuses a smaller answer bound
/// (protocol/documentation.md, section 5).
pub const DOC_ANSWER: u32 = 2_048;

/// Smith channel v5's `CallName`: fixed-width activation (u64), completion
/// (u32), position (u32), with no record prefix or tail.
pub const CALL_NAME_BYTES: u32 = 16;

/// A host call that cannot be translated into the engine's declared schema.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Problem {
    Malformed,
    TooLarge,
    Missing(&'static [u8]),
    Type(&'static [u8]),
    Range(&'static [u8]),
    UnknownTool,
}

#[cfg(test)]
mod tests;
