//! QA-B process probes: client/tools.md 5.5 area 5 and 6.3; jig domain/accounts.md 6
//! and client/domain/client.md 6. The terminal composition talks to the shipped server.
use super::{Process, ctl, sign_in};
use skein_fake_peers::oauth;
use skein_io::kernel::{Exit, Signal};
use skein_lib::{Duration, Time};
use skein_shell::{Clock, Host};
use skein_world::{Referee, real};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
};

const WITHIN: Duration = Duration::from_secs(40);

fn configure(directory: &Path) -> oauth::Peer {
    let mut issuer = sign_in::configuration(directory);
    let path = directory.join("temper.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).expect("configuration")).expect("JSON");
    let (_, fixture) = super::configuration::documents(
        directory,
        skein_scratch::paths::binary("temper").to_str().expect("a UTF-8 path"),
    );
    let fixture: serde_json::Value = serde_json::from_slice(&fixture).expect("fixture JSON");
    for field in ["forges", "identities", "setup", "demo", "webhooks", "workers", "clients"] {
        value["application"][field] = fixture["application"][field].clone();
    }
    fs::create_dir_all(directory.join("secrets")).expect("fixture secrets");
    for (file, bytes) in [
        ("forge-1.user", b"42\n".as_slice()),
        ("forge-1.api-token", b"forge-1-api-token-value"),
        ("forge-1.git-token", b"forge-1-git-token-value"),
    ] {
        super::write_private(&directory.join("secrets").join(file), bytes);
    }
    super::write_private(&path, &serde_json::to_vec(&value).expect("configuration JSON"));
    // Moving the one exported record back invalidates the file's refresh token.
    issuer
        .queue(skein_fake_oauth::Plan {
            status: 200,
            body: skein_fake_oauth::Body::Token(skein_oauth::TokenResponse {
                access_token: sign_in::ACCESS.as_ref().into(),
                refresh_token: Some(b"qa-b-rotated-refresh".as_slice().into()),
                expires_in: 300,
                scope: None,
                id_token: None,
                earliest_refresh_at: None,
            }),
            delay: Duration::ZERO,
            retry_after: Duration::ZERO,
        })
        .expect("move-in refresh");
    issuer
}
fn calls(procs: &[Process]) -> &ctl::Calls {
    let Process::Calls(calls) = &procs[1] else { panic!("ctl calls") };
    calls
}
fn queue(procs: &mut [Process], call: ctl::Call) {
    let Process::Calls(calls) = &mut procs[1] else { panic!("ctl calls") };
    calls.push(ctl::Step::Run(call));
}
fn success(ended: &super::Ended) {
    assert_eq!(ended.exit, Some(Exit::Code(0)), "ctl: {}", String::from_utf8_lossy(&ended.stderr));
}
fn account(ended: &super::Ended) -> serde_json::Value {
    success(ended);
    let value = ctl::json(ended);
    let rows = value["accounts"]["party"].as_array().expect("holder's accounts");
    let [account] = rows.as_slice() else { panic!("one signed-in account") };
    assert!(
        account["sources"].as_array().expect("sources").iter().any(|source| {
            source["kind"] == "sign_in" && source["usable"] == true && source["state"]["kind"] == "granted"
        }),
        "account remains granted, not moving"
    );
    account.clone()
}

struct Probe {
    name: &'static str,
    file: PathBuf,
    bytes: Vec<u8>,
    code: u8,
    words: Vec<u8>,
    precheck: bool,
    export: bool,
}
fn probes(directory: &Path, record: &[u8]) -> Vec<Probe> {
    let mut newer = record.to_vec();
    // jig's protocol/llm.md, 5.6: jig-accounts Moved replaces JIGM at version 1.
    assert!(newer.starts_with(&[0, 1]), "real export's jig-accounts version");
    let form = u16::from_be_bytes([newer[0], newer[1]]);
    let next = form.checked_add(1).expect("next form");
    newer[0..2].copy_from_slice(&next.to_be_bytes());
    let mut probes = Vec::new();
    for (name, bytes, mode, words) in [
        ("link", record.to_vec(), 0o600, "record file must not be a link"),
        ("group", record.to_vec(), 0o640, "record file mode 0o640 permits group or other access"),
        ("other", record.to_vec(), 0o604, "record file mode 0o604 permits group or other access"),
        ("empty", vec![], 0o600, "record file must contain 1 to 131072 bytes"),
        ("over-bound", vec![b'x'; 131_073], 0o600, "record file must contain 1 to 131072 bytes"),
        // The generated decoder refuses a current version with missing fields
        // (jig's protocol/llm.md, 5.6), after the CLI's version precheck.
        ("malformed", b"\0\x01".to_vec(), 0o600, "import: not ours"),
        ("newer", newer, 0o600, ""),
    ] {
        let file = directory.join(format!("{name}.move"));
        let target = if name == "link" { directory.join("link-target.move") } else { file.clone() };
        super::write_private(&target, &bytes);
        fs::set_permissions(&target, fs::Permissions::from_mode(mode)).expect("probe permissions");
        if name == "link" {
            symlink(&target, &file).expect("probe symbolic link");
        }
        let words = if name == "newer" {
            format!("temper ctl: {} holds a moved record of form v{next}; this temper reads up to v{form}: upgrade temper\n", file.display()).into_bytes()
        } else {
            format!("{}{}\n", if name == "malformed" { "" } else { "temper ctl: " }, words).into_bytes()
        };
        probes.push(Probe {
            name,
            file,
            bytes,
            code: if name == "malformed" { 2 } else { 1 },
            words,
            precheck: name != "malformed",
            export: false,
        });
    }
    for (name, exists) in [("existing", true), ("missing-parent", false)] {
        let file = if exists { directory.join("existing.move") } else { directory.join("absent/record.move") };
        let bytes = b"existing export sentinel".to_vec();
        if exists {
            super::write_private(&file, &bytes);
        }
        let words = if exists {
            format!("temper ctl: {} exists\n", file.display()).into_bytes()
        } else {
            b"temper ctl: FILE parent: errno 2\n".to_vec()
        };
        probes.push(Probe { name, file, bytes, code: 1, words, precheck: true, export: true });
    }
    probes
}
#[derive(Debug)]
enum Phase {
    Connect,
    Account,
    Export,
    Import,
    Ready,
    Probe,
    Stop,
}
struct Files {
    directory: PathBuf,
    phase: Phase,
    deadline: Time,
    settled: bool,
    number: u64,
    held: serde_json::Value,
    probes: Vec<Probe>,
    at: usize,
    baseline: usize,
}
impl Referee<Process> for Files {
    fn act(&mut self, _now: Time, procs: &mut [Process]) {
        let ended = calls(procs).ends();
        match self.phase {
            Phase::Connect if ended.len() == 2 => {
                success(&ended[0]);
                success(&ended[1]);
                let address = sign_in::authorization_address(&ended[1]).expect("fake authorization").to_vec();
                let Process::Visitor(visitor) = &mut procs[3] else { panic!("visitor") };
                visitor.hand(&address);
                queue(procs, ctl::Call::new(&["account", "list", "--until", "settled", "--json"]));
                queue(procs, ctl::Call::new(&["account", "list", "--json"]));
                self.phase = Phase::Account;
            }
            Phase::Account if ended.len() == 4 => {
                success(&ended[2]);
                self.number = account(&ended[3])["number"].as_u64().expect("account number");
                let file = self.directory.join("valid.move");
                queue(
                    procs,
                    ctl::Call::new(&["account", "export", &self.number.to_string(), file.to_str().expect("path")]),
                );
                self.phase = Phase::Export;
            }
            Phase::Export if ended.len() == 5 => {
                success(&ended[4]);
                let file = self.directory.join("valid.move");
                let record = fs::read(&file).expect("one real export shared by all FILE probes");
                self.probes = probes(&self.directory, &record);
                queue(
                    procs,
                    ctl::Call::new(&["account", "import", file.to_str().expect("path"), "--key", "qa-b-restore"]),
                );
                self.phase = Phase::Import;
            }
            Phase::Import if ended.len() == 6 => {
                success(&ended[5]);
                assert!(!self.directory.join("valid.move").exists(), "spent export removed");
                queue(procs, ctl::Call::new(&["account", "list", "--until", "usable", "--json"]));
                queue(procs, ctl::Call::new(&["account", "list", "--json"]));
                self.phase = Phase::Ready;
            }
            Phase::Ready if ended.len() == 8 => {
                success(&ended[6]);
                self.held = account(&ended[7]);
                self.number = self.held["number"].as_u64().expect("restored account");
                self.start_probe(procs);
            }
            Phase::Probe if ended.len() == self.baseline + if self.probes[self.at].precheck { 3 } else { 2 } => {
                let probe = &self.probes[self.at];
                let count = if probe.precheck { 2 } else { 1 };
                for result in &ended[self.baseline..self.baseline + count] {
                    assert_eq!(
                        result.exit,
                        Some(Exit::Code(probe.code)),
                        "{}: {}",
                        probe.name,
                        String::from_utf8_lossy(&result.stderr)
                    );
                    assert!(result.stdout.is_empty(), "{}: no record printed", probe.name);
                    assert_eq!(result.stderr, probe.words, "{} diagnostic", probe.name);
                }
                assert_eq!(
                    account(&ended[self.baseline + count]),
                    self.held,
                    "{} leaves the holder and source unchanged",
                    probe.name
                );
                if probe.name == "missing-parent" {
                    assert!(!probe.file.exists());
                } else {
                    assert!(
                        fs::read(&probe.file).expect("refused FILE kept") == probe.bytes,
                        "{} bytes unchanged",
                        probe.name
                    );
                }
                assert!(
                    !self.directory.join("absent-client").exists(),
                    "prechecks never opened a client directory or engine connection"
                );
                eprintln!(
                    "QA-B {}: pass (exit {}, FILE kept, holder unchanged{})",
                    probe.name,
                    probe.code,
                    if probe.precheck { ", precheck before client connection" } else { "" }
                );
                self.at += 1;
                if self.at == self.probes.len() {
                    let Process::Calls(calls) = &mut procs[1] else { panic!("calls") };
                    calls.release();
                    let Process::Binary(binary) = &mut procs[0] else { panic!("serve") };
                    binary.signal(Signal::Terminate);
                    let Process::Issuer(issuer) = &mut procs[2] else { panic!("issuer") };
                    issuer.shutdown();
                    self.phase = Phase::Stop;
                } else {
                    self.start_probe(procs);
                }
            }
            Phase::Connect
            | Phase::Account
            | Phase::Export
            | Phase::Import
            | Phase::Ready
            | Phase::Probe
            | Phase::Stop => {}
        }
    }
    fn observe(&mut self, _now: Time, procs: &[Process]) {
        let Process::Binary(binary) = &procs[0] else { panic!("serve") };
        assert!(
            matches!(self.phase, Phase::Stop) || binary.exit_status().is_none(),
            "serve: {}",
            String::from_utf8_lossy(binary.stderr())
        );
        self.settled = matches!(self.phase, Phase::Stop) && procs.iter().all(Host::is_empty);
    }
    fn next_deadline(&self) -> Option<Time> {
        (!self.settled).then_some(self.deadline)
    }
    fn overdue(&self, now: Time) -> Option<String> {
        (!self.settled && now >= self.deadline)
            .then(|| format!("FILE probes stalled at {:?}, probe {}", self.phase, self.at))
    }
    fn passed(&self) -> bool {
        self.settled
    }
}
impl Files {
    fn start_probe(&mut self, procs: &mut [Process]) {
        let probe = &self.probes[self.at];
        self.baseline = calls(procs).ends().len();
        let path = probe.file.to_str().expect("probe path");
        let call = if probe.export {
            ctl::Call::new(&["account", "export", &self.number.to_string(), path])
        } else {
            ctl::Call::new(&["account", "import", path, "--key", "qa-b-refused"])
        };
        queue(procs, call.clone());
        if probe.precheck {
            queue(procs, call.in_directory("absent-client"));
        }
        queue(procs, ctl::Call::new(&["account", "list", "--json"]));
        self.phase = Phase::Probe;
    }
}
pub fn files(directory: &Path) {
    let issuer = configure(directory);
    let clock = Clock::new();
    let mut binary = super::serve(directory);
    let streams = binary.take_streams().expect("serve pipes");
    let root = skein_shell::open_root(directory).expect("deployment root");
    let mut fds = streams.descriptors();
    fds.push(root);
    let mut world = real::World::new(Files {
        directory: directory.into(),
        phase: Phase::Connect,
        deadline: clock.now().now.saturating_add(WITHIN),
        settled: false,
        number: 0,
        held: serde_json::Value::Null,
        probes: Vec::new(),
        at: 0,
        baseline: 0,
    });
    world.spawn_with_fds(binary.descriptors(), || Process::Binary(Box::new(binary)));
    world.spawn_with_fds(fds, || {
        let mut calls = ctl::Calls::new(
            root,
            vec![
                ctl::Step::Run(ctl::Call::sign_in()),
                ctl::Step::Run(ctl::Call::new(&["account", "connect", "1", "--key", "qa-b-connect"])),
            ],
            Some(&streams),
            None,
        );
        calls.hold();
        Process::Calls(Box::new(calls))
    });
    world.spawn(|| Process::Issuer(Box::new(issuer)));
    world.spawn(|| Process::Visitor(Box::default()));
    let outcome = world.run(&clock, WITHIN);
    assert!(outcome.procs.iter().all(Host::is_empty), "all process IO drained");
    let Process::Binary(binary) = &outcome.procs[0] else { panic!("serve") };
    assert_eq!(binary.exit_status(), Some(Exit::Code(0)), "healthy deployment stop");
}

// A small raw native peer holds Written until the page has observed Moving.
// v19's two records are encoded here because temper's process package intentionally
// has no direct dependency on jig-client or its wire crate. This is test driving,
// not a second implementation of the move: the real engine owns every transition.
fn move_request(number: u32, written: bool) -> Vec<u8> {
    let mut body = vec![if written { 0x72 } else { 0x71 }; 16];
    body.push(0); // Owner::Jig
    body.extend_from_slice(&19_u32.to_be_bytes());
    body.extend_from_slice(&7_u32.to_be_bytes());
    body.extend_from_slice(&[23, 0, if written { 11 } else { 10 }]); // AccountChange, Own, Written/MoveOut
    body.extend_from_slice(&number.to_be_bytes());
    body.push(0); // no carried value
    body
}
fn post(address: std::net::SocketAddr, credential: &[u8], body: &[u8]) -> Vec<u8> {
    use skein_lib::{
        Writer,
        base64::{self, Alphabet},
    };
    use std::io::{Read, Write};
    let length = base64::encoded_len(Alphabet::Url, u32::try_from(credential.len()).expect("fake token length"))
        .expect("base64 length");
    let mut encoded = Writer::new(usize::try_from(length).expect("base64 length fits"));
    base64::encode(Alphabet::Url, credential, &mut encoded).expect("fake credential encoding");
    let bearer = String::from_utf8(encoded.finish().into_vec()).expect("base64 ASCII");
    let mut socket = std::net::TcpStream::connect(address).expect("real deployment");
    let timeout = Some(std::time::Duration::from_secs(5));
    socket.set_read_timeout(timeout).expect("bounded read");
    socket.set_write_timeout(timeout).expect("bounded write");
    write!(socket, "POST /jig/v1/request HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {bearer}\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).expect("request head");
    socket.write_all(body).expect("request body");
    let mut response = Vec::new();
    socket.take(262_144).read_to_end(&mut response).expect("whole answer");
    assert!(response.starts_with(b"HTTP/1.1 200 "), "native request answered 200");
    let at = response.windows(4).position(|bytes| bytes == b"\r\n\r\n").expect("HTTP head");
    response[at + 4..].to_vec()
}
fn final_record(response: &[u8]) -> &[u8] {
    let mut reader = skein_lib::Reader::new(response);
    assert_eq!(reader.u8(), Some(1), "KeyedAnswer::FinalAnswer");
    assert_eq!(reader.u8(), Some(0), "Owner::Jig");
    assert_eq!(reader.u32(), Some(19), "current wire");
    let len = reader.u32().expect("owned record length");
    let record = reader.bytes(len).expect("whole record");
    assert!(reader.is_empty(), "whole envelope");
    record
}
fn moved_record(response: &[u8], account: u32) -> (u32, Vec<u8>) {
    let mut reader = skein_lib::Reader::new(final_record(response));
    assert_eq!(reader.u8(), Some(0), "Final::Done");
    assert_eq!(reader.u8(), Some(1), "Outcome::Made");
    assert_eq!(reader.u8(), Some(4), "Made::MovingOut");
    assert_eq!(reader.u32(), Some(account), "the requested account");
    let number = reader.u32().expect("move number");
    let length = reader.u32().expect("record length");
    let record = reader.bytes(length).expect("whole moved record");
    assert!(record.starts_with(&[0, 1]), "real moved record's jig-accounts version");
    assert!(reader.is_empty(), "whole moved answer");
    (number, record.to_vec())
}

struct Viewer {
    root: Option<skein_io::kernel::Fd>,
    client: Option<Box<temper_client_shell_terminal::client::NativeClient>>,
    page: Option<temper_client_domain::Address>,
    selected: bool,
    complete: skein_lib::Queue<skein_io::kernel::Complete>,
    submit: skein_lib::Queue<skein_io::kernel::Submit>,
}
impl Viewer {
    fn waiting(root: skein_io::kernel::Fd) -> Self {
        Self {
            root: Some(root),
            client: None,
            page: None,
            selected: false,
            complete: skein_lib::Queue::with_capacity(1),
            submit: skein_lib::Queue::with_capacity(1),
        }
    }
    fn start(&mut self, address: std::net::SocketAddr, directory: &[u8], page: temper_client_domain::Address) {
        use temper_client_shell_terminal::client::{NativeClient, Settings, Start};
        let client = NativeClient::new(
            Settings {
                engine: skein_io::Addr::Ip(address),
                directory: directory.into(),
                limits: temper_client_shell_terminal::limits::native(),
            },
            self.root.take().expect("viewer root"),
            skein_shell::effective_user(),
            if directory == b"accounts-viewer" { 81 } else { 82 },
            Start::Token,
        )
        .expect("native terminal client");
        self.page = Some(page);
        self.client = Some(Box::new(client));
    }
    fn has_account(&self) -> bool {
        self.client.as_ref().is_some_and(|client| {
            client.tree().nodes().iter().any(|node| {
                node.role == skein_ui::Role::Disclosure
                    && node.name.as_deref().is_some_and(|name| skein_lib::bytes::find(name, b"ChatGPT").is_some())
            })
        })
    }
    fn text(&self, words: &[u8]) -> bool {
        self.client.as_ref().is_some_and(|client| {
            client.tree().nodes().iter().any(|node| {
                node.name.as_deref().is_some_and(|name| skein_lib::bytes::find(name, words).is_some())
                    || node.text.iter().any(|run| skein_lib::bytes::find(&run.text, words).is_some())
            })
        })
    }
}
enum MovingProcess {
    Standard(Process),
    Viewer(Viewer),
}
impl MovingProcess {
    fn host(&self) -> &dyn Host {
        match self {
            Self::Standard(host) => host,
            Self::Viewer(viewer) => viewer,
        }
    }
    fn host_mut(&mut self) -> &mut dyn Host {
        match self {
            Self::Standard(host) => host,
            Self::Viewer(viewer) => viewer,
        }
    }
}
impl Host for MovingProcess {
    fn iterate(&mut self, now: Time, wall: skein_lib::Wall) {
        self.host_mut().iterate(now, wall);
    }
    fn drain(&mut self) {
        self.host_mut().drain();
    }
    fn completions(&mut self) -> &mut skein_lib::Queue<skein_io::kernel::Complete> {
        self.host_mut().completions()
    }
    fn submissions(&mut self) -> &mut skein_lib::Queue<skein_io::kernel::Submit> {
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
impl Host for Viewer {
    fn iterate(&mut self, now: Time, wall: skein_lib::Wall) {
        if let Some(client) = &mut self.client {
            client.iterate(now, wall);
            if client.started() && !self.selected {
                let project = client
                    .tree()
                    .nodes()
                    .iter()
                    .find(|node| node.role == skein_ui::Role::Button && node.name.as_deref() == Some(b"Project 1"))
                    .map(|node| node.id);
                if let Some(project) = project {
                    assert!(client.act(project, skein_ui::Act::Press));
                    self.selected = true;
                    if let Some(page) = self.page.take() {
                        client.go(page);
                    }
                }
            }
        }
    }
    fn drain(&mut self) {
        if let Some(client) = &mut self.client {
            client.drain();
        }
    }
    fn completions(&mut self) -> &mut skein_lib::Queue<skein_io::kernel::Complete> {
        match &mut self.client {
            Some(client) => client.completions(),
            None => &mut self.complete,
        }
    }
    fn submissions(&mut self) -> &mut skein_lib::Queue<skein_io::kernel::Submit> {
        match &mut self.client {
            Some(client) => client.submissions(),
            None => &mut self.submit,
        }
    }
    fn work_pending(&self, now: Time) -> bool {
        self.client.as_ref().is_some_and(|client| client.work_pending(now))
    }
    fn next_deadline(&self) -> Option<Time> {
        self.client.as_ref().and_then(|client| client.next_deadline())
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        self.client.as_ref().and_then(|client| client.next_policy_deadline())
    }
    fn is_empty(&self) -> bool {
        self.client.as_ref().is_some_and(|client| client.is_empty())
    }
    fn worst_case(&self) -> u64 {
        self.client.as_ref().map_or(1, |client| client.worst_case())
    }
    fn operations(&self) -> u32 {
        self.client.as_ref().map_or(512, |client| client.operations())
    }
}
fn moving_calls(procs: &[MovingProcess]) -> &ctl::Calls {
    let MovingProcess::Standard(Process::Calls(calls)) = &procs[1] else { panic!("ctl calls") };
    calls
}
fn moving_queue(procs: &mut [MovingProcess], args: &[&str]) {
    let MovingProcess::Standard(Process::Calls(calls)) = &mut procs[1] else { panic!("ctl calls") };
    calls.push(ctl::Step::Run(ctl::Call::new(args)));
}
fn viewer(procs: &[MovingProcess], at: usize) -> &Viewer {
    let MovingProcess::Viewer(viewer) = &procs[at] else { panic!("native viewer") };
    viewer
}
fn peer_reply(receive: &std::sync::mpsc::Receiver<bool>) -> Option<bool> {
    match receive.try_recv() {
        Ok(done) => Some(done),
        Err(std::sync::mpsc::TryRecvError::Empty) => None,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => panic!("the native move peer failed"),
    }
}

#[derive(Debug)]
enum MovingPhase {
    Connect,
    Account,
    Watching,
    Moving,
    Task,
    Written,
    Back,
    Gone,
    Stop,
    Drain,
}
struct Moving {
    directory: PathBuf,
    phase: MovingPhase,
    deadline: Time,
    wake: Time,
    settled: bool,
    number: u32,
    send: std::sync::mpsc::Sender<(std::net::SocketAddr, u32)>,
    written: std::sync::mpsc::Sender<()>,
    receive: std::sync::mpsc::Receiver<bool>,
    moved: bool,
}
impl Moving {
    fn watch(&mut self, procs: &mut [MovingProcess]) {
        self.number =
            u32::try_from(account(&moving_calls(procs).ends()[3])["number"].as_u64().expect("account number"))
                .expect("account fits");
        let link = moving_calls(procs).link().to_vec();
        let address = std::str::from_utf8(&link)
            .expect("link")
            .strip_prefix("http://")
            .expect("loopback")
            .split('/')
            .next()
            .expect("authority")
            .parse()
            .expect("socket address");
        let (at, directory, page) = (4, b"accounts-viewer".as_slice(), temper_client_domain::Address::Accounts);
        let path = self.directory.join(std::str::from_utf8(directory).expect("viewer directory"));
        fs::create_dir(&path).expect("viewer private directory");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).expect("private viewer directory");
        super::write_private(
            &path.join("token"),
            &fs::read(self.directory.join("client/token")).expect("synthetic sign-in token"),
        );
        let MovingProcess::Viewer(viewer) = &mut procs[at] else { panic!("viewer") };
        viewer.start(address, directory, page);
        self.phase = MovingPhase::Watching;
    }
    fn begin_move(&mut self) {
        let address =
            String::from_utf8(fs::read(self.directory.join("client/address")).expect("fake client's engine address"))
                .expect("address");
        let address = address
            .trim()
            .strip_prefix("http://")
            .unwrap_or(address.trim())
            .split('/')
            .next()
            .expect("address")
            .parse()
            .expect("socket address");
        self.send.send((address, self.number)).expect("move peer");
        self.phase = MovingPhase::Moving;
    }
    fn stop(&mut self, procs: &mut [MovingProcess]) {
        let value = ctl::json(&moving_calls(procs).ends()[5]);
        assert!(
            value["accounts"]["party"].as_array().expect("accounts").is_empty(),
            "Written erased the source account"
        );
        for proc in procs {
            match proc {
                MovingProcess::Viewer(viewer) => viewer.client.as_mut().expect("client").close(),
                MovingProcess::Standard(Process::Calls(calls)) => calls.release(),
                MovingProcess::Standard(Process::Binary(binary)) => binary.signal(Signal::Terminate),
                MovingProcess::Standard(Process::Issuer(issuer)) => issuer.shutdown(),
                MovingProcess::Standard(
                    Process::Visitor(_) | Process::Person(_) | Process::Stdout(_) | Process::Peer(_),
                ) => {}
            }
        }
        self.phase = MovingPhase::Drain;
        self.settled = true;
    }
}

impl Referee<MovingProcess> for Moving {
    fn act(&mut self, now: Time, procs: &mut [MovingProcess]) {
        self.wake = now.saturating_add(Duration::from_millis(10));
        let ended = moving_calls(procs).ends();
        match self.phase {
            MovingPhase::Connect if ended.len() == 2 => {
                for result in ended {
                    success(result);
                }
                let address = sign_in::authorization_address(&ended[1]).expect("fake authorization").to_vec();
                let MovingProcess::Standard(Process::Visitor(visitor)) = &mut procs[3] else { panic!("visitor") };
                visitor.hand(&address);
                moving_queue(procs, &["account", "list", "--until", "settled", "--json"]);
                moving_queue(procs, &["account", "list", "--json"]);
                self.phase = MovingPhase::Account;
            }
            MovingPhase::Account if ended.len() == 4 => {
                self.watch(procs);
            }
            MovingPhase::Watching
                if viewer(procs, 4).has_account()
                    && viewer(procs, 4).text(b"Ready")
                    && viewer(procs, 4).text(b"ChatGPT sign-in") =>
            {
                self.begin_move();
            }
            MovingPhase::Moving => {
                if !self.moved
                    && let Some(done) = peer_reply(&self.receive)
                {
                    assert!(!done, "moving answer before Written");
                    self.moved = true;
                }
                if self.moved && viewer(procs, 4).text(b"Moving to another deployment") {
                    assert_no_account_buttons(viewer(procs, 4));
                    moving_queue(
                        procs,
                        &[
                            "chat",
                            "start",
                            "--project",
                            "1",
                            "--model",
                            "gpt-6-luna",
                            "--key",
                            "qa-b-moving-task",
                            "Wait on the moving account",
                            "--json",
                        ],
                    );
                    self.phase = MovingPhase::Task;
                }
            }
            MovingPhase::Task if ended.len() == 5 => {
                success(&ended[4]);
                let MovingProcess::Viewer(viewer) = &mut procs[4] else { panic!("task viewer") };
                viewer.client.as_mut().expect("native task client").go(temper_client_domain::Address::Chat { task: 1 });
                self.phase = MovingPhase::Written;
            }
            MovingPhase::Written
                if viewer(procs, 4).text(b"Waiting because the account is moving to another deployment") =>
            {
                let MovingProcess::Viewer(viewer) = &mut procs[4] else { panic!("accounts viewer") };
                viewer.client.as_mut().expect("client").go(temper_client_domain::Address::Accounts);
                self.phase = MovingPhase::Back;
            }
            MovingPhase::Back if viewer(procs, 4).text(b"Moving to another deployment") => {
                assert_no_account_buttons(viewer(procs, 4));
                self.written.send(()).expect("Written only after both observations");
                self.phase = MovingPhase::Gone;
            }
            MovingPhase::Gone => {
                if self.moved
                    && let Some(done) = peer_reply(&self.receive)
                {
                    assert!(done, "Written answered");
                    self.moved = false;
                }
                if !self.moved
                    && !viewer(procs, 4).has_account()
                    && !viewer(procs, 4).text(b"Moving to another deployment")
                {
                    moving_queue(procs, &["account", "list", "--json"]);
                    self.phase = MovingPhase::Stop;
                }
            }
            MovingPhase::Stop if ended.len() == 6 => {
                self.stop(procs);
            }
            MovingPhase::Connect
            | MovingPhase::Account
            | MovingPhase::Watching
            | MovingPhase::Task
            | MovingPhase::Written
            | MovingPhase::Back
            | MovingPhase::Stop
            | MovingPhase::Drain => {}
        }
    }
    fn observe(&mut self, now: Time, procs: &[MovingProcess]) {
        let MovingProcess::Standard(Process::Binary(binary)) = &procs[0] else { panic!("serve") };
        assert!(self.settled || binary.exit_status().is_none(), "serve: {}", String::from_utf8_lossy(binary.stderr()));
        assert!(
            self.settled || now < self.deadline,
            "moving clients stalled at {:?}; accounts moving: {}; task waiting: {}",
            self.phase,
            viewer(procs, 4).text(b"Moving to another deployment"),
            viewer(procs, 4).text(b"Waiting because the account is moving to another deployment")
        );
    }
    fn next_deadline(&self) -> Option<Time> {
        Some(self.deadline.min(self.wake))
    }
    fn overdue(&self, now: Time) -> Option<String> {
        (!self.settled && now >= self.deadline).then(|| format!("moving clients stalled at {:?}", self.phase))
    }
    fn passed(&self) -> bool {
        self.settled
    }
}
fn assert_no_account_buttons(viewer: &Viewer) {
    use skein_ui::Role;
    let tree = viewer.client.as_ref().expect("native client").tree();
    let at = tree
        .nodes()
        .iter()
        .position(|node| {
            node.role == Role::Disclosure
                && node.name.as_deref().is_some_and(|name| skein_lib::bytes::find(name, b"ChatGPT").is_some())
        })
        .expect("moving account disclosure");
    let size = usize::try_from(tree.nodes()[at].size).expect("account subtree size");
    assert!(
        tree.nodes()[at..at + size].iter().all(|node| node.role != Role::Button),
        "moving account has no buttons, including unnamed controls"
    );
}

pub fn terminal(directory: &Path) {
    let issuer = configure(directory);
    std::thread::scope(|scope| {
        let (send, requests) = std::sync::mpsc::channel();
        let (written, wait_written) = std::sync::mpsc::channel();
        let (result, receive) = std::sync::mpsc::channel();
        let peer = scope.spawn(move || {
            let Ok((address, account)) = requests.recv() else { return };
            // This reads only this test's synthetic sign-in, never a user's credential.
            let credential = fs::read(directory.join("client/token")).expect("fake-only client token");
            let response = post(address, &credential, &move_request(account, false));
            let (number, record) = moved_record(&response, account);
            let file = directory.join("terminal.move");
            super::write_private(&file, &record);
            fs::File::open(&file).expect("moved file").sync_all().expect("moved file synced");
            fs::File::open(directory).expect("moved directory").sync_all().expect("directory synced");
            result.send(false).expect("moving observation");
            if wait_written.recv().is_err() {
                return;
            }
            let answer = post(address, &credential, &move_request(number, true));
            assert!(final_record(&answer).starts_with(&[0]), "Written done");
            result.send(true).expect("written observation");
        });
        let clock = Clock::new();
        let mut binary = super::serve(directory);
        let streams = binary.take_streams().expect("serve pipes");
        let root = skein_shell::open_root(directory).expect("deployment root");
        let mut fds = streams.descriptors();
        fds.push(root);
        let mut world = real::World::new(Moving {
            directory: directory.into(),
            phase: MovingPhase::Connect,
            deadline: clock.now().now.saturating_add(WITHIN),
            wake: clock.now().now,
            settled: false,
            number: 0,
            send,
            written,
            receive,
            moved: false,
        });
        world.spawn_with_fds(binary.descriptors(), || MovingProcess::Standard(Process::Binary(Box::new(binary))));
        world.spawn_with_fds(fds, || {
            let mut calls = ctl::Calls::new(
                root,
                vec![
                    ctl::Step::Run(ctl::Call::sign_in()),
                    ctl::Step::Run(ctl::Call::new(&["account", "connect", "1", "--key", "qa-b-connect"])),
                ],
                Some(&streams),
                None,
            );
            calls.hold();
            MovingProcess::Standard(Process::Calls(Box::new(calls)))
        });
        world.spawn(|| MovingProcess::Standard(Process::Issuer(Box::new(issuer))));
        world.spawn(|| MovingProcess::Standard(Process::Visitor(Box::default())));
        let root = skein_shell::open_root(directory).expect("viewer parent root");
        world.spawn_with_fds(vec![root], || MovingProcess::Viewer(Viewer::waiting(root)));
        let outcome = world.run(&clock, WITHIN);
        assert!(outcome.procs.iter().all(Host::is_empty), "all native clients and process IO drained");
        let MovingProcess::Standard(Process::Binary(binary)) = &outcome.procs[0] else { panic!("serve") };
        assert_eq!(binary.exit_status(), Some(Exit::Code(0)), "healthy stop after the move");
        peer.join().expect("move peer");
    });
}
