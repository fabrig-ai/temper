#![allow(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros,
    reason = "semantic tree assertions"
)]
use super::{accounts::press, fixtures};
use jig_client::contract as c;
use skein_lib::List;
use skein_ui::{Act, Role};
use temper_client_domain::{self as d, pages::connect_model::Act as M};
const PROVIDER: u32 = 17;
fn model_action(root: &mut d::Domain, act: M) {
    fixtures::root_event(root, d::Event::Act { action: d::Action::Pages(d::pages::Act::ConnectModel(act)) });
}
fn offer(provider: u32, name: &[u8], sign_in: bool, program: bool) -> c::ProviderOffer {
    let mut programs = List::with_capacity(1);
    if program {
        programs.push(c::Text::new(Box::from(b"claude_code".as_slice()), 32).unwrap()).unwrap();
    }
    c::ProviderOffer {
        provider,
        name: text(name),
        key_bytes: 4096,
        sign_in,
        signed_in_plan: true,
        key: true,
        programs,
        programs_cut: false,
        none: false,
        manage_usage: None,
    }
}
pub(super) fn text(value: &[u8]) -> c::DocumentText {
    c::DocumentText::complete(c::Text::new(Box::from(value), 64).unwrap())
}
fn ways_root(offers: List<c::ProviderOffer>, demo: bool) -> d::Domain {
    let limits = fixtures::limits();
    let (mut root, watch, _) = fixtures::root_at(d::Address::ConnectModel { way: None }, &limits);
    let mut me = d::pages::accounts::current_me(root.client()).unwrap().clone();
    me.providers = offers;
    me.providers_cut = true;
    me.deployment_role = Some(c::HeldRole::Owner);
    if demo {
        return fixtures::demo_frame_root(
            d::Address::ConnectModel { way: None },
            fixtures::PROJECT,
            true,
            None,
            Some(me),
        );
    }
    fixtures::root_event(
        &mut root,
        d::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched { watch, event: c::WatchEvent::Delivered(c::Delivery::Me(Box::new(me))) },
            },
        },
    );
    root
}
pub(super) fn offered_root() -> d::Domain {
    let mut providers = List::with_capacity(1);
    providers.push(offer(PROVIDER, b"openai", true, false)).unwrap();
    ways_root(providers, false)
}
fn control_names(root: &d::Domain) -> Vec<Vec<u8>> {
    let mut view = fixtures::view();
    fixtures::build(&mut view, root)
        .nodes()
        .iter()
        .filter(|node| node.role == Role::Button || node.role == Role::Link)
        .filter_map(|node| node.name.as_ref().map(|name| name.to_vec()))
        .collect()
}
#[test]
fn the_model_choose_names_the_plan_sign_in_and_key_ways() {
    let root = offered_root();
    let view = fixtures::page_view(&root);
    let primary: Vec<_> = view
        .tree()
        .nodes()
        .iter()
        .filter(|n| n.styles.as_slice().contains(&crate::look::FLOW_PRIMARY))
        .map(|n| n.name.as_deref().unwrap())
        .collect();
    assert_eq!(primary, [b"Continue with ChatGPT".as_slice()]);
    let names = control_names(&root);
    assert!(names.contains(&b"Continue with ChatGPT".to_vec()));
    assert!(names.contains(&b"Use an API key".to_vec()));
}
#[test]
fn two_key_providers_name_each_key_way_by_its_brand() {
    let mut providers = List::with_capacity(2);
    providers.push(offer(PROVIDER, b"openai", false, false)).unwrap();
    providers.push(offer(PROVIDER + 1, b"Scripted", false, false)).unwrap();
    let names = control_names(&ways_root(providers, false));
    assert!(names.contains(&b"Use an OpenAI API key".to_vec()));
    assert!(names.contains(&b"Use a Scripted API key".to_vec()));
    assert!(!names.contains(&b"Use an API key".to_vec()));
}
#[test]
fn a_provider_without_a_plan_names_its_sign_in_by_its_brand() {
    let mut providers = List::with_capacity(1);
    let mut provider = offer(PROVIDER, b"anthropic", true, false);
    provider.signed_in_plan = false;
    providers.push(provider).unwrap();
    assert!(control_names(&ways_root(providers, false)).contains(&b"Sign in to Anthropic".to_vec()));
}
#[test]
fn the_demos_provider_offers_no_way() {
    let mut providers = List::with_capacity(2);
    providers.push(offer(PROVIDER, b"demo", true, true)).unwrap();
    providers.push(offer(PROVIDER + 1, b"openai", true, false)).unwrap();
    let names = control_names(&ways_root(providers, true));
    assert!(names.contains(&b"Continue with ChatGPT".to_vec()));
    assert!(names.contains(&b"Use an API key".to_vec()));
    assert!(!names.iter().any(|name| name.windows(b"demo".len()).any(|part| part == b"demo")));
}
#[test]
fn the_deployment_holder_offers_keys_only() {
    let mut providers = List::with_capacity(2);
    providers.push(offer(PROVIDER, b"openai", true, false)).unwrap();
    providers.push(offer(PROVIDER + 1, b"anthropic", false, true)).unwrap();
    let mut root = ways_root(providers, false);
    model_action(&mut root, M::Holder { holder: c::AccountHolder::Deployment });
    let names = control_names(&root);
    assert!(names.contains(&b"Use an OpenAI API key".to_vec()));
    assert!(names.contains(&b"Use an Anthropic API key".to_vec()));
    assert!(!names.contains(&b"Continue with ChatGPT".to_vec()));
    assert!(!names.contains(&b"Use your Claude subscription".to_vec()));
}
#[test]
fn a_program_way_shows_the_default_name_and_the_command_on_each_host() {
    let mut providers = List::with_capacity(1);
    providers.push(offer(PROVIDER, b"anthropic", false, true)).unwrap();
    let mut root = ways_root(providers, false);
    let mut view = fixtures::view();
    press(&mut view, &mut root, Role::Link, b"Use your Claude subscription", Act::Press);
    let tree = fixtures::build(&mut view, &root);
    let field = tree.nodes().iter().find(|node| node.name.as_deref() == Some(b"Program name")).unwrap();
    assert_eq!(field.value.as_ref().unwrap().text.as_ref(), b"claude-pat");
    assert!(fixtures::find(tree, Role::Button, b"Declare").is_some());
    assert!(String::from_utf8(fixtures::text(tree).into_vec()).unwrap().contains("temper program-login claude-pat"));
}

pub(super) fn flow_fixture(kind: d::WayKind) -> (d::Domain, skein_lib::Token) {
    let (mut root, me_watch, _, _, accounts_watch) = fixtures::root_at_full(
        d::Address::ConnectModel { way: Some(d::ModelWay { source: None, provider: PROVIDER, kind }) },
        &fixtures::limits(),
        b"Shop",
    );
    let mut me = d::pages::accounts::current_me(root.client()).unwrap().clone();
    me.providers = List::with_capacity(1);
    me.providers
        .push(offer(PROVIDER, if kind == d::WayKind::Program { b"anthropic" } else { b"openai" }, true, true))
        .unwrap();
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
    (root, accounts_watch.unwrap())
}
fn sent(root: &mut d::Domain, act: M) -> skein_lib::Token {
    let limits = fixtures::limits();
    let env = skein_lib::Env { now: skein_lib::Time::ZERO, wall: fixtures::NOW, limits };
    let mut out = skein_lib::Queue::with_capacity(d::max_out(&limits));
    d::step(root, &env, d::Event::Act { action: d::Action::Pages(d::pages::Act::ConnectModel(act)) }, &mut out);
    d::step(root, &env, d::Event::Client { event: jig_client::Event::Stored }, &mut out);
    out.iter()
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
        .expect("flow sends one request")
}
fn answer(root: &mut d::Domain, attempt: skein_lib::Token, outcome: c::OwnerOutcome) {
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
fn words(root: &d::Domain) -> String {
    String::from_utf8(fixtures::text(fixtures::page_view(root).tree()).into_vec()).unwrap()
}
fn ending(end: c::OwnerAccountEnd, display: &[u8]) -> d::Domain {
    let (mut root, watch) = flow_fixture(d::WayKind::SignIn);
    let attempt = sent(
        &mut root,
        M::SignIn { name: [1; 16], way: d::pages::accounts::Way::SignIn { provider: PROVIDER, plan: true } },
    );
    answer(
        &mut root,
        attempt,
        c::OwnerOutcome::AccountSignInStarted {
            source: 43,
            address: Box::from(b"https://issuer.example/visit".as_slice()),
        },
    );
    let mut doc = super::accounts::card_document(b"openai", display, c::OwnerAccountKind::SignIn);
    doc.accounts.get_mut(0).unwrap().manage_usage = Some(Box::from(b"https://provider.example/usage".as_slice()));
    doc.signing.clear();
    doc.last_end = Some(c::OwnerAccountLastEnd { source: 43, end });
    super::accounts::deliver(&mut root, watch, doc);
    root
}
#[test]
fn a_key_has_details_then_ready_with_its_account_and_chat_link() {
    let (mut root, _) = flow_fixture(d::WayKind::Key);
    let mut view = fixtures::view();
    assert!(fixtures::find(fixtures::build(&mut view, &root), Role::TextField, b"Key (pasted once)").is_some());
    press(
        &mut view,
        &mut root,
        Role::TextField,
        b"Key (pasted once)",
        Act::Input { value: Box::from(b"private-once".as_slice()) },
    );
    let attempt = sent(&mut root, M::Submit { name: [1; 16] });
    answer(&mut root, attempt, c::OwnerOutcome::AccountKeyKept { source: 43 });
    let text = words(&root);
    for expected in
        ["Ready", "Added your OpenAI API key", "temper’s agents now run on it.", "Account", "Pays for", "Your own work"]
    {
        assert!(text.contains(expected), "missing {expected}");
    }
    assert!(!format!("{:?}", fixtures::page_view(&root)).contains("private-once"));
    let tree = fixtures::build(&mut view, &root);
    let node = fixtures::find(tree, Role::Link, b"Start a chat").unwrap();
    assert_eq!(
        crate::act(&mut view, node, Act::Press),
        Some(d::Action::Pages(d::pages::Act::Go {
            address: d::Address::Chats { filter: d::ChatsFilter::All, start: false }
        }))
    );
}
#[test]
fn a_program_has_details_then_ready_with_the_declared_name_and_command() {
    let (mut root, _) = flow_fixture(d::WayKind::Program);
    assert!(words(&root).contains("temper program-login claude-pat"));
    let view = fixtures::page_view(&root);
    assert!(!view.tree().nodes().iter().any(|n| n.styles.as_slice().contains(&crate::look::FLOW_PRIMARY)));
    model_action(&mut root, M::ProgramName { value: Box::from(b"claude-alice".as_slice()) });
    let attempt = sent(&mut root, M::Submit { name: [1; 16] });
    answer(
        &mut root,
        attempt,
        c::OwnerOutcome::AccountProgramDeclared { source: 43, name: Box::from(b"claude-alice".as_slice()) },
    );
    let text = words(&root);
    for expected in [
        "Declared your Claude subscription · claude-alice",
        "temper program-login claude-alice",
        "Run this on each host temper’s agents run on, in a terminal there:",
        "Claude Code asks you to sign in. temper never reads its login.",
    ] {
        assert!(text.contains(expected), "missing {expected}");
    }
}
#[test]
fn a_sign_in_address_without_a_request_offers_its_button() {
    let (root, _) = flow_fixture(d::WayKind::SignIn);
    let view = fixtures::page_view(&root);
    assert!(fixtures::find(view.tree(), Role::Button, b"Continue with ChatGPT").is_some());
    assert!(fixtures::find(view.tree(), Role::Link, "‹ All ways".as_bytes()).is_some());
    assert!(words(&root).contains("Signing in"));
}
#[test]
fn a_completed_sign_in_is_ready_and_can_be_started_again() {
    let root = ending(c::OwnerAccountEnd::Made { account: 1, declined: false }, b"alice@example.com");
    let text = words(&root);
    for expected in [
        "Signed in to ChatGPT as alice@example.com",
        "temper’s agents now run on your ChatGPT plan.",
        "Account",
        "Pays for",
    ] {
        assert!(text.contains(expected), "missing {expected}");
    }
    let mut view = fixtures::page_view(&root);
    let primary: Vec<_> = view
        .tree()
        .nodes()
        .iter()
        .filter(|n| n.styles.as_slice().contains(&crate::look::FLOW_PRIMARY))
        .map(|n| n.name.as_deref().unwrap())
        .collect();
    assert_eq!(primary, [b"Start a chat".as_slice()]);
    assert!(fixtures::find(view.tree(), Role::Link, b"Manage usage").is_some());
    let node = fixtures::find(view.tree(), Role::Button, b"Continue with ChatGPT").unwrap();
    let Some(d::Action::Pages(d::pages::Act::ConnectModel(M::SignIn { .. }))) = crate::act(&mut view, node, Act::Press)
    else {
        panic!("sign-in binding")
    };
}
#[test]
fn a_completed_sign_in_without_a_display_names_the_plan() {
    assert!(
        words(&ending(c::OwnerAccountEnd::Made { account: 1, declined: false }, b"")).contains("Signed in to ChatGPT")
    );
}
#[test]
fn renewed_sign_ins_explain_how_to_add_another_account() {
    for display in [b"alice@example.com".as_slice(), b""] {
        let root = ending(c::OwnerAccountEnd::Renewed { account: 1, declined: false }, display);
        let text = words(&root);
        assert!(text.contains(if display.is_empty() {
            "That's your ChatGPT account again."
        } else {
            "That's alice@example.com again."
        }));
        assert!(text.contains("sign out of ChatGPT in your browser first, then Continue with ChatGPT."));
        assert!(fixtures::find(fixtures::page_view(&root).tree(), Role::Button, b"Continue with ChatGPT").is_some());
    }
}
#[test]
fn declined_plan_usage_offers_sign_in_again_and_a_key_link() {
    for end in [
        c::OwnerAccountEnd::Made { account: 1, declined: true },
        c::OwnerAccountEnd::Renewed { account: 1, declined: true },
    ] {
        let root = ending(end, b"alice@example.com");
        assert!(words(&root).contains("You signed in without plan usage."));
        let mut view = fixtures::page_view(&root);
        let key = fixtures::find(view.tree(), Role::Link, b"Use an API key").unwrap();
        let Some(d::Action::Pages(d::pages::Act::Go {
            address: d::Address::ConnectModel { way: Some(d::ModelWay { kind: d::WayKind::Key, .. }) },
        })) = crate::act(&mut view, key, Act::Press)
        else {
            panic!("key link")
        };
        assert!(fixtures::find(view.tree(), Role::Button, b"Continue with ChatGPT").is_some());
    }
}
#[test]
fn other_sign_in_endings_keep_their_words_and_offer_another_sign_in() {
    use c::{OwnerAccountEnd as E, OwnerAccountFailure as F, OwnerAccountRefusal as R};
    for (end, expected) in [
        (E::Refused { refusal: R::Held { provider: PROVIDER } }, "Another person here holds that ChatGPT account."),
        (
            E::Failed { failure: F::Misconfigured },
            "OpenAI refused temper's sign-in. Check the configuration's registration.",
        ),
        (E::Failed { failure: F::SignInAgain }, "Needs sign-in"),
        (E::Failed { failure: F::MissingScope { scope: 19 } }, "Missing required scope 19"),
        (E::Refused { refusal: R::Limit { bound: c::OwnerAccountBound::Sources } }, "Source limit reached."),
    ] {
        let root = ending(end, b"");
        assert!(words(&root).contains(expected), "missing {expected}");
        assert!(fixtures::find(fixtures::page_view(&root).tree(), Role::Button, b"Continue with ChatGPT").is_some());
    }
}
#[test]
fn the_accounts_banner_links_each_way_to_its_address_and_is_hidden_in_the_flow() {
    let (mut root, watch) = flow_fixture(d::WayKind::Key);
    let mut doc = super::accounts::document(0);
    doc.usable = false;
    super::accounts::deliver(&mut root, watch, doc);
    fixtures::root_event(
        &mut root,
        d::Event::Went { place: d::Place::Address(d::Address::Chats { filter: d::ChatsFilter::All, start: false }) },
    );
    let mut view = fixtures::page_view(&root);
    for (name, kind) in [
        (b"Continue with ChatGPT".as_slice(), d::WayKind::SignIn),
        (b"Use your ChatGPT subscription", d::WayKind::Program),
        (b"Use an API key", d::WayKind::Key),
    ] {
        let id = fixtures::find(view.tree(), Role::Link, name).unwrap();
        assert_eq!(
            crate::act(&mut view, id, Act::Press),
            Some(d::Action::Pages(d::pages::Act::Go {
                address: d::Address::ConnectModel { way: Some(d::ModelWay { source: None, provider: PROVIDER, kind }) }
            }))
        );
    }
    fixtures::root_event(
        &mut root,
        d::Event::Went { place: d::Place::Address(d::Address::ConnectModel { way: None }) },
    );
    assert!(!words(&root).contains("Agents need an LLM account to run."));
}
#[test]
fn the_demo_banner_links_to_choose() {
    let root =
        fixtures::demo_root(d::Address::Chats { filter: d::ChatsFilter::All, start: false }, fixtures::PROJECT, true);
    let mut view = fixtures::page_view(&root);
    let id = fixtures::find(view.tree(), Role::Link, b"Connect a model").unwrap();
    assert_eq!(
        crate::act(&mut view, id, Act::Press),
        Some(d::Action::Pages(d::pages::Act::Go { address: d::Address::ConnectModel { way: None } }))
    );
}
#[test]
fn ready_remembers_that_the_playgrounds_scripted_model_stepped_aside() {
    let provider = PROVIDER + 1;
    let mut me = d::pages::accounts::current_me(offered_root().client()).unwrap().clone();
    me.providers = List::with_capacity(1);
    me.providers.push(offer(provider, b"openai", false, false)).unwrap();
    let (mut root, accounts_watch) = fixtures::demo_frame_with_watch(
        d::Address::ConnectModel { way: Some(d::ModelWay { source: None, provider, kind: d::WayKind::Key }) },
        fixtures::PROJECT,
        true,
        None,
        Some(me),
    );
    fixtures::root_event(
        &mut root,
        d::Event::AccountKey { value: skein_lib::Secret::new(b"private-once".as_slice().into()) },
    );
    let attempt = sent(&mut root, M::Submit { name: [1; 16] });
    answer(&mut root, attempt, c::OwnerOutcome::AccountKeyKept { source: 43 });
    let mut doc = super::accounts::card_document(b"openai", b"", c::OwnerAccountKind::Key);
    doc.accounts.get_mut(0).unwrap().provider = Some(provider);
    doc.cut = false;
    super::accounts::deliver(&mut root, accounts_watch, doc);
    assert!(!root.demo().unwrap().1.model);
    assert!(words(&root).contains("The playground’s scripted model stepped aside."));
}

#[test]
fn a_returned_source_waits_for_the_watch_with_its_signing_card_alone() {
    let (mut root, _) = flow_fixture(d::WayKind::SignIn);
    assert!(d::pages::accounts::document(root.client()).is_none());
    fixtures::root_event(
        &mut root,
        d::Event::Went {
            place: d::Place::Address(d::Address::ConnectModel {
                way: Some(d::ModelWay { provider: PROVIDER, kind: d::WayKind::SignIn, source: Some(43) }),
            }),
        },
    );
    assert!(words(&root).contains("Signing in"));
    assert!(fixtures::find(fixtures::page_view(&root).tree(), Role::Button, b"Continue with ChatGPT").is_none());
    assert!(root.pages().connect_model().unwrap().request.pending.is_none());
}
#[test]
fn a_returned_source_draws_the_same_ending_words_as_an_own_sign_in() {
    let (mut root, watch) = flow_fixture(d::WayKind::SignIn);
    fixtures::root_event(
        &mut root,
        d::Event::Went {
            place: d::Place::Address(d::Address::ConnectModel {
                way: Some(d::ModelWay { provider: PROVIDER, kind: d::WayKind::SignIn, source: Some(43) }),
            }),
        },
    );
    let mut doc = super::accounts::card_document(b"openai", b"alice@example.com", c::OwnerAccountKind::SignIn);
    doc.signing.clear();
    doc.last_end =
        Some(c::OwnerAccountLastEnd { source: 43, end: c::OwnerAccountEnd::Made { account: 1, declined: true } });
    super::accounts::deliver(&mut root, watch, doc);
    assert!(words(&root).contains("You signed in without plan usage."));
    assert!(fixtures::find(fixtures::page_view(&root).tree(), Role::Button, b"Continue with ChatGPT").is_some());
}

#[test]
fn a_retired_reload_offers_no_demo_key_or_card() {
    use temper_client_protocol_wire::v1 as w;
    for address in [d::Address::ConnectModel { way: None }, d::Address::Welcome { step: d::Step::Model { way: None } }]
    {
        let (mut root, watch, _, _, _, _) = fixtures::root_at_with_forges(
            address,
            &fixtures::domain_limits(),
            b"Shop",
            Some((fixtures::PROJECT, true, w::DemoState::Ended)),
            None,
            Some(List::with_capacity(0)),
        );
        let mut me = root.pages().me(root.client()).unwrap().clone();
        me.providers = List::with_capacity(2);
        me.providers.push(offer(PROVIDER, b"demo", true, true)).unwrap();
        me.providers.push(offer(PROVIDER + 1, b"openai", true, false)).unwrap();
        fixtures::root_event(
            &mut root,
            d::Event::Client {
                event: jig_client::Event::Up {
                    up: jig_client::Up::Watched {
                        watch,
                        event: c::WatchEvent::Delivered(c::Delivery::Me(Box::new(me))),
                    },
                },
            },
        );
        for words in [fixtures::words(), fixtures::browser_words()] {
            let mut view = fixtures::view_of(words);
            let tree = fixtures::build(&mut view, &root);
            assert!(fixtures::find(tree, Role::Button, b"Continue with ChatGPT").is_some());
            assert!(fixtures::find(tree, Role::Button, b"Try the playground").is_none());
            assert!(fixtures::find(tree, Role::Link, b"Use a demo API key").is_none());
            for node in tree.nodes() {
                if let Some(name) = &node.name {
                    assert!(!name.windows(b"demo".len()).any(|part| part == b"demo"), "{name:?}");
                }
            }
        }
    }
}
