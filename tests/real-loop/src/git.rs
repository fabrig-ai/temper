//! Hosted fake git on the real filesystem, sharing the caller's pipe lifecycle.
use crate::{calls::Caller, world::Proc};
use skein_world::{HostedProgram, Inherited, StartupRoot, real::World};
use std::{cell::RefCell, ffi::OsStr, os::unix::ffi::OsStrExt, path::PathBuf};
use temper_fake_git_hosted::{Answer, Typed, fixtures::Scenario};
use temper_worker_protocol_git::invocation::Command;

pub struct Disk {
    scratch: skein_scratch::Scratch,
    scenario: Scenario,
}
impl Disk {
    #[must_use]
    pub fn new(make: fn() -> Scenario) -> Self {
        let disk = Self { scratch: skein_scratch::Scratch::new("real-git"), scenario: make() };
        for (path, bytes) in disk.scenario.checkout.files() {
            let path = disk.scratch.path().join(OsStr::from_bytes(path));
            std::fs::create_dir_all(path.parent().expect("file parent")).expect("checkout directories");
            std::fs::write(path, bytes).expect("checkout file");
        }
        disk
    }
    fn path(&self, name: &[u8]) -> PathBuf {
        self.scratch.path().join(OsStr::from_bytes(&self.scenario.at)).join(OsStr::from_bytes(name))
    }
    pub fn write(&mut self, name: &[u8], bytes: &[u8]) {
        let path = self.path(name);
        std::fs::create_dir_all(path.parent().expect("file parent")).expect("file directories");
        std::fs::write(path, bytes).expect("working file");
    }
    #[must_use]
    pub fn read(&self, name: &[u8]) -> Vec<u8> {
        std::fs::read(self.path(name)).expect("working file")
    }
    #[must_use]
    pub fn exists(&self, name: &[u8]) -> bool {
        self.path(name).exists()
    }
    #[must_use]
    pub fn forge(&self) -> std::cell::Ref<'_, skein_fake_peers::forge::Forge> {
        self.scenario.transport.forge.borrow()
    }
    pub fn run(&mut self, command: &Command, input: &[u8]) -> Answer {
        share(Some(self.scenario.transport.clone()));
        let mut world = World::new_controlled(|_| Judge);
        let root = skein_shell::open_root(self.scratch.path()).expect("git launch root");
        let mut spawn = temper_fake_git_hosted::fixtures::spawn_of(command);
        spawn.dir = self.scenario.at.clone().into();
        world.spawn_with_fds(vec![root], || Proc::Caller(Box::new(Caller::command(spawn, input, root))));
        world.host_roots(program(), roots);
        let outcome = world.run(&skein_shell::Clock::new(), crate::PATIENCE);
        share(None);
        assert!(outcome.killed.is_empty(), "git settled without a kill");
        let Proc::Caller(caller) = &outcome.procs[0] else { panic!("command root") };
        let result = &caller.calls[0];
        let Some(skein_io::kernel::Exit::Code(exit)) = result.exit else { panic!("git exit {:?}", result.exit) };
        Answer { exit, stdout: result.stdout.clone(), stderr: result.stderr.clone() }
    }
}
struct Judge;
impl skein_world::Referee<Proc> for Judge {
    fn act(&mut self, _: skein_lib::Time, _: &mut [Proc]) {}
    fn observe(&mut self, _: skein_lib::Time, _: &[Proc]) {}
    fn next_deadline(&self) -> Option<skein_lib::Time> {
        None
    }
    fn overdue(&self, _: skein_lib::Time) -> Option<String> {
        None
    }
    fn passed(&self) -> bool {
        true
    }
}
thread_local! { static SHARED: RefCell<Option<Typed>> = const { RefCell::new(None) }; }
pub fn share(transport: Option<Typed>) {
    SHARED.with(|shared| *shared.borrow_mut() = transport);
}
#[must_use]
pub fn transport() -> Typed {
    SHARED.with(|shared| shared.borrow().as_ref().expect("world's forge").clone())
}
fn git(spawn: &skein_io::kernel::Spawn, inherited: &Inherited) -> Proc {
    Proc::Git(Box::new(temper_fake_git_hosted::git(spawn, inherited, transport())))
}
pub(crate) fn program() -> HostedProgram<Proc> {
    HostedProgram { program: b"/usr/bin/git".as_slice().into(), make: git, instances: 1, operations: 1 }
}
pub(crate) fn roots(spawn: &skein_io::kernel::Spawn) -> Vec<StartupRoot> {
    vec![StartupRoot {
        name: b"worker".as_slice().into(),
        path: format!("/proc/self/fd/{}", spawn.root.raw()).into_bytes().into(),
    }]
}

fn routed(spawn: &skein_io::kernel::Spawn, inherited: &Inherited) -> Proc {
    match temper_fake_git::check(spawn) {
        Ok(_) | Err(temper_fake_git::Refusal::Usage) => {}
        Err(temper_fake_git::Refusal::Broken(rule)) => panic!("git's spawn broke {rule}"),
    }
    Proc::GitRoute(Box::new(temper_fake_git::Git::new(
        spawn,
        temper_fake_git_hosted::start(inherited),
        temper_fake_git::Route::new(crate::forge::protocol_limits()),
    )))
}
pub(crate) fn route_program() -> HostedProgram<Proc> {
    HostedProgram { make: routed, ..program() }
}
