//! Process probes on fake records. The relay holds complete exchanges, never logs their bytes.
#![expect(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    reason = "Process-tier fault injection uses independent real sockets and joined issuer/relay threads, with wall-time bounds; engine code remains in its world."
)]
use super::{Process, Scratch, configuration, sign_in};
use skein_fake_peers::oauth;
use skein_lib::{Duration, Time};
use skein_shell::{Clock, Host};
use skein_world::{Referee, real};
use std::{
    fs,
    io::{self, BufRead, BufReader, Read, Write},
    net::{Shutdown, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration as WallDuration, Instant},
};

const WAIT: WallDuration = WallDuration::from_secs(5);
pub const DEADLINE_MS: u64 = 500;

fn document(directory: &Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(directory.join("temper.json")).expect("configuration")).expect("JSON")
}
fn save(directory: &Path, value: &serde_json::Value) {
    super::write_private(&directory.join("temper.json"), &serde_json::to_vec(value).expect("configuration JSON"));
}

// Reuse the landed story's init, issuer and unreachable-forge helpers. Its
// private configure function cannot be called from this sibling module.
fn configure(directory: &Path) -> oauth::Peer {
    fs::create_dir(directory).expect("deployment");
    let issuer = sign_in::configuration(directory);
    let mut value = document(directory);
    let (_, fixture) =
        configuration::documents(directory, skein_scratch::paths::binary("temper").to_str().expect("a UTF-8 path"));
    let fixture: serde_json::Value = serde_json::from_slice(&fixture).expect("fixture JSON");
    for field in ["forges", "identities", "setup", "demo", "webhooks", "workers", "clients"] {
        value["application"][field] = fixture["application"][field].clone();
    }
    value["engine"]["limits"]["overrides"]["accounts.move_deadline_ms"] = DEADLINE_MS.into();
    fs::create_dir_all(directory.join("secrets")).expect("fixture secrets");
    for (file, bytes) in [
        ("forge-1.user", b"42\n".as_slice()),
        ("forge-1.api-token", b"forge-1-api-token-value"),
        ("forge-1.git-token", b"forge-1-git-token-value"),
    ] {
        super::write_private(&directory.join("secrets").join(file), bytes);
    }
    save(directory, &value);
    issuer
}

struct PeerJudge {
    stop: mpsc::Receiver<()>,
    stopping: bool,
    wake: Time,
}
impl Referee<Process> for PeerJudge {
    fn act(&mut self, now: Time, procs: &mut [Process]) {
        self.wake = now.saturating_add(Duration::from_millis(10));
        if !self.stopping && !matches!(self.stop.try_recv(), Err(mpsc::TryRecvError::Empty)) {
            let [Process::Issuer(issuer)] = procs else { panic!("issuer") };
            issuer.shutdown();
            self.stopping = true;
        }
    }
    fn observe(&mut self, _: Time, _: &[Process]) {}
    fn next_deadline(&self) -> Option<Time> {
        Some(self.wake)
    }
    fn overdue(&self, _: Time) -> Option<String> {
        None
    }
    fn passed(&self) -> bool {
        self.stopping
    }
}

struct Issuer {
    stop: Option<mpsc::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}
impl Issuer {
    fn start(mut peer: oauth::Peer) -> Self {
        for token in [b"qa-rotation-one".as_slice(), b"qa-rotation-two"] {
            peer.queue(skein_fake_oauth::Plan {
                status: 200,
                body: skein_fake_oauth::Body::Token(skein_oauth::TokenResponse {
                    access_token: sign_in::ACCESS.as_ref().into(),
                    refresh_token: Some(token.into()),
                    expires_in: 300,
                    scope: None,
                    id_token: None,
                    earliest_refresh_at: None,
                }),
                delay: Duration::ZERO,
                retry_after: Duration::ZERO,
            })
            .expect("rotation plan");
        }
        let (stop, receiver) = mpsc::channel();
        let thread = thread::spawn(move || {
            let clock = Clock::new();
            let mut world = real::World::new(PeerJudge { stop: receiver, stopping: false, wake: clock.now().now });
            world.spawn(|| Process::Issuer(Box::new(peer)));
            let outcome = world.run(&clock, Duration::from_secs(30));
            assert!(outcome.procs.iter().all(Host::is_empty), "issuer drains");
        });
        Self { stop: Some(stop), thread: Some(thread) }
    }
}
impl Drop for Issuer {
    fn drop(&mut self) {
        drop(self.stop.take());
        if let Some(thread) = self.thread.take() {
            let result = thread.join();
            if !thread::panicking() {
                result.expect("issuer thread");
            }
        }
    }
}

pub struct Engine {
    child: Child,
    client: PathBuf,
    pub directory: PathBuf,
    pub link: String,
    pub address: SocketAddr,
}
impl Engine {
    fn start(directory: &Path) -> Self {
        Self::start_as(directory, "client")
    }
    fn start_as(directory: &Path, client: &str) -> Self {
        let log = directory.join("qa-serve.log");
        super::write_private(&log, b"");
        let child = Command::new(skein_scratch::paths::binary("temper"))
            .arg("serve")
            .args(configuration::flags(directory))
            .current_dir(directory)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(fs::OpenOptions::new().append(true).open(log).expect("private log"))
            .spawn()
            .expect("serve starts");
        let mut engine = Self {
            child,
            client: directory.join(client),
            directory: directory.into(),
            link: String::new(),
            address: "127.0.0.1:0".parse().expect("socket"),
        };
        let stdout = engine.child.stdout.take().expect("serve stdout");
        let (sent, received) = mpsc::channel();
        thread::spawn(move || {
            let mut line = String::new();
            BufReader::new(stdout).read_line(&mut line).expect("serve link");
            let _ = sent.send(line);
        });
        engine.link = received.recv_timeout(WAIT).expect("serve prints its link").trim().into();
        assert!(
            !engine.link.is_empty(),
            "serve startup: {}",
            fs::read_to_string(directory.join("qa-serve.log")).expect("log")
        );
        engine.address = engine
            .link
            .strip_prefix("http://")
            .expect("loopback link")
            .split('/')
            .next()
            .expect("authority")
            .parse()
            .expect("engine address");
        success(&engine.call(&["sign-in", "-"], Some(engine.link.as_bytes())));
        engine
    }
    #[must_use]
    pub fn spawn(&self, arguments: &[&str]) -> Running {
        self.spawn_input(arguments, None)
    }
    fn spawn_input(&self, arguments: &[&str], input: Option<&[u8]>) -> Running {
        let mut child = Command::new(skein_scratch::paths::binary("temper"))
            .arg("ctl")
            .arg(&self.client)
            .args(arguments)
            .current_dir(&self.directory)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("ctl starts");
        if let Some(input) = input {
            let mut stdin = child.stdin.take().expect("stdin");
            stdin.write_all(input).expect("fake link sent");
            stdin.write_all(b"\n").expect("link line");
        } else {
            drop(child.stdin.take());
        }
        Running { child: Some(child), tool: arguments.first().expect("tool").to_string() }
    }
    #[must_use]
    pub fn call(&self, arguments: &[&str], input: Option<&[u8]>) -> Output {
        self.spawn_input(arguments, input).finish()
    }
    #[must_use]
    pub fn accounts(&self) -> serde_json::Value {
        let output = self.call(&["accounts", "--json"], None);
        success(&output);
        serde_json::from_slice(&output.stdout).expect("accounts JSON")
    }
    #[must_use]
    pub fn account_state(&self, expected: &str) -> serde_json::Value {
        let began = Instant::now();
        loop {
            let value = self.accounts();
            let rows = value["accounts"]["party"].as_array().expect("party accounts");
            if let [account] = rows.as_slice()
                && account["sources"]
                    .as_array()
                    .expect("sources")
                    .iter()
                    .any(|source| source["kind"] == "sign_in" && source["state"]["kind"] == expected)
            {
                return account.clone();
            }
            assert!(began.elapsed() < WAIT, "source never became {expected}: {value}");
            thread::sleep(WallDuration::from_millis(10));
        }
    }
    pub fn empty(&self) {
        assert!(self.accounts()["accounts"]["party"].as_array().expect("accounts").is_empty(), "source holds nothing");
    }
    pub fn direct(&self) {
        super::write_private(&self.client.join("address"), format!("http://{}\n", self.address).as_bytes());
    }
    #[must_use]
    pub fn relay(&self) -> Relay {
        let relay = Relay::start(self.address);
        super::write_private(&self.client.join("address"), format!("http://{}\n", relay.address).as_bytes());
        let _ = self.accounts();
        relay
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub struct Running {
    child: Option<Child>,
    tool: String,
}
impl Running {
    pub fn terminate(&self) {
        let status = Command::new("/usr/bin/kill")
            .args(["-TERM", &self.child.as_ref().expect("ctl").id().to_string()])
            .env_clear()
            .status()
            .expect("signal ctl");
        assert!(status.success());
    }
    #[must_use]
    pub fn finish(mut self) -> Output {
        let began = Instant::now();
        while self.child.as_mut().expect("ctl").try_wait().expect("poll ctl").is_none() {
            assert!(began.elapsed() < WAIT, "ctl {} exceeded its probe deadline", self.tool);
            thread::sleep(WallDuration::from_millis(2));
        }
        self.child.take().expect("ctl").wait_with_output().expect("ctl output")
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub struct Fixture {
    pub a: Option<Engine>,
    pub b: Engine,
    pub file: PathBuf,
    _issuer: Issuer,
    scratch: Scratch,
}
impl Fixture {
    #[must_use]
    pub fn new(name: &str) -> Self {
        let scratch = Scratch::new(&format!("{name}-{}", std::process::id()));
        let a = scratch.path().join("a");
        let b = scratch.path().join("b");
        let issuer = configure(&a);
        drop(configure(&b));
        let mut value = document(&b);
        value["engine"]["llm"]["providers"][0]["sources"]["sign_in"] =
            document(&a)["engine"]["llm"]["providers"][0]["sources"]["sign_in"].clone();
        save(&b, &value);
        let issuer = Issuer::start(issuer);
        let a = Engine::start(&a);
        let b = Engine::start(&b);
        let connected = a.call(&["connect", "1", "--key", "qa-connect"], None);
        success(&connected);
        let ended = super::Ended {
            exit: Some(skein_io::kernel::Exit::Code(0)),
            stdout: connected.stdout,
            stderr: connected.stderr,
        };
        visit(sign_in::authorization_address(&ended).expect("authorization address"));
        let _ = a.account_state("granted");
        Self { a: Some(a), b, file: scratch.path().join("account.move"), _issuer: issuer, scratch }
    }
    #[must_use]
    pub fn a(&self) -> &Engine {
        self.a.as_ref().expect("A runs")
    }
    #[must_use]
    pub fn number(&self) -> String {
        self.a().account_state("granted")["number"].as_u64().expect("account number").to_string()
    }
    #[must_use]
    pub fn path(&self) -> &str {
        self.file.to_str().expect("file path")
    }
    #[must_use]
    pub fn copy(&self, name: &str) -> PathBuf {
        let path = self.scratch.path().join(name);
        super::write_private(&path, &fs::read(&self.file).expect("fake moved file"));
        path
    }
    pub fn restart_a(&mut self) {
        let directory = self.a().directory.clone();
        drop(self.a.take());
        self.a = Some(Engine::start(&directory));
    }
    pub fn another_holder_on_b(&mut self) {
        self.b.child.kill().expect("stop B before changing its printed-link identity");
        self.b.child.wait().expect("B stopped");
        let directory = self.b.directory.clone();
        let mut value = document(&directory);
        value["engine"]["owners"][0]["subject"] = "qa-other-holder".into();
        save(&directory, &value);
        self.b = Engine::start_as(&directory, "other-client");
        self.b.empty();
    }
}

struct VisitJudge;
impl Referee<Process> for VisitJudge {
    fn act(&mut self, _: Time, _: &mut [Process]) {}
    fn observe(&mut self, _: Time, _: &[Process]) {}
    fn next_deadline(&self) -> Option<Time> {
        None
    }
    fn overdue(&self, _: Time) -> Option<String> {
        None
    }
    fn passed(&self) -> bool {
        true
    }
}
fn visit(address: &[u8]) {
    let mut visitor = super::sign_in_browser::Visitor::new();
    visitor.hand(address);
    let clock = Clock::new();
    let mut world = real::World::new(VisitJudge);
    world.spawn(|| Process::Visitor(Box::new(visitor)));
    let outcome = world.run(&clock, Duration::from_secs(5));
    let [Process::Visitor(visitor)] = outcome.procs.as_slice() else { panic!("browser") };
    assert_eq!(visitor.status(), Some(302), "issuer redirects the fake browser");
}

pub fn success(output: &Output) {
    assert!(output.status.success(), "ctl {:?}: {}", output.status.code(), String::from_utf8_lossy(&output.stderr));
}
pub fn refused(output: &Output, why: &str) {
    assert_eq!(output.status.code(), Some(2), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8_lossy(&output.stderr), format!("export: {why}\n"));
}

/// Leave the real CLI's export durably moving, with Written held off the engine.
/// Reuse the QA relay: no moved record or client credential is inspected.
#[must_use]
pub fn export_pending(directory: &Path, upstream: SocketAddr, account: u32, file: &Path) -> serde_json::Value {
    let client = directory.join("client");
    let relay = Relay::start(upstream);
    super::write_private(&client.join("address"), format!("http://{}\n", relay.address).as_bytes());
    let gate = relay.arm(2, Boundary::BeforeRequest);
    let child = Command::new(skein_scratch::paths::binary("temper"))
        .arg("ctl")
        .arg(&client)
        .arg("export")
        .arg(account.to_string())
        .arg(file)
        .current_dir(directory)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("pending fixture export");
    let export = Running { child: Some(child), tool: "export".into() };
    gate.wait();
    assert!(fs::metadata(file).expect("synced fixture move file").len() > 0, "record reached the CLI before Written");
    export.terminate();
    assert_eq!(export.finish().status.code(), Some(143), "stop the CLI while its Written is held");
    gate.release(false);
    super::write_private(&client.join("address"), format!("http://{upstream}\n").as_bytes());
    drop(relay);
    let output = Command::new(skein_scratch::paths::binary("temper"))
        .arg("ctl")
        .arg(&client)
        .args(["accounts", "--json"])
        .current_dir(directory)
        .env_clear()
        .output()
        .expect("pending account read");
    success(&output);
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("pending accounts JSON");
    let rows = value["accounts"]["party"].as_array().expect("fixture accounts");
    let row = rows.iter().find(|row| row["number"] == account).expect("export keeps its account until Written");
    assert!(
        row["sources"]
            .as_array()
            .expect("fixture sources")
            .iter()
            .any(|source| source["kind"] == "sign_in" && source["state"]["kind"] == "moving"),
        "the source is held moving before SIGTERM: {row}"
    );
    value
}

#[derive(Clone, Copy)]
pub enum Boundary {
    BeforeRequest,
    AfterReply,
}
struct Rule {
    left: usize,
    boundary: Boundary,
    reached: mpsc::Sender<()>,
    release: mpsc::Receiver<bool>,
}
pub struct Gate {
    reached: mpsc::Receiver<()>,
    release: Option<mpsc::Sender<bool>>,
}
impl Gate {
    pub fn wait(&self) {
        self.reached.recv_timeout(WAIT).expect("relay reaches the armed request");
    }
    pub fn release(mut self, deliver: bool) {
        let _ = self.release.take().expect("gate sender").send(deliver);
    }
}
impl Drop for Gate {
    fn drop(&mut self) {
        if let Some(sender) = self.release.take() {
            let _ = sender.send(false);
        }
    }
}
pub struct Relay {
    address: SocketAddr,
    rule: Arc<Mutex<Option<Rule>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl Relay {
    fn start(upstream: SocketAddr) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("relay listener");
        listener.set_nonblocking(true).expect("nonblocking relay");
        let address = listener.local_addr().expect("relay address");
        let rule = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let thread_rule = Arc::clone(&rule);
        let thread_stop = Arc::clone(&stop);
        let thread = thread::spawn(move || {
            let mut handlers = Vec::new();
            while !thread_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((socket, _)) => {
                        let rule = Arc::clone(&thread_rule);
                        let stop = Arc::clone(&thread_stop);
                        handlers.push(thread::spawn(move || {
                            if let Err(error) = relay(socket, upstream, &rule, &stop) {
                                eprintln!("QA relay connection ended: {error}");
                            }
                        }));
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(WallDuration::from_millis(1));
                    }
                    Err(error) => panic!("relay accept: {error}"),
                }
            }
            for handler in handlers {
                handler.join().expect("relay handler");
            }
        });
        Self { address, rule, stop, thread: Some(thread) }
    }
    /// No other keyed acts run while armed: export sends `move_out`, then `written`.
    #[must_use]
    pub fn arm(&self, request: usize, boundary: Boundary) -> Gate {
        let (reached, reached_receiver) = mpsc::channel();
        let (release, release_receiver) = mpsc::channel();
        *self.rule.lock().expect("relay rule") =
            Some(Rule { left: request, boundary, reached, release: release_receiver });
        Gate { reached: reached_receiver, release: Some(release) }
    }
}
impl Drop for Relay {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.rule.lock().expect("relay rule").take();
        self.thread.take().expect("relay thread").join().expect("relay drains");
    }
}

fn head(socket: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        socket.read_exact(&mut byte)?;
        bytes.push(byte[0]);
        assert!(bytes.len() <= 65_536, "bounded HTTP head");
    }
    Ok(bytes)
}
fn length(head: &[u8]) -> usize {
    std::str::from_utf8(head)
        .expect("HTTP head")
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length").then(|| value.trim().parse().expect("content length"))
        })
        .unwrap_or(0)
}
fn gate(rule: &Rule) -> bool {
    let _ = rule.reached.send(());
    rule.release.recv_timeout(WAIT).unwrap_or(false)
}
fn relay(
    mut downstream: TcpStream,
    upstream: SocketAddr,
    rules: &Mutex<Option<Rule>>,
    stop: &AtomicBool,
) -> io::Result<()> {
    downstream.set_read_timeout(Some(WAIT))?;
    downstream.set_write_timeout(Some(WAIT))?;
    let request = head(&mut downstream)?;
    let mut body = vec![0; length(&request)];
    assert!(body.len() <= 262_144, "bounded request");
    downstream.read_exact(&mut body)?;
    let text = std::str::from_utf8(&request).expect("request head");
    let rule = if text.lines().next().expect("request line").contains("/v1/request ") {
        let mut rules = rules.lock().expect("relay rule");
        if let Some(rule) = rules.as_mut() {
            rule.left -= 1;
            if rule.left == 0 { rules.take() } else { None }
        } else {
            None
        }
    } else {
        None
    };
    if let Some(rule) = &rule
        && matches!(rule.boundary, Boundary::BeforeRequest)
        && !gate(rule)
    {
        return Ok(());
    }
    let mut server = TcpStream::connect_timeout(&upstream, WAIT)?;
    server.set_read_timeout(Some(WallDuration::from_millis(50)))?;
    server.set_write_timeout(Some(WAIT))?;
    let mut forwarded = String::new();
    for line in text.lines() {
        if line.is_empty() || line.to_ascii_lowercase().starts_with("connection:") {
            continue;
        }
        if line.to_ascii_lowercase().starts_with("host:") {
            forwarded.push_str("Host: ");
            forwarded.push_str(&upstream.to_string());
            forwarded.push_str("\r\n");
        } else {
            forwarded.push_str(line);
            forwarded.push_str("\r\n");
        }
    }
    forwarded.push_str("Connection: close\r\n\r\n");
    server.write_all(forwarded.as_bytes())?;
    server.write_all(&body)?;
    if let Some(rule) = &rule
        && matches!(rule.boundary, Boundary::AfterReply)
    {
        server.set_read_timeout(Some(WAIT))?;
        let mut response = head(&mut server)?;
        let size = length(&response);
        assert!(size <= 262_144, "bounded reply");
        let offset = response.len();
        response.resize(offset + size, 0);
        server.read_exact(&mut response[offset..])?;
        if gate(rule) {
            downstream.write_all(&response)?;
        }
    } else {
        let mut buffer = [0; 8192];
        while !stop.load(Ordering::Relaxed) {
            match server.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => downstream.write_all(&buffer[..count])?,
                Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => {}
                Err(error) => return Err(error),
            }
        }
    }
    let _ = downstream.shutdown(Shutdown::Both);
    Ok(())
}
