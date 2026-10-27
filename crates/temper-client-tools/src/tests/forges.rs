use super::fixtures as f;
use crate::{
    forges,
    jig::{self, call::Parsed},
};
use jig_client::{self as client, contract as c};
use skein_lib::{Duration, List};
use temper_client_domain::forge::{
    self,
    forges::{Connection as C, Kind, Mode, Offer, Row, Step},
};
use temper_client_protocol_wire::{self as wire, v1 as w};
pub(super) fn row(connection: C) -> Row {
    Row {
        number: 3,
        kind: Kind::Forgejo,
        mode: Mode::Owned,
        connection,
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
    }
}
fn record(connection: w::Connection) -> client::Family {
    record_first(connection, None)
}
fn record_first(connection: w::Connection, first: Option<w::FirstRepository>) -> client::Family {
    let limits = forge::wire_limits(&f::limits_root().domain.forge).unwrap();
    let mut forges = List::with_capacity(1);
    forges
        .push(
            w::ForgeRow::new(
                &limits,
                w::ForgeRowParts {
                    number: 3,
                    kind: w::Kind::Forgejo,
                    mode: w::Mode::Owned,
                    connection,
                    offer: w::Offer::Offered,
                    first,
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
    client::Family {
        family: wire::FORGE,
        version: wire::CLIENT_VERSION,
        bytes: wire::forges(&w::Forges::new(&limits, w::ForgesParts { forges }).unwrap()),
    }
}
fn deliver(root: &mut f::Harness, record: client::Family) {
    let subject = forge::forges::watch(&root.limits.domain.forge).unwrap();
    let token = root.watch_token(&subject);
    root.client(client::Event::Up { up: client::Up::FamilyDelivered { watch: token, record } });
}
#[test]
fn forge_calls_round_trip_through_argv_json_and_help() {
    for argv in [
        &["forge", "list"][..],
        &["forge", "setup", "3"],
        &["forge", "setup", "3", "--key", "setup"],
        &["forge", "setup", "3", "--org", "team", "--repository", "widget", "--project", "shop"],
        &["wait", "forges", "--until", "made:3"],
        &["forge", "setup", "3", "--project", ""],
        &["wait", "forges", "--until", "settled"],
        &["wait", "forges", "--since", "0123456789abcdef", "--until", "connected:3", "--timeout", "2s"],
    ] {
        let call = f::call(argv);
        assert_eq!(f::printed(&call), Parsed::Call { call: call.clone(), json: false });
        let json = jig::json::write_call(crate::table::TOOLS, &call).unwrap();
        assert_eq!(f::from_json(&json), call);
        let help = f::help(Some(call.tool));
        assert!(skein_lib::bytes::find(&help, argv[0].as_bytes()).is_some());
    }
    for argv in [
        &["forge", "setup"][..],
        &["forge", "setup", "oops"],
        &["forge", "setup", "65536"],
        &["forge", "setup", "0"],
        &["wait", "forges", "--until", "busy"],
        &["wait", "forges", "--until", "made:x"],
        &["wait", "forges", "--until", "made:0"],
        &["wait", "forges", "--until", "made:65536"],
        &["wait", "forges", "--until", "made:"],
        &["wait", "forges", "--until", "connected:x"],
        &["wait", "forges", "--until", "connected:65536"],
        &["wait", "forges", "--until", "connected:0"],
        &["wait", "forges", "--until", "connected:"],
        &["wait", "forges", "--until", "connected:18446744073709551616"],
    ] {
        assert!(f::parse(argv).is_err(), "{argv:?}");
    }
}
#[test]
fn every_state_renders_as_text_and_structured_json() {
    let cases: &[(C, &[u8], &[u8])] = &[
        (C::NotConnected, b"Not connected", br#""state":"not_connected""#),
        (C::NotSetUp, b"Not set up", br#""state":"not_set_up""#),
        (C::SettingUp { step: None }, b"Setting up", br#""state":"setting_up""#),
        (C::SettingUp { step: Some(Step::Fetch) }, b"Setting up: fetching", br#""state":"setting_up","step":"fetch""#),
        (
            C::SettingUp { step: Some(Step::Prepare) },
            b"Setting up: preparing",
            br#""state":"setting_up","step":"prepare""#,
        ),
        (C::SettingUp { step: Some(Step::Run) }, b"Setting up: starting", br#""state":"setting_up","step":"run""#),
        (
            C::SetUpFailed { why: b"fetch failed".as_slice().into(), at: None },
            b"Set up failed: fetch failed",
            br#""state":"set_up_failed","why":"fetch failed""#,
        ),
        (C::Connecting, b"Connecting", br#""state":"connecting""#),
        (
            C::Connected { id: 42, login: b"bot".as_slice().into() },
            b"Connected as bot (42)",
            br#""state":"connected","id":42,"login":"bot""#,
        ),
        (
            C::Stopped { reason: b"retired".as_slice().into() },
            b"Stopped: retired",
            br#""state":"stopped","reason":"retired""#,
        ),
    ];
    for (state, words, fields) in cases {
        let rows = [row(state.clone())];
        let plain = forges::print_rows(Some(&rows), false, None, &f::limits_root());
        let json = forges::print_rows(Some(&rows), true, None, &f::limits_root());
        assert_eq!(plain.status, 0);
        assert_eq!(json.status, 0);
        let document = concat(&[
            br#"{"forges":[{"number":3,"kind":"forgejo","read_only":false,"mode":"owned","offer":"offered","#,
            fields,
            br#","installations":[],"ended":null,"first":null}]}"#,
        ]);
        let digest = jig::digest::Digest::of(&document).hex();
        let expected = concat(&[
            b"forge 3 \xc2\xb7 Forgejo \xc2\xb7 owned \xc2\xb7 ",
            words,
            b"\ndigest: forges ",
            &digest,
            b"\n",
        ]);
        assert_eq!(&*plain.stdout, expected.as_ref(), "{state:?}");
        let expected = concat(&[
            document.get(..document.len().checked_sub(1).unwrap()).unwrap(),
            b",\"digest\":\"",
            &digest,
            b"\"}",
        ]);
        assert_eq!(&*json.stdout, expected.as_ref(), "{state:?}");
    }
    let mut without = row(C::NotSetUp);
    without.offer = Offer::NotUnderTheBox;
    let plain = forges::print_rows(Some(&[without.clone()]), false, None, &f::limits_root());
    assert!(skein_lib::bytes::find(&plain.stdout, b"temper forge fetch 3").is_some());
    let json = forges::print_rows(Some(&[without]), true, None, &f::limits_root());
    assert!(skein_lib::bytes::find(&json.stdout, b"not_under_the_box").is_some());
}
#[test]
fn settled_excludes_setting_up_and_connecting_and_connected_names_a_forge() {
    for state in [
        C::NotConnected,
        C::NotSetUp,
        C::SetUpFailed { why: Box::default(), at: None },
        C::Stopped { reason: Box::default() },
        C::Connected { id: 1, login: Box::default() },
    ] {
        assert!(forges::holds(&[row(state)], b"settled"));
    }
    for state in [C::SettingUp { step: None }, C::SettingUp { step: Some(Step::Run) }, C::Connecting] {
        assert!(!forges::holds(&[row(C::NotConnected), row(state)], b"settled"));
    }
    let rows = [row(C::Connected { id: 1, login: Box::default() })];
    assert!(forges::holds(&rows, b"connected:3"));
    assert!(!forges::holds(&rows, b"connected:2"));
    assert!(!forges::holds(&[row(C::NotConnected)], b"connected:3"));
    assert!(forges::holds(&[], b"settled"));
}
#[test]
fn the_read_closes_after_its_first_document_and_refuses_invalid_bytes() {
    for (bytes, status) in [
        (record(w::Connection::NotSetUp), 0),
        (client::Family { bytes: b"bad".as_slice().into(), ..record(w::Connection::NotSetUp) }, 1),
    ] {
        let mut root = f::signed_in(f::PARTY);
        let call = root.submit(&["forge", "list"]);
        deliver(&mut root, bytes);
        assert_eq!(root.answer(call).unwrap().status, status);
        assert_eq!(root.closed.len(), 1);
        assert_eq!(root.watches.len(), 1, "only the runner's me remains");
        assert!(crate::root::next_deadline(&root.root).is_none());
    }
}
#[test]
fn a_wait_follows_each_setup_step_and_releases_the_watch_and_timer() {
    let mut root = f::signed_in(f::PARTY);
    let call = root.submit(&["wait", "forges", "--until", "settled"]);
    let limits = forge::wire_limits(&root.limits.domain.forge).unwrap();
    for step in [None, Some(w::SetUpStep::Prepare), Some(w::SetUpStep::Run)] {
        deliver(
            &mut root,
            record(w::Connection::SettingUp(w::SettingUp::new(&limits, w::SettingUpParts { step }).unwrap())),
        );
        assert!(root.answer(call).is_none());
    }
    deliver(&mut root, record(w::Connection::Connecting));
    assert!(root.answer(call).is_none());
    deliver(&mut root, record(w::Connection::NotConnected));
    let printed = root.answer(call).unwrap();
    assert_eq!(printed.status, 0);
    assert!(printed.stdout.ends_with(b"met\n"));
    assert_eq!(root.closed.len(), 1);
    assert!(crate::root::next_deadline(&root.root).is_none());
}
#[test]
fn since_uses_the_read_digest_and_plain_wait_requires_a_change() {
    for until in [false, true] {
        let mut root = f::signed_in(f::PARTY);
        let read = root.submit(&["forge", "list", "--json"]);
        deliver(&mut root, record(w::Connection::NotSetUp));
        let printed = root.answer(read).unwrap();
        let digest = f::last_digest(&printed.stdout);
        let flags = if until { &["--until", "settled"][..] } else { &[] };
        let call = root.wait_since(&["wait", "forges"], flags, &digest);
        deliver(&mut root, record(w::Connection::NotSetUp));
        assert!(root.answer(call).is_none());
        deliver(&mut root, record(w::Connection::NotConnected));
        assert_eq!(root.answer(call).unwrap().status, 0);
    }
    let mut root = f::signed_in(f::PARTY);
    let call = root.submit(&["wait", "forges"]);
    deliver(&mut root, record(w::Connection::NotSetUp));
    assert!(root.answer(call).is_none());
    deliver(&mut root, record(w::Connection::NotSetUp));
    assert!(root.answer(call).is_none());
    deliver(&mut root, record(w::Connection::NotConnected));
    assert_eq!(root.answer(call).unwrap().status, 0);
}
#[test]
fn timeout_prints_the_current_document_and_watch_refusals_keep_their_status() {
    let mut root = f::signed_in(f::PARTY);
    let call = root.submit(&["wait", "forges", "--until", "connected:3", "--timeout", "1s", "--json"]);
    deliver(&mut root, record(w::Connection::NotSetUp));
    root.now = root.now.checked_add(Duration::from_secs(1)).unwrap();
    root.fire();
    let printed = root.answer(call).unwrap();
    assert_eq!(printed.status, 3);
    assert!(f::json_has(&printed, br#""state":"not_set_up""#));
    assert!(f::json_has(&printed, br#""met":false,"why":"timeout""#));
    assert_eq!(root.closed.len(), 1);
    for (end, status) in [(c::WatchEnd::NotAllowed, 2), (c::WatchEnd::Unknown, 2)] {
        let mut root = f::signed_in(f::PARTY);
        let call = root.submit(&["wait", "forges", "--until", "settled"]);
        let token = root.watch_token(&forge::forges::watch(&root.limits.domain.forge).unwrap());
        root.client(client::Event::Up { up: client::Up::WatchEnded { watch: token, end } });
        assert_eq!(root.answer(call).unwrap().status, status);
        assert!(crate::root::next_deadline(&root.root).is_none());
    }
}
#[test]
fn setup_answers_print_their_words_json_and_status() {
    let limits = forge::wire_limits(&f::limits_root().domain.forge).unwrap();
    for (outcome, expected, status) in [
        (w::Outcome::SettingUp, b"setting up\n".as_slice(), 0),
        (w::Outcome::Unknown, b"unknown\n", 2),
        (w::Outcome::SetUpRefused(w::SetUpRefused::new(&limits, w::SetUpRefusedParts { why: w::SetUpRefusal::Busy }).unwrap()), b"refused: busy\n", 2),
        (w::Outcome::SetUpRefused(w::SetUpRefused::new(&limits, w::SetUpRefusedParts { why: w::SetUpRefusal::NotOffered }).unwrap()), b"refused: not offered\n", 2),
        (w::Outcome::SetUpRefused(w::SetUpRefused::new(&limits, w::SetUpRefusedParts { why: w::SetUpRefusal::NotUnderTheBox }).unwrap()), b"refused: not under the box\nrun `temper forge fetch 3`, `temper forge prepare 3`, `temper forge run 3 forgejo` and `temper forge run 3 runner`, then restart `temper serve`\n", 2),
    ] {
        for json in [false, true] {
            let mut root = f::signed_in(f::PARTY);
            let call = root.submit_prepared(f::call(&["forge", "setup", "3"]), json, None);
            let client::Ask::Family { scope, record } = root.adoption_bytes(call) else { panic!("family") };
            assert_eq!(*scope, c::Scope::Deployment);
            let w::Request::SetUp(request) = wire::decode_requesting(&record.bytes, &limits).unwrap().into_parts().request else { panic!("set up"); };
            assert_eq!(request.forge(), 3);
            root.adoption_answer(call, outcome.clone());
            if status == 0 {
                assert!(root.answer(call).is_none());
                assert!(root.progress.as_slice().last().is_some());
                continue;
            }
            let printed = root.answer(call).unwrap();
            assert_eq!(printed.status, status);
            let bytes = if status == 0 { &printed.stdout } else { &printed.stderr };
            if json { assert!(bytes.starts_with(b"{")); } else { assert_eq!(bytes.as_ref(), expected); }
        }
    }
}

#[test]
fn setup_keeps_drawn_keys_and_recovers_the_answer_in_its_own_words() {
    for json in [false, true] {
        let mut root = f::signed_in(f::PARTY);
        let call = root.submit(&["forge", "setup", "3"]);
        assert!(root.saved.contains_key(&call), "a drawn key is durable too");
        let key = root.sent_name(call).unwrap();
        let mut records = f::restored_adoption(f::PARTY);
        let record = records.get_mut(0).unwrap();
        record.key = key;
        record.request.request = root.adoption_bytes(call).clone();
        let mut root = f::started_with(records, f::SEED, json);
        root.deliver(c::Delivery::Me(Box::new(f::me(f::PARTY))));
        root.adoption_answer(0, w::Outcome::SettingUp);
        let expected = if json {
            b"{\"operation\":\"forge-setup\",\"outcome\":{\"setting_up\":{}}}\n".as_slice()
        } else {
            b"kept: forge-setup setting up\n"
        };
        assert_eq!(root.kept_lines.as_slice(), &[Box::<[u8]>::from(expected)]);
    }
}
#[test]
fn setup_names_are_stable_and_unconfirmed_calls_wait_for_me() {
    let mut root = f::started(List::with_capacity(0));
    let call = root.submit(&["forge", "setup", "3", "--key", "start-here"]);
    assert!(root.nothing_sent());
    root.deliver(c::Delivery::Me(Box::new(f::me(f::PARTY))));
    assert_eq!(root.sent_name(call), Some(crate::key::engine(&f::name(b"start-here"))));
    let mut other = f::signed_in(f::PARTY);
    let next = other.submit(&["forge", "setup", "3", "--key", "start-here"]);
    assert_eq!(root.sent_name(call), other.sent_name(next));
    assert_eq!(root.adoption_bytes(call), other.adoption_bytes(next));
}
#[test]
fn a_set_up_rejects_an_adoption_answer_and_preserves_entrance_refusals() {
    let outcome = client::Outcome::Answered(f::family_answer(f::adopted_with_unknown_protection()));
    assert_eq!(forges::setup_print(3, &outcome, false, false, &f::limits_root()).status, 1);
    let outcome = client::Outcome::Answered(client::Answered::Jig(c::Answer::Refused {
        refusal: c::Refusal::Role { held: None },
        at: None,
    }));
    for json in [false, true] {
        assert_eq!(forges::setup_print(3, &outcome, json, false, &f::limits_root()).status, 2);
    }
}

#[test]
fn all_children_share_the_call_budget_and_timeouts_release_it() {
    let mut root = f::started(List::with_capacity(0));
    // Calls wait for me, so no stream-table bound masks the root's call bound.
    let mut calls = List::with_capacity(root.limits.tools.rows);
    for at in 0..root.limits.tools.rows {
        let argv = if at % 2 == 0 {
            &["wait", "forges", "--timeout", "1s"][..]
        } else {
            &["wait", "accounts", "--timeout", "1s"]
        };
        calls.push(root.submit(argv)).unwrap();
    }
    let excess = root.submit(&["forge", "setup", "3", "--json"]);
    let printed = root.answer(excess).unwrap();
    assert_eq!(printed.status, 1);
    assert_eq!(printed.stderr.as_ref(), br#"{"error":"call limit"}"#);
    root.deliver(c::Delivery::Me(Box::new(f::me(f::PARTY))));
    root.now = root.now.checked_add(Duration::from_secs(1)).unwrap();
    root.fire();
    for call in &calls {
        assert!(root.answer(*call).is_some());
    }
    let next = root.submit(&["forge", "setup", "3"]);
    assert!(root.sends.contains_key(&next));
}

fn concat(parts: &[&[u8]]) -> Box<[u8]> {
    let mut size = 0usize;
    for part in parts {
        size = size.checked_add(part.len()).unwrap();
    }
    let mut out = skein_lib::Writer::new(size);
    for part in parts {
        out.put(part).unwrap();
    }
    out.finish()
}

fn first_record(state: w::FirstState) -> w::FirstRepository {
    w::FirstRepository::new(
        &w::CEILINGS,
        w::FirstRepositoryParts {
            playground: false,
            org: b"work".as_slice().into(),
            repository: b"hello".as_slice().into(),
            project: w::SetUpProject::Existing(
                w::ExistingProject::new(&w::CEILINGS, w::ExistingProjectParts { project: 1 }).unwrap(),
            ),
            state,
            label: b"docker".as_slice().into(),
        },
    )
    .unwrap()
}
fn made() -> w::FirstState {
    w::FirstState::Made(w::Made::new(&w::CEILINGS, w::MadeParts { id: 42 }).unwrap())
}
#[test]
fn setup_defaults_flags_and_project_names_are_carried_in_the_request() {
    // Group, verb, forge, flag and value.
    const WORDS_WITH_ONE_FLAG: u32 = 5;
    for (flags, org, repository, project) in [
        (&[][..], b"work".as_slice(), b"hello".as_slice(), 0),
        (&["--org", "team"][..], b"team", b"hello", 0),
        (&["--repository", "widget"][..], b"work", b"widget", 0),
        (&["--project", "7"][..], b"work", b"hello", 7),
        (&["--project", "shop"][..], b"work", b"hello", f::PROJECT),
    ] {
        let mut root = f::signed_in(f::PARTY);
        let mut words = List::with_capacity(WORDS_WITH_ONE_FLAG);
        words.push("forge").unwrap();
        words.push("setup").unwrap();
        words.push("3").unwrap();
        for word in flags {
            words.push(*word).unwrap();
        }
        let call = root.submit(words.as_slice());
        let client::Ask::Family { record, .. } = root.adoption_bytes(call) else { panic!("family") };
        let w::Request::SetUp(value) =
            wire::decode_requesting(&record.bytes, &w::CEILINGS).unwrap().into_parts().request
        else {
            panic!("setup")
        };
        assert_eq!((value.org(), value.repository()), (org, repository));
        match value.project() {
            w::SetUpProject::Existing(value) => {
                assert_ne!(project, 0);
                assert_eq!(value.project(), project);
            }
            w::SetUpProject::New(value) => {
                assert_eq!(project, 0);
                assert_eq!(value.name(), repository);
            }
        }
    }
}
#[test]
fn an_unknown_project_is_sent_as_new_and_both_new_refusals_exit_two() {
    for (why, words) in [
        (w::SetUpRefusal::NewProject, b"a new project".as_slice()),
        (w::SetUpRefusal::Invalid, b"invalid: the organization, repository or project name is empty or too long"),
    ] {
        let mut root = f::signed_in(f::PARTY);
        let call = root.submit(&["forge", "setup", "3", "--project", "Next"]);
        let client::Ask::Family { record, .. } = root.adoption_bytes(call) else { panic!("family") };
        let w::Request::SetUp(value) =
            wire::decode_requesting(&record.bytes, &w::CEILINGS).unwrap().into_parts().request
        else {
            panic!("setup")
        };
        let w::SetUpProject::New(value) = value.project() else { panic!("new project") };
        assert_eq!(value.name(), b"Next");
        root.adoption_answer(
            call,
            w::Outcome::SetUpRefused(w::SetUpRefused::new(&w::CEILINGS, w::SetUpRefusedParts { why }).unwrap()),
        );
        let printed = root.answer(call).unwrap();
        assert_eq!(printed.status, 2);
        assert!(skein_lib::bytes::find(&printed.stderr, words).is_some());
    }
}
#[test]
fn made_sends_the_home_adoption_with_its_stable_key_and_prints_its_answer() {
    for home_taken in [false, true] {
        let mut root = f::signed_in(f::PARTY);
        let call =
            root.submit(&["forge", "setup", "3", "--org", "team", "--repository", "widget", "--project", "shop"]);
        root.adoption_answer(call, w::Outcome::SettingUp);
        assert!(root.answer(call).is_none());
        assert_eq!(root.progress.as_slice(), &[Box::<[u8]>::from(b"setting up\n".as_slice())]);
        deliver(&mut root, record_first(w::Connection::Connecting, Some(first_record(w::FirstState::Making))));
        assert!(root.answer(call).is_none());
        deliver(&mut root, record_first(w::Connection::Connecting, Some(first_record(made()))));
        assert!(root.answer(call).is_none(), "creation precedes the forge's connection");
        deliver(
            &mut root,
            record_first(
                w::Connection::Connected(
                    w::Connected::new(&w::CEILINGS, w::ConnectedParts { id: 1, login: b"temper".as_slice().into() })
                        .unwrap(),
                ),
                Some(first_record(made())),
            ),
        );
        let adopted = root.sent_adoption(call);
        assert_eq!((adopted.project, adopted.forge, adopted.home, adopted.ci), (f::PROJECT, 3, true, true));
        assert_eq!(
            (adopted.owner.as_ref(), adopted.name.as_ref(), adopted.landing.as_ref()),
            (b"team".as_slice(), b"widget".as_slice(), b"main".as_slice())
        );
        assert_eq!(adopted.role, w::Role::Owned);
        assert_eq!(adopted.prefix.as_ref(), b"temper/");
        assert!(adopted.checks.is_empty());
        assert_eq!(root.sent_name(call), Some(crate::key::engine(&f::name(b"first-repository-3"))));
        assert_eq!(root.sent_name(call), Some(forge::first_repository_key(3, false)));
        assert_eq!(root.closed.len(), 1);
        root.adoption_answer(
            call,
            if home_taken { f::forge_refused(w::ForgeReason::Refused) } else { f::adopted_with_unknown_protection() },
        );
        if home_taken {
            assert!(root.answer(call).is_none());
            assert!(!root.sent_adoption(call).home);
            assert_eq!(root.sent_name(call), Some(crate::key::engine(&f::name(b"first-repository-additional-3"))));
            assert_eq!(root.sent_name(call), Some(forge::first_repository_key(3, true)));
            root.adoption_answer(call, f::adopted_with_unknown_protection());
        }
        assert_eq!(root.answer(call).unwrap().status, 0);
        assert!(root.answer(call).is_none(), "one terminal answer");
    }
}
#[test]
fn setup_failure_names_the_creation_or_box_step_and_exits_one() {
    let mut cases = List::with_capacity(5);
    cases
        .push((
            w::Connection::SetUpFailed(
                w::SetUpFailed::new(
                    &w::CEILINGS,
                    w::SetUpFailedParts { why: b"preparing".as_slice().into(), at: None },
                )
                .unwrap(),
            ),
            None,
            b"Set up failed: preparing".as_slice(),
        ))
        .unwrap();
    for (step, words) in [
        (w::CreateStep::Admin, b"checking the admin's token".as_slice()),
        (w::CreateStep::Org, b"making the organization"),
        (w::CreateStep::Repository, b"making the repository"),
        (w::CreateStep::File, b"adding the starter workflow"),
    ] {
        cases
            .push((
                w::Connection::Connecting,
                Some(first_record(w::FirstState::Failed(
                    w::CreateFailed::new(&w::CEILINGS, w::CreateFailedParts { step }).unwrap(),
                ))),
                words,
            ))
            .unwrap();
    }
    for (connection, first, words) in cases.into_boxed() {
        let mut root = f::signed_in(f::PARTY);
        let call = root.submit(&["forge", "setup", "3"]);
        root.adoption_answer(call, w::Outcome::SettingUp);
        deliver(&mut root, record_first(connection, first));
        let printed = root.answer(call).unwrap();
        assert_eq!(printed.status, 1);
        assert!(skein_lib::bytes::find(&printed.stderr, words).is_some());
        assert_eq!(root.closed.len(), 1);
        assert!(crate::root::next_deadline(&root.root).is_none());
    }
}
#[test]
fn made_wait_requires_the_named_forges_first_repository_to_be_made() {
    let mut root = f::signed_in(f::PARTY);
    let call = root.submit(&["wait", "forges", "--until", "made:3"]);
    for first in [None, Some(first_record(w::FirstState::Making))] {
        deliver(
            &mut root,
            record_first(
                w::Connection::Connected(
                    w::Connected::new(&w::CEILINGS, w::ConnectedParts { id: 1, login: b"temper".as_slice().into() })
                        .unwrap(),
                ),
                first,
            ),
        );
        assert!(root.answer(call).is_none(), "connected alone is insufficient");
    }
    deliver(&mut root, record_first(w::Connection::Connecting, Some(first_record(made()))));
    assert_eq!(root.answer(call).unwrap().status, 0);
    assert_eq!(root.closed.len(), 1);
    let mut row = row(C::Connecting);
    row.first = Some(forge::forges::First {
        playground: false,
        org: b"work".as_slice().into(),
        repository: b"hello".as_slice().into(),
        project: forge::forges::SetUpProject::Existing(1),
        state: forge::forges::FirstState::Made { id: 42 },
        label: b"docker".as_slice().into(),
    });
    assert!(!forges::holds(&[row], b"made:4"));
}
#[test]
fn first_repository_text_and_json_name_every_state_and_creation_step() {
    use forge::forges::{CreateStep, First, FirstState, SetUpProject};
    for (state, words, field) in [
        (FirstState::Making, b"Making work/hello".as_slice(), br#""state":"making""#.as_slice()),
        (FirstState::Made { id: 42 }, b"work/hello made, in 1", br#""state":"made","id":42"#),
        (
            FirstState::Failed { step: CreateStep::Admin },
            b"Making work/hello failed: checking the admin's token",
            br#""state":"failed","step":"admin"#,
        ),
        (
            FirstState::Failed { step: CreateStep::Org },
            b"Making work/hello failed: making the organization",
            br#""state":"failed","step":"org"#,
        ),
        (
            FirstState::Failed { step: CreateStep::Repository },
            b"Making work/hello failed: making the repository",
            br#""state":"failed","step":"repository"#,
        ),
        (
            FirstState::Failed { step: CreateStep::File },
            b"Making work/hello failed: adding the starter workflow",
            br#""state":"failed","step":"file"#,
        ),
    ] {
        let mut row = row(C::Connecting);
        row.first = Some(First {
            playground: false,
            org: b"work".as_slice().into(),
            repository: b"hello".as_slice().into(),
            project: SetUpProject::Existing(1),
            state,
            label: b"docker".as_slice().into(),
        });
        let plain = forges::print_rows(Some(&[row.clone()]), false, None, &f::limits_root());
        assert!(skein_lib::bytes::find(&plain.stdout, words).is_some());
        let json = forges::print_rows(Some(&[row]), true, None, &f::limits_root());
        assert!(f::json_has(&json, br#""first":{"org":"work","repository":"hello","project":{"existing":1}"#));
        assert!(f::json_has(&json, field));
    }
}

#[test]
fn demo_start_answers_print_their_words_json_and_status() {
    let l = &w::CEILINGS;
    for (outcome, expected, json_expected, status) in [
        (w::Outcome::Running, "running\n", r#"{"running":{}}"#, 0),
        (
            w::Outcome::StartRefused(
                w::StartRefused::new(l, w::StartRefusedParts { why: w::StartRefusal::NotOffered }).unwrap(),
            ),
            "refused: not offered\n",
            r#"{"start_refused":{"why":"not_offered"}}"#,
            2,
        ),
        (
            w::Outcome::StartFailed(
                w::StartFailed::new(
                    l,
                    w::StartFailedParts {
                        why: w::StartFailure::Group(
                            w::GroupFailed::new(l, w::GroupFailedParts { why: b"fetch failed".as_slice().into() })
                                .unwrap(),
                        ),
                    },
                )
                .unwrap(),
            ),
            "the fakes did not start (fetch failed)\n",
            r#"{"start_failed":{"why":{"group":{"why":"fetch failed"}}}}"#,
            1,
        ),
        (
            w::Outcome::StartFailed(
                w::StartFailed::new(l, w::StartFailedParts { why: w::StartFailure::Account }).unwrap(),
            ),
            "the account refused\n",
            r#"{"start_failed":{"why":"account"}}"#,
            1,
        ),
        (
            w::Outcome::StartFailed(
                w::StartFailed::new(l, w::StartFailedParts { why: w::StartFailure::Adoption }).unwrap(),
            ),
            "the adoption refused\n",
            r#"{"start_failed":{"why":"adoption"}}"#,
            1,
        ),
        (
            w::Outcome::StartFailed(
                w::StartFailed::new(l, w::StartFailedParts { why: w::StartFailure::NotConnected }).unwrap(),
            ),
            "the forge not connected in time\n",
            r#"{"start_failed":{"why":"not_connected"}}"#,
            1,
        ),
    ] {
        for json in [false, true] {
            let mut root = f::signed_in(f::PARTY);
            let call = root.submit_prepared(f::call(&["playground", "start", "--key", "playground"]), json, None);
            let client::Ask::Family { scope, record } = root.adoption_bytes(call) else { panic!("family") };
            assert_eq!(*scope, c::Scope::Deployment);
            assert_eq!(wire::decode_requesting(&record.bytes, l).unwrap().request(), &w::Request::StartDemo);
            root.adoption_answer(call, outcome.clone());
            let printed = root.answer(call).unwrap();
            assert_eq!(printed.status, status);
            let bytes = if status == 0 { &printed.stdout } else { &printed.stderr };
            assert_eq!(bytes.as_ref(), if json { json_expected.as_bytes() } else { expected.as_bytes() });
        }
    }
}

#[test]
fn demo_start_keeps_drawn_keys_and_recovers_its_own_answer() {
    for json in [false, true] {
        let mut root = f::signed_in(f::PARTY);
        let call = root.submit(&["playground", "start"]);
        assert!(root.saved.contains_key(&call), "a drawn demo start key is durable");
        let key = root.sent_name(call).unwrap();
        let mut records = f::restored_adoption(f::PARTY);
        let record = records.get_mut(0).unwrap();
        record.key = key;
        record.request.request = root.adoption_bytes(call).clone();
        let mut root = f::started_with(records, f::SEED, json);
        root.deliver(c::Delivery::Me(Box::new(f::me(f::PARTY))));
        root.adoption_answer(0, w::Outcome::Running);
        let expected = if json {
            b"{\"operation\":\"demo-start\",\"outcome\":{\"running\":{}}}\n".as_slice()
        } else {
            b"kept: demo-start running\n"
        };
        assert_eq!(root.kept_lines.as_slice(), &[Box::<[u8]>::from(expected)]);
    }
}

#[test]
fn forges_json_names_installations_and_the_last_connect_end() {
    use forge::forges::{Account, Installation};
    let mut row = row(C::NotConnected);
    row.kind = Kind::GitHub;
    row.installations = Box::new([
        Installation {
            id: 11,
            account: Some(Account { id: 111, login: Box::from(b"acme".as_slice()) }),
            suspended: true,
            stray: false,
        },
        Installation { id: 22, account: None, suspended: false, stray: true },
    ]);
    row.ended = Some(Box::from(b"the pasted value changed".as_slice()));
    let json = forges::print_rows(Some(&[row]), true, None, &f::limits_root());
    assert!(f::json_has(&json, br#""installations":[{"id":11,"account":{"id":111,"login":"acme"},"suspended":true,"stray":false},{"id":22,"account":null,"suspended":false,"stray":true}]"#));
    assert!(f::json_has(&json, br#""ended":"the pasted value changed""#));
}

fn settled_wait_with_installations(count: u32) {
    let mut root = f::signed_in(f::PARTY);
    let call = root.submit(&["wait", "forges", "--until", "settled", "--json"]);
    let limits = forge::wire_limits(&root.limits.domain.forge).unwrap();
    let mut installations = List::with_capacity(count);
    for id in 1..=count {
        installations
            .push(
                w::Installation::new(
                    &limits,
                    w::InstallationParts {
                        id: u64::from(id),
                        account: Some(
                            w::Account::new(&limits, w::AccountParts { id: 111, login: b"acme".as_slice().into() })
                                .unwrap(),
                        ),
                        suspended: false,
                        stray: false,
                    },
                )
                .unwrap(),
            )
            .unwrap();
    }
    let mut forges = List::with_capacity(1);
    forges
        .push(
            w::ForgeRow::new(
                &limits,
                w::ForgeRowParts {
                    number: 3,
                    kind: w::Kind::Github,
                    mode: w::Mode::Connected,
                    connection: w::Connection::Connected(
                        w::Connected::new(&limits, w::ConnectedParts { id: 42, login: b"app".as_slice().into() })
                            .unwrap(),
                    ),
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
    deliver(
        &mut root,
        client::Family {
            family: wire::FORGE,
            version: wire::CLIENT_VERSION,
            bytes: wire::forges(&w::Forges::new(&limits, w::ForgesParts { forges }).unwrap()),
        },
    );
    let printed = root.answer(call).expect("settled wait completes");
    assert_eq!(printed.status, 0);
    assert!(printed.stderr.is_empty());
    assert!(f::json_has(&printed, br#""kind":"github""#));
    assert!(f::json_has(&printed, br#""met":true"#));
    for id in 1..=count {
        let mut measure = jig::render::Text::measure();
        measure.number(u64::from(id));
        let mut number = jig::render::Text::writing_exact(measure);
        number.number(u64::from(id));
        let installation = concat(&[
            br#"{"id":"#,
            &number.finish(),
            br#","account":{"id":111,"login":"acme"},"suspended":false,"stray":false}"#,
        ]);
        assert!(f::json_has(&printed, &installation));
    }
    assert!(printed.stdout.len() <= usize::try_from(root.limits.tools.text).unwrap());
    assert_eq!(root.closed.len(), 1);
    assert!(crate::root::next_deadline(&root.root).is_none());
}

#[test]
fn settled_wait_prints_an_accepted_installation_with_its_account() {
    settled_wait_with_installations(1);
}

#[test]
fn settled_wait_prints_the_wire_maximum_installations_with_their_accounts() {
    settled_wait_with_installations(forge::forges::INSTALLATIONS);
}

#[test]
fn human_forges_names_the_app_accounts_unknown_installations_and_their_states() {
    use forge::forges::{Account, Installation};
    let mut row = row(C::Connected { id: 42, login: Box::from(b"my-app[bot]".as_slice()) });
    row.kind = Kind::GitHub;
    row.installations = Box::new([
        Installation {
            id: 11,
            account: Some(Account { id: 111, login: Box::from(b"acme".as_slice()) }),
            suspended: false,
            stray: false,
        },
        Installation { id: 22, account: None, suspended: true, stray: true },
    ]);
    let plain = forges::print_rows(Some(&[row]), false, None, &f::limits_root());
    assert_eq!(plain.status, 0);
    assert!(
        skein_lib::bytes::find(
            &plain.stdout,
            " · my-app\n  installation 11 · acme\n  installation 22 · suspended · not accepted · uninstalled at the next reconcile · forge accept 3 22\n".as_bytes(),
        )
        .is_some()
    );
}

#[test]
fn human_forges_names_the_last_connect_reason_only_when_present() {
    let mut row = row(C::NotConnected);
    for ended in [None, Some(Box::from(b"the pasted value changed".as_slice()))] {
        row.ended = ended;
        let plain = forges::print_rows(Some(&[row.clone()]), false, None, &f::limits_root());
        assert_eq!(plain.status, 0);
        assert_eq!(skein_lib::bytes::find(&plain.stdout, b"The last connect stopped: ").is_some(), row.ended.is_some(),);
        if row.ended.is_some() {
            assert!(
                skein_lib::bytes::find(
                    &plain.stdout,
                    " · The last connect stopped: the pasted value changed\n".as_bytes(),
                )
                .is_some()
            );
        }
    }
}

#[test]
fn human_forges_prints_an_app_installed_nowhere_yet() {
    let mut row = row(C::Connected { id: 42, login: Box::from(b"my-app[bot]".as_slice()) });
    row.kind = Kind::GitHub;
    let plain = forges::print_rows(Some(&[row]), false, None, &f::limits_root());
    assert_eq!(plain.status, 0);
    assert!(skein_lib::bytes::find(&plain.stdout, " · my-app · installed nowhere yet\n".as_bytes()).is_some());
}

#[test]
fn human_forges_bounds_the_wire_maximum_installations_with_longest_logins() {
    let limits = f::limits_root();
    let preset = forge::wire_limits(&limits.domain.forge).unwrap();
    for bounds in [preset, w::CEILINGS] {
        let mut login = skein_lib::Writer::new(usize::try_from(bounds.account_login).unwrap());
        for _ in 0..bounds.account_login {
            login.put(b"x").unwrap();
        }
        let login = login.finish();
        let mut installations = List::with_capacity(forge::forges::INSTALLATIONS);
        for id in 1..=forge::forges::INSTALLATIONS {
            installations
                .push(
                    w::Installation::new(
                        &bounds,
                        w::InstallationParts {
                            id: u64::from(id),
                            account: Some(
                                w::Account::new(&bounds, w::AccountParts { id: u64::from(id), login: login.clone() })
                                    .unwrap(),
                            ),
                            suspended: true,
                            stray: true,
                        },
                    )
                    .unwrap(),
                )
                .unwrap();
        }
        let mut rows = List::with_capacity(1);
        rows.push(
            w::ForgeRow::new(
                &bounds,
                w::ForgeRowParts {
                    number: 3,
                    kind: w::Kind::Github,
                    mode: w::Mode::Connected,
                    connection: w::Connection::Connected(
                        w::Connected::new(
                            &bounds,
                            w::ConnectedParts { id: 42, login: b"my-app[bot]".as_slice().into() },
                        )
                        .unwrap(),
                    ),
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
        let record = client::Family {
            family: wire::FORGE,
            version: wire::CLIENT_VERSION,
            bytes: wire::forges(&w::Forges::new(&bounds, w::ForgesParts { forges: rows }).unwrap()),
        };
        let mut domain_limits = limits.domain.forge;
        domain_limits.field_bytes = bounds.account_login;
        let rows = forge::forges::rows(&record, &domain_limits).unwrap();
        let plain = forges::print_rows(Some(&rows), false, None, &limits);
        if bounds.account_login == preset.account_login {
            assert_eq!(plain.status, 0);
            for words in [login.as_ref(), b"suspended", b"uninstalled at the next reconcile"] {
                let mut count = 0u32;
                for window in plain.stdout.windows(words.len()) {
                    if window == words {
                        count = count.checked_add(1u32).unwrap();
                    }
                }
                assert_eq!(count, forge::forges::INSTALLATIONS);
            }
            assert!(plain.stdout.len() <= usize::try_from(limits.tools.text).unwrap());
        } else {
            // The digest's JSON exceeds tools.text before the human row is rendered.
            assert_eq!(plain.status, 1);
            assert!(plain.stdout.is_empty());
            assert_eq!(plain.stderr.as_ref(), b"result limit\n");
        }
    }
}

#[test]
fn setup_makes_the_default_or_named_project_then_adopts_the_answered_number() {
    // Group, verb, forge, project flag and value.
    const WORDS_WITH_PROJECT: u32 = 5;
    for flags in [&[][..], &["--project", "Next"][..], &["--project", "7"][..]] {
        let mut root = f::signed_in(f::PARTY);
        let mut argv = List::with_capacity(WORDS_WITH_PROJECT);
        argv.push("forge").unwrap();
        argv.push("setup").unwrap();
        argv.push("3").unwrap();
        for flag in flags {
            argv.push(*flag).unwrap();
        }
        let call = root.submit(argv.as_slice());
        root.adoption_answer(call, w::Outcome::SettingUp);
        deliver(
            &mut root,
            record_first(
                w::Connection::Connected(
                    w::Connected::new(&w::CEILINGS, w::ConnectedParts { id: 1, login: b"temper".as_slice().into() })
                        .unwrap(),
                ),
                Some(first_record(made())),
            ),
        );
        let expected = if flags == ["--project", "7"] {
            7
        } else {
            let client::Ask::Jig(ask) = root.adoption_bytes(call) else { panic!("MakeProject before Adopt") };
            let c::Ask::MakeProject { name, policy } = ask.as_ref() else { panic!("MakeProject") };
            assert_eq!(name.bytes(), if flags.is_empty() { b"hello".as_slice() } else { b"Next" });
            assert_eq!(*policy, 1);
            assert_eq!(root.sent_name(call), Some(crate::key::engine(&f::name(b"start-project-3"))));
            let (attempt, _, _) = root.sends.get(&call).unwrap();
            root.client(client::Event::Up {
                up: client::Up::Answer {
                    attempt: *attempt,
                    envelope: skein_lib::RequestEnvelope::Final,
                    answer: Some(client::Answered::Jig(c::Answer::Done(c::Outcome::Owner(
                        c::OwnerOutcome::ProjectMade { project: 8 },
                    )))),
                    why: None,
                },
            });
            8
        };
        assert_eq!(root.sent_adoption(call).project, expected);
        assert!(root.sent_adoption(call).home);
        assert_eq!(root.sent_name(call), Some(forge::first_repository_key(3, false)));
        root.adoption_answer(call, f::adopted_with_unknown_protection());
        assert_eq!(root.answer(call).unwrap().status, 0);
    }
}

#[test]
fn refused_make_project_prints_jigs_words_and_sends_no_adoption() {
    let mut root = f::signed_in(f::PARTY);
    let call = root.submit(&["forge", "setup", "3"]);
    root.adoption_answer(call, w::Outcome::SettingUp);
    deliver(
        &mut root,
        record_first(
            w::Connection::Connected(
                w::Connected::new(&w::CEILINGS, w::ConnectedParts { id: 1, login: b"temper".as_slice().into() })
                    .unwrap(),
            ),
            Some(first_record(made())),
        ),
    );
    let (attempt, _, _) = root.sends.get(&call).unwrap();
    root.client(client::Event::Up {
        up: client::Up::Answer {
            attempt: *attempt,
            envelope: skein_lib::RequestEnvelope::Final,
            answer: Some(client::Answered::Jig(c::Answer::Refused { refusal: c::Refusal::NameTaken, at: None })),
            why: None,
        },
    });
    let printed = root.answer(call).unwrap();
    assert_eq!(printed.status, 2);
    assert!(skein_lib::bytes::find(&printed.stderr, b"Another project has that name").is_some());
    let client::Ask::Jig(_) = root.adoption_bytes(call) else { panic!("no adoption after refusal") };
}

#[test]
fn playground_setup_sends_its_copy_and_default_or_explicit_project() {
    // Group, verb, forge, playground switch, project flag and value.
    const PLAYGROUND_WORDS: u32 = 6;
    for (flags, project) in [
        (&["--playground"][..], b"widget".as_slice()),
        (&["--playground", "--project", "copy"][..], b"copy".as_slice()),
    ] {
        let mut root = f::signed_in(f::PARTY);
        let mut words = List::with_capacity(PLAYGROUND_WORDS);
        words.push("forge").unwrap();
        words.push("setup").unwrap();
        words.push("3").unwrap();
        for word in flags {
            words.push(*word).unwrap();
        }
        let call = root.submit(words.as_slice());
        let client::Ask::Family { record, .. } = root.adoption_bytes(call) else { panic!("family") };
        let w::Request::SetUp(value) =
            wire::decode_requesting(&record.bytes, &w::CEILINGS).unwrap().into_parts().request
        else {
            panic!("setup")
        };
        assert!(value.playground());
        assert_eq!((value.org(), value.repository()), (b"playground".as_slice(), b"widget".as_slice()));
        let w::SetUpProject::New(value) = value.project() else { panic!("new project") };
        assert_eq!(value.name(), project);
    }
}
#[test]
fn playground_setup_rejects_org_or_repository_before_sending() {
    for flag in ["--org", "--repository"] {
        let mut root = f::signed_in(f::PARTY);
        let call = root.submit(&["forge", "setup", "3", "--playground", flag, "other"]);
        assert_eq!(root.answer(call).unwrap().status, 1);
        assert!(root.nothing_sent(), "a usage error must send no request");
    }
}
