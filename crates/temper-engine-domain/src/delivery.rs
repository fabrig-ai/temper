//! Application-owned delivery evidence (protocol/store.md, family 22).
use alloc::boxed::Box;
use jig_domain_core::{CallKey, DeliveryOutcome};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DeliveryRecord {
    pub key: CallKey,
    pub reply: DeliveryReply,
}

/// Smith's delivery reply in the application's vocabulary, without codec bytes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DeliveryReply {
    Delivered(Box<[DeliveryReceipt]>),
    Nothing,
    Refused { marker: Option<DeliveryMarker>, explanation: Box<[u8]> },
    Failed { directory: u32, reason: DeliveryReason, diagnostic: Box<[u8]>, dropped: u64 },
    Stale,
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DeliveryReceipt {
    pub directory: u32,
    pub text: Box<[u8]>,
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DeliveryMarker {
    pub directory: u32,
    pub path: Box<[u8]>,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeliveryReason {
    Unreachable,
    RefusedByTarget,
    TimedOut,
    Broken,
    TooLarge,
    Missing,
    Busy,
    Unavailable,
    Cancelled,
    Unknown,
}
impl DeliveryReply {
    pub const RECEIPTS: u32 = 64;
    pub const TEXT: u32 = 512;
    pub const PATH: u32 = 4096;

    #[must_use]
    pub const fn outcome(&self) -> DeliveryOutcome {
        match self {
            Self::Delivered(_) => DeliveryOutcome::Delivered,
            Self::Nothing => DeliveryOutcome::Nothing,
            Self::Refused { .. } => DeliveryOutcome::Refused,
            Self::Failed { .. } => DeliveryOutcome::Failed,
            Self::Stale => DeliveryOutcome::Stale,
        }
    }
    #[must_use]
    pub fn valid(&self) -> bool {
        let receipts_bound = usize::try_from(Self::RECEIPTS).expect("receipt count fits usize");
        let text_bound = usize::try_from(Self::TEXT).expect("text bound fits usize");
        let path_bound = usize::try_from(Self::PATH).expect("path bound fits usize");
        match self {
            Self::Delivered(receipts) => {
                receipts.len() <= receipts_bound && {
                    let mut valid = true;
                    for row in receipts {
                        valid &= row.text.len() <= text_bound;
                    }
                    valid
                }
            }
            Self::Nothing | Self::Stale => true,
            Self::Refused { marker, explanation } => {
                explanation.len() <= text_bound
                    && match marker {
                        Some(marker) => marker.path.len() <= path_bound,
                        None => true,
                    }
            }
            Self::Failed { diagnostic, .. } => diagnostic.len() <= text_bound,
        }
    }
    /// Deep heap ownership, excluding this enum's inline slot.
    #[must_use]
    pub fn owned_bytes(&self) -> Option<u64> {
        match self {
            Self::Delivered(receipts) => {
                let mut bytes = u64::try_from(receipts.len())
                    .ok()?
                    .checked_mul(u64::try_from(size_of::<DeliveryReceipt>()).ok()?)?;
                for row in receipts {
                    bytes = bytes.checked_add(u64::try_from(row.text.len()).ok()?)?;
                }
                Some(bytes)
            }
            Self::Nothing | Self::Stale => Some(0),
            Self::Refused { marker, explanation } => u64::try_from(explanation.len()).ok()?.checked_add(match marker {
                Some(marker) => u64::try_from(marker.path.len()).ok()?,
                None => 0,
            }),
            Self::Failed { diagnostic, .. } => u64::try_from(diagnostic.len()).ok(),
        }
    }
    /// Owned bytes of the boxed family record, including its inline fields.
    #[must_use]
    pub fn record_bytes(&self) -> Option<u64> {
        u64::try_from(size_of::<DeliveryRecord>()).ok()?.checked_add(self.owned_bytes()?)
    }
    /// Smith's current reply size, for the existing call-answer admission budget.
    /// This counts fields; the protocol layer alone encodes them.
    #[must_use]
    pub fn reply_bytes(&self) -> Option<u64> {
        match self {
            Self::Nothing | Self::Stale => Some(1),
            Self::Delivered(rows) => {
                let mut bytes = 5_u64;
                for row in rows {
                    bytes = bytes.checked_add(8)?.checked_add(u64::try_from(row.text.len()).ok()?)?;
                }
                Some(bytes)
            }
            Self::Refused { marker, explanation } => {
                let marker = match marker {
                    Some(marker) => 8_u64.checked_add(u64::try_from(marker.path.len()).ok()?)?,
                    None => 0,
                };
                6_u64.checked_add(marker)?.checked_add(u64::try_from(explanation.len()).ok()?)
            }
            Self::Failed { diagnostic, .. } => 18_u64.checked_add(u64::try_from(diagnostic.len()).ok()?),
        }
    }
    /// The largest allowed reply's owned bytes, including receipt containers.
    #[must_use]
    pub fn worst_case() -> Option<u64> {
        u64::from(Self::RECEIPTS)
            .checked_mul(u64::try_from(size_of::<DeliveryReceipt>()).ok()?.checked_add(u64::from(Self::TEXT))?)
    }
}
