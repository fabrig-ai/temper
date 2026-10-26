//! Direct root fixture shared by focused routes and the ending sweep.

use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{Duration, Env, Queue, ReplyTo, Time, Token, Wall};
use std::collections::VecDeque;
use temper_engine_domain as engine;
use temper_engine_domain::{Delivery, Key, Record, Write};

use crate::protocol::Protocol;
use crate::store::Store;
use crate::walking::{config, limits};

pub struct Driver {
    creating: jig_core_world::fixtures::Creating,
    pub forge: Option<crate::forge_stand::ForgeStand>,
    pub root: engine::Domain,
    pub env: Env<engine::Limits>,
    pub out: Queue<engine::Request>,
    pub events: VecDeque<engine::Event>,
    pub store: Store,
    pub delivered: Vec<Delivery>,
    /// The protocol layer this world plays: decodes, renders, outcomes, accounts.
    pub protocol: Protocol,
    pub serial: u64,
    pub stopped: bool,
    pub fail_archive_once: bool,
    pub transactions: Vec<Vec<Write>>,
    pub host_requests: Vec<Box<engine::HostRequest>>,
    /// Every store load the root asked for, by its owner, in order.
    pub loads: Vec<(Token, temper_engine_domain::Range)>,
    /// How many inputs `try_step` handed back while the journal was full; the
    /// world offers each again later.
    pub returned: u32,
    /// Other outputs without a durability dependency.
    pub immediate: Vec<engine::Request>,
    /// Whether the deployment's first period was sent once the root was ready.
    pub period_sent: bool,
}

impl Driver {
    #[must_use]
    pub fn new(store: Store) -> Driver {
        Driver::configured(store, [config(91), config(91)], &limits())
    }
    #[must_use]
    pub fn forge() -> Driver {
        let mut driver = Driver::configured(Store::new(), [config(71), config(71)], &crate::fixtures::forge_limits());
        driver.forge = Some(crate::forge_stand::ForgeStand::new());
        driver
    }

    /// Independent constructions of the same full configuration: bootstrap and cold restore.
    /// Fixture-only array builders keep that constructor in one place rather than cloning
    /// jig's Config field by field (a test-only closure departure from programming-model.md, 1).
    #[must_use]
    pub fn configured(mut store: Store, configs: [engine::Config; 2], limits: &engine::Limits) -> Driver {
        let [bootstrap, config] = configs;
        if store.rows.is_empty() {
            store = crate::fixtures::restored_over(store, bootstrap, limits);
        }
        Self::cold(store, config, limits)
    }

    /// An empty deployment for stories that exercise the real first-start cut itself.
    #[must_use]
    pub fn cold(store: Store, config: engine::Config, limits: &engine::Limits) -> Driver {
        Driver {
            creating: jig_core_world::fixtures::Creating::new(&config.core, limits.tasks.tree_tasks),
            forge: None,
            root: engine::Domain::new(config, limits).expect("valid engine configuration"),
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits: *limits },
            out: Queue::with_capacity(engine::max_out(limits)),
            events: VecDeque::from([engine::Event::Restart]),
            store,
            delivered: Vec::new(),
            protocol: Protocol::default(),
            serial: 100,
            stopped: false,
            fail_archive_once: false,
            transactions: Vec::new(),
            host_requests: Vec::new(),
            loads: Vec::new(),
            returned: 0,
            immediate: Vec::new(),
            period_sent: false,
        }
    }
    #[must_use]
    pub fn with_repositories(
        store: Store,
        configs: [engine::Config; 2],
        limits: &engine::Limits,
        repositories: &[(&[u8], &[u8], u32)],
    ) -> Driver {
        let mut driver = Self::configured(store, configs, limits);
        driver.forge = Some(crate::forge_stand::ForgeStand::with(repositories));
        driver
    }

    /// Offer one input through admission; an input the journal cannot take
    /// is handed back to the world, which keeps it (jig's `domain/root.md`, 3).
    pub fn send(&mut self, event: engine::Event) {
        if let Err(event) = engine::try_step(&mut self.root, &self.env, event) {
            self.returned += 1;
            self.events.push_back(event);
        }
        engine::release(&mut self.root, &self.env, &mut self.out);
        self.collect();
        self.root.reclaim();
    }

    pub fn collect(&mut self) {
        for _ in 0..self.out.len() {
            let request = self.out.pop().expect("output count");
            let request = if let engine::Request::Store(engine::StoreRequest::Read { load, range }) = request {
                let result = self.store.read(range);
                self.events.push_back(engine::Event::Store(engine::StoreInput::ReadLoaded { load, result }));
                continue;
            } else {
                request
            };
            let request = match self.protocol.play(request) {
                Ok(events) => {
                    self.events.extend(events);
                    continue;
                }
                Err(request) => *request,
            };
            match request {
                engine::Request::ToChild(released) => self.events.push_back(engine::Event::Released(released)),
                engine::Request::Store(engine::StoreRequest::Commit { number, writes }) => {
                    self.transactions.push(writes.to_vec());
                    self.store.pending.push_back((number, writes));
                }
                engine::Request::Store(engine::StoreRequest::Load { owner, range, after, most, bytes }) => {
                    self.loads.push((owner, range.clone()));
                    let (rows, next) = self.store.page(&range, after.as_ref(), most);
                    assert!(rows.len() <= usize::try_from(most).expect("small count"));
                    assert!(bytes >= self.env.limits.loads.bytes);
                    if self.fail_archive_once
                        && matches!(
                            range,
                            temper_engine_domain::Range::Core(
                                temper_engine_domain::CoreRange::EscalationDecision { .. }
                            )
                        )
                    {
                        self.fail_archive_once = false;
                        self.events.push_back(engine::Event::Store(engine::StoreInput::Unloaded { owner }));
                    } else {
                        let texts = transcript_texts(&range, &rows);
                        self.events.push_back(engine::Event::Store(engine::StoreInput::Loaded {
                            owner,
                            rows,
                            next,
                            texts,
                        }));
                    }
                }
                engine::Request::Worker(engine::WorkerRequest::Deliver(delivery))
                | engine::Request::Party(engine::PartyRequest::Deliver(delivery)) => {
                    self.delivered.push(delivery);
                }
                engine::Request::Forge { call, repository, op } => {
                    self.forge.as_mut().expect("root route fixture has a forge").call(
                        call,
                        repository,
                        op,
                        &self.env.limits.forge.client,
                    );
                }
                engine::Request::Stop => self.stopped = true,
                engine::Request::Worker(engine::WorkerRequest::Host(request)) => self.host_requests.push(request),
                other @ (engine::Request::Party(_)
                | engine::Request::Worker(_)
                | engine::Request::Store(_)
                | engine::Request::Account(_)
                | engine::Request::StopGroup { .. }
                | engine::Request::SetUpGroup { .. }
                | engine::Request::Connect { .. }
                | engine::Request::Manifest { .. }
                | engine::Request::Reachable { .. }
                | engine::Request::Install { .. }
                | engine::Request::EraseCarried { .. }
                | engine::Request::SetUp { .. }
                | engine::Request::Bound { .. }
                | engine::Request::Return { .. }
                | engine::Request::Setup { .. }
                | engine::Request::Busy(_)
                | engine::Request::Unsettled(_)
                | engine::Request::Drained) => self.immediate.push(other),
            }
        }
    }

    pub fn advance(&mut self, apply: bool) {
        if let Some(forge) = &mut self.forge {
            self.events.extend(forge.replies());
        }
        if !self.period_sent && self.root.ready() {
            self.period_sent = true;
            self.period();
        }
        if let Some(event) = self.events.pop_front() {
            self.send(event);
        }
        if apply && !self.store.pending.is_empty() {
            let number = self.store.apply();
            self.send(engine::Event::Store(engine::StoreInput::Committed { number, fill: 0 }));
        }
        crate::poll(&mut self.root, &self.env, &mut self.out);
        self.collect();
        self.root.reclaim();
    }

    /// Drive immediate work while a story deliberately keeps a fleet deadline armed.
    pub fn pump(&mut self) {
        const STEPS: usize = 200; // The direct fixture's settlement step budget.
        for _ in 0..STEPS {
            self.advance(true);
        }
        assert!(self.events.is_empty() && self.store.pending.is_empty(), "immediate work drained: {:?}", self.root);
    }

    /// Fire timers at an absolute simulated time; the caller drives their outputs.
    pub fn fire_at(&mut self, elapsed: Duration) {
        self.env.now = Time::from_nanos(elapsed.as_nanos());
        self.env.wall = Wall::from_nanos(elapsed.as_nanos());
        engine::fire(&mut self.root, &self.env);
        engine::release(&mut self.root, &self.env, &mut self.out);
        self.collect();
    }

    pub fn settle(&mut self) {
        for _ in 0..200 {
            self.advance(true);
            if self.root.quiescent()
                && self.events.is_empty()
                && self.store.pending.is_empty()
                && self.forge.as_ref().is_none_or(crate::forge_stand::ForgeStand::quiescent)
            {
                return;
            }
        }
        panic!("direct root routes did not settle: {:?}", self.root);
    }

    /// One worker's empty hello on `channel`.
    pub fn hello(&mut self, channel: Token) {
        self.send(crate::protocol::hello(channel, Box::new([])));
    }

    /// A run's call of one engine tool, named by `call`.
    pub fn call(&mut self, channel: Token, task: u64, attempt: u64, call: u64, body: engine::Call) {
        let event = self.protocol.call(channel, task, attempt, call, body);
        self.send(event);
    }

    /// A run's answer, its outcome read back as `end`, temper's parts beside it.
    pub fn answer(
        &mut self,
        channel: Token,
        task: u64,
        attempt: u64,
        cumulative: u64,
        end: tasks::End,
        parts: engine::Parts,
    ) {
        let event = self.protocol.answer(channel, task, attempt, cumulative, end, parts);
        self.send(event);
    }

    /// The typed answer the root decided for the call named `call`, once rendered.
    #[must_use]
    pub fn answered(&self, call: u64) -> Option<&engine::CallAnswer> {
        let name = call.to_be_bytes();
        self.protocol.answers.iter().rev().find_map(|(named, answer)| (named.as_ref() == name).then_some(answer))
    }

    /// The deployment's first period for project one, at its first start.
    pub fn period(&mut self) {
        self.send(engine::Event::Party(engine::PartyInput::Period { project: 1, period: 1, budget: 1000, next: None }));
    }

    pub fn sign_in(&mut self) {
        self.serial += 1;
        self.send(engine::Event::Party(engine::PartyInput::SignedIn {
            held: None,
            reply_to: ReplyTo::new(Token::new(self.serial)),
            identity: people::Identity {
                key: people::IdentityKey { provider: 0, subject: 7_u64.to_be_bytes().into() },
                login: b"person".as_slice().into(),
                name: b"Person".as_slice().into(),
            },
        }));
    }

    /// A signed-in owner's chat, assigned to the one worker on channel 7.
    #[must_use]
    pub fn chat() -> (Driver, engine::Assignment) {
        Self::configured_chat([config(91), config(91)], &limits())
    }

    /// A chat with the story's bounded configuration.
    #[must_use]
    pub fn configured_chat(
        configuration: [engine::Config; 2],
        bounds: &engine::Limits,
    ) -> (Driver, engine::Assignment) {
        let mut driver = Driver::configured(Store::new(), configuration, bounds);
        driver.hello(Token::new(7));
        driver.settle();
        driver.sign_in();
        driver.settle();
        driver.send(engine::Event::Party(engine::PartyInput::Ask {
            models: Box::new([]),
            reply_to: ReplyTo::new(Token::new(1001)),
            sign_in: driver.session(),
            key: [31; 16],
            ask: driver.chat_ask(1, crate::walking_referee::QUESTION),
        }));
        driver.settle();
        let assignment = driver.last_assignment();
        (driver, assignment)
    }

    #[must_use]
    pub fn chat_ask(&self, project: u32, words: &[u8]) -> people::Ask {
        crate::walking::configured_chat(
            &self.creating,
            &self.store.rows,
            self.env.limits.tasks.tree_tasks,
            project,
            words,
        )
    }

    /// `task`'s durable live row, if it is live.
    #[must_use]
    pub fn task(&self, task: u64) -> Option<&tasks::TaskRecord> {
        let key = Key::Core(jig_domain_core::Key::Tasks(tasks::Key::Live(task)));
        if let Some(Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Live(row)))) = self.store.rows.get(&key)
        {
            Some(row)
        } else {
            None
        }
    }

    /// The latest durable assignment.
    #[must_use]
    pub fn last_assignment(&self) -> engine::Assignment {
        self.delivered
            .iter()
            .rev()
            .find_map(|delivery| {
                if let Delivery::Assigned { assignment, .. } = delivery { Some(assignment.clone()) } else { None }
            })
            .expect("durable assignment")
    }

    /// The owner's words `[key]` to `task`, settled; the message's number.
    pub fn say(&mut self, task: u64, key: u8) -> u64 {
        self.send(say_ask(self.session(), task, key));
        self.settle();
        self.delivered
            .iter()
            .rev()
            .find_map(|delivery| {
                if let Delivery::WebReply {
                    reply: people::Reply::Outcome(people::Outcome::Said { task: named, message, .. }),
                    ..
                } = delivery
                    && *named == task
                {
                    Some(*message)
                } else {
                    None
                }
            })
            .expect("committed keyed word")
    }

    #[must_use]
    pub fn session(&self) -> u64 {
        self.delivered
            .iter()
            .find_map(|delivery| {
                if let Delivery::WebReply { reply: people::Reply::SignedIn { sign_in, .. }, .. } = delivery {
                    Some(*sign_in)
                } else {
                    None
                }
            })
            .expect("durable sign-in reply")
    }

    // Leave the newly durable callback behind three later unanswered commits.
    pub fn pressure(&mut self) {
        assert_eq!(self.store.pending.len(), 1);
        self.sign_in();
        self.sign_in();
        assert_eq!(self.store.pending.len(), 3);
        let number = self.store.apply();
        self.send(engine::Event::Store(engine::StoreInput::Committed { number, fill: 0 }));
        self.sign_in();
        assert_eq!(self.store.pending.len(), 3);
        for _ in 0..20 {
            self.advance(false);
        }
        assert!(!self.root.quiescent(), "owed callback and store terminals prevent completion");
        assert_eq!(self.store.pending.len(), 3, "no callback write entered the full journal");
    }
}

/// The owner's words `[key]` to `task`.
#[must_use]
pub fn say_ask(sign_in: u64, task: u64, key: u8) -> engine::Event {
    engine::Event::Party(engine::PartyInput::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(2000 + u64::from(key))),
        sign_in,
        key: [key; 16],
        ask: people::Ask::Say { project: 1, task, words: Box::from([key]) },
    })
}

/// The service renders a transcript window's turns as text (temper-service
/// store/ranges.rs); this world's turns are text already, so each turn's
/// rendering is its own bytes' end within the tail.
fn transcript_texts(range: &temper_engine_domain::Range, rows: &[Record]) -> Option<engine::TranscriptTexts> {
    if !matches!(range, temper_engine_domain::Range::Core(temper_engine_domain::CoreRange::TaskTranscript { .. })) {
        return None;
    }
    let tail = usize::try_from(engine::TRANSCRIPT_TAIL_BYTES).expect("tail fits usize");
    Some(
        rows.iter()
            .map(|row| {
                let Record::Core(jig_domain_core::Record::Core(jig_domain_core::CoreRecord::Turn(turn))) = row else {
                    return None;
                };
                let start = turn.transcript.len().saturating_sub(tail);
                Some(Box::from(&turn.transcript[start..]))
            })
            .collect(),
    )
}
