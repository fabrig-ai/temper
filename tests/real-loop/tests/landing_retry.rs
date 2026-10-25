//! A producer parks after delivery and finishes its existing diff in a new activation.
use serde_json::{Value, json};
use skein_fake_llm_domain::api::{Finish, Line, Piece, Script, Turn};
use temper_real_loop_world::{
    Exit, Ran, argv,
    calls::{self, D1},
    forge, landing, run_forge,
};

const PRODUCER: u64 = 3;
const RETRY: &[u8] = b"producer-retry-ready";
const PARKED: usize = 4;
const RESUMED: usize = 6;
const CHANGE_HELD: usize = 7;
const PRODUCED: usize = 9;

fn fixture() -> forge::Fixture {
    forge::Fixture {
        configure: |value, agent, forge| {
            (landing::fixture().configure)(value, agent, forge);
            // This smoke owns producer retry, with CI on the existing README diff.
            // The producer's LLM barrier keeps it unlanded until the new hold finishes.
            value["application"]["landing"]["projects"][0]["rules"][0]["gates"] = json!([]);
        },
        ..landing::fixture()
    }
}

fn call(name: &[u8], value: &Value) -> Turn {
    Turn {
        lines: Box::new([Line::Call {
            name: name.into(),
            arguments: serde_json::to_vec(value).expect("tool JSON").into(),
        }]),
        finish: Finish::ToolCalls,
        tokens: 20,
    }
}
fn scripts(at: std::net::SocketAddr) -> Box<[Script]> {
    let mut scripts = landing::scripts(at, u64::from(forge::ID), landing::README);
    let coder = scripts.iter_mut().find(|script| script.cue.as_ref() == b"@coding Fix").expect("producer script");
    let mut turns = std::mem::take(&mut coder.turns).into_vec();
    let mut finish = turns.pop().expect("producer finish");
    turns.push(call(b"deliver", &json!({"fields":{"title":"Update README","body":"Changed by temper."}})));
    turns.push(call(b"wait", &json!({})));
    // The person's message creates the next activation. Hold its finish until
    // the caller has observed that new attempt: no CI/gate/merge can overtake it.
    let mut lines = finish.lines.into_vec();
    lines.insert(
        0,
        Line::Pieces {
            pieces: Box::new([Piece {
                text: b"Finishing the existing change.".as_slice().into(),
                hold: Some(RETRY.into()),
            }]),
        },
    );
    finish.lines = lines.into();
    turns.push(finish);
    coder.turns = turns.into();
    scripts
}
fn document(ran: &Ran, call: usize) -> Value {
    let mut value: Value = serde_json::from_slice(&ran.calls()[call].stdout).expect("task JSON");
    if value["task"].is_object() { value["task"].take() } else { value }
}

#[test]
fn a_retried_producer_finishes_its_existing_change() {
    let ran = run_forge(fixture(), scripts, || {
        vec![
            calls::sign_in(D1),
            calls::call(
                D1,
                argv![
                    "adopt",
                    "--project",
                    "1",
                    "--owner",
                    "acme",
                    "--name",
                    "app",
                    "--landing",
                    "main",
                    "--key",
                    "adopt-app"
                ],
            ),
            calls::call(D1, argv!["chat", "--project", "1", "--key", "retry", "@forge_chat Land a small change"]),
            calls::call(D1, argv!["wait", "task", "1", "--until", "replied"]),
            calls::call(D1, argv!["wait", "task", calls::number(PRODUCER), "--until", "phase:idle", "--json"]),
            calls::call(
                D1,
                argv![
                    "message",
                    calls::number(PRODUCER),
                    "Finish the existing change without another edit.",
                    "--key",
                    "retry-producer"
                ],
            ),
            calls::until(D1, argv!["task", calls::number(PRODUCER), "--json"], b"\"work\":\"writing\"", b"\"task\""),
            calls::call(D1, argv!["task", "2", "--json"]),
            calls::release_hold(RETRY),
            calls::call(D1, argv!["wait", "task", "2", "--until", "ended"]),
            calls::call(D1, argv!["task", calls::number(PRODUCER), "--json"]),
        ]
    });
    for (index, call) in ran.calls().iter().enumerate() {
        assert_eq!(call.exit, Some(Exit::Code(0)), "call {index}: {call:?}");
    }
    let parked = document(&ran, PARKED);
    let resumed = document(&ran, RESUMED);
    assert_eq!(parked["chip"]["phase"], "idle", "{parked}");
    let attempt =
        |value: &Value| value["chip"]["details"]["owner"]["details"]["attempt"].as_u64().expect("activation attempt");
    assert!(attempt(&resumed) > attempt(&parked), "a new activation after the parked one");
    assert_ne!(document(&ran, CHANGE_HELD)["chip"]["phase"], "ended", "finish barrier keeps the change unlanded");
    let produced = document(&ran, PRODUCED);
    assert!(
        produced["chip"]["details"]["owner"]["details"]["ending"]["done"]["change"].is_object(),
        "producer done(change): {produced}"
    );
    assert!(ran.forge().merged_green());
    assert_eq!(ran.forge().main_file(b"README.md"), [landing::README, landing::EDIT].concat());
}
