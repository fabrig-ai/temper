use jig_client_world::{
    harness::{App, Until},
    observe,
};
use skein_fake_person::{Face, Find, Found, Step, Target, TreeFace};
use skein_ui::Role;
use skein_world::domain::Verdict;
use temper_client_world::{app::Temper, fixtures, front::Joint};

fn running_delegate_page() -> (jig_client_world::harness::World<Temper, Joint>, jig_client_world::harness::ClientId, u64)
{
    let mut world = fixtures::world(fixtures::SEED);
    let fixtures::Dependents { first, .. } = fixtures::goal_with_dependent_delegates(world.front());
    let person = world.client(fixtures::PARTY, fixtures::task_page(first));
    (world, person, first)
}

#[test]
fn a_message_from_a_delegates_page_reaches_its_run() {
    let (mut world, person, task) = running_delegate_page();
    let words = b"Look closely at the retry path";
    world.person(
        person,
        vec![
            Step::See {
                find: Find::text("Steer this task · words reach its live run at the next turn.".as_bytes()),
                within: fixtures::WITHIN,
            },
            Step::See { find: Find::named(Role::TextArea, b"Message"), within: fixtures::WITHIN },
            Step::Type { find: Find::named(Role::TextArea, b"Message"), words: words.as_slice().into() },
            Step::Press { find: Find::named(Role::Button, b"Send") },
            Step::See { find: conversation(Find::text(words)), within: fixtures::WITHIN },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");

    let row = world.front().engine.task(task).expect("messaged delegate");
    assert!(row.inbox.iter().any(|message| {
        message.from == jig_domain_tasks::Party::Person(fixtures::PARTY) && message.words.as_ref() == words
    }));
    let next_turn = row.turn + 1;
    let number = fixtures::read_messages(world.front(), task);
    let row = world.front().engine.task(task).expect("delegate after reading");
    assert_eq!(row.turn, next_turn);
    assert_eq!(row.last_message, number);
    assert!(row.inbox.iter().all(|message| message.number > number), "the committed turn took the message");
    world.person(
        person,
        vec![
            Step::See { find: conversation(Find::text(b"I will follow that direction")), within: fixtures::WITHIN },
            Step::See { find: conversation(Find::text(words)), within: fixtures::WITHIN },
            Step::Gone { find: Find::text(b"Being sent"), within: fixtures::WITHIN },
            Step::Gone { find: Find::text(b"Sent"), within: fixtures::WITHIN },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
}

fn conversation(find: Find) -> Find {
    find.within(Box::new([Target::ExactRole { role: Role::Log, name: b"Conversation".as_slice().into() }]))
}

#[test]
fn a_delegate_stopped_from_its_page_is_held_then_released() {
    use jig_domain_tasks as tasks;
    let (mut world, person, task) = running_delegate_page();
    let attempt = world.front().engine.task(task).expect("running delegate").attempt;
    world.person(
        person,
        vec![
            Step::See { find: Find::named(Role::Button, b"Stop"), within: fixtures::WITHIN },
            Step::Press { find: Find::named(Role::Button, b"Stop") },
            Step::See { find: Find::named(Role::Dialog, b"Stop"), within: fixtures::WITHIN },
            Step::See {
                find: Find::text(b"Its run stops, and the task is held until it is released."),
                within: fixtures::WITHIN,
            },
            Step::Press { find: Find::named(Role::Button, b"Confirm") },
            Step::See { find: Find::named(Role::Button, b"Release task"), within: fixtures::WITHIN },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
    assert!(matches!(world.front().engine.task(task).expect("stopped delegate").phase, tasks::Phase::Held { .. }));
    world.person(
        person,
        vec![
            Step::Press { find: Find::named(Role::Button, b"Release task") },
            Step::See { find: Find::named(Role::Dialog, b"Release task"), within: fixtures::WITHIN },
            Step::See {
                find: Find::text(b"The next run resumes its conversation; tries count again from zero"),
                within: fixtures::WITHIN,
            },
            Step::Press { find: Find::named(Role::Button, b"Confirm") },
            Step::Gone { find: Find::named(Role::Button, b"Release task"), within: fixtures::WITHIN },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
    let row = world.front().engine.task(task).expect("released delegate");
    assert!(matches!(row.phase, tasks::Phase::Active(tasks::Active::Running { .. })));
    assert!(row.attempt > attempt, "release assigned a new run");
    assert_eq!(row.tries, tasks::Tries::NONE);
}

#[test]
fn a_goal_cancelled_from_its_page_ends_its_delegates() {
    use jig_domain_tasks as tasks;
    let mut world = fixtures::world(fixtures::SEED);
    let fixtures::Dependents { goal, first, second } = fixtures::goal_with_dependent_delegates(world.front());
    let person = world.client(fixtures::PARTY, fixtures::task_page(goal));
    world.person(
        person,
        vec![
            Step::See { find: Find::named(Role::Button, b"Cancel"), within: fixtures::WITHIN },
            Step::Press { find: Find::named(Role::Button, b"Cancel") },
            Step::See { find: Find::named(Role::Dialog, b"Cancel"), within: fixtures::WITHIN },
            Step::See { find: Find::text(b"Dangerous action: review its consequences"), within: fixtures::WITHIN },
            Step::See { find: Find::text(b"1 running tasks"), within: fixtures::WITHIN },
            Step::See { find: Find::text(b"1 waiting tasks"), within: fixtures::WITHIN },
            Step::Press { find: Find::named(Role::Button, b"Confirm") },
            Step::See { find: Find::named(Role::Status, b"Cancelled"), within: fixtures::WITHIN },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
    for task in [first, second] {
        let key = temper_engine_domain::Key::Core(jig_domain_core::Key::Tasks(tasks::Key::Ended(task)));
        let Some(temper_engine_domain::Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Ended(row)))) =
            world.front().engine.store.rows.get(&key)
        else {
            panic!("delegate {task} has an ended record");
        };
        assert!(matches!(row.phase, tasks::Phase::Ended(tasks::Ending::Cancelled { .. })), "delegate {task} cancelled");
    }
}

#[test]
fn a_waiting_delegates_message_is_refused_with_its_reason() {
    let mut world = fixtures::world(fixtures::SEED);
    let fixtures::Dependents { second, .. } = fixtures::goal_with_dependent_delegates(world.front());
    let person = world.client(fixtures::PARTY, fixtures::task_page(second));
    world.person(
        person,
        vec![
            Step::See { find: Find::text(b"Message"), within: fixtures::WITHIN },
            Step::See { find: Find::text(b"Messages reach it once it runs."), within: fixtures::WITHIN },
            Step::Gone { find: Find::named(Role::TextArea, b"Message"), within: fixtures::WITHIN },
            Step::Gone { find: Find::named(Role::Button, b"Send"), within: fixtures::WITHIN },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
    assert!(
        world
            .front()
            .engine
            .task(second)
            .expect("waiting delegate")
            .inbox
            .iter()
            .all(|message| { message.from != jig_domain_tasks::Party::Person(fixtures::PARTY) }),
        "the waiting delegate received no person message"
    );
}

#[test]
fn an_agents_older_turns_are_read_by_older() {
    let (mut world, person, task) = running_delegate_page();
    fixtures::extend_delegate_conversation(world.front(), task);
    world.person(
        person,
        vec![
            Step::See { find: conversation(Find::text(b"The latest findings")), within: fixtures::WITHIN },
            Step::Gone { find: conversation(Find::text(b"Findings in progress")), within: fixtures::WITHIN },
            Step::See { find: Find::named(Role::Button, b"Older"), within: fixtures::WITHIN },
            Step::Press { find: Find::named(Role::Button, b"Older") },
            Step::See { find: conversation(Find::text(b"Findings in progress")), within: fixtures::WITHIN },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
}

#[test]
fn a_goals_plan_says_which_task_a_delegate_waits_for() {
    let mut world = fixtures::world(fixtures::SEED);
    let fixtures::Dependents { goal, first, second } = fixtures::goal_with_dependent_delegates(world.front());
    let driver = &world.front().engine;
    let delegates = driver.task(goal).expect("goal").delegates.len();
    let spent = driver.task(first).expect("first delegate").numbers.spent;
    let second_spent = driver.task(second).expect("dependent delegate").numbers.spent;
    assert!(spent > 0, "the row has its own committed spend");
    let budget = fixtures::task_budget(world.front(), goal);
    let person = world.client(fixtures::PARTY, fixtures::task_page(goal));
    world.person(
        person,
        vec![
            Step::See {
                find: Find::named(Role::Status, format!("waiting on {delegates} delegates").as_bytes()).within(
                    Box::new([Target::ExactRole { role: Role::Region, name: b"Task details".as_slice().into() }]),
                ),
                within: fixtures::WITHIN,
            },
            Step::See {
                find: fixtures::plan_item(second, format!("waiting on task {first}").as_bytes()),
                within: fixtures::WITHIN,
            },
            Step::See {
                find: Find::text(format!("{spent} spent").as_bytes()).within(Box::new([Target::ExactRole {
                    role: Role::TreeItem,
                    name: format!("Task {first}").into_bytes().into(),
                }])),
                within: fixtures::WITHIN,
            },
            Step::See {
                find: Find::text(format!("{second_spent} spent").as_bytes()).within(Box::new([Target::ExactRole {
                    role: Role::TreeItem,
                    name: format!("Task {second}").into_bytes().into(),
                }])),
                within: fixtures::WITHIN,
            },
            Step::See {
                find: fixtures::dependency_item(second, Find::named(Role::Link, format!("Task {first}").as_bytes())),
                within: fixtures::WITHIN,
            },
            Step::See {
                find: Find::text(
                    format!("{} spent · {} left of {}", budget.numbers.spent, budget.left, budget.numbers.budget)
                        .as_bytes(),
                )
                .within(Box::new([Target::ExactRole {
                    role: Role::Region,
                    name: b"Budget and authority".as_slice().into(),
                }])),
                within: fixtures::WITHIN,
            },
            Step::Gone {
                find: Find::exactly_named(Role::TreeItem, format!("Task {goal}").as_bytes()),
                within: fixtures::WITHIN,
            },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
}

#[test]
fn a_finished_dependency_says_completed_in_its_dependants_line() {
    let mut world = fixtures::world(fixtures::SEED);
    let fixtures::Dependents { goal, first, second } = fixtures::goal_with_a_finished_dependency(world.front());
    let budget = fixtures::task_budget(world.front(), goal);
    let spent_below = budget.numbers.spent_below;
    assert!(spent_below > 0, "the completed delegate's expense settled to its goal");
    let person = world.client(fixtures::PARTY, fixtures::task_page(goal));
    world.person(
        person,
        vec![
            Step::See {
                find: fixtures::dependency_item(second, Find::named(Role::Link, format!("Task {first}").as_bytes())),
                within: fixtures::WITHIN,
            },
            Step::See {
                find: fixtures::dependency_item(second, Find::named(Role::Status, b"Completed")),
                within: fixtures::WITHIN,
            },
            Step::See {
                find: Find::named(Role::Term, b"Delegates spent").within(Box::new([Target::ExactRole {
                    role: Role::Region,
                    name: b"Budget and authority".as_slice().into(),
                }])),
                within: fixtures::WITHIN,
            },
            Step::See {
                find: Find::exactly_named(Role::Description, format!("{spent_below}").as_bytes()).within(Box::new([
                    Target::ExactRole { role: Role::Region, name: b"Budget and authority".as_slice().into() },
                ])),
                within: fixtures::WITHIN,
            },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
}

#[test]
fn a_delegates_page_shows_its_brief_run_and_report() {
    let mut world = fixtures::world(fixtures::SEED);
    let fixtures::Dependents { goal, first, second } = fixtures::goal_with_dependent_delegates(world.front());
    let attempt = world.front().engine.last_assignment().attempt;
    let person = world.client(fixtures::PARTY, fixtures::task_page(goal));
    world.person(person, open_delegate_and_return_to_goal(goal, first, attempt));
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");

    fixtures::report_first_delegate(world.front(), first);
    world.person(person, see_reported_delegate_and_dependency(first, second, attempt));
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
}

fn open_delegate_and_return_to_goal(goal: u64, first: u64, attempt: u64) -> Vec<Step<temper_client_domain::Address>> {
    vec![
        Step::See {
            find: Find::exactly_named(Role::Link, format!("Task {first}").as_bytes()).within(Box::new([
                Target::ExactRole { role: Role::TreeItem, name: format!("Task {first}").into_bytes().into() },
            ])),
            within: fixtures::WITHIN,
        },
        Step::Press {
            find: Find::exactly_named(Role::Link, format!("Task {first}").as_bytes()).within(Box::new([
                Target::ExactRole { role: Role::TreeItem, name: format!("Task {first}").into_bytes().into() },
            ])),
        },
        Step::See {
            find: Find::named(Role::Heading { level: 1 }, format!("T{first} · Investigate the change").as_bytes()),
            within: fixtures::WITHIN,
        },
        Step::See {
            find: Find::text(b"Investigate the change")
                .within(Box::new([Target::ExactRole { role: Role::Region, name: b"Task brief".as_slice().into() }])),
            within: fixtures::WITHIN,
        },
        Step::See {
            find: Find::text(format!("Asked by task {goal}").as_bytes())
                .within(Box::new([Target::ExactRole { role: Role::Region, name: b"Task brief".as_slice().into() }])),
            within: fixtures::WITHIN,
        },
        Step::See {
            find: Find::named(Role::Heading { level: 2 }, format!("Run {attempt} · live now").as_bytes()),
            within: fixtures::WITHIN,
        },
        Step::See {
            find: Find::text(b"Findings in progress")
                .within(Box::new([Target::ExactRole { role: Role::Log, name: b"Conversation".as_slice().into() }])),
            within: fixtures::WITHIN,
        },
        Step::Gone { find: Find::named(Role::Region, b"Task details"), within: fixtures::WITHIN },
        Step::Gone { find: Find::named(Role::Region, b"Result"), within: fixtures::WITHIN },
        Step::Press { find: Find::exactly_named(Role::Link, format!("Task {goal}").as_bytes()) },
        Step::See { find: Find::named(Role::Region, b"Task details"), within: fixtures::WITHIN },
    ]
}

fn see_reported_delegate_and_dependency(
    first: u64,
    second: u64,
    attempt: u64,
) -> Vec<Step<temper_client_domain::Address>> {
    vec![
        Step::Go { address: fixtures::task_page(first) },
        Step::See {
            find: Find::named(Role::Heading { level: 2 }, format!("Run {attempt} · ended").as_bytes()),
            within: fixtures::WITHIN,
        },
        Step::See {
            find: Find::named(Role::Status, b"Completed")
                .within(Box::new([Target::ExactRole { role: Role::Region, name: b"Result".as_slice().into() }])),
            within: fixtures::WITHIN,
        },
        Step::See {
            find: Find::text(b"The change is feasible")
                .within(Box::new([Target::ExactRole { role: Role::Region, name: b"Result".as_slice().into() }])),
            within: fixtures::WITHIN,
        },
        Step::Gone {
            find: Find::named(Role::Heading { level: 2 }, format!("Run {attempt} · live now").as_bytes()),
            within: fixtures::WITHIN,
        },
        Step::Go { address: fixtures::task_page(second) },
        Step::See {
            find: Find::exactly_named(Role::Link, format!("Task {first}").as_bytes()).within(Box::new([
                Target::ExactRole { role: Role::Region, name: b"Task brief".as_slice().into() },
                Target::ExactRole {
                    role: Role::List,
                    name: format!("Dependencies of task {second}").into_bytes().into(),
                },
            ])),
            within: fixtures::WITHIN,
        },
        Step::See {
            find: Find::named(Role::Status, b"Completed").within(Box::new([
                Target::ExactRole { role: Role::Region, name: b"Task brief".as_slice().into() },
                Target::ExactRole {
                    role: Role::List,
                    name: format!("Dependencies of task {second}").into_bytes().into(),
                },
            ])),
            within: fixtures::WITHIN,
        },
    ]
}

#[test]
fn a_finished_goals_page_shows_its_report() {
    use jig_domain_tasks as tasks;
    let mut world = fixtures::world(fixtures::SEED);
    let goal = fixtures::finished_goal(world.front());
    let key = temper_engine_domain::Key::Core(jig_domain_core::Key::Tasks(tasks::Key::Ended(goal)));
    let Some(temper_engine_domain::Record::Core(jig_domain_core::Record::Tasks(tasks::Stored::Ended(row)))) =
        world.front().engine.store.rows.get(&key)
    else {
        panic!("ended goal retained");
    };
    let tasks::Phase::Ended(tasks::Ending::Done(tasks::TaskResult::Report { words })) = &row.phase else {
        panic!("goal finished with a report");
    };
    let words = words.clone();
    let person = world.client(fixtures::PARTY, fixtures::task_page(goal));
    world.person(
        person,
        vec![
            Step::See {
                find: Find::named(Role::Status, b"Completed")
                    .within(Box::new([Target::ExactRole { role: Role::Region, name: b"Result".as_slice().into() }])),
                within: fixtures::WITHIN,
            },
            Step::See {
                find: Find::text(&words)
                    .within(Box::new([Target::ExactRole { role: Role::Region, name: b"Result".as_slice().into() }])),
                within: fixtures::WITHIN,
            },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
}

#[test]
fn a_goals_page_shows_its_plan_with_the_held_task_marked() {
    let mut world = fixtures::world(fixtures::SEED);
    let (goal, held) = fixtures::goal_with_held_delegate(world.front());
    let person = world.client(fixtures::PARTY, fixtures::task_page(goal));
    world.person(
        person,
        vec![
            Step::See { find: Find::named(Role::Heading { level: 2 }, b"Plan"), within: fixtures::WITHIN },
            Step::See { find: fixtures::plan_item(held, b"held"), within: fixtures::WITHIN },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
}
#[test]
fn a_task_with_the_chat_label_opens_as_its_chat() {
    let mut world = fixtures::world(fixtures::SEED);
    let task = fixtures::chat(world.front(), b"Hello");
    let person = world.client(fixtures::PARTY, fixtures::task_page(task));
    world.person(person, vec![Step::See { find: Find::named(Role::TextArea, b"Message"), within: fixtures::WITHIN }]);
    assert_eq!(world.run(Until::Done).verdict, Verdict::Passed);
}
#[test]
fn a_forge_panel_on_a_goals_page_says_shown_on_the_forge() {
    let mut world = fixtures::world(fixtures::SEED);
    let goal = fixtures::goal_with_forge_panel(world.front());
    let person = world.client(fixtures::PARTY, fixtures::task_page(goal));
    world.person(
        person,
        vec![Step::See { find: Find::text(b"The forge knows no change for this task."), within: fixtures::WITHIN }],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
}
#[test]
fn a_held_task_released_from_a_goals_plan_runs_again() {
    let mut world = fixtures::world(fixtures::SEED);
    let (goal, held) = fixtures::goal_with_held_delegate(world.front());
    let person = world.client(fixtures::PARTY, fixtures::task_page(goal));
    world.person(
        person,
        vec![
            Step::See { find: Find::named(Role::Button, b"Release task"), within: fixtures::WITHIN },
            Step::Press { find: Find::named(Role::Button, b"Release task") },
            Step::See { find: Find::named(Role::Dialog, b"Release task"), within: fixtures::WITHIN },
            Step::See {
                find: Find::text(b"The next run resumes its conversation; tries count again from zero"),
                within: fixtures::WITHIN,
            },
            Step::Press { find: Find::named(Role::Button, b"Confirm") },
            Step::See { find: fixtures::plan_item(held, b"running"), within: fixtures::WITHIN },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
    assert_eq!(world.front().engine.task(held).expect("released delegate").tries, jig_domain_tasks::Tries::NONE);
}

#[test]
fn a_chat_reply_is_identified_by_its_task_in_the_native_inbox() {
    let mut world = fixtures::world(fixtures::SEED);
    let task = fixtures::replied_chat(world.front(), b"Hello");
    let person = world.client(fixtures::PARTY, fixtures::inbox());
    // An inbox entry is named by its kind and its source task (client/README.md, 3.5).
    let entry = |kind: &str| format!("{kind} from T{task}").into_bytes().into_boxed_slice();
    let reply = Find::named(Role::Heading { level: 3 }, b"replied")
        .within(Box::new([skein_fake_person::Target::Role { role: Role::Item, name: entry("Reply") }]));
    world.person(person, vec![Step::See { find: reply, within: fixtures::WITHIN }]);
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");

    fixtures::finish_replied_chat(world.front(), task);
    world.person(
        person,
        vec![
            Step::See { find: Find::named(Role::Status, b"Done"), within: fixtures::WITHIN },
            Step::See { find: Find::named(Role::Heading { level: 3 }, b"replied"), within: fixtures::WITHIN },
        ],
    );
    assert_eq!(world.run(Until::Done).verdict, Verdict::Passed);
    let tree = world.tree(person);
    for (object, heading, name) in [
        (jig_client::contract::ObjectKey::Result(task), format!("T{task} Hello is done").as_bytes(), entry("Updates")),
        (jig_client::contract::ObjectKey::Task(task), format!("T{task} Hello replied").as_bytes(), entry("Reply")),
    ] {
        let item = tree
            .nodes()
            .iter()
            .find(|node| node.role == Role::Item && Temper::card(tree, node.id) == Some(observe::key(object)))
            .expect("independent same-task inbox card");
        let mut face = TreeFace::subtree(tree, item.id).expect("card subtree");
        assert_eq!(face.find(&Find::named(Role::Item, &name)), Found::Node(item.id));
        assert!(matches!(face.find(&Find::exactly_named(Role::Heading { level: 3 }, heading)), Found::Node(_)));
    }
    let root = world.root(person).person();
    let inbox = root.pages().inbox().expect("inbox");
    let Some(jig_client::Latest::Jig(jig_client::contract::Delivery::OwnerInbox(owner))) =
        root.client().document(inbox.entries).and_then(|document| document.latest)
    else {
        panic!("owner inbox delivered")
    };
    assert_eq!(owner.results_seen, 0, "Result read position is independent of conversation Seen");
    let reply = owner
        .updates
        .iter()
        .find(|row| {
            row.task_number() == task && matches!(row.object, jig_client::contract::OwnerInboxObject::Reply { .. })
        })
        .expect("same-task unread Reply");
    assert_eq!(reply.unread, Some(true));
    let line = reply.reply.as_ref().expect("Reply task context");
    assert!(line.ended_at.is_some());
    let conversation = line.conversation.expect("conversation read position");
    assert_eq!(conversation.seen, 0);
    assert!(conversation.unread.value > 0);
}

#[test]
fn a_client_closed_on_a_tasks_page_opens_at_the_inbox_again() {
    let mut world = fixtures::world(fixtures::SEED);
    let (goal, _) = fixtures::goal_with_held_delegate(world.front());
    let person = world.client(fixtures::PARTY, fixtures::task_page(goal));
    world.person(
        person,
        vec![
            Step::See { find: Find::named(Role::Heading { level: 2 }, b"Plan"), within: fixtures::WITHIN },
            Step::Reload,
            Step::See { find: Find::named(Role::Heading { level: 1 }, b"Inbox"), within: fixtures::WITHIN },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
}

#[test]
fn a_goals_specification_amended_from_its_page_reaches_the_engine() {
    let mut world = fixtures::world(fixtures::SEED);
    let (goal, _) = fixtures::goal_with_held_delegate(world.front());
    let revision = world.front().engine.task(goal).expect("goal").revisions.spec;
    let person = world.client(fixtures::PARTY, fixtures::task_page(goal));
    let words = b"Rotate tokens and preserve existing sessions";
    world.person(
        person,
        vec![
            Step::See { find: Find::named(Role::Button, b"Amend"), within: fixtures::WITHIN },
            Step::Press { find: Find::named(Role::Button, b"Amend") },
            Step::Type { find: Find::named(Role::TextArea, b"Specification"), words: words.as_slice().into() },
            Step::Press { find: Find::named(Role::Button, b"Send amendment") },
            Step::See {
                find: Find::named(Role::Status, b"Applied").within(Box::new([Target::Role {
                    role: Role::Region,
                    name: format!("Amend task {goal}").into_bytes().into_boxed_slice(),
                }])),
                within: fixtures::WITHIN,
            },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
    let task = world.front().engine.task(goal).expect("amended goal");
    assert_eq!(task.spec.words.as_ref(), words);
    assert!(task.revisions.spec > revision);
    assert!(
        world
            .root(person)
            .person()
            .pages()
            .task()
            .expect("goal page")
            .amending
            .as_ref()
            .expect("amendment outcome")
            .done
    );
}

#[test]
fn a_busy_ended_task_read_shows_progress_then_its_document() {
    // The shipped first backoff is one second, followed by the usual one-second response allowance.
    const AFTER_RETRY: skein_lib::Duration = skein_lib::Duration::from_secs(2);
    let mut world = fixtures::world(fixtures::SEED);
    let goal = fixtures::finished_goal(world.front());
    world.front().task_read_ends.push_back(jig_client::contract::ReadNone::Busy);
    let person = world.client(fixtures::PARTY, fixtures::task_page(goal));
    world.person(
        person,
        vec![
            Step::See {
                find: Find::named(Role::Status, format!("Reading task {goal}…").as_bytes()),
                within: fixtures::WITHIN,
            },
            Step::See { find: Find::text(b"The repository is ready"), within: AFTER_RETRY },
            Step::Gone {
                find: Find::named(Role::Status, format!("Reading task {goal}…").as_bytes()),
                within: fixtures::WITHIN,
            },
        ],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
}

#[test]
fn an_unknown_task_address_explains_its_missing_document() {
    const UNKNOWN_TASK: u64 = 999;
    let mut world = fixtures::world(fixtures::SEED);
    let person = world.client(fixtures::PARTY, fixtures::task_page(UNKNOWN_TASK));
    world.person(
        person,
        vec![Step::See { find: Find::named(Role::Status, b"This task could not be found."), within: fixtures::WITHIN }],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
}
