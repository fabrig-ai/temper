//! Google's sign-ins on the clients' listener (protocol/sign-in.md, 4 and 6).
mod limits;
mod pages;
use super::{Clients, fetch};
use alloc::boxed::Box;
use jig_domain_people as people;
use jig_protocol_engine_clients as kit;
use jig_protocol_engine_clients_http as mapping;
use skein_http::{Header, Method, server as http};
use skein_lib::{
    CryptoRng, Decimal, Duration, Env, List, Map, Queue, Secret, Time, Token, Wall, Writer,
    base64::{self, Alphabet},
    bytes,
    stream::{Down, Read, Up},
};
use skein_oauth as oauth;
use temper_engine_domain_forge as forge;
const FETCH: u64 = 1_u64 << 59;
const EXCHANGE: u64 = 1_u64 << 62;
const STREAM: u64 = 0x676f_6f67_6c65;

pub(crate) struct Google {
    settings: crate::settings::SignIn,
    limits: crate::limits::SignIn,
    identity_bytes: u32,
    rng: CryptoRng,
    pending: Map<Token, Pending>,
    next: u64,
    http: Queue<(Token, oauth::HttpRequest)>,
    endpoints: List<fetch::Endpoint>,
    fetches: Map<Token, Fetch>,
    next_fetch: u64,
    #[cfg(test)]
    pub(crate) scripted: bool,
}
struct Pending {
    tag: Box<[u8]>,
    then: Box<[u8]>,
    deadline: Time,
    call: Option<Token>,
    redirect: Box<[u8]>,
    client: Option<oauth::Client>,
    out: Queue<oauth::Request>,
    phase: Phase,
}
enum Phase {
    Starting,
    Visiting,
    Exchanging,
    Binding(people::Identity),
    Minting,
}
struct Fetch {
    exchange: Token,
    web: fetch::Web,
}
#[derive(Debug)]
pub(super) struct Reply {
    body: Option<Box<[u8]>>,
    offset: usize,
    returned: bool,
    /// How the call came, as jig decides its cookies' `Secure` (jig protocol/client.md, 17.3).
    via: mapping::Via,
}
#[must_use]
pub fn claims(target: &[u8]) -> bool {
    let (path, _) = split(target);
    path == b"/sign-in/google" || path == b"/sign-in/google/return"
}
fn split(target: &[u8]) -> (&[u8], &[u8]) {
    match bytes::find(target, b"?") {
        Some(at) => (target.get(..at).expect("query"), target.get(at.saturating_add(1)..).expect("query")),
        None => (target, b""),
    }
}
pub(super) fn encode(value: &[u8]) -> Box<[u8]> {
    const HEX: &[u8] = b"0123456789ABCDEF";
    let mut size = 0_usize;
    for byte in value {
        size = size
            .checked_add(if byte.is_ascii_alphanumeric() || b"-_.~".contains(byte) { 1 } else { 3 })
            .expect("bounded URL");
    }
    let mut out = Writer::new(size);
    for byte in value {
        if byte.is_ascii_alphanumeric() || b"-_.~".contains(byte) {
            out.put(core::slice::from_ref(byte)).expect("URL");
        } else {
            out.put(&[
                b'%',
                *HEX.get(usize::from(byte >> 4_u32)).expect("hex"),
                *HEX.get(usize::from(byte & 15)).expect("hex"),
            ])
            .expect("URL");
        }
    }
    out.finish()
}
fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => byte.checked_sub(b'0'),
        b'a'..=b'f' => byte.checked_sub(b'a')?.checked_add(10),
        b'A'..=b'F' => byte.checked_sub(b'A')?.checked_add(10),
        _ => None,
    }
}
fn decode(value: &[u8]) -> Option<Box<[u8]>> {
    let mut escapes = 0_usize;
    for byte in value {
        if *byte == b'%' {
            escapes = escapes.checked_add(1)?;
        }
    }
    let mut out = Writer::new(value.len().checked_sub(escapes.checked_mul(2)?)?);
    let mut at = 0_usize;
    for _ in 0..value.len() {
        let Some(byte) = value.get(at) else { break };
        let value = if *byte == b'%' {
            let hi = hex(*value.get(at.checked_add(1)?)?)?;
            let lo = hex(*value.get(at.checked_add(2)?)?)?;
            at = at.checked_add(3)?;
            hi.checked_mul(16)?.checked_add(lo)?
        } else {
            at = at.checked_add(1)?;
            if *byte == b'+' { b' ' } else { *byte }
        };
        out.put(&[value]).ok()?;
    }
    Some(out.finish())
}
fn query(query: &[u8], name: &[u8]) -> Option<Box<[u8]>> {
    let mut rest = Some(query);
    let mut found = None;
    for _ in 0..=query.len() {
        let Some(value) = rest.take() else { break };
        let field = match bytes::find(value, b"&") {
            Some(at) => {
                rest = value.get(at.saturating_add(1)..);
                value.get(..at)?
            }
            None => value,
        };
        let Some(at) = bytes::find(field, b"=") else { continue };
        if decode(field.get(..at)?)?.as_ref() == name {
            if found.is_some() {
                return None;
            }
            found = Some(decode(field.get(at.checked_add(1)?..)?)?);
        }
    }
    found
}
fn then(query_bytes: &[u8]) -> Box<[u8]> {
    if let Some(value) = query(query_bytes, b"then")
        && value.len() <= 1024
        && value.starts_with(b"/")
        && !value.starts_with(b"//")
        && !value.contains(&b'\\')
        && crate::startup::application::sign_in::visible(&value)
    {
        return value;
    }
    bytes::copy_of(b"/")
}
fn tag(head: &http::Call) -> Option<Box<[u8]>> {
    let header = head.header(b"cookie")?;
    let mut rest = Some(header);
    let mut found = None;
    for _ in 0..=header.len() {
        let Some(pair) = rest.take() else { break };
        let pair = match bytes::find(pair, b";") {
            Some(at) => {
                rest = pair.get(at.saturating_add(1)..);
                pair.get(..at)?
            }
            None => pair,
        };
        if let Some(value) = pair.trim_ascii().strip_prefix(b"temper-sign-in=") {
            if found.is_some() {
                return None;
            }
            let value = base64::decode(Alphabet::Url, value, 16).ok()?;
            if value.len() != 16 {
                return None;
            }
            found = Some(value);
        }
    }
    found
}
fn random(rng: &mut CryptoRng) -> Box<[u8]> {
    let mut value = [0_u8; 32];
    rng.fill(&mut value);
    let mut out = Writer::new(43);
    base64::encode(Alphabet::Url, &value, &mut out).expect("random base64");
    out.finish()
}
fn cookie(public: bool, name: &[u8], path: &[u8], value: &[u8], life: Duration, lax: bool) -> Header {
    let seconds = Decimal::of(life.as_nanos().checked_div(1_000_000_000).expect("seconds"));
    let encoded =
        base64::encoded_len(Alphabet::Url, u32::try_from(value.len()).expect("cookie bound")).expect("base64");
    let mut size = usize::try_from(encoded).expect("cookie");
    let attributes = if lax {
        b"; HttpOnly; SameSite=Lax; Max-Age=".as_slice()
    } else {
        b"; HttpOnly; SameSite=Strict; Max-Age=".as_slice()
    };
    let secure = if public { b"; Secure".as_slice() } else { b"".as_slice() };
    for part in [name, b"=", b"; Path=", path, attributes, seconds.as_bytes(), secure] {
        size = size.checked_add(part.len()).expect("bounded cookie");
    }
    let mut out = Writer::new(size);
    out.put(name).expect("name");
    out.put(b"=").expect("separator");
    base64::encode(Alphabet::Url, value, &mut out).expect("cookie value");
    for part in [b"; Path=".as_slice(), path, attributes, seconds.as_bytes(), secure] {
        out.put(part).expect("attributes");
    }
    Header { name: bytes::copy_of(b"Set-Cookie"), value: out.finish() }
}
impl Google {
    pub(super) fn new(
        settings: &crate::settings::SignIn,
        limits: crate::limits::SignIn,
        seeds: &crate::settings::Seeds,
        identity_bytes: u32,
    ) -> Self {
        Self {
            settings: crate::settings::SignIn {
                provider: settings.provider,
                client: settings.client.clone(),
                secret: settings.secret.clone(),
                issuer: settings.issuer.clone(),
                authorization: settings.authorization.clone(),
                token: settings.token.clone(),
                keys: settings.keys.clone(),
                transport: None,
                members: settings.members.clone(),
                forges: settings.forges.clone(),
            },
            limits,
            identity_bytes,
            rng: CryptoRng::new(&seeds.crypto, seeds.seed ^ STREAM).expect("validated cryptographic seed"),
            pending: Map::with_capacity(limits.pending),
            next: 1,
            http: Queue::with_capacity(limits.pending),
            endpoints: List::with_capacity(4),
            fetches: Map::with_capacity(limits.fetches),
            next_fetch: 1,
            #[cfg(test)]
            scripted: false,
        }
    }
    fn registration(&self, redirect: Box<[u8]>, choose: bool) -> oauth::Registration {
        oauth::Registration {
            authorization_url: self.settings.authorization.clone(),
            token_endpoint: self.settings.token.clone(),
            client_id: self.settings.client.clone(),
            redirect_uri: redirect,
            scope: bytes::copy_of(b"openid email profile"),
            wire: oauth::WireFormat::Form,
            client_secret: Some(bytes::copy_of(self.settings.secret.expose())),
            pkce_for_confidential: true,
            metadata_claim: None,
            terms: Some(oauth::Terms {
                parameters: if choose {
                    Box::new([oauth::Parameter {
                        name: bytes::copy_of(b"prompt"),
                        value: bytes::copy_of(b"select_account"),
                        sent: oauth::Sent::EverySignIn,
                    }])
                } else {
                    Box::new([])
                },
                openid: Some(oauth::OpenId {
                    issuer: self.settings.issuer.clone(),
                    key_set: Some(self.settings.keys.clone()),
                }),
                issued_client_id: false,
                varying_port: false,
                resource: None,
                required_scopes: None,
                revocation_endpoint: None,
                sign_in_again: Box::new([]),
            }),
        }
    }
    pub(super) fn next_deadline(&self) -> Option<Time> {
        let mut next = None;
        for (_, pending) in &self.pending {
            next = crate::accounts::earliest(next, Some(pending.deadline));
        }
        for (_, fetch) in &self.fetches {
            if !fetch.web.closing() && !fetch.web.closed() {
                next = crate::accounts::earliest(next, Some(fetch.web.deadline));
            }
        }
        next
    }
    pub(super) fn work_pending(&self) -> bool {
        if !self.http.is_empty() && self.fetches.len() < self.limits.fetches {
            return true;
        }
        for (_, pending) in &self.pending {
            if !pending.out.is_empty() {
                return true;
            }
        }
        for (_, fetch) in &self.fetches {
            if fetch.web.has_work() {
                return true;
            }
        }
        false
    }
    pub(super) fn closed(&self) -> bool {
        self.pending.is_empty() && self.fetches.is_empty() && self.http.is_empty()
    }
}
struct Claims {
    email: Option<Box<[u8]>>,
    verified: bool,
    name: Option<Box<[u8]>>,
}
fn claims_from(token: &[u8], limits: &oauth::Limits) -> Option<Claims> {
    let dot = bytes::find(token, b".")?;
    let rest = token.get(dot.checked_add(1)?..)?;
    let end = bytes::find(rest, b".")?;
    let payload = base64::decode(Alphabet::Url, rest.get(..end)?, limits.id_token_bytes).ok()?;
    let json = oauth::Json::from_bytes(&payload, limits).ok()?;
    let mut claims = Claims { email: None, verified: false, name: None };
    let mut depth = 0_u32;
    for (at, token) in json.as_tokens().iter().enumerate() {
        match token {
            skein_json::Token::ObjectStart | skein_json::Token::ArrayStart => depth = depth.saturating_add(1),
            skein_json::Token::ObjectEnd | skein_json::Token::ArrayEnd => depth = depth.saturating_sub(1),
            skein_json::Token::Key(key) if depth == 1 => {
                let value = json.as_tokens().get(at.saturating_add(1));
                match (key.as_ref(), value) {
                    (b"email", Some(skein_json::Token::String(value))) if value.len() <= 254 => {
                        claims.email = Some(value.clone());
                    }
                    (b"email_verified", Some(skein_json::Token::True)) => claims.verified = true,
                    (b"email_verified", Some(skein_json::Token::String(value))) if value.as_ref() == b"true" => {
                        claims.verified = true;
                    }
                    (b"name", Some(skein_json::Token::String(value))) => claims.name = Some(value.clone()),
                    _ => {}
                }
            }
            skein_json::Token::Key(_)
            | skein_json::Token::String(_)
            | skein_json::Token::Number(_)
            | skein_json::Token::True
            | skein_json::Token::False
            | skein_json::Token::Null => {}
        }
    }
    Some(claims)
}
impl Clients {
    fn google_via(&self, call: Token) -> mapping::Via {
        match self.google_replies.get(&call) {
            Some(reply) => reply.via,
            None => mapping::Via::Listener,
        }
    }
    #[expect(clippy::too_many_lines, reason = "the start and return share one bounded route admission")]
    pub(super) fn google_call(&mut self, call: Token, head: http::Call) {
        let (path, parameters) = split(&head.target);
        let returned = path == b"/sign-in/google/return";
        let via = mapping::via(&super::http_settings(self.public.as_ref()), &head);
        self.google_replies.insert(call, Reply { body: None, offset: 0, returned, via }).expect("bounded HTTP calls");
        let Some(mut google) = self.google.take() else {
            self.google_answer(call, 404, None, &[], None);
            return;
        };
        if head.method != Method::Get {
            self.google_answer(call, 405, None, &[], None);
        } else if self.stopping.is_some() {
            self.google_answer(call, 503, None, &pages::busy(b"/"), None);
        } else if returned {
            let tag = tag(&head);
            let mut exchange = None;
            if let Some(tag) = tag {
                for (key, pending) in &google.pending {
                    if skein_lib::sha256::equal(&pending.tag, &tag) {
                        exchange = Some(*key);
                        break;
                    }
                }
            }
            match exchange {
                Some(exchange) => {
                    let Some(mut pending) = google.pending.remove(&exchange) else { unreachable!("found pending") };
                    let visiting = match pending.phase {
                        Phase::Visiting => true,
                        Phase::Starting | Phase::Exchanging | Phase::Binding(_) | Phase::Minting => false,
                    };
                    let redirect_host = match crate::startup::sign_in_url(&pending.redirect) {
                        Some((origin, _, _, _)) => match origin.strip_prefix(b"https://") {
                            Some(host) => Some(host),
                            None => origin.strip_prefix(b"http://"),
                        },
                        None => None,
                    };
                    if !visiting || self.env.now >= pending.deadline || head.header(b"host") != redirect_host {
                        self.google_answer(call, 400, None, &pages::expired(&pending.then), None);
                        if !visiting && self.env.now < pending.deadline {
                            google.keep_pending(exchange, pending);
                        } else if let Some(held) = pending.call {
                            self.google_answer(held, 400, None, &pages::expired(&pending.then), None);
                        }
                    } else if let Some(error) = query(parameters, b"error") {
                        self.google_answer(call, 400, None, &pages::cancelled(&error, &pending.then), None);
                    } else {
                        self.server.hold(call, pending.deadline);
                        pending.call = Some(call);
                        pending.phase = Phase::Exchanging;
                        pending.client.as_mut().expect("visiting OAuth client").step(
                            oauth::Event::Redirected {
                                uri: pending.redirect.clone(),
                                state: query(parameters, b"state").unwrap_or_default(),
                                code: query(parameters, b"code"),
                                error: None,
                                client_id: None,
                                now: self.env.now,
                            },
                            &mut pending.out,
                        );
                        google.keep_pending(exchange, pending);
                    }
                }
                None => self.google_answer(call, 400, None, &pages::expired(b"/"), None),
            }
        } else {
            let then = then(parameters);
            if google.pending.len() >= google.limits.pending || self.starting.is_some() {
                self.google_answer(call, 503, None, &pages::busy(&then), None);
            } else {
                let redirect = match &self.public {
                    Some(origin) => origin.base(),
                    None => {
                        let host = super::printed::address_text(self.server.listening().expect("serving listener"))
                            .expect("bound listener");
                        let mut out = Writer::new(
                            7_usize.checked_add(host.len()).expect("origin").checked_add(1).expect("slash"),
                        );
                        out.put(b"http://").expect("scheme");
                        out.put(&host).expect("host");
                        out.put(b"/").expect("slash");
                        out.finish()
                    }
                };
                let path = b"sign-in/google/return";
                let mut out = Writer::new(redirect.len().checked_add(path.len()).expect("redirect"));
                out.put(&redirect).expect("origin");
                out.put(path).expect("path");
                let redirect = out.finish();
                let choose = query(parameters, b"account").as_deref() == Some(b"choose");
                let registration = google.registration(redirect.clone(), choose);
                let mut tag = [0_u8; 16];
                google.rng.fill(&mut tag);
                let mut pending = Pending {
                    tag: bytes::copy_of(&tag),
                    then,
                    deadline: self.env.now.saturating_add(google.limits.deadline),
                    call: Some(call),
                    redirect,
                    client: Some(oauth::Client::new(limits::client(google.limits)).expect("validated OAuth limits")),
                    out: Queue::with_capacity(oauth::MAX_OUT),
                    phase: Phase::Starting,
                };
                let state = random(&mut google.rng);
                let verifier = random(&mut google.rng);
                let nonce = random(&mut google.rng);
                pending.client.as_mut().expect("new client").step(
                    oauth::Event::SignIn {
                        registration,
                        key: 1,
                        generation: 1,
                        state,
                        verifier: Some(verifier),
                        nonce: Some(nonce),
                        port: None,
                        issued_client_id: None,
                        id_token: None,
                        subject: None,
                        now: self.env.now,
                    },
                    &mut pending.out,
                );
                let exchange = Token::new(EXCHANGE | google.next);
                google.next = google.next.checked_add(1).expect("sign-in sequence");
                self.server.hold(call, pending.deadline);
                google.keep_pending(exchange, pending);
            }
        }
        self.google = Some(google);
    }
    fn google_answer(&mut self, call: Token, status: u16, location: Option<&[u8]>, body: &[u8], set: Option<Header>) {
        let secure = self.google_via(call) == mapping::Via::Public;
        self.server.release(call);
        let Some(reply) = self.google_replies.get_mut(&call) else { return };
        if reply.body.is_some() {
            return;
        }
        reply.body = Some(bytes::copy_of(body));
        let mut headers = List::with_capacity(6);
        for (name, value) in [
            (b"Cache-Control".as_slice(), b"no-store".as_slice()),
            (b"Referrer-Policy", b"no-referrer"),
            (b"Content-Type", b"text/html; charset=utf-8"),
        ] {
            headers.push(Header { name: bytes::copy_of(name), value: bytes::copy_of(value) }).expect("response fields");
        }
        if let Some(location) = location {
            headers
                .push(Header { name: bytes::copy_of(b"Location"), value: bytes::copy_of(location) })
                .expect("redirect");
        }
        if let Some(set) = set {
            headers.push(set).expect("sign-in cookie");
        }
        if reply.returned {
            headers
                .push(cookie(secure, b"temper-sign-in", b"/sign-in/google", &[], Duration::ZERO, true))
                .expect("clear tag cookie");
        }
        self.server.answer(call, http::Request::Discard, &mut self.server_out);
        self.server.answer(
            call,
            http::Request::Respond(http::Response {
                status,
                headers: headers.into_boxed(),
                body: if body.is_empty() {
                    http::Body::None
                } else {
                    http::Body::Length(u64::try_from(body.len()).expect("bounded page"))
                },
                close: false,
            }),
            &mut self.server_out,
        );
        if !body.is_empty() {
            self.server.answer(
                call,
                http::Request::Reply(Down::Demand {
                    read: Read::Nothing,
                    room: self.send.min(u32::try_from(body.len()).expect("page")),
                }),
                &mut self.server_out,
            );
        }
    }
    pub(super) fn google_event(&mut self, call: Token, event: http::Event) {
        match event {
            http::Event::Reply(Up::Room) => {
                let reply = self.google_replies.get_mut(&call).expect("route reply");
                let Some(body) = &reply.body else { return };
                let end = reply.offset.saturating_add(usize::try_from(self.send).expect("chunk")).min(body.len());
                let chunk = bytes::copy_of(body.get(reply.offset..end).expect("page cursor"));
                reply.offset = end;
                let left = body.len().saturating_sub(end);
                self.server.answer(call, http::Request::Reply(Down::Send(chunk)), &mut self.server_out);
                if left == 0 {
                    self.server.answer(call, http::Request::Reply(Down::Finish), &mut self.server_out);
                } else {
                    self.server.answer(
                        call,
                        http::Request::Reply(Down::Demand {
                            read: Read::Nothing,
                            room: self.send.min(u32::try_from(left).expect("page")),
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
                self.google_replies.remove(&call);
                self.server.done(call, &mut self.server_out);
                if let Some(google) = &mut self.google {
                    let mut remove = None;
                    for (exchange, pending) in &google.pending {
                        if pending.call == Some(call) {
                            remove = Some(*exchange);
                            break;
                        }
                    }
                    if let Some(exchange) = remove {
                        google.pending.remove(&exchange);
                    }
                }
            }
            http::Event::Call(_) => unreachable!("head routed separately"),
            http::Event::Reply(_) | http::Event::Body(_) => {}
        }
    }
    pub(crate) fn bound(&mut self, request: Token, answer: forge::BindAnswer) {
        let Some(mut google) = self.google.take() else { return };
        if let Some(mut pending) = google.pending.remove(&request) {
            let phase = core::mem::replace(&mut pending.phase, Phase::Minting);
            match phase {
                Phase::Binding(identity) if self.env.now < pending.deadline && self.stopping.is_none() => {
                    match answer {
                        forge::BindAnswer::Bound => {
                            self.down(kit::Event::Mint { exchange: request, identity });
                            google.keep_pending(request, pending);
                        }
                        forge::BindAnswer::Full => {
                            if let Some(call) = pending.call {
                                self.google_answer(call, 503, None, &pages::busy(&pending.then), None);
                            }
                        }
                        forge::BindAnswer::Another => {
                            if let Some(call) = pending.call {
                                self.google_answer(
                                    call,
                                    403,
                                    None,
                                    &pages::another(&identity.key.subject, &pending.then),
                                    None,
                                );
                            }
                        }
                    }
                }
                Phase::Starting | Phase::Visiting | Phase::Exchanging | Phase::Binding(_) | Phase::Minting => {
                    if let Some(call) = pending.call {
                        self.google_answer(call, 400, None, &pages::expired(&pending.then), None);
                    }
                }
            }
        }
        self.google = Some(google);
    }
    pub(super) fn google_minted(&mut self, exchange: Token, answer: Option<(Secret, Wall)>) {
        let pending = match &mut self.google {
            Some(google) => google.pending.remove(&exchange),
            None => None,
        };
        let Some(pending) = pending else { return };
        let Some(call) = pending.call else { return };
        if self.env.now >= pending.deadline {
            self.google_answer(call, 400, None, &pages::expired(&pending.then), None);
        } else if let Some((credential, expires)) = answer {
            let life = Duration::from_nanos(expires.as_nanos().saturating_sub(self.env.wall.as_nanos()));
            let cookie =
                mapping::cookie(&super::http_settings(self.public.as_ref()), self.google_via(call), &credential, life);
            self.google_answer(call, 303, Some(&pending.then), &[], Some(cookie));
        } else {
            self.google_answer(call, 503, None, &pages::busy(&pending.then), None);
        }
    }
    #[expect(clippy::too_many_lines, reason = "each OAuth output is handled exhaustively at the composition boundary")]
    fn google_output(
        &mut self,
        google: &mut Google,
        exchange: Token,
        mut pending: Pending,
        events: &mut Queue<temper_engine_domain::Event>,
    ) {
        let Some(output) = pending.out.pop() else {
            google.keep_pending(exchange, pending);
            return;
        };
        match output {
            oauth::Request::Visit { url } => {
                if let Some(call) = pending.call.take() {
                    let set = cookie(
                        self.google_via(call) == mapping::Via::Public,
                        b"temper-sign-in",
                        b"/sign-in/google",
                        &pending.tag,
                        google.limits.deadline,
                        true,
                    );
                    self.google_answer(call, 303, Some(&url), &[], Some(set));
                }
                pending.phase = Phase::Visiting;
                google.keep_pending(exchange, pending);
            }
            oauth::Request::Http(request) => {
                google.http.push((exchange, request));
                google.keep_pending(exchange, pending);
            }
            oauth::Request::Tokens { record, missing: _ } => {
                let claims = match &record.id_token {
                    Some(token) => claims_from(token, &limits::client(google.limits).document),
                    None => None,
                };
                let Some(call) = pending.call else { return };
                let claims = claims.unwrap_or(Claims { email: None, verified: false, name: None });
                if claims.email.is_none() || !claims.verified {
                    self.google_answer(
                        call,
                        403,
                        None,
                        &pages::unverified(claims.email.as_deref(), &pending.then),
                        None,
                    );
                    return;
                }
                let email = claims.email.expect("verified address");
                let mut member = None;
                for configured in &google.settings.members {
                    if configured.address.eq_ignore_ascii_case(&email) {
                        member = Some(configured);
                        break;
                    }
                }
                let Some(member) = member else {
                    self.google_answer(call, 403, None, &pages::not_member(&email, &pending.then), None);
                    return;
                };
                let Some(subject) = record.subject else {
                    self.google_answer(call, 400, None, &pages::expired(&pending.then), None);
                    return;
                };
                if self.starting.is_some() || self.stopping.is_some() {
                    self.google_answer(call, 503, None, &pages::busy(&pending.then), None);
                    return;
                }
                let name = match claims.name {
                    Some(name)
                        if member.address.len().saturating_add(member.login.len()).saturating_add(name.len())
                            <= usize::try_from(google.identity_bytes).expect("identity bound") =>
                    {
                        name
                    }
                    Some(_) | None => member.login.clone(),
                };
                pending.client = None;
                pending.phase = Phase::Binding(people::Identity {
                    key: people::IdentityKey { provider: google.settings.provider, subject: member.address.clone() },
                    login: member.login.clone(),
                    name,
                });
                self.root(
                    temper_engine_domain::Event::Forge(Box::new(forge::Event::Bind {
                        request: exchange,
                        address: member.address.clone(),
                        subject,
                    })),
                    events,
                );
                google.keep_pending(exchange, pending);
            }
            oauth::Request::Failed { failure } => {
                if let Some(call) = pending.call {
                    let unreachable = match failure {
                        oauth::Failure::Unavailable
                        | oauth::Failure::TimedOut
                        | oauth::Failure::Unverified { check: oauth::Check::Keys } => true,
                        oauth::Failure::RateLimited { .. }
                        | oauth::Failure::Refused
                        | oauth::Failure::ClientRejected
                        | oauth::Failure::Malformed
                        | oauth::Failure::Incomplete
                        | oauth::Failure::Unverified { .. }
                        | oauth::Failure::InvalidRedirect
                        | oauth::Failure::Limit
                        | oauth::Failure::Cancelled => false,
                    };
                    if unreachable {
                        self.google_answer(call, 502, None, &pages::unreachable(&pending.then), None);
                    } else {
                        self.google_answer(call, 400, None, &pages::expired(&pending.then), None);
                    }
                }
            }
            oauth::Request::Revoked { .. } => unreachable!("sign-in never revokes"),
        }
    }
    #[expect(clippy::too_many_lines, reason = "one bounded pass joins the clients with their HTTP exchanges")]
    pub(super) fn google_advance(&mut self, events: &mut Queue<temper_engine_domain::Event>) {
        let Some(mut google) = self.google.take() else { return };
        let mut keys = List::with_capacity(google.limits.pending);
        for (key, _) in &google.pending {
            keys.push(*key).expect("bounded pending names");
        }
        for key in &keys {
            if events.room() == 0
                || self.out.room() < kit::MAX_OUT_DOWN
                || self.io.room() < 8
                || self.server_out.room() < 8
            {
                break;
            }
            let Some(mut pending) = google.pending.remove(key) else { unreachable!("pending name") };
            if self.env.now >= pending.deadline || self.stopping.is_some() {
                if let Some(call) = pending.call {
                    if self.stopping.is_some() {
                        self.google_answer(call, 503, None, &pages::busy(&pending.then), None);
                    } else {
                        self.google_answer(call, 400, None, &pages::expired(&pending.then), None);
                    }
                }
            } else {
                if pending.out.is_empty()
                    && let Some(client) = &mut pending.client
                {
                    client.step(oauth::Event::Tick { now: self.env.now }, &mut pending.out);
                }
                self.google_output(&mut google, *key, pending, events);
            }
        }
        for _ in 0..google.http.len() {
            let (exchange, request) = google.http.pop().expect("queued fetch");
            if self.stopping.is_none() && google.pending.contains_key(&exchange) {
                google.http.push((exchange, request));
            }
        }
        let env = Env { now: self.env.now, wall: self.env.wall, limits: limits::fetch(google.limits) };
        let mut keys = List::with_capacity(google.limits.fetches);
        for (key, _) in &google.fetches {
            keys.push(*key).expect("bounded fetch names");
        }
        for key in &keys {
            if self.io.room() < 8 {
                break;
            }
            let mut fetch = google.fetches.remove(key).expect("fetch name");
            if !google.pending.contains_key(&fetch.exchange) || self.stopping.is_some() {
                fetch.web.close(&env, true, &mut self.io);
            } else {
                fetch.web.progress(&env, &mut self.io);
            }
            if let Some(answer) = fetch.web.answer.take() {
                if let Some(pending) = google.pending.get_mut(&fetch.exchange)
                    && pending.out.is_empty()
                    && let Some(client) = &mut pending.client
                {
                    client.step(oauth::Event::Http(answer), &mut pending.out);
                }
                fetch.web.close(&env, true, &mut self.io);
            }
            if fetch.web.failure.take().is_some() {
                if let Some(pending) = google.pending.get_mut(&fetch.exchange)
                    && pending.out.is_empty()
                    && let Some(client) = &mut pending.client
                {
                    client.step(oauth::Event::Cancel, &mut pending.out);
                }
                fetch.web.close(&env, true, &mut self.io);
            }
            fetch.web.close_progress(&env, &mut self.io);
            if !fetch.web.closed() {
                google.keep_fetch(*key, fetch);
            }
        }
        #[cfg(test)]
        let scripted = google.scripted;
        #[cfg(not(test))]
        let scripted = false;
        if !scripted
            && google.fetches.len() < google.limits.fetches
            && self.io.room() >= 8
            && let Some((exchange, request)) = google.http.pop()
            && google.pending.contains_key(&exchange)
        {
            let mut endpoint = None;
            if let Some((origin, _, _, _)) = crate::startup::sign_in_url(&request.endpoint) {
                for configured in &google.endpoints {
                    if configured.origin.as_ref() == origin {
                        endpoint = Some(configured);
                        break;
                    }
                }
            }
            let id = request.id;
            let mut admitted = false;
            if let Some(endpoint) = endpoint {
                let plain = match endpoint.transport {
                    fetch::Transport::Plaintext => true,
                    fetch::Transport::Tls { .. } => false,
                };
                if (!plain || endpoint.address.ip().is_loopback())
                    && let Some(web) = fetch::Web::new(request, endpoint, &env.limits)
                {
                    let owner = Token::new(crate::tokens::SERVER | FETCH | google.next_fetch);
                    google.next_fetch = google.next_fetch.checked_add(1).expect("fetch sequence");
                    self.io.push(skein_io::Request::Connect { owner, addr: skein_io::Addr::Ip(endpoint.address) });
                    google.keep_fetch(owner, Fetch { exchange, web });
                    admitted = true;
                }
            }
            if !admitted
                && let Some(pending) = google.pending.get_mut(&exchange)
                && let Some(client) = &mut pending.client
            {
                client.step(
                    oauth::Event::Http(oauth::HttpResponse {
                        id,
                        status: 0,
                        body: Box::new([]),
                        retry_after: Duration::ZERO,
                        evidence: oauth::HttpEvidence::Unsent,
                        now: self.env.now,
                        wall: self.env.wall,
                    }),
                    &mut pending.out,
                );
            }
        }
        self.google = Some(google);
    }
    pub(super) fn google_up(&mut self, event: skein_io::Event) {
        let owner = super::event_owner(&event);
        if let Some(google) = &mut self.google
            && let Some(owner) = owner
            && let Some(fetch) = google.fetches.get_mut(&owner)
        {
            let env = Env { now: self.env.now, wall: self.env.wall, limits: limits::fetch(google.limits) };
            fetch.web.up(&env, event, &mut self.io);
        }
    }
    /// Runtime endpoints are resolved before the loop, and never learned from a redirect.
    pub(crate) fn google_endpoints(&mut self, endpoints: List<fetch::Endpoint>) {
        if let Some(google) = &mut self.google {
            google.endpoints = endpoints;
        }
    }
    #[cfg(test)]
    pub(crate) fn google_http(&mut self) -> Option<(Token, oauth::HttpRequest)> {
        self.google.as_mut()?.http.pop()
    }
    #[cfg(test)]
    pub(crate) fn google_http_answer(&mut self, exchange: Token, answer: oauth::HttpResponse) {
        if let Some(google) = &mut self.google
            && let Some(pending) = google.pending.get_mut(&exchange)
            && let Some(client) = &mut pending.client
        {
            client.step(oauth::Event::Http(answer), &mut pending.out);
        }
    }
}
impl Google {
    fn keep_pending(&mut self, exchange: Token, pending: Pending) {
        match self.pending.insert(exchange, pending) {
            Ok(_) => {}
            Err(_) => unreachable!("pending sign-in admitted"),
        }
    }
    fn keep_fetch(&mut self, owner: Token, fetch: Fetch) {
        match self.fetches.insert(owner, fetch) {
            Ok(_) => {}
            Err(_) => unreachable!("fetch admitted"),
        }
    }
}
impl Clients {
    pub(crate) fn configure_google(
        &mut self,
        sign_in: &crate::settings::SignIn,
        limits: crate::limits::SignIn,
        seeds: &crate::settings::Seeds,
        config: &jig_protocol_config::Engine,
    ) {
        self.google = Some(Google::new(
            sign_in,
            limits,
            seeds,
            self.env.limits.identity_bytes.min(config.limits.core.people.identity_bytes),
        ));
    }
}
pub(super) fn member_login(sign_in: &crate::settings::SignIn, address: &[u8]) -> Option<Box<[u8]>> {
    for member in &sign_in.members {
        if member.address.as_ref() == address {
            return Some(member.login.clone());
        }
    }
    None
}
#[cfg(test)]
mod tests;
