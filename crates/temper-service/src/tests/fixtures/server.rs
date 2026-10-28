use super::http::Peer;
use crate::{Service, server::Server};
use skein_lib::{Time, Wall};
pub fn server() -> Server {
    let jig = jig_protocol_config::profile(jig_protocol_config::Profile::Small);
    Server::new(
        super::http::address(),
        &crate::limits::server(&jig, &crate::limits::profile(crate::limits::Profile::Small)),
    )
}
pub fn server_head(server: &mut Server, out: &mut skein_lib::Queue<crate::server::Routed>) -> skein_http::server::Call {
    server_call(server, out).1
}
pub fn server_call(
    server: &mut Server,
    out: &mut skein_lib::Queue<crate::server::Routed>,
) -> (skein_lib::Token, skein_http::server::Call) {
    let mut wire = skein_world::stream::Wire::new(7);
    wire.write(b"POST /jig/v1/sign-in/link HTTP/1.1\r\nHost: 127.0.0.1:7100\r\nContent-Length: 0\r\n\r\n");
    for _ in 0_u8..64 {
        server.advance(out);
        while let Some(request) = out.pop() {
            match request {
                crate::server::Routed::Io(skein_io::Request::Stream { down, .. }) => wire.take(down),
                crate::server::Routed::Call { call, head } => return (call, head),
                crate::server::Routed::Io(_) | crate::server::Routed::Http { .. } => {
                    panic!("HTTP call or stream request")
                }
            }
        }
        if let Some(up) = wire.answer() {
            server.up(skein_io::Event::Stream { owner: CONNECTION, up }, out);
        }
    }
    panic!("the accepted connection emits its call")
}
const CONNECTION: skein_lib::Token = skein_lib::Token::new(1);
pub fn listening(service: &mut Service) {
    let engine = service.engine.as_mut().expect("engine");
    engine.clients.iterate(Time::ZERO, Wall::EPOCH, &mut engine.events, &[], None, &mut engine.accounts);
    while let Some(request) = service.engine.as_mut().expect("engine").clients.io.pop() {
        if let skein_io::Request::Listen { owner, addr } = request {
            assert!(service.engine.as_mut().expect("engine").clients.up(skein_io::Event::Listening {
                owner,
                listener: LISTENER,
                addr
            }));
        }
    }
}
pub fn request(service: &mut Service, bytes: &[u8]) -> Peer {
    listening(service);
    Peer::raw(service.engine.as_mut().expect("engine"), bytes)
}
const LISTENER: skein_lib::Token = skein_lib::Token::new(99);
pub fn answered(service: &mut Service, peer: &mut Peer) -> (u16, Box<[skein_http::Header]>) {
    peer.pump(service.engine.as_mut().expect("engine"), false);
    let bytes = peer.received();
    let mut status = 0_u16;
    for byte in &bytes[9..12] {
        status = status.checked_mul(10).unwrap().checked_add(u16::from(byte.checked_sub(b'0').unwrap())).unwrap();
    }
    let mut headers = skein_lib::List::with_capacity(32);
    let mut start = skein_lib::bytes::find(bytes, b"\r\n").expect("status line").checked_add(2).unwrap();
    loop {
        let end = start.checked_add(skein_lib::bytes::find(&bytes[start..], b"\r\n").expect("header line")).unwrap();
        if start == end {
            break;
        }
        let line = &bytes[start..end];
        let at = skein_lib::bytes::find(line, b":").expect("header separator");
        let mut name = skein_lib::Writer::new(at);
        for byte in &line[..at] {
            name.put(&[byte.to_ascii_lowercase()]).expect("header name");
        }
        headers
            .push(skein_http::Header { name: name.finish(), value: Box::from(&line[at.checked_add(2).unwrap()..]) })
            .expect("bounded headers");
        start = end.checked_add(2).unwrap();
    }
    (status, headers.into_boxed())
}
pub fn header<'a>(headers: &'a [skein_http::Header], name: &[u8]) -> Option<&'a [u8]> {
    for header in headers {
        if &*header.name == name {
            return Some(&header.value);
        }
    }
    None
}

const RESPONSE_STATUS: u16 = 204;
pub fn server_response(server: &mut Server, call: skein_lib::Token, out: &mut skein_lib::Queue<crate::server::Routed>) {
    server.answer(
        call,
        skein_http::server::Request::Respond(skein_http::server::Response {
            status: RESPONSE_STATUS,
            headers: Box::new([]),
            body: skein_http::server::Body::None,
            close: true,
        }),
        out,
    );
}
