//! Tests of the trusted OAuth output's consumer. Signature verification stays
//! in skein-oauth; these inputs stand immediately after its Tokens terminal.
#![expect(
    clippy::disallowed_macros,
    clippy::disallowed_methods,
    reason = "ordinary tests construct and inspect protocol fixtures"
)]
use super::*;
use crate::tests::{fixtures, sign_in as world};
fn token(payload: &[u8]) -> Box<[u8]> {
    let encoded = base64::encoded_len(Alphabet::Url, u32::try_from(payload.len()).unwrap()).unwrap();
    let mut out = Writer::new(usize::try_from(encoded).unwrap().checked_add(4).unwrap());
    out.put(b"e.").unwrap();
    base64::encode(Alphabet::Url, payload, &mut out).unwrap();
    out.put(b".s").unwrap();
    out.finish()
}
fn record(payload: &[u8], subject: &[u8]) -> oauth::SavedToken {
    oauth::SavedToken {
        key: 1,
        generation: 1,
        access_token: b"throwaway-access".as_slice().into(),
        refresh_token: None,
        metadata: None,
        expires_at: Wall::from_nanos(3_600_000_000_000),
        issued_client_id: None,
        granted_scopes: None,
        subject: Some(subject.into()),
        id_token: Some(token(payload)),
        earliest_refresh_at: None,
    }
}
fn finish(
    service: &mut crate::Service,
    issuer: &mut skein_fake_oauth::Issuer,
    payload: &[u8],
    subject: &[u8],
    then: &[u8],
) -> fixtures::http::Peer {
    let (url, cookie) = world::start(service, issuer, then);
    let state = query(split(&url).1, b"state").unwrap();
    let target = format!("/sign-in/google/return?state={}&code=c", core::str::from_utf8(&state).unwrap());
    let mut peer = world::peer(service, target.as_bytes(), Some(&cookie));
    world::pump(service, &mut peer);
    let engine = service.engine.as_mut().unwrap();
    let _request = engine.clients.google_http().unwrap();
    let google = engine.clients.google.as_mut().unwrap();
    let exchange = *google.pending.first().expect("pending sign-in").0;
    let pending = google.pending.get_mut(&exchange).unwrap();
    pending.out = Queue::with_capacity(oauth::MAX_OUT);
    pending.out.push(oauth::Request::Tokens { record: record(payload, subject), missing: Box::new([]) });
    world::pump(service, &mut peer);
    peer
}
#[test]
fn the_claims_reader_reads_only_top_level_fields_and_both_true_spellings() {
    for verified in ["true", "\"true\""] {
        let claims = claims_from(&token(format!("{{\"email\":\"sam@example.com\",\"email_verified\":{verified},\"name\":\"Sam\",\"nested\":{{\"email\":\"other@example.com\"}}}}").as_bytes()), &limits::client(fixtures::engine().limits.sign_in).document).unwrap();
        assert_eq!(claims.email.as_deref(), Some(b"sam@example.com".as_slice()));
        assert!(claims.verified);
        assert_eq!(claims.name.as_deref(), Some(b"Sam".as_slice()));
    }
}
#[test]
fn then_is_a_single_origin_path_after_one_decode() {
    for input in [
        b"".as_slice(),
        b"then=https%3A%2F%2Fevil.example",
        b"then=%2F%2Fevil.example",
        b"then=%2Fback%5Cslash",
        b"then=%2Fline%0D%0A",
        b"then=%2Fbad%GG",
        b"then=%2Fa&then=%2Fb",
    ] {
        assert_eq!(then(input).as_ref(), b"/");
    }
    assert_eq!(then(b"then=%2Fprojects%3Ftab%3Dmine%26sort%3Dnew").as_ref(), b"/projects?tab=mine&sort=new");
    let at = format!("then=%2F{}", "a".repeat(1023));
    assert_eq!(then(at.as_bytes()).len(), 1024);
    let past = format!("{at}a");
    assert_eq!(then(past.as_bytes()).as_ref(), b"/");
}
#[test]
fn endings_escape_addresses_errors_and_return_attributes() {
    let address = b"a<&\"@example.com";
    let then = b"/a?x=\"&y=<";
    for body in [
        pages::not_member(address, then),
        pages::another(address, then),
        pages::unverified(Some(address), then),
        pages::cancelled(b"<script>", then),
        pages::expired(then),
        pages::unreachable(then),
        pages::busy(then),
    ] {
        let text = core::str::from_utf8(&body).unwrap();
        assert!(!text.contains("<script>"));
        assert!(!text.contains("a<&\"@"));
        assert!(text.contains("then=%2Fa%3Fx%3D%22%26y%3D%3C"));
    }
}
#[test]
fn a_members_checked_identity_binds_and_mints_twice_then_refuses_another_subject() {
    let (mut service, mut issuer) = world::world(2);
    for _ in 0_u32..2 {
        let peer = finish(
            &mut service,
            &mut issuer,
            br#"{"email":"SAM@example.com","email_verified":true,"name":"Sam"}"#,
            b"first-sub",
            b"/sign-in/google?then=%2Fprojects%2F1",
        );
        assert!(
            peer.received().starts_with(b"HTTP/1.1 303"),
            "status: {}; events: {}; pending: {}; ready: {}",
            core::str::from_utf8(peer.received()).unwrap().lines().next().unwrap_or("no response"),
            service.engine.as_ref().unwrap().events.len(),
            service.engine.as_ref().unwrap().clients.google.as_ref().unwrap().pending.len(),
            service.engine.as_ref().unwrap().domain.ready()
        );
        assert_eq!(world::header(&peer, "location"), b"/projects/1");
        let text = core::str::from_utf8(peer.received()).unwrap();
        assert!(text.contains("jig-sign-in="));
        assert!(text.contains("SameSite=Strict"));
        assert!(text.contains("; Secure"));
        assert!(text.contains("temper-sign-in=; Path=/sign-in/google; HttpOnly; SameSite=Lax; Max-Age=0; Secure"));
    }
    let peer = finish(
        &mut service,
        &mut issuer,
        br#"{"email":"sam@example.com","email_verified":true}"#,
        b"another-sub",
        b"/sign-in/google",
    );
    world::ending(&peer, b"HTTP/1.1 403", b"signs in here with another Google account");
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
fn checked_nonmembers_and_unverified_addresses_mint_nothing() {
    for (payload, ending) in [
        (
            br#"{"email":"other@example.com","email_verified":true}"#.as_slice(),
            b"other@example.com can&#39;t sign in here".as_slice(),
        ),
        (br#"{"email":"sam@example.com","email_verified":false}"#, b"Google hasn&#39;t verified sam@example.com"),
        (br#"{"name":"No address","email_verified":true}"#, b"Google hasn&#39;t verified this account&#39;s address"),
    ] {
        let (mut service, mut issuer) = world::world(2);
        let peer = finish(&mut service, &mut issuer, payload, b"first-sub", b"/sign-in/google?then=%2F%2Fevil.example");
        world::ending(&peer, b"HTTP/1.1 403", ending);
        assert!(!core::str::from_utf8(peer.received()).unwrap().contains("jig-sign-in="));
    }
}
#[test]
fn mint_refused_at_the_sign_in_capacity_answers_busy() {
    let (mut service, mut issuer) = world::world(2);
    for _ in 0_u32..2 {
        let peer = finish(
            &mut service,
            &mut issuer,
            br#"{"email":"sam@example.com","email_verified":true}"#,
            b"first-sub",
            b"/sign-in/google",
        );
        assert!(peer.received().starts_with(b"HTTP/1.1 303"));
    }
    let peer = finish(
        &mut service,
        &mut issuer,
        br#"{"email":"sam@example.com","email_verified":true}"#,
        b"first-sub",
        b"/sign-in/google",
    );
    world::ending(&peer, b"HTTP/1.1 503", b"temper couldn&#39;t sign you in just now");
    assert!(!core::str::from_utf8(peer.received()).unwrap().contains("jig-sign-in="));
}
#[test]
fn the_recovery_link_uses_the_first_google_owner_without_a_link_owner() {
    let mut config = fixtures::engine();
    let sign_in = world::configured();
    config.jig.owners[0].identity = people::IdentityKey { provider: 3, subject: b"sam@example.com".as_slice().into() };
    let clients = Clients::new(
        &config.jig,
        &config.clients,
        &fixtures::seeds(),
        64,
        &config.limits.root.journal,
        Some(&sign_in),
        &[],
    )
    .unwrap();
    let identity = clients.initial.as_ref().unwrap().link.as_ref().unwrap();
    assert_eq!(identity.key.provider, 3);
    assert_eq!(identity.key.subject.as_ref(), b"sam@example.com");
    assert_eq!(identity.login.as_ref(), b"sam");
    assert_eq!(identity.name.as_ref(), b"sam");
}
#[test]
fn an_unconfigured_origin_fails_before_any_socket_is_opened() {
    let (mut service, _) = world::world(2);
    let google = service.engine.as_mut().unwrap().clients.google.as_mut().unwrap();
    google.scripted = false;
    google
        .endpoints
        .push(fetch::Endpoint {
            origin: b"http://127.0.0.1:31998".as_slice().into(),
            address: ([127, 0, 0, 1], 31998).into(),
            transport: fetch::Transport::Plaintext,
        })
        .unwrap();
    let mut peer = world::peer(&mut service, b"/sign-in/google", None);
    world::pump(&mut service, &mut peer);
    assert!(peer.received().starts_with(b"HTTP/1.1 502"));
}
#[test]
fn stopping_drops_pending_sign_ins_and_unsent_fetch_records() {
    let (mut service, _) = world::world(2);
    let mut peer = world::peer(&mut service, b"/sign-in/google", None);
    world::pump(&mut service, &mut peer);
    let clients = &mut service.engine.as_mut().unwrap().clients;
    assert!(!clients.google.as_ref().unwrap().http.is_empty());
    clients.down(kit::Event::Stop);
    clients.google_advance(&mut Queue::with_capacity(8));
    assert!(clients.google.as_ref().unwrap().closed());
}
#[test]
fn the_full_pending_table_fits_its_declared_memory_envelope() {
    let settings = world::configured();
    let limits = fixtures::engine().limits.sign_in;
    let seeds = fixtures::seeds();
    let meter = skein_world::domain::heap::Meter::new();
    meter.start();
    let mut google = Google::new(&settings, limits, &seeds, 1024);
    for number in 0..limits.pending {
        let redirect: Box<[u8]> = b"https://temper.example.com/sign-in/google/return".as_slice().into();
        let registration = google.registration(redirect.clone(), false);
        let state = random(&mut google.rng);
        let nonce = random(&mut google.rng);
        let verifier = random(&mut google.rng);
        let mut pending = Pending {
            tag: Box::new([0; 16]),
            then: core::iter::repeat_n(b'/', 1024).collect(),
            deadline: Time::from_nanos(600_000_000_000),
            call: None,
            redirect,
            client: Some(oauth::Client::new(limits::client(limits)).unwrap()),
            out: Queue::with_capacity(oauth::MAX_OUT),
            phase: Phase::Starting,
        };
        pending.client.as_mut().unwrap().step(
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
                now: Time::ZERO,
            },
            &mut pending.out,
        );
        google.keep_pending(Token::new(u64::from(number)), pending);
    }
    let measured = meter.end();
    meter.check(measured, worst_case(limits, 8).unwrap(), &"Google pending sign-ins");
    assert_eq!(google.pending.len(), limits.pending);
}
