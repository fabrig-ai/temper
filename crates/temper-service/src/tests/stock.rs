//! Stock settings are checked by jig and document members name their store replacement.
#![expect(
    clippy::disallowed_methods,
    clippy::disallowed_macros,
    reason = "ordinary step tests generate documents and compare embedded instruction bytes"
)]
use super::fixtures;
use jig_protocol_config as config;

fn generated() -> Box<[u8]> {
    use core::num::NonZeroU16;
    let answer = temper_init::files(
        &temper_init::Facts {
            binary: b"/bin/temper",
            git: Some(b"/bin/git"),
            claude: None,
            login: Some(b"ada"),
            ports: temper_init::Ports {
                clients: NonZeroU16::new(8367).unwrap(),
                owned: NonZeroU16::new(8368).unwrap(),
                forge: NonZeroU16::new(8369).unwrap(),
                llm: NonZeroU16::new(8370).unwrap(),
                webhooks: NonZeroU16::new(8371).unwrap(),
            },
        },
        &temper_init::Directories { config: b"/config", data: b"/data" },
        &[0; 32],
        &[1; 32],
    )
    .unwrap();
    for file in answer.files {
        if file.name == b"temper.json" {
            let temper_init::Content::Bytes { bytes, .. } = file.content else { panic!("document bytes") };
            return bytes;
        }
    }
    panic!("init writes temper.json")
}
fn single_model() -> Box<[u8]> {
    let mut document = generated();
    for (path, first) in [
        (b"engine.llm.providers".as_slice(), b"engine.llm.providers[0]".as_slice()),
        (b"engine.llm.endpoints", b"engine.llm.endpoints[0]"),
        (b"engine.llm.models", b"engine.llm.models[0]"),
    ] {
        let one = [b"[".as_slice(), &fixtures::value(&document, first), b"]"].concat();
        document = fixtures::with(&document, path, &one);
    }
    fixtures::with(&document, b"application.demo", b"null")
}
fn engine(document: &[u8]) -> config::Engine {
    fixtures::parse_engine(&fixtures::value(document, b"engine"))
}

#[test]
fn stock_chats_can_delegate_and_read_the_forge() {
    let engine = engine(&single_model());
    let defaults = crate::stock::defaults(&engine.llm);
    let policy = &defaults.policies.get(&1).expect("stock policy 1").policy;
    let families = jig_domain_core::ToolFamilies::JIG;
    for tools in [defaults.rules.ceiling.tools, policy.ceiling.tools, policy.roles[0].authority.tools] {
        assert_eq!(tools, jig_domain_authority::Tools(families.delegate.0 | families.read.0));
    }
}

#[test]
fn the_seeded_project_keeps_the_stocks_tool_families() {
    // Interim: the seeded project will take its policy from the stock at configuring-stock's
    // seed boundary. Then both the template's inline copy and this agreement test go.
    let engine = engine(&generated());
    let defaults = crate::stock::defaults(&engine.llm);
    let stock_tools = defaults.rules.ceiling.tools; // The stock's ceiling().tools.
    let core = config::core(&engine, [1; 16], 7);
    let seed = &core.seed_projects[0].policy.policy;
    assert_eq!(seed.ceiling.tools, stock_tools);
    assert_eq!(seed.roles[0].authority.tools, stock_tools);
}

#[test]
fn the_stock_passes_jigs_one_check() {
    let init = generated();
    let without_demo = fixtures::with(&init, b"application.demo", b"null");
    for (row, document) in [("init", without_demo), ("single model", single_model()), ("demo", init)] {
        crate::named(&document, &config::Directories::working()).expect("stock passes startup parsing");
        let engine = engine(&document);
        let core = config::core(&engine, [1; 16], 7);
        let defaults = crate::stock::defaults(&engine.llm);
        assert_eq!(defaults.charters[0].policy.instructions.as_ref(), include_bytes!("../stock/chat.txt"));
        assert_eq!(defaults.charters[1].policy.instructions.as_ref(), include_bytes!("../stock/coding.txt"));
        for charter in &defaults.charters {
            assert_eq!(charter.hosts, jig_domain_fleet::Kinds::Workers, "{row}");
        }
        let current = jig_domain_core::settings::Current::new(defaults, &engine.limits.core);
        assert_eq!(
            jig_domain_core::settings::check(&current, &core.settings, &core.connectors, &engine.limits.core),
            Ok(()),
            "{row}"
        );
    }
}
#[test]
fn the_chats_words_send_work_agreed_first_to_a_goal() {
    let words = include_str!("../stock/chat.txt");
    assert!(words.contains("When a task, not a person, requested you"), "a goal is told apart by its requester");
    assert!(
        words.contains("before anything is changed, send your requester your plan with message, as a question asking whether to go ahead, and wait for the answer. Then call change with what was agreed"),
        "the goal asks its requester with message and waits before change"
    );
    assert!(
        words.contains("When they want to agree a plan before anything is changed, never call change yourself: call goal with their words"),
        "the chat sends work agreed first to goal with the person's words"
    );
    assert!(
        words.contains("tell them a question will wait in their inbox"),
        "the chat names the waiting inbox question"
    );
    assert!(
        words.contains("send their answer to the goal with message, as the answer to that question's number"),
        "the chat relays the answer with message to the question's number"
    );
}

#[test]
fn each_charter_offers_every_configured_model() {
    for (document, names) in [
        (single_model(), [b"gpt-6-luna".as_slice()].as_slice()),
        (generated(), [b"gpt-6-luna".as_slice(), b"demo"].as_slice()),
    ] {
        let engine = engine(&document);
        let defaults = crate::stock::defaults(&engine.llm);
        for charter in &defaults.charters {
            assert_eq!(charter.policy.model.name.as_ref(), names[0]);
            assert_eq!(charter.policy.alternatives.len(), names.len() - 1);
            for (model, name) in charter.policy.alternatives.iter().zip(&names[1..]) {
                assert_eq!(model.name.as_ref(), *name);
            }
        }
    }
}
#[test]
fn the_document_members_are_refused_naming_temper_ctl_settings() {
    let init = generated();
    for member in ["charters", "templates", "policies", "rules"] {
        let original = fixtures::value(&init, b"engine");
        let mut engine = format!("{{\"{member}\":null,").into_bytes();
        engine.extend_from_slice(&original[1..]);
        let document = fixtures::with(&init, b"engine", &engine);
        let parsed =
            config::document(&document, &config::DocumentLimits { bytes: 1_048_576, tokens: 16_384, depth: 32 })
                .unwrap();
        let at = config::read::Object::new(&parsed, 0).unwrap().member(&parsed, b"engine").unwrap();
        let found = config::engine(&parsed, at, &config::Directories::working(), config::Defaults::Passed).unwrap_err();
        let mut problems = crate::startup::Problems::empty();
        problems.append(found, &config::Path::root().member(b"engine"));
        assert_eq!(problems.problems.len(), 1);
        assert_eq!(
            crate::startup::lines(&problems).as_ref(),
            format!("engine.{member}: settings live in the store; read them with temper ctl settings\n").as_bytes()
        );
    }
}

#[test]
fn stock_startup_requires_a_model_before_building_its_charters() {
    let mut document = generated();
    for path in [b"engine.llm.providers".as_slice(), b"engine.llm.endpoints", b"engine.llm.models"] {
        document = fixtures::with(&document, path, b"[]");
    }
    let found = crate::named(&document, &config::Directories::working()).unwrap_err();
    assert_eq!(crate::startup::lines(&found).as_ref(), b"engine.llm.models: past its bound\n");
}

#[test]
fn the_transition_keeps_the_documents_own_defaults() {
    let legacy = [
        br#"{"charters":[{
            "number":1,"instructions":"charters/chat.txt","model":"small","alternatives":[],
            "hosts":"workers","waiting_ms":10000,"resume":true,"turns":8,"time_ms":120000,
            "reserve_turns":1,"reserve_time_ms":60000,"spend":20,"inspect":true,"modify":true,
            "shell":false,"agents":true,"call_timeout_ms":10000
        }],"templates":[],"policies":[],"rules":{
            "ceiling":{"tools":1,"grants":[],
                "delegation":{"kinds":[{"kind":"charter","number":1}],"tasks":8,"depth":4},
                "budget":{"spend":100,"deadline_unix_seconds":null},"notes":0,"note_resources":[]},
            "period_spend":100,"minimum_run_spend":1,"maximum_run_spend":10,"implies":[],"requirements":[]
        },"#
        .as_slice(),
        &fixtures::ENGINE[1..],
    ]
    .concat();
    let document = fixtures::with(br#"{"engine":{}}"#, b"engine", &legacy);
    let document = fixtures::with(&document, b"engine.charters[0].turns", b"9");
    let parsed =
        config::document(&document, &config::DocumentLimits { bytes: 1_048_576, tokens: 16_384, depth: 32 }).unwrap();
    let at = config::read::Object::new(&parsed, 0).unwrap().member(&parsed, b"engine").unwrap();
    let engine = crate::startup::engine(&parsed, at, &config::Directories::working()).unwrap();
    assert_eq!(engine.defaults, config::Defaults::Document);
    assert_eq!(engine.charters[0].policy.turns, 9, "the document's edit survives the transition");
    let mut own = crate::stock::defaults(&engine.llm);
    own.charters[0].policy.turns = 1;
    let own = crate::startup::engine_with(
        &parsed,
        at,
        &config::Directories::working(),
        &crate::stock::Given::Own(Box::new(own)),
    )
    .unwrap();
    assert_eq!(own.charters[0].policy.turns, 9, "Document ignores Given::Own too");
    assert!(config::files(&engine).iter().any(|file| file.path.ends_with(b"charters/chat.txt")));
}

#[test]
fn the_reviewers_words_say_to_fail_when_it_cannot_review() {
    let engine = engine(&single_model());
    let defaults = crate::stock::defaults(&engine.llm);
    let words = core::str::from_utf8(&defaults.charters[1].policy.instructions).expect("review instructions");
    for phrase in [
        "read its pull request and its files at the named head",
        "approve it, or request changes with what to change",
        "If you cannot read them, or cannot judge the change for any other reason",
        "finish failed and say why: never request changes for that.",
    ] {
        assert!(words.contains(phrase), "missing review instruction: {phrase}");
    }
}

#[test]
fn a_worlds_own_defaults_replace_the_stock_without_document_members() {
    let bytes = generated();
    let document =
        config::document(&bytes, &config::DocumentLimits { bytes: 1_048_576, tokens: 16_384, depth: 32 }).unwrap();
    let at = config::read::Object::new(&document, 0).unwrap().member(&document, b"engine").unwrap();
    let stock = crate::startup::engine(&document, at, &config::Directories::working()).unwrap();
    let mut own = crate::stock::defaults(&stock.llm);
    own.charters[0].policy.waiting = skein_lib::Duration::from_millis(1);
    own.templates[0].flags.listed = true;
    own.policies.get_mut(&1).unwrap().policy.period_spend = 41;
    own.rules.maximum_run_spend = 2;
    let given = crate::stock::Given::Own(Box::new(own));
    let engine = crate::startup::engine_with(&document, at, &config::Directories::working(), &given).unwrap();
    assert_eq!(stock.charters[0].policy.waiting, skein_lib::Duration::from_secs(10));
    assert_eq!(engine.charters[0].policy.waiting, skein_lib::Duration::from_millis(1));
    assert!(engine.templates[0].flags.listed);
    assert_eq!(engine.policies.iter().find(|policy| policy.number == 1).unwrap().policy.period_spend, 41);
    assert_eq!(engine.rules.maximum_run_spend, 2);
}
