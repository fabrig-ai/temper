//! Public forge admission, without protocol or credential bytes.
#![expect(clippy::disallowed_methods, reason = "ordinary test assertions inspect captured requests")]
#![expect(clippy::match_like_matches_macro, reason = "explicit test patterns; matches is banned by the subset")]
use super::*;
use core::num::NonZeroU64;
use jig_domain_core::Carried;
use jig_domain_people::Role as PartyRole;

const VALUE: Carried = Carried { number: 1_u64 << 63_u32, digest: [17; 32] };
fn github(kind: Kind, mode: Mode) -> Domain {
    Domain::new(
        &LIMITS,
        4,
        skein_forge_client::Config { namespace: Box::from(b"test".as_slice()), writers: Box::new([]) },
        &[],
        &[Configured {
            owned: None,
            admin_connect: false,
            bot: Box::new([]),
            git: Box::new([]),
            web: None,
            read_only: false,
            number: 1,
            kind,
            mode,
            asked: false,
        }],
        false,
    )
    .unwrap()
}
fn connect(forge: u16) -> Ask {
    Ask::Connect { forge, how: How::GitHubApp { app: NonZeroU64::new(7).unwrap() } }
}
fn application(
    d: &mut Domain,
    number: u64,
    ask: Ask,
    role: Option<PartyRole>,
    carried: Option<Carried>,
) -> Box<[Request]> {
    outputs(
        d,
        Event::Application {
            request: Token::new(number),
            token: Token::new(number),
            retention: Duration::from_secs(60),
            ask,
            role,
            carried,
        },
    )
}
fn refused(d: &mut Domain, ask: Ask, role: Option<PartyRole>, carried: Option<Carried>, why: ForgeRefusal) {
    let install = if let Ask::Install { .. } = ask { true } else { false };
    let reading = if let Ask::Reachable { .. } = ask { true } else { false };
    let out = application(d, 1, ask, role, carried);
    let expected = if reading {
        Answered::Reachable(ReachableAnswer::Refused(why))
    } else if install {
        Answered::Install(InstallAnswer::Refused(why))
    } else {
        Answered::Connect(ConnectAnswer::Refused(why))
    };
    assert_eq!(d.application_answer(Token::new(1)), Some(expected));
    assert_eq!(
        out.iter()
            .filter(|r| if let Request::CarriedUnused { number } = r { *number == VALUE.number } else { false })
            .count(),
        usize::from(carried.is_some())
    );
    assert!(!out.iter().any(|r| {
        if let Request::Reachable { .. }
        | Request::Connect { .. }
        | Request::Manifest { .. }
        | Request::Install { .. } = r
        {
            true
        } else {
            false
        }
    }));
}
#[test]
fn connect_requires_the_deployment_owner_and_a_configured_real_github_and_key() {
    for role in
        [None, Some(PartyRole::Member), Some(PartyRole::Maintainer), Some(PartyRole::Observer), Some(PartyRole::Owner)]
    {
        let mut d = github(Kind::GitHub, Mode::Connected);
        if role == Some(PartyRole::Owner) {
            assert!(
                application(&mut d, 1, connect(1), role, Some(VALUE))
                    .iter()
                    .any(|r| if let Request::Connect { .. } = r { true } else { false })
            );
        } else {
            refused(&mut d, connect(1), role, Some(VALUE), ForgeRefusal::Role);
        }
    }
    refused(
        &mut github(Kind::GitHub, Mode::Connected),
        connect(2),
        Some(PartyRole::Owner),
        Some(VALUE),
        ForgeRefusal::NoSuchForge,
    );
    for (kind, mode) in [(Kind::Forgejo, Mode::Connected), (Kind::GitHub, Mode::Fake)] {
        refused(&mut github(kind, mode), connect(1), Some(PartyRole::Owner), Some(VALUE), ForgeRefusal::NotTaken);
    }
    refused(&mut github(Kind::GitHub, Mode::Connected), connect(1), Some(PartyRole::Owner), None, ForgeRefusal::NoKey);
    for state in [Connection::Connecting, Connection::Connected { id: 9, login: Box::from(b"app[bot]".as_slice()) }] {
        let mut d = github(Kind::GitHub, Mode::Connected);
        outputs(&mut d, Event::Connection { forge: 1, state });
        refused(&mut d, connect(1), Some(PartyRole::Owner), Some(VALUE), ForgeRefusal::Connected);
    }
}
#[test]
fn admitted_connect_marks_connecting_answers_then_erases_once_at_its_end() {
    for success in [false, true] {
        let mut d = github(Kind::GitHub, Mode::Connected);
        let out = application(&mut d, 1, connect(1), Some(PartyRole::Owner), Some(VALUE));
        assert_eq!(d.connection(1), Connection::Connecting);
        assert!(out.iter().any(|r| {
            if let Request::Connect { forge: 1, request, how: How::GitHubApp { app }, carried } = r {
                *request == Token::new(1) && app.get() == 7 && *carried == VALUE
            } else {
                false
            }
        }));
        assert_eq!(d.application_answer(Token::new(1)), None);
        outputs(&mut d, Event::ConnectAnswered { forge: 1, request: Token::new(1), answer: ConnectAnswer::Connecting });
        assert_eq!(d.application_answer(Token::new(1)), Some(Answered::Connect(ConnectAnswer::Connecting)));
        let out = application(&mut d, 2, connect(1), Some(PartyRole::Owner), Some(VALUE));
        assert_eq!(
            d.application_answer(Token::new(2)),
            Some(Answered::Connect(ConnectAnswer::Refused(ForgeRefusal::Busy)))
        );
        assert_eq!(out.iter().filter(|r| if let Request::CarriedUnused { .. } = r { true } else { false }).count(), 1);
        let out = outputs(
            &mut d,
            Event::ConnectEnded { stop: None, why: None, forge: 1, request: Token::new(1), connected: success },
        );
        assert_eq!(
            out.iter()
                .filter(|r| if let Request::EraseCarried { number } = r { *number == VALUE.number } else { false })
                .count(),
            1
        );
        assert!(
            outputs(
                &mut d,
                Event::ConnectEnded { stop: None, why: None, forge: 1, request: Token::new(1), connected: success }
            )
            .is_empty()
        );
        if !success {
            assert_eq!(d.connection(1), Connection::NotConnected);
        }
    }
}
#[test]
fn install_requires_connection_and_defers_its_answer_until_the_run_finishes() {
    let ask = Ask::Install { forge: 1, back: Some(Box::from(b"/forges".as_slice())) };
    refused(
        &mut github(Kind::GitHub, Mode::Connected),
        ask.clone(),
        Some(PartyRole::Owner),
        Some(VALUE),
        ForgeRefusal::NotConnected,
    );
    refused(&mut github(Kind::GitHub, Mode::Connected), ask.clone(), None, Some(VALUE), ForgeRefusal::Role);
    refused(
        &mut github(Kind::GitHub, Mode::Connected),
        Ask::Install { forge: 2, back: None },
        Some(PartyRole::Owner),
        Some(VALUE),
        ForgeRefusal::NoSuchForge,
    );
    refused(
        &mut github(Kind::Forgejo, Mode::Connected),
        ask.clone(),
        Some(PartyRole::Owner),
        Some(VALUE),
        ForgeRefusal::NotTaken,
    );
    for answer in [
        InstallAnswer::Connecting,
        InstallAnswer::Link(Box::from(b"http://fake/apps/app/installations/new".as_slice())),
    ] {
        let mut d = github(Kind::GitHub, Mode::Connected);
        outputs(
            &mut d,
            Event::Connection {
                forge: 1,
                state: Connection::Connected { id: 9, login: Box::from(b"app[bot]".as_slice()) },
            },
        );
        let out = application(&mut d, 1, ask.clone(), Some(PartyRole::Owner), Some(VALUE));
        assert!(out.iter().any(|r| if let Request::Install { forge: 1, back: Some(back), .. } = r {
            back.as_ref() == b"/forges"
        } else {
            false
        }));
        assert_eq!(out.iter().filter(|r| if let Request::CarriedUnused { .. } = r { true } else { false }).count(), 1);
        assert_eq!(d.application_answer(Token::new(1)), None);
        application(&mut d, 2, ask.clone(), Some(PartyRole::Owner), None);
        assert_eq!(
            d.application_answer(Token::new(2)),
            Some(Answered::Install(InstallAnswer::Refused(ForgeRefusal::Busy)))
        );
        outputs(
            &mut d,
            Event::InstallAnswered { stopped: None, forge: 1, request: Token::new(1), answer: answer.clone() },
        );
        assert_eq!(d.application_answer(Token::new(1)), Some(Answered::Install(answer)));
    }
}

#[test]
fn a_busy_service_refusal_names_the_carried_value_unused_once() {
    let mut d = github(Kind::GitHub, Mode::Connected);
    application(&mut d, 1, connect(1), Some(PartyRole::Owner), Some(VALUE));
    let out = outputs(
        &mut d,
        Event::ConnectAnswered { forge: 1, request: Token::new(1), answer: ConnectAnswer::Refused(ForgeRefusal::Busy) },
    );
    assert_eq!(out.iter().filter(|r| if let Request::CarriedUnused { .. } = r { true } else { false }).count(), 1);
    assert!(!out.iter().any(|r| if let Request::EraseCarried { .. } = r { true } else { false }));
    assert_eq!(d.connection(1), Connection::NotConnected);
}

const STOP_REASON: &[u8] = b"the pasted value changed";
fn stopped_connect() -> Domain {
    let mut d = github(Kind::GitHub, Mode::Connected);
    application(&mut d, 1, connect(1), Some(PartyRole::Owner), Some(VALUE));
    outputs(
        &mut d,
        Event::ConnectEnded {
            stop: None,
            forge: 1,
            request: Token::new(1),
            connected: false,
            why: Some(Box::from(STOP_REASON)),
        },
    );
    d
}
#[test]
fn stopped_connect_marks_its_end_until_the_next_admitted_run() {
    let mut d = stopped_connect();
    assert_eq!(d.forges().rows[0].ended.as_deref(), Some(STOP_REASON));
    let out = application(&mut d, 2, connect(1), Some(PartyRole::Owner), Some(VALUE));
    assert_eq!(d.forges().rows[0].ended, None);
    assert!(out.contains(&Request::MarkedForges));
}
#[test]
fn refusals_neither_set_nor_clear_the_last_connect_end() {
    let mut d = stopped_connect();
    refused(&mut d, connect(1), None, Some(VALUE), ForgeRefusal::Role);
    assert_eq!(d.forges().rows[0].ended.as_deref(), Some(STOP_REASON));
    let mut d = github(Kind::GitHub, Mode::Connected);
    refused(&mut d, connect(1), None, Some(VALUE), ForgeRefusal::Role);
    assert_eq!(d.forges().rows[0].ended, None);
}
#[test]
fn connected_and_the_next_install_clear_a_stopped_install_end() {
    for next_install in [false, true] {
        let mut d = github(Kind::GitHub, Mode::Connected);
        let state = Connection::Connected { id: 9, login: Box::from(b"app[bot]".as_slice()) };
        outputs(&mut d, Event::Connection { forge: 1, state: state.clone() });
        application(&mut d, 1, Ask::Install { forge: 1, back: None }, Some(PartyRole::Owner), None);
        let out = outputs(
            &mut d,
            Event::InstallAnswered {
                forge: 1,
                request: Token::new(1),
                answer: InstallAnswer::Connecting,
                stopped: Some(Box::from(STOP_REASON)),
            },
        );
        assert_eq!(d.forges().rows[0].ended.as_deref(), Some(STOP_REASON));
        assert!(out.contains(&Request::MarkedForges));
        let out = if next_install {
            application(&mut d, 2, Ask::Install { forge: 1, back: None }, Some(PartyRole::Owner), None)
        } else {
            outputs(&mut d, Event::Connection { forge: 1, state })
        };
        assert_eq!(d.forges().rows[0].ended, None);
        assert!(out.contains(&Request::MarkedForges));
    }
}
#[test]
fn connect_end_reasons_are_bounded_without_splitting_utf8() {
    // A two-byte character straddles the 256-byte boundary.
    let mut reason = skein_lib::Writer::new(CONNECT_WHY_BYTES + 1);
    reason.put(&[b'x'; CONNECT_WHY_BYTES - 1]).unwrap();
    reason.put("é".as_bytes()).unwrap();
    let mut d = github(Kind::GitHub, Mode::Connected);
    application(&mut d, 1, connect(1), Some(PartyRole::Owner), Some(VALUE));
    let out = outputs(
        &mut d,
        Event::ConnectEnded {
            stop: None,
            forge: 1,
            request: Token::new(1),
            connected: false,
            why: Some(reason.finish()),
        },
    );
    assert_eq!(d.forges().rows[0].ended.as_deref(), Some([b'x'; CONNECT_WHY_BYTES - 1].as_slice()));
    assert!(out.contains(&Request::MarkedForges));
}
#[test]
fn installation_updates_mark_rows_in_id_order_and_deletion_drops_one() {
    let mut limits = LIMITS;
    limits.client.repositories = 3; // The three configured forge fixtures below.
    limits.output = 128; // Includes the client's larger repository burst.
    let mut d = Domain::new(
        &limits,
        4,
        skein_forge_client::Config { namespace: Box::from(b"test".as_slice()), writers: Box::new([]) },
        &[],
        &[
            Configured {
                admin_connect: false,
                bot: Box::new([]),
                owned: None,
                git: Box::new([]),
                web: None,
                read_only: false,
                number: 1,
                kind: Kind::GitHub,
                mode: Mode::Connected,
                asked: false,
            },
            Configured {
                admin_connect: false,
                bot: Box::new([]),
                owned: None,
                git: Box::new([]),
                web: None,
                read_only: false,
                number: 2,
                kind: Kind::GitHub,
                mode: Mode::Connected,
                asked: false,
            },
            Configured {
                admin_connect: false,
                bot: Box::new([]),
                owned: None,
                git: Box::new([]),
                web: None,
                read_only: false,
                number: 3,
                kind: Kind::Forgejo,
                mode: Mode::Connected,
                asked: false,
            },
        ],
        false,
    )
    .unwrap();
    d.connection_limits(3, 4);
    let info = Installation { account: None, suspended: false, stray: false };
    for (forge, installation) in [(1, 22), (2, 33), (1, 11), (3, 44)] {
        let out = outputs(&mut d, Event::Installation { forge, installation, state: Some(info.clone()) });
        assert_eq!(out.as_ref(), [Request::MarkedForges]);
    }
    let rows = d.forges().rows;
    assert_eq!(rows[0].installations.as_ref(), [(11, info.clone()), (22, info.clone())]);
    assert_eq!(rows[1].installations.as_ref(), [(33, info.clone())]);
    assert!(rows[2].installations.is_empty());
    let out = outputs(&mut d, Event::Installation { forge: 1, installation: 11, state: None });
    assert_eq!(out.as_ref(), [Request::MarkedForges]);
    assert_eq!(d.forges().rows[0].installations.as_ref(), [(22, info)]);
}

fn manifest_ask(forge: u16) -> Ask {
    Ask::Connect { forge, how: How::GitHubManifest { back: Some(Box::from(b"/forges".as_slice())) } }
}
#[test]
fn manifests_need_no_key_erase_carried_and_release_after_the_link() {
    for carried in [None, Some(VALUE)] {
        let mut d = github(Kind::GitHub, Mode::Connected);
        let out = application(&mut d, 1, manifest_ask(1), Some(PartyRole::Owner), carried);
        assert!(out.contains(&Request::Manifest {
            forge: 1,
            request: Token::new(1),
            back: Some(Box::from(b"/forges".as_slice()))
        }));
        assert_eq!(out.contains(&Request::CarriedUnused { number: VALUE.number }), carried.is_some());
        assert_eq!(d.connection(1), Connection::NotConnected);
        let answer = ConnectAnswer::Link(Box::from(b"http://temper.test/forges/1/github/manifest?state=s".as_slice()));
        outputs(&mut d, Event::ConnectAnswered { forge: 1, request: Token::new(1), answer: answer.clone() });
        assert_eq!(d.application_answer(Token::new(1)), Some(Answered::Connect(answer)));
        assert!(
            application(&mut d, 2, manifest_ask(1), Some(PartyRole::Owner), None)
                .iter()
                .any(|r| if let Request::Manifest { .. } = r { true } else { false })
        );
    }
}
#[test]
fn manifest_refusals_follow_admission_order_and_erase_carried() {
    for (role, forge, kind, mode, held, connection, why) in [
        (None, 2, Kind::Forgejo, Mode::Fake, false, Connection::NotConnected, ForgeRefusal::Role),
        (
            Some(PartyRole::Owner),
            2,
            Kind::Forgejo,
            Mode::Fake,
            false,
            Connection::NotConnected,
            ForgeRefusal::NoSuchForge,
        ),
        (
            Some(PartyRole::Owner),
            1,
            Kind::Forgejo,
            Mode::Connected,
            false,
            Connection::Connected { id: 9, login: Box::from(b"app[bot]".as_slice()) },
            ForgeRefusal::NotTaken,
        ),
        (Some(PartyRole::Owner), 1, Kind::GitHub, Mode::Fake, false, Connection::NotConnected, ForgeRefusal::NotTaken),
        (
            Some(PartyRole::Owner),
            1,
            Kind::GitHub,
            Mode::Connected,
            true,
            Connection::Connected { id: 9, login: Box::from(b"app[bot]".as_slice()) },
            ForgeRefusal::Busy,
        ),
        (
            Some(PartyRole::Owner),
            1,
            Kind::GitHub,
            Mode::Connected,
            false,
            Connection::Connecting,
            ForgeRefusal::Connected,
        ),
        (
            Some(PartyRole::Owner),
            1,
            Kind::GitHub,
            Mode::Connected,
            false,
            Connection::Connected { id: 9, login: Box::from(b"app[bot]".as_slice()) },
            ForgeRefusal::Connected,
        ),
    ] {
        for carried in [None, Some(VALUE)] {
            let mut d = github(kind, mode);
            if held {
                application(&mut d, 2, manifest_ask(1), Some(PartyRole::Owner), None);
            }
            outputs(&mut d, Event::Connection { forge: 1, state: connection.clone() });
            refused(&mut d, manifest_ask(forge), role, carried, why);
        }
    }
}
#[test]
fn app_way_is_volatile_and_only_shown_for_connected_github() {
    let connected = Connection::Connected { id: 9, login: Box::from(b"app[bot]".as_slice()) };
    let mut d = github(Kind::GitHub, Mode::Connected);
    outputs(&mut d, Event::Connection { forge: 1, state: connected.clone() });
    assert_eq!(d.forges().rows[0].app, None);
    let mut pasted = github(Kind::GitHub, Mode::Connected);
    application(&mut pasted, 1, connect(1), Some(PartyRole::Owner), Some(VALUE));
    outputs(
        &mut pasted,
        Event::ConnectEnded { stop: None, forge: 1, request: Token::new(1), connected: true, why: None },
    );
    outputs(&mut pasted, Event::Connection { forge: 1, state: connected.clone() });
    assert_eq!(pasted.forges().rows[0].app, Some(AppWay::Pasted));
    let mut made = github(Kind::GitHub, Mode::Connected);
    outputs(&mut made, Event::AppRecorded { forge: 1, app: AppWay::Manifest });
    outputs(&mut made, Event::Connection { forge: 1, state: connected });
    assert_eq!(made.forges().rows[0].app, Some(AppWay::Manifest));
    outputs(&mut made, Event::Connection { forge: 1, state: Connection::NotConnected });
    assert_eq!(made.forges().rows[0].app, None);
}

fn reachable(installation: Option<u64>) -> Ask {
    Ask::Reachable { forge: 1, installation, page: 1 }
}
fn reachable_domain(kind: Kind, mode: Mode) -> Domain {
    let mut d = github(kind, mode);
    outputs(
        &mut d,
        Event::Connection { forge: 1, state: Connection::Connected { id: 9, login: b"bot".as_slice().into() } },
    );
    outputs(
        &mut d,
        Event::Installation {
            forge: 1,
            installation: 11,
            state: Some(Installation {
                account: Some((10, b"acme".as_slice().into())),
                suspended: false,
                stray: false,
            }),
        },
    );
    d
}
#[test]
fn reachable_requires_the_deployment_owner_a_real_configured_connected_forge_and_accepted_installation() {
    for role in [None, Some(PartyRole::Member), Some(PartyRole::Maintainer), Some(PartyRole::Observer)] {
        refused(&mut reachable_domain(Kind::GitHub, Mode::Connected), reachable(None), role, None, ForgeRefusal::Role);
    }
    refused(
        &mut reachable_domain(Kind::GitHub, Mode::Connected),
        Ask::Reachable { forge: 2, installation: None, page: 1 },
        Some(PartyRole::Owner),
        None,
        ForgeRefusal::NoSuchForge,
    );
    for kind in [Kind::GitHub, Kind::Forgejo] {
        refused(
            &mut reachable_domain(kind, Mode::Fake),
            reachable(None),
            Some(PartyRole::Owner),
            None,
            ForgeRefusal::NotTaken,
        );
        refused(
            &mut github(kind, Mode::Connected),
            reachable(None),
            Some(PartyRole::Owner),
            None,
            ForgeRefusal::NotConnected,
        );
    }
    for (installation, state) in [(Some(12), None), (Some(11), Some((true, false))), (None, Some((false, true)))] {
        let mut d = reachable_domain(Kind::GitHub, Mode::Connected);
        if let Some((suspended, stray)) = state {
            outputs(
                &mut d,
                Event::Installation {
                    forge: 1,
                    installation: 11,
                    state: Some(Installation { account: None, suspended, stray }),
                },
            );
        }
        refused(&mut d, reachable(installation), Some(PartyRole::Owner), None, ForgeRefusal::NotTaken);
    }
    let mut d = reachable_domain(Kind::GitHub, Mode::Connected);
    outputs(&mut d, Event::Installation { forge: 1, installation: 11, state: None });
    refused(&mut d, reachable(None), Some(PartyRole::Owner), None, ForgeRefusal::NotTaken);
    refused(
        &mut reachable_domain(Kind::Forgejo, Mode::Connected),
        reachable(Some(11)),
        Some(PartyRole::Owner),
        None,
        ForgeRefusal::NotTaken,
    );
    for kind in [Kind::Forgejo, Kind::GitHub] {
        let mut d = reachable_domain(kind, Mode::Connected);
        assert!(application(&mut d, 1, reachable(None), Some(PartyRole::Owner), None).iter().any(|r| {
            if let Request::Reachable { installation, page: 1, .. } = r {
                *installation == if kind == Kind::GitHub { Some(11) } else { None }
            } else {
                false
            }
        }));
    }
}
#[test]
fn reachable_runs_beside_a_writer_refuses_a_second_and_keeps_its_page_only_in_memory() {
    let mut d = reachable_domain(Kind::GitHub, Mode::Connected);
    assert!(
        application(&mut d, 1, Ask::Install { forge: 1, back: None }, Some(PartyRole::Owner), None)
            .iter()
            .any(|r| if let Request::Install { .. } = r { true } else { false })
    );
    assert!(
        application(&mut d, 2, reachable(None), Some(PartyRole::Owner), None)
            .iter()
            .any(|r| if let Request::Reachable { .. } = r { true } else { false })
    );
    application(&mut d, 3, reachable(None), Some(PartyRole::Owner), None);
    assert_eq!(
        d.application_answer(Token::new(3)),
        Some(Answered::Reachable(ReachableAnswer::Refused(ForgeRefusal::Busy)))
    );
    let page = ReachableAnswer::Page(Page {
        rows: Box::new([Reached {
            id: 42,
            owner_id: 10,
            owner: b"acme".as_slice().into(),
            name: b"app".as_slice().into(),
            private: true,
        }]),
        more: true,
    });
    let out = outputs(&mut d, Event::ReachableAnswered { forge: 1, request: Token::new(2), answer: page.clone() });
    assert_eq!(d.application_answer(Token::new(2)), Some(Answered::Reachable(page.clone())));
    assert_eq!(d.application_answer(Token::new(2)), Some(Answered::Reachable(page)));
    assert!(out.iter().all(|r| if let Request::Save { .. } = r { false } else { true }));
    assert_eq!(reachable_domain(Kind::GitHub, Mode::Connected).application_answer(Token::new(2)), None);
    assert!(
        application(&mut d, 4, reachable(None), Some(PartyRole::Owner), None)
            .iter()
            .any(|r| if let Request::Reachable { .. } = r { true } else { false })
    );
}
#[test]
fn read_only_app_refuses_a_manifest_but_accepts_a_paste() {
    let mut d = github(Kind::GitHub, Mode::Connected);
    d.configured.get_mut(&1).unwrap().read_only = true;
    refused(
        &mut d,
        Ask::Connect { forge: 1, how: How::GitHubManifest { back: None } },
        Some(PartyRole::Owner),
        None,
        ForgeRefusal::NotTaken,
    );
    assert!(
        application(&mut d, 2, connect(1), Some(PartyRole::Owner), Some(VALUE))
            .iter()
            .any(|r| if let Request::Connect { .. } = r { true } else { false })
    );
}

fn admin_ask(forge: u16, first: Option<FirstAsk>) -> Ask {
    Ask::Connect { forge, how: How::ForgejoAdmin { first } }
}
fn admin_domain() -> Domain {
    let mut d = github(Kind::Forgejo, Mode::Managed);
    d.configured.get_mut(&1).unwrap().admin_connect = true;
    d.configured.get_mut(&1).unwrap().bot = b"temper-bot".as_slice().into();
    d
}
fn first_ask() -> FirstAsk {
    FirstAsk {
        org: b"work".as_slice().into(),
        repository: b"hello".as_slice().into(),
        project: SetUpProject::Existing(1),
        label: b"docker".as_slice().into(),
    }
}
#[test]
fn admin_connect_requires_the_owner_before_checking_the_forge_or_connection() {
    for role in [None, Some(PartyRole::Member), Some(PartyRole::Maintainer), Some(PartyRole::Observer)] {
        for carried in [None, Some(VALUE)] {
            let mut d = github(Kind::Forgejo, Mode::Owned);
            outputs(
                &mut d,
                Event::Connection { forge: 1, state: Connection::Connected { id: 9, login: b"bot".as_slice().into() } },
            );
            refused(&mut d, admin_ask(1, None), role, carried, ForgeRefusal::Role);
        }
    }
}
#[test]
fn admin_connect_requires_the_string_managed_forgejo_form_before_checking_state() {
    for (forge, kind, mode, admin_connect) in [
        (2, Kind::Forgejo, Mode::Managed, true),
        (1, Kind::Forgejo, Mode::Owned, false),
        (1, Kind::Forgejo, Mode::Fake, false),
        (1, Kind::Forgejo, Mode::Connected, false),
        (1, Kind::Forgejo, Mode::Managed, false),
        (1, Kind::GitHub, Mode::Managed, true),
    ] {
        for carried in [None, Some(VALUE)] {
            let mut d = github(kind, mode);
            d.configured.get_mut(&1).unwrap().admin_connect = admin_connect;
            outputs(&mut d, Event::Connection { forge: 1, state: Connection::Connecting });
            refused(&mut d, admin_ask(forge, None), Some(PartyRole::Owner), carried, ForgeRefusal::NotTaken);
        }
    }
}
#[test]
fn admin_connect_refuses_connected_connecting_and_setting_up_before_busy_or_no_key() {
    for state in [
        Connection::Connected { id: 9, login: b"bot".as_slice().into() },
        Connection::Connecting,
        Connection::SettingUp { step: Some(SetUpStep::Prepare) },
    ] {
        for carried in [None, Some(VALUE)] {
            let mut d = admin_domain();
            application(&mut d, 2, admin_ask(1, None), Some(PartyRole::Owner), Some(VALUE));
            outputs(&mut d, Event::Connection { forge: 1, state: state.clone() });
            refused(&mut d, admin_ask(1, None), Some(PartyRole::Owner), carried, ForgeRefusal::Connected);
        }
    }
}
#[test]
fn admin_connect_refuses_a_run_on_the_forge_before_no_key() {
    for carried in [None, Some(VALUE)] {
        let mut d = admin_domain();
        application(&mut d, 2, admin_ask(1, None), Some(PartyRole::Owner), Some(VALUE));
        outputs(&mut d, Event::Connection { forge: 1, state: Connection::NotConnected });
        refused(&mut d, admin_ask(1, None), Some(PartyRole::Owner), carried, ForgeRefusal::Busy);
    }
}
#[test]
fn admin_connect_requires_a_carried_token() {
    for first in [None, Some(FirstAsk { org: b"temper-bot".as_slice().into(), ..first_ask() })] {
        refused(&mut admin_domain(), admin_ask(1, first), Some(PartyRole::Owner), None, ForgeRefusal::NoKey);
    }
}
#[test]
fn admin_connect_refuses_a_first_organization_named_as_the_bot() {
    let mut d = admin_domain();
    let before = d.forges();
    let first = FirstAsk { org: b"temper-bot".as_slice().into(), ..first_ask() };
    refused(&mut d, admin_ask(1, Some(first)), Some(PartyRole::Owner), Some(VALUE), ForgeRefusal::FirstIsBot);
    assert_eq!(d.forges(), before);
    assert!(d.first.is_empty());
}
#[test]
fn admin_connect_compares_the_bot_s_login_case_folded() {
    let first = FirstAsk { org: b"Temper-Bot".as_slice().into(), ..first_ask() };
    refused(
        &mut admin_domain(),
        admin_ask(1, Some(first)),
        Some(PartyRole::Owner),
        Some(VALUE),
        ForgeRefusal::FirstIsBot,
    );
}
#[test]
fn admin_connect_admits_another_organization_and_no_first() {
    for first in [Some(first_ask()), None] {
        let mut d = admin_domain();
        let out = application(&mut d, 1, admin_ask(1, first.clone()), Some(PartyRole::Owner), Some(VALUE));
        assert!(out.contains(&Request::Connect {
            forge: 1,
            request: Token::new(1),
            how: How::ForgejoAdmin { first },
            carried: VALUE,
        }));
    }
}
#[test]
fn admitted_admin_connect_marks_connecting_and_keeps_the_first_repository_intent() {
    for first in [None, Some(first_ask())] {
        let mut d = admin_domain();
        let out = application(&mut d, 1, admin_ask(1, first.clone()), Some(PartyRole::Owner), Some(VALUE));
        assert!(d.forges().rows[0].admin);
        assert_eq!(d.connection(1), Connection::Connecting);
        assert!(out.contains(&Request::Connect {
            forge: 1,
            request: Token::new(1),
            how: How::ForgejoAdmin { first: first.clone() },
            carried: VALUE
        }));
        assert_eq!(
            d.forges().rows[0].first,
            first.map(|first| First {
                org: first.org,
                repository: first.repository,
                project: first.project,
                state: FirstState::Making,
                label: first.label,
            })
        );
        assert!(!out.contains(&Request::CarriedUnused { number: VALUE.number }));
    }
}
#[test]
fn admin_connect_ends_erase_once_and_restore_the_previous_connection_on_failure() {
    for success in [false, true] {
        for previous in [Connection::NotConnected, Connection::Stopped { reason: b"stopped earlier".as_slice().into() }]
        {
            let mut d = admin_domain();
            outputs(&mut d, Event::Connection { forge: 1, state: previous.clone() });
            application(&mut d, 1, admin_ask(1, Some(first_ask())), Some(PartyRole::Owner), Some(VALUE));
            outputs(
                &mut d,
                Event::ConnectAnswered { forge: 1, request: Token::new(1), answer: ConnectAnswer::Connecting },
            );
            let state = Connection::Connected { id: 9, login: b"bot".as_slice().into() };
            let why = if success {
                outputs(&mut d, Event::FirstMade { forge: 1, id: 42 });
                outputs(&mut d, Event::Connection { forge: 1, state: state.clone() });
                None
            } else {
                outputs(&mut d, Event::FirstFailed { forge: 1, step: CreateStep::File });
                Some(Box::from(b"making work/hello: File (Refused)".as_slice()))
            };
            let event = Event::ConnectEnded {
                stop: None,
                forge: 1,
                request: Token::new(1),
                connected: success,
                why: why.clone(),
            };
            let out = outputs(
                &mut d,
                Event::ConnectEnded {
                    stop: None,
                    forge: 1,
                    request: Token::new(1),
                    connected: success,
                    why: why.clone(),
                },
            );
            assert_eq!(out.iter().filter(|r| **r == Request::EraseCarried { number: VALUE.number }).count(), 1);
            assert!(outputs(&mut d, event).is_empty());
            assert_eq!(d.connection(1), if success { state } else { previous });
            assert_eq!(
                d.forges().rows[0].first.as_ref().unwrap().state,
                if success { FirstState::Made { id: 42 } } else { FirstState::Failed { step: CreateStep::File } }
            );
            assert_eq!(d.forges().rows[0].ended, why);
            assert_eq!(d.forges().rows[0].app, None);
        }
    }
}
#[test]
fn a_failed_admin_connect_restores_the_failed_setup_with_its_step() {
    let mut d = admin_domain();
    outputs(&mut d, Event::Connection { forge: 1, state: Connection::SettingUp { step: Some(SetUpStep::Run) } });
    let failed = Connection::SetUpFailed { why: b"the box closed the pipe".as_slice().into() };
    outputs(&mut d, Event::Connection { forge: 1, state: failed.clone() });
    assert_eq!(d.forges().rows[0].failed, Some(FailedStep::Run));
    application(&mut d, 1, admin_ask(1, Some(first_ask())), Some(PartyRole::Owner), Some(VALUE));
    assert_eq!(d.connection(1), Connection::Connecting);
    assert_eq!(d.forges().rows[0].failed, None);
    outputs(&mut d, Event::ConnectEnded { forge: 1, request: Token::new(1), connected: false, why: None, stop: None });
    assert_eq!(d.connection(1), failed);
    assert_eq!(d.forges().rows[0].failed, Some(FailedStep::Run));
}
#[test]
fn the_next_admitted_admin_connect_replaces_the_intent_and_clears_the_previous_end() {
    for first in [None, Some(FirstAsk { repository: b"next".as_slice().into(), ..first_ask() })] {
        let mut d = admin_domain();
        application(&mut d, 1, admin_ask(1, Some(first_ask())), Some(PartyRole::Owner), Some(VALUE));
        outputs(
            &mut d,
            Event::ConnectEnded {
                stop: Some(AdminStop::Refused { status: 403, stage: SetupStage::Bot }),
                forge: 1,
                request: Token::new(1),
                connected: false,
                why: Some(Box::from(STOP_REASON)),
            },
        );
        refused(&mut d, admin_ask(1, None), None, Some(VALUE), ForgeRefusal::Role);
        assert_eq!(d.forges().rows[0].first.as_ref().unwrap().repository.as_ref(), b"hello");
        assert_eq!(d.forges().rows[0].ended.as_deref(), Some(STOP_REASON));
        assert_eq!(d.forges().rows[0].ended_stop, Some(AdminStop::Refused { status: 403, stage: SetupStage::Bot }));
        application(&mut d, 2, admin_ask(1, first.clone()), Some(PartyRole::Owner), Some(VALUE));
        assert_eq!(d.forges().rows[0].first.as_ref().map(|f| &f.repository), first.as_ref().map(|f| &f.repository));
        assert_eq!(d.forges().rows[0].ended, None);
        assert_eq!(d.forges().rows[0].ended_stop, None);
    }
}
#[test]
fn a_service_refusal_restores_the_admin_connects_previous_connection_and_releases_the_carry() {
    let mut d = admin_domain();
    let previous = Connection::Stopped { reason: Box::from(STOP_REASON) };
    outputs(&mut d, Event::Connection { forge: 1, state: previous.clone() });
    application(&mut d, 1, admin_ask(1, None), Some(PartyRole::Owner), Some(VALUE));
    let out = outputs(
        &mut d,
        Event::ConnectAnswered {
            forge: 1,
            request: Token::new(1),
            answer: ConnectAnswer::Refused(ForgeRefusal::NotTaken),
        },
    );
    assert!(out.contains(&Request::CarriedUnused { number: VALUE.number }));
    assert_eq!(d.connection(1), previous);
    assert!(
        outputs(
            &mut d,
            Event::ConnectEnded { stop: None, forge: 1, request: Token::new(1), connected: false, why: None }
        )
        .is_empty()
    );
}

#[test]
fn configured_admin_form_reaches_the_row() {
    let mut d = admin_domain();
    for admin in [false, true] {
        d.configured.get_mut(&1).unwrap().admin_connect = admin;
        assert_eq!(d.forges().rows[0].admin, admin);
    }
}
#[test]
fn typed_admin_stops_survive_a_cut_diagnostic_and_clear_on_connection() {
    for stop in [
        AdminStop::NotAdmin,
        AdminStop::Unmarked,
        AdminStop::Other,
        AdminStop::Refused { status: 401, stage: SetupStage::Admin },
        AdminStop::Refused { status: 403, stage: SetupStage::Repository },
    ] {
        let mut d = admin_domain();
        application(&mut d, 1, admin_ask(1, None), Some(PartyRole::Owner), Some(VALUE));
        let event = || Event::ConnectEnded {
            forge: 1,
            request: Token::new(1),
            connected: false,
            why: Some(Box::new([b'x'; CONNECT_WHY_BYTES + 1])),
            stop: Some(stop),
        };
        outputs(&mut d, event());
        assert!(outputs(&mut d, event()).is_empty());
        assert_eq!(d.forges().rows[0].ended.as_ref().unwrap().len(), CONNECT_WHY_BYTES);
        assert_eq!(d.forges().rows[0].ended_stop, Some(stop));
        outputs(
            &mut d,
            Event::Connection { forge: 1, state: Connection::Connected { id: 9, login: b"bot".as_slice().into() } },
        );
        assert_eq!(d.forges().rows[0].ended, None);
        assert_eq!(d.forges().rows[0].ended_stop, None);
    }
}
