//! The protocol layer the engine world plays (testing-strategy.md, 4): the
//! hub's link records a worker sends, the decode and render of engine tools,
//! the reading of an ended run's outcome, and the accounts component.
//! jig's worlds play the same parts (`testing/jig-test-service`).

use jig_domain_accounts as accounts;
use jig_domain_core as core;
use jig_domain_host as host;
use jig_domain_tasks as tasks;
use skein_lib::{Duration, ReplyTo, Token};
use std::collections::{BTreeMap, BTreeSet};
use temper_engine_domain as engine;

/// The bounds every scripted worker says in its hello: wide enough for the
/// world's one charter (jig's `domain/hosts.md`, 8).
#[must_use]
pub fn bounds() -> host::Bounds {
    host::Bounds {
        program_endpoints: Box::new([]),
        models: Box::new([host::ModelBound { name: b"scripted".as_slice().into(), window: 1024, output: 1024 }]),
        turns: 8,
        spend: 1000,
        time: Duration::from_secs(60),
        waiting: Duration::from_secs(60),
        // Checks finish within a changing charter's reserved time.
        checks: Duration::from_secs(10),
        message_bytes: 128,
        inbox: 4,
        unacknowledged: 8,
    }
}

/// One worker's hello on `channel`, listing the runs it hosts.
#[must_use]
pub fn hello(channel: Token, hosting: Box<[host::Hosting]>) -> engine::Event {
    hello_slots(channel, 1, hosting)
}

/// One worker's hello on `channel` with `slots` slots, listing the runs it hosts.
#[must_use]
pub fn hello_slots(channel: Token, slots: u32, hosting: Box<[host::Hosting]>) -> engine::Event {
    hello_at(channel, 0, slots, hosting)
}

/// A configured worker's hello at `worker`, with its slots and hosted runs.
#[must_use]
pub fn hello_at(channel: Token, worker: u32, slots: u32, hosting: Box<[host::Hosting]>) -> engine::Event {
    link(
        channel,
        engine::Link::Hello {
            worker,
            hello: host::Hello {
                slots,
                stop_bound: Duration::from_millis(100),
                workstreams: Box::new([]),
                hosting,
                bounds: bounds(),
            },
        },
    )
}

/// A hosted run in a worker's hello.
#[must_use]
pub fn hosting(task: u64, attempt: u64, phase: host::Phase) -> host::Hosting {
    host::Hosting { run: Token::new(task), attempt: Token::new(attempt), phase }
}

/// A worker channel closing.
#[must_use]
pub fn lost(channel: Token) -> engine::Event {
    link(channel, engine::Link::Lost)
}

/// One numbered turn of a run: its cumulative spend, the last message it
/// read, and its opaque body. The first turn opens the run's window.
#[must_use]
pub fn turn(
    channel: Token,
    task: u64,
    attempt: u64,
    number: u32,
    cumulative: u64,
    read: Option<u64>,
    body: &[u8],
) -> engine::Event {
    up(
        channel,
        host::LinkUp::Turn {
            run: Token::new(task),
            attempt: Token::new(attempt),
            turn: host::Turn {
                opens_window: number == 1,
                refused: Box::new([]),
                turn: number,
                spent: cumulative,
                read: read.map(Token::new),
                body: body.into(),
            },
        },
        engine::Parts::none(),
    )
}

/// One hub record with no application part.
#[must_use]
pub fn up(channel: Token, up: host::LinkUp, parts: engine::Parts) -> engine::Event {
    link(channel, engine::Link::Up { up, parts })
}

fn link(channel: Token, link: engine::Link) -> engine::Event {
    engine::Event::Worker(engine::WorkerInput::Link { channel, link })
}

/// What the world's protocol layer keeps: tool bodies by call name, ended
/// runs' terminals by outcome bytes, and every typed answer it rendered.
#[derive(Debug, Default)]
pub struct Protocol {
    pub sign_in_sources: BTreeSet<u32>,
    calls: BTreeMap<Box<[u8]>, engine::Call>,
    undecodable: BTreeSet<Box<[u8]>>,
    ends: BTreeMap<Box<[u8]>, tasks::End>,
    serial: u64,
    /// Each decided call's typed answer, by its opaque call name, in render order.
    pub answers: Vec<(Box<[u8]>, engine::CallAnswer)>,
    /// Let a story cut between the durable decision and its rendered answer.
    pub defer_render: bool,
    pub pending_renders: Vec<(ReplyTo, Box<[u8]>)>,
    /// Each account component request the root made.
    pub accounts: Vec<accounts::Request>,
    pub key_failure: Option<accounts::KeyFailure>,
    pub returns: Vec<(u32, Box<[u8]>)>,
    /// Worker channels that received their program-source inventory.
    pub sources: Vec<Token>,
    /// Core read loads the world answered as gone.
    pub reads: Vec<core::reads::Range>,
}

impl Protocol {
    /// A run's call that the world's protocol answers as an undecodable tool.
    pub fn undecodable(&mut self, channel: Token, task: u64, attempt: u64, name: u64) -> engine::Event {
        let call: Box<[u8]> = name.to_be_bytes().into();
        self.undecodable.insert(call.clone());
        up(
            channel,
            host::LinkUp::Call {
                run: Token::new(task),
                attempt: Token::new(attempt),
                call,
                tool: b"unknown".as_slice().into(),
                writes: true,
                input: Box::new([]),
                deadline: Duration::from_secs(10),
            },
            engine::Parts::none(),
        )
    }

    /// A run's call of one engine tool, named `name`; its decoded body waits
    /// for the root's decode request.
    pub fn call(&mut self, channel: Token, task: u64, attempt: u64, name: u64, body: engine::Call) -> engine::Event {
        let call: Box<[u8]> = name.to_be_bytes().into();
        self.calls.insert(call.clone(), body);
        up(
            channel,
            host::LinkUp::Call {
                run: Token::new(task),
                attempt: Token::new(attempt),
                call,
                tool: b"engine".as_slice().into(),
                writes: true,
                input: Box::new([]),
                deadline: Duration::from_secs(10),
            },
            engine::Parts::none(),
        )
    }

    /// A run's answer: the hub carries its outcome as opaque bytes, which the
    /// world's protocol reads back as `end`; temper's parts ride beside it.
    pub fn answer(
        &mut self,
        channel: Token,
        task: u64,
        attempt: u64,
        cumulative: u64,
        end: tasks::End,
        parts: engine::Parts,
    ) -> engine::Event {
        self.serial += 1;
        let outcome: Box<[u8]> = self.serial.to_be_bytes().into();
        self.ends.insert(outcome.clone(), end);
        let answer = host::Answer {
            read: None,
            refused: Box::new([]),
            turns: 0,
            spent: cumulative,
            ending: host::Ending::Ended { outcome, work: host::Work { left: None, saved: None } },
        };
        up(channel, host::LinkUp::Answer { run: Token::new(task), attempt: Token::new(attempt), answer }, parts)
    }

    /// Play one root request this layer answers; return any other to the caller.
    ///
    /// # Errors
    /// Returns the request when it is not the protocol layer's to answer.
    pub fn play(&mut self, request: engine::Request) -> Result<Vec<engine::Event>, Box<engine::Request>> {
        match request {
            engine::Request::Worker(engine::WorkerRequest::Deliver(engine::Delivery::Link {
                channel,
                down: host::LinkDown::Sources { sources: _ },
            })) => {
                self.sources.push(channel);
                // This scripted worker supplies an empty inventory, including for newly
                // declared sources: no login has been observed on it.
                Ok(vec![up(channel, host::LinkUp::Logins { logins: Box::new([]) }, engine::Parts::none())])
            }
            engine::Request::Worker(engine::WorkerRequest::Host(host_request)) => match *host_request {
                engine::HostRequest::Decode { to, task, attempt, call } => match self.calls.get(&call.name) {
                    Some(body) => {
                        Ok(vec![engine::Event::Worker(engine::WorkerInput::DecodedHostCall { to, body: body.clone() })])
                    }
                    None if self.undecodable.contains(&call.name) => {
                        Ok(vec![engine::Event::Worker(engine::WorkerInput::RenderedHostCall {
                            to,
                            answer: core::SettledAnswer::Host { error: true, body: b"unknown tool".as_slice().into() },
                        })])
                    }
                    None => Err(Box::new(engine::Request::Worker(engine::WorkerRequest::Host(Box::new(
                        engine::HostRequest::Decode { to, task, attempt, call },
                    ))))),
                },
                engine::HostRequest::Dropped { .. } => Ok(Vec::new()),
            },
            engine::Request::Worker(engine::WorkerRequest::Deliver(engine::Delivery::Host(delivery))) => {
                match *delivery {
                    engine::HostDelivery::Render { to, name, answer, .. } => {
                        self.answers.push((name.clone(), answer));
                        if self.defer_render {
                            self.pending_renders.push((to, name));
                            Ok(Vec::new())
                        } else {
                            Ok(vec![rendered(to, name)])
                        }
                    }
                    other @ engine::HostDelivery::Answer { .. } => Err(Box::new(engine::Request::Worker(
                        engine::WorkerRequest::Deliver(engine::Delivery::Host(Box::new(other))),
                    ))),
                }
            }
            engine::Request::Worker(engine::WorkerRequest::ReadResult { payload, outcome }) => {
                let end = self.ends.get(&outcome).expect("the world wrote this outcome").clone();
                Ok(vec![engine::Event::Worker(engine::WorkerInput::ResultRead { payload, end })])
            }
            engine::Request::Store(engine::StoreRequest::Read { load, range }) => {
                self.reads.push(*range);
                Ok(vec![engine::Event::Store(engine::StoreInput::ReadLoaded {
                    load,
                    result: core::reads::Loaded::Gone,
                })])
            }
            engine::Request::Return { source, path } => {
                self.returns.push((source, path));
                Ok(Vec::new())
            }
            engine::Request::Account(request) => {
                let answer = self.account(&request);
                self.accounts.push(request);
                Ok(answer)
            }
            other @ (engine::Request::Store(_)
            | engine::Request::Worker(_)
            | engine::Request::Party(_)
            | engine::Request::Forge { .. }
            | engine::Request::StopGroup { .. }
            | engine::Request::SetUpGroup { .. }
            | engine::Request::Connect { .. }
            | engine::Request::Manifest { .. }
            | engine::Request::Reachable { .. }
            | engine::Request::Install { .. }
            | engine::Request::EraseCarried { .. }
            | engine::Request::SetUp { .. }
            | engine::Request::Bound { .. }
            | engine::Request::Setup { .. }
            | engine::Request::ToChild(_)
            | engine::Request::Busy(_)
            | engine::Request::Unsettled(_)
            | engine::Request::Drained
            | engine::Request::Stop) => Err(Box::new(other)),
        }
    }

    /// The scripted account component's answer to one accounts request.
    fn account(&self, request: &accounts::Request) -> Vec<engine::Event> {
        match request {
            accounts::Request::Signing(accounts::Signing::Keep { source, .. }) => match self.key_failure {
                Some(why) => {
                    vec![engine::Event::Account(accounts::Event::Component(accounts::FromComponent::KeepFailed {
                        account: *source,
                        why,
                    }))]
                }
                None => vec![granted(*source, 1, false)],
            },
            accounts::Request::Component(accounts::ToComponent::Want { account }) => {
                vec![granted(*account, 1, self.sign_in_sources.contains(account))]
            }
            accounts::Request::Component(accounts::ToComponent::Rejected { account, generation }) => {
                vec![granted(*account, generation + 1, self.sign_in_sources.contains(account))]
            }
            accounts::Request::Component(accounts::ToComponent::Release { .. } | accounts::ToComponent::Close)
            | accounts::Request::Programs { .. }
            | accounts::Request::CheckProgram { .. }
            | accounts::Request::SignOutProgram { .. }
            | accounts::Request::Granted { .. }
            | accounts::Request::Save { .. }
            | accounts::Request::Erase { .. }
            | accounts::Request::Signing(_)
            | accounts::Request::Answer { .. }
            | accounts::Request::Closed => Vec::new(),
        }
    }
}

fn granted(account: u32, generation: u64, sign_in: bool) -> engine::Event {
    engine::Event::Account(accounts::Event::Component(accounts::FromComponent::Granted {
        form: accounts::Form::Credential(if sign_in { accounts::Scheme::Bearer } else { accounts::Scheme::Key }),
        account,
        generation,
        valid: Duration::from_secs(3600),
    }))
}

fn rendered(to: ReplyTo, name: Box<[u8]>) -> engine::Event {
    engine::Event::Worker(engine::WorkerInput::RenderedHostCall {
        to,
        answer: core::SettledAnswer::Host { error: false, body: name },
    })
}
