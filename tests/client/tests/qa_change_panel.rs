//! Adversarial checks of the forge panel through the joined client world.
use jig_client_world::harness::{ClientId, Settings, Until, World};
use skein_lib::{Duration, Token};
use temper_client_domain::forge::panel::State;
use temper_client_protocol_wire::v1 as wire;
use temper_client_world::{app::Temper, fixtures, front::Joint};
use temper_engine_domain as engine;

type ClientWorld = World<Temper, Joint>;

fn change_world() -> (ClientWorld, u64) {
    let limits = fixtures::domain_limits();
    let mut front = Joint::with_change_landing(fixtures::SEED, &limits);
    front.engine.forge =
        Some(temper_engine_domain_world::forge_stand::ForgeStand::with_checks(fixtures::REPOSITORIES, &[b"build"]));
    fixtures::adopt_change_repository(&mut front);
    front.engine.forge.as_mut().expect("fake forge").push_branch(7, b"temper/qa");
    let task = fixtures::goal_with_pushed_change(&mut front, b"temper/qa");
    let world = World::new(
        Settings {
            seed: fixtures::SEED,
            words: temper_client_view::words::jig_language(),
            heap_bytes_per_client: 64 * 1024 * 1024,
            ..Settings::default()
        },
        front,
    );
    (world, task)
}

fn tick(world: &mut ClientWorld) {
    let until = world.now().saturating_add(Duration::from_millis(100));
    let outcome = world.run(Until::Time(until));
    assert!(matches!(outcome.verdict, skein_world::domain::Verdict::Passed), "client world: {outcome:?}");
}

fn panel(world: &ClientWorld, client: ClientId) -> &wire::ChangePanel {
    let state = &world.root(client).person().forge().panel().as_ref().expect("task page's panel slot").state;
    let State::Shown(panel) = state else {
        panic!("design client/forge.md 9.2 promises the known change's panel, got {state:?}")
    };
    panel
}

fn review(world: &mut ClientWorld, client: ClientId) -> u64 {
    for _ in 0..100 {
        tick(world);
        if let Some(panel) = &world.root(client).person().forge().panel()
            && let State::Shown(value) = &panel.state
            && let Some(task) = value.runs()
        {
            return *task;
        }
    }
    panic!(
        "the pushed change must reach its blocking review; last slot: {:?}",
        world.root(client).person().forge().panel()
    );
}

// client/forge.md, 9.2: the whole document carries its one running gate and CI at its head.
#[test]
fn pending_review_panel_names_the_delegate_and_current_head() {
    let (mut world, task) = change_world();
    let client = world.client(fixtures::PARTY, fixtures::task_page(task));
    let delegate = review(&mut world, client);
    let value = panel(&world, client);
    assert_eq!(value.task(), task);
    assert_eq!(value.runs(), &Some(delegate));
    assert_eq!(value.step(), &wire::Step::Gating);
    assert_eq!(value.ci(), &wire::Ci::Passed);
    assert!(value.head().is_some());
    assert_eq!(value.gates().len(), 1);
    assert_eq!(
        value.gates().as_slice()[0].verdict(),
        &wire::Verdict::Pending,
        "design client/README.md 4.6: a running gate must report pending at its current head, got {value:?}"
    );
    assert!(value.gates().as_slice()[0].blocks());
    assert_eq!(value.branch(), b"temper/qa");
    assert_eq!(value.landing(), b"main");
}

// client/forge.md, 9.2: a change ended before its page opens keeps its last panel.
#[test]
fn late_open_landed_change_keeps_its_final_panel() {
    let (mut world, task) = change_world();
    let client = world.client(fixtures::PARTY, fixtures::inbox());
    let mut delegate = None;
    for _ in 0..100 {
        tick(&mut world);
        let front = world.front();
        delegate = front.engine.store.rows.keys().find_map(|key| {
            if let engine::Key::Core(jig_domain_core::Key::Tasks(jig_domain_tasks::Key::Live(number))) = key
                && *number != task
                && front.hosted_attempt(*number).is_some()
            {
                Some(*number)
            } else {
                None
            }
        });
        if delegate.is_some() {
            break;
        }
    }
    finish_review(
        &mut world,
        delegate.expect("blocking review task"),
        jig_domain_tasks::TaskResult::Verdict { code: 1, fields: Box::new([]), words: b"approved".as_slice().into() },
    );
    for _ in 0..150 {
        tick(&mut world);
        if world.front().engine.task(task).is_none() {
            break;
        }
    }
    assert!(
        world.front().engine.task(task).is_none(),
        "setup: the change must end before its task page is ever opened"
    );
    assert!(world.root(client).person().forge().panel().is_none());
    world.go(client, fixtures::task_page(task));
    for _ in 0..20 {
        tick(&mut world);
    }
    let value = panel(&world, client);
    let wire::Step::Landed(landed) = value.step() else {
        panic!("design client/forge.md 9.2: a page first opened after landing must receive its final panel: {value:?}");
    };
    let merge = *landed.merge();
    let number = value.pull().as_ref().expect("retained pull").number();
    let stand = world.front().engine.forge.as_mut().expect("fake forge");
    let skein_forge::api::Answer::Pull(pull) =
        stand.read(skein_forge::api::Repository { forge: 1, repository: 7 }, skein_forge::api::Read::Pull { number })
    else {
        panic!("landed pull on fake forge")
    };
    assert_eq!(
        pull.merged,
        Some(merge),
        "design client/forge.md 9.2: a late-open panel must retain the forge's merge commit"
    );
    assert!(world.front().watching_change(client, task));
}

fn finish_review(world: &mut ClientWorld, task: u64, result: jig_domain_tasks::TaskResult) {
    let attempt = world.front().hosted_attempt(task).expect("review attempt");
    world.front().engine.answer(
        Token::new(7),
        task,
        attempt,
        0,
        jig_domain_tasks::End::Finished { result, cancel_delegates: false },
        engine::Parts::none(),
    );
}

// client/forge.md, 9.2 and domain/forge.md, 8.5: a closed pull holds at its current step.
#[test]
fn externally_closed_pull_holds_the_panel_at_gating() {
    let (mut world, task) = change_world();
    let client = world.client(fixtures::PARTY, fixtures::task_page(task));
    review(&mut world, client);
    let number = panel(&world, client).pull().as_ref().expect("open pull").number();
    let limits = world.front().engine.env.limits.forge.client;
    let stand = world.front().engine.forge.as_mut().expect("fake forge");
    stand.call(
        Token::new(99_001),
        skein_forge::api::Repository { forge: 1, repository: 7 },
        skein_forge::api::Op::Write(skein_forge::api::Write::Close { number }),
        &limits,
    );
    assert_eq!(stand.replies().len(), 1, "external close must complete");
    for _ in 0..150 {
        tick(&mut world);
    }
    let value = panel(&world, client);
    assert_eq!(
        value.hold(),
        &Some(wire::Hold::Closed),
        "design client/forge.md 9.2: an externally closed pull's hold must be delivered, got {value:?}"
    );
    assert_eq!(
        value.step(),
        &wire::Step::Gating,
        "design client/forge.md 9.2: a held panel retains the step it was held at"
    );
    assert_eq!(value.ci(), &wire::Ci::Passed);
}

// client/forge.md, 9.2 and domain/forge.md, 8.4: a blocking rejection names its gate as repair reason.
#[test]
fn rejected_review_panel_names_the_gate_repair_reason() {
    let (mut world, task) = change_world();
    let client = world.client(fixtures::PARTY, fixtures::task_page(task));
    let delegate = review(&mut world, client);
    let gate = panel(&world, client).gates().as_slice()[0].number();
    finish_review(
        &mut world,
        delegate,
        jig_domain_tasks::TaskResult::Verdict {
            code: 2,
            fields: Box::new([]),
            words: b"repair the diff".as_slice().into(),
        },
    );
    for _ in 0..150 {
        tick(&mut world);
    }
    let value = panel(&world, client);
    let wire::Step::Repairing(repair) = value.step() else {
        panic!("design domain/forge.md 8.4: blocking review changes require a repair, got {value:?}");
    };
    let wire::Reason::Gate(reason) = repair.reason() else {
        panic!("design client/forge.md 9.2: a gate repair must carry that gate's number, got {repair:?}");
    };
    assert_eq!(reason.gate(), gate);
    assert_eq!(value.repairs().used(), 1);
    assert_eq!(value.gates().as_slice()[0].verdict(), &wire::Verdict::Changes);
}

// client/forge.md, 9.2: a cancelled change still has its panel when first opened afterwards.
#[test]
#[ignore = "needs: client landing fixture with an admitted Cancel action on a procedure task"]
fn late_open_cancelled_change_keeps_its_last_panel() {
    use skein_fake_person::{Find, Step};
    use skein_ui::Role;
    let (mut world, task) = change_world();
    let client = world.client(fixtures::PARTY, fixtures::task_page(task));
    review(&mut world, client);
    world.person(
        client,
        [
            Step::Press { find: Find::exactly_named(Role::Button, b"Cancel") },
            Step::See { find: Find::exactly_named(Role::Button, b"Cancel task"), within: fixtures::WITHIN },
            Step::Press { find: Find::exactly_named(Role::Button, b"Cancel task") },
        ]
        .into_iter()
        .collect(),
    );
    for _ in 0..150 {
        tick(&mut world);
    }
    let expected = panel(&world, client).clone();
    let ended = world.front().engine.task(task).is_none();
    assert!(ended, "setup: the cancelled change must finish closing");
    world.go(client, fixtures::inbox());
    for _ in 0..5 {
        tick(&mut world);
    }
    world.go(client, fixtures::task_page(task));
    let late = client;
    for _ in 0..150 {
        tick(&mut world);
    }
    assert_eq!(
        panel(&world, late),
        &expected,
        "design client/forge.md 9.2: a cancelled change's retained panel must be delivered when its page opens"
    );
}

// client/forge.md, 9.2: leaving the task page closes its panel's watch, even during a gate.
#[test]
fn leaving_a_live_change_page_releases_its_watch() {
    let (mut world, task) = change_world();
    let client = world.client(fixtures::PARTY, fixtures::task_page(task));
    review(&mut world, client);
    assert!(world.front().watching_change(client, task));
    world.go(client, fixtures::inbox());
    for _ in 0..10 {
        tick(&mut world);
    }
    assert!(
        !world.front().watching_change(client, task),
        "design client/forge.md 9.2: a panel is watched while its task page is open"
    );
    assert!(world.root(client).person().forge().panel().is_none());
    world.go(client, fixtures::task_page(task));
    review(&mut world, client);
    assert!(world.front().watching_change(client, task));
    assert_eq!(panel(&world, client).step(), &wire::Step::Gating);
}

// domain/forge.md, 8.8 and 5.3: CI changes only when a step reads it, and failed CI names its repair.
#[test]
fn ci_failure_is_read_into_the_panel_and_names_its_repair() {
    let (mut world, task) = change_world();
    let client = world.client(fixtures::PARTY, fixtures::task_page(task));
    review(&mut world, client);
    let head = panel(&world, client).head().expect("pushed head");
    let limits = world.front().engine.env.limits.forge.client;
    let stand = world.front().engine.forge.as_mut().expect("fake forge");
    stand.call(
        Token::new(99_002),
        skein_forge::api::Repository { forge: 1, repository: 7 },
        skein_forge::api::Op::Write(skein_forge::api::Write::Status {
            commit: head,
            context: b"build".as_slice().into(),
            check: skein_forge::api::Check::Failed,
            description: Box::new([]),
            link: Box::new([]),
        }),
        &limits,
    );
    assert_eq!(stand.replies().len(), 1, "external CI status must complete");
    assert_eq!(
        panel(&world, client).ci(),
        &wire::Ci::Passed,
        "design domain/forge.md 8.8: the panel keeps the last step's CI, until another step reads it"
    );
    for _ in 0..150 {
        tick(&mut world);
    }
    let value = panel(&world, client);
    assert_eq!(
        value.ci(),
        &wire::Ci::Failed,
        "design domain/forge.md 8.8: the next step's changed CI verdict must reach the watched panel: {value:?}"
    );
    let wire::Step::Repairing(repair) = value.step() else {
        panic!("design domain/forge.md 8.4: failed CI must enter repair: {value:?}");
    };
    assert_eq!(repair.reason(), &wire::Reason::Ci, "design client/forge.md 9.2: a CI repair must retain its CI reason");
    assert_eq!(value.repairs().used(), 1);
}
