use super::fixtures;
use crate::{Undecodable, application, keys, records};
use temper_engine_domain_forge as forge;

#[test]
fn the_application_sizes_are_the_largest_key_and_row() {
    // Tier's full finding, pending range, flight and staged judgement are the largest row.
    assert_eq!(
        application(&fixtures::LIMITS),
        Some(jig_protocol_store::sizes::Application { version: 10, key: 403, value: 175_879 })
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
