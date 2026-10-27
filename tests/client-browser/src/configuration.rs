//! The process tier's deployment, copied: `documents` from `crates/temper/tests/support/configuration.rs`
//! and `write` from `support::configuration` with `Forge::Unreached`, which are test modules of package
//! `temper` and cannot be imported. Three changes: the LLM endpoint is unreached at `127.0.0.1:9`, beside
//! the forge; `application.clients.bundle` names the bundle; and every relative path is made absolute
//! under the scratch, as jig's `tests/client-browser` does, since `temper serve` joins paths to its working
//! directory and the harness starts it in the test's.
#[path = "../../real-loop/src/turn_bounds.rs"]
mod turn_bounds;
use std::{fs, os::unix::fs::PermissionsExt, path::Path};

/// A closed loopback port: the forge's and the LLM endpoint's, which no story reaches.
pub(crate) const UNREACHED: &str = "127.0.0.1:9";
/// The paths the deployment names, each relative to its directory.
const RELATIVE: &[&str] = &[
    "secrets/worker-a.digest",
    "app.secret",
    "worker.secret",
    "secrets/forge-1.user",
    "secrets/forge-1.api-token",
    "secrets/forge-1.git-token",
    "state",
    "worker-root",
];

pub(crate) fn documents(directory: &Path, program: &Path, bundle: &Path) -> (Box<[u8]>, Box<[u8]>) {
    let agent = serde_json::json!({
        "profile":{"name":"standard","declared":{"memory":2_147_483_648_u64,"tool_payload":2048,"read_window":256,
            "conversations":1,"calls_per_response":1},
            "policy":{"max_turns":8,"group_stop":10,"inbox":8,"unacknowledged":8,"check_timeout":60000}},
        "endpoints":[{"name":"fake","number":0,"dialect":3,"provider":"openai",
            "address":UNREACHED,"transport":"tls","server_name":"skein.test","trust_der":directory.join("root.der"),
            "identity":"plain","connect":1000,"handshake":1000,
            "models":[{"name":"fake","prices":{"input":1,"cached":1,"output":1,"unit":1_000_000},"window":1_048_576,"output":2048,"reasoning_item":64,"head":60000,"idle":30000}]}],
        "environment":[]});
    let mut value: serde_json::Value =
        serde_json::from_slice(&jig_channel_world::fixtures::engine_document()).expect("engine JSON");
    value.as_object_mut().expect("whole object").remove("service");
    for member in ["charters", "templates", "policies", "rules"] {
        value["engine"].as_object_mut().expect("engine object").remove(member);
    }
    // The inherited project's unused depth eight exceeded the stock's four.
    for project in value["engine"]["projects"].as_array_mut().expect("seed projects") {
        project["policy"]["ceiling"]["delegation"]["depth"] = 4.into();
        for role in project["policy"]["roles"].as_array_mut().expect("seed roles") {
            role["authority"]["delegation"]["depth"] = 4.into();
        }
    }
    // Admit stock policy 1 and its ten grants.
    for (name, count) in
        [("core.policy_bytes", 4096), ("authority.grants", 16), ("notes.scopes", 32), ("tasks.charters", 4)]
    {
        value["engine"]["limits"]["overrides"][name] = count.into();
    }
    let worker: serde_json::Value =
        serde_json::from_slice(jig_channel_world::fixtures::WORKER_DOCUMENT).expect("worker JSON");
    value["workers"] = worker["workers"].clone();
    value["workers"][0]["engine"] = serde_json::json!({"in_process":"workers"});
    value["workers"][0]["agent"]["program"] = program.to_str().expect("program path").into();
    value["workers"][0]["agent"]["configuration"] = directory.join("agent.json").to_str().expect("agent path").into();
    value["workers"][0]["agent"]["arguments"] = serde_json::json!(["internal", "agent", directory.join("agent.json")]);
    value["workers"][0]["limits"]["overrides"]["hub.accounts"] = 4.into();
    value["workers"][0]["limits"]["overrides"]["hub.event_bytes"] = 2046.into();
    value["engine"]["hosts"]["listener"] = serde_json::json!([{"in_process":"workers"}]);
    value["engine"]["connectors"] = serde_json::json!([1]);
    value["engine"]["llm"]["endpoints"][0]["dialect"] = 3.into();
    value["engine"]["llm"]["endpoints"][0]["address"] = serde_json::json!({"socket":UNREACHED});
    value["engine"]["limits"]["overrides"]["core.load_slots"] = 8.into();
    value["engine"]["limits"]["overrides"]["documents.rows"] = 2.into();
    value["engine"]["limits"]["overrides"]["documents.lines"] = 2.into();
    value["engine"]["limits"]["overrides"]["documents.text_bytes"] = 128.into();
    value["engine"]["limits"]["overrides"]["people.sign_ins"] = 2.into();
    value["engine"]["limits"]["overrides"]["views.watchers"] = 16.into();
    value["engine"]["limits"]["overrides"]["views.watchers_per_party"] = 16.into();
    value["application"] = serde_json::json!({
        "webhooks":null,
        "clients":{"listener":{"socket":"127.0.0.1:0"},"public":null,"bundle":bundle},
        "forges":[{"number":1,"kind":"forgejo","api":{"socket":UNREACHED},"transport":"plaintext",
            "git":format!("http://{UNREACHED}"),"webhook_secret":null,"api_token":"secrets/forge-1.api-token","user":{"file":"secrets/forge-1.user"},"identity":1}],
        "identities":[{"number":1,"token":"secrets/forge-1.git-token","login":"temper",
            "author":{"name":"Temper","email":"temper@example.test"}}],
        "setup":[],"demo":null,"sign_in":null,"landing":{"deployment":[],"projects":[]},"charter":1,
        "limits":{"profile":"small","overrides":{"checkout.name_bytes":256,"checkout.repositories":2,
            "checkout.message_bytes":4096,"forge.name_bytes":64,"forge.resources_per_task":2,"git_component.stdout_bytes":2048}},
        "workers":[{"root":"worker-root","git":"/usr/bin/git"}]});
    turn_bounds::apply(&mut value, turn_bounds::turn_body(&agent));
    absolute_paths(&mut value, directory);
    (
        serde_json::to_vec(&agent).expect("agent JSON").into_boxed_slice(),
        serde_json::to_vec(&value).expect("deployment JSON").into_boxed_slice(),
    )
}
/// Every string that is one of the deployment's relative paths becomes that path under `directory`.
fn absolute_paths(value: &mut serde_json::Value, directory: &Path) {
    match value {
        serde_json::Value::String(text) if RELATIVE.contains(&text.as_str()) => {
            directory.join(&*text).to_str().expect("fixture path").clone_into(text);
        }
        serde_json::Value::Array(items) => {
            for item in items {
                absolute_paths(item, directory);
            }
        }
        serde_json::Value::Object(items) => {
            for item in items.values_mut() {
                absolute_paths(item, directory);
            }
        }
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => {}
    }
}
pub(crate) fn private(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).expect("fixture file");
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).expect("private fixture");
}
/// The deployment's files and its two documents, `agent.json` and `temper.json`, in `directory`.
pub(crate) fn write(directory: &Path, program: &Path, bundle: &Path) {
    assert!(Path::new("/usr/bin/git").is_file(), "/usr/bin/git is required");
    for name in ["", "accounts", "secrets", "state"] {
        fs::create_dir_all(directory.join(name)).expect("deployment directory");
        fs::set_permissions(directory.join(name), fs::Permissions::from_mode(0o700)).expect("private directory");
    }
    for (name, bytes) in jig_channel_world::fixtures::files(false).entries.into_boxed() {
        if name.starts_with(b"charters/") {
            continue;
        }
        private(&directory.join(std::str::from_utf8(&name).expect("fixture path")), bytes.expose());
    }
    private(&directory.join("worker.secret"), b"worker-secret");
    private(&directory.join("secrets/forge-1.user"), b"42\n");
    private(&directory.join("secrets/forge-1.api-token"), b"forge-1-api-token-value");
    private(&directory.join("secrets/forge-1.git-token"), b"forge-1-git-token-value");
    private(&directory.join("root.der"), skein_tls_world::pki::ROOT);
    let (agent, value) = documents(directory, program, bundle);
    private(&directory.join("agent.json"), &agent);
    private(&directory.join("temper.json"), &value);
}

/// The three directories share the scratch root, as the process tier's flags do.
pub(crate) fn flags(directory: &Path) -> Vec<std::ffi::OsString> {
    ["--config", "--data", "--cache"].into_iter().flat_map(|flag| [flag.into(), directory.into()]).collect()
}

/// Let init write its first-run deployment, then name the gate's bundle and reserve every loopback port.
pub(crate) fn demo(directory: &Path, program: &Path, bundle: &Path) {
    fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).expect("private demo directory");
    let initialized =
        std::process::Command::new(program).arg("init").args(flags(directory)).output().expect("initialize the demo");
    assert!(initialized.status.success(), "demo init: {}", String::from_utf8_lossy(&initialized.stderr));
    let path = directory.join("temper.json");
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).expect("demo deployment")).expect("JSON");
    let sockets = [
        &value["application"]["clients"]["listener"]["socket"],
        &value["application"]["forges"][0]["api"]["socket"],
        &value["application"]["demo"]["llm"]["listen"]["socket"],
        &value["application"]["webhooks"]["listener"]["socket"],
    ];
    // Keep all reservations until both documents are written, so the kernel cannot draw one twice.
    let listeners = sockets.map(|_| std::net::TcpListener::bind("127.0.0.1:0").expect("reserve demo port"));
    let replacements = std::array::from_fn::<_, 4, _>(|index| {
        (
            sockets[index].as_str().expect("demo socket").to_owned(),
            listeners[index].local_addr().expect("demo address").to_string(),
        )
    });
    replace_sockets(&mut value, &replacements);
    value["application"]["clients"]["bundle"] = serde_json::json!(bundle);
    private(&path, &serde_json::to_vec(&value).expect("demo JSON"));
    let path = directory.join("agent.json");
    let mut agent = serde_json::from_slice(&fs::read(&path).expect("demo agent")).expect("JSON");
    replace_sockets(&mut agent, &replacements);
    private(&path, &serde_json::to_vec(&agent).expect("demo agent JSON"));
}

/// Update references to a reserved listener in both documents, including the forge's git URL.
fn replace_sockets(value: &mut serde_json::Value, replacements: &[(String, String)]) {
    match value {
        serde_json::Value::String(text) => {
            for (before, after) in replacements {
                if text == before {
                    text.clone_from(after);
                    break;
                }
                if text.strip_prefix("http://") == Some(before.as_str()) {
                    *text = format!("http://{after}");
                    break;
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                replace_sockets(item, replacements);
            }
        }
        serde_json::Value::Object(items) => {
            for item in items.values_mut() {
                replace_sockets(item, replacements);
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
    }
}
