use super::fixtures;
use crate::{
    jig::{self, argv::Usage, call::Source},
    table,
};
#[test]
fn every_call_goes_from_argv_to_json_and_back_unchanged() {
    for words in fixtures::EVERY_TOOL {
        let mut call = fixtures::call(words);
        if call.tool == jig::rows::CHANGE_SETTINGS_AT || call.tool == jig::rows::PROPOSE_SETTINGS_AT {
            *call.values.get_mut(0).expect("FILE") =
                Some(jig::call::Values::One(jig::call::Value::Text(br#"{"items":[]}"#.as_slice().into())));
        }
        let json = jig::json::write_call(table::TOOLS, &call).expect("a call's JSON within its bound");
        fixtures::unique_keys(&json).expect("call JSON has unique keys");
        assert_eq!(fixtures::from_json(&json), call, "{words:?}");
    }
}
const DECISIONS: &[(&[u8], &[&str])] = &[
    (
        br#"{"tool":"proposal decide","arguments":{"proposal":"t3/17","outcome":"accept","reason":"yes"}}"#,
        &["proposal", "decide", "t3/17", "accept", "yes"],
    ),
    (
        br#"{"tool":"proposal decide","arguments":{"proposal":"p3/17","outcome":"reject"}}"#,
        &["proposal", "decide", "p3/17", "reject"],
    ),
    (
        br#"{"tool":"task decide","arguments":{"task":41,"hold":2,"outcome":"release","reason":"yes"}}"#,
        &["task", "decide", "41", "--hold", "2", "release", "yes"],
    ),
];
#[test]
fn decides_arguments_are_proposal_or_task_and_hold_then_outcome_and_reason() {
    for (json, words) in DECISIONS {
        assert_eq!(fixtures::from_json(json), fixtures::call(words), "{words:?}");
    }
    fixtures::json(br#"{"tool":"proposal decide","arguments":{"proposal":"3/17","outcome":"accept"}}"#)
        .expect_err("unprefixed proposal refused");
}
const UNKNOWN: &[&[u8]] = &[
    br#"{"tool":"me","arguments":{},"extra":1}"#,
    br#"{"tool":"me","tool":"me","arguments":{}}"#,
    br#"{"tool":"me","arguments":{},"arguments":{}}"#,
    br#"{"tool":"task show","arguments":{"task":41,"task":42}}"#,
    br#"{"tool":"me","arguments":{"extra":1}}"#,
    br#"{"tool":"task show","arguments":{"task":41,"project":"shop"}}"#,
];
#[test]
fn an_unknown_member_or_argument_is_a_usage_error() {
    for json in UNKNOWN {
        fixtures::json(json).expect_err("usage error");
    }
}
const WRONG: &[&[u8]] = &[
    br#"{"tool":1,"arguments":{}}"#,
    br#"{"tool":"task show","arguments":{"task":"41"}}"#,
    br#"{"tool":"task show","arguments":{"task":4.1}}"#,
    br#"{"tool":"task show","arguments":{"task":-1}}"#,
    br#"{"tool":"task show","arguments":{"task":1e2}}"#,
    br#"{"tool":"task show","arguments":{"task":41,"tree":false}}"#,
    br#"{"tool":"task show","arguments":{"task":41,"tree":"true"}}"#,
    br#"{"tool":"task message","arguments":{"task":41,"text":true,"key":"say"}}"#,
    br#"{"tool":"chat close","arguments":{"task":41,"move":2,"key":"close"}}"#,
    br#"{"tool":"wait","arguments":{"subject":"me","timeout":"1m"}}"#,
    br#"{"tool":"me","arguments":[]}"#,
    b"[]",
    br#"{"tool":"me","arguments":{}} {}"#,
];
#[test]
fn a_value_of_another_json_kind_is_a_usage_error() {
    for json in WRONG {
        fixtures::json(json).expect_err("usage error");
    }
}
const CLOSE: &[(&[u8], &[&str])] = &[
    (br#"{"tool":"chat close","arguments":{"task":41,"key":"close"}}"#, &["chat", "close", "41", "--key", "close"]),
    (
        br#"{"tool":"chat close","arguments":{"reason":"bye","task":41,"key":"close","move":[2,3]}}"#,
        &["chat", "close", "41", "--reason", "bye", "--move", "2", "--move", "3", "--key", "close"],
    ),
];
#[test]
fn json_uses_the_tables_close_reason() {
    for (json, words) in CLOSE {
        assert_eq!(fixtures::from_json(json), fixtures::call(words), "{words:?}");
    }
}
const WAIT_OK: &[(&str, Option<&str>)] = &[
    ("accounts", Some("usable")),
    ("accounts", Some("settled")),
    ("me", Some("waiting")),
    ("inbox", Some("waiting")),
    ("inbox", Some("updates")),
    ("task 41", Some("ended")),
    ("task 41", Some("waiting-on-me")),
    ("task 41", Some("held")),
    ("task 41", Some("replied")),
    ("tree 41", Some("ended")),
    ("run 41", Some("ended")),
    ("tasks --project shop --mine", None),
    ("task 41", Some("phase:waiting")),
    ("task 41", Some("phase:idle")),
    ("task 41", Some("phase:due")),
    ("task 41", Some("phase:preparing")),
    ("task 41", Some("phase:claimed")),
    ("task 41", Some("phase:running")),
    ("task 41", Some("phase:backing-off")),
    ("task 41", Some("phase:active")),
    ("task 41", Some("phase:closing")),
    ("task 41", Some("phase:held")),
];
const WAIT_BAD: &[(&str, Option<&str>)] = &[
    ("me", Some("ended")),
    ("inbox", Some("held")),
    ("tree 41", Some("waiting-on-me")),
    ("run 41", Some("held")),
    ("tasks --project shop", Some("waiting")),
    ("inbox --project shop", None),
    ("tasks --project shop --after 00", None),
    ("tasks --project shop --ended", None),
    ("task 41", Some("phase:done")),
    ("task 41", Some("phase:failed")),
    ("task 41", Some("phase:cancelled")),
    ("task 41", Some("phase:ended")),
];
// client/tools.md 5.4: timeout at its maximum and one second past it; zero is immediate.
const TIMEOUTS: &[(&[u8], bool)] = &[
    (br#"{"tool":"wait","arguments":{"subject":"me","timeout":0}}"#, true),
    (br#"{"tool":"wait","arguments":{"subject":"me","timeout":3600}}"#, true),
    (br#"{"tool":"wait","arguments":{"subject":"me","timeout":3601}}"#, false),
];
#[test]
fn json_wait_arguments_and_conditions_match_argv() {
    for (subject, until) in WAIT_OK {
        let json = fixtures::json_wait(subject, *until);
        assert_eq!(fixtures::from_json(&json), fixtures::wait_call(subject, *until), "{subject}, {until:?}");
    }
    for (subject, until) in WAIT_BAD {
        let json = fixtures::json_wait(subject, *until);
        fixtures::json(&json).expect_err("forbidden wait subject or condition");
    }
    for (json, accepted) in TIMEOUTS {
        assert_eq!(fixtures::json(json).is_ok(), *accepted, "{json:?}");
    }
}
#[test]
fn a_text_from_stdin_is_refused_under_call_stdin() {
    const INPUT: &[u8] = br#"{"tool":"task message","arguments":{"task":41,"text":"-","key":"say"}}"#;
    assert_eq!(jig::json::call(table::TOOLS, INPUT, Source::Stdin, &crate::limits::tools()), Err(Usage::Stdin));
    assert_eq!(fixtures::from_json(INPUT), fixtures::call(&["task", "message", "41", "-", "--key", "say"]));
}
#[test]
fn a_document_past_ask_bytes_is_a_usage_error() {
    const INPUT: &[u8] = br#"{"tool":"me","arguments":{}}"#;
    // Exactly the valid document's byte length, then one less.
    let length = u64::try_from(INPUT.len()).expect("fixture length fits");
    let mut limits = crate::limits::tools();
    for (bound, expected) in [(length, Ok(())), (length.checked_sub(1).expect("nonempty fixture"), Err(Usage::Json))] {
        limits.ask_bytes = bound;
        let result = jig::json::call(table::TOOLS, INPUT, Source::Stdin, &limits);
        let result = match result {
            Ok(_) => Ok(()),
            Err(error) => Err(error),
        };
        assert_eq!(result, expected, "{bound}");
    }
}
#[test]
fn each_tools_record_is_written_as_json() {
    for tool in table::TOOLS {
        let json = jig::json::write_tool(tool, &crate::limits::tools()).expect("a tool record's JSON");
        fixtures::unique_keys(&json).expect("tool JSON has unique keys");
        assert!(skein_lib::bytes::find(&json, tool.name).is_some(), "{tool:?}");
        assert!(skein_lib::bytes::find(&json, br#""forms":["#).is_some(), "{tool:?}");
        skein_json::tokenizer::validate(
            &json,
            &skein_json::tokenizer::Limits {
                depth: 6,
                string: 0,
                number: 20,
                chunk: 256,
                length: u32::try_from(json.len()).expect("small record"),
            },
        )
        .expect("valid record JSON");
    }
}

#[test]
fn the_key_guard_names_the_first_repeated_decoded_key_and_its_path() {
    for (json, expected) in [
        (br#"{"result":1,"result":2}"#.as_slice(), "repeated key \"result\" at $[\"result\"]"),
        (
            br#"{"tasks":[{"result":1,"\u0072esult":2,"result":3}]}"#,
            "repeated key \"result\" at $[\"tasks\"][0][\"result\"]",
        ),
    ] {
        assert_eq!(fixtures::unique_keys(json).expect_err("repeated key"), expected);
    }
    fixtures::unique_keys(br#"{"tasks":[{"result":null},{"result":{"nested":true}}],"other":[]}"#)
        .expect("keys in separate objects are independent");
    fixtures::unique_keys(br#"{"unfinished":[]"#).expect_err("invalid JSON is refused");
}

#[test]
fn every_json_golden_has_unique_keys() {
    let mut checked = 0_u32;
    for entry in std::fs::read_dir("src/tests/golden").expect("golden directory") {
        let path = entry.expect("golden entry").path();
        let name = path.file_name().expect("golden name").to_str().expect("UTF-8 name");
        // Current JSON goldens have .txt names; also cover future .json files.
        if path.extension() == Some(std::ffi::OsStr::new("json")) || name.ends_with("_json.txt") {
            let json = std::fs::read(&path).expect("read JSON golden");
            assert_eq!(fixtures::unique_keys(&json), Ok(()), "{}", path.display());
            checked = checked.checked_add(1).expect("bounded golden count");
        }
    }
    assert!(checked > 0, "at least one JSON golden checked");
}

#[test]
fn json_arguments_use_snake_case() {
    const JSON: &[u8] = br#"{"tool":"repository adopt","arguments":{"project":"shop","owner":"o","name":"n","landing":"main","no_home":true,"no_ci":true,"check":[1,2],"key":"adopt-repo"}}"#;
    let call = fixtures::call(&[
        "repository",
        "adopt",
        "--project",
        "shop",
        "--owner",
        "o",
        "--name",
        "n",
        "--landing",
        "main",
        "--no-home",
        "--no-ci",
        "--check",
        "1",
        "--check",
        "2",
        "--key",
        "adopt-repo",
    ]);
    assert_eq!(fixtures::from_json(JSON), call);
    let encoded = jig::json::write_call(table::TOOLS, &call).expect("JSON call");
    assert!(skein_lib::bytes::find(&encoded, br#""no_home":true"#).is_some());
}
#[test]
fn unencodable_argument_bytes_refuse_without_a_partial_record() {
    for words in [
        &[b"sign-in".as_slice(), b"\xff".as_slice()].as_slice(),
        &[b"task", b"message".as_slice(), b"41", b"\xff", b"--key", b"say"].as_slice(),
    ] {
        let jig::call::Parsed::Call { call, .. } = table::parse(words).expect("argv bytes") else {
            panic!("plain call");
        };
        assert_eq!(jig::json::write_call(table::TOOLS, &call), Err(jig::json::Refusal::Text));
    }
}

#[test]
fn the_tool_record_json_carries_prints_notes_and_bounds() {
    let mut limits = crate::limits::tools();
    // These differ from the host preset: every rendered bound must use the caller's limits.
    limits.rows = 7;
    limits.ask_bytes = 11;
    limits.wait_most = skein_lib::Duration::from_secs(13);
    let json = jig::json::write_tool(&super::help::RECORD, &limits).expect("fixture record JSON");
    fixtures::unique_keys(&json).expect("unique record keys");
    for member in [
        br#""prints":"task N""#.as_slice(),
        br#""notes":"First note.\nSecond note.\n""#,
        br#""bound":{"least":1,"most":7}"#,
        br#""bound":{"bytes":11}"#,
        br#""bound":{"seconds":13}"#,
        br#""bound":{"least":1,"most":65535}"#,
        br#""bound":null"#,
    ] {
        assert!(skein_lib::bytes::find(&json, member).is_some(), "missing {member:?}");
    }
}
