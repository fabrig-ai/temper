//! Child reads and retention, on the existing box/ctl real-loop harness.
use crate::support;
use skein_io::kernel::Exit;
use skein_lib::Duration;
use std::{
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};

// Same harness allowance as the other ctl stories: ten seconds reserved for teardown.
// The story's execution target remains two seconds; the owner measures it after integration.
const DEADLINE: Duration = Duration::from_secs(12);
const SOURCE_PAGE: &[u8] = b"<!doctype html><html><head></head><body>source provisioned fixture</body></html>";
const EFFECTIVE_PAGE: &[u8] = b"<!doctype html><html><head></head><body>retained effective fixture only</body></html>";

fn private(path: &Path) {
    fs::create_dir_all(path).expect("fixture directory");
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).expect("private fixture directory");
}
fn write(path: &Path, content: &[u8]) {
    fs::write(path, content).expect("fixture bytes");
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).expect("private fixture file");
}
fn bundle(path: &Path, page: &[u8]) {
    private(path);
    private(&path.join("0000000000000001"));
    write(&path.join("index.html"), page);
    write(&path.join("0000000000000001/main.js"), b"// process fixture\n");
}
fn value(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(path).expect("public fixture document")).expect("fixture JSON")
}
fn store(path: &Path, value: &serde_json::Value) {
    write(path, &serde_json::to_vec(value).expect("fixture JSON bytes"));
}
fn flags(config: &Path, data: &Path, cache: &Path) -> Vec<String> {
    vec![
        "--config".into(),
        config.display().to_string(),
        "--data".into(),
        data.display().to_string(),
        "--cache".into(),
        cache.display().to_string(),
        "--fake-backends".into(),
    ]
}
fn get_root(config: &Path) -> Vec<u8> {
    let document = value(&config.join("temper.json"));
    let address: SocketAddr = document["application"]["clients"]["listener"]["socket"]
        .as_str()
        .expect("effective clients socket")
        .parse()
        .expect("loopback clients");
    let timeout = std::time::Duration::from_millis(500);
    let mut socket = TcpStream::connect_timeout(&address, timeout).expect("running serve child");
    socket.set_read_timeout(Some(timeout)).expect("HTTP read bound");
    socket.set_write_timeout(Some(timeout)).expect("HTTP write bound");
    write!(socket, "GET / HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n").expect("one HTTP request");
    let mut bytes = Vec::new();
    socket.take(65_536).read_to_end(&mut bytes).expect("bounded HTTP reply");
    assert!(bytes.starts_with(b"HTTP/1.1 200"), "{}", String::from_utf8_lossy(&bytes));
    bytes
}
fn life(root: &Path, paths: &[PathBuf; 3], effective: &Path, client: &str) -> Vec<u8> {
    use support::ctl::{Call, Step};
    let client = root.join(client);
    private(&client);
    let outcome = support::boxed::run_ctl(
        root,
        flags(&paths[0], &paths[1], &paths[2]),
        &client,
        vec![Step::Run(Call::sign_in())],
        DEADLINE,
        || get_root(effective),
    );
    assert_eq!(outcome.exit, Some(Exit::Code(0)), "{}", String::from_utf8_lossy(&outcome.stderr));
    assert_eq!(outcome.left, Some(Vec::new()), "no child outlives its box");
    assert_eq!(outcome.calls.len(), 1, "one ordinary link sign-in");
    assert_eq!(outcome.calls[0].exit, Some(Exit::Code(0)), "{}", String::from_utf8_lossy(&outcome.calls[0].stderr));
    outcome.checked.expect("HTTP request ran before box stopped")
}
#[test]
fn the_serve_child_reads_retained_effective_documents_across_restart() {
    let scratch = support::Scratch::new("fake-backends-effective-child");
    let paths = ["source-config", "source-data", "cache"].map(|name| scratch.path().join(name));
    for path in &paths {
        private(path);
    }
    // No PATH discovery or owned click: the default demo/owned groups stay dormant.
    let init = std::process::Command::new(env!("CARGO_BIN_EXE_temper"))
        .arg("init")
        .args(flags(&paths[0], &paths[1], &paths[2]).into_iter().filter(|word| word != "--fake-backends"))
        .env_clear()
        .current_dir(scratch.path())
        .output()
        .expect("public init fixture");
    assert!(init.status.success(), "{}", String::from_utf8_lossy(&init.stderr));
    let supplied = scratch.path().join("provisioned-loopback");
    bundle(&supplied, SOURCE_PAGE);
    let mut document = value(&paths[0].join("temper.json"));
    document["application"]["clients"]["bundle"] = supplied.to_str().expect("bundle path").into();
    store(&paths[0].join("temper.json"), &document);
    write(&paths[1].join("source-only-fixture"), b"never imported into effective data");
    let originals = ["temper.json", "agent.json", "charters/chat.txt", "charters/coding.txt"].map(|name| {
        let path = paths[0].join(name);
        (name, fs::read(&path).expect("source public bytes"), fs::metadata(path).expect("source mode").mode())
    });
    let sandbox = paths[1].join("box/fake-backends");
    let effective = sandbox.join("config");
    let first = life(scratch.path(), &paths, &effective, "client-first");
    assert!(first.windows(b"source provisioned fixture".len()).any(|part| part == b"source provisioned fixture"));
    assert!(!sandbox.join("data/source-only-fixture").exists());
    let generated = value(&effective.join("temper.json"));
    assert_eq!(
        generated["workers"][0]["agent"]["configuration"],
        effective.join("agent.json").to_str().expect("effective agent")
    );
    assert_eq!(
        generated["workers"][0]["agent"]["arguments"],
        serde_json::json!(["agent", effective.join("agent.json")])
    );
    let peer = fs::read(effective.join("oauth.json")).expect("retained peer address");
    let secret = fs::read(sandbox.join("data/worker.secret")).expect("generated sandbox fixture secret");
    write(&sandbox.join("data/retained-fixture"), b"retained data");
    // Only the effective document names this second bundle. Its body proves that the child
    // reopened C' on restart, while the original configuration keeps its first bundle.
    let retained_bundle = effective.join("fixture-loopback");
    bundle(&retained_bundle, EFFECTIVE_PAGE);
    let mut edited = generated;
    edited["application"]["clients"]["bundle"] = retained_bundle.to_str().expect("bundle path").into();
    store(&effective.join("temper.json"), &edited);
    let second = life(scratch.path(), &paths, &effective, "client-second");
    assert!(
        second.windows(b"retained effective fixture only".len()).any(|part| part == b"retained effective fixture only")
    );
    assert_eq!(fs::read(effective.join("oauth.json")).expect("retained peer"), peer);
    assert_eq!(fs::read(sandbox.join("data/worker.secret")).expect("retained sandbox secret"), secret);
    assert_eq!(fs::read(sandbox.join("data/retained-fixture")).expect("retained data"), b"retained data");
    for (name, bytes, mode) in originals {
        let path = paths[0].join(name);
        assert_eq!(fs::read(&path).expect("source unchanged"), bytes, "{name}");
        assert_eq!(fs::metadata(path).expect("source mode unchanged").mode(), mode, "{name}");
    }
    assert_eq!(
        fs::read(paths[1].join("source-only-fixture")).expect("source fixture unchanged"),
        b"never imported into effective data"
    );
    scratch.remove();
}

mod qa;
