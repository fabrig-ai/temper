//! Root, view, link and native transport as one host (client/README.md, 6.2).
use crate::{
    limits,
    route::{self, Down, Duty},
};
use jig_client_protocol_link as link;
use jig_client_shell_native::Shell;
use link::Signing;
use skein_io::kernel::{Complete, Fd, Submit};
use skein_lib::{Env, List, Queue, Time, Wall};
use skein_shell::Host;
use temper_client_domain as domain;
use temper_client_view as view;

/// The engine, the adopted directory's last component, and fixed capacities.
#[derive(Clone, Debug)]
pub struct Settings {
    pub engine: skein_io::Addr,
    pub directory: Box<[u8]>,
    pub limits: limits::Limits,
}
/// How the caller established the sign-in; credentials remain in the shell.
#[derive(Debug)]
pub enum Start {
    Link(Box<[u8]>),
    Token,
}
/// Facts counted by the host, outside the client domain.
#[derive(Debug, Default)]
pub struct Seen {
    pub signed_in: u32,
    pub patches: u64,
    pub builds: u64,
    pub facts: u32,
}
/// Notices belong to the caller; no secret enters the root.
#[derive(Debug)]
pub enum Shown {
    Notice(route::Notice),
    Terminal(crate::terminal::Notice),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    WaitingToken,
    Running,
    Closing { started: bool },
}
/// The composition holds no person; its caller drives the semantic tree.
pub struct NativeClient {
    shell: Shell,
    root: domain::Domain,
    view: view::View,
    env: Env<domain::Limits>,
    out: Queue<domain::Request>,
    patches: Queue<skein_ui::Patch>,
    credential: Option<skein_lib::Secret>,
    with: Option<jig_client::With>,
    authentication: skein_lib::Map<skein_lib::Token, bool>,
    kept_keys: skein_lib::Set<jig_client::Key>,
    address: domain::Address,
    phase: Phase,
    dirty: bool,
    signing_out: bool,
    notices: Queue<Shown>,
    signing: Queue<Signing>,
    saving_signing: Option<Signing>,
    seen: Seen,
    memory: u64,
    loaded: bool,
}
impl NativeClient {
    /// Adopt the caller's private directory; validate a printed link before sending.
    /// # Errors
    /// Refuses a printed link that does not name the configured engine.
    pub fn new(
        settings: Settings,
        directory: Fd,
        user: u32,
        seed: u64,
        start: Start,
    ) -> Result<Self, jig_client_protocol_http::Unlinked> {
        Self::build(settings, directory, user, seed, start, domain::new)
    }
    /// A browser's root over the native transport: it sends `Return` and asks to `Leave`
    /// (client/README.md, 3.5, The tab's departure). Used only by the process tier
    /// (domain/testing.md, 2.6), behind `browser-root`; the terminal never makes one.
    /// # Errors
    /// Refuses a printed link that does not name the configured engine.
    #[cfg(feature = "browser-root")]
    pub fn departing(
        settings: Settings,
        directory: Fd,
        user: u32,
        seed: u64,
        start: Start,
    ) -> Result<Self, jig_client_protocol_http::Unlinked> {
        Self::build(settings, directory, user, seed, start, domain::new_departing)
    }
    fn build(
        settings: Settings,
        directory: Fd,
        user: u32,
        seed: u64,
        start: Start,
        make_root: fn(&domain::Limits, u64) -> domain::Domain,
    ) -> Result<Self, jig_client_protocol_http::Unlinked> {
        let bounds = settings.limits;
        let mut memory = limits::worst_case(&bounds).expect("native capacities are checked");
        let (view_limits, forge_words) = limits::view(&bounds).expect("native view capacities are checked");
        let notices = domain::max_out(&bounds.client);
        memory = memory
            .checked_add(Queue::<Shown>::worst_case(notices).expect("notice memory"))
            .and_then(|n| n.checked_add(Queue::<Signing>::worst_case(limits::EXCHANGES)?))
            .and_then(|n| n.checked_add(skein_lib::Map::<skein_lib::Token, bool>::worst_case(limits::EXCHANGES)?))
            .expect("native queues fit memory");
        memory = memory
            .checked_add(
                skein_lib::Set::<jig_client::Key>::worst_case(bounds.client.client.domain.requests.requests)
                    .expect("kept key slots"),
            )
            .and_then(|n| n.checked_add(bounds.client.client.domain.document_bytes))
            .expect("notice memory fits");
        let mut shell = Shell::new(jig_client_shell_native::Settings {
            engine: settings.engine,
            directory: settings.directory,
            limits: bounds.connections,
            io: bounds.io,
        });
        let with = match start {
            Start::Link(printed) => Some(jig_client::With::Link { secret: shell.sign_in(&printed)?, held: None }),
            Start::Token => None,
        };
        shell.adopt(directory, user, seed);
        let root = make_root(&bounds.client, seed);
        Ok(Self {
            shell,
            root,
            view: view::View::new(&view_limits, &bounds.client, forge_words, temper_client_protocol_wire::FORGE),
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits: bounds.client },
            out: Queue::with_capacity(domain::max_out(&bounds.client)),
            patches: Queue::with_capacity(view_limits.tree.patches),
            credential: None,
            with,
            authentication: skein_lib::Map::with_capacity(limits::EXCHANGES),
            kept_keys: skein_lib::Set::with_capacity(bounds.client.client.domain.requests.requests),
            address: domain::Address::Inbox { filter: temper_client_domain::address::Filter::ALL },
            phase: Phase::WaitingToken,
            dirty: false,
            signing_out: false,
            notices: Queue::with_capacity(notices),
            signing: Queue::with_capacity(limits::EXCHANGES),
            saving_signing: None,
            seen: Seen::default(),
            memory,
            loaded: false,
        })
    }
    #[must_use]
    pub fn tree(&self) -> &skein_ui::Tree {
        self.view.tree()
    }
    /// Read-only metadata: no credentials, request tokens or page bodies.
    #[must_use]
    pub fn pending(&self) -> crate::diagnostics::Pending {
        use crate::diagnostics::{Chat, Pending, Row};
        use jig_client::thread::Reading;
        let chat = self.root.pages().chat().map(|chat| {
            let mut retained = List::with_capacity(chat.thread.lines.rows.capacity());
            for row in &chat.thread.lines.rows {
                retained
                    .push(Row { position: row.position, committed_turn: row.event.committed_turn() })
                    .expect("one diagnostic per retained row");
            }
            let reading = match &chat.thread.reading {
                Some(Reading::Latest { known }) => Some(Reading::Latest { known: *known }),
                Some(Reading::Older) => Some(Reading::Older),
                Some(Reading::Newer) => Some(Reading::Newer),
                None => None,
            };
            Chat {
                document_state: self.root.client().document(chat.document).map(|document| document.state),
                activity: chat.task(self.root.client()).map(|task| task.activity),
                committed: chat.thread.committed,
                loaded: chat.thread.loaded,
                reading,
                last_read_none: chat.last_read_none,
                retained,
                staged: chat.thread.fresh.rows.len(),
            }
        });
        Pending {
            loaded: self.loaded,
            dirty: self.dirty,
            shell: self.shell.pending(),
            admission: self.root.client().admission(),
            chat,
            view_revision: self.view.revision(),
            builds: self.seen.builds,
            patches: self.seen.patches,
        }
    }
    /// Apply an offered act; false means no binding or an already closing client.
    pub fn act(&mut self, node: skein_ui::NodeId, act: skein_ui::Act) -> bool {
        if matches!(self.phase, Phase::Closing { .. }) {
            return false;
        }
        let Some(event) = view::event(&self.view, node, act) else {
            return false;
        };
        self.event(event);
        true
    }
    /// Navigation is a root event, even when its address did not change.
    pub fn go(&mut self, address: domain::Address) {
        if !matches!(self.phase, Phase::Closing { .. }) {
            self.address = address.clone();
            self.event(domain::Event::Went { place: domain::Place::Address(address) });
        }
    }
    /// # Errors
    /// Refuses a link outside the configured engine's address.
    pub fn sign_in(&mut self, printed: &[u8]) -> Result<(), jig_client_protocol_http::Unlinked> {
        let secret = self.shell.sign_in(printed)?;
        if self.loaded {
            self.event(domain::Event::Client {
                event: jig_client::Event::SignIn {
                    with: jig_client::With::Link { secret, held: self.credential.clone() },
                },
            });
            self.outputs();
        } else {
            self.with = Some(jig_client::With::Link { secret, held: self.credential.clone() });
        }
        Ok(())
    }
    /// Each sign-in outcome is handed to the caller unchanged, once.
    pub fn signing(&mut self) -> Option<Signing> {
        self.signing.pop()
    }
    /// Drain the caller's notices, including credentials to show once.
    pub fn notices(&mut self) -> Option<Shown> {
        self.notices.pop()
    }
    #[must_use]
    pub const fn started(&self) -> bool {
        match self.phase {
            Phase::Running => true,
            Phase::Closing { started } => started,
            Phase::WaitingToken => false,
        }
    }
    #[must_use]
    pub const fn seen(&self) -> &Seen {
        &self.seen
    }
    /// Finish the last small-store write and close; start no more exchanges.
    pub fn close(&mut self) {
        if !matches!(self.phase, Phase::Closing { .. }) {
            self.phase = Phase::Closing { started: self.started() };
            if self.started() {
                domain::take(&mut self.root, &self.env, &mut self.out);
            }
            self.outputs();
            self.shell.close();
        }
    }
    fn loaded(&mut self, credential: Option<skein_lib::Secret>, records: Vec<jig_client::Record>) {
        self.loaded = true;
        self.credential.clone_from(&credential);
        self.phase = Phase::Running;
        let mut kept = List::with_capacity(u32::try_from(records.len()).expect("bounded store"));
        let bounds = link::Limits::new(self.env.limits.client.domain, self.env.limits.client.exchanges)
            .expect("checked link bounds");
        for record in records {
            if let Some(key) = kept_key(&record, &bounds) {
                self.kept_keys.insert(key).expect("bounded kept keys");
            }
            kept.push(record).expect("counted store");
        }
        let with = match self.with.take() {
            Some(jig_client::With::Link { secret, .. }) => Some(jig_client::With::Link { secret, held: credential }),
            with => with.or_else(|| credential.map(jig_client::With::Held)),
        };
        self.event(domain::Event::Start { address: self.address.clone(), kept, with });
    }
    fn event(&mut self, event: domain::Event) {
        if let Some(notice) = crate::terminal::native::event(&event) {
            self.notices.push(Shown::Terminal(notice));
        }
        domain::step(&mut self.root, &self.env, event, &mut self.out);
        if self.root.client().link().state == jig_client::Link::Live
            && let Some(party) = self.root.client().link().party
        {
            let other = self
                .kept_keys
                .iter()
                .any(|key| self.root.client().pending(*key).is_some_and(|pending| pending.party != party));
            while let Some(key) = self.kept_keys.iter().next().copied() {
                self.kept_keys.remove(&key);
            }
            if other {
                self.notices.push(Shown::Terminal(crate::terminal::Notice::OtherPartyKept));
            }
        }
        self.dirty = true;
    }
    fn transport(&mut self, event: jig_client::Event) {
        let credential_notice = match &event {
            jig_client::Event::Reply { message, .. } => service_credential(message, &self.env.limits.client),
            jig_client::Event::Start { .. }
            | jig_client::Event::StartUnlinked { .. }
            | jig_client::Event::SignIn { .. }
            | jig_client::Event::SignOut
            | jig_client::Event::SignOutLost
            | jig_client::Event::Watch { .. }
            | jig_client::Event::Unwatch { .. }
            | jig_client::Event::Read { .. }
            | jig_client::Event::Send { .. }
            | jig_client::Event::Carry { .. }
            | jig_client::Event::Authorize { .. }
            | jig_client::Event::Streamed { .. }
            | jig_client::Event::StreamGone
            | jig_client::Event::Stored
            | jig_client::Event::NotStored
            | jig_client::Event::Up { .. }
            | jig_client::Event::Lost { .. } => None,
        };
        let signing = match &event {
            jig_client::Event::Reply { exchange, message } => self.authentication.remove(exchange).map(|out| {
                use jig_client_protocol_wire as w;
                let mut reader = skein_lib::Reader::new(message);
                if out {
                    match w::SignOutAnswer::decode(&w::CEILINGS, &mut reader) {
                        Ok(w::SignOutAnswer::SignedOut) if reader.is_empty() => Signing::SignedOut,
                        Ok(w::SignOutAnswer::SignedOut) | Err(_) => Signing::Lost,
                        Ok(w::SignOutAnswer::NotReady) => Signing::Again,
                    }
                } else {
                    match w::SignInAnswer::decode(&w::CEILINGS, &mut reader) {
                        Ok(w::SignInAnswer::SignedIn(value)) if reader.is_empty() => {
                            Signing::SignedIn { party: value.party() }
                        }
                        Ok(w::SignInAnswer::Refused) => Signing::Refused,
                        Ok(w::SignInAnswer::NotReady) => Signing::Again,
                        Ok(w::SignInAnswer::Busy) => Signing::Busy,
                        Ok(w::SignInAnswer::Full(value)) => Signing::Full {
                            sign_ins: value.sign_ins(),
                            earliest: skein_lib::Wall::from_nanos(value.earliest()),
                            link_until: skein_lib::Wall::from_nanos(value.link_until()),
                        },
                        Ok(w::SignInAnswer::SignedIn(_)) | Err(_) => Signing::Lost,
                    }
                }
            }),
            jig_client::Event::Lost { exchange } => self.authentication.remove(exchange).map(|_| Signing::Lost),
            jig_client::Event::Start { .. }
            | jig_client::Event::StartUnlinked { .. }
            | jig_client::Event::SignIn { .. }
            | jig_client::Event::SignOut
            | jig_client::Event::SignOutLost
            | jig_client::Event::Watch { .. }
            | jig_client::Event::Unwatch { .. }
            | jig_client::Event::Read { .. }
            | jig_client::Event::Send { .. }
            | jig_client::Event::Carry { .. }
            | jig_client::Event::Authorize { .. }
            | jig_client::Event::Streamed { .. }
            | jig_client::Event::StreamGone
            | jig_client::Event::Stored
            | jig_client::Event::NotStored
            | jig_client::Event::Up { .. } => None,
        };
        if matches!(event, jig_client::Event::NotStored) {
            self.notices.push(Shown::Terminal(crate::terminal::Notice::StoreFailed));
        }
        self.event(domain::Event::Client { event });
        if let Some((party, credential)) = credential_notice {
            self.notices.push(Shown::Notice(route::Notice::Credential { party, credential }));
        }
        let credential = self.root.client().link().credential.cloned();
        if credential != self.credential && (credential.is_some() || matches!(signing, Some(Signing::SignedOut))) {
            self.shell.keep(credential.clone());
            self.credential = credential;
            self.saving_signing = signing;
        } else if let Some(signing) = signing {
            self.signed(signing);
        }
    }
    fn signed(&mut self, signing: Signing) {
        if let Some(notice) = crate::terminal::native::signing(&signing, self.signing_out) {
            self.notices.push(Shown::Terminal(notice));
        }
        match &signing {
            Signing::SignedIn { .. } => {
                self.seen.signed_in = self.seen.signed_in.saturating_add(1);
                self.notices.push(Shown::Terminal(crate::terminal::Notice::SignedIn));
            }
            Signing::SignedOut => {
                self.signing_out = false;
                self.notices.push(Shown::Terminal(crate::terminal::Notice::SignedOut));
            }
            Signing::Again | Signing::Busy | Signing::Full { .. } | Signing::Lost | Signing::Refused => {
                if self.signing_out {
                    self.signing_out = false;
                    self.event(domain::Event::Client { event: jig_client::Event::SignOutLost });
                }
            }
        }
        self.signing.push(signing);
        self.outputs();
    }
    fn outputs(&mut self) {
        while let Some(request) = self.out.pop() {
            match route::down(request) {
                Down::Shell(jig_client::Request::ServiceCredential { .. }) => {}
                Down::Shell(request) => {
                    if let jig_client::Request::Exchange { exchange, message, .. } = &request {
                        let kind = match message {
                            jig_client::Message::SignInLink(_) | jig_client::Message::SignInService => Some(false),
                            jig_client::Message::SignOut => Some(true),
                            jig_client::Message::Authorize(_)
                            | jig_client::Message::Request(_)
                            | jig_client::Message::Read(_)
                            | jig_client::Message::Watch(_)
                            | jig_client::Message::Unwatch(_)
                            | jig_client::Message::StreamGone(_) => None,
                        };
                        if let Some(kind) = kind {
                            self.authentication.insert(*exchange, kind).expect("bounded sign-in exchanges");
                        }
                    }
                    self.shell.down(request);
                }
                Down::Notice(notice) => self.notices.push(Shown::Notice(notice)),
                Down::Duty(duty) => match duty {
                    Duty::Show { address, .. } => {
                        if self.address != address {
                            self.go(address);
                        }
                    }
                    Duty::SignOut => {
                        self.signing_out = true;
                        self.event(domain::Event::Client { event: jig_client::Event::SignOut });
                    }
                    Duty::Fact(_) => self.seen.facts = self.seen.facts.saturating_add(1),
                },
            }
        }
    }
}
fn deadline(policy: Option<Time>, next: Option<Wall>, now: Time, wall: Wall) -> Option<Time> {
    let view = next.map(|next| {
        now.saturating_add(skein_lib::Duration::from_nanos(next.as_nanos().saturating_sub(wall.as_nanos())))
    });
    [policy, view].into_iter().flatten().min()
}
impl Host for NativeClient {
    fn iterate(&mut self, now: Time, wall: Wall) {
        self.env.now = now;
        self.env.wall = wall;
        self.shell.iterate(now, wall);
        if matches!(self.phase, Phase::Closing { .. }) {
            while self.shell.events().pop().is_some() {}
            return;
        }
        if let Some((credential, records)) = self.shell.loaded() {
            self.loaded(credential, records);
            self.outputs();
        }
        if let Some(ok) = self.shell.kept()
            && let Some(signing) = self.saving_signing.take()
        {
            self.signed(if ok { signing } else { Signing::Lost });
        }
        while let Some(event) = self.shell.events().pop() {
            self.transport(event);
            self.outputs();
        }
        if self.started() {
            domain::fire(&mut self.root, &self.env, &mut self.out);
            self.outputs();
            domain::take(&mut self.root, &self.env, &mut self.out);
            self.outputs();
            view::build(&mut self.view, &self.root, wall, &mut self.patches);
            if !self.patches.is_empty() {
                self.seen.builds = self.seen.builds.saturating_add(1);
            }
            while self.patches.pop().is_some() {
                self.seen.patches = self.seen.patches.saturating_add(1);
            }
            self.dirty = false;
        }
        if !self.loaded
            && !self.shell.work_pending(now)
            && self.shell.next_deadline().is_none()
            && self.shell.submissions().is_empty()
            && self.shell.completions().is_empty()
        {
            self.notices.push(Shown::Terminal(crate::terminal::Notice::DirectoryRefused));
            self.close();
        }
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        self.shell.completions()
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        self.shell.submissions()
    }
    fn work_pending(&self, now: Time) -> bool {
        self.shell.work_pending(now)
            || !self.out.is_empty()
            || (!matches!(self.phase, Phase::Closing { .. }) && self.started() && self.dirty)
    }
    fn next_deadline(&self) -> Option<Time> {
        let policy = [self.shell.next_deadline(), self.next_policy_deadline()].into_iter().flatten().min();
        let next = if matches!(self.phase, Phase::Closing { .. }) { None } else { self.view.next_change() };
        deadline(policy, next, self.env.now, self.env.wall)
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        if matches!(self.phase, Phase::Closing { .. }) { None } else { domain::next_deadline(&self.root) }
    }
    fn is_empty(&self) -> bool {
        matches!(self.phase, Phase::Closing { .. }) && self.shell.is_empty()
    }
    fn worst_case(&self) -> u64 {
        self.memory
    }
    fn operations(&self) -> u32 {
        self.shell.operations()
    }
}

fn kept_key(record: &jig_client::Record, limits: &link::Limits) -> Option<jig_client::Key> {
    use jig_client_protocol_wire as w;
    let mut reader = skein_lib::Reader::new(&record.bytes);
    let kept = w::Kept::decode(&limits.wire, &mut reader).ok()?;
    if !reader.is_empty() {
        return None;
    }
    let mut reader = skein_lib::Reader::new(kept.record());
    let row =
        link::translate::store::restore_table(w::TableEntry::decode(&limits.wire, &mut reader).ok()?, limits).ok()?;
    if !reader.is_empty() || row.key.as_slice() != record.key.as_ref() {
        return None;
    }
    Some(row.request.key)
}
fn service_credential(message: &[u8], limits: &jig_client::Limits) -> Option<(u64, skein_lib::Secret)> {
    use jig_client_protocol_wire as w;
    let limits = link::Limits::new(limits.domain, limits.exchanges).ok()?;
    let mut reader = skein_lib::Reader::new(message);
    let w::KeyedAnswer::FinalAnswer(owned) = w::KeyedAnswer::decode(&limits.wire, &mut reader).ok()? else {
        return None;
    };
    if !reader.is_empty() || owned.owner() != &w::Owner::Jig || owned.version() != w::VERSION {
        return None;
    }
    let mut reader = skein_lib::Reader::new(owned.record());
    let w::Final::Done(done) = w::Final::decode(&limits.wire, &mut reader).ok()? else { return None };
    if !reader.is_empty() {
        return None;
    }
    let w::Outcome::Made(made) = done.into_parts().outcome else { return None };
    let w::Made::Service(service) = made.into_parts().what else { return None };
    let service = service.into_parts();
    if service.credential.expose().is_empty() {
        return None;
    }
    Some((service.party, service.credential))
}

#[cfg(test)]
mod tests;
