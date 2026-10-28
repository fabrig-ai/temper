//! The box's private started records (protocol/setup.md, 7.1).
//! `MakeDirectory` names one entry, so started is made beneath an opened box.
use crate::{layout::Placed, lines::joined, machine::Input};
use alloc::boxed::Box;
use skein_io::kernel::{Complete, Done, Error, Fd, Op, OpenHow, Submit};
use skein_lib::{Decimal, List, Queue, Token, bytes};
const PROBE_OPEN: Token = Token::new(u64::MAX - 1);
const PROBE_CLOSE: Token = Token::new(u64::MAX - 2);
const MAKE_BOX: Token = Token::new(u64::MAX - 3);
const MAKE_STARTED: Token = Token::new(u64::MAX - 4);
const CREATE: Token = Token::new(u64::MAX - 5);
const RECORD_CLOSE: Token = Token::new(u64::MAX - 6);
const BOX_OPEN: Token = Token::new(u64::MAX - 7);
const BOX_CLOSE: Token = Token::new(u64::MAX - 8);
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Probe,
    ProbeClose { fd: Fd },
    Idle,
    Box { forge: u16 },
    BoxOpen { forge: u16 },
    Started { forge: u16, dir: Fd },
    BoxClose { forge: u16, fd: Fd, error: Option<Error> },
    Create { forge: u16 },
    Close { forge: u16, fd: Fd },
}
#[derive(Debug)]
pub(crate) struct Records {
    forges: List<u16>,
    recorded: List<u16>,
    at: u32,
    phase: Phase,
    waiting: bool,
    notice: Option<Box<[u8]>>,
    answer: Option<Input>,
}
impl Records {
    pub fn new(steps: &[Placed]) -> Self {
        let count = u32::try_from(steps.len()).expect("bounded steps");
        let mut forges = List::with_capacity(count);
        for step in steps {
            if let Some(asked) = step.asked
                && asked.recorded
                && !forges.as_slice().contains(&asked.forge)
            {
                forges.push(asked.forge).expect("bounded groups");
            }
        }
        Self {
            forges,
            recorded: List::with_capacity(count),
            at: 0,
            phase: Phase::Probe,
            waiting: false,
            notice: None,
            answer: None,
        }
    }
    pub fn recorded(&self) -> &[u16] {
        self.recorded.as_slice()
    }
    pub fn has_notice(&self) -> bool {
        self.notice.is_some()
    }
    pub fn take_notice(&mut self) -> Option<Box<[u8]>> {
        self.notice.take()
    }
    pub fn take_answer(&mut self) -> Option<Input> {
        self.answer.take()
    }
    pub fn ready(&self) -> bool {
        self.phase == Phase::Idle && self.notice.is_none() && self.answer.is_none()
    }
    pub fn pending(&self) -> bool {
        !self.waiting && !self.ready()
    }
    pub fn write(&mut self, forge: u16) {
        assert!(self.ready(), "one group records at a time");
        self.phase = Phase::Box { forge };
    }
    pub fn owns(op: Token) -> bool {
        [PROBE_OPEN, PROBE_CLOSE, MAKE_BOX, MAKE_STARTED, CREATE, RECORD_CLOSE, BOX_OPEN, BOX_CLOSE].contains(&op)
    }
    pub fn tick(&mut self, root: Fd, subs: &mut Queue<Submit>) {
        if self.waiting || self.notice.is_some() || self.answer.is_some() || subs.room() == 0 {
            return;
        }
        let (op, kind) = match self.phase {
            Phase::Probe => {
                let Some(forge) = self.forges.get(self.at) else {
                    self.phase = Phase::Idle;
                    return;
                };
                (PROBE_OPEN, Op::Open { root, path: path(*forge), how: OpenHow::Read })
            }
            Phase::ProbeClose { fd } => (PROBE_CLOSE, Op::Close { fd }),
            Phase::Idle => return,
            Phase::Box { .. } => (MAKE_BOX, Op::MakeDirectory { dir: root, name: bytes::copy_of(b"box"), mode: 0o700 }),
            Phase::BoxOpen { .. } => {
                (BOX_OPEN, Op::Open { root, path: bytes::copy_of(b"box"), how: OpenHow::Directory })
            }
            Phase::Started { dir, .. } => {
                (MAKE_STARTED, Op::MakeDirectory { dir, name: bytes::copy_of(b"started"), mode: 0o700 })
            }
            Phase::BoxClose { fd, .. } => (BOX_CLOSE, Op::Close { fd }),
            Phase::Create { forge } => {
                (CREATE, Op::Open { root, path: path(forge), how: OpenHow::Create { mode: Some(0o600) } })
            }
            Phase::Close { fd, .. } => (RECORD_CLOSE, Op::Close { fd }),
        };
        subs.push(Submit { op, kind });
        self.waiting = true;
    }
    pub fn complete(&mut self, complete: Complete) {
        self.waiting = false;
        match self.phase {
            Phase::Probe => {
                let forge = *self.forges.get(self.at).expect("probing forge");
                match complete.result {
                    Ok(Done::Fd(fd)) => self.phase = Phase::ProbeClose { fd },
                    Err(Error::NotFound) => self.at = self.at.saturating_add(1),
                    Err(error) => {
                        self.notice = Some(diagnostic(forge, b"open", error));
                        self.at = self.at.saturating_add(1);
                    }
                    Ok(_) => unreachable!("open completion shape"),
                }
            }
            Phase::ProbeClose { .. } => {
                let forge = *self.forges.get(self.at).expect("probing forge");
                match complete.result {
                    Ok(Done::Nothing) => {
                        self.recorded.push(forge).expect("bounded records");
                    }
                    Err(error) => self.notice = Some(diagnostic(forge, b"close", error)),
                    Ok(_) => unreachable!("close completion shape"),
                }
                self.at = self.at.saturating_add(1);
                self.phase = Phase::Probe;
            }
            Phase::Box { forge } => match complete.result {
                Ok(Done::Nothing) | Err(Error::Exists) => self.phase = Phase::BoxOpen { forge },
                Err(error) => self.failed(forge, b"mkdir box", error),
                Ok(_) => unreachable!("mkdir completion shape"),
            },
            Phase::BoxOpen { forge } => match complete.result {
                Ok(Done::Fd(dir)) => self.phase = Phase::Started { forge, dir },
                Err(error) => self.failed(forge, b"open box", error),
                Ok(_) => unreachable!("directory open completion shape"),
            },
            Phase::Started { forge, dir } => {
                let error = match complete.result {
                    Ok(Done::Nothing) | Err(Error::Exists) => None,
                    Err(error) => Some(error),
                    Ok(_) => unreachable!("mkdir completion shape"),
                };
                self.phase = Phase::BoxClose { forge, fd: dir, error };
            }
            Phase::BoxClose { forge, error, .. } => match error {
                Some(error) => self.failed(forge, b"mkdir box/started", error),
                None => match complete.result {
                    Ok(Done::Nothing) => self.phase = Phase::Create { forge },
                    Err(error) => self.failed(forge, b"close box", error),
                    Ok(_) => unreachable!("close completion shape"),
                },
            },
            Phase::Create { forge } => match complete.result {
                Ok(Done::Fd(fd)) => self.phase = Phase::Close { forge, fd },
                Err(Error::Exists) => self.written(forge),
                Err(error) => self.failed(forge, b"create", error),
                Ok(_) => unreachable!("create completion shape"),
            },
            Phase::Close { forge, .. } => match complete.result {
                Ok(Done::Nothing) => self.written(forge),
                Err(error) => self.failed(forge, b"close", error),
                Ok(_) => unreachable!("close completion shape"),
            },
            Phase::Idle => unreachable!("no record operation while idle"),
        }
    }
    fn written(&mut self, forge: u16) {
        self.phase = Phase::Idle;
        self.answer = Some(Input::Written { forge });
    }
    fn failed(&mut self, forge: u16, operation: &[u8], error: Error) {
        self.notice = Some(diagnostic(forge, operation, error));
        self.phase = Phase::Idle;
        self.answer = Some(Input::NotWritten { forge });
    }
}
fn path(forge: u16) -> Box<[u8]> {
    joined(&[b"box/started/", Decimal::of(u64::from(forge)).as_bytes()])
}
fn diagnostic(forge: u16, operation: &[u8], error: Error) -> Box<[u8]> {
    let why: &[u8] = match error {
        Error::Permission => b"permission denied",
        Error::NotFound => b"not found",
        Error::Exists => b"exists",
        Error::NotADirectory => b"not a directory",
        Error::IsADirectory => b"is a directory",
        Error::NoSpace => b"no space",
        Error::ReadOnly => b"read only",
        Error::TooManyLinks => b"too many links",
        Error::NameTooLong => b"name too long",
        Error::Escape => b"outside root",
        Error::NotAFile => b"not a file",
        Error::TooManyOpenFiles => b"too many open files",
        Error::NoBufferSpace => b"no buffer space",
        Error::Cancelled => b"cancelled",
        Error::InvalidArgument => b"invalid argument",
        Error::Busy => b"busy",
        Error::NotEmpty => b"not empty",
        Error::Refused
        | Error::Reset
        | Error::BrokenPipe
        | Error::NotConnected
        | Error::AddressInUse
        | Error::AddressNotAvailable
        | Error::Unreachable
        | Error::TimedOut
        | Error::TooLate => b"unexpected file error",
        Error::Other(code) => {
            return joined(&[
                b"box: ",
                operation,
                b" ",
                &path(forge),
                b": error ",
                if code < 0 { b"-" } else { b"" },
                Decimal::of(u64::from(code.unsigned_abs())).as_bytes(),
                b"\n",
            ]);
        }
    };
    joined(&[b"box: ", operation, b" ", &path(forge), b": ", why, b"\n"])
}
