//! The ladder's real-model walks (domain/testing.md, 2.7).
//!
//! Like `forge_land`'s oracle, this driver uses bounded child commands. Its box
//! outlives the test on interruption so proof-live's final return hook can still
//! export the held account before terminating the box. Credentials go to ctl
//! and curl by path alone. No bot credential or store is read by the driver.
use serde_json::{Value, json};
use skein_lib::{Duration, Time};

use std::{
    fs,
    io::Write,
    os::unix::{ffi::OsStrExt, fs::PermissionsExt},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};

const REQUEST: &str =
    "Add CSV export to widget. Before anything is changed, have the work tell me its plan and wait for my go-ahead.";
const ANSWER: &str = "Go ahead. The CSV's first line must be the header fruit,count.";
const WHOLE: u64 = 374_930;

fn now() -> Time {
    skein_shell::Clock::new().now().now
}

fn require(yes: bool, message: &str) -> Result<(), String> {
    if yes { Ok(()) } else { Err(message.to_owned()) }
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).expect("configuration bytes")).expect("configuration JSON")
}

fn write_json(path: &Path, value: &Value) {
    super::super::write_private(path, &serde_json::to_vec(value).expect("configuration JSON"));
}

/// Setup may print progress JSON before its final answer. Validate every
/// document and return the last one, retaining all of them in the transcript.
pub fn ctl_json(bytes: &[u8]) -> Result<Value, String> {
    let mut last = None;
    for document in serde_json::Deserializer::from_slice(bytes).into_iter::<Value>() {
        last = Some(document.map_err(|error| format!("ctl JSON: {error}"))?);
    }
    last.ok_or_else(|| "ctl returned no JSON document".into())
}

/// `render/outcome.rs` and `document/act_answers.rs`: the top-level `task`
/// names an operation target, not the task created by the owner's chat act.
pub fn created_chat(value: &Value) -> Result<u64, String> {
    let created = &value["outcome"]["done"]["owner"]["created"];
    let first = created["first"].as_u64().ok_or("chat created.first missing")?;
    require(created["last"] == first && first != 0, "one chat created")?;
    Ok(first)
}

/// `change.rs::json` wraps waited panels; accounts keep their specialized
/// flat envelope, while Jig's other waits retain their named subject.
pub fn waited(args: &[&str], value: Value) -> Result<Value, String> {
    require(value["met"] == true, "wait condition unmet")?;
    if args.starts_with(&["change", "show"]) {
        let panel = value.get("change").filter(|v| v.is_object()).ok_or("waited change panel missing")?;
        Ok(panel.clone())
    } else {
        Ok(value)
    }
}

/// `read_queries.rs::encode_task_page` keeps live and ended reads separate;
/// `EndedGroup::Rows` adds its own rows object around the ended page.
pub fn child_rows(live: &Value, ended: &Value) -> Result<Vec<Value>, String> {
    require(live["live_after"].is_null(), "whole live child set")?;
    let mut rows = live["live"].as_array().ok_or("live children missing")?.clone();
    match &ended["ended"] {
        Value::String(none) if none == "none" => {}
        Value::Object(group) => {
            let page = group.get("rows").ok_or("ended page missing")?;
            require(page["after"].is_null() && page["cut"] == false, "whole ended child set")?;
            rows.extend(page["rows"].as_array().ok_or("ended children missing")?.iter().cloned());
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => {
            return Err("ended children group missing".into());
        }
    }
    Ok(rows)
}

/// The guide's init and the existing owned-forge fixture, with QA's model/room
/// recipe (qa.md, 3; configuration.md, 13.8), before the first start.
pub fn deployment(paths: &[PathBuf; 3]) {
    let _ = super::live_deployment(paths, super::LiveModel::ChatGpt { account: None });
    let config = paths[0].join("temper.json");
    let agent = paths[0].join("agent.json");
    let mut value = read_json(&config);
    let mut worker = read_json(&agent);
    let old = value["engine"]["llm"]["models"][0]["name"].as_str().expect("first model").to_owned();
    for provider in value["engine"]["llm"]["providers"].as_array_mut().expect("providers") {
        for model in provider["models"].as_array_mut().expect("provider models") {
            if *model == old {
                *model = "gpt-6.1-sol".into();
            }
        }
    }
    value["engine"]["llm"]["models"][0]["name"] = "gpt-6.1-sol".into();
    value["engine"]["llm"]["models"][0]["output"] = 32_000.into();
    value["engine"]["llm"]["models"][0]["efforts"] = json!(["high", "medium", "low"]);
    worker["endpoints"][0]["models"][0]["name"] = "gpt-6.1-sol".into();
    worker["endpoints"][0]["models"][0]["output"] = 32_000.into();
    worker["endpoints"][0]["models"][0]["reasoning_item"] = 32_768.into();
    value["engine"]["limits"]["overrides"]["hub.turn_bytes"] = 1_479_864.into();
    value["engine"]["limits"]["overrides"]["hub.transcript_bytes"] = 5_919_472.into();
    value["workers"][0]["limits"]["overrides"]["hub.turn_bytes"] = 1_479_864.into();
    value["workers"][0]["limits"]["overrides"]["hub.transcript_bytes"] = 5_919_472.into();
    value["workers"][0]["limits"]["overrides"]["hub.turn_queue_bytes"] = 11_838_912.into();
    value["engine"]["limits"]["overrides"]["core.resume_bytes"] = 1_479_868.into();
    value["engine"]["limits"]["overrides"]["people.requests"] = 256.into();
    for (name, count) in [
        ("root.loads.loads", 32),
        ("root.journal.transcript_bytes", 1_479_868),
        ("root.loads.transcript_bytes", 1_479_864),
        ("root.loads.bytes", 1_480_112),
        ("root.loads.reply_bytes", 1_480_112),
    ] {
        value["application"]["limits"]["overrides"][name] = count.into();
    }
    // Walk 1's admitted declarations, not a phase-bound increase.
    value["engine"]["memory_bytes"] = 817_372_775_016_049_u64.into();
    value["workers"][0]["memory_bytes"] = 3_700_000_000_000_u64.into();
    assert_eq!(value["engine"]["llm"]["models"][0]["efforts"][0], "high");
    assert_eq!(value["engine"]["llm"]["models"][0]["output"], 32_000);
    assert_eq!(value["engine"]["limits"]["overrides"]["people.requests"], 256);
    assert_eq!(value["engine"]["limits"]["overrides"]["hub.turn_bytes"], 1_479_864);
    assert_eq!(value["application"]["limits"]["overrides"]["root.loads.loads"], 32);
    assert_eq!(worker["endpoints"][0]["models"][0]["name"], "gpt-6.1-sol");
    assert_eq!(worker["endpoints"][0]["models"][0]["output"], 32_000);
    assert_eq!(worker["endpoints"][0]["models"][0]["reasoning_item"], 32_768);
    write_json(&config, &value);
    write_json(&agent, &worker);
    // Startup names the full deficit, including the box's pipes. Memory is a
    // declaration derived from this revision's limits, not a timing allowance.
    let mut directories = super::super::configuration::directories(paths[0].parent().expect("deployment root"));
    directories.config = paths[0].as_os_str().as_bytes().into();
    directories.data = paths[1].as_os_str().as_bytes().into();
    directories.cache = paths[2].as_os_str().as_bytes().into();
    if let Err(message) = temper::boxed_envelope_in(&directories, &fs::read(&config).expect("configuration")) {
        let deficit = message
            .split_once("engine.memory_bytes: the worst case passes memory_bytes by ")
            .and_then(|(_, suffix)| suffix.strip_suffix(" bytes\n"))
            .and_then(|number| number.parse::<u64>().ok())
            .unwrap_or_else(|| panic!("B0 setup refused: {message}"));
        let before = value["engine"]["memory_bytes"].as_u64().expect("engine declaration");
        value["engine"]["memory_bytes"] = before.checked_add(deficit).expect("memory declaration fits").into();
        write_json(&config, &value);
    }
    temper::boxed_envelope_in(&directories, &fs::read(&config).expect("configuration"))
        .unwrap_or_else(|message| panic!("B0 setup refused: {message}"));
}

/// A private command transcript, with a hard wall bound and no pipe backpressure.
struct Driver {
    binary: PathBuf,
    store: PathBuf,
    client: PathBuf,
    sequence: usize,
    until: Time,
    phase: &'static str,
    requested: Option<Time>,
    api: String,
    header: PathBuf,
    project: String,
}

impl Driver {
    fn new(store: &Path, client: &Path) -> Self {
        Self {
            binary: skein_scratch::paths::binary("temper"),
            store: store.into(),
            client: client.into(),
            sequence: 0,
            until: now().saturating_add(Duration::from_secs(300)),
            phase: "setup",
            requested: None,
            api: String::new(),
            header: PathBuf::new(),
            project: String::new(),
        }
    }

    fn execute(&mut self, mut command: Command, input: Option<&str>, keep: bool) -> Result<Vec<u8>, String> {
        let number = self.sequence;
        self.sequence += 1;
        let stdout = self.store.join(format!("call-{number}.out"));
        let stderr = self.store.join(format!("call-{number}.err"));
        command
            .current_dir(&self.store)
            .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
            .stdout(fs::File::create(&stdout).map_err(|e| e.to_string())?)
            .stderr(fs::File::create(&stderr).map_err(|e| e.to_string())?);
        let mut child = command.spawn().map_err(|e| e.to_string())?;
        if let Some(input) = input {
            child
                .stdin
                .take()
                .ok_or("command stdin missing")?
                .write_all(input.as_bytes())
                .map_err(|e| e.to_string())?;
        }
        let status = loop {
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                break status;
            }
            if now() >= self.until {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("phase {} exceeded its bound; call {number}", self.phase));
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        require(now() <= self.until, &format!("phase {} exceeded its bound", self.phase))?;
        require(
            status.success(),
            &format!("phase {}: call {number} failed ({status}); see {}", self.phase, stderr.display()),
        )?;
        let bytes = fs::read(&stdout).map_err(|e| e.to_string())?;
        if !keep {
            fs::remove_file(stdout).map_err(|e| e.to_string())?;
        }
        Ok(bytes)
    }

    fn ctl(&mut self, args: &[&str], input: Option<&str>) -> Result<Value, String> {
        let mut command = if args.starts_with(&["account", "import"]) {
            let mut locked = Command::new("flock");
            locked
                .arg("-x")
                .arg(self.store.join("login.lock"))
                .args(["sh", "-c", "test ! -e \"$1\" || exit 1; shift; exec \"$@\"", "rung-import"])
                .arg(self.store.join("returning"))
                .arg(&self.binary);
            locked
        } else {
            Command::new(&self.binary)
        };
        command.arg("ctl").arg(&self.client).args(args).arg("--json");
        command.env_clear().env("PATH", "/usr/bin:/bin");
        let bytes = self.execute(command, input, true)?;
        ctl_json(&bytes).map_err(|error| format!("phase {}: {error}", self.phase))
    }

    fn forge(&mut self, route: &str) -> Result<Vec<u8>, String> {
        let mut command = Command::new("curl");
        command.args(["-fsS", "--max-time", "10", "-H"]).arg(format!("@{}", self.header.display()));
        command.arg(format!("{}/repos/playground/widget/{route}", self.api));
        self.execute(command, None, true)
    }

    fn forge_json(&mut self, route: &str) -> Result<Value, String> {
        serde_json::from_slice(&self.forge(route)?).map_err(|e| format!("forge JSON: {e}"))
    }

    fn phase(&mut self, phase: &'static str, milliseconds: u64) {
        self.phase = phase;
        self.until =
            self.requested.expect("request clock").saturating_add(Duration::from_nanos(milliseconds * 1_000_000));
    }

    fn met(&self, bound: u64) {
        super::super::live::notice(&format!(
            "PHASE {} {}ms <= {}ms from request",
            self.phase,
            now().saturating_since(self.requested.expect("request clock")).as_nanos() / 1_000_000,
            bound
        ));
    }

    fn wait(&mut self, args: &[&str], condition: &str) -> Result<Value, String> {
        let left = self.until.saturating_since(now()).as_nanos() / 1_000_000;
        require(left > 0, &format!("phase {} exceeded its bound", self.phase))?;
        // ctl accepts whole seconds. The child is still killed at this
        // driver's exact millisecond deadline, including its final fraction.
        let timeout = left.div_ceil(1_000).to_string();
        let args: Vec<_> = args.iter().copied().chain(["--until", condition, "--timeout", &timeout]).collect();
        let value = self.ctl(&args, None)?;
        waited(&args, value).map_err(|error| format!("phase {}: {condition}: {error}", self.phase))
    }

    /// Activity is a paged public read. Do not mistake a cut first page for the
    /// entire conversation, or a call argument for a message received by work.
    fn activity(&mut self, task: u64) -> Result<Vec<Value>, String> {
        let task = task.to_string();
        let mut cursor = None;
        let mut rows = Vec::new();
        for _ in 0..256 {
            let mut args = vec!["task", "activity", &task, "--limit", "32"];
            if let Some(before) = cursor.as_deref() {
                args.extend(["--before", before]);
            }
            let page = self.ctl(&args, None)?;
            for row in page["rows"].as_array().ok_or("activity rows missing")? {
                if let Some(turn) = row.pointer("/event/owner/decoded/turn/decoded") {
                    require(turn["cut"] == false, "whole committed activity turn")?;
                }
            }
            rows.extend(page["rows"].as_array().ok_or("activity rows missing")?.iter().cloned());
            cursor = page["before"]["value"].as_str().map(str::to_owned);
            if cursor.is_none() {
                require(page["cut"] == false, "activity cut without a cursor")?;
                rows.sort_by_key(|row| row["position"].as_u64());
                rows.dedup_by_key(|row| row["position"].as_u64());
                return Ok(rows);
            }
        }
        Err("activity exceeded 256 pages".into())
    }

    fn children(&mut self, parent: &str) -> Result<Vec<Value>, String> {
        let project = self.project.clone();
        let live = self.ctl(&["task", "list", "--project", &project, "--parent", parent], None)?;
        let ended = self.ctl(&["task", "list", "--project", &project, "--parent", parent, "--ended"], None)?;
        child_rows(&live, &ended)
    }

    fn conversation(&mut self, chat: &str) -> Result<String, String> {
        let mut cursor = None;
        let mut words = String::new();
        for _ in 0..256 {
            let mut args = vec!["chat", "show", chat];
            if let Some(before) = cursor.as_deref() {
                args.extend(["--before", before]);
            }
            let page = self.ctl(&args, None)?;
            words.push_str(&page.to_string());
            cursor = page["activity"]["before"]["value"].as_str().map(str::to_owned);
            if cursor.is_none() {
                return Ok(words);
            }
        }
        Err("chat conversation exceeded 256 pages".into())
    }

    fn unchanged(&mut self, chat: &str, baseline: &Value) -> Result<u64, String> {
        let tree = self.ctl(&["task", "show", chat, "--tree"], None)?;
        let tree = &tree["tree"];
        require(tree["more"] == false && tree["folded"] == json!([]), "whole waiting tree")?;
        let rows = tree["rows"].as_array().ok_or("tree rows missing")?;
        require(rows.len() == 1, "only the planning work, no change before the answer")?;
        require(rows[0]["chip"]["executor"].get("agent").is_some(), "planning delegate is an agent")?;
        let goal = rows[0]["chip"]["task"].as_u64().ok_or("planning task number missing")?;
        require(self.children(&goal.to_string())?.is_empty(), "planning work has made no change, ended or live")?;
        require(self.forge_json("branches")? == *baseline, "no pushed branch or changed main before the answer")?;
        require(self.forge_json("pulls?state=all")? == json!([]), "no pull request before the answer")?;
        let activity = json!({"rows": self.activity(goal)?});
        let mut tools = super::successful_tools(&activity);
        let chat_activity = json!({"rows": self.activity(chat.parse().map_err(|_| "chat number")?)?});
        tools.extend(super::successful_tools(&chat_activity));
        require(
            !tools.iter().any(|tool| ["change", "write", "edit"].contains(&tool.as_str())),
            "no edit or change before approval",
        )?;
        Ok(goal)
    }

    fn funds(&mut self) -> Result<(), String> {
        let revision = self.ctl(&["settings", "show"], None)?["newest"].as_u64().ok_or("settings revision")?;
        let rules = self.ctl(&["settings", "show", "rules"], None)?;
        let policy = self.ctl(&["settings", "show", "policy", "1"], None)?;
        require(rules["revision"] == revision && policy["revision"] == revision, "same settings revision")?;
        let file = self.store.join("funds.json");
        write_json(&file, &funds_document(&rules, &policy)?);
        self.ctl(
            &[
                "settings",
                "apply",
                file.to_str().ok_or("funds path")?,
                "--against",
                &revision.to_string(),
                "--key",
                "rung-funds",
            ],
            None,
        )?;
        Ok(())
    }
}

/// Edit only numeric members present in ctl's values, preserving the rest of
/// the revision's settings document (walk 1, step 3).
pub fn funds_document(rules: &Value, policy: &Value) -> Result<Value, String> {
    fn spend(value: &mut Value, pointer: &str, amount: u64) -> Result<(), String> {
        let field = value.pointer_mut(pointer).ok_or_else(|| format!("funds: missing {pointer}"))?;
        require(field.is_u64(), &format!("funds: {pointer} is not a spend"))?;
        *field = amount.into();
        Ok(())
    }
    require(rules["setting"] == json!({"kind":"rules"}), "funds: rules selector")?;
    require(policy["setting"] == json!({"kind":"policy","number":1}), "funds: policy selector")?;
    let mut r = rules["value"].clone();
    let mut p = policy["value"].clone();
    for value in [&mut r, &mut p] {
        spend(value, "/ceiling/spend", 207_552)?;
        spend(value, "/period_spend", 449_696)?;
    }
    let role = p["roles"]
        .as_array()
        .ok_or("funds: policy roles")?
        .iter()
        .position(|role| role["number"] == 0)
        .ok_or("funds: role 0")?;
    spend(&mut p, &format!("/roles/{role}/authority/spend"), 207_552)?;
    spend(&mut p, &format!("/roles/{role}/period_spend"), 415_104)?;
    Ok(json!({"items":[{"setting":rules["setting"],"set":r}, {"setting":policy["setting"],"set":p}]}))
}

fn quote(path: &Path) -> String {
    format!("'{}'", path.to_str().expect("UTF-8 path").replace('\'', "'\"'\"'"))
}

/// Installed before import; used both normally and by the launcher's EXIT trap.
/// An interrupted import is covered even if its reply never reached the driver.
pub fn return_hook(binary: &Path, store: &Path, client: &Path, account: &Path) {
    let hook = store.join("give-back");
    let body = format!(
        r#"#!/bin/sh
set -eu
exec 9>{lock}
flock -x 9
: > {returning}
if [ ! -e {account} ]; then
  rung_account=$({binary} ctl {client} account list --json | jq -er '.accounts.party | map(select(.provider == 1 and .plan == true)) | if length == 1 then .[0].number else error("expected one held ChatGPT plan") end')
  {binary} ctl {client} account export "$rung_account" {account}
fi
[ "$(LC_ALL=C stat -c %F -- {account})" = 'regular file' ]
[ "$(stat -c %a -- {account})" = 600 ]
[ "$(stat -c %s -- {account})" -gt 0 ]
stat -c 'LOGIN BACK: %F mode=%a bytes=%s owner=%u' -- {account}
"#,
        account = quote(account),
        binary = quote(binary),
        client = quote(client),
        lock = quote(&store.join("login.lock")),
        returning = quote(&store.join("returning"))
    );
    super::super::write_private(&hook, body.as_bytes());
    fs::set_permissions(hook, fs::Permissions::from_mode(0o700)).expect("executable return hook");
}

fn stop_box(boxed: &mut Child) -> Result<(), String> {
    let status = Command::new("kill").args(["-TERM", &boxed.id().to_string()]).status().map_err(|e| e.to_string())?;
    require(status.success(), "signal box to stop")?;
    let until = now().saturating_add(Duration::from_secs(15));
    loop {
        if let Some(status) = boxed.try_wait().map_err(|e| e.to_string())? {
            return require(status.success(), "box drained successfully");
        }
        require(now() < until, "box did not drain after login return")?;
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// Run only by proof-live with a held record and a fresh private deployment.
pub fn green_light() {
    let account = super::live_account().expect("rung-b0 requires TEMPER_LIVE_ACCOUNT");
    let (store, kept) = super::live_store(true);
    assert!(!kept, "a rung starts with a fresh box");
    let paths = ["config", "data", "cache"].map(|name| store.join(name));
    deployment(&paths);
    let _ = super::cache_forgejo_binaries(&paths[2]);
    let client = store.join("client");
    fs::create_dir(&client).expect("fresh client");
    fs::set_permissions(&client, fs::Permissions::from_mode(0o700)).expect("private client");
    let mut driver = Driver::new(&store, &client);
    return_hook(&driver.binary, &store, &driver.client, &account);
    let mut boxed = Command::new("setsid")
        .arg(&driver.binary)
        .arg("box")
        .args(super::deployment_flags(&paths))
        .args(["--echo", "steps"])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .current_dir(&store)
        .stdin(Stdio::null())
        .stdout(fs::File::create(store.join("box.out")).expect("box output"))
        .stderr(fs::File::create(store.join("box.err")).expect("box errors"))
        .spawn()
        .expect("live box");
    super::super::live::notice(&format!("TAG rung-b0-{}", super::live_tag()));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| walk(&mut driver, &paths, &account)));
    // No ordinary failure or panic bypasses the return. The launcher's copy of
    // this act covers a signal to the test process itself.
    driver.phase = "return held login";
    driver.until = now().saturating_add(Duration::from_secs(60));
    let returned = driver.execute(Command::new(store.join("give-back")), None, true);
    if let Err(message) = returned {
        super::super::live::notice(&format!("LOGIN NOT BACK: {message}; box retained for recovery"));
        panic!("LOGIN NOT BACK: {message}; box retained for recovery");
    }
    super::super::live::notice("MUST login at its original path; stat regular, nonempty, 0600");
    let stopped = stop_box(&mut boxed);
    let result = result.unwrap_or_else(|panic| std::panic::resume_unwind(panic));
    result.unwrap_or_else(|message| {
        super::super::live::notice(&format!("FAILURE {message}"));
        panic!("{message}")
    });
    stopped.expect("box stopped after login return");
}

fn setup(driver: &mut Driver, paths: &[PathBuf; 3], account: &Path) -> Result<Value, String> {
    let link = loop {
        let output = fs::read_to_string(driver.store.join("box.out")).map_err(|e| e.to_string())?;
        if let Some(link) = output.lines().find(|line| line.starts_with("http://") && line.contains("secret=")) {
            break link.to_owned();
        }
        require(now() < driver.until, "setup: box sign-in link timeout; see box.err")?;
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    driver.ctl(&["sign-in", "-"], Some(&link))?;
    setup_tools(driver)?;
    let output = fs::read_to_string(driver.store.join("box.err")).map_err(|e| e.to_string())?;
    let baseline = setup_observer(driver, paths, &output)?;
    driver.ctl(&["account", "import", account.to_str().ok_or("held path")?, "--key", "connect-plan"], None)?;
    let accounts = driver.wait(&["account", "list"], "usable")?;
    require(
        accounts["accounts"]["party"]
            .as_array()
            .is_some_and(|rows| rows.len() == 1 && rows[0]["provider"] == 1 && rows[0]["plan"] == true),
        "fresh box holds one ChatGPT plan for the return hook",
    )?;
    Ok(baseline)
}

/// All ordinary ctl setup before importing the held account, shared by the
/// live walk and the process proof with the existing fake model.
fn setup_tools(driver: &mut Driver) -> Result<(), String> {
    driver.funds()?;
    driver.ctl(&["forge", "setup", "3", "--playground", "--key", "rung-forge"], None)?;
    driver.wait(&["forge", "list"], "settled")?;
    let forges = driver.ctl(&["forge", "list"], None)?;
    let first =
        forges["forges"].as_array().ok_or("forges missing")?.iter().find(|f| f["number"] == 3).ok_or("owned forge")?;
    require(
        first["first"]["org"] == "playground" && first["first"]["repository"] == "widget",
        "playground/widget adopted",
    )?;
    let me = driver.ctl(&["me"], None)?;
    let made = first["first"]["project"]["new"].as_str().ok_or("playground project name")?;
    let project = me["projects"]
        .as_array()
        .ok_or("projects missing")?
        .iter()
        .find(|p| p["name"]["bytes"] == made)
        .and_then(|p| p["project"].as_u64())
        .ok_or("playground project")?
        .to_string();
    driver.project.clone_from(&project);
    driver.ctl(&["project", "fund", &project, "449696", "--key", "rung-project-funds"], None)?;
    driver.ctl(&["project", "fund", &project, "415104", "--role", "0", "--key", "rung-role-funds"], None)?;
    let charter = driver.ctl(&["settings", "show", "charter", "1"], None)?;
    require(charter["value"]["model"]["name"] == "gpt-6.1-sol", "QA charter uses sol")?;
    Ok(())
}

fn setup_observer(driver: &mut Driver, paths: &[PathBuf; 3], output: &str) -> Result<Value, String> {
    driver.api = output
        .lines()
        .find_map(|line| line.split_once("Forge 3 (Forgejo): api ").map(|(_, api)| api.trim().to_owned()))
        .ok_or("box did not print person's forge API")?;
    let printed_header = output
        .lines()
        .find_map(|line| {
            line.split_once("Forge 3 (Forgejo): person ")?
                .1
                .split_once(", read header ")?
                .1
                .split_once(", write header ")
                .map(|(read, _)| read)
        })
        .ok_or("box did not print person's read header")?;
    require(
        Path::new(printed_header) == paths[1].join("forge-3/person_read.header"),
        "person's read header from the box",
    )?;
    driver.header = printed_header.into();
    super::super::live::notice(
        "SETUP sol high output=32000; engine memory=derived-before-start worker memory=3700000000000; rules/policy ceiling=207552 period=449696 role period=415104",
    );
    let memory = read_json(&paths[0].join("temper.json"))["engine"]["memory_bytes"].clone();
    super::super::live::notice(&format!("SETUP engine memory={memory}"));
    let baseline = driver.forge_json("branches")?;
    require(baseline.as_array().is_some_and(|b| b.len() == 1 && b[0]["name"] == "main"), "fresh main alone")?;
    driver.forge("raw/src/main.rs?ref=main")?;
    Ok(baseline)
}

/// Process-tier proof of the live setup through its last pre-import read.
/// The ordinary harness supervises the existing TLS fake; no account is added
/// and no real provider is contacted. The shared steps still use real Forgejo.
pub fn offline_setup(root: &Path) {
    use super::super::{
        boxed,
        ctl::{Call, Step},
    };
    use skein_io::kernel::Exit;
    use std::{cell::RefCell, rc::Rc};
    let paths = ["config", "data", "cache"].map(|name| root.join(name));
    deployment(&paths);
    let _issuer = super::super::sign_in::patch_configuration(&paths[0]);
    let peer = super::super::sign_in::plan_peer(Box::new([]));
    peer.configure(&paths[0]);
    let _ = super::cache_forgejo_binaries(&paths[2]);
    let driver = Rc::new(RefCell::new(Driver::new(root, &root.join("client"))));
    let tools = driver.clone();
    let prepared = Rc::new(RefCell::new(Ok(())));
    let tool_result = prepared.clone();
    let outcome = boxed::run_ctl_with_llm(
        root,
        super::deployment_flags(&paths),
        root,
        vec![
            Step::Run(Call::sign_in()),
            Step::Then(Box::new(move |_| {
                *tool_result.borrow_mut() = setup_tools(&mut tools.borrow_mut());
                // Return to the harness so it drains setup's printed forge paths
                // before handing the captured stderr to the observer checkpoint.
                Call::new(&["me", "--json"])
            })),
        ],
        boxed::BOX_DEADLINE,
        |stderr| -> Result<(), String> {
            prepared.borrow().clone()?;
            let mut driver = driver.borrow_mut();
            setup_observer(&mut driver, &paths, std::str::from_utf8(stderr).map_err(|e| e.to_string())?)?;
            let accounts = driver.ctl(&["account", "list"], None)?;
            require(accounts["accounts"]["party"] == json!([]), "no account imported by the offline setup")?;
            let project = driver.project.clone();
            let project = driver.ctl(&["project", "show", &project], None)?;
            require(
                project["policy"]["period_spend"] == 449_696
                    && project["current_period"]["numbers"]["budget"] == 449_696,
                "project policy and open period funded",
            )?;
            require(
                project["policy"]["roles"].as_array().is_some_and(|roles| {
                    roles.iter().any(|role| {
                        role["number"] == 0
                            && role["period_spend"] == 415_104
                            && role["authority"]["authority"]["spend"] == 207_552
                            && role["authority"]["cut"] == false
                    })
                }),
                "role 0 authority and pool funded",
            )?;
            require(project["cut"] == false && project["configured_pools_cut"] == false, "whole project funding read")?;
            Ok(())
        },
        boxed::CtlLlm { expected_exits: Vec::new(), peer, on_link: Box::new(|| {}) },
    );
    let stderr = String::from_utf8_lossy(&outcome.stderr);
    assert_eq!(outcome.exit, Some(Exit::Code(0)), "box stop: {stderr}");
    assert_eq!(outcome.left, Some(vec![]), "no surviving children: {stderr}");
    for call in &outcome.calls {
        assert_eq!(call.exit, Some(Exit::Code(0)), "{}", String::from_utf8_lossy(&call.stderr));
    }
    assert_eq!(outcome.calls.len(), 2, "sign-in and setup checkpoint");
    outcome.checked.expect("pre-import checkpoint").expect("all pre-import setup passed");
    assert!(outcome.model.expect("fake model observations").requests.is_empty(), "no model call before import");
    eprintln!("OFFLINE B0 setup passed through account-import boundary, zero model requests; box drained");
}

fn walk(driver: &mut Driver, paths: &[PathBuf; 3], account: &Path) -> Result<(), String> {
    let baseline = setup(driver, paths, account)?;
    let project = driver.project.clone();
    driver.requested = Some(now());
    driver.phase("inbox question", 40_480);
    let created = driver.ctl(
        &[
            "chat",
            "start",
            "--project",
            &project,
            "--model",
            "gpt-6.1-sol",
            "--effort",
            "high",
            "--key",
            "rung-b0",
            REQUEST,
        ],
        None,
    )?;
    let chat = created_chat(&created)?.to_string();
    driver.wait(&["inbox"], "waiting")?;
    driver.met(40_480);
    driver.phase("person's answer sent", 186_660);
    let inbox = driver.ctl(&["inbox", "--kind", "questions"], None)?;
    let rows = inbox["needs"]["rows"].as_array().ok_or("inbox questions missing")?;
    require(rows.len() == 1, "one question in the person's inbox")?;
    require(
        rows[0]["task"]["live"]["task"].as_u64().map(|n| n.to_string()) == Some(chat.clone()),
        "question is on the person's chat",
    )?;
    let question = rows[0]["object"]["question"]["message"].as_u64().ok_or("inbox question number")?.to_string();
    driver.phase("person's answer sent", 186_660);
    let goal = driver.unchanged(&chat, &baseline)?;
    let question_rows = driver.activity(chat.parse().map_err(|_| "chat number")?)?;
    let asked = question_rows
        .iter()
        .find_map(|row| {
            let heard = &row["event"]["owner"]["source"]["line"]["heard"];
            (heard["kind"] == "question" && heard["from"]["task"].as_u64() == Some(goal))
                .then(|| heard["words"].as_str())
                .flatten()
        })
        .ok_or("the inbox question was not asked by the planning work")?;
    require(
        asked.to_lowercase().contains("plan") && asked.contains('?'),
        "the question tells its plan and asks for go-ahead",
    )?;
    super::super::live::notice(&format!("QUESTION {asked}"));
    super::super::live::notice("MUST work's plan question reached the person's inbox; no change, branch or pull");
    std::thread::sleep(std::time::Duration::from_mins(2));
    require(driver.unchanged(&chat, &baseline)? == goal, "same waiting work after two minutes")?;
    super::super::live::notice("MUST after the two-minute hold: no change, no branch, no pull, main unchanged");
    driver.ctl(&["task", "answer", &chat, &question, ANSWER, "--key", "rung-answer"], None)?;
    driver.met(186_660);
    follow(driver, &chat, goal)
}

fn follow(driver: &mut Driver, chat: &str, goal: u64) -> Result<(), String> {
    let goal_text = goal.to_string();
    driver.phase("answer reached goal", 190_910);
    let answer_at = loop {
        let read = driver.ctl(&["task", "show", &goal_text], None)?;
        let rows = driver.activity(goal)?;
        let answered = rows.iter().find_map(|row| {
            let heard = &row["event"]["owner"]["source"]["line"]["heard"];
            (heard["kind"]["answer"]["question"].is_u64()
                && heard["from"]["task"].as_u64().map(|n| n.to_string()) == Some(chat.to_owned())
                && heard["words"] == ANSWER)
                .then(|| heard["at"].as_u64())
                .flatten()
        });
        if let Some(at) = answered {
            break at;
        }
        let digest = read["task"]["digest"].as_str().ok_or("goal digest")?;
        driver.wait(&["task", "show", &goal_text, "--since", digest], "changed")?;
    };
    driver.met(190_910);
    driver.phase("change delegated", 214_930);
    let change = loop {
        let read = driver.ctl(&["task", "show", &goal_text, "--tree"], None)?;
        let children = driver.children(&goal_text)?;
        if let Some(row) = children.iter().find(|r| r["chip"]["executor"].get("procedure").is_some()) {
            break row["chip"]["task"].as_u64().ok_or("change task number")?.to_string();
        }
        let digest = read["task"]["digest"].as_str().ok_or("work digest")?;
        driver.wait(&["task", "show", &goal_text, "--since", digest], "changed")?;
    };
    driver.met(214_930);
    let activity = driver.activity(goal)?;
    let tools = super::successful_tools(&json!({"rows":activity}));
    require(
        !tools.contains("write") && !tools.contains("edit"),
        "the planning work never edited before making its change",
    )?;
    let delegated_at = activity
        .iter()
        .filter(|row| super::successful_tools(&json!({"rows":[row]})).contains("change"))
        .filter_map(|row| row["at"].as_u64())
        .min()
        .ok_or("work's committed change call missing")?;
    require(answer_at <= delegated_at, "work received the answer before calling change")?;
    super::super::live::notice("MUST exact answer reached the planning work before change delegation");
    first_edit(driver, &change, answer_at)?;
    landing(driver, chat, &goal_text, &change)
}

fn first_edit(driver: &mut Driver, change: &str, answer_at: u64) -> Result<(), String> {
    driver.phase("first code write", 257_930);
    loop {
        let tree = driver.ctl(&["task", "show", change, "--tree"], None)?;
        for row in driver.children(change)? {
            let task = row["chip"]["task"].as_u64().ok_or("producer task number")?;
            for event in driver.activity(task)? {
                let tools = super::successful_tools(&json!({"rows":[event]}));
                if tools.contains("write") || tools.contains("edit") {
                    require(
                        event["at"].as_u64().is_some_and(|at| answer_at <= at),
                        "answer before the first code write",
                    )?;
                    driver.met(257_930);
                    return Ok(());
                }
            }
        }
        let digest = tree["tree"]["digest"].as_str().ok_or("change tree digest")?;
        driver.wait(&["task", "tree", change, "--since", digest], "changed")?;
    }
}

fn landing(driver: &mut Driver, chat: &str, goal_text: &str, change: &str) -> Result<(), String> {
    driver.phase("pull request and head CI", 286_410);
    driver.wait(&["change", "show", change], "pull")?;
    let panel = driver.wait(&["change", "show", change], "ci:passed")?;
    let number = panel["pull"]["number"].as_u64().ok_or("pull number")?;
    let pull = driver.forge_json(&format!("pulls/{number}"))?;
    require(panel["head"] == pull["head"]["sha"], "ctl and forge agree on PR head")?;
    let head = pull["head"]["sha"].as_str().ok_or("forge pull head")?;
    let status = driver.forge_json(&format!("commits/{head}/status"))?;
    require(status["state"] == "success", "forge CI green at the exact PR head")?;
    let runs = driver.forge_json(&format!("actions/runs?head_sha={head}"))?;
    let run = runs["workflow_runs"]
        .as_array()
        .ok_or("Forgejo workflow runs missing")?
        .iter()
        .find(|r| r["commit_sha"] == head && r["status"] == "success")
        .and_then(|r| r["id"].as_u64())
        .ok_or("box runner has not completed CI at the pull head")?;
    let jobs = driver.forge_json(&format!("actions/runs/{run}/jobs"))?;
    let job = jobs["jobs"]
        .as_array()
        .ok_or("Forgejo jobs missing")?
        .iter()
        .find(|job| job["status"] == "success")
        .and_then(|job| job["id"].as_u64())
        .ok_or("no successful runner job at pull head")?;
    let log = driver.forge(&format!("actions/jobs/{job}/logs"))?;
    require(String::from_utf8_lossy(&log).contains("Job succeeded"), "the box runner's job log agrees with green CI")?;
    driver.met(286_410);
    driver.phase("agent review approved", 363_470);
    let mut panel = panel;
    while !panel["gates"]
        .as_array()
        .is_some_and(|gates| gates.iter().any(|g| g["kind"] == "review" && g["verdict"] == "approved"))
    {
        let digest = panel["digest"].as_str().ok_or("change digest")?;
        panel = driver.wait(&["change", "show", change, "--since", digest], "changed")?;
    }
    driver.met(363_470);
    driver.phase("landed", 364_410);
    let panel = driver.wait(&["change", "show", change], "landed")?;
    let pull = driver.forge_json(&format!("pulls/{number}"))?;
    let main = driver.forge_json("branches/main")?;
    require(
        pull["merged"] == true
            && panel["merge"] == pull["merge_commit_sha"]
            && panel["merge"] == main["commit"]["id"]
            && pull["head"]["sha"] == head
            && panel["head"] == head,
        "ctl landing, forge merge and main agree",
    )?;
    driver.forge_json(&format!("pulls/{number}/reviews"))?;
    driver.met(364_410);
    driver.phase("main CI passed", 367_410);
    let merge = pull["merge_commit_sha"].as_str().ok_or("merge hash")?;
    loop {
        if driver.forge_json(&format!("commits/{merge}/status"))?["state"] == "success" {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    driver.met(367_410);
    driver.phase("chat's last word and whole", WHOLE);
    let source = driver.forge(&format!("raw/src/main.rs?ref={merge}"))?;
    require(
        source.windows(b"fruit,count".len()).any(|s| s == b"fruit,count"),
        "src/main.rs at landed main contains fruit,count",
    )?;
    super::super::live::notice(
        "MUST landed at forge main; CI green at PR head and main; agent review approved; src/main.rs holds fruit,count",
    );
    let work = driver.wait(&["task", "show", goal_text], "ended")?;
    let ended_at =
        work["task"]["chip"]["details"]["owner"]["details"]["ended_at"].as_u64().ok_or("work's report timestamp")?;
    loop {
        let snapshot = driver.ctl(&["task", "show", chat], None)?;
        let rows = driver.activity(chat.parse().map_err(|_| "chat number")?)?;
        let replies = super::replies_after(&json!({"rows": rows}), ended_at)?;
        if !replies.is_empty() {
            let words = driver.conversation(chat)?.to_lowercase();
            let own_words = json!(replies).to_string().to_lowercase();
            require(
                own_words.contains("landed") || own_words.contains(&format!("#{number}")),
                "chat's own final reply names the landing",
            )?;
            require(words.contains("landed"), "chat conversation carries the landed report")?;
            break;
        }
        let digest = snapshot["task"]["digest"].as_str().ok_or("chat digest")?;
        driver.wait(&["task", "show", chat, "--since", digest], "changed")?;
    }
    driver.met(WHOLE);
    super::super::live::notice("MUST chat says the change landed");
    driver.phase = "close chat";
    driver.until = now().saturating_add(Duration::from_secs(30));
    driver.ctl(&["chat", "close", chat, "--key", "rung-close"], None)?;
    Ok(())
}
