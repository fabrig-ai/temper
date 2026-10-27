use crate::*;
use temper_engine_domain_forge_issues as issues;
mod adoption;
mod binding;
mod change;
#[path = "tests/brief.rs"]
mod ci_brief;
mod connect;
mod demo;
mod fixtures;
mod flight;
mod panel;
mod projection;
mod purpose;
mod reconcile;
mod stopping;
use alloc::boxed::Box;
use skein_lib::{Duration, Env, Queue, Time, Token, Wall};

const REPO: skein_forge::api::Repository = skein_forge::api::Repository { forge: 1, repository: 2 };
const CLIENT: skein_forge_client::Limits = skein_forge_client::Limits {
    pending: 4,
    calls: 2,
    rate: 100,
    reserve: 0,
    window: Duration::from_secs(10),
    op_bytes: 512,
    answer_bytes: 4096,
    rows: 8,
    inbox: 16,
    read_attempts: 3,
    backoff: Duration::from_secs(1),
    backoff_max: Duration::from_secs(4),
    entries: 8,
    write_attempts: 3,
    lifetime: Duration::from_secs(10),
    clock_margin: Duration::from_secs(1),
    resources: 8,
    repositories: 2,
    poll: Duration::from_secs(5),
    poll_max: Duration::from_secs(30),
    hinted: Duration::from_secs(1),
    slow: Duration::from_secs(40),
    settle: Duration::from_secs(30),
    facts: 16,
};
const LIMITS: Limits = Limits {
    repositories: 2,
    tasks: 8,
    holds: 8,
    subscriptions: 8,
    entries: 8,
    named_calls: 16,
    changes: 8,
    issues: 8,
    resources_per_task: 4,
    paths_per_subscription: 4,
    // forge/host/owner/repository/branch/temper/root/marker-task.
    authority_segments: 8,
    name_bytes: 128,
    output: 80,
    facts: 16,
    adoptions: 2,
    applications: 4,
    bindings: 4,
    collaborators: 8,
    landings: 8,
    judge_projects: 2,
    judge_criteria: 8,
    brief_sections: 32,
    brief_bytes: 4096,
    issue_policy: temper_engine_domain_forge_issues::Limits {
        plan_items: 8,
        milestones: 8,
        title_bytes: 128,
        body_bytes: 512,
        comment_bytes: 256,
        interval: Duration::from_secs(5),
    },
    change_policy: temper_engine_domain_forge_change::Limits {
        repairs: 3,
        resolutions: 3,
        updates: 3,
        stall: Duration::from_secs(30),
        gates: 8,
        clean_heads: 8,
    },
    queue_window: Duration::from_secs(30),
    client: CLIENT,
};

fn env() -> Env<Limits> {
    Env { now: Time::from_nanos(100_000_000_000), wall: Wall::from_nanos(100_000_000_000), limits: LIMITS }
}
fn domain() -> Domain {
    Domain::new(
        &LIMITS,
        4,
        skein_forge_client::Config {
            namespace: Box::from(&b"test"[..]),
            writers: Box::new([skein_forge_client::Writer { forge: 1, author: 7 }]),
        },
        &[GitIdentity { forge: 1, credential: 1 }],
        &[],
        false,
    )
    .expect("valid connector")
}
fn outputs(d: &mut Domain, event: Event) -> Box<[Request]> {
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    step(d, &env(), event, &mut out);
    let mut values = skein_lib::List::with_capacity(out.len());
    for _ in 0..out.len() {
        values.push(out.pop().expect("counted output")).expect("sized from queue");
    }
    values.into_boxed()
}
fn ready(d: &mut Domain) -> Box<[Request]> {
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    resume(d, &env(), &mut out);
    let mut values = skein_lib::List::with_capacity(out.len());
    for _ in 0..out.len() {
        values.push(out.pop().expect("counted output")).expect("sized from queue");
    }
    values.into_boxed()
}
fn adopted() -> Repository {
    Repository {
        project: 5,
        home: true,
        provider: REPO,
        host: Box::from(&b"forge.example"[..]),
        owner: Box::from(&b"org"[..]),
        name: Box::from(&b"repo"[..]),
        prefix: Box::from(&b"temper/"[..]),
        role: Role::Owned,
        ci: true,
        checks: Box::new([]),
        kinds: Kinds {
            read: true,
            push: true,
            open: true,
            land: true,
            review: true,
            status: true,
            comment: true,
            issue: true,
            branch: true,
        },
        protection: Protection::Absent,
        settings: skein_forge::api::Settings {
            default_branch: Box::from(&b"main"[..]),
            merge: true,
            squash: true,
            rebase: false,
        },
    }
}
/// A commit status write: a set, made without a find.
fn status(number: u64, task: u64) -> skein_forge_client::Entry {
    skein_forge_client::Entry {
        number,
        task,
        repository: REPO,
        effect: skein_forge_client::Effect {
            write: skein_forge::api::Write::Status {
                commit: [3; 32],
                context: Box::from(&b"test"[..]),
                check: skein_forge::api::Check::Passed,
            },
            condition: skein_forge_client::Condition::None,
        },
        start: None,
        attempt: None,
        failures: 0,
        queued: false,
    }
}
fn branch() -> Name {
    Name { forge: 1, repository: 2, what: What::Branch(Box::new([Box::from(&b"temper"[..]), Box::from(&b"7"[..])])) }
}

#[test]
fn a_hold_and_writer_slot_are_exclusive_until_answered() {
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    let name = branch();
    let first = outputs(&mut d, Event::Hold { task: 7, resource: name.clone(), from: None });
    assert_eq!(first.len(), 1);
    assert_eq!(
        first[0],
        Request::Save {
            read_afresh: Some(name.clone()),
            release: None,
            record: Stored::Hold(Hold { name: name.clone(), task: 7, writer: None, drift: None })
        }
    );
    assert_eq!(
        outputs(&mut d, Event::Hold { task: 8, resource: name.clone(), from: None }).as_ref(),
        &[Request::Taken { task: 8, resource: name.clone(), by: 7 }]
    );
    outputs(&mut d, Event::Claim { task: 9, attempt: 1, writes: Box::new([name.clone()]), holders: Box::new([7]) });
    let reported = [Pushed { tag: tag(name.forge, name.repository), commit: [2; 32] }];
    assert_eq!(d.reported_heads(&LIMITS, 9, 1, &reported).as_ref(), &[(name.clone(), [2; 32])]);
    assert!(d.reported_heads(&LIMITS, 9, 2, &reported).is_empty(), "another attempt cannot report this writer");
    assert!(d.reported_heads(&LIMITS, 8, 1, &reported).is_empty(), "the holder cannot report its child's writer");
    assert!(d.reported_heads(&LIMITS, 9, 1, &[Pushed { tag: 99, commit: [2; 32] }]).is_empty());
    assert_eq!(
        outputs(
            &mut d,
            Event::Claim { task: 10, attempt: 1, writes: Box::new([name.clone()]), holders: Box::new([7]) }
        )
        .as_ref(),
        &[Request::Taken { task: 10, resource: name.clone(), by: 9 }]
    );
    outputs(&mut d, Event::Answered { task: 9, attempt: 1, pushed: Box::new([(name.clone(), [2; 32])]) });
    let claimed = outputs(
        &mut d,
        Event::Claim { task: 10, attempt: 1, writes: Box::new([name.clone()]), holders: Box::new([7]) },
    );
    assert_eq!(
        claimed.as_ref(),
        &[Request::Save {
            read_afresh: None,
            release: None,
            record: Stored::Hold(Hold {
                name,
                task: 7,
                writer: Some(Writer::Run { task: 10, attempt: 1 }),
                drift: None
            })
        }]
    );
}

#[test]
fn an_outbox_entry_waits_for_its_commit() {
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    assert_eq!(
        outputs(&mut d, Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic }).as_ref(),
        &[Request::RestartDone { stage: RestartStage::Restored }],
    );
    assert_eq!(
        outputs(&mut d, Event::ReadAfresh).as_ref(),
        &[Request::RestartDone { stage: RestartStage::ReadAfresh }],
    );
    assert_eq!(outputs(&mut d, Event::SettleOutbox).as_ref(), &[Request::RestartDone { stage: RestartStage::Settled }],);
    let entry = status(11, 7);
    assert_eq!(
        outputs(&mut d, Event::Enqueue { entry: entry.clone() }).as_ref(),
        &[Request::Save { read_afresh: None, release: Some(11), record: Stored::Entry { entry, held: false } }]
    );
    assert!(ready(&mut d).is_empty());
    outputs(&mut d, Event::Committed { entry: 11 });
    assert!(d.is_ready());
}

#[test]
#[expect(clippy::too_many_lines, reason = "one adoption test names every connector output explicitly")]
fn adoption_reads_permission_before_committing_its_role() {
    let mut d = domain();
    let request = Adoption {
        project: 5,
        home: true,
        forge: REPO.forge,
        host: Box::from(&b"forge.example"[..]),
        owner: Box::from(&b"org"[..]),
        name: Box::from(&b"repo"[..]),
        prefix: Box::from(&b"temper/"[..]),
        role: Role::Owned,
        landing: Box::from(&b"main"[..]),
        ci: true,
        checks: Box::new([]),
    };
    fixtures::begin_adoption(&mut d, Token::new(4), request);
    let sent = ready(&mut d);
    let (call, op) = match &sent[0] {
        Request::Call { call, op, .. } => (*call, op.clone()),
        Request::Resource { .. }
        | Request::Marked { .. }
        | Request::Adopted { .. }
        | Request::Save { .. }
        | Request::Erase { .. }
        | Request::Taken { .. }
        | Request::Refused { .. }
        | Request::Outcome { .. }
        | Request::ProjectionOutcome { .. }
        | Request::NotMade { .. }
        | Request::ContinueRelease { .. }
        | Request::Released { .. }
        | Request::Retained { .. }
        | Request::ReleaseFailed { .. }
        | Request::News { .. }
        | Request::Drift { .. }
        | Request::ProjectionEffect { .. }
        | Request::WakeProcedure { .. }
        | Request::ProjectAfter { .. }
        | Request::ProjectionFailed { .. }
        | Request::ProjectionSettled { .. }
        | Request::GoalTopic { .. }
        | Request::GoalUntopic { .. }
        | Request::ChangeDecision { .. }
        | Request::Read { .. }
        | Request::BriefClient { .. }
        | Request::BriefReady { .. }
        | Request::BriefSized { .. }
        | Request::EffectsSettled { .. }
        | Request::BriefTaken { .. }
        | Request::ApplicationPart { .. }
        | Request::ApplicationDone { .. }
        | Request::ApplicationAgain { .. }
        | Request::MarkedForges
        | Request::StopGroup { .. }
        | Request::SetUpGroup { .. }
        | Request::Connect { .. }
        | Request::Manifest { .. }
        | Request::Reachable { .. }
        | Request::Install { .. }
        | Request::Accept { .. }
        | Request::CarriedUnused { .. }
        | Request::EraseCarried { .. }
        | Request::SetUp { .. }
        | Request::Bound { .. }
        | Request::ListDemoActs
        | Request::SendDemoAct(_)
        | Request::DemoStart { .. }
        | Request::Settled { .. }
        | Request::RestartDone { .. } => panic!("permission call"),
    };
    assert_eq!(op, skein_forge::api::Op::Read(skein_forge::api::Read::Permission { user: 7 }));
    let result = outputs(
        &mut d,
        Event::Client(skein_forge_client::Event::Answered {
            free: 0,
            rate: None,
            call,
            cost: 1,
            result: Ok(skein_forge::api::Answer::Permission(skein_forge::api::Permission::Write)),
        }),
    );
    assert!(result.is_empty());
    let next = ready(&mut d);
    assert_eq!(next.len(), 1);
    let branches = match &next[0] {
        Request::Call { call, op: skein_forge::api::Op::Read(skein_forge::api::Read::Branches { prefix }), .. } => {
            assert_eq!(prefix.as_ref(), b"temper/");
            *call
        }
        Request::Call { .. }
        | Request::Marked { .. }
        | Request::Resource { .. }
        | Request::Adopted { .. }
        | Request::Save { .. }
        | Request::Erase { .. }
        | Request::Taken { .. }
        | Request::Refused { .. }
        | Request::Outcome { .. }
        | Request::ProjectionOutcome { .. }
        | Request::NotMade { .. }
        | Request::ContinueRelease { .. }
        | Request::Released { .. }
        | Request::Retained { .. }
        | Request::ReleaseFailed { .. }
        | Request::News { .. }
        | Request::Drift { .. }
        | Request::ProjectionEffect { .. }
        | Request::WakeProcedure { .. }
        | Request::ProjectAfter { .. }
        | Request::ProjectionFailed { .. }
        | Request::ProjectionSettled { .. }
        | Request::GoalTopic { .. }
        | Request::GoalUntopic { .. }
        | Request::ChangeDecision { .. }
        | Request::Read { .. }
        | Request::BriefClient { .. }
        | Request::BriefReady { .. }
        | Request::BriefSized { .. }
        | Request::EffectsSettled { .. }
        | Request::BriefTaken { .. }
        | Request::ApplicationPart { .. }
        | Request::ApplicationDone { .. }
        | Request::ApplicationAgain { .. }
        | Request::MarkedForges
        | Request::StopGroup { .. }
        | Request::SetUpGroup { .. }
        | Request::Connect { .. }
        | Request::Manifest { .. }
        | Request::Reachable { .. }
        | Request::Install { .. }
        | Request::Accept { .. }
        | Request::CarriedUnused { .. }
        | Request::EraseCarried { .. }
        | Request::SetUp { .. }
        | Request::Bound { .. }
        | Request::ListDemoActs
        | Request::SendDemoAct(_)
        | Request::DemoStart { .. }
        | Request::Settled { .. }
        | Request::RestartDone { .. } => panic!("branches call"),
    };
    assert!(
        outputs(
            &mut d,
            Event::Client(skein_forge_client::Event::Answered {
                free: 0,
                rate: None,
                call: branches,
                cost: 1,
                result: Ok(skein_forge::api::Answer::Branches(Box::new([]))),
            })
        )
        .is_empty()
    );
    let next = ready(&mut d);
    match &next[0] {
        Request::Call { op: skein_forge::api::Op::Read(skein_forge::api::Read::Settings), .. } => {}
        Request::Call { .. }
        | Request::Marked { .. }
        | Request::Resource { .. }
        | Request::Adopted { .. }
        | Request::Save { .. }
        | Request::Erase { .. }
        | Request::Taken { .. }
        | Request::Refused { .. }
        | Request::Outcome { .. }
        | Request::ProjectionOutcome { .. }
        | Request::NotMade { .. }
        | Request::ContinueRelease { .. }
        | Request::Released { .. }
        | Request::Retained { .. }
        | Request::ReleaseFailed { .. }
        | Request::News { .. }
        | Request::Drift { .. }
        | Request::ProjectionEffect { .. }
        | Request::WakeProcedure { .. }
        | Request::ProjectAfter { .. }
        | Request::ProjectionFailed { .. }
        | Request::ProjectionSettled { .. }
        | Request::GoalTopic { .. }
        | Request::GoalUntopic { .. }
        | Request::ChangeDecision { .. }
        | Request::Read { .. }
        | Request::BriefClient { .. }
        | Request::BriefReady { .. }
        | Request::BriefSized { .. }
        | Request::EffectsSettled { .. }
        | Request::BriefTaken { .. }
        | Request::ApplicationPart { .. }
        | Request::ApplicationDone { .. }
        | Request::ApplicationAgain { .. }
        | Request::MarkedForges
        | Request::StopGroup { .. }
        | Request::SetUpGroup { .. }
        | Request::Connect { .. }
        | Request::Manifest { .. }
        | Request::Reachable { .. }
        | Request::Install { .. }
        | Request::Accept { .. }
        | Request::CarriedUnused { .. }
        | Request::EraseCarried { .. }
        | Request::SetUp { .. }
        | Request::Bound { .. }
        | Request::ListDemoActs
        | Request::SendDemoAct(_)
        | Request::DemoStart { .. }
        | Request::Settled { .. }
        | Request::RestartDone { .. } => panic!("settings follows collision read"),
    }
}

#[test]
fn adoption_without_ci_refuses_an_empty_check_policy() {
    let mut d = domain();
    let request = Adoption {
        project: 5,
        home: true,
        forge: REPO.forge,
        host: Box::from(&b"forge.example"[..]),
        owner: Box::from(&b"org"[..]),
        name: Box::from(&b"repo"[..]),
        prefix: Box::from(&b"temper/"[..]),
        role: Role::Owned,
        landing: Box::from(&b"main"[..]),
        ci: false,
        checks: Box::new([]),
    };
    assert_eq!(
        fixtures::begin_adoption(&mut d, Token::new(5), request).as_ref(),
        &[Request::Adopted { reply_to: Token::new(5), result: Err(skein_forge::api::Error::Refused) }]
    );
    assert!(ready(&mut d).is_empty());
}

#[test]
fn restored_ci_subscriptions_are_read_in_the_fresh_step_and_delay_its_completion() {
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    outputs(
        &mut d,
        Event::Restore {
            record: Stored::Subscription(Subscriber {
                task: 7,
                number: 1,
                topic: Topic::Ci { repository: REPO, head: [4; 32] },
                own_change: None,
                goal_tasks: None,
                paths: Box::new([]),
            }),
        },
    );
    assert_eq!(
        outputs(&mut d, Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic }).as_ref(),
        &[Request::RestartDone { stage: RestartStage::Restored }]
    );
    let fresh = outputs(&mut d, Event::ReadAfresh);
    assert!(fresh.is_empty(), "the subscription read is scheduled before completion");
    let calls = ready(&mut d);
    let Request::Call { call, op: skein_forge::api::Op::Read(skein_forge::api::Read::Checks { commit }), .. } =
        &calls[0]
    else {
        panic!("CI checks requested in fresh step")
    };
    assert_eq!(*commit, [4; 32]);
    let done = outputs(
        &mut d,
        Event::Client(skein_forge_client::Event::Answered {
            free: 0,
            rate: None,
            call: *call,
            cost: 1,
            result: Ok(fixtures::checks(skein_forge::api::Check::Passed)),
        }),
    );
    assert!(done.contains(&Request::RestartDone { stage: RestartStage::ReadAfresh }));
}

#[test]
fn workspace_items_are_sized_context_is_read_only_and_saved_work_starts_the_next_run() {
    let mut d = domain();
    let mut repository = adopted();
    repository.role = Role::Context;
    outputs(&mut d, Event::Restore { record: Stored::Repository(repository) });
    let item = d
        .workspace_item(items::Seed {
            task: 7,
            parent: None,
            repository: REPO,
            run_branch: Box::from(&b"temper/7/r7"[..]),
            saved_branch: Some(Box::from(&b"temper/7/s7"[..])),
        })
        .expect("item");
    assert_eq!(item.start, items::Start::Saved(Box::from(&b"temper/7/s7"[..])));
    assert!(item.push.is_none());
    assert!(item.bytes > 0);
    assert_eq!(d.context_repositories(5, 1).expect("bounded context").len(), 1);
    assert!(d.context_repositories(5, 0).is_none());
}

// Each repository's git identity is the configured credential of its forge
// (protocol/credentials.md; brief D10).
#[test]
fn a_workspace_item_names_its_forges_git_identity() {
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    let item = d
        .workspace_item(items::Seed {
            task: 7,
            parent: None,
            repository: REPO,
            run_branch: Box::from(&b"temper/7/r7"[..]),
            saved_branch: None,
        })
        .expect("item");
    assert_eq!(item.credential, Some(1));
}

#[test]
fn named_resource_reports_distinguish_private_branches_context_and_shared_landings() {
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    outputs(&mut d, Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic });
    let private = branch();
    assert_eq!(d.resource_facts(&private), Some((Access::Owned, resources::HoldKind::Exclusive { wait: true })));
    let shared = Name { what: What::Branch(Box::new([Box::from(&b"main"[..])])), ..private.clone() };
    assert_eq!(d.resource_facts(&shared), Some((Access::Participant, resources::HoldKind::Shared)));
    let report = outputs(&mut d, Event::Names { task: 7, resources: Box::new([private.clone()]) });
    assert!(report.contains(&Request::Resource {
        project: 5,
        name: private.clone(),
        role: Access::Owned,
        hold: resources::HoldKind::Exclusive { wait: true }
    }));
    let mut context = adopted();
    context.role = Role::Context;
    outputs(&mut d, Event::Restore { record: Stored::Repository(context) });
    assert_eq!(d.resource_facts(&private), Some((Access::Context, resources::HoldKind::Shared)));
    assert_eq!(
        outputs(&mut d, Event::Hold { task: 7, resource: private, from: None }).as_ref(),
        &[Request::Refused { task: 7 }]
    );
}

#[test]
fn native_effect_rows_keep_the_clients_recovery_promises() {
    use skein_forge::api::Write;
    let empty = || Box::<[u8]>::default();
    let writes = [
        (3, EffectForm::Creation, Write::OpenPull { title: empty(), body: empty(), head: empty(), base: empty() }),
        (10, EffectForm::Set, Write::Edit { number: 1, title: None, body: None }),
        (11, EffectForm::Transition, Write::Update { number: 1 }),
        (4, EffectForm::Transition, Write::Merge { number: 1, head: [1; 32] }),
        (12, EffectForm::Set, Write::Close { number: 1 }),
        (12, EffectForm::Set, Write::Reopen { number: 1 }),
        (
            5,
            EffectForm::Creation,
            Write::Review { number: 1, key: empty(), verdict: skein_forge::api::Verdict::Approve, body: empty() },
        ),
        (7, EffectForm::Creation, Write::Post { number: 1, key: empty(), body: empty() }),
        (
            6,
            EffectForm::Set,
            Write::Status { commit: [1; 32], context: empty(), check: skein_forge::api::Check::Passed },
        ),
        (8, EffectForm::Creation, Write::CreateIssue { key: empty(), title: empty(), body: empty() }),
        (13, EffectForm::Set, Write::Edit { number: 1, title: None, body: None }),
        (13, EffectForm::Set, Write::Close { number: 1 }),
        (13, EffectForm::Set, Write::Reopen { number: 1 }),
        (14, EffectForm::Set, Write::SetReviewers { number: 1, reviewers: Box::new([]) }),
        (9, EffectForm::Creation, Write::CreateBranch { branch: empty(), commit: [1; 32] }),
        (15, EffectForm::Set, Write::DeleteBranch { branch: empty() }),
    ];
    for (kind, expected_form, write) in writes {
        let (form, recovery) = effect_shape(kind).expect("native effect kind");
        assert_eq!(form, expected_form, "kind {kind}");
        let expected = match skein_forge_client::recovery(&write) {
            skein_forge_client::Recovery::Keyed => Recovery::Keyed,
            skein_forge_client::Recovery::Conditional => Recovery::Conditional,
            skein_forge_client::Recovery::Idempotent => Recovery::Idempotent,
            skein_forge_client::Recovery::Unrecoverable => Recovery::Unrecoverable,
        };
        assert_eq!(recovery, expected, "kind {kind}");
    }
}

/// An issue creation whose lifetime ran out unanswered: unrecoverable, it
/// is held for a person (domain/forge.md, 6).
fn held_issue() -> skein_forge_client::Entry {
    skein_forge_client::Entry {
        queued: false,
        number: 11,
        task: 7,
        repository: REPO,
        effect: skein_forge_client::Effect {
            write: skein_forge::api::Write::CreateIssue {
                key: Box::from(&b"k"[..]),
                title: Box::from(&b"Finding"[..]),
                body: Box::from(&b"Fix"[..]),
            },
            condition: skein_forge_client::Condition::None,
        },
        start: Some(skein_forge_client::Position { at: Time::from_nanos(1), comment: 0, review: 0, review_page: 1 }),
        attempt: Some(skein_forge_client::Attempt {
            sent: Time::from_nanos(1),
            deadline: Time::from_nanos(2),
            wall: Wall::from_nanos(1),
            expires: Wall::from_nanos(2),
        }),
        failures: 1,
    }
}

/// A forge restored with the held issue creation, before its restart runs.
fn held_restored() -> Domain {
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    outputs(&mut d, Event::Restore { record: Stored::Entry { entry: held_issue(), held: true } });
    d
}

/// A restarted forge whose held issue creation waits for a person.
fn held_creation() -> Domain {
    let mut d = held_restored();
    outputs(&mut d, Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic });
    outputs(&mut d, Event::ReadAfresh);
    outputs(&mut d, Event::SettleOutbox);
    d
}

#[test]
fn a_held_effect_survives_a_restart_out_of_the_clients_outbox() {
    let mut d = held_restored();
    assert_eq!(
        outputs(&mut d, Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic }).as_ref(),
        &[Request::RestartDone { stage: RestartStage::Restored }]
    );
    assert_eq!(
        outputs(&mut d, Event::ReadAfresh).as_ref(),
        &[Request::RestartDone { stage: RestartStage::ReadAfresh }]
    );
    // Nothing is looked for: the client never had the held entry.
    assert_eq!(outputs(&mut d, Event::SettleOutbox).as_ref(), &[Request::RestartDone { stage: RestartStage::Settled }]);
    assert!(ready(&mut d).is_empty());
    assert!(!d.effect_room(&LIMITS, 7), "the held entry keeps its room");
}

#[test]
fn a_held_effect_settled_as_not_made_is_erased_and_reported() {
    let mut d = held_creation();
    assert_eq!(
        outputs(&mut d, Event::Resolve { entry: 11, again: false }).as_ref(),
        &[Request::Erase { key: Key::Entry(11) }, Request::NotMade { entry: 11, task: 7 }]
    );
}

#[test]
fn a_held_effect_asked_again_is_looked_for_first_even_with_an_otherwise_full_outbox() {
    let mut d = held_creation();
    for number in 12..19 {
        outputs(&mut d, Event::Enqueue { entry: status(number, 8) });
        outputs(&mut d, Event::Committed { entry: number });
    }
    assert!(!d.effect_room(&LIMITS, 0), "the outbox is full");
    assert_eq!(
        outputs(&mut d, Event::Resolve { entry: 11, again: true }).as_ref(),
        &[Request::Save {
            read_afresh: None,
            release: None,
            record: Stored::Entry { entry: held_issue(), held: false }
        }]
    );
    assert_eq!(
        ready(&mut d).as_ref(),
        &[Request::Call {
            call: Token::new(1),
            repository: REPO,
            op: skein_forge::api::Op::Read(skein_forge::api::Read::Items {
                since: Time::from_nanos(1),
                page: 1,
                kind: Some(skein_forge::api::Kind::Issue),
            }),
        }]
    );
}

fn settling_release(comment: Option<Box<[u8]>>) -> ReleaseRow {
    ReleaseRow {
        task: 7,
        root: 7,
        ending: ReleaseEnding::Done,
        close_pull: None,
        comment,
        pending: None,
        failed: false,
        effects_settled: false,
        releasing: false,
    }
}

#[test]
fn a_release_payload_counts_its_comment_and_pending_branch() {
    const COMMENT_BYTES: u64 = 3; // "why".
    // "why" plus two 16-byte segment boxes and "temper" + "7".
    const COMMENT_AND_BRANCH_BYTES: u64 = 42;
    let mut row = settling_release(Some(Box::from(b"why".as_slice())));
    assert_eq!(stored_bytes(&Stored::Release(row.clone())), Some(COMMENT_BYTES));
    row.pending = Some((20, Some(run_branch(b"7"))));
    assert_eq!(stored_bytes(&Stored::Release(row)), Some(COMMENT_AND_BRANCH_BYTES));
}

#[test]
fn closing_a_task_settles_its_held_effect_not_made_and_frees_its_room() {
    let mut d = held_creation();
    for number in 12..19 {
        outputs(&mut d, Event::Enqueue { entry: status(number, 8) });
    }
    let row = settling_release(None);
    assert_eq!(
        outputs(&mut d, Event::SettleEffects { task: 7, root: 7, ending: ReleaseEnding::Done }).as_ref(),
        &[
            Request::Save { read_afresh: None, release: None, record: Stored::Release(row.clone()) },
            Request::Erase { key: Key::Entry(11) },
            Request::NotMade { entry: 11, task: 7 },
            Request::ContinueRelease { task: 7 },
        ]
    );
    assert_eq!(
        outputs(&mut d, Event::Enqueue { entry: status(19, 8) }).as_ref(),
        &[Request::Save {
            read_afresh: None,
            release: Some(19),
            record: Stored::Entry { entry: status(19, 8), held: false }
        }]
    );
    let settled = ReleaseRow { effects_settled: true, ..row };
    assert_eq!(
        outputs(&mut d, Event::ContinueRelease { task: 7, entry: 20 }).as_ref(),
        &[
            Request::Save { read_afresh: None, release: None, record: Stored::Release(settled) },
            Request::EffectsSettled { task: 7 },
        ]
    );
}

fn run_branch(task: &[u8]) -> Name {
    Name { forge: 1, repository: 2, what: What::Branch(Box::new([Box::from(&b"temper"[..]), Box::from(task)])) }
}

/// A ready forge whose run 9 writes four branches the forge last saw at
/// commit 2, each held by its own task: their reads fill the client's table.
fn four_writers() -> Domain {
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    for (holder, task) in [(7, b"7".as_slice()), (8, b"8"), (10, b"10"), (11, b"11")] {
        let name = run_branch(task);
        let head = BranchHead { name: name.clone(), commit: [2; 32], owned: true };
        outputs(&mut d, Event::Restore { record: Stored::BranchHead(head) });
        outputs(&mut d, Event::Hold { task: holder, resource: name.clone(), from: None });
        outputs(&mut d, Event::Claim { task: 9, attempt: 1, writes: Box::new([name]), holders: Box::new([holder]) });
    }
    outputs(&mut d, Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic });
    outputs(&mut d, Event::ReadAfresh);
    outputs(&mut d, Event::SettleOutbox);
    d
}

/// The forge's answer to client call `call`: the branch is at `commit`.
fn branch_at(d: &mut Domain, call: u64, commit: u8) -> Box<[Request]> {
    let result = Ok(skein_forge::api::Answer::Commit([commit; 32]));
    let answered = outputs(
        d,
        Event::Client(skein_forge_client::Event::Answered {
            free: 0,
            rate: None,
            call: Token::new(call),
            cost: 1,
            result,
        }),
    );
    d.reclaim();
    answered
}

// A lost writer's slot is freed only once its branch is read afresh
// (jig's domain/tasks.md, 6.3): a read refused for want of room in the
// client waits, and is asked again once room frees, rather than given up.
#[test]
fn a_lost_writers_read_refused_for_room_is_asked_again_when_room_frees() {
    let mut d = four_writers();
    outputs(&mut d, Event::Lost { task: 9, attempt: 1 });
    ready(&mut d);
    ready(&mut d);
    // The first branch moved; its comparison finds the client's table full.
    assert!(branch_at(&mut d, 1, 4).is_empty(), "nothing refused");
    ready(&mut d);
    ready(&mut d);
    branch_at(&mut d, 2, 2);
    ready(&mut d);
    branch_at(&mut d, 3, 2);
    assert_eq!(
        ready(&mut d).as_ref(),
        &[Request::Call {
            call: Token::new(5),
            repository: REPO,
            op: skein_forge::api::Op::Read(skein_forge::api::Read::Compare { before: [2; 32], after: [4; 32] }),
        }]
    );
}

#[test]
fn a_release_waits_for_its_lost_writers_reread_even_after_restart() {
    for restart in [false, true] {
        for refused in [false, true] {
            let mut d = domain();
            outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
            outputs(&mut d, Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic });
            outputs(&mut d, Event::ReadAfresh);
            outputs(&mut d, Event::SettleOutbox);
            let name = run_branch(b"7");
            outputs(&mut d, Event::Hold { task: 7, resource: name.clone(), from: None });
            outputs(
                &mut d,
                Event::Claim { task: 9, attempt: 1, writes: Box::new([name.clone()]), holders: Box::new([7]) },
            );
            outputs(&mut d, Event::Lost { task: 9, attempt: 1 });
            let (mut call, _) = fixtures::next_call(&mut d);
            assert_eq!(d.hold(&name).unwrap().writer, Some(Writer::Lost { task: 9, attempt: 1 }));
            let closing = outputs(
                &mut d,
                Event::Release { task: 7, root: 7, ending: ReleaseEnding::Done, reason: None, entry: 20 },
            );
            assert!(!closing.contains(&Request::ReleaseFailed { task: 7 }), "wait for the reread");
            if restart {
                let hold = d.hold(&name).unwrap().clone();
                d = domain();
                outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
                outputs(&mut d, Event::Restore { record: Stored::Hold(hold) });
                for request in closing {
                    if let Request::Save { record: row @ Stored::Release(_), .. } = request {
                        outputs(&mut d, Event::Restore { record: row });
                    }
                }
                outputs(&mut d, Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic });
                outputs(&mut d, Event::ReadAfresh);
                (call, _) = fixtures::next_call(&mut d);
            }
            let error = if refused { skein_forge::api::Error::Refused } else { skein_forge::api::Error::Missing };
            let answered = fixtures::answer(&mut d, call, Err(error));
            if refused {
                assert!(answered.contains(&Request::ReleaseFailed { task: 7 }));
                assert!(!answered.contains(&Request::ContinueRelease { task: 7 }));
            } else {
                assert!(!answered.contains(&Request::ReleaseFailed { task: 7 }));
                assert!(answered.contains(&Request::ContinueRelease { task: 7 }));
                assert_eq!(d.hold(&name).unwrap().writer, None);
                let continued = outputs(&mut d, Event::ContinueRelease { task: 7, entry: 21 });
                let mut deleting = false;
                for request in continued {
                    if let Request::Save { record: Stored::Entry { entry, .. }, .. } = request
                        && let skein_forge::api::Write::DeleteBranch { branch } = entry.effect.write
                    {
                        assert_eq!(entry.task, 7);
                        assert_eq!(branch.as_ref(), b"temper/7");
                        deleting = true;
                    }
                }
                assert!(deleting, "release enqueues its branch deletion");
            }
        }
    }
}

/// Restart rebuilds the fresh read from the durable lost writer alone.
fn restarted_lost_writer() -> Domain {
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    outputs(
        &mut d,
        Event::Restore {
            record: Stored::Hold(Hold {
                name: run_branch(b"7"),
                task: 7,
                writer: Some(Writer::Lost { task: 9, attempt: 1 }),
                drift: None,
            }),
        },
    );
    outputs(&mut d, Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic });
    outputs(&mut d, Event::ReadAfresh);
    outputs(&mut d, Event::SettleOutbox);
    d
}

#[test]
fn a_lost_writers_read_answered_not_connected_is_asked_again_when_its_forge_connects() {
    for result in [
        Err(skein_forge::api::Error::Missing),
        Ok(skein_forge::api::Answer::Commit([2; 32])),
        Err(skein_forge::api::Error::Refused),
    ] {
        let mut d = restarted_lost_writer();
        let waiting = fixtures::answer_next(&mut d, Err(skein_forge::api::Error::NotConnected));
        assert!(!waiting.contains(&Request::Refused { task: 9 }));
        assert_eq!(d.hold(&run_branch(b"7")).unwrap().writer, Some(Writer::Lost { task: 9, attempt: 1 }));
        assert!(!d.is_ready(), "a connection wait has no ready work");
        assert!(ready(&mut d).is_empty(), "nothing asked while disconnected");
        outputs(
            &mut d,
            Event::Connection { forge: 1, state: Connection::Connected { id: 7, login: Box::from(b"bot".as_slice()) } },
        );
        assert!(d.is_ready());
        assert!(ready(&mut d).is_empty(), "resume first admits the waiting read");
        let (call, op) = fixtures::next_call(&mut d);
        assert_eq!(
            op,
            skein_forge::api::Op::Read(skein_forge::api::Read::Branch { branch: Box::from(b"temper/7".as_slice()) })
        );
        assert_eq!(d.hold(&run_branch(b"7")).unwrap().writer, Some(Writer::Lost { task: 9, attempt: 1 }));
        let refused = result == Err(skein_forge::api::Error::Refused);
        let answered = fixtures::answer(&mut d, call, result);
        assert_eq!(answered.contains(&Request::Refused { task: 9 }), refused);
        assert_eq!(d.hold(&run_branch(b"7")).unwrap().writer.is_some(), refused);
    }
}

#[test]
fn a_release_waits_for_its_lost_writers_read_until_its_forge_connects() {
    let mut d = restarted_lost_writer();
    fixtures::answer_next(&mut d, Err(skein_forge::api::Error::NotConnected));
    let closing =
        outputs(&mut d, Event::Release { task: 7, root: 7, ending: ReleaseEnding::Cancelled, reason: None, entry: 20 });
    assert!(!closing.contains(&Request::ReleaseFailed { task: 7 }));
    assert!(!closing.contains(&Request::Released { task: 7 }));
    assert!(outputs(&mut d, Event::ContinueRelease { task: 7, entry: 20 }).is_empty());
    assert!(!d.is_ready());
    outputs(
        &mut d,
        Event::Connection { forge: 1, state: Connection::Connected { id: 7, login: Box::from(b"bot".as_slice()) } },
    );
    assert!(ready(&mut d).is_empty(), "resume first admits the waiting read");
    let answered = fixtures::answer_next(&mut d, Err(skein_forge::api::Error::Missing));
    assert!(!answered.contains(&Request::ReleaseFailed { task: 7 }));
    assert!(answered.contains(&Request::ContinueRelease { task: 7 }));
    let deleting = outputs(&mut d, Event::ContinueRelease { task: 7, entry: 20 });
    let mut enqueued = false;
    for request in &deleting {
        if let Request::Save { record: Stored::Entry { entry, .. }, .. } = request {
            assert_eq!(entry.task, 7);
            assert_eq!(
                entry.effect.write,
                skein_forge::api::Write::DeleteBranch { branch: Box::from(b"temper/7".as_slice()) }
            );
            enqueued = true;
        }
    }
    assert!(enqueued, "release enqueues its branch deletion");
    outputs(&mut d, Event::Committed { entry: 20 });
    // The outbox looks for this cleanup effect before making it.
    fixtures::answer_next(
        &mut d,
        Ok(skein_forge::api::Answer::Items { items: Box::new([]), more: false, now: env().now }),
    );
    let (call, op) = fixtures::next_call(&mut d);
    assert_eq!(
        op,
        skein_forge::api::Op::Write(skein_forge::api::Write::DeleteBranch {
            branch: Box::from(b"temper/7".as_slice())
        })
    );
    let deleted = fixtures::answer(&mut d, call, Ok(skein_forge::api::Answer::Done));
    assert!(deleted.contains(&Request::ContinueRelease { task: 7 }));
    assert!(outputs(&mut d, Event::ContinueRelease { task: 7, entry: 21 }).contains(&Request::Released { task: 7 }));
    assert!(d.hold(&run_branch(b"7")).is_none());
}

#[test]
fn another_forges_connection_does_not_ask_a_lost_writers_read_again() {
    let mut d = restarted_lost_writer();
    fixtures::answer_next(&mut d, Err(skein_forge::api::Error::NotConnected));
    outputs(
        &mut d,
        Event::Connection { forge: 2, state: Connection::Connected { id: 8, login: Box::from(b"other".as_slice()) } },
    );
    assert!(!d.is_ready());
    assert!(ready(&mut d).is_empty());
    outputs(&mut d, Event::Connection { forge: 1, state: Connection::Connecting });
    assert!(!d.is_ready());
    assert!(ready(&mut d).is_empty());
    outputs(
        &mut d,
        Event::Connection { forge: 1, state: Connection::Connected { id: 7, login: Box::from(b"bot".as_slice()) } },
    );
    assert!(d.is_ready(), "its own forge's connection makes the read ready");
    assert!(ready(&mut d).is_empty(), "resume first admits the waiting read");
    let (_, op) = fixtures::next_call(&mut d);
    assert_eq!(
        op,
        skein_forge::api::Op::Read(skein_forge::api::Read::Branch { branch: Box::from(b"temper/7".as_slice()) })
    );
}

#[test]
fn a_release_waits_while_its_lost_writer_is_waiting_for_read_room() {
    let mut d = four_writers();
    outputs(&mut d, Event::Lost { task: 9, attempt: 1 });
    ready(&mut d);
    ready(&mut d);
    // Branch temper/10 sorts first; its comparison waits for call room.
    branch_at(&mut d, 1, 4);
    let closing =
        outputs(&mut d, Event::Release { task: 10, root: 10, ending: ReleaseEnding::Done, reason: None, entry: 20 });
    assert!(!closing.contains(&Request::ReleaseFailed { task: 10 }));
    ready(&mut d);
    ready(&mut d);
    branch_at(&mut d, 2, 2);
    ready(&mut d);
    branch_at(&mut d, 3, 2);
    let (call, _) = fixtures::next_call(&mut d);
    let answered = fixtures::answer(
        &mut d,
        call,
        Ok(skein_forge::api::Answer::Compare {
            before: [2; 32],
            after: [4; 32],
            contains_before: true,
            files: Box::new([]),
            commits: Box::new([]),
        }),
    );
    assert!(answered.contains(&Request::ContinueRelease { task: 10 }));
}

#[test]
fn a_release_still_fails_for_a_running_writer_or_a_lost_writer_without_a_read() {
    for writer in [Writer::Run { task: 9, attempt: 1 }, Writer::Lost { task: 9, attempt: 1 }] {
        let mut d = domain();
        outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
        outputs(
            &mut d,
            Event::Restore {
                record: Stored::Hold(Hold { name: run_branch(b"7"), task: 7, writer: Some(writer), drift: None }),
            },
        );
        let closing =
            outputs(&mut d, Event::Release { task: 7, root: 7, ending: ReleaseEnding::Done, reason: None, entry: 20 });
        assert!(closing.contains(&Request::ReleaseFailed { task: 7 }));
    }
}

#[test]
fn bounded_words_keep_the_requested_edge() {
    for (tail, gathered, cut) in [(true, b"cdefghij", b"ghij"), (false, b"abcdefgh", b"abcd")] {
        let mut words = brief::BriefWords::with_capacity(8, tail);
        brief::append(&mut words, b"abcdefghij");
        assert_eq!(
            brief::brief_part(&words, 1, 8),
            BriefRead::Got { bytes: Box::from(&gathered[..]), left: 2 },
            "Tail: {tail}",
        );
        assert_eq!(
            brief::brief_part(&words, 1, 4),
            BriefRead::Got { bytes: Box::from(&cut[..]), left: 6 },
            "Tail: {tail}",
        );
    }
}

fn routing_domain() -> Domain {
    Domain::new(
        &LIMITS,
        4,
        skein_forge_client::Config { namespace: Box::from(b"test".as_slice()), writers: Box::new([]) },
        &[GitIdentity { forge: 1, credential: 1 }],
        &[],
        false,
    )
    .unwrap()
}

#[test]
fn routing_connection_states_are_volatile_and_connected_gives_the_writer_once() {
    let mut d = routing_domain();
    assert_eq!(d.connection(1), Connection::NotConnected);
    for state in [Connection::Connecting, Connection::Stopped { reason: Box::from(b"key refused".as_slice()) }] {
        assert_eq!(
            outputs(&mut d, Event::Connection { forge: 1, state: state.clone() }).as_ref(),
            [Request::MarkedForges]
        );
        assert_eq!(d.connection(1), state);
    }
    let state = Connection::Connected { id: 7, login: Box::from(b"writer".as_slice()) };
    let first = outputs(&mut d, Event::Connection { forge: 1, state: state.clone() });
    assert_eq!(
        first.as_ref(),
        [
            Request::Save {
                record: Stored::Writer(skein_forge_client::Writer { forge: 1, author: 7 }),
                release: None,
                read_afresh: None
            },
            Request::MarkedForges
        ]
    );
    assert_eq!(d.connection(1), state);
    assert_eq!(
        outputs(&mut d, Event::Connection { forge: 1, state }).as_ref(),
        [Request::MarkedForges],
        "writer given twice saves once"
    );
    let mut restarted = domain();
    outputs(
        &mut restarted,
        Event::Restore { record: Stored::Writer(skein_forge_client::Writer { forge: 1, author: 7 }) },
    );
    assert_eq!(restarted.connection(1), Connection::NotConnected, "restored writer is no connection claim");
}
fn configured() -> [Configured; 2] {
    [
        Configured {
            admin_connect: false,
            bot: Box::new([]),
            owned: None,
            git: Box::new([]),
            web: None,
            read_only: false,
            number: 1,
            kind: Kind::Forgejo,
            mode: Mode::Owned,
            asked: true,
        },
        Configured {
            admin_connect: false,
            bot: Box::new([]),
            owned: None,
            git: Box::new([]),
            web: None,
            read_only: false,
            number: 2,
            kind: Kind::GitHub,
            mode: Mode::Owned,
            asked: true,
        },
    ]
}
fn setup_domain(forges: &[Configured], under_box: bool) -> Domain {
    Domain::new(
        &LIMITS,
        4,
        skein_forge_client::Config {
            namespace: Box::from(b"test".as_slice()),
            writers: Box::new([skein_forge_client::Writer { forge: 1, author: 7 }]),
        },
        &[],
        forges,
        under_box,
    )
    .unwrap()
}
#[test]
fn a_configured_git_base_is_bounded_by_its_priced_pull_address_bytes() {
    let base = [b'g'; 2049];
    for length in [2048, 2049] {
        let mut forge = configured()[0].clone();
        forge.git = base.get(..length).unwrap().into();
        let result = Domain::new(
            &LIMITS,
            4,
            skein_forge_client::Config { namespace: b"test".as_slice().into(), writers: Box::new([]) },
            &[],
            &[forge],
            false,
        );
        if length == 2048 {
            assert!(result.is_ok(), "the priced base is admitted");
        } else {
            assert_eq!(
                result.err(),
                Some(skein_forge::api::Error::TooLarge),
                "a larger base is refused before retaining it"
            );
        }
    }
}

fn setup_row(number: u16, kind: Kind, connection: Connection, offer: Offer) -> Row {
    Row {
        number,
        kind,
        mode: Mode::Owned,
        connection,
        failed: None,
        offer,
        first: None,
        installations: Box::new([]),
        ended: None,
        app: None,
        web: None,
        repositories: Box::new([]),
        owned: None,
        admin: false,
        read_only: false,
        ended_stop: None,
    }
}
#[test]
fn setup_rows_and_answers_agree_in_every_connection_state() {
    let not_offered = (Offer::NotOffered, Some(SetUpRefusal::NotOffered));
    let offered = (Offer::Offered, None);
    let busy = (Offer::Offered, Some(SetUpRefusal::Busy));
    let no_box = (Offer::NotUnderTheBox, Some(SetUpRefusal::NotUnderTheBox));
    let rows = [
        (Connection::NotConnected, [not_offered, not_offered]),
        (Connection::NotSetUp, [offered, no_box]),
        (Connection::SetUpFailed { why: b"failed".as_slice().into() }, [offered, no_box]),
        (Connection::SettingUp { step: None }, [busy, no_box]),
        (Connection::SettingUp { step: Some(SetUpStep::Fetch) }, [busy, no_box]),
        (Connection::SettingUp { step: Some(SetUpStep::Prepare) }, [busy, no_box]),
        (Connection::SettingUp { step: Some(SetUpStep::Run) }, [busy, no_box]),
        (Connection::Connecting, [not_offered, not_offered]),
        (Connection::Connected { id: 7, login: b"writer".as_slice().into() }, [not_offered, not_offered]),
        (Connection::Stopped { reason: b"stopped".as_slice().into() }, [not_offered, not_offered]),
    ];
    for (state, expected) in rows {
        for (under_box, (offer, refusal)) in [true, false].into_iter().zip(expected) {
            let mut d = setup_domain(&configured()[..1], under_box);
            outputs(&mut d, Event::Connection { forge: 1, state: state.clone() });
            assert_eq!(d.forges().rows.as_ref(), [setup_row(1, Kind::Forgejo, state.clone(), offer)]);
            let request = Token::new(91);
            let token = Token::new(92);
            let emitted = outputs(&mut d, setup_application(1, request, token));
            match refusal {
                None => {
                    assert_eq!(
                        emitted.as_ref(),
                        [
                            Request::MarkedForges,
                            Request::SetUp {
                                playground: false,
                                forge: 1,
                                request,
                                org: b"work".as_slice().into(),
                                repository: b"hello".as_slice().into()
                            }
                        ]
                    );
                    outputs(&mut d, Event::SetUpAnswered { forge: 1, request, answer: SetUpAnswer::SettingUp });
                    assert_eq!(d.application_answer(token), Some(Answered::SetUp(SetUpAnswer::SettingUp)));
                }
                Some(why) => {
                    assert_eq!(emitted.as_ref(), [Request::ApplicationDone { request, token }]);
                    assert_eq!(d.application_answer(token), Some(Answered::SetUp(SetUpAnswer::Refused(why))));
                }
            }
        }
    }
}
#[test]
fn setup_is_not_offered_without_owned_asked_configuration() {
    let rows: [Box<[Configured]>; 5] = [
        Box::new([Configured { mode: Mode::Connected, ..configured()[0].clone() }]),
        Box::new([Configured { mode: Mode::Managed, ..configured()[0].clone() }]),
        Box::new([Configured { mode: Mode::Fake, ..configured()[0].clone() }]),
        Box::new([Configured { asked: false, ..configured()[0].clone() }]),
        Box::new([]),
    ];
    for forges in rows {
        let mut d = setup_domain(&forges, true);
        outputs(&mut d, Event::Connection { forge: 1, state: Connection::NotSetUp });
        let token = Token::new(91);
        assert_eq!(d.offer(1), Offer::NotOffered);
        assert_eq!(
            outputs(&mut d, setup_application(1, token, token)).as_ref(),
            [Request::ApplicationDone { request: token, token }]
        );
        assert_eq!(d.application_answer(token), Some(Answered::SetUp(SetUpAnswer::Refused(SetUpRefusal::NotOffered))));
    }
}
#[test]
fn a_setup_on_another_forge_keeps_the_offer_but_answers_busy() {
    let mut d = setup_domain(&configured(), true);
    outputs(&mut d, Event::Connection { forge: 1, state: Connection::SettingUp { step: Some(SetUpStep::Run) } });
    outputs(&mut d, Event::Connection { forge: 2, state: Connection::NotSetUp });
    let token = Token::new(91);
    assert_eq!(d.offer(2), Offer::Offered);
    assert_eq!(
        outputs(&mut d, setup_application(2, token, token)).as_ref(),
        [Request::ApplicationDone { request: token, token }]
    );
    assert_eq!(d.application_answer(token), Some(Answered::SetUp(SetUpAnswer::Refused(SetUpRefusal::Busy))));
}
#[test]
fn closing_the_box_fails_setup_and_marks_every_offer_unavailable() {
    let mut d = setup_domain(&configured(), true);
    outputs(&mut d, Event::Connection { forge: 1, state: Connection::SettingUp { step: Some(SetUpStep::Run) } });
    outputs(&mut d, Event::Connection { forge: 2, state: Connection::NotSetUp });
    assert_eq!(outputs(&mut d, Event::BoxClosed).as_ref(), [Request::MarkedForges]);
    assert_eq!(
        d.forges().rows.as_ref(),
        [
            Row {
                failed: Some(FailedStep::Run),
                ..setup_row(
                    1,
                    Kind::Forgejo,
                    Connection::SetUpFailed { why: b"the box closed the pipe".as_slice().into() },
                    Offer::NotUnderTheBox
                )
            },
            setup_row(2, Kind::GitHub, Connection::NotSetUp, Offer::NotUnderTheBox),
        ]
    );
    let token = Token::new(91);
    assert_eq!(
        outputs(&mut d, setup_application(1, token, token)).as_ref(),
        [Request::ApplicationDone { request: token, token }]
    );
    assert_eq!(d.application_answer(token), Some(Answered::SetUp(SetUpAnswer::Refused(SetUpRefusal::NotUnderTheBox))));
}
#[test]
fn configured_forges_are_bounded_at_construction() {
    fn config() -> skein_forge_client::Config {
        skein_forge_client::Config { namespace: b"test".as_slice().into(), writers: Box::new([]) }
    }
    let count = usize::try_from(LIMITS.client.repositories).unwrap();
    let mut forges = skein_lib::List::with_capacity(LIMITS.client.repositories.checked_add(1).unwrap());
    for number in 0..=count {
        forges
            .push(Configured { owned: None, number: u16::try_from(number).unwrap(), ..configured()[0].clone() })
            .unwrap();
    }
    Domain::new(&LIMITS, 4, config(), &[], &forges.as_slice()[..count], false).unwrap();
    assert_eq!(
        Domain::new(&LIMITS, 4, config(), &[], forges.as_slice(), false).unwrap_err(),
        skein_forge::api::Error::TooLarge
    );
}

#[test]
fn set_up_states_are_kept_as_told_and_give_no_writer() {
    let mut d = routing_domain();
    for state in [
        Connection::NotSetUp,
        Connection::SettingUp { step: None },
        Connection::SettingUp { step: Some(SetUpStep::Prepare) },
        Connection::SetUpFailed { why: Box::from(b"the box closed the pipe".as_slice()) },
    ] {
        assert_eq!(
            outputs(&mut d, Event::Connection { forge: 1, state: state.clone() }).as_ref(),
            [Request::MarkedForges]
        );
        assert_eq!(d.connection(1), state);
    }
}

#[test]
fn routing_installations_keep_their_stray_and_suspended_states_without_durable_rows() {
    let mut d = domain();
    let state = Installation { account: Some((42, Box::from(b"account".as_slice()))), stray: true, suspended: true };
    for _ in 0_u32..2 {
        assert_eq!(
            outputs(&mut d, Event::Installation { forge: 1, installation: 11, state: Some(state.clone()) }).as_ref(),
            [Request::MarkedForges]
        );
    }
    assert_eq!(d.installation(1, 11), Some(&state));
    assert_eq!(
        outputs(&mut d, Event::Installation { forge: 1, installation: 11, state: None }).as_ref(),
        [Request::MarkedForges]
    );
    assert_eq!(d.installation(1, 11), None);
}

#[test]
fn routing_tables_take_every_installation_with_the_connection_components_bounds() {
    let mut d = domain();
    d.connection_limits(2, 8);
    for forge in 1..=2 {
        for installation in 1..=8 {
            let state = Installation { account: None, stray: true, suspended: false };
            assert_eq!(
                outputs(&mut d, Event::Installation { forge, installation, state: Some(state) }).as_ref(),
                [Request::MarkedForges]
            );
        }
    }
    assert!(d.installation(2, 8).unwrap().stray);
    assert!(connection_worst_case(2, 8, LIMITS.name_bytes).is_some());
    assert_eq!(connection_worst_case(u32::MAX, u32::MAX, LIMITS.name_bytes), None);
}

fn setup_application(forge: u16, request: Token, token: Token) -> Event {
    Event::Application {
        role: None,
        carried: None,
        request,
        token,
        retention: Duration::from_secs(60),
        ask: Ask::SetUp {
            playground: false,
            forge,
            org: b"work".as_slice().into(),
            repository: b"hello".as_slice().into(),
            project: SetUpProject::Existing(1),
        },
    }
}

#[test]
fn a_setup_answer_finishes_its_application_without_a_durable_record() {
    let mut d = setup_domain(&configured(), true);
    let request = Token::new(91);
    let token = Token::new(92);
    outputs(&mut d, Event::Connection { forge: 1, state: Connection::NotSetUp });
    outputs(&mut d, setup_application(1, request, token));
    assert_eq!(d.application_answer(token), None);
    let answer = SetUpAnswer::SettingUp;
    assert_eq!(
        outputs(&mut d, Event::SetUpAnswered { forge: 1, request, answer }).as_ref(),
        [Request::ApplicationDone { request, token }]
    );
    assert_eq!(d.application_answer(token), Some(Answered::SetUp(answer)));
    assert_eq!(d.connection(1), Connection::NotSetUp);
    assert_eq!(domain().application_answer(token), None, "restart keeps no setup outcome");
}

#[test]
fn a_connection_change_keeps_the_latest_setup_answer() {
    let mut d = setup_domain(&configured(), true);
    let request = Token::new(91);
    let token = Token::new(92);
    outputs(&mut d, Event::Connection { forge: 1, state: Connection::NotSetUp });
    outputs(&mut d, setup_application(1, request, token));
    outputs(&mut d, Event::SetUpAnswered { forge: 1, request, answer: SetUpAnswer::SettingUp });
    outputs(&mut d, Event::Connection { forge: 1, state: Connection::SettingUp { step: Some(SetUpStep::Fetch) } });
    assert_eq!(d.application_answer(token), Some(Answered::SetUp(SetUpAnswer::SettingUp)));
}

#[test]
fn setup_answers_reserve_room_until_retention_ends() {
    let mut d = setup_domain(&configured(), true);
    outputs(&mut d, Event::Connection { forge: 1, state: Connection::NotSetUp });
    for number in 1..=u64::from(LIMITS.applications) {
        let request = Token::new(number);
        outputs(&mut d, setup_application(1, request, request));
    }
    let request = Token::new(99);
    assert_eq!(
        outputs(&mut d, setup_application(1, request, request)).as_ref(),
        [Request::ApplicationAgain { request, why: jig_domain_people::Again::Busy }]
    );
    outputs(
        &mut d,
        Event::SetUpAnswered {
            forge: 1,
            request: Token::new(1),
            answer: SetUpAnswer::Refused(SetUpRefusal::NotOffered),
        },
    );
    assert_eq!(
        outputs(&mut d, setup_application(1, request, request)).as_ref(),
        [Request::ApplicationAgain { request, why: jig_domain_people::Again::Busy }]
    );
    let mut later = env();
    later.wall = Wall::from_nanos(later.wall.as_nanos() + Duration::from_secs(60).as_nanos());
    let mut out = Queue::with_capacity(max_out(&LIMITS));
    step(&mut d, &later, setup_application(1, request, request), &mut out);
    assert_eq!(d.application_answer(Token::new(1)), None, "expired answer released");
    assert_eq!(out.pop(), Some(Request::MarkedForges));
    assert_eq!(
        out.pop(),
        Some(Request::SetUp {
            playground: false,
            forge: 1,
            request,
            org: b"work".as_slice().into(),
            repository: b"hello".as_slice().into(),
        })
    );
    assert!(out.is_empty());
}

#[test]
fn a_refused_second_setup_keeps_the_first_tokens_answer() {
    let mut d = setup_domain(&configured(), true);
    d.connection_limits(2, 1);
    for forge in [1, 2] {
        outputs(&mut d, Event::Connection { forge, state: Connection::NotSetUp });
        let token = Token::new(u64::from(forge));
        outputs(&mut d, setup_application(forge, token, token));
        outputs(&mut d, Event::SetUpAnswered { forge, request: token, answer: SetUpAnswer::SettingUp });
    }
    outputs(
        &mut d,
        Event::Connection { forge: 1, state: Connection::Connected { id: 7, login: b"bot".as_slice().into() } },
    );
    let request = Token::new(3);
    let requests = outputs(&mut d, setup_application(1, request, request));
    assert_eq!(requests.as_ref(), [Request::ApplicationDone { request, token: request }]);
    let answer = SetUpAnswer::Refused(SetUpRefusal::NotOffered);
    assert_eq!(d.application_answer(Token::new(1)), Some(Answered::SetUp(SetUpAnswer::SettingUp)));
    assert_eq!(d.application_answer(request), Some(Answered::SetUp(answer)));
    assert_eq!(d.application_answer(Token::new(2)), Some(Answered::SetUp(SetUpAnswer::SettingUp)));
}

#[test]
fn unconfigured_forges_keep_each_refusal_beside_the_configured_forges() {
    let mut d = setup_domain(&configured()[..1], true);
    d.connection_limits(1, 1);
    outputs(&mut d, Event::Connection { forge: 1, state: Connection::NotSetUp });
    for forge in [1, 2, 3] {
        let request = Token::new(u64::from(forge));
        outputs(&mut d, setup_application(forge, request, request));
        outputs(
            &mut d,
            Event::SetUpAnswered {
                forge,
                request,
                answer: if forge == 1 {
                    SetUpAnswer::SettingUp
                } else {
                    SetUpAnswer::Refused(SetUpRefusal::NotOffered)
                },
            },
        );
    }
    assert_eq!(d.application_answer(Token::new(1)), Some(Answered::SetUp(SetUpAnswer::SettingUp)));
    assert_eq!(
        d.application_answer(Token::new(2)),
        Some(Answered::SetUp(SetUpAnswer::Refused(SetUpRefusal::NotOffered)))
    );
    assert_eq!(
        d.application_answer(Token::new(3)),
        Some(Answered::SetUp(SetUpAnswer::Refused(SetUpRefusal::NotOffered)))
    );
    assert_eq!(d.forges().rows.len(), 1);
}

#[test]
fn the_forges_document_orders_connections_and_every_connection_marks_it() {
    let mut d = setup_domain(&[configured()[1].clone(), configured()[0].clone()], true);
    d.connection_limits(2, 1);
    for (forge, state) in [(2, Connection::NotSetUp), (1, Connection::Connecting), (2, Connection::NotSetUp)] {
        assert_eq!(outputs(&mut d, Event::Connection { forge, state }).as_ref(), [Request::MarkedForges]);
    }
    assert_eq!(
        d.forges().rows.as_ref(),
        [
            setup_row(1, Kind::Forgejo, Connection::Connecting, Offer::NotOffered),
            setup_row(2, Kind::GitHub, Connection::NotSetUp, Offer::Offered)
        ]
    );
}

#[test]
fn a_configured_forge_without_routing_input_reads_not_connected() {
    let d = setup_domain(&configured()[..1], true);
    assert_eq!(d.forges().rows.as_ref(), [setup_row(1, Kind::Forgejo, Connection::NotConnected, Offer::NotOffered)]);
}

#[test]
fn a_new_project_is_admitted_and_retained_until_the_repository_is_made() {
    let mut d = setup_domain(&configured(), true);
    outputs(&mut d, Event::Connection { forge: 1, state: Connection::NotSetUp });
    let request = Token::new(91);
    let token = Token::new(92);
    let project = SetUpProject::New(b"Next".as_slice().into());
    let emitted = outputs(
        &mut d,
        Event::Application {
            role: None,
            carried: None,
            request,
            token,
            retention: Duration::from_secs(60),
            ask: Ask::SetUp {
                playground: false,
                forge: 1,
                org: b"work".as_slice().into(),
                repository: b"hello".as_slice().into(),
                project: project.clone(),
            },
        },
    );
    assert_eq!(
        emitted.as_ref(),
        [
            Request::MarkedForges,
            Request::SetUp {
                playground: false,
                forge: 1,
                request,
                org: b"work".as_slice().into(),
                repository: b"hello".as_slice().into()
            }
        ]
    );
    assert_eq!(d.forges().rows[0].first.as_ref().unwrap().project, project);
    assert!(d.application_answer(token).is_none());
}
#[test]
fn setup_names_are_validated_at_admission_on_both_sides_of_the_name_bound() {
    use skein_forge_setup::{given, host, limits};
    for domain_bound in [4, LIMITS.name_bytes] {
        let mut env = env();
        env.limits.name_bytes = domain_bound;
        for org in [true, false] {
            let kit_bound = if org { 40 } else { 100 };
            let bound = domain_bound.min(kit_bound);
            let mut cases = skein_lib::List::with_capacity(11);
            for length in [0, 1, bound, bound + 1, kit_bound, kit_bound + 1] {
                let mut bytes = skein_lib::Writer::new(usize::try_from(length).unwrap());
                for _ in 0..length {
                    bytes.put(b"x").unwrap();
                }
                cases.push(bytes.finish()).unwrap();
            }
            for name in [b".".as_slice(), b"..", b"a/b", b"-a", b"a._-9"] {
                cases.push(Box::from(name)).unwrap();
            }
            for name in cases.as_slice() {
                let mut d = setup_domain(&configured(), true);
                outputs(&mut d, Event::Connection { forge: 1, state: Connection::NotSetUp });
                let request = Token::new(91);
                let token = Token::new(92);
                let org_name: Box<[u8]> = if org { name.clone() } else { b"o".as_slice().into() };
                let repository: Box<[u8]> = if org { b"r".as_slice().into() } else { name.clone() };
                let plan = host::Plan::Create {
                    forge: 1,
                    org: org_name.clone(),
                    repository: repository.clone(),
                    files: Box::new([]),
                };
                let kit_limits = limits::Limits {
                    orgs: 1,
                    repositories: 1,
                    reachable: 1,
                    files: 1,
                    file_bytes: 4096,
                    deadline: Duration::from_secs(120),
                    backoff: Duration::from_millis(100),
                    backoff_max: Duration::from_secs(5),
                };
                let accepted = given::check_create(&kit_limits, domain_bound, &plan).is_ok();
                let ask = Ask::SetUp {
                    playground: false,
                    forge: 1,
                    org: org_name.clone(),
                    repository: repository.clone(),
                    project: SetUpProject::Existing(1),
                };
                let mut emitted = Queue::with_capacity(max_out(&env.limits));
                step(
                    &mut d,
                    &env,
                    Event::Application {
                        role: None,
                        carried: None,
                        request,
                        token,
                        retention: Duration::from_secs(60),
                        ask,
                    },
                    &mut emitted,
                );
                if accepted {
                    assert_eq!(emitted.pop(), Some(Request::MarkedForges));
                    assert_eq!(
                        emitted.pop(),
                        Some(Request::SetUp { playground: false, forge: 1, request, org: org_name, repository })
                    );
                    assert_eq!(d.forges().rows[0].first.as_ref().unwrap().state, FirstState::Making);
                } else {
                    assert_eq!(emitted.pop(), Some(Request::ApplicationDone { request, token }));
                    assert_eq!(
                        d.application_answer(token),
                        Some(Answered::SetUp(SetUpAnswer::Refused(SetUpRefusal::Invalid)))
                    );
                    assert!(d.forges().rows[0].first.is_none());
                }
                assert!(emitted.is_empty());
            }
        }
    }
}
#[test]
fn admitted_setup_projects_its_fields_as_making_and_the_next_admission_replaces_them() {
    let mut d = setup_domain(&configured(), true);
    for (number, org, repository, project) in
        [(91, b"work".as_slice(), b"hello".as_slice(), 1), (93, b"team", b"widget", 2)]
    {
        outputs(
            &mut d,
            Event::Connection { forge: 1, state: Connection::SetUpFailed { why: b"retry".as_slice().into() } },
        );
        let request = Token::new(number);
        let token = Token::new(number + 1);
        let first = First {
            playground: false,
            org: org.into(),
            repository: repository.into(),
            project: SetUpProject::Existing(project),
            state: FirstState::Making,
            label: Box::from(b"temper-host".as_slice()),
        };
        let ask = Ask::SetUp {
            playground: false,
            forge: 1,
            org: first.org.clone(),
            repository: first.repository.clone(),
            project: first.project.clone(),
        };
        assert_eq!(
            outputs(
                &mut d,
                Event::Application {
                    role: None,
                    carried: None,
                    request,
                    token,
                    retention: Duration::from_secs(60),
                    ask
                }
            )
            .as_ref(),
            [
                Request::MarkedForges,
                Request::SetUp { playground: false, forge: 1, request, org: org.into(), repository: repository.into() }
            ]
        );
        assert_eq!(d.forges().rows[0].first, Some(first));
        outputs(&mut d, Event::SetUpAnswered { forge: 1, request, answer: SetUpAnswer::SettingUp });
    }
}

#[test]
fn first_repository_tells_mark_made_failed_and_retry_and_ignore_absent_intents() {
    let mut d = setup_domain(&configured(), true);
    for forge in [1, 99] {
        assert!(outputs(&mut d, Event::FirstMade { forge, id: 42 }).is_empty());
        assert!(outputs(&mut d, Event::FirstFailed { forge, step: CreateStep::File }).is_empty());
    }
    outputs(&mut d, Event::Connection { forge: 1, state: Connection::NotSetUp });
    outputs(&mut d, setup_application(1, Token::new(91), Token::new(92)));
    assert_eq!(outputs(&mut d, Event::FirstMade { forge: 1, id: 42 }).as_ref(), [Request::MarkedForges]);
    assert_eq!(d.forges().rows[0].first.as_ref().unwrap().state, FirstState::Made { id: 42 });
    for step in [CreateStep::Admin, CreateStep::Org, CreateStep::Repository, CreateStep::File] {
        assert_eq!(outputs(&mut d, Event::FirstFailed { forge: 1, step }).as_ref(), [Request::MarkedForges]);
        assert_eq!(d.forges().rows[0].first.as_ref().unwrap().state, FirstState::Failed { step });
    }
    outputs(&mut d, Event::Connection { forge: 1, state: Connection::SetUpFailed { why: b"file".as_slice().into() } });
    outputs(&mut d, setup_application(1, Token::new(93), Token::new(94)));
    assert_eq!(d.forges().rows[0].first.as_ref().unwrap().state, FirstState::Making);
}

#[test]
fn a_tag_round_trips_its_pair_at_each_halfs_bound() {
    assert_eq!(tag(1, 1), 4_294_967_297);
    assert_eq!(tag(3, 1), 12_884_901_889);
    assert_eq!(tag(u16::MAX, u32::MAX), 0x0000_ffff_ffff_ffff);
    for forge in [0, u16::MAX] {
        for repository in [0, u32::MAX] {
            assert_eq!(untag(tag(forge, repository)), Some((forge, repository)));
        }
    }
    assert_eq!(untag(0x0001_0000_0000_0000), None, "bit 48 is outside the forge identity");
    assert_eq!(untag(u64::MAX), None);
}

#[test]
fn two_forges_repositories_with_one_id_resolve_apart() {
    let mut d = domain();
    for forge in [1, 3] {
        let mut repository = adopted();
        repository.provider.forge = forge;
        repository.home = forge == 1;
        outputs(&mut d, Event::Restore { record: Stored::Repository(repository.clone()) });
        assert_eq!(d.repository_tag(5, tag(forge, REPO.repository)), Some(&repository));
    }
    for forge in [1, 3] {
        let provider = skein_forge::api::Repository { forge, repository: REPO.repository };
        assert_eq!(d.repository_tag(5, tag(forge, REPO.repository)).expect("adopted repository").provider, provider);
        assert_eq!(d.repository_tag(6, tag(forge, REPO.repository)), None, "another project");
    }
    assert_eq!(d.repository_tag(5, u64::from(REPO.repository)), None, "a bare forge-local id");
    assert_eq!(d.repository_tag(5, 0x0001_0001_0000_0002), None, "invalid forge width");
}

#[test]
fn pushed_heads_match_their_own_forge_only() {
    let mut d = domain();
    for forge in [1, 3] {
        let mut repository = adopted();
        repository.provider.forge = forge;
        repository.home = forge == 1;
        outputs(&mut d, Event::Restore { record: Stored::Repository(repository) });
        let mut name = branch();
        name.forge = forge;
        outputs(&mut d, Event::Hold { task: 7, resource: name.clone(), from: None });
        outputs(&mut d, Event::Claim { task: 9, attempt: 1, writes: Box::new([name]), holders: Box::new([7]) });
    }
    for forge in [1, 3] {
        let mut expected = branch();
        expected.forge = forge;
        assert_eq!(
            d.reported_heads(&LIMITS, 9, 1, &[Pushed { tag: tag(forge, REPO.repository), commit: [2; 32] }]).as_ref(),
            &[(expected, [2; 32])]
        );
    }
    assert!(d.reported_heads(&LIMITS, 9, 1, &[Pushed { tag: 0x0001_0001_0000_0002, commit: [2; 32] }]).is_empty());
}

#[test]
fn forges_lists_web_and_only_each_forges_adoptions_for_every_kind() {
    for kind in [Kind::Forgejo, Kind::GitHub] {
        let mut d = setup_domain(
            &[Configured {
                admin_connect: false,
                bot: Box::new([]),
                owned: None,
                number: 1,
                kind,
                mode: Mode::Connected,
                asked: false,
                git: Box::new([]),
                web: Some(Box::from(b"https://forge.example".as_slice())),
                read_only: false,
            }],
            false,
        );
        let own = adopted();
        outputs(&mut d, Event::Restore { record: Stored::Repository(own.clone()) });
        let mut elsewhere = own.clone();
        elsewhere.provider.forge = 2;
        outputs(&mut d, Event::Restore { record: Stored::Repository(elsewhere) });
        let rows = d.forges();
        let [row] = rows.rows.as_ref() else { panic!("one configured forge") };
        assert_eq!(row.web.as_deref(), Some(b"https://forge.example".as_slice()));
        assert_eq!(row.repositories.as_ref(), &[AdoptedRow { owner: own.owner, name: own.name, project: own.project }]);
    }
}

#[test]
fn forgetting_or_restoring_an_adoption_publishes_the_forges_row() {
    let mut d = setup_domain(&[Configured { mode: Mode::Connected, ..configured()[0].clone() }], false);
    let own = adopted();
    outputs(&mut d, Event::Restore { record: Stored::Repository(own.clone()) });
    let out = outputs(&mut d, Event::ForgetAdoption { repository: own.provider, restore: None });
    assert!(out.contains(&Request::MarkedForges), "{out:?}");
    assert!(d.forges().rows[0].repositories.is_empty());
    let out = outputs(&mut d, Event::ForgetAdoption { repository: own.provider, restore: Some(own.clone()) });
    assert!(out.contains(&Request::MarkedForges), "{out:?}");
    assert_eq!(d.forges().rows[0].repositories.len(), 1);
}

#[test]
fn forges_carry_owned_startup_details_for_the_owned_row_alone() {
    let owned = Owned {
        forgejo: b"17.0.0".as_slice().into(),
        runner: Some(b"13.2.0".as_slice().into()),
        password: b"/fake/forge/person_password".as_slice().into(),
    };
    let configs = [
        Configured { owned: Some(owned.clone()), ..configured()[0].clone() },
        Configured { owned: None, mode: Mode::Managed, ..configured()[1].clone() },
    ];
    let domain = setup_domain(&configs, true);
    let rows = domain.forges().rows;
    assert_eq!(rows[0].owned, Some(owned));
    assert_eq!(rows[1].owned, None);
}

#[test]
fn setup_failures_name_the_held_step_until_state_changes() {
    let tell = |state| Event::Connection { forge: 1, state };
    let failure = Connection::SetUpFailed { why: b"stopped".as_slice().into() };
    let fetch = Connection::SettingUp { step: Some(SetUpStep::Fetch) };
    let prepare = Connection::SettingUp { step: Some(SetUpStep::Prepare) };
    let run = Connection::SettingUp { step: Some(SetUpStep::Run) };
    let connected = Connection::Connected { id: 7, login: b"temper".as_slice().into() };
    let stopped = Connection::Stopped { reason: b"stopped".as_slice().into() };
    let rows: [(Box<[Event]>, Option<FailedStep>); 22] = [
        (Box::new([tell(Connection::NotConnected), tell(failure.clone())]), None),
        (Box::new([tell(Connection::NotSetUp), tell(failure.clone())]), None),
        (Box::new([tell(Connection::SettingUp { step: None }), tell(failure.clone())]), None),
        (Box::new([tell(fetch.clone()), tell(failure.clone())]), Some(FailedStep::Fetch)),
        (Box::new([tell(prepare.clone()), tell(failure.clone())]), Some(FailedStep::Prepare)),
        (Box::new([tell(run.clone()), tell(failure.clone())]), Some(FailedStep::Run)),
        (Box::new([tell(Connection::Connecting), tell(failure.clone())]), Some(FailedStep::Connect)),
        (Box::new([tell(connected.clone()), tell(failure.clone())]), None),
        (Box::new([tell(stopped.clone()), tell(failure.clone())]), None),
        (Box::new([tell(failure.clone()), tell(failure.clone())]), None),
        (Box::new([tell(run.clone()), Event::BoxClosed]), Some(FailedStep::Run)),
        (
            Box::new([tell(prepare.clone()), tell(failure.clone()), Event::BoxClosed, tell(failure.clone())]),
            Some(FailedStep::Prepare),
        ),
        (Box::new([tell(fetch.clone()), tell(failure.clone()), tell(Connection::NotConnected)]), None),
        (Box::new([tell(fetch.clone()), tell(failure.clone()), tell(Connection::NotSetUp)]), None),
        (Box::new([tell(fetch.clone()), tell(failure.clone()), tell(Connection::SettingUp { step: None })]), None),
        (Box::new([tell(fetch.clone()), tell(failure.clone()), tell(prepare.clone())]), None),
        (Box::new([tell(fetch.clone()), tell(failure.clone()), tell(run.clone())]), None),
        (Box::new([tell(fetch.clone()), tell(failure.clone()), tell(Connection::Connecting)]), None),
        (Box::new([tell(fetch.clone()), tell(failure.clone()), tell(connected)]), None),
        (Box::new([tell(fetch.clone()), tell(failure.clone()), tell(stopped)]), None),
        (Box::new([tell(prepare), tell(failure.clone()), tell(run), tell(failure.clone())]), Some(FailedStep::Run)),
        (
            Box::new([tell(fetch), tell(failure.clone()), tell(Connection::Connecting), tell(failure)]),
            Some(FailedStep::Connect),
        ),
    ];
    for (index, (events, expected)) in rows.into_iter().enumerate() {
        let mut d = setup_domain(&configured()[..1], true);
        for event in events {
            outputs(&mut d, event);
        }
        assert_eq!(d.forges().rows[0].failed, expected, "row {index}");
    }
}

#[test]
fn workspace_landing_is_only_for_an_existing_inherited_writable_producer() {
    use temper_engine_domain_forge_change as change;
    for kind in [
        change::Delegate::Produce,
        change::Delegate::Repair(change::Repair::Gate(1)),
        change::Delegate::Resolve { base: [1; 32] },
        change::Delegate::Gate { number: 1, head: [1; 32] },
    ] {
        for existing in [false, true] {
            for saved in [false, true] {
                for writable in [false, true] {
                    let mut d = domain();
                    let mut repo = adopted();
                    repo.kinds.push = writable;
                    outputs(&mut d, Event::Restore { record: Stored::Repository(repo) });
                    let mut row = fixtures::opening_row();
                    row.delegate = Some((90, kind));
                    let task = row.task;
                    let branch = row.branch.clone();
                    let base = row.base.clone();
                    outputs(&mut d, Event::Restore { record: Stored::Change(row) });
                    if existing {
                        let name = run_branch(b"51");
                        assert!(items::branch_matches(&name, &branch), "the fixture's producer branch");
                        outputs(
                            &mut d,
                            Event::Restore {
                                record: Stored::BranchHead(BranchHead { name, commit: [1; 32], owned: true }),
                            },
                        );
                    }
                    let item = d
                        .workspace_item(items::Seed {
                            task: 90,
                            parent: Some(task),
                            repository: REPO,
                            run_branch: Box::from(&b"run"[..]),
                            saved_branch: if saved { Some(Box::from(&b"saved"[..])) } else { None },
                        })
                        .expect("item");
                    let eligible = kind == change::Delegate::Produce && existing && !saved && writable;
                    assert_eq!(item.landing.as_deref(), if eligible { Some(base.as_ref()) } else { None });
                    let start = match &item.start {
                        items::Start::Base(b)
                        | items::Start::Branch(b)
                        | items::Start::Saved(b)
                        | items::Start::Merge { branch: b, .. } => b,
                    };
                    let pushed = match &item.push {
                        Some(branch) => branch.len(),
                        None => 0,
                    };
                    let landing = match &item.landing {
                        Some(branch) => branch.len(),
                        None => 0,
                    };
                    let payload: usize =
                        item.name.len() + item.host.len() + item.owner.len() + start.len() + pushed + landing;
                    assert_eq!(item.bytes, u64::try_from(size_of::<items::Item>() + payload).expect("sized item"));
                }
            }
        }
    }
}

fn failed_issue() -> (Domain, issues::GoalView) {
    const GOAL: u64 = 7;
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    let view = issues::GoalView {
        phase: issues::Phase::Active,
        goal: GOAL,
        repository: u64::from(REPO.repository),
        title: "Work".into(),
        goal_text: "Complete the work".into(),
        plan: Box::new([]),
        milestones: Box::new([]),
        finished: None,
    };
    outputs(
        &mut d,
        Event::Restore {
            record: Stored::Issue(IssueRow {
                goal: GOAL,
                repository: REPO,
                state: issues::Projected::default(),
                number: None,
                pending: None,
                before: None,
                failed: true,
                desired: Some(view.clone()),
            }),
        },
    );
    (d, view)
}

fn has_projection_effect(requests: &[Request]) -> bool {
    for request in requests {
        let Request::ProjectionEffect { .. } = request else { continue };
        return true;
    }
    false
}

#[test]
fn a_failed_projection_waits_but_changed_facts_retry_immediately() {
    const ENTRY: u64 = 40;
    let (mut d, mut view) = failed_issue();
    let when = Wall::from_nanos(env().wall.as_nanos() + LIMITS.issue_policy.interval.as_nanos());
    d.projection_after(view.goal, when);
    let waiting = outputs(&mut d, Event::ProjectDesired { entry: ENTRY, goal: view.goal });
    assert!(waiting.contains(&Request::ProjectAfter { goal: view.goal, when: Some(when) }));
    assert!(!has_projection_effect(&waiting));
    view.goal_text = "New committed goal words".into();
    let retry = outputs(&mut d, Event::Project { entry: ENTRY, repository: REPO, view });
    assert!(has_projection_effect(&retry));
}

#[test]
fn a_closing_failed_projection_settles_and_keeps_its_failed_row() {
    const ENTRY: u64 = 40;
    let (mut d, mut view) = failed_issue();
    view.finished = Some("Goal finished".into());
    let goal = view.goal;
    let settled = outputs(&mut d, Event::Project { entry: ENTRY, repository: REPO, view });
    assert!(settled.contains(&Request::ProjectionSettled { goal }));
    assert!(!has_projection_effect(&settled));
    outputs(&mut d, Event::ForgetProjection { goal });
    assert!(d.issue(goal).expect("retained diagnostic").failed);
}

#[test]
fn an_untranslatable_closing_feed_settles_a_failed_projection() {
    const GOAL: u64 = 7;
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    let settled = outputs(&mut d, Event::FailProjection { goal: GOAL, repository: REPO, closing: true });
    assert!(settled.contains(&Request::ProjectionFailed { goal: GOAL }));
    assert!(settled.contains(&Request::ProjectionSettled { goal: GOAL }));
    assert!(d.issue(GOAL).expect("failed diagnostic").failed);
}
