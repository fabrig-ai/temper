//! Stable names for requests (contract.md, 3.1).
use alloc::boxed::Box;
use skein_lib::{RequestKey, sha256::Sha256};
/// One to 64 lower-case letters, digits or hyphens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Name {
    bytes: Box<[u8]>,
}
impl Name {
    #[must_use]
    pub fn new(bytes: &[u8]) -> Option<Self> {
        if bytes.is_empty() || bytes.len() > 64 {
            return None;
        }
        for byte in bytes {
            if !byte.is_ascii_lowercase() && !byte.is_ascii_digit() && *byte != b'-' {
                return None;
            }
        }
        Some(Self { bytes: Box::from(bytes) })
    }
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
/// First sixteen bytes of the salt, zero separator and name's digest.
#[must_use]
pub fn named(salt: &[u8], name: &Name) -> RequestKey {
    let mut hash = Sha256::new();
    hash.update(salt);
    hash.update(&[0]);
    hash.update(name.bytes());
    let digest = hash.finish();
    let mut key = [0; 16];
    for (slot, byte) in key.iter_mut().zip(&digest) {
        *slot = *byte;
    }
    key
}

/// Local requests partition their numbers by the root's reserved call prefix.
#[must_use]
pub(crate) fn local(asker: jig_client::Asker, call: u32) -> jig_client::Key {
    jig_client::Key { asker, number: (u64::from(call) << 32u32) | 1 }
}
pub(crate) fn reserve(next: &mut Option<u32>, key: jig_client::Key) {
    let prefix = u32::try_from(key.number >> 32u32).expect("high half fits");
    if let Some(current) = *next
        && prefix >= current
    {
        *next = prefix.checked_add(1);
    }
}
