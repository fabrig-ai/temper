use super::fixtures;
use skein_lib::{Queue, Time, Token};
const LISTENER: Token = Token::new(70);
const SOCKET: Token = Token::new(71);
const OWNER: Token = Token::new(0);
const CONNECTION: Token = Token::new(1);
const STALL_AT: Time = Time::from_nanos(30_000_000_000);

fn accepted() -> (crate::server::Server, Queue<crate::server::Routed>) {
    let mut server = fixtures::server();
    let mut out = Queue::with_capacity(16);
    assert!(server.advance(&mut out));
    let Some(crate::server::Routed::Io(skein_io::Request::Listen { owner, .. })) = out.pop() else {
        panic!("listener requested")
    };
    assert_eq!(owner, OWNER);
    server
        .up(skein_io::Event::Listening { owner: OWNER, listener: LISTENER, addr: fixtures::http::address() }, &mut out);
    server.up(skein_io::Event::Accepted { owner: OWNER, socket: SOCKET, peer: fixtures::http::address() }, &mut out);
    let Some(crate::server::Routed::Io(skein_io::Request::Bind { socket, .. })) = out.pop() else {
        panic!("socket bound")
    };
    assert_eq!(socket, SOCKET);
    assert_eq!(server.listening(), fixtures::http::address().socket());
    (server, out)
}
#[test]
fn an_accepted_connection_hands_its_call_to_the_mount() {
    let (mut server, mut out) = accepted();
    let head = fixtures::server_head(&mut server, &mut out);
    assert_eq!(head.method, skein_http::Method::Post);
    assert_eq!(&*head.target, b"/jig/v1/sign-in/link");
}
fn refused(bytes: &[u8], expected: u16) {
    let mut service = fixtures::started();
    let mut call = fixtures::request(&mut service, bytes);
    let (status, _) = fixtures::answered(&mut service, &mut call);
    assert_eq!(status, expected);
    assert_eq!(service.engine.as_ref().expect("engine").clients.early.len(), 0, "nothing reached the root");
}
#[test]
fn a_post_outside_jig_is_answered_not_found() {
    refused(b"POST /elsewhere HTTP/1.1\r\nHost: 127.0.0.1:7100\r\n\r\n", 404);
}
#[test]
fn a_get_of_the_printed_links_route_serves_temper_with_no_referrer() {
    let mut service = fixtures::started();
    let mut call = fixtures::request(
        &mut service,
        b"GET /jig/v1/sign-in/link?secret=AAAA HTTP/1.1\r\nHost: 127.0.0.1:7100\r\n\r\n",
    );
    let (status, headers) = fixtures::answered(&mut service, &mut call);
    assert_eq!(status, 200);
    assert_eq!(fixtures::header(&headers, b"referrer-policy"), Some(b"no-referrer".as_slice()));
    assert_eq!(service.engine.as_ref().expect("engine").clients.early.len(), 0, "nothing reached the root");
}
#[test]
fn a_host_off_the_list_is_answered_421_with_nothing_done() {
    refused(b"POST /jig/v1/read HTTP/1.1\r\nHost: evil.test:7100\r\nContent-Type: application/octet-stream\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", 421);
}
#[test]
fn a_post_not_octet_stream_is_answered_415_with_nothing_done() {
    refused(b"POST /jig/v1/read HTTP/1.1\r\nHost: 127.0.0.1:7100\r\nContent-Type: text/plain\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", 415);
}
#[test]
fn a_cookie_without_the_browser_header_is_answered_403_with_nothing_done() {
    refused(b"POST /jig/v1/read HTTP/1.1\r\nHost: 127.0.0.1:7100\r\nCookie: jig-sign-in=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\r\nContent-Type: application/octet-stream\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", 403);
}
#[test]
fn a_post_under_jig_reaches_the_component_as_its_message() {
    let mut service = fixtures::started();
    fixtures::listening(&mut service);
    let link = service.engine.as_mut().expect("engine").clients.links.pop().expect("printed link");
    let body = fixtures::http::link_body(service.engine.as_ref().expect("engine"), &link);
    let mut peer = fixtures::http::Peer::new(
        service.engine.as_mut().expect("engine"),
        b"POST",
        b"/jig/v1/sign-in/link",
        &body,
        None,
    );
    peer.pump(service.engine.as_mut().expect("engine"), false);
    assert_eq!(service.engine.as_ref().expect("engine").clients.early.len(), 1, "sign in reached the root");
}
#[test]
fn the_printed_link_names_the_bound_address_and_the_secret() {
    let secret = skein_lib::Secret::new(Box::new([0; 32]));
    assert_eq!(
        &*crate::clients::printed::link(fixtures::http::address().socket().unwrap(), &secret),
        b"http://127.0.0.1:7100/jig/v1/sign-in/link?secret=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
    );
}

#[test]
fn closing_ends_a_held_call_after_its_component_finishes() {
    let (mut server, mut out) = accepted();
    fixtures::server_head(&mut server, &mut out);
    while server.advance(&mut out) {
        while out.pop().is_some() {}
    }
    server.close(&mut out);
    while out.pop().is_some() {}
    // The component has closed and no response is queued for the held call.
    server.finish();
    while server.advance(&mut out) {}
    assert_eq!(server.next_deadline(), None);
    let Some(crate::server::Routed::Io(skein_io::Request::Close { entity })) = out.pop() else {
        panic!("held connection closed")
    };
    assert_eq!(entity, SOCKET);
}

#[test]
fn closing_sends_a_queued_response_before_ending_its_call() {
    let (mut server, mut out) = accepted();
    let (call, _) = fixtures::server_call(&mut server, &mut out);
    while server.advance(&mut out) {
        while out.pop().is_some() {}
    }
    fixtures::server_response(&mut server, call, &mut out);
    server.close(&mut out);
    server.finish();
    while out.pop().is_some() {}
    assert!(server.advance(&mut out));
    assert!(server.advance(&mut out));
    let Some(crate::server::Routed::Io(skein_io::Request::Stream { stream, .. })) = out.pop() else {
        panic!("queued response sent before closing")
    };
    assert_eq!(stream, SOCKET);
    while server.advance(&mut out) {}
    assert_eq!(server.next_deadline(), None);
}

#[test]
fn a_completed_closing_response_drains_while_the_server_serves() {
    let (mut server, mut out) = accepted();
    let (call, _) = fixtures::server_call(&mut server, &mut out);
    while server.advance(&mut out) {
        while out.pop().is_some() {}
    }
    server.answer(call, skein_http::server::Request::Discard, &mut out);
    fixtures::server_response(&mut server, call, &mut out);
    let mut completed = false;
    while server.advance(&mut out) {
        while let Some(event) = out.pop() {
            match event {
                crate::server::Routed::Io(skein_io::Request::Stream {
                    down: skein_lib::stream::Down::Demand { room, .. },
                    ..
                }) if room > 0 => {
                    server.up(skein_io::Event::Stream { owner: CONNECTION, up: skein_lib::stream::Up::Room }, &mut out);
                }
                crate::server::Routed::Http { event: skein_http::server::Event::Done(reuse), .. } => {
                    assert_eq!(reuse, skein_http::server::Reuse::Close);
                    completed = true;
                }
                crate::server::Routed::Io(_)
                | crate::server::Routed::Call { .. }
                | crate::server::Routed::Http { .. } => {}
            }
        }
    }
    assert!(completed, "response completed");
    server.done(call, &mut out);
    let Some(crate::server::Routed::Io(skein_io::Request::Close { entity })) = out.pop() else {
        panic!("completed response drains")
    };
    assert_eq!(entity, SOCKET);
}

#[test]
fn closing_waits_for_the_component_before_ending_its_call() {
    let (mut server, mut out) = accepted();
    fixtures::server_head(&mut server, &mut out);
    while server.advance(&mut out) {
        while out.pop().is_some() {}
    }
    server.close(&mut out);
    while out.pop().is_some() {}
    assert!(!server.advance(&mut out));
    assert_eq!(server.next_deadline(), Some(STALL_AT));
}

const ANSWERED: [(bool, &[u8]); 8] = [
    (true, b"GET /chats/7 HTTP/1.1\r\nHost: 127.0.0.1:7100\r\n\r\n"),
    (true, b"GET /bundle/0123456789abcdef/boot.js HTTP/1.1\r\nHost: 127.0.0.1:7100\r\n\r\n"),
    (true, b"GET /robots.txt HTTP/1.1\r\nHost: 127.0.0.1:7100\r\n\r\n"),
    (true, b"GET / HTTP/1.1\r\nHost: 127.0.0.1:7100\r\nIf-None-Match: \"0011223344556677\"\r\n\r\n"),
    (true, b"GET /bundle/fedcba9876543210/boot.js HTTP/1.1\r\nHost: 127.0.0.1:7100\r\n\r\n"),
    (true, b"DELETE /chats/7 HTTP/1.1\r\nHost: 127.0.0.1:7100\r\n\r\n"),
    (false, b"GET /jig/v1/sign-in/link?secret=AAAA HTTP/1.1\r\nHost: 127.0.0.1:7100\r\n\r\n"),
    (false, b"POST /jig/v1/read HTTP/1.1\r\nHost: evil.test:7100\r\nContent-Type: application/octet-stream\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"),
];
#[test]
fn each_answer_carries_the_policys_five_fields() {
    for (bundled, bytes) in ANSWERED {
        let mut service = fixtures::started();
        if bundled {
            service.adopt_bundle(fixtures::bundle::bundle());
        }
        let mut peer = fixtures::request(&mut service, bytes);
        let (_, headers) = fixtures::answered(&mut service, &mut peer);
        for (name, value) in fixtures::bundle::POLICY {
            assert_eq!(fixtures::header(&headers, name), Some(value), "{name:?} on {bytes:?}");
        }
    }
}
#[test]
fn the_policy_keeps_a_field_the_answer_already_holds() {
    let mut response = skein_http::server::Response {
        status: 200,
        headers: Box::new([skein_http::Header {
            name: Box::from(b"referrer-policy".as_slice()),
            value: Box::from(b"same-origin".as_slice()),
        }]),
        body: skein_http::server::Body::None,
        close: false,
    };
    crate::clients::policy::add(&mut response);
    let mut held = 0_u32;
    for header in &response.headers {
        if header.name.eq_ignore_ascii_case(b"Referrer-Policy") {
            assert_eq!(&*header.value, b"same-origin");
            held = held.checked_add(1).unwrap();
        }
    }
    assert_eq!(held, 1);
    assert_eq!(response.headers.len(), 5);
}

// A one-byte read body is deliberately withheld to retain each mounted call.
const HELD_READ_HEAD: &[u8] = b"POST /jig/v1/read HTTP/1.1\r\nHost: 127.0.0.1:7100\r\nContent-Type: application/octet-stream\r\nContent-Length: 1\r\n\r\n";
// Refuse an empty request too: it still needs Discard before its response.
const EMPTY_READ_HEAD: &[u8] = b"POST /jig/v1/read HTTP/1.1\r\nHost: 127.0.0.1:7100\r\nContent-Type: application/octet-stream\r\nContent-Length: 0\r\n\r\n";
const HELD_WATCH_HEAD: &[u8] = b"POST /jig/v1/watch HTTP/1.1\r\nHost: 127.0.0.1:7100\r\nContent-Type: application/octet-stream\r\nContent-Length: 1\r\n\r\n";
const PAGE_WATCHES: u32 = 3;
const PAGE_CALLS: u32 = 1 + PAGE_WATCHES;
const OVERLAPPING_PAGES: u32 = 2;

#[test]
fn a_pages_stream_and_watches_fit_while_the_old_pages_calls_are_still_open() {
    let mut service = fixtures::ready();
    let link = service.links().pop().unwrap();
    let engine = service.engine.as_mut().expect("engine");
    let body = fixtures::http::link_body(engine, &link);
    let mut sign_in = fixtures::http::Peer::new(engine, b"POST", b"/jig/v1/sign-in/link", &body, None);
    sign_in.pump(engine, true);
    let jig_client_protocol_wire::v19::SignInAnswer::SignedIn(answer) = sign_in.sign_in_answer(engine) else {
        panic!("signed in")
    };
    let bearer = fixtures::http::bearer(answer.token().as_ref().expect("native credential").expose());
    let mut streams = skein_lib::List::with_capacity(OVERLAPPING_PAGES);
    let mut watches = skein_lib::List::with_capacity(PAGE_WATCHES * OVERLAPPING_PAGES);
    for page in 0..OVERLAPPING_PAGES {
        let first = fixtures::http::SOCKET.raw() + 1 + u64::from(page * PAGE_CALLS);
        let mut stream =
            fixtures::http::Peer::new_on(engine, b"GET", b"/jig/v1/stream", &[], Some(&bearer), Token::new(first));
        assert!(stream.read_hello(engine), "each page opens its authenticated stream without Busy");
        assert!(streams.push(stream).is_ok());
        for watch in 0..PAGE_WATCHES {
            let socket = Token::new(first + 1 + u64::from(watch));
            let mut peer = fixtures::http::Peer::raw_on(engine, HELD_WATCH_HEAD, socket);
            // Delay the watch body: no call from either page can finish and free its slot.
            peer.hold_body(engine);
            assert!(peer.received().is_empty(), "the watch is admitted without a refusal");
            assert_eq!(peer.closed(), 0);
            assert!(watches.push(peer).is_ok());
        }
    }
    assert_eq!(streams.len() + watches.len(), PAGE_CALLS * OVERLAPPING_PAGES);
    assert_eq!(engine.clients.mapping_room_for_test(), 0, "all eight page calls remain mounted together");
    for index in 0..watches.len() {
        watches.get_mut(index).unwrap().close(engine);
    }
    for index in 0..streams.len() {
        streams.get_mut(index).unwrap().end(engine);
    }
}

#[test]
fn a_call_past_the_mounts_room_is_answered_busy() {
    let mut service = fixtures::started();
    let settings = fixtures::engine();
    let capacity = settings.jig.limits.clients_http.calls;
    // Give the HTTP fixture one extra connection to test mapping exhaustion independently.
    // The shipped server keeps its own eight-connection bound.
    let mut transport = crate::limits::server(&settings.jig.limits);
    transport.connections = capacity + 1;
    service.engine.as_mut().expect("engine").clients = crate::clients::Clients::new(
        &settings.jig,
        &settings.clients,
        &fixtures::seeds(),
        512,
        &settings.limits.root.journal,
        None,
        &[],
    )
    .unwrap()
    .with_server_for_test(crate::server::Server::new(fixtures::http::address(), &transport));
    fixtures::listening(&mut service);
    let mut peers = skein_lib::List::with_capacity(capacity);
    for number in 0..capacity {
        let socket = Token::new(fixtures::http::SOCKET.raw().checked_add(u64::from(number)).unwrap());
        let mut peer = fixtures::http::Peer::raw_on(service.engine.as_mut().expect("engine"), HELD_READ_HEAD, socket);
        peer.hold_body(service.engine.as_mut().expect("engine"));
        assert!(peers.push(peer).is_ok(), "one peer per mounted call");
    }
    let busy_socket = Token::new(fixtures::http::SOCKET.raw().checked_add(u64::from(capacity)).unwrap());
    let mut busy = fixtures::http::Peer::raw_on(service.engine.as_mut().expect("engine"), EMPTY_READ_HEAD, busy_socket);
    let (status, _) = fixtures::answered(&mut service, &mut busy);
    assert_eq!(status, 503);
    let body = skein_lib::bytes::find(busy.received(), b"\r\n\r\n").unwrap().checked_add(4).unwrap();
    assert_eq!(busy.received().get(body..), Some(b"".as_slice()));
    assert_eq!(service.engine.as_ref().expect("engine").clients.early.len(), 0, "nothing reached the root");
    // Only handling the HTTP terminal and retiring its call with server.done closes this socket.
    assert_eq!(busy.closed(), 1);
    peers.get_mut(0).unwrap().close(service.engine.as_mut().expect("engine"));
    let link = service.engine.as_mut().expect("engine").clients.links.pop().expect("printed link");
    let body = fixtures::http::link_body(service.engine.as_ref().expect("engine"), &link);
    let mut admitted = fixtures::http::Peer::new(
        service.engine.as_mut().expect("engine"),
        b"POST",
        b"/jig/v1/sign-in/link",
        &body,
        None,
    );
    admitted.handoff(service.engine.as_mut().expect("engine"));
    assert_eq!(
        service.engine.as_ref().expect("engine").clients.early.len(),
        1,
        "the released slot mounts another call"
    );
}
