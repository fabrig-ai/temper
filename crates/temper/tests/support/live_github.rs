//! The owner-run GitHub story (domain/testing.md, 2.7). Never invoked by the gate.
use super::{
    cluster, ctl, github, github_land,
    relay::{Guard, Relay},
    write_private,
};
use serde_json::{Value, json};
use skein_io::kernel::Exit;
use skein_lib::{Duration, Secret, Time, Wall};
use skein_shell::{Clock, Host};
use skein_world::{Referee, real};
use std::{
    cell::RefCell,
    fmt::Write,
    fs,
    io::Read,
    net::SocketAddr,
    path::{Path, PathBuf},
    rc::Rc,
};
const WITHIN: Duration = Duration::from_secs(1200);
const POLL: Duration = Duration::from_secs(2);
/// Install links asked while setup is busy, one a poll: the reconcile after a connect is the
/// engine's own, which `forge list --until settled` does not cover (client/tools.md, 5.4).
const BUSY: u32 = 30;
const REPOSITORY: &str = "temper-probe";

struct Inputs {
    owner: String,
    installation: u64,
    app: u64,
    slug: String,
    pem: PathBuf,
    git: PathBuf,
    run: String,
    prefix: String,
    scratch: PathBuf,
}
impl Inputs {
    fn read(scratch: &Path) -> Self {
        let owner = std::env::var("TEMPER_LIVE_GITHUB_OWNER").expect("owner from proof-live-github.sh");
        assert!(matches!(owner.as_str(), "freeekanayaka" | "fabrig-ai"), "probe owner only");
        let fixtures = PathBuf::from(std::env::var_os("TEMPER_LIVE_GITHUB_FIXTURES").expect("fixtures path"));
        let app: Value = serde_json::from_slice(&fs::read(fixtures.join("app.json")).expect("public App fixture"))
            .expect("App JSON");
        let installation = app["installations"]
            .as_array()
            .expect("fixture installations")
            .iter()
            .find(|row| row["account"] == owner)
            .expect("owner installation")["id"]
            .as_u64()
            .expect("installation id");
        let git = std::env::split_paths(&std::env::var_os("PATH").expect("PATH"))
            .map(|p| p.join("git"))
            .find(|p| p.is_file())
            .expect("real git on PATH")
            .canonicalize()
            .expect("git path");
        let version = std::process::Command::new(&git).arg("--version").env_clear().output().expect("git version");
        let version =
            std::str::from_utf8(&version.stdout).expect("version text").split_whitespace().nth(2).expect("git version");
        let mut parts = version.split('.').map(|part| part.parse::<u32>().expect("git version number"));
        let major = parts.next().expect("major");
        let minor = parts.next().expect("minor");
        assert!(major > 2 || (major == 2 && minor >= 41), "git >= 2.41 required");
        // UTC formatting through date; randomness comes from the OS, neither carries a credential.
        let date = std::process::Command::new("date").args(["-u", "+%Y%m%dT%H%M%S"]).output().expect("UTC date");
        assert!(date.status.success());
        let mut random = [0; 2];
        fs::File::open("/dev/urandom").expect("random source").read_exact(&mut random).expect("run randomness");
        let run =
            format!("{}{:02x}{:02x}", String::from_utf8(date.stdout).expect("UTC text").trim(), random[0], random[1]);
        let prefix = format!("temper/live-{run}-");
        Self {
            owner,
            installation,
            app: app["id"].as_u64().expect("App id"),
            slug: app["slug"].as_str().expect("App slug").into(),
            pem: fixtures.join("app.pem"),
            git,
            run,
            prefix,
            scratch: scratch.into(),
        }
    }
    fn manifest(&self, cleaned: bool) {
        write_private(&self.scratch.join("run.json"), &serde_json::to_vec(&json!({"run":self.run,"owner":self.owner,"repository":REPOSITORY,"prefix":self.prefix,"cleaned":cleaned})).expect("run metadata"));
    }
    fn base(&self) -> String {
        format!("/repos/{}/{}", self.owner, REPOSITORY)
    }
}

pub fn run() {
    let clock = Clock::new();
    let started = clock.now().now;
    let scratch = PathBuf::from(std::env::var_os("TMPDIR").expect("run through proof-live-github.sh"));
    let inputs = Inputs::read(&scratch);
    inputs.manifest(false);
    eprintln!("live github run={}", inputs.run);
    let mut relay = Relay::start(Guard {
        owner: inputs.owner.clone(),
        repository: REPOSITORY.into(),
        installation: inputs.installation,
        prefix: inputs.prefix.clone(),
    });
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| story(&inputs, &relay)));
    // World owns cgroup-contained Binary hosts; dropping an unwound world abandons and reaps
    // those trees before this cleanup (skein-world end_to_end.rs, Binary::drop).
    relay.quiesce();
    let cleanup = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| teardown(&inputs, &relay)));
    eprintln!(
        "live github run={} requests={} wall={}s state={} (removed on exit unless TEMPER_LIVE_GITHUB_KEEP=1)",
        inputs.run,
        relay.count(),
        clock.now().now.saturating_since(started).as_nanos() / 1_000_000_000,
        scratch.display()
    );
    if let Err(failed) = result {
        std::panic::resume_unwind(failed);
    }
    if let Err(failed) = cleanup {
        std::panic::resume_unwind(failed);
    }
    assert!(relay.refused().is_empty(), "relay refusals: {:?}", relay.refused());
}

pub(crate) fn document(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).expect("fixture document")).expect("fixture JSON")
}
fn save(path: &Path, value: &Value) {
    write_private(path, &serde_json::to_vec(value).expect("document JSON"));
}
fn render_owner(value: &mut Value, owner: &str) {
    match value {
        Value::String(s) if s == "acme" => *s = owner.into(),
        Value::Array(rows) => {
            for row in rows {
                render_owner(row, owner);
            }
        }
        Value::Object(fields) => {
            for (_, field) in fields {
                render_owner(field, owner);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}
/// Shape the live engine without reading inputs or constructing a relay.
pub(crate) fn configure_engine(value: &mut Value, owner: &str, api: SocketAddr, git: SocketAddr, workers: SocketAddr) {
    render_owner(value, owner);
    value["engine"]["hosts"]["listener"][0]["socket"] = workers.to_string().into();
    value["application"]["forges"][0] = json!({"number":1,"kind":"github","api":{"socket":api.to_string()},"transport":"plaintext","git":format!("http://{git}"),"web":null,"app_read_only":true});
    github_land::configure_landing(value, git, owner, REPOSITORY);
    value["application"]["limits"]["overrides"]["forge_component.log_poll_ms"] = 15000.into();
}
fn configure_llm(path: &Path, inputs: &Inputs) {
    github_land::configure_llm(path);
    let mut value = document(path);
    value["cued"][0]["turns"][0]["lines"][0]["arguments"]["repository"] =
        format!("{}/{}", inputs.owner, REPOSITORY).into();
    value["cued"][1]["turns"] = json!([
        {"lines":[{"call":"write","arguments":{"path":format!("probe/{}.md",inputs.run),"content":format!("temper live run {}\n",inputs.run)}}],"finish":"tool_calls","tokens":20},
        {"lines":[{"call":"finish","arguments":{"form":"change","fields":{"title":format!("Live run {}",inputs.run),"body":format!("Adds probe/{}.md.",inputs.run)}}}],"finish":"tool_calls","tokens":20}
    ]);
    save(path, &value);
}
#[derive(Default)]
struct Seen {
    landed: Option<Value>,
    identity: Option<Value>,
    tasks: Vec<u64>,
    outputs: Vec<Vec<u8>>,
    coder: Option<Value>,
}
#[derive(Debug, PartialEq)]
enum Stage {
    Llm,
    Engine,
    Worker,
    Install,
    Link,
    Accepted,
    Adopt,
    Chat,
    Landed,
    Reply,
    Pages,
    Close,
    Before,
    StopWorker,
    StopEngine,
    RestartEngine,
    RestartWorker,
    Reconnected,
    After,
    Ended,
    Stopping,
}
struct Judge<'a> {
    members: cluster::Members,
    inputs: &'a Inputs,
    relay: &'a Relay,
    jwt: &'a Secret,
    clients: SocketAddr,
    workers: SocketAddr,
    stage: Stage,
    deadline: Time,
    poll: Time,
    now: Time,
    seen: Rc<RefCell<Seen>>,
    replied: bool,
    digest: Option<String>,
    busy: u32,
    started: Time,
    landing: Option<Landing>,
}
fn story(inputs: &Inputs, relay: &Relay) {
    let clock = Clock::new();
    // This read precedes product startup, so mint independently rather than wait for its token.
    let (authorization, jwt) = mint_verifier(inputs, relay);
    // The first read: it shows the guard the probe's id, the one `/repositories/<id>` it allows.
    let probe = ask(relay, &authorization, &inputs.base());
    assert_eq!(probe["full_name"], format!("{}/{}", inputs.owner, REPOSITORY), "the probe repository");
    let [clients, workers] = super::free_addresses(&[relay.api, relay.git]);
    let mut description = cluster::Cluster::github_relay(&inputs.scratch, relay.git);
    description.bind("story.clients", &clients.to_string());
    description.prepare("llm");
    configure_llm(&description.dir("llm").join("llm.json"), inputs);
    let (mut members, hosts) = description.members();
    members.up(&["llm"]);
    let seen = Rc::new(RefCell::new(Seen::default()));
    let mut world = real::World::new(Judge {
        members,
        inputs,
        relay,
        jwt: &jwt,
        clients,
        workers,
        stage: Stage::Llm,
        deadline: clock.now().now.saturating_add(WITHIN),
        poll: clock.now().now,
        now: clock.now().now,
        seen: seen.clone(),
        replied: false,
        digest: None,
        busy: 0,
        started: clock.now().now,
        landing: None,
    });
    for host in hosts {
        world.spawn(|| host);
    }
    world.spawn(cluster::Root::client);
    let outcome = world.run(&clock, WITHIN);
    assert!(outcome.procs.iter().all(Host::is_empty), "no process outlives the story");
    let seen = seen.borrow();
    github_musts(inputs, relay, &authorization, &seen);
    scan(inputs, relay, &jwt, &seen.outputs);
}
impl Judge<'_> {
    fn queue(&mut self, host: &mut cluster::Root, args: Vec<ctl::Call>) {
        let link = format!("http://{}", self.members.capture("engine", "link").expect("engine link"));
        let mut batch = ctl::Calls::linked(&link, &self.inputs.scratch.join("client"));
        for call in args {
            batch.push(ctl::Step::Run(call));
        }
        let cluster::Root::Client { calls, .. } = host else { panic!("client root") };
        *calls = Some(Box::new(batch));
    }
    fn install(&mut self, host: &mut cluster::Root) {
        // A key's first answer is replayed (client/tools.md, 7): each ask again draws a fresh key.
        let key = format!("live-install-{}", self.busy);
        self.one(host, &["forge", "install", "1", "--key", &key], Stage::Link);
    }
    fn elapsed(&self) -> u64 {
        self.now.saturating_since(self.started).as_nanos() / 1_000_000_000
    }
    fn one(&mut self, host: &mut cluster::Root, args: &[&str], stage: Stage) {
        self.queue(host, vec![ctl::Call::new(args)]);
        self.stage = stage;
    }
    fn read_landing(&mut self, host: &mut cluster::Root, stage: Stage) {
        let mut reads = github_land::landing_reads();
        reads.push(ctl::Call::new(&["task", "show", "3", "--json"]));
        self.queue(host, reads);
        self.stage = stage;
    }
    fn begin(&mut self, host: &mut cluster::Root) {
        self.queue(
            host,
            vec![
                ctl::Call::sign_in(),
                ctl::Call::new(&[
                    "account",
                    "add-key",
                    "1",
                    self.members.dir("llm").join("llm.key").to_str().expect("LLM key path"),
                    "--deployment",
                    "--key",
                    "live-llm",
                ]),
                ctl::Call::new(&["account", "list", "--until", "usable"]),
                ctl::Call::new(&[
                    "forge",
                    "connect",
                    "1",
                    "github-app",
                    &self.inputs.app.to_string(),
                    self.inputs.pem.to_str().expect("App key path"),
                    "--key",
                    "live-app",
                ]),
                ctl::Call::new(&["forge", "list", "--until", "connected:1"]),
                ctl::Call::new(&["forge", "list", "--until", "settled"]),
            ],
        );
        self.stage = Stage::Install;
    }
    fn row(packet: &Value) -> Value {
        packet["forges"].as_array().expect("forge rows").iter().find(|row| row["number"] == 1).expect("forge 1").clone()
    }
    fn accepted(&self, row: &Value) -> bool {
        row["installations"].as_array().is_some_and(|rows| {
            rows.iter().any(|row| {
                row["id"] == self.inputs.installation
                    && row["account"]["login"] == self.inputs.owner
                    && row["stray"] == false
                    && row["suspended"] == false
            })
        })
    }
    #[expect(clippy::too_many_lines, reason = "the ordered client script shares M1's cluster and landing reads")]
    fn called(&mut self, host: &mut cluster::Root, calls: &ctl::Calls) {
        for (index, ended) in calls.ends().iter().enumerate() {
            self.seen.borrow_mut().outputs.extend([ended.stdout.clone(), ended.stderr.clone()]);
            if self.stage == Stage::Link
                && ended.exit == Some(Exit::Code(2))
                && ended.stderr.trim_ascii() == b"busy"
                && self.busy < BUSY
            {
                self.busy += 1;
                eprintln!(
                    "live github: install refused busy, asked again ({}/{BUSY}) at {}s",
                    self.busy,
                    self.elapsed()
                );
                self.install(host);
                return;
            }
            let expected = if index == 6 && matches!(self.stage, Stage::Before | Stage::After) { 2 } else { 0 };
            if ended.exit != Some(Exit::Code(expected)) {
                // The failed call's words are shown once the secrecy scan has passed over them.
                scan(self.inputs, self.relay, self.jwt, &[ended.stdout.clone(), ended.stderr.clone()]);
                panic!(
                    "live ctl call {index} failed in {:?}: {:?}, stdout: {}, stderr: {}",
                    self.stage,
                    ended.exit,
                    String::from_utf8_lossy(&ended.stdout),
                    String::from_utf8_lossy(&ended.stderr)
                );
            }
        }
        match self.stage {
            Stage::Install => self.install(host),
            Stage::Link => {
                let text = std::str::from_utf8(&calls.ends().last().expect("install").stdout).expect("link text");
                let link = text.lines().find(|s| s.starts_with("https://")).expect("GitHub installation link");
                let state = link
                    .split('?')
                    .nth(1)
                    .expect("link query")
                    .split('&')
                    .find_map(|part| part.strip_prefix("state="))
                    .expect("install state");
                assert!(state.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_%.".contains(&b)), "URL state");
                let url = format!(
                    "http://{}/forges/1/github/setup?installation_id={}&setup_action=install&state={state}",
                    self.clients, self.inputs.installation
                );
                let response = github::request_url(self.clients, "GET", &url, "", b"");
                assert_eq!(response.status, 200, "setup accepted");
                self.seen.borrow_mut().outputs.push(response.body);
                self.one(host, &["forge", "list", "--json"], Stage::Accepted);
            }
            Stage::Accepted => {
                let row = Self::row(&ctl::json(&calls.ends()[0]));
                if self.accepted(&row) {
                    self.one(
                        host,
                        &[
                            "repository",
                            "adopt",
                            "--project",
                            "1",
                            "--forge",
                            "1",
                            "--owner",
                            &self.inputs.owner,
                            "--name",
                            REPOSITORY,
                            "--landing",
                            "main",
                            "--prefix",
                            &self.inputs.prefix,
                            "--no-ci",
                            "--check",
                            "1",
                            "--key",
                            "live-adopt",
                            "--json",
                        ],
                        Stage::Adopt,
                    );
                } else {
                    self.one(host, &["forge", "list", "--json"], Stage::Accepted);
                }
            }
            Stage::Adopt => {
                assert!(ctl::json(&calls.ends()[0])["adopted"].is_object(), "repository adopted");
                self.one(
                    host,
                    &[
                        "chat",
                        "start",
                        "--project",
                        "1",
                        "--key",
                        "live-chat",
                        "@forge_chat Make the live probe",
                        "--json",
                    ],
                    Stage::Chat,
                );
            }
            Stage::Chat => {
                self.queue(
                    host,
                    vec![
                        ctl::Call::new(&["task", "show", "1", "--until", "replied"]),
                        ctl::Call::new(&["task", "show", "2", "--until", "ended", "--json"]),
                    ],
                );
                self.stage = Stage::Landed;
            }
            Stage::Landed => {
                let packet = ctl::json(calls.ends().last().expect("landed task"));
                assert_eq!(packet["task"]["chip"]["phase"], "ended");
                assert_eq!(packet["task"]["result"]["value"]["change"]["kind"], 1);
                self.seen.borrow_mut().landed = Some(packet["task"]["result"].clone());
                self.one(host, &["task", "show", "1", "--json"], Stage::Reply);
            }
            Stage::Reply => {
                self.digest = Some(
                    ctl::json(calls.ends().last().expect("reply snapshot"))["digest"]
                        .as_str()
                        .expect("task digest")
                        .into(),
                );
                self.one(host, &["chat", "show", "1", "--json"], Stage::Pages);
            }
            Stage::Pages => {
                let page = ctl::json(&calls.ends()[0]);
                self.replied |= page["activity"]["rows"].to_string().contains("The change landed.");
                if let Some(before) = page["activity"]["before"]["value"].as_str() {
                    self.one(host, &["chat", "show", "1", "--before", before, "--json"], Stage::Pages);
                } else if !self.replied {
                    self.one(
                        host,
                        &[
                            "task",
                            "show",
                            "1",
                            "--until",
                            "replied",
                            "--since",
                            &self.digest.clone().expect("snapshot"),
                            "--json",
                        ],
                        Stage::Reply,
                    );
                } else {
                    self.queue(
                        host,
                        vec![
                            ctl::Call::new(&["chat", "close", "1", "--key", "live-close", "--json"]),
                            ctl::Call::new(&["task", "show", "1", "--until", "ended", "--json"]),
                        ],
                    );
                    self.stage = Stage::Close;
                }
            }
            Stage::Close => self.read_landing(host, Stage::Before),
            Stage::Reconnected => {
                let row = Self::row(&ctl::json(calls.ends().last().expect("reconnected forges")));
                if self.accepted(&row) {
                    self.read_landing(host, Stage::After);
                } else {
                    self.one(host, &["forge", "list", "--json"], Stage::Reconnected);
                }
            }
            Stage::Before | Stage::After => self.check_landing(host, calls),
            Stage::Ended => self.ended_pages(host, calls),
            Stage::Llm
            | Stage::Engine
            | Stage::Worker
            | Stage::StopWorker
            | Stage::StopEngine
            | Stage::RestartEngine
            | Stage::RestartWorker
            | Stage::Stopping => unreachable!("calls only in script stages"),
        }
    }
    fn check_landing(&mut self, host: &mut cluster::Root, calls: &ctl::Calls) {
        let task = ctl::json(&calls.ends()[0]);
        assert_eq!(task["chip"]["phase"], "ended");
        let landed = self.seen.borrow().landed.clone().expect("first landing");
        assert_eq!(task["result"], landed, "same pull and merge");
        assert_eq!(ctl::json(&calls.ends()[1])["chip"]["phase"], "ended", "chat closed");
        super::forge_land::assert_no_live_roots(&ctl::json(&calls.ends()[5]));
        assert_eq!(calls.ends()[6].stderr.trim_ascii_end(), br#"{"error":"unknown"}"#);
        let row = Self::row(&ctl::json(&calls.ends()[4]));
        assert_eq!(row["state"], "connected");
        assert_eq!(row["login"], format!("{}[bot]", self.inputs.slug));
        assert!(row["id"].as_u64().is_some());
        assert!(self.accepted(&row), "same accepted installation");
        let coder = ctl::json(&calls.ends()[7]);
        assert_eq!(coder["chip"]["phase"], "ended");
        let mut landing = Landing {
            restarted: self.stage == Stage::After,
            tasks: vec![1],
            row,
            coder: coder["result"].clone(),
            pages: Vec::new(),
        };
        for index in [2, 3] {
            let packet = ctl::json(&calls.ends()[index]);
            super::forge_land::assert_no_live_tasks(&packet);
            landing.page(&packet);
        }
        self.landing = Some(landing);
        self.next_page(host);
    }
    fn ended_pages(&mut self, host: &mut cluster::Root, calls: &ctl::Calls) {
        let landing = self.landing.as_mut().expect("landing read");
        for ended in calls.ends() {
            landing.page(&ctl::json(ended));
        }
        self.next_page(host);
    }
    fn next_page(&mut self, host: &mut cluster::Root) {
        let pages = std::mem::take(&mut self.landing.as_mut().expect("landing read").pages);
        if !pages.is_empty() {
            let reads = pages
                .iter()
                .map(|next| ctl::Call::new(&next.split_ascii_whitespace().chain(["--json"]).collect::<Vec<_>>()))
                .collect();
            self.queue(host, reads);
            self.stage = Stage::Ended;
            return;
        }
        let Landing { restarted, mut tasks, row, coder, .. } = self.landing.take().expect("landing read");
        tasks.sort_unstable();
        assert_eq!(tasks, [1, 2, 3, 4], "complete task set");
        let mut seen = self.seen.borrow_mut();
        if restarted {
            assert_eq!(row["id"], seen.identity.as_ref().expect("prior bot")["id"]);
            assert_eq!(tasks, seen.tasks);
            assert_eq!(Some(&coder), seen.coder.as_ref());
            self.members.stop();
            self.stage = Stage::Stopping;
        } else {
            seen.identity = Some(row);
            seen.tasks = tasks;
            seen.coder = Some(coder);
            self.members.stop_one("w1");
            self.stage = Stage::StopWorker;
        }
    }
}
/// A landing read's checks, held while its ended delegates are read page by page.
struct Landing {
    restarted: bool,
    tasks: Vec<u64>,
    row: Value,
    coder: Value,
    pages: Vec<String>,
}
impl Landing {
    /// One page of a parent's ended delegates. A page may hold fewer rows than match and still name more:
    /// its `read_on` is the next read, followed until a page names none.
    fn page(&mut self, packet: &Value) {
        let rows = &packet["ended"]["rows"];
        assert_eq!(rows["cut"], false);
        self.tasks.extend(
            rows["rows"]
                .as_array()
                .expect("task rows")
                .iter()
                .map(|row| row["chip"]["task"].as_u64().expect("task id")),
        );
        if !rows["after"].is_null() {
            let next = packet["read_on"].as_str().expect("a page with more names its next read");
            assert!(next.contains("--after"), "the next read continues after this page: {next}");
            self.pages.push(next.to_owned());
        }
    }
}
impl Referee<cluster::Root> for Judge<'_> {
    fn act(&mut self, now: Time, hosts: &mut [cluster::Root]) {
        self.now = now;
        self.members.act(now, hosts);
        if self.members.failed() {
            self.stage = Stage::Stopping;
            return;
        }
        let host = hosts.last_mut().expect("client");
        if self.stage == Stage::Llm && self.members.is_up() {
            self.members.prepare("engine");
            let path = self.members.dir("engine").join("config/temper.json");
            let mut value = document(&path);
            configure_engine(&mut value, &self.inputs.owner, self.relay.api, self.relay.git, self.workers);
            save(&path, &value);
            self.members.up(&["engine"]);
            self.stage = Stage::Engine;
        } else if self.stage == Stage::Engine && self.members.is_up() {
            self.members.prepare("w1");
            let path = self.members.dir("w1").join("config/temper.json");
            let mut value = document(&path);
            value["application"]["workers"][0]["git"] = json!(self.inputs.git);
            save(&path, &value);
            self.members.up(&["w1"]);
            self.stage = Stage::Worker;
        } else if self.stage == Stage::Worker && self.members.is_up() {
            assert_eq!(self.members.linked("w1"), 1);
            self.begin(host);
        } else if self.stage == Stage::StopWorker && self.members.ended("w1") {
            self.members.stop_one("engine");
            self.stage = Stage::StopEngine;
        } else if self.stage == Stage::StopEngine && self.members.ended("engine") {
            self.members.restart("engine");
            self.stage = Stage::RestartEngine;
        } else if self.stage == Stage::RestartEngine && self.members.capture("engine", "link").is_some() {
            self.members.restart("w1");
            self.stage = Stage::RestartWorker;
        } else if self.stage == Stage::RestartWorker && self.members.is_up() {
            assert_eq!(self.members.linked("w1"), 2);
            self.queue(
                host,
                vec![
                    ctl::Call::new(&["forge", "list", "--until", "connected:1"]),
                    ctl::Call::new(&["forge", "list", "--json"]),
                ],
            );
            self.stage = Stage::Reconnected;
        } else if host.is_empty()
            && now >= self.poll
            && !matches!(
                self.stage,
                Stage::Llm
                    | Stage::Engine
                    | Stage::Worker
                    | Stage::StopWorker
                    | Stage::StopEngine
                    | Stage::RestartEngine
                    | Stage::RestartWorker
                    | Stage::Stopping
            )
        {
            let cluster::Root::Client { calls, .. } = host else { panic!("client") };
            let calls = calls.take().expect("finished batch");
            self.called(host, &calls);
            self.poll = now.saturating_add(POLL);
        }
    }
    fn observe(&mut self, now: Time, hosts: &[cluster::Root]) {
        self.members.observe(now, &hosts[..hosts.len() - 1]);
        assert!(self.relay.refused().is_empty(), "relay refusals: {:?}", self.relay.refused());
        if let Some(stopped) = self.members.stopped() {
            let scan = github::Scan::new(&fs::read(&self.inputs.pem).expect("PEM read only for scan"));
            let secrets = self.relay.tokens();
            let mut diagnostic = format!("live GitHub cluster failure={:?}\n", self.members.failure());
            for process in stopped {
                clean(&scan, self.jwt, &secrets, &process.stderr);
                writeln!(
                    diagnostic,
                    "{} life={} stderr:\n{}",
                    process.name,
                    process.life,
                    String::from_utf8_lossy(&process.stderr)
                )
                .expect("diagnostic text");
            }
            clean(&scan, self.jwt, &secrets, diagnostic.as_bytes());
            eprint!("{diagnostic}");
            assert!(hosts.iter().all(Host::is_empty));
            assert_eq!(
                stopped.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
                ["w1", "engine", "w1", "engine", "llm"]
            );
            for process in stopped {
                self.seen.borrow_mut().outputs.extend([process.stdout.clone(), process.stderr.clone()]);
                assert_eq!(process.exit, Some(Exit::Code(0)));
                assert!(!process.killed && process.survivors.is_empty());
            }
        }
    }
    fn next_deadline(&self) -> Option<Time> {
        Some(self.members.next_deadline().unwrap_or(self.deadline).min(self.deadline).min(if self.poll > self.now {
            self.poll
        } else {
            self.deadline
        }))
    }
    fn overdue(&self, now: Time) -> Option<String> {
        (now >= self.deadline).then(|| format!("live GitHub deadline in {:?}", self.stage))
    }
    fn passed(&self) -> bool {
        self.members.passed()
    }
}

fn mint_verifier(inputs: &Inputs, relay: &Relay) -> (Vec<u8>, Secret) {
    use skein_forge::github::jwt;
    use skein_lib::base64::{self, Alphabet};
    let pem = fs::read(&inputs.pem).expect("App PEM");
    let compact: String =
        std::str::from_utf8(&pem).expect("PEM text").lines().filter(|line| !line.starts_with("-----")).collect();
    let key = base64::decode(Alphabet::Standard, compact.as_bytes(), 4096).expect("App DER");
    let wall = Clock::new().now().wall.as_nanos();
    let issued = wall.saturating_sub(Duration::from_secs(60).as_nanos());
    let expires = issued.checked_add(jwt::LIFE.as_nanos()).expect("JWT expiry");
    let input =
        jwt::signing_input(inputs.app, Wall::from_nanos(issued), Wall::from_nanos(expires)).expect("App JWT claims");
    let signature = skein_tls::sign_rs256(&key, &input).expect("App key signs RS256");
    let jwt = jwt::token(&input, &signature).expect("App JWT");
    let auth = [b"Bearer ".as_slice(), jwt.expose()].concat();
    let path = format!("/app/installations/{}/access_tokens", inputs.installation);
    let (status, body) =
        relay.ask("POST", &path, &auth, br#"{"repositories":["temper-probe"]}"#).expect("verifier mint through relay");
    assert_eq!(status, 201, "verifier installation token minted");
    let value: Value = serde_json::from_slice(&body).expect("installation token JSON");
    let token = value["token"].as_str().expect("installation token").as_bytes();
    assert!(!token.is_empty() && !token.iter().any(u8::is_ascii_whitespace), "one installation token");
    // Relay records this mint before returning, for the scan and teardown even if the story panics.
    ([b"Bearer ".as_slice(), token].concat(), jwt)
}
fn ask(relay: &Relay, authorization: &[u8], path: &str) -> Value {
    let (status, body) = relay.ask("GET", path, authorization, b"").expect("verifier read through relay");
    assert_eq!(status, 200, "verifier read {path}");
    serde_json::from_slice(&body).expect("GitHub JSON")
}
fn github_musts(inputs: &Inputs, relay: &Relay, authorization: &[u8], seen: &Seen) {
    let base = inputs.base();
    let result = &seen.landed.as_ref().expect("landed result")["value"]["change"];
    let number = result["part"]["key"]["number"].as_u64().expect("pull number");
    let pull = ask(relay, authorization, &format!("{base}/pulls/{number}"));
    assert_eq!(pull["merged"], true);
    assert_eq!(pull["merge_commit_sha"], result["words"]["bytes"]);
    let head = pull["head"]["sha"].as_str().expect("pull head");
    // GitHub must agree with the exact head temper submitted for its change's merge.
    assert_eq!(relay.merge_heads(), [head], "pull head is the change head submitted by temper");
    let file = ask(relay, authorization, &format!("{base}/contents/probe/{}.md?ref=main", inputs.run));
    let compact: Vec<u8> =
        file["content"].as_str().expect("file base64").bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    let bytes = skein_lib::base64::decode(skein_lib::base64::Alphabet::Standard, &compact, 8192).expect("file bytes");
    assert_eq!(bytes.as_ref(), format!("temper live run {}\n", inputs.run).as_bytes());
    let checks = ask(relay, authorization, &format!("{base}/commits/{head}/check-runs?per_page=100"));
    let ci = checks["check_runs"]
        .as_array()
        .expect("check runs")
        .iter()
        .find(|run| run["name"] == "ci" && run["app"]["slug"] == "github-actions")
        .expect("real Actions ci");
    assert_eq!(ci["head_sha"], head);
    assert_eq!(ci["status"], "completed");
    assert_eq!(ci["conclusion"], "success");
    assert!(
        ci["completed_at"].as_str().expect("completion UTC") <= pull["merged_at"].as_str().expect("merge UTC"),
        "CI completed before merge"
    );
    let journal = relay.journal();
    for (method, path) in [("POST", format!("{base}/pulls")), ("PUT", format!("{base}/pulls/{number}/merge"))] {
        assert_eq!(
            journal.iter().filter(|row| row.0 == "api" && row.1 == method && row.2 == path).count(),
            1,
            "one {method} {path}"
        );
    }
    assert!(relay.refused().is_empty());
}
fn clean(scan: &github::Scan, jwt: &Secret, secrets: &[Vec<u8>], bytes: &[u8]) {
    scan.clean(bytes);
    assert!(skein_lib::bytes::find(bytes, jwt.expose()).is_none(), "no verifier JWT outside memory");
    for secret in secrets {
        assert!(skein_lib::bytes::find(bytes, secret).is_none(), "no minted token outside jig secrets");
    }
}
fn scan(inputs: &Inputs, relay: &Relay, jwt: &Secret, outputs: &[Vec<u8>]) {
    let scan = github::Scan::new(&fs::read(&inputs.pem).expect("PEM read only for scan"));
    let secrets = relay.tokens();
    let clean = |bytes: &[u8]| clean(&scan, jwt, &secrets, bytes);
    for bytes in outputs {
        clean(bytes);
    }
    let mut pending = ["engine", "w1", "llm"].map(|name| inputs.scratch.join(name)).to_vec();
    let excluded = github::secrets(&inputs.scratch.join("engine"));
    while let Some(path) = pending.pop() {
        if path == excluded {
            continue;
        }
        let metadata = fs::symlink_metadata(&path).expect("scan metadata");
        if metadata.is_dir() {
            pending.extend(fs::read_dir(path).expect("scan directory").map(|e| e.expect("entry").path()));
        } else if metadata.is_file() {
            clean(&fs::read(&path).expect("scan file"));
        }
    }
}
fn cleanup_refs(inputs: &Inputs, relay: &Relay, auth: &[u8]) -> bool {
    let base = inputs.base();
    let mut ok = true;
    let list = relay.teardown("GET", &format!("{base}/git/matching-refs/heads/{}", inputs.prefix), auth, b"");
    match list {
        Ok((200, body)) => {
            if let Ok(Value::Array(refs)) = serde_json::from_slice::<Value>(&body) {
                for row in refs {
                    let name = row["ref"].as_str().and_then(|r| r.strip_prefix("refs/heads/")).expect("branch ref");
                    assert!(name.starts_with(&inputs.prefix), "teardown confined to run");
                    let deleted = relay
                        .teardown("DELETE", &format!("{base}/git/refs/heads/{name}"), auth, b"")
                        .is_ok_and(|(s, _)| s == 204 || s == 404);
                    if !deleted {
                        eprintln!("live github branch left={name}");
                        ok = false;
                    }
                }
            } else {
                ok = false;
            }
        }
        _ => ok = false,
    }
    // Query by the run's exact head prefix, then verify it before closing any pull.
    match relay.teardown(
        "GET",
        &format!("{base}/pulls?state=open&head={}:{}1/c2&per_page=100", inputs.owner, inputs.prefix),
        auth,
        b"",
    ) {
        Ok((200, body)) => {
            if let Ok(Value::Array(pulls)) = serde_json::from_slice::<Value>(&body) {
                for pull in pulls {
                    let head = pull["head"]["ref"].as_str().expect("pull branch");
                    assert!(
                        head.starts_with(&inputs.prefix)
                            && pull["head"]["repo"]["full_name"] == format!("{}/{}", inputs.owner, REPOSITORY),
                        "run's pull only"
                    );
                    let n = pull["number"].as_u64().expect("pull number");
                    if !relay
                        .teardown("PATCH", &format!("{base}/pulls/{n}"), auth, br#"{"state":"closed"}"#)
                        .is_ok_and(|(s, _)| s == 200)
                    {
                        ok = false;
                    }
                }
            } else {
                ok = false;
            }
        }
        _ => ok = false,
    }
    ok
}
fn teardown(inputs: &Inputs, relay: &Relay) {
    let tokens = relay.tokens();
    // The verifier is minted first, before any product process can write or mint a token.
    let mut ok = tokens.first().is_none_or(|token| {
        let auth = [b"Bearer ".as_slice(), token].concat();
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cleanup_refs(inputs, relay, &auth))).unwrap_or(false)
    });
    for token in tokens {
        let auth = [b"Bearer ".as_slice(), &token].concat();
        if !relay.teardown("DELETE", "/installation/token", &auth, b"").is_ok_and(|(s, _)| s == 204 || s == 401) {
            ok = false;
        }
    }
    if ok {
        inputs.manifest(true);
    }
    assert!(ok, "GitHub teardown incomplete; script backstop will retry");
}
