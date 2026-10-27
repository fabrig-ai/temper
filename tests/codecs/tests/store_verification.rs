//! Verification and the change's post-merge states at the v10 baseline.
#[path = "support/store.rs"]
mod fixture;
#[path = "support/tiers.rs"]
mod tiers;
#[path = "support/verification.rs"]
mod verification;
use temper_engine_domain_forge::{Key, Stored, TierVerdict};
use temper_engine_domain_forge_tiers::{Blame, Failures};
use temper_protocol_store::{keys, records};

fn golden(name: &str, row: Stored) {
    let bounds = temper_protocol_store::bounds(&fixture::LIMITS).expect("fixture bounds");
    let bytes = records::encode(&row);
    let path = skein_scratch::paths::package().join("../../crates/temper-protocol-store/golden").join(name);
    if std::env::var_os("SKEIN_CODEGEN_WRITE").is_some_and(|value| value == "1") {
        std::fs::write(&path, &bytes).expect("write golden");
    }
    assert_eq!(std::fs::read(path).expect("committed golden").as_slice(), bytes.as_ref());
    let key = keys::encode(&temper_engine_domain_forge::stored_key(&row), u32::MAX).expect("key fits");
    assert_eq!(records::row(&key, &bytes, &bounds), Ok(row));
}

#[test]
fn verification_smallest_and_fullest_rows_have_v10_goldens_and_round_trip() {
    for (name, row) in
        [("verification-smallest.bin", verification::smallest()), ("verification-fullest.bin", verification::fullest())]
    {
        golden(name, row);
    }
}

#[test]
fn red_and_cleared_changes_have_v10_goldens_and_round_trip() {
    use temper_engine_domain_forge_change::State;
    for (name, state) in [
        ("change-red.bin", State::Red { merge: verification::COMMIT }),
        ("change-cleared.bin", State::Cleared { merge: verification::COMMIT }),
    ] {
        golden(name, Stored::Change(verification::change_row(state)));
    }
}

#[test]
fn verification_verdicts_keep_cleared_and_many_failures() {
    for blame in [Blame::Cleared, Blame::Owns(Failures::Many), Blame::Suspect(Failures::Many)] {
        let Stored::Verification(mut row) = verification::smallest() else { panic!("fixture verification") };
        row.tiers[0].verdict = Some(TierVerdict { run: 1, blame });
        let row = Stored::Verification(row);
        let bounds = temper_protocol_store::bounds(&fixture::LIMITS).expect("bounds");
        assert_eq!(records::decode(&records::encode(&row), &bounds), Ok(row));
    }
}

#[test]
fn verification_refuses_each_field_one_past_its_runtime_bound() {
    let row = verification::fullest();
    let bytes = records::encode(&row);
    for limits in [
        temper_engine_domain_forge::Limits { tiers: fixture::LIMITS.tiers - 1, ..fixture::LIMITS },
        temper_engine_domain_forge::Limits { failing_tests: fixture::LIMITS.failing_tests - 1, ..fixture::LIMITS },
        temper_engine_domain_forge::Limits { test_id_bytes: fixture::LIMITS.test_id_bytes - 1, ..fixture::LIMITS },
    ] {
        let bounds = temper_protocol_store::bounds(&limits).expect("smaller bounds");
        assert_eq!(records::decode(&bytes, &bounds), Err(temper_protocol_store::Undecodable));
    }
}

#[test]
fn verification_keys_use_family_26_and_require_the_whole_key() {
    let key = Key::Verification { repository: verification::REPOSITORY, commit: verification::COMMIT };
    let bytes = keys::encode(&key, u32::MAX).expect("key fits");
    assert_eq!(bytes[0], 26);
    assert_eq!(keys::decode(&bytes, &fixture::LIMITS), Some(key));
    for length in 0..bytes.len() {
        assert_eq!(keys::decode(&bytes[..length], &fixture::LIMITS), None);
    }
    let row = verification::smallest();
    let bounds = temper_protocol_store::bounds(&fixture::LIMITS).expect("bounds");
    let other = keys::encode(&Key::Verification { repository: verification::REPOSITORY, commit: [9; 32] }, u32::MAX)
        .expect("key fits");
    assert_eq!(records::row(&other, &records::encode(&row), &bounds), Err(temper_protocol_store::Undecodable));
}

#[test]
fn tier_smallest_and_fullest_rows_have_v10_goldens_and_round_trip() {
    for (name, row) in [("tier-smallest.bin", tiers::smallest()), ("tier-fullest.bin", tiers::fullest())] {
        golden(name, row);
    }
}
#[test]
fn tier_ranges_and_tests_refuse_one_past_each_runtime_bound() {
    let bytes = records::encode(&tiers::fullest());
    for limits in [
        temper_engine_domain_forge::Limits { changes: fixture::LIMITS.changes - 1, ..fixture::LIMITS },
        temper_engine_domain_forge::Limits { failing_tests: fixture::LIMITS.failing_tests - 1, ..fixture::LIMITS },
        temper_engine_domain_forge::Limits { test_id_bytes: fixture::LIMITS.test_id_bytes - 1, ..fixture::LIMITS },
    ] {
        let bounds = temper_protocol_store::bounds(&limits).expect("bounds");
        assert_eq!(records::decode(&bytes, &bounds), Err(temper_protocol_store::Undecodable));
    }
}
