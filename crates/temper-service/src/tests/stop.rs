use super::fixtures;
use crate::{End, Stop};
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

fn refused(why: OpenRefusal, expected: Stop) {
    let mut service = fixtures::started();
    service
        .engine
        .as_mut()
        .expect("engine")
        .store
        .above
        .push(jig_protocol_store::Event::OpenRefused { owner: skein_lib::Token::new(1), why });
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
        Family::Component => u32::from(current.component),
        Family::Core => u32::from(current.core),
        Family::Tasks => u32::from(current.tasks),
        Family::People => u32::from(current.people),
        Family::Notes => u32::from(current.notes),
        Family::Accounts => u32::from(current.accounts),
        Family::Application => current.application,
    };
    refused(
        OpenRefusal::Format { family, found: wanted.checked_add(1).expect("next format version"), wanted },
        Stop::StoreVersion,
    );
}

#[test]
fn another_component_version_stops_startup() {
    version(Family::Component);
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
fn non_format_open_refusals_keep_the_deployment_problem() {
    for why in [OpenRefusal::Deployment, OpenRefusal::Numbers { store: 9, header: 8 }, OpenRefusal::Failed] {
        let mut problems = jig_protocol_config::Problems::empty();
        problems.push(jig_protocol_config::Path::root().member(b"deployment"), jig_protocol_config::What::Refers);
        refused(why, Stop::Start(problems));
    }
}

#[test]
fn stores_before_embedded_people_v13_refuse_at_startup() {
    // Store 6 named a repository by its id alone; 7 has no Binding family;
    // 8 embeds people v11 refusal tags.
    let wanted = temper_protocol_store::STORE_VERSION;
    assert_eq!(wanted, 10);
    for found in [6, 7, 8] {
        refused(OpenRefusal::Format { family: Family::Application, found, wanted }, Stop::StoreVersion);
    }
}

#[test]
fn a_store_without_demo_acts_refuses_at_startup() {
    let wanted = temper_protocol_store::STORE_VERSION;
    assert_eq!(wanted, 10);
    refused(OpenRefusal::Format { family: Family::Application, found: 9, wanted }, Stop::StoreVersion);
}
