use super::fixtures;
use crate::worker_settings;

#[test]
fn the_link_s_credential_names_temper_s_payload_version() {
    let settings = fixtures::worker_settings();
    let files = fixtures::worker_files();
    let secret = files.contents(&settings.jig.secret).unwrap();
    let credential = worker_settings::credential(&settings, secret);
    assert_eq!(credential.payload, temper_protocol_worker_wire::PAYLOAD_VERSION);
    assert_eq!(credential.secret.expose(), b"worker-secret-value");
    assert_eq!(credential.name.as_ref(), b"worker-a");
}

#[test]
fn the_worker_sizes_each_part_from_its_payload_codec() {
    let settings = fixtures::worker_settings();
    let agent = fixtures::worker_agent();
    let limits = fixtures::worker_limits();
    let prepared = worker_settings::prepare(&settings, &agent, &limits).unwrap();
    assert_eq!(prepared.root.host.slots, 1);
    assert_eq!(prepared.root.checkout.repositories, 1);
    assert_eq!(prepared.prefix, (3_u64 << 60_u32) + (1_u64 << 48_u32));
    assert_eq!(u64::from(prepared.link.sizes.items), v1::Items::worst_case_bytes(&prepared.payload).unwrap());
    assert_eq!(prepared.payload.repository_git_some, 4096);
    assert!(worker_settings::worker_worst_case(&settings, &agent, &limits).unwrap() > 0);
}

#[test]
fn a_workspace_start_carries_one_path_per_directory() {
    use smith_host_domain as smith;
    let agent = fixtures::worker_agent();
    let mut start = decoded_chat_start(&agent);
    let mut directories = skein_lib::List::with_capacity(2);
    for (token, name, writable) in [(73, b"second".as_slice(), true), (41, b"first".as_slice(), false)] {
        directories
            .push(smith::Directory {
                root: Token::new(token),
                name: name.into(),
                host: Box::new([]),
                writable,
                git: true,
                conflicts: Box::new([]),
            })
            .unwrap();
    }
    start.directories = directories.into_boxed();
    let table = jig_protocol_accounts::table::Table::new(0, 0);
    let values = crate::worker::start_values(&table, &start, b"/worker", fixtures::NOW).unwrap();
    assert_eq!(
        values.paths.as_ref(),
        [Box::from(b"/worker/workspaces/73/second".as_slice()), Box::from(b"/worker/workspaces/41/first".as_slice()),]
    );
    encode_grants(*start, values, &agent).expect("two prepared directories encode with their paths");
}

fn decoded_chat_start(agent: &crate::settings::Agent) -> Box<smith_host_domain::Start> {
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

use jig_domain_host as hub;
use jig_protocol_engine_workers as engine_link;
use jig_protocol_worker_link as link;
use jig_protocol_worker_wire::Part;
use skein_channel::{Lower, LowerEvent};
use skein_io as io;
use skein_lib::{Duration, Intake, Queue, Reader, Secret, Time, Token, Wall, stream};
use temper_protocol_worker_wire::v1;
use temper_worker_domain::{self as root, wire};

#[test]
fn an_assignment_with_items_reaches_the_root_decoded_with_its_save() {
    let mut worker = fixtures::worker();
    worker.link(fixtures::NOW, fixtures::repository_assignment());
    let root::Event::Assign { assignment, items, save } = worker.events.pop().unwrap() else {
        panic!("assignment with items");
    };
    assert_eq!(items.unwrap(), fixtures::one_repository());
    assert_eq!(save.as_deref(), Some(b"temper/saved-7".as_slice()));
    assert!(assignment.assignment.save);
    assert_eq!(assignment.assignment.workspace.unwrap().workstream, 7);
}

#[test]
fn undecodable_items_refuse_the_start_as_invalid_naming_them() {
    let mut peer = LinkPeer::new();
    let mut worker = fixtures::started_worker().worker;
    peer.open(&mut worker);
    let link::Request::Down { down, .. } = fixtures::repository_assignment() else { panic!("assignment") };
    worker.link(
        fixtures::NOW,
        link::Request::Down { down, parts: Box::new([Part { token: Token::new(0), bytes: Box::new([255]) }]) },
    );
    let (up, parts) = peer.answer(&mut worker);
    let hub::LinkUp::Answer { answer, .. } = up else { panic!("refused answer") };
    assert_eq!(
        answer.ending,
        hub::Ending::Refused(hub::Refusal::Invalid(hub::Invalid::Start {
            cause: hub::StartCause::Other,
            why: b"workspace items".as_slice().into()
        }))
    );
    assert!(parts.is_empty());
    assert_eq!(worker.root.host().hosted(), 0);
}

#[test]
fn an_answer_goes_up_with_its_left_and_saved_parts() {
    let mut peer = LinkPeer::new();
    let mut worker = fixtures::started_worker().worker;
    peer.open(&mut worker);
    let hub::LinkUp::Answer { run, attempt, answer } = fixtures::answer(hub::Ending::Parked {
        work: hub::Work { left: Some(Token::new(11)), saved: Some(Token::new(12)) },
    }) else {
        panic!("answer")
    };
    worker.route(root::Request::Answer {
        run,
        attempt,
        answer,
        preparation: None,
        work: wire::Work {
            landed: Box::new([wire::Landed { tag: 3, commit: [7; 32] }]),
            saved: Some(Box::new([wire::SavedRepository { tag: 3, from_saved: true, landing: wire::Landing::Moved }])),
        },
    });
    let (up, parts) = peer.answer(&mut worker);
    let hub::LinkUp::Answer { run, attempt, answer } = up else { panic!("answer crossed") };
    assert_eq!((run, attempt), (Token::new(7), Token::new(2)));
    let hub::Ending::Parked { work } = answer.ending else { panic!("parked") };
    assert_eq!(parts.len(), 2);
    assert_eq!(work.left, Some(parts[0].token));
    assert_eq!(work.saved, Some(parts[1].token));
    let left = v1::Left::decode(&fixtures::wire_limits(), &mut Reader::new(&parts[0].bytes)).unwrap();
    let saved = v1::Saved::decode(&fixtures::wire_limits(), &mut Reader::new(&parts[1].bytes)).unwrap();
    assert_eq!((left.landed().get(0).unwrap().tag(), left.landed().get(0).unwrap().commit()), (3, &[7; 32]));
    let row = saved.repositories().get(0).unwrap();
    assert_eq!((row.tag(), row.from_saved(), row.landing()), (3, true, &v1::Landing::Moved));
}

#[test]
fn a_git_identity_s_value_lands_in_the_identities_table_only() {
    let mut worker = fixtures::worker();
    worker.link(fixtures::NOW, fixtures::application_value(1, 2, b"tok-2"));
    worker.link(fixtures::NOW, fixtures::application_value(1, 1, b"tok-1"));
    assert_eq!(
        worker.identities.newest(1, fixtures::NOW),
        Some(&crate::identities::Value::Forgejo(fixtures::identity(b"tok-2")))
    );
    assert_eq!(worker.values.value(1, 2, fixtures::NOW), None);
    assert!(worker.events.is_empty(), "values stay below the root");
    worker.link(
        fixtures::NOW,
        link::Request::Value {
            account: 1,
            generation: 3,
            expires: Time::from_nanos(u64::MAX),
            value: link::GrantValue::Application { secret: Secret::new(Box::new([255])) },
        },
    );
    assert_eq!(worker.undecodable_values, 1);
    assert_eq!(worker.identities.newest(1, fixtures::NOW), None);
}

#[test]
fn the_agent_runs_as_temper_agent_with_no_credential_in_its_environment() {
    let mut worker = fixtures::started_worker().worker;
    worker.link(fixtures::NOW, fixtures::chat_assignment());
    let (_, launched) = fixtures::spawn(&mut worker);
    assert_eq!(launched.program.as_ref(), fixtures::TEMPER);
    assert_eq!(
        launched.args.as_ref(),
        [Box::from(&b"internal"[..]), Box::from(&b"agent"[..]), Box::from(fixtures::AGENT_JSON)]
    );
    assert_eq!(launched.root, io::kernel::Fd::new(9));
    assert!(launched.env.is_empty(), "no credential reaches an agent");
}

#[test]
fn a_worker_s_secret_is_read_and_checked_at_its_path() {
    use jig_protocol_config::{Path, Problem, Segment, What};
    let empty = crate::settings::Files { entries: skein_lib::List::with_capacity(0) };
    for (row, files, secret_bound, expected) in
        [("missing", empty, 64, What::Unreadable), ("too long", fixtures::worker_files(), 1, What::Bound)]
    {
        let mut settings = fixtures::worker_settings();
        settings.jig.limits.link.secret = secret_bound;
        let problem = crate::Worker::new(
            &settings,
            &fixtures::worker_agent(),
            &fixtures::worker_limits(),
            &files,
            crate::WorkerRuntime { root: io::kernel::Fd::new(9), path: b"/worker".as_slice().into() },
            7,
        )
        .unwrap_err();
        assert_eq!(
            problem.problems.get(0),
            Some(&Problem {
                path: Path::root().member(b"workers").child(Segment::Index(0)).member(b"secret"),
                what: expected,
            }),
            "{row}"
        );
    }
}

struct LinkPeer {
    component: engine_link::Component,
    out: Queue<engine_link::Request>,
    engine_bytes: Intake,
    worker_bytes: Intake,
    engine_read: Option<stream::Read>,
    worker_read: Option<stream::Read>,
    owner: Token,
}
impl LinkPeer {
    fn new() -> Self {
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
    fn open(&mut self, worker: &mut crate::Worker) {
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
    fn answer(&mut self, worker: &mut crate::Worker) -> (hub::LinkUp, Box<[Part]>) {
        for _ in 0_u16..1000 {
            if let Some(engine_link::Request::Up { up: up @ hub::LinkUp::Answer { .. }, parts, .. }) = self.turn(worker)
            {
                return (up, parts);
            }
        }
        panic!("worker sends its answer");
    }
    fn turn(&mut self, worker: &mut crate::Worker) -> Option<engine_link::Request> {
        worker.iterate(fixtures::NOW, Wall::EPOCH);
        while let Some(request) = worker.io().pop() {
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
    fn lower(&mut self, event: LowerEvent) {
        self.component.down(
            fixtures::NOW,
            engine_link::Event::Lower { connection: Token::new(1), event },
            &mut self.out,
        );
    }
}

// Both concrete peers use the same stream contract: a delivery spends a real
// demand, and everything else stays in the intake below the component.
fn demanded_bytes(read: &mut Option<stream::Read>, intake: &mut Intake) -> Option<Box<[u8]>> {
    let bytes = intake.meet((*read)?)?;
    *read = None;
    Some(bytes)
}

const AGENT_OWNER: Token = Token::new(23);
const AGENT_CHILD: Token = Token::new(101);
const AGENT_INPUT: Token = Token::new(102);
const AGENT_OUTPUT: Token = Token::new(103);
const AGENT_ERROR: Token = Token::new(104);
const ADMITTED: u16 = 0x0106;
const TURN: u16 = 0x0109;
const ANSWER: u16 = 0x0110;
const LIVE: u16 = 0x0112;
const FRAME_HEADER_BYTES: u32 = 8; // skein's channel frame header.
const PEER_FRAMES: u32 = 6; // Accept, Terms, Admitted, Live, Turn, Answer.
const PEER_STEPS: u16 = 1000; // The existing LinkPeer's finite settling bound.

// Private agent peer for the Worker's Process IO, without a root assignment.
// The ledger measures actual frame writes and demanded byte deliveries; it
// neither supplies process events nor stands in for Worker::process_read.
struct AgentPeer {
    component: smith_protocol_channel::Component,
    events: Queue<smith_protocol_channel::OpenEvent>,
    out: Queue<Lower>,
    agent_bytes: Intake,
    worker_bytes: Intake,
    agent_read: Option<stream::Read>,
    worker_read: Option<stream::Read>,
    writes: skein_lib::List<(u16, u64)>,
    written: u64,
    consumed: u64,
    stdout_demands: u64,
    finished: bool,
    ends: u32,
    input_finishes: u32,
    output_closes: u32,
}
impl AgentPeer {
    fn start(worker: &mut crate::Worker) -> Self {
        let agent = fixtures::worker_agent();
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
        let mut peer = Self {
            component: smith_protocol_channel::Component::new(
                &limits,
                skein_channel::StreamMode::Two,
                agent.endpoints.clone(),
            )
            .unwrap(),
            events: Queue::with_capacity(smith_protocol_channel::max_out(&limits).to_domain),
            out: Queue::with_capacity(smith_protocol_channel::max_out(&limits).below),
            agent_bytes: Intake::with_capacity(host.channel.output_bytes),
            worker_bytes: Intake::with_capacity(channel.output_bytes),
            agent_read: None,
            worker_read: None,
            writes: skein_lib::List::with_capacity(PEER_FRAMES),
            written: 0,
            consumed: 0,
            stdout_demands: 0,
            finished: false,
            ends: 0,
            input_finishes: 0,
            output_closes: 0,
        };
        worker.route(root::Request::Process(Box::new(smith_host_domain::process::Request::Spawn {
            owner: AGENT_OWNER,
            workspace: None,
            deadline: Time::from_nanos(u64::MAX),
        })));
        let io::Request::Spawn { owner, .. } = worker.io().pop().expect("process spawn IO") else {
            panic!("agent spawn");
        };
        worker.up(
            fixtures::NOW,
            io::Event::Spawned { owner, child: AGENT_CHILD, pipes: Box::new([AGENT_INPUT, AGENT_OUTPUT, AGENT_ERROR]) },
        );
        peer.settle(worker);
        let opened = peer.events.pop();
        let Some(smith_protocol_channel::OpenEvent::Opened { .. }) = opened else {
            panic!("agent opening: {opened:?}");
        };
        let mut start = decoded_chat_start(&agent);
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
        peer.settle(worker);
        let Some(smith_protocol_channel::OpenEvent::Start { .. }) = peer.events.pop() else {
            panic!("agent receives Start");
        };
        peer.component.send_admitted(AGENT_OWNER, &mut peer.events, &mut peer.out).unwrap();
        peer.settle(worker);
        peer
    }
    fn send(worker: &mut crate::Worker, message: smith_host_domain::Down) {
        worker.route(root::Request::Process(Box::new(smith_host_domain::process::Request::Send {
            owner: AGENT_OWNER,
            process: AGENT_CHILD,
            message,
        })));
    }
    fn read(&mut self, worker: &mut crate::Worker) {
        worker.route(root::Request::Process(Box::new(smith_host_domain::process::Request::Read {
            owner: AGENT_OWNER,
            process: AGENT_CHILD,
        })));
        self.settle(worker);
    }
    fn lower(&mut self, event: LowerEvent) {
        self.component.from_below(event, &mut self.events, &mut self.out);
    }
    fn consumed_frames(&self) -> Box<[u16]> {
        let mut kinds = skein_lib::List::with_capacity(PEER_FRAMES);
        for &(kind, end) in &self.writes {
            if kind >= ADMITTED && end <= self.consumed {
                kinds.push(kind).unwrap();
            }
        }
        kinds.into_boxed()
    }
    fn worker_request(&mut self, worker: &mut crate::Worker, request: io::Request) {
        match request {
            io::Request::Output { stream, down: stream::OutputDown::Room { right, .. } } => {
                assert_eq!(stream, AGENT_INPUT);
                worker.up(
                    fixtures::NOW,
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
    fn settle(&mut self, worker: &mut crate::Worker) {
        for _ in 0..PEER_STEPS {
            worker.iterate(fixtures::NOW, Wall::EPOCH);
            self.component.fire(&mut self.events, &mut self.out);
            let mut progressed = false;
            while let Some(request) = worker.io().pop() {
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
                worker.up(fixtures::NOW, io::Event::Stream { owner: AGENT_OUTPUT, up: stream::Up::Bytes(bytes) });
            } else if self.worker_read.is_some() && self.worker_bytes.is_empty() && self.finished && self.ends == 0 {
                progressed = true;
                self.ends = self.ends.checked_add(1).unwrap();
                worker.up(fixtures::NOW, io::Event::Stream { owner: AGENT_OUTPUT, up: stream::Up::End });
            }
            if !progressed {
                return;
            }
        }
        panic!("agent channel settles within the existing peer bound");
    }
}

#[test]
fn the_worker_demands_each_cancellation_record_and_end() {
    use smith_domain_run as run;
    let mut worker = fixtures::started_worker().worker;
    let mut peer = AgentPeer::start(&mut worker);
    let opening_reads = peer.stdout_demands;
    assert!(peer.consumed_frames().is_empty(), "admission waits below Worker for Read");
    assert!(peer.worker_read.is_none());
    peer.read(&mut worker);
    assert_eq!(peer.consumed_frames().as_ref(), [ADMITTED], "Read must consume the queued admission");
    assert!(peer.stdout_demands > opening_reads, "admission is consumed through actual stdout demands");

    // A real demand waits through a Send/Cancel and is then spent by Live.
    peer.read(&mut worker);
    assert!(peer.worker_read.is_some(), "Read leaves a real stdout demand pending");
    let waiting = (peer.consumed, peer.stdout_demands, peer.worker_read);
    AgentPeer::send(&mut worker, smith_host_domain::Down::Cancel);
    peer.settle(&mut worker);
    let Some(smith_protocol_channel::OpenEvent::Cancel) = peer.events.pop() else {
        panic!("agent receives Cancel");
    };
    assert!(peer.events.is_empty(), "one Cancel reaches the real agent peer");
    assert_eq!(
        (peer.consumed, peer.stdout_demands, peer.worker_read),
        waiting,
        "Cancel preserves the outstanding demand"
    );
    assert!(
        peer.component
            .send_live(
                smith_protocol_channel::Piece {
                    kind: smith_protocol_channel::PieceKind::Text,
                    turn: 1,
                    retry: 0,
                    offset: 0,
                    bytes: b"last text"
                },
                AGENT_OWNER,
                &mut peer.events,
                &mut peer.out,
            )
            .unwrap()
    );
    peer.settle(&mut worker);
    assert_eq!(peer.consumed_frames().as_ref(), [ADMITTED, LIVE]);
    assert!(peer.worker_read.is_none(), "Live spends exactly one demand");

    let agent = fixtures::worker_agent();
    let body = cancellation_turn(&agent);
    let turn = smith_protocol_channel::decode_turn(&body, 1, &agent.limits.transcript, &agent.endpoints).unwrap();
    peer.component
        .send_turn(1, None, run::Spend::ZERO, true, &turn, AGENT_OWNER, &mut peer.events, &mut peer.out)
        .unwrap();
    peer.component
        .send_answer(
            fixtures::NOW,
            run::Answer::Failed { failure: run::Failure::Cancelled, spent: run::Spend::ZERO, turns: 1 },
            None,
            AGENT_OWNER,
            &mut peer.events,
            &mut peer.out,
        )
        .unwrap();
    let held = (peer.consumed, peer.stdout_demands);
    peer.settle(&mut worker);
    assert_eq!(
        (peer.consumed, peer.stdout_demands),
        held,
        "queued final records cannot consume even a header without Read"
    );
    assert_eq!(peer.consumed_frames().as_ref(), [ADMITTED, LIVE]);
    assert!(!peer.worker_bytes.is_empty(), "final Turn and Answer are held below Worker");
    let mut written = peer.writes.iter().rev();
    assert_eq!(written.next().unwrap().0, ANSWER, "both final frames reached the byte stream");
    assert_eq!(written.next().unwrap().0, TURN);

    peer.read(&mut worker);
    assert_eq!(peer.consumed_frames().as_ref(), [ADMITTED, LIVE, TURN]);
    assert!(peer.worker_read.is_none());
    assert_eq!(
        (peer.input_finishes, peer.ends, peer.output_closes),
        (0, 0, 0),
        "Turn cannot also deliver Answer or end"
    );
    let after_turn = (peer.consumed, peer.stdout_demands, peer.worker_bytes.len());
    peer.settle(&mut worker);
    assert_eq!(
        (peer.consumed, peer.stdout_demands, peer.worker_bytes.len()),
        after_turn,
        "the second frame stays untouched"
    );

    peer.read(&mut worker);
    assert_eq!(peer.consumed_frames().as_ref(), [ADMITTED, LIVE, TURN, ANSWER]);
    assert_eq!(
        (peer.input_finishes, peer.ends, peer.output_closes),
        (1, 0, 0),
        "the real Answer finishes stdin, leaving end for Read"
    );
    assert!(peer.worker_read.is_none());
    let reads_before_end = peer.stdout_demands;
    peer.read(&mut worker);
    assert_eq!((peer.ends, peer.output_closes), (1, 1), "one demanded end closes stdout");
    assert_eq!(peer.stdout_demands, reads_before_end + 1, "one actual IO demand receives EOF");
    assert_eq!(peer.consumed, peer.written, "all queued bytes were demanded");
    peer.settle(&mut worker);
    assert_eq!((peer.ends, peer.output_closes), (1, 1), "end settles only once");
}

fn cancellation_turn(agent: &crate::settings::Agent) -> Box<[u8]> {
    use smith_transcript::v6 as transcript;
    let limits = &agent.limits.transcript;
    let mut blocks = skein_lib::List::with_capacity(1);
    blocks
        .push(transcript::Block::Text(
            transcript::Text::new(limits, transcript::TextParts { text: b"last text".as_slice().into(), replay: None })
                .unwrap(),
        ))
        .unwrap();
    let mut messages = skein_lib::List::with_capacity(1);
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
            opening: Some(
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
            ),
            endpoint: b"fake".as_slice().into(),
            model: b"small".as_slice().into(),
            effort: None,
            dialect: fixtures::DIALECT,
            place: 1,
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

#[test]
fn retained_program_metadata_is_priced_exactly_once_at_capacity() {
    use crate::settings::{Adapter, PROGRAM_PATH_BYTES, Program};
    let (settings, mut agent, limits) = program_worker_settings();
    let without = worker_settings::worker_worst_case(&settings, &agent, &limits).unwrap();
    let empty_heap = worker_heap(&settings, &agent, &limits, without);
    let path: Box<[u8]> = Box::new([b'/'; PROGRAM_PATH_BYTES]);
    agent.programs = Box::new([Program { adapter: Adapter::ClaudeCode, path: path.clone(), directory: path }]);
    let with = worker_settings::worker_worst_case(&settings, &agent, &limits).unwrap();
    assert_eq!(with - without, u64::try_from(2 * PROGRAM_PATH_BYTES + size_of::<Program>()).unwrap());
    let declared_heap = worker_heap(&settings, &agent, &limits, with);
    assert_eq!(declared_heap - empty_heap, with - without, "the retained copy is priced once");
}
#[test]
fn worker_preparation_refuses_a_program_path_past_capacity() {
    use crate::settings::{Adapter, PROGRAM_PATH_BYTES, Program};
    let (settings, mut agent, limits) = program_worker_settings();
    agent.programs = Box::new([Program {
        adapter: Adapter::ClaudeCode,
        path: Box::new([b'/'; PROGRAM_PATH_BYTES + 1]),
        directory: Box::new([b'/'; PROGRAM_PATH_BYTES]),
    }]);
    worker_settings::prepare(&settings, &agent, &limits).unwrap_err();
}

fn worker_heap(
    settings: &crate::settings::Worker,
    agent: &crate::settings::Agent,
    limits: &crate::limits::Limits,
    bound: u64,
) -> u64 {
    let files = fixtures::worker_files();
    let meter = skein_world::domain::heap::Meter::new();
    meter.start();
    let worker = crate::Worker::new(settings, agent, limits, &files, fixtures::worker_runtime(), 7).unwrap();
    let measured = meter.end();
    let held = measured.held();
    meter.check(measured, bound, &"worker construction");
    drop(worker);
    held
}

#[test]
fn start_and_renewal_require_the_declared_form_and_live_generation() {
    use jig_protocol_accounts::table::{Scheme, Table, Value};
    use smith_host_domain as smith;
    let agent = fixtures::worker_agent();
    let forms = [
        smith::Form::Credential(smith::Scheme::Bearer),
        smith::Form::Credential(smith::Scheme::Key),
        smith::Form::Withdrawn,
    ];
    for (scheme, expected) in [(Scheme::Bearer, forms[0]), (Scheme::Key, forms[1])] {
        let mut table = Table::new(1, 64);
        for generation in [2, 3] {
            table
                .granted(
                    7,
                    generation,
                    Value { scheme: scheme.clone(), token: Secret::new(b"credential".as_slice().into()) },
                    Time::from_nanos(9),
                )
                .unwrap();
        }
        for (number, generation, now, live) in
            [(7, 2, 8, true), (7, 3, 8, true), (7, 1, 8, false), (7, 3, 9, false), (8, 3, 8, false)]
        {
            for form in forms {
                let grant = smith::Grant { account: number, generation, valid: Duration::from_secs(5), form };
                let now = Time::from_nanos(now);
                let mut start = decoded_chat_start(&agent);
                start.grants = Box::new([grant]);
                let opening = crate::worker::start_values(&table, &start, b"/worker", now);
                let renewal = crate::worker::grant_value(&table, grant, now);
                let accepted = live && form == expected;
                assert_eq!(opening.is_some(), accepted, "start {grant:?} at {now:?}");
                assert_eq!(renewal.is_some(), accepted, "renewal {grant:?} at {now:?}");
                if let Some(values) = opening {
                    assert_eq!(smith_host_protocol::grant_form(&values.credentials[0]).unwrap(), form);
                    encode_grants(*start, values, &agent).unwrap();
                }
            }
        }
    }
}

#[test]
fn a_program_grant_lends_only_its_validated_name_without_a_table_entry() {
    use smith_host_domain as smith;
    let agent = fixtures::worker_agent();
    let table = jig_protocol_accounts::table::Table::new(0, 0);
    for size in [0, 1, smith::SourceName::BYTES, smith::SourceName::BYTES + 1] {
        let bytes = repeated(b'a', size);
        let Some(name) = smith::SourceName::new(&bytes) else {
            assert!(size == 0 || size > smith::SourceName::BYTES);
            continue;
        };
        let grant = smith::Grant {
            account: 17,
            generation: 4,
            valid: Duration::from_secs(6),
            form: smith::Form::Program(name),
        };
        let mut start = decoded_chat_start(&agent);
        start.grants = Box::new([grant]);
        let values = crate::worker::start_values(&table, &start, b"/worker", Time::ZERO).unwrap();
        assert_eq!(values.credentials.as_ref(), [smith_protocol_channel::GrantValue::Program(bytes.clone())]);
        assert_eq!(crate::worker::grant_value(&table, grant, Time::ZERO).unwrap().credentials, values.credentials);
        let encoded = encode_grants(*start, values, &agent).unwrap();
        let body = &encoded.bytes()[encoded.bytes().len() - usize::try_from(encoded.body_len()).unwrap()..];
        let decoded = smith_channel::Start::decode(&agent.limits.bodies, &mut Reader::new(body)).unwrap();
        let value = decoded.grants().get(0).unwrap();
        assert_eq!(
            (value.account(), value.generation(), value.valid()),
            (grant.account, grant.generation, grant.valid)
        );
        let smith_channel::GrantValue::Program(value) = value.value() else { panic!("only a program name") };
        assert_eq!(value.name(), bytes.as_ref());
        let mut mismatched = decoded_chat_start(&agent);
        mismatched.grants = Box::new([grant]);
        let wrong = smith_host_protocol::Values {
            paths: Box::default(),
            credentials: Box::new([smith_protocol_channel::GrantValue::Bearer(Box::default())]),
        };
        encode_grants(*mismatched, wrong, &agent).unwrap_err();
    }
    for bytes in [b"UPPER".as_slice(), b"slash/name", b"claude_code"] {
        assert!(smith::SourceName::new(bytes).is_none());
    }
}

#[test]
fn grant_sizes_take_credentials_source_names_and_generated_identities() {
    let name = u32::try_from(smith_host_domain::SourceName::BYTES).unwrap();
    for (credential, identity, expected) in [
        (name - 1, name - 1, name),
        (name, name, name),
        (name + 1, name, name + 1),
        (name, name + 1, name + 1),
        (4096, name, 4096),
    ] {
        assert_eq!(worker_settings::grant_bytes(credential, identity), Some(expected));
    }
    let mut engine = fixtures::engine();
    let mut worker = fixtures::worker_settings();
    let payload = worker_settings::payload_limits(&fixtures::worker_limits()).unwrap();
    for credential in [1, name, 4096] {
        engine.jig.limits.token = credential;
        // Table capacity, not OAuth's separate parsing bound, sizes every credential scheme.
        engine.jig.limits.driver.client.document.token_bytes = 1;
        worker.jig.limits.token = credential;
        let left = crate::workers::limits(&engine).unwrap();
        let right = worker_settings::link_limits(&worker.jig, &payload).unwrap();
        let wire = crate::translate::limits(&engine.limits).unwrap();
        let identity = u32::try_from(v1::Identity::worst_case_bytes(&wire).unwrap()).unwrap();
        assert_eq!(left.sizes.grant, credential.max(name).max(identity));
        let root = crate::limits::engine(&engine.jig.limits, &engine.limits).unwrap();
        let effective = temper_engine_domain::core_limits(&root);
        let store = crate::store::limits(&root, &root.forge, &engine.jig.limits.store).unwrap();
        assert_eq!(left.sizes.answer, effective.call_answer_bytes);
        assert_eq!(left.sizes.call_input, effective.call_answer_bytes);
        assert_eq!(store.core.call_answer_bytes, left.sizes.answer);
        assert_eq!(right.sizes.answer, effective.call_answer_bytes);
        assert_eq!(
            right.sizes.grant,
            worker_settings::credential_bytes(credential)
                .max(name)
                .max(temper_protocol_worker_wire::largest(&payload).unwrap().grant)
        );
    }
    engine.jig.limits.hub.charter_bytes = u64::MAX;
    assert!(crate::workers::limits(&engine).is_none());
    worker.jig.limits.hub.charter_bytes = u64::MAX;
    assert!(worker_settings::link_limits(&worker.jig, &payload).is_none());
    assert!(
        worker_settings::worker_worst_case(&worker, &fixtures::worker_agent(), &fixtures::worker_limits()).is_none()
    );
}

#[test]
fn multiple_models_and_providers_preserve_the_supplied_grant_list() {
    use smith_host_domain as smith;
    let mut agent = fixtures::worker_agent();
    let mut endpoints = skein_lib::List::with_capacity(2);
    for (name, number) in [(b"fake".as_slice(), 1), (b"other", 2)] {
        endpoints
            .push(smith_protocol_channel::Endpoint { name: name.into(), number, dialect: fixtures::DIALECT })
            .unwrap();
    }
    agent.endpoints = smith_protocol_channel::Endpoints::new(endpoints);
    let mut charter = decoded_chat_start(&agent).charter.into_value();
    let mut main = charter.llm.clone();
    main.account = Some(7);
    let mut alternative = main.clone();
    alternative.model = b"alternative".as_slice().into();
    let mut other = main.clone();
    other.account = Some(19);
    other.endpoint.0 = 2;
    charter.llm = main;
    charter.models = Box::new([alternative, other]);
    let charter = smith_protocol_channel::encode_charter(&charter, &agent.limits.charter, &agent.endpoints).unwrap();
    let grants = [
        hub::Grant {
            account: 19,
            generation: 8,
            valid: Duration::from_secs(11),
            form: hub::Form::Credential(hub::Scheme::Key),
        },
        hub::Grant {
            account: 7,
            generation: 3,
            valid: Duration::from_secs(5),
            form: hub::Form::Credential(hub::Scheme::Bearer),
        },
        hub::Grant { account: 99, generation: 1, valid: Duration::from_secs(6), form: hub::Form::Application },
    ];
    let start = crate::worker_route::decode(
        Token::new(31),
        hub::Start {
            live: hub::LiveAsk::NONE,
            workspace: None,
            charter,
            activation: 2,
            identity: [3; 16],
            turns: Box::default(),
            answered: Box::default(),
            messages: Box::default(),
            grants: grants.into(),
        },
        &agent,
    )
    .unwrap();
    assert_eq!(start.charter.value().models.len(), 2);
    assert_eq!(
        start.grants.as_ref(),
        [
            smith::Grant {
                account: 19,
                generation: 8,
                valid: Duration::from_secs(11),
                form: smith::Form::Credential(smith::Scheme::Key)
            },
            smith::Grant {
                account: 7,
                generation: 3,
                valid: Duration::from_secs(5),
                form: smith::Form::Credential(smith::Scheme::Bearer)
            },
        ]
    );
}

#[test]
fn smith_channel_notices_keep_the_source_generation_and_optional_reset() {
    use smith_host_domain as smith;
    use smith_host_protocol::{OpenEvent, ProcessEvent};
    let owner = Token::new(32);
    for (notice, expected) in [
        (
            OpenEvent::Unauthorized { account: 17, generation: 4 },
            smith::Up::Unauthorized { account: 17, generation: 4 },
        ),
        (OpenEvent::Limited { account: 17, reset: None }, smith::Up::Limited { account: 17, reset: None }),
        (
            OpenEvent::Limited { account: 17, reset: Some(Duration::ZERO) },
            smith::Up::Limited { account: 17, reset: Some(Duration::ZERO) },
        ),
        (
            OpenEvent::Limited { account: 17, reset: Some(Duration::from_secs(9)) },
            smith::Up::Limited { account: 17, reset: Some(Duration::from_secs(9)) },
        ),
        (
            OpenEvent::Refused {
                account: 17,
                code: b"quota".as_slice().into(),
                reason: b"provider refused".as_slice().into(),
            },
            smith::Up::Refused {
                account: 17,
                code: b"quota".as_slice().into(),
                reason: b"provider refused".as_slice().into(),
            },
        ),
    ] {
        let actual = crate::worker_route::process(ProcessEvent::Channel { agent: owner, event: Box::new(notice) });
        assert_eq!(
            actual,
            Some(root::Event::Process(Box::new(smith::process::Event::Received { owner, message: expected })))
        );
    }
}

fn encode_grants(
    start: smith_host_domain::Start,
    values: smith_host_protocol::Values,
    agent: &crate::settings::Agent,
) -> Result<skein_channel::Frame, smith_host_protocol::Error> {
    smith_host_protocol::encode_start(
        start,
        smith_host_domain::Window { turns: agent.host_limits.turns, bytes: agent.host_limits.unacknowledged_bytes },
        values,
        &agent.limits.bodies,
        &agent.limits.charter,
        &agent.limits.transcript,
        &agent.endpoints,
    )
}

#[test]
fn key_values_reach_smith_at_their_bound_and_fit_the_workers_memory() {
    const SOURCE: u32 = 17;
    use jig_protocol_accounts::table::{Scheme, Value};
    use smith_host_domain as smith;
    let mut settings = fixtures::worker_settings();
    settings.jig.limits.token = 1;
    let key_bytes = settings.jig.limits.token.min(jig_client_protocol_wire::v19::CEILINGS.keyed_value_some);
    let agent = fixtures::worker_agent();
    let limits = fixtures::worker_limits();
    let files = fixtures::worker_files();
    let worst = worker_settings::worker_worst_case(&settings, &agent, &limits).unwrap();
    let meter = skein_world::domain::heap::Meter::new();
    meter.start();
    let mut worker = crate::Worker::new(&settings, &agent, &limits, &files, fixtures::worker_runtime(), 7).unwrap();
    for generation in [2, 3] {
        worker
            .values
            .granted(
                SOURCE,
                generation,
                Value { scheme: Scheme::Key, token: Secret::new(repeated(b'k', usize::try_from(key_bytes).unwrap())) },
                Time::from_nanos(9),
            )
            .unwrap();
    }
    let measured = meter.end();
    meter.check(measured, worst, &"two retained full-size key generations");
    for size in [key_bytes - 1, key_bytes, key_bytes + 1] {
        let token = Secret::new(repeated(b'k', usize::try_from(size).unwrap()));
        let accepted = worker.values.granted(SOURCE, 3, Value { scheme: Scheme::Key, token }, Time::from_nanos(9));
        assert_eq!(accepted.is_ok(), size <= key_bytes);
        if accepted.is_err() {
            continue;
        }
        let mut start = decoded_chat_start(&agent);
        start.grants = Box::new([smith::Grant {
            account: SOURCE,
            generation: 3,
            valid: Duration::from_secs(5),
            form: smith::Form::Credential(smith::Scheme::Key),
        }]);
        let values = crate::worker::start_values(&worker.values, &start, b"/worker", Time::ZERO).unwrap();
        let encoded = encode_grants(*start, values, &agent).unwrap();
        let offset = encoded.bytes().len() - usize::try_from(encoded.body_len()).unwrap();
        let decoded =
            smith_channel::Start::decode(&agent.limits.bodies, &mut Reader::new(&encoded.bytes()[offset..])).unwrap();
        let smith_channel::GrantValue::Credential(value) = decoded.grants().get(0).unwrap().value() else {
            panic!("credential")
        };
        assert_eq!(value.scheme(), &smith_channel::Scheme::Key);
        assert_eq!(value.bytes().expose().len(), usize::try_from(size).unwrap());
    }
}

fn repeated(byte: u8, len: usize) -> Box<[u8]> {
    let mut bytes = skein_lib::bytes::zeroed(len);
    bytes.fill(byte);
    bytes
}

fn program_worker_settings() -> (crate::settings::Worker, crate::settings::Agent, crate::limits::Limits) {
    (fixtures::worker_settings(), fixtures::worker_agent(), fixtures::worker_limits())
}

fn socket_stand() -> fixtures::WorkerStand {
    let mut settings = fixtures::worker_settings();
    settings.jig.engine = jig_protocol_config::Address::Addr(io::Addr::Ip(([127, 0, 0, 1], 7100).into()));
    let mut stand = fixtures::WorkerStand::new();
    stand.worker = crate::Worker::new(
        &settings,
        &fixtures::worker_agent(),
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

#[test]
fn stopping_keeps_a_dial_until_the_root_sends_stop() {
    let mut stand = socket_stand();
    stand.worker.stop();
    stand.worker.link(fixtures::NOW, link::Request::Dial);
    let request = stand.worker.io().pop().expect("stopping worker still dials");
    let io::Request::Connect { addr, .. } = request else { panic!("dial reaches IO") };
    assert_eq!(addr, io::Addr::Ip(([127, 0, 0, 1], 7100).into()));
}

#[test]
fn linked_is_emitted_after_ready_at_each_opening() {
    let mut stand = socket_stand();
    assert!(stand.worker.notes().is_empty(), "never before Ready");
    LinkPeer::new().open(&mut stand.worker);
    let expected =
        crate::Note::Linked { worker: b"worker-a".as_slice().into(), address: Box::new(([127, 0, 0, 1], 7100).into()) };
    assert_eq!(stand.worker.notes().pop(), Some(expected));
    assert!(stand.worker.notes().is_empty());
    let owner = Token::new(
        worker_settings::prepare(&fixtures::worker_settings(), &fixtures::worker_agent(), &fixtures::worker_limits())
            .unwrap()
            .prefix,
    );
    stand.worker.up(fixtures::NOW, io::Event::Closed { owner });
    stand.worker.iterate(fixtures::NOW, Wall::EPOCH);
    let at = stand.worker.next_deadline().expect("redial");
    stand.worker.iterate(at, Wall::EPOCH);
    let Some(io::Request::Connect { .. }) = stand.worker.io().pop() else { panic!("redial before reopening") };
    LinkPeer::new().open(&mut stand.worker);
    assert_eq!(
        stand.worker.notes().pop(),
        Some(crate::Note::Linked {
            worker: b"worker-a".as_slice().into(),
            address: Box::new(([127, 0, 0, 1], 7100).into())
        })
    );
    assert!(stand.worker.notes().is_empty(), "one per opening");
}

#[test]
fn refusals_are_notes_and_leave_later_dials_intact() {
    use skein_channel::Closed;
    let mut stand = socket_stand();
    for (why, reason) in [
        (Closed::RefusedPeer(256), crate::Refusal::Peer(256)),
        (Closed::RefusedPeer(257), crate::Refusal::Peer(257)),
        (Closed::RefusedPeer(258), crate::Refusal::Peer(258)),
        (Closed::RefusedPeer(259), crate::Refusal::Peer(259)),
        (Closed::RefusedPeer(260), crate::Refusal::Peer(260)),
        (Closed::RefusedPeer(261), crate::Refusal::Peer(261)),
        (Closed::RefusedPeer(42), crate::Refusal::Peer(42)),
        (Closed::RefusedHere(jig_protocol_worker_link::REFUSED_LIMITS), crate::Refusal::Terms),
        (Closed::RefusedHere(jig_protocol_worker_link::REFUSED_FRAMING), crate::Refusal::Malformed),
    ] {
        stand.worker.link(fixtures::NOW, link::Request::Fact { why });
        assert_eq!(
            stand.worker.notes().pop(),
            Some(crate::Note::Refused {
                worker: b"worker-a".as_slice().into(),
                address: Box::new(([127, 0, 0, 1], 7100).into()),
                reason
            })
        );
        stand.worker.link(fixtures::NOW, link::Request::Dial);
        let Some(io::Request::Connect { .. }) = stand.worker.io().pop() else { panic!("redial reaches IO") };
    }
    stand.worker.link(fixtures::NOW, link::Request::Fact { why: Closed::Stream });
    assert!(stand.worker.notes().is_empty());
}

fn alone_service() -> crate::Service {
    let part =
        crate::startup::worker_part(&fixtures::worker_alone_document(), &jig_protocol_config::Directories::working())
            .unwrap();
    crate::startup::worker_alone(part, &fixtures::files(), &fixtures::startup_agent(), fixtures::worker_runtime(), 7)
        .expect("worker alone prepared")
}

#[test]
fn a_worker_alone_has_no_engine_or_store_and_a_second_signal_aborts() {
    let mut service = alone_service();
    assert!(service.engine.is_none());
    assert_eq!(service.adopt_store(io::kernel::Fd::new(99), 1000), Err(io::kernel::Fd::new(99)));
    service.shutdown();
    assert!(service.worker.as_ref().unwrap().stopping());
    assert_eq!(service.end(), None);
    service.shutdown();
    assert_eq!(service.end(), Some(&crate::End::Aborted));
}

fn settle_worker_service(linked: bool) {
    let mut service = alone_service();
    let mut stand = socket_stand();
    if linked {
        LinkPeer::new().open(&mut stand.worker);
    } else {
        let owner = Token::new(
            worker_settings::prepare(
                &fixtures::worker_settings(),
                &fixtures::worker_agent(),
                &fixtures::worker_limits(),
            )
            .unwrap()
            .prefix,
        );
        stand.worker.up(fixtures::NOW, io::Event::Failed { owner, error: io::Error::Unreachable });
    }
    service.worker = Some(stand.worker);
    service.shutdown();
    stand.worker = service.worker.take().unwrap();
    let now = if linked { fixtures::NOW } else { fixtures::NOW.saturating_add(Duration::from_secs(60)) };
    for _ in 0_u16..1000 {
        stand.worker.iterate(now, Wall::EPOCH);
        // The link's Close is answered by its socket owner, as IO would.
        for _ in 0..stand.worker.io().len() {
            let request = stand.worker.io().pop().unwrap();
            match request {
                io::Request::Close { .. } | io::Request::Abort { .. } => {
                    stand.worker.up(
                        now,
                        io::Event::Closed {
                            owner: Token::new(
                                worker_settings::prepare(
                                    &fixtures::worker_settings(),
                                    &fixtures::worker_agent(),
                                    &fixtures::worker_limits(),
                                )
                                .unwrap()
                                .prefix,
                            ),
                        },
                    );
                }
                io::Request::Connect { owner, .. } => {
                    stand.worker.up(now, io::Event::Failed { owner, error: io::Error::Unreachable });
                }
                io::Request::Stream { stream, .. } | io::Request::Output { stream, .. } if stream == Token::new(44) => {
                }
                other @ (io::Request::Listen { .. }
                | io::Request::Bind { .. }
                | io::Request::Reject { .. }
                | io::Request::Stream { .. }
                | io::Request::Output { .. }
                | io::Request::Spawn { .. }
                | io::Request::Signal { .. }
                | io::Request::Usage { .. }
                | io::Request::Window { .. }) => stand.below.out.io.push(other),
            }
        }
        stand.drain();
        stand.input();
        if stand.worker.is_empty() {
            break;
        }
    }
    assert!(stand.worker.is_empty(), "all worker children and its root closed");
    service.worker = Some(stand.worker);
    crate::iterate(&mut service, now, Wall::EPOCH);
    assert_eq!(service.end(), Some(&crate::End::Drained));
    assert!(service.is_empty());
}
#[test]
fn the_first_signal_drains_a_linked_worker_without_waiting_for_grace() {
    settle_worker_service(true);
}
#[test]
fn a_worker_stopped_unlinked_has_ended_past_its_grace() {
    settle_worker_service(false);
}

#[test]
fn signals_route_to_a_worker_alone_then_abort() {
    let mut service = alone_service();
    service.told.push(io::Event::Shutdown { signal: io::kernel::ServiceSignal::Terminate, from: None });
    crate::iterate(&mut service, fixtures::NOW, Wall::EPOCH);
    assert!(service.worker.as_ref().unwrap().stopping());
    assert_eq!(service.end(), None, "its root is still closing");
    service.told.push(io::Event::Shutdown { signal: io::kernel::ServiceSignal::Terminate, from: None });
    crate::iterate(&mut service, fixtures::NOW, Wall::EPOCH);
    assert_eq!(service.end(), Some(&crate::End::Aborted));
}

#[test]
fn repeated_refusals_are_noted_once_until_the_reason_changes_or_the_worker_links() {
    use skein_channel::Closed;
    const ONE_REFUSAL: u32 = 1;
    const TWO_REFUSALS: u32 = 2;
    let mut stand = socket_stand();
    for _ in 0_u8..2 {
        stand.worker.link(fixtures::NOW, link::Request::Fact { why: Closed::RefusedPeer(256) });
    }
    assert_eq!(stand.worker.notes().len(), ONE_REFUSAL);
    stand.worker.link(
        fixtures::NOW,
        link::Request::Fact { why: Closed::RefusedHere(jig_protocol_worker_link::REFUSED_LIMITS) },
    );
    assert_eq!(stand.worker.notes().len(), TWO_REFUSALS);
    LinkPeer::new().open(&mut stand.worker);
    stand.worker.link(fixtures::NOW, link::Request::Fact { why: Closed::RefusedPeer(256) });
    for reason in [crate::Refusal::Peer(256), crate::Refusal::Terms] {
        assert_eq!(stand.worker.notes().pop(), Some(refusal_note(reason)));
    }
    assert_eq!(
        stand.worker.notes().pop(),
        Some(crate::Note::Linked {
            worker: b"worker-a".as_slice().into(),
            address: Box::new(([127, 0, 0, 1], 7100).into()),
        })
    );
    assert_eq!(stand.worker.notes().pop(), Some(refusal_note(crate::Refusal::Peer(256))));
    assert!(stand.worker.notes().is_empty(), "three refusals with Linked between the second and third");
}

fn refusal_note(reason: crate::Refusal) -> crate::Note {
    crate::Note::Refused {
        worker: b"worker-a".as_slice().into(),
        address: Box::new(([127, 0, 0, 1], 7100).into()),
        reason,
    }
}
