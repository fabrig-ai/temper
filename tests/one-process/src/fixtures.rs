//! Configuration bytes, private files and scripts shared by the stories.
#[cfg(test)]
pub(crate) mod durable;
#[path = "../../real-loop/src/turn_bounds.rs"]
pub mod turn_bounds;
use crate::world::Settings;
use jig_channel_world::fixtures as channel;
use skein_lib::{Duration, List, Secret};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
pub(crate) const FORGE: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 3000));
pub const HOOK_SECRET: &[u8] = b"world-webhook-secret";
pub const SEED: u64 = 7;
pub const ANSWER: &[u8] = b"World answer";
pub const LLM_TOKEN: &[u8] = channel::KEY;
pub const GIT_TOKEN: &[u8] = temper_fake_git_hosted::fixtures::TOKEN;
pub const API_TOKEN: &[u8] = temper_fake_git_hosted::fixtures::TOKEN;
#[must_use]
pub fn two_forge_records() -> Vec<temper_engine_domain_forge::Stored> {
    [1, 2]
        .into_iter()
        .map(|task| temper_engine_domain_forge::Stored::Names { task, resources: Box::default() })
        .collect()
}
pub const BRANCH_HEAD: u64 = 2;
pub const MOVED_BASE: u64 = 3;
pub(crate) const MERGE_BRANCH: &[u8] = b"temper/1/c2";
pub(crate) const PROJECT: u32 = 1;
pub(crate) const FORGE_CONNECTOR: u16 = 1;
pub(crate) const GRANT_KINDS: [u16; 2] = [3, 11];
pub(crate) const PROCEDURE_CODE: u16 = 2;
pub(crate) const AGENT_KIND: u16 = 1;
pub(crate) const CHARTER_NUMBER: u16 = 1;
pub(crate) const REPOSITORY: u32 = 2;
// A task names the repository by forge 1 and its id, packed (domain/forge.md, 4).
pub(crate) const REPOSITORY_TAG: u64 = temper_engine_domain_forge::tag(1, REPOSITORY);
pub(crate) const PARAMETER_NAMES: [u16; 3] = [1, 2, 3];
pub(crate) const CHANGE_KIND: u16 = 1;
pub(crate) const CHANGE_WORDS: u32 = 64;
pub(crate) const PRODUCER_TOOLS: u64 = 3;
pub(crate) const PRODUCER_TASKS: u32 = 2;
pub(crate) const PRODUCER_DEPTH: u32 = 1;
pub(crate) const PRODUCER_SPEND: u64 = 40;
pub(crate) const PRODUCER_NOTES: u32 = 0;
pub(crate) const CALL_TOKENS: u64 = 1;
pub(crate) const PERSON_WAIT: Duration = Duration::from_secs(5);
// crates/temper/tests/support/configuration.rs: the process tier's short lists.
pub(crate) const DOCUMENT_LINES: u32 = 2;
pub(crate) const CHAT_WAIT_MS: u32 = 1000;
pub(crate) const FORGE_STATUSES: u32 = 8;
pub(crate) const CI_SILENT: u32 = 0;
pub(crate) const CI_PASSES: u32 = 1000;
pub(crate) const CI_RERUNS: u32 = 0;
pub(crate) const WORKER_USER: u64 = 1;
pub(crate) const MOVING_USER: u64 = 2;
pub(crate) const AUTHORITY_DEPTH: u32 = 3;
pub(crate) const CHARTER_TURNS: u32 = 8;
pub(crate) const IMPLICATIONS: [u16; 2] = [1, 2];
pub(crate) const ENGINE_OVERRIDES: [(&str, u32); 20] = [
    ("tasks.tasks", 4),
    ("tasks.project_tasks", 4),
    ("tasks.tree_tasks", 4),
    ("people.people", 4),
    ("people.inbox_tasks", 8),
    ("fleet.attempts", 4),
    ("fleet.turns", 16),
    ("brief.sections", 8),
    ("brief.brief_bytes", 4096),
    ("tasks.result_bytes", 128),
    ("tasks.resource_reports", 4),
    ("tasks.pools", 2),
    ("tasks.parameters", 4),
    ("tasks.executor_kinds", 2),
    ("tasks.authority_grants", 4),
    ("tasks.authority_segments", 4),
    ("tasks.authority_bytes", 128),
    ("core.policy_bytes", 4096),
    ("authority.segments", 8),
    ("core.call_answer_bytes", 2048),
];
pub(crate) const ADOPTION_FORM_START: usize = 3;
pub(crate) const ADOPTION_FORM_STEPS: usize = 7;
pub(crate) const ADOPTION_REPLY_STEP: usize = 7;
pub(crate) const PREPARED_REPOSITORY: &[u8] = b"workspaces/0/app";
pub(crate) const AGENTS: u32 = 2;
pub(crate) const CLIENTS: std::net::SocketAddr =
    std::net::SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 7102);
#[must_use]
pub fn calm(seed: u64) -> Settings {
    Settings { seed, config: skein_sim::Config::calm() }
}
pub(crate) fn client_limits() -> temper_client_shell_terminal::limits::Limits {
    let mut limits = temper_client_shell_terminal::limits::native();
    limits.connections.body_bytes = 262_144;
    limits.client.client.domain.fields = 8;
    limits.client.client.domain.parts = 4;
    limits.client.client.domain.reads = 8;
    limits.client.client.domain.text = 128;
    limits.client.client.domain.turn_text = 128;
    limits.client.client.domain.document_bytes = 65_536;
    limits.client.client.domain.requests.bytes = 262_144;
    limits.client.client.domain.ask_bytes =
        limits.client.client.domain.requests.bytes / u64::from(limits.client.client.domain.requests.requests);
    limits
}
pub(crate) fn document() -> Vec<u8> {
    let mut value = world_document();
    value.as_object_mut().expect("deployment").remove("world_settings");
    serde_json::to_vec(&value).expect("deployment JSON")
}
pub(crate) fn world_document() -> serde_json::Value {
    let mut value: serde_json::Value = serde_json::from_slice(&channel::engine_document()).expect("engine JSON");
    value.as_object_mut().expect("object").remove("service");
    let worker: serde_json::Value = serde_json::from_slice(channel::WORKER_DOCUMENT).expect("worker JSON");
    value["workers"] = worker["workers"].clone();
    value["workers"][0]["engine"] = serde_json::json!({"in_process":"workers"});
    value["workers"][0]["agent"] = serde_json::json!({"program":"/opt/temper/bin/temper","configuration":"/opt/temper/agent.json","arguments":["internal", "agent","/opt/temper/agent.json"]});
    value["workers"][0]["limits"]["overrides"]["hub.accounts"] = 4.into();
    value["workers"][0]["limits"]["overrides"]["hub.event_bytes"] = 1024.into();
    value["engine"]["hosts"]["listener"] = serde_json::json!([{"in_process":"workers"}]);
    value["engine"]["connectors"] = serde_json::json!([1]);
    for (name, count) in [
        ("core.load_slots", 8),
        ("documents.rows", 2),
        ("documents.lines", DOCUMENT_LINES),
        ("documents.text_bytes", 128),
        ("people.sign_ins", 2),
        ("views.watchers", 16),
        ("views.watchers_per_party", 16),
    ] {
        value["engine"]["limits"]["overrides"][name] = count.into();
    }
    value["application"] = serde_json::json!({
        "webhooks":{"listener":{"socket":"127.0.0.1:0"}},
"clients":{"listener":{"socket":CLIENTS.to_string()},"public":null,"bundle":null},
        "forges":[{"number":1,"kind":"forgejo","api":{"socket":FORGE.to_string()},"transport":"plaintext",
            "git":format!("http://{FORGE}"),"webhook_secret":"secrets/forge-1.webhook-secret","api_token":"secrets/forge.api-token","user":{"file":"secrets/forge-1.user"},"identity":1}],
        "identities":[{"number":1,"token":"secrets/forge.git-token","login":"worker","author":{"name":"Worker","email":"worker@example.invalid"}}],
        "setup":[],"demo":null,"sign_in":null,"landing":{"deployment":[],"projects":[]},"charter":1,
        "limits":{"profile":"small","overrides":{"checkout.name_bytes":256,"checkout.repositories":2,"checkout.message_bytes":4096,"forge.name_bytes":64,"forge.resources_per_task":2,"git_component.stdout_bytes":2048}},
        "workers":[{"root":"worker","git":"/usr/bin/git"}]});
    crate::settings::stage_policies(&mut value);
    crate::stories::configure(&mut value);
    for member in ["charters", "templates", "policies", "rules"] {
        value["engine"].as_object_mut().expect("engine").remove(member);
    }
    if crate::stories::uses_stock() {
        // Keep stock policy 1 for the restore refusal; chats seed the world's policy 2.
        for seed in value["engine"]["projects"].as_array_mut().expect("seed projects") {
            seed["policy"] = 2.into();
        }
        for project in value["world_settings"]["policies"].as_array_mut().expect("world policies") {
            project["policy"]["ceiling"]["delegation"]["depth"] = 4.into();
            for role in project["policy"]["roles"].as_array_mut().expect("roles") {
                role["authority"]["delegation"]["depth"] = 4.into();
            }
        }
        for (name, count) in [("core.policy_bytes", 4096), ("authority.grants", 16), ("notes.scopes", 32)] {
            value["engine"]["limits"]["overrides"][name] = count.into();
        }
    }
    for (name, count) in [
        ("people.amendment_bytes", 512),
        ("tasks.hold_bytes", 256),
        ("tasks.funders", 4),
        ("tasks.dependencies", 2),
        ("tasks.charters", 4),
        ("notes.entries_per_scope", 2),
        ("notes.recalled", 2),
        ("people.holdings", 2),
        ("people.pending", 2),
        ("people.waiters", 2),
    ] {
        value["engine"]["limits"]["overrides"][name] = count.into();
    }
    value["engine"]["limits"]["overrides"]["people.projects"] = 2.into();
    value["engine"]["limits"]["overrides"]["authority.projects"] = 2.into();
    value["engine"]["limits"]["overrides"]["tasks.funders"] = 4.into();
    turn_bounds::apply(&mut value, agent().limits.bodies.turn_body);
    value
}
pub(crate) fn files() -> temper_service::settings::Files {
    let mut entries = List::with_capacity(10);
    for source in [channel::files(false), channel::files(true)] {
        for (name, value) in source.entries.into_boxed() {
            if name.starts_with(b"charters/") {
                continue;
            }
            let name = if name.as_ref() == b"agent.json" { b"/opt/temper/agent.json".as_slice().into() } else { name };
            entries.push((name, value)).expect("fixture files");
        }
    }
    for name in [b"secrets/forge.api-token".as_slice(), b"secrets/forge.git-token"] {
        entries.push((name.into(), Secret::new(temper_fake_git_hosted::fixtures::TOKEN.into()))).expect("forge files");
    }
    entries
        .push((b"secrets/forge-1.user".as_slice().into(), Secret::new(b"1\n".as_slice().into())))
        .expect("user file");
    entries
        .push((b"secrets/forge-1.webhook-secret".as_slice().into(), Secret::new(HOOK_SECRET.into())))
        .expect("webhook secret file");
    if crate::stories::held_change() && crate::stories::first_start() {
        entries
            .push((b"forge-1/admin_token".as_slice().into(), Secret::new(crate::restart_control::ADMIN_TOKEN.into())))
            .expect("fixture handover");
    }
    temper_service::settings::Files { entries }
}
pub(crate) fn seeds() -> temper_service::settings::Seeds {
    let seed = channel::seeds();
    temper_service::settings::Seeds {
        deployment: seed.deployment,
        seed: seed.seed,
        crypto: seed.crypto,
        installation: seed.installation,
    }
}
pub(crate) fn agent() -> temper_service::settings::Agent {
    let mut agent = channel::agent();
    let limits = agent_limits();
    agent.host_limits =
        smith_local_service::profile::host_limits(&agent_profile(), &limits).expect("agent host limits");
    agent.host_limits.agents = AGENTS;
    let channel = limits.channel;
    agent.limits = smith_host_protocol::Limits {
        bodies: channel.bodies,
        charter: channel.charter,
        transcript: channel.transcript,
        endpoints: channel.endpoints,
        channel: channel.channel,
        start_bytes: channel.start_bytes,
        calls: agent.host_limits.calls,
    };
    agent.limits.channel.output_bytes = smith_host_protocol::output_bytes(&agent.limits).expect("host frames");
    agent.bounds.message_bytes = 1024;
    agent.bounds.checks = Duration::from_secs(5);
    temper_service::settings::Agent {
        tool_payload: agent_profile().declared.tool_payload,
        programs: Box::new([]),
        endpoints: agent.endpoints,
        limits: agent.limits,
        bounds: agent.bounds,
        host_limits: agent.host_limits,
    }
}
pub(crate) fn agent_profile() -> smith_agent_service::profile::Profile {
    let mut profile = smith_agent_process_world::profile();
    profile.policy.host_tools =
        u32::try_from(temper_engine_smith::tools(Duration::from_secs(10)).len()).expect("tool count");
    profile.declared.tool_payload = 2048;
    profile.declared.read_window = 256;
    profile.declared.conversations = 1;
    profile.declared.calls_per_response = 1;
    profile.policy.check_timeout = Duration::from_secs(5);
    let hub = jig_protocol_config::profile(jig_protocol_config::Profile::Small).hub;
    profile.policy.inbox = hub.held.max(hub.inbox);
    profile.policy.sections = 8;
    profile
}
pub(crate) fn agent_limits() -> smith_agent_service::Limits {
    let mut limits =
        smith_agent_service::profile::derive(&agent_profile(), &agent_declarations()).expect("chat agent limits");
    limits.llm.accounts = 2;
    limits
}
pub(crate) fn agent_declarations() -> smith_agent_service::profile::Configuration {
    let mut declarations = smith_agent_process_world::declarations(0);
    declarations.endpoints[0].models[0].output = 2048;
    declarations
}
pub(crate) fn agent_configuration() -> smith_agent_service::Config {
    let mut config = channel::agent_configuration();
    config.limits = agent_limits();
    let (destinations, _) = config.llm_endpoints.into_parts();
    let mut destination = destinations.into_boxed().into_vec().pop().expect("one LLM endpoint");
    destination.limits = config.limits.llm.adapter.client;
    config.llm_endpoints = smith_protocol_llm::Endpoints::new(
        Box::new([smith_protocol_llm::ConfiguredEndpoint {
            name: smith_domain::llm::Endpoint(0),
            destination,
            connect: Duration::from_secs(1),
            handshake: Duration::from_secs(1),
            identity: smith_protocol_llm::IdentityProfile::Plain,
            models: Box::default(),
        }]),
        1,
    )
    .expect("derived provider endpoint");
    config
}
pub(crate) fn forges() -> List<temper_service::ForgeRuntime> {
    let mut forges = List::with_capacity(1);
    forges
        .push(temper_service::ForgeRuntime {
            number: 1,
            address: FORGE,
            transport: skein_forge_connection::settings::Transport::Plaintext { name: None },
            trust_roots: 0,
            trust_root_bytes: 0,
        })
        .expect("one loopback forge");
    forges
}
pub(crate) fn provider() -> skein_fake_peers::llm::Peer {
    let mut script = channel::one_chat(b"Hello");
    script.result = br#"{"form":"report","text":"World answer","fields":{}}"#.as_slice().into();
    scripted_provider(Box::new([skein_fake_llm_domain::api::Script {
        cue: Box::default(),
        turns: channel::provider_turns(&script),
    }]))
}
pub(crate) fn read_then_wait_provider() -> skein_fake_peers::llm::Peer {
    use skein_fake_llm_domain::api::{Finish, Line, Script, Turn};
    scripted_provider(Box::new([Script {
        cue: Box::default(),
        turns: Box::new([
            Turn {
                lines: Box::new([Line::Call {
                    name: b"read".as_slice().into(),
                    arguments: br#"{"path":"README.md"}"#.as_slice().into(),
                }]),
                finish: Finish::ToolCalls,
                tokens: 20,
            },
            Turn {
                lines: Box::new([
                    Line::Text { text: ANSWER.into() },
                    Line::Call { name: b"wait".as_slice().into(), arguments: b"{}".as_slice().into() },
                ]),
                finish: Finish::ToolCalls,
                tokens: 20,
            },
        ]),
    }]))
}
/// Temper's flat host schemas and a mounted workspace fit this fake's request.
pub(crate) fn scripted_provider(scripts: Box<[skein_fake_llm_domain::api::Script]>) -> skein_fake_peers::llm::Peer {
    use skein_fake_llm_protocol::{documents, provider};
    const REQUEST: u32 = 65_536;
    const STRINGS: u32 = 16_384;
    let mut config = skein_llm_world::fake::config();
    config.latency_min = Duration::ZERO;
    config.latency_max = Duration::ZERO;
    config.script_bytes = config.script_bytes.max(STRINGS);
    config.answer_bytes = config.answer_bytes.max(STRINGS);
    config.query_bytes = config.query_bytes.max(REQUEST);
    let mut protocol = skein_llm_world::fake::limits(&skein_llm_world::limits());
    protocol.documents.openai.request = REQUEST;
    protocol.documents.openai.strings = STRINGS;
    protocol.documents.openai.answer = protocol.documents.openai.answer.max(STRINGS);
    protocol.http.body = protocol.http.body.max(u64::from(REQUEST));
    protocol.sse.event = protocol.sse.event.max(STRINGS);
    protocol.documents.openai.tokens = 4096;
    protocol.documents.openai.output_items = 256;
    protocol.documents.anthropic.output_items = 256;
    skein_fake_peers::llm::Peer::new(
        (smith_agent_process_world::fake::loopback(), 443).into(),
        skein_fake_peers::Transport::Plaintext,
        smith_agent_process_world::fake::limits(),
        provider::Config {
            echo: skein_llm::openai::Echo::NONE,
            usage_fields: documents::UsageFields::ALL,
            provider: documents::Provider::OpenAi,
            path: skein_llm::Endpoint::openai().target,
            headers: Box::new([]),
            without_credential: false,
        },
        skein_llm::Credential { value: channel::KEY.into(), scheme: skein_llm::Scheme::Key, account_id: Box::new([]) },
        protocol,
        skein_fake_llm_domain::Domain::try_scripted(&config, 17, scripts).expect("bounded provider scripts"),
        config,
    )
    .expect("configured provider")
}
pub(crate) fn codec() -> jig_protocol_store::codec::Limits {
    let limits = jig_test_service_limits(&engine().limits);
    jig_protocol_store::codec::limits(&limits.root.core).expect("codec")
}
/// The configured models' names the people records name models by (jig's protocol/store.md, 5.1).
pub(crate) fn store_names() -> &'static [jig_domain_core::settings::ModelName] {
    static NAMES: std::sync::OnceLock<Box<[jig_domain_core::settings::ModelName]>> = std::sync::OnceLock::new();
    NAMES.get_or_init(|| jig_protocol_store::codec::names(&engine().llm.models))
}
fn engine() -> jig_protocol_config::Engine {
    let bytes = document();
    let size = u32::try_from(bytes.len()).expect("fixture document");
    let doc = jig_protocol_config::document(
        &bytes,
        &jig_protocol_config::DocumentLimits { bytes: size, tokens: size, depth: 32 },
    )
    .expect("document");
    let mut object = jig_protocol_config::read::Object::new(&doc, 0).expect("object");
    let at = object.member(&doc, b"engine").expect("engine member");
    temper_service::startup::engine_with(
        &doc,
        at,
        &jig_protocol_config::Directories::working(),
        &crate::stories::given(&serde_json::from_slice(&bytes).expect("deployment JSON")),
    )
    .expect("engine")
}
fn jig_test_service_limits(overrides: &jig_protocol_config::Limits) -> jig_test_service::limits::Limits {
    jig_test_service::limits::for_profile(jig_protocol_config::Profile::Small, overrides)
}

// A whole story's frozen peer records: room for the largest story, about
// 16 times Adopt's 65 recorded observations, at the skein peer fixture's 2 KiB per record.
const STORY_OBSERVATIONS: u32 = 1_024;
const STORY_OBSERVATION_BYTES: u32 = 2_097_152;
pub(crate) fn peer_limits() -> skein_fake_peers::Limits {
    skein_fake_peers::Limits {
        io: skein_io::Limits {
            sockets: 16,
            names: 0,
            refusals: 8,
            intake: 65_536,
            receive: 1024,
            output: 65_536,
            sends: 8,
            accepts: 4,
            backlog: 8,
            close_timeout: Duration::from_secs(1),
            retry: Duration::from_millis(1),
        },
        connections: 4,
        queue: 64,
        plaintext: 32_768,
        ciphertext: 32_768,
        observations: STORY_OBSERVATIONS,
        observation_bytes: STORY_OBSERVATION_BYTES,
    }
}
/// The Forgejo protocol's limits. Bodies, documents and strings hold the
/// git route's whole trees of the domain the worlds share (256 files of
/// 64 KiB, about 22.5 MB as base64), as the peer requires (skein's
/// `git::fits`, and a body fits one string); a buffer's pages are resident
/// only as a call fills them.
pub(crate) fn forge_limits() -> skein_fake_forge_protocol::Limits {
    skein_fake_forge_protocol::Limits {
        calls: 4,
        connections: 4,
        raw_bytes: 4096,
        http: skein_http::server::Limits {
            head: 4096,
            headers: 16,
            body: 25_165_824,
            read: 1024,
            response: 4096,
            send: 1024,
        },
        documents: skein_forge::forgejo::limits::Limits {
            json: skein_json::tokenizer::Limits {
                chunk: 1024,
                string: 25_165_824,
                number: 32,
                depth: 16,
                length: 25_165_824,
            },
            writer: skein_json::writer::Limits { depth: 16, length: 25_165_824 },
            string_bytes: 512,
            body_bytes: 25_165_824,
            inner: 256,
            target_bytes: 4096,
        },
    }
}

/// Provision a deployment source through the native client before each fresh story.
pub(crate) fn account_steps() -> Box<[skein_fake_person::Step<temper_client_domain::Address>]> {
    use skein_fake_person::{Find, Step};
    use skein_ui::Role;
    Box::new([
        Step::Go { address: temper_client_domain::Address::ConnectModel { way: None } },
        Step::See { find: Find::named(Role::Button, b"Use deployment accounts"), within: PERSON_WAIT },
        Step::Press { find: Find::named(Role::Button, b"Use deployment accounts") },
        Step::Press { find: Find::named(Role::Link, b"Use an API key") },
        Step::Type { find: Find::named(Role::TextField, b"Key (pasted once)"), words: channel::KEY.into() },
        Step::Press { find: Find::named(Role::Button, b"Add key") },
        Step::See { find: Find::text(b"Added your"), within: PERSON_WAIT },
    ])
}
