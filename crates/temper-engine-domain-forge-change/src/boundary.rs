//! Values owned by the connector top and passed to the change policy
//! (domain/forge.md, sections 8–10).
//!
//! The top keeps `Change`, including its pending phase, counters and clean
//! predecessor heads. This module keeps nothing and knows no API tokens.
//! `Facts` and `Heard` form a current snapshot, not a wake event.
use alloc::boxed::Box;
use skein_lib::{Duration, Wall};

/// A forge commit identifier, translated at the connector top.
pub type Commit = [u8; 32];

/// The status of a fact or requirement at the named head.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Status {
    /// No answer has been observed.
    Unknown,
    /// The answer remains in flight.
    Pending,
    /// The requirement holds.
    Passed,
    /// The requirement failed.
    Failed,
}

/// Whether a gate needs the current head or accepts clean predecessors.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Freshness {
    /// The verdict must name the current head.
    Exact,
    /// A verdict at a clean predecessor head remains valid.
    Clean,
}

/// How the top obtains a gate verdict.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum GateKind {
    /// An agent reviews the change.
    Agent,
    /// A person approves the change.
    Person,
    /// A configured check supplies a verdict.
    Check,
}

/// One gate the change or its project asks for.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Gate {
    pub number: u64,
    pub kind: GateKind,
    pub blocking: bool,
    pub freshness: Freshness,
    pub eager: bool,
}

/// A gate's current verdict and the head it names.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct GateReport {
    pub number: u64,
    pub head: Commit,
    pub status: Status,
}

/// Failures to obtain a gate's verdict at the change's last head.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct GateFailure {
    pub number: u64,
    pub count: u32,
}

/// Why a repair delegate is needed.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Repair {
    /// CI failed on the change's head.
    Ci,
    /// A blocking gate requested changes.
    Gate(u64),
    /// CI failed after a clean update from the base.
    Semantic,
    /// A landing's gate failed at its candidate (domain/landing.md, 7.3).
    Landing { gate: u16 },
}

/// The part of a change's authority its next delegate needs.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AuthorityLack {
    Tasks,
    Depth,
    Spend,
}

/// The amendment and release coordinates at the authority decision.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct AuthorityRecovery {
    pub task: u64,
    pub spec: u64,
    pub hold: u64,
    pub tasks: u64,
}

/// A precise, durable authority hold (domain/forge.md, 8.4).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct AuthorityHold {
    pub recovery: AuthorityRecovery,
    pub lack: AuthorityLack,
    pub had: u64,
    pub released: bool,
}

/// Why a change is held for its requester or a person.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Hold {
    /// A forge wait exceeded its stall bound.
    Stalled,
    /// Another repair would exceed the configured bound.
    Repairs,
    /// Another conflict resolution would exceed its bound.
    Resolutions,
    /// Another mechanical update would exceed its bound.
    Updates,
    /// The held branch moved outside its writer.
    BranchMoved,
    /// The held branch disappeared.
    BranchMissing,
    /// The pull request closed without a merge.
    PullClosed,
    /// The pull request was retargeted.
    Retargeted,
    /// A delegate or effect failed permanently.
    Failed,
    /// A person rejected a required gate.
    Rejected,
    /// The next delegate is beyond the change's remaining authority.
    Authority(AuthorityHold),
}

/// A change's committed place on its way to landing.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum State {
    /// A producing delegate has yet to push the branch.
    Producing { requested: bool },
    /// A pushed branch needs a pull request.
    Opening { requested: bool },
    /// A released change is recreating its deleted branch.
    Recreating { head: Commit },
    /// A released change is reopening or retargeting its pull request.
    Reopening { requested: bool, retarget: bool },
    /// CI is read on the pushed head.
    Checking { head: Commit },
    /// Required gates are sought on the checked head.
    Gating { head: Commit, asked: Box<[u64]> },
    /// Ready at `head`: its landing prepares, proves and lands it (domain/landing.md, 3).
    Ready { head: Commit, ready_since: Wall },
    /// The prepare's rebase onto `main` is outstanding (domain/landing.md, 5.2).
    Preparing { from: Commit, main: Commit, ready_since: Wall },
    /// The change is ready, waiting its turn.
    Queued { head: Commit, ready_since: Wall },
    /// The change owns the queue through an update and landing.
    First { head: Commit, ready_since: Wall },
    /// A mechanical update of the branch is outstanding.
    Updating { from: Commit, base: Commit, ready_since: Wall },
    /// A conflict resolution delegate is outstanding.
    Resolving { requested: bool, base: Commit },
    /// A repair delegate is outstanding.
    Repairing { requested: bool, why: Repair },
    /// A merge at the named head is outstanding.
    Landing { head: Commit },
    /// The pull request merged at this commit.
    Landed { merge: Commit },
    /// The task ended after its outstanding effects and holdings settled.
    Ended,
    /// No write is made until a release lifts this cause.
    Held { was: Box<State>, why: Hold },
}

/// How the change's project lands (domain/landing.md, 1; domain/forge.md, 8.2).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Way {
    Queue,
    Turn,
}

/// The top-owned procedure record for one change task.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Change {
    pub task: u64,
    pub way: Way,
    /// Its landing's task, once it is a member (domain/landing.md, 4.2).
    pub landing: Option<u64>,
    pub state: State,
    pub gates: Box<[Gate]>,
    pub clean: Box<[Commit]>,
    /// One bounded count per gate; cleared when the head changes or the task is released.
    pub gate_failures: Box<[GateFailure]>,
    /// A lifted hold awaiting the next fresh forge snapshot.
    pub released: Option<Hold>,
    pub repairs: u32,
    pub resolutions: u32,
    pub updates: u32,
    pub since: Wall,
    pub ready_since: Option<Wall>,
    pub owns_turn: bool,
    pub last_head: Option<Commit>,
}

/// Observed state of the change's pull request.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Pull {
    /// No pull request for the branch pair was found.
    Missing,
    /// The open pull request names this head and base.
    Open { head: Commit, base: Commit },
    /// Someone closed the pull request without merging it.
    Closed,
    /// The pull request merged at this commit.
    Merged { commit: Commit },
}

/// CI observed on a specific head.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Ci {
    pub head: Commit,
    pub status: Status,
}

/// What its landing asks of the change (domain/landing.md, 3).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Ask {
    /// Rebase the last known head onto the requested main tip.
    Prepare { head: Commit, main: Commit },
    /// Repair a real gate failure at the landing's candidate.
    Repair { gate: u16, candidate: Commit },
}

/// Fresh forge and queue facts gathered by the top.
#[expect(clippy::struct_excessive_bools, reason = "independent root-supplied facts")]
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Facts {
    /// Its landing's current request; absent under queue.
    pub ask: Option<Ask>,
    pub branch: Option<Commit>,
    pub expected_base: Commit,
    pub pull: Pull,
    pub ci: Ci,
    pub base_ci: Ci,
    pub base_tip: Commit,
    pub contains_base: Status,
    pub mergeable: Status,
    pub gates: Box<[GateReport]>,
    pub writer_taken: bool,
    pub drift: Option<Hold>,
    pub first: bool,
    pub may_merge: bool,
    pub queue_repair_active: bool,
    /// The task has a pending dependency (jig's `domain/tasks.md`, 5.7), so it is not ready.
    pub kept: bool,
    /// The end of the window Forgejo is still settling in, when open.
    pub settling: Option<Wall>,
}

/// Committed delegate and effect results supplied by the top.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EffectResult {
    /// No effect result is outstanding.
    None,
    /// The requested effect is still outstanding.
    Pending,
    /// The effect was made or found again.
    Made,
    /// A mechanical update conflicted with the base.
    Conflict,
    /// An effect failed permanently.
    Failed,
}

/// Remaining spend and the reserves of the two agent charters.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct DelegateSpend {
    pub recovery: AuthorityRecovery,
    pub left: u64,
    pub coding: u64,
    pub review: u64,
}

/// Current task and effect standing, independent of the wake trigger.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Heard {
    /// Current authority, translated from jig's live accounting.
    pub lacking: Option<AuthorityHold>,
    pub spend: Option<DelegateSpend>,
    pub delegate: Status,
    pub effect: EffectResult,
    pub released: bool,
    pub cancelled: bool,
}

/// The delegate the top should create in the same decision.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Delegate {
    /// Produce and push the change's branch.
    Produce,
    /// Repair CI or a gate failure on the branch.
    Repair(Repair),
    /// Resolve a conflicting update from a merge in progress.
    Resolve { base: Commit },
    /// Obtain a gate's verdict at the current head.
    Gate { number: u64, head: Commit },
}

/// The effect the top should commit after checking authority.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Effect {
    /// Open a pull request for the pushed branch.
    Open,
    /// Recreate a deleted held branch at its last known head.
    CreateBranch { head: Commit },
    /// Reopen a pull request closed by another person.
    Reopen,
    /// Restore the change's landing branch after a retarget.
    Retarget,
    /// Merge the landing branch into the change's branch.
    Update { head: Commit, base: Commit },
    /// Rebase the pull request onto `main` (domain/landing.md, 5.2).
    Prepare { head: Commit, main: Commit },
    /// Merge the pull request at its checked head.
    Merge { head: Commit, base: Commit },
}

/// What the top does with the procedure's one decision.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Decision {
    /// Nothing is due in this snapshot.
    None,
    /// Wait for a fact or deadline to change.
    Wait { until: Option<Wall> },
    /// Create one bounded delegate.
    Delegate(Delegate),
    /// Commit one effect to the top's outbox.
    Effect(Effect),
    /// Tell the top this change is ready for its queue or landing.
    Ready,
    /// Ask the top for the branch's one queue repair.
    QueueRepair,
    /// Finish with the observed merge commit.
    Finish { merge: Commit },
    /// Finish cancellation after the top settles pending effects.
    Cancel,
    /// Hold the task and tell its requester why.
    Hold(Hold),
}

/// A next state and its decision from one coherent snapshot.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Stepped {
    pub change: Change,
    pub decision: Decision,
}

/// Bounds on delegates, updates, stalls and supplied gate facts.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Limits {
    pub repairs: u32,
    pub resolutions: u32,
    pub updates: u32,
    pub stall: Duration,
    pub gates: u32,
    pub clean_heads: u32,
}

/// One change ready for a landing branch's derived queue.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Ready {
    pub task: u64,
    pub priority: i32,
    pub since: Wall,
}
