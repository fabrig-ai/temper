//! Google's routes against the pinned fake issuer at the fetch record boundary.
//! Accounts and signed claims pass through the issuer, OAuth client, root binding, and jig mint.
#![expect(
    clippy::disallowed_macros,
    clippy::disallowed_methods,
    reason = "ordinary tests construct and inspect protocol fixtures"
)]
#![expect(clippy::disallowed_types, reason = "ordinary tests build textual requests")]
use super::fixtures::{self, http::Peer};
use crate::{clients::Clients, settings};
use skein_fake_oauth as fake;
use skein_lib::{Duration, List, Queue, Secret, Time, Wall};
use skein_oauth as oauth;
use temper_engine_domain_forge as forge;
const NOW: Time = Time::from_nanos(1);
const WALL: Wall = Wall::from_nanos(1_000_000_000);
const PUBLIC: &[u8] = b"temper.example.com";
pub(crate) fn configured() -> settings::SignIn {
    settings::SignIn {
        forges: Box::new([]),
        provider: 3,
        client: b"temper-test".as_slice().into(),
        secret: Secret::new(b"throwaway-client-secret".as_slice().into()),
        issuer: b"http://127.0.0.1:31999".as_slice().into(),
        authorization: b"http://127.0.0.1:31999/authorize".as_slice().into(),
        token: b"http://127.0.0.1:31999/token".as_slice().into(),
        keys: b"http://127.0.0.1:31999/keys".as_slice().into(),
        transport: None,
        members: Box::new([crate::startup::application::sign_in::Member {
            address: b"sam@example.com".as_slice().into(),
            login: b"sam".as_slice().into(),
        }]),
    }
}
pub(crate) fn world(pending: u32) -> (crate::Service, fake::Issuer) {
    world_lifetime(pending, Duration::from_secs(600))
}
fn world_lifetime(pending: u32, lifetime: Duration) -> (crate::Service, fake::Issuer) {
    world_origin(pending, lifetime, true)
}
fn world_origin(pending: u32, lifetime: Duration, public: bool) -> (crate::Service, fake::Issuer) {
    world_settings(pending, lifetime, public, None, &[])
}
pub(crate) fn world_settings(
    pending: u32,
    lifetime: Duration,
    public: bool,
    provider: Option<jig_protocol_config::clients::Provider>,
    files: &[(Box<[u8]>, Secret)],
) -> (crate::Service, fake::Issuer) {
    let mut service = fixtures::ready();
    let mut config = fixtures::engine();
    let settings = configured();
    config.clients.public = if public {
        Some(crate::startup::application::Origin {
            scheme: crate::startup::application::Scheme::Https,
            host: PUBLIC.into(),
            port: None,
            name: PUBLIC.into(),
        })
    } else {
        None
    };
    let mut providers = List::with_capacity(3);
    for provider in config.jig.providers {
        providers.push(provider).unwrap();
    }
    providers
        .push(jig_protocol_config::Provider { number: 3, kind: jig_protocol_config::ProviderKind::Application })
        .unwrap();
    config.jig.providers = providers.into_boxed();
    config.jig.clients.provider = provider;
    let room = temper_engine_domain::max_out(&service.engine.as_ref().unwrap().env.limits)
        .checked_add(1)
        .unwrap()
        .checked_mul(jig_protocol_engine_clients::MAX_OUT_DOWN)
        .unwrap();
    let mut clients =
        Clients::new(&config.jig, &config.clients, &fixtures::seeds(), room, &config.limits.root.journal, None, files)
            .unwrap();
    let mut limits = config.limits.sign_in;
    limits.pending = pending;
    limits.deadline = lifetime;
    clients.configure_google(&settings, limits, &fixtures::seeds(), &config.jig);
    clients.google.as_mut().unwrap().scripted = true;
    clients.starting = None;
    service.engine.as_mut().unwrap().clients = clients;
    fixtures::listening(&mut service);
    let document = oauth_limits();
    let issuer = fake::Issuer::new(
        fake::Config {
            authorization_url: settings.authorization,
            token_endpoint: settings.token,
            client_id: settings.client,
            client_secret: Some(settings.secret.expose().into()),
            redirect_uri: if public {
                b"https://temper.example.com/sign-in/google/return".as_slice().into()
            } else {
                b"http://127.0.0.1:7100/sign-in/google/return".as_slice().into()
            },
            refresh_token: b"throwaway-refresh".as_slice().into(),
            terms: Some(fake::Terms {
                issued: Box::new([]),
                omit_issued: false,
                revocation_endpoint: None,
                varying_port: false,
                openid: Some(fake::OpenId {
                    issuer: settings.issuer,
                    key_set_url: settings.keys,
                    subject: b"google-sub".as_slice().into(),
                    lifetime: Duration::from_secs(3600),
                    header_kid: true,
                    fault: None,
                }),
            }),
        },
        fake::Limits { document, uri_bytes: 2048, request_bytes: 16_384, codes: 4, rotations: 4, plans: 4, carried: 4 },
    )
    .unwrap();
    (service, issuer)
}
fn oauth_limits() -> oauth::Limits {
    oauth::Limits {
        document_bytes: 65_536,
        string_bytes: 16_384,
        token_bytes: 8192,
        client_bytes: 512,
        issued_client_bytes: 256,
        granted_bytes: 256,
        subject_bytes: 255,
        id_token_bytes: 16_384,
        detail_bytes: 256,
        record_bytes: 65_536,
        depth: 16,
        tokens: 2048,
    }
}
pub(crate) fn peer(service: &mut crate::Service, target: &[u8], cookie: Option<&[u8]>) -> Peer {
    let cookie = cookie.map_or(String::new(), |value| format!("Cookie: {}\r\n", core::str::from_utf8(value).unwrap()));
    Peer::raw(
        service.engine.as_mut().unwrap(),
        format!(
            "GET {} HTTP/1.1\r\nHost: temper.example.com\r\n{cookie}Connection: close\r\n\r\n",
            core::str::from_utf8(target).unwrap()
        )
        .as_bytes(),
    )
}
pub(crate) fn pump(service: &mut crate::Service, peer: &mut Peer) {
    peer.pump_at(service.engine.as_mut().unwrap(), true, NOW, WALL);
}
fn answer_fetch(service: &mut crate::Service, issuer: &mut fake::Issuer) {
    let clients = &mut service.engine.as_mut().unwrap().clients;
    let (exchange, request) = clients.google_http().expect("issuer request");
    let event = match request.method {
        oauth::Method::Get => fake::Event::Get { id: request.id, endpoint: request.endpoint, now: NOW, wall: WALL },
        oauth::Method::Post => fake::Event::Post { request, now: NOW, wall: WALL },
    };
    let mut out = Queue::with_capacity(fake::MAX_OUT);
    issuer.step(event, &mut out);
    let Some(fake::Request::Http(answer)) = out.pop() else { panic!("issuer's HTTP answer") };
    clients.google_http_answer(exchange, answer);
}
pub(crate) fn header<'a>(peer: &'a Peer, name: &str) -> &'a [u8] {
    let text = core::str::from_utf8(peer.received()).unwrap();
    for line in text.split("\r\n") {
        if let Some((key, value)) = line.split_once(": ")
            && key.eq_ignore_ascii_case(name)
        {
            return value.as_bytes();
        }
    }
    panic!("missing response field {name}")
}
pub(crate) fn start(service: &mut crate::Service, issuer: &mut fake::Issuer, target: &[u8]) -> (Box<[u8]>, Box<[u8]>) {
    let mut peer = peer(service, target, None);
    pump(service, &mut peer);
    answer_fetch(service, issuer);
    pump(service, &mut peer);
    assert!(peer.received().starts_with(b"HTTP/1.1 303"));
    let cookie: Box<[u8]> = header(&peer, "set-cookie").into();
    assert!(cookie.ends_with(b"; Secure"));
    assert!(
        cookie
            .windows(b"HttpOnly; SameSite=Lax; Max-Age=".len())
            .any(|part| part == b"HttpOnly; SameSite=Lax; Max-Age=")
    );
    assert_eq!(header(&peer, "cache-control"), b"no-store");
    assert!(peer.is_closed(), "a bodyless redirect completes its HTTP exchange");
    (header(&peer, "location").into(), cookie.split(|byte| *byte == b';').next().unwrap().into())
}
fn redirect(issuer: &mut fake::Issuer, url: Box<[u8]>) -> Box<[u8]> {
    let mut out = Queue::with_capacity(fake::MAX_OUT);
    issuer.step(fake::Event::Authorize { url, now: NOW }, &mut out);
    let Some(fake::Request::Redirect { state, code, .. }) = out.pop() else { panic!("issuer redirect") };
    format!(
        "/sign-in/google/return?state={}&code={}",
        core::str::from_utf8(&state).unwrap(),
        core::str::from_utf8(&code).unwrap()
    )
    .into_bytes()
    .into_boxed_slice()
}
pub(crate) fn ending(peer: &Peer, status: &[u8], text: &[u8]) {
    assert!(peer.received().starts_with(status));
    if !text.is_empty() {
        assert!(peer.received().windows(text.len()).any(|part| part == text));
    }
    assert_eq!(header(peer, "cache-control"), b"no-store");
    assert_eq!(header(peer, "referrer-policy"), b"no-referrer");
    let cleared = b"temper-sign-in=; Path=/sign-in/google; HttpOnly; SameSite=Lax; Max-Age=0";
    assert!(peer.received().windows(cleared.len()).any(|part| part == cleared));
}
pub(crate) fn account(issuer: &mut fake::Issuer, subject: &[u8], address: Option<&[u8]>, verified: bool) {
    issuer
        .sign_in_as(fake::Account {
            subject: subject.into(),
            email: address.map(|address| fake::Email { address: address.into(), verified }),
            name: Some(b"Sam".as_slice().into()),
        })
        .unwrap();
}
fn token_plan(issuer: &mut fake::Issuer) {
    issuer
        .queue(fake::Plan {
            status: 200,
            body: fake::Body::Token(oauth::TokenResponse {
                access_token: b"throwaway-access".as_slice().into(),
                refresh_token: None,
                expires_in: 3600,
                scope: Some(b"openid email profile".as_slice().into()),
                id_token: None,
                earliest_refresh_at: None,
            }),
            delay: Duration::ZERO,
            retry_after: Duration::ZERO,
        })
        .unwrap();
}
pub(crate) fn signed_return(service: &mut crate::Service, issuer: &mut fake::Issuer, target: &[u8]) -> Peer {
    let (url, cookie) = start(service, issuer, target);
    let redirect = redirect(issuer, url);
    token_plan(issuer);
    let mut peer = peer(service, &redirect, Some(&cookie));
    pump(service, &mut peer);
    answer_fetch(service, issuer);
    pump(service, &mut peer);
    peer
}
#[test]
fn a_signed_member_mints_twice_and_keeps_the_binding_against_another_subject() {
    let (mut service, mut issuer) = world(2);
    account(&mut issuer, b"first-sub", Some(b"SAM@example.com"), true);
    for _ in 0_u32..2 {
        let peer = signed_return(&mut service, &mut issuer, b"/sign-in/google?then=%2Fprojects%2F1");
        ending(&peer, b"HTTP/1.1 303", b"");
        assert_eq!(header(&peer, "location"), b"/projects/1");
        let response = core::str::from_utf8(peer.received()).unwrap();
        assert!(response.contains("jig-sign-in="));
        assert!(response.contains("SameSite=Strict"));
        assert!(response.contains("Max-Age=0; Secure"));
    }
    account(&mut issuer, b"another-sub", Some(b"sam@example.com"), true);
    let peer = signed_return(&mut service, &mut issuer, b"/sign-in/google");
    ending(&peer, b"HTTP/1.1 403", b"signs in here with another Google account");
    let rows = fixtures::load(&mut service.engine.as_mut().unwrap().store, temper_engine_domain::Range::Forge);
    assert!(rows.iter().any(|row| match row {
        temper_engine_domain::Record::Forge { row } => {
            if let forge::Stored::Binding { address, subject } = row.as_ref() {
                address.as_ref() == b"sam@example.com" && subject.as_ref() == b"first-sub"
            } else {
                false
            }
        }
        temper_engine_domain::Record::Core(_) | temper_engine_domain::Record::Delivery { .. } => false,
    }));
}
#[test]
fn signed_nonmembers_missing_email_and_unverified_email_mint_nothing() {
    for (address, verified, text) in [
        (Some(b"other@example.com".as_slice()), true, b"other@example.com can&#39;t sign in here".as_slice()),
        (None, true, b"Google hasn&#39;t verified this account&#39;s address"),
        (Some(b"sam@example.com".as_slice()), false, b"Google hasn&#39;t verified sam@example.com"),
    ] {
        let (mut service, mut issuer) = world(2);
        account(&mut issuer, b"first-sub", address, verified);
        let peer = signed_return(&mut service, &mut issuer, b"/sign-in/google");
        ending(&peer, b"HTTP/1.1 403", text);
        assert!(!core::str::from_utf8(peer.received()).unwrap().contains("jig-sign-in="));
    }
}
#[test]
fn a_signed_member_with_a_bad_then_returns_home_and_mint_refused_is_busy() {
    let (mut service, mut issuer) = world(2);
    account(&mut issuer, b"first-sub", Some(b"sam@example.com"), true);
    for _ in 0_u32..2 {
        let peer = signed_return(&mut service, &mut issuer, b"/sign-in/google?then=%2F%2Fevil.example");
        ending(&peer, b"HTTP/1.1 303", b"");
        assert_eq!(header(&peer, "location"), b"/");
    }
    let peer = signed_return(&mut service, &mut issuer, b"/sign-in/google");
    ending(&peer, b"HTTP/1.1 503", b"temper couldn&#39;t sign you in just now");
    assert!(!core::str::from_utf8(peer.received()).unwrap().contains("jig-sign-in="));
}
#[test]
fn choosing_an_account_and_cancelling_keep_the_safe_return() {
    for error in ["access_denied", "another_error"] {
        let (mut service, mut issuer) = world(2);
        let (url, cookie) =
            start(&mut service, &mut issuer, b"/sign-in/google?then=%2Fprojects%3Ftab%3Dmine&account=choose");
        assert!(url.windows(b"prompt=select_account".len()).any(|part| part == b"prompt=select_account"));
        let mut peer =
            peer(&mut service, format!("/sign-in/google/return?error={error}&state=s").as_bytes(), Some(&cookie));
        pump(&mut service, &mut peer);
        ending(
            &peer,
            b"HTTP/1.1 400",
            if error == "access_denied" {
                b"You cancelled at Google. Nothing was kept."
            } else {
                b"Google answered another_error. Nothing was kept."
            },
        );
        assert!(
            peer.received()
                .windows(b"then=%2Fprojects%3Ftab%3Dmine".len())
                .any(|part| part == b"then=%2Fprojects%3Ftab%3Dmine")
        );
    }
}
#[test]
fn absent_unknown_used_and_restarted_tags_expire() {
    let (mut service, mut issuer) = world(2);
    for cookie in [None, Some(b"temper-sign-in=AAAAAAAAAAAAAAAAAAAAAA".as_slice())] {
        let mut peer = peer(&mut service, b"/sign-in/google/return?state=s&code=c", cookie);
        pump(&mut service, &mut peer);
        ending(&peer, b"HTTP/1.1 400", b"It took too long");
    }
    let (_, cookie) = start(&mut service, &mut issuer, b"/sign-in/google");
    let (mut restarted, _) = world(2);
    let mut after = peer(&mut restarted, b"/sign-in/google/return?state=s&code=c", Some(&cookie));
    pump(&mut restarted, &mut after);
    ending(&after, b"HTTP/1.1 400", b"It took too long");
    let mut cancelled = peer(&mut service, b"/sign-in/google/return?error=access_denied", Some(&cookie));
    pump(&mut service, &mut cancelled);
    let mut reused = peer(&mut service, b"/sign-in/google/return?code=c&state=s", Some(&cookie));
    pump(&mut service, &mut reused);
    ending(&reused, b"HTTP/1.1 400", b"It took too long");
}
#[test]
fn a_wrong_state_and_a_past_deadline_expire() {
    for late in [false, true] {
        let (mut service, mut issuer) = world_lifetime(2, Duration::from_secs(1));
        let (_, cookie) = start(&mut service, &mut issuer, b"/sign-in/google");
        let now = if late { Time::from_nanos(2_000_000_000) } else { NOW };
        let mut peer = peer(&mut service, b"/sign-in/google/return?code=c&state=wrong", Some(&cookie));
        peer.pump_at(service.engine.as_mut().unwrap(), true, now, WALL);
        ending(&peer, b"HTTP/1.1 400", b"It took too long");
    }
}
#[test]
fn pending_full_is_busy_and_unreachable_keys_end_before_the_visit() {
    let (mut service, mut issuer) = world(1);
    let _started = start(&mut service, &mut issuer, b"/sign-in/google");
    let mut busy = peer(&mut service, b"/sign-in/google", None);
    pump(&mut service, &mut busy);
    assert!(busy.received().starts_with(b"HTTP/1.1 503"));
    let (mut service, _) = world(1);
    let mut unreachable = peer(&mut service, b"/sign-in/google", None);
    pump(&mut service, &mut unreachable);
    let clients = &mut service.engine.as_mut().unwrap().clients;
    let (exchange, request) = clients.google_http().unwrap();
    clients.google_http_answer(
        exchange,
        oauth::HttpResponse {
            id: request.id,
            status: 0,
            body: Box::new([]),
            retry_after: Duration::ZERO,
            evidence: oauth::HttpEvidence::Unsent,
            now: NOW,
            wall: WALL,
        },
    );
    pump(&mut service, &mut unreachable);
    assert!(unreachable.received().starts_with(b"HTTP/1.1 502"));
}
#[test]
fn disabled_routes_and_wrong_methods_carry_the_policy_and_clear_returns() {
    for (method, path, disabled, expected) in [
        ("GET", "/sign-in/google/return", true, b"HTTP/1.1 404".as_slice()),
        ("POST", "/sign-in/google/return", false, b"HTTP/1.1 405"),
    ] {
        let (mut service, _) = world(2);
        if disabled {
            service.engine.as_mut().unwrap().clients.google = None;
        }
        let mut peer = Peer::raw(
            service.engine.as_mut().unwrap(),
            format!(
                "{method} {path} HTTP/1.1\r\nHost: temper.example.com\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
            )
            .as_bytes(),
        );
        pump(&mut service, &mut peer);
        ending(&peer, expected, b"");
        assert!(!header(&peer, "content-security-policy").is_empty());
    }
}
#[test]
fn fuzzy_route_endings_replay_from_the_seed() {
    for seed in 0_u64..24 {
        let mut rng = skein_lib::Rng::new(seed);
        let (mut service, mut issuer) = world(2);
        let target = if rng.chance(500) {
            b"/sign-in/google?then=%2Fprojects%3Ftab%3Dmine".as_slice()
        } else {
            b"/sign-in/google?then=%2F%2Fevil.example".as_slice()
        };
        if rng.chance(500) {
            let member = rng.chance(500);
            account(
                &mut issuer,
                b"seed-sub",
                Some(if member { b"sam@example.com" } else { b"other@example.com" }),
                true,
            );
            let peer = signed_return(&mut service, &mut issuer, target);
            ending(&peer, if member { b"HTTP/1.1 303" } else { b"HTTP/1.1 403" }, b"");
            if member {
                assert_eq!(
                    header(&peer, "location"),
                    if target.ends_with(b"evil.example") { b"/".as_slice() } else { b"/projects?tab=mine" }
                );
            }
            continue;
        }
        let (_, cookie) = start(&mut service, &mut issuer, target);
        let callback = match rng.below(3) {
            0 => b"/sign-in/google/return?error=access_denied".as_slice(),
            1 => b"/sign-in/google/return?error=%3Cscript%3E".as_slice(),
            _ => b"/sign-in/google/return?code=unused&state=wrong".as_slice(),
        };
        let mut peer = peer(&mut service, callback, Some(&cookie));
        pump(&mut service, &mut peer);
        ending(&peer, b"HTTP/1.1 400", b"The sign-in didn&#39;t finish");
        assert!(!core::str::from_utf8(peer.received()).unwrap().contains("<script>"), "seed {seed}");
        let mut replayed = self::peer(&mut service, callback, Some(&cookie));
        pump(&mut service, &mut replayed);
        ending(&replayed, b"HTTP/1.1 400", b"It took too long");
    }
}
#[test]
fn a_held_return_outlives_the_http_stall_and_gets_its_expired_page() {
    let (mut service, mut issuer) = world(2);
    let (url, cookie) = start(&mut service, &mut issuer, b"/sign-in/google");
    let target = redirect(&mut issuer, url);
    let mut peer = peer(&mut service, &target, Some(&cookie));
    pump(&mut service, &mut peer);
    peer.pump_at(service.engine.as_mut().unwrap(), true, Time::from_nanos(31_000_000_000), WALL);
    assert!(peer.received().is_empty(), "the OAuth call still owns the HTTP return");
    assert!(!peer.is_closed(), "the return outlives the HTTP stall");
    peer.pump_at(service.engine.as_mut().unwrap(), true, Time::from_nanos(601_000_000_000), WALL);
    ending(&peer, b"HTTP/1.1 400", b"It took too long");
}

#[test]
fn a_confidential_google_sign_in_without_public_uses_the_exact_loopback_redirect() {
    let (mut service, mut issuer) = world_origin(2, Duration::from_secs(600), false);
    account(&mut issuer, b"first-sub", Some(b"sam@example.com"), true);
    let mut start = Peer::raw(
        service.engine.as_mut().unwrap(),
        b"GET /sign-in/google HTTP/1.1\r\nHost: 127.0.0.1:7100\r\nConnection: close\r\n\r\n",
    );
    pump(&mut service, &mut start);
    answer_fetch(&mut service, &mut issuer);
    pump(&mut service, &mut start);
    assert!(start.received().starts_with(b"HTTP/1.1 303"));
    let location = header(&start, "location");
    assert!(
        core::str::from_utf8(location)
            .unwrap()
            .contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A7100%2Fsign-in%2Fgoogle%2Freturn")
    );
    let cookie = header(&start, "set-cookie");
    assert!(!cookie.ends_with(b"; Secure"));
    let cookie = cookie.split(|byte| *byte == b';').next().unwrap().to_vec();
    let target = redirect(&mut issuer, location.into());
    token_plan(&mut issuer);
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: 127.0.0.1:7100\r\nCookie: {}\r\nConnection: close\r\n\r\n",
        core::str::from_utf8(&target).unwrap(),
        core::str::from_utf8(&cookie).unwrap()
    );
    let mut returned = Peer::raw(service.engine.as_mut().unwrap(), request.as_bytes());
    pump(&mut service, &mut returned);
    answer_fetch(&mut service, &mut issuer);
    pump(&mut service, &mut returned);
    ending(&returned, b"HTTP/1.1 303", b"");
    assert!(core::str::from_utf8(returned.received()).unwrap().contains("jig-sign-in="));
    assert_eq!(header(&returned, "location"), b"/");
}
