//! Family 25's bounded policy record (protocol/store.md, 2).
use super::{FITS, names, verification, w};
use crate::Undecodable;
use alloc::boxed::Box;
use skein_lib::List;
use temper_engine_domain_forge as d;
use temper_engine_domain_forge_tiers as p;

fn encode_tests(tests: &[Box<[u8]>]) -> List<w::FailingTest> {
    let mut values = List::with_capacity(u32::try_from(tests.len()).expect(FITS));
    for test in tests {
        values
            .push(w::FailingTest::new(&w::CEILINGS, w::FailingTestParts { test_id: test.clone() }).expect(FITS))
            .expect(FITS);
    }
    values
}
fn decode_tests(tests: List<w::FailingTest>) -> Result<Box<[Box<[u8]>]>, Undecodable> {
    let mut values = List::with_capacity(tests.len());
    for test in tests.into_boxed() {
        let test = test.into_parts().test_id;
        if test.is_empty() {
            return Err(Undecodable);
        }
        if let Some(previous) = values.as_slice().last()
            && previous >= &test
        {
            return Err(Undecodable);
        }
        values.push(test).expect(FITS);
    }
    Ok(values.into_boxed())
}
fn encode_landings(landings: &[p::Landing]) -> List<w::TierLanding> {
    let mut values = List::with_capacity(u32::try_from(landings.len()).expect(FITS));
    for landing in landings {
        values
            .push(
                w::TierLanding::new(&w::CEILINGS, w::TierLandingParts { commit: landing.commit, key: landing.key })
                    .expect(FITS),
            )
            .expect(FITS);
    }
    values
}
fn decode_landings(landings: List<w::TierLanding>) -> Box<[p::Landing]> {
    let mut values = List::with_capacity(landings.len());
    for landing in landings.into_boxed() {
        let w::TierLandingParts { commit, key } = landing.into_parts();
        values.push(p::Landing { commit, key }).expect(FITS);
    }
    values.into_boxed()
}
fn encode_finding(finding: &p::Finding) -> w::TierFinding {
    let mut blamed = List::with_capacity(u32::try_from(finding.blamed.len()).expect(FITS));
    for blame in &finding.blamed {
        blamed.push(verification::encode_blame(blame)).expect(FITS);
    }
    w::TierFinding::new(
        &w::CEILINGS,
        w::TierFindingParts {
            failing: verification::encode_failures(&finding.failing),
            flakes: encode_tests(&finding.flakes),
            blamed,
        },
    )
    .expect(FITS)
}
fn decode_finding(finding: w::TierFinding) -> Result<p::Finding, Undecodable> {
    let w::TierFindingParts { failing, flakes, blamed } = finding.into_parts();
    let mut values = List::with_capacity(blamed.len());
    for blame in blamed.into_boxed() {
        values.push(verification::decode_blame(blame)?).expect(FITS);
    }
    Ok(p::Finding {
        failing: verification::decode_failures(failing)?,
        flakes: decode_tests(flakes)?,
        blamed: values.into_boxed(),
    })
}
fn encode_run(run: &p::Run) -> w::TierRun {
    let part = match &run.part {
        p::Part::Whole => w::TierPart::Whole,
        p::Part::Rerun { failed } => w::TierPart::Rerun(
            w::TierTests::new(&w::CEILINGS, w::TierTestsParts { tests: encode_tests(failed) }).expect(FITS),
        ),
        p::Part::Probe { landing, tests, finding } => {
            let tests = match tests {
                p::Selection::All => w::TierSelection::All,
                p::Selection::Tests(tests) => w::TierSelection::Tests(
                    w::TierTests::new(&w::CEILINGS, w::TierTestsParts { tests: encode_tests(tests) }).expect(FITS),
                ),
            };
            w::TierPart::Probe(
                w::TierProbe::new(
                    &w::CEILINGS,
                    w::TierProbeParts { landing: *landing, tests, finding: encode_finding(finding) },
                )
                .expect(FITS),
            )
        }
    };
    w::TierRun::new(
        &w::CEILINGS,
        w::TierRunParts {
            number: run.number,
            tip: run.tip,
            key: run.key,
            landings: encode_landings(&run.landings),
            part,
            faults: run.faults,
        },
    )
    .expect(FITS)
}
fn decode_run(run: w::TierRun) -> Result<p::Run, Undecodable> {
    let w::TierRunParts { number, tip, key, landings, part, faults } = run.into_parts();
    let landings = decode_landings(landings);
    let part = match part {
        w::TierPart::Whole => p::Part::Whole,
        w::TierPart::Rerun(tests) => p::Part::Rerun { failed: decode_tests(tests.into_parts().tests)? },
        w::TierPart::Probe(probe) => {
            let w::TierProbeParts { landing, tests, finding } = probe.into_parts();
            let tests = match tests {
                w::TierSelection::All => p::Selection::All,
                w::TierSelection::Tests(tests) => p::Selection::Tests(decode_tests(tests.into_parts().tests)?),
            };
            let finding = decode_finding(finding)?;
            if usize::try_from(landing).or(Err(Undecodable))? >= landings.len()
                || finding.blamed.len() != landings.len()
            {
                return Err(Undecodable);
            }
            p::Part::Probe { landing, tests, finding }
        }
    };
    Ok(p::Run { number, tip, key, landings, part, faults })
}
#[expect(clippy::manual_map, reason = "step translations use explicit matches")]
pub(super) fn encode(row: &d::TierRow) -> w::TierRow {
    let mut flakes = List::with_capacity(u32::try_from(row.record.flakes.len()).expect(FITS));
    for flake in &row.record.flakes {
        flakes
            .push(
                w::TierFlake::new(&w::CEILINGS, w::TierFlakeParts { test: flake.test.clone(), count: flake.count })
                    .expect(FITS),
            )
            .expect(FITS);
    }
    let judged = match row.record.judged {
        Some(j) => Some(w::TierJudged::new(&w::CEILINGS, w::TierJudgedParts { tip: j.tip, key: j.key }).expect(FITS)),
        None => None,
    };
    let run = match &row.record.run {
        Some(run) => Some(encode_run(run)),
        None => None,
    };
    let record = w::TierRecord::new(
        &w::CEILINGS,
        w::TierRecordParts {
            sequence: row.record.sequence,
            judged,
            failing: verification::encode_failures(&row.record.failing),
            flakes,
            run,
        },
    )
    .expect(FITS);
    let branch = match row.branch {
        d::TierBranch::Absent => w::TierBranch::Absent,
        d::TierBranch::Made => w::TierBranch::Made,
        d::TierBranch::Pending { entry } => {
            w::TierBranch::Pending(w::Number::new(&w::CEILINGS, w::NumberParts { number: entry }).expect(FITS))
        }
    };
    w::TierRow::new(
        &w::CEILINGS,
        w::TierRowParts {
            repository: names::encode_repository(row.repository),
            tier: row.tier.clone(),
            record,
            pending: encode_landings(&row.pending),
            task: row.task,
            branch,
        },
    )
    .expect(FITS)
}
pub(super) fn decode(row: w::TierRow) -> Result<d::TierRow, Undecodable> {
    let w::TierRowParts { repository, tier, record, pending, task, branch } = row.into_parts();
    if tier.is_empty() {
        return Err(Undecodable);
    }
    for byte in &tier {
        if !byte.is_ascii_alphanumeric() && *byte != b'-' && *byte != b'_' {
            return Err(Undecodable);
        }
    }
    let w::TierRecordParts { sequence, judged, failing, flakes, run } = record.into_parts();
    let judged = match judged {
        Some(j) => {
            let w::TierJudgedParts { tip, key } = j.into_parts();
            Some(p::Judged { tip, key })
        }
        None => None,
    };
    let run = match run {
        Some(run) => Some(decode_run(run)?),
        None => None,
    };
    let mut values = List::with_capacity(flakes.len());
    for flake in flakes.into_boxed() {
        let w::TierFlakeParts { test, count } = flake.into_parts();
        if let Some(previous) = values.as_slice().last() {
            let previous: &p::Flake = previous;
            if previous.test >= test {
                return Err(Undecodable);
            }
        }
        values.push(p::Flake { test, count }).expect(FITS);
    }
    let branch = match branch {
        w::TierBranch::Absent => d::TierBranch::Absent,
        w::TierBranch::Made => d::TierBranch::Made,
        w::TierBranch::Pending(n) => d::TierBranch::Pending { entry: n.into_parts().number },
    };
    Ok(d::TierRow {
        repository: names::decode_repository(repository),
        tier,
        record: p::Record {
            sequence,
            judged,
            failing: verification::decode_failures(failing)?,
            flakes: values.into_boxed(),
            run,
        },
        pending: decode_landings(pending),
        task,
        branch,
    })
}
