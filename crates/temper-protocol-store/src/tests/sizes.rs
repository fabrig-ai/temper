use super::fixtures;
use crate::{Undecodable, application, keys, records};
use temper_engine_domain_forge as forge;

#[test]
fn the_application_sizes_are_the_largest_key_and_row() {
    // Change's eight remarks each hold 4096 bytes; the complete row is 35403.
    assert_eq!(
        application(&fixtures::LIMITS),
        Some(jig_protocol_store::sizes::Application { version: 10, key: 403, value: 35_403 })
    );
}
#[test]
fn limits_past_a_ceiling_have_no_sizes() {
    for (name, field, value) in [("name", 0_u8, 4097_u32), ("answer", 1, 1_048_577)] {
        let mut limits = fixtures::LIMITS;
        match field {
            0 => limits.name_bytes = value,
            1 => limits.client.answer_bytes = value,
            _ => panic!("table field"),
        }
        assert_eq!(application(&limits), None, "{name}");
    }
}
#[test]
fn a_row_whose_key_disagrees_with_its_record_is_undecodable() {
    let bounds = crate::bounds(&fixtures::LIMITS).expect("fixture limits fit");
    let row = forge::Stored::Landed { commit: [1; 32], task: 2 };
    let value = records::encode(&row);
    for (name, other) in [
        ("right key", forge::Key::Landed([1; 32])),
        ("wrong commit", forge::Key::Landed([9; 32])),
        ("wrong family", forge::Key::Change(2)),
    ] {
        let key = keys::encode(&other, u32::MAX).expect("a fixed key fits");
        if name == "right key" {
            assert_eq!(records::row(&key, &value, &bounds), Ok(row.clone()));
        } else {
            assert_eq!(records::row(&key, &value, &bounds), Err(Undecodable), "{name}");
        }
    }
    assert_eq!(records::row(&[], &value, &bounds), Err(Undecodable), "malformed key");
    let key = keys::encode(&forge::Key::Landed([1; 32]), u32::MAX).expect("a fixed key fits");
    assert_eq!(records::row(&key, &[], &bounds), Err(Undecodable), "malformed value");
}
#[test]
fn the_store_version_is_ten() {
    assert_eq!(crate::STORE_VERSION, 10);
}

#[test]
fn demo_row_sizes_fit_the_existing_application_ceiling() {
    use crate::v10 as w;
    assert_eq!(w::Demo::worst_case_bytes(&w::CEILINGS), Some(3));
    let bounds = crate::bounds(&fixtures::LIMITS).expect("fixture bounds");
    for state in [forge::DemoState::Running, forge::DemoState::Ended] {
        let row = forge::Stored::Demo(forge::Demo { forge: u16::MAX, state });
        let value = records::encode(&row);
        let key = keys::encode(&forge::stored_key(&row), u32::MAX).expect("key");
        assert_eq!(key.len(), 9);
        assert_eq!(value.len(), 6);
        assert_eq!(records::row(&key, &value, &bounds), Ok(row));
    }
}

#[test]
fn the_largest_landing_rule_fits_the_kinds_bound() {
    use forge::{
        JudgeFreshness,
        settings::{self, LandingRule, RuleApproval, RuleGate, Value},
    };
    use jig_domain_authority::{Last, Pattern};
    use skein_lib::List;
    let bounds = &fixtures::SETTINGS_BOUNDS;
    let mut word = List::with_capacity(bounds.segment_bytes);
    for _ in 0..bounds.segment_bytes {
        word.push(b'x').expect("bounded word");
    }
    let word = word.into_boxed();
    let mut segments = List::with_capacity(bounds.segments);
    for _ in 0..bounds.segments {
        segments.push(word.clone()).expect("bounded segments");
    }
    let mut gates = List::with_capacity(bounds.gates);
    for number in 0..bounds.gates {
        gates.push(RuleGate { number, blocking: true, freshness: JudgeFreshness::Exact }).expect("bounded gates");
    }
    let mut approvals = List::with_capacity(bounds.roles);
    for role in 0..bounds.roles {
        approvals
            .push(RuleApproval { role, people: u32::MAX, freshness: JudgeFreshness::Clean })
            .expect("bounded roles");
    }
    let mut rule = LandingRule {
        pattern: Pattern { segments: segments.into_boxed(), last: Last::Exact(word.clone()) },
        enforce: true,
        ci: true,
        up_to_date: true,
        gates: gates.into_boxed(),
        approvals: approvals.into_boxed(),
    };
    let bound = records::landing_rule_bytes(bounds);
    assert_eq!(bound, 1400, "small's record bound, including lengths and tags");
    for last in [Last::Exact(word.clone()), Last::Open(word)] {
        rule.pattern.last = last;
        let value = Value::Rule(rule.clone());
        let bytes = records::encode_value(&value);
        assert_eq!(u64::try_from(bytes.len()).expect("encoded size"), bound, "fixed at every encoding bound");
        assert_eq!(records::decode_value(1, &bytes), Some(value));
    }
    // The kind's byte bound reserves a full first segment too. An admitted rule's
    // literal `forge` makes it smaller by 251 bytes at small's 256-byte word bound.
    *rule.pattern.segments.first_mut().expect("nonempty pattern") = b"forge".as_slice().into();
    let value = Value::Rule(rule);
    let bytes = records::encode_value(&value);
    assert_eq!(u64::try_from(bytes.len()).expect("encoded size"), 1149);
    assert_eq!(settings::judge(&settings::Item { kind: 1, scope: 0, number: 4, value: Some(value) }, bounds), Ok(()));
}
