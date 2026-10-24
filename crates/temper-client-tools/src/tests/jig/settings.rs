#![expect(
    clippy::disallowed_types,
    clippy::disallowed_macros,
    clippy::disallowed_methods,
    reason = "ordinary Rust fixtures and golden comparisons; testing-strategy.md, step tests"
)]
use crate::{
    jig::{
        self,
        call::{Value, Values},
        rows, settings_file,
    },
    tests::fixtures,
};
use jig_client::contract as c;
use skein_lib::{Duration, List, Wall};

fn list<T, I: IntoIterator<Item = T>>(values: I) -> List<T> {
    let values: Vec<T> = values.into_iter().collect();
    let mut out = List::with_capacity(u32::try_from(values.len()).expect("fixture length"));
    for value in values {
        match out.push(value) {
            Ok(()) => {}
            Err(_) => unreachable!("fixture capacity"),
        }
    }
    out
}
fn words(text: &str) -> c::Text {
    c::Text::new(text.as_bytes().into(), 1_048_576).expect("fixture text")
}
fn authority() -> c::OwnerAuthority {
    c::OwnerAuthority {
        models: Some(c::AuthorityModels::Named(list([b"model".as_slice().into()]))),
        tools: 3,
        grants: list([c::OwnerGrant {
            connector: 1,
            kind: 2,
            pattern: c::OwnerPattern {
                segments: list([b"a/b".as_slice().into(), b"\xff".as_slice().into()]),
                last: c::OwnerPatternEnd::Open(b"last".as_slice().into()),
            },
        }]),
        delegation: c::OwnerDelegation { kinds: list([c::OwnerAuthorityExecutor::Charter(3)]), tasks: 4, depth: 5 },
        spend: 600,
        deadline: Some(Wall::from_nanos(700)),
        notes: 1,
        note_resources: list([c::OwnerResourceScope {
            connector: 1,
            pattern: c::OwnerPattern { segments: list([]), last: c::OwnerPatternEnd::Exact(b"end".as_slice().into()) },
        }]),
    }
}
fn model() -> c::SettingModel {
    c::SettingModel {
        dialect: 1,
        endpoint: 2,
        program: Some(words("program")),
        name: words("model"),
        window: 4096,
        output: 1024,
        input_price: 1,
        cached_price: 2,
        cache_write_price: Some(3),
        output_price: 4,
        price_unit: 1000,
        efforts: list([words("high")]),
        effort: Some(words("high")),
    }
}
fn values() -> Vec<(c::Setting, c::SettingValue)> {
    vec![
        (
            c::Setting::Charter(c::SettingNumber { number: 3 }),
            c::SettingValue::Charter(Box::new(c::Charter {
                number: 3,
                instructions: words("keep \"boundaries\"\nwhole"),
                waiting: Duration::from_secs(1),
                resume: true,
                turns: 8,
                time: Duration::from_secs(2),
                reserve_turns: 1,
                reserve_time: Duration::from_secs(1),
                model: model(),
                alternatives: list([model()]),
                inspect: true,
                modify: true,
                shell: false,
                agents: false,
                call_timeout: Duration::from_secs(3),
                hosts: c::SettingHosts::Both,
                spend: 900,
            })),
        ),
        (
            c::Setting::Template(c::SettingNumber { number: 4 }),
            c::SettingValue::Template(Box::new(c::Template {
                number: 4,
                name: c::DocumentText::complete(words("work")),
                executor: c::TemplateExecutor::Charter(3),
                contract: c::TaskContractDocument::Report { words: 1024 },
                shallower: 1,
                budget: c::TemplateBudget::AsAllowed { cap: Some(1000) },
                deadline: c::TemplateDeadline::Span(Duration::from_secs(10)),
                tracked: Some(5),
                flags: c::TaskFlags {
                    conversation: false,
                    listed: true,
                    retention: c::Retention::Kept,
                    ended_by: c::EndedBy::Result,
                },
                label: 1,
                grace: Some(Duration::from_secs(1)),
                llm: Some(c::LlmChoice {
                    account: Some(9),
                    main: c::ModelEffort { model: words("model"), effort: Some(words("high")) },
                    subagents: list([c::ModelEffort { model: words("model"), effort: None }]),
                }),
            })),
        ),
        (
            c::Setting::Policy(c::SettingNumber { number: 5 }),
            c::SettingValue::Policy(Box::new(c::SettingPolicy {
                escalation_role: Some(1),
                ceiling: authority(),
                projections: list([authority().grants.get(0).expect("grant").clone()]),
                period_spend: 2000,
                roles: list([c::PolicyEditRole {
                    number: 1,
                    authority: authority(),
                    period_spend: 1000,
                    requests: 3,
                    decides: 1,
                }]),
                requirements: list([]),
                permissions: list([c::PolicyPermission { connector: 1, permission: 2, role: 1 }]),
                templates: list([c::TemplateOverride {
                    template: 4,
                    executor: None,
                    budget: Some(c::TemplateBudget::Amount(500)),
                    deadline: Some(c::TemplateDeadline::Given),
                    priority: None,
                }]),
            })),
        ),
        (
            c::Setting::Rules,
            c::SettingValue::Rules(Box::new(c::Rules {
                ceiling: authority(),
                period_spend: 3000,
                minimum_run_spend: 10,
                maximum_run_spend: 1000,
                implies: list([c::OwnerKindImplication { connector: 1, kind: 2, implies: 3 }]),
                requirements: list([]),
            })),
        ),
    ]
}
fn read(argv: &[&str], value: c::ReadValue) -> jig::Printed {
    let (mut root, call) = fixtures::read_call(argv);
    if argv.first() == Some(&"setting") {
        newest_for_setting(&mut root, call);
    }
    root.serve(call, c::ReadResult::Value(value));
    root.answer(call).expect("read completed")
}
fn newest_for_setting(root: &mut fixtures::Harness, call: u32) {
    if root.reads.get(&call).expect("setting read").1 == c::Read::Settings {
        root.serve(
            call,
            c::ReadResult::Value(c::ReadValue::Settings(Box::new(c::SettingsDocument {
                newest: 5,
                settings: list([]),
                held: list([]),
                pending: list([]),
            }))),
        );
    }
}
fn setting_json(setting: c::Setting, value: c::SettingValue) -> Vec<u8> {
    let mut root = fixtures::signed_in(fixtures::PARTY);
    let mut call = fixtures::call(&["setting", "rules"]);
    call.tool = rows::SETTING_AT;
    let number = root.submit_prepared(call, true, None);
    newest_for_setting(&mut root, number);
    root.serve(
        number,
        c::ReadResult::Value(c::ReadValue::Setting(Box::new(c::SettingDocument { setting, revision: 5, value }))),
    );
    root.answer(number).expect("setting answered").stdout.to_vec()
}
fn file_from_setting(bytes: &[u8]) -> Vec<u8> {
    // The document's selector and value are the FILE item's exact values.
    let text = core::str::from_utf8(bytes).expect("JSON");
    let text = text.replace(",\"revision\":5,\"value\":", ",\"set\":");
    format!("{{\"items\":[{text}]}}").into_bytes()
}
fn changed(bytes: &[u8], json: bool) -> (fixtures::Harness, u32) {
    let mut root = fixtures::signed_in(fixtures::PARTY);
    let mut call =
        fixtures::call(&["change-settings", "FILE", "--against", "5", "--running", "moves", "--key", "settings-5"]);
    *call.values.get_mut(0).expect("FILE") = Some(Values::One(Value::Text(bytes.into())));
    let number = root.submit_prepared(call, json, None);
    (root, number)
}
fn head(by: c::RevisionDecider) -> c::RevisionHead {
    c::RevisionHead {
        number: 5,
        against: 4,
        at: Wall::EPOCH,
        by,
        proposal: Some(c::ProposalKey { proposer: c::Proposer::Party(2), number: 7 }),
        moved: false,
    }
}
fn golden(name: &str, bytes: &[u8]) {
    let path = std::path::Path::new("src/tests/golden").join(name).with_extension("txt");
    if std::env::var_os("SKEIN_CODEGEN_WRITE").is_some() {
        std::fs::write(&path, bytes).expect("write settings golden");
    }
    assert_eq!(bytes, std::fs::read(path).expect("settings golden"));
}
#[test]
fn settings_calls_parse_and_write_back_with_existing_numbers_unchanged() {
    for argv in [
        vec!["settings"],
        vec!["setting", "charter", "3", "--revision", "5"],
        vec!["setting", "template", "4"],
        vec!["setting", "policy", "5"],
        vec!["setting", "rules"],
        vec!["revision", "5"],
        vec!["revisions", "--before", "5"],
        vec!["change-settings", "-", "--against", "5", "--running", "moves", "--key", "settings-5"],
    ] {
        let call = fixtures::call(&argv);
        assert_eq!(fixtures::back(&call), fixtures::owned(&argv));
        assert_eq!(fixtures::printed(&call), jig::call::Parsed::Call { call, json: false });
    }
    assert_eq!((rows::EXPORT_AT, rows::IMPORT_AT, rows::SETTINGS_AT, rows::CHANGE_SETTINGS_AT), (40, 41, 42, 46));
    for argv in [
        vec!["setting", "rules", "--project", "1"],
        vec!["setting", "charter", "3", "--project", "1"],
        vec!["revision", "4294967296"],
        vec!["settings", "--watch"],
        vec!["change-settings", "FILE", "--against", "5"],
    ] {
        assert!(fixtures::parse(&argv).is_err(), "{argv:?}");
    }
}
#[test]
fn each_whole_setting_json_is_exactly_the_set_value_and_sends_under_its_name() {
    for (setting, value) in values() {
        let json = setting_json(setting, value.clone());
        let file = file_from_setting(&json);
        let items = settings_file::decode(&file, &crate::limits::tools()).expect("whole setting round trip");
        assert_eq!(items.as_slice(), &[c::SettingItem { setting, edit: c::SettingEdit::Set(c::SettingSet { value }) }]);
        let (root, call) = changed(&file, false);
        assert_eq!(root.sent_name(call), Some(fixtures::named("settings-5")));
        let (_, _, jig_client::Ask::Jig(ask)) = root.sends.get(&call).expect("one keyed request") else {
            panic!("jig ask")
        };
        assert_eq!(
            ask.as_ref(),
            &c::Ask::ChangeSettings(c::ChangeSettings { against: 5, items, running: c::Running::Moves })
        );
    }
}
#[test]
fn settings_decoder_accepts_removed_and_reset_but_refuses_unknown_members_and_duplicates() {
    let limits = crate::limits::tools();
    let bytes = br#"{"items":[{"setting":{"kind":"charter","number":3},"edit":"removed"},{"setting":{"kind":"rules"},"edit":"reset"}]}"#;
    assert_eq!(settings_file::decode(bytes, &limits).expect("two edits").len(), 2);
    for (bytes, member) in [
        (br#"{"items":[],"extra":0}"#.as_slice(), "extra"),
        (br#"{"items":[{"setting":{"kind":"rules","project":1},"edit":"reset"}]}"#, "project"),
        (br#"{"items":[{"setting":{"kind":"rules"},"edit":"reset","other":1}]}"#, "other"),
        (
            br#"{"items":[{"setting":{"kind":"rules"},"edit":"reset"},{"setting":{"kind":"rules"},"edit":"removed"}]}"#,
            "duplicate setting",
        ),
        (br#"{"items":[{"setting":{"kind":"rules"},"edit":"reset","set":{}}]}"#, "exactly one"),
        (br#"{"items":[{"setting":{"kind":"rules"},"edit":"reset","edit":"removed"}]}"#, "duplicate member"),
    ] {
        let error = settings_file::decode(bytes, &limits).expect_err("invalid input");
        assert!(core::str::from_utf8(&error).expect("message").contains(member));
        let (mut root, call) = changed(bytes, false);
        assert!(root.sends.is_empty());
        assert_eq!(root.answer(call).expect("local refusal").status, 1);
    }
}
#[test]
fn settings_file_checks_file_and_item_count_bounds_before_sending() {
    let mut limits = crate::limits::tools();
    limits.ask_bytes = 10;
    assert!(
        core::str::from_utf8(&settings_file::decode(br#"{"items":[]}"#, &limits).expect_err("bytes bound"))
            .expect("message")
            .contains("file bytes: bound 10")
    );
    let file = format!("{{\"items\":[{}]}}", vec![r#"{"setting":{"kind":"rules"},"edit":"reset"}"#; 1025].join(","));
    assert!(
        core::str::from_utf8(&settings_file::decode(file.as_bytes(), &crate::limits::tools()).expect_err("item count"))
            .expect("message")
            .contains("items: bound 1024")
    );
}
#[test]
fn whole_settings_document_renderings_are_stable() {
    let inventory = c::ReadValue::Settings(Box::new(c::SettingsDocument {
        newest: 5,
        settings: list([
            c::SettingLine {
                setting: c::Setting::Charter(c::SettingNumber { number: 3 }),
                name: Some(c::DocumentText::complete(words("worker"))),
                follows: c::SettingFollows::Change,
                last: 5,
            },
            c::SettingLine { setting: c::Setting::Rules, name: None, follows: c::SettingFollows::Default, last: 0 },
        ]),
        held: list([c::HeldRevision {
            head: head(c::RevisionDecider::Party(c::RevisionParty { number: 2 })),
            holders: 3,
        }]),
        pending: list([c::SettingsProposal { proposer: c::Proposer::Party(2), number: 7 }]),
    }));
    let revision = c::ReadValue::Revision(Box::new(c::RevisionAnswer::Revision(c::RevisionDocument {
        head: head(c::RevisionDecider::Release),
        items: list([
            c::RevisionItem {
                setting: c::Setting::Charter(c::SettingNumber { number: 3 }),
                edit: c::RevisionEdit::TooLarge(c::Size { bytes: 1_000_000 }),
            },
            c::RevisionItem { setting: c::Setting::Rules, edit: c::RevisionEdit::Reset },
        ]),
    })));
    let revisions = c::ReadValue::Revisions(Box::new(c::RevisionsPage {
        rows: list([c::RevisionRow {
            head: head(c::RevisionDecider::FirstStart),
            items: list([c::RevisionItemHead { setting: c::Setting::Rules, edit: c::RevisionItemEdit::Set }]),
            more: 2,
        }]),
        next: Some(4),
        forgotten: false,
    }));
    let forgotten = c::ReadValue::Revision(Box::new(c::RevisionAnswer::Forgotten(c::RevisionForgotten { oldest: 2 })));
    for (argv, document, name) in [
        (vec!["settings"], inventory, "settings_inventory"),
        (vec!["revision", "5"], revision, "settings_revision"),
        (vec!["revisions"], revisions, "settings_revisions"),
        (vec!["revision", "1"], forgotten, "settings_forgotten"),
    ] {
        let text = read(&argv, document.clone());
        assert_eq!(text.status, 0);
        golden(name, &text.stdout);
        let mut argv = argv;
        argv.push("--json");
        let json = read(&argv, document);
        assert_eq!(json.status, 0);
        golden(&format!("{name}_json"), &json.stdout);
    }
    for (index, (setting, value)) in values().into_iter().enumerate() {
        let json = setting_json(setting, value);
        golden(&format!("settings_value_{index}_json"), &json);
    }
}
#[test]
fn settings_change_outcomes_name_the_revision_and_refusal_facts() {
    let file = br#"{"items":[{"setting":{"kind":"rules"},"edit":"reset"}]}"#;
    let (mut root, call) = changed(file, false);
    root.done(call, c::Outcome::SettingsChanged(c::OutcomeSettingsChanged { revision: 6 }));
    assert_eq!(root.answer(call).expect("changed").stdout.as_ref(), b"revision 6\n");
    for (refusal, expected) in [
        (c::Refusal::Stale(c::RefusalStale { newest: 6 }), "stale; newest revision 6"),
        (
            c::Refusal::Malformed {
                at: 1,
                why: c::Malformed::Settings(c::MalformedSettings {
                    setting: c::Setting::Rules,
                    fault: c::SettingFault::Owner,
                }),
            },
            "out of shape; item 1; rules: owner",
        ),
        (
            c::Refusal::Full(c::RefusalFull { revision: 2, tasks: list([41, 42]), more: 3 }),
            "full; oldest held revision 2; live tasks: 41 42; 3 more",
        ),
        (c::Refusal::Role { held: None }, "role"),
    ] {
        for json in [false, true] {
            let (mut root, call) = changed(file, json);
            root.answered(call, c::Answer::Refused { refusal: refusal.clone(), at: None });
            let answer = root.answer(call).expect("refused");
            assert_eq!(answer.status, 2);
            if !json && expected != "role" {
                assert!(core::str::from_utf8(&answer.stderr).expect("text").contains(expected));
            }
        }
    }
}

#[test]
fn settings_values_refuse_each_nonempty_byte_and_list_bound_on_both_sides() {
    // Exercise the same input at its bound and one below it. Runtime limits
    // come from jig's link, so the tools do not install a parallel preset.
    let (_, policy) = values().remove(2);
    let c::SettingValue::Policy(policy) = policy else {
        panic!("policy");
    };
    let wire =
        jig_client_protocol_link::translate::settings::wire_setting_policy(&policy, &crate::limits::tools().settings)
            .expect("wire policy");
    let grant_bytes = u32::try_from(wire.ceiling().grants().len()).expect("grant bytes");
    for (index, name, length) in [
        (0, "charter_instructions", 23),
        (0, "charter_alternatives", 1),
        (0, "setting_model_program_some", 7),
        (0, "setting_model_name", 5),
        (0, "setting_model_efforts", 1),
        (0, "setting_model_efforts_item", 4),
        (0, "setting_model_effort_some", 4),
        (1, "document_text_bytes", 4),
        (1, "model_effort_model", 5),
        (1, "model_effort_effort_some", 4),
        (2, "setting_policy_roles", 1),
        (2, "setting_policy_permissions", 1),
        (2, "setting_policy_templates", 1),
        (2, "owner_authority_grants", grant_bytes),
        (2, "owner_delegation_kinds", 1),
        (2, "owner_pattern_segments", 2),
        (2, "owner_pattern_segments_item", 3),
        (2, "owner_pattern_end_open_value", 4),
        (2, "model_set_names_names", 1),
        (2, "model_set_names_names_item", 5),
        (3, "rules_implies", 1),
    ] {
        let (setting, value) = values().remove(index);
        let file = file_from_setting(&setting_json(setting, value));
        for (bound, allowed) in [(length, true), (length - 1, false)] {
            let mut limits = crate::limits::tools();
            let wire = &mut limits.settings;
            match name {
                "charter_instructions" => wire.charter_instructions = bound,
                "charter_alternatives" => wire.charter_alternatives = bound,
                "setting_model_program_some" => wire.setting_model_program_some = bound,
                "setting_model_name" => wire.setting_model_name = bound,
                "setting_model_efforts" => wire.setting_model_efforts = bound,
                "setting_model_efforts_item" => wire.setting_model_efforts_item = bound,
                "setting_model_effort_some" => wire.setting_model_effort_some = bound,
                "document_text_bytes" => wire.document_text_bytes = bound,
                "model_effort_model" => wire.model_effort_model = bound,
                "model_effort_effort_some" => wire.model_effort_effort_some = bound,
                "setting_policy_roles" => wire.setting_policy_roles = bound,
                "setting_policy_permissions" => wire.setting_policy_permissions = bound,
                "setting_policy_templates" => wire.setting_policy_templates = bound,
                "owner_authority_grants" => wire.owner_authority_grants = bound,
                "owner_delegation_kinds" => wire.owner_delegation_kinds = bound,
                "owner_pattern_segments" => wire.owner_pattern_segments = bound,
                "owner_pattern_segments_item" => wire.owner_pattern_segments_item = bound,
                "owner_pattern_end_open_value" => wire.owner_pattern_end_open_value = bound,
                "model_set_names_names" => wire.model_set_names_names = bound,
                "model_set_names_names_item" => wire.model_set_names_names_item = bound,
                "rules_implies" => wire.rules_implies = bound,
                _ => panic!("fixture bound name"),
            }
            let result = settings_file::decode(&file, &limits);
            assert_eq!(result.is_ok(), allowed, "{name} bound {bound}: {result:?}");
            if let Err(error) = result {
                let text = core::str::from_utf8(&error).expect("message");
                assert!(text.contains("item 1") && text.contains(name), "{text}");
            }
        }
    }
}

#[test]
fn settings_reads_request_the_exact_contract_and_do_not_watch_settings() {
    for (argv, expected) in [
        (vec!["settings"], c::Read::Settings),
        (
            vec!["setting", "charter", "3", "--revision", "5"],
            c::Read::Setting(c::ReadSetting {
                setting: c::Setting::Charter(c::SettingNumber { number: 3 }),
                revision: 5,
            }),
        ),
        (vec!["setting", "rules"], c::Read::Settings),
        (vec!["revision", "5"], c::Read::Revision(c::ReadRevision { number: 5 })),
        (vec!["revisions", "--before", "5"], c::Read::Revisions(c::ReadRevisions { before: Some(5) })),
    ] {
        let (root, call) = fixtures::read_call(&argv);
        assert_eq!(root.reads.get(&call).expect("one read").1, expected);
        assert_eq!(root.watches.len(), 1, "only shared me watch");
    }
}

#[test]
fn an_unqualified_setting_reads_the_inventoried_revision_before_its_value() {
    for (argv, setting) in [
        (vec!["setting", "charter", "3"], c::Setting::Charter(c::SettingNumber { number: 3 })),
        (vec!["setting", "template", "4"], c::Setting::Template(c::SettingNumber { number: 4 })),
        (vec!["setting", "policy", "5"], c::Setting::Policy(c::SettingNumber { number: 5 })),
        (vec!["setting", "rules"], c::Setting::Rules),
    ] {
        let (mut root, call) = fixtures::read_call(&argv);
        assert_eq!(root.reads.get(&call).expect("first read").1, c::Read::Settings);
        newest_for_setting(&mut root, call);
        assert!(root.answer(call).is_none());
        assert_eq!(
            root.reads.get(&call).expect("value read").1,
            c::Read::Setting(c::ReadSetting { setting, revision: 5 })
        );
        root.serve(call, c::ReadResult::None(c::ReadNone::Unknown));
        assert_eq!(root.answer(call).expect("value refusal").status, 2);
        assert_eq!(root.watches.len(), 1, "only the shared me watch");
    }
}

#[test]
fn settings_reads_keep_their_neighbours_not_served_and_role_statuses() {
    for argv in [vec!["settings"], vec!["setting", "rules"], vec!["revision", "5"], vec!["revisions"]] {
        for (page, status) in [
            (jig_client::Readout::NotServed { family: None, served: None }, 1),
            (jig_client::Readout::Jig(c::ReadResult::None(c::ReadNone::Refused)), 2),
        ] {
            let (mut root, call) = fixtures::read_call(&argv);
            root.serve_readout(call, page);
            assert_eq!(root.answer(call).expect("terminal read").status, status);
        }
    }
}

#[test]
fn setting_text_and_revisions_include_whole_set_values_and_last_page_facts() {
    for (index, (setting, value)) in values().into_iter().enumerate() {
        let argv = match setting {
            c::Setting::Charter(_) => vec!["setting", "charter", "3"],
            c::Setting::Template(_) => vec!["setting", "template", "4"],
            c::Setting::Policy(_) => vec!["setting", "policy", "5"],
            c::Setting::Rules => vec!["setting", "rules"],
        };
        let answer = read(
            &argv,
            c::ReadValue::Setting(Box::new(c::SettingDocument { setting, revision: 5, value: value.clone() })),
        );
        assert_eq!(answer.status, 0);
        golden(&format!("settings_value_{index}"), &answer.stdout);
        for json in [false, true] {
            let mut argv = vec!["revision", "5"];
            if json {
                argv.push("--json");
            }
            let answer = read(
                &argv,
                c::ReadValue::Revision(Box::new(c::RevisionAnswer::Revision(c::RevisionDocument {
                    head: head(c::RevisionDecider::Release),
                    items: list([c::RevisionItem {
                        setting,
                        edit: c::RevisionEdit::Set(c::SettingSet { value: value.clone() }),
                    }]),
                }))),
            );
            assert_eq!(answer.status, 0);
            golden(&format!("settings_revision_value_{index}_{}", if json { "json" } else { "text" }), &answer.stdout);
        }
    }
    let answer = read(
        &["revisions"],
        c::ReadValue::Revisions(Box::new(c::RevisionsPage { rows: list([]), next: None, forgotten: true })),
    );
    assert_eq!(answer.stdout.as_ref(), b"older revisions were forgotten\n");
    let answer = read(&["revision", "99"], c::ReadValue::Revision(Box::new(c::RevisionAnswer::Unknown)));
    assert_eq!((answer.status, answer.stderr.as_ref()), (2, b"unknown revision\n".as_slice()));
}

#[test]
fn change_settings_running_defaults_to_stays_and_json_names_the_new_revision() {
    let mut call = fixtures::call(&["change-settings", "FILE", "--against", "5", "--key", "settings-5"]);
    *call.values.get_mut(0).expect("FILE") =
        Some(Values::One(Value::Text(br#"{"items":[{"setting":{"kind":"rules"},"edit":"reset"}]}"#.as_slice().into())));
    let mut root = fixtures::signed_in(fixtures::PARTY);
    let number = root.submit_prepared(call, true, None);
    let (_, _, jig_client::Ask::Jig(ask)) = root.sends.get(&number).expect("sent") else { panic!("jig ask") };
    let c::Ask::ChangeSettings(change) = ask.as_ref() else { panic!("change settings") };
    assert_eq!(change.running, c::Running::Stays);
    root.done(number, c::Outcome::SettingsChanged(c::OutcomeSettingsChanged { revision: 6 }));
    assert_eq!(root.answer(number).expect("durable answer").stdout.as_ref(), br#"{"revision":6}"#);
}

fn requirement() -> c::PolicyRequirement {
    c::PolicyRequirement {
        connector: 1,
        kind: 2,
        pattern: c::OwnerPattern {
            segments: list([b"one".as_slice().into()]),
            last: c::OwnerPatternEnd::Exact(b"two".as_slice().into()),
        },
        judge: c::PolicyJudge { connector: 1, requirement: 2, parameters: 3 },
        guard: c::PolicyGuard::Observed { freshness: Duration::from_secs(1) },
        must_be_guarded: true,
    }
}
#[test]
fn remaining_nested_collections_and_bytes_obey_jigs_bounds() {
    macro_rules! boundary {
        ($setting:expr, $value:expr, $field:ident, $length:expr) => {{
            let file = file_from_setting(&setting_json($setting, $value));
            let mut limits = crate::limits::tools();
            limits.settings.$field = $length;
            settings_file::decode(&file, &limits).expect("at the bound");
            limits.settings.$field = $length - 1;
            let why = settings_file::decode(&file, &limits).expect_err("past the bound");
            let why = core::str::from_utf8(&why).expect("bound text");
            assert!(why.contains("item 1") && why.contains(stringify!($field)), "{why}");
        }};
    }
    let (setting, value) = values().remove(2);
    boundary!(setting, value.clone(), setting_policy_projections, 1);
    let c::SettingValue::Policy(policy) = &value else {
        panic!("policy");
    };
    let wire =
        jig_client_protocol_link::translate::settings::wire_setting_policy(policy, &crate::limits::tools().settings)
            .expect("wire policy");
    let resource_bytes = u32::try_from(wire.ceiling().note_resources().len()).expect("resource bytes");
    boundary!(setting, value.clone(), owner_authority_note_resources, resource_bytes);
    boundary!(setting, value.clone(), owner_pattern_end_exact_value, 3);
    let c::SettingValue::Policy(mut policy) = value else { panic!("policy") };
    policy.requirements = list([requirement()]);
    boundary!(setting, c::SettingValue::Policy(policy), setting_policy_requirements, 1);
    let (setting, value) = values().remove(3);
    let c::SettingValue::Rules(mut rules) = value else { panic!("rules") };
    rules.requirements = list([requirement()]);
    boundary!(setting, c::SettingValue::Rules(rules), rules_requirements, 1);
    let (setting, value) = values().remove(1);
    boundary!(setting, value.clone(), llm_choice_subagents, 1);
    let c::SettingValue::Template(mut template) = value else { panic!("template") };
    template.contract = c::TaskContractDocument::Approval {
        state: b"yes".as_slice().into(),
        part: c::OwnerPart {
            connector: 1,
            kind: c::OwnerPartKind::Resource,
            key: c::OwnerPartKey::Path(list([b"path".as_slice().into()])),
        },
    };
    boundary!(setting, c::SettingValue::Template(template.clone()), task_contract_document_approval_state, 3);
    boundary!(setting, c::SettingValue::Template(template.clone()), owner_part_key_path_value, 1);
    boundary!(setting, c::SettingValue::Template(template.clone()), owner_part_key_path_value_item, 4);
    template.contract = c::TaskContractDocument::Verdict {
        choices: list([c::VerdictChoice {
            code: 1,
            words: 2,
            followups: 3,
            fields: list([c::FieldRule { name: b"field".as_slice().into(), max: 5 }]),
        }]),
        cut: false,
    };
    let wire = jig_client_protocol_link::translate::settings::wire_setting_value(
        &c::SettingValue::Template(template.clone()),
        &crate::limits::tools().settings,
    )
    .expect("wire template");
    let jig_client_protocol_wire::SettingValue::Template(wire) = wire else {
        panic!("wire template");
    };
    let jig_client_protocol_wire::TaskContractDocument::Verdict(verdict) = wire.contract() else {
        panic!("wire verdict");
    };
    let choice_bytes = u32::try_from(verdict.choices().len()).expect("choice bytes");
    boundary!(
        setting,
        c::SettingValue::Template(template.clone()),
        task_contract_document_verdict_choices,
        choice_bytes
    );
    boundary!(setting, c::SettingValue::Template(template.clone()), verdict_choice_fields, 1);
    boundary!(setting, c::SettingValue::Template(template), field_rule_name, 5);
}

#[test]
fn a_settings_change_keeps_the_exact_request_for_recovery_and_pending() {
    let file = br#"{"items":[{"setting":{"kind":"rules"},"edit":"reset"}]}"#;
    let (root, number) = changed(file, false);
    let (_, _, request) = root.sends.get(&number).expect("sent request");
    let records = list([skein_lib::RequestRecord {
        key: fixtures::named("settings-5"),
        scope: fixtures::PARTY,
        first_sent: None,
        request: jig_client::Held {
            key: jig_client::Key {
                asker: crate::root::RUNNER.asker(),
                number: (u64::from(fixtures::RESTORED_PREFIX) << 32u32) | 1,
            },
            request: request.clone(),
        },
    }]);
    let pending = jig::render::pending::pending(records.as_slice(), 0, false);
    assert_eq!(pending.stdout.as_ref(), b"change-settings, party 73, not sent\n");
    let mut restored = fixtures::started_with(records, fixtures::SEED, false);
    restored.deliver(c::Delivery::Me(Box::new(fixtures::me(fixtures::PARTY))));
    let (_, _, resent) = restored.sends.get(&0).expect("kept request resent");
    assert_eq!(resent, request);
    restored.done(0, c::Outcome::SettingsChanged(c::OutcomeSettingsChanged { revision: 6 }));
    assert_eq!(restored.kept_lines.as_slice(), &[Box::<[u8]>::from(b"kept: change-settings, revision 6\n".as_slice())]);
}

#[test]
fn settings_items_check_the_aggregate_wire_byte_bound_and_maximum_count() {
    let file = br#"{"items":[{"setting":{"kind":"rules"},"edit":"reset"}]}"#;
    let mut limits = crate::limits::tools();
    let items = settings_file::decode(file, &limits).expect("items");
    let wire = jig_client_protocol_link::translate::settings::wire_change_settings(
        &c::ChangeSettings { against: 5, items, running: c::Running::Stays },
        &limits.settings,
    )
    .expect("wire items");
    let size = u32::try_from(wire.items().len()).expect("wire bytes");
    limits.settings.change_settings_items = size;
    settings_file::decode(file, &limits).expect("at item bytes bound");
    limits.settings.change_settings_items = size - 1;
    let why = settings_file::decode(file, &limits).expect_err("past item bytes bound");
    assert!(core::str::from_utf8(&why).expect("message").contains("item 1: encoded items bytes: bound"));
    let items: Vec<String> = (0_u32..1024)
        .map(|number| format!(r#"{{"setting":{{"kind":"charter","number":{number}}},"edit":"reset"}}"#))
        .collect();
    let file = format!("{{\"items\":[{}]}}", items.join(","));
    assert_eq!(
        settings_file::decode(file.as_bytes(), &crate::limits::tools()).expect("at item count bound").len(),
        1024
    );
}
