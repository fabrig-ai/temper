//! The static page boots its world in a worker and spends that world's printed sign-in link
//! (playground.md, 2, 6 and 9); the server only supplies the bundle.
use skein_fake_person::{Find, Step as PersonStep};
use skein_fake_person_browser::{Harness, Outcome, Script, Step};
use skein_ui::Role;
use temper_client_browser_world::{Tier, WITHIN};

#[test]
fn the_playground_shows_the_welcome_signed_in_by_its_printed_link() {
    let tier = Tier::playground("playground");
    let see = |find| Step::Person(PersonStep::See { find, within: WITHIN });
    let mut script = Script::new(Box::new([
        see(Find::exactly_named(Role::Banner, b"Playground")),
        see(Find::text("Loading the playground…".as_bytes())),
        see(Find::exactly_named(Role::Heading { level: 1 }, b"Welcome to temper")),
        see(Find::exactly_named(Role::Button, b"Try the playground")),
        see(Find::exactly_named(Role::Button, b"Sign out")),
        Step::Address(Box::from(b"/welcome".as_slice())),
        Step::Person(PersonStep::Gone { find: Find::text("Loading the playground…".as_bytes()), within: WITHIN }),
    ]));
    assert_eq!(Harness::new(tier.playground_setup()).run(&mut [&mut script]), Outcome::Done);
}
