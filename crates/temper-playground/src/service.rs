//! The service as temper serve hosts its engine and worker (domain/testing.md, 2.5).
use skein_io::kernel::{Complete, Submit};
use skein_lib::{List, Queue, Time, Wall};
use skein_shell::Host;
use skein_sim_world::Inherited;

/// What temper serve reads at its start, handed over as values.
pub struct Start {
    pub document: Box<[u8]>,
    pub files: temper_service::settings::Files,
    pub agent: temper_service::settings::Agent,
    pub seeds: temper_service::settings::Seeds,
    pub forges: List<temper_service::ForgeRuntime>,
    /// The effective user temper serve reads at its start and the store's files belong to.
    pub user: u32,
}
/// The service's engine and one worker in one iterate (domain/testing.md, 2.5).
pub struct Service {
    service: temper_service::Service,
}
impl Service {
    /// Starts from values over the world's opened state and worker roots and signal source.
    #[must_use]
    pub fn start(start: &Start, inherited: &Inherited) -> Self {
        let mut forges = List::with_capacity(start.forges.len());
        for forge in &start.forges {
            forges
                .push(temper_service::ForgeRuntime {
                    number: forge.number,
                    address: forge.address,
                    transport: match &forge.transport {
                        skein_forge_connection::settings::Transport::Plaintext { name } => {
                            skein_forge_connection::settings::Transport::Plaintext { name: name.clone() }
                        }
                        skein_forge_connection::settings::Transport::Tls { server_name, trust } => {
                            skein_forge_connection::settings::Transport::Tls {
                                server_name: server_name.clone(),
                                trust: trust.clone(),
                            }
                        }
                    },
                    trust_roots: forge.trust_roots,
                    trust_root_bytes: forge.trust_root_bytes,
                })
                .expect("forge runtime capacity");
        }
        let root = inherited.roots.iter().find(|(name, _)| name.as_ref() == b"state").expect("state root").1;
        let worker = inherited.roots.iter().find(|(name, _)| name.as_ref() == b"worker").expect("worker root").1;
        let mut service = temper_service::startup::startup(
            &start.document,
            // Simulated configuration paths stay relative to their inherited roots.
            &jig_protocol_config::Directories::working(),
            &start.files,
            &start.agent,
            &start.seeds,
            None,
            temper_service::WorkerRuntime { root: worker, path: b"/worker".as_slice().into() },
            forges,
        )
        .expect("playground startup");
        service.adopt_store(root, start.user).expect("deployment store");
        service.adopt_signals(inherited.signal).expect("service signal");
        Self { service }
    }
    /// The service itself, for a harness that observes it.
    pub fn service(&mut self) -> &mut temper_service::Service {
        &mut self.service
    }
    /// Formats the roots for an outside observer's secret check.
    /// # Errors
    /// Returns the formatter's error if writing fails.
    pub fn fmt_roots(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.service.fmt_roots(formatter)
    }
}
impl Host for Service {
    fn iterate(&mut self, now: Time, wall: Wall) {
        temper_service::iterate(&mut self.service, now, wall);
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
        self.service.worst_case()
    }
    fn operations(&self) -> u32 {
        self.service.operations()
    }
}
