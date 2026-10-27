//! The person's inbox green light lets the work delegate a CSV change and land it.
//! The harness names its one fake repository acme/app; it stands as the first
//! run's playground/widget, with widget's src/main.rs in its tree.
use crate::{
    Exit, Ran, argv,
    calls::{self, D1, number},
    forge, landing,
};
use serde_json::{Value, json};
use skein_fake_forge_domain::api;
use skein_fake_llm_domain::api::{Finish, Line, Part, Query, Role, Script, Turn};
use std::net::SocketAddr;

// Fresh deployment numbers, checked against ctl documents and the LLM queries.
// Jig numbers messages deployment-wide; nothing precedes the work's question.
const CHAT: u64 = 1;
const WORK: u64 = 2;
const CHANGE: u64 = 3;
const PRODUCER: u64 = 4;
const QUESTION: u64 = 1;
const REQUEST: &str = "Add CSV export to widget: agree the plan with me before anything is changed.";
const CHAT_SPEND: u64 = 100;
const RUN_SPEND: u64 = 10;
const QUESTION_WORDS: &str = "Export widget's table as CSV in src/main.rs and land it on main. Go ahead?";
const ANSWER: &str = "Go ahead.";
const CHANGE_SPEC: &str = "@green_coding Export widget's table as CSV";
const WORK_REPORT: &str = "The CSV export landed.";
// Fits the chat template's 32-byte report contract.
const CHAT_REPORT: &str = "The CSV export landed.";
const CHAT_DELEGATED: &str = "T2 asks before it starts.";
const CHAT_ASKED: &str = "T2's question waits in your inbox.";
const CHAT_ANSWERED: &str = "Your green light went to T2.";
const WORK_ASKED: &str = "Asked for a green light.";
const WORK_DELEGATED: &str = "The change is under way.";
const CHAT_CUE: &str = "[Requested by person 1]";
const WORK_CUE: &str = "[Requested by task 1]";
const CODING_CUE: &str = "@green_coding";
const REVIEW_CUE: &str = "Review this change at the named head";
const SOURCE: &str = r#"fn main() {
    println!("Name | Value");
    for (name, value) in [("alpha", 1), ("beta", 2)] {
        println!("{name} | {value}");
    }
}
"#;
const CSV_SOURCE: &str = r#"fn main() {
    println!("name,value");
    for (name, value) in [("alpha", 1), ("beta", 2)] {
        println!("{name},{value}");
    }
}
"#;
const CSV_LINE: &str = "    println!(\"name,value\");";

#[must_use]
pub fn fixture() -> forge::Fixture {
    forge::Fixture {
        setup: || {
            let mut setup = (landing::fixture().setup)();
            let mut tree = setup.tree.into_vec();
            tree.push(api::File { path: b"src/main.rs".as_slice().into(), content: SOURCE.as_bytes().into() });
            setup.tree = tree.into_boxed_slice();
            setup.checks.cue =
                Some(api::Cue::File { path: b"src/main.rs".as_slice().into(), green: CSV_LINE.as_bytes().into() });
            setup
        },
        configure: |value, agent, forge| {
            (landing::fixture().configure)(value, agent, forge);
            // o/configuring-stock has not landed Message in stock; tools 3 keeps it for the chat.
            value["engine"]["rules"]["ceiling"]["tools"] = 3.into();
            crate::fixtures::own_policy(value);
            // A tracked goal reserves one full news message beside its question's answer credit.
            value["engine"]["limits"]["overrides"]["tasks.inbox_bytes"] = 8192.into();
            value["engine"]["rules"]["ceiling"]["delegation"]["depth"] = 5.into();
            value["engine"]["templates"][0]["budget"]["as_allowed"] = CHAT_SPEND.into();
            for project in value["engine"]["projects"].as_array_mut().expect("projects") {
                project["policy"]["ceiling"]["tools"] = 3.into();
                project["policy"]["ceiling"]["delegation"]["depth"] = 5.into();
                project["policy"]["roles"][0]["authority"]["tools"] = 3.into();
                project["policy"]["roles"][0]["authority"]["delegation"]["depth"] = 5.into();
                // Two halvings fund the goal and its change while each run reserves ten.
                project["policy"]["roles"][0]["authority"]["budget"]["spend"] = CHAT_SPEND.into();
            }
        },
        ..forge::fixture()
    }
}

fn call(name: &[u8], value: &Value) -> Turn {
    Turn {
        lines: Box::new([Line::Call {
            name: name.into(),
            arguments: serde_json::to_vec(value).expect("tool JSON").into_boxed_slice(),
        }]),
        finish: Finish::ToolCalls,
        tokens: 20,
    }
}
fn wait(words: &str) -> Turn {
    Turn {
        lines: Box::new([
            Line::Text { text: words.as_bytes().into() },
            Line::Call { name: b"wait".as_slice().into(), arguments: b"{}".as_slice().into() },
        ]),
        finish: Finish::ToolCalls,
        tokens: 20,
    }
}
fn change_fields() -> Value {
    json!({"title":"Export widget's table as CSV","body":"Export the widget table with a CSV header and rows."})
}
fn report(words: &str) -> Turn {
    call(b"finish", &json!({"form":"report","text":words,"fields":{}}))
}

#[must_use]
pub fn scripts(_forge: SocketAddr) -> Box<[Script]> {
    // Count-picked turns stay in step: the chat hears question, person, report;
    // the work hears the relayed answer, then its sole delegate's result.
    Box::new([
        Script {
            cue: CHAT_CUE.as_bytes().into(),
            turns: Box::new([
                call(b"goal", &json!({"words":REQUEST})),
                wait(CHAT_DELEGATED),
                wait(CHAT_ASKED),
                call(b"message", &json!({"target":WORK,"form":"answer","question":QUESTION,"words":ANSWER})),
                wait(CHAT_ANSWERED),
                report(CHAT_REPORT),
            ]),
        },
        Script {
            cue: WORK_CUE.as_bytes().into(),
            turns: Box::new([
                call(b"message", &json!({"target":CHAT,"form":"question","words":QUESTION_WORDS})),
                wait(WORK_ASKED),
                call(b"change", &json!({"words":CHANGE_SPEC})),
                wait(WORK_DELEGATED),
                report(WORK_REPORT),
            ]),
        },
        Script {
            cue: CODING_CUE.as_bytes().into(),
            turns: Box::new([
                call(b"read", &json!({"path":"src/main.rs"})),
                call(b"edit", &json!({"path":"src/main.rs","edits":[{"old":SOURCE,"new":CSV_SOURCE}]})),
                // A delivery mid-run, then a finish with nothing new: the finish
                // delivers what the run delivered.
                call(b"deliver", &json!({"fields":change_fields()})),
                call(b"finish", &json!({"form":"change","fields":change_fields()})),
            ]),
        },
        Script {
            cue: REVIEW_CUE.as_bytes().into(),
            turns: Box::new([call(
                b"finish",
                &json!({"form":"verdict","label":"1","text":"Checked","fields":{},"items":[]}),
            )]),
        },
    ])
}

// Recorded call positions; joining a retained watch consumes no call position.
const SIGNED_IN: usize = 0;
const ADOPTED: usize = 1;
const CHAT_STARTED: usize = 2;
const CHAT_ENDED: usize = 3;
const INBOX_WAITING: usize = 4;
const CHAT_IDLE: usize = 5;
const TREE: usize = 6;
const ASKED: usize = 7;
const ACTIVITY: usize = 8;
const ANSWERED: usize = 9;
const INBOX_EMPTY: usize = 10;
const WORK_ENDED: usize = 11;
const PRODUCED: usize = 12;

#[must_use]
pub fn acts() -> Vec<calls::Step> {
    use calls::{call, join, sign_in, start};
    vec![
        sign_in(D1),
        call(
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
        call(D1, argv!["chat", "--project", "1", "--key", "green", REQUEST]),
        start(D1, argv!["wait", "task", number(CHAT), "--until", "ended", "--json"]),
        call(D1, argv!["wait", "inbox", "--until", "waiting"]),
        // The question must outlive the chat's acknowledgement run, parked here.
        call(D1, argv!["wait", "task", number(CHAT), "--until", "phase:idle"]),
        call(D1, argv!["task", number(CHAT), "--tree", "--json"]),
        call(D1, argv!["inbox", "--kind", "questions", "--json"]),
        call(D1, argv!["task", number(CHAT), "--activity", "32", "--json"]),
        call(D1, argv!["answer", number(CHAT), number(QUESTION), ANSWER, "--key", "green-answer"]),
        join(CHAT_ENDED),
        call(D1, argv!["inbox", "--kind", "questions", "--json"]),
        call(D1, argv!["task", number(WORK), "--json"]),
        call(D1, argv!["task", number(PRODUCER), "--json"]),
    ]
}

fn document(ran: &Ran, position: usize) -> Value {
    serde_json::from_slice(&ran.calls()[position].stdout).expect("ctl JSON document")
}

fn check_question(ran: &Ran) {
    const ONE_QUESTION: usize = 1;
    let inbox = document(ran, ASKED);
    let rows = inbox["needs"]["rows"].as_array().expect("questions inbox rows");
    assert_eq!(rows.len(), ONE_QUESTION, "one question in the person's inbox: {inbox}");
    let question = &rows[0];
    assert_eq!(question["task"]["live"]["task"], CHAT, "the question is on the chat");
    assert_eq!(question["object"]["question"]["message"], QUESTION, "the question's message number");
    let activity = document(ran, ACTIVITY);
    let rows = activity["activity"]["rows"].as_array().expect("chat activity rows");
    assert!(
        rows.iter().any(|row| {
            let source = &row["event"]["owner"]["source"];
            let words = &source["line"]["words"];
            source["task"] == CHAT
                && words["message"] == QUESTION
                && words["by"]["task"] == WORK
                && words["kind"] == "question"
                && words["words"] == QUESTION_WORDS
        }),
        "the person's activity read holds T2's question, its number and words: {activity}"
    );
    let tree = document(ran, TREE);
    assert_eq!(tree["tree"]["root"]["task"], CHAT, "the waiting tree is the chat's");
    let rows = tree["tree"]["rows"].as_array().expect("tree rows");
    let children: Vec<_> = rows.iter().map(|row| (&row["chip"]["task"], &row["parent"])).collect();
    assert_eq!(children, vec![(&json!(WORK), &json!(CHAT))], "only T2 exists before change {CHANGE}: {tree}");
    assert_eq!(tree["tree"]["more"], false, "the waiting tree has no hidden page");
    assert_eq!(tree["tree"]["folded"], json!([]), "the waiting tree has no folded tasks");
    let inbox = document(ran, INBOX_EMPTY);
    assert!(inbox["needs"]["rows"].as_array().expect("last inbox rows").is_empty(), "no question remains: {inbox}");
}

fn query_at<'a>(ran: &'a Ran, cue: &str, replies: usize) -> &'a Query {
    ran.llm()
        .iter()
        .find_map(|seen| {
            let skein_fake_peers::llm::Observation::Query { query, .. } = seen else { return None };
            // Delegated specifications can mention other cues; the task's own is first.
            let system = String::from_utf8_lossy(&query.system);
            let first = [CHAT_CUE, WORK_CUE, CODING_CUE, REVIEW_CUE]
                .into_iter()
                .filter_map(|candidate| system.find(candidate).map(|at| (at, candidate)))
                .min();
            let said = query.messages.iter().filter(|message| message.role == Role::Assistant).count();
            (first.is_some_and(|(_, own)| own == cue) && said == replies).then_some(query)
        })
        .expect("the task's scripted query")
}

fn told(query: &Query) -> String {
    let mut words = Vec::new();
    for part in query.messages.iter().filter(|message| message.role == Role::User).flat_map(|message| &message.parts) {
        match part {
            Part::Text { text } | Part::ToolOutput { output: text, .. } => {
                words.extend_from_slice(text);
                words.push(b'\n');
            }
            Part::Opaque { .. } | Part::ToolCall { .. } => {}
        }
    }
    String::from_utf8(words).expect("agent's received words")
}

fn heard_tool(query: &Query, words: &str) -> bool {
    query.messages.iter().flat_map(|message| &message.parts).any(|part| {
        matches!(part, Part::ToolOutput { output, is_error: false, .. }
            if calls::occurrences(output, words.as_bytes()) != 0)
    })
}

fn check_queries(ran: &Ran) {
    // Before each response: T2's wait follows its question; T1's fourth turn
    // relays the answer; T2's third turn delegates the change.
    const WORK_WAIT: usize = 1;
    const CHAT_RELAY: usize = 3;
    const WORK_DELEGATE: usize = 2;
    const CHANGE_DELEGATED: usize = 3;
    let asked = query_at(ran, WORK_CUE, WORK_WAIT);
    let sent = format!("Message {QUESTION} sent");
    assert!(heard_tool(asked, &sent), "T2 was told the question's returned message number: {asked:?}");
    let relay = told(query_at(ran, CHAT_CUE, CHAT_RELAY));
    let question_at = relay.find(QUESTION_WORDS).expect("T1 was told T2's question before its relay");
    let answer_at = relay.find(ANSWER).expect("T1 was told the person's answer before its relay");
    assert!(question_at < answer_at, "T1 hears the question before the person's answer: {relay}");
    let delegate = told(query_at(ran, WORK_CUE, WORK_DELEGATE));
    assert!(delegate.contains(&format!("{CHAT}: {ANSWER}")), "T2 hears T1's answer before delegating: {delegate}");
    assert!(
        heard_tool(query_at(ran, WORK_CUE, CHANGE_DELEGATED), &format!("Delegated tasks: {CHANGE}")),
        "T2 was told the fresh change's task number"
    );
}

pub fn check(ran: &Ran) {
    let calls = [
        SIGNED_IN,
        ADOPTED,
        CHAT_STARTED,
        CHAT_ENDED,
        INBOX_WAITING,
        CHAT_IDLE,
        TREE,
        ASKED,
        ACTIVITY,
        ANSWERED,
        INBOX_EMPTY,
        WORK_ENDED,
        PRODUCED,
    ];
    assert_eq!(ran.calls().len(), calls.len(), "all prescribed ctl calls ran");
    for position in calls {
        assert_eq!(ran.calls()[position].exit, Some(Exit::Code(0)), "ctl call {position}: {:?}", ran.calls()[position]);
    }
    assert_eq!(ran.number(CHAT_STARTED), CHAT, "the fresh chat's task number");
    check_question(ran);
    check_queries(ran);
    assert!(ran.forge().merged_green(), "the change merged with CI green at its head");
    assert_eq!(ran.forge().main_file(b"src/main.rs"), CSV_SOURCE.as_bytes(), "widget's CSV edit landed on main");
    let work = document(ran, WORK_ENDED);
    assert_eq!(work["chip"]["task"], WORK, "T2's ended task number: {work}");
    assert_eq!(work["chip"]["phase"], "ended", "T2 ended");
    assert_eq!(work["result"]["value"]["report"]["bytes"], WORK_REPORT, "T2's report");
    let chat = document(ran, CHAT_ENDED);
    let parent = &chat["task"]["authority"]["authority"];
    let child = &work["authority"]["authority"];
    assert_eq!(child["tools"], parent["tools"], "the goal keeps the chat's tool families");
    for member in ["tasks", "depth"] {
        assert_eq!(
            child["delegation"][member].as_u64().expect("goal delegation"),
            parent["delegation"][member].as_u64().expect("chat delegation") - 1,
            "the goal takes one delegation {member}"
        );
    }
    assert_eq!(parent["spend"], CHAT_SPEND, "the chat's original purse");
    assert_eq!(
        child["spend"],
        (CHAT_SPEND - RUN_SPEND) / 2,
        "the goal gets half left after the chat's first run reserve"
    );
    let producer = document(ran, PRODUCED);
    assert_eq!(producer["chip"]["task"], PRODUCER, "the producer's task number: {producer}");
    let ending = &producer["chip"]["details"]["owner"]["details"]["ending"];
    assert!(ending["done"]["change"].is_object(), "the producer that delivered, then finished, is done: {producer}");
    let chat = document(ran, CHAT_ENDED);
    assert_eq!(chat["task"]["chip"]["task"], CHAT, "T1's ended task number");
    assert_eq!(chat["task"]["result"]["value"]["report"]["bytes"], CHAT_REPORT, "T1's report from the retained watch");
}
