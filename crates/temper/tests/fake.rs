//! The QA runner starts the shipped binary, then reads files and serves requests.
#[path = "support/listeners.rs"]
pub mod listeners;
pub mod support;
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Child, Command, Stdio},
    sync::Arc,
    time::Duration,
};

fn command(kind: &str, document: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_temper"));
    command
        .env_clear()
        .args(["fake", kind, "--document"])
        .arg(document)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}
fn llm_document(directory: &Path, tls: bool) -> Value {
    json!({"listen":{"socket":"127.0.0.1:0"},"key":directory.join("key"),"tls":if tls {json!({"root":directory.join("root.der")})}else{Value::Null},
        "models":[{"name":"qa","window":4096,"output":1024}],"cued":[],
        "asked":[{"cue":"say hello","turns":["hello from QA"]}],"otherwise":["fallback"]})
}
fn forge_document(directory: &Path) -> Value {
    json!({"listen":{"socket":"127.0.0.1:0"},"admin_token":directory.join("forge/admin_token"),"owner":"fake",
        "repositories":[{"name":"widget","landing":"main","ci":null}]})
}
fn document(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec(value).expect("valid fixture")).expect("valid fixture");
}
struct Running {
    child: Option<Child>,
    address: SocketAddr,
    control: Option<SocketAddr>,
}
impl Running {
    fn start(kind: &str, document: &Path, observations: Option<&Path>) -> Self {
        Self::command(command(kind, document), observations)
    }
    fn command(mut command: Command, observations: Option<&Path>) -> Self {
        command.args(["--ready", "ready ", "--seed", "23"]);
        if let Some(path) = observations {
            command.arg("--observations").arg(path);
        }
        let child = command.spawn().expect("valid fixture");
        let mut run =
            Self { child: Some(child), address: "127.0.0.1:0".parse().expect("valid fixture"), control: None };
        let mut bytes = Vec::new();
        for _ in 0..128 {
            let mut byte = [0_u8; 1];
            let read =
                run.child.as_mut().expect("running fake").stdout.as_mut().expect("stdout pipe").read_exact(&mut byte);
            if let Err(why) = read {
                let mut child = run.child.take().expect("running fake");
                if why.kind() != std::io::ErrorKind::UnexpectedEof {
                    child.kill().expect("stop fake after readiness failure");
                }
                let ended = child.wait_with_output().expect("reap fake after readiness failure");
                panic!(
                    "ready line: {why:?}; status {}; stderr {}; partial {:?}; command {command:?}",
                    ended.status,
                    String::from_utf8_lossy(&ended.stderr),
                    String::from_utf8_lossy(&bytes)
                );
            }
            bytes.push(byte[0]);
            if byte[0] == b'\n' {
                break;
            }
        }
        let line = String::from_utf8(bytes).expect("ready line is UTF-8");
        assert!(line.starts_with("ready "), "ready line: {line}");
        let mut addresses = line.strip_prefix("ready ").expect("valid fixture").split_whitespace();
        run.address = addresses.next().expect("provider address").parse().expect("valid fixture");
        run.control = addresses.next().map(|address| address.parse::<SocketAddr>().expect("control address"));
        assert!(addresses.next().is_none(), "ready line has only its listeners");
        if let Some(control) = run.control {
            assert!(control.ip().is_loopback());
            assert_ne!(control.port(), 0);
        }
        assert!(run.address.ip().is_loopback());
        assert_ne!(run.address.port(), 0);
        run
    }
    fn stop(mut self) {
        let child = self.child.take().expect("valid fixture");
        assert!(
            Command::new("/bin/kill")
                .args(["-TERM", &child.id().to_string()])
                .status()
                .expect("valid fixture")
                .success()
        );
        let ended = child.wait_with_output().expect("valid fixture");
        assert_eq!(ended.status.code(), Some(0), "{}", String::from_utf8_lossy(&ended.stderr));
        assert!(ended.stdout.is_empty(), "stdout holds only the ready line");
        assert!(ended.stderr.is_empty(), "{}", String::from_utf8_lossy(&ended.stderr));
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _killed = child.kill();
            let _ended = child.wait();
        }
    }
}
#[test]
#[should_panic(expected = "status exit status: 7; stderr fixture refused")]
fn readiness_eof_reports_child_status_and_stderr() {
    let mut command = Command::new("/bin/sh");
    command
        .env_clear()
        .args(["-c", "printf 'fixture refused' >&2; exit 7"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let _run = Running::command(command, None);
}
fn stream(address: SocketAddr) -> TcpStream {
    let stream = TcpStream::connect_timeout(&address, Duration::from_millis(300)).expect("valid fixture");
    stream.set_read_timeout(Some(Duration::from_millis(300))).expect("valid fixture");
    stream.set_write_timeout(Some(Duration::from_millis(300))).expect("valid fixture");
    stream
}
fn http(method: &str, target: &str, authorization: &str, body: &[u8]) -> Vec<u8> {
    let mut request=format!("{method} {target} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",body.len()).into_bytes();
    if !authorization.is_empty() {
        request.extend_from_slice(format!("Authorization: {authorization}\r\n").as_bytes());
    }
    request.extend_from_slice(b"\r\n");
    request.extend_from_slice(body);
    request
}
fn request(address: SocketAddr, method: &str, target: &str, authorization: &str, body: &[u8]) -> Vec<u8> {
    let mut stream = stream(address);
    stream.write_all(&http(method, target, authorization, body)).expect("valid fixture");
    let mut response = Vec::new();
    stream.read_to_end(&mut response).expect("valid fixture");
    response
}
fn body(response: &[u8]) -> Value {
    let boundary = response.windows(4).position(|part| part == b"\r\n\r\n").expect("valid fixture") + 4;
    serde_json::from_slice(&response[boundary..]).expect("valid fixture")
}
fn private_hex(path: &Path, expected: &[u8]) {
    let bytes = fs::read(path).expect("valid fixture");
    assert_eq!(bytes.len(), 40);
    assert!(bytes.iter().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b)));
    assert_eq!(bytes, expected);
    assert_eq!(fs::metadata(path).expect("valid fixture").permissions().mode() & 0o777, 0o600);
}
#[test]
#[expect(clippy::too_many_lines, reason = "one table covers refused startup and rollback for every output")]
fn refused_start() {
    struct Row {
        name: &'static str,
        kind: &'static str,
        document: Option<Value>,
        files: &'static [&'static str],
        directories: &'static [(&'static str, u32)],
        observations: Option<&'static str>,
        path: Option<&'static str>,
        reason: &'static str,
    }
    let scratch = support::Scratch::new("fake_refused");
    let llm = |name| Some(llm_document(&scratch.path().join(name), true));
    let forge = |name| Some(forge_document(&scratch.path().join(name)));
    let rows = [
        Row {
            name: "key",
            kind: "llm",
            document: llm("key"),
            files: &["key"],
            directories: &[],
            observations: None,
            path: Some("key"),
            reason: "retained output differs from expected seeded bytes",
        },
        Row {
            name: "added_key",
            kind: "llm",
            document: {
                let mut doc = llm_document(&scratch.path().join("added_key"), true);
                doc["added_keys"] = json!([scratch.path().join("added_key/second.key")]);
                Some(doc)
            },
            files: &["second.key"],
            directories: &[],
            observations: None,
            path: Some("second.key"),
            reason: "retained output differs from expected seeded bytes",
        },
        Row {
            name: "admin_token",
            kind: "forge",
            document: forge("admin_token"),
            files: &["forge/admin_token"],
            directories: &[("forge", 0o700)],
            observations: None,
            path: Some("forge/admin_token"),
            reason: "retained output differs from expected seeded bytes",
        },
        Row {
            name: "root",
            kind: "llm",
            document: llm("root"),
            files: &["root.der"],
            directories: &[],
            observations: None,
            path: Some("root.der"),
            reason: "retained output differs from expected seeded bytes",
        },
        Row {
            name: "observations",
            kind: "forge",
            document: forge("observations"),
            files: &["observations"],
            directories: &[],
            observations: Some("observations"),
            path: Some("observations"),
            reason: "File exists (os error 17)",
        },
        Row {
            name: "directory",
            kind: "forge",
            document: forge("directory"),
            files: &[],
            directories: &[("forge", 0o755)],
            observations: None,
            path: Some("forge"),
            reason: "must be a private directory of this user's, not reached through a link",
        },
        Row {
            name: "missing",
            kind: "llm",
            document: None,
            files: &[],
            directories: &[],
            observations: None,
            path: Some("document.json"),
            reason: "absent",
        },
        Row {
            name: "refused",
            kind: "llm",
            document: Some(json!({})),
            files: &[],
            directories: &[],
            observations: None,
            path: None,
            reason: "listen: missing",
        },
    ];
    for row in rows {
        let dir = scratch.path().join(row.name);
        fs::create_dir(&dir).expect("case directory");
        let doc = dir.join("document.json");
        if let Some(value) = row.document {
            document(&doc, &value);
        }
        for (name, mode) in row.directories {
            fs::create_dir(dir.join(name)).expect("existing directory");
            fs::set_permissions(dir.join(name), fs::Permissions::from_mode(*mode)).expect("directory mode");
        }
        for name in row.files {
            fs::write(dir.join(name), b"existing").expect("existing output");
        }
        let mut command = command(row.kind, &doc);
        if let Some(path) = row.observations {
            command.arg("--observations").arg(dir.join(path));
        }
        let before = snapshot(&dir);
        let output = command.output().expect("refused fake");
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let location = row.path.map_or_else(String::new, |path| format!("{}: ", dir.join(path).display()));
        let expected = format!("temper fake {}: {location}{}\n", row.kind, row.reason);
        assert_eq!(String::from_utf8(output.stderr).expect("UTF-8 refusal"), expected, "{}", row.name);
        assert_eq!(snapshot(&dir), before, "{}: refused startup restores the files", row.name);
    }
    refused_deployment_start(scratch.path());
    scratch.remove();
}

fn deployment_command(kind: &str, directory: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_temper"));
    command
        .env_clear()
        .args(["fake", kind])
        .args(support::configuration::flags(directory))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}
fn forge_setup(directory: &Path, listen: &str) -> Value {
    let mut config = support::configuration::check_document(directory);
    config["application"]["forges"][0]["api"] = json!({"socket":listen});
    config["application"]["forges"][0]["git"] = json!(format!("http://{listen}"));
    let fake = &mut config["application"]["setup"][0]["mode"]["fake"];
    fake["listen"] = json!({"socket":listen});
    fake["owner"] = json!("fake");
    fake["repositories"][0]["landing"] = json!("trunk");
    config
}
#[expect(clippy::too_many_lines, reason = "deployment forms refuse before creating files")]
fn refused_deployment_start(root: &Path) {
    const TOKEN: &[u8] = b"0123456789abcdef0123456789abcdef01234567";
    let directory = root.join("deployment");
    fs::create_dir(&directory).expect("deployment cases");
    for (name, number, mode, token, own_document, observations, reason, names_file) in [
        ("absent", "1", "fake", None, false, false, "absent", true),
        ("short", "1", "fake", Some(&TOKEN[..39]), false, false, "expected 40 lowercase hexadecimal digits", true),
        (
            "uppercase",
            "1",
            "fake",
            Some(b"A123456789abcdef0123456789abcdef01234567".as_slice()),
            false,
            false,
            "expected 40 lowercase hexadecimal digits",
            true,
        ),
        (
            "long",
            "1",
            "fake",
            Some(b"0123456789abcdef0123456789abcdef01234567890".as_slice()),
            false,
            false,
            "past 40 bytes",
            true,
        ),
        ("no_entry", "2", "fake", None, false, false, "the setup has no entry for it", false),
        ("owned", "1", "owned", None, false, false, "it is owned, not fake", false),
        ("managed", "1", "managed", None, false, false, "it is managed, not fake", false),
        (
            "both",
            "1",
            "fake",
            None,
            true,
            false,
            "usage: temper fake forge [DIRECTORIES] N [--ready PREFIX] [--seed N] [--observations FILE] | temper fake forge --document FILE [--ready PREFIX] [--seed N] [--observations FILE]",
            false,
        ),
        ("observations", "1", "fake", Some(TOKEN), false, true, "File exists (os error 17)", true),
    ] {
        let dir = directory.join(name);
        fs::create_dir(&dir).expect("case directory");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).expect("private");
        let mut config = forge_setup(&dir, "127.0.0.1:9");
        match mode {
            "owned" => support::owned::document(
                &mut config,
                &support::owned::Names {
                    forgejo: Path::new("/usr/bin/false"),
                    runner: None,
                    port: 9,
                    bot: "temper",
                    person: "ada",
                },
            ),
            "managed" => {
                config = support::configuration::old_form_check_document(&dir);
                config["application"]["setup"][0]["mode"] = json!({"managed":{"admin_token":"admin_token"}});
                config["application"]["setup"][0]["orgs"] = json!(["acme"]);
            }
            "fake" => {}
            _ => unreachable!("fixture mode"),
        }
        document(&dir.join("temper.json"), &config);
        if let Some(token) = token {
            fs::create_dir(dir.join("forge-1")).expect("token directory");
            fs::set_permissions(dir.join("forge-1"), fs::Permissions::from_mode(0o700)).expect("private");
            support::write_private(&dir.join("forge-1/admin_token"), token);
        }
        let mut command = deployment_command("forge", &dir);
        command.arg(number);
        if own_document {
            command.args(["--document", "unused.json"]);
        }
        if observations {
            fs::write(dir.join("observations"), b"existing").expect("existing observations");
            command.arg("--observations").arg(dir.join("observations"));
        }
        let before = snapshot(&dir);
        let output = command.output().expect("refused fake");
        assert_eq!(output.status.code(), Some(1), "{name}");
        assert!(output.stdout.is_empty());
        let file = if observations { "observations" } else { "forge-1/admin_token" };
        let location = if names_file { format!("{}: ", dir.join(file).display()) } else { String::new() };
        assert_eq!(
            String::from_utf8(output.stderr).expect("refusal"),
            format!("temper fake forge: {location}{reason}\n"),
            "{name}"
        );
        assert_eq!(snapshot(&dir), before, "{name}: refused startup leaves directories unchanged");
    }
    let bound = usize::try_from(temper_fake::demo::KEY_FILE_BYTES).expect("key file bound");
    for (name, kind, demo, file, mode, extra, reason, names_file) in [
        ("llm_null", "llm", false, None, 0o600, false, "application.demo is null: the deployment has no demo".to_owned(), false),
        ("llm_absent", "llm", true, None, 0o600, false, "absent".to_owned(), true),
        ("llm_public", "llm", true, Some(TOKEN.to_vec()), 0o644, false, "readable by others or reached through a link".to_owned(), true),
        ("llm_empty", "llm", true, Some(b"\n".to_vec()), 0o600, false, "holds no key".to_owned(), true),
        ("llm_long", "llm", true, Some(vec![b'k'; bound + 1]), 0o600, false, format!("past {bound} bytes"), true),
        ("llm_word", "llm", true, None, 0o600, true, "usage: temper fake llm [DIRECTORIES] [--ready PREFIX] [--seed N] [--observations FILE] | temper fake llm --document FILE [--ready PREFIX] [--seed N] [--observations FILE]".to_owned(), false),
    ] {
        let dir = directory.join(name);
        fs::create_dir(&dir).expect("case directory");
        let mut config = support::configuration::demo_document(&dir, "127.0.0.1:8370");
        if !demo {
            config["application"]["demo"] = Value::Null;
        }
        document(&dir.join("temper.json"), &config);
        let path = dir.join("demo/llm.key");
        if let Some(file) = file {
            fs::create_dir(dir.join("demo")).expect("key directory");
            support::write_private(&path, &file);
            fs::set_permissions(&path, fs::Permissions::from_mode(mode)).expect("key mode");
        }
        let mut command = deployment_command(kind, &dir);
        if extra {
            command.arg("extra");
        }
        let before = snapshot(&dir);
        let output = command.output().expect("refused demo");
        assert_eq!(output.status.code(), Some(1), "{name}");
        assert!(output.stdout.is_empty(), "{name}");
        let location = if names_file { format!("{}: ", path.display()) } else { String::new() };
        assert_eq!(output.stderr, format!("temper fake {kind}: {location}{reason}\n").as_bytes(), "{name}");
        assert_eq!(snapshot(&dir), before, "{name}: refused startup leaves directories unchanged");
    }
}

fn snapshot(root: &Path) -> std::collections::BTreeMap<std::path::PathBuf, (u32, Option<Vec<u8>>)> {
    let mut found = std::collections::BTreeMap::new();
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory).expect("valid fixture") {
            let entry = entry.expect("valid fixture");
            let meta = entry.metadata().expect("valid fixture");
            let path = entry.path();
            let bytes = if meta.is_dir() {
                directories.push(path.clone());
                None
            } else {
                Some(fs::read(&path).expect("valid fixture"))
            };
            found.insert(path, (meta.permissions().mode() & 0o777, bytes));
        }
    }
    found
}
#[test]
#[expect(clippy::too_many_lines, reason = "both forge forms serve the same requests and retain their files")]
fn forge_run() {
    for from_setup in [false, true] {
        let scratch = support::Scratch::new("fake_forge_run");
        let dir = scratch.path();
        let doc = dir.join("forge.json");
        let mut config = forge_document(dir);
        config["repositories"][0]["landing"] = json!("trunk");
        document(&doc, &config);
        let observations = dir.join("observations");
        let (run, token_path, token) = if from_setup {
            let (listen, path, token) = prepared_forge(dir);
            let mut command = deployment_command("forge", dir);
            command.arg("1");
            let run = Running::command(command, Some(&observations));
            assert_eq!(run.address, listen, "ready on fake.listen");
            (run, path, token)
        } else {
            (
                Running::start("forge", &doc, Some(&observations)),
                dir.join("forge/admin_token"),
                temper_fake::credential::credential(23, temper_fake::credential::ADMIN_TOKEN).to_vec(),
            )
        };
        private_hex(&token_path, &token);
        assert_eq!(
            fs::metadata(token_path.parent().expect("token directory")).expect("valid fixture").permissions().mode()
                & 0o777,
            0o700
        );
        assert_eq!(fs::metadata(&observations).expect("valid fixture").permissions().mode() & 0o777, 0o600);
        assert_eq!(
            fs::read(&observations).expect("valid fixture"),
            b"{\"observations_lost\":0}\n",
            "no seed observations"
        );
        let auth = format!("token {}", std::str::from_utf8(&token).expect("valid fixture"));
        let user = request(run.address, "GET", "/api/v1/user", &auth, b"");
        assert!(user.starts_with(b"HTTP/1.1 200"));
        assert_eq!(body(&user)["login"], "temper-admin");
        let repository = request(run.address, "GET", "/api/v1/repositories/1", &auth, b"");
        assert!(repository.starts_with(b"HTTP/1.1 200"));
        let repository = body(&repository);
        for (member, expected) in
            [("default_branch", "trunk".to_owned()), ("html_url", format!("http://{}/fake/widget", run.address))]
        {
            assert_eq!(repository[member], expected, "{member}");
        }
        let objects = request(run.address, "POST", "/.fake/git/objects", "", br#"{"commits":[1]}"#);
        assert!(objects.starts_with(b"HTTP/1.1 200"));
        let objects = body(&objects);
        assert_eq!(objects["objects"][0]["commit"], 1);
        assert!(objects["objects"][0]["parent"].is_null());
        let tree = objects["objects"][0]["tree"].as_array().expect("valid fixture");
        assert_eq!(tree.len(), temper_fake::forge::FILES.len());
        let path = b".forgejo/workflows/check.yml";
        let len = skein_lib::base64::encoded_len(
            skein_lib::base64::Alphabet::Standard,
            u32::try_from(path.len()).expect("valid fixture"),
        )
        .expect("valid fixture");
        let mut expected_path = skein_lib::Writer::new(usize::try_from(len).expect("valid fixture"));
        skein_lib::base64::encode(skein_lib::base64::Alphabet::Standard, path, &mut expected_path)
            .expect("valid fixture");
        let expected_path = expected_path.finish();
        assert!(
            tree.iter().any(|file| file["path"].as_str().expect("valid fixture").as_bytes() == expected_path.as_ref())
        );
        // Setup's admin flag and the domain's repository permission are separate.
        // Grant through the API, as a runner would, rather than seeding a grant.
        let grant = request(
            run.address,
            "PUT",
            "/api/v1/repos/fake/widget/collaborators/temper-admin",
            &auth,
            br#"{"permission":"admin"}"#,
        );
        assert!(grant.starts_with(b"HTTP/1.1 204"), "{}", String::from_utf8_lossy(&grant));
        let branch = request(
            run.address,
            "POST",
            "/api/v1/repos/fake/widget/branches",
            &auth,
            br#"{"new_branch_name":"work","old_ref_name":"910a2dec89025cc1000000000000000100000000"}"#,
        );
        assert!(branch.starts_with(b"HTTP/1.1 201"), "{}", String::from_utf8_lossy(&branch));
        let lines = fs::read_to_string(&observations).expect("valid fixture");
        let records =
            lines.lines().map(|line| serde_json::from_str::<Value>(line).expect("valid fixture")).collect::<Vec<_>>();
        assert!(records.iter().any(|line| line["observation"] == "moved" && line["branch"] == "work"));
        assert!(
            records.last().expect("valid fixture").get("observations_lost").is_some(),
            "checkpoint precedes the reply"
        );
        run.stop();
        let lines = fs::read_to_string(&observations).expect("valid fixture");
        assert_eq!(
            serde_json::from_str::<Value>(lines.lines().last().expect("valid fixture")).expect("valid fixture"),
            json!({"observations_lost":0})
        );
        assert_eq!(fs::read(&token_path).expect("token kept at stop"), token);
        assert!(observations.exists());
        assert!(doc.exists());
        scratch.remove();
    }
}

fn prepared_forge(directory: &Path) -> (SocketAddr, std::path::PathBuf, Vec<u8>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("available port");
    let listen = listener.local_addr().expect("listen");
    drop(listener);
    document(&directory.join("temper.json"), &forge_setup(directory, &listen.to_string()));
    let prepared = support::owned::prepare(directory, 1);
    assert_eq!(prepared.status.code(), Some(0), "{}", String::from_utf8_lossy(&prepared.stderr));
    let path = directory.join("forge-1/admin_token");
    let token = fs::read(&path).expect("prepared token");
    (listen, path, token)
}
#[test]
fn the_old_form_prepared_fake_reconciles_twice_before_it_stops() {
    let scratch = support::Scratch::new("fake_forge_reconcile");
    let (_, path, token) = prepared_forge(scratch.path());
    // temper-15 retains the explicit file-writing reconcile command.
    let mut config = support::configuration::old_form_check_document(scratch.path());
    let prepared: Value =
        serde_json::from_slice(&fs::read(scratch.path().join("temper.json")).expect("prepared document"))
            .expect("prepared JSON");
    config["application"]["forges"][0]["api"] = prepared["application"]["forges"][0]["api"].clone();
    config["application"]["forges"][0]["git"] = prepared["application"]["forges"][0]["git"].clone();
    config["application"]["setup"] = prepared["application"]["setup"].clone();
    document(&scratch.path().join("temper.json"), &config);
    let mut command = deployment_command("forge", scratch.path());
    command.arg("1");
    let run = Running::command(command, None);
    for pass in ["first", "second"] {
        let output = Command::new(env!("CARGO_BIN_EXE_temper"))
            .env_clear()
            .args(["forge", "reconcile"])
            .args(support::configuration::flags(scratch.path()))
            .arg("1")
            .output()
            .expect("reconcile ends");
        assert_eq!(output.status.code(), Some(0), "{pass}: {}", String::from_utf8_lossy(&output.stderr));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
    run.stop();
    assert_eq!(fs::read(path).expect("handover token kept"), token);
    scratch.remove();
}

const CHAT:&[u8]=br#"{"model":"qa","store":false,"instructions":"","input":[{"role":"user","content":[{"type":"input_text","text":"say hello"}]}],"stream":true}"#;
#[test]
#[expect(clippy::too_many_lines, reason = "all LLM forms serve the same requests and retain their files")]
fn llm_run() {
    for (name, tls, demo) in
        [("fake_llm_plain", false, false), ("fake_llm_tls", true, false), ("fake_llm_demo", false, true)]
    {
        let scratch = support::Scratch::new(name);
        let dir = scratch.path();
        let doc = dir.join("llm.json");
        document(&doc, &llm_document(dir, tls));
        let (run, key_path, key) = if demo {
            let listen = support::free_addresses::<1>(&[])[0];
            let config = support::configuration::demo_document(dir, &listen.to_string());
            document(&dir.join("temper.json"), &config);
            fs::create_dir(dir.join("demo")).expect("demo directory");
            let path = dir.join("demo/llm.key");
            let key = b"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_vec();
            support::write_private(&path, &[key.as_slice(), b"\n"].concat());
            let run = Running::command(deployment_command("llm", dir), None);
            assert_eq!(run.address, listen, "ready on application.demo.llm.listen");
            (run, path, key)
        } else {
            let run = Running::start("llm", &doc, None);
            let key = temper_fake::credential::credential(23, temper_fake::credential::KEY);
            private_hex(&dir.join("key"), &key);
            (run, dir.join("key"), key.to_vec())
        };
        let mut chat: Value = serde_json::from_slice(CHAT).expect("valid fixture");
        if demo {
            chat["instructions"] = json!("say hello");
            chat["input"][0]["content"][0]["text"] = json!("Begin the work your brief describes.");
        }
        let chat = serde_json::to_vec(&chat).expect("valid fixture");
        let before = snapshot(dir);
        let auth = format!("Bearer {}", std::str::from_utf8(&key).expect("valid fixture"));
        for (credential, status, text) in [
            (&*auth, b"HTTP/1.1 200".as_slice(), "hello from QA"),
            ("Bearer wrong", b"HTTP/1.1 401", "invalid_api_key"),
        ] {
            let response = if tls {
                assert_eq!(fs::read(dir.join("root.der")).expect("valid fixture"), skein_fake_peers::transport::ROOT);
                assert_eq!(
                    fs::metadata(dir.join("root.der")).expect("valid fixture").permissions().mode() & 0o777,
                    0o600
                );
                let mut roots = rustls::RootCertStore::empty();
                roots
                    .add(rustls::pki_types::CertificateDer::from(
                        fs::read(dir.join("root.der")).expect("valid fixture"),
                    ))
                    .expect("valid fixture");
                let config =
                    rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                        .with_safe_default_protocol_versions()
                        .expect("valid fixture")
                        .with_root_certificates(roots)
                        .with_no_client_auth();
                let connection = rustls::ClientConnection::new(
                    Arc::new(config),
                    rustls::pki_types::ServerName::try_from("skein.test").expect("valid fixture"),
                )
                .expect("valid fixture");
                let mut stream = rustls::StreamOwned::new(connection, stream(run.address));
                stream.write_all(&http("POST", "/v1/responses", credential, &chat)).expect("valid fixture");
                let mut response = Vec::new();
                match stream.read_to_end(&mut response) {
                    Ok(_) => {}
                    Err(why) if why.kind() == std::io::ErrorKind::UnexpectedEof => {}
                    Err(why) => panic!("TLS response: {why}"),
                }
                response
            } else {
                request(run.address, "POST", "/v1/responses", credential, &chat)
            };
            assert!(response.starts_with(status), "{}", String::from_utf8_lossy(&response));
            assert!(String::from_utf8_lossy(&response).contains(text), "{}", String::from_utf8_lossy(&response));
        }
        if !tls {
            let mut other: Value = serde_json::from_slice(&chat).expect("valid fixture");
            other["model"] = json!("another-model");
            let response = request(
                run.address,
                "POST",
                "/v1/responses",
                &auth,
                &serde_json::to_vec(&other).expect("valid fixture"),
            );
            assert!(response.starts_with(b"HTTP/1.1 200"));
            assert!(String::from_utf8_lossy(&response).contains("hello from QA"));
        }
        let mut other: Value = serde_json::from_slice(&chat).expect("valid fixture");
        other["instructions"] = json!("");
        other["input"][0]["content"][0]["text"] = json!("another request");
        if !tls {
            let response = request(
                run.address,
                "POST",
                "/v1/responses",
                &auth,
                &serde_json::to_vec(&other).expect("valid fixture"),
            );
            assert!(response.starts_with(b"HTTP/1.1 200"));
            assert!(String::from_utf8_lossy(&response).contains("fallback"));
        }
        run.stop();
        assert_eq!(snapshot(dir), before, "{name}: files and modes retained after stop");
        assert_eq!(
            fs::read(&key_path).expect("key retained"),
            if demo { [key.as_slice(), b"\n"].concat() } else { key }
        );
        scratch.remove();
    }
}
#[test]
fn ready_write_failure_stops_and_keeps_outputs() {
    let scratch = support::Scratch::new("fake_ready_failure");
    let doc = scratch.path().join("llm.json");
    document(&doc, &llm_document(scratch.path(), false));
    let output = command("llm", &doc)
        .args(["--ready", ""])
        .stdout(fs::OpenOptions::new().write(true).open("/dev/full").expect("valid fixture"))
        .output()
        .expect("valid fixture");
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stderr, b"temper fake llm: stdout: No space left on device (os error 28)\n");
    assert!(scratch.path().join("key").exists());
    scratch.remove();
}

use std::os::unix::ffi::OsStrExt;
use temper_worker_domain_checkout::git as checkout_git;
use temper_worker_protocol_git::invocation::{self, Command as GitCommand};

struct GitStep {
    command: GitCommand,
    kind: Option<checkout_git::Kind>,
    at: &'static str,
    input: &'static [u8],
    edit: Option<(&'static str, &'static [u8])>,
    extra: Option<&'static [u8]>,
    exit: i32,
    stdout: &'static str,
    stderr: &'static str,
}
fn git_step(command: GitCommand, kind: Option<checkout_git::Kind>, at: &'static str) -> GitStep {
    GitStep { command, kind, at, input: b"", edit: None, extra: None, exit: 0, stdout: "", stderr: "" }
}

enum GitEndpoint {
    Forge,
    NoRoute,
    Llm,
    Closed,
}

/// Answers the one request the row's git makes with an empty, unnamed 404,
/// on the test's own thread, between the spawn and the exit it waits for.
fn no_git_route(listener: &std::net::TcpListener) {
    use std::io::{BufRead, BufReader};
    let (stream, _) = listener.accept().expect("no-route request");
    stream.set_read_timeout(Some(Duration::from_millis(300))).expect("valid fixture");
    stream.set_write_timeout(Some(Duration::from_millis(300))).expect("valid fixture");
    let mut reader = BufReader::new(stream);
    let mut length = None;
    loop {
        let mut line = String::new();
        assert_ne!(reader.read_line(&mut line).expect("request head"), 0, "complete head");
        if line == "\r\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = Some(value.trim().parse::<usize>().expect("body length"));
        }
    }
    let mut body = vec![0; length.expect("Content-Length")];
    reader.read_exact(&mut body).expect("complete request body");
    reader
        .get_mut()
        .write_all(b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n")
        .expect("no-route answer");
}

struct GitRow {
    name: &'static str,
    endpoint: GitEndpoint,
    verb: bool,
    missing: Option<&'static str>,
    steps: Vec<GitStep>,
    files: Vec<(&'static str, Vec<u8>)>,
    branch: Option<&'static str>,
    unchanged: bool,
}

#[test]
#[expect(clippy::too_many_lines, reason = "one process table covers the git adapter's invocations and exits")]
fn git_run() {
    use checkout_git::Kind;
    // The seeded repository has commit 1. One commit-tree stores commit 2.
    const BASE: &[u8] = b"910a2dec89025cc1000000000000000100000000";
    const STORED: &[u8] = b"975835de1c9756ce000000000000000200000000";
    // The FNV tree id for forge::FILES with new.txt = "new\n" added.
    const TREE: &[u8] = b"a48b10c7a2c77a727ec978a5592769d800000000";
    let object = |id| invocation::object(id).expect("fixture object id");
    let clone = || {
        git_step(
            GitCommand::Clone { remote: Box::new([]), directory: b"widget".as_slice().into() },
            Some(Kind::Clone),
            "work",
        )
    };
    let check_out = || git_step(GitCommand::CheckOut { commit: object(BASE) }, Some(Kind::CheckOut), "work/widget");
    let mut files: Vec<_> = temper_fake::forge::FILES
        .iter()
        .map(|(path, content)| (std::str::from_utf8(path).expect("fixture path"), content.to_vec()))
        .collect();
    files.push((".git/temper-remote", b"{remote}".to_vec()));
    let mut changed_files = files.clone();
    changed_files.push(("new.txt", b"new\n".to_vec()));
    let rows = [
        GitRow {
            name: "version",
            endpoint: GitEndpoint::Forge,
            verb: false,
            missing: None,
            steps: vec![GitStep { stdout: "git version 2.53.0\n", ..git_step(GitCommand::Version, None, "work") }],
            files: vec![],
            branch: None,
            unchanged: true,
        },
        GitRow {
            name: "clone",
            endpoint: GitEndpoint::Forge,
            verb: false,
            missing: None,
            steps: vec![clone(), check_out()],
            files,
            branch: None,
            unchanged: false,
        },
        GitRow {
            name: "push",
            endpoint: GitEndpoint::Forge,
            verb: false,
            missing: None,
            steps: vec![
                clone(),
                check_out(),
                GitStep {
                    edit: Some(("new.txt", b"new\n")),
                    ..git_step(GitCommand::Add, Some(Kind::Commit), "work/widget")
                },
                GitStep {
                    stdout: "a48b10c7a2c77a727ec978a5592769d800000000\n",
                    ..git_step(GitCommand::WriteTree, Some(Kind::Commit), "work/widget")
                },
                GitStep {
                    input: b"made by the process\n",
                    stdout: "975835de1c9756ce000000000000000200000000\n",
                    ..git_step(
                        GitCommand::CommitTree { tree: object(TREE), parent: object(BASE), merging: None },
                        Some(Kind::Commit),
                        "work/widget",
                    )
                },
                GitStep {
                    stdout: "To {remote}\n*\t975835de1c9756ce000000000000000200000000:refs/heads/work\t[new branch]\nDone\n",
                    ..git_step(
                        GitCommand::Push {
                            remote: Box::new([]),
                            commit: object(STORED),
                            branch: b"work".as_slice().into(),
                            expected: None,
                        },
                        Some(Kind::Push),
                        "work/widget",
                    )
                },
            ],
            files: changed_files,
            branch: Some("work"),
            unchanged: false,
        },
        GitRow {
            name: "usage",
            endpoint: GitEndpoint::Forge,
            verb: false,
            missing: None,
            steps: vec![GitStep {
                extra: Some(b"--unsupported"),
                exit: 129,
                stderr: "usage: git [--version] [--help] [-C <path>] [-c <name>=<value>] <command> [<args>]\n",
                ..git_step(GitCommand::Version, None, "work")
            }],
            files: vec![],
            branch: None,
            unchanged: true,
        },
        GitRow {
            name: "environment",
            endpoint: GitEndpoint::Forge,
            verb: false,
            missing: Some("LC_ALL"),
            steps: vec![GitStep {
                exit: 125,
                stderr: "temper fake git: broken: fixed environment value\n",
                ..git_step(GitCommand::Version, None, "work")
            }],
            files: vec![],
            branch: None,
            unchanged: true,
        },
        GitRow {
            name: "no_route",
            endpoint: GitEndpoint::NoRoute,
            verb: false,
            missing: None,
            steps: vec![GitStep { exit: 128, stderr: "fatal: {address} serves no fake git route\n", ..clone() }],
            files: vec![],
            branch: None,
            unchanged: true,
        },
        GitRow {
            name: "refused",
            endpoint: GitEndpoint::Llm,
            verb: false,
            missing: None,
            steps: vec![GitStep {
                exit: 128,
                stderr: "fatal: unable to access '{remote}': The requested URL returned error: 401\n",
                ..clone()
            }],
            files: vec![],
            branch: None,
            unchanged: true,
        },
        GitRow {
            name: "closed",
            endpoint: GitEndpoint::Closed,
            verb: false,
            missing: None,
            steps: vec![GitStep {
                exit: 128,
                stderr: "fatal: unable to access '{remote}': Failed to connect to forge\n",
                ..clone()
            }],
            files: vec![],
            branch: None,
            unchanged: true,
        },
        GitRow {
            name: "verb",
            endpoint: GitEndpoint::Forge,
            verb: true,
            missing: None,
            steps: vec![GitStep { stdout: "git version 2.53.0\n", ..git_step(GitCommand::Version, None, "work") }],
            files: vec![],
            branch: None,
            unchanged: true,
        },
    ];
    for row in rows {
        let scratch = support::Scratch::new(&format!("fake_git_{}", row.name));
        let dir = scratch.path();
        fs::create_dir(dir.join("bin")).expect("git's directory");
        let git = dir.join("bin/git");
        std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_temper"), &git).expect("git link");
        fs::create_dir(dir.join("home")).expect("git's HOME");
        fs::create_dir(dir.join("work")).expect("worker directory");
        let doc = dir.join("forge.json");
        document(&doc, &forge_document(dir));
        let forge = Running::start("forge", &doc, None);
        let token = temper_fake::credential::credential(23, temper_fake::credential::ADMIN_TOKEN);
        let auth = format!("token {}", std::str::from_utf8(&token).expect("hex token"));
        // Setup's admin flag does not itself grant repository access.
        let grant = request(
            forge.address,
            "PUT",
            "/api/v1/repos/fake/widget/collaborators/temper-admin",
            &auth,
            br#"{"permission":"admin"}"#,
        );
        assert!(grant.starts_with(b"HTTP/1.1 204"), "{}", String::from_utf8_lossy(&grant));
        let mut llm = None;
        let mut no_route = None;
        let address = match row.endpoint {
            GitEndpoint::Forge => forge.address,
            GitEndpoint::NoRoute => {
                let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("no-route listener");
                let address = listener.local_addr().expect("no-route address");
                no_route = Some(listener);
                address
            }
            GitEndpoint::Llm => {
                let doc = dir.join("llm.json");
                document(&doc, &llm_document(dir, false));
                let run = Running::start("llm", &doc, None);
                let address = run.address;
                llm = Some(run);
                address
            }
            GitEndpoint::Closed => support::free_addresses::<1>(&[forge.address])[0],
        };
        let remote = format!("http://{address}/fake/widget.git");
        let expected = |template: &str| {
            template.replace("{remote}", &remote).replace("{address}", &address.to_string()).into_bytes()
        };
        let identity = temper_worker_protocol_git::Identity {
            token: skein_lib::Secret::new(token.as_slice().into()),
            login: b"temper-admin".as_slice().into(),
            name: b"QA".as_slice().into(),
            email: b"qa@temper.invalid".as_slice().into(),
        };
        let before = snapshot(&dir.join("work"));
        for mut step in row.steps {
            match &mut step.command {
                GitCommand::Clone { remote: url, .. } | GitCommand::Push { remote: url, .. } => {
                    *url = remote.as_bytes().into();
                }
                GitCommand::Version
                | GitCommand::Fetch { .. }
                | GitCommand::Create { .. }
                | GitCommand::CheckOut { .. }
                | GitCommand::Clean
                | GitCommand::Merge { .. }
                | GitCommand::Unmerged
                | GitCommand::Add
                | GitCommand::DiffIndex { .. }
                | GitCommand::WriteTree
                | GitCommand::CommitTree { .. }
                | GitCommand::IsAncestor { .. } => {}
            }
            let at = dir.join(step.at);
            if let Some((path, content)) = step.edit {
                fs::write(at.join(path), content).expect("worker edit");
            }
            let mut command = Command::new(if row.verb { Path::new(env!("CARGO_BIN_EXE_temper")) } else { &git });
            command.env_clear().current_dir(at).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
            if row.verb {
                command.args(["fake", "git"]);
            }
            command.args(invocation::arguments(&step.command).iter().map(|word| std::ffi::OsStr::from_bytes(word)));
            if let Some(extra) = step.extra {
                command.arg(std::ffi::OsStr::from_bytes(extra));
            }
            for entry in invocation::environment(dir.join("home").as_os_str().as_bytes(), step.kind, Some(&identity)) {
                let split = entry.iter().position(|b| *b == b'=').expect("environment entry");
                command.env(
                    std::ffi::OsStr::from_bytes(&entry[..split]),
                    std::ffi::OsStr::from_bytes(&entry[split + 1..]),
                );
            }
            if let Some(name) = row.missing {
                command.env_remove(name);
            }
            let mut child = command.spawn().expect("git process");
            child.stdin.take().expect("git stdin").write_all(step.input).expect("commit message");
            if let Some(listener) = &no_route {
                no_git_route(listener);
            }
            let output = child.wait_with_output().expect("git exit");
            assert_eq!(
                output.status.code(),
                Some(step.exit),
                "{}: {:?}: {}",
                row.name,
                step.command,
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(output.stdout, expected(step.stdout), "{}: {:?}: stdout", row.name, step.command);
            assert_eq!(output.stderr, expected(step.stderr), "{}: {:?}: stderr", row.name, step.command);
        }
        for (path, content) in row.files {
            let content = if content == b"{remote}" { remote.as_bytes().to_vec() } else { content };
            assert_eq!(
                fs::read(dir.join("work/widget").join(path)).expect("checkout file"),
                content,
                "{}: {path}",
                row.name
            );
        }
        if row.unchanged {
            assert_eq!(snapshot(&dir.join("work")), before, "{}: nothing written", row.name);
        }
        if let Some(branch) = row.branch {
            let response =
                request(forge.address, "GET", &format!("/api/v1/repos/fake/widget/branches/{branch}"), &auth, b"");
            assert!(response.starts_with(b"HTTP/1.1 200"));
            assert_eq!(body(&response)["commit"]["id"], std::str::from_utf8(STORED).expect("hex id"));
        }
        if let Some(llm) = llm {
            llm.stop();
        }
        forge.stop();
        scratch.remove();
    }
}

#[test]
fn check_run() {
    let scratch = support::Scratch::startup("fake-check-run");
    let directory = scratch.path();
    let (agent, _) = support::configuration::documents(directory, env!("CARGO_BIN_EXE_temper"));
    support::write_private(&directory.join("agent.json"), &agent);
    support::write_private(&directory.join("root.der"), skein_tls_world::pki::ROOT);
    document(&directory.join("temper.json"), &support::configuration::check_document(directory));
    let observations = directory.join("observations");
    fs::write(&observations, b"{\"observations_lost\":0}\n").expect("checkpoint");
    let store = directory.join("copy");
    let data = directory.join("data");
    let cache = directory.join("cache");
    let ended = Command::new(env!("CARGO_BIN_EXE_temper"))
        .env_clear()
        .args(["fake", "forge", "check", "--config"])
        .arg(directory)
        .arg("--data")
        .arg(&data)
        .arg("--cache")
        .arg(&cache)
        .arg("1")
        .arg(&observations)
        .arg(&store)
        .output()
        .expect("check process");
    assert_eq!(ended.status.code(), Some(2), "{}", String::from_utf8_lossy(&ended.stderr));
    assert_eq!(ended.stdout, b"{\"undecided\":\"store unreadable\"}\n");
    assert_eq!(
        ended.stderr,
        format!(
            "smith: trust roots: 1 taken, 0 skipped, 0 malformed blocks, 0 other blocks\n{{\"boundary\":{{\"lines\":1,\"lost\":0}}}}\ntemper fake forge check: opening {}: errno 2\n",
            store.display()
        )
        .as_bytes()
    );
    assert!(!data.exists());
    assert!(!cache.exists());
    scratch.remove();
}

#[test]
fn llm_holds_a_piece_until_control_releases_it() {
    let scratch = support::Scratch::new("fake_llm_hold");
    let doc = scratch.path().join("llm.json");
    let mut value = llm_document(scratch.path(), false);
    value["control"] = json!({"socket":"127.0.0.1:0"});
    value["asked"][0]["turns"] = json!([{"lines":[{"pieces":[
        {"text":"first piece","hold":null}, {"text":"second piece","hold":"gate"}
    ]}],"finish":"stop","tokens":6}]);
    document(&doc, &value);
    let run = Running::start("llm", &doc, None);
    let control = run.control.expect("control bound before ready");
    let key = fs::read(scratch.path().join("key")).expect("generated key");
    let authorization = format!("Bearer {}", std::str::from_utf8(&key).expect("key is hex"));
    let mut query = stream(run.address);
    query.write_all(&http("POST", "/v1/responses", &authorization, CHAT)).expect("send query");
    let mut response = Vec::new();
    // Read through the first complete delta event, so the assertion observes
    // the provider's bytes, without a timer or a sleep.
    for _ in 0..16_384 {
        let mut byte = [0];
        query.read_exact(&mut byte).expect("first piece arrives on the open stream");
        response.push(byte[0]);
        if response.windows(b"first piece".len()).any(|part| part == b"first piece") && response.ends_with(b"\n\n") {
            break;
        }
    }
    assert!(response.starts_with(b"HTTP/1.1 200"));
    assert!(String::from_utf8_lossy(&response).contains("first piece"));
    assert!(!String::from_utf8_lossy(&response).contains("second piece"));
    assert!(!String::from_utf8_lossy(&response).contains("response.completed"));
    let unknown = request(control, "GET", "/", "", b"");
    assert!(unknown.starts_with(b"HTTP/1.1 404 "));
    // The control response is an event barrier. A nonblocking read observes
    // any bytes already delivered and proves the stream has not ended.
    query.set_nonblocking(true).expect("nonblocking observation");
    let mut byte = [0];
    loop {
        match query.read(&mut byte) {
            Ok(0) => panic!("held stream ended before its release"),
            Ok(_) => response.push(byte[0]),
            Err(why) if why.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(why) => panic!("held stream: {why}"),
        }
    }
    query.set_nonblocking(false).expect("blocking event reads");
    assert!(!String::from_utf8_lossy(&response).contains("second piece"));
    assert!(!String::from_utf8_lossy(&response).contains("response.completed"));
    let released = request(control, "POST", "/release/gate", "", b"");
    assert!(released.starts_with(b"HTTP/1.1 204 No Content\r\n"));
    query.read_to_end(&mut response).expect("released stream completes");
    let text = String::from_utf8_lossy(&response);
    assert!(text.contains("second piece"), "{text}");
    assert!(text.contains("first piecesecond piece"), "{text}");
    assert_eq!(text.matches("event: response.completed").count(), 1);
    assert!(request(control, "POST", "/release/gate", "", b"").starts_with(b"HTTP/1.1 204 No Content\r\n"));
    let mut after = [0];
    assert_eq!(query.read(&mut after).expect("stream stays complete"), 0);
    run.stop();
    scratch.remove();
}
#[test]
fn llm_refuses_a_hold_without_control() {
    let scratch = support::Scratch::new("fake_llm_hold_refused");
    let doc = scratch.path().join("llm.json");
    let mut value = llm_document(scratch.path(), false);
    value["otherwise"] = json!([{"lines":[{"pieces":[{"text":"held","hold":"gate"}]}],"finish":"stop","tokens":1}]);
    document(&doc, &value);
    let output = command("llm", &doc).output().expect("run fake");
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stderr, b"temper fake llm: control: required when a piece has a hold\n");
    assert!(output.stdout.is_empty());
    assert!(!scratch.path().join("key").exists(), "refusal creates no outputs");
    scratch.remove();
}

#[test]
fn llm_fails_a_credential_its_line_names_and_accepts_the_added_key() {
    use temper_fake::credential::{KEY, added_key, credential};
    let scratch = support::Scratch::new("fake_llm_failing_credentials");
    let dir = scratch.path();
    let mut doc = llm_document(dir, false);
    doc["added_keys"] = json!([dir.join("second.key")]);
    let turn =
        |error| json!({"lines":[{"fail":{"error":error,"credentials":[0]}},{"text":"ok"}],"finish":"stop","tokens":1});
    doc["asked"][0]["turns"] = json!([turn(json!("unauthorized"))]);
    doc["cued"] = json!([{"cue":"limited","turns":[turn(json!({"exhausted":{"retry_after_ms":1000}}))]}]);
    let path = dir.join("llm.json");
    document(&path, &doc);
    let run = Running::start("llm", &path, None);
    let primary = credential(23, KEY);
    let second = credential(23, added_key(0));
    private_hex(&dir.join("key"), &primary);
    private_hex(&dir.join("second.key"), &second);
    assert_ne!(primary, second);
    let auth = |key: &[u8]| format!("Bearer {}", std::str::from_utf8(key).expect("hex key"));
    let refused = request(run.address, "POST", "/v1/responses", &auth(&primary), CHAT);
    assert!(refused.starts_with(b"HTTP/1.1 401 "), "{}", String::from_utf8_lossy(&refused));
    assert_eq!(body(&refused)["error"]["code"], "invalid_api_key");
    let accepted = request(run.address, "POST", "/v1/responses", &auth(&second), CHAT);
    assert!(accepted.starts_with(b"HTTP/1.1 200 "), "{}", String::from_utf8_lossy(&accepted));
    assert!(String::from_utf8_lossy(&accepted).contains("\"text\":\"ok\""));
    let mut limited: Value = serde_json::from_slice(CHAT).expect("query");
    limited["instructions"] = json!("limited");
    let limited = serde_json::to_vec(&limited).expect("query");
    let exhausted = request(run.address, "POST", "/v1/responses", &auth(&primary), &limited);
    assert!(exhausted.starts_with(b"HTTP/1.1 429 "), "{}", String::from_utf8_lossy(&exhausted));
    assert_eq!(body(&exhausted)["error"]["resets_in_seconds"], 1);
    run.stop();
    scratch.remove();
}

#[test]
fn fakes_claim_their_inherited_listeners_before_the_first_drive() {
    for kind in ["llm", "forge"] {
        let scratch = support::Scratch::startup(&format!("fake-inherited-{kind}"));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let mut value =
            if kind == "llm" { llm_document(scratch.path(), false) } else { forge_document(scratch.path()) };
        value["listen"] = json!({"socket":address.to_string()});
        let path = scratch.path().join("fake.json");
        document(&path, &value);
        let mut command = listeners::command(listener, "exec \"$@\" 3<&0 0</dev/null");
        command.args(["fake", kind, "--document"]).arg(path).args(["--ready", "inherited "]);
        let ended = listeners::probe(command, address, "inherited ");
        assert!(ended.ready.is_some(), "fake ready: {:?}", ended.stderr);
        assert!(ended.reply.expect("fake answers").starts_with(b"HTTP/1.1 "));
        assert!(ended.status.success(), "fake drains: {:?}", ended.stderr);
        scratch.remove();
    }
}
