//! Version, bounds and total outcome round trips (client/README.md, 4.6).
mod fixtures;
use crate::v1;
use alloc::boxed::Box;
use skein_lib::{List, Reader};

#[test]
fn an_adopt_of_another_version_is_refused() {
    let mut bytes = crate::requesting(
        &v1::Requesting::new(
            &fixtures::limits(),
            v1::RequestingParts { request: v1::Request::Adopt(fixtures::adopt()) },
        )
        .expect("request"),
    );
    *bytes.get_mut(1).expect("the record starts with a u16 version") =
        u8::try_from(crate::CLIENT_VERSION + 1).expect("test version fits");
    let problem =
        v1::Requesting::decode(&fixtures::limits(), &mut Reader::new(&bytes)).expect_err("another version is refused");
    assert_eq!(problem, v1::Problem { path: v1::Path::RequestingVersion, reason: skein_codec::Reason::Version });
}

#[test]
fn an_owner_one_byte_past_its_bound_is_refused() {
    let parts = fixtures::adopt().into_parts();
    v1::Adopt::new(&fixtures::limits(), v1::AdoptParts { owner: Box::new([b'a'; 16]), ..parts.clone() })
        .expect("at the bound");
    let made = v1::Adopt::new(&fixtures::limits(), v1::AdoptParts { owner: Box::new([b'a'; 17]), ..parts });
    assert_eq!(
        made.expect_err("past the owner bound"),
        v1::Problem { path: v1::Path::AdoptOwner, reason: skein_codec::Reason::Bound }
    );
}

#[test]
fn checks_one_past_their_count_are_refused() {
    let mut checks = List::with_capacity(3);
    for number in 1..=3 {
        checks.push(number).expect("counted room");
    }
    let parts = v1::AdoptParts { checks, ..fixtures::adopt().into_parts() };
    let problem = v1::Adopt::new(&fixtures::limits(), parts).expect_err("past the checks count");
    assert_eq!(problem, v1::Problem { path: v1::Path::AdoptChecks, reason: skein_codec::Reason::Bound });
}

#[test]
fn each_outcome_round_trips_unchanged() {
    let limits = fixtures::limits();
    for (row, outcome) in fixtures::outcome_rows().into_iter().enumerate() {
        let answer = v1::Answer::new(&limits, v1::AnswerParts { outcome }).expect("an outcome");
        let bytes = crate::answer(&answer);
        let back = v1::Answer::decode(&limits, &mut Reader::new(&bytes)).expect("decodes");
        assert_eq!(back, answer, "outcome row {row}");
    }
}

#[test]
fn limits_take_each_bound_from_its_source_field() {
    assert_eq!(
        fixtures::limits(),
        v1::Limits {
            owned_forge_forgejo: 64,
            owned_forge_runner_some: 64,
            owned_forge_password: 512,
            adoption_owner: 16,
            adoption_name: 16,
            forge_row_web_some: crate::ADDRESS_BYTES,
            forge_row_repositories: 16,
            reached_name: 16,
            page_rows: 100,
            install_back_some: 512,
            git_hub_manifest_back_some: 512,
            link_address: crate::ADDRESS_BYTES,
            forges_forges: crate::FORGES,
            return_path: 512,
            set_up_failed_why: crate::SET_UP_WHY_BYTES,
            group_failed_why: crate::SET_UP_WHY_BYTES,
            connected_login: 16,
            account_login: 16,
            forge_row_installations: crate::INSTALLATIONS,
            connect_stopped_why: crate::CONNECT_WHY_BYTES,
            stopped_reason: 16,
            pull_address: crate::ADDRESS_BYTES,
            queued_ahead: crate::TASKS_AHEAD,
            change_panel_owner: 16,
            change_panel_name: 16,
            change_panel_branch: 16,
            change_panel_landing: 16,
            change_panel_gates: crate::GATES,
            demo_answer_owner: 16,
            demo_answer_name: 16,
            demo_answer_cues: crate::CUES,
            demo_answer_cues_item: crate::CUE_BYTES,
            first_ask_org: 16,
            first_ask_repository: 16,
            first_ask_label: 64,
            set_up_org: 16,
            set_up_repository: 16,
            new_project_name: 16,
            first_repository_org: 16,
            first_repository_repository: 16,
            first_repository_label: 64,
            adopt_owner: 16,
            adopt_name: 16,
            adopt_prefix: 16,
            adopt_landing: 16,
            adopt_checks: 2,
            rule_branch: 16,
            rule_contexts: 2,
            rule_contexts_item: 16,
            adopted_owner: 16,
            adopted_name: 16,
            core_refused_refusal: 64,
        }
    );
    let bounds = crate::Bounds { checks: 65, ..fixtures::BOUNDS };
    assert_eq!(crate::limits(&bounds), None, "past the checks ceiling");
}

#[test]
fn each_owner_round_trips_its_number() {
    for (owner, number) in [(crate::Owner::Forge, 1), (crate::Owner::Pages, 2), (crate::Owner::Runner, 3)] {
        assert_eq!(crate::number(owner), number);
        assert_eq!(crate::owner(number), Some(owner));
    }
}

#[test]
fn an_unknown_owner_number_has_no_owner() {
    assert_eq!(crate::owner(65_535), None);
}

#[test]
fn the_demo_read_and_answer_round_trip_with_nonempty_cues() {
    let limits = fixtures::limits();
    let reading = v1::Reading::new(&limits, v1::ReadingParts { read: v1::Read::Demo }).expect("demo read");
    assert_eq!(crate::decode_reading(&crate::reading(&reading), &limits), Some(reading));
    for (row, cue, accepted) in [("cue", "Change it", true), ("empty cue", "", false)] {
        let mut cues = List::with_capacity(crate::CUES);
        cues.push(cue.as_bytes().into()).expect("one cue");
        let answer = v1::DemoAnswer::new(
            &limits,
            v1::DemoAnswerParts {
                state: v1::DemoState::Running,
                project: 7,
                forge: crate::FORGE,
                provider: 3,
                owner: b"acme".as_slice().into(),
                name: b"shop".as_slice().into(),
                cues,
            },
        )
        .expect("schema answer");
        let back = crate::decode_demo_answer(&crate::demo_answer(&answer), &limits);
        assert_eq!(back.is_some(), accepted, "{row}");
        if accepted {
            assert_eq!(back, Some(answer), "{row}");
        }
    }
}

#[test]
fn demo_cues_obey_their_count_and_text_bounds() {
    for state in [v1::DemoState::Running, v1::DemoState::Ended] {
        for (row, count, bytes, accepted) in [
            ("no cues", 0, b"x".as_slice(), true),
            ("one byte", 1, b"x".as_slice(), true),
            ("at count", crate::CUES, b"x".as_slice(), true),
            ("past count", crate::CUES + 1, b"x".as_slice(), false),
            ("at bytes", 1, &[b'a'; 256], true),
            ("past bytes", 1, &[b'a'; 257], false),
            ("not UTF-8", 1, &[255], false),
        ] {
            let limits = fixtures::limits();
            let mut cues = List::with_capacity(count);
            for _ in 0..count {
                cues.push(bytes.into()).expect("row's cue capacity");
            }
            let answer = v1::DemoAnswer::new(
                &limits,
                v1::DemoAnswerParts {
                    state: state.clone(),
                    project: 7,
                    forge: crate::FORGE,
                    provider: 3,
                    owner: b"acme".as_slice().into(),
                    name: b"shop".as_slice().into(),
                    cues,
                },
            );
            assert_eq!(answer.is_ok(), accepted, "{row}");
            if let Ok(answer) = answer {
                assert_eq!(crate::decode_demo_answer(&crate::demo_answer(&answer), &limits), Some(answer), "{row}");
            }
        }
    }
}

#[test]
fn a_change_watch_round_trips_and_refuses_trailing_bytes() {
    let limits = fixtures::limits();
    let subject = v1::ChangeSubject::new(&limits, v1::ChangeSubjectParts { task: 17 }).expect("change task");
    let watch = v1::Watching::new(&limits, v1::WatchingParts { watch: v1::Watch::Change(subject) }).expect("watch");
    let bytes = crate::watching(&watch);
    assert_eq!(crate::decode_watching(&bytes, &limits), Some(watch));
    let trailing = fixtures::trailing(&bytes);
    assert_eq!(crate::decode_watching(&trailing, &limits), None);
}

#[test]
fn each_change_panel_round_trips_unchanged() {
    let limits = fixtures::limits();
    for (row, panel) in fixtures::panel_rows().as_slice().iter().enumerate() {
        assert_eq!(crate::decode_change_panel(&crate::change_panel(panel), &limits).as_ref(), Some(panel), "row {row}");
    }
}

#[test]
fn a_change_panel_of_another_version_is_refused() {
    let limits = fixtures::limits();
    let panel = v1::ChangePanel::new(&limits, fixtures::panel_parts()).expect("panel");
    let mut bytes = crate::change_panel(&panel);
    *bytes.get_mut(1).expect("u16 version") = u8::try_from(crate::CLIENT_VERSION + 1).expect("test version fits");
    assert_eq!(
        v1::ChangePanel::decode(&limits, &mut Reader::new(&bytes)).expect_err("another version"),
        v1::Problem { path: v1::Path::ChangePanelVersion, reason: skein_codec::Reason::Version }
    );
    assert_eq!(crate::decode_change_panel(&bytes, &limits), None);
}

#[test]
fn a_change_panel_refuses_trailing_bytes_and_a_zero_queue_place() {
    let limits = fixtures::limits();
    for (row, place, trailing) in [("first", 1, false), ("zero", 0, false), ("trailing", 1, true)] {
        let parts = v1::ChangePanelParts {
            step: v1::Step::Queued(fixtures::queued(place, 0, &limits)),
            ..fixtures::panel_parts()
        };
        let panel = v1::ChangePanel::new(&limits, parts).expect("schema panel");
        let bytes = crate::change_panel(&panel);
        let bytes = if trailing { fixtures::trailing(&bytes) } else { bytes };
        assert_eq!(
            crate::decode_change_panel(&bytes, &limits),
            if place == 0 || trailing { None } else { Some(panel) },
            "{row}"
        );
    }
}

#[test]
fn change_panel_bounds_accept_at_and_refuse_one_past() {
    let limits = fixtures::limits();
    for (field, path, bound) in [
        (fixtures::PanelBound::Address, v1::Path::PullAddress, crate::ADDRESS_BYTES),
        (fixtures::PanelBound::Ahead, v1::Path::QueuedAhead, crate::TASKS_AHEAD),
        (fixtures::PanelBound::Gates, v1::Path::ChangePanelGates, crate::GATES),
        (fixtures::PanelBound::Owner, v1::Path::ChangePanelOwner, fixtures::BOUNDS.field_bytes),
        (fixtures::PanelBound::Name, v1::Path::ChangePanelName, fixtures::BOUNDS.field_bytes),
        (fixtures::PanelBound::Branch, v1::Path::ChangePanelBranch, fixtures::BOUNDS.field_bytes),
        (fixtures::PanelBound::Landing, v1::Path::ChangePanelLanding, fixtures::BOUNDS.field_bytes),
    ] {
        for count in [bound, bound + 1] {
            let made = fixtures::panel_at_bound(field, count);
            if count == bound {
                let panel = made.expect("at bound");
                assert_eq!(crate::decode_change_panel(&crate::change_panel(&panel), &limits), Some(panel), "{path:?}");
            } else {
                assert_eq!(made.expect_err("past bound"), v1::Problem { path, reason: skein_codec::Reason::Bound });
            }
        }
    }
}

#[test]
fn requesting_round_trips_and_refuses_trailing_and_version_three_adopt_bytes() {
    let limits = fixtures::limits();
    for request in [
        v1::Request::StartDemo,
        v1::Request::Adopt(fixtures::adopt()),
        v1::Request::SetUp(
            v1::SetUp::new(
                &limits,
                v1::SetUpParts {
                    forge: u16::MAX,
                    org: b"work".as_slice().into(),
                    repository: b"hello".as_slice().into(),
                    project: v1::SetUpProject::Existing(
                        v1::ExistingProject::new(&v1::CEILINGS, v1::ExistingProjectParts { project: 1 })
                            .expect("project"),
                    ),
                },
            )
            .expect("forge number"),
        ),
    ] {
        let record = v1::Requesting::new(&limits, v1::RequestingParts { request }).expect("request");
        let bytes = crate::requesting(&record);
        assert_eq!(crate::decode_requesting(&bytes, &limits), Some(record));
        assert_eq!(crate::decode_requesting(&fixtures::trailing(&bytes), &limits), None);
    }
    // Version 3's Adopt was versioned directly, without a request discriminator.
    let adopt = fixtures::adopt();
    let mut writer = skein_lib::Writer::new(usize::try_from(adopt.measure()).expect("length") + 2);
    writer.put(&[0, 3]).expect("old version");
    adopt.encode(&mut writer).expect("old adoption body");
    assert_eq!(crate::decode_requesting(&writer.finish(), &limits), None);
}

#[test]
fn forges_watch_round_trips_and_refuses_trailing_bytes() {
    let limits = fixtures::limits();
    let watch = v1::Watching::new(&limits, v1::WatchingParts { watch: v1::Watch::Forges }).expect("forges watch");
    let bytes = crate::watching(&watch);
    assert_eq!(crate::decode_watching(&bytes, &limits), Some(watch));
    assert_eq!(crate::decode_watching(&fixtures::trailing(&bytes), &limits), None);
}

fn forges_rows(numbers: &[u16], connection: &v1::Connection) -> v1::Forges {
    let limits = fixtures::limits();
    let mut forges = List::with_capacity(u32::try_from(numbers.len()).expect("small list"));
    for number in numbers {
        forges
            .push(
                v1::ForgeRow::new(
                    &limits,
                    v1::ForgeRowParts {
                        number: *number,
                        kind: v1::Kind::Forgejo,
                        mode: v1::Mode::Owned,
                        connection: connection.clone(),
                        offer: v1::Offer::Offered,
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
                .expect("row"),
            )
            .expect("row capacity");
    }
    v1::Forges::new(&limits, v1::ForgesParts { forges }).expect("forges")
}

#[test]
fn each_forge_offer_round_trips() {
    let limits = fixtures::limits();
    let mut forges = List::with_capacity(3);
    for (number, offer) in [(1, v1::Offer::NotOffered), (2, v1::Offer::NotUnderTheBox), (3, v1::Offer::Offered)] {
        let row = v1::ForgeRow::new(
            &limits,
            v1::ForgeRowParts {
                number,
                kind: v1::Kind::Forgejo,
                mode: v1::Mode::Owned,
                connection: v1::Connection::NotSetUp,
                offer,
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
        .expect("forge row");
        forges.push(row).expect("three rows");
    }
    let record = v1::Forges::new(&limits, v1::ForgesParts { forges }).expect("forges");
    let bytes = crate::forges(&record);
    assert_eq!(crate::decode_forges(&bytes, &limits), Some(record));
    assert_eq!(crate::decode_forges(&fixtures::trailing(&bytes), &limits), None);
}

#[test]
fn each_forge_connection_round_trips_at_its_bounds() {
    let limits = fixtures::limits();
    let failed = v1::SetUpFailed::new(&limits, v1::SetUpFailedParts { why: fixtures::field_bytes(256), at: None })
        .expect("why at bound");
    let connected = v1::Connected::new(&limits, v1::ConnectedParts { id: u64::MAX, login: fixtures::field_bytes(16) })
        .expect("login at bound");
    let stopped =
        v1::Stopped::new(&limits, v1::StoppedParts { reason: fixtures::field_bytes(16) }).expect("reason at bound");
    for connection in [
        v1::Connection::NotConnected,
        v1::Connection::NotSetUp,
        v1::Connection::SetUpFailed(failed),
        v1::Connection::Connecting,
        v1::Connection::Connected(connected),
        v1::Connection::Stopped(stopped),
    ] {
        let record = forges_rows(&[u16::MAX], &connection);
        let bytes = crate::forges(&record);
        assert_eq!(crate::decode_forges(&bytes, &limits), Some(record));
        assert_eq!(crate::decode_forges(&fixtures::trailing(&bytes), &limits), None);
    }
    for step in [None, Some(v1::SetUpStep::Fetch), Some(v1::SetUpStep::Prepare), Some(v1::SetUpStep::Run)] {
        let connection =
            v1::Connection::SettingUp(v1::SettingUp::new(&limits, v1::SettingUpParts { step }).expect("setup step"));
        let record = forges_rows(&[1], &connection);
        assert_eq!(crate::decode_forges(&crate::forges(&record), &limits), Some(record));
    }
}

#[test]
fn forge_fields_refuse_one_past_their_bounds_on_construction_and_decode() {
    let limits = fixtures::limits();
    let mut writer = skein_lib::Writer::new(4 + 257);
    writer.put(&257_u32.to_be_bytes()).expect("why length");
    writer.put(&[b'w'; 257]).expect("why bytes");
    assert_eq!(
        v1::SetUpFailed::decode(&limits, &mut Reader::new(&writer.finish())).expect_err("why past ceiling"),
        v1::Problem { path: v1::Path::SetUpFailedWhy, reason: skein_codec::Reason::Bound }
    );
    assert_eq!(
        v1::SetUpFailed::new(&limits, v1::SetUpFailedParts { why: fixtures::field_bytes(257), at: None })
            .expect_err("past bound"),
        v1::Problem { path: v1::Path::SetUpFailedWhy, reason: skein_codec::Reason::Bound }
    );
    assert_eq!(
        v1::Connected::new(&limits, v1::ConnectedParts { id: 1, login: fixtures::field_bytes(17) })
            .expect_err("past bound"),
        v1::Problem { path: v1::Path::ConnectedLogin, reason: skein_codec::Reason::Bound }
    );
    assert_eq!(
        v1::Stopped::new(&limits, v1::StoppedParts { reason: fixtures::field_bytes(17) }).expect_err("past bound"),
        v1::Problem { path: v1::Path::StoppedReason, reason: skein_codec::Reason::Bound }
    );
    // A record legal at schema ceilings must still respect the receiving side's bounds.
    for connection in [
        v1::Connection::Connected(
            v1::Connected::new(&v1::CEILINGS, v1::ConnectedParts { id: 1, login: fixtures::field_bytes(17) })
                .expect("schema login"),
        ),
        v1::Connection::Stopped(
            v1::Stopped::new(&v1::CEILINGS, v1::StoppedParts { reason: fixtures::field_bytes(17) })
                .expect("schema reason"),
        ),
    ] {
        let row = v1::ForgeRow::new(
            &v1::CEILINGS,
            v1::ForgeRowParts {
                number: 1,
                kind: v1::Kind::Github,
                mode: v1::Mode::Connected,
                connection,
                offer: v1::Offer::NotOffered,
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
        .expect("schema row");
        let mut forges = List::with_capacity(1);
        forges.push(row).expect("one row");
        let record = v1::Forges::new(&v1::CEILINGS, v1::ForgesParts { forges }).expect("schema forges");
        assert_eq!(crate::decode_forges(&crate::forges(&record), &limits), None);
    }
}

#[test]
fn forges_refuse_seventeen_rows_unsorted_rows_and_duplicate_numbers() {
    let limits = fixtures::limits();
    let numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
    let at = forges_rows(&numbers, &v1::Connection::NotConnected);
    assert_eq!(crate::decode_forges(&crate::forges(&at), &limits), Some(at.clone()));
    let mut parts = at.into_parts();
    let mut rows = List::with_capacity(17);
    for row in parts.forges.into_boxed() {
        rows.push(row).expect("room");
    }
    rows.push(
        v1::ForgeRow::new(
            &limits,
            v1::ForgeRowParts {
                number: 17,
                kind: v1::Kind::Github,
                mode: v1::Mode::Fake,
                connection: v1::Connection::NotSetUp,
                offer: v1::Offer::NotOffered,
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
        .expect("row"),
    )
    .expect("extra room");
    parts.forges = rows;
    assert_eq!(
        v1::Forges::new(&limits, parts).expect_err("past bound"),
        v1::Problem { path: v1::Path::ForgesForges, reason: skein_codec::Reason::Bound }
    );
    // A peer can send seventeen rows even though the constructor refuses them.
    let at = forges_rows(&numbers, &v1::Connection::NotConnected);
    let row = at.forges().get(0).expect("row");
    let length = at.measure().checked_add(row.measure()).expect("seventeen rows fit");
    let mut writer = skein_lib::Writer::new(usize::try_from(length).expect("length"));
    writer.put(&4_u16.to_be_bytes()).expect("version");
    writer.put(&17_u32.to_be_bytes()).expect("row count");
    for row in at.forges() {
        row.encode(&mut writer).expect("row bytes");
    }
    row.encode(&mut writer).expect("extra row bytes");
    assert_eq!(crate::decode_forges(&writer.finish(), &limits), None);
    for numbers in [&[2, 1][..], &[1, 1][..]] {
        let record = forges_rows(numbers, &v1::Connection::NotSetUp);
        assert_eq!(crate::decode_forges(&crate::forges(&record), &limits), None);
    }
}

#[test]
fn forge_login_and_stop_reason_round_trip_at_schema_ceilings() {
    let limits = v1::CEILINGS;
    for count in [1024, 1025] {
        let connected =
            v1::Connected::new(&limits, v1::ConnectedParts { id: u64::MAX, login: fixtures::field_bytes(count) });
        let stopped = v1::Stopped::new(&limits, v1::StoppedParts { reason: fixtures::field_bytes(count) });
        if count == 1025 {
            connected.expect_err("login past schema ceiling");
            stopped.expect_err("reason past schema ceiling");
            continue;
        }
        let connected = connected.expect("login ceiling");
        let stopped = stopped.expect("reason ceiling");
        for kind in [v1::Kind::Forgejo, v1::Kind::Github] {
            for mode in [v1::Mode::Connected, v1::Mode::Managed, v1::Mode::Owned, v1::Mode::Fake] {
                for connection in
                    [v1::Connection::Connected(connected.clone()), v1::Connection::Stopped(stopped.clone())]
                {
                    let row = v1::ForgeRow::new(
                        &limits,
                        v1::ForgeRowParts {
                            number: u16::MAX,
                            kind: kind.clone(),
                            mode: mode.clone(),
                            connection,
                            offer: v1::Offer::NotOffered,
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
                    .expect("forge row");
                    let mut forges = List::with_capacity(1);
                    forges.push(row).expect("one forge");
                    let record = v1::Forges::new(&limits, v1::ForgesParts { forges }).expect("forges");
                    assert_eq!(crate::decode_forges(&crate::forges(&record), &limits), Some(record));
                }
            }
        }
    }
}

#[test]
fn demo_start_outcomes_round_trip_at_each_boundary() {
    let limits = v1::CEILINGS;
    let mut outcomes = List::with_capacity(8);
    for outcome in [
        v1::Outcome::Running,
        v1::Outcome::StartRefused(
            v1::StartRefused::new(&limits, v1::StartRefusedParts { why: v1::StartRefusal::NotOffered })
                .expect("refusal"),
        ),
    ] {
        outcomes.push(outcome).expect("outcome room");
    }
    for why in [v1::StartFailure::Account, v1::StartFailure::Adoption, v1::StartFailure::NotConnected] {
        outcomes
            .push(v1::Outcome::StartFailed(
                v1::StartFailed::new(&limits, v1::StartFailedParts { why }).expect("failure"),
            ))
            .expect("outcome room");
    }
    for length in [0, 1, 256] {
        let group = v1::GroupFailed::new(&limits, v1::GroupFailedParts { why: skein_lib::bytes::zeroed(length) })
            .expect("bounded why");
        outcomes
            .push(v1::Outcome::StartFailed(
                v1::StartFailed::new(&limits, v1::StartFailedParts { why: v1::StartFailure::Group(group) })
                    .expect("group failure"),
            ))
            .expect("outcome room");
    }
    for outcome in outcomes.into_boxed() {
        let record = v1::Answer::new(&limits, v1::AnswerParts { outcome }).expect("answer");
        let bytes = crate::answer(&record);
        let mut reader = Reader::new(&bytes);
        assert_eq!(v1::Answer::decode(&limits, &mut reader), Ok(record));
        assert!(reader.is_empty());
    }
    v1::GroupFailed::new(&limits, v1::GroupFailedParts { why: skein_lib::bytes::zeroed(257) })
        .expect_err("past ceiling");
    let group =
        v1::GroupFailed::new(&limits, v1::GroupFailedParts { why: skein_lib::bytes::zeroed(256) }).expect("ceiling");
    let record = v1::Answer::new(
        &limits,
        v1::AnswerParts {
            outcome: v1::Outcome::StartFailed(
                v1::StartFailed::new(&limits, v1::StartFailedParts { why: v1::StartFailure::Group(group) })
                    .expect("failure"),
            ),
        },
    )
    .expect("answer");
    let bytes = crate::answer(&record);
    // Version (2), Outcome (1), StartFailure (1), then WHY's u32 length.
    let mut writer = skein_lib::Writer::new(bytes.len().checked_add(1).expect("extra byte"));
    writer.put(&bytes[..4]).expect("header");
    writer.put(&257_u32.to_be_bytes()).expect("length");
    writer.put(&bytes[8..]).expect("why");
    writer.put(b"x").expect("extra byte");
    let bytes = writer.finish();
    v1::Answer::decode(&limits, &mut Reader::new(&bytes)).expect_err("past ceiling on decode");
}

#[test]
fn demo_states_round_trip_at_smallest_and_ceiling() {
    use skein_lib::List;
    let limits = v1::CEILINGS;
    for state in [v1::DemoState::NotStarted, v1::DemoState::Running, v1::DemoState::Ended] {
        for full in [false, true] {
            let mut cues = List::with_capacity(limits.demo_answer_cues);
            if full {
                for _ in 0..limits.demo_answer_cues {
                    cues.push(skein_lib::bytes::zeroed(
                        usize::try_from(limits.demo_answer_cues_item).expect("ceiling"),
                    ))
                    .expect("cue room");
                }
            }
            let field = if full { usize::try_from(limits.demo_answer_owner).expect("ceiling") } else { 0 };
            let record = v1::DemoAnswer::new(
                &limits,
                v1::DemoAnswerParts {
                    project: if full { u32::MAX } else { 0 },
                    forge: if full { u16::MAX } else { 0 },
                    provider: if full { u32::MAX } else { 0 },
                    owner: skein_lib::bytes::zeroed(field),
                    name: skein_lib::bytes::zeroed(field),
                    state: state.clone(),
                    cues,
                },
            )
            .expect("demo answer");
            let bytes = crate::demo_answer(&record);
            assert_eq!(crate::decode_demo_answer(&bytes, &limits), Some(record));
        }
    }
    let record =
        v1::Requesting::new(&limits, v1::RequestingParts { request: v1::Request::StartDemo }).expect("start request");
    assert_eq!(crate::decode_requesting(&crate::requesting(&record), &limits), Some(record));
}

#[test]
fn pasted_apps_round_trip_and_check_nonzero_id() {
    let limits = fixtures::limits();
    for id in [0, 1, u64::MAX] {
        let app = v1::GitHubApp::new(&limits, v1::GitHubAppParts { id }).unwrap();
        let connect = v1::Connect::new(&limits, v1::ConnectParts { forge: 1, how: v1::How::GithubApp(app) }).unwrap();
        let record =
            v1::Requesting::new(&limits, v1::RequestingParts { request: v1::Request::Connect(connect) }).unwrap();
        let decoded = crate::decode_requesting(&crate::requesting(&record), &limits);
        assert_eq!(decoded, if id == 0 { None } else { Some(record) });
    }
}

#[test]
fn connect_outcomes_round_trip() {
    let limits = fixtures::limits();
    let mut outcomes = List::with_capacity(10);
    for outcome in [
        v1::Outcome::Connecting,
        v1::Outcome::Link(
            v1::Link::new(&limits, v1::LinkParts { address: Box::from(b"https://forge.test/install".as_slice()) })
                .unwrap(),
        ),
    ] {
        outcomes.push(outcome).unwrap();
    }
    for why in [
        v1::ForgeRefusal::Role,
        v1::ForgeRefusal::NoSuchForge,
        v1::ForgeRefusal::NotTaken,
        v1::ForgeRefusal::Connected,
        v1::ForgeRefusal::NotConnected,
        v1::ForgeRefusal::Busy,
        v1::ForgeRefusal::NoKey,
        v1::ForgeRefusal::FirstIsBot,
    ] {
        outcomes
            .push(v1::Outcome::ConnectRefused(
                v1::ConnectRefused::new(&limits, v1::ConnectRefusedParts { why }).unwrap(),
            ))
            .unwrap();
    }
    for outcome in outcomes.into_boxed() {
        let record = v1::Answer::new(&limits, v1::AnswerParts { outcome }).unwrap();
        assert_eq!(v1::Answer::decode(&limits, &mut Reader::new(&crate::answer(&record))).unwrap(), record);
    }
}

// client/README.md, 4.6: at most 64 installations and a 256-byte end.
const INSTALLATIONS: u32 = 64;
const CONNECT_WHY: usize = 256;
fn installation_row(count: u32, why_bytes: usize) -> v1::ForgeRowParts {
    let mut installations = List::with_capacity(count);
    for id in 0..count {
        installations
            .push(
                v1::Installation::new(
                    &v1::CEILINGS,
                    v1::InstallationParts {
                        id: u64::from(id),
                        account: Some(
                            v1::Account::new(
                                &v1::CEILINGS,
                                v1::AccountParts { id: 42, login: Box::from(b"acme".as_slice()) },
                            )
                            .unwrap(),
                        ),
                        suspended: true,
                        stray: false,
                    },
                )
                .unwrap(),
            )
            .unwrap();
    }
    v1::ForgeRowParts {
        number: 1,
        kind: v1::Kind::Github,
        mode: v1::Mode::Connected,
        connection: v1::Connection::NotConnected,
        offer: v1::Offer::NotOffered,
        first: None,
        installations,
        ended: Some(
            v1::ConnectStopped::new(&v1::CEILINGS, v1::ConnectStoppedParts { why: filled(why_bytes) }).unwrap(),
        ),
        app: None,
        web: None,
        repositories: List::with_capacity(0),
        owned: None,
        admin: false,
        read_only: false,
        ended_stop: None,
    }
}
// Encode the fields explicitly to feed invalid lengths to the receiving boundary.
fn installation_row_bytes(parts: v1::ForgeRowParts, why: &[u8]) -> Box<[u8]> {
    // Number (2), four enum tags, first tag, list length (4), end tag, why length (4),
    // app tag, web tag, repositories length (4), owned tag, admin bool, typed stop tag, read-only bool.
    let mut size = why.len().checked_add(26).unwrap();
    for installation in &parts.installations {
        size = size.checked_add(usize::try_from(installation.measure()).unwrap()).unwrap();
    }
    let mut writer = skein_lib::Writer::new(size);
    writer.put(&parts.number.to_be_bytes()).unwrap();
    parts.kind.encode(&mut writer).unwrap();
    parts.mode.encode(&mut writer).unwrap();
    parts.connection.encode(&mut writer).unwrap();
    parts.offer.encode(&mut writer).unwrap();
    writer.put(&[0]).unwrap(); // no first repository
    writer.put(&parts.installations.len().to_be_bytes()).unwrap();
    for installation in &parts.installations {
        installation.encode(&mut writer).unwrap();
    }
    writer.put(&[1]).unwrap(); // an end
    writer.put(&u32::try_from(why.len()).unwrap().to_be_bytes()).unwrap();
    writer.put(why).unwrap();
    writer.put(&[0]).unwrap(); // no App way
    writer.put(&[0]).unwrap(); // no web address
    writer.put(&0_u32.to_be_bytes()).unwrap(); // no adoptions
    writer.put(&[0]).unwrap(); // no owned details
    writer.put(&[0, 0, 0]).unwrap(); // admin false, no typed stop, read-only false
    writer.finish()
}
#[test]
fn installation_rows_round_trip_and_refuse_lengths_past_the_ceilings() {
    let limits = v1::CEILINGS;
    for count in [0, INSTALLATIONS] {
        let row = v1::ForgeRow::new(&limits, installation_row(count, CONNECT_WHY)).unwrap();
        let mut writer = skein_lib::Writer::new(usize::try_from(row.measure()).unwrap());
        row.encode(&mut writer).unwrap();
        assert_eq!(v1::ForgeRow::decode(&limits, &mut Reader::new(&writer.finish())), Ok(row));
    }
    for (count, why_bytes, accepted) in [
        (INSTALLATIONS, CONNECT_WHY, true),
        (INSTALLATIONS + 1, CONNECT_WHY, false),
        (INSTALLATIONS, CONNECT_WHY + 1, false),
    ] {
        let parts = installation_row(count, 0);
        let bytes = installation_row_bytes(parts, &filled(why_bytes));
        assert_eq!(v1::ForgeRow::decode(&limits, &mut Reader::new(&bytes)).is_ok(), accepted);
    }
    v1::ForgeRow::new(&limits, installation_row(INSTALLATIONS + 1, 0)).unwrap_err();
    v1::ConnectStopped::new(&limits, v1::ConnectStoppedParts { why: filled(CONNECT_WHY + 1) }).unwrap_err();
}
fn filled(bytes: usize) -> Box<[u8]> {
    let mut writer = skein_lib::Writer::new(bytes);
    for _ in 0..bytes {
        writer.put(b"x").unwrap();
    }
    writer.finish()
}

#[test]
fn manifest_connects_and_app_ways_round_trip() {
    let limits = fixtures::limits();
    for back in [None, Some(Box::from(b"/forges/connect?forge=1".as_slice()))] {
        let how = v1::GitHubManifest::new(&limits, v1::GitHubManifestParts { back }).unwrap();
        let connect =
            v1::Connect::new(&limits, v1::ConnectParts { forge: 1, how: v1::How::GithubManifest(how) }).unwrap();
        let record =
            v1::Requesting::new(&limits, v1::RequestingParts { request: v1::Request::Connect(connect) }).unwrap();
        assert_eq!(crate::decode_requesting(&crate::requesting(&record), &limits), Some(record));
    }
    for app in [None, Some(v1::AppWay::Manifest), Some(v1::AppWay::Pasted)] {
        let mut parts = installation_row(0, 0);
        parts.app = app;
        let row = v1::ForgeRow::new(&limits, parts).unwrap();
        let mut writer = skein_lib::Writer::new(usize::try_from(row.measure()).unwrap());
        row.encode(&mut writer).unwrap();
        assert_eq!(v1::ForgeRow::decode(&limits, &mut Reader::new(&writer.finish())), Ok(row));
    }
}

mod forgejo_admin;

#[test]
fn setup_failure_round_trips_each_step_and_none() {
    let limits = fixtures::limits();
    for at in [
        None,
        Some(v1::FailedStep::Fetch),
        Some(v1::FailedStep::Prepare),
        Some(v1::FailedStep::Run),
        Some(v1::FailedStep::Connect),
    ] {
        let value =
            v1::SetUpFailed::new(&limits, v1::SetUpFailedParts { why: b"stopped".as_slice().into(), at }).unwrap();
        let mut writer = skein_lib::Writer::new(usize::try_from(value.measure()).unwrap());
        value.encode(&mut writer).unwrap();
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        assert_eq!(v1::SetUpFailed::decode(&limits, &mut reader).unwrap(), value);
        assert!(reader.is_empty());
    }
}

#[test]
fn admin_rows_round_trip_the_form_stop_and_first_label() {
    let limits = v1::CEILINGS;
    let mut stops = List::with_capacity(13);
    for stop in [None, Some(v1::AdminStop::NotAdmin), Some(v1::AdminStop::Unmarked), Some(v1::AdminStop::Other)] {
        stops.push(stop).unwrap();
    }
    for stage in [
        v1::SetupStage::Admin,
        v1::SetupStage::Bot,
        v1::SetupStage::Token,
        v1::SetupStage::Org,
        v1::SetupStage::Repository,
        v1::SetupStage::File,
        v1::SetupStage::App,
        v1::SetupStage::Installation,
        v1::SetupStage::State,
    ] {
        stops
            .push(Some(v1::AdminStop::Refused(
                v1::AdminRefused::new(&limits, v1::AdminRefusedParts { status: 403, stage }).unwrap(),
            )))
            .unwrap();
    }
    for stop in stops.into_boxed() {
        for admin in [false, true] {
            let mut parts = installation_row(0, 0);
            parts.admin = admin;
            parts.ended_stop = stop.clone();
            parts.first = Some(first_with_label(filled(64)).unwrap());
            let row = v1::ForgeRow::new(&limits, parts).unwrap();
            let mut writer = skein_lib::Writer::new(usize::try_from(row.measure()).unwrap());
            row.encode(&mut writer).unwrap();
            assert_eq!(v1::ForgeRow::decode(&limits, &mut Reader::new(&writer.finish())), Ok(row));
        }
    }
}
fn first_with_label(label: Box<[u8]>) -> Result<v1::FirstRepository, v1::Problem> {
    v1::FirstRepository::new(
        &v1::CEILINGS,
        v1::FirstRepositoryParts {
            org: b"work".as_slice().into(),
            repository: b"hello".as_slice().into(),
            project: v1::SetUpProject::Existing(
                v1::ExistingProject::new(&v1::CEILINGS, v1::ExistingProjectParts { project: 1 }).unwrap(),
            ),
            state: v1::FirstState::Making,
            label,
        },
    )
}
#[test]
fn first_label_accepts_64_bytes_and_refuses_one_more() {
    first_with_label(filled(64)).unwrap();
    assert_eq!(
        first_with_label(filled(65)),
        Err(v1::Problem { path: v1::Path::FirstRepositoryLabel, reason: skein_codec::Reason::Bound })
    );
}
