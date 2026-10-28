//! Rung A's passing walk 5, replayed from its recorded completions on fake peers.
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

fn pacing() -> (&'static str, usize) {
    use std::sync::LazyLock;
    static PACE: LazyLock<Value> = LazyLock::new(|| {
        serde_json::from_str(include_str!("../tests/fixtures/rung-a/pace.json")).expect("pacing JSON")
    });
    assert_eq!(PACE["release_after_green_merge"], true);
    (
        PACE["cue"].as_str().expect("gate cue"),
        usize::try_from(PACE["assistant_before"].as_u64().expect("gate turn")).expect("bounded turn"),
    )
}

fn fixture() -> forge::Fixture {
    forge::Fixture {
        // Eleven model tasks take about twice B0's turns; keep a bounded per-story guard.
        patience: skein_lib::Duration::from_secs(40),
        reply_gate: Some(pacing()),
        configure_domain: |config| {
            // Retain all three pulls; this calm twin uses current API reads.
            config.limits.items = 3;
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
            // Three requests and their production, review and resolution attempts coexist.
            for (name, limit) in [("fleet.attempts", 4), ("brief.briefs", 4), ("core.call_answer_bytes", 8192)] {
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
        },
        ..green_light::fixture()
    }
}

fn recipe(_: SocketAddr, _: &Path) -> llm::Recipe {
    let document = temper_fake::llm::Document::decode(MODEL).expect("walk 5 twin document");
    let mut recipe = llm::scripted(Box::new([]));
    // Seventy-four recorded completions also retain their HTTP and query-history records.
    recipe.limits.observations = 1024;
    recipe.limits.observation_bytes = 16 * 1024 * 1024;
    recipe.domain = document.domain(17);
    recipe.domain_limits = temper_fake::llm::LIMITS;
    // Scale the walk's interleaving down while leaving each recorded completion intact.
    recipe.domain_limits.latency_min = skein_lib::Duration::from_millis(100);
    recipe.domain_limits.latency_max = recipe.domain_limits.latency_min;
    recipe
}

fn person(index: usize) -> &'static str {
    use std::sync::LazyLock;
    static WORDS: LazyLock<Value> = LazyLock::new(|| serde_json::from_str(PERSON).expect("person's requests"));
    WORDS["requests"][index].as_str().expect("request words")
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
        // In the live walk chat 2 starts after CSV's initial inspection, before its delegation.
        until(D1, argv!["task", "show", "1", "--activity", "32", "--json"], b"window", b"Authority refused"),
        call(D1, argv!["chat", "start", "--project", "1", person(1), "--key", "dark"]),
        until(D1, argv!["task", "show", "1", "--tree", "--json"], b"\"task\":3", b"Authority refused"),
        until(D1, argv!["task", "show", "3", "--tree", "--json"], b"\"task\":7", b"Authority refused"),
        call(D1, argv!["chat", "start", "--project", "1", person(2), "--key", "empty"]),
        until(D1, argv!["task", "show", "2", "--tree", "--json"], b"\"task\":5", b"Authority refused"),
        call(D1, argv!["chat", "close", "1", "--move", "3", "--key", "close-csv"]),
        call(D1, argv!["chat", "close", "2", "--move", "5", "--key", "close-dark"]),
        until(D1, argv!["task", "show", "8", "--tree", "--json"], b"\"task\":10", b"Authority refused"),
        call(D1, argv!["chat", "close", "8", "--move", "10", "--key", "close-empty"]),
        call(D1, argv!["change", "show", "3", "--until", "landed", "--json"]),
        call(D1, argv!["change", "show", "5", "--until", "held", "--json"]),
        call(D1, argv!["change", "show", "10", "--until", "held", "--json"]),
        call(D1, argv!["task", "show", "7", "--until", "ended", "--json"]),
        // Walk 5 stopped with these changes held and pulls open. Cancel them here to drain
        // cleanly; this cleanup also closes their pulls, after the scenario checks.
        call(D1, argv!["task", "cancel", "5", "Twin complete", "--key", "stop-dark"]),
        call(D1, argv!["task", "cancel", "10", "Twin complete", "--key", "stop-empty"]),
        call(D1, argv!["task", "show", "5", "--until", "ended"]),
        call(D1, argv!["task", "show", "10", "--until", "ended"]),
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
    assert_eq!(ran.calls().len(), 21);
    let panel: Value = serde_json::from_slice(&ran.calls()[13].stdout).expect("change panel");
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
    let expected = scripts["cued"][0]["turns"][3]["lines"][0]["arguments"]["content"].as_str().expect("CSV write");
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
    let last = ran
        .forge()
        .snapshots()
        .iter()
        .rfind(|snapshot| snapshot.at <= ran.calls()[15].ended)
        .expect("outside API snapshot before cleanup");
    assert_eq!(last.branches.iter().find(|(branch, _)| branch.as_ref() == b"main").expect("main").1, merges[0].1);
    assert_eq!(
        last.pulls.as_ref(),
        &[2, 3],
        "the two held pulls remain open; panels: {} / {}",
        String::from_utf8_lossy(&ran.calls()[14].stdout),
        String::from_utf8_lossy(&ran.calls()[15].stdout)
    );
    for position in [14, 15] {
        let value: Value = serde_json::from_slice(&ran.calls()[position].stdout).expect("held panel");
        assert!(value["change"]["task_hold"].is_object(), "the walk's other change held: {value}");
        assert_eq!(value["change"]["ci"], "passed");
        assert_eq!(value["change"]["step"], if position == 14 { "resolving" } else { "gating" });
        assert!(value["change"]["merge"].is_null());
    }
    assert!(String::from_utf8_lossy(&ran.calls()[16].stdout).contains("Approve."));
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
        let expected_parent = [
            "task 3", "task 5", "task 10", "person 1", "person 1", "person 1", "task 3", "task 5", "task 10",
            "task 10", "task 5",
        ][index];
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

/// Replay walk 5 with the same acts and terminal outcomes on fresh fake peers.
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
        let (cue, replies) = super::pacing();
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
        assert_eq!(scripts.len(), 11);
        let paced = scripts.iter().find(|script| script["cue"] == cue).expect("paced script exists");
        assert_eq!(paced["turns"].as_array().expect("paced turns").len(), replies + 1);
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
