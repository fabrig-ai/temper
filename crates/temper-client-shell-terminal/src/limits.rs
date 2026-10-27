//! Documented native-client constants and checked memory sums (client/README.md, 2).
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
/// Each exchange's deadline, including a quiet engine.
pub const DEADLINE: Duration = Duration::from_secs(30);
/// Field text and provisional conversation text (client/README.md, 5.4).
pub const CONVERSATION_TEXT: u32 = 512;
/// Shared text of one committed turn, including its calls (client/README.md, 5.4).
pub const TURN_TEXT: u32 = 2048;
/// Root, native transport and io bounds.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub client: client::Limits,
    pub connections: jig_client_shell_native::Limits,
    pub io: skein_io::Limits,
}
#[must_use]
#[expect(clippy::arithmetic_side_effects, reason = "the request count is the nonzero constant declared here")]
pub fn native() -> Limits {
    Limits {
        client: {
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
                    authority: 14,
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
        },
        connections: jig_client_shell_native::Limits {
            exchanges: EXCHANGES,
            deadline: DEADLINE,
            body_bytes: 33_554_432, // Whole small store and HTTP response bodies.
            http: skein_http::client::Limits {
                head: 4096,    // Incoming HTTP head bytes.
                headers: 16,   // Incoming HTTP fields.
                request: 4096, // Outgoing HTTP head bytes.
                read: 1024,    // Stream body chunk bytes.
                send: 1024,    // Outgoing body chunk bytes.
            },
        },
        io: skein_io::Limits {
            sockets: 16,                           // Stream and exchange sockets, with close overlap.
            names: 1,                              // No in-process listener, one spare name.
            refusals: 16,                          // Deferred io admission refusals.
            intake: 8192,                          // Per-socket unparsed bytes.
            receive: 4096,                         // One receive's bytes.
            output: 8192,                          // Per-socket queued bytes.
            sends: 8,                              // Queued sends per socket.
            accepts: 1,                            // io's minimum, this client never listens.
            backlog: 1,                            // Unused listener backlog.
            close_timeout: Duration::from_secs(5), // Graceful close bound.
            retry: Duration::from_millis(10),      // Kernel resource retry delay.
        },
    }
}
/// Checked words and the view bounds used at construction and accounting.
#[must_use]
pub fn view(limits: &Limits) -> Option<(temper_client_view::Limits, temper_client_view::words::Words)> {
    let words = temper_client_view::words::Words::english(
        temper_client_protocol_wire::FORGE,
        limits.client.client.domain.text,
        temper_client_view::words::Shell::Terminal,
    )
    .ok()?;
    let view = temper_client_view::Limits::of(&limits.client, &words)?;
    Some((view, words))
}
/// Sum the root, view, link and shell, with overflow refusing the preset.
#[must_use]
pub fn worst_case(limits: &Limits) -> Option<u64> {
    let (view, words) = view(limits)?;
    let root = client::worst_case(&limits.client)?;
    let view = temper_client_view::worst_case(&view, &limits.client, &words)?;
    root.checked_add(view)?.checked_add(shell(limits)?)
}
/// The published `Shell::worst_case` formula, with every operation checked.
#[must_use]
pub fn shell(limits: &Limits) -> Option<u64> {
    if limits.connections.exchanges != EXCHANGES {
        return None;
    }
    if limits.io.intake < limits.connections.http.head.max(limits.connections.http.read) {
        return None;
    }
    if limits.io.output < limits.connections.http.request.max(limits.connections.http.send) {
        return None;
    }
    let bytes = u64::from(limits.io.intake)
        .checked_add(u64::from(limits.io.output))?
        .checked_add(u64::from(limits.connections.body_bytes))?
        .checked_add(1024)?;
    // Shell shares Transport across every admitted io socket, including close overlap.
    let connections = u64::from(limits.io.sockets);
    let stream = jig_client_protocol_http::worst_case(&jig_client_protocol_http::Limits {
        record: limits.connections.body_bytes.checked_add(512)?,
        chunk: limits.connections.http.read,
    })?;
    let heard = limits.connections.http.read.checked_mul(3)?.checked_add(8)?;
    skein_io::worst_case(&limits.io)?
        .checked_add(stream)?
        .checked_add(skein_lib::Queue::<jig_client_protocol_http::Heard>::worst_case(heard)?)?
        .checked_add(connections.checked_mul(256)?.checked_mul(bytes)?)?
        .checked_add(connections.checked_mul(u64::from(limits.io.intake))?)?
        .checked_add(skein_lib::Queue::<skein_io::Request>::worst_case(limits.io.sockets.checked_mul(64)?)?)
}
/// The terminal's own fixed bounds, independent of configuration.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Terminal {
    pub line: u32,
    pub read: u32,
    pub interval: skein_lib::Duration,
    pub lines: u32,
    pub first: skein_lib::Duration,
    pub most: skein_lib::Duration,
}
#[must_use]
pub const fn terminal() -> Terminal {
    Terminal {
        line: 4096,
        read: 4096,
        interval: skein_lib::Duration::from_secs(1),
        lines: 32,
        first: skein_lib::Duration::from_millis(250),
        most: skein_lib::Duration::from_secs(8),
    }
}
