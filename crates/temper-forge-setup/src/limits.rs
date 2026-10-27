//! Managed orgs and reconcile's deadline and wait (protocol/configuration.md, 7).
use jig_protocol_config::names::{Name, Unit};
use skein_forge_setup::limits::Limits;
/// Every name this parser takes, in the order of its owner prefixes.
pub const NAMES: [Name; 8] = [
    Name {
        name: "setup.orgs",
        unit: Unit::Count,
        line: "The orgs a managed forge's setup entry may name; more refuse the start.",
    },
    Name {
        name: "setup.deadline_ms",
        unit: Unit::Milliseconds,
        line: "A forge's setup run, its wait for the forge among it, above `setup.backoff_max_ms`; past it, the run stops.",
    },
    Name {
        name: "setup.backoff_ms",
        unit: Unit::Milliseconds,
        line: "The first wait before a setup request is made again, doubling up to `setup.backoff_max_ms`.",
    },
    Name {
        name: "setup.backoff_max_ms",
        unit: Unit::Milliseconds,
        line: "The longest wait between two tries of a setup request, below `setup.deadline_ms`.",
    },
    Name {
        name: "setup.runs",
        unit: Unit::Count,
        line: "Setup runs at once, a forge's writing run and its reachability read both counted; a request past them is answered busy.",
    },
    Name {
        name: "setup.states",
        unit: Unit::Count,
        line: "One-time states held for a GitHub App's manifest and installation returns; past it, the oldest goes.",
    },
    Name {
        name: "setup.state_life_ms",
        unit: Unit::Milliseconds,
        line: "A one-time return state's life, at most an hour; a return after it fails.",
    },
    Name {
        name: "setup.fetch_bytes",
        unit: Unit::Bytes,
        line: "A fetched binary, written to its part file as it streams; one past it removes the part file and is refused, naming the URL.",
    },
];

#[expect(clippy::manual_find, reason = "step code uses a loop rather than a closure")]
fn find(name: &[u8]) -> Option<&'static Name> {
    for row in &NAMES {
        if row.name.as_bytes() == name {
            return Some(row);
        }
    }
    None
}

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
    // The resolved service input was accepted through the plan without changing the host.
    if name == b"forge.client.repositories" {
        return apply(&mut limits.plan(), name, value);
    }
    let row = find(name).ok_or(Refused::Unknown)?;
    if row.name == "setup.fetch_bytes" {
        return Err(Refused::Unknown);
    }
    let mut fetch = FETCH_BYTES;
    field(limits, &mut fetch, row.name.as_bytes(), Some(value))?;
    Ok(())
}
/// Applies the fetch override used by `check::checked`.
pub fn apply_fetch_bytes(fetch: &mut u64, name: &[u8], value: u64) -> Result<(), Refused> {
    let row = find(name).ok_or(Refused::Unknown)?;
    if row.name != "setup.fetch_bytes" {
        return Err(Refused::Unknown);
    }
    field(&mut host(), fetch, row.name.as_bytes(), Some(value))?;
    Ok(())
}
/// A named limit's value, a duration's in milliseconds; none for a name
/// with no row. The fetched binary's cap is kept outside `Host`.
#[must_use]
pub fn value(host: &Host, fetch_bytes: u64, name: &[u8]) -> Option<u64> {
    let row = find(name)?;
    let mut copy = *host;
    let mut fetch = fetch_bytes;
    field(&mut copy, &mut fetch, row.name.as_bytes(), None).ok()
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
    // The start hands this resolved service input to the plan, not a setup row.
    if name != b"forge.client.repositories" {
        let row = find(name).ok_or(Refused::Unknown)?;
        if row.name == "setup.runs"
            || row.name == "setup.states"
            || row.name == "setup.state_life_ms"
            || row.name == "setup.fetch_bytes"
        {
            return Err(Refused::Unknown);
        }
    }
    let mut copy = Host {
        orgs: limits.orgs,
        repositories: limits.repositories,
        deadline: limits.deadline,
        backoff: limits.backoff,
        backoff_max: limits.backoff_max,
        ..host()
    };
    let mut fetch = FETCH_BYTES;
    field(&mut copy, &mut fetch, name, Some(value))?;
    limits.orgs = copy.orgs;
    limits.repositories = copy.repositories;
    limits.deadline = copy.deadline;
    limits.backoff = copy.backoff;
    limits.backoff_max = copy.backoff_max;
    Ok(())
}
fn field(limits: &mut Host, fetch: &mut u64, name: &[u8], to: Option<u64>) -> Result<u64, Refused> {
    match name {
        b"setup.orgs" => count(&mut limits.orgs, to),
        b"setup.deadline_ms" => millis(&mut limits.deadline, to),
        b"setup.backoff_ms" => millis(&mut limits.backoff, to),
        b"setup.backoff_max_ms" => millis(&mut limits.backoff_max, to),
        b"setup.runs" => count(&mut limits.runs, to),
        b"setup.states" => count(&mut limits.states, to),
        b"setup.state_life_ms" => {
            if let Some(value) = to
                && value > 3_600_000
            {
                return Err(Refused::Range);
            }
            millis(&mut limits.state_life, to)
        }
        b"setup.fetch_bytes" => {
            if let Some(value) = to {
                *fetch = fetch_bytes(value)?;
            }
            Ok(*fetch)
        }
        // Resolved service input, deliberately absent from NAMES.
        b"forge.client.repositories" => count(&mut limits.repositories, to),
        _ => Err(Refused::Unknown),
    }
}
fn count(field: &mut u32, to: Option<u64>) -> Result<u64, Refused> {
    if let Some(value) = to {
        if value == 0 {
            return Err(Refused::Range);
        }
        *field = u32::try_from(value).or(Err(Refused::Range))?;
    }
    Ok(u64::from(*field))
}
fn millis(field: &mut Duration, to: Option<u64>) -> Result<u64, Refused> {
    if let Some(value) = to {
        if value == 0 {
            return Err(Refused::Range);
        }
        *field = duration(value)?;
    }
    Ok(field.as_nanos() / 1_000_000)
}
fn duration(value: u64) -> Result<Duration, Refused> {
    match value.checked_mul(1_000_000) {
        Some(n) => Ok(Duration::from_nanos(n)),
        None => Err(Refused::Range),
    }
}
