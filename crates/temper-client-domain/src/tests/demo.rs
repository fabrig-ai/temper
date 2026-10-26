use super::fixtures as f;
use crate::{
    forge::{self, demo},
    pages::Act,
};
use jig_client::{Handle, Link, Readable, Readout, Told, contract as c};
use skein_lib::{List, Queue};
use temper_client_protocol_wire::{self as wire, v1 as w};
const PROVIDER: u32 = 17;
const CUE: &[u8] = b"Change the title";
const DRAFT: &[u8] = b"My draft";
pub(super) fn answer(cue: &[u8]) -> Box<[u8]> {
    let mut cues = List::with_capacity(1);
    cues.push(cue.into()).unwrap();
    wire::demo_answer(
        &w::DemoAnswer::new(
            &w::CEILINGS,
            w::DemoAnswerParts {
                state: w::DemoState::Running,
                project: f::PROJECT,
                forge: wire::FORGE,
                provider: PROVIDER,
                owner: b"acme".as_slice().into(),
                name: b"shop".as_slice().into(),
                cues,
            },
        )
        .unwrap(),
    )
}
fn page() -> Readout {
    Readout::Family { family: wire::FORGE, record: Some(answer(CUE)) }
}
pub(super) fn page_with_state(state: w::DemoState) -> Readout {
    let mut parts = wire::decode_demo_answer(&answer(CUE), &w::CEILINGS).unwrap().into_parts();
    parts.state = state;
    Readout::Family {
        family: wire::FORGE,
        record: Some(wire::demo_answer(&w::DemoAnswer::new(&w::CEILINGS, parts).unwrap())),
    }
}
fn read(side: &mut forge::Side) -> Handle {
    let mut out = Queue::with_capacity(1);
    assert!(!demo::told(side, &Told::Link { link: Link::Live }, f::limits().forge, &mut out));
    let Some(forge::Request::Client { asked: jig_client::Event::Read { handle, read } }) = out.pop() else {
        panic!("demo read");
    };
    let Readable::Family { scope, window, record } = read else { panic!("family read") };
    assert_eq!(scope, c::Scope::Deployment);
    assert_eq!(window, c::Window { rows: 0, after: None });
    assert_eq!((record.family, record.version), (wire::FORGE, wire::CLIENT_VERSION));
    assert_eq!(wire::decode_reading(&record.bytes, &w::CEILINGS).unwrap().read(), &w::Read::Demo);
    assert_ne!(handle.number, 0, "zero belongs to me");
    assert!(out.is_empty());
    handle
}
#[test]
fn the_demo_is_read_once_per_live_link_and_replaced_pages_are_dropped() {
    for (row, gap, reads) in [
        ("live again", Link::Live, false),
        ("offline then live", Link::Offline, true),
        ("signed out then live", Link::SignedOut, true),
    ] {
        let mut side = forge::Side::new(&f::limits().forge, f::SEED);
        let first = read(&mut side);
        let mut out = Queue::with_capacity(1);
        assert!(!demo::told(&mut side, &Told::Link { link: gap }, f::limits().forge, &mut out), "{row}");
        assert!(out.is_empty(), "{row}");
        if reads {
            let next = read(&mut side);
            assert_ne!(first, next, "{row}");
            assert!(demo::told(&mut side, &Told::Page { handle: first, page: page() }, f::limits().forge, &mut out));
            assert!(side.demo().is_none(), "{row}: replaced page dropped");
            demo::told(&mut side, &Told::Page { handle: next, page: page() }, f::limits().forge, &mut out);
            assert!(side.demo().is_some(), "{row}");
        } else {
            assert!(!demo::told(&mut side, &Told::Link { link: Link::Live }, f::limits().forge, &mut out));
            assert!(out.is_empty(), "{row}");
        }
    }
}
#[test]
fn a_demo_page_replaces_the_last_answer_or_clears_it() {
    let mut another_version = answer(CUE);
    *another_version.get_mut(1).unwrap() = u8::try_from(wire::CLIENT_VERSION + 1).unwrap();
    for (row, page, present) in [
        ("demo", page(), true),
        ("none", Readout::Family { family: wire::FORGE, record: None }, false),
        ("not served", Readout::NotServed { family: Some(wire::FORGE), served: None }, false),
        ("limit", Readout::Limit, false),
        ("undecodable", Readout::Family { family: wire::FORGE, record: Some(Box::new([255])) }, false),
        ("another version", Readout::Family { family: wire::FORGE, record: Some(another_version) }, false),
        ("empty cue", Readout::Family { family: wire::FORGE, record: Some(answer(b"")) }, false),
        ("wrong family", Readout::Family { family: 99, record: Some(answer(CUE)) }, false),
        ("jig page", Readout::Jig(c::ReadResult::None(c::ReadNone::Unknown)), false),
    ] {
        let mut side = forge::Side::new(&f::limits().forge, f::SEED);
        let first = read(&mut side);
        assert!(!side.demo_answered(), "{row}: no answer yet");
        demo::told(
            &mut side,
            &Told::Page { handle: first, page: self::page() },
            f::limits().forge,
            &mut Queue::with_capacity(1),
        );
        let shown = forge::shown(&side);
        demo::told(&mut side, &Told::Link { link: Link::Offline }, f::limits().forge, &mut Queue::with_capacity(1));
        assert!(side.demo().is_some(), "{row}: keep the last answer across the gap");
        assert!(side.demo_answered(), "{row}: keep that the read answered across the gap");
        let handle = read(&mut side);
        assert!(
            demo::told(&mut side, &Told::Page { handle, page }, f::limits().forge, &mut Queue::with_capacity(1)),
            "{row}"
        );
        assert_eq!(side.demo().is_some(), present, "{row}");
        assert_eq!(forge::shown(&side) != shown, !present, "{row}: redraw only on change");
    }
}
pub(super) fn accounts(provider: Option<u32>, usable: bool, cut: bool) -> c::OwnerAccounts {
    let mut sources = List::with_capacity(1);
    sources
        .push(c::OwnerAccountSource {
            name: None,
            number: 1,
            place: 1,
            kind: c::OwnerAccountKind::Key,
            state: c::OwnerAccountState::Granted,
            usable,
            expiring: false,
            attention: false,
            spent_until: None,
        })
        .unwrap();
    let mut accounts = List::with_capacity(1);
    accounts
        .push(c::OwnerAccountLine {
            number: 1,
            place: 1,
            uses: None,
            plan: false,
            provider,
            provider_name: None,
            manage_usage: None,
            display: c::DocumentText::complete(f::acts::text(b"Model")),
            holder: c::OwnerAccountHolder::Party(f::PARTY),
            sources,
            cut,
        })
        .unwrap();
    c::OwnerAccounts {
        program_hosts: List::with_capacity(0),
        readiness: List::with_capacity(0),
        readiness_cut: false,
        accounts,
        signing: List::with_capacity(0),
        last_end: None,
        usable,
        settled: true,
        cut: false,
    }
}
#[test]
fn the_model_half_waits_for_accounts_and_yields_to_another_provider() {
    let answer = forge::family::demo(&answer(CUE), &w::CEILINGS).unwrap();
    let mut cut = accounts(Some(PROVIDER), true, false);
    cut.cut = true;
    for (row, answer, document, model) in [
        ("no answer", None, Some(accounts(Some(PROVIDER), true, false)), false),
        ("no document", Some(&answer), None, false),
        ("cut document", Some(&answer), Some(cut), false),
        ("demo provider usable", Some(&answer), Some(accounts(Some(PROVIDER), true, false)), true),
        ("other unusable", Some(&answer), Some(accounts(Some(PROVIDER + 1), false, false)), true),
        ("other usable", Some(&answer), Some(accounts(Some(PROVIDER + 1), true, false)), false),
        ("unknown provider usable", Some(&answer), Some(accounts(None, true, false)), false),
        ("other cut", Some(&answer), Some(accounts(Some(PROVIDER + 1), false, true)), false),
    ] {
        let forge = answer.is_some();
        assert_eq!(
            demo::halves(answer, document.as_ref(), None, &forge::forges::State::Rows(Box::new([]))),
            demo::Halves { model, forge },
            "{row}"
        );
    }
}
fn event(root: &mut crate::Domain, event: crate::Event) -> Queue<crate::Request> {
    let mut out = Queue::with_capacity(crate::max_out(&f::limits()));
    crate::step(root, &f::env(&f::limits()), event, &mut out);
    out
}
fn act(root: &mut crate::Domain, act: Act) -> Queue<crate::Request> {
    event(root, crate::Event::Act { action: crate::Action::Pages(act) })
}
fn opened(cue: &[u8]) -> (crate::Domain, skein_lib::Token) {
    let mut root = crate::unlinked(&f::limits(), f::SEED);
    let (me, account) = f::start_accounts(&mut root, List::with_capacity(0));
    for (watch, delivery) in [
        (me, c::Delivery::Me(Box::new(f::chats::me()))),
        (account, c::Delivery::OwnerAccounts(Box::new(accounts(Some(PROVIDER), true, false)))),
    ] {
        event(
            &mut root,
            crate::Event::Client {
                event: jig_client::Event::Up {
                    up: jig_client::Up::Watched { watch, event: c::WatchEvent::Delivered(delivery) },
                },
            },
        );
    }
    deliver_demo_read(&mut root, Readout::Family { family: wire::FORGE, record: Some(answer(cue)) });
    event(
        &mut root,
        crate::Event::Went {
            place: crate::Place::Address(crate::Address::Chats { filter: crate::ChatsFilter::All, start: false }),
        },
    );
    act(&mut root, Act::ChatWords { words: DRAFT.into() });
    (root, account)
}
#[test]
fn a_suggestion_starts_one_chat_with_its_cue_and_preserves_the_draft() {
    let mut at_bound = skein_lib::bytes::zeroed(usize::try_from(wire::CUE_BYTES).unwrap());
    at_bound.fill(b'a');
    let above_receiving_fixture = [b'a'; 33];
    for (row, words, pending, project, model, cue, count) in [
        ("demo project", CUE, false, f::PROJECT, true, 0, 1),
        ("at the composer's bound", &at_bound, false, f::PROJECT, true, 0, 1),
        ("above the receiving fixture", &above_receiving_fixture, false, f::PROJECT, true, 0, 1),
        ("pending", CUE, true, f::PROJECT, true, 0, 0),
        ("other project", CUE, false, f::PROJECT + 1, true, 0, 0),
        ("no model half", CUE, false, f::PROJECT, false, 0, 0),
        ("past cues", CUE, false, f::PROJECT, true, 1, 0),
    ] {
        let (mut root, account) = opened(words);
        if pending {
            act(&mut root, Act::StartChat);
        }
        if project != f::PROJECT {
            let limits = forge::wire_limits(&f::limits().forge).unwrap();
            let mut parts = wire::decode_demo_answer(&answer(CUE), &limits).unwrap().into_parts();
            parts.project = project;
            let record = wire::demo_answer(&w::DemoAnswer::new(&limits, parts).unwrap());
            deliver_demo_read(&mut root, Readout::Family { family: wire::FORGE, record: Some(record) });
        }
        if !model {
            event(
                &mut root,
                crate::Event::Client {
                    event: jig_client::Event::Up {
                        up: jig_client::Up::Watched {
                            watch: account,
                            event: c::WatchEvent::Delivered(c::Delivery::OwnerAccounts(Box::new(accounts(
                                Some(PROVIDER + 1),
                                true,
                                false,
                            )))),
                        },
                    },
                },
            );
        }
        let mut out = act(&mut root, Act::Suggest { cue });
        let mut sent = List::with_capacity(1);
        while let Some(request) = out.pop() {
            if let crate::Request::Client {
                request:
                    jig_client::Request::Down { down: jig_client::Down::Send { request: jig_client::Ask::Jig(ask), .. } },
            } = request
            {
                sent.push(*ask).expect("one chat start");
            }
        }
        assert_eq!(usize::try_from(sent.len()).unwrap(), count, "{row}");
        if count == 1 {
            assert_eq!(
                sent.as_slice(),
                [jig_client::shapes::chat(
                    &f::chats::me(),
                    f::PROJECT,
                    c::Text::new(words.into(), wire::CUE_BYTES).unwrap()
                )
                .unwrap()],
                "{row}"
            );
        }
        assert_eq!(root.pages().chats().unwrap().words.as_ref(), DRAFT, "{row}");
    }
}

#[test]
fn the_first_demo_read_answer_counts_even_when_it_has_no_record() {
    for page in [
        Readout::Family { family: wire::FORGE, record: None },
        Readout::NotServed { family: Some(wire::FORGE), served: None },
        Readout::Limit,
        Readout::Jig(c::ReadResult::None(c::ReadNone::Unknown)),
    ] {
        let mut side = forge::Side::new(&f::limits().forge, f::SEED);
        let handle = read(&mut side);
        let before = forge::shown(&side);
        demo::told(&mut side, &Told::Page { handle, page }, f::limits().forge, &mut Queue::with_capacity(1));
        assert!(side.demo_answered());
        assert!(side.demo().is_none());
        assert_eq!(side.demo_provider(), None);
        assert_eq!(side.retired_demo(), None);
        assert!(forge::shown(&side) > before, "an answered absence redraws the welcome");
        demo::told(&mut side, &Told::Link { link: Link::Offline }, f::limits().forge, &mut Queue::with_capacity(1));
        assert!(side.demo_answered());
    }
}

fn demo_side(state: w::DemoState) -> forge::Side {
    let mut side = forge::Side::new(&f::limits().forge, f::SEED);
    let mut parts = wire::decode_demo_answer(&answer(CUE), &w::CEILINGS).unwrap().into_parts();
    parts.state = state;
    let handle = read(&mut side);
    demo::told(
        &mut side,
        &Told::Page {
            handle,
            page: Readout::Family {
                family: wire::FORGE,
                record: Some(wire::demo_answer(&w::DemoAnswer::new(&w::CEILINGS, parts).unwrap())),
            },
        },
        f::limits().forge,
        &mut Queue::with_capacity(4),
    );
    let mut root = crate::unlinked(&f::limits(), f::SEED);
    let (me, _) = f::start_accounts(&mut root, List::with_capacity(0));
    event(
        &mut root,
        crate::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched {
                    watch: me,
                    event: c::WatchEvent::Delivered(c::Delivery::Me(Box::new(f::chats::me()))),
                },
            },
        },
    );
    forge::me::told(
        &mut side,
        root.client(),
        &Told::Changed { handle: forge::me::handle() },
        &mut Queue::with_capacity(4),
    );
    side
}
fn started(side: &mut forge::Side) -> skein_lib::Token {
    let mut out = Queue::with_capacity(4);
    demo::start(side, f::limits().forge, &mut out);
    let Some(forge::Request::Send { request, ask: jig_client::Ask::Family { scope, record }, .. }) = out.pop() else {
        panic!("start sent")
    };
    assert_eq!(scope, c::Scope::Deployment);
    assert_eq!(wire::decode_requesting(&record.bytes, &w::CEILINGS).unwrap().request(), &w::Request::StartDemo);
    assert!(out.is_empty());
    request
}
fn answered(side: &mut forge::Side, request: skein_lib::Token, outcome: w::Outcome) {
    let value = w::Answer::new(&w::CEILINGS, w::AnswerParts { outcome }).unwrap();
    let mut out = Queue::with_capacity(4);
    demo::told(
        side,
        &Told::Answered {
            key: jig_client::Key { asker: crate::owner::Owner::Forge.asker(), number: request.raw() },
            outcome: jig_client::Outcome::Answered(jig_client::Answered::Family(jig_client::Family {
                family: wire::FORGE,
                version: wire::CLIENT_VERSION,
                bytes: wire::answer(&value),
            })),
        },
        f::limits().forge,
        &mut out,
    );
    let Some(forge::Request::Client { asked: jig_client::Event::Read { .. } }) = out.pop() else {
        panic!("start answer rereads the demo")
    };
    assert!(out.is_empty());
}
#[test]
fn demo_state_decodes_and_start_is_single_across_pages_with_a_new_retry_key() {
    let mut side = demo_side(w::DemoState::NotStarted);
    assert_eq!(side.demo().unwrap().state, demo::DemoState::NotStarted);
    assert!(!side.demo_running());
    let first = started(&mut side);
    assert!(side.demo_running());
    assert!(side.demo_starting());
    let mut out = Queue::with_capacity(4);
    forge::step(
        &mut side,
        &skein_lib::Env { now: skein_lib::Time::ZERO, wall: skein_lib::Wall::EPOCH, limits: f::limits().forge },
        forge::Event::Left,
        &mut out,
    );
    demo::start(&mut side, f::limits().forge, &mut out);
    assert!(out.is_empty());
    answered(
        &mut side,
        first,
        w::Outcome::StartFailed(
            w::StartFailed::new(&w::CEILINGS, w::StartFailedParts { why: w::StartFailure::Account }).unwrap(),
        ),
    );
    assert_eq!(side.demo_failure(), Some(&demo::StartFailure::Account));
    assert!(side.demo_running());
    assert!(!side.demo_starting());
    let retry = started(&mut side);
    assert_ne!(first, retry);
    assert!(side.demo_failure().is_none());
}
#[test]
fn a_running_read_sends_no_start_and_a_persons_start_shows_its_failure() {
    let mut side = demo_side(w::DemoState::Running);
    let mut out = Queue::with_capacity(4);
    demo::told(&mut side, &Told::Link { link: Link::Offline }, f::limits().forge, &mut out);
    let handle = read(&mut side);
    demo::told(&mut side, &Told::Page { handle, page: page() }, f::limits().forge, &mut out);
    assert!(out.is_empty(), "running read sends no start");
    demo::start(&mut side, f::limits().forge, &mut out);
    assert!(out.is_empty(), "a person's start while running sends nothing");
    assert!(!side.demo_starting());

    let mut side = demo_side(w::DemoState::NotStarted);
    let request = started(&mut side);
    assert!(side.demo_starting());
    answered(
        &mut side,
        request,
        w::Outcome::StartFailed(
            w::StartFailed::new(&w::CEILINGS, w::StartFailedParts { why: w::StartFailure::NotConnected }).unwrap(),
        ),
    );
    assert!(!side.demo_starting());
    assert_eq!(side.demo_failure(), Some(&demo::StartFailure::NotConnected));
}
#[test]
fn running_unknown_and_not_offered_each_read_the_demo_again() {
    for outcome in [
        w::Outcome::Running,
        w::Outcome::Unknown,
        w::Outcome::StartRefused(
            w::StartRefused::new(&w::CEILINGS, w::StartRefusedParts { why: w::StartRefusal::NotOffered }).unwrap(),
        ),
    ] {
        let mut side = demo_side(w::DemoState::NotStarted);
        let request = started(&mut side);
        answered(&mut side, request, outcome);
        assert!(!side.demo_starting());
        assert!(side.demo_failure().is_none());
    }
}
#[test]
fn another_operations_outcome_is_not_a_start_answer() {
    let value = jig_client::Answered::Family(jig_client::Family {
        family: wire::FORGE,
        version: wire::CLIENT_VERSION,
        bytes: wire::answer(&w::Answer::new(&w::CEILINGS, w::AnswerParts { outcome: w::Outcome::SettingUp }).unwrap()),
    });
    assert!(demo::answer(&value, &f::limits().forge).is_none());
}

fn up(root: &mut crate::Domain, up: jig_client::Up) -> Queue<crate::Request> {
    event(root, crate::Event::Client { event: jig_client::Event::Up { up } })
}
fn deliver_accounts(
    root: &mut crate::Domain,
    watch: skein_lib::Token,
    document: c::OwnerAccounts,
) -> Queue<crate::Request> {
    up(
        root,
        jig_client::Up::Watched {
            watch,
            event: c::WatchEvent::Delivered(c::Delivery::OwnerAccounts(Box::new(document))),
        },
    )
}
fn ending_accounts(other_usable: bool) -> c::OwnerAccounts {
    accounts(Some(PROVIDER + 1), other_usable, false)
}
pub(super) type DeploymentAccount = (Option<u32>, bool, bool, c::OwnerAccountHolder);
pub(super) const DEMO_ACCOUNT: DeploymentAccount = (Some(PROVIDER), true, false, c::OwnerAccountHolder::Deployment);
pub(super) const OTHER_ACCOUNT: DeploymentAccount =
    (Some(PROVIDER + 1), true, false, c::OwnerAccountHolder::Deployment);
pub(super) fn deployment(rows: &[DeploymentAccount]) -> c::OwnerDeployment {
    let bounds = jig_client_protocol_link::Limits::new(f::limits().client.domain, 8).unwrap();
    let mut document = jig_client_protocol_link::translate::documents::owner_deployment(
        jig_client_test_records::owner_deployment_record(),
        &bounds,
    )
    .unwrap();
    document.accounts = List::with_capacity(u32::try_from(rows.len()).unwrap());
    for (provider, usable, cut, holder) in rows {
        let personal = accounts(*provider, *usable, *cut);
        let line = personal.accounts.get(0).unwrap();
        document
            .accounts
            .push(c::OwnerAccount {
                number: if *provider == Some(PROVIDER) { 42 } else { 43 },
                place: 0,
                provider: *provider,
                provider_name: None,
                manage_usage: None,
                display: line.display.clone(),
                holder: *holder,
                uses: None,
                plan: false,
                sources: line.sources.clone(),
                generation: None,
                state: c::OwnerAccountState::Granted,
                usable: *usable,
                expiring: false,
                attention: false,
                spent_until: None,
                runs: List::with_capacity(0),
                waiting: List::with_capacity(0),
                cut: *cut,
            })
            .unwrap();
    }
    document
}
pub(super) fn deliver_deployment(
    root: &mut crate::Domain,
    watch: skein_lib::Token,
    document: c::OwnerDeployment,
) -> Queue<crate::Request> {
    up(
        root,
        jig_client::Up::Watched {
            watch,
            event: c::WatchEvent::Delivered(c::Delivery::OwnerDeployment(Box::new(document))),
        },
    )
}
pub(super) fn deployment_watch(out: &mut Queue<crate::Request>) -> Option<skein_lib::Token> {
    let mut watch = None;
    let mut rest = Queue::with_capacity(out.len());
    while let Some(request) = out.pop() {
        if let crate::Request::Client {
            request:
                jig_client::Request::Down {
                    down:
                        jig_client::Down::Open { watch: token, subject: jig_client::Watchable::Jig(c::Subject::Deployment) },
                },
        } = request
        {
            assert!(watch.replace(token).is_none(), "one deployment watch");
        } else {
            rest.push(request);
        }
    }
    *out = rest;
    watch
}
fn closed(out: &Queue<crate::Request>, watch: skein_lib::Token) -> bool {
    for request in out {
        if let crate::Request::Client {
            request: jig_client::Request::Down { down: jig_client::Down::Close { watch: token } },
        } = request
            && *token == watch
        {
            return true;
        }
    }
    false
}
fn ending_root(
    role: Option<c::HeldRole>,
) -> (crate::Domain, skein_lib::Token, skein_lib::Token, Option<skein_lib::Token>) {
    ending_root_with_page(role, page())
}
fn ending_root_with_page(
    role: Option<c::HeldRole>,
    page: Readout,
) -> (crate::Domain, skein_lib::Token, skein_lib::Token, Option<skein_lib::Token>) {
    let mut root = crate::unlinked(&f::limits(), f::SEED);
    let (me, account) = f::start_accounts(&mut root, List::with_capacity(0));
    let mut frame = f::chats::me();
    frame.deployment_role = role;
    let mut out = up(
        &mut root,
        jig_client::Up::Watched { watch: me, event: c::WatchEvent::Delivered(c::Delivery::Me(Box::new(frame))) },
    );
    let mut forges = None;
    while let Some(request) = out.pop() {
        if let crate::Request::Client {
            request:
                jig_client::Request::Down {
                    down: jig_client::Down::Open { watch, subject: jig_client::Watchable::Family { .. } },
                },
        } = request
        {
            forges = Some(watch);
        }
    }
    let mut out = deliver_demo_read(&mut root, page);
    let watch = deployment_watch(&mut out);
    (root, account, forges.expect("deployment forges"), watch)
}
pub(super) fn deliver_demo_read(root: &mut crate::Domain, page: Readout) -> Queue<crate::Request> {
    demo::told(&mut root.forge, &Told::Link { link: Link::Offline }, f::limits().forge, &mut Queue::with_capacity(1));
    let handle = read(&mut root.forge);
    let mut out = Queue::with_capacity(crate::max_out(&f::limits()));
    crate::route::told(root, &f::env(&f::limits()), Told::Page { handle, page }, &mut out);
    crate::route::demo_progress(root, &f::env(&f::limits()), &mut out);
    out
}
fn forge_rows(root: &mut crate::Domain, watch: skein_lib::Token, connected: bool) -> Queue<crate::Request> {
    up(root, jig_client::Up::FamilyDelivered { watch, record: forge_document(connected) })
}
fn forge_document(connected: bool) -> jig_client::Family {
    let mut forges = List::with_capacity(1);
    let connection = if connected {
        w::Connection::Connected(
            w::Connected::new(&w::CEILINGS, w::ConnectedParts { id: 1, login: b"bot".as_slice().into() }).unwrap(),
        )
    } else {
        w::Connection::Connecting
    };
    forges
        .push(
            w::ForgeRow::new(
                &w::CEILINGS,
                w::ForgeRowParts {
                    number: wire::FORGE + 1,
                    kind: w::Kind::Forgejo,
                    mode: w::Mode::Owned,
                    connection,
                    offer: w::Offer::NotOffered,
                    first: None,
                    installations: List::with_capacity(0),
                    ended: None,
                    app: None,
                    web: None,
                    repositories: List::with_capacity(0),
                    owned: None,
                    admin: false,
                    read_only: false,
                    ended_stop: None,
                },
            )
            .unwrap(),
        )
        .unwrap();
    jig_client::Family {
        family: wire::FORGE,
        version: wire::CLIENT_VERSION,
        bytes: wire::forges(&w::Forges::new(&w::CEILINGS, w::ForgesParts { forges }).unwrap()),
    }
}
pub(super) fn removed(out: &mut Queue<crate::Request>) -> List<skein_lib::Token> {
    let mut attempts = List::with_capacity(4);
    while let Some(request) = out.pop() {
        if let crate::Request::Client {
            request:
                jig_client::Request::Down {
                    down: jig_client::Down::Send { attempt, request: jig_client::Ask::Jig(ask), .. },
                },
        } = request
        {
            assert_eq!(
                *ask,
                c::Ask::AccountChange {
                    holder: c::AccountHolder::Deployment,
                    change: c::AccountChange::Remove { account: 42 }
                }
            );
            attempts.push(attempt).unwrap();
        }
    }
    attempts
}
#[test]
fn the_model_end_removes_the_deployment_demo_account_once_and_drops_its_answer() {
    for answer in [
        None,
        Some(jig_client::Answered::Jig(c::Answer::Done(c::Outcome::Owner(c::OwnerOutcome::AccountRemoved {
            confirmed: true,
            failed_hosts: Box::new([]),
        })))),
        Some(jig_client::Answered::Jig(c::Answer::Refused {
            refusal: c::Refusal::Role { held: Some(c::HeldRole::Member) },
            at: None,
        })),
    ] {
        let (mut root, account, forges, watch) = ending_root(Some(c::HeldRole::Owner));
        let watch = watch.expect("running demo opens owner deployment watch");
        assert!(deployment_watch(&mut deliver_accounts(&mut root, account, ending_accounts(false))).is_none());
        forge_rows(&mut root, forges, false);
        assert!(root.demo().unwrap().1.model, "accounts alone hold before deployment delivery");
        let mut out = deliver_accounts(&mut root, account, ending_accounts(true));
        assert!(deployment_watch(&mut out).is_none(), "the first end keeps the existing watch");
        assert!(removed(&mut out).is_empty(), "wait for deployment delivery");
        assert!(deliver_accounts(&mut root, account, ending_accounts(true)).is_empty(), "watch opened once");
        let mut out = deliver_deployment(&mut root, watch, deployment(&[DEMO_ACCOUNT]));
        assert!(!closed(&out, watch), "running demo keeps the deployment model available");
        let attempts = removed(&mut out);
        assert_eq!(attempts.len(), 1);
        assert!(!root.demo().unwrap().1.model);
        let shown = crate::shown(&root);
        let attempt = *attempts.get(0).unwrap();
        up(
            &mut root,
            jig_client::Up::Answer { attempt, envelope: skein_lib::RequestEnvelope::Final, answer, why: None },
        );
        assert_eq!(crate::shown(&root), shown, "cleanup outcome has no view");
        assert!(removed(&mut deliver_accounts(&mut root, account, ending_accounts(false))).is_empty());
        assert!(!root.demo().unwrap().1.model, "the ended half never returns");
        assert!(removed(&mut forge_rows(&mut root, forges, true)).is_empty(), "the second end sends no remove");
    }
}
fn demo_reread(out: &mut Queue<crate::Request>) -> skein_lib::Token {
    let mut read = None;
    let mut remaining = Queue::with_capacity(out.len());
    while let Some(request) = out.pop() {
        if let crate::Request::Client {
            request:
                jig_client::Request::Down {
                    down: jig_client::Down::Read { read: token, what: Readable::Family { record, .. } },
                },
        } = &request
        {
            assert_eq!(wire::decode_reading(&record.bytes, &w::CEILINGS).unwrap().read(), &w::Read::Demo);
            assert!(read.replace(*token).is_none());
        } else {
            remaining.push(request);
        }
    }
    *out = remaining;
    read.expect("Demo reread")
}
fn answer_no_demo(root: &mut crate::Domain, read: skein_lib::Token) -> Queue<crate::Request> {
    up(root, jig_client::Up::ReadDone { read, result: Readout::Family { family: wire::FORGE, record: None } })
}
fn deliver_role(root: &mut crate::Domain, me: skein_lib::Token, role: Option<c::HeldRole>) -> Queue<crate::Request> {
    let mut frame = f::chats::me();
    frame.deployment_role = role;
    up(root, jig_client::Up::Watched { watch: me, event: c::WatchEvent::Delivered(c::Delivery::Me(Box::new(frame))) })
}
#[test]
fn the_forge_end_rereads_once_and_ended_settles_without_removal() {
    // The deployment lists the demo model's account: the engine removes it at this end, not the client.
    let (mut root, account, forges, watch) = ending_root_with_deployment();
    assert_eq!(root.forge().demo_provider(), Some(PROVIDER));
    assert!(deployment_watch(&mut deliver_accounts(&mut root, account, ending_accounts(false))).is_none());
    assert!(forge_rows(&mut root, forges, false).is_empty());
    let mut out = forge_rows(&mut root, forges, true);
    let read = demo_reread(&mut out);
    assert_eq!(root.forge().demo_provider(), Some(PROVIDER), "the ended playground stays known");
    let (reloaded, _, _, _) = ending_root_with_page(Some(c::HeldRole::Owner), page_with_state(w::DemoState::Ended));
    assert_eq!(root.forge().retired_demo(), reloaded.forge().retired_demo());
    assert_eq!(root.pages().project(), None);
    assert!(!closed(&out, watch), "the running answer still holds the watch");
    assert!(deployment_watch(&mut out).is_none());
    assert!(removed(&mut out).is_empty(), "forge end sends no Remove");
    assert!(forge_rows(&mut root, forges, true).is_empty(), "unchanged connected rows never repeat the read");
    let mut out = up(&mut root, jig_client::Up::ReadDone { read, result: page_with_state(w::DemoState::Ended) });
    assert!(closed(&out, watch), "settled removal drops the watch once running ends");
    assert!(removed(&mut out).is_empty());
    assert!(root.forge().demo().is_none());
    assert_eq!(root.forge().demo_provider(), Some(PROVIDER), "rereading Ended retains the provider");
    assert!(root.demo().is_none());
    assert_eq!(root.forge().retired_demo(), reloaded.forge().retired_demo());
    assert!(deliver_demo_read(&mut root, Readout::NotServed { family: Some(wire::FORGE), served: None }).is_empty());
    assert_eq!(
        root.forge().retired_demo(),
        reloaded.forge().retired_demo(),
        "an unavailable reread cannot erase retirement"
    );
    assert!(removed(&mut deliver_deployment(&mut root, watch, deployment(&[DEMO_ACCOUNT]))).is_empty());
    assert!(removed(&mut deliver_accounts(&mut root, account, ending_accounts(true))).is_empty());
}
fn ending_root_with_deployment() -> (crate::Domain, skein_lib::Token, skein_lib::Token, skein_lib::Token) {
    let (mut root, account, forges, watch) = ending_root(Some(c::HeldRole::Owner));
    let watch = watch.expect("owner deployment watch");
    deliver_deployment(&mut root, watch, deployment(&[DEMO_ACCOUNT]));
    (root, account, forges, watch)
}
#[test]
fn both_ends_seen_in_one_pass_send_no_remove() {
    let (mut root, account, forges, watch) = ending_root_with_deployment();
    // Deliver the forge rows without a progress pass so both ends are observed together.
    crate::route::client(
        &mut root,
        &f::env(&f::limits()),
        jig_client::Event::Up { up: jig_client::Up::FamilyDelivered { watch: forges, record: forge_document(true) } },
        &mut Queue::with_capacity(crate::max_out(&f::limits())),
    );
    let mut out = deliver_accounts(&mut root, account, ending_accounts(true));
    let read = demo_reread(&mut out);
    assert!(root.demo().is_none(), "both halves ended");
    assert!(removed(&mut out).is_empty());
    let mut out = answer_no_demo(&mut root, read);
    assert!(closed(&out, watch));
    assert!(removed(&mut out).is_empty());
}
#[test]
fn a_forge_end_after_the_model_remove_sends_nothing_more() {
    let (mut root, account, forges, watch) = ending_root_with_deployment();
    assert_eq!(removed(&mut deliver_accounts(&mut root, account, ending_accounts(true))).len(), 1);
    let mut out = forge_rows(&mut root, forges, true);
    let read = demo_reread(&mut out);
    assert!(removed(&mut out).is_empty());
    let mut out = answer_no_demo(&mut root, read);
    assert!(closed(&out, watch));
    assert!(removed(&mut out).is_empty());
    assert!(removed(&mut deliver_deployment(&mut root, watch, deployment(&[DEMO_ACCOUNT]))).is_empty());
}
#[test]
fn cleanup_watches_only_for_an_owner_and_closes_once_running_and_removal_end() {
    for role in [None, Some(c::HeldRole::Member), Some(c::HeldRole::Maintainer), Some(c::HeldRole::Owner)] {
        let (mut root, account, forges, watch) = ending_root(role);
        assert!(
            deployment_watch(&mut deliver_accounts(&mut root, account, ending_accounts(false))).is_none(),
            "before end"
        );
        let mut out = deliver_accounts(&mut root, account, ending_accounts(true));
        assert!(deployment_watch(&mut out).is_none(), "end never opens another watch");
        assert!(removed(&mut out).is_empty());
        if role == Some(c::HeldRole::Owner) {
            let watch = watch.expect("owner watch");
            let mut out = deliver_deployment(&mut root, watch, deployment(&[]));
            assert!(!closed(&out, watch), "running demo keeps the watch even with removal settled");
            assert!(removed(&mut out).is_empty());
            assert!(deployment_watch(&mut deliver_accounts(&mut root, account, ending_accounts(true))).is_none());
            let mut out = forge_rows(&mut root, forges, true);
            let read = demo_reread(&mut out);
            assert!(deployment_watch(&mut out).is_none(), "second end uses the existing watch");
            assert!(closed(&answer_no_demo(&mut root, read), watch), "settled removal closes once running ends");
            assert!(deployment_watch(&mut deliver_accounts(&mut root, account, ending_accounts(true))).is_none());
        } else {
            assert!(watch.is_none(), "non-owner never watches deployment");
        }
    }
}
#[test]
fn cleanup_needs_a_running_answer_and_positive_evidence_of_an_end() {
    let mut cut = ending_accounts(false);
    cut.cut = true;
    for (case, page, accounts, watches) in [
        ("none", Readout::Family { family: wire::FORGE, record: None }, Some(ending_accounts(true)), false),
        ("not started", page_with_state(w::DemoState::NotStarted), Some(ending_accounts(true)), false),
        ("missing", page(), None, true),
        ("cut", page(), Some(cut), true),
    ] {
        let (mut root, account, _, watch) = ending_root_with_page(Some(c::HeldRole::Owner), page);
        assert_eq!(watch.is_some(), watches, "{case}");
        let mut out = match accounts {
            Some(document) => deliver_accounts(&mut root, account, document),
            None => event(
                &mut root,
                crate::Event::Went {
                    place: crate::Place::Address(crate::Address::Chats {
                        filter: crate::ChatsFilter::All,
                        start: false,
                    }),
                },
            ),
        };
        assert!(deployment_watch(&mut out).is_none(), "{case}");
        assert!(removed(&mut out).is_empty(), "{case}");
        if let Some(watch) = watch {
            let mut out = deliver_deployment(&mut root, watch, deployment(&[DEMO_ACCOUNT]));
            assert!(!closed(&out, watch), "{case}: no end to settle");
            assert!(removed(&mut out).is_empty(), "{case}");
        }
    }
}

#[test]
fn deployment_model_skips_ineligible_accounts_and_keeps_document_order() {
    let mut document = deployment(&[
        DEMO_ACCOUNT,
        (Some(PROVIDER + 1), true, false, c::OwnerAccountHolder::Party(f::PARTY)),
        (Some(PROVIDER + 1), true, true, c::OwnerAccountHolder::Deployment),
        (Some(PROVIDER + 1), false, false, c::OwnerAccountHolder::Deployment),
        OTHER_ACCOUNT,
        (Some(PROVIDER + 2), true, false, c::OwnerAccountHolder::Deployment),
    ]);
    assert_eq!(demo::deployment_model(&document, PROVIDER), document.accounts.get(4));
    document.cut = true;
    assert!(demo::deployment_model(&document, PROVIDER).is_none());
}

#[test]
fn the_model_half_for_an_owner_reads_the_deployments_own_accounts_too() {
    let answer = forge::family::demo(&answer(CUE), &w::CEILINGS).unwrap();
    let accounts = accounts(Some(PROVIDER), true, false);
    let mut cut = deployment(&[]);
    cut.cut = true;
    for (row, owner, document, model) in [
        ("member without deployment", false, None, true),
        ("owner awaiting deployment", true, None, true),
        ("deployment cut", true, Some(cut), true),
        ("demo provider usable", true, Some(deployment(&[DEMO_ACCOUNT])), true),
        ("other provider usable", true, Some(deployment(&[OTHER_ACCOUNT])), false),
        (
            "other provider unusable",
            true,
            Some(deployment(&[(Some(PROVIDER + 1), false, false, c::OwnerAccountHolder::Deployment)])),
            true,
        ),
        (
            "other provider cut",
            true,
            Some(deployment(&[(Some(PROVIDER + 1), true, true, c::OwnerAccountHolder::Deployment)])),
            true,
        ),
        (
            "party held account",
            true,
            Some(deployment(&[(Some(PROVIDER + 1), true, false, c::OwnerAccountHolder::Party(f::PARTY))])),
            true,
        ),
    ] {
        let deployment = if owner { document.as_ref() } else { None };
        assert_eq!(
            demo::halves(Some(&answer), Some(&accounts), deployment, &forge::forges::State::Rows(Box::new([]))),
            demo::Halves { model, forge: true },
            "{row}"
        );
    }
}

#[test]
fn an_owners_deployment_account_of_another_provider_ends_the_model_half_and_removes_the_demo_account() {
    let (mut root, account, forges, watch) = ending_root(Some(c::HeldRole::Owner));
    let watch = watch.expect("one deployment watch at running demo read");
    deliver_accounts(&mut root, account, ending_accounts(false));
    forge_rows(&mut root, forges, false);
    let mut out = deliver_deployment(&mut root, watch, deployment(&[DEMO_ACCOUNT]));
    assert!(root.demo().unwrap().1.model);
    assert!(!closed(&out, watch), "before an end the watch stays open");
    assert!(deployment_watch(&mut out).is_none());
    assert!(removed(&mut out).is_empty());

    let mut out = deliver_deployment(&mut root, watch, deployment(&[DEMO_ACCOUNT, OTHER_ACCOUNT]));
    assert!(!root.demo().unwrap().1.model);
    assert!(!closed(&out, watch), "forge half still holds: keep the deployment model available");
    assert!(deployment_watch(&mut out).is_none(), "the model and removal share one watch");
    assert_eq!(removed(&mut out).len(), 1);
    for mut out in [
        deliver_accounts(&mut root, account, ending_accounts(false)),
        forge_rows(&mut root, forges, false),
        deliver_deployment(&mut root, watch, deployment(&[DEMO_ACCOUNT])),
    ] {
        assert!(deployment_watch(&mut out).is_none());
        assert!(removed(&mut out).is_empty());
    }
    assert!(!root.demo().unwrap().1.model, "the half never returns after its credential disappears");
    let mut out = forge_rows(&mut root, forges, true);
    let read = demo_reread(&mut out);
    assert!(!closed(&out, watch), "running answer keeps the watch");
    assert!(deployment_watch(&mut out).is_none());
    assert!(removed(&mut out).is_empty(), "forge end sends no second Remove");
    let mut out = answer_no_demo(&mut root, read);
    assert!(closed(&out, watch), "settled removal closes once the reread answers none");
    assert!(deployment_watch(&mut out).is_none());
    assert!(removed(&mut out).is_empty());
    assert!(root.demo().is_none());
    let mut out = deliver_accounts(&mut root, account, ending_accounts(false));
    assert!(deployment_watch(&mut out).is_none(), "settled end opens no second watch");
    assert!(removed(&mut out).is_empty());
    assert!(root.demo().is_none(), "neither ended half returns");
}

#[test]
fn an_owners_start_out_or_failed_before_a_running_read_shows_as_before() {
    let (mut root, account, forges, watch) =
        ending_root_with_page(Some(c::HeldRole::Owner), page_with_state(w::DemoState::NotStarted));
    assert!(watch.is_none(), "not started alone needs no watch");
    deliver_accounts(&mut root, account, ending_accounts(false));
    forge_rows(&mut root, forges, false);
    let mut out = act(&mut root, Act::Playground);
    let watch = deployment_watch(&mut out).expect("start out opens the owner watch");
    assert!(root.forge().demo_starting());
    assert_eq!(root.forge().demo().unwrap().state, demo::DemoState::NotStarted);
    assert!(root.demo().unwrap().1.model, "accounts alone hold before deployment delivery");
    let mut attempt = None;
    while let Some(request) = out.pop() {
        if let crate::Request::Client {
            request: jig_client::Request::Down { down: jig_client::Down::Send { attempt: token, .. } },
        } = request
        {
            assert!(attempt.replace(token).is_none(), "one start");
        }
    }
    let failure = w::Outcome::StartFailed(
        w::StartFailed::new(&w::CEILINGS, w::StartFailedParts { why: w::StartFailure::Account }).unwrap(),
    );
    let mut out = up(
        &mut root,
        jig_client::Up::Answer {
            attempt: attempt.expect("start attempt"),
            envelope: skein_lib::RequestEnvelope::Final,
            answer: Some(jig_client::Answered::Family(jig_client::Family {
                family: wire::FORGE,
                version: wire::CLIENT_VERSION,
                bytes: wire::answer(&w::Answer::new(&w::CEILINGS, w::AnswerParts { outcome: failure }).unwrap()),
            })),
            why: None,
        },
    );
    assert!(!root.forge().demo_starting());
    assert_eq!(root.forge().demo_failure(), Some(&demo::StartFailure::Account));
    assert!(root.demo().unwrap().1.model, "failure remains visible without a deployment delivery");
    assert!(!closed(&out, watch), "failed start keeps its watch");
    assert!(deployment_watch(&mut out).is_none());
    assert!(removed(&mut out).is_empty());
}

#[test]
fn the_demo_providers_deployment_account_ends_nothing() {
    let (mut root, account, forges, watch) = ending_root(Some(c::HeldRole::Owner));
    let watch = watch.expect("owner deployment watch");
    deliver_accounts(&mut root, account, ending_accounts(false));
    forge_rows(&mut root, forges, false);
    for document in [deployment(&[DEMO_ACCOUNT]), deployment(&[DEMO_ACCOUNT, DEMO_ACCOUNT])] {
        let mut out = deliver_deployment(&mut root, watch, document);
        assert!(root.demo().unwrap().1.model);
        assert!(!closed(&out, watch));
        assert!(deployment_watch(&mut out).is_none());
        assert!(removed(&mut out).is_empty());
    }
}

#[test]
fn a_members_model_half_is_unchanged() {
    for role in [None, Some(c::HeldRole::Member), Some(c::HeldRole::Maintainer)] {
        let (mut root, account, forges, watch) = ending_root(role);
        assert!(watch.is_none(), "{role:?}: running read opens no deployment watch");
        let mut out = deliver_accounts(&mut root, account, ending_accounts(false));
        assert!(deployment_watch(&mut out).is_none());
        assert!(removed(&mut out).is_empty());
        assert!(root.demo().unwrap().1.model);
        let mut out = deliver_accounts(&mut root, account, ending_accounts(true));
        assert!(deployment_watch(&mut out).is_none(), "{role:?}: end opens no deployment watch");
        assert!(removed(&mut out).is_empty());
        let mut out = forge_rows(&mut root, forges, false);
        assert!(deployment_watch(&mut out).is_none());
        assert!(removed(&mut out).is_empty());
        assert!(!root.demo().unwrap().1.model);
    }
}

#[test]
fn the_owner_role_opens_the_watch_when_the_running_read_arrived_first() {
    let mut root = crate::unlinked(&f::limits(), f::SEED);
    let (me, _) = f::start_accounts(&mut root, List::with_capacity(0));
    let mut out = deliver_demo_read(&mut root, page());
    assert!(deployment_watch(&mut out).is_none(), "wait for live owner evidence");
    let mut out = deliver_role(&mut root, me, Some(c::HeldRole::Owner));
    assert!(deployment_watch(&mut out).is_some(), "one watch when the owner role arrives");
    let mut out = deliver_role(&mut root, me, Some(c::HeldRole::Owner));
    assert!(deployment_watch(&mut out).is_none(), "repeated me never opens another watch");
}

#[test]
fn an_unsettled_removal_reopens_for_a_returning_owner_after_none() {
    let mut root = crate::unlinked(&f::limits(), f::SEED);
    let (me, account) = f::start_accounts(&mut root, List::with_capacity(0));
    deliver_demo_read(&mut root, page());
    let watch = deployment_watch(&mut deliver_role(&mut root, me, Some(c::HeldRole::Owner))).unwrap();
    deliver_accounts(&mut root, account, ending_accounts(true));
    let mut out = deliver_demo_read(&mut root, Readout::Family { family: wire::FORGE, record: None });
    assert!(!closed(&out, watch), "the end keeps the unsettled watch after none");
    assert!(deployment_watch(&mut out).is_none());
    let mut cut = deployment(&[DEMO_ACCOUNT]);
    cut.cut = true;
    let mut out = deliver_deployment(&mut root, watch, cut);
    assert!(removed(&mut out).is_empty(), "cut deployment cannot settle the removal");
    let mut out = deliver_role(&mut root, me, Some(c::HeldRole::Member));
    assert!(closed(&out, watch), "a non-owner holds no watch");
    assert!(removed(&mut out).is_empty());
    assert!(removed(&mut deliver_deployment(&mut root, watch, deployment(&[DEMO_ACCOUNT]))).is_empty());

    let mut out = deliver_role(&mut root, me, Some(c::HeldRole::Owner));
    let next = deployment_watch(&mut out).expect("returning owner resumes the unsettled removal");
    assert_ne!(watch, next);
    let mut out = deliver_deployment(&mut root, next, deployment(&[DEMO_ACCOUNT]));
    assert!(closed(&out, next), "none plus settled removal needs no watch");
    assert_eq!(removed(&mut out).len(), 1, "the first end retains the demo provider");
    let mut out = deliver_role(&mut root, me, Some(c::HeldRole::Owner));
    assert!(deployment_watch(&mut out).is_none(), "settled end stays closed");
    assert!(removed(&mut out).is_empty());
}

#[test]
fn a_retired_reload_is_independent_of_document_order() {
    for ended_first in [true, false] {
        let mut root = crate::unlinked(&f::limits(), f::SEED);
        let (me, account) = f::start_accounts(&mut root, List::with_capacity(0));
        if ended_first {
            assert!(deliver_demo_read(&mut root, page_with_state(w::DemoState::Ended)).is_empty());
        }
        let mut out = deliver_role(&mut root, me, Some(c::HeldRole::Owner));
        assert!(deployment_watch(&mut out).is_none());
        let forges = family_watch(&out);
        forge_rows(&mut root, forges, false);
        deliver_accounts(&mut root, account, accounts(Some(PROVIDER), true, false));
        if !ended_first {
            assert!(deliver_demo_read(&mut root, page_with_state(w::DemoState::Ended)).is_empty());
        }
        assert_eq!(root.forge().retired_demo(), Some(demo::RetiredDemo { provider: PROVIDER, project: f::PROJECT }));
        assert_eq!(root.forge().demo_provider(), Some(PROVIDER));
        assert!(root.forge().demo().is_none());
        assert!(root.demo().is_none());
        assert_eq!(root.pages().project(), None);
        assert_eq!(crate::pages::welcome::model_usable(&root), Some(false), "stale demo account is excluded");
        for act in [Act::Playground, Act::RetryDemo] {
            assert!(event(&mut root, crate::Event::Act { action: crate::Action::Pages(act) }).is_empty());
        }
        let mut out = deliver_accounts(&mut root, account, ending_accounts(true));
        assert!(deployment_watch(&mut out).is_none());
        assert!(removed(&mut out).is_empty());
        assert!(forge_rows(&mut root, forges, true).is_empty(), "retired demo never rereads on forge connection");
    }
}
fn family_watch(out: &Queue<crate::Request>) -> skein_lib::Token {
    for request in out {
        if let crate::Request::Client {
            request:
                jig_client::Request::Down {
                    down: jig_client::Down::Open { watch, subject: jig_client::Watchable::Family { .. } },
                },
        } = request
        {
            return *watch;
        }
    }
    panic!("forges watch")
}
#[test]
fn a_retired_project_is_skipped_but_a_running_one_stays() {
    const OTHER_PROJECT: u32 = 2;
    for (state, expected) in [(w::DemoState::Ended, Some(OTHER_PROJECT)), (w::DemoState::Running, Some(f::PROJECT))] {
        let mut root = crate::unlinked(&f::limits(), f::SEED);
        let (watch, _) = f::start_accounts(&mut root, List::with_capacity(0));
        deliver_demo_read(&mut root, page_with_state(state));
        let mut me = f::chats::me();
        let mut other = me.projects.get(0).unwrap().clone();
        other.project = OTHER_PROJECT;
        let mut projects = List::with_capacity(2);
        projects.push(me.projects.get(0).unwrap().clone()).unwrap();
        projects.push(other).unwrap();
        me.projects = projects;
        up(
            &mut root,
            jig_client::Up::Watched { watch, event: c::WatchEvent::Delivered(c::Delivery::Me(Box::new(me))) },
        );
        assert_eq!(root.pages().project(), expected);
    }
}

#[test]
fn a_forge_connected_before_the_playground_starts_retires_and_rereads_once() {
    for survivor in [false, true] {
        let mut root = crate::unlinked(&f::limits(), f::SEED);
        let (me_watch, account) = f::start_accounts(&mut root, List::with_capacity(0));
        let forges = family_watch(&deliver_role(&mut root, me_watch, Some(c::HeldRole::Owner)));
        let mut out = deliver_demo_read(&mut root, page_with_state(w::DemoState::NotStarted));
        assert!(deployment_watch(&mut out).is_none());
        if survivor {
            let mut me = f::chats::me();
            let mut other = me.projects.get(0).unwrap().clone();
            other.project = 2;
            let mut projects = List::with_capacity(2);
            projects.push(me.projects.get(0).unwrap().clone()).unwrap();
            projects.push(other).unwrap();
            me.projects = projects;
            up(
                &mut root,
                jig_client::Up::Watched {
                    watch: me_watch,
                    event: c::WatchEvent::Delivered(c::Delivery::Me(Box::new(me))),
                },
            );
        }
        let mut out = forge_rows(&mut root, forges, true);
        let reread = demo_reread(&mut out);
        assert_eq!(root.forge().retired_demo(), Some(demo::RetiredDemo { provider: PROVIDER, project: f::PROJECT }));
        assert_eq!(root.pages().project(), if survivor { Some(2) } else { None });
        assert!(root.demo().is_none());
        assert!(deployment_watch(&mut out).is_none(), "NotStarted has no model cleanup");
        assert!(removed(&mut out).is_empty());
        assert!(forge_rows(&mut root, forges, true).is_empty(), "only one reread");
        let mut out =
            up(&mut root, jig_client::Up::ReadDone { read: reread, result: page_with_state(w::DemoState::Ended) });
        assert!(removed(&mut out).is_empty());
        assert!(deployment_watch(&mut deliver_accounts(&mut root, account, ending_accounts(true))).is_none());
    }
}
