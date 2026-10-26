//! The protocol layers between the engine root and the worker root, as the
//! composed world plays them (testing-strategy.md, 4). jig's link records
//! cross as they are, both roots speaking the hub's vocabulary; what differs
//! is translated here by small total functions: an assignment, which the
//! engine gives typed and the hub takes with smith's charter encoded and
//! temper's workspace items beside it; an answer's parts, which the worker
//! gives in its own detail and the engine takes typed (jig's
//! `protocol/channel.md`, 6.2); and the engine's hand-outs of host calls and
//! results, decoded and rendered with smith's codecs and temper's charter
//! translation.

use jig_domain_accounts as accounts;
use jig_domain_core as core;
use jig_domain_fleet as fleet;
use jig_domain_host as host;
use jig_domain_tasks as tasks;
use skein_lib::{Decimal, Reader, Secret, Token};
use smith_domain_run as run;
use temper_engine_domain as engine;
use temper_protocol_worker_wire::v1;
use temper_service::{settings, translate};
use temper_worker_domain::wire;
use temper_worker_domain_world::protocol as worker_protocol;

pub const GIT_BASE: &[u8] = b"http://forge.test";

/// The hub's assignment beside the service's workspace and saved-work branch.
#[must_use]
pub fn assignment(assignment: &engine::Assignment) -> (host::Assignment, wire::Workspace, Option<Box<[u8]>>) {
    let forges = [settings::Forge {
        number: temper_engine_forge_world::REPO.forge,
        kind: settings::Kind::Forgejo,
        api: jig_protocol_config::Address::Host { name: b"forge.test".as_slice().into(), port: 80 },
        transport: settings::Transport::Plaintext,
        git: GIT_BASE.into(),
        git_program: None,
        web: None,
        app_read_only: false,
        api_token: Some(Secret::new(Box::new([]))),
        webhook_secret: None,
        user: 1,
        identity: 1,
    }];
    let items = translate::items(assignment, &forges, &temper_service::forge::tokens::Tokens::empty(), &v1::CEILINGS)
        .expect("world workspace fits the payload");
    let (repositories, save) = translate::workspace(items.as_ref());
    let workspace = items.map(|_| host::Workspace { workstream: assignment.workstream, items: Token::new(0) });
    let hub = host::Assignment {
        messages: messages(&assignment.messages),
        assignment: host::RunAssignment {
            identity: identity(assignment.task, assignment.conversation),
            live: assignment.live,
            run: Token::new(assignment.task),
            attempt: Token::new(assignment.attempt),
            workspace,
            save: save.is_some(),
            charter: charter(assignment),
            grants: grants(&assignment.grants),
        },
        turns: assignment.transcript.clone(),
        answered: answered(&assignment.settled),
    };
    (hub, repositories, save)
}

/// The worker's parts through the service's channel records, without bytes.
#[must_use]
pub fn parts(work: &wire::Work, preparation: Option<wire::Preparation>) -> engine::Parts {
    let (left, saved) = translate::work(work);
    let preparation = preparation.as_ref().and_then(translate::preparation);
    translate::parts(Some(left), saved, preparation)
}

/// A relayed call decoded into the engine's tool vocabulary: smith's call
/// name by smith's channel codec, its input by temper's charter translation.
#[must_use]
pub fn decode(call: &fleet::Call) -> engine::Call {
    let mut reader = Reader::new(&call.name);
    let name = smith_channel::CallName::decode(&smith_channel::CEILINGS, &mut reader).expect("smith's call name");
    assert!(reader.is_empty(), "one whole call name");
    let name =
        run::CallName { activation: name.activation(), completion: name.completion(), position: name.position() };
    let input = run::HostInput::attested(call.input.clone()).expect("bounded host input");
    let temper_engine_smith::Decoded::Call(body) =
        temper_engine_smith::call(name, &call.tool, &input).expect("the run calls the declared host schema")
    else {
        panic!("script tool answered at decode")
    };
    body
}

/// A decided call's answer rendered as smith takes it.
#[must_use]
pub fn render(answer: &engine::CallAnswer) -> core::SettledAnswer {
    let (body, error) = temper_engine_smith::answer(answer).into_parts();
    core::SettledAnswer::Host { error, body }
}

/// An ended answer's outcome read back as the engine's task ending: smith's
/// result codec, then temper's translation of an accepted result.
#[must_use]
pub fn end(outcome: &[u8]) -> tasks::End {
    let declared = smith_protocol_channel::decode_declared(outcome, &smith_charter::CEILINGS).expect("smith's result");
    let spent = run::Spend {
        turns: 0,
        input: None,
        output: None,
        cache_read: None,
        cache_write: None,
        reasoning: None,
        units: 0,
    };
    let answer = run::Answer::Accepted { outcome: declared, spent, turns: 0 };
    temper_engine_smith::result(answer, None).expect("a result the engine takes").end
}

fn identity(task: u64, conversation: u64) -> [u8; 16] {
    let mut identity = [0; 16];
    let (first, second) = identity.split_at_mut(8);
    first.copy_from_slice(&task.to_be_bytes());
    second.copy_from_slice(&conversation.to_be_bytes());
    identity
}

/// The engine's charter, smith's charter encoded: temper's translation, its
/// conversation resumed when the charter resumes and turns are kept.
fn charter(assignment: &engine::Assignment) -> Box<[u8]> {
    let charter = temper_engine_smith::charter(assignment);
    smith_protocol_channel::encode_charter(&charter, &smith_charter::CEILINGS, &worker_protocol::endpoints())
        .expect("smith encodes the engine's charter")
}

fn messages(words: &[tasks::Word]) -> Box<[host::StartMessage]> {
    let mut messages = Vec::with_capacity(words.len());
    for word in words {
        messages.push(host::StartMessage {
            name: Token::new(word.number),
            sender: sender(word.from),
            words: word.words.clone(),
            finish_requested: word.finish_requested(),
        });
    }
    messages.into_boxed_slice()
}

fn sender(from: tasks::Party) -> Box<[u8]> {
    match from {
        tasks::Party::Task(number) | tasks::Party::Person(number) => Box::from(Decimal::of(number).as_bytes()),
        tasks::Party::Deployment { .. } => Box::from(&b"deployment"[..]),
    }
}

fn grants(grants: &[accounts::Grant]) -> Box<[host::Grant]> {
    let mut translated = Vec::with_capacity(grants.len());
    for grant in grants {
        let form = match grant.form {
            accounts::Form::Credential(scheme) => host::Form::Credential(match scheme {
                accounts::Scheme::Bearer => host::Scheme::Bearer,
                accounts::Scheme::Key => host::Scheme::Key,
            }),
            accounts::Form::Application => host::Form::Application,
            accounts::Form::Program(name) => {
                host::Form::Program(host::SourceName::new(name.as_bytes()).expect("same source name grammar"))
            }
        };
        translated.push(host::Grant { form, account: grant.account, generation: grant.generation, valid: grant.valid });
    }
    translated.into_boxed_slice()
}

fn answered(settled: &[core::Settled]) -> Box<[host::AnsweredCall]> {
    let mut answered = Vec::with_capacity(settled.len());
    for core::Settled { key, call } in settled {
        let answer = match &call.answer {
            core::SettledAnswer::Host { error, body } => {
                host::SettledAnswer::Host { error: *error, body: body.clone() }
            }
            core::SettledAnswer::Delivery { outcome, evidence } => {
                host::SettledAnswer::Delivery { outcome: delivery(*outcome), evidence: evidence.clone() }
            }
        };
        answered.push(host::AnsweredCall {
            name: jig_protocol_charter::call_name(&jig_protocol_charter::CallKey {
                activation: key.attempt,
                completion: key.completion,
                position: key.position,
            }),
            tool: call.tool.clone(),
            answer,
        });
    }
    answered.into_boxed_slice()
}

/// As jig's protocol layer names a delivery's outcome: an invalid one was
/// refused (`testing/jig-test-service`).
const fn delivery(outcome: core::DeliveryOutcome) -> host::DeliveryOutcome {
    match outcome {
        core::DeliveryOutcome::Delivered => host::DeliveryOutcome::Delivered,
        core::DeliveryOutcome::Nothing => host::DeliveryOutcome::Nothing,
        core::DeliveryOutcome::Stale => host::DeliveryOutcome::Stale,
        core::DeliveryOutcome::Refused | core::DeliveryOutcome::Invalid => host::DeliveryOutcome::Refused,
        core::DeliveryOutcome::Failed => host::DeliveryOutcome::Failed,
    }
}
