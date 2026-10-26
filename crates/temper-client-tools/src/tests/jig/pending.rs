use crate::{jig::render, tests::fixtures};
#[test]
fn a_pending_row_reads_operation_party_not_sent() {
    let printed = render::pending::pending(fixtures::kept(1).as_slice(), 0, false);
    assert_eq!(&*printed.stdout, b"stop, party 73, not sent, unanswered: it clears when the engine answers it, or past its retention; pending lists it\n");
}
#[test]
fn pending_renders_first_sent() {
    use skein_lib::Wall;
    const SENT: u64 = 3_660_000_000_000; // A nonzero hour and minute expose UTC formatting and nanosecond JSON.
    let mut rows = fixtures::kept(1).into_boxed();
    rows[0].first_sent = Some(Wall::from_nanos(SENT));
    for (json, expected) in [
        (false, b"stop, party 73, first sent 01:01 UTC, unanswered: it clears when the engine answers it, or past its retention; pending lists it\n".as_slice()),
        (true, br#"{"pending":[{"operation":"stop","party":73,"first_sent":3660000000000,"unanswered":"it clears when the engine answers it, or past its retention; pending lists it"}],"unread":0}"#),
    ] {
        assert_eq!(render::pending::pending(&rows, 0, json).stdout.as_ref(), expected, "json {json}");
    }
}
