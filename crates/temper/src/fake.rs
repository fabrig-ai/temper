//! The deployment and document forms' files and peer loop (protocol/setup.md, 8.4).
pub(crate) mod check;
mod control;
pub mod git;
use skein_fake_peers::{forge as forge_peer, llm as llm_peer, oauth as oauth_peer};
use skein_io::kernel::{Complete, Done, Exit, Fd, Op, Submit};
use skein_lib::{List, Queue, Time, Token, Wall};
use skein_shell::{Fallback, Host as _, Origin, Setting, Settings};
use std::{
    cell::RefCell,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    os::unix::{ffi::OsStrExt, fs::OpenOptionsExt},
    path::{Path, PathBuf},
    rc::Rc,
};
use temper_fake::{
    credential::{self, credential},
    demo, forge, limits, llm, oauth,
};

pub const LLM_SETTINGS: Settings = Settings {
    tool: "temper",
    table: &[
        Setting {
            name: "document",
            value: "FILE",
            about: "The fake LLM's JSON document.",
            fallback: Fallback::Text(""),
        },
        Setting {
            name: "ready",
            value: "PREFIX",
            about: "Print PREFIX and the bound address once.",
            fallback: Fallback::Text(""),
        },
        Setting {
            name: "seed",
            value: "N",
            about: "The domain and credential seed, a decimal u64.",
            fallback: Fallback::Text("0"),
        },
        Setting {
            name: "observations",
            value: "FILE",
            about: "Create the LLM's JSON lines query stream.",
            fallback: Fallback::Text(""),
        },
    ],
};
pub const FORGE_SETTINGS: Settings = Settings {
    tool: "temper",
    table: &[
        Setting {
            name: "document",
            value: "FILE",
            about: "The fake forge's JSON document.",
            fallback: Fallback::Text(""),
        },
        Setting {
            name: "ready",
            value: "PREFIX",
            about: "Print PREFIX and the bound address once.",
            fallback: Fallback::Text(""),
        },
        Setting {
            name: "seed",
            value: "N",
            about: "The domain and credential seed, a decimal u64.",
            fallback: Fallback::Text("0"),
        },
        Setting {
            name: "observations",
            value: "FILE",
            about: "Create the forge's JSON lines observation stream.",
            fallback: Fallback::Text(""),
        },
    ],
};
pub const OAUTH_SETTINGS: Settings = Settings {
    tool: "temper",
    table: &[
        Setting {
            name: "document",
            value: "FILE",
            about: "The fake OAuth issuer's JSON document.",
            fallback: Fallback::Text(""),
        },
        Setting {
            name: "ready",
            value: "PREFIX",
            about: "Print PREFIX and the bound address once.",
            fallback: Fallback::Text(""),
        },
        Setting {
            name: "seed",
            value: "N",
            about: "The signed-in model credential seed, a decimal u64.",
            fallback: Fallback::Text("0"),
        },
    ],
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Llm,
    Forge,
    Oauth,
}
impl Kind {
    fn command(self) -> &'static str {
        match self {
            Self::Llm => "temper fake llm",
            Self::Forge => "temper fake forge",
            Self::Oauth => "temper fake oauth",
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum Source {
    Document(PathBuf),
    Demo { directories: super::Directories },
    Setup { directories: super::Directories, forge: u16 },
}
#[derive(Debug, PartialEq, Eq)]
pub struct Arguments {
    pub kind: Kind,
    pub source: Source,
    pub ready: Option<Box<[u8]>>,
    pub seed: u64,
    pub observations: Option<PathBuf>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Check {
    pub forge: u16,
    pub observations: PathBuf,
    pub store: PathBuf,
}
type Bytes = Box<[u8]>;
type EnvironmentEntry = (Bytes, Bytes);
pub fn configure(
    arguments: &[Bytes],
    environment: &[EnvironmentEntry],
    working: &[u8],
) -> Result<super::Command, String> {
    let Some((verb, arguments)) = arguments.split_first() else {
        return Err("temper fake: expected llm, forge, oauth or git\n".into());
    };
    if verb.as_ref() == b"git" {
        return Ok(super::Command::FakeGit { arguments: arguments.into() });
    }
    if verb.as_ref() == b"forge"
        && let Some((word, arguments)) = arguments.split_first()
        && word.as_ref() == b"check"
    {
        return check_command(arguments, environment, working);
    }
    let (kind, table) = match verb.as_ref() {
        b"llm" => (Kind::Llm, &LLM_SETTINGS),
        b"forge" => (Kind::Forge, &FORGE_SETTINGS),
        b"oauth" => (Kind::Oauth, &OAUTH_SETTINGS),
        _ => return Err(format!("temper fake: unknown fake {}\n", String::from_utf8_lossy(verb))),
    };
    let resolved = skein_shell::resolve(table, arguments, environment)
        .map_err(|why| super::prefixed(kind.command(), &why.to_string()))?;
    let source = if resolved.value(0).origin == Origin::Default {
        if kind == Kind::Oauth {
            return Err(super::prefixed(kind.command(), OAUTH_USAGE));
        }
        let (directories, rest) = super::directories::resolve(resolved.rest(), environment, working)
            .map_err(|why| super::prefixed(kind.command(), &why.to_string()))?;
        match kind {
            Kind::Forge => {
                let [number] = rest.as_ref() else {
                    return Err(super::prefixed(kind.command(), FORGE_USAGE));
                };
                let forge = forge_number(number).map_err(|why| super::prefixed(kind.command(), why))?;
                Source::Setup { directories, forge }
            }
            Kind::Llm => {
                if !rest.is_empty() {
                    return Err(super::prefixed(kind.command(), LLM_USAGE));
                }
                Source::Demo { directories }
            }
            Kind::Oauth => unreachable!("OAuth requires a document before resolving directories"),
        }
    } else {
        if let Some(word) = resolved.rest().first() {
            let why = if kind == Kind::Forge {
                FORGE_USAGE.into()
            } else {
                format!("unexpected argument {}", String::from_utf8_lossy(word))
            };
            return Err(super::prefixed(kind.command(), &why));
        }
        Source::Document(super::path(&resolved.value(0).bytes).to_path_buf())
    };
    let seed = &resolved.value(2).bytes;
    let seed = std::str::from_utf8(seed)
        .ok()
        .filter(|text| !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|text| text.parse::<u64>().ok())
        .ok_or_else(|| super::prefixed(kind.command(), "--seed N (TEMPER_SEED): expected a decimal u64"))?;
    // shell.md 6.5 has no absent setting; Default alone means absent.
    // In particular --ready "" is a present, empty prefix.
    let ready = if resolved.value(1).origin == Origin::Default { None } else { Some(resolved.value(1).bytes.clone()) };
    let observations = if kind == Kind::Oauth || resolved.value(3).origin == Origin::Default {
        None
    } else {
        Some(super::path(&resolved.value(3).bytes).to_path_buf())
    };
    Ok(super::Command::Fake { arguments: Arguments { kind, source, ready, seed, observations } })
}

const LLM_USAGE: &str = "usage: temper fake llm [DIRECTORIES] [--ready PREFIX] [--seed N] [--observations FILE] | temper fake llm --document FILE [--ready PREFIX] [--seed N] [--observations FILE]";
const FORGE_USAGE: &str = "usage: temper fake forge [DIRECTORIES] N [--ready PREFIX] [--seed N] [--observations FILE] | temper fake forge --document FILE [--ready PREFIX] [--seed N] [--observations FILE]";
const OAUTH_USAGE: &str = "usage: temper fake oauth --document FILE [--ready PREFIX] [--seed N]";
fn forge_number(number: &[u8]) -> Result<u16, &'static str> {
    std::str::from_utf8(number)
        .ok()
        .filter(|text| !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|text| text.parse::<u16>().ok())
        .ok_or("N: expected a forge number, a decimal u16")
}

fn check_command(
    arguments: &[Bytes],
    environment: &[EnvironmentEntry],
    working: &[u8],
) -> Result<super::Command, String> {
    let refused = |why: &str| super::prefixed("temper fake forge check", why);
    let (directories, rest) =
        super::directories::resolve(arguments, environment, working).map_err(|why| refused(&why.to_string()))?;
    let [number, observations, store] = rest.as_ref() else {
        return Err(refused("expected N OBSERVATIONS STORE"));
    };
    let forge = forge_number(number).map_err(refused)?;
    Ok(super::Command::FakeCheck {
        directories,
        check: Check { forge, observations: super::path(observations).into(), store: super::path(store).into() },
    })
}

/// Creates the peer outputs before readiness. A refused start rolls
/// back only what this invocation created; a running peer leaves every file.
pub fn run(arguments: &Arguments) -> Result<u8, String> {
    let command = arguments.kind.command();
    let mut created = Created::default();
    let opened = start(arguments, &mut created);
    let (mut host, mut kernel) = match opened {
        Ok(opened) => opened,
        Err(why) => {
            created.rollback();
            return Err(super::prefixed(command, &why));
        }
    };
    let exit = skein_shell::drive(&mut kernel, &skein_shell::Clock::new(), &mut host);
    Ok(match exit {
        Exit::Code(code) => code,
        Exit::Signal(_) => 1,
    })
}
#[derive(Default)]
struct Created {
    files: Vec<PathBuf>,
    directories: Vec<PathBuf>,
}
impl Created {
    fn file(&mut self, path: &Path, bytes: &[u8]) -> Result<File, String> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .map_err(|why| format!("{}: {why}", path.display()))?;
        self.files.push(path.into());
        file.write_all(bytes).map_err(|why| format!("{}: {why}", path.display()))?;
        Ok(file)
    }
    fn directory(&mut self, path: &Path) -> Result<(), String> {
        let mut missing = Vec::new();
        for parent in path.ancestors() {
            match fs::symlink_metadata(parent) {
                Ok(_) => break,
                Err(why) if why.kind() == io::ErrorKind::NotFound => missing.push(parent.to_path_buf()),
                Err(why) => return Err(format!("{}: {why}", parent.display())),
            }
        }
        // Record before creation, so a partially successful mkdir is also undone.
        self.directories.extend(missing.into_iter().rev());
        super::directories::make_private(path, skein_shell::effective_user())
    }
    fn rollback(self) {
        for path in self.files.into_iter().rev() {
            let _removed = fs::remove_file(path);
        }
        for path in self.directories.into_iter().rev() {
            let _removed = fs::remove_dir(path);
        }
    }
}
fn read_document(path: &Path, bound: u32) -> Result<Box<[u8]>, String> {
    skein_shell::read_file(path, bound).map_err(|why| format!("{}: {}", path.display(), super::unread(why)))
}
fn forge_document(source: &Source, seed: u64, created: &mut Created) -> Result<(forge::Document, [u8; 40]), String> {
    match source {
        Source::Demo { .. } => unreachable!("forge has document and setup forms"),
        Source::Document(path) => {
            let bytes = read_document(path, temper_fake::DOCUMENT_BYTES)?;
            let document =
                forge::Document::decode(&bytes).map_err(|why| String::from_utf8_lossy(&why.line()).into_owned())?;
            let token = credential(seed, credential::ADMIN_TOKEN);
            let path = super::path(document.admin_token());
            created.directory(path.parent().expect("absolute token file"))?;
            created.file(path, &token)?;
            Ok((document, token))
        }
        Source::Setup { directories, forge } => {
            let bytes = read_document(&directories.document(), super::CONFIG_BYTES)?;
            let super::setup::Deployment { checked, .. } = super::setup::check(&bytes, &directories.jig())?;
            let entry = checked.setup.entry(*forge).ok_or("the setup has no entry for it")?;
            let fake = match &entry.mode {
                temper_forge_setup::setup::Mode::Fake(fake) => fake,
                temper_forge_setup::setup::Mode::Owned(_) => return Err("it is owned, not fake".into()),
                temper_forge_setup::setup::Mode::Managed { .. } => return Err("it is managed, not fake".into()),
            };
            let path = super::path(&entry.directory).join("admin_token");
            let bytes = skein_shell::read_private_file(&path, 40, skein_shell::effective_user())
                .map_err(|why| format!("{}: {}", path.display(), super::unread(why)))?;
            let token: [u8; 40] = bytes
                .expose()
                .try_into()
                .ok()
                .filter(|bytes: &[u8; 40]| bytes.iter().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b)))
                .ok_or_else(|| format!("{}: expected 40 lowercase hexadecimal digits", path.display()))?;
            Ok((forge::Document::fake(fake, path.as_os_str().as_bytes()), token))
        }
    }
}

fn llm_configuration(source: &Source, seed: u64, created: &mut Created) -> Result<llm::Configuration, String> {
    Ok(match source {
        Source::Document(path) => {
            let bytes = read_document(path, temper_fake::DOCUMENT_BYTES)?;
            let document =
                llm::Document::decode(&bytes).map_err(|why| String::from_utf8_lossy(&why.line()).into_owned())?;
            let key = credential(seed, credential::KEY);
            created.file(super::path(document.key()), &key)?;
            for (i, path) in document.added_keys().iter().enumerate() {
                let stream = credential::added_key(u32::try_from(i).expect("fifteen added keys"));
                created.file(super::path(path), &credential(seed, stream))?;
            }
            if let Some(root) = document.root() {
                created.file(super::path(root), skein_fake_peers::transport::ROOT)?;
            }
            llm::configuration(&document, &key, seed)
        }
        Source::Demo { directories } => {
            let bytes = read_document(&directories.document(), super::CONFIG_BYTES)?;
            let deployment = super::setup::check(&bytes, &directories.jig())?;
            let demo = deployment.demo.ok_or("application.demo is null: the deployment has no demo")?;
            let path = super::path(demo.llm().key());
            let file = skein_shell::read_private_file(path, demo::KEY_FILE_BYTES, skein_shell::effective_user())
                .map_err(|why| format!("{}: {}", path.display(), super::unread(why)))?;
            let key = demo::key(file.expose()).map_err(|why| format!("{}: {}", path.display(), why.reason()))?;
            llm::configuration(demo.llm(), key, seed)
        }
        Source::Setup { .. } => unreachable!("LLM has document and demo forms"),
    })
}

fn start(arguments: &Arguments, created: &mut Created) -> Result<(Host, skein_shell::Kernel), String> {
    let listeners = skein_shell::inherited_listeners().map_err(super::listeners_refusal)?;
    if arguments.kind == Kind::Oauth && !listeners.is_empty() {
        super::close(listeners.into_iter().map(|(fd, _)| fd).collect());
        return Err("inherited listeners are not supported by the standalone OAuth issuer".into());
    }
    let mut control_address = None;
    let (mut peer, shared, observations, writer, buffer) = match arguments.kind {
        Kind::Llm => {
            let configuration = llm_configuration(&arguments.source, arguments.seed, created)?;
            control_address = configuration.control;
            let observations = arguments
                .observations
                .as_ref()
                .map(|path| created.file(path, b"{\"observations_lost\":0}\n"))
                .transpose()?;
            let peer =
                if observations.is_some() { llm::recording_peer(configuration) } else { llm::peer(configuration) }
                    .map_err(|why| format!("peer: {why:?}"))?;
            let writer = None;
            let buffer =
                List::with_capacity(if arguments.observations.is_some() { limits::OBSERVATION_BYTES } else { 0 });
            (Peer::Llm(peer), None, observations, writer, buffer)
        }
        Kind::Forge => {
            let (document, token) = forge_document(&arguments.source, arguments.seed, created)?;
            let (peer, shared) =
                forge::peer(&document, &token, arguments.seed).map_err(|why| format!("peer: {why:?}"))?;
            let mut buffer =
                List::with_capacity(if arguments.observations.is_some() { limits::OBSERVATION_BYTES } else { 0 });
            let (observations, writer) = match &arguments.observations {
                Some(path) => {
                    let mut writer = skein_fake_forge_protocol::observations::Writer::new(&forge::LIMITS.limits);
                    writer
                        .stop(&mut shared.borrow_mut().domain, &mut buffer)
                        .map_err(|why| format!("first checkpoint: {why:?}"))?;
                    let file = created.file(path, buffer.as_slice())?;
                    buffer.clear();
                    (Some(file), Some(writer))
                }
                None => (None, None),
            };
            (Peer::Forge(peer), Some(shared), observations, writer, buffer)
        }
        Kind::Oauth => {
            let Source::Document(path) = &arguments.source else { return Err(OAUTH_USAGE.into()) };
            let bytes = read_document(path, oauth::DOCUMENT_BYTES)?;
            let document =
                oauth::Document::decode(&bytes).map_err(|why| String::from_utf8_lossy(&why.line()).into_owned())?;
            let peer = document.peer(arguments.seed).map_err(|why| format!("peer: {why:?}"))?;
            (Peer::Oauth(peer), None, None, None, List::with_capacity(0))
        }
    };
    for (fd, addr) in listeners {
        let adopted = match &mut peer {
            Peer::Llm(peer) => peer.adopt_listener(fd, addr),
            Peer::Forge(peer) => peer.adopt_listener(fd, addr),
            Peer::Oauth(_) => unreachable!("OAuth refused inherited listeners before construction"),
        };
        adopted.map_err(|fd| format!("listener: fd {} at {addr} refused", fd.raw()))?;
    }
    let signals = skein_shell::open_termination_signals().map_err(|why| format!("signals: errno {why}"))?;
    let control = control_address.map(control::Control::new);
    let operations = host_operations(&peer, control.as_ref());
    let host = Host {
        peer,
        control,
        shared,
        signals,
        signal: Signal::New,
        cancel: Cancel::Idle,
        completions: Queue::with_capacity(operations),
        submissions: Queue::with_capacity(operations),
        ready: arguments.ready.clone(),
        observation_path: arguments.observations.clone(),
        observations,
        writer,
        git_calls: forge::observations::GitCalls::new(),
        installations: forge::observations::Installations::new(temper_fake::ENTRIES, 128),
        llm_cursor: 0,
        buffer,
        stopped: false,
        peer_submissions: 0,
        checkpoint: false,
        failed: false,
        command: arguments.kind.command(),
        seed: arguments.seed,
    };
    let kernel = match skein_shell::Kernel::open(skein_shell::Config { operations: host.operations() }) {
        Ok(kernel) => kernel,
        Err(why) => {
            super::close(vec![signals]);
            return Err(why.to_string());
        }
    };
    Ok((host, kernel))
}
#[expect(clippy::large_enum_variant, reason = "one peer lives on the host stack; only one variant is constructed")]
enum Peer {
    Llm(llm_peer::Peer),
    Forge(forge_peer::Peer),
    Oauth(oauth_peer::Peer),
}
impl Peer {
    fn host(&self) -> &dyn skein_shell::Host {
        match self {
            Self::Llm(p) => p,
            Self::Forge(p) => p,
            Self::Oauth(p) => p,
        }
    }
    fn host_mut(&mut self) -> &mut dyn skein_shell::Host {
        match self {
            Self::Llm(p) => p,
            Self::Forge(p) => p,
            Self::Oauth(p) => p,
        }
    }
    fn address(&self) -> Option<std::net::SocketAddr> {
        match self {
            Self::Llm(p) => p.address(),
            Self::Forge(p) => p.address(),
            Self::Oauth(p) => p.address(),
        }
    }
    fn shutdown(&mut self) {
        match self {
            Self::Llm(p) => p.shutdown(),
            Self::Forge(p) => p.shutdown(),
            Self::Oauth(p) => p.shutdown(),
        }
    }
}
fn host_operations(peer: &Peer, control: Option<&control::Control>) -> u32 {
    peer.host()
        .operations()
        .checked_add(2 + control.map_or(0, |_| control::Control::operations()))
        .expect("peer, control and signals")
}
// Both own tokens have slot u32::MAX, which io never issues: at skein
// 9d34791e, crates/skein-io/src/layer.rs:291-303 uses Id::token;
// crates/skein-lib/src/id.rs:41-44 puts the slot in the high half, and
// crates/skein-lib/src/slab.rs:49-56 allocates only 0..capacity (u32).
const SIGNAL: Token = Token::new(u64::MAX);
const CANCEL_SIGNAL: Token = Token::new(u64::MAX - 1);
#[derive(PartialEq, Eq)]
enum Signal {
    New,
    Reading,
    Close,
    Closing,
    Closed,
}
#[derive(PartialEq, Eq)]
enum Cancel {
    Idle,
    Waiting,
}
struct Host {
    peer: Peer,
    control: Option<control::Control>,
    shared: Option<Rc<RefCell<forge_peer::Forge>>>,
    signals: Fd,
    signal: Signal,
    cancel: Cancel,
    completions: Queue<Complete>,
    submissions: Queue<Submit>,
    ready: Option<Box<[u8]>>,
    observation_path: Option<PathBuf>,
    observations: Option<File>,
    writer: Option<skein_fake_forge_protocol::observations::Writer>,
    buffer: List<u8>,
    git_calls: forge::observations::GitCalls,
    installations: forge::observations::Installations,
    llm_cursor: usize,
    stopped: bool,
    peer_submissions: u32,
    checkpoint: bool,
    failed: bool,
    command: &'static str,
    seed: u64,
}
impl Host {
    fn checkpoint_or_llm(&self) -> bool {
        self.shared.is_none() || self.checkpoint
    }
    fn fail(&mut self, name: &str, why: &dyn std::fmt::Display) {
        eprintln!("{}: {name}: {why}", self.command);
        self.failed = true;
        self.stopped = true;
        self.peer.shutdown();
        if let Some(control) = &mut self.control {
            control.shutdown();
        }
        if self.signal == Signal::Reading && self.cancel == Cancel::Idle {
            self.submissions.push(Submit { op: CANCEL_SIGNAL, kind: Op::Cancel { target: SIGNAL } });
            self.cancel = Cancel::Waiting;
        }
    }
    /// The host has ended sending after an unwritten pass: `BrokenPipe`, as
    /// after this side's Shutdown (skein 9d34791e, crates/skein-io/src/kernel.rs:809-812).
    /// Unlike `Cancelled`, this reaches `broken` in `open_sent` and `Flush::Done` in
    /// `closing_sent` (crates/skein-io/src/stream.rs:1186-1190,1364-1366).
    /// The protocol closes a failed stream; `Close` on `Broken` cancels its `Recv`,
    /// settles both completions, then releases the fd (stream.rs:501,
    /// 1423-1449,1476-1497,1515-1530), even if the client keeps it open.
    fn suppress_sends(&mut self) {
        for _ in 0..self.submissions.len() {
            let submission = self.submissions.pop().expect("one pass over submissions");
            if matches!(submission.kind, Op::Send { .. }) {
                self.completions.push(Complete {
                    op: submission.op,
                    kind: submission.kind,
                    result: Err(skein_io::kernel::Error::BrokenPipe),
                });
            } else {
                self.submissions.push(submission);
            }
        }
    }
    fn llm_observations(&mut self) {
        if self.observations.is_none() || self.failed || self.checkpoint {
            return;
        }
        let Peer::Llm(peer) = &self.peer else { unreachable!("LLM records") };
        for record in peer.observations().iter().skip(self.llm_cursor) {
            if let llm_peer::Observation::Query { query, .. } = record
                && llm::observations::encode(query, &mut self.buffer).is_err()
            {
                self.buffer.clear();
                self.fail("observations", &"query record exceeds its byte bound");
                return;
            }
        }
        let observed = peer.observations().len() > self.llm_cursor;
        self.llm_cursor = peer.observations().len();
        // The peer retains connection, answer and query records for its whole life.
        let full = peer.observations().len() >= usize::try_from(limits::LLM_OBSERVATIONS).expect("record cap");
        let stopped = self.stopped && peer.is_empty();
        if !observed && !full && !stopped {
            return;
        }
        let file = self.observations.as_mut().expect("record file");
        let written = file.write_all(self.buffer.as_slice());
        self.buffer.clear();
        let written = written.and_then(|()| file.write_all(b"{\"observations_lost\":0}\n"));
        let written = if full {
            written.and_then(|()| {
                writeln!(file, "{{\"observation\":\"records_full\",\"capacity\":{}}}", limits::LLM_OBSERVATIONS)
            })
        } else {
            written
        };
        if let Err(why) = written {
            self.fail("observations", &why);
            return;
        }
        self.checkpoint |= stopped;
        if full {
            self.fail("observations", &"the LLM record is full; the stream has ended");
        }
    }
    fn observations(&mut self) {
        if matches!(self.peer, Peer::Llm(_)) {
            self.llm_observations();
            return;
        }
        let Some(shared) = &self.shared else { return };
        let stopped = self.stopped && self.peer.host().is_empty();
        let mut forge = shared.borrow_mut();
        // A later checkpoint would certify observations this FILE has lost.
        // After failure every pass, including stop, only drains and drops.
        if !self.failed
            && let Some(writer) = &mut self.writer
        {
            let result = if stopped && !self.checkpoint {
                writer.stop(&mut forge.domain, &mut self.buffer)
            } else {
                writer.drain(&mut forge.domain, &mut self.buffer)
            };
            self.checkpoint |= stopped;
            drop(forge);
            if result.is_err() {
                self.buffer.clear();
                self.fail("observations", &"drain exceeds its byte bound");
                return;
            }
            let full = if let Peer::Forge(peer) = &self.peer
                && let Some(calls) = peer.git_calls()
            {
                if let Ok(full) = self.git_calls.drain(calls, &mut self.buffer) {
                    full
                } else {
                    self.buffer.clear();
                    self.fail("observations", &"git-call drain exceeds its byte bound");
                    return;
                }
            } else {
                false
            };
            let installations_full = if let Peer::Forge(peer) = &self.peer
                && let Some(layer) = peer.github_layer()
            {
                if let Ok(full) = self.installations.drain(layer.installations(), &mut self.buffer) {
                    full
                } else {
                    self.buffer.clear();
                    self.fail("observations", &"installation drain exceeds its byte bound");
                    return;
                }
            } else {
                false
            };
            if !self.buffer.is_empty() {
                let result = self.observations.as_mut().expect("writer has file").write_all(self.buffer.as_slice());
                self.buffer.clear();
                if let Err(why) = result {
                    let path = self.observation_path.as_ref().expect("writer has path").display().to_string();
                    self.fail(&path, &why);
                    return;
                }
            }
            if installations_full {
                self.fail("observations", &"the GitHub installation record is full; the stream has ended");
            }
            if full {
                self.fail(
                    "observations",
                    &"the GitHub git-call record is full (limits.git_records); the stream has ended",
                );
            }
        } else {
            while forge.domain.pop_observation().is_some() {}
            self.checkpoint |= stopped;
        }
    }
}
impl skein_shell::Host for Host {
    fn iterate(&mut self, now: Time, wall: Wall) {
        for _ in 0..self.completions.len() {
            let mut complete = self.completions.pop().expect("one pass over completions");
            if complete.op != SIGNAL && complete.op != CANCEL_SIGNAL && control::owns(complete.op) {
                let control = self.control.as_mut().expect("control owns its completions");
                if control.completions.room() == 0 {
                    self.completions.push(complete);
                    break;
                }
                control::restore(&mut complete);
                control.completions.push(complete);
                continue;
            }
            if complete.op != SIGNAL && complete.op != CANCEL_SIGNAL && self.peer.host_mut().completions().room() == 0 {
                self.completions.push(complete);
                break;
            }
            if complete.op == CANCEL_SIGNAL {
                self.cancel = Cancel::Idle;
                match complete.result {
                    Ok(Done::Nothing) | Err(skein_io::kernel::Error::TooLate) => {}
                    Err(error) => {
                        // InvalidArgument/Other leave the read running (kernel.rs:45-46).
                        // Retry its cancellation; a descriptor closes only after its read.
                        self.fail("signals: cancel", &format_args!("{error:?}"));
                    }
                    Ok(_) => panic!("cancel completes with Nothing"),
                }
            } else if complete.op == SIGNAL {
                match self.signal {
                    Signal::Reading => {
                        self.signal = Signal::Close;
                        if !(matches!(complete.result, Ok(Done::Signalled(_)))
                            || self.failed && complete.result == Err(skein_io::kernel::Error::Cancelled))
                        {
                            self.fail("signals", &"read failed");
                        }
                        self.stopped = true;
                        self.peer.shutdown();
                        if let Some(control) = &mut self.control {
                            control.shutdown();
                        }
                    }
                    Signal::Closing => self.signal = Signal::Closed,
                    Signal::New | Signal::Close | Signal::Closed => panic!("one own operation"),
                }
            } else {
                self.peer.host_mut().completions().push(complete);
            }
        }
        if self.signal == Signal::New && self.failed {
            self.signal = Signal::Close;
        }
        if self.signal == Signal::New {
            self.submissions.push(Submit { op: SIGNAL, kind: Op::ReadSignal { fd: self.signals } });
            self.signal = Signal::Reading;
        }
        if self.signal == Signal::Close && self.cancel == Cancel::Idle {
            self.submissions.push(Submit { op: SIGNAL, kind: Op::Close { fd: self.signals } });
            self.signal = Signal::Closing;
        }
        if let Some(control) = &mut self.control {
            let Peer::Llm(peer) = &mut self.peer else { unreachable!("only LLM has control") };
            control.iterate(now, wall, peer);
            if control.failed && !self.failed {
                self.fail("control", &"listener failed");
            }
        }
        if !self.stopped
            && let Peer::Oauth(peer) = &mut self.peer
            && let Err(why) = oauth::refill(peer, self.seed)
        {
            self.fail("token plans", &format_args!("{why:?}"));
        }
        self.peer.host_mut().iterate(now, wall);
        while self.submissions.room() > 0 {
            let Some(submission) = self.peer.host_mut().submissions().pop() else { break };
            self.submissions.push(submission);
        }
        self.peer_submissions = self.peer.host_mut().submissions().len();
        if let Some(control) = &mut self.control {
            while self.submissions.room() > 0 {
                let Some(mut submission) = control.submissions.pop() else { break };
                control::tag(&mut submission);
                self.submissions.push(submission);
            }
        }
    }
    fn drain(&mut self) {
        if let Some(address) = self.peer.address()
            && self.control.as_ref().is_none_or(|control| control.address.is_some())
            && let Some(prefix) = self.ready.take()
        {
            let mut stdout = io::stdout();
            let result = stdout
                .write_all(&prefix)
                .and_then(|()| {
                    if let Some(control) = &self.control {
                        writeln!(stdout, "{address} {}", control.address.expect("control bound before ready"))
                    } else {
                        writeln!(stdout, "{address}")
                    }
                })
                .and_then(|()| stdout.flush());
            if let Err(why) = result {
                self.fail("stdout", &why);
            }
        }
        self.observations();
        if self.failed {
            self.suppress_sends();
        }
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        &mut self.completions
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        &mut self.submissions
    }
    fn work_pending(&self, now: Time) -> bool {
        self.peer.host().work_pending(now)
            || self.control.as_ref().is_some_and(|control| control.work_pending(now))
            || self.peer_submissions > 0
            || !self.completions.is_empty()
            || !self.submissions.is_empty()
            || (self.signal == Signal::Close && self.cancel == Cancel::Idle)
    }
    fn next_deadline(&self) -> Option<Time> {
        self.peer
            .host()
            .next_deadline()
            .into_iter()
            .chain(self.control.as_ref().and_then(control::Control::next_deadline))
            .min()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        self.peer.host().next_policy_deadline()
    }
    fn is_empty(&self) -> bool {
        self.peer.host().is_empty()
            && self.control.as_ref().is_none_or(control::Control::is_empty)
            && self.signal == Signal::Closed
            && self.cancel == Cancel::Idle
            && self.checkpoint_or_llm()
            && self.submissions.is_empty()
            && self.completions.is_empty()
    }
    fn worst_case(&self) -> u64 {
        self.peer.host().worst_case()
            + self.control.as_ref().map_or(0, |_| control::Control::worst_case())
            + List::<u8>::worst_case(self.buffer.capacity()).expect("bounded observation buffer")
            + List::<u64>::worst_case(temper_fake::ENTRIES).expect("bounded installation snapshot")
            + if self.writer.is_some() { u64::from(limits::OBSERVATION_STATE_BYTES) } else { 0 }
            + self.ready.as_ref().map_or(0, |prefix| u64::try_from(prefix.len()).expect("ready bytes"))
            + self
                .observation_path
                .as_ref()
                .map_or(0, |path| u64::try_from(path.as_os_str().len()).expect("path bytes"))
            + Queue::<Complete>::worst_case(self.completions.capacity()).expect("queue bound")
            + Queue::<Submit>::worst_case(self.submissions.capacity()).expect("queue bound")
    }
    fn operations(&self) -> u32 {
        host_operations(&self.peer, self.control.as_ref())
    }
    fn exit(&self) -> Option<Exit> {
        self.is_empty().then_some(Exit::Code(u8::from(self.failed)))
    }
}

#[cfg(test)]
mod tests;
