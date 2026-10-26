//! The same-origin visitor's bounds, independent of the client (playground.md, 4).
use crate::Story;
use skein_lib::Duration;
use skein_playground::visitor::{Limits, Settings, Visitor};

/// Eight domain exchanges and the browser's spare slot.
pub const VISITOR_EXCHANGES: u32 = 9;
/// The web client's largest response body (32 MiB).
pub const VISITOR_ANSWER: u32 = 33_554_432;
/// One bounded stream read.
pub const VISITOR_PIECE: u32 = 4096;
/// A turn's 1 MiB payload plus its path, headers and their containers.
pub const VISITOR_REQUEST: u32 = 1_064_960;
/// A batch of exchanges, stream operations and read demands between turns.
pub const VISITOR_DUTIES: u32 = 32;
/// The same-origin jar's entries.
pub const VISITOR_COOKIES: u32 = 8;
/// A cookie's name and value together.
pub const VISITOR_COOKIE: u32 = 4096;

/// The same-origin page client for a story, shared with its native replay.
#[must_use]
pub fn visitor(story: &Story) -> Visitor {
    let listener = story.clients();
    Visitor::new(Settings {
        listener,
        host: listener.to_string().into_bytes().into(),
        origin: format!("http://{listener}").into_bytes().into(),
        limits: Limits {
            io: skein_io::Limits {
                sockets: VISITOR_EXCHANGES + 1,
                names: 0,
                refusals: 4,
                intake: 8192,
                receive: 8192,
                output: 8192,
                sends: 4,
                accepts: 4,
                backlog: 8,
                close_timeout: Duration::from_millis(10),
                retry: Duration::from_millis(1),
            },
            http: skein_http::client::Limits { request: 8192, head: 8192, headers: 32, read: 4096, send: 8192 },
            exchanges: VISITOR_EXCHANGES,
            duties: VISITOR_DUTIES,
            request: VISITOR_REQUEST,
            answer: VISITOR_ANSWER,
            piece: VISITOR_PIECE,
            cookies: VISITOR_COOKIES,
            cookie: VISITOR_COOKIE,
        },
    })
}
