//! One command's lazy HTTP connection over kernel socket jobs.
use skein_http::client as http;
use skein_io::kernel::{self as k, Done, Fd, Op};
use skein_lib::{
    Env, Intake, Queue, Time, Wall,
    stream::{Down, Read, Up},
};
use std::collections::VecDeque;

/// The response is retained only until the route decodes this call's answer.
#[derive(Debug, PartialEq, Eq)]
pub struct Reply {
    pub status: u16,
    pub body: Vec<u8>,
}
/// A refused, reset, truncated or invalid HTTP exchange.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Failed;
#[derive(Debug)]
enum State {
    Dormant,
    Opening,
    Connecting(Fd),
    Open(Fd),
    Shutdown(Fd),
    Closing(Fd),
    Closed,
}
/// A lazy connection: one call at a time and one kernel operation in flight.
/// Its owner submits `next_job`'s job and returns it through `complete`, and closes
/// the connection before exiting. Read demands use an intake; short sends keep
/// their offset and output room is granted only when the previous send freed it.
#[derive(Debug)]
pub struct Connection {
    state: State,
    address: Option<k::Addr>,
    limits: http::Limits,
    body_limit: u32,
    client: http::Client,
    above: Queue<http::Event>,
    below: Queue<Down>,
    intake: Intake,
    read: Read,
    room: u32,
    sends: VecDeque<(Box<[u8]>, u32)>,
    call: Option<http::Call>,
    upload: Option<(Box<[u8]>, usize)>,
    response: Option<Reply>,
    result: Option<Result<Reply, Failed>>,
    busy: bool,
    reusable: bool,
    ended: bool,
}
impl Connection {
    /// Allocate the HTTP client and its bounded receive intake, without a socket.
    #[must_use]
    pub fn new(limits: http::Limits, body_limit: u32) -> Self {
        Self {
            state: State::Dormant,
            address: None,
            limits,
            body_limit,
            client: http::Client::new(&limits),
            above: Queue::with_capacity(8),
            below: Queue::with_capacity(8),
            intake: Intake::with_capacity(http::largest_read(&limits)),
            read: Read::Nothing,
            room: 0,
            sends: VecDeque::new(),
            call: None,
            upload: None,
            response: None,
            result: None,
            busy: false,
            reusable: true,
            ended: false,
        }
    }
    /// Begin a call on this command's listener; it never opens a second socket.
    pub fn call(&mut self, address: k::Addr, call: http::Call, body: Box<[u8]>) {
        assert!(!self.busy && self.result.is_none(), "take the previous call's answer");
        if let Some(previous) = self.address {
            assert_eq!(previous, address, "one listener per command");
        }
        self.address = Some(address);
        self.busy = true;
        if !self.reusable {
            self.fail();
            return;
        }
        if body.len() > usize::try_from(self.body_limit).expect("u32 fits") {
            self.fail();
            return;
        }
        self.call = Some(call);
        self.upload = (!body.is_empty()).then_some((body, 0));
        if matches!(self.state, State::Dormant) {
            self.state = State::Opening;
        }
    }
    /// Take the completed exchange; only HTTP Done makes an answer visible.
    pub fn take(&mut self) -> Option<Result<Reply, Failed>> {
        self.result.take()
    }
    /// Whether a call still owes its terminal answer.
    #[must_use]
    pub const fn busy(&self) -> bool {
        self.busy
    }
    fn env(&self) -> Env<http::Limits> {
        Env { now: Time::ZERO, wall: Wall::EPOCH, limits: self.limits }
    }
    fn down(&mut self, request: http::Request) {
        let env = self.env();
        http::down(&mut self.client, &env, request, &mut self.above, &mut self.below);
    }
    fn up(&mut self, event: Up) {
        let env = self.env();
        http::up(&mut self.client, &env, event, &mut self.above, &mut self.below);
    }
    fn body_demand(&mut self) {
        self.down(http::Request::Body(Down::Demand { read: Read::Arrived { max: self.limits.read }, room: 0 }));
    }
    fn fail(&mut self) {
        self.result = Some(Err(Failed));
        self.busy = false;
        self.reusable = false;
        self.call = None;
        self.upload = None;
        self.response = None;
        self.sends.clear();
        self.read = Read::Nothing;
        self.room = 0;
    }
    fn event(&mut self, event: http::Event) {
        match event {
            http::Event::Response(head) => {
                self.response = Some(Reply { status: head.status, body: Vec::new() });
                self.body_demand();
            }
            http::Event::Upload(Up::Room) => {
                if let Some((bytes, from)) = self.upload.take() {
                    let end = bytes.len().min(from + usize::try_from(self.limits.send).expect("u32 fits"));
                    self.down(http::Request::Upload(Down::Send(bytes[from..end].into())));
                    if end == bytes.len() {
                        self.down(http::Request::Upload(Down::Finish));
                    } else {
                        self.upload = Some((bytes, end));
                        self.down(http::Request::Upload(Down::Demand { read: Read::Nothing, room: self.limits.send }));
                    }
                }
            }
            http::Event::Body(Up::Bytes(bytes)) => {
                let reply = self.response.as_mut().expect("body follows response head");
                if reply.body.len().saturating_add(bytes.len()) > usize::try_from(self.body_limit).expect("u32 fits") {
                    self.fail();
                } else {
                    reply.body.extend_from_slice(&bytes);
                    self.body_demand();
                }
            }
            http::Event::Body(Up::End) | http::Event::Closed => {}
            http::Event::Done(reuse) => {
                self.reusable = reuse == http::Reuse::Keep;
                self.busy = false;
                self.result = Some(self.response.take().ok_or(Failed));
            }
            http::Event::Failed(_)
            | http::Event::Body(Up::Failed(_) | Up::Room)
            | http::Event::Upload(Up::Failed(_) | Up::End | Up::Bytes(_)) => self.fail(),
        }
    }
    /// Produce the next kernel job after routing all immediately available HTTP work.
    pub fn next_job(&mut self) -> Option<Op> {
        // A failed open/connect owes its terminal before the owner closes it.
        if self.result.is_some() {
            return None;
        }
        match self.state {
            State::Dormant | State::Closed => return None,
            State::Opening => return Some(Op::Socket { family: k::Family::of(&self.address.expect("call listener")) }),
            State::Connecting(fd) => return Some(Op::Connect { fd, addr: self.address.expect("call listener") }),
            State::Shutdown(fd) => return Some(Op::Shutdown { fd }),
            State::Closing(fd) => return Some(Op::Close { fd }),
            State::Open(_) => {}
        }
        if let Some(call) = self.call.take() {
            self.down(http::Request::Call(call));
            if self.upload.is_some() {
                self.down(http::Request::Upload(Down::Demand { read: Read::Nothing, room: self.limits.send }));
            }
        }
        // Each movement consumes an event, a demand or bounded intake bytes.
        loop {
            if let Some(event) = self.above.pop() {
                self.event(event);
                continue;
            }
            if let Some(down) = self.below.pop() {
                match down {
                    Down::Demand { read, room } => {
                        self.read = read;
                        self.room = room;
                    }
                    Down::Send(bytes) => self.sends.push_back((bytes, 0)),
                    Down::Finish => {}
                }
                continue;
            }
            if !self.busy {
                return None;
            }
            if self.room > 0 && self.sends.is_empty() {
                self.room = 0;
                self.read = Read::Nothing;
                self.up(Up::Room);
                continue;
            }
            if let Some(bytes) = self.intake.meet(self.read) {
                self.read = Read::Nothing;
                self.up(Up::Bytes(bytes));
                continue;
            }
            break;
        }
        let State::Open(fd) = self.state else { unreachable!("connected HTTP stream") };
        if let Some((bytes, from)) = self.sends.pop_front() {
            return Some(Op::Send { fd, bytes, from });
        }
        if self.read != Read::Nothing {
            if self.ended {
                self.fail();
                return None;
            }
            return Some(Op::Recv { fd, buf: vec![0; usize::try_from(self.intake.room()).expect("u32 fits")].into() });
        }
        None
    }
    /// Return exactly one completed job before asking for the next.
    pub fn complete(&mut self, op: Op, result: Result<Done, k::Error>) {
        match (op, result) {
            (Op::Socket { .. }, Ok(Done::Fd(fd))) => self.state = State::Connecting(fd),
            (Op::Connect { fd, .. }, Ok(Done::Nothing)) => self.state = State::Open(fd),
            (Op::Send { bytes, from, .. }, Ok(Done::Count(count))) if count > 0 => {
                let from = from + count;
                if usize::try_from(from).expect("offset") != bytes.len() {
                    self.sends.push_front((bytes, from));
                }
            }
            (Op::Recv { buf, .. }, Ok(Done::Count(count))) => {
                if count == 0 {
                    self.ended = true;
                    self.up(Up::End);
                } else {
                    self.intake
                        .append(&buf[..usize::try_from(count).expect("buffer count")])
                        .expect("receive within intake room");
                }
            }
            (Op::Shutdown { fd }, _) => self.state = State::Closing(fd),
            (Op::Close { .. }, _) => self.state = State::Closed,
            (_, Err(_)) => self.fail(),
            (_, Ok(_)) => panic!("socket completion has its operation's shape"),
        }
    }
    /// Withdraw HTTP work, then shut down and release the socket, if one opened.
    pub fn close(&mut self) {
        if matches!(self.state, State::Shutdown(_) | State::Closing(_) | State::Closed) {
            return;
        }
        self.down(http::Request::Close);
        while self.above.pop().is_some() {}
        while self.below.pop().is_some() {}
        self.sends.clear();
        self.intake.clear();
        self.read = Read::Nothing;
        self.room = 0;
        self.call = None;
        self.upload = None;
        self.busy = false;
        self.result = None;
        self.state = match self.state {
            State::Dormant | State::Opening | State::Closed => State::Closed,
            State::Connecting(fd) | State::Closing(fd) => State::Closing(fd),
            State::Open(fd) | State::Shutdown(fd) => State::Shutdown(fd),
        };
    }
    /// All socket descriptors have reported closed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        matches!(self.state, State::Closed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skein_http::{Header, Method};
    use std::collections::BTreeSet;
    fn request() -> Connection {
        let limits = http::Limits { request: 1024, head: 1024, headers: 8, read: 64, send: 64 };
        let mut connection = Connection::new(limits, 1024);
        connection.call(
            ([127, 0, 0, 1], 3000).into(),
            http::Call {
                method: Method::Post,
                target: b"/.fake/git/clone".as_slice().into(),
                headers: Box::new([Header {
                    name: b"Host".as_slice().into(),
                    value: b"127.0.0.1:3000".as_slice().into(),
                }]),
                body: http::Body::Length(2),
                close: false,
            },
            b"{}".as_slice().into(),
        );
        connection
    }
    #[test]
    fn socket_exchanges_answer_or_fail_and_close_every_descriptor() {
        // The literal reply's body is six bytes; every row sends its request short.
        const RESPONSE: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\n\r\nanswer";
        let rows = [
            ("whole", false, RESPONSE, usize::MAX, false, Ok(Reply { status: 200, body: b"answer".to_vec() })),
            ("pieces", false, RESPONSE, 1, false, Ok(Reply { status: 200, body: b"answer".to_vec() })),
            ("refused", true, b"".as_slice(), 1, false, Err(Failed)),
            ("reset", false, b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\n\r\na", 3, true, Err(Failed)),
            ("early close", false, b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\n\r\na", 3, false, Err(Failed)),
        ];
        for (name, refused, mut response, piece, reset, expected) in rows {
            let mut connection = request();
            let mut held = BTreeSet::new();
            let mut sent = Vec::new();
            let mut connects = 0;
            let mut result = None;
            for _ in 0..1024 {
                let job = connection.next_job();
                if result.is_none()
                    && let Some(reply) = connection.take()
                {
                    result = Some(reply);
                    connection.close();
                }
                if connection.is_closed() {
                    break;
                }
                let Some(mut job) = job else { continue };
                let result = match &mut job {
                    Op::Socket { .. } => {
                        assert!(held.insert(Fd::new(7)), "{name}");
                        Ok(Done::Fd(Fd::new(7)))
                    }
                    Op::Connect { .. } => {
                        connects += 1;
                        if refused { Err(k::Error::Refused) } else { Ok(Done::Nothing) }
                    }
                    Op::Send { bytes, from, .. } => {
                        let bytes = &bytes[usize::try_from(*from).expect("offset")..];
                        let count = bytes.len().min(3);
                        sent.extend_from_slice(&bytes[..count]);
                        Ok(Done::Count(u32::try_from(count).expect("three bytes")))
                    }
                    Op::Recv { buf, .. } => {
                        if response.is_empty() && reset {
                            Err(k::Error::Reset)
                        } else {
                            let count = response.len().min(buf.len()).min(piece);
                            buf[..count].copy_from_slice(&response[..count]);
                            response = &response[count..];
                            Ok(Done::Count(u32::try_from(count).expect("receive buffer")))
                        }
                    }
                    Op::Shutdown { .. } => Ok(Done::Nothing),
                    Op::Close { fd } => {
                        assert!(held.remove(fd), "{name}: closes once");
                        Ok(Done::Nothing)
                    }
                    other @ (Op::Bind { .. }
                    | Op::Listen { .. }
                    | Op::Accept { .. }
                    | Op::Open { .. }
                    | Op::Read { .. }
                    | Op::Write { .. }
                    | Op::Append { .. }
                    | Op::Truncate { .. }
                    | Op::Sync { .. }
                    | Op::Lock { .. }
                    | Op::Stat { .. }
                    | Op::Rename { .. }
                    | Op::Remove { .. }
                    | Op::MakeDirectory { .. }
                    | Op::List { .. }
                    | Op::Spawn { .. }
                    | Op::Wait { .. }
                    | Op::Signal { .. }
                    | Op::Usage
                    | Op::Window { .. }
                    | Op::ReadSignal { .. }
                    | Op::PipeRead { .. }
                    | Op::PipeWrite { .. }
                    | Op::Cancel { .. }) => panic!("connection emits only socket jobs: {other:?}"),
                };
                connection.complete(job, result);
            }
            assert_eq!(result, Some(expected), "{name}");
            assert_eq!(connects, 1, "{name}: connect once");
            assert!(connection.is_closed() && held.is_empty(), "{name}: all descriptors close");
            if !refused {
                assert!(sent.starts_with(b"POST /.fake/git/clone HTTP/1.1\r\n"), "{name}");
                assert!(sent.ends_with(b"\r\n\r\n{}"), "{name}");
            }
        }
    }
}
