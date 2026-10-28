//! Native HTTP connections on the host's io, with volatile records.
use jig_client as client;
use jig_client_shell_native::Transport;
use skein_io::{self as io, Addr};
use skein_lib::{Queue, Time, Token};
const OWNER: u64 = 1 << 60;
pub(crate) struct Memory {
    transport: Transport,
    pub(crate) events: Queue<client::Event>,
    requests: Queue<io::Request>,
}
impl Memory {
    pub(crate) fn new(engine: Addr, limits: &jig_client_shell_native::Limits) -> Self {
        let sockets = limits.exchanges.checked_add(1).expect("connections fit");
        let room = sockets.checked_mul(64).expect("queue bound");
        Self {
            transport: Transport::new(engine, limits, sockets, OWNER),
            events: Queue::with_capacity(room),
            requests: Queue::with_capacity(room),
        }
    }
    pub(crate) fn down(&mut self, now: Time, request: client::Request) {
        if matches!(request, client::Request::Save { .. } | client::Request::Erase { .. }) {
            self.events.push(client::Event::Stored);
        }
        self.transport.down(now, request);
    }
    pub(crate) fn owns(&self, owner: Token) -> bool {
        self.transport.owns(owner)
    }
    pub(crate) fn event(&mut self, now: Time, event: io::Event) {
        self.transport.event(now, event);
    }
    pub(crate) fn close(&mut self, now: Time) {
        self.transport.close(now);
        while self.events.pop().is_some() {}
    }
    pub(crate) fn progress(&mut self, now: Time) {
        self.transport.iterate(now, &mut self.events, &mut self.requests);
    }
    pub(crate) fn request(&mut self) -> Option<io::Request> {
        self.requests.pop()
    }
    pub(crate) fn next_deadline(&self) -> Option<Time> {
        self.transport.next_deadline()
    }
    pub(crate) fn pending(&self, now: Time) -> bool {
        !self.events.is_empty() || !self.requests.is_empty() || self.transport.work_pending(now)
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.transport.is_empty() && self.events.is_empty() && self.requests.is_empty()
    }
}
