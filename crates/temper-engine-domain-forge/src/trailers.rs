//! Terminal commit trailers and the forge's marker codes (domain/after-landing.md, 8).
use crate::Limits;
use alloc::boxed::Box;
use skein_lib::List;

pub(crate) fn marker(line: &[u8]) -> u32 {
    for (key, mask) in
        [(b"Taught:".as_slice(), 1_u32), (b"QA:".as_slice(), 2), (b"Fixes:".as_slice(), 4), (b"Repairs:".as_slice(), 8)]
    {
        if line.starts_with(key) {
            return mask;
        }
    }
    0
}

pub(crate) fn collect(kept: &[Box<[u8]>], message: &[u8], limits: &Limits, cut: &mut bool) -> Box<[Box<[u8]>]> {
    let mut lines = List::with_capacity(limits.trailers);
    for line in kept {
        lines.push(line.clone()).expect("admitted trailers");
    }
    let mut end = message.len();
    for byte in message.iter().rev() {
        if !byte.is_ascii_whitespace() {
            break;
        }
        end = end.checked_sub(1).expect("trailing byte");
    }
    let message = message.get(..end).expect("trimmed message");
    let mut start = 0_usize;
    let mut line_start = 0_usize;
    for end in 0..=message.len() {
        if end != message.len() && message.get(end) != Some(&b'\n') {
            continue;
        }
        let line = message.get(line_start..end).expect("line bounds");
        if line.is_empty() || line == b"\r" {
            start = end.checked_add(1).expect("newline");
        }
        line_start = end.checked_add(1).expect("newline");
    }
    let block = message.get(start..).unwrap_or(&[]);
    line_start = 0;
    for end in 0..=block.len() {
        if end != block.len() && block.get(end) != Some(&b'\n') {
            continue;
        }
        let line = block.get(line_start..end).expect("line bounds");
        if !valid_line(line) {
            return lines.into_boxed();
        }
        line_start = end.checked_add(1).expect("newline");
    }
    line_start = 0;
    for end in 0..=block.len() {
        if end != block.len() && block.get(end) != Some(&b'\n') {
            continue;
        }
        let line = block.get(line_start..end).expect("line bounds");
        line_start = end.checked_add(1).expect("newline");
        if marker(line) == 0 {
            continue;
        }
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let length = line.len().min(usize::try_from(limits.trailer_bytes).expect("u32 fits"));
        if lines.len() == limits.trailers {
            *cut = true;
            continue;
        }
        if line.len() > length {
            *cut = true;
        }
        lines.push(Box::from(line.get(..length).expect("bounded prefix"))).expect("trailer room");
    }
    lines.into_boxed()
}
fn valid_line(line: &[u8]) -> bool {
    let Some(colon) = skein_lib::bytes::find(line, b":") else { return false };
    if colon == 0 {
        return false;
    }
    for byte in line.get(..colon).expect("key prefix") {
        if !byte.is_ascii_alphanumeric() && *byte != b'-' {
            return false;
        }
    }
    line.get(colon.checked_add(1).expect("colon byte")) == Some(&b' ')
}
