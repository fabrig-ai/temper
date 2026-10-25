//! Whole worker stories: smith's process boundary, the protocol layer's
//! hand-outs played with smith's codecs, the real checkout, fake disk and
//! forge. The world plays jig's worker link as jig's worker worlds do: a
//! channel ready or lost, the grace passed. The referee reads remote trees
//! and records crossing boundaries.
use std::collections::VecDeque;
use std::fmt::Write;

use skein_lib::{Duration, Env, Queue, Time, Token, Wall};
use temper_fake_checkout::{
    Checkout,
    git::{Remote, Tree},
};
use temper_worker_checkout_world::{forge::Forge, translate};
use temper_worker_domain::{
    Domain, Event, Limits, Request, agent, checkout::git::Op, fire, host, max_out, resume, step, wire, worst_case,
};

use crate::protocol;

const RUN: Token = Token::new(81);
const ATTEMPT: Token = Token::new(13);
const WORKSTREAM: u64 = 81;
const PROCESS: Token = Token::new(707);
const CALL: Token = Token::new(44);
const REMOTE: &[u8] = b"org/app";
const PATH: &[u8] = b"code";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    Delivered,
    CancelledDelivery,
    ContactWithin,
    ContactPast,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub tree: Tree,
    pub trace: Vec<String>,
    pub cancellations: u32,
    pub saves: u32,
    pub stop_bound: Duration,
}

struct World {
    domain: Domain,
    env: Env<Limits>,
    out: Queue<Request>,
    owed: VecDeque<Event>,
    forge: Forge,
    disk: Checkout,
    first: u64,
    workspace: Option<Token>,
    process: Option<Token>,
    reading: bool,
    /// The channel to the engine is open: what goes up reaches it.
    up: bool,
    hold_push: bool,
    held: Option<(Token, Op)>,
    deliveries: u32,
    answer: Option<(host::Answer, wire::Work, Option<wire::Preparation>)>,
    /// The workstreams the last hello named.
    workstreams: Box<[u64]>,
    stops: u32,
    report: Report,
}

impl World {
    fn new(seed: u64) -> Self {
        let mut limits = crate::limits();
        limits.host.slots = 1;
        limits.checkout.workspaces = 1;
        limits.checkout.repositories = 1;
        limits.agent.agents = 1;
        limits.agent.no_progress = Duration::from_secs(120);
        assert!(worst_case(&limits).is_some());
        let mut forge = Forge::new(seed);
        let first = forge.repository(REMOTE, b"main", [(PATH.to_vec(), b"original\n".to_vec())].into());
        forge.create_branch(REMOTE, b"topic", first).expect("topic");
        forge.create_branch(REMOTE, b"saved", first).expect("saved");
        Self {
            domain: Domain::new(&limits),
            out: Queue::with_capacity(max_out(&limits)),
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
            owed: VecDeque::new(),
            forge,
            disk: Checkout::new(),
            first,
            workspace: None,
            process: None,
            reading: false,
            up: false,
            hold_push: false,
            held: None,
            deliveries: 0,
            answer: None,
            workstreams: Box::new([]),
            stops: 0,
            report: Report {
                tree: Tree::new(),
                trace: Vec::new(),
                cancellations: 0,
                saves: 0,
                stop_bound: Duration::ZERO,
            },
        }
    }

    fn event(&mut self, event: Event) {
        self.report.trace.push(format!("{:?} -> {event:?}", self.env.now));
        step(&mut self.domain, &self.env, event, &mut self.out);
        self.settle();
    }

    fn settle(&mut self) {
        for _ in 0..256 {
            while let Some(request) = self.out.pop() {
                self.report.trace.push(format!("{:?} <- {request:?}", self.env.now));
                self.take(request);
            }
            self.domain.reclaim();
            if self.domain.is_due(self.env.now) {
                fire(&mut self.domain, &self.env, &mut self.out);
                continue;
            }
            if self.domain.is_ready() {
                resume(&mut self.domain, &self.env, &mut self.out);
                continue;
            }
            let Some(event) = self.owed.pop_front() else { return };
            self.report.trace.push(format!("{:?} -> {event:?}", self.env.now));
            step(&mut self.domain, &self.env, event, &mut self.out);
        }
        panic!("bounded script failed to settle");
    }

    fn say(&mut self, message: agent::Up) {
        assert!(self.reading, "one terminal for the granted read");
        self.reading = false;
        let owner = self.process.expect("spawned");
        self.event(Event::Process(Box::new(agent::process::Event::Received { owner, message })));
    }

    fn take(&mut self, request: Request) {
        match request {
            Request::Source { .. } | Request::SourceFault => panic!("this fixture declares no program sources"),
            Request::Hello(hello) => {
                assert!(hello.hosting.len() <= 1);
                self.report.stop_bound = hello.stop_bound;
                self.workstreams = hello.workstreams;
            }
            Request::Link(up) => Self::link(&up),
            Request::Answer { run, attempt, answer, work, preparation } => {
                assert_eq!((run, attempt), (RUN, ATTEMPT));
                assert!(self.held.is_none(), "delivery settles before the final answer");
                if self.up {
                    assert!(self.answer.replace((answer, work, preparation)).is_none());
                }
            }
            Request::Stop => self.stops += 1,
            Request::Process(request) => self.process(*request),
            Request::Decode { agent, run, start } => {
                let start = protocol::decode(run, *start, &self.env.limits.agent);
                self.owed.push_back(Event::Decoded { agent, start });
            }
            Request::Encode { owner, value } => {
                self.owed.push_back(Event::Encoded { owner, encoded: protocol::encode(value) });
            }
            Request::Fact { owner, body } => {
                self.owed.push_back(Event::Activity { owner, activity: protocol::activity(&body) });
            }
            Request::Io { owner, op, deadline } => self.io(owner, op, deadline),
            Request::CancelRelay { .. } | Request::CancelIo { .. } => panic!("unscripted request"),
        }
    }

    fn link(up: &host::LinkUp) {
        match up {
            host::LinkUp::Activity { run, attempt, .. } => assert_eq!((*run, *attempt), (RUN, ATTEMPT)),
            host::LinkUp::Logins { .. }
            | host::LinkUp::SignedOut { .. }
            | host::LinkUp::Piece { .. }
            | host::LinkUp::Turn { .. }
            | host::LinkUp::Call { .. }
            | host::LinkUp::Unauthorized { .. }
            | host::LinkUp::Limited { .. }
            | host::LinkUp::AccountRefused { .. }
            | host::LinkUp::Answer { .. } => panic!("unscripted link record: {up:?}"),
        }
    }

    fn io(&mut self, owner: Token, op: Op, deadline: Time) {
        assert!(deadline > self.env.now);
        if let Op::Make { workspace } = &op {
            self.workspace = Some(*workspace);
        }
        if let Op::Commit { title, body, .. } = &op {
            if title.as_ref() == b"Save unfinished work" {
                self.report.saves += 1;
            } else {
                assert_eq!(title.as_ref(), b"Delivery");
                assert_eq!(body.as_ref(), b"Worker story");
            }
        }
        if self.hold_push && matches!(op, Op::Push { .. }) {
            assert!(self.held.replace((owner, op)).is_none());
        } else {
            self.forge.at(self.env.now);
            let done = translate::perform(&mut self.forge, &mut self.disk, op);
            self.owed.push_back(Event::Done { owner, done });
        }
    }

    fn process(&mut self, request: agent::process::Request) {
        match request {
            agent::process::Request::Spawn { owner, deadline, .. } => {
                assert!(deadline > self.env.now);
                assert!(self.process.replace(owner).is_none());
                let spawned = agent::process::Event::Spawned { owner, process: PROCESS };
                self.owed.push_back(Event::Process(Box::new(spawned)));
            }
            agent::process::Request::Send { owner, process, message } => {
                assert_eq!((Some(owner), process), (self.process, PROCESS));
                self.down(message);
                self.owed.push_back(Event::Process(Box::new(agent::process::Event::Sent { owner })));
            }
            agent::process::Request::Read { owner, process } => {
                assert_eq!((Some(owner), process), (self.process, PROCESS));
                assert!(!self.reading);
                self.reading = true;
            }
            agent::process::Request::Wait { owner, process } | agent::process::Request::Reap { owner, process } => {
                assert_eq!((Some(owner), process), (self.process, PROCESS));
            }
            agent::process::Request::Signal { owner, .. } => {
                self.owed.push_back(Event::Process(Box::new(agent::process::Event::Signalled { owner })));
            }
        }
    }

    fn down(&mut self, message: agent::Down) {
        match message {
            agent::Down::Start { start, .. } => {
                assert_eq!(start.activation, ATTEMPT.raw());
                assert_eq!(start.directories.len(), 1);
                assert_eq!(start.directories[0].name.as_ref(), b"app");
                assert_eq!(start.workspace, Some(start.directories[0].root));
            }
            agent::Down::Answer { call, reply } => {
                assert_eq!(call, CALL);
                let agent::Reply::Delivery(agent::Delivery::Delivered(receipts)) = reply else {
                    panic!("the complete edited tree is delivered: {reply:?}");
                };
                assert_eq!(receipts.receipts().len(), 1);
                assert_eq!(receipts.receipts()[0].directory(), 0);
                let landed = self.forge.branch(REMOTE, b"topic").expect("delivered branch");
                let mut encoded = String::with_capacity(64);
                for byte in translate::commit(landed).raw() {
                    write!(&mut encoded, "{byte:02x}").expect("String write");
                }
                assert_eq!(receipts.receipts()[0].text(), encoded.as_bytes(), "the receipt identifies the pushed head");
                self.deliveries += 1;
            }
            agent::Down::Cancel => self.report.cancellations += 1,
            agent::Down::Acknowledge { .. } | agent::Down::Grant { .. } | agent::Down::Message { .. } => {
                panic!("unscripted process record")
            }
        }
    }

    fn connect(&mut self) {
        self.up = true;
        self.event(Event::Ready);
    }

    fn start(&mut self) {
        self.start_from(wire::Start::Base { branch: Box::from(&b"main"[..]) });
    }

    fn start_from(&mut self, start: wire::Start) {
        self.connect();
        self.assign(start);
        self.say(agent::Up::Admitted);
        let place = temper_worker_domain::checkout::git::Place {
            git: None,
            workspace: self.workspace.expect("prepared"),
            repository: Box::from(&b"app"[..]),
        };
        self.disk.write(&[translate::path(&place).as_slice(), b"/code"].concat(), b"edited\n");
    }

    /// The engine assigns the run, its one repository starting at `start`.
    fn assign(&mut self, start: wire::Start) {
        let repository = wire::Repository {
            git: None,
            tag: 1,
            name: Box::from(&b"app"[..]),
            remote: Box::from(REMOTE),
            identity: 0,
            account: None,
            start,
            access: wire::Access::Writable {
                push: Box::from(&b"topic"[..]),
                expected: Some(translate::commit(self.first).raw()),
                landing: None,
            },
        };
        let assignment = host::Assignment {
            messages: Box::new([]),
            turns: Box::new([]),
            answered: Box::new([]),
            assignment: host::RunAssignment {
                identity: [9; 16],
                live: host::LiveAsk::NONE,
                run: RUN,
                attempt: ATTEMPT,
                workspace: Some(host::Workspace { workstream: WORKSTREAM, items: Token::new(0) }),
                save: true,
                charter: protocol::charter(),
                grants: Box::new([]),
            },
        };
        let items = wire::Workspace { repositories: Box::new([repository]) };
        self.event(Event::Assign { assignment, items: Ok(items), save: Some(Box::from(&b"saved"[..])) });
    }

    fn deliver(&mut self) {
        self.say(agent::Up::Call {
            call: CALL,
            name: agent::CallName { activation: ATTEMPT.raw(), completion: 1, position: 0 },
            deadline: self.env.now.saturating_add(Duration::from_secs(60)),
            ask: protocol::delivery(b"Delivery", b"Worker story"),
        });
    }

    fn finish(&mut self) {
        let answer = agent::Answer { read: None, turns: 0, spent: 0, result: protocol::report(b"done") };
        self.say(agent::Up::Answer { answer });
    }

    fn gone(&mut self) {
        let owner = self.process.expect("spawned");
        let exited = agent::process::Event::Exited { owner, exit: Some(agent::Exit::Code(0)) };
        self.event(Event::Process(Box::new(exited)));
        assert!(self.reading);
        self.reading = false;
        self.event(Event::Process(Box::new(agent::process::Event::Hangup { owner })));
        let reaped = agent::process::Event::Reaped { owner, detail: Box::new([]) };
        self.event(Event::Process(Box::new(reaped)));
    }

    fn story(&mut self, case: Case) {
        match case {
            Case::Delivered => {
                self.deliver();
                assert_eq!(self.deliveries, 1);
                self.finish();
                self.gone();
            }
            Case::CancelledDelivery => {
                self.hold_push = true;
                self.deliver();
                assert!(self.held.is_some());
                assert_eq!(self.forge.branch(REMOTE, b"topic"), Some(self.first));
                self.event(Event::Link(host::LinkDown::Cancel { run: RUN, attempt: ATTEMPT }));
                assert_eq!(self.report.cancellations, 1);
                self.gone();
                assert!(self.answer.is_none());
                self.hold_push = false;
                let (owner, op) = self.held.take().expect("push held");
                let done = translate::perform(&mut self.forge, &mut self.disk, op);
                self.event(Event::Done { owner, done });
            }
            Case::ContactWithin => {
                self.up = false;
                self.event(Event::Lost);
                self.env.now = self.env.now.saturating_add(Duration::from_secs(1));
                assert_eq!(self.report.cancellations, 0);
                self.connect();
                self.deliver();
                self.finish();
                self.gone();
            }
            Case::ContactPast => {
                self.up = false;
                self.event(Event::Lost);
                self.env.now = self.env.now.saturating_add(Duration::from_secs(60));
                self.event(Event::GracePassed);
                assert_eq!(self.report.cancellations, 1);
                self.gone();
                assert_eq!(self.report.saves, 1, "save before the cancellation answer reaches the engine");
                assert!(self.answer.is_none());
                self.connect();
            }
        }
    }

    fn run(mut self, case: Case) -> Report {
        self.start();
        self.story(case);
        assert_eq!(self.domain.agent().agents(), 0);
        assert_eq!(self.domain.workspaces(), 0);
        let (answer, work, preparation) = self.answer.take().expect("a terminal answer");
        assert_eq!(preparation, None);
        let branch = if case == Case::ContactPast { b"saved".as_slice() } else { b"topic".as_slice() };
        let commit = self.forge.branch(REMOTE, branch).expect("worker pushed");
        let expected = match case {
            Case::Delivered | Case::ContactWithin => None,
            Case::CancelledDelivery => Some(host::Reason::Engine),
            Case::ContactPast => Some(host::Reason::Contact),
        };
        let cancelled = match answer.ending {
            host::Ending::Ended { .. } => None,
            host::Ending::Failed { failure: host::Failure::Cancelled(reason), .. } => Some(reason),
            ending @ (host::Ending::Refused(_) | host::Ending::Parked { .. } | host::Ending::Failed { .. }) => {
                panic!("ending did not follow the story: {ending:?}")
            }
        };
        assert_eq!(cancelled, expected);
        if case == Case::ContactPast {
            let saved = work.saved.expect("saved work");
            let [wire::SavedRepository { landing: wire::Landing::Landed { commit: saved }, .. }] = saved.as_ref()
            else {
                panic!("one saved head");
            };
            assert_eq!(*saved, translate::commit(commit).raw());
        } else {
            assert_eq!(work.landed.as_ref(), [wire::Landed { tag: 1, commit: translate::commit(commit).raw() }]);
        }
        self.report.tree = self.forge.tree(commit);
        assert_eq!(self.report.tree, [(PATH.to_vec(), b"edited\n".to_vec())].into(), "the entire edited tree landed");
        assert!(self.forge.is_ancestor(self.first, commit));
        self.event(Event::Link(host::LinkDown::Acknowledge { run: RUN, attempt: ATTEMPT }));
        assert_eq!(self.domain.kept_answers(), 0);
        self.event(Event::Shutdown);
        assert_eq!(self.stops, 1, "a worker with nothing left stops, once");
        self.report
    }
}

#[must_use]
pub fn run(seed: u64, case: Case) -> Report {
    World::new(seed).run(case)
}

/// A run starting at `start`, whose unfinished edit is saved after contact
/// is lost past the grace: its work, with each repository's tag and start.
#[must_use]
pub fn saved(seed: u64, start: wire::Start) -> wire::Work {
    let mut world = World::new(seed);
    world.start_from(start);
    world.story(Case::ContactPast);
    assert_eq!(world.domain.agent().agents(), 0);
    assert_eq!(world.domain.workspaces(), 0);
    let (answer, work, preparation) = world.answer.take().expect("a terminal answer");
    assert_eq!(preparation, None);
    assert!(matches!(
        answer.ending,
        host::Ending::Failed { failure: host::Failure::Cancelled(host::Reason::Contact), .. }
    ));
    world.event(Event::Link(host::LinkDown::Acknowledge { run: RUN, attempt: ATTEMPT }));
    assert_eq!(world.domain.kept_answers(), 0);
    world.event(Event::Shutdown);
    assert_eq!(world.stops, 1);
    work
}

/// A run whose repository starts from a branch the forge does not have: how
/// its answer ends, and the preparation's detail beside it.
#[must_use]
pub fn unprepared(seed: u64) -> (host::Ending, Option<wire::Preparation>) {
    let mut world = World::new(seed);
    world.connect();
    world.assign(wire::Start::Branch { branch: Box::from(&b"gone"[..]) });
    let (answer, _, preparation) = world.answer.take().expect("an answer");
    (answer.ending, preparation)
}

/// The workstreams a worker's hello names once a run delivered and went.
#[must_use]
pub fn workstreams(seed: u64) -> Box<[u64]> {
    let mut world = World::new(seed);
    world.start();
    world.story(Case::Delivered);
    world.event(Event::Link(host::LinkDown::Acknowledge { run: RUN, attempt: ATTEMPT }));
    world.up = false;
    world.event(Event::Lost);
    world.connect();
    world.workstreams
}
