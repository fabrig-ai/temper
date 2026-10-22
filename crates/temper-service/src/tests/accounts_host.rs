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
        assert!(accounts.ids.contains_key(&old), "only Removed frees the driver's place");
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

fn carry_llm_key(engine: &mut crate::engine::Engine, value: &[u8]) -> u64 {
    let _later = engine.store.secrets(Token::new(123), store::Secrets::Carry { value: Secret::new(value.into()) });
    let Some(store::Event::Carried { number, .. }) = engine.store.above.pop() else { panic!("carried LLM key") };
    number
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
            value: Secret::new(b"orphaned-source-value".as_slice().into()),
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
    Accounts::new(config, &fixtures::seeds(), Some(runtime), 64, &config.limits.store.secrets)
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
    assert!(accounts.ids.contains_key(&9001));
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
    assert_eq!(restored.ids.get(&9001).expect("restored source").expose(), b"provider-subject");
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
    assert!(Accounts::new(&config, &fixtures::seeds(), None, 64, &config.limits.store.secrets).is_err());
}
#[test]
fn overflowing_account_bounds_are_refused_before_start() {
    const LAST_ACCOUNT_COUNT: u32 = 536_848_040;
    let (config, _) = registered();
    let mut overflow = config.limits.driver;
    overflow.accounts = u32::MAX;
    overflow.client.document.record_bytes = u32::MAX;
    // Under the record ceiling and 64 queue slots this is the final account count
    // whose driver records, tree nodes and queued payloads fit in u64.
    overflow.accounts = LAST_ACCOUNT_COUNT;
    assert!(worst_case(&overflow, 64, config.limits.store.secrets.page.rows).is_some());
    overflow.accounts = LAST_ACCOUNT_COUNT + 1;
    assert_eq!(worst_case(&overflow, 64, config.limits.store.secrets.page.rows), None);
    overflow.accounts = u32::MAX;
    assert_eq!(worst_case(&overflow, u32::MAX, u32::MAX), None);
}
