//! The second edition's opening checkpoint (draft/reference-case.md, 2 and 3).
use temper_real_loop_world::{
    Exit,
    case2::{
        self, CAST, acts,
        oracle::{self, Observed},
    },
    run_cluster_forge,
};

#[test]
fn the_case_starts_with_p_and_its_implementers_running_and_nothing_held() {
    let ran = run_cluster_forge(
        case2::WORKERS,
        &[],
        case2::fixture_hour_0(),
        case2::scripts::hour_0,
        case2::scripts::hour_0_rules,
        acts::hour_0,
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
    let observed = Observed::from_ran(&ran, &acts::reads(&CAST));
    assert!(oracle::hour_0_p_and_its_implementers_run(&CAST, &observed), "hour 0 cast: {:#?}", observed.documents);
    assert!(oracle::hour_0_nothing_held(&CAST, &observed), "hour 0 holds and findings: {:#?}", observed.documents);
}
