//! Host byte pagination follows `read_docs`.
use alloc::boxed::Box;
use skein_lib::{Decimal, Writer};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Offset,
    TooLarge,
}
pub fn page(text: &[u8], from: u32) -> Result<Box<[u8]>, Error> {
    let start = usize::try_from(from).expect("u32 fits").min(text.len());
    let rest = text.get(start..).expect("text suffix");
    // The shared renderer produces validated UTF-8; an offset may not split it.
    if !boundary(text, start) {
        return Err(Error::Offset);
    }
    let mut end = start;
    let mut line = start;
    let mut size = 0_u32;
    for byte in rest {
        let cost = if [b'"', b'\\', b'\n'].contains(byte) { 2 } else { 1 };
        if size.checked_add(cost).ok_or(Error::TooLarge)? > 1536 {
            break;
        }
        size = size.checked_add(cost).expect("page bound");
        end = end.checked_add(1).expect("bounded text");
        if *byte == b'\n' {
            line = end;
        }
    }
    if end < text.len() && line > start {
        end = line;
    }
    while !boundary(text, end) {
        end = end.checked_sub(1).ok_or(Error::TooLarge)?;
    }
    let body = text.get(start..end).expect("page range");
    let next = Decimal::of(u64::try_from(end).expect("text offset"));
    let marker = if end < text.len() { next.as_bytes().len().checked_add(7).ok_or(Error::TooLarge)? } else { 4 };
    let separator = usize::from(!body.is_empty() && !body.ends_with(b"\n"));
    let size = body.len().checked_add(separator).ok_or(Error::TooLarge)?;
    let mut out = Writer::new(size.checked_add(marker).ok_or(Error::TooLarge)?);
    out.put(body).expect("page room");
    if !body.is_empty() && !body.ends_with(b"\n") {
        out.put(b"\n").expect("separator room");
    }
    if end < text.len() {
        out.put(b"next: ").expect("marker room");
        out.put(next.as_bytes()).expect("offset room");
        out.put(b"\n").expect("marker room");
    } else {
        out.put(b"end\n").expect("marker room");
    }
    Ok(out.finish())
}

fn boundary(text: &[u8], at: usize) -> bool {
    match text.get(at) {
        Some(byte) => byte & 0xc0 != 0x80,
        None => true,
    }
}
