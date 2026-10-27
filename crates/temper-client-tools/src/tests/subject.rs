use super::fixtures;
use crate::jig::{
    argv::{self, Usage},
    call::{Parsed, Value, Values},
    subject::{self, Condition, Subject},
};
use jig_client::contract::Phase;
use skein_lib::Duration;
const UNTIL: &[(&[&str], bool)] = &[
    (&["account", "list", "--until", "usable"], true),
    (&["account", "list", "--until", "settled"], true),
    (&["account", "list", "--until", "waiting"], false),
    (&["me", "--until", "usable"], false),
    (&["me", "--until", "waiting"], true),
    (&["me", "--until", "ended"], false),
    (&["inbox", "--until", "updates"], true),
    (&["inbox", "--until", "waiting"], true),
    (&["task", "show", "41", "--until", "phase:backing-off"], true),
    (&["task", "tree", "41", "--until", "replied"], false),
    (&["task", "tree", "41", "--until", "waiting-on-me"], false),
    (&["task", "tree", "41", "--until", "ended"], true),
    (&["task", "run", "41", "--until", "ended"], true),
    (&["task", "run", "41", "--until", "held"], false),
    (&["task", "show", "41", "--until", "waiting-on-me"], true),
    (&["task", "show", "41", "--until", "held"], true),
    (&["task", "show", "41", "--until", "replied"], true),
    (&["task", "show", "41", "--until", "phase:done"], false),
    (&["task", "show", "41", "--until", "phase:failed"], false),
    (&["task", "show", "41", "--until", "phase:cancelled"], false),
    (&["task", "show", "41", "--until", "phase:ended"], false),
    (&["task", "show", "41", "--until", "ended"], true),
    (&["task", "list", "--project", "shop", "--until", "ended"], false),
];
#[test]
fn a_condition_is_taken_only_where_its_subject_names_it() {
    for (words, taken) in UNTIL {
        assert_eq!(fixtures::parse(words).is_ok(), *taken, "{words:?}");
    }
}
const PHASES: &[(&str, Phase, bool)] = &[
    ("waiting", Phase::Waiting, true),
    ("idle", Phase::Idle, true),
    ("due", Phase::Due, true),
    ("preparing", Phase::Preparing, true),
    ("claimed", Phase::Claimed, true),
    ("running", Phase::Running, true),
    ("backing-off", Phase::BackingOff, true),
    ("active", Phase::Active, true),
    ("closing", Phase::Closing, true),
    ("held", Phase::Held, true),
    ("done", Phase::Done, false),
    ("failed", Phase::Failed, false),
    ("cancelled", Phase::Cancelled, false),
    ("ended", Phase::Ended, false),
];
#[test]
fn each_phase_word_names_one_of_jigs_phases_and_back() {
    for (word, phase, live) in PHASES {
        assert_eq!(subject::phase(word.as_bytes()), Some(*phase), "{word}");
        assert_eq!(subject::phase_word(*phase), word.as_bytes(), "{word}");
        let mut condition = skein_lib::Writer::new(word.len().checked_add(b"phase:".len()).unwrap());
        condition.put(b"phase:").unwrap();
        condition.put(word.as_bytes()).unwrap();
        assert_eq!(subject::condition(&Subject::Task(41), &condition.finish()).is_ok(), *live, "{word}");
    }
    assert_eq!(subject::phase(b"unknown"), None);
}
/// Seconds at the default and ceiling, with the first refused second beside them.
const DEFAULT_SECONDS: u64 = 300;
const MOST_SECONDS: u64 = 3600;
const TIMEOUTS: &[(&str, u64)] =
    &[("0", 0), ("300", DEFAULT_SECONDS), ("5m", DEFAULT_SECONDS), ("3600s", MOST_SECONDS), ("1h", MOST_SECONDS)];
#[test]
fn a_timeout_defaults_to_300_seconds_and_stops_at_3600() {
    assert_eq!(
        fixtures::call(&["me", "--until", "changed"]).values[2],
        Some(Values::One(Value::Duration(Duration::from_secs(DEFAULT_SECONDS))))
    );
    for (word, seconds) in TIMEOUTS {
        let call = fixtures::call(&["me", "--until", "changed", "--timeout", word]);
        assert_eq!(call.values[2], Some(Values::One(Value::Duration(Duration::from_secs(*seconds)))), "{word}");
        assert_eq!(fixtures::printed(&call), Parsed::Call { call, json: false }, "{word}");
    }
    assert_eq!(fixtures::parse(&["me", "--until", "changed", "--timeout", "3601"]), Err(Usage::Rows { argument: 2 }));
    for word in ["-1", "0.5", "1d", "18446744073709551615h"] {
        assert_eq!(
            fixtures::parse(&["me", "--until", "changed", "--timeout", word]),
            Err(Usage::Kind { argument: 2 }),
            "{word}"
        );
    }
}
#[test]
fn wait_tasks_takes_neither_a_cursor_nor_ended() {
    for words in [
        &["task", "list", "--project", "shop", "--after", "abcd"][..],
        &["task", "list", "--project", "shop", "--ended"],
    ] {
        let words = fixtures::extended(words, &["--until", "changed"]);
        match fixtures::parse(words.as_slice()) {
            Err(Usage::WaitPart { tool: 2, .. }) => {}
            other => panic!("expected incompatible task-list flag, got {other:?}"),
        }
    }
    let call = fixtures::call(&[
        "task",
        "list",
        "--until",
        "changed",
        "--project",
        "shop",
        "--label",
        "chat",
        "--mine",
        "--tracked",
        "--order",
        "activity",
    ]);
    assert_eq!(fixtures::printed(&call), Parsed::Call { call, json: false });
}
const INBOX_FLAGS: &[&[&str]] = &[
    &["inbox", "--project", "shop"],
    &["inbox", "--kind", "replies"],
    &["inbox", "--root", "41"],
    &["inbox", "--after", "abcd"],
];
#[test]
fn wait_inbox_takes_no_subject_flag() {
    for words in INBOX_FLAGS {
        let words = fixtures::extended(words, &["--until", "changed"]);
        assert!(fixtures::parse(words.as_slice()).is_err(), "{words:?}");
    }
}
const READ_FLAGS: &[&[&str]] = &[
    &["task", "show", "41", "--tree"],
    &["task", "show", "41", "--run"],
    &["task", "show", "41", "--activity", "2"],
    &["me", "--tree"],
];
#[test]
fn other_read_flags_are_not_watch_arguments() {
    for words in READ_FLAGS {
        let words = fixtures::extended(words, &["--until", "changed"]);
        assert!(fixtures::parse(words.as_slice()).is_err(), "{words:?}");
    }
    for (words, at) in [
        (&["chat", "show", "41", "--until", "changed"][..], 1),
        (&["task", "activity", "41", "--until", "changed"], 1),
        (&["task", "turn", "41", "3.7", "--until", "changed"], 2),
    ] {
        assert_eq!(fixtures::parse(words), Err(Usage::Unknown { at }), "{words:?}");
    }
}
#[test]
fn terminal_phases_are_refused_and_ended_is_taken() {
    for (word, phase) in [
        ("phase:done", Phase::Done),
        ("phase:failed", Phase::Failed),
        ("phase:cancelled", Phase::Cancelled),
        ("phase:ended", Phase::Ended),
    ] {
        assert_eq!(
            subject::condition(&Subject::Task(41), word.as_bytes()),
            Err(Usage::TerminalPhase { phase }),
            "{word}"
        );
    }
    assert_eq!(subject::condition(&Subject::Task(41), b"ended"), Ok(Condition::Ended));
}
const DIGESTS: &[(&str, [u8; 8])] =
    &[("0011223344556677", [0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77]), ("ffffffffffffffff", [255; 8])];
#[test]
fn since_and_until_parse_together() {
    for (word, digest) in DIGESTS {
        let call = fixtures::call(&["me", "--since", word, "--until", "waiting"]);
        assert_eq!(call.values[0], Some(Values::One(Value::Digest(*digest))), "{word}");
        assert_eq!(call.values[1], Some(Values::One(Value::Condition(Condition::Waiting))), "{word}");
        assert_eq!(fixtures::printed(&call), Parsed::Call { call, json: false }, "{word}");
    }
    for word in ["00112233445566", "001122334455667788", "00112233445566AA"] {
        assert_eq!(fixtures::parse(&["me", "--since", word]), Err(Usage::Kind { argument: 0 }), "{word}");
    }
    let call = fixtures::call(&["task", "list", "--until", "changed", "--timeout", "5m", "--project", "shop"]);
    assert_eq!(fixtures::printed(&call), Parsed::Call { call, json: false });
}
#[test]
fn typed_wait_calls_follow_the_same_subject_condition_and_duration_checks() {
    let base = fixtures::call(&["task", "show", "41", "--until", "ended"]);
    for (slot, value, error) in [
        (5, Value::Condition(Condition::Phase(Phase::Done)), Usage::TerminalPhase { phase: Phase::Done }),
        (6, Value::Duration(Duration::from_secs(MOST_SECONDS.checked_add(1).unwrap())), Usage::Rows { argument: 6 }),
        (1, Value::Switch, Usage::WaitPart { tool: 3, argument: 1 }),
    ] {
        let mut call = base.clone();
        call.values[slot] = Some(Values::One(value));
        assert_eq!(argv::check(crate::table::TOOLS, &call, &crate::limits::tools()), Err(error), "{slot}");
    }
    for (flag, argument) in [("--ended", 4), ("--after", 6)] {
        let words = if flag == "--ended" {
            fixtures::extended(&["task", "list", "--project", "shop"], &[flag])
        } else {
            fixtures::extended(&["task", "list", "--project", "shop"], &[flag, "abcd"])
        };
        let mut call = fixtures::call(words.as_slice());
        call.values[9] = Some(Values::One(Value::Condition(Condition::Changed)));
        assert_eq!(
            argv::check(crate::table::TOOLS, &call, &crate::limits::tools()),
            Err(Usage::WaitPart { tool: 2, argument })
        );
    }
}
