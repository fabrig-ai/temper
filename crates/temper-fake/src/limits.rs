//! The QA peers' fixed admission and buffer sizes (protocol/setup.md, 8.4).
use skein_fake_peers::Limits;
use skein_lib::Duration;

pub const CONNECTIONS: u32 = 8;
pub const QUEUE: u32 = 64;
pub const HEAD: u32 = 8192;
pub const READ: u32 = 4096;
pub const SEND: u32 = 4096;
pub const HEADERS: u32 = 32;
pub const BUFFER: u32 = 32_768;
pub const FORGE_BODY: u32 = 524_288;
pub const JSON_DEPTH: u32 = 32;
pub const MODEL_CEILING: u32 = 16_384;
pub const PATH_BYTES: u32 = 4096;
pub const MODEL_NAME_BYTES: u32 = 4096;
pub const CUE_BYTES: u32 = 256;
pub const CALL_NAME_BYTES: u32 = 64;
pub const JSON_NUMBER: u32 = 32;
pub const PACE_MS: u32 = 60_000;
pub const LLM_REQUEST_BYTES: u32 = 1 << 20;
pub const LLM_TOKENS: u32 = 65_536;
pub const LLM_TOOLS: u32 = 64;
pub const LLM_HISTORY: u32 = 1024;
pub const LLM_METADATA_BYTES: u32 = 8192;
pub const LLM_DETAIL_BYTES: u32 = 4096;
pub const FORGE_RAW_BYTES: u32 = 8192;
pub const FORGE_STRING_BYTES: u32 = 8192;
pub const FORGE_INNER: u32 = 64;
/// One complete drain, including worst-case JSON escaping and a checkpoint.
/// 128 observations each fit 6 times their content/name bounds plus framing.
pub const OBSERVATION_BYTES: u32 = 16 << 20;
/// Eight repository names and their 32-entry deletion queues, with 256-byte
/// names, plus the writer's inline state and one pending observation.
pub const OBSERVATION_STATE_BYTES: u32 = 1 << 20;

/// Eight requests beside deliveries, port-zero admission and no frozen peer records.
#[must_use]
pub fn peer() -> Limits {
    let mut io = jig_protocol_config::profile(jig_protocol_config::Profile::Small).io;
    io.sockets = CONNECTIONS.saturating_mul(2).saturating_add(1);
    io.refusals = CONNECTIONS;
    io.intake = BUFFER;
    io.receive = READ;
    io.output = BUFFER;
    io.sends = 2;
    io.accepts = CONNECTIONS;
    io.backlog = CONNECTIONS;
    io.close_timeout = Duration::from_millis(100);
    io.names = 0;
    io.retry = Duration::from_millis(1);
    Limits {
        io,
        connections: CONNECTIONS,
        queue: QUEUE,
        plaintext: BUFFER,
        ciphertext: BUFFER,
        observations: 0,
        observation_bytes: 0,
    }
}

/// Retained for the peer's life, as smith's tests/agent-process/src/fake.rs
/// peer used by the real loop (not a drain budget).
pub const LLM_OBSERVATIONS: u32 = 256;
pub const LLM_OBSERVATION_BYTES: u32 = 4_194_304;
