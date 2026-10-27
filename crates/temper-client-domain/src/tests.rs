mod acts;
mod address;
mod admin;
mod amend;
mod demo;
pub(crate) mod fixtures;
mod forge_words;
mod forges;
mod form;
mod pages;
mod panel;
use crate::{Address, Event, Fact, Request, forge};
use jig_client::contract;
use skein_lib::{List, Queue};

#[test]
fn the_forge_address_goes_to_the_forge_side() {
    let limits = fixtures::limits();
    let mut root = crate::unlinked(&limits, fixtures::SEED);
    fixtures::start(&mut root, List::with_capacity(0));
    let address = forge::Address::Adopt { project: fixtures::PROJECT };
    let mut out = Queue::with_capacity(crate::max_out(&limits));
    crate::step(
        &mut root,
        &fixtures::env(&limits),
        Event::Went {
            place: crate::Place::Address(Address::Adopt {
                project: match address {
                    forge::Address::Adopt { project } => project,
                },
            }),
        },
        &mut out,
    );
    assert_eq!(root.forge().address(), Some(address));
    crate::step(&mut root, &fixtures::env(&limits), Event::Went { place: crate::Place::Missing }, &mut out);
    assert_eq!(root.forge().address(), None, "leaving drops the forge page");
}
#[test]
fn a_family_hello_outside_the_version_makes_the_side_out_of_date() {
    let limits = fixtures::limits();
    let mut root = crate::unlinked(&limits, fixtures::SEED);
    let mut out = Queue::with_capacity(crate::max_out(&limits));
    for (name, versions, expected) in [
        (
            "served",
            contract::VersionRange {
                first: temper_client_protocol_wire::CLIENT_VERSION,
                last: temper_client_protocol_wire::CLIENT_VERSION,
            },
            forge::Served::Yes,
        ),
        (
            "newer",
            contract::VersionRange {
                first: temper_client_protocol_wire::CLIENT_VERSION + 1,
                last: temper_client_protocol_wire::CLIENT_VERSION + 1,
            },
            forge::Served::OutOfDate,
        ),
        (
            "inverted",
            contract::VersionRange {
                first: temper_client_protocol_wire::CLIENT_VERSION + 1,
                last: temper_client_protocol_wire::CLIENT_VERSION,
            },
            forge::Served::OutOfDate,
        ),
    ] {
        crate::step(
            &mut root,
            &fixtures::env(&limits),
            Event::Client {
                event: jig_client::Event::Up {
                    up: jig_client::Up::FamilyServed { family: fixtures::FORGE, versions: Some(versions) },
                },
            },
            &mut out,
        );
        assert_eq!(root.forge().served(), expected, "{name}");
    }
}

#[test]
fn an_asker_naming_no_child_is_a_fact_and_reaches_none() {
    const UNKNOWN: jig_client::Asker = jig_client::Asker(65_535);
    const REQUEST: u64 = 17;
    let limits = fixtures::limits();
    let mut root = fixtures::filled();
    let mut out = Queue::with_capacity(crate::max_out(&limits));
    crate::route::told(
        &mut root,
        &fixtures::env(&limits),
        jig_client::Told::Answered {
            key: jig_client::Key { asker: UNKNOWN, number: REQUEST },
            outcome: jig_client::Outcome::Unknown,
        },
        &mut out,
    );
    assert_eq!(out.pop(), Some(Request::Fact { fact: Fact::UnknownAsker }));
    assert_eq!(root.forge().form().expect("form").outcome, None);
    assert!(out.is_empty());
}

#[test]
fn the_forges_side_learns_its_party_from_its_own_me_handle() {
    let limits = fixtures::limits();
    let mut root = crate::unlinked(&limits, fixtures::SEED);
    let watch = fixtures::start(&mut root, List::with_capacity(0));
    crate::route::forge(
        &mut root,
        &fixtures::env(&limits),
        forge::Event::Went { address: forge::Address::Adopt { project: fixtures::PROJECT } },
        &mut Queue::with_capacity(crate::max_out(&limits)),
    );
    assert!(!root.forge().form().expect("adoption page").confirmed);
    fixtures::confirm(&mut root, watch, fixtures::PARTY, &mut Queue::with_capacity(crate::max_out(&limits)));
    assert!(root.client().document(forge::me::handle()).is_some());
    assert!(root.forge().form().expect("adoption page").confirmed);
    crate::route::told(
        &mut root,
        &fixtures::env(&limits),
        jig_client::Told::Ended { handle: forge::me::handle(), why: jig_client::Why::SignedOut },
        &mut Queue::with_capacity(crate::max_out(&limits)),
    );
    assert!(!root.forge().form().expect("adoption page").confirmed);
}

#[test]
fn dropped_work_goes_to_the_shell_with_its_count() {
    let limits = fixtures::limits();
    let mut root = crate::unlinked(&limits, fixtures::SEED);
    let mut out = Queue::with_capacity(crate::max_out(&limits));
    crate::route::told(&mut root, &fixtures::env(&limits), jig_client::Told::Dropped { count: 2 }, &mut out);
    assert_eq!(out.pop(), Some(Request::Dropped { count: 2 }));
    assert!(out.is_empty());
}

mod change;

mod connect;

mod github_app;

#[test]
fn budget_hold_words_name_the_task_and_explain_the_available_actions() {
    let book = crate::words::phrasebook(128);
    let reason = contract::HoldCategory::Budget;
    let number = crate::words::hold_number(73, &reason);
    let text = book.say(0, jig_client::words::Word::hold(&reason).id(), number.as_slice()).unwrap().text;
    assert_eq!(text.as_ref(), b"Insufficient activation budget. A release alone holds it again: give the task more budget (amend 73 with a larger budget, or decide its budget proposal), then release it; or cancel it.");
}
