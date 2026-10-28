//! The owned process's version barrier, on the box's io (protocol/setup.md, 6.6/7.1).
use alloc::boxed::Box;
use core::net::SocketAddr;
use skein_io::{Addr, Event, Request};
use skein_lib::{
    Duration, List, Queue, Time, Token,
    stream::{Down, Read, Up},
};

// A local Forgejo normally stands in under a second. A bounded thirty seconds
// also covers a cold database; each refused connection is retried after 50 ms.
const DEADLINE: Duration = Duration::from_secs(30);
const RETRY: Duration = Duration::from_millis(50);
const REQUEST: &[u8] = b"GET /api/v1/version HTTP/1.0\r\nHost: localhost\r\n\r\n";
const STATUS_BYTES: u32 = 128;
#[derive(Debug)]
pub(crate) struct Ready {
    address: SocketAddr,
    active: bool,
    until: Time,
    at: Time,
    owner: Option<Token>,
    socket: Option<Token>,
    closing: bool,
    status: List<u8>,
    result: Option<bool>,
}
impl Ready {
    pub(crate) fn new(address: SocketAddr) -> Self {
        Self {
            address,
            active: false,
            until: Time::ZERO,
            at: Time::ZERO,
            owner: None,
            socket: None,
            closing: false,
            status: List::with_capacity(STATUS_BYTES),
            result: None,
        }
    }
    pub(crate) fn start(&mut self, now: Time) {
        assert!(self.owner.is_none(), "the previous version check drained");
        self.active = true;
        self.until = now.saturating_add(DEADLINE);
        self.at = now;
        self.result = None;
    }
    pub(crate) fn owns(&self, event: &Event) -> bool {
        let owner = match event {
            Event::Connecting { owner, .. }
            | Event::Connected { owner }
            | Event::Stream { owner, .. }
            | Event::Failed { owner, .. }
            | Event::Closed { owner } => *owner,
            Event::Listening { .. }
            | Event::Accepted { .. }
            | Event::Output { .. }
            | Event::Spawned { .. }
            | Event::Exited { .. }
            | Event::Usage { .. }
            | Event::Resized
            | Event::Window { .. }
            | Event::Shutdown { .. } => return false,
        };
        self.owner == Some(owner)
    }
    pub(crate) fn event(&mut self, event: Event, now: Time, out: &mut Queue<Request>) {
        match event {
            Event::Connecting { socket, .. } => {
                self.socket = Some(socket);
                if !self.active || self.result.is_some() {
                    self.close(out);
                }
            }
            Event::Connected { .. } => {
                if self.active && !self.closing {
                    out.push(Request::Stream {
                        stream: self.socket.expect("connected socket"),
                        down: Down::Demand {
                            read: Read::Nothing,
                            room: u32::try_from(REQUEST.len()).expect("small request"),
                        },
                    });
                }
            }
            Event::Stream { up, .. } => {
                if !self.active || self.closing {
                    return;
                }
                let stream = self.socket.expect("connected socket");
                match up {
                    Up::Room => {
                        out.push(Request::Stream { stream, down: Down::Send(Box::from(REQUEST)) });
                        out.push(Request::Stream {
                            stream,
                            down: Down::Demand { read: Read::Arrived { max: STATUS_BYTES }, room: 0 },
                        });
                    }
                    Up::Bytes(bytes) => {
                        for byte in &bytes {
                            if self.status.room() == 0 {
                                break;
                            }
                            self.status.push(*byte).expect("status cap");
                            if *byte == b'\n' {
                                break;
                            }
                        }
                        if self.status.as_slice().last() == Some(&b'\n') || self.status.room() == 0 {
                            let status = self.status.as_slice();
                            if status.starts_with(b"HTTP/1.1 200 ") || status.starts_with(b"HTTP/1.0 200 ") {
                                self.result = Some(true);
                            }
                            self.close(out);
                        } else {
                            out.push(Request::Stream {
                                stream,
                                down: Down::Demand { read: Read::Arrived { max: STATUS_BYTES }, room: 0 },
                            });
                        }
                    }
                    Up::End | Up::Failed(_) => self.close(out),
                }
            }
            Event::Closed { .. } => {
                self.owner = None;
                self.socket = None;
                self.closing = false;
                self.at = now.saturating_add(RETRY);
            }
            Event::Failed { .. } => {} // io owes Closed, before any retry.
            Event::Listening { .. }
            | Event::Accepted { .. }
            | Event::Output { .. }
            | Event::Spawned { .. }
            | Event::Exited { .. }
            | Event::Usage { .. }
            | Event::Resized
            | Event::Window { .. }
            | Event::Shutdown { .. } => unreachable!("version check owns only its connection"),
        }
    }
    fn close(&mut self, out: &mut Queue<Request>) {
        if !self.closing
            && let Some(entity) = self.socket
        {
            out.push(Request::Abort { entity });
            self.closing = true;
        }
    }
    pub(crate) fn cancel(&mut self, out: &mut Queue<Request>) {
        self.active = false;
        self.result = None;
        self.close(out);
    }
    pub(crate) fn tick(&mut self, now: Time, serial: &mut u64, out: &mut Queue<Request>) -> Option<bool> {
        if !self.active {
            return None;
        }
        if now >= self.until && self.result.is_none() {
            self.result = Some(false);
            self.close(out);
        }
        if self.owner.is_none() {
            if let Some(result) = self.result.take() {
                self.active = false;
                return Some(result);
            }
            if now >= self.at {
                *serial = serial.checked_add(1).expect("version serial");
                let owner = Token::new(*serial);
                self.owner = Some(owner);
                self.status.clear();
                out.push(Request::Connect { owner, addr: Addr::Ip(self.address) });
            }
        }
        None
    }
    pub(crate) fn next_deadline(&self) -> Option<Time> {
        if !self.active {
            None
        } else if self.owner.is_none() {
            Some(self.at.min(self.until))
        } else if self.closing || self.result.is_some() {
            None
        } else {
            Some(self.until)
        }
    }
    pub(crate) fn settled(&self) -> bool {
        !self.active && self.owner.is_none()
    }
}
