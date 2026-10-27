#![expect(
    clippy::disallowed_types,
    clippy::disallowed_macros,
    clippy::disallowed_methods,
    reason = "ordinary diagnostic fixtures render exact expected text"
)]
mod pre_settings;
use super::fixtures;
use crate::{End, Stop};
use core::fmt::Write as _;
use jig_protocol_store::{Family, OpenRefusal};
#[test]
fn a_signal_drains_and_a_second_aborts() {
    let mut service = fixtures::ready();
    service.shutdown();
    assert_eq!(service.end(), None, "nothing has closed yet");
    service.shutdown();
    assert_eq!(service.end(), Some(&End::Aborted));
}
#[test]
fn a_drain_closes_accounts_then_the_store() {
    let mut service = fixtures::ready();
    service.shutdown();
    for _ in 0_u32..256 {
        fixtures::turn(&mut service);
        if service.end().is_some() {
            break;
        }
    }
    assert_eq!(service.end(), Some(&End::Drained));
    assert!(service.engine.as_ref().expect("engine").accounts.as_ref().unwrap().is_empty());
    assert!(service.engine.as_ref().expect("engine").store.closed());
}
#[test]
fn signals_routed_from_io_drain_then_abort() {
    let mut service = fixtures::ready();
    service.told.push(skein_io::Event::Shutdown { signal: skein_io::kernel::ServiceSignal::Terminate, from: None });
    fixtures::turn(&mut service);
    assert_eq!(service.end(), None);
    service.told.push(skein_io::Event::Shutdown { signal: skein_io::kernel::ServiceSignal::Terminate, from: None });
    fixtures::turn(&mut service);
    assert_eq!(service.end(), Some(&End::Aborted));
    for _ in 0_u32..16 {
        fixtures::turn(&mut service);
        if service.is_empty() {
            break;
        }
    }
    assert!(service.is_empty());
}

fn refused(why: OpenRefusal) {
    let expected = Stop::StoreOpen { why: why.clone() };
    let mut service = fixtures::started();
    service
        .engine
        .as_mut()
        .expect("engine")
        .store
        .above
        .push(jig_protocol_store::Event::OpenRefused { owner: skein_lib::Token::new(1), why: why.clone() });
    crate::engine::iterate(
        service.engine.as_mut().expect("engine"),
        skein_lib::Time::from_nanos(1),
        skein_lib::Wall::EPOCH,
    );
    assert_eq!(service.end(), Some(&End::Stopped(expected)), "{why:?}");
}
fn version(family: Family) {
    let current =
        jig_protocol_store::format::current(temper_protocol_store::STORE_VERSION, fixtures::seeds().deployment);
    let wanted = match family {
        Family::Settings => u32::from(current.settings),
        Family::Component => u32::from(current.component),
        Family::Core => u32::from(current.core),
        Family::Tasks => u32::from(current.tasks),
        Family::People => u32::from(current.people),
        Family::Notes => u32::from(current.notes),
        Family::Accounts => u32::from(current.accounts),
        Family::Application => current.application,
        Family::Secrets | Family::SkeinOauth | Family::SmithTranscript => {
            unreachable!("kept families are checked by layout")
        }
    };
    refused(OpenRefusal::Format { family, found: wanted.checked_add(1).expect("next format version"), wanted });
}

#[test]
fn another_component_version_stops_startup() {
    version(Family::Component);
}

#[test]
fn a_refused_start_closes_every_child_without_an_external_wakeup() {
    let mut service = fixtures::started();
    service
        .engine
        .as_mut()
        .unwrap()
        .store
        .above
        .push(jig_protocol_store::Event::OpenRefused { owner: skein_lib::Token::new(1), why: OpenRefusal::Failed });
    for _ in 0_u32..64 {
        fixtures::turn(&mut service);
    }
    let engine = service.engine.as_ref().unwrap();
    assert!(
        service.is_empty(),
        "end {:?}, setup {} {} {}, forge {} {} {}, store {}, workers {}, clients {} {} {}, accounts {}",
        engine.end,
        engine.setup.closed,
        engine.setup.io.len(),
        engine.setup.above.len(),
        engine.forge.closed,
        engine.forge.io.len(),
        engine.forge.above.len(),
        engine.store.closed(),
        engine.workers.closed(),
        engine.clients.closed,
        engine.clients.io.len(),
        engine.clients.early.len(),
        engine.accounts.as_ref().is_none_or(crate::accounts::Accounts::is_empty)
    );
}

#[test]
fn a_refused_one_process_start_closes_its_unstarted_worker() {
    for boxed in [false, true] {
        let mut stand = fixtures::system::one_process_under_box(boxed);
        stand
            .service
            .engine
            .as_mut()
            .unwrap()
            .store
            .above
            .push(jig_protocol_store::Event::OpenRefused { owner: skein_lib::Token::new(1), why: OpenRefusal::Failed });
        for _ in 0_u32..256 {
            stand.turn();
        }
        let engine = stand.service.engine.as_ref().unwrap();
        assert!(
            stand.service.is_empty(),
            "work {}, end {:?}, setup {} {} {}, forge {} {} {}, store {}, workers {}, clients {} {} {}, accounts {}",
            stand.service.work_pending(skein_lib::Time::ZERO),
            engine.end,
            engine.setup.closed,
            engine.setup.io.len(),
            engine.setup.above.len(),
            engine.forge.closed,
            engine.forge.io.len(),
            engine.forge.above.len(),
            engine.store.closed(),
            engine.workers.closed(),
            engine.clients.closed,
            engine.clients.io.len(),
            engine.clients.early.len(),
            engine.accounts.as_ref().is_none_or(crate::accounts::Accounts::is_empty)
        );
    }
}

#[test]
fn another_core_version_stops_startup() {
    version(Family::Core);
}

#[test]
fn another_tasks_version_stops_startup() {
    version(Family::Tasks);
}

#[test]
fn another_people_version_stops_startup() {
    version(Family::People);
}

#[test]
fn another_notes_version_stops_startup() {
    version(Family::Notes);
}

#[test]
fn another_accounts_version_stops_startup() {
    version(Family::Accounts);
}

#[test]
fn another_application_version_stops_startup() {
    version(Family::Application);
}

#[test]
fn non_format_open_refusals_preserve_their_own_reason() {
    use jig_protocol_store::{Check, Generations, Older, Owner};
    for why in [
        OpenRefusal::Deployment,
        OpenRefusal::Numbers { store: 9, header: 8 },
        OpenRefusal::Failed,
        OpenRefusal::OlderBuild { why: Older::Unleveled },
        OpenRefusal::OlderBuild { why: Older::Unrecorded },
        OpenRefusal::OlderBuild { why: Older::Layout(Family::Application) },
        OpenRefusal::OlderBuild { why: Older::Directory },
        OpenRefusal::Generations { why: Generations::Busy },
        OpenRefusal::Ahead { owner: Owner::Jig, found: Some(2), wanted: Some(1) },
        OpenRefusal::Patch {
            owner: Owner::Application,
            patch: 2,
            key: b"record".as_slice().into(),
            halt: skein_codec::patch::Halt::NoSite(skein_codec::patch::Site::new(2, 0)),
        },
        OpenRefusal::Set { owner: Owner::Jig, patch: 2 },
        OpenRefusal::Check { why: Check::Load { key: b"record".as_slice().into() } },
    ] {
        refused(why);
    }
}

#[test]
fn stores_before_embedded_people_v13_refuse_at_startup() {
    // Store 6 named a repository by its id alone; 7 has no Binding family;
    // 8 embeds people v11 refusal tags.
    let wanted = temper_protocol_store::STORE_VERSION;
    assert_eq!(wanted, 10);
    for found in [6, 7, 8] {
        refused(OpenRefusal::Format { family: Family::Application, found, wanted });
    }
}

#[test]
fn a_store_without_demo_acts_refuses_at_startup() {
    let wanted = temper_protocol_store::STORE_VERSION;
    assert_eq!(wanted, 10);
    refused(OpenRefusal::Format { family: Family::Application, found: 9, wanted });
}

#[test]
fn unfinished_forge_writes_stay_with_the_drain() {
    let mut service = fixtures::ready();
    assert!(service.unsettled().is_empty());
    service.shutdown();
    let writes =
        [temper_engine_domain::Unsettled { entry: 7, task: 3 }, temper_engine_domain::Unsettled { entry: 9, task: 5 }];
    for write in writes {
        crate::route::request(
            service.engine.as_mut().expect("engine"),
            temper_engine_domain::Request::Unsettled(write),
        );
    }
    for _ in 0_u32..256 {
        fixtures::turn(&mut service);
        if service.end().is_some() {
            break;
        }
    }
    assert_eq!(service.end(), Some(&End::Drained));
    assert_eq!(service.unsettled(), &writes);
}

#[test]
fn a_second_signal_while_settling_aborts() {
    use skein_forge::api::{Answer, Check, Repository, Write};
    use skein_forge_client::{Condition, Effect, Entry, Event as Client};
    use skein_lib::Token;
    use temper_engine_domain as root;
    use temper_engine_domain_forge::{Event, Stored};
    let mut service = fixtures::ready();
    let entry = Entry {
        number: 7,
        task: 3,
        repository: Repository { forge: 1, repository: 2 },
        effect: Effect {
            write: Write::Status { commit: [3; 32], context: b"stop".as_slice().into(), check: Check::Passed },
            condition: Condition::None,
        },
        start: None,
        attempt: None,
        failures: 0,
        queued: false,
    };
    // Restore the connector-owned entry, then play its committed handoff to the client.
    for event in [
        Event::Connection {
            forge: 1,
            state: temper_engine_domain_forge::Connection::Connected { id: 7, login: b"writer".as_slice().into() },
        },
        Event::Restore { record: Stored::Entry { entry: entry.clone(), held: false } },
        Event::Client(Client::Make { entry }),
    ] {
        service.engine.as_mut().unwrap().step(root::Event::Forge(Box::new(event)));
    }
    for _ in 0_u32..16 {
        fixtures::turn(&mut service);
    }
    // The clock lookup is the first client call. The connection's write answer
    // is held by leaving its IO completions with the fixture.
    service.engine.as_mut().unwrap().step(root::Event::Forge(Box::new(Event::Client(Client::Answered {
        call: Token::new(1),
        cost: 1,
        free: 0,
        rate: None,
        result: Ok(Answer::Items { items: Box::new([]), more: false, now: skein_lib::Time::from_nanos(1) }),
    }))));
    for _ in 0_u32..16 {
        fixtures::turn(&mut service);
    }
    service.shutdown();
    for _ in 0_u32..16 {
        fixtures::turn(&mut service);
    }
    assert_eq!(service.end(), None);
    assert!(!service.engine.as_ref().unwrap().drain.as_ref().unwrap().closing, "still settling, before the drain");
    service.shutdown();
    assert_eq!(service.end(), Some(&End::Aborted));
}

// Three named writes, and jig's protocol/store.md, 10.3's 64-key cap.
const FLIGHT_KEYS: usize = 64;
const FIRST_WRITE: (u64, u64) = (7, 11);
const SECOND_WRITE: (u64, u64) = (9, 13);
const THIRD_WRITE: (u64, u64) = (17, 19);
const FLIGHT_REMEDY: &str = " is still in flight; start the build before this one and stop it to settle it\n";
fn flight_refusal(keys: Box<[Box<[u8]>]>, count: u32) -> OpenRefusal {
    OpenRefusal::InFlight { owner: jig_protocol_store::Owner::Application, family: "temper", count, keys }
}
fn flight_key(task: u64, entry: u64) -> Box<[u8]> {
    temper_protocol_store::keys::encode(&temper_engine_domain_forge::Key::Flight { task, entry }, u32::MAX)
        .expect("fixed flight key")
}
fn refusal_words(why: &OpenRefusal) -> String {
    format!("temper serve: {}\n", why.words().join("state"))
}
#[test]
fn an_in_flight_refusal_names_each_write() {
    const WRITES: u32 = 3; // The three keys below.
    let why = flight_refusal(
        Box::new([
            flight_key(FIRST_WRITE.0, FIRST_WRITE.1),
            flight_key(SECOND_WRITE.0, SECOND_WRITE.1),
            flight_key(THIRD_WRITE.0, THIRD_WRITE.1),
        ]),
        WRITES,
    );
    let expected = refusal_words(&why)
        + "temper serve: task 7's forge write 11 is still in flight; start the build before this one and stop it to settle it\n"
        + "temper serve: task 9's forge write 13 is still in flight; start the build before this one and stop it to settle it\n"
        + "temper serve: task 17's forge write 19 is still in flight; start the build before this one and stop it to settle it\n";
    assert_eq!(crate::refused_start(&why, b"state"), (expected.into_bytes().into_boxed_slice(), 1));
    refused(why);
}
#[test]
fn an_in_flight_refusal_names_64_keys_and_counts_the_rest() {
    const COUNT: u32 = 70; // 64 retained keys and six more (E2 acceptance).
    for retained in [FLIGHT_KEYS, FLIGHT_KEYS + 1] {
        let keys = (0..retained).map(|entry| flight_key(FIRST_WRITE.0, u64::try_from(entry).unwrap())).collect();
        let why = flight_refusal(keys, COUNT);
        let mut expected = refusal_words(&why);
        for entry in 0..FLIGHT_KEYS {
            write!(expected, "temper serve: task 7's forge write {entry}{FLIGHT_REMEDY}").unwrap();
        }
        expected.push_str("temper serve: and 6 more forge writes in flight\n");
        assert_eq!(crate::refused_start(&why, b"state"), (expected.into_bytes().into_boxed_slice(), 1));
    }
}
#[test]
fn an_unknown_flight_key_is_named_in_lowercase_hex() {
    let why = flight_refusal(Box::new([Box::from([0, 21, 255])]), 1);
    let expected = refusal_words(&why) + "temper serve: forge write key 0x0015ff" + FLIGHT_REMEDY;
    assert_eq!(crate::refused_start(&why, b"state"), (expected.into_bytes().into_boxed_slice(), 1));
}

#[test]
fn unresolved_forge_records_keep_the_named_start_failure() {
    use temper_engine_domain::{Record, Write};
    use temper_engine_domain_forge::Stored;
    const ABSENT_TASK: u64 = 999;
    // Same completed-restart horizon as the service's startup stories.
    const PASSES: u32 = 256;
    let mut previous = fixtures::ready().engine.expect("running engine");
    fixtures::commit(
        &mut previous.store,
        1,
        Box::new([Write::Save(Record::Forge {
            row: Box::new(Stored::Names { task: ABSENT_TASK, resources: Box::new([]) }),
        })]),
    );
    drop(previous.store.above.pop().expect("fixture commit answer"));
    let mut service = fixtures::restarting_from(previous);
    for _ in 0..PASSES {
        fixtures::turn(&mut service);
        if service.end().is_some() {
            break;
        }
    }
    assert_eq!(service.end(), Some(&End::Stopped(Stop::ForgeUnresolved { records: 1 })));
    let engine = service.engine.as_ref().expect("refused engine");
    assert!(!engine.store.component.mark_durable(), "no start mark on refusal");
    assert!(!engine.domain.connectors_restored(), "no adoption on refusal");
}
