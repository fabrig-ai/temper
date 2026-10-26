use jig_client_world::harness::{FrontFault, Until};
use skein_fake_person::{Find, Step};
use skein_ui::Role;
use skein_world::domain::Verdict;
use temper_client_world::fixtures;
#[test]
fn a_repository_adopted_from_the_navigation_link_is_held_by_the_engine() {
    let mut world = fixtures::world(fixtures::SEED);
    let person = world.client(fixtures::PARTY, fixtures::project());
    world.person(person, fixtures::adopt_and_see_outcome(b"acme", b"shop"));
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
    assert_eq!(world.front().applications_admitted(), 1);
    assert!(world.front().engine_adopted(1, 7), "the engine holds acme/shop, id 7, in the project, owned");
}
#[test]
fn adopting_a_repository_the_forge_lacks_says_not_found() {
    let mut world = fixtures::world(fixtures::SEED);
    let person = world.client(fixtures::PARTY, fixtures::project());
    let mut steps = fixtures::adopt(b"acme", b"none");
    steps.push(Step::See {
        find: Find::text(b"No repository acme/none on forge 1 that temper's forge user can see."),
        within: fixtures::WITHIN,
    });
    world.person(person, steps);
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
    assert_eq!(world.front().applications_admitted(), 1);
}
#[test]
fn a_second_home_is_refused_by_the_forge_connector() {
    let mut world = fixtures::world(fixtures::SEED);
    let person = world.client(fixtures::PARTY, fixtures::project());
    let mut first = fixtures::adopt(b"acme", b"shop");
    first.push(Step::See { find: Find::text(b"Adopted acme/shop on forge 1 as owned."), within: fixtures::WITHIN });
    world.person(person, first);
    assert_eq!(world.run(Until::Done).verdict, Verdict::Passed);
    let mut steps = fixtures::adopt(b"acme", b"second");
    steps.push(Step::See {
        find: Find::text(
            b"The project has a home already. Adopt this repository without making it home, or choose a new project.",
        ),
        within: fixtures::WITHIN,
    });
    world.person(person, steps);
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
    assert_eq!(world.front().applications_admitted(), 2);
}
#[test]
fn a_field_the_form_refuses_sends_nothing_to_the_engine() {
    let mut world = fixtures::world(fixtures::SEED);
    let person = world.client(fixtures::PARTY, fixtures::project());
    let mut steps = fixtures::adopt(b"", b"shop");
    steps.push(Step::See { find: Find::named(Role::Status, b"Required."), within: fixtures::WITHIN });
    world.person(person, steps);
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
    assert_eq!(world.front().applications_admitted(), 0);
    assert_eq!(world.front().engine.forge.as_ref().expect("fake forge").calls, 0);
}
#[test]
fn a_double_press_of_adopt_adopts_once() {
    let mut world = fixtures::world(fixtures::SEED);
    let person = world.client(fixtures::PARTY, fixtures::project());
    let mut steps = fixtures::adopt(b"acme", b"shop");
    steps.pop().expect("Adopt is last");
    world.person(person, steps);
    assert_eq!(world.run(Until::Done).verdict, Verdict::Passed);
    let node = world
        .tree(person)
        .nodes()
        .iter()
        .find(|node| node.role == Role::Button && node.name.as_deref() == Some(b"Adopt"))
        .expect("Adopt")
        .id;
    world.act(person, node, skein_ui::Act::Press);
    world.act(person, node, skein_ui::Act::Press);
    world.person(
        person,
        vec![Step::See { find: Find::text(b"Adopted acme/shop on forge 1 as owned."), within: fixtures::WITHIN }],
    );
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
    assert_eq!(world.front().applications_admitted(), 1);
}
#[test]
fn a_reload_with_an_adoption_pending_sends_it_once() {
    let mut world = fixtures::world(fixtures::SEED);
    let person = world.client(fixtures::PARTY, fixtures::project());
    world.person(
        person,
        vec![Step::See { find: Find::named(Role::Link, b"Adopt a repository"), within: fixtures::WITHIN }],
    );
    assert_eq!(world.run(Until::Done).verdict, Verdict::Passed);
    world.fault(FrontFault::NoAnswer);
    let mut steps = fixtures::adopt(b"acme", b"shop");
    steps.extend([
        Step::Reload,
        Step::See { find: Find::named(Role::Status, b"Live"), within: fixtures::WITHIN },
        Step::Pause { span: skein_lib::Duration::from_millis(50) },
    ]);
    world.person(person, steps);
    let outcome = world.run(Until::Done);
    assert_eq!(outcome.verdict, Verdict::Passed, "{outcome:?}");
    assert_eq!(world.front().applications_admitted(), 1);
    assert!(world.kept(person).is_empty(), "the retried adoption's record retired");
}
