//! Concrete integer JSON for the client contract (contract.md, 5).
use skein_json::writer::Encoder;
pub(super) fn encode_hold_category(
    value: &jig_client::contract::HoldCategory,
    out: &mut Encoder,
    budget: &mut super::Cut,
) {
    match value {
        jig_client::contract::HoldCategory::Exceeded(exceeded) => {
            out.object_start();
            out.key(b"exceeded");
            out.object_start();
            out.key(b"bound");
            out.string(match exceeded.bound {
                jig_client::contract::Bound::Reasoning => b"reasoning",
                jig_client::contract::Bound::Metadata => b"metadata",
                jig_client::contract::Bound::Items => b"items",
                jig_client::contract::Bound::Answer => b"answer",
                jig_client::contract::Bound::Tools => b"tools",
                jig_client::contract::Bound::HistoryItems => b"history_items",
                jig_client::contract::Bound::History => b"history",
                jig_client::contract::Bound::Window => b"window",
                jig_client::contract::Bound::Output => b"output",
                jig_client::contract::Bound::HostPart => b"host_part",
            });
            out.key(b"value");
            out.unsigned(exceeded.value);
            out.object_end();
            out.object_end();
        }
        jig_client::contract::HoldCategory::Failures { class } => {
            out.object_start();
            out.key(b"failures");
            out.object_start();
            out.key(b"class");
            encode_hold_failure(*(class), out, budget);
            out.object_end();
            out.object_end();
        }
        jig_client::contract::HoldCategory::Circle => out.string(b"circle"),
        jig_client::contract::HoldCategory::Stalled => out.string(b"stalled"),
        jig_client::contract::HoldCategory::Dependency { task } => {
            out.object_start();
            out.key(b"dependency");
            out.object_start();
            out.key(b"task");
            out.unsigned(*(task));
            out.object_end();
            out.object_end();
        }
        jig_client::contract::HoldCategory::Stopped => out.string(b"stopped"),
        jig_client::contract::HoldCategory::Placed { hold } => {
            out.object_start();
            out.key(b"placed");
            out.object_start();
            out.key(b"hold");
            out.unsigned(*(hold));
            out.object_end();
            out.object_end();
        }
        jig_client::contract::HoldCategory::Drift => out.string(b"drift"),
        jig_client::contract::HoldCategory::Effects => out.string(b"effects"),
        jig_client::contract::HoldCategory::EffectFailed => out.string(b"effect_failed"),
        jig_client::contract::HoldCategory::Uncertain { entry } => {
            out.object_start();
            out.key(b"uncertain");
            out.object_start();
            out.key(b"entry");
            out.unsigned(*(entry));
            out.object_end();
            out.object_end();
        }
        jig_client::contract::HoldCategory::Budget => out.string(b"budget"),
        jig_client::contract::HoldCategory::Deadline => out.string(b"deadline"),
        jig_client::contract::HoldCategory::HoldsWaited => out.string(b"holds_waited"),
        jig_client::contract::HoldCategory::ResourceUnavailable => out.string(b"resource_unavailable"),
        jig_client::contract::HoldCategory::ResourceAccess => out.string(b"resource_access"),
        jig_client::contract::HoldCategory::PoolLost { pool } => {
            out.object_start();
            out.key(b"pool_lost");
            out.object_start();
            out.key(b"pool");
            out.unsigned(*(pool));
            out.object_end();
            out.object_end();
        }
        jig_client::contract::HoldCategory::Procedure => out.string(b"procedure"),
    }
}

pub(super) fn encode_hold_failure(
    value: jig_client::contract::HoldFailure,
    out: &mut Encoder,
    _budget: &mut super::Cut,
) {
    let value = &value;
    match value {
        jig_client::contract::HoldFailure::Transient => out.string(b"transient"),
        jig_client::contract::HoldFailure::Permanent => out.string(b"permanent"),
        jig_client::contract::HoldFailure::Run => out.string(b"run"),
        jig_client::contract::HoldFailure::Agent => out.string(b"agent"),
        jig_client::contract::HoldFailure::Lost => out.string(b"lost"),
        jig_client::contract::HoldFailure::Invalid => out.string(b"invalid"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jig_client::contract::{Bound, Exceeded, HoldCategory};

    #[test]
    fn exceeded_holds_name_the_bound_and_its_numeric_value() {
        for (bound, expected) in [
            (Bound::Reasoning, br#"{"exceeded":{"bound":"reasoning","value":2048}}"#.as_slice()),
            (Bound::Metadata, br#"{"exceeded":{"bound":"metadata","value":2048}}"#.as_slice()),
            (Bound::Items, br#"{"exceeded":{"bound":"items","value":2048}}"#.as_slice()),
            (Bound::Answer, br#"{"exceeded":{"bound":"answer","value":2048}}"#.as_slice()),
            (Bound::Tools, br#"{"exceeded":{"bound":"tools","value":2048}}"#.as_slice()),
            (Bound::HistoryItems, br#"{"exceeded":{"bound":"history_items","value":2048}}"#.as_slice()),
            (Bound::History, br#"{"exceeded":{"bound":"history","value":2048}}"#.as_slice()),
            (Bound::Window, br#"{"exceeded":{"bound":"window","value":2048}}"#.as_slice()),
            (Bound::Output, br#"{"exceeded":{"bound":"output","value":2048}}"#.as_slice()),
            (Bound::HostPart, br#"{"exceeded":{"bound":"host_part","value":2048}}"#.as_slice()),
        ] {
            let category = HoldCategory::Exceeded(Exceeded { bound, value: 2048 });
            let limits = skein_json::writer::Limits { depth: 3, length: 256 };
            let mut budget = super::super::Cut { room: 0, left: 0, turn: None, subject: false, digest: None };
            let mut measure = Encoder::measure(&limits);
            encode_hold_category(&category, &mut measure, &mut budget);
            let mut out = Encoder::write(measure.measured().expect("hold size"), &limits);
            encode_hold_category(&category, &mut out, &mut budget);
            assert_eq!(out.finish().as_ref(), expected, "{bound:?}");
        }
    }
}
