//! client/tools.md, 11.4: ctl moves an account between two shipped deployments.
use super::{Process, ctl, sign_in};
use skein_fake_peers::{llm, oauth};
use skein_io::kernel::{Exit, Signal};
use skein_lib::{Duration, Time};
use skein_shell::{Clock, Host};
use skein_world::{Referee, real};
use std::{
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::mpsc,
};

const WITHIN: Duration = Duration::from_secs(20);
const FILE_REFRESH: &[u8] = b"fake-refresh";
const ROTATED_REFRESH: &[u8] = b"moved-refresh";
const A: usize = 0;
const B: usize = 2;
const VISITOR: usize = 4;

fn document(directory: &Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(directory.join("temper.json")).expect("configuration")).expect("JSON")
}

fn configure(directory: &Path, llm: SocketAddr) -> oauth::Peer {
    let issuer = sign_in::configuration(directory);
    let mut value = document(directory);
    value["engine"]["llm"]["endpoints"][0]["address"] = serde_json::json!({"socket": llm.to_string()});
    let agent_path = directory.join("agent.json");
    let mut agent: serde_json::Value = serde_json::from_slice(&fs::read(&agent_path).expect("agent")).expect("JSON");
    agent["endpoints"][0]["address"] = llm.to_string().into();
    super::write_private(&agent_path, &serde_json::to_vec(&agent).expect("agent JSON"));
    // Keep init's LLM registration, charters and worker, with the existing
    // unreachable-forge fixture: this story needs serve alone, not box's demo.
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
    super::write_private(&directory.join("temper.json"), &serde_json::to_vec(&value).expect("configuration JSON"));
    issuer
}

fn calls(procs: &[Process], deployment: usize) -> &ctl::Calls {
    let Process::Calls(calls) = &procs[deployment + 1] else { panic!("ctl calls") };
    calls
}
fn queue(procs: &mut [Process], deployment: usize, arguments: &[&str]) {
    let Process::Calls(calls) = &mut procs[deployment + 1] else { panic!("ctl calls") };
    calls.push(ctl::Step::Run(ctl::Call::new(arguments)));
}
fn success(ended: &super::Ended) {
    assert_eq!(ended.exit, Some(Exit::Code(0)), "ctl: {}", String::from_utf8_lossy(&ended.stderr));
}
fn account(ended: &super::Ended) -> serde_json::Value {
    let value = ctl::json(ended);
    let rows = value["accounts"]["party"].as_array().expect("holder's accounts");
    let [account] = rows.as_slice() else { panic!("one signed-in account") };
    assert_eq!(account["provider"], 1);
    assert_eq!(account["plan"], true);
    assert!(
        account["sources"].as_array().expect("sources").iter().any(|source| {
            source["kind"] == "sign_in" && source["usable"] == true && source["state"]["kind"] == "granted"
        }),
        "usable signed-in source"
    );
    account.clone()
}

#[derive(Debug)]
enum Phase {
    Connect,
    Account,
    Export,
    Withdrawn,
    Import,
    Empty,
    Stop,
}
struct Move {
    phase: Phase,
    file: PathBuf,
    record: Vec<u8>,
    number: u64,
    moved: u64,
    deadline: Time,
    settled: bool,
    result: mpsc::Sender<(u64, Vec<u8>)>,
}
impl Referee<Process> for Move {
    fn act(&mut self, _now: Time, procs: &mut [Process]) {
        for deployment in [A, B] {
            for ended in calls(procs, deployment).ends() {
                success(ended);
            }
        }
        match self.phase {
            Phase::Connect if calls(procs, A).ends().len() == 2 => {
                let address = sign_in::authorization_address(&calls(procs, A).ends()[1])
                    .expect("connect's authorization address")
                    .to_vec();
                let Process::Visitor(visitor) = &mut procs[VISITOR] else { panic!("browser") };
                visitor.hand(&address);
                queue(procs, A, &["account", "list", "--until", "settled", "--json"]);
                queue(procs, A, &["account", "list", "--json"]);
                self.phase = Phase::Account;
            }
            Phase::Account if calls(procs, A).ends().len() == 4 => {
                self.number = account(&calls(procs, A).ends()[3])["number"].as_u64().expect("account number");
                assert!(!self.file.exists(), "export creates a new file");
                queue(
                    procs,
                    A,
                    &["account", "export", &self.number.to_string(), self.file.to_str().expect("file path")],
                );
                self.phase = Phase::Export;
            }
            Phase::Export if calls(procs, A).ends().len() == 5 => {
                assert_eq!(
                    calls(procs, A).ends()[4].stdout,
                    format!("moved out: account {} to {}\n", self.number, self.file.display()).as_bytes()
                );
                assert_eq!(fs::metadata(&self.file).expect("exported file").permissions().mode() & 0o777, 0o600);
                self.record = fs::read(&self.file).expect("keep file bytes only for the stale-token check");
                queue(procs, A, &["account", "list", "--json"]);
                self.phase = Phase::Withdrawn;
            }
            Phase::Withdrawn if calls(procs, A).ends().len() == 6 => {
                let empty = ctl::json(&calls(procs, A).ends()[5]);
                assert!(
                    empty["accounts"]["party"].as_array().expect("A's accounts").is_empty(),
                    "A holds nothing before import"
                );
                let Process::Calls(b) = &mut procs[B + 1] else { panic!("B's calls") };
                b.push(ctl::Step::Run(ctl::Call::sign_in()));
                queue(procs, B, &["account", "import", self.file.to_str().expect("file path"), "--key", "move-in"]);
                self.phase = Phase::Import;
            }
            Phase::Import if calls(procs, B).ends().len() == 2 => {
                let output = std::str::from_utf8(&calls(procs, B).ends()[1].stdout).expect("import output");
                self.moved = output
                    .strip_prefix("moved in: account ")
                    .expect("import names its account")
                    .strip_suffix('\n')
                    .expect("one output line")
                    .parse()
                    .expect("B's account number");
                assert!(!self.file.exists(), "import removes the file");
                queue(procs, B, &["account", "list", "--until", "settled", "--json"]);
                queue(procs, B, &["account", "list", "--json"]);
                self.phase = Phase::Empty;
            }
            Phase::Empty if calls(procs, A).ends().len() == 6 && calls(procs, B).ends().len() == 4 => {
                assert_eq!(account(&calls(procs, B).ends()[3])["number"], self.moved, "B holds the imported account");
                let empty = ctl::json(&calls(procs, A).ends()[5]);
                assert!(empty["accounts"]["party"].as_array().expect("A's accounts").is_empty(), "A holds nothing");
                self.result.send((self.moved, std::mem::take(&mut self.record))).expect("move result");
                for deployment in [A, B] {
                    let Process::Calls(calls) = &mut procs[deployment + 1] else { panic!("ctl calls") };
                    calls.release();
                    let Process::Binary(binary) = &mut procs[deployment] else { panic!("serve") };
                    binary.signal(Signal::Terminate);
                }
                self.phase = Phase::Stop;
            }
            Phase::Connect
            | Phase::Account
            | Phase::Export
            | Phase::Withdrawn
            | Phase::Import
            | Phase::Empty
            | Phase::Stop => {}
        }
    }
    fn observe(&mut self, _now: Time, procs: &[Process]) {
        for deployment in [A, B] {
            let Process::Binary(binary) = &procs[deployment] else { panic!("serve") };
            assert!(
                matches!(self.phase, Phase::Stop) || binary.exit_status().is_none(),
                "serve failed: {}",
                String::from_utf8_lossy(binary.stderr())
            );
        }
        self.settled = matches!(self.phase, Phase::Stop) && procs.iter().all(Host::is_empty);
    }
    fn next_deadline(&self) -> Option<Time> {
        (!self.settled).then_some(self.deadline)
    }
    fn overdue(&self, now: Time) -> Option<String> {
        (!self.settled && now >= self.deadline).then(|| format!("account move stalled at {:?}", self.phase))
    }
    fn passed(&self) -> bool {
        self.settled
    }
}

fn move_account(a: &Path, b: &Path, file: &Path) -> (u64, Vec<u8>) {
    let clock = Clock::new();
    let (result, received) = mpsc::channel();
    let mut world = real::World::new(Move {
        phase: Phase::Connect,
        file: file.into(),
        record: Vec::new(),
        number: 0,
        moved: 0,
        deadline: clock.now().now.saturating_add(WITHIN),
        settled: false,
        result,
    });
    for (directory, steps) in [
        (
            a,
            vec![
                ctl::Step::Run(ctl::Call::sign_in()),
                ctl::Step::Run(ctl::Call::new(&["account", "connect", "1", "--key", "connect-move"])),
            ],
        ),
        (b, Vec::new()),
    ] {
        let mut binary = super::serve(directory);
        let streams = binary.take_streams().expect("serve pipes");
        let root = skein_shell::open_root(directory).expect("deployment root");
        let mut fds = streams.descriptors();
        fds.push(root);
        world.spawn_with_fds(binary.descriptors(), || Process::Binary(Box::new(binary)));
        world.spawn_with_fds(fds, || {
            let mut calls = ctl::Calls::new(root, steps, Some(&streams), None);
            calls.hold();
            Process::Calls(Box::new(calls))
        });
    }
    world.spawn(|| Process::Visitor(Box::default()));
    let outcome = world.run(&clock, WITHIN);
    for deployment in [A, B] {
        let Process::Binary(binary) = &outcome.procs[deployment] else { panic!("serve") };
        assert_eq!(binary.exit_status(), Some(Exit::Code(0)), "healthy stop before B's restart");
    }
    assert!(outcome.procs.iter().all(Host::is_empty), "both deployments drained");
    received.recv().expect("completed move")
}

struct Peers {
    stop: mpsc::Receiver<()>,
    ended: bool,
    deadline: Time,
    wake: Time,
}
impl Referee<Process> for Peers {
    fn act(&mut self, now: Time, procs: &mut [Process]) {
        self.wake = now.saturating_add(Duration::from_millis(10));
        if !self.ended && !matches!(self.stop.try_recv(), Err(mpsc::TryRecvError::Empty)) {
            let [Process::Issuer(issuer), Process::Peer(peer)] = procs else { panic!("independent peers") };
            issuer.shutdown();
            peer.shutdown();
            self.ended = true;
        }
    }
    fn observe(&mut self, _now: Time, _procs: &[Process]) {}
    fn next_deadline(&self) -> Option<Time> {
        Some(self.deadline.min(self.wake))
    }
    fn overdue(&self, now: Time) -> Option<String> {
        (now >= self.deadline).then(|| "account move peers did not settle".into())
    }
    fn passed(&self) -> bool {
        self.ended
    }
}

fn refuse_file_token(address: SocketAddr, client: &str, record: &[u8]) {
    // The fake's known token is checked against the actual exported bytes;
    // neither the moved form nor any credential is printed on a failure.
    assert!(skein_lib::bytes::find(record, FILE_REFRESH).is_some(), "file holds the fixture's refresh token");
    let body = format!(
        "grant_type=refresh_token&client_id={client}&refresh_token={}",
        std::str::from_utf8(FILE_REFRESH).expect("fixture token")
    );
    let mut socket = TcpStream::connect(address).expect("same fake issuer");
    socket.set_read_timeout(Some(std::time::Duration::from_secs(5))).expect("replay timeout");
    socket.write_all(format!("POST /api/accounts/oauth/token HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).expect("replay sent");
    let mut response = Vec::new();
    socket.take(8192).read_to_end(&mut response).expect("issuer refusal");
    assert!(response.starts_with(b"HTTP/1.1 400 "), "stale token refused");
    let body = response.split(|byte| *byte == b'\n').next_back().expect("refusal body");
    let error: serde_json::Value = serde_json::from_slice(body).expect("issuer error JSON");
    assert_eq!(error["error"], "invalid_grant");
}

fn assert_peers(issuer: &oauth::Peer, peer: &llm::Peer) {
    let observations = issuer.observations();
    assert_eq!(
        observations.iter().filter(|o| matches!(o, oauth::Observation::Post { .. })).count(),
        3,
        "A's code grant, B's one refresh, and the stale file replay"
    );
    assert_eq!(
        observations.iter().filter(|o| matches!(o, oauth::Observation::Answered { status: 200, .. })).count(),
        2
    );
    assert_eq!(
        observations.iter().filter(|o| matches!(o, oauth::Observation::Answered { status: 400, .. })).count(),
        1
    );
    assert_eq!(observations.iter().filter(|o| matches!(o, oauth::Observation::Revoke { .. })).count(), 0);
    let credentials: Vec<_> = peer
        .observations()
        .iter()
        .filter_map(|o| if let llm::Observation::Credential { index, .. } = o { Some(*index) } else { None })
        .collect();
    assert_eq!(credentials, [Some(0)], "B alone uses the refreshed account's grant");
    assert_eq!(
        peer.observations().iter().filter(|o| matches!(o, llm::Observation::Answered { result: Ok(()), .. })).count(),
        1,
        "one chat answer"
    );
}

fn rotation() -> skein_fake_oauth::Plan {
    skein_fake_oauth::Plan {
        status: 200,
        body: skein_fake_oauth::Body::Token(skein_oauth::TokenResponse {
            access_token: sign_in::ACCESS.as_ref().into(),
            refresh_token: Some(ROTATED_REFRESH.into()),
            expires_in: 300,
            scope: None,
            id_token: None,
            earliest_refresh_at: None,
        }),
        delay: Duration::ZERO,
        retry_after: Duration::ZERO,
    }
}

pub fn run(directory: &Path) {
    let peer = sign_in::plan_peer(Box::new([skein_fake_llm_domain::api::Script {
        cue: Box::default(),
        turns: Box::new([skein_fake_llm_domain::api::Turn {
            lines: Box::new([skein_fake_llm_domain::api::Line::Text { text: super::ANSWER.into() }]),
            finish: skein_fake_llm_domain::api::Finish::Stop,
            tokens: 1,
        }]),
    }]));
    let llm = peer.address();
    let a = directory.join("a");
    let b = directory.join("b");
    for deployment in [&a, &b] {
        fs::create_dir(deployment).expect("separate deployment");
    }
    let mut issuer = configure(&a, llm);
    drop(configure(&b, llm));
    let registration = document(&a)["engine"]["llm"]["providers"][0]["sources"]["sign_in"].clone();
    let mut second = document(&b);
    second["engine"]["llm"]["providers"][0]["sources"]["sign_in"] = registration.clone();
    super::write_private(&b.join("temper.json"), &serde_json::to_vec(&second).expect("B's configuration"));
    let address: SocketAddr = registration["issuer"]
        .as_str()
        .expect("issuer")
        .strip_prefix("http://")
        .expect("local issuer")
        .parse()
        .expect("issuer address");
    // A spare success makes stale-token refusal an issuer validation check.
    for _ in 0..2 {
        issuer.queue(rotation()).expect("rotation plan");
    }
    std::thread::scope(|scope| {
        let (stop, receiver) = mpsc::channel();
        let (ready, listening) = mpsc::channel();
        let peers = scope.spawn(move || {
            let clock = Clock::new();
            let mut world = real::World::new(Peers {
                stop: receiver,
                ended: false,
                deadline: clock.now().now.saturating_add(WITHIN),
                wake: clock.now().now,
            });
            world.spawn(|| Process::Issuer(Box::new(issuer)));
            peer.spawn(&mut world);
            ready.send(()).expect("peer world ready");
            let outcome = world.run(&clock, WITHIN);
            assert!(outcome.procs.iter().all(Host::is_empty), "peer io drained");
            let [Process::Issuer(issuer), Process::Peer(peer)] = outcome.procs.as_slice() else { panic!("peers") };
            assert_peers(issuer, peer);
        });
        listening.recv().expect("peers constructed");
        let file = directory.join("account.move");
        let (number, record) = move_account(&a, &b, &file);
        let seen = ctl::run(ctl::Story::new(
            &b,
            vec![
                ctl::Step::Run(ctl::Call::sign_in()),
                ctl::Step::Run(ctl::Call::new(&["account", "list", "--json"])),
                ctl::Step::Run(ctl::Call::new(&[
                    "chat",
                    "start",
                    "--project",
                    "1",
                    "--model",
                    "gpt-6-luna",
                    "--key",
                    "moved-chat",
                    "Hello",
                    "--json",
                ])),
                ctl::Step::Run(ctl::Call::new(&["task", "show", "1", "--until", "replied", "--json"])),
                // Stop holds the parked chat immediately, without a closing grace turn.
                ctl::Step::Run(ctl::Call::new(&["task", "stop", "1", "--json"])),
                ctl::Step::Run(ctl::Call::new(&["task", "show", "1", "--until", "held", "--json"])),
            ],
        ));
        for ended in &seen.calls {
            success(ended);
        }
        assert_eq!(account(&seen.calls[1])["number"], number, "B kept the imported account after restart");
        assert_eq!(ctl::json(&seen.calls[3])["met"], true, "the moved account answered");
        refuse_file_token(address, registration["client"].as_str().expect("client id"), &record);
        assert!(!file.exists(), "the imported file stays gone");
        drop(stop);
        peers.join().expect("independent peers");
    });
}
