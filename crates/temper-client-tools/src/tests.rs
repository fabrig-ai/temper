mod boundary;
pub(crate) mod fixtures;
mod help;
mod jig;
mod json;
mod key;
mod limits;
mod subject;
mod table;

mod adopt;

mod forges;

mod forge_connection;

mod repositories;

mod change;

mod words;

mod landing;
mod printed;

#[test]
fn stream_refusals_name_the_bound_and_size_with_the_engine_refusal_status() {
    for (bound, words) in [
        (
            jig_client::StreamBound::Component,
            b"the engine's 2 event streams are in use (clients.streams): end a waiting ctl call, or wait\n".as_slice(),
        ),
        (
            jig_client::StreamBound::Transport,
            b"the engine's 2 event streams are in use (clients_http.streams): end a waiting ctl call, or wait\n"
                .as_slice(),
        ),
    ] {
        let printed = crate::jig::render::stream_full(jig_client::StreamFull { bound, size: 2 }, false);
        assert_eq!(printed.status, 2);
        assert!(printed.stdout.is_empty());
        assert_eq!(printed.stderr.as_ref(), words);
    }
}
