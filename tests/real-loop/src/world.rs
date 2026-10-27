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
    Forge(Box<ForgePeer>),
    Issuer(Box<skein_fake_peers::oauth::Peer>),
    Service(Box<ServiceHost>),
    Caller(Box<Caller>),
    Ctl(Box<crate::calls::Recorded>),
    Terminal(Box<temper_client_shell_terminal::terminal::Terminal>),
    Agent(Box<skein_world::PipeHost<smith_agent_shell::Agent>>),
    Git(Box<temper_fake_git_hosted::Git<temper_fake_git_hosted::Typed>>),
    GitRoute(Box<temper_fake_git::Git<temper_fake_git::Route>>),
}
impl Proc {
    fn host(&self) -> &dyn Host {
        match self {
            Self::Llm(h) => &h.peer,
            Self::Forge(h) => &h.peer,
            Self::Issuer(h) => h.as_ref(),
            Self::Service(h) => h.as_ref(),
            Self::Caller(h) => h.as_ref(),
            Self::Ctl(h) => h.as_ref(),
            Self::Terminal(h) => h.as_ref(),
            Self::Agent(h) => h.as_ref(),
            Self::Git(h) => h.as_ref(),
            Self::GitRoute(h) => h.as_ref(),
        }
    }
    fn host_mut(&mut self) -> &mut dyn Host {
        match self {
            Self::Llm(h) => &mut h.peer,
            Self::Forge(h) => &mut h.peer,
            Self::Issuer(h) => h.as_mut(),
            Self::Service(h) => h.as_mut(),
            Self::Caller(h) => h.as_mut(),
            Self::Ctl(h) => h.as_mut(),
            Self::Terminal(h) => h.as_mut(),
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
            if let Self::Llm(llm) = self
                && let Some(name) = llm.release.take()
            {
                llm.peer
                    .release(name)
                    .unwrap_or_else(|error| panic!("release LLM hold {}: {error:?}", String::from_utf8_lossy(name)));
                llm.released = Some(name);
            }
            let pending = self.host().work_pending(now);
            match self {
                Self::Llm(peer) => peer.pending_streak.observe(pending),
                Self::Service(service) => service.pending_streak.observe(pending),
                Self::Forge(peer) => peer.pending_streak.observe(pending),
                Self::Issuer(_)
                | Self::Caller(_)
                | Self::Ctl(_)
                | Self::Terminal(_)
                | Self::Agent(_)
                | Self::Git(_)
                | Self::GitRoute(_) => {}
            }
            let host = self.host_mut();
            if pending
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
        !matches!(self, Self::Llm(llm) if llm.scripts.is_some())
            && (matches!(self, Self::Llm(llm) if llm.release.is_some()) || self.host().work_pending(now))
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
    sign_in: bool,
) -> CheckedOutcome<Proc> {
    let patience = crate::PATIENCE;
    // As the browser tier does, reserve a kernel port before the immutable
    // registration is made. Only this story pays for an issuer.
    let reservation =
        sign_in.then(|| std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).expect("issuer port"));
    let issuer = reservation.as_ref().map(|listener| listener.local_addr().expect("issuer address"));
    let mut world = World::new_controlled_checked(|controls| Judge {
        patience: crate::patience::Patience::new(patience),
        controls,
        workers: Vec::new(),
        engine_stopping: false,
        stopping: false,
        finished: false,
        failure: None,
        armed: false,
        wake: None,
        forge: None,
        github: None,
        llm_history: LlmHistory::default(),
        children: Children::default(),
    });
    world.spawn(|| {
        Proc::Llm(Box::new(Llm {
            pending_streak: crate::patience::PendingStreak::default(),
            peer: llm::peer(recipe()).expect("independent LLM peer"),
            scripts: None,
            recipe: None,
            rules: None,
            cue: false,
            release: None,
            released: None,
        }))
    });
    world.spawn_signals(|signals| {
        Proc::Service(Box::new(match issuer {
            Some(address) => ServiceHost::with_issuer(scratch.path().into(), signals, address),
            None => ServiceHost::new(scratch.path().into(), signals, direct_key),
        }))
    });
    host(&mut world, scratch.path(), steps, direct_key, 1);
    if let Some(listener) = reservation {
        use std::os::fd::{AsRawFd, IntoRawFd};
        let address = listener.local_addr().expect("issuer address");
        let fd = skein_io::kernel::Fd::new(listener.as_raw_fd());
        world.spawn_with_fds(vec![fd], || {
            let mut peer = crate::issuer::peer(address, &crate::issuer::registration(address));
            peer.adopt_listener(fd, address).expect("held issuer listener");
            let _ = listener.into_raw_fd();
            Proc::Issuer(Box::new(peer))
        });
    }
    world.run(&skein_shell::Clock::new(), patience)
}
type Rules = fn() -> Box<[Box<[skein_fake_llm_domain::api::Rule]>]>;
type Scripts = fn(std::net::SocketAddr) -> Box<[skein_fake_llm_domain::api::Script]>;
pub(crate) struct ForgePeer {
    pub(crate) peer: skein_fake_peers::forge::Peer,
    pub(crate) pending_streak: crate::patience::PendingStreak,
}
impl ForgePeer {
    fn new(peer: skein_fake_peers::forge::Peer) -> Self {
        Self { peer, pending_streak: crate::patience::PendingStreak::default() }
    }
}
impl std::ops::Deref for ForgePeer {
    type Target = skein_fake_peers::forge::Peer;
    fn deref(&self) -> &Self::Target {
        &self.peer
    }
}
impl std::ops::DerefMut for ForgePeer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.peer
    }
}
pub(crate) struct Llm {
    pub(crate) pending_streak: crate::patience::PendingStreak,
    pub(crate) peer: skein_fake_peers::llm::Peer,
    scripts: Option<Scripts>,
    recipe: Option<crate::ForgeRecipe>,
    rules: Option<Rules>,
    cue: bool,
    pub(crate) release: Option<&'static [u8]>,
    pub(crate) released: Option<&'static [u8]>,
}
impl Llm {
    fn release_hold(&mut self, caller: &Caller) {
        if let Some(name) = caller.releasing_hold() {
            if self.released == Some(name) {
                self.released = None;
                caller.hold_released.set(true);
            } else if !caller.hold_released.get() {
                self.release = Some(name);
            }
        }
    }
}
impl Llm {
    /// Plays the deferred scripts, by their turn rules where given, once the forge is bound.
    fn bind(&mut self, address: std::net::SocketAddr) {
        if let Some(recipe) = self.recipe.take() {
            self.peer = llm::peer((recipe.build)(address, &recipe.source)).expect("derived forge recipe");
            self.scripts = None;
            return;
        }
        let Some(scripts) = self.scripts.take() else { return };
        let recipe = match self.rules.take() {
            Some(rules) => llm::played_by_input(scripts(address), rules()),
            None => llm::scripted(scripts(address)),
        };
        self.peer = llm::peer(recipe).expect("bound forge scripts");
    }
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
    rules: Option<Rules>,
    recipe: Option<crate::ForgeRecipe>,
    steps: impl FnOnce() -> Vec<crate::calls::Step>,
) -> skein_world::real::Outcome<Proc> {
    let patience = crate::PATIENCE;
    let state = std::rc::Rc::new(std::cell::RefCell::new(None));
    let mut world = World::new_controlled(|controls| Judge {
        patience: crate::patience::Patience::new(patience),
        controls,
        workers: Vec::new(),
        engine_stopping: false,
        stopping: false,
        finished: false,
        failure: None,
        armed: false,
        wake: None,
        forge: Some(state.clone()),
        github: None,
        llm_history: LlmHistory::default(),
        children: Children::default(),
    });
    world.spawn(|| {
        Proc::Llm(Box::new(Llm {
            pending_streak: crate::patience::PendingStreak::default(),
            peer: llm::peer(llm::scripted(Box::new([]))).expect("deferred peer"),
            scripts: Some(scripts),
            recipe,
            rules,
            cue: false,
            release: None,
            released: None,
        }))
    });
    world.spawn_signals(|signals| {
        Proc::Service(Box::new(ServiceHost::configured(scratch.path().into(), signals, Some(fixture), false)))
    });
    host(&mut world, scratch.path(), steps, false, 1);
    world.spawn(|| {
        let (forge, peer) = forge::State::new(fixture);
        *state.borrow_mut() = Some(forge);
        Proc::Forge(Box::new(ForgePeer::new(peer)))
    });
    world.host_roots(crate::git::program(), crate::git::roots);
    let outcome = world.run(&skein_shell::Clock::new(), patience);
    crate::git::share(None);
    outcome
}
pub(crate) fn run_github(
    scratch: &skein_scratch::Scratch,
    fixture: crate::github::Fixture,
    scripts: Scripts,
    steps: impl FnOnce() -> Vec<crate::calls::Step>,
) -> skein_world::real::Outcome<Proc> {
    let patience = crate::PATIENCE;
    let (state, mut peer) = crate::github::State::new(fixture);
    let mut world = World::new_controlled(|controls| {
        let mut judge = cluster_judge(controls, &[], None, patience);
        judge.github = Some(state);
        judge
    });
    world.spawn(|| {
        Proc::Llm(Box::new(Llm {
            pending_streak: crate::patience::PendingStreak::default(),
            peer: llm::peer(llm::scripted(Box::new([]))).expect("deferred peer"),
            scripts: Some(scripts),
            recipe: None,
            rules: None,
            cue: false,
            release: None,
            released: None,
        }))
    });
    world.spawn_signals(|signals| {
        let mut service = ServiceHost::github(scratch.path().into(), signals, fixture);
        service.make_github_app(&mut peer);
        Proc::Service(Box::new(service))
    });
    // This story provisions its account explicitly, retaining every ctl output.
    host(&mut world, scratch.path(), steps, true, 1);
    world.spawn(|| Proc::Forge(Box::new(ForgePeer::new(peer))));
    world.host_roots(crate::git::route_program(), crate::git::roots);
    world.run(&skein_shell::Clock::new(), patience)
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
    world.host_roots(
        HostedProgram {
            program: crate::terminal::OPEN.into(),
            make: crate::terminal::host,
            instances: 1,
            operations: crate::terminal::operations(),
        },
        crate::terminal::roots,
    );
    let configuration = smith_agent_shell::config::read(&path.join("agent.json")).expect("agent operation bound");
    world.host_roots(
        HostedProgram {
            program: fixtures::AGENT.into(),
            make: agent::agent,
            instances: agents,
            operations: configuration
                .service
                .limits
                .routes
                .checked_add(3)
                // smith reserves one hosted open per permitted requested root.
                .and_then(|count| count.checked_add(configuration.service.limits.domain.run.directories))
                .expect("hosted agent operations"),
        },
        agent::roots,
    );
}
#[expect(clippy::struct_excessive_bools, reason = "independent referee cues for placement and shutdown")]
struct Judge {
    patience: crate::patience::Patience,
    controls: Controls,
    workers: Vec<&'static str>,
    engine_stopping: bool,
    stopping: bool,
    armed: bool,
    finished: bool,
    failure: Option<Failure>,
    wake: Option<Time>,
    forge: Option<std::rc::Rc<std::cell::RefCell<Option<forge::State>>>>,
    github: Option<crate::github::State>,
    llm_history: LlmHistory,
    children: Children,
}

struct Failure {
    message: String,
    settled: bool,
}

/// Hosted children append during a pass; killed children are removed between passes.
#[derive(Default)]
struct Children {
    seen: usize,
    finished: bool,
    terminals: Vec<usize>,
    calls: Vec<[CallRecords; 2]>,
    differing_calls: [usize; 2],
    lock_change: bool,
    last_setup: bool,
    locks: std::rc::Rc<std::cell::RefCell<crate::calls::recorded::LockChanges>>,
}
/// All retained attempts for one (call, setup), compared only when invalidated.
#[derive(Default)]
struct CallRecords {
    positions: Vec<usize>,
    differs: bool,
}
impl Children {
    fn observe(&mut self, children: &[Proc]) -> bool {
        let removed = children.len() < self.seen;
        if removed {
            self.seen = 0;
            self.terminals.clear();
            self.calls.clear();
            self.differing_calls = [0; 2];
            self.lock_change = false;
            self.locks.borrow_mut().clear();
        }
        for (offset, proc) in children[self.seen..].iter().enumerate() {
            if let Proc::Ctl(recorded) = proc {
                let index = self.seen + offset;
                if self.calls.len() <= recorded.call {
                    self.calls.resize_with(recorded.call + 1, Default::default);
                }
                let records = &mut self.calls[recorded.call][usize::from(recorded.setup)];
                records.positions.push(index);
                self.locks.borrow_mut().invalidate(recorded.call, recorded.setup);
                if !self.finished {
                    recorded.observe_with(&self.locks);
                }
            } else if matches!(proc, Proc::Terminal(_)) {
                self.terminals.push(self.seen + offset);
            }
        }
        self.seen = children.len();
        removed
    }
    fn apply(&mut self, children: &[Proc], caller: &mut Caller) {
        // Observe removals before this pass can append children with reused positions.
        if self.observe(children) {
            // Removal can reveal an older attempt at a reused call number. Restore
            // the retained hosts' admission order, just as the whole scan did.
            for proc in children {
                if let Proc::Ctl(recorded) = proc {
                    recorded.apply(caller);
                }
            }
        }
        self.locks.borrow_mut().apply(caller);
    }
    fn lock_changed(&mut self, children: &[Proc], caller: &Caller) -> bool {
        let mut locks = self.locks.borrow_mut();
        let setup = caller.setup();
        // Without an invalidation, neither the records nor the caller's lock
        // tuples changed; only leaving setup can change the answer.
        if locks.invalidated.is_empty() && self.last_setup == setup {
            return self.lock_change;
        }
        while let Some((call, setup)) = locks.invalidated.pop() {
            let phase = usize::from(setup);
            let records = &mut self.calls[call][phase];
            let differs = records.positions.len() > 1
                && records.positions.iter().any(|index| {
                    let Proc::Ctl(recorded) = &children[*index] else { panic!("ctl position") };
                    recorded.differs(caller)
                });
            if differs != records.differs {
                if differs {
                    self.differing_calls[phase] += 1;
                } else {
                    self.differing_calls[phase] -= 1;
                }
                records.differs = differs;
            }
        }
        // Historical disagreements persist after the queue drains. Setup records
        // cease to count when the caller reuses their positions for the story.
        self.last_setup = setup;
        self.lock_change =
            locks.changed(caller) || self.differing_calls[0] != 0 || setup && self.differing_calls[1] != 0;
        self.lock_change
    }
    fn finish(&mut self, children: &[Proc]) {
        // The referee owns the queue's heap. Hosts must release their subscriptions
        // before the referee drops, so no host frees another owner's allocation.
        for proc in children {
            if let Proc::Ctl(recorded) = proc {
                recorded.stop_observing();
            }
        }
        self.finished = true;
    }
    fn settled(&self, children: &[Proc]) -> bool {
        self.terminals.iter().all(|index| {
            let Proc::Terminal(terminal) = &children[*index] else { panic!("terminal position") };
            terminal.is_empty()
        })
    }
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
    fn finish_peers(&mut self, peer: &mut Llm, children: &mut [Proc]) {
        peer.scripts = None;
        peer.shutdown();
        for proc in children.iter_mut() {
            match proc {
                Proc::Forge(forge) => forge.shutdown(),
                Proc::Issuer(issuer) => issuer.shutdown(),
                Proc::Llm(_)
                | Proc::Service(_)
                | Proc::Caller(_)
                | Proc::Ctl(_)
                | Proc::Terminal(_)
                | Proc::Agent(_)
                | Proc::Git(_)
                | Proc::GitRoute(_) => {}
            }
        }
        self.children.finish(children);
    }
    fn fail_startup(&mut self, message: String, caller: &mut Caller, workers: &[Proc], caller_end: usize) {
        self.failure = Some(Failure { message, settled: false });
        caller.shutdown();
        for (position, worker) in workers.iter().enumerate() {
            if !worker.is_empty() {
                self.controls.signal(caller_end + position, ServiceSignal::Terminate);
            }
        }
    }
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
impl Judge {
    fn describe_pending(&self, now: Time, procs: &mut [Proc]) -> String {
        use std::fmt::Write;
        let mut report = format!(
            "temper judge overdue: stopping={} engine_stopping={} finished={}\n",
            self.stopping, self.engine_stopping, self.finished
        );
        if let Some(failure) = &self.failure {
            writeln!(report, "judge early_failure settled={} message={}", failure.settled, failure.message)
                .expect("string writer");
        }
        for (id, proc) in procs.iter_mut().enumerate() {
            crate::patience::describe(id, proc, now, &mut report);
        }
        if let Some(state) = &self.forge {
            let borrowed = state.borrow();
            if let Some(state) = borrowed.as_ref() {
                writeln!(report, "forge shared_pending={}", state.handle.borrow().pending.len())
                    .expect("string writer");
            }
        }
        if let Some(state) = &self.github {
            writeln!(report, "GitHub shared_pending={}", state.handle.borrow().pending.len()).expect("string writer");
        }
        report
    }
}
impl Referee<Proc> for Judge {
    fn act(&mut self, now: Time, procs: &mut [Proc]) {
        if self.patience.due(now) {
            let report = self.describe_pending(now, procs);
            self.patience.capture(report);
            crate::patience::fail_overdue::<Proc, _>(self, now);
        }
        self.wake = None;
        if self.failure.is_some() {
            return;
        }
        let engine_index = procs.iter().position(|proc| matches!(proc, Proc::Service(_))).expect("engine root");
        let caller_end = 3;
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
        peer.release_hold(caller);
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
        self.children.apply(children, caller);
        if let Some(state) = &self.forge {
            let mut borrowed = state.borrow_mut();
            let state = borrowed.as_mut().expect("spawned forge");
            let Proc::Forge(forge) = &mut children[0] else { panic!("forge admission order") };
            service.forge_address = forge.address();
            for proc in workers.iter_mut() {
                let Proc::Service(worker) = proc else { panic!("worker root") };
                worker.forge_address = forge.address();
            }
            if let Some(address) = forge.address() {
                peer.bind(address);
            }
            if let Some(hooks) = service.hooks
                && !state.hooked
            {
                state.hand_hooks(hooks);
            }
            person_push(now, caller, state, &mut service.seen);
            service.seen.collect(forge, &state.handle);
            caller.advanced.set(
                state.moved
                    && (caller.pushing().is_some()
                        || !service.seen.deliveries().is_empty() && !service.seen.responses().is_empty()),
            );
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
        if !caller.released && service.is_empty() {
            self.fail_startup(service.early_exit(engine_index), caller, workers, caller_end);
            self.finish_peers(peer, children);
            return;
        }
        if let Some(call) = caller.watching {
            service.arm_watch(call);
            caller.watched = service.delivered();
        }
        if caller.is_empty() && self.children.settled(children) && !self.stopping {
            check_ending(caller, service, workers, children);
            for (position, _) in self.workers.iter().enumerate() {
                self.controls.signal(caller_end + position, ServiceSignal::Terminate);
            }
            self.stopping = true;
        }
        if self.stopping && workers.iter().all(Host::is_empty) && !self.engine_stopping {
            self.controls.signal(engine_index, ServiceSignal::Terminate);
            self.engine_stopping = true;
        }
        if service.is_empty() && caller.is_empty() && !self.finished {
            self.finish_peers(peer, children);
            self.finished = true;
        }
    }
    fn observe(&mut self, now: Time, procs: &[Proc]) {
        if let Some(failure) = &mut self.failure {
            failure.settled = procs.iter().all(Host::is_empty);
            return;
        }
        let [Proc::Llm(peer), Proc::Service(service), Proc::Caller(caller), ..] = procs else {
            panic!("root admission order")
        };
        let caller_end = 3;
        let (workers, children) = procs[caller_end..].split_at(self.workers.len());
        let workers_changed = workers.iter().any(|proc| {
            let Proc::Service(worker) = proc else { panic!("worker root") };
            (peer.address().is_some() && worker.address.is_none())
                || (worker_started(worker)
                    && service.workers_address.is_some()
                    && worker.cluster.as_ref().is_some_and(|shape| !shape.ready()))
        });
        let _ = self.children.observe(children);
        if let Some(index) = self.children.terminals.last() {
            let Proc::Terminal(terminal) = &children[*index] else { panic!("terminal position") };
            // Exit lines mutate in place, rather than appending to a history.
            caller.terminal.refused.set(terminal.exit_line().is_some_and(|line| {
                line.as_ref() == b"temper open: the link was refused: use the fresh link temper serve printed\n"
            }));
        }
        caller.held.set(service.held_at(caller.held_index()));
        caller.links.set(service.links());
        let lock_changed = self.children.lock_changed(children, caller);
        let forge_changed = self.forge.as_ref().is_some_and(|state| {
            let borrowed = state.borrow();
            let state = borrowed.as_ref().expect("spawned forge");
            let Proc::Forge(forge) = &children[0] else { panic!("forge root") };
            (forge.address().is_some() && service.forge_address.is_none())
                || (service.hooks.is_some() && !state.hooked)
                || (caller.advancing() && state.hooked && !state.moved)
                || (caller.pushing().is_some() && !state.moved)
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
            || (caller.releasing_hold().is_some() && !caller.hold_released.get() && peer.release.is_none())
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
            || (caller.is_empty() && self.children.settled(children) && !self.stopping)
            || (service.is_empty() && !self.finished && (!caller.released || caller.is_empty()))
        {
            self.wake = Some(now);
        }
    }
    fn next_deadline(&self) -> Option<Time> {
        [self.wake, self.patience.deadline()].into_iter().flatten().min()
    }
    fn overdue(&self, _now: Time) -> Option<String> {
        // Preserve startup failures after io closes; a stalled drain still gets the patience snapshot.
        self.failure
            .as_ref()
            .filter(|failure| failure.settled)
            .map(|failure| failure.message.clone())
            .or_else(|| self.patience.overdue())
    }
    fn passed(&self) -> bool {
        self.finished && self.failure.is_none()
    }
}

fn check_ending(caller: &Caller, service: &ServiceHost, workers: &[Proc], children: &[Proc]) {
    let declared = caller.leftovers();
    let mut obligations = crate::ending::Obligations::default();
    obligations.read(service, declared);
    for worker in workers {
        let Proc::Service(worker) = worker else { panic!("worker root") };
        obligations.read(worker, declared);
    }
    for (index, child) in children.iter().enumerate() {
        obligations.read_child(3 + workers.len() + index, child);
    }
    obligations.check(declared).unwrap_or_else(|error| panic!("{error}"));
}

fn person_push(now: Time, caller: &Caller, state: &mut forge::State, seen: &mut forge::Seen) {
    if caller.advancing() && state.hooked && !state.moved {
        state.advance(now, skein_shell::Clock::new().now().wall);
    }
    if let Some((path, words)) = caller.pushing()
        && !state.moved
    {
        state.push(now, skein_shell::Clock::new().now().wall, path, words);
        seen.pushed(&state.handle);
    }
}

fn github_cues(
    state: &crate::github::State,
    peer: &mut Llm,
    service: &mut ServiceHost,
    caller: &mut Caller,
    children: &mut [Proc],
) {
    let Proc::Forge(forge) = &mut children[0] else { panic!("GitHub root") };
    service.forge_address = forge.address();
    if let Some(address) = forge.address()
        && let Some(scripts) = peer.scripts.take()
    {
        peer.peer = llm::peer(llm::scripted(scripts(address))).expect("bound GitHub scripts");
    }
    caller.github_app = service.github_seen.app.as_ref().map(|app| app.id);
    service.checkpoint_github(forge, &caller.calls);
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
    let patience = crate::PATIENCE;
    let mut world = World::new_controlled_checked(|controls| cluster_judge(controls, workers, None, patience));
    cluster_roots(&mut world, workers, &[], scratch, recipe, None, steps);
    world.run(&skein_shell::Clock::new(), patience)
}

pub(crate) fn run_cluster_forge(
    workers: &[&'static str],
    late: &[&str],
    scratch: &[skein_scratch::Scratch],
    fixture: forge::Fixture,
    scripts: Scripts,
    rules: Rules,
    steps: impl FnOnce() -> Vec<crate::calls::Step>,
) -> skein_world::real::Outcome<Proc> {
    let patience = crate::PATIENCE;
    let state = std::rc::Rc::new(std::cell::RefCell::new(None));
    let mut world = World::new_controlled(|controls| cluster_judge(controls, workers, Some(state.clone()), patience));
    cluster_roots(
        &mut world,
        workers,
        late,
        scratch,
        || llm::scripted(Box::new([])),
        Some((fixture, scripts, rules)),
        steps,
    );
    world.spawn(|| {
        let (forge, peer) = forge::State::new(fixture);
        *state.borrow_mut() = Some(forge);
        Proc::Forge(Box::new(ForgePeer::new(peer)))
    });
    world.run(&skein_shell::Clock::new(), patience)
}

fn cluster_judge(
    controls: Controls,
    workers: &[&'static str],
    forge: Option<std::rc::Rc<std::cell::RefCell<Option<forge::State>>>>,
    patience: skein_lib::Duration,
) -> Judge {
    Judge {
        patience: crate::patience::Patience::new(patience),
        controls,
        workers: workers.to_vec(),
        engine_stopping: false,
        stopping: false,
        finished: false,
        failure: None,
        armed: false,
        wake: None,
        forge,
        github: None,
        llm_history: LlmHistory::default(),
        children: Children::default(),
    }
}

fn cluster_roots<R: Referee<Proc>>(
    world: &mut World<Proc, R>,
    workers: &[&'static str],
    late: &[&str],
    scratch: &[skein_scratch::Scratch],
    recipe: impl FnOnce() -> llm::Recipe,
    forge: Option<(forge::Fixture, Scripts, Rules)>,
    steps: impl FnOnce() -> Vec<crate::calls::Step>,
) {
    let (engine, worker_scratch) = scratch.split_first().expect("engine scratch");
    world.spawn(|| {
        Proc::Llm(Box::new(Llm {
            pending_streak: crate::patience::PendingStreak::default(),
            peer: llm::peer(recipe()).expect("cluster LLM"),
            scripts: forge.map(|(_, scripts, _)| scripts),
            recipe: None,
            rules: forge.map(|(_, _, rules)| rules),
            cue: false,
            release: None,
            released: None,
        }))
    });
    world.spawn_signals(|signals| {
        Proc::Service(Box::new(ServiceHost::cluster(
            engine.path(),
            signals,
            crate::cluster::Shape::Engine(workers.to_vec()),
            forge.map(|(fixture, _, _)| fixture),
        )))
    });
    host(world, &engine.path().join("data"), steps, false, u32::try_from(workers.len()).expect("worker count"));
    for (name, scratch) in workers.iter().zip(worker_scratch) {
        world.spawn_signals(|signals| {
            Proc::Service(Box::new(ServiceHost::cluster(
                scratch.path(),
                signals,
                crate::cluster::Shape::Worker { name, engine: None, started: !late.contains(name) },
                forge.map(|(fixture, _, _)| fixture),
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

    fn caller() -> crate::calls::Caller {
        let mut caller = crate::calls::Caller::new(vec![], std::path::PathBuf::new(), skein_io::kernel::Fd::new(0));
        caller.calls.push(crate::calls::Called {
            locked: None,
            released: None,
            busy: 0,
            exit: None,
            stdout: vec![],
            stderr: vec![],
            spawned: skein_lib::Time::ZERO,
            ended: skein_lib::Time::ZERO,
            observed_at: None,
            tries: 0,
            early: 0,
        });
        caller
    }

    fn ctl(setup: bool) -> super::Proc {
        use skein_io::kernel::Fd;
        use temper_client_shell_cli::host::{Invocation, OneShot, Startup};
        super::Proc::Ctl(Box::new(crate::calls::Recorded::new(
            OneShot::new(
                Invocation {
                    directory: None,
                    parsed: temper_client_shell_cli::arguments::parse(&[b"help"]).expect("help"),
                },
                Startup {
                    parent: None,
                    file_parent: None,
                    stdin: Fd::new(0),
                    stdout: Fd::new(1),
                    stderr: Fd::new(2),
                    signals: Fd::new(3),
                    user: 0,
                    seed: 7,
                },
            )
            .expect("shipped tools limits"),
            0,
            setup,
        )))
    }

    fn lock_event(procs: &mut [super::Proc], index: usize, locked: Option<u64>, released: Option<u64>) {
        let super::Proc::Ctl(recorded) = &mut procs[index] else { panic!("ctl fixture") };
        recorded.locked = locked.map(skein_lib::Time::from_nanos);
        recorded.released = released.map(skein_lib::Time::from_nanos);
        recorded.record_change();
    }

    #[test]
    fn late_lock_completion_of_an_older_attempt_is_observed() {
        let mut caller = caller();
        let mut procs = vec![ctl(false), ctl(false)];
        let mut children = super::Children::default();
        children.apply(&procs, &mut caller);
        assert!(!children.lock_changed(&procs, &caller));

        lock_event(&mut procs, 0, Some(1), None);
        assert!(children.lock_changed(&procs, &caller), "the older attempt completed after its retry");
        children.apply(&procs, &mut caller);
        assert_eq!(caller.calls[0].locked, Some(skein_lib::Time::from_nanos(1)));
        assert!(children.lock_changed(&procs, &caller), "the newer empty attempt still disagrees");
        assert!(children.lock_changed(&procs, &caller), "the disagreement needs no new event");
    }

    #[test]
    fn setup_and_story_reuse_a_call_position_without_sharing_records() {
        let mut setup = crate::calls::Caller::provisioned(
            vec![crate::calls::sign_in(crate::calls::D1)],
            std::path::PathBuf::new(),
            skein_io::kernel::Fd::new(0),
        );
        setup.calls = caller().calls;
        let mut procs = vec![ctl(true), ctl(true)];
        lock_event(&mut procs, 0, Some(1), None);
        let mut children = super::Children::default();
        children.apply(&procs, &mut setup);
        assert!(children.lock_changed(&procs, &setup), "setup's older attempt disagrees");

        // The story's slot now has a fresh host, while the setup hosts remain.
        let mut caller = caller();
        assert!(!children.lock_changed(&procs, &caller), "finished setup's cached disagreement is ignored");
        procs.push(ctl(false));
        children.apply(&procs, &mut caller);
        assert!(!children.lock_changed(&procs, &caller), "a setup attempt is not a retry of the story's call");
        assert_eq!(caller.calls[0].locked, None);
    }

    #[test]
    fn removing_a_killed_attempt_rebuilds_the_lock_comparison() {
        let mut caller = caller();
        let mut procs = vec![ctl(false), ctl(false)];
        lock_event(&mut procs, 1, Some(1), None);
        let mut children = super::Children::default();
        children.apply(&procs, &mut caller);
        assert!(children.lock_changed(&procs, &caller));

        procs.pop().expect("killed retry");
        children.apply(&procs, &mut caller);
        assert_eq!(caller.calls[0].locked, None, "the retained attempt replaces the killed one");
        assert!(!children.lock_changed(&procs, &caller), "the removed attempt cannot keep the judge awake");
    }

    #[test]
    fn removing_all_killed_attempts_clears_the_cached_answer() {
        let mut caller = caller();
        let mut procs = vec![ctl(false), ctl(false)];
        lock_event(&mut procs, 1, Some(1), None);
        let mut children = super::Children::default();
        children.apply(&procs, &mut caller);
        assert!(children.lock_changed(&procs, &caller));

        procs.clear();
        children.apply(&procs, &mut caller);
        assert!(!children.lock_changed(&procs, &caller), "no attempt remains to disagree with the caller");
    }

    #[test]
    fn an_older_attempts_release_can_resolve_a_cached_disagreement() {
        let mut caller = caller();
        let mut procs = vec![ctl(false), ctl(false)];
        lock_event(&mut procs, 0, Some(1), None);
        lock_event(&mut procs, 1, Some(1), Some(2));
        let mut children = super::Children::default();
        children.apply(&procs, &mut caller);
        assert!(children.lock_changed(&procs, &caller), "the older attempt has not released its lock");

        lock_event(&mut procs, 0, Some(1), Some(2));
        assert!(!children.lock_changed(&procs, &caller), "both attempts now match the caller");
        children.apply(&procs, &mut caller);
        assert!(!children.lock_changed(&procs, &caller));
    }

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

#[cfg(test)]
mod teardown;
