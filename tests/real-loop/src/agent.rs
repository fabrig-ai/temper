//! The shipped smith agent, hosted on real pipes and independently opened roots.
use crate::{fixtures, world::Proc};
use skein_io::kernel::Spawn;
use skein_world::{Inherited, StartupRoot};
use std::{os::unix::ffi::OsStrExt, path::Path};
pub(crate) fn agent(spawn: &Spawn, inherited: &Inherited) -> Proc {
    let configuration = Path::new(std::ffi::OsStr::from_bytes(&spawn.args[2]));
    let configuration = smith_agent_shell::config::read(configuration).expect("shipped agent configuration");
    let stderr = inherited.pipes.iter().find(|(child, _)| *child == 2).expect("stderr").1;
    let (errors, capture) = smith_agent_process_world::AgentErrors::pipe(stderr);
    Proc::Agent(Box::new(
        capture.host(
            smith_agent_shell::Agent::new(
                configuration,
                smith_agent_shell::Resources {
                    input: inherited.pipes.iter().find(|(child, _)| *child == 0).expect("stdin").1,
                    output: inherited.pipes.iter().find(|(child, _)| *child == 1).expect("stdout").1,
                    error: Some(stderr),
                    signals: inherited.signal,
                    seed: fixtures::SEED,
                    pid: 101,
                    trace: None,
                    // Start carries the checkout paths after spawn. Open those paths in
                    // Start order, just as the shipped process does on this filesystem.
                    roots: None,
                    programs: None,
                },
                Box::new(errors),
            )
            .expect("hosted shipped agent"),
        ),
    ))
}
pub(crate) fn roots(_spawn: &Spawn) -> Vec<StartupRoot> {
    Vec::new()
}
