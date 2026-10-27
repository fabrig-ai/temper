//! Signing in by the printed link (client/browser.md, 4.1, story 1): the page takes the secret from its own
//! address, puts `/welcome` in its place, and the browser keeps jig's cookie; a reload signs in by that cookie, and a
//! spent link shows the signed-out words.
use skein_fake_person::{Find, Step as PersonStep};
use skein_fake_person_browser::{Harness, Outcome, Script, Step};
use skein_ui::Role;
use temper_client_browser_world::{FIRST_CHAT_ADDRESS, Tier, WITHIN, first_chat};

#[test]
fn the_link_signs_in_then_the_cookie_and_a_spent_link_is_refused() {
    let tier = Tier::new("link-cookie-spent").with_model();
    let see = |find| Step::Person(PersonStep::See { find, within: WITHIN });
    let mut steps = vec![
        see(Find::exactly_named(Role::Heading { level: 1 }, b"Welcome to temper")),
        see(Find::exactly_named(Role::Link, b"Set up temper")),
        Step::Address(Box::from(b"/welcome".as_slice())),
    ];
    steps.extend(temper_client_browser_world::start_chat(b"Hello"));
    steps.extend([
        Step::Person(PersonStep::Reload),
        see(first_chat(b"Hello")),
        see(tier.states().live),
        Step::Address(Box::from(FIRST_CHAT_ADDRESS)),
        // Reopen makes a fresh browser without the cookie to spend the original link again.
        Step::Reopen,
        see(tier.states().signed_out),
        Step::Address(Box::from(b"/welcome".as_slice())),
    ]);
    let mut script = Script::new(steps.into_boxed_slice());
    assert_eq!(Harness::new(tier.setup()).run(&mut [&mut script]), Outcome::Done);
}
