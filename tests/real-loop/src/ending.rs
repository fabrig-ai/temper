//! Completion obligations at the frozen stop boundary (domain/testing.md, 2.4).
use crate::service::ServiceHost;
use crate::world::Proc;
use jig_domain_tasks::{Active, Phase, Was};
use skein_shell::Host;
use temper_worker_domain::host;

/// Intentional unfinished work and the obligation states it may reach at stop.
#[derive(Clone, Copy, Debug)]
pub struct Leftover {
    pub task: u64,
    /// Permitted live obligations; a parked task also satisfies this declaration.
    pub may_be: &'static [Left],
    pub why: &'static str,
}

/// States that still owe work when a story's last call has returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Left {
    /// An activation has been requested but not prepared.
    Due,
    /// The engine is gathering the activation's brief and resources.
    Preparing,
    /// The engine committed the attempt, before its start was reported.
    Claimed,
    /// An admitted attempt has not reported its terminal.
    Running,
    /// Task closure still owes a run, delegates, effects or resource release.
    Closing,
    /// A prepared assignment still waits to be handed to a worker.
    AssignmentPending,
    /// The worker is preparing its hosted attempt's workspace.
    HostedPreparing,
    /// The worker has not finished starting the agent.
    HostedStarting,
    /// The worker's agent is at work.
    HostedActive,
    /// A waiting agent still occupies a live worker activation, before parking.
    HostedWaiting,
    /// The worker is stopping or saving an attempt before its terminal.
    HostedEnding,
}

#[derive(Debug)]
enum Subject {
    Task(u64),
    Child { index: usize, program: &'static str },
}
#[derive(Debug)]
struct Obligation {
    subject: Subject,
    state: Option<Left>,
    source: &'static str,
}
#[derive(Debug)]
enum Presence {
    Live,
    Ended,
}
#[derive(Debug, Default)]
pub(crate) struct Obligations {
    live: Vec<Obligation>,
    tasks: Vec<(u64, Presence)>,
}
impl Obligations {
    /// Called by the referee only, outside every host's heap meter.
    pub(crate) fn read(&mut self, service: &ServiceHost, declared: &[Leftover]) {
        let Some(service) = &service.service else { return };
        if let Some(engine) = service.engine_domain() {
            let tasks = engine.tasks();
            for row in &tasks.view_tasks() {
                let task = tasks.task(row.number).expect("live task view");
                self.phase(row.number, &task.phase);
            }
            // Terminal rows leave the live arena. Retained dependency stubs
            // still prove an ending; an absent row/stub is reported not found.
            for left in declared {
                if tasks.task(left.task).is_none() && tasks.stub(left.task).is_some() {
                    self.tasks.push((left.task, Presence::Ended));
                }
            }
            for task in &engine.pending_assignments() {
                self.task(*task, Left::AssignmentPending, "engine");
            }
        }
        if let Some(worker) = service.worker_domain() {
            for hosting in &worker.hosting() {
                let state = match hosting.phase {
                    host::Phase::Preparing => Left::HostedPreparing,
                    host::Phase::Starting => Left::HostedStarting,
                    host::Phase::Active => Left::HostedActive,
                    host::Phase::Waiting => Left::HostedWaiting,
                    host::Phase::Ending => Left::HostedEnding,
                    host::Phase::Answered => continue,
                };
                self.task(hosting.run.raw(), state, "worker");
            }
        }
    }
    pub(crate) fn read_child(&mut self, index: usize, child: &Proc) {
        if child.exit().is_some() {
            return;
        }
        match child {
            Proc::Agent(_) => self.child(index, "agent"),
            Proc::Git(_) | Proc::GitRoute(_) => self.child(index, "git"),
            Proc::Claude(_) => self.child(index, "claude"),
            Proc::Relay(_) => self.child(index, "relay"),
            Proc::Llm(_)
            | Proc::Forge(_)
            | Proc::Issuer(_)
            | Proc::Service(_)
            | Proc::Caller(_)
            | Proc::Ctl(_)
            | Proc::Terminal(_) => {}
        }
    }
    fn phase(&mut self, task: u64, phase: &Phase) {
        let presence = if matches!(phase, Phase::Ended(_)) { Presence::Ended } else { Presence::Live };
        self.tasks.push((task, presence));
        match phase {
            Phase::Active(active) | Phase::Held { was: Was::Active(active), .. } => self.active(task, *active),
            Phase::Closing(_) | Phase::Held { was: Was::Closing(_), .. } => self.task(task, Left::Closing, "engine"),
            Phase::Waiting | Phase::Held { was: Was::Waiting, .. } | Phase::Ended(_) => {}
        }
    }
    fn active(&mut self, task: u64, active: Active) {
        let state = match active {
            Active::Due => Left::Due,
            Active::Preparing => Left::Preparing,
            Active::Claimed { .. } => Left::Claimed,
            Active::Running { .. } => Left::Running,
            Active::Idle | Active::BackingOff { .. } => return,
        };
        self.task(task, state, "engine");
    }
    fn task(&mut self, task: u64, state: Left, source: &'static str) {
        self.live.push(Obligation { subject: Subject::Task(task), state: Some(state), source });
    }
    /// A still-alive hosted child is independent evidence, even if its task vanished.
    pub(crate) fn child(&mut self, index: usize, program: &'static str) {
        self.live.push(Obligation { subject: Subject::Child { index, program }, state: None, source: "worker child" });
    }
    pub(crate) fn check(&self, declared: &[Leftover]) -> Result<(), String> {
        use std::fmt::Write;
        let mut errors = String::new();
        for obligation in &self.live {
            match obligation.subject {
                Subject::Task(task) => {
                    let state = obligation.state.expect("task obligation has a state");
                    if let Some(left) = declared.iter().find(|left| left.task == task) {
                        if !left.may_be.contains(&state) {
                            writeln!(
                                errors,
                                "declared leftover task {task} is {state:?}, outside {:?}: {}",
                                left.may_be, left.why
                            )
                            .expect("string");
                        }
                    } else {
                        writeln!(errors, "undeclared {} task {task}: {state:?}", obligation.source).expect("string");
                    }
                }
                Subject::Child { index, program } => {
                    writeln!(errors, "undeclared live {program} child {index}").expect("string");
                }
            }
        }
        for (index, left) in declared.iter().enumerate() {
            if left.may_be.is_empty() {
                writeln!(errors, "declared leftover task {} has no allowed states", left.task).expect("string");
            }
            if left.why.trim().is_empty() {
                writeln!(errors, "declared leftover task {} has no reason", left.task).expect("string");
            }
            if declared[..index].iter().any(|other| other.task == left.task) {
                writeln!(errors, "duplicate leftover task {}", left.task).expect("string");
            }
            // The phase classifier gives every unfinished, unparked engine
            // task an obligation above. Presence without one therefore parks.
            match self.tasks.iter().find(|(task, _)| *task == left.task) {
                Some((_, Presence::Live)) => {}
                Some((_, Presence::Ended)) => {
                    writeln!(errors, "declared leftover task {} ended: {}", left.task, left.why).expect("string");
                }
                None => {
                    writeln!(errors, "declared leftover task {} not found: {}", left.task, left.why).expect("string");
                }
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(format!("story completion obligations:\n{errors}")) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_undeclared_live_attempt_names_its_task() {
        let mut obligations = Obligations::default();
        obligations.phase(41, &Phase::Active(Active::Running { attempt: 7 }));
        let error = obligations.check(&[]).expect_err("live attempt");
        assert!(error.contains("task 41: Running"), "{error}");
    }
    #[test]
    fn an_undeclared_closing_task_fails() {
        let mut obligations = Obligations::default();
        obligations.phase(
            42,
            &Phase::Closing(jig_domain_tasks::Closing {
                stage: jig_domain_tasks::Stage::Delegates,
                ending: jig_domain_tasks::Ending::Failed { reason: b"closing".as_slice().into() },
            }),
        );
        let error = obligations.check(&[]).expect_err("closing task");
        assert!(error.contains("task 42: Closing"), "{error}");
    }
    #[test]
    fn a_missing_declared_task_fails() {
        let error = Obligations::default()
            .check(&[Leftover { task: 43, may_be: &[Left::Due], why: "push-only change" }])
            .expect_err("absent task");
        assert!(error.contains("declared leftover task 43 not found"), "{error}");
    }
    #[test]
    fn a_parked_task_passes_without_a_declaration() {
        let mut obligations = Obligations::default();
        obligations.phase(44, &Phase::Active(Active::Idle));
        assert_eq!(obligations.check(&[]), Ok(()));
    }
    #[test]
    fn a_declared_task_in_any_of_its_allowed_states_passes() {
        let mut obligations = Obligations::default();
        obligations.phase(45, &Phase::Active(Active::Running { attempt: 8 }));
        assert_eq!(
            obligations.check(&[Leftover {
                task: 45,
                may_be: &[Left::Due, Left::Running],
                why: "push-only change may advance",
            }]),
            Ok(())
        );
    }
    #[test]
    fn a_wrong_state_names_the_state_it_was_in() {
        let mut obligations = Obligations::default();
        obligations.phase(46, &Phase::Active(Active::Claimed { attempt: 8 }));
        let error = obligations
            .check(&[Leftover {
                task: 46,
                may_be: &[Left::Due, Left::Running],
                why: "claim is outside this fixture's contract",
            }])
            .expect_err("unallowed state");
        assert!(error.contains("declared leftover task 46 is Claimed"), "{error}");
    }
    #[test]
    fn a_live_child_cannot_hide_behind_a_task_declaration() {
        let mut obligations = Obligations::default();
        obligations.phase(47, &Phase::Active(Active::Idle));
        obligations.child(47, "git");
        let error = obligations
            .check(&[Leftover { task: 47, may_be: &[Left::Due], why: "parked task" }])
            .expect_err("live git");
        assert!(error.contains("live git child 47"), "{error}");
    }
    #[test]
    fn an_ended_declared_task_fails() {
        let mut obligations = Obligations::default();
        obligations.phase(48, &Phase::Ended(jig_domain_tasks::Ending::Failed { reason: b"ended".as_slice().into() }));
        let error = obligations
            .check(&[Leftover { task: 48, may_be: &[Left::Due, Left::Closing], why: "stale unfinished work" }])
            .expect_err("ended task");
        assert!(error.contains("declared leftover task 48 ended"), "{error}");
    }
    #[test]
    fn a_declared_task_may_park_without_an_obligation() {
        for phase in [
            Phase::Waiting,
            Phase::Active(Active::Idle),
            Phase::Active(Active::BackingOff { until: skein_lib::Wall::EPOCH }),
        ] {
            let mut obligations = Obligations::default();
            obligations.phase(49, &phase);
            assert_eq!(
                obligations.check(&[Leftover { task: 49, may_be: &[Left::Due], why: "procedure waits for CI" }]),
                Ok(()),
                "{phase:?}"
            );
        }
    }
    #[test]
    fn an_empty_allowlist_fails_even_when_parked() {
        let mut obligations = Obligations::default();
        obligations.phase(50, &Phase::Waiting);
        let error = obligations
            .check(&[Leftover { task: 50, may_be: &[], why: "waiting fixture" }])
            .expect_err("empty allowlist");
        assert!(error.contains("task 50 has no allowed states"), "{error}");
    }
    #[test]
    fn an_empty_reason_fails_even_when_parked() {
        let mut obligations = Obligations::default();
        obligations.phase(51, &Phase::Waiting);
        let error =
            obligations.check(&[Leftover { task: 51, may_be: &[Left::Due], why: "  " }]).expect_err("empty reason");
        assert!(error.contains("task 51 has no reason"), "{error}");
    }
    #[test]
    fn duplicate_tasks_fail_even_with_different_allowlists() {
        let mut obligations = Obligations::default();
        obligations.phase(52, &Phase::Waiting);
        let error = obligations
            .check(&[
                Leftover { task: 52, may_be: &[Left::Due], why: "first" },
                Leftover { task: 52, may_be: &[Left::Running], why: "second" },
            ])
            .expect_err("duplicate task");
        assert!(error.contains("duplicate leftover task 52"), "{error}");
    }
    #[test]
    fn every_obligation_of_a_declared_task_must_be_allowed() {
        let mut obligations = Obligations::default();
        obligations.phase(53, &Phase::Active(Active::Claimed { attempt: 9 }));
        obligations.task(53, Left::AssignmentPending, "engine");
        let error = obligations
            .check(&[Leftover { task: 53, may_be: &[Left::Claimed], why: "only the claim was intended" }])
            .expect_err("pending assignment");
        assert!(error.contains("task 53 is AssignmentPending"), "{error}");
        assert_eq!(
            obligations.check(&[Leftover {
                task: 53,
                may_be: &[Left::Claimed, Left::AssignmentPending],
                why: "both obligations intended",
            }]),
            Ok(())
        );
    }
    #[test]
    fn a_worker_obligation_does_not_prove_the_engine_task_exists() {
        let mut obligations = Obligations::default();
        obligations.task(54, Left::HostedEnding, "worker");
        let error = obligations
            .check(&[Leftover { task: 54, may_be: &[Left::HostedEnding], why: "worker tail" }])
            .expect_err("missing engine task");
        assert!(error.contains("declared leftover task 54 not found"), "{error}");
    }
}
