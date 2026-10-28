//! Admission under pressure, through the real worker and its concrete peers.
use super::fixtures;
use super::fixtures::worker_peers::{
    AGENT_CHILD, AGENT_ERROR, AGENT_INPUT, AGENT_OUTPUT, AGENT_OWNER, AgentPeer, LinkPeer, PEER_STEPS, demanded_bytes,
    socket_stand,
};
use jig_domain_host as hub;
use jig_protocol_worker_link as link;
use skein_channel::{Lower, LowerEvent};
use skein_io as io;
use skein_lib::{Duration, Time, Token, Wall, stream};
use temper_worker_domain as root;

// Two queued IO requests reach the mark; one leaves a single entry's credit.
const WATERMARK: u32 = 2;
const BACKLOG_ENTITY: Token = Token::new(901);
const NEXT_RUN: Token = Token::new(902);
const PROGRESS_AFTER: Duration = Duration::from_secs(1);
// The automatic start can report Opened and Start before the peer takes them.
const OPENING_EVENTS: u32 = 2;

fn assignment() -> link::Request {
    let link::Request::Down { down: hub::LinkDown::Assign { mut assignment }, parts } = fixtures::chat_assignment()
    else {
        panic!("chat assignment")
    };
    assignment.assignment.run = NEXT_RUN;
    link::Request::Down { down: hub::LinkDown::Assign { assignment }, parts }
}

fn load(worker: &mut crate::Worker, entries: u32) {
    worker.env.limits.watermark = WATERMARK;
    for _ in 0..entries {
        worker.io().push(io::Request::Close { entity: BACKLOG_ENTITY });
    }
}

impl AgentPeer {
    /// Attach the same concrete agent peer to a spawn requested by the real
    /// root, letting the root drive its start and reads.
    fn assigned_for_admission(worker: &mut crate::Worker) -> Self {
        let link::Request::Down { down: hub::LinkDown::Assign { mut assignment }, parts } = fixtures::chat_assignment()
        else {
            panic!("chat assignment")
        };
        assignment.assignment.live.text = true;
        worker.link(fixtures::NOW, link::Request::Down { down: hub::LinkDown::Assign { assignment }, parts });
        worker.iterate(fixtures::NOW, Wall::EPOCH);
        let io::Request::Spawn { owner, .. } = worker.io().pop().expect("assigned agent spawn") else {
            panic!("assignment spawns its agent");
        };
        let mut peer = Self::new(&fixtures::worker_agent(), AGENT_OWNER);
        peer.events = skein_lib::Queue::with_capacity(OPENING_EVENTS);
        worker.up(
            fixtures::NOW,
            io::Event::Spawned { owner, child: AGENT_CHILD, pipes: Box::new([AGENT_INPUT, AGENT_OUTPUT, AGENT_ERROR]) },
        );
        peer.settle(worker);
        // The peer has received Opened and the root's real Start.
        while peer.events.pop().is_some() {}
        peer.component.send_admitted(AGENT_OWNER, &mut peer.events, &mut peer.out).unwrap();
        peer.settle(worker);
        peer
    }

    /// Deliver one checked live frame into the process adapter without
    /// iterating the worker, leaving its progress for the admission stage.
    fn queue_progress(&mut self, worker: &mut crate::Worker) {
        self.component
            .send_live(
                smith_protocol_channel::Piece {
                    kind: smith_protocol_channel::PieceKind::Text,
                    turn: 1,
                    retry: 0,
                    offset: 0,
                    bytes: b"progress",
                },
                AGENT_OWNER,
                &mut self.events,
                &mut self.out,
            )
            .unwrap();
        while let Some(request) = self.out.pop() {
            match request {
                Lower::Write(stream::OutputDown::Room { right, .. }) => {
                    self.lower(LowerEvent::Write(stream::OutputUp::Settled {
                        right,
                        outcome: stream::OutputOutcome::Granted,
                    }));
                }
                Lower::Write(stream::OutputDown::Send { bytes, .. }) => self.worker_bytes.append(&bytes).unwrap(),
                other @ (Lower::Read(_) | Lower::Write(_) | Lower::FinishWrite) => {
                    panic!("live frame output: {other:?}");
                }
            }
        }
        for _ in 0..PEER_STEPS {
            if let Some(bytes) = demanded_bytes(&mut self.worker_read, &mut self.worker_bytes) {
                worker.up(fixtures::NOW, io::Event::Stream { owner: AGENT_OUTPUT, up: stream::Up::Bytes(bytes) });
                if self.worker_bytes.is_empty() {
                    return;
                }
            }
            // Header and body each spend their own demand; stop after the
            // body, before the root takes the resulting progress event.
            worker.iterate(fixtures::NOW, Wall::EPOCH);
            while let Some(request) = worker.io().pop() {
                self.worker_request(worker, request);
            }
        }
        panic!("the complete live frame reaches the process adapter");
    }
}

fn progress_at(entries: u32) -> (fixtures::WorkerStand, Time) {
    let mut stand = fixtures::started_worker();
    let mut agent = AgentPeer::assigned_for_admission(&mut stand.worker);
    agent.queue_progress(&mut stand.worker);
    load(&mut stand.worker, entries);
    stand.worker.link(fixtures::NOW, assignment());
    let before = stand.worker.root.next_deadline().expect("the admitted agent has a watchdog");
    stand.worker.iterate(fixtures::NOW.saturating_add(PROGRESS_AFTER), Wall::EPOCH);
    (stand, before)
}

#[test]
fn new_work_waits_at_the_watermark_while_admitted_work_is_taken() {
    let (mut stand, before) = progress_at(WATERMARK);
    assert_eq!(stand.worker.engine.len(), 1, "the new assignment waits");
    assert_eq!(
        stand.worker.root.next_deadline(),
        Some(before.saturating_add(PROGRESS_AFTER)),
        "agent progress feeds its watchdog"
    );
    for request in stand.worker.io().iter() {
        if let io::Request::Spawn { .. } = request {
            panic!("waiting work prepares no checkout and spawns no agent");
        }
    }
}

#[test]
fn new_work_is_taken_one_entry_below_the_watermark() {
    let (stand, before) = progress_at(WATERMARK - 1);
    assert!(stand.worker.engine.is_empty(), "the assignment was stepped");
    assert_eq!(stand.worker.root.host().hosted(), 1, "the occupied worker keeps its admitted run");
    assert_eq!(stand.worker.root.next_deadline(), Some(before.saturating_add(PROGRESS_AFTER)));
}

#[test]
fn the_engines_later_records_wait_behind_its_waiting_new_work() {
    let mut stand = fixtures::started_worker();
    let mut engine = LinkPeer::new();
    engine.open(&mut stand.worker);
    load(&mut stand.worker, WATERMARK);
    stand.worker.link(fixtures::NOW, assignment());
    stand.worker.link(
        fixtures::NOW,
        link::Request::Down {
            down: hub::LinkDown::Cancel { run: NEXT_RUN, attempt: Token::new(2) },
            parts: Box::new([]),
        },
    );
    stand.worker.iterate(fixtures::NOW, Wall::EPOCH);
    let waiting = stand.worker.engine.len();
    let hosted_while_waiting = stand.worker.root.host().hosted();
    // Release the IO backlog; both records can now cross, in their order.
    while stand.worker.io().pop().is_some() {}
    stand.worker.iterate(fixtures::NOW, Wall::EPOCH);
    let mut spawned = None;
    let mut link_output = skein_lib::Queue::uncapped();
    while let Some(request) = stand.worker.io().pop() {
        if let io::Request::Spawn { owner, .. } = request {
            spawned = Some(owner);
        } else {
            link_output.push(request);
        }
    }
    while let Some(request) = link_output.pop() {
        stand.worker.io().push(request);
    }
    let owner = spawned.expect("assignment starts before cancel");
    stand.worker.up(fixtures::NOW, io::Event::Failed { owner, error: io::Error::Unreachable });
    stand.worker.up(fixtures::NOW, io::Event::Closed { owner });
    let (up, _) = engine.answer(&mut stand.worker);
    assert_eq!((waiting, hosted_while_waiting), (2, 0));
    let hub::LinkUp::Answer { run, attempt, answer } = up else { panic!("the engine gets the cancellation answer") };
    assert_eq!((run, attempt), (NEXT_RUN, Token::new(2)));
    assert_eq!(
        answer.ending,
        hub::Ending::Failed {
            failure: hub::Failure::Cancelled(hub::Reason::Engine),
            detail: Box::new([]),
            work: hub::Work { left: None, saved: None },
        }
    );
    assert_eq!(stand.worker.root.kept_answers(), 1);
    assert!(stand.worker.engine.is_empty());
}

#[test]
fn the_link_is_not_read_without_room_for_one_link_step() {
    let mut stand = socket_stand();
    let mut engine = LinkPeer::new();
    engine.open(&mut stand.worker);
    // Leave one fewer free slot than a link step needs.
    let filled = crate::worker::ENGINE_EVENTS - link::MAX_OUT_DOWN + 1;
    for _ in 0..filled {
        stand.worker.engine.push(root::Event::Lost);
    }
    let without_credit = stand.worker.room();
    // Use the actual service's IO event routing gate with a socket terminal.
    let mut service = super::worker::alone_service();
    service.worker = Some(stand.worker);
    let owner = engine.owner;
    service.told.push(io::Event::Closed { owner });
    crate::iterate(&mut service, fixtures::NOW, Wall::EPOCH);
    let waiting = service.told.len();
    crate::iterate(&mut service, fixtures::NOW, Wall::EPOCH);
    assert!(!without_credit);
    assert_eq!(waiting, 1, "socket input waits while the engine entrance lacks link credit");
    assert!(service.told.is_empty(), "the next iteration routes it after the entrance drained");
}
