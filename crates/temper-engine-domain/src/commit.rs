//! Commit spelling shared by change results and host answers.
use alloc::boxed::Box;
use skein_lib::List;

/// Render an adapter commit using ctl's existing width rule: a zero tail
/// after byte 20 denotes SHA-1; otherwise use all 32 bytes. Neither the
/// adapter commit nor the adopted repository retains an object format.
#[must_use]
pub fn commit_text(commit: &skein_forge::api::Commit) -> Box<[u8]> {
    let mut length = 20;
    for byte in &commit[20..] {
        if *byte != 0 {
            length = 32;
            break;
        }
    }
    let mut out = List::with_capacity(64);
    for byte in commit.get(..length).expect("a commit has 20 or 32 bytes") {
        for digit in temper_engine_domain_forge::hex(*byte) {
            out.push(digit).expect("commit digits fit");
        }
    }
    out.into_boxed()
}
