use super::fixtures;
use crate::jig::{
    argv::{self, Unwritten, Usage},
    call::{Parsed, Project, Source, Value, Values},
    table::{Keyed, Kind, Occurs, Tool, positional},
};
use jig_client::contract::QueryOrder;
use skein_lib::List;
const READS: &[&[&str]] = &[
    &["me"],
    &["inbox", "--project", "shop", "--kind", "replies", "--root", "41"],
    &["task", "list", "--project", "3", "--label", "chat", "--tracked", "--order", "priority"],
    &["task", "list", "--project", "3"],
    &["task", "show", "41", "--tree", "--activity", "5", "--run"],
    &["chat", "show", "41", "--before", "0a1b"],
    &["task", "activity", "41", "--limit", "32"],
    &["task", "turn", "41", "3.7", "--part", "2", "--from", "16384"],
    &["task", "run", "41"],
    &["account", "list"],
    &["model", "list"],
    &["model", "list", "--project", "shop"],
];
#[test]
fn each_read_parses_into_its_call_and_prints_back() {
    for words in READS {
        let call = fixtures::call(words);
        assert_eq!(fixtures::back(&call), fixtures::owned(words), "{words:?}");
        assert_eq!(fixtures::printed(&call), Parsed::Call { call, json: false }, "{words:?}");
    }
}
const HOST: &[&[&str]] =
    &[&["sign-in", "link"], &["sign-in", "-"], &["sign-out"], &["pending"], &["help"], &["help", "inbox"]];
#[test]
fn each_host_tool_parses_into_its_call_and_prints_back() {
    for words in HOST {
        assert_eq!(fixtures::back(&fixtures::call(words)), fixtures::owned(words), "{words:?}");
    }
    for words in fixtures::EVERY_TOOL {
        assert!(fixtures::parse(words).is_ok(), "{words:?}");
    }
}
const UNKNOWN: &[(&[&str], Usage)] = &[
    (&["absent"], Usage::NoGroup { at: 0 }),
    (&["me", "--kind", "replies"], Usage::Unknown { at: 0 }),
    (&["me", "extra"], Usage::Extra { at: 0 }),
    (&["inbox", "--no-project"], Usage::Unknown { at: 0 }),
];
#[test]
fn an_unknown_tool_or_argument_is_a_usage_error() {
    for (words, error) in UNKNOWN {
        assert_eq!(fixtures::parse(words), Err(*error), "{words:?}");
    }
}
const MISSING: &[(&[&str], Usage)] = &[
    (&["task", "list"], Usage::Missing { argument: 0 }),
    (&["task", "show"], Usage::Missing { argument: 0 }),
    (&["task", "turn", "41"], Usage::Missing { argument: 1 }),
    (&["inbox", "--root"], Usage::Missing { argument: 2 }),
    (&["inbox", "--root", "41", "--root", "42"], Usage::Twice { argument: 2 }),
    (&["task", "show", "41", "--tree", "--tree"], Usage::Twice { argument: 1 }),
];
#[test]
fn a_missing_or_repeated_argument_is_a_usage_error() {
    for (words, error) in MISSING {
        assert_eq!(fixtures::parse(words), Err(*error), "{words:?}");
    }
}
const INVALID: &[(&[&str], Usage)] = &[
    (&["task", "show", "x"], Usage::Kind { argument: 0 }),
    (&["task", "show", "18446744073709551616"], Usage::Kind { argument: 0 }),
    (&["task", "list", "--project", "4294967296"], Usage::Kind { argument: 0 }),
    (&["task", "list", "--project", "3", "--order", "ended"], Usage::Kind { argument: 5 }),
    (&["task", "list", "--project", "3", "--order", "changed"], Usage::Kind { argument: 5 }),
    (&["inbox", "--kind", "other"], Usage::Kind { argument: 1 }),
    (&["chat", "show", "41", "--before", "AB"], Usage::Kind { argument: 1 }),
    (&["chat", "show", "41", "--before", "a"], Usage::Kind { argument: 1 }),
    (&["task", "turn", "41", "3.4294967296"], Usage::Kind { argument: 1 }),
    (&["task", "activity", "41", "--limit", "33"], Usage::Rows { argument: 2 }),
    (&["task", "activity", "41", "--limit", "0"], Usage::Rows { argument: 2 }),
];
#[test]
fn a_value_outside_its_kind_or_rows_is_a_usage_error() {
    for (words, error) in INVALID {
        assert_eq!(fixtures::parse(words), Err(*error), "{words:?}");
    }
}
#[test]
fn a_project_is_a_number_or_a_name() {
    for (word, project) in [("3", Project::Number(3)), ("shop", Project::Name(Box::from(*b"shop")))] {
        assert_eq!(
            fixtures::call(&["task", "list", "--project", word]).values[0],
            Some(Values::One(Value::Project(project))),
            "{word}"
        );
    }
}
/// The parser checks stdin ownership for every table, including later text rows.
const STDIN_PAIR: Tool = Tool {
    name: b"pair",
    line: b"",
    prints: b"",
    result: b"",
    forms: &[&[
        positional(b"first", b"FILE", Kind::File, Occurs::Required, b""),
        positional(b"second", b"TEXT", Kind::Text, Occurs::Required, b""),
    ]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"",
};
#[test]
fn a_second_stdin_is_a_usage_error() {
    assert_eq!(argv::parse(&[&STDIN_PAIR], &[b"pair", b"-", b"-"], &crate::limits::tools()), Err(Usage::Stdin));
}
#[test]
fn call_and_json_are_the_hosts_options() {
    assert_eq!(fixtures::parse(&["--call", "-", "--json"]), Ok(Parsed::File { file: Source::Stdin, json: true }));
    assert_eq!(
        fixtures::parse(&["--json", "--call", "input"]),
        Ok(Parsed::File { file: Source::Path(Box::from(*b"input")), json: true })
    );
    assert_eq!(fixtures::parse(&["me", "--json"]), Ok(Parsed::Call { call: fixtures::call(&["me"]), json: true }));
    for words in [&["--call"][..], &["--call", "-", "me"], &["me", "--json", "--json"]] {
        assert_eq!(fixtures::parse(words), Err(Usage::Json), "{words:?}");
    }
    assert_eq!(
        fixtures::back(&fixtures::call(&["sign-in", "--", "--json"])),
        fixtures::owned(&["sign-in", "--", "--json"])
    );
}
#[test]
fn tasks_default_to_number_and_unwritten_orders_are_refused() {
    let mut call = fixtures::call(&["task", "list", "--project", "3"]);
    assert_eq!(call.values[5], Some(Values::One(Value::Order(QueryOrder::Number))));
    for order in [QueryOrder::Changed, QueryOrder::Ended] {
        call.values[5] = Some(Values::One(Value::Order(order)));
        assert_eq!(
            argv::back(crate::table::TOOLS, &call, &mut List::with_capacity(16)),
            Err(Unwritten::NoForm),
            "{order:?}"
        );
    }
}
const ACTS: &[&[&str]] = &[
    &["chat", "start", "hello", "--project", "shop", "--key", "start-chat"],
    &["task", "message", "41", "hello", "--key", "say-hello"],
    &["task", "message", "41", "-", "--key", "stdin"],
    &["chat", "close", "41", "--move", "2", "--move", "3", "--reason", "done", "--key", "close-chat"],
    &["task", "answer", "41", "3", "yes"],
    &["task", "answer", "41", "3", "yes", "--key", "answer"],
    &["proposal", "decide", "t41/3", "accept", "yes"],
    &["proposal", "decide", "p3/17", "reject"],
    &["task", "decide", "41", "leave", "later", "--hold", "2"],
    &["task", "release", "41", "--hold", "2", "--again"],
    &["task", "stop", "41", "--key", "stop"],
    &["task", "cancel", "41", "finished", "--grace", "60"],
];
#[test]
fn each_act_parses_into_its_call_and_prints_back() {
    for words in ACTS {
        let call = fixtures::call(words);
        assert_eq!(fixtures::back(&call), fixtures::owned(words), "{words:?}");
        assert_eq!(fixtures::printed(&call), Parsed::Call { call, json: false }, "{words:?}");
    }
}
const KEY_REQUIRED: &[&[&str]] = &[
    &["chat", "start", "hello", "--project", "shop"],
    &["task", "message", "41", "hello"],
    &["chat", "close", "41"],
    &["repository", "adopt", "--project", "shop", "--owner", "o", "--name", "n", "--landing", "main"],
];
#[test]
fn a_keyed_act_without_its_key_is_a_usage_error() {
    for words in KEY_REQUIRED {
        assert_eq!(fixtures::parse(words), Err(Usage::Key), "{words:?}");
    }
    for key in ["", "Bad", "a_b"] {
        assert_eq!(fixtures::parse(&["task", "message", "41", "hello", "--key", key]), Err(Usage::Key), "{key}");
    }
    assert_eq!(fixtures::parse(&["task", "stop", "41", "--key", "one", "--key", "two"]), Err(Usage::Key));
}
const KEY_NEVER: &[(&[&str], u16)] = &[
    (&["me", "--key", "key"], 0),
    (&["inbox", "--key", "key"], 0),
    (&["task", "list", "--project", "shop", "--key", "key"], 2),
    (&["task", "show", "41", "--key", "key"], 1),
    (&["chat", "show", "41", "--key", "key"], 1),
    (&["task", "activity", "41", "--key", "key"], 1),
    (&["task", "turn", "41", "3.7", "--key", "key"], 2),
    (&["task", "run", "41", "--key", "key"], 1),
    (&["me", "--key", "key", "--until", "changed"], 0),
    (&["sign-in", "file", "--key", "key"], 1),
    (&["sign-out", "--key", "key"], 0),
    (&["pending", "--key", "key"], 0),
    (&["help", "--key", "key"], 0),
];
#[test]
fn a_key_on_a_read_wait_or_host_tool_is_a_usage_error() {
    for (words, at) in KEY_NEVER {
        let error = fixtures::parse(words).expect_err("unkeyed tool");
        assert_eq!(error, Usage::Unknown { at: *at }, "{words:?}");
    }
}
#[test]
fn close_takes_an_optional_reason() {
    for (words, reason) in [
        (&["chat", "close", "41", "--key", "close"][..], None),
        (
            &["chat", "close", "41", "--reason", "done", "--key", "close"][..],
            Some(Values::One(Value::Text(Box::from(*b"done")))),
        ),
    ] {
        assert_eq!(fixtures::call(words).values[2], reason, "{words:?}");
    }
}
const DECISIONS: &[(&[&str], u8)] = &[
    (&["proposal", "decide", "t41/3", "accept"], 0),
    (&["proposal", "decide", "p3/17", "pass-up"], 0),
    (&["task", "decide", "41", "release", "--hold", "2"], 0),
    (&["task", "decide", "41", "leave", "--hold", "2"], 0),
    (&["task", "decide", "41", "pass-up", "--hold", "2"], 0),
];
#[test]
fn decides_two_forms_parse_by_their_arguments() {
    use jig_client::contract::{ProposalKey, Proposer};
    for (words, form) in DECISIONS {
        assert_eq!(fixtures::call(words).form, *form, "{words:?}");
    }
    for (word, proposer, number) in [("t41/3", Proposer::Task(41), 3), ("p3/17", Proposer::Party(3), 17)] {
        assert_eq!(
            fixtures::call(&["proposal", "decide", word, "accept"]).values[0],
            Some(Values::One(Value::Proposal(ProposalKey { proposer, number })))
        );
    }
    assert_eq!(fixtures::parse(&["proposal", "decide", "3/17", "accept"]), Err(Usage::Kind { argument: 0 }));
}
#[test]
fn adopts_flags_parse_and_its_absent_flags_stay_absent() {
    let call = fixtures::adoption();
    for index in [1, 5, 6, 7, 8, 9] {
        assert_eq!(call.values[index], None, "slot {index}");
    }
    let words = &[
        "repository",
        "adopt",
        "--project",
        "3",
        "--forge",
        "65535",
        "--owner",
        "o",
        "--name",
        "n",
        "--landing",
        "main",
        "--role",
        "context",
        "--prefix",
        "x/",
        "--no-home",
        "--no-ci",
        "--check",
        "1",
        "--check",
        "2",
        "--key",
        "adopt",
    ];
    assert_eq!(fixtures::back(&fixtures::call(words)), fixtures::owned(words));
    assert_eq!(
        fixtures::parse(&[
            "repository",
            "adopt",
            "--project",
            "3",
            "--forge",
            "65536",
            "--owner",
            "o",
            "--name",
            "n",
            "--landing",
            "main",
            "--key",
            "adopt"
        ]),
        Err(Usage::Kind { argument: 1 })
    );
}
/// Eight is the delegate/check ceiling; one more must fail before a call is made.
const REPEATS_MOST: u32 = 8;
#[test]
fn repeated_moves_and_checks_stop_at_their_bound() {
    for (base, flag, argument) in
        [(&["chat", "close", "41", "--key", "close"][..], "--move", 1), (fixtures::ADOPTION, "--check", 9)]
    {
        for count in [REPEATS_MOST, REPEATS_MOST.checked_add(1).unwrap()] {
            let mut words = List::with_capacity(32);
            for word in base {
                words.push(*word).unwrap();
            }
            for _ in 0..count {
                words.push(flag).unwrap();
                words.push("1").unwrap();
            }
            let parsed = fixtures::parse(words.as_slice());
            if count == REPEATS_MOST {
                assert!(parsed.is_ok(), "{flag} {count}");
            } else {
                assert_eq!(parsed, Err(Usage::Many { argument }), "{flag} {count}");
            }
        }
    }
}
#[test]
fn a_text_flag_keeps_json_as_its_value() {
    let call = fixtures::call(&["chat", "close", "41", "--reason", "--json", "--key", "close"]);
    assert_eq!(call.values[2], Some(Values::One(Value::Text(Box::from(*b"--json")))));
    assert_eq!(fixtures::printed(&call), Parsed::Call { call, json: false });
}

const LITERAL_TEXT: &[&[&str]] = &[
    &["task", "message", "41", "--key", "literal", "--", "--json"],
    &["chat", "start", "--project", "shop", "--key", "literal", "--", "--words"],
    &["task", "decide", "41", "leave", "--hold", "2", "--", "--reason"],
];
#[test]
fn a_literal_positional_keeps_its_marker_after_the_flags() {
    for words in LITERAL_TEXT {
        let call = fixtures::call(words);
        assert_eq!(fixtures::printed(&call), Parsed::Call { call, json: false }, "{words:?}");
    }
}
/// A tiny text budget exercises both sides without allocating the preset's maximum.
const ASK_BYTES: u64 = 2;
#[test]
fn text_is_bounded_for_argv_and_typed_calls() {
    let mut limits = crate::limits::tools();
    limits.ask_bytes = ASK_BYTES;
    for (text, result) in [("ok", Ok(())), ("bad", Err(Usage::Long { argument: 1 }))] {
        let words: &[&[u8]] = &[b"task", b"message", b"41", text.as_bytes(), b"--key", b"text"];
        let call = fixtures::call(&["task", "message", "41", text, "--key", "text"]);
        assert_eq!(
            match argv::parse(crate::table::TOOLS, words, &limits) {
                Ok(_) => Ok(()),
                Err(error) => Err(error),
            },
            result,
            "{text}"
        );
        assert_eq!(argv::check(crate::table::TOOLS, &call, &limits), result, "{text}");
    }
}

/// Small host limits let every bounded kind reach its edge in one row.
const BOUNDED: Tool = Tool {
    name: b"bounded",
    line: b"",
    prints: b"",
    result: b"",
    forms: &[&[
        crate::jig::table::flag(b"range", b"N", Kind::NumberRange { min: 2, max: 4 }, Occurs::Optional, b""),
        crate::jig::table::flag(b"count", b"K", Kind::Count, Occurs::Optional, b""),
        crate::jig::table::flag(b"forge", b"F", Kind::Number, Occurs::Optional, b""),
        crate::jig::table::flag(b"text", b"TEXT", Kind::Text, Occurs::Optional, b""),
        crate::jig::table::flag(b"timeout", b"D", Kind::Duration, Occurs::Optional, b""),
        crate::jig::table::flag(
            b"named",
            b"K",
            Kind::NamedNumber { words: &[b"settled"], prefixes: &[b"made:"], min: 2, max: 4 },
            Occurs::Optional,
            b"",
        ),
        crate::jig::table::flag(b"literal", b"TEXT", Kind::Literal, Occurs::Optional, b""),
        crate::jig::table::flag(b"items", b"FILE", Kind::File, Occurs::Optional, b""),
        crate::jig::table::flag(b"instructions", b"FILE", Kind::File, Occurs::Optional, b""),
        crate::jig::table::flag(b"spec", b"FILE", Kind::File, Occurs::Optional, b""),
    ]],
    keyed: Keyed::Never,
    area: 1,
    notes: b"",
};
type BoundEdge = (&'static [u8], &'static [u8], &'static [u8], crate::jig::table::Bound, Usage);
#[test]
fn bound_is_what_the_parser_accepts() {
    use crate::jig::table::{Bound, bound};
    let mut limits = crate::limits::tools();
    limits.rows = 2;
    limits.ask_bytes = 3;
    limits.wait = skein_lib::Duration::from_secs(1);
    limits.wait_most = skein_lib::Duration::from_secs(4);
    let edges: &[BoundEdge] = &[
        (b"--range", b"4", b"5", Bound::Numbers { least: 2, most: 4 }, Usage::Kind { argument: 0 }),
        (b"--count", b"2", b"3", Bound::Numbers { least: 1, most: 2 }, Usage::Rows { argument: 1 }),
        (b"--forge", b"65535", b"65536", Bound::Numbers { least: 0, most: 65535 }, Usage::Kind { argument: 2 }),
        (b"--text", b"abc", b"abcd", Bound::Bytes { most: 3 }, Usage::Long { argument: 3 }),
        (b"--timeout", b"4", b"5", Bound::Seconds { most: 4 }, Usage::Rows { argument: 4 }),
        (b"--named", b"made:4", b"made:5", Bound::Numbers { least: 2, most: 4 }, Usage::Kind { argument: 5 }),
        (b"--literal", b"abc", b"abcd", Bound::Bytes { most: 3 }, Usage::Long { argument: 6 }),
        (b"--items", b"abc", b"abcd", Bound::Bytes { most: 3 }, Usage::Long { argument: 7 }),
        (b"--instructions", b"abc", b"abcd", Bound::Bytes { most: 3 }, Usage::Long { argument: 8 }),
        (b"--spec", b"abc", b"abcd", Bound::Bytes { most: 3 }, Usage::Long { argument: 9 }),
    ];
    for (index, (spec, (flag, at, past, expected, error))) in BOUNDED.forms[0].iter().zip(edges).enumerate() {
        assert_eq!(bound(spec, &limits), *expected, "{spec:?}");
        let Parsed::Call { mut call, .. } =
            argv::parse(&[&BOUNDED], &[b"bounded", flag, at], &limits).expect("value at the bound parses")
        else {
            panic!("a call")
        };
        assert_eq!(argv::check(&[&BOUNDED], &call, &limits), Ok(()));
        if spec.kind == Kind::File {
            // argv names a path; the host supplies its text before checking again.
            call.values[index] = Some(Values::One(Value::Text(Box::from(*at))));
            assert_eq!(argv::check(&[&BOUNDED], &call, &limits), Ok(()));
            call.values[index] = Some(Values::One(Value::Text(Box::from(*past))));
        } else {
            assert_eq!(argv::parse(&[&BOUNDED], &[b"bounded", flag, past], &limits), Err(*error));
            let value = if let Bound::Numbers { most, .. } = expected {
                if let Kind::NamedNumber { .. } = spec.kind {
                    Value::Text(Box::from(*past))
                } else {
                    Value::Number(most.checked_add(1).expect("one past the numeric edge"))
                }
            } else if let Bound::Seconds { most } = expected {
                Value::Duration(skein_lib::Duration::from_secs(most.checked_add(1).expect("one past the timeout")))
            } else {
                Value::Text(Box::from(*past))
            };
            call.values[index] = Some(Values::One(value));
        }
        assert_eq!(argv::check(&[&BOUNDED], &call, &limits), Err(*error));
    }
    for (flag, least, below) in [
        (b"--range".as_slice(), b"2".as_slice(), b"1".as_slice()),
        (b"--count", b"1", b"0"),
        (b"--named", b"made:2", b"made:1"),
    ] {
        argv::parse(&[&BOUNDED], &[b"bounded", flag, least], &limits).expect("inclusive lower bound");
        argv::parse(&[&BOUNDED], &[b"bounded", flag, below], &limits).expect_err("below the lower bound");
    }
    argv::parse(&[&BOUNDED], &[b"bounded", b"--forge", b"0"], &limits).expect("forge zero stays accepted");
    for kind in [Kind::Number, Kind::File, Kind::Duration, Kind::Word { set: &[b"word"] }] {
        assert_eq!(bound(&positional(b"other", b"VALUE", kind, Occurs::Required, b""), &limits), Bound::None);
    }
}
