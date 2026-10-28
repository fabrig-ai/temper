//! A harness that steps the worker root as its loop would, and plays io and
//! the protocol layer around it: each process request ended by its terminal,
//! each start decoded and each payload encoded by a scripted codec, each git
//! operation failed.

use super::fixtures::{self, RUN};
use crate::checkout::git::{Done, Fault};
use crate::{Domain, Encodable, Encoded, Event, Limits, Request, agent, host, step, worst_case};
use alloc::boxed::Box;
use skein_lib::{Env, List, Queue, Time, Token, Wall};

const PROCESS: Token = Token::new(99);

pub struct Harness {
    pub domain: Domain,
    pub env: Env<Limits>,
    pub out: Queue<Request>,
    /// What io and the protocol layer owe the root, oldest first.
    pub owed: Queue<Event>,
    /// Encoded payloads held back while `hold` is set.
    pub held: Queue<Event>,
    pub hold: bool,
    /// smith has a read out on its agent's channel.
    pub reading: bool,
    /// The hub's token for the run, as its start was handed out.
    pub owner: Option<Token>,
    /// smith's token for the run's agent, as it was spawned.
    pub agent: Option<Token>,
}

impl Harness {
    pub fn new() -> Harness {
        let limits = fixtures::limits();
        assert!(worst_case(&limits).is_some());
        Harness {
            domain: Domain::new(&limits),
            out: Queue::uncapped(),
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
            owed: Queue::with_capacity(64),
            held: Queue::with_capacity(64),
            hold: false,
            reading: false,
            owner: None,
            agent: None,
        }
    }

    /// Steps the root with `event`, then reclaims, as the loop would.
    pub fn step(&mut self, event: Event) -> Box<[Request]> {
        let rows = self.unreclaimed(event);
        self.domain.reclaim();
        rows
    }

    /// Steps the root with `event`, within the loop's iteration.
    pub fn unreclaimed(&mut self, event: Event) -> Box<[Request]> {
        step(&mut self.domain, &self.env, event, &mut self.out);
        let mut rows = List::with_capacity(self.out.len());
        while let Some(row) = self.out.pop() {
            rows.push(row).expect("reserved outputs");
        }
        rows.into_boxed()
    }

    /// Steps the root with `event`, then with what is owed for it, and for
    /// that, until nothing is: every request emitted meanwhile, in order.
    pub fn run(&mut self, event: Event) -> Box<[Request]> {
        let mut rows = List::with_capacity(1_024);
        let mut next = Some(event);
        for _ in 0..64_u32 {
            let event = match next.take() {
                Some(event) => event,
                None => match self.owed.pop() {
                    Some(event) => event,
                    None => return rows.into_boxed(),
                },
            };
            for row in self.step(event) {
                self.owe(&row);
                rows.push(row).expect("a bounded story");
            }
            if self.domain.is_ready() {
                crate::resume(&mut self.domain, &self.env, &mut self.out);
                let mut ready = List::with_capacity(self.out.len());
                while let Some(row) = self.out.pop() {
                    ready.push(row).expect("reserved outputs");
                }
                self.domain.reclaim();
                for row in ready.into_boxed() {
                    self.owe(&row);
                    rows.push(row).expect("a bounded story");
                }
            }
        }
        panic!("a story settles")
    }

    /// What the protocol layer and io owe for `request`.
    fn owe(&mut self, request: &Request) {
        let event = match request {
            Request::Process(process) => match process.as_ref() {
                agent::process::Request::Spawn { owner, .. } => {
                    self.agent = Some(*owner);
                    agent::process::Event::Spawned { owner: *owner, process: PROCESS }
                }
                agent::process::Request::Send { owner, .. } => agent::process::Event::Sent { owner: *owner },
                agent::process::Request::Signal { owner, .. } => agent::process::Event::Signalled { owner: *owner },
                agent::process::Request::Read { .. } => {
                    self.reading = true;
                    return;
                }
                agent::process::Request::Wait { .. } | agent::process::Request::Reap { .. } => return,
            },
            Request::Decode { agent, run, start } => {
                self.owner = Some(*agent);
                let start = Ok(Box::new(fixtures::start(*run, start)));
                return self.owed.push(Event::Decoded { agent: *agent, start });
            }
            Request::Encode { owner, value } => {
                let encoded = Event::Encoded { owner: *owner, encoded: encode(value) };
                return if self.hold { self.held.push(encoded) } else { self.owed.push(encoded) };
            }
            Request::Fact { owner, .. } => return self.owed.push(Event::Activity { owner: *owner, activity: None }),
            Request::Io { owner, .. } => {
                return self.owed.push(Event::Done { owner: *owner, done: Done::Failed { fault: Fault::Unreachable } });
            }
            Request::Source { .. }
            | Request::SourceFault
            | Request::Hello(_)
            | Request::Link(_)
            | Request::Answer { .. }
            | Request::CancelRelay { .. }
            | Request::Stop
            | Request::CancelIo { .. } => return,
        };
        self.owed.push(Event::Process(Box::new(event)));
    }

    /// The payloads held back come back now, in order.
    pub fn release(&mut self) -> Box<[Request]> {
        self.hold = false;
        let mut rows = List::with_capacity(1_024);
        while let Some(event) = self.held.pop() {
            for row in self.run(event) {
                rows.push(row).expect("a bounded story");
            }
        }
        rows.into_boxed()
    }

    /// A run with no repository, started: smith admitted it.
    pub fn started(&mut self) {
        self.started_asking(host::LiveAsk::NONE);
    }

    /// A run with no repository whose assignment asks `live` of it, started.
    pub fn started_asking(&mut self, live: host::LiveAsk) {
        let mut assignment = fixtures::assignment(false, Box::new([]));
        assignment.assignment.live = live;
        self.run(Event::Link(host::LinkDown::Assign { assignment }));
        self.say(agent::Up::Admitted);
    }

    /// smith's agent says `message`, on the read out.
    pub fn say(&mut self, message: agent::Up) -> Box<[Request]> {
        assert!(self.reading, "the agent speaks on a read");
        self.reading = false;
        let owner = self.agent.expect("a spawned agent");
        self.run(Event::Process(Box::new(agent::process::Event::Received { owner, message })))
    }

    /// The agent's process exits, its channel ends and its tree empties.
    pub fn gone(&mut self) -> Box<[Request]> {
        let owner = self.agent.expect("a spawned agent");
        let mut rows = List::with_capacity(1_024);
        let mut events = List::with_capacity(3);
        if self.reading {
            self.reading = false;
            events.push(agent::process::Event::Hangup { owner }).expect("room");
        }
        events.push(agent::process::Event::Exited { owner, exit: Some(agent::Exit::Code(0)) }).expect("room");
        events.push(agent::process::Event::Reaped { owner, detail: Box::new([]) }).expect("room");
        for event in events.into_boxed() {
            for row in self.run(Event::Process(Box::new(event))) {
                rows.push(row).expect("a bounded story");
            }
        }
        rows.into_boxed()
    }

    /// The hub's start for the run, handed out to be decoded and left there,
    /// after an assignment with no repository.
    pub fn decode(&mut self, grants: Box<[host::Grant]>) -> (Token, host::Start) {
        let assignment = fixtures::assignment(false, grants);
        let rows = self.step(Event::Link(host::LinkDown::Assign { assignment }));
        for row in rows {
            if let Request::Decode { agent, run, start } = row {
                assert_eq!(run, RUN);
                return (agent, *start);
            }
        }
        panic!("the start is handed out")
    }
}

/// A scripted codec: a turn's bytes are its number's, a call's name its
/// completion's, a result a word.
fn encode(value: &Encodable) -> Encoded {
    match value {
        Encodable::Turn(turn) => Encoded::Turn(host::Turn {
            opens_window: turn.opens_window,
            refused: Box::new([]),
            turn: turn.number,
            spent: turn.spent,
            read: turn.read,
            body: Box::new(turn.number.to_be_bytes()),
        }),
        Encodable::CallName(name) => Encoded::CallName(Box::new(name.completion.to_be_bytes())),
        Encodable::Result(_) => Encoded::Result(Box::from(&b"result"[..])),
    }
}

/// Whether `rows` hold an answer ending as `expected`.
pub fn answered(rows: &[Request], expected: &host::Ending) -> bool {
    for row in rows {
        if let Request::Answer { answer, .. } = row
            && answer.ending == *expected
        {
            return true;
        }
    }
    false
}

/// Whether `rows` ask io for a git operation.
pub fn operated(rows: &[Request]) -> bool {
    for row in rows {
        if let Request::Io { .. } = row {
            return true;
        }
    }
    false
}
