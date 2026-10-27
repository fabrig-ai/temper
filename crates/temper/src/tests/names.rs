use temper_box::limits as box_limits;
use temper_forge_setup::limits as setup;
use temper_service::limits as service;
type Route = (&'static str, bool, bool, fn(&[u8], u64));

fn plan_and_host(name: &[u8], value: u64) {
    host(name, value);
    let small = setup::small();
    let mut copy = small.clone();
    assert_eq!(setup::apply(&mut copy, name, value), Ok(()));
    assert_eq!(copy, small);
}
fn host(name: &[u8], value: u64) {
    let small = setup::host();
    let mut copy = small;
    assert_eq!(setup::apply_host(&mut copy, name, value), Ok(()));
    assert_eq!(copy, small);
    let mut own = service::profile(service::Profile::Small);
    let before = own;
    assert_eq!(service::apply(&mut own, name, value), Ok(()));
    assert_eq!(own, before);
}
fn fetch(name: &[u8], value: u64) {
    let mut copy = setup::FETCH_BYTES;
    assert_eq!(setup::apply_fetch_bytes(&mut copy, name, value), Ok(()));
    assert_eq!(copy, setup::FETCH_BYTES);
}

#[test]
fn every_name_the_start_takes_is_a_row_routed_to_its_parser() {
    let names = service::NAMES.iter().chain(&setup::NAMES).chain(&box_limits::NAMES);
    for (index, row) in names.clone().enumerate() {
        for other in names.clone().skip(index + 1) {
            assert_ne!(row.name, other.name);
        }
    }
    let small = service::profile(service::Profile::Small);
    for row in &service::NAMES {
        let name = row.name.as_bytes();
        assert!(!service::handed_on(name), "{}", row.name);
        assert!(!service::host_setup(name), "{}", row.name);
        let value = service::value(&small, name).expect("service value");
        let mut copy = small;
        assert_eq!(service::apply(&mut copy, name, value), Ok(()), "{}", row.name);
        assert_eq!(copy, small, "{}", row.name);
    }
    // The task's routing contract, including the three host-only exceptions.
    let routes: &[Route] = &[
        ("setup.orgs", true, true, plan_and_host),
        ("setup.deadline_ms", true, true, plan_and_host),
        ("setup.backoff_ms", true, true, plan_and_host),
        ("setup.backoff_max_ms", true, true, plan_and_host),
        ("setup.runs", false, true, host),
        ("setup.states", false, true, host),
        ("setup.state_life_ms", false, true, host),
        ("setup.fetch_bytes", true, false, fetch),
    ];
    assert_eq!(setup::NAMES.len(), routes.len());
    for (row, (name, handed, host_setup, apply)) in setup::NAMES.iter().zip(routes) {
        assert_eq!(row.name, *name);
        let name = name.as_bytes();
        assert_eq!(service::handed_on(name), *handed, "{}", row.name);
        assert_eq!(service::host_setup(name), *host_setup, "{}", row.name);
        let value = setup::value(&setup::host(), setup::FETCH_BYTES, name).expect("setup value");
        apply(name, value);
    }
    let small = box_limits::small();
    for row in &box_limits::NAMES {
        let name = row.name.as_bytes();
        assert!(service::handed_on(name), "{}", row.name);
        assert!(!service::host_setup(name), "{}", row.name);
        let value = box_limits::value(&small, name).expect("box value");
        let mut copy = small;
        assert_eq!(box_limits::apply(&mut copy, name, value), Ok(()), "{}", row.name);
        assert_eq!(copy, small, "{}", row.name);
    }
}
