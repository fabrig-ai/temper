use super::fixtures;
use crate::forge::{
    self, family,
    forges::{Connection, Kind, Mode, Offer, Row, SetUpAnswer, SetUpOutcome, SetUpRefusal, Step, ask, rows, watch},
    words,
};
use jig_client::{Answered, Ask, Family, Watchable, contract as c};
use skein_lib::List;
use temper_client_protocol_wire::{self as wire, v1 as w};
fn limits() -> w::Limits {
    forge::wire_limits(&fixtures::limits().forge).unwrap()
}
fn answer(outcome: w::Outcome) -> Family {
    Family {
        family: wire::FORGE,
        version: wire::CLIENT_VERSION,
        bytes: wire::answer(&w::Answer::new(&limits(), w::AnswerParts { outcome }).unwrap()),
    }
}
#[test]
fn set_up_is_a_deployment_request_and_forges_a_deployment_watch() {
    let Ask::Family { scope, record } = ask(
        3,
        b"work".as_slice().into(),
        b"hello".as_slice().into(),
        forge::forges::SetUpProject::Existing(1),
        false,
        &fixtures::limits().forge,
    )
    .unwrap() else {
        panic!("family")
    };
    assert_eq!(scope, c::Scope::Deployment);
    assert_eq!((record.family, record.version), (wire::FORGE, wire::CLIENT_VERSION));
    let w::Request::SetUp(request) = wire::decode_requesting(&record.bytes, &limits()).unwrap().into_parts().request
    else {
        panic!("set up")
    };
    assert_eq!(request.forge(), 3);
    let Watchable::Family { scope, window, record } = watch(&fixtures::limits().forge).unwrap() else {
        panic!("family")
    };
    assert_eq!(scope, c::Scope::Deployment);
    assert_eq!(window, c::Window { rows: 0, after: None });
    assert_eq!(*wire::decode_watching(&record.bytes, &limits()).unwrap().watch(), w::Watch::Forges);
}
#[test]
fn set_up_accepts_only_its_outcomes_and_the_shared_unknown() {
    for (wire, expected) in [
        (w::Outcome::SettingUp, SetUpOutcome::SettingUp),
        (w::Outcome::Unknown, SetUpOutcome::Unknown),
        (
            w::Outcome::SetUpRefused(
                w::SetUpRefused::new(&limits(), w::SetUpRefusedParts { why: w::SetUpRefusal::NotOffered }).unwrap(),
            ),
            SetUpOutcome::Refused(SetUpRefusal::NotOffered),
        ),
        (
            w::Outcome::SetUpRefused(
                w::SetUpRefused::new(&limits(), w::SetUpRefusedParts { why: w::SetUpRefusal::NotUnderTheBox }).unwrap(),
            ),
            SetUpOutcome::Refused(SetUpRefusal::NotUnderTheBox),
        ),
        (
            w::Outcome::SetUpRefused(
                w::SetUpRefused::new(&limits(), w::SetUpRefusedParts { why: w::SetUpRefusal::Busy }).unwrap(),
            ),
            SetUpOutcome::Refused(SetUpRefusal::Busy),
        ),
    ] {
        let record = answer(wire);
        assert_eq!(family::set_up_answer(&record, &limits()), Some(expected));
        assert_eq!(
            forge::forges::answer(&Answered::Family(record), &fixtures::limits().forge),
            Some(SetUpAnswer::Outcome(expected))
        );
    }
    let adoption = answer(w::Outcome::ForgeRefused(
        w::ForgeRefused::new(&limits(), w::ForgeRefusedParts { reason: w::ForgeReason::NoSuchForge }).unwrap(),
    ));
    assert_eq!(family::set_up_answer(&adoption, &limits()), None);
    let link =
        jig_client_protocol_link::Limits::new(fixtures::limits().client.domain, fixtures::limits().client.exchanges)
            .unwrap();
    for outcome in [
        w::Outcome::SettingUp,
        w::Outcome::SetUpRefused(
            w::SetUpRefused::new(&limits(), w::SetUpRefusedParts { why: w::SetUpRefusal::Busy }).unwrap(),
        ),
    ] {
        assert_eq!(family::answer(&answer(outcome), &limits(), &link), None);
    }
    let mut record = answer(w::Outcome::SettingUp);
    record.bytes = Box::from(b"bad".as_slice());
    assert_eq!(family::set_up_answer(&record, &limits()), None);
}
fn cases() -> Box<[(w::Connection, Connection, &'static [u8])]> {
    Box::new([
        (w::Connection::NotConnected, Connection::NotConnected, b"Not connected"),
        (w::Connection::NotSetUp, Connection::NotSetUp, b"Not set up"),
        (
            w::Connection::SettingUp(w::SettingUp::new(&limits(), w::SettingUpParts { step: None }).unwrap()),
            Connection::SettingUp { step: None },
            b"Setting up",
        ),
        (
            w::Connection::SettingUp(
                w::SettingUp::new(&limits(), w::SettingUpParts { step: Some(w::SetUpStep::Fetch) }).unwrap(),
            ),
            Connection::SettingUp { step: Some(Step::Fetch) },
            b"Setting up: fetching",
        ),
        (
            w::Connection::SettingUp(
                w::SettingUp::new(&limits(), w::SettingUpParts { step: Some(w::SetUpStep::Prepare) }).unwrap(),
            ),
            Connection::SettingUp { step: Some(Step::Prepare) },
            b"Setting up: preparing",
        ),
        (
            w::Connection::SettingUp(
                w::SettingUp::new(&limits(), w::SettingUpParts { step: Some(w::SetUpStep::Run) }).unwrap(),
            ),
            Connection::SettingUp { step: Some(Step::Run) },
            b"Setting up: starting",
        ),
        (
            w::Connection::SetUpFailed(
                w::SetUpFailed::new(
                    &limits(),
                    w::SetUpFailedParts { why: Box::from(b"fetch failed".as_slice()), at: None },
                )
                .unwrap(),
            ),
            Connection::SetUpFailed { why: Box::from(b"fetch failed".as_slice()), at: None },
            b"Set up failed: fetch failed",
        ),
        (w::Connection::Connecting, Connection::Connecting, b"Connecting"),
        (
            w::Connection::Connected(
                w::Connected::new(&limits(), w::ConnectedParts { id: 42, login: Box::from(b"bot".as_slice()) })
                    .unwrap(),
            ),
            Connection::Connected { id: 42, login: Box::from(b"bot".as_slice()) },
            b"Connected as bot (42)",
        ),
        (
            w::Connection::Stopped(
                w::Stopped::new(&limits(), w::StoppedParts { reason: Box::from(b"retired".as_slice()) }).unwrap(),
            ),
            Connection::Stopped { reason: Box::from(b"retired".as_slice()) },
            b"Stopped: retired",
        ),
    ])
}
fn fixture_wire_owned(m: Mode) -> Option<w::OwnedForge> {
    if m == Mode::Owned {
        Some(
            w::OwnedForge::new(
                &limits(),
                w::OwnedForgeParts {
                    forgejo: b"17.0.0".as_slice().into(),
                    runner: Some(b"13.2.0".as_slice().into()),
                    password: b"/fake/forge/person_password".as_slice().into(),
                },
            )
            .unwrap(),
        )
    } else {
        None
    }
}
fn fixture_owned(m: Mode) -> Option<forge::forges::Owned> {
    if m == Mode::Owned {
        Some(forge::forges::Owned {
            forgejo: b"17.0.0".as_slice().into(),
            runner: Some(b"13.2.0".as_slice().into()),
            password: b"/fake/forge/person_password".as_slice().into(),
        })
    } else {
        None
    }
}
#[test]
fn every_connection_decodes_into_the_forges_words() {
    let mut records = List::with_capacity(10);
    let mut expected = List::with_capacity(10);
    for (index, (connection, state, words)) in cases().into_iter().enumerate() {
        let (kind, mode, offer, k, m, o) = match index % 4 {
            0 => (
                w::Kind::Forgejo,
                w::Mode::Connected,
                w::Offer::NotOffered,
                Kind::Forgejo,
                Mode::Connected,
                Offer::NotOffered,
            ),
            1 => (w::Kind::Github, w::Mode::Managed, w::Offer::Offered, Kind::GitHub, Mode::Managed, Offer::Offered),
            2 => (
                w::Kind::Forgejo,
                w::Mode::Owned,
                w::Offer::NotUnderTheBox,
                Kind::Forgejo,
                Mode::Owned,
                Offer::NotUnderTheBox,
            ),
            _ => (w::Kind::Github, w::Mode::Fake, w::Offer::NotOffered, Kind::GitHub, Mode::Fake, Offer::NotOffered),
        };
        let number = u16::try_from(index + 1).unwrap();
        records
            .push(
                w::ForgeRow::new(
                    &limits(),
                    w::ForgeRowParts {
                        number,
                        kind,
                        mode,
                        connection,
                        offer,
                        first: None,
                        installations: List::with_capacity(0),
                        ended: None,
                        app: None,
                        web: Some(b"https://forge.example".as_slice().into()),
                        repositories: {
                            let mut adopted = List::with_capacity(1);
                            adopted
                                .push(
                                    w::Adoption::new(
                                        &limits(),
                                        w::AdoptionParts {
                                            owner: b"acme".as_slice().into(),
                                            name: b"app".as_slice().into(),
                                            project: 7,
                                        },
                                    )
                                    .unwrap(),
                                )
                                .unwrap();
                            adopted
                        },
                        owned: fixture_wire_owned(m),
                        admin: false,
                        read_only: false,
                        ended_stop: None,
                    },
                )
                .unwrap(),
            )
            .unwrap();
        let row = Row {
            number,
            kind: k,
            mode: m,
            connection: state,
            offer: o,
            first: None,
            installations: Box::new([]),
            ended: None,
            app: None,
            web: Some(b"https://forge.example".as_slice().into()),
            repositories: Box::new([forge::forges::Adoption {
                owner: b"acme".as_slice().into(),
                name: b"app".as_slice().into(),
                project: 7,
            }]),
            owned: fixture_owned(m),
            admin: false,
            read_only: false,
            ended_stop: None,
        };
        assert_eq!(words::forge_connection(&row).as_ref(), words);
        expected.push(row).unwrap();
    }
    let record = Family {
        family: wire::FORGE,
        version: wire::CLIENT_VERSION,
        bytes: wire::forges(&w::Forges::new(&limits(), w::ForgesParts { forges: records }).unwrap()),
    };
    assert_eq!(rows(&record, &fixtures::limits().forge).unwrap().as_ref(), expected.as_slice());
    for bytes in [b"bad".as_slice(), &record.bytes[..record.bytes.len() - 1]] {
        assert!(rows(&Family { bytes: bytes.into(), ..record.clone() }, &fixtures::limits().forge).is_none());
    }
    assert!(rows(&Family { version: wire::CLIENT_VERSION + 1, ..record.clone() }, &fixtures::limits().forge).is_none());
    assert!(rows(&Family { family: wire::FORGE + 1, ..record }, &fixtures::limits().forge).is_none());
}
#[test]
fn the_offer_drives_the_manual_words_and_setup_actions() {
    let mut row = Row {
        number: 3,
        kind: Kind::Forgejo,
        mode: Mode::Owned,
        connection: Connection::NotSetUp,
        offer: Offer::Offered,
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
    };
    assert_eq!(words::forge_set_up_action(&row), Some(b"Set up a Forgejo here".as_slice()));
    row.offer = Offer::NotUnderTheBox;
    assert_eq!(words::forge_set_up_action(&row), None);
    assert_eq!(words::forge_connection(&row).as_ref(), b"Not set up: run `temper forge fetch 3`, `temper forge prepare 3`, `temper forge run 3 forgejo` and `temper forge run 3 runner`, then restart `temper serve`");
    row.connection = Connection::SetUpFailed { why: Box::from(b"why".as_slice()), at: None };
    row.offer = Offer::Offered;
    assert_eq!(words::forge_set_up_action(&row), Some(b"Set up again".as_slice()));
    for (kind, text) in [(Kind::Forgejo, b"Forgejo".as_slice()), (Kind::GitHub, b"GitHub")] {
        assert_eq!(words::forge_kind(kind), text);
    }
    for (mode, text) in [
        (Mode::Connected, b"connected".as_slice()),
        (Mode::Managed, b"managed"),
        (Mode::Owned, b"owned"),
        (Mode::Fake, b"fake"),
    ] {
        assert_eq!(words::forge_mode(mode), text);
    }
    for (outcome, text) in [
        (SetUpOutcome::SettingUp, b"setting up".as_slice()),
        (SetUpOutcome::Refused(SetUpRefusal::NotOffered), b"not offered"),
        (SetUpOutcome::Refused(SetUpRefusal::NotUnderTheBox), b"not under the box"),
        (SetUpOutcome::Refused(SetUpRefusal::Busy), b"busy"),
        (SetUpOutcome::Unknown, b"unknown"),
    ] {
        assert_eq!(words::set_up_outcome(outcome), text);
    }
    row.connection = Connection::Stopped { reason: Box::from([255u8]) };
    assert_eq!(words::forge_connection(&row).as_ref(), "Stopped: �".as_bytes());
}

#[test]
fn an_out_of_order_document_is_refused() {
    let mut forges = List::with_capacity(2);
    for number in [3, 1] {
        forges
            .push(
                w::ForgeRow::new(
                    &limits(),
                    w::ForgeRowParts {
                        number,
                        kind: w::Kind::Forgejo,
                        mode: w::Mode::Owned,
                        connection: w::Connection::NotSetUp,
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
    let record = Family {
        family: wire::FORGE,
        version: wire::CLIENT_VERSION,
        bytes: wire::forges(&w::Forges::new(&limits(), w::ForgesParts { forges }).unwrap()),
    };
    assert!(rows(&record, &fixtures::limits().forge).is_none());
}

#[test]
fn setup_carries_each_field_and_new_project_names_without_semantic_form_checks() {
    use forge::forges::SetUpProject;
    for project in
        [SetUpProject::Existing(7), SetUpProject::New(b"Next".as_slice().into()), SetUpProject::New(Box::default())]
    {
        let Ask::Family { record, .. } =
            ask(3, Box::default(), b"widget".as_slice().into(), project.clone(), false, &fixtures::limits().forge)
                .unwrap()
        else {
            panic!("family")
        };
        let w::Request::SetUp(request) =
            wire::decode_requesting(&record.bytes, &limits()).unwrap().into_parts().request
        else {
            panic!("setup")
        };
        assert_eq!(request.forge(), 3);
        assert_eq!(request.org(), b"");
        assert_eq!(request.repository(), b"widget");
        match (project, request.project()) {
            (SetUpProject::Existing(number), w::SetUpProject::Existing(value)) => assert_eq!(number, value.project()),
            (SetUpProject::New(name), w::SetUpProject::New(value)) => assert_eq!(name.as_ref(), value.name()),
            _ => panic!("project changed"),
        }
    }
}
#[test]
fn first_repository_decodes_each_state_project_and_creation_step() {
    use forge::forges::{CreateStep, First, FirstState, SetUpProject};
    let mut states = List::with_capacity(6);
    states.push((w::FirstState::Making, FirstState::Making)).unwrap();
    states
        .push((
            w::FirstState::Made(w::Made::new(&limits(), w::MadeParts { id: 42 }).unwrap()),
            FirstState::Made { id: 42 },
        ))
        .unwrap();
    for (wire, domain) in [
        (w::CreateStep::Admin, CreateStep::Admin),
        (w::CreateStep::Org, CreateStep::Org),
        (w::CreateStep::Repository, CreateStep::Repository),
        (w::CreateStep::File, CreateStep::File),
    ] {
        states
            .push((
                w::FirstState::Failed(w::CreateFailed::new(&limits(), w::CreateFailedParts { step: wire }).unwrap()),
                FirstState::Failed { step: domain },
            ))
            .unwrap();
    }
    for (wire_state, state) in states.into_boxed() {
        for new in [false, true] {
            let project = if new {
                w::SetUpProject::New(
                    w::NewProject::new(&limits(), w::NewProjectParts { name: b"Next".as_slice().into() }).unwrap(),
                )
            } else {
                w::SetUpProject::Existing(
                    w::ExistingProject::new(&limits(), w::ExistingProjectParts { project: 7 }).unwrap(),
                )
            };
            let first = w::FirstRepository::new(
                &limits(),
                w::FirstRepositoryParts {
                    playground: false,
                    org: b"team".as_slice().into(),
                    repository: b"widget".as_slice().into(),
                    project,
                    state: wire_state.clone(),
                    label: Box::from(b"docker_1.x-y".as_slice()),
                },
            )
            .unwrap();
            let mut forges = List::with_capacity(1);
            forges
                .push(
                    w::ForgeRow::new(
                        &limits(),
                        w::ForgeRowParts {
                            number: 3,
                            kind: w::Kind::Forgejo,
                            mode: w::Mode::Owned,
                            connection: w::Connection::Connecting,
                            offer: w::Offer::NotOffered,
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
            let record = Family {
                family: wire::FORGE,
                version: wire::CLIENT_VERSION,
                bytes: wire::forges(&w::Forges::new(&limits(), w::ForgesParts { forges }).unwrap()),
            };
            assert_eq!(
                rows(&record, &fixtures::limits().forge).unwrap()[0].first,
                Some(First {
                    playground: false,
                    org: b"team".as_slice().into(),
                    repository: b"widget".as_slice().into(),
                    project: if new { SetUpProject::New(b"Next".as_slice().into()) } else { SetUpProject::Existing(7) },
                    state,
                    label: Box::from(b"docker_1.x-y".as_slice()),
                })
            );
        }
    }
}

#[test]
fn demo_start_outcomes_fit_neither_adopt_nor_setup() {
    let link =
        jig_client_protocol_link::Limits::new(fixtures::limits().client.domain, fixtures::limits().client.exchanges)
            .unwrap();
    for outcome in [
        w::Outcome::Running,
        w::Outcome::StartRefused(
            w::StartRefused::new(&limits(), w::StartRefusedParts { why: w::StartRefusal::NotOffered }).unwrap(),
        ),
        w::Outcome::StartFailed(
            w::StartFailed::new(&limits(), w::StartFailedParts { why: w::StartFailure::Account }).unwrap(),
        ),
    ] {
        let record = answer(outcome);
        assert_eq!(family::set_up_answer(&record, &limits()), None);
        assert_eq!(family::answer(&record, &limits(), &link), None);
    }
}

#[test]
fn installation_accounts_flags_and_connect_ends_survive_client_decode() {
    use forge::forges::{Account, Installation};
    let limits = limits();
    let mut installations = List::with_capacity(2);
    for (id, account, suspended, stray) in [
        (
            11,
            Some(w::Account::new(&limits, w::AccountParts { id: 111, login: Box::from(b"acme".as_slice()) }).unwrap()),
            true,
            false,
        ),
        (22, None, false, true),
    ] {
        installations
            .push(w::Installation::new(&limits, w::InstallationParts { id, account, suspended, stray }).unwrap())
            .unwrap();
    }
    let mut forges = List::with_capacity(1);
    forges
        .push(
            w::ForgeRow::new(
                &limits,
                w::ForgeRowParts {
                    number: 1,
                    kind: w::Kind::Github,
                    mode: w::Mode::Connected,
                    connection: w::Connection::NotConnected,
                    offer: w::Offer::NotOffered,
                    first: None,
                    installations,
                    ended: Some(
                        w::ConnectStopped::new(
                            &limits,
                            w::ConnectStoppedParts { why: Box::from(b"the pasted value changed".as_slice()) },
                        )
                        .unwrap(),
                    ),
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
    let bytes = wire::forges(&w::Forges::new(&limits, w::ForgesParts { forges }).unwrap());
    let rows = family::forges(&bytes, &limits).unwrap();
    assert_eq!(
        rows[0].installations.as_ref(),
        [
            Installation {
                id: 11,
                account: Some(Account { id: 111, login: Box::from(b"acme".as_slice()) }),
                suspended: true,
                stray: false
            },
            Installation { id: 22, account: None, suspended: false, stray: true },
        ]
    );
    assert_eq!(rows[0].ended.as_deref(), Some(b"the pasted value changed".as_slice()));
}

#[test]
fn admin_form_and_typed_stops_survive_client_decode() {
    use forge::forges::{AdminStop, SetupStage};
    let limits = limits();
    for (stop, expected) in [
        (None, None),
        (Some(w::AdminStop::NotAdmin), Some(AdminStop::NotAdmin)),
        (Some(w::AdminStop::Unmarked), Some(AdminStop::Unmarked)),
        (Some(w::AdminStop::Other), Some(AdminStop::Other)),
        (
            Some(w::AdminStop::Refused(
                w::AdminRefused::new(&limits, w::AdminRefusedParts { status: 403, stage: w::SetupStage::Repository })
                    .unwrap(),
            )),
            Some(AdminStop::Refused { status: 403, stage: SetupStage::Repository }),
        ),
    ] {
        for admin in [false, true] {
            let mut forges = List::with_capacity(1);
            forges
                .push(
                    w::ForgeRow::new(
                        &limits,
                        w::ForgeRowParts {
                            number: 1,
                            kind: w::Kind::Forgejo,
                            mode: w::Mode::Managed,
                            connection: w::Connection::NotConnected,
                            offer: w::Offer::NotOffered,
                            first: None,
                            installations: List::with_capacity(0),
                            ended: None,
                            app: None,
                            web: None,
                            repositories: List::with_capacity(0),
                            owned: None,
                            admin,
                            read_only: false,
                            ended_stop: stop.clone(),
                        },
                    )
                    .unwrap(),
                )
                .unwrap();
            let bytes = wire::forges(&w::Forges::new(&limits, w::ForgesParts { forges }).unwrap());
            let rows = family::forges(&bytes, &limits).unwrap();
            assert_eq!(rows[0].admin, admin);
            assert_eq!(rows[0].ended_stop, expected);
        }
    }
}
