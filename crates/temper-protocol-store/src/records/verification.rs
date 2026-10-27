//! Family 26's bounded translations (protocol/store.md, 2).
use super::{FITS, names, w};
use crate::Undecodable;
use skein_lib::List;
use temper_engine_domain_forge as d;
use temper_engine_domain_forge_tiers::{Blame, Failures};

pub(super) fn encode_failures(value: &Failures) -> w::Failures {
    match value {
        Failures::Many => w::Failures::Many,
        Failures::Tests(tests) => {
            let mut encoded = List::with_capacity(u32::try_from(tests.len()).expect(FITS));
            for test in tests {
                encoded
                    .push(w::FailingTest::new(&w::CEILINGS, w::FailingTestParts { test_id: test.clone() }).expect(FITS))
                    .expect(FITS);
            }
            w::Failures::Tests(w::FailedTests::new(&w::CEILINGS, w::FailedTestsParts { tests: encoded }).expect(FITS))
        }
    }
}

pub(super) fn encode_blame(value: &Blame) -> w::Blame {
    match value {
        Blame::Cleared => w::Blame::Cleared,
        Blame::Owns(failures) => w::Blame::Owns(
            w::BlamedFailures::new(&w::CEILINGS, w::BlamedFailuresParts { failures: encode_failures(failures) })
                .expect(FITS),
        ),
        Blame::Suspect(failures) => w::Blame::Suspect(
            w::BlamedFailures::new(&w::CEILINGS, w::BlamedFailuresParts { failures: encode_failures(failures) })
                .expect(FITS),
        ),
    }
}

#[expect(clippy::manual_map, reason = "step translations use explicit matches without closures")]
pub(super) fn encode(row: &d::Verification) -> w::Verification {
    let mut changes = List::with_capacity(u32::try_from(row.changes.len()).expect(FITS));
    for task in &row.changes {
        changes.push(*task).expect(FITS);
    }
    let mut tiers = List::with_capacity(u32::try_from(row.tiers.len()).expect(FITS));
    for tier in &row.tiers {
        let verdict = match &tier.verdict {
            Some(verdict) => Some(
                w::TierVerdict::new(
                    &w::CEILINGS,
                    w::TierVerdictParts { run: verdict.run, blame: encode_blame(&verdict.blame) },
                )
                .expect(FITS),
            ),
            None => None,
        };
        tiers
            .push(
                w::TierVerification::new(
                    &w::CEILINGS,
                    w::TierVerificationParts { tier: tier.tier.clone(), verdict, repaired: tier.repaired },
                )
                .expect(FITS),
            )
            .expect(FITS);
    }
    w::Verification::new(
        &w::CEILINGS,
        w::VerificationParts {
            repository: names::encode_repository(row.repository),
            commit: row.commit,
            changes,
            tiers,
        },
    )
    .expect(FITS)
}

pub(super) fn decode_failures(value: w::Failures) -> Result<Failures, Undecodable> {
    Ok(match value {
        w::Failures::Many => Failures::Many,
        w::Failures::Tests(tests) => {
            let mut decoded = List::with_capacity(tests.tests().len());
            for test in tests.into_parts().tests.into_boxed() {
                let test = test.into_parts().test_id;
                if let Some(previous) = decoded.as_slice().last()
                    && previous >= &test
                {
                    return Err(Undecodable);
                }
                decoded.push(test).expect(FITS);
            }
            Failures::Tests(decoded.into_boxed())
        }
    })
}

pub(super) fn decode_blame(value: w::Blame) -> Result<Blame, Undecodable> {
    Ok(match value {
        w::Blame::Cleared => Blame::Cleared,
        w::Blame::Owns(value) => Blame::Owns(decode_failures(value.into_parts().failures)?),
        w::Blame::Suspect(value) => Blame::Suspect(decode_failures(value.into_parts().failures)?),
    })
}

pub(super) fn decode(row: w::Verification) -> Result<d::Verification, Undecodable> {
    let w::VerificationParts { repository, commit, changes, tiers } = row.into_parts();
    let mut decoded = List::with_capacity(tiers.len());
    for tier in tiers.into_boxed() {
        let w::TierVerificationParts { tier, verdict, repaired } = tier.into_parts();
        if tier.is_empty() {
            return Err(Undecodable);
        }
        for byte in &tier {
            if !byte.is_ascii_alphanumeric() && *byte != b'-' && *byte != b'_' {
                return Err(Undecodable);
            }
        }
        for previous in &decoded {
            let previous: &d::TierVerification = previous;
            if previous.tier == tier {
                return Err(Undecodable);
            }
        }
        let verdict = match verdict {
            Some(verdict) => {
                let w::TierVerdictParts { run, blame } = verdict.into_parts();
                Some(d::TierVerdict { run, blame: decode_blame(blame)? })
            }
            None => None,
        };
        decoded.push(d::TierVerification { tier, verdict, repaired }).expect(FITS);
    }
    Ok(d::Verification {
        repository: names::decode_repository(repository),
        commit,
        changes: changes.into_boxed(),
        tiers: decoded.into_boxed(),
    })
}
