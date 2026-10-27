//! A chat adds a landing dependency to a live report delegate.
use super::{README, admit};
use serde_json::json;
use skein_fake_llm_domain::api::{Finish, Line, Script, Turn};

// Fresh deployment: chat T1, batch e1 T2 and a3 T3; e1's coding child follows.
// The story checks these numbers against the client's final task documents.
pub const E1: u64 = 2;
pub const A3: u64 = 3;
pub const AFTER_MET: &str = "After Met: the change landed; a3 is done.";

#[must_use]
pub fn fixture() -> crate::forge::Fixture {
    crate::forge::Fixture {
        setup: super::fixture().setup,
        configure: |value, agent, forge| {
            (super::fixture().configure)(value, agent, forge);
            for project in value["world_settings"]["policies"].as_array_mut().expect("projects") {
                project["policy"]["ceiling"]["tools"] = 5.into();
                project["policy"]["roles"][0]["authority"]["tools"] = 5.into();
            }
            for name in ["tasks.batch", "authority.batch"] {
                value["engine"]["limits"]["overrides"][name] = 2.into();
            }
            crate::fixtures::own_policy(value);
            admit(value);
        },
        ..crate::forge::fixture()
    }
}

fn call(name: &[u8], value: &serde_json::Value) -> Turn {
    Turn {
        lines: Box::new([Line::Call {
            name: name.into(),
            arguments: serde_json::to_vec(value).expect("tool JSON").into_boxed_slice(),
        }]),
        finish: Finish::ToolCalls,
        tokens: 20,
    }
}

#[must_use]
pub fn scripts(forge: std::net::SocketAddr, repository: u64) -> Box<[Script]> {
    let mut scripts = super::scripts(forge, repository, README).into_vec();
    let Line::Call { arguments, .. } = &scripts[0].turns[0].lines[0] else { panic!("delegate") };
    let mut delegate: serde_json::Value = serde_json::from_slice(arguments).expect("delegate JSON");
    delegate["batch"].as_array_mut().expect("batch").push(json!({
        "executor":{"kind":"agent","charter":1},
        "spec":{"words":"@kept_a3 Continue after landing","parameters":[]},
        "contract":{"kind":"report","words":128},
        "authority":{"tools":0,"grants":[],"delegation":{"kinds":[],"tasks":0,"depth":0},
            "budget":{"spend":10},"notes":0}
    }));
    scripts[0] = Script {
        cue: b"@kept_chat".as_slice().into(),
        turns: Box::new([
            call(b"delegate", &delegate),
            call(
                b"amend",
                &json!({"target":A3,"revision":0,"amendment":{
                "dependencies":[E1],"reason":"a3 must wait for e1 landing"}}),
            ),
            Turn {
                lines: Box::new([
                    Line::Text { text: b"Delegated and amended; waiting.".as_slice().into() },
                    Line::Call { name: b"wait".as_slice().into(), arguments: b"{}".as_slice().into() },
                ]),
                finish: Finish::ToolCalls,
                tokens: 20,
            },
            call(b"finish", &json!({"form":"report","text":"Both delegates ended.","fields":{}})),
        ]),
    };
    scripts.push(Script {
        cue: b"@kept_a3".as_slice().into(),
        turns: Box::new([call(b"finish", &json!({"form":"report","text":AFTER_MET,"fields":{}}))]),
    });
    scripts.into_boxed_slice()
}
