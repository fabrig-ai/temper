//! The pages a browser moves between (client/browser.md, 4.1, stories 2 to 4): back and forward between the chats
//! and a chat, an address that names no page opens the inbox at `/`, Enter starts one chat where Shift+Enter breaks
//! its line, and a panic shows the failure page.
use skein_browser::boundary::Key;
use skein_fake_person::{Find, Step as PersonStep};
use skein_fake_person_browser::{Harness, Outcome, Script, Step, stories};
use skein_ui::Role;
use temper_client_browser_world::{ABSENT, FIRST_CHAT, FIRST_CHAT_ADDRESS, Tier, WITHIN, first_chat};

#[test]
fn a_non_page_address_opens_the_inbox_at_slash() {
    let tier = Tier::new("non-page-address");
    let see = |find| Step::Person(PersonStep::See { find, within: WITHIN });
    let mut script = Script::new(Box::new([
        see(Find::exactly_named(Role::Link, b"Set up temper")),
        Step::Person(PersonStep::Go { address: Box::from(b"/chats".as_slice()) }),
        see(tier.states().live),
        Step::Person(PersonStep::Press { find: Find::exactly_named(Role::Link, b"Chats") }),
        see(Find::exactly_named(Role::Heading { level: 1 }, b"Chats")),
        Step::Person(PersonStep::Go { address: Box::from(b"/nowhere".as_slice()) }),
        see(Find::exactly_named(Role::Heading { level: 1 }, b"Inbox")),
        Step::Address(Box::from(b"/".as_slice())),
    ]));
    assert_eq!(Harness::new(tier.setup()).run(&mut [&mut script]), Outcome::Done);
}

#[test]
fn enter_starts_one_chat_keeping_a_shift_enter_break() {
    let tier = Tier::new("composer").with_model();
    let see = |find| Step::Person(PersonStep::See { find, within: WITHIN });
    let composer = || Find::exactly_named(Role::TextArea, b"Start a chat");
    let chats = || Step::Person(PersonStep::Press { find: Find::exactly_named(Role::Link, b"Chats") });
    let mut script = Script::new(Box::new([
        see(Find::exactly_named(Role::Link, b"Set up temper")),
        Step::Person(PersonStep::Go { address: Box::from(b"/chats".as_slice()) }),
        see(tier.states().live),
        chats(),
        see(composer()),
        Step::Type { find: composer(), words: Box::from(b"Hello".as_slice()) },
        Step::KeyWithShift(Key::Enter),
        Step::Type { find: composer(), words: Box::from(b"there".as_slice()) },
        Step::Key(Key::Enter),
        see(first_chat(b"Hello")),
        Step::Address(Box::from(FIRST_CHAT_ADDRESS)),
        chats(),
        see(Find::exactly_named(Role::Link, FIRST_CHAT)),
        Step::Person(PersonStep::Gone { find: Find::exactly_named(Role::Link, b"T2"), within: ABSENT }),
    ]));
    assert_eq!(Harness::new(tier.setup()).run(&mut [&mut script]), Outcome::Done);
}

#[test]
fn a_panic_shows_the_failure_page() {
    let tier = Tier::new("panic");
    assert_eq!(stories::panic(&tier.setup(), b"/test/panic", Find::text(b"The application failed.")), Outcome::Done);
}

#[test]
fn a_chat_has_a_day_and_turn_clocks_beside_separately_named_speakers() {
    let tier = Tier::demo("chat-turn-times");
    let mut steps = temper_client_browser_world::start_playground_chat(b"Add CSV export to widget");
    for find in [
        Find::exactly_named(Role::Log, b"Conversation"),
        Find::exactly_named(Role::Paragraph, b"You"),
        Find::named(Role::Paragraph, "Today · ".as_bytes()),
        Find::named(Role::Time, b":"),
        Find::exactly_named(Role::TextArea, b"Message"),
        Find::exactly_named(Role::Button, b"Send"),
    ] {
        steps.push(Step::Person(PersonStep::See { find, within: WITHIN }));
    }
    let mut script = Script::new(steps.into_boxed_slice());
    assert_eq!(Harness::new(tier.setup()).run(&mut [&mut script]), Outcome::Done);
}

#[test]
fn history_keeps_the_chat_and_the_new_chat_focus_and_filter() {
    let tier = Tier::new("history").with_model();
    let see = |find| Step::Person(PersonStep::See { find, within: WITHIN });
    let press = |role, name: &[u8]| Step::Person(PersonStep::Press { find: Find::exactly_named(role, name) });
    let pressed = |position| Step::Keep {
        selector: format!("main [aria-label='Your chats'] + * > button[aria-pressed='true']:nth-child({position})")
            .into_bytes()
            .into_boxed_slice(),
        attribute: Some(b"aria-pressed".as_slice().into()),
        name: b"pressed-filter".as_slice().into(),
    };
    let mut steps = vec![
        see(Find::exactly_named(Role::Link, b"Set up temper")),
        Step::Person(PersonStep::Go { address: b"/".as_slice().into() }),
        see(tier.states().live),
        see(Find::exactly_named(Role::Heading { level: 1 }, b"Inbox")),
        press(Role::Link, b"+ New chat"),
        see(Find::exactly_named(Role::TextArea, b"Start a chat")),
        Step::Focused(Find::exactly_named(Role::TextArea, b"Start a chat")),
        Step::Address(b"/chats?at=start".as_slice().into()),
        press(Role::Button, b"Live"),
        see(Find::exactly_named(Role::Paragraph, b"No live chats.")),
        Step::Address(b"/chats?state=live".as_slice().into()),
        Step::Person(PersonStep::Reload),
        see(Find::exactly_named(Role::Button, b"Live")),
        pressed(2),
        Step::Back,
        see(Find::exactly_named(Role::Button, b"All")),
        Step::Address(b"/chats?at=start".as_slice().into()),
        pressed(1),
    ];
    // The filter starts on an empty list; the same visit can now make its chat.
    steps.push(Step::Person(PersonStep::Go { address: b"/welcome".as_slice().into() }));
    steps.extend(temper_client_browser_world::start_chat(b"Hello"));
    steps.extend([
        Step::Person(PersonStep::Go { address: b"/chats".as_slice().into() }),
        see(Find::exactly_named(Role::Heading { level: 1 }, b"Chats")),
        see(Find::exactly_named(Role::Link, FIRST_CHAT)),
        press(Role::Link, FIRST_CHAT),
        see(first_chat(b"Hello")),
        Step::Back,
        see(Find::exactly_named(Role::Heading { level: 1 }, b"Chats")),
        Step::Forward,
        see(first_chat(b"Hello")),
        Step::Person(PersonStep::Go { address: b"/chats".as_slice().into() }),
        see(Find::exactly_named(Role::Heading { level: 1 }, b"Chats")),
        Step::Person(PersonStep::Go { address: FIRST_CHAT_ADDRESS.into() }),
        see(first_chat(b"Hello")),
    ]);
    let mut script = Script::new(steps.into_boxed_slice());
    assert_eq!(Harness::new(tier.setup()).run(&mut [&mut script]), Outcome::Done);
}
