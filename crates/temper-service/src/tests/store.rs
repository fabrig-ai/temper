use super::fixtures;
use crate::store::{self, Store};
use skein_io::kernel::{Complete, Done, Fd, Op, Submit};
use skein_lib::Time;
use temper_engine_domain as engine;

#[test]
fn the_store_opens_over_memory_and_answers_a_commit() {
    let mut store = fixtures::memory_store();
    fixtures::commit(&mut store, 1, Box::new([]));
    let Some(jig_protocol_store::Event::Committed { number: 1, .. }) = store.above.pop() else {
        panic!("first commit")
    };
    assert!(store.above.is_empty());
}

#[test]
fn sizes_past_skein_kvs_limits_refuse_the_store() {
    let (root, _, profile) = fixtures::store_limits();
    let checked = store::limits(&root, &root.forge, &profile).expect("small fits");
    assert!(checked.store.is_usable());
    assert_eq!(checked.store, checked.secrets);
    assert_eq!(checked.core, engine::core_limits(&root));
    assert_eq!(checked.journal, engine::journal_limits(&root.journal));
    for field in ["key", "value", "ops", "commit", "page", "queue", "queued bytes"] {
        let mut short = profile;
        match field {
            "key" => short.store.key = checked.store.key.checked_sub(1).unwrap(),
            "value" => short.store.value = checked.store.value.checked_sub(1).unwrap(),
            "ops" => short.store.ops = checked.store.ops.checked_sub(1).unwrap(),
            "commit" => short.store.commit = checked.store.commit.checked_sub(1).unwrap(),
            "page" => short.store.page.bytes = checked.store.page.bytes.checked_sub(1).unwrap(),
            "queue" => short.store.queued = root.journal.commits.checked_sub(1).unwrap(),
            "queued bytes" => {
                short.store.queued_bytes = u64::from(root.journal.commits)
                    .checked_mul(u64::from(checked.store.commit))
                    .unwrap()
                    .checked_sub(1)
                    .unwrap();
            }
            _ => panic!("named row"),
        }
        assert!(store::limits(&root, &root.forge, &short).is_none(), "{field}");
    }
}

#[test]
fn a_forge_row_committed_is_restored_by_the_forge_interval() {
    let mut store = fixtures::memory_store();
    let row = fixtures::landed_row();
    fixtures::commit(
        &mut store,
        1,
        Box::new([engine::Write::Save(engine::Record::Forge { row: Box::new(row.clone()) })]),
    );
    let Some(jig_protocol_store::Event::Committed { number: 1, .. }) = store.above.pop() else {
        panic!("first commit")
    };
    let rows = fixtures::load(&mut store, engine::Range::Forge);
    assert_eq!(rows.as_ref(), [engine::Record::Forge { row: Box::new(row) }]);
}

#[test]
fn demo_states_committed_are_restored_by_the_forge_interval() {
    use temper_engine_domain_forge::{Demo, DemoState, Stored};
    for state in [DemoState::Running, DemoState::Ended] {
        let mut store = fixtures::memory_store();
        let row = Stored::Demo(Demo { forge: 1, state });
        fixtures::commit(
            &mut store,
            1,
            Box::new([engine::Write::Save(engine::Record::Forge { row: Box::new(row.clone()) })]),
        );
        let Some(jig_protocol_store::Event::Committed { number: 1, .. }) = store.above.pop() else {
            panic!("demo commit")
        };
        let rows = fixtures::load(&mut store, engine::Range::Forge);
        assert_eq!(rows.as_ref(), [engine::Record::Forge { row: Box::new(row) }]);
    }
}

#[test]
fn the_store_closes_after_its_drivers() {
    let mut store = fixtures::memory_store();
    let root = Fd::new(10);
    assert_eq!(store.adopt(root, 1000), Ok(()));
    assert_eq!(store.adopt(Fd::new(11), 1000), Err(Fd::new(11)));
    store.iterate();
    let openings = take_openings(&mut store, root);
    store.close();
    assert!(!store.closed());
    for (index, submit) in openings.into_iter().enumerate() {
        store.complete(Complete {
            op: submit.op,
            kind: submit.kind,
            result: Ok(Done::Fd(Fd::new(20_i32.checked_add(i32::try_from(index).unwrap()).unwrap()))),
        });
    }
    store.iterate();
    let closes = take_closes(&mut store, &[Fd::new(20), Fd::new(21)]);
    assert!(!store.closed());
    for submit in closes {
        store.complete(Complete { op: submit.op, kind: submit.kind, result: Ok(Done::Nothing) });
    }
    store.iterate();
    store.iterate();
    let submit = take_closes(&mut store, &[root]).into_iter().next().unwrap();
    assert!(!store.closed());
    store.complete(Complete { op: submit.op, kind: submit.kind, result: Ok(Done::Nothing) });
    store.iterate();
    assert!(store.closed());
    assert!(!store.work_pending(Time::ZERO));
    assert_eq!(store.next_deadline(), None);
}

fn take_openings(store: &mut Store, root: Fd) -> [Submit; 2] {
    let out = [store.submitted.pop().expect("secrets clone"), store.submitted.pop().expect("transcript clone")];
    for (index, submit) in out.iter().enumerate() {
        let Op::Open { root: found, path, .. } = &submit.kind else { panic!("directory open") };
        assert_eq!(*found, root);
        assert_eq!(path.as_ref(), b".");
        assert_eq!(submit.op.raw() >> 60_u32, 9_u64.checked_add(u64::try_from(index).unwrap()).unwrap());
        assert!(store::owns(submit.op));
    }
    assert!(store.submitted.is_empty());
    out
}

fn take_closes(store: &mut Store, fds: &[Fd]) -> Box<[Submit]> {
    assert_eq!(usize::try_from(store.submitted.len()).unwrap(), fds.len(), "closes for {fds:?}");
    let mut out = skein_lib::List::with_capacity(u32::try_from(fds.len()).unwrap());
    for fd in fds {
        let submit = store.submitted.pop().expect("close submission");
        let Op::Close { fd: found } = submit.kind else { panic!("close operation") };
        assert_eq!(found, *fd);
        assert!(store::owns(submit.op));
        out.push(submit).unwrap();
    }
    assert!(store.submitted.is_empty());
    out.into_boxed()
}

#[test]
fn a_writer_committed_is_reloaded_through_the_real_store() {
    let mut store = fixtures::memory_store();
    let row = temper_engine_domain_forge::Stored::Writer(skein_forge_client::Writer { forge: 1, author: 42 });
    fixtures::commit(
        &mut store,
        1,
        Box::new([engine::Write::Save(engine::Record::Forge { row: Box::new(row.clone()) })]),
    );
    let Some(jig_protocol_store::Event::Committed { number: 1, .. }) = store.above.pop() else {
        panic!("writer commit")
    };
    assert_eq!(
        fixtures::load(&mut store, engine::Range::Forge).as_ref(),
        [engine::Record::Forge { row: Box::new(row) }]
    );
}

#[test]
fn the_store_uses_the_roots_effective_core_limits() {
    let (root, parsed, profile) = fixtures::store_limits();
    let effective = engine::core_limits(&root);
    assert_ne!(parsed.policy_bytes, effective.policy_bytes);
    let checked = store::limits(&root, &root.forge, &profile).unwrap();
    assert_eq!(checked.core, effective);
}

#[test]
fn settings_history_loads_with_the_roots_effective_envelope() {
    use jig_domain_core::{CoreRecord, Record, reads, settings};
    use skein_lib::{Token, Wall};
    let (root, _, _) = fixtures::store_limits();
    let effective = engine::core_limits(&root);
    let mut store = fixtures::memory_store();
    let revision = settings::Revision {
        head: settings::Head {
            number: 1,
            against: 0,
            at: Wall::EPOCH,
            by: settings::Decider::FirstStart,
            proposal: None,
            moved: false,
        },
        items: Box::new([]),
    };
    fixtures::commit(
        &mut store,
        1,
        Box::new([engine::Write::Save(engine::Record::Core(Record::Core(CoreRecord::Settings(
            settings::Record::Revision { history: false, revision: revision.clone() },
        ))))]),
    );
    let Some(jig_protocol_store::Event::Committed { .. }) = store.above.pop() else {
        panic!("settings history commit")
    };
    store.ask(jig_protocol_store::Request::Read {
        owner: Token::new(42),
        range: reads::Range {
            party: 1,
            scope: reads::Scope::SettingsHistory { first: 1, last: 1, newest: 1, whole: true },
            after: None,
            backwards: false,
            rows: 1,
            bytes: reads::settings::load_bytes(&effective, 1).unwrap(),
        },
    });
    let Some(jig_protocol_store::Event::Read { result: reads::Loaded::Page { rows, next: None }, .. }) =
        store.above.pop()
    else {
        panic!("settings history must answer a value, not Busy")
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(reads::settings::record_revision(&rows[0].value), Some(&revision));
}
