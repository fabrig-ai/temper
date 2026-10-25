//! Executable calls beside serve, with every descriptor on one real ring.
mod member;
use super::{Ended, Process, WITHIN};
use skein_io::kernel::{Complete, Done, Error, Exit, Fd, Op, Pipe, Signal, Spawn, Submit, Target, Way};
use skein_lib::{Duration, Queue, Time, Token, Wall};
use skein_shell::{Clock, Host};
use skein_world::{Referee, end_to_end::Streams, real};
use std::{
    collections::VecDeque,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};
use temper_real_loop_world::calls::ReadCondition;

// The service's output bound also holds the complete tools guide (4,698 bytes).
const OUTPUT: usize = 65_536;
const MEMBER: u64 = 1 << 63;
const LINK: Token = Token::new(0);
const SPAWN: Token = Token::new(2);
const WRITE: Token = Token::new(3);
const STDOUT: Token = Token::new(4);
const STDERR: Token = Token::new(5);
const WAIT: Token = Token::new(6);
const REAP: Token = Token::new(7);
const CUT: Token = Token::new(8);
const CANCEL: Token = Token::new(9);
const STARTED: u64 = 1 << 31;
const OWN_CLOSE: u64 = 1 << 32;
// Poll pending promptly without holding up the interrupted call.
const PACE: Duration = Duration::from_millis(100);
// The shared engine document gives the native person's project number one.
pub const PROJECT: &str = "1";

#[derive(Clone)]
pub enum Stdin {
    Nothing,
    Link,
}
#[derive(Clone)]
pub struct Call {
    arguments: Vec<Box<[u8]>>,
    stdin: Stdin,
    directory: bool,
    client: Option<Box<[u8]>>,
}
impl Call {
    #[must_use]
    pub fn new(arguments: &[&str]) -> Self {
        Self {
            arguments: arguments.iter().map(|word| word.as_bytes().into()).collect(),
            stdin: Stdin::Nothing,
            directory: true,
            client: None,
        }
    }
    #[must_use]
    pub fn json(mut self) -> Self {
        self.arguments.push(b"--json".as_slice().into());
        self
    }
    #[must_use]
    pub fn bare(arguments: &[&str]) -> Self {
        Self { directory: false, ..Self::new(arguments) }
    }
    #[must_use]
    pub fn in_directory(self, name: &str) -> Self {
        Self { client: Some(name.as_bytes().into()), ..self }
    }
    #[must_use]
    pub fn sign_in() -> Self {
        Self { stdin: Stdin::Link, ..Self::new(&["sign-in", "-"]) }
    }
}
pub type NextCall = Box<dyn FnOnce(&[Ended]) -> Call>;
pub enum Step {
    Mark(&'static str),
    /// Wait for bytes in the box's captured stderr; requires a box referee.
    Stderr(&'static [u8]),
    Run(Call),
    /// Form a dependent call from the completed calls' documents.
    Then(NextCall),
    Start(Call),
    Join,
    Until(Call, &'static [u8]),
    UntilMatching(Call, ReadCondition),
    Cut,
    Pause,
    Resume,
    NextLink,
    /// Provision a service with only a project member role through jig.
    Member {
        scratch: PathBuf,
        directory: &'static str,
    },
    Open {
        directory: &'static str,
        until: Option<&'static [u8]>,
    },
}
pub struct Story<'a> {
    scratch: &'a Path,
    steps: Vec<Step>,
    forgejo: Option<u32>,
    measured: bool,
    tokens: Vec<String>,
}
impl<'a> Story<'a> {
    /// Bound each new story step, retaining its wall-time window and token observations.
    #[must_use]
    pub fn measured(mut self, tokens: &[&str]) -> Self {
        self.measured = true;
        self.tokens = tokens.iter().map(|name| (*name).into()).collect();
        self
    }
    #[must_use]
    pub fn new(scratch: &'a Path, steps: Vec<Step>) -> Self {
        Self { scratch, steps, forgejo: None, measured: false, tokens: Vec::new() }
    }
    #[must_use]
    pub fn on(scratch: &'a Path, forgejo: &super::forgejo::Throwaway, steps: Vec<Step>) -> Self {
        Self { scratch, steps, forgejo: Some(forgejo.pid()), measured: false, tokens: Vec::new() }
    }
}
pub struct Seen {
    pub calls: Vec<Ended>,
    pub serve: Ended,
    pub mismatches: Vec<String>,
    pub spans: Vec<(Wall, Wall)>,
    /// The call that requested a replacement link through the step that waited for it.
    pub link_spans: Vec<(Wall, Wall)>,
    /// Token modes after each call, in the order passed to `Story::measured`.
    pub tokens: Vec<Vec<Option<u32>>>,
}

enum Input {
    Eof,
    Hold,
    After(&'static [u8]),
}
struct Child {
    base: u64,
    pidfd: Option<Fd>,
    pipes: [Option<Fd>; 3],
    input: Vec<u8>,
    written: usize,
    writing: bool,
    reading: [bool; 2],
    waiting: bool,
    closing: u32,
    ended: Ended,
    cancelled: bool,
    signals: u32,
    typing: Input,
}
impl Child {
    fn new(input: Vec<u8>, base: u64) -> Self {
        Self {
            base,
            pidfd: None,
            pipes: [None; 3],
            input,
            written: 0,
            writing: false,
            reading: [false; 2],
            waiting: false,
            closing: 0,
            ended: Ended { exit: None, stdout: Vec::new(), stderr: Vec::new() },
            cancelled: false,
            signals: 0,
            typing: Input::Eof,
        }
    }
    fn token(&self, token: Token) -> Token {
        Token::new(self.base + token.raw())
    }
    fn close(&mut self, fd: Fd, submit: &mut Queue<Submit>) {
        self.closing += 1;
        submit.push(Submit {
            op: self.token(Token::new(100 + u64::try_from(fd.raw()).expect("descriptor"))),
            kind: Op::Close { fd },
        });
    }
    fn completed(&mut self, complete: Complete, submit: &mut Queue<Submit>, cancelling: bool) {
        let cancelling = cancelling || self.cancelled;
        if complete.op == self.token(CANCEL) || cancelling && complete.op == self.token(CUT) {
            assert!(
                matches!(complete.result, Ok(_) | Err(Error::NotFound | Error::Other(3))),
                "ctl cancellation signal"
            );
            self.signals -= 1;
            return;
        }
        if cancelling && complete.op == self.token(WRITE) && complete.result.is_err() {
            self.writing = false;
            self.written = self.input.len();
            return;
        }
        let done = complete.result.expect("ctl operation succeeds");
        match Token::new(complete.op.raw() - self.base) {
            SPAWN => {
                let (Done::Spawned { pidfd, .. }, Op::Spawn { spawn }) = (done, complete.kind) else {
                    panic!("spawned ctl")
                };
                self.pidfd = Some(pidfd);
                for (slot, pipe) in self.pipes.iter_mut().zip(spawn.pipes) {
                    *slot = pipe.parent;
                }
            }
            op if op.raw() >= 100 => self.closing -= 1,
            WRITE => {
                let Done::Count(count) = done else { panic!("stdin write count") };
                self.writing = false;
                self.written += usize::try_from(count).expect("bounded write");
            }
            STDOUT | STDERR => {
                let index = usize::from(complete.op == self.token(STDERR));
                self.reading[index] = false;
                let (Done::Count(count), Op::PipeRead { buf, .. }) = (done, complete.kind) else {
                    panic!("ctl pipe read")
                };
                let bytes = if index == 0 { &mut self.ended.stdout } else { &mut self.ended.stderr };
                bytes.extend_from_slice(&buf[..usize::try_from(count).expect("bounded read")]);
                assert!(bytes.len() <= OUTPUT, "bounded ctl output");
                if count == 0 {
                    let fd = self.pipes[index + 1].take().expect("output pipe");
                    self.close(fd, submit);
                }
            }
            WAIT => {
                let Done::Exit(exit) = done else { panic!("ctl exit") };
                self.ended.exit = Some(exit);
                submit.push(Submit {
                    op: self.token(REAP),
                    kind: Op::Wait { pidfd: self.pidfd.expect("ctl pidfd"), reap: true },
                });
            }
            REAP => {
                let fd = self.pidfd.take().expect("reaped ctl pidfd");
                self.close(fd, submit);
            }
            CUT => self.signals -= 1,
            _ => panic!("ctl operation token"),
        }
    }
    fn drive(&mut self, submit: &mut Queue<Submit>, cancelling: bool) {
        if self.pidfd.is_none() {
            return;
        }
        if cancelling && !self.cancelled && self.ended.exit.is_none() {
            self.cancelled = true;
            self.signals += 1;
            submit.push(Submit {
                op: self.token(CANCEL),
                kind: Op::Signal { pidfd: self.pidfd.expect("ctl pidfd"), signal: Signal::Kill, to: Target::Child },
            });
        }
        if let Input::After(words) = self.typing
            && skein_lib::bytes::find(&self.ended.stdout, words).is_some()
        {
            self.typing = Input::Eof;
            self.input = b"q\n".to_vec();
        }
        if self.pipes[1..].iter().all(Option::is_none) || cancelling {
            self.typing = Input::Eof;
        }
        if !self.writing
            && matches!(self.typing, Input::Eof)
            && let Some(fd) = self.pipes[0]
        {
            if self.written == self.input.len() {
                self.pipes[0] = None;
                self.close(fd, submit);
            } else {
                self.writing = true;
                submit.push(Submit {
                    op: self.token(WRITE),
                    kind: Op::PipeWrite { fd, bytes: self.input[self.written..].into(), from: 0 },
                });
            }
        }
        for (index, op) in [STDOUT, STDERR].into_iter().enumerate() {
            if !self.reading[index]
                && let Some(fd) = self.pipes[index + 1]
            {
                self.reading[index] = true;
                submit.push(Submit {
                    op: self.token(op),
                    kind: Op::PipeRead { fd, buf: vec![0; 4096].into_boxed_slice() },
                });
            }
        }
        if self.pipes.iter().all(Option::is_none) && !self.waiting {
            self.waiting = true;
            submit.push(Submit {
                op: self.token(WAIT),
                kind: Op::Wait { pidfd: self.pidfd.expect("ctl pidfd"), reap: false },
            });
        }
    }
    fn finished(&self) -> bool {
        self.ended.exit.is_some() && self.pidfd.is_none() && self.closing == 0 && self.signals == 0
    }
}

enum Action {
    Run,
    Until(Call, ReadCondition),
    Stderr(&'static [u8]),
    Signal,
    NextLink,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Admission {
    Batch,
    Open,
}
enum BoxStderr {
    Absent,
    Captured,
}
pub struct Calls {
    root: Option<Fd>,
    client: Box<[u8]>,
    output: Option<Fd>,
    reading: bool,
    closing: u32,
    link: Vec<u8>,
    selected: usize,
    within: Option<Duration>,
    deadline: Option<Time>,
    began: Option<Wall>,
    wall: Wall,
    spans: Vec<(Wall, Wall)>,
    link_spans: Vec<(Wall, Wall)>,
    tokens: Vec<Vec<Option<u32>>>,
    mismatches: Vec<String>,
    steps: VecDeque<Step>,
    child: Option<Child>,
    member: Option<member::Member>,
    action: Option<Action>,
    box_stderr: BoxStderr,
    started: Option<(Child, usize)>,
    awaiting_started: bool,
    cancelling: bool,
    admission: Admission,
    retry: Option<Time>,
    phase: Option<Time>,
    forgejo: Option<u32>,
    ends: Vec<Ended>,
    observed_at: Vec<Option<Wall>>,
    complete: Queue<Complete>,
    submit: Queue<Submit>,
}
impl Calls {
    /// Calls against an already running engine, with its printed link on sign-in's stdin.
    #[must_use]
    pub fn linked(link: &str, client: &Path) -> Self {
        assert!(client.is_absolute(), "absolute client directory");
        let root = skein_shell::open_root(client.parent().expect("client parent")).expect("client root");
        let mut calls = Self::new(root, Vec::new(), None, None);
        calls.client = client.as_os_str().as_bytes().into();
        calls.link = format!("{link}\n").into_bytes();
        calls
    }
    /// Keep the root available between a live referee's read batches.
    pub fn hold(&mut self) {
        self.admission = Admission::Open;
    }
    pub fn release(&mut self) {
        self.admission = Admission::Batch;
    }
    #[must_use]
    pub fn link(&self) -> &[u8] {
        super::printed_link(&self.link[self.selected..]).expect("serve printed its link")
    }
    #[must_use]
    pub fn foreground_idle(&self) -> bool {
        self.steps.is_empty() && self.child.is_none() && self.action.is_none() && !self.awaiting_started
    }
    #[must_use]
    pub fn idle(&self) -> bool {
        self.done()
    }
    /// Queue another step before this batch runs.
    pub fn push(&mut self, step: Step) {
        if !self.cancelling {
            self.steps.push_back(step);
        }
    }
    /// An owner abandoning these calls must cancel: stop admission and retries;
    /// kill in-flight children once their spawn
    /// completes, tolerating natural exit. Drain pipes, wait, reap and close as
    /// usual: `is_empty` remains false until every child and descriptor settles.
    /// Repeated cancellation has the same effect as one request. A future
    /// per-call Binary tree could also make abandonment without cancellation safe.
    pub fn cancel(&mut self) {
        self.cancelling = true;
        self.steps.clear();
        if let Some(member) = &mut self.member {
            member.close();
        }
        self.retry = None;
        self.awaiting_started = false;
        self.action = self.child.as_ref().map(|_| Action::Run);
    }
    /// Completed calls, in the order their steps ran.
    #[must_use]
    pub fn ends(&self) -> &[Ended] {
        &self.ends
    }
    /// Wall input at completed response observation, parallel to ends (not server sample time).
    #[must_use]
    pub fn observed_at(&self) -> &[Option<Wall>] {
        &self.observed_at
    }
    /// All bytes read from the box's stdout, including an incomplete link on failure.
    pub(super) fn stdout(&self) -> &[u8] {
        &self.link
    }
    pub(super) fn pending(&self) -> Option<&Ended> {
        self.child.as_ref().map(|child| &child.ended)
    }
    #[must_use]
    pub(super) fn with_box_stderr(mut self) -> Self {
        self.box_stderr = BoxStderr::Captured;
        self
    }
    /// The box referee checks after each ring wake, including stderr reads.
    pub(super) fn observe_box_stderr(&mut self, stderr: &[u8], now: Time) {
        while let Some(Action::Stderr(bytes)) = &self.action {
            if !stderr.windows(bytes.len()).any(|window| window == *bytes) {
                break;
            }
            self.action = None;
            self.advance(now);
        }
    }
    pub(super) fn new(root: Fd, steps: Vec<Step>, streams: Option<&Streams>, forgejo: Option<u32>) -> Self {
        let mut calls = Self {
            root: Some(root),
            client: b"client".as_slice().into(),
            output: None,
            reading: false,
            closing: 0,
            link: Vec::new(),
            selected: 0,
            within: None,
            deadline: None,
            began: None,
            wall: Wall::from_nanos(0),
            spans: Vec::new(),
            link_spans: Vec::new(),
            tokens: Vec::new(),
            mismatches: Vec::new(),
            steps: steps.into(),
            child: None,
            member: None,
            action: None,
            box_stderr: BoxStderr::Absent,
            started: None,
            awaiting_started: false,
            cancelling: false,
            admission: Admission::Batch,
            retry: None,
            phase: None,
            forgejo,
            ends: Vec::new(),
            observed_at: Vec::new(),
            complete: Queue::with_capacity(32),
            submit: Queue::with_capacity(32),
        };
        if let Some(Streams::Pipes { input, output }) = streams {
            calls.output = Some(*output);
            calls.close(*input);
        }
        calls
    }
    fn close(&mut self, fd: Fd) {
        self.closing += 1;
        self.submit.push(Submit {
            op: Token::new(OWN_CLOSE + u64::try_from(fd.raw()).expect("descriptor")),
            kind: Op::Close { fd },
        });
    }
    fn done(&self) -> bool {
        self.steps.is_empty()
            && self.child.is_none()
            && self.member.is_none()
            && self.started.is_none()
            && self.action.is_none()
    }
    fn spawn(&mut self, program: Box<[u8]>, args: Vec<Box<[u8]>>, input: Vec<u8>, started: bool) {
        let child = Child::new(input, if started { STARTED } else { 0 });
        self.submit.push(Submit {
            op: child.token(SPAWN),
            kind: Op::Spawn {
                spawn: Box::new(Spawn {
                    program,
                    args: args.into_boxed_slice(),
                    env: Box::new([]),
                    root: self.root.expect("scratch root"),
                    dir: Box::new([]),
                    pipes: [Way::In, Way::Out, Way::Out]
                        .into_iter()
                        .enumerate()
                        .map(|(child, way)| Pipe {
                            child: u32::try_from(child).expect("standard descriptor"),
                            way,
                            parent: None,
                        })
                        .collect(),
                }),
            },
        });
        if started {
            assert!(self.started.is_none(), "only one started call");
            let index = self.ends.len();
            self.ends.push(Ended { exit: None, stdout: Vec::new(), stderr: Vec::new() });
            self.observed_at.push(None);
            self.started = Some((child, index));
        } else {
            self.child = Some(child);
        }
    }
    fn start(&mut self, call: Call, started: bool) {
        let mut args: Vec<Box<[u8]>> = vec![b"ctl".as_slice().into()];
        if call.directory {
            args.push(call.client.unwrap_or_else(|| self.client.clone()));
        }
        args.extend(call.arguments);
        let input = match call.stdin {
            Stdin::Nothing => Vec::new(),
            Stdin::Link => {
                let mut bytes = self.link().to_vec();
                bytes.push(b'\n');
                bytes
            }
        };
        self.spawn(env!("CARGO_BIN_EXE_temper").as_bytes().into(), args, input, started);
    }
    fn finish(&mut self, now: Time) {
        if self.started.as_ref().is_some_and(|(child, _)| child.finished()) {
            let (child, index) = self.started.take().expect("finished started call");
            self.ends[index] = child.ended;
            self.observed_at[index] = Some(self.wall);
            self.awaiting_started = false;
        }
        if !self.child.as_ref().is_some_and(Child::finished) {
            return;
        }
        let ended = self.child.take().expect("finished ctl").ended;
        self.finish_step(true);
        match self.action.take().expect("current action") {
            Action::Run => {
                self.ends.push(ended);
                self.observed_at.push(Some(self.wall));
            }
            Action::Until(call, condition) => {
                if condition.matches(&ended.stdout) {
                    self.ends.push(ended);
                    self.observed_at.push(Some(self.wall));
                } else {
                    self.action = Some(Action::Until(call, condition));
                    self.retry = Some(now.saturating_add(PACE));
                }
            }
            Action::Stderr(_) => panic!("a stderr gate has no child"),
            Action::NextLink => unreachable!("link wait has no child"),
            Action::Signal => assert_eq!(
                ended.exit,
                Some(Exit::Code(0)),
                "Forgejo signal: {}",
                String::from_utf8_lossy(&ended.stderr)
            ),
        }
    }
    fn finish_step(&mut self, call: bool) {
        if let Some(wall) = self.began.take()
            && call
        {
            self.spans.push((wall, self.wall));
        }
        self.deadline = None;
    }
    fn bound(&mut self, now: Time) {
        if matches!(self.action, Some(Action::NextLink)) {
            let after =
                self.selected + self.link[self.selected..].iter().position(|b| *b == b'\n').map_or(0, |n| n + 1);
            if super::printed_link(&self.link[after..]).is_some() {
                self.selected = after;
                // Select the newest complete line; an incomplete line is never a link.
                while let Some(end) = self.link[self.selected..].iter().position(|b| *b == b'\n') {
                    let next = self.selected + end + 1;
                    if super::printed_link(&self.link[next..]).is_none() {
                        break;
                    }
                    self.selected = next;
                }
                self.action = None;
                if let Some((began, _)) = self.spans.last() {
                    self.link_spans.push((*began, self.wall));
                }
                self.finish_step(false);
            }
        }
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            let name = self
                .steps
                .iter()
                .find_map(|step| {
                    if let Step::Mark(name) = step { Some(name.strip_prefix("pool ").unwrap_or(name)) } else { None }
                })
                .unwrap_or("story step");
            self.mismatches.push(format!("{name}: timed out after WITHIN"));
            self.deadline = None;
            if let Some(child) = &mut self.child {
                child.drive(&mut self.submit, true);
            } else {
                if super::printed_link(&self.link).is_none() {
                    // No dependent call can be formed without the initial printed link.
                    self.steps.clear();
                }
                self.action = None;
                self.finish_step(false);
            }
        }
    }
    fn advance(&mut self, now: Time) {
        // A box that exits before printing its link must be diagnosed by
        // its referee, rather than panicking while constructing sign-in stdin.
        if matches!(self.steps.front(), Some(Step::Run(Call { stdin: Stdin::Link, .. })))
            && super::printed_link(&self.link).is_none()
        {
            return;
        }
        if self.cancelling
            || self.child.is_some()
            || self.member.is_some()
            || self.awaiting_started
            || matches!(self.action, Some(Action::Stderr(_) | Action::NextLink))
            || self.retry.is_some_and(|retry| retry > now)
        {
            return;
        }
        if let Some(Action::Until(call, _)) = &self.action {
            self.start(call.clone(), false);
            self.retry = None;
            return;
        }
        if let Some(within) = self.within
            && self.began.is_none()
            && !self.steps.is_empty()
            && !matches!(self.steps.front(), Some(Step::Mark(_)))
        {
            self.began = Some(self.wall);
            self.deadline = Some(now.saturating_add(within));
        }
        match self.steps.pop_front() {
            Some(Step::Mark(name)) => {
                if let Some(began) = self.phase.replace(now) {
                    eprintln!("{name}: {} ms", now.saturating_since(began).as_nanos() / 1_000_000);
                }
                self.advance(now);
            }
            Some(Step::Stderr(bytes)) => {
                assert!(matches!(self.box_stderr, BoxStderr::Captured), "Step::Stderr requires captured box stderr");
                assert!(!bytes.is_empty(), "Step::Stderr requires nonempty bytes");
                self.action = Some(Action::Stderr(bytes));
            }
            Some(Step::NextLink) => {
                self.action = Some(Action::NextLink);
                self.bound(now);
                if self.action.is_none() {
                    self.advance(now);
                }
            }
            Some(Step::Member { scratch, directory }) => {
                let root = skein_shell::open_root(&scratch).expect("member fixture root");
                self.member = Some(member::Member::new(root, &scratch, &scratch.join("client"), directory));
            }
            Some(Step::Open { directory, until }) => {
                self.action = Some(Action::Run);
                let args = super::terminal::open_arguments(Path::new(directory), self.link());
                self.spawn(env!("CARGO_BIN_EXE_temper").as_bytes().into(), args, Vec::new(), false);
                self.child.as_mut().expect("open child").typing = until.map_or(Input::Hold, Input::After);
            }
            Some(Step::Run(call)) => {
                self.action = Some(Action::Run);
                self.start(call, false);
            }
            Some(Step::Then(next)) => {
                let call = next(&self.ends);
                self.action = Some(Action::Run);
                self.start(call, false);
            }
            Some(Step::Start(call)) => self.start(call, true),
            Some(Step::Join) => self.awaiting_started = self.started.is_some(),
            Some(Step::Until(call, bytes)) => {
                self.action = Some(Action::Until(call.clone(), ReadCondition::Contains(bytes)));
                self.start(call, false);
            }
            Some(Step::UntilMatching(call, condition)) => {
                self.action = Some(Action::Until(call.clone(), condition));
                self.start(call, false);
            }
            Some(Step::Cut) => {
                let (child, _) = self.started.as_mut().expect("call still running at cut");
                child.signals += 1;
                self.awaiting_started = true;
                self.submit.push(Submit {
                    op: child.token(CUT),
                    kind: Op::Signal {
                        pidfd: child.pidfd.expect("started call pidfd"),
                        signal: Signal::Terminate,
                        to: Target::Child,
                    },
                });
            }
            Some(step @ (Step::Pause | Step::Resume)) => {
                self.action = Some(Action::Signal);
                let signal: &[u8] = if matches!(step, Step::Pause) { b"-STOP" } else { b"-CONT" };
                self.spawn(
                    b"/usr/bin/kill".as_slice().into(),
                    vec![signal.into(), self.forgejo.expect("owned Forgejo pid").to_string().into_bytes().into()],
                    Vec::new(),
                    false,
                );
            }
            None => {}
        }
    }
}
impl Host for Calls {
    fn iterate(&mut self, now: Time, wall: Wall) {
        self.wall = wall;
        while let Some(complete) = self.complete.pop() {
            if complete.op.raw() >= MEMBER {
                self.member.as_mut().expect("member fixture owns completion").complete(complete);
            } else if complete.op == LINK {
                self.reading = false;
                let Op::PipeRead { buf, .. } = complete.kind else { panic!("serve stdout") };
                let Done::Count(count) = complete.result.expect("serve read") else { panic!("read count") };
                self.link.extend_from_slice(&buf[..usize::try_from(count).expect("bounded read")]);
                assert!(self.link.len() <= OUTPUT, "bounded serve output");
                if count == 0 {
                    let fd = self.output.take().expect("serve stdout");
                    self.close(fd);
                }
            } else if complete.op.raw() >= OWN_CLOSE {
                complete.result.expect("fixture descriptor closed");
                self.closing -= 1;
            } else if complete.op.raw() >= STARTED {
                self.started.as_mut().expect("started call").0.completed(complete, &mut self.submit, self.cancelling);
            } else {
                self.child.as_mut().expect("running ctl").completed(complete, &mut self.submit, self.cancelling);
            }
        }
        if let Some(member) = &mut self.member {
            member.iterate(now, wall);
            while self.submit.room() > 0 {
                let Some(submit) = member.submission() else { break };
                self.submit.push(submit);
            }
            if member.is_empty() {
                self.member = None;
            }
        }
        self.finish(now);
        self.bound(now);
        if self.closing == 0 && (self.output.is_none() || super::printed_link(&self.link).is_some()) {
            self.advance(now);
        }
        if let Some(child) = &mut self.child {
            child.drive(&mut self.submit, self.cancelling);
        }
        if let Some((child, _)) = &mut self.started {
            child.drive(&mut self.submit, self.cancelling);
        }
        if self.done()
            && self.admission == Admission::Batch
            && let Some(root) = self.root.take()
        {
            self.close(root);
        }
        if !self.reading
            && let Some(fd) = self.output
        {
            self.reading = true;
            self.submit.push(Submit { op: LINK, kind: Op::PipeRead { fd, buf: vec![0; 1024].into_boxed_slice() } });
        }
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        &mut self.complete
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        &mut self.submit
    }
    fn work_pending(&self, now: Time) -> bool {
        !self.complete.is_empty()
            || !self.submit.is_empty()
            || self.retry.is_some_and(|retry| retry <= now)
            || self.member.as_ref().is_some_and(|member| member.work_pending(now))
    }
    fn next_deadline(&self) -> Option<Time> {
        self.retry
            .into_iter()
            .chain(self.deadline)
            .chain(self.member.as_ref().and_then(member::Member::next_deadline))
            .min()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        self.retry
            .into_iter()
            .chain(self.deadline)
            .chain(self.member.as_ref().and_then(member::Member::next_deadline))
            .min()
    }
    fn is_empty(&self) -> bool {
        self.done() && self.root.is_none() && self.output.is_none() && self.closing == 0
    }
    fn worst_case(&self) -> u64 {
        2_097_152 + self.member.as_ref().map_or(0, member::Member::worst_case)
    }
    fn operations(&self) -> u32 {
        // The provisioning client uses the same shipped native transport as ctl.
        let bounds = temper_client_shell_cli::limits::native();
        32 + skein_io::operations(&bounds.io).expect("native operations") + 2
    }
}

struct Judge {
    stopped: bool,
    settled: bool,
    deadline: Time,
    scratch: PathBuf,
    token_names: Vec<String>,
}
impl Referee<Process> for Judge {
    fn act(&mut self, _now: Time, procs: &mut [Process]) {
        use std::os::unix::fs::PermissionsExt;
        if !self.token_names.is_empty()
            && let Some(Process::Calls(calls)) = procs.get_mut(1)
        {
            while calls.tokens.len() < calls.ends.len() {
                calls.tokens.push(
                    self.token_names
                        .iter()
                        .map(|name| {
                            std::fs::symlink_metadata(self.scratch.join(name).join("token"))
                                .ok()
                                .map(|metadata| metadata.permissions().mode() & 0o777)
                        })
                        .collect(),
                );
            }
        }
        if procs.len() == 2 && !self.stopped {
            let Process::Calls(calls) = &procs[1] else { panic!("calls host") };
            if calls.done() {
                let Process::Binary(serve) = &mut procs[0] else { panic!("serve binary") };
                serve.signal(Signal::Terminate);
                self.stopped = true;
            }
        }
    }
    fn observe(&mut self, now: Time, procs: &[Process]) {
        self.settled = procs.iter().all(Host::is_empty);
        if let Process::Binary(serve) = &procs[0] {
            assert!(
                self.stopped || serve.exit_status().is_none(),
                "serve ended early: {}",
                String::from_utf8_lossy(serve.stderr())
            );
        }
        assert!(
            now < self.deadline || self.settled,
            "ctl story did not settle: {}",
            match &procs[1] {
                Process::Calls(calls) => format!(
                    "calls={} member={} child={:?}",
                    calls.ends.len(),
                    calls.member.as_ref().map_or_else(|| "none".into(), member::Member::status),
                    calls.child.as_ref().map(|child| (
                        child.ended.exit,
                        child.closing,
                        child.signals,
                        child.reading,
                        child.ended.stdout.len(),
                        child.ended.stderr.len()
                    ))
                ),
                _ => "other host".into(),
            }
        );
    }
    fn next_deadline(&self) -> Option<Time> {
        (!self.settled).then_some(self.deadline)
    }
    fn overdue(&self, now: Time) -> Option<String> {
        (!self.settled && now >= self.deadline).then(|| "ctl story deadline".into())
    }
    fn passed(&self) -> bool {
        self.settled
    }
}
#[must_use]
pub fn run(story: Story<'_>) -> Seen {
    let clock = Clock::new();
    let mut serve = super::serve(story.scratch);
    let streams = serve.take_streams().expect("serve pipes");
    let mut fds = streams.descriptors();
    let root = skein_shell::open_root(story.scratch).expect("scratch root");
    fds.push(root);
    let within = if story.measured {
        // Each wait keeps WITHIN, with one more WITHIN to drain after the last timeout.
        let waits = story.steps.iter().filter(|step| !matches!(step, Step::Mark(_))).count();
        Duration::from_nanos(WITHIN.as_nanos() * u64::try_from(waits + 1).expect("story waits"))
    } else {
        WITHIN
    };
    let mut world = real::World::new(Judge {
        stopped: false,
        settled: false,
        deadline: clock.now().now.saturating_add(within),
        scratch: story.scratch.into(),
        token_names: story.tokens,
    });
    world.spawn_with_fds(serve.descriptors(), || Process::Binary(Box::new(serve)));
    world.spawn_with_fds(fds, || {
        let mut calls = Calls::new(root, story.steps, Some(&streams), story.forgejo);
        calls.within = story.measured.then_some(WITHIN);
        if story.measured {
            let began = clock.now();
            calls.began = Some(began.wall);
            calls.deadline = Some(began.now.saturating_add(WITHIN));
        }
        Process::Calls(Box::new(calls))
    });
    let mut outcome = world.run(&clock, within);
    let Process::Binary(serve) = &outcome.procs[0] else { panic!("serve binary") };
    let serve = Ended { exit: serve.exit_status(), stdout: Vec::new(), stderr: serve.stderr().to_vec() };
    assert_eq!(serve.exit, Some(Exit::Code(0)), "serve drained: {}", String::from_utf8_lossy(&serve.stderr));
    let Process::Calls(calls) = &mut outcome.procs[1] else { panic!("calls host") };
    Seen {
        calls: std::mem::take(&mut calls.ends),
        serve,
        mismatches: std::mem::take(&mut calls.mismatches),
        spans: std::mem::take(&mut calls.spans),
        link_spans: std::mem::take(&mut calls.link_spans),
        tokens: std::mem::take(&mut calls.tokens),
    }
}
#[must_use]
pub fn alone(scratch: &Path, call: Call) -> Ended {
    let clock = Clock::new();
    let root = skein_shell::open_root(scratch).expect("scratch root");
    let mut world = real::World::new(Judge {
        stopped: false,
        settled: false,
        deadline: clock.now().now.saturating_add(WITHIN),
        scratch: scratch.into(),
        token_names: Vec::new(),
    });
    world.spawn_with_fds(vec![root], || Process::Calls(Box::new(Calls::new(root, vec![Step::Run(call)], None, None))));
    let mut outcome = world.run(&clock, WITHIN);
    let Process::Calls(calls) = &mut outcome.procs[0] else { panic!("calls host") };
    calls.ends.pop().expect("one ctl end")
}
#[must_use]
#[track_caller]
pub fn json(ended: &Ended) -> serde_json::Value {
    serde_json::from_slice(&ended.stdout).unwrap_or_else(|error| {
        panic!("ctl JSON: {error}; exit {:?}; stderr: {}", ended.exit, String::from_utf8_lossy(&ended.stderr))
    })
}

/// Use the revision actually read by the owner, including on a stale retry.
#[must_use]
pub fn settings_change(inventory: &Ended, file: &Path, key: &str) -> Call {
    settings_call("change-settings", inventory, file, key)
}
#[must_use]
pub fn settings_proposal(inventory: &Ended, file: &Path, key: &str) -> Call {
    settings_call("propose-settings", inventory, file, key)
}
fn settings_call(verb: &str, inventory: &Ended, file: &Path, key: &str) -> Call {
    let revision = json(inventory)["newest"].as_u64().expect("newest settings revision");
    Call::new(&[verb, file.to_str().expect("fixture FILE path"), "--against", &revision.to_string(), "--key", key])
}

/// Edit the whole charter value returned by ctl and wrap it in FILE's items.
pub fn charter_change_file(file: &Path, setting: &Ended, instructions: &str) {
    let document = json(setting);
    assert_eq!(document["setting"]["kind"], "charter");
    let mut value = document["value"].clone();
    value["instructions"] = instructions.into();
    let file_value = serde_json::json!({"items":[{"setting":document["setting"],"set":value}]});
    std::fs::write(file, serde_json::to_vec(&file_value).expect("settings FILE JSON")).expect("settings FILE");
}

#[cfg(test)]
mod read_condition_tests {
    use super::*;

    fn completed(stdout: &[u8]) -> Child {
        let mut child = Child::new(Vec::new(), 0);
        child.ended.exit = Some(Exit::Code(0));
        child.ended.stdout = stdout.to_vec();
        child
    }
    #[test]
    fn process_until_retains_matching_bytes_and_their_completion_wall() {
        let condition = ReadCondition::LimitedTask { task: 2, account: 1, source: 0 };
        let mut calls = Calls::new(Fd::new(0), Vec::new(), None, None);
        calls.action = Some(Action::Until(Call::new(&["task", "2", "--json"]), condition));
        calls.child = Some(completed(br#"{"chip":{"task":2,"phase":"due"},"run_wait":null}"#));
        calls.wall = Wall::from_nanos(10);
        calls.finish(Time::from_nanos(1));
        assert!(calls.ends.is_empty());
        assert!(calls.observed_at().is_empty());
        let stdout = b"{\n \"chip\": {\"task\":2,\"phase\":\"due\"},\n \"run_wait\":{\"account\":{\"account\":1,\"source\":0,\"why\":{\"limited\":{\"until\":30}}}}\n}\n";
        calls.child = Some(completed(stdout));
        calls.wall = Wall::from_nanos(20);
        calls.finish(Time::from_nanos(2));
        assert_eq!(calls.ends.len(), 1);
        assert_eq!(calls.ends[0].stdout, stdout);
        assert_eq!(calls.observed_at(), &[Some(Wall::from_nanos(20))]);
        calls.action = Some(Action::Run);
        calls.child = Some(completed(b"{\"accounts\":{}}"));
        calls.wall = Wall::from_nanos(40);
        calls.finish(Time::from_nanos(3));
        assert_eq!(calls.observed_at(), &[Some(Wall::from_nanos(20)), Some(Wall::from_nanos(40))]);
    }
}

/// Use the typed party proposal identity actually returned by the engine.
#[must_use]
pub fn proposal_key(ended: &Ended) -> String {
    let answer = json(ended);
    let proposal = &answer["outcome"]["done"]["proposed"]["proposal"];
    let party = proposal["proposer"]["party"].as_u64().expect("party proposal");
    let number = proposal["number"].as_u64().expect("proposal number");
    format!("p{party}/{number}")
}
