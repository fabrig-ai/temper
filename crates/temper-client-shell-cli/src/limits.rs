//! The shipped bounds for one call (client/tools.md, 12).
use skein_lib::Duration;
pub use temper_client_tools::limits::{DEADLINE, LOCK, LOCK_TRY};
pub const INTERRUPTED: u8 = 130;
pub const TERMINATED: u8 = 143;
/// A numeric HTTP socket address with its one final newline.
pub const ADDRESS: u32 = 64;
/// The printed URL, including a possible final newline.
pub const LINK: u32 = 256;
#[must_use]
pub fn settings(engine: skein_io::Addr, leaf: Box<[u8]>) -> jig_client_shell_native::Settings {
    let limits = native();
    jig_client_shell_native::Settings { engine, directory: leaf, limits: limits.connections, io: limits.io }
}
/// Concurrent HTTP exchanges, beside the event stream.
pub const EXCHANGES: u32 = 8;
/// The tools' root, native HTTP connections and socket io.
#[derive(Debug)]
pub struct Limits {
    pub root: temper_client_tools::root::Limits,
    pub connections: jig_client_shell_native::Limits,
    pub io: skein_io::Limits,
}
#[must_use]
pub fn native() -> Limits {
    Limits {
        root: temper_client_tools::root::Limits::new(
            temper_client_tools::limits::client(),
            temper_client_tools::limits::tools(),
        ),
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

/// Refuses unusable HTTP/io limits, overflowing stream counts and incompatible transport bounds.
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    if limits.connections.exchanges != EXCHANGES
        || limits.connections.body_bytes == 0
        || limits.connections.deadline == Duration::ZERO
        || limits.io.intake < limits.connections.http.head.max(limits.connections.http.read)
        || limits.io.output < limits.connections.http.request.max(limits.connections.http.send)
    {
        return false;
    }
    let Some(heard) = limits.connections.http.read.checked_mul(3) else { return false };
    let Some(record) = limits.connections.body_bytes.checked_add(512) else { return false };
    // skein's valid() replaces this at the set's repin
    skein_http::client::worst_case(&limits.connections.http).is_some()
        // jig's valid() replaces this at the set's repin
        && jig_client_protocol_http::worst_case(&jig_client_protocol_http::Limits {
            record, chunk: limits.connections.http.read,
        }).is_some()
        && heard.checked_add(8).is_some()
        && limits.io.is_usable()
        // skein's valid() replaces this at the set's repin
        && skein_io::worst_case(&limits.io).is_some()
}
