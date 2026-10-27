use temper_engine_domain_forge::{Stored, TierBranch, TierRow};
use temper_engine_domain_forge_tiers as p;
const REPOSITORY: skein_forge::api::Repository = skein_forge::api::Repository { forge: 1, repository: 2 };
const TIP: [u8; 32] = [3; 32];
const RANGE: usize = 17; // small's 2 * changes (8) + 1.
const TESTS: usize = 16; // small's failing_tests.
const TEST_BYTES: usize = 256; // small's test_id_bytes.
pub fn smallest() -> Stored {
    Stored::Tier(TierRow {
        repository: REPOSITORY,
        tier: b"browser".as_slice().into(),
        record: p::Record {
            sequence: 0,
            judged: None,
            failing: p::Failures::Tests(Box::new([])),
            flakes: Box::new([]),
            run: None,
        },
        pending: Box::new([]),
        task: None,
        branch: TierBranch::Absent,
    })
}
pub fn fullest() -> Stored {
    let tests: Box<[_]> = (0..TESTS)
        .map(|n| vec![u8::try_from(n + 1).expect("bounded test index"); TEST_BYTES].into_boxed_slice())
        .collect();
    let landings: Box<[_]> = (0..RANGE)
        .map(|n| p::Landing {
            commit: [u8::try_from(n).expect("bounded landing index"); 32],
            key: [u8::try_from(n).expect("bounded landing index"); 32],
        })
        .collect();
    let failures = p::Failures::Tests(tests.clone());
    Stored::Tier(TierRow {
        repository: REPOSITORY,
        tier: Box::new([b'a'; 32]),
        record: p::Record {
            sequence: 1,
            judged: Some(p::Judged { tip: TIP, key: TIP }),
            failing: failures.clone(),
            flakes: tests.iter().map(|test| p::Flake { test: test.clone(), count: u32::MAX }).collect(),
            run: Some(p::Run {
                number: 2,
                tip: TIP,
                key: TIP,
                landings: landings.clone(),
                faults: 2,
                part: p::Part::Probe {
                    landing: 1,
                    tests: p::Selection::Tests(tests.clone()),
                    finding: p::Finding {
                        failing: failures.clone(),
                        flakes: tests,
                        blamed: vec![p::Blame::Owns(failures); RANGE].into_boxed_slice(),
                    },
                },
            }),
        },
        pending: landings,
        task: Some(51),
        branch: TierBranch::Pending { entry: 11 },
    })
}
