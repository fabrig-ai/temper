//! Cross-check the written forge members, without reading their files.
use crate::{
    given, limits,
    problem::{Problems, What},
    setup::{Entry, Mode, Setup},
};
use alloc::boxed::Box;
use core::net::SocketAddr;
use jig_protocol_config::{Path, Segment};
use skein_forge_setup::limits::Limits;
use skein_json::Document;
use skein_lib::Writer;
/// How the forge's user is written.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum User {
    Id(u64),
    File(Box<[u8]>),
}
/// The forge and its joined identity, supplied by the binary.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Forge {
    pub number: u16,
    pub api: Option<SocketAddr>,
    pub plaintext: bool,
    pub git: Box<[u8]>,
    pub user: User,
    pub api_token: Box<[u8]>,
    pub webhook_secret: Option<Box<[u8]>>,
    pub identity_token: Box<[u8]>,
    pub identity_login: Box<[u8]>,
}
/// Forge members and the deployment's other listeners.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Members {
    pub forges: Box<[Forge]>,
    pub listeners: Box<[SocketAddr]>,
}
/// Setup and limits on which a command may act.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Checked {
    pub fetch_bytes: u64,
    /// Local forges admitted without any file credential members.
    pub records_only: Box<[u16]>,
    pub setup: Setup,
    pub limits: Limits,
}
fn require(ok: bool, path: Path, ps: &mut Problems) {
    if !ok {
        ps.push(path, What::Reason(b"invalid form"));
    }
}
pub(crate) fn joined(parts: &[&[u8]]) -> Box<[u8]> {
    let mut len = 0_usize;
    for p in parts {
        len = len.checked_add(p.len()).expect("bounded text");
    }
    let mut w = Writer::new(len);
    for p in parts {
        w.put(p).expect("measured text");
    }
    w.finish()
}
/// Checks each entry against its forge, and listener and entry uniqueness.
pub fn members(setup: &Setup, members: &Members, webhooks: bool, ps: &mut Problems) {
    for (i, e) in setup.entries.iter().enumerate() {
        let p = Path::root()
            .member(b"application")
            .member(b"setup")
            .child(Segment::Index(u32::try_from(i).expect("bounded entries")));
        for prev in setup.entries.iter().take(i) {
            require(e.forge != prev.forge, p.member(b"forge"), ps);
            require(e.directory.is_empty() || e.directory != prev.directory, p.member(b"directory"), ps);
        }
        let mut found = None;
        for f in &members.forges {
            if f.number == e.forge {
                found = Some(f);
            }
        }
        let Some(f) = found else {
            ps.push(p.member(b"forge"), What::Reason(b"unknown reference"));
            continue;
        };
        let fp = Path::root().member(b"application").member(b"forges");
        let mut fi = 0;
        for (n, candidate) in members.forges.iter().enumerate() {
            if candidate.number == f.number {
                fi = u32::try_from(n).expect("bounded forges");
            }
        }
        let fp = fp.child(Segment::Index(fi));
        let records = f.api_token.is_empty()
            && f.user == User::Id(0)
            && f.webhook_secret.is_none()
            && f.identity_token.is_empty();
        // temper-15 removes the old file cross-check, preserving its exact refusals.
        let client_managed = e.mode == (Mode::Managed { admin_token: None });
        if client_managed {
            require(f.api_token.is_empty(), fp.member(b"api_token"), ps);
            require(f.user == User::Id(0), fp.member(b"user"), ps);
            require(f.webhook_secret.is_none(), fp.member(b"webhook_secret"), ps);
            require(f.identity_token.is_empty(), p.member(b"forge"), ps);
            let loopback = match f.api {
                Some(address) => address.ip().is_loopback(),
                None => false,
            };
            require(!f.plaintext || loopback, fp.member(b"transport"), ps);
        } else if !records {
            if webhooks {
                require(
                    f.webhook_secret == Some(joined(&[&e.directory, b"/webhook_secret"])),
                    fp.member(b"webhook_secret"),
                    ps,
                );
            }
            require(f.api_token == joined(&[&e.directory, b"/api_token"]), fp.member(b"api_token"), ps);
            require(f.user == User::File(joined(&[&e.directory, b"/user"])), fp.member(b"user"), ps);
            require(f.identity_token == joined(&[&e.directory, b"/git_token"]), p.member(b"forge"), ps);
            require(f.identity_login == e.bot.login, p.member(b"bot").member(b"login"), ps);
        }
        match &e.mode {
            Mode::Managed { admin_token: None } => {}
            Mode::Managed { admin_token: Some(_) } => {
                require(!records, p.member(b"mode"), ps);
                require(!e.asked, p.member(b"start"), ps);
            }
            Mode::Owned(o) => {
                let op = p.member(b"mode").member(b"owned");
                require(
                    o.person.login != e.bot.login && o.person.login.as_ref() != b"temper-admin",
                    op.member(b"person").member(b"login"),
                    ps,
                );
                local_forge(o.listen, f, &fp, &op, members, setup.entries.get(..i).expect("earlier entries"), ps);
            }
            Mode::Fake(fake) => {
                let op = p.member(b"mode").member(b"fake");
                require(!e.asked, p.member(b"start"), ps);
                require(fake.owner != e.bot.login && fake.owner.as_ref() != b"temper-admin", op.member(b"owner"), ps);
                local_forge(fake.listen, f, &fp, &op, members, setup.entries.get(..i).expect("earlier entries"), ps);
            }
        }
    }
}
fn local_forge(
    listen: SocketAddr,
    forge: &Forge,
    fp: &Path,
    mode_path: &Path,
    members: &Members,
    previous: &[Entry],
    ps: &mut Problems,
) {
    let lp = mode_path.member(b"listen");
    require(forge.api == Some(listen), fp.member(b"api"), ps);
    require(forge.plaintext, fp.member(b"transport"), ps);
    require(git_listen(&forge.git, listen), fp.member(b"git"), ps);
    require(!members.listeners.contains(&listen), lp.clone(), ps);
    for prev in previous {
        let other = match &prev.mode {
            Mode::Owned(other) => Some(other.listen),
            Mode::Fake(other) => Some(other.listen),
            Mode::Managed { .. } => None,
        };
        if let Some(other) = other {
            require(listen != other, lp.clone(), ps);
        }
    }
}
#[expect(clippy::disallowed_methods, reason = "configuration protocol parses the written HTTP socket address")]
fn git_listen(git: &[u8], listen: SocketAddr) -> bool {
    let Some(bytes) = git.strip_prefix(b"http://") else {
        return false;
    };
    let Ok(text) = core::str::from_utf8(bytes) else {
        return false;
    };
    text.parse::<SocketAddr>() == Ok(listen)
}
/// Applies setup overrides, checks limits and decodes and cross-checks the list.
pub fn checked(
    document: &Document,
    at: u32,
    directories: &jig_protocol_config::Directories,
    members_value: &Members,
    listener: Option<SocketAddr>,
    overrides: &[(Box<[u8]>, u64)],
) -> Result<Checked, Problems> {
    let mut ps = Problems::empty();
    let mut limits = limits::small();
    let mut fetch_bytes = limits::FETCH_BYTES;
    let lp = Path::root().member(b"application").member(b"limits").member(b"overrides");
    for (name, value) in overrides {
        let result = if name.as_ref() == b"setup.fetch_bytes" {
            limits::apply_fetch_bytes(&mut fetch_bytes, name, *value)
        } else {
            limits::apply(&mut limits, name, *value)
        };
        if let Err(w) = result {
            ps.push(
                lp.member(name),
                What::Reason(match w {
                    limits::Refused::Unknown => b"no limit of temper's has this name",
                    limits::Refused::Range => b"outside its type",
                }),
            );
        }
    }
    let limits_ok = match skein_forge_setup::limits::check(&limits) {
        Ok(()) => true,
        Err(why) => {
            ps.extend(given::refusal(0, why));
            false
        }
    };
    let setup = Setup::read(document, at, &limits, directories, &mut ps);
    if let Some(setup) = &setup {
        members(setup, members_value, listener.is_some(), &mut ps);
        if limits_ok {
            for (position, entry) in setup.entries.iter().enumerate() {
                let values = given::values(entry, listener, &limits);
                if let Err(why) = skein_forge_setup::given::check(&values) {
                    ps.extend(given::refusal(u32::try_from(position).expect("bounded entries"), why));
                }
            }
        }
    }
    if ps.is_empty() {
        let mut records_only =
            skein_lib::List::with_capacity(u32::try_from(members_value.forges.len()).expect("bounded forges"));
        for f in &members_value.forges {
            let local = match setup.as_ref().expect("clean parse").entry(f.number) {
                Some(entry) => match &entry.mode {
                    Mode::Owned(_) | Mode::Fake(_) | Mode::Managed { admin_token: None } => true,
                    Mode::Managed { admin_token: Some(_) } => false,
                },
                None => false,
            };
            if local
                && f.api_token.is_empty()
                && f.user == User::Id(0)
                && f.webhook_secret.is_none()
                && f.identity_token.is_empty()
            {
                records_only.push(f.number).expect("one entry per forge");
            }
        }
        let records_only = records_only.into_boxed();
        Ok(Checked { fetch_bytes, records_only, setup: setup.expect("clean parse"), limits })
    } else {
        Err(ps)
    }
}
