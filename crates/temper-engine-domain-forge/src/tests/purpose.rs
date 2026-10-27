use crate::purpose::EffectPurpose;
use alloc::boxed::Box;

#[test]
fn effect_keys_are_stable_by_purpose_and_separate_deployments() {
    let purpose = EffectPurpose::Call { task: 7, attempt: 2, completion: 3, position: 4 };
    let first = purpose.key(&[1; 16], 44).expect("call key fits");
    assert_eq!(first, purpose.key(&[1; 16], 44).expect("same purpose fits"));
    assert_ne!(first, purpose.key(&[2; 16], 44).expect("other deployment fits"));
    assert_ne!(
        first,
        EffectPurpose::Call { task: 7, attempt: 2, completion: 3, position: 5 }
            .key(&[1; 16], 44)
            .expect("other call fits"),
    );
    assert!(purpose.key(&[1; 16], 43).is_none());
}

#[test]
fn projection_history_keys_keep_task_position_and_history_family() {
    let purpose = EffectPurpose::ProjectionHistory { goal: 1, task: 2, position: 3, family: 1 };
    let first = purpose.key(&[1; 16], 45).expect("bounded history key");
    assert_eq!(first, purpose.key(&[1; 16], 45).expect("stable history key"));
    for other in [
        EffectPurpose::ProjectionHistory { goal: 2, task: 2, position: 3, family: 1 },
        EffectPurpose::ProjectionHistory { goal: 1, task: 3, position: 3, family: 1 },
        EffectPurpose::ProjectionHistory { goal: 1, task: 2, position: 4, family: 1 },
        EffectPurpose::ProjectionHistory { goal: 1, task: 2, position: 3, family: 2 },
        EffectPurpose::Projection { goal: 1, part: 1, number: 3 },
    ] {
        assert_ne!(first, other.key(&[1; 16], 45).expect("separate history key"));
    }
    assert_eq!(first.len(), 45);
    assert!(purpose.key(&[1; 16], 44).is_none());
}

#[test]
fn identity_bytes_pin_tags_one_to_five() {
    let rows: [(EffectPurpose, &[u8]); 5] = [
        (
            EffectPurpose::Call { task: 7, attempt: 2, completion: 3, position: 4 },
            &[1, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4],
        ),
        (EffectPurpose::Step { task: 7, purpose: 9 }, &[2, 0, 0, 0, 0, 0, 0, 0, 7, 0, 9]),
        (
            EffectPurpose::Projection { goal: 1, part: 2, number: 3 },
            &[3, 0, 0, 0, 0, 0, 0, 0, 1, 2, 0, 0, 0, 0, 0, 0, 0, 3],
        ),
        (
            EffectPurpose::Release { task: 7, resource: Box::from(&b"ab"[..]) },
            &[4, 0, 0, 0, 0, 0, 0, 0, 7, 0, 2, b'a', b'b'],
        ),
        (
            EffectPurpose::ProjectionHistory { goal: 1, task: 2, position: 3, family: 1 },
            &[5, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 3],
        ),
    ];
    for (purpose, expected) in rows {
        assert_eq!(purpose.identity().as_deref(), Some(expected), "{purpose:?}");
    }
}

#[rustfmt::skip]
const CALL_KEY: [u8; 44] = [ 1, 0, 16, 7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,
    1, 0,0,0,0,0,0,0,7, 0,0,0,0,0,0,0,2, 0,0,0,3, 0,0,0,4 ];
#[rustfmt::skip]
const HISTORY_KEY: [u8; 45] = [ 1, 0, 16, 7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,
    5, 1, 0,0,0,0,0,0,0,1, 0,0,0,0,0,0,0,2, 0,0,0,0,0,0,0,3 ];

#[test]
fn a_call_key_is_the_framed_forty_four_bytes() {
    let purpose = EffectPurpose::Call { task: 7, attempt: 2, completion: 3, position: 4 };
    assert_eq!(purpose.key(&[7; 16], 44).as_deref(), Some(CALL_KEY.as_slice()));
    assert_eq!(purpose.key(&[7; 16], 43), None);
}

#[test]
fn a_history_key_is_the_framed_forty_five_bytes() {
    let purpose = EffectPurpose::ProjectionHistory { goal: 1, task: 2, position: 3, family: 1 };
    assert_eq!(purpose.key(&[7; 16], 45).as_deref(), Some(HISTORY_KEY.as_slice()));
    assert_eq!(purpose.key(&[7; 16], 44), None);
}

#[test]
fn a_tier_purpose_is_thirty_bytes_in_field_order() {
    const IDENTITY_BYTES: usize = 30; // after-landing.md, 10: tag + repository + tier + run + operation + part.
    const EXPECTED: [u8; IDENTITY_BYTES] = [
        8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 0x17, 0x18, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27,
        0x28, 0x31, 0x41, 0x42,
    ];
    let purpose = EffectPurpose::Tier {
        repository: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        tier: 0x1718,
        run: 0x2122_2324_2526_2728,
        operation: 0x31,
        part: 0x4142,
    };
    assert_eq!(purpose.identity().as_deref(), Some(EXPECTED.as_slice()));
}
