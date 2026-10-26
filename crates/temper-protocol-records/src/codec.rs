//! Translate skein setup's item bytes at the schema boundary.
use super::{Item, Unreadable, v1};
use alloc::boxed::Box;
#[expect(clippy::disallowed_types, reason = "keep the public row encoder's Vec result")]
use alloc::vec::Vec;
use skein_lib::{List, Reader, Secret, Writer};

/// The carried value's existing aggregate budget (protocol/credentials.md, 2.2).
pub const PASTED_BYTES: usize = 4096;
/// Version u16, variant u8 and secret length u32 use seven bytes.
pub const PASTED_PAYLOAD: usize = PASTED_BYTES - 7;

#[expect(clippy::disallowed_types, clippy::disallowed_methods, reason = "keep the public row encoder's Vec result")]
pub fn encode(item: Item, bytes: &[u8]) -> Result<Vec<u8>, Unreadable> {
    if bytes.len() > usize::try_from(v1::CEILINGS.app_key_value).expect("schema ceiling") {
        return Err(Unreadable);
    }
    let item = generated(item, bytes)?;
    let row = v1::Stored::new(&v1::CEILINGS, v1::StoredParts { item }).or(Err(Unreadable))?;
    let mut out = Writer::new(usize::try_from(row.measure()).expect("row length"));
    row.encode(&mut out).expect("measured row");
    Ok(out.finish().into_vec())
}
pub fn decode(item: Item, bytes: &[u8]) -> Result<Secret, Unreadable> {
    let row = v1::Stored::decode(&v1::CEILINGS, &mut Reader::new(bytes)).or(Err(Unreadable))?;
    let (found, value) = raw(row.into_parts().item)?;
    if found != item {
        return Err(Unreadable);
    }
    Ok(value)
}

fn generated(item: Item, bytes: &[u8]) -> Result<v1::Item, Unreadable> {
    let limits = &v1::CEILINGS;
    Ok(match item {
        Item::AdminToken => v1::Item::AdminToken(
            v1::AdminToken::new(limits, v1::AdminTokenParts { value: Secret::new(Box::from(bytes)) })
                .or(Err(Unreadable))?,
        ),
        Item::ApiToken => v1::Item::ApiToken(
            v1::ApiToken::new(limits, v1::ApiTokenParts { value: Secret::new(Box::from(bytes)) })
                .or(Err(Unreadable))?,
        ),
        Item::GitToken => v1::Item::GitToken(
            v1::GitToken::new(limits, v1::GitTokenParts { value: Secret::new(Box::from(bytes)) })
                .or(Err(Unreadable))?,
        ),
        Item::User => v1::Item::User(
            v1::User::new(limits, v1::UserParts { value: u64::from_le_bytes(bytes.try_into().or(Err(Unreadable))?) })
                .or(Err(Unreadable))?,
        ),
        Item::WebhookSecret => v1::Item::WebhookSecret(
            v1::WebhookSecret::new(limits, v1::WebhookSecretParts { value: Secret::new(Box::from(bytes)) })
                .or(Err(Unreadable))?,
        ),
        Item::AppId => v1::Item::AppId(
            v1::AppId::new(limits, v1::AppIdParts { value: u64::from_le_bytes(bytes.try_into().or(Err(Unreadable))?) })
                .or(Err(Unreadable))?,
        ),
        Item::AppSlug => v1::Item::AppSlug(
            v1::AppSlug::new(limits, v1::AppSlugParts { value: Box::from(bytes) }).or(Err(Unreadable))?,
        ),
        Item::AppKey => v1::Item::AppKey(
            v1::AppKey::new(limits, v1::AppKeyParts { value: Secret::new(Box::from(bytes)) }).or(Err(Unreadable))?,
        ),
        Item::Accepted => v1::Item::Accepted(accepted(bytes)?),
        Item::Origin => v1::Item::Origin(decoded_origin(bytes)?),
    })
}
fn accepted(bytes: &[u8]) -> Result<v1::Accepted, Unreadable> {
    let chunks = bytes.chunks_exact(8);
    let count = u32::try_from(chunks.len()).or(Err(Unreadable))?;
    if !chunks.remainder().is_empty() || count > v1::CEILINGS.accepted_values {
        return Err(Unreadable);
    }
    let mut values = List::with_capacity(count);
    for chunk in chunks {
        values.push(u64::from_le_bytes(chunk.try_into().or(Err(Unreadable))?)).or(Err(Unreadable))?;
    }
    v1::Accepted::new(&v1::CEILINGS, v1::AcceptedParts { values }).or(Err(Unreadable))
}
pub(super) fn decoded_origin(bytes: &[u8]) -> Result<v1::Origin, Unreadable> {
    let value = v1::Origin::decode(&v1::CEILINGS, &mut Reader::new(bytes)).or(Err(Unreadable))?;
    if value.api().is_empty() {
        return Err(Unreadable);
    }
    Ok(value)
}
fn origin_bytes(value: &v1::Origin) -> Result<Secret, Unreadable> {
    if value.api().is_empty() {
        return Err(Unreadable);
    }
    let mut out = Writer::new(usize::try_from(value.measure()).expect("origin length"));
    value.encode(&mut out).expect("measured origin");
    Ok(Secret::new(out.finish()))
}
// Skein setup's item boundary remains little-endian; the generated list on
// disk is big-endian. Keep this conversion usable without the forge runtime.
fn accepted_bytes(value: &v1::Accepted) -> Secret {
    let mut out = Writer::new(value.values().as_slice().len().checked_mul(8).expect("accepted length"));
    for id in value.values().as_slice() {
        out.put(&id.to_le_bytes()).expect("measured accepted item");
    }
    Secret::new(out.finish())
}
fn raw(value: v1::Item) -> Result<(Item, Secret), Unreadable> {
    Ok(match value {
        v1::Item::AdminToken(value) => (Item::AdminToken, value.into_parts().value),
        v1::Item::ApiToken(value) => (Item::ApiToken, value.into_parts().value),
        v1::Item::GitToken(value) => (Item::GitToken, value.into_parts().value),
        v1::Item::User(value) => (Item::User, Secret::new(Box::from(value.value().to_le_bytes()))),
        v1::Item::WebhookSecret(value) => (Item::WebhookSecret, value.into_parts().value),
        v1::Item::AppId(value) => (Item::AppId, Secret::new(Box::from(value.value().to_le_bytes()))),
        v1::Item::AppSlug(value) => (Item::AppSlug, Secret::new(value.into_parts().value)),
        v1::Item::AppKey(value) => (Item::AppKey, value.into_parts().value),
        v1::Item::Accepted(value) => (Item::Accepted, accepted_bytes(&value)),
        v1::Item::Origin(value) => (Item::Origin, origin_bytes(&value)?),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PastedError {
    Version,
    Unreadable,
}
impl PastedError {
    #[must_use]
    pub const fn message(self) -> &'static [u8] {
        match self {
            Self::Version => b"pasted with another version of temper: paste it again",
            Self::Unreadable => b"the pasted value is unreadable",
        }
    }
}
/// One item, as every existing producer and the setup reader accept.
pub fn encode_pasted(code: u16, bytes: &[u8]) -> Result<Secret, Unreadable> {
    if bytes.is_empty() || bytes.len() > PASTED_BYTES {
        return Err(Unreadable);
    }
    let item = if code == 11 {
        v1::PastedItem::Pem(
            v1::Pem::new(&v1::CEILINGS, v1::PemParts { value: Secret::new(Box::from(bytes)) }).or(Err(Unreadable))?,
        )
    } else {
        let (_, _, item) = super::parse((u64::from(super::FAMILY_FORGE) << 48) | u64::from(code)).ok_or(Unreadable)?;
        pasted_item(generated(item, bytes)?)
    };
    let row = v1::Pasted::new(&v1::CEILINGS, v1::PastedParts { item }).or(Err(Unreadable))?;
    let size = usize::try_from(row.measure()).expect("pasted length");
    if size > PASTED_BYTES {
        return Err(Unreadable);
    }
    let mut out = Writer::new(size);
    row.encode(&mut out).expect("measured pasted value");
    Ok(Secret::new(out.finish()))
}
pub fn decode_pasted(bytes: &[u8]) -> Result<(u16, Secret), PastedError> {
    if bytes.len() > PASTED_BYTES {
        return Err(PastedError::Unreadable);
    }
    let row = match v1::Pasted::decode(&v1::CEILINGS, &mut Reader::new(bytes)) {
        Ok(row) => row,
        Err(problem) => {
            return Err(if problem.reason == skein_codec::Reason::Version {
                PastedError::Version
            } else {
                PastedError::Unreadable
            });
        }
    };
    let item = match row.into_parts().item {
        v1::PastedItem::Pem(value) => return Ok((11, value.into_parts().value)),
        v1::PastedItem::AdminToken(value) => v1::Item::AdminToken(value),
        v1::PastedItem::ApiToken(value) => v1::Item::ApiToken(value),
        v1::PastedItem::GitToken(value) => v1::Item::GitToken(value),
        v1::PastedItem::User(value) => v1::Item::User(value),
        v1::PastedItem::WebhookSecret(value) => v1::Item::WebhookSecret(value),
        v1::PastedItem::AppId(value) => v1::Item::AppId(value),
        v1::PastedItem::AppSlug(value) => v1::Item::AppSlug(value),
        v1::PastedItem::AppKey(value) => v1::Item::AppKey(value),
        v1::PastedItem::Accepted(value) => v1::Item::Accepted(value),
        v1::PastedItem::Origin(value) => v1::Item::Origin(value),
    };
    let (item, value) = raw(item).or(Err(PastedError::Unreadable))?;
    Ok((item.code(), value))
}
fn pasted_item(item: v1::Item) -> v1::PastedItem {
    match item {
        v1::Item::AdminToken(value) => v1::PastedItem::AdminToken(value),
        v1::Item::ApiToken(value) => v1::PastedItem::ApiToken(value),
        v1::Item::GitToken(value) => v1::PastedItem::GitToken(value),
        v1::Item::User(value) => v1::PastedItem::User(value),
        v1::Item::WebhookSecret(value) => v1::PastedItem::WebhookSecret(value),
        v1::Item::AppId(value) => v1::PastedItem::AppId(value),
        v1::Item::AppSlug(value) => v1::PastedItem::AppSlug(value),
        v1::Item::AppKey(value) => v1::PastedItem::AppKey(value),
        v1::Item::Accepted(value) => v1::PastedItem::Accepted(value),
        v1::Item::Origin(value) => v1::PastedItem::Origin(value),
    }
}
