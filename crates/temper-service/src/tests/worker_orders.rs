//! The service keeps these orders under load (programming-model.md, 7.2):
//! each root entrance's requests, including payload returns, are dispatched
//! before the next entrance (W3), Hello precedes reconnect replay, and
//! admitted progress precedes a watchdog due in the same iteration (W4).

use super::fixtures::{self, worker_peers::*};
use jig_domain_host as hub;
use jig_protocol_worker_link as link;
use skein_io as io;
use skein_lib::{List, Time, Token, Wall};
use smith_protocol_channel as channel;
use temper_worker_domain as root;

// Three retained turns leave credit for a later terminal record.
const RETAINED: u32 = 3;
const WINDOW: u32 = RETAINED + 1;
// Two named messages fill the fixture's inbox before cancellation.
const MESSAGES: [(Token, &[u8]); 2] = [(Token::new(71), b"first"), (Token::new(72), b"second")];
const STEPS: usize = 1000; // The concrete peers' existing settlement budget.
const ADMITTED_FRAMES: usize = 1;
const WITH_TURN: usize = ADMITTED_FRAMES + 1;
const PROCESS_RECORDS: u32 = 3; // The two messages and cancellation.

fn chat() -> (Token, Token) {
    let link::Request::Down { down: hub::LinkDown::Assign { assignment }, .. } = fixtures::chat_assignment() else {
        panic!("chat assignment")
    };
    (assignment.assignment.run, assignment.assignment.attempt)
}

fn queued_message(name: Token, words: &[u8]) -> root::Event {
    let (run, attempt) = chat();
    root::Event::Link(hub::LinkDown::Message {
        run,
        attempt,
        name,
        sender: Box::from(&b"owner"[..]),
        words: words.into(),
        finish_requested: false,
    })
}

/// Socket notes are an edge. Hold their consumer's credit while translating
/// a real process record; the root entrance then waits in events.
fn hold_notes_while_translating(stand: &mut fixtures::WorkerStand) {
    while stand.worker.notes().room() > 0 {
        stand.worker.notes().push(crate::Note::Linked {
            worker: Box::from(&b"worker-a"[..]),
            address: Box::new(([127, 0, 0, 1], 7100).into()),
        });
    }
    stand.worker.iterate(fixtures::NOW, Wall::EPOCH);
    while stand.worker.notes().pop().is_some() {}
}

fn stage_turn(stand: &mut fixtures::WorkerStand, peer: &mut AgentPeer, number: u32, frames: usize) {
    let agent = fixtures::worker_agent();
    let body = turn_body(&agent, number);
    let turn = channel::decode_turn(&body, number, &agent.limits.transcript, &agent.endpoints).unwrap();
    peer.component
        .send_turn(
            number,
            None,
            smith_domain_run::Spend::ZERO,
            number == 1,
            &turn,
            peer.owner,
            &mut peer.events,
            &mut peer.out,
        )
        .unwrap();
    peer.stage_frame(&mut stand.worker, frames);
}

/// A real turn is translated with socket credit withheld. Two message
/// entrances and cancellation queue behind it before credit is restored.
fn several_entrances() -> (fixtures::WorkerStand, LinkPeer, AgentPeer) {
    let (mut stand, _, agent) = window_stand();
    let mut link = LinkPeer::new();
    link.open(&mut stand.worker);
    let mut peer = AgentPeer::assigned(&mut stand.worker, &agent);
    stage_turn(&mut stand, &mut peer, 1, WITH_TURN);
    hold_notes_while_translating(&mut stand);
    for (name, words) in MESSAGES {
        stand.worker.events.push(queued_message(name, words));
    }
    let (run, attempt) = chat();
    stand.worker.events.push(root::Event::Link(hub::LinkDown::Cancel { run, attempt }));
    (stand, link, peer)
}

#[test]
fn an_entrances_requests_are_dispatched_before_the_next_entrance() {
    let (mut stand, mut link, mut peer) = several_entrances();
    stand.worker.iterate(fixtures::NOW, Wall::EPOCH);
    // Agent output is allowed through while the engine link still withholds
    // its output credit: dispatch does not wait for wire transmission.
    peer.settle(&mut stand.worker);
    let mut delivered = List::with_capacity(PROCESS_RECORDS);
    while let Some(event) = peer.events.pop() {
        if let channel::OpenEvent::Message { name, .. } = event {
            delivered.push(Some(name)).expect("two messages and cancellation");
        } else if let channel::OpenEvent::Cancel = event {
            delivered.push(None).expect("two messages and cancellation");
        }
    }
    assert_eq!(
        delivered.as_slice(),
        [Some(MESSAGES[0].0), Some(MESSAGES[1].0), None],
        "process requests follow entrance order"
    );
    for _ in 0..STEPS {
        if link.records.as_slice().contains(&LinkRecord::Turn(1)) {
            break;
        }
        link.turn(&mut stand.worker);
    }
    finish_cancelled_agent(&mut stand, &mut link, &mut peer);
    assert_eq!(
        link.records.as_slice(),
        [LinkRecord::Hello, LinkRecord::Turn(1), LinkRecord::Turn(2), LinkRecord::Answer(2)],
        "both turn hand-outs and the result reach the link in order, each once"
    );
}

// The cancellation's final buffered records use the same real channel.
// Root exit/reap terminals supply the process lifetime proof after its EOF;
// engine acknowledgements settle the turns the real peer received.
fn finish_cancelled_agent(stand: &mut fixtures::WorkerStand, link: &mut LinkPeer, peer: &mut AgentPeer) {
    const FINAL_TURNS: u32 = 2; // One turn before cancellation, one buffered after it.
    const FINAL_FRAMES: usize = ADMITTED_FRAMES + 2 + 1;
    stage_turn(stand, peer, FINAL_TURNS, FINAL_FRAMES - 1);
    peer.settle(&mut stand.worker);
    peer.component
        .send_answer(
            fixtures::NOW,
            smith_domain_run::Answer::Failed {
                failure: smith_domain_run::Failure::Cancelled,
                spent: smith_domain_run::Spend::ZERO,
                turns: FINAL_TURNS,
            },
            None,
            peer.owner,
            &mut peer.events,
            &mut peer.out,
        )
        .unwrap();
    peer.settle(&mut stand.worker);
    for _ in 0..STEPS {
        if link.records.as_slice().contains(&LinkRecord::Turn(FINAL_TURNS)) {
            break;
        }
        link.turn(&mut stand.worker);
    }
    let (run, attempt) = chat();
    for turn in 1..=FINAL_TURNS {
        stand.worker.events.push(root::Event::Link(hub::LinkDown::TurnKept { run, attempt, turn }));
    }
    peer.settle(&mut stand.worker);
    for event in [
        smith_host_domain::process::Event::Exited { owner: peer.owner, exit: Some(smith_host_domain::Exit::Code(0)) },
        smith_host_domain::process::Event::Reaped { owner: peer.owner, detail: Box::new([]) },
    ] {
        stand.worker.events.push(root::Event::Process(Box::new(event)));
    }
    for _ in 0..STEPS {
        if link.records.as_slice().contains(&LinkRecord::Answer(FINAL_TURNS)) {
            break;
        }
        link.turn(&mut stand.worker);
    }
}

fn window_stand() -> (fixtures::WorkerStand, crate::settings::Worker, crate::settings::Agent) {
    let mut settings = fixtures::worker_settings();
    settings.jig.engine = jig_protocol_config::Address::Addr(io::Addr::Ip(([127, 0, 0, 1], 7100).into()));
    settings.jig.limits.hub.turns = WINDOW;
    settings.jig.limits.hub.turn_queue_bytes =
        settings.jig.limits.hub.turn_queue_bytes.checked_mul(u64::from(WINDOW)).expect("four turns fit");
    let mut agent = fixtures::worker_agent();
    agent.host_limits.turns = WINDOW;
    agent.host_limits.unacknowledged_bytes =
        agent.host_limits.unacknowledged_bytes.checked_mul(u64::from(WINDOW)).expect("four turns fit");
    agent.bounds.unacknowledged = WINDOW;
    let stand = socket_stand_with(&settings, &agent);
    (stand, settings, agent)
}

fn retained_turns_at_a_reconnect() -> (fixtures::WorkerStand, LinkPeer) {
    let (mut stand, settings, agent) = window_stand();
    let mut link = LinkPeer::new();
    link.open(&mut stand.worker);
    let mut peer = AgentPeer::assigned(&mut stand.worker, &agent);
    for number in 1..=RETAINED {
        stage_turn(
            &mut stand,
            &mut peer,
            number,
            ADMITTED_FRAMES.checked_add(usize::try_from(number).unwrap()).unwrap(),
        );
        stand.worker.iterate(fixtures::NOW, Wall::EPOCH);
        peer.settle(&mut stand.worker);
        for _ in 0..STEPS {
            if link.records.as_slice().contains(&LinkRecord::Turn(number)) {
                break;
            }
            link.turn(&mut stand.worker);
        }
    }
    let prefix = crate::worker_settings::prepare(&settings, &agent, &fixtures::worker_limits()).unwrap().prefix;
    stand.worker.up(fixtures::NOW, io::Event::Closed { owner: Token::new(prefix) });
    stand.worker.iterate(fixtures::NOW, Wall::EPOCH);
    let at = stand.worker.next_deadline().expect("redial");
    stand.worker.iterate(at, Wall::EPOCH);
    // The concrete opening helper supplies the connect terminals.
    while stand.worker.io().pop().is_some() {}
    (stand, LinkPeer::new())
}

#[test]
fn the_hello_precedes_the_replayed_turns_at_a_reconnect() {
    let (mut stand, mut link) = retained_turns_at_a_reconnect();
    link.open(&mut stand.worker);
    for _ in 0..STEPS {
        if link.records.len() > RETAINED {
            break;
        }
        link.turn(&mut stand.worker);
    }
    assert_eq!(
        link.records.as_slice(),
        [LinkRecord::Hello, LinkRecord::Turn(1), LinkRecord::Turn(2), LinkRecord::Turn(3)]
    );
}

/// Progress is last among several admitted events at the exact watchdog
/// deadline, after the real process channel has delivered the live piece.
fn progress_at_the_deadline() -> (fixtures::WorkerStand, AgentPeer, Time) {
    let mut stand = socket_stand();
    LinkPeer::new().open(&mut stand.worker);
    let mut peer = AgentPeer::assigned(&mut stand.worker, &fixtures::worker_agent());
    let _sent = peer
        .component
        .send_live(
            channel::Piece { turn: 1, retry: 0, kind: channel::PieceKind::Text, offset: 0, bytes: b"progress" },
            peer.owner,
            &mut peer.events,
            &mut peer.out,
        )
        .unwrap();
    peer.stage_frame(&mut stand.worker, WITH_TURN);
    for (name, words) in MESSAGES {
        stand.worker.events.push(queued_message(name, words));
    }
    let now = stand.worker.root.next_deadline().expect("agent watchdog armed");
    peer.now = now;
    (stand, peer, now)
}

#[test]
fn progress_taken_in_an_iteration_beats_a_watchdog_due_in_it() {
    let (mut stand, mut peer, now) = progress_at_the_deadline();
    stand.worker.iterate(now, Wall::EPOCH);
    assert_eq!(
        stand.worker.root.next_deadline(),
        Some(now.saturating_add(fixtures::worker_agent().host_limits.no_progress)),
        "progress was taken before the due watchdog"
    );
    peer.settle(&mut stand.worker);
    let mut cancelled = false;
    while let Some(event) = peer.events.pop() {
        if let channel::OpenEvent::Cancel = event {
            cancelled = true;
        }
    }
    let mut signalled = false;
    while let Some(request) = stand.worker.io().pop() {
        if let io::Request::Signal { .. } = request {
            signalled = true;
        }
    }
    assert!(!cancelled && !signalled, "the due iteration requests neither stop nor kill of the progressing agent");
}
