//! Forge commands check the document, then host only their own setup part.
use std::{
    ffi::OsString,
    fs,
    io::Write,
    os::unix::{
        ffi::OsStrExt,
        fs::{DirBuilderExt, OpenOptionsExt},
    },
};
use temper_forge_setup::{
    place::{File, Place},
    prepare::{Exit, Failure, Prepare},
    run::{self, Refusal},
    setup::Mode,
    spec::Which,
    upgrade::Course,
};
use temper_forge_setup_shell::{command, prepare, run as host};
/// A command's diagnostic, without any credential contents.
#[derive(Debug)]
pub struct Refused {
    verb: &'static str,
    number: Option<u16>,
    why: String,
}
impl Refused {
    #[must_use]
    pub fn line(&self) -> String {
        let forge = self.number.map_or_else(String::new, |n| format!("forge {n}: "));
        super::prefixed(&format!("temper forge {}", self.verb), &format!("{forge}{}", self.why))
    }
}
fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}
fn status(exit: Exit) -> String {
    match exit {
        Exit::Code(n) => n.to_string(),
        Exit::Signal(n) => format!("signal {n}"),
    }
}
fn command_error(why: command::Error) -> String {
    match why {
        command::Error::Interrupted => "interrupted".into(),
        command::Error::Kernel(why) => format!("command kernel: {why}"),
        command::Error::Spawn(why) => format!("command could not start: {why:?}"),
    }
}
fn failure(why: prepare::Failed, place: &Place) -> String {
    match why {
        prepare::Failed::Held(file) => format!(
            "{} is running or being prepared already",
            if file == File::RunnerLock { "runner" } else { "forgejo" }
        ),
        prepare::Failed::Io(file, why) => format!("{}: {file}: {why}", text(&place.directory)),
        prepare::Failed::Command(why) => command_error(why),
        prepare::Failed::Refused(why) => match why {
            Failure::Binary { program, printed, .. } => {
                format!("{} printed {printed:?}, not the pinned release", text(&program))
            }
            Failure::Command { command, exit, tail } => {
                format!("{command:?} ended with status {}: {}", status(exit), text(&tail))
            }
            Failure::Token { exit, tail } => {
                format!("no token for temper-admin: ended with status {}: {}", status(exit), text(&tail))
            }
            Failure::PersonToken { file, exit, tail } => {
                format!(
                    "{}: no person token: ended with status {}: {}",
                    text(&place.path(file)),
                    status(exit),
                    text(&tail)
                )
            }
            Failure::Unread { command } => format!("{command:?} printed no required value"),
            Failure::State(why) => format!("{}: state cannot be decoded: {why:?}", text(&place.directory)),
            Failure::Unrecorded => format!("{}: Forgejo files have no preparation state", text(&place.directory)),
            Failure::Newer { which, recorded } => {
                let (name, pin) = pinned(which);
                format!(
                    "{name} is prepared at {}, newer than this temper's pin {pin}: its migrations only go forward",
                    text(&recorded)
                )
            }
            Failure::Unreadable { which } => {
                format!("{} is prepared at a version this temper cannot read", pinned(which).0)
            }
        },
    }
}
fn pinned(which: Which) -> (&'static str, &'static str) {
    use skein_forge::forgejo::pin::{RUNNER_VERSION, VERSION};
    match which {
        Which::Forgejo => ("forgejo", VERSION),
        Which::Runner => ("runner", RUNNER_VERSION),
    }
}
fn run_refusal(why: Refusal, launch: &run::Launch, number: u16, directories: &super::Directories) -> String {
    let flags = directories.flags().iter().map(|arg| text(arg)).collect::<Vec<_>>().join(" ");
    let prepare = |why: String| format!("{why}: run temper forge prepare {flags} {number}");
    match why {
        Refusal::NoRunner => "the setup has no runner".into(),
        Refusal::NotPrepared => prepare("not prepared".into()),
        Refusal::Unfinished => prepare("its preparation did not end".into()),
        Refusal::Runner { recorded } => prepare(if recorded {
            "prepared with a runner, and the setup has none".into()
        } else {
            "prepared without a runner, and the setup has one".into()
        }),
        Refusal::Versions { course: Course::Unreadable, .. } => {
            "prepared at a version this temper cannot read: no command of this temper runs it".into()
        }
        Refusal::Versions { forgejo, runner, course } => {
            use skein_forge::forgejo::pin::{RUNNER_VERSION, VERSION};
            let recorded =
                runner.map_or_else(|| text(&forgejo), |runner| format!("{} / {}", text(&forgejo), text(&runner)));
            let pinned = if launch.runner { format!("{VERSION} / {RUNNER_VERSION}") } else { VERSION.into() };
            let reason = format!("prepared at {recorded}, this temper pins {pinned}");
            if course == Course::Newer {
                format!("{reason}: no command of this temper runs it")
            } else {
                prepare(reason)
            }
        }
        Refusal::Binary { printed } => format!("{} printed {printed:?}, not the pinned release", text(&launch.program)),
    }
}
fn run_failure(why: host::Failed, launch: &run::Launch, number: u16, directories: &super::Directories) -> String {
    match why {
        host::Failed::Refused(why) => run_refusal(why, launch, number, directories),
        host::Failed::Locked => format!(
            "{} is running or being prepared already",
            if launch.which == Which::Forgejo { "forgejo" } else { "runner" }
        ),
        host::Failed::Lock(why) => format!("{}: lock: {why}", text(&launch.lock)),
        host::Failed::State(why) => format!("{}: state {why}", text(&launch.state)),
        host::Failed::Version(why) => command_error(why),
        host::Failed::Exec(why) => format!("{}: {why}", text(&launch.program)),
    }
}
fn inherited_listener(which: Which) -> Result<Option<std::net::SocketAddr>, String> {
    let Some(count) = std::env::var_os("LISTEN_FDS") else { return Ok(None) };
    if which == Which::Runner {
        return Err("LISTEN_FDS: runner takes no listeners (fd 3)".into());
    }
    if count.as_bytes() != b"1" {
        return Err("LISTEN_FDS: forgejo requires exactly 1 listener at fd 3".into());
    }
    // Unlike inherited_listeners, listener_at preserves fd 3's exec flags.
    skein_shell::listener_at(skein_io::kernel::Fd::new(3)).map(Some).map_err(super::listeners_refusal)
}
pub(crate) fn start(
    directories: &super::Directories,
    arguments: &[OsString],
    ssl_cert_file: Option<std::path::PathBuf>,
) -> Result<u8, Refused> {
    let usage = || {
        Refused {
        verb: "prepare",
        number: None,
        why: "usage: temper forge fetch [DIRECTORIES] FORGE | temper forge prepare [DIRECTORIES] FORGE | temper forge run [DIRECTORIES] FORGE forgejo|runner | temper forge reconcile [DIRECTORIES] FORGE | temper forge sign-in [DIRECTORIES] FORGE".into(),
    }
    };
    let (verb, number, which) = match arguments {
        [verb, number] if verb == "fetch" => ("fetch", number, None),
        [verb, number] if verb == "prepare" => ("prepare", number, None),
        [verb, number] if verb == "sign-in" => ("sign-in", number, None),
        [verb, number] if verb == "reconcile" => ("reconcile", number, None),
        [verb, number, which] if verb == "run" => {
            let which = match which.to_str() {
                Some("forgejo") => Which::Forgejo,
                Some("runner") => Which::Runner,
                _ => return Err(usage()),
            };
            ("run", number, Some(which))
        }
        _ => return Err(usage()),
    };
    let number = number.to_str().and_then(|n| n.parse::<u16>().ok()).ok_or_else(usage)?;
    let refused = |why| Refused { verb, number: Some(number), why };
    // Inspect before reading the document or opening the lock: neither may occupy fd 3.
    let listener = which.map(inherited_listener).transpose().map_err(refused)?.flatten();
    let bytes = directories.read().map_err(refused)?;
    let super::setup::Deployment { named, checked, .. } =
        super::setup::check(&bytes, &directories.jig()).map_err(refused)?;
    if verb == "sign-in" {
        return super::forge_sign_in::run(directories, &bytes, &named, &checked, number).map_err(refused);
    }
    if verb == "reconcile" {
        let settings = reconcile_settings(&named, &checked, number).map_err(refused)?;
        return temper_forge_setup_shell::reconcile::reconcile(settings, &skein_shell::Clock::new())
            .map(|()| 0)
            .map_err(|why| {
                refused(
                    why.line().split_once(": ").expect("prefix").1.split_once(": ").expect("forge").1.trim_end().into(),
                )
            });
    }
    let entry = checked.setup.entry(number).ok_or_else(|| refused("the setup has no entry for it".into()))?;
    let owned = match &entry.mode {
        Mode::Owned(owned) => Some(owned),
        Mode::Managed { .. } => return Err(refused("it is managed, not owned".into())),
        Mode::Fake(_) if which.is_some() || verb == "fetch" => return Err(refused("it is fake, not owned".into())),
        Mode::Fake(_) => None,
    };
    if verb == "fetch" {
        let owned = owned.expect("fetch requires owned");
        if !owned.fetch.forgejo && !owned.fetch.runner {
            return Err(refused("neither of its binaries is \"fetched\"".into()));
        }
        let pins = temper_forge_setup::fetch::pins(owned, std::env::consts::ARCH)
            .map_err(|why| refused(text(&why.words())))?;
        let machine = temper_forge_setup::fetch::Fetch::new(pins, checked.fetch_bytes);
        return temper_forge_setup_shell::fetch::run(
            &jig_protocol_config::profile(jig_protocol_config::Profile::Small).io,
            &skein_shell::Trust::Machine { ssl_cert_file },
            machine,
        )
        .map(|()| 0)
        .map_err(|why| refused(why.words()));
    }
    let directory = super::path(&entry.directory);
    if which.is_none() && !directory.exists() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(directory)
            .map_err(|why| refused(format!("{}: {why}", directory.display())))?;
    }
    let user = skein_shell::effective_user();
    if directory.exists() {
        super::private_root(directory, user).map_err(|why| refused(format!("{}: {why}", directory.display())))?;
    }
    let Some(owned) = owned else {
        prepare_fake(&directory.join("admin_token")).map_err(refused)?;
        return Ok(0);
    };
    let place =
        Place { directory: directory.as_os_str().as_bytes().into(), user: super::login(user).map_err(refused)? };
    let limits = jig_protocol_config::profile(jig_protocol_config::Profile::Small).io;
    if let Some(which) = which {
        return run_process(owned, &place, which, listener, (number, directories), &limits).map_err(refused);
    }
    prepare::run(
        &limits,
        &place,
        Prepare::new(
            owned.clone(),
            place.clone(),
            webhook_listener(&named).map_err(refused)?.map(|socket| socket.ip()),
        )
        .with_sign_in(
            named.sign_in.as_ref().is_some_and(|sign_in| sign_in.forges.iter().any(|row| row.forge == number)),
        ),
    )
    .map_err(|why| refused(failure(why, &place)))?;
    Ok(0)
}
/// Admits fd 3 and runs the process, returning its exit or why it did not start.
fn run_process(
    owned: &temper_forge_setup::setup::Owned,
    place: &Place,
    which: Which,
    listener: Option<std::net::SocketAddr>,
    (number, directories): (u16, &super::Directories),
    limits: &skein_io::Limits,
) -> Result<u8, String> {
    if let Err(why) = run::admit_listener(owned, listener) {
        return Err(text(&why.words()));
    }
    let Ok(mut launch) = run::launch(owned, place, which) else { return Err("the setup has no runner".into()) };
    if listener.is_some() {
        launch.exec = temper_forge_setup::spec::listening(launch.exec);
    }
    if let Some(address) = listener {
        return temper_forge_setup_shell::parent::run(limits, &launch, address)
            .map_err(|why| run_failure(why, &launch, number, directories));
    }
    Err(run_failure(host::run(limits, &launch), &launch, number, directories))
}
/// The fake shares only the private directory and token with an owned forge (protocol/setup.md, 8.4).
fn prepare_fake(path: &std::path::Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(_) => return Ok(()),
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => {}
        Err(why) => return Err(format!("{}: {why}", path.display())),
    }
    let seed = skein_shell::crypto_seed().map_err(|errno| format!("random token: errno {errno}"))?;
    let mut token = [0; 40];
    for (byte, digits) in seed.expose().iter().take(20).zip(token.chunks_exact_mut(2)) {
        digits[0] = b"0123456789abcdef"[usize::from(byte / 16)];
        digits[1] = b"0123456789abcdef"[usize::from(byte % 16)];
    }
    match fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path) {
        Ok(mut file) => file.write_all(&token).map_err(|why| format!("{}: {why}", path.display())),
        Err(why) if why.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(why) => Err(format!("{}: {why}", path.display())),
    }
}
#[must_use]
pub fn run(directories: &super::Directories, arguments: &[OsString], ssl_cert_file: Option<std::path::PathBuf>) -> u8 {
    match start(directories, arguments, ssl_cert_file) {
        Ok(code) => code,
        Err(why) => {
            eprint!("{}", why.line());
            1
        }
    }
}
/// Checks the transport before reading files, then prepares the private directory.
pub fn reconcile_settings(
    named: &temper_service::startup::Named,
    checked: &temper_forge_setup::check::Checked,
    number: u16,
) -> Result<temper_forge_setup_shell::reconcile::Settings, String> {
    use skein_forge_connection::limits;
    let forge = named.forges.iter().find(|forge| forge.number == number).ok_or("the setup has no entry for it")?;
    let entry = checked.setup.entry(number).ok_or("the setup has no entry for it")?;
    let directory = super::path(&entry.directory).to_path_buf();
    let admin_token = match &entry.mode {
        Mode::Managed { admin_token: Some(admin_token) } => super::path(admin_token).to_path_buf(),
        Mode::Managed { admin_token: None } => {
            return Err(format!(
                "forge {number}'s admin’s token is given from the client: connect it as its admin there"
            ));
        }
        Mode::Owned(_) | Mode::Fake(_) => directory.join("admin_token"),
    };
    if !matches!(forge.transport, temper_service::startup::Transport::Plaintext) {
        return Err("its transport is TLS, and reconcile speaks plaintext to loopback alone".into());
    }
    if forge.records_only && matches!(entry.mode, Mode::Owned(_) | Mode::Fake(_)) {
        return Err(format!("forge {number} starts from its records: `temper serve` takes its handover"));
    }
    // temper-15 removes the legacy command's file-backed entries.
    if !directory.exists() {
        fs::DirBuilder::new().recursive(true).mode(0o700).create(&directory).map_err(|why| why.to_string())?;
    }
    super::private_root(&directory, skein_shell::effective_user())
        .map_err(|why| format!("{}: {why}", directory.display()))?;
    let address = super::address(&forge.api)?;
    let mut profile = temper_service::limits::profile(temper_service::limits::Profile::Small);
    for (name, value) in &named.overrides {
        if !name.starts_with(b"setup.") && !name.starts_with(b"box.") {
            temper_service::limits::apply(&mut profile, name, *value).map_err(|why| format!("limits: {why:?}"))?;
        }
    }
    let io = jig_protocol_config::profile(jig_protocol_config::Profile::Small).io;
    let setup_index = checked.setup.entries.iter().position(|entry| entry.forge == number).expect("checked entry");
    let given = temper_forge_setup::given::values(entry, webhook_listener(named)?, &checked.limits);
    Ok(temper_forge_setup_shell::reconcile::Settings {
        setup_index: u32::try_from(setup_index).expect("bounded setup entries"),
        given,
        address,
        host: skein_forge::host::host(address, None).expect("checked address"),
        user_agent: temper_service::forge::USER_AGENT,
        directory,
        admin_token,
        token_bytes: profile.forge_component.token_bytes,
        hook_secret_bytes: profile.forge_component.hook_secret_bytes,
        http: limits::http(&profile.forge_component, false, &io),
        documents: limits::documents(&profile.forge_component),
        io,
    })
}

fn webhook_listener(named: &temper_service::startup::Named) -> Result<Option<core::net::SocketAddr>, String> {
    match &named.webhooks {
        Some(webhooks) => super::address(&webhooks.listener).map(Some),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests;
