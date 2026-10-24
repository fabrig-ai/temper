//! Bounded configuration problems and run refusals; no credentials enter diagnostic text.
use alloc::boxed::Box;
use jig_protocol_config::{self as config, Path};
use skein_lib::{Decimal, List, Writer};
/// A refusal's words, or another limit with which it must agree.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum What {
    Reason(&'static [u8]),
    Own(&'static [u8]),
    Against(&'static [u8]),
}
/// The opaque, bounded collection returned by the checks.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Problems {
    entries: List<(Path, What)>,
    past: u32,
}
impl Problems {
    #[must_use]
    pub fn empty() -> Self {
        Self { entries: List::with_capacity(config::PROBLEMS), past: 0 }
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.past == 0
    }
    pub(crate) fn push(&mut self, path: Path, what: What) {
        if self.entries.push((path, what)).is_err() {
            self.past = self.past.saturating_add(1);
        }
    }
    pub(crate) fn extend(&mut self, found: Problems) {
        for (path, what) in &found.entries {
            self.push(path.clone(), *what);
        }
        self.past = self.past.saturating_add(found.past);
    }
    pub(crate) fn append(&mut self, found: config::Problems) {
        for p in &found.problems {
            let words = match p.what {
                config::What::PolicyEnvelope { .. } => b"policy bytes exceed the people-v13 codec envelope".as_slice(),
                config::What::AmendmentEnvelope { .. } => {
                    b"amendment bytes exceed the people-v13 codec envelope".as_slice()
                }
                config::What::CodecCeiling { .. } => b"required bound exceeds the people-v13 codec ceiling".as_slice(),
                config::What::CodecArithmetic => b"people-v13 codec bound arithmetic overflows".as_slice(),
                config::What::StoreCodec => b"effective limits exceed the combined store codec envelope".as_slice(),
                config::What::Unknown => b"unknown member".as_slice(),
                config::What::Missing => b"missing",
                config::What::Duplicate => b"duplicate",
                config::What::Kind => b"wrong value kind",
                config::What::Bound => b"past its bound",
                config::What::Unique => b"must be unique",
                config::What::Refers => b"unknown reference",
                config::What::ProviderKey { .. } => b"invalid provider key",
                config::What::Relative => b"relative path must be non-empty with no .. component",
                config::What::Json
                | config::What::Form
                | config::What::Plaintext
                | config::What::Digest
                | config::What::Reserve { .. }
                | config::What::Budget { .. }
                | config::What::Grace { .. }
                | config::What::Operations { .. }
                | config::What::Message
                | config::What::Silence
                | config::What::StopBound
                | config::What::Memory { .. }
                | config::What::Override
                | config::What::Confidential
                | config::What::NoProvider
                | config::What::Unreadable => b"invalid form",
            };
            self.push(p.path.clone(), What::Reason(words));
        }
        self.past = self.past.saturating_add(found.past);
    }
}
fn put(w: &mut Writer, n: &mut usize, measure: bool, b: &[u8]) {
    *n = n.checked_add(b.len()).expect("bounded diagnostics");
    if !measure {
        w.put(b).expect("measured text");
    }
}
fn render(ps: &Problems, w: &mut Writer, measure: bool) -> usize {
    let mut n = 0;
    for (path, what) in &ps.entries {
        let mut member = false;
        for segment in &path.segments {
            match segment {
                config::Segment::Member(name) => {
                    if member {
                        put(w, &mut n, measure, b".");
                    }
                    put(w, &mut n, measure, name);
                    member = true;
                }
                config::Segment::Index(i) => {
                    put(w, &mut n, measure, b"[");
                    put(w, &mut n, measure, Decimal::of(u64::from(*i)).as_bytes());
                    put(w, &mut n, measure, b"]");
                }
            }
        }
        match what {
            What::Own(label) => put(w, &mut n, measure, label),
            What::Reason(_) | What::Against(_) => {}
        }
        put(w, &mut n, measure, b": ");
        match what {
            What::Own(_) => put(w, &mut n, measure, b"invalid form"),
            What::Reason(words) => put(w, &mut n, measure, words),
            What::Against(other) => {
                put(w, &mut n, measure, b"must agree with application.limits.overrides.");
                put(w, &mut n, measure, other);
            }
        }
        put(w, &mut n, measure, b"\n");
    }
    if ps.past > 0 {
        put(w, &mut n, measure, b"and ");
        put(w, &mut n, measure, Decimal::of(u64::from(ps.past)).as_bytes());
        put(w, &mut n, measure, b" more\n");
    }
    n
}
/// A line per problem, each beginning with its document path.
#[must_use]
pub fn lines(problems: &Problems) -> Box<[u8]> {
    let n = render(problems, &mut Writer::new(0), true);
    let mut w = Writer::new(n);
    assert_eq!(render(problems, &mut w, false), n, "same measure");
    w.finish()
}

/// A run's inherited listener differs from the owned setup (protocol/setup.md, 6.4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Problem {
    Listener { found: core::net::SocketAddr, expected: core::net::SocketAddr },
}
impl Problem {
    /// Diagnostic words after the forge command's prefix.
    #[must_use]
    pub fn words(&self) -> Box<[u8]> {
        match self {
            Self::Listener { found, expected } => crate::check::joined(&[
                b"fd 3 listens at ",
                &crate::ini::socket(*found),
                b", expected owned.listen ",
                &crate::ini::socket(*expected),
            ]),
        }
    }
}
