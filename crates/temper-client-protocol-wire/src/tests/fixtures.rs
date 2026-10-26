use crate::{Bounds, v1};
use alloc::boxed::Box;
use skein_lib::List;

pub const BOUNDS: Bounds = Bounds { field_bytes: 16, checks: 2, contexts: 2, refusal_bytes: 64 };

pub fn limits() -> v1::Limits {
    crate::limits(&BOUNDS).expect("test bounds within ceilings")
}

pub fn adopt() -> v1::Adopt {
    v1::Adopt::new(
        &limits(),
        v1::AdoptParts {
            project: 1,
            home: true,
            forge: crate::FORGE,
            owner: Box::from(&b"acme"[..]),
            name: Box::from(&b"shop"[..]),
            prefix: Box::from(&b"temper/"[..]),
            role: v1::Role::Owned,
            landing: Box::from(&b"main"[..]),
            ci: true,
            checks: List::with_capacity(2),
        },
    )
    .expect("default adoption")
}

pub fn adopted() -> v1::Adopted {
    let limits = limits();
    let kinds = v1::Kinds::new(&limits, v1::KindsParts { skein_bools: [true; 9] }).expect("observed capabilities");
    v1::Adopted::new(
        &limits,
        v1::AdoptedParts {
            forge: crate::FORGE,
            repository: 7,
            owner: Box::from(&b"acme"[..]),
            name: Box::from(&b"shop"[..]),
            role: v1::Role::Owned,
            home: true,
            kinds,
            protection: v1::Protection::Unknown,
        },
    )
    .expect("adopted repository")
}

pub fn outcome_rows() -> [v1::Outcome; 20] {
    let limits = limits();
    let original = adopted().into_parts();
    let mut contexts = List::with_capacity(1);
    contexts.push(Box::from(&b"build"[..])).expect("one context");
    let rule = v1::Rule::new(
        &limits,
        v1::RuleParts { branch: Box::from(&b"main"[..]), contexts, approvals: 1, dismiss_stale: true },
    )
    .expect("protection rule");
    let absent = v1::Adopted::new(&limits, v1::AdoptedParts { protection: v1::Protection::Absent, ..original.clone() })
        .expect("no protection");
    let protected = v1::Adopted::new(&limits, v1::AdoptedParts { protection: v1::Protection::Rule(rule), ..original })
        .expect("protection observed");
    let refusal = v1::CoreRefused::new(&limits, v1::CoreRefusedParts { refusal: Box::from(&b"core refusal"[..]) })
        .expect("core refusal");
    [
        v1::Outcome::Adopted(adopted()),
        v1::Outcome::Adopted(absent),
        v1::Outcome::Adopted(protected),
        v1::Outcome::CoreRefused(refusal),
        forge_refused(v1::ForgeReason::NoSuchForge),
        forge_refused(v1::ForgeReason::NotFound),
        forge_refused(v1::ForgeReason::NoAccess),
        forge_refused(v1::ForgeReason::Refused),
        forge_refused(v1::ForgeReason::InFlight),
        forge_refused(v1::ForgeReason::TooLarge),
        forge_refused(v1::ForgeReason::Unreachable),
        forge_refused(v1::ForgeReason::InvalidAnswer),
        forge_refused(v1::ForgeReason::AnotherProject),
        forge_refused(v1::ForgeReason::HomeHeld),
        forge_refused(v1::ForgeReason::PrefixOccupied),
        v1::Outcome::Unknown,
        v1::Outcome::SettingUp,
        set_up_refused(v1::SetUpRefusal::NotOffered),
        set_up_refused(v1::SetUpRefusal::NotUnderTheBox),
        set_up_refused(v1::SetUpRefusal::Busy),
    ]
}

fn forge_refused(reason: v1::ForgeReason) -> v1::Outcome {
    v1::Outcome::ForgeRefused(
        v1::ForgeRefused::new(&limits(), v1::ForgeRefusedParts { reason }).expect("forge refusal"),
    )
}

pub fn pull(address_bytes: u32, limits: &v1::Limits) -> v1::Pull {
    v1::Pull::new(limits, v1::PullParts { number: 23, address: field_bytes(address_bytes) })
        .expect("pull within limits")
}

pub fn queued(place: u32, count: u32, limits: &v1::Limits) -> v1::Queued {
    let mut ahead = List::with_capacity(count);
    for task in 1..=count {
        ahead.push(u64::from(task)).expect("counted tasks");
    }
    v1::Queued::new(limits, v1::QueuedParts { place, ahead }).expect("queue within limits")
}

pub fn gates(count: u32) -> List<v1::Gate> {
    let mut gates = List::with_capacity(count);
    for number in 0..count {
        gates
            .push(
                v1::Gate::new(
                    &limits(),
                    v1::GateParts {
                        number: u64::from(number) + 1,
                        kind: v1::GateKind::Review,
                        blocks: true,
                        verdict: v1::Verdict::Pending,
                    },
                )
                .expect("gate"),
            )
            .expect("counted gates");
    }
    gates
}

pub fn panel_parts() -> v1::ChangePanelParts {
    let limits = limits();
    v1::ChangePanelParts {
        task: 17,
        forge: crate::FORGE,
        owner: b"acme".as_slice().into(),
        name: b"shop".as_slice().into(),
        branch: b"change/17".as_slice().into(),
        landing: b"main".as_slice().into(),
        head: None,
        pull: None,
        step: v1::Step::Producing,
        hold: None,
        runs: None,
        ci: v1::Ci::Unknown,
        gates: gates(0),
        repairs: v1::Used::new(&limits, v1::UsedParts { used: 1, limit: 2 }).expect("repairs"),
        resolutions: v1::Used::new(&limits, v1::UsedParts { used: 3, limit: 4 }).expect("resolutions"),
        updates: v1::Used::new(&limits, v1::UsedParts { used: 5, limit: 6 }).expect("updates"),
    }
}

pub fn panel_rows() -> List<v1::ChangePanel> {
    use v1::{Ci, Hold, Reason, Step};
    let limits = limits();
    let gate = Reason::Gate(v1::GateReason::new(&limits, v1::GateReasonParts { gate: 9 }).expect("gate reason"));
    let mut panels = List::with_capacity(15);
    for (step, hold, ci, [head, pull, runs]) in [
        (Step::Producing, None, Ci::Unknown, [false, false, false]),
        (Step::Opening, Some(Hold::Stalled), Ci::Pending, [true, false, false]),
        (Step::Recreating, Some(Hold::Repairs), Ci::Passed, [false, true, false]),
        (Step::Reopening, Some(Hold::Resolutions), Ci::Failed, [false, false, true]),
        (Step::Checking, Some(Hold::Updates), Ci::Unknown, [true, true, false]),
        (Step::Gating, Some(Hold::Moved), Ci::Pending, [true, false, true]),
        (Step::Queued(queued(1, 2, &limits)), Some(Hold::Missing), Ci::Passed, [false, true, true]),
        (Step::First, Some(Hold::Closed), Ci::Failed, [true, true, true]),
        (Step::Updating, Some(Hold::Retargeted), Ci::Unknown, [true, true, true]),
        (Step::Resolving, Some(Hold::Failed), Ci::Pending, [true, true, true]),
        (
            Step::Repairing(v1::Repairing::new(&limits, v1::RepairingParts { reason: Reason::Ci }).expect("CI repair")),
            Some(Hold::Rejected),
            Ci::Failed,
            [true, true, true],
        ),
        (
            Step::Repairing(v1::Repairing::new(&limits, v1::RepairingParts { reason: gate }).expect("gate repair")),
            None,
            Ci::Passed,
            [true, true, true],
        ),
        (
            Step::Repairing(
                v1::Repairing::new(&limits, v1::RepairingParts { reason: Reason::Semantic }).expect("semantic repair"),
            ),
            None,
            Ci::Passed,
            [true, true, true],
        ),
        (Step::Landing, None, Ci::Passed, [true, true, true]),
        (
            Step::Landed(v1::Landed::new(&limits, v1::LandedParts { merge: [0x47; 32] }).expect("merge")),
            None,
            Ci::Passed,
            [true, true, true],
        ),
    ] {
        let mut parts = panel_parts();
        parts.step = step;
        parts.hold = hold;
        parts.ci = ci;
        if head {
            parts.head = Some([0x39; 32]);
        }
        if pull {
            parts.pull = Some(self::pull(32, &limits));
        }
        if runs {
            parts.runs = Some(29);
        }
        parts.gates = List::with_capacity(12);
        for kind in [v1::GateKind::Review, v1::GateKind::Approval, v1::GateKind::Check] {
            for verdict in [v1::Verdict::None, v1::Verdict::Pending, v1::Verdict::Approved, v1::Verdict::Changes] {
                // Both blocking and advisory gates, every kind and verdict.
                parts
                    .gates
                    .push(
                        v1::Gate::new(
                            &limits,
                            v1::GateParts {
                                number: u64::from(parts.gates.len()) + 1,
                                kind: kind.clone(),
                                blocks: verdict == v1::Verdict::Approved,
                                verdict,
                            },
                        )
                        .expect("gate"),
                    )
                    .expect("twelve gates");
            }
        }
        panels.push(v1::ChangePanel::new(&limits, parts).expect("panel row")).expect("fifteen rows");
    }
    panels
}

pub fn panel_at_bound(field: PanelBound, count: u32) -> Result<v1::ChangePanel, v1::Problem> {
    let limits = limits();
    let mut parts = panel_parts();
    match field {
        PanelBound::Address => {
            parts.pull = Some(v1::Pull::new(&limits, v1::PullParts { number: 23, address: field_bytes(count) })?);
        }
        PanelBound::Ahead => {
            let mut ahead = List::with_capacity(count);
            for task in 1..=count {
                ahead.push(u64::from(task)).expect("counted tasks");
            }
            parts.step = v1::Step::Queued(v1::Queued::new(&limits, v1::QueuedParts { place: 1, ahead })?);
        }
        PanelBound::Gates => parts.gates = gates(count),
        PanelBound::Owner => {
            parts.owner = field_bytes(count);
        }
        PanelBound::Name => {
            parts.name = field_bytes(count);
        }
        PanelBound::Branch => {
            parts.branch = field_bytes(count);
        }
        PanelBound::Landing => {
            parts.landing = field_bytes(count);
        }
    }
    v1::ChangePanel::new(&limits, parts)
}

#[derive(Clone, Copy, Debug)]
pub enum PanelBound {
    Address,
    Ahead,
    Gates,
    Owner,
    Name,
    Branch,
    Landing,
}

pub fn field_bytes(count: u32) -> Box<[u8]> {
    let mut bytes = skein_lib::bytes::zeroed(usize::try_from(count).expect("field length fits usize"));
    bytes.fill(b'a');
    bytes
}

pub fn trailing(bytes: &[u8]) -> Box<[u8]> {
    let mut writer = skein_lib::Writer::new(bytes.len().checked_add(1).expect("one trailing byte fits"));
    writer.put(bytes).expect("record room");
    writer.put(&[0]).expect("trailing byte room");
    writer.finish()
}

fn set_up_refused(why: v1::SetUpRefusal) -> v1::Outcome {
    v1::Outcome::SetUpRefused(v1::SetUpRefused::new(&limits(), v1::SetUpRefusedParts { why }).expect("setup refusal"))
}
