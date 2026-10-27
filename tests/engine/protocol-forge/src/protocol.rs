//! The forge connector and real component against the fake forge through bytes.
use crate::fixtures;
use skein_fake_forge_domain::{self as forge, api as raw};
use skein_fake_forge_protocol::{Faults, Route, Tally};
use skein_forge_connection::Component;
use skein_forge_machine_world::joint::{self, Joint, REPO};
use skein_lib::{Duration, Env, List, Map, Queue, ReplyTo, Time, Token, Wall};
use std::collections::{BTreeMap, BTreeSet};
use temper_engine_domain_forge as top;
use temper_engine_forge_world::LIMITS;

/// A candidate base read (main tip or any Checks): send time, request, returned branch commit.
pub type BaseRead = (Wall, skein_forge::api::Read, Option<skein_forge::api::Commit>);

/// A top, transport, fake service and independently scripted people.
pub struct Protocol {
    top: top::Domain,
    env: Env<top::Limits>,
    joint: Joint,
    top_out: Queue<top::Request>,
    stored: Map<top::Key, top::Stored>,
    seen: List<top::Request>,
    pending: BTreeMap<u64, (u64, Token, bool, Option<usize>)>,
    base_reads: Vec<BaseRead>,
    answered: BTreeSet<u64>,
    serial: u64,
    generation: u64,
    seed: u64,
    fixture: fixtures::Fixture,
    writes: u32,
    calls: u32,
    costs: u64,
    secrets_kept: bool,
    item_pages: Vec<u64>,
    item_start: u64,
    request_times: Vec<Wall>,
}
impl Protocol {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self::cutting(seed, 0, Faults { reset: 0, drop: 0, server_error: 0 })
    }
    #[must_use]
    pub fn cutting(seed: u64, chance: u32, faults: Faults) -> Self {
        let mut config = fixtures::protocol_config();
        config.faults = faults;
        let joint = Joint::with(
            seed,
            chance,
            config,
            fixtures::limits(),
            fixtures::writer_settings(),
            fixtures::protocol_shared(),
            joint::checks(1000),
        );
        Self::with(seed, joint, fixtures::FORGEJO)
    }
    #[must_use]
    pub fn github(seed: u64) -> Self {
        Self::github_with_checks(seed, joint::checks(1000))
    }
    #[must_use]
    pub fn github_with_checks(seed: u64, checks: raw::Checks) -> Self {
        Self::github_with_limits(seed, checks, &LIMITS)
    }
    /// The same GitHub world with a story's explicit domain limits.
    #[must_use]
    pub fn github_with_limits(seed: u64, checks: raw::Checks, limits: &top::Limits) -> Self {
        let joint = Joint::github(
            seed,
            0,
            fixtures::limits(),
            fixtures::github_settings(),
            fixtures::protocol_shared(),
            checks,
        );
        Self::with_limits(seed, joint, fixtures::GITHUB, limits)
    }
    /// The Forgejo world with a story's explicit domain limits; the component's
    /// shared bounds follow their client limits.
    #[must_use]
    pub fn forgejo_with_limits(seed: u64, limits: &top::Limits) -> Self {
        let joint = Joint::with(
            seed,
            0,
            fixtures::protocol_config(),
            fixtures::limits(),
            fixtures::writer_settings(),
            fixtures::protocol_shared_for(&limits.client),
            joint::checks(1000),
        );
        Self::with_limits(seed, joint, fixtures::FORGEJO, limits)
    }
    fn with(seed: u64, joint: Joint, fixture: fixtures::Fixture) -> Self {
        Self::with_limits(seed, joint, fixture, &LIMITS)
    }
    fn with_limits(seed: u64, joint: Joint, fixture: fixtures::Fixture, limits: &top::Limits) -> Self {
        let mut world = Self {
            top: Self::top(seed, fixture.writer, limits),
            env: Env { now: joint.env.now, wall: joint.env.wall, limits: *limits },
            joint,
            fixture,
            top_out: Queue::with_capacity(top::max_out(limits)),
            stored: Map::with_capacity(128),
            seen: List::with_capacity(256),
            pending: BTreeMap::new(),
            base_reads: Vec::new(),
            answered: BTreeSet::new(),
            serial: 0,
            generation: 1,
            seed,
            writes: 0,
            calls: 0,
            costs: 0,
            secrets_kept: true,
            item_pages: Vec::new(),
            item_start: 0,
            request_times: Vec::new(),
        };
        world.accept_installations();
        world.event(top::Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic });
        world
    }
    fn accept_installations(&mut self) {
        if self.fixture.kind == skein_forge_connection::settings::Kind::GitHub {
            self.joint
                .action(skein_forge_connection::Request::Installations { forge: REPO.forge, accepted: Box::from([1]) });
        }
    }
    fn top(seed: u64, author: u64, limits: &top::Limits) -> top::Domain {
        top::Domain::new(
            limits,
            seed,
            skein_forge_client::Config {
                namespace: Box::from(b"world".as_slice()),
                writers: Box::new([skein_forge_client::Writer { forge: REPO.forge, author }]),
            },
            &[top::GitIdentity { forge: REPO.forge, credential: 1 }],
            &[],
            false,
        )
        .expect("configured connector")
    }
    pub fn event(&mut self, event: top::Event) {
        self.secrets_kept &= fixtures::token_absent(&event);
        top::step(&mut self.top, &self.env, event, &mut self.top_out);
        self.take_top();
        self.top.reclaim();
    }

    pub fn adopt(&mut self) {
        self.adopt_as(top::Role::Owned);
    }

    pub fn adopt_as(&mut self, role: top::Role) {
        for attempt in 0..8 {
            self.event(top::Event::Application {
                role: None,
                carried: None,
                request: Token::new(100 + attempt),
                token: Token::new(100 + attempt),
                retention: Duration::from_secs(60),
                ask: top::Ask::Adopt(top::Adoption {
                    project: 5,
                    home: role != top::Role::Context,
                    forge: REPO.forge,
                    host: Box::from(&b"forge.example"[..]),
                    owner: Box::from(self.fixture.owner),
                    name: Box::from(self.fixture.name),
                    prefix: Box::from(&b"temper/"[..]),
                    role,
                    landing: Box::from(&b"main"[..]),
                    ci: true,
                    checks: Box::new([]),
                }),
            });
            self.run_for(1);
            if !self.stored.contains_key(&top::Key::Repository(REPO)) {
                self.run_for(30);
            }
            if self.stored.contains_key(&top::Key::Repository(REPO)) {
                return;
            }
            self.run_for(60); // The person can retry after the durable refusal expires.
        }
        panic!("the person could not adopt the fixture repository");
    }

    #[must_use]
    pub fn wall(&self) -> Wall {
        self.env.wall
    }
    #[must_use]
    pub fn observations(&self) -> &[forge::Observation] {
        &self.joint.observations
    }
    #[must_use]
    pub fn seen(&self) -> &[top::Request] {
        self.seen.as_slice()
    }
    pub fn take_seen(&mut self) -> Box<[top::Request]> {
        let taken = core::mem::replace(&mut self.seen, List::with_capacity(256));
        taken.into_boxed()
    }
    #[must_use]
    pub fn writes(&self) -> u32 {
        self.writes
    }
    #[must_use]
    pub fn calls(&self) -> u32 {
        self.calls
    }
    #[must_use]
    pub fn stored(&self) -> &Map<top::Key, top::Stored> {
        &self.stored
    }
    pub fn slow_calls(&mut self, delay: Duration) {
        self.joint.fake_env.limits.latency_min = delay;
        self.joint.fake_env.limits.latency_max = delay;
    }
    /// Script a worker's pushed branch through the independent fake git.
    pub fn permission(&mut self, permission: raw::Permission) {
        forge::grant(&mut self.joint.domain, self.fixture.path, self.fixture.writer, permission);
    }

    pub fn produce(&mut self, branch: &[u8]) {
        self.produce_file(branch, b"file", b"change");
    }
    pub fn produce_file(&mut self, branch: &[u8], path: &[u8], content: &[u8]) {
        self.external(raw::Op::Git(raw::Git::Create { branch: Box::from(branch), commit: 1 }), 2);
        self.push_file(branch, path, content);
    }
    pub fn preload_history(&mut self, count: u32) {
        for number in 0..count {
            self.external(
                raw::Op::Write(raw::Write::CreateIssue {
                    title: Box::from(&b"old"[..]),
                    body: Box::from(number.to_be_bytes()),
                    labels: Box::new([]),
                }),
                2,
            );
        }
    }
    pub fn push(&mut self, branch: &[u8], content: &[u8]) {
        if self.joint.domain.branch(self.fixture.path, branch).is_none() {
            self.external(raw::Op::Git(raw::Git::Create { branch: Box::from(branch), commit: 1 }), 2);
        }
        self.push_file(branch, b"file", content);
    }
    pub fn push_file(&mut self, branch: &[u8], path: &[u8], content: &[u8]) {
        self.push_as(branch, path, content, self.fixture.writer);
    }
    pub fn push_as(&mut self, branch: &[u8], path: &[u8], content: &[u8], actor: u64) {
        let env = self.scripted_env(self.env.now);
        forge::advance(&mut self.joint.domain, &env, self.fixture.path, branch, path, content, actor)
            .expect("scripted push");
    }
    pub fn status(&mut self, branch: &[u8], check: raw::Check) {
        self.external(
            raw::Op::Write(raw::Write::Status {
                commit: self.branch(branch),
                context: Box::from(&b"build"[..]),
                state: check,
                description: Box::new([]),
                link: Box::new([]),
            }),
            self.fixture.writer,
        );
    }
    pub fn open_pull(&mut self, head: &[u8], base: &[u8]) -> u64 {
        let raw::Answer::Created(number) = self.external(
            raw::Op::Write(raw::Write::OpenPull {
                opens_empty: false,
                title: Box::from(&b"Change"[..]),
                body: Box::from(&b"Description"[..]),
                head: Box::from(head),
                base: Box::from(base),
            }),
            2,
        ) else {
            panic!("fixture pull opened")
        };
        number
    }
    pub fn close(&mut self, number: u64) {
        self.external(raw::Op::Write(raw::Write::Close { number }), 2);
    }
    pub fn resolve(&mut self, branch: &[u8], content: &[u8]) {
        let head = self.branch(branch);
        let base = self.branch(b"main");
        let merged = forge::merge_commit(
            &mut self.joint.domain,
            &self.joint.fake_env.limits,
            head,
            base,
            Box::new([raw::File { path: Box::from(&b"file"[..]), content: Box::from(content) }]),
            b"Resolve conflict",
        )
        .expect("fixture merge commit");
        assert_eq!(
            self.external(
                raw::Op::Git(raw::Git::Push { branch: Box::from(branch), commit: merged, expected: Some(head) }),
                self.fixture.writer,
            ),
            raw::Answer::Pushed(raw::Pushed::Pushed)
        );
    }
    #[must_use]
    pub fn branch(&self, branch: &[u8]) -> u64 {
        self.joint.domain.branch(self.fixture.path, branch).expect("fixture branch")
    }

    /// Old calls reach their accounting terminal without delivering an outcome to the new top.
    pub fn restart(&mut self) {
        self.finish();
        let component = Component::new(self.fixture.settings(), &fixtures::limits(), &fixtures::protocol_shared())
            .expect("fresh component");
        self.joint.replace(component);
        self.accept_installations();
        self.top = Self::top(self.seed, self.fixture.writer, &self.env.limits);
        let records: Vec<_> = self.stored.iter().map(|(_, row)| row.clone()).collect();
        for record in records {
            self.event(top::Event::Restore { record });
        }
        self.event(top::Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic });
    }
    pub fn issue(&mut self, title: &[u8], body: &[u8]) {
        self.external_at(
            raw::Op::Write(raw::Write::CreateIssue {
                title: Box::from(title),
                body: Box::from(body),
                labels: Box::from([]),
            }),
            2,
            self.env.now,
        );
    }
    /// Accounts the old process's abandoned calls at their deadlines, without delivering them to the top.
    pub fn finish(&mut self) {
        self.generation += 1;
        let mut terminal = Env { now: self.joint.env.now, wall: self.joint.env.wall, limits: self.joint.env.limits };
        terminal.now = terminal.now.saturating_add(terminal.limits.deadline);
        self.joint.component.fire(&terminal, &mut self.joint.above, &mut self.joint.below);
        while let Some(event) = self.joint.above.pop() {
            if let skein_forge_connection::Event::Answered(answer) = event {
                assert!(self.joint.answers.insert(answer.call.raw(), answer).is_none(), "one terminal");
            }
        }
        self.take_answers();
        assert!(self.pending.is_empty(), "every submitted call ended");
        assert_eq!(u64::from(self.calls), self.answered.len() as u64, "each call has one terminal");
    }
    fn external(&mut self, op: raw::Op, user: u64) -> raw::Answer {
        self.external_at(op, user, self.env.now)
    }
    // GitHub fake time is wall-based; Forgejo fake time is monotonic.
    fn scripted_env(&self, now: Time) -> Env<forge::Config> {
        let mut config = self.joint.fake_env.limits;
        config.latency_min = Duration::ZERO;
        config.latency_max = Duration::ZERO;
        let offset = self.joint.fake_env.now.as_nanos().saturating_sub(self.env.now.as_nanos());
        let now = Time::from_nanos(now.as_nanos().saturating_add(offset));
        let wall = if offset == 0 { self.joint.fake_env.wall } else { Wall::from_nanos(now.as_nanos()) };
        Env { now, wall, limits: config }
    }
    fn external_at(&mut self, op: raw::Op, user: u64, now: Time) -> raw::Answer {
        let env = self.scripted_env(now);
        let mut output = Queue::with_capacity(forge::MAX_OUT);
        forge::step(
            &mut self.joint.domain,
            &env,
            forge::Event::Call {
                reply_to: ReplyTo::new(Token::new(u64::MAX)),
                user,
                repository: Box::from(self.fixture.path),
                op,
            },
            &mut output,
        );
        let mut answer = None;
        for _ in 0..4 {
            if self.joint.domain.is_due(env.now) {
                forge::fire(&mut self.joint.domain, &env, &mut output);
            }
        }
        while let Some(request) = output.pop() {
            if let forge::Request::Reply { result, .. } = request {
                answer = Some(result.expect("scripted operation"));
            }
        }
        self.joint.domain.reclaim();
        answer.expect("scripted answer")
    }
    pub fn run_for(&mut self, seconds: u64) {
        let end = self.env.now.saturating_add(Duration::from_secs(seconds));
        for _ in 0..200_000 {
            self.joint.tick();
            // Only newly completed request lines: recording remains linear in requests.
            let complete = self.joint.request_lines()[self.request_times.len()..]
                .iter()
                .take_while(|line| line.ends_with(b"\r\n"))
                .count();
            self.request_times.resize(self.request_times.len() + complete, self.env.wall);
            self.take_answers();
            if self.top.is_ready() {
                top::resume(&mut self.top, &self.env, &mut self.top_out);
                self.take_top();
            } else if self.top.next_deadline().is_some_and(|at| at <= self.env.now) {
                top::fire(&mut self.top, &self.env, &mut self.top_out);
                self.take_top();
            } else if !self.joint.busy() {
                if self.env.now >= end {
                    return;
                }
                let offset = self.joint.fake_env.now.as_nanos().saturating_sub(self.env.now.as_nanos());
                let fake =
                    self.joint.domain.next_deadline().map(|at| Time::from_nanos(at.as_nanos().saturating_sub(offset)));
                let next = self
                    .top
                    .next_deadline()
                    .filter(|at| *at > self.env.now)
                    .unwrap_or(end)
                    .min(self.joint.component.next_deadline().filter(|at| *at > self.env.now).unwrap_or(end))
                    .min(fake.filter(|at| *at > self.env.now).unwrap_or(end))
                    .min(end)
                    .max(self.env.now.saturating_add(Duration::from_nanos(1)));
                self.joint.time(next);
                self.env.now = self.joint.env.now;
                self.env.wall = self.joint.env.wall;
            }
            self.top.reclaim();
        }
        panic!(
            "protocol world did not settle at {:?}, end {:?}, top {:?}, component {:?}, fake {:?}, busy {}",
            self.env.now,
            end,
            self.top.next_deadline(),
            self.joint.component.next_deadline(),
            self.joint.domain.next_deadline(),
            self.joint.busy()
        );
    }

    #[expect(clippy::too_many_lines, reason = "exhaustive routing of the forge outputs")]
    fn take_top(&mut self) {
        let mut calls = List::with_capacity(self.top_out.len());
        let mut restart = None;
        for _ in 0..self.top_out.len() {
            let request = self.top_out.pop().expect("counted top output");
            self.secrets_kept &= fixtures::token_absent(&request);
            match request {
                top::Request::ApplicationPart { request, .. } => {
                    top::step(&mut self.top, &self.env, top::Event::Adopt { request }, &mut self.top_out);
                }
                top::Request::ApplicationDone { .. } | top::Request::MarkedForges | top::Request::Marked { .. } => {}
                top::Request::ApplicationAgain { .. } => panic!("application capacity available"),
                top::Request::Adopted { reply_to, result } => {
                    let outcome = match &result {
                        Ok(adopted) => jig_domain_people::Outcome::Adopted { project: adopted.repository.project },
                        Err(_) => jig_domain_people::Outcome::Refused(jig_domain_people::Refusal::Unknown),
                    };
                    top::step(
                        &mut self.top,
                        &self.env,
                        top::Event::ApplicationAnswered { request: reply_to, outcome },
                        &mut self.top_out,
                    );
                    self.seen.push(top::Request::Adopted { reply_to, result }).expect("world output capacity");
                }
                top::Request::BriefClient { event } => {
                    top::step(&mut self.top, &self.env, top::Event::Client(event), &mut self.top_out);
                }
                top::Request::ProjectionEffect { row, entry, .. } => {
                    let mut kept = Queue::with_capacity(top::max_out(&self.env.limits));
                    top::step(&mut self.top, &self.env, top::Event::KeepProjection { row, entry }, &mut kept);
                    for _ in 0..kept.len() {
                        let top::Request::Save { record, .. } = kept.pop().expect("projection keep count") else {
                            panic!("admitted projection saves only")
                        };
                        self.stored.insert(key(&record), record).expect("world store capacity");
                    }
                }
                top::Request::Save { record, .. } => {
                    let key = key(&record);
                    self.stored.insert(key, record).expect("world store capacity");
                }
                top::Request::Erase { key } => {
                    self.stored.remove(&key).expect("erased row exists");
                }
                top::Request::Call { call, repository, op } => {
                    calls.push((call, repository, op)).expect("bounded calls");
                }
                top::Request::RestartDone { stage } => {
                    restart = match stage {
                        top::RestartStage::Restored => Some(top::Event::ReadAfresh),
                        top::RestartStage::ReadAfresh => Some(top::Event::SettleOutbox),
                        top::RestartStage::Settled => None,
                    };
                    self.seen.push(top::Request::RestartDone { stage }).expect("world output capacity");
                }
                other @ (top::Request::Settled { .. }
                | top::Request::Bound { .. }
                | top::Request::ListDemoActs
                | top::Request::SendDemoAct(_)
                | top::Request::DemoStart { .. }
                | top::Request::StopGroup { .. }
                | top::Request::SetUpGroup { .. }
                | top::Request::Connect { .. }
                | top::Request::Manifest { .. }
                | top::Request::Reachable { .. }
                | top::Request::Install { .. }
                | top::Request::Accept { .. }
                | top::Request::CarriedUnused { .. }
                | top::Request::EraseCarried { .. }
                | top::Request::SetUp { .. }
                | top::Request::Resource { .. }
                | top::Request::Taken { .. }
                | top::Request::Refused { .. }
                | top::Request::Outcome { .. }
                | top::Request::ProjectionOutcome { .. }
                | top::Request::NotMade { .. }
                | top::Request::ContinueRelease { .. }
                | top::Request::EffectsSettled { .. }
                | top::Request::Released { .. }
                | top::Request::Retained { .. }
                | top::Request::ReleaseFailed { .. }
                | top::Request::WakeProcedure { .. }
                | top::Request::ProjectAfter { .. }
                | top::Request::ProjectionSettled { .. }
                | top::Request::GoalTopic { .. }
                | top::Request::GoalUntopic { .. }
                | top::Request::ProjectionFailed { .. }
                | top::Request::ChangeDecision { .. }
                | top::Request::News { .. }
                | top::Request::Read { .. }
                | top::Request::BriefReady { .. }
                | top::Request::BriefSized { .. }
                | top::Request::BriefTaken { .. }
                | top::Request::Drift { .. }) => {
                    self.seen.push(other).expect("world output capacity");
                }
            }
        }
        for (call, repository, op) in calls.into_boxed() {
            self.submit(call, repository, op);
        }
        if let Some(event) = restart {
            self.event(event);
        }
    }

    fn submit(&mut self, call: Token, repository: skein_forge::api::Repository, op: skein_forge::api::Op) {
        self.calls += 1;
        if matches!(op, skein_forge::api::Op::Write(_)) {
            self.writes += 1;
        }
        self.serial += 1;
        let items = matches!(op, skein_forge::api::Op::Read(skein_forge::api::Read::Items { .. }));
        if matches!(op, skein_forge::api::Op::Read(skein_forge::api::Read::Items { page: 1, .. })) {
            self.item_start = self.tally().routes.get(&Route::Issues).copied().unwrap_or(0);
            self.item_pages.push(0);
        }
        let sample = if let skein_forge::api::Op::Read(read) = &op
            && (matches!(read, skein_forge::api::Read::Branch { branch } if branch.as_ref() == b"main")
                || matches!(read, skein_forge::api::Read::Checks { .. }))
        {
            self.base_reads.push((self.env.wall, read.clone(), None));
            Some(self.base_reads.len() - 1)
        } else {
            None
        };
        assert!(self.pending.insert(self.serial, (self.generation, call, items, sample)).is_none(), "unique call");
        self.joint.call(self.serial, repository, op);
    }
    fn take_answers(&mut self) {
        for (id, answer) in core::mem::take(&mut self.joint.answers) {
            assert!(self.answered.insert(id), "call {id} answered once");
            let (generation, call, items, sample) = self.pending.remove(&id).expect("submitted call");
            if let Some(sample) = sample
                && let Ok(skein_forge::api::Answer::Commit(commit)) = &answer.result
            {
                self.base_reads[sample].2 = Some(*commit);
            }
            if items {
                let pages = self.tally().routes.get(&Route::Issues).copied().unwrap_or(0) - self.item_start;
                *self.item_pages.last_mut().expect("listing pass") = pages;
            }
            self.costs += u64::from(answer.cost);
            self.secrets_kept &= fixtures::token_absent(&answer);
            if generation == self.generation {
                self.event(top::Event::Client(skein_forge_client::Event::Answered {
                    free: 0,
                    rate: None,
                    call,
                    cost: answer.cost,
                    result: answer.result,
                }));
            }
        }
    }
    /// Completed HTTP request lines and their world-clock send times; no headers.
    pub fn requests(&self) -> impl Iterator<Item = (Wall, &[u8])> {
        self.request_times.iter().copied().zip(self.joint.request_lines().iter().map(Vec::as_slice))
    }
    #[must_use]
    pub fn delivery_hints(&self) -> usize {
        self.joint.hints.len()
    }
    #[must_use]
    pub fn listing_passes(&self) -> &[u64] {
        &self.item_pages
    }
    #[must_use]
    pub fn base_reads(&self) -> &[BaseRead] {
        &self.base_reads
    }
    #[must_use]
    pub fn costs(&self) -> u64 {
        self.costs
    }
    #[must_use]
    pub fn tally(&self) -> &Tally {
        self.joint.tally()
    }
    #[must_use]
    pub fn start_reads(&self) -> u64 {
        self.tally().routes.get(&Route::User).copied().unwrap_or(0)
            + self.tally().routes.get(&Route::ApiSettings).copied().unwrap_or(0)
    }
    #[must_use]
    pub fn issues_made(&self) -> u32 {
        u32::try_from(
            self.joint
                .observations
                .iter()
                .filter(|o| matches!(o, forge::Observation::Opened { kind: raw::Kind::Issue, by: 1, .. }))
                .count(),
        )
        .expect("bounded issues")
    }
    #[must_use]
    pub fn token_kept(&self) -> bool {
        self.secrets_kept && fixtures::token_absent(&self.joint.observations)
    }
}
fn key(row: &top::Stored) -> top::Key {
    match row {
        top::Stored::Call { key, .. } => top::Key::Call(*key),
        top::Stored::Repository(row) => top::Key::Repository(row.provider),
        top::Stored::Hold(row) => top::Key::Hold(row.name.clone()),
        top::Stored::Names { task, .. } => top::Key::Names(*task),
        top::Stored::Subscription(row) => top::Key::Subscription { task: row.task, topic: row.topic.clone() },
        top::Stored::BranchHead(row) => top::Key::BranchHead(row.name.clone()),
        top::Stored::PullState(row) => top::Key::PullState(row.name.clone()),
        top::Stored::Ci(row) => top::Key::Ci { repository: row.repository, head: row.head },
        top::Stored::Landed { commit, .. } => top::Key::Landed(*commit),
        top::Stored::ProposedEffect(row) => top::Key::ProposedEffect(row.number),
        top::Stored::Entry { entry, .. } => top::Key::Entry(entry.number),
        top::Stored::Client(row) => top::Key::Client(match row {
            skein_forge_client::Stored::Live(row) => skein_forge_client::Key::Live(row.watch.resource.clone()),
            skein_forge_client::Stored::Repository(row) => skein_forge_client::Key::Repository(row.repository),
            skein_forge_client::Stored::Writer(_) => {
                unreachable!("the connector keeps the client's writer as Stored::Writer")
            }
        }),
        top::Stored::Change(row) => top::Key::Change(row.task),
        top::Stored::Issue(row) => top::Key::Issue(row.goal),
        top::Stored::Demo(row) => top::Key::Demo(row.forge),
        top::Stored::Binding { address, .. } => top::Key::Binding(address.clone()),
        top::Stored::DemoAct(row) => top::Key::DemoAct(row.key),
        top::Stored::Flight { task, entry } => top::Key::Flight { task: *task, entry: *entry },
        top::Stored::Release(row) => top::Key::Release(row.task),
        top::Stored::Application { token, .. } => top::Key::Application(*token),
        top::Stored::Writer(row) => top::Key::Writer(row.forge),
    }
}
