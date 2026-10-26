// Bounded admission fixtures have a fake runtime and retain fixture-relative file keys.
//! Fixtures for the owned forge commands, shared with the process tier.
use skein_forge::forgejo::pin::{RUNNER_VERSION, VERSION};
use skein_lib::Duration;
use std::{
    fs,
    net::SocketAddr,
    os::unix::{ffi::OsStrExt, fs::PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Output},
};
pub struct Names<'a> {
    pub forgejo: &'a Path,
    pub runner: Option<&'a Path>,
    pub port: u16,
    pub bot: &'a str,
    pub person: &'a str,
}
pub fn document(value: &mut serde_json::Value, names: &Names<'_>) {
    let mut owned = Owned::example();
    owned.forgejo = ([127, 0, 0, 1], names.port).into();
    owned.forgejo_binary = names.forgejo.into();
    let mut members = application(&owned);
    members["webhooks"] = serde_json::Value::Null;
    members["identities"][0]["login"] = names.bot.into();
    let setup = &mut members["setup"][0];
    setup["bot"] = serde_json::json!({"login":names.bot,"email":format!("{}@example.invalid", names.bot)});
    let mode = &mut setup["mode"]["owned"];
    mode["person"] = serde_json::json!({"login":names.person,"email":format!("{}@example.invalid", names.person)});
    if let Some(binary) = names.runner {
        mode["runner"]["binary"] = serde_json::json!(binary);
    } else {
        mode["runner"] = serde_json::Value::Null;
    }
    for name in ["forges", "identities", "setup"] {
        value["application"][name] = members[name].take();
    }
}

// temper-15 removes this old form and temper-01a's Adopt stand-in.
pub fn old_form(application: &mut serde_json::Value) {
    let webhooks = !application["webhooks"].is_null();
    let forge = &mut application["forges"][0];
    forge["identity"] = 1.into();
    forge["api_token"] = "forge-1/api_token".into();
    forge["user"] = serde_json::json!({"file":"forge-1/user"});
    forge["webhook_secret"] = if webhooks { "forge-1/webhook_secret".into() } else { serde_json::Value::Null };
    application["identities"][0].as_object_mut().expect("identity").remove("forge");
    application["identities"][0]["token"] = "forge-1/git_token".into();
}

pub fn configuration(directory: &Path, names: &Names<'_>) {
    super::configuration(directory, &super::Forge::Unreached);
    let path = directory.join("temper.json");
    let mut value = serde_json::from_slice(&fs::read(&path).expect("configuration")).expect("JSON");
    document(&mut value, names);
    old_form(&mut value["application"]);
    super::write_private(&path, &serde_json::to_vec(&value).expect("JSON"));
}
#[must_use]
pub fn prepare(directory: &Path, forge: u16) -> Output {
    Command::new(skein_scratch::paths::binary("temper"))
        .args(["forge", "prepare"])
        .args(super::configuration::flags(directory))
        .arg(forge.to_string())
        .current_dir(directory)
        .output()
        .expect("temper forge prepare")
}
#[must_use]
pub fn files(directory: &Path) -> Vec<PathBuf> {
    [
        "state",
        "admin_token",
        "person_password",
        "person_read.header",
        "person_write.header",
        "runner/secret",
        "runner/config.yaml",
    ]
    .iter()
    .map(|name| directory.join("forge-1").join(name))
    .collect()
}
pub struct Programs {
    directory: PathBuf,
}
impl Programs {
    pub fn fail(&self, program: &str, command: &str) {
        fs::write(self.directory.join(format!("{program}.fail")), command).expect("failure");
    }
    pub fn mend(&self, program: &str) {
        fs::remove_file(self.directory.join(format!("{program}.fail"))).expect("mended");
    }
    pub fn hang(&self, program: &str, command: &str) {
        fs::write(self.directory.join(format!("{program}.hang")), command).expect("hang");
    }
    #[must_use]
    pub fn hung(&self, within: Duration) -> u32 {
        let clock = skein_shell::Clock::new();
        let deadline = clock.now().now.saturating_add(within);
        loop {
            // The shell creates the file before it writes the pid: an empty file is not yet an answer.
            if let Some(pid) =
                fs::read_to_string(self.directory.join("hung")).ok().and_then(|text| text.trim().parse().ok())
            {
                return pid;
            }
            assert!(clock.now().now < deadline, "command hung within bound");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}
#[must_use]
pub fn scripted(directory: &Path) -> Programs {
    for (program, version) in [
        ("forgejo", format!("forgejo version {VERSION}")),
        ("runner", format!("forgejo-runner version v{RUNNER_VERSION}")),
    ] {
        let body = format!(
            r#"#!/bin/sh
if [ "$1" = --version ]; then echo '{version}'; exit; fi
if [ "$1" = web ] || [ "$3" = daemon ] || [ "$5" = web ]; then
    {{ echo $$; printf '%s\n' "$@"; echo ENVIRONMENT; env; }} > '{directory}/{program}.marker'
    exec sleep 60
fi
if [ "$1" = --work-path ]; then shift 4; fi
base='{directory}'
if [ -f "$base/{program}.fail" ] && [ "$1" = "$(cat "$base/{program}.fail")" ]; then echo broken >&2; exit 1; fi
if [ -f "$base/{program}.hang" ] && [ "$1" = "$(cat "$base/{program}.hang")" ]; then sleep 60 & echo $! > "$base/hung"; wait; fi
case "$1 $3" in
    'admin generate-access-token') echo fixture-admin-token;;
    'admin create') echo "generated random password is 'fixture-person-password'";;
    'forgejo-cli register') echo fixture-runner-uuid;;
esac
"#,
            directory = directory.display()
        );
        let path = directory.join(program);
        fs::write(&path, body).expect("program");
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).expect("executable");
    }
    configuration(
        directory,
        &Names {
            forgejo: &directory.join("forgejo"),
            runner: Some(&directory.join("runner")),
            port: 3000,
            bot: "temper",
            person: "ada",
        },
    );
    Programs { directory: directory.into() }
}
/// Writes a complete pinned preparation with credentials that must never be printed.
pub fn finished(directory: &Path) {
    use skein_lib::Secret;
    use temper_forge_setup::state::{self, Secrets, State};
    let root = directory.join("forge-1");
    fs::create_dir_all(root.join("forgejo")).expect("work path");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("private directory");
    let state = State {
        forgejo: VERSION.as_bytes().into(),
        runner: Some(RUNNER_VERSION.as_bytes().into()),
        prepared: true,
        secrets: Secrets {
            secret_key: Secret::new(b"secret-key-never-print".as_slice().into()),
            internal_token: Secret::new(b"internal-token-never-print".as_slice().into()),
            jwt_secret: Secret::new(b"jwt-secret-never-print".as_slice().into()),
        },
    };
    super::write_private(&root.join("state"), state::encode(&state).expose());
}

/// Whether the process is blocked in its `sleep`. The kernel names the process `sleep` as the exec begins, and
/// the dynamic loader holds a descriptor (3) until it has mapped the libraries, so only a process asleep has
/// closed every descriptor the exec and its startup opened.
fn asleep(pid: u32) -> bool {
    fs::read_to_string(format!("/proc/{pid}/stat"))
        .is_ok_and(|stat| stat.rsplit_once(") ").is_some_and(|(_, rest)| rest.starts_with("S ")))
}

pub struct Marker {
    pub pid: u32,
    pub arguments: Vec<String>,
    pub environment: Vec<String>,
    pub descriptors: Vec<u32>,
}
impl Programs {
    #[must_use]
    pub fn lock(&self, program: &str) -> PathBuf {
        self.directory.join(format!("forge-1/{program}.lock"))
    }
    pub fn marker(&self, program: &str, within: Duration) -> Marker {
        let clock = skein_shell::Clock::new();
        let deadline = clock.now().now.saturating_add(within);
        loop {
            if let Ok(text) = fs::read_to_string(self.directory.join(format!("{program}.marker"))) {
                let mut lines = text.lines();
                if let Some(pid) = lines.next().and_then(|line| line.parse::<u32>().ok())
                    && fs::read_to_string(format!("/proc/{pid}/comm")).is_ok_and(|name| name.trim() == "sleep")
                    && asleep(pid)
                {
                    let arguments =
                        lines.by_ref().take_while(|line| *line != "ENVIRONMENT").map(String::from).collect();
                    let environment = lines.map(String::from).collect();
                    let mut descriptors: Vec<_> = fs::read_dir(format!("/proc/{pid}/fd"))
                        .expect("descriptors")
                        .map(|entry| {
                            entry.expect("fd").file_name().to_str().expect("number").parse().expect("fd number")
                        })
                        .collect();
                    descriptors.sort_unstable();
                    return Marker { pid, arguments, environment, descriptors };
                }
            }
            assert!(clock.now().now < deadline, "process execs sleep within bound");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}
#[must_use]
pub fn command(directory: &Path, program: &str) -> Command {
    let mut command = Command::new(skein_scratch::paths::binary("temper"));
    command
        .args(["forge", "run"])
        .args(super::configuration::flags(directory))
        .args(["1", program])
        .current_dir(directory);
    command
}
/// A process proof's child, reaped even when an assertion fails.
pub struct Running {
    child: Option<std::process::Child>,
    log: PathBuf,
}
impl Running {
    #[must_use]
    pub fn id(&self) -> u32 {
        self.child.as_ref().expect("owned child").id()
    }
    #[must_use]
    pub fn start(directory: &Path, program: &str) -> Self {
        let log = directory.join(if program == "forgejo" { "server.log" } else { "runner.log" });
        let output = fs::File::create(&log).expect("log");
        let child = command(directory, program)
            .stdout(output.try_clone().expect("log clone"))
            .stderr(output)
            .spawn()
            .expect("run");
        Self { child: Some(child), log }
    }
    #[must_use]
    pub fn into_child(mut self) -> std::process::Child {
        self.child.take().expect("owned child")
    }
    pub fn declared(&mut self) {
        let clock = skein_shell::Clock::new();
        let deadline = clock.now().now.saturating_add(Duration::from_secs(60));
        loop {
            let text = fs::read_to_string(&self.log).expect("runner log");
            if text.contains("declared successfully") {
                return;
            }
            assert!(
                self.child.as_mut().expect("owned child").try_wait().expect("status").is_none(),
                "runner exited: {text}"
            );
            assert!(clock.now().now < deadline, "runner did not declare: {text}");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    pub fn stop(mut self) {
        self.kill();
    }
    fn kill(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            child.wait().expect("reaped");
        }
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        self.kill();
    }
}
pub fn answer(port: u16, path: &str, token: Option<&str>) -> std::io::Result<serde_json::Value> {
    use std::io::{Read, Write};
    let mut socket =
        std::net::TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), std::time::Duration::from_millis(200))?;
    socket.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
    let auth = token.map_or_else(String::new, |token| format!("Authorization: token {token}\r\n"));
    write!(socket, "GET {path} HTTP/1.0\r\nHost: 127.0.0.1\r\n{auth}Connection: close\r\n\r\n")?;
    let mut text = String::new();
    socket.take(1 << 20).read_to_string(&mut text)?;
    let (head, body) = text.split_once("\r\n\r\n").ok_or(std::io::ErrorKind::InvalidData)?;
    if !head.starts_with("HTTP/1.0 200 ") && !head.starts_with("HTTP/1.1 200 ") {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    serde_json::from_str(body).map_err(std::io::Error::other)
}
#[must_use]
pub fn await_answer(port: u16, path: &str, token: Option<&str>) -> serde_json::Value {
    let clock = skein_shell::Clock::new();
    let deadline = clock.now().now.saturating_add(Duration::from_secs(60));
    loop {
        if let Ok(value) = answer(port, path, token) {
            return value;
        }
        assert!(clock.now().now < deadline, "{path} did not answer");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
#[must_use]
pub fn prepared_processes(directory: &Path) -> u16 {
    let forgejo = std::env::var_os("TEMPER_FORGEJO").expect("TEMPER_FORGEJO");
    let runner = std::env::var_os("TEMPER_FORGEJO_RUNNER").expect("TEMPER_FORGEJO_RUNNER");
    let [address] = super::free_addresses(&[]);
    let port = address.port();
    configuration(
        directory,
        &Names { forgejo: Path::new(&forgejo), runner: Some(Path::new(&runner)), port, bot: "temper", person: "ada" },
    );
    let prepared = prepare(directory, 1);
    assert_eq!(prepared.status.code(), Some(0), "{}", String::from_utf8_lossy(&prepared.stderr));
    port
}

/// The setup's typed file under a numbered forge directory.
#[must_use]
pub fn file(directory: &Path, forge: u16, file: temper_forge_setup::place::File) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    let place = temper_forge_setup::place::Place {
        directory: directory.join(format!("forge-{forge}")).as_os_str().as_bytes().into(),
        user: Box::new([]),
    };
    PathBuf::from(std::ffi::OsStr::from_bytes(&place.path(file)))
}
pub fn record_versions(directory: &Path, forgejo: Option<&[u8]>, runner: Option<&[u8]>) {
    let path = file(directory, 1, temper_forge_setup::place::File::State);
    let mut state = temper_forge_setup::state::decode(&fs::read(&path).expect("state")).expect("record");
    if let Some(version) = forgejo {
        state.forgejo = version.into();
    }
    if let Some(version) = runner {
        state.runner = Some(version.into());
    }
    super::write_private(&path, temper_forge_setup::state::encode(&state).expose());
}
pub fn recorded_without_runner(directory: &Path) {
    let path = file(directory, 1, temper_forge_setup::place::File::State);
    let mut state = temper_forge_setup::state::decode(&fs::read(&path).expect("state")).expect("record");
    state.runner = None;
    super::write_private(&path, temper_forge_setup::state::encode(&state).expose());
}
pub fn setup_without_runner(directory: &Path) {
    let path = directory.join("temper.json");
    let mut config: serde_json::Value = serde_json::from_slice(&fs::read(&path).expect("config")).expect("JSON");
    config["application"]["setup"][0]["mode"]["owned"]["runner"] = serde_json::Value::Null;
    super::write_private(&path, &serde_json::to_vec(&config).expect("JSON"));
}
pub fn refused(directory: &Path, verb: &str, expected: &[u8]) {
    let result = if verb == "prepare" {
        prepare(directory, 1)
    } else if verb == "reconcile" {
        reconcile(directory, 1)
    } else {
        command(directory, "forgejo").output().expect("run refuses")
    };
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(result.stderr, expected);
}

/// Checks the dump archive's recorded evidence and keeps its bytes and timestamp for a repeated upgrade.
#[must_use]
pub fn dump(directory: &Path) -> (PathBuf, Vec<u8>, fs::Metadata) {
    let backups: Vec<_> = fs::read_dir(directory.join("forge-1/backups"))
        .expect("backups")
        .map(|entry| entry.expect("backup entry").path())
        .collect();
    assert_eq!(backups.len(), 1);
    let backup = backups[0].clone();
    assert_eq!(backup.file_name().expect("backup name"), format!("forgejo-16.0.5-to-{VERSION}.zip").as_str());
    let bytes = fs::read(&backup).expect("backup");
    let meta = fs::metadata(&backup).expect("metadata");
    assert_eq!(meta.permissions().mode() & 0o777, 0o600);
    assert!(bytes.starts_with(b"PK"));
    assert!(skein_lib::bytes::find(&bytes, b"forgejo-db.sql").is_some());
    assert!(skein_lib::bytes::find(&bytes, b"app.ini").is_some());
    (backup, bytes, meta)
}
/// Starts one owned process with its output retained for a failing proof.
#[must_use]
pub fn run(directory: &Path, program: &str) -> Running {
    Running::start(directory, program)
}
/// Prepares and starts, leaving readiness to reconcile's first exchange.
#[must_use]
pub fn start(directory: &Path) -> Running {
    let clock = skein_shell::Clock::new();
    let began = clock.now().now;
    let prepared = prepare(directory, 1);
    assert_eq!(prepared.status.code(), Some(0), "{}", String::from_utf8_lossy(&prepared.stderr));
    eprintln!("stage prepare: {} ms", clock.now().now.saturating_since(began).as_nanos() / 1_000_000);
    let began = clock.now().now;
    let running = run(directory, "forgejo");
    eprintln!("stage start: {} ms", clock.now().now.saturating_since(began).as_nanos() / 1_000_000);
    running
}
#[must_use]
pub fn reconcile(directory: &Path, forge: u16) -> Output {
    Command::new(skein_scratch::paths::binary("temper"))
        .args(["forge", "reconcile"])
        .args(super::configuration::flags(directory))
        .arg(forge.to_string())
        .current_dir(directory)
        .output()
        .expect("temper forge reconcile")
}
/// Attaches the owned Forgejo through preparation's private admin token.
#[must_use]
pub fn attach(directory: &Path, port: u16, running: Running) -> skein_throwaway_forgejo::Instance {
    use skein_throwaway_forgejo::{Instance, Settings, Token};
    let token = fs::read_to_string(directory.join("forge-1/admin_token")).expect("admin token");
    Instance::attach(running.into_child(), port, directory, Token::new(token.trim().into()), &Settings::small())
        .expect("owned forge answers")
}
/// Reads the converged bot, its id, and the absence of an organization.
pub fn assert_converged(directory: &Path, instance: &skein_throwaway_forgejo::Instance) {
    use skein_throwaway_forgejo::{Outgoing, Token};
    let api = fs::read_to_string(directory.join("forge-1/api_token")).expect("API token");
    let token = Token::new(api.trim().into());
    let response = instance.exchange(&Outgoing::get("/api/v1/user", Some(&token))).expect("bot answers");
    assert_eq!(response.status, 200);
    let bot: serde_json::Value = serde_json::from_slice(&response.body).expect("bot JSON");
    assert_eq!(bot["login"], "temper");
    let id = fs::read_to_string(directory.join("forge-1/user")).expect("user").trim().parse::<u64>().expect("id");
    assert_eq!(bot["id"], id);
    let admin = &instance.admin().token;
    let account = instance.exchange(&Outgoing::get("/api/v1/users/temper", Some(admin))).expect("account answers");
    let account: serde_json::Value = serde_json::from_slice(&account.body).expect("account JSON");
    assert_eq!(account["is_admin"], false);
    assert!(account["full_name"].as_str().expect("full name").contains("temper:bot"));
    let org = instance.exchange(&Outgoing::get("/api/v1/orgs/acme", Some(admin))).expect("org answers");
    assert_eq!(org.status, 404);
}
/// A managed forge whose directory reconcile must make itself.
pub fn managed(directory: &Path) {
    configuration(
        directory,
        &Names { forgejo: Path::new("/usr/bin/false"), runner: None, port: 9, bot: "temper", person: "ada" },
    );
    let path = directory.join("temper.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).expect("configuration")).expect("JSON");
    value["application"]["setup"][0]["orgs"] = serde_json::json!(["acme"]);
    value["application"]["setup"][0]["mode"] = serde_json::json!({"managed":{"admin_token":"admin_token"}});
    value["application"]["limits"]["overrides"]["setup.deadline_ms"] = 300.into();
    value["application"]["limits"]["overrides"]["setup.backoff_ms"] = 20.into();
    value["application"]["limits"]["overrides"]["setup.backoff_max_ms"] = 50.into();
    super::write_private(&path, &serde_json::to_vec(&value).expect("JSON"));
    super::write_private(&directory.join("admin_token"), b"managed-admin-token");
}
pub fn configuration_value(directory: &Path, change: fn(&mut serde_json::Value)) {
    let path = directory.join("temper.json");
    let mut value = serde_json::from_slice(&fs::read(&path).expect("configuration")).expect("JSON");
    change(&mut value);
    super::write_private(&path, &serde_json::to_vec(&value).expect("JSON"));
}

/// A minimal owned document for the reconcile proof, with no runner.
#[must_use]
pub fn reconcile_configuration(directory: &Path) -> u16 {
    let binary = std::env::var_os("TEMPER_FORGEJO").expect("TEMPER_FORGEJO");
    let [address] = super::free_addresses(&[]);
    let port = address.port();
    configuration(directory, &Names { forgejo: Path::new(&binary), runner: None, port, bot: "temper", person: "ada" });
    port
}

/// Both pinned executables are required before the process tier starts anything.
#[must_use]
pub fn binaries() -> (PathBuf, PathBuf) {
    let forgejo = std::env::var_os("TEMPER_FORGEJO").expect("TEMPER_FORGEJO must name the pinned Forgejo");
    let runner = std::env::var_os("TEMPER_FORGEJO_RUNNER").expect("TEMPER_FORGEJO_RUNNER must name the pinned runner");
    (forgejo.into(), runner.into())
}
/// Writes the process tier's owned setup and engine configuration without starting a process.
#[must_use]
pub fn tier_configuration(directory: &Path) -> std::net::SocketAddr {
    let owned = Owned::tier();
    super::configuration(
        directory,
        &super::Forge::Throwaway {
            forgejo: &owned.forgejo_binary,
            api: owned.forgejo,
            git: format!("http://{}", owned.forgejo),
        },
    );
    owned.forgejo
}

/// The complete owned deployment and its non-secret files, kept with the guide.
pub const EXAMPLE: &[u8] = include_bytes!("../../../../docs/examples/owned.json");
pub const SHIPPED: [(&str, &[u8]); 1] =
    [("owned/agent.json", include_bytes!("../../../../docs/examples/owned/agent.json"))];
/// The example's three listeners, provider, binaries and optional agent review.
pub struct Owned {
    pub forgejo: SocketAddr,
    pub clients: SocketAddr,
    pub webhooks: SocketAddr,
    pub llm: SocketAddr,
    pub forgejo_binary: PathBuf,
    pub runner_binary: PathBuf,
    pub review: bool,
}
impl Owned {
    #[must_use]
    pub fn example() -> Self {
        Self {
            forgejo: "127.0.0.1:3000".parse().expect("example Forgejo"),
            clients: "127.0.0.1:7100".parse().expect("example clients"),
            webhooks: "127.0.0.1:7101".parse().expect("example webhooks"),
            llm: "127.0.0.1:0".parse().expect("fake peer"),
            forgejo_binary: "/opt/forgejo/forgejo".into(),
            runner_binary: "/opt/forgejo/forgejo-runner".into(),
            review: true,
        }
    }
    #[must_use]
    pub fn tier() -> Self {
        let [forgejo] = super::free_addresses(&[]);
        Self::beside(forgejo)
    }
    #[must_use]
    pub fn beside(forgejo: SocketAddr) -> Self {
        let (forgejo_binary, runner_binary) = binaries();
        let [clients, webhooks] = super::free_addresses(&[forgejo]);
        Self { forgejo, clients, webhooks, forgejo_binary, runner_binary, review: false, ..Self::example() }
    }
}
/// Each value the example names and the test's own in its place; the runner's path goes before Forgejo's,
/// which prefixes it.
fn stand_ins(owned: &Owned) -> [(&'static str, String); 6] {
    [
        ("127.0.0.1:3000", owned.forgejo.to_string()),
        ("127.0.0.1:7100", owned.clients.to_string()),
        ("127.0.0.1:7101", owned.webhooks.to_string()),
        ("api.openai.com:443", owned.llm.to_string()),
        ("/opt/forgejo/forgejo-runner", owned.runner_binary.to_str().expect("runner path").to_owned()),
        ("/opt/forgejo/forgejo", owned.forgejo_binary.to_str().expect("Forgejo path").to_owned()),
    ]
}
/// One shipped file with the test's values; a value the example no longer names is refused rather than kept.
fn edited(bytes: &[u8], owned: &Owned) -> String {
    let text = std::str::from_utf8(bytes).expect("example UTF-8").to_owned();
    stand_ins(owned).iter().fold(text, |text, (needle, value)| {
        assert!(
            std::iter::once(EXAMPLE)
                .chain(SHIPPED.iter().map(|(_, file)| *file))
                .any(|file| std::str::from_utf8(file).expect("example UTF-8").contains(needle)),
            "the owned example names {needle}"
        );
        text.replace(needle, value)
    })
}
fn example_value(owned: &Owned) -> serde_json::Value {
    let mut value: serde_json::Value = serde_json::from_str(&edited(EXAMPLE, owned)).expect("owned JSON");
    value["engine"]["llm"]["endpoints"][0]["address"] = serde_json::json!({"socket":owned.llm.to_string()});
    if !owned.review {
        value["application"]["landing"]["projects"][0]["rules"][0]["gates"] = serde_json::json!([]);
    }
    value
}
#[must_use]
pub fn application(owned: &Owned) -> serde_json::Value {
    let mut value = example_value(owned)["application"].take();
    value["charter"] = 1.into();
    value
}
#[must_use]
pub fn example_document(directory: &Path, owned: &Owned) -> Vec<u8> {
    serde_json::to_string(&example_value(owned))
        .expect("example JSON")
        .replace("/home/me/temper", directory.to_str().expect("deployment path"))
        .replace("/usr/local/bin/temper", skein_scratch::paths::binary("temper").to_str().expect("a UTF-8 path"))
        .into_bytes()
}
/// The guide's copied files and private forge credentials.
pub fn lay(directory: &Path, owned: &Owned) {
    for name in ["owned", "secrets"] {
        let path = directory.join(name);
        fs::create_dir_all(&path).expect("example directory");
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).expect("private directory");
    }
    for (name, bytes) in SHIPPED {
        let text = edited(bytes, owned).replace("/home/me/temper", directory.to_str().expect("deployment path"));
        fs::write(directory.join(name), text).expect("shipped file");
    }
    local_agent(directory, "owned/agent.json", owned.llm);
    super::write_private(&directory.join("secrets/worker.secret"), b"worker-secret\n");
    super::write_private(
        &directory.join("secrets/worker.digest"),
        b"6fb46f7a92742970166379ed5195e79c4493a7cc5664280c039cfd4095ba5faf\n",
    );
}

/// Metadata reads use loopback and fixture trust, never DNS or the machine's root bundle.
fn local_agent(directory: &Path, relative: &str, address: SocketAddr) {
    let path = directory.join(relative);
    let mut agent: serde_json::Value = serde_json::from_slice(&fs::read(&path).expect("agent")).expect("agent JSON");
    for endpoint in agent["endpoints"].as_array_mut().expect("agent endpoints") {
        if endpoint["transport"] == "tls" {
            endpoint["address"] = address.to_string().into();
            endpoint["server_name"] = "skein.test".into();
            endpoint["trust_der"] = serde_json::json!(directory.join("root.der"));
        }
    }
    fs::write(path, serde_json::to_vec(&agent).expect("agent JSON")).expect("fake agent");
    super::write_private(&directory.join("root.der"), skein_tls_world::pki::ROOT);
}

// Reuse the shell's conversion without making a second public configuration API.
#[path = "../../src/agent.rs"]
mod agent;

/// The strings the service counts, at checked bounds; these paths are never opened.
pub struct Bounded {
    pub document: Vec<u8>,
    pub files: temper_service::settings::Files,
    pub agent: temper_service::settings::Agent,
    pub seeds: temper_service::settings::Seeds,
    setup: temper_forge_setup::check::Checked,
    forges: Box<[temper_service::ForgeRuntime]>,
}
impl Bounded {
    #[must_use]
    pub fn forges(&self) -> skein_lib::List<temper_service::ForgeRuntime> {
        use skein_forge_connection::settings::Transport;
        let mut forges = skein_lib::List::with_capacity(u32::try_from(self.forges.len()).expect("forge count"));
        for forge in &self.forges {
            let transport = match &forge.transport {
                Transport::Plaintext { name } => Transport::Plaintext { name: name.clone() },
                Transport::Tls { server_name, trust } => {
                    Transport::Tls { server_name: server_name.clone(), trust: trust.clone() }
                }
            };
            forges
                .push(temper_service::ForgeRuntime {
                    number: forge.number,
                    address: forge.address,
                    transport,
                    trust_roots: forge.trust_roots,
                    trust_root_bytes: forge.trust_root_bytes,
                })
                .expect("configured forge count");
        }
        forges
    }
    #[must_use]
    pub fn accounts(&self) -> temper_service::AccountsRuntime {
        use skein_oauth_accounts::{Endpoint, Origin, Transport};
        let value: serde_json::Value = serde_json::from_slice(&self.document).expect("bounded document");
        let mut runtime = temper_service::AccountsRuntime {
            providers: skein_lib::List::with_capacity(2),
            trust_roots: 0,
            trust_root_bytes: 0,
        };
        for provider in value["engine"]["llm"]["providers"].as_array().expect("providers") {
            let Some(sign_in) = provider["sources"].get("sign_in") else { continue };
            // Synthetic transports check registration and memory without resolving or opening endpoints.
            let endpoint = || {
                let (trust, _) =
                    skein_tls::Config::from_der(&[skein_tls_world::pki::ROOT.into()], &[]).expect("fixture trust");
                Endpoint {
                    address: "127.0.0.1:9".parse().expect("unopened stand-in"),
                    transport: Transport::Tls {
                        server_name: skein_tls::Name::new(sign_in["server_name"].as_str().expect("TLS name"))
                            .expect("configured TLS name"),
                        trust,
                    },
                }
            };
            let mut origins = skein_lib::List::with_capacity(1);
            origins
                .push(Origin {
                    uri: sign_in["issuer"].as_str().expect("issuer").as_bytes().into(),
                    endpoint: endpoint(),
                })
                .expect("one shared issuer origin");
            runtime
                .providers
                .push((
                    u32::try_from(provider["number"].as_u64().expect("provider number")).expect("u32 provider"),
                    endpoint(),
                    origins,
                ))
                .expect("bounded providers");
            // The shell prepares trust twice: the token endpoint and the shared issuer origin.
            // Each trust() accepts at most 1024 roots, each at most 65536 bytes.
            runtime.trust_roots += 2 * 1024;
            runtime.trust_root_bytes = 65_536;
        }
        runtime
    }
    #[must_use]
    pub fn requirements(&self) -> temper_service::startup::Requirements {
        temper_service::startup::memory_requirements(
            &self.document,
            &jig_protocol_config::Directories::working(),
            &self.files,
            &self.agent,
            &self.seeds,
            Some(&self.accounts()),
            self.forges(),
            Some(&self.setup),
        )
        .unwrap_or_else(|why| panic!("{}", String::from_utf8_lossy(&temper_service::startup::lines(&why))))
    }
    /// Admission without constructing a service, for both split and standalone profiles.
    pub fn admission(&self) -> Result<temper_service::startup::Requirements, temper_service::startup::Problems> {
        let value: serde_json::Value = serde_json::from_slice(&self.document).expect("profile JSON");
        let accounts = value.get("engine").map(|_| self.accounts());
        temper_service::startup::memory_requirements(
            &self.document,
            &jig_protocol_config::Directories::working(),
            &self.files,
            &self.agent,
            &self.seeds,
            accounts.as_ref(),
            self.forges(),
            value.get("engine").map(|_| &self.setup),
        )
    }

    #[must_use]
    pub fn memory_terms(&self) -> temper_service::startup::MemoryTerms {
        let value: serde_json::Value = serde_json::from_slice(&self.document).expect("profile JSON");
        let accounts = value.get("engine").map(|_| self.accounts());
        temper_service::startup::memory_terms(
            &self.document,
            &jig_protocol_config::Directories::working(),
            &self.files,
            &self.agent,
            &self.seeds,
            accounts.as_ref(),
            self.forges(),
            value.get("engine").map(|_| &self.setup),
        )
        .unwrap_or_else(|why| panic!("{}", String::from_utf8_lossy(&temper_service::startup::lines(&why))))
    }
    pub fn startup(&self, document: &[u8]) -> Result<temper_service::Service, temper_service::startup::Problems> {
        temper_service::startup::with_setup(
            document,
            &jig_protocol_config::Directories::working(),
            &self.files,
            Some(&self.agent),
            &self.seeds,
            Some(self.accounts()),
            Some(temper_service::WorkerRuntime {
                root: skein_io::kernel::Fd::new(9),
                path: b"/worker".as_slice().into(),
            }),
            self.forges(),
            Some(&self.setup),
            false,
        )
    }
}
/// One owned forge's longest base; small's token bound; two worker paths and two program paths.
#[must_use]
pub fn bounded(directory: &Path) -> Bounded {
    let owned = Owned { forgejo: "127.255.255.254:65535".parse().expect("longest owned forge"), ..Owned::example() };
    lay(directory, &owned);
    let value = serde_json::from_slice(&example_document(directory, &owned)).expect("example JSON");
    bounded_document(
        directory,
        value,
        temper_service::ForgeRuntime {
            number: 1,
            address: owned.forgejo,
            transport: skein_forge_connection::settings::Transport::Plaintext { name: None },
            trust_roots: 0,
            trust_root_bytes: 0,
        },
    )
}

/// The shipped owned form has file-free access and no orgs.
#[must_use]
pub fn bounded_records(directory: &Path) -> Bounded {
    let owned =
        Owned { forgejo: "127.255.255.254:65535".parse().expect("bounded records fixture"), ..Owned::example() };
    lay(directory, &owned);
    let mut value: serde_json::Value =
        serde_json::from_slice(&example_document(directory, &owned)).expect("bounded records fixture");
    // The records-only certificate also admits the longest configured directory
    // and a full records.value_bytes handover, without real filesystem reads.
    value["application"]["setup"][0]["directory"] = format!("/{}", "p".repeat(4095)).into();
    bounded_document(
        directory,
        value,
        temper_service::ForgeRuntime {
            number: 1,
            address: owned.forgejo,
            transport: skein_forge_connection::settings::Transport::Plaintext { name: None },
            trust_roots: 0,
            trust_root_bytes: 0,
        },
    )
}

/// Init's own document and agent, with the same counted paths at their bounds.
#[must_use]
pub fn bounded_template(directory: &Path) -> Bounded {
    use std::num::NonZeroU16;
    use temper_init::{Content, Directories, Facts, Ports};
    let answer = temper_init::files(
        &Facts {
            binary: b"/usr/local/bin/temper",
            git: Some(b"/usr/bin/git"),
            claude: Some(b"/usr/bin/claude"),
            login: Some(b"ada"),
            ports: Ports {
                clients: NonZeroU16::new(8367).expect("clients port"),
                owned: NonZeroU16::new(8368).expect("owned forge port"),
                forge: NonZeroU16::new(8369).expect("the fake forge port is nonzero"),
                llm: NonZeroU16::new(8370).expect("LLM port"),
                webhooks: NonZeroU16::new(8371).expect("webhooks port"),
            },
        },
        &Directories { config: directory.as_os_str().as_bytes(), data: directory.as_os_str().as_bytes() },
        &[7; 32],
        &[8; 32],
    )
    .expect("template files");
    for file in answer.files {
        match file.content {
            Content::Bytes { bytes, .. } => {
                let path = directory.join(std::ffi::OsStr::from_bytes(file.name));
                fs::create_dir_all(path.parent().expect("file parent")).expect("template directories");
                fs::write(path, bytes).expect("template file");
            }
            Content::Link { .. } => {}
        }
    }
    local_agent(directory, "agent.json", "127.0.0.1:8370".parse().expect("LLM stand-in"));
    let value =
        serde_json::from_slice(&fs::read(directory.join("temper.json")).expect("template")).expect("template JSON");
    let (trust, _) = skein_tls::Config::from_der(&[skein_tls_world::pki::ROOT.into()], &[]).expect("fixture trust");
    bounded_document(
        directory,
        value,
        temper_service::ForgeRuntime {
            number: 2,
            address: "127.255.255.254:65535".parse().expect("resolved GitHub stand-in"),
            transport: skein_forge_connection::settings::Transport::Tls {
                server_name: skein_tls::Name::new("api.github.com").expect("GitHub name"),
                trust,
            },
            // The binary's trust() admits at most 1024 roots of 65536 bytes each.
            // Price those bounds, independent of this machine's root bundle.
            trust_roots: 1024,
            trust_root_bytes: 65_536,
        },
    )
}

/// Checked-in process fixture profile with local destinations and synthetic named files.
/// Numeric allowances and policies are left exactly as shipped.
#[must_use]
pub fn deployment_profile(directory: &Path, relative: &str) -> Bounded {
    let root = skein_scratch::paths::workspace();
    let source = root.join(relative);
    let render = |path: &Path| {
        let text = fs::read_to_string(path).expect("profile source");
        let text = text.replace("{{", "{").replace("}}", "}");
        let mut text = text;
        for name in ["box", "engine", "w1", "w2", "forge", "llm"] {
            text = text.replace(&format!("{{proc.{name}.dir}}"), directory.to_str().expect("fixture path"));
        }
        for (name, value) in [
            ("{port.forgejo}", "3000"),
            ("{proc.forge.ready.address}", "127.0.0.1:3000"),
            ("{proc.llm.ready.address}", "127.0.0.1:9000"),
            ("{proc.engine.ready.workers}", "127.0.0.1:7000"),
            ("{artifact.temper}", "/fixture/temper"),
            ("{artifact.git}", "/fixture/git"),
            ("{program.forgejo}", "/fixture/forgejo"),
            ("{program.forgejo-runner}", "/fixture/runner"),
        ] {
            text = text.replace(name, value);
        }
        serde_json::from_str::<serde_json::Value>(&text).expect("rendered profile JSON")
    };
    let value = render(&source);
    let mut agent = render(&source.parent().expect("profile family").join("agent.json"));
    // Keep every Smith profile/model declaration; only destinations are local.
    for endpoint in agent["endpoints"].as_array_mut().expect("agent endpoints") {
        endpoint["address"] = "127.0.0.1:9000".into();
        endpoint["transport"] = "plaintext".into();
        let endpoint = endpoint.as_object_mut().expect("endpoint");
        endpoint.remove("server_name");
        endpoint.remove("trust_der");
    }
    let agent_path = directory.join("agent.json");
    fs::write(&agent_path, serde_json::to_vec(&agent).expect("agent JSON")).expect("local agent");
    let agent = agent::read(&agent_path).expect("actual Smith metadata");
    let document = serde_json::to_vec(&value).expect("profile JSON");
    let (files, setup, forges) = if value.get("engine").is_some() {
        let named =
            temper_service::named(&document, &jig_protocol_config::Directories::working()).unwrap_or_else(|why| {
                panic!("{relative}: {}", String::from_utf8_lossy(&temper_service::startup::lines(&why)))
            });
        let setup = checked_setup(&document, &named);
        let mut forges = skein_lib::List::with_capacity(u32::try_from(named.forges.len()).expect("forges"));
        for forge in &named.forges {
            forges
                .push(temper_service::ForgeRuntime {
                    number: forge.number,
                    address: "127.0.0.1:3000".parse().expect("fake forge"),
                    transport: skein_forge_connection::settings::Transport::Plaintext { name: None },
                    trust_roots: 0,
                    trust_root_bytes: 0,
                })
                .expect("forge count");
        }
        (named.files, setup, forges.into_boxed())
    } else {
        let part = temper_service::startup::worker_part(&document, &jig_protocol_config::Directories::working())
            .unwrap_or_else(|why| {
                panic!("{relative}: {}", String::from_utf8_lossy(&temper_service::startup::lines(&why)))
            });
        (
            part.files,
            temper_forge_setup::check::Checked {
                fetch_bytes: 0,
                records_only: Box::new([]),
                setup: temper_forge_setup::setup::Setup { entries: Box::new([]) },
                limits: temper_forge_setup::limits::small(),
            },
            Box::<[temper_service::ForgeRuntime]>::default(),
        )
    };
    let mut entries = skein_lib::List::with_capacity(u32::try_from(files.len()).expect("named files"));
    for file in files {
        let bytes = if file.path.ends_with(b".user") || file.path.ends_with(b"/user") {
            b"42".as_slice()
        } else {
            b"fixture".as_slice()
        };
        entries.push((file.path, skein_lib::Secret::new(bytes.into()))).expect("named file count");
    }
    Bounded {
        document,
        agent,
        setup,
        forges,
        files: temper_service::settings::Files { entries },
        seeds: temper_service::settings::Seeds {
            deployment: [7; 16],
            installation: [8; 16],
            seed: 7,
            crypto: skein_lib::Secret::new(Box::new([9; 32])),
        },
    }
}

fn bounded_document(directory: &Path, mut value: serde_json::Value, forge: temper_service::ForgeRuntime) -> Bounded {
    use skein_lib::{List, Secret};
    use temper_service::settings::{Files, PROGRAM_PATH_BYTES, Seeds};
    let path = format!("/{}", "p".repeat(PROGRAM_PATH_BYTES - 1));
    value["workers"][0]["agent"]["program"] = path.clone().into();
    value["workers"][0]["agent"]["arguments"][2] = path.clone().into();
    let document = serde_json::to_vec(&value).expect("bounded JSON");
    let named =
        temper_service::named(&document, &jig_protocol_config::Directories::working()).expect("bounded example named");
    let mut forges = List::with_capacity(u32::try_from(named.forges.len()).expect("forge count"));
    for local in &named.forges {
        if local.number != forge.number {
            let jig_protocol_config::Address::Addr(address) = &local.api else {
                panic!("additional bounded forges have socket addresses")
            };
            assert!(matches!(local.transport, temper_service::startup::Transport::Plaintext));
            forges
                .push(temper_service::ForgeRuntime {
                    number: local.number,
                    address: address.socket().expect("local forge socket"),
                    transport: skein_forge_connection::settings::Transport::Plaintext { name: None },
                    trust_roots: 0,
                    trust_root_bytes: 0,
                })
                .expect("configured forge count");
        }
    }
    forges.push(forge).expect("primary forge");
    let mut agent = agent::preflight(&named).expect("ordinary agent declarations");
    assert_eq!(agent.programs.len(), 1, "owned example declares Claude Code");
    agent.programs[0].path = path.as_bytes().into();
    agent.programs[0].directory = path.as_bytes().into();
    let setup = checked_setup(&document, &named);
    let mut named_files = named.files.into_vec();
    named_files.extend(temper_service::startup::setup_files(&setup));
    for read in temper_service::startup::handover_reads(&document, &jig_protocol_config::Directories::working(), &setup)
        .expect("bounded handover declarations")
    {
        named_files.push(jig_protocol_config::File {
            at: jig_protocol_config::Path::root(),
            path: read.path,
            max: read.max,
            private: true,
        });
    }
    let mut entries = List::with_capacity(u32::try_from(named_files.len()).expect("file count"));
    let token_bytes =
        temper_service::limits::profile(temper_service::limits::Profile::Small).forge_component.token_bytes;
    for file in named_files {
        let bytes = match file.path.as_ref() {
            b"forge-1/api_token" | b"demo/forge/api_token" | b"forge-3/api_token" => {
                vec![b't'; usize::try_from(token_bytes).expect("token bound")]
            }
            b"forge-1/user" | b"demo/forge/user" | b"forge-3/user" => b"42".to_vec(),
            b"forge-1/webhook_secret" | b"demo/forge/webhook_secret" | b"forge-3/webhook_secret" => {
                b"webhook-secret".to_vec()
            }
            b"demo/forge/admin_token" => b"0123456789abcdef0123456789abcdef01234567".to_vec(),
            name if name.ends_with(b"/admin_token") => vec![b't'; usize::try_from(file.max).expect("handover bound")],
            b"forge-1/git_token" | b"demo/forge/git_token" | b"forge-3/git_token" => b"git-token".to_vec(),
            name => fs::read(directory.join(std::ffi::OsStr::from_bytes(name))).expect("fixture file"),
        };
        entries.push((file.path, Secret::new(bytes.into_boxed_slice()))).expect("named file count");
    }
    Bounded {
        document,
        setup,
        files: Files { entries },
        agent,
        seeds: Seeds { deployment: [7; 16], installation: [8; 16], seed: 7, crypto: Secret::new(Box::new([9; 32])) },
        forges: forges.into_boxed(),
    }
}

// The bounded strings are synthetic, but the setup check is the shell's check on these same members.
fn checked_setup(bytes: &[u8], named: &temper_service::Named) -> temper_forge_setup::check::Checked {
    use jig_protocol_config::{self as config, read};
    use temper_forge_setup::check::{self, Members, User};
    let document = config::document(bytes, &config::DocumentLimits { bytes: 1_048_576, tokens: 1_048_576, depth: 32 })
        .expect("bounded document");
    let application = read::Object::new(&document, 0)
        .expect("bounded root")
        .member(&document, b"application")
        .expect("bounded application");
    let at = read::Object::new(&document, application)
        .expect("bounded application object")
        .member(&document, b"setup")
        .expect("bounded setup");
    let mut forges = Vec::new();
    for forge in &named.forges {
        let mut identity = None;
        for candidate in &named.identities {
            if candidate.number == forge.identity {
                identity = Some(candidate);
                break;
            }
        }
        let identity = identity.expect("named forge identity");
        forges.push(check::Forge {
            number: forge.number,
            api: match &forge.api {
                config::Address::Addr(address) => address.socket(),
                config::Address::Host { .. } => None,
            },
            plaintext: matches!(forge.transport, temper_service::startup::Transport::Plaintext),
            git: forge.git.clone(),
            user: match &forge.user {
                temper_service::startup::User::Id(id) => User::Id(*id),
                temper_service::startup::User::File(path) => User::File(path.clone()),
            },
            api_token: forge.api_token.clone(),
            webhook_secret: forge.webhook_secret.clone(),
            identity_token: identity.token.clone(),
            identity_login: identity.login.clone(),
        });
    }
    let mut listeners = Vec::new();
    if let config::Address::Addr(address) = &named.clients {
        listeners.extend(address.socket());
    }
    let listener = match &named.webhooks {
        Some(hooks) => match &hooks.listener {
            config::Address::Addr(address) => address.socket(),
            config::Address::Host { .. } => panic!("bounded webhooks use a socket"),
        },
        None => None,
    };
    listeners.extend(listener);
    check::checked(
        &document,
        at,
        &config::Directories::working(),
        &Members { forges: forges.into_boxed_slice(), listeners: listeners.into_boxed_slice() },
        listener,
        &named.overrides,
    )
    .expect("bounded setup check")
}

/// Ten percent headroom, rounded up to two significant decimal digits.
#[must_use]
pub fn memory_with_headroom(required: u64) -> u64 {
    let headed = (u128::from(required) * 11).div_ceil(10);
    let mut place = 1;
    while headed > 99 * place {
        place *= 10;
    }
    u64::try_from(headed.div_ceil(place) * place).expect("memory with headroom fits u64")
}

#[must_use]
pub fn memory_in_band(declared: u64, required: u64) -> bool {
    declared >= required && u128::from(declared) * 4 <= u128::from(required) * 5
}

/// Only this drift check writes the document's memory literals, in the established explicit mode.
pub fn check_memory(relative: &str, required: temper_service::startup::Requirements) {
    let write = std::env::var_os("SKEIN_CODEGEN_WRITE").is_some_and(|value| value == "1");
    check_memory_mode(relative, required, write);
}

pub fn check_memory_mode(relative: &str, required: temper_service::startup::Requirements, write: bool) {
    let path = skein_scratch::paths::package().join(relative);
    let mut text = fs::read_to_string(&path).expect("owned example");
    let value: serde_json::Value = serde_json::from_str(&text).expect("owned JSON");
    let again = "run SKEIN_CODEGEN_WRITE=1 heavy cargo nextest run --locked -p temper --test startup -E 'test(owned_memory_figures)'";
    for (previous, required, name) in [
        (
            value["engine"]["memory_bytes"].as_u64().expect("engine memory_bytes"),
            required.engine_bytes,
            "engine.memory_bytes",
        ),
        (
            value["workers"][0]["memory_bytes"].as_u64().expect("worker memory_bytes"),
            required.worker_bytes,
            "workers[0].memory_bytes",
        ),
    ] {
        let declared = if write && !memory_in_band(previous, required) {
            let declared = memory_with_headroom(required);
            let literal = format!("\"memory_bytes\": {previous}");
            assert_eq!(text.matches(&literal).count(), 1, "one literal per envelope");
            text = text.replace(&literal, &format!("\"memory_bytes\": {declared}"));
            declared
        } else {
            previous
        };
        assert!(
            memory_in_band(declared, required),
            "{relative}: {name} declares {declared}, outside worst case {required} through 1.25 times it; {again}"
        );
    }
    if write {
        fs::write(&path, text).expect("write only owned memory literals");
    }
}
/// temper-01a's Adopt stand-in (forge-setup.md, 4.5 and 4.8); temper-15 removes it.
/// Uses the kept webhook secret, drawing one only when absent.
/// Uses the person's token because preparation's admin token carries no write:repository; temper-14 settles Adopt's scopes.
pub fn adopt(
    directory: &Path,
    instance: &skein_throwaway_forgejo::Instance,
    person: &skein_throwaway_forgejo::Token,
    listener: Option<SocketAddr>,
) {
    use skein_throwaway_forgejo::Outgoing;
    let response = instance
        .exchange(&Outgoing::json(
            "PUT",
            "/api/v1/repos/acme/widget/collaborators/temper",
            Some(person),
            br#"{"permission":"write"}"#,
        ))
        .expect("collaborator answers");
    assert_eq!(response.status, 204, "bot collaborator");
    if let Some(listener) = listener {
        let path = directory.join("forge-1/webhook_secret");
        let secret = match fs::read_to_string(&path) {
            Ok(secret) => secret,
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => {
                let seed = skein_shell::crypto_seed().expect("hook entropy");
                let mut secret = String::with_capacity(64);
                for byte in seed.expose() {
                    for nibble in [byte / 16, byte % 16] {
                        secret.push(char::from(b"0123456789abcdef"[usize::from(nibble)]));
                    }
                }
                super::write_private(&path, secret.as_bytes());
                secret
            }
            Err(why) => panic!("kept hook secret unreadable: {why}"),
        };
        let events: Vec<_> = skein_forge::forgejo::delivery::SUBSCRIBED
            .iter()
            .map(|event| std::str::from_utf8(event).expect("event name"))
            .collect();
        let body = serde_json::to_vec(&serde_json::json!({"type":"forgejo", "config":{
            "url":format!("http://{listener}/hooks/1"), "content_type":"json", "secret":secret},
            "events":events, "active":true}))
        .expect("hook JSON");
        let response = instance
            .exchange(&Outgoing::json("POST", "/api/v1/repos/acme/widget/hooks", Some(person), &body))
            .expect("hook answers");
        assert_eq!(response.status, 201, "repository hook");
    }
}
