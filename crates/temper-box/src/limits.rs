//! Restart and shutdown bounds (protocol/configuration.md, 7).
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
    if ![
        b"box.backoff_ms".as_slice(),
        b"box.backoff_max_ms",
        b"box.restarts",
        b"box.window_ms",
        b"box.grace_ms",
        b"box.line_bytes",
        b"box.log_bytes",
        b"box.tail_lines",
    ]
    .contains(&name)
    {
        return Err(Refused::Unknown);
    }
    let value = u32::try_from(value).or(Err(Refused::Range))?;
    let duration = Duration::from_millis(u64::from(value));
    match name {
        b"box.backoff_ms" => limits.backoff = duration,
        b"box.backoff_max_ms" => limits.backoff_max = duration,
        b"box.restarts" => limits.restarts = value,
        b"box.window_ms" => limits.window = duration,
        b"box.grace_ms" => limits.grace = duration,
        b"box.line_bytes" => limits.line_bytes = value,
        b"box.log_bytes" => limits.log_bytes = value,
        b"box.tail_lines" => limits.tail_lines = value,
        _ => return Err(Refused::Unknown),
    }
    Ok(())
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
