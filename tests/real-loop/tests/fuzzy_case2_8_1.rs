//! The person's pause and accepted release (draft/reference-case.md, 8.1).
use temper_real_loop_world::{
    Exit,
    case2::{
        self, CAST, acts,
        oracle::{self, Observed},
    },
    run_cluster_forge,
};

#[test]
fn the_person_pauses_p_and_the_directors_proposed_release_resumes_it() {
    let ran = run_cluster_forge(
        case2::WORKERS,
        &[],
        case2::fixture_step_8_1(),
        case2::scripts::step_8_1,
        case2::scripts::step_8_1_rules,
        acts::step_8_1,
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
    let observed = Observed::from_ran(&ran, &acts::step_8_1_reads(&CAST));
    assert!(oracle::hour_0_p_and_its_implementers_run(&CAST, &observed), "hour 0 cast: {:#?}", observed.documents);
    assert!(oracle::hour_0_nothing_held(&CAST, &observed), "hour 0 holds and findings: {:#?}", observed.documents);
    assert!(
        oracle::step_8_1_p_and_its_implementers_held_by_the_persons_stop(&CAST, &observed),
        "stop: {:#?}",
        observed.documents
    );
    assert!(oracle::step_8_1_the_directors_release_refused_naming_the_person(&CAST, &observed), "release refused");
    assert!(
        oracle::step_8_1_the_proposal_waits_in_the_persons_inbox(&CAST, &observed),
        "proposal: {:#?}",
        observed.documents
    );
    assert!(oracle::step_8_1_p_resumes_from_its_transcript(&CAST, &observed), "resumed: {:#?}", observed.documents);
}
