use super::fixtures;
use crate::{
    Action, Address, Event, Request,
    pages::{
        self,
        accounts::{self, Act, Operation, Row, Way},
    },
};
use jig_client::{Down, contract as c};
use skein_lib::{List, Queue, Secret, Token};
const PROVIDER: u32 = 17;
const ACCOUNT: u32 = 29;
const SOURCE: u32 = 43;
const NAME: [u8; 16] = [97; 16];
fn offer(provider: u32) -> c::ProviderOffer {
    let mut programs = List::with_capacity(1);
    programs.push(c::Text::new(Box::from(b"claude_code".as_slice()), 32).unwrap()).unwrap();
    c::ProviderOffer {
        provider,
        name: c::DocumentText::complete(c::Text::new(Box::from(b"openai".as_slice()), 32).unwrap()),
        key_bytes: 4096,
        sign_in: true,
        signed_in_plan: true,
        key: true,
        programs,
        programs_cut: false,
        none: false,
        manage_usage: None,
    }
}
fn unordered_offers() -> List<c::ProviderOffer> {
    let mut providers = List::with_capacity(3);
    for provider in [PROVIDER + 2, PROVIDER, PROVIDER + 1] {
        providers.push(offer(provider)).unwrap();
    }
    providers
}
#[test]
fn account_ways_are_ordered_by_provider_then_sign_in_program_and_key() {
    assert_eq!(
        accounts::ways(&unordered_offers(), None, c::AccountHolder::Own).as_slice(),
        &[
            Way::SignIn { provider: PROVIDER, plan: true },
            Way::Program { provider: PROVIDER },
            Way::Key { provider: PROVIDER },
            Way::SignIn { provider: PROVIDER + 1, plan: true },
            Way::Program { provider: PROVIDER + 1 },
            Way::Key { provider: PROVIDER + 1 },
            Way::SignIn { provider: PROVIDER + 2, plan: true },
            Way::Program { provider: PROVIDER + 2 },
            Way::Key { provider: PROVIDER + 2 },
        ]
    );
}
#[test]
fn account_ways_leave_out_the_demos_provider() {
    let ways = accounts::ways(&unordered_offers(), Some(PROVIDER + 1), c::AccountHolder::Own);
    for way in &ways {
        assert_ne!(way.provider(), PROVIDER + 1);
    }
}
#[test]
fn deployment_account_ways_offer_keys_only() {
    assert_eq!(
        accounts::ways(&unordered_offers(), None, c::AccountHolder::Deployment).as_slice(),
        &[Way::Key { provider: PROVIDER }, Way::Key { provider: PROVIDER + 1 }, Way::Key { provider: PROVIDER + 2 },]
    );
}
#[test]
fn a_provider_with_no_offered_sources_offers_no_way() {
    let mut providers = List::with_capacity(1);
    let mut provider = offer(PROVIDER);
    provider.sign_in = false;
    provider.key = false;
    provider.programs.clear();
    provider.programs_cut = true;
    providers.push(provider).unwrap();
    assert!(accounts::ways(&providers, None, c::AccountHolder::Own).is_empty());
}

#[test]
fn adding_a_key_requires_the_value_written_for_that_row() {
    let mut root = own_accounts();
    key(&mut root, b"private-once");
    let other = Row { holder: c::AccountHolder::Own, account: ACCOUNT + 1 };
    assert!(
        sends(act(
            &mut root,
            Act::Submit { name: NAME, row: other, operation: Operation::AddKey { provider: PROVIDER } }
        ))
        .is_empty()
    );
    assert_eq!(root.pages().accounts().unwrap().request.broken, Some(accounts::Broken::ValueAgain));
    assert!(!root.pages().accounts().unwrap().request.has_key());
}

pub(super) fn event(root: &mut crate::Domain, event: Event) -> Queue<Request> {
    let limits = fixtures::limits();
    let mut out = Queue::uncapped();
    crate::step(root, &fixtures::env(&limits), event, &mut out);
    crate::take(root, &fixtures::env(&limits), &mut out);
    out
}
fn act(root: &mut crate::Domain, act: Act) -> Queue<Request> {
    event(root, Event::Act { action: Action::Pages(pages::Act::Accounts(act)) })
}
pub(super) fn frame(role: c::HeldRole, party: u64) -> c::WatchEvent {
    let c::WatchEvent::Delivered(c::Delivery::Me(mut me)) = fixtures::frame(party) else { unreachable!() };
    me.deployment_role = Some(role);
    me.providers = List::with_capacity(1);
    me.providers.push(offer(PROVIDER)).unwrap();
    c::WatchEvent::Delivered(c::Delivery::Me(me))
}
pub(super) fn opened(role: c::HeldRole) -> (crate::Domain, Token, Token) {
    let mut root = crate::unlinked(&fixtures::limits(), fixtures::SEED);
    let (me, accounts) = fixtures::start_accounts(&mut root, List::with_capacity(0));
    deliver(&mut root, me, frame(role, fixtures::PARTY));
    event(&mut root, Event::Went { place: crate::Place::Address(Address::Accounts) });
    (root, me, accounts)
}
pub(super) fn deliver(root: &mut crate::Domain, watch: Token, event: c::WatchEvent) -> Queue<Request> {
    self::event(root, Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Watched { watch, event } } })
}
pub(super) fn document(signing: bool) -> c::WatchEvent {
    let mut rows = List::with_capacity(1);
    if signing {
        rows.push(c::OwnerAccountSignIn {
            source: SOURCE,
            provider: PROVIDER,
            holder: c::OwnerAccountHolder::Party(fixtures::PARTY),
        })
        .unwrap();
    }
    c::WatchEvent::Delivered(c::Delivery::OwnerAccounts(Box::new(c::OwnerAccounts {
        program_hosts: List::with_capacity(0),
        readiness: List::with_capacity(0),
        readiness_cut: false,
        accounts: List::with_capacity(0),
        signing: rows,
        last_end: None,
        usable: false,
        settled: !signing,
        cut: false,
    })))
}
fn select(root: &mut crate::Domain, holder: c::AccountHolder) {
    act(root, Act::KeyFor { row: Row { holder, account: ACCOUNT }, source: None });
}
pub(super) fn key(root: &mut crate::Domain, bytes: &[u8]) {
    event(root, Event::AccountKey { value: Secret::new(Box::from(bytes)) });
}
fn submit(root: &mut crate::Domain, operation: Operation) -> Queue<Request> {
    act(root, Act::Submit { name: NAME, row: root.pages().accounts().unwrap().key_for.unwrap().0, operation })
}
type Sent = (Token, [u8; 16], jig_client::Ask, Option<Secret>);
pub(super) fn sends(mut out: Queue<Request>) -> Queue<Sent> {
    let mut sent = Queue::with_capacity(4);
    while let Some(request) = out.pop() {
        if let Request::Client { request: jig_client::Request::Save { .. } } = &request {
            panic!("a value is never saved");
        }
        if let Request::Client {
            request: jig_client::Request::Down { down: Down::Send { attempt, key, request, value } },
        } = request
        {
            sent.push((attempt, key, request, value));
        }
    }
    sent
}
#[test]
fn accounts_address_prints_the_canonical_accounts_page_path() {
    assert_eq!(crate::address::print(&Address::Accounts).as_ref(), b"/accounts");
}
#[test]
fn accounts_addresses_parse_the_supported_accounts_page_forms() {
    for path in [b"/accounts".as_slice(), b"/accounts/?x=1"] {
        assert_eq!(crate::address::parse(path), crate::Place::Address(Address::Accounts));
    }
}
#[test]
fn accounts_addresses_refuse_a_path_with_an_extra_segment() {
    assert_eq!(crate::address::parse(b"/accounts/2"), crate::Place::Missing);
}
#[test]
fn account_availability_is_unknown_before_the_first_account_delivery() {
    let (root, _, _) = opened(c::HeldRole::Member);
    assert_eq!(root.pages().availability(root.client()), None);
}
#[test]
fn account_availability_reports_the_delivered_usable_account_projection() {
    let (mut root, _, watch) = opened(c::HeldRole::Member);
    let c::WatchEvent::Delivered(c::Delivery::OwnerAccounts(mut usable)) = document(false) else { unreachable!() };
    usable.usable = true;
    deliver(&mut root, watch, c::WatchEvent::Delivered(c::Delivery::OwnerAccounts(usable)));
    assert_eq!(root.pages().availability(root.client()), Some(true));
}
#[test]
fn account_availability_reports_unusable_accounts_outside_the_accounts_page() {
    let (mut root, _, _) = current_accounts(c::HeldRole::Member);
    assert_eq!(root.pages().availability(root.client()), Some(false));
    event(&mut root, Event::Went { place: crate::Place::Address(Address::Adopt { project: fixtures::PROJECT }) });
    assert_eq!(root.pages().availability(root.client()), Some(false));
}
#[test]
fn account_availability_becomes_unknown_when_its_watch_is_lost() {
    let (mut root, _, watch) = current_accounts(c::HeldRole::Member);
    event(
        &mut root,
        Event::Client {
            event: jig_client::Event::Up { up: jig_client::Up::WatchEnded { watch, end: c::WatchEnd::Dropped } },
        },
    );
    assert_eq!(root.client().document(accounts::handle()).unwrap().state, jig_client::State::Behind);
    assert_eq!(root.pages().availability(root.client()), None);
}
#[test]
fn account_availability_forgets_its_document_when_the_person_signs_out() {
    let (mut root, _, _) = current_accounts(c::HeldRole::Member);
    event(&mut root, Event::SignedOut);
    assert_eq!(root.pages().availability(root.client()), None);
    assert!(accounts::document(root.client()).is_none());
}
#[test]
fn account_availability_returns_after_signing_in_with_fresh_deliveries() {
    let (mut root, _, _) = current_accounts(c::HeldRole::Member);
    event(&mut root, Event::SignedOut);
    let mut opened =
        event(&mut root, Event::Client { event: jig_client::Event::SignIn { with: jig_client::With::Transport } });
    let mut me = None;
    let mut accounts = None;
    while let Some(request) = opened.pop() {
        if let Request::Client { request: jig_client::Request::Down { down: Down::Open { watch, subject } } } = request
        {
            if subject == jig_client::Watchable::Jig(c::Subject::Me) {
                me = Some(watch);
            } else if subject == jig_client::Watchable::Jig(c::Subject::Accounts) {
                accounts = Some(watch);
            }
        }
    }
    deliver(&mut root, me.expect("me reopened"), frame(c::HeldRole::Member, fixtures::PARTY));
    deliver(&mut root, accounts.expect("accounts reopened"), document(false));
    assert_eq!(root.pages().availability(root.client()), Some(false));
}
#[test]
fn account_availability_forgets_its_document_when_service_is_withdrawn() {
    let (mut root, _, _) = current_accounts(c::HeldRole::Member);
    event(
        &mut root,
        Event::Client { event: jig_client::Event::Up { up: jig_client::Up::Served { served: false, versions: None } } },
    );
    assert_eq!(root.pages().availability(root.client()), None);
    assert!(accounts::document(root.client()).is_none());
}
#[test]
fn owners_see_deployment_accounts_on_the_open_accounts_page() {
    let (root, _, _) = opened(c::HeldRole::Owner);
    assert!(root.pages().accounts().unwrap().deployment.is_some());
}
#[test]
fn losing_the_owner_role_clears_the_deployment_key_draft() {
    let (mut root, me) = deployment_draft();
    deliver(&mut root, me, frame(c::HeldRole::Member, fixtures::PARTY));
    let page = root.pages().accounts().unwrap();
    assert!(page.deployment.is_none());
    assert!(!page.request.has_key());
}
#[test]
fn members_cannot_submit_a_key_for_the_deployment_holder() {
    let (mut root, _, _) = opened(c::HeldRole::Member);
    select(&mut root, c::AccountHolder::Deployment);
    key(&mut root, b"private-draft");
    assert!(sends(submit(&mut root, Operation::AddKey { provider: PROVIDER })).is_empty());
}
#[test]
fn changing_the_party_clears_the_deployment_account_draft() {
    let (mut root, me) = deployment_draft();
    deliver(&mut root, me, frame(c::HeldRole::Member, fixtures::PARTY));
    select(&mut root, c::AccountHolder::Deployment);
    key(&mut root, b"private-draft");
    submit(&mut root, Operation::AddKey { provider: PROVIDER });
    deliver(&mut root, me, frame(c::HeldRole::Owner, fixtures::PARTY + 1));
    let page = root.pages().accounts().unwrap();
    assert!(!page.request.has_key());
    assert!(page.request.pending.is_none());
}
#[test]
fn leaving_the_accounts_page_discards_its_private_key_draft() {
    let (mut root, _) = deployment_draft();
    event(&mut root, Event::Went { place: crate::Place::Missing });
    assert!(root.pages().accounts().is_none());
    event(&mut root, Event::Went { place: crate::Place::Address(Address::Accounts) });
    assert!(!root.pages().accounts().unwrap().request.has_key());
}
#[test]
fn signing_out_closes_the_accounts_page_with_its_draft() {
    let (mut root, _) = deployment_draft();
    event(&mut root, Event::SignedOut);
    assert!(root.pages().accounts().is_none());
}
#[test]
#[expect(clippy::disallowed_macros, reason = "the test captures Debug to verify secret redaction")]
fn adding_a_joined_key_sends_its_value_once_privately() {
    let mut root = own_accounts();
    let drawn = root.pages().account_name();
    key(&mut root, b"private-once");
    assert_eq!(root.pages().account_name(), drawn, "the name is independent of the value");
    let mut sent = sends(submit(&mut root, Operation::AddKey { provider: PROVIDER }));
    let (_, name, ask, value) = sent.pop().expect("one value send");
    assert_eq!(name, NAME);
    assert_eq!(value.unwrap().expose(), b"private-once");
    assert_eq!(
        ask,
        jig_client::Ask::Jig(Box::new(c::Ask::AccountChange {
            holder: c::AccountHolder::Own,
            change: c::AccountChange::AddKey {
                provider: PROVIDER,
                join: Some(ACCOUNT),
                digest: skein_lib::sha256::digest(b"private-once")
            },
        }))
    );
    assert!(!root.pages().accounts().unwrap().request.has_key());
    assert!(!format!("{:?}", root.pages()).contains("private-once"));
}
#[test]
fn account_keys_outside_the_value_bounds_are_never_sent() {
    let mut root = own_accounts();
    for bytes in [b"".as_slice(), &[b'x'; 4097]] {
        key(&mut root, bytes);
        assert!(sends(submit(&mut root, Operation::AddKey { provider: PROVIDER })).is_empty());
    }
}
#[test]
fn a_pending_joined_key_does_not_send_a_second_press() {
    let (mut root, _) = submitted_key();
    assert!(sends(submit(&mut root, Operation::AddKey { provider: PROVIDER })).is_empty());
}
#[test]
fn a_lost_key_send_requires_the_value_to_be_reentered() {
    let (mut root, _) = lost_key();
    assert!(sends(submit(&mut root, Operation::AddKey { provider: PROVIDER })).is_empty());
    assert_eq!(root.pages().account_name(), NAME);
}
#[test]
fn a_lost_key_send_refuses_a_different_retry_name() {
    let (mut root, _) = lost_key();
    key(&mut root, b"private-once");
    assert!(
        sends(act(
            &mut root,
            Act::Submit {
                name: [98; 16],
                row: Row { holder: c::AccountHolder::Own, account: ACCOUNT },
                operation: Operation::AddKey { provider: PROVIDER }
            }
        ))
        .is_empty()
    );
}
#[test]
fn a_same_name_key_retry_preserves_the_original_request() {
    let (mut root, ask) = lost_key();
    key(&mut root, b"private-once");
    let retry = sends(submit(&mut root, Operation::AddKey { provider: PROVIDER })).pop().expect("explicit retry");
    assert_eq!(retry.1, NAME);
    assert_eq!(retry.2, ask);
    assert_eq!(retry.3.unwrap().expose(), b"private-once");
}
#[test]
fn an_again_answer_leaves_the_key_retry_unsent() {
    let (mut root, _) = lost_key();
    key(&mut root, b"private-once");
    let retry = sends(submit(&mut root, Operation::AddKey { provider: PROVIDER })).pop().expect("explicit retry");
    let out = event(
        &mut root,
        Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Answer {
                    attempt: retry.0,
                    envelope: skein_lib::RequestEnvelope::Again,
                    answer: None,
                    why: None,
                },
            },
        },
    );
    assert!(sends(out).is_empty());
    assert_eq!(
        root.pages().accounts().unwrap().request.pending.as_ref().unwrap().outcome,
        Some(jig_client::Outcome::Unsent)
    );
}

#[test]
fn account_operations_send_the_row_holder_numbers_without_confirmation() {
    let (root, _, _) = opened(c::HeldRole::Owner);
    for holder in [c::AccountHolder::Own, c::AccountHolder::Deployment] {
        for (operation, ask) in [
            (Operation::Resume { source: SOURCE }, change(holder, c::AccountChange::Resume { source: SOURCE })),
            (
                Operation::CheckProgram { source: SOURCE },
                change(holder, c::AccountChange::CheckProgram { source: SOURCE }),
            ),
            (
                Operation::MoveAccount { place: 1 },
                change(holder, c::AccountChange::MoveAccount { account: ACCOUNT, place: 1 }),
            ),
            (
                Operation::MoveSource { source: SOURCE, place: 2 },
                change(holder, c::AccountChange::MoveSource { account: ACCOUNT, source: SOURCE, place: 2 }),
            ),
        ] {
            let (mut page, mut serial, mut out) = operation_page(root.client(), holder);
            page.act(
                root.client(),
                Act::Submit { name: NAME, row: Row { holder, account: ACCOUNT }, operation },
                &mut serial,
                &mut out,
            );
            assert_operation_send(&mut out, ask);
            assert_finished_operation(&mut page, root.client(), &mut serial, &mut out);
        }
    }
}
#[test]
fn destructive_account_operations_wait_for_confirmation_before_sending() {
    let (root, _, _) = opened(c::HeldRole::Owner);
    for holder in [c::AccountHolder::Own, c::AccountHolder::Deployment] {
        for (operation, ask) in [
            (Operation::SignOut { source: SOURCE }, c::Ask::SignOut { holder, source: SOURCE }),
            (Operation::RemoveKey { source: SOURCE }, change(holder, c::AccountChange::RemoveKey { source: SOURCE })),
            (Operation::RemoveAccount, change(holder, c::AccountChange::Remove { account: ACCOUNT })),
        ] {
            let (mut page, mut serial, mut out) = operation_page(root.client(), holder);
            page.act(
                root.client(),
                Act::Submit { name: NAME, row: Row { holder, account: ACCOUNT }, operation },
                &mut serial,
                &mut out,
            );
            assert!(out.is_empty());
            assert_eq!(page.request.confirm, Some((NAME, ask.clone())));
            page.act(root.client(), Act::Confirm, &mut serial, &mut out);
            assert_operation_send(&mut out, ask);
            assert_finished_operation(&mut page, root.client(), &mut serial, &mut out);
        }
    }
}
#[test]
fn key_operations_carry_the_row_holder_numbers_with_the_value() {
    let (root, _, _) = opened(c::HeldRole::Owner);
    let digest = skein_lib::sha256::digest(b"private-once");
    for holder in [c::AccountHolder::Own, c::AccountHolder::Deployment] {
        for (operation, ask) in [
            (
                Operation::AddKey { provider: PROVIDER },
                change(holder, c::AccountChange::AddKey { provider: PROVIDER, join: Some(ACCOUNT), digest }),
            ),
            (
                Operation::ReplaceKey { source: SOURCE },
                change(holder, c::AccountChange::ReplaceKey { source: SOURCE, digest }),
            ),
        ] {
            let (mut page, mut serial, mut out) = operation_page(root.client(), holder);
            if let Operation::ReplaceKey { source } = operation {
                page.act(
                    root.client(),
                    Act::KeyFor { row: Row { holder, account: ACCOUNT }, source: Some(source) },
                    &mut serial,
                    &mut out,
                );
                page.request.key(Secret::new(Box::from(b"private-once".as_slice())));
            }
            page.act(
                root.client(),
                Act::Submit { name: NAME, row: Row { holder, account: ACCOUNT }, operation },
                &mut serial,
                &mut out,
            );
            let request = out.pop().expect("one exact operation");
            let pages::Request::Client(jig_client::Event::Carry { name, request, value, .. }) = request else {
                panic!("keyed operation");
            };
            assert_eq!(name, NAME);
            assert_eq!(request, jig_client::Ask::Jig(Box::new(ask)));
            assert_eq!(value.expect("carried private value").expose(), b"private-once");
            assert_finished_operation(&mut page, root.client(), &mut serial, &mut out);
        }
    }
}
#[test]
fn account_move_operations_forward_the_engines_zero_based_first_place() {
    let (root, _, _) = opened(c::HeldRole::Owner);
    for (operation, change_) in [
        (Operation::MoveAccount { place: 0 }, c::AccountChange::MoveAccount { account: ACCOUNT, place: 0 }),
        (
            Operation::MoveSource { source: SOURCE, place: 0 },
            c::AccountChange::MoveSource { account: ACCOUNT, source: SOURCE, place: 0 },
        ),
    ] {
        let mut page = accounts::Accounts::default();
        let mut out = Queue::with_capacity(4);
        page.act(
            root.client(),
            Act::Submit { name: NAME, row: Row { holder: c::AccountHolder::Own, account: ACCOUNT }, operation },
            &mut 0,
            &mut out,
        );
        assert_operation_send(&mut out, change(c::AccountHolder::Own, change_));
    }
}

fn change(holder: c::AccountHolder, change: c::AccountChange) -> c::Ask {
    c::Ask::AccountChange { holder, change }
}
#[test]
fn another_partys_restored_account_work_is_not_shown() {
    let mut root = crate::unlinked(&fixtures::limits(), fixtures::SEED);
    let mut kept = List::with_capacity(1);
    kept.push(skein_lib::RequestRecord {
        key: NAME,
        scope: fixtures::PARTY.checked_add(1).unwrap(),
        first_sent: Some(skein_lib::Wall::EPOCH),
        request: jig_client::Held {
            key: jig_client::Key { asker: pages::me_handle().asker, number: 991 },
            request: jig_client::Ask::Jig(Box::new(c::Ask::SignIn {
                holder: c::AccountHolder::Own,
                provider: PROVIDER,
            })),
        },
    })
    .unwrap();
    let (me, _) = fixtures::start_accounts(&mut root, kept);
    event(&mut root, Event::Went { place: crate::Place::Address(Address::Accounts) });
    assert!(root.pages().accounts().is_none());
    deliver(&mut root, me, frame(c::HeldRole::Member, fixtures::PARTY));
    assert!(root.pages().accounts().unwrap().request.pending.is_none());
}

pub(super) fn started(root: &mut crate::Domain, pending: jig_client::Key) {
    crate::route::told(
        root,
        &fixtures::env(&fixtures::limits()),
        jig_client::Told::Answered {
            key: pending,
            outcome: jig_client::Outcome::Answered(jig_client::Answered::Jig(c::Answer::Done(c::Outcome::Owner(
                c::OwnerOutcome::AccountSignInStarted {
                    source: SOURCE,
                    address: Box::from(b"https://issuer/visit".as_slice()),
                },
            )))),
        },
        &mut Queue::uncapped(),
    );
}

fn current_accounts(role: c::HeldRole) -> (crate::Domain, Token, Token) {
    let (mut root, me, watch) = opened(role);
    deliver(&mut root, watch, document(false));
    (root, me, watch)
}
fn deployment_draft() -> (crate::Domain, Token) {
    let (mut root, me, _) = opened(c::HeldRole::Owner);
    select(&mut root, c::AccountHolder::Deployment);
    key(&mut root, b"private-draft");
    (root, me)
}
fn own_accounts() -> crate::Domain {
    let (mut root, _, _) = opened(c::HeldRole::Member);
    select(&mut root, c::AccountHolder::Own);
    root
}
fn submitted_key() -> (crate::Domain, Sent) {
    let mut root = own_accounts();
    key(&mut root, b"private-once");
    let sent = sends(submit(&mut root, Operation::AddKey { provider: PROVIDER })).pop().expect("one value send");
    (root, sent)
}
fn lost_key() -> (crate::Domain, jig_client::Ask) {
    let (mut root, (attempt, _, ask, _)) = submitted_key();
    event(
        &mut root,
        Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Answer {
                    attempt,
                    envelope: skein_lib::RequestEnvelope::Lost,
                    answer: None,
                    why: None,
                },
            },
        },
    );
    (root, ask)
}
fn operation_page(
    client: &jig_client::Client,
    holder: c::AccountHolder,
) -> (accounts::Accounts, u64, Queue<pages::Request>) {
    let mut page = accounts::Accounts::default();
    let mut out = Queue::with_capacity(8);
    let mut serial = 0;
    page.act(client, Act::KeyFor { row: Row { holder, account: ACCOUNT }, source: None }, &mut serial, &mut out);
    page.request.key(Secret::new(Box::from(b"private-once".as_slice())));
    (page, serial, out)
}
fn assert_operation_send(out: &mut Queue<pages::Request>, ask: c::Ask) {
    let request = out.pop().expect("one exact operation");
    let pages::Request::Client(jig_client::Event::Send { name, request, .. }) = request else {
        panic!("keyed operation");
    };
    assert_eq!(name, Some(NAME));
    assert_eq!(request, jig_client::Ask::Jig(Box::new(ask)));
}
fn assert_finished_operation(
    page: &mut accounts::Accounts,
    client: &jig_client::Client,
    serial: &mut u64,
    out: &mut Queue<pages::Request>,
) {
    assert!(out.is_empty());
    assert!(!page.request.has_key());
    page.act(client, Act::Confirm, serial, out);
    assert!(out.is_empty());
}

#[test]
fn cancel_abandons_a_named_lost_account_act() {
    for envelope in [skein_lib::RequestEnvelope::Lost, skein_lib::RequestEnvelope::Again] {
        let (mut root, _, _) = opened(c::HeldRole::Member);
        select(&mut root, c::AccountHolder::Own);
        key(&mut root, b"first-key");
        let attempt = sends(submit(&mut root, Operation::AddKey { provider: PROVIDER })).pop().unwrap().0;
        event(
            &mut root,
            Event::Client {
                event: jig_client::Event::Up {
                    up: jig_client::Up::Answer { attempt, envelope, answer: None, why: None },
                },
            },
        );
        act(&mut root, Act::Cancel);
        assert!(root.pages().accounts().unwrap().request.pending.is_none());
        key(&mut root, b"another-key");
        assert_eq!(
            sends(act(
                &mut root,
                Act::Submit {
                    name: [99; 16],
                    row: Row { holder: c::AccountHolder::Own, account: ACCOUNT },
                    operation: Operation::AddKey { provider: PROVIDER }
                }
            ))
            .len(),
            1
        );
    }
}
#[test]
fn cancel_abandons_a_restored_lost_account_act() {
    for outcome in [jig_client::Outcome::Lost, jig_client::Outcome::Unsent] {
        let (mut root, _, _) = opened(c::HeldRole::Member);
        let key = jig_client::Key { asker: pages::me_handle().asker, number: 991 };
        let mut out = Queue::uncapped();
        crate::route::told(
            &mut root,
            &fixtures::env(&fixtures::limits()),
            jig_client::Told::Kept {
                key,
                request: jig_client::Ask::Jig(Box::new(c::Ask::SignIn {
                    holder: c::AccountHolder::Own,
                    provider: PROVIDER,
                })),
            },
            &mut out,
        );
        crate::route::told(
            &mut root,
            &fixtures::env(&fixtures::limits()),
            jig_client::Told::Answered { key, outcome },
            &mut out,
        );
        assert_eq!(root.pages().accounts().unwrap().request.pending.as_ref().unwrap().name, None);
        act(&mut root, Act::Cancel);
        select(&mut root, c::AccountHolder::Own);
        assert_eq!(sends(submit(&mut root, Operation::Resume { source: SOURCE })).len(), 1);
    }
}
#[test]
fn typed_account_lists_outlive_the_task_window() {
    let (mut root, _, watch) = opened(c::HeldRole::Member);
    let count = fixtures::limits().client.domain.rows.checked_add(1).unwrap();
    let mut rows = List::with_capacity(count);
    let mut signing = List::with_capacity(count);
    for number in 1..=count {
        let mut sources = List::with_capacity(count);
        for source in 1..=count {
            sources
                .push(c::OwnerAccountSource {
                    name: None,
                    place: source,
                    number: source,
                    kind: c::OwnerAccountKind::Key,
                    state: c::OwnerAccountState::Granted,
                    usable: true,
                    expiring: false,
                    attention: false,
                    spent_until: None,
                })
                .unwrap();
        }
        rows.push(c::OwnerAccountLine {
            number,
            place: number,
            uses: None,
            plan: false,
            provider: Some(PROVIDER),
            provider_name: Some(c::DocumentText::complete(
                c::Text::new(Box::from(b"Scripted".as_slice()), 32).unwrap(),
            )),
            manage_usage: None,
            display: c::DocumentText::complete(c::Text::new(Box::from(b"Account".as_slice()), 32).unwrap()),
            holder: c::OwnerAccountHolder::Party(fixtures::PARTY),
            sources,
            cut: false,
        })
        .unwrap();
        signing
            .push(c::OwnerAccountSignIn {
                source: number,
                provider: PROVIDER,
                holder: c::OwnerAccountHolder::Party(fixtures::PARTY),
            })
            .unwrap();
    }
    let doc = c::OwnerAccounts {
        program_hosts: List::with_capacity(0),
        readiness: List::with_capacity(0),
        readiness_cut: false,
        accounts: rows,
        signing,
        last_end: None,
        usable: true,
        settled: false,
        cut: false,
    };
    deliver(&mut root, watch, c::WatchEvent::Delivered(c::Delivery::OwnerAccounts(Box::new(doc.clone()))));
    assert_eq!(accounts::document(root.client()), Some(&doc));
}

#[test]
fn key_for_another_row_or_source_drops_the_previous_value() {
    for (row, source) in [
        (Row { holder: c::AccountHolder::Own, account: ACCOUNT + 1 }, None),
        (Row { holder: c::AccountHolder::Own, account: ACCOUNT }, Some(SOURCE)),
        (Row { holder: c::AccountHolder::Deployment, account: ACCOUNT }, None),
    ] {
        let (mut root, _, _) = opened(c::HeldRole::Owner);
        select(&mut root, c::AccountHolder::Own);
        key(&mut root, b"private-once");
        act(&mut root, Act::KeyFor { row, source });
        assert!(!root.pages().accounts().unwrap().request.has_key());
    }
}
#[test]
fn key_for_the_same_field_preserves_its_value() {
    let mut root = own_accounts();
    key(&mut root, b"private-once");
    select(&mut root, c::AccountHolder::Own);
    assert!(root.pages().accounts().unwrap().request.has_key());
}
#[test]
fn a_rows_sign_in_sends_its_provider_and_follows_the_accounts_watch() {
    let (mut root, _, watch) = opened(c::HeldRole::Member);
    let row = Row { holder: c::AccountHolder::Own, account: ACCOUNT };
    let sent =
        sends(act(&mut root, Act::Submit { name: NAME, row, operation: Operation::SignIn { provider: PROVIDER } }))
            .pop()
            .unwrap();
    assert_eq!(sent.2, jig_client::Ask::Jig(Box::new(c::Ask::SignIn { holder: row.holder, provider: PROVIDER })));
    let pending = root.pages().accounts().unwrap().request.pending.as_ref().unwrap().key;
    started(&mut root, pending);
    deliver(&mut root, watch, document(true));
    assert_eq!(
        root.pages().accounts().unwrap().request.sign_in_state(root.client()),
        accounts::SignInState::Signing { source: SOURCE }
    );
    assert!(
        sends(act(&mut root, Act::Submit { name: [99; 16], row, operation: Operation::Resume { source: SOURCE } }))
            .is_empty()
    );
    deliver(&mut root, watch, document(false));
    let page = root.pages().accounts().unwrap();
    assert_eq!(page.request.sign_in_state(root.client()), accounts::SignInState::Ended { source: SOURCE });
    assert_eq!(page.request_row, Some(row));
}
#[test]
fn a_deployment_row_cannot_sign_in() {
    let (mut root, _, _) = opened(c::HeldRole::Owner);
    let row = Row { holder: c::AccountHolder::Deployment, account: ACCOUNT };
    assert!(
        sends(act(&mut root, Act::Submit { name: NAME, row, operation: Operation::SignIn { provider: PROVIDER } }))
            .is_empty()
    );
}

#[test]
fn a_pending_row_request_blocks_a_different_rows_act() {
    let (mut root, _) = submitted_key();
    let row = Row { holder: c::AccountHolder::Own, account: ACCOUNT + 1 };
    assert!(
        sends(act(&mut root, Act::Submit { name: [99; 16], row, operation: Operation::Resume { source: SOURCE + 1 } }))
            .is_empty()
    );
}
