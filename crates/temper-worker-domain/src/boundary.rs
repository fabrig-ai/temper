//! The records that cross the boundary with the protocol layer (4.4). The
//! domain defines them; the protocol crate depends on it.
//!
//! Four peers are behind it, and each record names which by its variant:
//!
//! - The engine, over jig's worker link (jig's `domain/hosts.md`, 6.1;
//!   jig's `protocol/channel.md`, section 10), which dials, redials, keeps the
//!   grace and the relays across channels, and tells the domain what it
//!   notices: a channel [`Event::Ready`], [`Event::Lost`], the grace passed
//!   ([`Event::GracePassed`]), a turn to send again ([`Event::Resend`]), a
//!   relay cancelled ([`Event::RelayCancelled`]). What the engine sends is
//!   jig's link vocabulary, the hub's ([`Event::Link`]), but an assignment,
//!   which comes with temper's workspace items beside it ([`Event::Assign`]);
//!   what goes up is the hub's ([`Request::Hello`], [`Request::Link`]), but
//!   a run's answer, which goes with temper's parts beside it
//!   ([`Request::Answer`]). The hub keeps every turn and answer until the
//!   engine acknowledges it, and the root sends them again right after every
//!   hello. A refusal goes once, and holds no slot.
//! - The protocol layer's routing, which decodes and encodes smith's
//!   payloads by hand-out (jig's `domain/hosts.md`, 6.8), keeping no state:
//!   a [`Request::Decode`] is answered by one [`Event::Decoded`] and a
//!   [`Request::Encode`] by one [`Event::Encoded`], each in the order it went
//!   out; a [`Request::Fact`] by one [`Event::Activity`], best effort, as
//!   facts are (`domain/worker.md`, section 7).
//! - io's agent processes and their channels, as smith's host domain defines
//!   them (`smith_host_domain::process`), carried whole: each of its requests
//!   is ended by the terminal its documentation names. A piece of a run's
//!   live content on a channel comes on its own, as [`Event::Live`], best
//!   effort (`domain/worker.md`, section 7).
//! - io's git and file operations, as the checkout defines them
//!   (`temper_worker_domain_checkout::git`): a [`Request::Io`] is ended by one
//!   [`Event::Done`], with `Cancelled` if a [`Request::CancelIo`] won its
//!   race.
//!
//! The shell tells the domain to shut down ([`Event::Shutdown`]); the domain
//! says [`Request::Stop`] once, when it has no agent, hand-out or workspace
//! left, and either no run and the engine has every turn and answer, or, the
//! engine out of reach past the grace, no run left to answer, so that it
//! gives up the turns and answers it keeps, counted (`domain/worker.md`,
//! section 2). The facts still out to be decoded do not hold it: they are
//! counted dropped.
//!
//! A request's `owner` is the token of whoever asked, echoed on its terminal
//! event. Tokens of different families may be equal: the variant routes a
//! terminal to the child domain that asked.

use alloc::boxed::Box;

use crate::wire;
use jig_domain_host as host;
use skein_lib::{Time, Token};
use smith_host_domain as agent;
use temper_worker_domain_checkout::git;

/// protocol -> domain
#[derive(PartialEq, Eq, Debug)]
#[expect(clippy::large_enum_variant, reason = "an assignment crosses whole, as jig's reference root takes it")]
pub enum Event {
    /// Terminal of one source command, correlated by its request owner.
    SourceAnswered { owner: Token, answer: host::sources::Answer },
    /// From the engine: an assignment, jig's record, with temper's workspace
    /// items with their per-repository saved-work branches (jig's `protocol/channel.md`,
    /// 6.2). The record's workspace, if it has one, names the workstream the
    /// checkout is keyed by; the root names the items to the hub itself.
    Assign { assignment: host::Assignment, items: Result<wire::Workspace, wire::Undecodable> },
    /// From the engine: any other record of jig's link. An assignment here
    /// has no workspace items.
    Link(host::LinkDown),
    /// A channel to the engine opened: the hub says its hello.
    Ready,
    /// The channel to the engine closed. Runs go on.
    Lost,
    /// The channel stayed down past the grace: every run is cancelled.
    GracePassed,
    /// The engine answered a turn busy, and its backoff passed: send it again.
    Resend { run: Token, attempt: Token, turn: u32 },
    /// Terminal for `CancelRelay`.
    RelayCancelled { run: Token, attempt: Token, call: Box<[u8]> },
    /// A terminal of one of smith's process requests.
    Process(Box<agent::process::Event>),
    /// A piece of a run's live content from its agent's channel, named by
    /// the agent's token: progress for its watchdog, and the hub's to send
    /// up (jig's `domain/hosts.md`, 6.5).
    Live { agent: Token, piece: host::Piece },
    /// Terminal for `Decode`: smith's start, typed, or what did not decode.
    Decoded { agent: Token, start: Result<Box<agent::Start>, Invalid> },
    /// Terminal for `Encode`.
    Encoded { owner: Token, encoded: Encoded },
    /// Terminal for `Fact`: the fact as jig's activity, if it decoded.
    Activity { owner: Token, activity: Option<host::Activity> },
    /// Terminal for `Io`.
    Done { owner: Token, done: git::Done },
    /// From the shell: cancel every run, admit no more, and stop once nothing
    /// is left.
    Shutdown,
}

/// domain -> protocol
#[derive(PartialEq, Eq, Debug)]
pub enum Request {
    /// Run one bounded program sign-in/sign-out command below the root (jig's domain/hosts.md, 5.1).
    Source { owner: Token, name: host::SourceName, operation: host::sources::Operation, deadline: Time },
    /// The source-command component violated its contract; stop this worker.
    SourceFault,
    /// What the worker says first on every channel to the engine.
    Hello(host::Hello),
    /// Anything else up the link, but an answer.
    Link(host::LinkUp),
    /// A run's one answer, jig's record, with temper's parts beside it: what
    /// the run left on the forge and saved, which the answer's work names,
    /// and, for a workspace that could not be prepared, which repository and
    /// why. A refusal's parts are empty.
    Answer {
        run: Token,
        attempt: Token,
        answer: host::Answer,
        work: wire::Work,
        preparation: Option<wire::Preparation>,
    },
    /// Cancel the local delivery and wait of a relay. Its call still ends
    /// exactly once with `Relayed` or `RelayCancelled`; remote effects remain.
    CancelRelay { run: Token, attempt: Token, call: Box<[u8]> },
    /// Nothing is left: the shell stops.
    Stop,
    /// One of smith's process requests.
    Process(Box<agent::process::Request>),
    /// Decode the hub's start into smith's, for the hosted run `run`.
    Decode { agent: Token, run: Token, start: Box<host::Start> },
    /// Encode one of smith's payloads for the hub.
    Encode { owner: Token, value: Encodable },
    /// Decode one of smith's channel facts into jig's activity.
    Fact { owner: Token, body: Box<[u8]> },
    /// Ask io for the git or file operation `op`, giving up at `deadline`.
    Io { owner: Token, op: git::Op, deadline: Time },
    /// Abandon the `Io` in flight for `owner`. Its terminal event still comes:
    /// `Done` with `Cancelled`, or whichever outcome won the race.
    CancelIo { owner: Token },
}

/// One of smith's payloads, handed out to be encoded.
#[derive(PartialEq, Eq, Debug)]
pub enum Encodable {
    Turn(agent::Turn),
    CallName(agent::CallName),
    Result(agent::RunResult),
}

/// One of smith's payloads, encoded, or what could not be.
#[derive(PartialEq, Eq, Debug)]
pub enum Encoded {
    Turn(host::Turn),
    CallName(Box<[u8]>),
    Result(Box<[u8]>),
    Undecodable,
}

/// What of the hub's start did not decode.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Invalid {
    Charter,
    Transcript,
    CallName,
    /// A resumed delivery's evidence, which must decode and agree with its
    /// outcome.
    Evidence,
}
