use crate::limits::{self, Refused, Unfit};
use skein_lib::Duration;
#[test]
fn the_small_profile_has_the_values_of_the_plan() {
    assert_eq!(
        limits::small(),
        limits::Limits {
            backoff: Duration::from_millis(500),
            backoff_max: Duration::from_secs(30),
            restarts: 5,
            window: Duration::from_secs(300),
            line_bytes: 256,
            log_bytes: 4_194_304,
            tail_lines: 10,
            grace: Duration::from_secs(10)
        }
    );
    assert_eq!(limits::check(&limits::small()), Ok(()));
    assert!(limits::worst_case(&limits::small(), 6).expect("fits") > 0, "state is priced");
}
#[test]
fn a_backoff_above_its_maximum_names_both_limits() {
    let limits = limits::Limits { backoff: Duration::from_secs(31), ..limits::small() };
    assert_eq!(limits::check(&limits), Err(Unfit::Backoff));
    assert_eq!(Unfit::Backoff.names(), &[b"box.backoff_ms".as_slice(), b"box.backoff_max_ms"]);
}
#[test]
fn a_maximum_not_below_the_window_names_both_limits() {
    let limits = limits::Limits { window: Duration::from_secs(30), ..limits::small() };
    assert_eq!(limits::check(&limits), Err(Unfit::Window));
    assert_eq!(Unfit::Window.names(), &[b"box.backoff_max_ms".as_slice(), b"box.window_ms"]);
}
#[test]
fn restarts_above_sixty_four_name_their_limit() {
    let limits = limits::Limits { restarts: 65, ..limits::small() };
    assert_eq!(limits::check(&limits), Err(Unfit::Restarts));
    assert_eq!(Unfit::Restarts.names(), &[b"box.restarts".as_slice()]);
}
#[test]
fn a_name_that_is_no_box_limit_is_unknown() {
    for name in [b"backoff_ms".as_slice(), b"box.nope", b"setup.restarts"] {
        assert_eq!(limits::apply(&mut limits::small(), name, 1), Err(Refused::Unknown), "{name:?}");
    }
}
#[test]
fn a_value_past_u32_is_out_of_range() {
    for name in [
        b"box.backoff_ms".as_slice(),
        b"box.backoff_max_ms",
        b"box.restarts",
        b"box.window_ms",
        b"box.grace_ms",
        b"box.line_bytes",
        b"box.log_bytes",
        b"box.tail_lines",
    ] {
        assert_eq!(limits::apply(&mut limits::small(), name, 4_294_967_296), Err(Refused::Range), "{name:?}");
        assert_eq!(limits::apply(&mut limits::small(), name, 0), Ok(()), "zero fits {name:?}");
    }
}

#[test]
fn pipe_lines_need_at_least_sixty_four_bytes_after_other_checks() {
    for (value, expected) in [(0, Err(Unfit::LineBytes)), (63, Err(Unfit::LineBytes)), (64, Ok(())), (256, Ok(()))] {
        let mut limits = limits::small();
        limits::apply(&mut limits, b"box.line_bytes", value).unwrap();
        assert_eq!(limits::check(&limits), expected);
    }
    assert_eq!(Unfit::LineBytes.names(), &[b"box.line_bytes".as_slice()]);
    let limits = limits::Limits { line_bytes: 0, restarts: 65, ..limits::small() };
    assert_eq!(limits::check(&limits), Err(Unfit::Restarts));
}

#[test]
fn logs_hold_a_whole_record_and_tails_hold_one_to_a_hundred_lines() {
    use limits::{MAX_TAIL_LINES, RECORD_BYTES};
    assert_eq!(RECORD_BYTES, 4118);
    for (value, expected) in [(RECORD_BYTES - 1, Err(Unfit::LogBytes)), (RECORD_BYTES, Ok(()))] {
        let mut limits = limits::small();
        limits::apply(&mut limits, b"box.log_bytes", u64::from(value)).unwrap();
        assert_eq!(limits::check(&limits), expected);
    }
    assert_eq!(Unfit::LogBytes.names(), &[b"box.log_bytes".as_slice()]);
    for (value, expected) in
        [(0, Err(Unfit::TailLines)), (1, Ok(())), (MAX_TAIL_LINES, Ok(())), (101, Err(Unfit::TailLines))]
    {
        let mut limits = limits::small();
        limits::apply(&mut limits, b"box.tail_lines", u64::from(value)).unwrap();
        assert_eq!(limits::check(&limits), expected);
    }
    assert_eq!(Unfit::TailLines.names(), &[b"box.tail_lines".as_slice()]);
}

#[test]
fn every_row_is_a_name_apply_takes_at_its_small_value() {
    let small = limits::small();
    for row in &limits::NAMES {
        let value = limits::value(&small, row.name.as_bytes()).expect("every row has a value");
        let mut copy = small;
        assert_eq!(limits::apply(&mut copy, row.name.as_bytes(), value), Ok(()), "{}", row.name);
        assert_eq!(copy, small, "{}", row.name);
    }
}

#[test]
fn a_name_with_no_row_is_refused() {
    let small = limits::small();
    for name in ["box.unknown"] {
        let mut copy = small;
        assert_eq!(limits::apply(&mut copy, name.as_bytes(), u64::MAX), Err(limits::Refused::Unknown));
        assert_eq!(limits::value(&copy, name.as_bytes()), None);
        assert_eq!(copy, small);
    }
}

#[test]
fn rows_are_unique_and_well_formed() {
    use jig_protocol_config::names::Unit;
    const PREFIXES: [&str; 1] = ["box."];
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
