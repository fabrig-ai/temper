//! Top-owned state and coherent current facts (domain/landing.md, 1–10).
use alloc::boxed::Box;
use skein_lib::{Duration, Wall};

use crate::Verdict;

/// A forge object id: 32 bytes, a SHA-1 one padded.
pub type Commit = [u8; 32];
/// A gate's key (domain/landing.md, 6.1).
pub type Key = [u8; 32];
/// Bound of a branch-name-safe gate or after-landing tier name.
pub const CHECK_NAME_BYTES: usize = 32;
/// Bound of a workflow file name.
pub const WORKFLOW_BYTES: usize = 255;

/// A repository's forge number and forge id.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Repository {
    pub forge: u16,
    pub repository: u32,
}

/// A repository's test gate (domain/landing.md, 13.1).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Gate {
    /// Unique branch-name-safe final segment of `temper/gate/<name>`.
    pub name: Box<[u8]>,
    /// The workflow file as named by dispatch.
    pub workflow: Box<[u8]>,
    /// Path prefixes of whole segments, never patterns.
    pub ignored: Box<[Box<[u8]>]>,
    pub blocking: bool,
}

/// The project's landing settings needed by the policy.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Project {
    pub gate_retries: u32,
}

/// Whether a fact holds, as for a change.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Status {
    Unknown,
    Pending,
    Passed,
    Failed,
}

/// A landing's committed place (domain/landing.md, 3, 5–8).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Phase {
    Waiting,
    Preparing,
    Proving,
    Proved { at: Wall },
    Turn { at: Wall },
    Landed,
    Held { was: Box<Phase>, why: Hold },
    Cancelled,
}

/// Why a release is needed (domain/landing.md, 7.4).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Hold {
    Faults { member: u16, gate: u16 },
}

/// One gate's proof at a member's candidate (domain/landing.md, 7).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Proof {
    Unkeyed,
    Keyed { key: Key },
    Dispatched { key: Key, try_: u16 },
    Passed { key: Key },
    Failed { key: Key },
}

/// What this landing has asked to post on the candidate.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Posted {
    Nothing,
    Pending,
    Verdict,
}

/// One gate's proof and posting, owned by the top.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct GateProof {
    pub proof: Proof,
    pub posted: Posted,
    /// Runner faults for this key, against `gate_retries`.
    pub faults: u32,
}

/// One change into one repository (domain/landing.md, 4.1).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Member {
    pub change: u64,
    pub repository: Repository,
    pub pull: u64,
    /// Head and main from which the candidate was prepared.
    pub head: Option<Commit>,
    pub base: Option<Commit>,
    pub candidate: Option<Commit>,
    /// One per repository gate, in configuration order.
    pub gates: Box<[GateProof]>,
    /// Preparation last asked of the change.
    pub asked: Option<u16>,
    /// Main's commit once landed: the candidate.
    pub landed: Option<Commit>,
}

/// The top-owned landing task record (domain/landing.md, 12).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Landing {
    pub task: u64,
    pub project: u32,
    pub phase: Phase,
    pub members: Box<[Member]>,
    /// Candidates' preparation, counted from 1.
    pub preparation: u16,
    pub stale_proofs: u32,
}

/// Current dispatch of this key. The top follows the current try, clearing
/// its old run when committing a retry (domain/landing.md, 7.4).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Flight {
    None,
    Mine { run: Option<Verdict> },
    Other,
}

/// Current facts for the gate at this candidate.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct GateFacts {
    /// From the repository's configured gate at this position.
    pub blocking: bool,
    pub key: Option<Key>,
    /// A receipt of this key: Passed or Failed, otherwise Unknown.
    pub receipt: Status,
    pub flight: Flight,
    /// Status of this landing's most recently requested posting effect.
    pub posting: Status,
}

/// Fresh facts for one member, in the landing's member order.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct MemberFacts {
    pub ready: bool,
    pub cancelled: bool,
    pub head: Commit,
    pub main: Commit,
    /// Whether head contains main.
    pub contains_main: Status,
    pub asked: bool,
    /// This landing's own merge effect: Unknown until asked, then Pending,
    /// Passed when made or Failed when refused.
    pub merge: Status,
    pub merged: Option<Commit>,
    pub gates: Box<[GateFacts]>,
}

/// The coherent snapshot supplied by the top.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Facts {
    pub members: Box<[MemberFacts]>,
    pub turn_free: bool,
    pub first: bool,
    pub released: bool,
}

/// The candidate's gate status (domain/landing.md, 10).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Posting {
    Pending,
    Ran { passed: bool },
    Received,
}

/// One thing due in this snapshot, as for a change.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Decision {
    None,
    Wait { until: Option<Wall> },
    Prepare { member: u16, head: Commit, main: Commit },
    Key { member: u16, gate: u16, candidate: Commit },
    Dispatch { member: u16, gate: u16, key: Key, try_: u16 },
    Retry { member: u16, gate: u16, key: Key, try_: u16 },
    Post { member: u16, gate: u16, posting: Posting },
    Repair { member: u16, gate: u16 },
    Proved,
    Merge { member: u16, candidate: Commit },
    Stale,
    Finish,
    Cancel,
    Hold(Hold),
}

/// State to commit atomically with the decision.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Stepped {
    pub landing: Landing,
    pub decision: Decision,
}

/// Admission bounds and the run polling interval.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Limits {
    pub members: u32,
    pub gates: u32,
    pub poll: Duration,
}
