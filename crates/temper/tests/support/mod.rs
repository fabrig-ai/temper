//! Shared configuration fixtures for startup and the process tier.
pub mod account_file_qa;
pub mod account_move;
pub mod account_move_qa;
pub mod boxed;
pub mod case;
pub mod cluster;
pub mod configuration;
pub mod ctl;
pub mod fetch;
pub mod first_repository;
pub mod github;
pub mod github_land;
pub mod owned;
pub mod terminal;
pub mod webhooks;
pub const WITHIN: skein_lib::Duration = skein_lib::Duration::from_secs(60);
pub mod forge_land;
pub mod forge_parent;
pub mod forge_sign_in;
pub mod forgejo;
pub mod forgejo_elsewhere;
pub mod forgejo_sign_in;
pub mod live;
pub mod live_github;
pub mod llm;
pub mod parent;
pub mod person;
pub mod playground;
pub mod program_login;
pub mod relay;
pub mod sign_in;
pub mod sign_in_browser;
pub mod sign_in_welcome;
use std::{fs, net::SocketAddr, os::unix::fs::PermissionsExt, path::Path};

/// `N` distinct loopback addresses that no listener holds now, none of
/// them in `taken`. A released port may be handed out again, so every
/// listener is held until all `N` are drawn, including a draw in `taken`.
/// Each skipped address stays held: at most `N + taken.len()` binds.
/// Two equal listeners fail temper-forge-setup's `check.rs:112`.
#[must_use]
pub fn free_addresses<const N: usize>(taken: &[SocketAddr]) -> [SocketAddr; N] {
    let mut addresses = [SocketAddr::from(([127, 0, 0, 1], 0)); N];
    let mut listeners = Vec::new();
    let mut drawn = 0;
    while drawn < N {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("free socket");
        let address = listener.local_addr().expect("socket address");
        listeners.push(listener);
        if !taken.contains(&address) {
            addresses[drawn] = address;
            drawn += 1;
        }
    }
    addresses
}

pub enum Forge<'a> {
    Unreached,
    Throwaway { forgejo: &'a Path, api: std::net::SocketAddr, git: String },
}
pub(super) fn write_private(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).expect("fixture file");
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).expect("private fixture");
}
pub fn configuration(directory: &Path, forge: &Forge<'_>) {
    assert!(Path::new("/usr/bin/git").is_file(), "/usr/bin/git is required");
    for name in ["accounts", "secrets", "state"] {
        fs::create_dir_all(directory.join(name)).expect("deployment directory");
    }
    for (name, bytes) in jig_channel_world::fixtures::files(false).entries.into_boxed() {
        if name.starts_with(b"charters/") {
            continue;
        }
        write_private(&directory.join(std::str::from_utf8(&name).expect("fixture path")), bytes.expose());
    }
    write_private(&directory.join("worker.secret"), b"worker-secret");
    write_private(&directory.join("secrets/forge-1.user"), b"42\n");
    write_private(&directory.join("secrets/forge-1.api-token"), b"forge-1-api-token-value");
    write_private(&directory.join("secrets/forge-1.git-token"), b"forge-1-git-token-value");
    write_private(&directory.join("root.der"), skein_tls_world::pki::ROOT);
    let (agent, value) =
        configuration::documents(directory, skein_scratch::paths::binary("temper").to_str().expect("a UTF-8 path"));
    write_private(&directory.join("agent.json"), &agent);
    let mut value: serde_json::Value = serde_json::from_slice(&value).expect("deployment JSON");
    forgejo::configuration(directory, forge, &mut value);
    write_private(&directory.join("temper.json"), &serde_json::to_vec(&value).expect("deployment JSON"));
}

/// A case's scratch, retained if a test panics.
pub struct Scratch {
    path: std::path::PathBuf,
}
impl Scratch {
    #[must_use]
    pub fn new(case: &str) -> Self {
        Self::at("process", case)
    }
    /// The live launcher supplies a new private TMPDIR, swept on every exit.
    #[must_use]
    pub fn live() -> Self {
        let base = std::env::var_os("TMPDIR").expect("run live through scripts/proof-live.sh");
        let path = std::path::PathBuf::from(base).join(format!("temper-live-{}", std::process::id()));
        fs::create_dir(&path).expect("fresh live scratch");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).expect("private live scratch");
        Self { path }
    }
    #[must_use]
    pub fn startup(case: &str) -> Self {
        Self::at("startup", case)
    }
    fn at(tier: &str, case: &str) -> Self {
        let path = skein_scratch::paths::target_tmp().join(tier).join(case);
        if path.exists() {
            fs::remove_dir_all(&path).expect("remove prior scratch");
        }
        fs::create_dir_all(&path).expect("scratch directory");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).expect("private scratch");
        Self { path }
    }
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn remove(self) {
        fs::remove_dir_all(self.path).expect("scratch cleanup");
    }
}

use skein_io::kernel::{Complete, Done, Exit, Fd, Op, Signal, Submit};
use skein_lib::{Duration, Queue, Time, Token, Wall};
use skein_shell::{Clock, Host};
use skein_world::{
    Referee,
    end_to_end::{Binary, Command, Expectation, Mode, Streams},
    real,
};

#[must_use]
pub fn serve(directory: &Path) -> Binary {
    Binary::start(serve_command(directory), Mode::Pipes, 65_536).expect("temper serve starts")
}
fn serve_command(directory: &Path) -> Command {
    Command {
        program: skein_scratch::paths::binary("temper"),
        arguments: std::iter::once("serve".into()).chain(configuration::flags(directory)).collect(),
        directory: directory.into(),
        environment: Vec::new(),
    }
}
#[must_use]
pub fn fake_llm(scripts: Box<[skein_fake_llm_domain::api::Script]>) -> llm::Listening {
    // The account's API key routes to /v1/responses: the real loop's recipe, on the fixture endpoint's
    // TLS port. smith's configured peer keeps the Codex path, which refuses a key.
    let recipe = temper_real_loop_world::llm::scripted(scripts);
    llm::Listening::new(
        skein_fake_peers::llm::Peer::new(
            (std::net::Ipv4Addr::LOCALHOST, 0).into(),
            skein_fake_peers::Transport::Tls,
            recipe.limits,
            recipe.config,
            recipe.credential,
            recipe.protocol_limits,
            recipe.domain,
            recipe.domain_limits,
        )
        .expect("configured independent peer"),
    )
}
#[must_use]
pub fn printed_link(stdout: &[u8]) -> Option<&[u8]> {
    let end = stdout.iter().position(|byte| *byte == b'\n')?;
    Some(&stdout[..end])
}
pub struct Ended {
    pub exit: Option<Exit>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}
const PIPE: Token = Token::new(1);
const CLOSE: Token = Token::new(2);
struct StdoutReader {
    input: Option<Fd>,
    output: Option<Fd>,
    reading: bool,
    closing: u32,
    bytes: Vec<u8>,
    completions: Queue<Complete>,
    submissions: Queue<Submit>,
}
impl StdoutReader {
    fn new(input: Fd, output: Fd) -> Self {
        Self {
            input: Some(input),
            output: Some(output),
            reading: false,
            closing: 0,
            bytes: Vec::new(),
            completions: Queue::with_capacity(8),
            submissions: Queue::with_capacity(8),
        }
    }
    fn close(&mut self, fd: Fd) {
        self.closing += 1;
        self.submissions.push(Submit { op: CLOSE, kind: Op::Close { fd } });
    }
}
impl Host for StdoutReader {
    fn iterate(&mut self, _now: Time, _wall: Wall) {
        while let Some(complete) = self.completions.pop() {
            if complete.op == CLOSE {
                self.closing -= 1;
                complete.result.expect("pipe closed");
            } else {
                assert_eq!(complete.op, PIPE, "stdout read token");
                self.reading = false;
                let Op::PipeRead { buf, .. } = complete.kind else { panic!("stdout read") };
                let Done::Count(count) = complete.result.expect("stdout read completes") else { panic!("read count") };
                self.bytes.extend_from_slice(&buf[..usize::try_from(count).expect("bounded read")]);
                assert!(self.bytes.len() <= 65_536, "bounded printed links");
                if count == 0 {
                    let fd = self.output.take().expect("stdout pipe");
                    self.close(fd);
                }
            }
        }
        if let Some(fd) = self.input.take() {
            self.close(fd);
        }
        if !self.reading
            && let Some(fd) = self.output
        {
            self.reading = true;
            self.submissions
                .push(Submit { op: PIPE, kind: Op::PipeRead { fd, buf: vec![0; 1024].into_boxed_slice() } });
        }
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        &mut self.completions
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        &mut self.submissions
    }
    fn work_pending(&self, _now: Time) -> bool {
        !self.completions.is_empty() || self.input.is_some() || (!self.reading && self.output.is_some())
    }
    fn next_deadline(&self) -> Option<Time> {
        None
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        None
    }
    fn is_empty(&self) -> bool {
        self.input.is_none() && self.output.is_none() && self.closing == 0
    }
    fn worst_case(&self) -> u64 {
        131_072
    }
    fn operations(&self) -> u32 {
        3
    }
}
enum Process {
    Calls(Box<ctl::Calls>),
    Binary(Box<Binary>),
    Stdout(Box<StdoutReader>),
    Person(Box<person::Person>),
    Peer(Box<skein_fake_peers::llm::Peer>),
    Issuer(Box<skein_fake_peers::oauth::Peer>),
    Visitor(Box<sign_in_browser::Visitor>),
}
impl Process {
    fn host(&self) -> &dyn Host {
        match self {
            Self::Binary(x) => x.as_ref(),
            Self::Calls(x) => x.as_ref(),
            Self::Stdout(x) => x.as_ref(),
            Self::Person(x) => x.as_ref(),
            Self::Peer(x) => x.as_ref(),
            Self::Issuer(x) => x.as_ref(),
            Self::Visitor(x) => x.as_ref(),
        }
    }
    fn host_mut(&mut self) -> &mut dyn Host {
        match self {
            Self::Binary(x) => x.as_mut(),
            Self::Calls(x) => x.as_mut(),
            Self::Stdout(x) => x.as_mut(),
            Self::Person(x) => x.as_mut(),
            Self::Peer(x) => x.as_mut(),
            Self::Issuer(x) => x.as_mut(),
            Self::Visitor(x) => x.as_mut(),
        }
    }
}
impl Host for Process {
    fn iterate(&mut self, now: Time, wall: Wall) {
        self.host_mut().iterate(now, wall);
    }
    fn drain(&mut self) {
        self.host_mut().drain();
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        self.host_mut().completions()
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        self.host_mut().submissions()
    }
    fn work_pending(&self, now: Time) -> bool {
        self.host().work_pending(now)
    }
    fn next_deadline(&self) -> Option<Time> {
        self.host().next_deadline()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        self.host().next_policy_deadline()
    }
    fn is_empty(&self) -> bool {
        self.host().is_empty()
    }
    fn worst_case(&self) -> u64 {
        self.host().worst_case()
    }
    fn operations(&self) -> u32 {
        self.host().operations()
    }
}
struct Judge {
    sent: bool,
    signal: Signal,
    deadline: Time,
    settled: bool,
}
impl Referee<Process> for Judge {
    fn act(&mut self, _now: Time, procs: &mut [Process]) {
        let Process::Stdout(stdout) = &procs[1] else { panic!("stdout reader") };
        if !self.sent && printed_link(&stdout.bytes).is_some() {
            let Process::Binary(binary) = &mut procs[0] else { panic!("binary") };
            binary.signal(self.signal);
            for proc in &mut *procs {
                if let Process::Peer(peer) = proc {
                    peer.shutdown();
                }
            }
            self.sent = true;
        }
    }
    fn observe(&mut self, now: Time, procs: &[Process]) {
        self.settled = self.sent && procs.iter().all(Host::is_empty);
        let Process::Binary(binary) = &procs[0] else { panic!("binary") };
        let Process::Stdout(stdout) = &procs[1] else { panic!("stdout reader") };
        assert!(
            self.sent || binary.exit_status().is_none(),
            "temper serve failed before its link: {}",
            String::from_utf8_lossy(binary.stderr())
        );
        assert!(
            now < self.deadline || self.settled,
            "signal sent: {}; stdout bytes: {}; exit: {:?}; binary empty: {}; stdout empty: {}; stderr: {}",
            self.sent,
            stdout.bytes.len(),
            binary.exit_status(),
            binary.is_empty(),
            stdout.is_empty(),
            String::from_utf8_lossy(binary.stderr())
        );
    }
    fn next_deadline(&self) -> Option<Time> {
        (!self.settled).then_some(self.deadline)
    }
    fn overdue(&self, now: Time) -> Option<String> {
        (!self.settled && now >= self.deadline)
            .then(|| "temper serve did not print its link and drain within its deadline".into())
    }
    fn passed(&self) -> bool {
        self.settled
    }
}
fn run_signal(directory: &Path, signal: Signal, deadline: Duration, peer: Option<llm::Listening>) -> Ended {
    let clock = Clock::new();
    if let Some(peer) = &peer {
        peer.configure(directory);
    }
    let mut binary = serve(directory);
    let streams = binary.take_streams().expect("binary pipes");
    let fds = streams.descriptors();
    let Streams::Pipes { input, output } = streams else { panic!("pipes mode") };
    let mut world = real::World::new(Judge {
        sent: false,
        signal,
        deadline: clock.now().now.saturating_add(Duration::from_nanos(
            deadline.as_nanos().saturating_sub(Duration::from_secs(10).as_nanos()),
        )),
        settled: false,
    });
    world.spawn_with_fds(binary.descriptors(), || Process::Binary(Box::new(binary)));
    world.spawn_with_fds(fds, || Process::Stdout(Box::new(StdoutReader::new(input, output))));
    if let Some(peer) = peer {
        peer.spawn(&mut world);
    }
    let outcome = world.run(&clock, deadline);
    assert!(outcome.procs.iter().all(Host::is_empty), "all real io settles");
    let Process::Binary(binary) = &outcome.procs[0] else { panic!("binary") };
    let Process::Stdout(stdout) = &outcome.procs[1] else { panic!("stdout reader") };
    Ended { exit: binary.exit_status(), stdout: stdout.bytes.clone(), stderr: binary.stderr().to_vec() }
}
#[must_use]
pub fn until_link_then(directory: &Path, signal: Signal, deadline: Duration) -> Ended {
    run_signal(directory, signal, deadline, None)
}

pub const ANSWER: &[u8] = b"Process answer";
pub use forgejo::landing_scripts;
pub use temper_real_loop_world::landing::EDIT;
#[must_use]
pub fn chat_script(answer: &[u8]) -> skein_fake_llm_domain::api::Script {
    use skein_fake_llm_domain::api::{Finish, Line, Script, Turn};
    let words = std::str::from_utf8(answer).expect("script answer text");
    Script {
        cue: Box::new([]),
        turns: Box::new([Turn {
            lines: Box::new([Line::Call {
                name: Box::from(*b"finish"),
                arguments: serde_json::to_vec(&serde_json::json!({"form":"report","text":words,"fields":{}}))
                    .expect("report JSON")
                    .into_boxed_slice(),
            }]),
            finish: Finish::ToolCalls,
            tokens: 20,
        }]),
    }
}
pub struct Seen {
    pub signed_in: u32,
    pub finished: bool,
    pub stderr: Vec<u8>,
}
#[derive(Clone, Copy)]
enum Ending {
    Stop,
    Box,
    Crash,
}
enum ChatPhase {
    SettingUp,
    Running,
    Stopping,
    Settled,
}
struct ChatJudge<'a> {
    phase: ChatPhase,
    ending: Ending,
    deadline: Time,
    forge: Option<&'a forgejo::Throwaway>,
    poll: Time,
    merged: bool,
}
impl ChatJudge<'_> {
    fn stopping(&self) -> bool {
        matches!(self.phase, ChatPhase::Stopping | ChatPhase::Settled)
    }
}
impl Referee<Process> for ChatJudge<'_> {
    fn act(&mut self, now: Time, procs: &mut [Process]) {
        if matches!(self.phase, ChatPhase::SettingUp) {
            let Process::Calls(calls) = &procs[2] else { panic!("native settings setup") };
            if calls.ends().len() < NATIVE_SETUP_CALLS {
                return;
            }
            for call in calls.ends() {
                assert_eq!(call.exit, Some(Exit::Code(0)), "{}", String::from_utf8_lossy(&call.stderr));
            }
            let link = calls.link().to_vec();
            let Process::Person(person) = &mut procs[1] else { panic!("native person") };
            person.ctl_ready(&link);
            self.phase = ChatPhase::Running;
        }
        if !self.stopping() && now >= self.poll {
            if let Some(forge) = self.forge {
                self.merged = forge.merged_pull().is_some();
            }
            self.poll = now.saturating_add(Duration::from_secs(2));
        }
        let Process::Person(person) = &procs[1] else {
            panic!("native person");
        };
        if !self.stopping()
            && person.finished
            && person.client.as_ref().is_some_and(Host::is_empty)
            && (self.forge.is_none() || self.merged)
        {
            let Process::Binary(binary) = &mut procs[0] else {
                panic!("binary");
            };
            match self.ending {
                Ending::Stop | Ending::Box => binary.signal(Signal::Terminate),
                Ending::Crash => binary.signal_tree(Signal::Kill),
            }
            for proc in procs {
                if let Process::Peer(peer) = proc {
                    peer.shutdown();
                }
            }
            self.phase = ChatPhase::Stopping;
        }
    }
    fn observe(&mut self, now: Time, procs: &[Process]) {
        if self.stopping() && procs.iter().all(Host::is_empty) {
            self.phase = ChatPhase::Settled;
        }
        let Process::Binary(binary) = &procs[0] else {
            panic!("binary");
        };
        let Process::Person(person) = &procs[1] else { panic!("native person") };
        let queries = procs.iter().find_map(|proc| match proc {
            Process::Peer(peer) => Some(
                peer.observations()
                    .iter()
                    .filter(|observation| matches!(observation, skein_fake_peers::llm::Observation::Query { .. }))
                    .count(),
            ),
            Process::Binary(_)
            | Process::Person(_)
            | Process::Stdout(_)
            | Process::Calls(_)
            | Process::Issuer(_)
            | Process::Visitor(_) => None,
        });
        assert!(
            person.failed.is_none(),
            "{}; fake queries: {queries:?}; forge: {}; temper serve stderr: {}",
            person.failed.as_deref().unwrap_or("person finished"),
            if person.failed.is_some() {
                self.forge.map_or_else(|| "none".into(), forgejo::Throwaway::failure_state)
            } else {
                "person running".into()
            },
            String::from_utf8_lossy(binary.stderr())
        );
        assert!(
            binary.exit_status().is_none() || self.stopping(),
            "temper serve failed: {}",
            String::from_utf8_lossy(binary.stderr())
        );
        match self.ending {
            Ending::Stop | Ending::Box => assert!(
                binary.exit_status().is_none_or(|exit| exit == Exit::Code(0)),
                "temper serve did not stop cleanly: {}",
                String::from_utf8_lossy(binary.stderr())
            ),
            Ending::Crash => assert!(!binary.stderr_overflowed(), "crashed temper serve overflowed stderr"),
        }
        assert!(
            now < self.deadline || matches!(self.phase, ChatPhase::Settled),
            "native chat did not settle: {}",
            String::from_utf8_lossy(binary.stderr())
        );
    }
    fn next_deadline(&self) -> Option<Time> {
        (!matches!(self.phase, ChatPhase::Settled)).then_some(
            if self.forge.is_some() && matches!(self.phase, ChatPhase::Running) {
                self.poll.min(self.deadline)
            } else {
                self.deadline
            },
        )
    }
    fn overdue(&self, now: Time) -> Option<String> {
        (!matches!(self.phase, ChatPhase::Settled) && now >= self.deadline)
            .then(|| "native person's chat did not finish and settle".into())
    }
    fn passed(&self) -> bool {
        matches!(self.phase, ChatPhase::Settled)
    }
}
#[must_use]
pub fn run(directory: &Path, script: person::Script, peer: Option<llm::Listening>, deadline: Duration) -> Seen {
    run_story(directory, script, peer, deadline, None, Ending::Stop)
}
/// Run the native person's script on the initialized box, including its demo setup.
#[must_use]
pub fn run_box(directory: &Path, script: person::Script, deadline: Duration) -> Seen {
    run_story(directory, script, None, deadline, None, Ending::Box)
}
/// domain/testing.md 2.6 bounds each landing story at 360 seconds.
pub const LANDING: Duration = Duration::from_secs(360);
/// Past `LANDING`: poll, `poll_max` and slow ship at 5, 30 and 40 s; stall ships at 30 min.
/// This sets the paces and stall to 600 s: above `LANDING`, but below the shipped 1800 s stall.
pub const RAISED_MS: u64 = 600_000;
const _: () = assert!(Duration::from_millis(RAISED_MS).as_nanos() > LANDING.as_nanos());
// The hinted job's 5 s sleep outlasts settle (protocol/setup.md 10, 13.4).
const HINTED_SETTLE_MS: u64 = 3000;

/// Raises the three keep paces, lowers the change's stall to 600 s, and shortens the hinted story's settle.
pub fn raise_paces(directory: &Path) {
    let path = directory.join("temper.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).expect("configuration")).expect("JSON");
    for name in
        ["forge.client.poll_ms", "forge.client.poll_max_ms", "forge.client.slow_ms", "forge.change_policy.stall_ms"]
    {
        value["application"]["limits"]["overrides"][name] = RAISED_MS.into();
    }
    value["application"]["limits"]["overrides"]["forge.client.settle_ms"] = HINTED_SETTLE_MS.into();
    write_private(&path, &serde_json::to_vec(&value).expect("configuration JSON"));
}
#[must_use]
pub fn run_until_landed(
    directory: &Path,
    script: person::Script,
    peer: llm::Listening,
    forge: &forgejo::Throwaway,
) -> Seen {
    run_story(directory, script, Some(peer), LANDING, Some(forge), Ending::Crash)
}
// Sign-in, inventory, whole template read, then one owner settings change.
const NATIVE_SETUP_CALLS: usize = 4;
fn native_settings_setup(directory: &Path) -> Vec<ctl::Step> {
    use ctl::{Call, Step};
    let file = directory.join("settings.json");
    vec![
        Step::Run(Call::sign_in().in_directory("private")),
        Step::Run(Call::new(&["settings", "--json"]).in_directory("private")),
        Step::Run(Call::new(&["setting", "template", "1", "--json"]).in_directory("private")),
        Step::Then(Box::new(move |calls| {
            // These scripts deliver their answer through finish; preserve the
            // inherited result ending before the native client makes its chat.
            ctl::setting_change_file(&file, &calls[2], |value| value["flags"]["ended_by"] = "result".into());
            ctl::settings_change(&calls[1], &file, "native-chat-settings").in_directory("private")
        })),
    ]
}
fn run_story(
    directory: &Path,
    script: person::Script,
    peer: Option<llm::Listening>,
    deadline: Duration,
    forge: Option<&forgejo::Throwaway>,
    ending: Ending,
) -> Seen {
    let clock = Clock::new();
    if let Some(peer) = &peer {
        peer.configure(directory);
    }
    let mut binary = match ending {
        Ending::Stop => serve(directory),
        Ending::Box => sign_in::start(directory),
        Ending::Crash => {
            Binary::start_with_tree(serve_command(directory), Mode::Pipes, 65_536, Expectation::Supervise, false)
                .expect("temper serve starts for a whole-tree crash")
        }
    };
    let streams = binary.take_streams().expect("binary pipes");
    let mut fds = streams.descriptors();
    let Streams::Pipes { input, output } = streams else {
        panic!("pipes mode");
    };
    let root = skein_shell::open_root(directory).expect("client's parent root");
    let setting_up = peer.is_some();
    let mut world = real::World::new(ChatJudge {
        phase: if setting_up { ChatPhase::SettingUp } else { ChatPhase::Running },
        ending,
        forge,
        poll: clock.now().now,
        merged: false,
        deadline: clock.now().now.saturating_add(Duration::from_nanos(
            deadline.as_nanos().saturating_sub(Duration::from_secs(10).as_nanos()),
        )),
    });
    world.spawn_with_fds(binary.descriptors(), || Process::Binary(Box::new(binary)));
    if setting_up {
        let ctl_root = skein_shell::open_root(directory).expect("ctl's parent root");
        fds.push(ctl_root);
        world.spawn_with_fds(vec![root], || {
            Process::Person(Box::new(person::Person::waiting_for_ctl_with_key(root, script)))
        });
        world.spawn_with_fds(fds, || {
            Process::Calls(Box::new(ctl::Calls::new(ctl_root, native_settings_setup(directory), Some(&streams), None)))
        });
    } else {
        fds.push(root);
        world.spawn_with_fds(fds, || Process::Person(Box::new(person::Person::new(root, input, output, script))));
    }
    if let Some(peer) = peer {
        peer.spawn(&mut world);
    }
    let outcome = world.run(&clock, deadline);
    assert!(outcome.procs.iter().all(Host::is_empty), "all real io settles");
    let Process::Binary(binary) = &outcome.procs[0] else {
        panic!("binary");
    };
    if matches!(ending, Ending::Crash) {
        assert!(binary.counts().is_some(), "the crashed temper serve tree settled before the restart");
    }
    let Process::Person(person) = &outcome.procs[1] else {
        panic!("native person");
    };
    Seen {
        // With settings setup, ctl signed in once and the native client loaded
        // that same session. Count the actual sign-in on either path.
        signed_in: person.client.as_ref().expect("native client").seen().signed_in + u32::from(setting_up),
        finished: person.finished,
        stderr: binary.stderr().to_vec(),
    }
}

#[must_use]
pub fn private(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o777 == 0o600)
}

/// One complete stdout line; the read finishes before the pipe changes hosts.
#[must_use]
pub fn read_line(output: Fd, within: Duration) -> Vec<u8> {
    let clock = Clock::new();
    let deadline = clock.now().now.saturating_add(within);
    let mut kernel = skein_shell::Kernel::open(skein_shell::Config { operations: 1 }).expect("link reader ring");
    let mut complete = Queue::with_capacity(1);
    let mut submit = Queue::with_capacity(1);
    let mut bytes = Vec::new();
    loop {
        submit.push(Submit { op: PIPE, kind: Op::PipeRead { fd: output, buf: vec![0; 1024].into_boxed_slice() } });
        loop {
            assert!(clock.now().now < deadline, "serve prints a link within its deadline");
            kernel.submit(&mut submit, skein_shell::Wait::Until(deadline));
            kernel.reap(&mut complete);
            if !complete.is_empty() {
                break;
            }
        }
        let complete = complete.pop().expect("read completed");
        let Op::PipeRead { buf, .. } = complete.kind else { panic!("link read") };
        let Done::Count(count) = complete.result.expect("link read succeeded") else { panic!("read count") };
        assert_ne!(count, 0, "serve ended before its link");
        bytes.extend_from_slice(&buf[..usize::try_from(count).expect("read count fits")]);
        assert!(bytes.len() <= 65_536, "bounded service output");
        if let Some(link) = printed_link(&bytes) {
            return link.to_vec();
        }
    }
}

/// Read the fake's credential-free git objects through the existing bounded HTTP helper.
#[must_use]
pub fn fake_objects(address: std::net::SocketAddr, commit: u64) -> serde_json::Value {
    github::objects(address, commit)
}
