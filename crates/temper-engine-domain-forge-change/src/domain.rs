//! One level-triggered change step (domain/forge.md, section 8).
//!
//! The top keeps the returned state and commits each decision before it sends
//! a delegate or effect. Pending states make a repeated step wait. This policy
//! never reads the forge or commits an outbox entry itself.
use alloc::boxed::Box;
use skein_lib::{List, Wall};

use crate::{
    Ask, AuthorityHold, AuthorityLack, Change, Decision, Delegate, Effect, EffectResult, Facts, Freshness, Gate,
    GateFailure, GateKind, Heard, Hold, Limits, Pull, Repair, State, Status, Stepped, Verification, Way,
};

/// Decide one change from current forge and task facts, independent of its wake.
#[must_use]
pub fn step(change: &Change, facts: &Facts, standing: &Heard, now: Wall, limits: &Limits) -> Stepped {
    let stepped = advance(change, facts, standing, now, limits);
    if let Decision::Delegate(delegate) = stepped.decision
        && let Some(why) = missing(standing, delegate)
    {
        // Keep the unrequested phase and its counters: no delegate was made.
        return hold(change.clone(), Hold::Authority(why));
    }
    stepped
}

fn advance(change: &Change, facts: &Facts, standing: &Heard, now: Wall, limits: &Limits) -> Stepped {
    if let Some(merge) = landed_merge(&change.state) {
        return verified(change.clone(), merge, facts.verification, standing.cancelled);
    }
    let mut next = change.clone();
    if change.gates.len() > usize::try_from(limits.gates).expect("u32 fits usize")
        || change.clean.len() > usize::try_from(limits.clean_heads).expect("u32 fits usize")
        || change.gate_failures.len() > usize::try_from(limits.gates).expect("u32 fits usize")
    {
        return hold(next, Hold::Failed);
    }
    if let Some(head) = facts.branch
        && change.last_head != Some(head)
    {
        next.gate_failures = Box::new([]);
    }
    let held = match &change.state {
        State::Landed { .. } | State::Red { .. } | State::Cleared { .. } => {
            unreachable!("verified states returned first")
        }
        State::Ended => return Stepped { change: next, decision: Decision::Cancel },
        State::Held { .. } => true,
        State::Producing { .. }
        | State::Opening { .. }
        | State::Recreating { .. }
        | State::Reopening { .. }
        | State::Checking { .. }
        | State::Gating { .. }
        | State::Queued { .. }
        | State::First { .. }
        | State::Ready { .. }
        | State::Preparing { .. }
        | State::Updating { .. }
        | State::Resolving { .. }
        | State::Repairing { .. }
        | State::Landing { .. } => false,
    };
    match facts.pull {
        Pull::Merged { commit } => {
            next.state = State::Landed { merge: commit };
            next.released = None;
            return verified(next, commit, facts.verification, standing.cancelled);
        }
        Pull::Missing | Pull::Open { .. } | Pull::Closed => {}
    }
    if standing.cancelled {
        next.state = State::Ended;
        next.released = None;
        next.owns_turn = false;
        return Stepped { change: next, decision: Decision::Cancel };
    }
    if standing.released {
        next = released(&next, now);
    }
    if let Some(why) = next.released.take() {
        return released_step(next, why, facts, standing);
    }
    if held {
        return Stepped { change: next, decision: Decision::None };
    }
    if facts.writer_taken {
        return if let State::Ready { .. } = change.state {
            Stepped { change: next, decision: Decision::Wait { until: None } }
        } else {
            waiting(next, now, limits)
        };
    }
    if let Some(why) = facts.drift {
        return hold(next, why);
    }
    // The connector reports a changed target branch through facts.drift.
    // A pull's base commit can lag the landing tip, including our own merge.
    if facts.pull == Pull::Closed && closed_pull_holds(&change.state) {
        return hold(next, Hold::PullClosed);
    }
    match &change.state {
        State::Producing { requested } => producing(next, *requested, facts, standing, now, limits),
        State::Opening { requested } => opening(next, *requested, facts, standing, now, limits),
        State::Recreating { head } => recreating(next, *head, facts, now, limits),
        State::Reopening { requested, retarget } => {
            reopening(next, *requested, *retarget, facts, standing, now, limits)
        }
        State::Checking { head } => checking(next, *head, facts, standing, now, limits),
        State::Gating { head, asked } => gating(next, *head, asked, facts, now, limits),
        State::Ready { head, ready_since } => ready(next, *head, *ready_since, facts, now, limits),
        State::Preparing { from, main, ready_since } => {
            preparing(next, *from, *main, *ready_since, facts, standing, now, limits)
        }
        State::Queued { head, ready_since } => queued(next, *head, *ready_since, facts),
        State::First { head, ready_since } => first(next, *head, *ready_since, facts, now, limits),
        State::Updating { from, base, ready_since: _ } => updating(next, *from, *base, facts, standing, now, limits),
        State::Resolving { requested, base } => resolving(next, *requested, *base, facts, standing, now, limits),
        State::Repairing { requested, why } => repairing(next, *requested, *why, facts, standing, now, limits),
        State::Landing { head } => landing(next, *head, standing, now, limits),
        State::Landed { .. } | State::Red { .. } | State::Cleared { .. } | State::Ended | State::Held { .. } => {
            unreachable!("terminal states returned above")
        }
    }
}

fn landed_merge(state: &State) -> Option<[u8; 32]> {
    match state {
        State::Landed { merge } | State::Red { merge } | State::Cleared { merge } => Some(*merge),
        State::Producing { .. }
        | State::Opening { .. }
        | State::Recreating { .. }
        | State::Reopening { .. }
        | State::Checking { .. }
        | State::Gating { .. }
        | State::Ready { .. }
        | State::Preparing { .. }
        | State::Queued { .. }
        | State::First { .. }
        | State::Updating { .. }
        | State::Resolving { .. }
        | State::Repairing { .. }
        | State::Landing { .. }
        | State::Held { .. }
        | State::Ended => None,
    }
}

fn verified(mut change: Change, merge: [u8; 32], verification: Verification, cancelled: bool) -> Stepped {
    let decision = match verification {
        Verification::Cleared => {
            change.state = State::Cleared { merge };
            Decision::Finish { merge }
        }
        Verification::Red => {
            change.state = State::Red { merge };
            Decision::None
        }
        Verification::Verifying => Decision::None,
    };
    Stepped { change, decision: if cancelled { Decision::Finish { merge } } else { decision } }
}

fn closed_pull_holds(state: &State) -> bool {
    match state {
        State::Reopening { .. } | State::Recreating { .. } => false,
        State::Producing { .. }
        | State::Opening { .. }
        | State::Checking { .. }
        | State::Gating { .. }
        | State::Queued { .. }
        | State::First { .. }
        | State::Ready { .. }
        | State::Preparing { .. }
        | State::Updating { .. }
        | State::Resolving { .. }
        | State::Repairing { .. }
        | State::Landing { .. }
        | State::Landed { .. }
        | State::Red { .. }
        | State::Cleared { .. }
        | State::Ended
        | State::Held { .. } => true,
    }
}

/// Lift a task's own change hold in the same decision as the task's release.
/// The persisted cause lets the next fresh snapshot perform drift recovery.
#[must_use]
pub fn released(change: &Change, now: Wall) -> Change {
    let mut next = change.clone();
    match &change.state {
        State::Held { was, why } => {
            next.state = *was.clone();
            next.released = Some(*why);
            next.gate_failures = Box::new([]);
            next.repairs = 0;
            next.resolutions = 0;
            next.updates = 0;
            next.since = now;
            next.owns_turn = false;
        }
        State::Producing { .. }
        | State::Opening { .. }
        | State::Recreating { .. }
        | State::Reopening { .. }
        | State::Checking { .. }
        | State::Gating { .. }
        | State::Queued { .. }
        | State::First { .. }
        | State::Ready { .. }
        | State::Preparing { .. }
        | State::Updating { .. }
        | State::Resolving { .. }
        | State::Repairing { .. }
        | State::Landing { .. }
        | State::Landed { .. }
        | State::Red { .. }
        | State::Cleared { .. }
        | State::Ended => {}
    }
    next
}

fn released_step(change: Change, why: Hold, facts: &Facts, standing: &Heard) -> Stepped {
    let delegate = if let State::Gating { head, .. } = change.state {
        Delegate::Gate { number: 0, head }
    } else {
        Delegate::Produce
    };
    if let Hold::Authority(prior) = why
        && let Some(current) = missing(standing, delegate)
        && current.lack == prior.lack
    {
        return hold(change, Hold::Authority(AuthorityHold { released: true, ..current }));
    }
    let was = change.state.clone();
    release(change, &was, why, facts)
}

fn release(mut change: Change, was: &State, why: Hold, facts: &Facts) -> Stepped {
    match why {
        Hold::PullClosed => {
            change.state = State::Reopening { requested: true, retarget: false };
            Stepped { change, decision: Decision::Effect(Effect::Reopen) }
        }
        Hold::Retargeted => {
            change.state = State::Reopening { requested: true, retarget: true };
            Stepped { change, decision: Decision::Effect(Effect::Retarget) }
        }
        Hold::BranchMissing => match change.last_head {
            Some(head) => {
                change.state = State::Recreating { head };
                Stepped { change, decision: Decision::Effect(Effect::CreateBranch { head }) }
            }
            None => {
                change.state = State::Producing { requested: false };
                Stepped { change, decision: Decision::None }
            }
        },
        Hold::BranchMoved => {
            change.clean = Box::new([]);
            change.state = match facts.branch {
                Some(head) => {
                    change.last_head = Some(head);
                    pushed(&change, head)
                }
                None => State::Producing { requested: false },
            };
            Stepped { change, decision: Decision::None }
        }
        Hold::Stalled
        | Hold::Repairs
        | Hold::Resolutions
        | Hold::Updates
        | Hold::Failed
        | Hold::Rejected
        | Hold::Authority(_) => {
            change.state = match was {
                State::Ready { head, .. }
                | State::Queued { head, .. }
                | State::First { head, .. }
                | State::Gating { head, .. } => State::Gating { head: *head, asked: Box::new([]) },
                State::Held { .. }
                | State::Landed { .. }
                | State::Red { .. }
                | State::Cleared { .. }
                | State::Ended => {
                    unreachable!("held state retains a live predecessor")
                }
                State::Producing { .. }
                | State::Opening { .. }
                | State::Recreating { .. }
                | State::Reopening { .. }
                | State::Checking { .. }
                | State::Preparing { .. }
                | State::Updating { .. }
                | State::Resolving { .. }
                | State::Repairing { .. }
                | State::Landing { .. } => was.clone(),
            };
            Stepped { change, decision: Decision::None }
        }
    }
}

fn waiting(change: Change, now: Wall, limits: &Limits) -> Stepped {
    let until = Wall::from_nanos(change.since.as_nanos().saturating_add(limits.stall.as_nanos()));
    if now >= until {
        hold(change, Hold::Stalled)
    } else {
        Stepped { change, decision: Decision::Wait { until: Some(until) } }
    }
}

fn settling(change: Change, facts: &Facts, now: Wall, limits: &Limits) -> Stepped {
    if let Some(end) = facts.settling
        && now < end
    {
        Stepped { change, decision: Decision::Wait { until: Some(end) } }
    } else {
        waiting(change, now, limits)
    }
}

fn hold(mut change: Change, why: Hold) -> Stepped {
    if change.owns_turn {
        change.owns_turn = false;
    }
    change.state = State::Held { was: Box::new(change.state.clone()), why };
    Stepped { change, decision: Decision::Hold(why) }
}

fn producing(
    mut change: Change,
    requested: bool,
    facts: &Facts,
    standing: &Heard,
    now: Wall,
    limits: &Limits,
) -> Stepped {
    match facts.branch {
        Some(head) => {
            change.state = State::Opening { requested: false };
            change.last_head = Some(head);
            change.since = now;
            Stepped { change, decision: Decision::None }
        }
        None if !requested => {
            change.state = State::Producing { requested: true };
            Stepped { change, decision: Decision::Delegate(Delegate::Produce) }
        }
        None => match standing.delegate {
            Status::Failed => hold(change, Hold::Failed),
            Status::Unknown | Status::Pending | Status::Passed => waiting(change, now, limits),
        },
    }
}

fn opening(
    mut change: Change,
    requested: bool,
    facts: &Facts,
    standing: &Heard,
    now: Wall,
    limits: &Limits,
) -> Stepped {
    let Some(head) = facts.branch else {
        return hold(change, Hold::BranchMissing);
    };
    change.last_head = Some(head);
    match facts.pull {
        Pull::Open { head: pull_head, .. } => {
            if head != pull_head {
                return hold(change, Hold::BranchMoved);
            }
            change.state = pushed(&change, head);
            change.since = now;
            Stepped { change, decision: Decision::None }
        }
        Pull::Missing if !requested => {
            change.state = State::Opening { requested: true };
            Stepped { change, decision: Decision::Effect(Effect::Open) }
        }
        Pull::Missing => match standing.effect {
            EffectResult::Failed | EffectResult::Conflict => hold(change, Hold::Failed),
            EffectResult::None | EffectResult::Pending | EffectResult::Made => waiting(change, now, limits),
        },
        Pull::Closed | Pull::Merged { .. } => unreachable!("closed and merged pulls returned above"),
    }
}

fn recreating(mut change: Change, head: [u8; 32], facts: &Facts, now: Wall, limits: &Limits) -> Stepped {
    match facts.branch {
        Some(observed) if observed == head => {
            change.state = match facts.pull {
                Pull::Closed => State::Reopening { requested: false, retarget: false },
                Pull::Missing | Pull::Open { .. } | Pull::Merged { .. } => State::Opening { requested: false },
            };
            Stepped { change, decision: Decision::None }
        }
        Some(_) => hold(change, Hold::BranchMoved),
        None => waiting(change, now, limits),
    }
}

fn reopening(
    mut change: Change,
    requested: bool,
    retarget: bool,
    facts: &Facts,
    standing: &Heard,
    now: Wall,
    limits: &Limits,
) -> Stepped {
    let Some(head) = facts.branch else {
        return hold(change, Hold::BranchMissing);
    };
    match facts.pull {
        Pull::Open { head: pull_head, .. } if pull_head == head => {
            change.state = pushed(&change, head);
            change.since = now;
            Stepped { change, decision: Decision::None }
        }
        Pull::Open { .. } | Pull::Closed | Pull::Missing if !requested => {
            change.state = State::Reopening { requested: true, retarget };
            let effect = if retarget { Effect::Retarget } else { Effect::Reopen };
            Stepped { change, decision: Decision::Effect(effect) }
        }
        Pull::Open { .. } | Pull::Closed | Pull::Missing => match standing.effect {
            EffectResult::Failed | EffectResult::Conflict => hold(change, Hold::Failed),
            EffectResult::None | EffectResult::Pending | EffectResult::Made => waiting(change, now, limits),
        },
        Pull::Merged { .. } => unreachable!("merged pull returned above"),
    }
}

fn checking(
    mut change: Change,
    head: [u8; 32],
    facts: &Facts,
    _standing: &Heard,
    now: Wall,
    limits: &Limits,
) -> Stepped {
    let Some(branch) = facts.branch else {
        return hold(change, Hold::BranchMissing);
    };
    if branch != head {
        return hold(change, Hold::BranchMoved);
    }
    change.last_head = Some(head);
    if facts.ci.head != head {
        return waiting(change, now, limits);
    }
    match facts.ci.status {
        Status::Passed => {
            change.state = State::Gating { head, asked: Box::new([]) };
            Stepped { change, decision: Decision::None }
        }
        Status::Failed => {
            let why = if change.clean.is_empty() { Repair::Ci } else { Repair::Semantic };
            repair_due(change, why, facts, now, limits)
        }
        Status::Unknown | Status::Pending => {
            if has_eager_gate(&change) {
                change.state = State::Gating { head, asked: Box::new([]) };
                Stepped { change, decision: Decision::None }
            } else {
                settling(change, facts, now, limits)
            }
        }
    }
}

fn has_eager_gate(change: &Change) -> bool {
    for gate in &change.gates {
        if gate.eager {
            return true;
        }
    }
    false
}

fn valid(change: &Change, gate: Gate, report: crate::GateReport, head: [u8; 32]) -> bool {
    if report.number != gate.number {
        return false;
    }
    if report.head == head {
        return true;
    }
    match gate.freshness {
        Freshness::Exact => false,
        Freshness::Clean => change.clean.contains(&report.head),
    }
}

fn verdict(change: &Change, gate: Gate, facts: &Facts, head: [u8; 32]) -> Status {
    let mut passed = false;
    let mut pending = false;
    let mut failed = false;
    for report in &facts.gates {
        if !valid(change, gate, *report, head) {
            continue;
        }
        match report.status {
            Status::Passed => passed = true,
            Status::Pending => pending = true,
            Status::Failed => failed = true,
            Status::Unknown => {}
        }
    }
    if failed {
        Status::Failed
    } else if pending {
        Status::Pending
    } else if passed {
        Status::Passed
    } else {
        Status::Unknown
    }
}

fn gating(mut change: Change, head: [u8; 32], asked: &[u64], facts: &Facts, now: Wall, limits: &Limits) -> Stepped {
    if facts.branch != Some(head) {
        return hold(change, Hold::BranchMoved);
    }
    for failure in &change.gate_failures {
        if failure.count >= 2 {
            for gate in &change.gates {
                if failure.number == gate.number {
                    return hold(change, Hold::Failed);
                }
            }
        }
    }
    if change.way == Way::Queue && facts.ci.head == head && facts.ci.status == Status::Failed {
        let why = if change.clean.is_empty() { Repair::Ci } else { Repair::Semantic };
        return repair_due(change, why, facts, now, limits);
    }
    let ci_passed = change.way == Way::Turn || (facts.ci.head == head && facts.ci.status == Status::Passed);
    let mut waiting_gate = false;
    for index in 0..change.gates.len() {
        let gate = *change.gates.get(index).expect("index is in bounds");
        if !ci_passed && !gate.eager {
            continue;
        }
        match verdict(&change, gate, facts, head) {
            Status::Failed if gate.blocking => match gate.kind {
                GateKind::Person => return hold(change, Hold::Rejected),
                GateKind::Agent | GateKind::Check => {
                    return repair_due(change, Repair::Gate(gate.number), facts, now, limits);
                }
            },
            Status::Unknown if !asked.contains(&gate.number) => {
                let mut collected = List::with_capacity(limits.gates);
                for number in asked {
                    collected.push(*number).expect("asked gates fit the limit");
                }
                collected.push(gate.number).expect("gate admission checked");
                change.state = State::Gating { head, asked: collected.into_boxed() };
                return Stepped { change, decision: Decision::Delegate(Delegate::Gate { number: gate.number, head }) };
            }
            Status::Unknown | Status::Pending if gate.blocking => waiting_gate = true,
            Status::Failed | Status::Passed | Status::Unknown | Status::Pending => {}
        }
    }
    if !ci_passed || waiting_gate {
        return waiting(change, now, limits);
    }
    if facts.kept {
        return Stepped { change, decision: Decision::Wait { until: None } };
    }
    let since = match change.ready_since {
        Some(since) => since,
        None => {
            change.ready_since = Some(now);
            now
        }
    };
    if change.way == Way::Turn {
        change.state = State::Ready { head, ready_since: since };
    } else if change.owns_turn {
        change.state = State::First { head, ready_since: since };
    } else {
        change.state = State::Queued { head, ready_since: since };
    }
    Stepped { change, decision: Decision::Ready }
}

fn queued(mut change: Change, head: [u8; 32], ready_since: Wall, facts: &Facts) -> Stepped {
    if facts.branch != Some(head) {
        return hold(change, Hold::BranchMoved);
    }
    if facts.kept
        || facts.ci.head != head
        || facts.ci.status != Status::Passed
        || !blocking_gates_pass(&change, facts, head)
    {
        change.state = State::Gating { head, asked: Box::new([]) };
        change.owns_turn = false;
        return Stepped { change, decision: Decision::None };
    }
    if facts.first {
        change.state = State::First { head, ready_since };
        change.owns_turn = true;
        Stepped { change, decision: Decision::None }
    } else {
        Stepped { change, decision: Decision::Wait { until: None } }
    }
}

fn blocking_gates_pass(change: &Change, facts: &Facts, head: [u8; 32]) -> bool {
    for gate in &change.gates {
        if gate.blocking && verdict(change, *gate, facts, head) != Status::Passed {
            return false;
        }
    }
    true
}

fn first(mut change: Change, head: [u8; 32], ready_since: Wall, facts: &Facts, now: Wall, limits: &Limits) -> Stepped {
    if facts.branch != Some(head) {
        return hold(change, Hold::BranchMoved);
    }
    if facts.kept
        || facts.ci.head != head
        || facts.ci.status != Status::Passed
        || !blocking_gates_pass(&change, facts, head)
    {
        change.state = State::Gating { head, asked: Box::new([]) };
        change.owns_turn = false;
        return Stepped { change, decision: Decision::None };
    }
    if facts.base_ci.head == facts.base_tip && facts.base_ci.status == Status::Failed {
        return if facts.queue_repair_active {
            Stepped { change, decision: Decision::Wait { until: None } }
        } else {
            Stepped { change, decision: Decision::QueueRepair }
        };
    }
    match facts.contains_base {
        Status::Failed => return update_due(change, head, facts.base_tip, ready_since, now, limits),
        Status::Unknown | Status::Pending => return waiting(change, now, limits),
        Status::Passed => {}
    }
    match facts.mergeable {
        Status::Passed => {
            if !facts.may_merge {
                return Stepped { change, decision: Decision::Wait { until: None } };
            }
            change.state = State::Landing { head };
            Stepped { change, decision: Decision::Effect(Effect::Merge { head, base: facts.expected_base }) }
        }
        Status::Failed => resolving_due(change, facts.base_tip, now, limits),
        Status::Unknown | Status::Pending => settling(change, facts, now, limits),
    }
}

fn updating(
    mut change: Change,
    from: [u8; 32],
    base: [u8; 32],
    facts: &Facts,
    standing: &Heard,
    now: Wall,
    limits: &Limits,
) -> Stepped {
    match standing.effect {
        EffectResult::Conflict => resolving_due(change, base, now, limits),
        EffectResult::Failed => hold(change, Hold::Failed),
        EffectResult::Made => {
            let Some(head) = facts.branch else { return waiting(change, now, limits) };
            if head == from {
                return waiting(change, now, limits);
            }
            if change.clean.len() >= usize::try_from(limits.clean_heads).expect("u32 fits usize") {
                return hold(change, Hold::Updates);
            }
            let mut clean = List::with_capacity(limits.clean_heads);
            for predecessor in &change.clean {
                clean.push(*predecessor).expect("clean history admitted");
            }
            clean.push(from).expect("clean history has room");
            change.clean = clean.into_boxed();
            change.state = pushed(&change, head);
            change.last_head = Some(head);
            change.since = now;
            Stepped { change, decision: Decision::None }
        }
        EffectResult::None | EffectResult::Pending => waiting(change, now, limits),
    }
}

fn repair_due(mut change: Change, why: Repair, facts: &Facts, now: Wall, limits: &Limits) -> Stepped {
    if change.way == Way::Queue {
        match facts.contains_base {
            Status::Failed => match facts.branch {
                Some(head) => {
                    let ready_since = change.ready_since.unwrap_or(now);
                    return update_due(change, head, facts.base_tip, ready_since, now, limits);
                }
                None => return hold(change, Hold::BranchMissing),
            },
            Status::Unknown | Status::Pending => return waiting(change, now, limits),
            Status::Passed => {}
        }
    }
    if change.repairs >= limits.repairs {
        return hold(change, Hold::Repairs);
    }
    change.repairs = change.repairs.checked_add(1).expect("repair count below limit");
    change.owns_turn = false;
    change.clean = Box::new([]);
    change.state = State::Repairing { requested: true, why };
    change.since = now;
    Stepped { change, decision: Decision::Delegate(Delegate::Repair(why)) }
}

fn update_due(
    mut change: Change,
    head: [u8; 32],
    base: [u8; 32],
    ready_since: Wall,
    now: Wall,
    limits: &Limits,
) -> Stepped {
    if change.updates >= limits.updates {
        return hold(change, Hold::Updates);
    }
    change.updates = change.updates.checked_add(1).expect("update count below limit");
    change.state = State::Updating { from: head, base, ready_since };
    change.since = now;
    Stepped { change, decision: Decision::Effect(Effect::Update { head, base }) }
}

fn resolving_due(mut change: Change, base: [u8; 32], now: Wall, limits: &Limits) -> Stepped {
    if change.resolutions >= limits.resolutions {
        return hold(change, Hold::Resolutions);
    }
    change.resolutions = change.resolutions.checked_add(1).expect("resolution count below limit");
    change.owns_turn = false;
    change.clean = Box::new([]);
    change.state = State::Resolving { requested: true, base };
    change.since = now;
    Stepped { change, decision: Decision::Delegate(Delegate::Resolve { base }) }
}

fn repairing(
    change: Change,
    requested: bool,
    why: Repair,
    facts: &Facts,
    standing: &Heard,
    now: Wall,
    limits: &Limits,
) -> Stepped {
    delegated(change, requested, Delegate::Repair(why), facts, standing, now, limits)
}

fn resolving(
    change: Change,
    requested: bool,
    base: [u8; 32],
    facts: &Facts,
    standing: &Heard,
    now: Wall,
    limits: &Limits,
) -> Stepped {
    delegated(change, requested, Delegate::Resolve { base }, facts, standing, now, limits)
}

fn delegated(
    mut change: Change,
    requested: bool,
    delegate: Delegate,
    facts: &Facts,
    standing: &Heard,
    now: Wall,
    limits: &Limits,
) -> Stepped {
    if !requested {
        change.state = match delegate {
            Delegate::Repair(why) => State::Repairing { requested: true, why },
            Delegate::Resolve { base } => State::Resolving { requested: true, base },
            Delegate::Produce | Delegate::Gate { .. } => unreachable!("delegated phase has one kind"),
        };
        return Stepped { change, decision: Decision::Delegate(delegate) };
    }
    match standing.delegate {
        Status::Passed => {
            let Some(head) = facts.branch else { return waiting(change, now, limits) };
            change.clean = Box::new([]);
            change.owns_turn = false;
            change.state = pushed(&change, head);
            change.last_head = Some(head);
            change.since = now;
            Stepped { change, decision: Decision::None }
        }
        Status::Failed => hold(change, Hold::Failed),
        Status::Unknown | Status::Pending => waiting(change, now, limits),
    }
}

fn landing(mut change: Change, head: [u8; 32], standing: &Heard, now: Wall, limits: &Limits) -> Stepped {
    match standing.effect {
        EffectResult::Failed | EffectResult::Conflict => {
            change.state = State::First { head, ready_since: change.ready_since.unwrap_or(now) };
            Stepped { change, decision: Decision::None }
        }
        EffectResult::None | EffectResult::Pending | EffectResult::Made => waiting(change, now, limits),
    }
}

/// Record a failed review without inventing a verdict (domain/forge.md, 8.3).
/// The top consumes the child's result once before calling this transition.
#[must_use]
pub fn review_failed(change: &Change, number: u64, head: [u8; 32], now: Wall, limits: &Limits) -> Change {
    let mut next = change.clone();
    let State::Gating { head: current, asked } = &change.state else { return next };
    if *current != head || !asked.contains(&number) {
        return next;
    }
    let mut failures = List::with_capacity(limits.gates);
    let mut count = 1;
    for failure in &change.gate_failures {
        if failure.number == number {
            count = failure.count.saturating_add(1).min(2);
        } else {
            failures.push(*failure).expect("admitted gate failures");
        }
    }
    failures.push(GateFailure { number, count }).expect("one count per admitted gate");
    next.gate_failures = failures.into_boxed();
    let mut remaining = List::with_capacity(limits.gates);
    for prior in asked {
        if *prior != number {
            remaining.push(*prior).expect("admitted gate requests");
        }
    }
    next.state = State::Gating { head, asked: remaining.into_boxed() };
    next.since = now;
    next
}

/// The hold's reason, shared by the requester and change panel.
#[must_use]
pub fn authority_words(why: AuthorityHold) -> Box<[u8]> {
    let mut words = List::with_capacity(512);
    match why.lack {
        AuthorityLack::Tasks => {
            put(&mut words, b"its ");
            put(&mut words, skein_lib::Decimal::of(why.had).as_bytes());
            put(&mut words, b" tasks are made");
        }
        AuthorityLack::Depth => put(&mut words, b"its depth is used"),
        AuthorityLack::Spend => {
            put(&mut words, b"its spend is below a run's reserve (it has ");
            put(&mut words, skein_lib::Decimal::of(why.had).as_bytes());
            put(&mut words, b" left)");
        }
    }
    if why.released {
        put(&mut words, b"; released without more ");
        put(
            &mut words,
            match why.lack {
                AuthorityLack::Tasks => b"tasks",
                AuthorityLack::Depth => b"depth",
                AuthorityLack::Spend => b"spend",
            },
        );
    }
    put(&mut words, b"; release it: task release ");
    put(&mut words, skein_lib::Decimal::of(why.recovery.task).as_bytes());
    put(&mut words, b" --hold ");
    put(&mut words, skein_lib::Decimal::of(why.recovery.hold).as_bytes());
    put(&mut words, b" (it holds again unless it has more); or cancel it: task cancel ");
    put(&mut words, skein_lib::Decimal::of(why.recovery.task).as_bytes());
    put(&mut words, b" REASON");
    words.into_boxed()
}

fn put(words: &mut List<u8>, bytes: &[u8]) {
    for byte in bytes {
        words.push(*byte).expect("authority hold fits its fixed bound");
    }
}

fn missing(standing: &Heard, delegate: Delegate) -> Option<AuthorityHold> {
    if let Some(why) = standing.lacking {
        return Some(why);
    }
    let spend = standing.spend?;
    let reserve = match delegate {
        Delegate::Gate { .. } => spend.review,
        Delegate::Produce | Delegate::Repair(_) | Delegate::Resolve { .. } => spend.coding,
    };
    (spend.left < reserve).then_some(AuthorityHold {
        recovery: spend.recovery,
        lack: AuthorityLack::Spend,
        had: spend.left,
        released: false,
    })
}
// A turn change judges every pushed head without reading CI (domain/landing.md, 3).
fn pushed(change: &Change, head: [u8; 32]) -> State {
    match change.way {
        Way::Queue => State::Checking { head },
        Way::Turn => State::Gating { head, asked: Box::new([]) },
    }
}

fn ready(mut change: Change, head: [u8; 32], ready_since: Wall, facts: &Facts, now: Wall, limits: &Limits) -> Stepped {
    if facts.branch != Some(head) {
        return hold(change, Hold::BranchMoved);
    }
    match facts.ask {
        None => Stepped { change, decision: Decision::Wait { until: None } },
        Some(Ask::Prepare { head: expected, main }) => {
            if head != expected {
                return hold(change, Hold::BranchMoved);
            }
            if change.updates >= limits.updates {
                return hold(change, Hold::Updates);
            }
            change.updates = change.updates.checked_add(1).expect("update count below limit");
            change.state = State::Preparing { from: head, main, ready_since };
            change.since = now;
            Stepped { change, decision: Decision::Effect(Effect::Prepare { head, main }) }
        }
        Some(Ask::Repair { gate, candidate: _ }) => repair_due(change, Repair::Landing { gate }, facts, now, limits),
    }
}

#[expect(clippy::too_many_arguments, reason = "the pending prepare keeps both heads and the ready time")]
fn preparing(
    mut change: Change,
    from: [u8; 32],
    main: [u8; 32],
    ready_since: Wall,
    facts: &Facts,
    standing: &Heard,
    now: Wall,
    limits: &Limits,
) -> Stepped {
    match standing.effect {
        EffectResult::Conflict => resolving_due(change, main, now, limits),
        EffectResult::Failed => hold(change, Hold::Failed),
        EffectResult::Made => {
            let Some(head) = facts.branch else { return waiting(change, now, limits) };
            if facts.contains_base != Status::Passed || facts.base_tip != main {
                return waiting(change, now, limits);
            }
            if head != from {
                if change.clean.len() >= usize::try_from(limits.clean_heads).expect("u32 fits usize") {
                    return hold(change, Hold::Updates);
                }
                let mut clean = List::with_capacity(limits.clean_heads);
                for predecessor in &change.clean {
                    clean.push(*predecessor).expect("clean history admitted");
                }
                clean.push(from).expect("clean history has room");
                change.clean = clean.into_boxed();
            }
            change.last_head = Some(head);
            change.ready_since = Some(ready_since);
            change.since = now;
            change.state = State::Gating { head, asked: Box::new([]) };
            // Clean verdicts carry; exact verdicts are sought at the candidate (domain/landing.md, 3).
            gating(change, head, &[], facts, now, limits)
        }
        EffectResult::None | EffectResult::Pending => waiting(change, now, limits),
    }
}
