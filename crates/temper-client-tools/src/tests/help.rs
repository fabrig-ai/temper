use super::fixtures;
use crate::{jig, table};
use skein_lib::List;
const GOLDENS: &[(Option<u8>, &str)] = &[
    (Some(3), "help_task"),
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
fn the_help_is_the_page_ctl_then_the_tools() {
    let help = fixtures::help(None);
    let page = temper_docs::find(b"ctl").expect("ctl page").text.as_bytes();
    let tools = help.strip_prefix(page).expect("the ctl page precedes the tools");
    fixtures::check_golden("help", tools);
}
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
    let list = help
        .strip_prefix(temper_docs::find(b"ctl").expect("ctl page").text.as_bytes())
        .expect("the ctl page precedes the generated list");
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
        let record = jig::json::write_tool(tool, &crate::limits::tools()).expect("tool record JSON");
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
}

const CHAT_READ: &[u8] = b"chat N [--before C]\n  N (task): a number; required. The chat's task number\n  --before C: a cursor, as a read prints it; optional. The cursor a page printed, to read further back\nRead a chat's conversation from its end\nPrints: The conversation, oldest line first: each turn's words, each run's start and end, then `read on with: chat N --before C` or `no more`\n--json: the record chat.\nKey: none.\nEach page is older than the one before it. A run still writing\nadds its words so far.\n";
const CHAT_CREATE: &[u8] = b"chat --project P TEXT [--model M] [--effort E] --key NAME\n  --project P: a project's name or number; required. The project\n  TEXT (text): text, at most 1048576 bytes, or - to read stdin; required. The opening words\n  --model M: text, at most 1048576 bytes, or - to read stdin; optional. A model the chat may use, as models lists it\n  --effort E: text, at most 1048576 bytes, or - to read stdin; optional. An effort that model offers\nStart a chat in a project\nPrints: `done: task N, chat`, or `proposed: P/K, to RECIPIENT` when another party must agree\n--json: the record chat_create.\nKey: --key NAME, required.\nThe chat's template chooses its model unless `--model` names one;\n`--effort` needs `--model`, and a model that declares efforts needs\n`--effort`. A chat past what your party may start, but within what\nit may propose, is proposed (exit 0). `chat not offered` and `chat\nexceeds offered bounds` (exit 1): your party may not start it.\nThen `wait task N --until replied`.\n";
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
    let short = bytes.len().checked_sub(fixtures::HELP_SHORTFALL).expect("nonempty help");
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

pub(super) const RECORD: jig::table::Tool = jig::table::Tool {
    name: b"fixture",
    line: b"Read a task",
    prints: b"task N",
    result: b"task",
    forms: &[
        &[
            jig::table::positional(
                b"task",
                b"N",
                jig::table::Kind::Number,
                jig::table::Occurs::Required,
                b"Task number",
            ),
            jig::table::flag(
                b"activity",
                b"K",
                jig::table::Kind::Count,
                jig::table::Occurs::Optional,
                b"Latest activity lines",
            ),
            jig::table::switch(b"tree", b"Include the live tree"),
            jig::table::repeated(b"word", b"K", jig::table::Kind::Word { set: &[b"a", b"b", b"c"] }, 2, b"Words"),
        ],
        &[
            jig::table::positional(
                b"text",
                b"TEXT",
                jig::table::Kind::Text,
                jig::table::Occurs::Required,
                b"Words to send",
            ),
            jig::table::flag(
                b"until",
                b"K",
                jig::table::Kind::NamedNumber {
                    words: &[b"settled"],
                    prefixes: &[b"connected:", b"made:"],
                    min: 1,
                    max: 65535,
                },
                jig::table::Occurs::Optional,
                b"Condition",
            ),
            jig::table::flag(b"timeout", b"D", jig::table::Kind::Duration, jig::table::Occurs::Optional, b"Timeout"),
        ],
    ],
    keyed: jig::table::Keyed::Optional,
    area: 1,
    notes: b"First note.\nSecond note.\n",
};
const RECORD_FORMS: &[u8] = b"fixture N [--activity K] [--tree] [--word a|b|c]... [--key NAME]\n  N (task): a number; required. Task number\n  --activity K: a count from 1 to 32; optional. Latest activity lines\n  --tree: a switch. Include the live tree\n  --word K: one of a, b or c; repeated, at most 2. Words\nfixture TEXT [--until K] [--timeout D] [--key NAME]\n  TEXT (text): text, at most 1048576 bytes, or - to read stdin; required. Words to send\n  --until K: one of settled, connected:N or made:N with N from 1 to 65535; optional. Condition\n  --timeout D: a duration: seconds, or a number with s, m or h, at most 3600 seconds; optional. Timeout\n";
#[test]
fn a_tools_record_reads_as_written() {
    let expected = expected_record(b"Read a task\nPrints: task N\n--json: the record task.\nKey: --key NAME, optional.\nFirst note.\nSecond note.\n");
    let mut out = skein_lib::Writer::new(expected.len());
    jig::help::tool(&RECORD, &crate::limits::tools(), &mut out).expect("fixture record fits");
    assert_eq!(out.finish(), expected);
}
#[test]
fn an_empty_prints_and_notes_are_left_out() {
    let record = jig::table::Tool { prints: b"", notes: b"", ..RECORD };
    let expected = expected_record(b"Read a task\n--json: the record task.\nKey: --key NAME, optional.\n");
    let mut out = skein_lib::Writer::new(expected.len());
    jig::help::tool(&record, &crate::limits::tools(), &mut out).expect("fixture record fits");
    assert_eq!(out.finish(), expected);
}
#[test]
fn each_row_keeps_its_notes_within_their_bound() {
    for row in table::TOOLS {
        assert!(row.notes.len() <= jig::table::NOTES_BYTES, "{row:?}");
        assert!(row.notes.is_empty() || row.notes.ends_with(b"\n"), "{row:?}");
    }
}

fn expected_record(ending: &[u8]) -> Box<[u8]> {
    let mut expected = skein_lib::Writer::new(RECORD_FORMS.len().checked_add(ending.len()).expect("record size"));
    expected.put(RECORD_FORMS).expect("forms");
    expected.put(ending).expect("ending");
    expected.finish()
}

#[test]
fn each_row_says_what_it_does_and_prints() {
    for row in table::TOOLS {
        assert!(!row.line.is_empty(), "{row:?}");
        assert!(!row.prints.is_empty(), "{row:?}");
        for form in row.forms {
            for argument in *form {
                assert!(!argument.line.is_empty(), "{row:?}: {argument:?}");
            }
        }
    }
}
