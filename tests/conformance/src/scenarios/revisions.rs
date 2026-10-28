//! The shared revision race through temper's worker and party boundaries.
use super::{World, ask, authority};
use jig_conformance::referee::Violation;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use temper_engine_domain as engine;

const DELEGATE_CALL: u64 = 8001;
const AMEND_CALL: u64 = 8002;
const PARTY_KEY: u8 = 55;

fn target(world: &World) -> u64 {
    world
        .peers
        .answers
        .iter()
        .find_map(|answer| {
            if let engine::CallAnswer::Delegated(tasks) = answer { tasks.first().map(|made| made.task) } else { None }
        })
        .expect("race delegate admitted")
}
fn call(world: &mut World, name: u64, completion: u32, tool: engine::Tool) {
    let assignment = &world.peers.assignments[0];
    crate::link::call(world, assignment.task, assignment.attempt, name, engine::Call { completion, position: 0, tool });
}
fn spec(words: &[u8]) -> tasks::Spec {
    tasks::Spec { asks: false, words: words.into(), parameters: Box::new([]), inputs: Box::new([]) }
}
pub(super) fn party_amend(world: &mut World) -> Result<(), Violation> {
    let mut authority = authority(world);
    authority.budget.spend = 20;
    authority.delegation = tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 };
    call(
        world,
        DELEGATE_CALL,
        1,
        engine::Tool::Delegate {
            batch: Box::new([engine::Delegate {
                executor: tasks::Executor::Agent { charter: 1 },
                spec: spec(b"race target"),
                contract: tasks::Contract::Report { words: 128 },
                authority,
                symbolic_grants: Box::new([]),
                dependencies: Box::new([]),
                wake: tasks::WakePolicy::DEFAULT,
                ..temper_engine_domain_world::fixtures::report_delegate(b"fixture", Box::new([]))
            }]),
        },
    );
    world.drain()?;
    ask(
        world,
        PARTY_KEY,
        people::Ask::Amend {
            project: 1,
            task: target(world),
            revision: 0,
            amendment: people::Amendment {
                tracked: None,
                addressed: None,
                instructions: None,
                spec: Some(people::Spec {
                    asks: false,
                    words: b"party choice".as_slice().into(),
                    parameters: Box::new([]),
                    inputs: Box::new([]),
                }),
                wake: None,
                dependencies: None,
                authority: None,
                llm: None,
                reason: b"party evidence".as_slice().into(),
            },
            release: None,
        },
    );
    Ok(())
}
pub(super) fn run_amend(world: &mut World) {
    call(
        world,
        AMEND_CALL,
        2,
        engine::Tool::Amend {
            target: target(world),
            revision: 0,
            amendment: tasks::Amendment {
                tracked: None,
                addressed: None,
                instructions: None,
                release: None,
                spec: Some(spec(b"run choice")),
                wake: None,
                dependencies: None,
                authority: None,
                llm: None,
                reason: b"run evidence".as_slice().into(),
            },
        },
    );
}
pub(super) fn check(world: &World) -> Result<(), Violation> {
    use jig_domain_core as core;
    let target = target(world);
    let row = &world.store.rows[&engine::Key::Core(core::Key::Tasks(tasks::Key::Live(target)))];
    let engine::Record::Core(core::Record::Tasks(tasks::Stored::Live(row))) = row else { panic!("race target live") };
    super::check(
        row.revisions.spec == 1
            && row.spec.words.as_ref() == b"party choice"
            && world
                .peers
                .answers
                .contains(&engine::CallAnswer::Moved { task: target, now: people::Moved::Spec { revision: 1 } }),
        "party amendment won the shared spec revision",
    )
}
