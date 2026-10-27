//! First proved, ties by task (domain/landing.md, 9).
use skein_lib::Wall;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Proved {
    pub task: u64,
    pub at: Wall,
}

/// The landing that takes a free project's turn.
#[must_use]
#[expect(clippy::manual_map, reason = "the subset uses explicit matches, not closures")]
pub fn first(proved: &[Proved]) -> Option<u64> {
    let mut selected: Option<Proved> = None;
    for candidate in proved {
        selected = match selected {
            Some(current) if (current.at, current.task) <= (candidate.at, candidate.task) => Some(current),
            Some(_) | None => Some(*candidate),
        };
    }
    match selected {
        Some(value) => Some(value.task),
        None => None,
    }
}
