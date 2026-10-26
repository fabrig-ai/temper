#![allow(clippy::disallowed_types, clippy::disallowed_methods, reason = "tests inspect semantic trees")]
use super::{connect_forge as cf, fixtures as f};
use skein_lib::List;
use skein_ui::{Act, Role, State};
use temper_client_domain::ChatsFilter;
use temper_client_domain::{self as d, Address, Choice, Step};
use temper_client_protocol_wire::{self as wire, v1 as w};
const HERE: u16 = 3;
fn root(address: Address, connection: w::Connection) -> d::Domain {
    f::forge_root(address, cf::rows(connection, w::Offer::Offered))
}
fn action(root: &d::Domain, role: Role, name: &[u8]) -> d::Action {
    let mut view = f::page_view(root);
    let node = f::find(view.tree(), role, name).expect("named action");
    crate::act(&mut view, node, Act::Press).expect("bound action")
}
fn go(address: Address) -> d::Action {
    d::Action::Pages(d::pages::Act::Go { address })
}
#[test]
fn the_forges_page_says_its_words_and_folds_each_row() {
    let view = f::page_view(&root(Address::Forges, cf::connected()));
    cf::words_in_order(
        view.tree(),
        &[
            "Forges",
            "Your forges 1",
            "Each forge temper is connected to, or setting up. Add one to bring its repositories to temper.",
            "+ Add a forge",
        ],
    );
    let row = view
        .tree()
        .nodes()
        .iter()
        .find(|n| n.role == Role::Disclosure && n.name.as_deref() == Some(b"Forgejo on this machine"))
        .unwrap();
    assert!(!row.states.as_slice().contains(&State::Expanded));
    assert_eq!(row.description.as_deref(), Some("forge 3 · Forgejo · Connected as temper".as_bytes()));
    assert!(f::find(view.tree(), Role::Term, b"Account").is_some());
    assert!(f::find(view.tree(), Role::Description, "temper · user 42".as_bytes()).is_some());
}
#[test]
fn each_forgejo_mode_says_its_access_in_a_slot() {
    for (mode, words) in [
        (w::Mode::Owned, b"Site admin, with temper's own token".as_slice()),
        (w::Mode::Managed, b"Site admin, with a token you gave"),
        (w::Mode::Connected, b"Its bot's tokens, which you gave"),
    ] {
        let mut rows = List::with_capacity(1);
        rows.push(cf::row(HERE, w::Kind::Forgejo, mode, cf::connected(), w::Offer::NotOffered)).unwrap();
        let root = f::forge_root(Address::Forges, rows);
        let view = f::page_view(&root);
        assert!(view.tree().nodes().iter().enumerate().any(|(index, node)| {
            if node.role != Role::Slot {
                return false;
            }
            let slot = &view.tree().nodes()[index..index + usize::try_from(node.size).unwrap()];
            slot.iter().any(|n| n.role == Role::Term && n.name.as_deref() == Some(b"Access"))
                && slot.iter().any(|n| n.role == Role::Description && n.name.as_deref() == Some(words))
        }));
    }
}
#[test]
fn an_owned_forge_keeps_its_access_when_stopped() {
    let root = root(
        Address::Forges,
        w::Connection::Stopped(
            w::Stopped::new(&w::CEILINGS, w::StoppedParts { reason: b"token refused".as_slice().into() }).unwrap(),
        ),
    );
    let view = f::page_view(&root);
    assert!(f::find(view.tree(), Role::Term, b"Access").is_some());
    assert!(f::find(view.tree(), Role::Description, b"Site admin, with temper's own token").is_some());
}
#[test]
fn the_playground_forge_has_no_access_line() {
    let mut rows = List::with_capacity(1);
    rows.push(cf::row(wire::FORGE, w::Kind::Forgejo, w::Mode::Fake, cf::connected(), w::Offer::NotOffered)).unwrap();
    let root = f::demo_forges_root(Address::Forges, rows);
    let view = f::page_view(&root);
    assert!(f::find(view.tree(), Role::Heading { level: 2 }, b"The playground").is_some());
    assert!(f::find(view.tree(), Role::Term, b"Account").is_some());
    assert!(f::find(view.tree(), Role::Term, b"Access").is_none());
}
#[test]
fn a_forge_row_opened_offers_its_action() {
    let failed = w::Connection::SetUpFailed(
        w::SetUpFailed::new(&w::CEILINGS, w::SetUpFailedParts { why: b"fetch failed".as_slice().into(), at: None })
            .unwrap(),
    );
    let root = root(Address::Forges, failed);
    assert_eq!(action(&root, Role::Button, b"Set up again"), d::Action::Forge(d::forge::Act::SetUp { forge: HERE }));
    let view = f::page_view(&root);
    assert!(!view.tree().nodes().iter().any(|n| n.styles.as_slice().contains(&crate::look::FLOW_PRIMARY)));
    let row = view
        .tree()
        .nodes()
        .iter()
        .find(|n| n.role == Role::Disclosure && n.name.as_deref() == Some(b"Forgejo on this machine"))
        .unwrap();
    assert_eq!(row.description.as_deref(), Some("forge 3 · Forgejo · Set up failed: fetch failed".as_bytes()));
    let root = self::root(Address::Forges, w::Connection::Connecting);
    assert_eq!(
        action(&root, Role::Link, "Open its set-up ›".as_bytes()),
        go(Address::ConnectForge { forge: Some(Choice::Forge(HERE)) })
    );
    let root = self::root(Address::Forges, cf::connected());
    assert_eq!(action(&root, Role::Link, b"Adopt a repository"), go(Address::Adopt { project: f::PROJECT }));
    let root = self::root(
        Address::Forges,
        w::Connection::Stopped(
            w::Stopped::new(&w::CEILINGS, w::StoppedParts { reason: b"token refused".as_slice().into() }).unwrap(),
        ),
    );
    let view = f::page_view(&root);
    assert!(f::find(view.tree(), Role::Paragraph, b"Restart temper box: its preparation mends it").is_some());
}
#[test]
fn no_forge_of_yours_yet_offers_connect_a_forge() {
    let root = root(Address::Forges, w::Connection::NotSetUp);
    let view = f::page_view(&root);
    assert!(f::find(view.tree(), Role::Paragraph, b"No forge of yours yet.").is_some());
    for name in [b"+ Add a forge".as_slice(), b"Connect a forge"] {
        assert_eq!(action(&root, Role::Link, name), go(Address::ConnectForge { forge: None }));
    }
}
#[test]
fn the_navigation_has_forges_with_its_dot_and_words() {
    for (connection, style, text) in [
        (w::Connection::Connecting, crate::look::NAV_LIMITED, b"a forge is setting up".as_slice()),
        (
            w::Connection::Stopped(w::Stopped::new(&w::CEILINGS, w::StoppedParts { reason: Box::default() }).unwrap()),
            crate::look::NAV_NEEDS,
            b"a forge needs you",
        ),
    ] {
        for address in [Address::Forges, Address::ConnectForge { forge: None }] {
            let root = root(address, connection.clone());
            let view = f::page_view(&root);
            let nav = view
                .tree()
                .nodes()
                .iter()
                .find(|n| n.role == Role::Link && n.name.as_deref() == Some(b"Forges"))
                .unwrap();
            assert!(nav.states.as_slice().contains(&State::Current));
            let dot = view.tree().nodes().iter().find(|n| n.styles.as_slice().contains(&style)).unwrap();
            assert_eq!(dot.role, Role::Text);
            assert_eq!(f::nodes_text(core::slice::from_ref(dot)).as_ref(), text);
        }
    }
}
#[test]
fn the_setup_strip_replaces_the_demo_banner_and_says_whats_missing() {
    let root = root(Address::Chats { filter: ChatsFilter::All, start: false }, w::Connection::NotSetUp);
    let view = f::page_view(&root);
    assert!(f::find(view.tree(), Role::Region, b"Setup").is_some());
    assert!(f::find(view.tree(), Role::Region, b"Playground").is_none());
    assert!(f::find(view.tree(), Role::Paragraph, b"temper has no forge and no model yet.").is_some());
    assert_eq!(
        action(&root, Role::Link, b"Continue setting up"),
        go(Address::Welcome { step: Step::Forge { forge: None } })
    );
    for address in [
        Address::Accounts,
        Address::Welcome { step: Step::Model { way: None } },
        Address::Forges,
        Address::ConnectForge { forge: None },
    ] {
        let view = f::page_view(&self::root(address, w::Connection::NotSetUp));
        assert!(f::find(view.tree(), Role::Region, b"Setup").is_none());
    }
    let (mut root, accounts) = f::forge_root_accounts(
        Address::Chats { filter: ChatsFilter::All, start: false },
        cf::rows(cf::connected(), w::Offer::Offered),
    );
    let view = f::page_view(&root);
    assert!(f::find(view.tree(), Role::Paragraph, b"temper has no model yet.").is_some());
    f::welcome_account(&mut root, accounts);
    assert!(f::find(f::page_view(&root).tree(), Role::Region, b"Setup").is_none());
}
#[test]
fn the_demo_banners_connect_a_forge_is_a_link() {
    let root =
        f::demo_frame_root(Address::Chats { filter: ChatsFilter::All, start: false }, f::PROJECT, true, None, None);
    assert_eq!(action(&root, Role::Link, b"Connect a forge"), go(Address::ConnectForge { forge: None }));
    assert!(f::find(f::page_view(&root).tree(), Role::Button, b"Connect a forge").is_none());
}
#[test]
fn a_set_up_under_way_shows_in_either_banner_with_view() {
    for demo in [false, true] {
        let rows = cf::rows(w::Connection::Connecting, w::Offer::Offered);
        let root = if demo {
            f::demo_forges_root(Address::Chats { filter: ChatsFilter::All, start: false }, rows)
        } else {
            f::forge_root(Address::Chats { filter: ChatsFilter::All, start: false }, rows)
        };
        let view = f::page_view(&root);
        assert!(f::find(view.tree(), Role::Paragraph, "Forgejo on this machine · Connecting".as_bytes()).is_some());
        assert_eq!(action(&root, Role::Link, b"View"), go(Address::ConnectForge { forge: Some(Choice::Forge(HERE)) }));
        assert!(f::find(view.tree(), Role::Link, b"Connect a forge").is_none());
    }
}
#[test]
fn the_playground_forge_shows_while_the_forge_half_holds() {
    let mut rows = List::with_capacity(2);
    rows.push(cf::row(wire::FORGE, w::Kind::Forgejo, w::Mode::Fake, cf::connected(), w::Offer::NotOffered)).unwrap();
    rows.push(cf::row(HERE, w::Kind::Forgejo, w::Mode::Owned, w::Connection::Connecting, w::Offer::Offered)).unwrap();
    let mut root = f::demo_forges_root(Address::Forges, rows);
    assert!(d::forge::listed::playground(&root).is_some());
    let view = f::page_view(&root);
    let row = f::find(
        view.tree(),
        Role::Disclosure,
        "Playground forge · the playground’s, until a forge of yours connects · acme/shop".as_bytes(),
    )
    .unwrap();
    assert!(!view.tree().nodes().iter().find(|n| n.id == row).unwrap().states.as_slice().contains(&State::Expanded));
    f::root_event(
        &mut root,
        d::Event::Went { place: d::Place::Address(Address::Chats { filter: ChatsFilter::All, start: false }) },
    );
    assert!(d::forge::listed::playground(&root).is_some());
    let root = f::demo_forges_root(Address::Forges, cf::rows(cf::connected(), w::Offer::Offered));
    assert!(d::forge::listed::playground(&root).is_none());
    assert!(f::find(f::page_view(&root).tree(), Role::Heading { level: 2 }, b"The playground").is_none());
}

#[test]
fn set_up_again_waits_for_an_offer() {
    let failed = w::Connection::SetUpFailed(
        w::SetUpFailed::new(&w::CEILINGS, w::SetUpFailedParts { why: b"fetch failed".as_slice().into(), at: None })
            .unwrap(),
    );
    let root = f::forge_root(Address::Forges, cf::rows(failed, w::Offer::NotOffered));
    assert!(f::find(f::page_view(&root).tree(), Role::Button, b"Set up again").is_none());
}

#[test]
fn a_full_forge_reason_and_login_fit_after_utf8_replacement() {
    for connection in [
        w::Connection::Stopped(
            w::Stopped::new(&w::CEILINGS, w::StoppedParts { reason: Box::from([255_u8; 256]) }).unwrap(),
        ),
        w::Connection::Connected(
            w::Connected::new(&w::CEILINGS, w::ConnectedParts { id: u64::MAX, login: Box::from([255_u8; 256]) })
                .unwrap(),
        ),
    ] {
        let root = root(Address::Forges, connection);
        let domain = f::domain_limits();
        let words = f::words();
        let limits = crate::Limits::of(&domain, &words).unwrap();
        let mut view = crate::View::new(&limits, &domain, words, wire::FORGE);
        crate::build(&mut view, &root, f::NOW, &mut skein_lib::Queue::with_capacity(limits.tree.patches));
        let row = view
            .tree()
            .nodes()
            .iter()
            .find(|n| n.role == Role::Disclosure && n.name.as_deref() == Some(b"Forgejo on this machine"))
            .unwrap();
        let summary = core::str::from_utf8(row.description.as_deref().unwrap()).unwrap();
        assert_eq!(summary.matches('�').count(), 256_usize);
    }
}

#[test]
fn the_github_row_draws_its_app_installations_and_last_connect() {
    let root = f::forge_root(Address::Forges, cf::github_rows(true));
    let view = f::browser_page_view(&root);
    assert!(f::find(view.tree(), Role::Term, b"Access").is_none());
    cf::words_in_order(
        view.tree(),
        &[
            "Account",
            "my-app[bot] · user 9",
            "temper’s App",
            "my-app · ",
            "acme",
            ", installation 2 · suspended",
            ", installation 3 · not accepted · uninstalled at the next reconcile",
            "The last connect stopped: fixture stopped",
            "Install on another account",
        ],
    );
}
#[test]
fn install_on_another_account_follows_an_accepted_installation() {
    for (accepted, name) in [(true, b"Install on another account".as_slice()), (false, b"Install on GitHub")] {
        let root = f::forge_root(Address::Forges, cf::github_rows(accepted));
        let view = f::page_view(&root);
        let install = view
            .tree()
            .nodes()
            .iter()
            .find(|node| node.role == Role::Button && node.name.as_deref() == Some(name))
            .unwrap();
        let style = if accepted { crate::look::FLOW_SECONDARY } else { crate::look::FLOW_PRIMARY };
        assert!(install.styles.as_slice().contains(&style));
        assert_eq!(
            action(&root, Role::Button, name),
            d::Action::Forge(d::forge::Act::Install { forge: 2, back: b"/forges".as_slice().into() })
        );
    }
}
#[test]
fn all_sixty_four_installations_fit_without_shortening_their_logins() {
    let mut rows = cf::github_rows(true);
    let mut row = rows.as_slice()[0].clone().into_parts();
    rows.clear();
    row.installations = List::with_capacity(d::forge::forges::INSTALLATIONS);
    for id in 1..=u64::from(d::forge::forges::INSTALLATIONS) {
        row.installations
            .push(
                w::Installation::new(
                    &w::CEILINGS,
                    w::InstallationParts {
                        id,
                        account: Some(
                            w::Account::new(&w::CEILINGS, w::AccountParts { id, login: Box::from([255_u8; 256]) })
                                .unwrap(),
                        ),
                        suspended: true,
                        stray: true,
                    },
                )
                .unwrap(),
            )
            .unwrap();
    }
    rows.push(w::ForgeRow::new(&w::CEILINGS, row).unwrap()).unwrap();
    let root = f::forge_root(Address::Forges, rows);
    let domain = f::limits();
    let words = f::browser_words();
    let limits = crate::Limits::of(&domain, &words).unwrap();
    let mut view = crate::View::new(&limits, &domain, words, wire::FORGE);
    crate::build(&mut view, &root, f::NOW, &mut skein_lib::Queue::with_capacity(limits.tree.patches));
    let text = f::text(view.tree());
    let text = core::str::from_utf8(&text).unwrap();
    assert_eq!(text.matches('�').count(), 64 * 256);
}

#[test]
fn a_connected_forge_in_the_page_facts_opens_through_temper() {
    let root = root(Address::Forges, cf::connected());
    let mut view = f::view_of(f::browser_words());
    let mut facts = crate::forge_sign_in::Forges::new();
    facts.read("3 forgejo https://forge.example:3000 owned");
    view.forge_sign_in(facts);
    f::build(&mut view, &root);
    let link = f::find(view.tree(), Role::Link, "Open Forgejo ↗".as_bytes()).expect("forge sign-in link");
    assert_eq!(
        crate::href(&mut view, link).as_deref(),
        Some(b"https://forge.example:3000/user/oauth2/temper".as_slice())
    );
    assert!(
        core::str::from_utf8(&f::main_text(view.tree()))
            .unwrap()
            .contains("Open Forgejo signs you in as pat, through temper: Forgejo’s sign-in is temper’s.")
    );
    let view = f::browser_page_view(&root);
    assert!(f::find(view.tree(), Role::Link, "Open Forgejo ↗".as_bytes()).is_none());
}

#[test]
fn an_owned_forge_opens_at_the_public_origin_in_the_page_facts() {
    let root = root(Address::Forges, cf::connected());
    let mut view = f::view_of(f::browser_words());
    let mut facts = crate::forge_sign_in::Forges::new();
    facts.read("3 forgejo https://git.example.com owned");
    assert_eq!(facts.number(3).unwrap().name(&f::browser_words()).as_ref(), b"Forgejo on this machine");
    view.forge_sign_in(facts);
    f::build(&mut view, &root);
    let link = f::find(view.tree(), Role::Link, "Open Forgejo ↗".as_bytes()).expect("forge sign-in link");
    assert_eq!(crate::href(&mut view, link).as_deref(), Some(b"https://git.example.com/user/oauth2/temper".as_slice()));
}

fn owned_row(mode: w::Mode, connection: w::Connection, runner: Option<&[u8]>) -> w::ForgeRow {
    let mut parts = cf::row(HERE, w::Kind::Forgejo, mode, connection, w::Offer::Offered).into_parts();
    if parts.mode == w::Mode::Owned {
        parts.owned = Some(
            w::OwnedForge::new(
                &w::CEILINGS,
                w::OwnedForgeParts {
                    forgejo: b"17.0.0".as_slice().into(),
                    runner: runner.map(Into::into),
                    password: b"/fake/forge/person_password".as_slice().into(),
                },
            )
            .unwrap(),
        );
    }
    w::ForgeRow::new(&w::CEILINGS, parts).unwrap()
}
fn slot_has(tree: &skein_ui::Tree, label: &[u8], words: &[u8]) -> bool {
    tree.nodes().iter().enumerate().any(|(index, node)| {
        if node.role != Role::Slot {
            return false;
        }
        let slot = &tree.nodes()[index..index.checked_add(usize::try_from(node.size).unwrap()).unwrap()];
        slot.iter().any(|n| n.role == Role::Term && n.name.as_deref() == Some(label))
            && slot.iter().any(|n| n.role == Role::Description && n.name.as_deref() == Some(words))
    })
}
#[test]
fn owned_ready_rows_say_the_versions_and_first_password_path_in_slots() {
    for (runner, runs) in [
        (
            Some(b"13.2.0".as_slice()),
            b"Forgejo 17.0.0 and its runner 13.2.0, on this machine. CI jobs run here, as you.".as_slice(),
        ),
        (None, b"Forgejo 17.0.0, on this machine, with no runner.".as_slice()),
    ] {
        for connection in [
            cf::connected(),
            w::Connection::Connecting,
            w::Connection::Stopped(
                w::Stopped::new(&w::CEILINGS, w::StoppedParts { reason: b"stopped".as_slice().into() }).unwrap(),
            ),
        ] {
            let mut rows = List::with_capacity(1);
            rows.push(owned_row(w::Mode::Owned, connection, runner)).unwrap();
            let root = f::forge_root(Address::Forges, rows);
            let view = f::page_view(&root);
            assert!(slot_has(view.tree(), b"Runs", runs));
            assert!(slot_has(
                view.tree(),
                b"First password",
                b"Your first password is in /fake/forge/person_password. Change it on Forgejo when you like."
            ));
        }
    }
}
#[test]
fn unprepared_owned_rows_and_managed_rows_have_no_runs_or_password_lines() {
    for (mode, connection) in [
        (w::Mode::Owned, w::Connection::NotConnected),
        (w::Mode::Owned, w::Connection::NotSetUp),
        (
            w::Mode::Owned,
            w::Connection::SettingUp(w::SettingUp::new(&w::CEILINGS, w::SettingUpParts { step: None }).unwrap()),
        ),
        (
            w::Mode::Owned,
            w::Connection::SetUpFailed(
                w::SetUpFailed::new(&w::CEILINGS, w::SetUpFailedParts { why: b"failed".as_slice().into(), at: None })
                    .unwrap(),
            ),
        ),
        (w::Mode::Managed, cf::connected()),
    ] {
        let mut rows = List::with_capacity(1);
        rows.push(owned_row(mode, connection, Some(b"13.2.0"))).unwrap();
        let root = f::forge_root(Address::Forges, rows);
        let view = f::page_view(&root);
        for label in [b"Runs".as_slice(), b"First password"] {
            assert!(f::find(view.tree(), Role::Term, label).is_none());
        }
        assert!(
            f::find(
                view.tree(),
                Role::Description,
                b"Your first password is in /fake/forge/person_password. Change it on Forgejo when you like."
            )
            .is_none()
        );
    }
}

#[test]
fn the_github_app_line_names_an_empty_installation_set_only_when_empty() {
    for empty in [false, true] {
        let mut parts = cf::github_rows(true).get(0).unwrap().clone().into_parts();
        if empty {
            parts.installations.clear();
        }
        let mut rows = List::with_capacity(1);
        rows.push(w::ForgeRow::new(&w::CEILINGS, parts).unwrap()).unwrap();
        let root = f::forge_root(Address::Forges, rows);
        let view = f::browser_page_view(&root);
        assert!(f::find(view.tree(), Role::Term, "temper’s App".as_bytes()).is_some());
        assert_eq!(f::find(view.tree(), Role::Slot, "my-app · installed nowhere yet".as_bytes()).is_some(), empty);
        assert_eq!(f::find(view.tree(), Role::Slot, b"acme").is_some(), !empty);
    }
}

#[test]
fn the_github_rows_install_block_follows_the_apps_way_and_acceptance() {
    for app in [None, Some(w::AppWay::Manifest), Some(w::AppWay::Pasted)] {
        for accepted in [false, true] {
            let mut parts = cf::github_rows(accepted).get(0).unwrap().clone().into_parts();
            parts.app = app.clone();
            let mut rows = List::with_capacity(1);
            rows.push(w::ForgeRow::new(&w::CEILINGS, parts).unwrap()).unwrap();
            let root = f::forge_root(Address::Forges, rows);
            let view = f::browser_page_view(&root);
            let manifest = "my-app is made on GitHub, under your account, and installed nowhere yet. Install it, and choose the repositories temper may use.";
            let pasted = "my-app is connected to temper, and installed nowhere yet. Install it, and choose the repositories temper may use.";
            let explanation = if app == Some(w::AppWay::Manifest) { manifest } else { pasted };
            assert_eq!(
                f::find(view.tree(), Role::Heading { level: 2 }, "Install temper’s App".as_bytes()).is_some(),
                !accepted
            );
            for words in [manifest, pasted] {
                assert_eq!(
                    f::find(view.tree(), Role::Paragraph, words.as_bytes()).is_some(),
                    !accepted && words == explanation
                );
            }
            if !accepted {
                cf::words_in_order(
                    view.tree(),
                    &[
                        "temper’s App",
                        "The last connect stopped: fixture stopped",
                        "Install temper’s App",
                        explanation,
                        "Install on GitHub",
                    ],
                );
            }
        }
    }
}

#[test]
fn the_github_row_opens_its_configured_web_address_only_when_carried() {
    for kind in [w::Kind::Github, w::Kind::Forgejo] {
        for web in [None, Some(b"https://github.test/base/?a=1&b=2".as_slice())] {
            let mut parts = cf::github_rows(true).get(0).unwrap().clone().into_parts();
            parts.kind = kind.clone();
            parts.web = web.map(Into::into);
            let mut rows = List::with_capacity(1);
            rows.push(w::ForgeRow::new(&w::CEILINGS, parts).unwrap()).unwrap();
            let root = f::forge_root(Address::Forges, rows);
            let view = f::browser_page_view(&root);
            let link = f::find(view.tree(), Role::Link, "Open on GitHub ↗".as_bytes());
            assert_eq!(link.is_some(), kind == w::Kind::Github && web.is_some());
            if let Some(link) = link {
                let node = view.tree().nodes().iter().find(|node| node.id == link).unwrap();
                assert_eq!(node.external.as_deref(), web);
            }
        }
    }
}

#[test]
fn a_stopped_github_row_has_no_app_or_install_block() {
    let mut parts = cf::github_rows(false).get(0).unwrap().clone().into_parts();
    parts.connection = w::Connection::Stopped(
        w::Stopped::new(&w::CEILINGS, w::StoppedParts { reason: b"fixture stopped".as_slice().into() }).unwrap(),
    );
    parts.installations.clear();
    let mut rows = List::with_capacity(1);
    rows.push(w::ForgeRow::new(&w::CEILINGS, parts).unwrap()).unwrap();
    let root = f::forge_root(Address::Forges, rows);
    let view = f::browser_page_view(&root);
    assert!(f::find(view.tree(), Role::Term, "temper’s App".as_bytes()).is_none());
    assert!(f::find(view.tree(), Role::Slot, "my-app · installed nowhere yet".as_bytes()).is_none());
    assert!(f::find(view.tree(), Role::Heading { level: 2 }, "Install temper’s App".as_bytes()).is_none());
    assert!(f::find(view.tree(), Role::Button, b"Install on GitHub").is_none());
    assert!(f::find(view.tree(), Role::Link, b"Connect an App of your own").is_some());
}

#[test]
fn the_owned_repository_body_names_its_project_without_a_numeric_made_line() {
    let root = f::forge_root(Address::Forges, cf::first_rows(cf::connected(), cf::made()));
    let view = f::page_view(&root);
    assert!(slot_has(view.tree(), b"Repositories", "work/hello · project Shop".as_bytes()));
    let row = view
        .tree()
        .nodes()
        .iter()
        .find(|n| n.role == Role::Disclosure && n.name.as_deref() == Some(b"Forgejo on this machine"))
        .unwrap();
    assert!(row.description.as_deref().unwrap().ends_with("work/hello · project Shop".as_bytes()));
    assert!(!String::from_utf8_lossy(&f::text(view.tree())).contains("made, in 1"));
}
#[test]
fn a_repository_making_or_failed_keeps_its_existing_body() {
    for (state, message) in [
        (w::FirstState::Making, "Making work/hello"),
        (
            w::FirstState::Failed(
                w::CreateFailed::new(&w::CEILINGS, w::CreateFailedParts { step: w::CreateStep::Repository }).unwrap(),
            ),
            "Making work/hello failed: making the repository",
        ),
    ] {
        let root = f::forge_root(Address::Forges, cf::first_rows(cf::connected(), state));
        let view = f::page_view(&root);
        assert!(f::find(view.tree(), Role::Paragraph, message.as_bytes()).is_some());
        assert!(f::find(view.tree(), Role::Term, b"Repositories").is_none());
    }
}

#[test]
fn a_pasted_app_hides_the_setup_strips_playground_before_installation_and_the_retirement_read() {
    let mut rows = cf::github_rows(false);
    let mut parts = rows.get(0).unwrap().clone().into_parts();
    parts.app = Some(w::AppWay::Pasted);
    parts.ended = None;
    parts.installations.clear();
    *rows.get_mut(0).unwrap() = w::ForgeRow::new(&w::CEILINGS, parts).unwrap();
    let root = f::demo_forges_state_root(
        Address::Chats { filter: ChatsFilter::All, start: false },
        rows,
        w::DemoState::NotStarted,
    );
    assert!(root.demo_answered());
    assert!(root.forge().demo().is_some(), "retirement has not been reread yet");
    let setup = d::forge::listed::setup(&root).expect("installation and model remain to set up");
    assert!(!setup.playground, "a connected App ends the playground offer before installation");
    let view = f::page_view(&root);
    assert!(f::find(view.tree(), Role::Region, b"Setup").is_some());
    assert!(f::find(view.tree(), Role::Link, b"Try the playground").is_none());
}
