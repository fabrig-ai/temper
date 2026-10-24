//! A procedure's whole panel, watched while its task page is open
//! (client/forge.md, 9.2; client/README.md, 4.6).
use super::{Limits, Request, Side, wire_limits};
use alloc::boxed::Box;
use jig_client::{Client, Handle, Latest, Watchable, Why, contract as c};
use skein_lib::Queue;
use temper_client_protocol_wire::{self as wire, v1 as w};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Subject {
    pub task: u64,
    pub project: u32,
}
#[derive(Debug, PartialEq, Eq)]
pub enum State {
    Loading,
    Shown(Box<w::ChangePanel>),
    Unknown,
    Ended(Why),
}
#[derive(Debug)]
pub struct Panel {
    pub subject: Subject,
    pub state: State,
    pub handle: Option<Handle>,
}
/// A delivery needs an unwatch on invalid bytes; a watch jig ended does not.
#[derive(Debug)]
pub enum Watched {
    Delivered(Option<Box<w::ChangePanel>>),
    Ended(Why),
}
/// A later document can recover a live change, or its landed result (client/forge.md, 9.2).
pub(crate) fn refreshable(task: &c::Task, forge: u16) -> bool {
    let procedure = match task.chip.executor {
        c::Executor::Procedure { connector, kind } => connector == forge && kind == 2,
        c::Executor::Agent { .. } | c::Executor::Party { .. } | c::Executor::Role { .. } => false,
    };
    let change = match task.result_contract {
        c::TaskContractDocument::Change { connector, .. } => connector == forge,
        c::TaskContractDocument::Report { .. }
        | c::TaskContractDocument::Verdict { .. }
        | c::TaskContractDocument::Approval { .. } => false,
    };
    let live = match task.chip.phase {
        c::Phase::Ended | c::Phase::Done | c::Phase::Failed | c::Phase::Cancelled => false,
        c::Phase::Waiting
        | c::Phase::Idle
        | c::Phase::Due
        | c::Phase::Preparing
        | c::Phase::Claimed
        | c::Phase::Running
        | c::Phase::Active
        | c::Phase::BackingOff
        | c::Phase::Closing
        | c::Phase::Held => true,
    };
    procedure
        && change
        && (live || crate::pages::delegation::facts(task).state == crate::pages::delegation::State::Landed)
}
pub(super) fn refresh(side: &mut Side, limits: Limits, subject: Subject, out: &mut Queue<Request>) {
    let Some(panel) = &side.panel else { return };
    if !side.panel_retry || panel.subject != subject {
        return;
    }
    side.panel = None;
    self::subject(side, limits, Some(subject), out);
}
pub(super) fn subject(side: &mut Side, limits: Limits, subject: Option<Subject>, out: &mut Queue<Request>) {
    #[expect(clippy::manual_map, reason = "the client's domain matches without a closure")]
    let current = match &side.panel {
        Some(panel) => Some(panel.subject),
        None => None,
    };
    if current == subject {
        return;
    }
    side.panel_retry = false;
    if let Some(panel) = side.panel.take()
        && let Some(handle) = panel.handle
    {
        out.push(Request::Client { asked: jig_client::Event::Unwatch { handle } });
    }
    if let Some(subject) = subject {
        side.serial = side.serial.checked_add(1).expect("forge handles fit");
        let handle = Handle { asker: crate::owner::Owner::Forge.asker(), number: side.serial };
        let limits = wire_limits(&limits).expect("checked forge limits");
        let change =
            w::ChangeSubject::new(&limits, w::ChangeSubjectParts { task: subject.task }).expect("change subject fits");
        let record =
            w::Watching::new(&limits, w::WatchingParts { watch: w::Watch::Change(change) }).expect("change watch fits");
        out.push(Request::Client {
            asked: jig_client::Event::Watch {
                handle,
                subject: Watchable::Family {
                    scope: c::Scope::Project(subject.project),
                    // The window is unused: every delivery is the whole panel (client/forge.md, 9.2).
                    window: c::Window { rows: 0, after: None },
                    record: jig_client::Family {
                        family: wire::FORGE,
                        version: wire::CLIENT_VERSION,
                        bytes: wire::watching(&record),
                    },
                },
            },
        });
        side.panel = Some(Panel { subject, state: State::Loading, handle: Some(handle) });
    }
    side.changed();
}
pub(super) fn decode(client: &Client, handle: Handle, limits: Limits) -> Watched {
    Watched::Delivered(record(client, handle, limits))
}
fn record(client: &Client, handle: Handle, limits: Limits) -> Option<Box<w::ChangePanel>> {
    let Latest::Family(record) = client.document(handle)?.latest? else { return None };
    if record.family != wire::FORGE || record.version != wire::CLIENT_VERSION {
        return None;
    }
    Some(Box::new(wire::decode_change_panel(&record.bytes, &wire_limits(&limits)?)?))
}
pub(super) fn watched(side: &mut Side, handle: Handle, watched: Watched, out: &mut Queue<Request>) {
    let Some(panel) = &mut side.panel else { return };
    if panel.handle != Some(handle) {
        return;
    }
    side.panel_retry = match &watched {
        Watched::Ended(Why::SubjectEnded) => true,
        Watched::Delivered(_)
        | Watched::Ended(Why::NotServed | Why::Unknown | Why::NotAllowed | Why::SignedOut | Why::Limit) => false,
    };
    if side.panel_retry {
        out.push(Request::PanelEnded { subject: panel.subject });
    }
    let state = match watched {
        Watched::Delivered(Some(record)) if record.task() == panel.subject.task => State::Shown(record),
        Watched::Delivered(_) => {
            panel.handle = None;
            out.push(Request::Client { asked: jig_client::Event::Unwatch { handle } });
            State::Unknown
        }
        // Only SubjectEnded may be refreshed by the root (client/forge.md, 9.2).
        Watched::Ended(Why::Unknown | Why::SubjectEnded) => {
            panel.handle = None;
            State::Unknown
        }
        Watched::Ended(why) => {
            panel.handle = None;
            State::Ended(why)
        }
    };
    if panel.state != state {
        panel.state = state;
        side.changed();
    }
}
