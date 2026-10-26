//! Reconcile's bounded host: one machine, one connection at a time.
use crate::exchange::{Exchange, pause};
use skein_forge_setup::reconcile::{self, Event, Failure, Need, Reconcile, Reply, Stage, Why};
use skein_io::{
    Io, Request,
    kernel::{Complete, Submit},
};
use skein_lib::{Duration, Env, Queue, Secret, Time, Token, Wall};
use skein_shell::{Clock, Config, Host, Kernel};
use std::{net::SocketAddr, os::unix::ffi::OsStrExt, path::PathBuf};
use temper_forge_setup::{
    given,
    place::{self, File, Place},
    problem::{self, Problems},
};
/// Checked command settings, including the bounds supplied by the profile.
pub struct Settings {
    pub setup_index: u32,
    pub given: skein_forge_setup::given::Given,
    pub address: SocketAddr,
    pub host: Box<[u8]>,
    pub user_agent: &'static str,
    pub directory: PathBuf,
    pub admin_token: PathBuf,
    pub token_bytes: u32,
    pub hook_secret_bytes: u32,
    pub http: skein_http::client::Limits,
    pub documents: skein_forge::forgejo::limits::Limits,
    pub io: skein_io::Limits,
}
/// Why the command did not converge; credentials are never retained here.
#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    Unanswered { forge: u16, deadline: Duration },
    Limits { forge: u16, name: &'static str },
    File { forge: u16, file: File, why: String },
    Stopped(Failure),
    Setup { forge: u16, problems: Problems },
}
impl Refused {
    /// One diagnostic line for the command's standard error.
    #[must_use]
    pub fn line(&self) -> String {
        let (forge, words) = match self {
            Self::Setup { forge, problems } => {
                return format!(
                    "temper forge reconcile: forge {forge}: {}",
                    String::from_utf8_lossy(&problem::lines(problems))
                );
            }
            Self::Unanswered { forge, deadline } => {
                (*forge, format!("Forgejo did not answer within {} ms", deadline.as_nanos() / 1_000_000))
            }
            Self::Limits { forge, name } => (*forge, format!("io.{name}: below the http client's largest read")),
            Self::File { forge, file, why } => (*forge, format!("{}: {why}", file_name(*file))),
            Self::Stopped(failure) => {
                let stage = match failure.step {
                    Stage::Installation => "installation",
                    Stage::App => "app",
                    Stage::State => "state",
                    Stage::Admin => "admin",
                    Stage::Bot => "bot",
                    Stage::Token => "token",
                    Stage::Org => "org",
                    Stage::Repository => "repository",
                    Stage::File => "file",
                };
                let why = match failure.why {
                    Why::KeyForm => "key unusable".into(),
                    Why::Unknown => "unknown, used or expired".into(),
                    Why::OtherApp => "another app".into(),
                    Why::Missing => "missing".into(),
                    Why::Busy => "busy".into(),
                    // temper-13 tells the root and the person about crowded installations.
                    Why::Crowded => "crowded".into(),
                    Why::OtherUser => "another user".into(),
                    Why::Refused(status) => format!("HTTP {status}"),
                    Why::NotAdmin => "not a site admin".into(),
                    Why::BotIsAdmin => "is a site admin".into(),
                    Why::Unmarked => "has no temper:bot marker".into(),
                    Why::Down => "unavailable".into(),
                    Why::Unexpected => "unexpected answer".into(),
                };
                (failure.forge, format!("{stage} {}: {why}", String::from_utf8_lossy(&failure.subject)))
            }
        };
        format!("temper forge reconcile: forge {forge}: {words}\n")
    }
}
/// The private-file bound, including the user's optional final newline.
#[must_use]
pub fn bound(settings: &Settings, file: File) -> u32 {
    match file {
        File::User => 21,
        File::WebhookSecret => settings.hook_secret_bytes,
        File::AppIni
        | File::State
        | File::AdminToken
        | File::PersonPassword
        | File::PersonReadHeader
        | File::PersonWriteHeader
        | File::ApiToken
        | File::GitToken
        | File::RunnerSecret
        | File::RunnerConfiguration
        | File::DumpPartial
        | File::ForgejoLock
        | File::RunnerLock => settings.token_bytes,
    }
}
fn file_name(file: File) -> String {
    let place = Place { directory: Box::new([]), user: Box::new([]) };
    String::from_utf8_lossy(&place.path(file)).trim_start_matches('/').into()
}
fn file_path(settings: &Settings, file: File) -> PathBuf {
    if file == File::AdminToken {
        return settings.admin_token.clone();
    }
    let place = Place { directory: settings.directory.as_os_str().as_bytes().into(), user: Box::new([]) };
    crate::path(&place.path(file)).into()
}
fn unread(why: skein_shell::Unread) -> String {
    match why {
        skein_shell::Unread::Absent => "absent".into(),
        skein_shell::Unread::NotAFile => "not a file".into(),
        skein_shell::Unread::TooLarge { max } => format!("past {max} bytes"),
        skein_shell::Unread::Unsafe(_) => "readable by others or reached through a link".into(),
        skein_shell::Unread::Kernel(_) => "could not read".into(),
    }
}
// temper-15 removes the forge files and this decimal-file/item translation.
fn user_item(bytes: &[u8], forge: u16) -> Result<Secret, Refused> {
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    let value = if !bytes.is_empty() && bytes.iter().all(u8::is_ascii_digit) {
        std::str::from_utf8(bytes).ok().and_then(|text| text.parse::<u64>().ok())
    } else {
        None
    };
    value.map(|value| Secret::new(Box::new(value.to_le_bytes()))).ok_or(Refused::File {
        forge,
        file: File::User,
        why: "could not read".into(),
    })
}
// temper-15 removes the forge files and this item/decimal-file translation.
fn user_file(bytes: &[u8], forge: u16) -> Result<Secret, Refused> {
    let value = <[u8; 8]>::try_from(bytes).map_err(|_| Refused::File {
        forge,
        file: File::User,
        why: "could not read".into(),
    })?;
    Ok(Secret::new(format!("{}\n", u64::from_le_bytes(value)).into_bytes().into_boxed_slice()))
}
fn webhook_secret(settings: &Settings) -> Result<(), Refused> {
    if settings.given.listener.is_none() {
        return Ok(());
    }
    let file = File::WebhookSecret;
    let refused = |why| Refused::File { forge: settings.given.forge, file, why };
    let path = file_path(settings, file);
    match skein_shell::read_private_file(&path, bound(settings, file), skein_shell::effective_user()) {
        Ok(_) => return Ok(()),
        Err(skein_shell::Unread::Absent) => (),
        Err(why) => return Err(refused(unread(why))),
    }
    // Since 75e20fd7 skein's reconcile draws no secret; Adopt will use this one; temper-14 decides whether the draw stays.
    let seed = skein_shell::crypto_seed().map_err(|_| refused("entropy unavailable".into()))?;
    let mut secret = [0; 64];
    for (byte, digits) in seed.expose().iter().zip(secret.chunks_exact_mut(2)) {
        digits[0] = b"0123456789abcdef"[usize::from(byte / 16)];
        digits[1] = b"0123456789abcdef"[usize::from(byte % 16)];
    }
    crate::files::write_private(&path, &secret).map_err(|_| refused("could not write whole".into()))
}
const OWNER: Token = Token::new(1);
/// A reconcile host whose caller supplies time on each loop iteration.
pub struct Running {
    settings: Settings,
    io: Io,
    machine: Reconcile,
    needs: Queue<Need>,
    event: Option<Event>,
    exchange: Option<Exchange>,
    pending: Option<(skein_forge::forgejo::request::Request, Secret)>,
    socket: Option<Token>,
    connected: bool,
    closing: bool,
    first: bool,
    attempt: u32,
    retry: Option<Time>,
    began: Time,
    end: Time,
    result: Option<Result<(), Refused>>,
    operations: u32,
    complete: Queue<Complete>,
    subs: Queue<Submit>,
    events: Queue<skein_io::Event>,
}
impl Running {
    /// Checks settings and starts a run with a deadline measured from `began`.
    pub fn new(settings: Settings, began: Time) -> Result<Self, Refused> {
        let machine = Self::check(&settings)?;
        Ok(Self::start(settings, machine, began))
    }
    fn check(settings: &Settings) -> Result<Reconcile, Refused> {
        let machine = Reconcile::new(&settings.given).map_err(|why| Refused::Setup {
            forge: settings.given.forge,
            problems: given::refusal(settings.setup_index, why),
        })?;
        for (name, enough) in [
            ("intake", settings.io.intake >= skein_http::client::largest_read(&settings.http)),
            ("output", settings.io.output >= skein_http::client::largest_room(&settings.http)),
        ] {
            if !enough {
                return Err(Refused::Limits { forge: settings.given.forge, name });
            }
        }
        Ok(machine)
    }
    fn start(settings: Settings, machine: Reconcile, began: Time) -> Self {
        let operations = skein_io::operations(&settings.io).expect("usable io limits");
        Self {
            machine,
            io: Io::new(&settings.io),
            end: began.saturating_add(settings.given.limits.deadline),
            settings,
            needs: Queue::with_capacity(reconcile::MAX_OUT),
            event: Some(Event::Begin),
            exchange: None,
            pending: None,
            socket: None,
            connected: false,
            closing: false,
            first: true,
            attempt: 0,
            retry: None,
            began,
            result: None,
            operations,
            complete: Queue::with_capacity(operations),
            subs: Queue::with_capacity(operations),
            events: Queue::with_capacity(
                skein_io::MAX_OUT_UP.events.max(skein_io::MAX_OUT_RESUME.events).max(skein_io::MAX_OUT_FIRE.events),
            ),
        }
    }
    /// The terminal answer, available once reconciliation has stopped.
    #[must_use]
    pub fn outcome(&self) -> Option<&Result<(), Refused>> {
        self.result.as_ref()
    }
    fn down(&mut self, env: &Env<skein_io::Limits>, request: Request) {
        skein_io::down(&mut self.io, env, request, &mut self.subs);
    }
    fn file_error(&self, file: File, why: String) -> Refused {
        Refused::File { forge: self.settings.given.forge, file, why }
    }
    fn local(&mut self, need: Need, now: Time, wall: Wall) {
        match need {
            Need::Send { request, auth } => {
                let (skein_forge_setup::call::Call::Forgejo(request), skein_forge_setup::call::Auth::Token(token)) =
                    (request, auth)
                else {
                    unreachable!(
                        "only in-process Forgejo Reconcile supplies needs; every Send is Forgejo with Token auth"
                    )
                };
                self.pending = Some((request, token));
                self.begin(now, wall);
            }
            Need::Read(item) => {
                let file = place::item(item);
                match skein_shell::read_private_file(
                    &file_path(&self.settings, file),
                    bound(&self.settings, file),
                    skein_shell::effective_user(),
                ) {
                    Ok(bytes) => {
                        let bytes = bytes.expose();
                        let value = if file == File::User {
                            user_item(bytes, self.settings.given.forge)
                        } else {
                            Ok(Secret::new(bytes.strip_suffix(b"\n").unwrap_or(bytes).into()))
                        };
                        match value {
                            Ok(value) => self.event = Some(Event::Read(Some(value))),
                            Err(why) => self.result = Some(Err(why)),
                        }
                    }
                    Err(skein_shell::Unread::Absent) => self.event = Some(Event::Read(None)),
                    Err(why) => self.result = Some(Err(self.file_error(file, unread(why)))),
                }
            }
            Need::Write { item, bytes } => {
                let file = place::item(item);
                let bytes = if file == File::User {
                    match user_file(bytes.expose(), self.settings.given.forge) {
                        Ok(bytes) => bytes,
                        Err(why) => {
                            self.result = Some(Err(why));
                            return;
                        }
                    }
                } else {
                    bytes
                };
                match crate::files::write_private(&file_path(&self.settings, file), bytes.expose()) {
                    Ok(()) => self.event = Some(Event::Written),
                    Err(_) => self.result = Some(Err(self.file_error(file, "could not write whole".into()))),
                }
            }
            Need::Draw(_) => match skein_shell::crypto_seed() {
                Ok(seed) => self.event = Some(Event::Drawn(seed)),
                Err(_) => self.result = Some(Err(self.file_error(File::AdminToken, "entropy unavailable".into()))),
            },
            Need::Done(_) => self.result = Some(webhook_secret(&self.settings)),
            Need::Stop(failure) => self.result = Some(Err(Refused::Stopped(failure))),
        }
    }
    fn begin(&mut self, now: Time, wall: Wall) {
        let (request, token) = self.pending.as_ref().expect("pending request");
        if let Ok(exchange) = Exchange::new(&self.settings, request, token, now, wall) {
            self.exchange = Some(exchange);
            self.down(
                &Env { now, wall, limits: self.settings.io },
                Request::Connect { owner: OWNER, addr: skein_io::Addr::Ip(self.settings.address) },
            );
        } else {
            self.first = false;
            self.event = Some(Event::Reply(Reply::Down));
            self.pending = None;
        }
    }

    fn answered(&mut self, reply: Reply, now: Time) {
        self.exchange = None;
        if self.first && matches!(reply, Reply::Down | Reply::Status(500..=599)) {
            if let Some(wait) = pause(
                self.settings.given.limits.backoff,
                self.settings.given.limits.backoff_max,
                self.settings.given.limits.deadline,
                self.attempt,
                self.began,
                now,
            ) {
                self.attempt = self.attempt.saturating_add(1);
                self.retry = Some(now.saturating_add(wait));
            } else {
                self.result = Some(Err(Refused::Unanswered {
                    forge: self.settings.given.forge,
                    deadline: self.settings.given.limits.deadline,
                }));
            }
        } else {
            self.first = false;
            self.pending = None;
            self.event = Some(Event::Reply(reply));
        }
    }
    fn events(&mut self, env: &Env<skein_io::Limits>) {
        while let Some(event) = self.events.pop() {
            match event {
                skein_io::Event::Connecting { socket, .. } => self.socket = Some(socket),
                skein_io::Event::Connected { .. } => self.connected = true,
                skein_io::Event::Stream { up, .. } => {
                    if let Some(exchange) = &mut self.exchange {
                        exchange.stream(&self.settings, env.now, env.wall, up);
                    }
                }
                skein_io::Event::Failed { .. } => {
                    if self.exchange.is_some() {
                        self.answered(Reply::Down, env.now);
                    }
                    self.connected = false;
                    self.closing = self.socket.is_some();
                }
                skein_io::Event::Closed { .. } => {
                    self.closing = false;
                    self.connected = false;
                    self.socket = None;
                }
                skein_io::Event::Listening { .. }
                | skein_io::Event::Accepted { .. }
                | skein_io::Event::Output { .. }
                | skein_io::Event::Spawned { .. }
                | skein_io::Event::Exited { .. }
                | skein_io::Event::Usage { .. }
                | skein_io::Event::Resized
                | skein_io::Event::Window { .. }
                | skein_io::Event::Shutdown { .. } => unreachable!("only socket operations"),
            }
        }
    }
}
impl Host for Running {
    fn iterate(&mut self, now: Time, wall: Wall) {
        let env = Env { now, wall, limits: self.settings.io };
        while self.io.is_ready() {
            skein_io::resume(&mut self.io, &env, &mut self.events, &mut self.subs);
            self.events(&env);
        }
        while let Some(complete) = self.complete.pop() {
            skein_io::up(&mut self.io, &env, complete, &mut self.events, &mut self.subs);
            self.events(&env);
        }
        while self.io.is_due(now) {
            skein_io::fire(&mut self.io, &env, &mut self.events, &mut self.subs);
            self.events(&env);
        }
        if self.result.is_none() && now >= self.end {
            self.retry = None;
            if self.first {
                self.result = Some(Err(Refused::Unanswered {
                    forge: self.settings.given.forge,
                    deadline: self.settings.given.limits.deadline,
                }));
                self.exchange = None;
            } else if self.exchange.is_some() {
                self.answered(Reply::Down, now);
            }
        }
        if self.exchange.is_none() && self.socket.is_some() && !self.closing {
            self.closing = true;
            self.down(&env, Request::Close { entity: self.socket.expect("socket to close") });
        }
        if self.closing {
            self.io.reclaim();
            return;
        }
        if self.retry.is_some_and(|time| now >= time) && self.result.is_none() {
            self.retry = None;
            self.begin(now, wall);
        }
        loop {
            if self.result.is_some() || self.retry.is_some() {
                break;
            }
            if let Some(event) = self.event.take() {
                reconcile::step(&mut self.machine, event, &mut self.needs);
            }
            if let Some(need) = self.needs.pop() {
                self.local(need, now, wall);
                continue;
            }
            if !self.connected {
                break;
            }
            let down = self.exchange.as_mut().and_then(|exchange| exchange.pump(&self.settings, now, wall));
            if let Some(down) = down {
                self.down(&env, Request::Stream { stream: self.socket.expect("connected socket"), down });
                continue;
            }
            if let Some(reply) = self.exchange.as_mut().and_then(|exchange| exchange.reply.take()) {
                self.answered(reply, now);
                self.closing = true;
                self.down(&env, Request::Close { entity: self.socket.expect("socket to close") });
            }
            break;
        }
        self.io.reclaim();
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        &mut self.complete
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        &mut self.subs
    }
    fn work_pending(&self, now: Time) -> bool {
        self.io.is_ready()
            || self.io.is_due(now)
            || !self.complete.is_empty()
            || (!self.closing && (self.event.is_some() || !self.needs.is_empty()))
    }
    fn next_deadline(&self) -> Option<Time> {
        [self.io.next_deadline(), self.retry, self.result.is_none().then_some(self.end)].into_iter().flatten().min()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        None
    }
    fn is_empty(&self) -> bool {
        self.result.is_some() && self.io.is_empty()
    }
    fn worst_case(&self) -> u64 {
        skein_io::worst_case(&self.settings.io).expect("io memory fits")
            + skein_http::client::worst_case(&self.settings.http).expect("http memory fits")
            + skein_json::tokenizer::worst_case(&self.settings.documents.json).expect("json memory fits")
            + Queue::<Complete>::worst_case(self.operations).expect("completions fit")
            + Queue::<Submit>::worst_case(self.operations).expect("submissions fit")
            + Queue::<skein_io::Event>::worst_case(self.events.capacity()).expect("events fit")
            // Each row needs at least one input byte; retained text is bounded by the input too.
            + u64::from(self.settings.documents.json.length).saturating_mul(u64::try_from(size_of::<skein_forge::forgejo::document::Row>()).expect("row size fits").saturating_add(1))
    }
    fn operations(&self) -> u32 {
        self.operations
    }
}
/// Runs one reconcile machine to completion on its own kernel and loop.
pub fn reconcile(settings: Settings, clock: &Clock) -> Result<(), Refused> {
    let machine = Running::check(&settings)?;
    let operations = skein_io::operations(&settings.io).expect("usable io limits");
    let mut kernel = Kernel::open(Config { operations }).map_err(|_| Refused::File {
        forge: settings.given.forge,
        file: File::AdminToken,
        why: "kernel unavailable".into(),
    })?;
    let mut running = Running::start(settings, machine, clock.now().now);
    skein_shell::drive(&mut kernel, clock, &mut running);
    running.result.expect("terminal reconcile")
}

#[cfg(test)]
mod tests {
    use super::{user_file, user_item};
    #[test]
    fn decimal_user_files_cross_as_little_endian_items_and_back() {
        for (file, id) in [(b"42\n".as_slice(), 42_u64), (b"18446744073709551615\n", u64::MAX)] {
            assert_eq!(user_item(file, 1).expect("decimal file").expose(), id.to_le_bytes());
            assert_eq!(user_file(&id.to_le_bytes(), 1).expect("User item").expose(), file);
        }
    }
    #[test]
    fn invalid_user_bytes_name_the_file_without_showing_its_contents() {
        for bytes in [b"".as_slice(), b"+42", b" 42", b"42\n\n", b"18446744073709551616", &[0xff]] {
            assert_eq!(
                user_item(bytes, 1).expect_err("invalid decimal").line(),
                "temper forge reconcile: forge 1: user: could not read\n"
            );
        }
        for bytes in [b"".as_slice(), &[0; 7], &[0; 9]] {
            assert_eq!(
                user_file(bytes, 1).expect_err("invalid item").line(),
                "temper forge reconcile: forge 1: user: could not read\n"
            );
        }
    }
}
