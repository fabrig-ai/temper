//! A goal asks its person before calling change, through the shipped box and ctl.
use super::{
    ctl::{Call, Step},
    forge_land::{self, Plan},
};
use serde_json::json;
use skein_fake_llm_domain::api::{Finish, Line, Script, Turn};

pub const REQUEST: &str = "Agree the README plan with me before appending: The agreed plan was completed.";
#[must_use]
pub fn words() -> String {
    let mut words = "I will append the agreed line to README.md, preserve its existing contents, and let the change task run its checks and review before it lands. ".to_owned();
    words.push_str(&"x".repeat(207 - words.len()));
    assert_eq!(words.len(), 207);
    words
}
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
fn wait() -> Turn {
    call(b"wait", &json!({}))
}
#[must_use]
pub fn scripts(plan: &Plan<'_>) -> Box<[Script]> {
    let mut scripts = forge_land::plan_scripts(plan).into_vec();
    let Line::Call { arguments, .. } = &scripts[0].turns[0].lines[0] else { panic!("change call") };
    let change: serde_json::Value = serde_json::from_slice(arguments).expect("change arguments");
    scripts[0].turns = Box::new([
        call(b"goal", &json!({"words":"@long-plan-goal Agree the README plan, then call change."})),
        wait(),
        wait(),
        call(b"message", &json!({"target":2,"form":"answer","question":1,"words":"Go ahead."})),
        wait(),
        Turn {
            lines: Box::new([Line::Text { text: forge_land::LANDED.as_bytes().into() }]),
            finish: Finish::Stop,
            tokens: 20,
        },
    ]);
    scripts.insert(
        1,
        Script {
            cue: b"@long-plan-goal".as_slice().into(),
            turns: Box::new([
                call(b"message", &json!({"target":1,"form":"question","words":words()})),
                wait(),
                call(b"change", &change),
                wait(),
                call(b"finish", &json!({"form":"report","text":"The change landed.","fields":{}})),
            ]),
        },
    );
    scripts.into_boxed_slice()
}
#[must_use]
pub fn steps() -> (Vec<Step>, usize, usize) {
    let mut steps = forge_land::forgejo_setup_steps("first");
    for args in [
        &["account", "connect", "1", "--key", "long-account"][..],
        &["account", "list", "--until", "settled", "--json"],
        &["chat", "start", "--project", "2", "--model", "gpt-6-luna", "--key", "long-chat", REQUEST, "--json"],
        &["task", "show", "1", "--until", "waiting-on-me", "--timeout", "60s", "--json"],
    ] {
        steps.push(Step::Run(Call::new(args)));
    }
    steps.push(Step::Run(Call::new(&["task", "show", "1", "--until", "phase:idle", "--timeout", "60s", "--json"])));
    let inbox = steps.iter().filter(|step| matches!(step, Step::Run(_))).count();
    steps.push(Step::Run(Call::new(&["inbox", "--kind", "questions"])));
    steps.push(Step::Run(Call::new(&["task", "activity", "2", "--json"])));
    let answer = steps.iter().filter(|step| matches!(step, Step::Run(_))).count();
    steps.push(Step::Run(Call::new(&["task", "answer", "1", "1", "Go ahead.", "--key", "long-answer"])));
    steps.push(Step::Run(Call::new(&["task", "show", "2", "--until", "ended", "--timeout", "60s", "--json"])));
    steps.push(Step::Run(Call::new(&["task", "show", "3", "--json"])));
    steps.push(Step::Run(Call::new(&["task", "show", "1", "--json"])));
    (steps, inbox, answer)
}
