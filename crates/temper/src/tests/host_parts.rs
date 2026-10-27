//! Measure stock charters at smith's entrance and retain typed host-part refusals.
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
            contract: match number {
                1 => tasks::Contract::Report { words: 512 },
                4 => tasks::Contract::Verdict {
                    choices: Box::new([
                        tasks::Verdict { code: 1, words: 512, fields: Box::new([]), followups: 0 },
                        tasks::Verdict { code: 2, words: 512, fields: Box::new([]), followups: 0 },
                    ]),
                },
                _ => tasks::Contract::Change { connector: 1, kind: 2, words: 512 },
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

fn entrance(
    assignment: &engine::Assignment,
    sections: &[run::Section],
    limits: &run::Limits,
    bound: u64,
) -> run::Request {
    let mut charter = temper_engine_smith::charter(assignment);
    charter.brief.sections = sections
        .iter()
        .map(|section| run::Section { title: section.title.clone(), text: section.text.clone() })
        .collect();
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
                    writable: assignment.run.policy.modify,
                    git: true,
                    conflicts: Box::new([]),
                }]),
            }),
            transcript: None,
            resumed: false,
        },
        &mut out,
    );
    out.pop().expect("entrance answers before a turn")
}

fn enter(assignment: &engine::Assignment, sections: &[run::Section], limits: &run::Limits, bound: u64) -> bool {
    let request = entrance(assignment, sections, limits, bound);
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

fn measure(number: u32, settings: bool, name: &str, sections: &[run::Section], limits: &run::Limits) {
    let assignment = assignment(number, settings);
    let charter = temper_engine_smith::charter(&assignment);
    let tools = charter.grants.host_tools.len();
    assert_eq!(
        tools,
        if number == 4 {
            19
        } else if settings {
            39
        } else {
            37
        }
    );
    let admitted = enter(&assignment, sections, limits, limits.host_part);
    let (mut lower, mut upper) = (0, limits.host_part);
    // Vary only this effect-free harness's bound to measure a refused charter;
    // no production limit changes. The smallest admitted bound is its byte sum.
    while !enter(&assignment, sections, limits, upper) {
        upper = upper.checked_mul(2).expect("bounded fixture host part");
    }
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        if enter(&assignment, sections, limits, middle) {
            upper = middle;
        } else {
            lower = middle + 1;
        }
    }
    assert!(enter(&assignment, sections, limits, upper));
    assert!(!enter(&assignment, sections, limits, upper - 1));
    let brief: usize = sections.iter().map(|section| section.text.len()).sum();
    eprintln!(
        "stock charter {number} {name}: {upper}/{} bytes, {tools} tools, {} sections/{brief} body bytes, admitted={admitted}; one {} git directory, no guide body",
        limits.host_part,
        sections.len(),
        if assignment.run.policy.modify { "writable" } else { "read-only" }
    );
}

fn largest_brief() -> Box<[run::Section]> {
    let limits = temper_service::limits::profile(temper_service::limits::Profile::Small).root.brief;
    assert_eq!(limits.sections, 3);
    let mut left = usize::try_from(limits.brief_bytes).expect("small brief");
    let mut sections = Vec::new();
    // Longest three distinct titles the root's section translation emits.
    for (index, title) in [b"Earlier attempts".as_slice(), b"Transcript tail", b"Dependencies"].into_iter().enumerate()
    {
        let length = usize::try_from(limits.read_bytes).expect("small read").min(left - (2 - index));
        left -= length;
        sections.push(run::Section { title: title.into(), text: vec![b'x'; length].into_boxed_slice() });
    }
    assert_eq!(left, 0);
    sections.into_boxed_slice()
}

fn review_of_35_writes() -> Box<[run::Section]> {
    // A controlled review brief, not a replay of the QA repository's contents.
    let mut task = String::from("Review these 35 writes at the named head:\n");
    for number in 0..35 {
        use std::fmt::Write;
        writeln!(task, "src/review_{number:02}.rs: replace old() with checked_new().").expect("fixture string");
    }
    Box::new([
        run::Section { title: b"Task".as_slice().into(), text: task.into_bytes().into_boxed_slice() },
        run::Section { title: b"CI".as_slice().into(), text: b"Required checks passed.\n".as_slice().into() },
        run::Section {
            title: b"Pull request".as_slice().into(),
            text: b"35 files changed at the pinned head; inspect the diff before deciding.\n".as_slice().into(),
        },
    ])
}

#[test]
fn stock_charters_meet_smith_with_empty_full_and_35_write_briefs() {
    let scratch = skein_scratch::Scratch::new("stock-host-part");
    let limits = limits(scratch.path());
    for (number, settings) in [(2, false), (1, false), (1, true), (4, false)] {
        measure(number, settings, "empty brief", &[], &limits);
    }
    let largest = largest_brief();
    measure(4, false, "maximum aggregate brief", &largest, &limits);
    measure(2, false, "maximum aggregate brief", &largest, &limits);
    measure(4, false, "synthetic 35-write review brief", &review_of_35_writes(), &limits);
}

#[test]
fn a_large_prepared_brief_keeps_the_host_part_hold_reason() {
    let scratch = skein_scratch::Scratch::new("large-brief-hold");
    let limits = limits(scratch.path());
    let run::Request::Answer { answer, .. } =
        entrance(&assignment(2, false), &largest_brief(), &limits, limits.host_part)
    else {
        panic!("the full producer brief exceeds the stock host part")
    };
    let terminal = temper_engine_smith::result(answer, None).expect("typed refusal reaches the hold");
    assert_eq!(
        terminal.end,
        tasks::End::Exceeded(tasks::Exceeded { bound: tasks::Bound::HostPart, value: limits.host_part })
    );
    assert_eq!((terminal.cumulative, terminal.turns), (0, 0));
}
