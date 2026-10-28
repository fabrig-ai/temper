//! The unit tests' values: the worker's limits, an engine's assignment and
//! workspace items, and what smith's agent says, typed.

use crate::{Limits, Request, agent, checkout, host, wire};
use alloc::boxed::Box;
use skein_lib::{Duration, List, Token};

pub const RUN: Token = Token::new(31);
pub const ATTEMPT: Token = Token::new(932);
pub const WORKSTREAM: u64 = 31;
pub const ACCOUNT: u32 = 1;
// A provider-supplied five-millisecond reset keeps the prior exhausted story.
pub const ACCOUNT_RESET: Duration = Duration::from_millis(5);

/// Two runs of up to two repositories each, two turns and two calls in flight
/// at a time, with smith's limits around the hub's.
pub fn limits() -> Limits {
    Limits {
        watermark: crate::WATERMARK,
        host: host::Limits {
            program_sources: 12,
            source_interval: Duration::from_secs(60),
            source_timeout: Duration::from_secs(10),
            slots: 2,
            accounts: 4,
            charter_bytes: 8_192,
            transcript_bytes: 1_024,
            delivery_evidence_bytes: 256,
            turn_bytes: 64,
            outcome_bytes: 4_096,
            detail_bytes: 32,
            held: 2,
            inbox: 2,
            piece_bytes: 4096,
            event_bytes: 96,
            run_calls: 2,
            facts: 64,
            told: 16,
            turns: 2,
            turn_queue_bytes: 128,
        },
        checkout: checkout::Limits {
            workspaces: 4,
            repositories: 2,
            name_bytes: 32,
            message_bytes: 512,
            conflicts: 0,
            path_bytes: 0,
            remote_timeout: Duration::from_secs(60),
            local_timeout: Duration::from_secs(10),
            facts: 64,
        },
        // This typed fixture has no smith channel; host text fits 512 bytes.
        reply_bytes: 512,
        agent: agent::Limits {
            agents: 2,
            directories: 8,
            conflicts: 0,
            path_bytes: 64,
            name_bytes: 64,
            accounts: 4,
            charter_bytes: 8_192,
            transcript_bytes: 1_024,
            answered_bytes: 4_096,
            message_bytes: 128,
            messages: 2,
            calls: 2,
            call_bytes: 512,
            answer_bytes: agent::Delivered::worst_case(),
            turns: 2,
            turn_bytes: 64,
            unacknowledged_bytes: 128,
            fact_bytes: 64,
            outcome_bytes: 4_096,
            detail_bytes: 32,
            spawn_timeout: Duration::from_secs(1),
            no_progress: Duration::from_secs(120),
            silence: Duration::from_secs(60),
            long_span: Duration::from_secs(60),
            long_open: 8,
            wall_time: Duration::from_secs(300),
            cancel_grace: Duration::from_secs(5),
            exit_grace: Duration::from_secs(5),
            teardown: Duration::from_secs(1),
            kill_after: Duration::from_secs(2),
            facts: 64,
        },
        bounds: host::Bounds {
            program_endpoints: Box::new([]),
            models: Box::new([]),
            turns: 8,
            spend: 100,
            time: Duration::from_secs(60),
            waiting: Duration::from_secs(60),
            checks: Duration::from_secs(60),
            message_bytes: 96,
            inbox: 2,
            unacknowledged: 2,
        },
        stop_bound: Duration::from_secs(20),
    }
}

/// jig's assignment of the run, in the workstream [`WORKSTREAM`] if it has
/// a workspace, with the grants `grants`.
pub fn assignment(workspace: bool, grants: Box<[host::Grant]>) -> host::Assignment {
    let workspace =
        if workspace { Some(host::Workspace { workstream: WORKSTREAM, items: Token::new(0) }) } else { None };
    host::Assignment {
        messages: Box::new([]),
        turns: Box::new([]),
        answered: Box::new([]),
        assignment: host::RunAssignment {
            live: host::LiveAsk::NONE,
            identity: [3; 16],
            run: RUN,
            attempt: ATTEMPT,
            workspace,
            save: false,
            charter: Box::from(&b"charter"[..]),
            grants,
        },
    }
}

/// Read-only repositories named `names`, each starting from `main`.
pub fn items(names: &[&[u8]]) -> wire::Workspace {
    let mut repositories = List::with_capacity(u32::try_from(names.len()).expect("a few names"));
    for (tag, name) in names.iter().enumerate() {
        let repository = wire::Repository {
            git: None,
            tag: u64::try_from(tag).expect("a few names"),
            name: Box::from(*name),
            remote: Box::from(&b"org/app"[..]),
            start: wire::Start::Base { branch: Box::from(&b"main"[..]) },
            access: wire::Access::ReadOnly,
            identity: 7,
            account: None,
        };
        repositories.push(repository).expect("room for each name");
    }
    wire::Workspace { repositories: repositories.into_boxed() }
}

/// The root's refusal of the run as an invalid start, for `why`.
pub fn refused(why: &[u8]) -> Request {
    let refusal = host::Refusal::Invalid(host::Invalid::Start { cause: host::StartCause::Other, why: Box::from(why) });
    Request::Answer {
        run: RUN,
        attempt: ATTEMPT,
        answer: host::Answer {
            read: None,
            refused: Box::new([]),
            turns: 0,
            spent: 0,
            ending: host::Ending::Refused(refusal),
        },
        work: wire::Work { landed: Box::new([]), saved: None },
        preparation: None,
    }
}

/// A grant of `account` in `table`.
pub fn grant(table: host::GrantTable, account: u32) -> host::Grant {
    host::Grant {
        form: match table {
            host::GrantTable::Accounts => host::Form::Credential(host::Scheme::Bearer),
            host::GrantTable::Application => host::Form::Application,
            host::GrantTable::Name => {
                host::Form::Program(host::SourceName::new(b"alice").expect("fixture source name"))
            }
        },
        account,
        generation: 1,
        valid: Duration::from_secs(60),
    }
}

/// smith's typed delivery fields, each a name and its value.
pub fn fields(named: &[(&[u8], &[u8])]) -> agent::Fields {
    let mut fields = List::with_capacity(u32::try_from(named.len()).expect("a few fields"));
    for (name, value) in named {
        let field = smith_domain_run::outcome::Field { name: Box::from(*name), value: Box::from(*value) };
        fields.push(field).expect("room for each field");
    }
    let change = smith_domain_run::outcome::Change { fields: fields.into_boxed() };
    agent::Fields::new(change, 512).expect("bounded fields")
}

/// A charter, as the protocol layer decodes one: smith's policy, which its
/// host domain carries without reading it.
pub fn charter() -> agent::Charter {
    use smith_domain_run::charter::{Endpoint, Grants, Llm, Reserve, Tools};
    let value = smith_domain_run::Charter {
        mode: smith_domain_run::Mode::Finite,
        resume: false,
        waiting: Duration::ZERO,
        instructions: Box::new([]),
        brief: smith_domain_run::Brief { sections: Box::new([]) },
        conventions: None,
        grants: Grants {
            wait: false,
            deliver: None,
            tools: Tools { inspect: false, modify: false, shell: false },
            agents: false,
            host_tools: Box::new([]),
        },
        outcome: smith_domain_run::outcome::OutcomeSpec {
            change: None,
            verdicts: Box::new([]),
            report: None,
            failure: None,
        },
        budget: smith_domain_run::Budget { turns: 1, spend: 1, time: Duration::from_secs(60) },
        reserve: Reserve { turns: 0, time: Duration::ZERO },
        llm: Llm {
            effort: None,
            prices: smith_domain_run::Prices { input: 1, cached: 1, cache_write: None, output: 1, unit: 1 },
            dialect: 0,
            account: Some(0),
            endpoint: Endpoint(0),
            model: Box::from(&b"model"[..]),
            window: 1_000,
            output: 100,
        },
        models: Box::new([]),
    };
    agent::Charter::new(value, 8_192).expect("a bounded charter")
}

/// The hub's start for the run `run`, decoded as the protocol layer would:
/// its charter, its inbox and its grants, no transcript.
pub fn start(run: Token, source: &host::Start) -> agent::Start {
    assert!(source.turns.is_empty() && source.answered.is_empty(), "a fresh conversation");
    let mut messages = List::with_capacity(u32::try_from(source.messages.len()).expect("a few messages"));
    for message in &source.messages {
        let message = agent::Message {
            name: message.name,
            label: message.sender.clone(),
            text: message.words.clone(),
            finish_requested: message.finish_requested,
        };
        messages.push(message).expect("room for each message");
    }
    let mut grants = List::with_capacity(u32::try_from(source.grants.len()).expect("a few grants"));
    for grant in &source.grants {
        let grant = crate::translate::grant(*grant).expect("account grant");
        grants.push(grant).expect("room for each grant");
    }
    agent::Start {
        identity: None,
        messages: messages.into_boxed(),
        logical_run: run,
        activation: source.activation,
        workspace: source.workspace,
        charter: charter(),
        transcript: None,
        answered: Box::new([]),
        directories: Box::new([]),
        grants: grants.into_boxed(),
        live: None,
    }
}

/// The turn `number` of the run, as smith's agent says it.
pub fn turn(number: u32) -> agent::Turn {
    use smith_domain_session::{llm, record};
    let body = record::Turn {
        model: Box::from(*b"model"),
        effort: None,
        version: record::VERSION,
        kind: record::Kind::Ordinary,
        endpoint: Some(llm::Endpoint(0)),
        dialect: 0,
        sequence: number,
        usage: llm::Usage::ZERO,
        jobs: 0,
        spent: u64::from(number),
        messages: Box::new([]),
    };
    let body = agent::TurnValue::new(body, 64).expect("a bounded turn");
    agent::Turn { opens_window: false, number, spent: u64::from(number), read: None, body }
}

/// The run's last word: parked, after `turns` turns.
pub fn parked(turns: u32) -> agent::Up {
    agent::Up::Answer {
        answer: agent::Answer { read: None, turns, spent: u64::from(turns), result: agent::RunResult::Parked },
    }
}

/// An assignment of the run `run` with the workspace items `names`, and
/// `messages` messages in its start's inbox.
pub fn assignment_of(run: u64, names: &[&[u8]], messages: u64) -> crate::Event {
    let mut assignment = assignment(true, Box::new([]));
    assignment.assignment.run = Token::new(run);
    let mut inbox = List::with_capacity(u32::try_from(messages).expect("a few messages"));
    for name in 1..=messages {
        let message = host::StartMessage {
            name: Token::new(name),
            sender: Box::from(&b"a"[..]),
            words: Box::from(&b"b"[..]),
            finish_requested: false,
        };
        inbox.push(message).expect("room for each message");
    }
    assignment.messages = inbox.into_boxed();
    crate::Event::Assign { assignment, items: Ok(items(names)), save: None }
}
