//! Rung A's CSV landing path from walk 8, replayed on fake peers.
use crate::{
    Exit, ForgeRecipe, Ran, argv,
    calls::{self, D1, Step},
    forge, green_light, llm, run_forge_with,
};
use serde_json::Value;
use std::{
    fmt::Write,
    net::SocketAddr,
    path::{Path, PathBuf},
};

const PERSON: &str = include_str!("../tests/fixtures/rung-a/person.json");
const MODEL: &[u8] = include_bytes!("../tests/fixtures/rung-a/llm.json");
const FORGE: &[u8] = include_bytes!("../tests/fixtures/rung-a/forge.json");

fn fixture() -> forge::Fixture {
    forge::Fixture {
        // Keep the three recorded model tasks under a bounded per-story guard.
        patience: skein_lib::Duration::from_secs(40),
        configure_domain: |config| {
            // This calm twin uses current API reads.
            config.limits.items = 1;
            config.settle_min = skein_lib::Duration::ZERO;
            config.settle_max = skein_lib::Duration::ZERO;
        },
        setup: || {
            let document = temper_fake::forge::Document::decode(FORGE).expect("rung A forge document");
            let repository = &document.repositories()[0];
            let mut setup = (green_light::fixture().setup)();
            setup.tree = repository.tree.clone().expect("playground seed");
            setup.checks = temper_fake::forge::checks(repository.ci.as_ref());
            setup
        },
        configure: |value, agent, address| {
            (green_light::fixture().configure)(value, agent, address);
            // Use the ordinary template and this story's policy period, rather than B0's case defaults.
            value["world_settings"].as_object_mut().expect("world settings").remove("case");
            // Retain the recorded change result and review beside the chat.
            for (name, limit) in [
                ("fleet.attempts", 4),
                ("brief.briefs", 4),
                ("core.call_answer_bytes", 8192),
                ("tasks.result_bytes", 512),
            ] {
                value["engine"]["limits"]["overrides"][name] = limit.into();
            }
            value["workers"][0]["limits"]["overrides"]["core.call_answer_bytes"] = 8192.into();
            value["application"]["limits"]["overrides"]["checkout.message_bytes"] = 8192.into();
            value["workers"][0]["limits"]["overrides"]["hub.event_bytes"] = 8190.into();
            agent["profile"]["declared"]["tool_payload"] = 8192.into();
            for project in value["world_settings"]["policies"].as_array_mut().expect("project policies") {
                project["policy"]["period_spend"] = 4000.into();
                project["policy"]["roles"][0]["period_spend"] = 4000.into();
            }
            agent["profile"]["declared"]["read_window"] = 4098.into();
            agent["profile"]["policy"]["max_turns"] = 16.into();
        },
        ..green_light::fixture()
    }
}

fn recipe(_: SocketAddr, _: &Path) -> llm::Recipe {
    let document = temper_fake::llm::Document::decode(MODEL).expect("walk 8 twin document");
    let mut recipe = llm::scripted(Box::new([]));
    // Twenty recorded completions also retain their HTTP and query-history records.
    recipe.limits.observations = 1024;
    recipe.limits.observation_bytes = 16 * 1024 * 1024;
    recipe.domain = document.domain(17);
    recipe.domain_limits = temper_fake::llm::LIMITS;
    // Scale the walk's interleaving down while leaving each recorded completion intact.
    let pace: Value = serde_json::from_str(include_str!("../tests/fixtures/rung-a/pace.json")).expect("pacing JSON");
    assert_eq!(pace["requests"], "sequential");
    assert_eq!(pace["close_after"], "final_answer");
    recipe.domain_limits.latency_min =
        skein_lib::Duration::from_millis(pace["model_latency_ms"].as_u64().expect("latency"));
    recipe.domain_limits.latency_max = recipe.domain_limits.latency_min;
    recipe
}

fn person(index: usize) -> &'static str {
    use std::sync::LazyLock;
    static WORDS: LazyLock<Value> = LazyLock::new(|| serde_json::from_str(PERSON).expect("person's requests"));
    WORDS["requests"][index].as_str().expect("request words")
}

fn csv_answer() -> &'static str {
    "CSV export is complete."
}
fn expected_csv(scripts: &Value) -> String {
    let forge: Value = serde_json::from_slice(FORGE).expect("forge JSON");
    let mut source = forge["repositories"][0]["tree"]
        .as_array()
        .expect("seed files")
        .iter()
        .find(|file| file["path"] == "src/main.rs")
        .expect("seed source")["content"]
        .as_str()
        .expect("seed text")
        .to_owned();
    for line in scripts["cued"][0]["turns"]
        .as_array()
        .expect("writer turns")
        .iter()
        .flat_map(|turn| turn["lines"].as_array().expect("writer lines"))
    {
        if line["call"] == "edit" {
            assert_eq!(line["arguments"]["path"], "src/main.rs");
            for edit in line["arguments"]["edits"].as_array().expect("edits") {
                let old = edit["old"].as_str().expect("old text");
                assert_eq!(source.matches(old).count(), 1, "each recorded edit has one target");
                source = source.replacen(old, edit["new"].as_str().expect("new text"), 1);
            }
        }
    }
    source
}

fn acts() -> Vec<Step> {
    use calls::{call, sign_in, until};
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
                "adopt"
            ],
        ),
        call(D1, argv!["chat", "start", "--project", "1", person(0), "--key", "csv"]),
        until(D1, argv!["task", "show", "1", "--tree", "--json"], b"\"task\":2", b"Authority refused"),
        // Walk 8 waits for CSV's final answer and closes each chat before the next request.
        call(D1, argv!["change", "show", "2", "--until", "landed", "--json"]),
        until(D1, argv!["chat", "show", "1"], csv_answer().as_bytes(), b"Authority refused"),
        call(D1, argv!["chat", "close", "1", "--key", "close-csv"]),
        // Dark theme and empty search return with the next walk's whole twin:
        // walk 8's bare advisor requests are now refused on main.
    ]
}

fn check(ran: &Ran) {
    for (index, call) in ran.calls().iter().enumerate() {
        assert_eq!(
            call.exit,
            Some(Exit::Code(0)),
            "call {index}: {} / {}",
            String::from_utf8_lossy(&call.stdout),
            String::from_utf8_lossy(&call.stderr)
        );
    }
    assert_eq!(ran.calls().len(), 7);
    let panel: Value = serde_json::from_slice(&ran.calls()[4].stdout).expect("change panel");
    let panel = &panel["change"];
    assert_eq!(panel["ci"], "passed");
    assert_eq!(panel["step"], "landed");
    assert_eq!(panel["pull"]["number"], 1);
    assert!(
        panel["gates"]
            .as_array()
            .expect("gates")
            .iter()
            .any(|gate| { gate["kind"] == "review" && gate["blocks"] == true && gate["verdict"] == "approved" }),
        "blocking agent review approved at the landed head: {panel}"
    );
    assert!(ran.forge().merged_green(), "forge API agrees CI passed at the merged head");
    let scripts: Value = serde_json::from_slice(MODEL).expect("recorded model document");
    let expected = expected_csv(&scripts);
    assert_eq!(ran.forge().main_file(b"src/main.rs"), expected.as_bytes(), "the recorded CSV program landed");
    let merges: Vec<_> = ran
        .forge()
        .observations()
        .iter()
        .filter_map(|record| {
            let skein_fake_forge_domain::Observation::Merged { repository, base, head, commit, .. } = record else {
                return None;
            };
            assert_eq!(repository.as_ref(), b"playground/widget");
            assert_eq!(base.as_ref(), b"main");
            Some((*head, *commit))
        })
        .collect();
    assert_eq!(merges.len(), 1, "only CSV lands");
    let hex = |id| {
        let mut text = String::new();
        for byte in &skein_fake_forge_domain::object_bytes(id)[..20] {
            write!(text, "{byte:02x}").expect("commit hex");
        }
        text
    };
    assert_eq!(panel["head"], hex(merges[0].0));
    assert_eq!(panel["merge"], hex(merges[0].1));
    let last = ran.forge().snapshots().last().expect("outside API snapshot");
    assert_eq!(last.branches.iter().find(|(branch, _)| branch.as_ref() == b"main").expect("main").1, merges[0].1);
    assert!(last.pulls.is_empty(), "no open pulls after CSV lands");
    check_cues(ran, &scripts);
}

fn check_cues(ran: &Ran, scripts: &Value) {
    // Inspect actual system text: every agent must have selected its own recorded script.
    let cued = scripts["cued"].as_array().expect("scripts");
    let mut counts = vec![0; cued.len()];
    for record in ran.llm() {
        let skein_fake_peers::llm::Observation::Query { query, .. } = record else { continue };
        let system = String::from_utf8_lossy(&query.system);
        let (_, index) = cued
            .iter()
            .enumerate()
            .filter_map(|(index, script)| system.find(script["cue"].as_str().expect("cue")).map(|at| (at, index)))
            .min()
            .unwrap_or_else(|| panic!("off-script system: {system}"));
        let expected_parent = ["task 2", "person 1", "task 2"][index];
        let own_parent =
            system.split_once("[Requested by ").expect("own requester").1.split_once(']').expect("requester end").0;
        assert_eq!(own_parent, expected_parent, "earliest cue selected another task's script: {system}");
        counts[index] += 1;
    }
    for (script, count) in cued.iter().zip(counts) {
        println!("{}: {count} queries", script["cue"]);
        assert_eq!(
            count,
            script["turns"].as_array().expect("turns").len(),
            "every recorded completion replays for {}",
            script["cue"]
        );
    }
}

/// Replay walk 8's CSV request and landing on fresh fake peers.
pub fn run() {
    let ran = run_forge_with(fixture(), ForgeRecipe { source: PathBuf::new(), build: recipe }, acts);
    check(&ran);
}

#[cfg(test)]
mod tests {
    use super::{FORGE, MODEL, PERSON};
    use serde_json::Value;
    use skein_fake_llm_domain::{self as domain, api};
    use skein_lib::{Env, Queue, ReplyTo, Time, Token, Wall};

    #[test]
    fn story_documents_fit_and_every_cue_plays_its_recorded_first_turn() {
        let forge = temper_fake::forge::Document::decode(FORGE).expect("forge document");
        let tree = forge.repositories()[0].tree.as_ref().expect("seeded tree");
        let source = tree.iter().find(|file| file.path.as_ref() == b"src/main.rs").expect("widget program");
        assert_eq!(source.content.as_ref(), temper_fake::forge::FILES[1].1);
        assert!(tree.iter().any(|file| file.path.as_ref() == b"README.md" && file.content.as_ref() == b"# widget\n"));
        assert!(tree.iter().any(|file| file.path.as_ref() == b".forgejo/workflows/ci.yml"));
        let _ = temper_fake::llm::Document::decode(MODEL).expect("whole twin fits");
        let value: Value = serde_json::from_slice(MODEL).expect("model JSON");
        let person: Value = serde_json::from_str(PERSON).expect("person JSON");
        let scripts = value["cued"].as_array().expect("script list");
        assert_eq!(scripts.len(), 3);
        for request in person["requests"].as_array().expect("requests") {
            assert!(scripts.iter().any(|script| script["cue"] == *request), "person's request has a script");
        }
        for script in scripts {
            let mut alone = value.clone();
            alone["cued"] = serde_json::json!([script]);
            let document = temper_fake::llm::Document::decode(&serde_json::to_vec(&alone).expect("one script JSON"))
                .expect("one script fits");
            let mut model = document.domain(17);
            let query = api::Query {
                credential: Some(0),
                model: b"fake".as_slice().into(),
                system: script["cue"].as_str().expect("cue").as_bytes().into(),
                messages: Box::new([api::Message {
                    role: api::Role::User,
                    parts: Box::new([api::Part::Text {
                        text: b"Begin the work your brief describes.".as_slice().into(),
                    }]),
                }]),
                tools: Box::new([]),
                choice: api::ToolChoice::None,
                max_tokens: 16384,
                caching: api::Caching::Unscoped,
            };
            let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits: temper_fake::llm::LIMITS };
            let mut out = Queue::with_capacity(1);
            domain::step(
                &mut model,
                &env,
                domain::Event::Call { reply_to: ReplyTo::new(Token::new(1)), call: Token::new(2), query },
                &mut out,
            );
            if out.is_empty() {
                domain::fire(&mut model, &env, &mut out);
            }
            let response = out.pop();
            let Some(domain::Request::Reply { result: Ok(answer), .. }) = response else {
                panic!("scripted answer for {}: {response:?}", script["cue"])
            };
            let api::Part::ToolCall { name, .. } = &answer.parts[0] else { panic!("recorded call") };
            assert_eq!(name.as_ref(), script["turns"][0]["lines"][0]["call"].as_str().expect("first call").as_bytes());
        }
    }
}
