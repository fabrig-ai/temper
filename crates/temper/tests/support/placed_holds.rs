//! Executable owner-release stories in one fresh deployment.
use super::{Ended, boxed, ctl};
use ctl::{Call, Step};
use serde_json::{Value, json};
use skein_fake_llm_domain::api::{Finish, Line, Part, Query, Role, Rule, Script, Turn};
use skein_io::kernel::Exit;
use std::path::Path;

// Creating and parking tasks consumes task identities, not message identities.
// No message precedes the stop, which takes the deployment's first message number.
const STOP_HOLD: u64 = 1;
const HOLD_REVISION: u64 = 1;
const TASK_HOLD_REVISION: u64 = 1;
// The owner's resume message takes 2; the coordinator's proposal takes 3.
const PROPOSAL: &str = "t1/3";

const GOAL: &str = "@held-goal";
const CHILD: &str = "@held-child";
const PARKED_GOAL: &[u8] = b"\"task\":2,\"title\":{\"bytes\":\"@held-goal\",\"cut\":false},\"executor\":{\"agent\":{\"charter\":2}},\"phase\":\"idle\"";
const PARKED_CHILD: &[u8] = b"\"task\":3,\"title\":{\"bytes\":\"@held-child\",\"cut\":false},\"executor\":{\"agent\":{\"charter\":2}},\"phase\":\"idle\"";

fn call(name: &[u8], arguments: &Value) -> Turn {
    Turn {
        lines: Box::new([Line::Call {
            name: name.into(),
            arguments: serde_json::to_vec(arguments).expect("tool JSON").into(),
        }]),
        finish: Finish::ToolCalls,
        tokens: 20,
    }
}
fn wait() -> Turn {
    let mut turn = call(b"wait", &json!({}));
    turn.lines = Box::new([
        Line::Text { text: b"Waiting for the owner's direction.".as_slice().into() },
        Line::Call { name: b"wait".as_slice().into(), arguments: b"{}".as_slice().into() },
    ]);
    turn
}
fn delegate(words: &str, children: bool) -> Turn {
    call(
        b"delegate",
        &json!({"batch":[{
            "executor":{"kind":"agent","charter":2},
            "tracked":{"priority":1},
            "spec":{"words":words,"parameters":[]},
            "contract":{"kind":"report","words":128},
            "llm":{"main":{"model":0}},
            "authority":{"tools":if children {3} else {0},"grants":[],
                "delegation":{"kinds":if children { json!([{"kind":"agent","number":2}]) } else { json!([]) },
                    "tasks":u8::from(children),"depth":u8::from(children)},
                "budget":{"spend":if children {20_000} else {8_000}},"notes":0}
        }]}),
    )
}
fn scripts() -> Box<[Script]> {
    Box::new([
        Script {
            cue: b"@hold-chat".as_slice().into(),
            turns: Box::new([
                delegate(GOAL, true),
                wait(),
                call(b"release", &json!({"target":2,"revision":TASK_HOLD_REVISION})),
                call(
                    b"propose",
                    &json!({"action":"release","task":2,"hold":STOP_HOLD,
                "revision":HOLD_REVISION,"reason":"Please resume the stopped goal"}),
                ),
                wait(),
            ]),
        },
        Script { cue: GOAL.as_bytes().into(), turns: Box::new([delegate(CHILD, false), wait()]) },
        Script {
            cue: CHILD.as_bytes().into(),
            turns: Box::new([call(b"read_docs", &json!({"topic":"hold"})), wait()]),
        },
    ])
}
fn rules(entries: &[(&str, u32)]) -> Box<[Rule]> {
    entries.iter().map(|(cue, turn)| Rule { cue: cue.as_bytes().into(), turn: *turn }).collect()
}
fn llm(config: &Path, expected_exits: Vec<(usize, u8)>) -> boxed::CtlLlm {
    let config = config.to_path_buf();
    let recipe = temper_real_loop_world::llm::played_by_input(
        scripts(),
        Box::new([
            rules(&[
                ("Ask to resume the stopped goal", 2),
                ("Refused: task 2 is held by hold", 3),
                ("Proposed as ", 4),
                ("Begin the work your brief describes.", 0),
                ("", 1),
            ]),
            rules(&[("Begin the work your brief describes.", 0), ("", 1)]),
            rules(&[("Begin the work your brief describes.", 0), ("", 1)]),
        ]),
    );
    let peer = super::llm::Listening::new(
        skein_fake_peers::llm::Peer::new(
            (std::net::Ipv4Addr::LOCALHOST, 0).into(),
            skein_fake_peers::Transport::Tls,
            recipe.limits,
            recipe.config,
            recipe.credential,
            recipe.protocol_limits,
            recipe.domain,
            recipe.domain_limits,
        )
        .expect("scripted holds peer"),
    );
    let address = peer.address();
    let path = config.join("agent.json");
    let mut agent: Value =
        serde_json::from_slice(&std::fs::read(&path).expect("agent configuration")).expect("agent JSON");
    agent["endpoints"][0]["address"] = address.to_string().into();
    agent["endpoints"][0]["server_name"] = "skein.test".into();
    agent["endpoints"][0]["trust_der"] = json!(config.join("root.der"));
    super::write_private(&config.join("root.der"), skein_tls_world::pki::ROOT);
    super::write_private(&path, &serde_json::to_vec(&agent).expect("agent JSON"));
    boxed::CtlLlm { peer, expected_exits, on_link: Box::new(|| {}) }
}
fn initialize(scratch: &Path, flags: &[String]) {
    let initialized = std::process::Command::new(skein_scratch::paths::binary("temper"))
        .arg("init")
        .args(flags)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .current_dir(scratch)
        .output()
        .expect("initialize story deployment");
    assert!(initialized.status.success(), "{}", String::from_utf8_lossy(&initialized.stderr));
}
fn initial_steps(scratch: &Path) -> Vec<Step> {
    let settings_file = scratch.join("hold-settings.json");
    let key = scratch.join("llm-key");
    super::write_private(&key, jig_channel_world::fixtures::KEY);
    vec![
        Step::Run(Call::sign_in()), // 0
        Step::Run(Call::new(&[
            "account",
            "add-key",
            "1",
            key.to_str().expect("UTF-8 fixture key path"),
            "--key",
            "hold-account",
        ])), // 1
        Step::Run(Call::new(&["settings", "show", "rules", "--json"])), // 2
        Step::Run(Call::new(&["settings", "show", "policy", "1", "--json"])), // 3
        Step::Then(Box::new(move |calls| {
            let rules_document = ctl::json(&calls[2]);
            let mut rules = rules_document["value"].clone();
            let mut policy = ctl::json(&calls[3])["value"].clone();
            // Give this story's coordinator control and propose, families 4 and 16,
            // through the public settings path, before creating its project.
            let tools = rules["ceiling"]["tools"].as_u64().expect("rules tool families") | 4 | 16;
            rules["ceiling"]["tools"] = tools.into();
            policy["ceiling"]["tools"] = tools.into();
            policy["roles"][0]["authority"]["tools"] = tools.into();
            let mut maintainer = policy["roles"][0].clone();
            maintainer["number"] = json!(1);
            policy["roles"].as_array_mut().expect("policy roles").push(maintainer);
            let change = json!({"items":[
                {"setting":{"kind":"rules"},"set":rules},
                {"setting":{"kind":"policy","number":1},"set":policy}
            ]});
            super::write_private(&settings_file, &serde_json::to_vec(&change).expect("settings change JSON"));
            Call::new(&[
                "settings",
                "apply",
                settings_file.to_str().expect("UTF-8 settings path"),
                "--against",
                &rules_document["revision"].to_string(),
                "--key",
                "hold-tools",
            ])
        })), // 4
        Step::Project { scratch: scratch.into(), client: scratch.join("client"), name: "held-work", number: 2 },
        Step::Run(Call::new(&["me", "--json"])), // 5
        Step::Maintainer { scratch: scratch.into(), directory: "maintainer", project: 2 },
        Step::Run(Call::new(&["me", "--json"]).in_directory("maintainer")), // 6
        Step::Run(Call::new(&[
            "chat",
            "start",
            "--project",
            "2",
            "--key",
            "hold-chat",
            "@hold-chat Delegate and wait",
        ])), // 7
        Step::Until(Call::new(&["task", "show", "3", "--json"]), PARKED_CHILD), // 8
        Step::Until(Call::new(&["task", "show", "2", "--json"]), PARKED_GOAL), // 9
        Step::Until(Call::new(&["task", "show", "1", "--json"]), b"\"phase\":\"idle\""), // 10
        Step::Run(Call::new(&["task", "stop", "2", "--key", "stop-goal"])), // 11
        Step::Run(Call::new(&["task", "show", "2", "--json"])),             // 12
        Step::Run(Call::new(&["task", "show", "3", "--json"])),             // 13
        Step::Run(Call::new(&["task", "run", "2"])),                        // 14
        Step::Run(Call::new(&["task", "run", "3"])),                        // 15
        Step::Run(Call::new(&["task", "show", "2"])),                       // 16
    ]
}
fn check_box(seen: &boxed::CtlOutcome<()>, expected_exits: &[(usize, u8)]) {
    assert_eq!(seen.exit, Some(Exit::Code(0)), "{}", String::from_utf8_lossy(&seen.stderr));
    assert_eq!(seen.left, Some(Vec::new()), "no surviving process");
    for (at, call) in seen.calls.iter().enumerate() {
        let code = expected_exits.iter().find(|(index, _)| *index == at).map_or(0, |(_, code)| *code);
        assert_eq!(call.exit, Some(Exit::Code(code)), "call {at}: {}", String::from_utf8_lossy(&call.stderr));
    }
}
fn placed(calls: &[Ended]) -> Value {
    assert_eq!(calls[11].stdout, b"done: task 2, stop\n");
    let task = ctl::json(&calls[12]);
    let child = ctl::json(&calls[13]);
    assert_eq!(task["chip"]["phase"], "held");
    assert_eq!(child["chip"]["phase"], "held");
    assert_eq!(task["hold"]["placed"], child["hold"]["placed"]);
    assert_eq!(task["hold"]["placed"]["owner"]["person"], ctl::json(&calls[5])["party"]["number"]);
    assert_eq!(task["hold"]["placed"]["may_lift"], true);
    assert!(String::from_utf8_lossy(&calls[14].stdout).starts_with("run: none"));
    assert!(String::from_utf8_lossy(&calls[15].stdout).starts_with("run: none"));
    assert!(String::from_utf8_lossy(&calls[16].stdout).contains("you may lift it"));
    assert_eq!(task["hold"]["placed"]["hold"], STOP_HOLD, "derived message counter");
    assert_eq!(task["hold"]["placed"]["revision"], HOLD_REVISION);
    assert_eq!(task["hold"]["owner"]["revision"], TASK_HOLD_REVISION);
    // Chip attempts retain the last claimed identity even after a run parks.
    // The run documents above prove no attempt remains live after the stop.
    task["hold"]["placed"].clone()
}
fn queries<'a>(seen: &'a super::forge_land::Seen, cue: &[u8]) -> Vec<&'a Query> {
    seen.requests.iter().filter(|query| query.system.windows(cue.len()).any(|bytes| bytes == cue)).collect()
}
fn transcript(seen: &super::forge_land::Seen, cue: &[u8]) {
    let requests = queries(seen, cue);
    let old = requests.get(1).expect("the request after the first committed tool turn");
    let new = requests.get(2).expect("the resumed model request");
    let mut saved = 0;
    let mut remaining = new.messages.iter();
    for message in old.messages.iter().filter(|message| {
        message.role == Role::Assistant || message.parts.iter().any(|part| matches!(part, Part::ToolOutput { .. }))
    }) {
        assert!(
            remaining.any(|next| next == message),
            "committed turns restored in order for {}",
            String::from_utf8_lossy(cue)
        );
        saved += 1;
    }
    assert!(saved >= 2, "a nonempty assistant/tool transcript before stop");
}

pub fn proposal(scratch: &Path, config: &Path, flags: Vec<String>) {
    initialize(scratch, &flags);
    let mut steps = initial_steps(scratch);
    steps.extend([
        Step::Then(Box::new(|calls| {
            let task = ctl::json(&calls[12]);
            Call::new(&[
                "task",
                "release",
                "2",
                "--hold",
                &task["hold"]["owner"]["revision"].to_string(),
                "--key",
                "maintainer-release",
            ])
            .in_directory("maintainer")
        })), // 17
        Step::Run(Call::new(&["task", "show", "2"]).in_directory("maintainer")), // 18
        Step::Run(Call::new(&["task", "message", "1", "Ask to resume the stopped goal", "--key", "request-release"])), // 19
        Step::Until(Call::new(&["inbox"]), PROPOSAL.as_bytes()), // 20
        Step::Run(Call::new(&["proposal", "show", PROPOSAL])),   // 21
        Step::Run(Call::new(&["proposal", "decide", PROPOSAL, "accept", "--key", "owner-accept"])), // 22
        Step::Until(Call::new(&["task", "show", "2", "--json"]), PARKED_GOAL), // 23
        Step::Until(Call::new(&["task", "show", "3", "--json"]), PARKED_CHILD), // 24
    ]);
    let seen = boxed::run_ctl_with_llm(
        scratch,
        flags,
        scratch,
        steps,
        boxed::BOX_DEADLINE,
        |_| (),
        llm(config, vec![(17, 2)]),
    );
    check_box(&seen, &[(17, 2)]);
    assert_eq!(seen.calls.len(), 25);
    let hold = placed(&seen.calls);
    let owner = hold["owner"]["person"].as_u64().expect("placed hold owner");
    let number = hold["hold"].as_u64().expect("placed hold number");
    let revision = hold["revision"].as_u64().expect("placed hold revision");
    assert_eq!(seen.calls[17].stderr, format!("task release: Held by hold {number}, placed by person {owner}: only they, or a role above theirs, may release it\n").as_bytes());
    assert!(
        String::from_utf8_lossy(&seen.calls[18].stdout)
            .contains(&format!("only person {owner}, or a role above theirs, may release it"))
    );
    assert!(
        String::from_utf8_lossy(&seen.calls[21].stdout)
            .contains(&format!("release task 2 from hold {number} (revision {revision})"))
    );
    assert!(ctl::json(&seen.calls[23])["hold"].is_null());
    assert!(ctl::json(&seen.calls[24])["hold"].is_null());
    let model = seen.model.as_ref().expect("scripted model observations");
    transcript(model, GOAL.as_bytes());
    transcript(model, CHILD.as_bytes());
    let refusal = format!(
        "Error: Refused: task 2 is held by hold {number} (revision {revision}), placed by person {owner}; only they may release it. Propose it: {{\"action\": \"release\", \"task\": 2, \"hold\": {number}, \"revision\": {revision}, \"reason\": \"...\"}}"
    );
    assert!(
        queries(model, b"@hold-chat").iter().any(|query| query.messages.iter().any(|message| message
            .parts
            .iter()
            .any(|part| matches!(part, Part::ToolOutput { output, .. } if output.as_ref() == refusal.as_bytes())))),
        "coordinator hears the owner refusal"
    );
}

pub fn lift(scratch: &Path, config: &Path, flags: Vec<String>) {
    initialize(scratch, &flags);
    let mut steps = initial_steps(scratch);
    steps.extend([
        Step::Run(Call::new(&[
            "hold",
            "lift",
            &STOP_HOLD.to_string(),
            "--rev",
            "1",
            "--with-tasks",
            "--key",
            "lift-goal",
        ])), // 17
        Step::Until(Call::new(&["task", "show", "2", "--json"]), PARKED_GOAL), // 18
        Step::Until(Call::new(&["task", "show", "3", "--json"]), PARKED_CHILD), // 19
        Step::Run(Call::new(&["task", "stop", "2", "--key", "retained-stop"])), // 20
        Step::Run(Call::new(&["task", "show", "2", "--json"])),                // 21
        Step::Then(Box::new(|calls| {
            let task = ctl::json(&calls[21]);
            Call::new(&[
                "hold",
                "lift",
                &task["hold"]["placed"]["hold"].to_string(),
                "--rev",
                "1",
                "--key",
                "retained-lift",
            ])
        })), // 22
        Step::Then(Box::new(|calls| {
            let task = ctl::json(&calls[21]);
            Call::new(&[
                "hold",
                "lift",
                &task["hold"]["placed"]["hold"].to_string(),
                "--rev",
                "1",
                "--key",
                "stale-lift",
            ])
        })), // 23
        Step::Run(Call::new(&["task", "show", "2", "--json"])),                // 24
        Step::Run(Call::new(&["task", "show", "3", "--json"])),                // 25
    ]);
    let seen = boxed::run_ctl_with_llm(
        scratch,
        flags,
        scratch,
        steps,
        boxed::BOX_DEADLINE,
        |_| (),
        llm(config, vec![(23, 2)]),
    );
    check_box(&seen, &[(23, 2)]);
    assert_eq!(seen.calls.len(), 26);
    placed(&seen.calls);
    assert_eq!(seen.calls[17].stdout, format!("done: hold {STOP_HOLD} lifted, 2 released\n").as_bytes());
    assert!(ctl::json(&seen.calls[18])["hold"].is_null());
    assert!(ctl::json(&seen.calls[19])["hold"].is_null());
    let retained = ctl::json(&seen.calls[21])["hold"]["placed"]["hold"].as_u64().expect("retained hold number");
    assert_eq!(seen.calls[22].stdout, format!("done: hold {retained} lifted, 0 released\n").as_bytes());
    assert!(
        String::from_utf8_lossy(&seen.calls[23].stderr).contains("revision now: 2"),
        "{}",
        String::from_utf8_lossy(&seen.calls[23].stderr)
    );
    for call in &seen.calls[24..=25] {
        let task = ctl::json(call);
        assert_eq!(task["chip"]["phase"], "held");
        assert_eq!(task["hold"]["placed"]["hold"], retained);
        assert_eq!(task["hold"]["placed"]["lifted"], true);
        assert_eq!(task["hold"]["placed"]["revision"], 2);
    }
}
