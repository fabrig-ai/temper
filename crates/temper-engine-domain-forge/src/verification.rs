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
        tiers
            .push(TierVerification { told: Box::new([]), tier: tier.name.clone(), verdict: None, repaired: None })
            .expect("tier bound");
    }
    let row = Verification {
        repairs: match d.changes.get(&task) {
            Some(row) => row.repairing,
            None => None,
        },
        repository,
        commit,
        changes: Box::new([task]),
        tiers: tiers.into_boxed(),
    };
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
        if key.0 == repository {
            keys.push(*key).expect("verification count");
        }
    }
    for key in &keys {
        let mut repair = judged.tip;
        for (commit, _) in landings {
            if let Some(candidate) = d.verifications.get(&(repository, *commit))
                && candidate.repairs == Some(key.1)
            {
                repair = *commit;
            }
        }
        let row = d.verifications.get_mut(key).expect("collected verification");
        let before = fact(row);
        let mut changed = false;
        for verification in &mut row.tiers {
            if verification.tier.as_ref() == tier
                && verification.repaired.is_none()
                && let Some(verdict) = &verification.verdict
                && red(&verdict.blame)
                && temper_engine_domain_forge_tiers::repaired(&verdict.blame, &judged.failing)
            {
                verification.repaired = Some(repair);
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
                let was_red = match &verification.verdict {
                    Some(verdict) => red(&verdict.blame),
                    None => false,
                };
                if was_red {
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

fn red(blame: &Blame) -> bool {
    match blame {
        Blame::Cleared => false,
        Blame::Owns(_) | Blame::Suspect(_) => true,
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
        bytes = bytes
            .checked_add(u64::try_from(tier.tier.len()).ok()?)?
            .checked_add(u64::try_from(tier.told.len()).ok()?.checked_mul(8)?)?;
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
            .checked_add(tests)?
            .checked_add(u64::from(l.subscriptions).checked_mul(8)?)?,
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
    // Repair scans retain one key per live verification while updating rows.
    let repair_keys = u64::from(l.changes).checked_mul(u64::try_from(size_of::<(Repository, Commit)>()).ok()?)?;
    Map::<(Repository, Commit), Verification>::worst_case(l.changes)?
        .checked_add(repair_keys)?
        .checked_add(u64::from(l.changes).checked_mul(tiers.checked_add(8)?)?)?
        .checked_add(Map::<u32, Box<[RepositoryTiers]>>::worst_case(l.judge_projects)?)?
        .checked_add(u64::from(l.judge_projects).checked_mul(configured)?)?
        .checked_add(Map::<u32, crate::TierSettings>::worst_case(l.judge_projects)?)
}

/// A goal subscription is eligible only for its own landing's members.
fn hears(subscriber: &crate::Subscriber, row: &Verification) -> bool {
    match &subscriber.topic {
        crate::Topic::Landings { repository, .. } if *repository == row.repository => {}
        crate::Topic::Landings { .. }
        | crate::Topic::Ci { .. }
        | crate::Topic::Pull { .. }
        | crate::Topic::Participation { .. } => return false,
    }
    let Some(tasks) = &subscriber.goal_tasks else { return false };
    for task in &row.changes {
        if tasks.contains(task) {
            return true;
        }
    }
    false
}

pub(crate) fn untold(d: &Domain, repository: Repository, tier: &[u8], run: u64) -> bool {
    for (_, row) in &d.verifications {
        if row.repository != repository {
            continue;
        }
        for verification in &row.tiers {
            if verification.tier.as_ref() != tier {
                continue;
            }
            if let Some(verdict) = &verification.verdict
                && verdict.run == run
                && red(&verdict.blame)
            {
                for (_, subscriber) in &d.subscriptions {
                    if hears(subscriber, row) && !verification.told.contains(&subscriber.task) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// One news and its durable told mark share a root decision. Keeping one
/// recipient per step also bounds output independently of the landing range.
pub(crate) fn tell(
    d: &mut Domain,
    repository: Repository,
    tier: &[u8],
    number: u64,
    run: &crate::ReceiptRun,
    log: &[u8],
    out: &mut Queue<Request>,
) {
    let mut found = None;
    for (key, row) in &d.verifications {
        if row.repository != repository {
            continue;
        }
        for (index, verification) in row.tiers.iter().enumerate() {
            if verification.tier.as_ref() != tier {
                continue;
            }
            if let Some(verdict) = &verification.verdict
                && verdict.run == number
                && red(&verdict.blame)
            {
                for (_, subscriber) in &d.subscriptions {
                    if hears(subscriber, row) && !verification.told.contains(&subscriber.task) {
                        found = Some((*key, index, subscriber.clone()));
                        break;
                    }
                }
            }
            if found.is_some() {
                break;
            }
        }
        if found.is_some() {
            break;
        }
    }
    let Some((key, index, subscriber)) = found else { return };
    let row = d.verifications.get_mut(&key).expect("untold verification");
    let verification = row.tiers.get_mut(index).expect("matching tier");
    let Some(TierVerdict { blame: Blame::Owns(failing) | Blame::Suspect(failing), .. }) = &verification.verdict else {
        unreachable!("red recipient")
    };
    out.push(Request::News {
        task: subscriber.task,
        subscription: subscriber.number,
        topic: subscriber.topic,
        news: crate::News::Red {
            tier: verification.tier.clone(),
            landing: row.commit,
            failing: failing.clone(),
            run: run.clone(),
            log: Box::from(log),
        },
        class: crate::Class::Wakes,
    });
    let mut told = List::with_capacity(
        u32::try_from(verification.told.len()).expect("bounded goals").checked_add(1).expect("one new goal"),
    );
    for goal in &verification.told {
        told.push(*goal).expect("previous goals");
    }
    told.push(subscriber.task).expect("one new goal");
    verification.told = told.into_boxed();
    save(row.clone(), out);
}

impl Domain {
    /// A landing's retained verdicts, for its change's final result.
    #[must_use]
    pub fn landing_verification(&self, repository: Repository, commit: Commit) -> Option<&Verification> {
        self.verifications.get(&(repository, commit))
    }
}

/// Supply the project's verification clock with the change's other limits.
pub(crate) fn change_limits(d: &Domain, repository: Repository, mut limits: change::Limits) -> change::Limits {
    if let Some(adopted) = d.repositories.get(&repository)
        && let Some(settings) = d.tier_settings.get(&adopted.project)
    {
        limits.verification_bound = settings.bound;
    }
    limits
}

/// A late verdict uses the log after the last verification member closes.
/// One recipient at a time keeps news and its durable acknowledgement atomic.
pub(crate) fn late_recipient(d: &Domain, row: &crate::TierRow) -> Option<(Commit, crate::Subscriber, Blame)> {
    let result = row.result.as_ref()?;
    let range = &row.record.run.as_ref()?.landings;
    let history = d.landing_logs.get(&row.repository)?;
    for (landing, blame) in range.iter().zip(&result.verdict.landings) {
        if d.verifications.contains_key(&(row.repository, landing.commit)) {
            continue;
        }
        for entry in &history.landings {
            if entry.commit != landing.commit {
                continue;
            }
            for (_, subscriber) in &d.subscriptions {
                if !hears_members(subscriber, row.repository, &entry.changes) {
                    continue;
                }
                let mut told = false;
                for mark in &result.told {
                    if mark.landing == landing.commit && mark.goal == subscriber.task {
                        told = true;
                    }
                }
                if !told {
                    return Some((landing.commit, subscriber.clone(), blame.clone()));
                }
            }
        }
    }
    None
}
fn hears_members(subscriber: &crate::Subscriber, repository: Repository, members: &[(u64, u64)]) -> bool {
    if let crate::Topic::Landings { repository: subscribed, .. } = subscriber.topic {
        if subscribed != repository {
            return false;
        }
    } else {
        return false;
    }
    let Some(tasks) = &subscriber.goal_tasks else {
        return false;
    };
    for (task, _) in members {
        if tasks.contains(task) {
            return true;
        }
    }
    false
}
pub(crate) fn tell_late(d: &Domain, row: &mut crate::TierRow, log: &[u8], out: &mut Queue<Request>) {
    let Some((landing, subscriber, blame)) = late_recipient(d, row) else {
        return;
    };
    let result = row.result.as_mut().expect("late result");
    let (news, class) = match blame {
        Blame::Cleared => (crate::News::Cleared { tier: row.tier.clone(), landing }, crate::Class::Kept),
        Blame::Owns(failing) | Blame::Suspect(failing) => (
            crate::News::Red { tier: row.tier.clone(), landing, failing, run: result.run.clone(), log: Box::from(log) },
            crate::Class::Wakes,
        ),
    };
    out.push(Request::News {
        task: subscriber.task,
        subscription: subscriber.number,
        topic: subscriber.topic,
        news,
        class,
    });
    let mut told = List::with_capacity(
        u32::try_from(result.told.len()).expect("bounded recipients").checked_add(1).expect("one recipient"),
    );
    for mark in &result.told {
        let mut live = false;
        for (_, subscriber) in &d.subscriptions {
            if subscriber.task == mark.goal {
                live = true;
            }
        }
        if live {
            told.push(*mark).expect("existing recipients");
        }
    }
    told.push(crate::LateTold { landing, goal: subscriber.task }).expect("new recipient");
    result.told = told.into_boxed();
}
fn live_goal(d: &Domain, repository: Repository, landing: Commit) -> bool {
    if let Some(verification) = d.verifications.get(&(repository, landing)) {
        for (_, subscriber) in &d.subscriptions {
            if hears(subscriber, verification) {
                return true;
            }
        }
    }
    if let Some(history) = d.landing_logs.get(&repository) {
        for entry in &history.landings {
            if entry.commit == landing {
                for (_, subscriber) in &d.subscriptions {
                    if hears_members(subscriber, repository, &entry.changes) {
                        return true;
                    }
                }
            }
        }
    }
    false
}
/// The existing range bounds stored reds; a full record preserves the current
/// applied verdict and holds its task until findings can consume records.
pub(crate) fn retain_reds(d: &Domain, row: &mut crate::TierRow, out: &mut Queue<Request>) -> bool {
    let result = row.result.as_ref().expect("judged range");
    let run = row.record.run.as_ref().expect("retained range");
    let capacity = d.tier_config_limits.changes.checked_mul(2).expect("range").checked_add(1).expect("range tip");
    let mut retained = List::with_capacity(capacity);
    for red in &row.reds {
        retained.push(red.clone()).expect("retained reds bound");
    }
    for (landing, blame) in run.landings.iter().zip(&result.verdict.landings) {
        if !red(blame) || live_goal(d, row.repository, landing.commit) {
            continue;
        }
        let mut duplicate = false;
        for red in &row.reds {
            if red.landing == landing.commit {
                duplicate = true;
            }
        }
        if duplicate {
            continue;
        }
        if retained
            .push(crate::TierRed { landing: landing.commit, blame: blame.clone(), run: result.run.clone() })
            .is_err()
        {
            return false;
        }
    }
    if row.reds.as_ref() != retained.as_slice() {
        row.reds = retained.into_boxed();
        out.push(Request::Save { read_afresh: None, release: None, record: Stored::Tier(row.clone()) });
    }
    true
}
