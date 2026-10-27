#![expect(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    reason = "ordinary Rust assertions over the static table and help snapshots; testing-strategy.md step tests"
)]
use super::fixtures;
use crate::{
    help,
    jig::{
        self,
        argv::Usage,
        call::{Value, Values},
        table::{Kind, PLACEHOLDERS, Place},
    },
    table,
};

#[test]
fn every_tool_names_a_group() {
    for row in table::TOOLS {
        let words: Vec<_> = row.name.split(|byte| *byte == b' ').collect();
        assert!((1..=2).contains(&words.len()) && words.iter().all(|word| !word.is_empty()), "{row:?}");
        assert!(table::GROUPS.iter().any(|group| group.name == words[0]), "{row:?}");
    }
    for group in table::GROUPS {
        let rows: Vec<_> = table::TOOLS.iter().filter(|row| jig::table::name_words(row.name).0 == group.name).collect();
        assert!(!rows.is_empty(), "{group:?}");
        if rows.iter().any(|row| row.name == group.name) {
            assert!(rows.iter().all(|row| row.name == group.name), "{group:?}");
        }
    }
}

#[test]
fn every_shown_is_in_the_vocabulary() {
    for row in table::TOOLS {
        for form in row.forms {
            for argument in *form {
                let valid = if argument.place == Place::Switch {
                    argument.shown.is_empty()
                } else if argument.kind == Kind::Group {
                    argument.shown
                        == [
                            b"questions".as_slice(),
                            b"proposals",
                            b"escalations",
                            b"person-tasks",
                            b"results",
                            b"replies",
                        ]
                        .join(&b'|')
                } else if argument.kind == Kind::Order {
                    argument.shown == b"activity|priority"
                } else if argument.name == b"until" {
                    argument.shown == b"CONDITION"
                } else if let Kind::Word { set } = argument.kind {
                    argument.shown == set.join(&b'|')
                } else {
                    PLACEHOLDERS.contains(&argument.shown) || {
                        let pair: Vec<_> = argument.shown.split(|byte| *byte == b'/').collect();
                        pair.len() == 2 && pair.iter().all(|word| PLACEHOLDERS.contains(word))
                    }
                };
                assert!(
                    valid,
                    "{}: {} shows {}",
                    String::from_utf8_lossy(row.name),
                    String::from_utf8_lossy(argument.name),
                    String::from_utf8_lossy(argument.shown)
                );
            }
        }
    }
}

type ArgvCase = (&'static [&'static str], u8, &'static [Option<Values>]);

#[test]
fn argv_takes_a_group_and_a_verb() {
    let cases: &[ArgvCase] = &[
        (
            &["task", "show", "41"],
            jig::rows::TASK_AT,
            &[Some(Values::One(Value::Number(41))), None, None, None, None, None, None],
        ),
        (&["forge", "setup", "3", "--key", "k"], 35, &[Some(Values::One(Value::Number(3))), None, None, None, None]),
        (&["me"], jig::rows::ME_AT, &[None, None, None]),
        (
            &["settings", "show", "charter", "2"],
            jig::rows::SETTING_AT,
            &[Some(Values::One(Value::Word(0))), Some(Values::One(Value::Number(2))), None],
        ),
        (&["settings", "show"], jig::rows::SETTINGS_AT, &[]),
        (
            &["task", "decide", "4", "--hold", "2", "release"],
            jig::rows::TASK_DECIDE_AT,
            &[
                Some(Values::One(Value::Number(4))),
                Some(Values::One(Value::Number(2))),
                Some(Values::One(Value::Word(0))),
                None,
            ],
        ),
    ];
    for (words, tool, values) in cases {
        let call = fixtures::call(words);
        assert_eq!(call.tool, *tool, "{words:?}");
        assert_eq!(call.form, 0, "{words:?}");
        assert_eq!(call.values.as_ref(), *values, "{words:?}");
    }
    let proposal = fixtures::call(&["proposal", "decide", "t4/1", "accept"]);
    assert_eq!(proposal.tool, jig::rows::DECIDE_AT);
    assert_eq!(
        proposal.values.as_ref(),
        &[
            Some(Values::One(Value::Proposal(jig_client::contract::ProposalKey {
                proposer: jig_client::contract::Proposer::Task(4),
                number: 1
            }))),
            Some(Values::One(Value::Word(0))),
            None,
            None,
        ]
    );
    for (words, expected) in [
        (&["tasks", "--project", "1"][..], "temper ctl: no group tasks; temper ctl help lists them\n"),
        (&["forge-setup", "3"], "temper ctl: no group forge-setup; temper ctl help lists them\n"),
        (&["task", "41"], "temper ctl: no tool task 41; temper ctl help task lists them\n"),
        (&["task"], "temper ctl: task needs a verb; temper ctl help task lists them\n"),
    ] {
        assert_eq!(refusal(words).as_ref(), expected.as_bytes(), "{words:?}");
    }
}

#[test]
fn call_json_names_the_words() {
    let call = fixtures::from_json(br#"{"tool":"task show","arguments":{"task":41}}"#);
    assert_eq!(call, fixtures::call(&["task", "show", "41"]));
    assert_eq!(fixtures::json(br#"{"tool":"task","arguments":{"task":41}}"#), Err(Usage::Tool));
}

fn refusal(words: &[&str]) -> Box<[u8]> {
    let why = fixtures::parse(words).expect_err("refused words");
    let words: Vec<_> = words.iter().map(|word| word.as_bytes()).collect();
    let mut measured = skein_lib::Writer::new(1024);
    write_refusal(why, &words, &mut measured);
    let mut exact = skein_lib::Writer::new(measured.written());
    write_refusal(why, &words, &mut exact);
    exact.finish()
}
fn write_refusal(why: Usage, words: &[&[u8]], out: &mut skein_lib::Writer) {
    out.put(b"temper ctl: ").expect("prefix");
    table::refusal(why, words, out).expect("refusal");
    out.put(b"\n").expect("newline");
}

#[test]
fn help_lists_groups_then_a_group_then_a_tool() {
    for (words, golden) in [
        (&["help"][..], "help_groups"),
        (&["help", "task"], "help_task"),
        (&["help", "task", "show"], "help_task_show"),
        (&["help", "me"], "help_me"),
    ] {
        let call = fixtures::call(words);
        let selected = help::selection(&call).expect("help selection");
        fixtures::check_golden(golden, &fixtures::selected_help(selected));
    }
    for (words, golden) in [(&["help", "bogus"][..], "help_bogus"), (&["help", "task", "bogus"], "help_task_bogus")] {
        fixtures::check_golden(golden, &refusal(words));
    }
}

// Every record in 04c4bf7bb, in its durable order. New rows append.
const KEPT_INDICES: &[&[u8]] = &[
    b"me",
    b"inbox",
    b"task list",
    b"task show",
    b"chat show",
    b"task activity",
    b"task turn",
    b"task run",
    b"chat start",
    b"task message",
    b"chat close",
    b"task answer",
    b"proposal decide",
    b"task release",
    b"task stop",
    b"task cancel",
    b"task tree",
    b"repository adopt",
    b"sign-in",
    b"sign-out",
    b"pending",
    b"help",
    b"account list",
    b"model list",
    b"account connect",
    b"account disconnect",
    b"account add-key",
    b"account replace-key",
    b"account forget",
    b"account rank",
    b"account retry",
    b"task amend",
    b"account declare",
    b"account verify",
    b"forge list",
    b"forge setup",
    b"playground start",
    b"forge connect",
    b"forge install",
    b"forge repositories",
    b"account export",
    b"account import",
    b"settings show",
    b"settings show",
    b"revision show",
    b"revision list",
    b"settings apply",
    b"settings propose",
    b"proposal show",
    b"change show",
    b"forge accept",
    b"revision advance",
];
#[test]
fn indices_are_kept() {
    for (index, name) in KEPT_INDICES.iter().enumerate() {
        assert_eq!(table::TOOLS[index].name, *name, "old index {index}");
    }
    assert_eq!(table::TOOLS[usize::from(jig::rows::TASK_DECIDE_AT)].name, b"task decide");
    for (index, group) in table::GROUPS.iter().enumerate() {
        if table::TOOLS.iter().any(|row| row.name == group.name) {
            continue;
        }
        let mut expected = Vec::new();
        for row in table::TOOLS {
            if jig::table::name_words(row.name).0 == group.name {
                expected.extend_from_slice(&fixtures::usage(row));
                expected.extend_from_slice(b"  ");
                expected.extend_from_slice(row.line);
                expected.push(b'\n');
            }
        }
        assert_eq!(
            &*fixtures::selected_help(help::Selection::Group(u8::try_from(index).expect("group index fits"))),
            expected,
            "{group:?}"
        );
    }
}

const WATCHED: &[&[&str]] = &[
    &["me"],
    &["inbox"],
    &["task", "show", "1"],
    &["task", "tree", "1"],
    &["task", "run", "1"],
    &["task", "list", "--project", "1"],
    &["account", "list"],
    &["forge", "list"],
    &["change", "show", "1"],
];
#[test]
fn each_watched_read_waits_given_since_or_until() {
    for read in WATCHED {
        for flags in [&["--until", "changed"][..], &["--since", "0123456789abcdef"], &[]] {
            let words = fixtures::extended(read, flags);
            let call = fixtures::call(words.as_slice());
            let row = table::TOOLS[usize::from(call.tool)];
            assert_eq!(jig::call::waiting(row, &call), !flags.is_empty(), "{words:?}");
            assert_eq!(fixtures::printed(&call), jig::call::Parsed::Call { call: call.clone(), json: false });
            let encoded = jig::json::write_call(table::TOOLS, &call).unwrap();
            assert_eq!(fixtures::from_json(&encoded), call);
        }
    }
}
#[test]
fn a_read_with_parts_refuses_a_wait() {
    for (words, expected) in [
        (
            &["inbox", "--kind", "needs", "--until", "waiting"][..],
            "temper ctl: inbox waits on its subject alone: drop --kind\n",
        ),
        (
            &["task", "show", "1", "--tree", "--until", "ended"],
            "temper ctl: task show waits on its subject alone: drop --tree\n",
        ),
        (
            &["task", "list", "--project", "1", "--ended", "--until", "changed"],
            "temper ctl: task list waits on its subject alone: drop --ended\n",
        ),
        (
            &["task", "show", "1", "--run", "--tree", "--until", "ended"],
            "temper ctl: task show waits on its subject alone: drop --run\n",
        ),
        (
            &["inbox", "--root", "1", "--kind", "needs", "--until", "waiting"],
            "temper ctl: inbox waits on its subject alone: drop --root\n",
        ),
    ] {
        assert_eq!(refusal(words).as_ref(), expected.as_bytes());
    }
}
#[test]
fn timeout_alone_is_refused() {
    assert_eq!(refusal(&["me", "--timeout", "5s"]).as_ref(), b"temper ctl: --timeout needs --since or --until\n");
    for read in WATCHED {
        let words = fixtures::extended(read, &["--timeout", "5s"]);
        assert_eq!(fixtures::parse(words.as_slice()), Err(Usage::Timeout));
    }
    assert_eq!(fixtures::json(br#"{"tool":"me","arguments":{"timeout":5}}"#), Err(Usage::Timeout));
}
#[test]
fn no_tool_is_named_wait() {
    assert!(table::TOOLS.iter().all(|row| row.name != b"wait"));
    assert!(table::GROUPS.iter().all(|row| row.name != b"wait"));
    assert_eq!(refusal(&["wait", "me"]).as_ref(), b"temper ctl: no group wait; temper ctl help lists them\n");
    assert_eq!(fixtures::json(br#"{"tool":"wait","arguments":{"subject":"me"}}"#), Err(Usage::Tool));
}
