//! Level-triggered verdict decisions (domain/after-landing.md, sections 3–5).
use crate::{
    Blame, Commit, Failures, Finding, Flake, Judged, Key, Landing, Limits, Next, Outcome, Part, Receipt, Record, Run,
    Selection, Settings, Suspects, TestId, Verdict,
};
use alloc::boxed::Box;
use skein_lib::List;

/// On a tip's move, whether the tier needs a run.
#[must_use]
pub fn due(record: &Record, tip: Commit) -> bool {
    if record.run.is_some() {
        return false;
    }
    match record.judged {
        Some(judged) => judged.tip != tip,
        None => true,
    }
}

/// Starts the next run, or judges the tip by its receipt.
#[must_use]
pub fn start(
    record: &Record,
    tip: Commit,
    key: Key,
    landings: Box<[Landing]>,
    receipt: Option<Receipt>,
    settings: &Settings,
) -> Next {
    let run = Run {
        number: record.sequence.checked_add(1).expect("the tier sequence fits u64"),
        tip,
        key,
        landings,
        part: Part::Whole,
        faults: 0,
    };
    match receipt {
        None => Next::Dispatch(run),
        Some(Receipt::Passed) => green(&run, Box::new([])),
        Some(Receipt::Failed(failing)) => blame(record, &run, failing, Box::new([]), *settings),
    }
}

/// Reads the completion of the committed part of a run.
#[must_use]
pub fn step(record: &Record, run: &Run, outcome: Outcome, settings: &Settings) -> Next {
    let reported = match outcome {
        Outcome::Fault => {
            let mut retry = run.clone();
            // Even u32::MAX retries has a terminal fault; it must not wrap.
            let Some(faults) = run.faults.checked_add(1) else {
                return Next::Held(retry);
            };
            retry.faults = faults;
            return if faults <= settings.retries { Next::Dispatch(retry) } else { Next::Held(retry) };
        }
        Outcome::Passed => Failures::Tests(Box::new([])),
        Outcome::Failed(failing) => failing,
    };
    match &run.part {
        Part::Whole => match reported {
            Failures::Tests(tests) => {
                if tests.is_empty() {
                    green(run, Box::new([]))
                } else {
                    dispatch(run, Part::Rerun { failed: tests })
                }
            }
            Failures::Many => blame(record, run, Failures::Many, Box::new([]), *settings),
        },
        Part::Rerun { failed } => {
            let confirmed = within(failed, &reported);
            let flakes = select(failed, &confirmed, false);
            blame(record, run, Failures::Tests(confirmed), flakes, *settings)
        }
        Part::Probe { landing, tests, finding } => {
            let mut finding = finding.clone();
            let (owned, remaining) = match tests {
                Selection::All => match reported {
                    Failures::Many => (Failures::Many, empty()),
                    Failures::Tests(tests) => {
                        if tests.is_empty() {
                            (empty(), Failures::Many)
                        } else {
                            (Failures::Many, empty())
                        }
                    }
                },
                Selection::Tests(tests) => {
                    let owned = within(tests, &reported);
                    let remaining = select(tests, &owned, false);
                    (Failures::Tests(owned), Failures::Tests(remaining))
                }
            };
            own(&mut finding, *landing, owned);
            probe(record, run, finding, remaining, Some(*landing))
        }
    }
}

fn empty() -> Failures {
    Failures::Tests(Box::new([]))
}

fn none(failing: &Failures) -> bool {
    match failing {
        Failures::Tests(tests) => tests.is_empty(),
        Failures::Many => false,
    }
}

fn length(length: usize) -> u32 {
    u32::try_from(length).expect("top-owned bounded slices fit u32")
}

fn cleared(run: &Run) -> Box<[Blame]> {
    let mut blamed = List::with_capacity(length(run.landings.len()));
    for _landing in &run.landings {
        blamed.push(Blame::Cleared).expect("one blame per landing");
    }
    blamed.into_boxed()
}

fn finish(run: &Run, finding: Finding) -> Next {
    Next::Judged(Verdict {
        number: run.number,
        tip: run.tip,
        key: run.key,
        failing: finding.failing,
        landings: finding.blamed,
        flakes: finding.flakes,
    })
}

fn green(run: &Run, flakes: Box<[TestId]>) -> Next {
    finish(run, Finding { failing: empty(), flakes, blamed: cleared(run) })
}

fn dispatch(run: &Run, part: Part) -> Next {
    let mut next = run.clone();
    next.part = part;
    next.faults = 0;
    Next::Dispatch(next)
}

/// Intersection or difference of two sorted sets, by a monotone merge.
fn select(left: &[TestId], right: &[TestId], shared: bool) -> Box<[TestId]> {
    let mut selected = List::with_capacity(length(left.len()));
    let mut from = 0_usize;
    for test in left {
        for other in right.get(from..).expect("merge cursor stays within right") {
            if other >= test {
                break;
            }
            from = from.checked_add(1).expect("cursor within the slice");
        }
        if (right.get(from) == Some(test)) == shared {
            selected.push(test.clone()).expect("a subset fits its source length");
        }
    }
    selected.into_boxed()
}

// An oversized partial report cannot identify which selected tests passed:
// conservatively retain the selection. Unrequested test ids never count.
fn within(selected: &[TestId], reported: &Failures) -> Box<[TestId]> {
    match reported {
        Failures::Tests(tests) => select(selected, tests, true),
        Failures::Many => Box::from(selected),
    }
}

fn blame(record: &Record, run: &Run, failing: Failures, flakes: Box<[TestId]>, settings: Settings) -> Next {
    let new = match &failing {
        Failures::Tests(tests) => match &record.failing {
            Failures::Tests(previous) => Failures::Tests(select(tests, previous, false)),
            Failures::Many => failing.clone(),
        },
        Failures::Many => match &record.failing {
            Failures::Many => empty(),
            Failures::Tests(_) => Failures::Many,
        },
    };
    let mut finding = Finding { failing, flakes, blamed: cleared(run) };
    if none(&new) || run.landings.is_empty() {
        return finish(run, finding);
    }
    if run.landings.len() == 1 {
        own(&mut finding, 0, new);
        return finish(run, finding);
    }
    match settings.suspects {
        Suspects::TellAll => {
            for blame in &mut finding.blamed {
                *blame = Blame::Suspect(new.clone());
            }
            finish(run, finding)
        }
        Suspects::Bisect => probe(record, run, finding, new, None),
    }
}

fn own(finding: &mut Finding, landing: u32, failing: Failures) {
    if !none(&failing) {
        let blame = finding.blamed.get_mut(usize::try_from(landing).expect("u32 fits usize"));
        *blame.expect("a probe names a landing in its range") = Blame::Owns(failing);
    }
}

#[expect(clippy::manual_map, reason = "the strict subset forbids closures")]
fn probe(record: &Record, run: &Run, mut finding: Finding, remaining: Failures, after: Option<u32>) -> Next {
    if none(&remaining) {
        return finish(run, finding);
    }
    let mut previous = match record.judged {
        Some(judged) => Some(judged.key),
        None => None,
    };
    let mut first = None;
    let mut last = None;
    for (index, landing) in run.landings.iter().enumerate() {
        let index = length(index);
        let later = match after {
            Some(after) => index > after,
            None => true,
        };
        if later && previous != Some(landing.key) {
            if first.is_none() {
                first = Some(index);
            }
            last = Some(index);
        }
        previous = Some(landing.key);
    }
    let first = first.unwrap_or(length(run.landings.len()).checked_sub(1).expect("a non-baseline has landings"));
    if last.is_none() || last == Some(first) {
        own(&mut finding, first, remaining);
        return finish(run, finding);
    }
    let tests = match remaining {
        Failures::Tests(tests) => Selection::Tests(tests),
        Failures::Many => Selection::All,
    };
    dispatch(run, Part::Probe { landing: first, tests, finding })
}

/// Saves the judged tip, failures and bounded cumulative flake counts.
#[must_use]
pub fn judged(record: &Record, verdict: &Verdict, limits: &Limits) -> Record {
    let mut flakes = List::with_capacity(limits.failing_tests);
    let mut previous = record.flakes.iter().peekable();
    let mut room = limits.failing_tests.saturating_sub(length(record.flakes.len()));
    for test in &verdict.flakes {
        // Each old entry is visited once, and new entries enter in order.
        for _ in 0..record.flakes.len() {
            let Some(old) = previous.peek() else { break };
            if old.test >= *test {
                break;
            }
            let old = previous.next().expect("the peeked old entry");
            flakes.push(old.clone()).expect("old flakes fit the bound");
        }
        let same = match previous.peek() {
            Some(old) => old.test == *test,
            None => false,
        };
        if same {
            let old = previous.next().expect("the matching old entry");
            flakes.push(Flake { test: test.clone(), count: old.count.saturating_add(1) }).expect("old flakes fit");
        } else if room != 0 {
            flakes.push(Flake { test: test.clone(), count: 1 }).expect("new flakes have room");
            room = room.checked_sub(1).expect("room is nonzero");
        }
    }
    for old in previous {
        flakes.push(old.clone()).expect("old flakes fit the bound");
    }
    Record {
        sequence: verdict.number,
        judged: Some(Judged { tip: verdict.tip, key: verdict.key }),
        failing: verdict.failing.clone(),
        flakes: flakes.into_boxed(),
        run: None,
    }
}

/// Whether the judged run passed the failures blamed on a landing.
#[must_use]
pub fn repaired(blame: &Blame, failing: &Failures) -> bool {
    let blamed = match blame {
        Blame::Cleared => return true,
        Blame::Owns(failures) | Blame::Suspect(failures) => failures,
    };
    match failing {
        Failures::Many => false,
        Failures::Tests(tests) => match blamed {
            Failures::Many => true,
            Failures::Tests(blamed) => select(blamed, tests, true).is_empty(),
        },
    }
}
