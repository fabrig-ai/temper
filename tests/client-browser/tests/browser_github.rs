//! Only Chromium proves that the manifest posts itself under its form-action policy (client/browser.md, 4.4).
use skein_fake_person::{Find, Step as PersonStep};
use skein_fake_person_browser::{Harness, Lent, Outcome, Script, Sent, Step};
use skein_ui::Role;
use temper_client_browser_world::{Tier, WITHIN};

mod manifest_history;
use manifest_history::{History, Observed};

#[test]
fn the_manifest_posts_once_and_back_waits_for_continue() {
    let (tier, github) = Tier::github("github-manifest");
    let mut github = Observed::new(github);
    let mut setup = tier.setup();
    // Hold the service's sign-in link for ctl; the tab first opens ctl's manifest link.
    setup.printed.sent = Sent::Command;
    let mut steps = tier.github_connect();
    steps.push(Step::Person(PersonStep::See {
        find: Find::exactly_named(Role::Link, b"Create GitHub App"),
        within: WITHIN,
    }));
    let mut script = History::new(Script::new(steps.into_boxed_slice()), github.counts());
    let mut harness = Harness::new(setup);
    let outcome = harness.run_lending(&mut [&mut script], &mut [Lent { name: b"github", peer: &mut github }]);
    let mut posts = 0;
    for record in github.peer.observations() {
        if let skein_fake_peers::forge::Observation::Head { call, .. } = record {
            let path = call.target.split(|byte| *byte == b'?').next().expect("path");
            match (call.method, path) {
                (skein_http::Method::Post, b"/settings/apps/new") => posts += 1,
                (skein_http::Method::Get, b"/favicon.ico") => {}
                _ => panic!("only the manifest post and an optional favicon request reach the fake"),
            }
        }
    }
    assert_eq!(posts, 2, "one automatic post, then one explicitly continued post");
    let link = harness.kept().iter().find(|kept| kept.name.as_ref() == b"manifest-link").expect("ctl manifest link");
    let link = std::str::from_utf8(&link.value).expect("manifest link UTF-8");
    let (page, state) = link.split_once("?state=").expect("manifest state");
    assert!(page.ends_with("/forges/1/github/manifest"));
    assert!(!state.is_empty());
    let origin = page.strip_suffix("forges/1/github/manifest").expect("clients origin");
    let layer = github.peer.github_layer().expect("GitHub layer");
    let (posted_state, manifest) = github.first_manifest.as_ref().expect("first automatic manifest post");
    assert_eq!(posted_state.as_ref(), state.as_bytes(), "the first post carries ctl's state");
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
