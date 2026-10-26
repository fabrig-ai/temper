//! Refusal cleanup through the service's actual handler and routed store answers.
use super::*;
use crate::tests::fixtures;
use skein_lib::{Duration, bytes};

#[test]
fn a_revoked_record_waits_for_erased_and_coalesces_a_sign_out() {
    let generalized = Map::<u32, Erasing>::worst_case(2);
    let previous = Map::<u32, Option<bool>>::worst_case(2);
    assert_eq!(generalized, previous, "the generalized pending state fits the existing map bound");
    for (notifications, expected_signed_out, removing) in
        [(&[][..], None, false), (&[true, true][..], Some(true), true), (&[false, false][..], Some(false), true)]
    {
        let mut service = fixtures::started();
        for _ in 0_u32..8 {
            fixtures::turn(&mut service);
        }
        let engine = service.engine.as_mut().expect("engine");
        let (config, runtime) = registered();
        let mut accounts = host(&config, runtime);
        accounts.request(domain::Request::Signing(domain::Signing::Restore {
            source: 1,
            provider: 3,
            kind: domain::Kind::SignIn,
        }));
        let candidate = accounts.env.limits.client.document;
        let records::Next::Store(old_keep) = accounts.records.keep(1, record(1, &candidate), &candidate) else {
            panic!("old record");
        };
        replace(&mut engine.store, old_keep);
        accounts.records.kept(1);
        while engine.store.above.pop().is_some() {}
        let mut events = Queue::with_capacity(16);
        let old = 1;
        let fresh = 9001;
        let candidate = accounts.env.limits.client.document;
        let mut record = record(fresh, &candidate);
        let records::Next::Store(keep) = accounts.records.keep(fresh, record.clone(), &candidate) else {
            panic!("fresh source is kept");
        };
        replace(&mut engine.store, keep);
        assert!(accounts.records.kept(fresh).is_some());
        // Hold unrelated store answers apart from this erase's terminal.
        while engine.store.above.pop().is_some() {}
        engine
            .values
            .accounts
            .granted(
                fresh,
                2,
                table::Value { token: Secret::new(bytes::copy_of(b"fresh-token")), scheme: table::Scheme::Bearer },
                Time::from_nanos(Duration::from_secs(60).as_nanos()),
            )
            .expect("fresh value fits");
        accounts.above.push(driver::Event::Failed {
            account: old,
            ends: driver::Ends::Refresh,
            failure: driver::Failure::Exchange(oauth::Failure::Refused),
        });
        accounts.iterate(Time::from_nanos(1), Wall::from_nanos(1), &mut engine.values, &mut events, &mut engine.store);
        let Some(root::Event::Account(domain::Event::Component(domain::FromComponent::Failed {
            account: 1,
            failure: domain::Failure::SignInAgain,
        }))) = events.pop()
        else {
            panic!("revocation fails needs sign-in immediately");
        };
        assert!(engine.values.accounts.value(old, 1, Time::from_nanos(1)).is_none());
        let Some(Erasing::Revoked) = accounts.erasing.get(&old) else {
            panic!("erase awaits durability");
        };
        record.key = old;
        let records::Next::Store(before) = accounts.records.keep(old, record.clone(), &candidate) else {
            panic!("the old slot remains admitted");
        };
        assert!(before.replaces.is_some(), "submission did not clear the held digest");
        assert!(accounts.records.stale(old).is_some());
        for confirmed in notifications {
            accounts.above.push(driver::Event::SignedOut { account: old, revoked: *confirmed });
        }
        accounts.iterate(Time::from_nanos(1), Wall::from_nanos(1), &mut engine.values, &mut events, &mut engine.store);
        assert_eq!(engine.store.above.len(), 1, "one erase for revocation and concurrent sign-out");
        assert!(events.is_empty(), "sign-out awaits erasure");
        let terminal = engine.store.above.pop().expect("held erase acknowledgement");
        let store::Event::Erased { .. } = terminal else {
            panic!("held erase acknowledgement");
        };
        engine.store.sources.push(terminal);
        accounts.answers(&mut engine.store, &mut events);
        // A duplicate acknowledgement and an unknown old-source acknowledgement do nothing.
        engine.store.sources.push(store::Event::Erased { owner: Token::new(PREFIX | u64::from(old)) });
        engine.store.sources.push(store::Event::Erased { owner: Token::new(PREFIX | u64::from(8000_u32)) });
        accounts.answers(&mut engine.store, &mut events);
        assert_eq!(signed_out(&mut events, old), expected_signed_out);
        let records::Next::Store(after) = accounts.records.keep(old, record, &candidate) else {
            panic!("erasure retained the slot");
        };
        assert_eq!(after.replaces, None, "only the acknowledgement cleared the digest");
        assert!(accounts.required.contains_key(&old), "only Removed frees the driver's place");
        assert_eq!(accounts.erasing.contains_key(&old), removing);
        assert_eq!(accounts.above.is_empty(), !removing, "only sign-out asks the driver to remove its account");
        assert!(engine.values.accounts.value(fresh, 2, Time::from_nanos(1)).is_some());
        let Some(store::Event::Secret { value: Some(_), .. }) = engine.store.secrets(
            Token::new(99),
            store::Secrets::Get { owner: store::SecretOwner::Source, number: u64::from(fresh) },
        ) else {
            panic!("the fresh different source keeps its record");
        };
    }
}

fn record(source: u32, limits: &oauth::Limits) -> oauth::SavedToken {
    let record = oauth::SavedToken {
        key: source,
        generation: 2,
        access_token: bytes::copy_of(b"fresh-token"),
        refresh_token: Some(bytes::copy_of(b"fresh-refresh-token")),
        metadata: None,
        expires_at: Wall::from_nanos(Duration::from_secs(60).as_nanos()),
        issued_client_id: None,
        granted_scopes: Some(bytes::copy_of(b"openid read")),
        subject: None,
        id_token: None,
        earliest_refresh_at: None,
    };
    assert!(oauth::encode_record(&record, limits).is_ok(), "record fits");
    record
}
fn signed_out(events: &mut Queue<root::Event>, old: u32) -> Option<bool> {
    let event = events.pop()?;
    let root::Event::Account(domain::Event::SignedOut { source, confirmed }) = event else {
        panic!("only a sign-out terminal follows erasure");
    };
    assert_eq!(source, old);
    assert!(events.is_empty(), "one terminal despite duplicate acknowledgements");
    Some(confirmed)
}

fn carried(engine: &mut crate::engine::Engine, request: store::Secrets) -> u64 {
    let _later = engine.store.secrets(Token::new(123), request);
    let Some(store::Event::Carried { number, .. }) = engine.store.above.pop() else { panic!("carried LLM key") };
    number
}
fn carry_llm_key(engine: &mut crate::engine::Engine, value: &[u8]) -> u64 {
    carried(engine, store::Secrets::Carry { kind: store::CarriedKind::Key, value: Secret::new(value.into()) })
}
fn held_address(engine: &mut crate::engine::Engine) -> u64 {
    carried(
        engine,
        store::Secrets::Hold {
            value: Secret::new(b"http://127.0.0.1:7103/callback?state=fake&code=fake".as_slice().into()),
        },
    )
}
fn pump_accounts(accounts: &mut Accounts, engine: &mut crate::engine::Engine) -> List<domain::FromComponent> {
    let mut events = Queue::with_capacity(64);
    let mut heard = List::with_capacity(64);
    for _ in 0_u32..32 {
        while let Some(event) = engine.store.above.pop() {
            engine.store.sources.push(event);
        }
        accounts.iterate(Time::from_nanos(1), Wall::from_nanos(1), &mut engine.values, &mut events, &mut engine.store);
        while let Some(event) = events.pop() {
            if let root::Event::Account(domain::Event::Component(event)) = event {
                heard.push(event).expect("bounded component events");
            }
        }
    }
    heard
}
#[test]
fn llm_key_work_reaches_the_store_and_the_workers_table_through_the_service() {
    let mut service = fixtures::started();
    for _ in 0_u32..8 {
        fixtures::turn(&mut service);
    }
    let engine = service.engine.as_mut().expect("engine");
    let mut accounts = engine.accounts.take().expect("accounts component");
    let carried = carry_llm_key(engine, b"first-llm-key");
    accounts.request(domain::Request::Signing(domain::Signing::Keep { source: 7, carried, replaces: false }));
    assert!(has_llm_key_grant(&pump_accounts(&mut accounts, engine), 1));
    assert_eq!(
        engine.values.accounts.value(7, 1, Time::from_nanos(1)).expect("worker value").token.expose(),
        b"first-llm-key"
    );
    let carried = carry_llm_key(engine, b"next-llm-key");
    accounts.request(domain::Request::Signing(domain::Signing::Keep { source: 7, carried, replaces: true }));
    assert!(has_llm_key_grant(&pump_accounts(&mut accounts, engine), 2));
    accounts.request(domain::Request::Signing(domain::Signing::Forget { source: 7 }));
    pump_accounts(&mut accounts, engine);
    assert!(engine.values.accounts.value(7, 1, Time::from_nanos(1)).is_none());
    assert!(engine.values.accounts.value(7, 2, Time::from_nanos(1)).is_none());
    let Some(store::Event::Secret { value: None, .. }) =
        engine.store.secrets(Token::new(123), store::Secrets::Get { owner: store::SecretOwner::Source, number: 7 })
    else {
        panic!("removed LLM key value")
    };
}
#[test]
fn service_restart_restores_an_llm_key_before_want_and_sweeps_only_unnamed_sources() {
    let mut service = fixtures::started();
    for _ in 0_u32..8 {
        fixtures::turn(&mut service);
    }
    let engine = service.engine.as_mut().expect("engine");
    let mut accounts = engine.accounts.take().expect("accounts component");
    let carried = carry_llm_key(engine, b"kept-llm-key");
    accounts.request(domain::Request::Signing(domain::Signing::Keep { source: 7, carried, replaces: false }));
    pump_accounts(&mut accounts, engine);
    engine.values.accounts.remove(7);
    let carried_orphan = carry_llm_key(engine, b"orphaned-carried-value");
    let _later = engine.store.secrets(
        Token::new(123),
        store::Secrets::Keep {
            owner: store::SecretOwner::Source,
            number: 8,
            value: jig_protocol_store::SecretValue::Record(Secret::new(b"orphaned-source-value".as_slice().into())),
        },
    );
    while engine.store.above.pop().is_some() {}
    let mut fresh_service = fixtures::started();
    let mut restarted = fresh_service.engine.as_mut().expect("engine").accounts.take().expect("new component");
    restarted.request(domain::Request::Signing(domain::Signing::Restore {
        source: 7,
        provider: 1,
        kind: domain::Kind::Key,
    }));
    restarted.request(domain::Request::Component(domain::ToComponent::Want { account: 7 }));
    restarted.request(domain::Request::Signing(domain::Signing::Restored));
    assert!(has_llm_key_grant(&pump_accounts(&mut restarted, engine), 1));
    assert_eq!(
        engine.values.accounts.value(7, 1, Time::from_nanos(1)).expect("restored value").token.expose(),
        b"kept-llm-key"
    );
    for (owner, number, exists) in [
        (store::SecretOwner::Source, 1, false),
        (store::SecretOwner::Source, 7, true),
        (store::SecretOwner::Source, 8, false),
        (store::SecretOwner::Application, carried_orphan, true),
    ] {
        let Some(store::Event::Secret { value, .. }) =
            engine.store.secrets(Token::new(123), store::Secrets::Get { owner, number })
        else {
            panic!("stored value")
        };
        assert_eq!(value.is_some(), exists, "{owner:?} {number}");
    }
}

fn has_llm_key_grant(heard: &List<domain::FromComponent>, wanted: u64) -> bool {
    for event in heard {
        if let domain::FromComponent::Granted {
            account: 7,
            generation,
            form: domain::Form::Credential(domain::Scheme::Key),
            ..
        } = event
            && *generation == wanted
        {
            return true;
        }
    }
    false
}

#[test]
fn aborting_does_not_submit_an_llm_key_adoption_to_the_closing_store() {
    let mut service = fixtures::started();
    for _ in 0_u32..8 {
        fixtures::turn(&mut service);
    }
    let engine = service.engine.as_mut().expect("engine");
    let mut accounts = engine.accounts.take().expect("accounts component");
    let carried = carry_llm_key(engine, b"aborted-llm-key");
    accounts.request(domain::Request::Signing(domain::Signing::Keep { source: 7, carried, replaces: false }));
    let mut events = Queue::with_capacity(64);
    accounts.iterate(Time::from_nanos(1), Wall::from_nanos(1), &mut engine.values, &mut events, &mut engine.store);
    assert!(!accounts.key_out.is_empty(), "adoption has not been submitted yet");
    accounts.abort();
    let heard = pump_accounts(&mut accounts, engine);
    assert!(!has_llm_key_grant(&heard, 1));
    assert!(accounts.is_empty(), "abort settles the component");
    let Some(store::Event::Secret { value: None, .. }) =
        engine.store.secrets(Token::new(123), store::Secrets::Get { owner: store::SecretOwner::Source, number: 7 })
    else {
        panic!("unadopted LLM key")
    };
    let Some(store::Event::Secret { value: Some(_), .. }) = engine
        .store
        .secrets(Token::new(123), store::Secrets::Get { owner: store::SecretOwner::Application, number: carried })
    else {
        panic!("carried value remains an orphan")
    };
}

fn registered() -> (config::Engine, AccountsRuntime) {
    let mut config = fixtures::engine().jig;
    config.llm.providers = Box::new([config::llm::Provider {
        number: 3,
        name: b"issuer".as_slice().into(),
        signed_in_plan: false,
        manage_usage: None,
        models: Box::new([]),
        keys: true,
        programs: Box::new([]),
        sign_in: Some(config::llm::Registration {
            issuer: b"http://127.0.0.1:7102".as_slice().into(),
            client: b"client".as_slice().into(),
            scopes: Box::new([b"openid".as_slice().into(), b"read".as_slice().into()]),
            name_hint: None,
            authorization: b"http://127.0.0.1:7102/authorize".as_slice().into(),
            token: b"http://127.0.0.1:7102/token".as_slice().into(),
            revocation: None,
            keys: b"http://127.0.0.1:7102/keys".as_slice().into(),
            endpoint_transport: config::Transport::Plaintext,
            must_grant: Box::new([b"read".as_slice().into()]),
            resource: None,
            redirect_path: b"callback".as_slice().into(),
            redirect_port: None,
        }),
    }]);
    let endpoint =
        || driver::Endpoint { address: ([127, 0, 0, 1], 7102).into(), transport: driver::Transport::Plaintext };
    let mut origins = List::with_capacity(1);
    origins.push(driver::Origin { uri: b"http://127.0.0.1:7102".as_slice().into(), endpoint: endpoint() }).unwrap();
    let mut providers = List::with_capacity(1);
    providers.push((3, endpoint(), origins)).unwrap();
    (config, AccountsRuntime { providers, trust_roots: 0, trust_root_bytes: 0 })
}
fn host(config: &config::Engine, runtime: AccountsRuntime) -> Accounts {
    Accounts::new(
        crate::system::ACCOUNT_APPLICATION.into(),
        config,
        &fixtures::seeds(),
        Some(runtime),
        64,
        &config.limits.store.secrets,
    )
    .expect("public registration fits")
}
#[test]
fn dynamic_sources_use_provider_templates_and_restore_their_kept_records() {
    let (config, runtime) = registered();
    let mut accounts = host(&config, runtime);
    let mut service = fixtures::started();
    fixtures::restart(&mut service);
    let engine = service.engine.as_mut().expect("engine");
    let _started = pump_accounts(&mut accounts, engine);
    // Source numbers are independent of the provider and configured source.
    accounts.request(domain::Request::Signing(domain::Signing::SignIn { source: 9001, provider: 3 }));
    let _started = pump_accounts(&mut accounts, engine);
    assert!(accounts.required.contains_key(&9001));
    assert_eq!(&**accounts.required.get(&9001).expect("required scopes"), b"read".as_slice());
    let mut saved = record(9001, &config.limits.driver.client.document);
    saved.metadata = Some(b"provider-subject".as_slice().into());
    let records::Next::Store(keep) = accounts.records.keep(9001, saved, &config.limits.driver.client.document) else {
        panic!("record reaches durable store");
    };
    replace(&mut engine.store, keep);
    let _kept = pump_accounts(&mut accounts, engine);
    let (config, runtime) = registered();
    let mut restored = host(&config, runtime);
    restored.request(domain::Request::Signing(domain::Signing::Restore {
        source: 9001,
        provider: 3,
        kind: domain::Kind::SignIn,
    }));
    restored.request(domain::Request::Signing(domain::Signing::Restored));
    let _restored = pump_accounts(&mut restored, engine);
    assert_eq!(&**restored.required.get(&9001).expect("restored scopes"), b"read".as_slice());
    assert!(!restored.inventory.active());
    restored.above.push(driver::Event::Failed {
        account: 9001,
        ends: driver::Ends::Refresh,
        failure: driver::Failure::Exchange(oauth::Failure::Refused),
    });
    let _revoked = pump_accounts(&mut restored, engine);
    let Some(store::Event::Secret { value: None, .. }) =
        engine.store.secrets(Token::new(123), store::Secrets::Get { owner: store::SecretOwner::Source, number: 9001 })
    else {
        panic!("revoked row erased")
    };
    let (config, runtime) = registered();
    let mut restarted = host(&config, runtime);
    restarted.request(domain::Request::Signing(domain::Signing::Restore {
        source: 9001,
        provider: 3,
        kind: domain::Kind::SignIn,
    }));
    restarted.request(domain::Request::Signing(domain::Signing::Restored));
    restarted.request(domain::Request::Component(domain::ToComponent::Want { account: 9001 }));
    let heard = pump_accounts(&mut restarted, engine);
    let mut unloaded = false;
    for event in &heard {
        if let domain::FromComponent::Failed { account: 9001, failure: domain::Failure::Unloaded { .. } } = event {
            unloaded = true;
        }
    }
    assert!(unloaded);
    assert!(engine.values.accounts.value(9001, 2, Time::from_nanos(1)).is_none());
}
#[test]
fn missing_registration_runtime_is_refused_before_start() {
    let (config, _) = registered();
    assert!(
        Accounts::new(
            crate::system::ACCOUNT_APPLICATION.into(),
            &config,
            &fixtures::seeds(),
            None,
            64,
            &config.limits.store.secrets
        )
        .is_err()
    );
}
#[test]
fn overflowing_account_bounds_are_refused_before_start() {
    const LAST_ACCOUNT_COUNT: u32 = 536_835_027;
    let (config, _) = registered();
    let mut overflow = config.limits.driver;
    overflow.accounts = u32::MAX;
    overflow.client.document.record_bytes = u32::MAX;
    // With schema-bounded moved records and 64 queue slots this is the final account count
    // whose driver records, moves, finishes, tree nodes and queued payloads fit in u64.
    overflow.accounts = LAST_ACCOUNT_COUNT;
    assert!(worst_case(&overflow, 64, config.limits.store.secrets.page.rows).is_some());
    overflow.accounts = LAST_ACCOUNT_COUNT + 1;
    assert_eq!(worst_case(&overflow, 64, config.limits.store.secrets.page.rows), None);
    overflow.accounts = u32::MAX;
    assert_eq!(worst_case(&overflow, u32::MAX, u32::MAX), None);
}

#[test]
fn a_restored_sign_ins_metadata_leaves_its_grant_a_bearer() {
    const SOURCE: u32 = 71;
    let (config, runtime) = registered();
    let mut saved = record(SOURCE, &config.limits.driver.client.document);
    saved.metadata = Some(bytes::copy_of(b"private-account-id"));
    saved.expires_at = Wall::from_nanos(Duration::from_secs(3_600).as_nanos());
    saved.issued_client_id = Some(bytes::copy_of(b"issued-client"));
    let mut service = fixtures::started();
    fixtures::restart(&mut service);
    let engine = service.engine.as_mut().expect("engine");
    let _kept = engine.store.secrets(
        Token::new(123),
        store::Secrets::Replace {
            owner: store::SecretOwner::Source,
            number: u64::from(SOURCE),
            value: jig_protocol_store::SecretValue::Record(Secret::new(
                oauth::encode_record(&saved, &config.limits.driver.client.document).expect("record fits"),
            )),
            replaces: None,
        },
    );
    while engine.store.above.pop().is_some() {}
    let mut accounts = host(&config, runtime);
    accounts.request(domain::Request::Signing(domain::Signing::Restore {
        source: SOURCE,
        provider: 3,
        kind: domain::Kind::SignIn,
    }));
    accounts.request(domain::Request::Signing(domain::Signing::Restored));
    accounts.request(domain::Request::Component(domain::ToComponent::Want { account: SOURCE }));
    let heard = pump_accounts(&mut accounts, engine);
    let mut granted = false;
    for event in &heard {
        if let domain::FromComponent::Granted { account: SOURCE, form, .. } = event {
            assert_eq!(*form, domain::Form::Credential(domain::Scheme::Bearer));
            granted = true;
        }
    }
    assert!(granted, "the sign-in announces a bearer grant: {heard:?}");
    let value =
        engine.values.accounts.value(SOURCE, saved.generation, Time::from_nanos(1)).expect("restored sign-in lent");
    let table::Scheme::Bearer = &value.scheme else { panic!("the sign-in lends a bearer") };
    assert_eq!(value.token.expose(), saved.access_token.as_ref());
}

fn pump_move_events(accounts: &mut Accounts, engine: &mut crate::engine::Engine) -> Queue<root::Event> {
    let mut events = Queue::with_capacity(64);
    for _ in 0_u32..32 {
        while let Some(event) = engine.store.above.pop() {
            engine.store.sources.push(event);
        }
        accounts.iterate(Time::from_nanos(1), Wall::from_nanos(1), &mut engine.values, &mut events, &mut engine.store);
    }
    events
}

fn moved_answer(engine: &mut crate::engine::Engine, bearer: &[u8], attempt: u8) -> Secret {
    use crate::tests::fixtures::http as peer;
    use jig_client_protocol_wire::v19 as w;
    use skein_lib::{Reader, Writer};
    let limits = engine.clients.env.limits.wire;
    let change = w::AccountChange::MoveOut(
        w::AccountChangeMoveOut::new(&limits, w::AccountChangeMoveOutParts { account: 19 }).unwrap(),
    );
    let ask = w::Ask::AccountChange(
        w::AskAccountChange::new(&limits, w::AskAccountChangeParts { holder: w::AccountHolder::Own, change }).unwrap(),
    );
    let mut bytes = Writer::new(usize::try_from(ask.measure()).unwrap());
    ask.encode(&mut bytes).unwrap();
    let keyed = w::Keyed::new(
        &limits,
        w::KeyedParts { key: [attempt; 16], owner: w::Owner::Jig, version: 19, request: bytes.finish(), value: None },
    )
    .unwrap();
    let mut bytes = Writer::new(usize::try_from(keyed.measure()).unwrap());
    keyed.encode(&mut bytes).unwrap();
    let mut call = peer::Peer::new(engine, b"POST", b"/jig/v1/request", &bytes.finish(), Some(bearer));
    call.pump(engine, false);
    let Some(root::Event::Party(root::PartyInput::Ask { reply_to, .. })) = engine.events.pop() else {
        panic!("move-out ask reaches the root")
    };
    engine.clients.down(jig_protocol_engine_clients::Event::Reply {
        to: reply_to,
        reply: jig_domain_people::Reply::Outcome(jig_domain_people::Outcome::AccountMovingOut {
            account: 19,
            number: 23,
            source: 71,
        }),
    });
    call.pump(engine, false);
    let at = bytes::find(call.received(), b"\r\n\r\n").unwrap().checked_add(4).unwrap();
    let w::KeyedAnswer::FinalAnswer(answer) =
        w::KeyedAnswer::decode(&limits, &mut Reader::new(&call.received()[at..])).unwrap()
    else {
        panic!("terminal move-out answer")
    };
    let w::Final::Done(done) = w::Final::decode(&limits, &mut Reader::new(answer.record())).unwrap() else {
        panic!("move-out done")
    };
    let w::Outcome::Made(made) = done.outcome() else { panic!("made") };
    let w::Made::MovingOut(moving) = made.what() else { panic!("moving out") };
    assert_eq!((moving.account(), moving.number()), (19, 23));
    moving.record().clone()
}

#[test]
fn a_move_out_answers_with_its_record_once_and_a_move_in_is_adopted() {
    use crate::tests::fixtures::http as peer;
    use jig_client_protocol_wire::v19 as w;

    let mut service = fixtures::ready();
    let link = service.links().pop().expect("sign-in link");
    let engine = service.engine.as_mut().expect("engine");
    let body = peer::link_body(engine, &link);
    let mut login = peer::Peer::new(engine, b"POST", b"/jig/v1/sign-in/link", &body, None);
    login.pump(engine, true);
    let w::SignInAnswer::SignedIn(answer) = login.sign_in_answer(engine) else { panic!("signed in") };
    let bearer = peer::bearer(answer.token().as_ref().expect("credential").expose());

    let (config, runtime) = registered();
    let mut accounts = host(&config, runtime);
    let document = config.limits.driver.client.document;
    let mut saved = record(71, &document);
    saved.subject = Some(bytes::copy_of(b"owner-subject"));
    saved.expires_at = Wall::from_nanos(Duration::from_secs(3_600).as_nanos());
    let encoded = oauth::encode_record(&saved, &document).expect("record fits");
    let _kept = engine.store.secrets(
        Token::new(123),
        store::Secrets::Replace {
            owner: store::SecretOwner::Source,
            number: 71,
            value: jig_protocol_store::SecretValue::Record(Secret::new(encoded)),
            replaces: None,
        },
    );
    while engine.store.above.pop().is_some() {}
    accounts.request(domain::Request::Signing(domain::Signing::Restore {
        source: 71,
        provider: 3,
        kind: domain::Kind::SignIn,
    }));
    accounts.request(domain::Request::Signing(domain::Signing::Restored));
    pump_accounts(&mut accounts, engine);
    accounts.request(domain::Request::Signing(domain::Signing::Withdraw {
        source: 71,
        display: bytes::copy_of(b"moved account"),
    }));
    let mut events = pump_move_events(&mut accounts, engine);
    let mut withdrawn = false;
    while let Some(event) = events.pop() {
        if let root::Event::Account(domain::Event::Withdrawn { source: 71, readable: true }) = event {
            withdrawn = true;
        }
    }
    assert!(withdrawn, "the driver removal and store read reach the child");
    engine.accounts = Some(accounts);
    let mut file = None;
    for attempt in 0_u8..2 {
        let moved_record = moved_answer(engine, &bearer, attempt);
        if attempt == 0 {
            let moved = jig_protocol_accounts::moved::decode(moved_record.expose(), &document).expect("moved file");
            assert_eq!(moved.application.as_ref(), crate::system::ACCOUNT_APPLICATION);
            assert_eq!(moved.record.refresh_token, saved.refresh_token);
            file = Some(moved_record);
        } else {
            assert!(moved_record.expose().is_empty(), "the second exchange has no file");
        }
    }
    adopt_moved_record(engine, file.expect("first moved record"), &document, saved.refresh_token.as_deref().unwrap());
}

fn adopt_moved_record(
    engine: &mut crate::engine::Engine,
    file: Secret,
    document: &oauth::Limits,
    refresh_token: &[u8],
) {
    let carried = carried(engine, store::Secrets::Carry { kind: store::CarriedKind::Moved, value: file });
    let mut accounts = engine.accounts.take().unwrap();
    accounts.request(domain::Request::Signing(domain::Signing::Open { source: 72, carried }));
    let mut events = pump_move_events(&mut accounts, engine);
    let mut opened = false;
    while let Some(event) = events.pop() {
        if let root::Event::Account(domain::Event::Opened { source: 72, provider: 3, .. }) = event {
            opened = true;
        }
    }
    assert!(opened, "the carried file is read and opened");
    accounts.request(domain::Request::Signing(domain::Signing::Adopt { source: 72, carried }));
    events = pump_move_events(&mut accounts, engine);
    let mut adopted = false;
    while let Some(event) = events.pop() {
        if let root::Event::Account(domain::Event::Adopted { source: 72, kept: true }) = event {
            adopted = true;
        }
    }
    assert!(adopted, "adoption is acknowledged by the store");
    let Some(store::Event::Secret { value: Some(store::SecretValue::Record(value)), .. }) =
        engine.store.secrets(Token::new(123), store::Secrets::Get { owner: store::SecretOwner::Source, number: 72 })
    else {
        panic!("adopted source record")
    };
    let adopted = oauth::decode_record(value.expose(), document).expect("adopted record");
    assert_eq!(adopted.key, 72);
    assert_eq!(adopted.refresh_token.as_deref(), Some(refresh_token));
    let Some(store::Event::Secret { value: None, .. }) = engine
        .store
        .secrets(Token::new(123), store::Secrets::Get { owner: store::SecretOwner::Application, number: carried })
    else {
        panic!("adoption consumes the carried file")
    };
}

#[test]
fn a_finish_reads_the_held_address_and_hands_it_to_the_driver() {
    for routed in [false, true] {
        let mut service = fixtures::started();
        fixtures::restart(&mut service);
        let engine = service.engine.as_mut().expect("engine");
        let (config, runtime) = registered();
        let mut accounts = host(&config, runtime);
        pump_accounts(&mut accounts, engine);
        let carried = held_address(engine);
        accounts.request(domain::Request::Signing(domain::Signing::Redirected { source: 71, carried }));
        accounts.start(&mut engine.store, &mut engine.values);
        let mut events = Queue::with_capacity(64);
        if routed {
            let Some(finishes::Next::Get { exchange, carried }) = accounts.finish_out.pop() else {
                panic!("finish asks for the held value");
            };
            let answer = engine.store.secrets(
                Token::new(FINISH_EXCHANGE | exchange.raw()),
                store::Secrets::Get { owner: store::SecretOwner::Application, number: carried },
            );
            engine.store.sources.push(answer.expect("held value answer"));
            accounts.answers(&mut engine.store, &mut events);
        } else {
            accounts.finish_steps(&mut events, &mut engine.store);
        }
        let Some(finishes::Next::Driver(request)) = accounts.finish_out.pop() else {
            panic!("the held value makes a driver request");
        };
        let driver::Request::Redirected { account, .. } = &request else {
            panic!("the driver receives a redirect");
        };
        assert_eq!(*account, 71);
        accounts.finish_out.push(finishes::Next::Driver(request));
        accounts.finish_steps(&mut events, &mut engine.store);
        assert!(accounts.finish_out.is_empty());
        // No sign-in is waiting for this source: the driver's refusal proves
        // that the request went down, without inspecting or formatting its URI.
        let mut events = pump_move_events(&mut accounts, engine);
        let Some(root::Event::Account(domain::Event::FinishRefused {
            source: 71,
            why: domain::SignInRedirectWhy::NotWaiting,
        })) = events.pop()
        else {
            panic!("a finish without a waiting sign-in is refused");
        };
        assert!(events.is_empty());
        let Some(store::Event::Secret { value: Some(_), .. }) = engine
            .store
            .secrets(Token::new(123), store::Secrets::Get { owner: store::SecretOwner::Application, number: carried })
        else {
            panic!("only the clients component erases the held value");
        };
    }
}

#[test]
fn a_finish_whose_held_address_is_gone_is_refused_not_waiting() {
    let mut service = fixtures::started();
    fixtures::restart(&mut service);
    let engine = service.engine.as_mut().expect("engine");
    let (config, runtime) = registered();
    let mut accounts = host(&config, runtime);
    accounts.request(domain::Request::Signing(domain::Signing::Redirected { source: 71, carried: 9001 }));
    let mut events = pump_move_events(&mut accounts, engine);
    let Some(root::Event::Account(domain::Event::FinishRefused {
        source: 71,
        why: domain::SignInRedirectWhy::NotWaiting,
    })) = events.pop()
    else {
        panic!("a finish whose held value is gone is refused");
    };
    assert!(events.is_empty());
    assert!(!accounts.finishes.pending());
    assert!(accounts.above.is_empty());
}

#[test]
fn the_drivers_taken_and_refusals_reach_the_child() {
    let mut service = fixtures::started();
    fixtures::restart(&mut service);
    let engine = service.engine.as_mut().expect("engine");
    let (config, runtime) = registered();
    let mut accounts = host(&config, runtime);
    pump_accounts(&mut accounts, engine);
    accounts.above.push(driver::Event::Taken { account: 71 });
    let mut events = pump_move_events(&mut accounts, engine);
    let Some(root::Event::Account(domain::Event::Finishing { source: 71 })) = events.pop() else {
        panic!("the driver's taken event reaches the accounts child");
    };
    assert!(events.is_empty());
    for (why, expected) in [
        (driver::Refusal::State, domain::SignInRedirectWhy::State),
        (driver::Refusal::Redirect, domain::SignInRedirectWhy::Address),
        (driver::Refusal::Full { bound: 3 }, domain::SignInRedirectWhy::TooMany),
    ] {
        accounts.above.push(driver::Event::Refused { account: 71, asked: driver::Asked::Redirected, why });
        let mut events = pump_move_events(&mut accounts, engine);
        let Some(root::Event::Account(domain::Event::FinishRefused { source, why })) = events.pop() else {
            panic!("driver refusal reaches the accounts child");
        };
        assert_eq!(source, 71);
        assert_eq!(why, expected);
        assert!(events.is_empty());
    }
}

#[test]
fn aborting_drops_a_finish_in_flight() {
    let mut service = fixtures::started();
    fixtures::restart(&mut service);
    let engine = service.engine.as_mut().expect("engine");
    let (config, runtime) = registered();
    let mut accounts = host(&config, runtime);
    pump_accounts(&mut accounts, engine);
    let carried = held_address(engine);
    accounts.request(domain::Request::Signing(domain::Signing::Redirected { source: 71, carried }));
    accounts.start(&mut engine.store, &mut engine.values);
    let Some(finishes::Next::Get { exchange, carried }) = accounts.finish_out.pop() else {
        panic!("finish waits for its store read");
    };
    let answer = engine.store.secrets(
        Token::new(FINISH_EXCHANGE | exchange.raw()),
        store::Secrets::Get { owner: store::SecretOwner::Application, number: carried },
    );
    assert!(accounts.finishes.pending());
    accounts.abort();
    engine.store.sources.push(answer.expect("late store answer"));
    let mut events = pump_move_events(&mut accounts, engine);
    let Some(root::Event::Account(domain::Event::Component(domain::FromComponent::Closed))) = events.pop() else {
        panic!("the aborted driver closes");
    };
    assert!(events.is_empty(), "the aborted finish has no answer from the driver or child");
    assert!(!accounts.finishes.pending());
    assert!(accounts.finish_out.is_empty());
    assert!(accounts.io.is_empty());
    assert!(accounts.is_empty());
}

#[test]
fn a_restored_sources_other_kind_fails_the_read() {
    const SOURCE: u32 = 7;
    for (kind, value) in [
        (domain::Kind::Key, store::SecretValue::Record(Secret::new(b"record".as_slice().into()))),
        (domain::Kind::SignIn, store::SecretValue::Key(Secret::new(b"key".as_slice().into()))),
    ] {
        let mut service = fixtures::ready();
        let engine = service.engine.as_mut().expect("engine");
        let mut accounts = engine.accounts.take().expect("accounts");
        engine.store.secrets(
            Token::new(123),
            store::Secrets::Keep { owner: store::SecretOwner::Source, number: u64::from(SOURCE), value },
        );
        accounts.dispatch(
            domain::Request::Signing(domain::Signing::Restore { source: SOURCE, provider: 1, kind }),
            &mut engine.store,
            &mut engine.values,
        );
        let mut failed = false;
        while let Some(event) = engine.store.above.pop() {
            if let store::Event::Corrupt { .. } = event {
                failed = true;
            }
        }
        assert!(failed, "the wrong kind follows the failed-read path");
        assert!(accounts.aborting);
    }
}
