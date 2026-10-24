use super::*;
use jig_client_world::harness::{App, Duty};
use skein_lib::{Env, Queue};

const CLIENT: ClientId = ClientId(0);
const CALLBACK_DELAY: Duration = Duration::from_millis(3);
const WALL: Wall = Wall::from_nanos(0);

#[test]
fn an_authenticated_reload_routes_the_restored_person_frame() {
    let limits = fixtures::domain_limits();
    let mut front = Joint::new(fixtures::SEED, limits);
    front.bind(CLIENT, fixtures::PARTY);
    let (mut root, _) = Temper::new(fixtures::SEED);
    settle_root(&mut front, &mut root, &limits, Vec::new());
    assert_eq!(root.person().client().link().state, jig_client::Link::Live);
    let kept = pending_adoption(&mut front, &mut root, &limits);
    front.reload(CLIENT, front.now);
    let (mut restored, mut view) = Temper::new(1102);
    settle_root(&mut front, &mut restored, &limits, kept);
    assert_eq!(restored.person().client().link().state, jig_client::Link::Live);
    assert_eq!(restored.person().client().link().party, Some(fixtures::PARTY));
    let mut patches = Queue::with_capacity(Temper::patch_limit(&view));
    Temper::build(&mut view, &restored, &mut patches);
    assert!(Temper::tree(&view).nodes().iter().any(|node| node.name.as_deref() == Some(b"Person")));
}

fn settle_root(
    front: &mut Joint,
    root: &mut crate::app::Root,
    limits: &client::Limits,
    kept: Vec<skein_lib::RequestRecord<jig_client::Held>>,
) {
    let mut env = Env { now: front.now, wall: WALL, limits: *limits };
    let mut out = Queue::with_capacity(Temper::max_out(limits));
    Temper::step(root, &env, Temper::start(fixtures::project(), kept, 0), &mut out);
    assert_eq!(flush(front, root, &env, &mut out), 1, "startup shares the own Me with the UI");
    for _ in 0..8 {
        let callbacks = front.poll(env.now, env.wall);
        if callbacks.is_empty() {
            return;
        }
        for (who, event) in callbacks {
            assert_eq!(who, CLIENT);
            env.now = env.now.saturating_add(CALLBACK_DELAY);
            front.received(CLIENT, &event);
            Temper::step(root, &env, event, &mut out);
            flush(front, root, &env, &mut out);
        }
    }
    panic!("root/front did not settle");
}

fn flush(front: &mut Joint, root: &mut crate::app::Root, env: &Env<client::Limits>, out: &mut Queue<Request>) -> u32 {
    let mut frames = 0;
    Temper::take(root, env, out);
    while let Some(request) = out.pop() {
        if matches!(
            &request,
            Request::Client {
                request: jig_client::Request::Down {
                    down: jig_client::Down::Open {
                        subject: jig_client::Watchable::Jig(jig_client::contract::Subject::Me),
                        ..
                    }
                }
            }
        ) {
            frames += 1;
        }
        if let Duty::Wire(request) = Temper::duty(request) {
            front.wire(CLIENT, env.now, env.wall, request);
        }
    }
    frames
}

fn pending_adoption(
    front: &mut Joint,
    root: &mut crate::app::Root,
    limits: &client::Limits,
) -> Vec<skein_lib::RequestRecord<jig_client::Held>> {
    use client::forge::{Act, Text, Typed};
    let env = Env { now: front.now, wall: WALL, limits: *limits };
    let mut out = Queue::with_capacity(Temper::max_out(limits));
    Temper::step(
        root,
        &env,
        RootEvent::Went { place: client::Place::Address(client::Address::Adopt { project: 1 }) }.into(),
        &mut out,
    );
    for (field, text) in [(Text::Owner, b"acme".as_slice()), (Text::Name, b"shop"), (Text::Landing, b"main")] {
        Temper::step(
            root,
            &env,
            RootEvent::Act {
                action: client::Action::Forge(Act::Write { field, text: text.into(), typed: Typed::Bytes }),
            }
            .into(),
            &mut out,
        );
    }
    front.fault(env.now, FrontFault::NoAnswer);
    Temper::step(root, &env, RootEvent::Act { action: client::Action::Forge(Act::Adopt) }.into(), &mut out);
    let mut kept = Vec::new();
    while let Some(request) = out.pop() {
        match Temper::duty(request) {
            Duty::Store(jig_client_world::harness::StoreWrite::Save(record)) => kept.push(record),
            Duty::Wire(request) => front.wire(CLIENT, env.now, env.wall, request),
            Duty::Store(_) | Duty::Show { .. } | Duty::SignIn { .. } | Duty::SignOut | Duty::Fact => {}
        }
    }
    assert_eq!(kept.len(), 1);
    kept
}

#[test]
fn a_real_sign_out_records_the_engine_receipt_before_the_callback() {
    let mut front = Joint::new(fixtures::SEED, fixtures::domain_limits());
    front.bind(CLIENT, fixtures::PARTY);
    front.poll(Time::ZERO, WALL);
    front.sign_out(CLIENT, Time::ZERO, WALL);
    let callbacks = front.poll(Time::ZERO, WALL);
    assert!(callbacks.iter().any(|(who, event)| *who == CLIENT
        && matches!(
            event.root(),
            Some(RootEvent::Client { event: jig_client::Event::Up { up: jig_client::Up::SignedOut } })
        )));
    assert!(
        front.observed().into_iter().any(|observation| matches!(
            observation, Observed::SignInEnded { client } if client == CLIENT
        )),
        "the real engine's sign-out answer must precede the callback"
    );
}

#[test]
fn the_referee_reads_live_from_tempers_link_region() {
    let limits = fixtures::domain_limits();
    let mut front = Joint::new(fixtures::SEED, limits);
    front.bind(CLIENT, fixtures::PARTY);
    let (mut root, mut view) = Temper::new(fixtures::SEED);
    settle_root(&mut front, &mut root, &limits, Vec::new());
    let mut patches = Queue::with_capacity(Temper::patch_limit(&view));
    Temper::build(&mut view, &root, &mut patches);
    assert!(Temper::seen(Temper::tree(&view)).contains(&jig_client_world::harness::Seen::Link { live: true }));
}

#[test]
fn an_unconfigured_model_is_answered_immediately() {
    use jig_client::contract as c;
    let limits = fixtures::domain_limits();
    let mut front = Joint::new(fixtures::SEED, limits);
    front.bind(CLIENT, fixtures::PARTY);
    let (mut root, _) = Temper::new(fixtures::SEED);
    settle_root(&mut front, &mut root, &limits, Vec::new());
    let text = |bytes: &[u8]| c::Text::new(bytes.into(), limits.client.domain.text).expect("fixture text");
    let me = client::pages::accounts::current_me(root.person().client()).expect("authenticated me");
    let mut ask = jig_client::shapes::chat(me, 1, text(b"chosen chat")).expect("chat template");
    let c::Ask::Create { tasks, .. } = &mut ask else { panic!("chat creation") };
    tasks.get_mut(0).expect("one chat").llm = Some(c::LlmChoice {
        account: None,
        main: c::ModelEffort { model: text(b"missing"), effort: None },
        subagents: skein_lib::List::with_capacity(0),
    });
    let attempt = Token::new(91);
    front.request(
        CLIENT,
        Request::Client {
            request: jig_client::Request::Down {
                down: jig_client::Down::Send {
                    attempt,
                    key: [31; 16],
                    request: jig_client::Ask::Jig(Box::new(ask)),
                    value: None,
                },
            },
        },
    );
    let [
        (
            who,
            Event::Root(RootEvent::Client {
                event: jig_client::Event::Up { up: jig_client::Up::Answer { attempt: actual, envelope, answer, why } },
            }),
        ),
    ] = front.callbacks.as_slice()
    else {
        panic!("one immediate callback")
    };
    assert_eq!(
        (*who, *actual, *envelope, answer.clone(), *why),
        (
            CLIENT,
            attempt,
            RequestEnvelope::Final,
            Some(jig_client::Answered::Jig(c::Answer::Refused {
                refusal: c::Refusal::Llm { model: text(b"missing"), why: c::LlmWhy::Unconfigured },
                at: None,
            })),
            None
        )
    );
}

// The application route's raw drop reaches temper's own client, not jig's test application.
#[test]
fn qa_a_panel_dropped_before_its_first_record_keeps_loading_until_jig_reopens_it() {
    use client::forge::panel::State;
    use jig_client_test_domain::family::Kind;
    use jig_client_world::harness::{Settings, Until, World};

    let limits = fixtures::domain_limits();
    let mut front = Joint::with_change_landing(fixtures::SEED, &limits);
    front.engine.forge =
        Some(temper_engine_domain_world::forge_stand::ForgeStand::with_checks(fixtures::REPOSITORIES, &[b"build"]));
    fixtures::adopt_change_repository(&mut front);
    front.engine.forge.as_mut().expect("fake forge").push_branch(7, b"temper/qa");
    let task = fixtures::goal_with_pushed_change(&mut front, b"temper/qa");
    let mut world = World::<Temper, Joint>::new(
        Settings {
            seed: fixtures::SEED,
            words: temper_client_view::words::jig_language(),
            heap_bytes_per_client: 64 * 1024 * 1024,
            ..Settings::default()
        },
        front,
    );
    let client = world.client(fixtures::PARTY, fixtures::task_page(task));
    let mut opening = None;
    for _ in 0..100 {
        let outcome = world.run(Until::Steps(2));
        assert!(
            matches!(outcome.verdict, skein_world::domain::Verdict::Passed | skein_world::domain::Verdict::Open { .. }),
            "{outcome:?}"
        );
        if let Some(panel) = world.root(client).person().forge().panel() {
            assert_eq!(panel.state, State::Loading);
            let handle = panel.handle.expect("opening panel handle");
            assert!(
                world
                    .root(client)
                    .person()
                    .client()
                    .document(handle)
                    .expect("retained panel document")
                    .latest
                    .is_none()
            );
            opening = Some(handle);
            break;
        }
    }
    let handle = opening.expect("the task document opens its panel before its first delivery");
    let front = world.front();
    let watcher = front
        .transport
        .watches()
        .into_iter()
        .find(|watcher| {
            let route = front.transport.watch(*watcher).expect("live transport route");
            route.client == client
                && matches!(route.subject, Watched::Application { subject, .. } if subject.kind == Kind::Panel(task))
        })
        .expect("the panel's application route");
    let route = front.transport.end_watch(watcher).expect("dropped panel route");
    front.send(translate::party(jig_domain_core::Event::Unwatch { watcher }));
    world.event(
        client,
        RootEvent::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::WatchEnded { watch: route.native, end: jig_client::contract::WatchEnd::Dropped },
            },
        }
        .into(),
    );
    let panel = world.root(client).person().forge().panel().expect("task page panel");
    assert_eq!(panel.handle, Some(handle));
    assert_eq!(panel.state, State::Loading);
    let document = world.root(client).person().client().document(handle).expect("retained panel document");
    assert_eq!(document.state, jig_client::State::Behind);
    assert!(document.latest.is_none());
    for _ in 0..100 {
        let until = world.now().saturating_add(Duration::from_millis(100));
        let outcome = world.run(Until::Time(until));
        assert!(matches!(outcome.verdict, skein_world::domain::Verdict::Passed), "{outcome:?}");
        let panel = world.root(client).person().forge().panel().expect("task page panel");
        assert_eq!(panel.handle, Some(handle));
        if let State::Shown(record) = &panel.state {
            assert_eq!(record.task(), task);
            assert!(world.front().watching_change(client, task));
            return;
        }
    }
    panic!("jig's retry must deliver the known change panel");
}
