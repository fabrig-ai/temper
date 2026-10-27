//! Deferred standalone preparation with its original admission envelope.
mod cue;
mod hooks;
mod watch;
use crate::fixtures;
pub use cue::Cue;
use skein_io::kernel::{Complete, Done, Fd, Op, Submit};
use skein_lib::{Queue, Time, Wall};
use skein_shell::Host;
use std::{
    net::{SocketAddr, TcpListener},
    path::PathBuf,
};

/// One fully settled service life, retained across a worker restart.
pub struct Life {
    /// Whether this service drained cooperatively.
    pub drained: bool,
    /// Tasks run during this life, in activation order.
    pub ran: Vec<u64>,
    /// Worker links opened during this life.
    pub linked: usize,
}

const LIVES: usize = 2;
#[derive(Clone, Copy, PartialEq, Eq)]
enum LifeState {
    First,
    Stopping,
    Restarting,
    Second,
}

pub struct ServiceHost {
    pub(crate) pending_streak: crate::patience::PendingStreak,
    restartable: bool,
    lives: Vec<Life>,
    state: LifeState,
    in_flight: usize,
    local_completion: bool,
    life_tasks: usize,
    life_links: usize,
    linked: usize,

    pub(crate) cluster: Option<crate::cluster::Shape>,
    pub(crate) workers_address: Option<SocketAddr>,
    pub(crate) notes: Vec<temper_service::Note>,
    pub(crate) runs: crate::cluster::Runs,
    started_at: Option<Time>,
    pub(crate) service: Option<temper_service::Service>,
    pub(crate) address: Option<SocketAddr>,
    pub(crate) link: Vec<u8>,
    watch: watch::Watch,
    hook_wire: hooks::Hooks,
    pub(crate) hooks: Option<SocketAddr>,
    pub(crate) forge_address: Option<SocketAddr>,
    pub(crate) fixture: Option<crate::forge::Fixture>,
    pub(crate) seen: crate::forge::Seen,
    github: Option<crate::github::Fixture>,
    clients: Option<SocketAddr>,
    pub(crate) github_seen: crate::github::Seen,
    cues: Vec<Cue>,
    links: u32,
    path: PathBuf,
    signals: Fd,
    forge: Option<TcpListener>,
    envelope: (u64, u32),
    completions: Queue<Complete>,
    submissions: Queue<Submit>,
}
impl ServiceHost {
    pub(crate) fn new(path: PathBuf, signals: Fd, direct_key: bool) -> Self {
        Self::configured(path, signals, None, direct_key)
    }
    pub(crate) fn configured(
        path: PathBuf,
        signals: Fd,
        fixture: Option<crate::forge::Fixture>,
        direct_key: bool,
    ) -> Self {
        Self::with_shape(path, signals, fixture, direct_key, None, None, None)
    }
    pub(crate) fn with_issuer(path: PathBuf, signals: Fd, issuer: SocketAddr) -> Self {
        Self::with_shape(path, signals, None, false, None, None, Some(issuer))
    }
    fn with_shape(
        path: PathBuf,
        signals: Fd,
        fixture: Option<crate::forge::Fixture>,
        direct_key: bool,
        cluster: Option<crate::cluster::Shape>,
        github: Option<crate::github::Fixture>,
        issuer: Option<SocketAddr>,
    ) -> Self {
        let clients = github.map(|_| {
            let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).expect("reserved clients listener");
            listener.local_addr().expect("reserved clients address")
        });
        let forge = (fixture.is_none() && github.is_none())
            .then(|| TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).expect("held forge listener"));
        let address = forge
            .as_ref()
            .map_or(([127, 0, 0, 1], 0).into(), |listener| listener.local_addr().expect("held forge address"));
        fixtures::write(&path, address, address).expect("stand-in standalone documents");
        if direct_key {
            fixtures::key_documents(&path).expect("standard key route");
        }
        if let Some(fixture) = fixture {
            crate::forge::write(&path, address, &fixture).expect("forge documents");
        }
        if let Some(fixture) = github {
            crate::github::write(&path, address, clients.expect("fixed clients"), fixture).expect("GitHub documents");
        }
        if let Some(shape) = &cluster {
            shape.write(path.parent().expect("cluster scratch"));
        }
        if let Some(address) = issuer {
            fixtures::sign_in_documents(&path, address).expect("local issuer registration");
        }
        let roots = Self::directories(&path, cluster.is_some());
        let config = std::fs::read(roots.document()).expect("standalone document");
        let mut envelope = temper::envelope_in(&roots, &config).expect("standalone admission envelope");
        // The deferred document names port 0 in its LLM, API and git addresses.
        // Each kernel-assigned port can add four decimal digits at preparation.
        if github.is_some() {
            // Besides API/git/web, GitHub setup retains web in each run
            // and its child plan (skein's forge-setup host admission).
            let runs = temper_service::limits::profile(temper_service::limits::Profile::Small).setup.runs;
            envelope.0 += (3 + 2 * u64::from(runs)) * 4;
            // Each of the clients' 8 HTTP connections (limits::server) may hold a
            // manifest page's policy, priced with web twice (temper-service setup.rs).
            envelope.0 += 2 * 8 * 4;
        } else if fixture.is_some() {
            envelope.0 += 3 * 4;
        } else if cluster.is_some() {
            envelope.0 += 4;
        }
        let restartable = matches!(cluster, Some(crate::cluster::Shape::Worker { .. }));
        let queued = if restartable { envelope.1 } else { 1 };
        Self {
            pending_streak: crate::patience::PendingStreak::default(),
            restartable,
            lives: Vec::with_capacity(if restartable { LIVES } else { 0 }),
            state: LifeState::First,
            in_flight: 0,
            local_completion: false,
            life_tasks: 0,
            life_links: 0,
            linked: 0,
            cluster,
            workers_address: None,
            notes: Vec::new(),
            runs: crate::cluster::Runs::default(),
            started_at: None,
            service: None,
            address: None,
            link: Vec::new(),
            watch: watch::Watch::default(),
            hook_wire: hooks::Hooks::default(),
            hooks: None,
            forge_address: None,
            fixture,
            github,
            clients,
            github_seen: crate::github::Seen::default(),
            seen: crate::forge::Seen::default(),
            cues: Vec::with_capacity(cue::CUES),
            links: 0,
            path,
            signals,
            forge,
            envelope,
            completions: Queue::with_capacity(queued),
            submissions: Queue::with_capacity(queued),
        }
    }
}
impl ServiceHost {
    pub(crate) fn cluster(
        path: &std::path::Path,
        signals: Fd,
        shape: crate::cluster::Shape,
        fixture: Option<crate::forge::Fixture>,
    ) -> Self {
        Self::with_shape(path.join("data"), signals, fixture, false, Some(shape), None, None)
    }
    pub(crate) fn github(path: PathBuf, signals: Fd, fixture: crate::github::Fixture) -> Self {
        Self::with_shape(path, signals, None, false, None, Some(fixture), None)
    }
    pub(crate) fn make_github_app(&mut self, peer: &mut skein_fake_peers::forge::Peer) {
        if let Some(fixture) = self.github
            && !fixture.connected
            && !fixture.manifest
        {
            self.github_seen.app =
                Some(crate::github::make_app(peer, self.clients.expect("clients"), &self.path, fixture));
        }
    }
    pub(crate) fn checkpoint_github(
        &mut self,
        peer: &mut skein_fake_peers::forge::Peer,
        calls: &[crate::calls::Called],
    ) {
        if let Some(fixture) = self.github
            && fixture.unaccepted_after_install
        {
            self.github_seen.seed_unaccepted(peer);
        }
        self.github_seen.checkpoint(peer, calls);
    }
    fn github_documents(&self, forge: SocketAddr) {
        let Some(fixture) = self.github else { return };
        if fixture.connected {
            temper_github_app::seed(
                &self.path.join("state"),
                &temper_github_app::records(&temper_github_app::APP, 1, &[crate::github::INSTALLATION], forge),
            )
            .expect("durable App seed");
        }
    }
    /// Settled lives in order; `ran()` and `linked()` retain their aggregate history.
    #[must_use]
    pub fn lives(&self) -> &[Life] {
        &self.lives
    }
    pub(crate) fn stopping(&self) -> bool {
        matches!(self.state, LifeState::Stopping | LifeState::Restarting)
    }
    pub(crate) fn stop_for_restart(&mut self) {
        assert!(
            self.restartable && self.state == LifeState::First && !self.is_empty(),
            "stop the first live worker service"
        );
        self.state = LifeState::Stopping;
    }
    pub(crate) fn settled(&self) -> bool {
        self.in_flight == 0
            && self.completions.is_empty()
            && self.submissions.is_empty()
            && self.service.as_ref().is_some_and(temper_service::Service::is_empty)
    }
    pub(crate) fn restart(&mut self) {
        assert!(self.state == LifeState::Stopping && self.settled(), "restart a settled worker");
        // The referee requests only; dropping and preparing stay in this host's iteration.
        self.state = LifeState::Restarting;
    }
    fn begin_next_life(&mut self) {
        self.life_tasks = self.runs.tasks.len();
        self.life_links = self.linked();
        self.service = None;
        self.state = LifeState::Second;
    }
    fn record_life(&mut self) {
        if self.restartable && self.settled() && self.lives.len() == usize::from(self.state == LifeState::Second) {
            assert!(self.lives.len() < LIVES, "bounded service lives");
            self.lives.push(Life {
                drained: self.end() == Some(&crate::End::Drained),
                ran: self.runs.tasks[self.life_tasks..].to_vec(),
                linked: self.linked() - self.life_links,
            });
        }
    }
    fn directories(path: &std::path::Path, cluster: bool) -> temper::directories::Directories {
        if cluster {
            let path = path.parent().expect("cluster scratch");
            crate::cluster::directories_in([&path.join("config"), &path.join("data"), &path.join("cache")])
        } else {
            directories(path)
        }
    }
    fn ready(&self) -> bool {
        self.cluster.as_ref().is_none_or(crate::cluster::Shape::ready)
            && ((self.fixture.is_none() && self.github.is_none()) || self.forge_address.is_some())
    }
    #[must_use]
    pub fn end(&self) -> Option<&crate::End> {
        self.service.as_ref().and_then(temper_service::Service::end)
    }
    #[must_use]
    pub fn started_at(&self) -> Option<Time> {
        self.started_at
    }
    #[must_use]
    pub fn assigned_at(&self) -> Option<Time> {
        self.runs.assigned_at
    }
    #[must_use]
    pub fn ran(&self) -> &[u64] {
        &self.runs.tasks
    }
    /// Worker openings retained from the shipped service's diagnostics.
    #[must_use]
    pub fn linked(&self) -> usize {
        self.linked
    }
    /// Links printed by this service start, independent of the scratch's old file.
    #[must_use]
    pub fn links(&self) -> u32 {
        self.links
    }
    pub(crate) fn arm_watch(&mut self, call: usize) {
        if self.watch.call != Some(call) {
            self.watch = watch::Watch::default();
            self.watch.call = Some(call);
        }
        self.watch.armed = true;
    }
    pub fn arm(&mut self, key: skein_lib::RequestKey) {
        assert!(self.cues.len() < cue::CUES, "bounded held cues");
        self.cues.push(Cue::new(key));
    }
    #[must_use]
    pub fn held(&self) -> bool {
        !self.cues.is_empty() && self.cues.iter().all(Cue::held)
    }
    pub(crate) fn held_at(&self, index: usize) -> bool {
        self.cues.get(index).is_some_and(Cue::held)
    }
    pub(crate) fn watching(&self) -> bool {
        self.watch.armed
    }
    pub(crate) fn delivered(&self) -> bool {
        self.watch.delivered
    }
}
impl Host for ServiceHost {
    fn iterate(&mut self, now: Time, wall: Wall) {
        if self.state == LifeState::Restarting {
            self.begin_next_life();
        }
        if self.service.is_none()
            && self.ready()
            && let Some(address) = self.address
        {
            let forge = self
                .forge_address
                .unwrap_or_else(|| self.forge.as_ref().expect("held forge").local_addr().expect("forge address"));
            if self.state != LifeState::Second {
                fixtures::bind(&self.path, address, forge).expect("bound standalone documents");
                self.github_documents(forge);
                if let Some(shape) = &self.cluster {
                    shape.write(self.path.parent().expect("cluster scratch"));
                }
            }
            let mut service = temper::prepare_in(&Self::directories(&self.path, self.cluster.is_some()))
                .expect("standalone preparation");
            if self.cluster.is_some() {
                assert!(service.worst_case() <= self.envelope.0, "cluster fits admission");
                assert!(service.operations() <= self.envelope.1, "cluster operations fit admission");
            } else if self.fixture.is_some() || self.github.is_some() {
                assert!(
                    service.worst_case() <= self.envelope.0,
                    "bound ports fit deferred admission: {} <= {}",
                    service.worst_case(),
                    self.envelope.0
                );
                assert_eq!(service.operations(), self.envelope.1, "preparation preserves operations");
            } else {
                assert_eq!(
                    (service.worst_case(), service.operations()),
                    self.envelope,
                    "preparation preserves admission"
                );
            }
            service.adopt_signals(self.signals).expect("service signal pipe");
            self.started_at.get_or_insert(now);
            self.service = Some(service);
        }
        if let Some(service) = &mut self.service {
            self.local_completion = false;
            if self.restartable {
                while let Some(complete) = self.completions.pop() {
                    self.in_flight = self.in_flight.checked_sub(1).expect("a completion has a submitted operation");
                    service.completions().push(complete);
                }
            }
            for complete in service.completions().iter() {
                if self.fixture.is_some() {
                    self.hook_wire.complete(complete, self.hooks, &mut self.seen.responses);
                }
                if let Some(crate::cluster::Shape::Worker { engine: Some(engine), .. }) = self.cluster {
                    self.runs.complete(complete, engine, now);
                }
                self.watch.complete(complete);
                for cue in &mut self.cues {
                    cue.complete(complete);
                }
            }
            temper_service::iterate(service, now, wall);
            while let Some(note) = service.listening().pop() {
                if let temper_service::Note::Listening { listener: temper_service::Listening::Workers, addr } = note {
                    self.workers_address = Some(addr);
                }
                self.linked += usize::from(matches!(note, temper_service::Note::Linked { .. }));
                self.notes.push(note);
            }
            while let Some(note) = service.notes().pop() {
                self.linked += usize::from(matches!(note, temper_service::Note::Linked { .. }));
                self.notes.push(note);
            }
            while let Some(link) = service.links().pop() {
                self.links += 1;
                self.link.clear();
                self.link.extend_from_slice(&link);
                self.link.push(b'\n');
                fixtures::private(&self.path.join("link"), &self.link).expect("private printed link");
            }
            self.hooks = self.hook_wire.address(&self.link);
            if service.is_empty() {
                self.forge = None;
            }
        }
        self.record_life();
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        if !self.restartable
            && let Some(service) = &mut self.service
        {
            service.completions()
        } else {
            &mut self.completions
        }
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        if !self.restartable {
            return match &mut self.service {
                Some(service) => service.submissions(),
                None => &mut self.submissions,
            };
        }
        if let Some(service) = &mut self.service {
            while let Some(submit) = service.submissions().pop() {
                if self.state == LifeState::Stopping && matches!(submit.kind, Op::Close { fd } if fd == self.signals) {
                    self.local_completion = true;
                    // The supervisor owns this pipe across both service lives.
                    // Complete the inner close, retaining the registered reader.
                    service.completions().push(Complete {
                        op: submit.op,
                        kind: submit.kind,
                        result: Ok(Done::Nothing),
                    });
                } else {
                    self.in_flight += 1;
                    self.submissions.push(submit);
                }
            }
        }
        &mut self.submissions
    }
    fn work_pending(&self, now: Time) -> bool {
        (self.restartable
            && (self.state == LifeState::Restarting || self.local_completion || !self.completions.is_empty()))
            || self.service.as_ref().map_or(self.address.is_some() && self.ready(), |service| service.work_pending(now))
    }
    fn next_deadline(&self) -> Option<Time> {
        self.service.as_ref().and_then(temper_service::Service::next_deadline)
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        self.service.as_ref().and_then(temper_service::Service::next_policy_deadline)
    }
    fn is_empty(&self) -> bool {
        if self.restartable {
            !self.stopping() && self.settled()
        } else {
            self.service.as_ref().is_some_and(temper_service::Service::is_empty)
        }
    }
    fn worst_case(&self) -> u64 {
        self.envelope.0
            + if self.cluster.is_some() { crate::cluster::Runs::MEMORY } else { 0 }
            + if self.restartable {
                Queue::<Complete>::worst_case(self.envelope.1).expect("host completions")
                    + Queue::<Submit>::worst_case(self.envelope.1).expect("host submissions")
                    + u64::try_from(LIVES * std::mem::size_of::<Life>()).expect("life records")
                    + crate::cluster::Runs::MEMORY
            } else {
                0
            }
            + watch::Watch::memory()
            + cue::CUES as u64 * Cue::memory()
            + if self.fixture.is_some() { hooks::Hooks::memory() } else { 0 }
    }
    fn operations(&self) -> u32 {
        self.envelope.1
    }
}

/// All directories are explicit and belong to this fixture's scratch.
#[must_use]
pub fn directories(directory: &std::path::Path) -> temper::directories::Directories {
    use std::os::unix::ffi::OsStrExt;
    let values = [
        b"--config".as_slice(),
        directory.as_os_str().as_bytes(),
        b"--data",
        directory.as_os_str().as_bytes(),
        b"--cache",
        directory.as_os_str().as_bytes(),
    ];
    temper::directories::resolve(&values.iter().map(|v| Box::from(*v)).collect::<Vec<_>>(), &[], b"/")
        .expect("fixture directories")
        .0
}
