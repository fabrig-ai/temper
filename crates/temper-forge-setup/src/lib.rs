//! Forge setup, checked before any command acts (protocol/setup.md, 8 and 11).
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;
use alloc::boxed::Box;
pub mod check;
pub mod fetch;
pub mod given;
pub mod ini;
pub mod limits;
pub mod name;
pub mod parent;
pub mod place;
pub mod prepare;
pub mod problem;
pub mod run;
pub mod runner;
pub mod setup;
pub mod sign_in;
pub mod spec;
pub mod state;
pub mod step;
pub mod upgrade;
pub use setup::Setup;
pub use step::{Group, Kind, Stage, Step, Steps, steps};
#[cfg(test)]
mod tests;

/// The first repository's starter CI (protocol/setup.md, 6.7).
pub const STARTER_PATH: &[u8] = b".forgejo/workflows/ci.yml";
pub const STARTER_WORKFLOW: &[u8] = b"on:\n  push:\n    branches: [main]\n  pull_request:\njobs:\n  check:\n    runs-on: LABEL\n    steps:\n      - run: echo \"temper's starter check passed; replace this step with your own checks\"\n";

/// Render only a wire-sized runner label; its alphabet cannot add YAML syntax.
#[must_use]
pub fn starter_workflow(label: &[u8]) -> Option<Box<[u8]>> {
    if label.is_empty() || label.len() > 64 {
        return None;
    }
    for byte in label {
        if !byte.is_ascii_alphanumeric() && !b"._-".contains(byte) {
            return None;
        }
    }
    let at = skein_lib::bytes::find(STARTER_WORKFLOW, b"LABEL").expect("starter template label");
    let after = at.checked_add(5).expect("template offset");
    let size = STARTER_WORKFLOW.len().checked_sub(5)?.checked_add(label.len())?;
    let mut writer = skein_lib::Writer::new(size);
    writer.put(STARTER_WORKFLOW.get(..at)?).ok()?;
    writer.put(label).ok()?;
    writer.put(STARTER_WORKFLOW.get(after..)?).ok()?;
    Some(writer.finish())
}

/// The playground source shared by its fake seed and its owned copy (protocol/setup.md, 6.7).
pub const PLAYGROUND_PATH: &[u8] = b"src/main.rs";
/// Widget prints the playground table.
pub const PLAYGROUND_MAIN: &[u8] = b"const TABLE: [(&str, u32); 3] = [(\"apples\", 3), (\"pears\", 5), (\"plums\", 2)];\n\nfn main() {\n    for (name, count) in TABLE {\n        println!(\"{name:<8}{count}\");\n    }\n}\n";
