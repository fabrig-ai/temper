//! The shell's real baseline configuration, over persistent memory maps.
use super::{protocol, with_patch_sets};
use crate::tests::fixtures;
use alloc::boxed::Box;
use jig_domain_core::{CoreRecord, Deployment, Record};
use skein_io::kernel::Fd;
use skein_lib::{Env, List, Queue, Time, Token, Wall};

const OWNER: Token = Token::new(1);
const ZERO: protocol::codec::component::Levels =
    protocol::codec::component::Levels { skein: Some(0), smith: Some(0), jig: Some(0), application: Some(0) };

fn component(limits: &protocol::Limits, stores: protocol::Stores) -> protocol::Store {
    let (application, _, _) = fixtures::store_limits();
    with_patch_sets(protocol::Store::over(
        limits,
        temper_protocol_store::application(&application.forge).expect("application sizes"),
        &[],
        stores,
    ))
}

fn ask(store: &mut protocol::Store, env: &Env<protocol::Limits>, request: protocol::Request) -> Box<[protocol::Event]> {
    let mut above = Queue::with_capacity(protocol::MAX_OUT_DOWN);
    let mut below = protocol::Below {
        store: Queue::with_capacity(skein_kv::MAX_OUT.below),
        secrets: Queue::with_capacity(skein_kv::MAX_OUT.below),
        transcripts: Queue::with_capacity(skein_kv::MAX_OUT.below),
    };
    protocol::down(store, env, request, &mut above, &mut below);
    assert!(below.store.is_empty() && below.secrets.is_empty() && below.transcripts.is_empty());
    let mut events = List::with_capacity(protocol::MAX_OUT_DOWN);
    while let Some(event) = above.pop() {
        events.push(event).expect("bounded events");
    }
    events.into_boxed()
}

fn open(store: &mut protocol::Store, env: &Env<protocol::Limits>, last: u64) {
    let events = ask(
        store,
        env,
        protocol::Request::Open {
            owner: OWNER,
            root: protocol::Root { token: OWNER, fd: Fd::new(1) },
            installation: fixtures::seeds().installation,
        },
    );
    let [protocol::Event::Opened { last: found, pending: false, .. }] = &*events else {
        panic!("ordinary baseline open: {events:?}")
    };
    assert_eq!(*found, last);
}

fn levels(stores: &mut protocol::Stores, limits: skein_kv::Limits) -> protocol::codec::component::Levels {
    let key = protocol::keys::encode_own(&protocol::keys::Own::Format, limits.key).expect("format key");
    let mut above = Queue::with_capacity(skein_kv::MAX_OUT.above);
    let mut below = Queue::with_capacity(skein_kv::MAX_OUT.below);
    skein_kv::down(
        &mut stores.store,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
        skein_kv::Request::Get { owner: OWNER, key },
        &mut above,
        &mut below,
    );
    let Some(skein_kv::Event::Got { value: Some(value), .. }) = above.pop() else { panic!("kept format") };
    assert!(above.is_empty() && below.is_empty());
    protocol::codec::component::Format::decode(&value).expect("format").levels
}

#[test]
fn four_owner_baselines_record_zero_and_reopen() {
    let (application, _, profile) = fixtures::store_limits();
    let limits = super::limits(&application, &application.forge, &profile).expect("checked store limits");
    let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
    let maps = protocol::Stores {
        store: skein_kv::Store::memory(&limits.store),
        secrets: skein_kv::Store::memory(&limits.secrets),
    };
    let mut store = component(&limits, maps);
    open(&mut store, &env, 0);
    let events = ask(
        &mut store,
        &env,
        protocol::Request::Commit {
            number: 1,
            writes: Box::new([protocol::Write::Save(Record::Core(CoreRecord::Deployment(Deployment {
                id: fixtures::seeds().deployment,
                projects: 0,
                tasks: 0,
                people: 0,
                sign_ins: 0,
                messages: 0,
                runs: 0,
                calls: 0,
                connector_rows: 0,
                applications: 0,
                commits: 1,
            })))]),
        },
    );
    let [protocol::Event::Committed { number: 1, .. }] = &*events else { panic!("first commit: {events:?}") };
    let mut maps = store.into_stores();
    assert_eq!(levels(&mut maps, limits.store), ZERO);
    let mut store = component(&limits, maps);
    open(&mut store, &env, 1);
    assert_eq!(levels(&mut store.into_stores(), limits.store), ZERO);
}
