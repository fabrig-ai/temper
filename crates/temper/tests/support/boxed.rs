//! Drives the box through its pipes, signals and supervised process tree.
use super::{Process, StdoutReader, printed_link};
use skein_io::kernel::{Exit, Signal};
use skein_lib::{Duration, Time};
use skein_shell::{Clock, Host};
use skein_world::{
    Referee,
    end_to_end::{Binary, Command, Expectation, Mode, Streams},
    real,
};
use std::{
    cell::Cell,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    rc::Rc,
};

/// What the referee does after the link arrives.
#[derive(Clone, Copy, Debug)]
pub enum Plan {
    /// Probe Forgejo and ask the box to stop.
    Stop,
    /// Kill Forgejo, wait for its replacement, and check the engine before stopping.
    KillForgejo,
}
/// Evidence retained after every process and pipe settles.
pub struct Outcome {
    pub exit: Option<Exit>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub forgejo_at_link: Option<u16>,
    pub left: Option<Vec<u32>>,
    pub forgejo_pids: Option<(u32, u32)>,
    pub engine_served: Option<bool>,
}
#[derive(Clone, Copy)]
enum Phase {
    Linked,
    Killed { engine: u32, forgejo: u32 },
    Stopping,
}
struct Judge {
    plan: Plan,
    forgejo: Option<SocketAddr>,
    forgejo_at_link: Rc<Cell<Option<u16>>>,
    forgejo_pids: Rc<Cell<Option<(u32, u32)>>>,
    engine_served: Rc<Cell<Option<bool>>>,
    temper_program: PathBuf,
    forgejo_program: Option<PathBuf>,
    phase: Phase,
    next_probe: Time,
    cleanup: bool,
    deadline: Time,
    settled: bool,
}
impl Judge {
    fn advance(&mut self, binary: &mut Binary, address: SocketAddr) {
        let pids = &binary.tree_status().live_pids;
        match self.phase {
            Phase::Linked => {
                self.forgejo_at_link.set(self.forgejo.and_then(|address| status(address, "/api/v1/version")));
                match self.plan {
                    Plan::Stop => self.stop(binary),
                    Plan::KillForgejo => {
                        if let Some(engine) = find(pids, &self.temper_program, b"serve")
                            && let Some(forgejo) =
                                find(pids, self.forgejo_program.as_deref().expect("Forgejo restart plan"), b"web")
                        {
                            let killed = std::process::Command::new("/usr/bin/kill")
                                .args(["-KILL", &forgejo.to_string()])
                                .status()
                                .expect("kill Forgejo");
                            assert!(killed.success(), "Forgejo's pid accepted the kill");
                            self.phase = Phase::Killed { engine, forgejo };
                        }
                    }
                }
            }
            Phase::Killed { engine, forgejo } => {
                if let Some(restarted) =
                    find(pids, self.forgejo_program.as_deref().expect("Forgejo restart plan"), b"web")
                    && restarted != forgejo
                    && self.forgejo.and_then(|address| status(address, "/api/v1/version")) == Some(200)
                {
                    self.forgejo_pids.set(Some((forgejo, restarted)));
                    self.engine_served.set(Some(
                        find(pids, &self.temper_program, b"serve") == Some(engine) && status(address, "/").is_some(),
                    ));
                    self.stop(binary);
                }
            }
            Phase::Stopping => {}
        }
    }
    fn stop(&mut self, binary: &mut Binary) {
        binary.signal(Signal::Terminate);
        self.phase = Phase::Stopping;
    }
}
fn find(pids: &[u32], program: &Path, argument: &[u8]) -> Option<u32> {
    pids.iter().copied().find(|pid| {
        std::fs::read_link(format!("/proc/{pid}/exe")).ok().as_deref() == Some(program)
            && std::fs::read(format!("/proc/{pid}/cmdline"))
                .is_ok_and(|bytes| bytes.split(|b| *b == 0).any(|arg| arg == argument))
    })
}
impl Referee<Process> for Judge {
    fn act(&mut self, now: Time, procs: &mut [Process]) {
        let Process::Stdout(stdout) = &procs[1] else { panic!("stdout reader") };
        let engine = printed_link(&stdout.bytes)
            .and_then(|link| std::str::from_utf8(link).ok()?.strip_prefix("http://")?.split('/').next()?.parse().ok());
        let Process::Binary(binary) = &mut procs[0] else { panic!("binary") };
        if let Some(engine) = engine
            && now >= self.next_probe
        {
            self.advance(binary, engine);
            self.next_probe = now.saturating_add(Duration::from_millis(200));
        }
        let left = binary.tree_status().live_at_leader_exit.as_ref().is_some_and(|pids| !pids.is_empty());
        if !self.cleanup && (left || now >= self.deadline) && !binary.is_empty() {
            binary.signal_tree(Signal::Kill);
            self.cleanup = true;
        }
    }
    fn observe(&mut self, _now: Time, procs: &[Process]) {
        self.settled = procs.iter().all(Host::is_empty);
    }
    fn next_deadline(&self) -> Option<Time> {
        (!self.cleanup && !self.settled).then_some(self.deadline)
    }
    fn overdue(&self, _now: Time) -> Option<String> {
        None
    }
    fn passed(&self) -> bool {
        self.settled
    }
}
/// Opt in only for a story whose assertions read a step's diagnostic bytes.
#[must_use]
pub fn echo_steps(mut flags: Vec<String>) -> Vec<String> {
    flags.extend(["--echo".into(), "steps".into()]);
    flags
}
#[must_use]
pub fn run_echo_steps(directory: &Path, forgejo: Option<SocketAddr>, plan: Plan, deadline: Duration) -> Outcome {
    let mut command = Command {
        program: skein_scratch::paths::binary("temper"),
        arguments: std::iter::once("box".into()).chain(super::configuration::flags(directory)).collect(),
        directory: directory.into(),
        environment: Vec::new(),
    };
    command.arguments.extend(["--echo".into(), "steps".into()]);
    run_command(command, forgejo, plan, deadline)
}
/// Starts the shipped box and drives its referee within the supplied deadline.
#[must_use]
pub fn run(directory: &Path, forgejo: Option<SocketAddr>, plan: Plan, deadline: Duration) -> Outcome {
    run_command(
        Command {
            program: skein_scratch::paths::binary("temper"),
            arguments: std::iter::once("box".into()).chain(super::configuration::flags(directory)).collect(),
            directory: directory.into(),
            environment: Vec::new(),
        },
        forgejo,
        plan,
        deadline,
    )
}
/// Drives the same box referee with an isolation wrapper around the shipped binary.
#[must_use]
pub fn run_command(command: Command, forgejo: Option<SocketAddr>, plan: Plan, deadline: Duration) -> Outcome {
    let clock = Clock::new();
    let mut binary =
        Binary::start_with_tree(dying_with_test(command), Mode::Pipes, 262_144, Expectation::Supervise, false)
            .expect("temper box starts");
    let streams = binary.take_streams().expect("binary streams");
    let fds = streams.descriptors();
    let Streams::Pipes { input, output } = streams else { panic!("pipes mode") };
    let forgejo_at_link = Rc::new(Cell::new(None));
    let forgejo_pids = Rc::new(Cell::new(None));
    let engine_served = Rc::new(Cell::new(None));
    let forgejo_program = if matches!(plan, Plan::KillForgejo) {
        Some(std::fs::canonicalize(super::owned::binaries().0).expect("Forgejo path"))
    } else {
        None
    };
    let mut world = real::World::new(Judge {
        plan,
        forgejo,
        forgejo_at_link: forgejo_at_link.clone(),
        forgejo_pids: forgejo_pids.clone(),
        engine_served: engine_served.clone(),
        temper_program: std::fs::canonicalize(skein_scratch::paths::binary("temper")).expect("temper path"),
        forgejo_program,
        phase: Phase::Linked,
        next_probe: Time::ZERO,
        cleanup: false,
        deadline: clock.now().now.saturating_add(Duration::from_nanos(
            deadline.as_nanos().saturating_sub(Duration::from_secs(10).as_nanos()).max(deadline.as_nanos() / 2),
        )),
        settled: false,
    });
    world.spawn_with_fds(binary.descriptors(), || Process::Binary(Box::new(binary)));
    world.spawn_with_fds(fds, || Process::Stdout(Box::new(StdoutReader::new(input, output))));
    let ended = world.run(&clock, deadline);
    let Process::Binary(binary) = &ended.procs[0] else { panic!("binary") };
    let Process::Stdout(stdout) = &ended.procs[1] else { panic!("stdout reader") };
    Outcome {
        exit: binary.exit_status(),
        stdout: stdout.bytes.clone(),
        stderr: binary.stderr().to_vec(),
        forgejo_at_link: forgejo_at_link.get(),
        left: binary.tree_status().live_at_leader_exit.clone(),
        forgejo_pids: forgejo_pids.get(),
        engine_served: engine_served.get(),
    }
}
/// The status line of a plain request, or none when no server answers.
#[must_use]
pub fn status(address: SocketAddr, target: &str) -> Option<u16> {
    let timeout = std::time::Duration::from_millis(100);
    let mut socket = TcpStream::connect_timeout(&address, timeout).ok()?;
    socket.set_read_timeout(Some(timeout)).ok()?;
    socket.set_write_timeout(Some(timeout)).ok()?;
    write!(socket, "GET {target} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n").ok()?;
    let mut bytes = [0_u8; 128];
    let length = socket.read(&mut bytes).ok()?;
    std::str::from_utf8(&bytes[..length]).ok()?.split_whitespace().nth(1)?.parse().ok()
}

/// The first run's evidence, including a check made while the box still serves.
pub struct StoryOutcome<T> {
    pub exit: Option<Exit>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub left: Option<Vec<u32>>,
    pub finished: bool,
    pub dormant_checked: bool,
    pub checked: T,
}
struct StoryJudge<'a, F, T> {
    check: Option<F>,
    checked: &'a Cell<Option<T>>,
    dormant_checked: &'a Cell<bool>,
    logs: PathBuf,
    stopping: bool,
    cleanup: bool,
    settled: bool,
    deadline: Time,
}
impl<F: FnOnce() -> T, T> Referee<Process> for StoryJudge<'_, F, T> {
    fn act(&mut self, now: Time, procs: &mut [Process]) {
        let Process::Person(person) = &procs[1] else { panic!("native person") };
        let finished = person.finished && person.client.as_ref().is_some_and(Host::is_empty);
        let Process::Binary(binary) = &mut procs[0] else { panic!("binary") };
        if !self.stopping && finished {
            self.checked.set(Some(self.check.take().expect("one check before stopping")()));
            binary.signal(Signal::Terminate);
            self.stopping = true;
        }
        let left = binary.tree_status().live_at_leader_exit.as_ref().is_some_and(|pids| !pids.is_empty());
        if !self.cleanup && (left || now >= self.deadline) && !binary.is_empty() {
            binary.signal_tree(Signal::Kill);
            self.cleanup = true;
        }
    }
    fn observe(&mut self, now: Time, procs: &[Process]) {
        self.settled = self.stopping && procs.iter().all(Host::is_empty);
        let Process::Binary(binary) = &procs[0] else { panic!("binary") };
        let Process::Person(person) = &procs[1] else { panic!("native person") };
        let stderr = String::from_utf8_lossy(binary.stderr());
        if person.choosing_playground() {
            for name in ["fake-llm.log", "fake-forge-1.log", "prepare-1.log"] {
                let path = self.logs.join(name);
                assert!(
                    std::fs::read(&path).expect("private step log exists").is_empty(),
                    "the demo's fakes stay dormant at the welcome: {}",
                    path.display()
                );
            }
            self.dormant_checked.set(true);
        }
        if person.failed.is_some() {
            for node in person.client.as_ref().expect("native client").tree().nodes() {
                eprintln!(
                    "{:?}: {} {}",
                    node.role,
                    String::from_utf8_lossy(node.name.as_deref().unwrap_or_default()),
                    node.text.iter().map(|run| String::from_utf8_lossy(&run.text)).collect::<String>()
                );
            }
        }
        assert!(person.failed.is_none(), "{:?}; temper box stderr: {stderr}", person.failed);
        assert!(binary.exit_status().is_none() || self.stopping, "temper box ended before the person: {stderr}");
        assert!(binary.exit_status().is_none_or(|exit| exit == Exit::Code(0)), "temper box failed: {stderr}");
        assert!(!binary.stderr_overflowed(), "temper box overflowed stderr: {stderr}");
        assert!(now < self.deadline || self.settled, "native person did not settle: {stderr}");
    }
    fn next_deadline(&self) -> Option<Time> {
        (!self.settled).then_some(self.deadline)
    }
    fn overdue(&self, now: Time) -> Option<String> {
        (!self.settled && now >= self.deadline).then(|| "box's person did not finish and settle".into())
    }
    fn passed(&self) -> bool {
        self.settled
    }
}
/// Runs the shipped box on its pipes with a fresh client rooted at `client`.
/// Checks the forge after the person finishes and their client settles, before SIGTERM.
#[must_use]
pub fn run_story<T>(
    directory: &Path,
    flags: Vec<String>,
    client: &Path,
    script: super::person::Script,
    deadline: Duration,
    check: impl FnOnce() -> T,
) -> StoryOutcome<T> {
    let data = flags.windows(2).find(|pair| pair[0] == "--data").expect("explicit data directory");
    let logs = Path::new(&data[1]).join("box/logs");
    let clock = Clock::new();
    let mut binary = Binary::start_with_tree(
        dying_with_test(Command {
            program: skein_scratch::paths::binary("temper"),
            arguments: std::iter::once("box".into()).chain(flags.into_iter().map(Into::into)).collect(),
            directory: directory.into(),
            // Init's optional git/claude discovery is exercised without PATH.
            // The demo uses the written link to this binary, and needs neither.
            environment: Vec::new(),
        }),
        Mode::Pipes,
        262_144,
        Expectation::Supervise,
        false,
    )
    .expect("temper box starts");
    let streams = binary.take_streams().expect("binary pipes");
    let mut fds = streams.descriptors();
    let Streams::Pipes { input, output } = streams else { panic!("pipes mode") };
    let root = skein_shell::open_root(client).expect("client's parent root");
    fds.push(root);
    let checked = Cell::new(None);
    let dormant_checked = Cell::new(false);
    let mut world = real::World::new(StoryJudge {
        check: Some(check),
        checked: &checked,
        dormant_checked: &dormant_checked,
        logs,
        stopping: false,
        cleanup: false,
        settled: false,
        deadline: clock.now().now.saturating_add(Duration::from_nanos(
            deadline.as_nanos().saturating_sub(Duration::from_secs(10).as_nanos()),
        )),
    });
    world.spawn_with_fds(binary.descriptors(), || Process::Binary(Box::new(binary)));
    world.spawn_with_fds(fds, || Process::Person(Box::new(super::person::Person::new(root, input, output, script))));
    let ended = world.run(&clock, deadline);
    assert!(ended.procs.iter().all(Host::is_empty), "all real io settles");
    let Process::Binary(binary) = &ended.procs[0] else { panic!("binary") };
    let Process::Person(person) = &ended.procs[1] else { panic!("native person") };
    StoryOutcome {
        exit: binary.exit_status(),
        stdout: person.stdout().to_vec(),
        stderr: binary.stderr().to_vec(),
        left: binary.tree_status().live_at_leader_exit.clone(),
        finished: person.finished,
        dormant_checked: dormant_checked.get(),
        checked: checked.take().expect("the finished person's forge check ran"),
    }
}

/// A box driven through ctl, with evidence taken before its stop.
pub struct CtlOutcome<T> {
    pub exit: Option<Exit>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub left: Option<Vec<u32>>,
    pub calls: Vec<super::Ended>,
    pub checked: Option<T>,
    pub visits: Vec<Vec<super::sign_in_browser::Hop>>,
}
struct CtlJudge<'a, F, T> {
    expected_exits: Vec<(usize, u8)>,
    face: Option<super::forge_land::FaceProcs>,
    visit: Visit,
    tab: Option<usize>,
    tab_visit: Visit,
    visits: &'a Cell<Vec<Vec<super::sign_in_browser::Hop>>>,
    reply: Option<super::forge_land::Reply>,
    on_link: Option<Box<dyn FnOnce()>>,
    check: Option<F>,
    checked: &'a Cell<Option<T>>,
    stopping: bool,
    cleanup: bool,
    settled: bool,
    deadline: Time,
    cleanup_deadline: Time,
}
enum Visit {
    AwaitingAddress,
    Handed,
}
impl<F: FnOnce() -> T, T> Referee<Process> for CtlJudge<'_, F, T> {
    fn act(&mut self, now: Time, procs: &mut [Process]) {
        let Process::Calls(calls) = &procs[1] else { panic!("ctl") };
        let failed = calls.ends().iter().enumerate().any(|(index, call)| {
            let expected = self.expected_exits.iter().find(|(at, _)| *at == index).map_or(0, |(_, code)| *code);
            call.exit != Some(Exit::Code(expected))
        });
        if let Some(face) = &self.face {
            let Process::Calls(calls) = &mut procs[1] else { panic!("ctl") };
            if matches!(self.visit, Visit::AwaitingAddress)
                && let Some(address) = calls.ends().iter().find_map(super::sign_in::authorization_address)
            {
                let address = address.to_vec();
                let Process::Visitor(visitor) = &mut procs[face.visitor] else { panic!("visitor") };
                visitor.hand(&address);
                self.visit = Visit::Handed;
            }
            let Process::Calls(calls) = &mut procs[1] else { panic!("ctl") };
            if !failed {
                self.reply.as_mut().expect("face reply reads").advance(calls, &mut self.expected_exits);
            }
        }
        if let Some(index) = self.tab {
            let (drivers, visitors) = procs.split_at_mut(index);
            let Process::Calls(calls) = &mut drivers[1] else { panic!("ctl") };
            let Process::Visitor(visitor) = &mut visitors[0] else { panic!("tab visitor") };
            if calls.visiting() {
                if matches!(self.tab_visit, Visit::Handed) && visitor.is_empty() {
                    let hops = visitor.hops().to_vec();
                    calls.visited(hops.last().expect("landing hop").status);
                    let mut visits = self.visits.take();
                    visits.push(hops);
                    self.visits.set(visits);
                    self.tab_visit = Visit::AwaitingAddress;
                } else if matches!(self.tab_visit, Visit::AwaitingAddress) {
                    let address = calls
                        .ends()
                        .last()
                        .and_then(super::sign_in::authorization_address)
                        .expect("last act printed a browser address");
                    visitor.hand(address);
                    self.tab_visit = Visit::Handed;
                }
            }
        }
        let (drivers, peers) = procs.split_at_mut(2);
        let [Process::Binary(binary), Process::Calls(calls)] = drivers else { panic!("box and ctl") };
        if printed_link(calls.stdout()).is_some()
            && let Some(on_link) = self.on_link.take()
        {
            on_link();
        }
        calls.observe_box_stderr(binary.stderr(), now);
        // The box's stderr may end a call: judge the calls again after it.
        let failed = calls.ends().iter().enumerate().any(|(index, call)| {
            let expected = self.expected_exits.iter().find(|(at, _)| *at == index).map_or(0, |(_, code)| *code);
            call.exit != Some(Exit::Code(expected))
        });
        let expired = now >= self.deadline;
        if !self.stopping && (calls.idle() || failed || expired || binary.exit_status().is_some()) {
            if self.face.is_some() {
                eprintln!(
                    "face box stop: idle {}, failed {failed}, expired {expired}, calls {}",
                    calls.idle(),
                    calls.ends().len()
                );
                for (index, call) in calls.ends().iter().enumerate().rev().take(3) {
                    eprintln!(
                        "face call {index}: {:?}; stdout {}; stderr {}",
                        call.exit,
                        String::from_utf8_lossy(&call.stdout),
                        String::from_utf8_lossy(&call.stderr)
                    );
                }
                eprintln!("face box stderr: {}", String::from_utf8_lossy(binary.stderr()));
            }
            if calls.idle() && !failed && !expired && binary.exit_status().is_none() {
                self.checked.set(Some(self.check.take().expect("one live check")()));
            }
            // A held batch keeps its root open for the reply referee. Release
            // it on failure too, so cancellation drains that descriptor.
            calls.release();
            calls.cancel();
            for proc in peers {
                match proc {
                    Process::Peer(peer) => peer.shutdown(),
                    Process::Issuer(issuer) => issuer.shutdown(),
                    Process::Binary(_)
                    | Process::Calls(_)
                    | Process::Person(_)
                    | Process::Stdout(_)
                    | Process::Visitor(_) => {}
                }
            }
            if binary.exit_status().is_none() {
                binary.signal(Signal::Terminate);
            }
            self.stopping = true;
        }
        let left = binary.tree_status().live_at_leader_exit.as_ref().is_some_and(|pids| !pids.is_empty());
        if !self.cleanup && (left || now >= self.cleanup_deadline) && !binary.is_empty() {
            binary.signal_tree(Signal::Kill);
            self.cleanup = true;
        }
    }
    fn observe(&mut self, _now: Time, procs: &[Process]) {
        self.settled = self.stopping && procs.iter().all(Host::is_empty);
        if self.face.is_some() && self.cleanup && !self.settled {
            eprintln!("face drain: {:?}", procs.iter().map(Host::is_empty).collect::<Vec<_>>());
        }
    }
    fn next_deadline(&self) -> Option<Time> {
        (!self.settled).then_some(if self.stopping { self.cleanup_deadline } else { self.deadline })
    }
    fn overdue(&self, _now: Time) -> Option<String> {
        None
    }
    fn passed(&self) -> bool {
        self.settled
    }
}
/// Uses the box's printed link for ctl sign-in, then drains the supervised tree.
/// A refused call stops the story immediately and retains its diagnostic.
#[must_use]
pub fn run_ctl<T>(
    directory: &Path,
    flags: Vec<String>,
    client_root: &Path,
    steps: Vec<super::ctl::Step>,
    deadline: Duration,
    check: impl FnOnce() -> T,
) -> CtlOutcome<T> {
    run_ctl_with_exits(directory, flags, client_root, steps, deadline, check, &[])
}
/// Continues through the exact expected nonzero calls, keeping the normal early-stop rule otherwise.
#[must_use]
pub fn run_ctl_with_exits<T>(
    directory: &Path,
    flags: Vec<String>,
    client_root: &Path,
    steps: Vec<super::ctl::Step>,
    deadline: Duration,
    check: impl FnOnce() -> T,
    expected_exits: &[(usize, u8)],
) -> CtlOutcome<T> {
    run_ctl_peers(
        directory,
        flags,
        client_root,
        steps,
        deadline,
        check,
        CtlPeers { expected_exits: expected_exits.to_vec(), model: CtlModel::None },
    )
    .0
}
/// An independent fake and a fixture hook after the box has written its files.
pub struct CtlLlm {
    pub expected_exits: Vec<(usize, u8)>,
    pub peer: super::llm::Listening,
    pub on_link: Box<dyn FnOnce()>,
}
struct CtlPeers {
    expected_exits: Vec<(usize, u8)>,
    model: CtlModel,
}
enum CtlModel {
    None,
    Llm(Box<CtlLlm>),
    Face(Box<super::forge_land::CtlFace>),
}
/// Drive the first task without changing the box's template or demo.
#[must_use]
pub fn run_ctl_with_llm<T>(
    directory: &Path,
    flags: Vec<String>,
    client_root: &Path,
    steps: Vec<super::ctl::Step>,
    deadline: Duration,
    check: impl FnOnce() -> T,
    llm: CtlLlm,
) -> CtlOutcome<T> {
    run_ctl_peers(
        directory,
        flags,
        client_root,
        steps,
        deadline,
        check,
        CtlPeers { expected_exits: llm.expected_exits.clone(), model: CtlModel::Llm(Box::new(llm)) },
    )
    .0
}
/// Drive the owned forge with a plan issuer, redirect visitor and bearer peer.
#[must_use]
pub fn run_ctl_with_face<T>(
    directory: &Path,
    flags: Vec<String>,
    client_root: &Path,
    steps: Vec<super::ctl::Step>,
    deadline: Duration,
    check: impl FnOnce() -> T,
    face: super::forge_land::CtlFace,
) -> (CtlOutcome<T>, super::forge_land::Seen) {
    let (outcome, seen) = run_ctl_peers(
        directory,
        flags,
        client_root,
        steps,
        deadline,
        check,
        CtlPeers { expected_exits: Vec::new(), model: CtlModel::Face(Box::new(face)) },
    );
    (outcome, seen.expect("face observations"))
}
fn run_ctl_peers<T>(
    directory: &Path,
    flags: Vec<String>,
    client_root: &Path,
    steps: Vec<super::ctl::Step>,
    deadline: Duration,
    check: impl FnOnce() -> T,
    peers: CtlPeers,
) -> (CtlOutcome<T>, Option<super::forge_land::Seen>) {
    let (peer, on_link, face, reply) = match peers.model {
        CtlModel::Llm(llm) => (Some(llm.peer), Some(llm.on_link), None, None),
        CtlModel::Face(story) => {
            (None, None, Some(story.face), Some(super::forge_land::Reply::new(story.first, story.reply)))
        }
        CtlModel::None => (None, None, None, None),
    };
    if let Some(super::forge_land::Face::Fake { peer, .. }) = &face {
        let config = flags.windows(2).find(|pair| pair[0] == "--config").expect("explicit config directory");
        peer.configure(Path::new(&config[1]));
    }
    let tab = steps.iter().any(|step| matches!(step, super::ctl::Step::Visit));
    assert!(!tab || face.is_none(), "box-owned peers or an independent face");
    let clock = Clock::new();
    let fake_backends = flags.iter().any(|flag| flag == "--fake-backends");
    eprintln!("T4 box-start {}", clock.now().wall.as_nanos());
    let mut binary = Binary::start_with_tree(
        dying_with_test(Command {
            program: skein_scratch::paths::binary("temper"),
            arguments: std::iter::once("box".into()).chain(flags.into_iter().map(Into::into)).collect(),
            directory: directory.into(),
            // Real repository stories, including a fake box's owned Forgejo,
            // need init to discover system git for their first chat's checkout.
            environment: if on_link.is_some() || face.is_some() || fake_backends {
                vec![("PATH".into(), "/usr/bin:/bin".into())]
            } else {
                Vec::new()
            },
        }),
        Mode::Pipes,
        262_144,
        Expectation::Supervise,
        false,
    )
    .expect("temper box starts");
    let streams = binary.take_streams().expect("box pipes");
    let mut fds = streams.descriptors();
    let root = skein_shell::open_root(client_root).expect("client root");
    fds.push(root);
    let checked = Cell::new(None);
    let visits = Cell::new(Vec::new());
    let mut world = real::World::new(CtlJudge {
        expected_exits: peers.expected_exits,
        face: face.as_ref().map(|_| super::forge_land::FaceProcs { issuer: 2, visitor: 3, peer: 4 }),
        visit: Visit::AwaitingAddress,
        tab: tab.then_some(2),
        tab_visit: Visit::AwaitingAddress,
        visits: &visits,
        reply,
        on_link: Some(Box::new(move || {
            eprintln!("T4 link {}", Clock::new().now().wall.as_nanos());
            if let Some(on_link) = on_link { on_link(); }
        })),
        check: Some(check),
        checked: &checked,
        stopping: false,
        cleanup: false,
        settled: false,
        // Leave ten seconds for cancellation, pipe drain and reaping.
        deadline: clock.now().now.saturating_add(Duration::from_nanos(
            deadline.as_nanos().saturating_sub(Duration::from_secs(10).as_nanos()),
        )),
        cleanup_deadline: clock.now().now.saturating_add(deadline),
    });
    world.spawn_with_fds(binary.descriptors(), || Process::Binary(Box::new(binary)));
    world.spawn_with_fds(fds, || {
        let mut calls = super::ctl::Calls::new(root, steps, Some(&streams), None).with_box_stderr();
        if face.is_some() {
            calls.hold();
        }
        Process::Calls(Box::new(calls))
    });
    if tab {
        world.spawn(|| Process::Visitor(Box::default()));
    }
    let face_procs = face.map(|face| face.spawn(&mut world));

    if let Some(peer) = peer {
        peer.spawn(&mut world);
    }
    let ended = world.run(&clock, deadline);
    eprintln!("T4 box-end {}", clock.now().wall.as_nanos());
    assert!(ended.procs.iter().all(Host::is_empty), "box, ctl and peer io settles");
    let [Process::Binary(binary), Process::Calls(calls), ..] = ended.procs.as_slice() else { panic!("box and ctl") };
    (
        CtlOutcome {
            exit: binary.exit_status(),
            stdout: calls.stdout().to_vec(),
            stderr: binary.stderr().to_vec(),
            left: binary.tree_status().live_at_leader_exit.clone(),
            calls: calls
                .ends()
                .iter()
                .map(|call| super::Ended { exit: call.exit, stdout: call.stdout.clone(), stderr: call.stderr.clone() })
                .collect(),
            checked: checked.take(),
            visits: visits.take(),
        },
        face_procs.as_ref().map(|at| super::forge_land::seen(&ended.procs, at)),
    )
}

/// Spawn on the calling test thread, which drives the world until teardown.
/// Linux's death signal follows that thread's lifetime, even across box's exec.
#[must_use]
pub fn dying_with_test(mut command: Command) -> Command {
    if cfg!(target_os = "linux") {
        let mut arguments = vec![
            "forge".into(),
            "child".into(),
            "--parent".into(),
            std::process::id().to_string().into(),
            // Terminate, so the box stops its steps as on any stop.
            "--signal".into(),
            "terminate".into(),
            "--".into(),
            command.program.into(),
        ];
        arguments.extend(command.arguments);
        command.program = skein_scratch::paths::binary("temper");
        command.arguments = arguments;
    }
    command
}
