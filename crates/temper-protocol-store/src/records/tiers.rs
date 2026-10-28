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
#[expect(clippy::manual_map, reason = "step translations use explicit matches")]
fn encode_pending_landing(landing: &d::TierLanding) -> w::TierPending {
    let key = match landing.key {
        Some(commit) => Some(w::Commit::new(&w::CEILINGS, w::CommitParts { commit }).expect(FITS)),
        None => None,
    };
    w::TierPending::new(&w::CEILINGS, w::TierPendingParts { commit: landing.commit, key }).expect(FITS)
}
#[expect(clippy::manual_map, reason = "step translations use explicit matches")]
fn decode_pending_landing(landing: w::TierPending) -> d::TierLanding {
    let w::TierPendingParts { commit, key } = landing.into_parts();
    let key = match key {
        Some(key) => Some(key.into_parts().commit),
        None => None,
    };
    d::TierLanding { commit, key }
}
fn encode_pending(landings: &[d::TierLanding]) -> List<w::TierPending> {
    let mut values = List::with_capacity(u32::try_from(landings.len()).expect(FITS));
    for landing in landings {
        values.push(encode_pending_landing(landing)).expect(FITS);
    }
    values
}
fn decode_pending(landings: List<w::TierPending>) -> Box<[d::TierLanding]> {
    let mut values = List::with_capacity(landings.len());
    for landing in landings.into_boxed() {
        values.push(decode_pending_landing(landing)).expect(FITS);
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
#[expect(clippy::too_many_lines, reason = "one complete durable tier row translation")]
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
    let flight = match &row.flight {
        Some(f) => {
            let mut pending = List::with_capacity(u32::try_from(f.pending.len()).expect(FITS));
            for attempt in &f.pending {
                pending.push(super::answers::encode_job_attempt(attempt)).expect(FITS);
            }
            Some(
                w::TierFlight::new(
                    &w::CEILINGS,
                    w::TierFlightParts {
                        next: match f.next {
                            Some(at) => Some(at.as_nanos()),
                            None => None,
                        },
                        entry: f.entry,
                        start: match f.start {
                            Some(at) => Some(at.as_nanos()),
                            None => None,
                        },
                        since: match f.since {
                            Some(at) => Some(at.as_nanos()),
                            None => None,
                        },
                        waiting: f.waiting,
                        fault: f.fault,
                        report: match &f.report {
                            Some(run) => Some(encode_receipt_run(run)),
                            None => None,
                        },
                        pending,
                    },
                )
                .expect(FITS),
            )
        }
        None => None,
    };
    let result = match &row.result {
        Some(r) => Some(
            w::TierResult::new(
                &w::CEILINGS,
                w::TierResultParts {
                    comments: encode_comments(&r.comments),
                    told: encode_told(&r.told),
                    verdict: w::TierJudgement::new(
                        &w::CEILINGS,
                        w::TierJudgementParts {
                            number: r.verdict.number,
                            tip: r.verdict.tip,
                            key: r.verdict.key,
                            finding: encode_finding(&p::Finding {
                                failing: r.verdict.failing.clone(),
                                flakes: r.verdict.flakes.clone(),
                                blamed: r.verdict.landings.clone(),
                            }),
                        },
                    )
                    .expect(FITS),
                    run: w::ReceiptRun::new(
                        &w::CEILINGS,
                        w::ReceiptRunParts { id: r.run.id, number: r.run.number, url: r.run.url.clone() },
                    )
                    .expect(FITS),
                    received: r.received,
                    status: encode_branch(r.status),
                    applied: r.applied,
                },
            )
            .expect(FITS),
        ),
        None => None,
    };
    w::TierRow::new(
        &w::CEILINGS,
        w::TierRowParts {
            dropped_reds: row.dropped_reds,
            held: encode_held(row.held.as_ref()),
            reds: encode_reds(&row.reds),
            repository: names::encode_repository(row.repository),
            tier: row.tier.clone(),
            record,
            pending: encode_pending(&row.pending),
            baseline: match &row.baseline {
                Some(baseline) => Some(encode_pending_landing(baseline)),
                None => None,
            },
            task: row.task,
            branch,
            probe: match &row.probe {
                Some(probe) => Some(
                    w::TierProbeBranch::new(
                        &w::CEILINGS,
                        w::TierProbeBranchParts { landing: probe.landing, branch: encode_branch(probe.branch) },
                    )
                    .expect(FITS),
                ),
                None => None,
            },
            flight,
            result,
        },
    )
    .expect(FITS)
}
#[expect(
    clippy::too_many_lines,
    clippy::manual_map,
    reason = "one durable row translation validates the related operation states without callbacks"
)]
pub(super) fn decode(row: w::TierRow) -> Result<d::TierRow, Undecodable> {
    let w::TierRowParts {
        dropped_reds,
        held,
        reds,
        repository,
        tier,
        record,
        pending,
        baseline,
        task,
        branch,
        probe,
        flight,
        result,
    } = row.into_parts();
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
    let flight = match flight {
        Some(f) => {
            let w::TierFlightParts { next, entry, start, since, waiting, fault, report, pending } = f.into_parts();
            let mut jobs = List::with_capacity(pending.len());
            for attempt in pending.into_boxed() {
                jobs.push(super::answers::decode_job_attempt(attempt)).expect(FITS);
            }
            Some(d::TierFlight {
                next: match next {
                    Some(at) => Some(skein_lib::Time::from_nanos(at)),
                    None => None,
                },
                entry,
                start: match start {
                    Some(at) => Some(skein_lib::Time::from_nanos(at)),
                    None => None,
                },
                since: match since {
                    Some(at) => Some(skein_lib::Time::from_nanos(at)),
                    None => None,
                },
                waiting,
                fault,
                report: match report {
                    Some(run) => Some(decode_receipt_run(run)),
                    None => None,
                },
                pending: jobs.into_boxed(),
            })
        }
        None => None,
    };
    let result = match result {
        Some(r) => {
            let w::TierResultParts { comments, told, verdict, run, received, status, applied } = r.into_parts();
            let w::TierJudgementParts { number, tip, key, finding } = verdict.into_parts();
            let finding = decode_finding(finding)?;
            let w::ReceiptRunParts { id, number: run_number, url } = run.into_parts();
            Some(d::TierResult {
                comments: decode_comments(comments)?,
                told: decode_told(told),
                verdict: p::Verdict {
                    number,
                    tip,
                    key,
                    failing: finding.failing,
                    flakes: finding.flakes,
                    landings: finding.blamed,
                },
                run: d::ReceiptRun { id, number: run_number, url },
                received,
                status: decode_branch(status),
                applied,
            })
        }
        None => None,
    };
    let probe = match probe {
        Some(probe) => {
            let w::TierProbeBranchParts { landing, branch } = probe.into_parts();
            let run = run.as_ref().ok_or(Undecodable)?;
            if usize::try_from(landing).or(Err(Undecodable))? >= run.landings.len() {
                return Err(Undecodable);
            }
            Some(d::TierProbeBranch { landing, branch: decode_branch(branch) })
        }
        None => None,
    };
    if held.is_some() && (run.is_none() || task.is_none()) {
        return Err(Undecodable);
    }
    if (baseline.is_some() && (judged.is_some() || run.is_some()))
        || (flight.is_some() && (run.is_none() || task.is_none()))
        || (result.is_some() && run.is_none())
    {
        return Err(Undecodable);
    }
    if let Some(result) = &result {
        let run = run.as_ref().ok_or(Undecodable)?;
        if result.verdict.number != run.number
            || result.verdict.tip != run.tip
            || result.verdict.key != run.key
            || result.verdict.landings.len() != run.landings.len()
        {
            return Err(Undecodable);
        }
    }
    Ok(d::TierRow {
        dropped_reds,
        held: decode_held(held),
        reds: decode_reds(reds)?,
        repository: names::decode_repository(repository),
        tier,
        record: p::Record {
            sequence,
            judged,
            failing: verification::decode_failures(failing)?,
            flakes: values.into_boxed(),
            run,
        },
        pending: decode_pending(pending),
        baseline: match baseline {
            Some(baseline) => Some(decode_pending_landing(baseline)),
            None => None,
        },
        task,
        branch,
        probe,
        flight,
        result,
    })
}

fn encode_branch(branch: d::TierBranch) -> w::TierBranch {
    match branch {
        d::TierBranch::Absent => w::TierBranch::Absent,
        d::TierBranch::Made => w::TierBranch::Made,
        d::TierBranch::Pending { entry } => {
            w::TierBranch::Pending(w::Number::new(&w::CEILINGS, w::NumberParts { number: entry }).expect(FITS))
        }
    }
}
fn decode_branch(branch: w::TierBranch) -> d::TierBranch {
    match branch {
        w::TierBranch::Absent => d::TierBranch::Absent,
        w::TierBranch::Made => d::TierBranch::Made,
        w::TierBranch::Pending(entry) => d::TierBranch::Pending { entry: entry.into_parts().number },
    }
}

fn encode_receipt_run(run: &d::ReceiptRun) -> w::ReceiptRun {
    w::ReceiptRun::new(&w::CEILINGS, w::ReceiptRunParts { id: run.id, number: run.number, url: run.url.clone() })
        .expect(FITS)
}
fn decode_receipt_run(run: w::ReceiptRun) -> d::ReceiptRun {
    let w::ReceiptRunParts { id, number, url } = run.into_parts();
    d::ReceiptRun { id, number, url }
}

fn encode_told(told: &[d::LateTold]) -> List<w::LateTold> {
    let mut values = List::with_capacity(u32::try_from(told.len()).expect(FITS));
    for mark in told {
        values
            .push(
                w::LateTold::new(&w::CEILINGS, w::LateToldParts { landing: mark.landing, goal: mark.goal })
                    .expect(FITS),
            )
            .expect(FITS);
    }
    values
}
fn decode_told(told: List<w::LateTold>) -> Box<[d::LateTold]> {
    let mut values = List::with_capacity(told.len());
    for mark in told.into_boxed() {
        let w::LateToldParts { landing, goal } = mark.into_parts();
        values.push(d::LateTold { landing, goal }).expect(FITS);
    }
    values.into_boxed()
}
#[expect(clippy::manual_map, reason = "record translations use explicit matches without callbacks")]
fn encode_held(held: Option<&d::TierHeld>) -> Option<w::TierHeld> {
    use temper_engine_domain_forge_landing::Fault;
    let held = held?;
    let fault = match held.fault {
        Fault::SetUp => w::TierFault::SetUp,
        Fault::Completion => w::TierFault::Completion,
        Fault::NoStep => w::TierFault::NoStep,
        Fault::Cancelled => w::TierFault::Cancelled,
        Fault::OtherCommit => w::TierFault::OtherCommit,
        Fault::Lost => w::TierFault::Lost,
    };
    let run = match &held.run {
        Some(run) => Some(encode_receipt_run(run)),
        None => None,
    };
    Some(
        w::TierHeld::new(&w::CEILINGS, w::TierHeldParts { fault, run, status: encode_branch(held.status) })
            .expect(FITS),
    )
}
#[expect(clippy::manual_map, reason = "record translations use explicit matches without callbacks")]
fn decode_held(held: Option<w::TierHeld>) -> Option<d::TierHeld> {
    use temper_engine_domain_forge_landing::Fault;
    let w::TierHeldParts { fault, run, status } = held?.into_parts();
    let fault = match fault {
        w::TierFault::SetUp => Fault::SetUp,
        w::TierFault::Completion => Fault::Completion,
        w::TierFault::NoStep => Fault::NoStep,
        w::TierFault::Cancelled => Fault::Cancelled,
        w::TierFault::OtherCommit => Fault::OtherCommit,
        w::TierFault::Lost => Fault::Lost,
    };
    let run = match run {
        Some(run) => Some(decode_receipt_run(run)),
        None => None,
    };
    Some(d::TierHeld { fault, run, status: decode_branch(status) })
}
fn encode_reds(reds: &[d::TierRed]) -> List<w::TierRed> {
    let mut values = List::with_capacity(u32::try_from(reds.len()).expect(FITS));
    for red in reds {
        values
            .push(
                w::TierRed::new(
                    &w::CEILINGS,
                    w::TierRedParts {
                        repaired: red.repaired,
                        landing: red.landing,
                        blame: verification::encode_blame(&red.blame),
                        run: encode_receipt_run(&red.run),
                    },
                )
                .expect(FITS),
            )
            .expect(FITS);
    }
    values
}
fn decode_reds(reds: List<w::TierRed>) -> Result<Box<[d::TierRed]>, Undecodable> {
    let mut values = List::with_capacity(reds.len());
    for red in reds.into_boxed() {
        let w::TierRedParts { repaired, landing, blame, run } = red.into_parts();
        let blame = verification::decode_blame(blame)?;
        if blame == p::Blame::Cleared {
            return Err(Undecodable);
        }
        values.push(d::TierRed { repaired, landing, blame, run: decode_receipt_run(run) }).expect(FITS);
    }
    Ok(values.into_boxed())
}

fn encode_comments(comments: &[d::TierComment]) -> List<w::TierComment> {
    let mut values = List::with_capacity(u32::try_from(comments.len()).expect(FITS));
    for comment in comments {
        values
            .push(
                w::TierComment::new(
                    &w::CEILINGS,
                    w::TierCommentParts {
                        pull: comment.pull,
                        operation: comment.operation,
                        part: comment.part,
                        body: comment.body.clone(),
                        state: encode_branch(comment.state),
                    },
                )
                .expect(FITS),
            )
            .expect(FITS);
    }
    values
}
fn decode_comments(comments: List<w::TierComment>) -> Result<Box<[d::TierComment]>, Undecodable> {
    let mut values = List::with_capacity(comments.len());
    for comment in comments.into_boxed() {
        let w::TierCommentParts { pull, operation, part, body, state } = comment.into_parts();
        if pull == 0 || (operation != 8 && operation != 9) || u32::from(part) != values.len() || body.is_empty() {
            return Err(Undecodable);
        }
        values.push(d::TierComment { pull, operation, part, body, state: decode_branch(state) }).expect(FITS);
    }
    Ok(values.into_boxed())
}
