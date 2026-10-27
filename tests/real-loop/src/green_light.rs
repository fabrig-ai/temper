//! B0's first passing walk, replayed on the fake peers (domain/testing.md, 2.4).
//! Until temper-playground exists, its story documents live in tests/fixtures/b0.
//! Regenerate the scripts, including the fresh fake commit identities, from the workspace root:
//! `sh tests/real-loop/tests/fixtures/b0/regenerate.sh [DEPLOYMENT]`.
//! The command runs tools/twin over walk 1's five model tasks; opaque reasoning is omitted.
use crate::{
    Exit, ForgeRecipe, Ran, argv,
    calls::{self, D1, number},
    forge, landing, llm,
};
use serde_json::Value;
use skein_fake_llm_domain::api::{Part, Query, Role};
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::LazyLock,
};

pub const TASKS: [u64; 5] = [1, 2, 4, 5, 6];
pub const CUES: [&str; 5] = [
    "[Requested by person 1]",
    "[Requested by task 1]",
    "Implement the approved CSV export for widget.",
    "Review this change at the named head",
    "This review is asked again at the same head",
];
const CHAT_CUE: &str = CUES[0];
const WORK_CUE: &str = CUES[1];
const CHAT: u64 = 1;
const WORK: u64 = 2;
const CHANGE: u64 = 3;
const PRODUCER: u64 = 4;
const QUESTION: u64 = 1;

fn person(member: &str) -> &'static str {
    static PERSON: LazyLock<Value> = LazyLock::new(|| {
        serde_json::from_str(include_str!("../tests/fixtures/b0/person.json")).expect("B0 person's words")
    });
    PERSON[member].as_str().expect("person's story member")
}

#[must_use]
pub fn fixture() -> forge::Fixture {
    forge::Fixture {
        setup: || {
            let document = temper_fake::forge::Document::decode(include_bytes!("../tests/fixtures/b0/forge.json"))
                .expect("B0 forge document");
            let repository = &document.repositories()[0];
            let mut setup = (landing::fixture().setup)();
            setup.name =
                format!("{}/{}", String::from_utf8_lossy(&repository.owner), String::from_utf8_lossy(&repository.name))
                    .into_bytes()
                    .into();
            setup.tree = repository.tree.clone().expect("walk's seeded tree");
            // The walk's starter CI echoes success; it does not compile widget.
            setup.checks = temper_fake::forge::checks(repository.ci.as_ref());
            setup
        },
        configure: |value, agent, forge| {
            (landing::fixture().configure)(value, agent, forge);
            crate::fixtures::own_policy(value);
            // The codec's result-field envelope grows with the live review's failure text.
            value["engine"]["memory_bytes"] = 1_200_000_000_000_000_000_u64.into();
            crate::fixtures::requester(value);
            value["world_settings"]["maximum_run_spend"] = 32.into();
            value["world_settings"]["cap"] = 1000.into();
            for project in value["world_settings"]["policies"].as_array_mut().expect("projects") {
                let policy = &mut project["policy"];
                policy["period_spend"] = 1000.into();
                policy["roles"][0]["period_spend"] = 1000.into();
                let families = jig_domain_core::ToolFamilies::JIG;
                policy["ceiling"]["tools"] = (families.delegate.0 | families.message.0 | families.read.0).into();
                policy["ceiling"]["delegation"]["depth"] = 5.into();
                policy["ceiling"]["budget"]["spend"] = 1000.into();
                for grant in policy["ceiling"]["grants"].as_array_mut().expect("grants") {
                    grant["pattern"]["value"] = "playground".into();
                }
                policy["roles"][0]["authority"] = policy["ceiling"].clone();
                for grant in policy["projections"].as_array_mut().expect("projection grants") {
                    grant["pattern"]["value"] = "playground".into();
                }
            }
            for rule in value["application"]["landing"]["projects"][0]["rules"].as_array_mut().expect("landing rules") {
                rule["pattern"]["segments"][2] = "playground".into();
                rule["pattern"]["segments"][3] = "widget".into();
            }
            for (name, size) in [
                ("core.policy_bytes", 5120),
                ("tasks.inbox_bytes", 16384),
                ("tasks.spec_bytes", 4096),
                ("tasks.result_bytes", 512),
                ("documents.text_bytes", 2048),
                ("documents.lines", 32),
                ("brief.sections", 8),
            ] {
                value["engine"]["limits"]["overrides"][name] = size.into();
            }
            // Keep the project's seeded policy in typed world defaults, as the reference stories do.
            for member in ["charters", "templates", "policies", "rules"] {
                value["engine"].as_object_mut().expect("engine").remove(member);
            }
            value["world_settings"]["case"] = true.into();
            value["world_settings"]["report_words"] = 32.into();
            landing::admit(value);
        },
        ..forge::fixture()
    }
}

#[must_use]
pub fn recipe(_forge: SocketAddr, _source: &Path) -> llm::Recipe {
    from_document(include_bytes!("../tests/fixtures/b0/llm.json"))
}
#[must_use]
pub fn from_document(bytes: &[u8]) -> llm::Recipe {
    let document = temper_fake::llm::Document::decode(bytes).expect("B0 LLM document");
    let mut recipe = llm::scripted(Box::new([]));
    recipe.domain = document.domain(17);
    recipe.domain_limits = temper_fake::llm::LIMITS;
    recipe
}
#[must_use]
pub fn run() -> Ran {
    crate::run_forge_with(fixture(), ForgeRecipe { source: PathBuf::new(), build: recipe }, acts)
}

const CHAT_STARTED: usize = 2;
const TREE: usize = 6;
const ASKED: usize = 7;
const ACTIVITY: usize = 8;
const ANSWERED: usize = 9;
const LANDED: usize = 11;
const WORK_ENDED: usize = 10;
const PRODUCED: usize = 12;
const FIRST_REVIEW: usize = 13;
const RETRY_REVIEW: usize = 14;
const LAST_WORD: usize = 16;
const INBOX_EMPTY: usize = 19;

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
        call(D1, argv!["chat", "start", "--project", "1", "--key", "green", person("request")]),
        call(D1, argv!["inbox", "--until", "waiting"]),
        call(D1, argv!["task", "show", number(CHAT), "--until", "phase:idle"]),
        call(D1, argv!["chat", "show", number(CHAT), "--json"]),
        call(D1, argv!["task", "show", number(CHAT), "--tree", "--json"]),
        call(D1, argv!["inbox", "--kind", "questions", "--json"]),
        call(D1, argv!["task", "show", number(CHAT), "--activity", "32", "--json"]),
        call(D1, argv!["task", "answer", number(CHAT), number(QUESTION), person("answer"), "--key", "green-answer"]),
        // The goal's end joins the landing; its change exists before reading its panel.
        call(D1, argv!["task", "show", number(WORK), "--until", "ended", "--json"]),
        call(D1, argv!["change", "show", number(CHANGE), "--until", "landed", "--json"]),
        call(D1, argv!["task", "show", number(PRODUCER), "--until", "ended", "--json"]),
        call(D1, argv!["task", "show", "5", "--json"]),
        call(D1, argv!["task", "show", "6", "--until", "ended", "--json"]),
        call(D1, argv!["task", "show", number(CHAT), "--until", "phase:idle"]),
        call(D1, argv!["chat", "show", number(CHAT), "--json"]),
        call(D1, argv!["chat", "close", number(CHAT), "--key", "close-green"]),
        call(D1, argv!["task", "show", number(CHAT), "--until", "ended"]),
        call(D1, argv!["inbox", "--kind", "questions", "--json"]),
    ]
}

fn document(ran: &Ran, position: usize) -> Value {
    let value: Value = serde_json::from_slice(&ran.calls()[position].stdout).expect("ctl JSON document");
    if value.get("met").is_some() { value["task"].clone() } else { value }
}

fn query_at<'a>(ran: &'a Ran, cue: &str, replies: usize) -> &'a Query {
    ran.llm()
        .iter()
        .find_map(|seen| {
            let skein_fake_peers::llm::Observation::Query { query, .. } = seen else { return None };
            // Delegated specifications can mention other cues; the task's own is first.
            let system = String::from_utf8_lossy(&query.system);
            let first = CUES.into_iter().filter_map(|candidate| system.find(candidate).map(|at| (at, candidate))).min();
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

pub fn check(ran: &Ran) {
    assert_eq!(ran.calls().len(), 20);
    for (position, call) in ran.calls().iter().enumerate() {
        assert_eq!(call.exit, Some(Exit::Code(0)), "ctl call {position}: {call:?}");
    }
    assert_eq!(ran.number(CHAT_STARTED), CHAT);
    assert!(
        String::from_utf8_lossy(&ran.calls()[5].stdout).contains("Approve this --csv plan?"),
        "chat acknowledged the waiting question"
    );
    assert!(ran.calls().iter().all(|call| call.tries == 1), "each read is issued once; waits use native watches");
    let inbox = document(ran, ASKED);
    let rows = inbox["needs"]["rows"].as_array().expect("questions");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["task"]["live"]["task"], CHAT);
    assert_eq!(rows[0]["object"]["question"]["message"], QUESTION);
    let tree = document(ran, TREE);
    let rows = tree["tree"]["rows"].as_array().expect("waiting tree");
    assert_eq!(rows.len(), 1, "no change before the answer: {tree}");
    assert_eq!(rows[0]["chip"]["task"], WORK);
    assert_eq!(tree["tree"]["more"], false);
    assert!(tree["tree"]["folded"].as_array().expect("folded tasks").is_empty(), "no hidden change before approval");
    assert!(String::from_utf8_lossy(&ran.calls()[ACTIVITY].stdout).contains(person("question")));
    let relay = told(query_at(ran, CHAT_CUE, 3));
    let question_at = relay.find(person("question")).expect("chat hears the plan");
    let answer_at = relay.find(person("answer")).expect("chat hears the answer");
    assert!(question_at < answer_at);
    assert!(heard_tool(query_at(ran, WORK_CUE, 4), "Message 1 sent"));
    assert!(told(query_at(ran, WORK_CUE, 5)).contains(person("answer")), "work hears approval before change");
    assert!(heard_tool(query_at(ran, WORK_CUE, 6), "Delegated tasks: 3"));
    assert!(ran.calls()[ANSWERED].ended <= ran.calls()[LANDED].ended);
    let seed = &ran.forge().snapshots()[0];
    assert_eq!(seed.branches.len(), 1);
    assert_eq!(seed.branches[0].0.as_ref(), b"main");
    for snapshot in ran.forge().snapshots().iter().filter(|snapshot| snapshot.at <= ran.calls()[ANSWERED].ended) {
        assert_eq!(snapshot.branches, seed.branches, "no branch pushed before approval");
        assert!(snapshot.pulls.is_empty(), "no pull before approval");
    }
    let reviewed = query_at(ran, CUES[4], 7);
    assert!(heard_tool(reviewed, "fn write_table"), "the retry actually read the produced source: {}", told(reviewed));
    assert_eq!(told(reviewed).matches("Error:").count(), 1, "only the walk's padded lookup fails");
    assert!(ran.forge().merged_green(), "CI passed at the merged head");
    let delivered = told(query_at(ran, WORK_CUE, 7));
    let merge = delivered
        .split_once("Result from task 3:")
        .expect("change's landing result")
        .1
        .split_whitespace()
        .next()
        .expect("merge commit");
    let work = document(ran, WORK_ENDED);
    let report = work["result"]["value"]["report"]["bytes"].as_str().expect("goal's report");
    assert!(report.contains(&merge[..7]), "goal must name the fresh merge {merge}: {report}");
    let source = String::from_utf8_lossy(ran.forge().main_file(b"src/main.rs"));
    assert!(source.contains("writeln!(output, \"fruit,count\")?;"), "CSV header landed: {source}");
    let scripts: Value =
        serde_json::from_slice(include_bytes!("../tests/fixtures/b0/llm.json")).expect("recorded scripts");
    let expected =
        scripts["cued"][2]["turns"][3]["lines"][0]["arguments"]["content"].as_str().expect("recorded main write");
    assert_eq!(source, expected, "the walk's whole program landed");
    assert_eq!(document(ran, WORK_ENDED)["chip"]["phase"], "ended");
    assert_eq!(document(ran, PRODUCED)["chip"]["phase"], "ended");
    assert!(document(ran, FIRST_REVIEW).to_string().contains("cannot complete the mandated review"));
    assert!(document(ran, RETRY_REVIEW).to_string().contains("Approve."));
    assert!(String::from_utf8_lossy(&ran.calls()[LAST_WORD].stdout).contains(person("last_word")));
    assert!(document(ran, INBOX_EMPTY)["needs"]["rows"].as_array().expect("last questions").is_empty());
}
