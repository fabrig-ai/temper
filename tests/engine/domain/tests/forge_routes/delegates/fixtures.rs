//! Shared producer completion and durable delegate inspection.
use super::super::*;

pub(crate) const OWNER: u64 = 1;
pub(crate) const PUSHER: u64 = 2;
pub(crate) const INITIAL_COMMIT: u64 = 1;
pub(crate) const BASE_COMMIT: u64 = 3;
const CHANNEL: Token = Token::new(7);
const ANSWER: u64 = 5;
const CONNECTOR: u16 = 0;
const CHANGE_KIND: u16 = 2;
const DELEGATE_INDEX: usize = 2;
const MAX_TICKS: u32 = 200;

pub(crate) fn answer_producer(world: &mut World, producer: &engine::Assignment) {
    let event = world.protocol.answer(
        CHANNEL,
        producer.task,
        producer.attempt,
        ANSWER,
        tasks::End::Finished {
            result: tasks::TaskResult::Change {
                connector: CONNECTOR,
                kind: CHANGE_KIND,
                resource: temper_engine_domain_forge::tag(forge_world::REPO.forge, forge_world::REPO.repository),
                words: Box::from(&b"pushed"[..]),
            },
            cancel_delegates: false,
        },
        engine::Parts::none(),
    );
    world.send(event);
}

pub(crate) fn next_delegate(world: &mut World) -> engine::Assignment {
    for _ in 0..MAX_TICKS {
        world.tick();
        if let Some(assignment) = world.assigned.get(DELEGATE_INDEX) {
            return assignment.clone();
        }
    }
    panic!("the change did not assign its next delegate");
}

pub(crate) fn assert_commit(world: &World, task: u64, name: u32, digits: &[u8]) {
    let Some(Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Live(row)))) =
        world.store.rows.get(&Key::Core(jig_domain_core::Key::Tasks(tasks::Key::Live(task))))
    else {
        panic!("task {task} has no durable live row");
    };
    let parameter = row.spec.parameters.iter().find(|parameter| {
        matches!(parameter, tasks::Parameter::Bytes { name: parameter_name, .. } if *parameter_name == name)
    });
    assert_eq!(parameter, Some(&tasks::Parameter::Bytes { name, value: digits.into() }), "task {task}");
}
