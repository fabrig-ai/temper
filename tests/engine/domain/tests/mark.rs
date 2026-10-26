//! Restored forge work follows the root's durable start mark.
use skein_forge_client::{Condition, Effect, Entry};
use skein_lib::{Duration, Time, Wall};
use temper_engine_domain::{self as engine, Record, Write};
use temper_engine_domain_forge as forge;
use temper_engine_domain_world::{direct::Driver, fixtures, forge_stand::ForgeStand, walking};

// The direct world's existing restart horizon.
const PASSES: u32 = 256;
const TASK: u64 = 51; // The shared change fixture's task.
const ENTRY: u64 = 1; // One restored outbox entry.
const SEED: u64 = 71; // The direct forge fixture's deployment.

#[test]
fn the_forge_reaches_no_forge_before_the_mark() {
    let limits = fixtures::forge_limits();
    let mut store = fixtures::restored_store(walking::config(SEED), &limits);
    let change = temper_engine_forge_world::changes::change_row();
    let entry = Entry {
        number: ENTRY,
        task: TASK,
        repository: temper_engine_forge_world::REPO,
        effect: Effect {
            write: skein_forge::api::Write::CreateIssue {
                key: b"mark-entry".as_slice().into(),
                title: b"Marked".as_slice().into(),
                body: b"After the mark".as_slice().into(),
            },
            condition: Condition::None,
        },
        start: None,
        attempt: None,
        failures: 0,
        queued: false,
    };
    store.seed(
        Box::new([
            Record::Forge { row: Box::new(forge::Stored::Entry { entry, held: false }) },
            Record::Forge { row: Box::new(forge::Stored::Change(change)) },
        ]),
        Box::new([]),
    );
    store.crash(SEED);
    let mut driver = Driver::cold(store, walking::config(SEED), &limits);
    driver.forge = Some(ForgeStand::new());
    for _ in 0..PASSES {
        if driver.store.pending.front().is_some_and(|(_, writes)| writes.contains(&Write::Wires)) {
            break;
        }
        if !driver.store.pending.is_empty() {
            let number = driver.store.apply();
            driver.send(engine::Event::Store(engine::StoreInput::Committed { number, fill: 0 }));
        }
        driver.advance(false);
    }
    let (mark, writes) = driver.store.pending.front().expect("restart mark");
    let mark = *mark;
    assert!(writes.contains(&Write::Wires));
    // The restored change's deadline is already due; explicitly fire the root too.
    driver.env.now = Time::from_nanos(Duration::from_secs(100).as_nanos());
    driver.env.wall = Wall::from_nanos(Duration::from_secs(100).as_nanos());
    for _ in 0..PASSES {
        engine::fire(&mut driver.root, &driver.env);
        driver.advance(false);
    }
    assert_eq!(driver.forge.as_ref().expect("fake").calls, 0);
    assert!(!driver.root.ready());
    let number = driver.store.apply();
    assert_eq!(number, mark);
    driver.send(engine::Event::Store(engine::StoreInput::Committed { number, fill: 0 }));
    driver.pump();
    assert!(driver.root.ready());
    assert_eq!(driver.forge.as_ref().expect("fake").writes, 1, "restored entry made once");
    driver.pump();
    assert_eq!(driver.forge.as_ref().expect("fake").writes, 1);
    let skein_forge::api::Answer::Items { items, more: false, .. } = driver.forge.as_mut().expect("fake").read(
        temper_engine_forge_world::REPO,
        skein_forge::api::Read::Items { since: Time::ZERO, page: 1, kind: Some(skein_forge::api::Kind::Issue) },
    ) else {
        panic!("one bounded issue page");
    };
    assert_eq!(items.len(), 1, "the fake forge holds the made issue");
}
