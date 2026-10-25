use super::{commit_text, landing_words};
use alloc::boxed::Box;

const SHA1: [u8; 32] = [
    0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef,
    0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
const SHA256: [u8; 32] = [0xab; 32];
const COMMITS: &[([u8; 32], &[u8])] = &[
    (SHA1, b"abcdef0123456789abcdef0123456789abcdef01"),
    (SHA256, b"abababababababababababababababababababababababababababababababab"),
];

#[test]
fn commit_text_uses_lowercase_digits_at_the_identifier_width() {
    for (commit, digits) in COMMITS {
        assert_eq!(commit_text(commit).as_ref(), *digits, "commit {commit:?}");
    }
}

#[test]
fn landing_words_are_the_merge_id_cut_to_the_contract_bound() {
    for (merge, bound, expected) in [
        (SHA1, 64, COMMITS[0].1),
        (SHA1, 40, COMMITS[0].1),
        (SHA256, 512, COMMITS[1].1),
        (SHA256, 64, COMMITS[1].1),
        (SHA256, 32, &COMMITS[1].1[..32]),
        (SHA1, 7, &b"abcdef0"[..]),
        (SHA1, 0, &b""[..]),
    ] {
        assert_eq!(landing_words(&merge, bound).as_ref(), expected, "merge {merge:?}, bound {bound}");
    }
}

#[test]
fn not_installed_ends_the_roots_adoption_part_as_missing_access() {
    use jig_domain_people as people;
    assert_eq!(
        super::adoption_failure(skein_forge::api::Error::NotInstalled),
        people::Outcome::Refused(people::Refusal::Authority {
            lacking: people::Lack::ResourceAccess,
            proposable: None
        })
    );
    assert_eq!(super::adoption_failure(skein_forge::api::Error::Busy), people::Outcome::Again(people::Again::Busy));
}

fn adopted(forge: u16) -> super::forge::Repository {
    super::forge::Repository {
        project: 5,
        home: forge == 1,
        provider: skein_forge::api::Repository { forge, repository: 1 },
        host: Box::from(&b"forge.example"[..]),
        owner: Box::from(&b"org"[..]),
        name: Box::from(&b"repo"[..]),
        prefix: Box::from(&b"temper/"[..]),
        role: super::forge::Role::Owned,
        ci: true,
        checks: Box::new([]),
        kinds: super::forge::Kinds {
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
        protection: super::forge::Protection::Absent,
        settings: skein_forge::api::Settings {
            default_branch: Box::from(&b"main"[..]),
            merge: true,
            squash: true,
            rebase: false,
        },
    }
}

#[test]
fn tree_branches_count_prefix_slashes_against_authority_segments() {
    // forge, host, owner, repository, branch, temper, root, marker/task.
    const AUTHORITY_SEGMENTS: u32 = 8;
    // The old live prefix puts live-x in a branch segment of its own.
    const DEEP_AUTHORITY_SEGMENTS: u32 = 9;
    const BRANCH_BYTES: u32 = 128;
    for (prefix, segments, fits) in [
        (b"temper/live-x/".as_slice(), AUTHORITY_SEGMENTS, false),
        (b"temper/live-x/", DEEP_AUTHORITY_SEGMENTS, true),
        (b"temper/", AUTHORITY_SEGMENTS, true),
        (b"temper/live-x-", AUTHORITY_SEGMENTS, true),
    ] {
        let mut repository = adopted(1);
        repository.prefix = prefix.into();
        for marker in [b'c', b'r', b's'] {
            for attempt in [None, Some(3)] {
                let branch = super::tree_branch(&repository, 1, marker, 2, attempt, BRANCH_BYTES).expect("tree branch");
                let what = super::branch_what(&branch, BRANCH_BYTES).expect("branch parts");
                assert_eq!(super::resource_name(&repository, &what, segments).is_some(), fits, "branch {branch:?}");
            }
        }
    }
}

#[test]
fn delegations_with_one_repository_id_take_their_own_forges_holding() {
    use super::{forge, tasks};
    use alloc::boxed::Box;
    let (mut domain, env) = two_forges();
    let connector = domain.config.forge_connector;
    for number in [1, 3] {
        let spec = tasks::Spec {
            words: Box::new([]),
            inputs: Box::new([]),
            parameters: Box::new([tasks::Parameter::Resource { name: 1, connector, resource: forge::tag(number, 1) }]),
        };
        let holdings = super::task_holdings(
            &mut domain,
            &env,
            5,
            7,
            8,
            tasks::Executor::Procedure { connector, code: 2 },
            &spec,
            None,
        )
        .expect("delegation admitted");
        let [tasks::Holding::Write { resource, .. }] = holdings.as_ref() else { panic!("one write holding") };
        let name = super::forge_name(connector, resource, env.limits.forge.name_bytes).expect("forge holding");
        assert_eq!((name.forge, name.repository), (number, 1));
    }
}

#[test]
fn saved_resources_keep_the_forge_in_their_path() {
    use super::forge;
    let tags = [forge::tag(1, 1), forge::tag(3, 1)];
    let resources = super::saved_resources(4, &tags).expect("valid tags");
    assert_ne!(resources[0].path, resources[1].path);
    assert_eq!(
        resources[0].path.as_ref(),
        &[alloc::boxed::Box::from(1_u16.to_be_bytes()), alloc::boxed::Box::from(1_u32.to_be_bytes())]
    );
    let decoded = super::saved_tags(&resources, 4).expect("valid saved paths");
    assert_eq!(decoded.as_ref(), tags.as_slice());
    let (domain, _) = two_forges();
    for (tag, number) in decoded.iter().zip([1, 3]) {
        assert_eq!(domain.forge.repository_tag(5, *tag).expect("own repository").provider.forge, number);
    }
    assert!(super::saved_resources(4, &[0x0001_0001_0000_0001]).is_none());
    let mut malformed = resources.clone();
    malformed[0].path[0] = alloc::boxed::Box::from(1_u32.to_be_bytes());
    assert!(super::saved_tags(&malformed, 4).is_none(), "a forge wider than u16 is invalid");
}

fn two_forges() -> (super::Domain, skein_lib::Env<super::Limits>) {
    use super::forge;
    let (mut domain, env) = crate::tests::resume_fixture::root();
    let mut out = skein_lib::Queue::with_capacity(forge::max_out(&env.limits.forge));
    for number in [1, 3] {
        forge::step(
            &mut domain.forge,
            &skein_lib::Env { now: env.now, wall: env.wall, limits: env.limits.forge },
            forge::Event::Restore { record: forge::Stored::Repository(adopted(number)) },
            &mut out,
        );
    }
    (domain, env)
}

#[test]
fn pushed_heads_route_only_to_their_own_forges_hold() {
    use super::{Work, forge};
    let (mut domain, env) = two_forges();
    let forge_env = skein_lib::Env { now: env.now, wall: env.wall, limits: env.limits.forge };
    let mut out = skein_lib::Queue::with_capacity(forge::max_out(&env.limits.forge));
    for number in [1, 3] {
        let name = forge::Name {
            forge: number,
            repository: 1,
            what: forge::What::Branch(Box::new([Box::from(b"main".as_slice())])),
        };
        forge::step(
            &mut domain.forge,
            &forge_env,
            forge::Event::Restore {
                record: forge::Stored::Hold(forge::Hold {
                    name,
                    task: 7,
                    writer: Some(forge::Writer::Run { task: 8, attempt: 1 }),
                    drift: None,
                }),
            },
            &mut out,
        );
    }
    for number in [1, 3] {
        super::left(&mut domain, &env, 8, 1, Box::new([forge::Pushed { tag: forge::tag(number, 1), commit: [2; 32] }]));
        let Some(Work::Forge(forge::Event::Answered { pushed, .. })) = domain.work.pop() else {
            panic!("reported heads")
        };
        let [(name, commit)] = pushed.as_ref() else { panic!("one matching hold") };
        assert_eq!((name.forge, name.repository, *commit), (number, 1, [2; 32]));
        let Some(Work::Forge(forge::Event::Lost { task: 8, attempt: 1 })) = domain.work.pop() else {
            panic!("writer release follows reported heads")
        };
    }
}

#[test]
fn writable_repositories_include_only_the_requested_project() {
    let (domain, _) = two_forges();
    assert_eq!(domain.forge.writable_repositories(5).len(), 2);
    assert!(domain.forge.writable_repositories(1).is_empty());
}

#[test]
fn writable_repositories_exclude_context_and_missing_push() {
    use super::forge;
    let (mut domain, env) = two_forges();
    let mut out = skein_lib::Queue::with_capacity(forge::max_out(&env.limits.forge));
    let mut context = adopted(1);
    context.role = forge::Role::Context;
    let mut read_only = adopted(3);
    read_only.kinds.push = false;
    for repository in [context, read_only] {
        forge::step(
            &mut domain.forge,
            &skein_lib::Env { now: env.now, wall: env.wall, limits: env.limits.forge },
            forge::Event::Restore { record: forge::Stored::Repository(repository) },
            &mut out,
        );
    }
    assert!(domain.forge.writable_repositories(5).is_empty());
}

#[test]
fn writable_repositories_exclude_an_ended_forge() {
    use super::forge;
    let (mut domain, env) = two_forges();
    domain.forge.configure_demo(Some(forge::DemoConfig {
        provider: 1,
        adoption: forge::Adoption {
            project: 5,
            home: true,
            forge: 1,
            host: Box::from(&b"forge.example"[..]),
            owner: Box::from(&b"org"[..]),
            name: Box::from(&b"repo"[..]),
            prefix: Box::from(&b"temper/"[..]),
            role: forge::Role::Owned,
            landing: Box::from(&b"main"[..]),
            ci: true,
            checks: Box::new([]),
        },
        deadline: skein_lib::Duration::from_secs(60),
    }));
    let mut out = skein_lib::Queue::with_capacity(forge::max_out(&env.limits.forge));
    forge::step(
        &mut domain.forge,
        &skein_lib::Env { now: env.now, wall: env.wall, limits: env.limits.forge },
        forge::Event::Restore { record: forge::Stored::Demo(forge::Demo { forge: 1, state: forge::DemoState::Ended }) },
        &mut out,
    );
    let repositories = domain.forge.writable_repositories(5);
    assert_eq!(repositories.len(), 1);
    assert_eq!(repositories[0].provider.forge, 3);
}

fn change_row(pull: Option<u64>) -> super::forge::ChangeRow {
    use super::{forge, forge_change as change};
    forge::ChangeRow {
        task: 7,
        repository: skein_forge::api::Repository { forge: 3, repository: 1 },
        branch: Box::from(b"temper/7".as_slice()),
        base: Box::from(b"main".as_slice()),
        title: Box::from(b"Change".as_slice()),
        body: Box::new([]),
        priority: 0,
        change: change::Change {
            task: 7,
            state: change::State::Producing { requested: false },
            gates: Box::new([]),
            clean: Box::new([]),
            repairs: 0,
            resolutions: 0,
            updates: 0,
            since: skein_lib::Wall::from_nanos(0),
            ready_since: None,
            owns_turn: false,
            last_head: None,
        },
        pull,
        pending: None,
        effect: change::EffectResult::None,
        delegate: None,
        delegate_status: change::Status::Unknown,
        verdicts: Box::new([]),
        gate_remarks: Box::new([]),
        drift: None,
        base_repair: false,
        queue_repair: None,
        queue_repairs: 0,
    }
}

fn change_names(pull: Option<u64>) -> Box<[super::forge::Name]> {
    use super::forge::{Name, What};
    Box::new([
        Name {
            forge: 3,
            repository: 1,
            what: What::Branch(Box::new([Box::from(b"temper".as_slice()), Box::from(b"7".as_slice())])),
        },
        Name {
            forge: 3,
            repository: 1,
            what: match pull {
                Some(number) => What::Pull(number),
                None => What::Branch(Box::new([Box::from(b"main".as_slice())])),
            },
        },
    ])
}

#[test]
fn a_changes_saved_pull_replaces_its_base_watch_within_two_names() {
    use super::{Decision, Work, forge};
    let (mut domain, mut env) = two_forges();
    env.limits.forge.resources_per_task = 2;
    let forge_env = skein_lib::Env { now: env.now, wall: env.wall, limits: env.limits.forge };
    let mut out = skein_lib::Queue::with_capacity(forge::max_out(&env.limits.forge));
    forge::step(
        &mut domain.forge,
        &forge_env,
        forge::Event::Restore { record: forge::Stored::Names { task: 7, resources: change_names(None) } },
        &mut out,
    );
    forge::step(
        &mut domain.forge,
        &forge_env,
        forge::Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic },
        &mut out,
    );
    while let Some(request) = out.pop() {
        drop(request);
    }
    let mut decision = Decision::new(&env.limits.journal);
    for pull in [None, Some(23)] {
        out.push(forge::Request::Save {
            record: forge::Stored::Change(change_row(pull)),
            release: None,
            read_afresh: None,
        });
        super::outputs(&mut domain, &env, &mut decision, &mut out);
        if pull.is_none() {
            assert!(domain.work.is_empty(), "before the pull, keep branch and base");
        }
    }
    let Some(Work::Forge(event @ forge::Event::Rename { .. })) = domain.work.pop() else {
        panic!("name the known pull")
    };
    let forge::Event::Rename { task, resources } = &event else { panic!("pull watch") };
    assert_eq!(*task, 7);
    assert_eq!(*resources, change_names(Some(23)));
    forge::step(&mut domain.forge, &forge_env, event, &mut out);
    assert_eq!(domain.forge.names(7), Some(change_names(Some(23)).as_ref()));
    assert!(domain.work.is_empty(), "the branch's hold needs no change");
}

#[test]
fn a_restored_changes_wake_names_its_known_pull_again() {
    use super::{Decision, Work, forge};
    let (mut domain, mut env) = two_forges();
    env.limits.forge.resources_per_task = 2;
    let forge_env = skein_lib::Env { now: env.now, wall: env.wall, limits: env.limits.forge };
    let mut out = skein_lib::Queue::with_capacity(forge::max_out(&env.limits.forge));
    // An old store, or a restart between saving the pull and saving its names.
    for record in
        [forge::Stored::Change(change_row(Some(23))), forge::Stored::Names { task: 7, resources: change_names(None) }]
    {
        forge::step(&mut domain.forge, &forge_env, forge::Event::Restore { record }, &mut out);
    }
    forge::step(
        &mut domain.forge,
        &forge_env,
        forge::Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic },
        &mut out,
    );
    while let Some(request) = out.pop() {
        drop(request);
    }
    out.push(forge::Request::WakeProcedure { task: 7 });
    let mut decision = Decision::new(&env.limits.journal);
    super::outputs(&mut domain, &env, &mut decision, &mut out);
    let Some(Work::Forge(event @ forge::Event::Rename { .. })) = domain.work.pop() else {
        panic!("restore pull watch")
    };
    let forge::Event::Rename { task, resources } = &event else { panic!("pull watch") };
    assert_eq!(*task, 7);
    assert_eq!(*resources, change_names(Some(23)));
    forge::step(&mut domain.forge, &forge_env, event, &mut out);
    let Some(Work::Tasks(super::tasks::Event::WakeProcedure { task: 7 })) = domain.work.pop() else {
        panic!("wake the change's procedure")
    };
    assert!(domain.work.is_empty());
    // Later wakes preserve the established watch's polling cadence.
    for _ in 0..out.len() {
        drop(out.pop());
    }
    out.push(forge::Request::WakeProcedure { task: 7 });
    super::outputs(&mut domain, &env, &mut decision, &mut out);
    let Some(Work::Tasks(super::tasks::Event::WakeProcedure { task: 7 })) = domain.work.pop() else {
        panic!("wake the change's procedure")
    };
    assert!(domain.work.is_empty());
}

/// A change's restored records with its forge state settled and the outputs drained.
fn restored_change(
    records: impl IntoIterator<Item = super::forge::Stored>,
) -> (super::Domain, skein_lib::Env<super::Limits>, skein_lib::Queue<super::forge::Request>) {
    use super::forge;
    let (mut domain, mut env) = two_forges();
    env.limits.forge.resources_per_task = 2;
    let forge_env = skein_lib::Env { now: env.now, wall: env.wall, limits: env.limits.forge };
    let mut out = skein_lib::Queue::with_capacity(forge::max_out(&env.limits.forge));
    for record in records {
        forge::step(&mut domain.forge, &forge_env, forge::Event::Restore { record }, &mut out);
    }
    forge::step(
        &mut domain.forge,
        &forge_env,
        forge::Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic },
        &mut out,
    );
    while let Some(request) = out.pop() {
        drop(request);
    }
    (domain, env, out)
}

#[test]
fn an_ended_changes_pull_wake_names_nothing() {
    use super::{Decision, Work, forge};
    // Its release unnamed it: the change row stays, its names row does not.
    let (mut domain, env, mut out) = restored_change([forge::Stored::Change(change_row(Some(23)))]);
    out.push(forge::Request::WakeProcedure { task: 7 });
    let mut decision = Decision::new(&env.limits.journal);
    super::outputs(&mut domain, &env, &mut decision, &mut out);
    let Some(Work::Tasks(super::tasks::Event::WakeProcedure { task: 7 })) = domain.work.pop() else {
        panic!("wake the change's procedure")
    };
    assert!(domain.work.is_empty(), "an ended change watches nothing");
    assert_eq!(domain.forge.names(7), None);
}

#[test]
fn a_change_unnamed_after_its_pull_swap_was_queued_stays_unnamed() {
    use super::{Decision, Work, forge};
    let (mut domain, env, mut out) = restored_change([
        forge::Stored::Change(change_row(Some(23))),
        forge::Stored::Names { task: 7, resources: change_names(None) },
    ]);
    let forge_env = skein_lib::Env { now: env.now, wall: env.wall, limits: env.limits.forge };
    out.push(forge::Request::WakeProcedure { task: 7 });
    let mut decision = Decision::new(&env.limits.journal);
    super::outputs(&mut domain, &env, &mut decision, &mut out);
    let Some(Work::Forge(event @ forge::Event::Rename { .. })) = domain.work.pop() else { panic!("queue pull watch") };
    // The release completes before the queued swap applies.
    forge::step(&mut domain.forge, &forge_env, forge::Event::Unnamed { task: 7 }, &mut out);
    assert_eq!(domain.forge.names(7), None);
    while let Some(request) = out.pop() {
        drop(request);
    }
    forge::step(&mut domain.forge, &forge_env, event, &mut out);
    assert_eq!(domain.forge.names(7), None, "the swap revives no watch");
    assert!(out.is_empty());
}

#[test]
fn final_desired_survives_full_projection_handoff_and_retries_at_the_normal_interval() {
    use super::{Work, forge};
    use skein_lib::{Env, Queue, Wall};
    use temper_engine_domain_forge_issues::{GoalView, Phase};
    let (mut domain, mut env) = two_forges();
    // The route needs the ordinary forge resource path, wider than the unrelated resume fixture.
    env.limits.authority.segments = 8;
    let goal = 7;
    let generation = 77;
    let view = GoalView {
        phase: Phase::Done,
        goal,
        repository: 1,
        title: "done".into(),
        goal_text: "done".into(),
        plan: Box::new([]),
        milestones: Box::new([]),
        finished: Some("done".into()),
    };
    let mut decision = crate::Decision::new(&env.limits.journal);
    let mut out = Queue::with_capacity(forge::max_out(&env.limits.forge));
    let provider = adopted(1).provider;
    let forge_env = Env { now: env.now, wall: env.wall, limits: env.limits.forge };
    for _ in 0..domain.forge_effects.capacity() {
        forge::step(
            &mut domain.forge,
            &forge_env,
            forge::Event::Project {
                entry: 11,
                repository: provider,
                view: view.clone(),
                closing_generation: Some(generation),
            },
            &mut out,
        );
        super::outputs(&mut domain, &env, &mut decision, &mut out);
        assert!(matches!(domain.work.pop(), Some(Work::Core(jig_domain_core::Event::EffectStart { .. }))));
        assert!(domain.work.is_empty());
    }
    assert_eq!(domain.forge_effects.len(), domain.forge_effects.capacity());
    forge::step(&mut domain.forge, &forge_env, forge::Event::ProjectDesired { entry: 11, goal }, &mut out);
    super::outputs(&mut domain, &env, &mut decision, &mut out);
    assert!(domain.work.is_empty(), "full admission schedules a timer without a same-decision retry");
    let accepted = domain.forge.issue(goal).unwrap().clone();
    assert_eq!(accepted.desired, Some(view));
    assert_eq!(accepted.closing_generation, Some(generation));
    assert_eq!(accepted.pending, None, "unkept candidate owns no writer or outbox entry");
    assert!(!accepted.failed, "transient full admission creates no failure debt");
    let due = Wall::from_nanos(env.wall.as_nanos() + env.limits.forge.issue_policy.interval.as_nanos());
    assert_eq!(domain.forge.next_wall_deadline(), Some(due));
    let owner = *domain.forge_effects.iter().next().unwrap().0;
    domain.forge_effects.remove(&owner);
    for _ in 0..goal {
        crate::fresh(&mut domain.core.counters, crate::Family::Task).unwrap();
    }
    assert_eq!(
        domain.core.restore_core(
            jig_domain_core::CoreRecord::Projection(jig_domain_core::ProjectionRecord::Goal {
                goal: jig_domain_core::ProjectionGoal {
                    number: goal,
                    project: 1,
                    priority: 1,
                    words: b"done".as_slice().into(),
                    phase: jig_domain_tasks::Phase::Waiting
                },
                pending: Box::new([]),
                changed: false,
                fed_at: None,
                closing_at: None,
            }),
            &crate::core_limits(&env.limits)
        ),
        jig_domain_core::Restored::Live
    );
    env.wall = due;
    forge::fire(&mut domain.forge, &Env { now: env.now, wall: env.wall, limits: env.limits.forge }, &mut out);
    assert!(out.iter().any(|request| matches!(request, forge::Request::ProjectAfter { goal: 7, when: None })));
    super::outputs(&mut domain, &env, &mut decision, &mut out);
    let Some(Work::Forge(forge::Event::ProjectDesired { entry, goal: retried })) = domain.work.pop() else {
        panic!("normal timer performs Begin before retrying accepted desired")
    };
    assert_eq!(retried, goal);
    assert!(entry > 0);
    assert_eq!(entry, domain.deployment().connector_rows);
    forge::step(
        &mut domain.forge,
        &Env { now: env.now, wall: env.wall, limits: env.limits.forge },
        forge::Event::ProjectDesired { entry, goal },
        &mut out,
    );
    super::outputs(&mut domain, &env, &mut decision, &mut out);
    assert!(matches!(domain.work.pop(), Some(Work::Core(jig_domain_core::Event::EffectStart {
        origin: jig_domain_core::EffectOrigin::Projection { goal: 7, source, entry: Some(kept) }, ..
    })) if source == entry && kept == entry));
    assert_eq!(domain.forge.issue(goal), Some(&accepted));
}
