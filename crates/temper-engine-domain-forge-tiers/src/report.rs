//! Report bytes to failing test ids (domain/after-landing.md, sections 2 and 4).
//!
//! This boundary reads the two report encodings; the connector top owns
//! artifact retrieval and run conclusions.
use crate::{Failures, Format, Limits, Outcome, TestId, Unreadable};
use alloc::boxed::Box;
use skein_lib::{List, Writer, bytes};

/// Reads a report, sorting and deduplicating its bounded failing ids.
pub fn report(format: Format, file: &[u8], limits: &Limits) -> Result<Failures, Unreadable> {
    match format {
        Format::Lines => Ok(lines(file, *limits)),
        Format::Junit => junit(file, *limits),
    }
}

/// A failed run without a readable, nonempty report is a runner fault.
#[must_use]
pub fn failed(report: Option<Result<Failures, Unreadable>>) -> Outcome {
    match report {
        None | Some(Err(Unreadable)) => Outcome::Fault,
        Some(Ok(Failures::Many)) => Outcome::Failed(Failures::Many),
        Some(Ok(Failures::Tests(tests))) => {
            if tests.is_empty() {
                Outcome::Fault
            } else {
                Outcome::Failed(Failures::Tests(tests))
            }
        }
    }
}

fn lines(file: &[u8], limits: Limits) -> Failures {
    let mut failures = Failures::Tests(Box::new([]));
    let mut from = 0_usize;
    for (at, byte) in file.iter().enumerate() {
        if *byte == b'\n' {
            line(&mut failures, file.get(from..at).expect("line lies within file"), limits);
            from = at.checked_add(1).expect("position within file");
        }
    }
    line(&mut failures, file.get(from..).expect("last line lies within file"), limits);
    failures
}

fn line(failures: &mut Failures, line: &[u8], limits: Limits) {
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    if !line.is_empty() {
        if line.len() > index(limits.test_id_bytes) {
            *failures = Failures::Many;
        } else {
            insert(failures, Box::from(line), limits);
        }
    }
}

fn index(value: u32) -> usize {
    usize::try_from(value).expect("u32 fits usize")
}

fn insert(failures: &mut Failures, test: TestId, limits: Limits) {
    let tests = match failures {
        Failures::Many => return,
        Failures::Tests(tests) => tests,
    };
    if tests.binary_search(&test).is_ok() {
        return;
    }
    if tests.len() >= index(limits.failing_tests) {
        *failures = Failures::Many;
        return;
    }
    let capacity = u32::try_from(tests.len()).expect("bounded ids").checked_add(1).expect("room for an id");
    let mut ordered = List::with_capacity(capacity);
    let mut new = Some(test);
    for old in tests.iter() {
        if let Some(test) = &new
            && test < old
        {
            let test = new.take().expect("new id is pending");
            ordered.push(test).expect("one new id fits");
        }
        ordered.push(old.clone()).expect("old ids fit");
    }
    if let Some(test) = new {
        ordered.push(test).expect("one new id fits");
    }
    *tests = ordered.into_boxed();
}

#[derive(Debug)]
struct Case {
    // None is an id past the byte bound, remembered only if the case fails.
    id: Option<TestId>,
    failed: bool,
}

fn junit(file: &[u8], limits: Limits) -> Result<Failures, Unreadable> {
    let mut failures = Failures::Tests(Box::new([]));
    let mut suite = false;
    let mut case: Option<Case> = None;
    let mut from = 0_usize;
    for (at, byte) in file.iter().enumerate() {
        if at < from || *byte != b'<' {
            continue;
        }
        let rest = file.get(at..).expect("tag starts within file");
        let special = if rest.starts_with(b"<!--") {
            Some(b"-->".as_slice())
        } else if rest.starts_with(b"<![CDATA[") {
            Some(b"]]>".as_slice())
        } else if rest.starts_with(b"<?") {
            Some(b"?>".as_slice())
        } else {
            None
        };
        if let Some(end) = special {
            let position = bytes::find(rest, end).ok_or(Unreadable)?;
            from = at.checked_add(position).ok_or(Unreadable)?.checked_add(end.len()).ok_or(Unreadable)?;
            continue;
        }
        let end = tag_end(rest).ok_or(Unreadable)?;
        from = at.checked_add(end).ok_or(Unreadable)?.checked_add(1).ok_or(Unreadable)?;
        let tag = rest.get(1..end).ok_or(Unreadable)?;
        let closing = tag.starts_with(b"/");
        let tag = tag.strip_prefix(b"/").unwrap_or(tag);
        let self_closing = tag.ends_with(b"/");
        let tag = tag.strip_suffix(b"/").unwrap_or(tag);
        let name_end = word_end(tag);
        let name = tag.get(..name_end).ok_or(Unreadable)?;
        let attributes = tag.get(name_end..).ok_or(Unreadable)?;
        if name.is_empty() {
            return Err(Unreadable);
        }
        if !closing && (name == b"testsuites" || name == b"testsuite") {
            suite = true;
        }
        if name == b"testcase" {
            if closing {
                let ended = case.take().ok_or(Unreadable)?;
                if ended.failed {
                    match ended.id {
                        Some(id) => insert(&mut failures, id, limits),
                        None => failures = Failures::Many,
                    }
                }
            } else {
                if case.is_some() {
                    return Err(Unreadable);
                }
                let name = attribute(attributes, Attribute::Name)?.ok_or(Unreadable)?;
                let class = attribute(attributes, Attribute::Class)?.unwrap_or(b"");
                let id = case_id(name, class, limits.test_id_bytes)?;
                if !self_closing {
                    case = Some(Case { id, failed: false });
                }
            }
        } else if !closing
            && (name == b"failure" || name == b"error")
            && let Some(case) = &mut case
        {
            case.failed = true;
        }
    }
    if !suite || case.is_some() {
        return Err(Unreadable);
    }
    Ok(failures)
}

// Quotes may contain '>'; '<' in a tag is never a terminator.
fn tag_end(tag: &[u8]) -> Option<usize> {
    let mut quote = None;
    for (at, byte) in tag.iter().enumerate().skip(1) {
        match quote {
            Some(open) => {
                if *byte == open {
                    quote = None;
                }
            }
            None => match byte {
                b'\'' | b'"' => quote = Some(*byte),
                b'>' => return Some(at),
                b'<' => return None,
                _ => {}
            },
        }
    }
    None
}

fn space(byte: u8) -> bool {
    byte == b' ' || byte == b'\t' || byte == b'\r' || byte == b'\n'
}

fn word_end(bytes: &[u8]) -> usize {
    for (at, byte) in bytes.iter().enumerate() {
        if space(*byte) || *byte == b'=' {
            return at;
        }
    }
    bytes.len()
}

fn trim(bytes: &[u8]) -> &[u8] {
    for (at, byte) in bytes.iter().enumerate() {
        if !space(*byte) {
            return bytes.get(at..).expect("within attributes");
        }
    }
    b""
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Attribute {
    Name,
    Class,
}

fn attribute(attributes: &[u8], wanted: Attribute) -> Result<Option<&[u8]>, Unreadable> {
    let wanted = match wanted {
        Attribute::Name => b"name".as_slice(),
        Attribute::Class => b"classname".as_slice(),
    };
    let mut rest = attributes;
    let mut found = None;
    // Each iteration consumes at least one byte; malformed attributes refuse.
    for _ in 0..attributes.len() {
        rest = trim(rest);
        if rest.is_empty() {
            break;
        }
        let end = word_end(rest);
        if end == 0 {
            return Err(Unreadable);
        }
        let key = rest.get(..end).ok_or(Unreadable)?;
        rest = trim(rest.get(end..).ok_or(Unreadable)?);
        rest = trim(rest.strip_prefix(b"=").ok_or(Unreadable)?);
        let (&quote, value) = rest.split_first().ok_or(Unreadable)?;
        if quote != b'\'' && quote != b'"' {
            return Err(Unreadable);
        }
        let end = bytes::find(value, &[quote]).ok_or(Unreadable)?;
        let attr = value.get(..end).ok_or(Unreadable)?;
        if key == wanted {
            found = Some(attr);
        }
        rest = value.get(end.checked_add(1).ok_or(Unreadable)?..).ok_or(Unreadable)?;
        if let Some(byte) = rest.first()
            && !space(*byte)
        {
            return Err(Unreadable);
        }
    }
    Ok(found)
}

fn case_id(name: &[u8], class: &[u8], limit: u32) -> Result<Option<TestId>, Unreadable> {
    let name = decode(name, limit)?;
    let class = decode(class, limit)?;
    let Some(name) = name else { return Ok(None) };
    let Some(class) = class else { return Ok(None) };
    if class.is_empty() {
        return Ok(Some(name));
    }
    let Some(length) = class.len().checked_add(1) else { return Ok(None) };
    let Some(length) = length.checked_add(name.len()) else { return Ok(None) };
    if length > index(limit) {
        return Ok(None);
    }
    let mut id = Writer::new(length);
    id.put(&class).expect("id's computed length");
    id.put(b".").expect("id's separator");
    id.put(&name).expect("id's computed length");
    Ok(Some(id.finish()))
}

fn decode(raw: &[u8], limit: u32) -> Result<Option<TestId>, Unreadable> {
    let capacity = raw.len().min(index(limit));
    let mut decoded = List::with_capacity(u32::try_from(capacity).expect("capacity bounded by u32"));
    let mut oversized = false;
    let mut from = 0_usize;
    for (at, byte) in raw.iter().enumerate() {
        if at < from {
            continue;
        }
        if *byte != b'&' {
            if decoded.push(*byte).is_err() {
                oversized = true;
            }
            continue;
        }
        let rest = raw.get(at.checked_add(1).ok_or(Unreadable)?..).ok_or(Unreadable)?;
        let end = bytes::find(rest, b";").ok_or(Unreadable)?;
        let entity = rest.get(..end).ok_or(Unreadable)?;
        let character = entity_char(entity).ok_or(Unreadable)?;
        let mut buffer = [0_u8; 4];
        let utf8 = character.encode_utf8(&mut buffer);
        for byte in utf8.as_bytes() {
            if decoded.push(*byte).is_err() {
                oversized = true;
            }
        }
        from = at.checked_add(end).ok_or(Unreadable)?.checked_add(2).ok_or(Unreadable)?;
    }
    if oversized { Ok(None) } else { Ok(Some(decoded.into_boxed())) }
}

fn entity_char(entity: &[u8]) -> Option<char> {
    match entity {
        b"amp" => Some('&'),
        b"lt" => Some('<'),
        b"gt" => Some('>'),
        b"quot" => Some('"'),
        b"apos" => Some('\''),
        _ => {
            let numeric = entity.strip_prefix(b"#")?;
            let (digits, base) = match numeric.strip_prefix(b"x") {
                Some(hex) => (hex, 16_u32),
                None => (numeric, 10_u32),
            };
            if digits.is_empty() {
                return None;
            }
            let mut value = 0_u32;
            for byte in digits {
                let digit = char::from(*byte).to_digit(base)?;
                value = value.checked_mul(base)?.checked_add(digit)?;
            }
            // XML 1.0 character references exclude NUL and surrogate codepoints.
            if value != 9
                && value != 10
                && value != 13
                && !(0x20..=0xD7FF).contains(&value)
                && !(0xE000..=0xFFFD).contains(&value)
                && !(0x0001_0000..=0x0010_FFFF).contains(&value)
            {
                return None;
            }
            char::from_u32(value)
        }
    }
}
