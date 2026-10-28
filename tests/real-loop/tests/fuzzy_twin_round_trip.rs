//! B0's twin recorded and twinned again: converter stability through the same acts/checks.
use serde_json::Value;
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
};
use temper_real_loop_world::{ForgeRecipe, green_light, llm, run_forge_with};
use temper_twin::{Input, Take};

fn data(source: &Path) -> PathBuf {
    let config: Value =
        serde_json::from_slice(&std::fs::read(source.join("temper.json")).expect("story configuration"))
            .expect("configuration JSON");
    source.join(config["engine"]["deployment"]["directory"].as_str().expect("deployment data directory"))
}

fn twin_recipe(_address: SocketAddr, source: &Path) -> llm::Recipe {
    let mut takes = Vec::new();
    for (task, cue) in green_light::TASKS.into_iter().zip(green_light::CUES) {
        takes.push(Take { task, cue: cue.as_bytes().into() });
    }
    let input = Input {
        data: data(source),
        base: include_bytes!("fixtures/b0/llm-base.json").as_slice().into(),
        takes: takes.into_boxed_slice(),
        // goal and change carry words alone: no forge grant address to replace.
        replaces: Box::new([]),
    };
    let twin = temper_twin::twin(&input).expect("the stopped green light has a twin");
    // The person's direct answer removes the chat's relay turn.
    assert_eq!(&*twin.report.turns, &[(1, 5), (2, 8), (4, 8), (5, 8), (6, 8)]);
    assert!(twin.report.dropped.iter().all(|(_, count)| *count == 0), "the fake emits no opaque reasoning");
    for (task, count) in &twin.report.turns {
        println!("task {task}: {count} turns");
    }
    assert!(twin.report.replaced.is_empty(), "the words-only scripts need no replacements");
    green_light::from_document(&twin.document)
}

#[test]
fn the_green_light_and_its_twin_land_the_same_csv_change() {
    let source = green_light::run();
    green_light::check(&source);
    let scratch = source.into_scratch(); // Drops the stopped world, retains its data.
    let replay = run_forge_with(
        green_light::fixture(),
        ForgeRecipe { source: scratch.path().into(), build: twin_recipe },
        green_light::acts,
    );
    green_light::check(&replay);
}
