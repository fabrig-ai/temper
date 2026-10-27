//! Scripted root for landing-by-turn stories (domain/landing.md, 16).
use crate::{REPO, World, changes};
use skein_lib::Wall;
use temper_engine_domain_forge as top;
use temper_engine_domain_forge_change as change;
use temper_engine_domain_forge_landing as landing;

pub const PROJECT: u32 = 5;
pub const CHANGE: u64 = 51;
pub const LANDING: u64 = 61;
#[must_use]
pub fn turn_world() -> World {
    let mut world = World::turn();
    world.adopt();
    assert!(world.project_landing(
        PROJECT,
        change::Way::Turn,
        Box::new([top::RepositoryGates {
            repository: REPO,
            gates: Box::new([landing::Gate {
                name: Box::from(b"gate".as_slice()),
                workflow: Box::from(b"gate.yml".as_slice()),
                ignored: Box::new([Box::from(b"docs".as_slice())]),
                blocking: true
            }])
        }])
    ));
    world
}
#[must_use]
pub fn landing_row(change: &top::ChangeRow) -> top::LandingRow {
    top::LandingRow {
        landing: landing::Landing {
            task: LANDING,
            project: PROJECT,
            phase: landing::Phase::Waiting,
            members: Box::new([landing::Member {
                change: change.task,
                repository: landing::Repository {
                    forge: change.repository.forge,
                    repository: change.repository.repository,
                },
                pull: change.pull.expect("opened change"),
                head: None,
                base: None,
                candidate: None,
                gates: Box::new([landing::GateProof {
                    proof: landing::Proof::Unkeyed,
                    posted: landing::Posted::Nothing,
                    faults: 0,
                }]),
                asked: None,
                landed: None,
            }]),
            preparation: 0,
            stale_proofs: 0,
        },
        asks: Box::new([None]),
        entries: Box::new([]),
    }
}
pub fn step_landing(world: &mut World, task: u64, entry: u64) -> landing::Decision {
    world.take_seen();
    world.event(top::Event::StepLanding {
        task,
        entry,
        heard: top::LandingHeard { released: false, cancelled: false },
    });
    world.run_for(1);
    let mut decision = None;
    let mut committed = None;
    for request in world.take_seen() {
        if let top::Request::LandingDecision { task: decided, decision: choice, entry, .. } = request
            && decided == task
        {
            decision = Some(choice);
            committed = entry;
        }
    }
    if let Some(entry) = committed {
        world.event(top::Event::Committed { entry });
        world.run_for(1);
    }
    decision.expect("landing step made a decision")
}
pub fn drive_landing(world: &mut World, task: u64) {
    // Each bounded step gets two fresh root entries: landing then its member.
    const STEPS: u64 = 80;
    for position in 0..STEPS {
        let entry = task * 100 + position * 2;
        let decision = step_landing(world, task, entry);
        let member = world.landing(task).landing.members[0].change;
        changes::change_step_task(world, member, entry + 1, change::Status::Passed, Box::new([]), false, false);
        if decision == landing::Decision::Finish {
            return;
        }
    }
    panic!("landing did not finish: {:?}", world.landing(task));
}
/// Open a produced change and let its actual procedure reach Ready.
pub fn ready_change(world: &mut World, task: u64, branch: &[u8]) {
    let pull = world.open_pull(branch, b"main");
    let head = crate::translate::commit(world.branch(branch));
    let mut row = changes::change_row();
    row.task = task;
    row.branch = Box::from(branch);
    row.change.task = task;
    row.change.state = change::State::Gating { head, asked: Box::new([]) };
    row.change.last_head = Some(head);
    row.change.since = world.wall();
    row.change.ready_since = Some(Wall::from_nanos(100_000_000_000));
    row.pull = Some(pull);
    world.event(top::Event::Change { row });
    changes::change_step_task(world, task, task, change::Status::Passed, Box::new([]), false, false);
}
#[must_use]
pub fn first_landing() -> World {
    let mut world = turn_world();
    world.produce(b"temper/51");
    world.push_file(b"main", b"base", b"main moved");
    ready_change(&mut world, CHANGE, b"temper/51");
    let row = landing_row(world.change(CHANGE));
    world.event(top::Event::Landing { row });
    drive_landing(&mut world, LANDING);
    world
}

/// A documentation-only successor whose prepare is performed by the change.
pub fn documentation_landing(world: &mut World, change: u64, landing: u64, branch: &[u8]) {
    world.produce_file(branch, b"docs/guide.md", b"A guide");
    ready_change(world, change, branch);
    let mut row = landing_row(world.change(change));
    row.landing.task = landing;
    world.event(top::Event::Landing { row });
    drive_landing(world, landing);
}
