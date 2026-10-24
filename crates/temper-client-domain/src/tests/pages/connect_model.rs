#![allow(clippy::disallowed_macros, reason = "unit assertions match typed requests and capture codec diagnostics")]
use super::{
    accounts::{deliver, document, event, frame, key, opened, sends, started},
    fixtures,
};
use crate::{
    Action, Address, Event, Step,
    pages::{
        self,
        accounts::{SignInState, Way},
        connect_model::{self, Act},
    },
};
use jig_client::contract as c;
use skein_lib::{Queue, Token};
const PROVIDER: u32 = 17;
const NAME: [u8; 16] = [97; 16];
#[expect(clippy::manual_map, reason = "the step-test subset reads optional ways without closures")]
fn go(root: &mut crate::Domain, way: Option<Way>) {
    let way = match way {
        Some(way) => Some(connect_model::model_way(way)),
        None => None,
    };
    event(root, Event::Went { place: crate::Place::Address(Address::ConnectModel { way }) });
}
fn act(root: &mut crate::Domain, act: Act) -> Queue<crate::Request> {
    event(root, Event::Act { action: Action::Pages(pages::Act::ConnectModel(act)) })
}
fn root(way: Way) -> (crate::Domain, Token, Token) {
    let (mut root, me, accounts) = opened(c::HeldRole::Owner);
    go(&mut root, Some(way));
    (root, me, accounts)
}
#[test]
fn a_sign_in_way_sends_one_own_sign_in_request() {
    let way = Way::SignIn { provider: PROVIDER, plan: true };
    let (mut root, _, _) = root(way);
    let sent = sends(act(&mut root, Act::SignIn { name: NAME, way })).pop().unwrap();
    assert_eq!(
        sent.2,
        jig_client::Ask::Jig(Box::new(c::Ask::SignIn { holder: c::AccountHolder::Own, provider: PROVIDER }))
    );
    assert!(sends(act(&mut root, Act::SignIn { name: NAME, way })).is_empty());
}
#[test]
fn a_key_way_adds_a_new_account_with_the_value_privately_once() {
    let (mut root, _, _) = root(Way::Key { provider: PROVIDER });
    act(&mut root, Act::Holder { holder: c::AccountHolder::Deployment });
    key(&mut root, b"private-once");
    let sent = sends(act(&mut root, Act::Submit { name: NAME })).pop().unwrap();
    assert_eq!(
        sent.2,
        jig_client::Ask::Jig(Box::new(c::Ask::AccountChange {
            holder: c::AccountHolder::Deployment,
            change: c::AccountChange::AddKey {
                provider: PROVIDER,
                join: None,
                digest: skein_lib::sha256::digest(b"private-once")
            }
        }))
    );
    assert_eq!(sent.3.unwrap().expose(), b"private-once");
    assert!(!root.pages().connect_model().unwrap().request.has_key());
    assert!(sends(act(&mut root, Act::Submit { name: NAME })).is_empty());
}
#[test]
fn a_program_way_declares_the_default_name_for_the_person() {
    let (mut root, _, _) = root(Way::Program { provider: PROVIDER });
    assert!(root.pages().connect_model().unwrap().program.is_none());
    let sent = sends(act(&mut root, Act::Submit { name: NAME })).pop().unwrap();
    assert_eq!(
        sent.2,
        jig_client::Ask::Jig(Box::new(c::Ask::AccountChange {
            holder: c::AccountHolder::Own,
            change: c::AccountChange::DeclareProgram {
                provider: PROVIDER,
                join: None,
                name: Box::from(b"claude-pat".as_slice())
            }
        }))
    );
}
#[test]
fn an_invalid_or_overlong_program_name_is_never_declared() {
    for value in [b"".as_slice(), b"Alice", b"bad name", &[b'a'; 33]] {
        let (mut root, _, _) = root(Way::Program { provider: PROVIDER });
        act(&mut root, Act::ProgramName { value: Box::from(value) });
        assert!(sends(act(&mut root, Act::Submit { name: NAME })).is_empty());
    }
}
#[test]
fn the_program_default_uses_the_lowercased_first_name_within_the_name_bound() {
    for (person, expected) in [
        (b"ALICE Example".as_slice(), b"claude-alice".as_slice()),
        (b"abcdefghijklmnopqrstuvwxyz", b"claude-abcdefghijklmnopqrstuvwxy"),
    ] {
        let (mut root, watch, _) = root(Way::Program { provider: PROVIDER });
        let c::WatchEvent::Delivered(c::Delivery::Me(mut me)) = frame(c::HeldRole::Owner, fixtures::PARTY) else {
            unreachable!()
        };
        me.party.name = c::DocumentText::complete(c::Text::new(Box::from(person), 32).unwrap());
        deliver(&mut root, watch, c::WatchEvent::Delivered(c::Delivery::Me(me)));
        assert_eq!(root.pages().connect_model().unwrap().program_name(root.client()).as_ref(), expected);
    }
}
#[test]
fn a_program_name_at_its_bound_is_declared() {
    let (mut root, _, _) = root(Way::Program { provider: PROVIDER });
    act(&mut root, Act::ProgramName { value: Box::new([b'a'; 32]) });
    let sent = sends(act(&mut root, Act::Submit { name: NAME })).pop().unwrap();
    let jig_client::Ask::Jig(ask) = sent.2 else { panic!("jig request") };
    let c::Ask::AccountChange { change: c::AccountChange::DeclareProgram { name, .. }, .. } = *ask else {
        panic!("program declaration")
    };
    assert_eq!(name.as_ref(), [b'a'; 32]);
}
#[test]
fn a_key_at_capacity_can_create_a_standalone_account() {
    let (mut root, _, _) = root(Way::Key { provider: PROVIDER });
    key(&mut root, &[b'x'; 4096]);
    assert!(sends(act(&mut root, Act::Submit { name: NAME })).pop().is_some());
}
#[test]
fn the_model_draft_is_kept_between_steps_and_dropped_on_leaving() {
    let way = Way::Key { provider: PROVIDER };
    let (mut root, _, _) = root(way);
    act(&mut root, Act::Holder { holder: c::AccountHolder::Deployment });
    key(&mut root, b"private-draft");
    for address in [
        Address::ConnectModel { way: None },
        Address::Welcome { step: Step::Model { way: Some(connect_model::model_way(way)) } },
    ] {
        event(&mut root, Event::Went { place: crate::Place::Address(address) });
        let page = root.pages().connect_model().unwrap();
        assert!(page.request.has_key());
        assert_eq!(page.holder, c::AccountHolder::Deployment);
    }
    event(&mut root, Event::Went { place: crate::Place::Address(Address::Accounts) });
    assert!(root.pages().connect_model().is_none());
    go(&mut root, Some(way));
    assert!(!root.pages().connect_model().unwrap().request.has_key());
    assert_eq!(root.pages().connect_model().unwrap().holder, c::AccountHolder::Own);
}
#[test]
fn unavailable_and_demo_ways_resolve_to_choose() {
    let (mut root, _, _) = root(Way::Key { provider: PROVIDER + 1 });
    assert_eq!(connect_model::way(root.pages(), root.client()), None);
    go(&mut root, Some(Way::Key { provider: PROVIDER }));
    root.pages.demo_provider = Some(PROVIDER);
    assert_eq!(connect_model::way(root.pages(), root.client()), None);
}
#[test]
fn deployment_offers_neither_sign_in_nor_program() {
    let (mut root, _, _) = root(Way::SignIn { provider: PROVIDER, plan: true });
    act(&mut root, Act::Holder { holder: c::AccountHolder::Deployment });
    assert_eq!(connect_model::way(root.pages(), root.client()), None);
    assert!(
        sends(act(&mut root, Act::SignIn { name: NAME, way: Way::SignIn { provider: PROVIDER, plan: true } }))
            .is_empty()
    );
    go(&mut root, Some(Way::Program { provider: PROVIDER }));
    assert_eq!(connect_model::way(root.pages(), root.client()), None);
}
#[test]
fn a_sign_in_starts_then_ends_only_from_the_accounts_watch() {
    let way = Way::SignIn { provider: PROVIDER, plan: true };
    let (mut root, _, watch) = root(way);
    act(&mut root, Act::SignIn { name: NAME, way });
    let pending = root.pages().connect_model().unwrap().request.pending.as_ref().unwrap().key;
    started(&mut root, pending);
    assert_eq!(
        root.pages().connect_model().unwrap().request.sign_in_state(root.client()),
        SignInState::Started { source: 43 }
    );
    deliver(&mut root, watch, document(true));
    assert_eq!(
        root.pages().connect_model().unwrap().request.sign_in_state(root.client()),
        SignInState::Signing { source: 43 }
    );
    deliver(&mut root, watch, document(false));
    assert_eq!(
        root.pages().connect_model().unwrap().request.sign_in_state(root.client()),
        SignInState::Ended { source: 43 }
    );
    assert!(root.pages().connect_model().unwrap().request.sign_in_address(root.client()).is_none());
}
#[test]
fn creation_kept_work_belongs_to_the_open_model_flow() {
    for ask in [
        c::Ask::SignIn { holder: c::AccountHolder::Own, provider: PROVIDER },
        c::Ask::AccountChange {
            holder: c::AccountHolder::Own,
            change: c::AccountChange::DeclareProgram {
                provider: PROVIDER,
                join: None,
                name: Box::from(b"claude-pat".as_slice()),
            },
        },
        c::Ask::AccountChange {
            holder: c::AccountHolder::Own,
            change: c::AccountChange::AddKey { provider: PROVIDER, join: None, digest: [0; 32] },
        },
    ] {
        let (mut root, _, _) = root(Way::Key { provider: PROVIDER });
        crate::route::told(
            &mut root,
            &fixtures::env(&fixtures::limits()),
            jig_client::Told::Kept {
                key: jig_client::Key { asker: pages::me_handle().asker, number: 991 },
                request: jig_client::Ask::Jig(Box::new(ask.clone())),
            },
            &mut Queue::with_capacity(crate::max_out(&fixtures::limits())),
        );
        assert_eq!(root.pages().connect_model().unwrap().request.pending.as_ref().unwrap().ask, ask);
        event(&mut root, Event::Went { place: crate::Place::Address(Address::Accounts) });
        assert!(root.pages().accounts().unwrap().request.pending.is_none());
    }
}

#[test]
fn a_choose_sign_in_shows_the_way_in_the_same_flow() {
    let way = Way::SignIn { provider: PROVIDER, plan: true };
    for current in [Address::ConnectModel { way: None }, Address::Welcome { step: Step::Model { way: None } }] {
        let (mut root, _, _) = opened(c::HeldRole::Member);
        event(&mut root, Event::Went { place: crate::Place::Address(current.clone()) });
        let out = act(&mut root, Act::SignIn { name: NAME, way });
        let expected = crate::Request::Show {
            address: connect_model::address(&current, Some(connect_model::model_way(way))),
            replace: false,
        };
        let mut shown = false;
        for request in &out {
            shown |= *request == expected;
        }
        assert!(shown);
    }
}
#[test]
fn a_member_cannot_choose_deployment_accounts() {
    let (mut root, _, _) = opened(c::HeldRole::Member);
    go(&mut root, None);
    act(&mut root, Act::Holder { holder: c::AccountHolder::Deployment });
    assert_eq!(root.pages().connect_model().unwrap().holder, c::AccountHolder::Own);
}
#[test]
fn losing_the_owner_role_clears_a_deployment_key_draft() {
    let (mut root, me, _) = root(Way::Key { provider: PROVIDER });
    act(&mut root, Act::Holder { holder: c::AccountHolder::Deployment });
    key(&mut root, b"private-draft");
    deliver(&mut root, me, frame(c::HeldRole::Member, fixtures::PARTY));
    let page = root.pages().connect_model().unwrap();
    assert!(!page.request.has_key());
    assert_eq!(page.holder, c::AccountHolder::Own);
}
#[test]
fn other_accounts_kept_work_belongs_to_management_while_the_flow_is_open() {
    let (mut root, _, _) = root(Way::Key { provider: PROVIDER });
    let ask = c::Ask::AccountChange { holder: c::AccountHolder::Own, change: c::AccountChange::Resume { source: 43 } };
    crate::route::told(
        &mut root,
        &fixtures::env(&fixtures::limits()),
        jig_client::Told::Kept {
            key: jig_client::Key { asker: pages::me_handle().asker, number: 991 },
            request: jig_client::Ask::Jig(Box::new(ask.clone())),
        },
        &mut Queue::with_capacity(crate::max_out(&fixtures::limits())),
    );
    assert!(root.pages().connect_model().unwrap().request.pending.is_none());
    event(&mut root, Event::Went { place: crate::Place::Address(Address::Accounts) });
    assert_eq!(root.pages().accounts().unwrap().request.pending.as_ref().unwrap().ask, ask);
}
#[test]
fn invalid_creation_keys_keep_the_bound_error_and_send_nothing() {
    for (value, broken) in
        [(b"".as_slice(), pages::accounts::Broken::Empty), (&[b'x'; 4097], pages::accounts::Broken::TooLong)]
    {
        let (mut root, _, _) = root(Way::Key { provider: PROVIDER });
        key(&mut root, value);
        assert!(sends(act(&mut root, Act::Submit { name: NAME })).is_empty());
        assert_eq!(root.pages().connect_model().unwrap().request.broken, Some(broken));
    }
}

#[test]
fn a_restored_sign_in_address_is_live_only_while_the_watch_names_its_source() {
    let (mut root, _, watch) = opened(c::HeldRole::Member);
    go(&mut root, Some(Way::SignIn { provider: PROVIDER, plan: true }));
    let pending = jig_client::Key { asker: pages::me_handle().asker, number: 991 };
    crate::route::told(
        &mut root,
        &fixtures::env(&fixtures::limits()),
        jig_client::Told::Kept {
            key: pending,
            request: jig_client::Ask::Jig(Box::new(c::Ask::SignIn {
                holder: c::AccountHolder::Own,
                provider: PROVIDER,
            })),
        },
        &mut Queue::with_capacity(crate::max_out(&fixtures::limits())),
    );
    started(&mut root, pending);
    deliver(&mut root, watch, document(true));
    let page = root.pages().connect_model().unwrap();
    assert_eq!(page.request.sign_in_state(root.client()), SignInState::Signing { source: 43 });
    assert_eq!(page.request.sign_in_address(root.client()), Some(b"https://issuer/visit".as_slice()));
    let c::WatchEvent::Delivered(c::Delivery::OwnerAccounts(mut cut)) = document(false) else { unreachable!() };
    cut.cut = true;
    deliver(&mut root, watch, c::WatchEvent::Delivered(c::Delivery::OwnerAccounts(cut)));
    assert_eq!(root.pages().connect_model().unwrap().request.sign_in_state(root.client()), SignInState::Unknown);
    deliver(&mut root, watch, document(false));
    let page = root.pages().connect_model().unwrap();
    assert_eq!(page.request.sign_in_state(root.client()), SignInState::Ended { source: 43 });
    assert_eq!(page.request.sign_in_address(root.client()), None);
    act(&mut root, Act::SignIn { name: NAME, way: Way::SignIn { provider: PROVIDER, plan: true } });
    let key = root.pages().connect_model().unwrap().request.pending.as_ref().unwrap().key;
    started(&mut root, key);
    let page = root.pages().connect_model().unwrap();
    assert_eq!(page.request.sign_in_state(root.client()), SignInState::Started { source: 43 });
    assert!(page.request.sign_in_address(root.client()).is_some(), "an older watch does not end a new sign-in");
}

#[test]
fn a_reloaded_retirement_excludes_the_demo_way_and_its_submit() {
    let (mut root, _, _) = root(Way::Key { provider: PROVIDER });
    crate::tests::demo::deliver_demo_read(
        &mut root,
        crate::tests::demo::page_with_state(temper_client_protocol_wire::v1::DemoState::Ended),
    );
    assert_eq!(connect_model::way(root.pages(), root.client()), None);
    key(&mut root, b"fake-demo-key");
    assert!(sends(act(&mut root, Act::Submit { name: NAME })).is_empty());
}
