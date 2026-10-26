//! Account moves are exercised through the CLI's file and HTTP boundaries.
use super::fixtures::{
    self, Frame,
    moves::{Answer, NUMBER, RECORD},
};
use jig_client_protocol_wire as w;
use skein_io::kernel::{Error, Exit, Kind, OpenHow, ServiceSignal, Stat};
const EXPORT: &[&[u8]] = &[b"export", b"7", b"/linked-parent/record"];
const IMPORT: &[&[u8]] = &[b"import", b"/linked-parent/record", b"--key", b"move-in"];
const LOST: &[u8] = b"temper ctl: the record of the move was lost on its way; the engine keeps the account and cancels the move at its deadline; export again after it\n";
const AT_BOUND: &[u8] = &[0; 131_072]; // tools.md, 12: 128 KiB whole.
const OVER_BOUND: &[u8] = &[b'r'; 131_073];
fn exporting(answers: impl IntoIterator<Item = Answer>) -> Frame {
    let mut f = Frame::act(EXPORT);
    f.move_answers.extend(answers);
    f.sign_in_then_deliver_me();
    f
}
fn importing(file: &'static [u8], answers: impl IntoIterator<Item = Answer>) -> Frame {
    let mut f = Frame::act(IMPORT);
    f.link = file;
    f.move_answers.extend(answers);
    f.sign_in_then_deliver_me();
    f
}
fn done(f: &Frame, code: u8, removed: bool) {
    assert_eq!(f.exit(), Some(Exit::Code(code)));
    assert_eq!(f.record_removed, removed);
    f.no_record_leaked();
}
#[test]
fn an_export_refuses_a_file_already_there_and_sends_nothing() {
    for kind in [Kind::File, Kind::Symlink] {
        let mut f = Frame::act(EXPORT);
        f.record_existing_kind = Some(kind);
        f.record_data = b"existing file or link target".to_vec();
        f.drain();
        done(&f, 1, false);
        assert_eq!(f.requests(), 0);
        assert!(!f.opened(b"directory"));
        assert_eq!(f.record_data, b"existing file or link target");
        assert_eq!(f.stderr(), b"temper ctl: /linked-parent/record exists\n");
    }
}
#[test]
fn an_export_writes_the_record_private_and_synced_before_written() {
    let f = exporting([Answer::Moving(RECORD), Answer::Done]);
    done(&f, 0, false);
    assert_eq!(f.record_how, Some(OpenHow::CreateNoFollow { mode: Some(0o600) }));
    assert_eq!(f.record_root, Some(fixtures::FILE_PARENT));
    assert_eq!(f.record_data, RECORD);
    assert_eq!(f.file_steps, ["create", "move-out", "write", "sync-file", "sync-directory", "written"]);
    assert_eq!(f.stdout(), b"moved out: account 7 to /linked-parent/record\n");
    assert_eq!(f.stderr(), b"");
    let (holder, w::AccountChange::Written(written)) = &f.move_requests[1] else { panic!("written") };
    assert_eq!(*holder, w::AccountHolder::Own);
    assert_eq!(written.number(), NUMBER);
    assert_ne!(f.act_keys()[0], f.act_keys()[1]);
}
#[test]
fn an_export_refused_by_the_engine_removes_the_file() {
    for (why, line) in [
        (w::AccountMoveWhy::NotSignedIn, b"export: not signed in\n".as_slice()),
        (w::AccountMoveWhy::Moving, b"export: moving\n"),
    ] {
        let f = exporting([Answer::Refused(why)]);
        done(&f, 2, true);
        assert_eq!(f.stderr(), line);
        assert_eq!(f.move_requests.len(), 1);
    }
}
#[test]
fn a_written_refused_cancelled_removes_the_file() {
    let f = exporting([Answer::Moving(RECORD), Answer::Refused(w::AccountMoveWhy::Cancelled)]);
    done(&f, 2, true);
    assert_eq!(f.stderr(), b"export: cancelled\n");
}
#[test]
fn an_export_whose_write_fails_removes_the_file_and_sends_no_written() {
    for fault in ["write", "sync-file", "sync-directory"] {
        let mut f = Frame::act(EXPORT);
        f.record_fault = Some(fault);
        f.move_answers.push_back(Answer::Moving(RECORD));
        f.sign_in_then_deliver_me();
        done(&f, 1, true);
        assert_eq!(f.move_requests.len(), 1);
        assert_eq!(f.stderr(), b"temper ctl: /linked-parent/record not written: NoSpace; the engine keeps the account and cancels the move at its deadline\n");
    }
}
#[test]
fn an_export_whose_record_is_lost_removes_the_file_and_sends_no_written() {
    for answer in [Answer::Moving(b""), Answer::Lost] {
        let f = exporting([answer]);
        done(&f, 1, true);
        assert_eq!(f.move_requests.len(), 1);
        assert_eq!(f.stderr(), LOST);
    }
}
#[test]
fn an_export_whose_written_is_unanswered_keeps_the_file() {
    let mut f = exporting([Answer::Moving(RECORD)]);
    f.advance_past(crate::limits::DEADLINE);
    done(&f, 1, false);
    assert_eq!(f.record_data, RECORD);
    assert!(skein_lib::bytes::find(f.stored(b"store").expect("written kept"), &f.act_keys()[1]).is_some());
}
#[test]
fn a_signal_before_written_removes_the_file_and_after_written_keeps_it() {
    for (answers, removed) in [(vec![], true), (vec![Answer::Moving(RECORD)], false)] {
        let mut f = exporting(answers);
        f.signal(ServiceSignal::Terminate);
        done(&f, crate::limits::TERMINATED, removed);
    }
}
#[test]
fn an_unsent_export_removes_the_file_without_written() {
    let f = exporting([Answer::Unsent]);
    done(&f, 1, true);
    assert_eq!(f.move_requests.len(), 1);
}
#[test]
fn an_import_refuses_a_file_not_private_a_link_empty_or_too_large_and_sends_nothing() {
    for (kind, mode, owner, size, error, bytes) in [
        (Kind::File, 0o640, 7, 1, None, RECORD),
        (Kind::File, 0o600, 8, 1, None, RECORD),
        (Kind::Directory, 0o600, 7, 1, None, RECORD),
        (Kind::File, 0o600, 7, 1, Some(Error::TooManyLinks), RECORD),
        (Kind::File, 0o600, 7, 0, None, b"".as_slice()),
        (Kind::File, 0o600, 7, 131_073, None, OVER_BOUND),
    ] {
        let mut f = Frame::act(IMPORT);
        f.link = bytes;
        f.input_stat = Some(Stat { kind, mode, owner, size, links: 1 });
        f.linked_leaf = error;
        f.drain();
        done(&f, 1, false);
        assert_eq!(f.requests(), 0);
        assert!(skein_lib::bytes::find(f.stderr(), b"record file").is_some());
        assert!(!f.opened(b"directory"));
    }
}
#[test]
fn an_import_of_a_newer_form_refuses_it_and_sends_nothing() {
    let f = importing(b"\0\x02opaque-fake-moved-secret\n", []);
    done(&f, 1, false);
    assert_eq!(f.requests(), 0);
    assert_eq!(f.stderr(), b"temper ctl: /linked-parent/record holds a moved record of form v2; this temper reads up to v1: upgrade temper\n");
}
#[test]
fn an_import_sends_malformed_short_current_or_older_forms_whole() {
    for bytes in [b"\0\x01malformed\n".as_slice(), b"\0", b"\0\0old\n", b"\n", AT_BOUND, RECORD] {
        let f = importing(bytes, []);
        assert_eq!(f.carried_keys.len(), 1);
        assert_eq!(f.carried_keys[0].expose(), bytes);
        assert_eq!(f.move_requests[0].0, w::AccountHolder::Own);
        f.no_record_leaked();
    }
}
#[test]
fn an_import_moved_in_prints_the_account_and_removes_the_file() {
    for (renewed, line) in [(false, b"moved in: account 7\n".as_slice()), (true, b"moved in: account 7, renewed\n")] {
        let f = importing(RECORD, [Answer::MovedIn(renewed)]);
        done(&f, 0, true);
        assert_eq!(f.stdout(), line);
        assert_eq!(f.stderr(), b"");
        assert_eq!(f.carried_keys[0].expose(), RECORD);
    }
}
#[test]
fn an_import_whose_file_cannot_be_removed_reports_the_spent_record() {
    let mut f = Frame::act(IMPORT);
    f.link = RECORD;
    f.record_fault = Some("remove");
    f.move_answers.push_back(Answer::MovedIn(false));
    f.sign_in_then_deliver_me();
    done(&f, 1, false);
    assert_eq!(f.stdout(), b"moved in: account 7\n");
    assert_eq!(f.stderr(), b"temper ctl: /linked-parent/record not removed: Permission; its record is spent\n");
}
#[test]
fn an_import_refused_or_lost_keeps_the_file() {
    for (answer, status) in [
        (Answer::Refused(w::AccountMoveWhy::NotOurs), 2),
        (Answer::Refused(w::AccountMoveWhy::Held), 2),
        (Answer::Refused(w::AccountMoveWhy::Moving), 2),
        (Answer::Lost, 1),
        (Answer::Unsent, 1),
    ] {
        let f = importing(RECORD, [answer]);
        done(&f, status, false);
        assert_eq!(f.move_requests.len(), 1);
    }
}
#[test]
fn account_move_json_has_public_outcomes_and_never_the_record() {
    for (words, answers, expected) in [
        (
            &[b"export".as_slice(), b"7", b"/linked-parent/record", b"--json"][..],
            vec![Answer::Moving(RECORD), Answer::Done],
            br#""outcome":{"account":7,"number":83,"file":"/linked-parent/record"}"#.as_slice(),
        ),
        (
            &[b"import".as_slice(), b"/linked-parent/record", b"--key", b"move-in", b"--json"][..],
            vec![Answer::MovedIn(true)],
            br#""account":7,"renewed":true"#,
        ),
    ] {
        let mut f = Frame::act(words);
        f.link = RECORD;
        f.move_answers.extend(answers);
        f.sign_in_then_deliver_me();
        assert_eq!(f.exit(), Some(Exit::Code(0)));
        assert!(skein_lib::bytes::find(f.stdout(), expected).is_some(), "{:?}", f.stdout());
        f.no_record_leaked();
    }
}

#[test]
fn an_export_create_that_wins_a_signal_race_is_removed() {
    let mut f = Frame::act(EXPORT);
    f.hold_record_create = true;
    f.race_record_create = true;
    f.signal(ServiceSignal::Terminate);
    done(&f, crate::limits::TERMINATED, true);
    assert_eq!(f.requests(), 0);
    assert_eq!(f.file_steps, ["create", "remove"]);
}
#[test]
fn an_export_removal_failure_does_not_add_a_second_error() {
    let mut f = Frame::act(EXPORT);
    f.record_fault = Some("remove");
    f.move_answers.push_back(Answer::Refused(w::AccountMoveWhy::Moving));
    f.sign_in_then_deliver_me();
    done(&f, 2, false);
    assert_eq!(f.stderr(), b"export: moving\n");
}
#[test]
fn an_import_cut_by_a_signal_or_deadline_keeps_the_file() {
    let mut f = importing(RECORD, []);
    f.signal(ServiceSignal::Interrupt);
    done(&f, crate::limits::INTERRUPTED, false);
    let mut f = importing(RECORD, []);
    f.advance_past(crate::limits::DEADLINE);
    done(&f, 1, false);
}
#[test]
fn a_later_call_resends_written_before_importing() {
    let mut export = exporting([Answer::Moving(RECORD)]);
    export.advance_past(crate::limits::DEADLINE);
    let key = export.act_keys()[1];
    let mut next = Frame::act_words_with(IMPORT, export.store_snapshot());
    next.link = RECORD;
    next.move_answers.extend([Answer::Done, Answer::MovedIn(false)]);
    next.sign_in_then_deliver_me();
    done(&next, 0, true);
    assert_eq!(next.act_keys()[0], key);
    assert!(matches!(&next.move_requests[0].1, w::AccountChange::Written(written) if written.number() == NUMBER));
    assert!(matches!(&next.move_requests[1].1, w::AccountChange::MoveIn(_)));
}
#[test]
fn json_calls_cannot_name_move_files() {
    for bytes in [
        br#"{"tool":"export","arguments":{"account":7,"file":"record"}}"#.as_slice(),
        br#"{"tool":"import","arguments":{"file":"record","key":"move-in"}}"#,
    ] {
        let mut f = Frame::read(&["--call", "-"]);
        f.input = bytes;
        f.drain();
        done(&f, 1, false);
        assert_eq!(f.requests(), 0);
        assert!(!f.opened(b"directory"));
    }
}
