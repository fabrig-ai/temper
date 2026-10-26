use super::*;
use crate::tests::fixtures;
use temper_client_protocol_wire::{self as wire, v1 as w};

const DEMO_SOURCE: u64 = 1;

fn enqueue(clients: &mut Clients) {
    let requesting = w::Requesting::new(&w::CEILINGS, w::RequestingParts { request: w::Request::StartDemo }).unwrap();
    clients.out.push(kit::Request::Application(kit::ToApplication::Request {
        reply_to: skein_lib::ReplyTo::new(Token::new(91)),
        sign_in: 1,
        key: [9; 16],
        digest: [7; 32],
        scope: people::ApplicationScope::Deployment,
        carried: None,
        family: wire::FORGE,
        version: wire::CLIENT_VERSION,
        record: wire::requesting(&requesting),
    }));
}

#[test]
fn start_demo_reads_each_time_and_carries_the_key_before_admission() {
    let mut service = fixtures::demo_service();
    let engine = service.engine.as_mut().unwrap();
    engine.clients.starting = None;
    enqueue(&mut engine.clients);
    engine.clients.advance(&mut engine.events, &engine.workers.forges, engine.demo.as_ref(), &mut engine.accounts);
    assert!(engine.events.is_empty());
    let read = service.take_handover_read().unwrap();
    assert!(read.path.ends_with(b"demo/llm.key"));
    assert_eq!(read.max, 4097);
    // The shell removes exactly one LF. This remaining LF belongs to the key.
    service.handover_read(read.forge, read.run, Ok(Some(skein_lib::Secret::new(b"test-key\n".as_slice().into()))));
    let engine = service.engine.as_mut().unwrap();
    let mut store = fixtures::memory_store();
    engine.clients.secrets_step(&mut store);
    store.iterate();
    while let Some(answer) = store.above.pop() {
        engine.clients.secret_answer(answer);
    }
    engine.clients.advance(&mut engine.events, &engine.workers.forges, engine.demo.as_ref(), &mut engine.accounts);
    let Some(root::Event::Party(root::PartyInput::Application {
        request: root::Application::Forge(temper_engine_domain_forge::Ask::StartDemo),
        carried: Some(carried),
        ..
    })) = engine.events.pop()
    else {
        panic!("carried start")
    };
    assert_ne!(carried.number, 0);
    assert_eq!(carried.digest, skein_lib::sha256::digest(b"test-key\n"));
    // The playground adds its key through accounts, whose store adoption
    // accepts a carried Key alone (jig's protocol/store.md, 9).
    let _later = store.secrets(
        Token::new(DEMO_SOURCE),
        jig_protocol_store::Secrets::Adopt {
            carried: carried.number,
            owner: jig_protocol_store::SecretOwner::Source,
            number: DEMO_SOURCE,
            replaces: None,
        },
    );
    store.iterate();
    let Some(jig_protocol_store::Event::Secret { value: Some(jig_protocol_store::SecretValue::Key(value)), .. }) =
        store.secrets(
            Token::new(DEMO_SOURCE),
            jig_protocol_store::Secrets::Get { owner: jig_protocol_store::SecretOwner::Source, number: DEMO_SOURCE },
        )
    else {
        panic!("the demo key is adoptable as a source")
    };
    assert_eq!(value.expose(), b"test-key\n");
    enqueue(&mut engine.clients);
    engine.clients.advance(&mut engine.events, &engine.workers.forges, engine.demo.as_ref(), &mut engine.accounts);
    let next = engine.clients.demo_read().unwrap();
    assert_eq!(next.path, read.path);
    assert_ne!(next.run, read.run);
}

#[test]
fn unreadable_empty_and_oversize_demo_keys_carry_nothing() {
    for answer in [
        Err(crate::HandoverUnread::Kernel(2)),
        Ok(None),
        Ok(Some(skein_lib::Secret::new(Box::new([])))),
        Ok(Some(skein_lib::Secret::new(Box::new([b'x'; 4097])))),
    ] {
        let mut service = fixtures::demo_service();
        let engine = service.engine.as_mut().unwrap();
        engine.clients.starting = None;
        enqueue(&mut engine.clients);
        engine.clients.advance(&mut engine.events, &engine.workers.forges, engine.demo.as_ref(), &mut engine.accounts);
        let read = service.take_handover_read().unwrap();
        service.handover_read(read.forge, read.run, answer);
        let engine = service.engine.as_mut().unwrap();
        engine.clients.advance(&mut engine.events, &engine.workers.forges, engine.demo.as_ref(), &mut engine.accounts);
        let Some(root::Event::Party(root::PartyInput::Application {
            request: root::Application::Forge(temper_engine_domain_forge::Ask::StartDemo),
            carried: None,
            ..
        })) = engine.events.pop()
        else {
            panic!("start without a carried value")
        };
        assert!(engine.clients.demo_carries.is_empty());
    }
}

#[test]
fn parked_demo_starts_are_bounded() {
    let mut service = fixtures::demo_service();
    let engine = service.engine.as_mut().unwrap();
    for _ in 0..=DEMO_STARTS {
        enqueue(&mut engine.clients);
        engine.clients.advance(&mut engine.events, &engine.workers.forges, engine.demo.as_ref(), &mut engine.accounts);
    }
    assert_eq!(engine.clients.demo_starts.len(), DEMO_STARTS);
    assert_eq!(engine.clients.demo_reads.len(), DEMO_STARTS);
    assert_eq!(engine.clients.out.len(), 1);
}
