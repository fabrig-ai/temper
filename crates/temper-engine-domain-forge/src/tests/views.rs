use super::*;
use jig_domain_people::ApplicationScope;
use temper_engine_domain_forge_landing as policy;

fn row(task: u64, phase: policy::Phase) -> LandingRow {
    LandingRow {
        landing: policy::Landing {
            task,
            project: 5,
            phase,
            preparation: 1,
            stale_proofs: 2,
            members: Box::new([policy::Member {
                change: fixtures::CHANGE,
                repository: policy::Repository { forge: REPO.forge, repository: REPO.repository },
                pull: 1,
                head: Some(fixtures::HEAD),
                base: Some(fixtures::BASE),
                candidate: Some(fixtures::HEAD),
                gates: Box::new([policy::GateProof {
                    proof: policy::Proof::Dispatched { key: [7; 32], try_: 2 },
                    posted: policy::Posted::Pending,
                    faults: 1,
                }]),
                asked: None,
                landed: None,
            }]),
        },
        asks: Box::new([None]),
        entries: Box::new([]),
    }
}
fn setup() -> Domain {
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    let mut change = fixtures::opening_row();
    change.pull = Some(1);
    outputs(&mut d, Event::Restore { record: Stored::Change(change) });
    assert!(d.project_landing(
        5,
        temper_engine_domain_forge_change::Way::Turn,
        Box::new([RepositoryGates {
            repository: REPO,
            gates: Box::new([policy::Gate {
                name: Box::from(b"gate".as_slice()),
                workflow: Box::from(b"gate.yml".as_slice()),
                ignored: Box::new([]),
                blocking: true
            }])
        }])
    ));
    d
}
fn document(d: &Domain, task: u64) -> Document {
    d.document(task, ApplicationScope::Project(5), env().wall, &LIMITS).expect("document")
}
fn view(d: &Domain, task: u64) -> LandingView {
    let Document::Landing(view) = document(d, task) else { panic!("landing view") };
    view
}
fn list(d: &Domain, ended: bool) -> LandingsView {
    let Document::Landings(view) = document(d, landings_subject(5, ended)) else { panic!("list view") };
    view
}
#[test]
fn a_landing_view_shows_its_member_gate_and_turn() {
    let mut d = setup();
    outputs(&mut d, Event::Restore { record: Stored::Landing(row(61, policy::Phase::Proving)) });
    outputs(&mut d, Event::Restore { record: Stored::Landing(row(62, policy::Phase::Turn { at: env().wall })) });
    let got = view(&d, 61);
    assert_eq!((got.task, got.project, got.turn, got.preparation, got.stale_proofs), (61, 5, Some(62), 1, 2));
    assert_eq!(got.phase, policy::Phase::Proving);
    let member = &got.members[0];
    assert_eq!(
        (member.change, member.pull, member.base, member.candidate),
        (51, Some(1), Some(fixtures::BASE), Some(fixtures::HEAD))
    );
    assert_eq!(member.branch.as_ref(), b"temper/51");
    assert_eq!(
        member.gates[0],
        LandingGate {
            name: Box::from(b"gate".as_slice()),
            blocking: true,
            key: Some([7; 32]),
            proof: GateProof::Dispatched { attempt: 2, run: None },
            faults: 1
        }
    );
    assert!(d.document(61, ApplicationScope::Project(6), env().wall, &LIMITS).is_none());
}
#[test]
fn a_gate_whose_key_another_landing_dispatched_is_waiting() {
    let mut d = setup();
    outputs(&mut d, Event::Restore { record: Stored::Landing(row(61, policy::Phase::Proving)) });
    let key =
        ReceiptKey { project: 5, repository: REPO, check: Check::Gate(Box::from(b"gate".as_slice())), key: [7; 32] };
    let mut flight = runs::follow();
    flight.task = 62;
    d.landing.flights.insert(key.clone(), flight).unwrap();
    assert_eq!(view(&d, 61).members[0].gates[0].proof, GateProof::Waiting);
    let flight = d.landing.flights.get_mut(&key).unwrap();
    flight.task = 61;
    flight.run = Some(skein_forge::api::Run {
        id: 7,
        number: 3,
        workflow: Box::from(b"gate.yml".as_slice()),
        commit: fixtures::HEAD,
        event: skein_forge::api::Event::Dispatch,
        actor: 1,
        state: skein_forge::api::RunState::Running,
        conclusion: None,
        url: Box::from(b"https://forge.example/run/7".as_slice()),
        created: env().now,
        jobs: Box::new([]),
    });
    assert_eq!(view(&d, 61).members[0].gates[0].proof, GateProof::Dispatched { attempt: 2, run: Some(7) });
    let flight = d.landing.flights.remove(&key).unwrap();
    d.landing.faults.insert((61, 0, 0), flight).unwrap();
    assert_eq!(view(&d, 61).members[0].gates[0].proof, GateProof::Dispatched { attempt: 2, run: Some(7) });
    let receipt = Receipt {
        failures: None,
        passed: true,
        candidate: fixtures::HEAD,
        ended: env().wall,
        run: ReceiptRun { id: 7, number: 3, url: Box::from(b"https://forge.example/run/7".as_slice()) },
    };
    d.landing.receipts.insert(key, receipt.clone()).unwrap();
    assert_eq!(view(&d, 61).members[0].gates[0].proof, GateProof::Passed(receipt.run));
}
#[test]
fn landings_list_the_turns_holder_first_then_newest() {
    let mut d = setup();
    for (task, phase) in
        [(61, policy::Phase::Turn { at: env().wall }), (62, policy::Phase::Proving), (63, policy::Phase::Waiting)]
    {
        outputs(&mut d, Event::Restore { record: Stored::Landing(row(task, phase)) });
    }
    let got = list(&d, false);
    assert_eq!(got.turn, Some(61));
    assert_eq!([got.landings[0].task, got.landings[1].task, got.landings[2].task], [61, 63, 62]);
    assert_eq!(got.landings[0].repositories[0].as_ref(), b"org/repo");
}
#[test]
fn ended_landings_are_listed_only_when_asked() {
    let mut d = setup();
    for (task, phase) in [(61, policy::Phase::Landed), (62, policy::Phase::Cancelled), (63, policy::Phase::Proving)] {
        outputs(&mut d, Event::Restore { record: Stored::Landing(row(task, phase)) });
    }
    assert_eq!(list(&d, false).landings.len(), 1);
    assert_eq!(list(&d, true).landings.len(), 3);
}
#[test]
fn saving_a_landing_marks_its_subject_and_its_projects_lists() {
    let mut d = setup();
    let mut row = row(61, policy::Phase::Waiting);
    row.landing.members[0].gates[0].proof = policy::Proof::Unkeyed;
    let got = outputs(&mut d, Event::Landing { row });
    let mut landing_marked = false;
    let mut member_marked = false;
    for request in &got {
        if let Request::Marked { tasks } = request {
            landing_marked |= tasks.as_ref() == [61, landings_subject(5, false), landings_subject(5, true)];
            member_marked |= tasks.as_ref() == [51, 61];
        }
    }
    assert!(landing_marked && member_marked);
}
