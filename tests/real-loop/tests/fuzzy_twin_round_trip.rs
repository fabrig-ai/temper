//! Recorded completions replay the complete green-light story on a fresh deployment.
use serde_json::{Value, json};
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
};
use temper_real_loop_world::{ForgeRecipe, Ran, green_light, llm, run_forge, run_forge_with};
use temper_twin::{Input, Take};

const TASKS: [u64; 4] = [1, 2, 4, 5];
const CUES: [&str; 4] =
    ["[Requested by person 1]", "[Requested by task 1]", "@green_coding", "Review this change at the named head"];

fn data(source: &Path) -> PathBuf {
    let config: Value =
        serde_json::from_slice(&std::fs::read(source.join("temper.json")).expect("story configuration"))
            .expect("configuration JSON");
    source.join(config["engine"]["deployment"]["directory"].as_str().expect("deployment data directory"))
}

fn twin_recipe(_address: SocketAddr, source: &Path) -> llm::Recipe {
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
        // goal and change carry words alone: no forge grant address to replace.
        replaces: Box::new([]),
    };
    let twin = temper_twin::twin(&input).expect("the stopped green light has a twin");
    assert_eq!(&*twin.report.turns, &[(1, 6), (2, 5), (4, 4), (5, 1)]);
    for (task, count) in &twin.report.turns {
        println!("task {task}: {count} turns");
    }
    assert!(twin.report.replaced.is_empty(), "the words-only scripts need no replacements");
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
    let scratch = source.into_scratch(); // Drops the stopped world, retains its data.
    let replay = run_forge_with(
        green_light::fixture(),
        ForgeRecipe { source: scratch.path().into(), build: twin_recipe },
        green_light::acts,
    );
    green_light::check(&replay);
}
