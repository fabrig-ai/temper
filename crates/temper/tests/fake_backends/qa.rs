//! Design-only adversarial STEP1 QA; see protocol/configuration.md, 12.1.
//! These stories use only the shipped CLI, its files and the existing box/ctl world.
//! No OAuth exchange, automatic peer supervision, owned Forgejo or download is needed.
use crate::support::{self, boxed, ctl};
use serde_json::{Value, json};
use skein_io::kernel::Exit;
use skein_lib::Duration;
use skein_world::end_to_end::Command;
use std::{
    ffi::OsString,
    fs,
    net::SocketAddr,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
};

const WITHIN: Duration = Duration::from_secs(30);
// The stable init identities and five issuer URLs of configuration.md, 12.1.
const GITHUB_FORGE: u16 = 2;
const OPENAI_PROVIDER: u16 = 1;
const OPENAI_ENDPOINT: &str = "openai";
const ISSUER_URLS: usize = 5;
// Each live story signs in, then reads accounts.
const CTL_CALLS: usize = 2;
const DIGEST: &str = "0123456789abcdef";
const PUBLIC: [&str; 4] = ["temper.json", "agent.json", "charters/chat.txt", "charters/coding.txt"];
const PRIVATE: [&str; 5] =
    ["worker.secret", "worker.digest", "demo/llm.key", "deployment/qa-source-record", "programs/qa-source-login"];

fn directory(path: &Path) {
    fs::DirBuilder::new().recursive(true).mode(0o700).create(path).expect("private fixture directory");
}

fn write(path: &Path, bytes: &[u8]) {
    directory(path.parent().expect("fixture parent"));
    fs::write(path, bytes).expect("fixture bytes");
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).expect("private fixture file");
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).expect("public configuration bytes")).expect("public configuration JSON")
}

fn write_json(path: &Path, value: &Value) {
    write(path, &serde_json::to_vec_pretty(value).expect("fixture JSON"));
}

fn bundle(path: &Path) {
    write(&path.join("index.html"), b"<!doctype html><html><body>STEP1 QA fixture</body></html>");
    write(&path.join(DIGEST).join("qa.js"), b"// inert fixture; never executed\n");
    write(&path.join("robots.txt"), b"User-agent: *\nDisallow: /\n");
}

fn flags(config: &Path, data: &Path, cache: &Path) -> Vec<String> {
    [
        "--config",
        config.to_str().expect("fixture path"),
        "--data",
        data.to_str().expect("fixture path"),
        "--cache",
        cache.to_str().expect("fixture path"),
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn command(root: &Path, verb: &str, arguments: Vec<String>) -> boxed::Outcome {
    boxed::run_command(
        Command {
            program: env!("CARGO_BIN_EXE_temper").into(),
            arguments: std::iter::once(OsString::from(verb)).chain(arguments.into_iter().map(OsString::from)).collect(),
            directory: root.into(),
            environment: Vec::new(),
        },
        None,
        boxed::Plan::Stop,
        WITHIN,
    )
}

fn drained(exit: Option<Exit>, left: &Option<Vec<u32>>, stderr: &[u8]) {
    assert_eq!(exit, Some(Exit::Code(0)), "{}", String::from_utf8_lossy(stderr));
    assert_eq!(left.as_deref(), Some([].as_slice()), "no child outlives its box");
}

fn refused(outcome: &boxed::Outcome, member: &str) {
    assert!(
        matches!(outcome.exit, Some(Exit::Code(code)) if code != 0),
        "must refuse without timing out: {:?}; {}",
        outcome.exit,
        String::from_utf8_lossy(&outcome.stderr)
    );
    assert_eq!(outcome.left.as_deref(), Some([].as_slice()), "a refusal leaves no child");
    let diagnostic = String::from_utf8_lossy(&outcome.stderr).to_lowercase();
    assert!(diagnostic.contains(member), "refusal names {member}: {diagnostic}");
}

struct Fixture {
    scratch: support::Scratch,
    config: PathBuf,
    data: PathBuf,
    cache: PathBuf,
}

impl Fixture {
    fn new(case: &str) -> Self {
        let scratch = support::Scratch::new(case);
        let config = scratch.path().join("source config");
        let data = scratch.path().join("source data");
        let cache = scratch.path().join("shared cache");
        let initialized = command(scratch.path(), "init", flags(&config, &data, &cache));
        drained(initialized.exit, &initialized.left, &initialized.stderr);

        // The ordinary gate embeds no bundle. Supply only the bounded file shape,
        // not WASM or a policy detector: runtime does not inspect navigation policy.
        let browser = config.join("qa bundle");
        bundle(&browser);
        let mut document = read_json(&config.join("temper.json"));
        document["application"]["clients"]["bundle"] = json!(browser);

        // Init releases its preferred ports. Give the two live listeners fresh
        // addresses, using the public harness, so independent QA cases do not
        // compete for the preferred clients/webhooks ports.
        let old: [SocketAddr; 2] = [
            document["application"]["clients"]["listener"]["socket"]
                .as_str()
                .expect("clients socket")
                .parse()
                .expect("clients address"),
            document["application"]["webhooks"]["listener"]["socket"]
                .as_str()
                .expect("webhooks socket")
                .parse()
                .expect("webhooks address"),
        ];
        let fresh = support::free_addresses::<2>(&old);
        document["application"]["clients"]["listener"] = json!({"socket": fresh[0].to_string()});
        document["application"]["webhooks"]["listener"] = json!({"socket": fresh[1].to_string()});
        write_json(&config.join("temper.json"), &document);

        // Synthetic source records and program-login state must neither change
        // nor appear under D'. These are never a user's records or login.
        write(&data.join("deployment/qa-source-record"), b"qa-source-record-must-not-import\n");
        write(&data.join("programs/qa-source-login"), b"qa-source-login-must-not-import\n");
        Self { scratch, config, data, cache }
    }

    fn effective(&self) -> PathBuf {
        self.data.join("box/fake-backends")
    }

    fn fake_flags(&self) -> Vec<String> {
        let mut arguments = flags(&self.config, &self.data, &self.cache);
        arguments.push("--fake-backends".into());
        arguments
    }

    fn start(&self) -> boxed::Outcome {
        command(self.scratch.path(), "box", self.fake_flags())
    }

    fn live<T>(&self, check: impl FnOnce() -> T) -> boxed::CtlOutcome<T> {
        boxed::run_ctl(
            self.scratch.path(),
            self.fake_flags(),
            self.scratch.path(),
            vec![ctl::Step::Run(ctl::Call::sign_in()), ctl::Step::Run(ctl::Call::new(&["accounts", "--json"]))],
            WITHIN,
            check,
        )
    }

    fn snapshot(&self) -> Vec<(PathBuf, Vec<u8>)> {
        PUBLIC
            .iter()
            .map(|name| self.config.join(name))
            .chain(PRIVATE.iter().map(|name| self.data.join(name)))
            .chain([self.config.join("qa bundle/index.html"), self.config.join("qa bundle").join(DIGEST).join("qa.js")])
            .filter(|path| path.exists())
            .map(|path| {
                let bytes = fs::read(&path).expect("fixture snapshot");
                (path, bytes)
            })
            .collect()
    }
}

fn untouched(snapshot: &[(PathBuf, Vec<u8>)]) {
    for (path, bytes) in snapshot {
        assert_eq!(&fs::read(path).expect("source retained"), bytes, "source changed: {}", path.display());
    }
}

fn successful<T>(outcome: &boxed::CtlOutcome<T>) {
    drained(outcome.exit, &outcome.left, &outcome.stderr);
    assert!(outcome.checked.is_some(), "live check reached: {}", String::from_utf8_lossy(&outcome.stderr));
    assert_eq!(outcome.calls.len(), CTL_CALLS, "sign-in and accounts completed");
    for call in &outcome.calls {
        assert_eq!(call.exit, Some(Exit::Code(0)), "{}", String::from_utf8_lossy(&call.stderr));
    }
    let _: Value = serde_json::from_slice(&outcome.calls[1].stdout).expect("accounts reached through the serve child");
}

fn row<'a>(table: &'a Value, member: &str, target: &Value) -> &'a Value {
    let rows: Vec<_> =
        table.as_array().expect("configuration table").iter().filter(|row| row[member] == *target).collect();
    assert_eq!(rows.len(), 1, "exactly one {member}={target}");
    rows[0]
}

fn socket(value: &Value) -> SocketAddr {
    let address = value.as_str().or_else(|| value["socket"].as_str()).expect("socket address");
    let address: SocketAddr = address.parse().expect("literal socket");
    assert!(address.ip().is_loopback() && address.port() != 0, "nonzero loopback peer");
    address
}

fn rebase_issuer(value: &mut Value, origin: &str) -> usize {
    match value {
        Value::String(text) => {
            if let Some(path) = text.strip_prefix("https://auth.openai.com") {
                *text = format!("{origin}{path}");
                1
            } else {
                0
            }
        }
        Value::Array(values) => values.iter_mut().map(|value| rebase_issuer(value, origin)).sum(),
        Value::Object(values) => {
            if values.contains_key("transport") {
                values.insert("transport".into(), json!("plaintext"));
                values.remove("server_name");
            }
            values.values_mut().map(|value| rebase_issuer(value, origin)).sum()
        }
        _ => 0,
    }
}

fn issuer_origin<'a>(source: &Value, effective: &'a Value) -> Option<&'a str> {
    match source {
        Value::String(text) if text == "https://auth.openai.com" => effective.as_str(),
        Value::Object(values) => values
            .iter()
            .find_map(|(key, source)| effective.get(key).and_then(|effective| issuer_origin(source, effective))),
        Value::Array(values) => values
            .iter()
            .enumerate()
            .find_map(|(index, source)| effective.get(index).and_then(|effective| issuer_origin(source, effective))),
        _ => None,
    }
}

#[test]
fn qa_split_directories_isolate_source_records_and_rewrite_every_public_root() {
    let fixture = Fixture::new("qa-fake-backends-isolation");
    let original = fixture.snapshot();
    let source = read_json(&fixture.config.join("temper.json"));
    let effective = fixture.effective();
    let outcome = fixture.live(|| {
        let document = read_json(&effective.join("config/temper.json"));
        let address = socket(&document["application"]["clients"]["listener"]);
        let served = boxed::status(address, &format!("/bundle/{DIGEST}/qa.js"));
        (document, read_json(&effective.join("config/agent.json")), served)
    });
    successful(&outcome);
    untouched(&original);
    let (document, agent, served) = outcome.checked.expect("effective documents read while live");
    assert_eq!(served, Some(200), "child serves the explicitly provisioned fixture bundle");
    assert!(effective.join("prepared").is_file(), "published preparation survives shutdown");
    assert_eq!(fs::metadata(&effective).expect("sandbox").permissions().mode() & 0o777, 0o700);
    assert!(fs::read_dir(&fixture.cache).expect("cache").next().is_none(), "welcome needs no fetch");
    for name in ["worker.secret", "worker.digest", "demo/llm.key"] {
        assert_ne!(
            fs::read(fixture.data.join(name)).expect("source fixture"),
            fs::read(effective.join("data").join(name)).expect("fresh sandbox secret"),
            "never import {name}"
        );
    }
    for name in ["deployment/qa-source-record", "programs/qa-source-login"] {
        assert!(!effective.join("data").join(name).exists(), "source state is not copied");
    }
    let worker = &document["workers"][0]["agent"];
    assert_eq!(worker["configuration"], json!(effective.join("config/agent.json")));
    assert_eq!(worker["arguments"], json!(["agent", effective.join("config/agent.json")]));
    assert_eq!(worker["program"], source["workers"][0]["agent"]["program"]);

    let github = row(&document["application"]["forges"], "number", &json!(GITHUB_FORGE));
    let forge = socket(&github["api"]);
    assert_eq!(github["transport"], "plaintext");
    assert_eq!(github["git"], format!("http://{forge}"));
    assert_eq!(github["web"], format!("http://{forge}"));
    let engine_model = row(&document["engine"]["llm"]["endpoints"], "name", &json!(OPENAI_ENDPOINT));
    let agent_model = row(&agent["endpoints"], "name", &json!(OPENAI_ENDPOINT));
    let model = socket(&engine_model["address"]);
    assert_eq!(socket(&agent_model["address"]), model, "engine and agent share the model peer");
    for endpoint in [engine_model, agent_model] {
        assert_eq!(endpoint["transport"], "plaintext");
        for member in ["server_name", "trust_der", "trust", "tls"] {
            assert!(endpoint.get(member).is_none(), "stale model TLS member {member}");
        }
    }

    let provider = row(&document["engine"]["llm"]["providers"], "number", &json!(OPENAI_PROVIDER));
    let source_provider = row(&source["engine"]["llm"]["providers"], "number", &json!(OPENAI_PROVIDER));
    let registration = &provider["sources"]["sign_in"];
    let source_registration = &source_provider["sources"]["sign_in"];
    let origin = issuer_origin(source_registration, registration).expect("shipped issuer origin");
    let issuer = socket(&json!(origin.strip_prefix("http://").expect("plaintext issuer")));
    let mut expected = source_registration.clone();
    assert_eq!(rebase_issuer(&mut expected, origin), ISSUER_URLS, "all shipped issuer URLs are checked");
    assert_eq!(registration, &expected, "registration paths, callback, client, resource and scopes are preserved");
    assert_ne!(forge, model);
    assert_ne!(forge, issuer);
    assert_ne!(model, issuer);

    let printed = format!("{}\n{}", String::from_utf8_lossy(&outcome.stdout), String::from_utf8_lossy(&outcome.stderr));
    for path in [effective.join("config"), effective.join("data"), fixture.cache.clone()] {
        assert!(
            printed.contains(path.to_str().expect("fixture path")),
            "effective directory printed: {}",
            path.display()
        );
    }
    for peer in ["fake forge", "fake oauth", "fake llm"] {
        assert!(printed.contains(peer), "manual peer command printed: {peer}");
    }
}

#[test]
fn qa_restart_preserves_effective_edits_data_and_secrets() {
    let fixture = Fixture::new("qa-fake-backends-retained");
    let original = fixture.snapshot();
    let first = fixture.live(|| ());
    successful(&first);
    let effective = fixture.effective();
    let path = effective.join("config/temper.json");
    let mut document = read_json(&path);
    let old = socket(&document["application"]["clients"]["listener"]);
    let [replacement] = support::free_addresses::<1>(&[old]);
    document["application"]["clients"]["listener"] = json!({"socket": replacement.to_string()});
    write_json(&path, &document);
    write(&effective.join("data/qa-retained-state"), b"retained after ordinary stop\n");
    let edited = fs::read(&path).expect("effective edit");
    let agent = fs::read(effective.join("config/agent.json")).expect("effective agent");
    let secret = fs::read(effective.join("data/worker.secret")).expect("effective worker secret");
    let second = fixture.live(|| boxed::status(replacement, "/"));
    successful(&second);
    assert_eq!(second.checked, Some(Some(200)), "restarted child uses the edited listener");
    assert_eq!(fs::read(&path).expect("retained edit"), edited);
    assert_eq!(fs::read(effective.join("config/agent.json")).expect("retained agent"), agent);
    assert_eq!(fs::read(effective.join("data/worker.secret")).expect("retained secret"), secret);
    assert!(effective.join("prepared").is_file(), "preparation remains published");
    assert_eq!(
        fs::read(effective.join("data/qa-retained-state")).expect("retained state"),
        b"retained after ordinary stop\n"
    );
    untouched(&original);
}

#[test]
fn qa_changed_source_document_requires_explicit_reset_without_rewriting_scratch() {
    for name in ["temper.json", "agent.json"] {
        let fixture = Fixture::new(&format!("qa-fake-backends-source-{name}"));
        let first = fixture.start();
        drained(first.exit, &first.left, &first.stderr);
        let effective = fixture.effective();
        let marker = fs::read(effective.join("prepared")).expect("prepared marker");
        let document = fs::read(effective.join("config/temper.json")).expect("effective document");
        let mut changed = fs::read(fixture.config.join(name)).expect("source document");
        changed.push(b'\n'); // Same JSON value, different public-document digest.
        write(&fixture.config.join(name), &changed);
        let original = fixture.snapshot();
        let outcome = fixture.start();
        refused(&outcome, "reset");
        untouched(&original);
        assert_eq!(fs::read(effective.join("prepared")).expect("marker retained"), marker);
        assert_eq!(fs::read(effective.join("config/temper.json")).expect("effective document retained"), document);
    }
}

#[test]
fn qa_same_public_bytes_from_a_different_source_directory_require_reset() {
    let fixture = Fixture::new("qa-fake-backends-source-identity");
    let first = fixture.start();
    drained(first.exit, &first.left, &first.stderr);
    let other = fixture.scratch.path().join("different config");
    directory(&other);
    for name in PUBLIC {
        if let Ok(bytes) = fs::read(fixture.config.join(name)) {
            write(&other.join(name), &bytes);
        }
    }
    let marker = fs::read(fixture.effective().join("prepared")).expect("marker");
    let mut arguments = flags(&other, &fixture.data, &fixture.cache);
    arguments.push("--fake-backends".into());
    let outcome = command(fixture.scratch.path(), "box", arguments);
    refused(&outcome, "reset");
    assert_eq!(fs::read(fixture.effective().join("prepared")).expect("marker retained"), marker);
}

#[test]
fn qa_partial_preparation_is_not_silently_completed_or_destroyed() {
    let fixture = Fixture::new("qa-fake-backends-partial");
    let original = fixture.snapshot();
    let partial = fixture.effective().join("config/temper.json");
    write(&partial, b"qa-interrupted-preparation\n");
    let outcome = fixture.start();
    refused(&outcome, "reset");
    assert!(!fixture.effective().join("prepared").exists(), "never publish an interrupted sandbox");
    assert_eq!(fs::read(partial).expect("partial retained"), b"qa-interrupted-preparation\n");
    untouched(&original);
}

#[test]
fn qa_second_box_refuses_and_first_survives_until_stop() {
    let fixture = Fixture::new("qa-fake-backends-exclusive");
    // Both invocations use the existing box world. The outer service is idle
    // while the inner refusal is driven; no second driver or polling is added.
    let first = fixture.live(|| {
        let path = fixture.effective().join("config/temper.json");
        let before = fs::read(&path).expect("live effective configuration");
        let contender = fixture.start();
        let after = fs::read(path).expect("configuration after contention");
        let document: Value = serde_json::from_slice(&after).expect("effective JSON");
        let address = socket(&document["application"]["clients"]["listener"]);
        (contender, before, after, boxed::status(address, "/"))
    });
    successful(&first);
    let (contender, before, after, still_serving) = first.checked.as_ref().expect("contending box completed");
    refused(contender, ""); // The contract prescribes exclusion, not exact lock-error wording.
    assert_eq!(before, after, "contender cannot rewrite a live sandbox");
    assert_eq!(*still_serving, Some(200), "first box remains alive after contention");
    let after_stop = fixture.start();
    drained(after_stop.exit, &after_stop.left, &after_stop.stderr);
}

fn unsupported(case: &str, member: &str, change: impl FnOnce(&mut Value)) {
    let fixture = Fixture::new(&format!("qa-fake-backends-{case}"));
    let path = fixture.config.join("temper.json");
    let mut document = read_json(&path);
    change(&mut document);
    write_json(&path, &document);
    let original = fixture.snapshot();
    let outcome = fixture.start();
    refused(&outcome, member);
    assert!(!fixture.effective().join("prepared").exists(), "unsupported source is never published");
    untouched(&original);
}

#[test]
fn qa_extra_worker_argument_is_rejected_before_preparation() {
    unsupported("worker-argv", "arguments", |value| {
        value["workers"][0]["agent"]["arguments"].as_array_mut().expect("agent argv").push(json!("extra"));
    });
}

#[test]
fn qa_duplicate_provider_is_rejected_before_preparation() {
    unsupported("duplicate-provider", "provider", |value| {
        let provider = row(&value["engine"]["llm"]["providers"], "number", &json!(OPENAI_PROVIDER)).clone();
        value["engine"]["llm"]["providers"].as_array_mut().expect("providers").push(provider);
    });
}

#[test]
fn qa_missing_github_target_is_rejected_before_preparation() {
    unsupported("missing-github", "forge", |value| {
        value["application"]["forges"].as_array_mut().expect("forges").retain(|forge| forge["number"] != GITHUB_FORGE);
    });
}

#[test]
fn qa_duplicate_openai_endpoint_is_rejected_before_preparation() {
    unsupported("duplicate-openai", "endpoint", |value| {
        let endpoint = row(&value["engine"]["llm"]["endpoints"], "name", &json!(OPENAI_ENDPOINT)).clone();
        value["engine"]["llm"]["endpoints"].as_array_mut().expect("endpoints").push(endpoint);
    });
}

#[test]
fn qa_unprovisioned_bundle_explains_offline_provisioning_without_initializing_source() {
    // This pass's ordinary binary is built with TEMPER_BUNDLE=none.
    for present in [false, true] {
        let scratch = support::Scratch::new(if present {
            "qa-fake-backends-empty-config"
        } else {
            "qa-fake-backends-absent-config"
        });
        let config = scratch.path().join("untouched config");
        let data = scratch.path().join("untouched data");
        let cache = scratch.path().join("cache");
        if present {
            directory(&config);
        }
        let mut arguments = flags(&config, &data, &cache);
        arguments.push("--fake-backends".into());
        let outcome = command(scratch.path(), "box", arguments);
        refused(&outcome, "application.clients.bundle");
        let diagnostic = String::from_utf8_lossy(&outcome.stderr);
        assert!(diagnostic.contains("scripts/bundle.sh --loopback"), "offline provisioning command: {diagnostic}");
        assert!(!data.join("box/fake-backends/prepared").exists());
        assert!(!data.join("box/fake-backends").exists(), "missing bundle refused before sandbox mutation");
        assert!(!config.join("temper.json").exists(), "original C is never initialized");
        assert!(!data.join("worker.secret").exists(), "original D is never initialized");
        assert_eq!(config.exists(), present, "absent source stays absent");
    }
}

#[test]
fn qa_invalid_explicit_bundle_cannot_publish_a_prepared_sandbox() {
    let fixture = Fixture::new("qa-fake-backends-bad-bundle");
    fs::remove_file(fixture.config.join("qa bundle/index.html")).expect("remove fixture page");
    let original = fixture.snapshot();
    let outcome = fixture.start();
    refused(&outcome, "bundle");
    assert!(!fixture.effective().join("prepared").exists());
    untouched(&original);
    assert!(!fixture.config.join("qa bundle/index.html").exists(), "source is never repaired");
}
