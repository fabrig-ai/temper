//! Managed orgs and reconcile's deadline and wait (protocol/configuration.md, 7).
use skein_forge_setup::limits::Limits;

/// A fetched binary's cap; largest pin: Forgejo 17.0.0 amd64, 122,053,192 bytes.
pub const FETCH_BYTES: u64 = 268_435_456;
/// Reads `setup.fetch_bytes`, which must be above zero.
pub fn fetch_bytes(value: u64) -> Result<u64, Refused> {
    if value == 0 { Err(Refused::Range) } else { Ok(value) }
}
/// Small-profile host admission, beside the pure plan profile returned by `small()`.
pub mod small {
    // Four forges, each with one writing run and one Reachable beside it.
    pub const RUNS: u32 = 8;
    // A manifest return and an installation return for each of four forges.
    pub const STATES: u32 = 8;
}
/// The engine host's setup limits (protocol/configuration.md, 7).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Host {
    pub runs: u32,
    pub states: u32,
    pub state_life: Duration,
    pub orgs: u32,
    pub reachable: u32,
    pub repositories: u32,
    pub deadline: Duration,
    pub backoff: Duration,
    pub backoff_max: Duration,
}
/// The host reuses the existing plan profile.
#[must_use]
pub fn host() -> Host {
    let plan = small();
    Host {
        runs: small::RUNS,
        states: small::STATES,
        state_life: Duration::from_secs(3600),
        orgs: plan.orgs,
        reachable: plan.reachable,
        repositories: plan.repositories,
        deadline: plan.deadline,
        backoff: plan.backoff,
        backoff_max: plan.backoff_max,
    }
}
impl Host {
    /// The same pure-plan limits used by the command's existing profile.
    #[must_use]
    pub fn plan(&self) -> Limits {
        Limits {
            orgs: self.orgs,
            reachable: self.reachable,
            files: 2,
            file_bytes: 4096,
            repositories: self.repositories,
            deadline: self.deadline,
            backoff: self.backoff,
            backoff_max: self.backoff_max,
        }
    }
}
/// Applies host overrides, including the pure plan's dotted names.
pub fn apply_host(limits: &mut Host, name: &[u8], value: u64) -> Result<(), Refused> {
    match name {
        b"setup.runs" | b"setup.states" => {
            let count = u32::try_from(value).or(Err(Refused::Range))?;
            if count == 0 {
                return Err(Refused::Range);
            }
            if name == b"setup.runs" {
                limits.runs = count;
            } else {
                limits.states = count;
            }
        }
        b"setup.state_life_ms" => {
            if value == 0 || value > 3_600_000 {
                return Err(Refused::Range);
            }
            limits.state_life = duration(value)?;
        }
        _ => {
            let mut plan = limits.plan();
            apply(&mut plan, name, value)?;
            limits.orgs = plan.orgs;
            limits.deadline = plan.deadline;
            limits.backoff = plan.backoff;
            limits.backoff_max = plan.backoff_max;
        }
    }
    Ok(())
}

use skein_lib::Duration;
/// Why an override cannot be applied.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Refused {
    Unknown,
    Range,
}
/// The shipped profile (forge-setup decision 5).
#[must_use]
pub fn small() -> Limits {
    // Legacy command default; checked startup supplies forge.client.repositories.
    let repositories = 2;
    Limits {
        orgs: 8,
        reachable: 8, // temper-10 supplies picker.page.
        files: 2,
        file_bytes: 4096,
        repositories,
        deadline: Duration::from_secs(120),
        backoff: Duration::from_millis(100),
        backoff_max: Duration::from_secs(5),
    }
}
/// Applies a dotted setup override, refusing zero and overflow.
pub fn apply(limits: &mut Limits, name: &[u8], value: u64) -> Result<(), Refused> {
    if ![
        b"setup.orgs".as_slice(),
        b"setup.deadline_ms",
        b"setup.backoff_ms",
        b"setup.backoff_max_ms",
        b"forge.client.repositories",
    ]
    .contains(&name)
    {
        return Err(Refused::Unknown);
    }
    if value == 0 {
        return Err(Refused::Range);
    }
    match name {
        b"forge.client.repositories" => limits.repositories = u32::try_from(value).or(Err(Refused::Range))?,
        b"setup.orgs" => limits.orgs = u32::try_from(value).or(Err(Refused::Range))?,
        b"setup.deadline_ms" => limits.deadline = duration(value)?,
        b"setup.backoff_ms" => limits.backoff = duration(value)?,
        b"setup.backoff_max_ms" => limits.backoff_max = duration(value)?,
        _ => return Err(Refused::Unknown),
    }
    Ok(())
}
fn duration(value: u64) -> Result<Duration, Refused> {
    match value.checked_mul(1_000_000) {
        Some(n) => Ok(Duration::from_nanos(n)),
        None => Err(Refused::Range),
    }
}
