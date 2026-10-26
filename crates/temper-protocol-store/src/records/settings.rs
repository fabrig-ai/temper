//! Values of temper's settings kinds (protocol/configuration.md, 13.2).
//! Bytes stay here; the forge judges the decoded value within its current bounds.
use alloc::boxed::Box;
use jig_domain_authority::{Last, Pattern};
use skein_lib::{List, Reader, Writer};
use temper_engine_domain_forge::{
    JudgeFreshness,
    settings::{Bounds, Kind, LandingRule, RuleApproval, RuleGate, Value},
};

use super::FITS;
use crate::v10 as w;

/// Decode a whole setting value. `None` is the protocol boundary's `Refusal::Decode`,
/// for malformed bytes or an unknown kind. The forge judges semantic bounds later.
#[must_use]
pub fn decode_value(kind: u32, bytes: &[u8]) -> Option<Value> {
    // Reader's input is bounded by u32, even when a caller supplies an arbitrary slice.
    u32::try_from(bytes.len()).ok()?;
    match Kind::of(kind)? {
        Kind::DeploymentRules | Kind::ProjectRules => {
            let parts = w::LandingRule::decode(&w::CEILINGS, &mut Reader::new(bytes)).ok()?.into_parts();
            let pattern = parts.pattern.into_parts();
            let last = match pattern.last {
                w::RuleLast::Exact(last) => Last::Exact(last.into_parts().value),
                w::RuleLast::Open(last) => Last::Open(last.into_parts().value),
            };
            let mut gates = List::with_capacity(parts.gates.len());
            for gate in parts.gates.into_boxed() {
                let gate = gate.into_parts();
                gates
                    .push(RuleGate {
                        number: gate.number,
                        blocking: gate.blocking,
                        freshness: decode_freshness(gate.freshness),
                    })
                    .expect("decoded count");
            }
            let mut approvals = List::with_capacity(parts.approvals.len());
            for approval in parts.approvals.into_boxed() {
                let approval = approval.into_parts();
                approvals
                    .push(RuleApproval {
                        role: approval.role,
                        people: approval.people,
                        freshness: decode_freshness(approval.freshness),
                    })
                    .expect("decoded count");
            }
            Some(Value::Rule(LandingRule {
                pattern: Pattern { segments: pattern.segments.into_boxed(), last },
                enforce: parts.enforce,
                ci: parts.ci,
                up_to_date: parts.up_to_date,
                gates: gates.into_boxed(),
                approvals: approvals.into_boxed(),
            }))
        }
        Kind::ForgeCharter => Some(Value::Charter(u32::from_be_bytes(bytes.try_into().ok()?))),
    }
}

/// Encode a value admitted within the schema ceilings. A charter is the codec's
/// bare big-endian u32, with no additional record or value tag.
#[must_use]
pub fn encode_value(value: &Value) -> Box<[u8]> {
    let rule = match value {
        Value::Charter(number) => return Box::new(number.to_be_bytes()),
        Value::Rule(rule) => encode_rule(rule),
    };
    let mut writer = Writer::new(usize::try_from(rule.measure()).expect("measured length fits"));
    rule.encode(&mut writer).expect("measured room");
    writer.finish()
}

fn encode_rule(rule: &LandingRule) -> w::LandingRule {
    let mut segments = List::with_capacity(u32::try_from(rule.pattern.segments.len()).expect(FITS));
    for segment in &rule.pattern.segments {
        segments.push(segment.clone()).expect("same count");
    }
    let (Last::Exact(value) | Last::Open(value)) = &rule.pattern.last;
    let terminal = w::RuleTerminal::new(&w::CEILINGS, w::RuleTerminalParts { value: value.clone() }).expect(FITS);
    let last = match rule.pattern.last {
        Last::Exact(_) => w::RuleLast::Exact(terminal),
        Last::Open(_) => w::RuleLast::Open(terminal),
    };
    let pattern = w::RulePattern::new(&w::CEILINGS, w::RulePatternParts { segments, last }).expect(FITS);
    let mut gates = List::with_capacity(u32::try_from(rule.gates.len()).expect(FITS));
    for gate in &rule.gates {
        gates
            .push(
                w::RuleGate::new(
                    &w::CEILINGS,
                    w::RuleGateParts {
                        number: gate.number,
                        blocking: gate.blocking,
                        freshness: encode_freshness(gate.freshness),
                    },
                )
                .expect(FITS),
            )
            .expect("same count");
    }
    let mut approvals = List::with_capacity(u32::try_from(rule.approvals.len()).expect(FITS));
    for approval in &rule.approvals {
        approvals
            .push(
                w::RuleApproval::new(
                    &w::CEILINGS,
                    w::RuleApprovalParts {
                        role: approval.role,
                        people: approval.people,
                        freshness: encode_freshness(approval.freshness),
                    },
                )
                .expect(FITS),
            )
            .expect("same count");
    }
    w::LandingRule::new(
        &w::CEILINGS,
        w::LandingRuleParts {
            pattern,
            enforce: rule.enforce,
            ci: rule.ci,
            up_to_date: rule.up_to_date,
            gates,
            approvals,
        },
    )
    .expect(FITS)
}

fn encode_freshness(value: JudgeFreshness) -> w::Freshness {
    match value {
        JudgeFreshness::Exact => w::Freshness::Exact,
        JudgeFreshness::Clean => w::Freshness::Clean,
    }
}

fn decode_freshness(value: w::Freshness) -> JudgeFreshness {
    match value {
        w::Freshness::Exact => JudgeFreshness::Exact,
        w::Freshness::Clean => JudgeFreshness::Clean,
    }
}

/// The largest record encoding at the kind's bounds, including length and enum tags.
/// `bounds` must fit the schema ceilings, as other admitted store bounds do.
#[must_use]
pub fn landing_rule_bytes(bounds: &Bounds) -> u64 {
    let limits = w::Limits {
        rule_pattern_segments: bounds.segments,
        rule_pattern_segments_item: bounds.segment_bytes,
        rule_terminal_value: bounds.segment_bytes,
        landing_rule_gates: bounds.gates,
        landing_rule_approvals: bounds.roles,
        ..w::CEILINGS
    };
    w::LandingRule::worst_case_bytes(&limits).expect("admitted kind bounds fit schema ceilings")
}
