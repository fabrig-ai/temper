use super::fixtures;
use crate::{Undecodable, application, keys, records};
use temper_engine_domain_forge as forge;

#[test]
fn the_application_sizes_are_the_largest_key_and_row() {
    // Change's eight remarks hold 4096 bytes each; eight failure counts add 100 bytes.
    // Authority causes add 42 bytes to each of eight holds, drift and released.
    assert_eq!(
        application(&fixtures::LIMITS, fixtures::LIMITS.client.op_bytes),
        Some(jig_protocol_store::sizes::Application { version: 10, key: 403, value: 35_933 })
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
        assert_eq!(application(&limits, limits.client.op_bytes), None, "{name}");
    }
}
#[test]
fn a_row_whose_key_disagrees_with_its_record_is_undecodable() {
    let bounds = crate::bounds(&fixtures::LIMITS, fixtures::LIMITS.client.op_bytes).expect("fixture limits fit");
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
    let bounds = crate::bounds(&fixtures::LIMITS, fixtures::LIMITS.client.op_bytes).expect("fixture bounds");
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
fn plan_text_uses_the_body_bound_without_changing_the_format() {
    // Eight items grow from 256 bytes of text each to 512 bytes each.
    const EXTRA_PLAN_BYTES: u64 = 2048;
    let bounds = crate::bounds(&fixtures::LIMITS, fixtures::LIMITS.client.op_bytes).expect("fixture bounds");
    let after = *bounds.limits();
    let before = crate::v10::Limits { plan_item_text: fixtures::LIMITS.issue_policy.comment_bytes, ..after };
    let old_view = crate::v10::GoalView::worst_case_bytes(&before).expect("old view size");
    let new_view = crate::v10::GoalView::worst_case_bytes(&after).expect("new view size");
    assert_eq!(new_view - old_view, EXTRA_PLAN_BYTES);
    // The schema's fixed framing plus the measured view; ChangeRow still dominates Row.
    assert_eq!(old_view, 5238);
    assert_eq!(new_view, 7286);
    assert_eq!(crate::v10::IssueRow::worst_case_bytes(&before), Some(5657));
    assert_eq!(crate::v10::IssueRow::worst_case_bytes(&after), Some(7705));
    assert_eq!(crate::v10::IssueRow::worst_case_heap(&before), Some(5888));
    assert_eq!(crate::v10::IssueRow::worst_case_heap(&after), Some(7936));
    assert_eq!(crate::v10::Row::worst_case_bytes(&before), Some(35_933));
    assert_eq!(crate::v10::Row::worst_case_bytes(&after), Some(35_933));
}
