//! Hosted commands observed through their pipes, exit and a service watch cue.
use crate::fixtures;
mod browse;
pub use browse::Browsed;
use browse::Browser;
const BROWSER: usize = usize::MAX - 1;
const SIGN_IN_HTTP: usize = usize::MAX - 2;
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
    InboxAfter(usize),
    Number(u64),
    GithubApp,
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
    Browse {
        call: usize,
    },
    /// Approve the terminal's last sign-in address at the fake issuer, keeping
    /// its Location unfollowed and secret in the world's memory.
    Authorize,
    /// GET the kept redirect on the real loopback listener; keep it for later acts.
    Return,
    /// A person at the hosted terminal.
    Terminal(crate::terminal::Act),
    Advance,
    /// Commit the person's named file on the fake forge's main. One `Advance` or
    /// `Push` per story: the forge moves once, and a second completes with nothing pushed.
    Push {
        path: &'static [u8],
        words: &'static [u8],
    },
    /// Release a fake LLM hold through its owner in the loop.
    ReleaseHold {
        name: &'static [u8],
    },
    /// Drain a named worker while retaining its root for a restart.
    StopWorker {
        name: &'static str,
    },
    /// Start a drained worker again on its retained directories.
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
        holds: ReadCondition,
        early: &'static [u8],
    },
}
/// A condition on one genuine ctl response, shared by both observation adapters.
#[derive(Clone, Copy, Debug)]
pub enum ReadCondition {
    Contains(&'static [u8]),
    LimitedTask {
        task: u64,
        account: u64,
        source: u64,
    },
    /// An open writing attempt on this task. Unmet documents resume a native
    /// subject watch through `watch_after`, rather than repeating a task read.
    WritingTask {
        task: u64,
    },
}
impl ReadCondition {
    #[must_use]
    pub fn matches(self, stdout: &[u8]) -> bool {
        match self {
            Self::Contains(bytes) => occurrences(stdout, bytes) != 0,
            Self::WritingTask { task } => {
                let Ok(value) = serde_json::from_slice::<serde_json::Value>(stdout) else { return false };
                let value = value.get("task").filter(|task| task.is_object()).unwrap_or(&value);
                value.pointer("/chip/task").and_then(serde_json::Value::as_u64) == Some(task)
                    && value.pointer("/chip/phase").and_then(serde_json::Value::as_str) == Some("running")
                    && value.pointer("/chip/details/owner/details/observation/work").and_then(serde_json::Value::as_str)
                        == Some("writing")
                    && value
                        .pointer("/chip/details/owner/phase/active/running/attempt")
                        .and_then(serde_json::Value::as_u64)
                        .is_some_and(|attempt| {
                            value.pointer("/chip/details/owner/details/attempt").and_then(serde_json::Value::as_u64)
                                == Some(attempt)
                        })
                    && value.get("result").is_some_and(serde_json::Value::is_null)
            }
            Self::LimitedTask { task, account, source } => {
                let Ok(value) = serde_json::from_slice::<serde_json::Value>(stdout) else { return false };
                value.pointer("/chip/task").and_then(serde_json::Value::as_u64) == Some(task)
                    && value.pointer("/chip/phase").and_then(serde_json::Value::as_str) == Some("due")
                    && value.pointer("/run_wait/account/account").and_then(serde_json::Value::as_u64) == Some(account)
                    && value.pointer("/run_wait/account/source").and_then(serde_json::Value::as_u64) == Some(source)
                    && value
                        .pointer("/run_wait/account/why/limited/until")
                        .and_then(serde_json::Value::as_u64)
                        .is_some()
            }
        }
    }
    /// Continue from the exact subject version just observed. `wait --since`
    /// checks the current version on subscription, including changes between
    /// commands, then sleeps on the service watch until that version changes.
    #[must_use]
    pub fn watch_after(self, stdout: &[u8]) -> Option<Vec<Box<[u8]>>> {
        let Self::WritingTask { task } = self else { return None };
        let value: serde_json::Value = serde_json::from_slice(stdout).expect("watched task JSON");
        let value = value.get("task").filter(|task| task.is_object()).unwrap_or(&value);
        assert_eq!(value.pointer("/chip/task").and_then(serde_json::Value::as_u64), Some(task));
        let digest = value["digest"].as_str().expect("watched task digest");
        Some(
            ["wait", "task", &task.to_string(), "--since", digest, "--json"]
                .into_iter()
                .map(|word| word.as_bytes().into())
                .collect(),
        )
    }
}
/// Follow the installation or manifest link printed by a completed ctl call.
#[must_use]
pub fn browse(call: usize) -> Step {
    Step::Browse { call }
}
/// The id handed to the person by the fake's manifest conversion.
#[must_use]
pub fn github_app() -> Arg {
    Arg::GithubApp
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
/// Release the named fake LLM hold before proceeding.
#[must_use]
pub fn release_hold(name: &'static [u8]) -> Step {
    Step::ReleaseHold { name }
}
#[must_use]
pub fn push(path: &'static [u8], words: &'static [u8]) -> Step {
    Step::Push { path, words }
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
    until_matching(at, argv, ReadCondition::Contains(holds), early)
}
#[must_use]
pub fn until_matching(at: Dir, argv: Argv, holds: ReadCondition, early: &'static [u8]) -> Step {
    Step::Until { at, argv, holds, early }
}
/// Wait for the named task's own writing attempt, retaining its genuine JSON.
#[must_use]
pub fn writing_task(at: Dir, task: u64) -> Step {
    until_matching(
        at,
        crate::argv!["wait", "task", number(task), "--until", "phase:running", "--json"],
        ReadCondition::WritingTask { task },
        b"\"task\"",
    )
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
/// The needs continuation supplied by an earlier inbox page.
#[must_use]
pub fn inbox_after(call: usize) -> Arg {
    Arg::InboxAfter(call)
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
    /// Wall input when the completed ctl response and exit were observed, not the server sample.
    pub observed_at: Option<Wall>,
    pub tries: u32,
    pub early: u32,
}
// A complete small rendered turn fits; a pipe chunk alone could miss a split reply.
const OUTPUT: usize = 65_536;
// Spawn, pipe closes, reads and wait for each call, plus the terminal typist.
const ROOM: u32 = 16 * fixtures::CALLS_AT_ONCE + 8;
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
    pub(crate) github_app: Option<u64>,
    pub(crate) browsed: Vec<Browsed>,
    browser: Option<Browser>,
    visit: Option<crate::issuer::Visit>,
    kept: Option<skein_lib::Secret>,
    pub(crate) returned: Vec<crate::issuer::Returned>,
    pub(crate) released: bool,
    pub(crate) advanced: Cell<bool>,
    pub(crate) hold_released: Cell<bool>,
    pub(crate) added: Cell<bool>,
    pub(crate) worker_ready: Cell<bool>,
    pub(crate) held: Cell<bool>,
    pub(crate) links: Cell<u32>,
    retry_link: Option<u32>,
    pub(crate) calls: Vec<Called>,
    pub(crate) terminal: crate::terminal::Typist,
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
            Step::Browse { .. }
            | Step::Authorize
            | Step::Return
            | Step::FixtureReady
            | Step::ReleaseHold { .. }
            | Step::Terminal(_)
            | Step::StopWorker { .. }
            | Step::StartWorker { .. }
            | Step::AddWorker { .. }
            | Step::Advance
            | Step::Push { .. }
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
    pub(crate) fn releasing_hold(&self) -> Option<&'static [u8]> {
        if let Some(Step::ReleaseHold { name }) = self.steps.get(self.next) { Some(*name) } else { None }
    }
    pub(crate) fn advancing(&self) -> bool {
        matches!(self.steps.get(self.next), Some(Step::Advance))
    }
    pub(crate) fn pushing(&self) -> Option<(&'static [u8], &'static [u8])> {
        if let Some(Step::Push { path, words }) = self.steps.get(self.next) { Some((*path, *words)) } else { None }
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
        let browsers = steps.iter().filter(|step| matches!(step, Step::Browse { .. })).count();
        let visits = steps.iter().filter(|step| matches!(step, Step::Authorize | Step::Return)).count();
        let memory = u64::try_from((steps.len() + 1) * (OUTPUT * 4 + 4096)).expect("caller memory")
            + Browser::MEMORY * u64::try_from(browsers).expect("browser count")
            + crate::issuer::Visit::MEMORY * u64::try_from(visits).expect("sign-in visits")
            + crate::terminal::Typist::worst_case()
            + Queue::<Complete>::worst_case(ROOM).expect("completions")
            + Queue::<Submit>::worst_case(ROOM).expect("submissions");
        let count = steps.len();
        Self {
            recording: Recording::Story,
            github_app: None,
            browsed: Vec::with_capacity(browsers),
            browser: None,
            visit: None,
            kept: None,
            returned: Vec::with_capacity(visits),
            released: false,
            advanced: Cell::new(false),
            hold_released: Cell::new(false),
            added: Cell::new(false),
            worker_ready: Cell::new(false),
            held: Cell::new(false),
            links: Cell::new(0),
            retry_link: None,
            calls: Vec::with_capacity(count),
            terminal: crate::terminal::Typist::default(),
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
        if call < SIGN_IN_HTTP {
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
            Step::Browse { .. }
            | Step::Authorize
            | Step::Return
            | Step::FixtureReady
            | Step::ReleaseHold { .. }
            | Step::Terminal(_)
            | Step::StopWorker { .. }
            | Step::StartWorker { .. }
            | Step::AddWorker { .. }
            | Step::Advance
            | Step::Push { .. }
            | Step::Join { .. }
            | Step::Watching { .. }
            | Step::Held { .. }
            | Step::Kill { .. }
            | Step::Probe { .. }
            | Step::Release { .. }
            | Step::Contended { .. } => panic!("command step"),
        };
        let mut words = vec![fixtures::directory(&self.scratch, *at).as_os_str().as_bytes().into()];
        if let Step::Until { holds, .. } = &self.steps[self.next]
            && let Some(index) = self.current
            && let Some(watch) = holds.watch_after(&self.calls[index].stdout)
        {
            words.extend(watch);
            return words;
        }
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
                Arg::InboxAfter(call) => {
                    let value: serde_json::Value =
                        serde_json::from_slice(&self.calls[*call].stdout).expect("inbox JSON");
                    let cursor = value
                        .pointer("/needs/after/value")
                        .and_then(serde_json::Value::as_str)
                        .expect("next needs page");
                    format!("00{cursor}").into_bytes().into()
                }
                Arg::GithubApp => self.github_app.expect("person made the App").to_string().into_bytes().into(),
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
                observed_at: None,
                tries: 0,
                early: 0,
            });
            self.running.push(None);
        }
        let call = &mut self.calls[index];
        call.stdout.clear();
        call.stderr.clear();
        call.exit = None;
        call.observed_at = None;
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
    fn complete(&mut self, complete: Complete, now: Time, wall: Wall) {
        if crate::terminal::Typist::owns(complete.op) {
            self.terminal.complete(complete, &mut self.submissions);
            return;
        }
        let index = self.owners.remove(&complete.op).expect("operation owner");
        if index == SIGN_IN_HTTP {
            if let Some(op) = self.visit.as_mut().expect("sign-in GET").complete(complete) {
                self.submit(SIGN_IN_HTTP, op);
            }
            return;
        }
        if index == BROWSER {
            if let Some(op) = self.browser.as_mut().expect("browser step").complete(complete) {
                self.submit(BROWSER, op);
            }
            return;
        }
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
            (Op::Spawn { spawn }, Done::Spawned { pidfd, .. }) => {
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
                self.calls[index].observed_at = Some(wall);
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
        if !self.terminal.finish(&mut self.submissions) {
            return;
        }
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
    fn kill_call(&mut self, index: usize) -> bool {
        if self.current.is_none() {
            let pidfd = self.running[index].as_ref().expect("kill running call").pidfd.expect("spawned call");
            self.submit(index, Op::Signal { pidfd, signal: Signal::Kill, to: skein_io::kernel::Target::Child });
            self.current = Some(index);
            return false;
        }
        if self.running[index].is_some() {
            return false;
        }
        self.current = None;
        true
    }
    fn interactive_act(&mut self) -> bool {
        if matches!(self.steps[self.next], Step::Authorize | Step::Return) {
            return self.sign_in_http();
        }
        if let Step::Browse { call } = self.steps[self.next] {
            return self.browse_act(call);
        }
        let Step::Terminal(act) = &self.steps[self.next] else { panic!("terminal step") };
        self.terminal.act(
            act,
            &self.scratch,
            self.root.expect("terminal launch root"),
            self.links.get(),
            self.kept.as_ref(),
            &mut self.submissions,
        )
    }
    fn watch_call(&mut self, call: usize) -> bool {
        assert!(self.running[call].as_ref().is_some_and(|r| r.started), "started watch call");
        self.watching = Some(call);
        if !self.watched {
            return false;
        }
        self.watching = None;
        self.watched = false;
        true
    }
    fn sign_in_http(&mut self) -> bool {
        let authorize = matches!(self.steps[self.next], Step::Authorize);
        if self.visit.is_none() {
            let url = if authorize {
                self.terminal.sign_in_address()
            } else {
                self.kept.as_ref().expect("kept redirect before Return").expose()
            };
            let visit = crate::issuer::Visit::new(url, authorize);
            let op = visit.start();
            self.visit = Some(visit);
            self.submit(SIGN_IN_HTTP, op);
            return false;
        }
        if !self.visit.as_ref().expect("sign-in GET").done {
            return false;
        }
        let visit = self.visit.take().expect("settled sign-in GET");
        if authorize {
            self.kept = Some(visit.redirect.expect("unfollowed issuer redirect"));
        } else {
            self.returned.push(visit.answer);
        }
        true
    }
    fn browse_act(&mut self, call: usize) -> bool {
        if self.browser.is_none() {
            assert!(self.running[call].is_none(), "browse a completed call");
            let browser = Browser::new(call, &self.calls[call].stdout);
            let op = browser.start();
            self.browser = Some(browser);
            self.submit(BROWSER, op);
            return false;
        }
        if !self.browser.as_ref().expect("browser").done {
            return false;
        }
        self.browsed.push(self.browser.take().expect("finished browser").seen);
        true
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
            Step::Browse { .. } | Step::Authorize | Step::Return | Step::Terminal(_) => {
                if !self.interactive_act() {
                    return;
                }
            }
            Step::FixtureReady if self.running.iter().any(Option::is_some) => return,
            Step::FixtureReady => self.fixture_ready(),
            Step::AddWorker { .. } if !self.added.replace(false) => return,
            Step::StopWorker { .. } | Step::StartWorker { .. } if !self.worker_ready.replace(false) => return,
            Step::Advance | Step::Push { .. } if !self.advanced.get() => return,
            Step::StopWorker { .. }
            | Step::StartWorker { .. }
            | Step::AddWorker { .. }
            | Step::Advance
            | Step::Push { .. } => {}
            Step::ReleaseHold { .. } => {
                if !self.hold_released.replace(false) {
                    return;
                }
            }
            Step::Held { .. } => return,
            Step::Kill { call } => {
                if !self.kill_call(*call) {
                    return;
                }
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
                if !self.watch_call(*call) {
                    return;
                }
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
                    && !holds.matches(&call.stdout)
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
            Some(Step::Authorize | Step::Return) => self.visit.as_ref().is_none_or(|visit| visit.done),
            Some(Step::Browse { .. }) => self.browser.as_ref().is_none_or(|browser| browser.done),
            Some(Step::Terminal(act)) => self.terminal.ready(act, self.links.get()),
            Some(Step::FixtureReady) => self.running.iter().all(Option::is_none),
            Some(Step::StopWorker { .. } | Step::StartWorker { .. }) => self.worker_ready.get(),
            Some(Step::Advance | Step::Push { .. }) => self.advanced.get(),
            Some(Step::ReleaseHold { .. }) => self.hold_released.get(),
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
            None => self.root.is_some() && self.running.iter().all(Option::is_none) && self.terminal.finish_ready(),
        }
    }
}
impl Host for Caller {
    fn iterate(&mut self, now: Time, wall: Wall) {
        while let Some(complete) = self.completions.pop() {
            self.complete(complete, now, wall);
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
        self.released
            && self.root.is_none()
            && self.probe.is_none()
            && self.owners.is_empty()
            && self.browser.is_none()
            && self.visit.is_none()
            && self.terminal.is_empty()
    }
    fn worst_case(&self) -> u64 {
        self.memory
    }
    fn operations(&self) -> u32 {
        ROOM
    }
}

pub(crate) mod recorded;
pub(crate) use recorded::Recorded;

#[cfg(test)]
mod read_condition_tests {
    use super::*;
    use serde_json::json;

    fn writing() -> serde_json::Value {
        json!({"chip":{"task":8,"phase":"running","details":{"owner":{
            "phase":{"active":{"running":{"attempt":9}}},
            "details":{"attempt":9,"observation":{"work":"writing"}}}}},
            "result":null,"digest":"0123456789abcdef",
            "lineage_chips":[{"task":4,"phase":"running","details":{"owner":{
                "details":{"observation":{"work":"writing"}}}}}]})
    }
    #[test]
    fn writing_task_rejects_an_ancestors_work_and_requires_its_own_open_attempt() {
        let condition = ReadCondition::WritingTask { task: 8 };
        let good = writing();
        for value in [&good, &json!({"task":good,"met":true})] {
            assert!(condition.matches(&serde_json::to_vec(value).expect("JSON")));
        }
        // Captured failure: reviewer 8 was due, with no attempt/observation;
        // the old substring accepted B's writing chip in lineage_chips.
        let mut due = good.clone();
        due["chip"]["phase"] = json!("due");
        due["chip"]["details"]["owner"]["phase"]["active"] = json!("due");
        due["chip"]["details"]["owner"]["details"]["attempt"] = json!(null);
        due["chip"]["details"]["owner"]["details"]["observation"] = json!(null);
        let bytes = serde_json::to_vec(&due).expect("JSON");
        assert!(ReadCondition::Contains(b"\"work\":\"writing\"").matches(&bytes));
        assert!(!condition.matches(&bytes));
        for (path, wrong) in [
            ("/chip/task", json!(4)),
            ("/chip/phase", json!("claimed")),
            ("/chip/details/owner/details/observation/work", json!("preparing")),
            ("/chip/details/owner/details/attempt", json!(10)),
            ("/chip/details/owner/phase/active/running/attempt", json!(null)),
            ("/result", json!({"ended":"finished"})),
        ] {
            let mut value = good.clone();
            *value.pointer_mut(path).expect("field") = wrong;
            assert!(!condition.matches(&serde_json::to_vec(&value).expect("JSON")), "{path}");
        }
    }
    #[test]
    fn writing_task_watches_each_observed_digest_and_retains_the_matching_response() {
        let mut caller = Caller::new(vec![writing_task(D1, 8)], PathBuf::new(), Fd::new(0));
        let words = |caller: &Caller| caller.words().into_iter().skip(1).collect::<Vec<_>>();
        let expected = |args: &[&str]| args.iter().map(|a| a.as_bytes().into()).collect::<Vec<Box<[u8]>>>();
        assert_eq!(words(&caller), expected(&["wait", "task", "8", "--until", "phase:running", "--json"]));
        caller.advance(Time::from_nanos(1));
        for (when, digest) in [(2, "0123456789abcdef"), (4, "fedcba9876543210")] {
            let mut value = writing();
            value["digest"] = json!(digest);
            value["chip"]["details"]["owner"]["details"]["observation"]["work"] = json!("preparing");
            caller.running[0] = None;
            caller.calls[0].exit = Some(Exit::Code(0));
            caller.calls[0].stdout = serde_json::to_vec(&json!({"task":value,"met":true})).expect("JSON");
            assert_eq!(words(&caller), expected(&["wait", "task", "8", "--since", digest, "--json"]));
            caller.advance(Time::from_nanos(when));
            assert_eq!(caller.next, 0);
        }
        let stdout = serde_json::to_vec(&json!({"task":writing(),"met":true})).expect("JSON");
        caller.running[0] = None;
        caller.calls[0].exit = Some(Exit::Code(0));
        caller.calls[0].stdout = stdout.clone();
        caller.calls[0].observed_at = Some(Wall::from_nanos(6));
        caller.advance(Time::from_nanos(7));
        assert_eq!(caller.next, 1);
        assert_eq!(caller.calls.len(), 1);
        assert_eq!(caller.calls[0].stdout, stdout);
        assert_eq!(caller.calls[0].observed_at, Some(Wall::from_nanos(6)));
    }
    fn limited() -> Vec<u8> {
        serde_json::to_vec_pretty(&json!({"chip":{"task":crate::case::A,"phase":"due"},
            "run_wait":{"account":{"account":crate::case::FIRST_ACCOUNT,"source":0,
                "why":{"limited":{"until":42}}}}}))
        .expect("task JSON")
    }
    fn condition() -> ReadCondition {
        ReadCondition::LimitedTask { task: crate::case::A, account: crate::case::FIRST_ACCOUNT, source: 0 }
    }
    #[test]
    fn limited_task_requires_all_fields_together_in_one_decoded_document() {
        let bytes = limited();
        assert!(condition().matches(&bytes), "JSON whitespace does not change the predicate");
        for (path, wrong) in [
            ("/chip/task", json!(999)),
            ("/chip/phase", json!("running")),
            ("/run_wait/account/account", json!(999)),
            ("/run_wait/account/source", json!(1)),
            ("/run_wait/account/why/limited/until", json!(null)),
            ("/run_wait/account/why/limited/until", json!("42")),
            ("/run_wait/account/why/limited/until", json!(-1)),
            ("/run_wait/account/why/limited/until", json!(42.5)),
            ("/run_wait/account/why/limited/until", json!(true)),
        ] {
            let mut value: serde_json::Value = serde_json::from_slice(&bytes).expect("JSON");
            *value.pointer_mut(path).expect("field") = wrong;
            assert!(!condition().matches(&serde_json::to_vec(&value).expect("JSON")), "{path}");
        }
        assert!(!condition().matches(br#"{"chip":{"task":2,"phase":"due"},"run_wait":null}"#));
        assert!(!condition().matches(br#"{"chip":{"task":2,"phase":"due"}}"#));
        let mut missing: serde_json::Value = serde_json::from_slice(&bytes).expect("JSON");
        missing["run_wait"]["account"]["why"]["limited"].as_object_mut().expect("limit").remove("until");
        assert!(!condition().matches(&serde_json::to_vec(&missing).expect("JSON")));
        assert!(!condition().matches(b"not JSON"));
        let mut joined = bytes.clone();
        joined.extend_from_slice(&bytes);
        assert!(!condition().matches(&joined), "fields cannot be taken from separate responses");
    }
    #[test]
    fn substring_conditions_keep_existing_non_json_semantics() {
        let condition = ReadCondition::Contains(b"ready");
        assert!(condition.matches(b"plain ready text"));
        assert!(!condition.matches(b"plain pending text"));
    }
    #[test]
    fn real_loop_until_retries_initial_due_and_retains_exact_matching_response() {
        let mut caller = Caller::new(
            vec![until_matching(D1, crate::argv!["task", number(crate::case::A), "--json"], condition(), b"ended")],
            PathBuf::new(),
            Fd::new(0),
        );
        caller.advance(Time::from_nanos(1));
        caller.running[0] = None;
        caller.calls[0].exit = Some(Exit::Code(0));
        caller.calls[0].stdout =
            serde_json::to_vec(&json!({"chip":{"task":crate::case::A,"phase":"due"},"run_wait":null}))
                .expect("initial due JSON");
        caller.calls[0].observed_at = Some(Wall::from_nanos(2));
        caller.advance(Time::from_nanos(3));
        assert_eq!(caller.next, 0, "initial admission must not advance to accounts");
        assert_eq!(caller.calls[0].tries, 2);
        assert_eq!(caller.calls[0].observed_at, None, "a retry clears its preceding response timestamp");
        let stdout = limited();
        caller.running[0] = None;
        caller.calls[0].exit = Some(Exit::Code(0));
        caller.calls[0].stdout = stdout.clone();
        caller.calls[0].observed_at = Some(Wall::from_nanos(4));
        caller.advance(Time::from_nanos(5));
        assert_eq!(caller.next, 1);
        assert_eq!(caller.calls.len(), 1, "retries retain the original call slot");
        assert_eq!(caller.calls[0].stdout, stdout, "retain actual response bytes without reconstruction");
        assert_eq!(caller.calls[0].observed_at, Some(Wall::from_nanos(4)));
    }
    #[test]
    fn real_loop_completion_uses_its_wall_input() {
        let mut caller = Caller::new(vec![call(D1, crate::argv!["accounts", "--json"])], PathBuf::new(), Fd::new(0));
        caller.advance(Time::from_nanos(1));
        caller.owners.insert(Token::new(100), 0);
        caller.complete(
            Complete {
                op: Token::new(100),
                kind: Op::Wait { pidfd: Fd::new(7), reap: true },
                result: Ok(Done::Exit(Exit::Code(0))),
            },
            Time::from_nanos(2),
            Wall::from_nanos(123),
        );
        assert_eq!(caller.calls[0].observed_at, Some(Wall::from_nanos(123)));
        assert_eq!(caller.calls[0].ended, Time::from_nanos(2));
    }
}
