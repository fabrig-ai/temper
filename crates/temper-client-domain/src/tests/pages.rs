use crate::ChatsFilter;
mod accounts;
mod chat;
mod chats;
mod choice;
mod connect_model;
mod delegation;
mod inbox;
mod task;
mod welcome;
use super::fixtures;
use crate::{
    Action, Address,
    pages::{self, Act, Event, Pages, Request},
};
use jig_client::{Link, Told};
use skein_lib::Queue;
const PAGES: jig_client::Asker = jig_client::Asker(2);

#[test]
fn the_pages_watch_me_from_start_under_their_own_asker() {
    let root = crate::unlinked(&fixtures::limits(), fixtures::SEED);
    let mut pages = Pages::new();
    let mut out = Queue::with_capacity(2);
    pages::step(&mut pages, &fixtures::env(&fixtures::limits()), root.client(), Event::Started, &mut out);
    assert_eq!(
        out.pop(),
        Some(Request::Client(jig_client::Event::Watch {
            handle: jig_client::Handle { asker: PAGES, number: 0 },
            subject: jig_client::Watchable::Jig(jig_client::contract::Subject::Me),
        }))
    );
    assert_eq!(
        out.pop(),
        Some(Request::Client(jig_client::Event::Watch {
            handle: pages::accounts::handle(),
            subject: jig_client::Watchable::Jig(jig_client::contract::Subject::Accounts),
        }))
    );
    assert!(out.is_empty());
}
#[test]
fn an_asker_naming_pages_reaches_the_pages_child() {
    let limits = fixtures::limits();
    let mut root = crate::unlinked(&limits, fixtures::SEED);
    let before = pages::shown(root.pages());
    crate::route::told(
        &mut root,
        &fixtures::env(&limits),
        Told::Changed { handle: pages::me_handle() },
        &mut Queue::with_capacity(crate::max_out(&limits)),
    );
    assert_eq!(pages::shown(root.pages()), before + 1);
}
#[test]
fn sign_out_gives_the_client_one_sign_out() {
    let root = crate::unlinked(&fixtures::limits(), fixtures::SEED);
    let mut pages = Pages::new();
    let mut out = Queue::with_capacity(2);
    pages::step(
        &mut pages,
        &fixtures::env(&fixtures::limits()),
        root.client(),
        Event::Act { act: Act::SignOut },
        &mut out,
    );
    assert_eq!(out.pop(), Some(Request::Client(jig_client::Event::SignOut)));
    assert!(out.is_empty());
}
#[test]
fn a_client_started_again_opens_at_the_inbox() {
    let root = crate::unlinked(&fixtures::limits(), fixtures::SEED);
    let mut pages = Pages::new();
    let mut out = Queue::with_capacity(2);
    pages::step(
        &mut pages,
        &fixtures::env(&fixtures::limits()),
        root.client(),
        Event::Went { address: Address::Chats { filter: ChatsFilter::All, start: false } },
        &mut out,
    );
    pages::step(&mut pages, &fixtures::env(&fixtures::limits()), root.client(), Event::Started, &mut out);
    assert_eq!(pages.address(), &Address::Inbox { filter: crate::address::Filter::ALL });
}
#[test]
fn a_link_state_change_prints_the_page_again() {
    let limits = fixtures::limits();
    let mut root = crate::unlinked(&limits, fixtures::SEED);
    let before = crate::shown(&root);
    crate::route::told(
        &mut root,
        &fixtures::env(&limits),
        Told::Link { link: Link::Offline },
        &mut Queue::with_capacity(crate::max_out(&limits)),
    );
    assert!(crate::shown(&root) > before);
}
#[test]
fn the_pages_sign_out_act_reaches_the_shared_client() {
    let limits = fixtures::limits();
    let mut root = crate::unlinked(&limits, fixtures::SEED);
    let mut out = Queue::with_capacity(crate::max_out(&limits));
    crate::step(
        &mut root,
        &fixtures::env(&limits),
        crate::Event::Act { action: Action::Pages(Act::SignOut) },
        &mut out,
    );
    assert_eq!(root.client().link().state, Link::SignedOut);
}

#[test]
fn leaving_a_page_unwatches_its_handles_and_drops_their_tells() {
    const LEFT: jig_client::Handle = jig_client::Handle { asker: PAGES, number: 1 };
    let limits = fixtures::limits();
    let mut root = crate::unlinked(&limits, fixtures::SEED);
    let watch = fixtures::start(&mut root, skein_lib::List::with_capacity(0));
    let mut out = Queue::with_capacity(crate::max_out(&limits));
    fixtures::confirm(&mut root, watch, fixtures::PARTY, &mut out);
    crate::step(
        &mut root,
        &fixtures::env(&limits),
        crate::Event::Went { place: crate::Place::Address(Address::Inbox { filter: crate::address::Filter::ALL }) },
        &mut out,
    );
    fixtures::opened(&mut root, &mut out);
    crate::step(
        &mut root,
        &fixtures::env(&limits),
        crate::Event::Went { place: crate::Place::Address(Address::Adopt { project: fixtures::PROJECT }) },
        &mut out,
    );
    let mut closes = 0_u32;
    while let Some(request) = out.pop() {
        if let crate::Request::Client { request: jig_client::Request::Down { down: jig_client::Down::Close { .. } } } =
            request
        {
            closes += 1;
        }
    }
    assert!(closes > 0, "the inbox's watch is closed on leaving");
    let before = pages::shown(root.pages());
    crate::route::told(&mut root, &fixtures::env(&limits), Told::Changed { handle: LEFT }, &mut out);
    assert_eq!(pages::shown(root.pages()), before);
}

#[test]
fn arriving_at_chats_or_a_task_sets_its_address_filter() {
    use crate::PlanFilter;
    let limits = fixtures::limits();
    let root = crate::unlinked(&limits, fixtures::SEED);
    for address in [
        Address::Chats { filter: ChatsFilter::Live, start: true },
        Address::Chats { filter: ChatsFilter::Closed, start: false },
        Address::Task { task: 7, filter: PlanFilter::Live },
    ] {
        let mut pages = Pages::new();
        pages::step(
            &mut pages,
            &fixtures::env(&limits),
            root.client(),
            Event::Went { address: address.clone() },
            &mut Queue::with_capacity(crate::max_out(&limits)),
        );
        assert_eq!(pages.address(), &address);
        if let Address::Chats { filter, .. } = address {
            assert_eq!(pages.chats().unwrap().filter, filter);
        }
        if let Address::Task { filter, .. } = address {
            assert_eq!(pages.task().unwrap().filter, filter);
        }
    }
}

#[test]
fn an_ended_task_opened_before_me_retries_a_busy_document_read() {
    use jig_client::{Down, Readable, Up, Watchable, contract as c};
    use skein_lib::{Duration, List, Time};
    const TASK: u64 = 2;
    let limits = fixtures::limits();
    let mut root = crate::unlinked(&limits, fixtures::SEED);
    let me = fixtures::start(&mut root, List::with_capacity(0));
    let address = Address::Task { task: TASK, filter: crate::PlanFilter::All };
    let mut out = Queue::with_capacity(crate::max_out(&limits));
    crate::step(
        &mut root,
        &fixtures::env(&limits),
        crate::Event::Went { place: crate::Place::Address(Address::Inbox { filter: crate::address::Filter::ALL }) },
        &mut out,
    );
    while out.pop().is_some() {}
    assert_eq!(root.client().link().state, Link::Starting);
    assert!(root.pages().me(root.client()).is_none(), "navigation precedes the first signed-in documents");
    crate::step(
        &mut root,
        &fixtures::env(&limits),
        crate::Event::Went { place: crate::Place::Address(address.clone()) },
        &mut out,
    );
    let mut task_watch = None;
    while let Some(request) = out.pop() {
        if let crate::Request::Client { request: jig_client::Request::Down { down: Down::Open { watch, subject } } } =
            request
            && subject == Watchable::Jig(c::Subject::Task { task: TASK })
        {
            task_watch = Some(watch);
        }
    }
    crate::step(
        &mut root,
        &fixtures::env(&limits),
        crate::Event::Client {
            event: jig_client::Event::Up {
                up: Up::WatchEnded { watch: task_watch.expect("task watch"), end: c::WatchEnd::SubjectEnded },
            },
        },
        &mut out,
    );
    let mut document_read = None;
    while let Some(request) = out.pop() {
        if let crate::Request::Client { request: jig_client::Request::Down { down: Down::Read { read, what } } } =
            request
            && what == Readable::Jig(c::Read::Task { task: TASK })
        {
            document_read = Some(read);
        }
    }
    crate::step(
        &mut root,
        &fixtures::env(&limits),
        crate::Event::Client {
            event: jig_client::Event::Up {
                up: Up::ReadDone {
                    read: document_read.expect("ended task read"),
                    result: jig_client::Readout::Jig(c::ReadResult::None(c::ReadNone::Busy)),
                },
            },
        },
        &mut out,
    );
    fixtures::confirm(&mut root, me, fixtures::PARTY, &mut out);
    let mut env = fixtures::env(&limits);
    env.now = Time::ZERO.saturating_add(Duration::from_secs(10));
    crate::fire(&mut root, &env, &mut out);
    assert_eq!(root.pages().address(), &address, "the person's navigation stays at the task");
    let mut retried = false;
    while let Some(request) = out.pop() {
        if let crate::Request::Client { request: jig_client::Request::Down { down: Down::Read { what, .. } } } = request
            && what == Readable::Jig(c::Read::Task { task: TASK })
        {
            retried = true;
        }
    }
    assert!(retried, "an ended task's busy document read must not leave its page empty forever");
}
