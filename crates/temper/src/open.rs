//! The terminal's arguments and startup (client/README.md, 6.1).
use skein_io::kernel::{Exit, Fd};
use skein_lib::{
    Secret,
    base64::{self, Alphabet},
};
use skein_shell::{Clock, Config, Host, Kernel};
use std::{ffi::OsString, net::SocketAddr, os::unix::ffi::OsStrExt, path::Path};
use temper_client_shell_terminal::{
    client::{NativeClient, Settings, Start},
    limits,
    terminal::{Loop, Terminal},
};

#[derive(Debug)]
pub enum Target {
    Link { engine: SocketAddr, link: Secret },
    Address { engine: SocketAddr },
}
#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    Usage,
    NotALink(Box<[u8]>),
    NotLoopback,
    Directory(String),
    NotSignedIn,
    LinkRefused,
}
impl Refused {
    #[must_use]
    pub fn line(&self, directory: &Path) -> String {
        let why = match self {
            Self::Usage => "usage: temper open DIRECTORY LINK_OR_ADDRESS".into(),
            Self::NotALink(argument) => format!("not a link or an address: {}", String::from_utf8_lossy(argument)),
            Self::NotLoopback => "the engine's address must be on loopback".into(),
            Self::Directory(why) => format!("{}: {why}", directory.display()),
            Self::NotSignedIn => "not signed in: give the link temper serve printed".into(),
            Self::LinkRefused => "the link was refused: use the fresh link temper serve printed".into(),
        };
        format!("temper open: {why}\n")
    }
}
pub fn target(argument: &[u8]) -> Result<Target, Refused> {
    let bad = || Refused::NotALink(argument.into());
    let text = std::str::from_utf8(argument).map_err(|_| bad())?;
    let rest = text.strip_prefix("http://").ok_or_else(bad)?;
    let (address, path) = rest.split_once('/').unwrap_or((rest, ""));
    let engine: SocketAddr = address.parse().map_err(|_| bad())?;
    if !path.is_empty() {
        let secret = path.strip_prefix("jig/v1/sign-in/link?secret=").ok_or_else(bad)?;
        let secret = base64::decode(Alphabet::Url, secret.as_bytes(), 32).map_err(|_| bad())?;
        if secret.len() != 32 {
            return Err(bad());
        }
    }
    if !engine.ip().is_loopback() {
        return Err(Refused::NotLoopback);
    }
    Ok(if path.is_empty() {
        Target::Address { engine }
    } else {
        Target::Link { engine, link: Secret::new(argument.into()) }
    })
}
/// Reads only whether this directory holds a bearer token; never lends it.
pub fn check_token(directory: &Path) -> Result<(), Refused> {
    match skein_shell::read_private_file(&directory.join("token"), 40, skein_shell::effective_user()) {
        Ok(token) if token.expose().is_empty() => Err(Refused::NotSignedIn),
        Err(skein_shell::Unread::Absent) => Err(Refused::NotSignedIn),
        Err(why) => Err(Refused::Directory(super::unread(why))),
        Ok(_) => Ok(()),
    }
}
pub(crate) fn start(arguments: &[OsString], clock: &Clock) -> Result<u8, Refused> {
    let [directory, argument] = arguments else { return Err(Refused::Usage) };
    let target = target(argument.as_bytes())?;
    let directory = Path::new(directory);
    let parent = directory.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let name = directory.file_name().ok_or_else(|| Refused::Directory("requires a last component".into()))?;
    let root = skein_shell::open_root(parent)
        .map_err(|number| Refused::Directory(format!("cannot open its parent: errno {number}")))?;
    let mut owned = vec![root];
    let result = (|| {
        let (engine, link, start) = match target {
            Target::Link { engine, link } => (engine, Some(link), Start::Link(argument.as_bytes().into())),
            Target::Address { engine } => {
                check_token(directory)?;
                (engine, None, Start::Token)
            }
        };
        let bounds = limits::native();
        let (view, _) = limits::view(&bounds).expect("native view bounds");
        let client = NativeClient::new(
            Settings { engine: skein_io::Addr::Ip(engine), directory: name.as_bytes().into(), limits: bounds },
            root,
            skein_shell::effective_user(),
            skein_shell::seed().expect("kernel seed"),
            start,
        )
        .map_err(|_| Refused::NotALink(argument.as_bytes().into()))?;
        let state = Loop::new(&limits::terminal(), &view.tree, directory.as_os_str().as_bytes().into(), link);
        let signals = skein_shell::open_termination_signals().expect("termination signals");
        owned.push(signals);
        let mut terminal = Terminal::new(client, state, Fd::new(0), Fd::new(1), signals).expect("terminal descriptors");
        let mut kernel = Kernel::open(Config { operations: terminal.operations() }).expect("terminal kernel");
        owned.clear();
        let exit = skein_shell::drive(&mut kernel, clock, &mut terminal);
        if let Some(line) = terminal.exit_line() {
            eprint!("{}", String::from_utf8_lossy(&line));
        }
        Ok(match exit {
            Exit::Code(code) => code,
            Exit::Signal(_) => 1,
        })
    })();
    super::close(owned);
    result
}
#[must_use]
pub fn run(arguments: &[OsString], clock: &Clock) -> u8 {
    match start(arguments, clock) {
        Ok(code) => code,
        Err(why) => {
            let directory = arguments.first().map_or(Path::new(""), Path::new);
            eprint!("{}", why.line(directory));
            1
        }
    }
}
