//! Tiers follow adopted landing branches (domain/after-landing.md, 2, 3 and 10).
use crate::{Domain, Limits, Name, Request, Stored, Tier, TierBranch, TierDecision, TierRow, What};
use alloc::boxed::Box;
use core::mem::size_of;
use skein_forge::api::{Commit, Repository, Write};
use skein_lib::{Env, List, Map, Queue, Token};
use temper_engine_domain_forge_tiers as policy;

pub(crate) fn valid(tier: &Tier, l: &Limits) -> bool {
    if tier.workflow.is_empty()
        || tier.workflow.len() > crate::WORKFLOW_BYTES
        || tier.workflow.contains(&b'/')
        || tier.artifact.is_empty()
        || tier.artifact.len() > crate::CHECK_NAME_BYTES
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
                flight: None,
                result: None,
            },
        };
        if policy::due(&row.record, tip) {
            let pending = core::mem::replace(&mut row.pending, Box::new([]));
            let receipt_key = receipt_key(d, &row, tip);
            let receipt = d.landing.receipts.get(&receipt_key).cloned();
            let cached = match &receipt {
                Some(receipt) if receipt.passed => Some(policy::Receipt::Passed),
                Some(receipt) => {
                    Some(policy::Receipt::Failed(receipt.failures.clone().unwrap_or(policy::Failures::Many)))
                }
                None => None,
            };
            // Retain the range even when the policy judges immediately: its task
            // must admit the status before committing the rest of the judgement.
            let policy::Next::Dispatch(run) =
                policy::start(&row.record, tip, tip, pending.clone(), None, &settings.verdict)
            else {
                unreachable!("without a receipt start dispatches the whole run")
            };
            row.record.run = Some(run);
            row.task = None;
            row.branch = TierBranch::Absent;
            row.flight = None;
            row.result = None;
            match policy::start(&row.record, tip, tip, pending, cached, &settings.verdict) {
                policy::Next::Dispatch(next) | policy::Next::Held(next) => row.record.run = Some(next),
                policy::Next::Judged(verdict) => {
                    row.result = Some(crate::TierResult {
                        verdict,
                        run: receipt.expect("an immediate judgement uses a receipt").run,
                        received: true,
                        status: TierBranch::Absent,
                    });
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

fn receipt_key(d: &Domain, row: &TierRow, key: Commit) -> crate::ReceiptKey {
    crate::ReceiptKey {
        project: d.repositories.get(&row.repository).expect("adopted tier").project,
        repository: row.repository,
        check: crate::Check::Tier(row.tier.clone()),
        key,
    }
}
fn purpose(d: &Domain, row: &TierRow, operation: u8, part: u16) -> crate::purpose::EffectPurpose {
    let mut repository = [0; 16];
    for (slot, byte) in repository.iter_mut().zip(
        u64::from(row.repository.forge)
            .to_be_bytes()
            .into_iter()
            .chain(u64::from(row.repository.repository).to_be_bytes()),
    ) {
        *slot = byte;
    }
    let mut index = None;
    for (at, tier) in crate::verification::tiers(d, row.repository).iter().enumerate() {
        if tier.name == row.tier {
            index = Some(u16::try_from(at).expect("tier index"));
        }
    }
    crate::purpose::EffectPurpose::Tier {
        repository,
        tier: index.expect("configured tier"),
        run: row.record.run.as_ref().expect("live run").number,
        operation,
        part,
    }
}
fn configured(d: &Domain, row: &TierRow) -> Tier {
    for tier in crate::verification::tiers(d, row.repository) {
        if tier.name == row.tier {
            return tier.clone();
        }
    }
    unreachable!("a live tier is configured")
}
fn following(d: &Domain, row: &TierRow) -> crate::runs::Follow {
    let run = row.record.run.as_ref().expect("live run");
    let task = row.task.expect("tier task");
    let mut follow = crate::runs::follow(
        task,
        0,
        purpose(d, row, 2, 0).identity().expect("tier purpose"),
        row.repository,
        configured(d, row).workflow,
        branch(&row.tier, run.number),
        run.tip,
        crate::landing::inputs(run.tip, run.key),
    );
    if let Some(flight) = &row.flight {
        follow.entry = flight.entry;
        follow.start = flight.start;
        follow.since = flight.since;
        follow.waiting = flight.waiting;
        if flight.start.is_some() || flight.waiting || !flight.pending.is_empty() || flight.fault {
            let mut writes = List::with_capacity(u32::try_from(flight.pending.len()).expect("bounded reruns"));
            for attempt in &flight.pending {
                writes.push(Write::Rerun { attempt: *attempt }).expect("pending reruns");
            }
            follow.writes = writes.into_boxed();
        }
        if flight.fault {
            follow.outcome = Some(temper_engine_domain_forge_landing::Verdict::Fault(
                temper_engine_domain_forge_landing::Fault::Lost,
            ));
        }
    }
    follow
}
fn remember(row: &mut TierRow, follow: &crate::runs::Follow) {
    let mut pending = List::with_capacity(u32::try_from(follow.writes.len()).expect("bounded writes"));
    for write in &follow.writes {
        if let Write::Rerun { attempt } = write {
            pending.push(*attempt).expect("one pending job per write");
        }
    }
    row.flight = Some(crate::TierFlight {
        next: match &row.flight {
            Some(flight) => flight.next,
            None => None,
        },
        entry: follow.entry,
        start: follow.start,
        since: follow.since,
        waiting: follow.waiting,
        fault: follow.outcome.is_some(),
        pending: pending.into_boxed(),
    });
}
pub(crate) fn rebuild(d: &mut Domain) {
    for (key, row) in &d.tier_rows {
        if row.flight.is_some() && row.task.is_some() && row.result.is_none() {
            d.tier_follows.insert(key.clone(), following(d, row)).expect("one follower per tier");
        }
    }
}
#[expect(
    clippy::disallowed_macros,
    reason = "T6 explicitly requires a debug assertion for parts that cannot yet arise"
)]
fn advance(d: &Domain, row: &mut TierRow, follow: &mut crate::runs::Follow) {
    use temper_engine_domain_forge_landing::Verdict;
    let Some(outcome) = follow.outcome else { return };
    let run = row.record.run.as_ref().expect("followed run");
    let outcome = match outcome {
        Verdict::Passed => policy::Outcome::Passed,
        Verdict::Failed => policy::failed(None),
        Verdict::Fault(_) => policy::Outcome::Fault,
        Verdict::Running => return,
    };
    let project = d.repositories.get(&row.repository).expect("adopted tier").project;
    let settings = d.tier_settings.get(&project).expect("tier settings");
    match policy::step(&row.record, run, outcome, &settings.verdict) {
        policy::Next::Dispatch(next) => {
            if next.part != run.part {
                debug_assert!(false, "failures remain faults until reports are read");
                let mut held = next;
                held.faults = settings.verdict.retries.saturating_add(1);
                row.record.run = Some(held);
                return;
            }
            let attempt = u16::try_from(next.faults).expect("bounded retry").checked_add(1).expect("retry attempt");
            row.record.run = Some(next);
            crate::runs::retry(follow, attempt);
            if follow.start.is_none() {
                follow.since = None;
            }
            remember(row, follow);
        }
        policy::Next::Held(run) => {
            row.record.run = Some(run);
            remember(row, follow);
        }
        policy::Next::Judged(verdict) => {
            let run = follow.run.as_ref().expect("a judged workflow has a run");
            row.result = Some(crate::TierResult {
                verdict,
                run: crate::ReceiptRun { id: run.id, number: run.number, url: run.url.clone() },
                received: false,
                status: TierBranch::Absent,
            });
        }
    }
}
fn wait(task: u64, out: &mut Queue<Request>) {
    out.push(Request::TierDecision { task, decision: TierDecision::Wait });
}
fn effect(d: &mut Domain, row: &TierRow, entry: u64, write: Write, operation: u8, part: u16, out: &mut Queue<Request>) {
    let task = row.task.expect("effect task");
    let entry = skein_forge_client::Entry {
        number: entry,
        task,
        repository: row.repository,
        effect: skein_forge_client::Effect { write, condition: skein_forge_client::Condition::None },
        start: None,
        attempt: None,
        failures: 0,
        queued: false,
    };
    save(row, out);
    crate::domain::keep_entry(d, entry.clone(), None, out);
    out.push(Request::TierEffect { task, entry, purpose: purpose(d, row, operation, part) });
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
    let was_reading = reading(d, &key);
    let mut row = d.tier_rows.remove(&key).expect("found tier");
    step_row(d, env, &key, &mut row, task, entry, out);
    let repository = row.repository;
    let done = row.record.run.is_none();
    d.tier_rows.insert(key.clone(), row).expect("replaces tier");
    if !was_reading {
        let mut owner = None;
        for (token, candidate) in &d.tier_reads {
            if *candidate == key {
                owner = Some(*token);
                break;
            }
        }
        if let Some(owner) = owner
            && let Some(follow) = d.tier_follows.get(&key)
            && let Some(read) = follow.read()
        {
            crate::domain::child(d, env, skein_forge_client::Event::Read { owner, repository, read }, out);
        }
    }
    if done {
        start_due(d, env, repository, out);
    }
}
fn step_row(
    d: &mut Domain,
    env: &Env<Limits>,
    key: &(Repository, Box<[u8]>),
    row: &mut TierRow,
    task: u64,
    entry: u64,
    out: &mut Queue<Request>,
) {
    let Some(run) = &row.record.run else {
        out.push(Request::TierDecision { task, decision: TierDecision::Held });
        return;
    };
    let project = d.repositories.get(&row.repository).expect("adopted tier").project;
    if run.faults > d.tier_settings.get(&project).expect("settings").verdict.retries {
        out.push(Request::TierDecision { task, decision: TierDecision::Held });
        return;
    }
    if row.result.is_none()
        && let Some(mut follow) = d.tier_follows.remove(key)
    {
        if follow.outcome.is_some() {
            advance(d, row, &mut follow);
            save(row, out);
        }
        d.tier_follows.insert(key.clone(), follow).expect("one follow per tier");
    }
    let run = row.record.run.as_ref().expect("checked live run");
    if run.faults > d.tier_settings.get(&project).expect("settings").verdict.retries {
        out.push(Request::TierDecision { task, decision: TierDecision::Held });
        return;
    }
    if let Some(result) = &row.result {
        if result.status == TierBranch::Made {
            finish(d, env, key, row, out);
            return;
        }
        if result.status != TierBranch::Absent {
            wait(task, out);
            return;
        }
    } else if row.branch != TierBranch::Made && row.branch != TierBranch::Absent {
        wait(task, out);
        return;
    }
    if d.entries.contains_key(&entry) || d.entries.len() == env.limits.entries {
        out.push(Request::TierDecision { task, decision: TierDecision::Held });
        return;
    }
    if let Some(result) = &mut row.result {
        let write = status(&row.tier, result);
        result.status = TierBranch::Pending { entry };
        effect(d, row, entry, write, 7, 0, out);
    } else if row.branch == TierBranch::Absent {
        let write = Write::CreateBranch { branch: branch(&row.tier, run.number), commit: run.tip };
        row.branch = TierBranch::Pending { entry };
        effect(d, row, entry, write, 1, 0, out);
    } else if row.branch == TierBranch::Made {
        let mut follow = match d.tier_follows.remove(key) {
            Some(follow) => follow,
            None => following(d, row),
        };
        if !follow.waiting && !follow.writes.is_empty() {
            let write = follow.write(entry).expect("pending write");
            let operation = if let Write::Rerun { .. } = &write { 3 } else { 2 };
            let faults = run.faults;
            remember(row, &follow);
            effect(d, row, entry, write, operation, u16::try_from(faults).expect("retry part"), out);
        } else if !reading(d, key) {
            if follow.read().is_some() {
                d.sequence = d.sequence.checked_add(1).expect("tier read sequence");
                let owner = Token::new(d.sequence | (1_u64 << 59));
                d.tier_reads.insert(owner, key.clone()).expect("one read per tier");
                d.tier_follows.insert(key.clone(), follow).expect("one follow per tier");
                wait(task, out);
                return;
            }
            wait(task, out);
        } else {
            wait(task, out);
        }
        d.tier_follows.insert(key.clone(), follow).expect("one follow per tier");
    } else {
        wait(task, out);
    }
}
fn reading(d: &Domain, key: &(Repository, Box<[u8]>)) -> bool {
    for (_, candidate) in &d.tier_reads {
        if candidate == key {
            return true;
        }
    }
    false
}
fn joined(parts: &[&[u8]]) -> Box<[u8]> {
    let mut length = 0_u32;
    for part in parts {
        length = length.checked_add(u32::try_from(part.len()).expect("bounded words")).expect("word length");
    }
    let mut bytes = List::with_capacity(length);
    for part in parts {
        for byte in *part {
            bytes.push(*byte).expect("measured words");
        }
    }
    bytes.into_boxed()
}
fn status(tier: &[u8], result: &crate::TierResult) -> Write {
    let (check, description) = match &result.verdict.failing {
        policy::Failures::Tests(tests) if tests.is_empty() => {
            (skein_forge::api::Check::Passed, Box::from(b"green".as_slice()))
        }
        policy::Failures::Tests(tests) => (
            skein_forge::api::Check::Failed,
            joined(&[
                b"red: ",
                skein_lib::Decimal::of(u64::try_from(tests.len()).expect("bounded tests")).as_bytes(),
                b" failing tests",
            ]),
        ),
        policy::Failures::Many => (skein_forge::api::Check::Failed, Box::from(b"red: many failing tests".as_slice())),
    };
    Write::Status {
        commit: result.verdict.tip,
        context: joined(&[b"temper/verdict/", tier]),
        check,
        description,
        link: result.run.url.clone(),
    }
}
fn finish(
    d: &mut Domain,
    env: &Env<Limits>,
    key: &(Repository, Box<[u8]>),
    row: &mut TierRow,
    out: &mut Queue<Request>,
) {
    let result = row.result.take().expect("status posted");
    let run = row.record.run.as_ref().expect("judged range");
    let mut landings = List::with_capacity(u32::try_from(run.landings.len()).expect("bounded range"));
    let mut cleared = 0_u64;
    for (landing, blame) in run.landings.iter().zip(&result.verdict.landings) {
        landings.push((landing.commit, blame.clone())).expect("one blame per landing");
        if *blame == policy::Blame::Cleared {
            cleared = cleared.checked_add(1).expect("bounded landings");
        }
    }
    crate::verification::judged(d, row.repository, &row.tier, result.verdict.number, landings.as_slice(), out);
    crate::log::judged(d, row.repository, &row.tier, landings.as_slice(), out);
    if !result.received && run.part == policy::Part::Whole {
        let passed = match &result.verdict.failing {
            policy::Failures::Tests(tests) => tests.is_empty(),
            policy::Failures::Many => false,
        };
        crate::landing::put_receipt(
            d,
            env,
            receipt_key(d, row, run.key),
            crate::Receipt {
                passed,
                failures: Some(result.verdict.failing.clone()),
                candidate: run.tip,
                ended: env.wall,
                run: result.run.clone(),
            },
            out,
        );
    }
    let tip = crate::landing::inputs(run.tip, run.key);
    let tip = &tip.first().expect("commit input").value;
    let kind = if result.received { b" judged by receipt: ".as_slice() } else { b": ".as_slice() };
    let colour = match &result.verdict.failing {
        policy::Failures::Tests(tests) if tests.is_empty() => b"green".as_slice(),
        policy::Failures::Tests(_) | policy::Failures::Many => b"red".as_slice(),
    };
    let words = joined(&[
        b"Tier ",
        &row.tier,
        b" run ",
        skein_lib::Decimal::of(run.number).as_bytes(),
        b" at ",
        tip.get(..12).expect("12 hex tip"),
        kind,
        colour,
        b", ",
        skein_lib::Decimal::of(cleared).as_bytes(),
        b" landings cleared",
    ]);
    row.record = policy::judged(
        &row.record,
        &result.verdict,
        &policy::Limits { failing_tests: env.limits.failing_tests, test_id_bytes: env.limits.test_id_bytes },
    );
    let task = row.task.take().expect("tier task");
    row.branch = TierBranch::Absent;
    row.flight = None;
    d.tier_follows.remove(key);
    save(row, out);
    out.push(Request::TierDecision { task, decision: TierDecision::Done { words } });
}
#[expect(clippy::match_like_matches_macro, reason = "the foundation uses explicit matches")]
pub(crate) fn answered(
    d: &mut Domain,
    env: &Env<Limits>,
    owner: Token,
    answer: Result<skein_forge::api::Answer, skein_forge::api::Error>,
    out: &mut Queue<Request>,
) {
    let Some(key) = d.tier_reads.remove(&owner) else { return };
    let Some(mut row) = d.tier_rows.remove(&key) else { return };
    let mut follow = d.tier_follows.remove(&key).expect("read's follower");
    let writer = d.writers.get(&row.repository.forge).copied().unwrap_or(0);
    let listing = if let Ok(skein_forge::api::Answer::Runs { .. }) = &answer { true } else { false };
    let ended = follow.answered(answer, writer, env.now, env.limits.client.lifetime).is_some();
    if follow.outcome.is_some() {
        advance(d, &mut row, &mut follow);
    }
    remember(&mut row, &follow);
    save(&row, out);
    if ended || (listing && follow.run.is_some()) {
        if let Some(task) = row.task {
            out.push(Request::WakeProcedure { task });
        }
    } else if let Some(flight) = &mut row.flight {
        flight.next = Some(env.now.saturating_add(env.limits.client.poll));
        save(&row, out);
    }
    d.tier_rows.insert(key.clone(), row).expect("replaces tier");
    d.tier_follows.insert(key, follow).expect("replaces follower");
}
pub(crate) fn hinted(d: &Domain, hint: &skein_forge::api::Hint, out: &mut Queue<Request>) {
    if let skein_forge::api::Change::Commit(commit) = hint.change {
        for (_, follow) in &d.tier_follows {
            if follow.repository == hint.repository && follow.commit == commit {
                out.push(Request::WakeProcedure { task: follow.task });
            }
        }
    }
}
pub(crate) fn deadline(d: &Domain) -> Option<skein_lib::Time> {
    if d.stopping {
        return None;
    }
    let mut next: Option<skein_lib::Time> = None;
    for (_, row) in &d.tier_rows {
        if let Some(flight) = &row.flight
            && row.result.is_none()
            && let Some(at) = flight.next
        {
            next = Some(match next {
                Some(old) => old.min(at),
                None => at,
            });
        }
    }
    next
}
pub(crate) fn fire(d: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    let mut found = None;
    for (key, row) in &d.tier_rows {
        if row.result.is_none()
            && let Some(flight) = &row.flight
            && let Some(at) = flight.next
            && at <= env.now
        {
            found = Some(key.clone());
            break;
        }
    }
    if let Some(key) = found {
        let row = d.tier_rows.get_mut(&key).expect("due tier");
        row.flight.as_mut().expect("due flight").next = None;
        save(row, out);
        if let Some(task) = row.task {
            out.push(Request::WakeProcedure { task });
        }
    }
}
pub(crate) fn veto(d: &mut Domain, task: u64, entry: u64, out: &mut Queue<Request>) {
    let mut key = None;
    for (candidate, row) in &d.tier_rows {
        if row.task == Some(task) {
            key = Some(candidate.clone());
            break;
        }
    }
    if let Some(key) = key {
        let row = d.tier_rows.get_mut(&key).expect("found tier");
        if row.branch == (TierBranch::Pending { entry }) {
            row.branch = TierBranch::Absent;
        }
        if let Some(result) = &mut row.result
            && result.status == (TierBranch::Pending { entry })
        {
            result.status = TierBranch::Absent;
        }
        if let Some(flight) = &mut row.flight
            && flight.entry == entry
        {
            flight.waiting = false;
            flight.fault = true;
        }
        save(row, out);
    }
    d.entries.remove(&entry);
    out.push(Request::Erase { key: crate::Key::Entry(entry) });
}
#[expect(clippy::match_like_matches_macro, reason = "the foundation uses explicit matches")]
pub(crate) fn outcome(d: &mut Domain, entry: u64, outcome: skein_forge_client::Outcome, out: &mut Queue<Request>) {
    use skein_forge_client::{Made, Outcome};
    let mut found = None;
    for (key, row) in &d.tier_rows {
        if row.branch == (TierBranch::Pending { entry })
            || match &row.flight {
                Some(flight) => flight.entry == entry,
                None => false,
            }
            || match &row.result {
                Some(result) => result.status == (TierBranch::Pending { entry }),
                None => false,
            }
        {
            found = Some(key.clone());
            break;
        }
    }
    if let Some(key) = found {
        let row = d.tier_rows.get_mut(&key).expect("found tier");
        let pending = TierBranch::Pending { entry };
        let flight_entry = match &row.flight {
            Some(flight) => flight.entry == entry,
            None => false,
        };
        let status_entry = match &row.result {
            Some(result) => result.status == pending,
            None => false,
        };

        if outcome == Outcome::Uncertain || outcome == Outcome::Queued {
            return;
        }
        let made = if let Outcome::Made { .. } = outcome { true } else { false };
        if row.branch == pending && if let Outcome::Made { made: Made::Branch(_), .. } = outcome { true } else { false }
        {
            row.branch = TierBranch::Made;
        }
        if status_entry && made {
            row.result.as_mut().expect("status result").status = TierBranch::Made;
        }
        if flight_entry {
            let follow = d.tier_follows.get_mut(&key).expect("effect follower");
            follow.waiting = false;
            if let Some(entry) = d.entries.get(&entry) {
                if let Write::Dispatch { .. } = entry.entry.effect.write
                    && let Some(start) = entry.entry.start
                {
                    follow.start = Some(start.at);
                }
                if let Some(attempt) = entry.entry.attempt {
                    follow.since = Some(attempt.sent);
                }
            }
            if !made {
                follow.outcome = Some(temper_engine_domain_forge_landing::Verdict::Fault(
                    temper_engine_domain_forge_landing::Fault::Lost,
                ));
            }
            remember(row, follow);
        }
        save(row, out);
        if let Some(task) = row.task {
            if !made && !flight_entry {
                out.push(Request::TierDecision { task, decision: TierDecision::Held });
            } else {
                out.push(Request::WakeProcedure { task });
            }
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
    if let Some(flight) = &row.flight {
        bytes = bytes.checked_add(array_bytes(flight.pending.len(), size_of::<skein_forge::api::JobAttempt>())?)?;
    }
    if let Some(result) = &row.result {
        bytes = bytes
            .checked_add(failures_bytes(&result.verdict.failing)?)?
            .checked_add(tests_bytes(&result.verdict.flakes)?)?
            .checked_add(array_bytes(result.verdict.landings.len(), size_of::<policy::Blame>())?)?
            .checked_add(u64::try_from(result.run.url.len()).ok()?)?;
        for blame in &result.verdict.landings {
            if let policy::Blame::Owns(f) | policy::Blame::Suspect(f) = blame {
                bytes = bytes.checked_add(failures_bytes(f)?)?;
            }
        }
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
pub(crate) fn failures_bytes(f: &policy::Failures) -> Option<u64> {
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
    let payload = u64::try_from(crate::CHECK_NAME_BYTES)
        .ok()?
        .checked_add(landings.checked_mul(u64::try_from(size_of::<policy::Landing>()).ok()?.checked_mul(2)?)?)?
        .checked_add(tests.checked_mul(5)?)?
        .checked_add(u64::from(l.failing_tests).checked_mul(u64::try_from(size_of::<policy::Flake>()).ok()?)?)?
        .checked_add(landings.checked_mul(u64::try_from(size_of::<policy::Blame>()).ok()?.checked_add(tests)?)?)?;
    // Whole-run dispatch inputs: two 64-hex values and their literal names.
    let inputs = array_bytes(2, size_of::<skein_forge::api::Input>())?
        .checked_add(u64::try_from(b"temper_commit".len().checked_add(b"temper_key".len())?).ok()?)?
        .checked_add(128)?;
    // Branch prefix/separator (13), bounded tier name, and u64's decimal (20).
    let dispatch = u64::try_from(crate::WORKFLOW_BYTES)
        .ok()?
        .checked_add(13 + 20)?
        .checked_add(u64::try_from(crate::CHECK_NAME_BYTES).ok()?)?
        .checked_add(inputs)?;
    let jobs = u64::from(l.client.rows);
    let follow_payload = dispatch
        .checked_mul(2)?
        .checked_add(30)?
        .checked_add(jobs.max(1).checked_mul(u64::try_from(size_of::<Write>()).ok()?)?)?
        .checked_add(jobs.checked_mul(u64::try_from(size_of::<skein_forge::api::RunJob>()).ok()?)?.checked_mul(2)?)?
        .checked_add(jobs.checked_mul(u64::try_from(size_of::<skein_forge::api::Run>()).ok()?)?)?
        .checked_add(u64::from(l.client.answer_bytes).checked_mul(2)?)?;
    let result_payload = landings
        .checked_mul(u64::try_from(size_of::<policy::Blame>()).ok()?.checked_add(tests)?)?
        .checked_add(tests.checked_mul(2)?)?
        .checked_add(2048)?;
    Map::<(Repository, Box<[u8]>), TierRow>::worst_case(rows)?
        .checked_add(Map::<(Repository, Box<[u8]>), crate::runs::Follow>::worst_case(rows)?)?
        .checked_add(Map::<Token, (Repository, Box<[u8]>)>::worst_case(rows)?)?
        .checked_add(
            u64::from(rows).checked_mul(
                payload
                    .checked_add(u64::try_from(crate::CHECK_NAME_BYTES).ok()?.checked_mul(3)?)?
                    .checked_add(follow_payload)?
                    .checked_add(result_payload)?
                    .checked_add(
                        u64::from(l.client.rows)
                            .checked_mul(u64::try_from(size_of::<skein_forge::api::JobAttempt>()).ok()?)?,
                    )?,
            )?,
        )
}

impl Domain {
    /// One repository's durable tier progress, including a receipt awaiting its task.
    #[must_use]
    pub fn tier_row(&self, repository: Repository, tier: &[u8]) -> Option<&TierRow> {
        self.tier_rows.get(&(repository, Box::from(tier)))
    }

    /// Validate a tier's saved effect against its run and operation before authority admission.
    #[must_use]
    pub fn tier_effect_kind(&self, entry: &skein_forge_client::Entry) -> Option<(u16, What)> {
        let row = self.tier_task(entry.task)?;
        let run = row.record.run.as_ref()?;
        if entry.repository != row.repository || self.entries.get(&entry.number)?.entry != *entry {
            return None;
        }
        let pending = TierBranch::Pending { entry: entry.number };
        let branch = branch(&row.tier, run.number);
        let kind = match &entry.effect.write {
            Write::CreateBranch { branch: name, commit }
                if row.branch == pending && *name == branch && *commit == run.tip =>
            {
                9
            }
            Write::Dispatch { workflow, branch: name, inputs }
                if *workflow == configured(self, row).workflow
                    && *name == branch
                    && *inputs == crate::landing::inputs(run.tip, run.key)
                    && row.flight.as_ref()?.entry == entry.number =>
            {
                16
            }
            Write::Rerun { attempt } if attempt.head == run.tip && row.flight.as_ref()?.entry == entry.number => 17,
            Write::Status { .. }
                if row.result.as_ref()?.status == pending
                    && entry.effect.write == status(&row.tier, row.result.as_ref()?) =>
            {
                return Some((6, What::Repository));
            }
            Write::CreateBranch { .. }
            | Write::Dispatch { .. }
            | Write::Rerun { .. }
            | Write::Status { .. }
            | Write::CreateIssue { .. }
            | Write::OpenPull { .. }
            | Write::Post { .. }
            | Write::Review { .. }
            | Write::Edit { .. }
            | Write::SetReviewers { .. }
            | Write::Close { .. }
            | Write::Reopen { .. }
            | Write::Merge { .. }
            | Write::Update { .. }
            | Write::DeleteBranch { .. } => return None,
        };
        Some((kind, What::Branch(path_parts(&branch))))
    }
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
