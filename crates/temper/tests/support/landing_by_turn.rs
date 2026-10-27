//! The person's API edits and forge evidence for an owned turn landing.
use super::{ctl, first_repository};
use serde_json::{Value, json};
use skein_lib::base64::{self, Alphabet};
use std::{fs, path::Path};

pub const WORKFLOW: &str = r#"on:
  workflow_dispatch:
    inputs:
      temper_commit: { required: true }
      temper_key: { required: true }
jobs:
  test:
    runs-on: temper-host
    steps:
      - run: |
          auth=$(printf 'x-access-token:%s' "${{ github.token }}" | base64 -w0)
          git -c "http.extraHeader=Authorization: basic $auth" clone "${{ github.server_url }}/${{ github.repository }}.git" .
          git checkout "${{ inputs.temper_commit }}" && sh check.sh
"#;
pub const NOTES: &[u8] = b"Main advanced after the task pushed.\n";

pub struct Person {
    api: String,
    header: String,
}
impl Person {
    pub fn new(root: &Path) -> Self {
        let config: Value = serde_json::from_slice(&fs::read(root.join("temper.json")).expect("configuration"))
            .expect("configuration JSON");
        let forge = config["application"]["forges"]
            .as_array()
            .expect("forges")
            .iter()
            .find(|forge| forge["number"] == 3)
            .expect("owned forge");
        Self {
            api: format!("http://{}/api/v1", forge["api"]["socket"].as_str().expect("forge socket")),
            header: root.join("forge-3/person_write.header").to_str().expect("header path").into(),
        }
    }
    pub fn request(&self, method: &str, route: &str, body: Option<Value>, status: u16) -> Value {
        let body = body.map(|body| body.to_string());
        let (received, value) = first_repository::header_request(
            &self.api,
            &self.header,
            method,
            &format!("/repos/work/hello/{route}"),
            body.as_deref(),
        );
        assert_eq!(received, status, "{method} {route}: {value}");
        value
    }
    pub fn get(&self, route: &str) -> Value {
        self.request("GET", route, None, 200)
    }
    pub fn add(&self, path: &str, content: &[u8]) {
        let size = base64::encoded_len(Alphabet::Standard, u32::try_from(content.len()).expect("file size"))
            .expect("base64 size");
        let mut out = skein_lib::Writer::new(usize::try_from(size).expect("base64 fits"));
        base64::encode(Alphabet::Standard, content, &mut out).expect("base64 file");
        let encoded = out.finish();
        self.request(
            "POST",
            &format!("contents/{path}"),
            Some(json!({
                "branch":"main", "message":format!("Add {path}"),
                "content":std::str::from_utf8(&encoded).expect("base64 text")
            })),
            201,
        );
    }
    pub fn contents(&self, path: &str) -> Vec<u8> {
        let file = self.get(&format!("contents/{path}?ref=main"));
        let encoded: Vec<_> = file["content"]
            .as_str()
            .expect("file content")
            .bytes()
            .filter(|byte| !byte.is_ascii_whitespace())
            .collect();
        base64::decode(Alphabet::Standard, &encoded, 1 << 20).expect("base64 content").into_vec()
    }
}

pub struct Setup {
    pub project: u64,
    pub repository: u64,
}
pub fn configure_repository(root: &Path) -> Setup {
    let person = Person::new(root);
    person.add("check.sh", b"test -s README.md\n");
    person.add(".forgejo/workflows/gate.yaml", WORKFLOW.as_bytes());
    let repo = person.get("");
    let me = ctl::alone(root, ctl::Call::new(&["me", "--json"]));
    let me = ctl::json(&me);
    // Project 1 is init's seed; setup adopted the first repository into its own project.
    let project = me["projects"]
        .as_array()
        .expect("projects")
        .iter()
        .find(|project| project["project"] != 1)
        .expect("first repository project")["project"]
        .as_u64()
        .expect("project number");
    Setup { project, repository: repo["id"].as_u64().expect("repository id") }
}
pub fn configure_turn(root: &Path, setup: &Setup) {
    let path = root.join("temper.json");
    let mut config: Value = serde_json::from_slice(&fs::read(&path).expect("configuration")).expect("JSON");
    config["application"]["landing"]["projects"].as_array_mut().expect("project landings").push(json!({
        "project":setup.project, "by":"turn", "rules":[],
        "gates":[{"forge":3, "repository":setup.repository,
            "gates":[{"name":"gate", "workflow":"gate.yaml", "ignored":["docs"], "blocking":true}]}]
    }));
    super::write_private(&path, &serde_json::to_vec(&config).expect("configuration JSON"));
}

/// The stock reserves policy 1's whole period to role 0's pool, which leaves its
/// project's landing task no spend (temper#16): this deployment keeps 7K of 48K free.
pub const ROLE_PERIOD_IN_K: u64 = 41;
const PERIOD_IN_K: u64 = 48;

/// Leave policy 1's projects spend for landing tasks, applying against the newest
/// revision read from the inventory and preserving the policy's other members.
pub fn fund_landings(root: &Path, policy: &super::Ended, inventory: &super::Ended) -> ctl::Call {
    let file = root.join("landing-settings.json");
    ctl::setting_change_file(&file, policy, |value| {
        let period = value["period_spend"].as_u64().expect("policy period spend");
        let role = value["roles"]
            .as_array_mut()
            .expect("policy roles")
            .iter_mut()
            .find(|role| role["number"] == 0)
            .expect("owner role");
        role["period_spend"] = json!(period / PERIOD_IN_K * ROLE_PERIOD_IN_K);
    });
    ctl::settings_change(inventory, &file, "landing-settings")
}

pub struct Evidence {
    pub list: super::Ended,
    pub show: super::Ended,
    pub panel: super::Ended,
    pub change: super::Ended,
    pub main: Value,
    pub pull: Value,
    pub commit: Value,
    pub runs: Value,
    pub status: Value,
    pub readme: Vec<u8>,
    pub notes: Vec<u8>,
}
pub fn evidence(root: &Path, setup: &Setup, change: u64) -> Evidence {
    let list = ctl::alone(
        root,
        ctl::Call::new(&["landing", "list", "--project", &setup.project.to_string(), "--ended", "--json"]),
    );
    let panel = ctl::json(&list);
    let landing = panel["landings"]
        .as_array()
        .expect("landing rows")
        .iter()
        .find(|row| row["changes"].as_array().expect("changes").contains(&json!(change)))
        .expect("the change's landing")["task"]
        .as_u64()
        .expect("landing task")
        .to_string();
    let show = ctl::alone(root, ctl::Call::new(&["landing", "show", &landing]));
    let panel = ctl::alone(root, ctl::Call::new(&["landing", "show", &landing, "--json"]));
    let change = ctl::alone(root, ctl::Call::new(&["change", "show", &change.to_string()]));
    let person = Person::new(root);
    let main = person.get("branches/main");
    let head = main["commit"]["id"].as_str().expect("main tip");
    Evidence {
        list,
        show,
        panel,
        change,
        pull: person.get("pulls/1"),
        commit: person.get(&format!("git/commits/{head}")),
        runs: person.get(&format!("actions/runs?head_sha={head}&limit=100")),
        status: person.get(&format!("commits/{head}/status")),
        readme: person.contents("README.md"),
        notes: person.contents("notes.txt"),
        main,
    }
}
