use super::fixtures;
use crate::jig::{
    argv::{self, Usage},
    call::{Parsed, Value, Values},
    subject::{self, Condition, Subject},
};
use jig_client::contract::Phase;
use skein_lib::Duration;
const UNTIL: &[(&[&str], bool)] = &[
    (&["wait", "accounts", "--until", "usable"], true),
    (&["wait", "accounts", "--until", "settled"], true),
    (&["wait", "accounts", "--until", "waiting"], false),
    (&["wait", "me", "--until", "usable"], false),
    (&["wait", "me", "--until", "waiting"], true),
    (&["wait", "me", "--until", "ended"], false),
    (&["wait", "inbox", "--until", "updates"], true),
    (&["wait", "inbox", "--until", "waiting"], true),
    (&["wait", "task", "41", "--until", "phase:backing-off"], true),
    (&["wait", "tree", "41", "--until", "replied"], false),
    (&["wait", "tree", "41", "--until", "waiting-on-me"], false),
    (&["wait", "tree", "41", "--until", "ended"], true),
    (&["wait", "run", "41", "--until", "ended"], true),
    (&["wait", "run", "41", "--until", "held"], false),
    (&["wait", "task", "41", "--until", "waiting-on-me"], true),
    (&["wait", "task", "41", "--until", "held"], true),
    (&["wait", "task", "41", "--until", "replied"], true),
    (&["wait", "task", "41", "--until", "phase:done"], false),
    (&["wait", "task", "41", "--until", "phase:failed"], false),
    (&["wait", "task", "41", "--until", "phase:cancelled"], false),
    (&["wait", "task", "41", "--until", "phase:ended"], false),
    (&["wait", "task", "41", "--until", "ended"], true),
    (&["wait", "tasks", "--project", "shop", "--until", "ended"], false),
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
        fixtures::call(&["wait", "me"]).values[3],
        Some(Values::One(Value::Duration(Duration::from_secs(DEFAULT_SECONDS))))
    );
    for (word, seconds) in TIMEOUTS {
        let call = fixtures::call(&["wait", "me", "--timeout", word]);
        assert_eq!(call.values[3], Some(Values::One(Value::Duration(Duration::from_secs(*seconds)))), "{word}");
        assert_eq!(fixtures::printed(&call), Parsed::Call { call, json: false }, "{word}");
    }
    assert_eq!(fixtures::parse(&["wait", "me", "--timeout", "3601"]), Err(Usage::Rows { argument: 3 }));
    for word in ["-1", "0.5", "1d", "18446744073709551615h"] {
        assert_eq!(fixtures::parse(&["wait", "me", "--timeout", word]), Err(Usage::Kind { argument: 3 }), "{word}");
    }
}
#[test]
fn wait_tasks_takes_neither_a_cursor_nor_ended() {
    for words in [
        &["wait", "tasks", "--project", "shop", "--after", "abcd"][..],
        &["wait", "tasks", "--project", "shop", "--ended"],
    ] {
        assert_eq!(fixtures::parse(words), Err(Usage::Unknown { at: 2 }), "{words:?}");
    }
    let call = fixtures::call(&[
        "wait",
        "tasks",
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
    &["wait", "inbox", "--project", "shop"],
    &["wait", "inbox", "--kind", "replies"],
    &["wait", "inbox", "--root", "41"],
    &["wait", "inbox", "--after", "abcd"],
];
#[test]
fn wait_inbox_takes_no_subject_flag() {
    for words in INBOX_FLAGS {
        assert_eq!(fixtures::parse(words), Err(Usage::Unknown { at: 1 }), "{words:?}");
    }
}
const READ_FLAGS: &[&[&str]] = &[
    &["wait", "task", "41", "--tree"],
    &["wait", "task", "41", "--run"],
    &["wait", "task", "41", "--activity", "2"],
    &["wait", "me", "--tree"],
];
#[test]
fn other_read_flags_are_not_watch_arguments() {
    for words in READ_FLAGS {
        assert_eq!(fixtures::parse(words), Err(Usage::Unknown { at: 1 }), "{words:?}");
    }
    for words in [&["wait", "chat", "41"][..], &["wait", "activity", "41"], &["wait", "turn", "41", "3.7"]] {
        assert_eq!(fixtures::parse(words), Err(Usage::Kind { argument: 0 }), "{words:?}");
    }
}
#[test]
fn ended_phase_errors_name_until_ended() {
    for word in ["phase:done", "phase:failed", "phase:cancelled", "phase:ended"] {
        assert_eq!(subject::condition(&Subject::Task(41), word.as_bytes()), Err(Usage::Condition), "{word}");
    }
    assert_eq!(subject::condition(&Subject::Task(41), b"ended"), Ok(Condition::Ended));
}
const DIGESTS: &[(&str, [u8; 8])] =
    &[("0011223344556677", [0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77]), ("ffffffffffffffff", [255; 8])];
#[test]
fn since_and_until_parse_together() {
    for (word, digest) in DIGESTS {
        let call = fixtures::call(&["wait", "me", "--since", word, "--until", "waiting"]);
        assert_eq!(call.values[1], Some(Values::One(Value::Digest(*digest))), "{word}");
        assert_eq!(call.values[2], Some(Values::One(Value::Condition(Condition::Waiting))), "{word}");
        assert_eq!(fixtures::printed(&call), Parsed::Call { call, json: false }, "{word}");
    }
    for word in ["00112233445566", "001122334455667788", "00112233445566AA"] {
        assert_eq!(fixtures::parse(&["wait", "me", "--since", word]), Err(Usage::Kind { argument: 1 }), "{word}");
    }
    let call = fixtures::call(&["wait", "tasks", "--timeout", "5m", "--project", "shop"]);
    assert_eq!(fixtures::printed(&call), Parsed::Call { call, json: false });
}
#[test]
fn typed_wait_calls_follow_the_same_subject_condition_and_duration_checks() {
    let base = fixtures::call(&["wait", "task", "41", "--until", "ended"]);
    for (slot, value, error) in [
        (0, Value::Subject(Subject::Me), Usage::Condition),
        (2, Value::Condition(Condition::Phase(Phase::Done)), Usage::Condition),
        (3, Value::Duration(Duration::from_secs(MOST_SECONDS.checked_add(1).unwrap())), Usage::Rows { argument: 3 }),
    ] {
        let mut call = base.clone();
        call.values[slot] = Some(Values::One(value));
        assert_eq!(argv::check(crate::table::TOOLS, &call, &crate::limits::tools()), Err(error), "{slot}");
    }
    for words in [
        &["task", "list", "--project", "shop", "--ended"][..],
        &["task", "list", "--project", "shop", "--after", "abcd"],
    ] {
        let mut call = base.clone();
        call.values[0] = Some(Values::One(Value::Subject(Subject::Tasks(Box::new(fixtures::call(words))))));
        assert_eq!(
            argv::check(crate::table::TOOLS, &call, &crate::limits::tools()),
            Err(Usage::Condition),
            "{words:?}"
        );
    }
}
