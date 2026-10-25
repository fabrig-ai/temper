//! Runtime world. The real root, hub, checkout and agent child domains meet
//! a scripted engine and process, the protocol layer's hand-outs answered
//! with smith's codecs, and deterministic git terminals.

use std::collections::VecDeque;
use std::fmt::{self, Write};

use skein_lib::{Duration, Env, Queue, Rng, Time, Token, Wall};
use skein_world::domain::heap::Meter;
use temper_worker_domain::checkout::git::{Commit, Done, Op, Want};
use temper_worker_domain::{Domain, Event, Limits, Request, agent, host, max_out, resume, step, wire, worst_case};

use crate::protocol;

pub const RUN: Token = Token::new(31);
pub const ATTEMPT: Token = Token::new(932);
const WORKSTREAM: u64 = 31;
const PROCESS: Token = Token::new(777);
const CALL: Token = Token::new(71);
const HEAD: [u8; 32] = [11; 32];
const BASE: [u8; 32] = [23; 32];
const LANDED: [u8; 32] = [47; 32];
const PATH: &[u8] = b"src/merge.rs";
const CAPACITY: usize = 2048;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Settings {
    pub seed: u64,
    pub turns: u32,
    pub byte_slots: u32,
    pub merging: bool,
    pub close: Close,
}

/// How the worker shuts down once its run has taken its turns.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Close {
    /// The engine has every turn and the answer first.
    Committed,
    /// The engine has the answer and some turns, then stays out of reach
    /// past the grace.
    Abandoned,
    /// The engine has some turns only, then stays out of reach past the
    /// grace.
    Unacknowledged,
    /// The channel stays down past the grace as the run winds down, and
    /// opens again before it ends.
    Reopened,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Stats {
    pub turns: u32,
    pub copies: u32,
    pub busy: u32,
    pub reconnects: u32,
    pub commits: u32,
    pub saves: u32,
    pub peak_retained: u32,
    pub abandoned: u64,
    pub peak_heap: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Observed {
    number: u32,
    spent: u64,
    length: usize,
    digest: u64,
}

/// A formatting sink with no allocation; full boundary records and facts
/// enter the trace, including byte bodies. No trace block enters the meter.
struct Digest(u64);
impl Write for Digest {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for byte in text.bytes() {
            self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(1_099_511_628_211);
        }
        Ok(())
    }
}
fn digest(value: &impl fmt::Debug) -> u64 {
    let mut writer = Digest(14_695_981_039_346_656_037);
    write!(writer, "{value:?}").expect("the digest accepts every byte");
    writer.0
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Bodies {
    Varied,
    Full,
}

pub struct World {
    settings: Settings,
    env: Env<Limits>,
    domain: Domain,
    out: Queue<Request>,
    owed: VecDeque<Event>,
    trace: Vec<u64>,
    boundaries: Vec<u64>,
    observed: Vec<Observed>,
    meter: Meter,
    bound: u64,
    rng: Rng,
    agent: Option<Token>,
    reading: bool,
    unresolved: bool,
    stops: u32,
    bodies: Bodies,
    answer: Option<(u32, u64, u64)>,
    stats: Stats,
}

/// The worker's limits for `settings`: one run of one repository, resumed
/// from a transcript of one turn, its turn window and smith's bytes as the
/// settings say, the hub's bytes room for smith's window; the facts dropped
/// unless `keep`.
fn limits(settings: Settings, keep: bool) -> Limits {
    let (encoded, owned) = crate::turn_bytes();
    let mut limits = crate::limits();
    limits.host.slots = 1;
    limits.checkout.repositories = 1;
    let transcript = protocol::transcript_turn(1, crate::TURN_TEXT, true);
    limits.host.transcript_bytes = u64::try_from(transcript.len() + size_of::<Box<[u8]>>()).expect("bounded");
    limits.checkout.conflicts = 1;
    limits.checkout.path_bytes = u32::try_from(PATH.len()).expect("bounded");
    limits.agent.conflicts = limits.checkout.conflicts;
    limits.host.turns = settings.turns;
    limits.agent.turns = settings.turns;
    limits.host.turn_queue_bytes = u64::from(settings.byte_slots.max(settings.turns)) * encoded;
    limits.agent.unacknowledged_bytes = u64::from(settings.byte_slots) * owned;
    limits.bounds.unacknowledged = settings.turns;
    if !keep {
        limits.host.facts = 0;
        limits.checkout.facts = 0;
        limits.agent.facts = 0;
        limits.host.told = 0;
    }
    limits
}

impl World {
    #[must_use]
    pub fn new(settings: Settings) -> Self {
        Self::with_facts(settings, true)
    }

    /// Runs the same records with facts and the runs' activity dropped.
    #[must_use]
    pub fn without_facts(settings: Settings) -> Self {
        Self::with_facts(settings, false)
    }

    fn with_facts(settings: Settings, keep: bool) -> Self {
        assert!(settings.turns > 0 && settings.byte_slots > 0);
        let limits = limits(settings, keep);
        let bound = worst_case(&limits).expect("world limits fit");
        World {
            settings,
            out: Queue::with_capacity(max_out(&limits)),
            domain: Domain::new(&limits),
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits },
            owed: VecDeque::with_capacity(CAPACITY),
            trace: Vec::with_capacity(CAPACITY),
            boundaries: Vec::with_capacity(CAPACITY),
            observed: Vec::with_capacity(64),
            meter: Meter::new(),
            bound,
            rng: Rng::new(settings.seed),
            agent: None,
            reading: false,
            unresolved: settings.merging,
            stops: 0,
            bodies: Bodies::Varied,
            answer: None,
            stats: Stats::default(),
        }
    }

    /// Saturates every retained body, transcript and conflict path in the
    /// counted-memory scenarios; the random worlds keep varied body sizes.
    pub fn fill_turns(&mut self) {
        self.bodies = Bodies::Full;
    }

    #[must_use]
    pub const fn stats(&self) -> Stats {
        self.stats
    }

    #[must_use]
    pub fn trace(&self) -> &[u64] {
        &self.trace
    }

    #[must_use]
    pub fn boundaries(&self) -> &[u64] {
        &self.boundaries
    }

    fn remember(&mut self, record: &impl fmt::Debug) {
        assert!(self.boundaries.len() < CAPACITY, "world boundary trace is bounded");
        self.boundaries.push(digest(record));
        self.remember_fact(record);
    }

    fn remember_fact(&mut self, record: &impl fmt::Debug) {
        assert!(self.trace.len() < CAPACITY, "world trace is bounded");
        self.trace.push(digest(record));
    }

    fn send(&mut self, event: Event) {
        self.remember(&event);
        self.meter.start();
        step(&mut self.domain, &self.env, event, &mut self.out);
        self.drain();
        self.settle();
    }

    fn drain(&mut self) {
        let measured = self.meter.end();
        for _ in 0..max_out(&self.env.limits) {
            let Some(request) = self.out.pop() else { break };
            // The runs' activity is best effort, as facts are.
            if let Request::Link(host::LinkUp::Activity { .. }) = &request {
                self.remember_fact(&request);
            } else {
                self.remember(&request);
                self.take(request);
            }
        }
        assert!(self.out.is_empty(), "max_out covers a replay burst");
        for _ in 0..1024 {
            let Some(fact) = self.domain.pop_fact() else { break };
            self.remember_fact(&fact);
        }
        self.meter.check(measured, self.bound, &self.settings);
        self.stats.peak_heap = self.stats.peak_heap.max(self.meter.held());
        self.stats.peak_retained = self.stats.peak_retained.max(self.domain.retained_turns());
        self.domain.reclaim();
    }

    fn settle(&mut self) {
        for _ in 0..128 {
            if self.domain.is_ready() {
                self.remember(&"resume");
                self.meter.start();
                resume(&mut self.domain, &self.env, &mut self.out);
                self.drain();
                continue;
            }
            let Some(event) = self.owed.pop_front() else { return };
            self.remember(&event);
            self.meter.start();
            step(&mut self.domain, &self.env, event, &mut self.out);
            self.drain();
        }
        panic!("the world owes at most 128 immediate terminals");
    }

    fn take(&mut self, request: Request) {
        match request {
            Request::Source { .. } | Request::SourceFault => panic!("this fixture declares no program sources"),
            Request::Hello(hello) => {
                assert_eq!(hello.stop_bound, crate::STOP_BOUND);
                assert!(hello.hosting.len() <= 1);
            }
            Request::Link(host::LinkUp::Turn { run, attempt, turn }) => self.turned(run, attempt, &turn),
            Request::Link(up) => panic!("unscripted link record: {up:?}"),
            Request::Answer { run, attempt, answer, work, preparation } => {
                assert_eq!((run, attempt, preparation), (RUN, ATTEMPT, None));
                self.answered(&answer, &work);
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
            Request::Io { owner, op, deadline } => {
                assert!(deadline > self.env.now);
                let done = self.git(op);
                self.owed.push_back(Event::Done { owner, done });
            }
            Request::CancelRelay { .. } | Request::CancelIo { .. } => panic!("unscripted output"),
        }
    }

    /// The referee of turns: a copy is the turn it copies, whole, and none
    /// comes before the turns before it.
    fn turned(&mut self, run: Token, attempt: Token, turn: &host::Turn) {
        assert_eq!((run, attempt), (RUN, ATTEMPT));
        let received =
            Observed { number: turn.turn, spent: turn.spent, length: turn.body.len(), digest: digest(&turn.body) };
        if let Some(previous) = self.observed.iter().find(|old| old.number == turn.turn) {
            assert_eq!(previous, &received, "replay preserves the full body and spend");
        } else {
            assert_eq!(turn.turn, u32::try_from(self.observed.len()).expect("bounded") + 1, "no turn gap");
            self.observed.push(received);
            self.stats.turns += 1;
        }
        self.stats.copies += 1;
    }

    fn answered(&mut self, answer: &host::Answer, work: &wire::Work) {
        assert_eq!(answer.turns, u32::try_from(self.observed.len()).expect("bounded"));
        let host::Ending::Parked { .. } = &answer.ending else { panic!("script only parks: {answer:?}") };
        assert_eq!(work.saved.is_some(), !self.settings.merging, "a merge in progress saves nothing");
        let received = (answer.turns, answer.spent, digest(&(answer, work)));
        if let Some(old) = self.answer {
            assert_eq!(old, received, "answer replay is identical");
        }
        self.answer = Some(received);
    }

    fn process(&mut self, request: agent::process::Request) {
        match request {
            agent::process::Request::Spawn { owner, workspace: _, deadline } => {
                assert!(deadline > self.env.now);
                assert!(self.agent.is_none());
                self.agent = Some(owner);
                let spawned = agent::process::Event::Spawned { owner, process: PROCESS };
                self.owed.push_back(Event::Process(Box::new(spawned)));
            }
            agent::process::Request::Send { owner, process, message } => {
                assert_eq!((Some(owner), process), (self.agent, PROCESS));
                self.down(message);
                self.owed.push_back(Event::Process(Box::new(agent::process::Event::Sent { owner })));
            }
            agent::process::Request::Read { owner, process } => {
                assert_eq!((Some(owner), process), (self.agent, PROCESS));
                assert!(!self.reading);
                self.reading = true;
            }
            agent::process::Request::Wait { owner, process } | agent::process::Request::Reap { owner, process } => {
                assert_eq!((Some(owner), process), (self.agent, PROCESS));
            }
            agent::process::Request::Signal { .. } => panic!("unscripted signal"),
        }
    }

    fn down(&self, message: agent::Down) {
        match message {
            agent::Down::Start { start, .. } => {
                assert!(start.transcript.is_some(), "the run resumes from the engine's transcript");
                assert_eq!(start.directories.len(), 1);
                assert!(start.grants.is_empty());
                let expected: &[Box<[u8]>] = if self.settings.merging { &[Box::from(PATH)] } else { &[] };
                assert_eq!(&*start.directories[0].conflicts, expected);
            }
            agent::Down::Answer { call, reply } => {
                assert_eq!(call, CALL);
                match reply {
                    agent::Reply::Delivery(agent::Delivery::Refused(refusal)) => {
                        assert_eq!(refusal.marker().expect("conflict marker").path(), PATH);
                    }
                    agent::Reply::Delivery(agent::Delivery::Delivered(_)) => {}
                    other @ (agent::Reply::Host { .. }
                    | agent::Reply::Delivery(
                        agent::Delivery::Nothing | agent::Delivery::Stale | agent::Delivery::Failed(_),
                    )
                    | agent::Reply::Busy
                    | agent::Reply::Unavailable
                    | agent::Reply::Withdrawn
                    | agent::Reply::TooLarge) => panic!("unexpected delivery result: {other:?}"),
                }
            }
            agent::Down::Acknowledge { .. } | agent::Down::Cancel => {}
            agent::Down::Message { .. } | agent::Down::Grant { .. } => panic!("unscripted down record"),
        }
    }

    fn git(&mut self, op: Op) -> Done {
        match op {
            Op::Make { .. } | Op::Clone { .. } | Op::Create { .. } | Op::CheckOut { .. } => Done::Succeeded,
            Op::Fetch { want, .. } => {
                let commit = match want {
                    Want::Branch { branch } => {
                        if branch.as_ref() == b"main" {
                            BASE
                        } else {
                            HEAD
                        }
                    }
                    Want::Commit { commit } => commit.raw(),
                    Want::Default => BASE,
                };
                Done::Fetched { commit: Commit::new(commit) }
            }
            Op::IsAncestor { .. } => panic!("this fixture has no producer retry"),
            Op::Merge { theirs, .. } => {
                assert_eq!(theirs.raw(), BASE);
                Done::Conflicted { files: Box::new([Box::from(PATH)]) }
            }
            Op::Commit { title, body, merging, .. } => {
                self.stats.commits += 1;
                if title.as_ref() == b"Save unfinished work" {
                    self.stats.saves += 1;
                    assert!(!self.settings.merging);
                } else {
                    assert_eq!(title.as_ref(), b"short title");
                    assert_eq!(body.as_ref(), b"a separate, longer body");
                }
                if self.settings.merging {
                    assert_eq!(merging, Some(Commit::new(BASE)));
                }
                if self.unresolved {
                    Done::Conflicted { files: Box::new([Box::from(PATH)]) }
                } else {
                    Done::Committed { commit: Commit::new(LANDED) }
                }
            }
            Op::Push { branch, expected, .. } => {
                if branch.as_ref() == b"topic" {
                    assert_eq!(expected, Some(Commit::new(HEAD)));
                } else {
                    assert_eq!(branch.as_ref(), b"saved");
                    assert_eq!(expected, None);
                }
                Done::Succeeded
            }
        }
    }

    fn deliver(&mut self, completion: u32) {
        self.say(agent::Up::Call {
            call: CALL,
            name: agent::CallName { activation: ATTEMPT.raw(), completion, position: 0 },
            deadline: self.env.now.saturating_add(Duration::from_secs(60)),
            ask: protocol::delivery(b"short title", b"a separate, longer body"),
        });
    }

    fn say(&mut self, message: agent::Up) {
        assert!(self.reading, "the agent may speak only after a granted read");
        self.reading = false;
        let owner = self.agent.expect("agent starts first");
        self.send(Event::Process(Box::new(agent::process::Event::Received { owner, message })));
    }

    fn link(&mut self, down: host::LinkDown) {
        self.send(Event::Link(down));
    }

    fn ack(&mut self, turn: u32) {
        self.link(host::LinkDown::TurnKept { run: RUN, attempt: ATTEMPT, turn });
    }

    fn reconnect(&mut self) {
        self.send(Event::Lost);
        self.send(Event::Ready);
        self.stats.reconnects += 1;
    }

    fn assign(&mut self) {
        let repository = wire::Repository {
            git: None,
            tag: 19,
            name: Box::from(&b"app"[..]),
            remote: Box::from(&b"org/app"[..]),
            identity: 0,
            account: None,
            start: if self.settings.merging {
                wire::Start::Merge { branch: Box::from(&b"topic"[..]), base: BASE }
            } else {
                wire::Start::Base { branch: Box::from(&b"main"[..]) }
            },
            access: wire::Access::Writable { push: Box::from(&b"topic"[..]), expected: Some(HEAD), landing: None },
        };
        let assignment = host::Assignment {
            messages: Box::new([]),
            turns: Box::new([protocol::transcript_turn(1, crate::TURN_TEXT, true)]),
            answered: Box::new([]),
            assignment: host::RunAssignment {
                identity: [7; 16],
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
        self.send(Event::Assign { assignment, items: Ok(items), save: Some(Box::from(&b"saved"[..])) });
    }

    /// One turn of the run, numbered `number`: the engine answers it busy
    /// and has it again, a wrong attempt's acknowledgement frees nothing,
    /// the channel drops and comes back, and the engine commits what it must
    /// for the agent to read on. Returns the turns committed.
    fn turn(&mut self, number: u32, mut committed: u32) -> u32 {
        let text = if self.bodies == Bodies::Full {
            crate::TURN_TEXT
        } else {
            usize::try_from(self.rng.between(1, crate::TURN_TEXT as u64)).expect("a few bytes")
        };
        self.say(agent::Up::Turn { turn: protocol::turn(number, u64::from(number) * 17, text) });
        let retained = self.domain.retained_turns();
        self.link(host::LinkDown::TurnKept { run: RUN, attempt: Token::new(999), turn: number });
        assert_eq!(self.domain.retained_turns(), retained, "wrong attempt cannot release credit");
        self.send(Event::Resend { run: RUN, attempt: ATTEMPT, turn: number });
        self.stats.busy += 1;
        self.reconnect();
        assert!(retained <= self.settings.turns);
        if retained == self.settings.turns {
            committed += 1;
            self.ack(committed);
        }
        for _ in 0..self.settings.turns {
            if self.reading {
                break;
            }
            committed += 1;
            self.ack(committed);
            self.ack(committed);
        }
        assert!(self.reading, "committing enough bytes restores maximum-turn credit");
        committed
    }

    /// Runs a complete scenario with bounded turns and deterministic losses,
    /// busy replies, duplicate or wrong acknowledgements, and a crossed
    /// answer acknowledgement; then shuts the worker down as the settings
    /// say.
    pub fn run(&mut self) {
        self.send(Event::Ready);
        self.assign();
        self.say(agent::Up::Admitted);
        self.deliver(1);
        if self.settings.merging {
            self.unresolved = false;
            self.deliver(2);
        }
        let count = u32::try_from(self.rng.between(3, 7)).expect("bounded turns");
        let mut committed = 0;
        for number in 1..=count {
            committed = self.turn(number, committed);
        }
        match self.settings.close {
            Close::Reopened => self.reopened(count, committed),
            Close::Committed | Close::Abandoned | Close::Unacknowledged => self.closed(count, committed),
        }
        assert_eq!(self.stops, 1, "the worker stops once");
        assert_eq!(self.stats.turns, count);
        assert!(self.stats.copies >= count * 2);
    }

    /// The run parks after `count` turns, and its agent goes.
    fn park(&mut self, count: u32) {
        let spent = u64::from(count) * 17 + 9;
        let answer = agent::Answer { read: None, turns: count, spent, result: agent::RunResult::Parked };
        self.say(agent::Up::Answer { answer });
        self.gone();
        assert!(self.answer.is_some());
        assert_eq!(self.domain.agent().agents(), 0);
        assert_eq!(self.domain.workspaces(), 0);
    }

    fn gone(&mut self) {
        let agent = self.agent.expect("live agent");
        let exited = agent::process::Event::Exited { owner: agent, exit: Some(agent::Exit::Code(0)) };
        self.send(Event::Process(Box::new(exited)));
        assert!(self.reading);
        self.reading = false;
        self.send(Event::Process(Box::new(agent::process::Event::Hangup { owner: agent })));
        self.send(Event::Process(Box::new(agent::process::Event::Reaped { owner: agent, detail: Box::new([]) })));
    }

    /// The run parks; then the worker shuts down once the engine has every
    /// turn and the answer, or, out of reach past the grace, gives up the
    /// turns it lacks (all but the `committed` first of `count`) and the
    /// answer if it lacks that too.
    fn closed(&mut self, count: u32, committed: u32) {
        self.park(count);
        self.reconnect();
        if self.settings.close != Close::Unacknowledged {
            self.link(host::LinkDown::Acknowledge { run: RUN, attempt: ATTEMPT });
            assert_eq!(self.domain.kept_answers(), 0, "the answer's acknowledgement frees its slot");
        }
        if self.settings.close == Close::Committed {
            for number in committed + 1..=count {
                self.ack(number);
            }
            self.send(Event::Shutdown);
            assert_eq!(self.domain.abandoned(), 0);
            return;
        }
        let lacking = count - committed + u32::from(self.settings.close == Close::Unacknowledged);
        self.send(Event::Shutdown);
        assert_eq!(self.stops, u32::from(lacking == 0), "a worker keeps what the engine lacks while it may come back");
        if lacking > 0 {
            self.send(Event::Lost);
            self.send(Event::GracePassed);
        }
        self.stats.abandoned = self.domain.abandoned();
        assert_eq!(self.stats.abandoned, u64::from(lacking));
    }

    /// The worker shuts down as the run lives; the channel stays down past
    /// the grace, then opens before the run ends: the hello replays every
    /// turn the engine lacks, and nothing is given up.
    fn reopened(&mut self, count: u32, committed: u32) {
        self.send(Event::Shutdown);
        self.send(Event::Lost);
        self.send(Event::GracePassed);
        let copies = self.stats.copies;
        self.send(Event::Ready);
        assert_eq!(self.stats.copies - copies, count - committed, "the hello replays every turn the engine lacks");
        self.park(count);
        for number in committed + 1..=count {
            self.ack(number);
        }
        self.link(host::LinkDown::Acknowledge { run: RUN, attempt: ATTEMPT });
        assert_eq!(self.domain.abandoned(), 0);
    }
}

#[cfg(test)]
mod referee_tests {
    use super::{ATTEMPT, Close, RUN, Settings, World, host};
    use temper_worker_domain::Request;

    fn turn(body: &[u8]) -> Request {
        Request::Link(host::LinkUp::Turn {
            run: RUN,
            attempt: ATTEMPT,
            turn: host::Turn {
                opens_window: false,
                refused: Box::new([]),
                turn: 1,
                spent: 17,
                read: None,
                body: Box::from(body),
            },
        })
    }

    #[test]
    #[should_panic(expected = "replay preserves the full body and spend")]
    fn referee_rejects_a_duplicate_turn_with_changed_opaque_bytes() {
        let settings = Settings { seed: 0, turns: 1, byte_slots: 1, merging: false, close: Close::Committed };
        let mut world = World::new(settings);
        world.take(turn(b"original"));
        world.take(turn(b"different"));
    }

    #[test]
    #[should_panic(expected = "no turn gap")]
    fn referee_rejects_an_unseen_prefix() {
        let settings = Settings { seed: 0, turns: 1, byte_slots: 1, merging: false, close: Close::Committed };
        let mut world = World::new(settings);
        let Request::Link(host::LinkUp::Turn { run, attempt, mut turn }) = turn(b"") else { unreachable!() };
        turn.turn = 2;
        world.take(Request::Link(host::LinkUp::Turn { run, attempt, turn }));
    }
}
