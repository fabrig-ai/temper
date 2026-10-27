//! Independent fake Forgejo and the native calls it received.
use crate::observations::{self, Store};
use jig_conformance::{Clock, referee as r};
use jig_test_system::{Copy, Observed, SystemKey, ValueObserved};
use skein_fake_forge_domain::{self as fake, api as raw};
use skein_forge::api;
use skein_lib::{Duration, Env, Queue, ReplyTo, Time, Token, Wall};
use std::cell::RefCell;
use std::collections::BTreeMap;
use temper_engine_domain as engine;
use temper_engine_domain::{self as root};
use temper_engine_domain_forge as forge;
use temper_engine_forge_world::translate;

struct Pending {
    root_call: Token,
    generation: u64,
    op: api::Op,
    effect: Option<(SystemKey, raw::Write, Copy, ValueObserved)>,
}
/// Forge state and transport faults persist when the engine crashes.
#[expect(clippy::struct_excessive_bools, reason = "independent transport faults and CI modes are orthogonal")]
pub struct FakeForge {
    pub domain: fake::Domain,
    config: fake::Config,
    client_limits: skein_forge_client::Limits,
    out: std::collections::VecDeque<fake::Request>,
    pending: BTreeMap<Token, Pending>,
    late: Vec<(Token, api::Repository, api::Op, SystemKey)>,
    pub hold: bool,
    pub merge_only: bool,
    pub failed: bool,
    pub late_next: bool,
    pub lose_merge_answer: bool,
    pub lose_issue_answer: bool,
    pub effects: Vec<Observed>,
    pub uncertain: Vec<SystemKey>,
    pub native_observations: Vec<fake::Observation>,
    owners: BTreeMap<Vec<u8>, u64>,
    new_observations: RefCell<Vec<r::Observed>>,
    seen: BTreeMap<SystemKey, u32>,
    serial: u64,
    pub generation: u64,
    pub head: api::Commit,
}
impl FakeForge {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self::new_with_ci(seed, 1000)
    }
    #[must_use]
    pub fn new_with_ci(seed: u64, passes: u32) -> Self {
        let mut config = temper_engine_forge_world::fake_config();
        config.latency_min = Duration::ZERO;
        config.latency_max = Duration::ZERO;
        let mut domain = fake::Domain::new(&config, seed);
        fake::repository(
            &mut domain,
            &config,
            raw::Setup {
                workflows: Box::new([]),
                id: 2,
                name: b"org/repo".as_slice().into(),
                default: b"main".as_slice().into(),
                tree: Box::new([]),
                labels: Box::new([]),
                checks: raw::Checks {
                    contexts: Box::new([b"build".as_slice().into()]),
                    latency_min: Duration::ZERO,
                    latency_max: Duration::ZERO,
                    silent: 0,
                    passes,
                    reruns: 0,
                    cue: None,
                },
                protection: None,
                hooked: true,
            },
        );
        fake::grant(&mut domain, b"org/repo", 1, raw::Permission::Admin);
        fake::grant(&mut domain, b"org/repo", 2, raw::Permission::Write);
        fake::grant(&mut domain, b"org/repo", 4, raw::Permission::Write);
        let head = translate::commit(domain.branch(b"org/repo", b"main").expect("main head"));
        Self {
            domain,
            config,
            client_limits: super::config::root(seed, None, None).1.forge.client,
            out: std::collections::VecDeque::new(),
            pending: BTreeMap::new(),
            late: Vec::new(),
            hold: false,
            merge_only: false,
            failed: false,
            late_next: false,
            lose_merge_answer: false,
            lose_issue_answer: false,
            effects: Vec::new(),
            uncertain: Vec::new(),
            native_observations: Vec::new(),
            owners: BTreeMap::new(),
            new_observations: RefCell::new(Vec::new()),
            seen: BTreeMap::new(),
            serial: 1,
            generation: 0,
            head,
        }
    }
    #[expect(
        clippy::wildcard_enum_match_arm,
        clippy::match_wildcard_for_single_variants,
        reason = "only forge entry rows associate the released write with its durable key"
    )]
    pub fn call(
        &mut self,
        store: &Store,
        clock: Clock,
        call: Token,
        repository: api::Repository,
        op: api::Op,
    ) -> Vec<engine::Event> {
        let key = if let api::Op::Write(write) = &op {
            let entry = store
                .rows
                .values()
                .find_map(|row| match row {
                    root::Record::Forge { row } => match row.as_ref() {
                        forge::Stored::Entry { entry, held: false } if &entry.effect.write == write => Some(entry),
                        _ => None,
                    },
                    _ => None,
                })
                .expect("released write has a durable forge entry");
            let key = observations::key(store, entry);
            let attempt = entry.attempt.expect("released write has a durable attempt");
            self.new_observations.borrow_mut().push(r::Observed::Released {
                requires: vec![r::Receipt::Outbox(
                    0,
                    key,
                    u32::try_from(attempt.sent.as_nanos() / 1_000_000).expect("bounded attempt timestamp"),
                )],
            });
            Some(key)
        } else {
            None
        };
        if let Some(key) = key
            && (self.hold || self.late_next)
            && (!self.merge_only || matches!(op, api::Op::Write(api::Write::Merge { .. })))
        {
            self.late_next = false;
            self.late.push((call, repository, op, key));
            self.uncertainty(key);
            return vec![engine::Event::Forge(Box::new(temper_engine_domain_forge::Event::Client(
                skein_forge_client::Event::Answered {
                    free: 0,
                    rate: None,
                    call,
                    cost: 1,
                    result: Err(api::Error::Timeout),
                },
            )))];
        }
        self.send(clock, call, op, key, false);
        Vec::new()
    }
    fn uncertainty(&mut self, key: SystemKey) {
        self.uncertain.push(key);
        self.new_observations.borrow_mut().push(r::Observed::Uncertain { connector: 0, key });
    }
    fn send(&mut self, clock: Clock, call: Token, op: api::Op, key: Option<SystemKey>, late: bool) {
        let native = translate::op(&op, &self.client_limits);
        let name = translate::locate_name(&op).unwrap_or_else(|| b"org/repo".as_slice().into());
        let effect = match (&native, key) {
            (raw::Op::Write(write), Some(key)) => {
                let count = self.seen.entry(key).or_default();
                *count += 1;
                let copy = if late {
                    Copy::Late
                } else if *count > 1 {
                    Copy::Retried
                } else {
                    Copy::First
                };
                Some((key, write.clone(), copy, self.before(write, key)))
            }
            _ => None,
        };
        let fake_call = Token::new(self.serial);
        self.serial += 1;
        self.pending.insert(
            fake_call,
            Pending { root_call: call, generation: if late { u64::MAX } else { self.generation }, op, effect },
        );
        let env = Env { now: Time::from_nanos(clock.now), wall: Wall::from_nanos(clock.now), limits: self.config };
        let mut queue = Queue::with_capacity(fake::MAX_OUT);
        fake::step(
            &mut self.domain,
            &env,
            fake::Event::Call { reply_to: ReplyTo::new(fake_call), user: 1, repository: name, op: native },
            &mut queue,
        );
        while let Some(request) = queue.pop() {
            self.out.push_back(request);
        }
        self.collect_native();
    }
    pub fn delayed_merge(&self) -> bool {
        self.late.iter().any(|(_, _, op, _)| matches!(op, api::Op::Write(api::Write::Merge { .. })))
    }
    pub fn release_late(&mut self, clock: Clock) {
        self.hold = false;
        for (call, _, op, key) in std::mem::take(&mut self.late) {
            self.send(clock, call, op, Some(key), true);
        }
    }
    pub fn poll(&mut self, clock: Clock) -> Vec<engine::Event> {
        let env = Env { now: Time::from_nanos(clock.now), wall: Wall::from_nanos(clock.now), limits: self.config };
        if self.domain.is_due(env.now) {
            let mut queue = Queue::with_capacity(fake::MAX_OUT);
            fake::fire(&mut self.domain, &env, &mut queue);
            while let Some(request) = queue.pop() {
                self.out.push_back(request);
            }
        }
        let mut events = Vec::new();
        while let Some(request) = self.out.pop_front() {
            if let Some(hint) = translate::hint(temper_engine_forge_world::REPO, &request) {
                events.push(engine::Event::Forge(Box::new(temper_engine_domain_forge::Event::Hint { hint })));
            }
            if let fake::Request::Reply { to, result } = request {
                let call = to.into_token();
                let Some(pending) = self.pending.remove(&call) else { continue };
                self.observe_ci(clock, &pending.op, &result);
                let key = pending.effect.as_ref().map(|(key, ..)| *key);
                if let Some((key, write, copy, before)) = pending.effect {
                    let (kind, resources, condition, target) = raw_parts(&write);
                    let observation = Observed {
                        kind,
                        key,
                        resources: resources.into_iter().map(|name| jig_test_system::Name(name.path)).collect(),
                        condition,
                        target,
                        before: vec![before],
                        applied: result.is_ok(),
                        copy,
                    };
                    self.effects.push(observation.clone());
                    self.new_observations.borrow_mut().push(r::Observed::Effect { connector: 0, effect: observation });
                }
                if self.lose_merge_answer
                    && matches!(pending.op, api::Op::Write(api::Write::Merge { .. }))
                    && result.is_ok()
                {
                    self.lose_merge_answer = false;
                    self.uncertainty(key.expect("lost merge flight"));
                    continue;
                }
                if self.lose_issue_answer
                    && matches!(pending.op, api::Op::Write(api::Write::CreateIssue { .. }))
                    && result.is_ok()
                {
                    self.lose_issue_answer = false;
                    self.uncertainty(key.expect("lost issue flight"));
                    continue;
                }
                if pending.generation == self.generation {
                    events.push(engine::Event::Forge(Box::new(temper_engine_domain_forge::Event::Client(
                        skein_forge_client::Event::Answered {
                            free: 0,
                            rate: None,
                            call: pending.root_call,
                            cost: 1,
                            result: match result {
                                Ok(answer) => Ok(translate::answer(&pending.op, answer, &self.client_limits)),
                                Err(error) => Err(translate::error(error, fake::time(&self.config, env.now))),
                            },
                        },
                    ))));
                }
            }
        }
        // Native changes are retained independently too, including refusals.
        self.collect_native();
        assert_eq!(self.domain.observations_lost(), 0, "forge observation capacity");
        self.domain.reclaim();
        events
    }
    // The referee's CI evidence comes from the native answer, including the
    // pull read that supplies CI when no contexts require a separate head read.
    fn observe_ci(&self, clock: Clock, op: &api::Op, result: &Result<raw::Answer, raw::Error>) {
        let (commit, passed) =
            if let (api::Op::Read(api::Read::Statuses { commit, .. }), Ok(raw::Answer::Statuses(statuses))) =
                (op, result)
            {
                (*commit, !statuses.is_empty() && statuses.iter().all(|status| status.state == raw::Check::Passed))
            } else if let (api::Op::Read(api::Read::Checks { commit }), Ok(raw::Answer::Checks(checks))) = (op, result)
            {
                (*commit, !checks.is_empty() && checks.iter().all(|check| check.status.state == raw::Check::Passed))
            } else if let (
                api::Op::Read(api::Read::Pull { .. } | api::Read::PullFor { .. }),
                Ok(raw::Answer::Pull(pull)),
            ) = (op, result)
            {
                (
                    translate::commit(pull.commit),
                    !pull.statuses.is_empty() && pull.statuses.iter().all(|status| status.state == raw::Check::Passed),
                )
            } else {
                return;
            };
        if !self
            .domain
            .branches(b"org/repo")
            .iter()
            .any(|(branch, head)| branch.as_ref() != b"main" && translate::commit(*head) == commit)
        {
            return;
        }
        let mut resource = observations::repository();
        resource.path.extend([b"branch".to_vec(), b"main".to_vec()]);
        self.new_observations.borrow_mut().push(r::Observed::Read {
            resource,
            state: passed.then(|| observations::value(&commit)),
            at: clock.now,
        });
    }

    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "all observations are retained while branch ownership uses only movement and deletion"
    )]
    fn collect_native(&mut self) {
        while let Some(observation) = self.domain.pop_observation() {
            match &observation {
                fake::Observation::Moved { branch, by, .. } => {
                    self.owners.insert(branch.to_vec(), *by);
                }
                fake::Observation::Deleted { branch, .. } => {
                    self.owners.remove(branch.as_ref());
                }
                _ => {}
            }
            self.native_observations.push(observation);
        }
    }
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "unsupported writes panic and absent read answers provide no evidence"
    )]
    fn before(&self, write: &raw::Write, key: SystemKey) -> ValueObserved {
        let (state, owned) = match write {
            raw::Write::Merge { number, .. } => (
                match self.domain.inspect(&self.config, b"org/repo", &raw::Read::Pull { number: *number }) {
                    Ok(raw::Answer::Pull(pull)) => Some(observations::value(&translate::commit(pull.commit))),
                    _ => None,
                },
                false,
            ),
            raw::Write::DeleteBranch { branch } => (
                self.domain.branch(b"org/repo", branch).map(|head| observations::value(&translate::commit(head))),
                self.owners.get(branch.as_ref()).is_some_and(|by| matches!(by, 1 | 2)),
            ),
            raw::Write::Status { commit, context, .. } => {
                let found =
                    match self.domain.inspect(&self.config, b"org/repo", &raw::Read::Statuses { commit: *commit }) {
                        Ok(raw::Answer::Statuses(statuses)) => {
                            statuses.into_vec().into_iter().find(|s| s.context == *context)
                        }
                        _ => None,
                    };
                found.map_or((None, false), |status| {
                    (
                        Some(observations::value(format!("{context:?}:{:?}", status.state).as_bytes())),
                        status.author == 1,
                    )
                })
            }
            raw::Write::EditItem { number, .. } | raw::Write::Close { number } => {
                match self.domain.inspect(&self.config, b"org/repo", &raw::Read::Item { number: *number, after: 0 }) {
                    Ok(raw::Answer::Item { item, .. }) => (
                        Some(observations::value(
                            format!("{:?}:{:?}:{:?}", item.title, item.body, item.state).as_bytes(),
                        )),
                        item.author == 1,
                    ),
                    _ => (None, false),
                }
            }
            raw::Write::OpenPull { .. } | raw::Write::CreateIssue { .. } | raw::Write::Comment { .. } => (None, false),
            other => panic!("pre-write evidence outside scenario vocabulary: {other:?}"),
        };
        ValueObserved { state, owner: owned.then_some(key) }
    }
    pub fn external(&mut self, clock: Clock, op: raw::Op) -> raw::Answer {
        self.external_as(clock, 1, op)
    }
    pub fn external_as(&mut self, clock: Clock, user: u64, op: raw::Op) -> raw::Answer {
        let token = Token::new(self.serial);
        self.serial += 1;
        let env = Env { now: Time::from_nanos(clock.now), wall: Wall::from_nanos(clock.now), limits: self.config };
        let mut queue = Queue::with_capacity(fake::MAX_OUT);
        fake::fire(&mut self.domain, &env, &mut queue);
        while let Some(request) = queue.pop() {
            self.out.push_back(request);
        }
        fake::step(
            &mut self.domain,
            &env,
            fake::Event::Call { reply_to: ReplyTo::new(token), user, repository: b"org/repo".as_slice().into(), op },
            &mut queue,
        );
        let mut answer = None;
        for _ in 0..1000 {
            while let Some(request) = queue.pop() {
                match request {
                    fake::Request::Reply { to, result } => {
                        let call = to.into_token();
                        if call == token {
                            answer = Some(result.expect("external forge operation"));
                        } else {
                            self.out.push_back(fake::Request::Reply { to: ReplyTo::new(call), result });
                        }
                    }
                    other @ fake::Request::Hook { .. } => self.out.push_back(other),
                }
            }
            if answer.is_some() {
                break;
            }
            assert!(self.domain.is_due(env.now), "zero-latency external operation remains due");
            fake::fire(&mut self.domain, &env, &mut queue);
        }
        self.collect_native();
        answer.expect("external reply")
    }
    pub fn fail(&mut self, clock: Clock, commit: u64) {
        self.external(
            clock,
            raw::Op::Write(raw::Write::Status {
                commit,
                context: b"build".as_slice().into(),
                state: raw::Check::Failed,
                description: Box::new([]),
                link: Box::new([]),
            }),
        );
    }
    pub fn create(&mut self, clock: Clock, branch: &[u8]) {
        self.external(
            clock,
            raw::Op::Git(raw::Git::Create {
                branch: branch.into(),
                commit: self.domain.branch(b"org/repo", b"main").expect("main"),
            }),
        );
    }
    pub fn advance(&mut self, clock: Clock, branch: &[u8], body: &[u8]) -> api::Commit {
        let env = Env { now: Time::from_nanos(clock.now), wall: Wall::from_nanos(clock.now), limits: self.config };
        let commit = fake::advance(
            &mut self.domain,
            &env,
            b"org/repo",
            branch,
            b"file",
            body,
            if body == b"other hand" { 4 } else { 1 },
        )
        .expect("external push");
        self.collect_native();
        if self.failed {
            self.fail(clock, commit);
        }
        translate::commit(commit)
    }
    /// Inspect the independent native forge.
    ///
    /// # Errors
    /// Returns the native refusal when that object cannot be read.
    pub fn inspect(&self, read: &raw::Read) -> Result<raw::Answer, raw::Error> {
        self.domain.inspect(&self.config, b"org/repo", read)
    }
    pub fn take_observed(&self) -> Vec<r::Observed> {
        std::mem::take(&mut self.new_observations.borrow_mut())
    }
}
#[expect(clippy::wildcard_enum_match_arm, reason = "unsupported native writes fail the outside translation explicitly")]
fn raw_parts(write: &raw::Write) -> (u16, Vec<r::Name>, Option<u64>, u64) {
    let mut name = observations::repository();
    let (kind, condition, target) = match write {
        raw::Write::Status { context, state, .. } => {
            (6, None, observations::value(format!("{context:?}:{state:?}").as_bytes()))
        }
        raw::Write::CreateIssue { title, body, .. } => {
            let unmarked = body.as_ref().splitn(2, |x| *x == b'\n').nth(1).unwrap_or(body);
            (8, None, observations::value(format!("{title:?}:{unmarked:?}").as_bytes()))
        }
        raw::Write::Comment { number, body } => {
            name.path.extend([b"issue".to_vec(), number.to_string().into_bytes()]);
            let unmarked = body.as_ref().splitn(2, |x| *x == b'\n').nth(1).unwrap_or(body);
            (7, None, observations::value(unmarked))
        }
        raw::Write::EditItem { number, title, body } => {
            name.path.extend([b"issue".to_vec(), number.to_string().into_bytes()]);
            (13, None, observations::value(format!("{title:?}:{body:?}").as_bytes()))
        }
        raw::Write::Close { number } => {
            name.path.extend([b"issue".to_vec(), number.to_string().into_bytes()]);
            (13, None, 1)
        }
        raw::Write::Merge { number, head, .. } => {
            name.path.extend([b"branch".to_vec(), b"main".to_vec()]);
            (4, Some(observations::value(&translate::commit(*head))), *number)
        }
        raw::Write::OpenPull { title, body, head, base, opens_empty: _ } => {
            (3, None, observations::value(format!("{title:?}:{body:?}:{head:?}:{base:?}").as_bytes()))
        }
        raw::Write::DeleteBranch { branch } => {
            name.path.push(b"branch".to_vec());
            name.path.extend(branch.split(|byte| *byte == b'/').map(<[u8]>::to_vec));
            (15, None, 0)
        }
        other => panic!("unbound native forge write {other:?}"),
    };
    (kind, vec![name], condition, target)
}
