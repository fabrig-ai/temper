//! Durable per-member tier comments (domain/after-landing.md, 9; domain/forge.md, 6).
use crate::{Domain, Limits, Request, TierBranch, TierComment, TierRow};
use alloc::boxed::Box;
use skein_forge::api::Write;
use skein_lib::{Decimal, Env, List, Queue};
use temper_engine_domain_forge_tiers::{Blame, Failures};

pub(crate) fn capacity(l: &Limits) -> Option<u32> {
    let count = l.landings_kept.checked_mul(l.changes)?.checked_mul(2)?;
    if count > 4096 {
        return None;
    } // Family 25's derived member queue ceiling.
    Some(count)
}

/// Materialize the recipients before applying the verdict can release a change.
pub(crate) fn plan(d: &Domain, row: &TierRow, env: &Env<Limits>) -> Box<[TierComment]> {
    let mut comments = List::with_capacity(capacity(&env.limits).expect("admitted comment bound"));
    let Some(history) = d.landing_logs.get(&row.repository) else { return comments.into_boxed() };
    let result = row.result.as_ref().expect("judged result");
    let run = row.record.run.as_ref().expect("judged range");
    for entry in &history.landings {
        let mut blame = None;
        for (landing, judgement) in run.landings.iter().zip(&result.verdict.landings) {
            if landing.commit == entry.commit {
                blame = Some(judgement);
                break;
            }
        }
        if let Some(blame) = blame {
            for (_, pull) in &entry.changes {
                let part = u16::try_from(comments.len()).expect("member index bound");
                let key = crate::tiers::purpose(d, row, 8, part)
                    .key(&d.namespace, env.limits.client.op_bytes)
                    .expect("tier key");
                let bound = env.limits.client.op_bytes.saturating_sub(u32::try_from(key.len()).expect("key bound"));
                let body = words(&row.tier, result.run.number, blame, bound);
                comments
                    .push(TierComment { pull: *pull, operation: 8, part, body, state: TierBranch::Absent })
                    .expect("history member bound");
            }
        }
        let mut repairs = false;
        if let Some(verification) = d.verifications.get(&(row.repository, entry.commit)) {
            for tier in &verification.tiers {
                if tier.tier == row.tier
                    && tier.repaired.is_none()
                    && let Some(verdict) = &tier.verdict
                {
                    match &verdict.blame {
                        Blame::Cleared => {}
                        Blame::Owns(_) | Blame::Suspect(_) => {
                            repairs =
                                temper_engine_domain_forge_tiers::repaired(&verdict.blame, &result.verdict.failing);
                        }
                    }
                }
            }
        }
        for red in &row.reds {
            if red.landing == entry.commit
                && !red.repaired
                && temper_engine_domain_forge_tiers::repaired(&red.blame, &result.verdict.failing)
            {
                repairs = true;
            }
        }
        if !repairs {
            continue;
        }
        let mut repair = result.verdict.tip;
        for (landing, _) in run.landings.iter().zip(&result.verdict.landings) {
            if let Some(candidate) = d.verifications.get(&(row.repository, landing.commit))
                && candidate.repairs == Some(entry.commit)
            {
                repair = landing.commit;
            }
        }
        let mut pull = None;
        for candidate in &history.landings {
            if candidate.commit == repair {
                pull = candidate.changes.first().copied();
                break;
            }
        }
        let Some((_, repair_pull)) = pull else { continue };
        let body = crate::tiers::joined(&[b"temper: repaired by #", Decimal::of(repair_pull).as_bytes()]);
        for (_, member) in &entry.changes {
            let part = u16::try_from(comments.len()).expect("member index bound");
            comments
                .push(TierComment { pull: *member, operation: 9, part, body: body.clone(), state: TierBranch::Absent })
                .expect("history repair member bound");
        }
    }
    comments.into_boxed()
}

fn words(tier: &[u8], run: u64, blame: &Blame, bound: u32) -> Box<[u8]> {
    let prefix = crate::tiers::joined(&[b"temper: tier ", tier, b" run ", Decimal::of(run).as_bytes()]);
    let mut text = List::with_capacity(bound);
    append(&mut text, &prefix);
    match blame {
        Blame::Cleared => append(&mut text, b" cleared this landing"),
        Blame::Owns(Failures::Many) | Blame::Suspect(Failures::Many) => append(&mut text, b": red, failing many"),
        Blame::Owns(Failures::Tests(tests)) | Blame::Suspect(Failures::Tests(tests)) => {
            append(&mut text, b": red, failing ");
            let mut separator = b"".as_slice();
            for test in tests {
                let size = separator.len().checked_add(test.len()).expect("bounded test id");
                if size > usize::try_from(text.room()).expect("room fits") {
                    break;
                }
                append(&mut text, separator);
                append(&mut text, test);
                separator = b", ";
            }
        }
    }
    text.into_boxed()
}
fn append(text: &mut List<u8>, bytes: &[u8]) {
    for byte in bytes {
        if text.push(*byte).is_err() {
            break;
        }
    }
}

/// Return true until every comment is settled; cleanup follows comments.
pub(crate) fn step(d: &mut Domain, row: &mut TierRow, env: &Env<Limits>, entry: u64, out: &mut Queue<Request>) -> bool {
    let Some(result) = &mut row.result else { return false };
    if !result.applied {
        return false;
    }
    for comment in &mut result.comments {
        match comment.state {
            TierBranch::Made => {}
            TierBranch::Pending { .. } => {
                out.push(Request::TierDecision {
                    task: row.task.expect("tier task"),
                    decision: crate::TierDecision::Wait,
                });
                return true;
            }
            TierBranch::Absent => {
                let operation = comment.operation;
                let part = comment.part;
                let pull = comment.pull;
                let body = comment.body.clone();
                let key = crate::tiers::purpose(d, row, operation, part)
                    .key(&d.namespace, env.limits.client.op_bytes)
                    .expect("tier key");
                if d.entries.contains_key(&entry) || d.entries.len() == env.limits.entries {
                    out.push(Request::TierDecision {
                        task: row.task.expect("tier task"),
                        decision: crate::TierDecision::Held,
                    });
                    return true;
                }
                let comment = row
                    .result
                    .as_mut()
                    .expect("tier result")
                    .comments
                    .get_mut(usize::from(part))
                    .expect("comment index");
                comment.state = TierBranch::Pending { entry };
                crate::tiers::effect(d, row, entry, Write::Post { number: pull, key, body }, operation, part, out);
                return true;
            }
        }
    }
    false
}

pub(crate) fn pending(row: &TierRow, entry: u64) -> bool {
    let Some(result) = &row.result else { return false };
    for comment in &result.comments {
        if comment.state == (TierBranch::Pending { entry }) {
            return true;
        }
    }
    false
}
pub(crate) fn kind(d: &Domain, row: &TierRow, entry: &skein_forge_client::Entry) -> Option<(u16, crate::What)> {
    let result = row.result.as_ref()?;
    for comment in &result.comments {
        if comment.state != (TierBranch::Pending { entry: entry.number }) {
            continue;
        }
        let key = crate::tiers::purpose(d, row, comment.operation, comment.part)
            .key(&d.namespace, d.tier_config_limits.client.op_bytes)?;
        if entry.effect.write == (Write::Post { number: comment.pull, key, body: comment.body.clone() }) {
            return Some((7, crate::What::Pull(comment.pull)));
        }
    }
    None
}
