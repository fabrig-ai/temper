//! Server-owned `OpenID` endpoints (protocol/sign-in.md, 9.2).
use alloc::boxed::Box;
use jig_protocol_config::{self as config, Path, Problems, clients::ProviderUrls};
use skein_lib::{Writer, bytes};

#[derive(Debug)]
pub(super) struct Documents {
    pub discovery: Box<[u8]>,
    pub keys: Box<[u8]>,
}
pub(super) fn problem(problem: config::Problem) -> Problems {
    let mut problems = Problems::empty();
    let mut path = Path::root().member(b"engine");
    for segment in &problem.path.segments {
        path = path.child(segment.clone());
    }
    problems.push(path, problem.what);
    problems
}
fn endpoint(origin: &[u8], path: &[u8]) -> Box<[u8]> {
    let mut out = Writer::new(origin.len().checked_add(path.len()).expect("provider URL"));
    out.put(origin).expect("origin");
    out.put(path).expect("route");
    out.finish()
}
pub(super) fn urls(
    clients: &crate::startup::application::Clients,
    listener: skein_io::Addr,
) -> Result<ProviderUrls, Problems> {
    let base = match &clients.public {
        Some(origin) => origin.base(),
        None => {
            let skein_io::Addr::Ip(listener) = listener else {
                return Err(super::invalid(config::What::Form));
            };
            let host = super::printed::address_text(listener).ok_or(super::invalid(config::What::Form))?;
            endpoint(b"http://", &host)
        }
    };
    let origin = match base.strip_suffix(b"/") {
        Some(origin) => bytes::copy_of(origin),
        None => base,
    };
    Ok(ProviderUrls {
        issuer: origin.clone(),
        public_origin: origin.clone(),
        authorization_endpoint: endpoint(&origin, b"/oauth/authorize"),
        token_endpoint: endpoint(&origin, b"/oauth/token"),
        userinfo_endpoint: endpoint(&origin, b"/oauth/userinfo"),
        jwks_uri: endpoint(&origin, b"/oauth/keys"),
    })
}

use super::Clients;
use jig_protocol_engine_clients as kit;
use skein_http::{Header, Method, server as http};
use skein_lib::{
    Intake, List, Secret, Token,
    stream::{Down, Read, Up},
};
const EXCHANGE: u64 = 1_u64 << 61;
pub(super) struct Reply {
    exchange: Option<Token>,
    incoming: Option<Intake>,
    authorization: Option<Secret>,
    content_type: Option<Box<[u8]>>,
    body: Option<Secret>,
    offset: usize,
}
pub(super) fn claims(target: &[u8]) -> bool {
    route(target).is_some()
}
fn route(target: &[u8]) -> Option<u8> {
    let path = match bytes::find(target, b"?") {
        Some(at) => target.get(..at)?,
        None => target,
    };
    match path {
        b"/.well-known/openid-configuration" => Some(0),
        b"/oauth/keys" => Some(1),
        b"/oauth/token" => Some(2),
        b"/oauth/userinfo" => Some(3),
        _ => None,
    }
}
impl Clients {
    #[expect(clippy::manual_map, reason = "the programming model uses explicit branches without closures")]
    pub(super) fn provider_call(&mut self, call: Token, head: http::Call) {
        let route = route(&head.target).expect("claimed route");
        self.provider_replies
            .insert(
                call,
                Reply {
                    exchange: None,
                    incoming: None,
                    authorization: None,
                    content_type: None,
                    body: None,
                    offset: 0,
                },
            )
            .ok()
            .expect("bounded HTTP calls");
        let Some(documents) = &self.provider_documents else {
            self.provider_response(call, 404, None, Secret::new(Box::new([])), false);
            return;
        };
        let method = if route == 2 { Method::Post } else { Method::Get };
        if head.method != method {
            self.provider_response(call, 405, None, Secret::new(Box::new([])), false);
            return;
        }
        if route < 2 {
            let document = if route == 0 { &documents.discovery } else { &documents.keys };
            self.provider_response(call, 200, None, Secret::new(document.clone()), false);
            return;
        }
        if self.stopping.is_some() {
            self.provider_response(call, 503, None, Secret::new(Box::new([])), true);
            return;
        }
        let exchange = Token::new(EXCHANGE.checked_add(self.next_provider).expect("provider exchanges"));
        self.next_provider = self.next_provider.checked_add(1).expect("provider exchanges");
        let reply = self.provider_replies.get_mut(&call).expect("route reply");
        reply.exchange = Some(exchange);
        reply.authorization = match head.header(b"authorization") {
            Some(value) => Some(Secret::new(bytes::copy_of(value))),
            None => None,
        };
        if route == 3 {
            let authorization = reply.authorization.take();
            self.down(kit::Event::Claims { exchange, authorization });
        } else {
            let bound = kit::provider_limits(&self.env.limits).request_bytes;
            if let http::Body::Length(length) = head.body
                && length > u64::from(bound)
            {
                self.provider_response(call, 413, None, Secret::new(Box::new([])), true);
                return;
            }
            reply.content_type = match head.header(b"content-type") {
                Some(value) => Some(bytes::copy_of(value)),
                None => None,
            };
            reply.incoming = Some(Intake::with_capacity(bound));
            self.provider_read(call);
        }
    }
    fn provider_read(&mut self, call: Token) {
        // Arrived is allowed to exceed the stream's intake; a positive maximum
        // also observes one byte beyond the issuer bound without storing it.
        let bound = kit::provider_limits(&self.env.limits).request_bytes;
        self.server.answer(
            call,
            http::Request::Body(Down::Demand { read: Read::Arrived { max: bound.saturating_add(1) }, room: 0 }),
            &mut self.server_out,
        );
    }
    pub(super) fn provider_answer(&mut self, exchange: Token, answer: kit::ProviderAnswer) {
        let mut call = None;
        for (key, reply) in &self.provider_replies {
            if reply.exchange == Some(exchange) {
                call = Some(*key);
                break;
            }
        }
        if let Some(call) = call {
            self.provider_response(call, answer.status, answer.challenge, answer.body, true);
        }
    }
    fn provider_response(
        &mut self,
        call: Token,
        status: u16,
        challenge: Option<Box<[u8]>>,
        body: Secret,
        private: bool,
    ) {
        let Some(reply) = self.provider_replies.get_mut(&call) else { return };
        reply.exchange = None;
        reply.incoming = None;
        reply.authorization = None;
        reply.content_type = None;
        let length = body.len();
        reply.body = Some(body);
        let mut headers = List::with_capacity(3);
        headers
            .push(Header { name: bytes::copy_of(b"Content-Type"), value: bytes::copy_of(b"application/json") })
            .expect("JSON header");
        if private {
            headers
                .push(Header { name: bytes::copy_of(b"Cache-Control"), value: bytes::copy_of(b"no-store") })
                .expect("private response");
        }
        if let Some(value) = challenge {
            headers.push(Header { name: bytes::copy_of(b"WWW-Authenticate"), value }).expect("challenge");
        }
        self.server.answer(call, http::Request::Discard, &mut self.server_out);
        self.server.answer(
            call,
            http::Request::Respond(http::Response {
                status,
                headers: headers.into_boxed(),
                body: if length == 0 {
                    http::Body::None
                } else {
                    http::Body::Length(u64::try_from(length).expect("bounded document"))
                },
                close: false,
            }),
            &mut self.server_out,
        );
        if length > 0 {
            self.provider_demand(call, length);
        }
    }
    fn provider_demand(&mut self, call: Token, length: usize) {
        self.server.answer(
            call,
            http::Request::Reply(Down::Demand {
                read: Read::Nothing,
                room: self.send.min(u32::try_from(length).expect("bounded document")),
            }),
            &mut self.server_out,
        );
    }
    pub(super) fn provider_event(&mut self, call: Token, event: http::Event) {
        match event {
            http::Event::Body(Up::Bytes(bytes)) => {
                let reply = self.provider_replies.get_mut(&call).expect("route reply");
                let Some(incoming) = &mut reply.incoming else { return };
                if incoming.append(&bytes).is_err() {
                    self.provider_response(call, 413, None, Secret::new(Box::new([])), true);
                } else {
                    self.provider_read(call);
                }
            }
            http::Event::Body(Up::End) => {
                let reply = self.provider_replies.get_mut(&call).expect("route reply");
                let Some(mut incoming) = reply.incoming.take() else { return };
                let exchange = reply.exchange.expect("pending redemption");
                let authorization = reply.authorization.take();
                let content_type = reply.content_type.take();
                self.down(kit::Event::Redeem {
                    exchange,
                    authorization,
                    content_type,
                    body: Secret::new(incoming.meet(Read::Fill(incoming.len())).expect("whole request")),
                });
            }
            http::Event::Reply(Up::Room) => {
                let reply = self.provider_replies.get_mut(&call).expect("route reply");
                let Some(body) = &reply.body else { return };
                let end = reply.offset.saturating_add(usize::try_from(self.send).expect("chunk")).min(body.len());
                let chunk = bytes::copy_of(body.expose().get(reply.offset..end).expect("reply cursor"));
                reply.offset = end;
                let left = body.len().saturating_sub(end);
                self.server.answer(call, http::Request::Reply(Down::Send(chunk)), &mut self.server_out);
                if left == 0 {
                    self.server.answer(call, http::Request::Reply(Down::Finish), &mut self.server_out);
                } else {
                    self.provider_demand(call, left);
                }
            }
            http::Event::Done(_)
            | http::Event::Ended
            | http::Event::Refused(_)
            | http::Event::Failed(_)
            | http::Event::Closed
            | http::Event::Reply(Up::Failed(_))
            | http::Event::Body(Up::Failed(_)) => {
                self.provider_replies.remove(&call);
                self.server.done(call, &mut self.server_out);
            }
            http::Event::Call(_) => unreachable!("head routed separately"),
            http::Event::Reply(_) | http::Event::Body(_) => {}
        }
    }
}

#[cfg(test)]
mod tests;
