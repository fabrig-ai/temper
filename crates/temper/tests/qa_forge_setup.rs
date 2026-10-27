//! Design-only QA of protocol/setup.md, 6.5, 7.1 and 8.2–8.3, through the shipped binary.
#[path = "support/listeners.rs"]
pub mod listeners;
pub mod support;

use std::{collections::BTreeSet, fs, net::TcpListener, path::Path, process::Command};

use serde_json::Value;
use skein_io::kernel::Exit;
use skein_lib::Duration;

const WITHIN: Duration = Duration::from_secs(10);
const FILES: [&str; 4] = ["api_token", "git_token", "user", "webhook_secret"];

fn init(directory: &Path) -> (Value, String) {
    let output = Command::new(skein_scratch::paths::binary("temper"))
        .arg("init")
        .args(support::configuration::flags(directory))
        .env_clear()
        .output()
        .expect("run init");
    assert!(output.status.success(), "init succeeds");
    assert!(output.stdout.is_empty(), "init has no stdout");
    let value = serde_json::from_slice(&fs::read(directory.join("temper.json")).expect("document")).expect("JSON");
    (value, String::from_utf8(output.stderr).expect("stderr text"))
}

fn entry<'a>(value: &'a Value, list: &str, key: &str, number: u16) -> &'a Value {
    value["application"][list].as_array().expect("entries").iter().find(|entry| entry[key] == number).expect("entry")
}

fn write(directory: &Path, value: &Value) {
    support::write_private(&directory.join("temper.json"), &serde_json::to_vec(value).expect("JSON"));
}

fn boxed(directory: &Path) {
    support::own_listener_files(directory);
    let ended = support::boxed::run(directory, None, support::boxed::Plan::Stop, WITHIN);
    assert!(support::printed_link(&ended.stdout).is_some(), "box reaches sign-in");
    assert_eq!(ended.exit, Some(Exit::Code(0)), "box stops cleanly");
    assert!(ended.left.as_ref().is_some_and(Vec::is_empty), "no surviving children");
}

#[test]
fn init_wires_forge_three_as_an_asked_owned_forge() {
    let scratch = support::Scratch::startup("qa-setup-members");
    let (value, _) = init(scratch.path());
    let forge = entry(&value, "forges", "number", 3);
    let setup = entry(&value, "setup", "forge", 3);
    assert_eq!(forge["kind"], "forgejo");
    assert_eq!(forge["transport"], "plaintext");
    assert_eq!(forge["api"], setup["mode"]["owned"]["listen"]);
    assert_eq!(forge["git"], format!("http://{}", forge["api"]["socket"].as_str().expect("socket")));
    // M4: the owned forge starts from its handover and records, naming no credential file.
    for member in ["api_token", "user", "webhook_secret"] {
        assert!(forge.get(member).is_none(), "the owned forge names no {member}");
    }
    let identity = entry(&value, "identities", "forge", 3);
    assert_eq!(identity["login"], "temper");
    assert!(identity.get("token").is_none(), "the owned identity names no token file");
    assert_eq!(setup["directory"], "forge-3");
    assert_eq!(setup["start"], "asked");
    assert_eq!(setup["bot"]["login"], "temper");
    let owned = &setup["mode"]["owned"];
    assert_eq!(owned["forgejo"], "fetched");
    assert_eq!(owned["runner"]["binary"], "fetched");
    assert_eq!(owned["runner"]["labels"], serde_json::json!(["temper-host:host"]));
    assert_eq!(owned["runner"]["capacity"], 1);
    assert!(owned["runner"]["path"].as_str().is_some_and(|path| !path.is_empty()));
    assert!(!scratch.path().join("forge-3").exists());
    scratch.remove();
}

#[test]
fn init_uses_the_effective_uids_login_or_names_its_fallback() {
    let scratch = support::Scratch::startup("qa-setup-person");
    let (value, stderr) = init(scratch.path());
    let uid = Command::new("/usr/bin/id").arg("-u").output().expect("effective uid");
    assert!(uid.status.success());
    let uid = String::from_utf8(uid.stdout).expect("uid text");
    let passwd = fs::read_to_string("/etc/passwd").expect("passwd");
    let found = passwd.lines().find_map(|line| {
        let fields: Vec<_> = line.split(':').collect();
        (fields.get(2) == Some(&uid.trim())).then(|| fields[0])
    });
    let person = &entry(&value, "setup", "forge", 3)["mode"]["owned"]["person"];
    let login = person["login"].as_str().expect("person login");
    if let Some(found) = found {
        if login == found {
            assert!(!stderr.contains("cannot be a Forgejo login"));
        } else {
            assert_eq!(login, "person");
            assert!(stderr.contains(&format!("init: {found} cannot be a Forgejo login: forge 3's person is person")));
        }
    } else {
        assert_eq!(login, "person");
        assert!(
            stderr
                .contains(&format!("init: uid {} has no login in /etc/passwd: forge 3's person is person", uid.trim()))
        );
    }
    assert_eq!(person["email"], format!("{login}@localhost"));
    assert!(person.get("orgs").is_none());
    scratch.remove();
}

#[test]
fn init_chooses_five_distinct_fixed_ports_when_defaults_are_occupied() {
    let held: Vec<_> = (8367..=8371).map(|port| TcpListener::bind(("127.0.0.1", port))).collect();
    assert!(
        held.iter()
            .all(|result| result.as_ref().err().is_none_or(|error| error.kind() == std::io::ErrorKind::AddrInUse))
    );
    let scratch = support::Scratch::startup("qa-setup-ports");
    let (value, _) = init(scratch.path());
    let application = &value["application"];
    let sockets = [
        &application["clients"]["listener"],
        &entry(&value, "setup", "forge", 3)["mode"]["owned"]["listen"],
        &entry(&value, "setup", "forge", 1)["mode"]["fake"]["listen"],
        &application["demo"]["llm"]["listen"],
        &application["webhooks"]["listener"],
    ];
    let ports: BTreeSet<_> = sockets
        .iter()
        .map(|socket| {
            let address: std::net::SocketAddr = socket["socket"].as_str().expect("socket").parse().expect("address");
            assert!(address.ip().is_loopback());
            assert_ne!(address.port(), 0);
            assert!(!(8367..=8371).contains(&address.port()), "an occupied default is replaced");
            address.port()
        })
        .collect();
    assert_eq!(ports.len(), 5, "all five chosen sockets must be held together");
    assert_eq!(entry(&value, "forges", "number", 3)["api"], sockets[1].clone());
    scratch.remove();
}

#[test]
fn a_box_on_inits_document_leaves_the_asked_forge_and_cache_absent() {
    let scratch = support::Scratch::startup("qa-setup-dormant");
    let config = scratch.path().join("config");
    let data = scratch.path().join("data");
    let cache = scratch.path().join("cache");
    let flags = [
        "--config".into(),
        config.as_os_str().to_owned(),
        "--data".into(),
        data.as_os_str().to_owned(),
        "--cache".into(),
        cache.as_os_str().to_owned(),
    ];
    let output = Command::new(skein_scratch::paths::binary("temper"))
        .arg("init")
        .args(&flags)
        .env_clear()
        .output()
        .expect("init");
    assert!(output.status.success());
    support::own_listener_files(&config);
    let command = skein_world::end_to_end::Command {
        program: skein_scratch::paths::binary("temper"),
        arguments: std::iter::once("box".into()).chain(flags).collect(),
        directory: scratch.path().into(),
        environment: Vec::new(),
    };
    let ended = support::boxed::run_command(command, None, support::boxed::Plan::Stop, WITHIN);
    assert!(support::printed_link(&ended.stdout).is_some(), "box reaches sign-in");
    assert_eq!(ended.exit, Some(Exit::Code(0)));
    assert!(ended.left.as_ref().is_some_and(Vec::is_empty));
    assert!(!data.join("forge-3").exists(), "no preparation before an ask");
    assert_eq!(fs::read_dir(cache).expect("cache directory").count(), 0, "no fetched binaries before an ask");
    scratch.remove();
}

#[test]
#[ignore = "superseded: M4 starts an owned forge from its handover and records and reads none of these files (setup.md 6.5); the old form goes with temper-15"]
fn asked_forge_accepts_every_subset_of_missing_files() {
    let scratch = support::Scratch::startup("qa-setup-subsets");
    init(scratch.path());
    boxed(scratch.path());
    fs::create_dir(scratch.path().join("forge-3")).expect("forge directory");
    for mask in 0..16 {
        for (bit, file) in FILES.iter().enumerate() {
            let target = scratch.path().join("forge-3").join(file);
            if mask & (1 << bit) == 0 {
                if target.exists() {
                    fs::remove_file(target).expect("absent file");
                }
            } else {
                fs::copy(scratch.path().join("demo/forge").join(file), target).expect("private fixture file");
            }
        }
        let command = skein_world::end_to_end::Command {
            program: skein_scratch::paths::binary("temper"),
            arguments: std::iter::once("serve".into()).chain(support::configuration::flags(scratch.path())).collect(),
            directory: scratch.path().into(),
            environment: Vec::new(),
        };
        let ended = support::boxed::run_command(command, None, support::boxed::Plan::Stop, WITHIN);
        assert!(
            support::printed_link(&ended.stdout).is_some(),
            "asked forge missing-file mask {mask} must not refuse: {}",
            String::from_utf8_lossy(&ended.stderr)
        );
        assert_eq!(ended.exit, Some(Exit::Code(0)), "mask {mask} starts and stops");
    }
    scratch.remove();
}

#[test]
#[ignore = "superseded: M4 starts an owned forge from its handover and records and reads none of these files (setup.md 6.5); the old form goes with temper-15"]
fn an_unasked_owned_forge_refuses_each_missing_file() {
    let scratch = support::Scratch::startup("qa-setup-unasked");
    let (mut value, _) = init(scratch.path());
    boxed(scratch.path());
    value["application"]["setup"]
        .as_array_mut()
        .expect("setup")
        .iter_mut()
        .find(|entry| entry["forge"] == 3)
        .expect("owned setup")
        .as_object_mut()
        .expect("entry")
        .remove("start");
    write(scratch.path(), &value);
    fs::create_dir(scratch.path().join("forge-3")).expect("forge directory");
    for absent in FILES {
        for file in FILES {
            let target = scratch.path().join("forge-3").join(file);
            if file == absent {
                if target.exists() {
                    fs::remove_file(target).expect("absent file");
                }
            } else {
                fs::copy(scratch.path().join("demo/forge").join(file), target).expect("private fixture file");
            }
        }
        let ended = Command::new(skein_scratch::paths::binary("temper"))
            .arg("serve")
            .args(support::configuration::flags(scratch.path()))
            .env_clear()
            .output()
            .expect("serve");
        assert_eq!(ended.status.code(), Some(1), "unasked forge missing {absent} must refuse");
        assert!(support::printed_link(&ended.stdout).is_none());
        assert!(String::from_utf8_lossy(&ended.stderr).contains(absent), "refusal names {absent}");
    }
    scratch.remove();
}

#[test]
fn serve_with_box_refuses_without_the_boxs_pipes() {
    let scratch = support::Scratch::startup("qa-setup-no-pipes");
    init(scratch.path());
    boxed(scratch.path());
    let mut arguments = vec!["internal".into(), "serve".into(), "--box".into()];
    arguments.extend(support::configuration::flags(scratch.path()));
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let mut command = listeners::command(listener, "exec \"$@\" 3<&0 4>&- 5>&- 0</dev/null");
    command.args(arguments);
    let ended = command.output().expect("serve");
    assert_eq!(ended.status.code(), Some(1));
    assert!(support::printed_link(&ended.stdout).is_none());
    let stderr = String::from_utf8_lossy(&ended.stderr);
    assert!(stderr.contains("fd 4"), "refusal names the pipe after the listener: {stderr}");
    scratch.remove();
}

#[test]
fn qa_box_drain_does_not_report_its_own_pipe_as_closed() {
    // protocol/forge.md 2.8: pipe failure is named once; serve's own end is the box's to make.
    let scratch = support::Scratch::startup("qa-m2-box-drain");
    init(scratch.path());
    support::own_listener_files(scratch.path());
    let ended = support::boxed::run(scratch.path(), None, support::boxed::Plan::Stop, WITHIN);
    assert!(support::printed_link(&ended.stdout).is_some(), "box reaches sign-in");
    assert_eq!(ended.exit, Some(Exit::Code(0)));
    assert!(ended.left.as_ref().is_some_and(Vec::is_empty));
    let stderr = String::from_utf8_lossy(&ended.stderr);
    assert!(!stderr.contains("box closed"), "normal drain must not report box closed: {stderr}");
    assert!(!stderr.contains("box link:"), "an idle link has no diagnostic at drain: {stderr}");
    assert!(!scratch.path().join("forge-3").exists(), "no ask, no owned forge preparation");
    scratch.remove();
}
