//! Bounds shared by parsing, running and rendering.
use skein_lib::Duration;
/// The generic tools' bounds, from the host's preset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub rows: u32,
    pub settings: jig_client_protocol_wire::Limits,
    pub program_login: &'static [u8],
    pub application: &'static [u8],
    pub ask_bytes: u64,
    pub result_bytes: u64,
    pub text: u32,
    pub part: u32,
    pub wait: Duration,
    pub wait_most: Duration,
}
/// A tool and each of its argument forms, tried in order.
#[derive(Debug)]
pub struct Tool {
    pub name: &'static [u8],
    pub line: &'static [u8],
    pub result: &'static [u8],
    pub forms: &'static [&'static [Argument]],
    pub keyed: Keyed,
    pub area: u8,
}
#[derive(Debug)]
pub struct Argument {
    pub name: &'static [u8],
    pub shown: &'static [u8],
    pub place: Place,
    pub kind: Kind,
    pub occurs: Occurs,
    pub line: &'static [u8],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Positional,
    Flag,
    Switch,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Occurs {
    Required,
    Optional,
    Repeated { most: u32 },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keyed {
    Never,
    Optional,
    Required,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Number,
    Count,
    /// Inclusive numeric bounds, checked in argv and JSON.
    NumberRange {
        min: u64,
        max: u64,
    },
    /// A closed word or a prefix followed by a bounded number.
    NamedNumber {
        words: &'static [&'static [u8]],
        prefixes: &'static [&'static [u8]],
        min: u64,
        max: u64,
    },
    Word {
        set: &'static [&'static [u8]],
    },
    Text,
    /// A literal name: a dash is a name, never stdin.
    Literal,
    File,
    Duration,
    Proposal,
    Digest,
    Condition,
    Subject,
    Project {
        empty: bool,
    },
    Turn,
    Cursor,
    Group,
    Order,
    Tool,
}
#[must_use]
pub const fn positional(
    name: &'static [u8],
    shown: &'static [u8],
    kind: Kind,
    occurs: Occurs,
    line: &'static [u8],
) -> Argument {
    Argument { name, shown, kind, occurs, line, place: Place::Positional }
}
#[must_use]
pub const fn flag(
    name: &'static [u8],
    shown: &'static [u8],
    kind: Kind,
    occurs: Occurs,
    line: &'static [u8],
) -> Argument {
    Argument { name, shown, kind, occurs, line, place: Place::Flag }
}
#[must_use]
pub const fn switch(name: &'static [u8], line: &'static [u8]) -> Argument {
    Argument { name, shown: b"", kind: Kind::Word { set: &[] }, occurs: Occurs::Optional, line, place: Place::Switch }
}
#[must_use]
pub const fn repeated(
    name: &'static [u8],
    shown: &'static [u8],
    kind: Kind,
    most: u32,
    line: &'static [u8],
) -> Argument {
    flag(name, shown, kind, Occurs::Repeated { most }, line)
}
