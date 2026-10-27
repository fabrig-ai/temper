use super::fixtures::panel as p;
use crate::forge::{
    change::{self, Ordinal as O, Slots},
    words::ChangeWord as W,
};
use skein_lib::List;
use skein_ui::phrases::Hole;
use temper_client_protocol_wire::v1 as w;
fn repair(reason: w::Reason) -> w::Step {
    w::Step::Repairing(w::Repairing::new(&w::CEILINGS, w::RepairingParts { reason }).unwrap())
}
#[test]
fn each_step_supplies_its_slot_line_and_held_word() {
    let cases = [
        (
            w::Step::Ready(w::InLanding::new(&w::CEILINGS, w::InLandingParts { landing: Some(61) }).unwrap()),
            Some(4),
            W::ReadyLine,
            W::HeldReady,
        ),
        (
            w::Step::Ready(w::InLanding::new(&w::CEILINGS, w::InLandingParts { landing: None }).unwrap()),
            Some(4),
            W::ReadyAloneLine,
            W::HeldReady,
        ),
        (
            w::Step::Preparing(w::InLanding::new(&w::CEILINGS, w::InLandingParts { landing: Some(61) }).unwrap()),
            Some(4),
            W::PrepareLine,
            W::HeldPreparing,
        ),
        (w::Step::Producing, Some(0), W::ProduceLine, W::HeldProducing),
        (w::Step::Opening, Some(1), W::OpenLine, W::HeldOpening),
        (w::Step::Recreating, Some(1), W::RecreateLine, W::HeldRecreating),
        (w::Step::Reopening, Some(1), W::ReopenLine, W::HeldReopening),
        (w::Step::Checking, Some(2), W::CheckLine, W::HeldChecking),
        (w::Step::Gating, Some(3), W::GateLine, W::HeldGating),
        (p::queued(), Some(4), W::QueueTh, W::HeldQueued),
        (w::Step::First, Some(4), W::FirstLine, W::HeldFirst),
        (w::Step::Updating, Some(4), W::UpdateLine, W::HeldUpdating),
        (w::Step::Resolving, Some(4), W::ResolveLine, W::HeldResolving),
        (repair(w::Reason::Ci), Some(2), W::RepairCi, W::HeldRepairing),
        (
            repair(w::Reason::Gate(w::GateReason::new(&w::CEILINGS, w::GateReasonParts { gate: 123 }).unwrap())),
            Some(3),
            W::RepairGate,
            W::HeldRepairing,
        ),
        (repair(w::Reason::Semantic), Some(4), W::RepairSemantic, W::HeldRepairing),
        (w::Step::Landing, Some(5), W::LandingLine, W::HeldLanding),
        (
            w::Step::Landed(w::Landed::new(&w::CEILINGS, w::LandedParts { merge: [0xab; 32] }).unwrap()),
            None,
            W::LandedLine,
            W::HeldLanded,
        ),
    ];
    for (step, current, word, held) in cases {
        assert_eq!(change::slot(&step), Slots { current, done: current.unwrap_or(6) });
        assert_eq!(change::held_step(&step), held);
        let panel = p::panel(17, step);
        let (line, holes) = change::line(&panel);
        assert_eq!(line, word);
        let expected: Box<[Hole]> = if word == W::QueueTh {
            Box::new([Hole::Number(65), Hole::Name(Box::from(panel.landing()))])
        } else if word == W::PrepareLine
            || word == W::FirstLine
            || word == W::UpdateLine
            || word == W::ResolveLine
            || word == W::RepairSemantic
        {
            Box::new([Hole::Name(Box::from(panel.landing()))])
        } else if word == W::ReadyLine {
            Box::new([Hole::Number(61)])
        } else if word == W::RepairGate {
            Box::new([Hole::Number(123)])
        } else if word == W::LandedLine {
            Box::new([Hole::Name(Box::from(*b"abababa"))])
        } else {
            Box::new([])
        };
        assert_eq!(holes.len(), expected.len());
        for (actual, expected) in holes.iter().zip(expected.iter()) {
            match (actual, expected) {
                (Hole::Number(actual), Hole::Number(expected)) => assert_eq!(actual, expected),
                (Hole::Name(actual), Hole::Name(expected)) => assert_eq!(actual, expected),
                _ => panic!("line hole kind"),
            }
        }
    }
}
#[test]
fn holds_supply_each_reason() {
    for (hold, word) in [
        (w::Hold::Stalled, W::Stalled),
        (w::Hold::Repairs, W::RepairsSpent),
        (w::Hold::Resolutions, W::ResolutionsSpent),
        (w::Hold::Updates, W::UpdatesSpent),
        (w::Hold::Moved, W::Moved),
        (w::Hold::Missing, W::Missing),
        (w::Hold::Closed, W::Closed),
        (w::Hold::Retargeted, W::Retargeted),
        (w::Hold::Failed, W::FailedHold),
        (w::Hold::Rejected, W::Rejected),
    ] {
        assert_eq!(change::hold(&hold), word);
    }
}
#[test]
fn queue_names_stay_nearest_first_and_unnamed_counts_saturate() {
    for (place, names, unnamed) in
        [(1, &[][..], 0), (3, &[7, 9][..], 0), (4, &[7, 9][..], 1), (3, &[][..], 2), (0, &[7, 9][..], 0)]
    {
        let mut ahead = List::with_capacity(2);
        for name in names {
            ahead.push(*name).unwrap();
        }
        let step = w::Step::Queued(w::Queued::new(&w::CEILINGS, w::QueuedParts { place, ahead }).unwrap());
        let queue = change::queue(&step).unwrap();
        assert_eq!((queue.place, queue.ahead, queue.unnamed), (place, names, unnamed));
    }
    let first = change::queue(&w::Step::First).unwrap();
    assert_eq!((first.place, first.ahead, first.unnamed), (1, &[][..], 0));
    assert!(change::queue(&w::Step::Producing).is_none());
}
#[test]
fn ordinals_keep_the_teens_and_the_last_digit() {
    for (number, expected) in [
        (1, O::St),
        (2, O::Nd),
        (3, O::Rd),
        (4, O::Th),
        (11, O::Th),
        (12, O::Th),
        (13, O::Th),
        (21, O::St),
        (22, O::Nd),
        (101, O::St),
        (111, O::Th),
        (112, O::Th),
    ] {
        assert_eq!(change::ordinal(number), expected);
    }
}
#[test]
fn merge_digits_are_lower_case_and_stop_at_seven_nibbles() {
    let mut merge = [0; 32];
    for (byte, value) in merge.iter_mut().zip([0x01, 0x23, 0xab, 0xcd]) {
        *byte = value;
    }
    let panel = p::panel(17, w::Step::Landed(w::Landed::new(&w::CEILINGS, w::LandedParts { merge }).unwrap()));
    let (_, holes) = change::line(&panel);
    let [Hole::Name(digits)] = holes.as_ref() else { panic!("merge digits") };
    assert_eq!(digits.as_ref(), b"0123abc");
}
