use crate::{
    jig::{conditions, subject::Condition},
    tests::fixtures as f,
};
use jig_client::{Latest, contract as c};
use skein_lib::Wall;
#[test]
fn replied_holds_once_a_chat_is_parked_and_not_while_it_writes() {
    let me = f::me(f::PARTY);
    let writing = f::chat_at(f::TASK, c::RunStage::Running { attempt: f::ATTEMPT }, Some(c::OwnerChipWork::Writing));
    let parked = f::chat_at(f::TASK, c::RunStage::Idle, None);
    assert_eq!(f::holds(Condition::Replied, &writing, Some(&me)), Some(false));
    assert_eq!(f::holds(Condition::Replied, &parked, Some(&me)), Some(true));
}
#[test]
fn each_live_phase_holds_only_for_its_chip_phase() {
    let mut latest = f::chat_at(f::TASK, c::RunStage::Idle, None);
    for phase in f::LIVE_PHASES {
        f::task_in(&mut latest).chip.phase = *phase;
        for wanted in f::LIVE_PHASES {
            assert_eq!(
                f::holds(Condition::Phase(*wanted), &latest, None),
                Some(phase == wanted),
                "{phase:?}/{wanted:?}"
            );
        }
    }
}
#[test]
fn replied_accepts_each_settled_chat_status_and_misses_the_others() {
    let me = f::me(f::PARTY);
    let rows = [
        (c::RunStage::Idle, None, true),
        (c::RunStage::Running { attempt: f::ATTEMPT }, Some(c::OwnerChipWork::Waiting), true),
        (c::RunStage::Due, None, false),
        (c::RunStage::Preparing, None, false),
        (c::RunStage::Claimed { attempt: f::ATTEMPT }, None, false),
        (c::RunStage::Running { attempt: f::ATTEMPT }, Some(c::OwnerChipWork::Preparing), false),
        (c::RunStage::Running { attempt: f::ATTEMPT }, Some(c::OwnerChipWork::Starting), false),
        (c::RunStage::Running { attempt: f::ATTEMPT }, Some(c::OwnerChipWork::Writing), false),
        (c::RunStage::Running { attempt: f::ATTEMPT }, Some(c::OwnerChipWork::Active), false),
        (c::RunStage::Running { attempt: f::ATTEMPT }, Some(c::OwnerChipWork::Ending), false),
        (
            c::RunStage::Running { attempt: f::ATTEMPT },
            Some(c::OwnerChipWork::Tool { call: b"c".as_slice().into(), tool: b"sh".as_slice().into() }),
            false,
        ),
        (c::RunStage::BackingOff { until: Wall::EPOCH }, None, false),
    ];
    for (stage, work, expected) in rows {
        let latest = f::chat_at(f::TASK, stage, work);
        assert_eq!(f::holds(Condition::Replied, &latest, Some(&me)), Some(expected), "{stage:?}");
    }
    let mut latest = f::chat_at(f::TASK, c::RunStage::Idle, None);
    for whom in [c::Whom::Person(f::PARTY), c::Whom::Person(f::OTHER_PARTY)] {
        f::task_in(&mut latest).waits_on = f::party_wait(whom, c::WaitedFor::Proposal);
        assert_eq!(f::holds(Condition::Replied, &latest, Some(&me)), Some(true), "{whom:?}");
    }
    for phase in [
        c::OwnerChipPhase::Held { reason: c::HoldCategory::Stopped, revision: f::HOLD },
        c::OwnerChipPhase::Closing(c::ClosingStage::Delegates),
    ] {
        f::chip_phase(&mut latest, phase);
        assert_eq!(f::holds(Condition::Replied, &latest, Some(&me)), Some(true));
    }
    let ended = f::ended_chat(c::Phase::Ended);
    assert_eq!(f::holds(Condition::Replied, &ended, Some(&me)), Some(true));
}
#[test]
fn waiting_on_me_uses_the_party_and_its_project_role() {
    let mut me = f::me(f::PARTY);
    let rows = [
        (c::HeldRole::Owner, 0),
        (c::HeldRole::Maintainer, 1),
        (c::HeldRole::Member, 2),
        (c::HeldRole::Observer, 3),
        (c::HeldRole::Policy { role: f::POLICY_ROLE }, f::POLICY_ROLE),
    ];
    for (role, number) in rows {
        me.projects.get_mut(f::EMPTY).expect("project fixture").role = role;
        for (whom, project, expected) in [
            (c::Whom::Person(f::PARTY), f::PROJECT, true),
            (c::Whom::Person(f::OTHER_PARTY), f::PROJECT, false),
            (c::Whom::Role { project: f::PROJECT, role: number }, f::PROJECT, true),
            (c::Whom::Role { project: f::PROJECT, role: number }, f::OTHER_PROJECT, false),
            (c::Whom::Role { project: f::OTHER_PROJECT, role: number }, f::OTHER_PROJECT, false),
        ] {
            for kind in [c::WaitedFor::Question, c::WaitedFor::Proposal] {
                assert_eq!(
                    conditions::waits_on_me(&f::party_wait(whom, kind), project, &me),
                    expected,
                    "{role:?}/{whom:?}/{kind:?}"
                );
            }
        }
    }
}
#[test]
fn waiting_on_me_reads_waits_even_while_the_task_is_held() {
    let me = f::me(f::PARTY);
    let mut latest = f::chat_at(f::TASK, c::RunStage::Idle, None);
    f::chip_phase(&mut latest, c::OwnerChipPhase::Held { reason: c::HoldCategory::Stopped, revision: f::HOLD });
    f::task_in(&mut latest).waits_on = f::waiting_on_me();
    assert_eq!(f::holds(Condition::WaitingOnMe, &latest, Some(&me)), Some(true));
}
#[test]
fn ended_follows_the_task_or_tree_root_phase() {
    let mut latest = f::chat_at(f::TASK, c::RunStage::Idle, None);
    for phase in f::LIVE_PHASES.iter().chain(f::ENDED_PHASES.iter()) {
        let task = f::task_in(&mut latest);
        task.chip.phase = *phase;
        let chip = task.chip.clone();
        let expected = f::ENDED_PHASES.contains(phase);
        for subject in [&latest, &f::tree_at(chip.clone()), &f::owner_tree_at(chip)] {
            assert_eq!(f::holds(Condition::Ended, subject, None), Some(expected), "{phase:?}");
        }
    }
}
#[test]
fn held_follows_the_task_phase() {
    let mut latest = f::chat_at(f::TASK, c::RunStage::Idle, None);
    for phase in f::LIVE_PHASES.iter().chain(f::ENDED_PHASES.iter()) {
        f::task_in(&mut latest).chip.phase = *phase;
        assert_eq!(f::holds(Condition::Held, &latest, None), Some(*phase == c::Phase::Held), "{phase:?}");
    }
}
#[test]
fn inbox_conditions_use_their_supplied_counts() {
    for (condition, needs, updates, expected) in [
        (Condition::Waiting, f::EMPTY, f::ONE_NEED, false),
        (Condition::Waiting, f::ONE_NEED, f::EMPTY, true),
        (Condition::Updates, f::ONE_NEED, f::EMPTY, false),
        (Condition::Updates, f::EMPTY, f::ONE_NEED, true),
    ] {
        for subject in [f::inbox_counts(needs, updates), f::owner_inbox_counts(needs, updates)] {
            assert_eq!(f::holds(condition, &subject, None), Some(expected), "{condition:?}/{needs}/{updates}");
        }
    }
}
#[test]
fn me_waiting_uses_its_supplied_total() {
    let mut me = f::me(f::PARTY);
    for (needs, expected) in [(f::EMPTY, false), (f::ONE_NEED, true)] {
        me.needs_total = needs;
        assert_eq!(
            f::holds(Condition::Waiting, &Latest::Jig(c::Delivery::Me(Box::new(me.clone()))), None),
            Some(expected)
        );
    }
}
#[test]
fn a_condition_waits_for_each_document_it_reads_to_be_live() {
    let task = f::chat_at(f::TASK, c::RunStage::Idle, None);
    let run = Latest::Jig(f::no_provisional());
    for (condition, subject) in [
        (Condition::Replied, &task),
        (Condition::WaitingOnMe, &task),
        (Condition::WaitingOnMe, &run),
        (Condition::Waiting, &task),
        (Condition::Updates, &task),
    ] {
        assert_eq!(f::holds(condition, subject, None), None, "{condition:?}");
    }
}
