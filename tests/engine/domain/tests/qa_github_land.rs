//! Shipped forge wait and required-brief bounds through the requester-backed root.
use jig_domain_tasks as tasks;
use skein_fake_forge_domain::{self as fake, api as raw};
use skein_lib::{Duration, Time, Token, Wall};
use temper_engine_domain as engine;
use temper_engine_domain_forge as forge;
use temper_engine_domain_forge_change as change;
use temper_engine_domain_world::forge_routes::{Change, Gate, StoryLimits, World, change_world_in};
use temper_engine_forge_world::REPO;

// domain/forge.md 8.4 and the shipped limits in the QA brief.
const STALL: Duration = Duration::from_secs(1800);
const GATHER: Duration = Duration::from_secs(30);
const READ_DELAY: Duration = Duration::from_secs(31);
const TICK: Duration = Duration::from_millis(100);
const CHANNEL: Token = Token::new(7);

fn change_row(world: &World) -> &forge::ChangeRow {
    world
        .store
        .rows
        .values()
        .find_map(|row| match row {
            engine::Record::Forge { row } => {
                if let forge::Stored::Change(row) = row.as_ref() {
                    Some(row)
                } else {
                    None
                }
            }
            engine::Record::Core(_) | engine::Record::Delivery { .. } => None,
        })
        .expect("requester-backed change")
}

fn task(world: &World, number: u64) -> &tasks::TaskRecord {
    let key = engine::Key::Core(jig_domain_core::Key::Tasks(tasks::Key::Live(number)));
    let Some(engine::Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Live(row)))) =
        world.store.rows.get(&key)
    else {
        panic!("live task {number}")
    };
    row
}

fn pushed(world: &mut World, producer: &engine::Assignment, branch: &[u8]) {
    assert!(matches!(
        world.external(2, raw::Op::Git(raw::Git::Create { branch: branch.into(), commit: 1 })),
        raw::Answer::Branch(raw::Created::Created)
    ));
    fake::advance(&mut world.fake, &world.fake_env, b"org/repo", branch, b"file", b"first", 1)
        .expect("producer pushed its branch");
    let event = world.protocol.answer(
        CHANNEL,
        producer.task,
        producer.attempt,
        5,
        tasks::End::Finished {
            result: tasks::TaskResult::Change {
                connector: 0,
                kind: 2,
                resource: forge::tag(REPO.forge, REPO.repository),
                words: b"pushed".as_slice().into(),
            },
            cancel_delegates: false,
        },
        engine::Parts::none(),
    );
    world.send(event);
}

fn silent_change() -> (World, u64) {
    let mut world = World::configured_limits(
        true,
        true,
        true,
        1000,
        Gate::default(),
        None,
        StoryLimits { stall: Some(STALL), ..StoryLimits::default() },
    );
    // Boundary snapshots drain reads without advancing the clock.
    world.fake_env.limits.latency_min = Duration::ZERO;
    world.fake_env.limits.latency_max = Duration::ZERO;
    let (mut world, requester, producer, branch) = change_world_in(world, Change::default());
    pushed(&mut world, &producer, &branch);
    for _ in 0..200 {
        world.tick();
        if matches!(change_row(&world).change.state, change::State::Checking { .. }) {
            return (world, requester.task);
        }
    }
    panic!("silent change never reached Checking: {:?}", change_row(&world));
}

fn at(world: &mut World, when: Wall) {
    assert!(world.env.wall <= when, "clock must move forwards");
    // Snapshot settlement cannot advance time to deliver the fake's default
    // 10-50 ms transport latency. Make newly issued reads answer at this
    // snapshot; Q6 still retains its selected required reply explicitly.
    world.fake_env.limits.latency_min = Duration::ZERO;
    world.fake_env.limits.latency_max = Duration::ZERO;
    // World::tick advances 100 ms. Drain continuations at the chosen snapshot
    // by giving each tick the same starting time; no step sees time go backwards.
    for _ in 0..200 {
        world.env.now = Time::from_nanos(when.as_nanos() - TICK.as_nanos());
        world.tick();
    }
    assert_eq!(world.env.wall, when);
}

#[test]
fn silent_ci_at_shipped_stall_boundary() {
    // QA: domain/forge.md 8.2, 8.4; Q2. Exercise the actual procedure's wake.
    let (mut world, _) = silent_change();
    let since = change_row(&world).change.since.as_nanos();
    let deadline = since + STALL.as_nanos();
    at(&mut world, Wall::from_nanos(deadline - TICK.as_nanos()));
    assert!(
        matches!(change_row(&world).change.state, change::State::Checking { .. }),
        "QA: silent CI must not stall before 1800 s: {:?}",
        change_row(&world)
    );
    at(&mut world, Wall::from_nanos(deadline));
    assert!(
        matches!(change_row(&world).change.state, change::State::Held { why: change::Hold::Stalled, .. }),
        "QA: silent CI must be Held(Stalled) at 1800 s: {:?}",
        change_row(&world)
    );
    at(&mut world, Wall::from_nanos(deadline + TICK.as_nanos()));
    assert!(
        matches!(change_row(&world).change.state, change::State::Held { why: change::Hold::Stalled, .. }),
        "QA: silent CI must remain held just after 1800 s: {:?}",
        change_row(&world)
    );
    assert_eq!(world.merges, 0, "QA: silent CI never permits a merge");
}

fn requester_heard_hold(world: &World, requester: u64, change: u64) -> bool {
    task(world, requester)
        .inbox
        .iter()
        .chain(
            world
                .assigned
                .iter()
                .filter(|assignment| assignment.task == requester)
                .flat_map(|assignment| &assignment.messages),
        )
        .any(|word| matches!(word.kind, tasks::MessageKind::Escalation { task, .. } if task == change))
}

#[test]
fn stalled_change_notifies_its_requester() {
    // QA: domain/forge.md 8.4; Q2. The requester is the live chat that delegated.
    let (mut world, requester) = silent_change();
    // Finish the scripted chat's active run as parked, as a real requester
    // does after delegating; its next assignment can then carry the notice.
    let event = world.protocol.answer(
        CHANNEL,
        requester,
        world.assigned[0].attempt,
        0,
        tasks::End::Parked,
        engine::Parts::none(),
    );
    world.send(event);
    world.until(temper_engine_domain_world::forge_routes::Until::Ready);
    let change = change_row(&world).task;
    let deadline = change_row(&world).change.since.as_nanos() + STALL.as_nanos();
    // Jump to the stall, then advance through any one-second Busy retries.
    // Unlike the boundary snapshots, this story must settle the requester notice.
    world.env.now = Time::from_nanos(deadline - TICK.as_nanos());
    for _ in 0..200 {
        world.tick();
        if matches!(task(&world, change).phase, tasks::Phase::Held { .. })
            && requester_heard_hold(&world, requester, change)
        {
            break;
        }
    }
    assert!(
        matches!(task(&world, change).phase, tasks::Phase::Held { .. }),
        "QA: the core procedure task must be held too: {:?}; forge: {:?}; pending: {:?}; events: {:?}",
        task(&world, change),
        change_row(&world),
        world.pending,
        world.events
    );
    assert!(
        matches!(change_row(&world).change.state, change::State::Held { why: change::Hold::Stalled, .. }),
        "QA: the hold card must carry the stall reason: {:?}",
        change_row(&world)
    );
    // The requester hears generically; the reason is on the hold card. Its
    // woken assignment may already have consumed the notice from its inbox.
    assert!(
        requester_heard_hold(&world, requester, change),
        "QA: the requesting chat must receive its delegate's hold notice: {:?}; assignments: {:?}",
        task(&world, requester),
        world.assigned
    );
    assert_eq!(world.merges, 0);
}

#[test]
fn slow_required_brief_reaches_shipped_gather_limit() {
    // QA: domain/forge.md 8.4; domain/testing.md 4.2; Q6. A required
    // section never becomes a successful partial brief when gather expires.
    // The cited design does not prescribe fail vs hold vs retry at expiry.
    let mut brief = World::configured(true, true).env.limits.brief;
    brief.gather = GATHER;
    let world = World::configured_brief(true, true, false, 0, Gate::default(), None, Some(brief));
    let (mut world, _requester, producer, branch) = change_world_in(world, Change::default());
    pushed(&mut world, &producer, &branch);
    // The public event queue is the scripted transport. Retain one required
    // Job terminal for 31 s; the forge and all unrelated replies keep running.
    let (call, started) = loop {
        world.tick();
        if let Some((call, _)) = world
            .pending
            .iter()
            .find(|(_, op)| matches!(op, skein_forge::api::Op::Read(skein_forge::api::Read::Job { .. })))
        {
            break (*call, world.env.wall);
        }
        assert!(world.assigned.len() <= 2, "repair must still be gathering");
        assert!(world.env.wall.as_nanos() < Duration::from_secs(120).as_nanos(), "failed CI must start a repair brief");
    };
    let mut delayed = None;
    for _ in 0..200 {
        world.tick();
        if let Some(position) = world.events.iter().position(|event| {
            matches!(event,
            engine::Event::Forge(event) if matches!(event.as_ref(),
                forge::Event::Client(skein_forge_client::Event::Answered { call: answered, .. })
                    if *answered == call))
        }) {
            delayed = world.events.remove(position);
            break;
        }
    }
    let delayed = delayed.expect("required Job terminal retained before root delivery");
    assert_eq!(world.env.limits.brief.gather, GATHER);
    for millis in [29_900, 30_000, 30_100] {
        at(&mut world, Wall::from_nanos(started.as_nanos() + Duration::from_millis(millis).as_nanos()));
        assert_eq!(
            world.assigned.len(),
            2,
            "QA: an unanswered required section cannot dispatch a partial repair brief at {millis} ms: {:?}",
            world.assigned.iter().skip(2).collect::<Vec<_>>()
        );
        assert_eq!(world.merges, 0, "failed CI cannot land while its repair gathers");
    }
    at(&mut world, Wall::from_nanos(started.as_nanos() + READ_DELAY.as_nanos()));
    world.send(delayed);
    let now = world.env.wall;
    at(&mut world, now);
    assert_eq!(world.merges, 0, "late brief input cannot land unrepaired CI");
}
