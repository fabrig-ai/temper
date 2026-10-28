//! One bounded HTTP/TLS exchange per socket (protocol/sign-in.md, 6.3).
//! Adapted from skein-oauth-accounts' exchange.rs; no account driver is used.
#![expect(
    clippy::single_match,
    reason = "explicit socket presence at the transport boundary, as in skein-oauth-accounts"
)]
use alloc::boxed::Box;
use skein_http::{Header, Method, client as http};
use skein_io::{Event as IoEvent, Request as IoRequest};
use skein_lib::stream::{Down, Read, Up};
use skein_lib::{Duration, Env, List, Queue, Time, Token, bytes};
use skein_oauth as oauth;
use skein_tls::client as tls;
const ROUTES: u32 = 8;
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub body: u32,
    pub http: http::Limits,
    pub tls: tls::Limits,
}
#[derive(Debug)]
pub enum Transport {
    Plaintext,
    Tls { server_name: skein_tls::Name, trust: skein_tls::Config },
}
#[derive(Debug)]
pub struct Endpoint {
    pub origin: Box<[u8]>,
    pub address: core::net::SocketAddr,
    pub transport: Transport,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Connecting,
    Handshaking,
    Calling,
    Answered,
    Closing,
    Closed,
}

pub(crate) struct Web {
    pub(crate) deadline: Time,
    pub(crate) socket: Option<Token>,
    phase: Phase,
    tls: Option<tls::Client>,
    http: http::Client,
    events: Queue<http::Event>,
    requests: Queue<http::Request>,
    plain: Queue<Down>,
    tls_events: Queue<tls::Event>,
    cipher: Queue<Down>,
    upload: Box<[u8]>,
    offset: usize,
    document: List<u8>,
    response: Option<http::Response>,
    pub(crate) answer: Option<oauth::HttpResponse>,
    pub(crate) failure: Option<oauth::Failure>,
    close_sent: bool,
    id: u64,
    pub(crate) sent: bool,
    abort: bool,
}

impl Web {
    pub(crate) fn new(request: oauth::HttpRequest, endpoint: &Endpoint, limits: &Limits) -> Option<Web> {
        let method = match request.method {
            oauth::Method::Post => Method::Post,
            oauth::Method::Get => Method::Get,
        };
        let authority_start = bytes::find(&request.endpoint, b"://")?.checked_add(3)?;
        let rest = request.endpoint.get(authority_start..)?;
        let mut end = rest.len();
        for separator in [b"/".as_slice(), b"?"] {
            if let Some(at) = bytes::find(rest, separator) {
                end = end.min(at);
            }
        }
        let authority = rest.get(..end)?;
        let suffix = rest.get(end..)?;
        let target = if suffix.is_empty() {
            bytes::copy_of(b"/")
        } else if suffix.starts_with(b"?") {
            let mut target = skein_lib::Writer::new(suffix.len().checked_add(1)?);
            target.put(b"/").ok()?;
            target.put(suffix).ok()?;
            target.finish()
        } else {
            bytes::copy_of(suffix)
        };
        let mut headers = List::with_capacity(2);
        headers.push(Header { name: bytes::copy_of(b"host"), value: bytes::copy_of(authority) }).expect("two headers");
        if method == Method::Post {
            headers
                .push(Header { name: bytes::copy_of(b"content-type"), value: bytes::copy_of(request.content_type) })
                .expect("two headers");
        }
        let mut requests = Queue::with_capacity(ROUTES);
        requests.push(http::Request::Call(http::Call {
            method,
            target,
            headers: headers.into_boxed(),
            body: if method == Method::Post {
                http::Body::Length(u64::try_from(request.body.len()).ok()?)
            } else {
                http::Body::None
            },
            close: true,
        }));
        if method == Method::Post {
            requests.push(http::Request::Upload(Down::Demand { read: Read::Nothing, room: limits.http.send }));
        }
        let tls = match &endpoint.transport {
            Transport::Tls { server_name, trust } => Some(tls::Client::new(trust, server_name.clone(), &limits.tls)),
            Transport::Plaintext => None,
        };
        Some(Web {
            deadline: request.deadline,
            socket: None,
            phase: Phase::Connecting,
            tls,
            http: http::Client::new(&limits.http),
            events: Queue::with_capacity(ROUTES),
            requests,
            plain: Queue::with_capacity(ROUTES),
            tls_events: Queue::with_capacity(ROUTES),
            cipher: Queue::with_capacity(ROUTES),
            upload: request.body,
            offset: 0,
            document: List::with_capacity(limits.body),
            response: None,
            answer: None,
            failure: None,
            close_sent: false,
            id: request.id,
            sent: false,
            abort: false,
        })
    }

    pub(crate) fn up(&mut self, env: &Env<Limits>, event: IoEvent, io: &mut Queue<IoRequest>) {
        match event {
            IoEvent::Connecting { socket, .. } => {
                self.socket = Some(socket);
                if self.phase == Phase::Closing {
                    self.close_socket(io);
                }
            }
            IoEvent::Connected { .. } => {
                if self.phase == Phase::Connecting {
                    match &mut self.tls {
                        Some(client) => {
                            self.phase = Phase::Handshaking;
                            tls::down(
                                client,
                                &Env { now: env.now, wall: env.wall, limits: env.limits.tls },
                                tls::Request::Handshake,
                                &mut self.tls_events,
                                &mut self.cipher,
                            );
                        }
                        None => self.phase = Phase::Calling,
                    }
                }
            }
            IoEvent::Stream { up, .. } => {
                if self.phase == Phase::Closed {
                    return;
                }
                match &mut self.tls {
                    Some(client) => tls::up(
                        client,
                        &Env { now: env.now, wall: env.wall, limits: env.limits.tls },
                        up,
                        &mut self.tls_events,
                        &mut self.cipher,
                    ),
                    None if self.phase == Phase::Closing => {}
                    None => http::up(
                        &mut self.http,
                        &Env { now: env.now, wall: env.wall, limits: env.limits.http },
                        up,
                        &mut self.events,
                        &mut self.plain,
                    ),
                }
            }
            IoEvent::Failed { .. } => {
                self.transport_failed(env);
                // io owns failed-connect settlement and sends Closed itself.
            }
            IoEvent::Closed { .. } => {
                if self.phase != Phase::Answered && self.phase != Phase::Closing {
                    self.transport_failed(env);
                }
                self.phase = Phase::Closed;
                self.socket = None;
            }
            IoEvent::Listening { .. }
            | IoEvent::Accepted { .. }
            | IoEvent::Output { .. }
            | IoEvent::Spawned { .. }
            | IoEvent::Exited { .. }
            | IoEvent::Usage { .. }
            | IoEvent::Resized
            | IoEvent::Window { .. }
            | IoEvent::Shutdown { .. } => {}
        }
    }

    pub(crate) fn transport_failed(&mut self, env: &Env<Limits>) {
        if self.answer.is_none()
            && self.phase != Phase::Answered
            && self.phase != Phase::Closing
            && self.phase != Phase::Closed
        {
            self.answer = Some(oauth::HttpResponse {
                id: self.id,
                status: 0,
                body: Box::new([]),
                retry_after: Duration::ZERO,
                evidence: if self.sent { oauth::HttpEvidence::Unknown } else { oauth::HttpEvidence::Unsent },
                now: env.now,
                wall: env.wall,
            });
            self.phase = Phase::Answered;
        }
    }

    pub(crate) fn progress(&mut self, env: &Env<Limits>, io: &mut Queue<IoRequest>) {
        let http_env = Env { now: env.now, wall: env.wall, limits: env.limits.http };
        let tls_env = Env { now: env.now, wall: env.wall, limits: env.limits.tls };
        if env.now >= self.deadline {
            self.transport_failed(env);
        }
        if self.phase == Phase::Closing || self.phase == Phase::Closed {
            return;
        }
        if let Some(event) = self.events.pop() {
            self.http_event(env, event);
            return;
        }
        if let Some(event) = self.tls_events.pop() {
            match event {
                tls::Event::Ready(_) => self.phase = Phase::Calling,
                tls::Event::Stream(up) => http::up(&mut self.http, &http_env, up, &mut self.events, &mut self.plain),
                tls::Event::Failed(_) => {
                    // HTTP has heard the stream failure and keeps a body read
                    // whole before it. Only a handshake failure bypasses HTTP.
                    if self.phase == Phase::Handshaking {
                        self.transport_failed(env);
                    }
                }
                tls::Event::Closed => {
                    self.phase = Phase::Closing;
                    self.close_socket(io);
                }
            }
            return;
        }
        if let Some(down) = self.cipher.pop() {
            match self.socket {
                Some(socket) => io.push(IoRequest::Stream { stream: socket, down }),
                None => {}
            }
            return;
        }
        if self.phase != Phase::Calling {
            return;
        }
        if let Some(down) = self.plain.pop() {
            match &down {
                Down::Send(bytes) if !bytes.is_empty() => self.sent = true,
                Down::Send(_) | Down::Demand { .. } | Down::Finish => {}
            }
            match &mut self.tls {
                Some(client) => {
                    tls::down(client, &tls_env, tls::Request::Stream(down), &mut self.tls_events, &mut self.cipher);
                }
                None => match self.socket {
                    Some(socket) => io.push(IoRequest::Stream { stream: socket, down }),
                    None => {}
                },
            }
            return;
        }
        if let Some(request) = self.requests.pop() {
            http::down(&mut self.http, &http_env, request, &mut self.events, &mut self.plain);
        }
    }

    fn http_event(&mut self, env: &Env<Limits>, event: http::Event) {
        match event {
            http::Event::Response(response) => {
                self.response = Some(response);
                self.requests.push(http::Request::Body(Down::Demand {
                    read: Read::Arrived { max: self.document.room().saturating_add(1) },
                    room: 0,
                }));
            }
            http::Event::Upload(Up::Room) => {
                let end = self
                    .offset
                    .saturating_add(usize::try_from(env.limits.http.send).expect("u32 fits usize"))
                    .min(self.upload.len());
                let bytes = bytes::copy_of(self.upload.get(self.offset..end).expect("bounded upload"));
                self.requests.push(http::Request::Upload(Down::Send(bytes)));
                self.offset = end;
                if end == self.upload.len() {
                    self.requests.push(http::Request::Upload(Down::Finish));
                } else {
                    self.requests
                        .push(http::Request::Upload(Down::Demand { read: Read::Nothing, room: env.limits.http.send }));
                }
            }
            http::Event::Body(Up::Bytes(bytes)) => {
                let length = u32::try_from(bytes.len()).ok();
                let too_large = match length {
                    Some(len) => len > self.document.room(),
                    None => true,
                };
                if too_large {
                    self.failure = Some(oauth::Failure::Limit);
                    self.phase = Phase::Answered;
                } else {
                    for byte in &bytes {
                        self.document.push(*byte).expect("response document admitted");
                    }
                    self.requests.push(http::Request::Body(Down::Demand {
                        read: Read::Arrived { max: self.document.room().saturating_add(1) },
                        room: 0,
                    }));
                }
            }
            http::Event::Done(_) => {
                let response = self.response.take().expect("HTTP done follows response");
                self.answer = Some(oauth::HttpResponse {
                    id: self.id,
                    status: response.status,
                    body: self.document.to_boxed(),
                    retry_after: retry_after(&response),
                    evidence: oauth::HttpEvidence::Response,
                    now: env.now,
                    wall: env.wall,
                });
                self.phase = Phase::Answered;
            }
            http::Event::Failed(_)
            | http::Event::Upload(Up::Failed(_) | Up::End | Up::Bytes(_))
            | http::Event::Body(Up::Failed(_) | Up::Room) => self.transport_failed(env),
            http::Event::Body(Up::End) | http::Event::Closed => {}
        }
    }

    pub(crate) fn closing(&self) -> bool {
        self.phase == Phase::Closing
    }

    pub(crate) fn close(&mut self, env: &Env<Limits>, abort: bool, io: &mut Queue<IoRequest>) {
        if self.phase == Phase::Closed {
            return;
        }
        if abort {
            self.answer = None;
            self.failure = None;
            if !self.abort {
                self.close_sent = false;
            }
            self.abort = true;
            self.phase = Phase::Closing;
            self.close_socket(io);
        } else if self.phase != Phase::Closing {
            self.phase = Phase::Closing;
            match &mut self.tls {
                Some(client) => tls::down(
                    client,
                    &Env { now: env.now, wall: env.wall, limits: env.limits.tls },
                    tls::Request::Close,
                    &mut self.tls_events,
                    &mut self.cipher,
                ),
                None => self.close_socket(io),
            }
        }
    }

    pub(crate) fn close_progress(&mut self, _env: &Env<Limits>, io: &mut Queue<IoRequest>) {
        if self.phase != Phase::Closing {
            return;
        }
        if let Some(down) = self.cipher.pop() {
            match self.socket {
                Some(socket) => io.push(IoRequest::Stream { stream: socket, down }),
                None => {}
            }
        } else if let Some(event) = self.tls_events.pop() {
            match event {
                tls::Event::Closed => self.close_socket(io),
                tls::Event::Ready(_) | tls::Event::Stream(_) | tls::Event::Failed(_) => {}
            }
        }
    }

    fn close_socket(&mut self, io: &mut Queue<IoRequest>) {
        if self.close_sent {
            return;
        }
        match self.socket {
            Some(socket) => {
                self.close_sent = true;
                if self.abort {
                    io.push(IoRequest::Abort { entity: socket });
                } else {
                    io.push(IoRequest::Close { entity: socket });
                }
            }
            None => {}
        }
    }

    pub(crate) fn closed(&self) -> bool {
        self.phase == Phase::Closed
    }

    pub(crate) fn has_work(&self) -> bool {
        match self.phase {
            Phase::Closed | Phase::Answered => self.answer.is_some() || self.failure.is_some(),
            Phase::Closing | Phase::Handshaking => !self.cipher.is_empty() || !self.tls_events.is_empty(),
            Phase::Connecting => false,
            Phase::Calling => {
                !self.events.is_empty()
                    || !self.requests.is_empty()
                    || !self.plain.is_empty()
                    || !self.tls_events.is_empty()
                    || !self.cipher.is_empty()
            }
        }
    }
}

fn retry_after(response: &http::Response) -> Duration {
    match response.header(b"retry-after") {
        Some(bytes) => match parse_seconds(bytes) {
            Some(seconds) => Duration::from_secs(seconds),
            None => Duration::ZERO,
        },
        None => Duration::ZERO,
    }
}

fn parse_seconds(bytes: &[u8]) -> Option<u64> {
    if bytes.is_empty() {
        return None;
    }
    let mut value = 0_u64;
    for byte in bytes {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value.checked_mul(10)?.checked_add(u64::from(byte.checked_sub(b'0')?))?;
    }
    Some(value)
}
/// Resolved destinations and their prepared TLS transports.
#[derive(Debug)]
pub struct Runtime {
    pub endpoints: List<Endpoint>,
}

#[cfg(test)]
mod tests {
    #![expect(clippy::disallowed_methods, reason = "ordinary tests inspect scripted HTTP streams")]
    use super::*;
    use skein_lib::Wall;
    fn limits() -> Limits {
        Limits {
            body: 64,
            http: http::Limits { request: 1024, head: 1024, headers: 16, read: 256, send: 256 },
            tls: tls::Limits { read: 1024, send: 1024, records: 131_072 },
        }
    }
    #[test]
    fn the_http_exchange_uses_the_configured_socket_and_collects_a_cut_body() {
        for cut in [1, 7, 128] {
            let limits = limits();
            let env = Env { now: Time::ZERO, wall: Wall::EPOCH, limits };
            let endpoint = Endpoint {
                origin: b"http://127.0.0.1:31999".as_slice().into(),
                address: ([127, 0, 0, 1], 31999).into(),
                transport: Transport::Plaintext,
            };
            let request = oauth::HttpRequest {
                id: 7,
                method: oauth::Method::Post,
                endpoint: b"http://127.0.0.1:31999/token".as_slice().into(),
                content_type: b"application/x-www-form-urlencoded",
                body: b"code=throwaway".as_slice().into(),
                deadline: Time::from_nanos(1_000_000_000),
            };
            let mut web = Web::new(request, &endpoint, &limits).unwrap();
            let owner = Token::new(1);
            let socket = Token::new(99);
            let mut io = Queue::with_capacity(32);
            web.up(&env, IoEvent::Connecting { owner, socket }, &mut io);
            web.up(&env, IoEvent::Connected { owner }, &mut io);
            let mut wire = skein_world::stream::Wire::new(cut);
            let mut responded = false;
            for _ in 0_u32..512 {
                web.progress(&env, &mut io);
                while let Some(request) = io.pop() {
                    match request {
                        IoRequest::Stream { stream, down } => {
                            assert_eq!(stream, socket);
                            wire.take(down);
                        }
                        IoRequest::Listen { .. }
                        | IoRequest::Connect { .. }
                        | IoRequest::Bind { .. }
                        | IoRequest::Reject { .. }
                        | IoRequest::Output { .. }
                        | IoRequest::Spawn { .. }
                        | IoRequest::Signal { .. }
                        | IoRequest::Usage { .. }
                        | IoRequest::Window { .. }
                        | IoRequest::Close { .. }
                        | IoRequest::Abort { .. } => panic!("only the configured stream"),
                    }
                }
                if !responded && wire.received.ends_with(b"code=throwaway") {
                    wire.write(b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}");
                    responded = true;
                }
                if let Some(up) = wire.answer() {
                    web.up(&env, IoEvent::Stream { owner, up }, &mut io);
                }
                if web.answer.is_some() {
                    break;
                }
            }
            let answer = web.answer.take().expect("HTTP response");
            assert_eq!(answer.id, 7);
            assert_eq!(answer.status, 200);
            assert_eq!(answer.body.as_ref(), b"{\"ok\":true}");
            let written = core::str::from_utf8(&wire.received).unwrap();
            assert!(written.contains("POST /token HTTP/1.1"));
            assert!(written.contains("code=throwaway"));
            web.close(&env, true, &mut io);
            let Some(IoRequest::Abort { entity }) = io.pop() else { panic!("socket abort") };
            assert_eq!(entity, socket);
            web.up(&env, IoEvent::Closed { owner }, &mut io);
            assert!(web.closed());
        }
    }
    #[test]
    fn a_connect_that_finishes_after_its_deadline_is_aborted() {
        let limits = limits();
        let endpoint = Endpoint {
            origin: b"http://127.0.0.1:31999".as_slice().into(),
            address: ([127, 0, 0, 1], 31999).into(),
            transport: Transport::Plaintext,
        };
        let request = oauth::HttpRequest {
            id: 8,
            method: oauth::Method::Get,
            endpoint: b"http://127.0.0.1:31999/keys".as_slice().into(),
            content_type: b"",
            body: Box::new([]),
            deadline: Time::from_nanos(1),
        };
        let mut web = Web::new(request, &endpoint, &limits).unwrap();
        let env = Env { now: Time::from_nanos(2), wall: Wall::EPOCH, limits };
        let mut io = Queue::with_capacity(32);
        web.progress(&env, &mut io);
        assert_eq!(web.answer.take().unwrap().evidence, oauth::HttpEvidence::Unsent);
        web.close(&env, true, &mut io);
        let owner = Token::new(1);
        let socket = Token::new(99);
        web.up(&env, IoEvent::Connecting { owner, socket }, &mut io);
        let Some(IoRequest::Abort { entity }) = io.pop() else { panic!("socket abort") };
        assert_eq!(entity, socket);
        web.up(&env, IoEvent::Closed { owner }, &mut io);
        assert!(web.closed());
    }
}
