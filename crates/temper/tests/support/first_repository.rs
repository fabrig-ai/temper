//! The first owned repository's task, using only temper's client reads.
use super::{
    boxed::CtlLlm,
    ctl::{Call, Step},
    write_private,
};
use serde_json::{Value, json};
use skein_fake_llm_domain::api::{Finish, Line, Script, Turn};
use std::{fs, path::Path};

#[must_use]
pub fn llm(config: &Path) -> CtlLlm {
    model(config, scripts())
}
/// The copy uses the same procedure, with its own README and a read of its source.
#[must_use]
pub fn playground_llm(config: &Path) -> CtlLlm {
    let mut scripts = scripts().into_vec();
    let Line::Call { arguments, .. } = &mut scripts[0].turns[0].lines[0] else { panic!("delegate call") };
    let mut delegation: Value = serde_json::from_slice(arguments).expect("delegate JSON");
    delegation["batch"][0]["spec"]["words"] = "@hello-readme Edit playground/widget README.md".into();
    *arguments = serde_json::to_vec(&delegation).expect("delegate JSON").into_boxed_slice();
    let mut turns = std::mem::take(&mut scripts[1].turns).into_vec();
    turns.insert(2, call(b"read", &json!({"path":"src/main.rs"})));
    turns[3] = call(
        b"edit",
        &json!({"path":"README.md","edits":[{
            "old":"# widget","new":"# widget\n\nChanged by temper."
        }]}),
    );
    scripts[1].turns = turns.into_boxed_slice();
    model(config, scripts.into_boxed_slice())
}
fn model(config: &Path, scripts: Box<[Script]>) -> CtlLlm {
    let config = config.to_path_buf();
    let peer = super::fake_llm(scripts);
    let address = peer.address();
    CtlLlm {
        peer,
        expected_exits: Vec::new(),
        on_link: Box::new(move || {
            // The box starts on empty directories. Only its generated agent's
            // endpoint moves, before any run: the template and demo stay shipped.
            let path = config.join("agent.json");
            let mut agent: Value = serde_json::from_slice(&fs::read(&path).expect("initialized agent")).expect("JSON");
            agent["endpoints"][0]["address"] = address.to_string().into();
            agent["endpoints"][0]["server_name"] = "skein.test".into();
            agent["endpoints"][0]["trust_der"] = json!(config.join("root.der"));
            write_private(&config.join("root.der"), skein_tls_world::pki::ROOT);
            write_private(&path, &serde_json::to_vec(&agent).expect("agent JSON"));
        }),
    }
}
fn call(name: &[u8], value: &Value) -> Turn {
    Turn {
        lines: Box::new([Line::Call {
            name: name.into(),
            arguments: serde_json::to_vec(value).expect("tool JSON").into_boxed_slice(),
        }]),
        finish: Finish::ToolCalls,
        tokens: 20,
    }
}

/// Price a process story's purse from init's default model and stock run cap.
#[must_use]
pub(super) fn run_spend(runs: u64) -> u64 {
    let mut template: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../../temper-init/template/temper.json")).expect("init template");
    // Only the default model prices this story; the demo's socket is filled by init.
    for member in ["providers", "endpoints", "models"] {
        template["engine"]["llm"][member].as_array_mut().expect("model configuration").truncate(1);
    }
    let bytes = serde_json::to_vec(&template).expect("single-model template");
    let document = jig_protocol_config::document(
        &bytes,
        &jig_protocol_config::DocumentLimits { bytes: 1_048_576, tokens: 65_536, depth: 32 },
    )
    .expect("init document");
    let at = jig_protocol_config::read::Object::new(&document, 0)
        .expect("init object")
        .member(&document, b"engine")
        .expect("init engine");
    let engine = temper_service::startup::engine(&document, at, &jig_protocol_config::Directories::working())
        .expect("init engine settings");
    // Price the story in stock run units, then round its fixture purse
    // to the least power of two covering those runs.
    engine
        .rules
        .maximum_run_spend
        .checked_mul(runs)
        .expect("stock runs")
        .checked_next_power_of_two()
        .expect("bounded story spend")
}

fn scripts() -> Box<[Script]> {
    // work/hello is forge 3's first repository; playground/widget is on forge 1.
    let repository = temper_engine_domain_forge::tag(3, 1);
    let spend = run_spend(5);
    let grant = |kind| json!({"connector":1,"kind":kind,"segments":["forge"],"terminal":"open","last":""});
    Box::new([
        Script {
            cue: b"@first-repository".as_slice().into(),
            turns: Box::new([
                call(
                    b"delegate",
                    &json!({"batch":[{
                        "executor":{"kind":"procedure","connector":1,"code":2},
                        "spec":{"words":"@hello-readme Edit work/hello README.md","parameters":[
                            {"name":1,"kind":"resource","connector":1,"resource":repository},
                            {"name":2,"kind":"bytes","value":"main"}]},
                        "contract":{"kind":"change","connector":1,"change_kind":1,"words":128},
                        "authority":{"tools":513,"grants":[grant(1),grant(2),grant(3),grant(4),grant(10),grant(11),grant(12)],
                            "delegation":{"kinds":[{"kind":"agent","number":2},{"kind":"agent","number":4}],"tasks":4,"depth":2},
                            "budget":{"spend":spend},"notes":0}
                    }]}),
                ),
                call(b"wait", &json!({})),
                call(b"finish", &json!({"form":"report","text":"work/hello landed.","fields":{}})),
            ]),
        },
        Script {
            cue: b"@hello-readme".as_slice().into(),
            turns: Box::new([
                call(b"read", &json!({"path":"README.md"})),
                call(b"read", &json!({"path":".forgejo/workflows/ci.yml"})),
                call(
                    b"edit",
                    &json!({"path":"README.md","edits":[{"old":"# hello","new":"# hello\n\nChanged by temper."}]}),
                ),
                call(
                    b"finish",
                    &json!({"form":"change","fields":{"title":"Update hello README","body":"Changed by temper."}}),
                ),
            ]),
        },
        review_script(),
    ])
}

#[must_use]
pub fn review_script() -> Script {
    Script {
        cue: b"Review this change at the named head".as_slice().into(),
        turns: Box::new([call(
            b"finish",
            &json!({
                "form":"verdict", "label":"1", "text":"Checked", "fields":{}, "items":[]
            }),
        )]),
    }
}
/// Exercise init's landing rule with the playground's CSV request on real git.
#[must_use]
pub fn csv_llm(config: &Path) -> CtlLlm {
    let mut scripts = scripts();
    scripts[0].cue = b"Add CSV export to widget".as_slice().into();
    let Line::Call { arguments, .. } = &mut scripts[0].turns[0].lines[0] else { panic!("delegate call") };
    let mut delegate: Value = serde_json::from_slice(arguments).expect("delegate JSON");
    delegate["batch"][0]["spec"]["words"] = "@hello-readme Add CSV export to widget in work/hello".into();
    *arguments = serde_json::to_vec(&delegate).expect("delegate JSON").into_boxed_slice();
    let mut coder = scripts[1].turns.to_vec();
    let template: Value =
        serde_json::from_str(include_str!("../../../temper-init/template/temper.json")).expect("shipped template");
    let edit = &template["application"]["demo"]["llm"]["cued"][0]["turns"][1]["lines"][0]["arguments"]["edits"][0];
    let seed = std::str::from_utf8(temper_fake::forge::FILES[1].1).expect("widget source");
    let source = seed.replace(edit["old"].as_str().expect("old main"), edit["new"].as_str().expect("CSV main"));
    let _ = coder.pop().expect("coder finish");
    let finish = call(
        b"finish",
        &json!({"form":"change", "fields":{
            "title":"Add CSV export to widget", "body":"Widget prints its table with --csv."
        }}),
    );
    coder.extend([
        call(b"write", &json!({"path":"src/main.rs", "content":source})),
        call(b"write", &json!({"path":"Cargo.toml", "content":"[package]\nname = \"widget\"\nversion = \"0.1.0\"\nedition = \"2024\"\n"})),
        finish,
    ]);
    scripts[1].turns = coder.into_boxed_slice();
    // The fake streams this review until the story has read the pending gate.
    let verdict = scripts[2].turns[0].clone();
    let mut read = call(b"read", &json!({"path":"src/main.rs"}));
    let mut lines = read.lines.into_vec();
    lines.insert(
        0,
        Line::Pieces {
            pieces: Box::new([skein_fake_llm_domain::api::Piece {
                text: b"Checked widget CSV export.".as_slice().into(),
                hold: Some(b"csv-review".as_slice().into()),
            }]),
        },
    );
    read.lines = lines.into_boxed_slice();
    scripts[2].turns = Box::new([read, verdict]);
    model(config, scripts)
}

/// Capture the pending review before allowing the scripted verdict to finish.
#[must_use]
pub fn csv_task_steps(root: &Path) -> Vec<Step> {
    let mut steps = task_steps(root);
    steps[2] = Step::Run(Call::new(&[
        "chat",
        "start",
        "--project",
        "2",
        "--key",
        "first-hello",
        "Add CSV export to widget",
        "--json",
    ]));
    let found = steps.split_off(5);
    steps.extend([
        Step::Run(Call::new(&["change", "show", "2", "--until", "ci:passed", "--timeout", "60s", "--json"])),
        Step::Until(Call::new(&["change", "show", "2", "--json"]), br#""verdict":"pending""#),
        Step::Then(Box::new(|calls| {
            let panel = super::ctl::json(calls.last().expect("pending panel"));
            assert_eq!(panel["ci"], "passed", "review follows head CI: {panel}");
            assert_eq!(panel["gates"], json!([{"number":1,"kind":"review","blocks":true,"verdict":"pending"}]));
            assert_eq!(panel["step"], "gating");
            assert!(panel["merge"].is_null(), "review blocks landing: {panel}");
            let task = panel["runs"].as_u64().expect("the pending gate names its reviewer's task");
            Call::new(&["task", "show", &task.to_string(), "--json"])
        })),
        Step::Then(Box::new(|calls| {
            let reviewer = super::ctl::json(calls.last().expect("reviewer's task"));
            let pending = super::ctl::json(&calls[calls.len() - 2]);
            assert_eq!(reviewer["chip"]["task"], pending["runs"]);
            assert_eq!(reviewer["chip"]["executor"], json!({"agent":{"charter":4}}));
            assert_ne!(reviewer["chip"]["phase"], "ended", "reviewer still runs: {reviewer}");
            Call::new(&["settings", "show", "charter", "4", "--json"])
        })),
        Step::Then(Box::new(|calls| {
            let charter = super::ctl::json(calls.last().expect("review charter"));
            assert_eq!(charter["value"]["instructions"], include_str!("../../../temper-service/src/stock/review.txt"));
            assert_eq!(charter["value"]["modify"], false);
            Call::new(&["change", "show", "2", "--json"])
        })),
        Step::ReleaseLlm(b"csv-review"),
        Step::Until(Call::new(&["change", "show", "2", "--json"]), br#""verdict":"approved""#),
        Step::Then(Box::new(|calls| {
            let approved = super::ctl::json(calls.last().expect("approved panel"));
            let pending = calls
                .iter()
                .filter_map(|call| serde_json::from_slice::<Value>(&call.stdout).ok())
                .find(|panel| panel["gates"][0]["verdict"] == "pending")
                .expect("pending review");
            assert_eq!(approved["head"], pending["head"], "review is approved at the CI head");
            assert_eq!(approved["ci"], "passed");
            Call::new(&["change", "show", "2", "--until", "landed", "--timeout", "120s", "--json"])
        })),
    ]);
    steps.extend(found);
    steps
}

/// The first repository has its own project; earlier demo tasks may precede its task tree.
#[derive(Clone, Copy)]
pub struct Task {
    pub project: u64,
    pub chat: u64,
    pub change: u64,
    pub producer: u64,
}
const FIRST: Task = Task { project: 2, chat: 1, change: 2, producer: 3 };

#[must_use]
pub fn task_steps(root: &Path) -> Vec<Step> {
    task_steps_for(root, FIRST)
}
#[must_use]
pub fn task_steps_for(root: &Path, task: Task) -> Vec<Step> {
    task_steps_in(root, task, "work/hello")
}
#[must_use]
pub fn playground_task_steps(root: &Path) -> Vec<Step> {
    task_steps_in(root, FIRST, "playground/widget")
}
fn task_steps_in(root: &Path, task: Task, repository: &str) -> Vec<Step> {
    let project = task.project.to_string();
    let chat = task.chat.to_string();
    let key = root.join("llm-key");
    write_private(&key, jig_channel_world::fixtures::KEY);
    vec![
        Step::Run(Call::new(&[
            "account",
            "add-key",
            "1",
            key.to_str().expect("key path"),
            "--key",
            "first-repository-llm",
        ])),
        Step::Run(Call::new(&["account", "list", "--until", "usable", "--timeout", "10s"])),
        // forge-setup makes the repository a project of its own (tools.md 5.5).
        Step::Run(Call::new(&[
            "chat",
            "start",
            "--project",
            &project,
            "--key",
            "first-hello",
            &format!("@first-repository Land a README change in {repository}"),
            "--json",
        ])),
        Step::Run(Call::new(&["task", "show", &chat, "--until", "replied", "--timeout", "120s", "--json"])),
        Step::Mark("first task's reply"),
    ]
    .into_iter()
    .chain(found_steps_for(task))
    .collect()
}

/// The same task and run reads on either box, without giving a second task.
#[must_use]
pub fn found_steps() -> Vec<Step> {
    found_steps_for(FIRST)
}
#[must_use]
pub fn found_steps_for(task: Task) -> Vec<Step> {
    let change = task.change.to_string();
    let producer = task.producer.to_string();
    vec![
        Step::Run(Call::new(&["task", "show", &change, "--until", "ended", "--timeout", "120s", "--json"])),
        Step::Mark("first change ended"),
        Step::Run(Call::new(&["task", "show", &change])),
        Step::Run(Call::new(&["task", "show", &producer, "--json"])),
    ]
}

/// Read the committed calls through ctl while the box is still running.
/// Keep assertion of their outcome until after the box and fake have drained.
#[must_use]
pub fn turns(client: &Path, task: &str) -> String {
    use skein_io::kernel::Exit;
    let mut before = None::<String>;
    let mut words = String::new();
    // The shipped document window may return fewer than --limit rows.
    for _ in 0..32 {
        let mut arguments = vec!["task", "activity", task, "--limit", "32", "--json"];
        if let Some(before) = &before {
            arguments.extend(["--before", before.as_str()]);
        }
        let activity = super::ctl::alone(client, Call::new(&arguments));
        assert_eq!(activity.exit, Some(Exit::Code(0)));
        let activity = super::ctl::json(&activity);
        for row in activity["rows"].as_array().expect("activity rows") {
            let Some(turn) = row.pointer("/event/owner/source/line/turn") else { continue };
            let name =
                format!("{}.{}", turn["attempt"].as_u64().expect("attempt"), turn["turn"].as_u64().expect("turn"));
            let ended = super::ctl::alone(client, Call::new(&["task", "turn", task, &name]));
            assert_eq!(ended.exit, Some(Exit::Code(0)), "{}", String::from_utf8_lossy(&ended.stderr));
            words.push_str(&String::from_utf8(ended.stdout).expect("turn UTF-8"));
        }
        before = activity.pointer("/before/value").and_then(Value::as_str).map(str::to_owned);
        if before.is_none() {
            return words;
        }
    }
    panic!("task {task}'s activity exceeded 32 pages")
}

/// Evidence taken through temper, plus the existing anonymous version probe.
pub struct Evidence {
    pub change: u64,
    pub address: std::net::SocketAddr,
    pub version: std::io::Result<Value>,
    pub delegation: String,
    pub producer: String,
    pub page: String,
    pub panel: super::Ended,
}
#[must_use]
pub fn live(config: &Path, data: &Path, client: &Path) -> Evidence {
    live_for(config, data, client, FIRST)
}

/// The process story uses only the API and header paths printed for the person,
/// after their forge's started line, before consulting the normal ctl evidence.
#[must_use]
pub fn live_from_hands(config: &Path, data: &Path, client: &Path, stderr: &[u8]) -> Evidence {
    let words = std::str::from_utf8(stderr).expect("box terminal UTF-8");
    let mut lines = words.lines().skip_while(|line| !line.ends_with("Forge 3 (Forgejo): started"));
    lines.next().expect("owned forge started");
    let api = lines.next().expect("API line after started").split_once("Forge 3 (Forgejo): api ").expect("API label").1;
    let person =
        lines.next().expect("person line after API").split_once("Forge 3 (Forgejo): person ").expect("person label").1;
    let (login, headers) = person.split_once(", read header ").expect("read header path");
    let (read, write) = headers.split_once(", write header ").expect("write header path");
    let document: Value =
        serde_json::from_slice(&fs::read(config.join("temper.json")).expect("configuration")).expect("JSON");
    let setup = document["application"]["setup"]
        .as_array()
        .expect("setup")
        .iter()
        .find(|entry| entry["forge"] == 3)
        .expect("forge 3 setup");
    assert_eq!(Some(login), setup["mode"]["owned"]["person"]["login"].as_str(), "init chose the person");
    let address: std::net::SocketAddr = api
        .strip_prefix("http://")
        .and_then(|url| url.strip_suffix("/api/v1"))
        .expect("local API URL")
        .parse()
        .expect("listener address");
    for (header, name) in [(read, "person_read.header"), (write, "person_write.header")] {
        assert!(Path::new(header).is_absolute(), "printed header path must be absolute");
        assert_eq!(Path::new(header), data.join("forge-3").join(name));
        let (status, body) = header_request(api, header, "GET", "/user", None);
        assert_eq!(status, 200, "person header must authenticate: {body}");
        assert_eq!(body["login"].as_str(), Some(login));
    }
    let (status, body) =
        header_request(api, read, "POST", "/repos/work/hello/issues", Some(r#"{"title":"read-header-must-refuse"}"#));
    assert_eq!(status, 403, "read header must refuse a write: {body}");
    let evidence = live(config, data, client);
    assert_eq!(evidence.address, address, "the printed API is this forge's listener");
    evidence
}

fn header_request(api: &str, header: &str, method: &str, route: &str, body: Option<&str>) -> (u16, Value) {
    let mut command = std::process::Command::new("curl");
    command.args([
        "--silent",
        "--show-error",
        "--noproxy",
        "*",
        "--max-time",
        "5",
        "--header",
        &format!("@{header}"),
        "--request",
        method,
        "--write-out",
        "\n%{http_code}",
    ]);
    if let Some(body) = body {
        command.args(["--header", "Content-Type: application/json", "--data", body]);
    }
    let output = command.arg(format!("{api}{route}")).output().expect("curl with header by path");
    assert!(output.status.success(), "header request failed: {}", String::from_utf8_lossy(&output.stderr));
    let text = std::str::from_utf8(&output.stdout).expect("HTTP response UTF-8");
    let (json, status) = text.rsplit_once('\n').expect("HTTP status");
    (status.parse().expect("HTTP status number"), serde_json::from_str(json).expect("API JSON"))
}
#[must_use]
pub fn live_for(config: &Path, data: &Path, client: &Path, task: Task) -> Evidence {
    use std::os::unix::fs::PermissionsExt;
    let document: Value =
        serde_json::from_slice(&fs::read(config.join("temper.json")).expect("configuration")).expect("JSON");
    let forge = document["application"]["forges"]
        .as_array()
        .expect("forges")
        .iter()
        .find(|forge| forge["number"] == 3)
        .expect("forge 3");
    let address: std::net::SocketAddr =
        forge["api"]["socket"].as_str().expect("forge address").parse().expect("socket");
    let record = fs::metadata(data.join("box/started/3")).expect("started record");
    assert!(record.is_file(), "the box kept forge 3's started record");
    assert_eq!(record.permissions().mode() & 0o777, 0o600, "private started record");
    Evidence {
        change: task.change,
        address,
        version: super::owned::answer(address.port(), "/api/v1/version", None),
        delegation: turns(client, &task.chat.to_string()),
        producer: turns(client, &task.producer.to_string()),
        page: change_page(config, client, task.change),
        panel: super::ctl::alone(client, Call::new(&["change", "show", &task.change.to_string(), "--json"])),
    }
}

/// Read the change panel with the signed-in native client; the shell keeps
/// its credential private, just as ctl does.
#[must_use]
pub fn change_page(config: &Path, client: &Path, task: u64) -> String {
    use skein_shell::{Clock, Host};
    use skein_world::{Referee, real};
    use temper_client_shell_terminal::client::{NativeClient, Settings, Start};
    struct Judge<'a> {
        task: u64,
        opened: bool,
        closing: bool,
        page: &'a std::cell::Cell<String>,
        until: skein_lib::Time,
    }
    impl Referee<NativeClient> for Judge<'_> {
        fn act(&mut self, now: skein_lib::Time, clients: &mut [NativeClient]) {
            let client = &mut clients[0];
            if client.started() && !self.opened {
                client.go(temper_client_domain::Address::Task {
                    task: self.task,
                    filter: temper_client_domain::PlanFilter::All,
                });
                self.opened = true;
            }
            if self.closing {
                return;
            }
            let mut page = String::new();
            for node in client.tree().nodes().iter().filter(|node| !node.text.is_empty()) {
                for run in &node.text {
                    page.push_str(&String::from_utf8_lossy(&run.text));
                }
                page.push('\n');
            }
            if ((page.contains("work/hello") || page.contains("playground/widget")) && page.contains("CI at its head"))
                || now >= self.until
            {
                self.page.set(page);
                client.close();
                self.closing = true;
            }
        }
        fn observe(&mut self, _now: skein_lib::Time, _clients: &[NativeClient]) {}
        fn next_deadline(&self) -> Option<skein_lib::Time> {
            (!self.closing).then_some(self.until)
        }
        fn overdue(&self, _now: skein_lib::Time) -> Option<String> {
            None
        }
        fn passed(&self) -> bool {
            self.closing
        }
    }
    let document: Value = serde_json::from_slice(&fs::read(config.join("temper.json")).expect("configuration"))
        .expect("configuration JSON");
    let engine = document
        .pointer("/application/clients/listener/socket")
        .and_then(Value::as_str)
        .expect("clients listener")
        .parse()
        .expect("socket");
    let root = skein_shell::open_root(client).expect("client's parent root");
    let clock = Clock::new();
    let page = std::cell::Cell::new(String::new());
    let mut world = real::World::new(Judge {
        task,
        opened: false,
        closing: false,
        page: &page,
        until: clock.now().now.saturating_add(skein_lib::Duration::from_secs(10)),
    });
    world.spawn_with_fds(vec![root], || {
        NativeClient::new(
            Settings {
                engine: skein_io::Addr::Ip(engine),
                directory: b"client".as_slice().into(),
                limits: temper_client_shell_terminal::limits::native(),
            },
            root,
            skein_shell::effective_user(),
            91,
            Start::Token,
        )
        .expect("native reader")
    });
    let ended = world.run(&clock, skein_lib::Duration::from_secs(20));
    assert!(ended.procs.iter().all(Host::is_empty), "native reader drained");
    page.take()
}

/// Both boxes must expose the same landed change, its producer and head's CI.
pub fn assert_task(run: &str, calls: &[super::Ended], stderr: &str, evidence: &Evidence) {
    use skein_io::kernel::Exit;
    let Evidence { delegation, producer, page, panel, .. } = evidence;
    assert_eq!(panel.exit, Some(Exit::Code(0)), "{run}: {}", String::from_utf8_lossy(&panel.stderr));
    let panel = super::ctl::json(panel);
    assert_eq!(panel["pull"]["address"], format!("http://{}/work/hello/pulls/1", evidence.address), "{run}: {panel}");
    eprintln!("{run} producer's committed calls: {producer}");
    eprintln!("{run} change page: {page}");
    eprintln!("{run} first task's committed calls: {delegation}");
    assert!(
        delegation.contains(&format!("Delegated tasks: {}", evidence.change)),
        "work/hello's change must be admitted: {delegation}"
    );
    for (index, call) in calls.iter().enumerate() {
        assert_eq!(
            call.exit,
            Some(Exit::Code(0)),
            "{run} task call {index}: stdout {}; stderr {}; box {stderr}",
            String::from_utf8_lossy(&call.stdout),
            String::from_utf8_lossy(&call.stderr)
        );
    }
    let task = &calls[calls.len() - 2];
    let task = String::from_utf8_lossy(&task.stdout);
    eprintln!("{run} landed task: {task}");
    let (landed, _) = super::ctl::landed_result(&task);
    assert!(landed.starts_with("result: Landed · pull request #1 · merge commit "), "{run}: {task}");
    assert!(task.contains("on forge 3"), "{run}: {task}");
    assert!(!task.contains("pull request:"), "{run}: a cut address is left to the panel: {task}");
    assert!(producer.contains("\"path\":\"README.md\"}\nRead "), "the worker read hello's cloned README: {producer}");
    assert!(producer.contains("Edited 1 matches"), "the worker applied its README edit: {producer}");
    assert!(
        producer.contains("\"path\":\".forgejo/workflows/ci.yml\"}\nRead "),
        "the worker cloned the starter workflow: {producer}"
    );
    assert!(page.contains("work/hello"), "{run}: {page}");
    if run == "first" {
        assert!(page.contains("CI at its head\nPassed"), "{run}: starter CI must pass at the pull's head: {page}");
    } else {
        // A restart forgets CI at the head, and a landed change's is not read again (domain/forge.md, 8.8).
        assert!(page.contains("CI at its head\nNot read since the restart"), "{run}: a restart forgets CI: {page}");
    }
}
