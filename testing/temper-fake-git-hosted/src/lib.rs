//! The fake git program hosted by skein's world, with shared scenarios.
#![forbid(unsafe_code)]
use skein_io::kernel::Spawn;
use skein_world::Inherited;
use std::collections::BTreeSet;
pub use temper_fake_git::{Answer, Checked, Git, Refusal, Start, Typed, answer, check, id};
pub mod fixtures;

/// Take the worker root and every descriptor inherited by this program.
#[must_use]
pub fn start(inherited: &Inherited) -> Start {
    let root = inherited.roots.iter().find(|(name, _)| name.as_ref() == b"worker").expect("git worker root").1;
    let mut owned: BTreeSet<_> = inherited.pipes.iter().map(|(_, fd)| *fd).collect();
    owned.insert(inherited.signal);
    owned.extend(inherited.dir);
    owned.extend(inherited.roots.iter().chain(&inherited.appends).map(|(_, fd)| *fd));
    Start { root, pipes: inherited.pipes.clone().into(), owned: owned.into_iter().collect() }
}

/// Assert the world's spawn contract before building its program.
#[must_use]
pub fn git(spawn: &Spawn, inherited: &Inherited, transport: Typed) -> Git<Typed> {
    match check(spawn) {
        Ok(_) | Err(Refusal::Usage) => {}
        Err(Refusal::Broken(rule)) => panic!("git's spawn broke {rule}"),
    }
    Git::new(spawn, start(inherited), transport)
}
