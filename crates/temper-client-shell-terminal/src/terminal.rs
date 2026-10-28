//! The terminal's loop over a client, with time supplied by its host.
use crate::limits;
use skein_lib::{Duration, Secret, Time};
use skein_ui::{Act, NodeId, Tree};
use skein_ui_text::Command;
pub(crate) mod native;
mod run;
pub(crate) mod waiting;
pub use run::Terminal;

/// The native client, or the machine world's script, as the terminal sees it.
pub trait Client {
    fn tree(&self) -> &Tree;
    /// Number of builds that emitted patches; zero before the first tree.
    fn built(&self) -> u64;
    fn act(&mut self, node: NodeId, act: Act) -> bool;
    /// # Errors
    /// Refuses a link that is malformed or names another engine.
    fn sign_in(&mut self, link: &[u8]) -> Result<(), jig_client_protocol_http::Unlinked>;
    fn notice(&mut self) -> Option<Notice>;
    fn close(&mut self);
}
/// Notices owned by the shell; credential bytes never enter the domain.
#[derive(Debug)]
pub enum Notice {
    SignedIn,
    SignInAgain,
    SignInBusy,
    SignInFull { sign_ins: u32, earliest: skein_lib::Wall, link_until: skein_lib::Wall },
    SignInRefused,
    SignedOut,
    Credential { party: u64, token: Secret },
    OtherPartyKept,
    Dropped { count: u32 },
    StoreFailed,
    DirectoryRefused,
}
/// temper's notices and retry state around skein's terminal loop.
pub struct Loop {
    pub terminal: skein_ui_text::Terminal,
    limits: limits::Terminal,
    terminal_operations: u32,
    directory: Box<[u8]>,
    link: Option<Secret>,
    lines: waiting::Waiting,
    built: u64,
    retry: Option<Time>,
    backoff: Duration,
    closing: bool,
    failure: Option<Box<[u8]>>,
}
impl Loop {
    #[must_use]
    pub fn new(
        limits: &limits::Terminal,
        view_limits: &skein_ui::Limits,
        directory: Box<[u8]>,
        link: Option<Secret>,
        requests: u32,
    ) -> Self {
        let settings = skein_ui_text::Settings {
            name: Box::from(*b"temper"),
            longest: limits.line,
            interval: limits.interval,
            status: Some(temper_client_view::frame::link_region()),
            lines: limits.lines,
            line_bytes: u32::try_from(directory.len().checked_add(160).expect("line bytes fit"))
                .expect("line bytes fit"),
        };
        let terminal_operations =
            skein_ui_text::Terminal::operations(&settings, view_limits).expect("terminal operations fit");
        let terminal = skein_ui_text::Terminal::new(settings, view_limits).expect("terminal settings");
        Self {
            terminal,
            limits: *limits,
            terminal_operations,
            directory,
            link,
            lines: waiting::Waiting::new(requests.checked_add(4).expect("waiting line room")),
            built: 0,
            retry: None,
            backoff: limits.first,
            closing: false,
            failure: None,
        }
    }
    pub fn close(&mut self) {
        self.closing = true;
        self.link = None;
        self.retry = None;
        self.terminal.close();
    }
    pub fn iterate<C: Client>(&mut self, now: Time, client: &mut C) {
        let changed = self.built != client.built();
        self.built = client.built();
        self.terminal.iterate(now, client.tree(), changed);
        if !self.closing && self.lines.is_empty() {
            while let Some(command) = self.terminal.commands().pop() {
                match command {
                    Command::Act { node, first, submit } => {
                        if client.act(node, first) && submit {
                            client.act(node, Act::Submit);
                        }
                    }
                    Command::Print | Command::Close | Command::Nothing | Command::Refused { .. } => {
                        unreachable!("the terminal carries out its own commands");
                    }
                }
            }
        }
        while let Some(notice) = client.notice() {
            self.notice(notice, now);
        }
        if !self.closing && self.retry.is_some_and(|deadline| now >= deadline) {
            self.retry = None;
            if let Some(link) = &self.link
                && client.sign_in(link.expose()).is_err()
            {
                self.notice(Notice::SignInRefused, now);
            }
        }
        if !self.closing {
            self.lines.say(self.terminal.room(), |line| self.terminal.say(line));
        }
        if self.terminal.closed() {
            client.close();
        }
    }
    fn wait(&mut self, line: Box<[u8]>) {
        if !self.closing {
            self.lines.push(line);
        }
    }
    fn notice(&mut self, notice: Notice, now: Time) {
        match notice {
            Notice::SignedIn => {
                self.link = None;
                self.retry = None;
            }
            notice @ (Notice::SignInAgain | Notice::SignInBusy) => {
                if self.link.is_some() {
                    self.wait(if matches!(notice, Notice::SignInBusy) {
                        Box::from(b"signing in: the engine is busy; trying again".as_slice())
                    } else {
                        Box::from(b"signing in: no answer from the engine; trying again".as_slice())
                    });
                    self.retry = Some(now.saturating_add(self.backoff));
                    self.backoff = self.backoff.saturating_mul(2).min(self.limits.most);
                }
            }
            Notice::SignInFull { sign_ins, earliest, link_until } => {
                self.failure =
                    Some(temper_client_domain::sign_in::full_line("temper open", sign_ins, earliest, link_until));
                self.close();
            }
            Notice::SignInRefused => {
                self.failure =
                    Some(Box::from(*b"temper open: the link was refused: use the fresh link temper serve printed\n"));
                self.close();
            }
            Notice::SignedOut => self.wait(join(&[
                b"signed out; to sign in again, run temper open ",
                &self.directory,
                b" LINK with a link temper serve printed",
            ])),
            Notice::Credential { party, token } => {
                let size = skein_lib::base64::encoded_len(
                    skein_lib::base64::Alphabet::Url,
                    u32::try_from(token.expose().len()).expect("bounded credential"),
                )
                .expect("credential length fits");
                let mut encoded = skein_lib::Writer::new(usize::try_from(size).expect("encoded length fits"));
                skein_lib::base64::encode(skein_lib::base64::Alphabet::Url, token.expose(), &mut encoded)
                    .expect("credential measured");
                let encoded = encoded.finish();
                self.wait(join(&[
                    b"credential for service ",
                    skein_lib::Decimal::of(party).as_bytes(),
                    b", shown once: ",
                    &encoded,
                ]));
            }
            Notice::Dropped { count } => self.wait(join(&[
                skein_lib::Decimal::of(u64::from(count)).as_bytes(),
                b" requests kept in ",
                &self.directory,
                b" were not sent: another version of temper made them",
            ])),
            Notice::OtherPartyKept => {
                self.wait(join(&[b"work of another party is kept in ", &self.directory, b", and waits for them"]));
            }
            Notice::StoreFailed => {
                self.wait(join(&[b"cannot write the small store in ", &self.directory]));
            }
            Notice::DirectoryRefused => {
                self.failure = Some(join(&[
                    b"temper open: ",
                    &self.directory,
                    b": refused (not a private directory of this user's, or its records unreadable)\n",
                ]));
                self.close();
            }
        }
    }
    #[must_use]
    pub fn next_deadline(&self) -> Option<Time> {
        [self.retry, self.terminal.next_deadline()].into_iter().flatten().min()
    }
    #[must_use]
    pub fn exit(&self) -> Option<(u8, Option<Box<[u8]>>)> {
        self.terminal.closed().then(|| (u8::from(self.failure.is_some()), self.failure.clone()))
    }
}
fn join(parts: &[&[u8]]) -> Box<[u8]> {
    let length = parts.iter().map(|part| part.len()).sum();
    let mut writer = skein_lib::Writer::new(length);
    for part in parts {
        writer.put(part).expect("measured shell line");
    }
    writer.finish()
}
