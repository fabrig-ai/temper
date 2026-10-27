//! Stock charters meet smith's actual derived host-part bound before a turn.
use jig_domain_tasks as tasks;
use skein_lib::{Env, Queue, ReplyTo, Time, Token, Wall};
use smith_domain_run as run;
use temper_engine_domain as engine;

fn limits(directory: &std::path::Path) -> run::Limits {
    let mut document: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../../temper-init/template/agent.json"))
            .expect("stock host-part fixture");
    document["programs"] = serde_json::json!({});
    for endpoint in document["endpoints"].as_array_mut().expect("stock host-part fixture") {
        endpoint["address"] = "127.0.0.1:443".into();
        if endpoint["transport"] == "tls" {
            let trust = directory.join("root.der");
            std::fs::write(&trust, skein_tls_world::pki::ROOT).expect("stock host-part fixture");
            endpoint["trust_der"] = serde_json::json!(trust);
        }
    }
    smith_agent_shell::config::from_value(document).expect("init's profile derives").service.limits.domain.run
}

fn assignment(number: u32, settings: bool) -> engine::Assignment {
    let mut llm = super::settings_fields::llm();
    let mut main = llm.models[0].clone();
    main.name = b"gpt-6-luna".as_slice().into();
    main.dialect = 3;
    main.endpoint = 1;
    main.window = 1_048_576;
    main.output = 8192;
    let mut demo = main.clone();
    demo.name = b"demo".as_slice().into();
    demo.endpoint = 3;
    demo.window = 131_072;
    llm.models = Box::new([main, demo]);
    let defaults = temper_service::stock::defaults(&llm);
    let charter = defaults.charters.iter().find(|charter| charter.number == number).expect("stock host-part fixture");
    let mut authority = jig_domain_core::task_authority(&defaults.rules.ceiling);
    authority.tools.0 |= jig_domain_core::ToolFamilies::JIG.settings;
    let mut policy = charter.policy.clone();
    if settings {
        policy.tools.0 |= jig_domain_core::ToolFamilies::JIG.settings;
    }
    engine::Assignment {
        task: 1,
        attempt: 1,
        conversation: 1,
        live: jig_domain_host::LiveAsk { text: false, reasoning: false, input: false },
        charter: number,
        run: Box::new(engine::RunCharter {
            revision: 1,
            ended_by: if number == 1 { tasks::EndedBy::Requester } else { tasks::EndedBy::Result },
            sources: vec![Some(1); charter.policy.alternatives.len() + 1].into_boxed_slice(),
            accounts: Box::new([]),
            placement: jig_domain_fleet::PlacementConstraint {
                channel: Token::new(1),
                kind: jig_domain_fleet::HostKind::Worker,
            },
            policy,
            contract: if number == 1 {
                tasks::Contract::Report { words: 512 }
            } else {
                tasks::Contract::Change { connector: 1, kind: 2, words: 512 }
            },
            authority,
            budget: charter.spend,
        }),
        sections: Box::new([]),
        messages: Box::new([]),
        saved: Box::new([]),
        workstream: 1,
        workspace: engine::ForgeWorkspace { repositories: Box::new([]) },
        transcript: Box::new([]),
        answered: Box::new([]),
        settled: Box::new([]),
        grants: Box::new([]),
    }
}

fn enter(number: u32, settings: bool, limits: &run::Limits, bound: u64) -> bool {
    let charter = temper_engine_smith::charter(&assignment(number, settings));
    let limits = run::Limits { host_part: bound, ..*limits };
    let mut domain = run::Domain::new(&limits, 0);
    let mut out = Queue::with_capacity(run::MAX_OUT);
    run::step(
        &mut domain,
        &Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
        run::Event::Start {
            messages: Box::new([]),
            reply_to: ReplyTo::new(Token::new(1)),
            host_run: Token::new(1),
            identity: None,
            activation: 1,
            window: run::Window { turns: u32::MAX, bytes: u64::MAX, largest_turn: 1 },
            charter,
            workspace: Some(run::Workspace {
                directories: Box::new([run::Directory {
                    name: b"work".as_slice().into(),
                    host: b"/workspace".as_slice().into(),
                    root: Token::new(1),
                    writable: true,
                    git: true,
                    conflicts: Box::new([]),
                }]),
            }),
            transcript: None,
            resumed: false,
        },
        &mut out,
    );
    let request = out.pop().expect("entrance answers before a turn");
    if let run::Request::Admitted { .. } = request {
        return true;
    }
    if let run::Request::Answer {
        answer: run::Answer::Refused(run::Refusal::Invalid(run::Invalid::HostPart { bound: actual })),
        ..
    } = request
    {
        assert_eq!(actual, bound);
        return false;
    }
    panic!("unexpected entrance: {request:?}");
}

#[test]
fn stock_producer_and_chat_fit_the_derived_host_part() {
    let scratch = skein_scratch::Scratch::new("stock-host-part");
    let limits = limits(scratch.path());
    for (number, settings) in [(2, false), (1, false), (1, true)] {
        let charter = temper_engine_smith::charter(&assignment(number, settings));
        let tools = charter.grants.host_tools.len();
        assert_eq!(tools, if settings { 39 } else { 37 });
        assert!(enter(number, settings, &limits, limits.host_part));
        let (mut lower, mut upper) = (0, limits.host_part);
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            if enter(number, settings, &limits, middle) {
                upper = middle;
            } else {
                lower = middle + 1;
            }
        }
        assert!(enter(number, settings, &limits, upper));
        assert!(!enter(number, settings, &limits, upper - 1));
        eprintln!(
            "stock charter {number}: {upper}/{} bytes, {tools} tools, empty brief, one writable git directory, no guide body",
            limits.host_part
        );
    }
}
