#![allow(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros,
    reason = "test assertions inspect semantic trees and diagnostic redaction"
)]
use super::{connect_model::text, fixtures};
use jig_client::contract as c;
use skein_lib::{List, Token};
use skein_ui::{Act, Role};
use temper_client_domain::{
    self as d,
    pages::accounts::{Act as A, Operation, Row},
};
const PROVIDER: u32 = 17;
const SOURCE: u32 = 43;
const ADDRESS: &[u8] = b"https://issuer.example/visit";
pub(super) fn card_document(provider: &[u8], display: &[u8], kind: c::OwnerAccountKind) -> c::OwnerAccounts {
    let mut doc = document(1);
    let row = doc.accounts.get_mut(0).unwrap();
    row.provider_name = Some(text(provider));
    row.display = text(display);
    row.plan = kind != c::OwnerAccountKind::Key;
    let source = row.sources.get_mut(0).unwrap();
    source.kind = kind;
    if kind == c::OwnerAccountKind::Program {
        source.name = Some(Box::from(display));
    }
    doc
}
fn card_heading(doc: c::OwnerAccounts) -> Vec<u8> {
    let (mut root, watch) = fixtures::account_root();
    deliver(&mut root, watch, doc);
    let mut view = fixtures::view();
    fixtures::build(&mut view, &root)
        .nodes()
        .iter()
        .find(|node| node.role == Role::Heading { level: 3 })
        .unwrap()
        .name
        .as_ref()
        .unwrap()
        .to_vec()
}
#[test]
fn a_cards_region_is_named_by_its_heading() {
    let (mut root, watch) = fixtures::account_root();
    deliver(&mut root, watch, card_document(b"openai", b"alice@example.com", c::OwnerAccountKind::SignIn));
    let mut view = fixtures::view();
    let tree = fixtures::build(&mut view, &root);
    let heading = tree.nodes().iter().find(|node| node.role == Role::Heading { level: 3 }).unwrap();
    assert!(tree.nodes().iter().any(|node| node.role == Role::Disclosure && node.name == heading.name));
}
#[test]
fn a_plan_card_names_the_plan_display_brand_and_holder() {
    assert_eq!(
        card_heading(card_document(b"openai", b"alice@example.com", c::OwnerAccountKind::SignIn)),
        "ChatGPT · alice@example.com · OpenAI · held by Pat".as_bytes()
    );
}
#[test]
fn a_key_only_card_names_the_brand_and_holder_once() {
    assert_eq!(
        card_heading(card_document(b"openai", b"", c::OwnerAccountKind::Key)),
        "OpenAI API key · held by Pat".as_bytes()
    );
}
#[test]
fn a_program_card_names_the_subscription_and_login() {
    let mut doc = card_document(b"anthropic", b"claude-alice", c::OwnerAccountKind::Program);
    // A program declaration has no provider display; its portable name belongs to the source.
    doc.accounts.get_mut(0).unwrap().display = text(b"");
    assert_eq!(card_heading(doc), "Claude subscription · claude-alice · Anthropic · held by Pat".as_bytes());
}
#[test]
fn a_deployment_card_names_its_holder_in_words() {
    let mut doc = card_document(b"openai", b"", c::OwnerAccountKind::Key);
    doc.accounts.get_mut(0).unwrap().holder = c::OwnerAccountHolder::Deployment;
    assert_eq!(card_heading(doc), "OpenAI API key · held by the deployment".as_bytes());
}
#[test]
fn an_empty_display_leaves_no_empty_heading_part() {
    assert_eq!(
        card_heading(card_document(b"openai", b"", c::OwnerAccountKind::SignIn)),
        "ChatGPT · OpenAI · held by Pat".as_bytes()
    );
}
#[test]
fn sources_name_their_kinds_in_the_providers_words() {
    for (provider, display, kind, expected) in [
        (b"openai".as_slice(), b"alice@example.com".as_slice(), c::OwnerAccountKind::SignIn, "ChatGPT sign-in · Ready"),
        (b"openai", b"", c::OwnerAccountKind::Key, "API key · Ready"),
        (b"anthropic", b"claude-alice", c::OwnerAccountKind::Program, "Claude Code login claude-alice · Ready"),
    ] {
        assert!(shown(card_document(provider, display, kind)).contains(expected));
    }
}
#[test]
fn account_management_keeps_both_account_lists() {
    let (root, _) = owner_root();
    let mut view = fixtures::view();
    let tree = fixtures::build(&mut view, &root);
    let headings: Vec<_> = tree
        .nodes()
        .iter()
        .filter(|node| node.role == Role::Heading { level: 2 })
        .map(|node| node.name.as_ref().unwrap().to_vec())
        .collect();
    assert_eq!(headings, [b"Your accounts".to_vec(), b"Deployment accounts".to_vec()]);
}
#[test]
fn your_accounts_heading_row_holds_its_count_and_the_connect_link() {
    let (mut root, watch) = fixtures::account_root();
    deliver(&mut root, watch, card_document(b"openai", b"Pat", c::OwnerAccountKind::SignIn));
    let mut view = fixtures::view();
    let tree = fixtures::build(&mut view, &root);
    let row = super::inbox::subtree(tree, super::inbox::styled(tree, crate::look::HEADING));
    assert!(row.iter().any(
        |node| node.role == Role::Heading { level: 2 } && node.name.as_deref() == Some(b"Your accounts".as_slice())
    ));
    assert!(row.iter().any(|node| node.styles.as_slice().contains(&crate::look::SUBTLE_COUNT)));
    assert!(
        row.iter().any(|node| node.role == Role::Link && node.name.as_deref() == Some(b"+ Connect a model".as_slice()))
    );
}
#[test]
fn a_cut_provider_name_keeps_the_numbered_card_fallback() {
    let mut doc = card_document(b"openai", b"Pat", c::OwnerAccountKind::SignIn);
    doc.accounts.get_mut(0).unwrap().provider_name.as_mut().unwrap().cut = true;
    assert_eq!(card_heading(doc), "Account 1 · LLM provider 17 · Pat".as_bytes());
}
#[test]
fn an_unknown_provider_keeps_its_configured_name() {
    assert_eq!(
        card_heading(card_document(b"Custom", b"Pat", c::OwnerAccountKind::SignIn)),
        "Custom · Pat · Custom · held by Pat".as_bytes()
    );
}
fn source(number: u32) -> c::OwnerAccountSource {
    c::OwnerAccountSource {
        name: None,
        number,
        place: number,
        kind: c::OwnerAccountKind::Key,
        state: c::OwnerAccountState::Granted,
        usable: true,
        expiring: false,
        attention: false,
        spent_until: None,
    }
}
pub(super) fn document(count: u32) -> c::OwnerAccounts {
    let mut accounts = List::with_capacity(count);
    let mut signing = List::with_capacity(count);
    for number in 1..=count {
        let mut sources = List::with_capacity(count);
        for n in 1..=count {
            sources.push(source(n)).unwrap();
        }
        accounts
            .push(c::OwnerAccountLine {
                number,
                place: number,
                uses: None,
                plan: true,
                provider: Some(PROVIDER),
                provider_name: Some(c::DocumentText::complete(
                    c::Text::new(Box::from(b"Scripted".as_slice()), 32).unwrap(),
                )),
                manage_usage: None,
                display: c::DocumentText::complete(c::Text::new(Box::from(b"Pat".as_slice()), 32).unwrap()),
                holder: c::OwnerAccountHolder::Party(73),
                sources,
                cut: true,
            })
            .unwrap();
        signing
            .push(c::OwnerAccountSignIn {
                source: number,
                provider: PROVIDER,
                holder: c::OwnerAccountHolder::Party(73),
            })
            .unwrap();
    }
    c::OwnerAccounts {
        program_hosts: List::with_capacity(0),
        readiness: List::with_capacity(0),
        readiness_cut: false,
        accounts,
        signing,
        last_end: None,
        usable: true,
        settled: false,
        cut: true,
    }
}
pub(super) fn deliver(root: &mut d::Domain, watch: Token, doc: c::OwnerAccounts) {
    fixtures::root_event(
        root,
        d::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched {
                    watch,
                    event: c::WatchEvent::Delivered(c::Delivery::OwnerAccounts(Box::new(doc))),
                },
            },
        },
    );
}
fn action(root: &mut d::Domain, act: A) {
    fixtures::root_event(root, d::Event::Act { action: d::Action::Pages(d::pages::Act::Accounts(act)) });
}
pub(super) fn press(view: &mut crate::View, root: &mut d::Domain, role: Role, name: &[u8], act: Act) {
    let node = fixtures::find(fixtures::build(view, root), role, name).expect("offered account control");
    let event = crate::event(view, node, act).expect("account control decodes");
    if let d::Event::Act { action: d::Action::Pages(d::pages::Act::Go { address }) } = event {
        fixtures::root_event(root, d::Event::Went { place: d::Place::Address(address) });
    } else {
        fixtures::root_event(root, event);
    }
}
fn shown(doc: c::OwnerAccounts) -> String {
    shown_at(doc, fixtures::limits().accounts)
}
fn shown_at(doc: c::OwnerAccounts, counts: d::limits::AccountLimits) -> String {
    let (mut root, watch) = fixtures::account_root();
    deliver(&mut root, watch, doc);
    let mut view = bounded_view(counts);
    String::from_utf8(fixtures::text(fixtures::build(&mut view, &root)).into_vec()).unwrap()
}
fn bounded_view(counts: d::limits::AccountLimits) -> crate::View {
    let mut domain = fixtures::limits();
    domain.accounts = counts;
    let words = fixtures::words();
    let bounds = crate::Limits::of(&domain, &words).unwrap();
    crate::View::new(&bounds, &domain, words, fixtures::FORGE)
}
#[test]
fn the_accounts_page_keeps_accounts_beyond_rows() {
    assert!(account_order(document(3)).contains(&3));
}
#[test]
fn the_accounts_page_keeps_sources_beyond_rows() {
    assert_eq!(shown(document(3)).matches("3. API key").count(), 3);
}
#[test]
fn the_accounts_page_keeps_sign_ins_beyond_rows() {
    assert!(shown(document(3)).contains("Sign-in in progress: source 3"));
}
#[test]
fn a_sign_in_without_an_account_row_stays_visible_at_page_level() {
    for count in [0, 1] {
        let (mut root, watch) = fixtures::account_root();
        let mut doc = document(count);
        doc.signing = List::with_capacity(1);
        doc.signing
            .push(c::OwnerAccountSignIn {
                source: SOURCE,
                provider: PROVIDER,
                holder: c::OwnerAccountHolder::Party(73),
            })
            .unwrap();
        deliver(&mut root, watch, doc);
        let mut view = fixtures::view();
        let tree = fixtures::build(&mut view, &root);
        let status = fixtures::person_find(
            tree,
            Role::Status,
            b"Sign-in in progress: source 43 \xc2\xb7 LLM provider 17 \xc2\xb7 Person 73",
        )
        .expect("another client's sign-in stays visible before it has an account row");
        let at = tree.nodes().iter().position(|node| node.id == status).unwrap();
        assert!(!tree.nodes().iter().enumerate().any(|(start, node)| {
            node.role == Role::Disclosure && start < at && at < start + usize::try_from(node.size).unwrap()
        }));
    }
}
#[test]
fn a_sign_in_in_an_omitted_row_stays_visible_without_duplicating_drawn_rows() {
    let doc = document(1);
    let counts = fixtures::limits().accounts;
    assert_eq!(shown_at(doc.clone(), counts).matches("Sign-in in progress: source 1").count(), 1);
    let shown = shown_at(doc, d::limits::AccountLimits { accounts: 0, ..counts });
    assert!(shown.contains("Sign-in in progress: source 1 · LLM provider 17 · Person 73"));
}
#[test]
fn the_accounts_page_preserves_supplied_order() {
    let mut doc = document(3);
    let first = doc.accounts.get(0).unwrap().clone();
    let third = doc.accounts.get(2).unwrap().clone();
    *doc.accounts.get_mut(0).unwrap() = third;
    *doc.accounts.get_mut(2).unwrap() = first;
    assert_eq!(account_order(doc), [3, 2, 1]);
}
#[test]
fn the_accounts_page_names_cut_facts() {
    assert!(shown(document(1)).contains("Some account facts were omitted."));
}
#[test]
fn the_accounts_page_caps_uses() {
    let mut doc = document(1);
    let mut uses = List::with_capacity(300);
    for party in 1..=300 {
        uses.push(c::OwnerAccountUse::Party(party)).unwrap();
    }
    doc.accounts.get_mut(0).unwrap().uses = Some(uses);
    let text = shown(doc);
    assert!(text.contains("Uses left out: 292"));
    assert!(!text.contains("Person 9;"));
}
fn key_draft() -> (d::Domain, crate::View) {
    let (mut root, watch) = fixtures::account_root();
    deliver(&mut root, watch, card_document(b"openai", b"alice@example.com", c::OwnerAccountKind::SignIn));
    let mut view = fixtures::view();
    press(&mut view, &mut root, Role::Button, b"Add an API key", Act::Press);
    press(
        &mut view,
        &mut root,
        Role::TextField,
        b"Key (pasted once)",
        Act::Input { value: Box::from(b"private-once".as_slice()) },
    );
    (root, view)
}
fn key_field(view: &crate::View) -> &skein_ui::Node {
    view.tree().nodes().iter().find(|n| n.name.as_deref() == Some(b"Key (pasted once)")).unwrap()
}
#[test]
fn the_key_field_declares_a_secret_entry() {
    let (_, view) = key_draft();
    assert_eq!(key_field(&view).entry, Some(skein_ui::EntryKind::Secret));
}
#[test]
fn a_key_draft_never_enters_the_view() {
    let (_, view) = key_draft();
    assert!(!format!("{view:?}").contains("private-once"));
}
#[test]
fn a_held_key_has_an_empty_view_value() {
    let (root, view) = key_draft();
    assert!(root.pages().accounts().unwrap().request.has_key());
    assert!(key_field(&view).value.as_ref().unwrap().text.is_empty());
}
#[test]
fn sending_a_key_writes_an_empty_field() {
    let (mut root, mut view) = key_draft();
    let before = key_field(&view).value.as_ref().unwrap().written;
    press(&mut view, &mut root, Role::Button, b"Add key", Act::Press);
    fixtures::build(&mut view, &root);
    let value = key_field(&view).value.as_ref().unwrap();
    assert!(value.text.is_empty());
    assert_ne!(value.written, before);
}
pub(super) fn sign_in_address(address: &[u8]) -> d::Domain {
    let mut root = super::connect_model::offered_root();
    let limits = fixtures::limits();
    let env = skein_lib::Env { now: skein_lib::Time::ZERO, wall: skein_lib::Wall::EPOCH, limits };
    let mut out = skein_lib::Queue::with_capacity(d::max_out(&limits));
    d::step(
        &mut root,
        &env,
        d::Event::Act {
            action: d::Action::Pages(d::pages::Act::ConnectModel(d::pages::connect_model::Act::SignIn {
                name: [97; 16],
                way: d::pages::accounts::Way::SignIn { provider: PROVIDER, plan: true },
            })),
        },
        &mut out,
    );
    fixtures::root_event(
        &mut root,
        d::Event::Went {
            place: d::Place::Address(d::Address::ConnectModel {
                way: Some(d::ModelWay { source: None, provider: PROVIDER, kind: d::WayKind::SignIn }),
            }),
        },
    );
    d::step(&mut root, &env, d::Event::Client { event: jig_client::Event::Stored }, &mut out);
    let attempt = out
        .iter()
        .find_map(|request| match request {
            d::Request::Client {
                request: jig_client::Request::Down { down: jig_client::Down::Send { attempt, .. } },
            } => Some(*attempt),
            d::Request::Client { .. }
            | d::Request::Show { .. }
            | d::Request::Leave { .. }
            | d::Request::SignOut
            | d::Request::Dropped { .. }
            | d::Request::Fact { .. } => None,
        })
        .expect("sign-in attempt");
    fixtures::root_event(
        &mut root,
        d::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Answer {
                    attempt,
                    envelope: skein_lib::RequestEnvelope::Final,
                    answer: Some(jig_client::Answered::Jig(c::Answer::Done(c::Outcome::Owner(
                        c::OwnerOutcome::AccountSignInStarted { source: SOURCE, address: Box::from(address) },
                    )))),
                    why: None,
                },
            },
        },
    );
    root
}
#[test]
fn a_sign_in_address_has_a_visible_link_word() {
    let root = sign_in_address(ADDRESS);
    let mut view = fixtures::view();
    let text = fixtures::text(fixtures::build(&mut view, &root));
    assert!(String::from_utf8(text.into_vec()).unwrap().contains("Open the sign-in page"));
}
#[test]
fn a_sign_in_address_names_the_engine_machine() {
    let root = sign_in_address(ADDRESS);
    let mut view = fixtures::view();
    let text = fixtures::text(fixtures::build(&mut view, &root));
    assert!(String::from_utf8(text.into_vec()).unwrap().contains("on the engine's machine"));
}
#[test]
fn an_address_containing_a_space_is_refused() {
    unsafe_address(b"https://issuer.example/bad path");
}
#[test]
fn an_uppercase_scheme_is_refused() {
    unsafe_address(b"HTTPS://issuer.example/visit");
}
fn unsafe_address(address: &[u8]) {
    let root = sign_in_address(address);
    let mut view = fixtures::view();
    let tree = fixtures::build(&mut view, &root);
    assert!(!tree.nodes().iter().any(|n| n.external.is_some()));
    assert!(String::from_utf8(fixtures::text(tree).into_vec()).unwrap().contains("not an http(s) address"));
}
#[test]
fn a_long_sign_in_address_is_refused() {
    let address = format!("https://issuer.example/{}", "x".repeat(1000));
    let root = sign_in_address(address.as_bytes());
    let mut view = fixtures::view();
    let tree = fixtures::build(&mut view, &root);
    assert!(!tree.nodes().iter().any(|n| n.external.is_some()));
    let text = String::from_utf8(fixtures::text(tree).into_vec()).unwrap();
    assert!(text.contains(&format!("longer than this client shows; bytes: {}", address.len())));
}

#[test]
fn supplied_source_states_keep_their_words() {
    use c::OwnerAccountState as S;
    let (mut root, watch) = fixtures::account_root();
    let mut view = fixtures::view();
    for (state, expected) in [
        (S::Asking, b"Checking".as_slice()),
        (S::Granted, b"Ready"),
        (S::Later { until: None }, b"Limited. Tasks on it wait."),
        (S::SignInAgain, b"Needs sign-in"),
        (S::Misconfigured, b"Check the configuration's registration."),
        (S::Cancelled, b"Cancelled"),
        (S::Closing, b"Closing"),
        (S::Moving, b"Moving to another deployment"),
        (
            S::Unloaded { at: c::OwnerAccountPlace::File, why: c::OwnerAccountUnread::Stalled },
            b"Cannot load File: Stalled",
        ),
    ] {
        let mut doc = document(1);
        doc.accounts.get_mut(0).unwrap().sources.get_mut(0).unwrap().state = state;
        deliver(&mut root, watch, doc);
        let shown = fixtures::text(fixtures::build(&mut view, &root));
        assert!(skein_lib::bytes::find(&shown, expected).is_some(), "{state:?}");
        assert!(skein_lib::bytes::find(&shown, b"Limited until").is_none(), "no invented reset");
    }
}
#[test]
fn move_refusals_keep_their_words() {
    use c::{OwnerAccountEnd as E, OwnerAccountRefusal as R};
    for (refusal, expected) in [
        (R::NotSignedIn, b"Not signed in".as_slice()),
        (R::NotOurs, b"Not this application's record"),
        (R::Cancelled, b"The move was cancelled"),
        (R::Moving, b"Moving to another deployment"),
    ] {
        assert_eq!(sign_in_end_words(document(0), E::Refused { refusal }), expected, "{refusal:?}");
    }
}
#[test]
fn a_supplied_reset_is_visible() {
    let (mut root, watch) = fixtures::account_root();
    let mut view = fixtures::view();
    let mut doc = document(1);
    doc.accounts.get_mut(0).unwrap().sources.get_mut(0).unwrap().state =
        c::OwnerAccountState::Later { until: Some(skein_lib::Wall::from_nanos(1_791_828_240_000_000_000)) };
    deliver(&mut root, watch, doc);
    assert!(
        skein_lib::bytes::find(&fixtures::text(fixtures::build(&mut view, &root)), b"Limited until Oct 12, 18:04 UTC")
            .is_some()
    );
}
#[test]
fn sign_in_ends_preserve_their_typed_details() {
    use c::{OwnerAccountEnd as E, OwnerAccountFailure as F, OwnerAccountRefusal as R};
    for (end, expected) in [
        (E::Made { account: 29, declined: true }, b"You signed in without plan usage.".as_slice()),
        (E::Renewed { account: 29, declined: false }, b"same account again; account 29"),
        (E::Refused { refusal: R::NotKept { why: c::OwnerAccountKeyFailure::TooLong } }, b"The key is too long."),
        (E::Refused { refusal: R::Limit { bound: c::OwnerAccountBound::Sources } }, b"Source limit reached."),
        (E::Refused { refusal: R::Kind { kind: c::OwnerAccountKind::Program } }, b"Source kind refused: Program login"),
        (E::Failed { failure: F::MissingScope { scope: 19 } }, b"Missing required scope 19"),
        (E::Failed { failure: F::Later { after: Some(skein_lib::Duration::from_secs(5)) } }, b"Try again after 5"),
    ] {
        let mut doc = document(0);
        doc.last_end = Some(c::OwnerAccountLastEnd { source: SOURCE, end });
        assert!(skein_lib::bytes::find(&sign_in_end_words(doc, end), expected).is_some());
    }
}
fn sign_in_end_words(mut doc: c::OwnerAccounts, end: c::OwnerAccountEnd) -> Vec<u8> {
    doc.last_end = Some(c::OwnerAccountLastEnd { source: SOURCE, end });
    let mut b = skein_ui::Builder::new(&fixtures::tree_limits());
    b.open(Role::Main);
    crate::pages::accounts::end(&mut b, &fixtures::words(), &end, &doc.accounts);
    b.close();
    let tree = b.finish();
    for (index, status) in tree.nodes().iter().enumerate() {
        if status.role != Role::Status {
            continue;
        }
        let node = tree.nodes().get(index.checked_add(1).unwrap()).unwrap();
        let mut words = Vec::new();
        for run in &node.text {
            words.extend_from_slice(&run.text);
        }
        if words.starts_with(b"Signed in") || words.starts_with("That’s".as_bytes()) {
            return words;
        }
    }
    fixtures::text(&tree).into_vec()
}
#[test]
fn a_completed_sign_in_names_the_plan_and_display() {
    assert_eq!(
        sign_in_end_words(
            card_document(b"openai", b"alice@example.com", c::OwnerAccountKind::SignIn),
            c::OwnerAccountEnd::Made { account: 1, declined: false }
        ),
        b"Signed in to ChatGPT as alice@example.com"
    );
}
#[test]
fn a_renewed_sign_in_explains_how_to_add_another_account() {
    assert_eq!(sign_in_end_words(card_document(b"openai", b"alice@example.com", c::OwnerAccountKind::SignIn), c::OwnerAccountEnd::Renewed { account: 1, declined: false }), "That’s alice@example.com again. To add another account, sign out of ChatGPT in your browser first, then Continue with ChatGPT.".as_bytes());
}
#[test]
fn a_sign_in_end_keeps_the_number_when_its_account_has_not_arrived() {
    for (end, expected) in [
        (c::OwnerAccountEnd::Made { account: 29, declined: false }, b"Signed in; account 29".as_slice()),
        (
            c::OwnerAccountEnd::Renewed { account: 29, declined: false },
            b"Signed in to the same account again; account 29",
        ),
    ] {
        assert_eq!(sign_in_end_words(document(0), end), expected);
    }
}
#[test]
fn a_sign_in_end_without_a_display_names_the_plan_unless_renewed() {
    for (end, expected) in [
        (c::OwnerAccountEnd::Made { account: 1, declined: false }, b"Signed in to ChatGPT".as_slice()),
        (
            c::OwnerAccountEnd::Renewed { account: 1, declined: false },
            b"Signed in to the same account again; account 1",
        ),
    ] {
        assert_eq!(sign_in_end_words(card_document(b"openai", b"", c::OwnerAccountKind::SignIn), end), expected);
    }
}
#[test]
fn a_named_sign_in_end_still_reports_declined_plan_usage() {
    assert!(
        sign_in_end_words(
            card_document(b"openai", b"alice@example.com", c::OwnerAccountKind::SignIn),
            c::OwnerAccountEnd::Made { account: 1, declined: true }
        )
        .ends_with(b"You signed in without plan usage.")
    );
}
fn answer_account_operation(root: &mut d::Domain, operation: Operation, outcome: c::OwnerOutcome) {
    let limits = fixtures::limits();
    let env = skein_lib::Env { now: skein_lib::Time::ZERO, wall: skein_lib::Wall::EPOCH, limits };
    let mut out = skein_lib::Queue::with_capacity(d::max_out(&limits));
    for act in [
        A::KeyFor { row: Row { holder: c::AccountHolder::Own, account: 1 }, source: None },
        A::Submit { name: [98; 16], row: Row { holder: c::AccountHolder::Own, account: 1 }, operation },
        A::Confirm,
    ] {
        d::step(root, &env, d::Event::Act { action: d::Action::Pages(d::pages::Act::Accounts(act)) }, &mut out);
    }
    d::step(root, &env, d::Event::Client { event: jig_client::Event::Stored }, &mut out);
    let attempt = out
        .iter()
        .find_map(|request| match request {
            d::Request::Client {
                request: jig_client::Request::Down { down: jig_client::Down::Send { attempt, .. } },
            } => Some(*attempt),
            d::Request::Client { .. }
            | d::Request::Show { .. }
            | d::Request::Leave { .. }
            | d::Request::SignOut
            | d::Request::Dropped { .. }
            | d::Request::Fact { .. } => None,
        })
        .expect("account operation attempt");
    fixtures::root_event(
        root,
        d::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Answer {
                    attempt,
                    envelope: skein_lib::RequestEnvelope::Final,
                    answer: Some(jig_client::Answered::Jig(c::Answer::Done(c::Outcome::Owner(outcome)))),
                    why: None,
                },
            },
        },
    );
}
#[test]
fn only_settled_account_statuses_have_the_done_style() {
    use crate::accounts::words::{self, Word as W};
    for (outcome, word, done) in [
        (
            Some((
                Operation::SignOut { source: 1 },
                c::OwnerOutcome::AccountSignedOut { confirmed: true, failed_hosts: Box::new([]) },
            )),
            W::Confirmed,
            true,
        ),
        (
            Some((
                Operation::SignOut { source: 1 },
                c::OwnerOutcome::AccountSignedOut { confirmed: false, failed_hosts: Box::new([]) },
            )),
            W::Unconfirmed,
            false,
        ),
        (
            Some((
                Operation::RemoveKey { source: 1 },
                c::OwnerOutcome::AccountRemoved { confirmed: true, failed_hosts: Box::new([]) },
            )),
            W::Removed,
            true,
        ),
        (
            Some((
                Operation::RemoveKey { source: 1 },
                c::OwnerOutcome::AccountRemoved { confirmed: false, failed_hosts: Box::new([]) },
            )),
            W::RemovedUnconfirmed,
            false,
        ),
    ] {
        let (mut root, watch) = fixtures::account_root();
        let mut doc = document(1);
        doc.settled = outcome.is_none();
        deliver(&mut root, watch, doc);
        if let Some((operation, outcome)) = outcome {
            answer_account_operation(&mut root, operation, outcome);
        }
        let mut view = fixtures::view();
        let tree = fixtures::build(&mut view, &root);
        let name = words::say(&fixtures::words(), word, &[]);
        let status = tree
            .nodes()
            .iter()
            .find(|n| n.role == Role::Status && n.name.as_deref() == Some(name.as_ref()))
            .expect("account status line");
        assert_eq!(
            status.styles.as_slice().contains(&crate::look::STATUS_DONE),
            done,
            "{}",
            String::from_utf8_lossy(&name)
        );
    }
}
#[cfg(target_pointer_width = "64")]
#[path = "../../../../testing/memory_figures.rs"]
mod memory_figures;
#[cfg(target_pointer_width = "64")]
#[path = "../../../../testing/memory_literal.rs"]
mod memory_literal;
// Tree storage, page scratch, forge/frame/account bindings, choices and words.

#[test]
#[cfg(target_pointer_width = "64")]
fn account_bounds_have_a_literal_price() {
    let domain = fixtures::limits();
    let words = fixtures::words();
    let bounds = crate::Limits::of(&domain, &words).unwrap();
    let actual = crate::worst_case(&bounds, &domain, &words).expect("priced fixture");
    memory_figures::check(
        &skein_scratch::paths::workspace().join("crates/temper/tests/figures/memory/client-view"),
        &crate::limits::memory_terms(&bounds, &domain, &words).expect("view memory terms"),
        actual,
    );
}
fn owner_root() -> (d::Domain, Token) {
    let (root, deployment, _) = owner_root_with_accounts();
    (root, deployment)
}
fn owner_root_with_accounts() -> (d::Domain, Token, Token) {
    let limits = fixtures::limits();
    let (mut root, me_watch, _, _, accounts) = fixtures::root_at_full(d::Address::Accounts, &limits, b"Shop");
    let mut me = d::pages::accounts::current_me(root.client()).unwrap().clone();
    me.deployment_role = Some(c::HeldRole::Owner);
    let env = skein_lib::Env { now: skein_lib::Time::ZERO, wall: skein_lib::Wall::EPOCH, limits };
    let mut out = skein_lib::Queue::with_capacity(d::max_out(&limits));
    d::step(
        &mut root,
        &env,
        d::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched {
                    watch: me_watch,
                    event: c::WatchEvent::Delivered(c::Delivery::Me(Box::new(me))),
                },
            },
        },
        &mut out,
    );
    let watch =
        out.iter()
            .find_map(|r| {
                if let d::Request::Client {
                    request:
                        jig_client::Request::Down {
                            down:
                                jig_client::Down::Open {
                                    watch,
                                    subject: jig_client::Watchable::Jig(c::Subject::Deployment),
                                },
                        },
                } = r
                {
                    Some(*watch)
                } else {
                    None
                }
            })
            .unwrap();
    let link = jig_client_protocol_link::Limits::new(limits.client.domain, limits.client.exchanges).unwrap();
    let deployment = jig_client_protocol_link::translate::documents::owner_deployment(
        jig_client_test_records::deployment_rows_record(),
        &link,
    )
    .unwrap();
    deliver_deployment(&mut root, watch, deployment);
    (root, watch, accounts.expect("own accounts watch"))
}
#[test]
fn an_owner_sees_deployment_accounts_after_their_own() {
    let (root, _) = owner_root();
    let mut view = fixtures::view();
    let text = fixtures::text(fixtures::build(&mut view, &root));
    assert!(
        skein_lib::bytes::find(&text, b"Your accounts").unwrap()
            < skein_lib::bytes::find(&text, b"Deployment accounts").unwrap()
    );
    assert!(skein_lib::bytes::find(&text, b"Account 41").is_some());
}
#[test]
fn the_rows_offer_no_number_fields() {
    let (root, mut view) = key_draft();
    let tree = fixtures::build(&mut view, &root);
    for name in [b"Provider number".as_slice(), b"Account number", b"Source number"] {
        assert!(!tree.nodes().iter().any(|n| n.name.as_deref() == Some(name)));
    }
}
#[test]
fn cancel_refreshes_the_rows_secret_field() {
    let (mut root, mut view) = key_draft();
    let before = root.pages().accounts().unwrap().request.key_written();
    // Cancel also belongs to an editor before its first request.
    action(&mut root, A::Cancel);
    fixtures::build(&mut view, &root);
    assert!(key_field(&view).value.as_ref().unwrap().written > before);
}

#[test]
fn expiring_has_a_separator() {
    let mut doc = document(1);
    doc.accounts.get_mut(0).unwrap().sources.get_mut(0).unwrap().expiring = true;
    assert!(shown(doc).contains("Ready · Expiring"));
}
#[test]
fn attention_has_a_separator() {
    let mut doc = document(1);
    doc.accounts.get_mut(0).unwrap().sources.get_mut(0).unwrap().attention = true;
    assert!(shown(doc).contains("Ready · Needs attention"));
}
#[test]
fn spent_has_its_own_time_word() {
    let mut doc = document(1);
    doc.accounts.get_mut(0).unwrap().sources.get_mut(0).unwrap().spent_until =
        Some(skein_lib::Wall::from_nanos(1_791_828_240_000_000_000));
    assert!(shown(doc).contains("Ready · Usage limit reached · Usage limit until Oct 12, 18:04 UTC"));
}
fn confirmation_words(operation: Operation) -> String {
    let (mut root, watch) = fixtures::account_root();
    deliver(&mut root, watch, document(1));
    action(&mut root, A::KeyFor { row: Row { holder: c::AccountHolder::Own, account: 1 }, source: None });
    action(&mut root, A::Submit { name: [72; 16], row: Row { holder: c::AccountHolder::Own, account: 1 }, operation });
    let mut view = fixtures::view();
    String::from_utf8(fixtures::text(fixtures::build(&mut view, &root)).into_vec()).unwrap()
}
#[test]
fn sign_out_confirmation_names_its_source() {
    assert!(
        confirmation_words(Operation::SignOut { source: 1 })
            .contains("Sign this source out? · Your own accounts · Account 1 · Source 1")
    );
}
#[test]
fn key_removal_confirmation_names_its_source() {
    assert!(
        confirmation_words(Operation::RemoveKey { source: 1 })
            .contains("Remove this key? · Your own accounts · Account 1 · Source 1")
    );
}
#[test]
fn account_removal_confirmation_names_its_holder() {
    let (mut root, _) = owner_root();
    action(&mut root, A::KeyFor { row: Row { holder: c::AccountHolder::Deployment, account: 41 }, source: None });
    action(
        &mut root,
        A::Submit {
            name: [72; 16],
            row: Row { holder: c::AccountHolder::Deployment, account: 41 },
            operation: Operation::RemoveAccount,
        },
    );
    let mut view = fixtures::view();
    let text = String::from_utf8(fixtures::text(fixtures::build(&mut view, &root)).into_vec()).unwrap();
    assert!(text.contains("Remove this account and all its sources? · The deployment's accounts · Account 41"));
}
#[test]
fn a_deployment_row_offers_no_sign_in() {
    let (root, _) = owner_root();
    let mut view = fixtures::view();
    let tree = fixtures::build(&mut view, &root);
    assert!(!tree.nodes().iter().any(|n| n.name.as_deref() == Some(b"Use deployment accounts")));
    assert!(!tree.nodes().iter().any(|n| n.name.as_deref() == Some(b"Continue with ChatGPT")));
}
#[test]
fn an_account_without_a_provider_has_one_separator() {
    let mut doc = document(1);
    doc.accounts.get_mut(0).unwrap().provider = None;
    doc.accounts.get_mut(0).unwrap().provider_name = None;
    assert!(shown(doc).contains("Account 1 · Pat"));
}
#[test]
fn empty_uses_have_no_label() {
    let mut doc = document(1);
    doc.accounts.get_mut(0).unwrap().uses = Some(List::with_capacity(0));
    assert!(!shown(doc).contains("Uses:"));
}
#[test]
fn an_empty_deployment_leaves_out_its_section() {
    let (mut root, watch) = owner_root();
    let mut doc = root.pages().accounts().unwrap().deployment(root.client()).unwrap().clone();
    doc.accounts = List::with_capacity(0);
    deliver_deployment(&mut root, watch, doc);
    let mut view = fixtures::view();
    let text = String::from_utf8(fixtures::text(fixtures::build(&mut view, &root)).into_vec()).unwrap();
    assert!(!text.contains("Deployment accounts"));
}
#[test]
fn accounts_past_the_count_are_named() {
    let mut counts = fixtures::limits().accounts;
    counts.accounts = 2;
    assert!(shown_at(document(3), counts).contains("Accounts left out: 1"));
}
#[test]
fn sources_past_the_count_are_named() {
    let mut counts = fixtures::limits().accounts;
    counts.sources = 2;
    assert!(shown_at(document(3), counts).contains("Sources left out: 1"));
}
#[test]
fn sign_ins_past_the_count_are_named() {
    let mut counts = fixtures::limits().accounts;
    counts.sign_ins = 2;
    assert!(shown_at(document(3), counts).contains("Sign-ins left out: 1"));
}
#[test]
fn each_source_row_offers_its_rank_control() {
    let mut doc = document(16);
    for at in 0..doc.accounts.len() {
        let row = doc.accounts.get_mut(at).unwrap();
        row.number = 1;
        row.place = 2;
        for at in 0..row.sources.len() {
            let source = row.sources.get_mut(at).unwrap();
            source.number = 1;
            source.place = 2;
        }
    }
    let (mut root, watch) = fixtures::account_root();
    deliver(&mut root, watch, doc);
    action(&mut root, A::KeyFor { row: Row { holder: c::AccountHolder::Own, account: 1 }, source: None });
    let mut view = bounded_view(fixtures::limits().accounts);
    let tree = fixtures::build(&mut view, &root);
    assert_eq!(tree.nodes().iter().filter(|n| n.name.as_deref() == Some(b"Move source up")).count(), 16 * 8);
}
#[test]
fn bindings_at_capacity_refuse_another_control() {
    let counts = fixtures::limits().accounts;
    let mut bindings = crate::accounts::Bindings::new(counts, fixtures::limits().client.domain.rows);
    for _ in 0..crate::accounts::binding_capacity(&counts, fixtures::limits().client.domain.rows) {
        assert!(bindings.insert(crate::accounts::Meaning::Key).is_some());
    }
    assert!(bindings.insert(crate::accounts::Meaning::Key).is_none());
}

#[test]
fn deployment_source_confirmation_names_its_account() {
    let (mut root, watch) = owner_root();
    let mut doc = root.pages().accounts().unwrap().deployment(root.client()).unwrap().clone();
    let mut sources = List::with_capacity(1);
    sources.push(source(43)).unwrap();
    doc.accounts.get_mut(0).unwrap().sources = sources;
    deliver_deployment(&mut root, watch, doc);
    action(&mut root, A::KeyFor { row: Row { holder: c::AccountHolder::Deployment, account: 41 }, source: Some(43) });
    action(
        &mut root,
        A::Submit {
            name: [72; 16],
            row: Row { holder: c::AccountHolder::Deployment, account: 41 },
            operation: Operation::RemoveKey { source: 43 },
        },
    );
    let mut view = fixtures::view();
    let text = String::from_utf8(fixtures::text(fixtures::build(&mut view, &root)).into_vec()).unwrap();
    assert!(text.contains("Remove this key? · The deployment's accounts · Account 41 · Source 43"));
}
#[test]
fn deployment_accounts_past_the_count_are_named() {
    let (mut root, watch) = owner_root();
    let mut doc = root.pages().accounts().unwrap().deployment(root.client()).unwrap().clone();
    let row = doc.accounts.get(0).unwrap().clone();
    let mut rows = List::with_capacity(3);
    for _ in 0_u32..3 {
        rows.push(row.clone()).unwrap();
    }
    doc.accounts = rows;
    deliver_deployment(&mut root, watch, doc);
    let mut counts = fixtures::limits().accounts;
    counts.accounts = 2;
    let mut view = bounded_view(counts);
    let tree = fixtures::build(&mut view, &root);
    let text = String::from_utf8(fixtures::text(tree).into_vec()).unwrap();
    assert_eq!(
        tree.nodes()
            .iter()
            .filter(|node| node.role == Role::Heading { level: 3 }
                && node.name.as_ref().is_some_and(|name| name.starts_with(b"Account 41")))
            .count(),
        2
    );
    assert!(text.contains("Accounts left out: 1"));
}

#[test]
fn the_link_word_counts_towards_the_address_bound() {
    let address = format!("https://i/{}", "x".repeat(490));
    assert!(address.len() < usize::try_from(fixtures::tree_limits().text_per_node).unwrap());
    let root = sign_in_address(address.as_bytes());
    let mut view = fixtures::view();
    let tree = fixtures::build(&mut view, &root);
    assert!(!tree.nodes().iter().any(|n| n.external.is_some()));
}

#[test]
fn typing_a_key_keeps_the_write_revision() {
    let (mut root, mut view) = key_draft();
    let before = key_field(&view).value.as_ref().unwrap().written;
    press(
        &mut view,
        &mut root,
        Role::TextField,
        b"Key (pasted once)",
        Act::Input { value: Box::from(b"another-key".as_slice()) },
    );
    fixtures::build(&mut view, &root);
    assert_eq!(key_field(&view).value.as_ref().unwrap().written, before);
}

#[test]
fn accounts_show_the_configured_workers_program_observation() {
    let mut doc = card_document(b"anthropic", b"work-login", c::OwnerAccountKind::Program);
    let mut programs = List::with_capacity(1);
    programs
        .push(c::OwnerProgramLogin {
            name: b"work-login".as_slice().into(),
            state: Some(c::OwnerProgramState::SignedInOtherwise { method: b"other-method".as_slice().into() }),
            sign_out: None,
            cleanup: false,
            cut: false,
        })
        .unwrap();
    doc.program_hosts = List::with_capacity(1);
    doc.program_hosts
        .push(c::OwnerProgramHost {
            name: b"configured-worker".as_slice().into(),
            channel: 17,
            connected: true,
            awaiting_logins: false,
            programs,
            cut: false,
        })
        .unwrap();
    let text = shown(doc);
    assert!(
        text.contains("configured-worker") && text.contains("work-login") && text.contains("other-method"),
        "{text}"
    );
}

#[test]
fn accounts_are_folded_disclosures_with_their_state_and_line() {
    let (mut root, watch) = fixtures::account_root();
    deliver(&mut root, watch, card_document(b"openai", b"alice", c::OwnerAccountKind::SignIn));
    let mut view = fixtures::view();
    let tree = fixtures::build(&mut view, &root);
    let row = tree.nodes().iter().find(|node| node.styles.as_slice().contains(&crate::look::ACCOUNT_ROW)).unwrap();
    assert!(!row.states.as_slice().contains(&skein_ui::State::Expanded));
    let summary = core::str::from_utf8(row.description.as_ref().unwrap()).unwrap();
    assert!(summary.contains("Ready"));
    assert!(summary.contains("Your own work, and projects where you’re the only member"));
    assert!(!summary.contains("no task uses it now"));
    // Opening is native shell state; every row keeps its content in the semantic tree.
    let at = tree.nodes().iter().position(|n| n.id == row.id).unwrap();
    let content = &tree.nodes()[at..at + usize::try_from(row.size).unwrap()];
    for name in [b"Pays for".as_slice(), b"Add an API key", b"Sign out", b"Remove account"] {
        assert!(content.iter().any(|n| n.name.as_deref() == Some(name)), "{name:?}");
    }
}
#[test]
fn a_limited_plan_offers_resume_and_moves_the_first_later_key() {
    let (mut root, watch) = fixtures::account_root();
    let mut doc = card_document(b"openai", b"alice", c::OwnerAccountKind::SignIn);
    let row = doc.accounts.get_mut(0).unwrap();
    row.sources.get_mut(0).unwrap().state = c::OwnerAccountState::Later { until: None };
    let sign_in = row.sources.get(0).unwrap().clone();
    row.sources = List::with_capacity(3);
    row.sources.push(sign_in).unwrap();
    row.sources.push(source(2)).unwrap();
    row.sources.push(source(3)).unwrap();
    deliver(&mut root, watch, doc);
    let mut view = fixtures::view();
    let text = String::from_utf8(fixtures::text(fixtures::build(&mut view, &root)).into_vec()).unwrap();
    assert!(text.contains("temper reached its usage limit on your ChatGPT plan. Tasks on it wait."));
    let node = fixtures::find(view.tree(), Role::Button, b"Use the API key first").unwrap();
    let Some(d::Event::Act { action }) = crate::event(&mut view, node, Act::Press) else { panic!("row act") };
    assert_eq!(
        action,
        d::Action::Pages(d::pages::Act::Accounts(A::Submit {
            name: root.pages().account_name(),
            row: Row { holder: c::AccountHolder::Own, account: 1 },
            operation: Operation::MoveSource { source: 2, place: 1 }
        }))
    );
}
#[test]
fn declined_plan_usage_belongs_to_the_ending_named_account() {
    let (mut root, watch) = fixtures::account_root();
    let mut doc = document(2);
    for index in 0..doc.accounts.len() {
        let row = doc.accounts.get_mut(index).unwrap();
        row.provider_name = Some(text(b"openai"));
        row.sources.get_mut(0).unwrap().kind = c::OwnerAccountKind::SignIn;
    }
    doc.last_end =
        Some(c::OwnerAccountLastEnd { source: 1, end: c::OwnerAccountEnd::Made { account: 2, declined: true } });
    deliver(&mut root, watch, doc);
    let mut view = fixtures::view();
    let tree = fixtures::build(&mut view, &root);
    let row = tree
        .nodes()
        .iter()
        .position(|n| n.role == Role::Heading { level: 3 } && n.name.as_ref().unwrap().starts_with(b"ChatGPT"))
        .unwrap();
    let text = String::from_utf8(fixtures::text(tree).into_vec()).unwrap();
    assert!(text.contains("You signed in without plan usage."));
    assert!(tree.nodes().iter().skip(row).any(|n| n.name.as_deref() == Some(b"Use an API key")));
}
#[test]
fn none_of_your_own_offers_the_connect_model_link() {
    let (mut root, watch) = fixtures::account_root();
    deliver(&mut root, watch, document(0));
    let mut view = fixtures::view();
    let text = String::from_utf8(fixtures::text(fixtures::build(&mut view, &root)).into_vec()).unwrap();
    assert!(text.contains("No model of yours yet."));
    assert!(fixtures::find(view.tree(), Role::Link, b"Connect a model").is_some());
}
#[test]
fn each_account_state_draws_its_words_and_state_controls() {
    for (state, word, act) in [
        (c::OwnerAccountState::Granted, "Ready", None),
        (
            c::OwnerAccountState::Later { until: None },
            "temper reached its usage limit on your ChatGPT plan. Tasks on it wait.",
            Some(b"Resume".as_slice()),
        ),
        (
            c::OwnerAccountState::Later { until: Some(skein_lib::Wall::EPOCH) },
            "Limited by OpenAI until",
            Some(b"Resume".as_slice()),
        ),
        (c::OwnerAccountState::SignInAgain, "Signed out of ChatGPT.", Some(b"Continue with ChatGPT".as_slice())),
        (
            c::OwnerAccountState::Misconfigured,
            "OpenAI refused temper’s sign-in. Check the configuration’s registration.",
            None,
        ),
        (
            c::OwnerAccountState::Unloaded { at: c::OwnerAccountPlace::File, why: c::OwnerAccountUnread::Unreadable },
            "Cannot load File: Unreadable",
            None,
        ),
    ] {
        let (mut root, watch) = fixtures::account_root();
        let mut doc = card_document(b"openai", b"alice", c::OwnerAccountKind::SignIn);
        doc.accounts.get_mut(0).unwrap().sources.get_mut(0).unwrap().state = state;
        deliver(&mut root, watch, doc);
        let mut view = fixtures::view();
        let tree = fixtures::build(&mut view, &root);
        let shown = String::from_utf8(fixtures::text(tree).into_vec()).unwrap();
        assert!(shown.contains(word), "{state:?}: {shown}");
        if let Some(act) = act {
            assert!(fixtures::find(tree, Role::Button, act).is_some(), "{state:?}");
        }
    }
}
#[test]
fn a_moving_account_shows_its_state_without_buttons() {
    let (mut root, watch) = fixtures::account_root();
    let mut doc = card_document(b"openai", b"alice", c::OwnerAccountKind::SignIn);
    deliver(&mut root, watch, doc.clone());
    // An editor opened before the move must not keep its buttons either.
    action(&mut root, A::KeyFor { row: Row { holder: c::AccountHolder::Own, account: 1 }, source: None });
    let row = doc.accounts.get_mut(0).unwrap();
    row.place = 2;
    let source = row.sources.get_mut(0).unwrap();
    source.state = c::OwnerAccountState::Moving;
    source.usable = false;
    source.place = 2;
    deliver(&mut root, watch, doc);
    let mut view = fixtures::view();
    let tree = fixtures::build(&mut view, &root);
    let row = tree.nodes().iter().find(|node| node.styles.as_slice().contains(&crate::look::ACCOUNT_ROW)).unwrap();
    assert!(core::str::from_utf8(row.description.as_ref().unwrap()).unwrap().contains("Moving to another deployment"));
    let at = tree.nodes().iter().position(|node| node.id == row.id).unwrap();
    let content = super::inbox::subtree(tree, at);
    assert!(content.iter().all(|node| node.role != Role::Button));
    assert!(skein_lib::bytes::find(&fixtures::text(tree), b"Moving to another deployment").is_some());
}
#[test]
fn a_programs_missing_login_names_its_host_command_and_check() {
    let (mut root, watch) = fixtures::account_root();
    let mut doc = card_document(b"anthropic", b"claude-alice", c::OwnerAccountKind::Program);
    doc.accounts.get_mut(0).unwrap().sources.get_mut(0).unwrap().usable = false;
    let mut programs = List::with_capacity(1);
    programs
        .push(c::OwnerProgramLogin {
            name: b"claude-alice".as_slice().into(),
            state: Some(c::OwnerProgramState::NotSignedIn),
            sign_out: None,
            cleanup: false,
            cut: false,
        })
        .unwrap();
    doc.program_hosts = List::with_capacity(1);
    doc.program_hosts
        .push(c::OwnerProgramHost {
            name: b"worker".as_slice().into(),
            channel: 1,
            connected: true,
            awaiting_logins: false,
            programs,
            cut: false,
        })
        .unwrap();
    deliver(&mut root, watch, doc);
    let mut view = fixtures::view();
    let tree = fixtures::build(&mut view, &root);
    let shown = String::from_utf8(fixtures::text(tree).into_vec()).unwrap();
    assert!(shown.contains("Claude Code isn't signed in for 'claude-alice' on worker."));
    assert!(shown.contains("temper program-login claude-alice"));
    assert!(fixtures::find(tree, Role::Button, b"Check again").is_some());
    assert!(fixtures::find(tree, Role::Button, b"Sign out on every machine").is_some());
}
#[test]
fn an_outcome_is_in_the_disclosure_that_sent_the_act() {
    let (mut root, watch) = fixtures::account_root();
    deliver(&mut root, watch, document(2));
    answer_account_operation(
        &mut root,
        Operation::SignOut { source: 1 },
        c::OwnerOutcome::AccountSignedOut { confirmed: true, failed_hosts: Box::new([]) },
    );
    let mut view = fixtures::view();
    let tree = fixtures::build(&mut view, &root);
    let rows: Vec<_> = tree
        .nodes()
        .iter()
        .enumerate()
        .filter(|(_, n)| n.styles.as_slice().contains(&crate::look::ACCOUNT_ROW))
        .collect();
    for (number, (at, row)) in rows.into_iter().enumerate() {
        let content = &tree.nodes()[at..at + usize::try_from(row.size).unwrap()];
        assert_eq!(
            content.iter().any(|n| n.name.as_deref() == Some(b"Signed out; revocation confirmed.")),
            number == 0
        );
    }
}

fn deliver_deployment(root: &mut d::Domain, watch: Token, doc: c::OwnerDeployment) {
    fixtures::root_event(
        root,
        d::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched {
                    watch,
                    event: c::WatchEvent::Delivered(c::Delivery::OwnerDeployment(Box::new(doc))),
                },
            },
        },
    );
}
#[test]
fn deployment_account_facts_exclude_waiting_tasks_from_its_users() {
    const RUNNING: u64 = 4;
    const WAITING: u64 = 9;
    let (mut root, watch) = owner_root();
    let mut doc = root.pages().accounts().unwrap().deployment(root.client()).unwrap().clone();
    let row = doc.accounts.get_mut(0).unwrap();
    let mut chip = fixtures::task::document().chip;
    row.runs = List::with_capacity(2);
    for task in [RUNNING, WAITING] {
        chip.task = task;
        row.runs.push(chip.clone()).unwrap();
    }
    row.waiting = List::with_capacity(1);
    row.waiting.push(chip).unwrap();
    deliver_deployment(&mut root, watch, doc);
    let mut view = fixtures::view();
    let tree = fixtures::build(&mut view, &root);
    for (name, value) in [(b"Used by".as_slice(), b"T4".as_slice()), (b"Waiting for it", b"T9")] {
        let at = tree.nodes().iter().position(|n| n.role == Role::Region && n.name.as_deref() == Some(name)).unwrap();
        let row = &tree.nodes()[at..at + usize::try_from(tree.nodes()[at].size).unwrap()];
        assert!(row.iter().any(|n| n.role == Role::Description && n.name.as_deref() == Some(value)), "{name:?}");
    }
}

#[test]
fn only_deployment_rows_show_usage_facts_and_their_use_summary() {
    let (mut root, deployment, accounts) = owner_root_with_accounts();
    let mut doc = root.pages().accounts().unwrap().deployment(root.client()).unwrap().clone();
    let row = doc.accounts.get_mut(0).unwrap();
    row.runs = List::with_capacity(0);
    row.waiting = List::with_capacity(0);
    deliver_deployment(&mut root, deployment, doc);
    deliver(&mut root, accounts, card_document(b"openai", b"alice", c::OwnerAccountKind::SignIn));
    let view = fixtures::page_view(&root);
    let mut rows: u32 = 0;
    for row in view.tree().nodes().iter().filter(|n| n.styles.as_slice().contains(&crate::look::ACCOUNT_ROW)) {
        let at = view.tree().nodes().iter().position(|n| n.id == row.id).unwrap();
        let content = &view.tree().nodes()[at..at + usize::try_from(row.size).unwrap()];
        let deployment = rows == 1;
        for name in [b"Used by".as_slice(), b"Waiting for it"] {
            assert_eq!(content.iter().any(|n| n.name.as_deref() == Some(name)), deployment);
        }
        let summary = core::str::from_utf8(row.description.as_deref().unwrap()).unwrap();
        assert_eq!(summary.contains("no task uses it now"), deployment);
        rows += 1;
    }
    assert_eq!(rows, 2);
}

#[test]
fn english_phrase_declarations_match_the_vocabulary() {
    let path = skein_scratch::paths::package().join("src/words.rs");
    let source = std::fs::read_to_string(&path).unwrap();
    let variants = source.split("pub enum Word {\n").nth(1).unwrap().split("\n}").next().unwrap();
    let names: Vec<_> = variants.lines().map(str::trim).filter(|line| line.ends_with(',')).collect();
    let mut all = format!("const ALL: [Word; {}] = [\n", names.len());
    for name in names {
        all.push_str("    Word::");
        all.push_str(name);
        all.push('\n');
    }
    all.push_str("];");
    let start = source.find("const ALL: [Word;").unwrap();
    let end = start + source[start..].find("\n];").unwrap() + 3;
    if std::env::var_os("SKEIN_CODEGEN_WRITE").is_some_and(|value| value == "1") {
        let updated = format!("{}{}{}", &source[..start], all, &source[end..]);
        std::fs::write(&path, updated).unwrap();
    } else {
        assert_eq!(&source[start..end], all);
    }
    for (marker, expected, actual) in crate::words::english_declarations() {
        memory_literal::check(&path, marker, expected, actual);
    }
}

#[test]
fn an_accounts_rows_sign_in_offers_the_paste() {
    let (mut root, watch) = super::connect_model::flow_fixture(d::WayKind::SignIn);
    fixtures::root_event(&mut root, d::Event::Went { place: d::Place::Address(d::Address::Accounts) });
    deliver(&mut root, watch, card_document(b"openai", b"fixture", c::OwnerAccountKind::SignIn));
    answer_account_operation(
        &mut root,
        Operation::SignIn { provider: PROVIDER },
        c::OwnerOutcome::AccountSignInStarted {
            source: SOURCE,
            address: Box::from(b"https://issuer.example/visit".as_slice()),
        },
    );
    let mut view = fixtures::view();
    let tree = fixtures::build(&mut view, &root);
    let field = fixtures::find(tree, Role::TextField, b"Address of the page that couldn't be reached").unwrap();
    let field_at = tree.nodes().iter().position(|n| n.id == field).unwrap();
    let disclosure = tree
        .nodes()
        .iter()
        .position(|n| {
            n.role == Role::Disclosure && n.name.as_deref() == Some(b"Signed in, but the page couldn't be reached?")
        })
        .unwrap();
    assert!(field_at > disclosure && field_at < disclosure + usize::try_from(tree.nodes()[disclosure].size).unwrap());
    let input = crate::event(&mut view, field, Act::Input { value: Box::from(b"fixture".as_slice()) }).unwrap();
    let d::Event::SignInAddress { value } = &input else { panic!("secret event") };
    assert!(value.expose() == b"fixture");
    fixtures::root_event(&mut root, input);
    let finish = fixtures::find(fixtures::build(&mut view, &root), Role::Button, b"Finish connecting").unwrap();
    assert_eq!(
        crate::act(&mut view, finish, Act::Press),
        Some(d::Action::Pages(d::pages::Act::Accounts(A::Finish { name: root.pages().account_name() })))
    );
}

fn account_order(doc: c::OwnerAccounts) -> Vec<u32> {
    let (mut root, watch) = fixtures::account_root();
    deliver(&mut root, watch, doc);
    let mut view = bounded_view(fixtures::limits().accounts);
    fixtures::build(&mut view, &root);
    let ids: Vec<_> = view
        .tree()
        .nodes()
        .iter()
        .filter(|node| node.role == Role::Button && node.name.as_deref() == Some(b"Remove account"))
        .map(|node| node.id)
        .collect();
    let mut numbers = Vec::new();
    for id in ids {
        if let Some(d::Action::Pages(d::pages::Act::Accounts(A::Submit { row, .. }))) =
            crate::act(&mut view, id, Act::Press)
        {
            numbers.push(row.account);
        }
    }
    numbers
}
#[test]
fn an_accounts_row_keeps_its_header_sources_facts_and_acts_in_order() {
    let (mut root, me_watch, _, _, accounts_watch) =
        fixtures::root_at_full(d::Address::Accounts, &fixtures::limits(), b"Shop");
    let mut me = d::pages::accounts::current_me(root.client()).unwrap().clone();
    me.projects.get_mut(0).unwrap().models = super::choice::line_models(true);
    fixtures::root_event(
        &mut root,
        d::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched {
                    watch: me_watch,
                    event: c::WatchEvent::Delivered(c::Delivery::Me(Box::new(me))),
                },
            },
        },
    );
    deliver(&mut root, accounts_watch.unwrap(), card_document(b"openai", b"Pat", c::OwnerAccountKind::SignIn));
    let view = fixtures::page_view(&root);
    let tree = view.tree();
    assert!(fixtures::find(tree, Role::Paragraph, "The LLM accounts temper’s agents run on for you. A task keeps its account: while the account is limited, the task waits for it.".as_bytes()).is_some());
    let row = super::inbox::styled(tree, crate::look::ACCOUNT_ROW);
    let nodes = super::inbox::subtree(tree, row);
    assert!(nodes[0].states.is_empty());
    let mut previous = 0;
    for (role, name) in [
        (Role::Status, b"Ready".as_slice()),
        (Role::Paragraph, b"Its sources in order: a run takes the first usable one.".as_slice()),
        (Role::Region, b"Models".as_slice()),
        (Role::Region, b"Pays for".as_slice()),
        (Role::Button, b"Remove account".as_slice()),
    ] {
        let at = nodes.iter().position(|node| node.role == role && node.name.as_deref() == Some(name)).unwrap();
        assert!(at > previous, "{name:?}");
        previous = at;
    }
    assert!(
        !nodes
            .iter()
            .any(|node| node.name.as_deref() == Some(b"Used by") || node.name.as_deref() == Some(b"Waiting for it"))
    );
}
