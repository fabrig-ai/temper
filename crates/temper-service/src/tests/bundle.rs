use super::fixtures;
use crate::clients::bundle::{Answer, route};
use skein_http::Method;
use skein_lib::Writer;

const ETAG: Fields = &[(b"If-None-Match", b"\"0011223344556677\"")];
type Fields = &'static [(&'static [u8], &'static [u8])];
type Row = (bool, Method, &'static [u8], Fields, Answer);
const ROUTES: [Row; 13] = [
    (false, Method::Get, b"/", &[], Answer::NoBundle),
    (false, Method::Get, b"/jig/v1/sign-in/link?secret=AAAA", &[], Answer::NoBundle),
    (false, Method::Get, b"/bundle/x", &[], Answer::NotFound),
    (false, Method::Get, b"/robots.txt", &[], Answer::NotFound),
    (false, Method::Post, b"/", &[], Answer::NotFound),
    (true, Method::Post, b"/bundle/0123456789abcdef/boot.js", &[], Answer::NotAllowed),
    (true, Method::Get, b"/bundle/0123456789abcdef/boot.js", &[], Answer::File),
    (true, Method::Get, b"/bundle/fedcba9876543210/boot.js", &[], Answer::NotFound),
    (true, Method::Get, b"/bundle/0123456789abcdef/missing.js", &[], Answer::NotFound),
    (true, Method::Get, b"/robots.txt", &[], Answer::Root),
    (true, Method::Get, b"/jig/v1/sign-in/link?secret=AAAA", &[], Answer::Page),
    (true, Method::Get, b"/chats/7", &[], Answer::Page),
    (true, Method::Get, b"/chats/7", ETAG, Answer::NotModified),
];
#[test]
fn each_head_is_routed_in_the_designs_order() {
    let bundle = fixtures::bundle::bundle();
    for (bundled, method, target, headers, answer) in ROUTES {
        let head = fixtures::bundle::head(method, target, headers);
        assert_eq!(route(bundled.then_some(&bundle), &head), answer, "{method:?} {target:?}");
    }
}
#[test]
fn if_none_match_is_a_star_or_a_list_of_tags_compared_weakly() {
    let bundle = fixtures::bundle::bundle();
    let rows: [(&[u8], Answer); 12] = [
        (b"\"0011223344556677\"", Answer::NotModified),
        (b"*", Answer::NotModified),
        (b" * ", Answer::NotModified),
        (b"W/\"0011223344556677\"", Answer::NotModified),
        (b"\"stale\", \"0011223344556677\"", Answer::NotModified),
        (b"\"0011223344556677\",\"stale\"", Answer::NotModified),
        (b"\"stale\",\tW/\"0011223344556677\" ", Answer::NotModified),
        (b"\"stale\"", Answer::Page),
        (b"0011223344556677", Answer::Page),
        (b"\"0011223344556677", Answer::Page),
        (b"\"stale\" \"0011223344556677\"", Answer::Page),
        (b"", Answer::Page),
    ];
    for (value, answer) in rows {
        let head = fixtures::bundle::head(Method::Get, b"/chats/7", &[(b"If-None-Match", value)]);
        assert_eq!(route(Some(&bundle), &head), answer, "{value:?}");
    }
}
fn get(target: &[u8], field: &[u8]) -> (crate::Service, fixtures::http::Peer, u16, Box<[skein_http::Header]>) {
    let mut service = fixtures::started();
    service.adopt_bundle(fixtures::bundle::bundle());
    let parts =
        [b"GET ".as_slice(), target, b" HTTP/1.1\r\nHost: 127.0.0.1:7100\r\nConnection: close\r\n", field, b"\r\n"];
    let mut size = 0_usize;
    for part in parts {
        size = size.checked_add(part.len()).expect("fixture request size");
    }
    let mut request = Writer::new(size);
    for part in parts {
        request.put(part).expect("fixture request");
    }
    let request = request.finish();
    let mut peer = fixtures::request(&mut service, &request);
    let (status, headers) = fixtures::answered(&mut service, &mut peer);
    (service, peer, status, headers)
}
#[test]
fn a_bundle_file_is_served_immutable_with_its_kind() {
    let (_, peer, status, headers) = get(b"/bundle/0123456789abcdef/boot.js", b"");
    assert_eq!(status, 200);
    assert_eq!(fixtures::header(&headers, b"content-type"), Some(b"text/javascript".as_slice()));
    assert_eq!(fixtures::header(&headers, b"cache-control"), Some(b"public, max-age=31536000, immutable".as_slice()));
    assert_eq!(fixtures::bundle::body(&peer), b"boot();");
}
#[test]
fn the_page_is_served_with_its_etag_for_revalidation() {
    let (_, peer, status, headers) = get(b"/chats/7", b"");
    assert_eq!(status, 200);
    assert_eq!(fixtures::header(&headers, b"content-type"), Some(b"text/html; charset=utf-8".as_slice()));
    assert_eq!(fixtures::header(&headers, b"cache-control"), Some(b"no-cache".as_slice()));
    assert_eq!(fixtures::header(&headers, b"etag"), Some(b"\"0011223344556677\"".as_slice()));
    assert_eq!(fixtures::bundle::body(&peer), b"<!doctype html><title>t</title>");
}
#[test]
fn a_matching_etag_is_answered_not_modified() {
    let (_, peer, status, headers) = get(b"/chats/7", b"If-None-Match: \"0011223344556677\"\r\n");
    assert_eq!(status, 304);
    assert_eq!(fixtures::header(&headers, b"etag"), Some(b"\"0011223344556677\"".as_slice()));
    assert_eq!(fixtures::bundle::body(&peer), b"");
}
#[test]
fn a_file_longer_than_one_send_arrives_whole() {
    let (_, peer, status, headers) = get(b"/bundle/0123456789abcdef/module.wasm", b"");
    assert_eq!(status, 200);
    assert_eq!(fixtures::header(&headers, b"content-type"), Some(b"application/wasm".as_slice()));
    assert!(
        fixtures::bundle::WASM_BYTES
            > crate::limits::server(
                &jig_protocol_config::profile(jig_protocol_config::Profile::Small),
                &crate::limits::profile(crate::limits::Profile::Small)
            )
            .http
            .send
    );
    assert_eq!(fixtures::bundle::body(&peer), &*fixtures::bundle::wasm());
}
#[test]
fn the_printed_links_route_serves_the_page() {
    let (service, peer, status, _) = get(b"/jig/v1/sign-in/link?secret=AAAA", b"");
    assert_eq!(status, 200);
    assert_eq!(fixtures::bundle::body(&peer), b"<!doctype html><title>t</title>");
    assert_eq!(service.engine.as_ref().expect("engine").clients.early.len(), 0, "nothing reached the root");
}

#[test]
#[expect(
    clippy::disallowed_methods,
    clippy::disallowed_macros,
    reason = "ordinary tests format requests and inspect the fixed HTML answer as UTF-8 text"
)]
fn without_a_bundle_the_root_and_printed_link_serve_temper_with_the_page_policy() {
    for target in ["/", "/jig/v1/sign-in/link?secret=AAAA"] {
        let mut service = fixtures::started();
        let request = format!("GET {target} HTTP/1.1\r\nHost: 127.0.0.1:7100\r\nConnection: close\r\n\r\n");
        let mut peer = fixtures::request(&mut service, request.as_bytes());
        let (status, headers) = fixtures::answered(&mut service, &mut peer);
        assert_eq!(status, 200);
        assert_eq!(fixtures::header(&headers, b"content-type"), Some(b"text/html; charset=utf-8".as_slice()));
        for (name, value) in fixtures::bundle::POLICY {
            assert_eq!(fixtures::header(&headers, name), Some(value));
        }
        let body = core::str::from_utf8(fixtures::bundle::body(&peer)).unwrap();
        assert!(body.starts_with("<!doctype html>"));
        assert!(body.contains("lang=\"en\""));
        assert!(body.contains("<title>temper</title>"));
        assert!(body.contains("<h1>temper</h1>"));
        for paragraph in [
            "This temper was built without its browser client.",
            "Open the link in a terminal with temper open DIRECTORY LINK.",
            "To serve the browser, build temper with the wasm32-unknown-unknown target, or name a bundle's directory in application.clients.bundle, and restart temper.",
        ] {
            assert!(body.contains(&format!("<p>{paragraph}</p>")));
        }
        assert_eq!(body.matches("<p>").count(), 3);
        for tag in ["<script", "<style", "<img"] {
            assert!(!body.contains(tag));
        }
        assert_eq!(service.engine.as_ref().unwrap().clients.early.len(), 0);
    }
}
