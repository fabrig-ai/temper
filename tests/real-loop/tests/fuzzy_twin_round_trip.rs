//! Recorded completions replay the complete green-light story on a fresh deployment.
use serde_json::{Value, json};
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
};
use temper_real_loop_world::{ForgeRecipe, Ran, green_light, llm, run_forge, run_forge_with};
use temper_twin::{Input, Replace, Take};

const TASKS: [u64; 4] = [1, 2, 4, 5];
const CUES: [&str; 4] =
    ["@green_chat Add CSV export to widget", "@green_work", "@green_coding", "Review this change at the named head"];

fn source_address(ran: &Ran) -> String {
    // This is the recorded delegate call's forge grant, not a credential.
    for seen in ran.llm() {
        if let skein_fake_peers::llm::Observation::Query { query, .. } = seen {
            for message in &query.messages {
                for part in &message.parts {
                    if let skein_fake_llm_domain::api::Part::ToolCall { name, arguments, .. } = part
                        && name.as_ref() == b"delegate"
                    {
                        let args: Value = serde_json::from_slice(arguments).expect("delegate arguments");
                        return args["batch"][0]["authority"]["grants"][0]["segments"][1]
                            .as_str()
                            .expect("forge address")
                            .into();
                    }
                }
            }
        }
    }
    panic!("the chat delegated its work");
}

fn data(source: &Path) -> PathBuf {
    let config: Value =
        serde_json::from_slice(&std::fs::read(source.join("temper.json")).expect("story configuration"))
            .expect("configuration JSON");
    source.join(config["engine"]["deployment"]["directory"].as_str().expect("deployment data directory"))
}

fn twin_recipe(address: SocketAddr, source: &Path) -> llm::Recipe {
    let old = std::fs::read(source.join("twin-source-address")).expect("source forge address");
    let old_text = String::from_utf8(old.clone()).expect("source socket address");
    let mut takes = Vec::new();
    for (task, cue) in TASKS.into_iter().zip(CUES) {
        takes.push(Take { task, cue: cue.as_bytes().into() });
    }
    let base = json!({"listen":{"socket":"127.0.0.1:0"}, "key":"/twin/llm.key", "tls":null,
        "models":[{"name":"fake","window":65536,"output":16384}],
        "cued":[],"asked":[],"otherwise":["off script"]});
    let input = Input {
        data: data(source),
        base: serde_json::to_vec(&base).expect("base JSON").into_boxed_slice(),
        takes: takes.into_boxed_slice(),
        replaces: Box::new([Replace {
            old: old.into_boxed_slice(),
            new: address.to_string().into_bytes().into_boxed_slice(),
        }]),
    };
    let twin = temper_twin::twin(&input).expect("the stopped green light has a twin");
    assert_eq!(&*twin.report.turns, &[(1, 6), (2, 5), (4, 4), (5, 1)]);
    for (task, count) in &twin.report.turns {
        println!("task {task}: {count} turns");
    }
    println!("--replace {old_text}={address}: {} occurrences (fresh forge listener)", twin.report.replaced[0]);
    assert!(twin.report.replaced[0] > 0, "the source forge listener was substituted");
    let document = temper_fake::llm::Document::decode(&twin.document).expect("fake accepts twin");
    let mut recipe = llm::scripted(Box::new([]));
    recipe.domain = document.domain(17);
    recipe.domain_limits = temper_fake::llm::LIMITS;
    recipe
}

fn record_frames(ran: &Ran) {
    // Explicit regeneration only; normal runs never rewrite repository fixtures.
    if std::env::var_os("TEMPER_TWIN_RECORD").is_none() {
        return;
    }
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/twin/tests/fixtures");
    std::fs::create_dir_all(&fixtures).expect("fixtures directory");
    for task in TASKS {
        let name = format!("{task:016x}");
        let path = data(ran.path()).join("transcripts").join(format!("{:016x}", task >> 12)).join(&name);
        std::fs::copy(path, fixtures.join(name)).expect("record task's actual frames");
    }
}

#[test]
fn the_green_light_and_its_twin_land_the_same_csv_change() {
    let source = run_forge(green_light::fixture(), green_light::scripts, green_light::acts);
    green_light::check(&source);
    record_frames(&source);
    std::fs::write(source.path().join("twin-source-address"), source_address(&source)).expect("source forge listener");
    let scratch = source.into_scratch(); // Drops the stopped world, retains its data.
    let replay = run_forge_with(
        green_light::fixture(),
        ForgeRecipe { source: scratch.path().into(), build: twin_recipe },
        green_light::acts,
    );
    green_light::check(&replay);
}
