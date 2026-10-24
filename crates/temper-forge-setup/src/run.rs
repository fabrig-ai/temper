//! Pure admission checks for a locked owned process (protocol/setup.md, 6.4).
use crate::{
    place::{File, Place},
    setup::Owned,
    spec::{self, Command, Spec, Which},
    state::State,
    upgrade::{self, Course},
};
use alloc::boxed::Box;
use skein_forge::forgejo::pin::{RUNNER_VERSION, VERSION, names_forgejo, names_runner};
/// Why a run did not start; credentials never enter a refusal.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Refusal {
    NoRunner,
    NotPrepared,
    Unfinished,
    Runner { recorded: bool },
    Versions { forgejo: Box<[u8]>, runner: Option<Box<[u8]>>, course: Course },
    Binary { printed: Box<str> },
}
/// Specifications and file names checked under the process's lock.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Launch {
    pub which: Which,
    pub runner: bool,
    pub program: Box<[u8]>,
    pub lock: Box<[u8]>,
    pub state: Box<[u8]>,
    pub version: Spec,
    pub exec: Spec,
}
/// Selects the existing command specs; an absent runner is refused first.
pub fn launch(owned: &Owned, place: &Place, which: Which) -> Result<Launch, Refusal> {
    if which == Which::Runner && owned.runner.is_none() {
        return Err(Refusal::NoRunner);
    }
    let (lock, command) = match which {
        Which::Forgejo => (File::ForgejoLock, Command::Web),
        Which::Runner => (File::RunnerLock, Command::Daemon),
    };
    let exec = spec::command(command, owned, place);
    Ok(Launch {
        which,
        runner: owned.runner.is_some(),
        program: exec.program.clone(),
        lock: place.path(lock),
        state: place.path(File::State),
        version: spec::version(which, owned, place),
        exec,
    })
}
/// Requires a completed preparation at every configured binary's pin.
pub fn admit_state(launch: &Launch, state: Option<&State>) -> Result<(), Refusal> {
    let state = state.ok_or(Refusal::NotPrepared)?;
    if !state.prepared {
        return Err(Refusal::Unfinished);
    }
    if launch.runner != state.runner.is_some() {
        return Err(Refusal::Runner { recorded: state.runner.is_some() });
    }
    let mut course = upgrade::course(&state.forgejo, VERSION.as_bytes());
    if launch.runner
        && let Some(runner) = &state.runner
    {
        course = course.max(upgrade::course(runner, RUNNER_VERSION.as_bytes()));
    }
    if course != Course::Same {
        return Err(Refusal::Versions { forgejo: state.forgejo.clone(), runner: state.runner.clone(), course });
    }
    Ok(())
}
/// Requires the first version line to name the selected binary's pin.
pub fn admit_binary(launch: &Launch, printed: &str) -> Result<(), Refusal> {
    let accepted = match launch.which {
        Which::Forgejo => names_forgejo(printed),
        Which::Runner => names_runner(printed),
    };
    if accepted { Ok(()) } else { Err(Refusal::Binary { printed: printed.into() }) }
}

/// Checks the inherited fd 3 address; absent means Forgejo binds as usual.
pub fn admit_listener(owned: &Owned, found: Option<core::net::SocketAddr>) -> Result<(), crate::problem::Problem> {
    match found {
        Some(found) if found != owned.listen => {
            Err(crate::problem::Problem::Listener { found, expected: owned.listen })
        }
        Some(_) | None => Ok(()),
    }
}
