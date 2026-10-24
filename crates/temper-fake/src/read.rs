//! Strict members and refusal paths shared by the two documents (protocol/setup.md, 8.4).
use alloc::boxed::Box;
use core::net::SocketAddr;
use jig_protocol_config::{self as config, Path, Segment, What};
use skein_json::{Document, document::Kind};
use skein_lib::{Decimal, List, Writer};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Refusal {
    pub path: Path,
    pub words: &'static [u8],
}
impl Refusal {
    #[must_use]
    pub fn line(&self) -> Box<[u8]> {
        let mut size = self.words.len().checked_add(3).expect("diagnostic suffix");
        let mut member = false;
        for segment in &self.path.segments {
            let extra = match segment {
                Segment::Member(name) => {
                    let extra = name.len().checked_add(usize::from(member)).expect("document member");
                    member = true;
                    extra
                }
                Segment::Index(n) => Decimal::of(u64::from(*n)).as_bytes().len().checked_add(2).expect("index text"),
            };
            size = size.checked_add(extra).expect("document path fits memory");
        }
        if self.path.segments.is_empty() {
            size = size.checked_add(8).expect("root label");
        }
        let mut out = Writer::new(size);
        if self.path.segments.is_empty() {
            out.put(b"document").expect("root label measured");
        }
        let mut member = false;
        for segment in &self.path.segments {
            match segment {
                Segment::Member(name) => {
                    if member {
                        out.put(b".").expect("measured dot");
                    }
                    out.put(name).expect("measured member");
                    member = true;
                }
                Segment::Index(n) => {
                    out.put(b"[").expect("measured bracket");
                    out.put(Decimal::of(u64::from(*n)).as_bytes()).expect("measured index");
                    out.put(b"]").expect("measured bracket");
                }
            }
        }
        out.put(b": ").expect("measured separator");
        out.put(self.words).expect("measured words");
        out.put(b"\n").expect("measured newline");
        out.finish()
    }
}
pub fn why(p: &Path, words: &'static [u8]) -> Refusal {
    Refusal { path: p.clone(), words }
}
pub(crate) fn kind(p: &Path, what: What) -> Refusal {
    let words: &[u8] = match what {
        What::Json => b"invalid JSON",
        What::Unknown => b"unknown member",
        What::Missing => b"missing",
        What::Duplicate => b"duplicate",
        What::Kind => b"wrong value kind",
        What::Bound => b"past its bound",
        What::Unique => b"must be unique",
        What::Relative => b"relative path must be non-empty with no .. component",
        What::Form
        | What::Refers
        | What::Plaintext
        | What::Confidential
        | What::NoProvider
        | What::Digest
        | What::Reserve { .. }
        | What::Budget { .. }
        | What::Grace { .. }
        | What::Operations { .. }
        | What::Message
        | What::Silence
        | What::StopBound
        | What::Memory { .. }
        | What::Override
        | What::ProviderKey { .. }
        | What::PolicyEnvelope { .. }
        | What::AmendmentEnvelope { .. }
        | What::CodecCeiling { .. }
        | What::CodecArithmetic
        | What::StoreCodec
        | What::Unreadable => b"invalid form",
    };
    why(p, words)
}
pub fn document(bytes: &[u8]) -> Result<Document, Refusal> {
    match config::document(bytes, &config::DocumentLimits { bytes: crate::DOCUMENT_BYTES, tokens: 65_536, depth: 32 }) {
        Ok(d) => Ok(d),
        Err(p) => Err(kind(&p.path, p.what)),
    }
}
pub fn object(d: &Document, at: u32, p: &Path) -> Result<config::read::Object, Refusal> {
    match config::read::Object::new(d, at) {
        Ok(o) => Ok(o),
        Err(w) => Err(kind(p, w)),
    }
}
pub fn required(o: &mut config::read::Object, d: &Document, name: &[u8], p: &Path) -> Result<u32, Refusal> {
    o.member(d, name).ok_or(why(&p.member(name), b"missing"))
}
pub fn finish(o: config::read::Object, d: &Document, p: &Path) -> Result<(), Refusal> {
    let mut ps = config::Problems::empty();
    o.finish(d, p, &mut ps);
    match ps.problems.get(0) {
        Some(p) => Err(kind(&p.path, p.what)),
        None => Ok(()),
    }
}
pub fn text(d: &Document, at: u32, max: u32, p: &Path) -> Result<Box<[u8]>, Refusal> {
    match config::read::text(d, at, max) {
        Ok(v) => Ok(v),
        Err(w) => Err(kind(p, w)),
    }
}
pub fn field(
    o: &mut config::read::Object,
    d: &Document,
    name: &[u8],
    max: u32,
    p: &Path,
) -> Result<Box<[u8]>, Refusal> {
    let at = required(o, d, name, p)?;
    text(d, at, max, &p.member(name))
}
pub fn integer(d: &Document, at: u32, max: u64, p: &Path) -> Result<u64, Refusal> {
    match config::read::integer(d, at, max) {
        Ok(v) => Ok(v),
        Err(w) => Err(kind(p, w)),
    }
}
pub fn list(d: &Document, at: u32, min: u32, p: &Path) -> Result<List<u32>, Refusal> {
    match config::read::list(d, at, crate::ENTRIES) {
        Ok(v) if v.len() >= min => Ok(v),
        Ok(_) => Err(why(p, b"must be non-empty")),
        Err(w) => Err(kind(p, w)),
    }
}
pub fn index(p: &Path, i: usize) -> Path {
    p.child(Segment::Index(u32::try_from(i).expect("bounded list")))
}
pub fn nonempty(v: &[u8], p: &Path) -> Result<(), Refusal> {
    if v.is_empty() { Err(why(p, b"must be non-empty")) } else { Ok(()) }
}
pub fn absolute(v: &[u8], p: &Path) -> Result<(), Refusal> {
    if !v.starts_with(b"/") || v.contains(&0) { Err(why(p, b"must be an absolute path without NUL")) } else { Ok(()) }
}
pub fn listen(d: &Document, at: u32, p: &Path) -> Result<SocketAddr, Refusal> {
    let mut ps = config::Problems::empty();
    let addr = config::address::read(d, at, p, &mut ps);
    if let Some(p) = ps.problems.get(0) {
        return Err(kind(&p.path, p.what));
    }
    let socket = match addr {
        Some(config::Address::Addr(a)) => a.socket(),
        Some(config::Address::Host { .. }) | None => None,
    };
    match socket {
        Some(a) if a.ip().is_loopback() => Ok(a),
        Some(_) | None => Err(why(p, b"must be a loopback socket address")),
    }
}
pub fn is_null(d: &Document, at: u32) -> bool {
    match d.token(at) {
        Some(t) => t.kind == Kind::Null,
        None => false,
    }
}
pub fn joined(parts: &[&[u8]]) -> Box<[u8]> {
    let mut len = 0_usize;
    for part in parts {
        len = len.checked_add(part.len()).expect("bounded names");
    }
    let mut out = Writer::new(len);
    for part in parts {
        out.put(part).expect("measured name");
    }
    out.finish()
}
