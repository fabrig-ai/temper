//! The playground's world (playground.md, 3 and 4), composed from values as temper serve and temper agent start from their files.
//! The one-process world composes it with its harness.
mod agent;
mod machine;
mod peers;
mod service;
mod shell;
mod signalled;
mod startup;
mod story;
mod visitor;
mod world;

pub use story::{A, STORIES, Story};
pub use visitor::{
    VISITOR_ANSWER, VISITOR_COOKIE, VISITOR_COOKIES, VISITOR_DUTIES, VISITOR_EXCHANGES, VISITOR_PIECE, VISITOR_REQUEST,
    visitor,
};
pub use world::{StoryStart, story_start, world};

/// The world's process types and its visitor (playground.md, 3).
pub struct Playground;
impl skein_playground::slices::Composition for Playground {
    type Process = Process;
    type Machine = Machine;

    fn visitor(process: &mut Process) -> Option<&mut skein_playground::visitor::Visitor> {
        match process {
            Process::Visitor(visitor) => Some(visitor),
            Process::Service(_)
            | Process::Agent(_)
            | Process::Git(_)
            | Process::Shell(_)
            | Process::Llm(_)
            | Process::Forge(_) => None,
        }
    }
}

#[cfg(test)]
mod tests;

pub use agent::{agent, agent_operations};
pub use machine::Machine;
pub use peers::{ForgeStart, LlmStart, forge, llm};
pub use service::{Service, Start};
pub use shell::Shell;
pub use signalled::Signalled;
use skein_io::kernel::{Complete, Exit, Submit};
use skein_lib::{Queue, Time, Wall};
use skein_shell::Host;
use skein_sim_world::{PipeHost, StartupRoot};
/// The fake git program hosted over inherited roots and the caller's transport.
pub use temper_fake_git_hosted::git;

/// The hosted agent program the worker spawns.
pub const AGENT: &[u8] = b"/opt/temper/bin/temper";
/// The fake git program the worker spawns.
pub const GIT: &[u8] = b"/usr/bin/git";
/// The restricted shell program agents spawn.
pub const SH: &[u8] = b"/bin/sh";

/// A process of the playground's world (playground.md, 4).
pub enum Process {
    Visitor(Box<skein_playground::visitor::Visitor>),
    Service(Box<Service>),
    Agent(Box<PipeHost<smith_agent_shell::Agent>>),
    Git(Box<temper_fake_git_hosted::Git<temper_fake_git_hosted::Typed>>),
    Shell(Box<Shell>),
    Llm(Box<Signalled<skein_fake_peers::llm::Peer>>),
    Forge(Box<Signalled<skein_fake_peers::forge::Peer>>),
}
impl Process {
    fn host(&self) -> &dyn Host {
        match self {
            Self::Visitor(h) => h.as_ref(),
            Self::Service(h) => h.as_ref(),
            Self::Agent(h) => h.as_ref(),
            Self::Git(h) => h.as_ref(),
            Self::Shell(h) => h.as_ref(),
            Self::Llm(h) => h.as_ref(),
            Self::Forge(h) => h.as_ref(),
        }
    }
    fn host_mut(&mut self) -> &mut dyn Host {
        match self {
            Self::Visitor(h) => h.as_mut(),
            Self::Service(h) => h.as_mut(),
            Self::Agent(h) => h.as_mut(),
            Self::Git(h) => h.as_mut(),
            Self::Shell(h) => h.as_mut(),
            Self::Llm(h) => h.as_mut(),
            Self::Forge(h) => h.as_mut(),
        }
    }
}
impl Host for Process {
    fn drain(&mut self) {
        self.host_mut().drain();
    }
    fn iterate(&mut self, now: Time, wall: Wall) {
        self.host_mut().iterate(now, wall);
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        self.host_mut().completions()
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        self.host_mut().submissions()
    }
    fn work_pending(&self, now: Time) -> bool {
        self.host().work_pending(now)
    }
    fn next_deadline(&self) -> Option<Time> {
        self.host().next_deadline()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        self.host().next_policy_deadline()
    }
    fn is_empty(&self) -> bool {
        self.host().is_empty()
    }
    fn exit(&self) -> Option<Exit> {
        self.host().exit()
    }
    fn worst_case(&self) -> u64 {
        self.host().worst_case()
    }
    fn operations(&self) -> u32 {
        self.host().operations()
    }
}
/// The worker root opened by git.
#[must_use]
pub fn worker_roots(_spawn: &skein_io::kernel::Spawn) -> Vec<StartupRoot> {
    vec![StartupRoot { name: b"worker".as_slice().into(), path: b"worker".as_slice().into() }]
}
/// The named worker base beneath which agents open their prepared workspaces.
#[must_use]
pub fn agent_roots(_spawn: &skein_io::kernel::Spawn) -> Vec<StartupRoot> {
    vec![StartupRoot { name: b"/worker".as_slice().into(), path: b"worker".as_slice().into() }]
}
