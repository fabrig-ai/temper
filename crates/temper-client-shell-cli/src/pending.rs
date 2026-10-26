//! Inspect the native store with the same decoding checks as the client.
use jig_client::Record;
use jig_client_protocol_link::Limits;
use temper_client_tools::jig::{Printed, render};

pub(crate) fn print(records: Vec<Record>, json: bool) -> Printed {
    let native = crate::limits::native();
    let limits =
        Limits::new(native.root.domain.client.domain, native.root.domain.client.exchanges).expect("link limits");
    let mut rows = Vec::new();
    let mut unread = 0;
    for record in records {
        if record.key.as_ref() == jig_client::STREAM_KEY {
            continue;
        }
        if let Some(row) = temper_client_tools::recovery::restore(&record, &limits) {
            rows.push(row);
        } else {
            unread += 1;
        }
    }
    render::pending::pending(&rows, unread, json)
}
