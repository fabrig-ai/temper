//! Tier stories with small bounds (testing-strategy.md, section 10).
#![expect(clippy::mod_module_files, reason = "T1 specifies shared helpers in src/tests/mod.rs")]
use crate::{
    Blame, Commit, Failures, Finding, Judged, Key, Landing, Limits, Next, Part, Record, Run, Settings, Suspects,
    TestId, Verdict, start,
};
use alloc::boxed::Box;

mod report;
mod verdict;

const BASE: Commit = [0; 32];
const L1: Commit = [1; 32];
const L2: Commit = [2; 32];
const L3: Commit = [3; 32];
const TIP: Commit = [4; 32];
const K_DOCS: Key = [10; 32];
const K_CODE: Key = [11; 32];
const K_TESTS: Key = [12; 32];
const K_TIP: Key = [13; 32];
// Task T1's report bounds and retry budget.
const LIMITS: Limits = Limits { failing_tests: 4, test_id_bytes: 16 };
const SETTINGS: Settings = Settings { retries: 2, suspects: Suspects::Bisect };

fn ids(names: &[&str]) -> Box<[TestId]> {
    let mut ids = skein_lib::List::with_capacity(u32::try_from(names.len()).expect("fixture size fits"));
    for name in names {
        ids.push(Box::from(name.as_bytes())).expect("fixture ids fit");
    }
    let mut ids = ids.into_boxed();
    ids.sort();
    ids
}

fn tests(names: &[&str]) -> Failures {
    Failures::Tests(ids(names))
}

fn landing(commit: Commit, key: Key) -> Landing {
    Landing { commit, key }
}

fn record(failing: Failures) -> Record {
    Record { sequence: 0, judged: Some(Judged { tip: BASE, key: K_DOCS }), failing, flakes: Box::new([]), run: None }
}

fn dispatched(next: Next) -> Run {
    match next {
        Next::Dispatch(run) => run,
        Next::Judged(_) | Next::Held(_) => panic!("expected a dispatched tier part"),
    }
}

fn verdict(next: Next) -> Verdict {
    match next {
        Next::Judged(verdict) => verdict,
        Next::Dispatch(_) | Next::Held(_) => panic!("expected a judged tier"),
    }
}

fn run(record: &Record, landings: Box<[Landing]>) -> Run {
    dispatched(start(record, TIP, K_TIP, landings, None, &SETTINGS))
}

fn rerun(record: &Record, landings: Box<[Landing]>, failed: &[&str]) -> Run {
    let mut run = run(record, landings);
    run.part = Part::Rerun { failed: ids(failed) };
    run
}

fn pair() -> Box<[Landing]> {
    Box::new([landing(L1, K_CODE), landing(TIP, K_TIP)])
}

fn three() -> Box<[Landing]> {
    Box::new([landing(L1, K_CODE), landing(L2, K_TESTS), landing(L3, K_TIP)])
}

fn finding(failing: Failures, blamed: &[Blame]) -> Finding {
    Finding { failing, flakes: ids(&[]), blamed: Box::from(blamed) }
}
