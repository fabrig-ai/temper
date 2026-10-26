use super::fixtures;
use crate::{guide, jig, table};
use skein_lib::List;
#[test]
fn every_call_line_of_the_guide_parses_with_the_table() {
    let mut calls = 0;
    for line in &fixtures::guide_lines() {
        if let Some(call) = line.trim_ascii_start().strip_prefix(b"temper ctl DIRECTORY ".as_slice()) {
            fixtures::parse_line(call).expect("each call of the guide parses with the table");
            calls += 1;
        }
    }
    assert_eq!(calls, fixtures::GUIDE_CALLS, "the guide shows the loop's calls");
}
const GOLDENS: &[(Option<u8>, &str)] = &[
    (None, "help"),
    (Some(12), "help_decide"),
    (Some(10), "help_close"),
    (Some(16), "help_wait"),
    (Some(20), "help_pending"),
    (Some(17), "help_adopt"),
    (Some(31), "help_amend"),
    (Some(37), "help_forge_connect"),
    (Some(38), "help_forge_install"),
    (Some(jig::rows::EXPORT_AT), "help_export"),
    (Some(jig::rows::IMPORT_AT), "help_import"),
    (Some(jig::rows::SETTINGS_AT), "help_settings"),
    (Some(jig::rows::SETTING_AT), "help_setting"),
    (Some(jig::rows::REVISION_AT), "help_revision"),
    (Some(jig::rows::REVISIONS_AT), "help_revisions"),
    (Some(jig::rows::CHANGE_SETTINGS_AT), "help_change_settings"),
    (Some(jig::rows::PROPOSE_SETTINGS_AT), "help_propose_settings"),
    (Some(jig::rows::PROPOSAL_AT), "help_proposal"),
    (Some(jig::rows::MOVE_TO_NEWEST_AT), "help_move_to_newest"),
];
#[test]
fn the_help_prints_as_its_golden_files() {
    for (tool, name) in GOLDENS {
        if std::env::var_os("SKEIN_CODEGEN_WRITE").is_some() {
            std::fs::write(
                std::path::Path::new("src/tests/golden").join(name).with_extension("txt"),
                fixtures::help(*tool),
            )
            .expect("write help golden");
        }
        assert_eq!(fixtures::help(*tool), fixtures::golden(name), "{name}");
    }
}
// client/tools.md 5.2--5.6, with the plan's typed proposals and close reason.
const USAGES: &[&[u8]] = &[
    b"me\n",
    b"inbox [--project P] [--kind K] [--root N] [--after C]\n",
    b"tasks --project P [--label chat|goal] [--tracked] [--mine] [--ended] [--order activity|priority] [--after C] [--parent N]\n",
    b"task N [--tree] [--activity K] [--run]\n",
    b"chat N [--before C]\n",
    b"activity N [--before C] [--limit K]\n",
    b"turn N A.T [--part K] [--from B]\n",
    b"run N\n",
    b"chat --project P TEXT [--model M] [--effort E] --key NAME\n",
    b"message N TEXT --key NAME\n",
    b"close N [--move D]... [--reason TEXT] --key NAME\n",
    b"answer N M TEXT [--key NAME]\n",
    b"decide tN/K|pN/K accept|reject|pass-up [REASON] [--running stays|moves] [--key NAME]\ndecide N --hold R release|leave|pass-up [REASON] [--key NAME]\n",
    b"release N --hold R [--again] [--key NAME]\n",
    b"stop N [--key NAME]\n",
    b"cancel N [--grace D] REASON [--key NAME]\n",
    b"wait SUBJECT [--since DIGEST] [--until CONDITION] [--timeout DURATION]\nwait forges [--since DIGEST] [--until settled|connected:F|made:F] [--timeout DURATION]\nwait change N [--project P] [--since DIGEST] [--until pull|ci:pending|ci:passed|ci:failed|held|landed] [--timeout DURATION]\n",
    b"adopt --project P [--forge F] --owner O --name N --landing B [--role owned|fork|context] [--prefix X] [--no-home] [--no-ci] [--check G]... --key NAME\n",
    b"sign-in FILE|-\n",
    b"sign-out\n",
    b"pending\n",
    b"help [TOOL]\n",
    b"accounts\n",
    b"models [--project P]\n",
    b"connect PROVIDER --key NAME\n",
    b"disconnect ACCOUNT [--source sign_in|key|program] [--key NAME]\n",
    b"add-key PROVIDER FILE [--account A] [--deployment] --key NAME\n",
    b"replace-key ACCOUNT FILE [--key NAME]\n",
    b"forget ACCOUNT [--source sign_in|key|program] [--key NAME]\n",
    b"rank ACCOUNT POSITION [--source sign_in|key|program] [--key NAME]\n",
    b"retry ACCOUNT [--source sign_in|key|program] [--key NAME]\n",
    b"amend N [--rev R] [--priority P | --untracked] [--deadline T | --no-deadline] [--instructions FILE] [--spec FILE] [--dependency M]... [--no-dependencies] [--model M [--effort E]] REASON [--key NAME]\n",
    b"declare PROVIDER ADAPTER NAME [--account A] --key NAME\n",
    b"verify ACCOUNT [--source sign_in|key|program] [--key NAME]\n",
    b"forges\n",
    b"forge-setup F [--org O] [--repository R] [--project P] [--playground] [--key NAME]\n",
    b"demo-start [--key NAME]\n",
    b"forge-connect F github-app ID FILE --key NAME\nforge-connect F github --key NAME\nforge-connect F forgejo FILE --key NAME\nforge-connect F forgejo-admin FILE [--first ORG/NAME] [--project P] [--label L] --key NAME\n",
    b"forge-install F [--key NAME]\n",
    b"repositories F [--installation I] [--page N]\n",
    b"export ACCOUNT FILE\n",
    b"import FILE --key NAME\n",
    b"settings\n",
    b"setting charter|template|policy N [--revision R]\nsetting rules\n",
    b"revision R\n",
    b"revisions [--before R]\n",
    b"change-settings FILE --against R [--running stays|moves] --key NAME\n",
    b"propose-settings FILE [REASON] --against R --key NAME\n",
    b"proposal tN/K|pN/K\n",
    b"change N [--project P]\n",
    b"forge-accept F I [--key NAME]\n",
    b"move-to-newest R --key NAME\n",
];
#[test]
fn a_tools_usage_reads_as_the_design_writes_it() {
    assert_eq!(table::TOOLS.len(), USAGES.len());
    for (tool, expected) in table::TOOLS.iter().zip(USAGES) {
        assert_eq!(&*fixtures::usage(tool), *expected, "{tool:?}");
    }
}
#[test]
fn each_derivation_names_every_tool() {
    let mut forms = List::with_capacity(fixtures::TOOL_FORMS);
    for words in fixtures::EVERY_TOOL {
        let mut call = fixtures::call(words);
        if call.tool == jig::rows::CHANGE_SETTINGS_AT || call.tool == jig::rows::PROPOSE_SETTINGS_AT {
            *call.values.get_mut(0).expect("FILE") =
                Some(jig::call::Values::One(jig::call::Value::Text(br#"{"items":[]}"#.as_slice().into())));
        }
        forms.push((call.tool, call.form)).expect("all table forms");
        let json = jig::json::write_call(table::TOOLS, &call).expect("call JSON");
        assert_eq!(fixtures::from_json(&json), call, "{words:?}");
    }
    assert_eq!(forms.len(), fixtures::TOOL_FORMS);
    let help = fixtures::help(None);
    let list = help.strip_prefix(guide::GUIDE).expect("the guide precedes the generated list");
    let mut expected = skein_lib::Writer::new(list.len());
    for (index, tool) in table::TOOLS.iter().enumerate() {
        for form in 0..tool.forms.len() {
            let pair = (u8::try_from(index).expect("row fits"), u8::try_from(form).expect("form fits"));
            let mut count = 0_u32;
            for value in &forms {
                if *value == pair {
                    count += 1;
                }
            }
            assert_eq!(count, fixtures::EXAMPLES_PER_FORM, "{tool:?} {form}");
        }
        let record = jig::json::write_tool(tool).expect("tool record JSON");
        let prefix_length = b"{\"name\":\""
            .len()
            .checked_add(tool.name.len())
            .expect("record prefix size")
            .checked_add(1)
            .expect("record prefix size");
        let mut prefix = skein_lib::Writer::new(prefix_length);
        prefix.put(br#"{"name":""#).expect("record prefix");
        prefix.put(tool.name).expect("tool name");
        prefix.put(b"\"").expect("end name");
        assert!(record.starts_with(&prefix.finish()), "{tool:?}");
        expected.put(&fixtures::usage(tool)).expect("usage list");
        expected.put(b"  ").expect("indent");
        expected.put(tool.line).expect("description");
        expected.put(b"\n").expect("newline");
    }
    assert_eq!(list, &*expected.finish(), "the list has exactly the table's rows");
    for line in &fixtures::guide_lines() {
        if let Some(call) = line.trim_ascii_start().strip_prefix(b"temper ctl DIRECTORY ".as_slice()) {
            fixtures::parse_line(call).expect("a prose example names a table member");
        }
    }
}
const EXITS: &[&[u8]] = &[
    b"0: a read answered, an act done or proposed, or a wait met.",
    b"1: usage or client failure, including limits, Held, NotKept and unknown past retention.",
    b"2: an engine refusal alone, including an engine key conflict.",
    b"3: an unmet wait's timeout or watch end.",
    b"Unconfirmed or a client-side key conflict is a tools bug (exit 1); report it as a finding.",
    b"Held may name another party's parked request; it is a client failure.",
];
#[test]
fn the_guide_distinguishes_engine_refusals_from_client_failures() {
    for expected in EXITS {
        assert!(skein_lib::bytes::find(guide::GUIDE, expected).is_some(), "{expected:?}");
    }
}
#[test]
fn the_guide_preserves_the_sized_turn_exit_status() {
    fixtures::guide_golden_has(&[b"A sized too-large turn stays\n   exit 0; report it as a finding"]);
}
#[test]
fn the_guide_preserves_the_operating_system_signal_status() {
    fixtures::guide_golden_has(&[b"A signal's status is the OS's, never 0."]);
}
#[test]
fn the_guide_waits_for_the_link_deadline_during_reconnect() {
    fixtures::guide_golden_has(&[
        b"A stopping engine's wait continues through reconnect and fails only past the link deadline.",
    ]);
}
#[test]
fn the_guide_shows_kept_work() {
    fixtures::guide_golden_has(&[
        b"kept: create done: task 41, chat",
        b"A kept JSON notice has only operation and outcome, one object per stderr line, never a request name or key.",
    ]);
}
#[test]
fn the_guide_shows_pending_work() {
    fixtures::guide_golden_has(&[
        b"chat, party 3, not sent",
        br#"{"pending":[{"operation":"chat","party":3,"first_sent":null}],"unread":2}"#,
        b"2 kept records these tools cannot read; a call that sends drops them",
    ]);
}
#[test]
fn the_guide_keeps_pending_local() {
    fixtures::guide_golden_has(&[b"pending reads only the store, with no lock, client or send."]);
}
#[test]
fn the_guide_refuses_an_invalid_store() {
    fixtures::guide_golden_has(&[b"An invalid store fails: DIRECTORY's store cannot be read (exit 1)."]);
}
#[test]
fn the_guide_shows_dropped_work() {
    fixtures::guide_golden_has(&[b"dropped: 2 kept records these tools cannot read", br#"{"dropped":2}"#]);
}
#[test]
fn the_guide_shows_parked_work() {
    fixtures::guide_golden_has(&[
        b"kept: 2 for party 3, unsent while it is not signed in here",
        br#"{"party":3,"unsent":2}"#,
        b"It answers and erases only the signed-in party's kept requests; other parties' work stays parked.",
    ]);
}
#[test]
fn the_guide_omits_request_identity() {
    let golden = fixtures::golden("help");
    for forbidden in [br#""name":"investigate-41""#.as_slice(), br#""key":"#.as_slice(), b"kept: investigate-41"] {
        assert!(skein_lib::bytes::find(&golden, forbidden).is_none(), "{forbidden:?}");
    }
}

const CHAT_READ: &[u8] = b"chat N [--before C]\nRead a conversation from its end\nResult: chat\n  task: Task number\n  before: Continue before this cursor\n";
const CHAT_CREATE: &[u8] = b"chat --project P TEXT [--model M] [--effort E] --key NAME\nStart a chat\nResult: chat_create\n  project: Project name or number\n  text: Opening words\n  model: Configured model name\n  effort: Effort offered by that model\n";
#[test]
fn help_includes_each_later_row_of_its_name() {
    // The table's read-chat and create-chat rows, in its published order.
    for (row, records) in [(4, &[CHAT_READ, CHAT_CREATE][..]), (8, &[CHAT_CREATE][..])] {
        let mut length = 0_usize;
        for record in records {
            length = length.checked_add(record.len()).expect("chat records fit");
        }
        let mut expected = skein_lib::Writer::new(length);
        for record in records {
            expected.put(record).expect("chat record length");
        }
        assert_eq!(fixtures::help(Some(row)), expected.finish(), "row {row}");
    }
}
#[test]
fn help_refuses_a_writer_one_byte_short() {
    let bytes = fixtures::help(None);
    let short = bytes.len().checked_sub(fixtures::HELP_SHORTFALL).expect("nonempty guide");
    let mut out = skein_lib::Writer::new(short);
    assert_eq!(crate::help::text(None, &mut out), Err(skein_lib::Overflow));
    assert_eq!(out.written(), fixtures::NO_HELP_BYTES, "overflow does not publish a partial help");
}
#[test]
fn the_moves_places_name_their_rows_in_the_table() {
    for (at, row) in [(jig::rows::EXPORT_AT, &jig::rows::EXPORT), (jig::rows::IMPORT_AT, &jig::rows::IMPORT)] {
        assert_eq!(table::TOOLS[usize::from(at)].name, row.name);
    }
}

#[test]
fn every_call_line_of_the_task_settings_topic_parses() {
    let source = std::fs::read_to_string("../../docs/guide/tasks.md").expect("tasks guide");
    let mut calls = 0_u32;
    for line in source.lines() {
        if let Some(call) = line.as_bytes().strip_prefix(b"temper ctl me ".as_slice()) {
            fixtures::parse_line(call).expect("tasks guide call parses");
            calls += 1;
        }
    }
    assert_eq!(calls, 15);
}
