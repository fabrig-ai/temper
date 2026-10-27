//! A landing of one and its documentation successor (domain/landing.md, 16).
use skein_fake_forge_domain::api as raw;
use skein_forge::api;
use temper_engine_domain_forge as top;
use temper_engine_domain_forge_change as change;
use temper_engine_domain_forge_landing as landing;
use temper_engine_forge_world::{
    REPO, World,
    landings::{self, CHANGE, LANDING},
    translate,
};

const DOC_CHANGE: u64 = 52;
const DOC_LANDING: u64 = 62;
const NEXT_CHANGE: u64 = 53;
const NEXT_LANDING: u64 = 63;

fn runs(world: &World) -> Box<[raw::Run]> {
    let raw::Answer::Runs(runs) = world.inspect(&raw::Read::Runs {
        workflow: Some(Box::from(b"gate.yml".as_slice())),
        commit: None,
        number: None,
        page: 1,
        limit: 16,
    }) else {
        panic!("workflow runs")
    };
    runs
}
fn receipt(world: &World) -> (&top::ReceiptKey, &top::Receipt) {
    world
        .stored()
        .iter()
        .find_map(|(_, row)| if let top::Stored::Receipt { key, receipt } = row { Some((key, receipt)) } else { None })
        .expect("saved receipt")
}
fn hex(bytes: &[u8]) -> Box<[u8]> {
    bytes.iter().flat_map(|byte| top::hex(*byte)).collect()
}
#[test]
fn a_landing_of_one_is_rebased_proved_by_one_run_and_fast_forwarded() {
    let mut world = landings::first_landing();
    let candidate = translate::commit(world.branch(b"temper/51"));
    let row = world.change(CHANGE);
    assert_eq!(row.change.state, change::State::Landed { merge: candidate });
    assert_eq!(translate::commit(world.branch(b"main")), candidate);
    assert_eq!(world.landing(LANDING).landing.phase, landing::Phase::Landed);
    let raw::Answer::Pull(pull) = world.inspect(&raw::Read::Pull { number: row.pull.expect("opened pull") }) else {
        panic!("pull")
    };
    assert_eq!(pull.merged, Some(world.branch(b"main")));
    let (key, receipt) = receipt(&world);
    assert_eq!(key.repository, REPO);
    assert!(receipt.passed);
    assert_eq!(receipt.candidate, candidate);
    let runs = runs(&world);
    assert_eq!(runs.len(), 1);
    assert_eq!(
        runs[0].inputs.as_ref(),
        &[
            (Box::from(b"temper_commit".as_slice()), hex(&candidate[..20])),
            (Box::from(b"temper_key".as_slice()), hex(&key.key))
        ]
    );
    let link = receipt.run.url.clone();
    let short = String::from_utf8(hex(&key.key[..8]).into_vec()).expect("hex");
    let writes: Vec<_> = world
        .take_calls()
        .into_iter()
        .filter_map(|op| match op {
            api::Op::Write(write) => Some(write),
            api::Op::Read(_) => None,
        })
        .collect();
    assert_eq!(writes.len(), 5);
    assert!(matches!(writes[0], api::Write::Update { style: api::UpdateStyle::Rebase, .. }));
    assert!(matches!(writes[1], api::Write::Dispatch { .. }));
    for (index, check, description, expected_link) in [
        (2, api::Check::Pending, format!("key {short}"), Box::<[u8]>::from([])),
        (3, api::Check::Passed, format!("key {short} passed"), link),
    ] {
        assert_eq!(
            writes[index],
            api::Write::Status {
                commit: candidate,
                context: Box::from(b"temper/gate/gate".as_slice()),
                check,
                description: description.into_bytes().into_boxed_slice(),
                link: expected_link
            }
        );
    }
    assert!(
        matches!(writes[4], api::Write::Merge { head, how: api::MergeHow::FastForwardOnly, .. } if head == candidate)
    );
}
#[test]
fn a_documentation_landing_lands_on_its_receipt_with_no_run() {
    let mut world = landings::first_landing();
    let first_key = receipt(&world).0.key;
    let first_link = receipt(&world).1.run.url.clone();
    world.take_calls();
    landings::documentation_landing(&mut world, DOC_CHANGE, DOC_LANDING, b"temper/52");
    let candidate = translate::commit(world.branch(b"temper/52"));
    assert_eq!(world.change(DOC_CHANGE).change.state, change::State::Landed { merge: candidate });
    assert_eq!(translate::commit(world.branch(b"main")), candidate);
    assert_eq!(world.landing(DOC_LANDING).landing.members[0].gates[0].proof, landing::Proof::Passed { key: first_key });
    assert_eq!(runs(&world).len(), 1);
    let writes: Vec<_> = world
        .take_calls()
        .into_iter()
        .filter_map(|op| match op {
            api::Op::Write(write) => Some(write),
            api::Op::Read(_) => None,
        })
        .collect();
    assert_eq!(writes.len(), 3);
    assert!(matches!(writes[0], api::Write::Update { style: api::UpdateStyle::Rebase, .. }));
    let short = String::from_utf8(hex(&first_key[..8]).into_vec()).expect("hex");
    assert_eq!(
        writes[1],
        api::Write::Status {
            commit: candidate,
            context: Box::from(b"temper/gate/gate".as_slice()),
            check: api::Check::Passed,
            description: format!("key {short} passed by receipt").into_bytes().into_boxed_slice(),
            link: first_link
        }
    );
    assert!(
        matches!(writes[2], api::Write::Merge { head, how: api::MergeHow::FastForwardOnly, .. } if head == candidate)
    );
}

#[test]
fn a_restart_finds_the_dispatched_run_without_dispatching_again() {
    const BEFORE_DISPATCH_STEPS: u64 = 20;
    let mut world = landings::turn_world();
    world.produce(b"temper/51");
    world.push_file(b"main", b"base", b"main moved");
    landings::ready_change(&mut world, CHANGE, b"temper/51");
    let row = landings::landing_row(world.change(CHANGE));
    world.event(top::Event::Landing { row });
    let mut dispatched = false;
    for position in 0..BEFORE_DISPATCH_STEPS {
        let entry = 100 + position * 2;
        let decision = landings::step_landing(&mut world, LANDING, entry);
        temper_engine_forge_world::changes::change_step_task(
            &mut world,
            CHANGE,
            entry + 1,
            change::Status::Passed,
            Box::new([]),
            false,
            false,
        );
        if matches!(decision, landing::Decision::Dispatch { .. }) {
            dispatched = true;
            break;
        }
    }
    assert!(dispatched);
    world.restart();
    world.run_for(1);
    landings::drive_landing(&mut world, LANDING);
    assert_eq!(runs(&world).len(), 1);
    assert_eq!(world.landing(LANDING).landing.phase, landing::Phase::Landed);
}

#[test]
fn a_finished_landing_surrenders_its_live_slot_to_the_next() {
    let mut world = landings::first_landing();
    landings::documentation_landing(&mut world, DOC_CHANGE, DOC_LANDING, b"temper/52");
    landings::documentation_landing(&mut world, NEXT_CHANGE, NEXT_LANDING, b"temper/53");
    assert_eq!(world.landing(NEXT_LANDING).landing.phase, landing::Phase::Landed);
    assert!(!world.stored().contains_key(&top::Key::Landing(LANDING)));
}

#[test]
fn a_failed_dispatch_still_expires_after_restart() {
    let mut world = landings::turn_world();
    assert!(world.project_landing(
        landings::PROJECT,
        change::Way::Turn,
        Box::new([top::RepositoryGates {
            repository: REPO,
            gates: Box::new([landing::Gate {
                name: Box::from(b"gate".as_slice()),
                workflow: Box::from(b"missing.yml".as_slice()),
                ignored: Box::new([]),
                blocking: true,
            }]),
        }]),
    ));
    world.produce(b"temper/51");
    landings::ready_change(&mut world, CHANGE, b"temper/51");
    let row = landings::landing_row(world.change(CHANGE));
    world.event(top::Event::Landing { row });
    let mut dispatched = false;
    for position in 0..20 {
        let entry = 100 + position * 2;
        let decision = landings::step_landing(&mut world, LANDING, entry);
        temper_engine_forge_world::changes::change_step_task(
            &mut world,
            CHANGE,
            entry + 1,
            change::Status::Passed,
            Box::new([]),
            false,
            false,
        );
        if matches!(decision, landing::Decision::Dispatch { .. }) {
            dispatched = true;
            break;
        }
    }
    assert!(dispatched);
    world.restart();
    world.run_for(11);
    let mut held = false;
    for position in 0..20 {
        if matches!(landings::step_landing(&mut world, LANDING, 1000 + position), landing::Decision::Hold(_)) {
            held = true;
            break;
        }
    }
    assert!(held, "failed dispatch must reach its bounded retry limit after restart");
}
