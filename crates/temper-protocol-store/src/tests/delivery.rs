use crate::{Undecodable, delivery, keys};
use temper_engine_domain::{CallKey, DeliveryMarker, DeliveryReason, DeliveryReceipt, DeliveryRecord, DeliveryReply};

const KEY: CallKey = CallKey { task: 1, attempt: 2, completion: 3, position: 4 };

#[test]
fn every_delivery_variant_round_trips_through_family_twenty_two() {
    let bounds = crate::bounds(&super::fixtures::LIMITS).unwrap();
    for reply in [
        DeliveryReply::Delivered(Box::new([DeliveryReceipt { directory: 2, text: b"receipt".as_slice().into() }])),
        DeliveryReply::Nothing,
        DeliveryReply::Refused {
            marker: Some(DeliveryMarker { directory: 3, path: b"hold".as_slice().into() }),
            explanation: b"explanation".as_slice().into(),
        },
        DeliveryReply::Failed {
            directory: 4,
            reason: DeliveryReason::TimedOut,
            diagnostic: b"diagnostic".as_slice().into(),
            dropped: 7,
        },
        DeliveryReply::Stale,
    ] {
        let row = DeliveryRecord { key: KEY, reply };
        let key = keys::delivery(KEY, u32::MAX).unwrap();
        let bytes = delivery::encode(&row).unwrap();
        assert_eq!(delivery::row(&key, &bytes, &bounds), Ok(row.clone()));
        let wire = delivery::encode_reply(&row.reply).unwrap();
        assert_eq!(row.reply.reply_bytes(), Some(u64::try_from(wire.len()).unwrap()));
        assert_eq!(delivery::decode_reply(&wire), Some(row.reply));
    }
}
#[test]
fn family_twenty_two_has_literal_key_and_record_bytes() {
    let row = DeliveryRecord { key: KEY, reply: DeliveryReply::Nothing };
    assert_eq!(
        keys::delivery(KEY, u32::MAX).unwrap().as_ref(),
        &[22, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 4]
    );
    assert_eq!(
        delivery::encode(&row).unwrap().as_ref(),
        &[0, 10, 21, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4, 1]
    );
}
#[test]
fn family_twenty_two_refuses_a_disagreeing_key_and_trailing_bytes() {
    let bounds = crate::bounds(&super::fixtures::LIMITS).unwrap();
    let row = DeliveryRecord { key: KEY, reply: DeliveryReply::Stale };
    let other = keys::delivery(CallKey { position: 5, ..KEY }, u32::MAX).unwrap();
    assert_eq!(delivery::row(&other, &delivery::encode(&row).unwrap(), &bounds), Err(Undecodable));
    assert_eq!(delivery::decode_reply(&[1, 0]), None);
}
#[test]
fn the_delivery_interval_covers_only_its_ordered_family() {
    let range = keys::delivery_interval();
    let key = keys::delivery(KEY, u32::MAX).unwrap();
    assert!(key.as_ref() >= range.start.as_ref());
    assert!(key.as_ref() < range.end.as_deref().unwrap());
    assert_eq!(keys::decode_delivery(&key), Some(KEY));
    assert!(temper_engine_domain::Range::Forge.contains(&temper_engine_domain::Key::Delivery(KEY)));
}
#[test]
fn smiths_receipt_count_and_text_bounds_hold_on_both_sides() {
    for (count, bytes, fits) in [
        (DeliveryReply::RECEIPTS, DeliveryReply::TEXT, true),
        (DeliveryReply::RECEIPTS + 1, DeliveryReply::TEXT, false),
        (DeliveryReply::RECEIPTS, DeliveryReply::TEXT + 1, false),
    ] {
        let mut rows = skein_lib::List::with_capacity(count);
        for _ in 0..count {
            rows.push(DeliveryReceipt { directory: 0, text: bytes_at_bound(bytes) }).unwrap();
        }
        let row = DeliveryRecord { key: KEY, reply: DeliveryReply::Delivered(rows.into_boxed()) };
        assert_eq!(delivery::encode(&row).is_some(), fits);
        assert_eq!(delivery::encode_reply(&row.reply).is_some(), fits);
    }
}
#[test]
fn smiths_refusal_and_diagnostic_bounds_hold_on_both_sides() {
    for (path, text, fits) in [
        (DeliveryReply::PATH, DeliveryReply::TEXT, true),
        (DeliveryReply::PATH + 1, DeliveryReply::TEXT, false),
        (DeliveryReply::PATH, DeliveryReply::TEXT + 1, false),
    ] {
        let reply = DeliveryReply::Refused {
            marker: Some(DeliveryMarker { directory: 0, path: bytes_at_bound(path) }),
            explanation: bytes_at_bound(text),
        };
        assert_eq!(delivery::encode(&DeliveryRecord { key: KEY, reply: reply.clone() }).is_some(), fits);
        assert_eq!(delivery::encode_reply(&reply).is_some(), fits);
    }
    for (bytes, fits) in [(DeliveryReply::TEXT, true), (DeliveryReply::TEXT + 1, false)] {
        let reply = DeliveryReply::Failed {
            directory: 0,
            reason: DeliveryReason::Broken,
            diagnostic: bytes_at_bound(bytes),
            dropped: 0,
        };
        assert_eq!(delivery::encode(&DeliveryRecord { key: KEY, reply: reply.clone() }).is_some(), fits);
        assert_eq!(delivery::encode_reply(&reply).is_some(), fits);
    }
}

fn bytes_at_bound(count: u32) -> Box<[u8]> {
    let mut bytes = skein_lib::List::with_capacity(count);
    for _ in 0..count {
        bytes.push(b'x').unwrap();
    }
    bytes.into_boxed()
}
