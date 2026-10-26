use crate::ChatsFilter;
use crate::tests::{demo, fixtures as f};
use crate::{
    Action, Address, Domain, Event, Place, Request, Step, forge,
    pages::{self, Act},
};
use jig_client::{Down, Link, Readout, Told, Up, contract as c};
use skein_lib::{List, Queue, Secret, Token};
use temper_client_protocol_wire::{self as wire, v1 as w};

fn event(root: &mut Domain, event: Event) -> Queue<Request> {
    let mut out = Queue::with_capacity(crate::max_out(&f::limits()));
    crate::step(root, &f::env(&f::limits()), event, &mut out);
    out
}
fn up(root: &mut Domain, up: Up) -> Queue<Request> {
    event(root, Event::Client { event: jig_client::Event::Up { up } })
}
fn go(root: &mut Domain, step: Step) -> Queue<Request> {
    event(root, Event::Went { place: Place::Address(Address::Welcome { step }) })
}
fn deliver(root: &mut Domain, watch: Token, delivery: c::Delivery) -> Queue<Request> {
    up(root, Up::Watched { watch, event: c::WatchEvent::Delivered(delivery) })
}
fn answer(root: &mut Domain, present: bool) {
    let mut out = Queue::with_capacity(1);
    forge::demo::told(&mut root.forge, &Told::Link { link: Link::Offline }, f::limits().forge, &mut out);
    forge::demo::told(&mut root.forge, &Told::Link { link: Link::Live }, f::limits().forge, &mut out);
    let Some(forge::Request::Client { asked: jig_client::Event::Read { handle, .. } }) = out.pop() else {
        panic!("demo read")
    };
    let page = if present {
        let mut parts =
            wire::decode_demo_answer(&demo::answer(b"Change the title"), &w::CEILINGS).unwrap().into_parts();
        parts.state = w::DemoState::NotStarted;
        Readout::Family {
            family: wire::FORGE,
            record: Some(wire::demo_answer(&w::DemoAnswer::new(&w::CEILINGS, parts).unwrap())),
        }
    } else {
        Readout::NotServed { family: Some(wire::FORGE), served: None }
    };
    crate::route::told(
        root,
        &f::env(&f::limits()),
        Told::Page { handle, page },
        &mut Queue::with_capacity(crate::max_out(&f::limits())),
    );
}
fn opened(owner: bool, step: Step) -> (Domain, Token, Token, Option<Token>) {
    let mut root = crate::unlinked(&f::limits(), f::SEED);
    let (me, accounts) = f::start_accounts(&mut root, List::with_capacity(0));
    let c::WatchEvent::Delivered(c::Delivery::Me(mut frame)) = f::frame(f::PARTY) else { panic!("me") };
    frame.projects = List::with_capacity(1);
    frame
        .projects
        .push(c::MeProject {
            project: f::PROJECT,
            name: jig_client::contract::DocumentText::complete(
                jig_client::contract::Text::new(b"Project 1".as_slice().into(), 64).expect("project name"),
            ),
            role: c::HeldRole::Member,
            pool: None,
            templates: List::with_capacity(0),
            creating: None,
            models: List::with_capacity(0),
        })
        .unwrap();
    frame.providers = List::with_capacity(1);
    frame
        .providers
        .push(c::ProviderOffer {
            provider: 17,
            name: c::DocumentText::complete(c::Text::new(b"openai".as_slice().into(), 32).unwrap()),
            key_bytes: 256,
            sign_in: true,
            signed_in_plan: true,
            key: true,
            programs: List::with_capacity(0),
            programs_cut: false,
            none: false,
            manage_usage: None,
        })
        .unwrap();
    frame.deployment_role = Some(if owner { c::HeldRole::Owner } else { c::HeldRole::Member });
    let mut out = deliver(&mut root, me, c::Delivery::Me(frame));
    go(&mut root, step);
    let mut forges = None;
    while let Some(request) = out.pop() {
        if let Request::Client {
            request:
                jig_client::Request::Down { down: Down::Open { watch, subject: jig_client::Watchable::Family { .. } } },
        } = request
        {
            forges = Some(watch);
        }
    }
    (root, me, accounts, forges)
}
fn rows(root: &mut Domain, watch: Token, number: u16) -> Queue<Request> {
    let limits = forge::wire_limits(&f::limits().forge).unwrap();
    let mut forges = List::with_capacity(1);
    forges
        .push(
            w::ForgeRow::new(
                &limits,
                w::ForgeRowParts {
                    number,
                    kind: w::Kind::Forgejo,
                    mode: w::Mode::Connected,
                    connection: w::Connection::Connected(
                        w::Connected::new(&limits, w::ConnectedParts { id: 42, login: b"bot".as_slice().into() })
                            .unwrap(),
                    ),
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
    up(
        root,
        Up::FamilyDelivered {
            watch,
            record: jig_client::Family {
                family: wire::FORGE,
                version: wire::CLIENT_VERSION,
                bytes: wire::forges(&w::Forges::new(&limits, w::ForgesParts { forges }).unwrap()),
            },
        },
    )
}
fn account(root: &mut Domain, watch: Token, provider: u32, usable: bool) -> Queue<Request> {
    deliver(root, watch, c::Delivery::OwnerAccounts(Box::new(demo::accounts(Some(provider), usable, false))))
}
fn moved(mut out: Queue<Request>, replace: bool) -> bool {
    while let Some(request) = out.pop() {
        if request == (Request::Show { address: Address::Chats { filter: ChatsFilter::All, start: false }, replace }) {
            return true;
        }
    }
    false
}
fn decided(root: &Domain) -> bool {
    root.pages().welcome().unwrap().decided
}
#[test]
fn the_welcome_moves_on_once_an_account_and_a_forge_are_the_persons() {
    let (mut root, _, accounts, forges) = opened(true, Step::Welcome);
    answer(&mut root, true);
    account(&mut root, accounts, 18, true);
    assert!(moved(rows(&mut root, forges.unwrap(), wire::FORGE + 1), true));
    assert!(decided(&root));
}
#[test]
fn the_demos_account_and_forge_keep_the_welcome() {
    for (provider, number) in [(17, wire::FORGE), (18, wire::FORGE), (17, wire::FORGE + 1)] {
        let (mut root, _, accounts, forges) = opened(true, Step::Welcome);
        answer(&mut root, true);
        account(&mut root, accounts, provider, true);
        assert!(!moved(rows(&mut root, forges.unwrap(), number), true));
        assert!(decided(&root));
    }
}
#[test]
fn an_account_alone_decides_for_a_person_without_the_forges_role() {
    let (mut root, _, accounts, forges) = opened(false, Step::Welcome);
    assert!(forges.is_some());
    answer(&mut root, true);
    assert!(moved(account(&mut root, accounts, 18, true), true));
}
#[test]
fn the_welcome_waits_for_its_facts() {
    let (mut root, _, accounts, forges) = opened(true, Step::Welcome);
    account(&mut root, accounts, 18, true);
    up(&mut root, Up::Watched { watch: forges.unwrap(), event: c::WatchEvent::Opened });
    assert!(!decided(&root));
    rows(&mut root, forges.unwrap(), wire::FORGE + 1);
    assert!(!decided(&root));
    answer(&mut root, true);
    let mut out = Queue::with_capacity(crate::max_out(&f::limits()));
    crate::fire(&mut root, &f::env(&f::limits()), &mut out);
    assert!(decided(&root));
    assert!(moved(out, true));
}
#[test]
fn a_refused_forges_watch_counts_as_no_forge() {
    for end in [c::WatchEnd::NotAllowed, c::WatchEnd::NotServed, c::WatchEnd::SubjectEnded] {
        let (mut root, _, accounts, forges) = opened(true, Step::Welcome);
        answer(&mut root, true);
        account(&mut root, accounts, 18, true);
        assert!(!moved(up(&mut root, Up::WatchEnded { watch: forges.unwrap(), end }), true));
        assert!(decided(&root));
    }
}
#[test]
fn an_unserved_demo_read_counts_as_no_demo() {
    let (mut root, _, accounts, forges) = opened(true, Step::Welcome);
    answer(&mut root, false);
    assert!(root.demo_answered());
    assert!(root.forge().demo().is_none());
    account(&mut root, accounts, 17, true);
    assert!(moved(rows(&mut root, forges.unwrap(), wire::FORGE), true));
}
#[test]
fn an_ended_accounts_watch_counts_as_no_account() {
    let (mut root, _, accounts, forges) = opened(true, Step::Welcome);
    answer(&mut root, true);
    rows(&mut root, forges.unwrap(), wire::FORGE + 1);
    assert!(!moved(up(&mut root, Up::WatchEnded { watch: accounts, end: c::WatchEnd::NotAllowed }), true));
    assert!(decided(&root));
}
#[test]
fn a_cut_accounts_document_counts_as_an_answer() {
    let (mut root, _, accounts, _) = opened(false, Step::Welcome);
    answer(&mut root, false);
    let mut document = demo::accounts(Some(18), true, true);
    document.cut = true;
    assert!(moved(deliver(&mut root, accounts, c::Delivery::OwnerAccounts(Box::new(document))), true));
    assert!(decided(&root));
}
#[test]
fn the_welcome_decides_once_each_time_it_opens() {
    let (mut root, _, accounts, _) = opened(false, Step::Welcome);
    answer(&mut root, true);
    assert!(!moved(account(&mut root, accounts, 18, false), true));
    assert!(decided(&root));
    assert!(!moved(account(&mut root, accounts, 18, true), true));
    go(&mut root, Step::Forge { forge: None });
    assert!(moved(go(&mut root, Step::Welcome), true));
}
#[test]
fn a_later_step_never_moves_on() {
    for step in [Step::Forge { forge: None }, Step::Model { way: None }, Step::Ready] {
        let (mut root, _, accounts, forges) = opened(true, step);
        answer(&mut root, true);
        account(&mut root, accounts, 18, true);
        assert!(!moved(rows(&mut root, forges.unwrap(), wire::FORGE + 1), true));
        assert!(!decided(&root));
    }
}
#[test]
fn try_the_playground_opens_the_chats_in_the_demos_project() {
    let (mut root, _, _, _) = opened(false, Step::Welcome);
    assert!(!moved(event(&mut root, Event::Act { action: Action::Pages(Act::Playground) }), false));
    answer(&mut root, true);
    assert!(moved(event(&mut root, Event::Act { action: Action::Pages(Act::Playground) }), false));
    assert_eq!(root.pages().project(), Some(f::PROJECT));
}
#[test]
fn the_model_step_sends_the_accounts_acts() {
    use pages::connect_model::Act as A;
    let (mut root, _, _, _) = opened(false, Step::Model { way: None });
    go(&mut root, Step::Model { way: Some(crate::ModelWay { source: None, provider: 17, kind: crate::WayKind::Key }) });
    event(&mut root, Event::AccountKey { value: Secret::new(b"private-test-key".as_slice().into()) });
    let mut out =
        event(&mut root, Event::Act { action: Action::Pages(Act::ConnectModel(A::Submit { name: [1; 16] })) });
    let mut sent = false;
    while let Some(request) = out.pop() {
        if let Request::Client { request: jig_client::Request::Down { down: Down::Send { value: Some(value), .. } } } =
            request
        {
            assert_eq!(value.expose(), b"private-test-key");
            sent = true;
        }
    }
    assert!(sent);
    assert!(!root.pages().connect_model().unwrap().request.has_key());
}
#[test]
fn leaving_the_welcome_keeps_the_sessions_forges() {
    let (mut root, _, _, forges) = opened(true, Step::Welcome);
    let watch = forges.unwrap();
    up(&mut root, Up::Watched { watch, event: c::WatchEvent::Opened });
    let mut out = event(
        &mut root,
        Event::Went { place: Place::Address(Address::Chats { filter: ChatsFilter::All, start: false }) },
    );
    let mut closed = false;
    while let Some(request) = out.pop() {
        if let Request::Client { request: jig_client::Request::Down { down: Down::Close { watch: closing } } } = request
        {
            closed |= closing == watch;
        }
    }
    assert!(!closed);
    assert!(root.pages().welcome().is_none());
}
#[test]
fn losing_the_owner_role_keeps_the_sessions_watch() {
    let (mut root, me, _, forges) = opened(true, Step::Welcome);
    let watch = forges.unwrap();
    up(&mut root, Up::Watched { watch, event: c::WatchEvent::Opened });
    let c::WatchEvent::Delivered(frame) = f::frame(f::PARTY) else { panic!("frame") };
    let mut out = deliver(&mut root, me, frame);
    let mut closed = false;
    while let Some(request) = out.pop() {
        if let Request::Client { request: jig_client::Request::Down { down: Down::Close { watch: closing } } } = request
        {
            closed |= closing == watch;
        }
    }
    assert!(!closed);
}
#[test]
fn an_invalid_forges_document_counts_as_no_forge() {
    let (mut root, _, accounts, forges) = opened(true, Step::Welcome);
    answer(&mut root, true);
    account(&mut root, accounts, 18, true);
    assert!(!moved(
        up(
            &mut root,
            Up::FamilyDelivered {
                watch: forges.unwrap(),
                record: jig_client::Family {
                    family: wire::FORGE,
                    version: wire::CLIENT_VERSION,
                    bytes: Box::new([255])
                }
            }
        ),
        true
    ));
    assert!(decided(&root));
    let forge::forges::State::Ended = root.forges() else { panic!("invalid rows end the watch") };
}

fn strip_forges(root: &mut Domain, watch: Token, connection: Option<w::Connection>) {
    let mut forges = List::with_capacity(1);
    if let Some(connection) = connection {
        forges
            .push(
                w::ForgeRow::new(
                    &w::CEILINGS,
                    w::ForgeRowParts {
                        number: 3,
                        kind: w::Kind::Forgejo,
                        mode: w::Mode::Owned,
                        connection,
                        offer: w::Offer::Offered,
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
    }
    up(
        root,
        Up::FamilyDelivered {
            watch,
            record: jig_client::Family {
                family: wire::FORGE,
                version: wire::CLIENT_VERSION,
                bytes: wire::forges(&w::Forges::new(&w::CEILINGS, w::ForgesParts { forges }).unwrap()),
            },
        },
    );
}
#[test]
fn the_setup_strip_shows_while_there_is_no_demo_and_the_first_run_is_not_over() {
    use forge::listed;
    let (mut root, _, accounts, forges) = opened(true, Step::Model { way: None });
    event(&mut root, Event::Went { place: Place::Address(Address::Chats { filter: ChatsFilter::All, start: false }) });
    assert!(listed::setup(&root).is_none());
    account(&mut root, accounts, 18, false);
    strip_forges(&mut root, forges.unwrap(), None);
    assert!(listed::setup(&root).is_none());
    answer(&mut root, false);
    assert!(listed::setup(&root).is_some());
    answer(&mut root, true);
    assert!(listed::setup(&root).is_some());
}
#[test]
fn the_strip_names_what_is_missing_and_a_non_owner_misses_the_model_alone() {
    use forge::listed;
    for owner in [false, true] {
        let (mut root, _, accounts, forges) = opened(owner, Step::Model { way: None });
        answer(&mut root, false);
        account(&mut root, accounts, 18, false);
        strip_forges(&mut root, forges.unwrap(), None);
        let setup = listed::setup(&root).unwrap();
        assert_eq!(setup.forge, owner);
        assert!(setup.model);
        account(&mut root, accounts, 18, true);
        if owner {
            let setup = listed::setup(&root).unwrap();
            assert!(setup.forge);
            assert!(!setup.model);
        } else {
            assert!(listed::setup(&root).is_none());
        }
    }
}
#[test]
fn continue_setting_up_goes_to_the_first_step_not_done() {
    use forge::listed;
    let (mut root, _, accounts, forges) = opened(true, Step::Model { way: None });
    let watch = forges.unwrap();
    answer(&mut root, false);
    account(&mut root, accounts, 18, false);
    strip_forges(&mut root, watch, None);
    assert_eq!(listed::setup(&root).unwrap().step, Step::Forge { forge: None });
    strip_forges(&mut root, watch, Some(w::Connection::Connecting));
    assert_eq!(listed::setup(&root).unwrap().step, Step::Model { way: None });
    account(&mut root, accounts, 18, true);
    assert_eq!(listed::setup(&root).unwrap().step, Step::Ready);
    strip_forges(
        &mut root,
        watch,
        Some(w::Connection::Connected(
            w::Connected::new(&w::CEILINGS, w::ConnectedParts { id: 42, login: b"temper".as_slice().into() }).unwrap(),
        )),
    );
    assert!(listed::setup(&root).is_none());
    account(&mut root, accounts, 18, false);
    let setup = listed::setup(&root).unwrap();
    assert!(!setup.forge);
    assert!(setup.model);
    assert_eq!(setup.step, Step::Model { way: None });
}

#[test]
fn the_playground_forge_shows_while_the_forge_half_holds() {
    let (mut root, _, accounts, forges) = opened(true, Step::Model { way: None });
    let watch = forges.unwrap();
    answer(&mut root, true);
    event(&mut root, Event::Act { action: Action::Pages(Act::Playground) });
    account(&mut root, accounts, 18, true);
    rows(&mut root, watch, wire::FORGE);
    assert_eq!(forge::listed::playground(&root).unwrap().1.number, wire::FORGE);
    rows(&mut root, watch, 3);
    assert!(forge::listed::playground(&root).is_none());
}

#[test]
fn without_a_demo_model_usable_reads_the_documents_usable() {
    for (document_usable, line_usable) in [(true, false), (false, true), (false, false), (true, true)] {
        let (mut root, _, accounts, _) = opened(false, Step::Model { way: None });
        answer(&mut root, false);
        let mut document = demo::accounts(Some(18), line_usable, false);
        document.usable = document_usable;
        deliver(&mut root, accounts, c::Delivery::OwnerAccounts(Box::new(document)));
        assert_eq!(pages::welcome::model_usable(&root), Some(document_usable));
    }
}

#[test]
fn with_a_demo_model_usable_reads_other_providers_lines() {
    for (provider, document_usable, line_usable, expected) in
        [(18, true, false, false), (18, false, true, true), (17, true, true, false)]
    {
        let (mut root, _, accounts, _) = opened(false, Step::Model { way: None });
        answer(&mut root, true);
        let mut document = demo::accounts(Some(provider), line_usable, false);
        document.usable = document_usable;
        deliver(&mut root, accounts, c::Delivery::OwnerAccounts(Box::new(document)));
        assert_eq!(pages::welcome::model_usable(&root), Some(expected));
    }
}

#[test]
fn ready_and_the_strip_count_an_owners_deployment_model() {
    let (mut root, _, accounts, _) = opened(true, Step::Model { way: None });
    answer(&mut root, true);
    account(&mut root, accounts, 18, false);
    let mut out = event(&mut root, Event::Act { action: Action::Pages(Act::Playground) });
    let watch = demo::deployment_watch(&mut out).expect("owner start opens the deployment watch");
    let mut out =
        demo::deliver_deployment(&mut root, watch, demo::deployment(&[demo::DEMO_ACCOUNT, demo::OTHER_ACCOUNT]));
    assert!(demo::removed(&mut out).is_empty(), "removal waits for the running read");
    assert_eq!(pages::welcome::model_usable(&root), Some(true));

    let mut out = demo::deliver_demo_read(
        &mut root,
        Readout::Family { family: wire::FORGE, record: Some(demo::answer(b"Change the title")) },
    );
    assert_eq!(demo::removed(&mut out).len(), 1);
    assert_eq!(pages::welcome::model_usable(&root), Some(true), "the watch keeps the model after removal");
    demo::deliver_deployment(&mut root, watch, demo::deployment(&[demo::OTHER_ACCOUNT]));
    assert_eq!(pages::welcome::model_usable(&root), Some(true), "removed demo account changes nothing");

    let (mut member, _, accounts, _) = opened(false, Step::Model { way: None });
    answer(&mut member, true);
    account(&mut member, accounts, 18, false);
    assert_eq!(pages::welcome::model_usable(&member), Some(false));
}
