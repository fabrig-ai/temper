//! A runtime driver that leaves payload hand-outs owed while two agents run.
//! Process terminals still advance; only the codec's FIFO is held.

use std::collections::VecDeque;

use skein_lib::{Env, Queue, Time, Token, Wall};
use temper_worker_domain::{Domain, Event, Request, agent, host, step};

use crate::protocol;

pub const RUNS: [Token; 2] = [Token::new(41), Token::new(42)];
pub const ATTEMPT: Token = Token::new(91);
pub const CALL: Token = Token::new(71);
/// Two turns, a call name and a result from each of the two agents.
pub const TURNS: u32 = 2;
/// Two outstanding turns leave one credit for the following call and result.
const WINDOW: u32 = TURNS + 1;
pub const HAND_OUTS: usize = 8;
// Each entrance produces only a handful of records in this script.
const OUTPUTS: u32 = 256;
const STEPS: usize = 128;

pub struct Driver {
    pub domain: Domain,
    pub requests: Vec<Request>,
    pub owed: VecDeque<Event>,
    pub handed: Vec<Token>,
    pub returned: Vec<Token>,
    pub owners: Vec<Token>,
    env: Env<temper_worker_domain::Limits>,
    out: Queue<Request>,
    immediate: VecDeque<Event>,
    agents: Vec<Token>,
}

impl Default for Driver {
    fn default() -> Self {
        Self::new()
    }
}

impl Driver {
    #[must_use]
    pub fn new() -> Self {
        let mut limits = crate::limits();
        limits.host.slots = u32::try_from(RUNS.len()).expect("two runs");
        limits.agent.agents = limits.host.slots;
        limits.host.turns = WINDOW;
        limits.agent.turns = WINDOW;
        let (encoded, owned) = crate::turn_bytes();
        limits.host.turn_queue_bytes = encoded * u64::from(WINDOW);
        limits.agent.unacknowledged_bytes = owned * u64::from(WINDOW);
        limits.bounds.unacknowledged = WINDOW;
        let mut driver = Self {
            domain: Domain::new(&limits),
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
            out: Queue::with_capacity(OUTPUTS),
            immediate: VecDeque::new(),
            owed: VecDeque::new(),
            requests: Vec::new(),
            agents: Vec::new(),
            handed: Vec::new(),
            returned: Vec::new(),
            owners: Vec::new(),
        };
        driver.send(Event::Ready);
        for run in RUNS {
            driver.send(Event::Link(host::LinkDown::Assign {
                assignment: host::Assignment {
                    assignment: host::RunAssignment {
                        identity: [7; 16],
                        live: host::LiveAsk::NONE,
                        run,
                        attempt: ATTEMPT,
                        workspace: None,
                        save: false,
                        charter: protocol::charter(),
                        grants: Box::new([]),
                    },
                    turns: Box::new([]),
                    answered: Box::new([]),
                    messages: Box::new([]),
                },
            }));
            driver.say(driver.agents.len() - 1, agent::Up::Admitted);
        }
        driver
    }

    pub fn send(&mut self, event: Event) {
        self.immediate.push_back(event);
        for _ in 0..STEPS {
            if let Some(event) = self.immediate.pop_front() {
                step(&mut self.domain, &self.env, event, &mut self.out);
            } else if self.domain.is_ready() {
                temper_worker_domain::resume(&mut self.domain, &self.env, &mut self.out);
            } else {
                return;
            }
            while let Some(request) = self.out.pop() {
                match request {
                    Request::Decode { agent, run, start } => {
                        self.owners.push(agent);
                        self.immediate.push_back(Event::Decoded {
                            agent,
                            start: protocol::decode(run, *start, &self.env.limits.agent),
                        });
                    }
                    Request::Encode { owner, value } => {
                        self.handed.push(owner);
                        self.owed.push_back(Event::Encoded { owner, encoded: protocol::encode(value) });
                    }
                    Request::Fact { owner, body } => {
                        self.immediate.push_back(Event::Activity { owner, activity: protocol::activity(&body) });
                    }
                    Request::Process(process) => self.process(&process),
                    request @ Request::Link(host::LinkUp::Turn { .. }) => {
                        if let Request::Link(host::LinkUp::Turn { run, attempt, turn }) = &request {
                            self.immediate.push_back(Event::Link(host::LinkDown::TurnKept {
                                run: *run,
                                attempt: *attempt,
                                turn: turn.turn,
                            }));
                        }
                        self.requests.push(request);
                    }
                    request @ Request::CancelRelay { .. } => {
                        if let Request::CancelRelay { run, attempt, call } = &request {
                            self.immediate.push_back(Event::RelayCancelled {
                                run: *run,
                                attempt: *attempt,
                                call: call.clone(),
                            });
                        }
                        self.requests.push(request);
                    }
                    request @ (Request::Source { .. }
                    | Request::SourceFault
                    | Request::Hello(_)
                    | Request::Link(_)
                    | Request::Answer { .. }
                    | Request::Stop
                    | Request::Io { .. }
                    | Request::CancelIo { .. }) => self.requests.push(request),
                }
            }
            self.domain.reclaim();
        }
        panic!("runtime order script settles within its finite budget");
    }

    fn process(&mut self, request: &agent::process::Request) {
        let event = match request {
            agent::process::Request::Spawn { owner, .. } => {
                self.agents.push(*owner);
                agent::process::Event::Spawned { owner: *owner, process: *owner }
            }
            agent::process::Request::Send { owner, .. } => agent::process::Event::Sent { owner: *owner },
            agent::process::Request::Signal { owner, .. } => agent::process::Event::Signalled { owner: *owner },
            agent::process::Request::Read { .. }
            | agent::process::Request::Wait { .. }
            | agent::process::Request::Reap { .. } => return,
        };
        self.immediate.push_back(Event::Process(Box::new(event)));
    }

    pub fn say(&mut self, index: usize, message: agent::Up) {
        let owner = *self.agents.get(index).expect("started agent");
        self.send(Event::Process(Box::new(agent::process::Event::Received { owner, message })));
    }

    pub fn gone(&mut self, index: usize) {
        let owner = *self.agents.get(index).expect("started agent");
        self.send(Event::Process(Box::new(agent::process::Event::Exited { owner, exit: Some(agent::Exit::Code(0)) })));
        self.send(Event::Process(Box::new(agent::process::Event::Hangup { owner })));
        self.send(Event::Process(Box::new(agent::process::Event::Reaped { owner, detail: Box::new([]) })));
    }

    pub fn release_one(&mut self) {
        let event = self.owed.pop_front().expect("held codec terminal");
        if let Event::Encoded { owner, .. } = &event {
            self.returned.push(*owner);
        }
        self.send(event);
    }
}
