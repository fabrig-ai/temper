//! Landing verification retained until its changes release (domain/after-landing.md, 5.3, 6 and 10).
use alloc::boxed::Box;
use core::mem::size_of;
use skein_forge::api::{Commit, Repository};
use skein_lib::{List, Map, Queue};
use temper_engine_domain_forge_change as change;
use temper_engine_domain_forge_tiers::{Blame, Failures};

use crate::{Domain, Key, Limits, RepositoryTiers, Request, Stored, Tier, TierVerdict, TierVerification, Verification};

impl Domain {
    /// Replace a project's tiers, refusing duplicate repositories, invalid or duplicate names and bounds.
    pub fn project_tiers(
        &mut self,
        project: u32,
        settings: crate::TierSettings,
        repositories: Box<[RepositoryTiers]>,
    ) -> bool {
        let (repository_limit, tier_limit) = self.tier_limits;
        if repositories.len() > usize::try_from(repository_limit).expect("u32 fits usize") {
            return false;
        }
        for (index, repository) in repositories.iter().enumerate() {
            if repository.tiers.len() > usize::try_from(tier_limit).expect("u32 fits usize") {
                return false;
            }
            for previous in repositories.get(..index).expect("enumerated repository prefix") {
                if previous.repository == repository.repository {
                    return false;
                }
            }
            for (index, tier) in repository.tiers.iter().enumerate() {
                if !valid_name(&tier.name) || !crate::tiers::valid(tier, &self.tier_config_limits) {
                    return false;
                }
                for previous in repository.tiers.get(..index).expect("enumerated tier prefix") {
                    if previous.name == tier.name {
                        return false;
                    }
                }
            }
        }
        if self.tiers.insert(project, repositories).is_err() {
            return false;
        }
        self.tier_settings.insert(project, settings).expect("same project bound");
        true
    }
}

pub(crate) fn valid_name(name: &[u8]) -> bool {
    if name.is_empty() || name.len() > crate::CHECK_NAME_BYTES {
        return false;
    }
    for byte in name {
        if !byte.is_ascii_alphanumeric() && *byte != b'-' && *byte != b'_' {
            return false;
        }
    }
    true
}

pub(crate) fn tiers(d: &Domain, repository: Repository) -> &[Tier] {
    let Some(adopted) = d.repositories.get(&repository) else { return &[] };
    let Some(repositories) = d.tiers.get(&adopted.project) else { return &[] };
    for configured in repositories {
        if configured.repository == repository {
            return &configured.tiers;
        }
    }
    &[]
}

/// Save verifying tiers beside the Landed row in the merge's output.
pub(crate) fn landed(d: &mut Domain, repository: Repository, commit: Commit, task: u64, out: &mut Queue<Request>) {
    if d.verifications.contains_key(&(repository, commit)) {
        // Found-again merge outcomes cannot reset a verdict already judged.
        return;
    }
    let configured = tiers(d, repository);
    if configured.is_empty() {
        return;
    }
    let mut tiers = List::with_capacity(u32::try_from(configured.len()).expect("admitted tiers"));
    for tier in configured {
        tiers.push(TierVerification { tier: tier.name.clone(), verdict: None, repaired: None }).expect("tier bound");
    }
    let row = Verification { repository, commit, changes: Box::new([task]), tiers: tiers.into_boxed() };
    d.verifications.insert((repository, commit), row.clone()).expect("one landing per live change");
    save(row, out);
}

pub(crate) fn verification(d: &Domain, repository: Repository, commit: Commit) -> change::Verification {
    match d.verifications.get(&(repository, commit)) {
        Some(row) => fact(row),
        None => change::Verification::Cleared,
    }
}

fn fact(row: &Verification) -> change::Verification {
    let mut cleared = true;
    for tier in &row.tiers {
        if tier.repaired.is_some() {
            continue;
        }
        match &tier.verdict {
            Some(TierVerdict { blame: Blame::Owns(_) | Blame::Suspect(_), .. }) => return change::Verification::Red,
            Some(TierVerdict { blame: Blame::Cleared, .. }) => {}
            None => cleared = false,
        }
    }
    if cleared { change::Verification::Cleared } else { change::Verification::Verifying }
}

/// Apply blame and repair older reds once this judged run passes their tests (5.3).
pub(crate) fn judged(
    d: &mut Domain,
    repository: Repository,
    tier: &[u8],
    judged: &temper_engine_domain_forge_tiers::Verdict,
    landings: &[(Commit, Blame)],
    out: &mut Queue<Request>,
) {
    // Earlier reds keep their own verdict. Only a judged run that no longer
    // fails any of their blamed tests repairs them, whether in this range or before it.
    let mut keys = List::with_capacity(d.verifications.len());
    for (key, _) in &d.verifications {
        if key.0 == repository { keys.push(*key).expect("verification count"); }
    }
    for key in &keys {
        let row = d.verifications.get_mut(key).expect("collected verification");
        let before = fact(row);
        let mut changed = false;
        for verification in &mut row.tiers {
            if verification.tier.as_ref() == tier && verification.repaired.is_none()
                && let Some(verdict) = &verification.verdict
                && let Blame::Owns(_) | Blame::Suspect(_) = &verdict.blame
                && temper_engine_domain_forge_tiers::repaired(&verdict.blame, &judged.failing)
            {
                verification.repaired = Some(judged.tip);
                changed = true;
            }
        }
        if changed {
            if fact(row) != before {
                for task in &row.changes {
                    d.procedure_wakes.insert(*task, true).expect("one pending wake per change");
                }
            }
            save(row.clone(), out);
        }
    }
    for (commit, blame) in landings {
        let Some(row) = d.verifications.get_mut(&(repository, *commit)) else { continue };
        let before = fact(row);
        let mut changed = false;
        for verification in &mut row.tiers {
            if verification.tier.as_ref() == tier {
                if let Some(TierVerdict { blame: Blame::Owns(_) | Blame::Suspect(_), .. }) = &verification.verdict {
                    continue;
                }
                let verdict = TierVerdict { run: judged.number, blame: blame.clone() };
                if verification.verdict.as_ref() != Some(&verdict) {
                    verification.verdict = Some(verdict);
                    changed = true;
                }
            }
        }
        if changed {
            let after = fact(row);
            if before != after {
                for task in &row.changes {
                    d.procedure_wakes.insert(*task, true).expect("one pending wake per change");
                }
            }
            save(row.clone(), out);
        }
    }
}

pub(crate) fn released(d: &mut Domain, task: u64, out: &mut Queue<Request>) {
    let mut found = None;
    for (key, row) in &d.verifications {
        if row.changes.contains(&task) {
            found = Some(*key);
            break;
        }
    }
    let Some(key) = found else { return };
    let mut row = d.verifications.remove(&key).expect("found verification");
    if row.changes.len() == 1 {
        out.push(Request::Erase { key: Key::Verification { repository: row.repository, commit: row.commit } });
    } else {
        let mut changes = List::with_capacity(u32::try_from(row.changes.len()).expect("admitted changes"));
        for change in &row.changes {
            if *change != task {
                changes.push(*change).expect("removes one change");
            }
        }
        row.changes = changes.into_boxed();
        d.verifications.insert(key, row.clone()).expect("replaces verification");
        save(row, out);
    }
}

fn save(row: Verification, out: &mut Queue<Request>) {
    out.push(Request::Save { read_afresh: None, release: None, record: Stored::Verification(row) });
}

pub(crate) fn stored_bytes(row: &Verification) -> Option<u64> {
    let mut bytes = u64::try_from(row.changes.len()).ok()?.checked_mul(8)?.checked_add(
        u64::try_from(row.tiers.len()).ok()?.checked_mul(u64::try_from(size_of::<TierVerification>()).ok()?)?,
    )?;
    for tier in &row.tiers {
        bytes = bytes.checked_add(u64::try_from(tier.tier.len()).ok()?)?;
        match &tier.verdict {
            Some(TierVerdict {
                blame: Blame::Owns(Failures::Tests(tests)) | Blame::Suspect(Failures::Tests(tests)),
                ..
            }) => {
                bytes = bytes.checked_add(
                    u64::try_from(tests.len()).ok()?.checked_mul(u64::try_from(size_of::<Box<[u8]>>()).ok()?)?,
                )?;
                for test in tests {
                    bytes = bytes.checked_add(u64::try_from(test.len()).ok()?)?;
                }
            }
            Some(TierVerdict {
                blame: Blame::Cleared | Blame::Owns(Failures::Many) | Blame::Suspect(Failures::Many),
                ..
            })
            | None => {}
        }
    }
    Some(bytes)
}

pub(crate) fn worst_case(l: &Limits) -> Option<u64> {
    let tests = u64::from(l.failing_tests)
        .checked_mul(u64::from(l.test_id_bytes).checked_add(u64::try_from(size_of::<Box<[u8]>>()).ok()?)?)?;
    let tiers = u64::from(l.tiers).checked_mul(
        u64::try_from(size_of::<TierVerification>())
            .ok()?
            .checked_add(u64::try_from(crate::CHECK_NAME_BYTES).ok()?)?
            .checked_add(tests)?,
    )?;
    let path = u64::from(l.name_bytes).checked_add(u64::try_from(size_of::<Box<[u8]>>()).ok()?)?;
    let tier = u64::try_from(size_of::<Tier>())
        .ok()?
        .checked_add(u64::try_from(crate::CHECK_NAME_BYTES).ok()?.checked_mul(2)?)?
        .checked_add(u64::try_from(crate::WORKFLOW_BYTES).ok()?)?
        .checked_add(u64::from(l.ignored_paths).checked_mul(path)?)?;
    let configured = u64::from(l.repositories).checked_mul(
        u64::try_from(size_of::<RepositoryTiers>()).ok()?.checked_add(u64::from(l.tiers).checked_mul(tier)?)?,
    )?;
    Map::<(Repository, Commit), Verification>::worst_case(l.changes)?
        .checked_add(u64::from(l.changes).checked_mul(tiers.checked_add(8)?)?)?
        .checked_add(Map::<u32, Box<[RepositoryTiers]>>::worst_case(l.judge_projects)?)?
        .checked_add(u64::from(l.judge_projects).checked_mul(configured)?)?
        .checked_add(Map::<u32, crate::TierSettings>::worst_case(l.judge_projects)?)
}
