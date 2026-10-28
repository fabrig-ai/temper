//! A chat's change runs through the root's deployment-funded landing task.
use super::*;
use skein_forge::api;
use temper_engine_domain_forge_change::State;

fn task_row(world: &World, task: u64) -> Option<&tasks::TaskRecord> {
    world.store.rows.values().find_map(|record| match record {
        Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Live(row) | tasks::Stored::Ended(row)))
            if row.number == task =>
        {
            Some(row.as_ref())
        }
        _ => None,
    })
}

fn produced_turn_change(landing_members: Option<u32>) -> (World, u64) {
    let mut world = World::configured_limits(
        true,
        true,
        false,
        1000,
        Gate::default(),
        None,
        StoryLimits { turn: true, landing_members, ..StoryLimits::default() },
    );
    world.adopt();
    produced_turn_change_in(world, true)
}

fn produced_turn_change_in(mut world: World, main_moves: bool) -> (World, u64) {
    world.send(protocol::hello_slots(Token::new(7), 2, Box::new([])));
    let ask = adoption_chat(&world);
    let chat = assigned_chat(&mut world, 92, ask);
    let engine::CallAnswer::Delegated(made) =
        change_tool::change(&mut world, &chat, 93, None, b"Small fix\nwith detail")
    else {
        panic!("change delegated")
    };
    let change = made[0].task;
    world.until(Until::SecondAssignment);
    let producer = world.assigned[1].clone();
    let branch = producer.workspace.repositories[0].push.as_ref().expect("producer branch").clone();
    assert!(branch.starts_with(b"temper/"));
    world.external(1, raw::Op::Git(raw::Git::Create { branch: branch.clone(), commit: 1 }));
    fake::advance(&mut world.fake, &world.fake_env, b"org/repo", &branch, b"file", b"fixed", 1)
        .expect("producer pushes");
    if main_moves {
        fake::advance(&mut world.fake, &world.fake_env, b"org/repo", b"main", b"other", b"main moved", 2)
            .expect("main moves under change");
    }
    let answer = world.protocol.answer(
        Token::new(7),
        producer.task,
        producer.attempt,
        5,
        tasks::End::Finished {
            result: tasks::TaskResult::Change {
                connector: 0,
                kind: 2,
                resource: forge_top::tag(forge_world::REPO.forge, forge_world::REPO.repository),
                words: b"pushed".as_slice().into(),
            },
            cancel_delegates: false,
        },
        engine::Parts::none(),
    );
    world.send(answer);
    (world, change)
}

#[test]
fn a_chats_change_lands_by_turn() {
    let (mut world, change) = produced_turn_change(None);
    let mut landing = None;
    for _ in 0..600 {
        world.tick();
        for record in world.store.rows.values() {
            if let Record::Forge { row } = record
                && let forge_top::Stored::Landing(row) = row.as_ref()
            {
                landing = Some(row.landing.task);
            }
        }
        if landing.is_some_and(|landing| {
            task_row(&world, landing).is_some_and(|row| matches!(row.phase, tasks::Phase::Ended(_)))
        }) && task_row(&world, change).is_some_and(|row| matches!(row.phase, tasks::Phase::Ended(_)))
        {
            break;
        }
    }
    let landing = landing.unwrap_or_else(|| {
        panic!(
            "landing made: {:?}",
            world.store.rows.values().filter(|record| matches!(record, Record::Forge { .. })).collect::<Vec<_>>()
        )
    });
    let row = task_row(&world, landing).expect("landing task");
    assert_eq!(row.executor, tasks::Executor::Procedure { connector: 0, code: 3 });
    assert_eq!(row.requester, tasks::Party::Deployment { project: 1 });
    assert_eq!(row.spec.words.as_ref(), b"Land Small fix");
    assert!(row.dependencies.is_empty());
    for kind in [3, 4, 6, 9, 12, 16, 17] {
        assert!(row.authority.grants.iter().any(|grant| grant.kind == kind), "landing granted kind {kind}");
    }
    assert!(!row.authority.grants.iter().any(|grant| grant.kind == 5 || grant.kind == 15));
    let tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Report { words })) = &row.phase else {
        panic!("landing done: {:?}; writes={:?}", row.phase, world.writes);
    };
    let report = words.clone();
    let change_row = task_row(&world, change).expect("change task");
    let tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Change { words, .. })) = &change_row.phase else {
        panic!("change done: {:?}", change_row.phase)
    };
    let change_result = words.clone();
    let raw::Answer::Commit(main) = world.external(1, raw::Op::Git(raw::Git::Fetch { want: raw::Want::Default }))
    else {
        panic!("main tip")
    };
    let main = translate::commit(main);
    let mut hex = String::new();
    for byte in &main[..20] {
        write!(&mut hex, "{byte:02x}").expect("commit hex");
    }
    assert_eq!(report.as_ref(), format!("org/repo {hex}\n").as_bytes());
    assert!(std::str::from_utf8(&change_result).expect("change result text").contains(&hex));
    assert_eq!(
        world
            .writes
            .iter()
            .filter(|write| matches!(write, api::Write::Update { style: api::UpdateStyle::Rebase, .. }))
            .count(),
        1
    );
    assert_eq!(
        world
            .writes
            .iter()
            .filter(|write| matches!(write, api::Write::Dispatch { workflow, .. } if workflow.as_ref() == b"gate.yml"))
            .count(),
        1
    );
    assert_eq!(world.writes.iter().filter(|write| matches!(write, api::Write::Status { commit, context, .. } if *commit == main && context.as_ref() == b"temper/gate/gate")).count(), 2);
    assert_eq!(world.writes.iter().filter(|write| matches!(write, api::Write::Merge { head, how: api::MergeHow::FastForwardOnly, .. } if *head == main)).count(), 1);
    assert_eq!(world.merges, 1);
}

#[test]
fn a_ready_change_waits_when_landing_room_is_full() {
    let (mut world, change) = produced_turn_change(Some(0));
    for _ in 0..300 {
        world.tick();
    }
    assert!(task_row(&world, change).is_some_and(|row| matches!(row.phase, tasks::Phase::Active(_))));
    let row = world
        .store
        .rows
        .values()
        .find_map(|record| match record {
            Record::Forge { row } => match row.as_ref() {
                forge_top::Stored::Change(row) if row.task == change => Some(row),
                _ => None,
            },
            Record::Core(_) => None,
        })
        .expect("change remains");
    assert_eq!(row.change.landing, None);
    assert!(matches!(row.change.state, temper_engine_domain_forge_change::State::Ready { .. }));
    assert!(world.store.rows.values().all(
        |record| !matches!(record, Record::Forge { row } if matches!(row.as_ref(), forge_top::Stored::Landing(_)))
    ));
    assert_eq!(world.merges, 0);
}

/// The real root gives the change's result only after its turn's tier verdict.
#[test]
fn a_change_landed_by_turn_verifies_until_its_tier_clears_it() {
    // The existing tier story allows three 600-tick settling windows for a 90 s workflow.
    const TIER_TICKS: u32 = 1800;
    const LANDING_TICKS: u32 = 600;
    let mut world = World::configured_limits(
        true,
        true,
        false,
        1000,
        Gate::default(),
        None,
        StoryLimits { turn: true, tier: true, ..StoryLimits::default() },
    );
    world.adopt();
    for _ in 0..TIER_TICKS {
        world.tick();
        if world.store.rows.values().any(|record| {
            matches!(record,
            Record::Forge { row } if matches!(row.as_ref(), forge_top::Stored::Tier(row)
                if row.record.judged.is_some() && row.record.run.is_none()))
        }) {
            break;
        }
    }
    assert!(
        world.store.rows.values().any(|record| matches!(record,
        Record::Forge { row } if matches!(row.as_ref(), forge_top::Stored::Tier(row)
            if row.record.judged.is_some() && row.record.run.is_none()))),
        "baseline green"
    );
    // The 90 s baseline outlives the fixture's 60 s sign-in.
    world.signed_in = None;
    world.sign_in();
    let (mut world, change) = produced_turn_change_in(world, false);
    let key = Key::Forge(forge_top::Key::Change(change));
    let mut merge = None;
    for _ in 0..LANDING_TICKS {
        world.tick();
        if let Some(Record::Forge { row }) = world.store.rows.get(&key)
            && let forge_top::Stored::Change(row) = row.as_ref()
            && let State::Landed { merge: commit } = row.change.state
        {
            merge = Some(commit);
            break;
        }
    }
    let merge = merge.expect("turn's merge is verifying");
    assert_eq!(world.merges, 1);
    assert!(!matches!(task_row(&world, change).expect("change task").phase, tasks::Phase::Ended(_)));
    let Some(Record::Forge { row }) = world
        .store
        .rows
        .get(&Key::Forge(forge_top::Key::Verification { repository: forge_world::REPO, commit: merge }))
    else {
        panic!("merge verification");
    };
    let forge_top::Stored::Verification(row) = row.as_ref() else {
        panic!("verification row");
    };
    assert_eq!(row.changes.as_ref(), &[change], "verdict belongs to the member's change");
    assert_eq!(row.tiers.len(), 1);
    assert!(row.tiers[0].verdict.is_none());
    for _ in 0..TIER_TICKS {
        world.tick();
        if task_row(&world, change).is_some_and(|row| matches!(row.phase, tasks::Phase::Ended(_))) {
            break;
        }
    }
    let Some(Record::Forge { row }) = world.store.rows.get(&key) else {
        panic!("cleared change");
    };
    let forge_top::Stored::Change(row) = row.as_ref() else {
        panic!("change row");
    };
    assert_eq!(row.change.state, State::Cleared { merge });
    assert!(matches!(
        task_row(&world, change).expect("change result").phase,
        tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Change { .. }))
    ));
}

const COLD_TICKS: u32 = 2400; // Four 600-tick windows cover a 90 s baseline and its next run.
const COLD_SEED: u64 = 71; // The root forge world's deployment/store seed.
const TIER_CUTS: [&str; 15] = [
    "baseline keyless",
    "baseline keyed",
    "directory walk",
    "pending keyless",
    "pending keyed",
    "branch being made",
    "dispatched",
    "followed",
    "status being posted",
    "comment being posted",
    "verification verifying",
    "verification cleared",
    "change landed",
    "change cleared",
    "tier receipt",
];

fn restart_tier_world() -> World {
    let mut world = World::configured_limits(
        true,
        true,
        false,
        1000,
        Gate::default(),
        None,
        StoryLimits { turn: true, tier: true, tier_tree: true, ..StoryLimits::default() },
    );
    for (path, body) in [(b"docs/guide.md".as_slice(), b"guide".as_slice()), (b"src/lib.rs", b"source")] {
        fake::advance(&mut world.fake, &world.fake_env, b"org/repo", b"main", path, body, 1).expect("baseline tree");
    }
    world
}

fn cold_tier(world: &mut World, at: &[&str]) {
    world.store.crash(COLD_SEED);
    world.restart(true, true);
    for _ in 0..COLD_TICKS {
        world.tick();
        if world.root.ready() {
            return;
        }
        assert!(
            world.root.restart_failure().is_none(),
            "cold cut {at:?}: {:?}; forge {:?}",
            world.root.restart_failure(),
            world.root.forge_restore_failure()
        );
    }
    panic!("cold admission at {at:?}: {:?}", world.root.restart_step());
}

fn tier_row(world: &World) -> Option<&forge_top::TierRow> {
    world.store.rows.values().find_map(|record| match record {
        Record::Forge { row } => match row.as_ref() {
            forge_top::Stored::Tier(row) => Some(row),
            _ => None,
        },
        Record::Core(_) => None,
    })
}

fn tier_cuts(world: &World) -> Vec<&'static str> {
    let mut found = Vec::new();
    if let Some(row) = tier_row(world) {
        if let Some(baseline) = row.baseline {
            found.push(if baseline.key.is_some() { "baseline keyed" } else { "baseline keyless" });
        }
        for landing in &row.pending {
            found.push(if landing.key.is_some() { "pending keyed" } else { "pending keyless" });
        }
        if let forge_top::TierBranch::Pending { .. } = row.branch {
            found.push("branch being made");
        }
        if let Some(flight) = &row.flight {
            if flight.start.is_some() {
                found.push("dispatched");
            }
            if flight.next.is_some() {
                found.push("followed");
            }
        }
        if let Some(result) = &row.result
            && let forge_top::TierBranch::Pending { .. } = result.status
        {
            found.push("status being posted");
        }
        if let Some(result) = &row.result
            && result.comments.iter().any(|comment| matches!(comment.state, forge_top::TierBranch::Pending { .. }))
        {
            found.push("comment being posted");
        }
        if let Some(baseline) = row.baseline
            && world.pending.values().any(|op| {
                matches!(op,
                api::Op::Read(api::Read::Tree { object, .. }) if *object != baseline.commit)
            })
        {
            found.push("directory walk");
        }
    }
    for record in world.store.rows.values() {
        if let Record::Forge { row } = record {
            match row.as_ref() {
                forge_top::Stored::Verification(row) => {
                    found.push(if row.tiers.iter().all(|tier| tier.verdict.is_some()) {
                        "verification cleared"
                    } else {
                        "verification verifying"
                    });
                }
                forge_top::Stored::Change(row) => match row.change.state {
                    State::Landed { .. } => found.push("change landed"),
                    State::Cleared { .. } => found.push("change cleared"),
                    _ => {}
                },
                forge_top::Stored::Receipt { key, .. } if matches!(key.check, forge_top::Check::Tier(_)) => {
                    found.push("tier receipt");
                }
                _ => {}
            }
        }
    }
    found.sort_unstable();
    found.dedup();
    found
}

fn cut_new_tier_records(world: &mut World, seen: &mut std::collections::BTreeSet<&'static str>) {
    let fresh: Vec<_> = tier_cuts(world).into_iter().filter(|at| !seen.contains(at)).collect();
    if fresh.is_empty() {
        return;
    }
    if fresh.contains(&"baseline keyless") && !fresh.contains(&"directory walk") {
        return;
    }
    seen.extend(fresh.iter().copied());
    cold_tier(world, &fresh);
}

fn green_baseline(world: &World) -> bool {
    tier_row(world).is_some_and(|row| row.record.judged.is_some() && row.record.run.is_none())
}

#[test]
fn a_cold_restart_reads_back_every_tier_record() {
    let mut world = restart_tier_world();
    world.adopt();
    let mut seen = std::collections::BTreeSet::new();
    // The keyed baseline is an intermediate writer output in the same atomic
    // decision as starting its run. Exercise that valid stored shape separately,
    // using the writer's documented commit fallback, before a tier task exists.
    for _ in 0..COLD_TICKS {
        world.tick();
        if let Some(row) = tier_row(&world)
            && row.baseline.is_some()
        {
            let mut keyed = row.clone();
            let baseline = keyed.baseline.as_mut().expect("baseline entry");
            baseline.key = Some(baseline.commit);
            let mut probe = restart_tier_world();
            probe.store = world.store.fork();
            probe.store.pending.clear();
            probe.store.seed(Box::new([Record::Forge { row: Box::new(forge_top::Stored::Tier(keyed)) }]), Box::new([]));
            cold_tier(&mut probe, &["baseline keyed"]);
            for _ in 0..COLD_TICKS {
                probe.tick();
                if green_baseline(&probe) {
                    break;
                }
            }
            assert!(green_baseline(&probe), "keyed baseline goes on to green");
            seen.insert("baseline keyed");
            break;
        }
    }
    for _ in 0..COLD_TICKS {
        cut_new_tier_records(&mut world, &mut seen);
        if seen.contains("followed") {
            break;
        }
        world.tick();
    }
    world.signed_in = None;
    world.sign_in();
    let (mut world, change) = produced_turn_change_in(world, false);
    for _ in 0..COLD_TICKS {
        cut_new_tier_records(&mut world, &mut seen);
        world.tick();
        if task_row(&world, change).is_some_and(|row| matches!(row.phase, tasks::Phase::Ended(_)))
            && seen.len() == TIER_CUTS.len()
            && green_baseline(&world)
        {
            break;
        }
    }
    assert_eq!(seen, TIER_CUTS.into_iter().collect(), "every requested durable cut was exercised");
    let restarted = cleared_tier_end(&world, change);
    let mut control = restart_tier_world();
    control.adopt();
    for _ in 0..COLD_TICKS {
        control.tick();
        if tier_row(&control).is_some_and(|row| row.flight.as_ref().is_some_and(|flight| flight.next.is_some())) {
            break;
        }
    }
    let (mut control, control_change) = produced_turn_change_in(control, false);
    for _ in 0..COLD_TICKS {
        control.tick();
        if task_row(&control, control_change).is_some_and(|row| matches!(row.phase, tasks::Phase::Ended(_)))
            && green_baseline(&control)
        {
            break;
        }
    }
    assert_eq!(restarted, cleared_tier_end(&control, control_change), "cold cuts reach the uninterrupted end");
}

fn cleared_tier_end(world: &World, change: u64) -> (State, api::Commit) {
    assert!(world.root.ready());
    assert_eq!(world.merges, 1);
    let Record::Forge { row } =
        world.store.rows.get(&Key::Forge(forge_top::Key::Change(change))).expect("change survives cold admission")
    else {
        panic!("change row")
    };
    let forge_top::Stored::Change(row) = row.as_ref() else { panic!("change row") };
    assert!(matches!(row.change.state, State::Cleared { .. }));
    assert!(matches!(
        task_row(world, change).expect("change result").phase,
        tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Change { .. }))
    ));
    assert!(green_baseline(world), "baseline and the landing's run both clear");
    (row.change.state.clone(), tier_row(world).expect("repository tier").record.judged.expect("judged tip").tip)
}
