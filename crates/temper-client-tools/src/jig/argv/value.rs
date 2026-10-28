//! Typed values shared by the table parser and printer.
use super::{Unwritten, Usage};
use crate::jig::subject::{self, Condition};
use crate::jig::{
    Limits,
    call::{Project, Value},
    table::{Argument, Bound, Kind, Place, Tool, bound},
};
use alloc::boxed::Box;
use jig_client::contract::{InboxGroup, ProposalKey, Proposer, QueryOrder, TurnName};
use skein_lib::{Decimal, Duration, Overflow, Writer};
pub(super) fn valid(
    table: &[&Tool],
    spec: &Argument,
    value: &Value,
    argument: u8,
    limits: &Limits,
) -> Result<(), Usage> {
    if spec.place == Place::Switch {
        return if *value == Value::Switch { Ok(()) } else { Err(Usage::Kind { argument }) };
    }
    let correct = match spec.kind {
        Kind::Number | Kind::Count | Kind::NumberRange { .. } => return valid_number(spec, value, argument, limits),
        Kind::NamedNumber { words, prefixes, .. } => {
            let Value::Text(bytes) = value else {
                return Err(Usage::Kind { argument });
            };
            let Bound::Numbers { least, most } = bound(spec, limits) else {
                return Err(Usage::Kind { argument });
            };
            named_number(bytes, words, prefixes, least, most)
        }
        Kind::Word { set } => {
            let Value::Word(index) = value else {
                return Err(Usage::Kind { argument });
            };
            set.get(usize::from(*index)).is_some()
        }
        Kind::Literal if *value == Value::Stdin => false,
        Kind::Literal | Kind::Text => return valid_text(spec, value, argument, limits),
        Kind::File => return valid_file(spec, value, argument, limits),
        Kind::Project { empty } => return valid_project(value, argument, empty),
        Kind::Turn => {
            let Value::Turn(_) = value else {
                return Err(Usage::Kind { argument });
            };
            true
        }
        Kind::Group => {
            let Value::Group(_) = value else {
                return Err(Usage::Kind { argument });
            };
            true
        }
        Kind::Order => {
            let Value::Order(_) = value else {
                return Err(Usage::Kind { argument });
            };
            true
        }
        Kind::Cursor => {
            let Value::Cursor(bytes) = value else {
                return Err(Usage::Kind { argument });
            };
            !bytes.is_empty()
        }
        Kind::Time => {
            let Value::Text(bytes) = value else { return Err(Usage::Kind { argument }) };
            super::deadline(bytes).is_some()
        }
        Kind::Duration => return valid_duration(spec, value, argument, limits),
        Kind::Proposal => {
            let Value::Proposal(_) = value else {
                return Err(Usage::Kind { argument });
            };
            true
        }
        Kind::Digest => {
            let Value::Digest(_) = value else {
                return Err(Usage::Kind { argument });
            };
            true
        }
        Kind::Condition => {
            let Value::Condition(_) = value else {
                return Err(Usage::Kind { argument });
            };
            true
        }

        Kind::Tool => {
            let Value::Tool(id) = value else {
                return Err(Usage::Kind { argument });
            };
            table.get(usize::from(*id)).is_some()
        }
    };
    if correct { Ok(()) } else { Err(Usage::Kind { argument }) }
}
pub(crate) fn decode(table: &[&Tool], kind: Kind, bounds: Bound, word: &[u8]) -> Option<Value> {
    Some(match kind {
        Kind::Number | Kind::Count => Value::Number(number(word)?),
        Kind::NumberRange { .. } => {
            let Bound::Numbers { least, most } = bounds else { return None };
            Value::Number(bounded_number(word, least, most)?)
        }
        Kind::NamedNumber { words, prefixes, .. } => {
            let Bound::Numbers { least, most } = bounds else { return None };
            Value::Text(named_text(word, words, prefixes, least, most)?)
        }
        Kind::Word { set } => {
            let mut found = None;
            for (index, name) in set.iter().enumerate() {
                if *name == word {
                    found = Some(u8::try_from(index).ok()?);
                    break;
                }
            }
            Value::Word(found?)
        }
        Kind::Text | Kind::File if word == b"-" => Value::Stdin,
        Kind::Text | Kind::Literal => Value::Text(Box::from(word)),
        Kind::File => Value::File(Box::from(word)),
        Kind::Project { empty } => {
            if word.is_empty() {
                return if empty { Some(Value::Project(Project::Name(Box::default()))) } else { None };
            }
            let mut digits = true;
            for byte in word {
                digits &= byte.is_ascii_digit();
            }
            Value::Project(if digits {
                Project::Number(u32::try_from(number(word)?).ok()?)
            } else {
                Project::Name(Box::from(word))
            })
        }
        Kind::Turn => Value::Turn(turn(word)?),
        Kind::Cursor => Value::Cursor(hex(word)?),
        Kind::Group => Value::Group(match word {
            b"questions" => InboxGroup::Questions,
            b"proposals" => InboxGroup::Proposals,
            b"escalations" => InboxGroup::Escalations,
            b"person-tasks" => InboxGroup::PersonTasks,
            b"results" => InboxGroup::Results,
            b"replies" => InboxGroup::Replies,
            _ => return None,
        }),
        Kind::Order => Value::Order(match word {
            b"activity" => QueryOrder::Activity,
            b"priority" => QueryOrder::Priority,
            _ => return None,
        }),
        Kind::Time => {
            super::deadline(word)?;
            Value::Text(Box::from(word))
        }
        Kind::Duration => Value::Duration(duration(word)?),
        Kind::Proposal => {
            let (tag, rest) = word.split_first()?;
            let mut slash = None;
            for (index, byte) in rest.iter().enumerate() {
                if *byte == b'/' {
                    slash = Some(index);
                    break;
                }
            }
            let slash = slash?;
            let owner = number(rest.get(..slash)?)?;
            let proposer = match tag {
                b't' => Proposer::Task(owner),
                b'p' => Proposer::Party(owner),
                _ => return None,
            };
            Value::Proposal(ProposalKey { proposer, number: number(rest.get(slash.checked_add(1)?..)?)? })
        }
        Kind::Digest => {
            if word.len() != 16 {
                return None;
            }
            let bytes = hex(word)?;
            let mut digest = [0; 8];
            for (slot, byte) in digest.iter_mut().zip(bytes.iter()) {
                *slot = *byte;
            }
            Value::Digest(digest)
        }
        Kind::Condition => Value::Condition(subject::parse(word)?),
        Kind::Tool => Value::Tool(super::tool_index(table, word)?),
    })
}
pub(super) fn number(word: &[u8]) -> Option<u64> {
    if word.is_empty() {
        return None;
    }
    let mut value = 0_u64;
    for byte in word {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value.checked_mul(10)?.checked_add(u64::from(byte.checked_sub(b'0')?))?;
    }
    Some(value)
}
fn hex(word: &[u8]) -> Option<Box<[u8]>> {
    if word.is_empty() || word.len().checked_rem(2)? != 0 {
        return None;
    }
    let mut out = Writer::new(word.len().checked_div(2)?);
    for pair in word.chunks_exact(2) {
        let mut value = 0_u8;
        for byte in pair {
            let nibble = match byte {
                b'0'..=b'9' => byte.checked_sub(b'0')?,
                b'a'..=b'f' => byte.checked_sub(b'a')?.checked_add(10)?,
                _ => return None,
            };
            value = value.checked_mul(16)?.checked_add(nibble)?;
        }
        out.put(&[value]).ok()?;
    }
    Some(out.finish())
}
pub(crate) fn encode(table: &[&Tool], kind: Kind, value: &Value) -> Result<Box<[u8]>, Unwritten> {
    Ok(match value {
        Value::Number(n) => Box::from(Decimal::of(*n).as_bytes()),
        Value::Word(index) => {
            let Kind::Word { set } = kind else {
                return Err(Unwritten::NoForm);
            };
            Box::from(*set.get(usize::from(*index)).ok_or(Unwritten::NoForm)?)
        }
        Value::Text(text) | Value::File(text) => text.clone(),
        Value::Stdin => Box::from(b"-".as_slice()),
        Value::Project(Project::Number(n)) => Box::from(Decimal::of(u64::from(*n)).as_bytes()),
        Value::Project(Project::Name(name)) => name.clone(),
        Value::Turn(turn) => {
            let attempt = Decimal::of(turn.attempt);
            let number = Decimal::of(u64::from(turn.number));
            let prefix = attempt.as_bytes().len().checked_add(1).ok_or(Unwritten::Overflow(Overflow))?;
            let size = prefix.checked_add(number.as_bytes().len()).ok_or(Unwritten::Overflow(Overflow))?;
            let mut out = Writer::new(size);
            written(out.put(attempt.as_bytes()))?;
            written(out.put(b"."))?;
            written(out.put(number.as_bytes()))?;
            out.finish()
        }
        Value::Cursor(bytes) => hexadecimal(bytes)?,
        Value::Group(group) => Box::from(match group {
            InboxGroup::Questions => b"questions".as_slice(),
            InboxGroup::Proposals => b"proposals",
            InboxGroup::Escalations => b"escalations",
            InboxGroup::PersonTasks => b"person-tasks",
            InboxGroup::Results => b"results",
            InboxGroup::Replies => b"replies",
        }),
        Value::Order(order) => Box::from(match order {
            QueryOrder::Number | QueryOrder::Changed | QueryOrder::Ended => return Err(Unwritten::NoForm),
            QueryOrder::Activity => b"activity".as_slice(),
            QueryOrder::Priority => b"priority",
        }),
        Value::Duration(duration) => {
            if duration.as_nanos().checked_rem(1_000_000_000) != Some(0) {
                return Err(Unwritten::NoForm);
            }
            Box::from(Decimal::of(duration.as_nanos().checked_div(1_000_000_000).ok_or(Unwritten::NoForm)?).as_bytes())
        }
        Value::Proposal(proposal) => {
            let (tag, owner) = match proposal.proposer {
                Proposer::Task(n) => (b"t", n),
                Proposer::Party(n) => (b"p", n),
            };
            let owner = Decimal::of(owner);
            let number = Decimal::of(proposal.number);
            let size =
                owner.as_bytes().len().checked_add(number.as_bytes().len()).ok_or(Unwritten::Overflow(Overflow))?;
            let size = size.checked_add(2).ok_or(Unwritten::Overflow(Overflow))?;
            let mut out = Writer::new(size);
            written(out.put(tag))?;
            written(out.put(owner.as_bytes()))?;
            written(out.put(b"/"))?;
            written(out.put(number.as_bytes()))?;
            out.finish()
        }
        Value::Digest(digest) => hexadecimal(digest)?,
        Value::Condition(condition) => {
            let bytes: &[u8] = match condition {
                Condition::Changed => b"changed",
                Condition::Waiting => b"waiting",
                Condition::Usable => b"usable",
                Condition::Settled => b"settled",
                Condition::Updates => b"updates",
                Condition::Ended => b"ended",
                Condition::WaitingOnMe => b"waiting-on-me",
                Condition::Held => b"held",
                Condition::Replied => b"replied",
                Condition::Phase(phase) => {
                    let word = subject::phase_word(*phase);
                    let mut out = Writer::new(word.len().checked_add(6).ok_or(Unwritten::Overflow(Overflow))?);
                    written(out.put(b"phase:"))?;
                    written(out.put(word))?;
                    return Ok(out.finish());
                }
            };
            Box::from(bytes)
        }
        Value::Switch => return Err(Unwritten::NoForm),
        Value::Tool(id) => Box::from(table.get(usize::from(*id)).ok_or(Unwritten::NoForm)?.name),
    })
}

fn written(result: Result<(), Overflow>) -> Result<(), Unwritten> {
    match result {
        Ok(()) => Ok(()),
        Err(error) => Err(Unwritten::Overflow(error)),
    }
}

pub(crate) fn hexadecimal(bytes: &[u8]) -> Result<Box<[u8]>, Unwritten> {
    let size = bytes.len().checked_mul(2).ok_or(Unwritten::Overflow(Overflow))?;
    let mut hex = Writer::new(size);
    for byte in bytes {
        for nibble in [byte.checked_div(16).unwrap_or(0), byte.checked_rem(16).unwrap_or(0)] {
            let digit = b"0123456789abcdef".get(usize::from(nibble)).ok_or(Unwritten::NoForm)?;
            written(hex.put(&[*digit]))?;
        }
    }
    Ok(hex.finish())
}

fn valid_text(spec: &Argument, value: &Value, argument: u8, limits: &Limits) -> Result<(), Usage> {
    if *value == Value::Stdin {
        return Ok(());
    }
    let Value::Text(text) = value else {
        return Err(Usage::Kind { argument });
    };
    if let Bound::Bytes { most } = bound(spec, limits)
        && u64::try_from(text.len()).unwrap_or(u64::MAX) > most
    {
        return Err(Usage::Long { argument });
    }
    Ok(())
}

fn duration(word: &[u8]) -> Option<Duration> {
    let (digits, unit) = match word.last()? {
        b's' => (word.get(..word.len().checked_sub(1)?)?, 1_u64),
        b'm' => (word.get(..word.len().checked_sub(1)?)?, 60),
        b'h' => (word.get(..word.len().checked_sub(1)?)?, 3600),
        _ => (word, 1),
    };
    Some(Duration::from_nanos(number(digits)?.checked_mul(unit)?.checked_mul(1_000_000_000)?))
}

fn valid_project(value: &Value, argument: u8, empty: bool) -> Result<(), Usage> {
    let Value::Project(project) = value else {
        return Err(Usage::Kind { argument });
    };
    match project {
        Project::Number(_) => Ok(()),
        Project::Name(name) => {
            if name.is_empty() && empty {
                return Ok(());
            }
            let mut digits = true;
            for byte in name {
                digits &= byte.is_ascii_digit();
            }
            if name.is_empty() || digits { Err(Usage::Kind { argument }) } else { Ok(()) }
        }
    }
}

fn valid_number(spec: &Argument, value: &Value, argument: u8, limits: &Limits) -> Result<(), Usage> {
    let Value::Number(number) = value else {
        return Err(Usage::Kind { argument });
    };
    if let Bound::Numbers { least, most } = bound(spec, limits)
        && (*number < least || *number > most)
    {
        return Err(if spec.kind == Kind::Count { Usage::Rows { argument } } else { Usage::Kind { argument } });
    }
    Ok(())
}

fn named_number(word: &[u8], words: &[&[u8]], prefixes: &[&[u8]], min: u64, max: u64) -> bool {
    if words.contains(&word) {
        return true;
    }
    for prefix in prefixes {
        if let Some(digits) = word.strip_prefix(*prefix)
            && bounded_number(digits, min, max).is_some()
        {
            return true;
        }
    }
    false
}

fn valid_duration(spec: &Argument, value: &Value, argument: u8, limits: &Limits) -> Result<(), Usage> {
    let Value::Duration(duration) = value else {
        return Err(Usage::Kind { argument });
    };
    if let Bound::Seconds { most } = bound(spec, limits)
        && *duration > Duration::from_secs(most)
    {
        return Err(Usage::Rows { argument });
    }
    if duration.as_nanos().checked_rem(1_000_000_000) == Some(0) { Ok(()) } else { Err(Usage::Kind { argument }) }
}
fn bounded_number(word: &[u8], min: u64, max: u64) -> Option<u64> {
    let n = number(word)?;
    if n < min || n > max { None } else { Some(n) }
}
fn named_text(word: &[u8], words: &[&[u8]], prefixes: &[&[u8]], min: u64, max: u64) -> Option<Box<[u8]>> {
    if named_number(word, words, prefixes, min, max) { Some(Box::from(word)) } else { None }
}

fn valid_file(spec: &Argument, value: &Value, argument: u8, limits: &Limits) -> Result<(), Usage> {
    if let Value::Text(_) = value
        && let Bound::Bytes { .. } = bound(spec, limits)
    {
        return valid_text(spec, value, argument, limits);
    }
    if *value == Value::Stdin {
        return Ok(());
    }
    let Value::File(_) = value else {
        return Err(Usage::Kind { argument });
    };
    Ok(())
}

fn turn(word: &[u8]) -> Option<TurnName> {
    let mut dot = None;
    for (index, byte) in word.iter().enumerate() {
        if *byte == b'.' {
            dot = Some(index);
            break;
        }
    }
    let split = dot?;
    let end = split.checked_add(1)?;
    Some(TurnName { attempt: number(word.get(..split)?)?, number: u32::try_from(number(word.get(end..)?)?).ok()? })
}
