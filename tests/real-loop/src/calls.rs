//! Hosted commands observed through their pipes, exit and a service watch cue.
use crate::fixtures;
use skein_io::kernel::{Complete, Done, Exit, Fd, Op, OpenHow, Pipe, Signal, Spawn, Submit, Way};
use skein_lib::{Queue, Time, Token, Wall};
use skein_shell::Host;
use std::{cell::Cell, collections::BTreeMap, os::unix::ffi::OsStrExt, path::PathBuf};
#[derive(Clone, Copy)]
pub struct Dir(pub u8);
pub const D1: Dir = Dir(1);
pub const D2: Dir = Dir(2);
pub enum Arg {
    Lit(&'static str),
    Task(usize),
    Delegate(usize),
    Digest(usize),
    ActivityBefore(usize),
    Number(u64),
    File(&'static str),
}
impl From<&'static str> for Arg {
    fn from(word: &'static str) -> Self {
        Self::Lit(word)
    }
}
pub struct Argv(pub Vec<Arg>);
#[macro_export]
macro_rules! argv { ($($word:expr),* $(,)?) => { $crate::calls::Argv(vec![$($crate::calls::Arg::from($word)),*]) }; }
pub enum Step {
    /// Finish runtime fixture provisioning before recording the story calls.
    FixtureReady,
    Advance,
    StopWorker {
        name: &'static str,
    },
    StartWorker {
        name: &'static str,
    },
    AddWorker {
        name: &'static str,
    },
    Probe {
        at: Dir,
    },
    Release {
        at: Dir,
    },
    Contended {
        calls: &'static [usize],
    },
    SignIn {
        at: Dir,
    },
    SameParty {
        at: Dir,
    },
    Held {
        name: &'static [u8],
    },
    Kill {
        call: usize,
    },
    Call {
        at: Dir,
        argv: Argv,
    },
    Start {
        at: Dir,
        argv: Argv,
    },
    Join {
        call: usize,
    },
    Watching {
        call: usize,
    },
    Until {
        at: Dir,
        argv: Argv,
        holds: &'static [u8],
        early: &'static [u8],
    },
}
/// Start a configured worker and wait for its link before the next act.
#[must_use]
pub fn add_worker(name: &'static str) -> Step {
    Step::AddWorker { name }
}
/// Terminate one worker and wait until its service and ring I/O settle.
#[must_use]
pub fn stop_worker(name: &'static str) -> Step {
    Step::StopWorker { name }
}
/// Restart a settled worker on its own directories and wait for its new link.
#[must_use]
pub fn start_worker(name: &'static str) -> Step {
    Step::StartWorker { name }
}
#[must_use]
pub fn advance() -> Step {
    Step::Advance
}
#[must_use]
pub fn probe(at: Dir) -> Step {
    Step::Probe { at }
}
#[must_use]
pub fn release(at: Dir) -> Step {
    Step::Release { at }
}
#[must_use]
pub fn contended(calls: &'static [usize]) -> Step {
    Step::Contended { calls }
}
#[must_use]
pub fn sign_in(at: Dir) -> Step {
    Step::SignIn { at }
}
#[must_use]
pub fn same_party(at: Dir) -> Step {
    Step::SameParty { at }
}
#[must_use]
pub fn held(name: &'static [u8]) -> Step {
    Step::Held { name }
}
#[must_use]
pub fn kill(call: usize) -> Step {
    Step::Kill { call }
}
#[must_use]
pub fn listed(stdout: &[u8]) -> Vec<u64> {
    std::str::from_utf8(stdout)
        .expect("tasks page")
        .lines()
        .filter_map(|line| line.split_once(" · ").and_then(|(number, _)| number.parse().ok()))
        .collect()
}
#[must_use]
pub fn call(at: Dir, argv: Argv) -> Step {
    Step::Call { at, argv }
}
#[must_use]
pub fn until(at: Dir, argv: Argv, holds: &'static [u8], early: &'static [u8]) -> Step {
    Step::Until { at, argv, holds, early }
}
#[must_use]
pub fn start(at: Dir, argv: Argv) -> Step {
    Step::Start { at, argv }
}
#[must_use]
pub fn join(call: usize) -> Step {
    Step::Join { call }
}
#[must_use]
pub fn watching(call: usize) -> Step {
    Step::Watching { call }
}
#[must_use]
pub fn file(name: &'static str) -> Arg {
    Arg::File(name)
}
#[must_use]
pub fn number(task: u64) -> Arg {
    Arg::Number(task)
}
#[must_use]
pub fn task(call: usize) -> Arg {
    Arg::Task(call)
}
#[must_use]
pub fn delegate(call: usize) -> Arg {
    Arg::Delegate(call)
}
#[must_use]
pub fn digest(call: usize) -> Arg {
    Arg::Digest(call)
}
/// The previous page cursor supplied by a task's activity read.
#[must_use]
pub fn activity_before(call: usize) -> Arg {
    Arg::ActivityBefore(call)
}
fn delegate_number(stdout: &[u8]) -> u64 {
    let value: serde_json::Value = serde_json::from_slice(stdout).expect("task tree JSON");
    let tree = &value["tree"];
    let root = tree["root"]["task"].as_u64().expect("tree root");
    let mut children = tree["rows"].as_array().expect("tree rows").iter().filter(|node| node["parent"] == root);
    let child = children.next().expect("one child task")["chip"]["task"].as_u64().expect("child task number");
    assert!(children.next().is_none(), "one child task");
    child
}
#[must_use]
pub fn occurrences(bytes: &[u8], needle: &[u8]) -> usize {
    bytes.windows(needle.len()).filter(|part| *part == needle).count()
}
pub(crate) fn task_number(stdout: &[u8]) -> u64 {
    std::str::from_utf8(stdout)
        .expect("call stdout")
        .split("done: task ")
        .nth(1)
        .expect("created task")
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .expect("task number")
        .parse()
        .expect("created task number")
}
#[derive(Debug)]
pub struct Called {
    pub locked: Option<Time>,
    pub released: Option<Time>,
    pub busy: u32,
    pub exit: Option<Exit>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub spawned: Time,
    pub ended: Time,
    pub tries: u32,
    pub early: u32,
}
// A complete small rendered turn fits; a pipe chunk alone could miss a split reply.
const OUTPUT: usize = 65_536;
const ROOM: u32 = 16 * fixtures::CALLS_AT_ONCE; // Spawn, pipe closes, reads and wait for each call.
#[derive(Default)]
struct Running {
    pending: usize,
    started: bool,
    pidfd: Option<Fd>,
    stdout: Option<Fd>,
    stderr: Option<Fd>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Recording {
    Fixture,
    Story,
}
pub(crate) struct Caller {
    recording: Recording,
    pub(crate) released: bool,
    pub(crate) advanced: Cell<bool>,
    pub(crate) added: Cell<bool>,
    pub(crate) worker_ready: Cell<bool>,
    pub(crate) held: Cell<bool>,
    pub(crate) links: Cell<u32>,
    retry_link: Option<u32>,
    pub(crate) calls: Vec<Called>,
    steps: Vec<Step>,
    next: usize,
    scratch: PathBuf,
    root: Option<Fd>,
    current: Option<usize>,
    probe: Option<(Dir, Fd)>,
    probing: bool,
    running: Vec<Option<Running>>,
    owners: BTreeMap<Token, usize>,
    pub(crate) watching: Option<usize>,
    pub(crate) watched: bool,
    completions: Queue<Complete>,
    submissions: Queue<Submit>,
    serial: u64,
    memory: u64,
    command: Option<Spawn>,
    input: Box<[u8]>,
}
impl Caller {
    pub(crate) fn held_names(&self) -> impl Iterator<Item = &[u8]> {
        self.steps.iter().filter_map(|step| match step {
            Step::Held { name } => Some(*name),
            Step::FixtureReady
            | Step::StopWorker { .. }
            | Step::StartWorker { .. }
            | Step::AddWorker { .. }
            | Step::Advance
            | Step::SignIn { .. }
            | Step::SameParty { .. }
            | Step::Kill { .. }
            | Step::Call { .. }
            | Step::Start { .. }
            | Step::Join { .. }
            | Step::Watching { .. }
            | Step::Until { .. }
            | Step::Probe { .. }
            | Step::Release { .. }
            | Step::Contended { .. } => None,
        })
    }
    pub(crate) fn adding_worker(&self) -> Option<&'static str> {
        match self.steps.get(self.next) {
            Some(Step::AddWorker { name }) => Some(*name),
            _ => None,
        }
    }
    pub(crate) fn stopping_worker(&self) -> Option<&'static str> {
        if let Some(Step::StopWorker { name }) = self.steps.get(self.next) { Some(*name) } else { None }
    }
    pub(crate) fn starting_worker(&self) -> Option<&'static str> {
        if let Some(Step::StartWorker { name }) = self.steps.get(self.next) { Some(*name) } else { None }
    }
    pub(crate) fn advancing(&self) -> bool {
        matches!(self.steps.get(self.next), Some(Step::Advance))
    }
    pub(crate) fn held_index(&self) -> usize {
        self.steps[..self.next].iter().filter(|step| matches!(step, Step::Held { .. })).count()
    }
    pub(crate) fn requested_kills(&self) -> usize {
        self.steps.iter().filter(|step| matches!(step, Step::Kill { .. })).count()
    }
    pub(crate) fn provisioned(mut steps: Vec<Step>, scratch: PathBuf, root: Fd) -> Self {
        let Some((position, at)) = steps.iter().enumerate().find_map(|(index, step)| {
            if let Step::SignIn { at } | Step::SameParty { at } = step { Some((index, *at)) } else { None }
        }) else {
            return Self::new(steps, scratch, root);
        };
        // A real keyed request follows the story's first sign-in. Its setup result
        // is consumed here; the story keeps its original named call positions.
        steps.insert(
            position + 1,
            call(at, crate::argv!["add-key", "1", file("fixture.key"), "--deployment", "--key", "fixture-llm"]),
        );
        steps.insert(position + 2, Step::FixtureReady);
        let mut caller = Self::new(steps, scratch, root);
        caller.recording = Recording::Fixture;
        caller
    }
    pub(crate) fn new(steps: Vec<Step>, scratch: PathBuf, root: Fd) -> Self {
        let memory = u64::try_from((steps.len() + 1) * (OUTPUT * 4 + 4096)).expect("caller memory")
            + Queue::<Complete>::worst_case(ROOM).expect("completions")
            + Queue::<Submit>::worst_case(ROOM).expect("submissions");
        let count = steps.len();
        Self {
            recording: Recording::Story,
            released: false,
            advanced: Cell::new(false),
            added: Cell::new(false),
            worker_ready: Cell::new(false),
            held: Cell::new(false),
            links: Cell::new(0),
            retry_link: None,
            calls: Vec::with_capacity(count),
            steps,
            next: 0,
            scratch,
            root: Some(root),
            current: None,
            probe: None,
            probing: false,
            running: Vec::with_capacity(count),
            owners: BTreeMap::new(),
            watching: None,
            watched: false,
            completions: Queue::with_capacity(ROOM),
            submissions: Queue::with_capacity(ROOM),
            serial: 0,
            memory,
            command: None,
            input: Box::new([]),
        }
    }
    pub(crate) fn command(mut spawn: Spawn, input: &[u8], root: Fd) -> Self {
        assert!(input.len() <= OUTPUT, "bounded command input");
        spawn.root = root;
        let mut caller = Self::new(Vec::new(), PathBuf::new(), root);
        caller.command = Some(spawn);
        caller.input = input.into();
        caller.released = true;
        caller
    }
    fn submit(&mut self, call: usize, kind: Op) {
        self.serial += 1;
        if call != usize::MAX {
            self.running[call].as_mut().expect("running command").pending += 1;
        }
        self.owners.insert(Token::new(self.serial), call);
        self.submissions.push(Submit { op: Token::new(self.serial), kind });
    }
    fn read(&mut self, call: usize, fd: Fd) {
        self.submit(call, Op::PipeRead { fd, buf: vec![0; 4096].into_boxed_slice() });
    }
    fn words(&self) -> Vec<Box<[u8]>> {
        let (at, argv) = match &self.steps[self.next] {
            Step::SignIn { at } | Step::SameParty { at } => {
                return vec![
                    fixtures::directory(&self.scratch, *at).as_os_str().as_bytes().into(),
                    b"sign-in".as_slice().into(),
                    self.scratch.join("link").as_os_str().as_bytes().into(),
                ];
            }
            Step::Call { at, argv } | Step::Start { at, argv } | Step::Until { at, argv, .. } => (at, argv),
            Step::FixtureReady
            | Step::StopWorker { .. }
            | Step::StartWorker { .. }
            | Step::AddWorker { .. }
            | Step::Advance
            | Step::Join { .. }
            | Step::Watching { .. }
            | Step::Held { .. }
            | Step::Kill { .. }
            | Step::Probe { .. }
            | Step::Release { .. }
            | Step::Contended { .. } => panic!("command step"),
        };
        let mut words = vec![fixtures::directory(&self.scratch, *at).as_os_str().as_bytes().into()];
        for arg in &argv.0 {
            words.push(match arg {
                Arg::Lit(word) => word.as_bytes().into(),
                Arg::Task(call) => task_number(&self.calls[*call].stdout).to_string().into_bytes().into(),
                Arg::Delegate(call) => delegate_number(&self.calls[*call].stdout).to_string().into_bytes().into(),
                Arg::Digest(call) => {
                    let value: serde_json::Value =
                        serde_json::from_slice(&self.calls[*call].stdout).expect("task JSON");
                    value["task"]["digest"].as_str().expect("task digest").as_bytes().into()
                }
                Arg::ActivityBefore(call) => {
                    let value: serde_json::Value =
                        serde_json::from_slice(&self.calls[*call].stdout).expect("activity JSON");
                    value
                        .pointer("/activity/before/value")
                        .or_else(|| value.pointer("/before/value"))
                        .and_then(serde_json::Value::as_str)
                        .expect("previous activity page")
                        .as_bytes()
                        .into()
                }
                Arg::Number(number) => number.to_string().into_bytes().into(),
                Arg::File(name) => self.scratch.join(name).as_os_str().as_bytes().into(),
            });
        }
        words
    }
    fn spawn(&mut self, now: Time, index: usize) {
        let args = self.words().into_boxed_slice();
        if matches!(self.steps[self.next], Step::SameParty { .. }) && self.current.is_none() {
            self.retry_link = Some(self.links.get());
        }
        self.launch(
            now,
            index,
            Spawn {
                program: fixtures::CTL.into(),
                args,
                env: vec![
                    format!("CALL={index}").into_bytes().into_boxed_slice(),
                    format!("FIXTURE={}", self.setup()).into_bytes().into_boxed_slice(),
                ]
                .into_boxed_slice(),
                root: self.root.expect("launch root"),
                dir: Box::new([]),
                pipes: [(0, Way::In), (1, Way::Out), (2, Way::Out)]
                    .map(|(child, way)| Pipe { child, way, parent: None })
                    .into(),
            },
        );
    }
    fn launch(&mut self, now: Time, index: usize, spawn: Spawn) {
        if self.calls.len() == index {
            self.calls.push(Called {
                locked: None,
                released: None,
                busy: 0,
                exit: None,
                stdout: Vec::new(),
                stderr: Vec::new(),
                spawned: now,
                ended: now,
                tries: 0,
                early: 0,
            });
            self.running.push(None);
        }
        let call = &mut self.calls[index];
        call.stdout.clear();
        call.stderr.clear();
        call.exit = None;
        call.tries += 1;
        self.running[index] = Some(Running::default());
        if matches!(self.steps.get(self.next), Some(Step::Start { .. }))
            && matches!(self.steps.get(self.next + 1), Some(Step::Watching { call }) if *call == index)
        {
            // Arm before the spawned host can connect; release still waits for its task delivery.
            self.watching = Some(index);
        }
        self.submit(index, Op::Spawn { spawn: Box::new(spawn) });
    }
    fn complete(&mut self, complete: Complete, now: Time) {
        let index = self.owners.remove(&complete.op).expect("operation owner");
        if index == usize::MAX {
            match (complete.kind, complete.result.expect("probe operation")) {
                (Op::Open { .. }, Done::Fd(fd)) => {
                    let Step::Probe { at } = self.steps[self.next] else { panic!("probe step") };
                    self.probe = Some((at, fd));
                    self.submit(usize::MAX, Op::Lock { fd });
                    return;
                }
                (Op::Lock { .. } | Op::Close { .. }, Done::Nothing) => self.probing = false,
                (kind, done) => panic!("probe completion {kind:?}: {done:?}"),
            }
            return;
        }
        self.running[index].as_mut().expect("running command").pending -= 1;
        match (complete.kind, complete.result.expect("caller operation")) {
            (Op::Spawn { spawn }, Done::Spawned { pidfd }) => {
                let running = self.running[index].as_mut().expect("running command");
                running.pidfd = Some(pidfd);
                running.started = true;
                for pipe in spawn.pipes {
                    let fd = pipe.parent.expect("parent pipe");
                    match pipe.child {
                        0 => {
                            let bytes = std::mem::take(&mut self.input);
                            self.write(index, fd, bytes, 0);
                        }
                        1 => {
                            self.running[index].as_mut().expect("running command").stdout = Some(fd);
                            self.read(index, fd);
                        }
                        2 => {
                            self.running[index].as_mut().expect("running command").stderr = Some(fd);
                            self.read(index, fd);
                        }
                        _ => panic!("standard pipes"),
                    }
                }
            }
            (Op::PipeWrite { fd, bytes, from }, Done::Count(count)) => {
                self.write(index, fd, bytes, from + count);
            }
            (Op::PipeRead { fd, buf }, Done::Count(count)) => {
                if count == 0 {
                    if self.running[index].as_mut().expect("running command").stdout == Some(fd) {
                        self.running[index].as_mut().expect("running command").stdout = None;
                    } else {
                        assert_eq!(self.running[index].as_mut().expect("running command").stderr, Some(fd));
                        self.running[index].as_mut().expect("running command").stderr = None;
                    }
                    self.submit(index, Op::Close { fd });
                } else {
                    let stdout = self.running[index].as_ref().expect("running command").stdout == Some(fd);
                    let call = &mut self.calls[index];
                    let bytes = if stdout { &mut call.stdout } else { &mut call.stderr };
                    bytes.extend_from_slice(&buf[..usize::try_from(count).expect("read count")]);
                    assert!(bytes.len() <= OUTPUT, "bounded command output");
                    self.read(index, fd);
                }
            }
            (Op::Wait { pidfd, reap: false }, Done::Exit(_)) => {
                self.submit(index, Op::Wait { pidfd, reap: true });
            }
            (Op::Wait { pidfd, reap: true }, Done::Exit(exit)) => {
                self.calls[index].exit = Some(exit);
                self.calls[index].ended = now;
                self.submit(index, Op::Close { fd: pidfd });
            }
            (Op::Close { .. } | Op::Signal { .. }, Done::Nothing) => {}
            (kind, done) => panic!("caller completion {kind:?}: {done:?}"),
        }
    }
    fn write(&mut self, index: usize, fd: Fd, bytes: Box<[u8]>, from: u32) {
        if usize::try_from(from).expect("input offset") == bytes.len() {
            self.submit(index, Op::Close { fd });
        } else {
            self.submit(index, Op::PipeWrite { fd, bytes, from });
        }
    }
    fn settle(&mut self) {
        for index in 0..self.running.len() {
            let Some(running) = &mut self.running[index] else { continue };
            if running.pending != 0 {
                continue;
            }
            if let Some(pidfd) = running.pidfd.take() {
                self.submit(index, Op::Wait { pidfd, reap: false });
            } else {
                assert!(self.calls[index].exit.is_some(), "completed command exit");
                self.running[index] = None;
            }
        }
    }
    fn acquire_probe(&mut self, at: Dir) -> bool {
        if self.probing {
            return false;
        }
        if self.probe.is_none() {
            let path = fixtures::directory(&self.scratch, at);
            let path = path.strip_prefix(&self.scratch).expect("beneath scratch").as_os_str().as_bytes().into();
            self.probing = true;
            self.submit(usize::MAX, Op::Open { root: self.root.expect("launch root"), path, how: OpenHow::Directory });
            return false;
        }
        true
    }
    fn release_probe(&mut self, at: Dir) -> bool {
        if self.probing {
            return false;
        }
        if let Some((held, fd)) = self.probe.take() {
            assert_eq!(held.0, at.0, "release probed directory");
            self.probing = true;
            self.submit(usize::MAX, Op::Close { fd });
            return false;
        }
        true
    }
    fn finish(&mut self) {
        if self.running.iter().all(Option::is_none)
            && let Some(fd) = self.root.take()
        {
            self.submit(usize::MAX, Op::Close { fd });
        }
    }
    pub(crate) fn setup(&self) -> bool {
        self.recording == Recording::Fixture
    }
    fn fixture_ready(&mut self) {
        assert!(self.running.iter().all(Option::is_none), "fixture commands settled");
        let key = self.calls.pop().expect("runtime key call");
        assert_eq!(key.exit, Some(Exit::Code(0)), "runtime key provisioned: {}", String::from_utf8_lossy(&key.stderr));
        self.running.pop().expect("settled key call");
        self.recording = Recording::Story;
    }
    fn advance(&mut self, now: Time) {
        if matches!(self.steps.get(self.next), Some(Step::Held { .. })) && self.held.get() {
            // Issue the following kill in this pass, before the hosted caller's next turn.
            self.next += 1;
        }
        let Some(step) = self.steps.get(self.next) else {
            self.finish();
            return;
        };
        match step {
            Step::FixtureReady if self.running.iter().any(Option::is_some) => return,
            Step::FixtureReady => self.fixture_ready(),
            Step::AddWorker { .. } if !self.added.replace(false) => return,
            Step::StopWorker { .. } | Step::StartWorker { .. } if !self.worker_ready.replace(false) => return,
            Step::Advance if !self.advanced.get() => return,
            Step::StopWorker { .. } | Step::StartWorker { .. } | Step::AddWorker { .. } | Step::Advance => {}
            Step::Held { .. } => return,
            Step::Kill { call } => {
                let index = *call;
                if self.current.is_none() {
                    let pidfd = self.running[index].as_ref().expect("kill running call").pidfd.expect("spawned call");
                    self.submit(index, Op::Signal { pidfd, signal: Signal::Kill, to: skein_io::kernel::Target::Child });
                    self.current = Some(index);
                    return;
                }
                if self.running[index].is_some() {
                    return;
                }
                self.current = None;
            }
            Step::Probe { at } => {
                if !self.acquire_probe(*at) {
                    return;
                }
            }
            Step::Release { at } => {
                if !self.release_probe(*at) {
                    return;
                }
            }
            Step::Contended { calls } => {
                if calls.iter().any(|call| self.calls[*call].busy == 0) {
                    return;
                }
            }
            Step::Watching { call } => {
                assert!(self.running[*call].as_ref().is_some_and(|r| r.started), "started watch call");
                self.watching = Some(*call);
                if !self.watched {
                    return;
                }
                self.watching = None;
                self.watched = false;
            }
            Step::Join { call } => {
                if self.running[*call].is_some() {
                    return;
                }
            }
            Step::SignIn { .. }
            | Step::SameParty { .. }
            | Step::Call { .. }
            | Step::Start { .. }
            | Step::Until { .. } => {
                let Some(index) = self.current else {
                    assert!(self.running.iter().flatten().count() < fixtures::CALLS_AT_ONCE as usize);
                    let index = self.calls.len();
                    self.spawn(now, index);
                    self.current = Some(index);
                    return;
                };
                if let Step::Start { .. } = step {
                    if !self.running[index].as_ref().is_some_and(|r| r.started) {
                        return;
                    }
                } else if self.running[index].is_some() {
                    return;
                }
                let call = &mut self.calls[index];
                if matches!(step, Step::SameParty { .. }) && call.tries == 1 {
                    let link = *self.retry_link.get_or_insert(self.links.get());
                    if self.links.get() <= link {
                        return;
                    }
                    self.retry_link = None;
                    self.spawn(now, index);
                    return;
                }
                if let Step::Until { holds, early, .. } = step
                    && occurrences(&call.stdout, holds) == 0
                    && call.exit == Some(Exit::Code(0))
                {
                    call.early += u32::from(occurrences(&call.stdout, early) != 0);
                    self.spawn(now, index);
                    return;
                }
                self.current = None;
            }
        }
        self.next += 1;
    }
    fn can_advance(&self) -> bool {
        match self.steps.get(self.next) {
            Some(Step::FixtureReady) => self.running.iter().all(Option::is_none),
            Some(Step::StopWorker { .. } | Step::StartWorker { .. }) => self.worker_ready.get(),
            Some(Step::Advance) => self.advanced.get(),
            Some(Step::AddWorker { .. }) => self.added.get(),
            Some(Step::Held { .. }) => self.held.get(),
            Some(Step::SameParty { .. }) if self.retry_link.is_some() => {
                self.retry_link.is_some_and(|link| self.links.get() > link)
            }
            Some(Step::Probe { .. } | Step::Release { .. }) => !self.probing,
            Some(Step::Contended { calls }) => calls.iter().all(|call| self.calls[*call].busy != 0),
            Some(Step::Watching { .. }) => self.watched || self.watching.is_none(),
            Some(Step::Join { call }) => self.running[*call].is_none(),
            Some(Step::Start { .. }) => {
                self.current.is_none_or(|i| self.running[i].as_ref().is_some_and(|r| r.started))
            }
            Some(_) => self.current.is_none_or(|i| self.running[i].is_none()),
            None => self.root.is_some() && self.running.iter().all(Option::is_none),
        }
    }
}
impl Host for Caller {
    fn iterate(&mut self, now: Time, _wall: Wall) {
        while let Some(complete) = self.completions.pop() {
            self.complete(complete, now);
        }
        self.settle();
        if self.released {
            if let Some(spawn) = self.command.take() {
                self.launch(now, 0, spawn);
            } else {
                self.advance(now);
            }
        }
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        &mut self.completions
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        &mut self.submissions
    }
    fn work_pending(&self, _now: Time) -> bool {
        !self.completions.is_empty() || self.released && self.can_advance()
    }
    fn next_deadline(&self) -> Option<Time> {
        None
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        None
    }
    fn is_empty(&self) -> bool {
        self.released && self.root.is_none() && self.probe.is_none() && self.owners.is_empty()
    }
    fn worst_case(&self) -> u64 {
        self.memory
    }
    fn operations(&self) -> u32 {
        ROOM
    }
}

mod recorded;
pub(crate) use recorded::Recorded;
