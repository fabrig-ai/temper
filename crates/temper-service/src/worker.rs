//! One worker root and its channels, with values below the domain (jig's
//! protocol/service.md, 6).
use crate::{
    identities::IdentityTable,
    settings::{Agent, Files},
    worker_route, worker_settings,
};
use alloc::boxed::Box;
use jig_domain_host as hub;
use jig_protocol_accounts::table::{Scheme, Table, Value};
use jig_protocol_config::{Problems, What};
use jig_protocol_worker_link as link;
use skein_channel::{Lower, LowerEvent};
use skein_io::{self as io, kernel::Fd};
use skein_lib::{Decimal, Env, List, Map, Queue, Time, Token, Wall, Writer, stream};
use smith_host_domain as smith;
use smith_host_protocol as host;
use temper_worker_domain as root;
use worker_settings::problem;

/// The worker's root directory, opened by the shell.
#[derive(Debug)]
pub struct WorkerRuntime {
    pub root: Fd,
    pub path: Box<[u8]>,
}
/// The filesystem's absolute path envelope, including the worker root.
pub(crate) const WORKER_PATH_BYTES: usize = 4096;
#[derive(Debug)]
pub struct Worker {
    pub(super) root: root::Domain,
    pub(crate) env: Env<root::Limits>,
    pub(crate) git: crate::worker_git::Git,
    agent: Agent,
    sources: crate::program_sources::Component,
    link: link::Component,
    link_out: Queue<link::Request>,
    out: Queue<root::Request>,
    pub(crate) events: Queue<root::Event>,
    process_events: Queue<host::ProcessEvent>,
    processes: Map<Token, host::Process>,
    owners: Map<Token, Token>,
    entities: Map<Token, Token>,
    retired: List<Token>,
    pub(crate) values: Table,
    pub(crate) identities: IdentityTable,
    pub(crate) undecodable_values: u64,
    payload: temper_protocol_worker_wire::v1::Limits,
    pub(super) io: Queue<io::Request>,
    connection: Connection,
    socket: Option<Token>,
    prefix: u64,
    next: u64,
    program: Box<[u8]>,
    arguments: Box<[Box<[u8]>]>,
    directory: Option<Fd>,
    path: Box<[u8]>,
    root_close: Option<Token>,
    pub(super) submitted: Queue<io::kernel::Submit>,
    pub(super) started: bool,
    closed: bool,
    stopping: bool,
    pub(super) aborted: Option<()>,
}
// In-process workers keep no operator diagnostics; socket workers own their
// endpoint, bounded notes and service ending together (protocol/README.md, 15).
#[derive(Debug)]
enum Connection {
    InProcess(io::Name),
    Socket(Box<Remote>),
}
#[derive(Debug)]
struct Remote {
    address: core::net::SocketAddr,
    worker: Box<[u8]>,
    notes: Queue<crate::Note>,
    end: Option<crate::End>,
    hello_pending: bool,
    last_refusal: Option<(core::net::SocketAddr, crate::Refusal)>,
}
impl Worker {
    pub fn new(
        settings: &crate::settings::Worker,
        agent: &Agent,
        limits: &crate::limits::Limits,
        files: &Files,
        runtime: WorkerRuntime,
        seed: u64,
    ) -> Result<Self, Problems> {
        if runtime.path.len() > WORKER_PATH_BYTES {
            return Err(problem(b"root", What::Bound));
        }
        let prepared = worker_settings::prepare(settings, agent, limits)?;
        let config = &settings.jig;
        let secret = files.contents(&config.secret).ok_or(problem(b"secret", What::Unreadable))?;
        let bytes = secret.strip_suffix(b"\n").unwrap_or(secret);
        if bytes.len() > usize::try_from(config.limits.link.secret).expect("secret bound") {
            return Err(problem(b"secret", What::Bound));
        }
        let link = link::Component::new(&prepared.link, worker_settings::credential(settings, secret), seed)
            .or(Err(problem(b"limits", What::Bound)))?;
        let room = prepared.room;
        let slots = config.slots;
        let git = crate::worker_git::Git::new(settings, limits, &runtime.path, runtime.root, seed)?;
        let operations = git.component.operations().checked_add(2).ok_or(problem(b"limits", What::Bound))?;
        let bytes = crate::worker_worst_case(settings, agent, limits).ok_or(problem(b"memory_bytes", What::Bound))?;
        if let Err(what) =
            jig_protocol_config::fits(config.memory, &[jig_protocol_config::Share { name: b"worker", bytes }])
        {
            return Err(problem(b"memory_bytes", what));
        }
        Ok(Self {
            git,
            sources: crate::program_sources::Component::new(config.limits.hub.detail_bytes),
            root: root::Domain::new(&prepared.root),
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits: prepared.root },
            agent: Agent {
                programs: agent.programs.clone(),
                endpoints: agent.endpoints.clone(),
                limits: agent.limits,
                bounds: agent.bounds.clone(),
                host_limits: agent.host_limits,
            },
            link,
            link_out: Queue::with_capacity(link::MAX_OUT_DOWN),
            out: Queue::with_capacity(room),
            events: Queue::with_capacity(room),
            process_events: Queue::with_capacity(room),
            processes: Map::with_capacity(slots),
            owners: Map::with_capacity(slots),
            entities: Map::with_capacity(slots.checked_mul(4).expect("process entities")),
            retired: List::with_capacity(slots),
            values: Table::new(
                config.limits.core.accounts.accounts.checked_add(1).expect("account slots"),
                worker_settings::credential_bytes(config.limits.token),
            ),
            identities: IdentityTable::new(config.limits.core.accounts.credentials),
            undecodable_values: 0,
            payload: prepared.payload,
            io: Queue::with_capacity(prepared.lower),
            connection: match prepared.address {
                io::Addr::InProcess(name) => Connection::InProcess(name),
                io::Addr::Ip(address) => Connection::Socket(Box::new(Remote {
                    address,
                    worker: config.name.clone(),
                    notes: Queue::with_capacity(prepared.lower),
                    end: None,
                    hello_pending: false,
                    last_refusal: None,
                })),
            },
            socket: None,
            prefix: prepared.prefix,
            next: 2,
            program: config.agent.program.clone(),
            arguments: config.agent.arguments.clone(),
            directory: Some(runtime.root),
            path: runtime.path,
            root_close: None,
            submitted: Queue::with_capacity(operations),
            started: false,
            closed: false,
            stopping: false,
            aborted: None,
        })
    }
    pub fn configure_program_sources(
        &mut self,
        resources: crate::program_sources::Resources,
    ) -> Result<(), crate::program_sources::Resources> {
        self.sources.configure(resources)
    }
    pub(crate) fn stopping(&self) -> bool {
        self.stopping
    }
    pub(crate) fn end(&self) -> Option<&crate::End> {
        match &self.connection {
            Connection::Socket(remote) => remote.end.as_ref(),
            Connection::InProcess(_) => None,
        }
    }
    pub(crate) fn set_end(&mut self, end: crate::End) {
        let Connection::Socket(remote) = &mut self.connection else { unreachable!("worker alone has a socket") };
        remote.end = Some(end);
    }
    pub(crate) fn notes(&mut self) -> &mut Queue<crate::Note> {
        let Connection::Socket(remote) = &mut self.connection else { unreachable!("worker alone has a socket") };
        &mut remote.notes
    }
    pub fn stop(&mut self) {
        if !self.stopping {
            self.stopping = true;
            self.git_request(temper_worker_protocol_git::Request::Shutdown);
            self.events.push(root::Event::Shutdown);
        }
    }
    /// End immediately, dropping domain work and aborting every owned IO entity.
    pub fn abort(&mut self) {
        if self.aborted.is_some() {
            return;
        }
        self.aborted = Some(());
        self.stopping = true;
        self.git.component.abort(&mut self.git.out);
        self.git_drain();
        for _ in 0..self.events.len() {
            let _event = self.events.pop();
        }
        for _ in 0..self.out.len() {
            let _request = self.out.pop();
        }
        self.link_down(self.env.now, link::Event::Abort);
        if let Some(at) = self.sources.next_deadline() {
            self.sources.fire(at, &mut self.events, &mut self.io);
        }
        for (entity, _) in &self.entities {
            self.io.push(io::Request::Abort { entity: *entity });
        }
    }
    #[must_use]
    pub fn accepts(&self, event: &io::Event) -> bool {
        if self.git.component.accepts(event) {
            return true;
        }
        match crate::workers::owner(event) {
            Some(owner) => {
                self.sources.owns(owner)
                    || owner == Token::new(self.prefix)
                    || self.owners.get(&owner).is_some()
                    || self.entities.get(&owner).is_some()
            }
            None => false,
        }
    }
    pub(crate) fn room(&self) -> bool {
        self.io.room() >= root::max_out(&self.env.limits).checked_mul(link::MAX_OUT_DOWN).expect("worker batch")
            && self.events.room() >= link::MAX_OUT_DOWN
            && self.process_events.room() >= 8
            && match &self.connection {
                Connection::InProcess(_) => true,
                Connection::Socket(remote) => {
                    remote.notes.room()
                        >= root::max_out(&self.env.limits).checked_mul(link::MAX_OUT_DOWN).expect("worker notes batch")
                }
            }
    }
    pub fn up(&mut self, now: Time, mut event: io::Event) {
        if self.git.component.accepts(&event) {
            self.git.component.up(now, event, &mut self.git.out);
            self.git_drain();
            return;
        }
        let owner = crate::workers::owner(&event).expect("routed owner");
        if self.sources.owns(owner) {
            self.sources.from_io(event, &mut self.events, &mut self.io);
            return;
        }
        if owner == Token::new(self.prefix) {
            match event {
                io::Event::Connecting { socket, .. } => {
                    self.socket = Some(socket);
                    if self.aborted.is_some() {
                        self.io.push(io::Request::Abort { entity: socket });
                    }
                }
                io::Event::Connected { .. } => {
                    self.link_down(now, link::Event::Dialled { connection: Token::new(self.prefix) });
                }
                io::Event::Failed { .. } => self.link_down(now, link::Event::Unreachable),
                io::Event::Stream { up, .. } => self.link_down(
                    now,
                    link::Event::Lower { connection: Token::new(self.prefix), event: LowerEvent::Read(up) },
                ),
                io::Event::Output { up, .. } => self.link_down(
                    now,
                    link::Event::Lower { connection: Token::new(self.prefix), event: LowerEvent::Write(up) },
                ),
                io::Event::Closed { .. } => {
                    self.socket = None;
                    self.link_down(now, link::Event::Gone { connection: Token::new(self.prefix) });
                }
                io::Event::Listening { .. }
                | io::Event::Accepted { .. }
                | io::Event::Spawned { .. }
                | io::Event::Exited { .. }
                | io::Event::Usage { .. }
                | io::Event::Window { .. }
                | io::Event::Resized
                | io::Event::Shutdown { .. } => {}
            }
        } else {
            let agent = match self.owners.get(&owner) {
                Some(agent) => *agent,
                None => *self.entities.get(&owner).expect("owned pipe"),
            };
            if let io::Event::Spawned { child, pipes, .. } = &event {
                self.entities.insert(*child, agent).expect("child per process");
                if self.aborted.is_some() {
                    self.io.push(io::Request::Abort { entity: *child });
                }
                for pipe in pipes {
                    self.entities.insert(*pipe, agent).expect("three pipes per process");
                    if self.aborted.is_some() {
                        self.io.push(io::Request::Abort { entity: *pipe });
                    }
                }
            }
            let process = self.processes.get_mut(&agent).expect("owned process");
            if self.owners.get(&owner).is_some() {
                owner_of(&mut event, process.io_owner());
            }
            process.from_io(now, event, &mut self.process_events, &mut self.io);
        }
    }
    fn link_down(&mut self, now: Time, event: link::Event) {
        self.link.down(now, event, &mut self.link_out);
        self.drain_link(now);
    }
    fn drain_link(&mut self, now: Time) {
        for _ in 0..self.link_out.capacity() {
            if self.link_out.is_empty() {
                break;
            }
            let request = self.link_out.pop().expect("link output");
            self.link(now, request);
        }
        self.values.expire(now);
    }
    pub(crate) fn link(&mut self, _now: Time, request: link::Request) {
        match request {
            link::Request::Ready => {
                if let Connection::Socket(remote) = &mut self.connection {
                    remote.hello_pending = true;
                }
                self.events.push(root::Event::Ready);
            }
            link::Request::Lost => {
                if let Connection::Socket(remote) = &mut self.connection {
                    remote.hello_pending = false;
                }
                self.events.push(root::Event::Lost);
            }
            link::Request::GracePassed => self.events.push(root::Event::GracePassed),
            link::Request::Resend { run, attempt, turn } => {
                self.events.push(root::Event::Resend { run, attempt, turn });
            }
            link::Request::RelayCancelled { run, attempt, call } => {
                self.events.push(root::Event::RelayCancelled { run, attempt, call });
            }
            link::Request::Down { down: hub::LinkDown::Assign { assignment }, parts }
                if assignment.assignment.workspace.is_some() =>
            {
                let (items, save) = match worker_route::items(&assignment, &parts, &self.payload) {
                    Ok((items, save)) => (Ok(items), save),
                    Err(why) => (Err(why), None),
                };
                self.events.push(root::Event::Assign { assignment, items, save });
            }
            link::Request::Down { down, .. } => self.events.push(root::Event::Link(down)),
            link::Request::Value { account, generation, expires, value } => {
                let value = match value {
                    link::GrantValue::Credential { scheme: link::Scheme::Bearer, token } => {
                        Value { token, scheme: Scheme::Bearer }
                    }
                    link::GrantValue::Credential { scheme: link::Scheme::Key, token } => {
                        Value { token, scheme: Scheme::Key }
                    }
                    link::GrantValue::Withdrawn { id, bearer } => {
                        Value { token: bearer, scheme: Scheme::Withdrawn { id } }
                    }
                    // An application's grant is a configured git identity.
                    link::GrantValue::Application { secret } => {
                        self.identity(account, generation, &secret, expires);
                        return;
                    }
                };
                self.values.granted(account, generation, value, expires).expect("wire values fit tables");
            }
            link::Request::Dial => {
                let addr = match &self.connection {
                    Connection::InProcess(name) => io::Addr::InProcess(*name),
                    Connection::Socket(remote) => io::Addr::Ip(remote.address),
                };
                self.io.push(io::Request::Connect { owner: Token::new(self.prefix), addr });
            }
            link::Request::Lower { lower, .. } => {
                if let Some(socket) = self.socket {
                    self.io.push(match lower {
                        Lower::Read(down) => io::Request::Stream { stream: socket, down },
                        Lower::Write(down) => io::Request::Output { stream: socket, down },
                        Lower::FinishWrite => io::Request::Stream { stream: socket, down: stream::Down::Finish },
                    });
                }
            }
            link::Request::Close { .. } => {
                if let Some(entity) = self.socket {
                    self.io.push(io::Request::Close { entity });
                }
            }
            link::Request::Abort { .. } => {
                if let Some(entity) = self.socket {
                    self.io.push(io::Request::Abort { entity });
                }
            }
            link::Request::Closed => self.closed = true,
            link::Request::Fact { why } => {
                let reason = match why {
                    skein_channel::Closed::RefusedPeer(code) => Some(crate::Refusal::Peer(code)),
                    skein_channel::Closed::RefusedHere(2) => Some(crate::Refusal::Terms),
                    skein_channel::Closed::RefusedHere(3) => Some(crate::Refusal::Malformed),
                    skein_channel::Closed::RefusedHere(_)
                    | skein_channel::Closed::Owner
                    | skein_channel::Closed::Framing
                    | skein_channel::Closed::Stream
                    | skein_channel::Closed::Truncated => None,
                };
                if let Some(reason) = reason
                    && let Connection::Socket(remote) = &mut self.connection
                    && remote.last_refusal != Some((remote.address, reason))
                {
                    remote.last_refusal = Some((remote.address, reason));
                    remote.notes.push(crate::Note::Refused {
                        worker: remote.worker.clone(),
                        address: Box::new(remote.address),
                        reason,
                    });
                }
            }
        }
    }
    fn identity(&mut self, account: u32, generation: u64, secret: &skein_lib::Secret, expires: Time) {
        let value = worker_route::identity(secret, &self.payload);
        let undecodable = value.is_none();
        let unfit = self.identities.granted(account, generation, value, expires).is_err();
        if undecodable || unfit {
            self.undecodable_values = self.undecodable_values.saturating_add(1);
        }
    }
    pub(super) fn step(&mut self, event: root::Event) {
        root::step(&mut self.root, &self.env, event, &mut self.out);
        self.drain_root();
    }
    fn drain_root(&mut self) {
        for _ in 0..self.out.capacity() {
            let Some(request) = self.out.pop() else { break };
            self.route(request);
        }
        assert!(self.out.is_empty(), "each payload hand-out is answered in this iteration");
    }
    pub(crate) fn route(&mut self, request: root::Request) {
        match request {
            root::Request::Source { owner, name, operation, deadline } => {
                let io_owner = Token::new(self.prefix.checked_add(self.next).expect("source owner range"));
                self.next = self.next.checked_add(1).expect("source owner sequence");
                assert!(self.next < (1_u64 << 48_u32), "worker owner range");
                self.sources.start(
                    owner,
                    io_owner,
                    name,
                    operation,
                    self.directory.expect("worker root"),
                    deadline,
                    &mut self.events,
                    &mut self.io,
                );
            }
            root::Request::SourceFault => self.link_down(self.env.now, link::Event::SourceFault),
            root::Request::Hello(hello) => {
                self.link_down(self.env.now, link::Event::Hello { hello });
                if let Connection::Socket(remote) = &mut self.connection
                    && remote.hello_pending
                {
                    remote.hello_pending = false;
                    remote.last_refusal = None;
                    remote
                        .notes
                        .push(crate::Note::Linked { worker: remote.worker.clone(), address: Box::new(remote.address) });
                }
            }
            root::Request::Link(up) => self.link_down(self.env.now, link::Event::Up { up, parts: Box::default() }),
            root::Request::Answer { run, attempt, answer, work, preparation } => {
                let parts = worker_route::answer_parts(&answer, &work, preparation.as_ref());
                self.link_down(
                    self.env.now,
                    link::Event::Up { up: hub::LinkUp::Answer { run, attempt, answer }, parts },
                );
            }
            root::Request::Io { owner, op, deadline } => {
                let value = match crate::worker_git::identity(&op) {
                    Some(number) => self.identities.newest(number, self.env.now),
                    None => None,
                };
                match crate::worker_git::pick(value, &op) {
                    Ok(identity) => {
                        self.git_request(temper_worker_protocol_git::Request::Io { owner, op, deadline, identity });
                    }
                    Err(done) => {
                        self.git.out.events.push(temper_worker_protocol_git::Event::Done { owner, done });
                        self.git_drain();
                    }
                }
            }
            root::Request::CancelIo { owner } => {
                self.git_request(temper_worker_protocol_git::Request::CancelIo { owner });
            }
            root::Request::CancelRelay { run, attempt, call } => {
                self.link_down(self.env.now, link::Event::CancelRelay { run, attempt, call });
            }
            root::Request::Stop => self.link_down(self.env.now, link::Event::Stop),
            root::Request::Process(request) => self.process(*request),
            root::Request::Decode { agent, run, start } => {
                let start = worker_route::decode(run, *start, &self.agent);
                root::step(&mut self.root, &self.env, root::Event::Decoded { agent, start }, &mut self.out);
            }
            root::Request::Encode { owner, value } => {
                let encoded = worker_route::encode(value, &self.agent);
                root::step(&mut self.root, &self.env, root::Event::Encoded { owner, encoded }, &mut self.out);
            }
            root::Request::Fact { owner, body } => {
                let activity = worker_route::activity(&body, &self.agent);
                root::step(&mut self.root, &self.env, root::Event::Activity { owner, activity }, &mut self.out);
            }
        }
    }
    fn process(&mut self, request: smith::process::Request) {
        match request {
            smith::process::Request::Spawn { owner, deadline, .. } => self.process_spawn(owner, deadline),
            smith::process::Request::Send { owner, message, .. } => self.process_send(owner, message),
            smith::process::Request::Signal { owner, signal, .. } => self.process_signal(owner, signal),
            smith::process::Request::Read { .. } => Self::process_read(),
            smith::process::Request::Wait { .. } => Self::process_wait(),
            smith::process::Request::Reap { .. } => Self::process_reap(),
        }
    }
    fn process_spawn(&mut self, owner: Token, deadline: Time) {
        let mut process = host::Process::new(
            owner,
            &self.agent.limits,
            self.env.limits.agent.detail_bytes,
            self.agent.endpoints.clone(),
        )
        .expect("startup checks process limits");
        let mut spawned = Queue::with_capacity(1);
        process.spawn(
            host::Launch {
                program: self.program.clone(),
                arguments: self.arguments.clone(),
                environment: Box::default(),
                root: self.directory.expect("prepared worker root"),
                directory: b".".as_slice().into(),
            },
            deadline,
            &mut spawned,
        );
        let global = Token::new(self.prefix.checked_add(self.next).expect("worker owner range"));
        self.next = self.next.checked_add(1).expect("worker owner sequence");
        assert!(self.next < (1_u64 << 48_u32), "worker owner range");
        self.owners.insert(global, owner).expect("one owner per process");
        self.processes.insert(owner, process).expect("root bounds processes");
        let mut request = spawned.pop().expect("one spawn");
        if let io::Request::Spawn { owner, .. } = &mut request {
            *owner = global;
        }
        self.io.push(request);
    }
    fn process_send(&mut self, owner: Token, message: smith::Down) {
        let values = match &message {
            smith::Down::Start { start, .. } => start_values(&self.values, start, &self.path, self.env.now),
            smith::Down::Grant { grant } => grant_value(&self.values, *grant, self.env.now),
            smith::Down::Message { .. }
            | smith::Down::Answer { .. }
            | smith::Down::Acknowledge { .. }
            | smith::Down::Cancel => Some(host::Values { paths: Box::default(), credentials: Box::default() }),
        };
        let Some(values) = values else {
            self.events.push(root::Event::Process(Box::new(smith::process::Event::Unsent { owner })));
            return;
        };
        let process = self.processes.get_mut(&owner).expect("root owns process");
        let token = owner;
        let sent = match message {
            smith::Down::Start { start, window } => {
                process.send_start(start, window, values, token, &mut self.process_events, &mut self.io)
            }
            smith::Down::Grant { grant } => process.send_grant(
                grant,
                values.credentials.into_iter().next().expect("one value"),
                token,
                &mut self.process_events,
                &mut self.io,
            ),
            smith::Down::Message { name, label, text } => {
                process.send_message(name, label, text, token, &mut self.process_events, &mut self.io)
            }
            smith::Down::Answer { call, reply } => {
                process.send_reply(call, reply, token, &mut self.process_events, &mut self.io)
            }
            smith::Down::Acknowledge { turn } => {
                process.send_acknowledge(turn, token, &mut self.process_events, &mut self.io)
            }
            smith::Down::Cancel => process.send_cancel(token, &mut self.process_events, &mut self.io),
        };
        if sent.is_err() {
            self.events.push(root::Event::Process(Box::new(smith::process::Event::Unsent { owner })));
        }
    }
    fn process_signal(&mut self, owner: Token, signal: smith::Signal) {
        let signal = match signal {
            smith::Signal::Terminate => io::kernel::Signal::Terminate,
            smith::Signal::Kill => io::kernel::Signal::Kill,
        };
        self.processes.get(&owner).expect("root owns process").signal(signal, &mut self.io);
        self.events.push(root::Event::Process(Box::new(smith::process::Event::Signalled { owner })));
    }
    // Smith's process adapter keeps reads, exit waits and reaping active.
    fn process_read() {}
    fn process_wait() {}
    fn process_reap() {}
    pub fn iterate(&mut self, now: Time, wall: Wall) {
        self.env.now = now;
        self.env.wall = wall;
        self.sources.fire(now, &mut self.events, &mut self.io);
        if !self.started {
            self.started = true;
        }
        if !self.git.started && !self.stopping {
            self.git.started = true;
            self.git.component.start(now, &mut self.git.out);
            self.git_drain();
        }
        if self.git.component.is_due(now) {
            self.git.component.fire(now, &mut self.git.out);
            self.git_drain();
        }
        if self.git.ready {
            self.link.fire(now, &mut self.link_out);
            self.drain_link(now);
        }
        let mut agents = List::with_capacity(self.processes.len());
        for (agent, _) in &self.processes {
            agents.push(*agent).expect("process count");
        }
        for agent in &agents {
            self.processes.get_mut(agent).expect("owned process").fire(now, &mut self.process_events, &mut self.io);
        }
        for _ in 0..self.process_events.len() {
            let event = self.process_events.pop().expect("process events");
            match &event {
                host::ProcessEvent::Unspawned { agent, .. } | host::ProcessEvent::Reaped { agent, .. } => {
                    self.retired.push(*agent).expect("retired processes");
                }
                host::ProcessEvent::Spawned { .. }
                | host::ProcessEvent::Exited { .. }
                | host::ProcessEvent::Channel { .. } => {}
            }
            if self.aborted.is_none()
                && let Some(event) = worker_route::process(event)
            {
                self.events.push(event);
            }
        }
        if self.aborted.is_some() {
            for _ in 0..self.events.len() {
                let _event = self.events.pop();
            }
        }
        for _ in 0..self.events.capacity() {
            if !self.room() {
                break;
            }
            let Some(event) = self.events.pop() else { break };
            self.step(event);
        }
        if self.aborted.is_none() && self.room() && self.root.is_due(now) {
            root::fire(&mut self.root, &self.env, &mut self.out);
            self.drain_root();
        }
        if self.aborted.is_none() && self.room() && self.root.is_ready() {
            root::resume(&mut self.root, &self.env, &mut self.out);
            self.drain_root();
        }
        for index in 0..self.retired.len() {
            let agent = *self.retired.get(index).expect("retired agent");
            drop(self.processes.remove(&agent));
            let mut owners = List::with_capacity(self.owners.len());
            for (owner, found) in &self.owners {
                if *found == agent {
                    owners.push(*owner).expect("owner count");
                }
            }
            for owner in &owners {
                let _owner = self.owners.remove(owner);
            }
            let mut entities = List::with_capacity(self.entities.len());
            for (owner, found) in &self.entities {
                if *found == agent {
                    entities.push(*owner).expect("entity count");
                }
            }
            for owner in &entities {
                let _entity = self.entities.remove(owner);
            }
        }
        for agent in &agents {
            if let Some(process) = self.processes.get_mut(agent) {
                for _ in 0..self.out.capacity() {
                    let Some(request) = process.next_channel_down() else { break };
                    process.channel_down(request, &mut self.io);
                }
            }
        }
        self.retired.clear();
        self.root.reclaim();
        self.git.component.reclaim();
        self.close_root();
    }
    #[must_use]
    pub fn next_deadline(&self) -> Option<Time> {
        if !self.started || self.aborted.is_some() {
            return None;
        }
        let link = if self.git.ready { self.link.next_deadline() } else { None };
        let mut next = [
            self.root.next_deadline(),
            link,
            self.values.next_expiry(),
            self.git.component.next_deadline(),
            self.sources.next_deadline(),
        ]
        .into_iter()
        .flatten()
        .min();
        for (_, process) in &self.processes {
            if let Some(at) = process.next_deadline() {
                next = Some(match next {
                    Some(next) => next.min(at),
                    None => at,
                });
            }
        }
        next
    }
    #[must_use]
    pub fn work_pending(&self, now: Time) -> bool {
        !self.started
            || !self.io.is_empty()
            || !self.events.is_empty()
            || !self.process_events.is_empty()
            || !self.submitted.is_empty()
            || (self.aborted.is_none() && self.root.is_ready())
            || match self.next_deadline() {
                Some(at) => at <= now,
                None => false,
            }
    }
    pub(super) fn closed(&self) -> bool {
        self.closed && self.sources.settled() && self.processes.is_empty() && self.io.is_empty()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.closed() && self.directory.is_none() && self.submitted.is_empty() && self.git.component.is_empty()
    }
    pub(super) fn close_root(&mut self) {
        if self.closed()
            && self.git.component.is_empty()
            && self.root_close.is_none()
            && let Some(fd) = self.directory
        {
            self.root_close = Some(Token::new(self.prefix.checked_add(1).expect("root close")));
            self.submitted.push(io::kernel::Submit {
                op: Token::new(self.prefix.checked_add(1).expect("root close")),
                kind: io::kernel::Op::Close { fd },
            });
        }
    }
    pub fn complete(&mut self, now: Time, complete: io::kernel::Complete) -> Result<(), io::kernel::Complete> {
        if self.git.component.owns(complete.op) {
            self.git.component.complete(now, complete, &mut self.git.out);
            self.git_drain();
            return Ok(());
        }
        if complete.op == Token::new(self.prefix.checked_add(1).expect("root close")) {
            assert!(self.root_close.is_some(), "the worker closes its own descriptor");
            self.directory = None;
            self.root_close = None;
            Ok(())
        } else {
            Err(complete)
        }
    }
    pub fn io(&mut self) -> &mut Queue<io::Request> {
        &mut self.io
    }
    pub fn submitted(&mut self) -> &mut Queue<io::kernel::Submit> {
        &mut self.submitted
    }
    #[must_use]
    pub fn refused(&self) -> Option<&Problems> {
        self.git.refused.as_ref()
    }
    #[must_use]
    pub fn operations(&self) -> u32 {
        worker_settings::operations(self.git.component.limits()).expect("checked worker operations")
    }
}
pub(crate) fn grant_value(table: &Table, grant: smith::Grant, now: Time) -> Option<host::Values> {
    let value = account(table, grant, now)?;
    Some(host::Values { paths: Box::default(), credentials: Box::new([value]) })
}
fn account(table: &Table, grant: smith::Grant, now: Time) -> Option<smith_protocol_channel::GrantValue> {
    match grant.form {
        smith::Form::Program(name) => return Some(smith_protocol_channel::GrantValue::Program(name.as_bytes().into())),
        smith::Form::Credential(_) | smith::Form::Withdrawn => {}
    }
    let value = table.value(grant.account, grant.generation, now)?;
    let actual = match &value.scheme {
        Scheme::Bearer => smith::Form::Credential(smith::Scheme::Bearer),
        Scheme::Key => smith::Form::Credential(smith::Scheme::Key),
        Scheme::Withdrawn { .. } => smith::Form::Withdrawn,
    };
    if grant.form != actual {
        return None;
    }
    let token = value.token.expose().into();
    Some(match &value.scheme {
        Scheme::Bearer => smith_protocol_channel::GrantValue::Bearer(token),
        Scheme::Key => smith_protocol_channel::GrantValue::Key(token),
        Scheme::Withdrawn { id } => {
            smith_protocol_channel::GrantValue::Withdrawn { account_id: id.expose().into(), bearer: token }
        }
    })
}
pub(crate) fn start_values(table: &Table, start: &smith::Start, root: &[u8], now: Time) -> Option<host::Values> {
    let mut values = List::with_capacity(u32::try_from(start.grants.len()).ok()?);
    for grant in &start.grants {
        values.push(account(table, *grant, now)?).ok()?;
    }
    let mut paths = List::with_capacity(u32::try_from(start.directories.len()).ok()?);
    for directory in &start.directories {
        let token = Decimal::of(directory.root.raw());
        let capacity = root
            .len()
            .checked_add(b"/workspaces/".len())?
            .checked_add(1)?
            .checked_add(token.as_bytes().len())?
            .checked_add(directory.name.len())?;
        let mut path = Writer::new(capacity);
        path.put(root).ok()?;
        path.put(b"/workspaces/").ok()?;
        path.put(token.as_bytes()).ok()?;
        path.put(b"/").ok()?;
        path.put(&directory.name).ok()?;
        paths.push(path.finish()).ok()?;
    }
    Some(host::Values { paths: paths.into_boxed(), credentials: values.into_boxed() })
}
fn owner_of(event: &mut io::Event, new: Token) {
    match event {
        io::Event::Spawned { owner, .. }
        | io::Event::Exited { owner, .. }
        | io::Event::Failed { owner, .. }
        | io::Event::Closed { owner } => *owner = new,
        io::Event::Listening { .. }
        | io::Event::Accepted { .. }
        | io::Event::Connecting { .. }
        | io::Event::Connected { .. }
        | io::Event::Stream { .. }
        | io::Event::Output { .. }
        | io::Event::Usage { .. }
        | io::Event::Window { .. }
        | io::Event::Resized
        | io::Event::Shutdown { .. } => {}
    }
}

/// Additional retained data for a socket worker; in-process envelopes stay fixed.
pub(crate) fn remote_worst_case(name: u32, room: u32) -> Option<u64> {
    u64::try_from(size_of::<Remote>())
        .ok()?
        .checked_add(u64::from(name))?
        .checked_add(Queue::<crate::Note>::worst_case(room)?)?
        .checked_add(
            u64::from(room)
                .checked_mul(u64::from(name).checked_add(u64::try_from(size_of::<core::net::SocketAddr>()).ok()?)?)?,
        )
}
