//! Workflow failures and runner faults (domain/landing.md, 7.3–7.4).
use crate::Commit;
use alloc::boxed::Box;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RunState {
    Waiting,
    Running,
    Completed,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Conclusion {
    Success,
    Failure,
    Cancelled,
    Skipped,
}
/// The failed step is zero-based, with set-up first and completion last.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Job {
    pub conclusion: Option<Conclusion>,
    pub failed: Option<u32>,
    pub steps: u32,
}
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Run {
    pub commit: Commit,
    pub state: RunState,
    pub conclusion: Option<Conclusion>,
    pub jobs: Box<[Job]>,
    pub cancelled_by_temper: bool,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Verdict {
    Running,
    Passed,
    Failed,
    Fault(Fault),
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Fault {
    SetUp,
    Completion,
    NoStep,
    Cancelled,
    OtherCommit,
    Lost,
}

/// Classify only the run and step facts, never log words.
#[must_use]
pub fn verdict(run: &Run, candidate: Commit) -> Verdict {
    if run.commit != candidate {
        return Verdict::Fault(Fault::OtherCommit);
    }
    match run.state {
        RunState::Waiting | RunState::Running => return Verdict::Running,
        RunState::Completed => {}
    }
    match run.conclusion {
        Some(Conclusion::Success) => Verdict::Passed,
        Some(Conclusion::Cancelled) if !run.cancelled_by_temper => Verdict::Fault(Fault::Cancelled),
        Some(Conclusion::Cancelled) => Verdict::Running,
        Some(Conclusion::Failure) => failed_jobs(&run.jobs),
        Some(Conclusion::Skipped) | None => Verdict::Fault(Fault::NoStep),
    }
}

fn failed_jobs(jobs: &[Job]) -> Verdict {
    let mut first = None;
    for job in jobs {
        if job.conclusion != Some(Conclusion::Failure) {
            continue;
        }
        let fault = match job.failed {
            Some(step) if step > 0 && step < job.steps.saturating_sub(1) => return Verdict::Failed,
            Some(0) if job.steps > 0 => Fault::SetUp,
            Some(step) if job.steps > 1 && step == job.steps.saturating_sub(1) => Fault::Completion,
            Some(_) | None => Fault::NoStep,
        };
        if first.is_none() {
            first = Some(fault);
        }
    }
    Verdict::Fault(first.unwrap_or(Fault::NoStep))
}
