//! Temper's members, read strictly with jig's readers (protocol/configuration.md, 3, 4, 10).
mod demo;
mod landing;
mod public;
mod read;
pub mod sign_in;
use super::{Problems, What};
use crate::{limits, settings};
use alloc::boxed::Box;
use jig_protocol_config::{self as config, Path};
pub use public::Origin;
#[cfg(test)]
pub use public::Scheme;
use skein_json::{Document, document::Kind};
use skein_lib::{List, bytes};

#[derive(Debug)]
pub struct Members {
    pub clients: Clients,
    pub sign_in: Option<sign_in::SignIn>,
    pub webhooks: Option<Webhooks>,
    pub forges: Box<[Forge]>,
    pub demo: Option<settings::Demo>,
    pub asked_owned: Box<[u16]>,
    pub identities: Box<[Identity]>,
    pub landing: temper_engine_domain::LandingPolicy,
    /// The landing's projects in the document's order, which names a refused one's path.
    pub landing_projects: Box<[u32]>,
    pub charter: u32,
    pub limits: limits::Limits,
    pub overrides: Box<[(Box<[u8]>, u64)]>,
    pub workers: Box<[Worker]>,
}
/// The application's clients listener (protocol/configuration.md, section 3).
#[derive(Debug)]
pub struct Clients {
    /// The socket address used for the listener, Host guard and printed links.
    pub listener: config::Address,
    /// `application.clients.bundle`: the bundle's directory; `None` serves no browser.
    pub bundle: Option<Box<[u8]>>,
    /// The browser origin and jig public name (protocol/sign-in.md, 2.2).
    pub public: Option<Origin>,
}
/// The optional webhook listener (protocol/configuration.md, section 3).
#[derive(Debug)]
pub struct Webhooks {
    pub listener: config::Address,
}
/// Read a socket listener, and the clients' bundle and public origin when asked.
#[must_use]
pub fn clients(document: &Document, at: u32, path: &Path, bundle: bool, problems: &mut Problems) -> Option<Clients> {
    let mut object = read::object(document, at, path, problems)?;
    let at = read::required(&mut object, document, b"listener", path, problems);
    let mut directory = None;
    if bundle
        && let Some(at) = read::required(&mut object, document, b"bundle", path, problems)
        && document.token(at)?.kind != Kind::Null
    {
        match config::read::text(document, at, 1_048_576) {
            Ok(value) => {
                if !value.starts_with(b"/") {
                    problems.push(path.member(b"bundle"), What::Absolute);
                }
                directory = Some(value);
            }
            Err(what) => problems.push(path.member(b"bundle"), What::Jig(what)),
        }
    }
    let mut public = None;
    if bundle
        && let Some(at) = read::nullable(&mut object, document, b"public", path, problems)
        && document.token(at)?.kind != Kind::Null
    {
        match config::read::text(document, at, 267) {
            Ok(value) => match Origin::parse(&value) {
                Some(origin) => public = Some(origin),
                None => problems.push(path.member(b"public"), What::Jig(config::What::Form)),
            },
            Err(what) => problems.push(path.member(b"public"), What::Jig(what)),
        }
    }
    read::finish(object, document, path, problems);
    let path = path.member(b"listener");
    let mut found = config::Problems::empty();
    let listener = config::address::read(document, at?, &path, &mut found);
    problems.append(found, &Path::root());
    let listener = listener?;
    let socket = match listener {
        config::Address::Addr(skein_io::Addr::Ip(_)) => true,
        config::Address::Addr(skein_io::Addr::InProcess(_)) | config::Address::Host { .. } => false,
    };
    read::form(socket, path, problems);
    Some(Clients { listener, bundle: directory, public })
}
#[derive(Debug)]
pub struct Forge {
    pub records_only: bool,
    pub number: u16,
    pub kind: settings::Kind,
    pub api: config::Address,
    pub transport: Transport,
    pub git: Box<[u8]>,
    pub git_program: Option<Box<[u8]>>,
    pub web: Option<Box<[u8]>>,
    pub app_read_only: bool,
    pub api_token: Box<[u8]>,
    pub webhook_secret: Option<Box<[u8]>>,
    pub user: User,
    pub identity: u32,
}
/// The forge's browser origin (protocol/sign-in.md, 9.1 and 9.7).
pub(crate) fn forge_origin(git: &[u8], public: Option<&temper_forge_setup::setup::Origin>) -> Box<[u8]> {
    let Some(public) = public else { return bytes::copy_of(git) };
    let port = skein_lib::Decimal::of(u64::from(public.port));
    let mut parts = List::with_capacity(4);
    parts.push(b"https://".as_slice()).expect("four origin pieces");
    parts.push(public.host.as_ref()).expect("four origin pieces");
    if public.port != 443 {
        parts.push(b":".as_slice()).expect("four origin pieces");
        parts.push(port.as_bytes()).expect("four origin pieces");
    }
    let mut length = 0_usize;
    for part in &parts {
        length = length.checked_add(part.len()).expect("bounded origin");
    }
    let mut out = skein_lib::Writer::new(length);
    for part in &parts {
        out.put(part).expect("measured origin");
    }
    out.finish()
}
/// How a forge's account id is supplied.
#[derive(Debug, PartialEq, Eq)]
pub enum User {
    /// An explicit positive account id.
    Id(u64),
    /// A private file holding the decimal account id.
    File(Box<[u8]>),
}
/// Twenty decimal digits and one final line feed.
pub const USER_BYTES: u32 = 21;
#[derive(Debug)]
pub enum Transport {
    Plaintext,
    Tls { server_name: Box<[u8]>, trust: Option<Box<[u8]>> },
}
#[derive(Debug)]
pub struct Identity {
    pub number: u32,
    pub forge: Option<u16>,
    pub token: Box<[u8]>,
    pub token_configured: bool,
    pub login: Box<[u8]>,
    pub author: settings::Author,
}
#[derive(Debug)]
pub struct Worker {
    pub root: Box<[u8]>,
    pub git: Option<Box<[u8]>>,
}
fn configured_limits(
    document: &Document,
    at: u32,
    path: &Path,
    problems: &mut Problems,
    handed: &mut List<(Box<[u8]>, u64)>,
) -> Option<limits::Limits> {
    let mut object = read::object(document, at, path, problems)?;
    let profile = read::text(&mut object, document, b"profile", path, problems);
    let overrides = read::required(&mut object, document, b"overrides", path, problems);
    read::finish(object, document, path, problems);
    if let Some(profile) = profile
        && profile.as_ref() != b"small"
    {
        problems.push(path.member(b"profile"), What::Profile);
    }
    let mut limits = limits::profile(limits::Profile::Small);
    let path = path.member(b"overrides");
    if let Some(at) = overrides
        && let Some(mut object) = read::object(document, at, &path, problems)
    {
        let end = config::read::after(document, at).ok()?;
        for index in at..end {
            let Some(token) = document.token(index) else { continue };
            if token.kind != Kind::Key {
                continue;
            }
            let name = document.text(token).expect("object key text");
            let Some(value_at) = object.member(document, name) else { continue };
            match config::read::integer(document, value_at, u64::MAX) {
                Ok(value) => {
                    if limits::handed_on(name) {
                        handed.push((bytes::copy_of(name), value)).expect("bounded overrides");
                    }
                    if (!limits::handed_on(name) || limits::host_setup(name))
                        && let Err(what) = limits::apply(&mut limits, name, value)
                    {
                        problems.push(path.member(name), What::Override(what));
                    }
                }
                Err(config::What::Bound) => problems.push(path.member(name), What::Override(limits::Refused::Range)),
                Err(what) => problems.push(path.member(name), What::Jig(what)),
            }
        }
        read::finish(object, document, &path, problems);
    }
    // Pass the resolved client capacity once, including the profile default.
    handed
        .push((bytes::copy_of(b"forge.client.repositories"), u64::from(limits.forge.client.repositories)))
        .expect("bounded setup inputs");
    Some(limits)
}
fn transport(document: &Document, at: u32, path: &Path, problems: &mut Problems) -> Option<Transport> {
    if document.token(at)?.kind == Kind::String {
        let value = match config::read::text(document, at, 16) {
            Ok(value) => value,
            Err(what) => {
                problems.push(path.clone(), What::Jig(what));
                return None;
            }
        };
        read::form(value.as_ref() == b"plaintext", path.clone(), problems);
        return Some(Transport::Plaintext);
    }
    let mut outer = read::object(document, at, path, problems)?;
    let tls = read::required(&mut outer, document, b"tls", path, problems);
    read::finish(outer, document, path, problems);
    let path = path.member(b"tls");
    let mut object = read::object(document, tls?, &path, problems)?;
    let server_name = read::text(&mut object, document, b"server_name", &path, problems);
    let at = read::required(&mut object, document, b"trust", &path, problems);
    let mut trust = None;
    if let Some(at) = at
        && document.token(at)?.kind != Kind::Null
    {
        match config::read::text(document, at, 1_048_576) {
            Ok(value) => trust = Some(value),
            Err(what) => problems.push(path.member(b"trust"), What::Jig(what)),
        }
    }
    read::finish(object, document, &path, problems);
    Some(Transport::Tls { server_name: server_name?, trust })
}
fn user(document: &Document, at: u32, path: &Path, problems: &mut Problems) -> Option<User> {
    if document.token(at)?.kind != Kind::ObjectStart {
        return match config::read::integer(document, at, u64::MAX) {
            Ok(value) => {
                read::form(value > 0, path.clone(), problems);
                Some(User::Id(value))
            }
            Err(what) => {
                problems.push(path.clone(), What::Jig(what));
                None
            }
        };
    }
    let mut object = read::object(document, at, path, problems)?;
    let file = read::text(&mut object, document, b"file", path, problems);
    read::finish(object, document, path, problems);
    if let Some(file) = &file {
        read::form(!file.is_empty(), path.member(b"file"), problems);
        if file.is_empty() {
            return None;
        }
    }
    Some(User::File(file?))
}
#[expect(clippy::disallowed_methods, reason = "the configuration protocol parses URL hosts and ports")]
fn git_base(input: &[u8]) -> bool {
    let (rest, plain) = if let Some(rest) = input.strip_prefix(b"https://") {
        (rest, false)
    } else if let Some(rest) = input.strip_prefix(b"http://") {
        (rest, true)
    } else {
        return false;
    };
    if rest.is_empty() || rest.ends_with(b"/") {
        return false;
    }
    for byte in rest {
        if byte.is_ascii_control() || byte.is_ascii_whitespace() || b"@?#\\".contains(byte) {
            return false;
        }
    }
    let mut end = rest.len();
    for (index, byte) in rest.iter().enumerate() {
        if *byte == b'/' {
            end = index;
            break;
        }
    }
    let Ok(authority) = core::str::from_utf8(rest.get(..end).expect("host prefix")) else { return false };
    let (host, port) = if let Some(ipv6) = authority.strip_prefix('[') {
        let Some((host, suffix)) = ipv6.split_once(']') else { return false };
        if suffix.is_empty() {
            (host, None)
        } else if let Some(port) = suffix.strip_prefix(':') {
            (host, Some(port))
        } else {
            return false;
        }
    } else if let Some((host, port)) = authority.split_once(':') {
        (host, Some(port))
    } else {
        (authority, None)
    };
    if host.is_empty() {
        return false;
    }
    if let Some(port) = port
        && port.parse::<u16>().is_err()
    {
        return false;
    }
    match host.parse::<core::net::IpAddr>() {
        Ok(ip) => !plain || ip.is_loopback(),
        Err(_) => {
            for byte in host.bytes() {
                if !byte.is_ascii_alphanumeric() && byte != b'-' && byte != b'.' {
                    return false;
                }
            }
            !plain || host.eq_ignore_ascii_case("localhost")
        }
    }
}
fn absent(object: &mut config::read::Object, document: &Document, name: &[u8], path: &Path, problems: &mut Problems) {
    if object.member(document, name).is_some() {
        read::form(false, path.member(name), problems);
    }
}
fn nullable_text(
    object: &mut config::read::Object,
    document: &Document,
    name: &[u8],
    path: &Path,
    problems: &mut Problems,
    optional: bool,
) -> Option<Box<[u8]>> {
    let at =
        if optional { object.member(document, name) } else { read::required(object, document, name, path, problems) };
    let at = at?;
    if document.token(at)?.kind == Kind::Null {
        return None;
    }
    match config::read::text(document, at, 1_048_576) {
        Ok(value) => Some(value),
        Err(what) => {
            problems.push(path.member(name), What::Jig(what));
            None
        }
    }
}
#[expect(clippy::too_many_lines, reason = "one reader preserves old and records-only diagnostic order until temper-15")]
fn forge(document: &Document, at: u32, path: &Path, problems: &mut Problems) -> Option<Forge> {
    let mut object = read::object(document, at, path, problems)?;
    let number = read::short(&mut object, document, b"number", path, problems);
    let kind = read::text(&mut object, document, b"kind", path, problems);
    let kind = match kind?.as_ref() {
        b"forgejo" => settings::Kind::Forgejo,
        b"github" => settings::Kind::GitHub,
        _ => {
            read::form(false, path.member(b"kind"), problems);
            return None;
        }
    };
    let github = kind == settings::Kind::GitHub;
    let app_read_only = if object.member(document, b"app_read_only").is_some() {
        read::form(github, path.member(b"app_read_only"), problems);
        read::boolean(&mut object, document, b"app_read_only", path, problems)?
    } else {
        false
    };
    let at = read::required(&mut object, document, b"api", path, problems);
    let mut found = config::Problems::empty();
    let api = match at {
        Some(at) if github && document.token(at)?.kind == Kind::Null => {
            Some(config::Address::Host { name: Box::from(b"api.github.com".as_slice()), port: 443 })
        }
        Some(at) => config::address::read(document, at, &path.member(b"api"), &mut found),
        None => None,
    };
    problems.append(found, &Path::root());
    let at = read::required(&mut object, document, b"transport", path, problems);
    let transport = match at {
        Some(at) if github && document.token(at)?.kind == Kind::Null => {
            Some(Transport::Tls { server_name: Box::from(b"api.github.com".as_slice()), trust: None })
        }
        Some(at) => transport(document, at, &path.member(b"transport"), problems),
        None => None,
    };
    let git = nullable_text(&mut object, document, b"git", path, problems, false);
    let git_program = match object.member(document, b"git_program") {
        Some(at) => match config::read::text(document, at, 4096) {
            Ok(value) => {
                read::form(!value.is_empty() && !value.contains(&0), path.member(b"git_program"), problems);
                Some(value)
            }
            Err(what) => {
                problems.push(path.member(b"git_program"), What::Jig(what));
                None
            }
        },
        None => None,
    };
    read::form(github || git.is_some(), path.member(b"git"), problems);
    let git = match git {
        Some(git) => git,
        None => Box::from(b"https://github.com".as_slice()),
    };
    // Old Forgejo documents may omit web; temper-15 removes the file-era shape.
    let web = nullable_text(&mut object, document, b"web", path, problems, !github);
    let web = match kind {
        settings::Kind::Forgejo => {
            read::form(web.is_none(), path.member(b"web"), problems);
            None
        }
        settings::Kind::GitHub => Some(web.unwrap_or(Box::from(b"https://github.com".as_slice()))),
    };
    if let Some(web) = &web {
        read::form(git_base(web), path.member(b"web"), problems);
    }
    // Transitional file members are refused on GitHub; temper-15 removes them.
    let records = !github
        && object.member(document, b"api_token").is_none()
        && object.member(document, b"user").is_none()
        && object.member(document, b"webhook_secret").is_none();
    let api_token = if github || records {
        if github {
            absent(&mut object, document, b"api_token", path, problems);
        }
        Some(Box::from(b"".as_slice()))
    } else {
        read::text(&mut object, document, b"api_token", path, problems)
    };
    let mut webhook_secret = None;
    let user = if github || records {
        absent(&mut object, document, b"webhook_secret", path, problems);
        absent(&mut object, document, b"user", path, problems);
        Some(User::Id(0)) // No file user: temper-02 supplies the records' bot identity.
    } else {
        webhook_secret = nullable_text(&mut object, document, b"webhook_secret", path, problems, false);
        let at = read::required(&mut object, document, b"user", path, problems);
        match at {
            Some(at) => user(document, at, &path.member(b"user"), problems),
            None => None,
        }
    };
    // Transitional inverse join; temper-15 removes forges[].identity.
    let identity = match object.member(document, b"identity") {
        Some(at) => match config::read::integer(document, at, u64::from(u32::MAX)) {
            Ok(value) => {
                read::form(value > 0, path.member(b"identity"), problems);
                u32::try_from(value).expect("checked identity number")
            }
            Err(what) => {
                problems.push(path.member(b"identity"), What::Jig(what));
                return None;
            }
        },
        None => 0,
    };
    read::finish(object, document, path, problems);
    read::form(git_base(&git), path.member(b"git"), problems);
    let api = api?;
    let transport = transport?;
    forge_endpoint(&api, &transport, path, problems);
    Some(Forge {
        records_only: records,
        number: number?,
        kind,
        api,
        transport,
        git,
        git_program,
        web,
        app_read_only,
        api_token: api_token?,
        webhook_secret,
        user: user?,
        identity,
    })
}
fn forge_endpoint(api: &config::Address, transport: &Transport, path: &Path, problems: &mut Problems) {
    match api {
        config::Address::Addr(skein_io::Addr::InProcess(_)) => read::form(false, path.member(b"api"), problems),
        config::Address::Addr(skein_io::Addr::Ip(_)) | config::Address::Host { .. } => {}
    }
    match transport {
        Transport::Plaintext => {
            let loopback = match api {
                config::Address::Addr(skein_io::Addr::Ip(socket)) => socket.ip().is_loopback(),
                config::Address::Addr(skein_io::Addr::InProcess(_)) | config::Address::Host { .. } => false,
            };
            if !loopback {
                problems.push(path.member(b"transport"), What::Loopback);
            }
        }
        Transport::Tls { .. } => {}
    }
}
fn identity(document: &Document, at: u32, path: &Path, problems: &mut Problems) -> Option<Identity> {
    let mut object = read::object(document, at, path, problems)?;
    let number = read::number(&mut object, document, b"number", path, problems);
    let forge = match object.member(document, b"forge") {
        Some(at) => match config::read::integer(document, at, u64::from(u16::MAX)) {
            Ok(value) => Some(u16::try_from(value).expect("checked forge number")),
            Err(what) => {
                problems.push(path.member(b"forge"), What::Jig(what));
                None
            }
        },
        None => None,
    };
    // temper-15 removes token; the forge join below refuses it on GitHub.
    let token_configured = object.member(document, b"token").is_some();
    let mut reread = read::object(document, at, path, problems)?;
    let token = nullable_text(&mut reread, document, b"token", path, problems, true).unwrap_or(Box::new([]));
    let login = nullable_text(&mut object, document, b"login", path, problems, false);
    if let Some(login) = &login {
        read::form(!login.is_empty(), path.member(b"login"), problems);
    }
    let login = login.unwrap_or(Box::new([]));
    let author_at = read::required(&mut object, document, b"author", path, problems);
    read::finish(object, document, path, problems);
    if !login.is_empty() {
        let mut valid = !login.is_empty();
        for byte in &login {
            if *byte == b':' || byte.is_ascii_control() {
                valid = false;
            }
        }
        read::form(valid, path.member(b"login"), problems);
    }
    let author_path = path.member(b"author");
    let author_at = author_at?;
    if document.token(author_at)?.kind == Kind::Null {
        // Null author/login acquire their bot defaults in temper-02.
        return Some(Identity {
            number: number?,
            forge,
            token,
            token_configured,
            login,
            author: settings::Author { name: Box::new([]), email: Box::new([]) },
        });
    }
    let mut author = read::object(document, author_at, &author_path, problems)?;
    let name = read::text(&mut author, document, b"name", &author_path, problems);
    let email = read::text(&mut author, document, b"email", &author_path, problems);
    read::finish(author, document, &author_path, problems);
    for (field, value) in [(b"name".as_slice(), &name), (b"email".as_slice(), &email)] {
        if let Some(value) = value {
            let mut valid = !value.is_empty();
            for byte in value {
                if b"\0\n<>".contains(byte) {
                    valid = false;
                }
            }
            read::form(valid, author_path.member(field), problems);
        }
    }
    Some(Identity {
        number: number?,
        forge,
        token,
        token_configured,
        login,
        author: settings::Author { name: name?, email: email? },
    })
}
fn references(
    forges: &mut List<Forge>,
    identities: &mut List<Identity>,
    charter: u32,
    jig: &config::Engine,
    problems: &mut Problems,
) {
    let path = Path::root().member(b"application");
    for (index, forge) in forges.iter().enumerate() {
        let path = read::index(&path.member(b"forges"), index);
        for previous in forges.iter().take(index) {
            if previous.number == forge.number {
                problems.push(path.member(b"number"), What::Jig(config::What::Unique));
            }
        }
        let mut count = 0_u32;
        for identity in identities.iter() {
            if identity.forge == Some(forge.number) || forge.identity != 0 && identity.number == forge.identity {
                count = count.saturating_add(1);
                if identity.forge.is_some() && forge.identity != 0 {
                    count = count.saturating_add(1);
                }
            }
        }
        if count != 1 {
            problems.push(path.member(b"identity"), What::Jig(config::What::Refers));
        }
    }
    for index in 0..identities.len() {
        let identity = identities.get(index).expect("identity index");
        let path = read::index(&path.member(b"identities"), usize::try_from(index).expect("identity index"));
        for previous in identities.iter().take(usize::try_from(index).expect("identity index")) {
            if previous.number == identity.number {
                problems.push(path.member(b"number"), What::Jig(config::What::Unique));
            }
        }
        let number = identity.number;
        let mut count = 0_u32;
        let mut joined = None;
        for forge_index in 0..forges.len() {
            let forge = forges.get_mut(forge_index).expect("forge index");
            if identity.forge == Some(forge.number) || forge.identity != 0 && forge.identity == number {
                count = count.saturating_add(1);
                joined = Some(forge.number);
                forge.identity = number;
                match forge.kind {
                    // temper-15 removes the old file identity branch. Presence selects it.
                    settings::Kind::Forgejo if !forge.records_only || identity.token_configured => {
                        if forge.records_only {
                            let fp = read::index(
                                &Path::root().member(b"application").member(b"forges"),
                                usize::try_from(forge_index).expect("bounded forges"),
                            );
                            for name in [b"api_token".as_slice(), b"webhook_secret", b"user"] {
                                problems.push(fp.member(name), What::Jig(config::What::Missing));
                            }
                        }
                        read::form(!identity.token.is_empty(), path.member(b"token"), problems);
                        read::form(!identity.login.is_empty(), path.member(b"login"), problems);
                        read::form(!identity.author.name.is_empty(), path.member(b"author"), problems);
                    }
                    settings::Kind::Forgejo => {
                        read::form(!identity.author.name.is_empty(), path.member(b"author"), problems);
                    }
                    settings::Kind::GitHub => read::form(!identity.token_configured, path.member(b"token"), problems),
                }
            }
        }
        if count != 1 {
            problems.push(path.member(b"forge"), What::Jig(config::What::Refers));
        }
        identities.get_mut(index).expect("identity index").forge = joined;
    }
    {
        let mut found = false;
        for entry in &jig.charters {
            if entry.number == charter {
                found = true;
            }
        }
        if !found {
            problems.push(path.member(b"charter"), What::Jig(config::What::Refers));
        }
    }
}
fn workers(
    document: &Document,
    ats: Option<List<u32>>,
    count: u32,
    directories: &config::Directories,
    path: &Path,
    problems: &mut Problems,
) -> List<Worker> {
    let mut workers = List::with_capacity(count);
    if let Some(ats) = ats {
        for (index, at) in ats.iter().enumerate() {
            let path = read::index(&path.member(b"workers"), index);
            let Some(mut object) = read::object(document, *at, &path, problems) else { continue };
            let root = read::text(&mut object, document, b"root", &path, problems);
            let at = read::required(&mut object, document, b"git", &path, problems);
            let mut git = None;
            if let Some(at) = at {
                let null = match document.token(at) {
                    Some(token) => token.kind == Kind::Null,
                    None => false,
                };
                if !null {
                    match config::read::text(document, at, 1_048_576) {
                        Ok(value) => git = Some(value),
                        Err(what) => problems.push(path.member(b"git"), What::Jig(what)),
                    }
                }
            }
            read::finish(object, document, &path, problems);
            if let Some(root) = root {
                if let Some(git) = &git
                    && !git.starts_with(b"/")
                {
                    problems.push(path.member(b"git"), What::Absolute);
                }
                if let Some(root) = joined(directories, config::Directory::Data, &root, &path.member(b"root"), problems)
                {
                    workers.push(Worker { root, git }).expect("bounded workers");
                }
            }
        }
    }
    workers
}
fn resolve_identity(directories: &config::Directories, identity: &mut Identity, path: &Path, problems: &mut Problems) {
    if !identity.token.is_empty()
        && let Some(value) =
            joined(directories, config::Directory::Data, &identity.token, &path.member(b"token"), problems)
    {
        identity.token = value;
    }
}
fn github_listener(forges: &List<Forge>, clients: Option<&Clients>, path: &Path, problems: &mut Problems) {
    if let Some(clients) = clients {
        for forge in forges {
            if forge.kind == settings::Kind::GitHub
                && let config::Address::Addr(skein_io::Addr::Ip(socket)) = &clients.listener
                && socket.port() == 0
            {
                read::form(false, path.member(b"clients").member(b"listener"), problems);
                break;
            }
        }
    }
}
pub(super) fn worker_alone(
    document: &Document,
    at: u32,
    directories: &config::Directories,
    problems: &mut Problems,
) -> Option<(Worker, limits::Limits)> {
    let path = Path::root().member(b"application");
    let mut object = read::object(document, at, &path, problems)?;
    let ats = read::list(&mut object, document, b"workers", document.len(), &path, problems);
    if match &ats {
        Some(ats) => ats.len() != 1,
        None => false,
    } {
        problems.push(path.member(b"workers"), What::WorkerAlone);
    }
    let worker = workers(document, ats, document.len(), directories, &path, problems).into_boxed().into_iter().next();
    let mut overrides = List::with_capacity(document.len());
    let limits = match read::required(&mut object, document, b"limits", &path, problems) {
        Some(at) => configured_limits(document, at, &path.member(b"limits"), problems, &mut overrides),
        None => None,
    };
    if let Some(limits) = limits {
        super::checks::limits(&limits, &limits.git_component, problems);
        let bounds = temper_protocol_worker_wire::Bounds {
            repositories: limits.checkout.repositories,
            name_bytes: limits.checkout.name_bytes,
            remote_bytes: limits.checkout.name_bytes,
            conflicts: limits.checkout.conflicts,
            path_bytes: limits.checkout.path_bytes,
            token_bytes: temper_protocol_worker_wire::v1::CEILINGS.token_token,
            entries: limits.forges.installations,
            entry_token_bytes: limits.forge_component.token_bytes,
            text_bytes: temper_protocol_worker_wire::v1::CEILINGS.identity_name,
        };
        super::checks::channel(None, Some(&bounds), &[], &limits, problems);
    }
    super::finish_shape(document, object, &path, problems);
    Some((worker?, limits?))
}
fn webhooks(document: &Document, at: Option<u32>, path: &Path, problems: &mut Problems) -> Option<Webhooks> {
    let at = at?;
    if document.token(at)?.kind == Kind::Null {
        return None;
    }
    let value = clients(document, at, &path.member(b"webhooks"), false, problems)?;
    Some(Webhooks { listener: value.listener })
}
#[expect(clippy::too_many_lines, reason = "startup reads and checks the application members together")]
pub fn parse(
    document: &Document,
    at: u32,
    jig: &config::Engine,
    directories: &config::Directories,
    problems: &mut Problems,
) -> Option<Members> {
    let path = Path::root().member(b"application");
    let mut object = read::object(document, at, &path, problems)?;
    let mut overrides = List::with_capacity(document.len());
    let setup_at = read::required(&mut object, document, b"setup", &path, problems);
    let demo_at = read::required(&mut object, document, b"demo", &path, problems);
    let limit_at = read::required(&mut object, document, b"limits", &path, problems);
    let limits = match limit_at {
        Some(at) => configured_limits(document, at, &path.member(b"limits"), problems, &mut overrides),
        None => None,
    };
    let bounds = limits.unwrap_or(limits::profile(limits::Profile::Small));
    let forge_ats = read::list(&mut object, document, b"forges", bounds.forge.client.repositories, &path, problems);
    let identity_ats =
        read::list(&mut object, document, b"identities", jig.limits.core.accounts.credentials, &path, problems);
    let workers_ats = read::list(&mut object, document, b"workers", jig.limits.core.fleet.workers, &path, problems);
    let charter = read::number(&mut object, document, b"charter", &path, problems);
    let clients_at = read::required(&mut object, document, b"clients", &path, problems);
    let clients = match clients_at {
        Some(at) => clients(document, at, &path.member(b"clients"), true, problems),
        None => None,
    };
    let sign_in_at = read::nullable(&mut object, document, b"sign_in", &path, problems);
    let sign_in = sign_in::parse(document, sign_in_at, jig, bounds.sign_in, directories, problems);
    let landing_at = read::required(&mut object, document, b"landing", &path, problems);
    let at = read::required(&mut object, document, b"webhooks", &path, problems);
    let webhooks = webhooks(document, at, &path, problems);
    read::finish(object, document, &path, problems);
    let forge_count = match &forge_ats {
        Some(ats) => ats.len(),
        None => 0,
    };
    let identity_count = match &identity_ats {
        Some(ats) => ats.len(),
        None => 0,
    };
    if forge_ats.is_some() && forge_count == 0 {
        problems.push(path.member(b"forges"), What::Jig(config::What::Bound));
    }
    if identity_ats.is_some() && forge_ats.is_some() && identity_count != forge_count {
        problems.push(path.member(b"identities"), What::Jig(config::What::Bound));
    }
    let mut forges = List::with_capacity(bounds.forge.client.repositories);
    if let Some(ats) = forge_ats {
        for (index, at) in ats.iter().enumerate() {
            if let Some(mut forge) = forge(document, *at, &read::index(&path.member(b"forges"), index), problems) {
                resolve_forge(directories, &mut forge, &read::index(&path.member(b"forges"), index), problems);
                forges.push(forge).expect("bounded forges");
            }
        }
    }
    let mut identities = List::with_capacity(jig.limits.core.accounts.credentials);
    if let Some(ats) = identity_ats {
        for (index, at) in ats.iter().enumerate() {
            if let Some(mut identity) =
                identity(document, *at, &read::index(&path.member(b"identities"), index), problems)
            {
                resolve_identity(
                    directories,
                    &mut identity,
                    &read::index(&path.member(b"identities"), index),
                    problems,
                );
                identities.push(identity).expect("bounded identities");
            }
        }
    }
    if let Some(sign_in) = &sign_in {
        sign_in::check_forges(&sign_in.forges, forges.as_slice(), jig, document, setup_at, problems);
    }
    github_listener(&forges, clients.as_ref(), &path, problems);
    let demo = demo::parse(document, demo_at, setup_at, &forges, directories, problems);
    let workers = workers(document, workers_ats, jig.limits.core.fleet.workers, directories, &path, problems);
    if let Some(charter) = charter
        && forges.len() == forge_count
        && identities.len() == identity_count
    {
        references(&mut forges, &mut identities, charter, jig, problems);
    }
    let landing = match landing_at {
        Some(at) => landing::parse(document, at, jig, &bounds, forges.as_slice(), problems),
        None => None,
    };
    if !problems.is_empty() {
        return None;
    }
    let (landing, landing_projects) = landing?;
    Some(Members {
        clients: clients?,
        sign_in,
        webhooks,
        forges: forges.into_boxed(),
        demo,
        asked_owned: asked_owned(document, setup_at),
        identities: identities.into_boxed(),
        workers: workers.into_boxed(),
        charter: charter?,
        limits: limits?,
        overrides: overrides.into_boxed(),
        landing,
        landing_projects,
    })
}
// Read only the startup view; the setup owner checks the complete entries.
fn asked_entry(document: &Document, at: u32) -> Option<u16> {
    let mut entry = config::read::Object::new(document, at).ok()?;
    let start = config::read::text(document, entry.member(document, b"start")?, 4096).ok()?;
    if start.as_ref() != b"asked" {
        return None;
    }
    let mut mode = config::read::Object::new(document, entry.member(document, b"mode")?).ok()?;
    mode.member(document, b"owned")?;
    let number = config::read::integer(document, entry.member(document, b"forge")?, u64::from(u16::MAX)).ok()?;
    Some(u16::try_from(number).expect("bounded forge number"))
}
fn asked_owned(document: &Document, at: Option<u32>) -> Box<[u16]> {
    let entries = match at {
        Some(at) => match config::read::list(document, at, document.len()) {
            Ok(entries) => entries,
            Err(_) => return Box::new([]),
        },
        None => return Box::new([]),
    };
    let mut asked = List::with_capacity(entries.len());
    for at in entries.into_boxed() {
        if let Some(number) = asked_entry(document, at) {
            asked.push(number).expect("setup capacity");
        }
    }
    asked.into_boxed()
}
/// Files required at startup, including every forge except asked owned ones.
pub fn files(members: &Members, api_token_bytes: u32, token_bytes: u32) -> Box<[config::File]> {
    listed_files(members, api_token_bytes, token_bytes, false)
}
/// Transitional credentials of asked owned forges; their absence is allowed at startup.
pub fn asked_files(members: &Members, api_token_bytes: u32, token_bytes: u32) -> Box<[config::File]> {
    listed_files(members, api_token_bytes, token_bytes, true)
}
fn listed_files(members: &Members, api_token_bytes: u32, token_bytes: u32, asked: bool) -> Box<[config::File]> {
    let count = members
        .forges
        .len()
        .checked_mul(4)
        .expect("bounded forges")
        .checked_add(members.identities.len())
        .expect("bounded files");
    let mut files =
        List::with_capacity(u32::try_from(count.checked_add(2).expect("sign-in files")).expect("bounded files"));
    if !asked && let Some(sign_in) = &members.sign_in {
        sign_in::files(sign_in, members.limits.sign_in, &mut files);
    }
    for (index, forge) in members.forges.iter().enumerate() {
        let path = read::index(&Path::root().member(b"application").member(b"forges"), index);
        let include = members.asked_owned.contains(&forge.number) == asked;
        // temper-15 removes file reads; records-only entries never name a file.
        if include && forge.kind == settings::Kind::Forgejo && !forge.records_only {
            files
                .push(config::File {
                    at: path.member(b"api_token"),
                    path: forge.api_token.clone(),
                    max: api_token_bytes,
                    private: true,
                })
                .expect("bounded files");
        }
        if include && let User::File(name) = &forge.user {
            files
                .push(config::File {
                    at: path.member(b"user").member(b"file"),
                    path: name.clone(),
                    max: USER_BYTES,
                    private: true,
                })
                .expect("bounded files");
        }
        if include && let Some(name) = &forge.webhook_secret {
            files
                .push(config::File {
                    at: path.member(b"webhook_secret"),
                    path: name.clone(),
                    max: members.limits.forge_component.hook_secret_bytes,
                    private: true,
                })
                .expect("bounded files");
        }
        match &forge.transport {
            Transport::Plaintext => {}
            Transport::Tls { trust, .. } => {
                if !asked && let Some(trust) = trust {
                    files
                        .push(config::File {
                            at: path.member(b"transport").member(b"tls").member(b"trust"),
                            path: trust.clone(),
                            max: 1_048_576,
                            private: false,
                        })
                        .expect("bounded files");
                }
            }
        }
    }
    for (index, identity) in members.identities.iter().enumerate() {
        if identity.token.is_empty()
            || members.asked_owned.contains(&identity.forge.expect("checked identity join")) != asked
        {
            continue;
        }
        files
            .push(config::File {
                at: read::index(&Path::root().member(b"application").member(b"identities"), index).member(b"token"),
                path: identity.token.clone(),
                max: token_bytes,
                private: true,
            })
            .expect("bounded files");
    }
    files.into_boxed()
}
#[derive(Debug)]
pub struct Resolved {
    pub sign_in: Option<settings::SignIn>,
    pub webhooks: Option<skein_forge_connection::hooks::Webhooks>,
    pub forges: Box<[settings::Forge]>,
    pub identities: Box<[settings::Identity]>,
}
pub(super) fn contents(
    files: &settings::Files,
    name: &[u8],
    max: u32,
    at: Path,
    problems: &mut Problems,
) -> Option<Box<[u8]>> {
    for (path, value) in &files.entries {
        if path.as_ref() == name {
            if value.expose().len() > usize::try_from(max).expect("file bound") {
                problems.push(at, What::Jig(config::What::Bound));
                return None;
            }
            return Some(bytes::copy_of(value.expose()));
        }
    }
    problems.push(at, What::Jig(config::What::Unreadable));
    None
}
fn resolved_transport(
    transport: &Transport,
    files: &settings::Files,
    path: &Path,
    problems: &mut Problems,
) -> settings::Transport {
    match transport {
        Transport::Plaintext => settings::Transport::Plaintext,
        Transport::Tls { server_name, trust } => {
            let trust = match trust {
                Some(name) => contents(
                    files,
                    name,
                    1_048_576,
                    path.member(b"transport").member(b"tls").member(b"trust"),
                    problems,
                ),
                None => None,
            };
            settings::Transport::Tls { server_name: server_name.clone(), trust }
        }
    }
}
/// Resolve setup modes and the fake forge's program after both readers resolve data paths.
pub fn git_programs(forges: &mut [settings::Forge], setup: &temper_forge_setup::setup::Setup, problems: &mut Problems) {
    let application = Path::root().member(b"application");
    for (index, entry) in setup.entries.iter().enumerate() {
        for forge in forges.iter_mut() {
            if forge.number == entry.forge {
                forge.mode = match &entry.mode {
                    temper_forge_setup::setup::Mode::Managed { .. } => temper_engine_domain_forge::Mode::Managed,
                    temper_forge_setup::setup::Mode::Owned(_) => temper_engine_domain_forge::Mode::Owned,
                    temper_forge_setup::setup::Mode::Fake(_) => temper_engine_domain_forge::Mode::Fake,
                };
            }
        }
        if let temper_forge_setup::setup::Mode::Fake(fake) = &entry.mode {
            for (forge_index, forge) in forges.iter_mut().enumerate() {
                if forge.number == entry.forge {
                    if forge.git_program.is_some() {
                        problems.push(
                            read::index(&application.member(b"forges"), forge_index).member(b"git_program"),
                            What::Jig(config::What::Form),
                        );
                        problems.push(
                            read::index(&application.member(b"setup"), index)
                                .member(b"mode")
                                .member(b"fake")
                                .member(b"git"),
                            What::Jig(config::What::Form),
                        );
                    } else {
                        forge.git_program = Some(fake.git.clone());
                    }
                }
            }
        }
    }
}
pub(super) fn missing_asked_files(members: &Members, files: &settings::Files, number: u16) -> bool {
    if !members.asked_owned.contains(&number) {
        return false;
    }
    for forge in &members.forges {
        if forge.number != number {
            continue;
        }
        if files.contents(&forge.api_token).is_none() {
            return true;
        }
        if let User::File(name) = &forge.user
            && files.contents(name).is_none()
        {
            return true;
        }
        if let Some(name) = &forge.webhook_secret
            && files.contents(name).is_none()
        {
            return true;
        }
    }
    for identity in &members.identities {
        if identity.forge == Some(number) && !identity.token.is_empty() && files.contents(&identity.token).is_none() {
            return true;
        }
    }
    false
}
fn resolved_user(
    forge: &Forge,
    files: &settings::Files,
    path: &Path,
    missing: bool,
    problems: &mut Problems,
) -> Option<u64> {
    match &forge.user {
        User::Id(value) => Some(*value),
        User::File(name) if missing && files.contents(name).is_none() => Some(0),
        User::File(name) => {
            let at = path.member(b"user").member(b"file");
            match contents(files, name, USER_BYTES, at.clone(), problems) {
                Some(value) => match read::decimal(&value) {
                    Some(value) => Some(value),
                    None => {
                        problems.push(at, What::Jig(config::What::Form));
                        None
                    }
                },
                None => None,
            }
        }
    }
}
pub fn resolve(members: &Members, files: &settings::Files, problems: &mut Problems) -> Option<Resolved> {
    let mut forges = List::with_capacity(u32::try_from(members.forges.len()).expect("bounded forges"));
    let mut identities = List::with_capacity(u32::try_from(members.identities.len()).expect("bounded identities"));
    for (index, forge) in members.forges.iter().enumerate() {
        let path = read::index(&Path::root().member(b"application").member(b"forges"), index);
        let missing = missing_asked_files(members, files, forge.number);
        // temper-15 removes the old-form file token branch.
        let token = match forge.kind {
            settings::Kind::Forgejo if missing || forge.records_only => None,
            settings::Kind::Forgejo => Some(config::secret(contents(
                files,
                &forge.api_token,
                super::API_TOKEN_BYTES,
                path.member(b"api_token"),
                problems,
            )?)),
            settings::Kind::GitHub => None,
        };
        let mut webhook_secret = None;
        if let Some(name) = &forge.webhook_secret
            && (!missing || files.contents(name).is_some())
            && let Some(value) = contents(
                files,
                name,
                members.limits.forge_component.hook_secret_bytes,
                path.member(b"webhook_secret"),
                problems,
            )
        {
            webhook_secret = Some(config::secret(value));
        }
        let transport = resolved_transport(&forge.transport, files, &path, problems);
        let user = resolved_user(forge, files, &path, missing, problems);
        if let Some(user) = user {
            forges
                .push(settings::Forge {
                    mode: temper_engine_domain_forge::Mode::Connected,
                    number: forge.number,
                    kind: forge.kind,
                    api: forge.api.clone(),
                    transport,
                    git: forge.git.clone(),
                    git_program: forge.git_program.clone(),
                    web: forge.web.clone(),
                    app_read_only: forge.app_read_only,
                    api_token: token,
                    webhook_secret: if missing { None } else { webhook_secret },
                    user,
                    identity: forge.identity,
                })
                .expect("bounded forges");
        }
    }
    for (index, identity) in members.identities.iter().enumerate() {
        let path = read::index(&Path::root().member(b"application").member(b"identities"), index).member(b"token");
        let token = if identity.token.is_empty()
            || missing_asked_files(members, files, identity.forge.expect("checked identity join"))
        {
            Some(Box::from(b"".as_slice()))
        } else {
            contents(
                files,
                &identity.token,
                jig_protocol_worker_wire::CEILINGS.application_value_secret,
                path,
                problems,
            )
        };
        if let Some(token) = token {
            identities
                .push(settings::Identity {
                    number: identity.number,
                    forge: identity.forge.expect("checked identity join"),
                    token: config::secret(token),
                    login: identity.login.clone(),
                    author: identity.author.clone(),
                })
                .expect("bounded identities");
        }
    }
    if !problems.is_empty() {
        return None;
    }
    let mut webhooks = None;
    if let Some(value) = &members.webhooks
        && let config::Address::Addr(listener) = &value.listener
    {
        webhooks = Some(skein_forge_connection::hooks::Webhooks { listener: *listener });
    }
    Some(Resolved {
        sign_in: match &members.sign_in {
            Some(sign_in) => sign_in::resolve(sign_in, members.limits.sign_in, files, problems),
            None => None,
        },
        forges: forges.into_boxed(),
        identities: identities.into_boxed(),
        webhooks,
    })
}

fn joined(
    directories: &config::Directories,
    kind: config::Directory,
    value: &[u8],
    path: &Path,
    problems: &mut Problems,
) -> Option<Box<[u8]>> {
    match directories.join(kind, value) {
        Ok(value) => Some(value),
        Err(what) => {
            problems.push(path.clone(), What::Jig(what));
            None
        }
    }
}

fn resolve_forge(directories: &config::Directories, forge: &mut Forge, fp: &Path, problems: &mut Problems) {
    if let Some(program) = &mut forge.git_program
        && let Some(resolved) =
            joined(directories, config::Directory::Data, program, &fp.member(b"git_program"), problems)
    {
        *program = resolved;
    }
    if let Transport::Tls { trust: Some(trust), .. } = &mut forge.transport
        && let Some(value) = joined(
            directories,
            config::Directory::Config,
            trust,
            &fp.member(b"transport").member(b"tls").member(b"trust"),
            problems,
        )
    {
        *trust = value;
    }
    // temper-15 removes the old-form token path resolution.
    if forge.kind == settings::Kind::Forgejo
        && !forge.records_only
        && let Some(value) =
            joined(directories, config::Directory::Data, &forge.api_token, &fp.member(b"api_token"), problems)
    {
        forge.api_token = value;
    }
    if let Some(value) = &mut forge.webhook_secret
        && let Some(resolved) =
            joined(directories, config::Directory::Data, value, &fp.member(b"webhook_secret"), problems)
    {
        *value = resolved;
    }
    if let User::File(value) = &mut forge.user
        && let Some(resolved) =
            joined(directories, config::Directory::Data, value, &fp.member(b"user").member(b"file"), problems)
    {
        *value = resolved;
    }
}

/// Public forge configuration for the connector, decided before its first input.
pub(crate) fn configured(settings: &settings::Engine) -> Box<[temper_engine_domain_forge::Configured]> {
    use temper_engine_domain_forge::{Configured, Kind, Mode};
    let mut rows = List::with_capacity(u32::try_from(settings.forges.len()).expect("configured forges"));
    for forge in &settings.forges {
        let mut mode = Mode::Connected;
        let mut admin_connect = false;
        let mut bot: Box<[u8]> = Box::new([]);
        let mut owned = None;
        let mut asked = match &settings.demo {
            Some(demo) => demo.forge == forge.number,
            None => false,
        };
        for setup in &settings.setup {
            if setup.entry.forge == forge.number {
                bot.clone_from(&setup.entry.bot.login);
                (mode, admin_connect) = match &setup.entry.mode {
                    temper_forge_setup::setup::Mode::Managed { admin_token } => (Mode::Managed, admin_token.is_none()),
                    temper_forge_setup::setup::Mode::Owned(value) => {
                        use skein_forge::forgejo::pin;
                        let place = temper_forge_setup::place::Place {
                            directory: setup.entry.directory.clone(),
                            user: Box::new([]),
                        };
                        owned = Some(temper_engine_domain_forge::Owned {
                            forgejo: pin::VERSION.as_bytes().into(),
                            runner: value.runner.is_some().then_some(pin::RUNNER_VERSION.as_bytes().into()),
                            password: place.path(temper_forge_setup::place::File::PersonPassword),
                        });
                        (Mode::Owned, false)
                    }
                    temper_forge_setup::setup::Mode::Fake(_) => (Mode::Fake, false),
                };
                asked |= setup.entry.asked;
                break;
            }
        }
        let kind = match forge.kind {
            settings::Kind::Forgejo => Kind::Forgejo,
            settings::Kind::GitHub => Kind::GitHub,
        };
        rows.push(Configured {
            bot,
            admin_connect,
            owned,
            number: forge.number,
            kind,
            mode,
            asked,
            git: if forge.git.len()
                <= usize::try_from(temper_engine_domain_forge::PULL_ADDRESS_BYTES).expect("u32 fits usize")
            {
                forge.git.clone()
            } else {
                Box::new([])
            },
            web: forge.web.clone(),
            read_only: forge.app_read_only,
        })
        .expect("configured forges");
    }
    rows.into_boxed()
}
