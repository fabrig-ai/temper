//! Admission order and cues on one checked real world.
use crate::{agent, calls::Caller, fixtures, forge, llm, service::ServiceHost};
use skein_fake_peers::llm::Observation::{Answered, Query};
use skein_io::kernel::{Complete, Exit, ServiceSignal, Submit};
use skein_lib::{Queue, Time, Wall};
use skein_shell::Host;
use skein_world::{
    HostedProgram, Referee,
    real::{CheckedOutcome, Controls, World},
};

pub(crate) enum Proc {
    Llm(Box<Llm>),
    Forge(Box<skein_fake_peers::forge::Peer>),
    Service(Box<ServiceHost>),
    Caller(Box<Caller>),
    Ctl(Box<crate::calls::Recorded>),
    Agent(Box<skein_world::PipeHost<smith_agent_shell::Agent>>),
    Git(Box<temper_fake_git_hosted::Git<temper_fake_git_hosted::Typed>>),
    GitRoute(Box<temper_fake_git::Git<temper_fake_git::Route>>),
}
impl Proc {
    fn host(&self) -> &dyn Host {
        match self {
            Self::Llm(h) => &h.peer,
            Self::Forge(h) => h.as_ref(),
            Self::Service(h) => h.as_ref(),
            Self::Caller(h) => h.as_ref(),
            Self::Ctl(h) => h.as_ref(),
            Self::Agent(h) => h.as_ref(),
            Self::Git(h) => h.as_ref(),
            Self::GitRoute(h) => h.as_ref(),
        }
    }
    fn host_mut(&mut self) -> &mut dyn Host {
        match self {
            Self::Llm(h) => &mut h.peer,
            Self::Forge(h) => h.as_mut(),
            Self::Service(h) => h.as_mut(),
            Self::Caller(h) => h.as_mut(),
            Self::Ctl(h) => h.as_mut(),
            Self::Agent(h) => h.as_mut(),
            Self::Git(h) => h.as_mut(),
            Self::GitRoute(h) => h.as_mut(),
        }
    }
}
impl Host for Proc {
    fn iterate(&mut self, now: Time, wall: Wall) {
        if !matches!(self, Self::Llm(llm) if llm.scripts.is_some()) {
            if let Self::Llm(llm) = self
                && std::mem::take(&mut llm.cue)
            {
                llm.peer.release(crate::cluster::PLACED).expect("placement hold");
            }
            let host = self.host_mut();
            if host.work_pending(now)
                || !host.completions().is_empty()
                || host.next_deadline().is_some_and(|deadline| deadline <= now)
                || host.next_policy_deadline().is_some_and(|deadline| deadline <= now)
            {
                host.iterate(now, wall);
            }
        }
    }
    fn drain(&mut self) {
        self.host_mut().drain();
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        self.host_mut().completions()
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        self.host_mut().submissions()
    }
    fn work_pending(&self, now: Time) -> bool {
        !matches!(self, Self::Llm(llm) if llm.scripts.is_some()) && self.host().work_pending(now)
    }
    fn next_deadline(&self) -> Option<Time> {
        self.host().next_deadline()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        self.host().next_policy_deadline()
    }
    fn is_empty(&self) -> bool {
        self.host().is_empty()
    }
    fn exit(&self) -> Option<Exit> {
        self.host().exit()
    }
    fn worst_case(&self) -> u64 {
        self.host().worst_case()
    }
    fn operations(&self) -> u32 {
        self.host().operations()
    }
}
pub(crate) fn run(
    scratch: &skein_scratch::Scratch,
    recipe: impl FnOnce() -> llm::Recipe,
    steps: impl FnOnce() -> Vec<crate::calls::Step>,
    direct_key: bool,
) -> CheckedOutcome<Proc> {
    let mut world = World::new_controlled_checked(|controls| Judge {
        controls,
        workers: Vec::new(),
        engine_stopping: false,
        stopping: false,
        finished: false,
        armed: false,
        wake: None,
        forge: None,
        github: None,
        llm_history: LlmHistory::default(),
    });
    world.spawn(|| {
        Proc::Llm(Box::new(Llm { peer: llm::peer(recipe()).expect("independent LLM peer"), scripts: None, cue: false }))
    });
    world
        .spawn_signals(|signals| Proc::Service(Box::new(ServiceHost::new(scratch.path().into(), signals, direct_key))));
    host(&mut world, scratch.path(), steps, direct_key, 1);
    world.run(&skein_shell::Clock::new(), crate::PATIENCE)
}
type Scripts = fn(std::net::SocketAddr) -> Box<[skein_fake_llm_domain::api::Script]>;
pub(crate) struct Llm {
    pub(crate) peer: skein_fake_peers::llm::Peer,
    scripts: Option<Scripts>,
    cue: bool,
}
impl std::ops::Deref for Llm {
    type Target = skein_fake_peers::llm::Peer;
    fn deref(&self) -> &Self::Target {
        &self.peer
    }
}
impl std::ops::DerefMut for Llm {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.peer
    }
}
pub(crate) fn run_forge(
    scratch: &skein_scratch::Scratch,
    fixture: forge::Fixture,
    scripts: Scripts,
    steps: impl FnOnce() -> Vec<crate::calls::Step>,
) -> skein_world::real::Outcome<Proc> {
    let state = std::rc::Rc::new(std::cell::RefCell::new(None));
    let mut world = World::new_controlled(|controls| Judge {
        controls,
        workers: Vec::new(),
        engine_stopping: false,
        stopping: false,
        finished: false,
        armed: false,
        wake: None,
        forge: Some(state.clone()),
        github: None,
        llm_history: LlmHistory::default(),
    });
    world.spawn(|| {
        Proc::Llm(Box::new(Llm {
            peer: llm::peer(llm::scripted(Box::new([]))).expect("deferred peer"),
            scripts: Some(scripts),
            cue: false,
        }))
    });
    world.spawn_signals(|signals| {
        Proc::Service(Box::new(ServiceHost::configured(scratch.path().into(), signals, Some(fixture), false)))
    });
    host(&mut world, scratch.path(), steps, false, 1);
    world.spawn(|| {
        let (forge, peer) = forge::State::new(fixture);
        *state.borrow_mut() = Some(forge);
        Proc::Forge(Box::new(peer))
    });
    world.host_roots(crate::git::program(), crate::git::roots);
    let outcome = world.run(&skein_shell::Clock::new(), crate::PATIENCE);
    crate::git::share(None);
    outcome
}
pub(crate) fn run_github(
    scratch: &skein_scratch::Scratch,
    fixture: crate::github::Fixture,
    scripts: Scripts,
    steps: impl FnOnce() -> Vec<crate::calls::Step>,
) -> skein_world::real::Outcome<Proc> {
    let (state, peer) = crate::github::State::new(fixture);
    let mut world = World::new_controlled(|controls| {
        let mut judge = cluster_judge(controls, &[], None);
        judge.github = Some(state);
        judge
    });
    world.spawn(|| {
        Proc::Llm(Box::new(Llm {
            peer: llm::peer(llm::scripted(Box::new([]))).expect("deferred peer"),
            scripts: Some(scripts),
            cue: false,
        }))
    });
    world
        .spawn_signals(|signals| Proc::Service(Box::new(ServiceHost::github(scratch.path().into(), signals, fixture))));
    // This story provisions its account explicitly, retaining every ctl output.
    host(&mut world, scratch.path(), steps, true, 1);
    world.spawn(|| Proc::Forge(Box::new(peer)));
    world.host_roots(crate::git::route_program(), crate::git::roots);
    world.run(&skein_shell::Clock::new(), crate::PATIENCE)
}
fn host<R: Referee<Proc>>(
    world: &mut World<Proc, R>,
    path: &std::path::Path,
    steps: impl FnOnce() -> Vec<crate::calls::Step>,
    direct_key: bool,
    agents: u32,
) {
    let root = skein_shell::open_root(path).expect("caller launch root");
    world.spawn_with_fds(vec![root], || {
        Proc::Caller(Box::new(if direct_key {
            Caller::new(steps(), path.into(), root)
        } else {
            Caller::provisioned(steps(), path.into(), root)
        }))
    });
    let limits = temper_client_shell_cli::limits::native();
    world.host_roots(
        HostedProgram {
            program: fixtures::CTL.into(),
            make: fixtures::ctl,
            instances: fixtures::CALLS_AT_ONCE,
            operations: skein_io::operations(&limits.io).expect("tools operations") * 2 + 5,
        },
        fixtures::ctl_roots,
    );
    let configuration = smith_agent_shell::config::read(&path.join("agent.json")).expect("agent operation bound");
    world.host_roots(
        HostedProgram {
            program: fixtures::AGENT.into(),
            make: agent::agent,
            instances: agents,
            operations: configuration.service.limits.routes + 3,
        },
        agent::roots,
    );
}
#[expect(clippy::struct_excessive_bools, reason = "independent referee cues for placement and shutdown")]
struct Judge {
    controls: Controls,
    workers: Vec<&'static str>,
    engine_stopping: bool,
    stopping: bool,
    armed: bool,
    finished: bool,
    wake: Option<Time>,
    forge: Option<std::rc::Rc<std::cell::RefCell<Option<forge::State>>>>,
    github: Option<crate::github::State>,
    llm_history: LlmHistory,
}

#[derive(Default)]
struct LlmHistory {
    seen: usize,
    pending: i64,
    visits: usize,
}
impl LlmHistory {
    fn observe(&mut self, observations: &[skein_fake_peers::llm::Observation]) -> i64 {
        // Deferred forge/GitHub peers are replaced before they run; a driven peer only appends.
        let suffix = observations.get(self.seen..).expect("LLM observation history never shrinks");
        self.visits += suffix.len();
        self.pending = suffix.iter().fold(self.pending, |n, event| {
            n + i64::from(matches!(event, Query { .. })) - i64::from(matches!(event, Answered { .. }))
        });
        self.seen = observations.len();
        self.pending
    }
}
impl Judge {
    fn worker_actions(&mut self, caller_end: usize, caller: &Caller, workers: &mut [Proc]) {
        if let Some(name) = caller.adding_worker() {
            let position = worker_position(workers, name).expect("configured worker");
            let Proc::Service(worker) = &mut workers[position] else { panic!("worker root") };
            let Some(crate::cluster::Shape::Worker { started, .. }) = &mut worker.cluster else {
                panic!("worker shape")
            };
            *started = true;
            caller.added.set(worker.linked() > 0);
        }
        if let Some(name) = caller.stopping_worker() {
            let position = worker_position(workers, name).expect("configured worker");
            let Proc::Service(worker) = &mut workers[position] else { panic!("worker root") };
            if !worker.stopping() {
                worker.stop_for_restart();
                self.controls.signal(caller_end + position, ServiceSignal::Terminate);
            }
            caller.worker_ready.set(worker.settled());
        }
        if let Some(name) = caller.starting_worker() {
            let position = worker_position(workers, name).expect("configured worker");
            let Proc::Service(worker) = &mut workers[position] else { panic!("worker root") };
            if worker.stopping() {
                worker.restart();
            }
            caller.worker_ready.set(worker.linked() > worker.lives().iter().map(|life| life.linked).sum());
        }
    }
}
impl Referee<Proc> for Judge {
    fn act(&mut self, now: Time, procs: &mut [Proc]) {
        self.wake = None;
        let engine_index = procs.iter().position(|proc| matches!(proc, Proc::Service(_))).expect("engine root");
        let caller_end = procs.iter().position(|proc| matches!(proc, Proc::Caller(_))).expect("caller root") + 1;
        let (roots, children) = procs.split_at_mut(caller_end);
        let [Proc::Llm(peer), Proc::Service(service), Proc::Caller(caller)] = roots else {
            panic!("root admission order")
        };
        let (workers, children) = children.split_at_mut(self.workers.len());
        for proc in workers.iter_mut() {
            let Proc::Service(worker) = proc else { panic!("worker root") };
            worker.address = peer.address();
            if let Some(crate::cluster::Shape::Worker { engine, .. }) = &mut worker.cluster {
                *engine = service.workers_address;
            }
        }
        self.worker_actions(caller_end, caller, workers);
        if placed(workers) && !self.finished {
            peer.cue = true;
        }
        if !self.armed {
            for name in caller.held_names() {
                let name = temper_client_tools::jig::key::Name::new(name).expect("held request name");
                service.arm(temper_client_tools::key::engine(&name));
            }
            self.armed = true;
        }
        for proc in children.iter_mut() {
            if let Proc::Ctl(recorded) = proc {
                if recorded.setup && !caller.setup() {
                    continue;
                }
                let result = &mut caller.calls[recorded.call];
                result.locked = recorded.locked;
                result.released = recorded.released;
                result.busy = recorded.busy;
            }
        }
        if let Some(state) = &self.forge {
            let mut borrowed = state.borrow_mut();
            let state = borrowed.as_mut().expect("spawned forge");
            let Proc::Forge(forge) = &mut children[0] else { panic!("forge admission order") };
            service.forge_address = forge.address();
            for proc in workers.iter_mut() {
                let Proc::Service(worker) = proc else { panic!("worker root") };
                worker.forge_address = forge.address();
            }
            if let Some(address) = forge.address()
                && let Some(scripts) = peer.scripts.take()
            {
                peer.peer = llm::peer(llm::scripted(scripts(address))).expect("bound forge scripts");
            }
            if let Some(hooks) = service.hooks
                && !state.hooked
            {
                state.hand_hooks(hooks);
            }
            if caller.advancing() && state.hooked && !state.moved {
                state.advance(now, skein_shell::Clock::new().now().wall);
            }
            service.seen.collect(forge, &state.handle);
            caller
                .advanced
                .set(state.moved && !service.seen.deliveries().is_empty() && !service.seen.responses().is_empty());
            if service.is_empty() {
                forge.shutdown();
            }
        }
        if let Some(state) = &self.github {
            github_cues(state, peer, service, caller, children);
        }
        service.address = peer.address();
        if service.links() != 0
            && workers
                .iter()
                .all(|proc| matches!(proc, Proc::Service(worker) if !worker_started(worker) || worker.linked() > 0))
        {
            caller.released = true;
        }
        if let Some(call) = caller.watching {
            service.arm_watch(call);
            caller.watched = service.delivered();
        }
        if caller.is_empty() && !self.stopping {
            for (position, _) in self.workers.iter().enumerate() {
                self.controls.signal(caller_end + position, ServiceSignal::Terminate);
            }
            self.stopping = true;
        }
        if self.stopping && workers.iter().all(Host::is_empty) && !self.engine_stopping {
            self.controls.signal(engine_index, ServiceSignal::Terminate);
            self.engine_stopping = true;
        }
        if service.is_empty() && !self.finished {
            peer.shutdown();
            self.finished = true;
        }
    }
    fn observe(&mut self, now: Time, procs: &[Proc]) {
        let [Proc::Llm(peer), Proc::Service(service), Proc::Caller(caller), ..] = procs else {
            panic!("root admission order")
        };
        let caller_end = procs.iter().position(|proc| matches!(proc, Proc::Caller(_))).expect("caller root") + 1;
        let (workers, children) = procs[caller_end..].split_at(self.workers.len());
        let workers_changed = workers.iter().any(|proc| {
            let Proc::Service(worker) = proc else { panic!("worker root") };
            (peer.address().is_some() && worker.address.is_none())
                || (worker_started(worker)
                    && service.workers_address.is_some()
                    && worker.cluster.as_ref().is_some_and(|shape| !shape.ready()))
        });
        caller.held.set(service.held_at(caller.held_index()));
        caller.links.set(service.links());
        let lock_changed = children.iter().any(|proc| match proc {
            Proc::Ctl(recorded) => {
                if recorded.setup && !caller.setup() {
                    return false;
                }
                caller.calls[recorded.call].busy != recorded.busy
                    || caller.calls[recorded.call].locked != recorded.locked
                    || caller.calls[recorded.call].released != recorded.released
            }
            Proc::Llm(_)
            | Proc::Forge(_)
            | Proc::Service(_)
            | Proc::Caller(_)
            | Proc::Agent(_)
            | Proc::Git(_)
            | Proc::GitRoute(_) => false,
        });
        let forge_changed = self.forge.as_ref().is_some_and(|state| {
            let borrowed = state.borrow();
            let state = borrowed.as_ref().expect("spawned forge");
            let Proc::Forge(forge) = &children[0] else { panic!("forge root") };
            (forge.address().is_some() && service.forge_address.is_none())
                || (service.hooks.is_some() && !state.hooked)
                || (caller.advancing() && state.hooked && !state.moved)
                || (caller.advancing()
                    && !caller.advanced.get()
                    && (!forge.observations().is_empty() || !service.seen.responses().is_empty()))
        });
        let pending = self.llm_history.observe(peer.observations());
        let placing = placed(workers) && pending > 0;
        let named_worker = |name| {
            worker_position(workers, name).map(|position| {
                let Proc::Service(worker) = &workers[position] else { panic!("worker root") };
                worker.as_ref()
            })
        };
        let worker_action = caller
            .stopping_worker()
            .and_then(named_worker)
            .is_some_and(|worker| !worker.stopping() || worker.settled() && !caller.worker_ready.get())
            || caller.starting_worker().and_then(named_worker).is_some_and(|worker| {
                worker.stopping()
                    || worker.linked() > worker.lives().iter().map(|life| life.linked).sum()
                        && !caller.worker_ready.get()
            });
        if worker_action || caller.adding_worker().and_then(named_worker).is_some_and(|worker| {
            !worker_started(worker) || worker.linked() > 0 && !caller.added.get()
        })
            || placing
            || workers_changed
            || (self.stopping && workers.iter().all(Host::is_empty) && !self.engine_stopping)
            || (self.github.is_some() && children.first().is_some_and(|proc|
                matches!(proc, Proc::Forge(forge) if forge.address().is_some() && service.forge_address.is_none())))
            || forge_changed
            || lock_changed
            || (peer.address().is_some() && service.address.is_none())
            || (service.links() != 0
                && !caller.released
                && workers.iter().all(|proc| matches!(proc, Proc::Service(worker) if !worker_started(worker) || worker.linked() > 0)))
            || (caller.watching.is_some() && (!service.watching() || service.delivered() && !caller.watched))
            || (caller.is_empty() && !self.stopping)
            || (service.is_empty() && !self.finished)
        {
            self.wake = Some(now);
        }
    }
    fn next_deadline(&self) -> Option<Time> {
        self.wake
    }
    fn overdue(&self, _now: Time) -> Option<String> {
        None
    }
    fn passed(&self) -> bool {
        self.finished
    }
}

fn github_cues(
    state: &crate::github::State,
    peer: &mut Llm,
    service: &mut ServiceHost,
    caller: &Caller,
    children: &mut [Proc],
) {
    let Proc::Forge(forge) = &mut children[0] else { panic!("GitHub root") };
    service.forge_address = forge.address();
    if let Some(address) = forge.address()
        && let Some(scripts) = peer.scripts.take()
    {
        peer.peer = llm::peer(llm::scripted(scripts(address))).expect("bound GitHub scripts");
    }
    service.github_seen.checkpoint(forge, &caller.calls);
    if service.is_empty() {
        service.github_seen.collect(forge, &state.handle);
        forge.shutdown();
    }
}

pub(crate) fn run_cluster(
    workers: &[&'static str],
    scratch: &[skein_scratch::Scratch],
    recipe: impl FnOnce() -> llm::Recipe,
    steps: impl FnOnce() -> Vec<crate::calls::Step>,
) -> CheckedOutcome<Proc> {
    let mut world = World::new_controlled_checked(|controls| cluster_judge(controls, workers, None));
    cluster_roots(&mut world, workers, &[], scratch, recipe, None, steps);
    world.run(&skein_shell::Clock::new(), crate::PATIENCE)
}

pub(crate) fn run_cluster_forge(
    workers: &[&'static str],
    late: &[&str],
    scratch: &[skein_scratch::Scratch],
    fixture: forge::Fixture,
    scripts: Scripts,
    steps: impl FnOnce() -> Vec<crate::calls::Step>,
) -> skein_world::real::Outcome<Proc> {
    let state = std::rc::Rc::new(std::cell::RefCell::new(None));
    let mut world = World::new_controlled(|controls| cluster_judge(controls, workers, Some(state.clone())));
    cluster_roots(&mut world, workers, late, scratch, || llm::scripted(Box::new([])), Some((fixture, scripts)), steps);
    world.spawn(|| {
        let (forge, peer) = forge::State::new(fixture);
        *state.borrow_mut() = Some(forge);
        Proc::Forge(Box::new(peer))
    });
    world.run(&skein_shell::Clock::new(), crate::PATIENCE)
}

fn cluster_judge(
    controls: Controls,
    workers: &[&'static str],
    forge: Option<std::rc::Rc<std::cell::RefCell<Option<forge::State>>>>,
) -> Judge {
    Judge {
        controls,
        workers: workers.to_vec(),
        engine_stopping: false,
        stopping: false,
        finished: false,
        armed: false,
        wake: None,
        forge,
        github: None,
        llm_history: LlmHistory::default(),
    }
}

fn cluster_roots<R: Referee<Proc>>(
    world: &mut World<Proc, R>,
    workers: &[&'static str],
    late: &[&str],
    scratch: &[skein_scratch::Scratch],
    recipe: impl FnOnce() -> llm::Recipe,
    forge: Option<(forge::Fixture, Scripts)>,
    steps: impl FnOnce() -> Vec<crate::calls::Step>,
) {
    let (engine, worker_scratch) = scratch.split_first().expect("engine scratch");
    world.spawn(|| {
        Proc::Llm(Box::new(Llm {
            peer: llm::peer(recipe()).expect("cluster LLM"),
            scripts: forge.map(|(_, scripts)| scripts),
            cue: false,
        }))
    });
    world.spawn_signals(|signals| {
        Proc::Service(Box::new(ServiceHost::cluster(
            engine.path(),
            signals,
            crate::cluster::Shape::Engine(workers.to_vec()),
            forge.map(|(fixture, _)| fixture),
        )))
    });
    host(world, &engine.path().join("data"), steps, false, u32::try_from(workers.len()).expect("worker count"));
    for (name, scratch) in workers.iter().zip(worker_scratch) {
        world.spawn_signals(|signals| {
            Proc::Service(Box::new(ServiceHost::cluster(
                scratch.path(),
                signals,
                crate::cluster::Shape::Worker { name, engine: None, started: !late.contains(name) },
                forge.map(|(fixture, _)| fixture),
            )))
        });
    }
    let mut git = crate::git::program();
    git.instances = u32::try_from(workers.len()).expect("worker git count");
    world.host_roots(git, crate::git::roots);
}

fn worker_position(workers: &[Proc], name: &str) -> Option<usize> {
    workers.iter().position(|proc| matches!(proc, Proc::Service(worker)
        if matches!(worker.cluster, Some(crate::cluster::Shape::Worker { name: worker_name, .. }) if worker_name == name)))
}

fn worker_started(worker: &ServiceHost) -> bool {
    !matches!(worker.cluster, Some(crate::cluster::Shape::Worker { started: false, .. }))
}

fn placed(workers: &[Proc]) -> bool {
    !workers.is_empty()
        && workers
            .iter()
            .all(|proc| matches!(proc, Proc::Service(worker) if !worker_started(worker) || worker.runs.assigned()))
}

#[cfg(test)]
mod tests {
    use super::LlmHistory;
    use skein_fake_llm_domain::api;
    use skein_fake_peers::llm::Observation::{self, Accepted, Answered, Closed, Credential, Head, Query};
    use skein_lib::Token;

    #[test]
    fn llm_history_matches_every_prefix_and_visits_each_record_once() {
        let connection = Token::new(1);
        let query = || Query {
            connection,
            query: api::Query {
                credential: None,
                caching: api::Caching::Unscoped,
                model: b"fake".as_slice().into(),
                system: Box::new([]),
                tools: Box::new([]),
                choice: api::ToolChoice::None,
                messages: Box::new([]),
                max_tokens: 1,
            },
        };
        let answered = || Answered { connection: Some(connection), result: Ok(()) };
        let detached = || Answered { connection: None, result: Err(api::Error::Unavailable) };
        let rows: [(&str, Vec<Observation>); 7] = [
            ("empty", vec![]),
            ("balanced", vec![query(), answered()]),
            ("duplicate queries and waves", vec![query(), query(), answered(), detached(), query(), answered()]),
            ("closed route", vec![query(), Closed { connection }, detached()]),
            ("negative prefix", vec![detached(), query(), query(), answered()]),
            ("pending", vec![query(), answered(), query()]),
            (
                "other records",
                vec![
                    Accepted { connection },
                    Head { connection, headers: Box::new([]) },
                    Credential {
                        connection,
                        route: skein_llm::Provider::OpenAi,
                        scheme: Some(skein_llm::Scheme::Key),
                        index: Some(0),
                    },
                    Closed { connection },
                ],
            ),
        ];
        for (name, records) in rows {
            let mut history = LlmHistory::default();
            let mut reference_visits = 0;
            for length in 0..=records.len() {
                for _ in 0..3 {
                    let prefix = &records[..length];
                    let expected = prefix.iter().fold(0_i64, |n, event| {
                        n + i64::from(matches!(event, Query { .. })) - i64::from(matches!(event, Answered { .. }))
                    });
                    reference_visits += prefix.len();
                    let pending = history.observe(prefix);
                    assert_eq!(pending, expected, "{name}, prefix {length}");
                    for placed in [false, true] {
                        assert_eq!(placed && pending > 0, placed && expected > 0, "{name}, prefix {length}");
                    }
                    assert_eq!(history.visits, length, "{name}, prefix {length}");
                }
            }
            assert_eq!(history.seen, records.len(), "{name}");
            assert_eq!(history.visits, records.len(), "{name}");
            assert_eq!(reference_visits > history.visits, !records.is_empty(), "{name}");
        }
    }
}
