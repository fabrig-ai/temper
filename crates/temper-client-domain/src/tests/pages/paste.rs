#![allow(
    clippy::disallowed_macros,
    clippy::disallowed_methods,
    clippy::disallowed_types,
    reason = "typed request and secret-redaction assertions"
)]
use super::{
    accounts::{deliver, document, event, opened, sends, started},
    fixtures,
};
use crate::{
    Action, Address, Event,
    pages::{
        self,
        accounts::{Operation, PasteState as S, Row, Way, request::Request},
        connect_model::Act,
    },
};
use jig_client::{Told, contract as c};
use skein_lib::{Queue, Secret};
const NAME: [u8; 16] = [98; 16];
const WAY: Way = Way::SignIn { provider: 17, plan: true };
fn secret(bytes: &[u8]) -> Secret {
    Secret::new(Box::from(bytes))
}
fn out() -> Queue<pages::Request> {
    Queue::with_capacity(4)
}
fn form(root: &crate::Domain) -> Request {
    let mut request = Request::default();
    request.send(NAME, c::Ask::SignIn { holder: c::AccountHolder::Own, provider: 17 }, false, &mut 0, &mut out());
    let key = request.pending.as_ref().unwrap().key;
    request.told(
        root.client(),
        &Told::Answered {
            key,
            outcome: finishing_answer(c::OwnerOutcome::AccountSignInStarted {
                source: 43,
                address: Box::from(b"https://issuer.test/start".as_slice()),
            }),
        },
    );
    request
}
fn finishing_answer(outcome: c::OwnerOutcome) -> jig_client::Outcome {
    jig_client::Outcome::Answered(jig_client::Answered::Jig(c::Answer::Done(c::Outcome::Owner(outcome))))
}
fn refusal(why: c::SignInRedirectWhy) -> jig_client::Outcome {
    jig_client::Outcome::Answered(jig_client::Answered::Jig(c::Answer::Refused {
        refusal: c::Refusal::SignInRedirect { why },
        at: None,
    }))
}
fn finish(request: &mut Request, root: &crate::Domain) -> Queue<pages::Request> {
    let mut result = out();
    request.finish(root.client(), NAME, &mut 10, &mut result);
    result
}
fn answer(request: &mut Request, root: &crate::Domain, outcome: jig_client::Outcome) {
    let key = request.paste.pending.as_ref().unwrap().key;
    assert!(request.told(root.client(), &Told::Answered { key, outcome }));
}
fn end(root: &mut crate::Domain, watch: skein_lib::Token) {
    let c::WatchEvent::Delivered(c::Delivery::OwnerAccounts(mut doc)) = document(false) else { unreachable!() };
    doc.last_end =
        Some(c::OwnerAccountLastEnd { source: 43, end: c::OwnerAccountEnd::Made { account: 29, declined: false } });
    deliver(root, watch, c::WatchEvent::Delivered(c::Delivery::OwnerAccounts(doc)));
}
#[test]
fn the_paste_waits_under_a_started_sign_in_and_is_closed_without_one() {
    let (root, _, _) = opened(c::HeldRole::Member);
    assert_eq!(Request::default().paste_state(root.client()), S::Closed);
    assert_eq!(form(&root).paste_state(root.client()), S::Waiting { ready: false });
}
#[test]
fn a_paste_is_trimmed_around_and_held_until_sent() {
    let (root, _, _) = opened(c::HeldRole::Member);
    let mut request = form(&root);
    request.paste_value(secret(b" \t\r\nfixture address\n\r\t "), 256);
    assert_eq!(request.paste_state(root.client()), S::Waiting { ready: true });
    let pages::Request::Client(jig_client::Event::Carry { request: ask, value, .. }) =
        finish(&mut request, &root).pop().unwrap()
    else {
        panic!("carry")
    };
    assert!(value.unwrap().expose() == b"fixture address");
    assert_eq!(
        ask,
        jig_client::Ask::Jig(Box::new(c::Ask::AccountChange {
            holder: c::AccountHolder::Own,
            change: c::AccountChange::FinishSignIn {
                source: 43,
                digest: skein_lib::sha256::digest(b"fixture address")
            }
        }))
    );
}
#[test]
fn a_paste_at_the_bound_is_held_and_one_past_it_is_too_long() {
    let (root, _, _) = opened(c::HeldRole::Member);
    let mut request = form(&root);
    for (bytes, state) in [
        (b"abcd".as_slice(), S::Waiting { ready: true }),
        (b"abcde", S::TooLong),
        (b" \t\r\n", S::Waiting { ready: false }),
    ] {
        request.paste_value(secret(bytes), 4);
        assert_eq!(request.paste_state(root.client()), state);
    }
}
#[test]
fn finish_sends_one_carried_finish_for_the_sign_ins_source_and_drops_the_value() {
    let (root, _, _) = opened(c::HeldRole::Member);
    let mut request = form(&root);
    request.paste_value(secret(b"fixture"), 256);
    let written = request.paste.written();
    let original = request.pending.as_ref().unwrap().key;
    assert_eq!(finish(&mut request, &root).len(), 1);
    assert!(finish(&mut request, &root).is_empty());
    assert_eq!(request.paste.written(), written + 1);
    assert_eq!(request.pending.as_ref().unwrap().key, original);
    answer(&mut request, &root, jig_client::Outcome::Lost);
    assert!(finish(&mut request, &root).is_empty());
}
#[test]
fn a_finish_out_is_sending_and_the_page_is_busy() {
    let (root, _, _) = opened(c::HeldRole::Member);
    let mut request = form(&root);
    request.paste_value(secret(b"fixture"), 256);
    finish(&mut request, &root);
    request.paste_value(secret(b"another"), 256);
    assert_eq!(request.paste_state(root.client()), S::Sending);
    assert!(request.busy(root.client()));
    let key = request.paste.pending.as_ref().unwrap().key;
    assert!(request.told(root.client(), &Told::Pending { key }));
    answer(&mut request, &root, jig_client::Outcome::Unsent);
    assert!(finish(&mut request, &root).is_empty());
}
#[test]
fn answered_finishing_reads_finishing_until_the_watch_shows_the_end() {
    let (mut root, _, watch) = opened(c::HeldRole::Member);
    let mut request = form(&root);
    request.paste_value(secret(b"fixture"), 256);
    finish(&mut request, &root);
    answer(&mut request, &root, finishing_answer(c::OwnerOutcome::AccountFinishing { source: 43 }));
    assert_eq!(request.paste_state(root.client()), S::Finishing);
    end(&mut root, watch);
    request.refresh(root.client());
    assert_eq!(request.paste_state(root.client()), S::Closed);
}
#[test]
fn refused_address_state_and_too_many_keep_the_sign_in_waiting_and_take_another_paste() {
    for why in [c::SignInRedirectWhy::Address, c::SignInRedirectWhy::State, c::SignInRedirectWhy::TooMany] {
        let (root, _, _) = opened(c::HeldRole::Member);
        let mut request = form(&root);
        request.paste_value(secret(b"fixture"), 256);
        finish(&mut request, &root);
        answer(&mut request, &root, refusal(why));
        assert_eq!(request.paste_state(root.client()), S::Refused { why });
        assert!(finish(&mut request, &root).is_empty());
        request.paste_value(secret(b"next fixture"), 256);
        assert_eq!(request.paste_state(root.client()), S::Waiting { ready: true });
        assert_eq!(finish(&mut request, &root).len(), 1);
    }
}
#[test]
fn refused_not_waiting_follows_the_watch_end_or_reads_ended() {
    let (mut root, _, watch) = opened(c::HeldRole::Member);
    let mut request = form(&root);
    request.paste_value(secret(b"fixture"), 256);
    finish(&mut request, &root);
    answer(&mut request, &root, refusal(c::SignInRedirectWhy::NotWaiting));
    assert_eq!(request.paste_state(root.client()), S::Ended);
    end(&mut root, watch);
    request.refresh(root.client());
    assert_eq!(request.paste_state(root.client()), S::Closed);
}
#[test]
fn a_sign_in_gone_without_an_end_reads_ended() {
    let (mut root, _, watch) = opened(c::HeldRole::Member);
    let mut request = form(&root);
    deliver(&mut root, watch, document(true));
    request.refresh(root.client());
    deliver(&mut root, watch, document(false));
    request.refresh(root.client());
    assert_eq!(request.paste_state(root.client()), S::Ended);
}
#[test]
fn lost_and_unsent_ask_for_the_paste_again_and_send_nothing() {
    for outcome in [jig_client::Outcome::Lost, jig_client::Outcome::Unsent] {
        let (root, _, _) = opened(c::HeldRole::Member);
        let mut request = form(&root);
        request.paste_value(secret(b"fixture"), 256);
        finish(&mut request, &root);
        answer(&mut request, &root, outcome);
        assert_eq!(request.paste_state(root.client()), S::Again);
        assert!(finish(&mut request, &root).is_empty());
    }
}
#[test]
fn leaving_the_page_or_a_new_sign_in_drops_the_paste() {
    let (root, _, _) = opened(c::HeldRole::Member);
    for new_sign_in in [false, true] {
        let mut request = form(&root);
        request.paste_value(secret(b"fixture"), 256);
        if new_sign_in {
            request.send(
                NAME,
                c::Ask::SignIn { holder: c::AccountHolder::Own, provider: 17 },
                false,
                &mut 2,
                &mut out(),
            );
        } else {
            request.clear_key();
        }
        assert!(finish(&mut request, &root).is_empty());
    }
}
fn model(root: &mut crate::Domain, act: Act) -> Queue<crate::Request> {
    event(root, Event::Act { action: Action::Pages(pages::Act::ConnectModel(act)) })
}
fn flow(root: &mut crate::Domain) {
    event(
        root,
        Event::Went {
            place: crate::Place::Address(Address::ConnectModel { way: Some(pages::connect_model::model_way(WAY)) }),
        },
    );
}
#[test]
fn an_accounts_rows_paste_finishes_that_rows_sign_in() {
    let (mut root, _, watch) = opened(c::HeldRole::Member);
    let c::WatchEvent::Delivered(c::Delivery::OwnerAccounts(mut doc)) = document(false) else { unreachable!() };
    doc.accounts = skein_lib::List::with_capacity(1);
    doc.accounts
        .push(c::OwnerAccountLine {
            number: 29,
            place: 0,
            uses: None,
            plan: true,
            provider: Some(17),
            provider_name: None,
            manage_usage: None,
            display: c::DocumentText::complete(c::Text::new(Box::from(b"fixture".as_slice()), 32).unwrap()),
            holder: c::OwnerAccountHolder::Party(fixtures::PARTY),
            sources: skein_lib::List::with_capacity(0),
            cut: false,
        })
        .unwrap();
    deliver(&mut root, watch, c::WatchEvent::Delivered(c::Delivery::OwnerAccounts(doc)));
    let row = Row { holder: c::AccountHolder::Own, account: 29 };
    event(
        &mut root,
        Event::Act {
            action: Action::Pages(pages::Act::Accounts(pages::accounts::Act::Submit {
                name: NAME,
                row,
                operation: Operation::SignIn { provider: 17 },
            })),
        },
    );
    let key = root.pages().accounts().unwrap().request.pending.as_ref().unwrap().key;
    started(&mut root, key);
    event(&mut root, Event::SignInAddress { value: secret(b"fixture") });
    let sent = sends(event(
        &mut root,
        Event::Act { action: Action::Pages(pages::Act::Accounts(pages::accounts::Act::Finish { name: [99; 16] })) },
    ))
    .pop()
    .unwrap();
    assert_eq!(
        sent.2,
        jig_client::Ask::Jig(Box::new(c::Ask::AccountChange {
            holder: c::AccountHolder::Own,
            change: c::AccountChange::FinishSignIn { source: 43, digest: skein_lib::sha256::digest(b"fixture") }
        }))
    );
    assert_eq!(root.pages().accounts().unwrap().request_row, Some(row));
}
#[test]
fn signing_in_elsewhere_sends_the_same_sign_in_and_departs_nowhere() {
    let (mut root, _, _) = opened(c::HeldRole::Member);
    root.departing = true;
    flow(&mut root);
    let sent = sends(model(&mut root, Act::SignInElsewhere { name: NAME, way: WAY })).pop().unwrap();
    assert_eq!(sent.2, jig_client::Ask::Jig(Box::new(c::Ask::SignIn { holder: c::AccountHolder::Own, provider: 17 })));
    let key = root.pages().connect_model().unwrap().request.pending.as_ref().unwrap().key;
    let mut result = Queue::uncapped();
    crate::route::told(
        &mut root,
        &fixtures::env(&fixtures::limits()),
        Told::Answered {
            key,
            outcome: finishing_answer(c::OwnerOutcome::AccountSignInStarted {
                source: 43,
                address: Box::from(b"https://issuer.test/start".as_slice()),
            }),
        },
        &mut result,
    );
    assert!(result.is_empty());
    assert!(root.returning.is_none());
    let page = root.pages().connect_model().unwrap();
    assert!(page.elsewhere());
    assert_eq!(page.request.paste_state(root.client()), S::Waiting { ready: false });
    assert_eq!(root.pages().address(), &Address::ConnectModel { way: Some(pages::connect_model::model_way(WAY)) });
}
#[test]
fn signing_in_natively_after_elsewhere_departs_again() {
    let (mut root, _, _) = opened(c::HeldRole::Member);
    root.departing = true;
    flow(&mut root);
    model(&mut root, Act::SignInElsewhere { name: NAME, way: WAY });
    let key = root.pages().connect_model().unwrap().request.pending.as_ref().unwrap().key;
    started(&mut root, key);
    model(&mut root, Act::SignIn { name: [99; 16], way: WAY });
    assert!(!root.pages().connect_model().unwrap().elsewhere());
    let key = root.pages().connect_model().unwrap().request.pending.as_ref().unwrap().key;
    started(&mut root, key);
    assert!(root.returning.is_some());
}
#[test]
fn no_debug_of_the_page_or_the_event_holds_the_address() {
    let (root, _, _) = opened(c::HeldRole::Member);
    let mut request = form(&root);
    let bytes = b"fixture-private-paste-marker";
    request.paste_value(secret(bytes), 256);
    for debug in [
        format!("{request:?}"),
        format!("{:?}", request.paste),
        format!("{:?}", Event::SignInAddress { value: secret(bytes) }),
    ] {
        assert!(!debug.as_bytes().windows(bytes.len()).any(|part| part == bytes));
    }
}
#[test]
fn a_kept_finish_restores_only_the_pastes_request_without_a_name() {
    let (root, _, _) = opened(c::HeldRole::Member);
    let mut request = form(&root);
    let original = request.pending.as_ref().unwrap().key;
    let key = jig_client::Key { asker: original.asker, number: 99 };
    assert!(request.told(
        root.client(),
        &Told::Kept {
            key,
            request: jig_client::Ask::Jig(Box::new(c::Ask::AccountChange {
                holder: c::AccountHolder::Own,
                change: c::AccountChange::FinishSignIn { source: 43, digest: [0; 32] }
            }))
        }
    ));
    assert_eq!(request.pending.as_ref().unwrap().key, original);
    assert_eq!(request.paste.pending.as_ref().unwrap().key, key);
    assert!(request.paste.pending.as_ref().unwrap().name.is_none());
}
