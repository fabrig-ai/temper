//! client/tools.md, 11.4: two live tasks keep their charter until an owner moves them.
use super::{Ended, ctl};
use serde_json::json;
use skein_fake_llm_domain::api::{Finish, Line, Script, Turn};

// The delegate document includes its ancestor chat chip; select charter 2's own chip.
pub const PARKED_DELEGATE: &[u8] = b"\"executor\":{\"agent\":{\"charter\":2}},\"phase\":\"idle\"";
pub const INSTRUCTIONS: &str = "Read the owner's newly revised delegate instructions.";

fn call(name: &[u8], arguments: &serde_json::Value) -> Turn {
    Turn {
        lines: Box::new([Line::Call {
            name: name.into(),
            arguments: serde_json::to_vec(arguments).expect("tool JSON").into_boxed_slice(),
        }]),
        finish: Finish::ToolCalls,
        tokens: 20,
    }
}
#[must_use]
pub fn scripts() -> Box<[Script]> {
    Box::new([
        Script {
            cue: b"@revision-chat".as_slice().into(),
            turns: Box::new([
                call(
                    b"delegate",
                    &json!({"batch":[{
                        "executor":{"kind":"agent","charter":2},
                        "spec":{"words":"@revision-delegate Wait for the owner's message","parameters":[]},
                        "contract":{"kind":"report","words":128},
                        "authority":{"tools":0,"grants":[],"delegation":{"kinds":[],"tasks":0,"depth":0},
                            "budget":{"spend":10},"notes":0}
                    }]}),
                ),
                call(b"wait", &json!({})),
            ]),
        },
        Script {
            cue: b"@revision-delegate".as_slice().into(),
            turns: Box::new([call(b"wait", &json!({})), call(b"wait", &json!({}))]),
        },
    ])
}

#[must_use]
pub fn llm(config: &std::path::Path) -> super::boxed::CtlLlm {
    let config = config.to_path_buf();
    let peer = super::fake_llm(scripts());
    let address = peer.address();
    super::boxed::CtlLlm {
        peer,
        expected_exits: Vec::new(),
        on_link: Box::new(move || {
            let path = config.join("agent.json");
            let mut agent: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).expect("agent")).expect("JSON");
            agent["endpoints"][0]["address"] = address.to_string().into();
            agent["endpoints"][0]["server_name"] = "skein.test".into();
            agent["endpoints"][0]["trust_der"] = json!(config.join("root.der"));
            super::write_private(&config.join("root.der"), skein_tls_world::pki::ROOT);
            super::write_private(&path, &serde_json::to_vec(&agent).expect("agent JSON"));
        }),
    }
}

pub fn assert_story(calls: &[Ended], model: &super::forge_land::Seen) {
    let before = ctl::json(&calls[5]);
    let after = ctl::json(&calls[14]);
    assert_eq!(before["chip"]["phase"], "idle");
    assert_eq!(after["chip"]["phase"], "idle");
    let first_run = before["chip"]["details"]["owner"]["details"]["attempt"].as_u64().expect("first run");
    let next_run = after["chip"]["details"]["owner"]["details"]["attempt"].as_u64().expect("next run");
    assert!(next_run > first_run, "the delegate ran again");
    let revision = ctl::newest(&calls[2]);
    let newest = revision + 1;
    assert_eq!(calls[7].stdout, format!("revision {newest}\n").as_bytes());
    let task = String::from_utf8_lossy(&calls[8].stdout);
    assert!(task.lines().any(|line| line == format!("settings: revision {revision} · newest {newest}")), "{task}");
    let inventory = ctl::json(&calls[9]);
    assert_eq!(inventory["newest"], newest);
    let held = inventory["held"].as_array().expect("held revisions");
    let old_held = held.iter().find(|row| row["head"]["number"] == revision).expect("R is held");
    assert_eq!(old_held["holders"], 2, "{inventory}");
    let old = ctl::json(&calls[3]);
    let old_instructions = old["value"]["instructions"].as_str().expect("old instructions").as_bytes();
    assert!(!old_instructions.is_empty());
    let old_text = String::from_utf8_lossy(&calls[10].stdout);
    assert!(old_text.starts_with(&format!("charter 2, revision {revision}\n")), "{old_text}");
    let printed_instructions = serde_json::to_string(&old["value"]["instructions"]).expect("instruction JSON");
    assert!(old_text.contains(&printed_instructions), "{old_text}");
    assert_eq!(calls[11].stdout, format!("moved 2 tasks to revision {newest}\n").as_bytes());
    assert!(after["settings"]["run"].is_null(), "delegate parked: {after}");
    // Both tasks are parked again, so this count includes two distinct delegate runs.
    let delegate: Vec<_> = model
        .requests
        .iter()
        .filter(|query| query.system.windows(b"@revision-delegate".len()).any(|part| part == b"@revision-delegate"))
        .collect();
    assert_eq!(delegate.len(), 2, "delegate's two run requests");
    assert!(delegate[0].system.windows(old_instructions.len()).any(|part| part == old_instructions));
    assert!(!delegate[0].system.windows(INSTRUCTIONS.len()).any(|part| part == INSTRUCTIONS.as_bytes()));
    assert!(delegate[1].system.windows(INSTRUCTIONS.len()).any(|part| part == INSTRUCTIONS.as_bytes()));
    let inventory = ctl::json(&calls[15]);
    assert_eq!(inventory["newest"], newest);
    let held = inventory["held"].as_array().expect("held revisions");
    assert!(held.iter().all(|row| row["head"]["number"] != revision), "{inventory}");
    let newest_held = held.iter().find(|row| row["head"]["number"] == newest).expect("newest holders");
    assert_eq!(newest_held["holders"], 2, "the chat and delegate remain live: {inventory}");
    assert!(model.answers.iter().all(Result::is_ok), "{:?}", model.answers);
}
