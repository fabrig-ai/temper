//! Deferred replies for the setup route (client/forge.md, 9.1).
use super::Clients;
use crate::setup::{MANIFEST_SCRIPT_HASH, MANIFEST_STYLE_HASH, PAGE_STYLE_HASH};
use alloc::boxed::Box;
use skein_http::{Header, server as http};
use skein_lib::{
    List, Token, bytes,
    stream::{Down, Read, Up},
};

#[derive(Debug)]
pub(super) struct Reply {
    body: Option<Box<[u8]>>,
    offset: usize,
}
pub(super) fn claims(target: &[u8]) -> bool {
    let path = match bytes::find(target, b"?") {
        Some(at) => target.get(..at).expect("query"),
        None => target,
    };
    path.starts_with(b"/forges/")
        && (path.ends_with(b"/github/setup")
            || path.ends_with(b"/github/manifest")
            || path.ends_with(b"/github/redirect"))
}
impl Clients {
    pub(super) fn github_call(&mut self, call: Token, head: http::Call) {
        self.github.insert(call, Reply { body: None, offset: 0 }).expect("bounded HTTP calls");
        if self.github_calls.room() == 0 {
            crate::setup::busy(self, call, &head.target);
            return;
        }
        self.github_calls.push((call, head));
    }
    pub(crate) fn github_answer(&mut self, call: Token, status: u16, location: Option<&[u8]>, body: &[u8]) {
        if location.is_some() || body.is_empty() {
            self.github_response(call, status, location, body, None);
            return;
        }
        let parts = [b"default-src 'none'; style-src 'sha256-".as_slice(), PAGE_STYLE_HASH, b"'"];
        self.github_policy_answer(call, status, body, &parts);
    }
    pub(crate) fn github_manifest_answer(&mut self, call: Token, body: &[u8], web: &[u8]) {
        let parts = [
            b"default-src 'none'; form-action ".as_slice(),
            web,
            b"; script-src 'sha256-",
            MANIFEST_SCRIPT_HASH,
            b"'; style-src 'sha256-",
            MANIFEST_STYLE_HASH,
            b"'",
        ];
        self.github_policy_answer(call, 200, body, &parts);
    }
    fn github_policy_answer(&mut self, call: Token, status: u16, body: &[u8], parts: &[&[u8]]) {
        let mut size = 0_usize;
        for part in parts {
            size = size.checked_add(part.len()).expect("bounded CSP");
        }
        let mut writer = skein_lib::Writer::new(size);
        for part in parts {
            writer.put(part).expect("measured CSP");
        }
        self.github_response(call, status, None, body, Some(&writer.finish()));
    }
    fn github_response(&mut self, call: Token, status: u16, location: Option<&[u8]>, body: &[u8], csp: Option<&[u8]>) {
        let Some(reply) = self.github.get_mut(&call) else { return };
        reply.body = Some(bytes::copy_of(body));
        let mut headers = List::with_capacity(4);
        for (name, value) in [
            (b"Cache-Control".as_slice(), b"no-store".as_slice()),
            (b"Referrer-Policy", b"no-referrer"),
            (b"Content-Type", b"text/html; charset=utf-8"),
        ] {
            headers.push(Header { name: bytes::copy_of(name), value: bytes::copy_of(value) }).expect("four headers");
        }
        if let Some(location) = location {
            headers
                .push(Header { name: bytes::copy_of(b"Location"), value: bytes::copy_of(location) })
                .expect("location header");
        }
        if let Some(csp) = csp {
            headers
                .push(Header { name: bytes::copy_of(b"Content-Security-Policy"), value: bytes::copy_of(csp) })
                .expect("policy header");
        }
        self.server.answer(call, http::Request::Discard, &mut self.server_out);
        self.server.answer(
            call,
            http::Request::Respond(http::Response {
                status,
                headers: headers.into_boxed(),
                // An empty redirect has no body stream to Finish before keep-alive reuse.
                body: if body.is_empty() {
                    http::Body::None
                } else {
                    http::Body::Length(u64::try_from(body.len()).expect("bounded page"))
                },
                close: false,
            }),
            &mut self.server_out,
        );
        if body.is_empty() {
            return;
        }
        self.server.answer(
            call,
            http::Request::Reply(Down::Demand {
                read: Read::Nothing,
                room: self.send.min(u32::try_from(body.len()).expect("bounded page")),
            }),
            &mut self.server_out,
        );
    }
    pub(super) fn github_event(&mut self, call: Token, event: http::Event) {
        match event {
            http::Event::Reply(Up::Room) => {
                let reply = self.github.get_mut(&call).expect("route reply");
                let Some(body) = &reply.body else { return };
                let end = reply.offset.saturating_add(usize::try_from(self.send).expect("chunk bound")).min(body.len());
                let chunk = bytes::copy_of(body.get(reply.offset..end).expect("page cursor"));
                reply.offset = end;
                let left = body.len().checked_sub(end).expect("page cursor");
                self.server.answer(call, http::Request::Reply(Down::Send(chunk)), &mut self.server_out);
                if left == 0 {
                    self.server.answer(call, http::Request::Reply(Down::Finish), &mut self.server_out);
                } else {
                    self.server.answer(
                        call,
                        http::Request::Reply(Down::Demand {
                            read: Read::Nothing,
                            room: self.send.min(u32::try_from(left).expect("page bound")),
                        }),
                        &mut self.server_out,
                    );
                }
            }
            http::Event::Done(_)
            | http::Event::Ended
            | http::Event::Refused(_)
            | http::Event::Failed(_)
            | http::Event::Closed
            | http::Event::Reply(Up::Failed(_))
            | http::Event::Body(Up::Failed(_)) => {
                self.github.remove(&call);
                self.server.done(call, &mut self.server_out);
            }
            http::Event::Body(_) | http::Event::Reply(_) => {}
            http::Event::Call(_) => unreachable!("head dispatched separately"),
        }
    }
}
