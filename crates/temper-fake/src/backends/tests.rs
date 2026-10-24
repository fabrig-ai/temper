//! Pure document contracts over current init output. No peer or filesystem is involved.
#![expect(
    clippy::disallowed_types,
    clippy::disallowed_macros,
    clippy::disallowed_methods,
    reason = "ordinary tests edit bounded public JSON fixtures with serde"
)]
#![expect(clippy::default_numeric_fallback, reason = "JSON fixture integers have no Rust arithmetic unit")]
use super::{Documents, Paths, Peers, rewrite};
use crate::Refusal;
use alloc::{vec, vec::Vec};
use core::num::NonZeroU16;
use serde_json::{Value, json};

fn fixture() -> (Value, Value, Paths, Peers) {
    let port = NonZeroU16::new;
    let answer = temper_init::files(
        &temper_init::Facts {
            binary: b"/fixture/temper",
            git: Some(b"/usr/bin/git"),
            claude: None,
            login: Some(b"person"),
            ports: temper_init::Ports {
                clients: port(8367).expect("port"),
                owned: port(8368).expect("port"),
                forge: port(8369).expect("port"),
                llm: port(8370).expect("port"),
                webhooks: port(8371).expect("port"),
            },
        },
        &temper_init::Directories { config: b"/source/config", data: b"/source/data" },
        &[7; 32],
        &[9; 32],
    )
    .expect("init fixture");
    let mut temper = Value::Null;
    let mut agent = Value::Null;
    for file in answer.files {
        if let temper_init::Content::Bytes { bytes, .. } = file.content {
            match file.name {
                b"temper.json" => temper = serde_json::from_slice(&bytes).expect("temper JSON"),
                b"agent.json" => agent = serde_json::from_slice(&bytes).expect("agent JSON"),
                _ => {}
            }
        }
    }
    let paths = Paths {
        config: b"/effective/config".as_slice().into(),
        data: b"/effective/data".as_slice().into(),
        cache: b"/cache".as_slice().into(),
        bundle: b"/effective/config/bundle-loopback".as_slice().into(),
    };
    let peers = Peers {
        github: "127.0.0.1:9101".parse().expect("peer"),
        oauth: "127.0.0.1:9102".parse().expect("peer"),
        llm: "127.0.0.1:9103".parse().expect("peer"),
    };
    (temper, agent, paths, peers)
}
fn write(t: &Value, a: &Value, p: &Paths, peers: &Peers) -> Result<Documents, Refusal> {
    rewrite(&serde_json::to_vec(t).expect("temper bytes"), &serde_json::to_vec(a).expect("agent bytes"), p, peers)
}
fn json(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).expect("effective JSON")
}

#[test]
fn init_targets_change_together_and_preserve_registration_terms() {
    let (mut temper, agent, paths, peers) = fixture();
    temper["engine"]["deployment"]["seed"] = 99.into();
    let docs = write(&temper, &agent, &paths, &peers).expect("rewrite init");
    let effective = json(&docs.temper);
    let effective_agent = json(&docs.agent);
    let forge = &effective["application"]["forges"][1];
    assert_eq!(forge["api"], json!({"socket":"127.0.0.1:9101"}));
    assert_eq!(forge["git"], "http://127.0.0.1:9101");
    assert_eq!(forge["web"], "http://127.0.0.1:9101");
    assert_eq!(forge["transport"], "plaintext");
    let sign_in = &effective["engine"]["llm"]["providers"][0]["sources"]["sign_in"];
    for (name, expected) in [
        ("issuer", "http://127.0.0.1:9102"),
        ("authorization", "http://127.0.0.1:9102/api/accounts/authorize"),
        ("token", "http://127.0.0.1:9102/api/accounts/oauth/token"),
        ("revocation", "http://127.0.0.1:9102/api/accounts/oauth/revoke"),
        ("keys", "http://127.0.0.1:9102/.well-known/jwks.json"),
    ] {
        assert_eq!(sign_in[name], expected, "{name}");
    }
    for name in ["client", "name_hint", "scopes", "must_grant", "resource", "redirect_port", "redirect_path"] {
        assert_eq!(sign_in[name], temper["engine"]["llm"]["providers"][0]["sources"]["sign_in"][name], "{name}");
    }
    assert_eq!(sign_in["transport"], "plaintext");
    assert!(sign_in.get("server_name").is_none());
    assert_eq!(effective["engine"]["llm"]["endpoints"][0]["address"], json!({"socket":"127.0.0.1:9103"}));
    assert_eq!(effective_agent["endpoints"][0]["address"], "127.0.0.1:9103");
    for endpoint in [&effective["engine"]["llm"]["endpoints"][0], &effective_agent["endpoints"][0]] {
        assert_eq!(endpoint["transport"], "plaintext");
        assert!(endpoint.get("server_name").is_none());
    }
    assert_eq!(effective["engine"]["deployment"]["seed"], 99);
    assert_eq!(effective["application"]["clients"]["bundle"], "/effective/config/bundle-loopback");
    assert_eq!(effective["workers"][0]["agent"]["configuration"], "/effective/config/agent.json");
    assert_eq!(effective["workers"][0]["agent"]["arguments"], json!(["agent", "/effective/config/agent.json"]));
    assert_eq!(effective_agent["endpoints"][1], agent["endpoints"][1]);
    assert_eq!(docs, write(&temper, &agent, &paths, &peers).expect("deterministic rewrite"));
}

#[test]
fn generated_peers_use_existing_formats_and_the_first_added_key() {
    let (temper, agent, paths, peers) = fixture();
    let docs = write(&temper, &agent, &paths, &peers).expect("rewrite init");
    let forge = crate::forge::Document::decode(&docs.github).expect("existing forge format");
    let llm = crate::llm::Document::decode(&docs.llm).expect("existing model format");
    assert_eq!(forge.listen(), peers.github);
    assert_eq!(llm.listen(), peers.llm);
    assert_eq!(llm.key(), b"/effective/data/llm.key");
    assert_eq!(llm.added_keys(), &[Box::<[u8]>::from(b"/effective/data/signed-in.key".as_slice())]);
    assert_eq!(json(&docs.github)["github"]["accounts"].as_array().expect("accounts").len(), 1);
    assert_eq!(json(&docs.github)["repositories"].as_array().expect("repositories").len(), 1);
    assert_eq!(json(&docs.llm)["otherwise"], json!(["Hello from the local fake model."]));
    assert_eq!(
        json(&docs.oauth),
        json!({"listen":{"socket":"127.0.0.1:9102"},"redirect_port":1455,"redirect_path":"auth/callback"})
    );
}

#[test]
fn target_selection_uses_ids_and_names_and_removes_stale_tls() {
    let (mut temper, mut agent, paths, peers) = fixture();
    temper["application"]["forges"].as_array_mut().expect("forges").reverse();
    temper["engine"]["llm"]["providers"].as_array_mut().expect("providers").reverse();
    temper["engine"]["llm"]["endpoints"][0]["trust"] = "/stale/root.der".into();
    agent["endpoints"][0]["trust_der"] = "/stale/root.der".into();
    agent["endpoints"].as_array_mut().expect("endpoints").reverse();
    let docs = write(&temper, &agent, &paths, &peers).expect("structural rewrite");
    let effective = json(&docs.temper);
    let effective_agent = json(&docs.agent);
    assert_eq!(effective["engine"]["llm"]["providers"][0], temper["engine"]["llm"]["providers"][0]);
    assert!(effective["engine"]["llm"]["endpoints"][0].get("trust").is_none());
    assert!(effective_agent["endpoints"][1].get("trust_der").is_none());
    assert_eq!(effective_agent["endpoints"][1]["address"], "127.0.0.1:9103");
}

#[test]
fn malformed_targets_have_named_refusals() {
    let (temper, agent, paths, peers) = fixture();
    for (pointer, replacement, words) in [
        ("/application/forges/1/kind", json!("forgejo"), "forge 2 must be GitHub"),
        ("/application/forges/1/number", json!("2"), "wrong value kind"),
        ("/application/forges/1/api", json!(42), "wrong value kind"),
        ("/engine/llm/providers/0/sources/sign_in/issuer", json!(false), "wrong value kind"),
        ("/engine/llm/providers/0/sources/sign_in/client", json!("other"), "expected the shipped registration"),
        ("/engine/llm/providers/0/sources/sign_in/redirect_port", json!(0), "expected shipped callback 1455"),
        (
            "/workers/0/agent/arguments",
            json!(["other", "/source/config/agent.json"]),
            "expected agent and its configuration",
        ),
        (
            "/workers/0/agent/arguments/1",
            json!("/other/agent.json"),
            "configuration and argv must name the same agent document",
        ),
        ("/engine/llm/endpoints/0/address", json!(null), "wrong value kind"),
    ] {
        let mut input = temper.clone();
        *input.pointer_mut(pointer).expect("fixture pointer") = replacement;
        let refusal = write(&input, &agent, &paths, &peers).expect_err("bad target refused");
        assert_eq!(refusal.words, words.as_bytes(), "{pointer}: {}", String::from_utf8_lossy(&refusal.line()));
        assert!(!refusal.path.segments.is_empty(), "{pointer}");
    }
}

#[test]
fn duplicate_targets_and_members_are_refused() {
    let (mut temper, agent, paths, peers) = fixture();
    let target = temper["application"]["forges"][1].clone();
    temper["application"]["forges"].as_array_mut().expect("forges").push(target);
    assert_eq!(write(&temper, &agent, &paths, &peers).expect_err("duplicate forge").words, b"target must be unique");
    let agent = serde_json::to_vec(&agent).expect("agent bytes");
    assert_eq!(
        rewrite(br#"{"engine":{},"engine":{}}"#, &agent, &paths, &peers).expect_err("duplicate member").words,
        b"duplicate"
    );
}

#[test]
fn input_byte_bound_is_checked_before_any_output() {
    let (temper, agent, paths, peers) = fixture();
    let mut source: Vec<u8> = serde_json::to_vec(&temper).expect("temper bytes");
    let bound = usize::try_from(crate::DOCUMENT_BYTES).expect("document bound");
    source.resize(bound, b' ');
    let agent = serde_json::to_vec(&agent).expect("agent bytes");
    rewrite(&source, &agent, &paths, &peers).expect("document exactly at its byte bound");
    source.push(b' ');
    assert_eq!(rewrite(&source, &agent, &paths, &peers).expect_err("past document bound").words, b"past its bound");
}

#[test]
fn peer_and_effective_path_bounds_refuse_before_rewriting() {
    let (temper, agent, paths, peers) = fixture();
    for address in ["127.0.0.1:0", "127.0.0.1:1455", "192.0.2.1:9101"] {
        let mut input = peers;
        input.github = address.parse().expect("peer fixture");
        assert!(write(&temper, &agent, &paths, &input).is_err(), "{address}");
    }
    for value in [b"relative".as_slice(), b"/effective/../source", b"/bad\0path"] {
        let mut input = paths.clone();
        input.config = value.into();
        assert!(write(&temper, &agent, &input, &peers).is_err(), "{value:?}");
    }
    let mut input = paths;
    input.data = vec![b'/'; 4097].into();
    write(&temper, &agent, &input, &peers).expect_err("effective data path past its byte bound");
}
