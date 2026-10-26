use super::{demo, fixtures as f};
use crate::ChatsFilter;
use crate::{
    Address, Choice, Event, Place, Request, Step,
    forge::{
        self,
        connect::{self, Shown},
        forges::{self, Connection as C, Kind, Mode, Offer, Row, State},
    },
};
use jig_client::{self as client, Down, Told, Up, contract as c};
use skein_lib::{Duration, List, Queue, Time, Token};
use temper_client_protocol_wire::{self as wire, v1 as w};
const HERE: u16 = 3;
fn row(number: u16, kind: Kind, mode: Mode, connection: C, offer: Offer) -> Row {
    Row {
        number,
        kind,
        mode,
        connection,
        offer,
        first: None,
        installations: Box::new([]),
        ended: None,
        app: None,
        web: None,
        repositories: Box::new([]),
        owned: None,
        admin: false,
        read_only: false,
        ended_stop: None,
    }
}
fn rows(connection: C) -> State {
    State::Rows(Box::new([row(HERE, Kind::Forgejo, Mode::Owned, connection, Offer::Offered)]))
}
#[test]
fn the_flow_shows_the_step_its_row_is_at() {
    for (connection, expected) in [
        (C::NotSetUp, Shown::Details(Choice::Forge(HERE))),
        (C::SettingUp { step: None }, Shown::SettingUp(HERE)),
        (C::SettingUp { step: Some(forges::Step::Prepare) }, Shown::SettingUp(HERE)),
        (C::SetUpFailed { why: b"fetch failed".as_slice().into(), at: None }, Shown::SettingUp(HERE)),
        (C::Connecting, Shown::SettingUp(HERE)),
        (C::Connected { id: 1, login: b"bot".as_slice().into() }, Shown::Ready(HERE)),
        (C::Stopped { reason: b"stopped".as_slice().into() }, Shown::Choose),
    ] {
        assert_eq!(connect::step(Some(Choice::Forge(HERE)), &rows(connection), None), expected);
    }
    assert_eq!(connect::step(None, &rows(C::NotSetUp), None), Shown::Choose);
    assert_eq!(connect::step(Some(Choice::Forge(9)), &rows(C::NotSetUp), None), Shown::Choose);
    assert_eq!(connect::step(Some(Choice::Elsewhere), &State::Unknown, None), Shown::Details(Choice::Elsewhere));
    let demo = forge::family::demo(&demo::answer(b"title"), &w::CEILINGS).unwrap();
    assert_eq!(connect::step(Some(Choice::Forge(demo.forge)), &rows(C::NotSetUp), Some(&demo)), Shown::Choose);
    let github = State::Rows(Box::new([row(2, Kind::GitHub, Mode::Managed, C::NotConnected, Offer::NotOffered)]));
    assert_eq!(connect::step(Some(Choice::Forge(2)), &github, None), Shown::Details(Choice::Forge(2)));
    let fake = State::Rows(Box::new([row(HERE, Kind::Forgejo, Mode::Fake, C::NotSetUp, Offer::Offered)]));
    assert_eq!(connect::step(Some(Choice::Forge(HERE)), &fake, None), Shown::Choose);
}
#[test]
fn choose_offers_github_forgejo_here_and_elsewhere_by_offer() {
    let demo = forge::family::demo(&demo::answer(b"title"), &w::CEILINGS).unwrap();
    let state = State::Rows(Box::new([
        row(demo.forge, Kind::GitHub, Mode::Managed, C::NotConnected, Offer::NotOffered),
        row(2, Kind::GitHub, Mode::Managed, C::NotConnected, Offer::NotOffered),
        row(3, Kind::Forgejo, Mode::Owned, C::NotSetUp, Offer::NotUnderTheBox),
        row(4, Kind::Forgejo, Mode::Owned, C::NotSetUp, Offer::NotOffered),
        row(5, Kind::GitHub, Mode::Fake, C::NotConnected, Offer::NotOffered),
        row(6, Kind::Forgejo, Mode::Managed, C::NotConnected, Offer::NotOffered),
        row(7, Kind::Forgejo, Mode::Owned, C::NotSetUp, Offer::Offered),
    ]));
    // Demo numbers belong to their own fixture; the other six numbers stay distinct.
    assert_eq!(
        connect::choices(&state, Some(&demo)).as_slice(),
        &[Choice::Forge(2), Choice::Forge(3), Choice::Forge(7), Choice::Elsewhere]
    );
    assert_eq!(connect::next_number(&State::Rows(Box::new([]))), 1);
    assert_eq!(connect::next_number(&rows(C::NotSetUp)), 4);
}
fn event(root: &mut crate::Domain, event: Event) -> Queue<Request> {
    event_at(root, event, Time::ZERO)
}
fn event_at(root: &mut crate::Domain, event: Event, now: Time) -> Queue<Request> {
    let mut out = Queue::with_capacity(crate::max_out(&f::limits()));
    let mut env = f::env(&f::limits());
    env.now = now;
    crate::step(root, &env, event, &mut out);
    out
}
fn up(root: &mut crate::Domain, up: Up) -> Queue<Request> {
    event(root, Event::Client { event: client::Event::Up { up } })
}
fn watch(mut out: Queue<Request>) -> Option<Token> {
    while let Some(request) = out.pop() {
        if let Request::Client {
            request: client::Request::Down { down: Down::Open { watch, subject: client::Watchable::Family { .. } } },
        } = request
        {
            return Some(watch);
        }
    }
    None
}
pub(super) fn opened(owner: bool) -> (crate::Domain, Token, Token) {
    let mut root = crate::unlinked(&f::limits(), f::SEED);
    let (me, _) = f::start_accounts(&mut root, List::with_capacity(0));
    let c::WatchEvent::Delivered(c::Delivery::Me(mut frame)) = f::frame(f::PARTY) else { panic!("me") };
    frame.deployment_role = Some(if owner { c::HeldRole::Owner } else { c::HeldRole::Member });
    let out = up(&mut root, Up::Watched { watch: me, event: c::WatchEvent::Delivered(c::Delivery::Me(frame)) });
    (root, me, watch(out).expect("session forges watch"))
}
#[test]
fn the_forges_watch_opens_once_me_is_delivered_for_every_person() {
    for owner in [false, true] {
        let (mut root, me, _) = opened(owner);
        assert!(watch(up(&mut root, Up::Watched { watch: me, event: f::frame(f::PARTY) })).is_none());
        assert!(
            watch(event(&mut root, Event::Went { place: Place::Address(Address::ConnectForge { forge: None }) }))
                .is_none()
        );
    }
}
#[test]
fn a_refused_forges_watch_is_kept_until_the_next_sign_in() {
    let (mut root, me, token) = opened(false);
    up(&mut root, Up::WatchEnded { watch: token, end: c::WatchEnd::NotAllowed });
    let State::Ended = root.forges() else { panic!("refused watch ended") };
    assert!(watch(up(&mut root, Up::Watched { watch: me, event: f::frame(f::PARTY) })).is_none());
    event(&mut root, Event::SignedOut);
    let mut out = event(&mut root, Event::Client { event: client::Event::SignIn { with: client::With::Transport } });
    let mut served = up(&mut root, Up::Served { served: true, versions: None });
    while let Some(request) = served.pop() {
        out.push(request);
    }
    let mut me = None;
    let mut reopened = None;
    while let Some(request) = out.pop() {
        if let Request::Client { request: client::Request::Down { down: Down::Open { watch, subject } } } = request {
            match subject {
                client::Watchable::Jig(c::Subject::Me) => me = Some(watch),
                client::Watchable::Family { .. } => reopened = Some(watch),
                client::Watchable::Jig(_) => {}
            }
        }
    }
    let delivered = watch(up(&mut root, Up::Watched { watch: me.unwrap(), event: f::frame(f::PARTY) }));
    assert!(reopened.or(delivered).is_some());
}
#[test]
fn signing_out_unwatches_the_forges() {
    let (mut root, _, token) = opened(true);
    up(&mut root, Up::Watched { watch: token, event: c::WatchEvent::Opened });
    event(&mut root, Event::SignedOut);
    assert!(root.client().document(forges::handle()).is_none());
    let State::Unknown = root.forges() else { panic!("forges cleared") };
}
#[test]
fn the_forge_half_holds_until_a_forge_of_the_persons_connects() {
    let demo = forge::family::demo(&demo::answer(b"title"), &w::CEILINGS).unwrap();
    for (connection, expected) in [
        (C::NotSetUp, true),
        (C::SettingUp { step: None }, true),
        (C::Connecting, true),
        (C::Connected { id: 1, login: b"bot".as_slice().into() }, false),
    ] {
        assert_eq!(forge::demo::halves(Some(&demo), None, None, &rows(connection)).forge, expected);
    }
    let state = State::Rows(Box::new([row(
        demo.forge,
        Kind::Forgejo,
        Mode::Fake,
        C::Connected { id: 1, login: b"bot".as_slice().into() },
        Offer::NotOffered,
    )]));
    assert!(forge::demo::halves(Some(&demo), None, None, &state).forge);
}
#[test]
fn the_forge_half_does_not_hold_without_the_forges_rows() {
    let demo = forge::family::demo(&demo::answer(b"title"), &w::CEILINGS).unwrap();
    for state in [State::Unknown, State::Ended] {
        assert!(!forge::demo::halves(Some(&demo), None, None, &state).forge);
    }
}
fn setup(root: &mut crate::Domain) -> Queue<Request> {
    f::form_act(root, forge::Act::SetUp { forge: HERE })
}
fn answered(root: &mut crate::Domain, outcome: w::Outcome) -> Queue<Request> {
    let key =
        client::Key { asker: crate::owner::Owner::Forge.asker(), number: root.forge().set_up().unwrap().request.raw() };
    let limits = forge::wire_limits(&f::limits().forge).unwrap();
    let mut out = Queue::with_capacity(crate::max_out(&f::limits()));
    crate::route::told(
        root,
        &f::env(&f::limits()),
        Told::Answered {
            key,
            outcome: client::Outcome::Answered(client::Answered::Family(client::Family {
                family: wire::FORGE,
                version: wire::CLIENT_VERSION,
                bytes: wire::answer(&w::Answer::new(&limits, w::AnswerParts { outcome }).unwrap()),
            })),
        },
        &mut out,
    );
    out
}
#[test]
fn set_up_sends_one_at_a_time() {
    let mut root = f::on_adoption_page();
    let mut out = setup(&mut root);
    let mut sends = 0_u32;
    while let Some(request) = out.pop() {
        if let Request::Client { request: client::Request::Down { down: Down::Send { .. } } } = request {
            sends += 1;
        }
    }
    assert_eq!(sends, 1);
    assert!(setup(&mut root).is_empty());
    assert!(root.forge().form().unwrap().pending.is_none());
}
#[test]
fn a_refused_set_up_keeps_its_refusal() {
    let mut root = f::on_adoption_page();
    setup(&mut root);
    let limits = forge::wire_limits(&f::limits().forge).unwrap();
    answered(
        &mut root,
        w::Outcome::SetUpRefused(
            w::SetUpRefused::new(&limits, w::SetUpRefusedParts { why: w::SetUpRefusal::Busy }).unwrap(),
        ),
    );
    assert_eq!(
        root.forge().set_up().unwrap().answer,
        Some(forges::SetUpAnswer::Outcome(forges::SetUpOutcome::Refused(forges::SetUpRefusal::Busy)))
    );
    assert!(root.forge().set_up().unwrap().progress.is_none());
    assert!(root.forge().form().unwrap().outcome.is_none());
}
#[test]
fn an_accepted_set_up_moves_the_welcome_on_to_model() {
    let mut root = f::on_adoption_page();
    event(
        &mut root,
        Event::Went {
            place: Place::Address(Address::Welcome { step: Step::Forge { forge: Some(Choice::Forge(HERE)) } }),
        },
    );
    setup(&mut root);
    let mut out = answered(&mut root, w::Outcome::SettingUp);
    assert_eq!(
        out.pop(),
        Some(Request::Show { address: Address::Welcome { step: Step::Model { way: None } }, replace: false })
    );
    assert!(out.is_empty());
}

#[test]
fn a_set_up_survives_page_changes_and_parks_out_of_date() {
    let mut root = f::on_adoption_page();
    setup(&mut root);
    let request = root.forge().set_up().unwrap().request;
    event(&mut root, Event::Went { place: Place::Address(Address::ConnectForge { forge: None }) });
    event(&mut root, Event::Forge(forge::Event::Served { served: forge::Served::OutOfDate }));
    assert_eq!(root.forge().set_up().unwrap().request, request);
    assert_eq!(root.forge().set_up().unwrap().progress, Some(skein_lib::RequestProgress::Parked));
    assert!(setup(&mut root).is_empty());
    answered(&mut root, w::Outcome::SettingUp);
    assert!(root.forge().set_up().unwrap().progress.is_none());
    assert_eq!(
        root.forge().set_up().unwrap().answer,
        Some(forges::SetUpAnswer::Outcome(forges::SetUpOutcome::SettingUp))
    );
}

#[test]
fn an_unreadable_set_up_answer_is_unknown_and_allows_another_request() {
    let mut root = f::on_adoption_page();
    setup(&mut root);
    let request = root.forge().set_up().unwrap().request;
    let key = client::Key { asker: crate::owner::Owner::Forge.asker(), number: request.raw() };
    crate::route::told(
        &mut root,
        &f::env(&f::limits()),
        Told::Answered {
            key,
            outcome: client::Outcome::Answered(client::Answered::Family(client::Family {
                family: wire::FORGE,
                version: wire::CLIENT_VERSION,
                bytes: Box::new([255]),
            })),
        },
        &mut Queue::with_capacity(crate::max_out(&f::limits())),
    );
    assert_eq!(
        root.forge().set_up().unwrap().answer,
        Some(forges::SetUpAnswer::Outcome(forges::SetUpOutcome::Unknown))
    );
    assert!(root.forge().set_up().unwrap().progress.is_none());
    assert!(!setup(&mut root).is_empty());
    assert_ne!(root.forge().set_up().unwrap().request, request);
}

#[test]
fn an_out_of_date_link_unwatches_the_forges() {
    let (mut root, _, token) = opened(true);
    up(&mut root, Up::Watched { watch: token, event: c::WatchEvent::Opened });
    crate::route::told(
        &mut root,
        &f::env(&f::limits()),
        Told::Link { link: client::Link::OutOfDate },
        &mut Queue::with_capacity(crate::max_out(&f::limits())),
    );
    assert!(root.client().document(forges::handle()).is_none());
    let State::Unknown = root.forges() else { panic!("forges cleared") };
}

#[test]
fn the_forges_page_lists_the_persons_forges_not_fakes_or_the_demos() {
    use forge::listed;
    let demo = forge::family::demo(&demo::answer(b"title"), &w::CEILINGS).unwrap();
    let connections = [
        C::NotConnected,
        C::NotSetUp,
        C::SettingUp { step: None },
        C::SetUpFailed { why: Box::default(), at: None },
        C::Connecting,
        C::Connected { id: 42, login: Box::default() },
        C::Stopped { reason: Box::default() },
    ];
    for connection in connections {
        let expected = match connection {
            C::NotConnected | C::NotSetUp => false,
            C::SettingUp { .. } | C::SetUpFailed { .. } | C::Connecting | C::Connected { .. } | C::Stopped { .. } => {
                true
            }
        };
        let candidate = row(HERE, Kind::Forgejo, Mode::Owned, connection, Offer::Offered);
        assert_eq!(listed::personal(&candidate, Some(&demo)), expected);
        let mut fake = candidate.clone();
        fake.mode = Mode::Fake;
        assert!(!listed::personal(&fake, None));
        fake.mode = Mode::Owned;
        fake.number = demo.forge;
        assert!(!listed::personal(&fake, Some(&demo)));
    }
    let state = State::Rows(Box::new([
        row(demo.forge, Kind::Forgejo, Mode::Connected, C::Connecting, Offer::NotOffered),
        row(2, Kind::GitHub, Mode::Managed, C::Connecting, Offer::NotOffered),
        row(3, Kind::Forgejo, Mode::Fake, C::Connecting, Offer::NotOffered),
        row(4, Kind::Forgejo, Mode::Owned, C::NotSetUp, Offer::Offered),
        row(5, Kind::Forgejo, Mode::Owned, C::Stopped { reason: Box::default() }, Offer::Offered),
    ]));
    let listed = listed::rows(&state, Some(&demo));
    assert_eq!(listed.len(), 2_u32);
    assert_eq!(listed.as_slice()[0].number, 2_u16);
    assert_eq!(listed.as_slice()[1].number, 5_u16);
    assert!(listed::rows(&State::Unknown, None).is_empty());
    assert!(listed::rows(&State::Ended, None).is_empty());
}
#[test]
fn the_dot_is_red_for_a_forge_that_needs_the_person_else_amber_while_one_sets_up() {
    use forge::listed::{self, Dot};
    for (connection, expected) in [
        (C::NotSetUp, Dot::None),
        (C::Connected { id: 42, login: Box::default() }, Dot::None),
        (C::Connecting, Dot::Amber),
        (C::SettingUp { step: None }, Dot::Amber),
        (C::SetUpFailed { why: Box::default(), at: None }, Dot::Red),
        (C::Stopped { reason: Box::default() }, Dot::Red),
    ] {
        assert_eq!(listed::dot(&rows(connection), None), expected);
    }
    for connections in
        [[C::Connecting, C::Stopped { reason: Box::default() }], [C::Stopped { reason: Box::default() }, C::Connecting]]
    {
        let state = State::Rows(Box::new([
            row(2, Kind::GitHub, Mode::Managed, connections[0].clone(), Offer::NotOffered),
            row(HERE, Kind::Forgejo, Mode::Owned, connections[1].clone(), Offer::Offered),
        ]));
        assert_eq!(listed::dot(&state, None), Dot::Red);
        assert_eq!(
            connect::underway_forge(&state, None).unwrap().number,
            if connections[0] == C::Connecting { 2 } else { HERE }
        );
    }
    let demo = forge::family::demo(&demo::answer(b"title"), &w::CEILINGS).unwrap();
    let state = State::Rows(Box::new([
        row(demo.forge, Kind::GitHub, Mode::Managed, C::Connecting, Offer::NotOffered),
        row(HERE, Kind::Forgejo, Mode::Fake, C::Stopped { reason: Box::default() }, Offer::NotOffered),
    ]));
    assert_eq!(listed::dot(&state, Some(&demo)), Dot::None);
    assert!(connect::underway_forge(&state, Some(&demo)).is_none());
}

fn sent(mut out: Queue<Request>) -> (Token, skein_lib::RequestKey, w::Request) {
    let mut found = None;
    while let Some(request) = out.pop() {
        if let Request::Client {
            request:
                client::Request::Down { down: Down::Send { attempt, key, request: client::Ask::Family { record, .. }, .. } },
        } = request
        {
            assert!(found.is_none(), "one family request");
            found = Some((
                attempt,
                key,
                wire::decode_requesting(&record.bytes, &w::CEILINGS).unwrap().into_parts().request,
            ));
        }
    }
    found.expect("sent family request")
}
fn reply(root: &mut crate::Domain, attempt: Token, outcome: w::Outcome) -> Queue<Request> {
    reply_at(root, attempt, outcome, Time::ZERO)
}
fn reply_at(root: &mut crate::Domain, attempt: Token, outcome: w::Outcome, now: Time) -> Queue<Request> {
    event_at(
        root,
        Event::Client {
            event: client::Event::Up {
                up: Up::Answer {
                    attempt,
                    envelope: skein_lib::RequestEnvelope::Final,
                    answer: Some(client::Answered::Family(client::Family {
                        family: wire::FORGE,
                        version: wire::CLIENT_VERSION,
                        bytes: wire::answer(&w::Answer::new(&w::CEILINGS, w::AnswerParts { outcome }).unwrap()),
                    })),
                    why: None,
                },
            },
        },
        now,
    )
}
fn first(state: forges::FirstState) -> forges::First {
    forges::First {
        playground: false,
        org: b"work".as_slice().into(),
        repository: b"hello".as_slice().into(),
        project: forges::SetUpProject::Existing(1),
        state,
        label: Box::from(b"temper-host".as_slice()),
    }
}
fn with_first(connection: C, state: forges::FirstState) -> State {
    let mut row = row(HERE, Kind::Forgejo, Mode::Owned, connection, Offer::Offered);
    row.first = Some(first(state));
    State::Rows(Box::new([row]))
}
fn deliver_first(
    root: &mut crate::Domain,
    watch: Token,
    connection: w::Connection,
    state: w::FirstState,
) -> Queue<Request> {
    let first = w::FirstRepository::new(
        &w::CEILINGS,
        w::FirstRepositoryParts {
            playground: false,
            org: b"work".as_slice().into(),
            repository: b"hello".as_slice().into(),
            project: w::SetUpProject::Existing(
                w::ExistingProject::new(&w::CEILINGS, w::ExistingProjectParts { project: 1 }).unwrap(),
            ),
            state,
            label: Box::from(b"temper-host".as_slice()),
        },
    )
    .unwrap();
    deliver_first_record(root, watch, connection, first)
}
fn deliver_first_record(
    root: &mut crate::Domain,
    watch: Token,
    connection: w::Connection,
    first: w::FirstRepository,
) -> Queue<Request> {
    let mut rows = List::with_capacity(1);
    rows.push(
        w::ForgeRow::new(
            &w::CEILINGS,
            w::ForgeRowParts {
                number: HERE,
                kind: w::Kind::Forgejo,
                mode: w::Mode::Owned,
                connection,
                offer: w::Offer::Offered,
                first: Some(first),
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
            record: client::Family {
                family: wire::FORGE,
                version: wire::CLIENT_VERSION,
                bytes: wire::forges(&w::Forges::new(&w::CEILINGS, w::ForgesParts { forges: rows }).unwrap()),
            },
        },
    )
}
fn made() -> w::FirstState {
    w::FirstState::Made(w::Made::new(&w::CEILINGS, w::MadeParts { id: 42 }).unwrap())
}
fn connected() -> w::Connection {
    w::Connection::Connected(
        w::Connected::new(&w::CEILINGS, w::ConnectedParts { id: 7, login: b"temper".as_slice().into() }).unwrap(),
    )
}
fn write_setup(root: &mut crate::Domain, field: forges::SetupField, text: &[u8]) {
    assert!(f::form_act(root, forge::Act::SetupWrite { field, text: text.into() }).is_empty());
}
#[test]
fn a_set_up_asks_for_a_new_project_regardless_of_the_current_project() {
    for project in [None, Some(7_u32)] {
        let (mut root, me, _) = opened(true);
        if let Some(project) = project {
            let c::WatchEvent::Delivered(c::Delivery::Me(mut frame)) = f::frame(f::PARTY) else { panic!("me") };
            frame.projects = List::with_capacity(1);
            frame
                .projects
                .push(c::MeProject {
                    project,
                    name: jig_client::contract::DocumentText::complete(
                        jig_client::contract::Text::new(b"Project 1".as_slice().into(), 64).expect("project name"),
                    ),
                    role: c::HeldRole::Owner,
                    pool: None,
                    templates: List::with_capacity(0),
                    creating: None,
                    models: List::with_capacity(0),
                })
                .unwrap();
            up(&mut root, Up::Watched { watch: me, event: c::WatchEvent::Delivered(c::Delivery::Me(frame)) });
        }
        let (_, _, w::Request::SetUp(value)) = sent(setup(&mut root)) else { panic!("setup") };
        assert_eq!((value.org(), value.repository()), (b"work".as_slice(), b"hello".as_slice()));
        let w::SetUpProject::New(value) = value.project() else { panic!("new project") };
        assert_eq!(value.name(), b"hello");
    }
}
#[test]
fn a_set_up_sends_the_fields_the_person_gave() {
    let (mut root, _, _) = opened(true);
    write_setup(&mut root, forges::SetupField::Organization, b"team");
    write_setup(&mut root, forges::SetupField::Repository, b"widget");
    assert_eq!(root.forge().setup_form().text(forges::SetupField::Project), b"widget");
    write_setup(&mut root, forges::SetupField::Project, b"Next");
    write_setup(&mut root, forges::SetupField::Repository, b"app");
    let (_, _, w::Request::SetUp(value)) = sent(setup(&mut root)) else { panic!("setup") };
    assert_eq!((value.org(), value.repository()), (b"team".as_slice(), b"app".as_slice()));
    let w::SetUpProject::New(value) = value.project() else { panic!("new project") };
    assert_eq!(value.name(), b"Next");
    let last = root.forge().set_up().unwrap();
    assert_eq!(
        (&*last.org, &*last.repository, &last.project),
        (b"team".as_slice(), b"app".as_slice(), &forges::SetUpProject::New(b"Next".as_slice().into()))
    );
}
#[test]
fn an_empty_field_is_required_and_sends_nothing() {
    for field in [forges::SetupField::Organization, forges::SetupField::Repository, forges::SetupField::Project] {
        let (mut root, _, _) = opened(true);
        write_setup(&mut root, field, b"");
        assert!(setup(&mut root).is_empty());
        assert!(root.forge().setup_form().required(field));
        assert!(root.forge().set_up().is_none());
    }
}
#[test]
fn set_up_again_sends_the_same_fields() {
    let (mut root, _, watch) = opened(true);
    write_setup(&mut root, forges::SetupField::Organization, b"team");
    let (attempt, _, w::Request::SetUp(original)) = sent(setup(&mut root)) else { panic!("setup") };
    reply(&mut root, attempt, w::Outcome::SettingUp);
    deliver_first(
        &mut root,
        watch,
        w::Connection::Connecting,
        w::FirstState::Failed(
            w::CreateFailed::new(&w::CEILINGS, w::CreateFailedParts { step: w::CreateStep::File }).unwrap(),
        ),
    );
    write_setup(&mut root, forges::SetupField::Organization, b"changed");
    let (_, _, w::Request::SetUp(retry)) = sent(setup(&mut root)) else { panic!("setup") };
    assert_eq!(original, retry);
}
#[test]
fn ready_waits_on_the_first_repository_made() {
    for (state, expected) in [
        (forges::FirstState::Making, Shown::SettingUp(HERE)),
        (forges::FirstState::Made { id: 42 }, Shown::Ready(HERE)),
    ] {
        let rows = with_first(C::Connected { id: 7, login: b"temper".as_slice().into() }, state);
        assert_eq!(connect::step(Some(Choice::Forge(HERE)), &rows, None), expected);
        assert_eq!(connect::personal(&rows, None).unwrap().number, HERE);
    }
    assert_eq!(
        connect::step(Some(Choice::Forge(HERE)), &with_first(C::Connecting, forges::FirstState::Made { id: 42 }), None),
        Shown::SettingUp(HERE)
    );
}
#[test]
fn a_first_repository_failed_shows_its_step() {
    for (step, words) in [
        (forges::CreateStep::Admin, b"Set up failed: checking the admin's token".as_slice()),
        (forges::CreateStep::Org, b"Set up failed: making the organization"),
        (forges::CreateStep::Repository, b"Set up failed: making the repository"),
        (forges::CreateStep::File, b"Set up failed: adding the starter workflow"),
    ] {
        let rows = with_first(C::Connected { id: 7, login: Box::default() }, forges::FirstState::Failed { step });
        assert_eq!(connect::step(Some(Choice::Forge(HERE)), &rows, None), Shown::SettingUp(HERE));
        let row = connect::personal(&rows, None).unwrap();
        assert_eq!(forge::words::forge_connection(row).as_ref(), words);
        assert_eq!(forge::listed::dot(&rows, None), forge::listed::Dot::Red);
    }
    let rows = with_first(C::Connected { id: 7, login: Box::default() }, forges::FirstState::Making);
    assert_eq!(forge::listed::dot(&rows, None), forge::listed::Dot::Amber);
}
#[test]
fn the_client_that_set_up_adopts_the_made_repository_once_on_another_page() {
    let (mut root, _, watch) = opened(true);
    let (attempt, _, _) = sent(setup(&mut root));
    reply(&mut root, attempt, w::Outcome::SettingUp);
    event(&mut root, Event::Went { place: Place::Address(Address::Chats { filter: ChatsFilter::All, start: false }) });
    assert!(deliver_first(&mut root, watch, connected(), w::FirstState::Making).is_empty());
    assert!(deliver_first(&mut root, watch, w::Connection::Connecting, made()).is_empty());
    let out = deliver_first(&mut root, watch, connected(), made());
    let (attempt, project_key, name) = sent_project(out);
    assert_eq!(project_key, forge::start_project_key(HERE, 0));
    assert_eq!(name.as_ref(), b"hello");
    let out = project_reply(
        &mut root,
        attempt,
        c::Answer::Done(c::Outcome::Owner(c::OwnerOutcome::ProjectMade { project: 7 })),
    );
    let (_, key, w::Request::Adopt(adoption)) = sent(out) else { panic!("adoption") };
    assert_eq!(key, forge::first_repository_key(HERE, false));
    assert_eq!((adoption.project(), adoption.forge(), adoption.home(), adoption.ci()), (7, HERE, true, true));
    assert_eq!(
        (adoption.owner(), adoption.name(), adoption.prefix(), adoption.landing()),
        (b"work".as_slice(), b"hello".as_slice(), b"temper/".as_slice(), b"main".as_slice())
    );
    assert_eq!(adoption.role(), &w::Role::Owned);
    assert!(deliver_first(&mut root, watch, connected(), made()).is_empty());
}
#[test]
fn a_client_that_did_not_set_up_adopts_nothing() {
    let (mut root, _, watch) = opened(true);
    assert!(deliver_first(&mut root, watch, connected(), made()).is_empty());
    assert!(root.forge().set_up().is_none());
}
#[test]
fn the_adoption_is_keyed_as_ctls_and_retried_once_without_home() {
    let (mut root, _, watch) = opened(true);
    let (attempt, _, _) = sent(setup(&mut root));
    reply(&mut root, attempt, w::Outcome::SettingUp);
    let out = deliver_first(&mut root, watch, connected(), made());
    let (attempt, _, _) = sent_project(out);
    let out = project_reply(
        &mut root,
        attempt,
        c::Answer::Done(c::Outcome::Owner(c::OwnerOutcome::ProjectMade { project: 7 })),
    );
    let (attempt, _, _) = sent(out);
    let refused = w::Outcome::ForgeRefused(
        w::ForgeRefused::new(&w::CEILINGS, w::ForgeRefusedParts { reason: w::ForgeReason::HomeHeld }).unwrap(),
    );
    let (attempt, key, w::Request::Adopt(adoption)) = sent(reply(&mut root, attempt, refused.clone())) else {
        panic!("retry")
    };
    assert_eq!(key, forge::first_repository_key(HERE, true));
    assert!(!adoption.home());
    let mut out = reply(&mut root, attempt, refused);
    while let Some(request) = out.pop() {
        if let Request::Client { request: client::Request::Down { down: Down::Send { .. } } } = request {
            panic!("no third adoption");
        }
    }
    assert_eq!(
        root.forge().set_up().unwrap().adoption.as_ref().unwrap().outcome,
        Some(forge::Answer::ForgeRefused(forge::Reason::HomeHeld))
    );
    assert!(deliver_first(&mut root, watch, connected(), made()).is_empty());
}
#[test]
fn new_project_and_invalid_refusals_say_their_words() {
    for (why, expected) in [
        (w::SetUpRefusal::NewProject, b"a new project".as_slice()),
        (w::SetUpRefusal::Invalid, b"invalid: the organization, repository or project name is empty or too long"),
    ] {
        let (mut root, _, _) = opened(true);
        let (attempt, _, _) = sent(setup(&mut root));
        reply(
            &mut root,
            attempt,
            w::Outcome::SetUpRefused(w::SetUpRefused::new(&w::CEILINGS, w::SetUpRefusedParts { why }).unwrap()),
        );
        let Some(forges::SetUpAnswer::Outcome(forges::SetUpOutcome::Refused(why))) =
            root.forge().set_up().unwrap().answer
        else {
            panic!("refusal")
        };
        assert_eq!(forge::words::set_up_refusal(why), expected);
    }
}

#[test]
fn set_up_again_on_another_client_uses_the_rows_fields() {
    let (mut root, _, watch) = opened(true);
    let value = w::FirstRepository::new(
        &w::CEILINGS,
        w::FirstRepositoryParts {
            playground: true,
            org: b"playground".as_slice().into(),
            repository: b"widget".as_slice().into(),
            project: w::SetUpProject::Existing(
                w::ExistingProject::new(&w::CEILINGS, w::ExistingProjectParts { project: 7 }).unwrap(),
            ),
            state: w::FirstState::Failed(
                w::CreateFailed::new(&w::CEILINGS, w::CreateFailedParts { step: w::CreateStep::File }).unwrap(),
            ),
            label: Box::from(b"temper-host".as_slice()),
        },
    )
    .unwrap();
    deliver_first_record(&mut root, watch, w::Connection::Connecting, value);
    let (_, _, w::Request::SetUp(value)) = sent(setup(&mut root)) else { panic!("setup") };
    assert_eq!((value.org(), value.repository()), (b"playground".as_slice(), b"widget".as_slice()));
    assert!(value.playground());
    let w::SetUpProject::Existing(project) = value.project() else { panic!("existing project") };
    assert_eq!(project.project(), 7_u32);
}
#[test]
fn signing_out_forgets_the_automatic_adoption_intent() {
    let (mut root, _, _) = opened(true);
    setup(&mut root);
    event(&mut root, Event::SignedOut);
    assert!(root.forge().set_up().is_none());
}

fn sent_project(mut out: Queue<Request>) -> (Token, skein_lib::RequestKey, Box<[u8]>) {
    let mut found = None;
    while let Some(request) = out.pop() {
        if let Request::Client {
            request: client::Request::Down { down: Down::Send { attempt, key, request: client::Ask::Jig(ask), .. } },
        } = request
        {
            let c::Ask::MakeProject { name, policy } = *ask else { panic!("MakeProject") };
            assert_eq!(policy, 1);
            assert!(found.is_none());
            found = Some((attempt, key, Box::from(name.bytes())));
        }
    }
    found.expect("sent MakeProject")
}
fn project_reply(root: &mut crate::Domain, attempt: Token, answer: c::Answer) -> Queue<Request> {
    up(
        root,
        Up::Answer {
            attempt,
            envelope: skein_lib::RequestEnvelope::Final,
            answer: Some(client::Answered::Jig(answer)),
            why: None,
        },
    )
}
#[test]
fn name_taken_keeps_jigs_refusal_and_a_new_name_uses_a_new_key_before_adopting() {
    let (mut root, _, watch) = opened(true);
    let (attempt, _, _) = sent(setup(&mut root));
    reply(&mut root, attempt, w::Outcome::SettingUp);
    let out = deliver_first(&mut root, watch, connected(), made());
    let (attempt, key, _) = sent_project(out);
    let refusal = c::Answer::Refused { refusal: c::Refusal::NameTaken, at: None };
    let mut out = project_reply(&mut root, attempt, refusal.clone());
    while let Some(request) = out.pop() {
        if let Request::Client { request: client::Request::Down { down: Down::Send { .. } } } = request {
            panic!("a refused MakeProject must not send adoption");
        }
    }
    assert!(root.forge().set_up().unwrap().adoption.is_none());
    assert_eq!(
        root.forge().set_up().unwrap().starting.as_ref().unwrap().value.outcome,
        Some(client::Outcome::Answered(client::Answered::Jig(refusal)))
    );
    assert!(deliver_first(&mut root, watch, connected(), made()).is_empty());
    write_setup(&mut root, forges::SetupField::Project, b"Other");
    let (attempt, retry_key, name) = sent_project(f::form_act(&mut root, forge::Act::StartIn { forge: HERE }));
    assert_eq!(name.as_ref(), b"Other");
    assert_ne!(key, retry_key);
    assert_eq!(retry_key, crate::key::named(crate::key::TOOLS, &crate::key::Name::new(b"start-project-3-1").unwrap()));
    let out = project_reply(
        &mut root,
        attempt,
        c::Answer::Done(c::Outcome::Owner(c::OwnerOutcome::ProjectMade { project: 8 })),
    );
    let (_, key, w::Request::Adopt(adoption)) = sent(out) else { panic!("Adopt after ProjectMade") };
    assert_eq!(key, forge::first_repository_key(HERE, false));
    assert_eq!(adoption.project(), 8);
}

fn github_picker_rows() -> State {
    let mut github = row(
        2,
        Kind::GitHub,
        Mode::Connected,
        C::Connected { id: 9, login: b"app[bot]".as_slice().into() },
        Offer::NotOffered,
    );
    github.installations = Box::new([1, 2].map(|id| forges::Installation {
        id,
        account: Some(forges::Account { id, login: b"acme".as_slice().into() }),
        suspended: false,
        stray: false,
    }));
    State::Rows(Box::new([github]))
}
fn picker_opened() -> (crate::Domain, Token, Token, Queue<Request>) {
    let (mut root, me, watch) = opened(true);
    deliver_picker(&mut root, watch, false);
    let out = event(
        &mut root,
        Event::Went { place: Place::Address(Address::ConnectForge { forge: Some(Choice::Forge(2)) }) },
    );
    (root, me, watch, out)
}
fn picker_act(root: &mut crate::Domain, act: forge::picker::Act) -> Queue<Request> {
    f::form_act(root, forge::Act::Picker { forge: 2, act })
}
fn reachable_page() -> w::Outcome {
    let mut rows = List::with_capacity(2);
    for (id, name) in [(10, b"first".as_slice()), (20, b"second".as_slice())] {
        rows.push(
            w::Reached::new(
                &w::CEILINGS,
                w::ReachedParts {
                    id,
                    owner: w::Account::new(&w::CEILINGS, w::AccountParts { id: 1, login: b"acme".as_slice().into() })
                        .unwrap(),
                    name: name.into(),
                    private: id == 20,
                },
            )
            .unwrap(),
        )
        .unwrap();
    }
    w::Outcome::Page(w::Page::new(&w::CEILINGS, w::PageParts { rows, more: true }).unwrap())
}
fn banner_project(root: &mut crate::Domain, me: Token, name: &[u8]) -> Queue<Request> {
    let c::WatchEvent::Delivered(c::Delivery::Me(mut frame)) = f::frame(f::PARTY) else { panic!("me") };
    frame.projects = List::with_capacity(1);
    frame
        .projects
        .push(c::MeProject {
            project: 7,
            name: c::DocumentText::complete(c::Text::new(name.into(), 256).unwrap()),
            role: c::HeldRole::Owner,
            pool: None,
            templates: List::with_capacity(0),
            creating: None,
            models: List::with_capacity(0),
        })
        .unwrap();
    up(root, Up::Watched { watch: me, event: c::WatchEvent::Delivered(c::Delivery::Me(frame)) })
}
#[test]
fn github_without_an_accepted_installation_is_connecting() {
    let mut state = github_picker_rows();
    let State::Rows(rows) = &mut state else { panic!("rows") };
    rows[0].installations = Box::default();
    assert_eq!(connect::step(Some(Choice::Forge(2)), &state, None), Shown::SettingUp(2));
}
#[test]
fn ready_reads_pages_and_accounts_with_fresh_keys_and_follows_the_chosen_name_until_written() {
    use forge::picker::Act;
    let (mut root, _, _, out) = picker_opened();
    let (attempt, first_key, w::Request::Reachable(read)) = sent(out) else { panic!("Reachable") };
    assert_eq!((read.forge(), *read.installation(), read.page()), (2, Some(1), 1));
    assert!(root.forge().picker(2).unwrap().answer.is_none());
    reply(&mut root, attempt, reachable_page());
    assert_eq!(root.forge().picker(2).unwrap().name.as_ref(), b"first");
    let written = root.forge().picker(2).unwrap().written;
    picker_act(&mut root, Act::Choose(1));
    assert_eq!(root.forge().picker(2).unwrap().name.as_ref(), b"second");
    assert!(root.forge().picker(2).unwrap().written > written);
    picker_act(&mut root, Act::Write(b"mine".as_slice().into()));
    picker_act(&mut root, Act::Choose(0));
    assert_eq!(root.forge().picker(2).unwrap().name.as_ref(), b"mine");
    let (attempt, next_key, w::Request::Reachable(read)) = sent(picker_act(&mut root, Act::Page(2))) else {
        panic!("next")
    };
    assert_eq!(read.page(), 2);
    assert_ne!(next_key, first_key);
    reply(&mut root, attempt, reachable_page());
    let (_, _, w::Request::Reachable(read)) = sent(picker_act(&mut root, Act::Account(2))) else { panic!("account") };
    assert_eq!((*read.installation(), read.page()), (Some(2), 1));
}
#[test]
fn ready_makes_a_project_then_adopts_into_it_and_reload_dedup_checks_its_banner_name() {
    for (banner, retry) in [(b"second".as_slice(), false), (b"old".as_slice(), true)] {
        let (mut root, me, _, out) = picker_opened();
        let (attempt, _, _) = sent(out);
        reply(&mut root, attempt, reachable_page());
        picker_act(&mut root, forge::picker::Act::Choose(1));
        let (attempt, key, name) = sent_project(picker_act(&mut root, forge::picker::Act::Adopt));
        assert_eq!(key, forge::start_project_key(2, 0));
        assert_eq!(name.as_ref(), b"second");
        assert!(root.forge().picker(2).unwrap().pending());
        assert!(picker_act(&mut root, forge::picker::Act::Adopt).is_empty());
        let out = project_reply(
            &mut root,
            attempt,
            c::Answer::Done(c::Outcome::Owner(c::OwnerOutcome::ProjectMade { project: 7 })),
        );
        let mut out = out;
        while let Some(request) = out.pop() {
            if let Request::Client { request: client::Request::Down { down: Down::Send { .. } } } = request {
                panic!("wait for the project's banner name");
            }
        }
        let out = banner_project(&mut root, me, banner);
        if retry {
            let (_, key, name) = sent_project(out);
            assert_eq!(key, forge::start_project_key(2, 1));
            assert_eq!(name.as_ref(), b"second");
            assert!(root.forge().picker(2).unwrap().adoption.is_none());
        } else {
            let (_, key, w::Request::Adopt(adoption)) = sent(out) else { panic!("adopt") };
            assert_eq!(key, forge::first_repository_key(2, false));
            assert_eq!(adoption.project(), 7);
            assert_eq!(
                (adoption.owner(), adoption.name(), adoption.home()),
                (b"acme".as_slice(), b"second".as_slice(), true)
            );
            assert!(root.forge().set_up().is_none(), "Ready keeps its start in the picker");
        }
    }
}
#[test]
fn not_listed_installs_with_back_and_changed_installations_reread_page_one() {
    let (mut root, _, watch, out) = picker_opened();
    let (attempt, _, _) = sent(out);
    reply(&mut root, attempt, reachable_page());
    let (attempt, _, _) = sent(picker_act(&mut root, forge::picker::Act::Page(2)));
    reply(&mut root, attempt, reachable_page());
    let back = b"/forges/connect?forge=2".as_slice();
    let (_, _, w::Request::Install(install)) =
        sent(f::form_act(&mut root, forge::Act::Install { forge: 2, back: back.into() }))
    else {
        panic!("Install")
    };
    assert_eq!(install.back().as_deref(), Some(back));
    let (_, _, w::Request::Reachable(read)) = sent(deliver_picker(&mut root, watch, true)) else { panic!("reread") };
    assert_eq!(read.page(), 1);
}

fn deliver_picker(root: &mut crate::Domain, watch: Token, changed: bool) -> Queue<Request> {
    let mut installations = List::with_capacity(2);
    for id in [1, 2] {
        let login: &[u8] = if changed && id == 2 { b"changed" } else { b"acme" };
        installations
            .push(
                w::Installation::new(
                    &w::CEILINGS,
                    w::InstallationParts {
                        id,
                        account: Some(
                            w::Account::new(&w::CEILINGS, w::AccountParts { id, login: login.into() }).unwrap(),
                        ),
                        suspended: false,
                        stray: false,
                    },
                )
                .unwrap(),
            )
            .unwrap();
    }
    let mut rows = List::with_capacity(1);
    rows.push(
        w::ForgeRow::new(
            &w::CEILINGS,
            w::ForgeRowParts {
                number: 2,
                kind: w::Kind::Github,
                mode: w::Mode::Connected,
                connection: connected(),
                offer: w::Offer::NotOffered,
                first: None,
                installations,
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
            record: client::Family {
                family: wire::FORGE,
                version: wire::CLIENT_VERSION,
                bytes: wire::forges(&w::Forges::new(&w::CEILINGS, w::ForgesParts { forges: rows }).unwrap()),
            },
        },
    )
}

#[test]
fn a_picker_read_forgotten_by_a_restarted_service_gets_a_fresh_key() {
    let (mut root, _, _, out) = picker_opened();
    let (attempt, first_key, _) = sent(out);
    let (_, next_key, w::Request::Reachable(read)) = sent(reply(&mut root, attempt, w::Outcome::Unknown)) else {
        panic!("fresh Reachable after unknown")
    };
    assert_ne!(first_key, next_key);
    assert_eq!(read.page(), 1);
}

fn busy() -> w::Outcome {
    w::Outcome::ConnectRefused(
        w::ConnectRefused::new(&w::CEILINGS, w::ConnectRefusedParts { why: w::ForgeRefusal::Busy }).unwrap(),
    )
}
fn fire_at(root: &mut crate::Domain, now: Time) -> Queue<Request> {
    let mut env = f::env(&f::limits());
    env.now = now;
    let mut out = Queue::with_capacity(crate::max_out(&env.limits));
    crate::fire(root, &env, &mut out);
    crate::take(root, &env, &mut out);
    out
}
fn busy_words(root: &crate::Domain) {
    let picker = root.forge().picker(2).unwrap();
    assert!(picker.selected().is_none());
    assert_eq!(picker.answer, Some(forge::github::Answer::Refused(w::ForgeRefusal::Busy)));
    assert_eq!(
        forge::words::app_refusal(picker.answer.clone().unwrap(), 2).unwrap().as_ref(),
        b"temper is busy with forge 2: try again when it ends."
    );
}
#[test]
fn a_busy_picker_waits_then_reads_rows_with_a_fresh_key() {
    let (mut root, _, _, out) = picker_opened();
    let (attempt, key, _) = sent(out);
    assert_eq!(f::sends(&mut reply(&mut root, attempt, busy())), 0, "no immediate retry");
    busy_words(&root);
    let at = Time::ZERO.checked_add(Duration::from_secs(2)).unwrap();
    assert_eq!(forge::next_deadline(root.forge()), Some(at));
    assert!(crate::next_deadline(&root).unwrap() <= at, "the root includes the forge's timer");
    assert_eq!(f::sends(&mut fire_at(&mut root, Time::from_nanos(1_999_999_999))), 0);
    busy_words(&root);
    let (attempt, next_key, w::Request::Reachable(read)) = sent(fire_at(&mut root, at)) else {
        panic!("fresh Reachable after bounded wait")
    };
    assert_ne!(key, next_key);
    assert_eq!((read.forge(), *read.installation(), read.page()), (2, Some(1), 1));
    assert_eq!(forge::next_deadline(root.forge()), None);
    reply_at(&mut root, attempt, reachable_page(), at);
    let picker = root.forge().picker(2).unwrap();
    let Some(forge::github::Answer::Page(page)) = &picker.answer else { panic!("rows after busy") };
    assert_eq!(page.rows.len(), 2);
    assert_eq!(picker.selected().unwrap().name.as_ref(), b"first");
    assert_eq!(forge::next_deadline(root.forge()), None);
}
#[test]
fn a_picker_busy_past_three_rereads_keeps_its_words_without_more_reads() {
    let (mut root, _, _, out) = picker_opened();
    let (mut attempt, mut key, _) = sent(out);
    let mut now = Time::ZERO;
    for reread in 0_u8..=3 {
        assert_eq!(f::sends(&mut reply_at(&mut root, attempt, busy(), now)), 0);
        busy_words(&root);
        assert_eq!(root.forge().picker(2).unwrap().retry_exhausted(), reread == 3);
        if reread == 3 {
            break;
        }
        let at = now.checked_add(Duration::from_secs(2)).unwrap();
        assert_eq!(forge::next_deadline(root.forge()), Some(at));
        let (next_attempt, next_key, w::Request::Reachable(_)) = sent(fire_at(&mut root, at)) else {
            panic!("bounded reread")
        };
        assert_ne!(key, next_key);
        attempt = next_attempt;
        key = next_key;
        now = at;
    }
    assert_eq!(forge::next_deadline(root.forge()), None);
    for seconds in [2, 20, 200] {
        let later = now.checked_add(Duration::from_secs(seconds)).unwrap();
        assert_eq!(f::sends(&mut fire_at(&mut root, later)), 0);
        busy_words(&root);
        assert_eq!(forge::next_deadline(root.forge()), None);
    }
    let now = Time::from_nanos(206_000_000_000);
    event_at(&mut root, Event::Went { place: Place::Address(Address::Forges) }, now);
    let mut out = event_at(
        &mut root,
        Event::Went { place: Place::Address(Address::ConnectForge { forge: Some(Choice::Forge(2)) }) },
        now,
    );
    assert_eq!(f::sends(&mut out), 0, "navigation does not reset the retry bound");
    assert_eq!(forge::next_deadline(root.forge()), None);
    busy_words(&root);
    let (attempt, _, _) = sent(event_at(
        &mut root,
        Event::Act {
            action: crate::Action::Forge(forge::Act::Picker { forge: 2, act: forge::picker::Act::Account(2) }),
        },
        now,
    ));
    reply_at(&mut root, attempt, busy(), now);
    assert_eq!(forge::next_deadline(root.forge()), now.checked_add(Duration::from_secs(2)));
}
#[test]
fn a_hidden_picker_waits_until_ready_is_shown_again_before_retrying() {
    let (mut root, _, _, out) = picker_opened();
    let (attempt, _, _) = sent(out);
    reply(&mut root, attempt, busy());
    event(&mut root, Event::Went { place: Place::Address(Address::Forges) });
    assert_eq!(forge::next_deadline(root.forge()), None);
    let now = Time::from_nanos(20_000_000_000);
    assert_eq!(f::sends(&mut fire_at(&mut root, now)), 0);
    let mut out = event_at(
        &mut root,
        Event::Went { place: Place::Address(Address::ConnectForge { forge: Some(Choice::Forge(2)) }) },
        now,
    );
    assert_eq!(f::sends(&mut out), 0);
    busy_words(&root);
    let at = now.checked_add(Duration::from_secs(2)).unwrap();
    assert_eq!(forge::next_deadline(root.forge()), Some(at));
    let (_, _, w::Request::Reachable(_)) = sent(fire_at(&mut root, at)) else { panic!("visible reread") };
}
#[test]
fn refused_picker_reads_keep_no_access_and_not_found_words_without_rows() {
    for (reason, answer, words) in [
        (
            w::ForgeReason::NoAccess,
            forge::github::Answer::NoAccess,
            b"GitHub refused temper's read of the App's repositories: the App may be suspended. See Forges.".as_slice(),
        ),
        (
            w::ForgeReason::NotFound,
            forge::github::Answer::NotFound,
            b"GitHub no longer has this installation: install the App again.".as_slice(),
        ),
    ] {
        let (mut root, _, _, out) = picker_opened();
        let (attempt, _, _) = sent(out);
        reply(&mut root, attempt, reachable_page());
        let (attempt, _, _) = sent(picker_act(&mut root, forge::picker::Act::Account(1)));
        assert_eq!(
            f::sends(&mut reply(
                &mut root,
                attempt,
                w::Outcome::ForgeRefused(w::ForgeRefused::new(&w::CEILINGS, w::ForgeRefusedParts { reason }).unwrap())
            )),
            0
        );
        assert_eq!(forge::next_deadline(root.forge()), None);
        assert_eq!(f::sends(&mut fire_at(&mut root, Time::from_nanos(20_000_000_000))), 0);
        let picker = root.forge().picker(2).unwrap();
        assert_eq!(picker.answer, Some(answer.clone()));
        assert!(picker.selected().is_none(), "the old page is gone");
        assert_eq!(forge::words::app_refusal(answer, 2).unwrap().as_ref(), words);
    }
}

#[test]
fn github_adoption_advances_only_the_welcome() {
    for welcome in [true, false] {
        let (mut root, me, _, out) = picker_opened();
        let (attempt, _, _) = sent(out);
        reply(&mut root, attempt, reachable_page());
        if welcome {
            event(
                &mut root,
                Event::Went {
                    place: Place::Address(Address::Welcome { step: Step::Forge { forge: Some(Choice::Forge(2)) } }),
                },
            );
        }
        let (attempt, _, _) = sent_project(picker_act(&mut root, forge::picker::Act::Adopt));
        project_reply(
            &mut root,
            attempt,
            c::Answer::Done(c::Outcome::Owner(c::OwnerOutcome::ProjectMade { project: 7 })),
        );
        let (attempt, _, _) = sent(banner_project(&mut root, me, b"first"));
        let outcome = w::Outcome::Adopted(
            w::Adopted::new(
                &w::CEILINGS,
                w::AdoptedParts {
                    forge: 2,
                    repository: 7,
                    owner: b"acme".as_slice().into(),
                    name: b"first".as_slice().into(),
                    role: w::Role::Owned,
                    home: true,
                    kinds: w::Kinds::new(&w::CEILINGS, w::KindsParts { skein_bools: [true; 9] }).unwrap(),
                    protection: w::Protection::Unknown,
                },
            )
            .unwrap(),
        );
        let out = reply(&mut root, attempt, outcome);
        let mut moves = 0_u32;
        for request in &out {
            if let Request::Show { address, replace } = request {
                assert!(welcome);
                assert_eq!(address, &Address::Welcome { step: Step::Model { way: None } });
                assert!(!replace);
                moves = moves.checked_add(1).unwrap();
            }
        }
        assert_eq!(moves, u32::from(welcome));
        if !welcome {
            assert_eq!(root.pages().address(), &Address::ConnectForge { forge: Some(Choice::Forge(2)) });
        }
        let Some(forge::Answer::Adopted(_)) = &root.forge().picker(2).unwrap().adoption.as_ref().unwrap().outcome
        else {
            panic!("successful adoption remains available to Ready")
        };
    }
}
