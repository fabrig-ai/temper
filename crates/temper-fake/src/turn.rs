//! The two turn spellings, shared with the box's asked scripts (protocol/setup.md, 8.4).
mod failure;
use crate::{Refusal, read};
use alloc::boxed::Box;
use core::mem::size_of;
use jig_protocol_config::{Path, read as config};
use skein_fake_llm_domain::api::{self, Finish, Line, Piece};
use skein_json::{
    Document,
    document::Kind,
    writer::{Encoder, Limits},
};
use skein_lib::List;

/// A checked turn: text, or explicit lines, finish and token count.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Turn {
    value: api::Turn,
}
impl Turn {
    /// Reads and checks either spelling at its document path.
    pub fn read(d: &Document, at: u32, p: &Path) -> Result<Self, Refusal> {
        Self::read_standalone(d, at, p, None)
    }
    /// The standalone document permits pieces and failures for its accepted credentials.
    pub(crate) fn read_standalone(d: &Document, at: u32, p: &Path, credentials: Option<u32>) -> Result<Self, Refusal> {
        let token = d.token(at).ok_or(read::why(p, b"missing"))?;
        if token.kind == Kind::String {
            let text = read::text(d, at, crate::ANSWER_BYTES, p)?;
            let tokens = u64::try_from(text.len()).expect("bounded text").div_ceil(4);
            let value = api::Turn { lines: Box::new([Line::Text { text }]), finish: Finish::Stop, tokens };
            check_answer(&value, p)?;
            return Ok(Self { value });
        }
        let mut o = read::object(d, at, p)?;
        let lines_at = read::required(&mut o, d, b"lines", p)?;
        let finish = read::field(&mut o, d, b"finish", 32, p)?;
        let tokens_at = read::required(&mut o, d, b"tokens", p)?;
        read::finish(o, d, p)?;
        let finish = match finish.as_ref() {
            b"stop" => Finish::Stop,
            b"tool_calls" => Finish::ToolCalls,
            b"length" => Finish::Length,
            b"content_filter" => Finish::ContentFilter,
            _ => return Err(read::why(&p.member(b"finish"), b"expected stop, tool_calls, length or content_filter")),
        };
        let tokens = read::integer(d, tokens_at, u64::MAX, &p.member(b"tokens"))?;
        if tokens == 0 {
            return Err(read::why(&p.member(b"tokens"), b"must be at least 1"));
        }
        let items = read::list(d, lines_at, 1, &p.member(b"lines"))?;
        let mut lines = List::with_capacity(items.len());
        for (i, at) in items.iter().enumerate() {
            let line = line(d, *at, &read::index(&p.member(b"lines"), i), credentials)?;
            lines.push(line).expect("bounded lines");
        }
        let value = api::Turn { lines: lines.into_boxed(), finish, tokens };
        check_answer(&value, p)?;
        // Let skein own line-position rules; diagnose the individual turn before
        // the document's joint script check loses its path.
        if credentials.is_some() {
            let scripts =
                api::Scripts { cued: Box::new([]), asked: Box::new([]), otherwise: Box::new([value.clone()]) };
            let menu = api::Menu { arguments: Box::new([]), invalid: Box::new([]) };
            if skein_fake_llm_domain::Domain::played(&crate::llm::LIMITS, 0, scripts, menu).is_err() {
                return Err(read::why(p, b"invalid turn"));
            }
        }
        Ok(Self { value })
    }
}
/// The single conversion from a checked spelling to skein's turn.
#[must_use]
pub fn into_skein(turn: Turn) -> api::Turn {
    turn.value
}

fn line(d: &Document, at: u32, p: &Path, credentials: Option<u32>) -> Result<Line, Refusal> {
    let mut o = read::object(d, at, p)?;
    let text_at = o.member(d, b"text");
    let call_at = o.member(d, b"call");
    let args_at = o.member(d, b"arguments");
    let pieces_at = if credentials.is_some() { o.member(d, b"pieces") } else { None };
    let fail_at = if credentials.is_some() { o.member(d, b"fail") } else { None };
    read::finish(o, d, p)?;
    if let Some(at) = fail_at {
        if text_at.is_some() || call_at.is_some() || args_at.is_some() || pieces_at.is_some() {
            return Err(read::why(p, b"expected fail alone"));
        }
        return failure::read(d, at, credentials.expect("standalone failure"), &p.member(b"fail"));
    }
    if let Some(at) = pieces_at {
        if text_at.is_some() || call_at.is_some() || args_at.is_some() {
            return Err(read::why(p, b"expected pieces alone"));
        }
        return pieces(d, at, &p.member(b"pieces"));
    }
    if let Some(at) = text_at {
        if call_at.is_some() || args_at.is_some() {
            return Err(read::why(p, b"expected text alone, or call and arguments"));
        }
        let text = read::text(d, at, crate::ANSWER_BYTES, &p.member(b"text"))?;
        read::nonempty(&text, &p.member(b"text"))?;
        return Ok(Line::Text { text });
    }
    let at = call_at.ok_or(read::why(&p.member(b"call"), b"missing"))?;
    let name = read::text(d, at, crate::limits::CALL_NAME_BYTES, &p.member(b"call"))?;
    read::nonempty(&name, &p.member(b"call"))?;
    for byte in &name {
        if !byte.is_ascii_alphanumeric() && *byte != b'_' && *byte != b'-' {
            return Err(read::why(&p.member(b"call"), b"expected ASCII letters, digits, _ or -"));
        }
    }
    let at = args_at.ok_or(read::why(&p.member(b"arguments"), b"missing"))?;
    if d.token(at).ok_or(read::why(p, b"missing"))?.kind != Kind::ObjectStart {
        return Err(read::why(&p.member(b"arguments"), b"expected a JSON object"));
    }
    let arguments = compact(d, at, &p.member(b"arguments"))?;
    Ok(Line::Call { name, arguments })
}
fn pieces(d: &Document, at: u32, p: &Path) -> Result<Line, Refusal> {
    let items = read::list(d, at, 1, p)?;
    let mut pieces = List::with_capacity(items.len());
    for (i, at) in items.iter().enumerate() {
        let p = read::index(p, i);
        let mut o = read::object(d, *at, &p)?;
        let text = read::field(&mut o, d, b"text", crate::ANSWER_BYTES, &p)?;
        let hold_at = read::required(&mut o, d, b"hold", &p)?;
        read::finish(o, d, &p)?;
        read::nonempty(&text, &p.member(b"text"))?;
        let hold = if read::is_null(d, hold_at) {
            None
        } else {
            let hold = read::text(d, hold_at, 64, &p.member(b"hold"))?;
            read::nonempty(&hold, &p.member(b"hold"))?;
            Some(hold)
        };
        pieces.push(Piece { text, hold }).expect("sixteen pieces");
    }
    Ok(Line::Pieces { pieces: pieces.into_boxed() })
}
fn compact(d: &Document, at: u32, p: &Path) -> Result<Box<[u8]>, Refusal> {
    let Ok(end) = config::after(d, at) else {
        return Err(read::why(p, b"invalid JSON"));
    };
    let limits = Limits { depth: 32, length: crate::ANSWER_BYTES };
    let mut measure = Encoder::measure(&limits);
    render(d, at, end, &mut measure);
    let Ok(len) = measure.measured() else {
        return Err(read::why(p, b"past its bound"));
    };
    let mut out = Encoder::write(len, &limits);
    render(d, at, end, &mut out);
    Ok(out.finish())
}
fn render(d: &Document, at: u32, end: u32, out: &mut Encoder) {
    for i in at..end {
        let token = d.token(i).expect("checked subtree");
        let text = d.text(token).expect("checked text");
        match token.kind {
            Kind::ObjectStart => out.object_start(),
            Kind::ObjectEnd => out.object_end(),
            Kind::ArrayStart => out.array_start(),
            Kind::ArrayEnd => out.array_end(),
            Kind::Key => out.key(text),
            Kind::String => out.string(text),
            Kind::Number => out.number(text),
            Kind::True => out.boolean(true),
            Kind::False => out.boolean(false),
            Kind::Null => out.null(),
            Kind::Long => unreachable!("whole document keeps text"),
        }
    }
}
fn check_answer(turn: &api::Turn, p: &Path) -> Result<(), Refusal> {
    let wrappers = turn.lines.len().checked_mul(size_of::<api::Part>()).expect("sixteen lines");
    let mut size = u64::try_from(wrappers).expect("bounded wrappers");
    for line in &turn.lines {
        let bytes = match line {
            Line::Text { text } => text.len(),
            Line::Call { name, arguments } => {
                name.len().checked_add(arguments.len()).expect("bounded call").checked_add(21).expect("call id")
            }
            Line::Pieces { pieces } => {
                let mut bytes = 0_usize;
                for piece in pieces {
                    bytes = bytes.checked_add(piece.text.len()).expect("sixteen bounded pieces");
                }
                bytes
            }
            Line::Fail { credentials, .. } => credentials.len().checked_mul(size_of::<u32>()).expect("sixteen indices"),
            Line::Opaque { .. } => unreachable!("no opaque document lines"),
        };
        size = size.checked_add(u64::try_from(bytes).expect("bounded line")).expect("sixteen bounded lines");
    }
    if size > u64::from(crate::ANSWER_BYTES) {
        return Err(read::why(p, b"answer past its byte bound"));
    }
    Ok(())
}
