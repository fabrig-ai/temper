//! A real owned-Forgejo failure and repair, on the first-repository fixtures.
use super::{
    ctl::{Call, Step},
    first_repository::{self, call},
};
use serde_json::{Value, json};
use skein_fake_llm_domain::api::{Line, Piece, Script};
use skein_lib::{Duration, Time};
use std::{
    fs,
    path::{Path, PathBuf},
};

const REPAIR: &str = "Repair the failed CI on the change branch";
const REVIEW: &str = "Review this change at the named head";
const REPOSITORY: &str = "/api/v1/repos/playground/widget";
fn now() -> Time {
    skein_shell::Clock::new().now().now
}

const NOTE: &str = "Widget prints its table with --csv.\n";
// Source: ladder-b01/g1/report.md, the proved gate also printed byte for byte
// in qa-focused/rung-b1-brief.md's appendix. No action or external service.
const WORKFLOW: &str = r#"on:
  push:
    branches: [main]
  pull_request:
jobs:
  check:
    runs-on: temper-host
    steps:
      - run: echo "temper's starter check passed; replace this step with your own checks"
  changes:
    if: ${{ github.event_name == 'pull_request' }}
    runs-on: temper-host
    steps:
      - name: Each pull request adds changes/<its number>.md
        run: |
          echo 'pull_request.number=${{ github.event.pull_request.number }}'
          auth=$(printf 'x-access-token:%s' "${{ github.token }}" | base64 -w0)
          git -c "http.extraHeader=Authorization: basic $auth" clone --quiet --no-checkout "${{ github.server_url }}/${{ github.repository }}.git" repo
          git -C repo checkout --quiet '${{ github.event.pull_request.head.sha }}'
          test -f 'repo/changes/${{ github.event.pull_request.number }}.md' || { echo 'Missing changes/${{ github.event.pull_request.number }}.md'; exit 1; }
"#;
const CONTRIBUTING: &str = "# Contributing\n\nEach pull request adds `changes/<its number>.md`: one line saying what it\nchanges, for the next release's notes. CI checks it.\n";

#[must_use]
pub fn llm(config: &Path) -> super::boxed::CtlLlm {
    let mut scripts = first_repository::csv_scripts().into_vec();
    let Line::Call { arguments, .. } = &mut scripts[0].turns[0].lines[0] else { panic!("delegate") };
    let mut delegate: Value = serde_json::from_slice(arguments).expect("delegate JSON");
    delegate["batch"][0]["contract"]["words"] = 512.into();
    delegate["batch"][0]["spec"]["words"] = "@hello-readme Add CSV export to playground/widget".into();
    *arguments = serde_json::to_vec(&delegate).expect("delegate JSON").into_boxed_slice();
    // Use the shared CSV implementation, writing only src/main.rs at H1.
    let mut source = scripts[1]
        .turns
        .iter()
        .find(|turn| {
            turn.lines.iter().any(|line| {
                let Line::Call { name, arguments } = line else { return false };
                name.as_ref() == b"write"
                    && serde_json::from_slice::<Value>(arguments).expect("write JSON")["path"] == "src/main.rs"
            })
        })
        .expect("shared CSV source")
        .clone();
    let Line::Call { arguments, .. } = &mut source.lines[0] else { panic!("CSV write") };
    let mut write: Value = serde_json::from_slice(arguments).expect("CSV write JSON");
    write["content"] = write["content"].as_str().expect("CSV source").replace("name,count", "fruit,count").into();
    *arguments = serde_json::to_vec(&write).expect("CSV write JSON").into_boxed_slice();
    let finish = scripts[1].turns.last().expect("coder finish").clone();
    // smith's write checks the version the agent read; playground/widget
    // already has src/main.rs, unlike the CSV fixture's empty work/hello.
    scripts[1].turns = Box::new([call(b"read", &json!({"path":"src/main.rs"})), source, finish]);
    scripts[0].turns[2] = call(b"finish", &json!({"form":"report", "text":"playground/widget landed.", "fields":{}}));
    let mut write = call(b"write", &json!({"path":"changes/1.md", "content":NOTE}));
    let mut lines = write.lines.into_vec();
    lines.insert(
        0,
        Line::Pieces {
            pieces: Box::new([Piece {
                text: b"Repairing the missing release note.".as_slice().into(),
                hold: Some(b"csv-repair".as_slice().into()),
            }]),
        },
    );
    write.lines = lines.into_boxed_slice();
    scripts.push(Script {
        cue: REPAIR.as_bytes().into(),
        turns: Box::new([write, call(b"finish", &json!({"form":"change", "fields":{
            "title":"Add CSV export to widget", "body":"Widget prints its table with --csv and has its release note."
        }}))]),
    });
    first_repository::model(config, scripts.into_boxed_slice())
}

struct Forge {
    address: String,
    header: PathBuf,
}
impl Forge {
    fn at(paths: &[PathBuf; 3]) -> Self {
        let document: Value =
            serde_json::from_slice(&fs::read(paths[0].join("temper.json")).expect("config")).expect("JSON");
        let forge = document["application"]["forges"]
            .as_array()
            .expect("forges")
            .iter()
            .find(|forge| forge["number"] == 3)
            .expect("forge 3");
        Self {
            address: forge["api"]["socket"].as_str().expect("address").into(),
            header: paths[0].join("person.header"),
        }
    }
    fn person(paths: &[PathBuf; 3]) -> Self {
        let forge = Self::at(paths);
        let document: Value =
            serde_json::from_slice(&fs::read(paths[0].join("temper.json")).expect("config")).expect("JSON");
        let setup = document["application"]["setup"]
            .as_array()
            .expect("setup")
            .iter()
            .find(|entry| entry["forge"] == 3)
            .expect("owned setup");
        let login = setup["mode"]["owned"]["person"]["login"].as_str().expect("person login");
        let work = paths[1].join("forge-3/forgejo");
        let output = std::process::Command::new(super::owned::binaries().0)
            .arg("--work-path")
            .arg(&work)
            .arg("--config")
            .arg(work.join("app.ini"))
            .args(["admin", "user", "must-change-password", "--unset", login])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", paths[1].join("forge-3/home"))
            .stdin(std::process::Stdio::null())
            .output()
            .expect("person password flag");
        assert!(output.status.success(), "clear person's first-password flag (output redacted)");
        let token = super::forge_land::scoped_token(paths, true, login, "write:repository,read:issue");
        super::write_private(&forge.header, format!("Authorization: token {token}\n").as_bytes());
        forge
    }
    fn request(&self, method: &str, suffix: &str, body: Option<&Value>) -> Value {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut command = Command::new("/usr/bin/curl");
        command
            .args(["--silent", "--show-error", "--fail", "--max-time", "10", "--request", method, "--header"])
            .arg(format!("@{}", self.header.display()))
            .args(["--header", "Content-Type: application/json"])
            .arg(format!("http://{}{REPOSITORY}/{suffix}", self.address))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if body.is_some() {
            command.args(["--data-binary", "@-"]);
        }
        let mut child = command.spawn().expect("owned API request");
        if let Some(body) = body {
            child
                .stdin
                .take()
                .expect("request stdin")
                .write_all(&serde_json::to_vec(body).expect("JSON"))
                .expect("request body");
        } else {
            drop(child.stdin.take());
        }
        let output = child.wait_with_output().expect("owned API response");
        assert!(output.status.success(), "{method} {suffix}: {}", String::from_utf8_lossy(&output.stderr));
        serde_json::from_slice(&output.stdout).expect("API JSON")
    }
    fn get(&self, suffix: &str) -> Value {
        self.request("GET", suffix, None)
    }
    fn statuses(&self, head: &str) -> Value {
        self.get(&format!("commits/{head}/status"))
    }
    fn install(&self) {
        let workflow = self.get("contents/.forgejo/workflows/ci.yml?ref=main");
        let previous = self.get("branches/main")["commit"]["id"].clone();
        let committed = self.request("POST", "contents", Some(&json!({
            "branch":"main", "message":"CI: each pull request adds changes/<its number>.md",
            "files":[
                {"operation":"update", "path":".forgejo/workflows/ci.yml", "sha":workflow["sha"], "content":encode(WORKFLOW)},
                {"operation":"create", "path":"CONTRIBUTING.md", "content":encode(CONTRIBUTING)}
            ]
        })));
        let head = committed["commit"]["sha"].as_str().expect("gate commit");
        let commit = self.get(&format!("git/commits/{head}"));
        assert_eq!(commit["parents"].as_array().expect("parents").len(), 1);
        assert_eq!(commit["parents"][0]["sha"], previous, "one person commit");
        assert_eq!(self.contents(".forgejo/workflows/ci.yml", head), WORKFLOW);
        assert_eq!(self.contents("CONTRIBUTING.md", head), CONTRIBUTING);
        let until = now().saturating_add(Duration::from_secs(60));
        loop {
            let status = self.statuses(head);
            if status["state"] == "success" {
                assert_contexts(&status, "push", "skipped");
                break;
            }
            assert!(now() < until, "main gate CI phase: {status}");
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let protection = self.get("branch_protections");
        for rule in protection.as_array().expect("protection rules") {
            assert!(
                rule["status_check_contexts"].as_array().is_none_or(Vec::is_empty),
                "the combined CI verdict must include both contexts: {rule}"
            );
        }
    }
    fn contents(&self, path: &str, head: &str) -> String {
        let value = self.get(&format!("contents/{path}?ref={head}"));
        let content = value["content"].as_str().expect("base64 content");
        let bytes = skein_lib::base64::decode(skein_lib::base64::Alphabet::Standard, content.as_bytes(), 1 << 20)
            .expect("file bytes");
        String::from_utf8(bytes.into_vec()).expect("UTF-8 file")
    }
    fn head(&self, panel: &Value, changes: &str) {
        let pull = self.get("pulls/1");
        assert_eq!(pull["number"], 1);
        let head = pull["head"]["sha"].as_str().expect("pull head");
        assert_eq!(panel["head"], head, "ctl and Forgejo agree at {changes}");
        assert_eq!(panel["pull"]["address"], format!("http://{}/playground/widget/pulls/1", self.address));
        // ctl and the API are separate samples. Forgejo can publish another
        // pending context after a brief success containing only the first job.
        // Keep the reviewer held until both of this head's jobs have finished.
        let until = now().saturating_add(Duration::from_secs(60));
        let statuses = loop {
            let statuses = self.statuses(head);
            if changes == "failure"
                || (statuses["state"] == "success"
                    && statuses["statuses"]
                        .as_array()
                        .is_some_and(|rows| rows.len() == 2 && rows.iter().all(|row| row["status"] == "success")))
            {
                break statuses;
            }
            assert!(now() < until, "H2's two CI contexts phase: {statuses}");
            std::thread::sleep(std::time::Duration::from_millis(100));
        };
        assert_eq!(statuses["state"], if changes == "failure" { "failure" } else { "success" });
        assert_contexts(&statuses, "pull_request", changes);
        let files = self.get("pulls/1/files");
        let names: Vec<_> = files
            .as_array()
            .expect("PR files")
            .iter()
            .map(|file| file["filename"].as_str().expect("filename"))
            .collect();
        if changes == "failure" {
            assert_eq!(names, ["src/main.rs"], "H1 has no release note");
        } else {
            assert!(names.contains(&"src/main.rs") && names.contains(&"changes/1.md"));
            assert_eq!(self.contents("changes/1.md", head), NOTE);
        }
        eprintln!("owned CSV {changes}: head {head}; statuses {}", statuses["statuses"]);
    }
}
fn encode(text: &str) -> String {
    let mut writer = skein_lib::Writer::new(text.len().div_ceil(3) * 4);
    skein_lib::base64::encode(skein_lib::base64::Alphabet::Standard, text.as_bytes(), &mut writer).expect("base64");
    String::from_utf8(writer.finish().into_vec()).expect("base64 text")
}
fn assert_contexts(status: &Value, event: &str, changes: &str) {
    let rows = status["statuses"].as_array().expect("two status contexts");
    assert_eq!(rows.len(), 2, "{status}");
    for (job, state) in [("check", "success"), ("changes", changes)] {
        let context = format!("/ {job} ({event})");
        let row = rows.iter().find(|row| row["context"] == context).expect("exact Forgejo job context");
        assert_eq!(row["status"], state, "{context}: {status}");
    }
}

#[must_use]
pub fn steps(paths: &[PathBuf; 3], client: &Path, began: Time) -> Vec<Step> {
    let mut steps = vec![Step::Run(Call::sign_in()), Step::Run(Call::new(&["forge", "setup", "3", "--playground"]))];
    let gate_paths = paths.clone();
    steps.push(Step::Then(Box::new(move |_| {
        Forge::person(&gate_paths).install();
        eprintln!("owned CSV main gate passed: {} ms", now().saturating_since(began).as_nanos() / 1_000_000);
        Call::new(&["forge", "list", "--json"])
    })));
    let mut task = first_repository::csv_task_steps(client);
    // The existing CSV steps up to the first reply, followed by the repair's
    // phase, then the existing pending-review/approval/landing steps.
    let tail = task.split_off(5);
    steps.extend(task);
    steps.push(Step::Until(Call::new(&["change", "show", "2", "--json"]), br#""runs":3"#));
    steps.push(Step::Run(Call::new(&["task", "show", "3", "--json"])));
    steps.push(Step::Then(Box::new(|calls| {
        let producer = super::ctl::json(calls.last().expect("producer task"));
        eprintln!("owned CSV producer task: {producer}");
        Call::new(&["change", "show", "2", "--json"])
    })));
    steps.push(Step::Until(Call::new(&["change", "show", "2", "--json"]), br#""step":"repairing""#));
    let red_paths = paths.clone();
    steps.push(Step::Then(Box::new(move |calls| {
        let panel = super::ctl::json(calls.last().expect("repairing panel"));
        assert_eq!(panel["ci"], "failed");
        assert_eq!(panel["repairs"]["used"], 1);
        Forge::at(&red_paths).head(&panel, "failure");
        eprintln!("owned CSV H1 failed, repair delegated: {} ms", now().saturating_since(began).as_nanos() / 1_000_000);
        Call::new(&["task", "show", &panel["runs"].as_u64().expect("repair task").to_string(), "--json"])
    })));
    steps.push(Step::ReleaseLlm { name: b"csv-repair", cue: REPAIR.as_bytes() });
    // The shared steps capture pending review at H2; add the forge observation
    // immediately before releasing that review.
    for step in tail {
        if matches!(step, Step::ReleaseLlm { name: b"csv-review", .. }) {
            let green_paths = paths.clone();
            steps.push(Step::Then(Box::new(move |calls| {
                let panel = super::ctl::json(calls.last().expect("pending review panel"));
                Forge::at(&green_paths).head(&panel, "success");
                let red = calls
                    .iter()
                    .filter_map(|call| serde_json::from_slice::<Value>(&call.stdout).ok())
                    .find(|panel| panel["step"] == "repairing")
                    .expect("H1");
                assert_ne!(red["head"], panel["head"], "repair pushed H2 on the same pull");
                eprintln!(
                    "owned CSV H2 passed, review pending: {} ms",
                    now().saturating_since(began).as_nanos() / 1_000_000
                );
                Call::new(&["change", "show", "2", "--json"])
            })));
        }
        steps.push(step);
    }
    steps
}

#[must_use]
pub fn landed(paths: &[PathBuf; 3], client: &Path, began: Time) -> Value {
    let panel = super::ctl::alone(client, Call::new(&["change", "show", "2", "--json"]));
    let panel = super::ctl::json(&panel);
    let forge = Forge::at(paths);
    let pull = forge.get("pulls/1");
    assert_eq!(panel["step"], "landed");
    assert_eq!(panel["gates"][0]["verdict"], "approved");
    assert_eq!(pull["merged"], true);
    assert_eq!(pull["head"]["sha"], panel["head"]);
    assert_eq!(pull["merge_commit_sha"], panel["merge"]);
    let main = forge.get("branches/main");
    assert_eq!(main["commit"]["id"], panel["merge"]);
    let merge = panel["merge"].as_str().expect("merge SHA");
    assert_eq!(forge.contents("changes/1.md", merge), NOTE);
    assert!(forge.contents("src/main.rs", merge).contains("fruit,count"), "main holds CSV header");
    eprintln!("owned CSV landed: {} ms; {panel}", now().saturating_since(began).as_nanos() / 1_000_000);
    panel
}

pub fn assert_briefs(model: &super::forge_land::Seen, landed: &Value) {
    let brief = |cue: &str| {
        model
            .requests
            .iter()
            .find_map(|query| {
                let system = String::from_utf8_lossy(&query.system);
                system.contains(cue).then(|| system.into_owned())
            })
            .unwrap_or_else(|| panic!("model never received {cue}"))
    };
    let repair = brief(REPAIR);
    eprintln!("owned CSV repair brief: {repair}");
    assert!(repair.contains("/ changes (pull_request)"), "repair brief names the failing context: {repair}");
    assert!(repair.contains("Missing changes/1.md"), "repair brief carries the real failed step's output: {repair}");
    let review = brief(REVIEW);
    eprintln!("owned CSV review brief: {review}");
    assert!(review.contains("Files and diff:"), "review's pull section contains its H2 diff: {review}");
    assert!(review.contains(landed["head"].as_str().expect("H2")), "review names H2: {review}");
    let diff = review.split_once("Files and diff:").expect("review diff").1;
    for text in ["src/main.rs", "changes/1.md", "After:\n", "fruit,count", NOTE.trim()] {
        assert!(diff.contains(text), "review diff includes {text}: {review}");
    }
    let mut received = String::new();
    for query in &model.requests {
        if String::from_utf8_lossy(&query.system).contains("[Requested by person 1]") {
            for message in &query.messages {
                if message.role == skein_fake_llm_domain::api::Role::User {
                    for part in &message.parts {
                        match part {
                            skein_fake_llm_domain::api::Part::Text { text }
                            | skein_fake_llm_domain::api::Part::ToolOutput { output: text, .. } => {
                                received.push_str(&String::from_utf8_lossy(text));
                            }
                            skein_fake_llm_domain::api::Part::Opaque { .. }
                            | skein_fake_llm_domain::api::Part::ToolCall { .. } => {}
                        }
                    }
                }
            }
        }
    }
    let head = landed["head"].as_str().expect("landed head");
    assert!(received.contains(&format!("checks at {head}:")), "requester received checks: {received}");
    assert!(received.contains("/ check (pull_request) passed"), "{received}");
    assert!(received.contains("/ changes (pull_request) passed"), "{received}");
    assert!(received.contains("repairs: 1: CI failed: / changes (pull_request)"), "{received}");
    assert!(received.contains(&format!("reviews: approved at {head}")), "{received}");
}
