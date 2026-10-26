//! The world's worker module entry (playground.md, 7).
use crate::{Machine, Playground, Process, STORIES};
use skein_sim_world::World;
use skein_ui_web::bridge::Start;
use wasm_bindgen::prelude::wasm_bindgen;

/// Steps a slice runs at most (skein's playground.md, 3).
/// The browser welcome measures 2.566-3.672 s, 46-73 ms longest slices,
/// and 612.9 MiB at 32 steps. Its 73 ms worst is nearer the 16-33 ms goal
/// than 64 steps' 74 ms; neither reaches it. The 256-step welcome takes
/// 3.277-3.984 s, so these measurements add no one-fifth delay.
pub const BUDGET: u32 = 32;

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
