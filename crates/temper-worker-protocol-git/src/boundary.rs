//! The git component's boundary (protocol/git.md, sections 2 and 7).
use alloc::boxed::Box;
use skein_io::{self as io, kernel};
use skein_lib::{Queue, Secret, Time, Token};
use temper_worker_domain_checkout::git;

/// An identity's value, held only while its operation is in flight.
/// The token's debug representation contains its length only.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Identity {
    pub token: Secret,
    pub login: Box<[u8]>,
    pub name: Box<[u8]>,
    pub email: Box<[u8]>,
}

/// Settings the service supplies; paths resolve beneath `root`.
#[derive(Debug)]
pub struct Settings {
    pub git: Option<Box<[u8]>>,
    pub home: Box<[u8]>,
    pub root: kernel::Fd,
    pub prefix: u64,
    pub serial: u64,
    pub largest_read: u32,
    pub largest_room: u32,
}

/// Each Io request ends with exactly one Done echoing its owner.
#[derive(Debug)]
#[cfg_attr(
    target_pointer_width = "64",
    expect(
        clippy::large_enum_variant,
        reason = "bounded operation payloads remain inline and are counted in worst_case"
    )
)]
pub enum Request {
    Io { owner: Token, op: git::Op, deadline: Time, identity: Option<Identity> },
    CancelIo { owner: Token },
    Shutdown,
}

/// Start's terminal, or an operation's terminal.
#[derive(PartialEq, Eq, Debug)]
#[expect(clippy::large_enum_variant, reason = "bounded diagnostics stay inline, as the checkout's Done")]
pub enum Event {
    Started,
    Unusable { why: Unusable },
    Done { owner: Token, done: git::Done },
}

/// Why the component could not start.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Unusable {
    Version { found: Option<Version> },
    Layout { error: kernel::Error },
}

/// The three numeric fields of git's version.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

/// Parent-owned queues, reserved for `MAX_OUT` before every entry point.
#[derive(Debug)]
pub struct Outputs {
    pub events: Queue<Event>,
    pub io: Queue<io::Request>,
    pub kernel: Queue<kernel::Submit>,
}
