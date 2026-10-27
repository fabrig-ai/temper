use temper_engine_domain_forge::{Stored, TierVerdict, TierVerification, Verification};
use temper_engine_domain_forge_tiers::{Blame, Failures};

// Small's bounds (domain/after-landing.md, 4).
pub const FAILING_TESTS: usize = 16;
pub const TEST_ID_BYTES: usize = 256;
pub const REPOSITORY: skein_forge::api::Repository = skein_forge::api::Repository { forge: 1, repository: 2 };
pub const COMMIT: [u8; 32] = [3; 32];
pub const TASK: u64 = 51;

pub fn smallest() -> Stored {
    Stored::Verification(Verification {
        repository: REPOSITORY,
        commit: COMMIT,
        changes: Box::new([TASK]),
        tiers: Box::new([TierVerification { tier: b"browser".as_slice().into(), verdict: None, repaired: None }]),
    })
}

pub fn fullest() -> Stored {
    let mut tests = Vec::with_capacity(FAILING_TESTS);
    for test in 0..FAILING_TESTS {
        tests.push(vec![u8::try_from(test).expect("fixture test index"); TEST_ID_BYTES].into_boxed_slice());
    }
    let failures = Failures::Tests(tests.into_boxed_slice());
    Stored::Verification(Verification {
        repository: REPOSITORY,
        commit: COMMIT,
        changes: Box::new([TASK]),
        tiers: Box::new([
            TierVerification {
                tier: Box::new([b'a'; 32]),
                verdict: Some(TierVerdict { run: 1, blame: Blame::Owns(failures.clone()) }),
                repaired: Some([4; 32]),
            },
            TierVerification {
                tier: Box::new([b'b'; 32]),
                verdict: Some(TierVerdict { run: 2, blame: Blame::Suspect(failures) }),
                repaired: None,
            },
        ]),
    })
}

pub fn change_row(state: temper_engine_domain_forge_change::State) -> temper_engine_domain_forge::ChangeRow {
    use temper_engine_domain_forge_change as change;
    temper_engine_domain_forge::ChangeRow {
        task: TASK,
        repository: REPOSITORY,
        branch: b"temper/51".as_slice().into(),
        base: b"main".as_slice().into(),
        title: b"Fix".as_slice().into(),
        body: Box::new([]),
        priority: 0,
        change: change::Change {
            task: TASK,
            state,
            gates: Box::new([]),
            clean: Box::new([]),
            gate_failures: Box::new([]),
            repairs: 0,
            resolutions: 0,
            updates: 0,
            since: skein_lib::Wall::EPOCH,
            ready_since: None,
            owns_turn: false,
            last_head: Some(COMMIT),
        },
        pull: Some(1),
        pending: None,
        effect: change::EffectResult::Made,
        delegate: None,
        delegate_status: change::Status::Passed,
        verdicts: Box::new([]),
        gate_remarks: Box::new([]),
        drift: None,
        base_repair: false,
        queue_repair: None,
        queue_repairs: 0,
    }
}
