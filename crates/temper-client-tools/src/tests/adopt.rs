use super::fixtures;
use jig_client::{self as client, contract as c};
use temper_client_protocol_wire::v1 as w;
#[test]
fn adopt_prints_adopted_in_the_forges_words() {
    let (mut root, call) = fixtures::adopt_root(&[]);
    root.adoption_answer(call, fixtures::adopted_with_unknown_protection());
    let printed = root.answer(call).expect("answered once durable");
    assert_eq!((printed.status, &*printed.stdout), (0, fixtures::golden("adopt_adopted").as_ref()));
}
#[test]
fn adopt_json_preserves_the_adopted_record() {
    let (mut root, call) = fixtures::adopt_root(fixtures::JSON_FLAG);
    root.adoption_answer(call, fixtures::adopted_with_unknown_protection());
    let printed = root.answer(call).expect("answered once durable");
    assert_eq!((printed.status, printed.stdout.as_ref()), (0, b"{\"adopted\":{\"forge\":1,\"repository\":7,\"owner\":\"o\",\"name\":\"n\",\"role\":\"owned\",\"home\":true,\"kinds\":{\"read\":true,\"push\":true,\"open\":true,\"land\":true,\"review\":true,\"status\":true,\"comment\":true,\"issue\":true,\"branch\":true},\"protection\":{\"unknown\":{}}}}".as_slice()));
}
#[test]
fn each_broken_mark_exits_1_with_nothing_sent() {
    for (extra, expected) in [
        (&["--forge", "0"][..], b"A forge number is from 1 to 65535.".as_slice()),
        (&["--check", "0"][..], b"Checks are numbers from 1, each once.".as_slice()),
        (&["--check", "1", "--check", "1"][..], b"Checks are numbers from 1, each once.".as_slice()),
        (&["--no-ci"][..], b"Without CI, name at least one check.".as_slice()),
    ] {
        let (mut root, call) = fixtures::adopt_root(extra);
        let printed = root.answer(call).expect("local form failure");
        assert_eq!(printed.status, 1, "{extra:?}");
        assert!(skein_lib::bytes::find(&printed.stderr, expected).is_some(), "{extra:?}");
        assert!(root.nothing_sent(), "{extra:?}");
    }
}
#[test]
fn long_form_values_do_not_send_retained_defaults() {
    use crate::jig::call::{Value, Values};
    for field in [2, 3, 4, 6] {
        for (extra, sends) in [(fixtures::EMPTY, true), (fixtures::FIELD_OVERFLOW, false)] {
            let mut root = fixtures::signed_in(fixtures::PARTY);
            let mut call = fixtures::adoption();
            *call.values.get_mut(field).expect("text field") =
                Some(Values::One(Value::Text(fixtures::long_form_value(extra))));
            root.submit_call(call, false);
            assert_eq!(root.nothing_sent(), !sends, "field {field}, extra {extra}");
        }
    }
}
#[test]
fn absent_adoption_flags_use_the_forms_defaults() {
    let (root, call) = fixtures::adopt_root(&[]);
    let a = root.sent_adoption(call);
    assert_eq!(
        (a.forge, a.home, a.role, a.prefix.as_ref(), a.ci, a.checks.len()),
        (1, true, w::Role::Owned, b"temper/".as_slice(), true, fixtures::EMPTY)
    );
}
#[test]
fn context_adoption_clears_home() {
    let (root, call) = fixtures::adopt_root(&["--role", "context"]);
    let a = root.sent_adoption(call);
    assert_eq!((a.role, a.home), (w::Role::Context, false));
}
#[test]
fn supplied_adoption_flags_reach_the_form() {
    let (root, call) = fixtures::adopt_root(&[
        "--forge",
        "2",
        "--role",
        "fork",
        "--prefix",
        "branch/",
        "--no-home",
        "--no-ci",
        "--check",
        "3",
        "--check",
        "5",
    ]);
    let a = root.sent_adoption(call);
    assert_eq!(
        (a.forge, a.role, a.home, a.ci, a.prefix.as_ref(), a.checks.as_slice()),
        (2, w::Role::Fork, false, false, b"branch/".as_slice(), &[3, 5][..])
    );
}
#[test]
fn each_forge_refusal_exits_2_in_its_words() {
    for (reason, expected) in [
        (w::ForgeReason::NoSuchForge, b"No forge 1 is configured.\n".as_slice()),
        (w::ForgeReason::NotFound, b"No repository o/n on forge 1 that temper's forge user can see.\n".as_slice()),
        (w::ForgeReason::NoAccess, b"temper's forge user may not read o/n on forge 1.\n".as_slice()),
        (w::ForgeReason::Refused, b"The forge connector refused it: another project writes o/n, the project has a home already, or branches under temper/ are not this deployment's.\n".as_slice()),
        (w::ForgeReason::InFlight, b"o/n is being adopted already; try again when that ends.\n".as_slice()),
        (w::ForgeReason::TooLarge, b"A field is past the forge's bounds, or the forge's repositories are full.\n".as_slice()),
        (w::ForgeReason::Unreachable, b"Forge 1 did not answer; nothing was adopted. Try again.\n".as_slice()),
        (w::ForgeReason::InvalidAnswer, b"Forge 1 answered what temper did not expect; nothing was adopted.\n".as_slice()),
    ] {
        let (mut root, call) = fixtures::adopt_root(&[]);
        root.adoption_answer(call, fixtures::forge_refused(reason.clone()));
        let printed = root.answer(call).expect("refused durably");
        assert_eq!((printed.status, printed.stderr.as_ref()), (2, expected), "{reason:?}");
    }
}
#[test]
fn forge_refusal_has_its_typed_json_outcome() {
    let (mut root, call) = fixtures::adopt_root(fixtures::JSON_FLAG);
    root.adoption_answer(call, fixtures::forge_refused(w::ForgeReason::TooLarge));
    let printed = root.answer(call).expect("refused");
    assert_eq!(
        (printed.status, printed.stderr.as_ref()),
        (2, b"{\"forge_refused\":{\"reason\":\"too_large\"}}".as_slice())
    );
}
#[test]
fn unknown_past_the_retention_exits_1() {
    let (mut root, call) = fixtures::adopt_root(&[]);
    root.adopt_outcome(call, client::Outcome::Unknown);
    let printed = root.answer(call).expect("unknown answered");
    assert_eq!(
        (printed.status, printed.stderr.as_ref()),
        (1, b"It is not known whether o/n was adopted.\n".as_slice())
    );
}
#[test]
fn adopts_client_failures_exit_1() {
    for (outcome, reason) in [
        (client::Outcome::Held, b"name held by another request".as_slice()),
        (client::Outcome::Limit, b"request limit".as_slice()),
        (client::Outcome::NotKept, b"request not kept".as_slice()),
        (client::Outcome::Unconfirmed, b"tools' bug; unconfirmed request; report it as a finding".as_slice()),
    ] {
        let (mut root, call) = fixtures::adopt_root(&[]);
        root.adopt_outcome(call, outcome);
        let printed = root.answer(call).expect("client failure answered");
        assert_eq!(printed.status, 1);
        assert!(skein_lib::bytes::find(&printed.stderr, reason).is_some(), "{reason:?}");
    }
}
#[test]
fn a_named_adoption_has_identical_request_bytes_under_two_clocks() {
    let (first, call) = fixtures::adopt_root(&[]);
    let mut second = fixtures::started_with(fixtures::adoption_prefix_restore(), fixtures::OTHER_SEED, false);
    second.deliver(c::Delivery::Me(Box::new(fixtures::me(fixtures::PARTY))));
    second.now = skein_lib::Time::from_nanos(fixtures::LATER);
    second.wall = skein_lib::Wall::from_nanos(fixtures::LATER);
    let other = second.submit(fixtures::ADOPTION);
    assert_eq!(
        (first.adoption_bytes(call), first.sent_name(call)),
        (second.adoption_bytes(other), second.sent_name(other))
    );
}
#[test]
fn adopt_uses_the_roots_call_prefix_above_all_kept_keys() {
    let mut root = fixtures::started(fixtures::adoption_prefix_restore());
    root.deliver(c::Delivery::Me(Box::new(fixtures::me(fixtures::PARTY))));
    let call = root.submit(fixtures::ADOPTION);
    let key =
        client::Key { asker: fixtures::forge_asker(), number: (u64::from(fixtures::RESTORED_PREFIX + 1) << 32u32) | 1 };
    assert!(root.root.client().pending(key).is_some());
    root.adoption_answer(call, fixtures::adopted_with_unknown_protection());
    assert_eq!(root.answer(call).expect("correlated call").status, 0);
}
#[test]
fn adoption_waits_for_live_me() {
    let mut root = fixtures::started(skein_lib::List::with_capacity(fixtures::EMPTY));
    let call = root.submit(fixtures::ADOPTION);
    assert!(root.nothing_sent());
    root.deliver(c::Delivery::Me(Box::new(fixtures::me(fixtures::PARTY))));
    assert_eq!(root.sent_adoption(call).project, fixtures::PROJECT);
}
#[test]
fn a_restored_adoption_prints_its_outcome_once() {
    for (json, expected) in [
        (false, fixtures::golden("adopt_refused")),
        (
            true,
            Box::from(
                b"{\"operation\":\"adopt\",\"outcome\":{\"forge_refused\":{\"reason\":\"too_large\"}}}\n".as_slice(),
            ),
        ),
    ] {
        let mut root = fixtures::started_with(fixtures::restored_adoption(fixtures::PARTY), fixtures::SEED, json);
        root.deliver(c::Delivery::Me(Box::new(fixtures::me(fixtures::PARTY))));
        assert_eq!(root.root.kept(), fixtures::ADOPT_RESTORES);
        root.adoption_answer(fixtures::EMPTY, fixtures::forge_refused(w::ForgeReason::TooLarge));
        assert_eq!(root.root.kept(), fixtures::EMPTY);
        root.adopt_outcome(fixtures::RESTORED_PREFIX, client::Outcome::Unknown);
        assert_eq!(root.kept_lines.as_slice(), &[expected]);
    }
}
#[test]
fn another_partys_adoption_stays_parked() {
    let mut root = fixtures::started(fixtures::restored_adoption(fixtures::OTHER_PARTY));
    root.deliver(c::Delivery::Me(Box::new(fixtures::me(fixtures::PARTY))));
    assert_eq!(root.root.kept(), fixtures::EMPTY);
    assert!(root.nothing_sent());
    assert_eq!(
        root.kept_lines.as_slice(),
        &[Box::from(b"kept: 1 for party 74, unsent while it is not signed in here\n".as_slice())]
    );
    let call = root.submit(fixtures::ME_CALL);
    assert_eq!(root.answer(call).expect("unrelated read").status, 0);
}
#[test]
fn another_partys_adoption_name_is_held() {
    let mut root = fixtures::started(fixtures::restored_adoption(fixtures::OTHER_PARTY));
    root.deliver(c::Delivery::Me(Box::new(fixtures::me(fixtures::PARTY))));
    let words = fixtures::extended(fixtures::ADOPTION, fixtures::CHANGED_ADOPTION);
    let call = root.submit(words.as_slice());
    let printed = root.answer(call).expect("held name");
    assert_eq!((printed.status, printed.stderr.as_ref()), (1, b"name held by another request\n".as_slice()));
}
#[test]
fn a_task_reads_forge_resource_in_its_words() {
    let (mut root, call, task) = fixtures::part_read();
    root.serve(call, fixtures::task_result(task));
    let printed = root.answer(call).expect("task answered");
    assert_eq!((printed.status, printed.stdout.as_ref()), (0, fixtures::golden("forge_part").as_ref()));
}
#[test]
fn an_unserved_forge_part_prints_shown_on_the_forge() {
    let (mut root, call, mut task) = fixtures::part_read();
    let c::TaskResource::Owner(resource) = task.resources.get_mut(fixtures::EMPTY).expect("one resource") else {
        panic!("owner resource")
    };
    resource.part.kind = c::OwnerPartKind::Effect;
    root.serve(call, fixtures::task_result(task));
    let printed = root.answer(call).expect("task answered");
    assert!(skein_lib::bytes::find(&printed.stdout, b"Shown on the forge.").is_some());
}
#[test]
fn core_refusal_keeps_its_typed_outcome() {
    let (mut root, call) = fixtures::adopt_root(fixtures::JSON_FLAG);
    root.adoption_answer(call, fixtures::core_key_conflict());
    let printed = root.answer(call).expect("core refused durably");
    assert_eq!(
        (printed.status, printed.stderr.as_ref()),
        (2, b"{\"core_refused\":{\"refusal\":\"key_conflict\"}}".as_slice())
    );
}
#[test]
fn entrance_refusal_keeps_its_typed_outcome() {
    let (mut root, call) = fixtures::adopt_root(fixtures::JSON_FLAG);
    root.answered(call, c::Answer::Refused { refusal: c::Refusal::KeyConflict, at: None });
    let printed = root.answer(call).expect("entrance refused durably");
    assert_eq!(
        (printed.status, printed.stderr.as_ref()),
        (2, b"{\"entrance_refused\":{\"refusal\":\"key_conflict\"}}".as_slice())
    );
}
#[test]
fn malformed_adoption_answers_exit_1() {
    let (mut root, call) = fixtures::adopt_root(&[]);
    root.adopt_outcome(call, fixtures::malformed_adoption_answer());
    let printed = root.answer(call).expect("decoding failure");
    assert_eq!((printed.status, printed.stderr.as_ref()), (1, b"adoption answer decoding failed\n".as_slice()));
}
#[test]
fn client_key_collision_names_the_tools_bug() {
    let (mut root, _) = fixtures::adopt_root(&[]);
    crate::root::act_fixture::repeat_number(&mut root.root);
    let call = root.submit(fixtures::ADOPTION);
    let printed = root.answer(call).expect("collision answered locally");
    assert_eq!(
        (printed.status, printed.stderr.as_ref()),
        (1, b"tools' bug; client key collision; report it as a finding\n".as_slice())
    );
}
#[test]
fn required_adoption_fields_send_nothing_when_empty() {
    for field in fixtures::REQUIRED_FIELDS {
        let mut root = fixtures::signed_in(fixtures::PARTY);
        let call = root.submit_call(fixtures::blank_adoption(*field), false);
        let printed = root.answer(call).expect("required mark");
        assert_eq!((printed.status, printed.stderr.as_ref()), (1, b"Required.\n".as_slice()), "field {field}");
        assert!(root.nothing_sent(), "field {field}");
    }
}
#[test]
fn a_forge_branch_path_stays_whole_past_the_text_window() {
    let (mut root, call, task, expected) = fixtures::branch_part_read();
    root.serve(call, fixtures::task_result(task));
    let printed = root.answer(call).expect("branch resource");
    assert!(skein_lib::bytes::find(&printed.stdout, &expected).is_some());
}
#[test]
fn another_partys_adoption_json_counts_unsent() {
    let mut root = fixtures::started_with(fixtures::restored_adoption(fixtures::OTHER_PARTY), fixtures::SEED, true);
    root.deliver(c::Delivery::Me(Box::new(fixtures::me(fixtures::PARTY))));
    assert_eq!(root.kept_lines.as_slice(), &[Box::from(b"{\"party\":74,\"unsent\":1}\n".as_slice())]);
}
#[test]
fn a_restored_adoption_with_a_drawn_engine_key_keeps_its_context() {
    let mut records = fixtures::restored_adoption(fixtures::PARTY);
    records.get_mut(fixtures::EMPTY).expect("one record").key = fixtures::DRAWN_ADOPTION_KEY;
    let mut root = fixtures::started(records);
    root.deliver(c::Delivery::Me(Box::new(fixtures::me(fixtures::PARTY))));
    root.adoption_answer(fixtures::EMPTY, w::Outcome::Unknown);
    assert_eq!(
        root.kept_lines.as_slice(),
        &[Box::from(b"kept: adopt It is not known whether o/n was adopted.\n".as_slice())]
    );
}

#[test]
fn a_saved_setup_request_is_not_restored_as_an_adoption() {
    let mut records = fixtures::restored_adoption(fixtures::PARTY);
    let held = records.get_mut(fixtures::EMPTY).expect("one saved request");
    let client::Ask::Family { record, .. } = &mut held.request.request else { panic!("family request") };
    let request = w::Requesting::new(
        &w::CEILINGS,
        w::RequestingParts {
            request: w::Request::SetUp(
                w::SetUp::new(
                    &w::CEILINGS,
                    w::SetUpParts {
                        playground: false,
                        forge: 3,
                        org: b"work".as_slice().into(),
                        repository: b"hello".as_slice().into(),
                        project: w::SetUpProject::Existing(
                            w::ExistingProject::new(&w::CEILINGS, w::ExistingProjectParts { project: 1 })
                                .expect("project"),
                        ),
                    },
                )
                .expect("forge"),
            ),
        },
    )
    .expect("setup request");
    record.bytes = temper_client_protocol_wire::requesting(&request);
    let mut root = fixtures::started(records);
    root.deliver(c::Delivery::Me(Box::new(fixtures::me(fixtures::PARTY))));
    root.adopt_outcome(fixtures::RESTORED_PREFIX, client::Outcome::Unknown);
    assert!(root.kept_lines.is_empty(), "a setup request has no adoption outcome to print");
}
