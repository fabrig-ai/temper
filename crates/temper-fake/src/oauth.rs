//! Standalone auto-grant issuer (protocol/setup.md, 7.5).
//! Decoding and plan construction do no io; the binary drives the existing peer.
use crate::{Refusal, credential, read};
use alloc::boxed::Box;
use core::net::SocketAddr;
use jig_protocol_config::{self as config, Path};
use skein_fake_oauth as issuer;
use skein_fake_peers::{Transport, oauth::Peer};
use skein_lib::{Decimal, Duration};

pub const DOCUMENT_BYTES: u32 = 16_384;
pub const URI_BYTES: u32 = 1024;
/// Used authorization codes remain in the primitive until issuer restart.
pub const CODE_SLOTS: u32 = 64;
/// Two connections, each with at most one token request per peer pass.
pub const PLAN_SLOTS: u32 = 2;
const LIFETIME: u64 = 86_400;
const REFRESH: &[u8] = b"fake-refresh";
const SCOPES: &[u8] = b"openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";

/// A checked loopback issuer and callback, with the shipped registration fixed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Document {
    listen: SocketAddr,
    origin: Box<[u8]>,
    redirect_uri: Box<[u8]>,
}
impl Document {
    pub fn decode(bytes: &[u8]) -> Result<Self, Refusal> {
        let d = match config::document(bytes, &config::DocumentLimits { bytes: DOCUMENT_BYTES, tokens: 64, depth: 8 }) {
            Ok(d) => d,
            Err(p) => return Err(read::kind(&p.path, p.what)),
        };
        let p = Path::root();
        let mut o = read::object(&d, 0, &p)?;
        let listen_at = read::required(&mut o, &d, b"listen", &p)?;
        let port_at = o.member(&d, b"redirect_port");
        let path_at = o.member(&d, b"redirect_path");
        read::finish(o, &d, &p)?;
        let lp = p.member(b"listen");
        let mut socket = read::object(&d, listen_at, &lp)?;
        let text = read::field(&mut socket, &d, b"socket", URI_BYTES, &lp)?;
        read::finish(socket, &d, &lp)?;
        let listen = read::listen(&d, listen_at, &lp)?;
        if listen.port() == 0 {
            return Err(read::why(&lp, b"must have a nonzero port"));
        }
        let port = match port_at {
            Some(at) => read::integer(&d, at, u64::from(u16::MAX), &p.member(b"redirect_port"))?,
            None => 1455,
        };
        if port == 0 {
            return Err(read::why(&p.member(b"redirect_port"), b"must have a nonzero port"));
        }
        let path = match path_at {
            Some(at) => read::text(&d, at, URI_BYTES, &p.member(b"redirect_path"))?,
            None => b"auth/callback".as_slice().into(),
        };
        if path.is_empty() || path.starts_with(b"/") {
            return Err(read::why(&p.member(b"redirect_path"), b"expected a non-empty relative URI path"));
        }
        let mut escaped = 0_u8;
        for byte in &path {
            if escaped > 0 {
                if !byte.is_ascii_hexdigit() {
                    return Err(read::why(&p.member(b"redirect_path"), b"invalid URI escape"));
                }
                escaped = escaped.saturating_sub(1);
            } else if *byte == b'%' {
                escaped = 2;
            } else if !byte.is_ascii_alphanumeric() && !b"-._~!$&'()*+,;=:@/".contains(byte) {
                return Err(read::why(&p.member(b"redirect_path"), b"expected a non-empty relative URI path"));
            }
        }
        if escaped != 0 {
            return Err(read::why(&p.member(b"redirect_path"), b"invalid URI escape"));
        }
        let origin = read::joined(&[b"http://", &text]);
        // This is the longest derived path, including the primitive's metadata route.
        let issuer_bytes =
            origin.len().checked_add(b"/.well-known/openid-configuration".len()).expect("bounded socket");
        if issuer_bytes > usize::try_from(URI_BYTES).expect("URI bound") {
            return Err(read::why(&lp, b"issuer URI past its bound"));
        }
        let redirect_uri = read::joined(&[b"http://127.0.0.1:", Decimal::of(port).as_bytes(), b"/", &path]);
        if redirect_uri.len() > usize::try_from(URI_BYTES).expect("URI bound") {
            return Err(read::why(&p.member(b"redirect_path"), b"callback URI past its bound"));
        }
        Ok(Self { listen, origin, redirect_uri })
    }

    /// No observation stream is retained by the long-running standalone issuer.
    pub fn peer(&self, seed: u64) -> Result<Peer, skein_fake_peers::Error> {
        let profile = config::profile::profile(config::profile::Profile::Small);
        let limits = limits();
        let Some(response) = skein_fake_peers::oauth::least_response(&limits) else {
            return Err(skein_fake_peers::Error::Limits);
        };
        let mut peer = Peer::new(
            self.listen,
            Transport::Plaintext,
            skein_fake_peers::Limits {
                io: profile.io,
                connections: PLAN_SLOTS,
                queue: 64,
                plaintext: 131_072,
                ciphertext: 131_072,
                observations: 0,
                observation_bytes: 0,
            },
            self.config(),
            limits,
            skein_http::server::Limits { response, body: u64::from(limits.request_bytes), ..profile.driver.server },
        )?
        .without_records();
        match refill(&mut peer, seed) {
            Ok(()) => Ok(peer),
            Err(issuer::Error::Limits | issuer::Error::Full | issuer::Error::Plan) => {
                Err(skein_fake_peers::Error::Limits)
            }
        }
    }

    fn config(&self) -> issuer::Config {
        issuer::Config {
            authorization_url: read::joined(&[&self.origin, b"/api/accounts/authorize"]),
            token_endpoint: read::joined(&[&self.origin, b"/api/accounts/oauth/token"]),
            client_id: b"dynamic_agent_client".as_slice().into(),
            client_secret: None,
            redirect_uri: self.redirect_uri.clone(),
            refresh_token: REFRESH.into(),
            terms: Some(issuer::Terms {
                issued: Box::new([b"issued-client".as_slice().into()]),
                omit_issued: false,
                varying_port: true,
                revocation_endpoint: Some(read::joined(&[&self.origin, b"/api/accounts/oauth/revoke"])),
                openid: Some(issuer::OpenId {
                    issuer: self.origin.clone(),
                    key_set_url: read::joined(&[&self.origin, b"/.well-known/jwks.json"]),
                    subject: b"fake-person".as_slice().into(),
                    lifetime: Duration::from_secs(LIFETIME),
                    header_kid: true,
                    fault: None,
                }),
            }),
        }
    }
}

fn limits() -> issuer::Limits {
    let profile = config::profile::profile(config::profile::Profile::Small);
    issuer::Limits {
        document: profile.driver.client.document,
        uri_bytes: URI_BYTES,
        request_bytes: 4096,
        codes: CODE_SLOTS,
        rotations: 2,
        carried: 5,
        plans: PLAN_SLOTS,
    }
}

fn plan(seed: u64) -> issuer::Plan {
    let access = credential::credential(seed, credential::added_key(0));
    issuer::Plan {
        status: 200,
        body: issuer::Body::Token(skein_oauth::TokenResponse {
            access_token: access.as_slice().into(),
            refresh_token: Some(REFRESH.into()),
            expires_in: LIFETIME,
            scope: Some(SCOPES.into()),
            id_token: None,
            earliest_refresh_at: None,
        }),
        delay: Duration::ZERO,
        retry_after: Duration::ZERO,
    }
}

/// Top up before each peer pass. Full alone means the bounded queue needs no more;
/// malformed plans/configuration remain errors. Revoking REFRESH requires restart.
pub fn refill(peer: &mut Peer, seed: u64) -> Result<(), issuer::Error> {
    for _ in 0..PLAN_SLOTS {
        match peer.queue(plan(seed)) {
            Ok(()) => {}
            Err(issuer::Error::Full) => return Ok(()),
            Err(why @ (issuer::Error::Limits | issuer::Error::Plan)) => return Err(why),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
