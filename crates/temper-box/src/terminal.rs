//! Pure terminal words and targets (protocol/setup.md, 7.2).
use crate::{
    layout::{self, Placed},
    lines::joined,
    machine::Why,
    pipe,
};
use alloc::boxed::Box;
use skein_lib::{Decimal, Duration, List, Wall};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fact {
    Starting { step: usize },
    Started { step: usize },
    GroupStarting { forge: u16 },
    GroupStarted { forge: u16 },
    GroupFailed { forge: u16, why: Why },
    GroupStopped { forge: u16 },
    Restarting { step: usize, why: Why, delay: Duration },
    GivingUp { step: usize, why: Why },
    Stopping,
    Stopped,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    Stdout,
    Stderr,
}
#[derive(PartialEq, Eq, Debug)]
pub struct Item {
    pub target: Target,
    pub bytes: Box<[u8]>,
}
pub const PRELUDE: &[u8] = b"\nOpen this link in a browser on this machine. It signs you in as this\ntemper's owner, once and for a short while, so keep it private.\nOpened again once spent, it prints a fresh one here.\n\n";
pub const FOOTER: &[u8] = b"\nCtrl-C stops temper.\n\n";
pub const REFUSAL: &[u8] = b"box: serve: sign-in link too long\n";
/// Largest fixed item, also allowing the ten-byte UTC prefix.
pub const FIXED_BYTES: u32 = 256;

#[derive(Debug)]
pub struct Formatter {
    first_printed: bool,
}
#[expect(clippy::new_without_default, reason = "the programming model uses explicit constructors")]
impl Formatter {
    #[must_use]
    pub fn new() -> Self {
        Self { first_printed: false }
    }
    #[must_use]
    pub fn block_entries(&self) -> u32 {
        if self.first_printed { 2 } else { 3 }
    }
    /// Caller reserves `block_entries` before extracting the line. State changes
    /// only after the complete adjacent block has entered the terminal FIFO.
    #[must_use]
    pub fn block(&self, url: Box<[u8]>, wall: Wall) -> List<Item> {
        let mut items = List::with_capacity(self.block_entries());
        let words = if self.first_printed {
            self.notice(b"A fresh sign-in link:\n", wall)
        } else {
            Item { target: Target::Stderr, bytes: skein_lib::bytes::copy_of(PRELUDE) }
        };
        items.push(words).expect("block count");
        items.push(Item { target: Target::Stdout, bytes: url }).expect("block count");
        if !self.first_printed {
            items.push(Item { target: Target::Stderr, bytes: skein_lib::bytes::copy_of(FOOTER) }).expect("block count");
        }
        items
    }
    pub fn admitted_block(&mut self) {
        self.first_printed = true;
    }
    #[must_use]
    pub fn fact(&self, fact: Fact, steps: &[Placed], wall: Wall) -> Item {
        if let Fact::Started { step } = fact {
            return self.notices(steps.get(step).expect("started step").step.started_notice(), wall);
        }
        if let Fact::GroupStarted { forge } = fact {
            for placed in steps {
                if let Some(asked) = placed.asked
                    && asked.forge == forge
                    && !placed.step.started_notice().is_empty()
                {
                    return self.notices(placed.step.started_notice(), wall);
                }
            }
        }
        let words = match fact {
            Fact::Starting { step } => joined(&[b"Starting ", steps.get(step).expect("step fact").step.name(), b"\n"]),
            Fact::Started { .. } => unreachable!("started notice handled above"),
            Fact::GroupStarting { forge } => {
                joined(&[layout::label(steps, forge).expect("group label"), b": starting\n"])
            }
            Fact::GroupStarted { forge } => {
                joined(&[layout::label(steps, forge).expect("group label"), b": started\n"])
            }
            Fact::GroupFailed { forge, why: reason } => {
                joined(&[layout::label(steps, forge).expect("group label"), b": failed, ", &why(reason, steps), b"\n"])
            }
            Fact::GroupStopped { forge } => {
                joined(&[layout::label(steps, forge).expect("group label"), b": stopped\n"])
            }
            Fact::Restarting { why: reason, delay, .. } => {
                joined(&[&why(reason, steps), b"; restarting in ", &seconds(delay), b" s\n"])
            }
            Fact::GivingUp { why: reason, .. } => joined(&[&why(reason, steps), b"; giving up\n"]),
            Fact::Stopping => skein_lib::bytes::copy_of(b"Stopping\n"),
            Fact::Stopped => skein_lib::bytes::copy_of(b"Stopped\n"),
        };
        self.notice(&words, wall)
    }
    fn notices(&self, lines: &[u8], wall: Wall) -> Item {
        let mut writer = skein_lib::Writer::new(notice_bytes(lines, self.first_printed));
        let mut first = 0;
        for (at, byte) in lines.iter().enumerate() {
            if *byte == b'\n' {
                let end = at.checked_add(1).expect("line end");
                writer
                    .put(&self.notice(lines.get(first..end).expect("notice line"), wall).bytes)
                    .expect("measured notice");
                first = end;
            }
        }
        assert_eq!(first, lines.len(), "public notice lines end in LF");
        Item { target: Target::Stderr, bytes: writer.finish() }
    }
    #[must_use]
    pub fn notice(&self, words: &[u8], wall: Wall) -> Item {
        let bytes = if self.first_printed {
            let day = wall.as_secs().rem_euclid(86_400);
            let hours = pair(day.div_euclid(3_600));
            let minutes = pair(day.div_euclid(60).rem_euclid(60));
            let seconds = pair(day.rem_euclid(60));
            joined(&[&hours, b":", &minutes, b":", &seconds, b"Z ", words])
        } else {
            skein_lib::bytes::copy_of(words)
        };
        Item { target: Target::Stderr, bytes }
    }
}
/// The same exact WHY words feed terminal notices and bounded numeric replies.
#[must_use]
pub fn why(why: Why, steps: &[Placed]) -> Box<[u8]> {
    match why {
        Why::Busy => skein_lib::bytes::copy_of(b"busy"),
        Why::NotAsked => skein_lib::bytes::copy_of(b"not asked"),
        Why::Stopping => skein_lib::bytes::copy_of(b"stopping"),
        Why::Exited { step, exit } => pipe::exited(steps.get(step).expect("step reason").step.name(), exit),
        Why::NotStarted { step } => pipe::not_started(steps.get(step).expect("step reason").step.name()),
        Why::NotReady { step } => joined(&[steps.get(step).expect("step reason").step.name(), b" version unavailable"]),
        Why::NotWritten => skein_lib::bytes::copy_of(pipe::not_written()),
    }
}
/// UTC (10), longest WHY suffix/signal (at most 38), restart words (20),
/// decimal u64 nanoseconds in seconds (at most 21), punctuation and LF all
/// fit 128 extra bytes. Sum independent label/name maxima conservatively.
#[must_use]
pub fn max_bytes(steps: &[Placed]) -> u32 {
    let mut label = 0_u32;
    let mut name = 0_u32;
    let mut notice = 0_u32;
    for placed in steps {
        name = name.max(u32::try_from(placed.step.name().len()).expect("bounded step name"));
        notice = notice.max(u32::try_from(notice_bytes(placed.step.started_notice(), true)).expect("bounded notice"));
        if let Some(words) = &placed.label {
            label = label.max(u32::try_from(words.len()).expect("bounded label"));
        }
    }
    label
        .checked_add(name)
        .expect("lifecycle words")
        .checked_add(128)
        .expect("lifecycle line")
        .max(FIXED_BYTES)
        .max(notice)
}
fn notice_bytes(lines: &[u8], timed: bool) -> usize {
    let mut length = lines.len();
    if timed {
        for byte in lines {
            if *byte == b'\n' {
                length = length.checked_add(10).expect("notice timestamps");
            }
        }
    }
    length
}
fn seconds(delay: Duration) -> Box<[u8]> {
    let nanos = delay.as_nanos();
    let whole = Decimal::of(nanos.div_euclid(1_000_000_000));
    let mut part = nanos.rem_euclid(1_000_000_000);
    if part == 0 {
        return skein_lib::bytes::copy_of(whole.as_bytes());
    }
    let mut fraction = [b'0'; 9];
    for digit in fraction.iter_mut().rev() {
        *digit = b'0'.checked_add(u8::try_from(part.rem_euclid(10)).expect("decimal digit")).expect("ASCII digit");
        part = part.div_euclid(10);
    }
    let mut end = fraction.len();
    for digit in fraction.iter().rev() {
        if *digit != b'0' {
            break;
        }
        end = end.checked_sub(1).expect("nonzero fraction");
    }
    joined(&[whole.as_bytes(), b".", fraction.get(..end).expect("trimmed fraction")])
}
fn pair(value: u64) -> [u8; 2] {
    [
        b'0'.checked_add(u8::try_from(value.div_euclid(10)).expect("two digits")).expect("ASCII digit"),
        b'0'.checked_add(u8::try_from(value.rem_euclid(10)).expect("one digit")).expect("ASCII digit"),
    ]
}

/// Last successfully logged diagnostic pieces, already in echo's spelling.
#[derive(Debug)]
pub(crate) struct Tail {
    lines: skein_lib::Queue<Box<[u8]>>,
}
impl Tail {
    pub fn new(lines: u32) -> Self {
        Self { lines: skein_lib::Queue::with_capacity(lines) }
    }
    pub fn push(&mut self, line: Box<[u8]>) {
        if self.lines.room() == 0 {
            drop(self.lines.pop());
        }
        self.lines.push(line);
    }
    pub fn reset(&mut self) {
        while let Some(line) = self.lines.pop() {
            drop(line);
        }
    }
    pub fn append(&self, item: &mut Item, path: &[u8]) {
        if self.lines.is_empty() {
            return;
        }
        let mut length =
            item.bytes.len().checked_add(10).expect("Log words").checked_add(path.len()).expect("log path");
        for line in &self.lines {
            length = length.checked_add(4).expect("indent").checked_add(line.len()).expect("tail line");
        }
        let mut writer = skein_lib::Writer::new(length);
        writer.put(&item.bytes).expect("failure line");
        for line in &self.lines {
            writer.put(b"    ").expect("indent");
            writer.put(line).expect("tail line");
        }
        writer.put(b"    Log: ").expect("Log words");
        writer.put(path).expect("log path");
        writer.put(b"\n").expect("log LF");
        item.bytes = writer.finish();
    }
    pub fn worst_case(lines: u32, name: usize) -> u64 {
        let bytes = u64::try_from(name)
            .expect("step name")
            .checked_add(u64::from(crate::host::LINE_BYTES))
            .expect("piece")
            .checked_add(3)
            .expect("prefix and LF");
        skein_lib::Queue::<Box<[u8]>>::worst_case(lines)
            .expect("tail table")
            .checked_add(bytes.checked_mul(u64::from(lines)).expect("tail pieces"))
            .expect("tail memory")
    }
}
/// Only failures of an actual step's exit or readiness carry a tail.
pub(crate) fn tail_step(fact: Fact) -> Option<usize> {
    let why = match fact {
        Fact::Restarting { why, .. } | Fact::GivingUp { why, .. } | Fact::GroupFailed { why, .. } => why,
        Fact::Starting { .. }
        | Fact::Started { .. }
        | Fact::GroupStarting { .. }
        | Fact::GroupStarted { .. }
        | Fact::GroupStopped { .. }
        | Fact::Stopping
        | Fact::Stopped => return None,
    };
    match why {
        Why::Exited { step, .. } | Why::NotReady { step } => Some(step),
        Why::NotStarted { .. } | Why::NotWritten | Why::Busy | Why::NotAsked | Why::Stopping => None,
    }
}
pub(crate) fn failure_bytes(steps: &[Placed], data: &[u8]) -> u32 {
    let mut name = 0_u32;
    for step in steps {
        name = name.max(u32::try_from(step.step.name().len()).expect("step name"));
    }
    let line = name.checked_add(crate::host::LINE_BYTES).expect("piece").checked_add(7).expect("indent, prefix, LF");
    max_bytes(steps)
        .checked_add(line.checked_mul(crate::limits::MAX_TAIL_LINES).expect("tail lines"))
        .expect("failure block")
        .checked_add(u32::try_from(data.len()).expect("data path"))
        .expect("log path")
        .checked_add(name)
        .expect("log filename")
        .checked_add(32)
        .expect("Log words and path suffix")
}
