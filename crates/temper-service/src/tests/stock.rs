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
fn without_projects() -> Box<[u8]> {
    let mut document = generated();
    for (path, value) in [
        (b"engine.projects".as_slice(), b"[]".as_slice()),
        (b"application.demo", b"null"),
        (b"application.landing", br#"{"deployment":[],"projects":[]}"#),
    ] {
        document = fixtures::with(&document, path, value);
    }
    document
}
fn limited(document: &[u8], name: &[u8], value: u64) -> Box<[u8]> {
    let original = fixtures::value(document, b"engine.limits.overrides");
    let parsed =
        config::document(&original, &config::DocumentLimits { bytes: 16_384, tokens: 4096, depth: 4 }).unwrap();
    let mut overrides = format!("{{\"{}\":{value}", core::str::from_utf8(name).unwrap());
    for at in (1..parsed.len().checked_sub(1).unwrap()).step_by(2) {
        let key = parsed.text(parsed.token(at).unwrap()).unwrap();
        if key != name {
            let value = config::read::integer(&parsed, at.checked_add(1).unwrap(), u64::MAX).unwrap();
            overrides.push_str(",\"");
            overrides.push_str(core::str::from_utf8(key).unwrap());
            overrides.push_str("\":");
            overrides.push_str(&value.to_string());
        }
    }
    overrides.push('}');
    fixtures::with(document, b"engine.limits.overrides", overrides.as_bytes())
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
fn each_floor_refuses_one_below_the_stocks_need() {
    use jig_domain_core::settings::{self, Setting};
    let document = without_projects();
    let engine = engine(&document);
    let core = config::core(&engine, [1; 16], 7);
    let defaults = crate::stock::defaults(&engine.llm);
    let mut run = (0, Setting::Charter(1));
    for charter in &defaults.charters {
        let bytes = jig_domain_core::policy_owned_bytes(&charter.policy).unwrap();
        if bytes > run.0 {
            run = (bytes, Setting::Charter(charter.number));
        }
    }
    let current = settings::Current::new(defaults, &engine.limits.core);
    let rows: &[(&[u8], u64, Setting, &str)] = &[
        (b"core.policy_bytes", 2374, Setting::Policy(1), "the stock's policy 1 needs 2374 bytes"),
        (b"core.run_bytes", run.0, run.1, ""),
        (b"authority.grants", 10, Setting::Rules, "the stock's rules need 10 grants"),
        (b"authority.executors", 3, Setting::Rules, "the stock's rules need 3 executors"),
        (b"authority.segments", 1, Setting::Rules, "the stock's rules need 1 segment"),
        (b"authority.segment_bytes", 5, Setting::Rules, "the stock's rules need 5 bytes"),
        (b"authority.roles", 1, Setting::Policy(1), "the stock's policy 1 needs 1 role"),
        (b"authority.projects", 1, Setting::Policy(1), "the stock offers 1 policy"),
        (b"tasks.charters", 2, Setting::Charter(1), "the stock offers 2 charters"),
        (b"core.templates", 2, Setting::Template(1), "the stock offers 2 templates"),
        (b"documents.text_bytes", 4, Setting::Template(1), "the stock's template 1 needs 4 bytes"),
        (b"tasks.result_bytes", 32, Setting::Template(1), "the stock's template 1 needs 32 bytes"),
        (b"tasks.grace_ms", 5000, Setting::Template(1), "the stock's template 1 needs 5000 ms of grace"),
        (b"core.settings.alternatives", 1, Setting::Charter(1), "the stock's charter 1 needs 1 alternative"),
        (b"core.settings.permissions", 1, Setting::Policy(1), "the stock's policy 1 needs 1 permission"),
    ];
    // Every positive scalar bound reached by jig's whole-set check is at its
    // floor together. This catches a stock value acquiring another unmet bound.
    let mut minimum = engine.limits;
    for &(name, needed, _, _) in rows {
        config::overrides::apply(&mut minimum, name, needed).unwrap();
    }
    assert_eq!(settings::check(&current, &core.settings, &core.connectors, &minimum.core), Ok(()));
    assert_eq!(crate::stock::floors(&crate::stock::defaults(&engine.llm), &minimum), Ok(()));
    for &(name, needed, setting, words) in rows {
        let mut at = fixtures::parse_engine(&fixtures::value(&document, b"engine"));
        config::overrides::apply(&mut at.limits, name, needed).unwrap();
        let mut problems = crate::startup::Problems::empty();
        crate::startup::checks::stock(&at, &mut problems);
        assert!(problems.is_empty(), "{name:?}");
        assert_eq!(settings::check(&current, &core.settings, &core.connectors, &at.limits.core), Ok(()));
        if let Err(why) = crate::startup::named(&limited(&document, name, needed), &config::Directories::working()) {
            panic!("{name:?} at its floor: {:?}", crate::startup::lines(&why));
        }
        config::overrides::apply(&mut at.limits, name, needed - 1).unwrap();
        assert!(settings::check(&current, &core.settings, &core.connectors, &at.limits.core).is_err(), "{name:?}");
        let found = crate::stock::floors(&crate::stock::defaults(&engine.llm), &at.limits).unwrap_err();
        assert_eq!(found, crate::stock::Floor { limit: name, configured: needed - 1, needed, setting });
        crate::startup::checks::stock(&at, &mut problems);
        let words = if name == b"core.run_bytes" {
            let Setting::Charter(number) = setting else { panic!("run policy's charter") };
            format!("the stock's charter {number} needs {needed} bytes")
        } else {
            words.to_owned()
        };
        let name = core::str::from_utf8(name).unwrap();
        assert_eq!(
            crate::startup::lines(&problems).as_ref(),
            format!(
                "engine.limits.overrides.{name}: {name} is {}; {words}; raise it in engine.limits.overrides\n",
                needed - 1
            )
            .as_bytes()
        );
        let refused =
            crate::startup::named(&limited(&document, name.as_bytes(), needed - 1), &config::Directories::working())
                .unwrap_err();
        assert_eq!(crate::startup::lines(&refused), crate::startup::lines(&problems));
    }
}

#[test]
fn a_stock_policy_past_its_limit_is_refused_while_naming_files() {
    let document = limited(&without_projects(), b"core.policy_bytes", 100);
    // named only parses the document and lists its files. No file read or
    // listener opening can occur before this refusal.
    let found = crate::startup::named(&document, &config::Directories::working()).unwrap_err();
    assert_eq!(
        crate::startup::lines(&found).as_ref(),
        b"engine.limits.overrides.core.policy_bytes: core.policy_bytes is 100; the stock's policy 1 needs 2374 bytes; raise it in engine.limits.overrides\n"
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
    assert!(config::files(&engine).iter().any(|file| file.path.ends_with(b"charters/chat.txt")));
}
