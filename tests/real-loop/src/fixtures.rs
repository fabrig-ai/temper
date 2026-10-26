//! The process-tier documents adapted to a real loop on kernel-assigned ports.
use std::{
    io::Write,
    net::SocketAddr,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
};
#[path = "turn_bounds.rs"]
pub mod turn_bounds;
/// A second key the fake LLM peer accepts, for the accounts' stories.
pub const SECOND_KEY: &[u8] = b"real-loop-second-llm-key";
pub const TOKEN: &[u8] = jig_channel_world::fixtures::TOKEN;
pub const SEED: u64 = jig_channel_world::fixtures::SEED;
pub const AGENT: &[u8] = b"/real-loop/temper-agent";
#[must_use]
pub fn documents(directory: &Path, llm: SocketAddr, forge: SocketAddr) -> (serde_json::Value, serde_json::Value) {
    let program = std::str::from_utf8(AGENT).expect("agent program");
    let agent = serde_json::json!({
        "profile":{"name":"standard","declared":{"memory":2_147_483_648_u64,"tool_payload":2048,"read_window":256,
            "conversations":1,"calls_per_response":1},
            "policy":{"max_turns":8,"check_timeout":60000,"group_stop":10,"inbox":8,"unacknowledged":8}},
        "endpoints":[{"name":"fake","number":0,"dialect":3,"provider":"openai",
            "address":llm.to_string(),"transport":"plaintext",
            "identity":"plain","connect":1000,"handshake":1000,
            "models":[{"name":"fake","prices":{"input":1,"cached":1,"output":1,"unit":1_000_000},"window":1_048_576,"output":2048,"reasoning_item":64,"head":60000,"idle":30000}]}],
        "environment":[]});
    let mut value: serde_json::Value =
        serde_json::from_slice(&jig_channel_world::fixtures::engine_document()).expect("engine JSON");
    value.as_object_mut().expect("whole object").remove("service");
    let worker: serde_json::Value =
        serde_json::from_slice(jig_channel_world::fixtures::WORKER_DOCUMENT).expect("worker JSON");
    value["workers"] = worker["workers"].clone();
    value["workers"][0]["engine"] = serde_json::json!({"in_process":"workers"});
    value["workers"][0]["agent"]["program"] = program.into();
    value["workers"][0]["agent"]["configuration"] = directory.join("agent.json").to_str().expect("agent path").into();
    value["workers"][0]["agent"]["arguments"] = serde_json::json!(["agent", directory.join("agent.json")]);
    value["workers"][0]["limits"]["overrides"]["hub.accounts"] = 4.into();
    value["workers"][0]["limits"]["overrides"]["hub.event_bytes"] = 2046.into();
    value["engine"]["hosts"]["listener"] = serde_json::json!([{"in_process":"workers"}]);
    value["engine"]["connectors"] = serde_json::json!([1]);
    value["engine"]["llm"]["endpoints"][0]["dialect"] = 3.into();
    value["engine"]["llm"]["endpoints"][0]["address"] = serde_json::json!({"socket":llm.to_string()});
    value["engine"]["limits"]["overrides"]["core.load_slots"] = 8.into();
    value["engine"]["limits"]["overrides"]["documents.rows"] = 2.into();
    value["engine"]["limits"]["overrides"]["documents.lines"] = 2.into();
    value["engine"]["limits"]["overrides"]["documents.text_bytes"] = 128.into();
    value["engine"]["limits"]["overrides"]["people.sign_ins"] = 4.into();
    // Two concurrent acts each need one task and half the project purse.
    value["engine"]["limits"]["overrides"]["tasks.project_tasks"] = 2.into();
    value["engine"]["limits"]["overrides"]["views.watchers"] = 16.into();
    value["engine"]["limits"]["overrides"]["views.watchers_per_party"] = 16.into();
    value["application"] = serde_json::json!({
        "webhooks":null,
"clients":{"listener":{"socket":"127.0.0.1:0"},"public":null,"bundle":null},
        "forges":[{"number":1,"kind":"forgejo","api":{"socket":forge.to_string()},"transport":"plaintext",
            "git":format!("http://{forge}"),"webhook_secret":null,"api_token":"secrets/forge-1.api-token","user":{"file":"secrets/forge-1.user"},"identity":1}],
        "identities":[{"number":1,"token":"secrets/forge-1.git-token","login":"temper",
            "author":{"name":"Temper","email":"temper@example.test"}}],
        "setup":[],"demo":null,"sign_in":null,"landing":{"deployment":[],"projects":[]},"charter":1,
        "limits":{"profile":"small","overrides":{"checkout.name_bytes":256,"checkout.repositories":2,
            "checkout.message_bytes":4096,"forge.name_bytes":64,"forge.resources_per_task":2,"git_component.stdout_bytes":2048}},
        "workers":[{"root":"worker-root","git":"/usr/bin/git"}]});
    stock_document(&mut value);
    turn_bounds::apply(&mut value, turn_bounds::turn_body(&agent));
    (agent, value)
}

/// Offer the shipped sign-in way only to the stories hosting its fake issuer.
pub(crate) fn sign_in_documents(directory: &Path, issuer: SocketAddr) -> std::io::Result<()> {
    let path = directory.join("temper.json");
    let mut config: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    config["engine"]["llm"]["providers"] = serde_json::json!([{
        "name":"openai", "number":1, "signed_in_plan":true, "manage_usage":format!("https://{issuer}/settings/usage"), "models":["fake"],
        "sources":{"keys":true,"sign_in":crate::issuer::registration(issuer)}
    }]);
    private(&path, &serde_json::to_vec(&config)?)
}

/// Create the standalone tree; every private directory and file has its final mode.
pub fn write(directory: &Path, llm: SocketAddr, forge: SocketAddr) -> std::io::Result<()> {
    for name in ["", "accounts", "secrets", "state", "clients"] {
        std::fs::create_dir_all(directory.join(name))?;
        std::fs::set_permissions(directory.join(name), std::fs::Permissions::from_mode(0o700))?;
    }
    for (name, bytes) in jig_channel_world::fixtures::files(false).entries.into_boxed() {
        if !name.starts_with(b"charters/") {
            private(&directory.join(std::str::from_utf8(&name).expect("fixture path")), bytes.expose())?;
        }
    }
    private(&directory.join("worker.secret"), b"worker-secret")?;
    private(&directory.join("fixture.key"), jig_channel_world::fixtures::KEY)?;
    private(&directory.join("second.key"), SECOND_KEY)?;
    private(&directory.join("secrets/forge-1.user"), b"42\n")?;
    private(&directory.join("secrets/forge-1.api-token"), b"forge-1-api-token-value")?;
    private(&directory.join("secrets/forge-1.git-token"), b"forge-1-git-token-value")?;
    let (mut agent, mut config) = documents(directory, llm, forge);
    if crate::claude::configured(directory) {
        program_documents(directory, &mut agent, &mut config);
    }
    private(&directory.join("agent.json"), &serde_json::to_vec(&agent)?)?;
    private(&directory.join("temper.json"), &serde_json::to_vec(&config)?)
}
/// Program stories retain the direct provider and add Claude Code on its own source.
fn program_documents(directory: &Path, agent: &mut serde_json::Value, config: &mut serde_json::Value) {
    agent["programs"] = serde_json::json!({"claude_code": {
        "path": std::str::from_utf8(crate::claude::PROGRAM).expect("fake program path"),
        "directory": directory.join("programs")
    }});
    let mut endpoint = agent["endpoints"][0].clone();
    endpoint["name"] = "fake-claude".into();
    endpoint["number"] = 2.into();
    endpoint["dialect"] = 2.into();
    endpoint["provider"] = "anthropic".into();
    endpoint["program"] = "claude_code".into();
    endpoint["models"][0]["name"] = "fake-claude".into();
    agent["endpoints"].as_array_mut().expect("agent endpoints").push(endpoint);
    let llm = &mut config["engine"]["llm"];
    let mut endpoint = llm["endpoints"][0].clone();
    endpoint["name"] = "fake-claude".into();
    endpoint["number"] = 2.into();
    endpoint["dialect"] = 2.into();
    endpoint["provider"] = "anthropic".into();
    endpoint["program"] = "claude_code".into();
    llm["endpoints"].as_array_mut().expect("engine endpoints").push(endpoint);
    let mut model = llm["models"][0].clone();
    model["name"] = "fake-claude".into();
    model["endpoint"] = "fake-claude".into();
    llm["models"].as_array_mut().expect("engine models").push(model);
    llm["providers"].as_array_mut().expect("providers").push(serde_json::json!({
        "name":"anthropic", "number":2, "signed_in_plan":false,
        "manage_usage":"https://claude.ai/settings/usage", "models":["fake-claude"],
        "sources":{"programs":["claude_code"]}
    }));
    turn_bounds::apply(config, turn_bounds::turn_body(agent));
}
/// Keep the configured documents and their derived bounds; only assigned endpoints change.
pub(crate) fn bind(directory: &Path, llm: SocketAddr, forge: SocketAddr) -> std::io::Result<()> {
    let mut agent: serde_json::Value = serde_json::from_slice(&std::fs::read(directory.join("agent.json"))?)?;
    let mut config: serde_json::Value = serde_json::from_slice(&std::fs::read(directory.join("temper.json"))?)?;
    bind_documents(&mut agent, &mut config, llm, forge);
    private(&directory.join("agent.json"), &serde_json::to_vec(&agent)?)?;
    private(&directory.join("temper.json"), &serde_json::to_vec(&config)?)
}

fn bind_documents(agent: &mut serde_json::Value, config: &mut serde_json::Value, llm: SocketAddr, forge: SocketAddr) {
    let previous = config["application"]["forges"][0]["api"]["socket"].as_str().expect("fixture forge").to_owned();
    let address = forge.to_string();
    let previous_http = format!("http://{previous}");
    let http = format!("http://{address}");
    // These records carry forge destinations and authority patterns. Listeners
    // and the independent LLM destination never participate in this rebinding.
    for path in ["/application/forges", "/application/landing", "/engine/rules", "/engine/projects", "/engine/policies"]
    {
        if let Some(value) = config.pointer_mut(path) {
            bind_forge(value, [(&previous, &address), (&previous_http, &http)]);
        }
    }
    agent["endpoints"][0]["address"] = llm.to_string().into();
    config["engine"]["llm"]["endpoints"][0]["address"] = serde_json::json!({"socket":llm.to_string()});
}

fn bind_forge(value: &mut serde_json::Value, addresses: [(&str, &str); 2]) {
    use serde_json::Value;
    match value {
        Value::String(value) => {
            for (previous, address) in addresses {
                if value == previous {
                    *value = address.into();
                    break;
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                bind_forge(value, addresses);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                bind_forge(value, addresses);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

pub(crate) fn private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)?.write_all(bytes)
}

/// The tools' executable is hosted once per call, with two concurrent slots.
pub const CTL: &[u8] = b"/real-loop/temper-ctl";
pub const CALLS_AT_ONCE: u32 = 2;
#[must_use]
pub fn directory(scratch: &Path, at: crate::calls::Dir) -> std::path::PathBuf {
    scratch.join("clients").join(format!("client-{}", at.0))
}
pub(crate) fn ctl_roots(spawn: &skein_io::kernel::Spawn) -> Vec<skein_world::StartupRoot> {
    use std::os::unix::ffi::OsStrExt;
    let directory = Path::new(std::ffi::OsStr::from_bytes(&spawn.args[0]));
    let mut roots = vec![skein_world::StartupRoot {
        name: b"parent".as_slice().into(),
        path: directory.parent().expect("client parent").as_os_str().as_bytes().into(),
    }];
    let parsed =
        temper_client_shell_cli::arguments::parse(&spawn.args[1..].iter().map(AsRef::as_ref).collect::<Vec<_>>())
            .expect("story arguments");
    if let Some(path) = temper_client_shell_cli::arguments::file_root(&parsed) {
        roots.push(skein_world::StartupRoot { name: b"file_parent".as_slice().into(), path });
    }
    for (number, path) in temper_client_shell_cli::arguments::text_files(&parsed) {
        roots.push(skein_world::StartupRoot { name: format!("text-{number}").into_bytes().into(), path });
    }
    roots
}
pub(crate) fn ctl(spawn: &skein_io::kernel::Spawn, inherited: &skein_world::Inherited) -> crate::world::Proc {
    use std::os::unix::ffi::OsStrExt;
    use temper_client_shell_cli::{
        arguments,
        host::{Invocation, OneShot, Startup},
    };
    let directory = Path::new(std::ffi::OsStr::from_bytes(&spawn.args[0]));
    let pipe = |child| inherited.pipes.iter().find(|(number, _)| *number == child).expect("standard pipe").1;
    crate::world::Proc::Ctl(Box::new(crate::calls::Recorded::new(
        OneShot::with_text_files(
            Invocation {
                directory: Some(directory.file_name().expect("directory leaf").as_bytes().into()),
                parsed: arguments::parse(&spawn.args[1..].iter().map(AsRef::as_ref).collect::<Vec<_>>())
                    .expect("story arguments"),
            },
            Startup {
                parent: Some(inherited.roots[0].1),
                file_parent: inherited
                    .roots
                    .iter()
                    .find(|(name, _)| name.as_ref() == b"file_parent")
                    .map(|(_, fd)| *fd),
                stdin: pipe(0),
                stdout: pipe(1),
                stderr: pipe(2),
                signals: inherited.signal,
                user: skein_shell::effective_user(),
                seed: SEED,
            },
            inherited
                .roots
                .iter()
                .filter_map(|(name, fd)| {
                    name.strip_prefix(b"text-").map(|number| {
                        (std::str::from_utf8(number).expect("text argument").parse().expect("text argument"), *fd)
                    })
                })
                .collect(),
        )
        .expect("shipped tools limits"),
        std::str::from_utf8(spawn.env[0].strip_prefix(b"CALL=").expect("call binding"))
            .expect("call index")
            .parse()
            .expect("call index"),
        spawn.env.get(1).is_some_and(|value| value.as_ref() == b"FIXTURE=true"),
    )))
}

/// The project supplied by the standalone engine fixture.
pub const PROJECT: &str = "1";

/// A signed-in chat is the common opener for tools stories.
#[must_use]
pub fn signed_chat(key: &'static str) -> Vec<crate::calls::Step> {
    use crate::{
        argv,
        calls::{D1, call, sign_in},
        scripts,
    };
    vec![sign_in(D1), call(D1, argv!["chat", "--project", PROJECT, "--key", key, scripts::WORDS])]
}

/// Both directories ask for the same named request.
pub const LOST_NAME: &str = "lost-answer";
/// Sign-in is call zero, so the started request is call one.
pub const LOST_CALL: usize = 1;

/// Stop a sending call only after its named request is durably held.
#[must_use]
pub fn lost_chat() -> Vec<crate::calls::Step> {
    use crate::calls::{Step, held, kill, start};
    let mut steps = signed_chat(LOST_NAME);
    let Step::Call { at, argv } = steps.pop().expect("chat opener") else { panic!("chat call") };
    steps.extend([start(at, argv), held(LOST_NAME.as_bytes()), kill(LOST_CALL)]);
    steps
}

/// The key smoke uses the standard `OpenAI` route rather than the Codex account route.
pub(crate) fn key_documents(directory: &Path) -> std::io::Result<()> {
    let path = directory.join("agent.json");
    let mut agent: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    agent["endpoints"][0]["provider"] = "openai".into();
    agent["endpoints"][0]["dialect"] = 3.into();
    let config_path = directory.join("temper.json");
    let mut config: serde_json::Value = serde_json::from_slice(&std::fs::read(&config_path)?)?;
    config["engine"]["llm"]["endpoints"][0]["dialect"] = 3.into();
    config["engine"]["limits"]["overrides"]["people.projects"] = 2.into();
    config["engine"]["limits"]["overrides"]["authority.projects"] = 2.into();
    // Each project needs its period and the owner's pool in the tasks ledger.
    config["engine"]["limits"]["overrides"]["tasks.funders"] = 4.into();
    private(&config_path, &serde_json::to_vec(&config)?)?;
    private(&path, &serde_json::to_vec(&agent)?)
}

/// One owner transaction with whole values, after sign-in and before the story.
#[must_use]
pub fn change_settings(at: crate::calls::Dir, key: &'static str, items: serde_json::Value) -> Vec<crate::calls::Step> {
    let mut file = serde_json::json!({});
    file["items"] = items;
    let bytes = serde_json::to_vec(&file).expect("settings FILE").into_boxed_slice();
    PENDING_SETTINGS.with(|slot| {
        let mut slot = slot.borrow_mut();
        let files = slot.get_or_insert_with(|| skein_lib::Map::with_capacity(16));
        assert!(
            files.insert(key, bytes).expect("at most sixteen setup settings files").is_none(),
            "unique setup settings keys"
        );
    });
    vec![crate::calls::call(
        at,
        crate::argv!["change-settings", crate::calls::file("settings.json"), "--against", "1", "--key", key],
    )]
}
// A step factory runs immediately before Caller construction, under its heap meter.
// Move its bounded FILE table into that Caller; no global file state is retained.
pub(crate) type SettingsFiles = skein_lib::Map<&'static str, Box<[u8]>>;
thread_local! { static PENDING_SETTINGS: std::cell::RefCell<Option<SettingsFiles>> = const { std::cell::RefCell::new(None) }; }
pub(crate) fn take_settings() -> SettingsFiles {
    PENDING_SETTINGS.with(|slot| slot.borrow_mut().take().unwrap_or_else(|| skein_lib::Map::with_capacity(0)))
}

fn stock_document(value: &mut serde_json::Value) {
    for member in ["charters", "templates", "policies", "rules"] {
        value["engine"].as_object_mut().expect("engine").remove(member);
    }
    for project in value["engine"]["projects"].as_array_mut().expect("projects") {
        project["policy"]["ceiling"]["delegation"]["depth"] = 4.into();
        for role in project["policy"]["roles"].as_array_mut().expect("roles") {
            role["authority"]["delegation"]["depth"] = 4.into();
        }
    }
    for (name, count) in
        [("core.policy_bytes", 4096), ("authority.grants", 16), ("notes.scopes", 32), ("tasks.charters", 2)]
    {
        let held = value["engine"]["limits"]["overrides"][name].as_u64().unwrap_or(0);
        value["engine"]["limits"]["overrides"][name] = held.max(count).into();
    }
}
/// Existing callbacks for blocked and reference-case fixtures still edit document defaults.
/// Stage their original values; only those final projects exceeding stock keep them.
pub(crate) fn document_defaults(value: &mut serde_json::Value) {
    if value["engine"].get("charters").is_some() {
        return;
    }
    let original: serde_json::Value =
        serde_json::from_slice(&jig_channel_world::fixtures::engine_document()).expect("original fixture");
    for member in ["charters", "templates", "policies", "rules"] {
        if let Some(member_value) = original["engine"].get(member) {
            value["engine"][member] = member_value.clone();
        }
    }
    value["engine"]["charters"][0]["waiting_ms"] = 1.into();
    value["engine"]["templates"][0]["grace_ms"] = 1.into();
    value["engine"]["templates"][0]["budget"] = serde_json::json!({"as_allowed":50});
}
fn beyond_stock(value: &serde_json::Value) -> bool {
    value["engine"]["projects"].as_array().expect("projects").iter().any(|project| {
        let ceiling = &project["policy"]["ceiling"];
        ceiling["tools"].as_u64().expect("tools") & !1 != 0
            || ceiling["notes"].as_u64().expect("notes") != 0
            || ceiling["delegation"]["depth"].as_u64().expect("depth") > 4
            || ceiling["delegation"]["kinds"].as_array().expect("kinds").iter().any(|kind| kind["kind"] == "role")
    })
}
/// The QA conversation uses stock's requester ending.
pub fn requester(value: &mut serde_json::Value) {
    value["world_settings"]["requester"] = true.into();
}
/// The QA person follows the chat on their page.
pub fn listed(value: &mut serde_json::Value) {
    value["world_settings"]["listed"] = true.into();
}
pub(crate) fn finish_configuration(directory: &Path, value: &mut serde_json::Value) -> std::io::Result<()> {
    let flags = value.as_object_mut().expect("configuration").remove("world_settings").unwrap_or(serde_json::json!({}));
    if beyond_stock(value) {
        std::fs::create_dir_all(directory.join("charters"))?;
        std::fs::set_permissions(directory.join("charters"), std::fs::Permissions::from_mode(0o700))?;
        for (name, bytes) in jig_channel_world::fixtures::files(false).entries.into_boxed() {
            if name.starts_with(b"charters/") {
                private(&directory.join(std::str::from_utf8(&name).expect("charter path")), bytes.expose())?;
            }
        }
    } else {
        if directory.join("charters").exists() {
            std::fs::remove_dir(directory.join("charters"))?;
        }
        stock_document(value);
        private(&directory.join("world-settings.json"), &serde_json::to_vec(&flags)?)?;
    }
    private(&directory.join("temper.json"), &serde_json::to_vec(value)?)
}
pub(crate) fn setup_items(directory: &Path) -> Option<serde_json::Value> {
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(directory.join("temper.json")).expect("deployment document"))
            .expect("deployment JSON");
    if value["engine"].get("charters").is_some() {
        return None;
    }
    let flags = if directory.join("world-settings.json").exists() {
        serde_json::from_slice(&std::fs::read(directory.join("world-settings.json")).expect("fixture flags"))
            .expect("fixture flags JSON")
    } else {
        serde_json::json!({})
    };
    let stock = crate::settings::stock(&value);
    let mut charter = stock["charter"].clone();
    charter["waiting"] = 1_000_000.into();
    let mut template = stock["template"].clone();
    template["grace"] = 1_000_000.into();
    template["budget"]["as_allowed"]["cap"] = 50.into();
    if flags["requester"] != true {
        template["flags"]["ended_by"] = "result".into();
    }
    if flags["listed"] == true {
        template["flags"]["listed"] = true.into();
    }
    Some(serde_json::json!([
        crate::settings::item("charter", 1, charter),
        crate::settings::item("template", 1, template)
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn configured(
        fixture: crate::forge::Fixture,
        llm: SocketAddr,
        forge: SocketAddr,
    ) -> (serde_json::Value, serde_json::Value) {
        let (mut agent, mut config) = documents(Path::new("/fixture"), llm, forge);
        (fixture.configure)(&mut config, &mut agent, forge);
        if !beyond_stock(&config) {
            stock_document(&mut config);
        }
        turn_bounds::apply(&mut config, turn_bounds::turn_body(&agent));
        (agent, config)
    }

    #[test]
    fn rebinding_preserves_the_whole_configured_documents() {
        const LLM_PORT: u16 = 42_001;
        const FORGE_PORT: u16 = 42_002;
        let deferred = ([127, 0, 0, 1], 0).into();
        let llm = ([127, 0, 0, 1], LLM_PORT).into();
        let forge = ([127, 0, 0, 1], FORGE_PORT).into();
        for fixture in [crate::forge::fixture(), crate::landing::fixture(), crate::case::fixture_6_5()] {
            let (mut agent, mut config) = configured(fixture, deferred, deferred);
            bind_documents(&mut agent, &mut config, llm, forge);
            assert_eq!((agent, config), configured(fixture, llm, forge));
        }
    }
}
