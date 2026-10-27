//! A root assignment as one Smith activation. The caller supplies Smith's
//! already decoded history and mounts; no encoding or file access occurs here.

use alloc::boxed::Box;
use jig_domain_accounts as accounts;
use jig_domain_brief as brief;
use jig_domain_tasks as tasks;
use skein_lib::{Decimal, List, ReplyTo, Token, bytes};
use smith_domain as smith;
use smith_domain_run as run;
use temper_engine_domain as engine;

/// Translate one model with the source grant selected for this run.
#[must_use]
pub fn model(model: &engine::Model, grant: Option<u32>) -> run::charter::Llm {
    run::charter::Llm {
        effort: model.effort.clone(),
        prices: run::Prices {
            input: model.input_price,
            cached: model.cached_price,
            cache_write: model.cache_write_price,
            output: model.output_price,
            unit: model.price_unit,
        },
        dialect: model.dialect,
        account: grant,
        endpoint: run::charter::Endpoint(model.endpoint),
        model: model.name.clone(),
        window: model.window,
        output: model.output,
    }
}

/// Render one already bounded root brief section with its kind as a title.
#[must_use]
pub fn section(section: &engine::BriefSection) -> run::Section {
    let title: &[u8] = match section.kind {
        engine::BriefKind::Core(brief::Core::Task) => b"Task",
        engine::BriefKind::Core(brief::Core::Lineage) => b"Lineage",
        engine::BriefKind::Core(brief::Core::Results) => b"Dependencies",
        engine::BriefKind::Core(brief::Core::Plan) => b"Plan",
        engine::BriefKind::Core(brief::Core::Attempts) => b"Earlier attempts",
        engine::BriefKind::Core(brief::Core::Calls) => b"Calls",
        engine::BriefKind::Core(brief::Core::Waiting) => b"Waiting",
        engine::BriefKind::Core(brief::Core::NotesIndex) => b"Notes",
        engine::BriefKind::Core(brief::Core::TranscriptTail) => b"Transcript tail",
        engine::BriefKind::Forge(engine::ForgeBriefKind::Home) => b"Home",
        engine::BriefKind::Forge(engine::ForgeBriefKind::Ci) => b"CI",
        engine::BriefKind::Forge(engine::ForgeBriefKind::Reviews) => b"Reviews",
        engine::BriefKind::Forge(engine::ForgeBriefKind::Pull) => b"Pull request",
    };
    let text = match &section.body {
        engine::BriefBody::Text(text) => text.clone(),
        engine::BriefBody::Missing(reason) => match reason {
            brief::GatherMissing::Failed => bytes::copy_of(b"[read failed]"),
            brief::GatherMissing::Late => bytes::copy_of(b"[read timed out]"),
            brief::GatherMissing::Budget => bytes::copy_of(b"[read exceeded limit]"),
        },
    };
    run::Section { title: bytes::copy_of(title), text }
}

/// Produce Smith's complete typed start after the root's claim is durable.
/// The caller owns the live reply right, prepared workspace and concrete
/// history; the engine deliberately retains its stored transcript as bytes.
/// This is temper's own charter translation (domain/agent.md, 4.1).
#[must_use]
pub fn start(
    assignment: &engine::Assignment,
    workspace: Option<run::Workspace>,
    transcript: Option<smith::Transcript>,
    reply_to: ReplyTo,
    host_run: Token,
    window: smith::Window,
) -> smith::Event {
    let mut charter = charter(assignment);
    charter.resume = assignment.run.policy.resume && transcript.is_some();
    let mut messages = List::with_capacity(assignment.messages.len().try_into().expect("bounded start messages"));
    for word in &assignment.messages {
        messages
            .push(run::Message {
                name: Token::new(word.number),
                label: engine::inbox_sender(word),
                text: engine::inbox_words(word),
                finish_requested: word.finish_requested(),
            })
            .expect("bounded start messages");
    }
    let mut grants = List::with_capacity(assignment.grants.len().try_into().expect("bounded grants"));
    for grant in &assignment.grants {
        // Application grants are the worker's own, for git (domain/worker.md, 4.1).
        match grant.form.table() {
            accounts::GrantTable::Accounts | accounts::GrantTable::Name => grants
                .push(smith::Grant {
                    renewable: match grant.form {
                        accounts::Form::Credential(accounts::Scheme::Bearer) => true,
                        accounts::Form::Credential(accounts::Scheme::Key)
                        | accounts::Form::Application
                        | accounts::Form::Program(_) => false,
                    },
                    name: smith::GrantName { account: grant.account, generation: grant.generation },
                    valid: grant.valid,
                })
                .expect("bounded grants"),
            accounts::GrantTable::Application => {}
        }
    }
    smith::Event::Start {
        reply_to,
        host_run,
        activation: assignment.attempt,
        identity: None,
        window,
        messages: messages.into_boxed(),
        charter,
        workspace,
        transcript,
        // The host will supply recovered answers when it keeps their durable
        // records; this typed system route has none to pass yet.
        answered: Box::new([]),
        grants: grants.into_boxed(),
    }
}

/// The channel charter takes its mode from the task's ending and resumes
/// only when policy allows and turns are kept (domain/agent.md, 4.6).
#[must_use]
pub fn charter(assignment: &engine::Assignment) -> run::Charter {
    let mut sections = List::with_capacity(assignment.sections.len().try_into().expect("bounded brief sections"));
    for item in &assignment.sections {
        sections.push(section(item)).expect("bounded brief sections");
    }
    let policy = &assignment.run.policy;
    let mut models = List::with_capacity(policy.alternatives.len().try_into().expect("bounded model list"));
    for (index, candidate) in policy.alternatives.iter().enumerate() {
        let grant = *assignment
            .run
            .sources
            .get(index.checked_add(1).expect("bounded model index"))
            .expect("retained alternative source binding");
        models.push(model(candidate, grant)).expect("bounded model list");
    }
    let outcome = contract(&assignment.run.contract);
    let mode = match assignment.run.ended_by {
        tasks::EndedBy::Result => run::Mode::Finite,
        tasks::EndedBy::Requester => run::Mode::Open,
    };
    run::Charter {
        mode,
        resume: policy.resume && !assignment.transcript.is_empty(),
        waiting: policy.waiting,
        instructions: policy.instructions.clone(),
        brief: run::Brief { sections: sections.into_boxed() },
        conventions: Some(run::Conventions {
            guide: bytes::copy_of(b"AGENTS.md"),
            checks: bytes::copy_of(b".temper/pre-pr"),
        }),
        grants: run::charter::Grants {
            wait: true,
            deliver: match mode {
                run::Mode::Finite => outcome.change.clone(),
                run::Mode::Open => None,
            },
            tools: run::charter::Tools { inspect: policy.inspect, modify: policy.modify, shell: policy.shell },
            agents: policy.agents,
            host_tools: crate::calls::tools_for(
                policy.call_timeout,
                tasks::Tools(policy.tools.0),
                assignment.run.authority.tools,
            ),
        },
        outcome,
        budget: run::Budget { turns: policy.turns, spend: assignment.run.budget, time: policy.time },
        reserve: run::charter::Reserve { turns: policy.reserve_turns, time: policy.reserve_time },
        llm: model(&policy.model, *assignment.run.sources.first().expect("main source binding")),
        models: models.into_boxed(),
    }
}

fn contract(contract: &tasks::Contract) -> run::outcome::OutcomeSpec {
    match contract {
        tasks::Contract::Report { words } => run::outcome::OutcomeSpec {
            change: None,
            verdicts: Box::new([]),
            report: Some(run::outcome::TextSpec { max: *words, fields: Box::new([]) }),
            failure: Some(run::outcome::TextSpec { max: *words, fields: Box::new([]) }),
        },
        tasks::Contract::Verdict { choices } => {
            let mut verdicts = List::with_capacity(choices.len().try_into().expect("bounded verdict list"));
            for choice in choices {
                let mut fields = List::with_capacity(choice.fields.len().try_into().expect("bounded verdict fields"));
                for field in &choice.fields {
                    fields
                        .push(run::outcome::FieldRule { name: field.name.clone(), max: field.max })
                        .expect("bounded verdict fields");
                }
                verdicts
                    .push(run::outcome::VerdictRule {
                        name: bytes::copy_of(Decimal::of(u64::from(choice.code)).as_bytes()),
                        text_max: choice.words,
                        fields: fields.into_boxed(),
                        items: run::outcome::ItemSpec { min: 0, max: 0, kinds: Box::new([]) },
                    })
                    .expect("bounded verdict list");
            }
            run::outcome::OutcomeSpec {
                change: None,
                verdicts: verdicts.into_boxed(),
                report: None,
                failure: Some(run::outcome::TextSpec { max: 1024, fields: Box::new([]) }),
            }
        }
        tasks::Contract::Change { words, .. } => run::outcome::OutcomeSpec {
            change: Some(run::outcome::ChangeSpec {
                checks_must_pass: true,
                fields: Box::new([
                    run::outcome::FieldRule { name: bytes::copy_of(b"title"), max: *words },
                    run::outcome::FieldRule { name: bytes::copy_of(b"body"), max: *words },
                ]),
            }),
            verdicts: Box::new([]),
            report: None,
            failure: Some(run::outcome::TextSpec { max: *words, fields: Box::new([]) }),
        },
        // An approval is a person's; an agent given one may only fail.
        tasks::Contract::Approval { .. } => run::outcome::OutcomeSpec {
            change: None,
            verdicts: Box::new([]),
            report: None,
            failure: Some(run::outcome::TextSpec { max: 1024, fields: Box::new([]) }),
        },
    }
}
