//! The fake LLM's one-connection control listener on the process's ring.
use skein_io::{
    self as io,
    kernel::{Complete, Op, Submit},
};
use skein_lib::{Duration, Env, Queue, Time, Token, Wall};
use std::net::SocketAddr;
use temper_fake::llm::control::Exchange;

const LISTENER: Token = Token::new(0);
const QUEUE: u32 = 32;
// io's flight slot occupies the upper 32 bits. This io has fewer than
// 2^31 flight slots, so the high bit separates its operations from the peer's.
const TAG: u64 = 1 << 63;
const LIMITS: io::Limits = io::Limits {
    sockets: 2,
    names: 0,
    refusals: 1,
    intake: 1024,
    receive: 1024,
    output: 256,
    sends: 2,
    accepts: 1,
    backlog: 1,
    close_timeout: Duration::from_secs(1),
    retry: Duration::from_millis(1),
};

pub(super) fn owns(op: Token) -> bool {
    op.raw() & TAG != 0
}
fn translate(op: &mut Token) {
    *op = Token::new(op.raw() ^ TAG);
}
pub(super) fn tag(submission: &mut Submit) {
    translate(&mut submission.op);
    if let Op::Cancel { target } = &mut submission.kind {
        translate(target);
    }
}
pub(super) fn restore(complete: &mut Complete) {
    translate(&mut complete.op);
    if let Op::Cancel { target } = &mut complete.kind {
        translate(target);
    }
}

struct Client {
    owner: Token,
    socket: Token,
    exchange: Exchange,
    closing: bool,
}
pub(super) struct Control {
    io: io::Io,
    events: Queue<io::Event>,
    requests: Queue<io::Request>,
    pub completions: Queue<Complete>,
    pub submissions: Queue<Submit>,
    listener: Option<Token>,
    client: Option<Client>,
    next: u64,
    pub address: Option<SocketAddr>,
    stopping: bool,
    pub failed: bool,
}
impl Control {
    pub fn new(address: SocketAddr) -> Self {
        let mut requests = Queue::with_capacity(QUEUE);
        requests.push(io::Request::Listen { owner: LISTENER, addr: io::Addr::Ip(address) });
        Self {
            io: io::Io::new(&LIMITS),
            events: Queue::with_capacity(QUEUE),
            requests,
            completions: Queue::with_capacity(QUEUE),
            submissions: Queue::with_capacity(QUEUE),
            listener: None,
            client: None,
            next: 1,
            address: None,
            stopping: false,
            failed: false,
        }
    }
    pub fn shutdown(&mut self) {
        self.stopping = true;
    }
    pub fn iterate(&mut self, now: Time, wall: Wall, peer: &mut skein_fake_peers::llm::Peer) {
        let env = Env { now, wall, limits: LIMITS };
        for _ in 0..QUEUE {
            if self.events.room() < 3 || self.submissions.room() < 2 {
                break;
            }
            if !self.io.is_ready() {
                break;
            }
            io::resume(&mut self.io, &env, &mut self.events, &mut self.submissions);
        }
        for _ in 0..QUEUE {
            if self.events.room() < 3 || self.submissions.room() < 2 {
                break;
            }
            let Some(complete) = self.completions.pop() else { break };
            io::up(&mut self.io, &env, complete, &mut self.events, &mut self.submissions);
        }
        for _ in 0..QUEUE {
            if self.events.room() < 3 || self.submissions.room() < 2 || !self.io.is_due(now) {
                break;
            }
            io::fire(&mut self.io, &env, &mut self.events, &mut self.submissions);
        }
        for _ in 0..QUEUE {
            if self.requests.room() < 10 {
                break;
            }
            let Some(event) = self.events.pop() else { break };
            self.event(event, now, wall);
        }
        if self.requests.room() >= 12 {
            if let Some(client) = &mut self.client
                && !client.closing
            {
                if self.stopping {
                    client.exchange.shutdown();
                } else if let Some(name) = client.exchange.release() {
                    client.exchange.released(peer.release(&name).is_ok());
                }
                client.exchange.advance(now, wall);
                while let Some(down) = client.exchange.pop() {
                    self.requests.push(io::Request::Stream { stream: client.socket, down });
                }
                if client.exchange.done() {
                    self.requests.push(io::Request::Close { entity: client.socket });
                    client.closing = true;
                }
            }
            if self.stopping
                && let Some(listener) = self.listener.take()
            {
                self.requests.push(io::Request::Close { entity: listener });
            }
        }
        for _ in 0..QUEUE {
            if self.submissions.room() < 2 || !self.io.takes() {
                break;
            }
            let Some(request) = self.requests.pop() else { break };
            io::down(&mut self.io, &env, request, &mut self.submissions);
        }
        self.io.reclaim();
    }
    fn event(&mut self, event: io::Event, now: Time, wall: Wall) {
        match event {
            io::Event::Listening { listener, addr, .. } => {
                self.listener = Some(listener);
                self.address = addr.socket();
            }
            io::Event::Accepted { socket, .. } => {
                if self.stopping || self.client.is_some() {
                    self.requests.push(io::Request::Reject { socket });
                } else {
                    let owner = Token::new(self.next);
                    self.next = self.next.checked_add(1).expect("finite control connection ids");
                    self.requests.push(io::Request::Bind { socket, owner });
                    self.client = Some(Client { owner, socket, exchange: Exchange::new(), closing: false });
                }
            }
            io::Event::Stream { owner, up } => {
                if let Some(client) = &mut self.client
                    && !client.closing
                {
                    assert_eq!(client.owner, owner, "the active control connection");
                    client.exchange.up(now, wall, up);
                }
            }
            io::Event::Closed { owner } => {
                if owner == LISTENER {
                    self.listener = None;
                } else {
                    assert_eq!(self.client.as_ref().expect("closing control connection").owner, owner);
                    self.client = None;
                }
            }
            io::Event::Failed { owner, .. } => {
                if owner == LISTENER {
                    self.failed = true;
                    self.stopping = true;
                } else if let Some(client) = &mut self.client {
                    assert_eq!(client.owner, owner, "the failed control connection");
                    client.closing = true;
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
            | io::Event::Shutdown { .. } => panic!("control only listens and serves classic byte streams"),
        }
    }
    pub fn work_pending(&self, now: Time) -> bool {
        self.io.is_ready()
            || self.io.is_due(now)
            || !self.events.is_empty()
            || !self.requests.is_empty()
            || !self.completions.is_empty()
            || !self.submissions.is_empty()
            || (self.stopping && self.listener.is_some())
            || self
                .client
                .as_ref()
                .is_some_and(|client| !client.closing && (self.stopping || client.exchange.work_pending()))
    }
    pub fn is_empty(&self) -> bool {
        self.io.is_empty()
            && self.events.is_empty()
            && self.requests.is_empty()
            && self.completions.is_empty()
            && self.submissions.is_empty()
            && self.client.is_none()
    }
    pub fn next_deadline(&self) -> Option<Time> {
        self.io.next_deadline()
    }
    pub fn operations() -> u32 {
        io::operations(&LIMITS).expect("fixed control limits")
    }
}
