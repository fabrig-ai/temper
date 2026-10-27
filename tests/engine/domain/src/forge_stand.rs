//! An independent fake forge beneath the direct root fixture.
use skein_fake_forge_domain::{self as fake, api as raw};
use skein_forge::api;
use skein_lib::{Env, Queue, ReplyTo, Time, Token, Wall};
use temper_engine_domain as engine;
use temper_engine_forge_world::translate;

pub struct ForgeStand {
    forge: u16,
    domain: fake::Domain,
    env: Env<fake::Config>,
    out: Queue<fake::Request>,
    pending: std::collections::BTreeMap<Token, api::Op>,
    limits: skein_forge_client::Limits,
    repositories: std::collections::BTreeMap<u32, Box<[u8]>>,
    pub calls: u32,
}

impl Default for ForgeStand {
    fn default() -> Self {
        Self::new()
    }
}

impl ForgeStand {
    /// This independent peer stands for one configured forge, defaulting to one.
    #[must_use]
    pub fn at_forge(mut self, forge: u16) -> Self {
        self.forge = forge;
        self
    }
    #[must_use]
    pub fn quiescent(&self) -> bool {
        self.pending.is_empty() && self.out.is_empty()
    }
    #[must_use]
    pub fn new() -> ForgeStand {
        Self::with(&[(b"org", b"repo", 2), (b"org", b"private", 3)])
    }
    #[must_use]
    pub fn with(repositories: &[(&[u8], &[u8], u32)]) -> ForgeStand {
        Self::with_checks(repositories, &[])
    }

    /// Repositories with the named required CI contexts, all passing.
    #[must_use]
    pub fn with_checks(repositories: &[(&[u8], &[u8], u32)], contexts: &[&[u8]]) -> ForgeStand {
        Self::with_setup(
            repositories
                .iter()
                .map(|(owner, repository, id)| {
                    let mut name = owner.to_vec();
                    name.push(b'/');
                    name.extend_from_slice(repository);
                    raw::Setup {
                        id: *id,
                        name: name.into(),
                        default: b"main".as_slice().into(),
                        tree: Box::new([]),
                        labels: Box::new([]),
                        checks: raw::Checks {
                            contexts: contexts.iter().map(|context| Box::from(*context)).collect(),
                            latency_min: skein_lib::Duration::ZERO,
                            latency_max: skein_lib::Duration::ZERO,
                            silent: 0,
                            passes: 1000,
                            reruns: 0,
                            cue: None,
                        },
                        protection: None,
                        hooked: false,
                    }
                })
                .collect(),
        )
    }

    /// Repository contents and CI supplied by the story, including silent CI.
    /// Calls remain synchronous; delayed required reads use the forge route world.
    #[must_use]
    pub fn with_setup(setups: Box<[raw::Setup]>) -> ForgeStand {
        let mut config = temper_engine_forge_world::fake_config();
        config.limits.repositories = u32::try_from(setups.len()).expect("bounded repositories");
        // The fake holds owner/repository as one name; callers can supply long
        // names to exercise the real root's independent field admission limits.
        for setup in &setups {
            config.limits.name_bytes =
                config.limits.name_bytes.max(u32::try_from(setup.name.len()).expect("fixture name"));
        }
        config.latency_min = skein_lib::Duration::ZERO;
        config.latency_max = skein_lib::Duration::ZERO;
        let mut domain = fake::Domain::new(&config, 71);
        let mut names = std::collections::BTreeMap::new();
        for setup in setups {
            let id = setup.id;
            let name = setup.name.clone();
            fake::repository(&mut domain, &config, setup);
            if name.as_ref() != b"org/private" {
                for (user, permission) in
                    [(1, raw::Permission::Admin), (2, raw::Permission::Write), (7, raw::Permission::Read)]
                {
                    fake::grant(&mut domain, &name, user, permission);
                }
            }
            assert!(names.insert(id, name).is_none());
        }
        ForgeStand {
            forge: 1,
            domain,
            env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits: config },
            out: Queue::with_capacity(fake::MAX_OUT),
            pending: std::collections::BTreeMap::new(),
            limits: crate::fixtures::forge_limits().forge.client,
            repositories: names,
            calls: 0,
        }
    }

    /// Push one commit on a new branch, as `forge_routes.rs`'s `check_gate_landing` does.
    pub fn push_branch(&mut self, repository: u32, branch: &[u8]) {
        assert!(self.quiescent(), "push between stand calls");
        let name = self.repositories.get(&repository).expect("configured repository").clone();
        // Finish setup's CI before making the push's synchronous calls.
        while self.domain.is_due(self.env.now) {
            fake::fire(&mut self.domain, &self.env, &mut self.out);
        }
        assert!(self.out.is_empty(), "setup emits no replies or hooks");
        // Read this repository's base: commit ids depend on repository creation order.
        fake::step(
            &mut self.domain,
            &self.env,
            fake::Event::Call {
                reply_to: ReplyTo::new(Token::new(10_000)),
                user: 1,
                repository: name.clone(),
                op: raw::Op::Git(raw::Git::Fetch { want: raw::Want::Default }),
            },
            &mut self.out,
        );
        fake::fire(&mut self.domain, &self.env, &mut self.out);
        let Some(fake::Request::Reply { result: Ok(raw::Answer::Commit(commit)), .. }) = self.out.pop() else {
            panic!("repository's main head");
        };
        fake::step(
            &mut self.domain,
            &self.env,
            fake::Event::Call {
                reply_to: ReplyTo::new(Token::new(10_001)),
                user: 1,
                repository: name.clone(),
                op: raw::Op::Git(raw::Git::Create { branch: branch.into(), commit }),
            },
            &mut self.out,
        );
        fake::fire(&mut self.domain, &self.env, &mut self.out);
        assert!(matches!(
            self.out.pop(),
            Some(fake::Request::Reply { result: Ok(raw::Answer::Branch(raw::Created::Created)), .. })
        ));
        fake::advance(&mut self.domain, &self.env, &name, branch, b"file", b"review this diff", 1)
            .expect("pushed change head");
        // The push's zero-latency CI is complete before the engine starts.
        while self.domain.is_due(self.env.now) {
            fake::fire(&mut self.domain, &self.env, &mut self.out);
        }
        assert!(self.quiescent(), "push emits no unrelated replies or hooks");
        self.domain.reclaim();
    }

    /// A scripted worker pushes a new head, then publishes its CI status.
    pub fn push_checked(&mut self, repository: u32, branch: &[u8], first: bool, passing: bool) {
        if first {
            self.push_branch(repository, branch);
        }
        assert!(self.quiescent(), "worker push between forge calls");
        let name = self.repositories.get(&repository).expect("worker repository").clone();
        // Distinct pushes must have distinct forge update times for incremental polls.
        self.env.now = self.env.now.saturating_add(skein_lib::Duration::from_secs(1));
        self.env.wall = Wall::from_nanos(self.env.now.as_nanos());
        let content: &[u8] = if passing { b"repaired" } else { b"needs repair" };
        let head = fake::advance(&mut self.domain, &self.env, &name, branch, b"file", content, 1)
            .expect("worker pushed a new head");
        // The fake recomputes pull mergeability two seconds after a push.
        self.env.now = self.env.now.saturating_add(skein_lib::Duration::from_secs(2));
        self.env.wall = Wall::from_nanos(self.env.now.as_nanos());
        while self.domain.is_due(self.env.now) {
            fake::fire(&mut self.domain, &self.env, &mut self.out);
        }
        assert!(self.out.is_empty());
        fake::step(
            &mut self.domain,
            &self.env,
            fake::Event::Call {
                reply_to: ReplyTo::new(Token::new(10_002)),
                user: 1,
                repository: name,
                op: raw::Op::Write(raw::Write::Status {
                    commit: head,
                    context: b"build".as_slice().into(),
                    state: if passing { raw::Check::Passed } else { raw::Check::Failed },
                }),
            },
            &mut self.out,
        );
        fake::fire(&mut self.domain, &self.env, &mut self.out);
        assert!(matches!(self.out.pop(), Some(fake::Request::Reply { result: Ok(raw::Answer::Done), .. })));
        assert!(self.quiescent());
        self.domain.reclaim();
    }

    /// Observe between engine steps, with no pending calls or due fake work.
    ///
    /// Reads leave the fake as the engine sees it: rate limiting and faults
    /// are disabled, latencies and CI outcomes are fixed, and this never moves
    /// time or fires another event. Internally the fake advances its RNG,
    /// answered tally and reclaimed call-slot generation; none of those changes
    /// repository ids, CI ids, quotas or future event times in this stand.
    /// The engine's `calls`, `pending` and translation limits stay untouched.
    pub fn read(&mut self, repository: api::Repository, read: api::Read) -> api::Answer {
        assert!(self.quiescent(), "read between stand calls");
        assert!(!self.domain.is_due(self.env.now), "finish fake work before observing");
        assert_eq!(repository.forge, self.forge);
        let op = api::Op::Read(read);
        let name = translate::locate_name(&op)
            .unwrap_or_else(|| self.repositories.get(&repository.repository).expect("configured repository").clone());
        let observation = Token::new(90_001);
        fake::step(
            &mut self.domain,
            &self.env,
            fake::Event::Call {
                reply_to: ReplyTo::new(observation),
                user: 1,
                repository: name,
                op: translate::op(&op, &self.limits),
            },
            &mut self.out,
        );
        fake::fire(&mut self.domain, &self.env, &mut self.out);
        let Some(fake::Request::Reply { to, result }) = self.out.pop() else {
            panic!("one synchronous observation reply");
        };
        assert_eq!(to.into_token(), observation);
        let answer = translate::answer(&op, result.expect("stand observation"), &self.limits);
        self.domain.reclaim();
        assert!(self.quiescent());
        assert!(!self.domain.is_due(self.env.now));
        answer
    }

    pub fn call(&mut self, call: Token, repository: api::Repository, op: api::Op, limits: &skein_forge_client::Limits) {
        self.limits = *limits;
        self.calls = self.calls.checked_add(1).expect("stand calls bound");
        assert_eq!(repository.forge, self.forge);
        let name: Box<[u8]> = if let Some(name) = translate::locate_name(&op) {
            name
        } else {
            self.repositories.get(&repository.repository).expect("stand repository is configured").clone()
        };
        let raw = translate::op(&op, limits);
        assert!(self.pending.insert(call, op).is_none());
        fake::step(
            &mut self.domain,
            &self.env,
            fake::Event::Call { reply_to: ReplyTo::new(call), user: 1, repository: name, op: raw },
            &mut self.out,
        );
    }

    pub fn replies(&mut self) -> Vec<engine::Event> {
        let mut replies = Vec::new();
        for _ in 0..8 {
            if !self.domain.is_due(self.env.now) {
                break;
            }
            fake::fire(&mut self.domain, &self.env, &mut self.out);
        }
        for _ in 0..self.out.len() {
            if let fake::Request::Reply { to, result } = self.out.pop().expect("stand output") {
                let call = to.into_token();
                let op = self.pending.remove(&call).expect("one stand terminal");
                let result = match result {
                    Ok(answer) => Ok(translate::answer(&op, answer, &self.limits)),
                    Err(error) => Err(translate::error(error, self.env.now)),
                };
                replies.push(engine::Event::Forge(Box::new(temper_engine_domain_forge::Event::Client(
                    skein_forge_client::Event::Answered { free: 0, rate: None, call, cost: 1, result },
                ))));
            }
        }
        self.domain.reclaim();
        replies
    }
}
