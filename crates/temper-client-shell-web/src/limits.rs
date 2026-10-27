//! The page's bounds, the terminal's client bounds (client/README.md, 2).
#![deny(
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::panic,
    clippy::wildcard_enum_match_arm
)]
use skein_lib::{Duration, RequestLimits};
use temper_client_domain as client;
/// Concurrent HTTP exchanges, beside the event stream.
pub const EXCHANGES: u32 = 8;
/// Whole small-store and HTTP response bodies.
pub const BODY_BYTES: u32 = 33_554_432;
/// Stream body chunk bytes.
pub const PIECE: u32 = 4096;
/// Field text and provisional conversation text (client/README.md, 5.4).
pub const CONVERSATION_TEXT: u32 = 512;
/// Shared text of one committed turn, including its calls (client/README.md, 5.4).
pub const TURN_TEXT: u32 = 2048;
/// The root's bounds: `temper-client-shell-terminal`'s `native().client`.
#[must_use]
#[expect(clippy::arithmetic_side_effects, reason = "the request count is the nonzero constant declared here")]
pub fn client() -> client::Limits {
    let requests = RequestLimits {
        requests: 16,                   // Durable keyed requests.
        bytes: 16_777_216,              // Whole request-table payload bytes.
        out: 32,                        // Table outputs per iteration.
        first: Duration::from_secs(1),  // First request retry span.
        most: Duration::from_secs(30),  // Longest request retry span.
        margin: Duration::from_secs(1), // Retention's safety margin.
    };
    let client = jig_client::Limits {
        domain: jig_client::DomainLimits {
            accounts: jig_client::AccountCounts { accounts: 16, sources: 8, sign_ins: 4 },
            owners: &[jig_client::Spoken {
                family: temper_client_protocol_wire::FORGE,
                version: temper_client_protocol_wire::CLIENT_VERSION,
            }],
            requests,
            // A jig ask reserves this much, so the table holds `requests` asks.
            ask_bytes: requests.bytes / u64::from(requests.requests),
            handles: 17,
            watches: 16, // Includes the shared me watch.
            turn_bytes: 1_048_576,
            backoff: jig_client::Backoff { first: Duration::from_secs(1), most: Duration::from_secs(30) },
            authority: 14, // Must hold the engine's complete stock authority list.
            heartbeat: Duration::from_secs(30),
            reads: 16,
            parts: 16,
            delegates: 8,
            document_bytes: 1_048_576,
            records: 16,
            kept: 16,
            projects: 2,
            text: CONVERSATION_TEXT,
            turn_text: TURN_TEXT,
            rows: 3,
            fields: 16,
            segments: 9,
        },
        exchanges: EXCHANGES,
    };
    client::Limits {
        forge: client::forge::Limits { field_bytes: 256, checks: 8 },
        client,
        // A page shows sixteen accounts per holder, eight sources/uses each, four live sign-ins.
        // These modest display counts are independent of task windows and document byte capacity.
        accounts: client::limits::AccountLimits { accounts: 16, sources: 8, uses: 8, sign_ins: 4 },
    }
}
/// Checked words and the view bounds used at construction and accounting.
#[must_use]
pub fn view(client: &client::Limits) -> Option<(temper_client_view::Limits, temper_client_view::words::Words)> {
    let words = temper_client_view::words::Words::english(
        temper_client_protocol_wire::FORGE,
        client.client.domain.text,
        temper_client_view::words::Shell::Browser,
    )
    .ok()?;
    let view = temper_client_view::Limits::of(client, &words)?;
    Some((view, words))
}
/// The event stream's record and chunk bounds.
#[must_use]
pub fn stream() -> Option<jig_client_protocol_http::Limits> {
    Some(jig_client_protocol_http::Limits { record: BODY_BYTES.checked_add(512)?, chunk: PIECE })
}
/// Sum the root, view, stream and the exchanges' bodies, with overflow refusing the bounds.
#[must_use]
pub fn worst_case(client: &client::Limits) -> Option<u64> {
    let mut total = 0_u64;
    for (_, bytes) in memory_terms(client)? {
        total = total.checked_add(bytes)?;
    }
    Some(total)
}
/// The page's root, view, stream decoder and concurrent exchange bodies.
#[must_use]
pub fn memory_terms(client: &client::Limits) -> Option<[(&'static str, u64); 4]> {
    let (bounds, words) = view(client)?;
    let root = client::worst_case(client)?;
    let view = temper_client_view::worst_case(&bounds, client, &words)?;
    let stream = jig_client_protocol_http::worst_case(&stream()?)?;
    let bodies = u64::from(EXCHANGES.checked_add(1)?).checked_mul(u64::from(BODY_BYTES))?;
    Some([("domain", root), ("view", view), ("http", stream), ("host", bodies)])
}
