//! The worker's limits and its worst case (programming-model.md, section 6).

use alloc::boxed::Box;
use core::mem::size_of;

use crate::wire;
use jig_domain_host as host;
use skein_lib::{Duration, Id, List, Map, Queue, Slab, Token};
use smith_host_domain as agent;
use temper_worker_domain_checkout as checkout;

use crate::agents::{Bound, Call};
use crate::facts::Fact;
use crate::translate::SAVED;
use crate::workspace::{Items, Root, Workspace};

/// The worker domain's limits (section 7), handed to every step read-only: its
/// child domains', each handed down to the one it bounds, and what the hub's
/// hello says (jig's `domain/hosts.md`, section 2).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Limits {
    pub host: host::Limits,
    pub checkout: checkout::Limits,
    pub agent: agent::Limits,
    /// The longest host answer the agent takes: smith's `host_reply_bytes`,
    /// its channel's `bodies.host_reply_text`. A hub's answer past it
    /// reaches the agent as smith's `TooLarge` (`domain/agent.md`, 4.3).
    pub reply_bytes: u32,
    /// The bounds of what a run on this worker may be given: its agent's
    /// models and its runs' turns, spend, time and inbox, for the engine to
    /// place only what fits.
    pub bounds: host::Bounds,
    /// The longest the worker may take to stop a run it lost contact for
    /// (`domain/worker.md`, section 2; jig's `domain/hosts.md`, 6.6), which
    /// its configuration derives and the hello states. The engine's grace is
    /// strictly longer.
    pub stop_bound: Duration,
    /// The backlog, in entries, at which the worker's service stops taking
    /// new work from the engine (skein's `programming-model.md`, 7.1;
    /// `domain/worker.md`, 3.1): a limit on pacing, not on memory.
    pub watermark: u32,
}

/// The watermark's default: the stock `temper init` small profile's former
/// `Prepared::room`, twice its root output bound of 5,099. Its hub emits at
/// most 48 requests, smith 10, checkout 2: (8 * 48 + 2) * (10 + 2) + 8 * 48
/// + 1 * (8 * 2 + 1) replays + 64 activity + 2 root requests = 5,099.
pub const WATERMARK: u32 = 10_198;

/// The bytes a message's sender and words take beyond themselves once smith
/// renders them, its separator (smith's `smith-domain-run`, `rendered_bytes`).
pub const SEPARATOR: u64 = 2;

/// The bytes of a workstream key: the holding task's number, big-endian.
const KEY: u32 = 8;

/// A rule of [`worst_case`] that `limits` break, so that a start names the
/// limits on both its sides.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Unfit {
    /// smith's own check of the agent's limits.
    Agent,
    /// The checkout's repositories past smith's directories, or the agent's.
    Directories,
    /// The checkout's conflicts past the agent's.
    Conflicts,
    /// The checkout's paths past the agent's.
    Paths,
    /// The checkout's names past the agent's.
    Names,
    /// A workstream's key past the checkout's names.
    Key,
    /// The hub's accounts past the agent's.
    Accounts,
    /// The host's slots past the checkout's workspaces.
    Workspaces,
    /// The host's slots past the agent's processes.
    Agents,
    /// The agent's turns past the hub's.
    Turns,
    /// The agent's turns, each of the hub's turn bytes, past the hub's turn queue.
    TurnQueue,
    /// The agent's calls past the hub's run's.
    Calls,
    /// The hub's event with smith's separator past the agent's message.
    Events,
    /// The hub's held events past the agent's messages.
    Held,
    /// The hub's inbox past the agent's messages.
    Inbox,
    /// The agent's call past the checkout's commit message.
    CallBytes,
    /// The save's message past the checkout's commit message.
    Saved,
    /// The hello's inbox past the hub's.
    BoundsInbox,
    /// The hello's message past the hub's event.
    BoundsMessage,
    /// The hello's unacknowledged turns past the agent's.
    Unacknowledged,
}

/// Each rule [`worst_case`] holds `limits` to beyond the memory it counts
/// that they break, in its order; empty, they fit.
#[must_use]
pub fn unfit(limits: &Limits) -> List<Unfit> {
    let Limits { host: host_limits, checkout: checkout_limits, agent: agent_limits, .. } = limits;
    let turn_queue = match u64::from(agent_limits.turns).checked_mul(host_limits.turn_bytes) {
        Some(bytes) => bytes <= host_limits.turn_queue_bytes,
        None => false,
    };
    let events = match host_limits.event_bytes.checked_add(SEPARATOR) {
        Some(bytes) => bytes <= agent_limits.message_bytes,
        None => false,
    };
    let mut broken = List::with_capacity(UNFIT);
    for (fits, unfit) in [
        (agent::check(agent_limits).is_ok(), Unfit::Agent),
        (
            checkout_limits.repositories <= agent::MAX_DIRECTORIES
                && checkout_limits.repositories <= agent_limits.directories,
            Unfit::Directories,
        ),
        (checkout_limits.conflicts <= agent_limits.conflicts, Unfit::Conflicts),
        (checkout_limits.path_bytes <= agent_limits.path_bytes, Unfit::Paths),
        (checkout_limits.name_bytes <= agent_limits.name_bytes, Unfit::Names),
        (KEY <= checkout_limits.name_bytes, Unfit::Key),
        (host_limits.accounts <= agent_limits.accounts, Unfit::Accounts),
        (host_limits.slots <= checkout_limits.workspaces, Unfit::Workspaces),
        (host_limits.slots <= agent_limits.agents, Unfit::Agents),
        (agent_limits.turns <= host_limits.turns, Unfit::Turns),
        (turn_queue, Unfit::TurnQueue),
        (agent_limits.calls <= host_limits.run_calls, Unfit::Calls),
        (events, Unfit::Events),
        (host_limits.held <= agent_limits.messages, Unfit::Held),
        (host_limits.inbox <= agent_limits.messages, Unfit::Inbox),
        (agent_limits.call_bytes <= u64::from(checkout_limits.message_bytes), Unfit::CallBytes),
        (len(SAVED) <= u64::from(checkout_limits.message_bytes), Unfit::Saved),
        (limits.bounds.inbox <= host_limits.inbox, Unfit::BoundsInbox),
        (u64::from(limits.bounds.message_bytes) <= host_limits.event_bytes, Unfit::BoundsMessage),
        (limits.bounds.unacknowledged <= agent_limits.turns, Unfit::Unacknowledged),
    ] {
        if !fits {
            broken.push(unfit).expect("one entry per rule");
        }
    }
    broken
}
const UNFIT: u32 = 20;

/// The most memory the domain holds under `limits`, in bytes (6.3), or `None`
/// if it does not fit a `u64` or the limits cannot be honoured: the child
/// domains' own, or limits under which one child domain would refuse what
/// another passes it within its own, or a hello that promises what the hub
/// refuses. The checkout's repositories, names and conflicts must fit
/// smith's directories, and a workstream's key its names;
/// the host's runs must each find a workspace and an agent (no more slots
/// than workspaces or agent processes); what goes down to a run must be what
/// an agent takes (an inbound event, rendered, no larger than its messages,
/// and no more held for a run until it is live, nor carried in its start,
/// than may wait for it in its agent), and smith's turns unacknowledged, each
/// no larger encoded than the host takes a turn, must fit the host's reserve
/// for a stopping run, its turns and its bytes; a run's calls must fit the
/// host's; a run's push message, as smith bounds a call, and the save's
/// must fit a commit message; and the hello's bounds must be within the
/// hub's inbox and events and smith's turn window.
///
/// It is the child domains', plus what the root keeps: the agents it bound
/// and their calls in flight with their names and what they ask, the
/// ends waiting behind hand-outs, a record of each
/// workspace and the map that finds those being prepared, each run's
/// workspace items until the engine has its answer, and the bounded facts.
/// The uncapped inter-child queues are outside this estimate. What the
/// queued requests own is counted where they end up.
#[must_use]
pub fn worst_case(limits: &Limits) -> Option<u64> {
    let Limits { host: host_limits, checkout: checkout_limits, agent: agent_limits, .. } = limits;
    if !unfit(limits).is_empty() {
        return None;
    }
    let children = host::worst_case(host_limits)?
        .checked_add(checkout::worst_case(checkout_limits)?)?
        .checked_add(agent::worst_case(agent_limits)?)?;
    let calls = agent_limits.agents.checked_mul(agent_limits.calls)?;
    let call = host_limits
        .event_bytes
        .checked_add(agent_limits.call_bytes)?
        .checked_add(u64::from(agent_limits.name_bytes))?;
    let bindings = Map::<Token, Bound>::worst_case(agent_limits.agents)?
        .checked_add(Map::<(Token, Token), Call>::worst_case(calls)?)?
        .checked_add(u64::from(calls).checked_mul(call)?)?
        .checked_add(Map::<Token, Box<[u8]>>::worst_case(agent_limits.agents)?)?
        .checked_add(u64::from(agent_limits.agents).checked_mul(u64::from(agent_limits.detail_bytes))?)?;
    let facts = Queue::<Fact>::worst_case(facts(limits)?)?;
    children.checked_add(bindings)?.checked_add(workspace_memory(limits)?)?.checked_add(facts)
}

/// The root's records of workspaces, and each run's workspace items until
/// the engine has its answer: a slot's, and one more for an assignment the
/// hub refuses in the step it comes in.
fn workspace_memory(limits: &Limits) -> Option<u64> {
    let host_limits = &limits.host;
    let checkout_limits = &limits.checkout;
    let slots = host_limits.slots;
    let name = u64::from(checkout_limits.name_bytes);
    let conflicts = u64::from(checkout_limits.conflicts)
        .checked_mul(u64::try_from(size_of::<Box<[u8]>>()).ok()?.checked_add(u64::from(checkout_limits.path_bytes))?)?;
    let root = u64::try_from(size_of::<Root>())
        .ok()?
        .checked_add(name)?
        .checked_add(u64::try_from(size_of::<checkout::Conflicts>()).ok()?)?
        .checked_add(conflicts)?;
    let workspaces = Slab::<Workspace>::worst_case(slots)?
        .checked_add(Map::<Token, Id<Workspace>>::worst_case(slots)?)?
        .checked_add(u64::from(slots).checked_mul(u64::from(checkout_limits.repositories).checked_mul(root)?)?)?;
    let staged = slots.checked_add(1)?;
    let item = u64::try_from(size_of::<wire::Repository>())
        .ok()?
        .checked_add(name.checked_mul(5)?)?
        .checked_add(u64::from(checkout::PROGRAM_BYTES))?;
    let feedback = u64::try_from(size_of::<wire::SavedRepository>()).ok()?.checked_add(conflicts)?;
    // Each retained item has a u64 tag and one from-saved flag beside its feedback.
    let per = u64::from(checkout_limits.repositories)
        .checked_mul(item.checked_add(feedback)?.checked_add(9)?)?
        .checked_add(Map::<u64, [u8; 32]>::worst_case(checkout_limits.repositories)?)?
        .checked_add(name)?
        .checked_add(agent::Delivered::worst_case())?;
    let items = Map::<Token, Items>::worst_case(staged)?
        .checked_add(Map::<(Token, Token), Token>::worst_case(staged)?)?
        .checked_add(u64::from(staged).checked_mul(per)?)?;
    workspaces.checked_add(items)
}

fn len(bytes: &[u8]) -> u64 {
    u64::try_from(bytes.len()).expect("a length fits in a u64")
}

/// Facts kept until the loop drains them: as many as the child domains keep.
pub(crate) fn facts(limits: &Limits) -> Option<u32> {
    limits.host.facts.checked_add(limits.checkout.facts)?.checked_add(limits.agent.facts)
}
