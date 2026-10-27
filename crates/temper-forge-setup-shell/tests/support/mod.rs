use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};
use temper_forge_setup::{place::Place, spec::Spec};
pub struct Scratch {
    path: PathBuf,
}
impl Scratch {
    #[must_use]
    pub fn new(name: &str) -> Self {
        let path = skein_scratch::paths::target_tmp().join(name);
        if path.exists() {
            fs::remove_dir_all(&path).expect("prior scratch");
        }
        fs::create_dir_all(&path).expect("scratch");
        Self { path }
    }
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
    #[must_use]
    pub fn place(&self) -> Place {
        use std::os::unix::ffi::OsStrExt;
        Place { directory: self.path.as_os_str().as_bytes().into(), user: b"test-user".as_slice().into() }
    }
    pub fn remove(self) {
        fs::remove_dir_all(self.path).expect("scratch removed");
    }
}
#[must_use]
pub fn script(directory: &Path, body: &str) -> Spec {
    use std::os::unix::ffi::OsStrExt;
    let file = directory.join("program");
    fs::write(&file, format!("#!/bin/sh\n{body}\n")).expect("script");
    fs::set_permissions(&file, fs::Permissions::from_mode(0o700)).expect("executable");
    Spec {
        program: file.as_os_str().as_bytes().into(),
        directory: directory.as_os_str().as_bytes().into(),
        arguments: Box::new([]),
        environment: Box::new([b"PATH=/usr/bin:/bin".as_slice().into()]),
        output: 1024,
    }
}
#[must_use]
pub fn io() -> skein_io::Limits {
    jig_protocol_config::profile(jig_protocol_config::Profile::Small).io
}

pub const ADMIN: &[u8] = br#"{"id":1,"login":"temper-admin","is_admin":true,"full_name":""}"#;
#[must_use]
pub fn settings(address: std::net::SocketAddr) -> temper_forge_setup_shell::reconcile::Settings {
    use temper_forge_setup::setup::{Account, Entry, Mode, Owned};
    let scratch = Scratch::new(&format!("reconcile-{}", address.port()));
    fs::set_permissions(scratch.path(), fs::Permissions::from_mode(0o700)).expect("private scratch");
    temper_forge_setup_shell::files::write_private(&scratch.path().join("admin_token"), b"admin-token-value\n")
        .expect("admin token");
    let io = io();
    let profile = temper_service::limits::profile(temper_service::limits::Profile::Small).forge_component;
    let entry = Entry {
        asked: false,
        forge: 1,
        directory: Box::new([]),
        bot: Account { login: b"temper-bot".as_slice().into(), email: b"bot@example.invalid".as_slice().into() },
        orgs: Box::new([]),
        mode: Mode::Owned(Owned {
            public: None,
            fetch: temper_forge_setup::setup::Fetch { forgejo: false, runner: false, mirror: None },
            forgejo: Box::new([]),
            runner: None,
            listen: address,
            person: Account { login: b"ada".as_slice().into(), email: b"ada@example.invalid".as_slice().into() },
        }),
    };
    let mut limits = temper_forge_setup::limits::small();
    limits.deadline = skein_lib::Duration::from_secs(2);
    limits.backoff = skein_lib::Duration::from_millis(50);
    limits.backoff_max = skein_lib::Duration::from_millis(100);
    temper_forge_setup_shell::reconcile::Settings {
        setup_index: 0,
        given: temper_forge_setup::given::values(&entry, None, &limits),
        address,
        host: address.to_string().into_bytes().into_boxed_slice(),
        user_agent: "test",
        directory: scratch.path().into(),
        admin_token: scratch.path().join("admin_token"),
        token_bytes: profile.token_bytes,
        hook_secret_bytes: profile.hook_secret_bytes,
        http: skein_forge_connection::limits::http(&profile, false, &io),
        documents: skein_forge_connection::limits::documents(&profile),
        io,
    }
}
pub struct Peer {
    listener: std::net::TcpListener,
}
pub struct Heard {
    pub requests: Vec<Received>,
}
pub struct Received {
    pub line: String,
    pub headers: Vec<(String, String)>,
}
impl Peer {
    #[must_use]
    pub fn reserve() -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("free port");
        Self { listener }
    }
    #[must_use]
    pub fn address(&self) -> std::net::SocketAddr {
        self.listener.local_addr().expect("address")
    }
    pub fn serve_after(self, ms: u64, replies: &[(u16, &[u8])]) -> Heard {
        use std::io::{Read, Write};
        std::thread::sleep(std::time::Duration::from_millis(ms));
        let listener = self.listener;
        listener.set_nonblocking(true).expect("nonblocking accept");
        let clock = skein_shell::Clock::new();
        let mut requests = Vec::new();
        for response in replies.iter().map(Some).chain(std::iter::once(None)) {
            let end = clock.now().now.saturating_add(skein_lib::Duration::from_millis(if response.is_some() {
                400
            } else {
                30
            }));
            let mut socket = loop {
                if let Ok((socket, _)) = listener.accept() {
                    break socket;
                }
                if clock.now().now >= end {
                    return Heard { requests };
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            };
            socket.set_read_timeout(Some(std::time::Duration::from_millis(350))).expect("read timeout");
            let mut bytes = Vec::new();
            let mut byte = [0];
            while !bytes.ends_with(b"\r\n\r\n") {
                if socket.read(&mut byte).unwrap_or(0) == 0 {
                    break;
                }
                bytes.push(byte[0]);
            }
            let text = String::from_utf8(bytes).expect("request head");
            let mut lines = text.lines();
            requests.push(Received {
                line: lines.next().expect("request line").into(),
                headers: lines
                    .filter_map(|line| line.split_once(':'))
                    .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().into()))
                    .collect(),
            });
            if let Some((status, body)) = response {
                write!(
                    socket,
                    "HTTP/1.1 {status} Answer\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .expect("response head");
                socket.write_all(body).expect("response body");
            } else {
                let _ = socket.read(&mut byte);
            }
        }
        Heard { requests }
    }
}
/// One refused answer and the requests it allowed the host to make.
#[must_use]
pub fn refused(status: u16, body: &[u8], expected: &str) -> Heard {
    let peer = Peer::reserve();
    let settings = settings(peer.address());
    std::thread::scope(|scope| {
        let server = scope.spawn(|| peer.serve_after(0, &[(status, body)]));
        let refused = temper_forge_setup_shell::reconcile::reconcile(settings, &skein_shell::Clock::new())
            .expect_err("request refused");
        assert_eq!(refused.line(), expected);
        server.join().expect("peer thread")
    })
}
