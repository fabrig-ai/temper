//! Retained first-edition story (domain/testing.md, 5.3): M1's message, M2's admission and QA's owner-landed test.
use temper_real_loop_world::{Exit, case, run_cluster_forge};

#[test]
fn m1s_end_message_starts_m2_beside_a_tester_without_forge_effects_whose_test_lands_only_through_a1s_change() {
    // Left out: A's fresh next run (5.5's gap; no milestone-end operation),
    // a Codex tester reading only the design (the tester here is scripted),
    // the three kinds of finding and A1's decisions (one test lands here),
    // and STATUS's update (report words, no STATUS file in the fixture).
    let ran = run_cluster_forge(
        case::PAUSE_WORKERS,
        &[],
        case::fixture_6_8(),
        case::scripts::step_6_8,
        case::scripts::step_6_8_rules,
        case::acts::step_6_8,
    );
    for (at, called) in ran.calls().iter().enumerate() {
        assert_eq!(
            called.exit,
            Some(Exit::Code(0)),
            "call {at}: {}\n{}",
            String::from_utf8_lossy(&called.stderr),
            String::from_utf8_lossy(&called.stdout)
        );
    }
    let observed = case::oracle::Observed::from_ran(&ran, case::acts::READS_6_8);
    case::oracle::assert_met(&case::oracle::step_6_8(&observed));
    // Calibrate the Add against the successful observation, without a second run.
    for (read, path, value) in [
        (case::acts::Read::QaChangeEnded, "/task/spec/specification/inputs", serde_json::json!([])),
        (case::acts::Read::QaChangeEnded, "/task/requester/task", serde_json::json!(case::qa::A)),
        (case::acts::Read::QaTesterEnded, "/task/result/value/report/bytes", serde_json::json!("other test")),
    ] {
        let mut false_evidence = observed.clone();
        *false_evidence.documents.get_mut(&read).expect("read").pointer_mut(path).expect("field") = value;
        assert!(
            case::oracle::step_6_8(&false_evidence)
                .iter()
                .any(|miss| miss.promise
                    == case::oracle::Promise::Add("the tester's tests land only through A1's change")),
            "{path}"
        );
    }
    let mut wrong_tree = observed.clone();
    wrong_tree.kit.main.remove(case::qa::QA_TEST_PATH.as_bytes());
    assert!(
        case::oracle::step_6_8(&wrong_tree)
            .iter()
            .any(|miss| miss.promise == case::oracle::Promise::Add("the tester's tests land only through A1's change"))
    );
}
