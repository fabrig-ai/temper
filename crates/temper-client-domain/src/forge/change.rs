//! A change panel's drawing facts (client/README.md, 5.10).
use super::{
    panel,
    words::{self, ChangeWord as W},
};
use alloc::boxed::Box;
use skein_lib::bytes::copy_of;
use skein_ui::phrases::Hole;
use temper_client_protocol_wire::v1 as w;

/// Maximum supplied pull address; the view prices its label beside it.
pub const ADDRESS_BYTES: u32 = temper_client_protocol_wire::ADDRESS_BYTES;
pub type Said = (W, Box<[Hole]>);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slots {
    pub current: Option<u8>,
    pub done: u8,
}
#[must_use]
pub fn slot(step: &w::Step) -> Slots {
    let current = match step {
        w::Step::Producing => 0,
        w::Step::Opening | w::Step::Recreating | w::Step::Reopening => 1,
        w::Step::Checking => 2,
        w::Step::Gating => 3,
        w::Step::Ready(_)
        | w::Step::Preparing(_)
        | w::Step::Queued(_)
        | w::Step::First
        | w::Step::Updating
        | w::Step::Resolving => 4,
        w::Step::Repairing(repair) => match repair.reason() {
            w::Reason::Ci => 2,
            w::Reason::Gate(_) => 3,
            w::Reason::Semantic => 4,
        },
        w::Step::Landing => 5,
        w::Step::Landed(_) => return Slots { current: None, done: 6 },
    };
    Slots { current: Some(current), done: current }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ordinal {
    St,
    Nd,
    Rd,
    Th,
}
#[must_use]
pub const fn ordinal(n: u32) -> Ordinal {
    match n % 100 {
        11..=13 => Ordinal::Th,
        _ => match n % 10 {
            1 => Ordinal::St,
            2 => Ordinal::Nd,
            3 => Ordinal::Rd,
            _ => Ordinal::Th,
        },
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Queue<'a> {
    pub place: u32,
    pub ahead: &'a [u64],
    pub unnamed: u32,
}
#[must_use]
pub fn queue(step: &w::Step) -> Option<Queue<'_>> {
    match step {
        w::Step::Queued(q) => Some(Queue {
            place: q.place(),
            ahead: q.ahead().as_slice(),
            unnamed: q.place().saturating_sub(1).saturating_sub(q.ahead().len()),
        }),
        w::Step::First => Some(Queue { place: 1, ahead: &[], unnamed: 0 }),
        w::Step::Ready(_)
        | w::Step::Preparing(_)
        | w::Step::Producing
        | w::Step::Opening
        | w::Step::Recreating
        | w::Step::Reopening
        | w::Step::Checking
        | w::Step::Gating
        | w::Step::Updating
        | w::Step::Resolving
        | w::Step::Repairing(_)
        | w::Step::Landing
        | w::Step::Landed(_) => None,
    }
}
#[must_use]
pub const fn ordinal_word(n: u32, bar: bool) -> W {
    match (ordinal(n), bar) {
        (Ordinal::St, true) => W::QueuedSt,
        (Ordinal::Nd, true) => W::QueuedNd,
        (Ordinal::Rd, true) => W::QueuedRd,
        (Ordinal::Th, true) => W::QueuedTh,
        (Ordinal::St, false) => W::QueueSt,
        (Ordinal::Nd, false) => W::QueueNd,
        (Ordinal::Rd, false) => W::QueueRd,
        (Ordinal::Th, false) => W::QueueTh,
    }
}
#[must_use]
pub fn line(panel: &w::ChangePanel) -> Said {
    let word = match panel.step() {
        w::Step::Ready(ready) => match ready.landing() {
            Some(task) => return (W::ReadyLine, Box::new([Hole::Number(*task)])),
            None => W::ReadyAloneLine,
        },
        w::Step::Preparing(_) => return landing_line(W::PrepareLine, panel),
        w::Step::Producing => W::ProduceLine,
        w::Step::Opening => W::OpenLine,
        w::Step::Recreating => W::RecreateLine,
        w::Step::Reopening => W::ReopenLine,
        w::Step::Checking => W::CheckLine,
        w::Step::Gating => W::GateLine,
        w::Step::Queued(q) => {
            return (
                ordinal_word(q.place(), false),
                Box::new([Hole::Number(u64::from(q.place())), Hole::Name(copy_of(panel.landing()))]),
            );
        }
        w::Step::First => return landing_line(W::FirstLine, panel),
        w::Step::Updating => return landing_line(W::UpdateLine, panel),
        w::Step::Resolving => return landing_line(W::ResolveLine, panel),
        w::Step::Repairing(repair) => match repair.reason() {
            w::Reason::Ci => W::RepairCi,
            w::Reason::Gate(gate) => return (W::RepairGate, Box::new([Hole::Number(gate.gate())])),
            w::Reason::Semantic => return landing_line(W::RepairSemantic, panel),
        },
        w::Step::Landing => W::LandingLine,
        w::Step::Landed(landed) => {
            let mut digits = [0; 7];
            for (i, digit) in digits.iter_mut().enumerate() {
                let byte = *landed.merge().get(i / 2).expect("seven nibbles in 32 bytes");
                let nibble = if i % 2 == 0 { byte >> 4_u8 } else { byte & 15 };
                *digit = *b"0123456789abcdef".get(usize::from(nibble)).expect("hex nibble");
            }
            return (W::LandedLine, Box::new([Hole::Name(copy_of(&digits))]));
        }
    };
    (word, Box::new([]))
}
fn landing_line(word: W, panel: &w::ChangePanel) -> Said {
    (word, Box::new([Hole::Name(copy_of(panel.landing()))]))
}
#[must_use]
pub const fn held_step(step: &w::Step) -> W {
    match step {
        w::Step::Ready(_) => W::HeldReady,
        w::Step::Preparing(_) => W::HeldPreparing,
        w::Step::Producing => W::HeldProducing,
        w::Step::Opening => W::HeldOpening,
        w::Step::Recreating => W::HeldRecreating,
        w::Step::Reopening => W::HeldReopening,
        w::Step::Checking => W::HeldChecking,
        w::Step::Gating => W::HeldGating,
        w::Step::Queued(_) => W::HeldQueued,
        w::Step::First => W::HeldFirst,
        w::Step::Updating => W::HeldUpdating,
        w::Step::Resolving => W::HeldResolving,
        w::Step::Repairing(_) => W::HeldRepairing,
        w::Step::Landing => W::HeldLanding,
        w::Step::Landed(_) => W::HeldLanded,
    }
}
#[must_use]
pub const fn hold(hold: &w::Hold) -> W {
    match hold {
        w::Hold::Stalled => W::Stalled,
        w::Hold::Repairs => W::RepairsSpent,
        w::Hold::Resolutions => W::ResolutionsSpent,
        w::Hold::Updates => W::UpdatesSpent,
        w::Hold::Moved => W::Moved,
        w::Hold::Missing => W::Missing,
        w::Hold::Closed => W::Closed,
        w::Hold::Retargeted => W::Retargeted,
        w::Hold::Failed => W::FailedHold,
        w::Hold::Rejected => W::Rejected,
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gate {
    pub name: W,
    pub number: u64,
    pub verdict: W,
}
#[must_use]
pub fn gate(gate: &w::Gate) -> Gate {
    let name = match (gate.kind(), gate.blocks()) {
        (w::GateKind::Review, true) => W::ReviewBlocking,
        (w::GateKind::Review, false) => W::ReviewAdvisory,
        (w::GateKind::Approval, true) => W::ApprovalBlocking,
        (w::GateKind::Approval, false) => W::ApprovalAdvisory,
        (w::GateKind::Check, true) => W::CheckBlocking,
        (w::GateKind::Check, false) => W::CheckAdvisory,
    };
    let verdict = match gate.verdict() {
        w::Verdict::None => W::NotYet,
        w::Verdict::Pending => W::Waiting,
        w::Verdict::Approved => W::Approved,
        w::Verdict::Changes => W::Changes,
    };
    Gate { name, number: gate.number(), verdict }
}
/// A page owns only the panel for its own subject.
#[must_use]
pub fn for_task(panel: Option<&panel::Panel>, task: u64) -> Option<&panel::Panel> {
    match panel {
        Some(panel) if panel.subject.task == task => Some(panel),
        Some(_) | None => None,
    }
}
/// A borrowed drawing projection, never retained by the page.
#[derive(Clone, Copy, Debug)]
pub struct Change<'a> {
    panel: &'a w::ChangePanel,
}
#[must_use]
pub fn shown(state: &panel::State) -> Option<Change<'_>> {
    match state {
        panel::State::Shown(panel) => Some(Change { panel }),
        panel::State::Loading | panel::State::Unknown | panel::State::Ended(_) => None,
    }
}
#[must_use]
pub const fn unavailable(state: &panel::State) -> Option<W> {
    match state {
        panel::State::Loading => Some(W::Loading),
        panel::State::Unknown => Some(W::Unknown),
        panel::State::Ended(_) => Some(W::Ended),
        panel::State::Shown(_) => None,
    }
}
impl<'a> Change<'a> {
    #[must_use]
    pub fn forge(self) -> u16 {
        self.panel.forge()
    }

    #[must_use]
    pub fn slots(self) -> Slots {
        slot(self.panel.step())
    }
    #[must_use]
    pub fn queue(self) -> Option<Queue<'a>> {
        queue(self.panel.step())
    }
    #[must_use]
    pub fn line(self) -> Said {
        line(self.panel)
    }
    #[must_use]
    pub fn hold(self) -> Option<(W, W)> {
        Some((held_step(self.panel.step()), hold(self.panel.hold().as_ref()?)))
    }
    #[must_use]
    pub fn runs(self) -> Option<u64> {
        *self.panel.runs()
    }
    #[must_use]
    pub fn pull(self) -> Said {
        match self.panel.pull() {
            Some(pull) => (W::Pull, Box::new([Hole::Number(pull.number())])),
            None => (W::NoPull, Box::new([])),
        }
    }
    #[must_use]
    pub fn repository(self) -> Said {
        (
            W::Repository,
            Box::new([
                Hole::Name(words::full_name(self.panel.owner(), self.panel.name())),
                Hole::Name(copy_of(self.panel.branch())),
                Hole::Name(copy_of(self.panel.landing())),
            ]),
        )
    }
    #[must_use]
    pub fn address(self) -> Option<&'a [u8]> {
        let address = self.panel.pull().as_ref()?.address();
        if address.is_empty() { None } else { Some(address) }
    }
    /// A landed change's CI is not read again: unknown there is a restart's
    /// forgetting (domain/forge.md, 8.8), not CI still to come.
    #[must_use]
    pub fn ci(self) -> W {
        match self.panel.ci() {
            w::Ci::Unknown => match self.panel.step() {
                w::Step::Landed(_) => W::NotRead,
                w::Step::Ready(_)
                | w::Step::Preparing(_)
                | w::Step::Producing
                | w::Step::Opening
                | w::Step::Recreating
                | w::Step::Reopening
                | w::Step::Checking
                | w::Step::Gating
                | w::Step::Queued(_)
                | w::Step::First
                | w::Step::Updating
                | w::Step::Resolving
                | w::Step::Repairing(_)
                | w::Step::Landing => W::NotYet,
            },
            w::Ci::Pending => W::Running,
            w::Ci::Passed => W::Passed,
            w::Ci::Failed => W::Failed,
        }
    }
    #[must_use]
    pub fn gates(self) -> u32 {
        self.panel.gates().len()
    }
    #[must_use]
    pub fn gate(self, index: u32) -> Option<Gate> {
        Some(gate(self.panel.gates().get(index)?))
    }
    #[must_use]
    pub fn bounds(self) -> [Said; 3] {
        let mut result: [Said; 3] =
            [(W::Repairs, Box::new([])), (W::Resolutions, Box::new([])), (W::Updates, Box::new([]))];
        for ((_, holes), used) in
            result.iter_mut().zip([self.panel.repairs(), self.panel.resolutions(), self.panel.updates()])
        {
            *holes = Box::new([Hole::Number(u64::from(used.used())), Hole::Number(u64::from(used.limit()))]);
        }
        result
    }
}
