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
