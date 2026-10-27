//! Shared workflow following, independent of its owning procedure (domain/landing.md, 7).
use alloc::boxed::Box;
use skein_forge::api;
use skein_lib::{Duration, List, Time};
use temper_engine_domain_forge_landing as policy;

/// One caller-owned dispatch and its current run. The caller persists its outbox entry.
#[derive(Clone, Debug)]
pub struct Follow {
    pub task: u64,
    pub entry: u64,
    pub purpose: Box<[u8]>,
    pub repository: api::Repository,
    pub workflow: Box<[u8]>,
    pub branch: Box<[u8]>,
    pub commit: api::Commit,
    pub inputs: Box<[api::Input]>,
    pub start: Option<Time>,
    pub waiting: bool,
    pub since: Option<Time>,
    pub run: Option<api::Run>,
    pub outcome: Option<policy::Verdict>,
    pub writes: Box<[api::Write]>,
    pub page: u32,
}
#[derive(Clone, Debug)]
pub struct Ended {
    pub run: Option<api::Run>,
    pub outcome: policy::Verdict,
}

/// Start a workflow under the caller's task, entry and stable purpose.
#[must_use]
#[expect(clippy::too_many_arguments, reason = "the shared follower receives the caller's entire dispatch")]
pub fn follow(
    task: u64,
    entry: u64,
    purpose: Box<[u8]>,
    repository: api::Repository,
    workflow: Box<[u8]>,
    branch: Box<[u8]>,
    commit: api::Commit,
    inputs: Box<[api::Input]>,
) -> Follow {
    let write = api::Write::Dispatch { workflow: workflow.clone(), branch: branch.clone(), inputs: inputs.clone() };
    Follow {
        task,
        entry,
        purpose,
        repository,
        workflow,
        branch,
        commit,
        inputs,
        start: None,
        waiting: false,
        since: None,
        run: None,
        outcome: None,
        writes: Box::new([write]),
        page: 1,
    }
}

/// Retry each failed job of a job fault, otherwise dispatch afresh. The caller
/// supplies one outbox entry for each write, preserving its ordinary commit fence.
pub fn retry(follow: &mut Follow, attempt: u16) {
    assert!(attempt > 1, "a retry follows the first attempt");
    let mut writes = List::with_capacity(match &follow.run {
        Some(run) => u32::try_from(run.jobs.len()).expect("bounded jobs").max(1),
        None => 1,
    });
    match follow.outcome {
        Some(policy::Verdict::Fault(policy::Fault::SetUp | policy::Fault::Completion | policy::Fault::NoStep)) => {
            if let Some(run) = &follow.run {
                for job in &run.jobs {
                    if job.conclusion == Some(api::Conclusion::Failure) {
                        writes
                            .push(api::Write::Rerun {
                                attempt: api::JobAttempt {
                                    head: follow.commit,
                                    run: run.id,
                                    job: job.id,
                                    attempt: job.attempt,
                                },
                            })
                            .expect("one rerun per failed job");
                    }
                }
            }
        }
        Some(
            policy::Verdict::Running
            | policy::Verdict::Passed
            | policy::Verdict::Failed
            | policy::Verdict::Fault(policy::Fault::Cancelled | policy::Fault::OtherCommit | policy::Fault::Lost),
        )
        | None => {}
    }
    if writes.is_empty() {
        writes
            .push(api::Write::Dispatch {
                workflow: follow.workflow.clone(),
                branch: follow.branch.clone(),
                inputs: follow.inputs.clone(),
            })
            .expect("one dispatch");
        follow.run = None;
        follow.start = None;
    }
    follow.writes = writes.into_boxed();
    follow.outcome = None;
    follow.page = 1;
}

impl Follow {
    /// Take the next committed write; the caller records the supplied entry number.
    pub fn write(&mut self, entry: u64) -> Option<api::Write> {
        let mut remaining = List::with_capacity(u32::try_from(self.writes.len()).expect("bounded jobs"));
        let mut first = None;
        for write in core::mem::replace(&mut self.writes, Box::new([])) {
            if first.is_none() {
                first = Some(write);
            } else {
                remaining.push(write).expect("remaining writes");
            }
        }
        self.writes = remaining.into_boxed();
        self.entry = entry;
        self.waiting = first.is_some();
        if let Some(api::Write::Rerun { .. }) = &first
            && let Some(run) = &mut self.run
        {
            run.state = api::RunState::Running;
            run.conclusion = None;
        }
        first
    }
    #[must_use]
    pub fn read(&self) -> Option<api::Read> {
        if self.outcome.is_some() || self.start.is_none() || self.waiting {
            return None;
        }
        Some(match &self.run {
            Some(run) => api::Read::Run { run: run.id },
            None => api::Read::Runs { workflow: self.workflow.clone(), commit: self.commit, page: self.page },
        })
    }
    /// Accept a matching run only from temper's own dispatch window. A terminal
    /// observation is returned once, then kept as the caller's current verdict.
    pub fn answered(
        &mut self,
        answer: Result<api::Answer, api::Error>,
        writer: u64,
        now: Time,
        lifetime: Duration,
    ) -> Option<Ended> {
        match answer {
            Ok(api::Answer::Runs { runs, more }) => {
                for run in runs {
                    if run.actor == writer
                        && run.event == api::Event::Dispatch
                        && run.commit == self.commit
                        && run.workflow == self.workflow
                        && Some(run.created) >= self.start
                    {
                        self.run = Some(run);
                        break;
                    }
                }
                if self.run.is_none() && more {
                    self.page = self.page.checked_add(1).expect("bounded run pages");
                } else {
                    self.page = 1;
                }
            }
            Ok(api::Answer::Run(run)) => {
                if match &self.run {
                    Some(known) => known.id == run.id,
                    None => false,
                } {
                    if !self.writes.is_empty() {
                        self.run = Some(run);
                        return None;
                    }
                    let outcome = verdict(&run, self.commit);
                    self.run = Some(run);
                    if outcome != policy::Verdict::Running && self.outcome.is_none() {
                        self.outcome = Some(outcome);
                        return Some(Ended { run: self.run.clone(), outcome });
                    }
                }
            }
            Ok(_) | Err(_) => {}
        }
        if self.run.is_none()
            && self.outcome.is_none()
            && let Some(start) = self.since.or(self.start)
            && now >= start.saturating_add(lifetime)
        {
            let outcome = policy::Verdict::Fault(policy::Fault::Lost);
            self.outcome = Some(outcome);
            return Some(Ended { run: None, outcome });
        }
        None
    }
}

fn conclusion(value: Option<api::Conclusion>) -> Option<policy::Conclusion> {
    match value {
        Some(api::Conclusion::Success) => Some(policy::Conclusion::Success),
        Some(api::Conclusion::Failure) => Some(policy::Conclusion::Failure),
        Some(api::Conclusion::Cancelled) => Some(policy::Conclusion::Cancelled),
        Some(api::Conclusion::Skipped) => Some(policy::Conclusion::Skipped),
        None => None,
    }
}
#[must_use]
#[expect(clippy::manual_map, reason = "the foundation uses explicit option matches without closures")]
pub fn verdict(run: &api::Run, candidate: api::Commit) -> policy::Verdict {
    let mut jobs = List::with_capacity(u32::try_from(run.jobs.len()).expect("bounded jobs"));
    for job in &run.jobs {
        jobs.push(policy::Job {
            conclusion: conclusion(job.conclusion),
            failed: match &job.step {
                Some(step) => Some(step.number),
                None => None,
            },
            steps: job.steps,
        })
        .expect("bounded jobs");
    }
    policy::verdict(
        &policy::Run {
            commit: run.commit,
            state: match run.state {
                api::RunState::Waiting => policy::RunState::Waiting,
                api::RunState::Running => policy::RunState::Running,
                api::RunState::Completed => policy::RunState::Completed,
            },
            conclusion: conclusion(run.conclusion),
            jobs: jobs.into_boxed(),
            cancelled_by_temper: false,
        },
        candidate,
    )
}
