//! B1 walk 5's recorded agents on the fake peers (domain/testing.md, 2.4).
//! Regenerate from the workspace root with tests/real-loop/tests/fixtures/b1/regenerate.sh.
use crate::{
    Exit, ForgeRecipe, Ran, argv,
    calls::{self, D1, number},
    forge, green_light, llm,
};
use serde_json::Value;
use skein_fake_forge_domain::{Observation, api, object_bytes};
use skein_fake_llm_domain::api::{Part, Query, Role};
use std::{
    fmt::Write,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::LazyLock,
};

pub const TASKS: [u64; 6] = [1, 2, 3, 5, 6, 7];
pub const CUES: [&str; 6] = [
    "[Requested by person 1]",
    "[Requested by task 1]",
    "You are an advisor: you read what your brief asks and report what you found; you change nothing and ask no one.",
    "Implement CSV export for the existing hard-coded TABLE data in src/main.rs",
    "Repair the failed CI on the change branch",
    "Review this change at the named head",
];
const CHAT: u64 = 1;
const WORK: u64 = 2;
const ADVISOR: u64 = 3;
const CHANGE: u64 = 4;
const PRODUCER: u64 = 5;
const REPAIR: u64 = 6;
const REVIEW: u64 = 7;
const QUESTION: u64 = 2;
const PULL: u64 = 2;
const CHECK: &[u8] = b"/ check (pull_request)";
const CHANGES: &[u8] = b"/ changes (pull_request)";
const REPOSITORY: &[u8] = b"playground/widget";

fn person(member: &str) -> &'static str {
    static WORDS: LazyLock<Value> = LazyLock::new(|| {
        serde_json::from_str(include_str!("../tests/fixtures/b1/person.json")).expect("B1 person's words")
    });
    WORDS[member].as_str().expect("person's words")
}

#[must_use]
pub fn fixture() -> forge::Fixture {
    forge::Fixture {
        setup: || {
            let document = temper_fake::forge::Document::decode(include_bytes!("../tests/fixtures/b1/forge.json"))
                .expect("B1 forge document");
            let repository = &document.repositories()[0];
            let mut setup = (green_light::fixture().setup)();
            setup.tree = repository.tree.clone().expect("widget with the person's gate");
            setup.checks = temper_fake::forge::checks(repository.ci.as_ref());
            setup.checks.contexts = Box::new([CHECK.into(), CHANGES.into()]);
            setup.protection.as_mut().expect("main protection").contexts.clone_from(&setup.checks.contexts);
            setup
        },
        configure: green_light::fixture().configure,
        ..forge::fixture()
    }
}

#[must_use]
pub fn recipe(forge: SocketAddr, _source: &Path) -> llm::Recipe {
    // This marker was introduced by twin in calls' arguments alone. Recorded
    // text, including the walk's forge address and merge, remains untouched.
    let document = include_str!("../tests/fixtures/b1/llm.json").replace("http://B1-FORGE", &format!("http://{forge}"));
    let mut recipe = green_light::from_document(document.as_bytes());
    // The walk has 48 completions. The peer retains six wire observations
    // per call, already beyond the shared 256-record smoke-test allowance.
    recipe.limits.observations = 512;
    recipe
}

#[must_use]
pub fn run() -> Ran {
    crate::run_forge_with(fixture(), ForgeRecipe { source: PathBuf::new(), build: recipe }, acts)
}

const TREE: usize = 6;
const ASKED: usize = 7;
const ACTIVITY: usize = 8;
const ANSWERED: usize = 9;
const WORK_ENDED: usize = 10;
const LANDED: usize = 11;
const ADVISOR_ENDED: usize = 12;
const PRODUCED: usize = 13;
const REPAIRED: usize = 14;
const REVIEWED: usize = 15;
const LAST_WORD: usize = 17;
const INBOX_EMPTY: usize = 20;

#[must_use]
pub fn acts() -> Vec<calls::Step> {
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
                "playground",
                "--name",
                "widget",
                "--landing",
                "main",
                "--key",
                "adopt-widget"
            ],
        ),
        call(D1, argv!["chat", "start", "--project", "1", "--key", "repair", person("request")]),
        call(D1, argv!["inbox", "--until", "waiting"]),
        call(D1, argv!["task", "show", number(CHAT), "--until", "phase:idle"]),
        call(D1, argv!["chat", "show", number(CHAT), "--json"]),
        call(D1, argv!["task", "show", number(CHAT), "--tree", "--json"]),
        call(D1, argv!["inbox", "--kind", "questions", "--json"]),
        call(D1, argv!["task", "show", number(CHAT), "--activity", "32", "--json"]),
        call(D1, argv!["task", "answer", number(CHAT), number(QUESTION), person("answer"), "--key", "repair-answer"]),
        call(D1, argv!["task", "show", number(WORK), "--until", "ended", "--json"]),
        call(D1, argv!["change", "show", number(CHANGE), "--until", "landed", "--json"]),
        call(D1, argv!["task", "show", number(ADVISOR), "--until", "ended", "--json"]),
        call(D1, argv!["task", "show", number(PRODUCER), "--until", "ended", "--json"]),
        call(D1, argv!["task", "show", number(REPAIR), "--until", "ended", "--json"]),
        call(D1, argv!["task", "show", number(REVIEW), "--until", "ended", "--json"]),
        call(D1, argv!["task", "show", number(CHAT), "--until", "phase:idle"]),
        call(D1, argv!["chat", "show", number(CHAT), "--json"]),
        call(D1, argv!["chat", "close", number(CHAT), "--key", "close-repair"]),
        call(D1, argv!["task", "show", number(CHAT), "--until", "ended"]),
        call(D1, argv!["inbox", "--kind", "questions", "--json"]),
    ]
}

fn document(ran: &Ran, position: usize) -> Value {
    let value: Value = serde_json::from_slice(&ran.calls()[position].stdout).expect("ctl JSON");
    if value.get("met").is_some() { value["task"].clone() } else { value }
}

fn own_cue(query: &Query) -> Option<&'static str> {
    let system = String::from_utf8_lossy(&query.system);
    CUES.into_iter().filter_map(|cue| system.find(cue).map(|at| (at, cue))).min().map(|(_, cue)| cue)
}

fn query_at(ran: &Ran, script: usize, replies: usize) -> &Query {
    ran.llm()
        .iter()
        .find_map(|seen| {
            let skein_fake_peers::llm::Observation::Query { query, .. } = seen else { return None };
            let said = query.messages.iter().filter(|message| message.role == Role::Assistant).count();
            (own_cue(query) == Some(CUES[script]) && said == replies).then_some(query)
        })
        .expect("recorded task query")
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
    String::from_utf8(words).expect("received text")
}

fn hex(id: u64) -> String {
    let mut words = String::new();
    for byte in object_bytes(id) {
        write!(words, "{byte:02x}").expect("hexadecimal commit");
    }
    words
}

pub fn check(ran: &Ran) {
    assert_eq!(ran.calls().len(), 21);
    for (position, call) in ran.calls().iter().enumerate() {
        assert_eq!(call.exit, Some(Exit::Code(0)), "ctl call {position}: {call:?}");
        assert_eq!(call.tries, 1, "native watches, no repeated reads");
    }
    check_waiting(ran);
    let (first, repaired, merge) = check_ci(ran);
    check_repair(ran, first);
    check_landing(ran, repaired, merge);
}

fn check_waiting(ran: &Ran) {
    assert_eq!(ran.number(2), CHAT);
    let inbox = document(ran, ASKED);
    let rows = inbox["needs"]["rows"].as_array().expect("questions");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["task"]["live"]["task"], CHAT);
    assert_eq!(rows[0]["object"]["question"]["message"], QUESTION);
    assert!(String::from_utf8_lossy(&ran.calls()[ACTIVITY].stdout).contains(person("question")));
    let tree = document(ran, TREE);
    assert!(!tree.to_string().contains("\"task\":4"), "no change before approval: {tree}");
    assert_eq!(tree["tree"]["more"], false);
    assert!(tree["tree"]["folded"].as_array().expect("folded tasks").is_empty());
    let relay = told(query_at(ran, 0, 3));
    assert!(relay.find(person("question")).expect("question") < relay.find(person("answer")).expect("answer"));
    assert!(told(query_at(ran, 1, 6)).contains(person("answer")), "approval before change call");
    let seed = &ran.forge().snapshots()[0];
    assert_eq!(seed.branches.len(), 1);
    assert_eq!(seed.branches[0].0.as_ref(), b"main");
    for snapshot in ran.forge().snapshots().iter().filter(|snapshot| snapshot.at <= ran.calls()[ANSWERED].ended) {
        assert_eq!(snapshot.branches, seed.branches, "nothing pushed before approval");
        assert!(snapshot.pulls.is_empty(), "no pull before approval");
    }
}

fn check_ci(ran: &Ran) -> (u64, u64, u64) {
    let records = ran.forge().observations();
    let moves: Vec<_> = records
        .iter()
        .filter_map(|record| {
            let Observation::Moved { repository, branch, from, to, .. } = record else { return None };
            (repository.as_ref() == REPOSITORY && branch.as_ref() != b"main").then_some((branch, *from, *to))
        })
        .collect();
    assert_eq!(moves.len(), 2, "producer and one repair");
    let (branch, from, first) = moves[0];
    let (repaired_branch, previous, repaired) = moves[1];
    assert_eq!(branch, repaired_branch);
    assert_eq!(from, None);
    assert_eq!(previous, Some(first));
    assert_ne!(first, repaired);
    for (head, context, expected) in [
        (first, CHECK, api::Check::Passed),
        (first, CHANGES, api::Check::Failed),
        (repaired, CHECK, api::Check::Passed),
        (repaired, CHANGES, api::Check::Passed),
    ] {
        let sequence: Vec<_> = records
            .iter()
            .filter_map(|record| {
                let Observation::Reported { repository, commit, context: name, state, .. } = record else {
                    return None;
                };
                (repository.as_ref() == REPOSITORY && *commit == head && name.as_ref() == context).then_some(*state)
            })
            .collect();
        assert_eq!(
            sequence,
            [api::Check::Pending, expected],
            "head {} context {}",
            hex(head),
            String::from_utf8_lossy(context)
        );
    }
    let pulls: Vec<_> = records
        .iter()
        .filter_map(|record| {
            let Observation::Opened { number, branches: Some(_), .. } = record else { return None };
            Some(*number)
        })
        .collect();
    assert_eq!(pulls, [PULL], "one pull through red, repair and green");
    let merges: Vec<_> = records
        .iter()
        .filter_map(|record| {
            let Observation::Merged { repository, base, head, commit, .. } = record else { return None };
            Some((repository.as_ref(), base.as_ref(), *head, *commit))
        })
        .collect();
    assert_eq!(merges.len(), 1);
    let (repository, base, head, merge) = merges[0];
    assert_eq!(repository, REPOSITORY);
    assert_eq!(base, b"main");
    assert_eq!(head, repaired);
    assert!(ran.forge().merged_green());
    (first, repaired, merge)
}

fn check_repair(ran: &Ran, first: u64) {
    let repair = document(ran, REPAIRED);
    assert_eq!(repair["requester"]["task"], CHANGE);
    let opening = format!("Repair CI at {}: / changes (pull_request)\n{}", &hex(first)[..12], CUES[4]);
    assert_eq!(repair["spec"]["specification"]["words"]["bytes"], opening);
    let system = String::from_utf8_lossy(&query_at(ran, 4, 0).system);
    let ci = system.split_once("## CI\n\n").expect("CI brief").1.split_once("\n## ").expect("next section").0;
    assert!(ci.contains(&format!("Repair the failed check at change head {}", &hex(first)[..16])));
    assert!(ci.contains("Failed check: / changes (pull_request)\nDescription: CI failed\nLink: http://"));
    assert!(ci.contains("Failed step: 1 Run / changes (pull_request)\nEnd of its log:\n"));
    assert!(ci.contains("Missing changes/2.md"), "the failed job names the missing note: {ci}");
    assert!(!ci.contains("cut"), "whole failure log");
}

fn check_landing(ran: &Ran, repaired: u64, merge: u64) {
    let change = document(ran, LANDED);
    let result = &change["result"]["value"]["change"]["words"];
    assert_eq!(result["cut"], false);
    let words = result["bytes"].as_str().expect("landed evidence");
    let lines: Vec<_> = words.lines().collect();
    assert_eq!(lines.len(), 5);
    assert_eq!(lines[0], hex(merge));
    assert!(lines[1].ends_with("/playground/widget/pulls/2"));
    assert_eq!(
        lines[2],
        format!("checks at {}: / check (pull_request) passed; / changes (pull_request) passed", hex(repaired))
    );
    assert_eq!(lines[3], "repairs: 1: CI failed: / changes (pull_request)");
    assert_eq!(lines[4], format!("reviews: approved at {}", hex(repaired)));
    assert!(told(query_at(ran, 1, 8)).contains(words), "change's requester heard the whole result");
    let last = String::from_utf8_lossy(&ran.calls()[LAST_WORD].stdout);
    assert!(last.contains("CSV export landed"));
    assert!(last.contains("checks passed after one repair"));
    assert!(last.contains("fruit,count"));
    assert!(!last.contains("tests passed"), "no unseen test claimed");
    assert!(told(query_at(ran, 0, 5)).contains("checks passed after one repair"), "chat was told the checks evidence");
    let source = String::from_utf8_lossy(ran.forge().main_file(b"src/main.rs"));
    assert!(source.contains("String::from(\"fruit,count\\n\")"));
    let scripts: Value = serde_json::from_slice(include_bytes!("../tests/fixtures/b1/llm.json")).expect("walk scripts");
    let producer = &scripts["cued"][3]["turns"];
    let mut expected = producer[3]["lines"][0]["arguments"]["content"].as_str().expect("walk source").to_owned();
    for edit in producer[5]["lines"][0]["arguments"]["edits"].as_array().expect("walk edits") {
        let old = edit["old"].as_str().expect("old source");
        let new = edit["new"].as_str().expect("new source");
        assert_eq!(expected.matches(old).count(), 1);
        expected = expected.replacen(old, new, 1);
    }
    assert_eq!(source, expected, "the walk's whole produced source landed");
    assert_eq!(
        ran.forge().main_file(b"changes/2.md"),
        b"Repair CI by adding the required pull request changelog entry.\n"
    );
    for position in [WORK_ENDED, ADVISOR_ENDED, PRODUCED, REPAIRED, REVIEWED] {
        assert_eq!(document(ran, position)["chip"]["phase"], "ended");
    }
    assert!(document(ran, REVIEWED).to_string().contains("Approve."));
    assert!(document(ran, INBOX_EMPTY)["needs"]["rows"].as_array().expect("last questions").is_empty());
}
