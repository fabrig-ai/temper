use skein_forge_connection::settings::Kind;
use skein_lib::{Rng, Secret};
use temper_protocol_records::{self as records, Origin, Rows};
#[test]
fn arbitrary_record_values_never_panic_and_origins_round_trip() {
    let mut rng = Rng::new(0x01_2026);
    let bounds = skein_forge_setup::item::Bounds { token_bytes: 64, name_bytes: 64, key_bytes: 1196, installations: 8 };
    for _ in 0..4000 {
        let bytes = temper_codecs_world::drawn(&mut rng, 4100);
        let _version = records::decode(records::Item::ApiToken, &bytes);
        let origin = Origin {
            kind: if rng.below(2) == 0 { Kind::Forgejo } else { Kind::GitHub },
            api: format!("fake-{}.test:{}", rng.next_u64(), rng.below(65536)).into_boxed_str(),
        };
        assert_eq!(Origin::decode(&origin.encode()), Ok(origin));
        if let Ok(origin) = Origin::decode(&bytes) {
            assert_eq!(Origin::decode(&origin.encode()), Ok(origin));
        }
        let mut rows = Rows::new();
        rows.push((1 << 48) | (1 << 16) | rng.between(0, 12), Secret::new(bytes.into_boxed_slice()), 4096);
        if let Some(rows) = rows.forge(1) {
            let origin = Origin { kind: Kind::GitHub, api: Box::from("fake.test:443") };
            let _access = records::access(Kind::GitHub, &origin, rows, &bounds, false);
        }
    }
}
