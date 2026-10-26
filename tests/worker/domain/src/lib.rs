//! The complete worker: jig's hub, smith's process host and temper's
//! checkout, against scripted engine and agent peers, the protocol layer's
//! routing played with smith's codecs, and the fake forge and disk (jig's
//! `domain/hosts.md`, sections 4, 6.6, 6.8 and 11).
//! `whole` checks remote trees, delivery settlement, contact loss and the
//! hello's stop bound. `runtime` measures retained state and replays small
//! turn windows.

use skein_lib::Duration;
use temper_worker_domain::{Limits, agent, checkout, host};

pub mod evidence;
pub mod protocol;
pub mod runtime;
pub mod whole;

/// The most text in one of the scripted agent's turns.
pub const TURN_TEXT: usize = 24;

// smith's protocol/channel.md section 6 assigns Codex replay dialect one.
pub const DIALECT: u32 = 1;

/// The bytes of the largest turn the scripted agent says: encoded, as the
/// hub keeps it, and owned, as smith's host counts it.
#[must_use]
pub fn turn_bytes() -> (u64, u64) {
    let mut encoded = 0;
    let mut owned = 0;
    for (sequence, first) in [(1, true), (1_000, false)] {
        let turn = protocol::session_turn(sequence, u64::MAX, TURN_TEXT, first);
        owned = owned.max(turn.owned_bytes().expect("an owned turn"));
        let bytes = protocol::transcript_turn(sequence, TURN_TEXT, first);
        encoded = encoded.max(u64::try_from(bytes.len()).expect("a length"));
    }
    (encoded, owned)
}

/// The calm worker's limits: room for three runs of up to two repositories
/// each, a turn window of one, with the engine's charters, and events and
/// outcomes of a few hundred bytes.
#[must_use]
pub fn limits() -> Limits {
    let (encoded, owned) = turn_bytes();
    Limits {
        host: host::Limits {
            program_sources: 12,
            source_interval: skein_lib::Duration::from_secs(60),
            source_timeout: skein_lib::Duration::from_secs(10),
            accounts: 4,
            slots: 3,
            charter_bytes: 8_192,
            transcript_bytes: 0,
            delivery_evidence_bytes: 0,
            turn_bytes: encoded,
            outcome_bytes: 4_096,
            detail_bytes: 32,
            held: 2,
            inbox: 2,
            piece_bytes: 4096,
            event_bytes: 96,
            run_calls: 2,
            facts: 256,
            told: 16,
            turns: 1,
            turn_queue_bytes: encoded,
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
            facts: 256,
        },
        reply_bytes: whole::REPLY_BYTES,
        agent: agent::Limits {
            accounts: 4,
            directories: 8,
            name_bytes: 256,
            agents: 3,
            charter_bytes: 65_536,
            transcript_bytes: 65_536,
            answered_bytes: 4_096,
            turn_bytes: owned,
            turns: 1,
            unacknowledged_bytes: owned,
            conflicts: 0,
            path_bytes: 64,
            message_bytes: 128,
            messages: 2,
            calls: 2,
            call_bytes: 512,
            answer_bytes: agent::Delivered::worst_case(),
            fact_bytes: 32,
            outcome_bytes: 4_096,
            detail_bytes: 32,
            spawn_timeout: Duration::from_secs(1),
            no_progress: Duration::from_secs(10),
            silence: Duration::from_secs(5),
            long_span: Duration::from_secs(120),
            long_open: 4,
            wall_time: Duration::from_secs(900),
            cancel_grace: Duration::from_secs(5),
            exit_grace: Duration::from_secs(5),
            teardown: Duration::from_secs(1),
            kill_after: Duration::from_secs(2),
            facts: 256,
        },
        bounds: host::Bounds {
            program_endpoints: Box::new([]),
            models: Box::new([host::ModelBound { name: Box::from(&b"fake"[..]), window: 1_048_576, output: 512 }]),
            turns: 8,
            spend: 100,
            time: Duration::from_secs(60),
            waiting: Duration::from_secs(60),
            checks: Duration::from_secs(60),
            message_bytes: 96,
            inbox: 2,
            unacknowledged: 1,
        },
        stop_bound: STOP_BOUND,
    }
}

/// The stop bound the worker's configuration derives: past the link's
/// noticing and its grace, a push's deadline, then a save's.
pub const STOP_BOUND: Duration = Duration::from_secs(300);

/// The process observation used by cross-world delivery replay stories.
pub use temper_worker_domain::agent::SavedReply as SavedDeliveryReply;
