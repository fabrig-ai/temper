//! One process, joined through io's workers channel (protocol/README.md, 8).
use crate::{
    AccountsRuntime, End, Stop, Worker, accounts, clients, engine, limits, route::box_link, settings, stop, store,
    translate, values, workers,
};
use alloc::boxed::Box;
use jig_protocol_config::{Path, Problems, What};
use skein_io::{
    self as io, Io,
    kernel::{Complete, Fd, Submit},
};
use skein_lib::{Env, List, Map, Queue, Time, Token, Wall};
pub(crate) const ACCOUNT_APPLICATION: &[u8] = b"temper";

/// The engine root, its protocol children and the kernel queues.
#[expect(missing_debug_implementations, reason = "the service owns account token records")]
pub struct Service {
    pub(crate) engine: Option<engine::Engine>,
    pub(crate) worker: Option<Worker>,
    io: Io,
    io_env: Env<io::Limits>,
    pub(crate) completions: Queue<Complete>,
    pub(crate) submissions: Queue<Submit>,
    raw_submissions: Queue<Submit>,
    kernel: accounts::Kernel,
    pub(crate) told: Queue<io::Event>,
    box_pipe: Option<BoxPipe>,
    signals: Option<Token>,
    signals_closing: bool,
    bytes: u64,
    memory_limit: u64,
    links: Queue<Box<[u8]>>,
}
struct BoxPipe {
    requests: Token,
    answers: Token,
    state: BoxPipeState,
    reading: bool,
    output: BoxOutput,
    read_bytes: u32,
}
#[derive(PartialEq, Eq)]
enum BoxPipeState {
    Open,
    Ended,
    Closing,
}
#[derive(PartialEq, Eq)]
enum BoxOutput {
    Idle,
    Waiting,
    Room,
}
// Existing reservation for setup/forge handoffs beside a root routing pass.
const SETUP_ROUTING_HEADROOM: u32 = 2;
impl Service {
    pub fn configure_program_sources(
        &mut self,
        resources: crate::program_sources::Resources,
    ) -> Result<(), crate::program_sources::Resources> {
        match &mut self.worker {
            Some(worker) => worker.configure_program_sources(resources),
            None => Err(resources),
        }
    }
    /// Construct the root, its deployment store and its configured accounts.
    pub fn engine(
        settings: settings::Engine,
        seeds: &settings::Seeds,
        accounts: Option<AccountsRuntime>,
        forges: List<crate::ForgeRuntime>,
    ) -> Result<Self, Problems> {
        let io = settings.jig.limits.io;
        Self::new(settings, seeds, accounts, forges, None, 0, io)
    }
    /// Join the configured engine and worker in one io, with their checked memory envelopes.
    pub(crate) fn new(
        settings: settings::Engine,
        seeds: &settings::Seeds,
        accounts: Option<AccountsRuntime>,
        forges: List<crate::ForgeRuntime>,
        worker: Option<Worker>,
        worker_bytes: u64,
        io_limits: io::Limits,
    ) -> Result<Self, Problems> {
        let (forges, roots) = crate::forge::prepare(&settings, forges)?;
        let owned = skein_forge_connection::settings::owned_bytes(&forges)
            .ok_or(start_problem(b"memory_bytes", What::Bound))?;
        Self::prepared(settings, seeds, accounts, (forges, roots, owned), worker, worker_bytes, io_limits, false)
    }
    #[expect(clippy::too_many_arguments, reason = "startup supplies runtime facts before the domain is built")]
    #[expect(
        clippy::too_many_lines,
        reason = "one startup constructs the service; explicit matches follow the strict subset"
    )]
    pub(crate) fn prepared(
        mut settings: settings::Engine,
        seeds: &settings::Seeds,
        accounts: Option<AccountsRuntime>,
        forges: (skein_forge_connection::Settings, u64, u64),
        worker: Option<Worker>,
        worker_bytes: u64,
        io_limits: io::Limits,
        under_box: bool,
    ) -> Result<Self, Problems> {
        let limits =
            limits::engine(&settings.jig.limits, &settings.limits).ok_or(start_problem(b"limits", What::Bound))?;
        let (mut forge_settings, forge_roots, forge_owned) = forges;
        let shared = crate::forge::shared(&settings.limits, &settings.jig.limits.io);
        settings.jig.limits.clients.wire.keyed_value_some =
            limits::client_value_bytes(&settings.jig.limits, &settings.limits);
        if settings.jig.defaults == jig_protocol_config::Defaults::Document {
            read_charters(&mut settings)?;
        }
        let store_limits = store::limits(&limits, &limits.forge, &settings.jig.limits.store)
            .ok_or(start_problem(b"limits", What::Bound))?;
        let max = temper_engine_domain::max_out(&limits);
        let queue = max.checked_mul(2).ok_or(start_problem(b"limits", What::Bound))?;
        let worker_operations = match &worker {
            Some(worker) => worker.operations(),
            None => 0,
        };
        let operations =
            startup_operations(&io_limits, queue, worker_operations).ok_or(start_problem(b"limits", What::Bound))?;
        let bytes = startup_worst_case(
            &settings,
            &limits,
            &store_limits,
            accounts.as_ref(),
            worker_operations,
            &io_limits,
            (forge_roots, forge_owned, &forge_settings),
        )?;

        if let Err(what) =
            jig_protocol_config::fits(settings.jig.memory, &[jig_protocol_config::Share { name: b"service", bytes }])
        {
            return Err(start_problem(b"memory_bytes", what));
        }
        let room = max.checked_mul(6).expect("account routing room");
        let accounts = accounts_driver(&settings, seeds, accounts, room, &store_limits.secrets)?;
        let mut values =
            values::Values::new(settings.jig.limits.driver.accounts, values::credential_bytes(&settings.jig));
        let wire = translate::limits(&settings.limits).ok_or(start_problem(b"limits", What::Bound))?;
        values.fill(&settings, &wire)?;
        let (forge_config, identities) = crate::forge::config(&settings, seeds.deployment);
        let mut first_periods = List::with_capacity(limits.authority.projects);
        for project in &settings.jig.projects {
            first_periods
                .push((project.number, project.policy.period_spend))
                .or(Err(start_problem(b"projects", What::Bound)))?;
        }
        let client_room = max
            .checked_add(1)
            .expect("client pass headroom")
            .checked_mul(jig_protocol_engine_clients::MAX_OUT_DOWN)
            .expect("client routing room");
        let mut clients = clients::Clients::new(
            &settings.jig,
            &settings.clients,
            seeds,
            client_room,
            &limits.journal,
            settings.sign_in.as_ref(),
            settings.files.entries.as_slice(),
        )?;
        if let Some(sign_in) = &settings.sign_in {
            clients.configure_google(sign_in, settings.limits.sign_in, seeds, &settings.jig);
        }
        let setup_room = max.checked_add(SETUP_ROUTING_HEADROOM).expect("setup routing headroom");
        let setup = crate::setup::prepare(&settings, &mut forge_settings, &seeds.crypto, setup_room, under_box)?;
        let forge = crate::forge::Layer::new(
            forge_settings,
            settings.limits.forge_component,
            &shared,
            max.checked_add(2).expect("forge routing headroom"),
            &settings.forges,
        )
        .expect("checked forge limits");
        forget_handover(&mut settings);
        let notes = Queue::with_capacity(note_bound(&settings, &limits).expect("validated diagnostic room"));
        let configured = crate::startup::application::configured(&settings);
        // Workers take the forge settings; derive the demo's adoption host first.
        let demo_config = match &settings.demo {
            Some(demo) => Some(demo.domain(&settings.forges, settings.limits.setup.deadline)),
            None => None,
        };
        let workers = workers::Workers::configured(&mut settings, seeds.deployment, max)?;
        let config = temper_engine_domain::Config {
            core: jig_protocol_config::core(&settings.jig, seeds.deployment, seeds.seed),
            landing: settings.landing,
            forge: forge_config,
            identities,
            forge_charter: settings.charter,
            forges: configured,
            under_box,
            demo: demo_config,
        };
        let store = store::Store::new(&limits, &store_limits, &config.core.settings.models, seeds.installation);
        let demo = settings.demo;
        let own = settings.limits;
        let memory_limit =
            settings.jig.memory.checked_add(worker_bytes).ok_or(start_problem(b"memory_bytes", What::Bound))?;
        Ok(Self {
            memory_limit,
            worker,
            engine: Some(engine::Engine::new(
                config,
                &limits,
                store,
                engine::Children { accounts, workers, clients, demo, forge, setup, notes, limits: own },
                values,
                first_periods,
            )?),
            io: Io::new(&io_limits),
            io_env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits: io_limits },
            completions: Queue::with_capacity(queue),
            submissions: Queue::with_capacity(queue),
            raw_submissions: Queue::with_capacity(queue),
            told: Queue::with_capacity(queue),
            kernel: accounts::Kernel::new(operations),
            box_pipe: None,
            signals: None,
            signals_closing: false,
            bytes: bytes.checked_add(worker_bytes).ok_or(start_problem(b"memory_bytes", What::Bound))?,
            links: Queue::with_capacity(queue),
        })
    }
    /// Build the worker and shared IO without allocating any engine children.
    pub(crate) fn worker_alone(
        settings: &settings::Worker,
        limits: &limits::Limits,
        agent: &settings::Agent,
        files: &settings::Files,
        runtime: crate::WorkerRuntime,
        seed: u64,
    ) -> Result<Self, Problems> {
        let io_limits = settings.jig.limits.io;
        let prepared = crate::worker_settings::prepare(settings, agent, limits)?;
        let queue = prepared.room;
        let worker_operations = crate::worker_settings::operations(&limits.git_component)
            .ok_or(crate::worker_settings::problem(b"limits", What::Bound))?;
        let operations = worker_alone_operations(&io_limits, worker_operations)
            .ok_or(crate::worker_settings::problem(b"limits", What::Bound))?;
        let bytes = worker_alone_worst_case(settings, agent, limits)
            .ok_or(crate::worker_settings::problem(b"memory_bytes", What::Bound))?;
        if let Err(what) =
            jig_protocol_config::fits(settings.jig.memory, &[jig_protocol_config::Share { name: b"service", bytes }])
        {
            return Err(crate::worker_settings::problem(b"memory_bytes", what));
        }
        Ok(Self {
            memory_limit: settings.jig.memory,
            engine: None,
            worker: Some(Worker::new(settings, agent, limits, files, runtime, seed)?),
            io: Io::new(&io_limits),
            io_env: Env { now: Time::ZERO, wall: Wall::EPOCH, limits: io_limits },
            completions: Queue::with_capacity(queue),
            submissions: Queue::with_capacity(queue),
            raw_submissions: Queue::with_capacity(operations),
            told: Queue::with_capacity(queue),
            kernel: accounts::Kernel::new(operations),
            box_pipe: None,
            signals: None,
            signals_closing: false,
            bytes,
            links: Queue::with_capacity(queue),
        })
    }
    pub fn take_handover_read(&mut self) -> Option<crate::HandoverRead> {
        let engine = self.engine.as_mut()?;
        match engine.clients.demo_read() {
            Some(read) => Some(read),
            None => crate::setup::lifecycle::take_read(engine),
        }
    }
    pub fn handover_read(
        &mut self,
        forge: u16,
        run: Token,
        answer: Result<Option<skein_lib::Secret>, crate::HandoverUnread>,
    ) {
        if let Some(engine) = &mut self.engine {
            if engine.clients.owns_demo_read(run) {
                engine.clients.demo_read_answer(run, answer);
            } else {
                crate::setup::lifecycle::read(engine, forge, run, answer);
            }
        }
    }
    pub fn take_handover_removal(&mut self) -> Option<crate::HandoverRemoval> {
        crate::setup::lifecycle::take_removal(self.engine.as_mut()?)
    }
    pub fn handover_removed(&mut self, forge: u16, run: Token, result: Result<(), i32>) {
        if let Some(engine) = &mut self.engine {
            crate::setup::lifecycle::removed(engine, forge, run, result);
        }
    }
    /// A manifest page can be reloaded until its own setup clock or capacity ends it.
    #[must_use]
    pub fn setup_page(&self, forge: u16, state: &[u8], now: Time) -> Option<&[u8]> {
        self.engine.as_ref()?.setup.page(forge, state, now)
    }
    /// Adopt the binary's deployment directory.
    pub fn adopt_store(&mut self, root: Fd, effective_user: u32) -> Result<(), Fd> {
        match &mut self.engine {
            Some(engine) => engine.store.adopt(root, effective_user),
            None => Err(root),
        }
    }
    /// Hold the bundle the binary read at start, served on the clients' listener.
    pub fn adopt_bundle(&mut self, bundle: clients::bundle::Bundle) {
        self.engine.as_mut().expect("engine bundle").clients.adopt_bundle(bundle);
    }
    /// Earliest deadline of the root and its children.
    #[must_use]
    pub fn next_policy_deadline(&self) -> Option<Time> {
        accounts::earliest(
            match &self.engine {
                Some(engine) => engine.next_deadline(),
                None => None,
            },
            match &self.worker {
                Some(worker) => worker.next_deadline(),
                None => None,
            },
        )
    }
    /// Whether an iteration has work on the supplied clock.
    #[must_use]
    pub fn work_pending(&self, now: Time) -> bool {
        (match &self.engine {
            Some(engine) => engine.work_pending(now),
            None => false,
        }) || self.worker_runs()
            && match &self.worker {
                Some(worker) => worker.work_pending(now),
                None => false,
            }
            || self.io.is_ready()
            || self.io.is_due(now)
            || !self.completions.is_empty()
            || !self.told.is_empty()
            || !self.raw_submissions.is_empty()
            || self.end().is_some() && self.signals.is_some() && !self.signals_closing
            || self.end().is_some()
                && match &self.box_pipe {
                    Some(pipe) => pipe.state != BoxPipeState::Closing,
                    None => false,
                }
    }
    /// A first call drains; another aborts, keeping a failure's reason.
    pub fn shutdown(&mut self) {
        if let Some(engine) = &mut self.engine {
            stop::shutdown(engine);
            if engine.end.is_some()
                && let Some(worker) = &mut self.worker
            {
                worker.abort();
            }
        } else if let Some(worker) = &mut self.worker {
            if worker.stopping() {
                worker.abort();
                worker.set_end(End::Aborted);
            } else {
                worker.stop();
            }
        }
    }
    /// How this service ended, once it has.
    #[must_use]
    pub fn end(&self) -> Option<&End> {
        match &self.engine {
            Some(engine) => engine.end.as_ref(),
            None => match &self.worker {
                Some(worker) => worker.end(),
                None => None,
            },
        }
    }
    /// Sent forge entries left without their outcome when this stop reached its bound.
    #[must_use]
    pub fn unsettled(&self) -> &[temper_engine_domain::Unsettled] {
        if let Some(engine) = &self.engine
            && let Some(drain) = &engine.drain
        {
            drain.unsettled.as_slice()
        } else {
            &[]
        }
    }
    fn worker_runs(&self) -> bool {
        match &self.engine {
            Some(engine) => {
                engine.end.is_some() || engine.store.opened && engine.workers.opened() && engine.clients.opened()
            }
            None => true,
        }
    }
    /// Adopt a held listener before the first down pass (protocol/setup.md, 7.1).
    /// A Listen for its address claims it; refusal leaves the descriptor with the caller.
    pub fn adopt_listener(&mut self, fd: Fd, addr: core::net::SocketAddr) -> Result<(), Fd> {
        match self.io.adopt_listener(fd, addr) {
            Ok(_) => Ok(()),
            Err(fd) => Err(fd),
        }
    }
    /// Adopt the box's lines and requests (protocol/setup.md, 7.1).
    /// io may keep a bounded read pending until close cancels it.
    /// On refusal the caller retains both descriptors.
    pub fn adopt_box(&mut self, requests: Fd, answers: Fd, line_bytes: u32) -> Result<(), Fd> {
        if self.box_pipe.is_some() || self.io_env.limits.sockets.saturating_sub(self.io.sockets()) < 2 {
            return Err(requests);
        }
        // io's envelope already counts pipe intake, output and operation buffers.
        // The link adds its partial and outgoing lines, including LF.
        // The temporary output queue is counted at startup, even without a pipe.
        let Some(extra) = box_bytes(line_bytes) else { return Err(requests) };
        let Some(bytes) = self.bytes.checked_add(extra) else { return Err(requests) };
        if bytes > self.memory_limit || self.engine.is_none() || line_bytes == 0 {
            return Err(requests);
        }
        self.bytes = bytes;
        self.engine.as_mut().expect("box engine").box_link = box_link::Link::new(line_bytes, true);
        let requests = self.io.adopt_write_pipe(requests, None).expect("two reserved pipe slots");
        let answers = self.io.adopt_read_pipe(answers).expect("second reserved pipe slot");
        self.box_pipe = Some(BoxPipe {
            requests,
            answers,
            state: BoxPipeState::Open,
            reading: false,
            output: BoxOutput::Idle,
            read_bytes: line_bytes.min(self.io_env.limits.intake),
        });
        Ok(())
    }
    /// Adopt the shell's termination-signal source.
    pub fn adopt_signals(&mut self, descriptor: Fd) -> Result<(), Fd> {
        if self.signals.is_some() {
            return Err(descriptor);
        }
        match self.io.adopt_signals_for(descriptor, Token::new(1)) {
            Ok(source) => {
                self.signals = Some(source);
                Ok(())
            }
            Err(descriptor) => Err(descriptor),
        }
    }
    /// Kernel completions, filled by the shell.
    pub const fn completions(&mut self) -> &mut Queue<Complete> {
        &mut self.completions
    }
    /// Kernel submissions, taken by the shell.
    pub const fn submissions(&mut self) -> &mut Queue<Submit> {
        &mut self.submissions
    }
    /// Printed sign-in links, taken by the binary.
    pub const fn links(&mut self) -> &mut Queue<Box<[u8]>> {
        &mut self.links
    }
    /// Bound listeners, taken by the binary before other diagnostics.
    pub fn listening(&mut self) -> &mut Queue<crate::Note> {
        match &mut self.engine {
            Some(engine) => &mut engine.listening,
            None => self.worker.as_mut().expect("worker alone").notes(),
        }
    }
    /// Operator diagnostics, taken by the binary.
    pub fn notes(&mut self) -> &mut Queue<crate::Note> {
        match &mut self.engine {
            Some(engine) => &mut engine.notes,
            None => self.worker.as_mut().expect("worker alone").notes(),
        }
    }
    /// Root, component and I/O deadlines on the injected clock.
    #[must_use]
    pub fn next_deadline(&self) -> Option<Time> {
        accounts::earliest(self.next_policy_deadline(), self.io.next_deadline())
    }
    /// The checked memory envelope for everything this service owns.
    #[must_use]
    pub const fn worst_case(&self) -> u64 {
        self.bytes
    }
    /// Maximum simultaneous kernel operations, for the shell's ring.
    #[must_use]
    pub fn operations(&self) -> u32 {
        if self.engine.is_none() {
            return self.raw_submissions.capacity();
        }
        io::operations(&self.io_env.limits)
            .expect("checked io operations")
            .checked_add(self.raw_submissions.capacity().checked_mul(4).expect("component operations"))
            .expect("engine operations")
            .checked_add(match &self.worker {
                Some(worker) => worker.operations(),
                None => 0,
            })
            .expect("service operations")
    }
    /// Every owned child has closed and every queue and kernel route is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.end().is_some()
            && match &self.worker {
                Some(worker) => worker.is_empty(),
                None => true,
            }
            && match &self.engine {
                Some(engine) => {
                    engine.setup.closed
                        && engine.setup.io.is_empty()
                        && engine.setup.above.is_empty()
                        && engine.forge.closed
                        && engine.forge.io.is_empty()
                        && engine.forge.above.is_empty()
                        && engine.store.closed()
                        && engine.workers.closed()
                        && engine.clients.closed
                        && engine.clients.io.is_empty()
                        && match &engine.accounts {
                            Some(accounts) => accounts.is_empty(),
                            None => true,
                        }
                }
                None => true,
            }
            && self.io.is_empty()
            && self.completions.is_empty()
            && self.submissions.is_empty()
            && self.raw_submissions.is_empty()
            && self.told.is_empty()
            && self.kernel.is_empty()
    }
    fn room(&self, max: io::MaxOut) -> bool {
        self.told.room() >= max.events && self.raw_submissions.room() >= max.submissions
    }
    fn route_components(&mut self) {
        if let Some(engine) = &mut self.engine {
            for _ in 0..engine.store.submitted.len() {
                if self.raw_submissions.room() == 0 {
                    break;
                }
                self.raw_submissions.push(engine.store.submitted.pop().expect("store submissions"));
            }
            if engine.store.opened || engine.end.is_some() {
                for _ in 0..engine.setup.io.len() {
                    if self.raw_submissions.room() < io::MAX_OUT_DOWN.submissions || !self.io.takes() {
                        break;
                    }
                    let request = engine.setup.io.pop().expect("setup IO requests");
                    io::down(&mut self.io, &self.io_env, crate::setup::tag(request), &mut self.raw_submissions);
                }
                for _ in 0..engine.forge.io.len() {
                    if self.raw_submissions.room() < io::MAX_OUT_DOWN.submissions || !self.io.takes() {
                        break;
                    }
                    let request = engine.forge.io.pop().expect("forge io requests");
                    io::down(&mut self.io, &self.io_env, crate::forge::tag(request), &mut self.raw_submissions);
                }
                for _ in 0..engine.clients.io.len() {
                    if self.raw_submissions.room() < io::MAX_OUT_DOWN.submissions || !self.io.takes() {
                        break;
                    }
                    let request = engine.clients.io.pop().expect("client io requests");
                    io::down(&mut self.io, &self.io_env, request, &mut self.raw_submissions);
                }
                for _ in 0..engine.workers.io.len() {
                    if self.raw_submissions.room() < io::MAX_OUT_DOWN.submissions || !self.io.takes() {
                        break;
                    }
                    let request = engine.workers.io.pop().expect("worker io requests");
                    io::down(&mut self.io, &self.io_env, request, &mut self.raw_submissions);
                }
            }
            if (engine.store.opened || engine.end.is_some())
                && let Some(accounts) = &mut engine.accounts
            {
                for _ in 0..accounts.io.len() {
                    if self.raw_submissions.room() < io::MAX_OUT_DOWN.submissions || !self.io.takes() {
                        break;
                    }
                    let request = accounts::tag(accounts.io.pop().expect("account io requests"));
                    io::down(&mut self.io, &self.io_env, request, &mut self.raw_submissions);
                }
            }
        }
        if let Some(worker) = &mut self.worker {
            for _ in 0..worker.submitted().len() {
                if self.raw_submissions.room() == 0 {
                    break;
                }
                let submit = worker.submitted().pop().expect("worker kernel submissions");
                self.raw_submissions.push(submit);
            }
            for _ in 0..worker.io().len() {
                if self.raw_submissions.room() < io::MAX_OUT_DOWN.submissions || !self.io.takes() {
                    break;
                }
                let request = worker.io().pop().expect("worker io requests");
                io::down(&mut self.io, &self.io_env, request, &mut self.raw_submissions);
            }
        }
    }
    /// Format only the roots for a world's secrets check.
    pub fn fmt_roots(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut roots = formatter.debug_list();
        if let Some(engine) = &self.engine {
            roots.entry(&engine.domain);
        }
        if let Some(worker) = &self.worker {
            roots.entry(&worker.root);
        }
        roots.finish()
    }
    fn complete(&mut self, complete: Complete) {
        if store::owns(complete.op) {
            self.engine.as_mut().expect("engine store completion").store.complete(complete);
        } else if crate::tokens::prefix(complete.op) == 3 || crate::tokens::prefix(complete.op) == 6 {
            self.worker
                .as_mut()
                .expect("worker operation prefix")
                .complete(self.io_env.now, complete)
                .expect("worker operation owner");
        } else {
            io::up(&mut self.io, &self.io_env, complete, &mut self.told, &mut self.raw_submissions);
        }
    }
}
fn start_problem(member: &[u8], what: What) -> Problems {
    let mut problems = Problems::empty();
    problems.push(Path::root().member(b"engine").member(member), what);
    problems
}
/// The accounts' driver, when the engine configures an account; its records
/// are kept in the secrets' store, at most `secret_bytes` each.
fn accounts_driver(
    settings: &settings::Engine,
    seeds: &settings::Seeds,
    runtime: Option<AccountsRuntime>,
    room: u32,
    secrets: &skein_kv::Limits,
) -> Result<Option<accounts::Accounts>, Problems> {
    if settings.jig.llm.providers.is_empty() {
        return Ok(None);
    }
    Ok(Some(accounts::Accounts::new(ACCOUNT_APPLICATION.into(), &settings.jig, seeds, runtime, room, secrets)?))
}
fn read_charters(settings: &mut settings::Engine) -> Result<(), Problems> {
    for charter in &mut settings.jig.charters {
        charter.policy.instructions = settings
            .files
            .contents(&charter.policy.instructions)
            .ok_or(start_problem(b"charters", What::Unreadable))?
            .into();
    }
    Ok(())
}
/// Drive store terminals, account events, one root input and released outputs.
pub fn iterate(service: &mut Service, now: Time, wall: Wall) {
    service.io_env.now = now;
    service.io_env.wall = wall;
    if let Some(engine) = &mut service.engine {
        engine::clock(engine, now, wall);
    }
    for _ in 0..service.io_env.limits.sockets.checked_add(service.io_env.limits.refusals).expect("io entries") {
        if !service.io.is_ready() || !service.room(io::MAX_OUT_RESUME) {
            break;
        }
        io::resume(&mut service.io, &service.io_env, &mut service.told, &mut service.raw_submissions);
    }
    if !service.io.is_ready() {
        for _ in 0..service.completions.len() {
            if !service.room(io::MAX_OUT_UP) {
                break;
            }
            let complete = service.completions.pop().expect("completion count");
            let complete = service.kernel.complete(complete, &service.raw_submissions);
            service.complete(complete);
        }
        if service.completions.is_empty() {
            for _ in 0..service.io_env.limits.sockets.checked_mul(2).expect("io timers") {
                if !service.io.is_due(now) || !service.room(io::MAX_OUT_FIRE) {
                    break;
                }
                io::fire(&mut service.io, &service.io_env, &mut service.told, &mut service.raw_submissions);
            }
        }
    }
    route_events(service);
    if let Some(engine) = &mut service.engine {
        engine::iterate(engine, now, wall);
    }
    let runs = service.worker_runs();
    if let Some(worker) = &mut service.worker {
        if let Some(engine) = &service.engine
            && (engine.end.is_some()
                || match &engine.drain {
                    Some(drain) => drain.closing,
                    None => false,
                })
        {
            worker.abort();
        }
        if runs && worker.room() {
            worker.iterate(now, wall);
        }
        if let Some(engine) = &mut service.engine {
            if let Some(problems) = worker.refused() {
                if engine.drain.is_none() && engine.end.is_none() {
                    stop::shutdown(engine);
                }
                if let Some(End::Drained) = engine.end {
                    engine.end = Some(End::Stopped(Stop::Start(problems.clone())));
                }
            }
        } else {
            if worker.end().is_none()
                && let Some(problems) = worker.refused()
            {
                worker.set_end(End::Stopped(Stop::Start(problems.clone())));
                worker.abort();
            }
            if worker.is_empty() && worker.end().is_none() {
                worker.set_end(End::Drained);
            }
        }
    }
    close_sources(service);
    if let Some(engine) = &mut service.engine {
        engine.abort_if_ended();
    }
    box_io(service);
    service.route_components();
    if let Some(engine) = &mut service.engine
        && engine.clients.starting.is_none()
    {
        for _ in 0..engine.clients.links.len() {
            if service.links.room() == 0 {
                break;
            }
            let link = engine.clients.links.pop().expect("printed sign-in links");
            service.links.push(link);
        }
    }
    service.kernel.publish(&mut service.raw_submissions, &mut service.submissions);
    service.io.reclaim();
}
fn close_sources(service: &mut Service) {
    if service.end().is_some()
        && !service.signals_closing
        && service.raw_submissions.room() >= io::MAX_OUT_DOWN.submissions
        && let Some(entity) = service.signals
    {
        service.signals_closing = true;
        io::down(&mut service.io, &service.io_env, io::Request::Close { entity }, &mut service.raw_submissions);
    }
    if service.end().is_some()
        && service.raw_submissions.room() >= io::MAX_OUT_DOWN.submissions.saturating_mul(2)
        && let Some(pipe) = &mut service.box_pipe
        && pipe.state != BoxPipeState::Closing
    {
        pipe.state = BoxPipeState::Closing;
        for entity in [pipe.requests, pipe.answers] {
            io::down(&mut service.io, &service.io_env, io::Request::Close { entity }, &mut service.raw_submissions);
        }
    }
}
fn route_events(service: &mut Service) {
    for _ in 0..service.told.len() {
        if let Some(engine) = &service.engine
            && (!engine.workers.room()
                || !engine.clients.room(1)
                || engine.events.room() < engine::route_out()
                || !engine.setup.room()
                || !engine.forge.room(1)
                || match &engine.accounts {
                    Some(accounts) => !accounts.up_room(),
                    None => false,
                })
        {
            break;
        }
        if let Some(worker) = &service.worker
            && !worker.room()
        {
            break;
        }
        if box_notes_full(service) {
            break;
        }
        let event = service.told.pop().expect("io event count");
        if let io::Event::Shutdown { .. } = event {
            service.shutdown();
            continue;
        }
        let owner = match &event {
            io::Event::Listening { owner, .. }
            | io::Event::Accepted { owner, .. }
            | io::Event::Connecting { owner, .. }
            | io::Event::Connected { owner }
            | io::Event::Stream { owner, .. }
            | io::Event::Output { owner, .. }
            | io::Event::Spawned { owner, .. }
            | io::Event::Exited { owner, .. }
            | io::Event::Usage { owner, .. }
            | io::Event::Window { owner, .. }
            | io::Event::Failed { owner, .. }
            | io::Event::Closed { owner } => Some(*owner),
            io::Event::Resized | io::Event::Shutdown { .. } => None,
        };
        if owner == Some(Token::new(1)) {
            if let io::Event::Closed { .. } = event {
                service.signals = None;
            }
            continue;
        }
        if let Some(pipe) = &service.box_pipe
            && (owner == Some(pipe.requests) || owner == Some(pipe.answers))
        {
            box_event(service, event);
            continue;
        }
        if let Some(worker) = &mut service.worker
            && worker.accepts(&event)
        {
            worker.up(service.io_env.now, event);
        } else if let Some(engine) = &mut service.engine {
            if let Some(owner) = owner
                && crate::tokens::prefix(owner) == 11
            {
                engine.setup.up(event);
            } else if let Some(owner) = owner
                && crate::tokens::prefix(owner) == 5
            {
                engine.forge.up(event);
            } else if let Some(owner) = owner
                && clients::owns(owner)
            {
                if !engine.clients.up(event) {
                    engine.end = Some(End::Stopped(Stop::Listener));
                }
            } else if let Some(owner) = owner
                && accounts::owns(owner)
            {
                if let Some(accounts) = &mut engine.accounts {
                    accounts.up(accounts::untag(event));
                }
            } else if !engine.workers.up(service.io_env.now, event, &mut engine.events) {
                engine.end = Some(End::Stopped(Stop::Listener));
            }
            for (listener, addr) in [
                (crate::Listening::Clients, engine.clients.listening.take()),
                (crate::Listening::Workers, engine.workers.listening.take()),
            ] {
                if let Some(addr) = addr {
                    engine
                        .listening
                        .try_push(crate::Note::Listening { listener, addr })
                        .expect("one note per listener bind");
                }
            }
        }
    }
}

// A conservative payload envelope uses the root's admitted footprint per held
// event/page: each payload is a bounded part of that root (programming-model.md, 6).
pub(crate) fn startup_worst_case(
    settings: &settings::Engine,
    root: &temper_engine_domain::Limits,
    store: &jig_protocol_store::Limits,
    runtime: Option<&AccountsRuntime>,
    worker_operations: u32,
    io: &io::Limits,
    forge_bytes: (u64, u64, &skein_forge_connection::Settings),
) -> Result<u64, Problems> {
    crate::startup::memory_sum(&startup_memory_terms(
        settings,
        root,
        store,
        runtime,
        worker_operations,
        io,
        forge_bytes,
    )?)
    .ok_or(start_problem(b"memory_bytes", What::Bound))
}
pub(crate) fn startup_memory_terms(
    settings: &settings::Engine,
    root: &temper_engine_domain::Limits,
    store: &jig_protocol_store::Limits,
    runtime: Option<&AccountsRuntime>,
    worker_operations: u32,
    io: &io::Limits,
    forge_bytes: (u64, u64, &skein_forge_connection::Settings),
) -> Result<[crate::startup::MemoryTerm; 20], Problems> {
    let queue = temper_engine_domain::max_out(root).checked_mul(2).ok_or(start_problem(b"limits", What::Bound))?;
    let operations = startup_operations(io, queue, worker_operations).ok_or(start_problem(b"limits", What::Bound))?;
    memory(settings, root, store, runtime, operations, io, forge_bytes)
        .ok_or(start_problem(b"memory_bytes", What::Bound))
}

fn checked_setup_bytes(settings: &settings::Engine) -> Option<u64> {
    let mut bytes = u64::try_from(settings.records_only.len()).ok()?.checked_mul(4)?;
    if !settings.setup.is_empty() {
        // The shell retains the resolved repositories pair alongside Checked.
        bytes = bytes.checked_add(
            u64::try_from(size_of::<(Box<[u8]>, u64)>().checked_add(b"forge.client.repositories".len())?).ok()?,
        )?;
        bytes = bytes.checked_add(u64::try_from(size_of::<Option<temper_forge_setup::check::Checked>>()).ok()?)?;
    }
    for setup in &settings.setup {
        bytes = bytes
            .checked_add(setup.entry.heap_bytes()?.checked_mul(2)?)?
            .checked_add(u64::try_from(size_of::<settings::Setup>()).ok()?.checked_mul(2)?)?;
    }
    Some(bytes)
}
/// Kernel work in a worker-only process, independent of its domain's room.
pub(crate) fn worker_alone_operations(io: &io::Limits, worker_operations: u32) -> Option<u32> {
    // Allow staged successors beside io's in-flight operations. Every io entry
    // point emits at most MAX_OUT_DOWN.submissions; the worker adds git's
    // operations and closes counted by worker_settings.
    io::operations(io)?.checked_mul(io::MAX_OUT_DOWN.submissions)?.checked_add(worker_operations)
}
fn startup_operations(io: &io::Limits, queue: u32, worker_operations: u32) -> Option<u32> {
    io::operations(io)?.checked_add(queue.checked_mul(4)?)?.checked_add(worker_operations)
}
fn handover_diagnostics(settings: &settings::Engine, queue: u32, events: u32) -> Option<u64> {
    // Client reasons fit 256 bytes; only operator Notes carry the full path.
    // Supplement the root's fixed reason allowance in retained connections
    // and event/request handoffs, and price the full Note separately.
    let mut file_diagnostic = 0_u64;
    for setup in &settings.setup {
        if settings.records_only.contains(&setup.entry.forge) {
            file_diagnostic = file_diagnostic.max(u64::try_from(setup.entry.directory.len()).ok()?.checked_add(108)?);
        }
    }
    let file_copies = u64::from(events)
        .checked_mul(2)?
        .checked_add(u64::from(queue))?
        .checked_add(u64::try_from(settings.forges.len()).ok()?.checked_mul(2)?)?
        .checked_add(4)?;
    let reason_growth =
        if file_diagnostic == 0 { 0 } else { 256_u64.checked_sub(temper_engine_domain_forge::STOPPED_TEXT_BYTES)? };
    reason_growth
        .checked_mul(file_copies)?
        .checked_add(file_diagnostic.checked_mul(u64::try_from(settings.forges.len()).ok()?.checked_add(1)?)?)
}
#[expect(clippy::too_many_lines, reason = "one exhaustive sum of the composed service envelopes")]
fn memory(
    settings: &settings::Engine,
    root: &temper_engine_domain::Limits,
    store: &jig_protocol_store::Limits,
    runtime: Option<&AccountsRuntime>,
    operations: u32,
    io: &io::Limits,
    forge: (u64, u64, &skein_forge_connection::Settings),
) -> Option<[crate::startup::MemoryTerm; 20]> {
    let queue = temper_engine_domain::max_out(root).checked_mul(2)?;
    let retained = temper_engine_domain::worst_case(root)?;
    let payload = temper_engine_domain::payload_worst_case(root)?;
    let events = queue.checked_mul(engine::route_out())?;
    let connections = temper_engine_domain_forge::connection_worst_case(
        settings.limits.forge_component.forges,
        settings.limits.forge_component.installations,
        settings.limits.forge.name_bytes.max(temper_protocol_worker_wire::v1::CEILINGS.identity_login),
    )?;
    let system = handover_diagnostics(settings, queue, events)?
        .checked_add(u64::try_from(size_of::<Service>()).ok()?)?
        .checked_add(List::<(u32, u64)>::worst_case(root.authority.projects)?)?
        .checked_add(List::<temper_engine_domain::Unsettled>::worst_case(root.forge.entries)?)?
        .checked_add(Queue::<temper_engine_domain::Event>::worst_case(events)?)?
        .checked_add(Queue::<temper_engine_domain::Request>::worst_case(temper_engine_domain::max_out(root))?)?
        .checked_add(payload.checked_mul(u64::from(events).checked_mul(2)?.checked_add(1)?)?)?
        .checked_add(Queue::<Complete>::worst_case(queue)?)?
        .checked_add(Queue::<Submit>::worst_case(queue)?.checked_mul(2)?)?
        .checked_add(Queue::<io::Event>::worst_case(queue)?)?
        .checked_add(Queue::<Box<[u8]>>::worst_case(queue)?)?
        .checked_add(Queue::<crate::Note>::worst_case(note_bound(settings, root)?)?)?
        .checked_add(u64::from(temper_engine_domain_forge::demo_act_bound(&root.forge)?).checked_mul(256)?)?
        .checked_add(Queue::<crate::Note>::worst_case(3)?)?
        .checked_add(Queue::<box_link::Out>::worst_case(3)?)?
        .checked_add(u64::try_from(settings.forges.len()).ok()?.checked_mul(crate::setup::diagnostics::worst_case()?)?)?
        .checked_add(u64::from(queue).checked_mul(u64::from(io.output).checked_add(512)?)?)?
        .checked_add(Map::<Token, (Token, Option<Token>)>::worst_case(operations)?)?
        .checked_add(Map::<Token, Token>::worst_case(operations)?)?;
    let workers = workers::worst_case(
        settings,
        temper_engine_domain::max_out(root).checked_mul(jig_protocol_engine_workers::MAX_OUT_DOWN)?,
    )?;
    let clients = clients::worst_case(
        &settings.jig.limits.clients,
        &limits::server(&settings.jig.limits),
        &settings.jig.limits.clients_http,
        temper_engine_domain::max_out(root).checked_add(1)?.checked_mul(jig_protocol_engine_clients::MAX_OUT_DOWN)?,
        &root.journal,
    )?;
    let mut public_bytes = 0_u64;
    if let Some(public) = &settings.clients.public {
        // The clients retain the origin; listening gives the mount its Host copy.
        public_bytes = u64::try_from(public.name.len())
            .ok()?
            .checked_mul(3)?
            .checked_add(u64::try_from(public.host.len()).ok()?.checked_mul(2)?)?;
    }
    let bundle = clients::bundle::worst_case(&settings.limits.clients)?;
    let mut google = 0;
    if settings.sign_in.is_some() {
        google =
            clients::google::worst_case(settings.limits.sign_in, limits::server(&settings.jig.limits).connections)?;
    }
    let mut demo_bytes = 0_u64;
    if let Some(demo) = &settings.demo {
        let mut retained = List::<Box<[u8]>>::worst_case(u32::try_from(demo.cues.len()).ok()?)?
            .checked_add(u64::try_from(demo.owner.len()).ok()?)?
            .checked_add(u64::try_from(demo.name.len()).ok()?)?
            .checked_add(u64::try_from(demo.landing.len()).ok()?)?
            .checked_add(u64::try_from(demo.key_file.len()).ok()?)?;
        for cue in &demo.cues {
            retained = retained.checked_add(u64::try_from(cue.len()).ok()?)?;
        }
        demo_bytes = u64::try_from(demo.key_file.len()).ok()?.checked_mul(u64::from(clients::DEMO_STARTS))?;
        // Configuration, plus one transient typed answer and its encoded record.
        demo_bytes = demo_bytes.checked_add(retained.checked_mul(2)?)?.checked_add(
            temper_client_protocol_wire::v1::DemoAnswer::worst_case_bytes(&temper_client_protocol_wire::v1::CEILINGS)?,
        )?;
    }
    let forges = skein_forge_connection::limits::Forges::of(forge.2)?;
    let batch = temper_engine_domain::max_out(root).checked_add(SETUP_ROUTING_HEADROOM)?;
    let forge_connections = skein_forge_connection::limits::worst_case(
        &settings.limits.forge_component,
        &crate::forge::shared(&settings.limits, &settings.jig.limits.io),
        forges,
        forge.1,
    )?
    .checked_add(Queue::<skein_forge_connection::Event>::worst_case(
        batch.checked_mul(skein_forge_connection::MAX_OUT.above)?,
    )?)?
    .checked_add(
        u64::from(batch.checked_mul(skein_forge_connection::MAX_OUT.above)?)
            .checked_mul(u64::from(settings.limits.forge.client.answer_bytes))?,
    )?
    .checked_add(Queue::<io::Request>::worst_case(batch.checked_mul(skein_forge_connection::MAX_OUT.below)?)?)?;
    let identities = u32::try_from(settings.identities.len()).ok()?;
    let wire = translate::limits(&settings.limits)?;
    let identities =
        values::Identities::worst_case(identities)?.checked_add(List::<u32>::worst_case(identities)?)?.checked_add(
            u64::from(identities)
                .checked_mul(2)?
                .checked_mul(temper_protocol_worker_wire::v1::Identity::worst_case_bytes(&wire)?)?,
        )?;
    let tokens = crate::forge::tokens::worst_case(settings)?;
    let mut accounts = 0;
    if !settings.jig.llm.providers.is_empty() {
        let driver = settings.jig.limits.driver;
        accounts = accounts::worst_case(&driver, queue.checked_mul(3)?, store.secrets.page.rows)?;
    }
    let configured = accounts::configured_bytes(&settings.jig)?;
    let roots = match runtime {
        Some(runtime) => skein_tls::roots_worst_case(runtime.trust_roots, runtime.trust_root_bytes)?,
        None => 0,
    };
    Some([
        ("engine", retained),
        ("system", system),
        ("forge.held", connections.checked_add(crate::forge::held_worst_case(settings)?)?),
        ("io", skein_io::worst_case(io)?),
        ("store", store::worst_case(root, store)?),
        (
            "values",
            jig_protocol_accounts::table::worst_case(
                settings.jig.limits.driver.accounts,
                values::credential_bytes(&settings.jig),
            )?,
        ),
        ("workers", workers),
        ("clients", clients),
        ("clients.public", public_bytes),
        ("clients.bundle", bundle),
        ("clients.google", google),
        ("clients.demo", demo_bytes),
        ("forge", forge_connections),
        ("identities", identities),
        ("forge.tokens", tokens),
        ("accounts", accounts.checked_add(configured)?.checked_add(roots)?),
        ("forge.records", crate::forge::records::worst_case(settings)?),
        ("setup.checked", checked_setup_bytes(settings)?),
        ("forge.settings", forge.0),
        ("setup", crate::setup::worst_case(settings, forge.2, batch)?),
    ])
}

/// The worker's envelope plus the process IO, routing and kernel queues.
pub(crate) fn worker_alone_worst_case(
    worker: &settings::Worker,
    agent: &settings::Agent,
    limits: &limits::Limits,
) -> Option<u64> {
    crate::worker_worst_case(worker, agent, limits)?
        .checked_add(crate::startup::memory_sum(&worker_process_memory_terms(worker, agent, limits)?)?)
}
/// Process queues and IO added only when the worker owns the process.
pub(crate) fn worker_process_memory_terms(
    worker: &settings::Worker,
    agent: &settings::Agent,
    limits: &limits::Limits,
) -> Option<[crate::startup::MemoryTerm; 2]> {
    let queue = crate::worker_settings::prepare(worker, agent, limits).ok()?.room;
    let operations =
        worker_alone_operations(&worker.jig.limits.io, crate::worker_settings::operations(&limits.git_component)?)?;
    let system = u64::try_from(size_of::<Service>())
        .ok()?
        .checked_add(Queue::<Complete>::worst_case(queue)?)?
        .checked_add(Queue::<Submit>::worst_case(queue)?)?
        .checked_add(Queue::<Submit>::worst_case(operations)?)?
        .checked_add(Queue::<io::Event>::worst_case(queue)?)?
        .checked_add(Queue::<Box<[u8]>>::worst_case(queue)?)?
        .checked_add(u64::from(queue).checked_mul(u64::from(worker.jig.limits.io.output).checked_add(512)?)?)?
        .checked_add(Map::<Token, (Token, Option<Token>)>::worst_case(operations)?)?
        .checked_add(Map::<Token, Token>::worst_case(operations)?)?;
    Some([("system", system), ("system.io", io::worst_case(&worker.jig.limits.io)?)])
}

fn forget_handover(settings: &mut settings::Engine) {
    for configured in &mut settings.forges {
        configured.api_token = None;
    }
    for setup in &mut settings.setup {
        setup.handover = None;
    }
}

fn box_notes_full(service: &Service) -> bool {
    let Some(engine) = &service.engine else { return false };
    let Some(pipe) = &service.box_pipe else { return false };
    pipe.state == BoxPipeState::Open
        && engine.notes.room() == 0
        && match service.told.iter().next() {
            Some(event) => box_owner(event, pipe),
            None => false,
        }
}
fn box_owner(event: &io::Event, pipe: &BoxPipe) -> bool {
    match event {
        io::Event::Stream { owner, .. } | io::Event::Failed { owner, .. } | io::Event::Closed { owner } => {
            *owner == pipe.requests || *owner == pipe.answers
        }
        io::Event::Listening { .. }
        | io::Event::Accepted { .. }
        | io::Event::Connecting { .. }
        | io::Event::Connected { .. }
        | io::Event::Output { .. }
        | io::Event::Spawned { .. }
        | io::Event::Exited { .. }
        | io::Event::Usage { .. }
        | io::Event::Window { .. }
        | io::Event::Resized
        | io::Event::Shutdown { .. } => false,
    }
}
fn box_event(service: &mut Service, event: io::Event) {
    use skein_lib::stream::Up;
    let pipe = service.box_pipe.as_mut().expect("box event");
    if pipe.state != BoxPipeState::Open {
        return;
    }
    let engine = service.engine.as_mut().expect("box engine");
    let mut out = Queue::with_capacity(3);
    match event {
        io::Event::Stream { owner, up } => match up {
            Up::Bytes(bytes) => {
                assert!(owner == pipe.answers, "box read owner");
                pipe.reading = false;
                engine.box_link.read(&bytes, &mut out);
            }
            Up::Room => {
                assert!(owner == pipe.requests, "box write owner");
                pipe.output = BoxOutput::Room;
            }
            Up::End | Up::Failed(_) => {
                pipe.state = BoxPipeState::Ended;
                engine.box_link.closed(&mut out);
            }
        },
        io::Event::Failed { .. } | io::Event::Closed { .. } => {
            pipe.state = BoxPipeState::Ended;
            engine.box_link.closed(&mut out);
        }
        io::Event::Listening { .. }
        | io::Event::Accepted { .. }
        | io::Event::Connecting { .. }
        | io::Event::Connected { .. }
        | io::Event::Output { .. }
        | io::Event::Spawned { .. }
        | io::Event::Exited { .. }
        | io::Event::Usage { .. }
        | io::Event::Window { .. }
        | io::Event::Resized
        | io::Event::Shutdown { .. } => unreachable!("box pipe event"),
    }
    box_link::drain(engine, &mut out);
}
fn box_io(service: &mut Service) {
    use skein_lib::stream::{Delimiter, Down, Read};
    let Some(pipe) = &mut service.box_pipe else { return };
    if pipe.state != BoxPipeState::Open {
        return;
    }
    if !pipe.reading && service.raw_submissions.room() >= io::MAX_OUT_DOWN.submissions && service.io.takes() {
        io::down(
            &mut service.io,
            &service.io_env,
            io::Request::Stream {
                stream: pipe.answers,
                down: Down::Demand { read: Read::Scan { until: Delimiter::LF, max: pipe.read_bytes }, room: 0 },
            },
            &mut service.raw_submissions,
        );
        pipe.reading = true;
    }
    let engine = service.engine.as_mut().expect("box engine");
    if engine.box_link.write.is_some()
        && service.raw_submissions.room() >= io::MAX_OUT_DOWN.submissions
        && service.io.takes()
    {
        if pipe.output == BoxOutput::Room {
            let line = engine.box_link.write.take().expect("box line ready");
            io::down(
                &mut service.io,
                &service.io_env,
                io::Request::Stream { stream: pipe.requests, down: Down::Send(line) },
                &mut service.raw_submissions,
            );
            pipe.output = BoxOutput::Idle;
        } else if pipe.output == BoxOutput::Idle {
            // Full output credit waits for the preceding line to leave io.
            io::down(
                &mut service.io,
                &service.io_env,
                io::Request::Stream {
                    stream: pipe.requests,
                    down: Down::Demand { read: Read::Nothing, room: service.io_env.limits.output },
                },
                &mut service.raw_submissions,
            );
            pipe.output = BoxOutput::Waiting;
        }
    }
}

fn box_bytes(line_bytes: u32) -> Option<u64> {
    u64::from(line_bytes).checked_add(1)?.checked_mul(2)
}

impl Service {
    /// Start-time endpoints only; the browser and the issuer never choose a socket.
    pub fn configure_google_endpoints(
        &mut self,
        runtime: clients::fetch::Runtime,
    ) -> Result<(), clients::fetch::TooLarge> {
        let extra = runtime.worst_case().ok_or(clients::fetch::TooLarge)?;
        let bytes = self.bytes.checked_add(extra).ok_or(clients::fetch::TooLarge)?;
        if bytes > self.memory_limit {
            return Err(clients::fetch::TooLarge);
        }
        if let Some(engine) = &mut self.engine {
            engine.clients.google_endpoints(runtime.endpoints);
            self.bytes = bytes;
        }
        Ok(())
    }
}

// One forge cohort and the end's bounded act answers can be named together.
fn note_bound(settings: &settings::Engine, root: &temper_engine_domain::Limits) -> Option<u32> {
    u32::try_from(settings.forges.len()).ok()?.checked_add(temper_engine_domain_forge::demo_act_bound(&root.forge)?)
}
