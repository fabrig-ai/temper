//! Worker-root stories at the protocol's boundary (jig's `domain/hosts.md`,
//! sections 4, 6, 6.5 and 6.8; `domain/worker.md`, sections 2, 4.1, 4.3 and
//! 7).

mod admission;
mod entrance;
pub(crate) mod fixtures;
mod harness;
mod lifecycle;
mod limits;
mod live;

mod translate;
