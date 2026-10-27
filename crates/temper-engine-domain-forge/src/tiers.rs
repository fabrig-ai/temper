//! Tiers follow adopted landing branches (domain/after-landing.md, 2, 3 and 10).
use crate::{Domain, Limits, Name, Request, Stored, Tier, TierBranch, TierDecision, TierRow, What};
use alloc::boxed::Box;
use core::mem::size_of;
use skein_forge::api::{Commit, Repository, Write};
use skein_lib::{Env, List, Map, Queue, Token};
use temper_engine_domain_forge_tiers as policy;

pub(crate) fn valid(tier: &Tier, l: &Limits) -> bool {
    if tier.workflow.is_empty()
        || tier.workflow.len() > usize::try_from(crate::WORKFLOW_BYTES).expect("workflow bound fits usize")
        || tier.workflow.contains(&b'/')
        || tier.artifact.is_empty()
        || tier.artifact.len() > usize::try_from(crate::CHECK_NAME_BYTES).expect("tier name bound fits usize")
        || tier.artifact.contains(&b'/')
        || tier.ignored.len() > usize::try_from(l.ignored_paths).expect("path count fits usize")
    {
        return false;
    }
    for path in &tier.ignored {
        if path.is_empty() || path.len() > usize::try_from(l.name_bytes).expect("path bound fits usize") {
            return false;
        }
        for part in path_parts(path).as_ref() {
            if part.is_empty() || part.as_ref() == b"." || part.as_ref() == b".." {
                return false;
            }
        }
    }
    true
}

fn path_parts(bytes: &[u8]) -> Box<[Box<[u8]>]> {
    let count = u32::try_from(bytes.len()).expect("bounded path").checked_add(1).expect("one part after each slash");
    let mut parts = List::with_capacity(count);
    let mut from = 0;
    for at in 0..=bytes.len() {
        if at == bytes.len() || bytes.get(at) == Some(&b'/') {
            parts.push(Box::from(bytes.get(from..at).expect("enumerated path part"))).expect("one part per byte");
            from = at.checked_add(1).expect("bounded path cursor");
        }
    }
    parts.into_boxed()
}

fn main_name(d: &Domain, repository: Repository) -> Option<Name> {
    let adopted = d.repositories.get(&repository)?;
    let branch = &adopted.settings.default_branch;
    if branch.is_empty() {
        return None;
    }
    let mut parts = List::with_capacity(d.tier_config_limits.name_bytes);
    for part in path_parts(branch).as_ref() {
        if part.is_empty() {
            return None;
        }
        parts.push(part.clone()).ok()?;
    }
    Some(Name { forge: repository.forge, repository: repository.repository, what: What::Branch(parts.into_boxed()) })
}

pub(crate) fn is_main(d: &Domain, repository: Repository, branch: &[u8]) -> bool {
    !crate::verification::tiers(d, repository).is_empty()
        && match d.repositories.get(&repository) {
            Some(adopted) => adopted.settings.default_branch.as_ref() == branch,
            None => false,
        }
}

pub(crate) fn watched(d: &Domain) -> List<Name> {
    let mut names = List::with_capacity(d.tier_limits.0);
    for (repository, _) in &d.repositories {
        if !d.forge_ended(repository.forge)
            && !crate::verification::tiers(d, *repository).is_empty()
            && let Some(name) = main_name(d, *repository)
        {
            names.push(name).expect("one main per adopted repository");
        }
    }
    names
}

/// Keep the global client watch set under the tiers' reserved owner token.
pub(crate) fn watch(d: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    if watched(d).is_empty() {
        return;
    }
    let mut watches = Map::with_capacity(env.limits.client.resources);
    for (_, names) in &d.names {
        for name in names {
            if !d.forge_ended(name.forge)
                && let Some(resource) = crate::domain::to_client(name, &env.limits)
            {
                let participating = crate::domain::participating(d, &resource);
                watches
                    .insert(resource.clone(), skein_forge_client::Watch { resource, participating })
                    .expect("admitted watches");
            }
        }
    }
    add_watches(d, env, &mut watches);
    let mut list = List::with_capacity(env.limits.client.resources);
    for (_, watch) in &watches {
        list.push(watch.clone()).expect("watch bound");
    }
    crate::domain::child(
        d,
        env,
        skein_forge_client::Event::Keep { owner: Token::new(1_u64 << 60), watches: list.into_boxed() },
        out,
    );
}

pub(crate) fn add_watches(
    d: &Domain,
    env: &Env<Limits>,
    watches: &mut Map<skein_forge_client::Resource, skein_forge_client::Watch>,
) {
    for name in &watched(d) {
        if let Some(resource) = crate::domain::to_client(name, &env.limits) {
            watches
                .insert(resource.clone(), skein_forge_client::Watch { resource, participating: false })
                .expect("tiered mains fit client resources");
        }
    }
}

pub(crate) fn restart(d: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    let mut repositories = List::with_capacity(env.limits.repositories);
    for (repository, _) in &d.repositories {
        repositories.push(*repository).expect("adopted repositories");
    }
    for repository in &repositories {
        start_due(d, env, *repository, out);
    }
}

pub(crate) fn start_due(d: &mut Domain, env: &Env<Limits>, repository: Repository, out: &mut Queue<Request>) {
    let Some(name) = main_name(d, repository) else { return };
    let Some(head) = d.heads.get(&name) else { return };
    let tip = head.commit;
    let Some(adopted) = d.repositories.get(&repository) else { return };
    let Some(settings) = d.tier_settings.get(&adopted.project).copied() else { return };
    let configured = crate::verification::tiers(d, repository);
    let mut names = List::with_capacity(env.limits.tiers);
    for tier in configured {
        names.push(tier.name.clone()).expect("configured tiers");
    }
    for tier in names.into_boxed() {
        let key = (repository, tier.clone());
        let mut row = match d.tier_rows.remove(&key) {
            Some(row) => row,
            None => TierRow {
                repository,
                tier,
                record: policy::Record {
                    sequence: 0,
                    judged: None,
                    failing: policy::Failures::Tests(Box::new([])),
                    flakes: Box::new([]),
                    run: None,
                },
                pending: Box::new([]),
                task: None,
                branch: TierBranch::Absent,
            },
        };
        if policy::due(&row.record, tip) {
            let pending = core::mem::replace(&mut row.pending, Box::new([]));
            match policy::start(&row.record, tip, tip, pending, None, &settings.verdict) {
                policy::Next::Dispatch(run) | policy::Next::Held(run) => {
                    row.record.run = Some(run);
                    row.task = None;
                    row.branch = TierBranch::Absent;
                }
                policy::Next::Judged(verdict) => {
                    row.record = policy::judged(
                        &row.record,
                        &verdict,
                        &policy::Limits {
                            failing_tests: env.limits.failing_tests,
                            test_id_bytes: env.limits.test_id_bytes,
                        },
                    );
                }
            }
            save(&row, out);
        }
        if row.task.is_none()
            && let Some(run) = &row.record.run
        {
            out.push(Request::TierRun { repository, tier: row.tier.clone(), number: run.number, tip: run.tip });
        }
        d.tier_rows.insert(key, row).expect("one row per repository and tier");
    }
}

fn own(d: &Domain, repository: Repository, commit: Commit) -> bool {
    if d.verifications.contains_key(&(repository, commit)) {
        return true;
    }
    match d.landed.get(&commit) {
        Some(task) => match d.changes.get(task) {
            Some(change) => change.repository == repository,
            None => false,
        },
        None => false,
    }
}

pub(crate) fn moved(
    d: &mut Domain,
    env: &Env<Limits>,
    repository: Repository,
    _before: Commit,
    after: Commit,
    commits: Option<&[Commit]>,
    out: &mut Queue<Request>,
) {
    let mut landings = List::with_capacity(env.limits.changes.checked_add(1).expect("landing count"));
    if let Some(commits) = commits {
        for commit in commits {
            if own(d, repository, *commit) && !landings.as_slice().contains(commit) {
                landings.push(*commit).expect("live changes bound temper landings");
            }
        }
    }
    if !landings.as_slice().contains(&after) {
        landings.push(after).expect("one outside tip");
    }
    let mut rows = List::with_capacity(env.limits.tiers);
    for (key, row) in &d.tier_rows {
        if row.repository == repository {
            rows.push(key.clone()).expect("repository tiers");
        }
    }
    for key in &rows {
        let mut row = d.tier_rows.remove(key).expect("collected tier");
        for commit in &landings {
            append(d, env, &mut row, *commit);
        }
        save(&row, out);
        d.tier_rows.insert(key.clone(), row).expect("replaces tier");
    }
    start_due(d, env, repository, out);
}

fn append(d: &Domain, env: &Env<Limits>, row: &mut TierRow, commit: Commit) {
    if let Some(landing) = row.pending.last()
        && landing.commit == commit
    {
        return;
    }
    let bound = env.limits.changes.checked_mul(2).expect("twice the change bound").checked_add(1).expect("range bound");
    let mut pending = List::with_capacity(bound.checked_add(1).expect("one incoming landing"));
    for landing in &row.pending {
        pending.push(*landing).expect("existing range");
    }
    pending.push(policy::Landing { commit, key: commit }).expect("one incoming landing");
    let mut skip = None;
    if pending.len() > bound {
        for (index, pair) in pending.as_slice().windows(2).enumerate() {
            let [earlier, later] = pair else { unreachable!("two-element window") };
            if !d.verifications.contains_key(&(row.repository, earlier.commit))
                && !d.verifications.contains_key(&(row.repository, later.commit))
            {
                skip = Some(index);
                break;
            }
        }
        assert!(skip.is_some(), "at most changes protected landings leave adjacent outside landings");
    }
    let mut compacted = List::with_capacity(bound);
    for (index, landing) in pending.into_boxed().into_iter().enumerate() {
        if skip != Some(index) {
            compacted.push(landing).expect("compacted range bound");
        }
    }
    row.pending = compacted.into_boxed();
}

pub(crate) fn task(
    d: &mut Domain,
    repository: Repository,
    tier: Box<[u8]>,
    number: u64,
    task: Option<u64>,
    out: &mut Queue<Request>,
) {
    let Some(row) = d.tier_rows.get_mut(&(repository, tier)) else { return };
    if let Some(run) = &row.record.run
        && run.number == number
        && (row.task.is_none() || task.is_none())
    {
        row.task = task;
        save(row, out);
    }
}

/// Decimal run numbers keep the branch stable across steps and restarts.
#[must_use]
pub fn branch(tier: &[u8], number: u64) -> Box<[u8]> {
    let digits = skein_lib::Decimal::of(number);
    let count = 13_usize
        .checked_add(tier.len())
        .expect("bounded tier name")
        .checked_add(digits.as_bytes().len())
        .expect("bounded decimal run");
    let mut bytes = List::with_capacity(u32::try_from(count).expect("tier branch bytes"));
    for byte in b"temper/tier/".iter().chain(tier).chain(b".").chain(digits.as_bytes()) {
        bytes.push(*byte).expect("branch sized from its parts");
    }
    bytes.into_boxed()
}

pub(crate) fn step(d: &mut Domain, env: &Env<Limits>, task: u64, entry: u64, out: &mut Queue<Request>) {
    let mut found = None;
    for (key, row) in &d.tier_rows {
        if row.task == Some(task) {
            found = Some(key.clone());
            break;
        }
    }
    let Some(key) = found else {
        out.push(Request::TierDecision { task, decision: TierDecision::Held });
        return;
    };
    let mut row = d.tier_rows.remove(&key).expect("found tier");
    let decision = match &row.record.run {
        None => TierDecision::Held,
        Some(run) => {
            if row.branch == TierBranch::Absent {
                if d.entries.contains_key(&entry) || d.entries.len() == env.limits.entries {
                    d.tier_rows.insert(key, row).expect("replaces tier");
                    out.push(Request::TierDecision { task, decision: TierDecision::Held });
                    return;
                }
                let entry = skein_forge_client::Entry {
                    number: entry,
                    task,
                    repository: row.repository,
                    effect: skein_forge_client::Effect {
                        write: Write::CreateBranch { branch: branch(&row.tier, run.number), commit: run.tip },
                        condition: skein_forge_client::Condition::None,
                    },
                    start: None,
                    attempt: None,
                    failures: 0,
                    queued: false,
                };
                let mut repository = [0; 16];
                let identity = u64::from(row.repository.forge)
                    .to_be_bytes()
                    .into_iter()
                    .chain(u64::from(row.repository.repository).to_be_bytes());
                for (slot, byte) in repository.iter_mut().zip(identity) {
                    *slot = byte;
                }
                let mut index = None;
                for (at, tier) in crate::verification::tiers(d, row.repository).iter().enumerate() {
                    if tier.name == row.tier {
                        index = Some(u16::try_from(at).expect("tier index"));
                    }
                }
                let Some(tier) = index else {
                    d.tier_rows.insert(key, row).expect("replaces tier");
                    out.push(Request::TierDecision { task, decision: TierDecision::Held });
                    return;
                };
                let purpose =
                    crate::purpose::EffectPurpose::Tier { repository, tier, run: run.number, operation: 1, part: 0 };
                row.branch = TierBranch::Pending { entry: entry.number };
                save(&row, out);
                crate::domain::keep_entry(d, entry.clone(), None, out);
                out.push(Request::TierEffect { task, entry, purpose });
                d.tier_rows.insert(key, row).expect("replaces tier");
                return;
            }
            TierDecision::Wait
        }
    };
    d.tier_rows.insert(key, row).expect("replaces tier");
    out.push(Request::TierDecision { task, decision });
}

pub(crate) fn veto(d: &mut Domain, task: u64, entry: u64, out: &mut Queue<Request>) {
    // Only a never-committed entry can be vetoed by admission.
    let mut key = None;
    for (candidate, row) in &d.tier_rows {
        if row.branch == (TierBranch::Pending { entry }) {
            key = Some(candidate.clone());
            break;
        }
    }
    if let Some(key) = key {
        let row = d.tier_rows.get_mut(&key).expect("found tier");
        if row.task == Some(task) && row.branch == (TierBranch::Pending { entry }) {
            row.branch = TierBranch::Absent;
            save(row, out);
            d.entries.remove(&entry);
            out.push(Request::Erase { key: crate::Key::Entry(entry) });
        }
    }
}

pub(crate) fn outcome(d: &mut Domain, entry: u64, outcome: skein_forge_client::Outcome, out: &mut Queue<Request>) {
    let mut key = None;
    for (candidate, row) in &d.tier_rows {
        if row.branch == (TierBranch::Pending { entry }) {
            key = Some(candidate.clone());
            break;
        }
    }
    if let Some(key) = key {
        let row = d.tier_rows.get_mut(&key).expect("found tier");
        if row.branch != (TierBranch::Pending { entry }) {
            return;
        }
        match outcome {
            skein_forge_client::Outcome::Made { made: skein_forge_client::Made::Branch(_), .. } => {
                row.branch = TierBranch::Made;
            }
            skein_forge_client::Outcome::Uncertain | skein_forge_client::Outcome::Queued => return,
            skein_forge_client::Outcome::Made { .. }
            | skein_forge_client::Outcome::Failed(_)
            | skein_forge_client::Outcome::Raced { .. }
            | skein_forge_client::Outcome::Held
            | skein_forge_client::Outcome::Withdrawn => {
                if let Some(task) = row.task {
                    out.push(Request::TierDecision { task, decision: TierDecision::Held });
                }
                return;
            }
        }
        save(row, out);
        if let Some(task) = row.task {
            out.push(Request::TierDecision { task, decision: TierDecision::Wait });
        }
    }
}

fn save(row: &TierRow, out: &mut Queue<Request>) {
    out.push(Request::Save { read_afresh: None, release: None, record: Stored::Tier(row.clone()) });
}

pub(crate) fn stored_bytes(row: &TierRow) -> Option<u64> {
    let mut bytes = u64::try_from(row.tier.len())
        .ok()?
        .checked_add(array_bytes(row.pending.len(), size_of::<policy::Landing>())?)?;
    bytes = bytes.checked_add(failures_bytes(&row.record.failing)?)?;
    for flake in &row.record.flakes {
        bytes = bytes
            .checked_add(u64::try_from(size_of::<policy::Flake>()).ok()?)?
            .checked_add(u64::try_from(flake.test.len()).ok()?)?;
    }
    if let Some(run) = &row.record.run {
        bytes = bytes.checked_add(array_bytes(run.landings.len(), size_of::<policy::Landing>())?)?;
        match &run.part {
            policy::Part::Whole => {}
            policy::Part::Rerun { failed } => bytes = bytes.checked_add(tests_bytes(failed)?)?,
            policy::Part::Probe { tests, finding, .. } => {
                if let policy::Selection::Tests(tests) = tests {
                    bytes = bytes.checked_add(tests_bytes(tests)?)?;
                }
                bytes = bytes
                    .checked_add(failures_bytes(&finding.failing)?)?
                    .checked_add(tests_bytes(&finding.flakes)?)?
                    .checked_add(array_bytes(finding.blamed.len(), size_of::<policy::Blame>())?)?;
                for blame in &finding.blamed {
                    if let policy::Blame::Owns(f) | policy::Blame::Suspect(f) = blame {
                        bytes = bytes.checked_add(failures_bytes(f)?)?;
                    }
                }
            }
        }
    }
    Some(bytes)
}
fn array_bytes(count: usize, bytes: usize) -> Option<u64> {
    u64::try_from(count).ok()?.checked_mul(u64::try_from(bytes).ok()?)
}
fn tests_bytes(tests: &[Box<[u8]>]) -> Option<u64> {
    let mut bytes = array_bytes(tests.len(), size_of::<Box<[u8]>>())?;
    for test in tests {
        bytes = bytes.checked_add(u64::try_from(test.len()).ok()?)?;
    }
    Some(bytes)
}
fn failures_bytes(f: &policy::Failures) -> Option<u64> {
    match f {
        policy::Failures::Many => Some(0),
        policy::Failures::Tests(tests) => tests_bytes(tests),
    }
}
pub(crate) fn worst_case(l: &Limits) -> Option<u64> {
    let rows = l.repositories.checked_mul(l.tiers)?;
    let landings = u64::from(l.changes.checked_mul(2)?.checked_add(1)?);
    let tests = u64::from(l.failing_tests)
        .checked_mul(u64::from(l.test_id_bytes).checked_add(u64::try_from(size_of::<Box<[u8]>>()).ok()?)?)?;
    let payload = u64::from(crate::CHECK_NAME_BYTES)
        .checked_add(landings.checked_mul(u64::try_from(size_of::<policy::Landing>()).ok()?.checked_mul(2)?)?)?
        .checked_add(tests.checked_mul(5)?)?
        .checked_add(u64::from(l.failing_tests).checked_mul(u64::try_from(size_of::<policy::Flake>()).ok()?)?)?
        .checked_add(landings.checked_mul(u64::try_from(size_of::<policy::Blame>()).ok()?.checked_add(tests)?)?)?;
    Map::<(Repository, Box<[u8]>), TierRow>::worst_case(rows)?
        .checked_add(u64::from(rows).checked_mul(payload.checked_add(u64::from(crate::CHECK_NAME_BYTES))?)?)
}

impl Domain {
    /// The run owned by a tier procedure task, if it still exists.
    #[must_use]
    #[expect(clippy::manual_find, reason = "step code searches explicitly without callbacks")]
    pub fn tier_task(&self, task: u64) -> Option<&TierRow> {
        for (_, row) in &self.tier_rows {
            if row.task == Some(task) {
                return Some(row);
            }
        }
        None
    }
}
