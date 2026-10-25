use super::fixtures;
use crate::{
    jig::{self, argv::Usage, call::Source},
    table,
};
#[test]
fn every_call_goes_from_argv_to_json_and_back_unchanged() {
    for words in fixtures::EVERY_TOOL {
        let mut call = fixtures::call(words);
        if matches!(call.tool, jig::rows::CHANGE_SETTINGS_AT | jig::rows::PROPOSE_SETTINGS_AT) {
            *call.values.get_mut(0).expect("FILE") =
                Some(jig::call::Values::One(jig::call::Value::Text(br#"{"items":[]}"#.as_slice().into())));
        }
        let json = jig::json::write_call(table::TOOLS, &call).expect("a call's JSON within its bound");
        assert_eq!(fixtures::from_json(&json), call, "{words:?}");
    }
}
const DECISIONS: &[(&[u8], &[&str])] = &[
    (
        br#"{"tool":"decide","arguments":{"proposal":"t3/17","outcome":"accept","reason":"yes"}}"#,
        &["decide", "t3/17", "accept", "yes"],
    ),
    (br#"{"tool":"decide","arguments":{"proposal":"p3/17","outcome":"reject"}}"#, &["decide", "p3/17", "reject"]),
    (
        br#"{"tool":"decide","arguments":{"task":41,"hold":2,"outcome":"release","reason":"yes"}}"#,
        &["decide", "41", "--hold", "2", "release", "yes"],
    ),
];
#[test]
fn decides_arguments_are_proposal_or_task_and_hold_then_outcome_and_reason() {
    for (json, words) in DECISIONS {
        assert_eq!(fixtures::from_json(json), fixtures::call(words), "{words:?}");
    }
    fixtures::json(br#"{"tool":"decide","arguments":{"proposal":"3/17","outcome":"accept"}}"#)
        .expect_err("unprefixed proposal refused");
}
const UNKNOWN: &[&[u8]] = &[
    br#"{"tool":"me","arguments":{},"extra":1}"#,
    br#"{"tool":"me","tool":"me","arguments":{}}"#,
    br#"{"tool":"me","arguments":{},"arguments":{}}"#,
    br#"{"tool":"task","arguments":{"task":41,"task":42}}"#,
    br#"{"tool":"me","arguments":{"extra":1}}"#,
    br#"{"tool":"task","arguments":{"task":41,"project":"shop"}}"#,
];
#[test]
fn an_unknown_member_or_argument_is_a_usage_error() {
    for json in UNKNOWN {
        fixtures::json(json).expect_err("usage error");
    }
}
const WRONG: &[&[u8]] = &[
    br#"{"tool":1,"arguments":{}}"#,
    br#"{"tool":"task","arguments":{"task":"41"}}"#,
    br#"{"tool":"task","arguments":{"task":4.1}}"#,
    br#"{"tool":"task","arguments":{"task":-1}}"#,
    br#"{"tool":"task","arguments":{"task":1e2}}"#,
    br#"{"tool":"task","arguments":{"task":41,"tree":false}}"#,
    br#"{"tool":"task","arguments":{"task":41,"tree":"true"}}"#,
    br#"{"tool":"message","arguments":{"task":41,"text":true,"key":"say"}}"#,
    br#"{"tool":"close","arguments":{"task":41,"move":2,"key":"close"}}"#,
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
    (br#"{"tool":"close","arguments":{"task":41,"key":"close"}}"#, &["close", "41", "--key", "close"]),
    (
        br#"{"tool":"close","arguments":{"reason":"bye","task":41,"key":"close","move":[2,3]}}"#,
        &["close", "41", "--reason", "bye", "--move", "2", "--move", "3", "--key", "close"],
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
    const INPUT: &[u8] = br#"{"tool":"message","arguments":{"task":41,"text":"-","key":"say"}}"#;
    assert_eq!(jig::json::call(table::TOOLS, INPUT, Source::Stdin, &crate::limits::tools()), Err(Usage::Stdin));
    assert_eq!(fixtures::from_json(INPUT), fixtures::call(&["message", "41", "-", "--key", "say"]));
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
        let json = jig::json::write_tool(tool).expect("a tool record's JSON");
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
fn json_arguments_use_snake_case() {
    const JSON: &[u8] = br#"{"tool":"adopt","arguments":{"project":"shop","owner":"o","name":"n","landing":"main","no_home":true,"no_ci":true,"check":[1,2],"key":"adopt-repo"}}"#;
    let call = fixtures::call(&[
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
        &[b"message".as_slice(), b"41", b"\xff", b"--key", b"say"].as_slice(),
    ] {
        let jig::call::Parsed::Call { call, .. } = table::parse(words).expect("argv bytes") else {
            panic!("plain call");
        };
        assert_eq!(jig::json::write_call(table::TOOLS, &call), Err(jig::json::Refusal::Text));
    }
}
