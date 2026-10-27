use super::*;
use temper_engine_domain_forge_landing as policy;

fn landing_fixture() -> Domain {
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    let mut row = fixtures::opening_row();
    row.repository = REPO;
    row.pull = Some(1);
    row.change.landing = None;
    outputs(&mut d, Event::Restore { record: Stored::Change(row) });
    assert!(d.project_landing(5, temper_engine_domain_forge_change::Way::Turn, Box::new([])));
    d
}
fn landing_row(task: u64, phase: policy::Phase) -> LandingRow {
    LandingRow {
        landing: policy::Landing { task, project: 5, phase, members: Box::new([]), preparation: 0, stale_proofs: 0 },
        asks: Box::new([]),
        entries: Box::new([]),
    }
}
#[test]
fn landing_preflight_checks_rules_and_live_room() {
    let mut d = landing_fixture();
    assert_eq!(d.landing_admits(6, &[fixtures::CHANGE]), Err(LandingRefusal::Way));
    assert_eq!(d.landing_admits(5, &[]), Err(LandingRefusal::Members));
    assert_eq!(d.landing_admits(5, &[fixtures::CHANGE; 3]), Err(LandingRefusal::Members));
    assert_eq!(d.landing_admits(5, &[999]), Err(LandingRefusal::Repository));
    assert_eq!(d.landing_admits(5, &[fixtures::CHANGE; 2]), Err(LandingRefusal::Repository));
    assert_eq!(d.landing_admits(5, &[fixtures::CHANGE]), Ok(()));
    for task in 100..108 {
        d.landing.rows.insert(task, landing_row(task, policy::Phase::Preparing)).expect("room");
    }
    assert_eq!(d.landing_admits(5, &[fixtures::CHANGE]), Err(LandingRefusal::Landings));
    d.landing.rows.get_mut(&100).expect("landing").landing.phase = policy::Phase::Landed;
    assert_eq!(d.landing_admits(5, &[fixtures::CHANGE]), Ok(()));
    d.landing.rows.get_mut(&100).expect("landing").entries =
        Box::new([LandingEntry { entry: 1, member: 0, gate: 0, operation: Operation::Merge }]);
    assert_eq!(d.landing_admits(5, &[fixtures::CHANGE]), Err(LandingRefusal::Landings));
}
