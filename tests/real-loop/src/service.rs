//! Deferred standalone preparation with its original admission envelope.
mod cue;
mod hooks;
mod watch;
use crate::fixtures;
pub use cue::Cue;
use skein_io::kernel::{Complete, Fd, Submit};
use skein_lib::{Queue, Time, Wall};
use skein_shell::Host;
use std::{
    net::{SocketAddr, TcpListener},
    path::PathBuf,
};

pub struct ServiceHost {
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
    direct_key: bool,
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
        Self::with_shape(path, signals, fixture, direct_key, None, None)
    }
    fn with_shape(
        path: PathBuf,
        signals: Fd,
        fixture: Option<crate::forge::Fixture>,
        direct_key: bool,
        cluster: Option<crate::cluster::Shape>,
        github: Option<crate::github::Fixture>,
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
        } else if fixture.is_some() {
            envelope.0 += 3 * 4;
        } else if cluster.is_some() {
            envelope.0 += 4;
        }
        Self {
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
            direct_key,
            path,
            signals,
            forge,
            envelope,
            completions: Queue::with_capacity(1),
            submissions: Queue::with_capacity(1),
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
        Self::with_shape(path.join("data"), signals, fixture, false, Some(shape), None)
    }
    pub(crate) fn github(path: PathBuf, signals: Fd, fixture: crate::github::Fixture) -> Self {
        Self::with_shape(path, signals, None, false, None, Some(fixture))
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
        self.notes.iter().filter(|note| matches!(note, temper_service::Note::Linked { .. })).count()
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
        if self.service.is_none()
            && self.ready()
            && let Some(address) = self.address
        {
            let forge = self
                .forge_address
                .unwrap_or_else(|| self.forge.as_ref().expect("held forge").local_addr().expect("forge address"));
            fixtures::write(&self.path, address, forge).expect("bound standalone documents");
            if self.direct_key {
                fixtures::key_documents(&self.path).expect("standard key route");
            }
            if let Some(fixture) = self.fixture {
                crate::forge::write(&self.path, forge, &fixture).expect("forge documents");
            }
            if let Some(fixture) = self.github {
                crate::github::write(&self.path, forge, self.clients.expect("fixed clients"), fixture)
                    .expect("GitHub documents");
                temper_github_app::seed(
                    &self.path.join("state"),
                    &temper_github_app::records(&temper_github_app::APP, 1, &[crate::github::INSTALLATION], forge),
                )
                .expect("durable App seed");
            }
            if let Some(shape) = &self.cluster {
                shape.write(self.path.parent().expect("cluster scratch"));
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
            self.started_at = Some(now);
            self.service = Some(service);
        }
        if let Some(service) = &mut self.service {
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
                self.notes.push(note);
            }
            while let Some(note) = service.notes().pop() {
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
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        match &mut self.service {
            Some(service) => service.completions(),
            None => &mut self.completions,
        }
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        match &mut self.service {
            Some(service) => service.submissions(),
            None => &mut self.submissions,
        }
    }
    fn work_pending(&self, now: Time) -> bool {
        self.service.as_ref().map_or(self.address.is_some() && self.ready(), |service| service.work_pending(now))
    }
    fn next_deadline(&self) -> Option<Time> {
        self.service.as_ref().and_then(temper_service::Service::next_deadline)
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        self.service.as_ref().and_then(temper_service::Service::next_policy_deadline)
    }
    fn is_empty(&self) -> bool {
        self.service.as_ref().is_some_and(temper_service::Service::is_empty)
    }
    fn worst_case(&self) -> u64 {
        self.envelope.0
            + if self.cluster.is_some() { crate::cluster::Runs::MEMORY } else { 0 }
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
