//! Checked agent metadata and effect-free programs-directory preflight (configuration.md, 5).
use std::{os::unix::ffi::OsStrExt, path::Path};
use temper_service::{self as service, settings::Agent};
fn path(bytes: &[u8]) -> &Path {
    Path::new(std::ffi::OsStr::from_bytes(bytes))
}
pub(crate) fn read(configuration: &Path) -> Result<Agent, String> {
    let configuration = smith_agent_shell::config::read(configuration)?;
    let limits = &configuration.service.limits;
    let host_limits = smith_local_service::profile::host_limits(&configuration.profile, limits)
        .map_err(|_| "agent host limits refused")?;
    let channel = limits.channel;
    let mut protocol = smith_host_protocol::Limits {
        bodies: channel.bodies,
        charter: channel.charter,
        transcript: channel.transcript,
        endpoints: channel.endpoints,
        channel: channel.channel,
        start_bytes: channel.start_bytes,
        calls: host_limits.calls,
    };
    protocol.channel.output_bytes = smith_host_protocol::output_bytes(&protocol).ok_or("agent output bound")?;
    let mut models = Vec::new();
    for endpoint in &configuration.declarations.endpoints {
        for model in &endpoint.models {
            models.push(jig_domain_host::ModelBound {
                name: model.name.clone(),
                window: model.window,
                output: model.output,
            });
        }
    }
    let policy = configuration.profile.policy;
    let message_bytes = limits
        .domain
        .run
        .message_bytes
        .checked_sub(u32::try_from(temper_worker_domain::SEPARATOR).expect("message separator fits u32"))
        .ok_or("agent message_bytes: too small for the rendered separator")?;
    let programs = configuration
        .programs
        .into_iter()
        .map(|(name, program)| service::settings::Program {
            adapter: match name {
                smith_agent_shell::config::ProgramName::ClaudeCode => service::settings::Adapter::ClaudeCode,
            },
            path: program.path.as_os_str().as_bytes().into(),
            directory: program.directory.as_os_str().as_bytes().into(),
        })
        .collect();
    Ok(Agent {
        tool_payload: configuration.profile.declared.tool_payload,
        programs,
        endpoints: configuration.service.channel_endpoints,
        limits: protocol,
        host_limits,
        bounds: jig_domain_host::Bounds {
            program_endpoints: configuration.endpoint_programs.keys().copied().collect::<Vec<_>>().into_boxed_slice(),
            models: models.into_boxed_slice(),
            turns: policy.max_turns,
            spend: policy.max_spend,
            time: policy.max_time,
            waiting: policy.max_waiting,
            checks: policy.check_timeout,
            message_bytes,
            inbox: policy.inbox,
            unacknowledged: policy.unacknowledged,
        },
    })
}
/// Read declarations and check existing programs directories at their names (io.md, 5.3).
/// Missing directories stay missing; neither this nor smith's check reads any login.
pub(crate) fn preflight(named: &service::Named) -> Result<Agent, String> {
    let worker = named.worker.as_ref().ok_or("workers: standalone takes one worker")?;
    preflight_worker(worker)
}
pub(crate) fn preflight_worker(worker: &service::startup::NamedWorker) -> Result<Agent, String> {
    let agent = read(path(&worker.agent)).map_err(|why| format!("workers[0].agent.configuration: {why}"))?;
    for program in &agent.programs {
        smith_agent_shell::program::private_directory(path(&program.directory))
            .map_err(|why| program_directory_refusal(&why))?;
    }
    Ok(agent)
}
pub(crate) fn program_directory_refusal(why: &smith_agent_shell::program::Refusal) -> String {
    format!("workers[0].agent.configuration: programs.claude_code.directory: {why}")
}
