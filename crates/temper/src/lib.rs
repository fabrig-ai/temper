//! Startup for temper's standalone binary (protocol/configuration.md, section 2).
// tests::configuration includes the shared integration fixture, which names temper::directories.
extern crate self as temper;
use jig_protocol_config::{self as config, read};
use skein_io::kernel::{Error, Exit, Fd};
use skein_lib::{List, Secret};
use skein_shell::{Clock, Config, Kernel};
use std::{
    ffi::OsString,
    fmt::Write,
    net::ToSocketAddrs,
    os::unix::{ffi::OsStrExt, fs::FileTypeExt},
    path::{Path, PathBuf},
};
use temper_service::{
    self as service,
    settings::{Files, Seeds},
};

pub mod directories;
use directories::{Directories, private_root};
mod accounts_runtime;
mod agent;
pub mod box_command;
pub mod bundle;
pub mod ctl;
pub mod fake;
pub mod forge;
mod forge_sign_in;
mod google_runtime;
mod host;
mod init;
pub mod open;
mod program_login;
mod setup;
mod worker_alone;
pub use host::Host;
pub const USAGE: &str = "usage: temper init [DIRECTORIES] | temper serve [DIRECTORIES] [--box] | temper agent AGENT.json | temper program-login [DIRECTORIES] NAME | temper open DIRECTORY LINK_OR_ADDRESS | temper forge fetch [DIRECTORIES] FORGE | temper forge prepare [DIRECTORIES] FORGE | temper forge run [DIRECTORIES] FORGE forgejo|runner | temper forge reconcile [DIRECTORIES] FORGE | temper forge sign-in [DIRECTORIES] FORGE | temper box [DIRECTORIES] | temper fake llm --document FILE [--ready PREFIX] [--seed N] | temper fake forge [DIRECTORIES] N [--ready PREFIX] [--seed N] [--observations FILE] | temper fake forge --document FILE [--ready PREFIX] [--seed N] [--observations FILE] | temper fake forge check [DIRECTORIES] N OBSERVATIONS STORE | temper fake git ARGS... | temper ctl DIRECTORY TOOL [ARGUMENT...] | temper ctl help [TOOL]";
const CONFIG_BYTES: u32 = 1 << 20;

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Init { directories: Directories },
    Serve { directories: Directories, boxed: bool },
    Box { directories: Directories },
    ProgramLogin { directories: Directories, name: OsString },
    Agent { configuration: PathBuf },
    Open { arguments: Vec<OsString> },
    Ctl { arguments: Vec<OsString> },
    Fake { arguments: fake::Arguments },
    FakeCheck { directories: Directories, check: fake::Check },
    FakeGit { arguments: Box<[Box<[u8]>]> },
    Forge { directories: Directories, arguments: Vec<OsString>, ssl_cert_file: Option<PathBuf> },
}
#[must_use]
pub fn usage() -> String {
    let mut help = format!("{USAGE}\nDIRECTORIES: [--config DIR] [--data DIR] [--cache DIR]\n").into_bytes();
    skein_shell::help(&directories::SETTINGS, &mut help);
    help.extend_from_slice(b"\ntemper fake llm --document FILE [--ready PREFIX] [--seed N]\n");
    skein_shell::help(&fake::LLM_SETTINGS, &mut help);
    help.extend_from_slice(b"\ntemper fake forge [DIRECTORIES] N [--ready PREFIX] [--seed N] [--observations FILE]\n");
    help.extend_from_slice(b"\ntemper fake forge --document FILE [--ready PREFIX] [--seed N] [--observations FILE]\n");
    skein_shell::help(&fake::FORGE_SETTINGS, &mut help);
    help.extend_from_slice(b"\ntemper fake forge check [DIRECTORIES] N OBSERVATIONS STORE\n");
    help.extend_from_slice(b"\ntemper fake git ARGS...\n");
    String::from_utf8(help).expect("help is UTF-8")
}
fn prefixed(command: &str, why: &str) -> String {
    let mut lines = String::new();
    for line in why.lines() {
        writeln!(lines, "{command}: {line}").expect("writing into a string");
    }
    lines
}
pub(crate) fn problems(found: &service::startup::Problems) -> String {
    prefixed("temper serve", &String::from_utf8_lossy(&service::startup::lines(found)))
}
pub fn configure(args: impl Iterator<Item = OsString>) -> Result<Command, String> {
    let arguments: Vec<Box<[u8]>> = args.map(|arg| Box::from(arg.as_bytes())).collect();
    let working = std::env::current_dir().map_err(|why| why.to_string())?;
    configure_with(&arguments, &skein_shell::environment(), working.as_os_str().as_bytes())
}
type EnvironmentEntry = (Box<[u8]>, Box<[u8]>);
pub fn configure_with(
    arguments: &[Box<[u8]>],
    environment: &[EnvironmentEntry],
    working: &[u8],
) -> Result<Command, String> {
    let invalid = || format!("temper: invalid arguments\n{}", usage());
    let Some((command, arguments)) = arguments.split_first() else { return Err(invalid()) };
    let os =
        |args: &[Box<[u8]>]| args.iter().map(|arg| std::ffi::OsStr::from_bytes(arg).to_owned()).collect::<Vec<_>>();
    match command.as_ref() {
        b"fake" => return fake::configure(arguments, environment, working),
        b"ctl" => return Ok(Command::Ctl { arguments: os(arguments) }),
        b"open" => return Ok(Command::Open { arguments: os(arguments) }),
        b"agent" => {
            return match arguments {
                [configuration] => Ok(Command::Agent { configuration: path(configuration).into() }),
                _ => Err(invalid()),
            };
        }
        b"init" | b"serve" | b"box" | b"program-login" | b"forge" => {}
        _ => return Err(invalid()),
    }
    let (verb, arguments) = if command.as_ref() == b"forge" {
        let Some((verb, arguments)) = arguments.split_first() else { return Err(invalid()) };
        if ![b"fetch".as_slice(), b"prepare", b"run", b"reconcile", b"sign-in"].contains(&verb.as_ref()) {
            return Err(invalid());
        }
        (Some(verb), arguments)
    } else {
        (None, arguments)
    };
    let (directories, rest) =
        directories::resolve(arguments, environment, working).map_err(|why| format!("temper: {why}\n{}", usage()))?;
    let rest = os(&rest);
    match command.as_ref() {
        b"init" if rest.is_empty() => Ok(Command::Init { directories }),
        b"serve" if rest.is_empty() || rest == ["--box"] => Ok(Command::Serve { directories, boxed: !rest.is_empty() }),
        b"box" if rest.is_empty() => Ok(Command::Box { directories }),
        b"program-login" if rest.len() == 1 => Ok(Command::ProgramLogin { directories, name: rest[0].clone() }),
        b"forge" => {
            let verb = verb.expect("forge command word");
            let valid = if verb.as_ref() == b"run" {
                rest.len() == 2 && (rest[1] == "forgejo" || rest[1] == "runner")
            } else {
                rest.len() == 1
            };
            if !valid || rest[0].to_str().and_then(|n| n.parse::<u16>().ok()).is_none() {
                return Err(invalid());
            }
            let mut arguments = os(std::slice::from_ref(verb));
            arguments.extend(rest);
            let ssl_cert_file = environment
                .iter()
                .find(|(name, _)| name.as_ref() == b"SSL_CERT_FILE")
                .filter(|(_, value)| !value.is_empty())
                .map(|(_, value)| path(value).to_path_buf());
            Ok(Command::Forge { directories, arguments, ssl_cert_file })
        }
        _ => Err(invalid()),
    }
}
pub fn run(command: Command) -> Result<u8, String> {
    let exit = match command {
        Command::Init { directories } => return init::run(&directories).map_err(|why| prefixed("temper init", &why)),
        Command::Serve { directories, boxed } => {
            serve(&directories, boxed).map_err(|why| prefixed("temper serve", &why))?
        }
        Command::Box { directories } => box_command::run(&directories).map_err(|why| prefixed("temper box", &why))?,
        Command::Agent { configuration } => smith_agent_shell::run(&configuration, Box::new(std::io::stderr()))
            .map_err(|why| prefixed("temper agent", &why))?,
        Command::ProgramLogin { directories, name } => {
            return program_login::run(&directories, &name)
                .map_err(|why| prefixed("temper program-login", &why.lines().collect::<Vec<_>>().join("; ")));
        }
        Command::Fake { arguments } => return fake::run(&arguments),
        Command::FakeCheck { directories, check } => return Ok(fake::check::run(&directories, &check)),
        Command::FakeGit { arguments } => return Ok(fake::git::run(&arguments)),
        Command::Forge { directories, arguments, ssl_cert_file } => {
            return Ok(forge::run(&directories, &arguments, ssl_cert_file));
        }
        Command::Ctl { arguments } => return Ok(ctl::run(&arguments, &Clock::new())),
        Command::Open { arguments } => return Ok(open::run(&arguments, &Clock::new())),
    };
    Ok(match exit {
        Exit::Code(code) => code,
        Exit::Signal(_) => 1,
    })
}
fn path(bytes: &[u8]) -> &Path {
    Path::new(std::ffi::OsStr::from_bytes(bytes))
}
fn member(document: &skein_json::Document, at: u32, key: &[u8]) -> u32 {
    read::Object::new(document, at).expect("named checked object").member(document, key).expect("named checked member")
}
fn errno(error: Error) -> i32 {
    match error {
        Error::Refused => 111,
        Error::Busy => 11,
        Error::Reset => 104,
        Error::BrokenPipe => 32,
        Error::NotConnected => 107,
        Error::AddressInUse => 98,
        Error::AddressNotAvailable => 99,
        Error::Unreachable => 101,
        Error::TimedOut => 110,
        Error::TooManyOpenFiles => 24,
        Error::NoBufferSpace => 12,
        Error::Cancelled => 125,
        Error::TooLate | Error::NotFound => 2,
        Error::Exists => 17,
        Error::NotADirectory => 20,
        Error::IsADirectory => 21,
        Error::NotEmpty => 39,
        Error::Permission => 13,
        Error::NoSpace => 28,
        Error::ReadOnly => 30,
        Error::TooManyLinks => 40,
        Error::NameTooLong => 36,
        Error::Escape => 18,
        Error::NotAFile => 6,
        Error::InvalidArgument => 22,
        Error::Other(number) => number,
    }
}
fn unread(why: skein_shell::Unread) -> String {
    match why {
        skein_shell::Unread::Absent => "absent".into(),
        skein_shell::Unread::NotAFile => "not a file".into(),
        skein_shell::Unread::TooLarge { max } => format!("past {max} bytes"),
        skein_shell::Unread::Unsafe(_) => "readable by others or reached through a link".into(),
        skein_shell::Unread::Kernel(error) => format!("errno {}", errno(error)),
    }
}
fn file_path(at: config::Path) -> String {
    let mut found = service::startup::Problems::empty();
    found.push(at, service::startup::What::Jig(config::What::Unreadable));
    let bytes = service::startup::lines(&found);
    String::from_utf8_lossy(&bytes).trim_end().strip_suffix(": not read").expect("path and reason").into()
}
fn read_files(names: Box<[config::File]>, user: u32) -> Result<Files, String> {
    read_files_with_asked(names, Box::new([]), user)
}
fn read_files_with_asked(names: Box<[config::File]>, asked: Box<[config::File]>, user: u32) -> Result<Files, String> {
    let count = names.len().checked_add(asked.len()).expect("bounded files");
    let mut entries = List::with_capacity(u32::try_from(count).expect("bounded files"));
    let mut refused = String::new();
    for (file, optional) in
        names.into_iter().map(|file| (file, false)).chain(asked.into_iter().map(|file| (file, true)))
    {
        let resolved = path(&file.path);
        let value = if file.private {
            skein_shell::read_private_file(resolved, file.max, user)
        } else {
            skein_shell::read_file(resolved, file.max).map(Secret::new)
        };
        match value {
            Ok(value) => entries.push((file.path, value)).expect("named capacity"),
            Err(skein_shell::Unread::Absent) if optional => {}
            Err(why) => {
                writeln!(refused, "{}: {}: {}", file_path(file.at), path(&file.path).display(), unread(why))
                    .expect("writing into a string");
            }
        }
    }
    if refused.is_empty() { Ok(Files { entries }) } else { Err(refused) }
}
fn address(configured: &config::Address) -> Result<std::net::SocketAddr, String> {
    match configured {
        config::Address::Addr(address) => address.socket().ok_or_else(|| "requires a socket address".into()),
        config::Address::Host { name, port } => (std::str::from_utf8(name).map_err(|why| why.to_string())?, *port)
            .to_socket_addrs()
            .map_err(|why| why.to_string())?
            .next()
            .ok_or_else(|| "endpoint has no address".into()),
    }
}
fn trust(file: Option<&[u8]>) -> Result<(skein_tls::Config, u32, u32), String> {
    let source = file
        .map_or(skein_shell::Trust::Machine { ssl_cert_file: None }, |file| skein_shell::Trust::Der(path(file).into()));
    let roots = skein_shell::read_trust_roots(
        &source,
        &skein_shell::RootBounds { bundle_bytes: CONFIG_BYTES, certificates: 1024, certificate_bytes: 65_536 },
    )
    .map_err(|why| match why {
        skein_shell::RootsRefusal::NoBundle => "no trust bundle".into(),
        skein_shell::RootsRefusal::Unread { path, why } => format!("{}: {}", path.display(), unread(why)),
        skein_shell::RootsRefusal::TooMany { bound } => format!("past {bound} trust roots"),
        skein_shell::RootsRefusal::TooLong { index, bound } => format!("trust root {index}: past {bound} bytes"),
    })?;
    let count = u32::try_from(roots.certificates.len()).expect("bounded roots");
    let each =
        roots.certificates.iter().map(|root| u32::try_from(root.len()).expect("bounded root")).max().unwrap_or(0);
    let (trust, _) = skein_tls::Config::from_der(&roots.certificates, &[]).map_err(|why| match why {
        skein_tls::Refusal::Roots => "no supported trust roots",
        skein_tls::Refusal::Alpn => "protocol names refused",
        skein_tls::Refusal::Provider => "TLS provider refused",
    })?;
    Ok((trust, count, each))
}
fn server_name(bytes: &[u8]) -> Result<skein_tls::Name, String> {
    skein_tls::Name::new(std::str::from_utf8(bytes).map_err(|why| why.to_string())?)
        .ok_or_else(|| "invalid server name".into())
}
fn accounts(engine: &config::Engine) -> Result<service::AccountsRuntime, String> {
    accounts_runtime::resolve(engine)
}

fn forges(
    configured: &[service::startup::Forge],
    resolve: fn(&config::Address) -> Result<std::net::SocketAddr, String>,
) -> Result<List<service::ForgeRuntime>, String> {
    use skein_forge_connection::settings::Transport;
    let mut runtime = List::with_capacity(u32::try_from(configured.len()).expect("bounded forges"));
    for (index, forge) in configured.iter().enumerate() {
        let prefix = format!("application.forges[{index}]");
        let address = match resolve(&forge.api) {
            Ok(address) => address,
            Err(_) if matches!(forge.api, config::Address::Host { .. }) => service::forge::HELD_ADDRESS,
            Err(why) => return Err(format!("{prefix}.api: {why}")),
        };
        let (transport, trust_roots, trust_root_bytes) = match &forge.transport {
            service::startup::Transport::Plaintext => (Transport::Plaintext { name: None }, 0, 0),
            service::startup::Transport::Tls { server_name: name, trust: file } => {
                let (trust, count, each) =
                    trust(file.as_deref()).map_err(|why| format!("{prefix}.transport.tls.trust: {why}"))?;
                (Transport::Tls { server_name: server_name(name)?, trust }, count, each)
            }
        };
        runtime
            .push(service::ForgeRuntime { number: forge.number, address, transport, trust_roots, trust_root_bytes })
            .expect("forge count");
    }
    Ok(runtime)
}
fn prepare(directories: &Directories, bytes: &[u8], owned: &mut Vec<Fd>) -> Result<service::Service, String> {
    prepare_with_store(directories, bytes, owned).map(|(service, _)| service)
}
fn prepare_with_store(
    directories: &Directories,
    bytes: &[u8],
    owned: &mut Vec<Fd>,
) -> Result<(service::Service, Box<[u8]>), String> {
    prepare_with_limits(directories, bytes, owned, false).map(|(service, store, _)| (service, store))
}
#[expect(clippy::too_many_lines, reason = "startup opens checked resources in dependency order")]
fn prepare_with_limits(
    directories: &Directories,
    bytes: &[u8],
    owned: &mut Vec<Fd>,
    under_box: bool,
) -> Result<(service::Service, Box<[u8]>, u32), String> {
    if service::startup::shape(bytes, &directories.jig())
        .map_err(|why| String::from_utf8_lossy(&service::startup::lines(&why)).into_owned())?
        == service::startup::Shape::WorkerAlone
    {
        return worker_alone::prepare(directories, bytes, owned).map(|service| (service, Box::new([]) as Box<[u8]>, 0));
    }
    let setup::Deployment { named, checked, limits, .. } = setup::check(bytes, &directories.jig())?;
    let document =
        config::document(bytes, &config::DocumentLimits { bytes: CONFIG_BYTES, tokens: CONFIG_BYTES, depth: 32 })
            .expect("named checked document");
    let engine =
        config::engine(&document, member(&document, 0, b"engine"), &directories.jig()).expect("named checked engine");
    let forges = forges(&named.forges, address)?;
    let agent = named.worker.as_ref().map(|_| agent::preflight(&named)).transpose()?;
    let user = skein_shell::effective_user();
    let mut named_files = named.files.into_vec();
    named_files.extend(service::startup::setup_files(&checked));
    let mut files = read_files_with_asked(named_files.into_boxed_slice(), named.asked_files, user)?;
    let reads = service::startup::handover_reads(bytes, &directories.jig(), &checked)
        .map_err(|why| String::from_utf8_lossy(&service::startup::lines(&why)).into_owned())?;
    let mut entries = List::with_capacity(
        files.entries.len().checked_add(u32::try_from(reads.len()).expect("checked handovers")).expect("checked files"),
    );
    for entry in files.entries.into_boxed() {
        entries.push(entry).expect("checked files");
    }
    for read in reads {
        match host::read_handover(&read) {
            Ok(Some(value)) => entries.push((read.path, value)).expect("checked handovers"),
            Ok(None) => {}
            Err(why) => return Err(format!("{}: the handover is unreadable: {why:?}", path(&read.path).display())),
        }
    }
    files.entries = entries;
    let google = google_runtime::resolve(named.sign_in.as_ref())?;
    let mut bundle = match named.bundle.as_deref() {
        Some(configured) => Some(bundle::read(configured, named.bundle_limits)?),
        None => bundle::own(named.bundle_limits)?,
    };
    if let Some(bundle) = &mut bundle {
        bundle.fill_sign_in(named.sign_in.is_some()).and_then(|()| {
            bundle.fill_forge_sign_in(named.sign_in.as_ref(), &named.forges, &checked.setup, named.bundle_limits.bundle_bytes)
        }).ok_or_else(|| {
            format!(
                "application.clients.bundle: {}: page needs </head> and room within clients.bundle_bytes for sign-in metadata",
                named.bundle.as_deref().map_or("embedded".into(), |name| String::from_utf8_lossy(name))
            )
        })?;
    }
    let runtime = if let Some(worker) = &named.worker {
        let root_path = path(&worker.root);
        if std::fs::symlink_metadata(root_path).is_ok() {
            private_root(root_path, user).map_err(|why| format!("application.workers[0].root: {why}"))?;
        }
        let root = skein_shell::make_root(root_path, 0o700)
            .map_err(|errno| format!("application.workers[0].root: errno {errno}"))?;
        owned.push(root);
        private_root(root_path, user).map_err(|why| format!("application.workers[0].root: {why}"))?;
        Some(service::WorkerRuntime { root, path: root_path.as_os_str().as_bytes().into() })
    } else {
        None
    };
    let accounts = accounts(&engine)?;
    let seed = skein_shell::seed().map_err(|errno| format!("random seed: errno {errno}"))?;
    let crypto = skein_shell::crypto_seed().map_err(|errno| format!("cryptographic seed: errno {errno}"))?;
    let drawn = skein_shell::crypto_seed().map_err(|errno| format!("deployment id: errno {errno}"))?;
    let mut deployment = [0_u8; 16];
    deployment.copy_from_slice(drawn.expose().get(..16).expect("32-byte seed"));
    // Drawn at every start; the service keeps it at the first and restores
    // the kept one at every later start (jig's configuration.md 8).
    let drawn = skein_shell::crypto_seed().map_err(|errno| format!("installation id: errno {errno}"))?;
    let mut installation = [0_u8; 16];
    installation.copy_from_slice(drawn.expose().get(..16).expect("32-byte seed"));
    let mut service = service::startup::with_setup(
        bytes,
        &directories.jig(),
        &files,
        agent.as_ref(),
        &Seeds { deployment, installation, seed, crypto },
        Some(accounts),
        runtime,
        forges,
        Some(&checked),
        under_box,
    )
    .map_err(|why| String::from_utf8_lossy(&service::startup::lines(&why)).into_owned())?;
    service
        .configure_google_endpoints(google)
        .map_err(|_| "application.sign_in: TLS roots exceed engine.memory_bytes")?;
    if let Some(agent) = &agent
        && let Some(program) = agent.programs.first()
    {
        // Source commands use the agent's explicit command environment.
        let environment =
            smith_agent_shell::config::read(path(&named.worker.as_ref().expect("local agent checked").agent))?
                .service
                .environment
                .command_entries();
        let resources = service::program_sources::Resources::configured(program, environment)
            .ok_or("workers[0].agent.configuration: program source resources exceed their bounds")?;
        service.configure_program_sources(resources).map_err(|_| "worker refused program source resources")?;
    }
    let root = skein_shell::make_root(path(&named.deployment), 0o700)
        .map_err(|errno| format!("engine.deployment.directory: errno {errno}"))?;
    owned.push(root);
    service.adopt_store(root, user).map_err(|_| "store refused its deployment root")?;
    if let Some(bundle) = bundle {
        service.adopt_bundle(bundle);
    }
    Ok((service, named.deployment, limits.line_bytes))
}
fn close(owned: Vec<Fd>) {
    for fd in owned {
        skein_shell::close_keeper_fd(fd);
    }
}
/// Derive the standalone admission envelope without retaining its descriptors.
pub fn envelope_in(directories: &Directories, config: &[u8]) -> Result<(u64, u32), String> {
    directories.open(skein_shell::effective_user())?;
    let mut owned = Vec::new();
    let result = prepare(directories, config, &mut owned).map(|service| (service.worst_case(), service.operations()));
    close(owned);
    result.map_err(|why| prefixed("temper serve", &why))
}
pub fn prepare_in(directories: &Directories) -> Result<service::Service, String> {
    let bytes = directories.read().map_err(|why| prefixed("temper serve", &why))?;
    let mut owned = Vec::new();
    let result = prepare(directories, &bytes, &mut owned);
    if result.is_err() {
        close(owned);
    }
    result.map_err(|why| prefixed("temper serve", &why))
}
fn listeners_refusal(why: skein_shell::ListenersRefusal) -> String {
    use skein_shell::ListenersRefusal as Why;
    match why {
        Why::BadCount => "LISTEN_FDS: expected a decimal listener count".into(),
        Why::TooMany { bound } => format!("LISTEN_FDS: listener count exceeds {bound}"),
        Why::NotOpen { fd } => format!("LISTEN_FDS: fd {} is not open", fd.raw()),
        Why::NotSocket { fd, .. } => format!("LISTEN_FDS: fd {} is not a socket", fd.raw()),
        Why::NotStream { fd, .. } => format!("LISTEN_FDS: fd {} is not a stream socket", fd.raw()),
        Why::NotListening { fd } => format!("LISTEN_FDS: fd {} is not listening", fd.raw()),
        Why::NotIp { fd } => format!("LISTEN_FDS: fd {} is not an IP listener", fd.raw()),
        Why::Kernel { fd, errno } => format!("LISTEN_FDS: fd {}: errno {errno}", fd.raw()),
    }
}
fn serve(directories: &Directories, boxed: bool) -> Result<Exit, String> {
    let listeners = skein_shell::inherited_listeners().map_err(listeners_refusal)?;
    let requests = 3 + i32::try_from(listeners.len()).expect("bounded inherited listeners");
    let answers = requests + 1;
    if boxed {
        for fd in [requests, answers] {
            if !std::fs::metadata(format!("/proc/self/fd/{fd}")).is_ok_and(|m| m.file_type().is_fifo()) {
                return Err(format!("--box: fd {fd} is not an open pipe"));
            }
        }
    }
    let bytes = directories.read()?;
    let mut owned = listeners.iter().map(|(fd, _)| *fd).collect::<Vec<_>>();
    let prepared = prepare_with_limits(directories, &bytes, &mut owned, boxed);
    let (mut service, store, line_bytes) = match prepared {
        Ok(prepared) => prepared,
        Err(why) => {
            close(owned);
            return Err(why);
        }
    };
    for (fd, addr) in listeners {
        if let Err(fd) = service.adopt_listener(fd, addr) {
            close(owned);
            return Err(format!("listener: fd {} at {addr} refused", fd.raw()));
        }
    }
    if boxed {
        owned.extend([Fd::new(requests), Fd::new(answers)]);
        if let Err(fd) = service.adopt_box(Fd::new(requests), Fd::new(answers), line_bytes) {
            close(owned);
            return Err(format!("--box: fd {} refused", fd.raw()));
        }
    }
    let opened = skein_shell::open_termination_signals()
        .map_err(|errno| format!("signals: errno {errno}"))
        .and_then(|signals| {
            owned.push(signals);
            service.adopt_signals(signals).map_err(|_| "signals refused".into())
        })
        .and_then(|()| Kernel::open(Config { operations: service.operations() }).map_err(|why| why.to_string()));
    let mut kernel = match opened {
        Ok(kernel) => kernel,
        Err(why) => {
            close(owned);
            return Err(why);
        }
    };
    let mut host = Host::new(service, directories.clone());
    host.store = store;
    Ok(skein_shell::drive(&mut kernel, &Clock::new(), &mut host))
}
/// The effective user's passwd login; forge commands preserve lookup refusals.
pub(crate) fn login(user: u32) -> Result<Box<[u8]>, String> {
    let passwd = skein_shell::read_file(std::path::Path::new("/etc/passwd"), 1 << 20)
        .map_err(|why| format!("user: {}", unread(why)))?;
    for line in passwd.split(|c| *c == b'\n') {
        let fields: Vec<_> = line.split(|c| *c == b':').collect();
        if fields.get(2).and_then(|id| std::str::from_utf8(id).ok()).and_then(|id| id.parse::<u32>().ok()) == Some(user)
        {
            return Ok(fields[0].into());
        }
    }
    Err(format!("user {user}: no login in /etc/passwd"))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod box_tests {

    #[test]
    fn serve_box_flag_is_carried_to_the_command() {
        for boxed in [false, true] {
            let mut args = ["serve", "--config", "/c", "--data", "/d", "--cache", "/k"]
                .iter()
                .map(|word| Box::from(word.as_bytes()))
                .collect::<Vec<_>>();
            if boxed {
                args.push(Box::from(b"--box".as_slice()));
            }
            let super::Command::Serve { boxed: found, .. } =
                super::configure_with(&args, &[], b"/").expect("serve arguments")
            else {
                panic!("serve command");
            };
            assert_eq!(found, boxed);
        }
    }

    #[test]
    fn serve_refuses_unknown_or_repeated_flags() {
        for flags in [&["--other"][..], &["--box", "--box"][..]] {
            let args = core::iter::once("serve")
                .chain(flags.iter().copied())
                .chain(["--config", "/c", "--data", "/d", "--cache", "/k"])
                .map(|word| Box::from(word.as_bytes()))
                .collect::<Vec<_>>();
            assert!(super::configure_with(&args, &[], b"/").is_err());
        }
    }
}
