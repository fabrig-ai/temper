//! The reference case on the cluster's processes (domain/testing.md, 2.6 and 5.3).
mod project;
use super::{Ended, Scratch, WITHIN, cluster, ctl};
use serde_json::{Value, json};
use skein_fake_llm_domain::api::{Error, Finish, Line, Report, Rule, Script};
use skein_io::kernel::Exit;
use skein_lib::{Duration, Time, Wall, base64};
use skein_shell::{Clock, Host};
use skein_world::{Referee, real};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
};
use temper_real_loop_world::{
    calls,
    case::{
        acts,
        oracle::{Kit, Observed, Told},
    },
    forge::Fixture,
};

// domain/testing.md, 2.6: pass at least every 20 ms while PLACED's release is due.
const PLACED_RELEASE_INTERVAL: Duration = Duration::from_millis(20);

pub struct Run {
    calls: Vec<Ended>,
    observed_at: Vec<Option<Wall>>,
    told: Vec<Told>,
    kit: Kit,
    workers: BTreeMap<u64, String>,
    steps: Vec<calls::Step>,
    first_workers: BTreeMap<u64, String>,
}
impl Run {
    #[must_use]
    pub fn calls(&self) -> &[Ended] {
        &self.calls
    }
    #[must_use]
    pub fn observed(&self, reads: &[(acts::Read, usize)]) -> Observed {
        let documents = reads
            .iter()
            .filter_map(|(read, position)| {
                let call = self.calls.get(*position)?;
                if call.exit != Some(Exit::Code(0)) {
                    return None;
                }
                Some((*read, serde_json::from_slice(&call.stdout).expect("case document")))
            })
            .collect();
        let observed_at = reads
            .iter()
            .filter_map(|(read, position)| Some((*read, self.observed_at.get(*position).copied().flatten()?)))
            .collect();
        Observed { documents, observed_at, told: self.told.clone(), commands: Vec::new(), kit: self.kit.clone() }
            .with_commands(&self.steps)
    }
    #[must_use]
    pub fn first_placed(&self) -> &BTreeMap<u64, String> {
        &self.first_workers
    }
    /// Placement only where a document retains a live observation channel.
    #[must_use]
    pub fn placed(&self) -> &BTreeMap<u64, String> {
        &self.workers
    }
}
fn read(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).expect("prepared document")).expect("JSON")
}
fn write(path: &Path, document: &Value) {
    super::write_private(path, &serde_json::to_vec(document).expect("JSON"));
}
fn text(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).expect("case UTF-8")
}
fn encode(scripts: &[Script], rules: &[Box<[Rule]>]) -> Value {
    assert_eq!(scripts.len(), rules.len(), "index-parallel turn rules");
    scripts
        .iter()
        .zip(rules)
        .map(|(script, rules)| {
            let turns: Vec<_> = script
                .turns
                .iter()
                .map(|turn| {
                    let lines: Vec<_> = turn
                        .lines
                        .iter()
                        .map(|line| match line {
                            Line::Text { text: words } => json!({"text": text(words)}),
                            Line::Call { name, arguments } => json!({
                                "call": text(name),
                                "arguments": serde_json::from_slice::<Value>(arguments).expect("arguments")
                            }),
                            Line::Pieces { pieces } => json!({"pieces":pieces.iter().map(|piece| json!({
                                "text":text(&piece.text),"hold":piece.hold.as_deref().map(text)
                            })).collect::<Vec<_>>()}),
                            Line::Fail { error, report, credentials } => {
                                assert_eq!(*report, Report::none(), "case failure report unsupported: {report:?}");
                                let error = match error {
                                    Error::Overloaded => json!("overloaded"),
                                    Error::RateLimited { retry_after } => json!({"rate_limited": {
                                        "retry_after_ms": retry_after.as_nanos() / 1_000_000
                                    }}),
                                    Error::Unavailable => json!("unavailable"),
                                    Error::ContextTooLong => json!("context_too_long"),
                                    Error::Unauthorized => json!("unauthorized"),
                                    Error::Exhausted { retry_after } => json!({"exhausted": {
                                        "retry_after_ms": retry_after.map(|delay| delay.as_nanos() / 1_000_000)
                                    }}),
                                    Error::Refused => json!("refused"),
                                    Error::Unsupported => json!("unsupported"),
                                    Error::InvalidRequest => json!("invalid_request"),
                                };
                                json!({"fail": {"error": error, "credentials": credentials}})
                            }
                            other @ Line::Opaque { .. } => {
                                panic!("case script line unsupported: {other:?}")
                            }
                        })
                        .collect();
                    let finish = match turn.finish {
                        Finish::Stop => "stop",
                        Finish::ToolCalls => "tool_calls",
                        Finish::Length => "length",
                        Finish::ContentFilter => "content_filter",
                    };
                    json!({"lines": lines, "finish": finish, "tokens": turn.tokens})
                })
                .collect();
            let mut script = json!({"cue": text(&script.cue), "turns": turns});
            if !rules.is_empty() {
                script["inputs"] = rules.iter().map(|rule| json!({"cue":text(&rule.cue),"turn":rule.turn})).collect();
            }
            script
        })
        .collect()
}
/// Bring lab's combined document to tests/real-loop/src/fixtures.rs's base
/// before applying the case fixture once, as that world's cluster does.
fn base(value: &mut Value) {
    for member in ["charters", "templates", "policies", "rules"] {
        value["engine"].as_object_mut().expect("engine").remove(member);
    }
    // The forge's delegates use the same ordinary charter as the real loop.
    value["application"]["charter"] = 1.into();
}
fn process_settings(value: &mut Value) -> Value {
    let given = temper_real_loop_world::fixtures::own_defaults(value, &value["world_settings"]);
    let temper_service::stock::Given::Own(defaults) = given else { panic!("case defaults") };
    let items = temper_real_loop_world::case::defaults::items(&defaults);
    value.as_object_mut().expect("deployment").remove("world_settings");
    // Settings and the project are established by the owner after sign-in.
    // Local setup supplies credentials, without adopting an incidental project first.
    value["engine"]["projects"] = json!([]);
    let landing = std::mem::take(&mut value["application"]["landing"]["projects"]);
    assert_eq!(value["application"]["landing"]["deployment"], json!([]));
    let mut rules = Vec::new();
    for project in landing.as_array().expect("project landing rules") {
        assert_eq!(project["project"], temper_real_loop_world::case::PROJECT);
        rules.extend(project["rules"].as_array().expect("landing rules").iter().cloned());
    }
    value["application"]["landing"]["projects"] = json!([]);
    value["application"]["landing"]["deployment"] = rules.into();
    for member in ["charters", "templates", "policies", "rules"] {
        value["engine"].as_object_mut().expect("engine").remove(member);
    }
    for (name, minimum) in
        [("tasks.charters", 2), ("authority.grants", 16), ("notes.scopes", 32), ("core.policy_bytes", 4096)]
    {
        let held = value["engine"]["limits"]["overrides"][name].as_u64().unwrap_or(0);
        value["engine"]["limits"]["overrides"][name] = held.max(minimum).into();
    }
    items
}

fn reconcile(directory: &Path) {
    let reconciled = std::process::Command::new(skein_scratch::paths::binary("temper"))
        .env_clear()
        .args(["forge", "reconcile", "--config"])
        .arg(directory.join("config"))
        .arg("--data")
        .arg(directory.join("data"))
        .arg("--cache")
        .arg(directory.join("cache"))
        .arg("1")
        .current_dir(directory)
        .output()
        .expect("case forge credentials reconciled");
    assert!(reconciled.status.success(), "case forge reconcile: {}", String::from_utf8_lossy(&reconciled.stderr));
}

/// Establish the same forge identity without adopting a project before sign-in.
fn setup(value: &mut Value, forge: SocketAddr, directory: &Path, admin_token: &Value) {
    value["application"]["webhooks"] = json!({"listener":{"socket":"127.0.0.1:0"}});
    value["application"]["forges"] = json!([{
        "number":1,"kind":"forgejo","api":{"socket":forge.to_string()},
        "transport":"plaintext","git":format!("http://{forge}"),
        "api_token":directory.join("api_token"),"user":{"file":directory.join("user")},
        "webhook_secret":directory.join("webhook_secret")
    }]);
    value["application"]["identities"] = json!([{
        "number":1,"forge":1,"login":"temper","author":{"name":"Temper","email":"temper@example.test"},
        "token":directory.join("git_token")
    }]);
    value["application"]["setup"] = json!([{
        "forge":1,"directory":directory,"bot":{"login":"temper","email":"temper@example.test"},
        "orgs":["acme"],"mode":{"managed":{"admin_token":admin_token}}
    }]);
}

pub fn run(
    workers: &[&str],
    fixture: Fixture,
    scripts: fn(SocketAddr) -> Box<[Script]>,
    rules: fn() -> Box<[Box<[Rule]>]>,
    steps: Vec<calls::Step>,
    placed: &[u64],
) -> Run {
    assert!(workers == ["ws"] || workers == ["ws", "s1", "s2"], "case cast");
    let cast = if workers.len() == 1 { vec!["ws"] } else { vec!["ws", "s1", "s2"] };
    if cast.len() == 1 {
        assert!(placed.is_empty(), "one worker needs no placement mapping");
    }
    let late = steps.iter().any(|step| matches!(step, calls::Step::AddWorker { .. }));
    let clock = Clock::new();
    let began = clock.now().now;
    let scratch = Scratch::new(&format!("case_process_{}", std::process::id()));
    let mut description = cluster::Cluster::read(scratch.path());
    description.rename("w1", "ws");
    if cast.len() > 1 {
        description.rename("w2", "s1");
        description.copy("s1", "s2");
    }
    description.record_llm();
    let [hosts] = super::free_addresses(&[]);
    description.bind("proc.engine.ready.workers", &hosts.to_string());
    let forge = description.prepare("forge");
    let mut document = read(&forge.join("forge.json"));
    let kit = (fixture.setup)();
    let ci = match kit.checks.cue {
        Some(skein_fake_forge_domain::api::Cue::File { path, green }) => {
            json!({"path":text(&path),"green":text(&green)})
        }
        Some(skein_fake_forge_domain::api::Cue::Message { .. }) => panic!("case forge message cue unsupported"),
        None => Value::Null,
    };
    document["owner"] = "acme".into();
    document["repositories"] = json!([
        {"id":kit.id,"name":"app","landing":text(&kit.default),"ci":ci,"tree":kit.tree.iter().map(|file| json!({"path":text(&file.path),"content":text(&file.content)})).collect::<Vec<_>>()},
        {"id":1,"name":"spare","landing":"main","ci":null}
    ]);
    write(&forge.join("forge.json"), &document);
    let git = scratch.path().join("git");
    let (mut members, roots) = description.members();
    members.up(&["forge"]);
    let result = std::rc::Rc::new(std::cell::RefCell::new(None));
    let mut world = real::World::new(Judge {
        members,
        cast,
        stage: Stage::Forge,
        deadline: began.saturating_add(WITHIN),
        marked: began,
        phases: Vec::new(),
        fixture,
        scripts,
        rules,
        hosts,
        git,
        steps: steps.into(),
        ran: Vec::new(),
        calls: Vec::new(),
        observed_at: Vec::new(),
        slots: Vec::new(),
        client: scratch.path().join("client"),
        provisioning: false,
        live: None,
        placed: placed.to_vec(),
        run: result.clone(),
        stopped: false,
        late,
        control: None,
        placed_release: None,
        assigned: BTreeSet::new(),
        channels: BTreeMap::new(),
        workers: BTreeMap::new(),
        first_workers: BTreeMap::new(),
        processed: 0,
        stopped_links: None,
        admin_token: Vec::new(),
        settings: Value::Null,
        project: None,
    });
    for root in roots {
        world.spawn(|| root);
    }
    world.spawn(cluster::Root::client);
    let outcome = world.run(&clock, WITHIN);
    assert!(outcome.procs.iter().all(Host::is_empty), "every root empty");
    scratch.remove();
    result.borrow_mut().take().expect("case observations")
}
#[derive(Debug)]
enum Stage {
    Forge,
    Llm,
    Engine,
    Ws,
    S1,
    S2,
    Added { target: usize },
    Placed,
    BeforeAdd,
    Stopping,
    Starting { target: usize },
    BeforeStop,
    Acts,
    Provision,
    ReadRules,
    ChangeSettings,
    Live,
    Accounts,
    Stop,
}
struct Judge {
    members: cluster::Members,
    cast: Vec<&'static str>,
    stage: Stage,
    deadline: Time,
    marked: Time,
    phases: Vec<(&'static str, skein_lib::Duration)>,
    fixture: Fixture,
    scripts: fn(SocketAddr) -> Box<[Script]>,
    rules: fn() -> Box<[Box<[Rule]>]>,
    hosts: SocketAddr,
    git: PathBuf,
    steps: VecDeque<calls::Step>,
    ran: Vec<calls::Step>,
    calls: Vec<Ended>,
    observed_at: Vec<Option<Wall>>,
    slots: Vec<usize>,
    client: PathBuf,
    provisioning: bool,
    live: Option<Value>,
    placed: Vec<u64>,
    run: std::rc::Rc<std::cell::RefCell<Option<Run>>>,
    stopped: bool,
    late: bool,
    control: Option<SocketAddr>,
    placed_release: Option<Time>,
    assigned: BTreeSet<String>,
    channels: BTreeMap<u64, u64>,
    workers: BTreeMap<u64, String>,
    first_workers: BTreeMap<u64, String>,
    processed: usize,
    stopped_links: Option<usize>,
    admin_token: Vec<u8>,
    settings: Value,
    project: Option<u32>,
}
impl Judge {
    fn mark(&mut self, phase: &'static str, now: Time) {
        self.phases.push((phase, now.saturating_since(self.marked)));
        self.marked = now;
    }
    fn calls(root: &mut cluster::Root) -> &mut ctl::Calls {
        let cluster::Root::Client { calls: Some(calls), .. } = root else { panic!("client calls") };
        calls
    }
    fn queue(&self, root: &mut cluster::Root) {
        let link = format!("http://{}", self.members.capture("engine", "link").expect("engine link"));
        let mut batch = ctl::Calls::linked(&link, &self.client);
        batch.hold();
        let cluster::Root::Client { calls, .. } = root else { panic!("client root") };
        *calls = Some(Box::new(batch));
    }
    fn read_startup_rules(&mut self, root: &mut cluster::Root) {
        for call in Self::calls(root).ends() {
            success(call);
        }
        Self::calls(root).push(ctl::Step::Run(ctl::Call::new(&["setting", "rules", "--json"])));
        self.stage = Stage::ReadRules;
    }
    fn change_settings(&mut self, root: &mut cluster::Root) {
        let read = Self::calls(root).ends().last().expect("startup rules");
        success(read);
        let original = ctl::json(read);
        for item in self.settings.as_array_mut().expect("case setting items") {
            if item["setting"]["kind"] == "rules" {
                item["set"]["requirements"] = original["value"]["requirements"].clone();
            }
        }
        let file = self.client.parent().expect("case scratch").join("case-settings.json");
        write(&file, &json!({"items":self.settings}));
        Self::calls(root).push(ctl::Step::Run(ctl::Call::new(&[
            "change-settings",
            file.to_str().expect("settings path"),
            "--against",
            "1",
            "--key",
            "case-settings",
        ])));
        self.stage = Stage::ChangeSettings;
    }
    fn make_project(&mut self, root: &mut cluster::Root) {
        success(Self::calls(root).ends().last().expect("case settings changed"));
        let made = project::make(&self.client);
        assert_eq!(u64::from(made), temper_real_loop_world::case::PROJECT, "the case keeps its project identity");
        self.project = Some(made);
        self.stage = Stage::Acts;
    }
    fn configure(&mut self) {
        let engine_dir = self.members.prepare("engine");
        let ws_dir = self.members.prepare("ws");
        let mut value = read(&engine_dir.join("config/temper.json"));
        let ws = read(&ws_dir.join("config/temper.json"));
        let mut agent = read(&ws_dir.join("config/agent.json"));
        // With a control listener, the fake's ready line names provider then control.
        let mut addresses = self.members.capture("llm", "address").expect("LLM addresses").split_whitespace();
        let llm: SocketAddr = addresses.next().expect("LLM provider").parse().expect("provider socket");
        let control = addresses.next().map(|address| address.parse::<SocketAddr>().expect("control socket"));
        assert_eq!(control, self.control, "LLM control ready at its reserved address");
        assert!(addresses.next().is_none(), "only provider and control addresses");
        value["engine"]["llm"]["endpoints"][0]["address"] = json!({"socket":llm.to_string()});
        agent["endpoints"][0]["address"] = llm.to_string().into();
        value["workers"] = ws["workers"].clone();
        value["application"]["workers"] = ws["application"]["workers"].clone();
        base(&mut value);
        let forge = self.members.capture("forge", "address").expect("forge address").parse().expect("socket");
        // The fake forge removes its token file when it ends, so the token is read while it runs.
        let document = read(&self.members.dir("forge").join("forge.json"));
        self.admin_token =
            fs::read(document["admin_token"].as_str().expect("admin token file")).expect("fake admin token");
        (self.fixture.configure)(&mut value, &mut agent, forge);
        self.settings = process_settings(&mut value);
        setup(&mut value, forge, &self.members.dir("forge").join("setup"), &document["admin_token"]);
        super::configuration::turn_bounds::apply(&mut value, super::configuration::turn_bounds::turn_body(&agent));
        value["engine"]["hosts"]["listener"] = json!([{"socket":self.hosts.to_string()}]);
        for key in ["fleet.workers", "fleet.attempts"] {
            // tests/real-loop/src/cluster.rs: one admitted position/attempt per worker.
            value["engine"]["limits"]["overrides"][key] = 3.into();
        }
        // accounts' program hosts hold documents.lines rows (jig documents/programs.rs `hosts`);
        // the lab's 2 would cut s2 from the placement read.
        value["engine"]["limits"]["overrides"]["documents.lines"] = 3.into();
        value["engine"]["hosts"]["workers"][0]["name"] = "ws".into();
        value["engine"]["hosts"]["workers"].as_array_mut().expect("hosts").truncate(self.cast.len());
        let secret = b"s2-case-fixture-secret";
        if self.cast.len() > 1 {
            value["engine"]["hosts"]["workers"][1]["name"] = "s1".into();
            value["engine"]["hosts"]["workers"]
                .as_array_mut()
                .expect("hosts")
                .push(json!({"name":"s2","digest":"secrets/s2.digest"}));
            let mut hash = skein_lib::sha256::Sha256::new();
            hash.update(secret);
            let digest = hash.finish().iter().fold(String::new(), |mut hex, byte| {
                use std::fmt::Write as _;
                write!(hex, "{byte:02x}").expect("hex");
                hex
            });
            super::write_private(&engine_dir.join("data/secrets/s2.digest"), digest.as_bytes());
        }
        for &name in &self.cast {
            let dir = if name == "ws" { ws_dir.clone() } else { self.members.prepare(name) };
            let mut worker = read(&dir.join("config/temper.json"));
            if name == "s2" {
                // Copy keeps the source templates; give the new member its own paths and secret.
                let source = self.members.dir("s1");
                worker = serde_json::from_str(
                    &serde_json::to_string(&worker)
                        .expect("worker")
                        .replace(source.to_str().expect("source path"), dir.to_str().expect("worker path")),
                )
                .expect("copied worker");
                fs::remove_file(dir.join("data/secrets/w2.secret")).expect("remove copied secret");
                worker["workers"][0]["secret"] = "secrets/s2.secret".into();
                super::write_private(&dir.join("data/secrets/s2.secret"), secret);
            }
            let mut shaped = value["workers"][0].clone();
            // Preserve this member's independent paths and secret, copying only fixture policy.
            shaped["name"] = name.into();
            shaped["secret"] = worker["workers"][0]["secret"].clone();
            shaped["agent"] = worker["workers"][0]["agent"].clone();
            worker["workers"] = json!([shaped]);
            worker["application"]["limits"] = value["application"]["limits"].clone();
            worker["application"]["workers"][0]["git"] = self.git.to_str().expect("git path").into();
            write(&dir.join("config/temper.json"), &worker);
            write(&dir.join("config/agent.json"), &agent);
        }
        value["workers"] = json!([]);
        value["application"]["workers"] = json!([]);
        write(&engine_dir.join("config/temper.json"), &value);
        reconcile(&engine_dir);
        permit_repository(forge, &self.admin_token);
    }
    fn arguments(&self, argv: &calls::Argv) -> Vec<String> {
        argv.0
            .iter()
            .enumerate()
            .map(|(position, arg)| match arg {
                calls::Arg::GithubApp => panic!("GithubApp unsupported"),
                calls::Arg::Lit(word) => (*word).into(),
                calls::Arg::Number(_)
                    if position > 0 && matches!(&argv.0[position - 1], calls::Arg::Lit("--project")) =>
                {
                    self.project.expect("ProjectMade before acts").to_string()
                }
                calls::Arg::Number(n) => n.to_string(),
                calls::Arg::File(name) => {
                    self.client.parent().expect("scratch").join(name).to_str().expect("file path").into()
                }
                calls::Arg::Task(n) => text(&self.calls[*n].stdout)
                    .split("done: task ")
                    .nth(1)
                    .expect("created task")
                    .split(|c: char| !c.is_ascii_digit())
                    .next()
                    .expect("task")
                    .into(),
                calls::Arg::Delegate(n) => {
                    let v = ctl::json(&self.calls[*n]);
                    let tree = &v["tree"];
                    let root = &tree["root"]["task"];
                    let children: Vec<_> = tree["rows"]
                        .as_array()
                        .expect("tree rows")
                        .iter()
                        .filter(|row| &row["parent"] == root)
                        .collect();
                    assert_eq!(children.len(), 1);
                    children[0]["chip"]["task"].as_u64().expect("child").to_string()
                }
                calls::Arg::Digest(n) => ctl::json(&self.calls[*n])["task"]["digest"].as_str().expect("digest").into(),
                calls::Arg::ActivityBefore(n) => {
                    let v = ctl::json(&self.calls[*n]);
                    v.pointer("/activity/before/value")
                        .or_else(|| v.pointer("/before/value"))
                        .and_then(Value::as_str)
                        .expect("before")
                        .into()
                }
                calls::Arg::InboxAfter(n) => {
                    let v = ctl::json(&self.calls[*n]);
                    let cursor = v.pointer("/needs/after/value").and_then(Value::as_str).expect("next needs page");
                    format!("00{cursor}")
                }
            })
            .collect()
    }
    fn prepare_calls(&mut self, root: &mut cluster::Root) -> bool {
        let batch = Self::calls(root);
        if !batch.foreground_idle() {
            return false;
        }
        self.calls = self
            .slots
            .iter()
            .map(|slot| {
                let ended = &batch.ends()[*slot];
                Ended { exit: ended.exit, stdout: ended.stdout.clone(), stderr: ended.stderr.clone() }
            })
            .collect();
        self.observed_at = self.slots.iter().map(|slot| batch.observed_at()[*slot]).collect();
        for ended in &batch.ends()[self.processed..] {
            if ended.exit.is_none() {
                continue;
            }
            success(ended);
            if !self.placed.is_empty()
                && let Ok(document) = serde_json::from_slice::<Value>(&ended.stdout)
            {
                live_channels(&document, &mut self.channels);
            }
        }
        self.processed = batch.ends().len();
        if !self.provisioning && self.calls.len() == 1 {
            self.provisioning = true;
            let key = self.members.dir("llm").join("llm.key");
            batch.push(ctl::Step::Run(ctl::Call::new(&[
                "add-key",
                "1",
                key.to_str().expect("key"),
                "--deployment",
                "--key",
                "case-llm",
            ])));
            batch.push(ctl::Step::Run(ctl::Call::new(&["wait", "accounts", "--until", "usable"])));
            self.stage = Stage::Provision;
            return false;
        }
        if !self.placed.is_empty()
            && matches!(self.ran.last(), Some(calls::Step::Call { argv, .. }) if self.arguments(argv)[0] == "chat")
            && !matches!(self.live.as_ref(), Some(value) if value["chat_act"] == self.ran.len())
        {
            // The harness's own read, as the provisioning is: the director's chat
            // can end before an act reads it live, and an ended document names no
            // worker, so read its running document once.
            let director = text(&self.calls.last().expect("chat call").stdout)
                .split("done: task ")
                .nth(1)
                .expect("chat task")
                .split(|c: char| !c.is_ascii_digit())
                .next()
                .expect("task")
                .to_owned();
            batch.push(ctl::Step::Run(ctl::Call::new(&[
                "wait",
                "task",
                &director,
                "--until",
                "phase:running",
                "--json",
            ])));
            self.stage = Stage::Live;
            return false;
        }
        true
    }
    fn advance(&mut self, now: Time, root: &mut cluster::Root) {
        use calls::Step;
        if !self.prepare_calls(root) {
            return;
        }
        let batch = Self::calls(root);
        let Some(step) = self.steps.pop_front() else {
            self.mark("acts", now);
            batch.push(ctl::Step::Run(ctl::Call::new(&["accounts", "--json"])));
            self.stage = Stage::Accounts;
            return;
        };
        match &step {
            Step::SignIn { at } => {
                assert_eq!(at.0, 1);
                self.slots.push(batch.ends().len());
                batch.push(ctl::Step::Run(ctl::Call::sign_in()));
            }
            Step::Call { at, argv } => {
                assert_eq!(at.0, 1);
                let mut args = self.arguments(argv);
                if args.first().is_some_and(|verb| verb == "wait") && !args.iter().any(|arg| arg == "--json") {
                    args.push("--json".into());
                }
                if matches!(self.steps.front(), Some(Step::StopWorker { name: "ws" })) {
                    let director = temper_real_loop_world::case::HANG.to_string();
                    assert_eq!(args, ["wait", "task", &director, "--until", "replied", "--json"]);
                    batch.push(ctl::Step::Run(ctl::Call::new(&args.iter().map(String::as_str).collect::<Vec<_>>())));
                    // Replied also accepts WaitingForYou while work is still writing.
                    // After the committed-read barrier, retain the exact waiting
                    // observation the oracle requires before stopping ws.
                    self.slots.push(batch.ends().len() + 1);
                    batch.push(ctl::Step::Until(
                        ctl::Call::new(&["task", &director, "--json"]),
                        b"\"work\":\"waiting\"",
                    ));
                } else {
                    self.slots.push(batch.ends().len());
                    batch.push(ctl::Step::Run(ctl::Call::new(&args.iter().map(String::as_str).collect::<Vec<_>>())));
                }
            }
            Step::Start { at, argv } => {
                assert_eq!(at.0, 1);
                let args = self.arguments(argv);
                self.slots.push(batch.ends().len());
                batch.push(ctl::Step::Start(ctl::Call::new(&args.iter().map(String::as_str).collect::<Vec<_>>())));
            }
            Step::Until { at, argv, holds, early: _ } => {
                assert_eq!(at.0, 1);
                let args = self.arguments(argv);
                self.slots.push(batch.ends().len());
                batch.push(ctl::Step::UntilMatching(
                    ctl::Call::new(&args.iter().map(String::as_str).collect::<Vec<_>>()),
                    *holds,
                ));
            }
            Step::Join { call } => {
                assert!(*call < self.slots.len());
                batch.push(ctl::Step::Join);
            }
            Step::AddWorker { name } => {
                assert_eq!(*name, "s2");
                batch.push(ctl::Step::Run(ctl::Call::new(&["accounts", "--json"])));
                self.stage = Stage::BeforeAdd;
            }
            Step::StopWorker { name } => {
                assert_eq!(*name, "ws");
                batch.push(ctl::Step::Run(ctl::Call::new(&["accounts", "--json"])));
                self.stage = Stage::BeforeStop;
            }
            Step::StartWorker { name } => {
                assert_eq!(*name, "ws");
                assert!(self.members.ended(name), "{name} stayed ended through the chat's ending");
                assert_eq!(Some(self.members.linked(name)), self.stopped_links, "{name} stayed unlinked");
                self.stopped_links = None;
                let target = self.members.linked(name) + 1;
                self.members.restart(name);
                self.stage = Stage::Starting { target };
            }
            Step::EndsLeaving(_) => panic!("EndsLeaving requires the real-loop completion judge"),
            Step::FixtureReady => panic!("FixtureReady unsupported"),
            Step::Advance => panic!("Advance unsupported"),
            Step::Push { .. } => panic!("Push unsupported"),
            Step::ReleaseHold { .. } => panic!("ReleaseHold unsupported"),
            Step::Probe { .. } => panic!("Probe unsupported"),
            Step::Release { .. } => panic!("Release unsupported"),
            Step::Contended { .. } => panic!("Contended unsupported"),
            Step::SameParty { .. } => panic!("SameParty unsupported"),
            Step::Held { .. } => panic!("Held unsupported"),
            Step::Kill { .. } => panic!("Kill unsupported"),
            Step::Watching { call } => {
                let slot = *self.slots.get(*call).expect("Watching names a started call");
                assert!(batch.ends()[slot].exit.is_none(), "Watching call {call} already ended");
                // No delivery barrier: ended documents are retained (domain/testing.md, 2.6).
            }
            Step::Terminal(_) => panic!("Terminal unsupported"),
            Step::Browse { .. } => panic!("Browse unsupported"),
            Step::Authorize => panic!("Authorize unsupported"),
            Step::Return => panic!("Return unsupported"),
        }
        // The oracle's commands are the acts' verbs (Observed::with_commands).
        self.ran.push(step);
    }
}
impl Judge {
    fn release_placed(&mut self, now: Time) {
        if matches!(self.stage, Stage::Accounts | Stage::Stop) {
            self.placed_release = None;
            return;
        }
        if let Some(control) = self.control {
            // The placement read follows s2's start; all three are started by then.
            // Cache the cumulative predicate instead of reading placement on each pass.
            if self.placed_release.is_none() && !["ws", "s1", "s2"].iter().all(|name| self.assigned.contains(*name)) {
                return;
            }
            release(control);
            self.placed_release = Some(now.saturating_add(PLACED_RELEASE_INTERVAL));
        }
    }
    fn start_llm(&mut self, now: Time) {
        self.mark("forge ready", now);
        let address = self.members.capture("forge", "address").expect("forge address").parse().expect("socket");
        let directory = self.members.prepare("llm");
        let path = directory.join("llm.json");
        let mut document = read(&path);
        document["models"] =
            json!([{"name":"fake","window":1_048_576,"output":8192},{"name":"owner","window":1_048_576,"output":8192}]);
        document["added_keys"] =
            json!([self.client.parent().expect("scratch").join(temper_real_loop_world::case::SECOND_KEY_FILE)]);
        document["cued"] = encode(&(self.scripts)(address), &(self.rules)());
        assert!(self.cast.len() > 1 || !has_holds(&document["cued"]), "one worker has no placed hold");
        if has_holds(&document["cued"]) {
            let [control] = super::free_addresses(&[address, self.hosts]);
            self.control = Some(control);
            document["control"] = json!({"socket":control.to_string()});
        }
        write(&path, &document);
        self.members.up(&["llm"]);
        self.stage = Stage::Llm;
    }
    fn ws_ready(&mut self, now: Time, root: &mut cluster::Root) {
        self.mark("ws ready", now);
        if self.cast.len() == 1 {
            self.queue(root);
            self.stage = Stage::Acts;
        } else {
            self.members.up(&["s1"]);
            self.stage = Stage::S1;
        }
    }
    fn add_worker(&mut self, root: &mut cluster::Root) {
        let document = ctl::json(Self::calls(root).ends().last().expect("before add"));
        self.map_channels(&document);
        for name in ["ws", "s1"] {
            assert!(self.assigned.contains(name), "{name} held a run before s2 started");
        }
        let target = self.members.linked("s2") + 1;
        self.members.up(&["s2"]);
        self.stage = Stage::Added { target };
    }
    fn record_placement(&mut self, root: &mut cluster::Root) {
        let batch = Self::calls(root);
        for call in &batch.ends()[self.processed..] {
            success(call);
            live_channels(&ctl::json(call), &mut self.channels);
        }
        self.processed = batch.ends().len();
        let accounts = ctl::json(batch.ends().last().expect("placement hosts"));
        self.map_channels(&accounts);
        assert!(
            ["ws", "s1", "s2"].iter().all(|name| self.assigned.contains(*name)),
            "every started worker held a run: assigned {:?}; channels {:?}; placed {:?}; accounts {accounts}",
            self.assigned,
            self.channels,
            self.workers
        );
        self.stage = Stage::Acts;
    }
    fn finish_acts(&mut self, now: Time, root: &mut cluster::Root) {
        let accounts = Self::calls(root).ends().last().expect("post-act accounts");
        success(accounts);
        let accounts = ctl::json(accounts);
        self.collect(&accounts);
        self.mark("reads", now);
        Self::calls(root).release();
        self.members.stop();
        self.stage = Stage::Stop;
    }
}
impl Referee<cluster::Root> for Judge {
    fn act(&mut self, now: Time, roots: &mut [cluster::Root]) {
        self.members.act(now, roots);
        if self.members.failed() {
            self.stage = Stage::Stop;
            self.placed_release = None;
            return;
        }
        if let Some(links) = self.stopped_links {
            assert!(self.members.ended("ws"), "ws remains ended until StartWorker");
            assert_eq!(self.members.linked("ws"), links, "ws did not relink before StartWorker");
        }
        let root = roots.last_mut().expect("client root");
        match self.stage {
            Stage::Forge if self.members.is_up() => self.start_llm(now),
            Stage::Llm if self.members.is_up() => {
                self.mark("llm ready", now);
                self.configure();
                self.members.up(&["engine"]);
                self.stage = Stage::Engine;
            }
            Stage::Engine if self.members.is_up() && root.is_empty() => {
                self.mark("engine ready", now);
                self.members.up(&["ws"]);
                self.stage = Stage::Ws;
            }
            Stage::Ws if self.members.is_up() => self.ws_ready(now, root),
            Stage::S1 if self.members.is_up() => {
                self.mark("s1 ready", now);
                if self.late {
                    self.queue(root);
                    self.stage = Stage::Acts;
                } else {
                    self.members.up(&["s2"]);
                    self.stage = Stage::S2;
                }
            }
            Stage::S2 if self.members.is_up() => {
                self.queue(root);
                self.stage = Stage::Acts;
            }
            Stage::BeforeAdd if Self::calls(root).foreground_idle() => self.add_worker(root),
            Stage::Added { target } if self.members.linked("s2") == target => {
                let task = temper_real_loop_world::case::BENCHMARK.to_string();
                let batch = Self::calls(root);
                batch.push(ctl::Step::Run(ctl::Call::new(&["wait", "task", &task, "--until", "replied", "--json"])));
                batch.push(ctl::Step::Run(ctl::Call::new(&["accounts", "--json"])));
                self.stage = Stage::Placed;
            }
            Stage::Placed if Self::calls(root).foreground_idle() => self.record_placement(root),
            Stage::BeforeStop if Self::calls(root).foreground_idle() => {
                self.map_channels(&ctl::json(Self::calls(root).ends().last().expect("first life hosts")));
                self.first_workers = std::mem::take(&mut self.workers);
                self.channels.clear();
                self.members.stop_one("ws");
                self.stage = Stage::Stopping;
            }
            Stage::Stopping if self.members.ended("ws") && Self::calls(root).foreground_idle() => {
                self.stopped_links = Some(self.members.linked("ws"));
                self.stage = Stage::Acts;
            }
            Stage::Starting { target } if self.members.linked("ws") == target => self.stage = Stage::Acts,
            Stage::Acts => self.advance(now, root),
            Stage::Accounts if Self::calls(root).foreground_idle() => self.finish_acts(now, root),
            Stage::Provision if Self::calls(root).foreground_idle() => self.read_startup_rules(root),
            Stage::ReadRules if Self::calls(root).foreground_idle() => self.change_settings(root),
            Stage::ChangeSettings if Self::calls(root).foreground_idle() => self.make_project(root),
            Stage::Live if Self::calls(root).foreground_idle() => {
                let read = Self::calls(root).ends().last().expect("director's running read");
                success(read);
                live_channels(&ctl::json(read), &mut self.channels);
                self.live = Some(json!({"chat_act":self.ran.len()}));
                self.stage = Stage::Acts;
            }
            Stage::Forge
            | Stage::Llm
            | Stage::Engine
            | Stage::Ws
            | Stage::S1
            | Stage::S2
            | Stage::BeforeAdd
            | Stage::Added { .. }
            | Stage::Placed
            | Stage::BeforeStop
            | Stage::Stopping
            | Stage::Starting { .. }
            | Stage::Provision
            | Stage::ReadRules
            | Stage::ChangeSettings
            | Stage::Live
            | Stage::Accounts
            | Stage::Stop => {}
        }
        self.release_placed(now);
    }
    fn observe(&mut self, now: Time, roots: &[cluster::Root]) {
        self.members.observe(now, &roots[..roots.len() - 1]);
        if self.members.failed() {
            // Once drained, report the member's failure before checking the story's cast.
            let _ = self.members.passed();
            return;
        }
        if let Some(stopped) = self.members.stopped() {
            assert_eq!(
                stopped.iter().map(|member| member.name.as_str()).collect::<Vec<_>>(),
                if self.cast.len() == 1 {
                    vec!["ws", "engine", "llm", "forge"]
                } else if self.late {
                    vec!["s2", "s1", "ws", "engine", "llm", "forge"]
                } else {
                    // ws's first life by its act, then the cast in reverse start order: ws's second life last started.
                    vec!["ws", "ws", "s2", "s1", "engine", "llm", "forge"]
                }
            );
            for &worker in &self.cast {
                assert_eq!(
                    self.members.linked(worker),
                    if self.cast.len() > 1 && !self.late && worker == "ws" { 2 } else { 1 },
                    "{worker} linked once per life"
                );
            }
            for member in stopped {
                if self.cast.contains(&member.name.as_str()) {
                    assert_eq!(
                        member.stderr.split(|byte| *byte == b'\n').filter(|line| line.starts_with(b"linked ")).count(),
                        1,
                        "{} life {} linked once",
                        member.name,
                        member.life
                    );
                }
                assert_eq!(
                    member.exit,
                    Some(Exit::Code(0)),
                    "{}: {}",
                    member.name,
                    String::from_utf8_lossy(&member.stderr)
                );
                assert!(!member.killed && member.survivors.is_empty(), "no surviving process");
            }
            if !self.stopped {
                for member in ["llm", "forge"] {
                    let bytes = fs::read(self.members.dir(member).join("observations.jsonl")).expect("final records");
                    let last =
                        bytes.split(|byte| *byte == b'\n').rfind(|line| !line.is_empty()).expect("final checkpoint");
                    assert_eq!(
                        serde_json::from_slice::<Value>(last).expect("checkpoint"),
                        json!({"observations_lost":0})
                    );
                }
                if self.late {
                    self.check_workspaces();
                } else {
                    self.check_tokens();
                }
                self.stopped = true;
                self.mark("stop", now);
                eprintln!("case processes: {:?}", self.phases);
            }
        }
    }
    fn next_deadline(&self) -> Option<Time> {
        Some(self.members.next_deadline().into_iter().chain(self.placed_release).fold(self.deadline, Time::min))
    }
    fn overdue(&self, now: Time) -> Option<String> {
        (now >= self.deadline).then(|| {
            let flight = self
                .ran
                .last()
                .map_or_else(|| "none".into(), |step| format!("{}: {}", self.ran.len() - 1, describe(step)));
            let next = self
                .steps
                .front()
                .map_or_else(|| "none".into(), |step| format!("{}: {}", self.ran.len(), describe(step)));
            let message = format!("case processes waited in {:?}; act in flight {flight}; next act {next}", self.stage);
            eprintln!("{message}");
            message
        })
    }
    fn passed(&self) -> bool {
        self.members.passed()
    }
}
fn describe(step: &calls::Step) -> String {
    use calls::Step;
    match step {
        Step::EndsLeaving(leftovers) => format!("EndsLeaving {leftovers:?}"),
        Step::FixtureReady => "FixtureReady".into(),
        Step::Browse { call } => format!("Browse call {call}"),
        Step::Authorize => "Authorize".into(),
        Step::Return => "Return".into(),
        Step::Terminal(_) => "Terminal".into(),
        Step::ReleaseHold { name } => format!("ReleaseHold {}", String::from_utf8_lossy(name)),
        Step::Advance => "Advance".into(),
        Step::Push { path, .. } => format!("Push {}", String::from_utf8_lossy(path)),
        Step::AddWorker { name } => format!("AddWorker {name}"),
        Step::StopWorker { name } => format!("StopWorker {name}"),
        Step::StartWorker { name } => format!("StartWorker {name}"),
        Step::Probe { at } => format!("Probe at {}", at.0),
        Step::Release { at } => format!("Release at {}", at.0),
        Step::Contended { calls } => format!("Contended {calls:?}"),
        Step::SignIn { at } => format!("SignIn at {}", at.0),
        Step::SameParty { at } => format!("SameParty at {}", at.0),
        Step::Held { name } => format!("Held {}", String::from_utf8_lossy(name)),
        Step::Kill { call } => format!("Kill call {call}"),
        Step::Call { at, argv } => format!("Call at {}: {}", at.0, describe_argv(argv)),
        Step::Start { at, argv } => format!("Start at {}: {}", at.0, describe_argv(argv)),
        Step::Join { call } => format!("Join call {call}"),
        Step::Watching { call } => format!("Watching call {call}"),
        Step::Until { at, argv, holds, early } => format!(
            "Until at {}: {}; holds {:?}; early {:?}",
            at.0,
            describe_argv(argv),
            holds,
            String::from_utf8_lossy(early),
        ),
    }
}
fn describe_argv(argv: &calls::Argv) -> String {
    argv.0
        .iter()
        .map(|arg| match arg {
            calls::Arg::Lit(word) => (*word).into(),
            calls::Arg::GithubApp => "github-app".into(),
            calls::Arg::Number(number) => number.to_string(),
            calls::Arg::File(name) => format!("file({name})"),
            calls::Arg::Task(call) => format!("task(call {call})"),
            calls::Arg::Delegate(call) => format!("delegate(call {call})"),
            calls::Arg::Digest(call) => format!("digest(call {call})"),
            calls::Arg::ActivityBefore(call) => format!("activity-before(call {call})"),
            calls::Arg::InboxAfter(call) => format!("inbox-after(call {call})"),
        })
        .collect::<Vec<_>>()
        .join(" ")
}
fn success(call: &Ended) {
    assert_eq!(
        call.exit,
        Some(Exit::Code(0)),
        "call: {} {}",
        String::from_utf8_lossy(&call.stderr),
        String::from_utf8_lossy(&call.stdout)
    );
}
impl Judge {
    fn collect(&mut self, accounts: &Value) {
        let path = self.members.dir("llm").join("observations.jsonl");
        let mut told = Vec::new();
        let mut checkpoints = 0;
        for line in fs::read(path).expect("LLM records").split(|byte| *byte == b'\n').filter(|line| !line.is_empty()) {
            use temper_fake::llm::observations::Observation;
            match temper_fake::llm::observations::decode(line).expect("query record") {
                Observation::Query(query) => {
                    if let Some(query) = Told::new(&query.system, &query.model, query.credential, &query.messages) {
                        told.push(query);
                    }
                }
                Observation::Checkpoint { lost } => {
                    assert_eq!(lost, 0, "lossless query stream");
                    checkpoints += 1;
                }
                Observation::Full { .. } => panic!("LLM record full"),
            }
        }
        assert!(checkpoints > 0, "query checkpoint");
        let rows: Vec<Value> = fs::read_to_string(self.members.dir("forge").join("observations.jsonl"))
            .expect("forge records")
            .lines()
            .map(|line| serde_json::from_str(line).expect("forge JSON"))
            .collect();
        assert_eq!(rows.last().expect("forge checkpoint")["observations_lost"], 0);
        let kit = if self.late {
            let merged = rows
                .iter()
                .rev()
                .find(|row| row["observation"] == "merged" && row["repository"] == "acme/app")
                .expect("kit merged");
            let head = merged["head"].as_u64().expect("merged head");
            let merge = merged["commit"].as_u64().expect("merge commit");
            let mut checks = BTreeMap::new();
            for row in &rows {
                if std::ptr::eq(row, merged) {
                    break;
                }
                if row["observation"] == "reported" && row["repository"] == "acme/app" && row["commit"] == head {
                    checks.insert(row["context"].as_str().expect("check context"), row["state"] == "passed");
                }
            }
            let green = !checks.is_empty() && checks.values().all(|passed| *passed);
            let address = self.members.capture("forge", "address").expect("forge address").parse().expect("socket");
            let objects = super::github::objects(address, merge);
            let object = objects["objects"]
                .as_array()
                .expect("objects")
                .iter()
                .find(|object| object["commit"] == merge)
                .expect("merge object");
            let main = object["tree"]
                .as_array()
                .expect("main tree")
                .iter()
                .map(|file| (decode(&file["path"]), decode(&file["content"])))
                .collect();
            Kit { records: Vec::new(), main, merged_green: green, pulls: Vec::new() }
        } else {
            assert!(!rows.iter().any(|row| row["observation"] == "merged"), "restart story merges nothing");
            Kit { records: Vec::new(), main: BTreeMap::new(), merged_green: false, pulls: Vec::new() }
        };
        for call in &self.calls {
            success(call);
        }
        if !self.placed.is_empty() {
            self.map_channels(accounts);
        }
        let workers = std::mem::take(&mut self.workers);
        *self.run.borrow_mut() = Some(Run {
            calls: std::mem::take(&mut self.calls),
            observed_at: std::mem::take(&mut self.observed_at),
            told,
            kit,
            workers,
            steps: std::mem::take(&mut self.ran),
            first_workers: std::mem::take(&mut self.first_workers),
        });
    }
}
impl Judge {
    fn check_tokens(&self) {
        let token = &self.admin_token;
        assert!(!token.is_empty());
        for &name in &self.cast {
            let dir = self.members.dir(name);
            let worker = read(&dir.join("config/temper.json"));
            let secret = worker["workers"][0]["secret"].as_str().expect("link secret");
            for path in [dir.join("config/temper.json"), dir.join("config/agent.json"), dir.join("data").join(secret)] {
                let bytes = fs::read(&path).expect("worker input");
                assert!(
                    !bytes.windows(token.len()).any(|window| window == token),
                    "{name}: forge admin token in {}",
                    path.display()
                );
            }
        }
    }
    fn check_workspaces(&self) {
        use temper_real_loop_world::case;
        let workspaces = |worker| {
            let directory = self.members.dir(worker);
            let document = read(&directory.join("config/temper.json"));
            let root = document["application"]["workers"][0]["root"].as_str().expect("worker root");
            directory.join("data").join(root).join("workspaces")
        };
        assert!(fs::read_dir(workspaces("ws")).expect("ws workspaces").next().is_none(), "ws holds no workspace");
        assert!(
            fs::read_dir(workspaces("s1")).expect("s1 workspaces").any(|entry| {
                let path = entry.expect("workspace").path().join("app/docs/design/export.md");
                fs::read(path).is_ok_and(|words| words == case::DESIGN_WORDS.as_bytes())
            }),
            "s1's kit workspace holds the design"
        );
    }
    fn map_channels(&mut self, accounts: &Value) {
        let deployment = accounts
            .pointer("/accounts/deployment_facts/program_hosts")
            .and_then(Value::as_array)
            .expect("worker channels");
        let party =
            accounts.pointer("/accounts/program_hosts").and_then(Value::as_array).expect("party worker channels");
        // Both lists are supplied host facts in this one read; rendering may cut the later list.
        self.channels.retain(|task, channel| {
            let Some(host) = deployment.iter().chain(party).find(|host| host["channel"] == *channel) else {
                return true;
            };
            let name = host["name"].as_str().expect("worker name");
            self.assigned.insert(name.into());
            if self.placed.contains(task) {
                self.workers.insert(*task, name.into());
            }
            false
        });
    }
}

fn decode(value: &Value) -> Vec<u8> {
    base64::decode(
        base64::Alphabet::Standard,
        value.as_str().expect("base64").as_bytes(),
        temper_fake::forge::LIMITS.limits.content_bytes,
    )
    .expect("file bytes")
    .into_vec()
}
fn live_channels(value: &Value, channels: &mut BTreeMap<u64, u64>) {
    if let (Some(task), Some(channel)) = (
        value.get("task").and_then(Value::as_u64),
        value.pointer("/details/owner/details/observation/channel").and_then(Value::as_u64),
    ) {
        channels.insert(task, channel);
    }
    for child in
        value.as_object().into_iter().flat_map(serde_json::Map::values).chain(value.as_array().into_iter().flatten())
    {
        live_channels(child, channels);
    }
}

fn has_holds(value: &Value) -> bool {
    value.get("hold").is_some_and(|hold| !hold.is_null())
        || value
            .as_object()
            .into_iter()
            .flat_map(serde_json::Map::values)
            .chain(value.as_array().into_iter().flatten())
            .any(has_holds)
}
fn release(address: SocketAddr) {
    let within = std::time::Duration::from_secs(10);
    let mut socket = TcpStream::connect_timeout(&address, within).expect("PLACED control connect");
    socket.set_read_timeout(Some(within)).expect("control read bound");
    socket.set_write_timeout(Some(within)).expect("control write bound");
    let name = text(temper_real_loop_world::cluster::PLACED);
    write!(
        socket,
        "POST /release/{name} HTTP/1.1\r\nHost: {address}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )
    .expect("PLACED release");
    let mut response = Vec::new();
    socket.take(4096).read_to_end(&mut response).expect("PLACED response");
    assert!(response.starts_with(b"HTTP/1.1 204 "), "PLACED release: {}", String::from_utf8_lossy(&response));
}

// Managed setup creates the bot but leaves each existing repository's access to
// its owner. Supply the same collaborator grant the former fake setup supplied.
fn permit_repository(address: SocketAddr, token: &[u8]) {
    let within = std::time::Duration::from_secs(10);
    let mut socket = TcpStream::connect_timeout(&address, within).expect("fixture forge");
    socket.set_read_timeout(Some(within)).expect("read bound");
    socket.set_write_timeout(Some(within)).expect("write bound");
    let body = br#"{"permission":"write"}"#;
    write!(socket, "PUT /api/v1/repos/acme/app/collaborators/temper HTTP/1.1\r\nHost: {address}\r\nAuthorization: token {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", text(token), body.len()).expect("collaborator head");
    socket.write_all(body).expect("collaborator body");
    let mut response = Vec::new();
    socket.take(65_537).read_to_end(&mut response).expect("collaborator response");
    assert!(response.len() <= 65_536);
    assert!(response.starts_with(b"HTTP/1.1 204 "), "fixture bot collaborator: {}", String::from_utf8_lossy(&response));
}
