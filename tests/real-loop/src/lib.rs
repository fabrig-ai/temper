//! The standalone stack on one checked real loop (domain/testing.md, 2.4).
#![forbid(unsafe_code)]

pub mod case;
pub mod claude;
pub mod cluster;
pub use cluster::{Cluster, run_cluster, run_cluster_forge};
pub mod fixtures;
pub mod forge;
pub mod git;
pub mod github;
pub mod issuer;
pub mod landing;
pub mod llm;
pub mod scripts;
pub mod template_bounds;
pub mod terminal;

/// A real deadline bounds every story, including startup and drain.
pub const PATIENCE: skein_lib::Duration = skein_lib::Duration::from_secs(20);
mod agent;
pub mod calls;
pub mod ending;
pub mod service;
mod world;
pub use skein_io::kernel::Exit;
pub use skein_lib::Time;
pub use skein_scratch::Scratch;
pub use temper_service::End;

/// The checked result retains its owners until assertions finish.
pub struct Ran {
    outcome: Outcome,
    scratch: Scratch,
}
enum Outcome {
    Checked(skein_world::real::CheckedOutcome<world::Proc>),
    Forge(skein_world::real::Outcome<world::Proc>),
}
impl std::ops::Deref for Outcome {
    type Target = skein_world::real::Outcome<world::Proc>;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Checked(outcome) => outcome,
            Self::Forge(outcome) => outcome,
        }
    }
}
impl Ran {
    #[must_use]
    pub fn forge(&self) -> &forge::Seen {
        &self.service().seen
    }
    #[must_use]
    pub fn github(&self) -> &github::Seen {
        &self.service().github_seen
    }
    #[must_use]
    pub fn calls(&self) -> &[calls::Called] {
        let world::Proc::Caller(caller) = &self.outcome.procs[2] else { panic!("caller root") };
        &caller.calls
    }
    #[must_use]
    pub fn browsed(&self) -> &[calls::Browsed] {
        let world::Proc::Caller(caller) = &self.outcome.procs[2] else { panic!("caller root") };
        &caller.browsed
    }
    /// What the person read from the hosted terminal.
    #[must_use]
    pub fn terminal(&self) -> &terminal::Read {
        let world::Proc::Caller(caller) = &self.outcome.procs[2] else { panic!("caller root") };
        &caller.terminal.read
    }
    /// Answers to native GETs of the kept redirect, in story order.
    #[must_use]
    pub fn returned(&self) -> &[issuer::Returned] {
        let world::Proc::Caller(caller) = &self.outcome.procs[2] else { panic!("caller root") };
        &caller.returned
    }
    /// Count complete token POSTs; this fixture queues only a code grant.
    #[must_use]
    pub fn code_exchanges(&self) -> usize {
        let peer = self
            .outcome
            .procs
            .iter()
            .find_map(|proc| if let world::Proc::Issuer(peer) = proc { Some(peer) } else { None })
            .expect("story's issuer");
        peer.observations()
            .iter()
            .filter(|event| matches!(event, skein_fake_peers::oauth::Observation::Post { .. }))
            .count()
    }
    /// The task number printed by this call's completed act.
    #[must_use]
    pub fn number(&self, call: usize) -> u64 {
        calls::task_number(&self.calls()[call].stdout)
    }
    fn requested_kills(&self) -> usize {
        let world::Proc::Caller(caller) = &self.outcome.procs[2] else { panic!("caller root") };
        caller.requested_kills()
    }
    fn service(&self) -> &service::ServiceHost {
        let world::Proc::Service(service) = &self.outcome.procs[1] else { panic!("service root") };
        service
    }
    #[must_use]
    pub fn served(&self) -> Option<&End> {
        self.service().service.as_ref().and_then(temper_service::Service::end)
    }
    #[must_use]
    pub fn link(&self) -> &[u8] {
        &self.service().link
    }
    #[must_use]
    pub fn llm(&self) -> &[skein_fake_peers::llm::Observation] {
        let world::Proc::Llm(peer) = &self.outcome.procs[0] else { panic!("LLM root") };
        peer.peer.observations()
    }
    #[must_use]
    pub fn killed(&self) -> &[skein_world::real::Killed] {
        &self.outcome.killed
    }
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        self.scratch.path()
    }
    #[must_use]
    pub fn into_scratch(self) -> Scratch {
        drop(self.outcome);
        self.scratch
    }
}
#[must_use]
pub fn run(
    scripts: impl FnOnce() -> Box<[skein_fake_llm_domain::api::Script]>,
    steps: impl FnOnce() -> Vec<calls::Step>,
) -> Ran {
    run_on(Scratch::new("real-loop"), || llm::scripted(scripts()), steps)
}
#[must_use]
pub fn run_on(scratch: Scratch, recipe: impl FnOnce() -> llm::Recipe, steps: impl FnOnce() -> Vec<calls::Step>) -> Ran {
    let outcome = world::run(&scratch, recipe, steps, false, false);
    checked(Ran { outcome: Outcome::Checked(outcome), scratch })
}
/// Run a terminal sign-in with a synthetic issuer and no provisioned key.
#[must_use]
pub fn run_sign_in(steps: impl FnOnce() -> Vec<calls::Step>) -> Ran {
    let scratch = Scratch::new("real-sign-in");
    let outcome = world::run(&scratch, || llm::scripted(Box::new([])), steps, true, true);
    checked(Ran { outcome: Outcome::Checked(outcome), scratch })
}
fn checked(ran: Ran) -> Ran {
    assert_eq!(ran.served(), Some(&End::Drained), "service end {:?}", ran.served());
    assert_eq!(ran.killed().len(), ran.requested_kills(), "only requested kills: {:?}", ran.killed());
    for killed in ran.killed() {
        assert_eq!(killed.exit, Exit::Signal(9), "requested kill");
    }
    ran
}

#[must_use]
pub fn run_forge(
    fixture: forge::Fixture,
    scripts: fn(std::net::SocketAddr) -> Box<[skein_fake_llm_domain::api::Script]>,
    steps: impl FnOnce() -> Vec<calls::Step>,
) -> Ran {
    let scratch = Scratch::new("real-forge");
    run_forge_on(scratch, fixture, scripts, steps)
}

/// Run a forge story on a scratch with a privately laid program source.
#[must_use]
pub fn run_forge_on(
    scratch: Scratch,
    fixture: forge::Fixture,
    scripts: fn(std::net::SocketAddr) -> Box<[skein_fake_llm_domain::api::Script]>,
    steps: impl FnOnce() -> Vec<calls::Step>,
) -> Ran {
    let outcome = world::run_forge(&scratch, fixture, scripts, None, steps);
    checked(Ran { outcome: Outcome::Forge(outcome), scratch })
}

/// Run a forge story whose scripts pick each turn by their input rules (skein's fake-llm.md, 2.4).
#[must_use]
pub fn run_forge_by_input(
    fixture: forge::Fixture,
    scripts: fn(std::net::SocketAddr) -> Box<[skein_fake_llm_domain::api::Script]>,
    rules: fn() -> Box<[Box<[skein_fake_llm_domain::api::Rule]>]>,
    steps: impl FnOnce() -> Vec<calls::Step>,
) -> Ran {
    let scratch = Scratch::new("real-forge");
    let outcome = world::run_forge(&scratch, fixture, scripts, Some(rules), steps);
    checked(Ran { outcome: Outcome::Forge(outcome), scratch })
}

/// Run the private-key smoke on a standard `OpenAI` endpoint.
#[must_use]
pub fn run_key_on(
    scratch: Scratch,
    recipe: impl FnOnce() -> llm::Recipe,
    steps: impl FnOnce() -> Vec<calls::Step>,
) -> Ran {
    let outcome = world::run(&scratch, recipe, steps, true, false);
    checked(Ran { outcome: Outcome::Checked(outcome), scratch })
}

/// Run a GitHub story on the layer's authenticated HTTP git route.
#[must_use]
pub fn run_github(
    fixture: github::Fixture,
    scripts: fn(std::net::SocketAddr) -> Box<[skein_fake_llm_domain::api::Script]>,
    steps: impl FnOnce() -> Vec<calls::Step>,
) -> Ran {
    let scratch = Scratch::new("real-github");
    let outcome = world::run_github(&scratch, fixture, scripts, steps);
    checked(Ran { outcome: Outcome::Forge(outcome), scratch })
}

pub mod settings;
