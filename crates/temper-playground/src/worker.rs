//! The world's worker module entry (playground.md, 7).
use crate::{Machine, Playground, Process, STORIES};
use skein_sim_world::World;
use skein_ui_web::bridge::Start;
use wasm_bindgen::prelude::wasm_bindgen;

/// Steps a slice runs at most (skein's playground.md, 3): the browser
/// tier measures a slice's latency against it.
pub const BUDGET: u32 = 256;

/// Install the worker's slices loop, waiting for the visit's start message.
#[wasm_bindgen(start)]
pub fn start() {
    skein_playground::worker::run::<Playground>(make, BUDGET);
}

/// Story names on the bridge are their directory names (`a`).
fn make(start: &Start) -> World<Process, Machine> {
    let story = STORIES
        .iter()
        .copied()
        .find(|story| story.name.to_ascii_lowercase().as_bytes() == start.story.as_ref())
        .expect("known playground story directory");
    crate::world(story, start.seed, start.start)
}
