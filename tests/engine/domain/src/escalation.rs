//! Real root/children, two authenticated owners, paged durable fake store and
//! scripted priced worker for held-chat decisions. Service policy and transport
//! are driven only through actual boundaries.

use crate::escalation_referee::{QUESTION, REASON, REPORT, Referee, Story};
use crate::protocol::{self, Protocol};
use crate::store::Store;
use crate::walking;
use jig_domain_authority as authority;
use jig_domain_fleet as fleet;
use jig_domain_host as host;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use skein_lib::{Env, Queue, ReplyTo, Time, Token, Wall};
use std::collections::{BTreeMap, VecDeque};
use temper_engine_domain as engine;
use temper_engine_domain::{Delivery, Record, Write};

/// One durable process cut, without changing the outside scripts or store
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cut {
    /// Keep the process throughout.
    None,

    /// Lose completion after priced failure and requester hold apply, before ACK
    Held,

    /// Lose completion after the first accepted decision's atomic archive applies,
    /// before its keyed reply escapes.
    Decision,
}

/// Finite deterministic scenario configuration, with tiny pages and optional
/// diagnostic saturation (testing-strategy.md, section 3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Settings {
    /// Real child's reproducible seed.
    pub seed: u64,

    /// Outside people decisions, including both race outcomes.
    pub story: Story,

    /// At most one durable cut.
    pub cut: Cut,

    /// Extra iterations before applying each submitted commit.
    pub commit_delay: u32,

    /// Extra iterations before returning a captured one-row page.
    pub page_delay: u32,

    /// Drain facts or deliberately saturate their tiny queues.
    pub facts: bool,
}

impl Settings {
    /// Calm no-cut story; callers vary only outside configuration
    #[must_use]
    pub const fn calm(seed: u64, story: Story) -> Settings {
        Settings { seed, story, cut: Cut::None, commit_delay: 0, page_delay: 0, facts: true }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum WorkerAnswer {
    Idle,
    Retained,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SignInScript {
    Waiting,
    Sent,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Recovery {
    None,
    Pending,
    Sent,
}

#[derive(Clone, Debug)]
struct AskInput {
    owner: usize,
    key: [u8; 16],
    ask: people::Ask,
}

#[derive(Clone, Copy, Debug)]
enum ReadPurpose {
    Requester,
    FinalRole,
    Rejected,
}

/// A held chat the scripted person waits to see; the root's reads are the
/// core's, so the world sees it in its durable row (jig's
/// `client/domain/contract.md`, 6).
#[derive(Clone, Debug)]
struct PendingRead {
    to: Token,
    owner: usize,
    escalation: tasks::Escalation,
    purpose: ReadPurpose,
}

/// Frozen world including real root state, store and pending outside scripts.
/// Referee predictions come from inputs and durable boundary evidence, never
/// from a private-state query.
#[derive(Debug)]
pub struct World {
    settings: Settings,
    domain: engine::Domain,
    environment: Env<engine::Limits>,
    out: Queue<engine::Request>,
    events: VecDeque<(u32, engine::Event)>,
    sessions: [Option<u64>; 2],
    people: [Option<u64>; 2],
    task: Option<u64>,
    assignment: Option<engine::Assignment>,
    assigned: usize,
    worker_answer: WorkerAnswer,
    asks: BTreeMap<Token, AskInput>,
    ask_rights: BTreeMap<Token, Token>,
    next_ask: u64,
    pending_read: Option<PendingRead>,
    sign_in_script: SignInScript,
    first_choice: Option<(usize, u64, people::EscalationDecision, u8)>,
    iteration: u32,
    commit_wait: u32,
    clock_offset: u64,
    recovery: Recovery,
    frozen_cut: Option<Cut>,
    /// Ordered atomic fake store, shared with existing journal/load worlds
    pub store: Store,

    /// Outside obligations and independent committed evidence checks
    pub referee: Referee,

    /// Outside observer immediately before the one unassigned-claim refusal
    /// cohort, retained for positive-control and corruption checks; never a
    /// root-state oracle.
    pub unplaced_referee: Option<Referee>,

    /// Exact boundary trace, excluding optional diagnostic observations
    pub trace: Vec<String>,

    /// Submitted atomic cohorts retained for meaningful corruption negatives
    pub transactions: Vec<Vec<Write>>,

    /// Actual process cuts injected by this world.
    pub restarts: u32,

    /// Real paged requests, including named archive IO.
    pub pages: u32,

    protocol: Protocol,
    period_sent: bool,
}

// Initial configuration grants acceptance of escalation proposals without the
// separate direct Release permission. Restart reconstructs this same policy.
fn config(seed: u64) -> engine::Config {
    let mut config = walking::config(seed);
    config.core.owners = Box::new([
        people::InitialOwner {
            project: 1,
            identity: people::IdentityKey { provider: 0, subject: 7_u64.to_be_bytes().into() },
        },
        people::InitialOwner {
            project: 1,
            identity: people::IdentityKey { provider: 0, subject: 8_u64.to_be_bytes().into() },
        },
    ]);
    let mut policy = config.core.seed_projects[0].policy.policy.clone();
    policy.roles[0].requests = authority::Requests(1 | 4);
    assert!(policy.roles[0].requests.allows(authority::RequestKind::Create));
    assert!(policy.roles[0].requests.allows(authority::RequestKind::Accept));
    assert!(!policy.roles[0].requests.allows(authority::RequestKind::Release));
    assert!(policy.roles[0].decides.allows(authority::ProposalKind::Escalation));
    config.core.seed_projects[0].policy.policy = policy;
    config
}

/// Exact configured child and root capacities for this finite escalation world;
/// memory and route regressions reuse them rather than drift from the driver
#[must_use]
pub fn limits() -> engine::Limits {
    let mut limits = walking::limits();
    limits.tasks.retries.run.retries = 0;
    limits.people.initial_owners = 2;
    limits.people.requests = 8;
    limits.journal.writes = tasks::max_out(&limits.tasks) * 8
        + people::max_out(&limits.people) * 4
        + limits.people.pending * 2
        + fleet::max_out(&limits.fleet) * 4
        + temper_engine_domain_forge::max_out(&limits.forge)
        + 2 * limits.authority.projects
        + limits.tasks.tasks
        + 6;
    limits
}

impl World {
    /// Build actual children/policy, with two initial owners and zero run retries.
    /// Initial policy grants Create/Accept and escalation decisions, omitting direct Release.
    /// Outside requests use the authenticated root boundary
    #[must_use]
    pub fn new(settings: Settings) -> World {
        let limits = limits();
        let config = config(settings.seed);
        let store = crate::fixtures::restored_store(self::config(settings.seed), &limits);
        let mut world = World {
            domain: engine::Domain::new(config, &limits).expect("valid engine configuration"),
            environment: Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
            out: Queue::with_capacity(engine::max_out(&limits)),
            events: VecDeque::new(),
            sessions: [None, None],
            people: [None, None],
            task: None,
            assignment: None,
            assigned: 0,
            worker_answer: WorkerAnswer::Idle,
            asks: BTreeMap::new(),
            ask_rights: BTreeMap::new(),
            next_ask: 400,
            pending_read: None,
            sign_in_script: SignInScript::Waiting,
            first_choice: None,
            iteration: 0,
            commit_wait: 0,
            clock_offset: 0,
            recovery: Recovery::None,
            frozen_cut: None,
            store,
            referee: Referee::new(settings.story),
            unplaced_referee: None,
            trace: Vec::new(),
            transactions: Vec::new(),
            restarts: 0,
            pages: 0,
            protocol: Protocol::default(),
            period_sent: false,
            settings,
        };
        world.queue(engine::Event::Restart { pending: world.store.pending_generation }, 0);
        world.hello();
        world
    }

    fn queue(&mut self, event: engine::Event, delay: u32) {
        self.events.push_back((self.iteration + delay + 1, event));
    }

    fn hello(&mut self) {
        let hosting: Box<[host::Hosting]> = if self.worker_answer == WorkerAnswer::Retained {
            let assignment = self.assignment.as_ref().expect("outside worker retains its answer");
            Box::new([protocol::hosting(assignment.task, assignment.attempt, host::Phase::Answered)])
        } else {
            Box::new([])
        };
        self.queue(protocol::hello(Token::new(7), hosting), 0);
    }

    fn sign_in(&mut self, index: usize) {
        self.queue(
            engine::Event::Party(engine::PartyInput::SignedIn {
                held: None,
                reply_to: ReplyTo::new(Token::new(101 + u64::try_from(index).expect("two people"))),
                identity: people::Identity {
                    key: people::IdentityKey {
                        provider: 0,
                        subject: (7 + u64::try_from(index).expect("two people")).to_be_bytes().into(),
                    },
                    login: format!("owner{index}").into_bytes().into(),
                    name: format!("Owner {index}").into_bytes().into(),
                },
            }),
            0,
        );
    }

    fn ask_input(&mut self, to: Token, input: &AskInput) {
        self.queue(
            engine::Event::Party(engine::PartyInput::Ask {
                models: Box::new([]),
                reply_to: ReplyTo::new(to),
                sign_in: self.sessions[input.owner].expect("authenticated outside owner"),
                key: input.key,
                ask: input.ask.clone(),
            }),
            0,
        );
    }

    fn retry_ask(&mut self, to: Token, reply: &people::Reply) {
        assert!(self.next_ask < 416, "bounded named-query pressure retries");
        let fresh = Token::new(self.next_ask);
        self.next_ask += 1;
        self.referee.ask_refused(to, reply, fresh).expect("one retryable named-query right");
        let logical = self.ask_rights.remove(&to).unwrap_or(to);
        assert!(self.ask_rights.insert(fresh, logical).is_none());
        let input = self.asks.remove(&to).expect("one pending named query");
        self.ask_input(fresh, &input);
        assert!(self.asks.insert(fresh, input).is_none());
    }

    fn ask(
        &mut self,
        to: u64,
        owner: usize,
        key: u8,
        revision: u64,
        decision: people::EscalationDecision,
        outcome: people::Outcome,
    ) {
        let token = Token::new(to);
        let ask =
            people::Ask::DecideEscalation { project: 1, task: self.task.expect("chat started"), revision, decision };
        let input = AskInput { owner, key: [key; 16], ask: ask.clone() };
        self.referee.ask(token, self.people[owner].expect("signed-in actor"), input.key, ask, outcome);
        assert!(self.asks.insert(token, input.clone()).is_none(), "fresh outside request right");
        self.ask_input(token, &input);
    }

    fn read(&mut self, to: u64, owner: usize, escalation: tasks::Escalation) {
        let purpose = match to {
            201 => ReadPurpose::Requester,
            202 => ReadPurpose::FinalRole,
            204 => ReadPurpose::Rejected,
            _ => panic!("named initial read purpose"),
        };
        self.read_input(PendingRead { to: Token::new(to), owner, escalation, purpose }, 0);
    }

    fn read_input(&mut self, read: PendingRead, _delay: u32) {
        assert!(self.pending_read.is_none(), "at most one current-view script obligation");
        let person = self.people[read.owner].expect("signed-in named reader");
        self.referee.read(read.to, person, read.escalation.clone());
        self.pending_read = Some(read);
    }

    /// The scripted person sees the held chat once its durable row shows it.
    fn held_view(&mut self) {
        let Some(read) = self.pending_read.as_ref() else { return };
        let to = read.to;
        if !self.referee.viewed(&self.store.rows, to).expect("independent durable held view") {
            return;
        }
        let read = self.pending_read.take().expect("pending outside held view");
        match read.purpose {
            ReadPurpose::Requester => self.initial_choice(),
            ReadPurpose::FinalRole => self.ask(
                211,
                1,
                11,
                1,
                people::EscalationDecision::Pass,
                people::Outcome::Refused(people::Refusal::NoFurther),
            ),
            ReadPurpose::Rejected => {}
        }
    }

    fn answer(&mut self) {
        let assignment = self.assignment.as_ref().expect("outside worker assigned");
        let (cumulative, end) = if self.assigned == 1 {
            (3, tasks::End::Failed(tasks::Class::Run))
        } else {
            (
                2,
                tasks::End::Finished {
                    result: tasks::TaskResult::Report { words: REPORT.into() },
                    cancel_delegates: false,
                },
            )
        };
        let (task, attempt) = (assignment.task, assignment.attempt);
        self.worker_answer = WorkerAnswer::Retained;
        let event = self.protocol.answer(Token::new(7), task, attempt, cumulative, end, engine::Parts::none());
        self.queue(event, 0);
    }

    /// The winner of decision `stage`: passing keeps the hold's revision
    /// (jig 00de0b8c), so the first and final decisions share revision one.
    fn winning_choice(&mut self, stage: u64, owner: usize, decision: people::EscalationDecision, key: u8) {
        let revision = 1;
        let by = self.people[owner].expect("authenticated winning actor");
        self.referee.offer(stage, by, decision.clone());
        self.first_choice = Some((owner, revision, decision.clone(), key));
        let choice = match decision {
            people::EscalationDecision::Release => people::EscalationChoice::Released,
            people::EscalationDecision::Reject { .. } => people::EscalationChoice::Rejected,
            people::EscalationDecision::Pass => people::EscalationChoice::Passed,
        };
        let outcome = people::Outcome::EscalationDecided {
            task: self.task.expect("chat started"),
            revision,
            by,
            at: Wall::EPOCH,
            choice,
        };
        self.ask(if stage == 1 { 210 } else { 220 }, owner, key, revision, decision, outcome);
    }

    fn initial_choice(&mut self) {
        match self.settings.story {
            Story::Release => self.winning_choice(1, 0, people::EscalationDecision::Release, 10),
            Story::Reject => {
                self.winning_choice(1, 0, people::EscalationDecision::Reject { reason: REASON.into() }, 10);
            }
            Story::PassRelease | Story::RaceRelease | Story::RaceReject => {
                self.winning_choice(1, 0, people::EscalationDecision::Pass, 10);
            }
        }
    }

    fn final_choice(&mut self) {
        let decision = if self.settings.story == Story::RaceReject {
            people::EscalationDecision::Reject { reason: REASON.into() }
        } else {
            people::EscalationDecision::Release
        };
        self.winning_choice(2, 1, decision, 12);
        if matches!(self.settings.story, Story::RaceRelease | Story::RaceReject) {
            let loser = if self.settings.story == Story::RaceReject {
                people::EscalationDecision::Release
            } else {
                people::EscalationDecision::Reject { reason: REASON.into() }
            };
            let choice = if self.settings.story == Story::RaceReject {
                people::EscalationChoice::Rejected
            } else {
                people::EscalationChoice::Released
            };
            let outcome = people::Outcome::EscalationDecided {
                task: self.task.expect("chat"),
                revision: 1,
                by: self.people[1].expect("winning owner"),
                at: Wall::EPOCH,
                choice,
            };
            self.ask(221, 0, 13, 1, loser, outcome);
        }
    }

    fn after_choice(&mut self, outcome: people::Outcome) {
        let (owner, revision, decision, key) = self.first_choice.clone().expect("outside winning ask retained");
        self.ask(230, owner, key, revision, decision.clone(), outcome.clone());
        self.ask(231, 0, 14, revision, decision.clone(), outcome);
        let different = match decision {
            people::EscalationDecision::Release | people::EscalationDecision::Pass => {
                people::EscalationDecision::Reject { reason: REASON.into() }
            }
            people::EscalationDecision::Reject { .. } => people::EscalationDecision::Release,
        };
        let token = Token::new(232);
        let ask = people::Ask::DecideEscalation {
            project: 1,
            task: self.task.expect("chat started"),
            revision,
            decision: different,
        };
        let input = AskInput { owner, key: [key; 16], ask: ask.clone() };
        self.referee.key_conflict(
            &self.store.rows,
            token,
            self.people[owner].expect("signed-in actor"),
            input.key,
            ask,
        );
        assert!(self.asks.insert(token, input.clone()).is_none(), "fresh conflict request right");
        self.ask_input(token, &input);
        if !self.settings.story.succeeds() {
            self.read(
                204,
                0,
                tasks::Escalation::Rejected {
                    revision,
                    by: self.people[owner].expect("winner"),
                    reason: REASON.into(),
                },
            );
        }
    }

    fn restart(&mut self) {
        assert!(self.store.pending.is_empty(), "cut follows a whole transaction with no later submitted commit");
        self.domain = engine::Domain::new(config(self.settings.seed), &self.environment.limits)
            .expect("valid engine configuration");
        self.events.clear();
        self.out = Queue::with_capacity(engine::max_out(&self.environment.limits));
        self.frozen_cut = None;
        self.recovery = Recovery::Pending;
        self.restarts += 1;
        self.trace.push(format!("restart {:?}", self.settings.cut));
        self.queue(engine::Event::Restart { pending: self.store.pending_generation }, 0);
        self.hello();
    }

    /// Drive one actual root iteration and later fake-store terminals; no direct
    /// child calls or private-state decisions occur.
    pub fn iterate(&mut self) {
        self.iteration += 1;
        let now = u64::from(self.iteration) * 1_000_000 + self.clock_offset;
        self.environment.now = Time::from_nanos(now);
        self.environment.wall = Wall::from_nanos(now);
        if self.frozen_cut.is_some() {
            self.apply_store();
            return;
        }
        crate::poll(&mut self.domain, &self.environment, &mut self.out);
        self.observe();
        if self.frozen_cut.is_some() {
            self.apply_store();
            return;
        }
        if self.events.front().is_some_and(|(due, _)| *due <= self.iteration) {
            let (_, event) = self.events.pop_front().expect("one due scripted event");
            self.trace.push(format!("input {event:?}"));
            if let Err(event) = engine::try_step(&mut self.domain, &self.environment, event) {
                self.queue(event, 1);
            }
            engine::release(&mut self.domain, &self.environment, &mut self.out);
            self.observe();
            if self.frozen_cut.is_some() {
                self.apply_store();
                return;
            }
        }
        engine::fire(&mut self.domain, &self.environment);
        engine::release(&mut self.domain, &self.environment, &mut self.out);
        self.observe();
        if self.frozen_cut.is_some() {
            self.apply_store();
            return;
        }
        if self.settings.facts {
            self.domain.drain_facts();
        }
        self.domain.reclaim();
        self.apply_store();
        self.held_view();
        // The deployment's first period, at its first start (jig's
        // `protocol/configuration.md`, 5).
        if self.domain.ready() && !self.period_sent {
            self.period_sent = true;
            self.queue(
                engine::Event::Party(engine::PartyInput::Period { project: 1, period: 1, budget: 1000, next: None }),
                0,
            );
        }
        if self.domain.ready() && self.period_sent && self.sign_in_script == SignInScript::Waiting {
            self.sign_in_script = SignInScript::Sent;
            self.sign_in(0);
        }
        if self.domain.ready() && self.recovery == Recovery::Pending {
            self.recovery = Recovery::Sent;
            if self.worker_answer == WorkerAnswer::Retained {
                self.answer();
            }
            let pending: Vec<_> = self.asks.iter().map(|(to, input)| (*to, input.clone())).collect();
            for (to, input) in pending {
                self.ask_input(to, &input);
            }
        }
        if self.settings.cut != Cut::None
            && self.recovery == Recovery::Sent
            && self.clock_offset == 0
            && !self.events.iter().any(|(_, event)| {
                matches!(
                    event,
                    engine::Event::Worker(engine::WorkerInput::Link {
                        link: engine::Link::Up { up: host::LinkUp::Answer { .. }, .. },
                        ..
                    })
                )
            })
        {
            // A decision can commit its new claim before assignment escapes.
            // No worker may invent hosting that claim; let actual fleet grace
            // retire it and any retained old answer.
            self.clock_offset = 6_000_000_000;
        }
    }

    fn apply_store(&mut self) {
        if !self.store.pending.is_empty() {
            if self.commit_wait != 0 {
                self.commit_wait -= 1;
            } else {
                let writes = &self.store.pending.front().expect("pending commit").1;
                let held = writes.iter().any(|write| matches!(write, Write::Save(Record::Core(jig_domain_core::Record::Core(jig_domain_core::CoreRecord::Terminal(terminal)))) if terminal.end == tasks::End::Failed(tasks::Class::Run)));
                let decided = writes.iter().any(|write| {
                    matches!(
                        write,
                        Write::Save(Record::Core(jig_domain_core::Record::Core(
                            jig_domain_core::CoreRecord::EscalationDecision(_)
                        )))
                    )
                });
                let number = self.store.apply();
                self.trace.push(format!("applied {number}"));
                self.commit_wait = self.settings.commit_delay;
                if self.restarts == 0
                    && ((self.settings.cut == Cut::Held && held) || (self.settings.cut == Cut::Decision && decided))
                {
                    self.restart();
                } else {
                    self.queue(engine::Event::Store(engine::StoreInput::Committed { number, fill: 0 }), 0);
                }
            }
        }
    }

    fn observe(&mut self) {
        assert!(self.out.len() <= engine::max_out(&self.environment.limits), "declared root output room");
        while let Some(request) = self.out.pop() {
            self.trace.push(format!("output {request:?}"));
            let request = match self.protocol.play(request) {
                Ok(events) => {
                    for event in events {
                        self.queue(event, 0);
                    }
                    continue;
                }
                Err(request) => *request,
            };
            match request {
                engine::Request::ToChild(released) => self.queue(engine::Event::Released(released), 0),
                engine::Request::Store(engine::StoreRequest::Switch) => {
                    let answer = self.store.switch();
                    self.queue(engine::Event::Store(answer), 0);
                }
                engine::Request::Store(engine::StoreRequest::Commit { number, writes }) => {
                    if writes.iter().any(|write| matches!(write, Write::Save(Record::Core(jig_domain_core::Record::Core(jig_domain_core::CoreRecord::Terminal(terminal)))) if terminal.end == tasks::End::Refused)) {
                        assert!(self.unplaced_referee.is_none(), "one unassigned-claim terminal in this script");
                        self.unplaced_referee = Some(self.referee.clone());
                    }
                    self.referee.commit(&writes).expect("independent escalation transaction referee");
                    self.transactions.push(writes.to_vec());
                    if self.store.pending.is_empty() {
                        self.commit_wait = self.settings.commit_delay;
                    }
                    let held_cut = self.settings.cut == Cut::Held && writes.iter().any(|write| matches!(write, Write::Save(Record::Core(jig_domain_core::Record::Core(jig_domain_core::CoreRecord::Terminal(terminal)))) if terminal.end == tasks::End::Failed(tasks::Class::Run)));
                    let decision_cut = self.settings.cut == Cut::Decision
                        && writes.iter().any(|write| {
                            matches!(
                                write,
                                Write::Save(Record::Core(jig_domain_core::Record::Core(
                                    jig_domain_core::CoreRecord::EscalationDecision(_)
                                )))
                            )
                        });
                    self.store.pending.push_back((number, writes));
                    if self.restarts == 0 && (held_cut || decision_cut) {
                        self.frozen_cut = Some(self.settings.cut);
                        return;
                    }
                }
                engine::Request::Store(engine::StoreRequest::Load { owner, range, after, most, .. }) => {
                    self.pages += 1;
                    let (rows, next) = self.store.page(&range, after.as_ref(), most);
                    self.queue(
                        engine::Event::Store(engine::StoreInput::Loaded { owner, rows, next, texts: None }),
                        self.settings.page_delay,
                    );
                }
                engine::Request::Worker(engine::WorkerRequest::Deliver(delivery))
                | engine::Request::Party(engine::PartyRequest::Deliver(delivery)) => self.delivery(delivery),
                engine::Request::Stop => panic!("escalation root stopped: {:?}", self.trace),
                other @ (engine::Request::Forge { .. }
                | engine::Request::Worker(_)
                | engine::Request::Party(_)
                | engine::Request::Store(_)
                | engine::Request::Account(_)
                | engine::Request::StopGroup { .. }
                | engine::Request::SetUpGroup { .. }
                | engine::Request::Connect { .. }
                | engine::Request::Manifest { .. }
                | engine::Request::Reachable { .. }
                | engine::Request::Install { .. }
                | engine::Request::Accept { .. }
                | engine::Request::EraseCarried { .. }
                | engine::Request::SetUp { .. }
                | engine::Request::Bound { .. }
                | engine::Request::Return { .. }
                | engine::Request::Setup { .. }
                | engine::Request::Busy(_)
                | engine::Request::Unsettled(_)
                | engine::Request::Drained) => panic!("unexpected escalation output {other:?}"),
            }
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one exhaustive outside delivery adapter drives the finite authenticated script"
    )]
    fn delivery(&mut self, delivery: Delivery) {
        match delivery {
            Delivery::SourceSignedOut { .. } => panic!("this story issues no source sign-out"),
            Delivery::WebReply {
                to,
                reply: reply @ (people::Reply::Again(_) | people::Reply::Outcome(people::Outcome::Again(_))),
            } => self.retry_ask(to.into_token(), &reply),
            Delivery::WebReply { to, reply: people::Reply::SignedIn { person, sign_in: session, .. } } => {
                let index = usize::try_from(to.into_token().raw() - 101).expect("two named sign-in reply rights");
                self.referee.signed_in(&self.store.rows, index, person, session).expect("durable exact owner sign-in");
                self.sessions[index] = Some(session);
                self.people[index] = Some(person);
                if index == 0 {
                    self.sign_in(1);
                } else {
                    self.queue(
                        engine::Event::Party(engine::PartyInput::Ask {
                            models: Box::new([]),
                            reply_to: ReplyTo::new(Token::new(103)),
                            sign_in: self.sessions[0].expect("requester signed in"),
                            key: [5; 16],
                            ask: crate::walking::chat(1, QUESTION),
                        }),
                        0,
                    );
                }
            }
            Delivery::WebReply {
                to,
                reply: people::Reply::Outcome(people::Outcome::Created { first: task, last, given }),
                ..
            } => {
                assert_eq!(task, last, "one template task");
                assert_eq!(given, Some(people::Given { task, amount: 500 }), "template funding receipt");
                assert_eq!(to.into_token(), Token::new(103), "sole chat-start right");
                self.referee.started(&self.store.rows, task).expect("atomic actual chat start");
                self.task = Some(task);
            }
            Delivery::WebReply { to, reply: people::Reply::Outcome(outcome), .. } => {
                let token = to.into_token();
                let checked = self.referee.replied(&self.store.rows, token, people::Reply::Outcome(outcome.clone()));
                assert!(checked.is_ok(), "keyed terminal: {checked:?}; {:?}; {token:?}: {outcome:?}", self.settings);
                self.asks.remove(&token).expect("one pending outside ask terminal");
                let logical = self.ask_rights.remove(&token).unwrap_or(token);
                match logical.raw() {
                    210 if self.settings.story.passes() => self.read(
                        202,
                        1,
                        tasks::Escalation::Waiting {
                            revision: 1,
                            holder: tasks::EscalationHolder::Role { project: 1, role: 0 },
                            entry: 1,
                            since: self.environment.wall,
                        },
                    ),
                    210 | 220 => self.after_choice(outcome),
                    211 => self.final_choice(),
                    230 | 231 | 232 | 221 => {}
                    _ => panic!("unscripted terminal right"),
                }
            }
            Delivery::WebReply { to, reply: people::Reply::Refused(refusal), .. } => {
                let token = to.into_token();
                let logical = self.ask_rights.remove(&token).unwrap_or(token);
                assert_eq!(logical, Token::new(232), "sole immediate key-conflict right; refusal {refusal:?}");
                self.referee
                    .replied(&self.store.rows, token, people::Reply::Refused(refusal))
                    .expect("immediate refusal preserves the original saved winner");
                self.asks.remove(&token).expect("one pending outside conflict terminal");
            }
            Delivery::Assigned { channel, assignment } => {
                assert_eq!(channel, Token::new(7));
                self.referee.assigned(&self.store.rows, &assignment).expect("independent fresh actual claim and brief");
                self.assignment = Some(assignment);
                self.assigned += 1;
                self.answer();
            }
            Delivery::Link { channel, down: host::LinkDown::Acknowledge { run, attempt } } => {
                assert_eq!(channel, Token::new(7));
                let (task, attempt) = (run.raw(), attempt.raw());
                self.referee.acknowledged(&self.store.rows, task, attempt).expect("independent priced terminal ACK");
                self.worker_answer = WorkerAnswer::Idle;
                if self.assigned == 1 {
                    self.read(
                        201,
                        0,
                        tasks::Escalation::Waiting {
                            revision: 1,
                            holder: tasks::EscalationHolder::Person(self.people[0].expect("requester")),
                            entry: 1,
                            since: self.environment.wall,
                        },
                    );
                }
            }
            Delivery::Result { person, task, words } => self
                .referee
                .result(&self.store.rows, person, task, &words)
                .expect("independent exact final report and funding"),
            Delivery::Link { down: host::LinkDown::Grant { .. }, .. } => {}
            Delivery::DemoActed { .. }
            | Delivery::DemoNote { .. }
            | Delivery::Application { .. }
            | Delivery::Forges { .. }
            | Delivery::ForgeDocument { .. }
            | Delivery::Panel { .. }
            | Delivery::WebReply { .. }
            | Delivery::CarriedUnused { .. }
            | Delivery::Link { .. }
            | Delivery::ForgeCall { .. }
            | Delivery::Bound { .. }
            | Delivery::Read { .. }
            | Delivery::Document { .. }
            | Delivery::WatchEnded { .. }
            | Delivery::Drained
            | Delivery::Refuse { .. }
            | Delivery::TurnBusy { .. } => panic!("unexpected escalation delivery {delivery:?}"),
            Delivery::Restart(_)
            | Delivery::Fleet(_)
            | Delivery::ProjectMade { .. }
            | Delivery::ForgeCommitted { .. }
            | Delivery::ReadLoad { .. }
            | Delivery::ReadEscalationDecision { .. }
            | Delivery::Relay { .. }
            | Delivery::Load { .. } => panic!("internal root callback leaked to world"),
            Delivery::Host(_) => panic!("escalation world sent no calls"),
            Delivery::Procedure { .. } => panic!("escalation world sent no procedures"),
        }
    }

    /// Complete every outside obligation and actual pending IO within 600 passes.
    /// A held rejected task is a final story condition, not hidden work
    pub fn run(&mut self) {
        for _ in 0..600 {
            if self.settled() {
                self.referee.final_state(&self.store.rows).expect("independent final finite funding");
                return;
            }
            self.iterate();
        }
        panic!("escalation story did not settle: {}", self.unsettled());
    }

    fn unsettled(&self) -> String {
        format!(
            "settings {:?}; iteration {}; pending asks {:?}; pending read {:?}; worker {:?}; \
             recovery {:?}; cut {:?}; referee {:?}; pending store {}; events {:?}; output {:?}; \
             recent boundary trace {:?}",
            self.settings,
            self.iteration,
            self.asks,
            self.pending_read,
            self.worker_answer,
            self.recovery,
            self.frozen_cut,
            self.referee,
            self.store.pending.len(),
            self.events,
            self.out,
            self.trace.iter().rev().take(32).collect::<Vec<_>>(),
        )
    }

    fn settled(&self) -> bool {
        self.referee.done()
            && self.worker_answer == WorkerAnswer::Idle
            && self.asks.is_empty()
            && self.pending_read.is_none()
            && self.events.is_empty()
            && self.store.pending.is_empty()
            && self.out.is_empty()
            && self.domain.quiescent()
    }
}

/// Compare complete frozen root/children/store/scripts each iteration, including
/// diagnostic state with the same settings (testing-strategy.md, section 6).
#[must_use]
pub fn run_replayed(settings: Settings) -> World {
    let mut first = World::new(settings);
    let mut replay = World::new(settings);
    for _ in 0..600 {
        if first.settled() {
            assert!(replay.settled(), "replay settled at same cut: {settings:?}; {}", replay.unsettled());
            first.referee.final_state(&first.store.rows).expect("replayed finite funding");
            return first;
        }
        first.iterate();
        replay.iterate();
        assert_eq!(
            format!("{first:?}"),
            format!("{replay:?}"),
            "complete frozen escalation state for {settings:?} at iteration {}",
            first.iteration
        );
    }
    panic!("escalation replay did not settle: {}", first.unsettled());
}
