//! The tools' preset and host bounds (client/tools.md, 12).
pub use crate::jig::call::{KEY_BYTES, MOVED_BYTES};
use skein_lib::{Duration, RequestLimits};
use temper_client_domain as client;
pub const ROWS: u32 = 32;
pub const TEXT: u32 = 32_768;
/// jig's portable login-directory name and configured adapter-name bounds.
pub use crate::jig::call::{PROGRAM_ADAPTER, PROGRAM_NAME};
pub const PART: u32 = 16_384;
pub const DOCUMENT: u64 = 1_048_576;
/// A document's bytes can each become six JSON bytes (`\u00XX` in skein's
/// encoder). Reserve the same 1 KiB envelope as the me window for the printed
/// object's digest, continuation and result framing, beyond the document.
pub const RESULT_BYTES: u64 = DOCUMENT * 6 + 1024;
pub const DEADLINE: Duration = Duration::from_secs(30);
pub const LOCK: Duration = Duration::from_secs(60);
pub const LOCK_TRY: Duration = Duration::from_millis(100);
pub const WAIT: Duration = Duration::from_secs(300);
pub const WAIT_MOST: Duration = Duration::from_secs(3600);
/// The terminal's client preset, with the tools' rows and result text.
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
            authority: 14,
            heartbeat: Duration::from_secs(30),
            reads: 16,
            parts: 16,
            delegates: 8,
            document_bytes: DOCUMENT,
            records: 16,
            kept: 16,
            projects: 2,
            text: TEXT,
            turn_text: TEXT,
            rows: ROWS,
            fields: 16,
            segments: 9,
        },
        exchanges: 8,
    };

    client::Limits {
        forge: client::forge::Limits { field_bytes: 256, checks: 8 },
        client,
        // A page shows sixteen accounts per holder, eight sources/uses each, four live sign-ins.
        // These modest display counts are independent of task windows and document byte capacity.
        accounts: client::limits::AccountLimits { accounts: 16, sources: 8, uses: 8, sign_ins: 4 },
    }
}
/// Bounds consumed by the generic tools.
#[must_use]
pub fn tools() -> crate::jig::Limits {
    let preset = client().client;
    let client = preset.domain;
    crate::jig::Limits {
        rows: client.rows,
        settings: jig_client_protocol_link::Limits::new(client, preset.exchanges)
            .expect("checked host client limits")
            .wire,
        program_login: b"temper program-login",
        application: b"temper",
        ask_bytes: client.ask_bytes,
        result_bytes: RESULT_BYTES,
        text: TEXT,
        part: PART,
        wait: WAIT,
        wait_most: WAIT_MOST,
    }
}

#[must_use]
pub fn form() -> settings_form::Limits {
    crate::jig::form_limits(&tools())
}
