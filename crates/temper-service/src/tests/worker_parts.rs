use crate::worker_route;
use jig_domain_host as host;
use skein_lib::Writer;
use smith_host_domain as agent;
use temper_worker_domain::Invalid;

#[test]
fn delivery_evidence_disagreeing_with_its_outcome_is_invalid() {
    let record = smith_channel::DeliveryReply::new(
        &smith_channel::CEILINGS,
        smith_channel::DeliveryReplyParts { value: smith_channel::Delivery::Nothing },
    )
    .unwrap();
    let mut writer = Writer::new(usize::try_from(record.measure()).unwrap());
    record.encode(&mut writer).unwrap();
    let bytes = writer.finish();
    for (row, outcome, evidence, expected) in [
        ("agreeing", host::DeliveryOutcome::Nothing, bytes.as_ref(), Ok(agent::Delivery::Nothing)),
        ("disagreeing", host::DeliveryOutcome::Delivered, bytes.as_ref(), Err(Invalid::Evidence)),
        ("malformed", host::DeliveryOutcome::Nothing, &[255, 0, 1], Err(Invalid::Evidence)),
    ] {
        assert_eq!(worker_route::evidence(outcome, evidence), expected, "{row}");
    }
    let mut writer = Writer::new(bytes.len() + 1);
    writer.put(&bytes).unwrap();
    writer.put(&[0]).unwrap();
    let trailing = writer.finish();
    assert_eq!(worker_route::evidence(host::DeliveryOutcome::Nothing, &trailing), Err(Invalid::Evidence), "trailing");
}

#[test]
fn a_call_name_is_encoded_as_smiths_channel_record() {
    let config = fixtures::agent();
    let name = agent::CallName { activation: 7, completion: 2, position: 3 };
    let encoded = worker_route::encode(temper_worker_domain::Encodable::CallName(name), &config);
    let temper_worker_domain::Encoded::CallName(bytes) = encoded else { panic!("call name encoded") };
    let record = smith_channel::CallName::decode(&config.limits.bodies, &mut Reader::new(&bytes)).unwrap();
    assert_eq!((record.activation(), record.completion(), record.position()), (7, 2, 3));
}

#[test]
fn a_process_exit_keeps_its_owner_and_status() {
    let owner = Token::new(9);
    assert_eq!(
        worker_route::process(smith_host_protocol::ProcessEvent::Exited {
            agent: owner,
            exit: Some(skein_io::kernel::Exit::Code(3)),
        }),
        Some(temper_worker_domain::Event::Process(Box::new(agent::process::Event::Exited {
            owner,
            exit: Some(agent::Exit::Code(3)),
        }))),
    );
}

use super::fixtures;
use crate::identities::{IdentityTable, Unfit};
use crate::translate;
use jig_protocol_worker_wire::Part;
use skein_lib::{Reader, Secret, Token};
use temper_protocol_worker_wire::{self as payload, v1};
use temper_worker_domain::wire;

#[test]
fn an_engine_s_items_reach_the_worker_s_workspace_field_for_field() {
    let assignment = fixtures::assignment_with(fixtures::repository(3, b"acme", b"app"));
    let config = fixtures::engine();
    let limits = fixtures::wire_limits();
    let items = translate::items(&assignment, &config.forges, &crate::forge::tokens::Tokens::empty(), &limits)
        .unwrap()
        .unwrap();
    for program in [None, Some(Box::from(b"/fake/git".as_slice()))] {
        let mut fields = items.clone().into_parts();
        let mut repositories = skein_lib::List::with_capacity(1);
        for repository in fields.repositories.into_boxed() {
            let mut fields = repository.into_parts();
            fields.git = program.clone();
            repositories.push(v1::Repository::new(&limits, fields).unwrap()).unwrap();
        }
        fields.repositories = repositories;
        let record = v1::Items::new(&limits, fields).unwrap();
        let parts = [Part { token: Token::new(9), bytes: payload::items(&record) }];
        let (workspace, save) = worker_route::items(&fixtures::worker_assignment(), &parts, &limits).unwrap();
        let mut expected = fixtures::one_repository();
        expected.repositories[0].git = program;
        assert_eq!(workspace, expected);
        assert_eq!(save, None);
    }
    assert_eq!(translate::workspace(None), (wire::Workspace { repositories: Box::new([]) }, None));
}

#[test]
fn items_whose_bytes_do_not_decode_are_undecodable() {
    let assignment = fixtures::worker_assignment();
    let limits = fixtures::wire_limits();
    for (row, parts) in [
        ("missing", Box::<[Part]>::default()),
        ("malformed", Box::from([Part { token: Token::new(9), bytes: Box::from([255, 0]) }])),
        ("wrong token", Box::from([Part { token: Token::new(8), bytes: Box::from([255, 0]) }])),
    ] {
        assert_eq!(worker_route::items(&assignment, &parts, &limits), Err(wire::Undecodable), "{row}");
    }
}

#[test]
fn left_and_saved_go_up_under_the_tokens_the_answer_names() {
    let work = wire::Work {
        landed: Box::new([wire::Landed { tag: 3, commit: [7; 32] }]),
        saved: Some(Box::new([wire::SavedRepository { tag: 3, from_saved: true, landing: wire::Landing::Moved }])),
    };
    let answer = fixtures::answer(host::Ending::Parked {
        work: host::Work { left: Some(Token::new(11)), saved: Some(Token::new(12)) },
    });
    let host::LinkUp::Answer { answer, .. } = answer else { panic!("answer fixture") };
    let parts = worker_route::answer_parts(&answer, &work, None);
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].token, Token::new(11));
    let left = v1::Left::decode(&fixtures::wire_limits(), &mut Reader::new(&parts[0].bytes)).unwrap();
    let head = left.landed().get(0).unwrap();
    assert_eq!((head.tag(), head.commit()), (3, &[7; 32]));
    assert_eq!(parts[1].token, Token::new(12));
    let saved = v1::Saved::decode(&fixtures::wire_limits(), &mut Reader::new(&parts[1].bytes)).unwrap();
    let row = saved.repositories().get(0).unwrap();
    assert_eq!((row.tag(), row.from_saved(), row.landing()), (3, true, &v1::Landing::Moved));
    let (_, saved) = translate::work(&wire::Work { landed: Box::new([]), saved: None });
    assert_eq!(saved, None);
}

#[test]
fn a_permanent_preparation_goes_up_by_tag_and_a_transient_one_not() {
    let limits = fixtures::wire_limits();
    let work = wire::Work { landed: Box::new([]), saved: None };
    for (row, why, expected) in preparation_rows() {
        let answer = fixtures::answer(host::Ending::Failed {
            failure: host::Failure::Unprepared(host::Preparation::Permanent { resource: Some(Token::new(13)) }),
            detail: Box::new([]),
            work: host::Work { left: None, saved: None },
        });
        let host::LinkUp::Answer { answer, .. } = answer else { panic!("answer fixture") };
        let parts = worker_route::answer_parts(&answer, &work, Some(&why));
        let decoded = match parts.first() {
            Some(part) => {
                assert_eq!(part.token, Token::new(13), "{row}");
                Some(v1::Preparation::decode(&limits, &mut Reader::new(&part.bytes)).unwrap())
            }
            None => None,
        };
        assert_eq!(decoded, expected, "{row}");
    }
}

#[test]
fn an_identity_breaking_a_field_rule_is_dropped() {
    let limits = fixtures::wire_limits();
    for (row, login, name, email, expected) in [
        ("valid", b"temper-bot".as_slice(), b"Temper Bot".as_slice(), b"bot@example.test".as_slice(), true),
        ("login colon", b"bot:bad", b"Bot", b"bot@test", false),
        ("login control", b"bot\t", b"Bot", b"bot@test", false),
        ("name empty", b"bot", b"", b"bot@test", false),
        ("name newline", b"bot", b"Bot\n", b"bot@test", false),
        ("name NUL", b"bot", b"Bot\0", b"bot@test", false),
        ("email empty", b"bot", b"Bot", b"", false),
        ("email bracket", b"bot", b"Bot", b"<bot@test>", false),
    ] {
        let record = v1::Identity::new(
            &limits,
            v1::IdentityParts {
                kind: v1::Kind::Forgejo(
                    v1::Token::new(&limits, v1::TokenParts { token: Secret::new(b"credential".as_slice().into()) })
                        .unwrap(),
                ),
                login: login.into(),
                name: name.into(),
                email: email.into(),
            },
        )
        .unwrap();
        let value = worker_route::identity(&payload::identity(&record), &limits);
        assert_eq!(value.is_some(), expected, "{row}");
        if expected {
            assert_eq!(value, Some(crate::identities::Value::Forgejo(fixtures::identity(b"credential"))), "{row}");
        }
    }
    assert_eq!(worker_route::identity(&Secret::new(Box::from([255, 0])), &limits), None, "malformed");
}

#[test]
fn a_late_older_generation_leaves_the_newest_in_place() {
    let mut table = IdentityTable::new(2);
    for (generation, token, newest) in [
        (2, b"tok-2".as_slice(), b"tok-2".as_slice()),
        (1, b"tok-1", b"tok-2"),
        (3, b"tok-3", b"tok-3"),
        (1, b"late-1", b"tok-3"),
        (3, b"again-3", b"again-3"),
    ] {
        table
            .granted(
                1,
                generation,
                Some(crate::identities::Value::Forgejo(fixtures::identity(token))),
                skein_lib::Time::from_nanos(u64::MAX),
            )
            .unwrap();
        assert_eq!(
            table.newest(1, fixtures::NOW),
            Some(&crate::identities::Value::Forgejo(fixtures::identity(newest))),
            "generation {generation}"
        );
    }
    assert_eq!(table.newest(2, fixtures::NOW), None);
    for number in [0, 3] {
        assert_eq!(
            table.granted(
                number,
                1,
                Some(crate::identities::Value::Forgejo(fixtures::identity(b"bad"))),
                skein_lib::Time::from_nanos(u64::MAX)
            ),
            Err(Unfit),
            "number {number}"
        );
    }
}

fn preparation_rows() -> [(&'static str, wire::Preparation, Option<v1::Preparation>); 5] {
    let limits = fixtures::wire_limits();
    [
        (
            "missing repository",
            wire::Preparation::Missing { tag: 3, missing: wire::Missing::Repository },
            Some(v1::Preparation::Missing(
                v1::Missed::new(&limits, v1::MissedParts { tag: 3, missing: v1::Missing::Repository }).unwrap(),
            )),
        ),
        (
            "missing branch",
            wire::Preparation::Missing { tag: 7, missing: wire::Missing::Branch },
            Some(v1::Preparation::Missing(
                v1::Missed::new(&limits, v1::MissedParts { tag: 7, missing: v1::Missing::Branch }).unwrap(),
            )),
        ),
        (
            "missing commit",
            wire::Preparation::Missing { tag: 9, missing: wire::Missing::Commit },
            Some(v1::Preparation::Missing(
                v1::Missed::new(&limits, v1::MissedParts { tag: 9, missing: v1::Missing::Commit }).unwrap(),
            )),
        ),
        (
            "refused",
            wire::Preparation::Refused { tag: 4 },
            Some(v1::Preparation::Refused(v1::Tag::new(&limits, v1::TagParts { tag: 4 }).unwrap())),
        ),
        ("transient", wire::Preparation::Transient, None),
    ]
}

#[test]
#[expect(clippy::disallowed_macros, reason = "formatting Debug is necessary to prove it omits command secrets")]
fn program_resource_debug_exposes_only_counts() {
    let resources = crate::program_sources::Resources::new(
        smith_protocol_program::Programs {
            claude: b"/private/program".as_slice().into(),
            directory: b"/private/logins".as_slice().into(),
        },
        Box::new([Box::from(b"API_KEY=private-value".as_slice())]),
    )
    .expect("explicit command resources fit");
    assert_eq!(format!("{resources:?}"), "Resources { programs: 1, environment_entries: 1 }");
}

#[test]
fn an_empty_github_identity_is_decoded_and_kept() {
    let limits = fixtures::wire_limits();
    let record = v1::Identity::new(
        &limits,
        v1::IdentityParts {
            kind: v1::Kind::Github(
                v1::Entries::new(&limits, v1::EntriesParts { entries: skein_lib::List::with_capacity(0) }).unwrap(),
            ),
            login: b"x-access-token".as_slice().into(),
            name: b"Temper".as_slice().into(),
            email: b"bot@example.test".as_slice().into(),
        },
    )
    .unwrap();
    let secret = payload::identity(&record);
    assert!(payload::valid(&record));
    let expected = worker_route::identity(&secret, &limits).expect("valid GitHub value");
    let crate::identities::Value::GitHub { entries, .. } = &expected else { panic!("GitHub kind") };
    assert!(entries.is_empty());
    let mut worker = fixtures::worker();
    worker.link(
        fixtures::NOW,
        jig_protocol_worker_link::Request::Value {
            account: 1,
            generation: 1,
            expires: skein_lib::Time::from_nanos(u64::MAX),
            value: jig_protocol_worker_link::GrantValue::Application { secret },
        },
    );
    assert_eq!(worker.undecodable_values, 0);
    assert_eq!(worker.identities.newest(1, fixtures::NOW), Some(&expected));
}
