//! One level-triggered decision (domain/landing.md, 3–10).
use crate::{
    Decision, Facts, Flight, GateFacts, GateProof, Hold, Landing, Limits, Member, Phase, Posted, Posting, Project,
    Proof, Status, Stepped, Verdict,
};
use alloc::boxed::Box;
use skein_lib::{List, Wall};

/// Add a member after its upstreams while the landing can still grow
/// (domain/landing.md, 4.1–4.2). Existing candidates and proofs remain valid.
#[must_use]
pub fn join(mut landing: Landing, member: Member, after: &[u64]) -> Option<Landing> {
    match landing.phase {
        Phase::Waiting | Phase::Preparing | Phase::Proving | Phase::Proved { .. } => {}
        Phase::Turn { .. } | Phase::Landed | Phase::Cancelled | Phase::Held { .. } => return None,
    }
    for prior in &landing.members {
        if prior.change == member.change || prior.repository == member.repository {
            return None;
        }
    }
    let mut position = 0;
    for (index, prior) in landing.members.iter().enumerate() {
        if after.contains(&prior.change) {
            position = index + 1;
        }
    }
    let mut members = List::with_capacity(u32::try_from(landing.members.len() + 1).expect("bounded members"));
    for prior in landing.members.get(..position).expect("member position") {
        members.push(prior.clone()).expect("member room");
    }
    members.push(member).expect("new member room");
    for prior in landing.members.get(position..).expect("member position") {
        members.push(prior.clone()).expect("member room");
    }
    landing.members = members.into_boxed();
    landing.phase = Phase::Preparing;
    Some(landing)
}

/// Remove a cancelled member, retaining the remaining members' proofs
/// (domain/landing.md, 4.2, 5.1).
#[must_use]
pub fn leave(mut landing: Landing, change: u64) -> Landing {
    let mut members = List::with_capacity(u32::try_from(landing.members.len()).expect("bounded members"));
    for member in &landing.members {
        if member.change != change {
            members.push(member.clone()).expect("member room");
        }
    }
    landing.members = members.into_boxed();
    landing.phase = if landing.members.is_empty() { Phase::Cancelled } else { Phase::Preparing };
    landing
}

/// Decide against a coherent snapshot. The top commits the state and its
/// effect together; effect facts always describe the current preparation/try.
#[must_use]
pub fn step(landing: &Landing, facts: &Facts, project: &Project, limits: &Limits, now: Wall) -> Stepped {
    admitted(landing, facts, limits);
    let mut next = landing.clone();
    match &landing.phase {
        Phase::Landed | Phase::Cancelled => return done(next, Decision::None),
        Phase::Held { .. } => {
            if !facts.released {
                return wait(next, None);
            }
            next.phase = Phase::Preparing;
            for member in &mut next.members {
                for gate in &mut member.gates {
                    gate.faults = 0;
                }
            }
            return done(next, Decision::None);
        }
        Phase::Waiting | Phase::Preparing | Phase::Proving | Phase::Proved { .. } | Phase::Turn { .. } => {}
    }
    for member in &facts.members {
        if member.cancelled {
            return cancelled(next, facts, limits);
        }
    }
    match landing.phase {
        Phase::Waiting | Phase::Preparing | Phase::Proving => {
            for (member, fresh) in next.members.iter().zip(&facts.members) {
                if member.landed.is_none() && !fresh.ready {
                    next.phase = Phase::Waiting;
                    if let Some(decision) = failed_post(&mut next, facts) {
                        return done(next, decision);
                    }
                    return wait(next, None);
                }
            }
            if let Some(decision) = failed_post(&mut next, facts) {
                return done(next, decision);
            }
        }
        Phase::Proved { .. } | Phase::Turn { .. } | Phase::Landed | Phase::Held { .. } | Phase::Cancelled => {}
    }
    match landing.phase {
        Phase::Waiting => {
            next.phase = Phase::Preparing;
            done(next, Decision::None)
        }
        Phase::Preparing => preparing(next, facts),
        Phase::Proving => {
            for (member, fresh) in next.members.iter().zip(&facts.members) {
                if member.landed.is_none() && member.head != Some(fresh.head) {
                    next.phase = Phase::Preparing;
                    return done(next, Decision::None);
                }
            }
            proving(next, facts, *project, limits, now)
        }
        Phase::Proved { at } => {
            if facts.turn_free && facts.first {
                next.phase = Phase::Turn { at };
                done(next, Decision::None)
            } else {
                wait(next, None)
            }
        }
        Phase::Turn { .. } => turn(next, facts),
        Phase::Landed | Phase::Held { .. } | Phase::Cancelled => unreachable!("inactive phases returned above"),
    }
}

fn admitted(landing: &Landing, facts: &Facts, limits: &Limits) {
    assert!(landing.members.len() <= usize::try_from(limits.members).expect("u32 fits usize"), "members admitted");
    assert!(landing.members.len() <= usize::from(u16::MAX).saturating_add(1), "member positions fit u16");
    assert_eq!(landing.members.len(), facts.members.len(), "one fact per member");
    for (member, fresh) in landing.members.iter().zip(&facts.members) {
        assert!(member.gates.len() <= usize::try_from(limits.gates).expect("u32 fits usize"), "gates admitted");
        assert!(member.gates.len() <= usize::from(u16::MAX).saturating_add(1), "gate positions fit u16");
        assert_eq!(member.gates.len(), fresh.gates.len(), "one fact per gate");
    }
}

fn done(landing: Landing, decision: Decision) -> Stepped {
    Stepped { landing, decision }
}
fn wait(landing: Landing, until: Option<Wall>) -> Stepped {
    done(landing, Decision::Wait { until })
}

fn cancelled(mut landing: Landing, facts: &Facts, limits: &Limits) -> Stepped {
    let mut live = List::with_capacity(limits.members);
    for (member, fresh) in landing.members.iter().zip(&facts.members) {
        if !fresh.cancelled {
            live.push(member.clone()).expect("members admitted");
        }
    }
    landing.members = live.into_boxed();
    if landing.members.is_empty() {
        landing.phase = Phase::Cancelled;
        done(landing, Decision::Cancel)
    } else {
        landing.phase = Phase::Preparing;
        done(landing, Decision::None)
    }
}

fn preparing(mut landing: Landing, facts: &Facts) -> Stepped {
    let mut waiting = false;
    for (index, (member, fresh)) in landing.members.iter_mut().zip(&facts.members).enumerate() {
        if member.landed.is_some() {
            continue;
        }
        if member.candidate.is_some() && member.head == Some(fresh.head) && member.base == Some(fresh.main) {
            continue;
        }
        if fresh.asked {
            waiting = true;
            continue;
        }
        match fresh.contains_main {
            Status::Passed => {
                if member.asked.is_none() || member.base != Some(fresh.main) {
                    landing.preparation = landing.preparation.checked_add(1).expect("preparation fits u16");
                }
                member.head = Some(fresh.head);
                member.base = Some(fresh.main);
                member.candidate = Some(fresh.head);
                member.asked = None;
                for gate in &mut member.gates {
                    *gate = GateProof { proof: Proof::Unkeyed, posted: Posted::Nothing, faults: 0 };
                }
            }
            Status::Failed => {
                if member.asked.is_some() && member.head == Some(fresh.head) && member.base == Some(fresh.main) {
                    waiting = true;
                    continue;
                }
                landing.preparation = landing.preparation.checked_add(1).expect("preparation fits u16");
                member.asked = Some(landing.preparation);
                member.head = Some(fresh.head);
                member.base = Some(fresh.main);
                member.candidate = None;
                return done(
                    landing,
                    Decision::Prepare { member: position(index), head: fresh.head, main: fresh.main },
                );
            }
            Status::Unknown | Status::Pending => waiting = true,
        }
    }
    if waiting {
        return wait(landing, None);
    }
    landing.phase = Phase::Proving;
    done(landing, Decision::None)
}

fn position(index: usize) -> u16 {
    u16::try_from(index).expect("position admitted as u16")
}

// A repair may make a member unready before its failure status is posted.
// Posting that known receipt still belongs to its old candidate.
fn failed_post(landing: &mut Landing, facts: &Facts) -> Option<Decision> {
    for (index, (member, fresh)) in landing.members.iter_mut().zip(&facts.members).enumerate() {
        for (number, (gate, fact)) in member.gates.iter_mut().zip(&fresh.gates).enumerate() {
            match gate.proof {
                Proof::Failed { .. } if gate.posted != Posted::Verdict => {
                    if fact.posting == Status::Pending {
                        return Some(Decision::Wait { until: None });
                    }
                    gate.posted = Posted::Verdict;
                    return Some(Decision::Post {
                        member: position(index),
                        gate: position(number),
                        posting: Posting::Ran { passed: false },
                    });
                }
                Proof::Unkeyed
                | Proof::Keyed { .. }
                | Proof::Dispatched { .. }
                | Proof::Passed { .. }
                | Proof::Failed { .. } => {}
            }
        }
    }
    None
}

fn proving(mut landing: Landing, facts: &Facts, project: Project, limits: &Limits, now: Wall) -> Stepped {
    let mut blocked = false;
    let mut poll = false;
    for (index, (member, fresh)) in landing.members.iter_mut().zip(&facts.members).enumerate() {
        if member.landed.is_some() {
            continue;
        }
        let candidate = member.candidate.expect("proving has every candidate");
        for (number, (gate, fact)) in member.gates.iter_mut().zip(&fresh.gates).enumerate() {
            let decision = gate_step(gate, fact, position(index), position(number), candidate, project);
            match decision {
                Decision::None => {}
                Decision::Wait { .. } => {
                    if fact.blocking {
                        blocked = true;
                        match fact.flight {
                            Flight::Mine { .. } => poll = true,
                            Flight::None | Flight::Other => {}
                        }
                    }
                }
                Decision::Hold(why) => {
                    landing.phase = Phase::Held { was: Box::new(landing.phase.clone()), why };
                    return done(landing, decision);
                }
                Decision::Key { .. }
                | Decision::Dispatch { .. }
                | Decision::Retry { .. }
                | Decision::Post { .. }
                | Decision::Repair { .. } => return done(landing, decision),
                Decision::Prepare { .. }
                | Decision::Proved
                | Decision::Merge { .. }
                | Decision::Stale
                | Decision::Finish
                | Decision::Cancel => unreachable!("gate decisions only"),
            }
        }
    }
    if blocked {
        let until =
            if poll { Some(Wall::from_nanos(now.as_nanos().saturating_add(limits.poll.as_nanos()))) } else { None };
        return wait(landing, until);
    }
    landing.phase = Phase::Proved { at: now };
    done(landing, Decision::Proved)
}

fn gate_step(
    gate: &mut GateProof,
    fact: &GateFacts,
    member: u16,
    number: u16,
    candidate: crate::Commit,
    project: Project,
) -> Decision {
    let key = match gate.proof {
        Proof::Unkeyed => match fact.key {
            Some(key) => {
                gate.proof = Proof::Keyed { key };
                key
            }
            None => return Decision::Key { member, gate: number, candidate },
        },
        Proof::Keyed { key } | Proof::Dispatched { key, .. } | Proof::Passed { key } | Proof::Failed { key } => key,
    };
    match fact.receipt {
        Status::Passed => {
            let received = match gate.proof {
                Proof::Dispatched { .. } => false,
                Proof::Keyed { .. } | Proof::Passed { .. } | Proof::Failed { .. } | Proof::Unkeyed => {
                    gate.posted == Posted::Nothing
                }
            };
            gate.proof = Proof::Passed { key };
            if gate.posted != Posted::Verdict {
                if fact.posting == Status::Pending {
                    return Decision::Wait { until: None };
                }
                gate.posted = Posted::Verdict;
                return Decision::Post {
                    member,
                    gate: number,
                    posting: if received { Posting::Received } else { Posting::Ran { passed: true } },
                };
            }
            return Decision::None;
        }
        Status::Failed => {
            let failed = gate.proof == Proof::Failed { key };
            gate.proof = Proof::Failed { key };
            if !failed && fact.blocking {
                return Decision::Repair { member, gate: number };
            }
            if gate.posted != Posted::Verdict {
                if fact.posting == Status::Pending {
                    return Decision::Wait { until: None };
                }
                gate.posted = Posted::Verdict;
                return Decision::Post { member, gate: number, posting: Posting::Ran { passed: false } };
            }
            return Decision::Wait { until: None };
        }
        Status::Unknown | Status::Pending => {}
    }
    match gate.proof {
        Proof::Passed { .. } => return Decision::None,
        Proof::Failed { .. } => return Decision::Wait { until: None },
        Proof::Dispatched { .. } if gate.posted == Posted::Nothing => {
            gate.posted = Posted::Pending;
            return Decision::Post { member, gate: number, posting: Posting::Pending };
        }
        Proof::Unkeyed | Proof::Keyed { .. } | Proof::Dispatched { .. } => {}
    }
    match fact.flight {
        Flight::Other => Decision::Wait { until: None },
        Flight::None => match gate.proof {
            Proof::Keyed { .. } => {
                gate.proof = Proof::Dispatched { key, try_: 1 };
                Decision::Dispatch { member, gate: number, key, try_: 1 }
            }
            Proof::Unkeyed | Proof::Dispatched { .. } | Proof::Passed { .. } | Proof::Failed { .. } => {
                Decision::Wait { until: None }
            }
        },
        Flight::Mine { run: None | Some(Verdict::Running | Verdict::Passed | Verdict::Failed) } => {
            Decision::Wait { until: None }
        }
        Flight::Mine { run: Some(Verdict::Fault(_)) } => {
            if gate.faults >= project.gate_retries {
                if fact.blocking {
                    return Decision::Hold(Hold::Faults { member, gate: number });
                }
                return Decision::Wait { until: None };
            }
            let try_ = match gate.proof {
                Proof::Dispatched { try_, .. } => try_.checked_add(1).expect("gate try fits u16"),
                Proof::Unkeyed | Proof::Keyed { .. } | Proof::Passed { .. } | Proof::Failed { .. } => 1,
            };
            gate.faults = gate.faults.checked_add(1).expect("faults below retry limit");
            gate.proof = Proof::Dispatched { key, try_ };
            Decision::Retry { member, gate: number, key, try_ }
        }
    }
}

fn turn(mut landing: Landing, facts: &Facts) -> Stepped {
    // Record read-back successes before any stale exit (domain/landing.md, 8).
    for (member, fresh) in landing.members.iter_mut().zip(&facts.members) {
        if member.candidate.is_some() && fresh.merged == member.candidate {
            member.landed = member.candidate;
        }
    }
    for (index, (member, fresh)) in landing.members.iter_mut().zip(&facts.members).enumerate() {
        if member.landed.is_some() {
            continue;
        }
        let candidate = member.candidate.expect("turn has every candidate");
        if fresh.merged == Some(candidate) {
            member.landed = Some(candidate);
            continue;
        }
        if member.base != Some(fresh.main) || fresh.merge == Status::Failed {
            return stale(landing, facts);
        }
        return match fresh.merge {
            Status::Unknown => done(landing, Decision::Merge { member: position(index), candidate }),
            Status::Pending | Status::Passed => wait(landing, None),
            Status::Failed => unreachable!("refused merges returned stale above"),
        };
    }
    landing.phase = Phase::Landed;
    done(landing, Decision::Finish)
}

fn stale(mut landing: Landing, facts: &Facts) -> Stepped {
    landing.phase = Phase::Preparing;
    landing.stale_proofs = landing.stale_proofs.checked_add(1).expect("stale proof count fits u32");
    for (member, fresh) in landing.members.iter_mut().zip(&facts.members) {
        if member.landed.is_none() && (member.base != Some(fresh.main) || fresh.merge == Status::Failed) {
            member.candidate = None;
            member.asked = None;
            for gate in &mut member.gates {
                *gate = GateProof { proof: Proof::Unkeyed, posted: Posted::Nothing, faults: 0 };
            }
        }
    }
    done(landing, Decision::Stale)
}
