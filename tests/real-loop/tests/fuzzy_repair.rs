//! The first run's change fails CI at its first head for what it holds; the engine's
//! repair writes the CSV header on its branch, CI passes there, and it lands.
//! The harness names its one fake repository acme/app; it stands as the first
//! run's playground/widget, with widget's src/main.rs in its tree.
use serde_json::{Value, json};
use skein_fake_forge_domain::{Observation, api, object_bytes};
use skein_fake_llm_domain::api::{Finish, Line, Part, Query, Role, Script, Turn};
use std::{fmt::Write, net::SocketAddr};
use temper_real_loop_world::{
    Exit, Ran, argv,
    calls::{self, D1, number},
    forge, landing, run_forge,
};

// Fresh deployment numbers: the coder, the repair and the reviewer are T2's delegates, in order.
const CHAT: u64 = 1;
const CHANGE: u64 = 2;
const REPAIR: u64 = 4;
const REQUEST: &str = "@repair_chat Add CSV export to widget";
const CHANGE_SPEC: &str = "@repair_coding Export widget's table as CSV";
const CHAT_CUE: &str = "@repair_chat";
const CODING_CUE: &str = "@repair_coding";
// The engine's own task words for a CI repair (forge_route.rs); first in the repair's system text.
const REPAIR_CUE: &str = "Repair the failed CI on the change branch";
const REVIEW_CUE: &str = "Review this change at the named head";
const CHAT_DELEGATED: &str = "The change is under way.";
// Fits the chat template's 32-byte report contract.
const CHAT_REPORT: &str = "The CSV export landed.";
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

const ROWS: &str = "        println!(\"{name} | {value}\");";
const CSV_ROWS: &str = "        println!(\"{name},{value}\");";
const HEADER: &str = "    println!(\"Name | Value\");";
const FIRST_SOURCE: &str = r#"fn main() {
    println!("Name | Value");
    for (name, value) in [("alpha", 1), ("beta", 2)] {
        println!("{name},{value}");
    }
}
"#;

fn fixture() -> forge::Fixture {
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
            // Coder and repair leave four units for the review; this story
            // retains its one-unit admission floor through its own defaults.
            value["world_settings"]["minimum_run_spend"] = 1.into();
            value["engine"]["limits"]["overrides"]["tasks.result_bytes"] = 512.into();
            value["engine"]["limits"]["overrides"]["documents.text_bytes"] = 512.into();
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
fn report(words: &str) -> Turn {
    call(b"finish", &json!({"form":"report","text":words,"fields":{}}))
}

fn scripts(forge: SocketAddr) -> Box<[Script]> {
    let grant = |kind| json!({"connector":1,"kind":kind,"segments":["forge",forge.to_string()],"terminal":"open","last":"acme"});
    let grants = json!([grant(2), grant(3), grant(4), grant(10), grant(11), grant(12)]);
    let change = json!({"batch":[{
        "executor":{"kind":"procedure","connector":1,"code":2},
        "spec":{"words":CHANGE_SPEC,"parameters":[
            {"name":1,"kind":"resource","connector":1,"resource":temper_engine_domain_forge::tag(1, forge::ID)},
            {"name":2,"kind":"bytes","value":"main"}]},
        "contract":{"kind":"change","connector":1,"change_kind":1,"words":512},
        "authority":{"tools":1,"grants":grants,
            "delegation":{"kinds":[{"kind":"agent","number":1},{"kind":"agent","number":4}],"tasks":4,"depth":2},
            "budget":{"spend":10},"notes":0}
    }]});
    Box::new([
        Script {
            cue: CHAT_CUE.as_bytes().into(),
            turns: Box::new([call(b"delegate", &change), wait(CHAT_DELEGATED), report(CHAT_REPORT)]),
        },
        Script {
            cue: CODING_CUE.as_bytes().into(),
            turns: Box::new([
                call(b"read", &json!({"path":"src/main.rs"})),
                call(b"edit", &json!({"path":"src/main.rs","edits":[{"old":ROWS,"new":CSV_ROWS}]})),
                call(
                    b"finish",
                    &json!({"form":"change","fields":{
                    "title":"Export widget's table as CSV","body":"Write widget's rows as CSV."}}),
                ),
            ]),
        },
        Script {
            cue: REPAIR_CUE.as_bytes().into(),
            turns: Box::new([
                call(b"read", &json!({"path":"src/main.rs"})),
                call(b"edit", &json!({"path":"src/main.rs","edits":[{"old":HEADER,"new":CSV_LINE}]})),
                call(
                    b"finish",
                    &json!({"form":"change","fields":{
                    "title":"Write the CSV header","body":"CI wants the CSV header line."}}),
                ),
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

// The change branch's heads: the coder's, then the repair's.
const HEADS: usize = 2;
const ONE_MERGE: usize = 1;

// Recorded call positions.
const SIGNED_IN: usize = 0;
const ADOPTED: usize = 1;
const CHAT_STARTED: usize = 2;
const CHAT_ENDED: usize = 3;
const CHANGE_READ: usize = 4;
const REPAIR_READ: usize = 5;

fn acts() -> Vec<calls::Step> {
    use calls::{call, sign_in};
    vec![
        sign_in(D1),
        call(
            D1,
            argv![
                "repository",
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
        call(D1, argv!["chat", "start", "--project", "1", "--key", "repair", REQUEST]),
        call(D1, argv!["task", "show", number(CHAT), "--until", "ended", "--json"]),
        call(D1, argv!["task", "show", number(CHANGE), "--json"]),
        call(D1, argv!["task", "show", number(REPAIR), "--json"]),
    ]
}

fn document(ran: &Ran, position: usize) -> Value {
    serde_json::from_slice(&ran.calls()[position].stdout).expect("ctl JSON document")
}

fn own_cue(query: &Query) -> Option<&'static str> {
    let system = String::from_utf8_lossy(&query.system);
    [CHAT_CUE, CODING_CUE, REPAIR_CUE, REVIEW_CUE]
        .into_iter()
        .filter_map(|cue| system.find(cue).map(|at| (at, cue)))
        .min()
        .map(|(_, cue)| cue)
}

fn query_at<'a>(ran: &'a Ran, cue: &str, replies: usize) -> &'a Query {
    ran.llm()
        .iter()
        .find_map(|seen| {
            let skein_fake_peers::llm::Observation::Query { query, .. } = seen else { return None };
            let first = own_cue(query);
            let said = query.messages.iter().filter(|message| message.role == Role::Assistant).count();
            (first == Some(cue) && said == replies).then_some(query)
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
            if output.as_ref() == words.as_bytes())
    })
}

fn check_ci(ran: &Ran) -> (u64, u64) {
    let records = ran.forge().observations();
    let moves: Vec<_> = records
        .iter()
        .enumerate()
        .filter_map(|(position, record)| {
            let Observation::Moved { repository, branch, from, to, .. } = record else { return None };
            (repository.as_ref() == b"acme/app" && branch.as_ref() != b"main").then_some((
                position,
                branch.as_ref(),
                *from,
                *to,
            ))
        })
        .collect();
    assert_eq!(moves.len(), HEADS, "the change branch moved exactly twice: {moves:?}");
    let (first_move, branch, from, h1) = moves[0];
    let (repair_move, repaired_branch, previous, h2) = moves[1];
    assert_eq!(branch, repaired_branch, "both heads belong to the one change branch");
    assert_eq!(from, None, "the change branch was created at H1");
    assert_eq!(previous, Some(h1), "the repair moved the change branch from H1 to H2");
    assert_ne!(h1, h2, "the repair delivered a new head");
    let checks: Vec<_> = records
        .iter()
        .enumerate()
        .filter_map(|(position, record)| {
            let Observation::Reported { repository, commit, context, state, .. } = record else { return None };
            (repository.as_ref() == b"acme/app" && context.as_ref() == b"check" && [h1, h2].contains(commit))
                .then_some((position, *commit, *state))
        })
        .collect();
    let sequence: Vec<_> = checks.iter().map(|(_, commit, state)| (*commit, *state)).collect();
    assert_eq!(
        sequence,
        vec![(h1, api::Check::Pending), (h1, api::Check::Failed), (h2, api::Check::Pending), (h2, api::Check::Passed)],
        "CI failed at H1, then passed at H2, in exactly that order"
    );
    assert!(first_move < checks[0].0, "H1 was pushed before its CI began");
    assert!(checks[1].0 < repair_move, "H1 failed before the repair pushed H2");
    assert!(repair_move < checks[2].0, "H2 was pushed before its CI began");
    let merges: Vec<_> = records
        .iter()
        .enumerate()
        .filter_map(|(position, record)| {
            let Observation::Merged { repository, base, head, commit, .. } = record else { return None };
            Some((position, repository.as_ref(), base.as_ref(), *head, *commit))
        })
        .collect();
    assert_eq!(merges.len(), ONE_MERGE, "exactly one change merged: {merges:?}");
    let (merged_at, repository, base, head, commit) = merges[0];
    assert_eq!(repository, b"acme/app", "the merge belongs to widget's repository");
    assert_eq!(base, b"main", "the change landed on main");
    assert_eq!(head, h2, "the merge used exactly the repaired head H2");
    assert!(checks[3].0 < merged_at, "H2 passed CI before merging");
    // Three heads retain CI now (forge.rs's statuses sizing for 6.4), so
    // main's merge commit also reports its own Pending then Passed.
    let main_checks: Vec<_> = records
        .iter()
        .enumerate()
        .filter_map(|(position, record)| {
            let Observation::Reported { repository, commit: checked, context, state, .. } = record else {
                return None;
            };
            (repository.as_ref() == b"acme/app" && *checked == commit && context.as_ref() == b"check")
                .then_some((position, *state))
        })
        .collect();
    assert!(
        matches!(main_checks.as_slice(), [(pending, api::Check::Pending), (passed, api::Check::Passed)]
            if merged_at < *pending && pending < passed),
        "main's merge commit passed CI after the merge: {main_checks:?}"
    );
    (h1, commit)
}

fn check_repair(ran: &Ran, h1: u64) {
    let repair = document(ran, REPAIR_READ);
    assert_eq!(repair["chip"]["task"], REPAIR, "the repair is T4");
    assert_eq!(
        repair["spec"]["specification"]["words"]["bytes"], REPAIR_CUE,
        "T4's specification is the engine's CI repair request"
    );
    assert_eq!(repair["requester"]["task"], CHANGE, "the change requested the repair");
    let opened = ran
        .llm()
        .iter()
        .filter(|seen| {
            let skein_fake_peers::llm::Observation::Query { query, .. } = seen else { return false };
            own_cue(query) == Some(REPAIR_CUE) && !query.messages.iter().any(|message| message.role == Role::Assistant)
        })
        .count();
    assert_eq!(opened, 1, "exactly one repair conversation opened on the engine's task words");
    let query = query_at(ran, REPAIR_CUE, 0);
    let system = std::str::from_utf8(&query.system).expect("repair system text");
    let ci = system
        .split_once("## CI\n\n")
        .expect("the repair's CI heading")
        .1
        .split_once("\n## ")
        .expect("the section following CI")
        .0;
    let config: Value =
        serde_json::from_slice(&std::fs::read(ran.path().join("temper.json")).expect("the story's configuration"))
            .expect("story configuration JSON");
    let address =
        config["application"]["forges"][0]["api"]["socket"].as_str().expect("the fake forge's assigned address");
    let expected = format!(
        "Repair the failed check at change head {}. Push a new head and wait for checks there.\n\
         Failed check: check\nDescription: CI failed\n\
         Link: http://{address}/acme/app/actions/runs/1/jobs/0\n\
         Failed step: 1 Run check\nEnd of its log:\n\
         failing check check 0\nfailing check check 1\nfailing check check 2\nfailing check check 3\n\n\
         Repair the failed check.\n",
        hex(&object_bytes(h1)[..8])
    );
    assert_eq!(ci, expected, "the repair was told H1, the failed check, link, step and whole log, with no cut marker");
    // Smith's read rendering numbers each line and ends with its window summary.
    let mut read = String::new();
    for (index, line) in FIRST_SOURCE.lines().enumerate() {
        writeln!(read, "{}: {line}", index + 1).expect("numbered source line");
    }
    let lines = FIRST_SOURCE.lines().count();
    write!(read, "({lines} of {lines} lines; skipped 0) [end of file]").expect("read window summary");
    assert!(
        heard_tool(query_at(ran, REPAIR_CUE, 1), &read),
        "the repair read exactly H1's numbered source with the old header and CSV rows"
    );
}

fn hex(bytes: &[u8]) -> String {
    let mut words = String::new();
    for byte in bytes {
        write!(words, "{byte:02x}").expect("hexadecimal commit");
    }
    words
}

fn check_landing(ran: &Ran, commit: u64) {
    assert!(ran.forge().merged_green(), "the change merged with CI green at its head");
    assert_eq!(
        ran.forge().main_file(b"src/main.rs"),
        CSV_SOURCE.as_bytes(),
        "widget's CSV header and rows landed on main"
    );
    let change = document(ran, CHANGE_READ);
    let merge_hex = hex(&object_bytes(commit));
    assert_eq!(change["chip"]["task"], CHANGE, "the change's task number");
    assert_eq!(change["result"]["value"]["change"]["kind"], 1, "the landed result is a change of kind 1");
    assert_eq!(
        change["result"]["value"]["change"]["words"]["bytes"].as_str().expect("landing words").lines().next(),
        Some(merge_hex.as_str()),
        "the change's first result line names the merge commit in full lowercase hex"
    );
    assert_eq!(change["result"]["value"]["change"]["words"]["cut"], false, "the result document is whole");
    let words = change["result"]["value"]["change"]["words"]["bytes"].as_str().expect("landing words");
    let head = ran
        .forge()
        .observations()
        .iter()
        .find_map(|seen| {
            let Observation::Merged { head, .. } = seen else { return None };
            Some(*head)
        })
        .expect("merged head");
    let head = hex(&object_bytes(head));
    let lines: Vec<_> = words.lines().collect();
    assert_eq!(lines.len(), 5, "{words}");
    assert_eq!(lines[2], format!("checks at {head}: check passed"));
    assert_eq!(lines[3], "repairs: 1: CI failed: check");
    assert_eq!(lines[4], format!("reviews: approved at {head}"));
    assert!(told(query_at(ran, CHAT_CUE, 2)).contains(words), "the requester heard the whole result");
    let result = format!("Result from task {CHANGE}:\n{merge_hex}");
    assert!(told(query_at(ran, CHAT_CUE, 2)).contains(&result), "T1 heard T2's merge commit before reporting");
    let chat = document(ran, CHAT_ENDED);
    assert_eq!(chat["task"]["chip"]["task"], CHAT, "the ended chat's task number");
    assert_eq!(
        chat["task"]["result"]["value"]["report"]["bytes"], CHAT_REPORT,
        "T1 reported that the CSV export landed"
    );
}

#[test]
fn a_change_failing_ci_is_repaired_and_lands() {
    let ran = run_forge(fixture(), scripts, acts);
    let calls = [SIGNED_IN, ADOPTED, CHAT_STARTED, CHAT_ENDED, CHANGE_READ, REPAIR_READ];
    assert_eq!(ran.calls().len(), calls.len(), "all six prescribed ctl calls ran");
    for position in calls {
        assert_eq!(ran.calls()[position].exit, Some(Exit::Code(0)), "ctl call {position}: {:?}", ran.calls()[position]);
    }
    assert_eq!(ran.number(CHAT_STARTED), CHAT, "the fresh chat's task number");
    let (h1, commit) = check_ci(&ran);
    check_repair(&ran, h1);
    check_landing(&ran, commit);
}
