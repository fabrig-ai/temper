//! Smith's fake Claude Code and shipped MCP relay (domain/testing.md, 2.4, 4.1).
use crate::world::Proc;
use skein_io::kernel::Spawn;
use skein_world::{HostedProgram, Inherited};
use std::path::Path;

/// The hosted temper executable's relay entry point, as main.rs dispatches it.
pub(crate) const RELAY: &[u8] = b"/real-loop/temper";
pub(crate) const PROGRAM: &[u8] = b"/real-loop/claude";

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
    Proc::Claude(Box::new((smith_fake_claude::hosted(&spawn.program).make)(spawn, inherited)))
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
