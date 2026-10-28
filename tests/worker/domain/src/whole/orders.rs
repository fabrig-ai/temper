//! The whole worker's failed prepares, with real fake-disk/forge operations
//! completed in waves, so several checkout terminals wait together.

use std::collections::{BTreeMap, VecDeque};

use skein_lib::Token;
use temper_worker_domain::{Event, Fact, Request, checkout, step, wire};

use super::{World, assignment, translate};

/// Three workspaces keep three independent operation completions waiting.
pub const RUNS: u32 = 3;
const SEED: u64 = 9211;
const WAVES: usize = 32; // A prepare needs Make, Clone, Fetch, and its release.

#[derive(Debug, PartialEq, Eq)]
pub enum Crossing {
    Io { workspace: Token },
    Released { workspace: Token },
    Answer { run: Token, idle: u32 },
}

pub struct Report {
    pub crossed: Vec<Crossing>,
    pub peak_held: usize,
}

/// Hold every operation terminal until all the current wave's operations
/// have run, then offer that wave FIFO. Released facts are collected at
/// each entry-point boundary, before handing its output to the engine.
#[must_use]
pub fn failed_prepares() -> Report {
    let mut world = World::with_slots(SEED, RUNS);
    let mut report = Report { crossed: Vec::new(), peak_held: 0 };
    let mut places = BTreeMap::new();
    let mut held = VecDeque::new();
    for number in 1..=RUNS {
        let event = assignment(
            world.first,
            Token::new(u64::from(number)),
            u64::from(number),
            wire::Start::Branch { branch: Box::from(&b"gone"[..]) },
        );
        step(&mut world.domain, &world.env, event, &mut world.out);
        collect(&mut world, &mut report, &mut places, &mut held, None);
    }
    for _ in 0..WAVES {
        report.peak_held = report.peak_held.max(held.len());
        if held.is_empty() {
            return report;
        }
        let wave = std::mem::take(&mut held);
        for (owner, done) in wave {
            step(&mut world.domain, &world.env, Event::Done { owner, done }, &mut world.out);
            collect(&mut world, &mut report, &mut places, &mut held, Some(owner));
            world.domain.reclaim();
        }
    }
    panic!("failed prepares settle within the whole world's wave budget");
}

fn collect(
    world: &mut World,
    report: &mut Report,
    places: &mut BTreeMap<Token, Token>,
    held: &mut VecDeque<(Token, checkout::git::Done)>,
    completing: Option<Token>,
) {
    while let Some(fact) = world.domain.pop_fact() {
        if let Fact::Checkout { fact: checkout::Fact::Released { .. } } = fact {
            let owner = completing.expect("a failed prepare releases on its last operation terminal");
            let workspace = *places.get(&owner).expect("Make named the operation's workspace");
            report.crossed.push(Crossing::Released { workspace });
        }
    }
    while let Some(request) = world.out.pop() {
        match request {
            Request::Io { owner, op, .. } => {
                let workspace = match &op {
                    checkout::git::Op::Make { workspace } => {
                        places.insert(owner, *workspace);
                        *workspace
                    }
                    checkout::git::Op::Clone { at, .. }
                    | checkout::git::Op::Fetch { at, .. }
                    | checkout::git::Op::Create { at, .. }
                    | checkout::git::Op::CheckOut { at, .. }
                    | checkout::git::Op::IsAncestor { at, .. }
                    | checkout::git::Op::Merge { at, .. }
                    | checkout::git::Op::Commit { at, .. }
                    | checkout::git::Op::Push { at, .. } => at.workspace,
                };
                report.crossed.push(Crossing::Io { workspace });
                held.push_back((owner, translate::perform(&mut world.forge, &mut world.disk, op)));
            }
            Request::Answer { run, .. } => {
                report.crossed.push(Crossing::Answer { run, idle: world.domain.checkout().idle() });
            }
            Request::Hello(_) | Request::Link(_) => {}
            other @ (Request::Source { .. }
            | Request::SourceFault
            | Request::Stop
            | Request::CancelIo { .. }
            | Request::CancelRelay { .. }
            | Request::Process(_)
            | Request::Decode { .. }
            | Request::Encode { .. }
            | Request::Fact { .. }) => panic!("failed prepares have no agent or source requests: {other:?}"),
        }
    }
}
