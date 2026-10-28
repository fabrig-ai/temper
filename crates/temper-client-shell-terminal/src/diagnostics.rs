//! Bounded, payload-free state for an overdue real-loop snapshot.
use jig_client::{contract as c, thread::Reading};
use skein_lib::List;

#[derive(Debug)]
pub struct Pending {
    pub loaded: bool,
    pub dirty: bool,
    pub shell: jig_client_shell_native::diagnostics::Pending,
    pub admission: jig_client::Admission,
    pub chat: Option<Chat>,
    pub view_revision: Option<u64>,
    pub builds: u64,
    pub patches: u64,
}
#[derive(Debug)]
pub struct Chat {
    pub document_state: Option<jig_client::State>,
    pub activity: Option<u64>,
    pub committed: u64,
    pub loaded: u64,
    pub reading: Option<Reading>,
    pub last_read_none: Option<c::ReadNone>,
    pub retained: List<Row>,
    pub staged: u32,
}
#[derive(Debug)]
pub struct Row {
    pub position: u64,
    pub committed_turn: Option<c::TurnName>,
}
