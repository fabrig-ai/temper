//! Hosted agent construction from startup values.
use skein_sim_world::{Inherited, PipeHost, PipeWriter};
/// A hosted agent as temper agent runs it, from its configuration as values.
#[must_use]
pub fn agent(
    inherited: &Inherited,
    configuration: smith_agent_shell::config::Configuration,
    seed: u64,
    pid: u32,
) -> crate::Signalled<PipeHost<smith_agent_shell::Agent>> {
    let stderr = inherited.pipes.iter().find(|(child, _)| *child == 2).expect("stderr").1;
    let (errors, capture) = PipeWriter::new(stderr, 4096);
    crate::Signalled::new(
        capture.host(
            smith_agent_shell::Agent::new(
                configuration,
                smith_agent_shell::Resources {
                    input: inherited.pipes.iter().find(|(child, _)| *child == 0).expect("stdin").1,
                    output: inherited.pipes.iter().find(|(child, _)| *child == 1).expect("stdout").1,
                    error: Some(stderr),
                    signals: inherited.signal,
                    seed,
                    pid,
                    trace: None,
                    roots: Some(inherited.roots.clone().into_boxed_slice()),
                    programs: None,
                },
                Box::new(errors),
            )
            .expect("hosted agent"),
        ),
        inherited.dir,
    )
}
