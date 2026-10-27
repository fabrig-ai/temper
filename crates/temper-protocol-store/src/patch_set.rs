//! The application's one sequence and both kept families (jig's store.md, 10.3).
use jig_protocol_store::{KeptFamily, KeptMap, Keys, Owner, PatchSet};

pub const APPLICATION: PatchSet = PatchSet {
    owner: Owner::Application,
    name: "temper",
    patches: crate::patches::PATCHES,
    fresh: None,
    families: &[
        KeptFamily {
            name: "temper-store",
            texts: crate::patches::kept::temper_store::TEXTS,
            map: KeptMap::Store,
            keys: &[Keys { from: &[0], to: &[20], root: "Row" }, Keys { from: &[21], to: &[22], root: "Row" }],
            frames: None,
            flights: &[Keys { from: &[21], to: &[22], root: "Row" }],
        },
        KeptFamily {
            name: "temper-secrets",
            texts: temper_protocol_records::patches::kept::temper_secrets::TEXTS,
            map: KeptMap::Secrets,
            keys: &[Keys { from: &[0], to: &[128], root: "Stored" }],
            frames: None,
            flights: &[],
        },
    ],
};
