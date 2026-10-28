//! The application's listener and HTTP connections (jig's protocol/service.md, 2).
use core::net::SocketAddr;
use skein_http::server as http;
use skein_io as io;
use skein_lib::{
    Duration, Env, Map, Queue, Time, Token, Wall,
    stream::{Down, Up},
};

const LISTENER: Token = Token::new(0);
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub connections: u32,
    pub http: http::Limits,
    pub stall: Duration,
}
#[derive(Debug)]
pub enum Routed {
    Io(io::Request),
    Call { call: Token, head: http::Call },
    Http { call: Token, event: http::Event },
}
#[derive(Debug)]
struct Connection {
    socket: Token,
    http: http::Server,
    events: Queue<http::Event>,
    requests: Queue<http::Request>,
    below: Queue<Down>,
    call: Option<Token>,
    reuse: Option<http::Reuse>,
    closing: bool,
    draining: bool,
    deadline: Time,
    held: Option<Time>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Phase {
    Active,
    Stopped,
    Closing,
    Aborting,
}
#[derive(Debug)]
pub struct Server {
    address: io::Addr,
    listener: Option<Token>,
    listening: bool,
    starting: bool,
    phase: Phase,
    finished: bool,
    connections: Map<Token, Connection>,
    next: u64,
    env: Env<Limits>,
}
impl Server {
    #[must_use]
    pub fn new(listener: io::Addr, limits: &Limits) -> Self {
        Self {
            address: listener,
            listener: None,
            listening: true,
            starting: true,
            phase: Phase::Active,
            finished: false,
            connections: Map::with_capacity(limits.connections),
            next: 1,
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits: *limits },
        }
    }
    #[must_use]
    pub fn listening(&self) -> Option<SocketAddr> {
        if self.listener.is_some() { self.address.socket() } else { None }
    }
    #[must_use]
    pub fn opened(&self) -> bool {
        !self.listening && self.listener.is_some()
    }
    #[must_use]
    pub fn closed(&self) -> bool {
        !self.listening && self.listener.is_none() && self.connections.is_empty()
    }
    fn name(&mut self) -> Token {
        let name = Token::new(self.next);
        self.next = self.next.checked_add(1).expect("server names do not exhaust");
        name
    }
    fn http_env(&self) -> Env<http::Limits> {
        Env { now: self.env.now, wall: self.env.wall, limits: self.env.limits.http }
    }
    pub fn up(&mut self, event: io::Event, out: &mut Queue<Routed>) {
        match event {
            io::Event::Listening { listener, addr, .. } => {
                self.listening = false;
                self.listener = Some(listener);
                self.address = addr;
                if self.phase != Phase::Active {
                    self.end_listener(out);
                }
            }
            io::Event::Accepted { owner, socket, .. } => {
                if owner != LISTENER
                    || self.phase != Phase::Active
                    || self.connections.len() == self.connections.capacity()
                {
                    out.push(Routed::Io(io::Request::Reject { socket }));
                } else {
                    let owner = self.name();
                    let mut requests = Queue::with_capacity(8);
                    requests.push(http::Request::Next);
                    self.connections
                        .insert(
                            owner,
                            Connection {
                                socket,
                                http: http::Server::new(&self.env.limits.http),
                                events: Queue::with_capacity(8),
                                requests,
                                below: Queue::with_capacity(8),
                                call: None,
                                reuse: None,
                                closing: false,
                                draining: false,
                                deadline: self.env.now.saturating_add(self.env.limits.stall),
                                held: None,
                            },
                        )
                        .expect("admitted connection");
                    out.push(Routed::Io(io::Request::Bind { socket, owner }));
                }
            }
            io::Event::Stream { owner, up } => {
                let env = self.http_env();
                if let Some(connection) = self.connections.get_mut(&owner)
                    && !connection.closing
                {
                    connection.deadline =
                        self.env.now.saturating_add(self.env.limits.stall).max(connection.held.unwrap_or(Time::ZERO));
                    let ended = match &up {
                        Up::End => connection.call.is_some(),
                        Up::Bytes(_) | Up::Room | Up::Failed(_) => false,
                    };
                    http::up(&mut connection.http, &env, up, &mut connection.events, &mut connection.below);
                    if ended {
                        end(connection, false, out);
                    }
                }
            }
            io::Event::Closed { owner } => {
                if owner == LISTENER {
                    self.listening = false;
                    self.listener = None;
                }
                if let Some(connection) = self.connections.remove(&owner)
                    && let Some(call) = connection.call
                {
                    out.push(Routed::Http { call, event: http::Event::Closed });
                }
            }
            io::Event::Failed { owner, .. } => {
                if owner == LISTENER {
                    self.listening = false;
                }
                if let Some(connection) = self.connections.get_mut(&owner) {
                    end(connection, false, out);
                }
            }
            io::Event::Connecting { .. }
            | io::Event::Connected { .. }
            | io::Event::Output { .. }
            | io::Event::Spawned { .. }
            | io::Event::Exited { .. }
            | io::Event::Usage { .. }
            | io::Event::Resized
            | io::Event::Window { .. }
            | io::Event::Shutdown { .. } => {}
        }
    }
    fn owner(&self, call: Token) -> Option<Token> {
        for (owner, connection) in &self.connections {
            if connection.call == Some(call) {
                return Some(*owner);
            }
        }
        None
    }
    /// Hold an application call through its own deadline, leaving the normal
    /// stall interval for its ending page to reach the peer.
    pub(crate) fn hold(&mut self, call: Token, deadline: Time) {
        if let Some(owner) = self.owner(call) {
            let connection = self.connections.get_mut(&owner).expect("held call");
            let until = deadline.saturating_add(self.env.limits.stall);
            connection.held = Some(until);
            connection.deadline = connection.deadline.max(until);
        }
    }
    pub(crate) fn release(&mut self, call: Token) {
        if let Some(owner) = self.owner(call) {
            let connection = self.connections.get_mut(&owner).expect("released call");
            connection.held = None;
            connection.deadline = self.env.now.saturating_add(self.env.limits.stall);
        }
    }
    pub fn answer(&mut self, call: Token, mut request: http::Request, out: &mut Queue<Routed>) {
        if let http::Request::Respond(response) = &mut request {
            crate::clients::policy::add(response);
        }
        if let http::Request::Next = request {
            self.done(call, out);
            return;
        }
        if let Some(owner) = self.owner(call) {
            let connection = self.connections.get_mut(&owner).expect("call owner");
            if !connection.closing {
                connection.deadline = self.env.now.saturating_add(self.env.limits.stall);
                connection.requests.push(request);
            }
        }
    }
    pub fn done(&mut self, call: Token, out: &mut Queue<Routed>) {
        if let Some(owner) = self.owner(call) {
            let connection = self.connections.get_mut(&owner).expect("call owner");
            connection.call = None;
            connection.held = None;
            if connection.reuse.take() == Some(http::Reuse::Keep) && self.phase < Phase::Closing {
                connection.requests.push(http::Request::Next);
            } else {
                end(connection, self.phase != Phase::Aborting, out);
            }
        }
    }
    fn end_listener(&mut self, out: &mut Queue<Routed>) {
        if let Some(listener) = self.listener {
            out.push(Routed::Io(if self.phase == Phase::Aborting {
                io::Request::Abort { entity: listener }
            } else {
                io::Request::Close { entity: listener }
            }));
        }
    }
    pub fn stop(&mut self, out: &mut Queue<Routed>) {
        if self.phase == Phase::Active {
            self.phase = Phase::Stopped;
            self.end_listener(out);
        }
    }
    pub fn close(&mut self, out: &mut Queue<Routed>) {
        self.stop(out);
        if self.phase != Phase::Aborting {
            self.phase = Phase::Closing;
        }
    }
    /// The clients component and its mount have no more responses to supply.
    pub fn finish(&mut self) {
        self.finished = true;
    }
    pub fn abort(&mut self, out: &mut Queue<Routed>) {
        self.phase = Phase::Aborting;
        self.end_listener(out);
    }
    #[must_use]
    pub fn next_deadline(&self) -> Option<Time> {
        let mut next = None;
        for (_, connection) in &self.connections {
            if !connection.closing {
                next = Some(match next {
                    Some(at) => Time::min(at, connection.deadline),
                    None => connection.deadline,
                });
            }
        }
        next
    }
    pub fn clock(&mut self, now: Time, wall: Wall) {
        self.env.now = now;
        self.env.wall = wall;
    }
    pub fn fire(&mut self, out: &mut Queue<Routed>) {
        let mut due = None;
        for (owner, connection) in &self.connections {
            if !connection.closing && connection.deadline <= self.env.now {
                due = Some(*owner);
                break;
            }
        }
        if let Some(owner) = due {
            end(self.connections.get_mut(&owner).expect("due connection"), false, out);
        }
    }
    /// Runs one queued HTTP transition, or one shutdown transition.
    pub fn advance(&mut self, out: &mut Queue<Routed>) -> bool {
        if self.starting {
            self.starting = false;
            out.push(Routed::Io(io::Request::Listen { owner: LISTENER, addr: self.address }));
            return true;
        }
        let env = self.http_env();
        let mut selected = None;
        for (owner, connection) in &self.connections {
            if (self.phase == Phase::Aborting && (!connection.closing || connection.draining))
                || (self.phase >= Phase::Closing && !connection.closing && (connection.call.is_none() || self.finished))
                || !connection.below.is_empty()
                || !connection.requests.is_empty()
                || !connection.events.is_empty()
            {
                selected = Some(*owner);
                break;
            }
        }
        let Some(owner) = selected else { return false };
        let call = self.name();
        let connection = self.connections.get_mut(&owner).expect("selected connection");
        if self.phase == Phase::Aborting
            || (self.phase >= Phase::Closing
                && (connection.call.is_none()
                    || (self.finished
                        && connection.below.is_empty()
                        && connection.requests.is_empty()
                        && connection.events.is_empty())))
        {
            end(connection, self.phase != Phase::Aborting, out);
        } else if let Some(down) = connection.below.pop() {
            out.push(Routed::Io(io::Request::Stream { stream: connection.socket, down }));
        } else if let Some(request) = connection.requests.pop() {
            http::down(&mut connection.http, &env, request, &mut connection.events, &mut connection.below);
        } else if let Some(event) = connection.events.pop() {
            match event {
                http::Event::Call(head) => {
                    connection.call = Some(call);
                    out.push(Routed::Call { call, head });
                }
                event @ (http::Event::Ended
                | http::Event::Body(_)
                | http::Event::Reply(_)
                | http::Event::Refused(_)
                | http::Event::Done(_)
                | http::Event::Failed(_)
                | http::Event::Closed) => {
                    if let http::Event::Done(reuse) = &event {
                        connection.reuse = Some(*reuse);
                    }
                    if let Some(call) = connection.call {
                        out.push(Routed::Http { call, event });
                    } else {
                        end(connection, false, out);
                    }
                }
            }
        }
        true
    }
}
fn end(connection: &mut Connection, drain: bool, out: &mut Queue<Routed>) {
    if !connection.closing || (connection.draining && !drain) {
        connection.closing = true;
        connection.draining = drain;
        connection.events = Queue::with_capacity(8);
        connection.requests = Queue::with_capacity(8);
        connection.below = Queue::with_capacity(8);
        out.push(Routed::Io(if drain {
            io::Request::Close { entity: connection.socket }
        } else {
            io::Request::Abort { entity: connection.socket }
        }));
    }
}
/// Listener and HTTP edge limits required by the connection lifecycle.
#[must_use]
pub fn valid(limits: &Limits) -> bool {
    // skein's valid() replaces this at the set's repin
    limits.connections != 0 && limits.stall != Duration::ZERO && http::worst_case(&limits.http).is_some()
}

#[cfg(test)]
#[test]
fn listener_limits_accept_stock_and_refuse_zero_connections_or_stall() {
    let mut limits = crate::limits::server(&crate::tests::fixtures::engine().jig.limits);
    assert!(valid(&limits));
    limits.connections = 0;
    assert!(!valid(&limits));
    limits = crate::limits::server(&crate::tests::fixtures::engine().jig.limits);
    limits.stall = Duration::ZERO;
    assert!(!valid(&limits));
    limits = crate::limits::server(&crate::tests::fixtures::engine().jig.limits);
    limits.http.read = 0;
    assert!(!valid(&limits));
}
