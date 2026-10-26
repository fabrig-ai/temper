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
    llm_with(config, scripts())
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
    llm_with(config, scripts.into_boxed_slice())
}
fn llm_with(config: &Path, scripts: Box<[Script]>) -> CtlLlm {
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
fn scripts() -> Box<[Script]> {
    // work/hello is forge 3's first repository; playground/widget is on forge 1.
    let repository = temper_engine_domain_forge::tag(3, 1);
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
                        "authority":{"tools":1,"grants":[grant(2),grant(3),grant(4),grant(10),grant(11),grant(12)],
                            "delegation":{"kinds":[{"kind":"agent","number":2}],"tasks":4,"depth":2},
                            "budget":{"spend":10},"notes":0}
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
    ])
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
        Step::Run(Call::new(&["add-key", "1", key.to_str().expect("key path"), "--key", "first-repository-llm"])),
        Step::Run(Call::new(&["wait", "accounts", "--until", "usable", "--timeout", "10s"])),
        // forge-setup makes the repository a project of its own (tools.md 5.5).
        Step::Run(Call::new(&[
            "chat",
            "--project",
            &project,
            "--key",
            "first-hello",
            &format!("@first-repository Land a README change in {repository}"),
            "--json",
        ])),
        Step::Run(Call::new(&["wait", "task", &chat, "--until", "replied", "--timeout", "120s", "--json"])),
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
        Step::Run(Call::new(&["wait", "task", &change, "--until", "ended", "--timeout", "120s", "--json"])),
        Step::Mark("first change ended"),
        Step::Run(Call::new(&["task", &change])),
        Step::Run(Call::new(&["task", &producer, "--json"])),
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
        let mut arguments = vec!["activity", task, "--limit", "32", "--json"];
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
            let ended = super::ctl::alone(client, Call::new(&["turn", task, &name]));
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
        panel: super::ctl::alone(client, Call::new(&["change", &task.change.to_string(), "--json"])),
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
