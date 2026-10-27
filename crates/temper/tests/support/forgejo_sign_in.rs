//! Owned Forgejo signing its member in through temper's printed setup command.
use super::{
    configuration,
    forge_sign_in::{command, configure, success},
};
use jig_client_protocol_wire as wire;
use serde_json::{Value, json};
use skein_lib::{
    Reader, Writer,
    base64::{self, Alphabet},
};
use std::{fs, path::Path, process::Command};
use std::{
    io::{Read, Write},
    net::TcpStream,
    os::{fd::AsRawFd, unix::process::CommandExt},
    process::{Child, Stdio},
    time::Duration,
};

struct Serve {
    child: Child,
    reaped: bool,
}
impl Serve {
    fn stop(mut self) {
        let clock = skein_shell::Clock::new();
        let deadline = clock.now().now.saturating_add(skein_lib::Duration::from_secs(10));
        skein_shell::signal_group(self.child.id(), skein_io::kernel::Signal::Terminate).expect("terminate serve");
        loop {
            if let Some(status) = self.child.try_wait().expect("serve status") {
                self.reaped = true;
                assert!(status.success(), "serve drained successfully");
                assert!(!skein_shell::group_live(self.child.id()).expect("serve group"), "no surviving serve child");
                return;
            }
            assert!(clock.now().now < deadline, "serve drains within ten seconds");
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}
impl Drop for Serve {
    fn drop(&mut self) {
        if self.reaped {
            return;
        }
        let _ = skein_shell::signal_group(self.child.id(), skein_io::kernel::Signal::Kill);
        self.child.wait().expect("serve reaped");
    }
}

struct Answer {
    status: u16,
    location: Option<String>,
    body: Vec<u8>,
}
#[derive(Default)]
struct Browser {
    // These two loopback origins share a host, as browser cookies do; paths still apply.
    cookies: Vec<(String, String, String)>,
}
impl Browser {
    fn call(&mut self, url: &str, body: Option<&[u8]>) -> Answer {
        let (host, path) = url.strip_prefix("http://").expect("local HTTP").split_once('/').expect("URL path");
        let path = format!("/{path}");
        let address: std::net::SocketAddr = host.parse().expect("literal socket");
        assert!(address.ip().is_loopback(), "fixture peers are loopback only");
        let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2)).expect("HTTP connection");
        stream.set_read_timeout(Some(Duration::from_secs(10))).expect("read bound");
        stream.set_write_timeout(Some(Duration::from_secs(2))).expect("write bound");
        let cookie = self
            .cookies
            .iter()
            .filter(|(_, _, scope)| path.starts_with(scope))
            .map(|(name, value, _)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join("; ");
        let data = body.unwrap_or_default();
        let method = if body.is_some() { "POST" } else { "GET" };
        write!(stream, "{method} {path} HTTP/1.0\r\nHost: {host}\r\nConnection: close\r\nCookie: {cookie}\r\n")
            .expect("request head");
        if body.is_some() {
            write!(stream, "Content-Type: application/octet-stream\r\nJig-Client: browser\r\nOrigin: http://{host}\r\nSec-Fetch-Site: same-origin\r\nContent-Length: {}\r\n", data.len()).expect("browser headers");
        }
        stream.write_all(b"\r\n").expect("head end");
        stream.write_all(data).expect("body");
        let mut bytes = Vec::new();
        stream.take(1 << 20).read_to_end(&mut bytes).expect("bounded answer");
        let split = bytes.windows(4).position(|part| part == b"\r\n\r\n").expect("HTTP head");
        let head = std::str::from_utf8(&bytes[..split]).expect("head text");
        let status = head.split_whitespace().nth(1).expect("status").parse().expect("status number");
        let mut location = None;
        for line in head.lines().skip(1) {
            let (name, value) = line.split_once(':').expect("header");
            let value = value.trim();
            if name.eq_ignore_ascii_case("location") {
                location = Some(if value.starts_with('/') { format!("http://{host}{value}") } else { value.into() });
            }
            if name.eq_ignore_ascii_case("set-cookie") {
                let mut parts = value.split(';');
                let (name, value) = parts.next().expect("cookie").split_once('=').expect("cookie value");
                let scope = parts.find_map(|part| part.trim().strip_prefix("Path=")).unwrap_or("/");
                self.cookies.retain(|(old, _, path)| old != name || path != scope);
                self.cookies.push((name.into(), value.into(), scope.into()));
            }
        }
        Answer { status, location, body: bytes[split + 4..].to_vec() }
    }
}

/// Forgejo's API names the account the provider's claims made.
fn member_named(instance: &skein_throwaway_forgejo::Instance) {
    let response = instance
        .exchange(&skein_throwaway_forgejo::Outgoing::get("/api/v1/users/sam", Some(&instance.admin().token)))
        .expect("member API");
    assert_eq!(response.status, 200, "Forgejo created the member");
    let member: Value = serde_json::from_slice(&response.body).expect("member JSON");
    assert_eq!(member["login"], "sam");
    assert_eq!(member["email"], "sam@example.com");
    // Forgejo keeps the claims' `sub` as the account's login name: Google's
    // provider and the member's address, never the printed link's provider.
    assert_eq!(member["login_name"], "3:sam@example.com");
}

/// The one process proof of Forgejo's settings and the provider's claims.
pub fn run(directory: &Path) {
    let clock = skein_shell::Clock::new();
    let whole = clock.now().now;
    let forge = super::owned::tier_configuration(directory);
    let [temper] = super::free_addresses(&[forge]);
    let [listener] = super::reserve_listeners(&[forge, temper]);
    let google = listener.local_addr().expect("issuer address");
    configure(directory, &forge.to_string(), &temper.to_string(), &google.to_string());
    let registration = json!({"issuer":format!("http://{google}"),"authorization":format!("http://{google}/auth"),"token":format!("http://{google}/token"),"revocation":format!("http://{google}/revoke"),"keys":format!("http://{google}/keys"),"client":"temper-test","scopes":[]});
    let _google =
        super::sign_in::issuer(listener, &registration, format!("http://{temper}/sign-in/google/return").as_bytes());
    let prepared = super::owned::prepare(directory, 1);
    success(&prepared);
    let instructions = command(directory).output().expect("sign-in instructions");
    success(&instructions);
    let secret = fs::read(directory.join("forgejo.secret")).expect("scratch client secret");
    for output in [&instructions.stdout, &instructions.stderr] {
        assert!(!output.windows(secret.len()).any(|part| part == secret), "credential absent from output");
    }
    let printed_command =
        std::str::from_utf8(&instructions.stdout).expect("instructions").lines().next().expect("command");
    let running = super::owned::run(directory, "forgejo");
    let instance = super::owned::attach(directory, forge.port(), running);
    let past_start = clock.now().now;
    let reconciled = super::owned::reconcile(directory, 1);
    success(&reconciled);
    let log = fs::File::create(directory.join("sign-in-serve.log")).expect("private log");
    let mut serve = Serve {
        child: Command::new(skein_scratch::paths::binary("temper"))
            .arg("serve")
            .args(configuration::flags(directory))
            .current_dir(directory)
            .process_group(0)
            .stdout(Stdio::piped())
            .stderr(log)
            .spawn()
            .expect("serve"),
        reaped: false,
    };
    let stdout = serve.child.stdout.take().expect("stdout pipe");
    let link = super::read_line(skein_io::kernel::Fd::new(stdout.as_raw_fd()), super::WITHIN);
    let added = Command::new("/bin/sh")
        .args(["-c", printed_command])
        .current_dir(directory.join("forge-1/forgejo"))
        .output()
        .expect("printed command");
    assert!(added.status.success(), "printed add-oauth command failed");
    for output in [&added.stdout, &added.stderr] {
        let lower = String::from_utf8_lossy(output).to_lowercase();
        assert!(!lower.contains("failed") && !lower.contains("error"), "add-oauth reported failure");
        assert!(!output.windows(secret.len()).any(|part| part == secret), "credential absent from add-oauth output");
    }
    let limits = jig_protocol_config::profile::profile(jig_protocol_config::profile::Profile::Small).clients.wire;
    let link = std::str::from_utf8(&link).expect("printed link");
    let secret = base64::decode(Alphabet::Url, link.split_once("secret=").expect("link secret").1.as_bytes(), 32)
        .expect("link encoding");
    let record = wire::LinkSignIn::new(&limits, wire::LinkSignInParts { secret }).expect("link record");
    let mut out = Writer::new(usize::try_from(record.measure()).expect("record size"));
    record.encode(&mut out).expect("link wire");
    let mut browser = Browser::default();
    let signed = browser.call(&format!("http://{temper}/jig/v1/sign-in/link"), Some(&out.finish()));
    assert_eq!(signed.status, 200);
    assert!(browser.cookies.iter().any(|(name, _, _)| name == "jig-sign-in"), "printed link set the browser cookie");
    let opened = browser.call(&format!("http://{forge}/user/oauth2/temper"), None);
    assert_eq!(opened.status, 307);
    let authorization = opened.location.expect("provider authorization redirect");
    assert!(authorization.starts_with(&format!("http://{temper}/oauth/authorize?")));
    let page = browser.call(&authorization, None);
    assert_eq!(page.status, 200);
    let query = authorization.split_once('?').expect("authorization query").1;
    let record = wire::Authorize::new(&limits, wire::AuthorizeParts { query: query.as_bytes().into() })
        .expect("authorize record");
    let mut out = Writer::new(usize::try_from(record.measure()).expect("record size"));
    record.encode(&mut out).expect("authorize wire");
    let authorized = browser.call(&format!("http://{temper}/jig/v1/authorize"), Some(&out.finish()));
    assert_eq!(authorized.status, 200);
    let wire::AuthorizeAnswer::Go(go) =
        wire::AuthorizeAnswer::decode(&limits, &mut Reader::new(&authorized.body)).expect("authorize answer")
    else {
        panic!("signed-in page must receive go")
    };
    let mut callback = browser.call(std::str::from_utf8(go.location()).expect("callback URL"), None);
    for _ in 0..5 {
        if !matches!(callback.status, 302 | 303 | 307) {
            break;
        }
        callback = browser.call(&callback.location.expect("callback redirect"), None);
    }
    assert_eq!(callback.status, 200);
    member_named(&instance);
    serve.stop();
    instance.stop().expect("Forgejo stopped");
    eprintln!(
        "Forgejo sign-in: whole {:?}; past Forgejo start {:?}",
        clock.now().now.saturating_since(whole),
        clock.now().now.saturating_since(past_start)
    );
}
