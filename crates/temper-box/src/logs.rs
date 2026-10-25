//! Private bounded diagnostic logs (protocol/setup.md, 7.2). One owned kernel
//! operation at a time; a retained piece gates further diagnostic reads.
use crate::{host::LINE_BYTES, layout::Placed, lines::joined};
use alloc::boxed::Box;
use skein_io::kernel::{self, Complete, Done, Entry, Error, Fd, Kind, Op, OpenHow, Submit};
use skein_lib::{Decimal, List, Queue, Token, Wall, bytes};

const OP: Token = Token::new(u64::MAX - 9);
#[derive(Debug)]
struct Log {
    name: Box<[u8]>,
    fd: Option<Fd>,
    size: u32,
    failed: bool,
}
#[derive(Debug)]
pub(crate) struct Piece {
    pub step: usize,
    pub bytes: Box<[u8]>,
    pub wall: Wall,
}
impl Piece {
    pub fn echo(&self, name: &[u8]) -> Box<[u8]> {
        joined(&[name, b": ", &self.bytes, ending(&self.bytes)])
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    BoxMake,
    BoxOpen,
    OldOpen,
    PreviousOpen,
    List,
    Remove,
    PreviousClose,
    PreviousRemove,
    OldClose,
    Move,
    Make,
    Open,
    Create,
    Idle,
    Write,
    RotateClose,
    Rotate,
    Recreate,
    FailedClose,
    Cleanup,
    Closed,
}
#[derive(Debug)]
pub(crate) struct Logs {
    files: List<Log>,
    data: Box<[u8]>,
    bound: u32,
    phase: Phase,
    waiting: bool,
    box_dir: Option<Fd>,
    old: Option<Fd>,
    previous: Option<Fd>,
    dir: Option<Fd>,
    entry: Option<Box<[u8]>>,
    at: u32,
    unavailable: bool,
    notice: Option<Box<[u8]>>,
    piece: Option<Piece>,
    record: Option<Box<[u8]>>,
    written: u32,
    done: bool,
}
impl Logs {
    pub fn new(steps: &[Placed], data: &[u8], bound: u32) -> Self {
        let mut logs = List::with_capacity(u32::try_from(steps.len()).expect("bounded steps"));
        for placed in steps {
            let mut name = bytes::copy_of(placed.step.name());
            for byte in &mut name {
                if *byte == b' ' {
                    *byte = b'-';
                }
            }
            // Validate the original name too: '.' must not become a legal '.log'.
            let failed = !kernel::is_name(&name);
            logs.push(Log { name: joined(&[&name, b".log"]), fd: None, size: 0, failed }).expect("one log per step");
        }
        Self {
            files: logs,
            data: bytes::copy_of(data),
            bound,
            phase: Phase::BoxMake,
            waiting: false,
            box_dir: None,
            old: None,
            previous: None,
            dir: None,
            entry: None,
            at: 0,
            unavailable: false,
            notice: None,
            piece: None,
            record: None,
            written: 0,
            done: false,
        }
    }
    pub fn owns(token: Token) -> bool {
        token == OP
    }
    pub fn ready(&self) -> bool {
        self.phase == Phase::Idle && self.notice.is_none()
    }
    pub fn takes(&self) -> bool {
        self.ready() && self.phase == Phase::Idle && self.piece.is_none()
    }
    pub fn failed(&self, step: usize) -> bool {
        self.unavailable || self.files.get(u32::try_from(step).expect("step index")).expect("log step").failed
    }
    pub fn path(&self, step: usize) -> Box<[u8]> {
        joined(&[&self.directory(), &self.files.get(u32::try_from(step).expect("step index")).expect("log step").name])
    }
    fn directory(&self) -> Box<[u8]> {
        joined(&[&self.data, if self.data.as_ref() == b"/" { b"" } else { b"/" }, b"box/logs/"])
    }
    pub fn take_notice(&mut self) -> Option<Box<[u8]>> {
        self.notice.take()
    }
    pub fn has_notice(&self) -> bool {
        self.notice.is_some()
    }
    pub fn put(&mut self, piece: Piece) {
        assert!(self.takes(), "one retained log piece");
        let step = u32::try_from(piece.step).expect("step index");
        self.at = step;
        if self.failed(piece.step) {
            self.done = true;
        } else {
            let record = joined(&[&timestamp(piece.wall), b" ", &piece.bytes, ending(&piece.bytes)]);
            let log = self.files.get(step).expect("log step");
            let length = u32::try_from(record.len()).expect("bounded record");
            assert!(length <= self.bound, "one whole record fits");
            self.phase = if length > self.bound.checked_sub(log.size).expect("file within bound") {
                Phase::RotateClose
            } else {
                Phase::Write
            };
            self.record = Some(record);
            self.written = 0;
            self.done = false;
        }
        self.piece = Some(piece);
    }
    #[expect(clippy::manual_map, reason = "programming-model.md: match rather than closure combinators")]
    pub fn step(&self) -> Option<usize> {
        match &self.piece {
            Some(piece) => Some(piece.step),
            None => None,
        }
    }
    pub fn completed(&self) -> bool {
        self.done && self.notice.is_none()
    }
    pub fn take(&mut self) -> Option<Piece> {
        if !self.completed() {
            return None;
        }
        self.done = false;
        self.piece.take()
    }
    pub fn settled(&self) -> bool {
        self.ready() && self.phase == Phase::Idle && self.piece.is_none() && !self.waiting
    }
    pub fn closed(&self) -> bool {
        self.phase == Phase::Closed
    }
    pub fn close(&mut self) {
        assert!(self.settled(), "logs drain before closing");
        self.unavailable = false;
        self.phase = Phase::Cleanup;
    }
    pub fn pending(&self) -> bool {
        !self.waiting && self.notice.is_none() && self.phase != Phase::Closed && self.phase != Phase::Idle
    }
    fn log(&self) -> &Log {
        self.files.get(self.at).expect("active log")
    }
    fn log_mut(&mut self) -> &mut Log {
        self.files.get_mut(self.at).expect("active log")
    }
    fn directory_fd(&self) -> Fd {
        self.box_dir.expect("opened box")
    }
    pub fn tick(&mut self, root: Fd, out: &mut Queue<Submit>) {
        if self.waiting || self.notice.is_some() || out.room() == 0 {
            return;
        }
        let kind = match self.phase {
            Phase::BoxMake => Op::MakeDirectory { dir: root, name: bytes::copy_of(b"box"), mode: 0o700 },
            Phase::BoxOpen => Op::Open { root, path: bytes::copy_of(b"box"), how: OpenHow::DirectoryNoFollow },
            Phase::OldOpen | Phase::Open => {
                Op::Open { root: self.directory_fd(), path: bytes::copy_of(b"logs"), how: OpenHow::DirectoryNoFollow }
            }
            Phase::PreviousOpen => Op::Open {
                root: self.directory_fd(),
                path: bytes::copy_of(b"logs.previous"),
                how: OpenHow::DirectoryNoFollow,
            },
            Phase::List => Op::List {
                fd: self.previous.expect("previous logs"),
                entries: Box::new([Entry::BLANK]),
                names: bytes::zeroed(kernel::LONGEST_NAME),
            },
            Phase::Remove => Op::Remove {
                dir: self.previous.expect("previous logs"),
                name: self.entry.take().expect("listed entry"),
                directory: false,
            },
            Phase::PreviousClose => Op::Close { fd: self.previous.take().expect("previous logs") },
            Phase::PreviousRemove => {
                Op::Remove { dir: self.directory_fd(), name: bytes::copy_of(b"logs.previous"), directory: true }
            }
            Phase::OldClose => Op::Close { fd: self.old.take().expect("old logs") },
            Phase::Move => Op::Rename {
                from_dir: self.directory_fd(),
                from: bytes::copy_of(b"logs"),
                to_dir: self.directory_fd(),
                to: bytes::copy_of(b"logs.previous"),
                no_replace: false,
            },
            Phase::Make => Op::MakeDirectory { dir: self.directory_fd(), name: bytes::copy_of(b"logs"), mode: 0o700 },
            Phase::Create => {
                if self.at == self.files.len() {
                    self.phase = Phase::Idle;
                    self.notice = Some(joined(&[b"Logs: ", &self.directory(), b"\n"]));
                    return;
                }
                if self.log().failed {
                    self.refuse_step(Error::InvalidArgument);
                    self.at = self.at.checked_add(1).expect("log index");
                    return;
                }
                Op::Open {
                    root: self.dir.expect("logs directory"),
                    path: self.log().name.clone(),
                    how: OpenHow::CreateNoFollow { mode: Some(0o600) },
                }
            }
            Phase::Write => Op::Write {
                fd: self.log().fd.expect("log file"),
                bytes: self.record.take().expect("retained record"),
                from: self.written,
                at: u64::from(self.log().size).checked_add(u64::from(self.written)).expect("log offset"),
            },
            Phase::RotateClose | Phase::FailedClose => Op::Close { fd: self.log_mut().fd.take().expect("log file") },
            Phase::Rotate => Op::Rename {
                from_dir: self.dir.expect("logs directory"),
                from: self.log().name.clone(),
                to_dir: self.dir.expect("logs directory"),
                to: joined(&[&self.log().name, b".1"]),
                no_replace: false,
            },
            Phase::Recreate => Op::Open {
                root: self.dir.expect("logs directory"),
                path: self.log().name.clone(),
                how: OpenHow::CreateNoFollow { mode: Some(0o600) },
            },
            Phase::Cleanup => {
                // At most one descriptor is released in a turn. Close errors still
                // consume the descriptor, as for the record writer.
                let fd = self.cleanup_fd();
                if let Some(fd) = fd {
                    Op::Close { fd }
                } else {
                    self.phase = if self.unavailable { Phase::Idle } else { Phase::Closed };
                    return;
                }
            }
            Phase::Idle | Phase::Closed => return,
        };
        out.push(Submit { op: OP, kind });
        self.waiting = true;
    }
    fn cleanup_fd(&mut self) -> Option<Fd> {
        for index in 0..self.files.len() {
            let log = self.files.get_mut(index).expect("log index");
            if log.fd.is_some() {
                return log.fd.take();
            }
        }
        if self.previous.is_some() {
            return self.previous.take();
        }
        if self.old.is_some() {
            return self.old.take();
        }
        if self.dir.is_some() {
            return self.dir.take();
        }
        self.box_dir.take()
    }
    pub fn complete(&mut self, complete: Complete) {
        assert!(self.waiting, "one logger operation");
        self.waiting = false;
        // A failed write returns its buffer too; preserve the original piece for fallback.
        let mut kind = Some(complete.kind);
        if self.phase == Phase::Write {
            let Some(Op::Write { bytes, .. }) = kind.take() else { unreachable!("write buffer") };
            self.record = Some(bytes);
        }
        match (self.phase, complete.result) {
            (Phase::BoxMake, Ok(Done::Nothing) | Err(Error::Exists)) => self.phase = Phase::BoxOpen,
            (Phase::BoxOpen, Ok(Done::Fd(fd))) => {
                self.box_dir = Some(fd);
                self.phase = Phase::OldOpen;
            }
            (Phase::OldOpen, Ok(Done::Fd(fd))) => {
                self.old = Some(fd);
                self.phase = Phase::PreviousOpen;
            }
            (Phase::OldOpen, Err(Error::NotFound)) | (Phase::Move, Ok(Done::Nothing)) => self.phase = Phase::Make,
            (Phase::PreviousOpen, Err(Error::NotFound)) | (Phase::PreviousRemove, Ok(Done::Nothing)) => {
                self.phase = Phase::OldClose;
            }
            (Phase::PreviousOpen, Ok(Done::Fd(fd))) => {
                self.previous = Some(fd);
                self.phase = Phase::List;
            }
            (Phase::List, Ok(Done::Count(0))) => self.phase = Phase::PreviousClose,
            (Phase::List, Ok(Done::Count(1))) => {
                let Some(Op::List { entries, names, .. }) = kind.take() else { unreachable!("list buffer") };
                let entry = entries.first().expect("one entry");
                if entry.kind == Kind::Directory {
                    self.refuse_run(Error::IsADirectory);
                } else {
                    self.entry = Some(bytes::copy_of(entry.name(&names).expect("kernel entry name")));
                    self.phase = Phase::Remove;
                }
            }
            (Phase::Remove, Ok(Done::Nothing)) => self.phase = Phase::List,
            (Phase::PreviousClose, Ok(Done::Nothing)) => self.phase = Phase::PreviousRemove,
            (Phase::OldClose, Ok(Done::Nothing)) => self.phase = Phase::Move,
            (Phase::Make, Ok(Done::Nothing)) => self.phase = Phase::Open,
            (Phase::Open, Ok(Done::Fd(fd))) => {
                self.dir = Some(fd);
                self.phase = Phase::Create;
                self.at = 0;
            }
            (Phase::Create, Ok(Done::Fd(fd))) => {
                self.log_mut().fd = Some(fd);
                self.at = self.at.checked_add(1).expect("log index");
            }
            (Phase::Create, Err(error)) => {
                self.refuse_step(error);
                self.at = self.at.checked_add(1).expect("log index");
            }
            (Phase::Write, Ok(Done::Count(count))) if count > 0 => {
                self.written = self.written.checked_add(count).expect("bounded write");
                if usize::try_from(self.written).expect("written bytes") == self.record.as_ref().expect("record").len()
                {
                    let written = self.written;
                    self.log_mut().size = self.log().size.checked_add(written).expect("bounded log");
                    self.record = None;
                    self.phase = Phase::Idle;
                    self.done = true;
                }
            }
            (Phase::Write, Ok(Done::Count(0))) => self.fail_piece(Error::Other(0)),
            (Phase::RotateClose, Ok(Done::Nothing)) => self.phase = Phase::Rotate,
            (Phase::Rotate, Ok(Done::Nothing)) => self.phase = Phase::Recreate,
            (Phase::Recreate, Ok(Done::Fd(fd))) => {
                self.log_mut().fd = Some(fd);
                self.log_mut().size = 0;
                self.phase = Phase::Write;
            }
            (Phase::Write | Phase::RotateClose | Phase::Rotate | Phase::Recreate, Err(error)) => self.fail_piece(error),
            (Phase::FailedClose, Ok(Done::Nothing) | Err(_)) => {
                self.phase = Phase::Idle;
                self.done = true;
            }
            (Phase::Cleanup, Ok(Done::Nothing) | Err(_)) => {}
            (_, Err(error)) => self.refuse_run(error),
            (_, Ok(_)) => unreachable!("logger operation completion shape"),
        }
    }
    fn refuse_step(&mut self, error: Error) {
        self.log_mut().failed = true;
        self.notice = Some(joined(&[
            b"box: cannot write ",
            &self.path(usize::try_from(self.at).expect("step index")),
            b": ",
            &error_name(error),
            b"; the step's lines follow here\n",
        ]));
    }
    fn fail_piece(&mut self, error: Error) {
        self.refuse_step(error);
        self.record = None;
        self.phase = if self.log().fd.is_some() {
            Phase::FailedClose
        } else {
            self.done = true;
            Phase::Idle
        };
    }
    fn refuse_run(&mut self, error: Error) {
        self.unavailable = true;
        self.notice = Some(joined(&[
            b"box: cannot write ",
            &self.directory(),
            b": ",
            &error_name(error),
            b"; the steps' lines follow here\n",
        ]));
        self.entry = None;
        self.phase = Phase::Cleanup;
    }
    pub fn worst_case(steps: &[Placed], data: &[u8]) -> u64 {
        let count = u32::try_from(steps.len()).expect("bounded steps");
        let mut names = 0_u64;
        for step in steps {
            names = names
                .checked_add(u64::try_from(step.step.name().len()).expect("name size").checked_add(4).expect("suffix"))
                .expect("log names");
        }
        let path = u64::try_from(data.len())
            .expect("data path")
            .checked_add(names)
            .expect("log paths")
            .checked_add(256)
            .expect("notice words");
        List::<Log>::worst_case(count)
            .expect("log table")
            .checked_add(path.checked_mul(4).expect("paths and notices"))
            .expect("logger memory")
            .checked_add(u64::from(LINE_BYTES).checked_mul(3).expect("piece, record, echo"))
            .expect("logger payloads")
            .checked_add(u64::try_from(kernel::LONGEST_NAME).expect("directory names"))
            .expect("logger listing")
            .checked_add(u64::try_from(size_of::<Self>()).expect("logger state"))
            .expect("logger memory")
    }
    pub fn max_notice(steps: &[Placed], data: &[u8]) -> u32 {
        let mut longest = 0;
        for step in steps {
            longest = longest.max(u32::try_from(step.step.name().len()).expect("step name"));
        }
        u32::try_from(data.len())
            .expect("data path")
            .checked_add(longest)
            .expect("log path")
            .checked_add(256)
            .expect("notice words")
    }
}
fn ending(piece: &[u8]) -> &'static [u8] {
    if piece.ends_with(b"\n") { b"" } else { b"\n" }
}
fn error_name(error: Error) -> Box<[u8]> {
    let name: &[u8] = match error {
        Error::Permission => b"Permission",
        Error::NotFound => b"NotFound",
        Error::Exists => b"Exists",
        Error::NotADirectory => b"NotADirectory",
        Error::IsADirectory => b"IsADirectory",
        Error::NoSpace => b"NoSpace",
        Error::ReadOnly => b"ReadOnly",
        Error::TooManyLinks => b"TooManyLinks",
        Error::NameTooLong => b"NameTooLong",
        Error::Escape => b"Escape",
        Error::NotAFile => b"NotAFile",
        Error::TooManyOpenFiles => b"TooManyOpenFiles",
        Error::NoBufferSpace => b"NoBufferSpace",
        Error::Cancelled => b"Cancelled",
        Error::InvalidArgument => b"InvalidArgument",
        Error::Busy => b"Busy",
        Error::NotEmpty => b"NotEmpty",
        Error::Refused => b"Refused",
        Error::Reset => b"Reset",
        Error::BrokenPipe => b"BrokenPipe",
        Error::NotConnected => b"NotConnected",
        Error::AddressInUse => b"AddressInUse",
        Error::AddressNotAvailable => b"AddressNotAvailable",
        Error::Unreachable => b"Unreachable",
        Error::TimedOut => b"TimedOut",
        Error::TooLate => b"TooLate",
        Error::Other(code) => {
            return joined(&[
                b"Other(",
                if code < 0 { b"-" } else { b"" },
                Decimal::of(u64::from(code.unsigned_abs())).as_bytes(),
                b")",
            ]);
        }
    };
    bytes::copy_of(name)
}
/// Wall fits 1970..2554. Whole Gregorian years and months are a small bounded
/// loop; every subtraction has its preceding comparison, including leap days.
fn timestamp(wall: Wall) -> Box<[u8]> {
    let seconds = wall.as_secs();
    let mut days = seconds.div_euclid(86_400);
    let mut year = 1970_u64;
    for _ in 1970_u32..=2554 {
        let leap = year.rem_euclid(4) == 0 && (year.rem_euclid(100) != 0 || year.rem_euclid(400) == 0);
        let length = if leap { 366 } else { 365 };
        if days < length {
            break;
        }
        days = days.checked_sub(length).expect("whole year");
        year = year.checked_add(1).expect("bounded year");
    }
    let mut month = 1_u64;
    for length in [
        31_u64,
        if year.rem_euclid(4) == 0 && (year.rem_euclid(100) != 0 || year.rem_euclid(400) == 0) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ] {
        if days < length {
            break;
        }
        days = days.checked_sub(length).expect("whole month");
        month = month.checked_add(1).expect("bounded month");
    }
    joined(&[
        Decimal::of(year).as_bytes(),
        b"-",
        &pair(month),
        b"-",
        &pair(days.checked_add(1).expect("day of month")),
        b"T",
        &pair(seconds.div_euclid(3_600).rem_euclid(24)),
        b":",
        &pair(seconds.div_euclid(60).rem_euclid(60)),
        b":",
        &pair(seconds.rem_euclid(60)),
        b"Z",
    ])
}
fn pair(value: u64) -> [u8; 2] {
    [
        b'0'.checked_add(u8::try_from(value.div_euclid(10)).expect("pair tens")).expect("ASCII digit"),
        b'0'.checked_add(u8::try_from(value.rem_euclid(10)).expect("pair ones")).expect("ASCII digit"),
    ]
}
