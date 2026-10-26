use super::*;
use temper_engine_domain_forge as domain;
use temper_forge_setup::setup::{Account, Entry, Fake, Mode, Repository};

const HANDOVER: &[u8] = b"0123456789abcdef0123456789abcdef01234567";
const HOOK: &[u8] = b"file-hook-secret";
fn old_form_configured(hooks: bool) -> crate::settings::Engine {
    let mut config = fixtures::engine();
    config.forges[0].api = jig_protocol_config::Address::Addr(skein_io::Addr::Ip(([127, 0, 0, 1], 9).into()));
    // A listener alone must not make Adopt draw a secret the connection cannot read.
    config.webhooks =
        Some(skein_forge_connection::hooks::Webhooks { listener: skein_io::Addr::Ip(([127, 0, 0, 1], 7101).into()) });
    if hooks {
        config.forges[0].webhook_secret = Some(Secret::new(HOOK.into()));
    }
    config.setup = Box::new([crate::settings::Setup {
        entry: Entry {
            forge: 1,
            directory: b"/data/forge".as_slice().into(),
            bot: Account { login: b"temper".as_slice().into(), email: b"temper@example.test".as_slice().into() },
            orgs: Box::new([]),
            asked: false,
            mode: Mode::Fake(Fake {
                listen: ([127, 0, 0, 1], 9).into(),
                git: b"/data/git".as_slice().into(),
                owner: b"demo".as_slice().into(),
                repositories: Box::new([
                    Repository { name: b"widget".as_slice().into(), landing: b"main".as_slice().into() },
                    Repository { name: b"other".as_slice().into(), landing: b"main".as_slice().into() },
                ]),
            }),
        },
        handover: Some(Secret::new(HANDOVER.into())),
    }]);
    fixtures::fill_files(&mut config);
    config
}
fn configured(hooks: bool) -> crate::settings::Engine {
    let mut config = old_form_configured(false);
    config.records_only = Box::new([1]);
    config.forges[0].api_token = None;
    config.forges[0].user = 0;
    config.forges[0].webhook_secret = None;
    config.identities[0].token = Secret::new(Box::new([]));
    config.setup[0].handover = None;
    if !hooks {
        config.webhooks = None;
    }
    config.limits.forge.repositories = 3;
    config.limits.forge.client.repositories = 3;
    config.limits.forge.output = 96;
    fixtures::fill_files(&mut config);
    config
}
fn record_answers(service: &mut crate::Service) {
    let engine = service.engine.as_mut().unwrap();
    while let Some(event) = engine.store.above.pop() {
        answer(engine, event);
    }
    while engine.setup.answers.pop().is_some() {}
    advance(engine);
}
fn reconcile_bot(service: &mut crate::Service, face: &mut Face) {
    local_sent(service, face, b"GET /api/v1/user ");
    assert!(contains(&face.wire.received, b"test-admin"));
    face.reply(service, br#"{"id":1,"login":"temper-admin","is_admin":true}"#);
    local_sent(service, face, b"GET /api/v1/users/temper ");
    face.reply(service, br#"{"id":42,"login":"temper","full_name":"temper:bot","is_admin":false}"#);
    for token in [b"test-api".as_slice(), b"test-git"] {
        local_sent(service, face, b"GET /api/v1/user ");
        assert!(contains(&face.wire.received, token));
        face.reply(service, br#"{"id":42,"login":"temper"}"#);
    }
}
#[expect(clippy::disallowed_macros, reason = "the protocol peer formats its repository fixture")]
fn record_repository(service: &mut crate::Service, face: &mut Face, id: u64, permission: &[u8]) {
    local_sent(service, face, format!("GET /api/v1/repositories/{id} ").as_bytes());
    face.reply(service, format!(r#"{{"id":{id},"owner":{{"id":1,"login":"demo"}},"name":"widget","object_format_name":"sha1","default_branch":"main","default_merge_style":"merge","allow_merge_commits":true,"allow_squash_merge":true,"allow_rebase":true,"html_url":"http://127.0.0.1:9/demo/widget"}}"#).as_bytes());
    local_sent(service, face, b"/collaborators/temper/permission ");
    assert!(contains(&face.wire.received, b"test-admin"));
    face.reply(service, permission);
}
fn record_hook(service: &mut crate::Service, face: &mut Face, secret: &[u8]) {
    local_sent(service, face, b"GET /api/v1/settings/api ");
    face.reply(service, br#"{"max_response_items":50,"default_paging_num":30}"#);
    local_sent(service, face, b"/hooks?page=1&limit=32 ");
    face.reply(service, b"[]");
    local_sent(service, face, b"POST /api/v1/repos/demo/widget/hooks ");
    assert!(contains(&face.wire.received, b"http://127.0.0.1:7101/hooks/1"));
    assert!(contains(&face.wire.received, secret), "the hook uses the record's secret");
    assert!(service.engine.as_ref().unwrap().forge.starts.is_empty());
    face.reply(
        service,
        br#"{"id":1,"config":{"url":"http://127.0.0.1:7101/hooks/1"},"events":["push"],"active":true}"#,
    );
}
const WRITE_PERMISSION: &[u8] = br#"{"permission":"write","user":{"id":42,"login":"temper"}}"#;
fn records_reconciled(hooks: bool, handover: bool) -> (crate::Service, Face, Box<[u8]>) {
    let mut config = configured(hooks);
    if handover {
        config.setup[0].handover = Some(Secret::new(b"test-admin".as_slice().into()));
    }
    let mut service = local_service(config, local_store(true));
    restore_local(&mut service);
    let mut face = Face::new();
    if handover {
        assert_eq!(service.engine.as_ref().unwrap().setup.runs.first().unwrap().1.kind, Kind::LocalTakeOver);
        let remove = takeover_done(&mut service, &mut face);
        face.hold_done = false;
        service.handover_removed(1, remove.run, Ok(()));
        advance(service.engine.as_mut().unwrap());
    }
    assert_eq!(service.engine.as_ref().unwrap().setup.runs.first().unwrap().1.kind, Kind::LocalReconcile);
    reconcile_bot(&mut service, &mut face);
    record_repository(&mut service, &mut face, 41, WRITE_PERMISSION);
    let secret = if hooks {
        let engine = service.engine.as_ref().unwrap();
        assert_eq!(engine.setup.writes.first().unwrap().1.number, records::number(1, Item::WebhookSecret));
        record_answers(&mut service);
        let secret: Box<[u8]> = service
            .engine
            .as_ref()
            .unwrap()
            .forge_records
            .rows
            .forge(1)
            .unwrap()
            .get(Item::WebhookSecret)
            .unwrap()
            .into();
        record_hook(&mut service, &mut face, &secret);
        secret
    } else {
        Box::new([])
    };
    assert!(service.engine.as_ref().unwrap().forge.starts.is_empty());
    record_answers(&mut service);
    (service, face, secret)
}
#[test]
fn records_fake_takes_over_reconciles_adopts_in_order_then_connects_from_records() {
    for hooks in [false, true] {
        let (mut service, mut face, secret) = records_reconciled(hooks, true);
        for id in [1, 2] {
            assert_eq!(service.engine.as_ref().unwrap().setup.lifecycle.adoptions.get(&1).unwrap().next.get(), id);
            assert_eq!(service.engine.as_ref().unwrap().setup.runs.first().unwrap().1.kind, Kind::Adopt);
            assert!(service.engine.as_ref().unwrap().forge.starts.is_empty());
            local_sent(&mut service, &mut face, b"GET /api/v1/user ");
            assert!(contains(&face.wire.received, b"test-admin"));
            face.reply(&mut service, br#"{"id":1,"login":"temper-admin","is_admin":true}"#);
            face.hold_done = true;
            record_repository(&mut service, &mut face, id, WRITE_PERMISSION);
            if hooks {
                record_hook(&mut service, &mut face, &secret);
            }
            assert!(face.done.is_some());
            assert!(service.engine.as_ref().unwrap().forge.starts.is_empty(), "no Connect before the last Adopt Done");
            service.engine.as_mut().unwrap().setup.above.push(face.done.take().unwrap());
            face.hold_done = false;
            advance(service.engine.as_mut().unwrap());
        }
        let engine = service.engine.as_ref().unwrap();
        assert!(engine.setup.lifecycle.adoptions.is_empty());
        let (_, access) = engine.forge.starts.get(0).expect("Connect after every Adopt");
        assert_eq!(access.user, 42);
        assert_eq!(
            access.credential,
            skein_forge::credential::Credential::Token(Secret::new(b"test-api".as_slice().into()))
        );
        assert_eq!(access.webhook_secret, if hooks { Some(Secret::new(secret)) } else { None });
        assert_eq!(engine.forge_records.rows.forge(1).unwrap().get(Item::AdminToken).unwrap(), b"test-admin");
        assert!(service.take_handover_removal().is_none());
    }
}
#[test]
fn records_fake_stops_at_adopt_without_connecting_or_starting_the_next_repository() {
    let (mut service, mut face, _) = records_reconciled(false, false);
    local_sent(&mut service, &mut face, b"GET /api/v1/user ");
    face.reply(&mut service, br#"{"id":1,"login":"person","is_admin":false}"#);
    let engine = service.engine.as_ref().unwrap();
    assert!(engine.setup.lifecycle.adoptions.is_empty());
    assert!(engine.setup.runs.is_empty());
    assert!(engine.forge.starts.is_empty());
    assert!(engine.setup.io.is_empty());
    let mut stopped = false;
    for note in &engine.notes {
        if let crate::Note::ForgeSetupStopped { number: 1, why } = note {
            assert_eq!(
                why,
                &crate::SetupStopped::Stopped {
                    plan: crate::SetupPlan::Adopt,
                    step: crate::SetupStep { stage: reconcile::Stage::Admin, why: reconcile::Why::NotAdmin },
                }
            );
            stopped = true;
        }
    }
    assert!(stopped);
    let mut not_connected = false;
    for event in &engine.events {
        if let root::Event::Forge(value) = event
            && let domain::Event::Connection { forge: 1, state: domain::Connection::NotConnected } = value.as_ref()
        {
            not_connected = true;
        }
    }
    assert!(not_connected);
}
#[test]
fn records_fake_restart_reconciles_restored_repositories_and_adopts_only_the_missing_one() {
    for adopted in [1_u64, 2] {
        let mut store = local_store(true);
        let mut writes = List::with_capacity(2);
        for id in 1..=adopted {
            let mut repository = local_repository();
            repository.provider.repository = u32::try_from(id).unwrap();
            repository.name = if id == 1 { b"seeded-1".as_slice().into() } else { b"seeded-2".as_slice().into() };
            repository.project = u32::try_from(id + 1).unwrap();
            writes
                .push(root::Write::Save(root::Record::Forge { row: Box::new(domain::Stored::Repository(repository)) }))
                .unwrap();
        }
        fixtures::commit(&mut store, 2, writes.into_boxed());
        assert!(store.above.pop().is_some());
        let mut service = local_service(configured(false), store);
        restore_local(&mut service);
        assert_eq!(service.engine.as_ref().unwrap().domain.adopted_ids(1).len(), usize::try_from(adopted + 1).unwrap());
        let mut face = Face::new();
        reconcile_bot(&mut service, &mut face);
        for id in 1..=adopted {
            record_repository(&mut service, &mut face, id, WRITE_PERMISSION);
        }
        record_repository(&mut service, &mut face, 41, WRITE_PERMISSION);
        record_answers(&mut service);
        if adopted == 1 {
            local_sent(&mut service, &mut face, b"GET /api/v1/user ");
            assert_eq!(service.engine.as_ref().unwrap().setup.runs.first().unwrap().1.kind, Kind::Adopt);
            face.reply(&mut service, br#"{"id":1,"login":"temper-admin","is_admin":true}"#);
            record_repository(&mut service, &mut face, 2, WRITE_PERMISSION);
        }
        assert_eq!(service.engine.as_ref().unwrap().forge.starts.len(), 1);
        assert!(service.engine.as_ref().unwrap().setup.lifecycle.adoptions.is_empty());
        assert!(service.engine.as_ref().unwrap().setup.runs.is_empty());
    }
}
#[test]
fn records_fake_given_uses_the_webhooks_listener_without_a_file_secret() {
    for hooks in [false, true] {
        let config = configured(hooks);
        let (endpoints, _) = crate::forge::prepare(&config, fixtures::forge_runtime()).unwrap();
        let (settings, _) = settings::fixed(&config, &endpoints).unwrap();
        let host::Kind::Forgejo(given) = &settings.forges.get(0).unwrap().kind else { panic!("Forgejo Given") };
        assert_eq!(given.listener, if hooks { Some(([127, 0, 0, 1], 7101).into()) } else { None });
    }
}
fn old_form_service(hooks: bool) -> crate::Service {
    let config = old_form_configured(hooks);
    assert_eq!(
        crate::startup::application::configured(&config).as_ref(),
        [domain::Configured {
            bot: b"temper".as_slice().into(),
            admin_connect: false,
            owned: None,
            web: None,
            read_only: false,
            number: 1,
            kind: domain::Kind::Forgejo,
            mode: domain::Mode::Fake,
            asked: false
        }]
    );
    let mut service = crate::Service::engine(config, &fixtures::seeds(), None, fixtures::forge_runtime()).unwrap();
    service.engine.as_mut().expect("engine").store = fixtures::records_store();
    advance(service.engine.as_mut().expect("engine"));
    assert!(service.engine.as_ref().expect("engine").setup.runs.is_empty(), "wait for records");
    service.engine.as_mut().expect("engine").forge.iterate(Time::ZERO, Wall::EPOCH);
    for _ in 0..service.engine.as_mut().expect("engine").forge.io.len() {
        let Some(skein_io::Request::Listen { addr, .. }) = service.engine.as_mut().expect("engine").forge.io.pop()
        else {
            panic!("no API connection without access")
        };
        // The fixture supplies the forge's successful bind result before starting setup.
        service.engine.as_mut().expect("engine").setup.webhooks_listening(addr.socket().unwrap());
    }
    for _ in 0_u32..8 {
        crate::forge::records::advance(service.engine.as_mut().expect("engine"));
    }
    advance(service.engine.as_mut().expect("engine"));
    assert!(service.engine.as_ref().expect("engine").forge_records.done());
    assert_eq!(service.engine.as_ref().expect("engine").setup.runs.len(), 1);
    service
}
fn sent(service: &mut crate::Service, face: &mut Face, request: &[u8]) {
    face.sent(service);
    assert!(contains(&face.wire.received, request), "wanted {request:?}, got {:?}", face.wire.received);
    assert!(contains(&face.wire.received, HANDOVER), "setup uses the handover");
}
fn repository(service: &mut crate::Service, face: &mut Face, second: bool) {
    sent(service, face, b"GET /api/v1/user ");
    let run = *service.engine.as_mut().expect("engine").setup.runs.first().unwrap().0;
    assert_eq!(service.engine.as_ref().expect("engine").setup.runs.get(&run).unwrap().kind, Kind::Adopt);
    assert!(service.engine.as_ref().expect("engine").setup.runs.get(&run).unwrap().internal);
    assert_eq!(
        service.engine.as_ref().expect("engine").setup.lifecycle.adoptions.get(&1).unwrap().next.get(),
        if second { 2 } else { 1 }
    );
    assert_eq!(
        service.engine.as_mut().expect("engine").setup.answers.pop(),
        Some((run, host::Answer::Read(Some(Secret::new(HANDOVER.into())))))
    );
    face.reply(service, br#"{"id":1,"login":"temper-admin","is_admin":true}"#);
    sent(service, face, if second { b"GET /api/v1/repositories/2 " } else { b"GET /api/v1/repositories/1 " });
    face.reply(
        service,
        if second {
            br#"{"id":2,"owner":{"id":1,"login":"demo"},"name":"other","object_format_name":"sha1","default_branch":"main","default_merge_style":"merge","allow_merge_commits":true,"allow_squash_merge":true,"allow_rebase":true,"html_url":"http://127.0.0.1:9/demo/widget"}"#
        } else {
            br#"{"id":1,"owner":{"id":1,"login":"demo"},"name":"widget","object_format_name":"sha1","default_branch":"main","default_merge_style":"merge","allow_merge_commits":true,"allow_squash_merge":true,"allow_rebase":true,"html_url":"http://127.0.0.1:9/demo/widget"}"#
        },
    );
    sent(service, face, b"/collaborators/temper/permission ");
    face.reply(service, br#"{"permission":"read","user":{"id":42,"login":"temper"}}"#);
    sent(
        service,
        face,
        if second {
            b"PUT /api/v1/repos/demo/other/collaborators/temper "
        } else {
            b"PUT /api/v1/repos/demo/widget/collaborators/temper "
        },
    );
    assert!(contains(&face.wire.received, br#""permission":"write""#));
    assert!(service.engine.as_ref().expect("engine").forge.starts.is_empty());
    face.wire.write(b"HTTP/1.1 204 No Content\r\n\r\n");
    for _ in 0_u32..1024 {
        face.turn(service);
    }
}
#[test]
fn old_form_fake_repositories_are_adopted_in_order_then_connected_with_the_files_access() {
    for hooks in [false, true] {
        let mut service = old_form_service(hooks);
        let mut face = Face::new();
        for second in [false, true] {
            repository(&mut service, &mut face, second);
            if hooks {
                let run = *service.engine.as_mut().expect("engine").setup.runs.first().unwrap().0;
                assert_eq!(
                    service.engine.as_mut().expect("engine").setup.answers.pop(),
                    Some((run, host::Answer::Read(Some(Secret::new(HOOK.into())))))
                );
                sent(&mut service, &mut face, b"GET /api/v1/settings/api ");
                face.reply(&mut service, br#"{"max_response_items":50,"default_paging_num":30}"#);
                sent(&mut service, &mut face, b"/hooks?page=1&limit=32 ");
                face.reply(&mut service, b"[]");
                sent(
                    &mut service,
                    &mut face,
                    if second {
                        b"POST /api/v1/repos/demo/other/hooks "
                    } else {
                        b"POST /api/v1/repos/demo/widget/hooks "
                    },
                );
                assert!(contains(&face.wire.received, b"http://127.0.0.1:7101/hooks/1"));
                assert!(contains(&face.wire.received, HOOK));
                assert!(service.engine.as_ref().expect("engine").forge.starts.is_empty());
                face.reply(
                    &mut service,
                    br#"{"id":1,"config":{"url":"http://127.0.0.1:7101/hooks/1"},"events":["push"],"active":true}"#,
                );
            }
            assert_eq!(service.engine.as_ref().expect("engine").forge.starts.len(), u32::from(second));
        }
        assert!(service.engine.as_ref().expect("engine").setup.lifecycle.adoptions.is_empty());
        assert!(service.engine.as_ref().expect("engine").setup.runs.is_empty());
        let (forge, access) = service.engine.as_mut().expect("engine").forge.starts.get(0).unwrap();
        assert_eq!((*forge, access.user), (1, 42));
        assert_eq!(
            access.credential,
            skein_forge::credential::Credential::Token(Secret::new(b"api-token".as_slice().into()))
        );
        assert_eq!(access.webhook_secret, if hooks { Some(Secret::new(HOOK.into())) } else { None });
        assert!(!service.engine.as_ref().expect("engine").forge.io.is_empty(), "Connect starts the Forgejo user check");
        lifecycle::connected(service.engine.as_mut().expect("engine"), 1);
        advance(service.engine.as_mut().expect("engine"));
        assert!(service.engine.as_ref().expect("engine").setup.runs.is_empty(), "no fake Reconcile");
        let Some(root::Event::Forge(_)) = service.engine.as_mut().expect("engine").events.pop() else {
            panic!("Forgejo connected event")
        };
        assert!(service.engine.as_ref().expect("engine").workers.forges[0].api_token.is_none());
    }
}
#[test]
fn a_stopped_old_form_fake_adoption_names_its_step_and_drops_the_remaining_repositories_and_access() {
    let mut service = old_form_service(false);
    let mut face = Face::new();
    sent(&mut service, &mut face, b"GET /api/v1/user ");
    face.reply(&mut service, br#"{"id":1,"login":"person","is_admin":false}"#);
    let root::SetupEnd::Stopped { forge, reason } = terminal(&mut service) else { panic!("stopped adoption") };
    assert_eq!(forge, 1);
    assert_eq!(reason.as_ref(), b"setup Admin temper-admin: NotAdmin");
    assert!(service.engine.as_ref().expect("engine").setup.lifecycle.adoptions.is_empty());
    assert!(service.engine.as_ref().expect("engine").setup.runs.is_empty());
    assert!(service.engine.as_ref().expect("engine").forge.starts.is_empty());
    assert!(service.engine.as_ref().expect("engine").setup.io.is_empty());
    assert_eq!(service.engine.as_ref().expect("engine").domain.forge_connection(1), domain::Connection::NotConnected);
    assert!(service.end().is_none(), "the engine runs on");
}
#[test]
fn old_form_fake_given_uses_the_webhooks_listener_only_with_a_file_secret() {
    for hooks in [false, true] {
        let config = old_form_configured(hooks);
        let (endpoints, _) = crate::forge::prepare(&config, fixtures::forge_runtime()).unwrap();
        let (settings, _) = settings::fixed(&config, &endpoints).unwrap();
        let host::Kind::Forgejo(given) = &settings.forges.get(0).unwrap().kind else { panic!("Forgejo Given") };
        assert_eq!(given.listener, if hooks { Some(([127, 0, 0, 1], 7101).into()) } else { None });
        assert_eq!(given.bot_login.as_ref(), b"temper");
        assert!(given.adopted.is_empty());
    }
}
#[test]
fn old_form_owned_and_managed_serve_reads_without_start_time_writing_plans() {
    for managed in [false, true] {
        let mut config = old_form_configured(false);
        config.setup[0].handover = None;
        config.setup[0].entry.mode = if managed {
            Mode::Managed { admin_token: Some(b"/data/admin".as_slice().into()) }
        } else {
            fixtures::owned_setup(1).entry.mode
        };
        assert_eq!(
            crate::startup::application::configured(&config).as_ref(),
            [domain::Configured {
                bot: b"temper".as_slice().into(),
                admin_connect: false,
                owned: if managed {
                    None
                } else {
                    Some(domain::Owned {
                        forgejo: skein_forge::forgejo::pin::VERSION.as_bytes().into(),
                        runner: None,
                        password: b"/data/forge/person_password".as_slice().into(),
                    })
                },
                web: None,
                read_only: false,
                number: 1,
                kind: domain::Kind::Forgejo,
                mode: if managed { domain::Mode::Managed } else { domain::Mode::Owned },
                asked: false,
            }]
        );
        let (mut endpoints, _) = crate::forge::prepare(&config, fixtures::forge_runtime()).unwrap();
        let setup = prepare(&config, &mut endpoints, &fixtures::seeds().crypto, 2, false).unwrap();
        assert!(setup.lifecycle.adoptions.is_empty());
        assert!(setup.runs.is_empty());
        assert!(endpoints.forges.get(0).unwrap().access.is_some());
        assert!(setup.served.contains_key(&1));
        let (settings, _) = setup.pending.unwrap();
        let host::Kind::Forgejo(_) = &settings.forges.get(0).unwrap().kind else { panic!("Forgejo Given") };
        let mut service = crate::Service::engine(config, &fixtures::seeds(), None, fixtures::forge_runtime()).unwrap();
        service.engine.as_mut().expect("engine").store = fixtures::records_store();
        for _ in 0_u32..8 {
            crate::forge::records::advance(service.engine.as_mut().expect("engine"));
        }
        advance(service.engine.as_mut().expect("engine"));
        assert!(
            service.engine.as_ref().expect("engine").setup.runs.is_empty(),
            "no start-time writing plan for owned or managed"
        );
        assert!(service.engine.as_ref().expect("engine").setup.io.is_empty());
    }
}

#[test]
fn an_unserved_old_form_fake_drops_its_adoptions_and_names_the_exclusion_without_connecting() {
    let config = old_form_configured(false);
    let (mut endpoints, _) = crate::forge::prepare(&config, fixtures::forge_runtime()).unwrap();
    endpoints.forges.get_mut(0).unwrap().address = skein_io::Addr::Ip(([192, 0, 2, 7], 9).into());
    let setup = prepare(&config, &mut endpoints, &fixtures::seeds().crypto, 2, false).unwrap();
    assert!(endpoints.forges.get(0).unwrap().access.is_none());
    let mut service = old_form_service(false);
    service.engine.as_mut().expect("engine").setup = setup;
    let root::SetupEnd::Stopped { forge, reason } = terminal(&mut service) else { panic!("unserved fake") };
    assert_eq!(forge, 1);
    assert_eq!(reason.as_ref(), b"setup host serves plaintext only on loopback");
    assert!(service.engine.as_ref().expect("engine").setup.lifecycle.adoptions.is_empty());
    assert!(service.engine.as_ref().expect("engine").setup.runs.is_empty());
    assert!(service.engine.as_ref().expect("engine").forge.starts.is_empty());
    assert!(service.engine.as_ref().expect("engine").setup.io.is_empty());
}

#[test]
fn an_unresolved_old_form_fake_keeps_its_entry_but_suppresses_file_access_and_adoption() {
    let mut config = old_form_configured(false);
    config.forges[0].api = jig_protocol_config::Address::Host { name: b"fake.example".as_slice().into(), port: 9 };
    let mut runtime = fixtures::forge_runtime();
    runtime.get_mut(0).unwrap().address = crate::forge::HELD_ADDRESS;
    let mut service = crate::Service::engine(config, &fixtures::seeds(), None, runtime).unwrap();
    let engine = service.engine.as_mut().unwrap();
    assert!(engine.setup.served.contains_key(&1));
    assert!(engine.setup.lifecycle.adoptions.is_empty());
    engine.store = fixtures::records_store();
    engine.forge_records.loaded(Box::new([]), None);
    for _ in 0_u32..8 {
        crate::forge::records::advance(engine);
    }
    assert_eq!(
        engine.notes.pop(),
        Some(crate::Note::ForgeNotConnected {
            number: 1,
            why: crate::NotConnected::Unreachable { host: b"fake.example".as_slice().into() }
        })
    );
    engine.forge.iterate(Time::ZERO, Wall::EPOCH);
    advance(engine);
    assert!(engine.forge.starts.is_empty());
    assert!(engine.setup.runs.is_empty());
    assert!(engine.setup.io.is_empty());
    for request in &engine.forge.io {
        if let skein_io::Request::Connect { .. } = request {
            panic!("the held forge never dials");
        }
    }
}

#[test]
fn an_unresolved_old_form_forgejos_asked_check_is_refused_before_its_unstaged_connected_path() {
    let mut config = old_form_configured(false);
    config.forges[0].api = jig_protocol_config::Address::Host { name: b"fake.example".as_slice().into(), port: 9 };
    let mut runtime = fixtures::forge_runtime();
    runtime.get_mut(0).unwrap().address = crate::forge::HELD_ADDRESS;
    let mut service = crate::Service::engine(config, &fixtures::seeds(), None, runtime).unwrap();
    let engine = service.engine.as_mut().unwrap();
    engine.store = fixtures::records_store();
    engine.forge_records.loaded(Box::new([]), None);
    for _ in 0_u32..8 {
        crate::forge::records::advance(engine);
    }
    advance(engine);
    start(&mut service, 1, host::Plan::Check { forge: 1 });
    let root::SetupEnd::Stopped { forge, reason } = terminal(&mut service) else { panic!("held Check refused") };
    assert_eq!(forge, 1);
    assert_eq!(reason.as_ref(), b"unreachable: fake.example");
    let engine = service.engine.as_ref().unwrap();
    assert!(engine.forge.starts.is_empty());
    assert!(engine.setup.runs.is_empty());
    assert!(engine.setup.io.is_empty());
}
