//! Accepted streams, the workers' component and its typed root handoff
//! (jig's protocol/channel.md, 10.2).
use crate::{
    assignment::Encoding,
    settings::{Files, Forge},
    values::Values,
};
use alloc::boxed::Box;
use jig_domain_host as host;
use jig_protocol_config::{self as config, Path, Problems, What};
use jig_protocol_engine_workers as kit;
use jig_protocol_worker_wire::Part;
use skein_channel::{Lower, LowerEvent};
use skein_io::{self as io};
use skein_lib::{List, Map, Queue, Reader, Time, Token, stream};
use temper_engine_domain as root;
use temper_protocol_worker_wire::v1 as wire;
const LISTENERS: u64 = crate::tokens::WORKER_LISTENERS;
const CONNECTIONS: u64 = crate::tokens::WORKER_CONNECTIONS;
#[derive(Debug)]
pub struct Workers {
    /// Bound address awaiting the system's note routing.
    pub(crate) listening: Option<core::net::SocketAddr>,
    pub(crate) component: kit::Component,
    out: Queue<kit::Request>,
    pub(crate) io: Queue<io::Request>,
    pub(crate) listeners: List<(Token, Option<Token>)>,
    pub(crate) sockets: Map<Token, Token>,
    next: u64,
    pub(crate) opened: bool,
    ending: Option<Closing>,
    pub(crate) component_closed: bool,
    pub(crate) encoding: Encoding,
    pub(crate) forges: Box<[Forge]>,
    pub(crate) bad_parts: u64,
}
fn invalid(what: What) -> Problems {
    let mut found = Problems::empty();
    found.push(Path::root().member(b"engine").member(b"hosts"), what);
    found
}
impl Workers {
    pub fn new(
        config: &config::Engine,
        files: &Files,
        encoding: Encoding,
        forges: Box<[Forge]>,
        limits: &kit::Limits,
        room: u32,
    ) -> Result<Self, Problems> {
        let mut workers = List::with_capacity(limits.names);
        for worker in &config.hosts.workers {
            let digest = match config::digest(files.contents(&worker.digest).ok_or(invalid(What::Unreadable))?) {
                Ok(digest) => digest,
                Err(what) => return Err(invalid(what)),
            };
            workers.push(kit::Worker { name: worker.name.clone(), digest }).or(Err(invalid(What::Bound)))?;
        }
        let component = kit::Component::new(limits, workers.into_boxed(), temper_protocol_worker_wire::PAYLOAD_VERSION)
            .or(Err(invalid(What::Bound)))?;
        let count = u32::try_from(config.hosts.listener.len()).or(Err(invalid(What::Bound)))?;
        if count == 0 || count > config.limits.io.sockets {
            return Err(invalid(What::Bound));
        }
        let mut listeners = List::with_capacity(count);
        let mut io = Queue::with_capacity(room);
        for (index, address) in config.hosts.listener.iter().enumerate() {
            let config::Address::Addr(address) = address else {
                return Err(invalid(What::Form));
            };
            let owner = Token::new(
                LISTENERS.checked_add(u64::try_from(index).expect("listener count")).expect("listener owner"),
            );
            io.push(io::Request::Listen { owner, addr: *address });
            listeners.push((owner, None)).expect("listener count");
        }
        Ok(Self {
            listening: None,
            component,
            out: Queue::with_capacity(kit::MAX_OUT_DOWN),
            io,
            listeners,
            sockets: Map::with_capacity(limits.workers.checked_add(limits.opening).expect("connections")),
            next: 1,
            opened: false,
            ending: None,
            component_closed: false,
            encoding,
            forges,
            bad_parts: 0,
        })
    }
    #[must_use]
    pub fn next_deadline(&self) -> Option<Time> {
        self.component.next_deadline()
    }
    #[must_use]
    pub fn opened(&self) -> bool {
        self.opened
    }
    #[must_use]
    pub fn batch_room(&self, batch: u32) -> bool {
        self.io.room() >= batch.checked_mul(kit::MAX_OUT_DOWN).expect("worker route room")
    }
    #[must_use]
    pub fn room(&self) -> bool {
        self.io.room() >= kit::MAX_OUT_DOWN
    }
    #[must_use]
    pub fn work_pending(&self, now: Time) -> bool {
        !self.io.is_empty()
            || !self.out.is_empty()
            || match self.next_deadline() {
                Some(deadline) => deadline <= now,
                None => false,
            }
    }
    pub fn down(&mut self, now: Time, event: kit::Event, events: &mut Queue<root::Event>) {
        self.component.down(now, event, &mut self.out);
        self.drain(events);
    }
    pub fn assignment(
        &mut self,
        now: Time,
        channel: Token,
        value: root::Assignment,
        values: &Values,
        events: &mut Queue<root::Event>,
    ) {
        let (task, attempt) = (value.task, value.attempt);
        match self.encoded_assignment(now, channel, value, values) {
            Some(event) => self.down(now, event, events),
            None => events.push(root::Event::Worker(root::WorkerInput::Link {
                channel,
                link: root::Link::Up {
                    up: host::LinkUp::Answer {
                        run: Token::new(task),
                        attempt: Token::new(attempt),
                        answer: host::Answer {
                            read: None,
                            refused: Box::default(),
                            turns: 0,
                            spent: 0,
                            ending: host::Ending::Refused(host::Refusal::Invalid(host::Invalid::Start {
                                cause: host::StartCause::Other,
                                why: b"assignment cannot encode".as_slice().into(),
                            })),
                        },
                    },
                    parts: root::Parts::none(),
                },
            })),
        }
    }
    #[must_use]
    pub fn encoded_assignment(
        &self,
        now: Time,
        channel: Token,
        value: root::Assignment,
        values: &Values,
    ) -> Option<kit::Event> {
        crate::assignment::down(value, &self.forges, &self.encoding, values, channel, now)
    }
    pub fn send(
        &mut self,
        now: Time,
        channel: Token,
        down: host::LinkDown,
        values: &Values,
        events: &mut Queue<root::Event>,
    ) {
        self.down(now, delivery(now, channel, down, values), events);
    }
    pub fn up(&mut self, now: Time, event: io::Event, events: &mut Queue<root::Event>) -> bool {
        let Some(owner) = owner(&event) else {
            return false;
        };
        match event {
            io::Event::Listening { listener, addr, .. } => {
                self.listening = addr.socket();
                for index in 0..self.listeners.len() {
                    let (name, slot) = self.listeners.get_mut(index).expect("listener index");
                    if *name == owner {
                        *slot = Some(listener);
                    }
                }
                self.opened = listeners_open(&self.listeners);
                self.late_listener(listener);
            }
            io::Event::Accepted { socket, .. } => {
                let connection = Token::new(CONNECTIONS.checked_add(self.next).expect("connection owner"));
                self.next = self.next.checked_add(1).expect("connection owner sequence");
                if self.sockets.insert(connection, socket).is_err() {
                    self.io.push(io::Request::Reject { socket });
                } else {
                    self.down(now, kit::Event::Accepted { connection }, events);
                }
            }
            io::Event::Stream { up, .. } => {
                self.down(now, kit::Event::Lower { connection: owner, event: LowerEvent::Read(up) }, events);
            }
            io::Event::Output { up, .. } => {
                self.down(now, kit::Event::Lower { connection: owner, event: LowerEvent::Write(up) }, events);
            }
            io::Event::Closed { .. } => {
                if self.listener_closed(owner) {
                    return self.ending.is_some();
                }
                let _socket = self.sockets.remove(&owner);
                self.down(now, kit::Event::Gone { connection: owner }, events);
            }
            io::Event::Failed { .. }
            | io::Event::Connecting { .. }
            | io::Event::Connected { .. }
            | io::Event::Spawned { .. }
            | io::Event::Exited { .. }
            | io::Event::Usage { .. }
            | io::Event::Resized
            | io::Event::Window { .. }
            | io::Event::Shutdown { .. } => return false,
        }
        true
    }
    fn drain(&mut self, events: &mut Queue<root::Event>) {
        for _ in 0..self.out.len() {
            match self.out.pop().expect("worker requests") {
                kit::Request::Take { connection } => self.io.push(io::Request::Bind {
                    socket: *self.sockets.get(&connection).expect("accepted socket"),
                    owner: connection,
                }),
                kit::Request::TurnAway { connection } => {
                    if let Some(socket) = self.sockets.remove(&connection) {
                        self.io.push(io::Request::Reject { socket });
                    }
                }
                kit::Request::Close { connection } => {
                    if let Some(socket) = self.sockets.get(&connection) {
                        self.io.push(io::Request::Close { entity: *socket });
                    }
                }
                kit::Request::Abort { connection } => {
                    if let Some(socket) = self.sockets.get(&connection) {
                        self.io.push(io::Request::Abort { entity: *socket });
                    }
                }
                kit::Request::Lower { connection, lower } => {
                    if let Some(socket) = self.sockets.get(&connection) {
                        self.io.push(match lower {
                            Lower::Read(down) => io::Request::Stream { stream: *socket, down },
                            Lower::Write(down) => io::Request::Output { stream: *socket, down },
                            Lower::FinishWrite => io::Request::Stream { stream: *socket, down: stream::Down::Finish },
                        });
                    }
                }
                kit::Request::Hello { channel, worker, hello } => {
                    events.push(root::Event::Worker(root::WorkerInput::Link {
                        channel,
                        link: root::Link::Hello { worker, hello },
                    }));
                }
                kit::Request::Up { channel, up, parts } => {
                    self.received(channel, up, &parts, events);
                }
                kit::Request::Lost { channel } => {
                    events.push(root::Event::Worker(root::WorkerInput::Link { channel, link: root::Link::Lost }));
                }
                kit::Request::Closed => self.component_closed = true,
            }
        }
    }
    pub fn iterate(&mut self, now: Time, events: &mut Queue<root::Event>) {
        if self.room() && events.room() >= kit::MAX_OUT_FIRE {
            self.component.fire(now, &mut self.out);
            self.drain(events);
        }
    }
    pub fn abort(&mut self, now: Time, events: &mut Queue<root::Event>) {
        if self.ending != Some(Closing::Abort) {
            self.ending = Some(Closing::Abort);
            for (_, listener) in &self.listeners {
                if let Some(listener) = listener {
                    self.io.push(io::Request::Abort { entity: *listener });
                }
            }
            self.down(now, kit::Event::Abort, events);
        }
    }
}
fn listeners_open(listeners: &List<(Token, Option<Token>)>) -> bool {
    for (_, entity) in listeners {
        if entity.is_none() {
            return false;
        }
    }
    true
}
#[must_use]
pub fn owner(event: &io::Event) -> Option<Token> {
    match event {
        io::Event::Listening { owner, .. }
        | io::Event::Accepted { owner, .. }
        | io::Event::Connecting { owner, .. }
        | io::Event::Connected { owner }
        | io::Event::Stream { owner, .. }
        | io::Event::Output { owner, .. }
        | io::Event::Spawned { owner, .. }
        | io::Event::Exited { owner, .. }
        | io::Event::Usage { owner, .. }
        | io::Event::Window { owner, .. }
        | io::Event::Failed { owner, .. }
        | io::Event::Closed { owner } => Some(*owner),
        io::Event::Resized | io::Event::Shutdown { .. } => None,
    }
}

fn delivery(now: Time, channel: Token, down: host::LinkDown, values: &Values) -> kit::Event {
    let grants = match &down {
        host::LinkDown::Assign { assignment } => values.grants(&assignment.assignment.grants, now),
        host::LinkDown::Grant { grant, .. } => values.grants(::core::slice::from_ref(grant), now),
        host::LinkDown::Sources { .. }
        | host::LinkDown::Check { .. }
        | host::LinkDown::SignOut { .. }
        | host::LinkDown::Message { .. }
        | host::LinkDown::Relayed { .. }
        | host::LinkDown::TurnKept { .. }
        | host::LinkDown::Acknowledge { .. }
        | host::LinkDown::Cancel { .. } => Box::default(),
    };
    kit::Event::Down { channel, down, parts: Box::default(), values: grants }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Closing {
    Drain,
    Abort,
}
impl Workers {
    /// Decode application records in this step, discarding and counting a bad set.
    pub fn received(&mut self, channel: Token, up: host::LinkUp, records: &[Part], events: &mut Queue<root::Event>) {
        let parts = match parts(&up, records, &self.encoding.limits) {
            Some(parts) => parts,
            None => {
                self.bad_parts = self.bad_parts.saturating_add(1);
                root::Parts::none()
            }
        };
        events.push(root::Event::Worker(root::WorkerInput::Link { channel, link: root::Link::Up { up, parts } }));
    }
    fn listener_closed(&mut self, owner: Token) -> bool {
        for index in 0..self.listeners.len() {
            let (name, listener) = self.listeners.get_mut(index).expect("listener index");
            if *name == owner {
                *listener = None;
                return true;
            }
        }
        false
    }
    fn late_listener(&mut self, listener: Token) {
        match self.ending {
            Some(Closing::Abort) => self.io.push(io::Request::Abort { entity: listener }),
            Some(Closing::Drain) => self.io.push(io::Request::Close { entity: listener }),
            None => {}
        }
    }
    /// Close listeners and drain the channel component.
    pub fn close(&mut self, now: Time, events: &mut Queue<root::Event>) {
        self.ending = Some(Closing::Drain);
        for (_, listener) in &self.listeners {
            if let Some(listener) = listener {
                self.io.push(io::Request::Close { entity: *listener });
            }
        }
        self.down(now, kit::Event::Close, events);
    }
    /// Every listener, accepted socket and queued request has closed.
    #[must_use]
    pub fn closed(&self) -> bool {
        if !self.component_closed || !self.sockets.is_empty() || !self.io.is_empty() {
            return false;
        }
        for (_, listener) in &self.listeners {
            if listener.is_some() {
                return false;
            }
        }
        true
    }
}
/// Decode the answer's named parts; any missing or bad part discards all parts.
#[must_use]
pub fn parts(up: &host::LinkUp, parts: &[Part], limits: &wire::Limits) -> Option<root::Parts> {
    let (work, preparation) = match up {
        host::LinkUp::Answer { answer, .. } => match &answer.ending {
            host::Ending::Ended { work, .. } | host::Ending::Parked { work } => (Some(work), None),
            host::Ending::Failed { work, failure, .. } => (
                Some(work),
                match failure {
                    host::Failure::Unprepared(host::Preparation::Permanent { resource }) => *resource,
                    host::Failure::Unprepared(host::Preparation::Transient)
                    | host::Failure::Run(_)
                    | host::Failure::Agent(_)
                    | host::Failure::Cancelled(_) => None,
                },
            ),
            host::Ending::Refused(_) => (None, None),
        },
        host::LinkUp::Logins { .. }
        | host::LinkUp::SignedOut { .. }
        | host::LinkUp::Piece { .. }
        | host::LinkUp::Activity { .. }
        | host::LinkUp::Unauthorized { .. }
        | host::LinkUp::Limited { .. }
        | host::LinkUp::AccountRefused { .. }
        | host::LinkUp::Turn { .. }
        | host::LinkUp::Call { .. } => (None, None),
    };
    let left = match work {
        Some(work) => left(work.left, parts, limits).ok()?,
        None => None,
    };
    let saved = match work {
        Some(work) => saved(work.saved, parts, limits).ok()?,
        None => None,
    };
    let preparation = preparation_part(preparation, parts, limits).ok()?;
    Some(crate::translate::parts(left, saved, preparation))
}
fn bytes(token: Token, parts: &[Part]) -> Option<&[u8]> {
    for part in parts {
        if part.token == token {
            return Some(&part.bytes);
        }
    }
    None
}
fn left(token: Option<Token>, parts: &[Part], limits: &wire::Limits) -> Result<Option<wire::Left>, ()> {
    let Some(token) = token else {
        return Ok(None);
    };
    let mut reader = Reader::new(bytes(token, parts).ok_or(())?);
    let value = wire::Left::decode(limits, &mut reader).or(Err(()))?;
    if !reader.is_empty() {
        return Err(());
    }
    Ok(Some(value))
}
fn saved(token: Option<Token>, parts: &[Part], limits: &wire::Limits) -> Result<Option<wire::Saved>, ()> {
    let Some(token) = token else {
        return Ok(None);
    };
    let mut reader = Reader::new(bytes(token, parts).ok_or(())?);
    let value = wire::Saved::decode(limits, &mut reader).or(Err(()))?;
    if !reader.is_empty() {
        return Err(());
    }
    Ok(Some(value))
}
fn preparation_part(
    token: Option<Token>,
    parts: &[Part],
    limits: &wire::Limits,
) -> Result<Option<wire::Preparation>, ()> {
    let Some(token) = token else {
        return Ok(None);
    };
    let mut reader = Reader::new(bytes(token, parts).ok_or(())?);
    let value = wire::Preparation::decode(limits, &mut reader).or(Err(()))?;
    if !reader.is_empty() {
        return Err(());
    }
    Ok(Some(value))
}
/// Channel bounds, with application parts measured from temper's codecs.
#[must_use]
pub fn limits(settings: &crate::settings::Engine) -> Option<kit::Limits> {
    let jig = &settings.jig.limits;
    let wire = crate::translate::limits(&settings.limits)?;
    let core = jig.core;
    let hub = jig.hub;
    let link = jig.link;
    let ceilings = jig_protocol_worker_wire::CEILINGS;
    let sizes = jig_protocol_worker_wire::sizes::Sizes {
        program_sources: core.fleet.program_sources,
        piece: hub.piece_bytes,
        charter: u32::try_from(hub.charter_bytes).ok()?,
        transcript: u32::try_from(hub.transcript_bytes).ok()?,
        turn: u32::try_from(hub.turn_bytes).ok()?,
        message: core.tasks.message_bytes,
        sender: ceilings.message_sender,
        call_name: u32::try_from(core.fleet.call_name_bytes).ok()?,
        tool_name: ceilings.call_tool,
        call_input: core.call_answer_bytes,
        answer: core.call_answer_bytes,
        result: u32::try_from(hub.outcome_bytes).ok()?,
        detail: hub.detail_bytes,
        grant: crate::worker_settings::grant_bytes(
            crate::values::credential_bytes(&settings.jig),
            u32::try_from(wire::Identity::worst_case_bytes(&wire)?).ok()?,
        )?,
        name: link.name,
        secret: link.secret,
        model_name: ceilings.model_bound_name,
        items: u32::try_from(wire::Items::worst_case_bytes(&wire)?).ok()?,
        left: u32::try_from(wire::Left::worst_case_bytes(&wire)?).ok()?,
        saved: u32::try_from(wire::Saved::worst_case_bytes(&wire)?).ok()?,
        evidence: u32::try_from(hub.delivery_evidence_bytes).ok()?,
        resource: u32::try_from(wire::Preparation::worst_case_bytes(&wire)?).ok()?,
        slots: core.fleet.slots,
        calls: hub.run_calls,
        turns: hub.turns,
        inbox: core.tasks.run_inbox,
        answered: core.call_records,
        models: ceilings.bounds_models,
        workstreams: core.fleet.workstreams,
        accounts: core.accounts.accounts,
        credentials: u32::try_from(settings.identities.len()).ok()?,
        facts: link.facts,
    };
    let limits = kit::Limits {
        sizes,
        names: link.names,
        workers: core.fleet.workers,
        opening: link.opening,
        chunk: link.chunk,
        handshake: link.handshake,
        hello: link.hello,
        ping: link.ping,
        silence: link.silence,
        stall: link.stall,
    };
    if !valid(&limits) {
        return None;
    }
    Some(limits)
}

/// Check the channel's limits independently of the service's containers.
#[must_use]
pub fn valid(limits: &kit::Limits) -> bool {
    // jig's valid() replaces this at the set's repin
    kit::valid(limits)
}

#[cfg(test)]
#[test]
fn channel_limits_accept_stock_and_refuse_zero_handshake() {
    let mut limits = limits(&crate::tests::fixtures::engine()).unwrap();
    assert!(valid(&limits));
    limits.handshake = skein_lib::Duration::ZERO;
    assert!(!valid(&limits));
}

impl Workers {
    /// Construct checked channels from the service's typed settings.
    pub fn configured(
        settings: &mut crate::settings::Engine,
        deployment: [u8; 16],
        max: u32,
    ) -> Result<Self, Problems> {
        let worker_limits = limits(settings).ok_or(invalid(What::Bound))?;
        let mut endpoints =
            List::with_capacity(u32::try_from(settings.jig.llm.endpoints.len()).expect("endpoint count"));
        for endpoint in &settings.jig.llm.endpoints {
            endpoints
                .push(smith_protocol_channel::Endpoint {
                    name: endpoint.name.clone(),
                    number: endpoint.number,
                    dialect: endpoint.dialect,
                })
                .expect("endpoint count");
        }
        let workers = Self::new(
            &settings.jig,
            &settings.files,
            Encoding {
                limits: crate::translate::limits(&settings.limits).ok_or(invalid(What::Bound))?,
                endpoints: smith_protocol_channel::Endpoints::new(endpoints),
                deployment,
            },
            ::core::mem::replace(&mut settings.forges, Box::new([])),
            &worker_limits,
            max.checked_mul(jig_protocol_engine_workers::MAX_OUT_DOWN).expect("worker routing room"),
        )?;
        Ok(workers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hello_forwards_a_nonzero_configured_worker_position() {
        let mut service = crate::tests::fixtures::started();
        let channel = Token::new(7);
        let hello = host::Hello {
            slots: 1,
            stop_bound: skein_lib::Duration::from_millis(100),
            workstreams: Box::new([]),
            hosting: Box::new([]),
            bounds: crate::tests::fixtures::worker_agent().bounds,
        };
        let mut events = Queue::with_capacity(1);
        service.engine.as_mut().expect("engine").workers.out.push(kit::Request::Hello { channel, worker: 1, hello });
        service.engine.as_mut().expect("engine").workers.drain(&mut events);
        let Some(root::Event::Worker(root::WorkerInput::Link {
            channel: forwarded_channel,
            link: root::Link::Hello { worker, .. },
        })) = events.pop()
        else {
            panic!("hello reaches the root");
        };
        assert_eq!((forwarded_channel, worker), (channel, 1));
    }
}
