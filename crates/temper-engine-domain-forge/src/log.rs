//! Durable landing history, independent of changes and tier range retention
//! (domain/after-landing.md, 8 and 10).
use crate::{Domain, LandingLog, Limits, Logged, Mark, Request, Stored, Verdict};
use alloc::boxed::Box;
use core::mem::{size_of, size_of_val};
use skein_forge::api::{Commit, Repository};
use skein_lib::{List, Map, Queue, Wall};
use temper_engine_domain_forge_tiers::{Blame, Failures};

/// A move of an adopted repository's landing branch, as the engine saw it.
pub(crate) fn moved(d: &mut Domain, repository: Repository, after: Commit, seen: Wall, out: &mut Queue<Request>) {
    if !d.repositories.contains_key(&repository) {
        return;
    }
    if let Some(row) = d.landing_logs.get(&repository) {
        for entry in &row.landings {
            if entry.commit == after {
                return;
            }
        }
    }
    let configured = crate::verification::tiers(d, repository);
    let mut marks = List::with_capacity(u32::try_from(configured.len()).expect("admitted tiers"));
    for tier in configured {
        marks.push(Mark { tier: tier.name.clone(), verdict: Verdict::Verifying }).expect("tier bound");
    }
    let mut landings = List::with_capacity(d.tier_config_limits.landings_kept);
    if let Some(row) = d.landing_logs.get(&repository) {
        let full = row.landings.len() == usize::try_from(d.tier_config_limits.landings_kept).expect("u32 fits");
        for (index, entry) in row.landings.iter().enumerate() {
            if !full || index != 0 {
                landings.push(entry.clone()).expect("ring drops oldest");
            }
        }
    }
    landings
        .push(Logged { commit: after, seen, changes: Box::new([]), verdicts: marks.into_boxed() })
        .expect("one new ring entry");
    let row = LandingLog { repository, landings: landings.into_boxed() };
    d.landing_logs.insert(repository, row.clone()).expect("one log per adopted repository");
    save(row, out);
}

/// Temper's own merge of `change` at `commit` (by queue, or a turn's member).
pub(crate) fn landed(
    d: &mut Domain,
    repository: Repository,
    commit: Commit,
    change: u64,
    seen: Wall,
    out: &mut Queue<Request>,
) {
    moved(d, repository, commit, seen, out);
    let Some(row) = d.landing_logs.get_mut(&repository) else { return };
    for entry in &mut row.landings {
        if entry.commit != commit || entry.changes.contains(&change) {
            continue;
        }
        let mut changes = List::with_capacity(d.tier_config_limits.changes);
        for saved in &entry.changes {
            changes.push(*saved).expect("admitted changes");
        }
        changes.push(change).expect("changes bounded as Verification.changes");
        entry.changes = changes.into_boxed();
        save(row.clone(), out);
        return;
    }
}

/// A judged run's blames, beside `verification::judged`.
pub(crate) fn judged(
    d: &mut Domain,
    repository: Repository,
    tier: &[u8],
    landings: &[(Commit, Blame)],
    out: &mut Queue<Request>,
) {
    let Some(row) = d.landing_logs.get_mut(&repository) else { return };
    let mut start = 0;
    let mut changed = false;
    for (commit, blame) in landings {
        let mut end = None;
        for (index, entry) in row.landings.iter().enumerate() {
            if entry.commit == *commit {
                end = Some(index);
                break;
            }
        }
        let Some(end) = end else { continue };
        let verdict = verdict(blame);
        // Compaction replaces adjacent outside landings by their later tip.
        // Their still-verifying marks share that tip's blame. Previously judged
        // entries precede the run and retain their own judgement.
        for (index, entry) in row.landings.iter_mut().enumerate() {
            if index < start || index > end {
                continue;
            }
            for mark in &mut entry.verdicts {
                if mark.tier.as_ref() == tier
                    && (index == end || mark.verdict == Verdict::Verifying)
                    && mark.verdict != verdict
                {
                    mark.verdict = verdict;
                    changed = true;
                }
            }
        }
        start = end.checked_add(1).expect("bounded ring");
    }
    if changed {
        save(row.clone(), out);
    }
}

fn verdict(blame: &Blame) -> Verdict {
    match blame {
        Blame::Cleared => Verdict::Cleared,
        Blame::Owns(Failures::Many) | Blame::Suspect(Failures::Many) => Verdict::Red { tests: None },
        Blame::Owns(Failures::Tests(tests)) | Blame::Suspect(Failures::Tests(tests)) => {
            Verdict::Red { tests: Some(u32::try_from(tests.len()).expect("bounded test ids")) }
        }
    }
}

fn save(row: LandingLog, out: &mut Queue<Request>) {
    out.push(Request::Save { read_afresh: None, release: None, record: Stored::LandingLog(row) });
}

pub(crate) fn stored_bytes(row: &LandingLog) -> Option<u64> {
    let mut bytes = u64::try_from(size_of_val(row.landings.as_ref())).ok()?;
    for entry in &row.landings {
        bytes = bytes
            .checked_add(u64::try_from(size_of_val(entry.changes.as_ref())).ok()?)?
            .checked_add(u64::try_from(size_of_val(entry.verdicts.as_ref())).ok()?)?;
        for mark in &entry.verdicts {
            bytes = bytes.checked_add(u64::try_from(mark.tier.len()).ok()?)?;
        }
    }
    Some(bytes)
}

pub(crate) fn worst_case(l: &Limits) -> Option<u64> {
    let marks = u64::from(l.tiers).checked_mul(
        u64::try_from(size_of::<Mark>()).ok()?.checked_add(u64::try_from(crate::CHECK_NAME_BYTES).ok()?)?,
    )?;
    let entry = u64::try_from(size_of::<Logged>())
        .ok()?
        .checked_add(u64::from(l.changes).checked_mul(8)?)?
        .checked_add(marks)?;
    // A replacement ring is built beside the retained row before insertion;
    // emitted Save copies are charged by the root's output envelope.
    let rings =
        u64::from(l.repositories).checked_add(1)?.checked_mul(u64::from(l.landings_kept))?.checked_mul(entry)?;
    Map::<Repository, LandingLog>::worst_case(l.repositories)?.checked_add(rings)
}
