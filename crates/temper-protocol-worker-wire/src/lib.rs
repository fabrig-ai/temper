//! temper's parts on jig's channel (protocol/channel.md, sections 3 to 6 and 8).
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
extern crate alloc;

#[rustfmt::skip]
#[path = "generated/v1.rs"]
pub mod v1;

use alloc::boxed::Box;
use skein_lib::{Secret, Writer};

/// The application's channel payload version (protocol/channel.md, section 3).
pub const PAYLOAD_VERSION: u32 = 4;

/// Domain bounds shared by both sides of the channel (protocol/channel.md,
/// section 8).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Bounds {
    pub repositories: u32,
    pub name_bytes: u32,
    pub remote_bytes: u32,
    pub conflicts: u32,
    pub path_bytes: u32,
    pub token_bytes: u32,
    pub entries: u32,
    pub entry_token_bytes: u32,
    pub text_bytes: u32,
}

/// Derive every schema limit from its domain bound, refusing past a ceiling.
#[must_use]
pub fn limits(bounds: &Bounds) -> Option<v1::Limits> {
    let limits = v1::Limits {
        repository_name: bounds.name_bytes,
        merge_branch: bounds.name_bytes,
        branch_branch: bounds.name_bytes,
        writable_push: bounds.name_bytes,
        writable_landing_some: bounds.name_bytes,
        items_save_some: bounds.name_bytes,
        repository_remote: bounds.remote_bytes,
        repository_git_some: v1::CEILINGS.repository_git_some,
        items_repositories: bounds.repositories,
        left_landed: bounds.repositories,
        saved_repositories: bounds.repositories,
        files_files: bounds.conflicts,
        files_files_item: bounds.path_bytes,
        diagnostic_output: v1::CEILINGS.diagnostic_output,
        token_token: bounds.token_bytes,
        entries_entries: bounds.entries,
        entry_token: bounds.entry_token_bytes,
        entry_account_login: bounds.text_bytes,
        identity_login: bounds.text_bytes,
        identity_name: bounds.text_bytes,
        identity_email: bounds.text_bytes,
    };
    match v1::check_limits(&limits) {
        Ok(()) => Some(limits),
        Err(_) => None,
    }
}

/// Largest application parts as jig's worker wire sizes take them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Largest {
    pub items: u32,
    pub left: u32,
    pub saved: u32,
    pub resource: u32,
    pub grant: u32,
}

/// The generated worst cases, refused if a size does not fit a u32.
#[must_use]
pub fn largest(limits: &v1::Limits) -> Option<Largest> {
    Some(Largest {
        items: narrow(v1::Items::worst_case_bytes(limits)?)?,
        left: narrow(v1::Left::worst_case_bytes(limits)?)?,
        saved: narrow(v1::Saved::worst_case_bytes(limits)?)?,
        resource: narrow(v1::Preparation::worst_case_bytes(limits)?)?,
        grant: narrow(v1::Identity::worst_case_bytes(limits)?)?,
    })
}

fn narrow(bytes: u64) -> Option<u32> {
    u32::try_from(bytes).ok()
}

/// Encode Items into exactly its measured room.
#[must_use]
pub fn items(value: &v1::Items) -> Box<[u8]> {
    let mut writer = Writer::new(usize::try_from(value.measure()).expect("measured length fits usize"));
    value.encode(&mut writer).expect("measured room");
    writer.finish()
}

/// Encode Left into exactly its measured room.
#[must_use]
pub fn left(value: &v1::Left) -> Box<[u8]> {
    let mut writer = Writer::new(usize::try_from(value.measure()).expect("measured length fits usize"));
    value.encode(&mut writer).expect("measured room");
    writer.finish()
}

/// Encode Saved into exactly its measured room.
#[must_use]
pub fn saved(value: &v1::Saved) -> Box<[u8]> {
    let mut writer = Writer::new(usize::try_from(value.measure()).expect("measured length fits usize"));
    value.encode(&mut writer).expect("measured room");
    writer.finish()
}

/// Encode Preparation into exactly its measured room.
#[must_use]
pub fn preparation(value: &v1::Preparation) -> Box<[u8]> {
    let mut writer = Writer::new(usize::try_from(value.measure()).expect("measured length fits usize"));
    value.encode(&mut writer).expect("measured room");
    writer.finish()
}

/// Encode an identity directly into a secret (protocol/channel.md, section 6).
#[must_use]
pub fn identity(value: &v1::Identity) -> Secret {
    let mut writer = Writer::new(usize::try_from(value.measure()).expect("measured length fits usize"));
    value.encode(&mut writer).expect("measured room");
    Secret::new(writer.finish())
}

/// Check configuration's login and author rules (protocol/channel.md, section 6).
#[must_use]
pub fn valid(identity: &v1::Identity) -> bool {
    for byte in identity.login() {
        if *byte == b':' || byte.is_ascii_control() {
            return false;
        }
    }
    author(identity.name()) && author(identity.email())
}

fn author(text: &[u8]) -> bool {
    if text.is_empty() {
        return false;
    }
    for byte in text {
        if *byte == 0 || *byte == b'\n' || *byte == b'<' || *byte == b'>' {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests;
