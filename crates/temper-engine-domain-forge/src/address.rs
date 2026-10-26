//! Pull addresses shared by landing words and change panels (client/forge.md, 9.2).
use alloc::boxed::Box;
use skein_lib::{Decimal, Writer};

/// A missing or oversized address leaves the pull request's words alone.
pub const PULL_ADDRESS_BYTES: u32 = 2048;

/// Build a pull address from public configuration, independent of wire or records.
#[must_use]
pub fn pull_address(
    kind: crate::Kind,
    git: &[u8],
    web: Option<&[u8]>,
    owner: &[u8],
    name: &[u8],
    number: u64,
) -> Option<Box<[u8]>> {
    let (mut base, route) = match kind {
        crate::Kind::GitHub => (web?, b"/pull/".as_slice()),
        crate::Kind::Forgejo => (git, b"/pulls/".as_slice()),
    };
    while base.last() == Some(&b'/') {
        base = base.get(..base.len().checked_sub(1)?)?;
    }
    if base.is_empty() {
        return None;
    }
    let digits = Decimal::of(number);
    let parts = [base, b"/", owner, b"/", name, route, digits.as_bytes()];
    let mut length = 0_usize;
    for part in parts {
        length = length.checked_add(part.len())?;
    }
    if length > usize::try_from(PULL_ADDRESS_BYTES).expect("u32 fits usize") {
        return None;
    }
    let mut writer = Writer::new(length);
    for part in parts {
        writer.put(part).ok()?;
    }
    Some(writer.finish())
}
