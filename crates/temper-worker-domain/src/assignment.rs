//! An assignment's workspace items, checked at the entrance
//! (`domain/worker.md`, 4.1): what the checkout cannot take is refused as an
//! invalid start, in words that name the item (jig's `domain/hosts.md`, 6.3).
//! The hub checks the rest of the assignment: its charter, conversation,
//! inbox and grants. The repositories' names are each the directory a
//! repository sits in, so one path component unique within its workspace;
//! the root relies on the workspace fitting the limits once it has passed.

use temper_worker_domain_checkout as checkout;

use crate::Limits;
use crate::wire::{Access, Repository, Start, Workspace};

/// Whether the workspace `items` and the saved-work branch `save` fit
/// `limits`, and the words for what does not.
pub(crate) fn check(items: &Workspace, save: Option<&[u8]>, limits: &Limits) -> Result<(), &'static [u8]> {
    if let Some(branch) = save
        && !name(branch, limits)
    {
        return Err(b"save branch");
    }
    let count = u64::try_from(items.repositories.len()).expect("a length fits in a u64");
    if count > u64::from(limits.checkout.repositories) {
        return Err(b"too many repositories");
    }
    for repository in &items.repositories {
        self::repository(repository, limits)?;
    }
    // Bounded: the repositories are within the limits, checked above.
    for (index, repository) in items.repositories.iter().enumerate() {
        for other in items.repositories.iter().skip(index.saturating_add(1)) {
            if other.name == repository.name {
                return Err(b"repository named twice");
            }
        }
    }
    Ok(())
}

fn repository(repository: &Repository, limits: &Limits) -> Result<(), &'static [u8]> {
    if !name(&repository.name, limits) || !component(&repository.name) {
        return Err(b"repository name");
    }
    if !name(&repository.remote, limits) {
        return Err(b"remote");
    }
    if let Some(program) = &repository.git
        && (!program.starts_with(b"/") || len(program) > u64::from(checkout::PROGRAM_BYTES) || program.contains(&0))
    {
        return Err(b"repository git");
    }
    match &repository.start {
        Start::Base { branch } | Start::Branch { branch } | Start::Saved { branch } => {
            if !name(branch, limits) {
                return Err(b"branch");
            }
        }
        Start::Commit { .. } => {}
        Start::Merge { branch, .. } => {
            if limits.checkout.conflicts == 0 {
                return Err(b"merge in progress");
            }
            if !name(branch, limits) {
                return Err(b"branch");
            }
        }
    }
    match &repository.access {
        Access::ReadOnly => Ok(()),
        Access::Writable { push, landing, .. } => {
            if let Some(branch) = landing
                && !name(branch, limits)
            {
                return Err(b"landing branch");
            }
            if name(push, limits) { Ok(()) } else { Err(b"push branch") }
        }
    }
}

/// A name is at least a byte, and at most the limit.
fn name(bytes: &[u8], limits: &Limits) -> bool {
    !bytes.is_empty() && len(bytes) <= u64::from(limits.checkout.name_bytes)
}

/// A repository's name is the directory it sits in: one path component, which
/// names nothing else.
fn component(name: &[u8]) -> bool {
    let special = name == b"." || name == b".." || name.eq_ignore_ascii_case(b".git");
    !(special || name.contains(&b'/') || name.contains(&0))
}

/// The length of `bytes`.
pub(crate) fn len(bytes: &[u8]) -> u64 {
    u64::try_from(bytes.len()).expect("a length fits in a u64")
}
