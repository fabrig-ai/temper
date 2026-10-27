//! The connector's whole change document (domain/forge.md, 8.8).
use crate::{ChangeRow, Domain, Limits};
use alloc::boxed::Box;
use jig_domain_people::ApplicationScope;
use skein_lib::{List, Wall};
use temper_engine_domain_forge_change as change;

/// Tasks ahead of a queued change (client/README.md, 4.6).
pub const PANEL_AHEAD: u32 = 64;
/// Gates in a change document (client/README.md, 4.6).
pub const PANEL_GATES: u32 = 64;
/// Bytes in each repository or branch name (client/README.md, 4.6).
pub const PANEL_NAME_BYTES: u32 = 1024;
/// Subjects marked together (domain/root.md, section 10).
pub const MARKED: u32 = 32;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PanelStep {
    Ended,
    Producing,
    Opening,
    Recreating,
    Reopening,
    Checking,
    Gating,
    Queued { place: u32, ahead: Box<[u64]> },
    First,
    Updating,
    Resolving,
    Repairing(change::Repair),
    Landing,
    Landed { merge: [u8; 32] },
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PanelGate {
    pub number: u64,
    pub kind: change::GateKind,
    pub blocking: bool,
    pub verdict: change::Status,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PanelUsed {
    pub used: u32,
    pub limit: u32,
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Panel {
    pub task: u64,
    pub forge: u16,
    pub owner: Box<[u8]>,
    pub name: Box<[u8]>,
    pub branch: Box<[u8]>,
    pub landing: Box<[u8]>,
    pub head: Option<[u8; 32]>,
    pub pull: Option<u64>,
    pub step: PanelStep,
    pub hold: Option<change::Hold>,
    pub runs: Option<u64>,
    pub ci: change::Status,
    pub gates: Box<[PanelGate]>,
    pub repairs: PanelUsed,
    pub resolutions: PanelUsed,
    pub updates: PanelUsed,
}
impl Panel {
    #[must_use]
    pub fn within(&self) -> bool {
        let names = usize::try_from(PANEL_NAME_BYTES).expect("u32 fits usize");
        self.owner.len() <= names
            && self.name.len() <= names
            && self.branch.len() <= names
            && self.landing.len() <= names
            && self.gates.len() <= usize::try_from(PANEL_GATES).expect("u32 fits usize")
            && match &self.step {
                PanelStep::Queued { place, ahead } => {
                    *place > 0 && ahead.len() <= usize::try_from(PANEL_AHEAD).expect("u32 fits usize")
                }
                PanelStep::Producing
                | PanelStep::Opening
                | PanelStep::Recreating
                | PanelStep::Reopening
                | PanelStep::Checking
                | PanelStep::Gating
                | PanelStep::First
                | PanelStep::Updating
                | PanelStep::Resolving
                | PanelStep::Repairing(_)
                | PanelStep::Landing
                | PanelStep::Landed { .. }
                | PanelStep::Ended => true,
            }
    }
}

impl Domain {
    /// Purely read a retained change in the project that holds its repository.
    #[must_use]
    #[expect(clippy::manual_map, reason = "the strict subset uses explicit matches")]
    pub fn panel(&self, task: u64, scope: ApplicationScope, now: Wall, limits: &Limits) -> Option<Panel> {
        let ApplicationScope::Project(project) = scope else { return None };
        let row = self.changes.get(&task)?;
        let repository = self.repositories.get(&row.repository)?;
        if repository.project != project {
            return None;
        }
        let mut state = &row.change.state;
        let mut hold = None;
        while let change::State::Held { was, why } = state {
            hold = Some(*why);
            state = was;
        }
        let step = match state {
            change::State::Ended => PanelStep::Ended,
            change::State::Producing { .. } => PanelStep::Producing,
            change::State::Opening { .. } => PanelStep::Opening,
            change::State::Recreating { .. } => PanelStep::Recreating,
            change::State::Reopening { .. } => PanelStep::Reopening,
            change::State::Checking { .. } => PanelStep::Checking,
            change::State::Gating { .. } | change::State::CancellingGates { .. } => PanelStep::Gating,
            // landing.md 3: the top's task
            change::State::Ready { .. } | change::State::Preparing { .. } => return None,
            change::State::Queued { .. } => {
                let (place, ahead) = queue_place(self, row, now, limits);
                PanelStep::Queued { place, ahead }
            }
            change::State::First { .. } => PanelStep::First,
            change::State::Updating { .. } => PanelStep::Updating,
            change::State::Resolving { .. } => PanelStep::Resolving,
            change::State::Repairing { why, .. } => PanelStep::Repairing(*why),
            change::State::Landing { .. } => PanelStep::Landing,
            change::State::Landed { merge } => PanelStep::Landed { merge: *merge },
            change::State::Held { .. } => unreachable!("held steps unwrapped"),
        };
        let head = row.change.last_head;
        let mut gates = List::with_capacity(PANEL_GATES);
        for gate in &row.change.gates {
            let mut verdict = change::Status::Unknown;
            for report in &row.verdicts {
                if gate.number == report.number
                    && head.is_some()
                    && (Some(report.head) == head
                        || (gate.freshness == change::Freshness::Clean && row.change.clean.contains(&report.head)))
                {
                    verdict = worse(verdict, report.status);
                }
            }
            // A gate whose review runs at the current head is pending until it reports.
            if let Some((_, change::Delegate::Gate { number, head: asked })) = row.delegate
                && number == gate.number
                && Some(asked) == head
            {
                verdict = worse(verdict, change::Status::Pending);
            }
            gates
                .push(PanelGate { number: gate.number, kind: gate.kind, blocking: gate.blocking, verdict })
                .expect("admitted panel gates");
        }
        let read = if row.terminal() { row.ci.as_ref() } else { self.panel_ci.get(&task) };
        let ci = match read {
            Some(read) if Some(read.head) == head => read.status,
            Some(_) | None => change::Status::Unknown,
        };
        let panel = Panel {
            task,
            forge: row.repository.forge,
            owner: repository.owner.clone(),
            name: repository.name.clone(),
            branch: row.branch.clone(),
            landing: row.base.clone(),
            head,
            pull: row.pull,
            step,
            hold,
            runs: match row.delegate {
                Some((task, _)) => Some(task),
                None => None,
            },
            ci,
            gates: gates.into_boxed(),
            repairs: PanelUsed { used: row.change.repairs, limit: limits.change_policy.repairs },
            resolutions: PanelUsed { used: row.change.resolutions, limit: limits.change_policy.resolutions },
            updates: PanelUsed { used: row.change.updates, limit: limits.change_policy.updates },
        };
        assert!(panel.within(), "admitted change fits the panel");
        Some(panel)
    }
}
fn worse(a: change::Status, b: change::Status) -> change::Status {
    use change::Status::{Failed, Passed, Pending, Unknown};
    match (a, b) {
        (Failed, _) | (_, Failed) => Failed,
        (Pending, _) | (_, Pending) => Pending,
        (Passed, _) | (_, Passed) => Passed,
        (Unknown, Unknown) => Unknown,
    }
}

pub(crate) fn ready(row: &ChangeRow) -> Option<change::Ready> {
    let (change::State::Queued { ready_since: since, .. } | change::State::First { ready_since: since, .. }) =
        row.change.state
    else {
        return None;
    };
    Some(change::Ready { task: row.task, priority: row.priority, since })
}
fn queue_place(d: &Domain, row: &ChangeRow, now: Wall, limits: &Limits) -> (u32, Box<[u64]>) {
    let (ready, repair) = crate::domain::queue_ready(d, row, limits);
    let mut own = None;
    for entry in &ready {
        if entry.task == row.task {
            own = Some(*entry);
        }
    }
    let own = match own {
        Some(own) => own,
        None => change::Ready {
            task: row.task,
            priority: row.priority,
            since: row.change.ready_since.unwrap_or(row.change.since),
        },
    };
    let mut place = 1_u32;
    for entry in &ready {
        if queue_before(entry, &own, repair, limits, now) {
            place = place.checked_add(1).expect("bounded queue place");
        }
    }
    let mut ahead = List::with_capacity(PANEL_AHEAD);
    // Repeated selection retains only the nearest predecessors, without sorting a second queue.
    let mut cursor = own;
    while ahead.room() > 0 {
        let mut nearest = None;
        for entry in &ready {
            if queue_before(entry, &cursor, repair, limits, now) {
                nearest = match nearest {
                    Some(current) if queue_before(entry, &current, repair, limits, now) => Some(current),
                    Some(_) | None => Some(*entry),
                };
            }
        }
        let Some(entry) = nearest else { break };
        if ahead.room() > 0 {
            ahead.push(entry.task).expect("nearest ahead room");
        }
        cursor = entry;
    }
    (place, ahead.into_boxed())
}
pub(crate) fn queue_before(
    a: &change::Ready,
    b: &change::Ready,
    repair: Option<u64>,
    limits: &Limits,
    now: Wall,
) -> bool {
    if Some(a.task) == repair {
        return a.task != b.task;
    }
    if Some(b.task) == repair {
        return false;
    }
    change::Ready::before(a, b, limits.queue_window, now)
}
