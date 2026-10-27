use super::fixtures as f;
use jig_client::{self as client, contract as c};
use skein_lib::{Reader, RequestEnvelope, Secret};
use temper_client_protocol_wire::{self as wire, v1 as w};

const CONNECT: &[&str] = &["forge", "connect", "3", "github-app", "4242", "app.pem", "--key", "app"];
const ADMIN: &[&str] = &["forge", "connect", "3", "forgejo-admin", "token", "--key", "app"];
const TOKEN: &[u8] = b"admin-token-placeholder-0123\n\n";
const MANIFEST: &[&str] = &["forge", "connect", "3", "github", "--key", "app"];
const INSTALL: &[&str] = &["forge", "install", "3"];
const ACCEPT: &[&str] = &["forge", "accept", "3", "22"];
// An opaque fake key; neither these step tests nor the shell tests contact a forge.
const PEM: &[u8] = b"fake-app-pem";
const AT_BOUND: &[u8] = &[b'p'; 4089];
const OVER_BOUND: &[u8] = &[b'p'; 4090];
const ADDRESS: &[u8] = b"https://forge.invalid/install/app";

fn request(root: &f::Harness, call: u32) -> w::Request {
    let client::Ask::Family { scope: c::Scope::Deployment, record } = &root.sends[&call].2 else {
        panic!("deployment family request")
    };
    assert_eq!(record.family, wire::FORGE);
    assert_eq!(record.version, wire::CLIENT_VERSION);
    let limits = temper_client_domain::forge::wire_limits(&root.limits.domain.forge).expect("wire limits");
    let mut reader = Reader::new(&record.bytes);
    let request = w::Requesting::decode(&limits, &mut reader).expect("request").into_parts().request;
    assert!(reader.is_empty());
    request
}
fn link() -> w::Outcome {
    let limits = temper_client_domain::forge::wire_limits(&f::limits_root().domain.forge).expect("wire limits");
    w::Outcome::Link(w::Link::new(&limits, w::LinkParts { address: ADDRESS.into() }).expect("link"))
}

#[test]
fn connect_and_install_check_their_argv_bounds() {
    for forge in ["1", "65535"] {
        for id in ["1", "18446744073709551615"] {
            let words = &["forge", "connect", forge, "github-app", id, "app.pem", "--key", "app"];
            let call = f::call(words);
            assert_eq!(f::back(&call), f::owned(words));
        }
        f::parse(&["forge", "install", forge]).expect("forge bound");
    }
    for forge in ["0", "65536"] {
        f::parse(&["forge", "connect", forge, "github-app", "1", "app.pem", "--key", "app"]).expect_err("forge bound");
        f::parse(&["forge", "install", forge]).expect_err("forge bound");
    }
    for id in ["0", "18446744073709551616"] {
        f::parse(&["forge", "connect", "3", "github-app", id, "app.pem", "--key", "app"]).expect_err("id bound");
    }
    for words in [
        &["forge", "connect", "3", "unknown", "--key", "app"][..],
        &["forge", "connect", "3", "github-app", "1", "app.pem"],
        &["forge", "connect", "3", "github-app", "1", "app.pem", "--key"],
        &["forge", "connect", "3", "github-app", "1", "-", "--key", "app"],
    ] {
        f::parse(words).expect_err("invalid call");
    }
}
#[test]
fn connect_carries_the_bundle_once_without_saving_it() {
    for pem in [PEM, AT_BOUND] {
        let mut root = f::signed_in(f::PARTY);
        let call = root.submit_value(CONNECT, Some(Secret::new(pem.into())));
        let w::Request::Connect(connect) = request(&root, call) else { panic!("connect") };
        assert_eq!(connect.forge(), 3);
        let w::How::GithubApp(app) = connect.into_parts().how else { panic!("pasted App") };
        assert_eq!(app.id(), 4242);
        assert_eq!(root.sent_name(call), Some(f::named("app")));
        let mut expected = skein_lib::Writer::new(pem.len().checked_add(7).expect("bundle length"));
        expected.put(&[0, 1, 10]).expect("header");
        expected.put(&u32::try_from(pem.len()).expect("bounded").to_be_bytes()).expect("length");
        expected.put(pem).expect("PEM");
        assert_eq!(root.carried[&call].expose(), expected.finish().as_ref());
        assert!(root.saved.is_empty());
        assert!(root.answer(call).is_none());
    }
}
#[test]
fn connect_refuses_empty_or_oversized_values_before_sending() {
    for pem in [b"".as_slice(), OVER_BOUND] {
        let mut root = f::signed_in(f::PARTY);
        let call = root.submit_value(CONNECT, Some(Secret::new(pem.into())));
        assert_eq!(root.answer(call).expect("local failure").status, 1);
        assert!(root.nothing_sent());
    }
    let mut root = f::signed_in(f::PARTY);
    let call = root.submit(CONNECT);
    assert_eq!(root.answer(call).expect("missing file value").status, 1);
    assert!(root.nothing_sent());
}
#[test]
fn install_sends_no_back_or_value_and_accepts_an_optional_key() {
    for words in [INSTALL, &["forge", "install", "3", "--key", "install"]] {
        let mut root = f::signed_in(f::PARTY);
        let call = root.submit(words);
        let w::Request::Install(install) = request(&root, call) else { panic!("install") };
        assert_eq!(install.forge(), 3);
        assert!(install.back().is_none());
        assert!(root.carried.is_empty());
        if words.len() > INSTALL.len() {
            assert_eq!(root.sent_name(call), Some(f::named("install")));
        }
    }
}
#[test]
fn unoffered_ways_exit_two_without_reading_or_sending_a_value() {
    let words = &["forge", "connect", "3", "forgejo", "tokens", "--key", "app"];
    let mut root = f::signed_in(f::PARTY);
    let call = root.submit(words);
    let printed = root.answer(call).expect("not offered");
    assert_eq!(printed.status, 2);
    assert!(skein_lib::bytes::find(&printed.stderr, b"not offered yet").is_some());
    assert!(root.nothing_sent());
}
#[test]
fn connect_and_install_print_their_durable_outcomes() {
    for json in [false, true] {
        for (words, outcome, status, text, structured) in [
            (CONNECT, w::Outcome::Connecting, 0, b"connecting\ntemper ctl DIR forge list --until connected:3\n".as_slice(), br#"{"connecting":{}}"#.as_slice()),
            (ADMIN, w::Outcome::Connecting, 0, b"connecting\ntemper ctl DIR forge list --until connected:3\n".as_slice(), br#"{"connecting":{}}"#.as_slice()),
            (MANIFEST, link(), 0, b"Open this address in a browser on the engine's machine:\nhttps://forge.invalid/install/app\ntemper ctl DIR forge list --until settled\n".as_slice(), br#"{"link":{"address":"https://forge.invalid/install/app"}}"#.as_slice()),
            (INSTALL, link(), 0, b"Open this address in a browser on the engine's machine:\nhttps://forge.invalid/install/app\ntemper ctl DIR forge list --until settled\n", br#"{"link":{"address":"https://forge.invalid/install/app"}}"#),
            (MANIFEST, w::Outcome::Connecting, 1, b"connecting\n".as_slice(), br#"{"connecting":{}}"#.as_slice()),
            (INSTALL, w::Outcome::Connecting, 1, b"connecting\n", br#"{"connecting":{}}"#),
            (INSTALL, w::Outcome::Unknown, 1, b"unknown\n", br#"{"unknown":{}}"#),
        ] {
            let mut root = f::signed_in(f::PARTY);
            let value = if words == CONNECT { Some(Secret::new(PEM.into())) } else if words == ADMIN { Some(Secret::new(TOKEN.into())) } else { None };
            let call = root.submit_prepared(f::call(words), json, value);
            root.adoption_answer(call, outcome);
            let printed = root.answer(call).expect("durable outcome");
            assert_eq!(printed.status, status);
            no_token(&printed);
            assert_eq!(if status == 0 { &printed.stdout } else { &printed.stderr }.as_ref(), if json { structured } else { text });
        }
    }
}
#[test]
fn every_forge_refusal_prints_words_json_and_exit_two() {
    for (why, name, text) in [
        (w::ForgeRefusal::Role, "role", "not the deployment's owner"),
        (w::ForgeRefusal::NoSuchForge, "no_such_forge", "no such forge"),
        (w::ForgeRefusal::NotTaken, "not_taken", "not taken by this forge"),
        (w::ForgeRefusal::Connected, "connected", "connected already: forge-replace replaces its key"),
        (w::ForgeRefusal::NotConnected, "not_connected", "not connected: connect the App first"),
        (w::ForgeRefusal::Busy, "busy", "busy"),
        (w::ForgeRefusal::NoKey, "no_key", "no key"),
        (w::ForgeRefusal::FirstIsBot, "first_is_bot", "the organization is the bot's login: name another organization"),
    ] {
        for json in [false, true] {
            for words in [CONNECT, ADMIN, MANIFEST, INSTALL, ACCEPT] {
                let mut root = f::signed_in(f::PARTY);
                let value = if words == CONNECT {
                    Some(Secret::new(PEM.into()))
                } else if words == ADMIN {
                    Some(Secret::new(TOKEN.into()))
                } else {
                    None
                };
                let call = root.submit_prepared(f::call(words), json, value);
                let limits = temper_client_domain::forge::wire_limits(&root.limits.domain.forge).expect("limits");
                root.adoption_answer(
                    call,
                    w::Outcome::ConnectRefused(
                        w::ConnectRefused::new(&limits, w::ConnectRefusedParts { why: why.clone() }).expect("refusal"),
                    ),
                );
                let printed = root.answer(call).expect("refused");
                assert_eq!(printed.status, 2);
                no_token(&printed);
                assert!(printed.stdout.is_empty());
                let parts = if json {
                    [b"{\"refused\":{\"why\":\"".as_slice(), name.as_bytes(), b"\"}}"]
                } else {
                    [b"".as_slice(), text.as_bytes(), b"\n"]
                };
                let length = parts[0]
                    .len()
                    .checked_add(parts[1].len())
                    .expect("refusal length")
                    .checked_add(parts[2].len())
                    .expect("refusal length");
                let mut expected = skein_lib::Writer::new(length);
                for part in parts {
                    expected.put(part).expect("refusal part");
                }
                assert_eq!(printed.stderr.as_ref(), expected.finish().as_ref());
            }
        }
    }
}
#[test]
fn a_lost_or_unsent_connect_names_how_to_retry_and_is_never_replayed() {
    for words in [CONNECT, ADMIN] {
        for envelope in [RequestEnvelope::Again, RequestEnvelope::Lost] {
            let mut root = f::signed_in(f::PARTY);
            let call = root.submit_value(words, Some(Secret::new(TOKEN.into())));
            let attempt = root.sends[&call].0;
            root.client(client::Event::Up {
                up: client::Up::Answer { attempt, envelope, answer: None, why: Some(client::Retry::Busy) },
            });
            let printed = root.answer(call).expect("one-shot failure");
            assert_eq!(printed.status, 1);
            assert!(skein_lib::bytes::find(&printed.stderr, b"supply --key app and the same file again").is_some());
            assert!(root.saved.is_empty());
            assert_eq!(root.sends.len(), 1);
            no_token(&printed);
        }
    }
}

#[test]
fn an_install_recovers_its_durable_answer_with_its_own_operation_name() {
    for json in [false, true] {
        let mut root = f::signed_in(f::PARTY);
        let call = root.submit(INSTALL);
        assert!(root.saved.contains_key(&call));
        let mut records = f::restored_adoption(f::PARTY);
        let record = records.get_mut(0).expect("one restore");
        record.key = root.sent_name(call).expect("engine key");
        record.request.request = root.adoption_bytes(call).clone();
        let mut root = f::started_with(records, f::SEED, json);
        root.deliver(c::Delivery::Me(Box::new(f::me(f::PARTY))));
        root.adoption_answer(0, link());
        let expected = if json {
            b"{\"operation\":\"forge-install\",\"outcome\":{\"link\":{\"address\":\"https://forge.invalid/install/app\"}}}\n".as_slice()
        } else {
            b"kept: forge-install Open this address in a browser on the engine's machine:\nhttps://forge.invalid/install/app\ntemper ctl DIR forge list --until settled\n"
        };
        assert_eq!(root.kept_lines.as_slice(), &[Box::<[u8]>::from(expected)]);
    }
}
#[test]
fn a_connect_waits_for_me_and_keeps_its_request_stable_across_clocks() {
    let mut root = f::started(skein_lib::List::with_capacity(0));
    let call = root.submit_value(CONNECT, Some(Secret::new(PEM.into())));
    assert!(root.nothing_sent());
    root.deliver(c::Delivery::Me(Box::new(f::me(f::PARTY))));
    let mut later = f::signed_in(f::PARTY);
    later.now = later.now.checked_add(skein_lib::Duration::from_secs(42)).expect("later clock");
    let next = later.submit_value(CONNECT, Some(Secret::new(PEM.into())));
    assert_eq!(root.sent_name(call), later.sent_name(next));
    assert_eq!(root.adoption_bytes(call), later.adoption_bytes(next));
    assert_eq!(root.carried[&call].expose(), later.carried[&next].expose());
}
#[test]
fn a_connect_refuses_an_install_link_and_both_refuse_unrelated_outcomes() {
    for (words, outcome) in [(CONNECT, link()), (CONNECT, w::Outcome::SettingUp), (INSTALL, w::Outcome::Running)] {
        let mut root = f::signed_in(f::PARTY);
        let call = root.submit_value(
            words,
            if words == CONNECT {
                Some(Secret::new(PEM.into()))
            } else if words == ADMIN {
                Some(Secret::new(TOKEN.into()))
            } else {
                None
            },
        );
        root.adoption_answer(call, outcome);
        assert_eq!(root.answer(call).expect("invalid answer").status, 1);
    }
}

#[test]
fn manifest_connect_sends_no_back_or_carried_value() {
    let mut root = f::signed_in(f::PARTY);
    let call = root.submit(MANIFEST);
    let w::Request::Connect(connect) = request(&root, call) else { panic!("connect") };
    assert_eq!(connect.forge(), 3);
    let w::How::GithubManifest(manifest) = connect.how() else { panic!("manifest") };
    assert_eq!(manifest.back(), &None);
    assert!(root.carried.is_empty());
    assert_eq!(root.sent_name(call), Some(f::named("app")));
    assert!(root.saved.contains_key(&call));
    for words in
        [&["forge", "connect", "3", "github", "extra", "--key", "app"][..], &["forge", "connect", "3", "github"]]
    {
        f::parse(words).expect_err("invalid manifest call");
    }
}

fn no_token(printed: &crate::jig::Printed) {
    for bytes in [&printed.stdout, &printed.stderr] {
        assert!(skein_lib::bytes::find(bytes, TOKEN).is_none());
        assert!(skein_lib::bytes::find(bytes, b"admin-token-placeholder-0123").is_none());
    }
}
#[test]
fn admin_connect_carries_item_one_with_the_file_bytes_unchanged() {
    for token in [TOKEN, AT_BOUND] {
        let mut root = f::signed_in(f::PARTY);
        let call = root.submit_value(ADMIN, Some(Secret::new(token.into())));
        let w::Request::Connect(connect) = request(&root, call) else { panic!("connect") };
        let w::How::ForgejoAdmin(admin) = connect.how() else { panic!("admin") };
        assert!(admin.first().is_none());
        let mut expected = skein_lib::Writer::new(token.len() + 7);
        expected.put(&[0, 1, 0]).unwrap();
        expected.put(&u32::try_from(token.len()).expect("bounded token").to_be_bytes()).unwrap();
        expected.put(token).unwrap();
        let expected = expected.finish();
        assert_eq!(root.carried[&call].expose(), expected.as_ref());
        assert!(expected.len() <= 4096);
        assert!(root.saved.is_empty());
        root.adoption_answer(call, w::Outcome::Connecting);
        let printed = root.answer(call).expect("connecting");
        assert_eq!(printed.status, 0);
        assert_eq!(printed.stdout.as_ref(), b"connecting\ntemper ctl DIR forge list --until connected:3\n");
        no_token(&printed);
    }
}
#[test]
fn admin_connect_refuses_empty_and_over_bound_tokens() {
    for token in [b"".as_slice(), OVER_BOUND] {
        let mut root = f::signed_in(f::PARTY);
        let call = root.submit_value(ADMIN, Some(Secret::new(token.into())));
        let printed = root.answer(call).expect("refused locally");
        assert_eq!(printed.status, 1);
        no_token(&printed);
        assert!(root.nothing_sent());
    }
}
#[test]
fn admin_first_repository_uses_default_and_custom_project_and_label() {
    for (extra, project, label) in [
        (&["--first", "work/hello"][..], b"hello".as_slice(), b"docker".as_slice()),
        (&["--first", "work/hello", "--project", "other", "--label", "runner_1.x-y"][..], b"other", b"runner_1.x-y"),
    ] {
        let words = f::extended(ADMIN, extra);
        let mut root = f::signed_in(f::PARTY);
        let call = root.submit_value(words.as_slice(), Some(Secret::new(TOKEN.into())));
        let w::Request::Connect(connect) = request(&root, call) else { panic!("connect") };
        let w::How::ForgejoAdmin(admin) = connect.how() else { panic!("admin") };
        let first = admin.first().as_ref().expect("first repository");
        assert_eq!(first.org(), b"work");
        assert_eq!(first.repository(), b"hello");
        assert_eq!(first.label(), label);
        let w::SetUpProject::New(new) = first.project() else { panic!("new project") };
        assert_eq!(new.name(), project);
        root.adoption_answer(call, w::Outcome::Connecting);
        assert!(root.answer(call).is_none(), "waits for creation and adoption");
        assert_eq!(root.progress.as_slice(), &[Box::<[u8]>::from(b"connecting\n".as_slice())]);
    }
}
#[test]
fn admin_first_repository_resolves_an_existing_project_by_name_or_number() {
    for project in ["shop", "1"] {
        let extra = ["--first", "work/hello", "--project", project];
        let words = f::extended(ADMIN, &extra);
        let mut root = f::signed_in(f::PARTY);
        let call = root.submit_value(words.as_slice(), Some(Secret::new(TOKEN.into())));
        let w::Request::Connect(connect) = request(&root, call) else { panic!("connect") };
        let w::How::ForgejoAdmin(admin) = connect.how() else { panic!("admin") };
        let w::SetUpProject::Existing(existing) = admin.first().as_ref().expect("first").project() else {
            panic!("existing project")
        };
        assert_eq!(existing.project(), f::PROJECT);
    }
}
#[test]
fn admin_first_repository_refuses_malformed_names_and_labels_or_dependent_flags() {
    let mut long_org = "x".repeat(257);
    long_org.push_str("/repo");
    let mut long_repo = "x".repeat(257);
    long_repo.insert_str(0, "org/");
    let long_label = "x".repeat(65);
    for extra in [
        &["--project", "other"][..],
        &["--label", "docker"],
        &["--first", "org"],
        &["--first", "/repo"],
        &["--first", "org/"],
        &["--first", "org/repo/extra"],
        &["--first", &long_org],
        &["--first", &long_repo],
        &["--first", "org/repo", "--label", ""],
        &["--first", "org/repo", "--label", "bad label"],
        &["--first", "org/repo", "--label", &long_label],
    ] {
        let words = f::extended(ADMIN, extra);
        let mut root = f::signed_in(f::PARTY);
        let call = root.submit_value(words.as_slice(), Some(Secret::new(TOKEN.into())));
        let printed = root.answer(call).expect("refused locally");
        assert_eq!(printed.status, 1);
        no_token(&printed);
        assert!(root.nothing_sent());
    }
}
#[test]
fn an_admin_connect_is_not_recovered_as_a_durable_request() {
    let mut root = f::signed_in(f::PARTY);
    let call = root.submit_value(ADMIN, Some(Secret::new(TOKEN.into())));
    assert!(root.saved.is_empty());
    let mut restored = crate::forges::Forges::new();
    let env = skein_lib::Env {
        now: root.now,
        wall: root.wall,
        limits: crate::root::Limits::new(root.limits.domain, root.limits.tools),
    };
    let mut out = skein_lib::Queue::with_capacity(f::OUTPUTS);
    crate::forges::told(
        &mut restored,
        root.root.client(),
        &env,
        &client::Told::Kept {
            key: client::Key { asker: temper_client_domain::owner::Owner::Forge.asker(), number: 5 },
            request: root.adoption_bytes(call).clone(),
        },
        &mut out,
    );
    assert_eq!(restored.calls(), 0);
    assert!(out.is_empty());
}

fn first_page(root: &mut f::Harness, connection: w::Connection, state: w::FirstState, stop: Option<w::AdminStop>) {
    let l = temper_client_domain::forge::wire_limits(&root.limits.domain.forge).unwrap();
    let first = w::FirstRepository::new(
        &l,
        w::FirstRepositoryParts {
            playground: false,
            org: b"work".as_slice().into(),
            repository: b"hello".as_slice().into(),
            project: w::SetUpProject::New(
                w::NewProject::new(&l, w::NewProjectParts { name: b"hello".as_slice().into() }).unwrap(),
            ),
            label: b"docker".as_slice().into(),
            state,
        },
    )
    .unwrap();
    let mut rows = skein_lib::List::with_capacity(1);
    rows.push(
        w::ForgeRow::new(
            &l,
            w::ForgeRowParts {
                number: 3,
                kind: w::Kind::Forgejo,
                mode: w::Mode::Managed,
                connection,
                offer: w::Offer::NotOffered,
                first: Some(first),
                installations: skein_lib::List::with_capacity(0),
                ended: None,
                app: None,
                web: None,
                repositories: skein_lib::List::with_capacity(0),
                owned: None,
                admin: true,
                read_only: false,
                ended_stop: stop,
            },
        )
        .unwrap(),
    )
    .unwrap();
    let subject = temper_client_domain::forge::forges::watch(&root.limits.domain.forge).unwrap();
    let watch = root.watch_token(&subject);
    root.client(client::Event::Up {
        up: client::Up::FamilyDelivered {
            watch,
            record: client::Family {
                family: wire::FORGE,
                version: wire::CLIENT_VERSION,
                bytes: wire::forges(&w::Forges::new(&l, w::ForgesParts { forges: rows }).unwrap()),
            },
        },
    });
}
fn made() -> w::FirstState {
    w::FirstState::Made(w::Made::new(&w::CEILINGS, w::MadeParts { id: 42 }).unwrap())
}
fn connected() -> w::Connection {
    w::Connection::Connected(
        w::Connected::new(&w::CEILINGS, w::ConnectedParts { id: 5, login: b"elsewhere-bot".as_slice().into() })
            .unwrap(),
    )
}
#[test]
fn admin_first_makes_a_new_project_or_uses_its_number_or_name_then_prints_the_adoption() {
    for project in [None, Some("Next"), Some("1"), Some("shop")] {
        for json in [false, true] {
            let mut root = f::signed_in(f::PARTY);
            let mut words = f::extended(ADMIN, &["--first", "work/hello"]);
            if let Some(project) = project {
                words.push("--project").unwrap();
                words.push(project).unwrap();
            }
            let call = root.submit_prepared(f::call(words.as_slice()), json, Some(Secret::new(TOKEN.into())));
            root.adoption_answer(call, w::Outcome::Connecting);
            assert!(root.answer(call).is_none());
            first_page(&mut root, w::Connection::Connecting, made(), None);
            assert!(root.answer(call).is_none(), "waits for Connected too");
            first_page(&mut root, connected(), made(), None);
            let expected = if project == Some("1") || project == Some("shop") {
                f::PROJECT
            } else {
                let client::Ask::Jig(ask) = root.adoption_bytes(call) else { panic!("MakeProject") };
                let c::Ask::MakeProject { name, .. } = ask.as_ref() else { panic!("MakeProject") };
                assert_eq!(name.bytes(), if project.is_none() { b"hello".as_slice() } else { b"Next" });
                let attempt = root.sends[&call].0;
                root.client(client::Event::Up {
                    up: client::Up::Answer {
                        attempt,
                        envelope: RequestEnvelope::Final,
                        answer: Some(client::Answered::Jig(c::Answer::Done(c::Outcome::Owner(
                            c::OwnerOutcome::ProjectMade { project: 8 },
                        )))),
                        why: None,
                    },
                });
                8
            };
            let adoption = root.sent_adoption(call);
            assert_eq!(adoption.project, expected);
            assert!(adoption.home);
            assert!(adoption.ci);
            assert_eq!(adoption.role, w::Role::Owned);
            assert_eq!(root.sent_name(call), Some(temper_client_domain::forge::first_repository_key(3, false)));
            root.adoption_answer(call, f::adopted_with_unknown_protection());
            let printed = root.answer(call).unwrap();
            assert_eq!(printed.status, 0);
            no_token(&printed);
            assert!(
                skein_lib::bytes::find(&printed.stdout, if json { b"\"project\":" } else { b"project " }).is_some()
            );
        }
    }
}
#[test]
fn admin_first_reruns_use_the_same_project_and_adoption_keys_and_keep_an_existing_home() {
    for project in [None, Some("shop")] {
        let mut root = f::signed_in(f::PARTY);
        let mut words = f::extended(ADMIN, &["--first", "work/hello"]);
        if let Some(project) = project {
            words.push("--project").unwrap();
            words.push(project).unwrap();
        }
        let mut previous = None;
        let mut previous_project = None;
        for _ in 0u8..2 {
            let call = root.submit_value(words.as_slice(), Some(Secret::new(TOKEN.into())));
            root.adoption_answer(call, w::Outcome::Connecting);
            first_page(&mut root, connected(), made(), None);
            if project.is_none() {
                let client::Ask::Jig(_) = root.adoption_bytes(call) else { panic!("MakeProject") };
                let key = root.sent_name(call).unwrap();
                if let Some(previous) = previous_project {
                    assert_eq!(key, previous, "same MakeProject key on rerun");
                }
                previous_project = Some(key);
                let attempt = root.sends[&call].0;
                root.client(client::Event::Up {
                    up: client::Up::Answer {
                        attempt,
                        envelope: RequestEnvelope::Final,
                        answer: Some(client::Answered::Jig(c::Answer::Done(c::Outcome::Owner(
                            c::OwnerOutcome::ProjectMade { project: 8 },
                        )))),
                        why: None,
                    },
                });
            }
            let key = root.sent_name(call).unwrap();
            if let Some(previous) = previous {
                assert_eq!(key, previous, "engine deduplicates the adoption");
            }
            previous = Some(key);
            root.adoption_answer(call, f::forge_refused(w::ForgeReason::Refused));
            assert!(!root.sent_adoption(call).home);
            assert_eq!(root.sent_name(call), Some(temper_client_domain::forge::first_repository_key(3, true)));
            root.adoption_answer(call, f::adopted_with_unknown_protection());
            let printed = root.answer(call).unwrap();
            assert_eq!(printed.status, 0);
            no_token(&printed);
        }
    }
}
#[test]
fn admin_first_stop_and_failed_create_exit_one_without_the_token() {
    for json in [false, true] {
        for (state, stop, words) in [
            (made(), Some(w::AdminStop::NotAdmin), b"not a site admin".as_slice()),
            (made(), Some(w::AdminStop::Unmarked), b"unmarked bot login"),
            (made(), Some(w::AdminStop::Other), b"admin connection stopped"),
            (
                made(),
                Some(w::AdminStop::Refused(
                    w::AdminRefused::new(&w::CEILINGS, w::AdminRefusedParts { status: 403, stage: w::SetupStage::Org })
                        .unwrap(),
                )),
                b"403 at organization",
            ),
            (
                w::FirstState::Failed(
                    w::CreateFailed::new(&w::CEILINGS, w::CreateFailedParts { step: w::CreateStep::Repository })
                        .unwrap(),
                ),
                None,
                b"making the repository",
            ),
        ] {
            let mut root = f::signed_in(f::PARTY);
            let words_call = f::extended(ADMIN, &["--first", "work/hello"]);
            let call = root.submit_prepared(f::call(words_call.as_slice()), json, Some(Secret::new(TOKEN.into())));
            root.adoption_answer(call, w::Outcome::Connecting);
            first_page(&mut root, w::Connection::Connecting, state, stop);
            let printed = root.answer(call).unwrap();
            assert_eq!(printed.status, 1);
            assert!(skein_lib::bytes::find(&printed.stderr, words).is_some());
            no_token(&printed);
            assert!(crate::root::next_deadline(&root.root).is_none());
        }
    }
}

#[test]
fn admin_first_refused_as_the_bot_s_login_exits_two_naming_the_remedy() {
    for json in [false, true] {
        let mut root = f::signed_in(f::PARTY);
        let words = f::extended(ADMIN, &["--first", "temper-bot/hello"]);
        let call = root.submit_prepared(f::call(words.as_slice()), json, Some(Secret::new(TOKEN.into())));
        let w::Request::Connect(connect) = request(&root, call) else { panic!("connect") };
        let w::How::ForgejoAdmin(admin) = connect.how() else { panic!("admin connect") };
        assert_eq!(admin.first().as_ref().unwrap().org(), b"temper-bot");
        let limits = temper_client_domain::forge::wire_limits(&root.limits.domain.forge).unwrap();
        root.adoption_answer(
            call,
            w::Outcome::ConnectRefused(
                w::ConnectRefused::new(&limits, w::ConnectRefusedParts { why: w::ForgeRefusal::FirstIsBot }).unwrap(),
            ),
        );
        let printed = root.answer(call).unwrap();
        assert_eq!(printed.status, 2);
        assert!(printed.stdout.is_empty());
        assert!(
            skein_lib::bytes::find(
                &printed.stderr,
                if json { b"first_is_bot" } else { b"the organization is the bot's login: name another organization" },
            )
            .is_some()
        );
        no_token(&printed);
        assert_eq!(root.sends.len(), 1);
        assert_eq!(request(&root, call), w::Request::Connect(connect));
        assert!(crate::root::next_deadline(&root.root).is_none());
    }
}

#[test]
fn accept_sends_the_existing_id_without_a_value_and_prints_its_next_call() {
    for words in [ACCEPT, &["forge", "accept", "3", "22", "--key", "accept"]] {
        for json in [false, true] {
            let mut root = f::signed_in(f::PARTY);
            let call = root.submit_prepared(f::call(words), json, None);
            let w::Request::Accept(accept) = request(&root, call) else { panic!("accept") };
            assert_eq!((accept.forge(), accept.installation()), (3, 22));
            assert!(root.carried.is_empty());
            let limits = temper_client_domain::forge::wire_limits(&root.limits.domain.forge).expect("limits");
            root.adoption_answer(
                call,
                w::Outcome::Accepted(
                    w::AcceptedInstallation::new(&limits, w::AcceptedInstallationParts { installation: 22 })
                        .expect("accepted"),
                ),
            );
            let printed = root.answer(call).expect("accepted");
            assert_eq!(printed.status, 0);
            assert_eq!(
                printed.stdout.as_ref(),
                if json {
                    br#"{"forge":3,"accepted":22}"#.as_slice()
                } else {
                    b"accepted: installation 22 on forge 3\ntemper ctl DIR forge repositories 3 --installation 22\n"
                }
            );
        }
    }
    for words in [
        &["forge", "accept", "0", "22"][..],
        &["forge", "accept", "65536", "22"],
        &["forge", "accept", "3", "0"],
        &["forge", "accept", "3", "18446744073709551616"],
    ] {
        f::parse(words).expect_err("nonzero numbers within bounds");
    }
}
#[test]
fn a_stopped_accept_prints_its_reason_and_keeps_the_json_reason() {
    for (why, text, expected_json) in [
        (
            w::ForgeReason::NotFound,
            b"GitHub has no installation 22 of this App\n".as_slice(),
            br#"{"forge_refused":{"reason":"not_found"}}"#.as_slice(),
        ),
        (
            w::ForgeReason::NoAccess,
            b"GitHub refused the App's read of installation 22\n",
            br#"{"forge_refused":{"reason":"no_access"}}"#,
        ),
        (
            w::ForgeReason::Unreachable,
            b"GitHub did not answer: try again\n",
            br#"{"forge_refused":{"reason":"unreachable"}}"#,
        ),
        (
            w::ForgeReason::TooLarge,
            b"temper's accepted installation set is full\n",
            br#"{"forge_refused":{"reason":"too_large"}}"#,
        ),
        (
            w::ForgeReason::InvalidAnswer,
            b"GitHub answered what temper did not expect\n",
            br#"{"forge_refused":{"reason":"invalid_answer"}}"#,
        ),
    ] {
        for json in [false, true] {
            let mut root = f::signed_in(f::PARTY);
            let call = root.submit_prepared(f::call(ACCEPT), json, None);
            let limits = temper_client_domain::forge::wire_limits(&root.limits.domain.forge).expect("limits");
            root.adoption_answer(
                call,
                w::Outcome::ForgeRefused(
                    w::ForgeRefused::new(&limits, w::ForgeRefusedParts { reason: why.clone() }).expect("refusal"),
                ),
            );
            let printed = root.answer(call).expect("refused");
            assert_eq!(printed.status, 2);
            assert!(printed.stdout.is_empty());
            if json {
                assert_eq!(printed.stderr.as_ref(), expected_json);
            } else {
                assert_eq!(printed.stderr.as_ref(), text);
            }
        }
    }
}

#[test]
fn an_accept_recovers_its_durable_answer_with_its_own_operation_name() {
    for json in [false, true] {
        let mut root = f::signed_in(f::PARTY);
        let call = root.submit(ACCEPT);
        assert!(root.saved.contains_key(&call));
        let mut records = f::restored_adoption(f::PARTY);
        let record = records.get_mut(0).expect("one restore");
        record.key = root.sent_name(call).expect("engine key");
        record.request.request = root.adoption_bytes(call).clone();
        let mut root = f::started_with(records, f::SEED, json);
        root.deliver(c::Delivery::Me(Box::new(f::me(f::PARTY))));
        let limits = temper_client_domain::forge::wire_limits(&root.limits.domain.forge).expect("limits");
        root.adoption_answer(
            0,
            w::Outcome::Accepted(
                w::AcceptedInstallation::new(&limits, w::AcceptedInstallationParts { installation: 22 })
                    .expect("accepted"),
            ),
        );
        let expected = if json {
            b"{\"operation\":\"forge-accept\",\"outcome\":{\"forge\":3,\"accepted\":22}}\n".as_slice()
        } else {
            b"kept: forge-accept accepted: installation 22 on forge 3\ntemper ctl DIR forge repositories 3 --installation 22\n"
        };
        assert_eq!(root.kept_lines.as_slice(), &[Box::<[u8]>::from(expected)]);
    }
}
