//! Smith's fake Claude Code and shipped MCP relay (domain/testing.md, 2.4, 4.1).
use crate::world::Proc;
use skein_io::kernel::Spawn;
use skein_world::{HostedProgram, Inherited};
use std::{os::unix::ffi::OsStrExt, path::Path};

/// The hosted temper executable's relay entry point, as main.rs dispatches it.
pub(crate) const RELAY: &[u8] = b"/real-loop/temper";
pub(crate) const PROGRAM: &[u8] = b"/real-loop/claude";

// Runtime provisioning creates the direct key account before a program is declared.
pub const ACCOUNT: u64 = 2;
pub const SOURCE: &str = "live";

/// Sign in, declare the private source and join its worker-side verification.
/// Account documents stay at calls 2 and 6 for the source's focused story.
#[must_use]
pub fn sign_in() -> Vec<crate::calls::Step> {
    use crate::{
        argv,
        calls::{D1, call, number, sign_in},
    };
    vec![
        sign_in(D1),
        call(D1, argv!["declare", "2", "claude_code", SOURCE, "--key", "declare-program"]),
        call(D1, argv!["accounts", "--json"]),
        call(D1, argv!["verify", number(ACCOUNT), "--source", "program", "--json"]),
        call(D1, argv!["wait", "accounts", "--until", "settled", "--json"]),
        call(D1, argv!["wait", "accounts", "--until", "usable", "--json"]),
        call(D1, argv!["accounts", "--json"]),
    ]
}

pub(crate) fn program(path: &[u8]) -> HostedProgram<Proc> {
    let factory = smith_fake_claude::hosted(path);
    HostedProgram {
        program: factory.program,
        make: fake,
        // A status probe may run beside a conversation, as in smith's program world.
        instances: 2,
        operations: factory.operations,
    }
}

fn fake(spawn: &Spawn, inherited: &Inherited) -> Proc {
    let launch = launch(spawn);
    Proc::Claude(Box::new((smith_fake_claude::hosted(&launch.program).make)(&launch, inherited)))
}

/// The hosted fake uses filesystem names; Spawn's cwd is relative to its root descriptor.
pub(crate) fn launch(spawn: &Spawn) -> Spawn {
    let root = std::fs::read_link(format!("/proc/self/fd/{}", spawn.root.raw())).expect("fake launch root");
    let directory = root.join(std::ffi::OsStr::from_bytes(&spawn.dir));
    Spawn {
        program: spawn.program.clone(),
        args: spawn.args.clone(),
        env: spawn.env.clone(),
        root: spawn.root,
        dir: directory.as_os_str().as_bytes().into(),
        pipes: spawn.pipes.clone(),
    }
}

pub(crate) fn relay(path: &[u8]) -> HostedProgram<Proc> {
    HostedProgram {
        program: path.into(),
        make: make_relay,
        instances: 1,
        operations: smith_agent_shell::relay::operations(),
    }
}

fn make_relay(spawn: &Spawn, inherited: &Inherited) -> Proc {
    assert_eq!(spawn.args.first().map(AsRef::as_ref), Some(smith_agent_shell::relay::FLAG.as_bytes()));
    let pipe = |number| inherited.pipes.iter().find(|(child, _)| *child == number).map(|(_, fd)| *fd);
    Proc::Relay(Box::new(smith_agent_shell::relay::Process::new(
        &spawn.args,
        &spawn.env,
        smith_agent_shell::relay::Faces {
            stdin: pipe(0).expect("relay input"),
            stdout: pipe(1).expect("relay output"),
            stderr: pipe(2),
            signal: inherited.signal,
        },
    )))
}

/// Lay a private, signed-in fake source; its presence opts this scratch into program configuration.
pub fn source(scratch: &skein_scratch::Scratch, name: &str, story: &smith_fake_claude::Story) -> std::io::Result<()> {
    let directory = smith_agent_shell::program::prepare(&scratch.path().join("programs"), name)
        .map_err(|why| std::io::Error::other(why.to_string()))?;
    crate::fixtures::private(&directory.join(smith_fake_claude::STORY_FILE), &story.encode())?;
    crate::fixtures::private(&directory.join(smith_fake_claude::LOGIN_FILE), b"claude.ai")
}

pub(crate) fn configured(directory: &Path) -> bool {
    directory.join("programs").is_dir()
}
