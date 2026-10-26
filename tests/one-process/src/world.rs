//! One real startup, native person, hosted agent and git, and loopback byte peers.
use crate::{
    fixtures,
    machine::{self, Machine, SignalHost},
    referee::{self, Capture, Store, Wire},
    stories,
};
use jig_client_protocol_link::Signing;
use skein_fake_peers::forge::{Delivery, Observation, Peer};
use skein_fake_person::{Doing, Face, Find, Found, Next, Step, TreeFace};
use skein_io::kernel::{Complete, Exit, Fd, Submit};
use skein_lib::{Duration, Queue, Time, Wall};
use skein_shell::Host;
use skein_world::{Inherited, Memory};
use std::{cell::RefCell, net::SocketAddr, rc::Rc};
use temper_client_shell_terminal::client::{NativeClient, Settings as ClientSettings, Start};
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub seed: u64,
    pub config: skein_sim::Config,
}
#[derive(Clone, Copy, Debug)]
pub enum Story {
    Chat { words: &'static [u8] },
    ChatReadThenWait { words: &'static [u8] },
    Adoption,
    Adopt,
    Merge,
}
#[derive(Debug)]
pub struct Outcome {
    pub verdict: skein_world::domain::Verdict,
    pub signed_in: u32,
    pub settings_revision: u32,
    pub chats_answered: u32,
    pub adopted: u32,
    pub pushed: Vec<Pushed>,
    pub deliveries: Vec<Delivery>,
    pub drained: bool,
    pub restarts: u32,
    pub claims_settled_at_restart: u32,
    pub adoptions_answered: u32,
    pub adoption_pending_at_restart: bool,
}
#[derive(Clone, Copy)]
pub(crate) enum Phase {
    Story,
    SignIn,
    Drain,
    Restart,
    Open,
    Fault { cut: skein_world::Cut, before: u32 },
}
#[derive(Default)]
struct Progress {
    setup_party: Option<u64>,
    at: usize,
    life: u32,
    signed: bool,
    sent: Vec<(u32, [u8; 16])>,
    answers: Vec<(u32, [u8; 16])>,
    press_had_frame: Vec<(u32, bool)>,
    cache: referee::client::cache::History,
}
#[derive(Debug)]
pub struct Pushed {
    pub branch: Box<[u8]>,
    pub commit: u64,
    pub parents: Vec<u64>,
}
thread_local! {
    static STARTUP: RefCell<Option<crate::deployment::Startup>> = const { RefCell::new(None) };
    static STORE: RefCell<Option<Rc<RefCell<Store>>>> = const { RefCell::new(None) };
    static PROGRESS: RefCell<Progress> = RefCell::new(Progress::default());
}
pub(crate) fn setup_signed(party: u64) {
    PROGRESS.with(|progress| progress.borrow_mut().setup_party = Some(party));
}
pub(crate) struct ServiceHost {
    pub(crate) service: temper_service::Service,
    pub(crate) hooks: Option<SocketAddr>,
    bound_hooks: Option<(Fd, SocketAddr)>,
    capture: Capture,
    store: Rc<RefCell<Store>>,
    pub(crate) commits: Vec<crate::referee::durable::Commit>,
    pub(crate) wire: Wire,
    pub(crate) attached: bool,
    pub(crate) cut_before: Option<u32>,
    pub(crate) submitted: u32,
    pub(crate) before_iteration: u32,
    codec: jig_protocol_store::codec::Limits,
    names: Box<[jig_domain_core::settings::ModelName]>,
    families: temper_protocol_store::Bounds,
}
impl Host for ServiceHost {
    fn iterate(&mut self, now: Time, wall: Wall) {
        self.before_iteration = self.submitted;
        for complete in self.service.completions().iter() {
            if let Some(bound) = hooks_address(&mut self.bound_hooks, complete) {
                self.hooks = Some(bound);
            }
            self.capture.complete(complete);
            self.wire.complete(complete);
        }
        self.commits.extend(self.store.borrow_mut().observe(
            &mut self.capture.events,
            &self.codec,
            &self.names,
            &self.families,
        ));
        temper_service::iterate(&mut self.service, now, wall);
        for submit in self.service.submissions().iter() {
            self.capture.pending(&submit.kind);
        }
        self.submitted += self.service.submissions().len();
        if self.cut_before.is_some_and(|before| self.before_iteration <= before && before < self.submitted) {
            referee::Referee::check_roots(self);
            self.cut_before = None;
        }
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        self.service.completions()
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        self.service.submissions()
    }
    fn work_pending(&self, now: Time) -> bool {
        self.service.work_pending(now)
    }
    fn next_deadline(&self) -> Option<Time> {
        self.service.next_deadline()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        self.service.next_policy_deadline()
    }
    fn is_empty(&self) -> bool {
        self.service.is_empty()
    }
    fn worst_case(&self) -> u64 {
        self.service.worst_case() + 8 * 1024 * 1024
    }
    fn operations(&self) -> u32 {
        self.service.operations()
    }
}
pub(crate) enum Proc {
    Service(Box<ServiceHost>),
    Ctl(Box<crate::ctl::Recorded>),
    Agent(Box<skein_world::PipeHost<smith_agent_shell::Agent>>),
    Git(Box<temper_fake_git_hosted::Git<temper_fake_git_hosted::Typed>>),
    Shell(Box<stories::Shell>),
    Peer(Box<SignalHost<skein_fake_peers::llm::Peer>>),
    Forge(Box<SignalHost<Peer>>),
    Client(Box<SignalHost<Person>>),
}
impl Proc {
    fn host(&self) -> &dyn Host {
        match self {
            Self::Service(h) => h.as_ref(),
            Self::Ctl(h) => h.as_ref(),
            Self::Agent(h) => h.as_ref(),
            Self::Git(h) => h.as_ref(),
            Self::Shell(h) => h.as_ref(),
            Self::Peer(h) => h.as_ref(),
            Self::Forge(h) => h.as_ref(),
            Self::Client(h) => h.as_ref(),
        }
    }
    fn host_mut(&mut self) -> &mut dyn Host {
        match self {
            Self::Service(h) => h.as_mut(),
            Self::Ctl(h) => h.as_mut(),
            Self::Agent(h) => h.as_mut(),
            Self::Git(h) => h.as_mut(),
            Self::Shell(h) => h.as_mut(),
            Self::Peer(h) => h.as_mut(),
            Self::Forge(h) => h.as_mut(),
            Self::Client(h) => h.as_mut(),
        }
    }
}
impl Host for Proc {
    fn drain(&mut self) {
        self.host_mut().drain();
    }
    fn iterate(&mut self, now: Time, wall: Wall) {
        self.host_mut().iterate(now, wall);
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        self.host_mut().completions()
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        self.host_mut().submissions()
    }
    fn work_pending(&self, now: Time) -> bool {
        self.host().work_pending(now)
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
pub(crate) struct Person {
    pub(crate) wire: referee::client::Wire,
    cache: referee::client::cache::Cache,
    client: Option<NativeClient>,
    root: Option<Fd>,
    actor: skein_fake_person::Person<temper_client_domain::Address>,
    provision: Option<skein_fake_person::Person<temper_client_domain::Address>>,
    wake: Option<Time>,
    pub(crate) signed_in: u32,
    pub(crate) finished: bool,
    project_selected: bool,
    setup: Option<crate::ctl::Caller>,
    printed_link: Box<[u8]>,
    completions: Queue<Complete>,
    submissions: Queue<Submit>,
    offset: usize,
    pub(crate) life: u32,
    cutting: bool,
}
const CHAT_WITHIN: Duration = Duration::from_secs(5);
impl Person {
    fn new(root: Fd, scenario: Story) -> Self {
        let steps = match scenario {
            Story::Chat { words } | Story::ChatReadThenWait { words } => vec![
                Step::Go {
                    address: temper_client_domain::Address::Chats {
                        filter: temper_client_domain::ChatsFilter::All,
                        start: false,
                    },
                },
                Step::See { find: Find::named(skein_ui::Role::TextArea, b"Start a chat"), within: CHAT_WITHIN },
                Step::Type { find: Find::named(skein_ui::Role::TextArea, b"Start a chat"), words: words.into() },
                Step::Press { find: Find::named(skein_ui::Role::Button, b"Start") },
                Step::See { find: Find::text(fixtures::ANSWER), within: CHAT_WITHIN },
            ],
            Story::Adoption => stories::adoption_person(),
            Story::Adopt | Story::Merge => stories::person(),
        };
        let mut offset = PROGRESS.with(|progress| progress.borrow().at);
        if matches!(scenario, Story::Adoption) && offset < fixtures::ADOPTION_REPLY_STEP {
            offset = 0;
        }
        let mut remaining: Vec<_> = steps.into_iter().skip(offset).collect();
        if matches!(scenario, Story::Adoption) && offset >= fixtures::ADOPTION_REPLY_STEP {
            remaining.clear();
        }
        let life = PROGRESS.with(|progress| {
            progress.borrow_mut().life += 1;
            progress.borrow().life
        });
        Self {
            wire: referee::client::Wire::default(),
            cache: referee::client::cache::Cache::new(
                root,
                life,
                PROGRESS.with(|progress| progress.borrow().cache.clone()),
            ),
            client: None,
            root: Some(root),
            actor: skein_fake_person::Person::new(remaining.into_boxed_slice()),
            provision: (offset == 0).then(|| skein_fake_person::Person::new(fixtures::account_steps())),
            wake: None,
            signed_in: 0,
            finished: false,
            project_selected: false,
            setup: None,
            printed_link: Box::new([]),
            completions: Queue::with_capacity(512),
            submissions: Queue::with_capacity(512),
            offset,
            life,
            cutting: matches!(scenario, Story::Adoption),
        }
    }
    pub(crate) fn sign_in(&mut self, link: &[u8]) {
        assert!(self.client.is_none(), "one printed link");
        self.printed_link = link.into();
        if stories::needs_settings() {
            self.setup = Some(crate::ctl::Caller::new(self.root.expect("client parent"), link));
            return;
        }
        self.client = Some(
            NativeClient::new(
                ClientSettings {
                    engine: skein_io::Addr::Ip(fixtures::CLIENTS),
                    directory: b"private".as_slice().into(),
                    limits: stories::client_limits(),
                },
                self.root.take().expect("private root"),
                1000,
                fixtures::SEED,
                Start::Link(link.into()),
            )
            .expect("service's printed link"),
        );
    }
    pub(crate) fn finish(&mut self) {
        self.finished = true;
        if let Some(client) = &mut self.client {
            client.close();
        }
    }
}
// Select the story's project and provision its real account before its own acts.
/// The banner's button for a project `me` leaves unnamed, "Project N"
/// (client/README.md, 5.1).
fn an_unnamed_project(name: Option<&[u8]>) -> bool {
    let Some(number) = name.and_then(|name| name.strip_prefix(b"Project ".as_slice())) else { return false };
    !number.is_empty() && number.iter().all(u8::is_ascii_digit)
}
fn prepare_account(
    client: &mut NativeClient,
    selected: &mut bool,
    provision: &mut Option<skein_fake_person::Person<temper_client_domain::Address>>,
    wake: &mut Option<Time>,
    now: Time,
) -> bool {
    if !*selected
        && let Some(node) = client
            .tree()
            .nodes()
            .iter()
            .find(|node| node.role == skein_ui::Role::Button && an_unnamed_project(node.name.as_deref()))
            .map(|node| (node.id, node.states.iter().any(|state| *state == skein_ui::State::Pressed)))
    {
        let (node, pressed) = node;
        if !pressed {
            assert!(client.act(node, skein_ui::Act::Press), "offered project selection");
        }
        *selected = true;
    }
    let Some(actor) = provision else { return false };
    match actor.next(&mut TreeFace::new(client.tree()), now) {
        Next::Do(Doing::Act { node, act }) => assert!(client.act(node, act), "offered account setup act"),
        Next::Do(Doing::Go { address }) => client.go(address),
        Next::Do(Doing::Reload) => panic!("account setup has no reload"),
        Next::Wait { until } => *wake = until,
        Next::Done => {
            *provision = None;
            *wake = Some(now);
        }
        Next::Failed { step, why } => panic!("account setup step {step}: {why:?}"),
    }
    true
}
impl Person {
    fn prepare_client(&mut self, now: Time, wall: Wall) {
        while let Some(complete) = self.completions.pop() {
            if crate::ctl::Caller::owns(complete.op) {
                self.setup.as_mut().expect("ctl setup").completions().push(complete);
                continue;
            }
            let client = self.client.as_mut().expect("native completion");
            self.cache.complete(&complete);
            let received = self.wire.received.len();
            self.wire.complete(&complete);
            for event in &self.wire.received[received..] {
                if let referee::client::Received::ApplicationSent { key } = event {
                    PROGRESS.with(|progress| progress.borrow_mut().sent.push((self.life, *key)));
                } else if let referee::client::Received::Application { key, .. } = event {
                    PROGRESS.with(|progress| progress.borrow_mut().answers.push((self.life, *key)));
                }
            }
            client.completions().push(complete);
        }
        if let Some(setup) = &mut self.setup {
            setup.iterate(now, wall);
            while let Some(submit) = setup.submissions().pop() {
                self.submissions.push(submit);
            }
            if setup.is_empty() {
                self.setup = None;
                let party = PROGRESS.with(|progress| progress.borrow().setup_party.expect("observed CLI sign-in"));
                self.wire.received.push(referee::client::Received::SignedIn(party));
                self.signed_in = 1;
                PROGRESS.with(|progress| progress.borrow_mut().signed = true);
                self.client = Some(
                    NativeClient::new(
                        ClientSettings {
                            engine: skein_io::Addr::Ip(fixtures::CLIENTS),
                            directory: b"private".as_slice().into(),
                            limits: stories::client_limits(),
                        },
                        self.root.take().expect("private parent"),
                        1000,
                        fixtures::SEED,
                        Start::Token,
                    )
                    .expect("saved sign-in"),
                );
            }
        }
    }
}
impl Host for Person {
    fn iterate(&mut self, now: Time, wall: Wall) {
        self.prepare_client(now, wall);
        let Some(client) = &mut self.client else { return };
        client.iterate(now, wall);
        while let Some(signing) = client.signing() {
            if matches!(signing, Signing::Again) {
                client.sign_in(&self.printed_link).expect("retry the printed link while startup restores settings");
                continue;
            }
            assert!(matches!(signing, Signing::SignedIn { .. }), "sign-in failed: {signing:?}");
            self.signed_in += 1;
            PROGRESS.with(|progress| progress.borrow_mut().signed = true);
        }
        while let Some(notice) = client.notices() {
            if self.cutting
                && matches!(
                    notice,
                    temper_client_shell_terminal::client::Shown::Notice(
                        temper_client_shell_terminal::route::Notice::Fact(
                            jig_client_protocol_link::Fact::StreamDropped
                        )
                    )
                )
            {
                continue;
            }
            assert!(
                !matches!(
                    notice,
                    temper_client_shell_terminal::client::Shown::Notice(
                        temper_client_shell_terminal::route::Notice::Fact(_)
                    )
                ),
                "client protocol fact: {notice:?}"
            );
        }
        self.wake = None;
        let preparing = client.started()
            && !self.finished
            && prepare_account(client, &mut self.project_selected, &mut self.provision, &mut self.wake, now);
        if client.started() && self.project_selected && !self.finished && !preparing {
            let before = self.actor.at();
            let mut face = TreeFace::new(client.tree());
            let next = if self.cutting
                && self.life > 1
                && !self
                    .wire
                    .received
                    .iter()
                    .any(|event| matches!(event, referee::client::Received::Application { .. }))
            {
                Next::Wait { until: None }
            } else if self.cutting && self.offset + before as usize + 1 == fixtures::ADOPTION_REPLY_STEP {
                match face.find(&Find::exactly_named(skein_ui::Role::Button, b"Adopt")) {
                    Found::Node(node) if face.allows(node, &skein_ui::Act::Press) => self.actor.next(&mut face, now),
                    Found::Node(_) | Found::Absent | Found::Pending => Next::Wait { until: None },
                }
            } else {
                self.actor.next(&mut face, now)
            };
            match next {
                Next::Do(Doing::Act { node, act }) => {
                    if self.cutting && self.offset + before as usize + 1 == fixtures::ADOPTION_REPLY_STEP {
                        let frame = self.wire.received.iter().any(|event| {
                            matches!(
                                event,
                                referee::client::Received::Delivery(jig_client::contract::Delivery::Me(_))
                                    | referee::client::Received::Read(jig_client::contract::ReadValue::Me(_))
                            )
                        });
                        PROGRESS.with(|progress| progress.borrow_mut().press_had_frame.push((self.life, frame)));
                    }
                    assert!(client.act(node, act), "offered person act");
                }
                Next::Do(Doing::Go { address }) => client.go(address),
                Next::Do(Doing::Reload) => panic!("chat script has no reload"),
                Next::Wait { until } => self.wake = until,
                Next::Done => {
                    self.finished = true;
                    client.close();
                }
                Next::Failed { step, why } => person_failed(step, &why, &self.wire, self.life),
            }
            if self.actor.at() != before {
                PROGRESS.with(|progress| {
                    progress.borrow_mut().at = self.offset + usize::try_from(self.actor.at()).expect("person step");
                });
                self.wake = Some(now);
            }
        }
        while let Some(submit) = client.submissions().pop() {
            self.submissions.push(submit);
        }
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        &mut self.completions
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        &mut self.submissions
    }
    fn work_pending(&self, now: Time) -> bool {
        !self.completions.is_empty()
            || self.wake.is_some_and(|at| at <= now)
            || self.client.as_ref().is_some_and(|client| client.work_pending(now))
            || self.setup.as_ref().is_some_and(|setup| setup.work_pending(now))
    }
    fn next_deadline(&self) -> Option<Time> {
        [self.client.as_ref().and_then(Host::next_deadline), self.wake].into_iter().flatten().min()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        [self.client.as_ref().and_then(Host::next_policy_deadline), self.wake].into_iter().flatten().min()
    }
    fn is_empty(&self) -> bool {
        self.finished && self.client.as_ref().is_some_and(Host::is_empty)
    }
    fn worst_case(&self) -> u64 {
        self.client.as_ref().map_or(64 * 1024 * 1024, Host::worst_case)
    }
    fn operations(&self) -> u32 {
        512
    }
}
fn person_failed(step: u32, why: &skein_fake_person::Why, wire: &referee::client::Wire, life: u32) -> ! {
    let sent: Vec<_> = wire
        .received
        .iter()
        .filter_map(|event| match event {
            referee::client::Received::ApplicationSent { key } => Some(*key),
            referee::client::Received::SignedIn(_)
            | referee::client::Received::Answer { .. }
            | referee::client::Received::Application { .. }
            | referee::client::Received::Delivery(_)
            | referee::client::Received::Read(_) => None,
        })
        .collect();
    let answered: Vec<_> = wire
        .received
        .iter()
        .filter_map(|event| match event {
            referee::client::Received::Application { key, .. } => Some(*key),
            referee::client::Received::SignedIn(_)
            | referee::client::Received::Answer { .. }
            | referee::client::Received::ApplicationSent { .. }
            | referee::client::Received::Delivery(_)
            | referee::client::Received::Read(_) => None,
        })
        .collect();
    let durable = STORE.with(|store| store.borrow().as_ref().expect("world store").borrow().rows.values().filter(|row| matches!(row, temper_engine_domain::Record::Forge { row } if matches!(row.as_ref(), temper_engine_domain_forge::Stored::Application { .. }))).count());
    PROGRESS.with(|progress| {
        let progress = progress.borrow();
        let before: Vec<_> = progress.sent.iter().filter(|(sent_life, _)| *sent_life < life).collect();
        panic!("person step {step}: {why:?}; adoption keys sent {sent:?}, answered {answered:?}, durable outcomes {durable}; pre-cut sent {before:?}; Adopt presses had frame {:?}; private cache {:?}", progress.press_had_frame, progress.cache.borrow());
    })
}
fn service(inherited: &Inherited) -> Proc {
    let root = inherited.roots.iter().find(|(name, _)| name.as_ref() == b"state").expect("state root").1;
    let worker = inherited.roots.iter().find(|(name, _)| name.as_ref() == b"worker").expect("worker root").1;
    let mut service = STARTUP.with(|slot| {
        let slot = slot.borrow();
        let startup = slot.as_ref().expect("phase startup inputs");
        temper_service::startup::startup_with(
            &startup.configuration,
            // Simulated fixture paths stay relative to their inherited roots.
            &jig_protocol_config::Directories::working(),
            &startup.files(),
            &fixtures::agent(),
            &fixtures::seeds(),
            None,
            temper_service::WorkerRuntime { root: worker, path: b"/worker".as_slice().into() },
            fixtures::forges(),
            &stories::given(&serde_json::from_slice(&startup.configuration).expect("deployment JSON")),
        )
        .expect("one-process startup")
    });
    service.adopt_store(root, 1000).expect("deployment store");
    service.adopt_signals(inherited.signal).expect("service signal");
    let mut capture = Capture::default();
    capture.root(root);
    let (codec, names) = STARTUP.with(|slot| {
        let slot = slot.borrow();
        let bytes = &slot.as_ref().expect("phase inputs").configuration;
        (fixtures::codec_for(bytes), fixtures::names_for(bytes))
    });
    let families =
        temper_protocol_store::bounds(&temper_service::limits::profile(temper_service::limits::Profile::Small).forge)
            .expect("application codec bounds");
    Proc::Service(Box::new(ServiceHost {
        service,
        hooks: None,
        bound_hooks: None,
        capture,
        store: STORE.with(|store| store.borrow().as_ref().expect("world store").clone()),
        commits: Vec::new(),
        wire: Wire::new(STORE.with(|store| store.borrow().as_ref().expect("world store").clone())),
        attached: false,
        cut_before: None,
        submitted: 0,
        before_iteration: 0,
        codec,
        names,
        families,
    }))
}
#[expect(
    clippy::zero_sized_map_values,
    reason = "smith's endpoint program selections use the sole adapter name as the map's value"
)]
fn agent(_spawn: &skein_io::kernel::Spawn, inherited: &Inherited) -> Proc {
    let stderr = inherited.pipes.iter().find(|(child, _)| *child == 2).expect("stderr").1;
    let (errors, capture) = smith_agent_process_world::AgentErrors::pipe(stderr);
    Proc::Agent(Box::new(
        capture.host(
            smith_agent_shell::Agent::new(
                smith_agent_shell::config::Configuration {
                    service: fixtures::agent_configuration(),
                    memory: fixtures::agent_profile().declared.memory,
                    trace: None,
                    profile: fixtures::agent_profile(),
                    declarations: fixtures::agent_declarations(),
                    programs: std::collections::BTreeMap::new(),
                    endpoint_programs: std::collections::BTreeMap::new(),
                },
                smith_agent_shell::Resources {
                    input: inherited.pipes.iter().find(|(child, _)| *child == 0).expect("stdin").1,
                    output: inherited.pipes.iter().find(|(child, _)| *child == 1).expect("stdout").1,
                    error: Some(stderr),
                    signals: inherited.signal,
                    seed: fixtures::SEED,
                    pid: 101,
                    trace: None,
                    roots: Some(inherited.roots.iter().map(|(_, fd)| *fd).collect()),
                    programs: None,
                },
                Box::new(errors),
            )
            .expect("hosted agent"),
        ),
    ))
}
fn git(spawn: &skein_io::kernel::Spawn, inherited: &Inherited) -> Proc {
    Proc::Git(Box::new(temper_fake_git_hosted::git(spawn, inherited, transport())))
}
fn shell(spawn: &skein_io::kernel::Spawn, inherited: &Inherited) -> Proc {
    Proc::Shell(Box::new(stories::Shell::new(spawn, inherited)))
}
fn worker_root(_spawn: &skein_io::kernel::Spawn) -> Vec<skein_world::StartupRoot> {
    vec![skein_world::StartupRoot { name: b"worker".as_slice().into(), path: b"worker".as_slice().into() }]
}
fn agent_root(_spawn: &skein_io::kernel::Spawn) -> Vec<skein_world::StartupRoot> {
    vec![skein_world::StartupRoot { name: b"worker".as_slice().into(), path: b"agent".as_slice().into() }]
}
#[must_use]
pub fn run(settings: Settings, scenario: Story) -> Outcome {
    run_phase(
        settings,
        scenario,
        Phase::Story,
        machine::fresh(),
        Store::new(),
        Rc::new(RefCell::new(referee::Run::default())),
    )
    .0
}
#[must_use]
pub fn drain_mid_run(settings: Settings, restart: bool) -> Outcome {
    let run = Rc::new(RefCell::new(referee::Run::default()));
    let (mut outcome, disk, store) = run_phase(
        settings,
        Story::ChatReadThenWait { words: b"Hello" },
        Phase::Drain,
        machine::fresh(),
        Store::new(),
        run.clone(),
    );
    if restart {
        let (next, _, _) =
            run_phase(settings, Story::ChatReadThenWait { words: b"Hello" }, Phase::Restart, disk, store, run);
        assert_eq!(outcome.settings_revision, 2, "one hosted ctl settings change before the signal");
        assert_eq!(next.settings_revision, outcome.settings_revision, "the changed settings survive restart");
        outcome.restarts = next.restarts;
        outcome.claims_settled_at_restart = next.claims_settled_at_restart;
        outcome.drained &= next.drained;
    }
    outcome
}
#[must_use]
pub fn crash(settings: Settings, cut: skein_world::Cut, draw: &mut skein_lib::Rng) -> (u32, Outcome) {
    // The pinned world arms cuts before running. Observe the deterministic
    // pending window on fresh disk, then replay that scenario with its cut.
    let setup = Rc::new(RefCell::new(referee::Run::default()));
    run_phase(settings, Story::Adoption, Phase::Story, machine::fresh(), Store::new(), setup.clone());
    let (start, end) = setup.borrow().window.expect("saved pending adoption window");
    let length = end.checked_sub(start).expect("pending window ordering");
    assert!(length > 0, "pending window has a submission");
    let before = start + u32::try_from(draw.below(u64::from(length))).expect("bounded pending draw");
    let outcome = run_phase(
        settings,
        Story::Adoption,
        Phase::Fault { cut, before },
        machine::fresh(),
        Store::new(),
        Rc::new(RefCell::new(referee::Run::default())),
    )
    .0;
    (before, outcome)
}
pub(crate) fn saved_adoptions() -> std::collections::BTreeSet<[u8; 16]> {
    PROGRESS.with(|progress| referee::client::cache::saved(&progress.borrow().cache))
}
pub fn require_pending_at_cut(outcome: &Outcome, seed: u64, before: u32) {
    if !outcome.adoption_pending_at_restart {
        PROGRESS.with(|progress| {
            let progress = progress.borrow();
            panic!("seed {seed}, before {before}: pending adoption at cut; original sent {:?}; answers {:?}; frames {:?}; private cache {:?}", progress.sent, progress.answers, progress.press_had_frame, progress.cache.borrow());
        });
    }
}
fn person(inherited: &Inherited) -> Proc {
    let root = inherited.roots.first().expect("client root").1;
    let signed = PROGRESS.with(|progress| progress.borrow().signed);
    let mut person = Person::new(root, Story::Adoption);
    if signed {
        person.client = Some(
            NativeClient::new(
                ClientSettings {
                    engine: skein_io::Addr::Ip(fixtures::CLIENTS),
                    directory: b"private".as_slice().into(),
                    limits: stories::client_limits(),
                },
                person.root.take().expect("private root"),
                1000,
                fixtures::SEED,
                Start::Token,
            )
            .expect("stored sign-in"),
        );
        person.signed_in = 1;
    }
    Proc::Client(Box::new(SignalHost::new(person, Some(inherited.signal))))
}
fn llm_peer(inherited: &Inherited) -> Proc {
    Proc::Peer(Box::new(SignalHost::new(fixtures::provider(), Some(inherited.signal))))
}

fn hooks_address(bound: &mut Option<(Fd, SocketAddr)>, complete: &Complete) -> Option<SocketAddr> {
    use skein_io::kernel::{Done, Op};
    match (&complete.kind, &complete.result) {
        (Op::Bind { fd, addr: asked }, Ok(Done::Bound(addr))) if asked.port() == 0 => {
            *bound = Some((*fd, *addr));
            None
        }
        (Op::Listen { fd, .. }, Ok(_)) if bound.is_some_and(|(at, _)| at == *fd) => bound.take().map(|(_, addr)| addr),
        _ => None,
    }
}
thread_local! { static SHARED: RefCell<Option<temper_fake_git_hosted::Typed>> = const { RefCell::new(None) }; }
fn share(transport: Option<temper_fake_git_hosted::Typed>) {
    SHARED.with(|shared| *shared.borrow_mut() = transport);
}
fn transport() -> temper_fake_git_hosted::Typed {
    SHARED.with(|shared| shared.borrow().as_ref().expect("world's forge").clone())
}
fn peer() -> Peer {
    Peer::new(
        fixtures::FORGE,
        fixtures::peer_limits(),
        temper_fake_git_hosted::fixtures::table(b"acme/app", None),
        fixtures::forge_limits(),
        transport().forge,
        fixtures::SEED,
    )
    .expect("forge peer")
}

fn fake_forge(inherited: &Inherited) -> Proc {
    Proc::Forge(Box::new(SignalHost::new(peer(), Some(inherited.signal))))
}
fn host_programs(world: &mut skein_world::World<Proc, referee::Referee, Machine>) {
    let limits = temper_client_shell_cli::limits::native();
    world.host_roots(
        skein_world::HostedProgram {
            program: crate::ctl::PROGRAM.into(),
            make: |spawn, inherited| Proc::Ctl(Box::new(crate::ctl::host(spawn, inherited))),
            instances: 1,
            operations: skein_io::operations(&limits.io).expect("ctl operations") * 2 + 5,
        },
        crate::ctl::roots,
    );
    world.host_roots(
        skein_world::HostedProgram {
            program: b"/opt/temper/bin/temper".as_slice().into(),
            make: agent,
            instances: fixtures::AGENTS,
            operations: fixtures::agent_limits().routes + 3,
        },
        agent_root,
    );
    world.host_roots(
        skein_world::HostedProgram {
            program: b"/usr/bin/git".as_slice().into(),
            make: git,
            instances: 1,
            operations: 1,
        },
        worker_root,
    );
    world.host_roots(
        skein_world::HostedProgram { program: b"/bin/sh".as_slice().into(), make: shell, instances: 1, operations: 1 },
        agent_root,
    );
}
fn deliveries(procs: &[Proc]) -> Vec<Delivery> {
    procs
        .iter()
        .filter_map(|proc| if let Proc::Forge(peer) = proc { Some(peer.observations()) } else { None })
        .flatten()
        .filter_map(|observation| {
            if let Observation::Delivery { outcome, .. } = observation {
                Some(match outcome {
                    Delivery::Written => Delivery::Written,
                    Delivery::NoAddress => Delivery::NoAddress,
                    Delivery::NoRoom => Delivery::NoRoom,
                    Delivery::Refused => Delivery::Refused,
                    Delivery::Reset => Delivery::Reset,
                })
            } else {
                None
            }
        })
        .collect()
}

/// Two native starts on one disk, checking the held sign-in's stored identity and credential.
#[must_use]
pub fn sign_in_again(settings: Settings) -> [Outcome; 2] {
    let scenario = Story::Chat { words: b"Hello" };
    let run = Rc::new(RefCell::new(referee::Run::default()));
    let (first, mut disk, store) =
        run_phase(settings, scenario, Phase::SignIn, machine::fresh(), Store::new(), run.clone());
    let before = sign_ins(&store);
    assert_eq!(before.len(), 1, "one live sign-in before the fresh link");
    let token = token_bytes(&mut disk);
    assert_eq!(token.len(), 40, "the native bearer is saved on disk");
    let (again, mut disk, store) = run_phase(settings, scenario, Phase::SignIn, disk, store, run);
    assert_eq!(token_bytes(&mut disk), token, "same directory keeps every token byte");
    assert_eq!(sign_ins(&store), before, "same sign-in number and expiry, pool unchanged");
    [first, again]
}

#[expect(clippy::wildcard_enum_match_arm, reason = "the snapshot selects only durable sign-in rows")]
fn sign_ins(store: &Store) -> Vec<(u64, u64, Wall)> {
    store
        .rows
        .values()
        .filter_map(|row| match row {
            temper_engine_domain::Record::Core(jig_domain_core::Record::People(
                jig_domain_people::Stored::SignIn { number, person, expires },
            )) => Some((*number, *person, *expires)),
            _ => None,
        })
        .collect()
}

fn token_bytes(disk: &mut skein_fake_machine::Machine) -> Vec<u8> {
    let root = disk.reopen_root(2);
    let file = disk.open(root, b"private/token", skein_fake_machine::How::ReadNoFollow).expect("native token file");
    let size = u32::try_from(disk.stat(file).size).expect("bounded token");
    let bytes = disk.read(file, 0, size).expect("read native token");
    disk.close(file);
    disk.close(root);
    bytes
}

fn write_settings(disk: &mut skein_fake_machine::Machine) {
    if stories::needs_settings() {
        let root = disk.lay(&[skein_fake_machine::Item::file(b"settings.json", &stories::settings_file())]);
        disk.close(root);
    }
}

fn settings_revision(records: &Store) -> u32 {
    records
        .rows
        .values()
        .filter_map(|row| {
            if let temper_engine_domain::Record::Core(jig_domain_core::Record::Core(
                jig_domain_core::CoreRecord::Settings(jig_domain_core::settings::Record::Revision {
                    history: false,
                    revision,
                }),
            )) = row
            {
                Some(revision.head.number)
            } else {
                None
            }
        })
        .max()
        .unwrap_or(0)
}

fn run_phase(
    settings: Settings,
    scenario: Story,
    phase: Phase,
    disk: skein_fake_machine::Machine,
    previous: Store,
    run: Rc<RefCell<referee::Run>>,
) -> (Outcome, skein_fake_machine::Machine, Store) {
    let (outcome, disk, store, _) = run_phase_with_startup(settings, scenario, phase, disk, previous, run, None);
    (outcome, disk, store)
}
fn run_phase_with_startup(
    settings: Settings,
    scenario: Story,
    phase: Phase,
    mut disk: skein_fake_machine::Machine,
    previous: Store,
    run: Rc<RefCell<referee::Run>>,
    startup: Option<crate::deployment::Startup>,
) -> (Outcome, skein_fake_machine::Machine, Store, crate::deployment::Startup) {
    stories::set(scenario);
    stories::setup_settings(phase);
    let startup = startup.unwrap_or_else(crate::deployment::Startup::fixture);
    STARTUP.with(|slot| *slot.borrow_mut() = Some(startup));
    if !matches!(phase, Phase::Open) {
        write_settings(&mut disk);
    }
    PROGRESS.with(|progress| *progress.borrow_mut() = Progress::default());
    let client = matches!(phase, Phase::Story | Phase::SignIn | Phase::Drain).then(|| disk.reopen_root(2));
    let store = Rc::new(RefCell::new(previous));
    STORE.with(|cell| {
        *cell.borrow_mut() = Some(store.clone());
    });
    let transport = stories::forge(scenario);
    let observed_forge = Rc::new(RefCell::new(referee::forge::Forge::new(transport.clone(), scenario)));
    share(Some(transport.clone()));
    let mut world = skein_world::World::new_controlled(settings.seed, settings.config, Memory::Unchecked, |controls| {
        referee::Referee::new(controls, store.clone(), scenario, observed_forge.clone(), run.clone(), phase)
    })
    .with_machine(Machine(disk));
    host_programs(&mut world);
    world.spawn_restartable(
        vec![
            skein_world::StartupRoot { name: b"state".as_slice().into(), path: b"state".as_slice().into() },
            skein_world::StartupRoot { name: b"worker".as_slice().into(), path: b"worker".as_slice().into() },
        ],
        service,
    );
    match phase {
        Phase::Fault { cut, before } => {
            world.spawn_restartable(
                vec![skein_world::StartupRoot {
                    name: b"private".as_slice().into(),
                    path: b"client".as_slice().into(),
                }],
                person,
            );
            world.spawn_restartable(vec![], llm_peer);
            world.spawn_restartable(vec![], fake_forge);
            world.cut(0, before, cut);
        }
        Phase::Restart | Phase::Open => {
            world.spawn(|| Proc::Peer(Box::new(SignalHost::new(stories::provider(scenario), None))));
            world.spawn(|| Proc::Forge(Box::new(SignalHost::new(peer(), None))));
        }
        Phase::Story | Phase::SignIn | Phase::Drain => {
            world.spawn_root(skein_sim::Handle::new(client.expect("story client root").raw()), |root| {
                Proc::Client(Box::new(SignalHost::new(Person::new(root, scenario), None)))
            });
            world.spawn(|| Proc::Peer(Box::new(SignalHost::new(stories::provider(scenario), None))));
            world.spawn(|| Proc::Forge(Box::new(SignalHost::new(peer(), None))));
        }
    }
    let mut outcome = world.run();
    if matches!(scenario, Story::Adopt) {
        observed_forge.borrow().require_hooks(stories::ADOPT_HOOKS);
    }
    let signed_in = signed_in(&outcome.procs);
    let chats_answered = if matches!(scenario, Story::ChatReadThenWait { .. }) {
        u32::from(referee::chat_waited(&store.borrow()))
    } else {
        referee::terminals(&store.borrow())
    };
    let adopted = stories::adopted(&store.borrow().rows);
    if matches!(phase, Phase::Fault { .. }) {
        referee::require_replayed(&run.borrow(), &store.borrow());
    }
    let pushed = std::mem::take(&mut observed_forge.borrow_mut().pushed);
    let deliveries = deliveries(&outcome.procs);
    assert!(outcome.procs.iter().all(Host::is_empty), "all processes drain");
    share(None);
    STORE.with(|cell| {
        *cell.borrow_mut() = None;
    });
    let records = std::mem::take(&mut *store.borrow_mut());
    let result = Outcome {
        verdict: skein_world::domain::Verdict::Passed,
        signed_in,
        settings_revision: settings_revision(&records),
        chats_answered,
        adopted,
        pushed,
        deliveries,
        drained: run.borrow().drained,
        restarts: run.borrow().restarts,
        claims_settled_at_restart: run.borrow().settled,
        adoptions_answered: u32::try_from(run.borrow().answers.len()).expect("adoption answers"),
        adoption_pending_at_restart: run.borrow().pending_adoption,
    };
    drop(run);
    let startup = STARTUP.with(|slot| slot.borrow_mut().take().expect("phase inputs"));
    (result, std::mem::replace(&mut outcome.machine.0, machine::fresh()), records, startup)
}

#[cfg(test)]
mod tests;

/// A stopped one-process story, retaining its machine and exact startup inputs.
pub struct World {
    disk: skein_fake_machine::Machine,
    startup: crate::deployment::Startup,
    pub outcome: Outcome,
}
impl World {
    #[must_use]
    pub fn run(settings: Settings, story: Story) -> Self {
        let (outcome, disk, _, startup) = run_phase_with_startup(
            settings,
            story,
            Phase::Story,
            machine::fresh(),
            Store::new(),
            Rc::new(RefCell::new(referee::Run::default())),
            None,
        );
        Self { disk, startup, outcome }
    }
    /// Capture all entries after every process has drained (protocol/README.md, 9).
    #[must_use]
    pub fn deployment(&mut self) -> crate::deployment::Deployment {
        crate::deployment::Deployment::capture(&mut self.disk, &self.startup)
    }
}
/// Lay a kept deployment on a fresh machine and run startup, then drain.
#[must_use]
pub fn over(deployment: &crate::deployment::Deployment, settings: Settings) -> World {
    let (outcome, disk, _, startup) = run_phase_with_startup(
        settings,
        Story::Chat { words: b"Hello" },
        Phase::Open,
        deployment.machine(),
        Store::over(deployment),
        Rc::new(RefCell::new(referee::Run::default())),
        Some(deployment.startup()),
    );
    World { disk, startup, outcome }
}

fn signed_in(procs: &[Proc]) -> u32 {
    procs
        .iter()
        .filter_map(|proc| match proc {
            Proc::Client(person) => Some(person.signed_in),
            Proc::Service(_)
            | Proc::Agent(_)
            | Proc::Git(_)
            | Proc::Shell(_)
            | Proc::Peer(_)
            | Proc::Forge(_)
            | Proc::Ctl(_) => None,
        })
        .sum()
}
