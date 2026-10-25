//! temper's application parts (jig's `protocol/channel.md`, 6.2): what the
//! engine gives a run's workspace, beside jig's assignment, and what a run
//! left on the forge, beside jig's answer. In the domain they travel typed
//! beside jig's records; on the channel they are temper's bytes.

pub use crate::push::{PushDiagnostic, PushFailure, PushReason};
use alloc::boxed::Box;

/// Workspace items whose channel bytes did not decode (protocol/channel.md, 7).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Undecodable;

/// The repositories a run works in (`domain/worker.md`, 4.1). The checkout
/// that holds them is keyed by jig's workstream, the holding task's number;
/// an empty list means the run has no workspace items, so no checkout is
/// prepared or held.
#[derive(PartialEq, Eq, Hash, Debug)]
pub struct Workspace {
    pub repositories: Box<[Repository]>,
}

/// A repository of a workspace. Names are compared byte for byte.
#[derive(PartialEq, Eq, Hash, Debug)]
pub struct Repository {
    /// Deployment repository name, echoed in landed work.
    pub tag: u64,
    /// The directory it sits in, side by side with the workspace's others
    /// (`domain/worker.md`, section 5), and what the agent calls it: one path
    /// component, unique within the workspace.
    pub name: Box<[u8]>,
    /// The forge's address of the repository, never interpreted: the
    /// protocol layer maps it to a URL.
    pub remote: Box<[u8]>,
    /// The item's program, carried to checkout and never read.
    pub git: Option<Box<[u8]>>,
    pub start: Start,
    pub access: Access,
    /// Who the worker is to the forge for this repository, to read it and to
    /// push to it, and who commits to it: one of temper's application
    /// credentials, granted with the assignment, which the protocol layer
    /// maps to credentials and an author.
    pub identity: u32,
    /// The forge account whose entry of the identity's value authenticates git
    /// on this repository (protocol/channel.md, section 4); copied, never read.
    pub account: Option<u64>,
}

/// Where a repository's checkout starts.
#[derive(PartialEq, Eq, Hash, Debug)]
pub enum Start {
    Merge {
        branch: Box<[u8]>,
        base: [u8; 32],
    },
    /// A base branch, made from the default branch if it does not exist yet.
    Base {
        branch: Box<[u8]>,
    },
    Branch {
        branch: Box<[u8]>,
    },
    /// A commit, by its object id: the full id in 32 bytes, as the protocol
    /// layer makes it of git's (a SHA-256 id as it is, a SHA-1 id followed by
    /// twelve zero bytes), compared and never looked inside.
    Commit {
        commit: [u8; 32],
    },
    /// Saved work, on the saved-work branch `branch`.
    Saved {
        branch: Box<[u8]>,
    },
}

/// Whether a repository may be written.
#[derive(PartialEq, Eq, Hash, Debug)]
pub enum Access {
    /// Writable branch, fenced to the expected previous head.
    Writable {
        push: Box<[u8]>,
        expected: Option<[u8; 32]>,
        landing: Option<Box<[u8]>>,
    },
    ReadOnly,
}

/// What a run is told of a push.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[expect(clippy::large_enum_variant, reason = "bounded diagnostics stay inline and are included in worst_case")]
pub enum Push {
    Conflicted {
        repository: u32,
        files: Box<[Box<[u8]>]>,
    },
    Done,
    /// A branch moved since the run started: no change of the run can land
    /// there.
    Moved,
    /// A push failed, and none moved.
    Failed {
        failure: PushFailure,
    },
    /// No repository had a change.
    Nothing,
}

/// What became of one repository in a push or a save.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[expect(clippy::large_enum_variant, reason = "bounded diagnostics stay inline and are included in worst_case")]
pub enum Landing {
    Conflicted {
        files: Box<[Box<[u8]>]>,
    },
    /// A failed invocation, with its typed reason and bounded diagnostics.
    Explained {
        failure: PushFailure,
    },
    /// Its change is on the branch, as `commit`, by its object id (as
    /// [`Start::Commit`]'s).
    Landed {
        commit: [u8; 32],
    },
    /// The branch moved since the run started: nothing was pushed.
    Moved,
    /// The push failed, and did not land: a retry may succeed.
    Failed,
    /// The forge refused the push: the identity's permissions, a protected
    /// branch, a hook. A retry will not do better.
    Refused,
    /// It had no change.
    Unchanged,
}

/// What a run left on the forge: the repositories its pushes landed in, by
/// their deployment tags, ascending, each with the last commit landed
/// there (the head a pull request is to show); and its save, if one was made,
/// each repository's outcome in the assignment's order.
#[derive(PartialEq, Eq, Hash, Debug)]
pub struct Work {
    pub landed: Box<[Landed]>,
    pub saved: Option<Box<[SavedRepository]>>,
}

/// One repository's save, named by tag with how its checkout started.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct SavedRepository {
    pub tag: u64,
    pub from_saved: bool,
    pub landing: Landing,
}

/// A repository a run's pushes landed in: its deployment tag, and
/// the last commit landed there.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Landed {
    pub tag: u64,
    pub commit: [u8; 32],
}

/// Why a workspace could not be prepared, in temper's detail: what jig's
/// answer says as transient or permanent, naming the repository.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Preparation {
    /// The forge could not be reached, something failed on the worker's
    /// side, an operation ran out of time, or the workspace is held by
    /// another run of its workstream: a retry may work.
    Transient,
    /// The forge does not have what the assignment names for the repository
    /// named by `tag`: permanent until someone makes it.
    Missing { tag: u64, missing: Missing },
    /// The forge refused the identity of the repository named by `tag`:
    /// permanent until the identity's credentials or permissions change.
    Refused { tag: u64 },
}

/// What the forge does not have.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Missing {
    Repository,
    /// A branch to start from, or the default branch a base branch is to be
    /// made from.
    Branch,
    Commit,
}
