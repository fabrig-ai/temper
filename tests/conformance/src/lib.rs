//! Temper's engine on jig's crash harness and outside referee.
//! The forge observes native API calls, and store snapshots are independent of
//! root state (jig's domain/testing.md, sections 5–6).
mod config;
pub mod link;
pub mod observations;
mod scenarios;
pub mod stories;
pub mod systems;
#[cfg(test)]
mod tests;

use jig_conformance::referee as r;
use jig_conformance::{Application, Clock, Input, Output};
use jig_domain_host as host;
use jig_domain_people as people;
use jig_domain_tasks as tasks;
use jig_fake_workers as workers;
use observations::Store;
use skein_lib::{Duration, Env, Queue, ReplyTo, Time, Token, Wall};
use temper_engine_domain as engine;
use temper_engine_domain::{self as root};
use temper_engine_domain_forge as forge;
use temper_engine_domain_world::protocol::Protocol;

/// Scenario identity and seed retained across cold starts.
#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub seed: u64,
    pub scenario: Option<jig_conformance::scenarios::Scenario>,
    pub story: Option<stories::Story>,
    limits: engine::Limits,
    maximum: u64,
}
impl Config {
    #[must_use]
    pub fn new(seed: u64, scenario: Option<jig_conformance::scenarios::Scenario>) -> Self {
        Self::configured(seed, scenario, None)
    }
    #[must_use]
    pub fn for_story(seed: u64, story: stories::Story) -> Self {
        Self::configured(seed, None, Some(story))
    }
    fn configured(
        seed: u64,
        scenario: Option<jig_conformance::scenarios::Scenario>,
        story: Option<stories::Story>,
    ) -> Self {
        let limits = config::root(seed, scenario, story).1;
        let maximum = engine::worst_case(&limits).expect("finite root")
            + u64::try_from(std::mem::size_of::<Domain>() - std::mem::size_of::<engine::Domain>())
                .expect("driver size");
        Self { seed, scenario, story, limits, maximum }
    }
}
/// Timers enter independently from external root inputs.
#[expect(clippy::large_enum_variant, reason = "the test driver retains native events directly for the harness trace")]
pub enum Event {
    Root(engine::Event),
    Fire,
}
impl std::fmt::Debug for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Root(event) => {
                if let engine::Event::Store(engine::StoreInput::Loaded { owner, rows, next, texts: None }) = event {
                    // The store retains the exact rows; traces need the page's
                    // identity and bounds rather than another formatted copy.
                    f.debug_struct("Loaded")
                        .field("owner", owner)
                        .field("rows", &rows.len())
                        .field("next", next)
                        .finish()
                } else {
                    f.debug_tuple("Root").field(event).finish()
                }
            }
            Self::Fire => f.write_str("Fire"),
        }
    }
}
/// Send an external input through the conformance harness.
pub fn send(world: &mut jig_conformance::Harness<Temper>, event: engine::Event) {
    world.send(Event::Root(event));
}
/// The application binding.
pub struct Temper;
/// Driver progress bookkeeping around the concrete engine, outside its state.
pub struct Domain {
    root: engine::Domain,
    outputs: Queue<engine::Request>,
    probed: bool,
    /// An input admission handed back; its peer sends it again (jig's
    /// `domain/root.md`, 3).
    returned: Option<engine::Event>,
}

/// Independent worker, party and store evidence; retained across crashes.
#[expect(clippy::struct_excessive_bools, reason = "each startup request has independent request and reply evidence")]
pub struct Peers {
    pub policy: r::Policy,
    /// Advanced at each independently requested policy edit.
    pub policy_generation: u64,
    pub seed: u64,
    pub scenario: Option<jig_conformance::scenarios::Scenario>,
    pub story: Option<stories::Story>,
    pub worker: workers::Worker,
    pub assignments: Vec<engine::Assignment>,
    pub answers: Vec<root::CallAnswer>,
    pub sign_in: Option<u64>,
    pub adopted: bool,
    signed_in: bool,
    adopting: bool,
    chatted: bool,
    /// The period and the git identity's credential were sent to this start.
    started: bool,
    applied: u64,
    first_start: Option<r::Snapshot>,
    observed: Vec<r::Observed>,
    /// The protocol layer this world plays: decodes, renders, outcomes, accounts.
    protocol: Protocol,
    requests: std::collections::BTreeMap<u64, [u8; 16]>,
    ask_bodies: std::collections::BTreeMap<u64, (u64, [u8; 16], people::Ask)>,
    ask_retries: std::collections::BTreeMap<u64, u64>,
    pub pending_proposal: Option<u64>,
    pub saved_deadline: Option<Wall>,
    pub initial_assignments: usize,
    pub changes: Vec<u64>,
    children: Vec<u64>,
    forgetting: std::collections::BTreeMap<u64, Vec<u64>>,
    lifecycle: scenarios::lifecycle::State,
    query: scenarios::queries::State,
    pub branches: Vec<Box<[u8]>>,
    pub produced: std::collections::BTreeMap<u64, skein_forge::api::Commit>,
    pub reported: std::collections::BTreeMap<u64, skein_forge::api::Commit>,
    /// Calls by name, kept by the worker until their answers are relayed.
    calls: std::collections::BTreeMap<u64, (u64, u64, engine::Call)>,
    retries: std::collections::BTreeMap<u64, u64>,
    generation: u64,
    uncertain: std::collections::BTreeMap<jig_test_system::SystemKey, u16>,
    words: std::collections::BTreeSet<(u64, u64)>,
    effects: std::collections::BTreeMap<jig_test_system::SystemKey, observations::EffectEvidence>,
    held: std::collections::BTreeMap<jig_test_system::SystemKey, u16>,
    settled: std::collections::BTreeSet<jig_test_system::SystemKey>,
}
/// Released outputs, drained without inspecting the root.
#[derive(Debug)]
pub enum Delivery {
    Root(engine::Request),
    /// An input admission handed back, for its peer to send again.
    Returned(Box<engine::Event>),
    Resume,
}

fn env(config: &Config, clock: Clock) -> Env<engine::Limits> {
    Env { now: Time::from_nanos(clock.now), wall: Wall::from_nanos(clock.now), limits: config.limits }
}

impl Application for Temper {
    type Domain = Domain;
    type Store = Store;
    type Write = engine::Write;
    type Config = Config;
    type Systems = systems::FakeForge;
    type Peers = Peers;
    type Key = root::Key;
    type Record = observations::Record;
    type Event = Event;
    type Delivery = Delivery;

    fn build(config: &Config) -> Domain {
        let (mut configuration, limits) = config::root(config.seed, config.scenario, config.story);
        if config.scenario == Some(jig_conformance::scenarios::Scenario::ChangedJudge) {
            for rule in &mut configuration.landing.deployment {
                rule.enforce = false;
            }
            if let Some(rules) = configuration.landing.projects.get_mut(&1) {
                for rule in rules {
                    rule.enforce = false;
                }
            }
        }
        Domain {
            root: engine::Domain::new(configuration, &limits).expect("valid engine configuration"),
            outputs: Queue::with_capacity(engine::max_out(&limits)),
            probed: false,
            returned: None,
        }
    }
    fn store(config: &Config) -> Store {
        let configuration = config::root(config.seed, config.scenario, config.story).0;
        temper_engine_domain_world::fixtures::restored_over(
            Store::with_limits(&config.limits),
            configuration,
            &config.limits,
        )
    }

    fn systems(config: &Config, _: u64) -> systems::FakeForge {
        let mut system = systems::FakeForge::new_with_ci(
            config.seed,
            if config.story == Some(stories::Story::RepairCi) { 0 } else { 1000 },
        );
        system.lose_merge_answer = config.story == Some(stories::Story::LostMerge);
        system.lose_issue_answer = config.story == Some(stories::Story::IssueCreation);
        system.merge_only = config.story == Some(stories::Story::OtherHand)
            || matches!(
                config.scenario,
                Some(
                    jig_conformance::scenarios::Scenario::OtherHand
                        | jig_conformance::scenarios::Scenario::ChangedJudge
                )
            );
        system
    }
    fn peers(config: &Config, _: u64) -> Peers {
        let script: Box<[workers::Script]> = if config.story == Some(stories::Story::PersonWords) {
            Box::new([workers::Script::Silence { for_: Duration::from_secs(1) }, workers::Script::Park])
        } else if config.story.is_some() {
            Box::new([workers::Script::Wait, workers::Script::Finish { report: b"story done".as_slice().into() }])
        } else {
            use jig_conformance::scenarios::Scenario;
            match config.scenario {
                Some(Scenario::QueryWatchedWhileTasksEnd) => Box::new([
                    workers::Script::Wait,
                    workers::Script::Finish { report: b"query task done".as_slice().into() },
                ]),
                Some(Scenario::ConversationEndsWhileRead) => Box::new([
                    workers::Script::Wait,
                    workers::Script::Turn { body: b"conversation turn".as_slice().into(), cost: 1, read: None },
                    workers::Script::Wait,
                    workers::Script::Finish { report: b"conversation done".as_slice().into() },
                ]),
                Some(Scenario::Compound) => Box::new([workers::Script::Wait, workers::Script::Wait]),
                Some(Scenario::Grace) => Box::new([
                    workers::Script::Wait,
                    workers::Script::Wait,
                    workers::Script::Finish { report: b"grace finished".as_slice().into() },
                ]),
                Some(jig_conformance::scenarios::Scenario::RunBudget) => {
                    let mut values = vec![workers::Script::Wait];
                    for index in 0..12 {
                        values.push(workers::Script::Turn {
                            body: format!("completion {index}").into_bytes().into(),
                            cost: if index < 8 { 2 } else { 1 },
                            read: None,
                        });
                    }
                    values.push(workers::Script::Park);
                    values.into_boxed_slice()
                }
                Some(
                    jig_conformance::scenarios::Scenario::OtherHand
                    | jig_conformance::scenarios::Scenario::ChangedJudge
                    | jig_conformance::scenarios::Scenario::StandingTask,
                ) => Box::new([
                    workers::Script::Wait,
                    workers::Script::Finish { report: b"period done".as_slice().into() },
                ]),
                _ => Box::new([workers::Script::Wait]),
            }
        };
        // The referee must see receipt one and receipt two separately, even though
        // both are durable before the harness begins restoring the live root.
        let first = temper_engine_domain_world::fixtures::first_start_over(
            Store::with_limits(&config.limits),
            config::root(config.seed, config.scenario, config.story).0,
            &config.limits,
        );
        Peers {
            seed: config.seed,
            scenario: config.scenario,
            story: config.story,
            policy: observations::policy(config.seed, config.scenario, config.story),
            policy_generation: 0,
            worker: workers::Worker::new(
                link::CHANNEL,
                config.limits.fleet.slots,
                (0..16).map(|_| script.clone()).collect(),
            ),
            assignments: Vec::new(),
            answers: Vec::new(),
            sign_in: None,
            adopted: false,
            signed_in: false,
            adopting: false,
            chatted: false,
            started: false,
            applied: 0,
            first_start: Some(observations::snapshot(&first)),
            observed: Vec::new(),
            protocol: Protocol::default(),
            requests: std::collections::BTreeMap::new(),
            ask_bodies: std::collections::BTreeMap::new(),
            ask_retries: std::collections::BTreeMap::new(),
            pending_proposal: None,
            saved_deadline: None,
            initial_assignments: 0,
            changes: Vec::new(),
            children: Vec::new(),
            forgetting: std::collections::BTreeMap::new(),
            lifecycle: scenarios::lifecycle::State::new(config.seed),
            query: scenarios::queries::State::default(),
            branches: Vec::new(),
            produced: std::collections::BTreeMap::new(),
            reported: std::collections::BTreeMap::new(),
            calls: std::collections::BTreeMap::new(),
            retries: std::collections::BTreeMap::new(),
            generation: 0,
            uncertain: std::collections::BTreeMap::new(),
            words: std::collections::BTreeSet::new(),
            effects: std::collections::BTreeMap::new(),
            held: std::collections::BTreeMap::new(),
            settled: std::collections::BTreeSet::new(),
        }
    }
    fn policy(peers: &Peers) -> &r::Policy {
        &peers.policy
    }
    fn policy_generation(peers: &Peers) -> u64 {
        peers.policy_generation
    }
    fn recovery(_: &Config, connector: u16, kind: u16) -> r::Recovery {
        assert_eq!(connector, 0);
        match kind {
            3 | 4 => r::Recovery::Conditional,
            6 | 10 | 12..=15 => r::Recovery::Idempotent,
            2 | 5 | 7 | 8 | 9 | 11 => r::Recovery::Unrecoverable,
            _ => panic!("unknown forge kind {kind}"),
        }
    }
    fn start(_: &Config, _: &mut Store) -> Vec<Event> {
        vec![Event::Root(engine::Event::Restart { pending: false })]
    }
    fn cold(peers: &mut Peers, store: &Store, _: Clock) {
        peers.observed.push(r::Observed::ColdStore { receipts: observations::snapshot(store).receipts });
        for record in store.rows.values() {
            if let root::Record::Forge { row } = record
                && let forge::Stored::Entry { entry, held: false } = row.as_ref()
                && entry.attempt.is_some()
            {
                let key = observations::key(store, entry);
                peers.uncertain.insert(key, observations::parts(&entry.effect.write).0);
                peers.observed.push(r::Observed::Uncertain { connector: 0, key });
            }
        }
        peers.worker.restart_engine();
        peers.generation += 1;
        peers.started = false;
        peers.retries.extend(peers.calls.keys().map(|call| (*call, 0)));
        peers.ask_retries.extend(peers.ask_bodies.keys().map(|call| (*call, 0)));
        if peers.sign_in.is_none() {
            peers.signed_in = false;
        }
        if !peers.adopted {
            peers.adopting = false;
        }
        if peers.assignments.is_empty() {
            peers.chatted = false;
        }
    }
    fn timers(_: &Config) -> Vec<Event> {
        vec![Event::Fire]
    }
    fn step(domain: &mut Domain, config: &Config, clock: Clock, input: Input<Event, observations::Record>) {
        let environment = env(config, clock);
        domain.probed = matches!(input, Input::Event(Event::Fire));
        let event = match input {
            Input::Event(Event::Root(event)) => event,
            Input::Event(Event::Fire) => {
                engine::fire(&mut domain.root, &environment);
                return;
            }
            Input::Committed(number) => engine::Event::Store(engine::StoreInput::Committed { number, fill: 0 }),
            Input::Failed(number) => engine::Event::Store(engine::StoreInput::Uncommitted { number }),
            Input::Restore(_) => panic!("temper restores through bounded load terminals"),
        };
        if let Err(event) = engine::try_step(&mut domain.root, &environment, event) {
            assert!(domain.returned.replace(event).is_none(), "one input per iteration");
        }
    }
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "every native nonstore request is retained unchanged as a released delivery"
    )]
    fn release(domain: &mut Domain, config: &Config, clock: Clock) -> Vec<Output<engine::Write, Delivery>> {
        let environment = env(config, clock);
        engine::release(&mut domain.root, &environment, &mut domain.outputs);
        if domain.outputs.is_empty() {
            engine::release(&mut domain.root, &environment, &mut domain.outputs);
        }
        if domain.outputs.is_empty() {
            engine::step(&mut domain.root, &environment, engine::Event::Resume);
            engine::release(&mut domain.root, &environment, &mut domain.outputs);
        }
        let mut outputs = Vec::new();
        while let Some(request) = domain.outputs.pop() {
            outputs.push(match request {
                engine::Request::Store(engine::StoreRequest::Commit { number, writes }) => {
                    Output::Commit { number, writes }
                }
                engine::Request::Stop => Output::Stop,
                other => Output::Deliver(Delivery::Root(other)),
            });
        }
        if let Some(event) = domain.returned.take() {
            outputs.push(Output::Deliver(Delivery::Returned(Box::new(event))));
        }
        if outputs.is_empty() && !domain.root.quiescent() && !domain.probed {
            outputs.push(Output::Deliver(Delivery::Resume));
        }
        if !outputs.is_empty() && !matches!(outputs.first(), Some(Output::Deliver(Delivery::Resume))) {
            domain.probed = false;
        }
        outputs
    }
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "the peer retains asks and charge evidence without interpreting other inputs"
    )]
    fn inbound(peers: &mut Peers, _: &Store, _: Clock, input: &mut Event) {
        let Event::Root(input) = input else { return };
        match input {
            engine::Event::Party(engine::PartyInput::Ask { models: _, reply_to, sign_in, key, ask }) => {
                let call = destination(reply_to);
                peers.requests.insert(call, *key);
                peers.ask_bodies.insert(call, (*sign_in, *key, ask.clone()));
            }
            engine::Event::Party(engine::PartyInput::Application { reply_to, key, .. }) => {
                peers.requests.insert(destination(reply_to), *key);
            }
            engine::Event::Worker(engine::WorkerInput::Link {
                link: engine::Link::Up { up: host::LinkUp::Turn { run, attempt, turn }, .. },
                ..
            }) => peers.observed.push(r::Observed::Completion {
                task: run.raw(),
                attempt: attempt.raw(),
                cumulative: turn.spent,
            }),
            engine::Event::Worker(engine::WorkerInput::Link {
                link: engine::Link::Up { up: host::LinkUp::Answer { run, attempt, answer }, .. },
                ..
            }) => peers.observed.push(r::Observed::Completion {
                task: run.raw(),
                attempt: attempt.raw(),
                cumulative: answer.spent,
            }),
            _ => {}
        }
    }
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "one boundary adapter acknowledges selected native outputs and retains their receipts"
    )]
    fn deliver(
        peers: &mut Peers,
        systems: &mut systems::FakeForge,
        store: &mut Store,
        clock: Clock,
        delivery: Delivery,
    ) -> Vec<Input<Event, observations::Record>> {
        let request = match delivery {
            Delivery::Root(request) => request,
            Delivery::Returned(event) => return vec![Input::Event(Event::Root(*event))],
            Delivery::Resume => return vec![Input::Event(Event::Fire)],
        };
        let request = match request {
            engine::Request::Store(engine::StoreRequest::Read { load, range }) => {
                let result = store.read(range);
                return vec![Input::Event(Event::Root(engine::Event::Store(engine::StoreInput::ReadLoaded {
                    load,
                    result,
                })))];
            }
            request => request,
        };
        // The protocol layer answers at once (testing-strategy.md, 4).
        let request = match peers.protocol.play(request) {
            Ok(events) => return events.into_iter().map(|event| Input::Event(Event::Root(event))).collect(),
            Err(request) => *request,
        };
        let mut events = Vec::new();
        match request {
            engine::Request::ToChild(released) => events.push(engine::Event::Released(released)),
            engine::Request::Store(engine::StoreRequest::Switch) => {
                events.push(engine::Event::Store(store.switch()));
            }
            engine::Request::Store(engine::StoreRequest::Load { owner, range, after, most, .. }) => {
                let (rows, next) = store.page(&range, after.as_ref(), most);
                events.push(engine::Event::Store(engine::StoreInput::Loaded { owner, rows, next, texts: None }));
            }
            engine::Request::Forge { call, repository, op } => {
                events.extend(systems.call(store, clock, call, repository, op));
            }
            engine::Request::Worker(engine::WorkerRequest::Deliver(delivery))
            | engine::Request::Party(engine::PartyRequest::Deliver(delivery)) => {
                if let engine::Delivery::Assigned { assignment, .. } = &delivery
                    && matches!(
                        peers.scenario,
                        Some(
                            jig_conformance::scenarios::Scenario::Compound
                                | jig_conformance::scenarios::Scenario::Grace
                                | jig_conformance::scenarios::Scenario::ConversationEndsWhileRead
                                | jig_conformance::scenarios::Scenario::QueryWatchedWhileTasksEnd
                        )
                    )
                {
                    // These scripts leave their push workspace at its initial head;
                    // a real worker reports that head even when no file changed.
                    peers.reported.insert(assignment.task, systems.head);
                }
                if let engine::Delivery::Document { watcher, .. } = &delivery {
                    events.push(engine::Event::Party(engine::PartyInput::Delivered { watcher: *watcher }));
                }
                scenarios::queries::delivered(peers, store, &delivery);
                scenarios::lifecycle::delivered(peers, &delivery);
                delivered(peers, clock, delivery);
            }
            // A refused sign-in is asked again at the next poll.
            engine::Request::Party(engine::PartyRequest::SignInRefused { .. }) => peers.signed_in = false,
            engine::Request::Drained => peers.observed.push(r::Observed::Drained { at: store.applied }),
            _ => {}
        }
        events.into_iter().map(|event| Input::Event(Event::Root(event))).collect()
    }
    fn poll(
        peers: &mut Peers,
        systems: &mut systems::FakeForge,
        store: &mut Store,
        clock: Clock,
        ready: bool,
    ) -> Vec<Input<Event, observations::Record>> {
        systems.generation = peers.generation;
        let mut events = systems.poll(clock);
        if ready {
            if !peers.started {
                peers.started = true;
                // The project's first period comes from outside the root, once
                // (jig's `protocol/configuration.md`, 5); the repository's git
                // identity is an application credential (domain/root.md, 7.1).
                if !store.rows.keys().any(|key| {
                    matches!(
                        key,
                        root::Key::Core(jig_domain_core::Key::Tasks(tasks::Key::Ledger(tasks::Funder::Period {
                            project: 1,
                            ..
                        })))
                    )
                }) {
                    events.push(engine::Event::Party(engine::PartyInput::Period {
                        project: 1,
                        period: 1,
                        budget: 1000,
                        next: None,
                    }));
                }
                events.push(engine::Event::Credential { number: 1, generation: 1, valid: Duration::from_secs(3600) });
            }
            let asks: Vec<_> =
                peers.ask_retries.iter().filter_map(|(call, at)| (*at <= clock.now).then_some(*call)).collect();
            for call in asks {
                peers.ask_retries.remove(&call);
                if let Some((sign_in, key, ask)) = peers.ask_bodies.get(&call) {
                    events.push(engine::Event::Party(engine::PartyInput::Ask {
                        models: Box::new([]),
                        reply_to: ReplyTo::new(Token::new(call)),
                        sign_in: *sign_in,
                        key: *key,
                        ask: ask.clone(),
                    }));
                }
            }
            events.extend(link::retried(peers, clock.now));
            for up in peers.worker.tick(Time::from_nanos(clock.now)) {
                events.push(link::worker_event(peers, up));
            }
            if !peers.signed_in {
                peers.signed_in = true;
                events.push(engine::Event::Party(engine::PartyInput::SignedIn {
                    held: None,
                    reply_to: ReplyTo::new(Token::new(90)),
                    identity: people::Identity {
                        key: people::IdentityKey { provider: 0, subject: 7_u64.to_be_bytes().into() },
                        login: b"owner".as_slice().into(),
                        name: b"Owner".as_slice().into(),
                    },
                }));
            }
            if let Some(sign_in) = peers.sign_in {
                if !peers.adopting {
                    peers.adopting = true;
                    events.push(engine::Event::Party(engine::PartyInput::Application {
                        scope: jig_domain_people::ApplicationScope::Project(1),
                        reply_to: ReplyTo::new(Token::new(91)),
                        sign_in,
                        key: [91; 16],
                        digest: [5; 32],
                        carried: None,
                        request: temper_engine_domain::Application::Forge(forge::Ask::Adopt(forge::Adoption {
                            project: 1,
                            home: true,
                            forge: temper_engine_forge_world::REPO.forge,
                            host: b"forge.example".as_slice().into(),
                            owner: b"org".as_slice().into(),
                            name: b"repo".as_slice().into(),
                            prefix: b"temper/".as_slice().into(),
                            role: forge::Role::Owned,
                            landing: b"main".as_slice().into(),
                            ci: true,
                            checks: Box::new([]),
                        })),
                    }));
                }
                if peers.adopted && !peers.chatted {
                    peers.chatted = true;
                    events.push(engine::Event::Party(engine::PartyInput::Ask {
                        models: Box::new([]),
                        reply_to: ReplyTo::new(Token::new(92)),
                        sign_in,
                        key: [92; 16],
                        ask: chat(peers, store, 1, b"work"),
                    }));
                }
            }
        }
        events.into_iter().map(|event| Input::Event(Event::Root(event))).collect()
    }
    fn submitted(peers: &mut Peers, store: &Store, _: Clock) {
        if let Some((number, writes)) = store.pending.back() {
            let mut forgotten = Vec::new();
            for write in writes {
                if let engine::Write::Erase(engine::Key::Core(jig_domain_core::Key::Tasks(tasks::Key::Ended(task)))) =
                    write
                {
                    forgotten.push(*task);
                }
            }
            if !forgotten.is_empty() {
                peers.forgetting.insert(*number, forgotten);
            }
        }
    }
    fn observed(peers: &mut Peers, systems: &systems::FakeForge, store: &Store, _: Clock) -> Vec<r::Observed> {
        let mut observations = Vec::new();
        if let Some(snapshot) = peers.first_start.take() {
            // The bootstrap rows and its separate wires mark have the same public snapshot.
            observations.push(r::Observed::Durable { number: 1, snapshot: snapshot.clone() });
            observations.push(r::Observed::Durable { number: 2, snapshot });
            peers.applied = 2;
        }
        if peers.applied != store.applied {
            peers.applied = store.applied;
            let mut snapshot = observations::snapshot(store);
            observations::resolved(store, &mut snapshot, &mut peers.effects, &mut peers.held, &mut peers.settled);
            for record in store.rows.values() {
                if let root::Record::Core(jig_domain_core::Record::Tasks(
                    tasks::Stored::Live(task) | tasks::Stored::Ended(task),
                )) = record
                {
                    for word in &task.inbox {
                        if matches!(word.from, tasks::Party::Person(_))
                            && peers.words.insert((task.number, word.number))
                        {
                            peers.observed.push(r::Observed::Words { task: task.number, message: word.number });
                        }
                    }
                }
            }
            observations.push(r::Observed::Durable { number: store.applied, snapshot });
            while let Some((&number, _)) = peers.forgetting.first_key_value() {
                if number > store.applied {
                    break;
                }
                for task in peers.forgetting.remove(&number).expect("submitted retention") {
                    observations.push(r::Observed::Forgotten { task });
                }
            }
        }
        observations.append(&mut peers.observed);
        observations.extend(systems.take_observed());
        let mut totals = std::collections::BTreeMap::<u64, (u64, u64)>::new();
        for (&(task, attempt), expense) in &peers.worker.expenses {
            if expense.lost_turns > 0 {
                observations.push(r::Observed::LostTurns {
                    task,
                    attempt,
                    turns: expense.lost_turns,
                    spent: expense.lost,
                });
            }
            let total = totals.entry(task).or_default();
            total.0 += expense.spent;
            total.1 += expense.lost;
        }
        for (task, (spent, lost_bound)) in totals {
            observations.push(r::Observed::Spend { task, spent, lost_bound });
        }
        observations
    }
    fn ready(domain: &Domain) -> bool {
        domain.root.ready()
    }
    fn quiescent(domain: &Domain) -> bool {
        domain.root.quiescent() || domain.probed
    }
    fn reclaim(domain: &mut Domain) {
        domain.root.reclaim();
    }
    fn maximum(config: &Config) -> u64 {
        config.maximum
    }
    fn configuration_heap(_: &Config) -> u64 {
        0
    }
}

/// One delivery to the worker or the person, and the durable receipts its
/// release requires.
#[expect(clippy::wildcard_enum_match_arm, reason = "the scripted peers take only the deliveries their stories use")]
fn delivered(peers: &mut Peers, clock: Clock, delivery: root::Delivery) {
    let mut requires = Vec::new();
    match delivery {
        root::Delivery::Application { to, outcome } => {
            assert!(matches!(
                outcome,
                Some(temper_engine_domain::ApplicationAnswer::Adopt(forge::ApplicationOutcome::Adopted(_)))
            ));
            let call = to.into_token().raw();
            if let Some(key) = peers.requests.remove(&call) {
                requires.push(r::Receipt::Party(1, key));
            }
            peers.adopted = true;
        }
        root::Delivery::Assigned { channel, assignment } => link::assigned(peers, channel, assignment),
        root::Delivery::Link { down, .. } => requires = link::down(peers, Time::from_nanos(clock.now), down),
        root::Delivery::WebReply { to, reply } => {
            let call = to.into_token().raw();
            // Admission pressure is retried; a refusal saves no answer
            // (jig b7bdd9e8: refusal facts and `Again`).
            match &reply {
                people::Reply::Again(_) | people::Reply::Full { .. } => {
                    peers.ask_retries.insert(call, clock.now + 1_000_000_000);
                }
                people::Reply::Outcome(_) | people::Reply::Refused(_) | people::Reply::SignedIn { .. } => {
                    peers.ask_bodies.remove(&call);
                }
                people::Reply::SignedOut | people::Reply::Signer(_) => {}
            }
            let key = peers.requests.remove(&call);
            // Seen watermarks have no kept request answer.
            if matches!(reply, people::Reply::Outcome(_))
                && !matches!(reply, people::Reply::Outcome(people::Outcome::MarkedSeen { .. }))
                && let Some(key) = key
            {
                requires.push(r::Receipt::Party(1, key));
            }
            if let people::Reply::SignedIn { sign_in, .. } = reply {
                peers.sign_in = Some(sign_in);
                requires.push(r::Receipt::SignIn(sign_in));
            }
        }
        root::Delivery::Result { task, person, .. } => {
            requires.push(r::Receipt::Ended(task));
            peers.observed.push(r::Observed::Result { task, to: r::Source::Person(person) });
        }
        _ => {}
    }
    if !requires.is_empty() {
        peers.observed.push(r::Observed::Released { requires });
    }
}

fn destination(to: &mut ReplyTo) -> u64 {
    let token = std::mem::replace(to, ReplyTo::new(Token::new(0))).into_token();
    *to = ReplyTo::new(token);
    token.raw()
}

pub(crate) fn chat(peers: &Peers, store: &Store, project: u32, words: &[u8]) -> people::Ask {
    let (configuration, limits) = config::root(peers.seed, peers.scenario, peers.story);
    let creating = jig_core_world::fixtures::Creating::new(&configuration.core, limits.tasks.tree_tasks);
    temper_engine_domain_world::walking::configured_chat(
        &creating,
        &store.rows,
        limits.tasks.tree_tasks,
        project,
        words,
    )
}
