// Fixture parsing without a deployment keeps written paths.
use crate::{
    Setup,
    check::{Forge, Members, User},
    limits,
    problem::Problems,
};
use alloc::boxed::Box;
use core::net::{Ipv4Addr, SocketAddr};
use skein_json::Document;
use skein_lib::{Writer, bytes};
pub const TWO: &[u8] = br#"[
{"forge":1,"directory":"forge-1","bot":{"login":"temper","email":"temper@example.invalid"},"orgs":["acme"],"mode":{"managed":{"admin_token":"secrets/forgejo-admin"}}},
{"forge":2,"directory":"forge-2","bot":{"login":"temper","email":"temper@example.invalid"},"mode":{"owned":{"forgejo":"/opt/forgejo/forgejo","runner":{"binary":"/opt/forgejo/forgejo-runner","labels":["temper-host:host"],"capacity":1,"path":"/usr/local/bin:/usr/bin:/bin"},"listen":{"socket":"127.0.0.1:3000"},"person":{"login":"ada","email":"ada@example.invalid"}}}}
]"#;
/// TWO followed by the demo's fake entry (protocol/setup.md, 8).
pub fn three() -> Box<[u8]> {
    changed(b"\n]", br#",
{"forge":3,"directory":"demo/forge","bot":{"login":"temper","email":"temper@example.invalid"},"mode":{"fake":{"listen":{"socket":"127.0.0.1:8369"},"git":"demo/bin/git","owner":"demo","repositories":[{"name":"widget","landing":"main"}]}}}
]"#)
}
pub fn fake_setup() -> Setup {
    read_setup(&three())
}
pub fn document(bytes: &[u8]) -> Document {
    jig_protocol_config::document(
        bytes,
        &jig_protocol_config::DocumentLimits { bytes: 1_048_576, tokens: 16_384, depth: 32 },
    )
    .expect("test JSON")
}
pub fn setup() -> Setup {
    read_setup(TWO)
}
fn read_setup(bytes: &[u8]) -> Setup {
    Setup::read(
        &document(bytes),
        0,
        &limits::small(),
        &jig_protocol_config::Directories::working(),
        &mut Problems::empty(),
    )
    .expect("design's setup")
}
pub fn members() -> Members {
    Members {
        forges: Box::new([
            Forge {
                number: 1,
                api: None,
                plaintext: false,
                git: bytes::copy_of(b"https://forge.example"),
                user: User::File(bytes::copy_of(b"forge-1/user")),
                webhook_secret: None,
                api_token: bytes::copy_of(b"forge-1/api_token"),
                identity_token: bytes::copy_of(b"forge-1/git_token"),
                identity_login: bytes::copy_of(b"temper"),
            },
            Forge {
                number: 2,
                api: Some(SocketAddr::from((Ipv4Addr::LOCALHOST, 3000))),
                plaintext: true,
                git: bytes::copy_of(b"http://127.0.0.1:3000"),
                user: User::File(bytes::copy_of(b"forge-2/user")),
                webhook_secret: None,
                api_token: bytes::copy_of(b"forge-2/api_token"),
                identity_token: bytes::copy_of(b"forge-2/git_token"),
                identity_login: bytes::copy_of(b"temper"),
            },
            Forge {
                number: 3,
                api: Some(SocketAddr::from((Ipv4Addr::LOCALHOST, 8369))),
                plaintext: true,
                git: bytes::copy_of(b"http://127.0.0.1:8369"),
                user: User::File(bytes::copy_of(b"demo/forge/user")),
                webhook_secret: None,
                api_token: bytes::copy_of(b"demo/forge/api_token"),
                identity_token: bytes::copy_of(b"demo/forge/git_token"),
                identity_login: bytes::copy_of(b"temper"),
            },
        ]),
        listeners: Box::new([]),
    }
}
pub fn changed(from: &[u8], to: &[u8]) -> Box<[u8]> {
    changed_in(TWO, from, to)
}
pub fn changed_in(source: &[u8], from: &[u8], to: &[u8]) -> Box<[u8]> {
    let at = bytes::find(source, from).expect("fixture text exists");
    let after = at.checked_add(from.len()).unwrap();
    let len = source.len().checked_sub(from.len()).unwrap().checked_add(to.len()).unwrap();
    let mut w = Writer::new(len);
    for part in [source.get(..at).unwrap(), to, source.get(after..).unwrap()] {
        w.put(part).unwrap();
    }
    w.finish()
}
pub fn owned() -> crate::setup::Owned {
    match setup().entries[1].mode.clone() {
        crate::setup::Mode::Owned(owned) => owned,
        crate::setup::Mode::Managed { .. } | crate::setup::Mode::Fake(_) => panic!("owned fixture"),
    }
}
pub fn place() -> crate::place::Place {
    crate::place::Place { directory: b"/srv/forge-2".as_slice().into(), user: b"ada".as_slice().into() }
}
pub fn secrets() -> crate::state::Secrets {
    use skein_lib::Secret;
    crate::state::Secrets {
        secret_key: Secret::new(b"key-secret".as_slice().into()),
        internal_token: Secret::new(b"internal-secret".as_slice().into()),
        jwt_secret: Secret::new(b"jwt-secret".as_slice().into()),
    }
}

pub struct Drive {
    runner: bool,
    machine: crate::prepare::Prepare,
    out: skein_lib::Queue<crate::prepare::Request>,
}
impl Drive {
    pub fn new(owned: crate::setup::Owned) -> Self {
        Self {
            runner: owned.runner.is_some(),
            machine: crate::prepare::Prepare::new(owned, place(), None),
            out: skein_lib::Queue::with_capacity(crate::prepare::MAX_OUT),
        }
    }
    pub fn send(&mut self, event: crate::prepare::Event) -> crate::prepare::Request {
        crate::prepare::step(&mut self.machine, event, &mut self.out);
        let request = self.out.pop().expect("one request");
        assert!(self.out.is_empty());
        request
    }
    pub fn versions(&mut self, found: crate::prepare::Found, runner: bool) -> crate::prepare::Request {
        use crate::{
            prepare::{Event, Request},
            spec::Which,
        };
        assert_eq!(
            self.send(Event::Found(found)),
            Request::Version { which: Which::Forgejo, spec: crate::spec::version(Which::Forgejo, &owned(), &place()) }
        );
        let mut request = self.send(Event::Printed(line(Which::Forgejo)));
        if runner {
            assert_eq!(
                request,
                Request::Version {
                    which: Which::Runner,
                    spec: crate::spec::version(Which::Runner, &owned(), &place())
                }
            );
            request = self.send(Event::Printed(line(Which::Runner)));
        }
        request
    }
    pub fn reach(&mut self, found: crate::prepare::Found) -> crate::prepare::Request {
        self.versions(found, self.runner)
    }
    pub fn drawn(&mut self, runner: bool) -> crate::prepare::Request {
        use crate::prepare::{Event, Request};
        let count = if runner { 5 } else { 4 };
        for value in 0..count {
            let next = self.send(Event::Drew(draw(value)));
            if value.checked_add(1).unwrap() == count {
                return next;
            }
            assert_eq!(next, Request::Draw);
        }
        unreachable!("four or five draws")
    }
}
pub fn draw(value: u8) -> skein_lib::Secret {
    skein_lib::Secret::new(Box::new([value; 32]))
}
pub fn line(which: crate::spec::Which) -> Box<str> {
    use skein_forge::forgejo::pin::{RUNNER_VERSION, VERSION};
    let (prefix, version) = match which {
        crate::spec::Which::Forgejo => (b"forgejo version ".as_slice(), VERSION),
        crate::spec::Which::Runner => (b"forgejo-runner version v".as_slice(), RUNNER_VERSION),
    };
    let mut w = Writer::new(prefix.len().checked_add(version.len()).unwrap());
    w.put(prefix).unwrap();
    w.put(version.as_bytes()).unwrap();
    let bytes = w.finish();
    let mut chunks = bytes.utf8_chunks();
    let chunk = chunks.next().unwrap();
    assert!(chunk.invalid().is_empty());
    assert!(chunks.next().is_none());
    chunk.valid().into()
}
pub fn first() -> crate::prepare::Found {
    crate::prepare::Found {
        state: None,
        admin_token: false,
        person_headers: [false; 2],
        runner_secret: false,
        files: false,
    }
}
pub fn later() -> crate::prepare::Found {
    use skein_forge::forgejo::pin::{RUNNER_VERSION, VERSION};
    crate::prepare::Found {
        state: Some(crate::state::encode(&crate::state::State {
            forgejo: VERSION.as_bytes().into(),
            runner: Some(RUNNER_VERSION.as_bytes().into()),
            prepared: true,
            secrets: secrets(),
        })),
        admin_token: true,
        person_headers: [true; 2],
        runner_secret: true,
        files: true,
    }
}
pub fn person_created() -> crate::prepare::Event {
    ran(b"generated random password is 'person-password'\nNew user 'ada' has been successfully created!\n")
}
pub fn ran(stdout: &[u8]) -> crate::prepare::Event {
    use crate::prepare::{Event, Exit, Ran};
    Event::Ran(Ran {
        exit: Exit::Code(0),
        stdout: skein_lib::Secret::new(stdout.into()),
        stderr: skein_lib::Secret::new(Box::new([])),
    })
}
pub fn secret(value: &[u8]) -> skein_lib::Secret {
    skein_lib::Secret::new(bytes::copy_of(value))
}
pub fn prepared() -> crate::state::State {
    use skein_forge::forgejo::pin::{RUNNER_VERSION, VERSION};
    crate::state::State {
        forgejo: VERSION.as_bytes().into(),
        runner: Some(RUNNER_VERSION.as_bytes().into()),
        prepared: true,
        secrets: secrets(),
    }
}
pub fn launch(which: crate::spec::Which) -> crate::run::Launch {
    crate::run::launch(&owned(), &place(), which).expect("configured process")
}
pub fn suffixed_line(which: crate::spec::Which, suffix: &str) -> Box<str> {
    let line = line(which);
    let mut writer = Writer::new(line.len().checked_add(suffix.len()).unwrap());
    writer.put(line.as_bytes()).unwrap();
    writer.put(suffix.as_bytes()).unwrap();
    let bytes = writer.finish();
    let mut chunks = bytes.utf8_chunks();
    let chunk = chunks.next().unwrap();
    assert!(chunk.invalid().is_empty());
    assert!(chunks.next().is_none());
    chunk.valid().into()
}

pub fn older(forgejo: Option<&[u8]>, runner: Option<&[u8]>) -> crate::prepare::Found {
    let mut state = prepared();
    if let Some(version) = forgejo {
        state.forgejo = version.into();
    }
    if let Some(version) = runner {
        state.runner = Some(version.into());
    }
    crate::prepare::Found { state: Some(crate::state::encode(&state)), ..later() }
}
pub fn backup(recorded: &[u8]) -> Box<[u8]> {
    let pin = skein_forge::forgejo::pin::VERSION.as_bytes();
    let mut name = Writer::new(recorded.len().checked_add(pin.len()).unwrap().checked_add(16).unwrap());
    for piece in [b"forgejo-".as_slice(), recorded, b"-to-", pin, b".zip"] {
        name.put(piece).unwrap();
    }
    name.finish()
}
pub fn ran_ok() -> crate::prepare::Event {
    ran(b"runner-uuid\n")
}
pub fn run_command(command: crate::spec::Command) -> crate::prepare::Request {
    crate::prepare::Request::Run { command, spec: crate::spec::command(command, &owned(), &place()) }
}
pub fn runner_added() -> crate::prepare::Found {
    let mut state = prepared();
    state.runner = None;
    crate::prepare::Found { state: Some(crate::state::encode(&state)), runner_secret: false, ..later() }
}
pub fn failed() -> crate::prepare::Event {
    crate::prepare::Event::Ran(crate::prepare::Ran {
        exit: crate::prepare::Exit::Code(1),
        stdout: secret(b""),
        stderr: secret(b"broken"),
    })
}

pub fn hooked_members() -> Members {
    let mut members = members();
    members.forges[0].webhook_secret = Some(bytes::copy_of(b"forge-1/webhook_secret"));
    members.forges[1].webhook_secret = Some(bytes::copy_of(b"forge-2/webhook_secret"));
    members
}

pub fn listener() -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, 9898))
}
