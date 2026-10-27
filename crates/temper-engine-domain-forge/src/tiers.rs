//! Tiers follow adopted landing branches (domain/after-landing.md, 2, 3 and 10).
use crate::{Domain, Limits, Name, Request, Stored, Tier, TierBranch, TierDecision, TierRow, What};
use alloc::boxed::Box;
use core::mem::size_of;
use skein_forge::api::{Commit, Repository, Write};
use skein_lib::{Env, List, Map, Queue, Token};
use temper_engine_domain_forge_landing as landing;
use temper_engine_domain_forge_tiers as policy;

/// Workflow, ignored-path count and paths, artifact (u16 lengths), then
/// format: `JUnit` = 0, lines = 1 (domain/landing.md, 6.1).
pub(crate) fn definition(tier: &Tier) -> Box<[u8]> {
    let mut length = tier
        .workflow
        .len()
        .checked_add(tier.artifact.len())
        .expect("bounded definition")
        .checked_add(7)
        .expect("three words and format");
    for path in &tier.ignored {
        length = length.checked_add(2).expect("path word").checked_add(path.len()).expect("bounded path");
    }
    let mut out = skein_lib::Writer::new(length);
    word(&mut out, tier.workflow.len());
    out.put(&tier.workflow).expect("sized workflow");
    word(&mut out, tier.ignored.len());
    for path in &tier.ignored {
        word(&mut out, path.len());
        out.put(path).expect("sized path");
    }
    word(&mut out, tier.artifact.len());
    out.put(&tier.artifact).expect("sized artifact");
    out.put(&[match tier.format {
        policy::Format::Junit => 0,
        policy::Format::Lines => 1,
    }])
    .expect("format byte");
    out.finish()
}
fn word(out: &mut skein_lib::Writer, value: usize) {
    out.put(&u16::try_from(value).expect("bounded definition word").to_be_bytes()).expect("sized word");
}

/// Transient trees are deliberately re-read after restart; resolved keys live
/// in family 25. One walk and one workflow read may coexist for a tier.
#[derive(Debug)]
pub(crate) struct Walk {
    owner: Token,
    commit: Commit,
    object: Commit,
    page: u32,
    trees: List<landing::Tree>,
    entries: List<landing::Entry>,
    next: Option<skein_lib::Time>,
}

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
    start_due_for(d, env, repository, None, out);
}
fn start_due_for(
    d: &mut Domain,
    env: &Env<Limits>,
    repository: Repository,
    only: Option<&[u8]>,
    out: &mut Queue<Request>,
) {
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
        if let Some(only) = only
            && only != tier.as_ref()
        {
            continue;
        }
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
                baseline: None,
                task: None,
                branch: TierBranch::Absent,
                probe: None,
                flight: None,
                result: None,
            },
        };
        if row.record.judged.is_none() && row.record.run.is_none() && row.baseline.is_none() {
            row.baseline = Some(crate::TierLanding { commit: tip, key: None });
            save(&row, out);
        }
        let mut ready = true;
        for landing in &row.pending {
            if landing.key.is_none() {
                ready = false;
            }
        }
        let (tip, tip_key) = match row.baseline {
            Some(baseline) => (baseline.commit, baseline.key),
            None if ready => match row.pending.last() {
                Some(landing) => (tip, landing.key),
                None => (tip, None),
            },
            None => (tip, None),
        };
        if let Some(tip_key) = tip_key
            && policy::due(&row.record, tip)
        {
            let pending = keyed_range(&mut row);
            let receipt_key = receipt_key(d, &row, tip_key);
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
                policy::start(&row.record, tip, tip_key, pending.clone(), None, &settings.verdict)
            else {
                unreachable!("without a receipt start dispatches the whole run")
            };
            row.record.run = Some(run);
            row.task = None;
            row.branch = TierBranch::Absent;
            row.probe = None;
            row.flight = None;
            row.result = None;
            match policy::start(&row.record, tip, tip_key, pending, cached, &settings.verdict) {
                policy::Next::Dispatch(next) | policy::Next::Held(next) => row.record.run = Some(next),
                policy::Next::Judged(verdict) => {
                    row.result = Some(crate::TierResult {
                        verdict,
                        run: receipt.expect("an immediate judgement uses a receipt").run,
                        received: true,
                        status: TierBranch::Absent,
                        applied: false,
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
        d.tier_rows.insert(key.clone(), row).expect("one row per repository and tier");
        walk(d, env, &key, out);
    }
}

/// A baseline covers no arrivals; its growing range waits for the next run.
fn keyed_range(row: &mut TierRow) -> Box<[policy::Landing]> {
    if row.baseline.take().is_some() {
        return Box::new([]);
    }
    let mut pending = List::with_capacity(u32::try_from(row.pending.len()).expect("bounded range"));
    for landing in &row.pending {
        pending
            .push(policy::Landing { commit: landing.commit, key: landing.key.expect("resolved range") })
            .expect("sized range");
    }
    row.pending = Box::new([]);
    pending.into_boxed()
}

fn walk(d: &mut Domain, env: &Env<Limits>, key: &(Repository, Box<[u8]>), out: &mut Queue<Request>) {
    if d.tier_walks.contains_key(key) || d.stopping {
        return;
    }
    let row = d.tier_rows.get(key).expect("tier to key");
    let mut commit = None;
    if let Some(baseline) = row.baseline
        && baseline.key.is_none()
    {
        commit = Some(baseline.commit);
    }
    if commit.is_none() {
        for landing in &row.pending {
            if landing.key.is_none() {
                commit = Some(landing.commit);
                break;
            }
        }
    }
    if let Some(commit) = commit {
        d.sequence = d.sequence.checked_add(1).expect("tier tree read sequence");
        let walk = Walk {
            owner: Token::new(d.sequence | (1_u64 << 59)),
            commit,
            object: commit,
            page: 1,
            trees: List::with_capacity(env.limits.trees_in_flight),
            entries: List::with_capacity(env.limits.key_entries),
            next: None,
        };
        ask_tree(d, env, key.clone(), walk, out);
    }
}

fn ask_tree(d: &mut Domain, env: &Env<Limits>, key: (Repository, Box<[u8]>), walk: Walk, out: &mut Queue<Request>) {
    let read = skein_forge::api::Read::Tree { object: walk.object, page: walk.page };
    let owner = walk.owner;
    let repository = key.0;
    d.tier_reads.insert(owner, key.clone()).expect("walk and follower per tier");
    d.tier_walks.insert(key, walk).expect("one walk per tier");
    crate::domain::child(d, env, skein_forge_client::Event::Read { owner, repository, read }, out);
}
fn tree_answer(
    d: &mut Domain,
    env: &Env<Limits>,
    key: (Repository, Box<[u8]>),
    answer: Result<skein_forge::api::Answer, skein_forge::api::Error>,
    out: &mut Queue<Request>,
) {
    let mut walk = d.tier_walks.remove(&key).expect("tree walk");
    if d.stopping {
        return;
    }
    if answer == Err(skein_forge::api::Error::Busy) {
        walk.next = Some(env.now.saturating_add(env.limits.client.poll));
        d.tier_walks.insert(key, walk).expect("replaces walk");
        return;
    }
    let mut keyed = landing::Keyed::TooLarge;
    if let Ok(skein_forge::api::Answer::Tree { entries, more }) = answer {
        let mut fits = true;
        for entry in entries {
            if walk
                .entries
                .push(landing::Entry {
                    name: entry.name,
                    mode: entry.mode,
                    object: entry.object,
                    kind: match entry.kind {
                        skein_forge::api::EntryKind::Blob => landing::EntryKind::Blob,
                        skein_forge::api::EntryKind::Tree => landing::EntryKind::Tree,
                        skein_forge::api::EntryKind::Commit => landing::EntryKind::Commit,
                    },
                })
                .is_err()
            {
                fits = false;
                break;
            }
        }
        if fits && more {
            if let Some(page) = walk.page.checked_add(1) {
                walk.page = page;
                ask_tree(d, env, key, walk, out);
                return;
            }
            fits = false;
        }
        if fits {
            let entries = core::mem::replace(&mut walk.entries, List::with_capacity(env.limits.key_entries));
            if walk.trees.push(landing::Tree { object: walk.object, entries: entries.into_boxed() }).is_ok() {
                let row = d.tier_rows.get(&key).expect("walk tier");
                let tier = configured(d, row);
                keyed = landing::key(
                    landing::Repository { forge: key.0.forge, repository: key.0.repository },
                    &tier.name,
                    &definition(&tier),
                    &tier.ignored,
                    walk.commit,
                    walk.trees.as_slice(),
                    &landing::KeyLimits { entries: env.limits.key_entries, path_bytes: env.limits.key_path_bytes },
                );
            }
        }
    }
    let resolved = match keyed {
        landing::Keyed::Need(object) if walk.trees.len() < env.limits.trees_in_flight => {
            walk.object = object;
            walk.page = 1;
            ask_tree(d, env, key, walk, out);
            return;
        }
        landing::Keyed::Key(key) => key,
        landing::Keyed::Need(_) | landing::Keyed::TooLarge => walk.commit,
    };
    let Some(row) = d.tier_rows.get_mut(&key) else { return };
    if let Some(baseline) = &mut row.baseline
        && baseline.commit == walk.commit
    {
        baseline.key = Some(resolved);
    }
    for landing in &mut row.pending {
        if landing.commit == walk.commit {
            landing.key = Some(resolved);
        }
    }
    save(row, out);
    start_due_for(d, env, key.0, Some(&key.1), out);
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
    pending.push(crate::TierLanding { commit, key: None }).expect("one incoming landing");
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
// domain/after-landing.md, 10: operation distinguishes dispatch, runner retry,
// probe creation and deletion; part identifies whole (0), rerun (1), probe (2+k).
fn part_number(part: &policy::Part) -> u16 {
    match part {
        policy::Part::Whole => 0,
        policy::Part::Rerun { .. } => 1,
        policy::Part::Probe { landing, .. } => u16::try_from(*landing)
            .expect("bounded range index").checked_add(2).expect("bounded probe part"),
    }
}
fn probe_branch(tier: &[u8], number: u64, landing: u32) -> Box<[u8]> {
    joined(&[&branch(tier, number), b".", skein_lib::Decimal::of(u64::from(landing)).as_bytes()])
}
fn part_branch(row: &TierRow) -> Box<[u8]> {
    let run = row.record.run.as_ref().expect("live run");
    match &run.part {
        policy::Part::Probe { landing, .. } => probe_branch(&row.tier, run.number, *landing),
        policy::Part::Whole | policy::Part::Rerun { .. } => branch(&row.tier, run.number),
    }
}
fn candidate(run: &policy::Run) -> (Commit, policy::Key, Option<&[policy::TestId]>) {
    match &run.part {
        policy::Part::Whole => (run.tip, run.key, None),
        policy::Part::Rerun { failed } => (run.tip, run.key, Some(failed)),
        policy::Part::Probe { landing, tests, .. } => {
            let landing = run.landings.get(usize::try_from(*landing).expect("range index")).expect("probe landing");
            let tests = match tests {
                policy::Selection::Tests(tests) => Some(tests.as_ref()),
                policy::Selection::All => None,
            };
            (landing.commit, landing.key, tests)
        }
    }
}
/// Whole inputs have landing's bound; the selection adds at most
/// failing_tests × (test_id_bytes + 1) bytes (domain/after-landing.md, 2, 4).
fn part_inputs(commit: Commit, key: policy::Key, tests: Option<&[policy::TestId]>) -> Box<[skein_forge::api::Input]> {
    let base = crate::landing::inputs(commit, key);
    let Some(tests) = tests else { return base };
    let mut inputs = List::with_capacity(3);
    for input in base {
        inputs.push(input).expect("two landing inputs");
    }
    let mut length = 0_usize;
    for test in tests {
        length = length.checked_add(test.len()).expect("bounded test ids").checked_add(1).expect("newline per id");
    }
    let mut value = skein_lib::Writer::new(length);
    for test in tests {
        value.put(test).expect("sized selection");
        value.put(b"\n").expect("newline per id");
    }
    inputs.push(skein_forge::api::Input { name: Box::from(b"temper_tests".as_slice()), value: value.finish() })
        .expect("one selection input");
    inputs.into_boxed()
}
fn following(d: &Domain, row: &TierRow) -> crate::runs::Follow {
    let run = row.record.run.as_ref().expect("live run");
    let task = row.task.expect("tier task");
    let (commit, key, tests) = candidate(run);
    let mut follow = crate::runs::follow(
        task,
        0,
        purpose(d, row, 2, part_number(&run.part)).identity().expect("tier purpose"),
        row.repository,
        configured(d, row).workflow,
        part_branch(row),
        commit,
        part_inputs(commit, key, tests),
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
        if let Some(report) = &flight.report {
            follow.run = Some(skein_forge::api::Run {
                id: report.id, number: report.number, url: report.url.clone(),
                workflow: follow.workflow.clone(), commit, event: skein_forge::api::Event::Dispatch,
                actor: 0, state: skein_forge::api::RunState::Completed,
                conclusion: Some(skein_forge::api::Conclusion::Failure), created: skein_lib::Time::ZERO,
                jobs: Box::new([]),
            });
            follow.outcome = Some(landing::Verdict::Failed);
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
        fault: match follow.outcome {
            Some(landing::Verdict::Fault(_)) => true,
            Some(landing::Verdict::Failed | landing::Verdict::Passed | landing::Verdict::Running) | None => false,
        },
        report: match follow.outcome {
            Some(landing::Verdict::Failed) => {
                let run = follow.run.as_ref().expect("failed workflow");
                Some(crate::ReceiptRun { id: run.id, number: run.number, url: run.url.clone() })
            }
            Some(landing::Verdict::Fault(_) | landing::Verdict::Passed | landing::Verdict::Running) | None => None,
        },
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
fn advance(d: &Domain, row: &mut TierRow, follow: &mut crate::runs::Follow, outcome: policy::Outcome) {
    let run = row.record.run.as_ref().expect("followed run");
    let project = d.repositories.get(&row.repository).expect("adopted tier").project;
    let settings = d.tier_settings.get(&project).expect("tier settings");
    match policy::step(&row.record, run, outcome, &settings.verdict) {
        policy::Next::Dispatch(next) => {
            if next.part != run.part {
                row.record.run = Some(next);
                row.flight = None;
                *follow = following(d, row);
            } else {
                let attempt = u16::try_from(next.faults).expect("bounded retry").checked_add(1).expect("retry attempt");
                row.record.run = Some(next);
                crate::runs::retry(follow, attempt);
                if follow.start.is_none() {
                    follow.since = None;
                }
            }
            remember(row, follow);
        }
        policy::Next::Held(run) => {
            row.record.run = Some(run);
            remember(row, follow);
            // The terminal fault is already counted; it must not be counted again.
            follow.outcome = None;
        }
        policy::Next::Judged(verdict) => {
            let run = match &row.flight {
                Some(flight) if flight.report.is_some() => flight.report.clone().expect("read report"),
                Some(_) | None => {
                    let run = follow.run.as_ref().expect("a judged workflow has a run");
                    crate::ReceiptRun { id: run.id, number: run.number, url: run.url.clone() }
                }
            };
            row.result = Some(crate::TierResult {
                verdict, run, received: false, status: TierBranch::Absent, applied: false,
            });
        }
    }
}
fn observed(d: &Domain, row: &mut TierRow, follow: &mut crate::runs::Follow) {
    let outcome = match follow.outcome {
        Some(landing::Verdict::Passed) => policy::Outcome::Passed,
        Some(landing::Verdict::Fault(_)) => policy::Outcome::Fault,
        Some(landing::Verdict::Failed | landing::Verdict::Running) | None => return,
    };
    advance(d, row, follow, outcome);
}
fn report_read(d: &Domain, row: &TierRow, env: &Env<Limits>) -> Option<skein_forge::api::Read> {
    let report = row.flight.as_ref()?.report.as_ref()?;
    Some(skein_forge::api::Read::Artifact {
        run: report.id, name: configured(d, row).artifact, max_bytes: env.limits.client.answer_bytes,
    })
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
                if let Some(walk) = d.tier_walks.get(&key)
                    && walk.owner == *token
                {
                    continue;
                }
                owner = Some(*token);
                break;
            }
        }
        if let Some(owner) = owner
            && let Some(follow) = d.tier_follows.get(&key)
            && let Some(read) = report_read(d, d.tier_rows.get(&key).expect("saved tier"), env).or(follow.read())
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
            observed(d, row, &mut follow);
            save(row, out);
        }
        d.tier_follows.insert(key.clone(), follow).expect("one follow per tier");
    }
    if let Some(result) = &row.result {
        if result.status == TierBranch::Made && !result.applied {
            apply(d, env, row, out);
        }
    }
    let run = row.record.run.as_ref().expect("checked live run");
    if run.faults > d.tier_settings.get(&project).expect("settings").verdict.retries {
        out.push(Request::TierDecision { task, decision: TierDecision::Held });
        return;
    }
    // Delete each completed probe before another probe can be made. Its index
    // remains durable even after the policy has advanced to the next part.
    let cleanup = match &row.probe {
        Some(probe) => row.result.is_some() || match &run.part {
            policy::Part::Probe { landing, .. } => *landing != probe.landing,
            policy::Part::Whole | policy::Part::Rerun { .. } => true,
        },
        None => false,
    };
    if cleanup {
        let probe = row.probe.as_ref().expect("probe to delete");
        if probe.branch == TierBranch::Made && !d.entries.contains_key(&entry) && d.entries.len() < env.limits.entries {
            let landing = probe.landing;
            let write = Write::DeleteBranch { branch: probe_branch(&row.tier, run.number, landing) };
            row.probe.as_mut().expect("probe").branch = TierBranch::Pending { entry };
            effect(d, row, entry, write, 6, u16::try_from(landing).expect("range index").checked_add(2).expect("part"), out);
        } else {
            wait(task, out);
        }
        return;
    }
    if let Some(result) = &row.result {
        if result.applied {
            if row.branch == TierBranch::Absent {
                finish(d, env, key, row, out);
            } else if row.branch == TierBranch::Made && !d.entries.contains_key(&entry) && d.entries.len() < env.limits.entries {
                let write = Write::DeleteBranch { branch: branch(&row.tier, run.number) };
                row.branch = TierBranch::Pending { entry };
                effect(d, row, entry, write, 6, 0, out);
            } else {
                wait(task, out);
            }
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
        if let policy::Part::Probe { landing, .. } = &run.part {
            if row.probe.is_none() {
                let write = Write::CreateBranch { branch: part_branch(row), commit: candidate(run).0 };
                let part = part_number(&run.part);
                row.probe = Some(crate::TierProbeBranch { landing: *landing, branch: TierBranch::Pending { entry } });
                effect(d, row, entry, write, 4, part, out);
                return;
            }
            if row.probe.as_ref().expect("probe branch").branch != TierBranch::Made {
                wait(task, out);
                return;
            }
        }
        let mut follow = match d.tier_follows.remove(key) {
            Some(follow) => follow,
            None => following(d, row),
        };
        if !follow.waiting && !follow.writes.is_empty() {
            let write = follow.write(entry).expect("pending write");
            let operation = if let Write::Rerun { .. } = &write { 3 }
                else if let policy::Part::Probe { .. } = &run.part { 5 } else { 2 };
            let part = part_number(&run.part);
            remember(row, &follow);
            effect(d, row, entry, write, operation, part, out);
        } else if !reading(d, key) {
            if report_read(d, row, env).or(follow.read()).is_some() {
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
    for (owner, candidate) in &d.tier_reads {
        if candidate == key {
            if let Some(walk) = d.tier_walks.get(key)
                && walk.owner == *owner
            {
                continue;
            }
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
fn apply(d: &mut Domain, env: &Env<Limits>, row: &mut TierRow, out: &mut Queue<Request>) {
    let result = row.result.as_ref().expect("status posted").clone();
    let run = row.record.run.as_ref().expect("judged range");
    let mut landings = List::with_capacity(u32::try_from(run.landings.len()).expect("bounded range"));
    for (landing, blame) in run.landings.iter().zip(&result.verdict.landings) {
        landings.push((landing.commit, blame.clone())).expect("one blame per landing");
    }
    crate::verification::judged(d, row.repository, &row.tier, &result.verdict, landings.as_slice(), out);
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
    row.result.as_mut().expect("judged result").applied = true;
    save(row, out);
}
fn finish(
    d: &mut Domain,
    env: &Env<Limits>,
    key: &(Repository, Box<[u8]>),
    row: &mut TierRow,
    out: &mut Queue<Request>,
) {
    let result = row.result.take().expect("judgement applied and branches deleted");
    let run = row.record.run.as_ref().expect("judged range");
    let mut cleared = 0_u64;
    for blame in &result.verdict.landings {
        if *blame == policy::Blame::Cleared {
            cleared = cleared.checked_add(1).expect("bounded range");
        }
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
    if let Some(walk) = d.tier_walks.get(&key)
        && walk.owner == owner
    {
        tree_answer(d, env, key, answer, out);
        return;
    }
    let Some(mut row) = d.tier_rows.remove(&key) else { return };
    let mut follow = d.tier_follows.remove(&key).expect("read's follower");
    let reporting = match &row.flight { Some(flight) => flight.report.is_some(), None => false };
    let writer = d.writers.get(&row.repository.forge).copied().unwrap_or(0);
    let listing = if let Ok(skein_forge::api::Answer::Runs { .. }) = &answer { true } else { false };
    let ended = if reporting {
        if answer == Err(skein_forge::api::Error::Busy) {
            false
        } else {
            let tier = configured(d, &row);
            let limits = policy::Limits { failing_tests: env.limits.failing_tests, test_id_bytes: env.limits.test_id_bytes };
            let outcome = match answer {
                // A cut report cannot name every failure: blame by landing alone
                // (domain/after-landing.md, 4), even when its prefix is parseable.
                Ok(skein_forge::api::Answer::File { truncated: true, .. }) => policy::Outcome::Failed(policy::Failures::Many),
                Ok(skein_forge::api::Answer::File { bytes, .. }) => policy::failed(Some(policy::report(tier.format, &bytes, &limits))),
                Ok(_) | Err(_) => policy::failed(None),
            };
            advance(d, &mut row, &mut follow, outcome);
            true
        }
    } else {
        let ended = follow.answered(answer, writer, env.now, env.limits.client.lifetime).is_some();
        observed(d, &mut row, &mut follow);
        ended
    };
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
    for (_, walk) in &d.tier_walks {
        if let Some(at) = walk.next {
            next = Some(match next {
                Some(old) => old.min(at),
                None => at,
            });
        }
    }
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
    let mut due = None;
    for (key, walk) in &d.tier_walks {
        if let Some(at) = walk.next
            && at <= env.now
        {
            due = Some(key.clone());
            break;
        }
    }
    if let Some(key) = due {
        let mut walk = d.tier_walks.remove(&key).expect("due walk");
        walk.next = None;
        ask_tree(d, env, key, walk, out);
        return;
    }
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
            row.branch = if row.result.is_some() { TierBranch::Made } else { TierBranch::Absent };
        }
        if let Some(probe) = &mut row.probe
            && probe.branch == (TierBranch::Pending { entry })
        {
            probe.branch = TierBranch::Made;
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
            || match &row.probe { Some(probe) => probe.branch == (TierBranch::Pending { entry }), None => false }
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
        let deleting = match d.entries.get(&entry) {
            Some(saved) => if let Write::DeleteBranch { .. } = &saved.entry.effect.write { true } else { false },
            None => false,
        };
        if row.branch == pending && deleting {
            row.branch = if made { TierBranch::Absent } else { TierBranch::Made };
        }
        if let Some(probe) = &mut row.probe
            && probe.branch == pending
        {
            if deleting && made {
                row.probe = None;
            } else if made || deleting {
                probe.branch = TierBranch::Made;
            } else {
                row.probe = None;
            }
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
        .checked_add(array_bytes(row.pending.len(), size_of::<crate::TierLanding>())?)?;
    bytes = bytes.checked_add(failures_bytes(&row.record.failing)?)?;
    for flake in &row.record.flakes {
        bytes = bytes
            .checked_add(u64::try_from(size_of::<policy::Flake>()).ok()?)?
            .checked_add(u64::try_from(flake.test.len()).ok()?)?;
    }
    if let Some(flight) = &row.flight {
        if let Some(report) = &flight.report {
            bytes = bytes.checked_add(u64::try_from(report.url.len()).ok()?)?;
        }
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
        .checked_add(landings.checked_mul(
            u64::try_from(size_of::<policy::Landing>().checked_add(size_of::<crate::TierLanding>())?).ok()?,
        )?)?
        .checked_add(tests.checked_mul(5)?)?
        .checked_add(u64::from(l.failing_tests).checked_mul(u64::try_from(size_of::<policy::Flake>()).ok()?)?)?
        .checked_add(landings.checked_mul(u64::try_from(size_of::<policy::Blame>()).ok()?.checked_add(tests)?)?)?;
    // Two landing inputs and the bounded selection coexist in each dispatch.
    let inputs = array_bytes(3, size_of::<skein_forge::api::Input>())?
        .checked_add(u64::try_from(b"temper_commit".len().checked_add(b"temper_key".len())?).ok()?)?
        .checked_add(128)?
        .checked_add(u64::try_from(b"temper_tests".len()).ok()?)?
        .checked_add(u64::from(l.failing_tests).checked_mul(u64::from(l.test_id_bytes).checked_add(1)?)?)?;
    // Branch prefix/separator (13), bounded tier name, and u64's decimal (20).
    let dispatch = u64::try_from(crate::WORKFLOW_BYTES)
        .ok()?
        .checked_add(13 + 20 + 11)?
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
        .checked_add(4096)?; // Result and a failed part awaiting its report can each retain a run URL.
    // Match the landing walk's tree, entry and pure-key scratch bounds. Each
    // tier may key incoming landings while its workflow is still running.
    let entry = u64::try_from(size_of::<landing::Entry>()).ok()?.checked_add(u64::from(l.name_bytes))?;
    let trees = u64::from(l.trees_in_flight).checked_mul(
        u64::try_from(size_of::<landing::Tree>()).ok()?.checked_add(u64::from(l.key_entries).checked_mul(entry)?)?,
    )?;
    let nodes = l.key_entries.checked_mul(l.trees_in_flight)?.checked_add(1)?;
    let scratch = Map::<Box<[u8]>, [u8; 64]>::worst_case(nodes)?
        .checked_add(Map::<[u8; 32], ()>::worst_case(l.trees_in_flight)?)?
        .checked_add(
            u64::from(nodes)
                .checked_mul(u64::from(l.trees_in_flight.checked_add(1)?).checked_mul(u64::from(l.name_bytes))?)?,
        )?;
    // The configured tier clone and its length-prefixed definition coexist
    // during hashing, as do the page being accumulated and supplied trees.
    let definition = u64::try_from(crate::WORKFLOW_BYTES.checked_add(crate::CHECK_NAME_BYTES)?.checked_add(7)?)
        .ok()?
        .checked_add(u64::from(l.ignored_paths).checked_mul(u64::from(l.name_bytes).checked_add(2)?)?)?;
    let config =
        u64::try_from(crate::WORKFLOW_BYTES.checked_add(crate::CHECK_NAME_BYTES.checked_mul(2)?)?).ok()?.checked_add(
            u64::from(l.ignored_paths)
                .checked_mul(u64::try_from(size_of::<Box<[u8]>>()).ok()?.checked_add(u64::from(l.name_bytes))?)?,
        )?;
    let walk_payload = trees
        .checked_add(u64::from(l.key_entries).checked_mul(entry)?)?
        .checked_add(scratch)?
        .checked_add(definition)?
        .checked_add(config)?;
    Map::<(Repository, Box<[u8]>), TierRow>::worst_case(rows)?
        .checked_add(Map::<(Repository, Box<[u8]>), Walk>::worst_case(rows)?)?
        .checked_add(Map::<(Repository, Box<[u8]>), crate::runs::Follow>::worst_case(rows)?)?
        .checked_add(Map::<Token, (Repository, Box<[u8]>)>::worst_case(rows.checked_mul(2)?)?)?
        .checked_add(
            u64::from(rows).checked_mul(
                payload
                    .checked_add(u64::try_from(crate::CHECK_NAME_BYTES).ok()?.checked_mul(5)?)?
                    .checked_add(follow_payload)?
                    .checked_add(walk_payload)?
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
        let whole = branch(&row.tier, run.number);
        let current = part_branch(row);
        let (commit, key, tests) = candidate(run);
        match &entry.effect.write {
            Write::CreateBranch { branch: name, commit: at } if
                (row.branch == pending && *name == whole && *at == run.tip)
                || (match &row.probe { Some(probe) => probe.branch == pending, None => false } && *name == current && *at == commit) =>
                Some((9, What::Branch(path_parts(name)))),
            Write::DeleteBranch { branch: name } if
                (row.branch == pending && *name == whole && row.result.as_ref()?.applied)
                || match &row.probe {
                    Some(probe) => probe.branch == pending && *name == probe_branch(&row.tier, run.number, probe.landing),
                    None => false,
                } => Some((15, What::Branch(path_parts(name)))),
            Write::Dispatch { workflow, branch: name, inputs } if
                *workflow == configured(self, row).workflow && *name == current
                && *inputs == part_inputs(commit, key, tests) && row.flight.as_ref()?.entry == entry.number =>
                Some((16, What::Branch(path_parts(name)))),
            Write::Rerun { attempt } if attempt.head == commit && row.flight.as_ref()?.entry == entry.number =>
                Some((17, What::Branch(path_parts(&current)))),
            Write::Status { .. } if row.result.as_ref()?.status == pending
                && entry.effect.write == status(&row.tier, row.result.as_ref()?) => Some((6, What::Repository)),
            Write::CreateBranch { .. } | Write::DeleteBranch { .. } | Write::Dispatch { .. } | Write::Rerun { .. }
            | Write::Status { .. } | Write::CreateIssue { .. } | Write::OpenPull { .. } | Write::Post { .. }
            | Write::Review { .. } | Write::Edit { .. } | Write::SetReviewers { .. } | Write::Close { .. }
            | Write::Reopen { .. } | Write::Merge { .. } | Write::Update { .. } => None,
        }
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
