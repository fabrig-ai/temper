//! Child reads and retention, on the existing box/ctl real-loop harness.
use crate::support;
use skein_io::kernel::Exit;
use std::{
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};

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
fn flags(config: &Path, data: &Path, cache: &Path, bundle: Option<&Path>) -> Vec<String> {
    let mut flags = vec![
        "--config".into(),
        config.display().to_string(),
        "--data".into(),
        data.display().to_string(),
        "--cache".into(),
        cache.display().to_string(),
        "--fake-backends".into(),
    ];
    if let Some(bundle) = bundle {
        flags.extend(["--bundle".into(), bundle.display().to_string()]);
    }
    flags
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
fn life(root: &Path, paths: &[PathBuf; 3], effective: &Path, client: &str, bundle: Option<&Path>) -> Vec<u8> {
    use support::ctl::{Call, Step};
    let client = root.join(client);
    private(&client);
    let outcome = support::boxed::run_ctl(
        root,
        flags(&paths[0], &paths[1], &paths[2], bundle),
        &client,
        vec![Step::Run(Call::sign_in())],
        crate::BOX_DEADLINE,
        || {
            for name in ["github.json", "oauth.json", "llm.json", "forgejo.json"] {
                let peer = value(&effective.join(name));
                let address: SocketAddr =
                    peer["listen"]["socket"].as_str().expect("retained peer socket").parse().expect("peer address");
                let timeout = std::time::Duration::from_millis(500);
                let mut socket = TcpStream::connect_timeout(&address, timeout).expect("supervised peer listener");
                socket.set_read_timeout(Some(timeout)).expect("peer read bound");
                socket.set_write_timeout(Some(timeout)).expect("peer write bound");
                write!(socket, "GET / HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n").expect("peer request");
                let mut status = [0_u8; 12];
                socket.read_exact(&mut status).expect("bounded peer HTTP status");
                assert!(
                    status.starts_with(b"HTTP/1.1 ") && status[9..].iter().all(u8::is_ascii_digit),
                    "{name}: {status:?}"
                );
            }
            get_root(effective)
        },
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
    // C is truly empty; the box runs ordinary init only in its effective directories.
    // The default demo/owned groups stay dormant throughout this story.
    let source_mode = fs::metadata(&paths[0]).expect("empty source directory").mode();
    let supplied = scratch.path().join("provisioned-loopback");
    bundle(&supplied, SOURCE_PAGE);
    write(&paths[1].join("source-only-fixture"), b"never imported into effective data");
    let sandbox = paths[1].join("box/fake-backends");
    let effective = sandbox.join("config");
    let first = life(scratch.path(), &paths, &effective, "client-first", Some(&supplied));
    assert!(first.windows(b"source provisioned fixture".len()).any(|part| part == b"source provisioned fixture"));
    assert!(!sandbox.join("data/source-only-fixture").exists());
    let generated = value(&effective.join("temper.json"));
    assert_eq!(
        generated["workers"][0]["agent"]["configuration"],
        effective.join("agent.json").to_str().expect("effective agent")
    );
    assert_eq!(
        generated["workers"][0]["agent"]["arguments"],
        serde_json::json!(["internal", "agent", effective.join("agent.json")])
    );
    let adapter = value(&effective.join("agent.json"));
    let sibling = skein_scratch::paths::binary("temper").parent().expect("temper parent").join("fake-claude");
    assert_eq!(adapter["programs"]["claude_code"]["path"], sibling.to_str().expect("adapter path"));
    assert_eq!(
        adapter["programs"]["claude_code"]["directory"],
        sandbox.join("data/programs").to_str().expect("program directory")
    );
    let admin = sandbox.join("data/forgejo-admin.token");
    assert_eq!(fs::metadata(&admin).expect("private token output").mode() & 0o777, 0o600);
    let token_inode = fs::metadata(&admin).expect("token inode").ino();
    let forgejo_document = fs::read(effective.join("forgejo.json")).expect("retained forgejo peer");
    let peer = fs::read(effective.join("oauth.json")).expect("retained peer address");
    let secret = fs::read(sandbox.join("data/worker.secret")).expect("generated sandbox fixture secret");
    write(&sandbox.join("data/retained-fixture"), b"retained data");
    // The restart's CLI selection must replace the retained bundle while keeping other edits.
    // Its body proves that the child reopened the updated effective document.
    let retained_bundle = effective.join("fixture-loopback");
    bundle(&retained_bundle, EFFECTIVE_PAGE);
    let mut edited = generated;
    edited["engine"]["deployment"]["seed"] = 42.into();
    store(&effective.join("temper.json"), &edited);
    fs::remove_dir_all(&supplied).expect("first provisioned bundle no longer available");
    let second = life(scratch.path(), &paths, &effective, "client-second", Some(&retained_bundle));
    assert!(
        second.windows(b"retained effective fixture only".len()).any(|part| part == b"retained effective fixture only")
    );
    assert_eq!(fs::read(effective.join("oauth.json")).expect("retained peer"), peer);
    assert!(fs::read(effective.join("forgejo.json")).expect("retained forgejo peer") == forgejo_document);
    assert_eq!(fs::metadata(&admin).expect("retained token inode").ino(), token_inode);
    assert_eq!(fs::read(sandbox.join("data/worker.secret")).expect("retained sandbox secret"), secret);
    assert_eq!(fs::read(sandbox.join("data/retained-fixture")).expect("retained data"), b"retained data");
    edited["application"]["clients"]["bundle"] = retained_bundle.to_str().expect("bundle path").into();
    assert_eq!(value(&effective.join("temper.json")), edited);
    assert!(fs::read_dir(&paths[0]).expect("source configuration still empty").next().is_none());
    assert_eq!(fs::metadata(&paths[0]).expect("source mode unchanged").mode(), source_mode);
    assert_eq!(
        fs::read(paths[1].join("source-only-fixture")).expect("source fixture unchanged"),
        b"never imported into effective data"
    );
    scratch.remove();
}

mod oauth;
mod qa;
mod walk;

mod closed;
