//! Protection survives all three durable paths at `STORE_VERSION` 10 in place.
use skein_forge::api::Bypass;
use temper_protocol_store::{STORE_VERSION, Undecodable, records, v10};

#[path = "support/store.rs"]
mod fixture;
#[path = "support/protection.rs"]
mod protection;

#[test]
fn protection_fields_have_v10_golden_bytes_and_round_trip() {
    let bounds =
        temper_protocol_store::bounds(&fixture::LIMITS, fixture::LIMITS.client.op_bytes).expect("fixture bounds");
    assert_eq!(STORE_VERSION, 10);
    for (name, flags, bypass) in [
        ("unset", 0, Bypass::Never),
        ("queue", 1, Bypass::Never),
        ("signatures", 2, Bypass::Never),
        ("linear", 4, Bypass::Never),
        ("code-owners", 8, Bypass::Never),
        ("pulls-only", 0, Bypass::PullsOnly),
        ("always", 0, Bypass::Always),
        ("all", 15, Bypass::Always),
    ] {
        let [answer, repository, application] = protection::rows(protection::rule(flags, bypass));
        let bytes = records::encode(&answer);
        let path = skein_scratch::paths::package()
            .join("../../crates/temper-protocol-store/golden")
            .join(format!("protection-{name}.bin"));
        if std::env::var_os("SKEIN_CODEGEN_WRITE").is_some_and(|value| value == "1") {
            std::fs::write(&path, &bytes).expect("write protection golden");
        }
        assert_eq!(std::fs::read(path).expect("protection golden").as_slice(), bytes.as_ref(), "{name}");
        for value in [answer, repository, application] {
            let key = temper_protocol_store::keys::encode(&temper_engine_domain_forge::stored_key(&value), u32::MAX)
                .expect("record key");
            assert_eq!(records::row(&key, &records::encode(&value), &bounds), Ok(value), "{name}");
        }
    }
}

#[test]
fn every_protection_flag_combination_and_bypass_survives_the_store() {
    let bounds =
        temper_protocol_store::bounds(&fixture::LIMITS, fixture::LIMITS.client.op_bytes).expect("fixture bounds");
    for flags in 0..16 {
        for bypass in [Bypass::Never, Bypass::PullsOnly, Bypass::Always] {
            for row in protection::rows(protection::rule(flags, bypass)) {
                assert_eq!(records::decode(&records::encode(&row), &bounds), Ok(row));
            }
        }
    }
}

#[test]
fn protection_bounds_accept_the_limit_and_refuse_one_past() {
    let limits = fixture::LIMITS.client;
    let bounds =
        temper_protocol_store::bounds(&fixture::LIMITS, fixture::LIMITS.client.op_bytes).expect("fixture bounds");
    for (field, at) in
        [("branch", limits.answer_bytes), ("contexts bytes", limits.answer_bytes), ("contexts count", limits.rows)]
    {
        for extra in [0, 1] {
            let mut rule = protection::rule(15, Bypass::Always);
            rule.contexts = Box::new([]);
            let length = usize::try_from(at + extra).expect("small fixture bound");
            match field {
                "branch" => rule.branch = vec![b'x'; length].into_boxed_slice(),
                "contexts bytes" => {
                    // Both four-byte lengths are allowed beside each context.
                    // Fill every prefix slot so this also reaches the schema's byte limit.
                    let mut contexts = vec![Box::<[u8]>::default(); usize::try_from(limits.rows).expect("rows")];
                    contexts[0] = vec![b'x'; length].into_boxed_slice();
                    rule.contexts = contexts.into_boxed_slice();
                }
                "contexts count" => rule.contexts = vec![Box::<[u8]>::default(); length].into_boxed_slice(),
                _ => panic!("named protection bound"),
            }
            for row in protection::rows(rule) {
                let decoded = records::decode(&records::encode(&row), &bounds);
                if extra == 0 {
                    assert_eq!(decoded, Ok(row), "{field} at limit");
                } else {
                    assert_eq!(decoded, Err(Undecodable), "{field} one past");
                }
            }
        }
    }
}

#[test]
fn protection_rejects_invalid_boolean_and_bypass_tags() {
    // Empty branch and contexts, zero approvals, then the five boolean facts and bypass.
    let valid = [0_u8; 18];
    assert!(v10::Protection::decode(&v10::CEILINGS, &mut skein_lib::Reader::new(&valid)).is_ok());
    for at in 12..18 {
        let mut invalid = valid;
        invalid[at] = if at == 17 { 3 } else { 2 };
        assert!(v10::Protection::decode(&v10::CEILINGS, &mut skein_lib::Reader::new(&invalid)).is_err());
    }
}
