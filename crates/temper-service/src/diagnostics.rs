//! Read-only, payload-free snapshots, constructed only when explicitly requested.
use skein_lib::{List, Time};

#[derive(Debug)]
#[expect(clippy::struct_excessive_bools, reason = "each flag names a separate service readiness term")]
pub struct Pending {
    pub work_pending: bool,
    pub worker_runs: bool,
    pub io_ready: bool,
    pub io_due: bool,
    pub completions: u32,
    pub told: u32,
    pub raw_submissions: u32,
    pub ended: bool,
    pub signals: bool,
    pub signals_closing: bool,
    pub box_pipe_pending: bool,
    pub engine: Option<Engine>,
    pub worker: Option<Worker>,
}

#[derive(Debug)]
#[expect(clippy::struct_excessive_bools, reason = "the snapshot exposes the engine's independent readiness gates")]
pub struct Engine {
    pub work_pending: bool,
    pub clients: bool,
    pub setup: bool,
    pub setup_lifecycle: bool,
    pub store: bool,
    pub forge: bool,
    pub workers: bool,
    pub accounts: bool,
    pub store_opened: bool,
    pub ended: bool,
    pub room: bool,
    pub begin: bool,
    pub ready: bool,
    pub domain_ready: bool,
    pub release_pending: bool,
    pub blocked: bool,
    pub pending: Option<EngineEvent>,
    pub events: List<EngineEvent>,
    pub due: bool,
    pub first_periods: u32,
    pub settled_revision: Option<u64>,
    pub revision: u64,
    pub next_deadline: Option<Time>,
    pub requests: u32,
    pub workers_opened: bool,
    pub clients_opened: bool,
}

#[derive(Debug)]
pub enum EngineEvent {
    Setup,
    Store,
    Worker,
    Party,
    Account,
    Credential,
    Forge,
    Released,
    Restart,
    Resume,
    Stop,
}

#[derive(Debug)]
#[expect(clippy::struct_excessive_bools, reason = "each flag identifies a separate worker readiness gate")]
pub struct Worker {
    pub work_pending: bool,
    pub started: bool,
    pub aborted: bool,
    pub root_ready: bool,
    pub events: List<WorkerEvent>,
    pub process_events: u32,
    pub io: u32,
    pub submitted: u32,
    pub outputs: u32,
    pub link_outputs: u32,
    pub retained_turns: u32,
    pub kept_answers: u32,
    pub next_deadline: Option<Time>,
    pub attempts: List<jig_domain_host::Hosting>,
}

#[derive(Debug)]
pub enum WorkerEvent {
    SourceAnswered,
    Assign,
    Link(LinkEvent),
    Ready,
    Lost,
    GracePassed,
    Resend,
    RelayCancelled,
    Process,
    Live,
    Decoded,
    Encoded,
    Activity,
    Done,
    Shutdown,
}

#[derive(Debug)]
pub enum LinkEvent {
    Sources,
    Check,
    SignOut,
    Assign,
    Message,
    Relayed,
    TurnKept,
    Acknowledge,
    Cancel,
    Grant,
}
