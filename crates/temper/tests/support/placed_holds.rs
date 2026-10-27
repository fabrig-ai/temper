//! Executable owner-release stories, with the fake's next scripts built from ctl's placed hold.
use super::{Ended, boxed, ctl};
use ctl::{Call, Step};
use serde_json::{Value, json};
use skein_fake_llm_domain::api::{Finish, Line, Part, Query, Role, Rule, Script, Turn};
use skein_io::kernel::Exit;
use std::path::Path;

const GOAL: &str = "@held-goal";
const CHILD: &str = "@held-child";
const PARKED: &[u8] = b"\"executor\":{\"agent\":{\"charter\":2}},\"phase\":\"idle\"";

fn call(name: &[u8], arguments: &Value) -> Turn {
    Turn {
        lines: Box::new([Line::Call { name: name.into(), arguments: serde_json::to_vec(arguments).unwrap().into() }]),
        finish: Finish::ToolCalls,
        tokens: 20,
    }
}
fn wait() -> Turn {
    call(b"wait", &json!({}))
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
fn scripts(hold: Option<&Value>) -> Box<[Script]> {
    let mut chat = vec![if hold.is_some() { wait() } else { delegate(GOAL, true) }, wait()];
    if let Some(hold) = hold {
        chat.extend([
            call(b"release", &json!({"target":2,"revision":hold["task_revision"]})),
            call(
                b"propose",
                &json!({"action":"release","task":2,"hold":hold["hold"],
                "revision":hold["revision"],"reason":"Please resume the stopped goal"}),
            ),
            wait(),
        ]);
    } else {
        chat.extend([wait(), wait(), wait()]);
    }
    Box::new([
        Script { cue: b"@hold-chat".as_slice().into(), turns: chat.into_boxed_slice() },
        Script {
            cue: GOAL.as_bytes().into(),
            turns: Box::new([if hold.is_some() { wait() } else { delegate(CHILD, false) }, wait()]),
        },
        Script {
            cue: CHILD.as_bytes().into(),
            turns: Box::new([
                if hold.is_some() { wait() } else { call(b"read_docs", &json!({"topic":"hold"})) },
                wait(),
            ]),
        },
    ])
}
fn rules(entries: &[(&str, u32)]) -> Box<[Rule]> {
    entries.iter().map(|(cue, turn)| Rule { cue: cue.as_bytes().into(), turn: *turn }).collect()
}
fn llm(config: &Path, hold: Option<&Value>, expected_exits: Vec<(usize, u8)>) -> boxed::CtlLlm {
    let config = config.to_path_buf();
    let recipe = temper_real_loop_world::llm::played_by_input(
        scripts(hold),
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
    let mut agent: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    agent["endpoints"][0]["address"] = address.to_string().into();
    agent["endpoints"][0]["server_name"] = "skein.test".into();
    agent["endpoints"][0]["trust_der"] = json!(config.join("root.der"));
    super::write_private(&config.join("root.der"), skein_tls_world::pki::ROOT);
    super::write_private(&path, &serde_json::to_vec(&agent).unwrap());
    boxed::CtlLlm { peer, expected_exits, on_link: Box::new(|| {}) }
}
fn initialize(scratch: &Path, config: &Path, flags: &[String]) {
    let initialized = std::process::Command::new(skein_scratch::paths::binary("temper"))
        .arg("init")
        .args(flags)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .current_dir(scratch)
        .output()
        .expect("initialize story deployment");
    assert!(initialized.status.success(), "{}", String::from_utf8_lossy(&initialized.stderr));
    let path = config.join("temper.json");
    let mut document: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    // MakeService requires an actual policy role. Give this story's maintainer
    // the existing policy's authority, with its distinct built-in rank.
    for project in document["engine"]["projects"].as_array_mut().unwrap() {
        let roles = project["policy"]["roles"].as_array_mut().unwrap();
        let mut maintainer = roles[0].clone();
        maintainer["number"] = json!(1);
        roles.push(maintainer);
    }
    super::write_private(&path, &serde_json::to_vec(&document).unwrap());
}
fn initial_steps(scratch: &Path) -> Vec<Step> {
    let key = scratch.join("llm-key");
    super::write_private(&key, jig_channel_world::fixtures::KEY);
    vec![
        Step::Run(Call::sign_in()), // 0
        Step::Run(Call::new(&["account", "add-key", "1", key.to_str().unwrap(), "--key", "hold-account"])), // 1
        Step::Run(Call::new(&["me", "--json"])), // 2
        Step::Maintainer { scratch: scratch.into(), directory: "maintainer" },
        Step::Run(Call::new(&["me", "--json"]).in_directory("maintainer")), // 3
        Step::Run(Call::new(&[
            "chat",
            "start",
            "--project",
            "1",
            "--key",
            "hold-chat",
            "@hold-chat Delegate and wait",
        ])), // 4
        Step::Until(Call::new(&["task", "show", "3", "--json"]), PARKED),   // 5
        Step::Until(Call::new(&["task", "show", "2", "--json"]), PARKED),   // 6
        Step::Until(Call::new(&["task", "show", "1", "--json"]), b"\"phase\":\"idle\""), // 7
        Step::Run(Call::new(&["task", "stop", "2", "--key", "stop-goal"])), // 8
        Step::Run(Call::new(&["task", "show", "2", "--json"])),             // 9
        Step::Run(Call::new(&["task", "show", "3", "--json"])),             // 10
        Step::Run(Call::new(&["task", "run", "2"])),                        // 11
        Step::Run(Call::new(&["task", "run", "3"])),                        // 12
        Step::Run(Call::new(&["task", "show", "2"])),                       // 13
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
    assert_eq!(calls[8].stdout, b"done: task 2, stop\n");
    let task = ctl::json(&calls[9]);
    let child = ctl::json(&calls[10]);
    assert_eq!(task["chip"]["phase"], "held");
    assert_eq!(child["chip"]["phase"], "held");
    assert_eq!(task["hold"]["placed"], child["hold"]["placed"]);
    assert_eq!(task["hold"]["placed"]["owner"]["person"], ctl::json(&calls[2])["party"]["number"]);
    assert_eq!(task["hold"]["placed"]["may_lift"], true);
    assert!(String::from_utf8_lossy(&calls[11].stdout).starts_with("run: none"));
    assert!(String::from_utf8_lossy(&calls[12].stdout).starts_with("run: none"));
    assert!(String::from_utf8_lossy(&calls[13].stdout).contains("you may lift it"));
    let mut hold = task["hold"]["placed"].clone();
    hold["task_revision"] = task["hold"]["owner"]["revision"].clone();
    hold
}
fn queries<'a>(seen: &'a super::forge_land::Seen, cue: &[u8]) -> Vec<&'a Query> {
    seen.requests.iter().filter(|query| query.system.windows(cue.len()).any(|bytes| bytes == cue)).collect()
}
fn transcript(before: &super::forge_land::Seen, after: &super::forge_land::Seen, cue: &[u8]) {
    let old = queries(before, cue);
    let new = queries(after, cue);
    let old = old.last().expect("the stopped live model request");
    let new = new.first().expect("the resumed model request");
    let mut saved = 0;
    for message in old.messages.iter().filter(|message| message.role == Role::Assistant) {
        assert!(new.messages.contains(message), "earlier assistant turn restored for {}", String::from_utf8_lossy(cue));
        saved += 1;
    }
    assert!(saved > 0, "a nonempty committed transcript before stop");
    for message in &old.messages {
        for part in &message.parts {
            if let Part::ToolOutput { .. } = part {
                assert!(
                    new.messages.iter().any(|message| message.parts.contains(part)),
                    "earlier tool result restored"
                );
            }
        }
    }
}
/// Stop and inspect using the first fixed scripts; resume with scripts built from the observed hold.
pub fn proposal(scratch: &Path, config: &Path, flags: Vec<String>) {
    initialize(scratch, config, &flags);
    let mut steps = initial_steps(scratch);
    steps.extend([
        Step::Then(Box::new(|calls| {
            let task = ctl::json(&calls[9]);
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
        })), // 14
        Step::Run(Call::new(&["task", "show", "2"]).in_directory("maintainer")), // 15
    ]);
    let first = boxed::run_ctl_with_llm(
        scratch,
        flags.clone(),
        scratch,
        steps,
        boxed::BOX_DEADLINE,
        |_| (),
        llm(config, None, vec![(14, 2)]),
    );
    check_box(&first, &[(14, 2)]);
    assert_eq!(
        first.calls.len(),
        16,
        "last task {}; model answers {:?}; requests {}",
        String::from_utf8_lossy(&first.calls.last().unwrap().stdout),
        first.model.as_ref().map(|m| &m.answers),
        first.model.as_ref().map_or(0, |m| m.requests.len())
    );
    for before in [&first.calls[5], &first.calls[6]] {
        let before = ctl::json(before);
        assert!(
            before["chip"]["details"]["owner"]["details"]["attempt"]
                .as_u64()
                .expect("the cancellation story needs live attempts before the stop")
                > 0
        );
    }
    let hold = placed(&first.calls);
    let owner = hold["owner"]["person"].as_u64().unwrap();
    let number = hold["hold"].as_u64().unwrap();
    let revision = hold["revision"].as_u64().unwrap();
    assert_eq!(first.calls[14].stderr, format!("release: Held by hold {number}, placed by person {owner}: only they, or a role above theirs, may release it\n").as_bytes());
    assert!(
        String::from_utf8_lossy(&first.calls[15].stdout)
            .contains(&format!("only person {owner}, or a role above theirs, may release it"))
    );
    let second = boxed::run_ctl_with_llm(
        scratch,
        flags,
        scratch,
        vec![
            Step::Run(Call::new(&[
                "task",
                "message",
                "1",
                "Ask to resume the stopped goal",
                "--key",
                "request-release",
            ])),
            Step::Until(Call::new(&["inbox"]), b"t1/1"),
            Step::Run(Call::new(&["proposal", "show", "t1/1"])),
            Step::Run(Call::new(&["proposal", "decide", "t1/1", "accept", "--key", "owner-accept"])),
            Step::Until(Call::new(&["task", "show", "2", "--json"]), PARKED),
            Step::Until(Call::new(&["task", "show", "3", "--json"]), PARKED),
        ],
        boxed::BOX_DEADLINE,
        |_| (),
        llm(config, Some(&hold), Vec::new()),
    );
    check_box(&second, &[]);
    assert_eq!(second.calls.len(), 6);
    assert!(
        String::from_utf8_lossy(&second.calls[2].stdout)
            .contains(&format!("release task 2 from hold {number} (revision {revision})"))
    );
    assert!(ctl::json(&second.calls[4])["hold"].is_null());
    assert!(ctl::json(&second.calls[5])["hold"].is_null());
    let before = first.model.as_ref().unwrap();
    let after = second.model.as_ref().unwrap();
    transcript(before, after, GOAL.as_bytes());
    transcript(before, after, CHILD.as_bytes());
    let refusal = format!(
        "Refused: task 2 is held by hold {number} (revision {revision}), placed by person {owner}; only they may release it. Propose it: {{\"action\": \"release\", \"task\": 2, \"hold\": {number}, \"revision\": {revision}, \"reason\": \"...\"}}"
    );
    assert!(queries(after,b"@hold-chat").iter().any(|query| query.messages.iter().any(|message| message.parts.iter().any(|part| matches!(part, Part::ToolOutput { output, is_error: true, .. } if output.as_ref() == refusal.as_bytes())))),"coordinator hears the owner refusal");
}

pub fn lift(scratch: &Path, config: &Path, flags: Vec<String>) {
    initialize(scratch, config, &flags);
    let first = boxed::run_ctl_with_llm(
        scratch,
        flags.clone(),
        scratch,
        initial_steps(scratch),
        boxed::BOX_DEADLINE,
        |_| (),
        llm(config, None, Vec::new()),
    );
    check_box(&first, &[]);
    let hold = placed(&first.calls);
    let number = hold["hold"].as_u64().unwrap();
    let revision = hold["revision"].as_u64().unwrap();
    let second = boxed::run_ctl_with_llm(
        scratch,
        flags,
        scratch,
        vec![
            Step::Run(Call::new(&[
                "hold",
                "lift",
                &number.to_string(),
                "--rev",
                &revision.to_string(),
                "--with-tasks",
                "--key",
                "lift-goal",
            ])),
            Step::Run(Call::new(&[
                "hold",
                "lift",
                &number.to_string(),
                "--rev",
                &revision.to_string(),
                "--with-tasks",
                "--key",
                "stale-lift",
            ])),
            Step::Until(Call::new(&["task", "show", "2", "--json"]), PARKED),
            Step::Until(Call::new(&["task", "show", "3", "--json"]), PARKED),
        ],
        boxed::BOX_DEADLINE,
        |_| (),
        llm(config, Some(&hold), vec![(1, 2)]),
    );
    check_box(&second, &[(1, 2)]);
    assert_eq!(second.calls[0].stdout, format!("done: hold {number} lifted, 2 released\n").as_bytes());
    assert!(
        String::from_utf8_lossy(&second.calls[1].stderr).contains("revision now: 2"),
        "{}",
        String::from_utf8_lossy(&second.calls[1].stderr)
    );
    assert!(ctl::json(&second.calls[2])["hold"].is_null());
    assert!(ctl::json(&second.calls[3])["hold"].is_null());
}
