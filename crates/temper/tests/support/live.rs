//! A fresh `ChatGPT` sign-in and the native chat/change stories on real Responses.
use super::{Process, Seen, forgejo::Throwaway, person, write_private};
use jig_protocol_store::{codec, transcripts};
use serde_json::Value;
use skein_io::kernel::Signal;
use skein_lib::{Duration, Time};
use skein_shell::{Clock, Host};
use skein_world::{Referee, real};
use std::{
    cell::RefCell,
    fs,
    path::{Path, PathBuf},
    rc::Rc,
};

#[must_use]
pub fn configure(directory: &Path, forge: &Throwaway) -> String {
    let path = directory.join("temper.json");
    let mut value: Value = serde_json::from_slice(&fs::read(&path).expect("configuration")).expect("JSON");
    let owned: Value = serde_json::from_slice(super::owned::EXAMPLE).expect("owned example");
    value["engine"]["llm"] = owned["engine"]["llm"].clone();
    value["engine"]["limits"]["overrides"]["documents.rows"] = 32.into();
    let agent_path = directory.join("agent.json");
    let mut agent: Value = serde_json::from_slice(super::owned::SHIPPED[0].1).expect("owned agent JSON");
    if let Some(program) = agent["programs"].get_mut("claude_code") {
        let claude = std::env::var_os("PATH").and_then(|path| {
            std::env::split_paths(&path).map(|directory| directory.join("claude")).find(|program| program.is_file())
        });
        if let Some(claude) = claude {
            program["path"] = serde_json::json!(claude);
            program["directory"] = serde_json::json!(directory.join("programs"));
        } else {
            agent["programs"].as_object_mut().expect("programs").remove("claude_code");
        }
    }
    agent["profile"]["policy"]["max_turns"] = 8.into();
    value["workers"][0]["limits"]["overrides"]["hub.event_bytes"] = 4096.into();
    value["application"]["limits"]["overrides"]["checkout.message_bytes"] = 4098.into();
    let scripts = super::landing_scripts(super::ANSWER, forge);
    let skein_fake_llm_domain::api::Line::Call { arguments, .. } = &scripts[0].turns[0].lines[0] else {
        panic!("the process fixture delegates the coding procedure");
    };
    let instructions = format!(
        "A task whose contract is a change reads, edits and finishes with a change, and delegates nothing. Follow the task's role exactly. For the initial greeting conversation, call finish with {{\"form\":\"report\",\"text\":\"Process answer\",\"fields\":{{}}}}. \
        For the conversation requesting a small change, call delegate exactly with {}, then call wait with {{}}. \
        For the delegated coding task, read README.md, append exactly Changed by temper. followed by a newline, use edit, \
        then finish with {{\"form\":\"change\",\"fields\":{{\"title\":\"Update README\",\"body\":\"Changed by temper.\"}}}}. \
        For a change review task, finish with {{\"form\":\"verdict\",\"label\":\"1\",\"text\":\"Checked\",\"fields\":{{}},\"items\":[]}}.",
        std::str::from_utf8(arguments).expect("delegate JSON").replace("@coding", "\\u0040coding")
    );
    super::configuration::turn_bounds::apply(&mut value, super::configuration::turn_bounds::turn_body(&agent));
    write_private(&path, &serde_json::to_vec(&value).expect("live configuration JSON"));
    write_private(&agent_path, &serde_json::to_vec(&agent).expect("live agent JSON"));
    instructions
}

pub struct Pushed {
    pub seen: Seen,
    pub branch: Box<[u8]>,
    pub pushed: Box<[u8]>,
    pub wall_seconds: u64,
}
type Push = super::forgejo::BranchHead;
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Starting,
    Signing,
    Running,
    Stopping,
    Settled,
}
struct Judge<'a> {
    directory: &'a Path,
    forge: &'a Throwaway,
    baseline: Box<[u8]>,
    pushed: Rc<RefCell<Option<Push>>>,
    poll: Time,
    deadline: Time,
    phase: Phase,
    signing_until: Time,
    checked: usize,
}
fn files(directory: &Path, output: &mut Vec<PathBuf>) {
    if !directory.exists() {
        return;
    }
    for entry in fs::read_dir(directory).expect("proof directory").map(|entry| entry.expect("proof entry")) {
        let path = entry.path();
        if entry.file_type().expect("proof file type").is_dir() {
            files(&path, output);
        } else {
            output.push(path);
        }
    }
}
fn pushed(directory: &Path, forge: &Throwaway, baseline: &[u8]) -> Option<Push> {
    let heads: Vec<_> = forge
        .branch_heads()
        .into_iter()
        .filter(|(branch, commit)| branch.starts_with(b"temper/") && commit.as_ref() != baseline)
        .collect();
    if heads.is_empty() {
        return None;
    }
    let mut paths = Vec::new();
    files(&directory.join("workspaces"), &mut paths);
    for path in paths {
        if path.file_name().is_none_or(|name| name != "HEAD") {
            continue;
        }
        let git = path.parent().expect("HEAD parent");
        if git.file_name().is_none_or(|name| name != ".git") {
            continue;
        }
        let output = std::process::Command::new("/usr/bin/git")
            .env_clear()
            .arg("--git-dir")
            .arg(git)
            .args(["cat-file", "--batch-all-objects", "--batch-check=%(objectname) %(objecttype)"])
            .output()
            .expect("git's object database");
        assert!(output.status.success(), "the run's git objects can be read");
        for line in output.stdout.split(|byte| *byte == b'\n') {
            let Some(commit) = line.strip_suffix(b" commit") else { continue };
            if let Some((branch, _)) = heads.iter().find(|(_, head)| head.as_ref() == commit) {
                return Some((branch.clone(), commit.into()));
            }
        }
    }
    None
}
pub fn account_ready(value: &Value) {
    ready_account(value).unwrap_or_else(|message| panic!("{message}"));
}
pub fn ready_account(value: &Value) -> Result<(), String> {
    let account = value["accounts"]["party"]
        .as_array()
        .ok_or("person's accounts missing")?
        .iter()
        .find(|account| account["provider"] == 1)
        .ok_or("ChatGPT account missing")?;
    if account["plan"] != true {
        return Err("ChatGPT is not a plan".into());
    }
    let sources = account["sources"].as_array().ok_or("account sources missing")?;
    if !sources
        .iter()
        .any(|source| source["kind"] == "sign_in" && source["usable"] == true && source["state"]["kind"] == "granted")
    {
        return Err(format!("live account {} has no usable granted sign-in", account["number"]));
    }
    Ok(())
}
fn tasks_ready(value: &Value) {
    match value {
        Value::Object(fields) => {
            assert!(
                !fields
                    .get("phase")
                    .and_then(Value::as_str)
                    .is_some_and(|phase| matches!(phase, "failed" | "cancelled")),
                "live task cannot answer: {value}"
            );
            for child in fields.values() {
                tasks_ready(child);
            }
        }
        Value::Array(items) => {
            for child in items {
                tasks_ready(child);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}
impl Judge<'_> {
    fn running(&self) -> bool {
        matches!(self.phase, Phase::Running | Phase::Stopping | Phase::Settled)
    }
    fn stopping(&self) -> bool {
        matches!(self.phase, Phase::Stopping | Phase::Settled)
    }
    fn account(&mut self, now: Time, procs: &mut [Process]) {
        use super::ctl::{Call, Step};
        let Process::Calls(calls) = &mut procs[2] else { panic!("live ctl") };
        if self.phase == Phase::Starting
            && let [_, connected, ..] = calls.ends()
        {
            assert_eq!(
                connected.exit,
                Some(skein_io::kernel::Exit::Code(0)),
                "connect: {}",
                String::from_utf8_lossy(&connected.stderr)
            );
            // This is the authorization address, never the browser's final callback or a token.
            notice(&String::from_utf8_lossy(&connected.stdout));
            client_notice(&String::from_utf8_lossy(calls.link()));
            self.phase = Phase::Signing;
        }
        if !self.running() && calls.ends().len() >= SETUP_CALLS {
            for ended in calls.ends() {
                assert_eq!(
                    ended.exit,
                    Some(skein_io::kernel::Exit::Code(0)),
                    "live sign-in: {}",
                    String::from_utf8_lossy(&ended.stderr)
                );
            }
            let [_, _, _, account, ..] = calls.ends() else { panic!("live setup calls") };
            account_ready(&super::ctl::json(account));
            let link = calls.link().to_vec();
            self.checked = SETUP_CALLS;
            self.phase = Phase::Running;
            let Process::Person(person) = &mut procs[1] else { panic!("native person") };
            person.ctl_ready(&link);
        }
        if !self.running() {
            return;
        }
        let Process::Calls(calls) = &mut procs[2] else { panic!("live ctl") };
        for ended in &calls.ends()[self.checked..] {
            assert_eq!(
                ended.exit,
                Some(skein_io::kernel::Exit::Code(0)),
                "live health read: {}",
                String::from_utf8_lossy(&ended.stderr)
            );
            let value = super::ctl::json(ended);
            if value.get("accounts").is_some() {
                account_ready(&value);
            } else {
                tasks_ready(&value);
            }
        }
        self.checked = calls.ends().len();
        if self.phase == Phase::Running && now >= self.poll && calls.idle() {
            for arguments in [
                &["accounts", "--json"][..],
                &["tasks", "--project", "1", "--json"],
                &["tasks", "--project", "1", "--ended", "--json"],
            ] {
                calls.push(Step::Run(Call::new(arguments)));
            }
        }
    }
}
// Sign-in/account setup, then the stock inventory, charter read and one settings change.
const SETUP_CALLS: usize = 7;
fn sign_in_within() -> Duration {
    jig_protocol_config::profile::profile(jig_protocol_config::profile::Profile::Small).driver.client.sign_in_time
}
impl Referee<Process> for Judge<'_> {
    fn act(&mut self, now: Time, procs: &mut [Process]) {
        self.account(now, procs);
        if self.phase == Phase::Running && now >= self.poll {
            assert!(completions(self.directory) <= 25, "live completion budget exceeded");
            if self.pushed.borrow().is_none() {
                *self.pushed.borrow_mut() = pushed(self.directory, self.forge, &self.baseline);
            }
            self.poll = now.saturating_add(Duration::from_millis(500));
        }
        let Process::Person(person) = &procs[1] else { panic!("native person") };
        if !self.stopping()
            && person.finished
            && person.client.as_ref().is_some_and(Host::is_empty)
            && self.pushed.borrow().is_some()
        {
            let Process::Calls(calls) = &mut procs[2] else { panic!("live ctl") };
            if !calls.idle() {
                return;
            }
            calls.release();
            let Process::Binary(binary) = &mut procs[0] else { panic!("binary") };
            binary.signal(Signal::Terminate);
            self.phase = Phase::Stopping;
        }
    }
    fn observe(&mut self, now: Time, procs: &[Process]) {
        if self.stopping() && procs.iter().all(Host::is_empty) {
            self.phase = Phase::Settled;
        }
        let Process::Binary(binary) = &procs[0] else { panic!("binary") };
        let Process::Person(person) = &procs[1] else { panic!("native person") };
        assert!(person.failed.is_none(), "{}", person.failed.as_deref().unwrap_or("person"));
        assert!(
            self.stopping() || binary.exit_status().is_none(),
            "serve failed: {}",
            String::from_utf8_lossy(binary.stderr())
        );
        assert!(self.running() || now < self.signing_until, "ChatGPT sign-in deadline");
        assert!(
            now < self.deadline || self.phase == Phase::Settled,
            "live story deadline; pushed={}",
            self.pushed.borrow().is_some()
        );
    }
    fn next_deadline(&self) -> Option<Time> {
        (self.phase != Phase::Settled).then_some(if !self.running() {
            self.signing_until.min(self.deadline)
        } else if self.stopping() {
            self.deadline
        } else {
            self.poll.min(self.deadline)
        })
    }
    fn overdue(&self, now: Time) -> Option<String> {
        (self.phase != Phase::Settled && now >= self.deadline).then(|| "live native stories did not settle".into())
    }
    fn passed(&self) -> bool {
        self.phase == Phase::Settled
    }
}

#[must_use]
pub fn run(directory: &Path, forge: &Throwaway, instructions: &str, deadline: Duration) -> Pushed {
    use super::ctl::{Call, Calls, Step};
    let instructions = instructions.to_owned();
    let file = directory.join("settings.json");
    let clock = Clock::new();
    let started = clock.now().now;
    let mut binary = super::serve(directory);
    let streams = binary.take_streams().expect("binary pipes");
    let mut fds = streams.descriptors();
    let ctl_root = skein_shell::open_root(directory).expect("ctl parent root");
    fds.push(ctl_root);
    let root = skein_shell::open_root(directory).expect("native parent root");
    let pushed = Rc::new(RefCell::new(None));
    let mut world = real::World::new(Judge {
        directory,
        forge,
        baseline: forge.branch_head(b"main").expect("initial main"),
        pushed: pushed.clone(),
        poll: started,
        deadline: started.saturating_add(deadline),
        phase: Phase::Starting,
        signing_until: started.saturating_add(sign_in_within()),
        checked: 0,
    });
    world.spawn_with_fds(binary.descriptors(), || Process::Binary(Box::new(binary)));
    world.spawn_with_fds(vec![root], || {
        Process::Person(Box::new(person::Person::waiting_for_ctl(root, person::live_adopt_and_change(forge))))
    });
    world.spawn_with_fds(fds, || {
        let mut calls = Calls::new(
            ctl_root,
            vec![
                Step::Run(Call::sign_in()),
                Step::Run(Call::new(&["connect", "1", "--key", "live-sign-in"])),
                Step::Run(Call::new(&["wait", "accounts", "--until", "settled", "--json"])),
                Step::Run(Call::new(&["accounts", "--json"])),
                Step::Run(Call::new(&["settings", "--json"])),
                Step::Run(Call::new(&["setting", "charter", "1", "--json"])),
                Step::Then(Box::new(move |calls| {
                    super::ctl::setting_change_file(&file, &calls[5], |value| {
                        value["turns"] = 5.into();
                        value["time"] = 240_000_000_000_u64.into();
                        value["instructions"] = instructions.into();
                    });
                    super::ctl::settings_change(&calls[4], &file, "live-settings")
                })),
            ],
            Some(&streams),
            None,
        );
        calls.hold();
        Process::Calls(Box::new(calls))
    });
    let outcome = world.run(&clock, deadline);
    assert!(outcome.procs.iter().all(Host::is_empty), "all real io settles");
    let Process::Binary(binary) = &outcome.procs[0] else { panic!("binary") };
    assert_eq!(binary.exit_status(), Some(skein_io::kernel::Exit::Code(0)));
    let Process::Person(person) = &outcome.procs[1] else { panic!("native person") };
    let (branch, pushed) = pushed.borrow_mut().take().expect("a new run commit was pushed");
    Pushed {
        seen: Seen { signed_in: 1, finished: person.finished, stderr: binary.stderr().to_vec() },
        branch,
        pushed,
        wall_seconds: clock.now().now.saturating_since(started).as_nanos() / 1_000_000_000,
    }
}

#[must_use]
pub fn completions(directory: &Path) -> u32 {
    count_completions(directory, &super::configuration::directories(directory))
}

/// Count this story's committed turns through ctl, including restarted
/// attempts. Each source turn names one transcript frame, as the fresh
/// story's file counter does, without opening any kept deployment file.
pub fn ctl_completions(client: &Path, tasks: &[u64]) -> Result<u32, String> {
    let mut turns = std::collections::BTreeSet::new();
    for task in tasks {
        let task = task.to_string();
        let mut before = None::<String>;
        for page in 0..32 {
            let mut args = vec!["activity", &task, "--limit", "32", "--json"];
            if let Some(before) = &before {
                args.extend(["--before", before]);
            }
            let read = super::ctl::alone(client, super::ctl::Call::new(&args));
            if read.exit != Some(skein_io::kernel::Exit::Code(0)) {
                return Err("completion read failed".into());
            }
            let activity: Value = serde_json::from_slice(&read.stdout).map_err(|error| error.to_string())?;
            for row in activity["rows"].as_array().ok_or("completion rows missing")? {
                let Some(turn) = row.pointer("/event/owner/source/line/turn") else { continue };
                turns.insert((
                    task.clone(),
                    turn["attempt"].as_u64().ok_or("completion attempt missing")?,
                    turn["turn"].as_u64().ok_or("completion turn missing")?,
                ));
            }
            before = activity["before"]["value"].as_str().map(str::to_owned);
            if before.is_none() {
                break;
            }
            if page == 31 {
                return Err("completion pagination exceeded 32 pages".into());
            }
        }
    }
    u32::try_from(turns.len()).map_err(|_| "completion count overflow".into())
}

fn count_completions(config: &Path, directories: &temper::directories::Directories) -> u32 {
    use std::os::unix::ffi::OsStrExt;
    let value = fs::read(config.join("temper.json")).expect("configuration");
    let document = jig_protocol_config::document(
        &value,
        &jig_protocol_config::DocumentLimits { bytes: 1 << 20, tokens: 1 << 20, depth: 32 },
    )
    .expect("document");
    let at = jig_protocol_config::read::Object::new(&document, 0)
        .expect("object")
        .member(&document, b"engine")
        .expect("engine member");
    let engine = temper_service::startup::engine(&document, at, &directories.jig()).expect("engine configuration");
    let limits = codec::limits(&engine.limits.core).expect("store codec limits");
    let mut paths = Vec::new();
    let deployment = Path::new(std::ffi::OsStr::from_bytes(&engine.deployment.directory));
    files(&deployment.join("transcripts"), &mut paths);
    let mut count = 0;
    for path in paths {
        let bytes = fs::read(path).expect("transcript");
        let mut at = 0;
        while at + 4 <= bytes.len() {
            let length = usize::try_from(u32::from_be_bytes(bytes[at..at + 4].try_into().expect("frame length")))
                .expect("u32 fits");
            if length < 12 || at + length > bytes.len() {
                break;
            }
            transcripts::frame::decode(&bytes[at..at + length], &limits).expect("durable turn");
            count += 1;
            at += length;
        }
    }
    count
}

/// Redact link secrets and structured credential values before diagnostic output.
#[must_use]
pub fn redact(text: &str) -> String {
    let mut output = text.to_owned();
    for key in ["secret=", "access_token=", "refresh_token=", "id_token=", "token=", "Bearer "] {
        let mut at = 0;
        while let Some(found) = output[at..].find(key) {
            let start = at + found + key.len();
            let end = output[start..]
                .find(|c: char| c.is_whitespace() || matches!(c, '&' | '"' | '\'' | '<' | '>'))
                .map_or(output.len(), |n| start + n);
            output.replace_range(start..end, "…");
            at = start + "…".len();
        }
    }
    for key in [
        "secret",
        "token",
        "access_token",
        "refresh_token",
        "id_token",
        "credential",
        "authorization",
        "api_key",
        "password",
    ] {
        let marker = format!("\"{key}\"");
        let mut at = 0;
        while let Some(found) = output[at..].find(&marker) {
            let start = at + found + marker.len();
            at = start;
            let rest = output[start..].trim_start();
            let Some(rest) = rest.strip_prefix(':') else { continue };
            let value = output.len() - rest.trim_start().len();
            let mut stream = serde_json::Deserializer::from_str(&output[value..]).into_iter::<Value>();
            if stream.next().is_some_and(|value| value.is_ok()) {
                let end = value + stream.byte_offset();
                output.replace_range(value..end, "\"…\"");
                at = value + "\"…\"".len();
            }
        }
    }
    output
}

fn append_notice(text: &str) {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let Some(path) = std::env::var_os("TEMPER_LIVE_NOTICE") else { return };
    // The launcher creates this private file before starting heavy.
    match fs::OpenOptions::new().append(true).mode(0o600).open(path) {
        Ok(mut file) => {
            if let Err(error) = writeln!(file, "{text}") {
                eprintln!("live notice write: {error}");
            }
        }
        Err(error) => eprintln!("live notice open: {error}"),
    }
}

/// The same safe diagnostics reach stderr and, optionally, the person's terminal.
pub fn notice(text: &str) {
    let safe = redact(text);
    eprintln!("{safe}");
    append_notice(&safe);
}

/// Only the transient private notice contains the full sign-in link.
pub fn client_notice(link: &str) {
    eprintln!("CLIENT {}", link.split('?').next().unwrap_or(link));
    append_notice(&format!("CLIENT {link}"));
}
