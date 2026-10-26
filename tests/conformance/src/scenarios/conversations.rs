//! A pair of conversations read across ends and cold starts.
use super::{World, ask, check};
use jig_conformance::{referee::Violation, scenarios::Action};
use jig_domain_core as core;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use temper_engine_domain as root;

const PARTY: u64 = 1;
const PROJECT: u32 = 1;
const HOST_MESSAGE: u64 = 100;
const CREATE_KEPT: u8 = 100;
const CREATE_RELEASED: u8 = 101;
const MARK_KEPT: u8 = 102;
const MARK_RELEASED: u8 = 103;
const MARK_RESULTS: u8 = 104;
const KEPT: &[u8] = b"kept conversation";
const RELEASED: &[u8] = b"forgettable conversation";

pub(super) fn action(world: &mut World, action: Action) {
    match action {
        Action::Conversation { forgettable } => {
            let mut member = jig_core_world::stories::reads::conversation_member(forgettable);
            member.words = if forgettable { RELEASED } else { KEPT }.into();
            member.authority.spend = 10;
            member.authority.tools = super::authority(world).tools.0;
            member.authority.grants = super::authority(world)
                .grants
                .iter()
                .map(|grant| people::Grant {
                    connector: grant.connector,
                    kind: grant.kind,
                    pattern: people::Pattern {
                        segments: grant.pattern.segments.clone(),
                        last: match &grant.pattern.last {
                            tasks::Last::Exact(words) => people::Last::Exact(words.clone()),
                            tasks::Last::Open(words) => people::Last::Open(words.clone()),
                        },
                    },
                })
                .collect();
            member.flags.listed = forgettable;
            member.authority.delegation = people::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 };
            ask(
                world,
                if forgettable { CREATE_RELEASED } else { CREATE_KEPT },
                people::Ask::Create { project: PROJECT, batch: Box::new([member]) },
            );
        }
        Action::Turn => {
            world.peers.children = [KEPT, RELEASED]
                .map(|words| {
                    world
                        .store
                        .rows
                        .values()
                        .find_map(|record| match record {
                            root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row)))
                                if row.spec.words.as_ref() == words =>
                            {
                                Some(row.number)
                            }
                            root::Record::Core(_) | root::Record::Forge { .. } | root::Record::Delivery { .. } => None,
                        })
                        .expect("created conversation")
                })
                .to_vec();
            release(world);
        }
        Action::MarkTurns => {
            for (index, task) in world.peers.children.clone().into_iter().enumerate() {
                ask(
                    world,
                    if index == 0 { MARK_KEPT } else { MARK_RELEASED },
                    people::Ask::MarkSeen { seen: people::Seen::Conversation { task, through: 1 } },
                );
            }
        }
        Action::MarkResults => {
            let through = world
                .store
                .rows
                .values()
                .filter_map(|record| match record {
                    root::Record::Core(core::Record::Tasks(tasks::Stored::Ended(row))) => Some(row.result_position),
                    root::Record::Core(_) | root::Record::Forge { .. } | root::Record::Delivery { .. } => None,
                })
                .max()
                .expect("ended results");
            ask(world, MARK_RESULTS, people::Ask::MarkSeen { seen: people::Seen::Results { through } });
        }
        Action::WatchQuery
        | Action::EndTask
        | Action::ReadEnded
        | Action::EndConversations
        | Action::Compound
        | Action::CompoundAgain
        | Action::CancelWithGrace
        | Action::Finish
        | Action::Ready
        | Action::Late
        | Action::Effect(_)
        | Action::DeliverLate
        | Action::OtherHand
        | Action::HoldWrites
        | Action::ReleaseWrites
        | Action::CheckDeadline
        | Action::ChangeJudge
        | Action::FillPool
        | Action::ShrinkPool
        | Action::FinishHolder(_)
        | Action::Propose
        | Action::NarrowPolicy
        | Action::AcceptProposal
        | Action::StartStanding
        | Action::Period(_)
        | Action::Resume
        | Action::PartyAmend
        | Action::RunAmend => unreachable!("conversation action"),
    }
}

fn release(world: &mut World) {
    for index in 0..world.peers.children.len() {
        finish(world, index);
    }
}

pub(super) fn finish(world: &mut World, index: usize) {
    let task = world.peers.children[index];
    let attempt = world.peers.assignments.iter().rev().find(|a| a.task == task).expect("conversation assigned").attempt;
    world.peers.worker.message(task, attempt, HOST_MESSAGE, b"continue conversation".as_slice().into());
}

pub(super) fn check_ending(world: &World) -> Result<(), Violation> {
    let kept = world.peers.children[0];
    let forgotten = world.peers.children[1];
    check(
        world.store.rows.get(&root::Key::Core(core::Key::People(people::Key::EndedRead { party: PARTY, task: kept })))
            == Some(&root::Record::Core(core::Record::People(people::Stored::EndedRead {
                party: PARTY,
                task: kept,
                through: 1,
            }))),
        "kept conversation lost its marked turn",
    )?;
    check(
        world.store.rows.keys().all(|key| match key {
            root::Key::Core(
                core::Key::Tasks(
                    tasks::Key::Ended(task)
                    | tasks::Key::EndedByParty { task, .. }
                    | tasks::Key::EndedByProject { task, .. },
                )
                | core::Key::People(people::Key::ReadConversation { task, .. } | people::Key::EndedRead { task, .. }),
            ) => *task != forgotten,
            root::Key::Core(core::Key::Tasks(tasks::Key::Update {
                task, object: tasks::UpdateObject::Result, ..
            })) => task.value() != forgotten,
            root::Key::Wires | root::Key::Core(_) | root::Key::Forge { .. } | root::Key::Delivery(_) => true,
        }),
        "forgettable conversation left an owner, result update, index or read position",
    )?;
    check(
        world.store.rows.values().all(|record| match record {
            root::Record::Core(core::Record::Tasks(tasks::Stored::Result(row))) => row.task != forgotten,
            root::Record::Core(_) | root::Record::Forge { .. } | root::Record::Delivery { .. } => true,
        }),
        "forgettable conversation left its result row",
    )
}
