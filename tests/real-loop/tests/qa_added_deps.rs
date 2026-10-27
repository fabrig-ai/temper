//! Landing dependencies and the words delivered to hosted agents.
use skein_fake_llm_domain::api::{self, Script};
use std::net::SocketAddr;
use temper_real_loop_world::{
    Ran, argv,
    calls::{self, D1},
    forge, landing,
};

// Sign-in, adoption, chat, its idle wait, the report wait, then its document.
const CHAT_CALL: usize = 2;
const REPORT_WAIT: usize = 4;
const REPORT_DOCUMENT: usize = 5;
// Fresh refusal deployment: one root and its two report delegates.
const PARENT_TASK: u64 = 1;
const REPORT_TASK: u64 = 2;
const PREREQUISITE_TASK: u64 = 3;
// Creating an unrelated root first shifts the parent's delegates by one.
const OUTSIDE_TASK: u64 = 1;
const SHIFTED_REPORT_TASK: u64 = 3;
const FIRST_PARENT_CALL: usize = 1;
const PARENT_CALL_AFTER_OUTSIDE: usize = 3;

fn landing_scripts(forge: SocketAddr) -> Box<[Script]> {
    landing::kept::scripts(forge, u64::from(forge::ID))
}

fn landed_report() -> Ran {
    temper_real_loop_world::run_forge(landing::kept::fixture(), landing_scripts, || {
        vec![
            calls::sign_in(D1),
            calls::call(
                D1,
                argv![
                    "adopt",
                    "--project",
                    "1",
                    "--forge",
                    "1",
                    "--owner",
                    "acme",
                    "--name",
                    "app",
                    "--landing",
                    "main",
                    "--prefix",
                    "temper/",
                    "--key",
                    "qa-adopt"
                ],
            ),
            calls::call(D1, argv!["chat", "--project", "1", "--key", "qa-kept", "@kept_chat"]),
            calls::call(D1, argv!["wait", "task", calls::task(CHAT_CALL), "--until", "phase:idle", "--timeout", "5"]),
            calls::call(
                D1,
                argv!["wait", "task", calls::number(landing::kept::A3), "--until", "ended", "--timeout", "5"],
            ),
            calls::call(D1, argv!["task", calls::number(landing::kept::A3), "--json"]),
            calls::call(D1, argv!["wait", "task", calls::task(CHAT_CALL), "--until", "ended", "--timeout", "5"]),
        ]
    })
}

#[test]
fn landing_releases_the_kept_report_delegate() {
    // draft/reference-case.md 4.2, 4.3; jig's tasks.md 5.7, 8.1.
    let ran = landed_report();
    let waited = &ran.calls()[REPORT_WAIT];
    assert_eq!(
        waited.exit,
        Some(temper_real_loop_world::Exit::Code(0)),
        "kept report must finish after landing: {}",
        String::from_utf8_lossy(&waited.stderr)
    );
    assert!(ran.forge().merged_green(), "the dependency must be released by a successful landing");
    let document = String::from_utf8_lossy(&ran.calls()[REPORT_DOCUMENT].stdout);
    assert!(document.contains(landing::kept::AFTER_MET), "resumed report must be retained: {document}");
}

fn agent_text(ran: &Ran, cue: &str) -> String {
    ran.llm()
        .iter()
        .filter_map(|record| {
            if let skein_fake_peers::llm::Observation::Query { query, .. } = record {
                String::from_utf8_lossy(&query.system).contains(cue).then_some(query)
            } else {
                None
            }
        })
        .flat_map(|query| query.messages.iter())
        .flat_map(|message| message.parts.iter())
        .filter_map(|part| {
            if let api::Part::Text { text } = part { Some(String::from_utf8_lossy(text).into_owned()) } else { None }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn met_notice_tells_the_agent_which_dependency_finished() {
    // jig's domain/tasks.md 8.1: the core tells the resumed agent its last dependency is met.
    let ran = landed_report();
    let text = agent_text(&ran, "@kept_a3");
    assert!(
        text.contains("T2 is done; the task can go on"),
        "the resumed agent must hear an identifiable Met notice for task 2, rather than an empty numbered message: {text:?}"
    );
}

fn call_turn(name: &[u8], arguments: &serde_json::Value) -> api::Turn {
    api::Turn {
        lines: Box::new([api::Line::Call {
            name: name.into(),
            arguments: serde_json::to_vec(arguments).expect("tool JSON").into(),
        }]),
        finish: api::Finish::ToolCalls,
        tokens: 20,
    }
}

fn report_turn() -> api::Turn {
    call_turn(b"finish", &serde_json::json!({"form":"report","text":"Reported.","fields":{}}))
}

fn refusal_scripts(target: u64, dependency: u64, cancel: bool) -> Box<[Script]> {
    use serde_json::json;
    let report = |words| {
        json!({
            "executor":{"kind":"agent","charter":1},
            "spec":{"words":words,"parameters":[]},
            "contract":{"kind":"report","words":128},
            "authority":{"tools":0,"grants":[],"delegation":{"kinds":[],"tasks":0,"depth":0},
                "budget":{"spend":10},"notes":0}
        })
    };
    let mut turns = vec![call_turn(b"delegate", &json!({"batch":[report("@qa-report"),report("@qa-prerequisite")]}))];
    if cancel {
        turns.push(call_turn(b"cancel", &json!({"target":dependency,"reason":"cancel before amendment"})));
    }
    turns.extend([
        call_turn(
            b"amend",
            &json!({"target":target,"revision":0,
            "amendment":{"dependencies":[dependency],"reason":"new prerequisite"}}),
        ),
        call_turn(b"wait", &json!({})),
        report_turn(),
    ]);
    if cancel {
        // The cancelled delegate's result can wake the wait before the report's arrives: finish again once it has.
        turns.push(report_turn());
    }
    Box::new([
        Script { cue: b"@qa-parent".as_slice().into(), turns: turns.into() },
        Script { cue: b"@qa-report".as_slice().into(), turns: Box::new([report_turn()]) },
        Script { cue: b"@qa-prerequisite".as_slice().into(), turns: Box::new([report_turn()]) },
        Script { cue: b"@qa-outside".as_slice().into(), turns: Box::new([call_turn(b"wait", &json!({}))]) },
    ])
}

fn refusal_fixture() -> forge::Fixture {
    let mut fixture = landing::kept::fixture();
    fixture.configure = |value, agent, forge| {
        (landing::kept::fixture().configure)(value, agent, forge);
        // The kept fixture grants delegate + amend (5); cancel adds its bit (8).
        value["engine"]["rules"]["ceiling"]["tools"] = 13.into();
        for project in value["engine"]["projects"].as_array_mut().expect("projects") {
            project["policy"]["ceiling"]["tools"] = 13.into();
            project["policy"]["roles"][0]["authority"]["tools"] = 13.into();
        }
    };
    fixture
}

fn refusal_story(scripts: fn(SocketAddr) -> Box<[Script]>, outside: bool) -> Ran {
    temper_real_loop_world::run_forge(refusal_fixture(), scripts, || {
        let mut steps = vec![calls::sign_in(D1)];
        if outside {
            // Sign-in is call zero; the outside chat is call one.
            const OUTSIDE_CHAT: usize = 1;
            steps.extend([
                calls::call(D1, argv!["chat", "--project", "1", "--key", "qa-outside", "@qa-outside"]),
                calls::call(
                    D1,
                    argv!["wait", "task", calls::task(OUTSIDE_CHAT), "--until", "phase:idle", "--timeout", "5"],
                ),
            ]);
        }
        let parent_call = if outside { PARENT_CALL_AFTER_OUTSIDE } else { FIRST_PARENT_CALL };
        steps.extend([
            calls::call(D1, argv!["chat", "--project", "1", "--key", "qa-parent", "@qa-parent"]),
            calls::call(D1, argv!["wait", "task", calls::task(parent_call), "--until", "ended", "--timeout", "5"]),
        ]);
        steps
    })
}

fn refusal_words(ran: &Ran) -> String {
    ran.llm()
        .iter()
        .filter_map(|record| {
            if let skein_fake_peers::llm::Observation::Query { query, .. } = record { Some(query) } else { None }
        })
        .flat_map(|query| query.messages.iter())
        .flat_map(|message| message.parts.iter())
        .filter_map(|part| {
            if let api::Part::ToolOutput { output, .. } = part {
                let text = String::from_utf8_lossy(output);
                text.contains("Refused").then(|| text.into_owned())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn circle_refusal_names_the_waiting_tasks_to_the_agent() {
    // jig's domain/tasks.md 6.7, 7; engine.md 7.3: a refused circle names its tasks.
    // Fresh deployment: parent 1, report delegate 2, prerequisite delegate 3.
    let ran = refusal_story(|_| refusal_scripts(REPORT_TASK, PARENT_TASK, false), false);
    let words = refusal_words(&ran);
    assert!(
        words.contains("wait for one another") && words.contains("T1") && words.contains("T2"),
        "the agent must hear why task 2 after its requester 1 circles: {words:?}"
    );
}

#[test]
fn unreachable_dependency_refusal_names_the_task_to_the_agent() {
    // jig's domain/tasks.md 7; engine.md 7.3: a new dependency must be in the amender's reach.
    // Outside root 1, parent 2, report delegate 3, prerequisite delegate 4.
    let ran = refusal_story(|_| refusal_scripts(SHIFTED_REPORT_TASK, OUTSIDE_TASK, false), true);
    let words = refusal_words(&ran);
    assert!(
        words.contains('1')
            && (words.to_ascii_lowercase().contains("depend") || words.to_ascii_lowercase().contains("reach")),
        "the agent must hear which dependency is outside its reach: {words:?}"
    );
}

#[test]
fn cancelled_dependency_refusal_names_the_task_to_the_agent() {
    // jig's domain/tasks.md 7; engine.md 7.3: adding a cancelled prerequisite is refused, naming it.
    let ran = refusal_story(|_| refusal_scripts(REPORT_TASK, PREREQUISITE_TASK, true), false);
    let words = refusal_words(&ran);
    assert!(
        words.contains('3')
            && (words.to_ascii_lowercase().contains("cancel") || words.to_ascii_lowercase().contains("depend")),
        "the agent must hear that dependency 3 ended cancelled: {words:?}"
    );
}

fn failed_prerequisite_scripts(_: SocketAddr) -> Box<[Script]> {
    use serde_json::json;
    let mut scripts = refusal_scripts(REPORT_TASK, PREREQUISITE_TASK, false);
    let parent = scripts.iter_mut().find(|script| script.cue.as_ref() == b"@qa-parent").expect("parent script");
    let mut original = parent.turns.iter();
    let delegate = original.next().expect("delegation turn").clone();
    let amend = original.next().expect("amendment turn").clone();
    parent.turns = Box::new([
        delegate,
        call_turn(b"wait", &json!({})),
        amend,
        call_turn(b"cancel", &json!({"target":REPORT_TASK,"reason":"report cannot continue"})),
        report_turn(),
        // The cancelled report's result refuses the first finish: finish again once it is read.
        report_turn(),
    ]);
    scripts.iter_mut().find(|script| script.cue.as_ref() == b"@qa-report").expect("report script").turns =
        Box::new([call_turn(b"wait", &json!({}))]);
    scripts.iter_mut().find(|script| script.cue.as_ref() == b"@qa-prerequisite").expect("prerequisite script").turns =
        Box::new([call_turn(b"finish", &json!({"form":"failure","reason":"declared failure","fields":{}}))]);
    scripts
}

#[test]
fn failed_dependency_refusal_names_the_task_to_the_agent() {
    // jig's domain/tasks.md 7; engine.md 7.3: adding a failed prerequisite is refused, naming it.
    let ran = refusal_story(failed_prerequisite_scripts, false);
    let words = refusal_words(&ran);
    assert!(
        words.contains('3')
            && (words.to_ascii_lowercase().contains("fail") || words.to_ascii_lowercase().contains("depend")),
        "the agent must hear that dependency 3 ended failed: {words:?}"
    );
}
