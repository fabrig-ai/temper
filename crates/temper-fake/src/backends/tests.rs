//! Pure document contracts over current init output. No peer or filesystem is involved.
#![expect(
    clippy::disallowed_types,
    clippy::disallowed_macros,
    clippy::disallowed_methods,
    reason = "ordinary tests edit bounded public JSON fixtures with serde"
)]
#![expect(clippy::default_numeric_fallback, reason = "JSON fixture integers have no Rust arithmetic unit")]
use super::{Documents, Paths, Peers, SEED, rewrite, steps};
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
        claude: b"/fixture/fake-claude".as_slice().into(),
    };
    let peers = Peers {
        github: "127.0.0.1:9101".parse().expect("peer"),
        oauth: "127.0.0.1:9102".parse().expect("peer"),
        llm: "127.0.0.1:9103".parse().expect("peer"),
        forgejo: "127.0.0.1:9104".parse().expect("peer"),
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
fn welcome_steps_name_only_the_four_held_peers_and_share_the_seed() {
    let (_, _, _, peers) = fixture();
    assert_eq!(SEED, 7);
    for config in [b"/effective/config".as_slice(), b"/effective/config/"] {
        let given = steps(config, &peers);
        assert_eq!(given.len(), 4);
        for (step, name, command, document, listen) in [
            (
                &given[0],
                b"fake github".as_slice(),
                b"forge".as_slice(),
                b"/effective/config/github.json".as_slice(),
                peers.github,
            ),
            (
                &given[2],
                b"fake model".as_slice(),
                b"llm".as_slice(),
                b"/effective/config/llm.json".as_slice(),
                peers.llm,
            ),
            (
                &given[1],
                b"fake issuer".as_slice(),
                b"oauth".as_slice(),
                b"/effective/config/oauth.json".as_slice(),
                peers.oauth,
            ),
            (
                &given[3],
                b"fake forgejo".as_slice(),
                b"forge".as_slice(),
                b"/effective/config/forgejo.json".as_slice(),
                peers.forgejo,
            ),
        ] {
            assert_eq!(step.name(), name);
            assert_eq!(step.kind(), temper_forge_setup::Kind::Long);
            assert_eq!(
                step.arguments(),
                [b"fake".as_slice(), command, b"--document", document, b"--seed", b"7"].map(Box::<[u8]>::from)
            );
            assert_eq!(step.listeners(), &[listen]);
            assert_eq!(step.ready(), None);
        }
    }
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
    assert_eq!(json(&docs.github)["github"]["app"]["setup_url"], "http://127.0.0.1:8367/forges/2/github/setup");
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
    // Fill every TLS removal slot alongside all welcome git/program edits.
    const TLS: [&str; 4] = ["server_name", "trust", "trust_der", "identity_file"];
    let (mut temper, mut agent, paths, peers) = fixture();
    for pointer in ["/application/forges/1", "/engine/llm/providers/0/sources/sign_in", "/engine/llm/endpoints/0"] {
        for member in TLS {
            temper.pointer_mut(pointer).expect("rewrite target")[member] = "/stale/root.der".into();
        }
    }
    for member in TLS {
        agent["endpoints"][0][member] = "/stale/root.der".into();
    }
    temper["application"]["forges"].as_array_mut().expect("forges").reverse();
    temper["engine"]["llm"]["providers"].as_array_mut().expect("providers").reverse();
    agent["endpoints"].as_array_mut().expect("endpoints").reverse();
    let docs = write(&temper, &agent, &paths, &peers).expect("structural rewrite");
    let effective = json(&docs.temper);
    let effective_agent = json(&docs.agent);
    assert_eq!(effective["engine"]["llm"]["providers"][0], temper["engine"]["llm"]["providers"][0]);
    assert!(effective["engine"]["llm"]["endpoints"][0].get("trust").is_none());
    assert!(effective_agent["endpoints"][1].get("trust_der").is_none());
    assert_eq!(effective_agent["endpoints"][1]["address"], "127.0.0.1:9103");
    for target in [
        &effective["application"]["forges"][1],
        &effective["engine"]["llm"]["providers"][1]["sources"]["sign_in"],
        &effective["engine"]["llm"]["endpoints"][0],
        &effective_agent["endpoints"][1],
    ] {
        for member in TLS {
            assert!(target.get(member).is_none(), "removed {member}: {target}");
        }
    }
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

#[test]
fn managed_four_and_adapter_are_ordinary_and_preserve_existing_entries() {
    for present in [false, true] {
        let (temper, mut agent, paths, peers) = fixture();
        if present {
            agent["programs"] = json!({"claude_code":{"path":"/source/claude","directory":"/source/programs"}});
        }
        let docs = write(&temper, &agent, &paths, &peers).expect("ordinary rewrite");
        let effective = json(&docs.temper);
        let effective_agent = json(&docs.agent);
        for index in [0_usize, 2] {
            assert_eq!(effective["application"]["forges"][index], temper["application"]["forges"][index]);
        }
        assert_eq!(effective["application"]["forges"][1]["git_program"], "/effective/data/demo/bin/git");
        assert!(
            effective["application"]["forges"][2].get("git_program").is_none(),
            "Forgejo here keeps the machine git"
        );
        assert_eq!(effective["application"]["forges"][3]["git_program"], "/effective/data/demo/bin/git");
        assert_eq!(
            effective["application"]["forges"][3],
            json!({"number":4,"kind":"forgejo","api":{"socket":"127.0.0.1:9104"},"git":"http://127.0.0.1:9104","git_program":"/effective/data/demo/bin/git","web":null,"transport":"plaintext"})
        );
        assert_eq!(effective["application"]["setup"][2]["mode"], "managed");
        assert_eq!(
            effective_agent["programs"]["claude_code"],
            json!({"path":"/fixture/fake-claude","directory":"/effective/data/programs"})
        );
        assert!(effective["engine"]["llm"]["providers"][0]["sources"].get("programs").is_none());
        let document = crate::forge::Document::decode(&docs.forgejo).expect("existing Forgejo schema");
        assert_eq!(document.listen(), peers.forgejo);
        assert_eq!(document.admin_token(), b"/effective/data/forgejo-admin.token");
    }
}

#[test]
fn program_offer_has_a_runnable_model_route_in_both_documents() {
    let (temper, agent, paths, peers) = fixture();
    let docs = write(&temper, &agent, &paths, &peers).expect("program route rewrite");
    let effective = json(&docs.temper);
    let effective_agent = json(&docs.agent);
    let llm = &effective["engine"]["llm"];
    let provider = llm["providers"]
        .as_array()
        .expect("providers")
        .iter()
        .find(|provider| provider["number"] == 2)
        .expect("program's provider");
    assert_eq!(
        provider,
        &json!({
            "name":"anthropic", "number":2, "signed_in_plan":true,
            "models":["fake-claude"], "sources":{"programs":["claude_code"]}
        })
    );
    let openai = &llm["providers"][0];
    assert_eq!(openai["models"], temper["engine"]["llm"]["providers"][0]["models"]);
    assert!(openai["sources"].get("programs").is_none());
    assert!(provider["models"].as_array().expect("provider models").contains(&json!("fake-claude")));
    assert!(provider["sources"]["programs"].as_array().expect("program adapters").contains(&json!("claude_code")));
    let model = llm["models"]
        .as_array()
        .expect("models")
        .iter()
        .find(|model| model["name"] == "fake-claude")
        .expect("program model");
    assert_eq!(model["endpoint"], "fake-claude");
    let engine_endpoint = llm["endpoints"]
        .as_array()
        .expect("engine endpoints")
        .iter()
        .find(|endpoint| endpoint["name"] == model["endpoint"])
        .expect("engine program endpoint");
    let agent_endpoint = effective_agent["endpoints"]
        .as_array()
        .expect("agent endpoints")
        .iter()
        .find(|endpoint| endpoint["name"] == model["endpoint"])
        .expect("agent program endpoint");
    for endpoint in [engine_endpoint, agent_endpoint] {
        assert_eq!(endpoint["program"], "claude_code");
        assert_eq!(endpoint["provider"], "anthropic");
        assert_eq!(endpoint["number"], 2);
    }
    let agent_model = agent_endpoint["models"].as_array().expect("agent models").first().expect("program model");
    for field in ["name", "window", "output", "prices"] {
        assert_eq!(model[field], agent_model[field], "program declarations agree: {field}");
    }
    for charter in effective["engine"]["charters"].as_array().expect("charters") {
        assert!(charter["alternatives"].as_array().expect("charter alternatives").contains(&json!("fake-claude")));
        assert_eq!(charter["model"], "gpt-6-luna", "the existing default remains credential-backed");
    }
}

#[test]
fn managed_admin_name_accepts_the_actual_fake_seeded_admin_and_token() {
    use skein_forge::forgejo::document::{Document, User};
    use skein_forge_setup::{
        item::Item,
        reachable::{self, Reachable},
        reconcile::{Event, Need, Reply},
    };
    use skein_lib::{Queue, Secret};
    let (temper, agent, paths, peers) = fixture();
    let docs = write(&temper, &agent, &paths, &peers).expect("rewrite");
    let document = crate::read::document(&docs.temper).expect("effective document");
    let application =
        super::member(&document, 0, b"application", &jig_protocol_config::Path::root()).expect("application");
    let setup = super::member(&document, application, b"setup", &jig_protocol_config::Path::root()).expect("setup");
    let mut problems = temper_forge_setup::problem::Problems::empty();
    let limits = temper_forge_setup::limits::small();
    let setup = temper_forge_setup::Setup::read(
        &document,
        setup,
        &limits,
        &jig_protocol_config::Directories::working(),
        &mut problems,
    )
    .expect("ordinary setup fixture");
    let given = temper_forge_setup::given::values(setup.entry(4).expect("managed entry"), None, &limits);
    assert_eq!(given.admin_name.as_ref(), b"admin");
    let token = crate::credential::credential(SEED, crate::credential::ADMIN_TOKEN);
    let forge = crate::forge::Document::decode(&docs.forgejo).expect("forge document");
    let (mut peer, _) = crate::forge::peer(&forge, &token, SEED).expect("seeded peer");
    let store = peer.setup_mut();
    let admin = store.find(b"temper-admin").expect("actual seeded admin");
    assert!(store.token_user(&token) == Some(admin.id), "seed token authenticates the seeded admin");
    let user =
        User { id: admin.id, login: admin.login.clone(), is_admin: admin.admin, full_name: admin.full_name.clone() };
    let mut machine = Reachable::new(&given, None, 1).expect("managed Given admitted");
    let mut out = Queue::with_capacity(reachable::MAX_OUT);
    reachable::step(&mut machine, Event::Begin, &mut out);
    let Some(Need::Read(Item::AdminToken)) = out.pop() else { panic!("read the admin token") };
    reachable::step(&mut machine, Event::Read(Some(Secret::new(token.as_slice().into()))), &mut out);
    let Some(Need::Send { .. }) = out.pop() else { panic!("send the next ordinary request") };
    reachable::step(&mut machine, Event::Reply(Reply::Document(Document::User(user))), &mut out);
    let Some(Need::Send { .. }) = out.pop() else {
        panic!("ordinary reachability must accept the seeded admin despite its different login")
    };
    reachable::step(
        &mut machine,
        Event::Reply(Reply::Document(Document::ApiSettings(skein_forge::forgejo::document::ApiSettings {
            max_response_items: 32,
            default_paging_num: 30,
        }))),
        &mut out,
    );
    let Some(Need::Send { .. }) = out.pop() else { panic!("send the next ordinary request") };
    reachable::step(&mut machine, Event::Reply(Reply::Rows(Box::new([]))), &mut out);
    let Some(Need::Done(skein_forge_setup::reconcile::Outcome::Reachable { more: false, .. })) = out.pop() else {
        panic!("the ordinary picker finishes")
    };
    peer.shutdown();
}
