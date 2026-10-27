//! client/README.md, 8 and 10; domain/forge.md, 8.2–8.8.
//! A task page watches the whole change through CI, its gate and landing.
use jig_client::contract as c;
use jig_client_world::harness::{ClientId, Until, World};
use jig_domain_tasks as tasks;
use skein_fake_person::{Face, Find, Found, Step, Target, TreeFace};
use skein_forge::api;
use skein_lib::Token;
use skein_ui::Role;
use skein_world::domain::Verdict;
use std::fmt::Write as _;
use temper_client_domain::forge::{
    change::{Slots, slot},
    panel::State,
};
use temper_client_protocol_wire::v1 as w;
use temper_client_world::{app::Temper, fixtures, front::Joint};
use temper_engine_domain as engine;
use temper_engine_domain_world::forge_stand::ForgeStand;

const REPOSITORY: api::Repository = api::Repository { forge: 1, repository: 7 };
const BRANCH: &[u8] = b"temper/handed-change";
const BASE: &[u8] = b"main";
const BUILD: &[u8] = b"build";
const STEPS: u32 = 200;
const WORKER: Token = Token::new(7);

struct Facts {
    pull: api::Pull,
    checks: Box<[api::Status]>,
    main: api::Commit,
}

// Observe only between engine steps, without charging the engine's calls.
fn read(front: &mut Joint, what: api::Read) -> api::Answer {
    front.engine.forge.as_mut().expect("story's forge").read(REPOSITORY, what)
}

fn facts(front: &mut Joint) -> Facts {
    let api::Answer::Pull(pull) = read(front, api::Read::PullFor { head: BRANCH.into(), base: BASE.into() }) else {
        panic!("opened pull request")
    };
    let api::Answer::Checks(checks) = read(front, api::Read::Checks { commit: pull.commit }) else {
        panic!("CI contexts at the pull's head")
    };
    let api::Answer::Commit(main) = read(front, api::Read::Branch { branch: BASE.into() }) else { panic!("main head") };
    Facts { pull, checks, main }
}

// Joint polling fires deadlines and settles immediate work at simulated time. A run of two
// iterations brings the client at most one panel: the transport holds the next until the
// client takes this one (one document in flight), a later iteration's poll sends it, and it
// is due at least the world's least latency (1 ms) after that poll. Keeping the panel after
// each run so checks every delivery's order, not a sample of them.
fn step(world: &mut World<Temper, Joint>, person: ClientId, panels: &mut Vec<w::ChangePanel>) {
    let outcome = world.run(Until::Steps(2));
    assert!(matches!(outcome.verdict, Verdict::Passed | Verdict::Open { .. }), "{outcome:?}");
    keep_panel(world, person, panels);
}

fn keep_panel(world: &World<Temper, Joint>, person: ClientId, panels: &mut Vec<w::ChangePanel>) {
    if let Some(panel) = world.root(person).person().forge().panel() {
        match &panel.state {
            State::Loading => {}
            State::Shown(value) => {
                if panels.last() != Some(value.as_ref()) {
                    if let Some(previous) = panels.last() {
                        assert!(
                            order(value.step()) >= order(previous.step()),
                            "panel went backwards: {panels:?} -> {value:?}"
                        );
                        let before = slot(previous.step());
                        let after = slot(value.step());
                        assert!(after.done >= before.done, "page's done steps went backwards: {before:?} -> {after:?}");
                        // No current slot means all six are done, after every current slot.
                        assert!(
                            after.current.unwrap_or(6) >= before.current.unwrap_or(6),
                            "page's current step went backwards: {before:?} -> {after:?}"
                        );
                    }
                    panels.push(value.as_ref().clone());
                }
            }
            State::Unknown | State::Ended(_) => panic!("change watch ended: {panel:?}"),
        }
    }
}

// client/forge.md, 2.1 and 9.2: the successful handed-branch path.
fn order(step: &w::Step) -> u8 {
    match step {
        w::Step::Producing => 0,
        w::Step::Opening => 1,
        w::Step::Checking => 2,
        w::Step::Gating => 3,
        w::Step::Queued(_) => 4,
        w::Step::First => 5,
        w::Step::Landing => 6,
        w::Step::Landed(_) => 7,
        w::Step::Ended
        | w::Step::Recreating
        | w::Step::Reopening
        | w::Step::Updating
        | w::Step::Resolving
        | w::Step::Repairing(_) => {
            panic!("the passing head needs no recovery: {step:?}")
        }
    }
}

fn pull_opened(front: &Joint, goal: u64) -> bool {
    front.engine.store.rows.values().any(|record| {
        matches!(record, engine::Record::Forge { row } if matches!(row.as_ref(),
            temper_engine_domain_forge::Stored::Change(change) if change.task == goal && change.pull.is_some()))
    })
}

fn gate_task(front: &Joint, goal: u64) -> Option<tasks::TaskRecord> {
    front.engine.store.rows.values().find_map(|record| {
        if let engine::Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Live(row))) = record
            && row.root == goal
            && row.number != goal
            && front.hosted_attempt(row.number) == Some(row.attempt)
        {
            Some(*row.clone())
        } else {
            None
        }
    })
}

fn finished(document: &c::Task, goal: u64, pull: u64) {
    assert_eq!(document.chip.task, goal);
    assert_eq!(document.chip.phase, c::Phase::Ended);
    let Some(c::TaskResult::Document(result)) = document.result.as_ref() else { panic!("historical task result") };
    assert_eq!(result.task.task, goal);
    let c::Ending::Task(ending) = &result.ending else { panic!("task's finished result") };
    let c::OwnerChipEnding::Done(c::ResultValue::Change { part, .. }) = ending.as_ref() else {
        panic!("successfully landed change result");
    };
    assert_eq!(part.key, c::OwnerPartKey::Number(pull));
}

fn page_step(name: &[u8]) -> Find {
    Find::exactly_named(Role::Item, name)
        .within(Box::new([Target::ExactRole { role: Role::List, name: b"Steps".as_slice().into() }]))
}

fn its_task(number: u64) -> Find {
    let name = format!("Task {number}");
    Find::exactly_named(Role::Link, name.as_bytes()).within(Box::new([
        Target::ExactRole { role: Role::Tree, name: b"Its tasks".as_slice().into() },
        Target::ExactRole { role: Role::TreeItem, name: name.as_bytes().into() },
    ]))
}

fn drawn<'a>(world: &'a World<Temper, Joint>, person: ClientId, find: &Find) -> &'a skein_ui::Node {
    let tree = world.tree(person);
    let Found::Node(id) = TreeFace::new(tree).find(find) else { panic!("person's tree lacks {find:?}") };
    tree.find(id).expect("found node")
}

fn gate_page(
    world: &mut World<Temper, Joint>,
    person: ClientId,
    goal: u64,
    gate: u64,
    pull: u64,
    panel: &w::ChangePanel,
) {
    let tag = format!("Change · Task {goal}");
    let pull_line = format!(
        "PR #{pull} · acme/shop · {} → {}",
        std::str::from_utf8(BRANCH).expect("branch"),
        std::str::from_utf8(BASE).expect("base")
    );
    let bounds = format!(
        "Repairs {} of {} · resolutions {} of {} · updates {} of {}",
        panel.repairs().used(),
        panel.repairs().limit(),
        panel.resolutions().used(),
        panel.resolutions().limit(),
        panel.updates().used(),
        panel.updates().limit()
    );
    world.person(
        person,
        vec![
            Step::See { find: Find::text(tag.as_bytes()), within: fixtures::WITHIN },
            Step::See { find: Find::text(pull_line.as_bytes()), within: fixtures::WITHIN },
            Step::See { find: page_step(b"Gating"), within: fixtures::WITHIN },
            Step::See { find: Find::text(b"Waiting for its gates at its head."), within: fixtures::WITHIN },
            Step::See {
                find: Find::named(Role::Slot, b"CI at its headPassed").within(Box::new([Target::ExactRole {
                    role: Role::Region,
                    name: b"Gates at its head".as_slice().into(),
                }])),
                within: fixtures::WITHIN,
            },
            Step::See {
                find: Find::named(Role::Slot, "Gate 1 · review · blockingWaiting".as_bytes()).within(Box::new([
                    Target::ExactRole { role: Role::Region, name: b"Gates at its head".as_slice().into() },
                ])),
                within: fixtures::WITHIN,
            },
            Step::See {
                find: Find::exactly_named(Role::Link, "View on forge ↗".as_bytes()), within: fixtures::WITHIN
            },
            Step::See { find: Find::text(bounds.as_bytes()), within: fixtures::WITHIN },
            Step::See { find: its_task(gate), within: fixtures::WITHIN },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
    assert!(drawn(world, person, &page_step(b"Gating")).states.as_slice().contains(&skein_ui::State::Current));
    assert_eq!(
        drawn(world, person, &Find::exactly_named(Role::Link, "View on forge ↗".as_bytes())).external.as_deref(),
        Some(format!("http://forge.example/acme/shop/pulls/{pull}").as_bytes())
    );
}

fn landed_page(world: &mut World<Temper, Joint>, person: ClientId, goal: u64, gate: u64, merge: api::Commit) -> u64 {
    let mut children: Vec<_> = world
        .front()
        .engine
        .store
        .rows
        .values()
        .filter_map(|record| {
            if let engine::Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Ended(row))) = record
                && row.requester == tasks::Party::Task(goal)
            {
                Some(row.number)
            } else {
                None
            }
        })
        .collect();
    children.sort_unstable();
    assert!(children.contains(&gate), "the gate is an ended child");
    let merge_line =
        format!("Landed · merge commit {:02x}{:02x}{:02x}{:x}", merge[0], merge[1], merge[2], merge[3] >> 4);
    let mut script = vec![
        Step::See { find: page_step(b"Landed"), within: fixtures::WITHIN },
        Step::See { find: Find::text(merge_line.as_bytes()), within: fixtures::WITHIN },
    ];
    for child in &children {
        script.push(Step::See { find: its_task(*child), within: fixtures::WITHIN });
    }
    for absent in ["None.", "None yet.", "More of its ended tasks are not shown."] {
        script.push(Step::Gone {
            find: Find::exactly_named(Role::Paragraph, absent.as_bytes())
                .within(Box::new([Target::ExactRole { role: Role::Region, name: b"Its tasks".as_slice().into() }])),
            within: fixtures::WITHIN,
        });
    }
    script.push(Step::See {
        find: Find::exactly_named(Role::Link, "View on forge ↗".as_bytes()),
        within: fixtures::WITHIN,
    });
    world.person(person, script);
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
    let root = world.root(person).person();
    let page = root
        .pages()
        .task()
        .expect("task page")
        .ended
        .as_ref()
        .expect("ended children read")
        .page
        .as_ref()
        .expect("kept ended children page");
    let c::EndedGroup::Rows { rows, after, cut } = &page.ended else { panic!("ended children rows") };
    assert_eq!(after, &None);
    assert!(!cut);
    let mut kept: Vec<_> = rows.iter().map(|row| row.chip.task).collect();
    kept.sort_unstable();
    assert_eq!(kept, children, "every ended child is kept exactly once");
    outcome.callbacks
}

#[test]
fn a_change_handed_its_pushed_branch_lands_after_ci_and_its_gate() {
    let mut world = fixtures::world(fixtures::SEED);
    *world.front() = Joint::with_change_landing(fixtures::SEED, &fixtures::domain_limits());
    world.front().engine.forge = Some(ForgeStand::with_checks(fixtures::REPOSITORIES, &[BUILD]));
    fixtures::adopt_change_repository(world.front());
    world.front().engine.forge.as_mut().expect("story's forge").push_branch(REPOSITORY.repository, BRANCH);
    let api::Answer::Commit(head) = read(world.front(), api::Read::Branch { branch: BRANCH.into() }) else {
        panic!("pushed branch head")
    };
    let goal = fixtures::goal_with_pushed_change(world.front(), BRANCH);

    let person = world.client(fixtures::PARTY, fixtures::task_page(goal));
    let mut panels = Vec::new();

    for _ in 0..STEPS {
        if pull_opened(world.front(), goal) {
            break;
        }
        step(&mut world, person, &mut panels);
    }
    assert!(pull_opened(world.front(), goal), "pull opened within the step budget");
    let opened = facts(world.front());
    assert_ne!(opened.pull.number, 0);
    assert_eq!(opened.pull.state, api::State::Open);
    assert_eq!(opened.pull.head.as_ref(), BRANCH);
    assert_eq!(opened.pull.base.as_ref(), BASE);
    assert_eq!(opened.pull.commit, head);
    assert_eq!(opened.pull.merged, None);

    for _ in 0..STEPS {
        if gate_task(world.front(), goal).is_some() {
            break;
        }
        step(&mut world, person, &mut panels);
    }
    let gate = gate_task(world.front(), goal).expect("gate assigned within the step budget");
    let checked = facts(world.front());
    assert_eq!(checked.pull.number, opened.pull.number);
    assert_eq!(checked.pull.commit, head);
    assert_eq!(checked.pull.ci, api::Ci::Passed);
    assert_eq!(checked.checks.len(), 1);
    assert_eq!(checked.checks[0].context.as_ref(), BUILD);
    assert_eq!(checked.checks[0].check, api::Check::Passed);
    assert_eq!(checked.pull.merged, None, "blocking gate has no verdict yet");
    assert_eq!(
        world.front().engine.task(goal).expect("change task").made,
        2,
        "a handed branch needs only its gate, no producer answer"
    );
    assert_eq!(gate.requester, tasks::Party::Task(goal));
    assert!(matches!(gate.contract, tasks::Contract::Verdict { .. }));
    assert!(gate.spec.parameters.contains(&tasks::Parameter::Bytes { name: 3, value: BRANCH.into() }));
    let mut hexadecimal = String::with_capacity(40);
    for byte in &head[..20] {
        write!(&mut hexadecimal, "{byte:02x}").expect("hexadecimal head");
    }
    assert!(gate.spec.parameters.contains(&tasks::Parameter::Bytes { name: 8, value: hexadecimal.as_bytes().into() }));

    let gate_line = b"Waiting for its gates at its head.";
    world.person(person, vec![Step::See { find: Find::text(gate_line), within: fixtures::WITHIN }]);
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
    keep_panel(&world, person, &mut panels);
    panel_waiting_for_gate(panels.last().expect("gate panel"), &checked);
    gate_page(&mut world, person, goal, gate.number, checked.pull.number, panels.last().expect("gate panel"));

    let verdict = tasks::TaskResult::Verdict { code: 1, fields: Box::new([]), words: b"approved".as_slice().into() };
    world.front().engine.answer(
        WORKER,
        gate.number,
        gate.attempt,
        0,
        tasks::End::Finished { result: verdict.clone(), cancel_delegates: false },
        engine::Parts::none(),
    );
    world.front().engine.settle();
    for _ in 0..STEPS {
        if world.front().engine.task(goal).is_none() {
            break;
        }
        step(&mut world, person, &mut panels);
    }
    assert!(world.front().engine.task(goal).is_none(), "change finished within the step budget");
    let gate_key = engine::Key::Core(jig_domain_core::Key::Tasks(tasks::Key::Ended(gate.number)));
    let Some(engine::Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Ended(answered)))) =
        world.front().engine.store.rows.get(&gate_key)
    else {
        panic!("gate's durable answer");
    };
    assert_eq!(answered.phase, tasks::Phase::Ended(tasks::Ending::Done(verdict)));
    let landed = facts(world.front());
    assert_eq!(landed.pull.number, opened.pull.number);
    assert_eq!(landed.pull.commit, head);
    assert_eq!(landed.pull.state, api::State::Closed);
    let merge = landed.pull.merged.expect("pull request's merge commit");
    assert_eq!(landed.main, merge);
    assert_ne!(merge, checked.main);

    let callbacks = landed_page(&mut world, person, goal, gate.number, merge);
    keep_panel(&world, person, &mut panels);
    let root = world.root(person).person();
    let document = root.pages().task().expect("task page").document(root.client()).expect("finished task document");
    finished(document, goal, landed.pull.number);
    panels_landed(&panels, goal, head, &opened, &checked, merge);
    quiet(&mut world, person, goal, panels.last().expect("landed panel"), callbacks);
}

fn panel_waiting_for_gate(panel: &w::ChangePanel, facts: &Facts) {
    assert_eq!(panel.step(), &w::Step::Gating);
    assert_eq!(panel.pull().as_ref().expect("pull opened").number(), facts.pull.number);
    assert_eq!(panel.head(), &Some(facts.pull.commit));
    assert_eq!(panel.ci(), &w::Ci::Passed);
    let [gate] = panel.gates().as_slice() else { panic!("one blocking gate") };
    assert!(gate.blocks());
    assert_ne!(gate.verdict(), &w::Verdict::Approved);
}

fn panels_landed(
    panels: &[w::ChangePanel],
    goal: u64,
    head: api::Commit,
    opened: &Facts,
    checked: &Facts,
    merge: api::Commit,
) {
    let last = panels.last().expect("change panel delivered");
    assert_eq!(last.task(), goal);
    assert_eq!(last.forge(), REPOSITORY.forge);
    assert_eq!(last.owner(), b"acme");
    assert_eq!(last.name(), b"shop");
    assert_eq!(last.branch(), BRANCH);
    assert_eq!(last.landing(), BASE);
    assert_eq!(last.head(), &Some(head));
    assert_eq!(last.pull().as_ref().expect("pull in panel").number(), opened.pull.number);
    assert_eq!(last.ci(), &w::Ci::Passed);
    assert_eq!(slot(last.step()), Slots { current: None, done: 6 });
    let [gate] = last.gates().as_slice() else { panic!("one gate in the panel") };
    assert_eq!(gate.number(), 1);
    assert_eq!(gate.kind(), &w::GateKind::Review);
    assert!(gate.blocks());
    assert_eq!(gate.verdict(), &w::Verdict::Approved);
    let w::Step::Landed(result) = last.step() else { panic!("landed panel: {panels:?}") };
    assert_eq!(result.merge(), &merge);
    assert!(panels.iter().any(|panel| matches!(panel.step(), w::Step::Gating)));
    for panel in panels {
        assert_eq!(panel.task(), goal);
        if let Some(pull) = panel.pull() {
            assert_eq!(pull.number(), opened.pull.number);
            assert_eq!(
                pull.address(),
                format!("http://forge.example/acme/shop/pulls/{}", opened.pull.number).as_bytes()
            );
        }
        if panel.ci() == &w::Ci::Passed {
            assert_eq!(panel.head(), &Some(checked.pull.commit));
        }
    }
}

fn quiet(world: &mut World<Temper, Joint>, person: ClientId, goal: u64, last: &w::ChangePanel, mut callbacks: u64) {
    let handle = world.root(person).person().forge().panel().expect("panel slot").handle.expect("live handle");
    assert_eq!(
        world.root(person).person().client().document(handle).expect("live document").state,
        jig_client::State::Live
    );
    assert!(world.front().watching_change(person, goal));
    // Taking the last delivery can release another task document. Drain those
    // acknowledgements before checking the retained, idle watch.
    let mut settled = false;
    for _ in 0..STEPS {
        let outcome = world.run(Until::Done);
        assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
        if outcome.callbacks == callbacks {
            settled = true;
            break;
        }
        callbacks = outcome.callbacks;
    }
    assert!(settled, "final delivery acknowledgements settle within the step budget");
    // With the final deliveries consumed, settling again has no callback at all.
    let quiet = world.run(Until::Done);
    assert_eq!(quiet.verdict, Verdict::Passed, "{quiet:?}");
    assert_eq!(quiet.callbacks, callbacks, "no new delivery while the retained change is quiet");
    assert!(world.front().engine.root.quiescent());
    assert!(world.front().watching_change(person, goal));
    let panel = world.root(person).person().forge().panel().expect("quiet panel");
    assert_eq!(panel.handle, Some(handle));
    assert_eq!(panel.state, State::Shown(Box::new(last.clone())));
}

#[test]
fn a_forge_procedure_without_a_change_has_an_unknown_panel() {
    let mut world = fixtures::world(fixtures::SEED);
    let task = fixtures::goal_with_forge_panel(world.front());
    let procedure = world.front().engine.task(task).expect("live procedure").executor;
    assert_eq!(procedure, tasks::Executor::Procedure { connector: 1, code: 2 });
    assert!(
        !world.front().engine.store.rows.values().any(|record| {
            matches!(record, engine::Record::Forge { row } if matches!(row.as_ref(),
            temper_engine_domain_forge::Stored::Change(change) if change.task == task))
        }),
        "the existing procedure fixture has no connector change"
    );
    let person = world.client(fixtures::PARTY, fixtures::task_page(task));
    world.person(
        person,
        vec![
            Step::See { find: Find::text(b"The forge knows no change for this task."), within: fixtures::WITHIN },
            Step::Gone { find: Find::text(b"Shown on the forge."), within: fixtures::WITHIN },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
    let panel = world.root(person).person().forge().panel().expect("procedure's panel slot");
    assert_eq!(panel.subject.task, task);
    assert_eq!(panel.state, State::Unknown);
    assert!(panel.handle.is_none());
    assert!(!world.front().watching_change(person, task));
}

fn demo_change_page(demo_forge: u16, ended: bool) {
    let mut world = fixtures::world(fixtures::SEED);
    *world.front() = Joint::with_change_landing(fixtures::SEED, &fixtures::domain_limits());
    world.front().engine.forge = Some(ForgeStand::with_checks(fixtures::REPOSITORIES, &[BUILD]));
    fixtures::adopt_change_repository(world.front());
    world.front().engine.forge.as_mut().expect("story's forge").push_branch(REPOSITORY.repository, BRANCH);
    let goal = fixtures::goal_with_pushed_change(world.front(), BRANCH);
    let mut cues = skein_lib::List::with_capacity(1);
    cues.push(b"Change the title".as_slice().into()).expect("one cue");
    world.front().demo = Some(
        w::DemoAnswer::new(
            &w::CEILINGS,
            w::DemoAnswerParts {
                state: if ended { w::DemoState::Ended } else { w::DemoState::Running },
                project: 1,
                forge: demo_forge,
                provider: 17,
                owner: b"acme".as_slice().into(),
                name: b"shop".as_slice().into(),
                cues,
            },
        )
        .expect("scripted demo read beside the real engine"),
    );
    let person = world.client(fixtures::PARTY, fixtures::task_page(goal));
    let on_demo = demo_forge == REPOSITORY.forge;
    let link = Find::exactly_named(Role::Link, "View on forge ↗".as_bytes());
    let address = format!("http://forge.example/acme/shop/pulls/{}", facts_after_open(&mut world).pull.number);
    world.person(
        person,
        vec![
            Step::See { find: Find::text(b"Waiting for its gates at its head."), within: fixtures::WITHIN },
            if on_demo {
                Step::See {
                    find: Find::exactly_named(Role::Paragraph, format!("View on forge ↗ {address}").as_bytes()),
                    within: fixtures::WITHIN,
                }
            } else {
                Step::See { find: link.clone(), within: fixtures::WITHIN }
            },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
    let root = world.root(person).person();
    if ended {
        assert!(root.forge().demo().is_none());
        assert_eq!(root.forge().retired_demo().expect("retired identity").forge, demo_forge);
    } else {
        assert_eq!(root.forge().demo().expect("started demo").forge, demo_forge);
    }
    let external: Vec<_> =
        world.tree(person).nodes().iter().filter(|node| node.external.as_deref() == Some(address.as_bytes())).collect();
    if on_demo {
        assert!(external.is_empty(), "no node can navigate to the demo address");
        assert!(
            !world
                .tree(person)
                .nodes()
                .iter()
                .any(|node| node.role == Role::Link && node.name.as_deref() == Some("View on forge ↗".as_bytes()))
        );
    } else {
        assert_eq!(external.len(), 1);
        assert_eq!(external[0].role, Role::Link);
    }
}
fn facts_after_open(world: &mut World<Temper, Joint>) -> Facts {
    for _ in 0..STEPS {
        if world.front().engine.store.rows.values().any(|record| matches!(record,
            engine::Record::Forge { row } if matches!(row.as_ref(), temper_engine_domain_forge::Stored::Change(change) if change.pull.is_some()))) {
            return facts(world.front());
        }
        let outcome = world.run(Until::Steps(2));
        assert!(matches!(outcome.verdict, Verdict::Passed | Verdict::Open { .. }), "{outcome:?}");
    }
    panic!("pull opened within the step budget");
}
#[test]
fn a_demo_changes_address_is_text_not_a_link() {
    demo_change_page(REPOSITORY.forge, false);
}
#[test]
fn another_forges_change_keeps_its_link() {
    demo_change_page(REPOSITORY.forge + 1, false);
}
#[test]
fn an_ended_demo_changes_address_stays_text() {
    demo_change_page(REPOSITORY.forge, true);
}
