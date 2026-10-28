use super::*;
use crate::reconcile::{Reconciliation, Reference};
use alloc::boxed::Box;
use skein_lib::{Time, Wall};

const TASK: u64 = 7;
const ENTRY: u64 = 11;
const OTHER_TASK: u64 = 99;
const OTHER_FORGE: u16 = 99;
const OTHER_REPOSITORY: skein_forge::api::Repository = skein_forge::api::Repository { forge: 1, repository: 99 };

fn resolved(d: &Domain, row: &Stored, live: bool) -> bool {
    let tasks: &[u64] = if live { &[TASK] } else { &[] };
    crate::reconcile::references_resolve(d, &LIMITS, row, tasks)
}
fn held_domain() -> Domain {
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    outputs(&mut d, Event::Restore { record: Stored::Entry { entry: status(ENTRY, TASK), held: true } });
    d
}
fn issue(closed: bool) -> Stored {
    Stored::Issue(IssueRow {
        goal: TASK,
        repository: REPO,
        state: temper_engine_domain_forge_issues::Projected { closed, ..Default::default() },
        number: None,
        pending: None,
        before: None,
        failed: false,
        desired: None,
    })
}

#[test]
fn an_ended_goals_pending_projection_entry_restores() {
    for (pending, repository, expected) in [
        (Some(ENTRY), REPO, true),
        (Some(ENTRY + 1), REPO, false),
        (None, REPO, false),
        (Some(ENTRY), OTHER_REPOSITORY, false),
    ] {
        let mut d = domain();
        let mut entry = held_issue();
        entry.effect.write = skein_forge::api::Write::Post {
            number: 1,
            key: Box::from(b"milestone".as_slice()),
            body: Box::from(b"Goal finished".as_slice()),
        };
        outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
        outputs(&mut d, Event::Restore { record: Stored::Entry { entry: entry.clone(), held: false } });
        let Stored::Issue(mut row) = issue(false) else { unreachable!("issue fixture") };
        row.pending = pending;
        row.repository = repository;
        row.number = Some(1);
        row.state.opened = true;
        row.desired = Some(issues::GoalView {
            phase: issues::Phase::Done,
            goal: TASK,
            repository: u64::from(REPO.repository),
            title: "Work".into(),
            goal_text: "Complete the work".into(),
            plan: Box::new([]),
            milestones: Box::new([]),
            finished: Some("Goal finished".into()),
        });
        outputs(&mut d, Event::Restore { record: Stored::Issue(row) });
        assert_eq!(resolved(&d, &Stored::Entry { entry, held: false }, false), expected);
        assert_eq!(resolved(&d, &Stored::Flight { task: TASK, entry: ENTRY }, false), expected);
    }
}
fn release() -> Stored {
    Stored::Release(ReleaseRow {
        task: TASK,
        root: TASK,
        ending: ReleaseEnding::Done,
        close_pull: None,
        comment: None,
        pending: None,
        failed: false,
        effects_settled: true,
        releasing: true,
    })
}
fn subscription() -> Stored {
    Stored::Subscription(Subscriber {
        markers: 0,
        task: TASK,
        number: 1,
        topic: Topic::Ci { repository: REPO, head: [3; 32] },
        own_change: Some(OTHER_TASK),
        goal_tasks: Some(Box::new([OTHER_TASK])),
        paths: Box::new([]),
    })
}
fn hold(task: u64, writer: Option<Writer>) -> Stored {
    Stored::Hold(Hold { name: branch(), task, writer, drift: None })
}
fn owner_rows() -> [Stored; 8] {
    [
        Stored::Call {
            key: NamedCall { task: TASK, attempt: 1, completion: 1, position: 1 },
            answer: CallPayload::Refused(skein_forge::api::Error::Refused),
        },
        Stored::ProposedEffect(ProposedEffect { number: 1, resource: What::Repository, entry: status(ENTRY, TASK) }),
        hold(TASK, Some(Writer::Lost { task: TASK, attempt: 1 })),
        Stored::Names { task: TASK, resources: Box::new([branch()]) },
        subscription(),
        Stored::Entry { entry: status(ENTRY, TASK), held: true },
        issue(false),
        release(),
    ]
}
fn repository_rows(repository: skein_forge::api::Repository) -> [Stored; 7] {
    let mut name = branch();
    name.forge = repository.forge;
    name.repository = repository.repository;
    let mut change = fixtures::opening_row();
    change.repository = repository;
    [
        Stored::BranchHead(BranchHead { name: name.clone(), commit: [3; 32], owned: true }),
        Stored::PullState(PullState {
            name,
            head: [3; 32],
            ci: skein_forge::api::Ci::Passed,
            state: skein_forge::api::State::Open,
        }),
        Stored::Ci(CiState { repository, head: [3; 32], status: skein_forge::api::Ci::Passed }),
        Stored::Client(skein_forge_client::Stored::Repository(skein_forge_client::RepositoryRecord {
            repository,
            mark: None,
            clock: Time::ZERO,
        })),
        Stored::Client(skein_forge_client::Stored::Live(skein_forge_client::LiveRecord {
            watch: skein_forge_client::Watch {
                resource: skein_forge_client::Resource { repository, what: skein_forge_client::What::Repository },
                participating: false,
            },
            position: skein_forge_client::Position { at: Time::ZERO, comment: 0, review: 0, review_page: 0 },
            listed: Time::ZERO,
            comment_after: 0,
            comment_page: 0,
            comment_scanning: false,
            comment_repair: false,
            review_page: 0,
            comments: Box::new([]),
            reviews: Box::new([]),
            cached: Box::new(skein_forge_client::Cached { item: None, pull: None, tip: None }),
            pushed: None,
            echoes: Box::new([]),
        })),
        Stored::Change(change),
        Stored::Application {
            token: 1,
            until: Wall::EPOCH,
            outcome: ApplicationOutcome::Adopted(AdoptedRepository {
                repository,
                owner: b"org".as_slice().into(),
                name: b"repo".as_slice().into(),
                role: Role::Owned,
                home: true,
                kinds: adopted().kinds,
                protection: Protection::Absent,
            }),
        },
    ]
}
#[test]
fn live_owners_resolve_for_every_live_record_family() {
    let d = held_domain();
    for row in owner_rows() {
        assert!(resolved(&d, &row, true), "{row:?}");
    }
}
#[test]
fn absent_owners_refuse_for_every_live_record_family() {
    let d = held_domain();
    for row in owner_rows() {
        assert!(!resolved(&d, &row, false), "{row:?}");
    }
}
#[test]
fn held_repositories_resolve_for_every_repository_record_family() {
    let d = held_domain();
    for row in repository_rows(REPO) {
        assert!(resolved(&d, &row, true), "{row:?}");
    }
}
#[test]
fn absent_repositories_refuse_for_every_repository_record_family() {
    let d = held_domain();
    for row in repository_rows(OTHER_REPOSITORY) {
        assert!(!resolved(&d, &row, true), "{row:?}");
    }
}
#[test]
fn historical_records_need_no_live_task_or_member() {
    let d = held_domain();
    for row in [
        Stored::Landed { task: OTHER_TASK, commit: [9; 32] },
        Stored::Change(fixtures::opening_row()),
        issue(true),
        Stored::Binding { address: b"former@example.com".as_slice().into(), subject: b"former".as_slice().into() },
        Stored::DemoAct(DemoAct { key: [1; 16], act: DemoActKind::Close { project: 1, task: OTHER_TASK } }),
        Stored::DemoAct(DemoAct { key: [2; 16], act: DemoActKind::RemoveAccount { account: 99 } }),
        Stored::Application {
            token: 1,
            until: Wall::EPOCH,
            outcome: ApplicationOutcome::ForgeRefused(AdoptionRefusal::NotFound),
        },
    ] {
        assert!(resolved(&d, &row, false), "{row:?}");
    }
}
#[test]
fn configured_forges_resolve_without_being_connected() {
    let d = held_domain();
    for row in [Stored::Repository(adopted()), Stored::Writer(skein_forge_client::Writer { forge: 1, author: 7 })] {
        assert!(resolved(&d, &row, false), "{row:?}");
    }
}
#[test]
fn a_restored_writer_does_not_configure_a_missing_forge() {
    let mut d = held_domain();
    let mut repository = adopted();
    repository.provider.forge = OTHER_FORGE;
    let writer = skein_forge_client::Writer { forge: OTHER_FORGE, author: 7 };
    outputs(&mut d, Event::Restore { record: Stored::Writer(writer) });
    for row in [Stored::Repository(repository), Stored::Writer(writer)] {
        assert!(!resolved(&d, &row, true), "{row:?}");
    }
}
#[test]
fn flights_resolve_only_to_the_entry_of_the_same_task() {
    let d = held_domain();
    assert!(resolved(&d, &Stored::Flight { task: TASK, entry: ENTRY }, true));
}
#[test]
fn flights_refuse_an_absent_entry_or_another_tasks_entry() {
    let d = held_domain();
    for row in [Stored::Flight { task: TASK, entry: 99 }, Stored::Flight { task: OTHER_TASK, entry: ENTRY }] {
        assert!(!resolved(&d, &row, true), "{row:?}");
    }
}
#[test]
fn a_demo_record_requires_the_configured_demo() {
    let d = held_domain();
    assert!(!resolved(&d, &Stored::Demo(Demo { forge: 1, state: DemoState::Ended }), false));
}
#[test]
fn reconciliation_reference_storage_is_priced_by_its_fields() {
    // Need: 8-byte tag plus 16-byte repository/task-entry union; row: 4,
    // kind: 1, padding: 3. Every retained reference occupies 32 bytes.
    const REFERENCE_BYTES: usize = 32;
    // List header 32, row offset 4, verdict 8, alignment padding 4.
    const RECONCILIATION_BYTES: usize = 48;
    const DEMO_BYTES: usize = 4;
    assert_eq!(size_of::<Reference>(), REFERENCE_BYTES);
    assert_eq!(size_of::<Reconciliation>(), RECONCILIATION_BYTES);
    assert_eq!(size_of::<Option<Demo>>(), DEMO_BYTES);
}

fn demo_domain() -> Domain {
    let mut d = held_domain();
    d.configure_demo(Some(DemoConfig {
        provider: 1,
        adoption: fixtures::adoption(),
        deadline: Duration::from_secs(10),
    }));
    outputs(&mut d, Event::Restore { record: Stored::Demo(Demo { forge: 1, state: DemoState::Ended }) });
    d
}
#[test]
fn the_configured_ended_demo_and_its_retired_owners_resolve() {
    let d = demo_domain();
    for row in [
        Stored::Demo(Demo { forge: 1, state: DemoState::Ended }),
        Stored::Names { task: OTHER_TASK, resources: Box::new([branch()]) },
        hold(OTHER_TASK, Some(Writer::Lost { task: OTHER_TASK, attempt: 1 })),
        subscription(),
        Stored::Entry { entry: status(ENTRY, OTHER_TASK), held: true },
        issue(false),
        Stored::Flight { task: TASK, entry: ENTRY },
    ] {
        assert!(resolved(&d, &row, false), "{row:?}");
    }
}
#[test]
fn an_ended_demo_does_not_resolve_a_different_demo_record() {
    assert!(!resolved(&demo_domain(), &Stored::Demo(Demo { forge: OTHER_FORGE, state: DemoState::Ended }), false));
}
#[test]
fn a_mixed_resource_set_still_requires_its_live_owner() {
    let d = demo_domain();
    let mut other = branch();
    other.forge = OTHER_FORGE;
    assert!(!resolved(&d, &Stored::Names { task: OTHER_TASK, resources: Box::new([branch(), other]) }, false));
}

#[test]
fn a_held_writer_must_resolve_independently_of_the_owner() {
    let d = held_domain();
    let row = hold(TASK, Some(Writer::Run { task: OTHER_TASK, attempt: 1 }));
    assert!(!resolved(&d, &row, true));
}
#[test]
fn a_release_root_must_resolve_independently_of_the_closing_task() {
    let d = held_domain();
    let Stored::Release(mut row) = release() else { unreachable!("release fixture") };
    row.root = OTHER_TASK;
    assert!(!resolved(&d, &Stored::Release(row), true));
}
#[test]
fn every_resource_bearing_owner_refuses_a_missing_repository() {
    let d = domain();
    // Call has no repository. Release's task/root are checked separately.
    for row in [
        Stored::ProposedEffect(ProposedEffect { number: 1, resource: What::Repository, entry: status(ENTRY, TASK) }),
        hold(TASK, None),
        Stored::Names { task: TASK, resources: Box::new([branch()]) },
        subscription(),
        Stored::Entry { entry: status(ENTRY, TASK), held: true },
        issue(false),
    ] {
        assert!(!resolved(&d, &row, true), "{row:?}");
    }
}

#[test]
fn a_flight_on_a_live_forge_requires_its_entrys_task() {
    assert!(!resolved(&held_domain(), &Stored::Flight { task: TASK, entry: ENTRY }, false));
}
