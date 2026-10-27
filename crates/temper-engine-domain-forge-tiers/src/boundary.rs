//! Top-owned tier records and verdicts (domain/after-landing.md, sections 2–5).
use alloc::boxed::Box;

pub type Commit = [u8; 32];
/// A tier's key at a commit (domain/landing.md, 6.1), computed by the top.
pub type Key = [u8; 32];
/// A test's id, at most `Limits::test_id_bytes` bytes.
pub type TestId = Box<[u8]>;

/// Bounds on what a run reports (domain/after-landing.md, 4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Limits {
    pub failing_tests: u32,
    pub test_id_bytes: u32,
}

/// A project's settings the verdict reads (domain/after-landing.md, 11).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Settings {
    pub retries: u32,
    pub suspects: Suspects,
}
/// Several landings' new failures: bisected, or every landing told.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Suspects {
    Bisect,
    TellAll,
}

/// A tier's report file's format (domain/after-landing.md, 2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Junit,
    Lines,
}

/// The failing tests a run names.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Failures {
    /// Sorted, unique, at most `failing_tests`; empty is none.
    Tests(Box<[TestId]>),
    /// Past `failing_tests`, or an id past `test_id_bytes`: blamed by
    /// landing alone, never by test.
    Many,
}

/// A report file its format cannot read.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Unreadable;

/// What the top observed of one dispatched part of a tier.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Outcome {
    Passed,
    /// Failed naming these tests: never `Tests` empty.
    Failed(Failures),
    /// The runner's fault (domain/landing.md, 7.4), or a failed run whose
    /// report is absent, unreadable or names no test (domain/after-landing.md, 4).
    Fault,
}

/// A receipt at the tip's key, as a tier reads it (domain/landing.md, 6.2).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Receipt {
    Passed,
    Failed(Failures),
}

/// A tier's record, the top's (store family 25, `Tier`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Record {
    /// Judged runs so far; the next run is numbered `sequence + 1`.
    pub sequence: u64,
    /// The last judged tip and the tier's key there; none before the baseline.
    pub judged: Option<Judged>,
    /// The tier's failures at the last judged tip.
    pub failing: Failures,
    /// Flakes counted by test, sorted by test, at most `failing_tests`.
    pub flakes: Box<[Flake]>,
    /// The run in flight.
    pub run: Option<Run>,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Judged {
    pub tip: Commit,
    pub key: Key,
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Flake {
    pub test: TestId,
    pub count: u32,
}

/// A run in flight: its tip, its range and the part dispatched now.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Run {
    /// Its number in the tier's sequence.
    pub number: u64,
    pub tip: Commit,
    /// The tier's key at the tip.
    pub key: Key,
    /// The landings in its range, oldest first; empty for the baseline.
    pub landings: Box<[Landing]>,
    pub part: Part,
    /// Faults of this part so far.
    pub faults: u32,
}

/// One landing in a run's range (domain/after-landing.md, 3): its commit, the
/// last of its own range, and the tier's key there (the last landing's
/// is the tip's).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Landing {
    pub commit: Commit,
    pub key: Key,
}

/// The part of the tier dispatched now.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Part {
    /// The whole tier at the tip.
    Whole,
    /// The failed tests alone, again at the tip (5.2, 1).
    Rerun { failed: Box<[TestId]> },
    /// A probe: `tests` alone at landing `landing`'s commit (5.2, 4).
    Probe { landing: u32, tests: Selection, finding: Finding },
}
/// What a probe runs: these tests alone, or, after many, the whole tier.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Selection {
    Tests(Box<[TestId]>),
    All,
}

/// A verdict being found, kept across probes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Finding {
    /// The run's failures once rerun: the tier's at the tip once judged.
    pub failing: Failures,
    /// Tests that failed, then passed alone.
    pub flakes: Box<[TestId]>,
    /// One per landing of the range, oldest first, so far.
    pub blamed: Box<[Blame]>,
}

/// What a verdict lays on one landing for this tier (5.2, 5.3).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Blame {
    /// Nothing: cleared.
    Cleared,
    /// These new failures are its own.
    Owns(Failures),
    /// Told all: a suspect of these new failures, with every landing of its range.
    Suspect(Failures),
}

/// A judged run's verdict.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Verdict {
    pub number: u64,
    pub tip: Commit,
    pub key: Key,
    /// The tier's failures at the tip: the record's next `failing`.
    pub failing: Failures,
    /// One per landing of the range, oldest first.
    pub landings: Box<[Blame]>,
    pub flakes: Box<[TestId]>,
}

/// What the top does next for a tier.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Next {
    /// Dispatch `run.part` now: a new part, or the same again after a fault.
    Dispatch(Run),
    /// Judged: the top saves the verdict, then the record `judged` makes.
    Judged(Verdict),
    /// Past `retries`: held for a person's release (3), its landings
    /// still verifying. The release dispatches `run` again, faults 0.
    Held(Run),
}
