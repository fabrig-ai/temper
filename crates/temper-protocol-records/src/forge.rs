//! Forge startup access and the facade to skein setup (protocol/credentials.md, 2.1).
use crate::{CARRIED, FAMILY_FORGE, Item, Unreadable, decode, parse, v1};
use alloc::boxed::Box;
use skein_forge_connection::{Access, settings::Kind};
use skein_forge_setup::item;
use skein_lib::{Map, Secret, Writer};
const FORGES: u32 = 65536;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Origin {
    pub kind: Kind,
    pub api: Box<str>,
}
impl Origin {
    #[must_use]
    pub fn encode(&self) -> Box<[u8]> {
        let value = generated_origin(self).expect("configured origin within schema ceiling");
        let mut out = Writer::new(usize::try_from(value.measure()).expect("origin length"));
        value.encode(&mut out).expect("measured origin");
        out.finish()
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, Unreadable> {
        origin_from_record(crate::codec::decoded_origin(bytes)?)
    }
}
/// Display is part of the stored format, including IPv6 brackets.
#[must_use]
#[expect(clippy::disallowed_macros, reason = "credentials.md pins Rust's socket Display as Origin's bytes")]
pub fn origin(kind: Kind, address: &jig_protocol_config::Address) -> Origin {
    let api = match address {
        jig_protocol_config::Address::Addr(addr) => {
            alloc::format!("{}", addr.socket().expect("checked forge socket")).into_boxed_str()
        }
        jig_protocol_config::Address::Host { name, port } => {
            let port = skein_lib::Decimal::of(u64::from(*port));
            let mut exact = Writer::new(
                name.len().checked_add(1).expect("separator").checked_add(port.as_bytes().len()).expect("port"),
            );
            exact.put(name).expect("host");
            exact.put(b":").expect("separator");
            exact.put(port.as_bytes()).expect("port");
            let text = exact.finish();
            text_to_box(&text)
        }
    };
    Origin { kind, api }
}
#[expect(clippy::disallowed_methods, reason = "configuration has already validated host text")]
fn text_to_box(bytes: &[u8]) -> Box<str> {
    Box::from(core::str::from_utf8(bytes).expect("checked host text"))
}
#[derive(Debug)]
pub struct Rows {
    forges: Map<u16, ForgeRows>,
}
#[derive(Debug)]
pub struct ForgeRows {
    values: [Option<Secret>; 10],
    unreadable: bool,
    unreadable_items: u16,
}
#[expect(clippy::new_without_default, reason = "the programming model uses explicit constructors")]
impl Rows {
    #[must_use]
    pub fn new() -> Self {
        Self { forges: Map::with_capacity(FORGES) }
    }
    /// Only numbers below CARRIED are records; no value at or above it is read.
    pub fn push(&mut self, number: u64, bytes: Secret, value_bytes: u32) {
        if number >= CARRIED || number >> 48_u32 != u64::from(FAMILY_FORGE) {
            return;
        }
        let Ok(forge) = u16::try_from((number >> 16_u32) & 0xffff_ffff) else { return };
        if self.forges.get(&forge).is_none() {
            let row = ForgeRows {
                values: [None, None, None, None, None, None, None, None, None, None],
                unreadable: false,
                unreadable_items: 0,
            };
            self.forges.insert(forge, row).expect("all u16 forge numbers fit");
        }
        let row = self.forges.get_mut(&forge).expect("inserted forge");
        let Some((_, _, item)) = parse(number) else {
            row.unreadable = true;
            return;
        };
        let index = usize::from(item.code()).checked_sub(1).expect("positive item");
        let bit = 1_u16 << index;
        let decoded = if bytes.expose().len() > usize::try_from(value_bytes).expect("u32 bound") {
            Err(Unreadable)
        } else {
            decode(item, bytes.expose())
        };
        match decoded {
            Ok(value) => {
                *row.values.get_mut(index).expect("ten item codes") = Some(value);
                row.unreadable_items &= !bit;
            }
            Err(Unreadable) => row.unreadable_items |= bit,
        }
    }
    #[must_use]
    pub fn forge(&self, forge: u16) -> Option<&ForgeRows> {
        self.forges.get(&forge)
    }
}
impl ForgeRows {
    /// Read skein setup bytes translated from a validated schema row.
    pub fn get(&self, item: Item) -> Result<&[u8], NotConnected> {
        let index = usize::from(item.code()).checked_sub(1).expect("positive item");
        if self.unreadable || self.unreadable_items & (1_u16 << index) != 0 {
            return Err(NotConnected::Unreadable);
        }
        Ok(self.values.get(index).expect("ten codes").as_ref().ok_or(NotConnected::Incomplete)?.expose())
    }
    #[must_use]
    pub fn has(&self, item: Item) -> bool {
        self.get(item) != Err(NotConnected::Incomplete)
    }
}
#[derive(PartialEq, Eq, Debug)]
pub enum NotConnected {
    /// Start could not resolve this host; an operator diagnostic, never a wire value.
    Unreachable {
        host: Box<[u8]>,
    },
    NoRecords,
    Incomplete,
    Unreadable,
    OtherForge {
        kind: Kind,
        api: Box<str>,
    },
}
// The requested diagnostic vocabulary is a format boundary, not a step effect.
#[expect(clippy::disallowed_macros, reason = "temper-01 requires Display for operator diagnostics")]
#[expect(clippy::disallowed_methods, reason = "operator diagnostic formats a configuration-validated host")]
impl core::fmt::Display for NotConnected {
    fn fmt(&self, out: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Unreachable { host } => {
                out.write_str("unreachable: ")?;
                out.write_str(core::str::from_utf8(host).expect("checked host"))
            }
            Self::NoRecords => out.write_str("it has no records"),
            Self::Incomplete => out.write_str("its records are incomplete"),
            Self::Unreadable => out.write_str("its records are unreadable"),
            Self::OtherForge { kind, api } => write!(
                out,
                "its records were made for {} at {api}",
                match kind {
                    Kind::Forgejo => "forgejo",
                    Kind::GitHub => "github",
                }
            ),
        }
    }
}
#[derive(Debug)]
pub struct Start {
    pub access: Access,
    pub accepted: Box<[u64]>,
    pub slug: Option<Box<[u8]>>,
}
pub fn access(
    kind: Kind,
    expected: &Origin,
    rows: &ForgeRows,
    bounds: &item::Bounds,
    webhooks: bool,
) -> Result<Start, NotConnected> {
    if rows.unreadable || rows.unreadable_items != 0 {
        return Err(NotConnected::Unreadable);
    }
    // Validate even optional/unused items: one unreadable row refuses the set.
    for (index, bytes) in rows.values.iter().enumerate() {
        let Some(bytes) = bytes else { continue };
        let bytes = bytes.expose();
        if index == 9 {
            Origin::decode(bytes).or(Err(NotConnected::Unreadable))?;
            continue;
        }
        let kit = [
            item::Item::AdminToken,
            item::Item::ApiToken,
            item::Item::GitToken,
            item::Item::User,
            item::Item::WebhookSecret,
            item::Item::AppId,
            item::Item::AppSlug,
            item::Item::AppKey,
            item::Item::Accepted,
        ];
        if !item::fits(*kit.get(index).expect("nine setup items"), bytes, bounds) {
            return Err(NotConnected::Unreadable);
        }
    }
    let found = Origin::decode(rows.get(Item::Origin)?).or(Err(NotConnected::Unreadable))?;
    if found != *expected {
        return Err(NotConnected::OtherForge { kind: found.kind, api: found.api });
    }
    let other = match kind {
        Kind::Forgejo => rows.has(Item::AppKey),
        Kind::GitHub => rows.has(Item::ApiToken),
    };
    if other {
        return Err(NotConnected::OtherForge {
            kind: match kind {
                Kind::Forgejo => Kind::GitHub,
                Kind::GitHub => Kind::Forgejo,
            },
            api: found.api,
        });
    }
    let user = item::user(rows.get(Item::User)?).ok_or(NotConnected::Unreadable)?.get();
    let (credential, slug, accepted, webhook_secret) = match kind {
        Kind::Forgejo => (
            skein_forge::credential::Credential::Token(Secret::new(Box::from(rows.get(Item::ApiToken)?))),
            None,
            Box::from([]),
            if webhooks && rows.has(Item::WebhookSecret) {
                Some(Secret::new(Box::from(rows.get(Item::WebhookSecret)?)))
            } else {
                None
            },
        ),
        Kind::GitHub => (
            skein_forge::credential::Credential::App {
                id: item::app_id(rows.get(Item::AppId)?).ok_or(NotConnected::Unreadable)?.get(),
                key: Secret::new(Box::from(rows.get(Item::AppKey)?)),
            },
            Some(Box::from(rows.get(Item::AppSlug)?)),
            if rows.has(Item::Accepted) {
                item::accepted(rows.get(Item::Accepted)?, bounds.installations).ok_or(NotConnected::Unreadable)?
            } else {
                Box::new([])
            },
            None,
        ),
    };
    Ok(Start { access: Access { credential, user, webhook_secret }, accepted, slug })
}

fn generated_origin(value: &Origin) -> Result<v1::Origin, Unreadable> {
    let kind = match value.kind {
        Kind::Forgejo => v1::Kind::Forgejo,
        Kind::GitHub => v1::Kind::Github,
    };
    v1::Origin::new(&v1::CEILINGS, v1::OriginParts { kind, api: Box::from(value.api.as_bytes()) }).or(Err(Unreadable))
}
#[expect(clippy::disallowed_methods, reason = "the generated text decoder validates UTF-8")]
fn origin_from_record(value: v1::Origin) -> Result<Origin, Unreadable> {
    let parts = value.into_parts();
    if parts.api.is_empty() {
        return Err(Unreadable);
    }
    let kind = match parts.kind {
        v1::Kind::Forgejo => Kind::Forgejo,
        v1::Kind::Github => Kind::GitHub,
    };
    Ok(Origin { kind, api: Box::from(core::str::from_utf8(&parts.api).or(Err(Unreadable))?) })
}
