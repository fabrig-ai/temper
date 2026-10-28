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
        .push(Logged {
            trailers: Box::new([]),
            cut: false,
            commit: after,
            seen,
            changes: Box::new([]),
            verdicts: marks.into_boxed(),
        })
        .expect("one new ring entry");
    let pending = match d.landing_logs.get(&repository) {
        Some(row) => row.pending.clone(),
        None => Box::new([]),
    };
    let row = LandingLog { pending, repository, landings: landings.into_boxed() };
    d.landing_logs.insert(repository, row.clone()).expect("one log per adopted repository");
    save(row, out);
}

/// Temper's own merge of `change` at `commit` (by queue, or a turn's member).
pub(crate) fn landed(
    d: &mut Domain,
    repository: Repository,
    commit: Commit,
    (change, pull): (u64, u64),
    seen: Wall,
    out: &mut Queue<Request>,
) {
    moved(d, repository, commit, seen, out);
    let Some(row) = d.landing_logs.get_mut(&repository) else { return };
    for entry in &mut row.landings {
        if entry.commit != commit {
            continue;
        }
        let mut known = false;
        for (task, _) in &entry.changes {
            if *task == change {
                known = true;
                break;
            }
        }
        if known {
            continue;
        }
        let mut changes = List::with_capacity(d.tier_config_limits.changes);
        for saved in &entry.changes {
            changes.push(*saved).expect("admitted changes");
        }
        changes.push((change, pull)).expect("changes bounded as Verification.changes");
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
        for trailer in &entry.trailers {
            bytes = bytes.checked_add(u64::try_from(trailer.len()).ok()?)?;
        }
        bytes = bytes.checked_add(u64::try_from(size_of_val(entry.trailers.as_ref())).ok()?)?;
        for mark in &entry.verdicts {
            bytes = bytes.checked_add(u64::try_from(mark.tier.len()).ok()?)?;
        }
    }
    for read in &row.pending {
        bytes = bytes.checked_add(pending_bytes(read)?)?;
    }
    Some(bytes)
}

pub(crate) fn worst_case(l: &Limits) -> Option<u64> {
    let marks = u64::from(l.tiers).checked_mul(
        u64::try_from(size_of::<Mark>()).ok()?.checked_add(u64::try_from(crate::CHECK_NAME_BYTES).ok()?)?,
    )?;
    let entry = u64::try_from(size_of::<Logged>())
        .ok()?
        .checked_add(u64::from(l.changes).checked_mul(16)?)?
        .checked_add(marks)?
        .checked_add(u64::from(l.trailers).checked_mul(u64::from(l.trailer_bytes).checked_add(16)?)?)?;
    // A replacement ring is built beside the retained row before insertion;
    // emitted Save copies are charged by the root's output envelope.
    let rings =
        u64::from(l.repositories).checked_add(1)?.checked_mul(u64::from(l.landings_kept))?.checked_mul(entry)?;
    let pending = u64::from(l.landings.max(l.client.resources))
        .checked_mul(
            u64::try_from(size_of::<crate::LandingNews>())
                .ok()?
                .checked_add(crate::limits::pending_landing_payload(l)?)?,
        )?
        .checked_mul(u64::from(l.repositories).checked_add(2)?)?;
    Map::<Repository, LandingLog>::worst_case(l.repositories)?.checked_add(rings)?.checked_add(pending)
}

pub(crate) fn seen(d: &Domain, repository: Repository, commit: Commit) -> Option<Wall> {
    for entry in &d.landing_logs.get(&repository)?.landings {
        if entry.commit == commit {
            return Some(entry.seen);
        }
    }
    None
}

/// Save trailer facts on the retained entry without replacing its member/verdict facts.
pub(crate) fn trailers(
    d: &mut Domain,
    topic: &crate::Topic,
    commit: Commit,
    trailers: &[Box<[u8]>],
    cut: bool,
    out: &mut Queue<Request>,
) {
    let (repository, branch) = match topic {
        crate::Topic::Landings { repository, branch } => (repository, branch),
        crate::Topic::Ci { .. } | crate::Topic::Pull { .. } | crate::Topic::Participation { .. } => {
            unreachable!("landing topic")
        }
    };
    let repository = *repository;
    let is_main = match d.repositories.get(&repository) {
        Some(row) => row.settings.default_branch == *branch,
        None => false,
    };
    let Some(row) = d.landing_logs.get_mut(&repository) else { return };
    let mut pending = List::with_capacity(u32::try_from(row.pending.len()).expect("pending count"));
    for read in &row.pending {
        if read.after != commit || read.topic != *topic {
            pending.push(read.clone()).expect("removes finished range");
        }
    }
    row.pending = pending.into_boxed();
    for entry in &mut row.landings {
        if is_main && entry.commit == commit {
            entry.trailers = Box::from(trailers);
            entry.cut = cut;
            save(row.clone(), out);
            return;
        }
    }
    save(row.clone(), out);
}

/// Record the read phase with each next request; its reply may be lost at a stop.
pub(crate) fn pending(d: &mut Domain, read: &crate::LandingNews, out: &mut Queue<Request>) {
    let repository = match &read.topic {
        crate::Topic::Landings { repository, .. } => repository,
        crate::Topic::Ci { .. } | crate::Topic::Pull { .. } | crate::Topic::Participation { .. } => {
            unreachable!("landing topic")
        }
    };
    let repository = *repository;
    if !d.landing_logs.contains_key(&repository) {
        d.landing_logs
            .insert(repository, LandingLog { repository, pending: Box::new([]), landings: Box::new([]) })
            .expect("one log per repository");
    }
    let row = d.landing_logs.get_mut(&repository).expect("landing log");
    let mut pending = List::with_capacity(d.tier_config_limits.landings.max(d.tier_config_limits.client.resources));
    for previous in &row.pending {
        if previous.topic != read.topic || previous.after != read.after {
            pending.push(previous.clone()).expect("pending bound");
        }
    }
    pending.push(read.clone()).expect("pending slot admitted by the top");
    row.pending = pending.into_boxed();
    save(row.clone(), out);
}
fn pending_bytes(row: &crate::LandingNews) -> Option<u64> {
    let mut bytes = u64::try_from(size_of::<crate::LandingNews>()).ok()?;
    match &row.topic {
        crate::Topic::Landings { branch, .. } => bytes = bytes.checked_add(u64::try_from(branch.len()).ok()?)?,
        crate::Topic::Ci { .. } | crate::Topic::Pull { .. } | crate::Topic::Participation { .. } => {
            unreachable!("landing topic")
        }
    }
    if let Some(files) = &row.files {
        bytes = bytes.checked_add(u64::try_from(size_of_val(files.as_ref())).ok()?)?;
        for file in files {
            bytes = bytes.checked_add(u64::try_from(file.len()).ok()?)?;
        }
    }
    bytes = bytes.checked_add(u64::try_from(size_of_val(row.trailers.as_ref())).ok()?)?;
    for trailer in &row.trailers {
        bytes = bytes.checked_add(u64::try_from(trailer.len()).ok()?)?;
    }
    Some(bytes)
}
