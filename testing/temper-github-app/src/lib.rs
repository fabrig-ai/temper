//! A shared fixture for GitHub stories (domain/testing.md, 2.4 and 2.6).
mod store;
use std::{net::SocketAddr, path::Path};
use temper_protocol_records::{self as codec, Item};

/// A fake GitHub App: the layer's App and the records temper keeps for it.
#[derive(Clone, Copy, Debug)]
pub struct App {
    pub id: u64,
    pub slug: &'static str,
    pub bot: u64,
    pub key: usize,
}
/// The fixture App on the layer's first fixed, public test key.
pub const APP: App = App { id: 4242, slug: "temper-test", bot: 4243, key: 0 };

/// The App's six versioned records (protocol/credentials.md, 2.1).
/// `origin` names the forge API, as temper's start checks it. Accepted ids
/// are sorted and deduplicated to match the record's canonical encoding.
#[must_use]
pub fn records(app: &App, forge: u16, accepted: &[u64], origin: SocketAddr) -> Box<[(u64, Box<[u8]>)]> {
    let mut accepted = accepted.to_vec();
    accepted.sort_unstable();
    accepted.dedup();
    let accepted: Vec<u8> = accepted.iter().flat_map(|id| id.to_le_bytes()).collect();
    let origin = codec::Origin {
        kind: skein_forge_connection::settings::Kind::GitHub,
        api: origin.to_string().into_boxed_str(),
    }
    .encode();
    let key = skein_fake_forge_github::keys::KEYS.get(app.key).expect("fixture key index");
    [
        (Item::User, app.bot.to_le_bytes().to_vec()),
        (Item::AppId, app.id.to_le_bytes().to_vec()),
        (Item::AppSlug, app.slug.as_bytes().to_vec()),
        (Item::AppKey, key.to_vec()),
        (Item::Accepted, accepted),
        (Item::Origin, origin.into_vec()),
    ]
    .into_iter()
    .map(|(item, bytes)| {
        (
            codec::number(forge, item),
            codec::encode(item, &bytes).expect("fixture item within schema").into_boxed_slice(),
        )
    })
    .collect()
}

/// Keep application records through jig's public store API on a private
/// shell loop. Returns after every keep is durable and all files close.
pub fn seed(data: &Path, records: &[(u64, Box<[u8]>)]) -> Result<(), String> {
    store::run(data, records, false).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_seeded_records_are_what_temper_loads_at_start() {
        let scratch = skein_scratch::Scratch::new("temper-github-app");
        let origin = "127.0.0.1:4321".parse().unwrap();
        let records = records(&APP, 17, &[3, 1, 3], origin);
        seed(scratch.path(), &records).unwrap();
        let loaded = store::run(scratch.path(), &[], true).unwrap();
        assert_eq!(loaded.len(), 6);
        let mut rows = codec::Rows::new();
        for (number, bytes) in &loaded {
            assert!(records.iter().any(|row| row == &(*number, bytes.clone())));
            rows.push(*number, skein_lib::Secret::new(bytes.clone()), 4096);
        }
        let rows = rows.forge(17).unwrap();
        assert_eq!(rows.get(Item::AppKey).unwrap(), skein_fake_forge_github::keys::KEYS[0]);
        let start = codec::access(
            skein_forge_connection::settings::Kind::GitHub,
            &codec::Origin { kind: skein_forge_connection::settings::Kind::GitHub, api: origin.to_string().into() },
            rows,
            &skein_forge_setup::item::Bounds { token_bytes: 512, name_bytes: 256, key_bytes: 4096, installations: 8 },
            false,
        )
        .unwrap();
        assert_eq!(start.access.user, APP.bot);
        assert_eq!(start.slug.as_deref(), Some(APP.slug.as_bytes()));
        assert_eq!(&*start.accepted, &[1, 3]);
        match start.access.credential {
            skein_forge::credential::Credential::App { id, key } => {
                assert_eq!(id, APP.id);
                assert_eq!(key.expose(), skein_fake_forge_github::keys::KEYS[0]);
            }
            skein_forge::credential::Credential::Token(_) => panic!("App access expected"),
        }
    }
}
