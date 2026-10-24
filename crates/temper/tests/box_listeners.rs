//! The box holds serve's clients port before any forge is asked to start.
use std::{
    fs,
    net::{SocketAddr, TcpListener},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("temper-box-listeners-{}-{name}", std::process::id()));
    fs::create_dir(&root).expect("fresh scratch");
    root
}
fn command(root: &Path, action: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_temper"));
    command.arg(action);
    for name in ["config", "data", "cache"] {
        command.arg(format!("--{name}")).arg(root.join(name));
    }
    command
}
fn clients(document: &serde_json::Value) -> SocketAddr {
    document["application"]["clients"]["listener"]["socket"]
        .as_str()
        .expect("clients listener")
        .parse()
        .expect("socket")
}

#[test]
fn a_fresh_box_holds_serves_clients_port() {
    let root = scratch("first");
    let mut child = command(&root, "box").stdout(Stdio::null()).stderr(Stdio::null()).spawn().expect("box");
    let document = root.join("config/temper.json");
    let mut address = None;
    let mut owned = None;
    for _ in 0..600 {
        if let Ok(bytes) = fs::read(&document)
            && let Ok(document) = serde_json::from_slice(&bytes)
        {
            address = Some(clients(&document));
            owned = Some(
                document["application"]["setup"][1]["mode"]["owned"]["listen"]["socket"]
                    .as_str()
                    .expect("owned listener")
                    .parse::<SocketAddr>()
                    .expect("socket"),
            );
            break;
        }
        if child.try_wait().expect("status").is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    // Init is over and the host retains the reservation even while the group stays dormant.
    std::thread::sleep(Duration::from_millis(100));
    let running = child.try_wait().expect("box status").is_none();
    let held = address.map(|address| TcpListener::bind(address).is_err());
    let unheld = owned.map(|address| TcpListener::bind(address).is_ok());
    let dormant = !root.join("data/forge-3").exists();
    if running {
        child.kill().expect("stop fixture box");
    }
    child.wait().expect("box reaped");
    fs::remove_dir_all(&root).expect("cleanup");
    assert!(running, "box stays up");
    assert_eq!(held, Some(true), "the configured clients port stays reserved");
    assert!(dormant, "the owned forge has not been prepared");
    assert_eq!(unheld, Some(true), "the dormant owned forge binds its own port when started");
}

#[test]
fn a_later_box_refuses_a_taken_port_naming_the_port_and_document() {
    let root = scratch("taken");
    let output = command(&root, "init").output().expect("init");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let file = root.join("config/temper.json");
    let bytes = fs::read(&file).expect("document");
    let document = serde_json::from_slice(&bytes).expect("JSON");
    let previous = clients(&document);
    let holder = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).expect("held port");
    let address = holder.local_addr().expect("address");
    let text = String::from_utf8(bytes).expect("UTF-8").replace(&previous.to_string(), &address.to_string());
    fs::write(&file, text).expect("configured taken port");
    let output = command(&root, "box").output().expect("box");
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "taken listener refuses the box");
    assert!(diagnostic.contains(&address.port().to_string()), "{diagnostic}");
    assert!(diagnostic.contains(file.to_str().expect("UTF-8 scratch path")), "{diagnostic}");
    assert!(diagnostic.contains("cannot listen"), "{diagnostic}");
    assert!(output.stdout.is_empty(), "refused before steps start");
    fs::remove_dir_all(&root).expect("cleanup");
}
