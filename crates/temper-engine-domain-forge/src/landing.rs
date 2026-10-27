//! Top-owned landing rows, coherent reads, receipts and effects (domain/landing.md, 2–15).
use crate::domain::{child, emit, keep_entry};
use crate::{
    Check, Domain, Key, LandingEntry, LandingHeard, LandingRow, Limits, Operation, Receipt, ReceiptKey, ReceiptRun,
    RepositoryGates, Request, Stored, runs,
};
use alloc::boxed::Box;
use core::mem::size_of;
use skein_forge::api;
use skein_lib::{Env, List, Map, Queue, Token, Wall};
use temper_engine_domain_forge_change as change;
use temper_engine_domain_forge_landing as policy;

type Position = (u64, u16, u16);
type RepositoryGate = (api::Repository, Box<[u8]>);
type KeyAt = (api::Commit, policy::Key);
#[derive(Debug)]
pub(crate) struct State {
    pub(crate) rows: Map<u64, LandingRow>,
    pub(crate) receipts: Map<ReceiptKey, Receipt>,
    pub(crate) projects: Map<u32, (change::Way, Box<[RepositoryGates]>)>,
    pub(crate) pending: Map<Token, Pending>,
    pub(crate) keys: Map<Position, (api::Commit, policy::Key)>,
    pub(crate) main_keys: Map<RepositoryGate, KeyAt>,
    pub(crate) flights: Map<ReceiptKey, runs::Follow>,
    pub(crate) faults: Map<Position, runs::Follow>,
    pub(crate) results: Map<(u64, u16, u16, u8), policy::Status>,
    pub(crate) limits: Limits,
}
#[derive(Debug)]
pub(crate) struct Pending {
    task: u64,
    entry: u64,
    heard: LandingHeard,
    facts: Box<[policy::MemberFacts]>,
    stage: Stage,
    polled: List<ReceiptKey>,
    trees: List<policy::Tree>,
    entries: List<policy::Entry>,
    woken: bool,
}
#[derive(Clone, Debug)]
enum Stage {
    Branch(u16),
    Main(u16),
    Compare(u16),
    Pull(u16),
    Collect,
    Run { key: ReceiptKey, pages: u32 },
    Tree { member: u16, gate: u16, candidate: api::Commit, object: api::Commit, page: u32, main: bool },
}
fn slots(l: &Limits) -> Option<u32> {
    l.landings.checked_mul(l.landing_members)?.checked_mul(l.gates)
}
fn result_slots(l: &Limits) -> Option<u32> {
    slots(l)?.checked_mul(4)?.checked_add(l.landings.checked_mul(l.landing_members)?)
}
fn receipt_slots(l: &Limits) -> Option<u32> {
    l.repositories.checked_mul(l.gates)?.checked_mul(l.receipts_per_gate)
}
impl State {
    pub fn new(l: &Limits) -> Self {
        let slots = slots(l).expect("landing bounds checked");
        Self {
            rows: Map::with_capacity(l.landings),
            receipts: Map::with_capacity(receipt_slots(l).expect("receipt bounds")),
            projects: Map::with_capacity(l.judge_projects),
            pending: Map::with_capacity(l.landings),
            keys: Map::with_capacity(slots),
            main_keys: Map::with_capacity(l.repositories.checked_mul(l.gates).expect("gate bounds")),
            flights: Map::with_capacity(slots),
            faults: Map::with_capacity(slots),
            results: Map::with_capacity(result_slots(l).expect("effect bounds")),
            limits: *l,
        }
    }
    pub fn way(&self, project: u32) -> change::Way {
        match self.projects.get(&project) {
            Some((way, _)) => *way,
            None => change::Way::Queue,
        }
    }
    fn gates(&self, project: u32, repository: api::Repository) -> &[policy::Gate] {
        if let Some((_, repositories)) = self.projects.get(&project) {
            for row in repositories {
                if row.repository == repository {
                    return &row.gates;
                }
            }
        }
        &[]
    }
}
impl Domain {
    /// A project's way and repository test gates (domain/landing.md, 13.1).
    pub fn project_landing(&mut self, project: u32, way: change::Way, gates: Box<[RepositoryGates]>) -> bool {
        let l = &self.landing.limits;
        if gates.len() > usize::try_from(l.repositories).expect("u32 fits") {
            return false;
        }
        for (index, repository) in gates.iter().enumerate() {
            if way == change::Way::Turn
                && let Some(forge) = self.configured.get(&repository.repository.forge)
                && forge.kind == crate::Kind::GitHub
            {
                return false;
            }
            for prior in gates.get(..index).expect("preceding positions") {
                if prior.repository == repository.repository {
                    return false;
                }
            }
            if repository.gates.len() > usize::try_from(l.gates).expect("u32 fits") {
                return false;
            }
            for (index, gate) in repository.gates.iter().enumerate() {
                if gate.name.is_empty()
                    || gate.name.len() > policy::CHECK_NAME_BYTES
                    || gate.workflow.is_empty()
                    || gate.workflow.len() > policy::WORKFLOW_BYTES
                    || gate.ignored.len() > usize::try_from(l.ignored_paths).expect("u32 fits")
                {
                    return false;
                }
                for byte in &gate.name {
                    if !byte.is_ascii_alphanumeric() && *byte != b'_' && *byte != b'-' {
                        return false;
                    }
                }
                for prior in repository.gates.get(..index).expect("preceding positions") {
                    if prior.name == gate.name {
                        return false;
                    }
                }
                for path in &gate.ignored {
                    if path.is_empty()
                        || path.len() > usize::try_from(l.name_bytes).expect("u32 fits")
                        || path.first() == Some(&b'/')
                        || path.last() == Some(&b'/')
                    {
                        return false;
                    }
                }
            }
        }
        self.landing.projects.insert(project, (way, gates)).is_ok()
    }
    #[must_use]
    pub fn landing(&self, task: u64) -> Option<&LandingRow> {
        self.landing.rows.get(&task)
    }
    #[must_use]
    pub fn receipt(&self, key: &ReceiptKey) -> Option<&Receipt> {
        self.landing.receipts.get(key)
    }
}
fn repository(member: &policy::Member) -> api::Repository {
    api::Repository { forge: member.repository.forge, repository: member.repository.repository }
}
fn gate_key(row: &LandingRow, member: u16, _gate: u16, name: &[u8], key: policy::Key) -> ReceiptKey {
    let m = row.landing.members.get(usize::from(member)).expect("member");
    ReceiptKey { project: row.landing.project, repository: repository(m), check: Check::Gate(Box::from(name)), key }
}
fn proof_key(proof: policy::Proof) -> Option<policy::Key> {
    match proof {
        policy::Proof::Unkeyed => None,
        policy::Proof::Keyed { key }
        | policy::Proof::Dispatched { key, .. }
        | policy::Proof::Passed { key }
        | policy::Proof::Failed { key } => Some(key),
    }
}
fn save(d: &mut Domain, row: LandingRow, out: &mut Queue<Request>) {
    d.landing.rows.insert(row.landing.task, row.clone()).expect("admitted landing");
    emit(out, Request::Save { record: Stored::Landing(row), release: None, read_afresh: None });
}
pub(crate) fn register(d: &mut Domain, env: &Env<Limits>, row: LandingRow, out: &mut Queue<Request>) {
    let task = row.landing.task;
    let mut valid = !d.landing.rows.contains_key(&task)
        && !row.landing.members.is_empty()
        && row.landing.members.len() <= usize::try_from(env.limits.landing_members).expect("u32 fits")
        && row.asks.len() == row.landing.members.len()
        && row.entries.is_empty();
    for (index, member) in row.landing.members.iter().enumerate() {
        let repo = repository(member);
        let gates = d.landing.gates(row.landing.project, repo);
        valid &= member.gates.len() == gates.len()
            && member.gates.len() <= usize::try_from(env.limits.gates).expect("u32 fits");
        valid &= match d.changes.get(&member.change) {
            Some(change) => {
                change.repository == repo && change.pull == Some(member.pull) && change.change.landing.is_none()
            }
            None => false,
        };
        valid &= match d.repositories.get(&repo) {
            Some(repo) => repo.project == row.landing.project,
            None => false,
        };
        for prior in row.landing.members.get(..index).expect("preceding positions") {
            valid &= prior.repository != member.repository && prior.change != member.change;
        }
    }
    if !valid || d.landing.way(row.landing.project) != change::Way::Turn {
        emit(out, Request::Refused { task });
        return;
    }
    if d.landing.rows.len() == env.limits.landings && !retire_finished(d, out) {
        emit(out, Request::Refused { task });
        return;
    }
    for member in &row.landing.members {
        let change = d.changes.get_mut(&member.change).expect("validated member");
        change.change.landing = Some(task);
        emit(out, Request::Save { record: Stored::Change(change.clone()), release: None, read_afresh: None });
    }
    save(d, row, out);
}
fn fresh(gates: &[policy::Gate]) -> policy::MemberFacts {
    let mut facts = List::with_capacity(u32::try_from(gates.len()).expect("bounded gates"));
    for gate in gates {
        facts
            .push(policy::GateFacts {
                blocking: gate.blocking,
                key: None,
                receipt: policy::Status::Unknown,
                flight: policy::Flight::None,
                posting: policy::Status::Unknown,
            })
            .expect("bounded gates");
    }
    policy::MemberFacts {
        ready: false,
        cancelled: false,
        head: [0; 32],
        main: [0; 32],
        contains_main: policy::Status::Unknown,
        asked: false,
        merge: policy::Status::Unknown,
        merged: None,
        gates: facts.into_boxed(),
    }
}
pub(crate) fn step(
    d: &mut Domain,
    env: &Env<Limits>,
    task: u64,
    entry: u64,
    heard: LandingHeard,
    out: &mut Queue<Request>,
) {
    if d.stopping {
        return;
    }
    let mut existing = None;
    for (token, pending) in &d.landing.pending {
        if pending.task == task {
            existing = Some(*token);
        }
    }
    if let Some(token) = existing {
        let pending = d.landing.pending.get_mut(&token).expect("pending step");
        pending.entry = entry;
        pending.heard = heard;
        pending.woken = true;
        return;
    }
    let Some(row) = d.landing.rows.get(&task) else {
        emit(out, Request::Refused { task });
        return;
    };
    let mut facts = List::with_capacity(env.limits.landing_members);
    for member in &row.landing.members {
        facts.push(fresh(d.landing.gates(row.landing.project, repository(member)))).expect("admitted members");
    }
    let Some(first) = row.landing.members.first() else {
        return;
    };
    let Some(change) = d.changes.get(&first.change) else {
        return;
    };
    let repo = change.repository;
    let read = api::Read::Branch { branch: change.branch.clone() };
    d.sequence = d.sequence.checked_add(1).expect("read sequence");
    let owner = Token::new(d.sequence | (1_u64 << 61));
    d.landing
        .pending
        .insert(
            owner,
            Pending {
                task,
                entry,
                heard,
                facts: facts.into_boxed(),
                stage: Stage::Branch(0),
                polled: List::with_capacity(slots(&env.limits).expect("bounded gates")),
                trees: List::with_capacity(env.limits.trees_in_flight),
                entries: List::with_capacity(env.limits.key_entries),
                woken: false,
            },
        )
        .expect("one chain per landing");
    child(d, env, skein_forge_client::Event::Read { owner, repository: repo, read }, out);
}
fn ask(
    d: &mut Domain,
    env: &Env<Limits>,
    owner: Token,
    pending: Pending,
    repository: api::Repository,
    read: api::Read,
    out: &mut Queue<Request>,
) {
    d.landing.pending.insert(owner, pending).expect("replaces pending chain");
    child(d, env, skein_forge_client::Event::Read { owner, repository, read }, out);
}
fn wait(task: u64, env: &Env<Limits>, out: &mut Queue<Request>) {
    emit(
        out,
        Request::LandingDecision {
            task,
            decision: policy::Decision::Wait {
                until: Some(Wall::from_nanos(env.wall.as_nanos().saturating_add(env.limits.client.poll.as_nanos()))),
            },
            entry: None,
        },
    );
}
#[expect(clippy::too_many_lines, reason = "one exhaustive bounded landing read chain")]
#[expect(clippy::match_like_matches_macro, reason = "the foundation forbids matches macros in step code")]
pub(crate) fn answered(
    d: &mut Domain,
    env: &Env<Limits>,
    owner: Token,
    answer: Result<api::Answer, api::Error>,
    out: &mut Queue<Request>,
) {
    let Some(mut p) = d.landing.pending.remove(&owner) else {
        return;
    };
    if d.stopping {
        return;
    }
    if answer == Err(api::Error::Busy) {
        wait(p.task, env, out);
        return;
    }
    let Some(row) = d.landing.rows.get(&p.task).cloned() else {
        return;
    };
    match p.stage.clone() {
        Stage::Branch(index) | Stage::Main(index) | Stage::Compare(index) | Stage::Pull(index) => {
            let member = row.landing.members.get(usize::from(index)).expect("member");
            let Some(change) = d.changes.get(&member.change) else {
                return;
            };
            let repo = change.repository;
            let fact = p.facts.get_mut(usize::from(index)).expect("member fact");
            let read = match p.stage {
                Stage::Branch(_) => {
                    let Ok(api::Answer::Commit(head)) = answer else {
                        wait(p.task, env, out);
                        return;
                    };
                    fact.head = head;
                    p.stage = Stage::Main(index);
                    api::Read::Branch { branch: change.base.clone() }
                }
                Stage::Main(_) => {
                    let Ok(api::Answer::Commit(main)) = answer else {
                        wait(p.task, env, out);
                        return;
                    };
                    fact.main = main;
                    p.stage = Stage::Compare(index);
                    api::Read::Compare { before: main, after: fact.head }
                }
                Stage::Compare(_) => {
                    fact.contains_main = match answer {
                        Ok(api::Answer::Compare { before, after, contains_before, .. })
                            if before == fact.main && after == fact.head =>
                        {
                            if contains_before {
                                policy::Status::Passed
                            } else {
                                policy::Status::Failed
                            }
                        }
                        Ok(_) | Err(_) => policy::Status::Unknown,
                    };
                    p.stage = Stage::Pull(index);
                    api::Read::Pull { number: member.pull }
                }
                Stage::Pull(_) => {
                    let Ok(api::Answer::Pull(pull)) = answer else {
                        wait(p.task, env, out);
                        return;
                    };
                    fact.merged = pull.merged;
                    fact.ready = if let change::State::Ready { .. } = change.change.state { true } else { false };
                    fact.asked = row.asks.get(usize::from(index)).expect("member ask").is_some();
                    fact.cancelled = p.heard.cancelled;
                    if usize::from(index) + 1 < row.landing.members.len() {
                        let index = index.checked_add(1).expect("member index");
                        let next = row.landing.members.get(usize::from(index)).expect("member");
                        let change = d.changes.get(&next.change).expect("member's change");
                        let repo = change.repository;
                        let read = api::Read::Branch { branch: change.branch.clone() };
                        p.stage = Stage::Branch(index);
                        ask(d, env, owner, p, repo, read, out);
                        return;
                    }
                    p.stage = Stage::Collect;
                    collect(d, env, owner, p, out);
                    return;
                }
                Stage::Collect | Stage::Run { .. } | Stage::Tree { .. } => unreachable!("member stage"),
            };
            ask(d, env, owner, p, repo, read, out);
        }
        Stage::Run { key, pages } => {
            let writer = d.writers.get(&key.repository.forge).copied().unwrap_or(0);
            let flight = d.landing.flights.get_mut(&key).expect("in-flight read");
            let listing = if let Ok(api::Answer::Runs { .. }) = &answer { true } else { false };
            let ended = flight.answered(answer, writer, env.now, env.limits.client.lifetime);
            let next = if listing && pages < env.limits.client.rows { flight.read() } else { None };
            if let Some(ended) = ended {
                match ended.outcome {
                    policy::Verdict::Passed | policy::Verdict::Failed => {
                        let run = ended.run.expect("a verdict has a run");
                        let receipt = Receipt {
                            passed: ended.outcome == policy::Verdict::Passed,
                            candidate: flight.commit,
                            ended: env.wall,
                            run: ReceiptRun { id: run.id, number: run.number, url: run.url },
                        };
                        put_receipt(d, env, key.clone(), receipt, out);
                        finish_flight(d, &key, out);
                    }
                    policy::Verdict::Fault(_) => {
                        let mut position = None;
                        for (index, member) in row.landing.members.iter().enumerate() {
                            if repository(member) == key.repository {
                                for (number, proof) in member.gates.iter().enumerate() {
                                    if proof_key(proof.proof) == Some(key.key) {
                                        position = Some((
                                            p.task,
                                            u16::try_from(index).expect("member"),
                                            u16::try_from(number).expect("gate"),
                                        ));
                                    }
                                }
                            }
                        }
                        if let Some(position) = position
                            && let Some(flight) = d.landing.flights.remove(&key)
                        {
                            d.landing.faults.insert(position, flight).expect("bounded faults");
                        }
                    }
                    policy::Verdict::Running => {}
                }
            } else if let Some(read) = next {
                p.stage = Stage::Run { key: key.clone(), pages: pages.checked_add(1).expect("bounded pages") };
                ask(d, env, owner, p, key.repository, read, out);
                return;
            }
            p.polled.push(key).expect("one poll per flight");
            p.stage = Stage::Collect;
            collect(d, env, owner, p, out);
        }
        Stage::Tree { member, gate, candidate, object, page, main } => {
            let Ok(api::Answer::Tree { entries, more }) = answer else {
                wait(p.task, env, out);
                return;
            };
            for entry in entries {
                if p.entries
                    .push(policy::Entry {
                        name: entry.name,
                        mode: entry.mode,
                        object: entry.object,
                        kind: match entry.kind {
                            api::EntryKind::Blob => policy::EntryKind::Blob,
                            api::EntryKind::Tree => policy::EntryKind::Tree,
                            api::EntryKind::Commit => policy::EntryKind::Commit,
                        },
                    })
                    .is_err()
                {
                    emit(out, Request::Refused { task: p.task });
                    return;
                }
            }
            let repo = repository(row.landing.members.get(usize::from(member)).expect("member"));
            if more {
                let Some(page) = page.checked_add(1) else {
                    emit(out, Request::Refused { task: p.task });
                    return;
                };
                p.stage = Stage::Tree { member, gate, candidate, object, page, main };
                ask(d, env, owner, p, repo, api::Read::Tree { object, page }, out);
                return;
            }
            let entries = core::mem::replace(&mut p.entries, List::with_capacity(env.limits.key_entries));
            if p.trees.push(policy::Tree { object, entries: entries.into_boxed() }).is_err() {
                emit(out, Request::Refused { task: p.task });
                return;
            }
            let configured =
                d.landing.gates(row.landing.project, repo).get(usize::from(gate)).expect("configured gate").clone();
            let keyed = policy::key(
                policy::Repository { forge: repo.forge, repository: repo.repository },
                &configured.name,
                &policy::definition(&configured),
                &configured.ignored,
                candidate,
                p.trees.as_slice(),
                &policy::KeyLimits { entries: env.limits.key_entries, path_bytes: env.limits.key_path_bytes },
            );
            match keyed {
                policy::Keyed::Need(object) => {
                    p.stage = Stage::Tree { member, gate, candidate, object, page: 1, main };
                    ask(d, env, owner, p, repo, api::Read::Tree { object, page: 1 }, out);
                }
                policy::Keyed::Key(key) => {
                    if main {
                        d.landing
                            .main_keys
                            .insert((repo, configured.name.clone()), (candidate, key))
                            .expect("bounded repository gates");
                        p.trees = List::with_capacity(env.limits.trees_in_flight);
                        p.stage = Stage::Collect;
                        collect(d, env, owner, p, out);
                    } else {
                        d.landing.keys.insert((p.task, member, gate), (candidate, key)).expect("bounded landing gates");
                        emit(
                            out,
                            Request::LandingDecision {
                                task: p.task,
                                decision: policy::Decision::Key { member, gate, candidate },
                                entry: None,
                            },
                        );
                    }
                }
                policy::Keyed::TooLarge => emit(out, Request::Refused { task: p.task }),
            }
        }
        Stage::Collect => unreachable!("collect emits the next read itself"),
    }
}
fn collect(d: &mut Domain, env: &Env<Limits>, owner: Token, mut p: Pending, out: &mut Queue<Request>) {
    let row = d.landing.rows.get(&p.task).expect("pending landing");
    for (index, member) in row.landing.members.iter().enumerate() {
        let repo = repository(member);
        for (number, gate) in d.landing.gates(row.landing.project, repo).iter().enumerate() {
            let main = p.facts.get(index).expect("member fact").main;
            if match d.landing.main_keys.get(&(repo, gate.name.clone())) {
                Some((tip, _)) => *tip != main,
                None => true,
            } {
                p.stage = Stage::Tree {
                    member: u16::try_from(index).expect("member position"),
                    gate: u16::try_from(number).expect("gate position"),
                    candidate: main,
                    object: main,
                    page: 1,
                    main: true,
                };
                ask(d, env, owner, p, repo, api::Read::Tree { object: main, page: 1 }, out);
                return;
            }
        }
    }
    for (key, flight) in &d.landing.flights {
        let mut posted = false;
        for member in &row.landing.members {
            if repository(member) != key.repository {
                continue;
            }
            for proof in &member.gates {
                if proof_key(proof.proof) == Some(key.key) && proof.posted != policy::Posted::Nothing {
                    posted = true;
                }
            }
        }
        if !posted || flight.task != p.task || p.polled.as_slice().contains(key) {
            continue;
        }
        if let Some(read) = flight.read() {
            let key = key.clone();
            p.stage = Stage::Run { key: key.clone(), pages: 1 };
            ask(d, env, owner, p, key.repository, read, out);
            return;
        }
    }
    if p.entry == 0 {
        emit(out, Request::WakeProcedure { task: p.task });
    } else if p.woken {
        step(d, env, p.task, p.entry, p.heard, out);
    } else {
        decide(d, env, owner, p, out);
    }
}
fn result(d: &Domain, task: u64, member: u16, gate: u16, operation: u8) -> policy::Status {
    d.landing.results.get(&(task, member, gate, operation)).copied().unwrap_or(policy::Status::Unknown)
}
fn turn(d: &Domain, project: u32) -> (bool, Option<u64>) {
    let mut proved = List::with_capacity(d.landing.limits.landings);
    let mut free = true;
    for (_, row) in &d.landing.rows {
        if row.landing.project != project {
            continue;
        }
        match row.landing.phase {
            policy::Phase::Turn { .. } => free = false,
            policy::Phase::Proved { at } => {
                proved.push(policy::Proved { task: row.landing.task, at }).expect("bounded landings");
            }
            policy::Phase::Waiting
            | policy::Phase::Preparing
            | policy::Phase::Proving
            | policy::Phase::Landed
            | policy::Phase::Held { .. }
            | policy::Phase::Cancelled => {}
        }
    }
    (free, policy::first(proved.as_slice()))
}
#[expect(clippy::too_many_lines, reason = "one policy decision and its atomic durable outputs")]
fn decide(d: &mut Domain, env: &Env<Limits>, owner: Token, mut p: Pending, out: &mut Queue<Request>) {
    let mut row = d.landing.rows.get(&p.task).expect("pending landing").clone();
    if continue_retry(d, &mut row, p.entry, out) {
        return;
    }
    for (index, (member, fact)) in row.landing.members.iter().zip(p.facts.iter_mut()).enumerate() {
        let position = u16::try_from(index).expect("member position");
        fact.merge = result(d, p.task, position, 0, 5);
        for (number, gate) in d.landing.gates(row.landing.project, repository(member)).iter().enumerate() {
            let number = u16::try_from(number).expect("gate position");
            let fact = fact.gates.get_mut(usize::from(number)).expect("gate fact");
            let candidate = member.candidate.unwrap_or([0; 32]);
            fact.key = match d.landing.keys.get(&(p.task, position, number)) {
                Some((tip, key)) if *tip == candidate => Some(*key),
                Some(_) | None => None,
            };
            if let Some(key) = proof_key(member.gates.get(usize::from(number)).expect("gate proof").proof).or(fact.key)
            {
                let key = gate_key(&row, position, number, &gate.name, key);
                fact.receipt = match d.landing.receipts.get(&key) {
                    Some(receipt) if receipt.passed => policy::Status::Passed,
                    Some(_) => policy::Status::Failed,
                    None => policy::Status::Unknown,
                };
                fact.flight = flight_fact(&d.landing, &key, (p.task, position, number));
            }
            fact.posting = result(
                d,
                p.task,
                position,
                number,
                if member.gates.get(usize::from(number)).expect("gate proof").posted == policy::Posted::Verdict {
                    4
                } else {
                    3
                },
            );
        }
    }
    let (free, first) = turn(d, row.landing.project);
    let facts = policy::Facts {
        members: p.facts.clone(),
        turn_free: free,
        first: first == Some(p.task),
        released: p.heard.released,
    };
    let stepped = policy::step(
        &row.landing,
        &facts,
        &policy::Project { gate_retries: 2 },
        &policy::Limits { members: env.limits.landing_members, gates: env.limits.gates, poll: env.limits.client.poll },
        env.wall,
    );
    let prior = row.clone();
    row.landing = stepped.landing;
    let decision = stepped.decision;
    if row.landing.members.len() != prior.landing.members.len() {
        let mut asks = List::with_capacity(env.limits.landing_members);
        for member in &row.landing.members {
            let mut ask = None;
            for (index, old) in prior.landing.members.iter().enumerate() {
                if old.change == member.change {
                    ask = *prior.asks.get(index).expect("member ask");
                }
            }
            asks.push(ask).expect("member asks");
        }
        row.asks = asks.into_boxed();
    }
    let mut committed = None;
    match decision {
        policy::Decision::Prepare { member, head, main } => {
            *row.asks.get_mut(usize::from(member)).expect("member ask") = Some(change::Ask::Prepare { head, main });
            emit(
                out,
                Request::WakeProcedure { task: row.landing.members.get(usize::from(member)).expect("member").change },
            );
        }
        policy::Decision::Repair { member, gate } => {
            *row.asks.get_mut(usize::from(member)).expect("member ask") = Some(change::Ask::Repair {
                gate,
                candidate: row
                    .landing
                    .members
                    .get(usize::from(member))
                    .expect("member")
                    .candidate
                    .expect("proved candidate"),
            });
            emit(
                out,
                Request::WakeProcedure { task: row.landing.members.get(usize::from(member)).expect("member").change },
            );
        }
        policy::Decision::Key { member, gate, candidate } => {
            let repo = repository(row.landing.members.get(usize::from(member)).expect("member"));
            p.stage = Stage::Tree { member, gate, candidate, object: candidate, page: 1, main: false };
            save(d, row, out);
            ask(d, env, owner, p, repo, api::Read::Tree { object: candidate, page: 1 }, out);
            return;
        }
        policy::Decision::Dispatch { member, gate, key, try_ }
        | policy::Decision::Retry { member, gate, key, try_ } => {
            if d.entries.contains_key(&p.entry) || d.entries.len() == env.limits.entries {
                emit(out, Request::Refused { task: p.task });
                return;
            }
            let m = row.landing.members.get(usize::from(member)).expect("member");
            let repo = repository(m);
            let configured =
                d.landing.gates(row.landing.project, repo).get(usize::from(gate)).expect("configured gate").clone();
            let candidate = m.candidate.expect("candidate before dispatch");
            let change = d.changes.get(&m.change).expect("member change");
            let key = gate_key(&row, member, gate, &configured.name, key);
            let purpose = purpose_identity(&row, member, gate, 1, try_);
            if let Some(mut fault) = d.landing.faults.remove(&(p.task, member, gate)) {
                runs::retry(&mut fault, try_);
                fault.purpose.clone_from(&purpose);
                d.landing.flights.insert(key.clone(), fault).expect("bounded key flights");
            } else if let Some(flight) = d.landing.flights.get_mut(&key) {
                runs::retry(flight, try_);
                flight.purpose = purpose;
            } else {
                let inputs = inputs(candidate, key.key);
                let flight = runs::follow(
                    p.task,
                    p.entry,
                    purpose,
                    repo,
                    configured.workflow.clone(),
                    change.branch.clone(),
                    candidate,
                    inputs,
                );
                d.landing.flights.insert(key.clone(), flight).expect("bounded key flights");
            }
            let flight = d.landing.flights.get_mut(&key).expect("new flight");
            let write = flight.write(p.entry).expect("dispatch or retry write");
            flight.outcome = None;
            let operation = if let api::Write::Rerun { .. } = write { Operation::Rerun } else { Operation::Dispatch };
            flight.purpose = purpose_identity(
                &row,
                member,
                gate,
                match operation {
                    Operation::Dispatch => 1,
                    Operation::Rerun => 2,
                    Operation::Pending => 3,
                    Operation::Verdict => 4,
                    Operation::Merge => 5,
                },
                try_,
            );
            keep(d, &mut row, p.entry, member, gate, operation, write, skein_forge_client::Condition::None, out);
            committed = Some(p.entry);
        }
        policy::Decision::Post { member, gate, posting } => {
            if d.entries.contains_key(&p.entry) || d.entries.len() == env.limits.entries {
                emit(out, Request::Refused { task: p.task });
                return;
            }
            let m = row.landing.members.get(usize::from(member)).expect("member");
            let repo = repository(m);
            let configured =
                d.landing.gates(row.landing.project, repo).get(usize::from(gate)).expect("configured gate").clone();
            let key = gate_key(
                &row,
                member,
                gate,
                &configured.name,
                proof_key(m.gates.get(usize::from(gate)).expect("gate proof").proof).expect("keyed posting"),
            );
            let (write, operation) = posting_write(
                m.candidate.expect("candidate"),
                &configured.name,
                &key,
                posting,
                d.landing.receipts.get(&key),
            );
            keep(d, &mut row, p.entry, member, gate, operation, write, skein_forge_client::Condition::None, out);
            committed = Some(p.entry);
        }
        policy::Decision::Merge { member, candidate } => {
            if d.entries.contains_key(&p.entry) || d.entries.len() == env.limits.entries {
                emit(out, Request::Refused { task: p.task });
                return;
            }
            let m = row.landing.members.get(usize::from(member)).expect("member");
            let change = d.changes.get(&m.change).expect("member change");
            let condition = skein_forge_client::Condition::Merge { base: change.base.clone() };
            let write = api::Write::Merge { number: m.pull, head: candidate, how: api::MergeHow::FastForwardOnly };
            keep(d, &mut row, p.entry, member, 0, Operation::Merge, write, condition, out);
            committed = Some(p.entry);
        }
        policy::Decision::Proved => {
            save(d, row.clone(), out);
            let (free, first) = turn(d, row.landing.project);
            if free && let Some(first) = first {
                let selected = d.landing.rows.get_mut(&first).expect("proved landing");
                if let policy::Phase::Proved { at } = selected.landing.phase {
                    selected.landing.phase = policy::Phase::Turn { at };
                }
                let selected = selected.clone();
                save(d, selected.clone(), out);
                if first == p.task {
                    row = selected;
                } else {
                    emit(out, Request::WakeProcedure { task: first });
                }
            }
        }
        policy::Decision::Finish => {
            for member in &row.landing.members {
                emit(out, Request::WakeProcedure { task: member.change });
            }
        }
        policy::Decision::None
        | policy::Decision::Wait { .. }
        | policy::Decision::Stale
        | policy::Decision::Cancel
        | policy::Decision::Hold(_) => {}
    }
    if row.landing.preparation != prior.landing.preparation || decision == policy::Decision::Stale {
        reset_results(d, &row, &prior);
    }
    let project = row.landing.project;
    save(d, row, out);
    if decision != policy::Decision::Proved {
        assign_turn(d, project, out);
    }
    emit(out, Request::LandingDecision { task: p.task, decision, entry: committed });
}
fn purpose_identity(row: &LandingRow, member: u16, gate: u16, operation: u8, attempt: u16) -> Box<[u8]> {
    crate::purpose::EffectPurpose::Landing {
        task: row.landing.task,
        member,
        gate,
        operation,
        preparation: row.landing.preparation,
        attempt,
    }
    .identity()
    .expect("fixed purpose")
}
fn inputs(candidate: api::Commit, key: policy::Key) -> Box<[api::Input]> {
    let mut length = 32;
    if candidate[20..] == [0; 12] {
        length = 20;
    }
    Box::new([
        api::Input {
            name: Box::from(b"temper_commit".as_slice()),
            value: hex(candidate.get(..length).expect("commit width")),
        },
        api::Input { name: Box::from(b"temper_key".as_slice()), value: hex(&key) },
    ])
}
fn concat(parts: &[&[u8]]) -> Box<[u8]> {
    let mut length = 0_usize;
    for part in parts {
        length = length.checked_add(part.len()).expect("bounded words");
    }
    let mut bytes = List::with_capacity(u32::try_from(length).expect("bounded words"));
    for part in parts {
        for byte in *part {
            bytes.push(*byte).expect("sized words");
        }
    }
    bytes.into_boxed()
}
fn posting_write(
    candidate: api::Commit,
    name: &[u8],
    key: &ReceiptKey,
    posting: policy::Posting,
    receipt: Option<&Receipt>,
) -> (api::Write, Operation) {
    let short = hex(&key.key[..8]);
    let (check, ending, operation) = match posting {
        policy::Posting::Pending => (api::Check::Pending, b"".as_slice(), Operation::Pending),
        policy::Posting::Ran { passed: true } => (api::Check::Passed, b" passed".as_slice(), Operation::Verdict),
        policy::Posting::Ran { passed: false } => (api::Check::Failed, b" failed".as_slice(), Operation::Verdict),
        policy::Posting::Received => (api::Check::Passed, b" passed by receipt".as_slice(), Operation::Verdict),
    };
    let link: Box<[u8]> = if posting == policy::Posting::Pending {
        Box::new([])
    } else {
        match receipt {
            Some(receipt) => receipt.run.url.clone(),
            None => Box::new([]),
        }
    };
    (
        api::Write::Status {
            commit: candidate,
            context: concat(&[b"temper/gate/", name]),
            check,
            description: concat(&[b"key ", &short, ending]),
            link,
        },
        operation,
    )
}
fn operation(op: Operation) -> u8 {
    match op {
        Operation::Dispatch => 1,
        Operation::Rerun => 2,
        Operation::Pending => 3,
        Operation::Verdict => 4,
        Operation::Merge => 5,
    }
}
#[expect(clippy::too_many_arguments, reason = "one effect and its member/gate identity")]
fn keep(
    d: &mut Domain,
    row: &mut LandingRow,
    entry: u64,
    member: u16,
    gate: u16,
    op: Operation,
    write: api::Write,
    condition: skein_forge_client::Condition,
    out: &mut Queue<Request>,
) {
    let mut entries = List::with_capacity(d.landing.limits.entries);
    for old in &row.entries {
        entries.push(*old).expect("bounded entries");
    }
    entries.push(LandingEntry { entry, member, gate, operation: op }).expect("entry room checked");
    row.entries = entries.into_boxed();
    d.landing
        .results
        .insert((row.landing.task, member, gate, operation(op)), policy::Status::Pending)
        .expect("bounded effect positions");
    keep_entry(
        d,
        skein_forge_client::Entry {
            number: entry,
            task: row.landing.task,
            repository: repository(row.landing.members.get(usize::from(member)).expect("member")),
            effect: skein_forge_client::Effect { write, condition },
            start: None,
            attempt: None,
            failures: 0,
            queued: false,
        },
        None,
        out,
    );
}

fn put_receipt(d: &mut Domain, env: &Env<Limits>, key: ReceiptKey, receipt: Receipt, out: &mut Queue<Request>) {
    let mut count = 0_u32;
    let mut oldest: Option<(Wall, ReceiptKey)> = None;
    let protected = match &key.check {
        Check::Gate(name) => d.landing.main_keys.get(&(key.repository, name.clone())).copied(),
        Check::Tier(_) => None,
    };
    for (saved, value) in &d.landing.receipts {
        if saved.project != key.project || saved.repository != key.repository || saved.check != key.check {
            continue;
        }
        count = count.checked_add(1).expect("receipt bound");
        if Some(saved.key)
            == match protected {
                Some((_, key)) => Some(key),
                None => None,
            }
            || saved == &key
        {
            continue;
        }
        if match &oldest {
            Some((ended, _)) => value.ended < *ended,
            None => true,
        } {
            oldest = Some((value.ended, saved.clone()));
        }
    }
    if !d.landing.receipts.contains_key(&key) && count >= env.limits.receipts_per_gate {
        if let Some((_, oldest)) = oldest {
            d.landing.receipts.remove(&oldest);
            emit(out, Request::Erase { key: Key::Receipt(oldest) });
        } else {
            return;
        }
    }
    d.landing.receipts.insert(key.clone(), receipt.clone()).expect("bounded receipts per gate");
    emit(out, Request::Save { record: Stored::Receipt { key, receipt }, read_afresh: None, release: None });
}
fn finish_flight(d: &mut Domain, key: &ReceiptKey, out: &mut Queue<Request>) {
    let Some(flight) = d.landing.flights.remove(key) else {
        return;
    };
    let Some(mut row) = d.landing.rows.get(&flight.task).cloned() else {
        return;
    };
    let mut entries = List::with_capacity(d.landing.limits.entries);
    for entry in &row.entries {
        let member = row.landing.members.get(usize::from(entry.member)).expect("member");
        let matches = if repository(member) == key.repository {
            match key.check {
                Check::Gate(ref name) => {
                    match d.landing.gates(row.landing.project, key.repository).get(usize::from(entry.gate)) {
                        Some(gate) => &gate.name == name,
                        None => false,
                    }
                }
                Check::Tier(_) => false,
            }
        } else {
            false
        };
        if matches && (entry.operation == Operation::Dispatch || entry.operation == Operation::Rerun) {
            if d.entries.remove(&entry.entry).is_some() {
                emit(out, Request::Erase { key: Key::Entry(entry.entry) });
            }
        } else {
            entries.push(*entry).expect("bounded entries");
        }
    }
    row.entries = entries.into_boxed();
    save(d, row, out);
    for (_, waiting) in &d.landing.rows {
        if waiting.landing.project == key.project {
            emit(out, Request::WakeProcedure { task: waiting.landing.task });
        }
    }
}
/// Preserve a settled dispatch entry until its run ends: its start window
/// is required to reconstruct a Dispatched proof after restart (landing.md, 15).
pub(crate) fn outcome(
    d: &mut Domain,
    entry: u64,
    outcome: skein_forge_client::Outcome,
    out: &mut Queue<Request>,
) -> bool {
    let mut found = None;
    for (task, row) in &d.landing.rows {
        for item in &row.entries {
            if item.entry == entry {
                found = Some((*task, *item));
            }
        }
    }
    let Some((task, item)) = found else {
        return false;
    };
    let (status, keep_window) = match outcome {
        skein_forge_client::Outcome::Made { .. } => (policy::Status::Passed, true),
        skein_forge_client::Outcome::Failed(_) => (policy::Status::Failed, true),
        skein_forge_client::Outcome::Uncertain | skein_forge_client::Outcome::Queued => {
            (policy::Status::Pending, false)
        }
        skein_forge_client::Outcome::Raced { .. }
        | skein_forge_client::Outcome::Held
        | skein_forge_client::Outcome::Withdrawn => (policy::Status::Failed, false),
    };
    d.landing
        .results
        .insert((task, item.member, item.gate, operation(item.operation)), status)
        .expect("bounded effect positions");
    let mut owned_key = None;
    for (key, flight) in &d.landing.flights {
        if flight.entry == entry {
            owned_key = Some(key.clone());
        }
    }
    if let Some(key) = owned_key {
        let flight = d.landing.flights.get_mut(&key).expect("owned flight");
        flight.waiting = status == policy::Status::Pending;
        if let Some(entry) = d.entries.get(&entry)
            && let Some(attempt) = entry.entry.attempt
        {
            flight.since = Some(attempt.sent);
        }
        if item.operation == Operation::Dispatch
            && let Some(entry) = d.entries.get(&entry)
        {
            flight.start = match entry.entry.start {
                Some(start) => Some(start.at),
                None => flight.start,
            };
        }
        if status == policy::Status::Failed {
            flight.outcome = Some(policy::Verdict::Fault(policy::Fault::Lost));
        }
    }
    if status != policy::Status::Pending && item.operation != Operation::Dispatch && item.operation != Operation::Rerun
    {
        let mut row = d.landing.rows.get(&task).expect("landing effect").clone();
        let mut entries = List::with_capacity(d.landing.limits.entries);
        for old in &row.entries {
            if old.entry != entry {
                entries.push(*old).expect("bounded entries");
            }
        }
        row.entries = entries.into_boxed();
        save(d, row, out);
    }
    emit(out, Request::WakeProcedure { task });
    keep_window && (item.operation == Operation::Dispatch || item.operation == Operation::Rerun)
}
pub(crate) fn hinted(d: &Domain, hint: &api::Hint, out: &mut Queue<Request>) {
    if let api::Change::Commit(commit) = hint.change {
        for (_, flight) in &d.landing.flights {
            if flight.repository == hint.repository && flight.commit == commit {
                emit(out, Request::WakeProcedure { task: flight.task });
            }
        }
    }
}
pub(crate) fn ask_for(d: &Domain, change: &crate::ChangeRow) -> Option<change::Ask> {
    let row = d.landing.rows.get(&change.change.landing?)?;
    for (index, member) in row.landing.members.iter().enumerate() {
        if member.change == change.task {
            if let Some(ask) = *row.asks.get(index).expect("member ask") {
                return Some(ask);
            }
            for (index, gate) in member.gates.iter().enumerate() {
                if let policy::Proof::Failed { .. } = gate.proof
                    && change.change.last_head == member.candidate
                {
                    return Some(change::Ask::Repair {
                        gate: u16::try_from(index).expect("gate position"),
                        candidate: member.candidate?,
                    });
                }
            }
        }
    }
    None
}
#[expect(clippy::match_like_matches_macro, reason = "the foundation forbids matches macros in step code")]
pub(crate) fn change_advanced(
    d: &mut Domain,
    row: &crate::ChangeRow,
    prior: &change::Change,
    decision: change::Decision,
    out: &mut Queue<Request>,
) {
    let Some(task) = row.change.landing else {
        return;
    };
    let Some(mut landing) = d.landing.rows.get(&task).cloned() else {
        return;
    };
    let settled = if let change::State::Preparing { .. } = prior.state {
        if let change::State::Preparing { .. } = row.change.state { false } else { true }
    } else {
        false
    };
    let repair = if let change::Decision::Delegate(change::Delegate::Repair(_)) = decision { true } else { false };
    for (index, member) in landing.landing.members.iter().enumerate() {
        if member.change == row.task && (settled || repair) {
            *landing.asks.get_mut(index).expect("member ask") = None;
        }
    }
    if d.landing.rows.get(&task) != Some(&landing) {
        save(d, landing, out);
    }
    if prior != &row.change {
        emit(out, Request::WakeProcedure { task });
    }
}
#[expect(clippy::manual_map, reason = "the foundation uses explicit option matches without closures")]
pub(crate) fn rebuild(d: &mut Domain) {
    let mut flights = List::with_capacity(slots(&d.landing.limits).expect("flight bound"));
    for (_, row) in &d.landing.rows {
        for (index, member) in row.landing.members.iter().enumerate() {
            let repo = repository(member);
            let Some(change) = d.changes.get(&member.change) else {
                continue;
            };
            for (number, proof) in member.gates.iter().enumerate() {
                let policy::Proof::Dispatched { key, try_ } = proof.proof else {
                    continue;
                };
                let Some(gate) = d.landing.gates(row.landing.project, repo).get(number) else {
                    continue;
                };
                let Some(candidate) = member.candidate else {
                    continue;
                };
                let mut newest = None;
                for entry in &row.entries {
                    if usize::from(entry.member) == index
                        && usize::from(entry.gate) == number
                        && entry.operation == Operation::Dispatch
                    {
                        newest = Some(entry.entry);
                    }
                }
                let Some(entry) = newest else {
                    continue;
                };
                let mut follow = runs::follow(
                    row.landing.task,
                    entry,
                    purpose_identity(
                        row,
                        u16::try_from(index).expect("member"),
                        u16::try_from(number).expect("gate"),
                        1,
                        try_,
                    ),
                    repo,
                    gate.workflow.clone(),
                    change.branch.clone(),
                    candidate,
                    inputs(candidate, key),
                );
                follow.writes = Box::new([]);
                follow.waiting = d.flights.contains_key(&(row.landing.task, entry));
                if let Some(entry) = d.entries.get(&entry)
                    && let Some(attempt) = entry.entry.attempt
                {
                    follow.since = Some(attempt.sent);
                }
                if let Some(entry) = d.entries.get(&entry) {
                    follow.start = match entry.entry.start {
                        Some(start) => Some(start.at),
                        None => None,
                    };
                }
                flights
                    .push((
                        gate_key(
                            row,
                            u16::try_from(index).expect("member"),
                            u16::try_from(number).expect("gate"),
                            &gate.name,
                            key,
                        ),
                        follow,
                    ))
                    .expect("bounded flights");
            }
        }
    }
    for (key, flight) in flights.into_boxed() {
        d.landing.flights.insert(key, flight).expect("bounded flights");
    }
}
fn hex(bytes: &[u8]) -> Box<[u8]> {
    let mut out =
        List::with_capacity(u32::try_from(bytes.len().checked_mul(2).expect("hex width")).expect("bounded hex"));
    for byte in bytes {
        for digit in crate::hex(*byte) {
            out.push(digit).expect("sized hex");
        }
    }
    out.into_boxed()
}
pub(crate) fn row_bytes(row: &LandingRow) -> Option<u64> {
    let mut bytes = u64::try_from(row.landing.members.len())
        .ok()?
        .checked_mul(u64::try_from(size_of::<policy::Member>()).ok()?)?
        .checked_add(
            u64::try_from(row.asks.len()).ok()?.checked_mul(u64::try_from(size_of::<Option<change::Ask>>()).ok()?)?,
        )?
        .checked_add(
            u64::try_from(row.entries.len()).ok()?.checked_mul(u64::try_from(size_of::<LandingEntry>()).ok()?)?,
        )?;
    for member in &row.landing.members {
        bytes = bytes.checked_add(
            u64::try_from(member.gates.len()).ok()?.checked_mul(u64::try_from(size_of::<policy::GateProof>()).ok()?)?,
        )?;
    }
    if let policy::Phase::Held { .. } = row.landing.phase {
        bytes = bytes.checked_add(u64::try_from(size_of::<policy::Phase>()).ok()?)?;
    }
    Some(bytes)
}
pub(crate) fn receipt_bytes(key: &ReceiptKey, receipt: &Receipt) -> Option<u64> {
    let name = match &key.check {
        Check::Gate(name) | Check::Tier(name) => name,
    };
    u64::try_from(name.len()).ok()?.checked_add(u64::try_from(receipt.run.url.len()).ok()?)
}
/// The bounded tables, nested payloads, transient read snapshots and key walker.
pub(crate) fn worst_case(l: &Limits) -> Option<u64> {
    let slots = slots(l)?;
    if l.landing_members > u32::from(u16::MAX)
        || l.gates > u32::from(u16::MAX)
        || l.receipts_per_gate < 2
        || l.trees_in_flight == 0
        || l.key_entries == 0
        || l.key_path_bytes == 0
    {
        return None;
    }
    let gate = u64::try_from(size_of::<policy::Gate>())
        .ok()?
        .checked_add(32 + 255)?
        .checked_add(u64::from(l.ignored_paths).checked_mul(16 + u64::from(l.name_bytes))?)?;
    let rows =
        u64::from(l.landings).checked_mul(
            u64::from(l.landing_members)
                .checked_mul(u64::try_from(size_of::<(policy::Member, Option<change::Ask>)>()).ok()?.checked_add(
                    u64::from(l.gates).checked_mul(u64::try_from(size_of::<policy::GateProof>()).ok()?)?,
                )?)?
                .checked_add(u64::from(l.entries).checked_mul(u64::try_from(size_of::<LandingEntry>()).ok()?)?)?
                .checked_add(u64::try_from(size_of::<policy::Phase>()).ok()?)?,
        )?;
    let trees =
        u64::from(l.trees_in_flight).checked_mul(
            u64::try_from(size_of::<policy::Tree>()).ok()?.checked_add(u64::from(l.key_entries).checked_mul(
                u64::try_from(size_of::<policy::Entry>()).ok()?.checked_add(u64::from(l.name_bytes))?,
            )?)?,
        )?;
    let member_facts = u64::from(l.landing_members).checked_mul(
        u64::try_from(size_of::<policy::MemberFacts>())
            .ok()?
            .checked_add(u64::from(l.gates).checked_mul(u64::try_from(size_of::<policy::GateFacts>()).ok()?)?)?,
    )?;
    // The policy's B-tree maps hold at most the entries of the supplied trees,
    // plus their root. Price their nodes through the same lib bound, with a
    // 64-byte value covering either of key.rs's Pending variants.
    let walk_nodes = l.key_entries.checked_mul(l.trees_in_flight)?.checked_add(1)?;
    let walker = Map::<Box<[u8]>, [u8; 64]>::worst_case(walk_nodes)?
        .checked_add(Map::<[u8; 32], ()>::worst_case(l.trees_in_flight)?)?
        .checked_add(
            u64::from(walk_nodes)
                .checked_mul(u64::from(l.trees_in_flight.checked_add(1)?).checked_mul(u64::from(l.name_bytes))?)?,
        )?;
    let pending = trees
        .checked_add(walker)?
        .checked_add(u64::from(l.key_entries).checked_mul(64 + u64::from(l.name_bytes))?)?
        .checked_add(u64::from(l.key_path_bytes).checked_mul(3)?)?
        .checked_add(member_facts.checked_mul(2)?)?
        .checked_add(u64::from(slots).checked_mul(u64::try_from(size_of::<ReceiptKey>()).ok()?.checked_add(32)?)?)?;
    Map::<u64, LandingRow>::worst_case(l.landings)?
        .checked_add(rows.checked_mul(4)?)?
        .checked_add(Map::<ReceiptKey, Receipt>::worst_case(receipt_slots(l)?)?)?
        .checked_add(u64::from(receipt_slots(l)?).checked_mul(32 + 2048)?)?
        .checked_add(Map::<u32, (change::Way, Box<[RepositoryGates]>)>::worst_case(l.judge_projects)?)?
        .checked_add(u64::from(l.judge_projects).checked_mul(u64::from(l.repositories).checked_mul(
            u64::try_from(size_of::<RepositoryGates>()).ok()?.checked_add(u64::from(l.gates).checked_mul(gate)?)?,
        )?)?)?
        .checked_add(Map::<Token, Pending>::worst_case(l.landings)?)?
        .checked_add(u64::from(l.landings).checked_mul(pending)?)?
        .checked_add(Map::<Position, (api::Commit, policy::Key)>::worst_case(slots)?)?
        .checked_add(Map::<RepositoryGate, KeyAt>::worst_case(l.repositories.checked_mul(l.gates)?)?)?
        .checked_add(u64::from(l.repositories.checked_mul(l.gates)?).checked_mul(32)?)?
        .checked_add(Map::<Position, runs::Follow>::worst_case(slots)?)?
        .checked_add(Map::<ReceiptKey, runs::Follow>::worst_case(slots)?)?
        .checked_add(
            u64::from(slots).checked_mul(
                u64::from(l.client.op_bytes)
                    .checked_mul(6)?
                    .checked_add(u64::from(l.client.answer_bytes).checked_mul(2)?)?
                    .checked_add(32)?,
            )?,
        )?
        .checked_add(Map::<(u64, u16, u16, u8), policy::Status>::worst_case(result_slots(l)?)?)
}

fn assign_turn(d: &mut Domain, project: u32, out: &mut Queue<Request>) {
    let (free, first) = turn(d, project);
    if free && let Some(task) = first {
        let row = d.landing.rows.get_mut(&task).expect("proved landing");
        if let policy::Phase::Proved { at } = row.landing.phase {
            row.landing.phase = policy::Phase::Turn { at };
        }
        let row = row.clone();
        save(d, row, out);
        emit(out, Request::WakeProcedure { task });
    }
}
fn reset_results(d: &mut Domain, row: &LandingRow, prior: &LandingRow) {
    for (index, member) in row.landing.members.iter().enumerate() {
        if member.candidate == prior.landing.members.get(index).expect("member").candidate {
            continue;
        }
        let member = u16::try_from(index).expect("member");
        d.landing.results.remove(&(row.landing.task, member, 0, 5));
        for gate in 0..d.landing.limits.gates {
            d.landing.faults.remove(&(row.landing.task, member, u16::try_from(gate).expect("gate")));
            for operation in 1..=5 {
                d.landing.results.remove(&(row.landing.task, member, u16::try_from(gate).expect("gate"), operation));
            }
        }
    }
}
fn continue_retry(d: &mut Domain, row: &mut LandingRow, entry: u64, out: &mut Queue<Request>) -> bool {
    let mut pending = None;
    for (key, flight) in &d.landing.flights {
        if flight.task == row.landing.task && !flight.writes.is_empty() {
            pending = Some(key.clone());
            break;
        }
    }
    let Some(key) = pending else {
        return false;
    };
    let mut position = None;
    for (member, m) in row.landing.members.iter().enumerate() {
        if repository(m) != key.repository {
            continue;
        }
        for (gate, proof) in m.gates.iter().enumerate() {
            if let policy::Proof::Dispatched { key: proof, try_ } = proof.proof
                && proof == key.key
            {
                position = Some((u16::try_from(member).expect("member"), u16::try_from(gate).expect("gate"), try_));
            }
        }
    }
    let Some((member, gate, try_)) = position else {
        return false;
    };
    if result(d, row.landing.task, member, gate, 2) == policy::Status::Pending
        || d.entries.contains_key(&entry)
        || d.entries.len() == d.landing.limits.entries
    {
        emit(
            out,
            Request::LandingDecision {
                task: row.landing.task,
                decision: policy::Decision::Wait { until: None },
                entry: None,
            },
        );
        return true;
    }
    let flight = d.landing.flights.get_mut(&key).expect("pending reruns");
    if flight.waiting
        || match &flight.run {
            Some(run) => run.state != api::RunState::Completed,
            None => true,
        }
    {
        emit(
            out,
            Request::LandingDecision {
                task: row.landing.task,
                decision: policy::Decision::Wait { until: None },
                entry: None,
            },
        );
        return true;
    }
    let write = flight.write(entry).expect("next failed job");
    keep(d, row, entry, member, gate, Operation::Rerun, write, skein_forge_client::Condition::None, out);
    save(d, row.clone(), out);
    emit(
        out,
        Request::LandingDecision {
            task: row.landing.task,
            decision: policy::Decision::Retry { member, gate, key: key.key, try_ },
            entry: Some(entry),
        },
    );
    true
}

pub(crate) fn refresh(d: &mut Domain, env: &Env<Limits>, out: &mut Queue<Request>) {
    let mut tasks = List::with_capacity(env.limits.landings);
    for (task, row) in &d.landing.rows {
        if row.landing.phase != policy::Phase::Landed && row.landing.phase != policy::Phase::Cancelled {
            tasks.push(*task).expect("bounded landings");
        }
    }
    for task in tasks.into_boxed() {
        step(d, env, task, 0, LandingHeard { released: false, cancelled: false }, out);
    }
}

pub(crate) fn judge(
    d: &Domain,
    row: &LandingRow,
    criterion: crate::Criterion,
    head: api::Commit,
    evidence: &crate::ChangeEvidence,
    reviewers: &[crate::Reviewer],
) -> Option<crate::JudgeVerdict> {
    for (position, member) in row.landing.members.iter().enumerate() {
        if member.candidate != Some(head) {
            continue;
        }
        let repo = repository(member);
        if evidence.head != Some(head) {
            return Some(crate::JudgeVerdict::Wait);
        }
        if criterion == crate::Criterion::Ci {
            for (number, configured) in d.landing.gates(row.landing.project, repo).iter().enumerate() {
                if !configured.blocking {
                    continue;
                }
                let Some(key) = proof_key(member.gates.get(number).expect("gate proof").proof) else {
                    return Some(crate::JudgeVerdict::Wait);
                };
                let key =
                    gate_key(row, u16::try_from(position).ok()?, u16::try_from(number).ok()?, &configured.name, key);
                match d.landing.receipts.get(&key) {
                    Some(receipt) if receipt.passed => {}
                    Some(_) => return Some(crate::JudgeVerdict::Refuse),
                    None => return Some(crate::JudgeVerdict::Wait),
                }
            }
            return Some(crate::JudgeVerdict::Met);
        }
        let change = d.changes.get(&member.change)?;
        return Some(crate::judge::judge(
            criterion,
            d.repositories.get(&repo)?,
            head,
            &change.change.clean,
            evidence,
            reviewers,
        ));
    }
    None
}

/// An active key's owner takes precedence over a previous owner's fault.
pub(crate) fn flight_fact(state: &State, key: &ReceiptKey, position: Position) -> policy::Flight {
    match state.flights.get(key) {
        Some(flight) if flight.task == position.0 => policy::Flight::Mine { run: flight.outcome },
        Some(_) => policy::Flight::Other,
        None => match state.faults.get(&position) {
            Some(fault) => policy::Flight::Mine { run: fault.outcome },
            None => policy::Flight::None,
        },
    }
}

/// Terminal rows with no effects left surrender their live slot on admission
/// (domain/landing.md, 12); receipts have their independent retention.
fn retire_finished(d: &mut Domain, out: &mut Queue<Request>) -> bool {
    let mut finished = None;
    for (task, row) in &d.landing.rows {
        if !row.entries.is_empty() {
            continue;
        }
        match row.landing.phase {
            policy::Phase::Landed | policy::Phase::Cancelled => {
                finished = Some(*task);
                break;
            }
            policy::Phase::Waiting
            | policy::Phase::Preparing
            | policy::Phase::Proving
            | policy::Phase::Proved { .. }
            | policy::Phase::Turn { .. }
            | policy::Phase::Held { .. } => {}
        }
    }
    let Some(task) = finished else {
        return false;
    };
    d.landing.rows.remove(&task);
    for member in 0..d.landing.limits.landing_members {
        d.landing.results.remove(&(task, u16::try_from(member).expect("member position"), 0, 5));
        for gate in 0..d.landing.limits.gates {
            let member = u16::try_from(member).expect("admitted member position");
            let gate = u16::try_from(gate).expect("admitted gate position");
            d.landing.keys.remove(&(task, member, gate));
            d.landing.faults.remove(&(task, member, gate));
            for operation in 1..=5 {
                d.landing.results.remove(&(task, member, gate, operation));
            }
        }
    }
    emit(out, Request::Erase { key: Key::Landing(task) });
    true
}
