//! The application's delivery record and smith's current wire reply, translated at the protocol boundary.
use crate::{Bounds, Undecodable, v10 as store};
use alloc::boxed::Box;
use skein_lib::{List, Reader, Writer};
use smith_channel as wire;
use temper_engine_domain::{self as d, DeliveryReply};

/// Encode a family 22 row under the unchanged application store version.
#[must_use]
pub fn encode(row: &d::DeliveryRecord) -> Option<Box<[u8]>> {
    let key = store::NamedCall::new(
        &store::CEILINGS,
        store::NamedCallParts {
            task: row.key.task,
            attempt: row.key.attempt,
            completion: row.key.completion,
            position: row.key.position,
        },
    )
    .ok()?;
    let record =
        store::DeliveryRecord::new(&store::CEILINGS, store::DeliveryRecordParts { key, reply: to_store(&row.reply)? })
            .ok()?;
    let row = store::Row::new(&store::CEILINGS, store::RowParts { value: store::Value::Delivery(record) }).ok()?;
    let mut writer = Writer::new(usize::try_from(row.measure()).ok()?);
    row.encode(&mut writer).ok()?;
    Some(writer.finish())
}
/// Decode and validate the entire row, including its key.
pub fn row(key: &[u8], bytes: &[u8], bounds: &Bounds) -> Result<d::DeliveryRecord, Undecodable> {
    let key = crate::keys::decode_delivery(key).ok_or(Undecodable)?;
    let mut reader = Reader::new(bytes);
    let value = store::Row::decode(bounds.limits(), &mut reader).or(Err(Undecodable))?;
    if !reader.is_empty() {
        return Err(Undecodable);
    }
    let store::Value::Delivery(value) = value.into_parts().value else { return Err(Undecodable) };
    let value = value.into_parts();
    let fields = value.key.into_parts();
    let recorded = d::CallKey {
        task: fields.task,
        attempt: fields.attempt,
        completion: fields.completion,
        position: fields.position,
    };
    let reply = from_store(value.reply);
    if key != recorded || key.task == 0 || key.attempt == 0 || key.completion == 0 || !reply.valid() {
        return Err(Undecodable);
    }
    Ok(d::DeliveryRecord { key, reply })
}
/// Read smith's evidence once into application-owned fields.
#[must_use]
pub fn decode_reply(bytes: &[u8]) -> Option<DeliveryReply> {
    let mut reader = Reader::new(bytes);
    let value = wire::DeliveryReply::decode(&wire::CEILINGS, &mut reader).ok()?;
    if !reader.is_empty() {
        return None;
    }
    Some(from_wire(value.into_parts().value))
}
/// Encode the current smith reply for an assignment, without changing the down-channel.
#[must_use]
pub fn encode_reply(reply: &DeliveryReply) -> Option<Box<[u8]>> {
    let value = to_wire(reply)?;
    let record = wire::DeliveryReply::new(&wire::CEILINGS, wire::DeliveryReplyParts { value }).ok()?;
    let mut writer = Writer::new(usize::try_from(record.measure()).ok()?);
    record.encode(&mut writer).ok()?;
    Some(writer.finish())
}

fn to_store(reply: &DeliveryReply) -> Option<store::DeliveryReply> {
    use store as w;
    if !reply.valid() {
        return None;
    }
    Some(match reply {
        DeliveryReply::Delivered(rows) => {
            let mut receipts = List::with_capacity(DeliveryReply::RECEIPTS);
            for row in rows {
                receipts
                    .push(
                        w::DeliveryReceipt::new(
                            &w::CEILINGS,
                            w::DeliveryReceiptParts { directory: row.directory, text: row.text.clone() },
                        )
                        .ok()?,
                    )
                    .ok()?;
            }
            w::DeliveryReply::Delivered(
                w::DeliveryDelivered::new(&w::CEILINGS, w::DeliveryDeliveredParts { receipts }).ok()?,
            )
        }
        DeliveryReply::Nothing => w::DeliveryReply::Nothing,
        DeliveryReply::Stale => w::DeliveryReply::Stale,
        DeliveryReply::Refused { marker, explanation } => {
            let marker = match marker {
                Some(marker) => Some(
                    w::DeliveryMarker::new(
                        &w::CEILINGS,
                        w::DeliveryMarkerParts { directory: marker.directory, path: marker.path.clone() },
                    )
                    .ok()?,
                ),
                None => None,
            };
            w::DeliveryReply::Refused(
                w::DeliveryRefusal::new(
                    &w::CEILINGS,
                    w::DeliveryRefusalParts { marker, explanation: explanation.clone() },
                )
                .ok()?,
            )
        }
        DeliveryReply::Failed { directory, reason, diagnostic, dropped } => w::DeliveryReply::Failed(
            w::DeliveryFailure::new(
                &w::CEILINGS,
                w::DeliveryFailureParts {
                    directory: *directory,
                    reason: match reason {
                        d::DeliveryReason::Unreachable => w::DeliveryReason::Unreachable,
                        d::DeliveryReason::RefusedByTarget => w::DeliveryReason::RefusedByTarget,
                        d::DeliveryReason::TimedOut => w::DeliveryReason::TimedOut,
                        d::DeliveryReason::Broken => w::DeliveryReason::Broken,
                        d::DeliveryReason::TooLarge => w::DeliveryReason::TooLarge,
                        d::DeliveryReason::Missing => w::DeliveryReason::Missing,
                        d::DeliveryReason::Busy => w::DeliveryReason::Busy,
                        d::DeliveryReason::Unavailable => w::DeliveryReason::Unavailable,
                        d::DeliveryReason::Cancelled => w::DeliveryReason::Cancelled,
                        d::DeliveryReason::Unknown => w::DeliveryReason::Unknown,
                    },
                    diagnostic: diagnostic.clone(),
                    dropped: *dropped,
                },
            )
            .ok()?,
        ),
    })
}
fn from_store(reply: store::DeliveryReply) -> DeliveryReply {
    use store as w;
    match reply {
        w::DeliveryReply::Delivered(value) => {
            let rows = value.into_parts().receipts.into_boxed();
            let mut receipts = List::with_capacity(DeliveryReply::RECEIPTS);
            for row in rows {
                let row = row.into_parts();
                receipts
                    .push(d::DeliveryReceipt { directory: row.directory, text: row.text })
                    .expect("decoded receipt count");
            }
            DeliveryReply::Delivered(receipts.into_boxed())
        }
        w::DeliveryReply::Nothing => DeliveryReply::Nothing,
        w::DeliveryReply::Stale => DeliveryReply::Stale,
        w::DeliveryReply::Refused(value) => {
            let value = value.into_parts();
            let marker = match value.marker {
                Some(marker) => {
                    let marker = marker.into_parts();
                    Some(d::DeliveryMarker { directory: marker.directory, path: marker.path })
                }
                None => None,
            };
            DeliveryReply::Refused { marker, explanation: value.explanation }
        }
        w::DeliveryReply::Failed(value) => {
            let value = value.into_parts();
            DeliveryReply::Failed {
                directory: value.directory,
                reason: match value.reason {
                    w::DeliveryReason::Unreachable => d::DeliveryReason::Unreachable,
                    w::DeliveryReason::RefusedByTarget => d::DeliveryReason::RefusedByTarget,
                    w::DeliveryReason::TimedOut => d::DeliveryReason::TimedOut,
                    w::DeliveryReason::Broken => d::DeliveryReason::Broken,
                    w::DeliveryReason::TooLarge => d::DeliveryReason::TooLarge,
                    w::DeliveryReason::Missing => d::DeliveryReason::Missing,
                    w::DeliveryReason::Busy => d::DeliveryReason::Busy,
                    w::DeliveryReason::Unavailable => d::DeliveryReason::Unavailable,
                    w::DeliveryReason::Cancelled => d::DeliveryReason::Cancelled,
                    w::DeliveryReason::Unknown => d::DeliveryReason::Unknown,
                },
                diagnostic: value.diagnostic,
                dropped: value.dropped,
            }
        }
    }
}

fn to_wire(reply: &DeliveryReply) -> Option<wire::Delivery> {
    use wire as w;
    if !reply.valid() {
        return None;
    }
    Some(match reply {
        DeliveryReply::Delivered(rows) => {
            let mut receipts = List::with_capacity(DeliveryReply::RECEIPTS);
            for row in rows {
                receipts
                    .push(
                        w::Receipt::new(
                            &w::CEILINGS,
                            w::ReceiptParts { directory: row.directory, text: row.text.clone() },
                        )
                        .ok()?,
                    )
                    .ok()?;
            }
            w::Delivery::Delivered(w::Delivered::new(&w::CEILINGS, w::DeliveredParts { receipts }).ok()?)
        }
        DeliveryReply::Nothing => w::Delivery::Nothing,
        DeliveryReply::Stale => w::Delivery::Stale,
        DeliveryReply::Refused { marker, explanation } => {
            let marker = match marker {
                Some(marker) => Some(
                    w::Marker::new(
                        &w::CEILINGS,
                        w::MarkerParts { directory: marker.directory, path: marker.path.clone() },
                    )
                    .ok()?,
                ),
                None => None,
            };
            w::Delivery::Refused(
                w::DeliveryRefusal::new(
                    &w::CEILINGS,
                    w::DeliveryRefusalParts { marker, explanation: explanation.clone() },
                )
                .ok()?,
            )
        }
        DeliveryReply::Failed { directory, reason, diagnostic, dropped } => w::Delivery::Failed(
            w::DeliveryFailure::new(
                &w::CEILINGS,
                w::DeliveryFailureParts {
                    directory: *directory,
                    reason: match reason {
                        d::DeliveryReason::Unreachable => w::DeliveryReason::Unreachable,
                        d::DeliveryReason::RefusedByTarget => w::DeliveryReason::RefusedByTarget,
                        d::DeliveryReason::TimedOut => w::DeliveryReason::TimedOut,
                        d::DeliveryReason::Broken => w::DeliveryReason::Broken,
                        d::DeliveryReason::TooLarge => w::DeliveryReason::TooLarge,
                        d::DeliveryReason::Missing => w::DeliveryReason::Missing,
                        d::DeliveryReason::Busy => w::DeliveryReason::Busy,
                        d::DeliveryReason::Unavailable => w::DeliveryReason::Unavailable,
                        d::DeliveryReason::Cancelled => w::DeliveryReason::Cancelled,
                        d::DeliveryReason::Unknown => w::DeliveryReason::Unknown,
                    },
                    diagnostic: diagnostic.clone(),
                    dropped: *dropped,
                },
            )
            .ok()?,
        ),
    })
}
fn from_wire(reply: wire::Delivery) -> DeliveryReply {
    use wire as w;
    match reply {
        w::Delivery::Delivered(value) => {
            let rows = value.into_parts().receipts.into_boxed();
            let mut receipts = List::with_capacity(DeliveryReply::RECEIPTS);
            for row in rows {
                let row = row.into_parts();
                receipts
                    .push(d::DeliveryReceipt { directory: row.directory, text: row.text })
                    .expect("decoded receipt count");
            }
            DeliveryReply::Delivered(receipts.into_boxed())
        }
        w::Delivery::Nothing => DeliveryReply::Nothing,
        w::Delivery::Stale => DeliveryReply::Stale,
        w::Delivery::Refused(value) => {
            let value = value.into_parts();
            let marker = match value.marker {
                Some(marker) => {
                    let marker = marker.into_parts();
                    Some(d::DeliveryMarker { directory: marker.directory, path: marker.path })
                }
                None => None,
            };
            DeliveryReply::Refused { marker, explanation: value.explanation }
        }
        w::Delivery::Failed(value) => {
            let value = value.into_parts();
            DeliveryReply::Failed {
                directory: value.directory,
                reason: match value.reason {
                    w::DeliveryReason::Unreachable => d::DeliveryReason::Unreachable,
                    w::DeliveryReason::RefusedByTarget => d::DeliveryReason::RefusedByTarget,
                    w::DeliveryReason::TimedOut => d::DeliveryReason::TimedOut,
                    w::DeliveryReason::Broken => d::DeliveryReason::Broken,
                    w::DeliveryReason::TooLarge => d::DeliveryReason::TooLarge,
                    w::DeliveryReason::Missing => d::DeliveryReason::Missing,
                    w::DeliveryReason::Busy => d::DeliveryReason::Busy,
                    w::DeliveryReason::Unavailable => d::DeliveryReason::Unavailable,
                    w::DeliveryReason::Cancelled => d::DeliveryReason::Cancelled,
                    w::DeliveryReason::Unknown => d::DeliveryReason::Unknown,
                },
                diagnostic: value.diagnostic,
                dropped: value.dropped,
            }
        }
    }
}
