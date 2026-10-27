//! Restart and shutdown bounds (protocol/configuration.md, 7).
use jig_protocol_config::names::{Name, Unit};
/// Every name this parser takes, in the order of its owner prefixes.
pub const NAMES: [Name; 8] = [
    Name {
        name: "box.backoff_ms",
        unit: Unit::Milliseconds,
        line: "The first wait before the box restarts a step that exited, doubling up to `box.backoff_max_ms`.",
    },
    Name {
        name: "box.backoff_max_ms",
        unit: Unit::Milliseconds,
        line: "The longest wait before the box restarts a step, below `box.window_ms`.",
    },
    Name {
        name: "box.restarts",
        unit: Unit::Count,
        line: "A step's exits within `box.window_ms` past which the box gives up, at most 64.",
    },
    Name {
        name: "box.window_ms",
        unit: Unit::Milliseconds,
        line: "The window in which a step's exits count against `box.restarts`.",
    },
    Name {
        name: "box.grace_ms",
        unit: Unit::Milliseconds,
        line: "A step's grace after each of its stop's two signals.",
    },
    Name {
        name: "box.line_bytes",
        unit: Unit::Bytes,
        line: "A line of the box's pipe either way, its line feed among them, at least 64; a longer line is dropped.",
    },
    Name {
        name: "box.log_bytes",
        unit: Unit::Bytes,
        line: "A step's log file, past which it moves to the same name ending `.log.1` and a new one starts.",
    },
    Name {
        name: "box.tail_lines",
        unit: Unit::Count,
        line: "The lines of a step's log the box keeps and writes under a failure's line, from 1 to 100.",
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

use skein_lib::{Duration, Queue, Time};
/// The box limits, durations in milliseconds at configuration boundaries.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Limits {
    pub backoff: Duration,
    pub backoff_max: Duration,
    pub restarts: u32,
    pub window: Duration,
    pub grace: Duration,
    pub line_bytes: u32,
    pub log_bytes: u32,
    pub tail_lines: u32,
}
/// The most restarts whose exit times can be retained.
pub const MAX_RESTARTS: u32 = 64;
/// One timestamp, the longest diagnostic piece, and its added LF.
pub const RECORD_BYTES: u32 = 21_u32
    .checked_add(crate::host::LINE_BYTES)
    .expect("record piece fits u32")
    .checked_add(1)
    .expect("record LF fits u32");
/// The largest failure tail retained per step.
pub const MAX_TAIL_LINES: u32 = 100;
/// The failed cross-check or restart bound.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Unfit {
    Backoff,
    Window,
    Restarts,
    LineBytes,
    LogBytes,
    TailLines,
}
/// An unknown name or a value beyond u32.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Refused {
    Unknown,
    Range,
}
impl Unfit {
    /// The names to report for this failure.
    #[must_use]
    pub fn names(self) -> &'static [&'static [u8]] {
        match self {
            Self::Backoff => &[b"box.backoff_ms", b"box.backoff_max_ms"],
            Self::Window => &[b"box.backoff_max_ms", b"box.window_ms"],
            Self::Restarts => &[b"box.restarts"],
            Self::LineBytes => &[b"box.line_bytes"],
            Self::LogBytes => &[b"box.log_bytes"],
            Self::TailLines => &[b"box.tail_lines"],
        }
    }
}
/// The shipped profile (forge-setup decision 5).
#[must_use]
pub fn small() -> Limits {
    Limits {
        backoff: Duration::from_millis(500),
        backoff_max: Duration::from_secs(30),
        restarts: 5,
        window: Duration::from_secs(300),
        grace: Duration::from_secs(10),
        line_bytes: 256,
        log_bytes: 4_194_304,
        tail_lines: 10,
    }
}
/// Applies one whole dotted name.
pub fn apply(limits: &mut Limits, name: &[u8], value: u64) -> Result<(), Refused> {
    let row = find(name).ok_or(Refused::Unknown)?;
    field(limits, row.name.as_bytes(), Some(value))?;
    Ok(())
}
/// A named limit's value, a duration's in milliseconds; none for a name
/// with no row.
#[must_use]
pub fn value(limits: &Limits, name: &[u8]) -> Option<u64> {
    let row = find(name)?;
    let mut copy = *limits;
    field(&mut copy, row.name.as_bytes(), None).ok()
}
fn field(limits: &mut Limits, name: &[u8], to: Option<u64>) -> Result<u64, Refused> {
    match name {
        b"box.backoff_ms" => millis(&mut limits.backoff, to),
        b"box.backoff_max_ms" => millis(&mut limits.backoff_max, to),
        b"box.restarts" => count(&mut limits.restarts, to),
        b"box.window_ms" => millis(&mut limits.window, to),
        b"box.grace_ms" => millis(&mut limits.grace, to),
        b"box.line_bytes" => count(&mut limits.line_bytes, to),
        b"box.log_bytes" => count(&mut limits.log_bytes, to),
        b"box.tail_lines" => count(&mut limits.tail_lines, to),
        _ => Err(Refused::Unknown),
    }
}

fn count(field: &mut u32, to: Option<u64>) -> Result<u64, Refused> {
    if let Some(value) = to {
        *field = u32::try_from(value).or(Err(Refused::Range))?;
    }
    Ok(u64::from(*field))
}
fn millis(field: &mut Duration, to: Option<u64>) -> Result<u64, Refused> {
    if let Some(value) = to {
        let value = u32::try_from(value).or(Err(Refused::Range))?;
        *field = Duration::from_millis(u64::from(value));
    }
    Ok(field.as_nanos() / 1_000_000)
}
/// Checks relationships in their documented order.
pub fn check(limits: &Limits) -> Result<(), Unfit> {
    if limits.backoff > limits.backoff_max {
        return Err(Unfit::Backoff);
    }
    if limits.backoff_max >= limits.window {
        return Err(Unfit::Window);
    }
    if limits.restarts > MAX_RESTARTS {
        return Err(Unfit::Restarts);
    }
    if limits.line_bytes < 64 {
        return Err(Unfit::LineBytes);
    }
    if limits.log_bytes < RECORD_BYTES {
        return Err(Unfit::LogBytes);
    }
    if limits.tail_lines == 0 || limits.tail_lines > MAX_TAIL_LINES {
        return Err(Unfit::TailLines);
    }
    Ok(())
}
/// Machine state, exit history and two timer slots per step.
#[must_use]
pub fn worst_case(limits: &Limits, steps: u32) -> Option<u64> {
    let state = u64::try_from(size_of::<crate::machine::State>()).ok()?;
    let history = Queue::<Time>::worst_case(limits.restarts.checked_add(1)?)?;
    let timers = u64::try_from(size_of::<Option<Time>>()).ok()?.checked_mul(2)?;
    let groups = u64::try_from(size_of::<crate::machine::GroupState>()).ok()?;
    state.checked_add(groups)?.checked_add(history)?.checked_add(timers)?.checked_mul(u64::from(steps))
}
