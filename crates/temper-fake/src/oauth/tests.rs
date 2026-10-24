//! Wrapper policy at the decoder/issuer layer; cryptography belongs to skein.
#![expect(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros,
    reason = "ordinary tests construct JSON and inspect the primitive's output"
)]
#![expect(clippy::default_numeric_fallback, reason = "serde JSON fixture integers")]
use super::{
    CODE_SLOTS, DOCUMENT_BYTES, Document, PLAN_SLOTS, SCOPES, URI_BYTES, credential, issuer, limits, plan, read, refill,
};
use crate::Refusal;
use alloc::vec::Vec;
use serde_json::{Value, json};
use skein_lib::{Duration, Queue, Time, Wall, base64};

const DOCUMENT: &[u8] = br#"{"listen":{"socket":"127.0.0.1:8123"}}"#;
const SEED: u64 = 23;
const VERIFIER: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQ";

fn document(value: &Value) -> Result<Document, Refusal> {
    Document::decode(&serde_json::to_vec(value).unwrap())
}

#[test]
fn decoder_refuses_invalid_members_before_constructing_a_peer() {
    for (bytes, expected) in [
        (b"{".as_slice(), "document: invalid JSON\n"),
        (b"[]", "document: wrong value kind\n"),
        (b"{}", "listen: missing\n"),
        (br#"{"listen":{"socket":"127.0.0.1:8123"},"unknown":0}"#, "unknown: unknown member\n"),
        (br#"{"listen":{"socket":"127.0.0.1:8123"},"listen":{"socket":"127.0.0.1:8124"}}"#, "listen: duplicate\n"),
        (br#"{"listen":{"socket":"127.0.0.1:8123","socket":"127.0.0.1:8124"}}"#, "listen.socket: duplicate\n"),
        (
            br#"{"listen":{"socket":"127.0.0.1:8123"},"redirect_port":1,"redirect_port":2}"#,
            "redirect_port: duplicate\n",
        ),
        (
            br#"{"listen":{"socket":"127.0.0.1:8123"},"redirect_path":"a","redirect_path":"b"}"#,
            "redirect_path: duplicate\n",
        ),
    ] {
        assert_eq!(Document::decode(bytes).unwrap_err().line().as_ref(), expected.as_bytes());
    }
    for (member, replacement, expected) in [
        ("listen", json!(null), "listen: wrong value kind\n"),
        ("listen", json!({"socket":1}), "listen.socket: wrong value kind\n"),
        ("listen", json!({"socket":"127.0.0.1:0"}), "listen: must have a nonzero port\n"),
        ("listen", json!({"socket":"192.0.2.1:8123"}), "listen: must be a loopback socket address\n"),
        ("listen", json!({"socket":"127.0.0.1:8123","extra":0}), "listen.extra: unknown member\n"),
        ("redirect_port", json!(0), "redirect_port: must have a nonzero port\n"),
        ("redirect_port", json!(65_536), "redirect_port: past its bound\n"),
        ("redirect_port", json!(-1), "redirect_port: past its bound\n"),
        ("redirect_port", json!("1455"), "redirect_port: wrong value kind\n"),
        ("redirect_path", json!(null), "redirect_path: wrong value kind\n"),
        ("redirect_path", json!(""), "redirect_path: expected a non-empty relative URI path\n"),
        ("redirect_path", json!("/callback"), "redirect_path: expected a non-empty relative URI path\n"),
        ("redirect_path", json!("callback?x"), "redirect_path: expected a non-empty relative URI path\n"),
        ("redirect_path", json!("callback#x"), "redirect_path: expected a non-empty relative URI path\n"),
        ("redirect_path", json!("callback\r\n"), "redirect_path: expected a non-empty relative URI path\n"),
        ("redirect_path", json!("callback%"), "redirect_path: invalid URI escape\n"),
        ("redirect_path", json!("callback%2g"), "redirect_path: invalid URI escape\n"),
    ] {
        let mut value: Value = serde_json::from_slice(DOCUMENT).unwrap();
        value[member] = replacement;
        assert_eq!(document(&value).unwrap_err().line().as_ref(), expected.as_bytes(), "{member}");
    }
}

#[test]
fn document_and_callback_bounds_hold_on_both_sides() {
    let mut bytes = DOCUMENT.to_vec();
    bytes.resize(usize::try_from(DOCUMENT_BYTES).unwrap(), b' ');
    Document::decode(&bytes).unwrap();
    bytes.push(b' ');
    assert_eq!(Document::decode(&bytes).unwrap_err().line().as_ref(), b"document: invalid JSON\n");
    let prefix = b"http://127.0.0.1:65535/";
    let path_bytes = usize::try_from(URI_BYTES).unwrap() - prefix.len();
    let mut value: Value = serde_json::from_slice(DOCUMENT).unwrap();
    value["listen"] = json!({"socket":"[::1]:65535"});
    value["redirect_port"] = json!(65_535);
    value["redirect_path"] = json!("x".repeat(path_bytes));
    let checked = document(&value).unwrap();
    assert_eq!(checked.redirect_uri.len(), usize::try_from(URI_BYTES).unwrap());
    value["redirect_path"] = json!("x".repeat(path_bytes + 1));
    assert_eq!(document(&value).unwrap_err().line().as_ref(), b"redirect_path: callback URI past its bound\n");
    value["redirect_path"] = json!("x".repeat(usize::try_from(URI_BYTES).unwrap() + 1));
    assert_eq!(document(&value).unwrap_err().line().as_ref(), b"redirect_path: past its bound\n");
    value["listen"]["socket"] = json!("x".repeat(usize::try_from(URI_BYTES).unwrap() + 1));
    assert_eq!(document(&value).unwrap_err().line().as_ref(), b"listen.socket: past its bound\n");
}

#[test]
fn defaults_and_registration_follow_the_shipped_chatgpt_template() {
    let checked = Document::decode(DOCUMENT).unwrap();
    assert_eq!(checked.redirect_uri.as_ref(), b"http://127.0.0.1:1455/auth/callback");
    let template: Value = serde_json::from_slice(include_bytes!("../../../temper-init/template/temper.json")).unwrap();
    let registration = &template["engine"]["llm"]["providers"][0]["sources"]["sign_in"];
    let config = checked.config();
    assert_eq!(config.client_id.as_ref(), registration["client"].as_str().unwrap().as_bytes());
    let terms = config.terms.unwrap();
    let openid = terms.openid.unwrap();
    for (member, endpoint) in [
        ("issuer", openid.issuer.as_ref()),
        ("authorization", config.authorization_url.as_ref()),
        ("token", config.token_endpoint.as_ref()),
        ("revocation", terms.revocation_endpoint.as_deref().unwrap()),
        ("keys", openid.key_set_url.as_ref()),
    ] {
        let suffix = registration[member].as_str().unwrap().strip_prefix("https://auth.openai.com").unwrap();
        assert_eq!(endpoint, format!("http://127.0.0.1:8123{suffix}").as_bytes(), "{member}");
    }
    let scopes =
        registration["scopes"].as_array().unwrap().iter().map(|s| s.as_str().unwrap()).collect::<Vec<_>>().join(" ");
    assert_eq!(SCOPES, scopes.as_bytes());
    assert_eq!(registration["must_grant"], json!(["chatgpt.tokens.use.direct"]));
    assert_eq!(registration["resource"], "https://api.openai.com/v1");
    assert_eq!(openid.subject.as_ref(), b"fake-person");
    assert_eq!(terms.issued.as_ref(), &[Box::<[u8]>::from(b"issued-client".as_slice())]);
    assert!(terms.varying_port && !terms.omit_issued, "shipped dynamic registration terms");
}

#[test]
fn refill_keeps_the_first_added_model_bearer_for_each_seed() {
    let model = crate::llm::Document::decode(br#"{"listen":{"socket":"127.0.0.1:0"},"key":"/tmp/primary","added_keys":["/tmp/signed-in"],"tls":null,"models":[{"name":"qa","window":4096,"output":1024}],"cued":[],"asked":[],"otherwise":["reply"]}"#).unwrap();
    let issuer = Document::decode(DOCUMENT).unwrap();
    for seed in [0, SEED, u64::MAX] {
        let primary = credential::credential(seed, credential::KEY);
        let configuration = crate::llm::configuration(&model, &primary, seed);
        let mut peer = issuer.peer(seed).unwrap();
        refill(&mut peer, seed).unwrap();
        refill(&mut peer, seed).unwrap();
        let granted = peer.granted();
        assert_eq!(granted.len(), usize::try_from(PLAN_SLOTS).unwrap(), "refill stops at Full");
        for access in &granted {
            assert_eq!(access.as_ref(), configuration.added_keys[0].expose());
            assert_ne!(access.as_ref(), primary.as_slice(), "API key and signed-in bearer have distinct streams");
        }
        assert!(peer.observations().is_empty(), "standalone peer retains no records");
    }
}

fn step(issuer: &mut issuer::Issuer, event: issuer::Event) -> issuer::Request {
    let mut out = Queue::with_capacity(issuer::MAX_OUT);
    issuer.step(event, &mut out);
    out.pop().expect("one primitive terminal")
}

fn post(issuer: &mut issuer::Issuer, endpoint: &[u8], body: Box<[u8]>) -> Value {
    let response = step(
        issuer,
        issuer::Event::Post {
            request: skein_oauth::HttpRequest {
                method: skein_oauth::Method::Post,
                id: 1,
                endpoint: endpoint.into(),
                content_type: b"application/x-www-form-urlencoded",
                body,
                deadline: Time::ZERO.saturating_add(Duration::from_secs(10)),
            },
            now: Time::ZERO,
            wall: Wall::EPOCH,
        },
    );
    let issuer::Request::Http(response) = response else { panic!("token response") };
    assert_eq!(response.status, 200);
    serde_json::from_slice(&response.body).unwrap()
}

fn authorize(issuer: &mut issuer::Issuer, endpoint: &[u8]) -> issuer::Request {
    let challenge = skein_oauth::challenge(VERIFIER).unwrap();
    let url = read::joined(&[endpoint,
        b"?response_type=code&client_id=dynamic_agent_client&redirect_uri=http://127.0.0.1:1455/auth/callback&state=0123456789abcdef&nonce=fake-nonce&resource=https://api.openai.com/v1&code_challenge_method=S256&code_challenge=", &challenge]);
    step(issuer, issuer::Event::Authorize { url, now: Time::ZERO })
}

#[test]
fn code_retention_admits_exactly_sixty_four_sign_ins() {
    let config = Document::decode(DOCUMENT).unwrap().config();
    let authorization = config.authorization_url.clone();
    let mut issuer = issuer::Issuer::new(config, limits()).unwrap();
    for _ in 0..CODE_SLOTS {
        let grant = authorize(&mut issuer, &authorization);
        assert!(matches!(grant, issuer::Request::Redirect { .. }), "admitted within the code cap");
    }
    let refused = authorize(&mut issuer, &authorization);
    assert!(matches!(refused, issuer::Request::Refused), "restart required past the code cap");
}

fn token_part(encoded: &str) -> Value {
    let bytes = base64::decode(base64::Alphabet::Url, encoded.as_bytes(), limits().document.id_token_bytes).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[test]
fn three_code_exchanges_and_a_refresh_keep_the_fixed_bearer() {
    let checked = Document::decode(DOCUMENT).unwrap();
    let config = checked.config();
    let endpoint = config.token_endpoint.clone();
    let authorization = config.authorization_url.clone();
    let mut issuer = issuer::Issuer::new(config, limits()).unwrap();
    let keys = step(
        &mut issuer,
        issuer::Event::Get {
            id: 1,
            endpoint: b"http://127.0.0.1:8123/.well-known/jwks.json".as_slice().into(),
            now: Time::ZERO,
            wall: Wall::EPOCH,
        },
    );
    let issuer::Request::Http(keys) = keys else { panic!("configured JWKS document") };
    let keys: Value = serde_json::from_slice(&keys.body).unwrap();
    // Policy plans are fed to the existing independent primitive. Each pass
    // reserves at most two plans; the peer refill itself is checked above.
    for _ in 0..3 {
        issuer.queue(plan(SEED)).unwrap();
        let redirect = authorize(&mut issuer, &authorization);
        let issuer::Request::Redirect { code, client_id, .. } = redirect else { panic!("auto-granted authorization") };
        assert_eq!(client_id.as_deref(), Some(b"issued-client".as_slice()));
        let body = read::joined(&[b"grant_type=authorization_code&client_id=issued-client&redirect_uri=http://127.0.0.1:1455/auth/callback&code=", &code, b"&code_verifier=", VERIFIER]);
        let response = post(&mut issuer, &endpoint, body);
        assert_eq!(
            response["access_token"],
            core::str::from_utf8(&credential::credential(SEED, credential::added_key(0))).unwrap()
        );
        assert_eq!(response["refresh_token"], "fake-refresh");
        assert_eq!(response["scope"], core::str::from_utf8(SCOPES).unwrap());
        assert_eq!(response["expires_in"], 86_400_u64, "long enough for a human walk");
        let parts: Vec<_> = response["id_token"].as_str().expect("OpenID token").split('.').collect();
        let [header, claims, _signature] = parts.as_slice() else { panic!("signed ID token") };
        let header = token_part(header);
        let claims = token_part(claims);
        assert_eq!(header["kid"], keys["keys"][0]["kid"], "the configured JWKS names the ID token's key");
        assert_eq!(claims["iss"], "http://127.0.0.1:8123");
        assert_eq!(claims["aud"], "issued-client");
        assert_eq!(claims["sub"], "fake-person");
        assert_eq!(claims["nonce"], "fake-nonce");
        assert_eq!(claims["exp"], 86_400_u64);
    }
    issuer.queue(plan(SEED)).unwrap();
    let response = post(
        &mut issuer,
        &endpoint,
        b"grant_type=refresh_token&client_id=issued-client&refresh_token=fake-refresh".as_slice().into(),
    );
    assert_eq!(
        response["access_token"],
        core::str::from_utf8(&credential::credential(SEED, credential::added_key(0))).unwrap()
    );
    assert_eq!(response["refresh_token"], "fake-refresh");
    assert_eq!(issuer.generation(), 0, "the fixed refresh token consumes no rotation slots");
}
