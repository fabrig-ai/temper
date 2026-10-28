//! The real loop's synthetic issuer and browser (domain/testing.md, 2.4; skein's oauth.md, 6.5).
use skein_fake_peers::{Transport, oauth};
use skein_lib::Duration;

pub(crate) fn peer(address: std::net::SocketAddr, registration: &serde_json::Value) -> oauth::Peer {
    let bytes =
        |member: &str| -> Box<[u8]> { registration[member].as_str().expect("registration member").as_bytes().into() };
    let profile = jig_protocol_config::profile::profile(jig_protocol_config::profile::Profile::Small);
    let limits = skein_fake_oauth::Limits {
        document: profile.driver.client.document,
        uri_bytes: 1024,
        request_bytes: 4096,
        codes: 1,
        rotations: 2,
        carried: 5,
        plans: 1,
    };
    let mut peer = oauth::Peer::new(
        address,
        Transport::Plaintext,
        skein_fake_peers::Limits {
            io: profile.io,
            connections: 2,
            queue: 64,
            plaintext: 131_072,
            ciphertext: 131_072,
            observations: 64,
            observation_bytes: 4096,
        },
        skein_fake_oauth::Config {
            authorization_url: bytes("authorization"),
            token_endpoint: bytes("token"),
            client_id: bytes("client"),
            client_secret: None,
            redirect_uri: b"http://127.0.0.1:0/auth/callback".as_slice().into(),
            refresh_token: b"fake-refresh".as_slice().into(),
            terms: Some(skein_fake_oauth::Terms {
                issued: Box::new([b"issued-client".as_slice().into()]),
                omit_issued: false,
                varying_port: true,
                revocation_endpoint: Some(bytes("revocation")),
                openid: Some(skein_fake_oauth::OpenId {
                    issuer: bytes("issuer"),
                    key_set_url: bytes("keys"),
                    subject: b"real-loop-person".as_slice().into(),
                    lifetime: Duration::from_secs(300),
                    header_kid: true,
                    fault: None,
                }),
            }),
        },
        limits,
        skein_http::server::Limits {
            response: oauth::least_response(&limits).expect("issuer response"),
            body: u64::from(limits.request_bytes),
            ..profile.driver.server
        },
    )
    .expect("local issuer");
    let scopes = registration["scopes"]
        .as_array()
        .expect("shipped scopes")
        .iter()
        .map(|scope| scope.as_str().expect("scope"))
        .collect::<Vec<_>>()
        .join(" ");
    peer.queue(skein_fake_oauth::Plan {
        status: 200,
        body: skein_fake_oauth::Body::Token(skein_oauth::TokenResponse {
            access_token: b"real-loop-access".as_slice().into(),
            refresh_token: Some(b"fake-refresh".as_slice().into()),
            expires_in: 300,
            scope: Some(scopes.into_bytes().into()),
            id_token: None,
            earliest_refresh_at: None,
        }),
        delay: Duration::ZERO,
        retry_after: Duration::ZERO,
    })
    .expect("one fake grant");
    peer
}

/// The shipped `OpenAI` way, with every issuer endpoint on the local fake and
/// no preferred callback port: the accounts driver binds port zero.
pub(crate) fn registration(address: std::net::SocketAddr) -> serde_json::Value {
    let origin = format!("http://{address}");
    serde_json::json!({
        "issuer":origin, "client":"dynamic_agent_client", "name_hint":"temper",
        "authorization":format!("{origin}/api/accounts/authorize"),
        "token":format!("{origin}/api/accounts/oauth/token"),
        "revocation":format!("{origin}/api/accounts/oauth/revoke"),
        "keys":format!("{origin}/.well-known/jwks.json"),
        "transport":"plaintext",
        "scopes":["openid","profile","email","offline_access","resource.invoke","chatgpt.tokens.use.direct"],
        "must_grant":["chatgpt.tokens.use.direct"],
        "resource":format!("{origin}/v1"), "redirect_path":"auth/callback"
    })
}

/// A native listener answer. A listener already closed can refuse the connection.
/// No request address or response headers are retained here.
pub struct Returned {
    pub status: Option<u16>,
    pub body: Box<[u8]>,
    pub error: Option<skein_io::kernel::Error>,
}

use skein_io::kernel::{Complete, Done, Family, Op};
use skein_lib::Secret;
use std::net::SocketAddr;

/// One unfollowed GET on the world's ring. Deliberately has no Debug: the
/// request and an authorization answer's Location carry a secret in transit.
pub(crate) struct Visit {
    address: SocketAddr,
    target: Box<[u8]>,
    bytes: Vec<u8>,
    pub(crate) redirect: Option<Secret>,
    pub(crate) answer: Returned,
    authorize: bool,
    pub(crate) done: bool,
}
impl Visit {
    const RESPONSE: usize = 65_536;
    pub(crate) const URI: usize = 1024;

    pub(crate) fn new(url: &[u8], authorize: bool) -> Self {
        assert!(url.len() <= Self::URI, "bounded sign-in address: {} bytes", url.len());
        let rest = url.strip_prefix(b"http://").expect("local fake HTTP address");
        let at = rest.iter().position(|byte| *byte == b'/').expect("sign-in path");
        let address: SocketAddr =
            std::str::from_utf8(&rest[..at]).expect("numeric HTTP authority").parse().expect("loopback socket");
        assert_eq!(address.ip(), std::net::Ipv4Addr::LOCALHOST, "sign-in stays on IPv4 loopback");
        assert_ne!(address.port(), 0, "kernel-assigned sign-in port");
        assert!(rest[at..].iter().all(|byte| (0x21..=0x7e).contains(byte)), "visible HTTP target");
        Self {
            address,
            target: rest[at..].into(),
            bytes: Vec::with_capacity(Self::RESPONSE),
            redirect: None,
            answer: Returned { status: None, body: Box::new([]), error: None },
            authorize,
            done: false,
        }
    }
    pub(crate) fn start(&self) -> Op {
        Op::Socket { family: Family::of(&self.address) }
    }
    pub(crate) fn complete(&mut self, complete: Complete) -> Option<Op> {
        // Never format an operation: Send and Recv hold the kept secret's bytes.
        let done = match complete.result {
            Ok(done) => done,
            Err(error) => {
                assert!(!self.authorize, "local fake issuer operation failed");
                self.answer.error = Some(error);
                let Op::Connect { fd, .. } = complete.kind else { panic!("listener GET operation failed") };
                return Some(Op::Close { fd });
            }
        };
        match (complete.kind, done) {
            (Op::Socket { .. }, Done::Fd(fd)) => Some(Op::Connect { fd, addr: self.address }),
            (Op::Connect { fd, .. }, Done::Nothing) => {
                let mut bytes = b"GET ".to_vec();
                bytes.extend_from_slice(&self.target);
                bytes.extend_from_slice(
                    format!(" HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", self.address).as_bytes(),
                );
                Some(Op::Send { fd, bytes: bytes.into(), from: 0 })
            }
            (Op::Send { fd, bytes, from }, Done::Count(count)) => {
                assert!(count > 0, "sign-in GET send progresses");
                let from = from + count;
                Some(if from as usize == bytes.len() {
                    Op::Recv { fd, buf: vec![0; 4096].into() }
                } else {
                    Op::Send { fd, bytes, from }
                })
            }
            (Op::Recv { fd, buf }, Done::Count(count)) => {
                assert!(self.bytes.len() + count as usize <= Self::RESPONSE, "bounded sign-in answer");
                self.bytes.extend_from_slice(&buf[..count as usize]);
                if self.response() {
                    Some(Op::Close { fd })
                } else {
                    assert!(count > 0, "sign-in answer ended before its Content-Length");
                    Some(Op::Recv { fd, buf })
                }
            }
            (Op::Close { .. }, Done::Nothing) => {
                self.done = true;
                None
            }
            _ => panic!("unexpected sign-in HTTP completion"),
        }
    }
    fn response(&mut self) -> bool {
        let Some(end) = self.bytes.windows(4).position(|part| part == b"\r\n\r\n") else { return false };
        let header = std::str::from_utf8(&self.bytes[..end]).expect("HTTP header encoding");
        let mut lines = header.split("\r\n");
        let status: u16 = lines
            .next()
            .expect("HTTP status")
            .split_whitespace()
            .nth(1)
            .expect("HTTP status code")
            .parse()
            .expect("numeric HTTP status");
        let mut length = None;
        let mut location = None;
        for line in lines {
            let (name, value) = line.split_once(':').expect("HTTP header member");
            if name.eq_ignore_ascii_case("content-length") {
                length = Some(value.trim().parse::<usize>().expect("HTTP body length"));
            } else if name.eq_ignore_ascii_case("location") {
                assert!(location.is_none(), "one redirect Location");
                location = Some(value.trim().as_bytes());
            }
        }
        let length = length.expect("sign-in peers frame with Content-Length");
        assert!(end + 4 + length <= Self::RESPONSE, "bounded framed sign-in answer");
        if self.bytes.len() < end + 4 + length {
            return false;
        }
        if self.authorize {
            assert_eq!(status, 302, "fake issuer approves immediately");
            let location = location.expect("issuer redirect Location");
            assert!(location.len() <= Self::URI, "bounded kept redirect: {} bytes", location.len());
            self.redirect = Some(Secret::new(location.into()));
        } else {
            self.answer.status = Some(status);
            self.answer.body = self.bytes[end + 4..end + 4 + length].into();
        }
        true
    }
}
