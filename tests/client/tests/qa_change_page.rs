//! QA1: a real procedure's ended children through the engine and the person.
use jig_client_world::harness::{ClientId, Settings, Until, World};
use skein_fake_person::{Find, Step, Target};
use skein_lib::Token;
use skein_ui::Role;
use temper_client_domain::forge::panel::State;
use temper_client_protocol_wire::v1 as w;
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
fn see(world: &mut ClientWorld, client: ClientId, find: Find) {
    world.person(client, vec![Step::See { find, within: fixtures::WITHIN }]);
    let outcome = world.run(Until::Done);
    assert!(matches!(outcome.verdict, skein_world::domain::Verdict::Passed), "person's observation: {outcome:?}");
}
fn panel(world: &ClientWorld, client: ClientId) -> &w::ChangePanel {
    let panel = world.root(client).person().forge().panel().expect("panel slot");
    let State::Shown(value) = &panel.state else { panic!("known change must show its panel: {panel:?}") };
    value
}
fn gate_task(front: &Joint, goal: u64) -> Option<u64> {
    front.engine.store.rows.values().find_map(|record| {
        if let engine::Record::Core(jig_domain_core::Record::Tasks(jig_domain_tasks::Stored::Live(row))) = record
            && row.root == goal
            && row.number != goal
            && front.hosted_attempt(row.number) == Some(row.attempt)
        {
            Some(row.number)
        } else {
            None
        }
    })
}
fn progress(world: &mut ClientWorld) {
    let driver = &world.front().engine;
    assert!(driver.root.next_deadline(&driver.env).is_some(), "engine has a deadline while the story needs progress");
    let outcome = world.run(Until::Steps(2));
    assert!(
        matches!(outcome.verdict, skein_world::domain::Verdict::Passed | skein_world::domain::Verdict::Open { .. }),
        "engine progress: {outcome:?}"
    );
}
fn review(world: &mut ClientWorld, client: ClientId, goal: u64) -> u64 {
    let assigned = loop {
        if let Some(task) = gate_task(world.front(), goal) {
            break task;
        }
        progress(world);
    };
    see(world, client, Find::text(b"Waiting for its gates at its head."));
    let task = panel(world, client).runs().expect("gate delegate");
    assert_eq!(task, assigned, "panel names the assigned review");
    see(world, client, fixtures::plan_item(task, b"Running"));
    task
}
fn finish_review(world: &mut ClientWorld, task: u64) {
    let attempt = world.front().hosted_attempt(task).expect("hosted review");
    world.front().engine.answer(
        Token::new(7),
        task,
        attempt,
        0,
        jig_domain_tasks::End::Finished {
            result: jig_domain_tasks::TaskResult::Verdict {
                code: 1,
                fields: Box::new([]),
                words: b"QA approved".as_slice().into(),
            },
            cancel_delegates: false,
        },
        engine::Parts::none(),
    );
}
fn its_tasks(find: Find) -> Find {
    find.within(Box::new([Target::ExactRole { role: Role::Region, name: b"Its tasks".as_slice().into() }]))
}
fn text(world: &ClientWorld, client: ClientId) -> String {
    let mut value = String::new();
    for node in world.tree(client).nodes() {
        for run in &node.text {
            value.push_str(core::str::from_utf8(&run.text).expect("visible UTF-8"));
        }
    }
    value
}

// client/README.md 5.10: "Its ended ones are 5.8's ended rows, read once from
// the engine" and "read again when its tree or its document changes";
// their state is their ending.
#[test]
fn a_review_stays_in_its_tasks_after_it_leaves_the_live_tree_and_after_reopening() {
    let (mut world, task) = change_world();
    let client = world.client(fixtures::PARTY, fixtures::task_page(task));
    let delegate = review(&mut world, client, task);
    finish_review(&mut world, delegate);
    see(&mut world, client, fixtures::plan_item(delegate, b"Completed"));
    while world.front().engine.task(task).is_some() {
        progress(&mut world);
    }
    see(&mut world, client, Find::text("Landed · merge commit".as_bytes()));
    assert!(world.front().engine.task(delegate).is_none(), "review is ended in the real engine");
    let name = format!("Task {delegate}");
    see(&mut world, client, its_tasks(Find::named(Role::Link, name.as_bytes())));
    world.go(client, fixtures::inbox());
    world.go(client, fixtures::task_page(task));
    see(&mut world, client, fixtures::plan_item(delegate, b"Completed"));
    assert!(matches!(panel(&world, client).step(), w::Step::Landed(_)), "retained panel after reopening");
    assert!(!text(&world, client).contains("None."), "an ended procedure with an ended child is not empty");
}

// client/README.md 5.10: the query reads "its children in the order they
// were made" (QueryOrder::Ended, by number);
// a page for the ended child itself must never list that child as its own child.
#[test]
fn an_ended_child_of_the_change_is_never_listed_as_its_own_child() {
    let (mut world, task) = change_world();
    let client = world.client(fixtures::PARTY, fixtures::task_page(task));
    let delegate = review(&mut world, client, task);
    finish_review(&mut world, delegate);
    see(&mut world, client, fixtures::plan_item(delegate, b"Completed"));
    world.go(client, fixtures::task_page(delegate));
    see(&mut world, client, Find::text(b"QA approved"));
    assert!(
        world.root(client).person().pages().task().expect("agent task").ended.is_none(),
        "an agent child page does not read its parent's ended tasks"
    );
    assert!(!text(&world, client).contains("Its tasks"), "agent child's page cannot inherit its parent's children");
}

// client/README.md 5.10: an already-running procedure still draws its
// panel's step line when its page first opens (client/forge.md 9.2).
#[test]
fn a_change_prepared_by_the_existing_engine_driver_shows_its_current_line() {
    let (mut world, task) = change_world();
    world.front().engine.settle();
    let client = world.client(fixtures::PARTY, fixtures::task_page(task));
    see(&mut world, client, Find::exactly_named(Role::Region, b"Gates at its head"));
    let value = panel(&world, client);
    let step = value.step().clone();
    let page = text(&world, client);
    let line = match step {
        w::Step::Ready(ref ready) => match ready.landing() {
            Some(_) => "Ready: its landing, task ",
            None => "Ready: waiting for room for its landing.",
        },
        w::Step::Preparing(_) => "Being rebased onto main for its landing.",
        w::Step::Producing => "Producing its branch.",
        w::Step::Opening => "Opening its pull request.",
        w::Step::Recreating => "Making its branch again at the head temper last knew.",
        w::Step::Reopening => "Reopening its pull request.",
        w::Step::Checking => "Waiting for CI at its head.",
        w::Step::Gating => "Waiting for its gates at its head.",
        w::Step::Queued(_) => "in the queue for main",
        w::Step::First => "First in the queue for main.",
        w::Step::Updating => "Updating from main.",
        w::Step::Resolving => "Resolving its conflicts with main.",
        w::Step::Repairing(_) => "Repairing:",
        w::Step::Landing => "Merging at its head.",
        w::Step::Landed(_) => "Landed · merge commit",
    };
    assert!(page.contains(line), "panel at {step:?} supplies line {line:?}, drawn page: {page}");
}

// client/README.md 5.10: "Its ended ones are 5.8's ended rows, read once from
// the engine";
// client/forge.md 9.2: an ended change keeps its last panel when opened later.
#[test]
fn a_page_first_opened_after_landing_reads_its_ended_review() {
    let (mut world, task) = change_world();
    let driver = &mut world.front().engine;
    let assignment = loop {
        driver.settle();
        if let Some(assignment) = driver.delivered.iter().find_map(|delivery| {
            if let engine::Delivery::Assigned { assignment, .. } = delivery {
                (assignment.task != task).then_some(assignment.clone())
            } else {
                None
            }
        }) {
            break assignment;
        }
        let deadline =
            driver.root.next_deadline(&driver.env).expect("existing engine driver has a deadline before the gate runs");
        driver.fire_at(skein_lib::Duration::from_nanos(deadline.as_nanos()));
    };
    let child = assignment.task;
    driver.answer(
        Token::new(7),
        child,
        assignment.attempt,
        0,
        jig_domain_tasks::End::Finished {
            result: jig_domain_tasks::TaskResult::Verdict {
                code: 1,
                fields: Box::new([]),
                words: b"QA approved".as_slice().into(),
            },
            cancel_delegates: false,
        },
        engine::Parts::none(),
    );
    driver.settle();
    while driver.task(task).is_some() {
        let deadline =
            driver.root.next_deadline(&driver.env).expect("existing engine driver has a deadline until landing ends");
        driver.fire_at(skein_lib::Duration::from_nanos(deadline.as_nanos()));
        driver.settle();
    }
    let client = world.client(fixtures::PARTY, fixtures::task_page(task));
    see(&mut world, client, fixtures::plan_item(child, b"Completed"));
    let name = format!("Task {child}");
    see(&mut world, client, its_tasks(Find::named(Role::Link, name.as_bytes())));
    assert!(matches!(panel(&world, client).step(), w::Step::Landed(_)), "ended panel keeps its landing");
    assert!(world.front().engine.task(child).is_none(), "listed child is actually ended");
}
