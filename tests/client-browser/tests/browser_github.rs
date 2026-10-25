//! Only Chromium proves that the manifest posts itself under its form-action policy (client/browser.md, 4.4).
use skein_fake_person::{Find, Step as PersonStep};
use skein_fake_person_browser::{Harness, Lent, Outcome, Script, Sent, Step};
use skein_ui::Role;
use temper_client_browser_world::{Tier, WITHIN};

#[test]
fn the_manifest_page_posts_itself_to_the_configured_github_web() {
    let (tier, mut github) = Tier::github("github-manifest");
    let mut setup = tier.setup();
    // Hold the service's sign-in link for ctl; the tab first opens ctl's manifest link.
    setup.printed.sent = Sent::Command;
    let mut steps = tier.github_connect();
    steps.push(Step::Person(PersonStep::See {
        find: Find::exactly_named(Role::Link, b"Create GitHub App"),
        within: WITHIN,
    }));
    let mut script = Script::new(steps.into_boxed_slice());
    let mut harness = Harness::new(setup);
    let outcome = harness.run_lending(&mut [&mut script], &mut [Lent { name: b"github", peer: &mut github }]);
    let mut posts = 0;
    for record in github.observations() {
        if let skein_fake_peers::forge::Observation::Head { call, .. } = record {
            let path = call.target.split(|byte| *byte == b'?').next().expect("path");
            match (call.method, path) {
                (skein_http::Method::Post, b"/settings/apps/new") => posts += 1,
                (skein_http::Method::Get, b"/favicon.ico") => {}
                _ => panic!("only the manifest post and an optional favicon request reach the fake"),
            }
        }
    }
    assert_eq!(posts, 1, "the page posts once, without following create");
    let link = harness.kept().iter().find(|kept| kept.name.as_ref() == b"manifest-link").expect("ctl manifest link");
    let link = std::str::from_utf8(&link.value).expect("manifest link UTF-8");
    let (page, state) = link.split_once("?state=").expect("manifest state");
    assert!(page.ends_with("/forges/1/github/manifest"));
    assert!(!state.is_empty());
    let origin = page.strip_suffix("forges/1/github/manifest").expect("clients origin");
    let layer = github.github_layer().expect("GitHub layer");
    let manifest = layer.manifest(state.as_bytes()).expect("page posted its manifest with ctl's state");
    assert_eq!(outcome, Outcome::Done);
    assert_eq!(manifest.url.as_ref(), origin.as_bytes());
    assert_eq!(manifest.redirect_url.as_ref(), format!("{origin}forges/1/github/redirect").as_bytes());
    assert_eq!(manifest.setup_url.as_deref(), Some(format!("{origin}forges/1/github/setup").as_bytes()));
    // Public, so that GitHub offers the account's organizations too (skein forge-setup.md, section 5).
    assert!(manifest.public);
    assert!(manifest.name.starts_with(b"temper-"));
    assert!(manifest.setup_on_update);
    // The hook's address and the manifest's other fixed fields are skein's, tested there.
    assert!(manifest.hook_attributes.as_ref().expect("manifest hook").active);
    let permissions: Vec<_> = manifest.default_permissions.iter().map(|row| (row.name.as_ref(), row.access)).collect();
    assert_eq!(permissions, skein_forge::github::setup::PERMISSIONS);
    let events: Vec<_> = manifest.default_events.iter().map(AsRef::as_ref).collect();
    assert_eq!(events, skein_forge::github::setup::EVENTS);
    assert_eq!(layer.installations().iter().count(), 0, "the test stops before create or installation");
    // Reaching the fake's link proves the automatic form post passed the page's CSP.
}

#[test]
fn back_from_github_shows_the_manifest_without_posting_again_until_continue() {
    let (tier, mut github) = Tier::github("github-manifest-history");
    let mut setup = tier.setup();
    setup.printed.sent = Sent::Command;
    let see =
        |role, name: &[u8]| Step::Person(PersonStep::See { find: Find::exactly_named(role, name), within: WITHIN });
    let mut steps = tier.github_connect();
    steps.extend([
        see(Role::Link, b"Create GitHub App"),
        Step::Back,
        see(Role::Heading { level: 2 }, "temper’s App, on GitHub".as_bytes()),
        see(Role::Button, b"Continue to GitHub"),
        see(Role::Link, b"Back to temper"),
        Step::Person(PersonStep::Press { find: Find::exactly_named(Role::Button, b"Continue to GitHub") }),
        see(Role::Link, b"Create GitHub App"),
    ]);
    let mut script = Script::new(steps.into_boxed_slice());
    let outcome =
        Harness::new(setup).run_lending(&mut [&mut script], &mut [Lent { name: b"github", peer: &mut github }]);
    assert_eq!(outcome, Outcome::Done);
    let mut posts = 0;
    for record in github.observations() {
        if let skein_fake_peers::forge::Observation::Head { call, .. } = record
            && call.method == skein_http::Method::Post
            && call.target.starts_with(b"/settings/apps/new?")
        {
            posts += 1;
        }
    }
    assert_eq!(posts, 2, "one automatic post, then one explicitly continued post");
}

#[test]
fn connect_github_creates_installs_and_returns_to_the_welcome() {
    let (tier, github) = Tier::github("github-chain");
    // Only request heads are needed; keep connection lifecycle and domain calls out of the record.
    let mut github =
        github.keeping(skein_fake_peers::forge::Kept { heads: true, ..skein_fake_peers::forge::Kept::NONE });
    let setup = tier.setup();
    let see =
        |role, name: &[u8]| Step::Person(PersonStep::See { find: Find::exactly_named(role, name), within: WITHIN });
    let press = |role, name: &[u8]| Step::Person(PersonStep::Press { find: Find::exactly_named(role, name) });
    let mut script = Script::new(Box::new([
        see(Role::Link, b"Set up temper"),
        Step::Person(PersonStep::Go { address: b"/welcome?at=forge".as_slice().into() }),
        see(Role::Button, b"Connect GitHub"),
        press(Role::Button, b"Connect GitHub"),
        see(Role::Link, b"Create GitHub App"),
        press(Role::Link, b"Create GitHub App"),
        // Conversion, installation and the setup's 303 return the tab without further navigation.
        see(Role::Heading { level: 2 }, b"Choose a repository to start in"),
        Step::Address(b"/welcome?at=forge&forge=1".as_slice().into()),
        see(Role::Choice, b"acme/app"),
        press(Role::Choice, b"acme/app"),
        see(Role::Button, b"Adopt acme/app"),
        press(Role::Button, b"Adopt acme/app"),
        see(Role::Heading { level: 1 }, b"Connect a model"),
        Step::Address(b"/welcome?at=model".as_slice().into()),
        // The Model step's forge summary is rendered from the adopted repository.
        see(Role::Paragraph, "GitHub · acme/app, in the new project app".as_bytes()),
    ]));
    let outcome =
        Harness::new(setup).run_lending(&mut [&mut script], &mut [Lent { name: b"github", peer: &mut github }]);
    let posts = github
        .observations()
        .iter()
        .filter(|record| {
            matches!(record, skein_fake_peers::forge::Observation::Head { call, .. }
            if call.method == skein_http::Method::Post && call.target.starts_with(b"/settings/apps/new?"))
        })
        .count();
    if outcome != Outcome::Done {
        eprintln!("GitHub chain observations: {:#?}", github.observations());
    }
    assert_eq!(outcome, Outcome::Done);
    assert_eq!(posts, 1, "one manifest post through installation and adoption");
    assert_eq!(github.github_layer().expect("GitHub layer").installations().iter().count(), 1, "one installation");
}
