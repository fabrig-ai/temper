use super::{
    BASE, K_CODE, K_DOCS, K_TESTS, K_TIP, L1, L2, L3, LIMITS, SETTINGS, TIP, dispatched, finding, ids, landing, pair,
    record, rerun, run, tests, three, verdict,
};
use crate::{
    Blame, Failures, Flake, Judged, Next, Outcome, Part, Receipt, Selection, Settings, Suspects, due, judged, repaired,
    start, step,
};
use alloc::boxed::Box;

#[test]
fn due_waits_for_run_and_for_a_new_tip() {
    let mut record = record(tests(&[]));
    assert!(!due(&record, BASE), "the judged tip is already covered");
    assert!(due(&record, TIP), "a new tip needs a run");
    record.run = Some(run(&record, pair()));
    assert!(!due(&record, TIP), "one tier has only one run in flight");
}

#[test]
fn a_new_tier_is_due_for_its_baseline() {
    let mut record = record(tests(&[]));
    record.judged = None;
    assert!(due(&record, BASE), "the baseline has not been judged");
}

#[test]
fn a_green_run_clears_range_and_moves_record() {
    let mut record = record(tests(&["a"]));
    let run = run(&record, pair());
    record.run = Some(run.clone());
    let verdict = verdict(step(&record, &run, Outcome::Passed, &SETTINGS));
    assert_eq!(verdict.landings.as_ref(), &[Blame::Cleared, Blame::Cleared]);
    assert_eq!(
        judged(&record, &verdict, &LIMITS),
        crate::Record {
            sequence: 1,
            judged: Some(Judged { tip: TIP, key: K_TIP }),
            failing: tests(&[]),
            flakes: Box::new([]),
            run: None,
        }
    );
}

#[test]
fn a_whole_failure_reruns_only_the_failed_tests() {
    let record = record(tests(&[]));
    let run = run(&record, pair());
    let rerun = dispatched(step(&record, &run, Outcome::Failed(tests(&["a", "b"])), &SETTINGS));
    assert_eq!(rerun, crate::Run { part: Part::Rerun { failed: ids(&["a", "b"]) }, ..run });
}

#[test]
fn a_flake_clears_range_and_is_counted_twice() {
    let record = record(tests(&[]));
    let run = rerun(&record, pair(), &["a"]);
    let verdict = verdict(step(&record, &run, Outcome::Passed, &SETTINGS));
    assert_eq!(verdict.landings.as_ref(), &[Blame::Cleared, Blame::Cleared]);
    assert_eq!(verdict.failing, tests(&[]));
    assert_eq!(verdict.flakes, ids(&["a"]));
    let once = judged(&record, &verdict, &LIMITS);
    assert_eq!(once.flakes.as_ref(), &[Flake { test: Box::from(b"a".as_slice()), count: 1 }]);
    let next_run = rerun(&once, pair(), &["a"]);
    let next_verdict = super::verdict(step(&once, &next_run, Outcome::Passed, &SETTINGS));
    let twice = judged(&once, &next_verdict, &LIMITS);
    assert_eq!(twice.sequence, 2);
    assert_eq!(twice.flakes.as_ref(), &[Flake { test: Box::from(b"a".as_slice()), count: 2 }]);
}

#[test]
fn one_landing_owns_only_new_confirmed_failures() {
    let record = record(tests(&["a"]));
    let run = rerun(&record, Box::new([landing(TIP, K_TIP)]), &["a", "b", "c"]);
    let verdict = verdict(step(&record, &run, Outcome::Failed(tests(&["a", "b", "foreign"])), &SETTINGS));
    assert_eq!(verdict.failing, tests(&["a", "b"]));
    assert_eq!(verdict.flakes, ids(&["c"]));
    assert_eq!(verdict.landings.as_ref(), &[Blame::Owns(tests(&["b"]))]);
}

#[test]
fn unchanged_first_landing_is_skipped_by_the_probe() {
    let record = record(tests(&[]));
    let landings = Box::new([landing(L1, K_DOCS), landing(L2, K_TESTS), landing(L3, K_TIP)]);
    let run = rerun(&record, landings, &["a", "b"]);
    let probe = dispatched(step(&record, &run, Outcome::Failed(tests(&["a", "b"])), &SETTINGS));
    assert_eq!(
        probe.part,
        Part::Probe {
            landing: 1,
            tests: Selection::Tests(ids(&["a", "b"])),
            finding: finding(tests(&["a", "b"]), &[Blame::Cleared, Blame::Cleared, Blame::Cleared]),
        }
    );
    let verdict = verdict(step(&record, &probe, Outcome::Failed(tests(&["a", "b"])), &SETTINGS));
    assert_eq!(verdict.landings.as_ref(), &[Blame::Cleared, Blame::Owns(tests(&["a", "b"])), Blame::Cleared]);
}

#[test]
fn unchanged_last_landing_needs_no_further_probe() {
    let record = record(tests(&[]));
    let run = rerun(&record, Box::new([landing(L1, K_CODE), landing(L2, K_TIP), landing(L3, K_TIP)]), &["a", "b"]);
    let probe = dispatched(step(&record, &run, Outcome::Failed(tests(&["a", "b"])), &SETTINGS));
    assert_eq!(
        probe.part,
        Part::Probe {
            landing: 0,
            tests: Selection::Tests(ids(&["a", "b"])),
            finding: finding(tests(&["a", "b"]), &[Blame::Cleared, Blame::Cleared, Blame::Cleared]),
        }
    );
    let verdict = verdict(step(&record, &probe, Outcome::Passed, &SETTINGS));
    assert_eq!(verdict.landings.as_ref(), &[Blame::Cleared, Blame::Owns(tests(&["a", "b"])), Blame::Cleared]);
}

#[test]
fn two_landings_own_the_tests_they_first_fail() {
    let record = record(tests(&[]));
    let run = rerun(&record, three(), &["a", "b"]);
    let first = dispatched(step(&record, &run, Outcome::Failed(tests(&["a", "b"])), &SETTINGS));
    let second = dispatched(step(&record, &first, Outcome::Failed(tests(&["a", "foreign"])), &SETTINGS));
    assert_eq!(
        second.part,
        Part::Probe {
            landing: 1,
            tests: Selection::Tests(ids(&["b"])),
            finding: finding(tests(&["a", "b"]), &[Blame::Owns(tests(&["a"])), Blame::Cleared, Blame::Cleared]),
        }
    );
    let verdict = verdict(step(&record, &second, Outcome::Failed(tests(&["b"])), &SETTINGS));
    assert_eq!(verdict.landings.as_ref(), &[Blame::Owns(tests(&["a"])), Blame::Owns(tests(&["b"])), Blame::Cleared]);
}

#[test]
fn probes_preserve_inherited_failures_and_flakes() {
    let record = record(tests(&["a"]));
    let run = rerun(&record, three(), &["a", "b", "c", "d"]);
    let first = dispatched(step(&record, &run, Outcome::Failed(tests(&["a", "b", "c"])), &SETTINGS));
    let second = dispatched(step(&record, &first, Outcome::Failed(tests(&["b"])), &SETTINGS));
    let verdict = verdict(step(&record, &second, Outcome::Passed, &SETTINGS));
    assert_eq!(verdict.failing, tests(&["a", "b", "c"]));
    assert_eq!(verdict.flakes, ids(&["d"]));
    assert_eq!(verdict.landings.as_ref(), &[Blame::Owns(tests(&["b"])), Blame::Cleared, Blame::Owns(tests(&["c"]))]);
}

#[test]
fn inherited_failures_clear_the_next_landing_without_repair() {
    let record = record(tests(&["a", "b"]));
    let run = rerun(&record, Box::new([landing(TIP, K_TIP)]), &["a", "b"]);
    let verdict = verdict(step(&record, &run, Outcome::Failed(tests(&["a", "b"])), &SETTINGS));
    assert_eq!(verdict.landings.as_ref(), &[Blame::Cleared]);
    assert_eq!(verdict.failing, tests(&["a", "b"]));
    assert!(!repaired(&Blame::Owns(tests(&["a", "b"])), &verdict.failing), "inherited tests still fail");
}

#[test]
fn later_green_repairs_an_earlier_red_landing() {
    let record = record(tests(&["a", "b"]));
    let run = run(&record, pair());
    let verdict = verdict(step(&record, &run, Outcome::Passed, &SETTINGS));
    assert!(repaired(&Blame::Owns(tests(&["a", "b"])), &verdict.failing), "both blamed tests passed");
}

#[test]
fn tell_all_names_every_landing_as_a_suspect() {
    let record = record(tests(&[]));
    let run = rerun(&record, pair(), &["a"]);
    let settings = Settings { suspects: Suspects::TellAll, ..SETTINGS };
    let verdict = verdict(step(&record, &run, Outcome::Failed(tests(&["a"])), &settings));
    assert_eq!(verdict.landings.as_ref(), &[Blame::Suspect(tests(&["a"])), Blame::Suspect(tests(&["a"]))]);
}

#[test]
fn many_failures_are_owned_without_a_test_rerun() {
    let record = record(tests(&[]));
    let run = run(&record, Box::new([landing(TIP, K_TIP)]));
    let verdict = verdict(step(&record, &run, Outcome::Failed(Failures::Many), &SETTINGS));
    assert_eq!(verdict.failing, Failures::Many);
    assert_eq!(verdict.landings.as_ref(), &[Blame::Owns(Failures::Many)]);
}

#[test]
fn many_failures_probe_the_whole_tier() {
    let record = record(tests(&[]));
    let run = run(&record, pair());
    let probe = dispatched(step(&record, &run, Outcome::Failed(Failures::Many), &SETTINGS));
    assert_eq!(
        probe.part,
        Part::Probe {
            landing: 0,
            tests: Selection::All,
            finding: finding(Failures::Many, &[Blame::Cleared, Blame::Cleared]),
        }
    );
    let verdict = verdict(step(&record, &probe, Outcome::Passed, &SETTINGS));
    assert_eq!(verdict.landings.as_ref(), &[Blame::Cleared, Blame::Owns(Failures::Many)]);
}

#[test]
fn a_red_whole_probe_owns_many_indivisibly() {
    let record = record(tests(&[]));
    for outcome in [Outcome::Failed(Failures::Many), Outcome::Failed(tests(&["a"]))] {
        let run = run(&record, pair());
        let probe = dispatched(step(&record, &run, Outcome::Failed(Failures::Many), &SETTINGS));
        let verdict = verdict(step(&record, &probe, outcome, &SETTINGS));
        assert_eq!(verdict.landings.as_ref(), &[Blame::Owns(Failures::Many), Blame::Cleared]);
    }
}

#[test]
fn many_after_many_is_inherited_whole() {
    let record = record(Failures::Many);
    let run = run(&record, pair());
    let verdict = verdict(step(&record, &run, Outcome::Failed(Failures::Many), &SETTINGS));
    assert_eq!(verdict.failing, Failures::Many);
    assert_eq!(verdict.landings.as_ref(), &[Blame::Cleared, Blame::Cleared]);
}

#[test]
fn named_failures_after_many_are_all_new() {
    let record = record(Failures::Many);
    let run = rerun(&record, Box::new([landing(TIP, K_TIP)]), &["a"]);
    let verdict = verdict(step(&record, &run, Outcome::Failed(tests(&["a"])), &SETTINGS));
    assert_eq!(verdict.landings.as_ref(), &[Blame::Owns(tests(&["a"]))]);
}

#[test]
fn baseline_records_failures_without_blaming_any_landing() {
    let mut record = record(tests(&[]));
    record.judged = None;
    let run = rerun(&record, Box::new([]), &["a"]);
    let verdict = verdict(step(&record, &run, Outcome::Failed(tests(&["a"])), &SETTINGS));
    assert_eq!(verdict.landings.as_ref(), &[]);
    assert_eq!(judged(&record, &verdict, &LIMITS).failing, tests(&["a"]));
}

#[test]
fn passed_receipt_judges_tip_without_dispatching() {
    let record = record(tests(&["a"]));
    let verdict = verdict(start(&record, TIP, K_TIP, pair(), Some(Receipt::Passed), &SETTINGS));
    assert_eq!(verdict.failing, tests(&[]));
    assert_eq!(verdict.landings.as_ref(), &[Blame::Cleared, Blame::Cleared]);
}

#[test]
fn failed_receipt_blames_without_rerunning_tests() {
    let record = record(tests(&[]));
    let verdict = verdict(start(
        &record,
        TIP,
        K_TIP,
        Box::new([landing(TIP, K_TIP)]),
        Some(Receipt::Failed(tests(&["a"]))),
        &SETTINGS,
    ));
    assert_eq!(verdict.landings.as_ref(), &[Blame::Owns(tests(&["a"]))]);
    assert_eq!(verdict.flakes, ids(&[]));
}

#[test]
fn failed_receipt_starts_probes_when_range_needs_them() {
    let record = record(tests(&[]));
    let run = dispatched(start(&record, TIP, K_TIP, pair(), Some(Receipt::Failed(tests(&["a"]))), &SETTINGS));
    assert_eq!(
        run.part,
        Part::Probe {
            landing: 0,
            tests: Selection::Tests(ids(&["a"])),
            finding: finding(tests(&["a"]), &[Blame::Cleared, Blame::Cleared])
        }
    );
}

#[test]
fn two_faults_retry_same_part_and_third_holds() {
    let record = record(tests(&[]));
    let whole = run(&record, pair());
    let rerun = rerun(&record, pair(), &["a"]);
    let probe = dispatched(step(&record, &rerun, Outcome::Failed(tests(&["a"])), &SETTINGS));
    for run in [whole, rerun, probe] {
        let one = dispatched(step(&record, &run, Outcome::Fault, &SETTINGS));
        let two = dispatched(step(&record, &one, Outcome::Fault, &SETTINGS));
        assert_eq!(one, crate::Run { faults: 1, ..run.clone() });
        assert_eq!(two, crate::Run { faults: 2, ..run.clone() });
        assert_eq!(step(&record, &two, Outcome::Fault, &SETTINGS), Next::Held(crate::Run { faults: 3, ..run }));
    }
}

#[test]
fn successful_part_resets_its_fault_count() {
    let record = record(tests(&[]));
    let mut run = run(&record, pair());
    run.faults = SETTINGS.retries;
    let next = dispatched(step(&record, &run, Outcome::Failed(tests(&["a"])), &SETTINGS));
    assert_eq!(next.faults, 0);
}

#[test]
fn one_changed_key_owns_without_a_probe() {
    let record = record(tests(&[]));
    let run = rerun(&record, Box::new([landing(L1, K_DOCS), landing(L2, K_TIP), landing(L3, K_TIP)]), &["a"]);
    let verdict = verdict(step(&record, &run, Outcome::Failed(tests(&["a"])), &SETTINGS));
    assert_eq!(verdict.landings.as_ref(), &[Blame::Cleared, Blame::Owns(tests(&["a"])), Blame::Cleared]);
}

#[test]
fn no_changed_key_assigns_to_the_last_landing() {
    let record = record(tests(&[]));
    let run = rerun(&record, Box::new([landing(L1, K_DOCS), landing(TIP, K_DOCS)]), &["a"]);
    let verdict = verdict(step(&record, &run, Outcome::Failed(tests(&["a"])), &SETTINGS));
    assert_eq!(verdict.landings.as_ref(), &[Blame::Cleared, Blame::Owns(tests(&["a"]))]);
}

#[test]
fn unexpected_rerun_tests_do_not_prevent_flake_detection() {
    let record = record(tests(&[]));
    let run = rerun(&record, pair(), &["a"]);
    let verdict = verdict(step(&record, &run, Outcome::Failed(tests(&["foreign"])), &SETTINGS));
    assert_eq!(verdict.failing, tests(&[]));
    assert_eq!(verdict.flakes, ids(&["a"]));
    assert_eq!(verdict.landings.as_ref(), &[Blame::Cleared, Blame::Cleared]);
}

#[test]
fn oversized_rerun_conservatively_retains_the_selected_tests() {
    let record = record(tests(&[]));
    let run = rerun(&record, Box::new([landing(TIP, K_TIP)]), &["a"]);
    let verdict = verdict(step(&record, &run, Outcome::Failed(Failures::Many), &SETTINGS));
    assert_eq!(verdict.failing, tests(&["a"]));
    assert_eq!(verdict.flakes, ids(&[]));
    assert_eq!(verdict.landings.as_ref(), &[Blame::Owns(tests(&["a"]))]);
}

#[test]
fn flakes_enter_in_order_without_displacing_existing_counts() {
    let mut record = record(tests(&[]));
    record.flakes = Box::new([
        Flake { test: Box::from(b"b".as_slice()), count: 2 },
        Flake { test: Box::from(b"d".as_slice()), count: u32::MAX },
    ]);
    let run = rerun(&record, pair(), &["a", "b", "c", "d"]);
    let verdict = verdict(step(&record, &run, Outcome::Passed, &SETTINGS));
    let next = judged(&record, &verdict, &LIMITS);
    assert_eq!(
        next.flakes.as_ref(),
        &[
            Flake { test: Box::from(b"a".as_slice()), count: 1 },
            Flake { test: Box::from(b"b".as_slice()), count: 3 },
            Flake { test: Box::from(b"c".as_slice()), count: 1 },
            Flake { test: Box::from(b"d".as_slice()), count: u32::MAX },
        ]
    );
}

#[test]
fn full_flake_record_keeps_old_counts_and_refuses_new() {
    let mut record = record(tests(&[]));
    record.flakes = Box::new([
        Flake { test: Box::from(b"b".as_slice()), count: 1 },
        Flake { test: Box::from(b"c".as_slice()), count: 1 },
        Flake { test: Box::from(b"d".as_slice()), count: 1 },
        Flake { test: Box::from(b"e".as_slice()), count: 1 },
    ]);
    let run = rerun(&record, pair(), &["a", "e"]);
    let verdict = verdict(step(&record, &run, Outcome::Passed, &SETTINGS));
    assert_eq!(
        judged(&record, &verdict, &LIMITS).flakes.as_ref(),
        &[
            Flake { test: Box::from(b"b".as_slice()), count: 1 },
            Flake { test: Box::from(b"c".as_slice()), count: 1 },
            Flake { test: Box::from(b"d".as_slice()), count: 1 },
            Flake { test: Box::from(b"e".as_slice()), count: 2 },
        ]
    );
}

#[test]
fn repair_depends_on_the_blamed_failures_only() {
    let cases = [
        (Blame::Cleared, Failures::Many, true),
        (Blame::Owns(tests(&["a"])), Failures::Many, false),
        (Blame::Suspect(tests(&["a"])), tests(&["a", "b"]), false),
        (Blame::Owns(tests(&["a"])), tests(&["b"]), true),
        (Blame::Suspect(tests(&["a"])), tests(&["b"]), true),
        (Blame::Owns(Failures::Many), tests(&["b"]), true),
        (Blame::Suspect(Failures::Many), Failures::Many, false),
    ];
    for (blame, failing, expected) in cases {
        assert_eq!(repaired(&blame, &failing), expected);
    }
}
