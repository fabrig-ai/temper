use crate::limits;
// The task's four plan names and seven host names precede the fetch row.
const PLAN_NAMES: usize = 4;
const HOST_NAMES: usize = 7;

#[test]
fn every_row_is_a_name_apply_takes_at_its_small_value() {
    let small = limits::host();
    for row in &limits::NAMES[..HOST_NAMES] {
        let value = limits::value(&small, limits::FETCH_BYTES, row.name.as_bytes()).expect("host value");
        let mut copy = small;
        assert_eq!(limits::apply_host(&mut copy, row.name.as_bytes(), value), Ok(()), "{}", row.name);
        assert_eq!(copy, small, "{}", row.name);
    }
}
#[test]
fn every_plan_row_applies_its_small_value_without_change() {
    let small = limits::small();
    for row in &limits::NAMES[..PLAN_NAMES] {
        let value = limits::value(&limits::host(), limits::FETCH_BYTES, row.name.as_bytes()).expect("plan value");
        let mut copy = small.clone();
        assert_eq!(limits::apply(&mut copy, row.name.as_bytes(), value), Ok(()), "{}", row.name);
        assert_eq!(copy, small, "{}", row.name);
    }
}
#[test]
fn the_fetch_row_applies_its_small_value_without_change() {
    let name = b"setup.fetch_bytes";
    let value = limits::value(&limits::host(), limits::FETCH_BYTES, name).expect("fetch value");
    let mut fetch = limits::FETCH_BYTES;
    assert_eq!(limits::apply_fetch_bytes(&mut fetch, name, value), Ok(()));
    assert_eq!(fetch, limits::FETCH_BYTES);
}
#[test]
fn a_name_with_no_row_is_refused() {
    let name = b"setup.unknown";
    let mut host = limits::host();
    let mut plan = limits::small();
    let mut fetch = limits::FETCH_BYTES;
    assert_eq!(limits::apply_host(&mut host, name, u64::MAX), Err(limits::Refused::Unknown));
    assert_eq!(limits::apply(&mut plan, name, u64::MAX), Err(limits::Refused::Unknown));
    assert_eq!(limits::apply_fetch_bytes(&mut fetch, name, u64::MAX), Err(limits::Refused::Unknown));
    assert_eq!(limits::value(&host, fetch, name), None);
    assert_eq!(host, limits::host());
    assert_eq!(plan, limits::small());
    assert_eq!(fetch, limits::FETCH_BYTES);
}
#[test]
fn checked_keeps_refusing_the_three_host_only_names() {
    use super::fixtures;
    for name in ["setup.runs", "setup.states", "setup.state_life_ms"] {
        let problems = crate::check::checked(
            &fixtures::document(fixtures::TWO),
            0,
            &jig_protocol_config::Directories::working(),
            &fixtures::members(),
            None,
            &[(name.as_bytes().into(), 1)],
        )
        .expect_err("host-only input is not a plan override");
        let mut expected = crate::problem::Problems::empty();
        expected.push(
            jig_protocol_config::Path::root()
                .member(b"application")
                .member(b"limits")
                .member(b"overrides")
                .member(name.as_bytes()),
            crate::problem::What::Reason(b"no limit of temper's has this name"),
        );
        assert_eq!(problems, expected);
    }
}

#[test]
fn rows_are_unique_and_well_formed() {
    use jig_protocol_config::names::Unit;
    const PREFIXES: [&str; 1] = ["setup."];
    let byte_names: &[&str] = &[];
    let mut groups = [false; PREFIXES.len()];
    let mut previous = 0;
    for (index, row) in limits::NAMES.iter().enumerate() {
        for other in limits::NAMES.iter().skip(index + 1) {
            assert_ne!(row.name, other.name);
        }
        let mut prefix = None;
        for (index, candidate) in PREFIXES.iter().enumerate() {
            if row.name.starts_with(candidate) {
                prefix = Some(index);
            }
        }
        let prefix = prefix.expect("owner prefix");
        assert!(prefix >= previous, "{}", row.name);
        previous = prefix;
        groups[prefix] = true;
        for byte in row.name.bytes() {
            assert!(byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_.".contains(&byte));
        }
        let unit = if row.name.ends_with("_ms") {
            Unit::Milliseconds
        } else if row.name.ends_with("_bytes")
            || row.name.as_bytes().ends_with(b".bytes")
            || byte_names.contains(&row.name)
        {
            Unit::Bytes
        } else {
            Unit::Count
        };
        assert_eq!(row.unit, unit, "{}", row.name);
    }
    for seen in groups {
        assert!(seen);
    }
}
#[test]
fn each_line_is_one_sentence_of_at_most_160_bytes() {
    const LINE_BYTES: usize = 160; // The names table's documentation bound.
    for row in &limits::NAMES {
        assert!(!row.line.is_empty(), "{}", row.name);
        assert!(row.line.len() <= LINE_BYTES, "{}", row.name);
        assert!(!row.line.contains(['\r', '\n']), "{}", row.name);
        assert!(row.line.ends_with('.'), "{}", row.name);
    }
}

#[test]
fn the_resolved_service_input_stays_outside_the_setup_rows() {
    const REPOSITORIES: u64 = 3; // A resolved capacity different from the plan's small value of two.
    let name = b"forge.client.repositories";
    let small = limits::host();
    let mut copy = small;
    let mut plan = limits::small();
    assert_eq!(limits::apply_host(&mut copy, name, REPOSITORIES), Ok(()));
    assert_eq!(copy, small);
    assert_eq!(limits::apply(&mut plan, name, REPOSITORIES), Ok(()));
    assert_eq!(u64::from(plan.repositories), REPOSITORIES);
    assert_eq!(limits::value(&copy, limits::FETCH_BYTES, name), None);
}
