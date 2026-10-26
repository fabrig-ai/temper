use super::fixtures;
use skein_lib::{Duration, Time};
use temper_engine_domain as root;

#[test]
fn the_root_reports_the_earliest_credentials_expiry() {
    let (mut domain, mut env) = fixtures::root();
    env.now = Time::from_nanos(5_000_000_000);
    assert_eq!(domain.next_deadline(&env), None);
    for (number, valid, expected) in [(1, 7, 12_u64), (2, 2, 7)] {
        root::try_step(
            &mut domain,
            &env,
            root::Event::Credential { number, generation: 1, valid: Duration::from_secs(valid) },
        )
        .expect("credential admitted");
        assert_eq!(domain.next_deadline(&env), Some(Time::from_nanos(expected.checked_mul(1_000_000_000).unwrap())));
    }
}

#[test]
fn the_forge_reports_the_earliest_procedure_or_projection_wait() {
    let own = crate::limits::profile(crate::limits::Profile::Small);
    let mut domain = temper_engine_domain_forge::Domain::new(
        &own.forge,
        7,
        skein_forge_client::Config {
            namespace: Box::new([7; 16]),
            writers: Box::new([skein_forge_client::Writer { forge: 1, author: 42 }]),
        },
        &[temper_engine_domain_forge::GitIdentity { forge: 1, credential: 1 }],
        &[],
        false,
    )
    .unwrap();
    let env = skein_lib::Env { now: Time::ZERO, wall: skein_lib::Wall::EPOCH, limits: own.forge };
    assert_eq!(domain.next_wall_deadline(), None);
    domain.wait_change(&env, 1, Some(skein_lib::Wall::from_nanos(7)));
    assert_eq!(domain.next_wall_deadline(), Some(skein_lib::Wall::from_nanos(7)));
    domain.projection_after(2, skein_lib::Wall::from_nanos(3));
    assert_eq!(domain.next_wall_deadline(), Some(skein_lib::Wall::from_nanos(3)));
    domain.projection_settled(2);
    assert_eq!(domain.next_wall_deadline(), Some(skein_lib::Wall::from_nanos(7)));
    domain.offered_change(1);
    assert_eq!(domain.next_wall_deadline(), None);
}

#[test]
fn the_restart_asks_one_load_at_a_time() {
    let mut service = fixtures::started();
    for _ in 0_u32..16 {
        fixtures::turn(&mut service);
        if store_answered(&service) {
            break;
        }
    }
    let event = service.engine.as_mut().expect("engine").store.above.pop();
    let Some(jig_protocol_store::Event::Restored { owner, rows, next }) = event else {
        panic!("first restart load: {event:?}")
    };
    assert!(rows.is_empty());
    assert_eq!(next, None);
    assert!(service.engine.as_ref().expect("engine").store.above.is_empty());
    fixtures::turn(&mut service);
    assert!(service.engine.as_ref().expect("engine").store.above.is_empty(), "no next load before the first answer");
    service.engine.as_mut().expect("engine").store.above.push(jig_protocol_store::Event::Restored {
        owner,
        rows,
        next,
    });
    for _ in 0_u32..16 {
        fixtures::turn(&mut service);
        if !service.engine.as_mut().expect("engine").store.above.is_empty() {
            break;
        }
    }
    assert_eq!(service.end(), None);
    assert_eq!(service.engine.as_ref().expect("engine").store.above.len(), 1, "one next store operation");
    fixtures::restart(&mut service);
}
/// Whether the store has answered the engine. The accounts' answers, as
/// their handed-in record kept (jig's protocol/store.md, 9), are taken on
/// the next turn.
fn store_answered(service: &crate::Service) -> bool {
    for event in &service.engine.as_ref().expect("engine").store.above {
        let jig_protocol_store::Event::Kept { owner } = event else { return true };
        if !crate::accounts::owns(*owner) {
            return true;
        }
    }
    false
}
#[test]
fn a_store_answer_is_taken_with_the_journal_full() {
    let mut service = fixtures::ready();
    let engine = service.engine.as_mut().expect("engine");
    let mut number = 2;
    for _ in 0_u32..engine.env.limits.journal.commits.checked_add(1).unwrap() {
        if root::try_step(&mut engine.domain, &engine.env, fixtures::period(number)).is_err() {
            break;
        }
        number = number.checked_add(1).unwrap();
    }
    assert!(root::try_step(&mut engine.domain, &engine.env, fixtures::period(number)).is_err(), "journal is full");
    root::release(&mut engine.domain, &engine.env, &mut engine.requests);
    let Some(root::Request::Store(root::StoreRequest::Commit { number, writes })) = engine.requests.pop() else {
        panic!("commit")
    };
    assert!(engine.store.down(root::StoreRequest::Commit { number, writes }));
    crate::engine::iterate(engine, Time::from_nanos(1), skein_lib::Wall::from_nanos(1));
    for event in &engine.store.above {
        if let jig_protocol_store::Event::Committed { number: found, .. } = event {
            assert_ne!(*found, number, "original terminal taken while admission was blocked");
        }
    }
    assert!(root::try_step(&mut engine.domain, &engine.env, fixtures::period(20)).is_ok(), "answer freed journal room");
}
#[test]
fn a_failed_commit_releases_nothing_after_it() {
    let mut service = fixtures::ready();
    let engine = service.engine.as_mut().expect("engine");
    let first = engine.domain.deployment().commits.checked_add(1).unwrap();
    root::try_step(&mut engine.domain, &engine.env, fixtures::period(2)).unwrap();
    root::try_step(&mut engine.domain, &engine.env, fixtures::period(3)).unwrap();
    engine.store.above.push(jig_protocol_store::Event::Failed { number: first });
    engine.store.above.push(jig_protocol_store::Event::Committed { number: first.checked_add(1).unwrap(), fill: 0 });
    fixtures::turn(&mut service);
    assert_eq!(service.end(), Some(&crate::End::Stopped(crate::Stop::CommitFailed { number: first })));
    assert!(service.engine.as_ref().expect("engine").requests.is_empty(), "later outputs stayed behind the barrier");
    assert!(service.engine.as_ref().expect("engine").store.above.is_empty());
}

#[test]
fn a_frame_refusal_stops_with_its_details_and_releases_no_later_output() {
    let mut service = fixtures::ready();
    let engine = service.engine.as_mut().expect("engine");
    let number = engine.domain.deployment().commits.checked_add(1).unwrap();
    root::try_step(&mut engine.domain, &engine.env, fixtures::period(2)).unwrap();
    root::try_step(&mut engine.domain, &engine.env, fixtures::period(3)).unwrap();
    engine.store.above.push(jig_protocol_store::Event::FrameTooLarge { number, task: 11, bytes: 79, limit: 78 });
    engine.store.above.push(jig_protocol_store::Event::Committed { number: number.checked_add(1).unwrap(), fill: 0 });
    fixtures::turn(&mut service);
    assert_eq!(
        service.end(),
        Some(&crate::End::Stopped(crate::Stop::FrameTooLarge { number, task: 11, bytes: 79, limit: 78 }))
    );
    let engine = service.engine.as_ref().expect("engine");
    assert!(engine.requests.is_empty(), "later outputs stayed behind the existing Uncommitted barrier");
    assert!(engine.store.above.is_empty());
}

#[test]
fn an_engine_that_cannot_fit_its_memory_is_refused_before_start() {
    let mut settings = fixtures::engine();
    settings.jig.memory = 1;
    let Err(problems) = crate::Service::engine(settings, &fixtures::seeds(), None, fixtures::forge_runtime()) else {
        panic!("memory refused")
    };
    let problem = problems.problems.get(0).unwrap();
    assert_eq!(problem.path, jig_protocol_config::Path::root().member(b"engine").member(b"memory_bytes"));
}
#[test]
fn kernel_operations_are_routed_back_to_their_owner() {
    use skein_io::kernel::{Complete, Done, Error, Fd, Op, Submit};
    use skein_lib::{Queue, Token};
    let mut kernel = crate::accounts::Kernel::new(4);
    let mut sockets = Queue::with_capacity(2);
    sockets.push(Submit { op: Token::new(1), kind: Op::Close { fd: Fd::new(6) } });
    let mut output = Queue::with_capacity(4);
    kernel.publish(&mut sockets, &mut output);
    let request = output.pop().expect("socket operation");
    sockets.push(Submit { op: Token::new(2), kind: Op::Cancel { target: Token::new(1) } });
    let complete =
        kernel.complete(Complete { op: request.op, kind: request.kind, result: Ok(Done::Nothing) }, &sockets);
    assert_eq!(complete.op, Token::new(1));
    kernel.publish(&mut sockets, &mut output);
    let cancel = output.pop().expect("cancel after target completion");
    assert_eq!(cancel.kind, Op::Cancel { target: request.op });
    let complete =
        kernel.complete(Complete { op: cancel.op, kind: cancel.kind, result: Err(Error::TooLate) }, &sockets);
    assert_eq!(complete.op, Token::new(2));
    assert_eq!(complete.kind, Op::Cancel { target: Token::new(1) });
}
#[test]
fn an_io_listener_failure_stops_the_engine() {
    let mut service = fixtures::started();
    service.told.push(skein_io::Event::Closed { owner: skein_lib::Token::new(crate::tokens::WORKER_LISTENERS) });
    fixtures::turn(&mut service);
    assert_eq!(service.end(), Some(&crate::End::Stopped(crate::Stop::Listener)));
}

// Both note scopes and their store replies must fit before the held input is retried.
const OWNER_PASSES: u32 = 16;
const WATCHER: skein_lib::Token = skein_lib::Token::new(17);
// A notes page leaves its Core::Notes::Loaded continuation for the next Resume.
fn a_notes_page_waiting_for_its_owner() -> crate::Service {
    use skein_lib::{Queue, ReplyTo, Token, Wall};
    let mut service = fixtures::signed_worker();
    service.engine.as_mut().expect("engine").step(root::Event::Party(root::PartyInput::Watch {
        watcher: WATCHER,
        sign_in: 1,
        subject: jig_domain_core::documents::Watched::Frame,
        window: jig_domain_core::documents::Window { rows: 1, after: None },
    }));
    fixtures::settle_root(&mut service, &mut Queue::with_capacity(32));
    service.engine.as_mut().expect("engine").step(root::Event::Party(root::PartyInput::Delivered { watcher: WATCHER }));
    fixtures::settle_root(&mut service, &mut Queue::with_capacity(32));
    service.engine.as_mut().expect("engine").step(root::Event::Party(root::PartyInput::Ask {
        models: Box::new([]),
        reply_to: ReplyTo::new(Token::new(101)),
        sign_in: 1,
        key: [31; 16],
        ask: fixtures::chat(1, b"question"),
    }));
    // Startup is settled; the created task's preparation now reads its notes.
    for _ in 0..OWNER_PASSES {
        crate::engine::iterate(service.engine.as_mut().expect("engine"), Time::from_nanos(1), Wall::from_nanos(1));
        if let Some(jig_protocol_store::Event::Loaded { .. }) =
            service.engine.as_mut().expect("engine").store.above.iter().next()
        {
            let event = service.engine.as_mut().expect("engine").store.above.pop().expect("notes page");
            let engine = service.engine.as_mut().expect("engine");
            crate::store::route::up(&mut engine.store.routes, event, &mut engine.store.routed);
            let Some(crate::store::route::Up::Root(event)) =
                service.engine.as_mut().expect("engine").store.routed.pop()
            else {
                panic!("notes page routed to the root")
            };
            service.engine.as_mut().expect("engine").step(*event);
            return service;
        }
    }
    panic!("task preparation asks for notes");
}

#[test]
fn an_unwatch_refused_behind_owner_work_is_admitted() {
    use skein_lib::Wall;
    let mut service = a_notes_page_waiting_for_its_owner();
    let engine = service.engine.as_mut().expect("engine");
    engine.step(root::Event::Party(root::PartyInput::Unwatch { watcher: WATCHER }));
    let refused = engine.blocked && engine.pending.is_some();
    for _ in 0..OWNER_PASSES {
        crate::engine::iterate(engine, Time::from_nanos(1), Wall::from_nanos(1));
    }
    assert_eq!((refused, engine.pending.is_none(), engine.blocked), (true, true, false));
}

// The service's existing restart fixture drains within 256 iterations.
const STARTUP_PASSES: u32 = 256;
#[test]
fn nothing_leaves_before_the_mark() {
    let previous = fixtures::ready().engine.expect("previous engine");
    let reopened = fixtures::reopened_store(previous.store, &previous.env.limits);
    let mut service = fixtures::start_with_store(&[(1, 100)], Some(reopened));
    service.engine.as_mut().expect("engine").store.hold_mark = true;
    for _ in 0..STARTUP_PASSES { fixtures::turn(&mut service); }
    let engine = service.engine.as_mut().expect("engine");
    assert!(engine.store.held_mark.is_some(), "public restore reaches its mark without secret bootstrap");
    assert!(!engine.store.component.mark_durable());
    assert_eq!(engine.accounts.as_ref().expect("accounts").startup_state().0, false);
    assert!(engine.accounts.as_ref().expect("accounts").startup_state().1 > 0, "early account requests retained");
    assert_eq!(engine.store.secret_requests, 0);
    assert!(engine.store.forge_loads.is_empty());
    assert!(!engine.forge_records.done());
    assert!(engine.forge.starts.is_empty());
    assert!(!engine.forge.startup_begun());
    assert!(!engine.setup.startup_restored());
    let mark = engine.store.held_mark.take().expect("held mark");
    engine.store.hold_mark = false;
    assert!(engine.store.down(mark));
    fixtures::restart(&mut service);
    let engine = service.engine.as_ref().expect("engine");
    assert!(engine.store.component.mark_durable());
    assert_eq!(engine.accounts.as_ref().expect("accounts").startup_state(), (true, 0));
    assert_eq!(engine.store.forge_loads.as_slice(), [None], "one application-secrets pass");
    assert!(engine.forge_records.done());
    assert!(engine.forge.startup_begun());
    assert!(engine.setup.startup_restored());
    let secrets = engine.store.secret_requests;
    for _ in 0..STARTUP_PASSES { fixtures::turn(&mut service); }
    let engine = service.engine.as_ref().expect("engine");
    assert_eq!(engine.accounts.as_ref().expect("accounts").startup_state(), (true, 0));
    assert_eq!(engine.store.secret_requests, secrets, "bootstrap is not repeated");
    assert_eq!(engine.store.forge_loads.as_slice(), [None]);
}
