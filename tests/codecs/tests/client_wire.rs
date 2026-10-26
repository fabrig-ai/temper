//! jig's core refusal remains opaque in the forge family (client/README.md, 4.6).
use jig_client_protocol_wire as jig;
use skein_lib::{List, Reader, Writer};
use temper_client_protocol_wire::v1;

#[test]
fn a_core_refusal_carries_jigs_refusal_bytes_unchanged() {
    let refusal = jig::Refusal::NotWaiting;
    let mut writer = Writer::new(usize::try_from(refusal.measure()).expect("measured length"));
    refusal.encode(&mut writer).expect("measured room");
    let carried = v1::CoreRefused::new(&v1::CEILINGS, v1::CoreRefusedParts { refusal: writer.finish() })
        .expect("within the ceiling");
    let parts = v1::AnswerParts { outcome: v1::Outcome::CoreRefused(carried) };
    let answer = v1::Answer::new(&v1::CEILINGS, parts).expect("an answer");
    let bytes = temper_client_protocol_wire::answer(&answer);
    let decoded = v1::Answer::decode(&v1::CEILINGS, &mut Reader::new(&bytes)).expect("decodes");
    let v1::Outcome::CoreRefused(kept) = decoded.into_parts().outcome else { panic!("a core refusal") };
    let back = jig::Refusal::decode(&jig::CEILINGS, &mut Reader::new(kept.refusal())).expect("jig's refusal");
    assert_eq!(back, refusal);
}

#[test]
fn version_seven_request_answer_and_forges_goldens_round_trip() {
    use temper_client_protocol_wire as wire;
    for bytes in [
        include_bytes!("../../../crates/temper-client-protocol-wire/golden/record_requesting_smallest.bin").as_slice(),
        include_bytes!("../../../crates/temper-client-protocol-wire/golden/record_requesting_full.bin").as_slice(),
    ] {
        let record = wire::decode_requesting(bytes, &v1::CEILINGS).expect("request golden");
        assert_eq!(wire::requesting(&record).as_ref(), bytes);
    }
    for bytes in [
        include_bytes!("../../../crates/temper-client-protocol-wire/golden/record_answer_smallest.bin").as_slice(),
        include_bytes!("../../../crates/temper-client-protocol-wire/golden/record_answer_full.bin").as_slice(),
    ] {
        let record = v1::Answer::decode(&v1::CEILINGS, &mut Reader::new(bytes)).expect("answer golden");
        assert_eq!(wire::answer(&record).as_ref(), bytes);
    }
    for bytes in [
        include_bytes!("../../../crates/temper-client-protocol-wire/golden/record_forges_smallest.bin").as_slice(),
        include_bytes!("../../../crates/temper-client-protocol-wire/golden/record_forges_full.bin").as_slice(),
    ] {
        let record = wire::decode_forges(bytes, &v1::CEILINGS).expect("forges golden");
        assert_eq!(wire::forges(&record).as_ref(), bytes);
    }
}

fn project() -> v1::SetUpProject {
    v1::SetUpProject::Existing(
        v1::ExistingProject::new(&v1::CEILINGS, v1::ExistingProjectParts { project: 7 })
            .expect("record within schema ceilings"),
    )
}
fn first(state: v1::FirstState) -> v1::FirstRepository {
    v1::FirstRepository::new(
        &v1::CEILINGS,
        v1::FirstRepositoryParts {
            org: b"work".as_slice().into(),
            repository: b"hello".as_slice().into(),
            project: project(),
            state,
            label: Box::from(b"temper-host".as_slice()),
        },
    )
    .expect("record within schema ceilings")
}
#[test]
fn setup_and_every_first_repository_state_round_trip_at_version_eleven() {
    use temper_client_protocol_wire as wire;
    assert_eq!(wire::CLIENT_VERSION, 11);
    for project in [
        project(),
        v1::SetUpProject::New(
            v1::NewProject::new(&v1::CEILINGS, v1::NewProjectParts { name: b"Next".as_slice().into() })
                .expect("record within schema ceilings"),
        ),
    ] {
        let request = v1::Requesting::new(
            &v1::CEILINGS,
            v1::RequestingParts {
                request: v1::Request::SetUp(
                    v1::SetUp::new(
                        &v1::CEILINGS,
                        v1::SetUpParts {
                            forge: 3,
                            org: b"work".as_slice().into(),
                            repository: b"hello".as_slice().into(),
                            project,
                        },
                    )
                    .expect("record within schema ceilings"),
                ),
            },
        )
        .expect("record within schema ceilings");
        assert_eq!(wire::decode_requesting(&wire::requesting(&request), &v1::CEILINGS), Some(request));
    }
    for state in [
        v1::FirstState::Making,
        v1::FirstState::Made(
            v1::Made::new(&v1::CEILINGS, v1::MadeParts { id: u64::MAX }).expect("record within schema ceilings"),
        ),
        v1::FirstState::Failed(
            v1::CreateFailed::new(&v1::CEILINGS, v1::CreateFailedParts { step: v1::CreateStep::Admin })
                .expect("record within schema ceilings"),
        ),
        v1::FirstState::Failed(
            v1::CreateFailed::new(&v1::CEILINGS, v1::CreateFailedParts { step: v1::CreateStep::Org })
                .expect("record within schema ceilings"),
        ),
        v1::FirstState::Failed(
            v1::CreateFailed::new(&v1::CEILINGS, v1::CreateFailedParts { step: v1::CreateStep::Repository })
                .expect("record within schema ceilings"),
        ),
        v1::FirstState::Failed(
            v1::CreateFailed::new(&v1::CEILINGS, v1::CreateFailedParts { step: v1::CreateStep::File })
                .expect("record within schema ceilings"),
        ),
    ] {
        let row = v1::ForgeRow::new(
            &v1::CEILINGS,
            v1::ForgeRowParts {
                number: 3,
                kind: v1::Kind::Forgejo,
                mode: v1::Mode::Owned,
                connection: v1::Connection::Connecting,
                offer: v1::Offer::NotOffered,
                first: Some(first(state)),
                installations: skein_lib::List::with_capacity(0),
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
        .expect("record within schema ceilings");
        let mut rows = skein_lib::List::with_capacity(1);
        rows.push(row).expect("one row capacity");
        let rows =
            v1::Forges::new(&v1::CEILINGS, v1::ForgesParts { forges: rows }).expect("record within schema ceilings");
        assert_eq!(wire::decode_forges(&wire::forges(&rows), &v1::CEILINGS), Some(rows));
    }
}

#[test]
fn first_repository_names_are_bounded_at_construction_and_decode() {
    // Tiny receiving bounds exercise both sides of each new byte field without a large fixture.
    for size in [2, 3] {
        let name = vec![b'x'; size].into_boxed_slice();
        let mut limits = v1::CEILINGS;
        limits.set_up_org = 2;
        limits.set_up_repository = 2;
        limits.new_project_name = 2;
        limits.first_repository_org = 2;
        limits.first_repository_repository = 2;
        for field in 0..3 {
            let parts = v1::SetUpParts {
                forge: 3,
                org: if field == 0 { name.clone() } else { b"o".as_slice().into() },
                repository: if field == 1 { name.clone() } else { b"r".as_slice().into() },
                project: if field == 2 {
                    v1::SetUpProject::New(
                        v1::NewProject::new(&v1::CEILINGS, v1::NewProjectParts { name: name.clone() })
                            .expect("record within schema ceilings"),
                    )
                } else {
                    project()
                },
            };
            assert_eq!(v1::SetUp::new(&limits, parts.clone()).is_ok(), size == 2, "setup field {field}");
            let record = v1::SetUp::new(&v1::CEILINGS, parts).expect("record within schema ceilings");
            let mut writer = Writer::new(usize::try_from(record.measure()).expect("measured length fits usize"));
            record.encode(&mut writer).expect("measured room");
            assert_eq!(
                v1::SetUp::decode(&limits, &mut Reader::new(&writer.finish())).is_ok(),
                size == 2,
                "setup field {field}"
            );
        }
        for field in 0..2 {
            let parts = v1::FirstRepositoryParts {
                org: if field == 0 { name.clone() } else { b"o".as_slice().into() },
                repository: if field == 1 { name.clone() } else { b"r".as_slice().into() },
                project: project(),
                state: v1::FirstState::Making,
                label: Box::from(b"temper-host".as_slice()),
            };
            assert_eq!(v1::FirstRepository::new(&limits, parts.clone()).is_ok(), size == 2, "row field {field}");
            let record = v1::FirstRepository::new(&v1::CEILINGS, parts).expect("record within schema ceilings");
            let mut writer = Writer::new(usize::try_from(record.measure()).expect("measured length fits usize"));
            record.encode(&mut writer).expect("measured room");
            assert_eq!(
                v1::FirstRepository::decode(&limits, &mut Reader::new(&writer.finish())).is_ok(),
                size == 2,
                "row field {field}"
            );
        }
    }
    for size in [1024, 1025] {
        let name = vec![b'x'; size].into_boxed_slice();
        assert_eq!(
            v1::NewProject::new(&v1::CEILINGS, v1::NewProjectParts { name: name.clone() }).is_ok(),
            size == 1024
        );
        for org in [true, false] {
            let parts = v1::SetUpParts {
                forge: 3,
                org: if org { name.clone() } else { b"o".as_slice().into() },
                repository: if org { b"r".as_slice().into() } else { name.clone() },
                project: project(),
            };
            assert_eq!(v1::SetUp::new(&v1::CEILINGS, parts).is_ok(), size == 1024);
            let parts = v1::FirstRepositoryParts {
                org: if org { name.clone() } else { b"o".as_slice().into() },
                repository: if org { b"r".as_slice().into() } else { name.clone() },
                project: project(),
                state: v1::FirstState::Making,
                label: Box::from(b"temper-host".as_slice()),
            };
            assert_eq!(v1::FirstRepository::new(&v1::CEILINGS, parts).is_ok(), size == 1024);
        }
    }
}

#[test]
fn demo_start_outcomes_round_trip_at_each_boundary() {
    let limits = v1::CEILINGS;
    let mut outcomes = skein_lib::List::with_capacity(8);
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
        let bytes = temper_client_protocol_wire::answer(&record);
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
    let bytes = temper_client_protocol_wire::answer(&record);
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
    let limits = v1::CEILINGS;
    for state in [v1::DemoState::NotStarted, v1::DemoState::Running] {
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
            let bytes = temper_client_protocol_wire::demo_answer(&record);
            assert_eq!(temper_client_protocol_wire::decode_demo_answer(&bytes, &limits), Some(record));
        }
    }
    let record =
        v1::Requesting::new(&limits, v1::RequestingParts { request: v1::Request::StartDemo }).expect("start request");
    assert_eq!(
        temper_client_protocol_wire::decode_requesting(&temper_client_protocol_wire::requesting(&record), &limits),
        Some(record)
    );
}

#[test]
fn reachable_and_page_goldens_round_trip_with_ids_privacy_and_more() {
    use temper_client_protocol_wire as wire;
    for bytes in [
        include_bytes!("../../../crates/temper-client-protocol-wire/golden/enum_request_reachable_smallest.bin")
            .as_slice(),
        include_bytes!("../../../crates/temper-client-protocol-wire/golden/enum_request_reachable_full.bin").as_slice(),
    ] {
        let mut reader = Reader::new(bytes);
        let request = v1::Request::decode(&v1::CEILINGS, &mut reader).expect("reachable golden");
        assert!(reader.is_empty());
        let mut writer = Writer::new(bytes.len());
        request.encode(&mut writer).expect("measured room");
        assert_eq!(writer.finish().as_ref(), bytes);
    }
    for bytes in [
        include_bytes!("../../../crates/temper-client-protocol-wire/golden/enum_outcome_page_smallest.bin").as_slice(),
        include_bytes!("../../../crates/temper-client-protocol-wire/golden/enum_outcome_page_full.bin").as_slice(),
    ] {
        let mut reader = Reader::new(bytes);
        let outcome = v1::Outcome::decode(&v1::CEILINGS, &mut reader).expect("page golden");
        assert!(reader.is_empty());
        let mut writer = Writer::new(bytes.len());
        outcome.encode(&mut writer).expect("measured room");
        assert_eq!(writer.finish().as_ref(), bytes);
        let answer = v1::Answer::new(&v1::CEILINGS, v1::AnswerParts { outcome }).expect("answer");
        let encoded = wire::answer(&answer);
        assert_eq!(v1::Answer::decode(&v1::CEILINGS, &mut Reader::new(&encoded)), Ok(answer));
    }
}
