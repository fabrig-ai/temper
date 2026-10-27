use super::fixtures;
use crate::{jig, table};
use skein_lib::List;
const GOLDENS: &[(Option<u8>, &str)] = &[
    (Some(3), "help_task_show"),
    (Some(12), "help_proposal_decide"),
    (Some(jig::rows::TASK_DECIDE_AT), "help_task_decide"),
    (Some(10), "help_chat_close"),
    (Some(16), "help_wait"),
    (Some(20), "help_pending"),
    (Some(17), "help_repository_adopt"),
    (Some(31), "help_task_amend"),
    (Some(37), "help_forge_connect"),
    (Some(38), "help_forge_install"),
    (Some(jig::rows::EXPORT_AT), "help_account_export"),
    (Some(jig::rows::IMPORT_AT), "help_account_import"),
    (Some(jig::rows::SETTINGS_AT), "help_settings_show"),
    (Some(jig::rows::SETTING_AT), "help_settings_show"),
    (Some(jig::rows::REVISION_AT), "help_revision_show"),
    (Some(jig::rows::REVISIONS_AT), "help_revision_list"),
    (Some(jig::rows::CHANGE_SETTINGS_AT), "help_settings_apply"),
    (Some(jig::rows::PROPOSE_SETTINGS_AT), "help_settings_propose"),
    (Some(jig::rows::PROPOSAL_AT), "help_proposal_show"),
    (Some(jig::rows::MOVE_TO_NEWEST_AT), "help_revision_advance"),
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
    b"inbox [--project PROJECT] [--kind needs|updates] [--root TASK] [--after CURSOR]\n",
    b"task list --project PROJECT [--label chat|goal] [--tracked] [--mine] [--ended] [--order activity|priority] [--after CURSOR] [--parent TASK]\n",
    b"task show TASK [--tree] [--activity COUNT] [--run]\n",
    b"chat show TASK [--before CURSOR]\n",
    b"task activity TASK [--before CURSOR] [--limit COUNT]\n",
    b"task turn TASK TURN [--part PART] [--from OFFSET]\n",
    b"task run TASK\n",
    b"chat start --project PROJECT TEXT [--model MODEL] [--effort EFFORT] --key KEY\n",
    b"task message TASK TEXT --key KEY\n",
    b"chat close TASK [--move TASK]... [--reason TEXT] --key KEY\n",
    b"task answer TASK QUESTION TEXT [--key KEY]\n",
    b"proposal decide PROPOSAL accept|reject|pass-up [REASON] [--running stays|moves] [--key KEY]\n",
    b"task release TASK --hold REV [--again] [--key KEY]\n",
    b"task stop TASK [--key KEY]\n",
    b"task cancel TASK [--grace DURATION] REASON [--key KEY]\n",
    b"wait  [--since DIGEST] [--until CONDITION] [--timeout DURATION]\nwait forges [--since DIGEST] [--until CONDITION] [--timeout DURATION]\nwait change TASK [--project PROJECT] [--since DIGEST] [--until pull|ci:pending|ci:passed|ci:failed|held|landed] [--timeout DURATION]\n",
    b"repository adopt --project PROJECT [--forge FORGE] --owner OWNER --name REPOSITORY --landing BRANCH [--role owned|fork|context] [--prefix PREFIX] [--no-home] [--no-ci] [--check GATE]... --key KEY\n",
    b"sign-in FILE\n",
    b"sign-out\n",
    b"pending\n",
    b"help [GROUP [VERB]]\n",
    b"account list\n",
    b"model list [--project PROJECT]\n",
    b"account connect PROVIDER --key KEY\n",
    b"account disconnect ACCOUNT [--source sign_in|key|program] [--key KEY]\n",
    b"account add-key PROVIDER FILE [--account ACCOUNT] [--deployment] --key KEY\n",
    b"account replace-key ACCOUNT FILE [--key KEY]\n",
    b"account forget ACCOUNT [--source sign_in|key|program] [--key KEY]\n",
    b"account rank ACCOUNT POSITION [--source sign_in|key|program] [--key KEY]\n",
    b"account retry ACCOUNT [--source sign_in|key|program] [--key KEY]\n",
    b"task amend TASK [--rev REV] [--priority PRIORITY | --untracked] [--deadline TIME | --no-deadline] [--instructions FILE] [--spec FILE] [--dependency TASK]... [--no-dependencies] [--model MODEL [--effort EFFORT]] REASON [--key KEY]\n",
    b"account declare PROVIDER ADAPTER NAME [--account ACCOUNT] --key KEY\n",
    b"account verify ACCOUNT [--source sign_in|key|program] [--key KEY]\n",
    b"forge list\n",
    b"forge setup FORGE [--org ORG] [--repository REPOSITORY] [--project PROJECT] [--playground] [--key KEY]\n",
    b"playground start [--key KEY]\n",
    b"forge connect FORGE github-app APP FILE --key KEY\nforge connect FORGE github --key KEY\nforge connect FORGE forgejo FILE --key KEY\nforge connect FORGE forgejo-admin FILE [--first ORG/REPOSITORY] [--project PROJECT] [--label LABEL] --key KEY\n",
    b"forge install FORGE [--key KEY]\n",
    b"forge repositories FORGE [--installation INSTALLATION] [--page PAGE]\n",
    b"account export ACCOUNT FILE\n",
    b"account import FILE --key KEY\n",
    b"settings show\n",
    b"settings show charter|template|policy NUMBER [--revision REVISION]\nsettings show rules\n",
    b"revision show REVISION\n",
    b"revision list [--before REVISION]\n",
    b"settings apply FILE --against REVISION [--running stays|moves] --key KEY\n",
    b"settings propose FILE [REASON] --against REVISION --key KEY\n",
    b"proposal show PROPOSAL\n",
    b"change show TASK [--project PROJECT]\n",
    b"forge accept FORGE INSTALLATION [--key KEY]\n",
    b"revision advance REVISION --key KEY\n",
    b"task decide TASK --hold REV release|leave|pass-up [REASON] [--key KEY]\n",
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
    for group in table::GROUPS {
        expected.put(group.name).expect("group");
        expected.put(b"  ").expect("separator");
        expected.put(group.line).expect("line");
        expected.put(b"\n").expect("newline");
    }
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
    }
    assert_eq!(list, &*expected.finish(), "the list has exactly the table's rows");
}

#[test]
fn help_includes_each_row_of_its_name() {
    assert_eq!(fixtures::help(Some(jig::rows::SETTINGS_AT)), fixtures::help(Some(jig::rows::SETTING_AT)));
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
            jig::table::repeated(b"word", b"a|b|c", jig::table::Kind::Word { set: &[b"a", b"b", b"c"] }, 2, b"Words"),
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
const RECORD_FORMS: &[u8] = b"fixture N [--activity K] [--tree] [--word a|b|c]... [--key KEY]\n  N (task): a number; required. Task number\n  --activity K: a count from 1 to 32; optional. Latest activity lines\n  --tree: a switch. Include the live tree\n  --word a|b|c: one of a, b or c; repeated, at most 2. Words\nfixture TEXT [--until K] [--timeout D] [--key KEY]\n  TEXT (text): text, at most 1048576 bytes, or - to read stdin; required. Words to send\n  --until K: one of settled, connected:N or made:N with N from 1 to 65535; optional. Condition\n  --timeout D: a duration: seconds, or a number with s, m or h, at most 3600 seconds; optional. Timeout\n";
#[test]
fn a_tools_record_reads_as_written() {
    let expected = expected_record(b"Read a task\nPrints: task N\n--json: the record task.\nKey: --key KEY, optional.\nFirst note.\nSecond note.\n");
    let mut out = skein_lib::Writer::new(expected.len());
    jig::help::tool(&RECORD, &crate::limits::tools(), &mut out).expect("fixture record fits");
    assert_eq!(out.finish(), expected);
}
#[test]
fn an_empty_prints_and_notes_are_left_out() {
    let record = jig::table::Tool { prints: b"", notes: b"", ..RECORD };
    let expected = expected_record(b"Read a task\n--json: the record task.\nKey: --key KEY, optional.\n");
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
                if row.name != jig::rows::HELP.name {
                    assert!(!argument.line.is_empty(), "{row:?}: {argument:?}");
                }
            }
        }
    }
}
