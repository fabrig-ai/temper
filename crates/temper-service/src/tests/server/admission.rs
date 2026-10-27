//! Hold the transport's page calls and closing sockets independently of the mapping.
use super::{LISTENER, OWNER, fixtures};
use crate::server::{Routed, Server};
use skein_http::{client, server as http};
use skein_io as io;
use skein_lib::{Env, Queue, Token, stream::Up};

struct Peer {
    socket: Token,
    owner: Token,
    call: Option<Token>,
    wire: skein_world::stream::Wire,
    closing: bool,
}
impl Peer {
    fn open(server: &mut Server, out: &mut Queue<Routed>, socket: Token, request: &[u8]) -> Self {
        server.up(io::Event::Accepted { owner: OWNER, socket, peer: fixtures::http::address() }, out);
        let Some(Routed::Io(io::Request::Bind { socket: bound, owner })) = out.pop() else {
            panic!("connection within the transport bound is admitted")
        };
        assert_eq!(bound, socket);
        let mut peer = Self { socket, owner, call: None, wire: skein_world::stream::Wire::new(7), closing: false };
        peer.wire.write(request);
        peer.pump(server, out, false);
        assert!(peer.call.is_some(), "the HTTP head reaches the mount");
        peer
    }
    fn pump(&mut self, server: &mut Server, out: &mut Queue<Routed>, until_close: bool) {
        for _ in 0_u32..256 {
            server.advance(out);
            while let Some(event) = out.pop() {
                match event {
                    Routed::Io(io::Request::Stream { stream, down }) => {
                        assert_eq!(stream, self.socket);
                        self.wire.take(down);
                    }
                    Routed::Io(io::Request::Close { entity }) => {
                        assert_eq!(entity, self.socket);
                        self.closing = true;
                        // Withhold Closed: the transport still owns this socket.
                    }
                    Routed::Call { call, .. } => {
                        assert!(self.call.replace(call).is_none());
                    }
                    Routed::Http { call, event: http::Event::Done(_) } => server.done(call, out),
                    Routed::Http { .. } => {}
                    Routed::Io(other) => panic!("unexpected transport request: {other:?}"),
                }
            }
            if self.closing || (!until_close && self.call.is_some()) {
                return;
            }
            if let Some(up) = self.wire.answer() {
                server.up(io::Event::Stream { owner: self.owner, up }, out);
            }
        }
        panic!("the queued HTTP transition settles");
    }
    fn answer(&mut self, server: &mut Server, out: &mut Queue<Routed>) {
        let call = self.call.unwrap();
        server.answer(call, http::Request::Discard, out);
        server.answer(
            call,
            http::Request::Respond(http::Response {
                status: 200,
                headers: Box::new([]),
                body: http::Body::None,
                close: false,
            }),
            out,
        );
        self.pump(server, out, true);
        assert!(self.wire.received.starts_with(b"HTTP/1.1 200"));
        assert!(self.closing);
    }
}

const DISCOVERY: &[u8] =
    b"GET /.well-known/openid-configuration HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
const PAGE_HEADS: [&[u8]; 4] = [
    b"GET /jig/v1/stream HTTP/1.1\r\nHost: localhost\r\n\r\n",
    super::HELD_READ_HEAD,
    super::HELD_READ_HEAD,
    super::HELD_READ_HEAD,
];

#[test]
fn page_calls_and_closing_requests_fill_the_transport_bound_and_the_next_socket_is_rejected() {
    let limits = crate::limits::server(&fixtures::engine().jig.limits);
    assert_eq!(limits.connections, 8, "the shipped transport bound");
    let mut server = Server::new(fixtures::http::address(), &limits);
    let mut out = Queue::with_capacity(16);
    assert!(server.advance(&mut out));
    let Some(Routed::Io(io::Request::Listen { owner, .. })) = out.pop() else { panic!("listener requested") };
    assert_eq!(owner, OWNER);
    server.up(io::Event::Listening { owner: OWNER, listener: LISTENER, addr: fixtures::http::address() }, &mut out);
    let mut peers = skein_lib::List::with_capacity(limits.connections);
    // Hold one stream and three read heads, as the page overlap fixture holds
    // its watches. Only transport ownership is under test, not authentication
    // or mapping admission.
    for head in PAGE_HEADS {
        let socket = Token::new(100 + u64::from(peers.len()));
        assert!(peers.push(Peer::open(&mut server, &mut out, socket, head)).is_ok());
    }
    // Fresh close connections have received their answers, but io has not
    // confirmed any close. Their HTTP calls are retired; their slots are not.
    for _ in 4..limits.connections {
        let socket = Token::new(100 + u64::from(peers.len()));
        let mut peer = Peer::open(&mut server, &mut out, socket, DISCOVERY);
        peer.answer(&mut server, &mut out);
        assert!(peers.push(peer).is_ok());
    }
    let mut closing = 0_u32;
    for peer in &peers {
        if peer.closing {
            closing += 1;
        }
    }
    assert_eq!(closing, 4);
    let next = Token::new(108);
    server.up(io::Event::Accepted { owner: OWNER, socket: next, peer: fixtures::http::address() }, &mut out);
    let Some(Routed::Io(io::Request::Reject { socket })) = out.pop() else {
        panic!("socket past the transport bound is rejected")
    };
    assert_eq!(socket, next);
    assert!(!server.advance(&mut out));
    assert!(out.is_empty(), "no bind, HTTP call, or response for the rejected socket");

    // An unanswered close is a client failure, not an HTTP status. The
    // browser harness maps this EOF (or a reset) to its synthetic status 0.
    let env = Env {
        now: skein_lib::Time::ZERO,
        wall: skein_lib::Wall::EPOCH,
        limits: client::Limits { request: 4096, head: 4096, headers: 32, read: 1, send: 4096 },
    };
    let mut client = client::Client::new(&env.limits);
    let mut above = Queue::with_capacity(16);
    let mut below = Queue::with_capacity(16);
    client::down(
        &mut client,
        &env,
        client::Request::Call(client::Call {
            method: skein_http::Method::Get,
            target: b"/.well-known/openid-configuration".as_slice().into(),
            headers: Box::new([skein_http::Header {
                name: b"Host".as_slice().into(),
                value: b"localhost".as_slice().into(),
            }]),
            body: client::Body::None,
            close: true,
        }),
        &mut above,
        &mut below,
    );
    client::up(&mut client, &env, Up::Room, &mut above, &mut below);
    assert!(above.is_empty(), "the client has sent its request and awaits a response");
    client::up(&mut client, &env, Up::End, &mut above, &mut below);
    assert_eq!(above.pop(), Some(client::Event::Failed(client::Error::Truncated { answered: false })));

    server.up(io::Event::Closed { owner: peers.get(4).unwrap().owner }, &mut out);
    let admitted = Peer::open(&mut server, &mut out, next, DISCOVERY);
    assert!(admitted.call.is_some(), "io Closed releases exactly the needed slot");
}
