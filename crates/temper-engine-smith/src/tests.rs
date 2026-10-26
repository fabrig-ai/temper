//! Typed boundary tests through the public conversions.
#![expect(clippy::disallowed_macros, reason = "tests assemble JSON cases from shared fields")]

use jig_domain_accounts as accounts;
use jig_domain_brief as brief;
use jig_domain_tasks as tasks;
use skein_lib::{Duration, ReplyTo, Token};
use smith_domain as smith;
use smith_domain_run as run;
use temper_engine_domain as engine;

const CALL_TASK: u64 = 23;

use crate::{ChangeResource, Problem, answer, call, result, start, tools, turn};

fn decoded(tool: &[u8], json: &[u8]) -> Result<crate::Decoded, Problem> {
    let input = run::HostInput::attested(json.into()).expect("bounded object exterior");
    call(run::CallName { activation: 7, completion: 2, position: 1 }, tool, &input)
}

fn decided(tool: &[u8], json: &[u8]) -> Result<engine::Call, Problem> {
    match decoded(tool, json)? {
        crate::Decoded::Call(call) => Ok(call),
        crate::Decoded::Answered(_) => panic!("decided tool answered at decode"),
    }
}

fn assignment() -> engine::Assignment {
    engine::Assignment {
        task: 23,
        attempt: 7,
        conversation: 7,
        live: jig_domain_host::LiveAsk::NONE,
        charter: 1,
        run: Box::new(engine::RunCharter {
            revision: 1,
            ended_by: jig_domain_tasks::EndedBy::Result,
            sources: Box::new([Some(1)]),
            accounts: Box::new([jig_domain_authority::AccountBinding { provider: 1, account: 1, source: 1 }]),
            placement: jig_domain_fleet::PlacementConstraint {
                channel: Token::new(1),
                kind: jig_domain_fleet::HostKind::Worker,
            },
            policy: engine::RunPolicy {
                instructions: b"Do the task".as_slice().into(),
                waiting: Duration::from_secs(1),
                resume: true,
                turns: 8,
                time: Duration::from_secs(60),
                reserve_turns: 1,
                reserve_time: Duration::from_secs(10),
                model: engine::Model {
                    program: None,
                    dialect: 1,
                    endpoint: 1,
                    name: b"scripted".as_slice().into(),
                    window: 8192,
                    output: 1024,
                    input_price: 1,
                    cached_price: 1,
                    cache_write_price: None,
                    output_price: 1,
                    price_unit: 1000,
                    efforts: Box::new([]),
                    effort: None,
                },
                alternatives: Box::new([]),
                inspect: true,
                modify: false,
                shell: false,
                agents: false,
                call_timeout: Duration::from_secs(10),
            },
            contract: tasks::Contract::Report { words: 128 },
            authority: tasks::Authority {
                tools: tasks::Tools(1),
                models: tasks::Models::NONE,
                grants: Box::new([]),
                delegation: tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 },
                budget: tasks::Budget { spend: 40, deadline: None },
                notes: tasks::Scopes(0),
                note_resources: Box::new([]),
            },
            budget: 40,
        }),
        sections: Box::new([engine::BriefSection {
            kind: engine::BriefKind::Core(brief::Core::Task),
            body: engine::BriefBody::Text(b"Say hello".as_slice().into()),
        }]),
        messages: Box::new([]),
        saved: Box::new([]),
        workspace: engine::ForgeWorkspace { repositories: Box::new([]) },
        workstream: 23,
        transcript: Box::new([]),
        answered: Box::new([]),
        settled: Box::new([]),
        grants: Box::new([accounts::Grant {
            form: accounts::Form::Credential(accounts::Scheme::Bearer),
            account: 1,
            generation: 2,
            valid: Duration::from_secs(30),
        }]),
    }
}

#[test]
fn root_charter_becomes_a_smith_start_with_ordered_brief_and_exact_budget() {
    let window = smith::Window { turns: 1, bytes: 100_000 };
    let smith::Event::Start { activation, charter, grants, transcript, window: started_window, .. } =
        start(&assignment(), None, None, ReplyTo::new(Token::new(50)), Token::new(23), window)
    else {
        panic!("start conversion produces Smith Start")
    };
    assert_eq!(activation, 7);
    assert_eq!(started_window, window);
    assert_eq!(charter.instructions.as_ref(), b"Do the task");
    assert_eq!(charter.brief.sections[0].title.as_ref(), b"Task");
    assert_eq!(charter.brief.sections[0].text.as_ref(), b"Say hello");
    assert_eq!(charter.budget.spend, 40);
    assert_eq!(charter.grants.host_tools.len(), tools(Duration::from_secs(10)).len());
    assert_eq!(grants[0].name.generation, 2);
    assert!(transcript.is_none());
}

#[test]
fn a_charters_mode_is_its_tasks_ending() {
    let change = run::outcome::ChangeSpec {
        checks_must_pass: true,
        fields: Box::new([
            run::outcome::FieldRule { name: b"title".as_slice().into(), max: 128 },
            run::outcome::FieldRule { name: b"body".as_slice().into(), max: 128 },
        ]),
    };
    for (ended_by, mode, deliver) in
        [(tasks::EndedBy::Result, run::Mode::Finite, Some(change)), (tasks::EndedBy::Requester, run::Mode::Open, None)]
    {
        let mut assignment = assignment();
        assignment.run.ended_by = ended_by;
        assignment.run.contract = tasks::Contract::Change { connector: 1, kind: 1, words: 128 };
        let charter = crate::charter(&assignment);
        assert_eq!(charter.mode, mode);
        assert!(charter.grants.wait);
        assert_eq!(charter.grants.deliver, deliver);
    }
}

#[test]
fn a_starting_run_hears_its_met_dependency_by_name() {
    // A run resumed by its last dependency's end hears the Met notice that woke it (jig's
    // domain/tasks.md, section 8.1), as a live run does.
    let mut assignment = assignment();
    assignment.messages = Box::new([tasks::Word {
        number: 2,
        from: tasks::Party::Task(2),
        kind: tasks::MessageKind::Met { dependency: 2, removed: false },
        words: Box::new([]),
        at: skein_lib::Wall::EPOCH,
        hits: 1,
        eligible: true,
    }]);
    let window = smith::Window { turns: 1, bytes: 100_000 };
    let smith::Event::Start { messages, .. } =
        start(&assignment, None, None, ReplyTo::new(Token::new(50)), Token::new(23), window)
    else {
        panic!("start conversion produces Smith Start")
    };
    assert_eq!(messages[0].label.as_ref(), b"2");
    assert_eq!(messages[0].text.as_ref(), b"T2 is done; the task can go on");
}

#[test]
fn selected_llm_bindings_reach_smith() {
    let mut assignment = assignment();
    assignment.run.policy.model.effort = Some(b"high".as_slice().into());
    let alternative = engine::Model {
        name: b"alternative".as_slice().into(),
        effort: Some(b"low".as_slice().into()),
        ..assignment.run.policy.model.clone()
    };
    assignment.run.policy.alternatives = Box::new([alternative.clone()]);
    assignment.run.sources = Box::new([Some(17), Some(23)]);
    assignment.run.accounts = Box::new([
        jig_domain_authority::AccountBinding { provider: 1, account: 11, source: 17 },
        jig_domain_authority::AccountBinding { provider: 2, account: 19, source: 23 },
    ]);
    let charter = crate::charter(&assignment);
    assert_eq!(charter.llm.account, Some(17));
    assert_eq!(charter.llm.effort.as_deref(), Some(b"high".as_slice()));
    assert_eq!(charter.models[0].account, Some(23));
    assert_eq!(charter.models[0].model, alternative.name);
    assert_eq!(charter.models[0].effort, alternative.effort);
    assignment.run.sources = Box::new([None, None]);
    let charter = crate::charter(&assignment);
    assert_eq!(charter.llm.account, None);
    assert_eq!(charter.models[0].account, None);
}

#[test]
fn message_schema_decodes_a_named_question_and_rejects_a_malformed_body() {
    assert_eq!(
        decided(b"message", br#"{"target":23,"form":"question","words":"Ready?"}"#),
        Ok(engine::Call {
            completion: 2,
            position: 1,
            tool: engine::Tool::Message {
                target: 23,
                form: engine::MessageForm::Question,
                words: b"Ready?".as_slice().into(),
            },
        })
    );
    assert_eq!(decided(b"message", br#"{"target":0,"form":"words","words":"x"}"#), Err(Problem::Range(b"target")));
    assert_eq!(decided(b"message", br#"{"target":23,,"form":"words","words":"x"}"#), Err(Problem::Malformed));
}

#[test]
fn every_declared_tool_decodes_its_minimum_shape_and_rejects_a_wrong_shape() {
    let cases: &[(&[u8], &[u8], &[u8])] = &[
        (b"read_items", br#"{"repository":"qa/pilot","since":0,"page":1}"#, br#"{"repository":"qa/pilot","page":0}"#),
        (b"read_item", br#"{"repository":"qa/pilot","number":1,"after":0}"#, br#"{"repository":"qa/pilot","page":0}"#),
        (b"read_pull", br#"{"repository":"qa/pilot","number":1}"#, br#"{"repository":"qa/pilot","page":0}"#),
        (b"read_pull_for", br#"{"repository":"qa/pilot","head":"h","base":"b"}"#, br#"{"repository":"qa/pilot","page":0}"#),
        (b"read_reviews", br#"{"repository":"qa/pilot","number":1,"page":1}"#, br#"{"repository":"qa/pilot","page":0}"#),
        (b"read_statuses", br#"{"repository":"qa/pilot","commit":"0000000000000000000000000000000000000000000000000000000000000000","page":1}"#, br#"{"repository":"qa/pilot","page":0}"#),
        (b"read_remarks", br#"{"repository":"qa/pilot","number":1,"review":1,"page":1}"#, br#"{"repository":"qa/pilot","page":0}"#),
        (b"read_branch", br#"{"repository":"qa/pilot","branch":"main"}"#, br#"{"repository":"qa/pilot","page":0}"#),
        (b"read_branches", br#"{"repository":"qa/pilot","prefix":"temper/"}"#, br#"{"repository":"qa/pilot","kind":"wrong"}"#),
        (b"read_pull_files", br#"{"repository":"qa/pilot","number":1,"head":"0000000000000000000000000000000000000000000000000000000000000000","page":1}"#, br#"{"repository":"qa/pilot","page":0}"#),
        (b"read_compare", br#"{"repository":"qa/pilot","before":"0000000000000000000000000000000000000000000000000000000000000000","after":"0000000000000000000000000000000000000000000000000000000000000000"}"#, br#"{"repository":"qa/pilot","page":0}"#),
        (b"read_checks", br#"{"repository":"qa/pilot","commit":"0000000000000000000000000000000000000000000000000000000000000000"}"#, br#"{"repository":"qa/pilot","page":0}"#),
        (b"read_file", br#"{"repository":"qa/pilot","head":"0000000000000000000000000000000000000000000000000000000000000000","path":"file","max_bytes":32}"#, br#"{"repository":"qa/pilot","page":0}"#),
        (b"read_job", br#"{"repository":"qa/pilot","head":"0000000000000000000000000000000000000000000000000000000000000000","run":1,"job":1,"attempt":1,"max_bytes":32}"#, br#"{"repository":"qa/pilot","page":0}"#),
        (b"read_protection", br#"{"repository":"qa/pilot","branch":"main"}"#, br#"{"repository":"qa/pilot","page":0}"#),
        (b"read_settings", br#"{"repository":"qa/pilot"}"#, br#"{"repository":"qa/pilot","kind":"wrong"}"#),
        (b"read_collaborators", br#"{"repository":"qa/pilot","page":1}"#, br#"{"repository":"qa/pilot","page":0}"#),
        (b"read_permission", br#"{"repository":"qa/pilot","user":1}"#, br#"{"repository":"qa/pilot","page":0}"#),
        (b"effect_forge", br#"{"repository":{"forge":1,"repository":2},"resource":{"kind":"repository"},"write":{"kind":"close","number":3}}"#, br#"{"repository":{"forge":1,"repository":2},"resource":{},"write":{"kind":"close","number":3}}"#),
        (b"subscribe_forge", br#"{"topic":{"repository":{"forge":1,"repository":2},"kind":"pull","number":3}}"#, br#"{"topic":{}}"#),
        (b"change", br#"{"words":"edit README"}"#, br#"{"words":""}"#),
        (b"goal", br#"{"words":"plan"}"#, br"{}"),
        (b"delegate", br#"{"batch":[]}"#, br#"{"batch":{}}"#),
        (b"amend", br#"{"target":3,"revision":1,"amendment":{"reason":"x"}}"#, br#"{"target":3,"amendment":{}}"#),
        (b"message", br#"{"target":3,"form":"words","words":"x"}"#, br#"{"target":3,"form":"unknown","words":"x"}"#),
        (b"cancel", br#"{"target":3,"reason":"x"}"#, br#"{"target":3}"#),
        (b"release", br#"{"target":3,"revision":1}"#, br#"{"target":0}"#),
        (b"introduce", br#"{"left":3,"right":4}"#, br#"{"left":3}"#),
        (b"unsubscribe", br#"{"subscription":3}"#, br#"{"subscription":"x"}"#),
        (b"withdraw", br#"{"proposal":3}"#, br#"{"proposal":0}"#),
        (b"decide_escalation", br#"{"task":3,"revision":1,"decision":"release"}"#, br#"{"task":3,"revision":1,"decision":"unknown"}"#),
        (b"decide", br#"{"proposer":3,"proposal":1,"decision":"accept"}"#, br#"{"proposer":3,"proposal":1,"decision":"unknown"}"#),
        (b"subscribe", br#"{"kind":"task","target":3}"#, br#"{"kind":"task"}"#),
        (b"propose", br#"{"action":"release","task":3,"reason":"x"}"#, br#"{"action":"release","reason":"x"}"#),
        (b"note", br#"{"scope":{"kind":"project","project":1},"description":"slow start","body":"warm it first"}"#, br#"{"scope":{"kind":"project"},"description":"slow start","body":"warm it first"}"#),
        (b"read_docs", br#"{"topic":"box"}"#, br#"{"topic":3}"#),
        (b"recall", br#"{"kind":"name","name":1}"#, br#"{"kind":"name","name":0}"#),
    ];
    let declared = tools(Duration::from_secs(10));
    assert_eq!(declared.len(), cases.len());
    for tool in &declared {
        crate::json::parse(&tool.schema).expect("declared JSON schema");
        assert!(tool.description.len() <= 8_192, "description: {:?}", tool.name);
        assert!(tool.schema.len() <= 65_536, "schema: {:?}", tool.name);
    }
    for (name, valid, invalid) in cases {
        let mut found = false;
        for tool in &declared {
            if tool.name.as_ref() == *name {
                found = true;
            }
        }
        assert!(found, "tool declared: {name:?}");
        assert!(decoded(name, valid).is_ok(), "valid input for {name:?}");
        assert!(decoded(name, invalid).is_err(), "invalid input for {name:?}");
    }
}

#[test]
fn delegate_tracking_is_optional_and_requires_a_priority_object() {
    let member = r#""executor":{"kind":"agent","charter":1},"spec":{"words":"x"},"contract":{"kind":"report","words":128},"authority":{"tools":1,"grants":[],"delegation":{"kinds":[],"tasks":0,"depth":0},"budget":{"spend":40},"notes":0}"#;
    for (field, expected) in [("", None), (r#", "tracked":{"priority":4294967295}"#, Some(u32::MAX))] {
        let input = format!("{{\"batch\":[{{{member}{field}}}]}}");
        let engine::Tool::Delegate { batch } = decided(b"delegate", input.as_bytes()).expect("delegate").tool else {
            panic!("delegate tool")
        };
        assert_eq!(batch[0].tracked, expected);
        assert_eq!(batch[0].label, 0);
    }
    for (tracked, problem) in [
        ("8", Problem::Type(b"tracked")),
        (r#""untracked""#, Problem::Type(b"tracked")),
        (r#"{"priority":"8"}"#, Problem::Type(b"priority")),
        (r#"{"priority":4294967296}"#, Problem::Range(b"priority")),
        ("{}", Problem::Missing(b"priority")),
    ] {
        let input = format!("{{\"batch\":[{{{member},\"tracked\":{tracked}}}]}}");
        assert_eq!(decided(b"delegate", input.as_bytes()), Err(problem), "{tracked}");
    }
}

#[test]
fn only_a_priority_amendment_can_omit_its_revision() {
    let engine::Tool::Amend { target, revision, amendment } =
        decided(b"amend", br#"{"target":3,"amendment":{"reason":"x","tracked":{"priority":8}}}"#)
            .expect("priority without revision")
            .tool
    else {
        panic!("amend tool")
    };
    assert_eq!((target, revision), (3, 0));
    assert_eq!(amendment.tracked, Some(tasks::Tracking::Tracked { priority: 8 }));
    for other in [
        r#", "spec":{"words":"x"}"#,
        r#", "wake":{"words":{"kind":"never"},"notices":{"kind":"never"},"news":{"kind":"never"},"results":"never","questions":false,"answers":false,"timers":false}"#,
        r#", "dependencies":[]"#,
        r#", "authority":{"tools":1,"grants":[],"delegation":{"kinds":[],"tasks":0,"depth":0},"budget":{"spend":40},"notes":0}"#,
        r#", "instructions":"""#,
        r#", "llm":{"main":{"model":0}}"#,
    ] {
        let input =
            format!("{{\"target\":3,\"amendment\":{{\"reason\":\"x\",\"tracked\":{{\"priority\":8}}{other}}}}}");
        assert_eq!(decided(b"amend", input.as_bytes()), Err(Problem::Missing(b"revision")), "{other}");
    }
    assert_eq!(
        decided(b"amend", br#"{"target":3,"amendment":{"reason":"x","tracked":"untracked"}}"#),
        Err(Problem::Missing(b"revision"))
    );
    assert_eq!(
        decided(b"amend", br#"{"target":3,"revision":"0","amendment":{"reason":"x","tracked":{"priority":8}}}"#),
        Err(Problem::Type(b"revision"))
    );
}

#[test]
fn amendments_replace_tracking_and_instructions_including_empty_words() {
    for instructions in ["x", ""] {
        let input = format!(
            "{{\"target\":3,\"revision\":1,\"amendment\":{{\"reason\":\"x\",\"tracked\":\"untracked\",\"instructions\":\"{instructions}\"}}}}"
        );
        let engine::Tool::Amend { amendment, .. } = decided(b"amend", input.as_bytes()).expect("amend").tool else {
            panic!("amend tool")
        };
        assert_eq!(amendment.tracked, Some(tasks::Tracking::Untracked));
        assert_eq!(amendment.instructions.as_deref(), Some(instructions.as_bytes()));
    }
    for (tracked, problem) in [
        ("8", Problem::Type(b"tracked")),
        (r#""tracked""#, Problem::Range(b"tracked")),
        (r#"{"priority":"8"}"#, Problem::Type(b"priority")),
        (r#"{"priority":4294967296}"#, Problem::Range(b"priority")),
        ("{}", Problem::Missing(b"priority")),
    ] {
        let input = format!("{{\"target\":3,\"revision\":1,\"amendment\":{{\"reason\":\"x\",\"tracked\":{tracked}}}}}");
        assert_eq!(decided(b"amend", input.as_bytes()), Err(problem), "{tracked}");
    }
    assert_eq!(
        decided(b"amend", br#"{"target":3,"revision":1,"amendment":{"reason":"x","instructions":8}}"#),
        Err(Problem::Type(b"instructions"))
    );
}

#[test]
fn the_declared_deadline_example_decodes_the_whole_authority() {
    const EXAMPLE: &[u8] = br#"{"target":3,"revision":1,"amendment":{"reason":"Finish by Friday.","authority":{"tools":5,"grants":[],"delegation":{"kinds":[],"tasks":0,"depth":0},"budget":{"spend":1000,"deadline":1793404800000000000},"notes":0}}}"#;
    for tool in tools(Duration::from_secs(10)) {
        if tool.name.as_ref() == b"amend" {
            let schema = crate::json::parse(&tool.schema).expect("amend schema");
            assert_eq!(
                schema.required(b"examples").expect("examples").array(b"examples").expect("examples array"),
                &[crate::json::parse(EXAMPLE).expect("example")]
            );
        }
    }
    let engine::Tool::Amend { target, revision, amendment } =
        decided(b"amend", EXAMPLE).expect("deadline example").tool
    else {
        panic!("amend tool")
    };
    assert_eq!((target, revision), (3, 1));
    assert_eq!(
        amendment.authority,
        Some(tasks::Authority {
            tools: tasks::Tools(5),
            grants: Box::new([]),
            delegation: tasks::Delegation { kinds: Box::new([]), tasks: 0, depth: 0 },
            budget: tasks::Budget {
                spend: 1000,
                deadline: Some(skein_lib::Wall::from_nanos(1_793_404_800_000_000_000))
            },
            notes: tasks::Scopes(0),
            note_resources: Box::new([]),
            models: tasks::Models::NONE,
        })
    );
    assert_eq!(amendment.reason.as_ref(), b"Finish by Friday.");
    assert!(amendment.tracked.is_none() && amendment.instructions.is_none());
}

const DELEGATE_MEMBER: &str = r#""executor":{"kind":"agent","charter":1},"spec":{"words":"x"},"contract":{"kind":"report","words":128},"authority":{"tools":1,"grants":[],"delegation":{"kinds":[],"tasks":0,"depth":0},"budget":{"spend":40},"notes":0}"#;

#[test]
fn a_delegate_deadline_in_seconds_is_a_span() {
    for (field, expected) in [
        ("", tasks::Deadline::Named),
        (r#", "deadline_seconds":14400"#, tasks::Deadline::Span(Duration::from_secs(14_400))),
    ] {
        let input = format!("{{\"batch\":[{{{DELEGATE_MEMBER}{field}}}]}}");
        let engine::Tool::Delegate { batch } = decided(b"delegate", input.as_bytes()).expect("delegate deadline").tool
        else {
            panic!("delegate tool")
        };
        assert_eq!(batch[0].deadline, expected);
        assert_eq!(batch[0].authority.budget.deadline, None);
    }
}

#[test]
fn a_delegate_names_one_deadline() {
    const MAX_SECONDS: u64 = u64::MAX / 1_000_000_000;
    for (seconds, wall, accepted) in
        [(1, true, false), (0, false, false), (MAX_SECONDS + 1, false, false), (MAX_SECONDS, false, true)]
    {
        let member = if wall {
            DELEGATE_MEMBER.replace(r#""spend":40"#, r#""spend":40,"deadline":20000000000"#)
        } else {
            DELEGATE_MEMBER.into()
        };
        let input = format!("{{\"batch\":[{{{member},\"deadline_seconds\":{seconds}}}]}}");
        let result = decided(b"delegate", input.as_bytes());
        if accepted {
            let engine::Tool::Delegate { batch } = result.expect("largest whole second").tool else {
                panic!("delegate tool")
            };
            assert_eq!(batch[0].deadline, tasks::Deadline::Span(Duration::from_secs(MAX_SECONDS)));
        } else {
            assert_eq!(result, Err(Problem::Range(b"deadline_seconds")), "seconds={seconds}, wall={wall}");
        }
    }
}

#[test]
fn proposed_amendments_share_controls_but_still_require_a_revision() {
    let engine::Tool::Propose { action: engine::ProposedAction::Amend { revision, amendment, .. }, .. } =
        decided(b"propose", br#"{"action":"amend","task":3,"revision":1,"amendment":{"reason":"x","tracked":{"priority":8},"instructions":""},"reason":"x"}"#)
            .expect("proposed amendment").tool
    else { panic!("proposed amendment") };
    assert_eq!(revision, 1);
    assert_eq!(amendment.tracked, Some(tasks::Tracking::Tracked { priority: 8 }));
    assert_eq!(amendment.instructions.as_deref(), Some(b"".as_slice()));
    assert_eq!(
        decided(
            b"propose",
            br#"{"action":"amend","task":3,"amendment":{"reason":"x","tracked":{"priority":8}},"reason":"x"}"#
        ),
        Err(Problem::Missing(b"revision"))
    );
}

#[test]
fn an_effect_proposal_decodes_its_connector_payload_and_rejects_an_unknown_resource() {
    assert_eq!(
        decided(b"propose", br#"{"action":"effect","repository":{"forge":1,"repository":2},"resource":{"kind":"repository"},"write":{"kind":"close","number":3},"reason":"x"}"#),
        Ok(engine::Call {
            completion: 2, position: 1,
            tool: engine::Tool::Propose {
                action: engine::ProposedAction::Effect {
                    repository: skein_forge::api::Repository { forge: 1, repository: 2 },
                    resource: temper_engine_domain_forge::What::Repository,
                    write: Box::new(skein_forge::api::Write::Close { number: 3 }),
                },
                reason: b"x".as_slice().into(), as_holder: false,
            },
        })
    );
    assert_eq!(decided(b"propose", br#"{"action":"effect","repository":{"forge":1,"repository":2},"resource":{},"write":{"kind":"close","number":3},"reason":"x"}"#), Err(Problem::Missing(b"kind")));
}

fn spend(units: u64) -> run::Spend {
    run::Spend {
        turns: 1,
        input: Some(2),
        output: Some(3),
        cache_read: Some(0),
        cache_write: Some(0),
        reasoning: None,
        units,
    }
}

fn change_answer() -> run::Answer {
    run::Answer::Accepted {
        outcome: run::outcome::Declared::Change(run::outcome::Change {
            fields: Box::new([
                run::outcome::Field { name: b"title".as_slice().into(), value: b"Fix".as_slice().into() },
                run::outcome::Field { name: b"body".as_slice().into(), value: b"Details".as_slice().into() },
            ]),
        }),
        spent: spend(9),
        turns: 1,
    }
}

#[test]
fn smith_terminals_preserve_charge_and_the_root_result_contract() {
    assert_eq!(
        result(run::Answer::Parked { spent: spend(5), turns: 1 }, None),
        Ok(crate::Terminal { cumulative: 5, turns: 1, end: tasks::End::Parked })
    );
    assert_eq!(
        result(
            run::Answer::Accepted {
                outcome: run::outcome::Declared::Report(run::outcome::Report {
                    text: b"done".as_slice().into(),
                    fields: Box::new([]),
                }),
                spent: spend(7),
                turns: 1,
            },
            None,
        ),
        Ok(crate::Terminal {
            cumulative: 7,
            turns: 1,
            end: tasks::End::Finished {
                result: tasks::TaskResult::Report { words: b"done".as_slice().into() },
                cancel_delegates: false,
            },
        })
    );
    assert_eq!(result(change_answer(), None), Err(Problem::Missing(b"resource")));
    assert_eq!(
        result(change_answer(), Some(ChangeResource { connector: 1, kind: 2, resource: 42 })),
        Ok(crate::Terminal {
            cumulative: 9,
            turns: 1,
            end: tasks::End::Finished {
                result: tasks::TaskResult::Change {
                    connector: 1,
                    kind: 2,
                    resource: 42,
                    words: b"Details".as_slice().into(),
                },
                cancel_delegates: false,
            },
        })
    );
}

#[test]
fn root_call_answers_become_bounded_smith_text_or_error() {
    let done = answer(&temper_engine_domain::CallAnswer::Controlled);
    assert_eq!(done.text(), b"Done");
    assert!(!done.error());
    let denied = answer(&temper_engine_domain::CallAnswer::Unavailable);
    assert!(denied.error());
    assert!(!denied.text().is_empty());
}

#[test]
fn a_charter_resumes_when_its_policy_does_and_turns_are_kept() {
    let mut assignment = assignment();
    for (resume, turns, expected) in
        [(false, false, false), (false, true, false), (true, false, false), (true, true, true)]
    {
        assignment.run.policy.resume = resume;
        assignment.transcript = if turns { Box::new([b"turn".as_slice().into()]) } else { Box::new([]) };
        assert_eq!(crate::charter(&assignment).resume, expected, "resume={resume}, turns={turns}");
    }
}

#[test]
fn a_job_answer_names_its_failed_step_before_its_log() {
    use skein_forge::api;
    let attempt = api::JobAttempt { head: [1; 32], run: 7, job: 8, attempt: 2 };
    let step = Some(api::Step { number: 1, name: Box::from(&b"build"[..]) });
    let read = api::Answer::Job { attempt, step, log: Box::from(&b"failing check build 3\n"[..]), truncated: true };
    let rendered = answer(&engine::CallAnswer::ForgeRead(Box::new(Ok(read))));
    assert_eq!(rendered.text(), b"Job 8 run 7, failed step 1 build:\n[earlier log bytes cut]\nfailing check build 3\n");
    assert!(!rendered.error(), "A read answer is no error");
}

#[test]
fn a_job_answer_with_no_failed_step_says_none_failed() {
    use skein_forge::api;
    let attempt = api::JobAttempt { head: [1; 32], run: 7, job: 8, attempt: 2 };
    let step = None;
    let read = api::Answer::Job { attempt, step, log: Box::from(&b"failing check build 3\n"[..]), truncated: false };
    let rendered = answer(&engine::CallAnswer::ForgeRead(Box::new(Ok(read))));
    assert_eq!(rendered.text(), b"Job 8 run 7, no step failed:\nfailing check build 3\n");
    assert!(!rendered.error(), "A read answer is no error");
}

#[test]
fn a_job_answer_with_an_empty_step_name_names_its_number_alone() {
    use skein_forge::api;
    let attempt = api::JobAttempt { head: [1; 32], run: 7, job: 8, attempt: 2 };
    let step = Some(api::Step { number: 1, name: Box::new([]) });
    let read = api::Answer::Job { attempt, step, log: Box::from(&b"failing check build 3\n"[..]), truncated: false };
    let rendered = answer(&engine::CallAnswer::ForgeRead(Box::new(Ok(read))));
    assert_eq!(rendered.text(), b"Job 8 run 7, failed step 1:\nfailing check build 3\n");
    assert!(!rendered.error(), "A read answer is no error");
}

struct RevisionCall {
    name: &'static [u8],
    missing: &'static [u8],
    valid: &'static [u8],
    again: Option<bool>,
}
const REVISION_CALLS: &[RevisionCall] = &[
    RevisionCall {
        name: b"amend",
        missing: br#"{"target":23,"amendment":{"reason":"change"}}"#,
        valid: br#"{"target":23,"revision":0,"amendment":{"reason":"change"}}"#,
        again: None,
    },
    RevisionCall {
        name: b"release",
        missing: br#"{"target":23}"#,
        valid: br#"{"target":23,"revision":0}"#,
        again: None,
    },
    RevisionCall {
        name: b"release",
        missing: br#"{"target":23,"again":false}"#,
        valid: br#"{"target":23,"revision":0,"again":false}"#,
        again: Some(false),
    },
    RevisionCall {
        name: b"release",
        missing: br#"{"target":23,"again":true}"#,
        valid: br#"{"target":23,"revision":0,"again":true}"#,
        again: Some(true),
    },
];
#[test]
fn amend_and_release_calls_name_their_revision() {
    for row in REVISION_CALLS {
        assert_eq!(decided(row.name, row.missing), Err(Problem::Missing(b"revision")), "{:?}", row.name);
        let expected = if row.name == b"amend" {
            engine::Tool::Amend {
                target: CALL_TASK,
                revision: 0,
                amendment: tasks::Amendment {
                    tracked: None,
                    addressed: None,
                    instructions: None,
                    release: None,
                    spec: None,
                    wake: None,
                    dependencies: None,
                    authority: None,
                    llm: None,
                    reason: Box::new(*b"change"),
                },
            }
        } else {
            engine::Tool::Release { target: CALL_TASK, revision: 0, again: row.again }
        };
        let body = decided(row.name, row.valid).expect("revision-bearing tool");
        assert_eq!(body.tool, expected, "{:?}", row.name);
    }
}

const ANSWER_CASES: &[(engine::CallAnswer, &[u8], bool)] = &[
    (
        engine::CallAnswer::Moved {
            task: CALL_TASK,
            now: jig_domain_people::Moved::Hold { revision: 2, reason: jig_domain_people::HoldReason::Circle },
        },
        b"Moved: task 23 is held at hold revision 2 now, for a circle of waits",
        true,
    ),
    (
        engine::CallAnswer::Again { task: CALL_TASK, required: true },
        b"Refused: task 23 holds an effect whose outcome is uncertain; say again: true or false",
        true,
    ),
    (
        engine::CallAnswer::Again { task: CALL_TASK, required: false },
        b"Refused: task 23 holds no uncertain effect; leave again out",
        true,
    ),
    (
        engine::CallAnswer::PassedUp { task: CALL_TASK },
        b"Beyond your authority: task 23's escalation was passed up",
        false,
    ),
    (engine::CallAnswer::Revised { task: CALL_TASK, revision: 2 }, b"Done: task 23 is at spec revision 2", false),
    (
        engine::CallAnswer::Amended { task: 3, revision: 1, changed: engine::Changed { tracked: None, fields: 0 } },
        b"Done: task 3 is at spec revision 1",
        false,
    ),
    (
        engine::CallAnswer::Amended { task: 3, revision: 1, changed: engine::Changed {
            tracked: None, fields: engine::Changed::AUTHORITY | engine::Changed::INSTRUCTIONS,
        } },
        b"Done: task 3 is at spec revision 1; changed: authority, instructions",
        false,
    ),
    (
        engine::CallAnswer::Amended { task: 3, revision: 0, changed: engine::Changed {
            tracked: Some(tasks::Tracking::Tracked { priority: 8 }), fields: 0,
        } },
        b"Done: task 3 is at spec revision 0; changed: priority 8",
        false,
    ),
    (
        engine::CallAnswer::Amended { task: 3, revision: 2, changed: engine::Changed {
            tracked: Some(tasks::Tracking::Untracked), fields: engine::Changed::SPEC,
        } },
        b"Done: task 3 is at spec revision 2; changed: untracked, spec",
        false,
    ),
    (
        engine::CallAnswer::Amended { task: 3, revision: 2, changed: engine::Changed {
            tracked: Some(tasks::Tracking::Tracked { priority: u32::MAX }),
            fields: engine::Changed::SPEC | engine::Changed::WAKE | engine::Changed::DEPENDENCIES
                | engine::Changed::AUTHORITY | engine::Changed::INSTRUCTIONS | engine::Changed::LLM,
        } },
        b"Done: task 3 is at spec revision 2; changed: priority 4294967295, spec, wake, dependencies, authority, instructions, llm",
        false,
    ),
    (
        engine::CallAnswer::Moved { task: CALL_TASK, now: jig_domain_people::Moved::Spec { revision: 2 } },
        b"Moved: task 23 is at spec revision 2 now",
        true,
    ),
    (
        engine::CallAnswer::Moved {
            task: CALL_TASK,
            now: jig_domain_people::Moved::Hold { revision: 2, reason: jig_domain_people::HoldReason::Stopped },
        },
        b"Moved: task 23 is held at hold revision 2 now, because it was stopped",
        true,
    ),
    (
        engine::CallAnswer::Moved { task: CALL_TASK, now: jig_domain_people::Moved::Person { revision: 2 } },
        b"Moved: task 23 is at person revision 2 now",
        true,
    ),
    (
        engine::CallAnswer::Moved { task: CALL_TASK, now: jig_domain_people::Moved::Note { revision: 2 } },
        b"Moved: task 23 is at note revision 2 now",
        true,
    ),
];

#[test]
fn each_of_jigs_new_call_answers_renders_its_task_and_value() {
    for (value, words, error) in ANSWER_CASES {
        let rendered = answer(value);
        assert_eq!(rendered.text(), *words, "{value:?}");
        assert_eq!(rendered.error(), *error, "{value:?}");
    }
}

#[test]
fn a_job_request_is_not_a_turn() {
    assert_eq!(
        turn(smith::Request::Job { owner: Token::new(1), act: smith::tools::JobAct::Hasten }, Box::default()).err(),
        Some(Problem::Type(b"request"))
    );
}

#[test]
fn account_notices_are_not_turns_and_terminal_failure_classes_stay_distinct() {
    for notice in [
        smith::Request::Unauthorized { grant: smith::GrantName { account: 17, generation: 3 } },
        smith::Request::Limited { account: 17, reset: None },
        smith::Request::Limited { account: 17, reset: Some(Duration::ZERO) },
        smith::Request::Refused { account: 17, code: b"quota".as_slice().into(), reason: b"refused".as_slice().into() },
    ] {
        assert_eq!(turn(notice, Box::default()), Err(Problem::Type(b"request")));
    }
    for (failure, class) in [
        (run::Failure::Stale, tasks::Class::Transient),
        (run::Failure::Transcript(run::TranscriptRefusal::Malformed), tasks::Class::Transient),
        (run::Failure::Budget { exhausted: run::Exhausted::Turns, used: Some(2), bound: Some(2) }, tasks::Class::Run),
        (run::Failure::Policy(run::Policy::Unfinished { nudges: 1, rejected: 1 }), tasks::Class::Run),
        (run::Failure::Model(run::Fault::Provider), tasks::Class::Run),
        (run::Failure::Cancelled, tasks::Class::Agent),
    ] {
        assert_eq!(
            result(run::Answer::Failed { failure, spent: spend(13), turns: 2 }, None),
            Ok(crate::Terminal { cumulative: 13, turns: 2, end: tasks::End::Failed(class) })
        );
    }
}

#[test]
fn queued_names_the_forge_outcome() {
    let rendered = answer(&engine::CallAnswer::ForgeEffect {
        entry: 1,
        deadline: skein_lib::Wall::EPOCH,
        outcome: Some(skein_forge_client::Outcome::Queued),
    });
    assert_eq!(rendered.text(), b"Forge effect 1: queued");
    assert!(!rendered.error());
}

#[test]
fn not_installed_names_the_forge_refusal() {
    let rendered = answer(&engine::CallAnswer::ForgeEffectRefused(skein_forge::api::Error::NotInstalled));
    assert_eq!(rendered.text(), b"Forge effect refused: not installed");
    assert!(rendered.error());
}

#[test]
fn a_delegate_keeps_its_llm_preference() {
    let call = decided(
        b"delegate",
        br#"{"batch":[{
        "executor":{"kind":"agent","charter":1},"spec":{"words":"work"},
        "contract":{"kind":"report","words":32},
        "authority":{"tools":0,"grants":[],"delegation":{"kinds":[],"tasks":0,"depth":0},
            "budget":{"spend":0},"notes":0},
        "llm":{"main":{"model":2,"effort":1},"subagents":[{"model":3}],"account":43}
    }]}"#,
    )
    .expect("valid delegate");
    let engine::Tool::Delegate { batch } = call.tool else { panic!("delegate") };
    assert_eq!(
        batch[0].llm,
        Some(tasks::LlmChoice {
            account: Some(43),
            main: tasks::ModelEffort { model: 2, effort: Some(1) },
            subagents: Box::new([tasks::ModelEffort { model: 3, effort: None }]),
        })
    );
}

#[test]
fn an_amendment_keeps_its_llm_preference() {
    let call = decided(
        b"amend",
        br#"{"target":23,"revision":7,"amendment":{
        "reason":"change payer","llm":{"main":{"model":2,"effort":1},"subagents":[{"model":3}],"account":43}
    }}"#,
    )
    .expect("valid amendment");
    let engine::Tool::Amend { amendment, .. } = call.tool else { panic!("amendment") };
    assert_eq!(
        amendment.llm,
        Some(tasks::LlmChoice {
            account: Some(43),
            main: tasks::ModelEffort { model: 2, effort: Some(1) },
            subagents: Box::new([tasks::ModelEffort { model: 3, effort: None }]),
        })
    );
}

fn refusal_words(why: tasks::Refusal, blocked: Option<Box<[u64]>>, expected: &[u8]) {
    let value =
        engine::CallAnswer::ControlRefused(tasks::Problem { task: Some(99), capacity: None, why, blocked_by: blocked });
    let rendered = answer(&value);
    assert_eq!(rendered.text(), expected);
    assert!(rendered.error());
}

#[test]
fn a_circle_names_its_tasks() {
    refusal_words(
        tasks::Refusal::Circle { cut: false },
        Some(Box::new([3, 5, 7])),
        b"Refused: these tasks would wait for one another: T3, T5, T7",
    );
}

#[test]
fn a_cut_circle_says_its_lists_are_incomplete() {
    refusal_words(
        tasks::Refusal::Circle { cut: true },
        Some(Box::new([3, 5, 7])),
        b"Refused: these tasks would wait for one another: T3, T5, T7; some task lists are incomplete",
    );
}

#[test]
fn a_failed_dependency_is_named() {
    refusal_words(
        tasks::Refusal::DependencyEnded { how: tasks::Status::Failed },
        Some(Box::new([4])),
        b"Refused: dependency T4 failed",
    );
}

#[test]
fn a_cancelled_dependency_is_named() {
    refusal_words(
        tasks::Refusal::DependencyEnded { how: tasks::Status::Cancelled },
        Some(Box::new([4])),
        b"Refused: dependency T4 was cancelled",
    );
}

#[test]
fn a_done_dependency_is_named() {
    refusal_words(
        tasks::Refusal::DependencyEnded { how: tasks::Status::Done },
        Some(Box::new([4])),
        b"Refused: T4 is done",
    );
}

#[test]
fn kept_names_each_dependency() {
    refusal_words(
        tasks::Refusal::Kept,
        Some(Box::new([4, 6])),
        b"Refused: kept until T4 is done, kept until T6 is done",
    );
}

#[test]
fn a_refused_effect_names_its_pending_dependencies() {
    let value = engine::CallAnswer::EffectRefused(tasks::Problem {
        task: Some(99),
        capacity: None,
        why: tasks::Refusal::Kept,
        blocked_by: Some(Box::new([4, 6])),
    });
    let rendered = answer(&value);
    assert_eq!(rendered.text(), b"Refused: kept until T4 is done, kept until T6 is done");
    assert!(rendered.error());
}

#[test]
fn dependency_refusals_without_tasks_are_plain_lines() {
    let lists: [Option<Box<[u64]>>; 2] = [None, Some(Box::new([]))];
    for blocked in lists {
        for (why, expected) in [
            (tasks::Refusal::Circle { cut: false }, b"Refused: these tasks would wait for one another".as_slice()),
            (tasks::Refusal::Circle { cut: true }, b"Refused: these tasks would wait for one another"),
            (tasks::Refusal::DependencyEnded { how: tasks::Status::Failed }, b"Refused: a dependency has ended"),
            (tasks::Refusal::Kept, b"Refused: kept by its dependencies"),
        ] {
            refusal_words(why, blocked.clone(), expected);
        }
    }
}

#[test]
fn change_decodes_words_and_an_optional_repository() {
    for input in [br#"{"words":"edit README"}"#.as_slice(), br#"{"words":"edit README","repository":"work/hello"}"#] {
        let call = decided(b"change", input).expect("change is declared and decoded");
        assert_eq!(call.completion, 2);
        assert_eq!(call.position, 1);
        let engine::Tool::Change { words, repository } = call.tool else { panic!("change") };
        assert_eq!(words.as_ref(), b"edit README");
        assert_eq!(repository.as_deref(), if input.contains(&b'/') { Some(b"work/hello".as_slice()) } else { None });
    }
}

#[test]
fn change_refuses_missing_words() {
    assert_eq!(decided(b"change", b"{}"), Err(Problem::Missing(b"words")));
}

#[test]
fn change_refuses_empty_words() {
    assert_eq!(decided(b"change", br#"{"words":""}"#), Err(Problem::Range(b"words")));
}

#[test]
fn change_refuses_non_string_words() {
    assert_eq!(decided(b"change", br#"{"words":1}"#), Err(Problem::Type(b"words")));
}

#[test]
fn change_refuses_malformed_repository_names() {
    for name in ["", "hello", "/hello", "work/", "work/hello/other"] {
        let input = format!(r#"{{"words":"edit README","repository":"{name}"}}"#);
        assert_eq!(decided(b"change", input.as_bytes()), Err(Problem::Range(b"repository")));
    }
}

#[test]
fn a_delegate_executor_or_grant_denial_suggests_change() {
    use jig_domain_authority::{Answer, Finding, Source};
    for finding in [Finding::Executor { source: Source::Task }, Finding::Grant { source: Source::Task }] {
        let rendered =
            answer(&engine::CallAnswer::DelegationDenied { answer: Answer::Refuse, findings: Box::new([finding]) });
        assert!(rendered.error());
        assert!(rendered.text().ends_with(b"For a change to a repository, call change with what to change."));
        assert!(rendered.text().len() <= run::HostAnswer::CAPACITY);
    }
}

#[test]
#[expect(clippy::manual_map, reason = "the strict subset constructs optional text without closures")]
fn change_repository_refusals_render_the_three_choices() {
    struct Case {
        named: Option<&'static [u8]>,
        repositories: &'static [&'static [u8]],
        expected: &'static [u8],
    }
    for case in [
        Case { named: None, repositories: &[], expected: b"this project has no repository to change" },
        Case {
            named: None,
            repositories: &[b"work/hello", b"work/other"],
            expected: b"name the repository: work/hello, work/other",
        },
        Case {
            named: Some(b"work/x"),
            repositories: &[b"work/hello"],
            expected: b"no repository work/x here; this project has: work/hello",
        },
    ] {
        let mut repositories = skein_lib::List::with_capacity(2);
        for name in case.repositories {
            repositories.push(Box::from(*name)).expect("fixture repositories");
        }
        let requested = match case.named {
            Some(name) => Some(Box::from(name)),
            None => None,
        };
        let value = engine::CallAnswer::ChangeRefused {
            problem: Box::new(tasks::Problem::new(None, tasks::Refusal::Unknown)),
            named: requested,
            repositories: repositories.into_boxed(),
        };
        let rendered = answer(&value);
        assert!(rendered.error());
        assert_eq!(rendered.text(), case.expected);
        assert!(rendered.text().len() <= run::HostAnswer::CAPACITY);
    }
}

mod docs;

#[test]
fn flat_forge_reads_decode_the_repository_name_and_name_bad_members() {
    assert_eq!(
        decided(b"read_branch", br#"{"branch":"main"}"#).unwrap().tool,
        engine::Tool::ReadNamed {
            repository: None,
            read: skein_forge::api::Read::Branch { branch: b"main".as_slice().into() }
        }
    );
    assert_eq!(
        decided(b"read_pull_for", br#"{"repository":"qa/pilot","head":"temper/1","base":"main"}"#).unwrap().tool,
        engine::Tool::ReadNamed {
            repository: Some(b"qa/pilot".as_slice().into()),
            read: skein_forge::api::Read::PullFor {
                head: b"temper/1".as_slice().into(),
                base: b"main".as_slice().into()
            }
        }
    );
    for (tool, input, problem) in [
        (b"read_branch".as_slice(), b"{}".as_slice(), Problem::Missing(b"branch")),
        (b"read_branch", br#"{"branch":1}"#, Problem::Type(b"branch")),
        (b"read_reviews", br#"{"number":1,"page":4294967296}"#, Problem::Range(b"page")),
        (b"read_reviews", br#"{"number":1,"page":0}"#, Problem::Range(b"page")),
        (b"read_checks", br#"{"commit":"bad"}"#, Problem::Range(b"commit")),
    ] {
        assert_eq!(decided(tool, input), Err(problem));
    }
    assert_eq!(
        decided(b"read_items", b"{}").unwrap().tool,
        engine::Tool::ReadNamed {
            repository: None,
            read: skein_forge::api::Read::Items { since: skein_lib::Time::ZERO, page: 1, kind: None }
        }
    );
    let engine::Tool::ReadNamed { read: skein_forge::api::Read::Checks { commit }, .. } =
        decided(b"read_checks", br#"{"commit":"abcdef0123456789abcdef0123456789abcdef01"}"#).unwrap().tool
    else {
        panic!("commit read");
    };
    assert_eq!(commit[20..], [0; 12]);
}

#[test]
fn every_forge_read_declares_its_flat_members_as_json() {
    let members: &[(&[u8], &[&[u8]])] = &[
        (b"read_items", &[b"since", b"page", b"item_kind"]),
        (b"read_item", &[b"number", b"after"]),
        (b"read_pull", &[b"number"]),
        (b"read_pull_for", &[b"head", b"base"]),
        (b"read_reviews", &[b"number", b"page"]),
        (b"read_statuses", &[b"commit", b"page"]),
        (b"read_remarks", &[b"number", b"review", b"page"]),
        (b"read_branch", &[b"branch"]),
        (b"read_branches", &[b"prefix"]),
        (b"read_pull_files", &[b"number", b"head", b"page"]),
        (b"read_compare", &[b"before", b"after"]),
        (b"read_checks", &[b"commit"]),
        (b"read_file", &[b"head", b"path", b"max_bytes"]),
        (b"read_job", &[b"head", b"run", b"job", b"attempt", b"max_bytes"]),
        (b"read_protection", &[b"branch"]),
        (b"read_settings", &[]),
        (b"read_collaborators", &[b"page"]),
        (b"read_permission", &[b"user"]),
    ];
    let mut count = 0;
    for tool in tools(Duration::from_secs(1)) {
        let mut names = None;
        for (name, fields) in members {
            if *name == tool.name.as_ref() {
                names = Some(fields);
                break;
            }
        }
        let Some(names) = names else {
            continue;
        };
        count += 1;
        let schema = crate::json::parse(&tool.schema).expect("JSON schema");
        assert!(!schema.required(b"additionalProperties").unwrap().boolean(b"additionalProperties").unwrap());
        let properties = schema.required(b"properties").unwrap();
        let crate::json::Value::Object(fields) = properties else {
            panic!("properties object");
        };
        assert_eq!(fields.len(), names.len() + 1);
        assert_eq!(
            properties.required(b"repository").unwrap().required(b"type").unwrap().text(b"type").unwrap().as_ref(),
            b"string"
        );
        for name in *names {
            let property = properties.required(name).unwrap();
            assert!(!property.required(b"description").unwrap().text(b"description").unwrap().is_empty());
            assert!(property.field(b"type").unwrap().is_some());
        }
    }
    assert_eq!(count, members.len());
}

#[test]
fn job_and_attempt_zero_decode_but_run_and_explicit_byte_limit_remain_positive() {
    for attempt_number in [0, 1] {
        let input = format!(
            r#"{{"head":"0000000000000000000000000000000000000000","run":1,"job":0,"attempt":{attempt_number}}}"#
        );
        let call = decided(b"read_job", input.as_bytes()).expect("zero job or attempt");
        let engine::Tool::ReadNamed { read: skein_forge::api::Read::Job { attempt, max_bytes }, .. } = call.tool else {
            panic!("decoded job read")
        };
        assert_eq!((attempt.job, attempt.attempt, max_bytes), (0, attempt_number, 0));
    }
    for (input, field) in [
        (
            br#"{"head":"0000000000000000000000000000000000000000","run":0,"job":0,"attempt":0}"#.as_slice(),
            b"run".as_slice(),
        ),
        (
            br#"{"head":"0000000000000000000000000000000000000000","run":1,"job":0,"attempt":4294967296}"#.as_slice(),
            b"attempt".as_slice(),
        ),
        (
            br#"{"head":"0000000000000000000000000000000000000000","run":1,"job":0,"attempt":0,"max_bytes":0}"#
                .as_slice(),
            b"max_bytes".as_slice(),
        ),
    ] {
        assert_eq!(decided(b"read_job", input).err(), Some(Problem::Range(field)));
    }
    let tools = tools(Duration::from_secs(10));
    let mut schema = None;
    for tool in &tools {
        if tool.name.as_ref() == b"read_job" {
            schema = Some(&tool.schema);
        }
    }
    let schema = schema.expect("read_job tool");
    for property in [
        br#""job":{"type":"integer","description":"a CI job id, which may be 0 on Forgejo","minimum":0"#.as_slice(),
        br#""attempt":{"type":"integer","description":"a CI attempt number, which may be 0 on Forgejo","minimum":0"#
            .as_slice(),
    ] {
        assert!(skein_lib::bytes::find(schema, property).is_some());
    }
}

#[test]
fn goal_decodes_words_and_an_optional_advisor() {
    for (input, expected) in [
        (br#"{"words":"plan"}"#.as_slice(), false),
        (br#"{"words":"plan","advisor":true}"#, true),
        (br#"{"words":"plan","advisor":false}"#, false),
    ] {
        let call = decided(b"goal", input).expect("goal declared and decoded");
        assert_eq!(call.tool, engine::Tool::Goal { words: Box::from(b"plan".as_slice()), advisor: expected });
    }
    assert_eq!(decided(b"goal", b"{}"), Err(Problem::Missing(b"words")));
    assert_eq!(decided(b"goal", br#"{"words":""}"#), Err(Problem::Range(b"words")));
    assert_eq!(decided(b"goal", br#"{"words":"plan","advisor":1}"#), Err(Problem::Type(b"advisor")));
}
