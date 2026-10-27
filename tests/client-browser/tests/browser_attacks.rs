//! The attacks that need a browser (client/browser.md, 4.6, story 6; jig's protocol/browser.md, 15).
use jig_client_fake_person_browser::pages;
use skein_fake_person::{Find, Step as PersonStep};
use skein_fake_person_browser::{
    Harness, Outcome, Script, Step,
    origins::{Origin, Page},
};
use skein_lib::Duration;
use skein_ui::Role;
use temper_client_browser_world::{ABSENT, Tier};

// Keep jig's attack stories' five-second page/refusal wait and one-second absence check.
const PAGE_WAIT: Duration = Duration::from_secs(5);
const ATTACKER: u32 = 0; // The setup holds exactly one attacker listener.

fn see(find: Find) -> Step {
    Step::Person(PersonStep::See { find, within: PAGE_WAIT })
}
fn unchanged(tier: &Tier) -> [Step; 3] {
    [
        Step::Person(PersonStep::Go { address: tier.target().page.into() }),
        see(Find::exactly_named(Role::Button, b"Sign out")),
        Step::Person(PersonStep::Gone { find: tier.target().done, within: ABSENT }),
    ]
}

/// One listener is visited as 127.0.0.1 and localhost because the two request proofs need two origins:
/// the same host on another port can carry its cookie; another host is another site. Framing uses
/// that same listener's localhost alias, all in one browser/service session.
#[test]
fn cross_origin_requests_do_nothing_and_a_frame_shows_nothing() {
    let tier = Tier::new("browser-attacks");
    let target = tier.target();
    let live = || Find::exactly_named(Role::Button, b"Sign out");
    let mut setup = tier.setup();
    setup.expected.push(b"Failed to load resource".as_slice().into());
    let aliases = Page {
        path: b"/aliases".as_slice().into(),
        kind: b"text/html".as_slice().into(),
        body: br##"<!doctype html><title>Attacker aliases</title>
<a id="forging" href="#">Attack from localhost</a>
<a id="framing" href="#">Frame from localhost</a>
<script>
const url = new URL(location.href);
url.hostname = 'localhost';
for (const path of ['forging', 'framing']) {
    url.pathname = '/' + path;
    document.getElementById(path).href = url.href;
}
</script>"##
            .as_slice()
            .into(),
        headers: vec![],
    };
    setup.origins.push(Origin {
        host: b"127.0.0.1".as_slice().into(),
        // Reuse jig's two forged fetches and its frame unchanged.
        pages: vec![pages::forging(&target), pages::framing(&target), aliases],
    });
    let trouble = |words: &[u8]| Step::Trouble { words: words.into(), within: PAGE_WAIT };
    let aliases = || Step::Visit { origin: ATTACKER, path: b"/aliases".as_slice().into() };
    let press = |name: &[u8]| Step::Person(PersonStep::Press { find: Find::exactly_named(Role::Link, name) });
    let mut steps = vec![
        see(live()),
        Step::Visit { origin: ATTACKER, path: b"/forging".as_slice().into() },
        trouble(b"blocked by CORS policy"),
        trouble(b"from origin 'http://127.0.0.1:"),
    ];
    steps.extend(unchanged(&tier));
    steps.extend([
        see(live()),
        aliases(),
        see(Find::exactly_named(Role::Link, b"Attack from localhost")),
        press(b"Attack from localhost"),
        // Diagnostics accumulate in a session: require this alias's own refusal as well as jig's words.
        trouble(b"from origin 'http://localhost:"),
        trouble(b"blocked by CORS policy"),
    ]);
    steps.extend(unchanged(&tier));
    steps.extend([
        see(live()),
        aliases(),
        see(Find::exactly_named(Role::Link, b"Frame from localhost")),
        press(b"Frame from localhost"),
        trouble(b"frame-ancestors"),
        Step::Person(PersonStep::Gone { find: live(), within: ABSENT }),
    ]);
    let mut script = Script::new(steps.into_boxed_slice());
    assert_eq!(Harness::new(setup).run(&mut [&mut script]), Outcome::Done);
}
