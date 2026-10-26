//! Story A's semantic script, from the box's playground (playground.md, 5.2).
use skein_fake_person::{Find, Step, Target};
use skein_lib::Duration;
use skein_ui::Role;
use temper_client_domain::{Address, address::ChatsFilter};

const WITHIN: Duration = Duration::from_secs(40);
const LANDS_WITHIN: Duration = Duration::from_secs(122);

fn see(find: Find) -> Step<Address> {
    Step::See { find, within: WITHIN }
}
fn conversation() -> Box<[Target]> {
    Box::new([Target::Role { role: Role::Log, name: b"Conversation".as_slice().into() }])
}
fn card() -> Box<[Target]> {
    Box::new([
        Target::Role { role: Role::Log, name: b"Conversation".as_slice().into() },
        Target::Role { role: Role::Region, name: b"Task 2".as_slice().into() },
    ])
}

/// The signed-in welcome, without starting the demo.
#[must_use]
pub fn welcome() -> Box<[Step<Address>]> {
    Box::new([see(Find::exactly_named(Role::Button, b"Try the playground"))])
}

/// Land CSV through the forge, then ask each other suggestion in a new chat.
#[must_use]
pub fn whole() -> Box<[Step<Address>]> {
    let mut steps = vec![
        see(Find::exactly_named(Role::Button, b"Try the playground")),
        Step::Press { find: Find::exactly_named(Role::Button, b"Try the playground") },
        see(Find::exactly_named(Role::Status, b"Ready")),
        see(Find::exactly_named(Role::Paragraph, "Using Scripted model · Default".as_bytes())),
        see(Find::named(Role::Button, b"Add CSV export to widget")),
        Step::Press { find: Find::named(Role::Button, b"Add CSV export to widget") },
        see(Find::exactly_named(Role::Paragraph, b"Add CSV export to widget").within(conversation())),
        see(Find::text("I’ll delegate this change in playground/widget. It will push a branch, open a pull request, check CI and land into main on Playground forge.".as_bytes()).within(conversation())),
        Step::See {
            find: Find::text("CSV export is available from widget’s table. The change landed on Playground forge. This was a scripted change.".as_bytes()).within(conversation()),
            within: LANDS_WITHIN,
        },
        see(Find::text("T2 · CHANGE".as_bytes()).within(card())),
        see(Find::exactly_named(Role::Status, b"Landed").within(card())),
        see(Find::text("CI passed. Landed · merge commit ".as_bytes()).within(card())),
        Step::Gone { find: Find::exactly_named(Role::Status, b"Applied"), within: WITHIN },
    ];
    for (cue, answer) in [
        (
            b"Add a dark theme to widget".as_slice(),
            b"I would give widget a dark theme that follows the system's preference".as_slice(),
        ),
        (
            b"Fix widget's empty search state".as_slice(),
            b"I would have widget say when a search finds nothing".as_slice(),
        ),
    ] {
        steps.extend([
            Step::Go { address: Address::Chats { filter: ChatsFilter::All, start: false } },
            see(Find::named(Role::Button, cue)),
            Step::Press { find: Find::named(Role::Button, cue) },
            see(Find::exactly_named(Role::Paragraph, cue).within(conversation())),
            see(Find::text(answer).within(conversation())),
            see(Find::text(b"I am the scripted model and play this from a script, so nothing is changed")
                .within(conversation())),
        ]);
    }
    steps.into_boxed_slice()
}
