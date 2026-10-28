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
/// A tool's notes are at most this many bytes (client/tools.md, 5.1).
pub const NOTES_BYTES: usize = 1_024;
/// A tool and each of its argument forms, tried in order.
#[derive(Debug)]
pub struct Tool {
    pub name: &'static [u8],
    pub line: &'static [u8],
    /// What it prints, one line.
    pub prints: &'static [u8],
    pub result: &'static [u8],
    pub forms: &'static [&'static [Argument]],
    pub keyed: Keyed,
    pub area: u8,
    /// What this tool alone does beyond its line; lines end with `\n`.
    pub notes: &'static [u8],
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
    /// UTC Gregorian time to the minute.
    Time,
    Proposal,
    Digest,
    Condition,
    Project {
        empty: bool,
    },
    Turn,
    Cursor,
    Group,
    Order,
    Tool,
}
/// What the parser accepts of an argument beyond its kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bound {
    None,
    Numbers { least: u64, most: u64 },
    Bytes { most: u64 },
    Seconds { most: u64 },
}
/// One source for the parser's checks and the help.
#[must_use]
pub fn bound(argument: &Argument, limits: &Limits) -> Bound {
    match argument.kind {
        Kind::NumberRange { min, max } | Kind::NamedNumber { min, max, .. } => Bound::Numbers { least: min, most: max },
        Kind::Count => Bound::Numbers { least: 1, most: u64::from(limits.rows) },
        Kind::Number if argument.name == b"forge" => Bound::Numbers { least: 0, most: 65_535 },
        Kind::Text | Kind::Literal => Bound::Bytes { most: limits.ask_bytes },
        Kind::File if argument.name == b"items" || argument.name == b"instructions" || argument.name == b"spec" => {
            Bound::Bytes { most: limits.ask_bytes }
        }
        Kind::Duration if argument.name == b"timeout" => {
            Bound::Seconds { most: limits.wait_most.as_nanos().checked_div(1_000_000_000).expect("nonzero divisor") }
        }
        Kind::Number
        | Kind::File
        | Kind::Duration
        | Kind::Time
        | Kind::Word { .. }
        | Kind::Proposal
        | Kind::Digest
        | Kind::Condition
        | Kind::Project { .. }
        | Kind::Turn
        | Kind::Cursor
        | Kind::Group
        | Kind::Order
        | Kind::Tool => Bound::None,
    }
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

/// The published vocabulary of argument placeholders.
pub const PLACEHOLDERS: &[&[u8]] = &[
    b"TASK",
    b"PROJECT",
    b"FORGE",
    b"INSTALLATION",
    b"ACCOUNT",
    b"PROVIDER",
    b"ADAPTER",
    b"SOURCE",
    b"POSITION",
    b"NAME",
    b"REVISION",
    b"REV",
    b"PROPOSAL",
    b"QUESTION",
    b"NUMBER",
    b"TEMPLATE",
    b"CURSOR",
    b"COUNT",
    b"PART",
    b"OFFSET",
    b"PAGE",
    b"TURN",
    b"DIGEST",
    b"CONDITION",
    b"DURATION",
    b"TIME",
    b"PRIORITY",
    b"MODEL",
    b"EFFORT",
    b"AMOUNT",
    b"OWNER",
    b"ORG",
    b"REPOSITORY",
    b"BRANCH",
    b"PREFIX",
    b"GATE",
    b"LABEL",
    b"IDS",
    b"APP",
    b"ROLE",
    b"FILE",
    b"TEXT",
    b"REASON",
    b"KEY",
    b"GROUP",
    b"VERB",
];

/// A published name has at most one separating space.
pub(crate) fn name_words(name: &[u8]) -> (&[u8], Option<&[u8]>) {
    for (at, byte) in name.iter().enumerate() {
        if *byte == b' ' {
            let (group, rest) = name.split_at_checked(at).expect("name position");
            return (group, rest.strip_prefix(b" "));
        }
    }
    (name, None)
}
