//! Query deliveries and ended pages checked against the durable index.
use super::{World, ask, check};
use crate::{Peers, observations::Store};
use jig_conformance::{referee::Violation, scenarios::Action};
use jig_domain_core::{self as core, documents, reads};
use jig_domain_tasks as tasks;
use skein_lib::{ReplyTo, Token};
use temper_engine_domain as root;

const WATCH: u64 = 7090;
const READ: u64 = 7091;
const FIRST_QUERY_CHAT: u8 = 110;
const SECOND_QUERY_CHAT: u8 = 111;
const INITIAL: u8 = 0;
const WAITING: u8 = 1;
const READY: u8 = 2;
const ADOPTION_WAIT: u64 = 6_000_000_000;
const RETRY_WAIT: u64 = 1_000_000_000;
#[derive(Clone, Debug, Default)]
pub(crate) struct State {
    next: Option<reads::Cursor>,
    found: Vec<u64>,
    checks: u32,
    deliveries: u32,
}
fn query() -> documents::TaskQuery {
    jig_core_world::fixtures::OWN_TASKS
}
fn indexed(store: &Store) -> Vec<u64> {
    store
        .rows
        .keys()
        .filter_map(|key| match key {
            root::Key::Core(core::Key::Tasks(tasks::Key::EndedByParty { party: 1, task, .. })) => Some(*task),
            root::Key::Wires | root::Key::Core(_) | root::Key::Forge { .. } | root::Key::Delivery(_) => None,
        })
        .collect()
}
fn read(world: &mut World, from: Option<reads::Cursor>) {
    crate::send(
        world,
        root::Event::Party(root::PartyInput::Read {
            reply_to: ReplyTo::new(Token::new(READ)),
            sign_in: world.peers.sign_in.expect("signed in"),
            read: reads::Read::Tasks { query: query(), live: None, ended: from },
        }),
    );
}
pub(super) fn advance(world: &mut World, action: Action, phase: &mut u8) -> Result<bool, Violation> {
    match action {
        Action::WatchQuery => {
            if *phase < READY && world.peers.children.is_empty() {
                ask(
                    world,
                    if *phase == INITIAL { FIRST_QUERY_CHAT } else { SECOND_QUERY_CHAT },
                    crate::chat(&world.peers, &world.store, 1, b"query task"),
                );
                *phase += 1;
                return Ok(true);
            }
            if world.peers.children.is_empty() {
                world.peers.children = world
                    .store
                    .rows
                    .values()
                    .filter_map(|record| match record {
                        root::Record::Core(core::Record::Tasks(tasks::Stored::Live(row))) => Some(row.number),
                        root::Record::Core(_) | root::Record::Forge { .. } | root::Record::Delivery { .. } => None,
                    })
                    .collect();
            }
            crate::send(
                world,
                root::Event::Party(root::PartyInput::Watch {
                    watcher: Token::new(WATCH),
                    sign_in: world.peers.sign_in.expect("signed in"),
                    subject: documents::Watched::Query { query: query() },
                    window: documents::Window { rows: 4, after: None },
                }),
            );
        }
        Action::EndTask => {
            let task = *world.peers.children.iter().find(|task| !super::ended(world, **task)).expect("live query task");
            if *phase == INITIAL && !world.peers.assignments.iter().any(|a| a.task == task) {
                world.advance(ADOPTION_WAIT);
                *phase = WAITING;
                return Ok(true);
            }
            if *phase == WAITING {
                world.advance(RETRY_WAIT);
                *phase = READY;
                return Ok(true);
            }
            super::message(world, task);
        }
        Action::ReadEnded => {
            if *phase == INITIAL {
                world.peers.query.found.clear();
                *phase = WAITING;
                read(world, Some(reads::Cursor::Store { scope: reads::Scope::Tasks { query: query() }, after: None }));
                return Ok(true);
            }
            if let Some(next) = world.peers.query.next.take() {
                read(world, Some(next));
                return Ok(true);
            }
            super::check_lazy(world.peers.query.found == indexed(&world.store), || {
                format!(
                    "ended query differs from its durable party index: found={:?}, index={:?}",
                    world.peers.query.found,
                    indexed(&world.store)
                )
            })?;
            world.peers.query.checks += 1;
        }
        Action::Compound
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
        | Action::RunAmend
        | Action::Conversation { .. }
        | Action::Turn
        | Action::MarkTurns
        | Action::EndConversations
        | Action::MarkResults => unreachable!("query action"),
    }
    Ok(false)
}
fn line(store: &Store, line: &documents::TaskLine) {
    let task = line.chip.number;
    let record = store
        .rows
        .get(&root::Key::Core(core::Key::Tasks(tasks::Key::Live(task))))
        .or_else(|| store.rows.get(&root::Key::Core(core::Key::Tasks(tasks::Key::Ended(task)))));
    let Some(root::Record::Core(core::Record::Tasks(tasks::Stored::Live(owner) | tasks::Stored::Ended(owner)))) =
        record
    else {
        panic!("delivered query line has no durable owner")
    };
    assert_eq!((line.project, line.requester), (owner.project, owner.requester));
}
pub(crate) fn delivered(peers: &mut Peers, store: &Store, delivery: &root::Delivery) {
    if let root::Delivery::Document { watcher, document } = delivery
        && *watcher == Token::new(WATCH)
    {
        let documents::Document::Lines(lines) = document.as_ref() else { panic!("watched query") };
        for value in &lines.lines {
            line(store, value);
        }
        peers.query.deliveries += 1;
    }
    if let root::Delivery::Read { reply, answer } = delivery
        && *reply == ReplyTo::new(Token::new(READ))
    {
        let reads::Answer::Tasks(page) = answer else { panic!("ended query page: {answer:?}") };
        let reads::EndedGroup::Rows { rows, next, .. } = &page.ended else { panic!("ended index") };
        for value in rows {
            line(store, value);
            peers.query.found.push(value.chip.number);
        }
        peers.query.next.clone_from(next);
    }
}

pub(super) fn check_ending(world: &World) -> Result<(), Violation> {
    check(
        world.peers.children.len() == 3
            && indexed(&world.store).len() == 3
            && world.peers.query.checks == 3
            && world.peers.query.deliveries >= 4,
        "query was not watched and read across all three task ends and restarts",
    )
}
