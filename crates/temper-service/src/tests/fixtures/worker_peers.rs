//! Concrete worker peers shared by worker, order and admission tests.
use super::super::fixtures;
use crate::worker_settings;
use jig_domain_host as hub;
use jig_protocol_engine_workers as engine_link;
use jig_protocol_worker_link as link;
use jig_protocol_worker_wire::Part;
use skein_channel::{Lower, LowerEvent};
use skein_io as io;
use skein_lib::{Intake, List, Queue, Time, Token, Wall, stream};
use temper_worker_domain as root;

// Two openings, three retained turns and one final answer.
const LINK_RECORDS: u32 = 6;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum LinkRecord {
    Hello,
    Turn(u32),
    Answer(u32),
}

pub(crate) struct LinkPeer {
    pub(crate) records: List<LinkRecord>,
    pub(crate) component: engine_link::Component,
    pub(crate) out: Queue<engine_link::Request>,
    pub(crate) engine_bytes: Intake,
    pub(crate) worker_bytes: Intake,
    pub(crate) engine_read: Option<stream::Read>,
    pub(crate) worker_read: Option<stream::Read>,
    pub(crate) owner: Token,
}
impl LinkPeer {
    pub(crate) fn new() -> Self {
        let settings = fixtures::worker_settings();
        let prepared =
            worker_settings::prepare(&settings, &fixtures::worker_agent(), &fixtures::worker_limits()).unwrap();
        let limits = engine_link::Limits {
            sizes: prepared.link.sizes,
            names: 1,
            workers: 1,
            opening: 1,
            chunk: prepared.link.chunk,
            handshake: prepared.link.handshake,
            hello: prepared.link.handshake,
            ping: prepared.link.ping,
            silence: prepared.link.silence,
            stall: prepared.link.stall,
        };
        Self {
            records: List::with_capacity(LINK_RECORDS),
            component: engine_link::Component::new(
                &limits,
                Box::new([engine_link::Worker {
                    name: settings.jig.name,
                    digest: io::digest::digest(fixtures::worker_files().contents(b"worker-secret").unwrap()),
                }]),
                temper_protocol_worker_wire::PAYLOAD_VERSION,
            )
            .unwrap(),
            out: Queue::with_capacity(64),
            engine_bytes: Intake::with_capacity(65536),
            worker_bytes: Intake::with_capacity(65536),
            engine_read: None,
            worker_read: None,
            owner: Token::new(prepared.prefix),
        }
    }
    pub(crate) fn open(&mut self, worker: &mut crate::Worker) {
        worker.iterate(fixtures::NOW, Wall::EPOCH);
        worker.up(fixtures::NOW, io::Event::Connecting { owner: self.owner, socket: Token::new(44) });
        worker.up(fixtures::NOW, io::Event::Connected { owner: self.owner });
        self.component.down(fixtures::NOW, engine_link::Event::Accepted { connection: Token::new(1) }, &mut self.out);
        for _ in 0_u16..1000 {
            if let Some(engine_link::Request::Hello { .. }) = self.turn(worker) {
                return;
            }
        }
        panic!("worker opens and says hello");
    }
    pub(crate) fn answer(&mut self, worker: &mut crate::Worker) -> (hub::LinkUp, Box<[Part]>) {
        for _ in 0_u16..1000 {
            if let Some(engine_link::Request::Up { up: up @ hub::LinkUp::Answer { .. }, parts, .. }) = self.turn(worker)
            {
                return (up, parts);
            }
        }
        panic!("worker sends its answer");
    }
    pub(crate) fn turn(&mut self, worker: &mut crate::Worker) -> Option<engine_link::Request> {
        worker.iterate(fixtures::NOW, Wall::EPOCH);
        for _ in 0..worker.io().len() {
            let request = worker.io().pop().expect("queued IO");
            if !stream_request(&request, &[self.owner, Token::new(44)]) {
                worker.io().push(request);
                continue;
            }
            match request {
                io::Request::Output { down: stream::OutputDown::Room { right, .. }, .. } => worker.up(
                    fixtures::NOW,
                    io::Event::Output {
                        owner: self.owner,
                        up: stream::OutputUp::Settled { right, outcome: stream::OutputOutcome::Granted },
                    },
                ),
                io::Request::Output { down: stream::OutputDown::Send { bytes, .. }, .. } => {
                    self.engine_bytes.append(&bytes).unwrap();
                }
                io::Request::Stream { down: stream::Down::Demand { read, .. }, .. } => self.worker_read = Some(read),
                other @ (io::Request::Listen { .. }
                | io::Request::Connect { .. }
                | io::Request::Bind { .. }
                | io::Request::Reject { .. }
                | io::Request::Stream { .. }
                | io::Request::Output { .. }
                | io::Request::Spawn { .. }
                | io::Request::Signal { .. }
                | io::Request::Usage { .. }
                | io::Request::Window { .. }
                | io::Request::Close { .. }
                | io::Request::Abort { .. }) => panic!("link request: {other:?}"),
            }
        }
        while let Some(request) = self.out.pop() {
            match request {
                engine_link::Request::Lower { lower: Lower::Write(stream::OutputDown::Room { right, .. }), .. } => self
                    .lower(LowerEvent::Write(stream::OutputUp::Settled {
                        right,
                        outcome: stream::OutputOutcome::Granted,
                    })),
                engine_link::Request::Lower { lower: Lower::Write(stream::OutputDown::Send { bytes, .. }), .. } => {
                    self.worker_bytes.append(&bytes).unwrap();
                }
                engine_link::Request::Lower { lower: Lower::Read(stream::Down::Demand { read, .. }), .. } => {
                    self.engine_read = Some(read);
                }
                engine_link::Request::Take { .. } => {}
                request @ (engine_link::Request::Hello { .. } | engine_link::Request::Up { .. }) => {
                    if let engine_link::Request::Hello { .. } = &request {
                        self.records.push(LinkRecord::Hello).expect("bounded link records");
                    } else if let engine_link::Request::Up { up: hub::LinkUp::Turn { turn, .. }, .. } = &request {
                        self.records.push(LinkRecord::Turn(turn.turn)).expect("bounded link records");
                    } else if let engine_link::Request::Up { up: hub::LinkUp::Answer { answer, .. }, .. } = &request {
                        self.records.push(LinkRecord::Answer(answer.turns)).expect("bounded link records");
                    }
                    return Some(request);
                }
                other @ (engine_link::Request::TurnAway { .. }
                | engine_link::Request::Lower { .. }
                | engine_link::Request::Close { .. }
                | engine_link::Request::Abort { .. }
                | engine_link::Request::Lost { .. }
                | engine_link::Request::Closed) => panic!("engine request: {other:?}"),
            }
        }
        if let Some(bytes) = demanded_bytes(&mut self.engine_read, &mut self.engine_bytes) {
            self.lower(LowerEvent::Read(stream::Up::Bytes(bytes)));
        }
        if let Some(bytes) = demanded_bytes(&mut self.worker_read, &mut self.worker_bytes) {
            worker.up(fixtures::NOW, io::Event::Stream { owner: self.owner, up: stream::Up::Bytes(bytes) });
        }
        None
    }
    pub(crate) fn lower(&mut self, event: LowerEvent) {
        self.component.down(
            fixtures::NOW,
            engine_link::Event::Lower { connection: Token::new(1), event },
            &mut self.out,
        );
    }
}

// Both concrete peers use the same stream contract: a delivery spends a real
// demand, and everything else stays in the intake below the component.
pub(crate) fn demanded_bytes(read: &mut Option<stream::Read>, intake: &mut Intake) -> Option<Box<[u8]>> {
    let bytes = intake.meet((*read)?)?;
    *read = None;
    Some(bytes)
}

pub(crate) const AGENT_OWNER: Token = Token::new(23);
pub(crate) const AGENT_CHILD: Token = Token::new(101);
pub(crate) const AGENT_INPUT: Token = Token::new(102);
pub(crate) const AGENT_OUTPUT: Token = Token::new(103);
pub(crate) const AGENT_ERROR: Token = Token::new(104);
pub(crate) const ADMITTED: u16 = 0x0106;
pub(crate) const TURN: u16 = 0x0109;
pub(crate) const ANSWER: u16 = 0x0110;
pub(crate) const LIVE: u16 = 0x0112;
pub(crate) const FRAME_HEADER_BYTES: u32 = 8; // skein's channel frame header.
pub(crate) const PEER_FRAMES: u32 = 6; // Accept, Terms, Admitted, Live, Turn, Answer.
const PEER_EVENTS: u32 = 2 + PEER_FRAMES; // Opened and Start, then the six scripted records.
pub(crate) const PEER_STEPS: u16 = 1000; // The existing LinkPeer's finite settling bound.

// Private agent peer for the Worker's Process IO, without a root assignment.
// The ledger measures actual frame writes and demanded byte deliveries; it
// neither supplies process events nor stands in for Worker::process_read.
pub(crate) struct AgentPeer {
    pub(crate) owner: Token,
    pub(crate) now: Time,
    pub(crate) component: smith_protocol_channel::Component,
    pub(crate) events: Queue<smith_protocol_channel::OpenEvent>,
    pub(crate) out: Queue<Lower>,
    pub(crate) agent_bytes: Intake,
    pub(crate) worker_bytes: Intake,
    pub(crate) agent_read: Option<stream::Read>,
    pub(crate) worker_read: Option<stream::Read>,
    pub(crate) writes: List<(u16, u64)>,
    pub(crate) written: u64,
    pub(crate) consumed: u64,
    pub(crate) stdout_demands: u64,
    pub(crate) finished: bool,
    pub(crate) ends: u32,
    pub(crate) input_finishes: u32,
    pub(crate) output_closes: u32,
}
impl AgentPeer {
    pub(crate) fn start(worker: &mut crate::Worker) -> Self {
        Self::launch(worker, &fixtures::worker_agent(), AGENT_OWNER, false)
    }
    pub(crate) fn assigned(worker: &mut crate::Worker, agent: &crate::settings::Agent) -> Self {
        let owner = assigned_agent(worker, agent);
        Self::launch(worker, agent, owner, true)
    }
    pub(crate) fn new(agent: &crate::settings::Agent, owner: Token) -> Self {
        let host = agent.limits;
        // The host's output bound prices initiator frames. The new responder
        // needs its own directional frame bound; Worker keeps its existing one.
        let mut channel = host.channel;
        for version in &smith_protocol_channel::schema_for_start(&host.bodies, host.start_bytes).unwrap().versions {
            for kind in &version.kinds {
                if kind.direction == skein_channel::Direction::FromResponder {
                    channel.output_bytes =
                        channel.output_bytes.max(kind.largest.checked_add(FRAME_HEADER_BYTES).unwrap());
                }
            }
        }
        let limits = smith_protocol_channel::Limits {
            bodies: host.bodies,
            start_bytes: host.start_bytes,
            charter: host.charter,
            transcript: host.transcript,
            channel,
            endpoints: host.endpoints,
            calls: host.calls,
            resume_turns: agent.host_limits.turns,
            search_hits: host.transcript.found_hits,
            turns: agent.host_limits.turns,
            fact_reserve_frames: 1,
            fact_reserve_bytes: 128,
            grants: agent.host_limits.accounts,
        };
        Self {
            owner,
            now: fixtures::NOW,
            component: smith_protocol_channel::Component::new(
                &limits,
                skein_channel::StreamMode::Two,
                agent.endpoints.clone(),
            )
            .unwrap(),
            events: Queue::with_capacity(smith_protocol_channel::max_out(&limits).to_domain.max(PEER_EVENTS)),
            out: Queue::with_capacity(smith_protocol_channel::max_out(&limits).below),
            agent_bytes: Intake::with_capacity(host.channel.output_bytes),
            worker_bytes: Intake::with_capacity(channel.output_bytes),
            agent_read: None,
            worker_read: None,
            writes: List::with_capacity(PEER_FRAMES),
            written: 0,
            consumed: 0,
            stdout_demands: 0,
            finished: false,
            ends: 0,
            input_finishes: 0,
            output_closes: 0,
        }
    }
    fn launch(worker: &mut crate::Worker, agent: &crate::settings::Agent, owner: Token, rooted: bool) -> Self {
        let mut peer = Self::new(agent, owner);
        if !rooted {
            worker.route(root::Request::Process(Box::new(smith_host_domain::process::Request::Spawn {
                owner,
                workspace: None,
                deadline: Time::from_nanos(u64::MAX),
            })));
        }
        let mut spawned = None;
        for _ in 0..worker.io().len() {
            let request = worker.io().pop().expect("queued IO");
            if let io::Request::Spawn { owner, .. } = request {
                spawned = Some(owner);
                break;
            }
            worker.io().push(request);
        }
        let owner = spawned.expect("process spawn IO");
        worker.up(
            fixtures::NOW,
            io::Event::Spawned { owner, child: AGENT_CHILD, pipes: Box::new([AGENT_INPUT, AGENT_OUTPUT, AGENT_ERROR]) },
        );
        peer.settle(worker);
        let opened = peer.events.pop();
        let Some(smith_protocol_channel::OpenEvent::Opened { .. }) = opened else {
            panic!("agent opening: {opened:?}");
        };
        if !rooted {
            let mut start = decoded_chat_start(agent);
            start.live = Some(smith_host_domain::channel::LiveAsk { text: true, reasoning: false, input: false });
            Self::send(
                worker,
                smith_host_domain::Down::Start {
                    start: *start,
                    window: smith_host_domain::Window {
                        turns: agent.host_limits.turns,
                        bytes: agent.host_limits.unacknowledged_bytes,
                    },
                },
            );
        }
        peer.settle(worker);
        let Some(smith_protocol_channel::OpenEvent::Start { .. }) = peer.events.pop() else {
            panic!("agent receives Start");
        };
        peer.component.send_admitted(peer.owner, &mut peer.events, &mut peer.out).unwrap();
        peer.settle(worker);
        peer
    }
    pub(crate) fn send(worker: &mut crate::Worker, message: smith_host_domain::Down) {
        worker.route(root::Request::Process(Box::new(smith_host_domain::process::Request::Send {
            owner: AGENT_OWNER,
            process: AGENT_CHILD,
            message,
        })));
    }
    pub(crate) fn read(&mut self, worker: &mut crate::Worker) {
        worker.route(root::Request::Process(Box::new(smith_host_domain::process::Request::Read {
            owner: AGENT_OWNER,
            process: AGENT_CHILD,
        })));
        self.settle(worker);
    }
    pub(crate) fn lower(&mut self, event: LowerEvent) {
        self.component.from_below(event, &mut self.events, &mut self.out);
    }
    pub(crate) fn consumed_frames(&self) -> Box<[u16]> {
        let mut kinds = List::with_capacity(PEER_FRAMES);
        for &(kind, end) in &self.writes {
            if kind >= ADMITTED && end <= self.consumed {
                kinds.push(kind).unwrap();
            }
        }
        kinds.into_boxed()
    }
    pub(crate) fn worker_request(&mut self, worker: &mut crate::Worker, request: io::Request) {
        match request {
            io::Request::Output { stream, down: stream::OutputDown::Room { right, .. } } => {
                assert_eq!(stream, AGENT_INPUT);
                worker.up(
                    self.now,
                    io::Event::Output {
                        owner: AGENT_INPUT,
                        up: stream::OutputUp::Settled { right, outcome: stream::OutputOutcome::Granted },
                    },
                );
            }
            io::Request::Output { stream, down: stream::OutputDown::Send { bytes, .. } } => {
                assert_eq!(stream, AGENT_INPUT);
                self.agent_bytes.append(&bytes).unwrap();
            }
            io::Request::Stream { stream, down: stream::Down::Demand { read, room: 0 } } => {
                if stream == AGENT_OUTPUT {
                    if read == stream::Read::Nothing {
                        self.worker_read = None;
                    } else {
                        assert!(self.worker_read.is_none(), "one outstanding stdout demand");
                        self.stdout_demands = self.stdout_demands.checked_add(1).unwrap();
                        self.worker_read = Some(read);
                    }
                } else {
                    assert_eq!(stream, AGENT_ERROR, "only stderr is independent of the channel");
                }
            }
            io::Request::Stream { stream, down: stream::Down::Finish } => {
                assert_eq!(stream, AGENT_INPUT);
                self.input_finishes = self.input_finishes.checked_add(1).unwrap();
            }
            io::Request::Close { entity } => {
                assert_eq!(entity, AGENT_OUTPUT);
                self.output_closes = self.output_closes.checked_add(1).unwrap();
            }
            other @ (io::Request::Listen { .. }
            | io::Request::Connect { .. }
            | io::Request::Bind { .. }
            | io::Request::Reject { .. }
            | io::Request::Stream { .. }
            | io::Request::Output { .. }
            | io::Request::Spawn { .. }
            | io::Request::Signal { .. }
            | io::Request::Usage { .. }
            | io::Request::Window { .. }
            | io::Request::Abort { .. }) => panic!("agent IO: {other:?}"),
        }
    }
    pub(crate) fn settle(&mut self, worker: &mut crate::Worker) {
        self.pump(worker, None);
    }
    pub(crate) fn stage_frame(&mut self, worker: &mut crate::Worker, frames: usize) {
        self.pump(worker, Some(frames));
    }
    fn pump(&mut self, worker: &mut crate::Worker, frames: Option<usize>) {
        for _ in 0..PEER_STEPS {
            worker.iterate(self.now, Wall::EPOCH);
            self.component.fire(&mut self.events, &mut self.out);
            let mut progressed = false;
            for _ in 0..worker.io().len() {
                let request = worker.io().pop().expect("queued IO");
                if !stream_request(&request, &[AGENT_INPUT, AGENT_OUTPUT, AGENT_ERROR]) {
                    worker.io().push(request);
                    continue;
                }
                progressed = true;
                self.worker_request(worker, request);
            }
            while let Some(request) = self.out.pop() {
                progressed = true;
                match request {
                    Lower::Write(stream::OutputDown::Room { right, .. }) => {
                        self.lower(LowerEvent::Write(stream::OutputUp::Settled {
                            right,
                            outcome: stream::OutputOutcome::Granted,
                        }));
                    }
                    Lower::Write(stream::OutputDown::Send { bytes, .. }) => {
                        let header = skein_channel::parse_header(
                            bytes.get(..usize::try_from(FRAME_HEADER_BYTES).unwrap()).unwrap(),
                        )
                        .unwrap();
                        assert_eq!(
                            u32::try_from(bytes.len()).unwrap(),
                            FRAME_HEADER_BYTES.checked_add(header.body_len).unwrap(),
                            "one whole frame per channel write"
                        );
                        self.written = self.written.checked_add(u64::try_from(bytes.len()).unwrap()).unwrap();
                        self.writes.push((header.kind, self.written)).unwrap();
                        self.worker_bytes.append(&bytes).unwrap();
                    }
                    Lower::Read(stream::Down::Demand { read, room: 0 }) => self.agent_read = Some(read),
                    Lower::FinishWrite => self.finished = true,
                    other @ (Lower::Read(_)
                    | Lower::Write(stream::OutputDown::Cancel { .. } | stream::OutputDown::Release { .. })) => {
                        panic!("agent lower: {other:?}")
                    }
                }
            }
            if let Some(bytes) = demanded_bytes(&mut self.agent_read, &mut self.agent_bytes) {
                progressed = true;
                self.lower(LowerEvent::Read(stream::Up::Bytes(bytes)));
            }
            if let Some(bytes) = demanded_bytes(&mut self.worker_read, &mut self.worker_bytes) {
                progressed = true;
                self.consumed = self.consumed.checked_add(u64::try_from(bytes.len()).unwrap()).unwrap();
                worker.up(self.now, io::Event::Stream { owner: AGENT_OUTPUT, up: stream::Up::Bytes(bytes) });
            } else if self.worker_read.is_some() && self.worker_bytes.is_empty() && self.finished && self.ends == 0 {
                progressed = true;
                self.ends = self.ends.checked_add(1).unwrap();
                worker.up(self.now, io::Event::Stream { owner: AGENT_OUTPUT, up: stream::Up::End });
            }
            if let Some(count) = frames
                && self.consumed_frames().len() >= count
            {
                return;
            }
            if !progressed {
                return;
            }
        }
        panic!("agent channel settles within the existing peer bound");
    }
}

pub(crate) fn decoded_chat_start(agent: &crate::settings::Agent) -> Box<smith_host_domain::Start> {
    let link::Request::Down { down: hub::LinkDown::Assign { assignment }, .. } = fixtures::chat_assignment() else {
        panic!("chat assignment")
    };
    let source = assignment.assignment;
    crate::worker_route::decode(
        source.run,
        hub::Start {
            live: hub::LiveAsk::NONE,
            workspace: Some(Token::new(3)),
            charter: source.charter,
            activation: 1,
            identity: source.identity,
            turns: Box::new([]),
            answered: Box::new([]),
            messages: Box::new([]),
            grants: Box::new([]),
        },
        agent,
    )
    .unwrap()
}

pub(crate) fn cancellation_turn(agent: &crate::settings::Agent) -> Box<[u8]> {
    turn_body(agent, 1)
}
pub(crate) fn turn_body(agent: &crate::settings::Agent, number: u32) -> Box<[u8]> {
    use smith_transcript::v6 as transcript;
    let limits = &agent.limits.transcript;
    let mut blocks = List::with_capacity(1);
    blocks
        .push(transcript::Block::Text(
            transcript::Text::new(limits, transcript::TextParts { text: b"last text".as_slice().into(), replay: None })
                .unwrap(),
        ))
        .unwrap();
    let mut messages = List::with_capacity(1);
    messages
        .push(
            transcript::Message::new(limits, transcript::MessageParts { role: transcript::Role::Assistant, blocks })
                .unwrap(),
        )
        .unwrap();
    let turn = transcript::Turn::new(
        limits,
        transcript::TurnParts {
            kind: transcript::TurnKind::Ordinary,
            opening: if number == 1 {
                Some(
                    transcript::Opening::new(
                        limits,
                        transcript::OpeningParts {
                            affinity: transcript::Affinity::new(
                                limits,
                                transcript::AffinityParts { key: [0; 16], thread: 0 },
                            )
                            .unwrap(),
                            account: None,
                            backend: transcript::Backend::Direct,
                        },
                    )
                    .unwrap(),
                )
            } else {
                None
            },
            endpoint: b"fake".as_slice().into(),
            model: b"small".as_slice().into(),
            effort: None,
            dialect: fixtures::DIALECT,
            place: number,
            usage: transcript::Usage::new(
                limits,
                transcript::UsageParts {
                    input: None,
                    output: None,
                    cache_read: None,
                    cache_write: None,
                    reasoning: None,
                },
            )
            .unwrap(),
            spent: 0,
            jobs: 0,
            messages,
        },
    )
    .unwrap();
    let mut writer = skein_lib::Writer::new(usize::try_from(turn.measure()).unwrap());
    turn.encode(&mut writer).unwrap();
    writer.finish()
}

// Each peer leaves the other peer's IO queued. This lets the same concrete
// byte drivers share one Worker without granting the other's output credit.
fn stream_request(request: &io::Request, streams: &[Token]) -> bool {
    if let io::Request::Output { stream, .. } | io::Request::Stream { stream, .. } = request {
        streams.contains(stream)
    } else if let io::Request::Close { entity } = request {
        streams.contains(entity)
    } else {
        false
    }
}

// Only fixture setup drives the root directly: capture smith's opaque process
// owner as Spawn crosses, then hand every request to the real service adapter.
fn assigned_agent(worker: &mut crate::Worker, agent: &crate::settings::Agent) -> Token {
    const OUTPUTS: u32 = 256; // This chat start has fewer than a dozen requests.
    let link::Request::Down { mut down, .. } = fixtures::chat_assignment() else { panic!("chat assignment") };
    if let hub::LinkDown::Assign { assignment } = &mut down {
        assignment.assignment.live = hub::LiveAsk { text: true, reasoning: false, input: false };
    }
    let mut out = Queue::with_capacity(OUTPUTS);
    root::step(&mut worker.root, &worker.env, root::Event::Link(down), &mut out);
    let mut spawned = None;
    while let Some(request) = out.pop() {
        if let root::Request::Decode { agent: owner, run, start } = request {
            let start = crate::worker_route::decode(run, *start, agent);
            root::step(&mut worker.root, &worker.env, root::Event::Decoded { agent: owner, start }, &mut out);
        } else {
            if let root::Request::Process(ref process) = request
                && let smith_host_domain::process::Request::Spawn { owner, .. } = process.as_ref()
            {
                spawned = Some(*owner);
            }
            worker.route(request);
        }
    }
    spawned.expect("the assigned chat spawns smith")
}

pub(crate) fn socket_stand() -> fixtures::WorkerStand {
    let mut settings = fixtures::worker_settings();
    settings.jig.engine = jig_protocol_config::Address::Addr(io::Addr::Ip(([127, 0, 0, 1], 7100).into()));
    socket_stand_with(&settings, &fixtures::worker_agent())
}

pub(crate) fn socket_stand_with(
    settings: &crate::settings::Worker,
    agent: &crate::settings::Agent,
) -> fixtures::WorkerStand {
    let mut stand = fixtures::WorkerStand::new();
    stand.worker = crate::Worker::new(
        settings,
        agent,
        &fixtures::worker_limits(),
        &fixtures::worker_files(),
        fixtures::worker_runtime(),
        7,
    )
    .unwrap();
    for _ in 0_u16..1000 {
        if !stand.turn() {
            return stand;
        }
    }
    panic!("socket worker starts");
}
