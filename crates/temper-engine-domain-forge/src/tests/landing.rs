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

#[test]
fn landing_merge_carries_coherent_evidence_and_veto_withdraws_it() {
    use skein_forge::api::{Answer, Op, Read};
    use temper_engine_domain_forge_change as change;
    let mut d = landing_fixture();
    outputs(&mut d, Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic });
    outputs(&mut d, Event::ReadAfresh);
    outputs(&mut d, Event::SettleOutbox);
    fixtures::prime_main(&mut d);
    d.changes.get_mut(&fixtures::CHANGE).expect("change").change.state =
        change::State::Ready { head: fixtures::HEAD, ready_since: env().wall };
    let mut row = landing_row(61, policy::Phase::Turn { at: env().wall });
    row.landing.members = Box::new([policy::Member {
        change: fixtures::CHANGE,
        repository: policy::Repository { forge: REPO.forge, repository: REPO.repository },
        pull: 1,
        head: Some(fixtures::HEAD),
        base: Some(fixtures::BASE),
        candidate: Some(fixtures::HEAD),
        gates: Box::new([]),
        asked: None,
        landed: None,
    }]);
    row.asks = Box::new([None]);
    outputs(&mut d, Event::Landing { row });
    outputs(
        &mut d,
        Event::StepLanding { task: 61, entry: 91, heard: LandingHeard { released: false, cancelled: false } },
    );
    fixtures::answer_next(&mut d, Ok(Answer::Commit(fixtures::HEAD)));
    fixtures::answer_next(&mut d, Ok(Answer::Commit(fixtures::BASE)));
    fixtures::answer_next(
        &mut d,
        Ok(Answer::Compare {
            before: fixtures::BASE,
            after: fixtures::HEAD,
            contains_before: true,
            files: Box::new([]),
            commits: Box::new([]),
        }),
    );
    fixtures::answer_next(&mut d, Ok(Answer::Pull(fixtures::open_pull())));
    let (call, op) = fixtures::next_call(&mut d);
    assert_eq!(op, Op::Read(Read::Reviews { number: 1, page: 1 }));
    let requests = fixtures::answer(&mut d, call, Ok(Answer::Reviews { reviews: Box::new([]), more: false }));
    let mut evidence = None;
    for request in &requests {
        if let Request::LandingDecision { evidence: Some(facts), entry: Some(91), .. } = request {
            evidence = Some(facts);
        }
    }
    let evidence = evidence.expect("merge evidence");
    assert_eq!(evidence.change, fixtures::CHANGE);
    assert_eq!(evidence.facts.head, Some(fixtures::HEAD));
    assert_eq!(evidence.facts.base_tip, fixtures::BASE);
    assert_eq!(evidence.facts.contains_base, change::Status::Passed);
    assert_eq!(evidence.facts.ci, change::Status::Passed);
    assert!(evidence.facts.reviews_complete);
    let (repository, _, described) = d.describe_landing_effect(61, 91).expect("saved merge description");
    assert_eq!(repository, REPO);
    assert_eq!(described.kind, 4);
    assert_eq!(described.state, fixtures::HEAD);
    let vetoed = outputs(&mut d, Event::VetoLanding { task: 61, entry: 91 });
    assert!(vetoed.contains(&Request::Erase { key: Key::Entry(91) }));
    assert!(d.describe_landing_effect(61, 91).is_none());
    assert_eq!(d.landing.results.get(&(61, 0, 0, 5)), Some(&policy::Status::Failed));
    outputs(&mut d, Event::Committed { entry: 91 });
    for request in ready(&mut d) {
        if let Request::Call { op: Op::Write(_), .. } = request {
            panic!("vetoed write reached client");
        }
    }
}

#[test]
fn landing_deadline_wakes_once_and_an_offered_step_clears_it() {
    let mut d = landing_fixture();
    outputs(&mut d, Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic });
    outputs(&mut d, Event::ReadAfresh);
    outputs(&mut d, Event::SettleOutbox);
    d.wait_landing(&env(), 61, Some(env().wall));
    assert_eq!(d.next_wall_deadline(), Some(env().wall));
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    fire(&mut d, &env(), &mut out);
    let mut wakes = 0_u32;
    while let Some(request) = out.pop() {
        if request == (Request::WakeProcedure { task: 61 }) {
            wakes = wakes.checked_add(1).expect("bounded wake count");
        }
    }
    assert_eq!(wakes, 1);
    assert_eq!(d.next_wall_deadline(), None);
    d.wait_landing(&env(), 61, None);
    d.offered_landing(61);
    assert_eq!(d.next_wall_deadline(), None);
}

fn member_landing(d: &mut Domain) {
    let mut row = landing_row(61, policy::Phase::Preparing);
    row.landing.members = Box::new([policy::Member {
        change: fixtures::CHANGE,
        repository: policy::Repository { forge: REPO.forge, repository: REPO.repository },
        pull: 1,
        head: None,
        base: None,
        candidate: None,
        gates: Box::new([]),
        asked: None,
        landed: None,
    }]);
    row.asks = Box::new([None]);
    outputs(d, Event::Landing { row });
}
#[test]
fn cancelling_a_landing_leaves_its_change_live_and_detached() {
    let mut d = landing_fixture();
    member_landing(&mut d);
    let requests = outputs(&mut d, Event::SettleEffects { task: 61, root: 61, ending: ReleaseEnding::Cancelled });
    assert_eq!(d.landing(61).expect("cancelled landing").landing.phase, policy::Phase::Cancelled);
    assert_eq!(d.change(fixtures::CHANGE).expect("member stays").change.landing, None);
    for request in &requests {
        if let Request::Save { record: Stored::Release(row), .. } = request {
            assert_ne!(row.task, fixtures::CHANGE, "member is not cancelled");
        }
    }
}
#[test]
fn cancelling_the_last_member_cancels_its_landing_without_reading_the_missing_branch() {
    let mut d = landing_fixture();
    member_landing(&mut d);
    outputs(
        &mut d,
        Event::SettleEffects { task: fixtures::CHANGE, root: fixtures::CHANGE, ending: ReleaseEnding::Cancelled },
    );
    assert!(d.landing(61).expect("landing").landing.members.is_empty());
    let requests = outputs(
        &mut d,
        Event::StepLanding { task: 61, entry: 92, heard: LandingHeard { released: false, cancelled: false } },
    );
    assert!(requests.contains(&Request::LandingDecision {
        task: 61,
        decision: policy::Decision::Cancel,
        entry: None,
        evidence: None
    }));
}
