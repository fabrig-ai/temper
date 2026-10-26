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
    inner: temper_service::Service,
    printer: Option<skein_sim_world::Printer>,
    files: Option<temper_service::settings::Files>,
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
        let directories = jig_protocol_config::Directories::working();
        let setup = crate::startup::setup(&start.document, &directories);
        let mut service = temper_service::startup::with_setup(
            &start.document,
            // Simulated configuration paths stay relative to their inherited roots.
            &directories,
            &start.files,
            Some(&start.agent),
            &start.seeds,
            None,
            Some(temper_service::WorkerRuntime { root: worker, path: b"/worker".as_slice().into() }),
            forges,
            Some(&setup),
            false,
        )
        .expect("playground startup");
        service.adopt_store(root, start.user).expect("deployment store");
        service.adopt_signals(inherited.signal).expect("service signal");
        Self { inner: service, printer: None, files: None }
    }
    /// Sends sign-in links to the world's standard output, as temper serve does.
    #[must_use]
    pub fn with_printer(mut self, printer: skein_sim_world::Printer) -> Self {
        self.printer = Some(printer);
        self
    }
    /// Supplies the host's private files for later demo-key reads and handover removals.
    #[must_use]
    pub fn with_files(mut self, files: &temper_service::settings::Files) -> Self {
        self.files = Some(temper_service::settings::Files { entries: files.entries.clone() });
        self
    }
    /// The service itself, for a harness that observes it.
    pub fn service(&mut self) -> &mut temper_service::Service {
        &mut self.inner
    }
    /// Formats the roots for an outside observer's secret check.
    /// # Errors
    /// Returns the formatter's error if writing fails.
    pub fn fmt_roots(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt_roots(formatter)
    }
}
impl Host for Service {
    fn iterate(&mut self, now: Time, wall: Wall) {
        temper_service::iterate(&mut self.inner, now, wall);
        if let Some(files) = &mut self.files {
            while let Some(read) = self.inner.take_handover_read() {
                let answer = match files.entries.iter().find(|(path, _)| *path == read.path) {
                    Some((_, value)) if value.expose().len() > read.max as usize => {
                        Err(temper_service::HandoverUnread::TooLarge)
                    }
                    Some((_, value)) => {
                        let bytes = value.expose();
                        Ok(Some(skein_lib::Secret::new(bytes.strip_suffix(b"\n").unwrap_or(bytes).into())))
                    }
                    None => Ok(None),
                };
                self.inner.handover_read(read.forge, read.run, answer);
            }
            while let Some(remove) = self.inner.take_handover_removal() {
                // Retain absence on later reads, just as an unlinked handover.
                let mut retained = List::with_capacity(files.entries.capacity());
                for (path, value) in &files.entries {
                    if *path != remove.path {
                        retained.push((path.clone(), value.clone())).expect("existing files fit");
                    }
                }
                files.entries = retained;
                self.inner.handover_removed(remove.forge, remove.run, Ok(()));
            }
        }
        if let Some(printer) = &mut self.printer {
            use std::io::Write;
            while let Some(link) = self.inner.links().pop() {
                if printer
                    .write_all(&link)
                    .and_then(|()| printer.write_all(b"\n"))
                    .and_then(|()| printer.flush())
                    .is_err()
                {
                    self.inner.shutdown();
                }
            }
        }
    }
    fn completions(&mut self) -> &mut Queue<Complete> {
        self.inner.completions()
    }
    fn submissions(&mut self) -> &mut Queue<Submit> {
        self.inner.submissions()
    }
    fn work_pending(&self, now: Time) -> bool {
        self.inner.work_pending(now)
    }
    fn next_deadline(&self) -> Option<Time> {
        self.inner.next_deadline()
    }
    fn next_policy_deadline(&self) -> Option<Time> {
        self.inner.next_policy_deadline()
    }
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    fn worst_case(&self) -> u64 {
        self.inner.worst_case()
    }
    fn operations(&self) -> u32 {
        self.inner.operations()
    }
}
