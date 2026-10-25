use crate::lines::{Lines, Output, route};
use skein_lib::{Wall, bytes};
const READ: Wall = Wall::from_nanos(12_000_000_000);
const EOF: Wall = Wall::from_nanos(19_000_000_000);
fn line(bytes: &[u8], wall: Wall) -> Output {
    Output::Line { bytes: bytes::copy_of(bytes), wall }
}
fn cap() -> Box<[u8]> {
    super::fixtures::line_cap()
}
#[test]
fn diagnostics_are_named_and_every_cap_or_eof_piece_gets_lf() {
    let mut lines = Lines::new(false);
    lines.push(bytes::copy_of(b"first\nsecond\n"), READ);
    assert_eq!(lines.take(b"forgejo"), Some(Output::Piece { bytes: bytes::copy_of(b"first\n"), wall: READ }));
    assert_eq!(lines.take(b"forgejo"), Some(Output::Piece { bytes: bytes::copy_of(b"second\n"), wall: READ }));
    lines.push(cap(), READ);
    let expected = cap();
    assert_eq!(lines.take(b"runner"), Some(Output::Piece { bytes: expected, wall: READ }));
    lines.push(bytes::copy_of(b"tail"), READ);
    assert_eq!(lines.take(b"runner"), None);
    lines.end(EOF);
    assert_eq!(lines.take(b"runner"), Some(Output::Piece { bytes: bytes::copy_of(b"tail"), wall: EOF }));
    assert_eq!(lines.take(b"runner"), None);
}
#[test]
fn split_links_wait_for_lf_or_eof_and_keep_completion_wall() {
    let mut lines = Lines::new(true);
    lines.push(bytes::copy_of(b"http://split"), READ);
    assert_eq!(lines.take(b"serve"), None);
    lines.push(bytes::copy_of(b"/link\nnext\nlast"), EOF);
    assert_eq!(lines.take(b"serve"), Some(line(b"http://split/link\n", EOF)));
    assert_eq!(lines.take(b"serve"), Some(line(b"next\n", EOF)));
    assert_eq!(lines.take(b"serve"), None);
    lines.end(READ); // A backward wall jump is still the supplied EOF time.
    assert_eq!(lines.take(b"serve"), Some(line(b"last\n", READ)));
    assert!(lines.is_empty());
}
#[test]
fn exactly_4096_payload_bytes_are_accepted_with_lf_and_eof() {
    for eof in [false, true] {
        let mut lines = Lines::new(true);
        lines.push(cap(), READ);
        assert_eq!(lines.take(b"serve"), None, "payload cap alone is not completion");
        if eof {
            lines.end(EOF);
        } else {
            lines.push(bytes::copy_of(b"\n"), EOF);
        }
        let expected = crate::lines::joined(&[&cap(), b"\n"]);
        assert_eq!(lines.take(b"serve"), Some(line(&expected, EOF)));
        assert_eq!(lines.take(b"serve"), None);
    }
}
#[test]
fn overflow_discards_all_bytes_once_through_lf_or_eof_and_recovers() {
    for (extra_chunks, eof) in [(0_u32, false), (2_u32, false), (0_u32, true), (2_u32, true)] {
        let mut lines = Lines::new(true);
        lines.push(cap(), READ);
        assert_eq!(lines.take(b"serve"), None);
        lines.push(bytes::copy_of(b"x"), READ); // 4097: no prefix or tail escapes.
        assert_eq!(lines.take(b"serve"), None);
        for _ in 0..extra_chunks {
            lines.push(cap(), READ);
            assert_eq!(lines.take(b"serve"), None);
        }
        if eof {
            lines.end(EOF);
        } else {
            lines.push(bytes::copy_of(b"refused-tail\ngood\n"), EOF);
        }
        assert_eq!(lines.take(b"serve"), Some(Output::Refused { wall: EOF }));
        assert_eq!(lines.take(b"serve"), if eof { None } else { Some(line(b"good\n", EOF)) });
        assert!(lines.is_empty());
    }
}
#[test]
fn held_legacy_first_piece_keeps_cap_transfer_and_residual_tail() {
    let mut lines = Lines::new(true);
    lines.hold();
    lines.push(cap(), READ);
    assert_eq!(lines.first(), Some(cap()));
    lines.push(bytes::copy_of(b"tail\nnext\n"), EOF);
    assert_eq!(lines.take(b"serve"), Some(line(b"tail\n", EOF)));
    assert_eq!(lines.take(b"serve"), Some(line(b"next\n", EOF)));
    let mut incomplete = Lines::new(true);
    incomplete.hold();
    incomplete.push(bytes::copy_of(b"half"), READ);
    assert_eq!(incomplete.first(), None);
    incomplete.end(EOF);
    assert_eq!(incomplete.first(), None, "EOF does not start the legacy link step");
    assert_eq!(incomplete.take(b"serve"), Some(Output::Legacy { bytes: bytes::copy_of(b"half") }));
}
#[test]
fn only_the_serve_steps_stdout_is_plain() {
    for (step, fd, expected) in [(b"serve".as_slice(), 1_u32, true), (b"serve", 2, false), (b"runner", 1, false)] {
        assert_eq!(route(b"serve", step, fd), expected);
    }
}
