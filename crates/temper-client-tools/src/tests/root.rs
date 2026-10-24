use crate::tests::fixtures;
use jig_client::{self as client, contract as c};

// Queue the last delivery and its terminal before routing either notification,
// as the native client can do when it drains several wire events together.
fn retained_delivery(root: &mut fixtures::Harness, subject: client::Watchable, delivery: c::Delivery) {
    let watch = root.watch_token(&subject);
    let mut pending = skein_lib::Queue::with_capacity(fixtures::OUTPUTS);
    for up in [
        client::Up::Watched { watch, event: c::WatchEvent::Delivered(delivery) },
        client::Up::WatchEnded { watch, end: c::WatchEnd::SubjectEnded },
    ] {
        root.root.client.step(root.now, root.wall, client::Event::Up { up }, &mut pending);
    }
    root.watches.remove(&watch).expect("engine ended watch");
    let env = skein_lib::Env {
        now: root.now,
        wall: root.wall,
        limits: super::Limits { domain: root.limits.domain, tools: root.limits.tools },
    };
    let mut child = skein_lib::Queue::with_capacity(fixtures::OUTPUTS);
    let mut out = skein_lib::Queue::with_capacity(fixtures::OUTPUTS);
    super::drain(&mut root.root, &env, &mut pending, &mut child, &mut out);
    root.outputs(out);
}

#[test]
fn an_ended_tree_delivery_waits_for_the_tasks_read() {
    for json in [false, true] {
        let argv = fixtures::extended(&["task", "41", "--tree"], if json { &["--json"] } else { &[] });
        let (mut root, call) = fixtures::read_call(argv.as_slice());
        let task = fixtures::ended_task(c::Phase::Done);
        let client::Latest::Jig(delivery) = fixtures::owner_tree_at(task.chip.clone()) else {
            panic!("owner tree")
        };
        retained_delivery(&mut root, fixtures::tree_watch(), delivery);
        assert!(root.answer(call).is_none(), "task read is still pending");
        root.serve(call, fixtures::historical_task_result(task, c::Ending::Done));
        let answer = root.answer(call).expect("retained tree and historical task complete the read");
        assert_eq!(answer.status, 0);
        assert!(answer.stderr.is_empty());
        assert!(fixtures::has(&answer, if json { b"\"tree\":" } else { b"tree:\n" }));
        assert_eq!(root.watches.len(), fixtures::SHARED_WATCHES);
    }
}

#[test]
fn a_filled_tree_slot_survives_its_close_while_the_task_read_finishes() {
    let (mut root, call) = fixtures::read_call(&["task", "41", "--tree"]);
    root.deliver_subject(&fixtures::tree_watch(), c::Delivery::Tree(Box::new(fixtures::composite_tree())));
    assert!(root.answer(call).is_none(), "task read is still pending after the tree closes");
    root.serve(call, fixtures::task_result(fixtures::goal(fixtures::TASK, fixtures::waiting_on_me())));
    assert_eq!(root.answer(call).expect("both slots filled").status, 0);
}

#[test]
fn a_wait_checks_its_first_task_document_even_when_the_watch_has_ended() {
    let (mut root, call) = fixtures::waiting_call(fixtures::WAIT_TASK, fixtures::UNTIL_REPLIED);
    retained_delivery(
        &mut root,
        fixtures::task_watch(),
        c::Delivery::Task(Box::new(fixtures::ended_task(c::Phase::Done))),
    );
    let answer = root.answer(call).expect("first retained document already meets replied");
    assert_eq!(answer.status, 0);
    assert!(answer.stdout.ends_with(b",\"met\":true}"));
    assert!(root.reads.is_empty(), "the first document settled the condition");
    assert_eq!(crate::root::next_deadline(&root.root), None);
}
#[test]
fn two_calls_in_flight_are_answered_each_under_its_own_number() {
    let mut root = fixtures::signed_in(fixtures::PARTY);
    let a = root.submit(fixtures::TASK_CALL);
    let b = root.submit(fixtures::OTHER_TASK_CALL);
    root.serve(b, fixtures::task_result(fixtures::goal(fixtures::OTHER_TASK, fixtures::waiting_on_me())));
    assert!(root.answer(a).is_none());
    assert_eq!(root.answer(b).expect("second answered").status, 0);
    root.serve(a, fixtures::task_result(fixtures::goal(fixtures::TASK, fixtures::waiting_on_me())));
    assert_eq!(root.answer(a).expect("first answered").status, 0);
}
#[test]
fn an_unknown_asker_tells_no_child() {
    for (asker, facts) in [(fixtures::UNKNOWN_ASKER, 1), (temper_client_domain::owner::Owner::Forge.asker(), 0)] {
        let mut root = fixtures::signed_in(fixtures::PARTY);
        root.client(client::Event::Watch {
            handle: client::Handle { asker, number: 1 },
            subject: client::Watchable::Jig(c::Subject::Me),
        });
        assert_eq!(root.facts, facts, "{asker:?}");
    }
}
#[test]
fn new_calls_start_above_all_kept_call_numbers() {
    let mut root = fixtures::started(fixtures::kept(fixtures::RESTORED_PREFIX));
    assert_eq!(root.root.next, Some(fixtures::RESTORED_PREFIX + 1));
    root.deliver(c::Delivery::Me(Box::new(fixtures::me(fixtures::PARTY))));
    let call = root.submit(fixtures::TASK_CALL);
    root.serve(call, fixtures::task_result(fixtures::goal(fixtures::TASK, fixtures::waiting_on_me())));
    assert_eq!(root.answer(call).expect("new read answered").status, 0);
}
#[test]
fn call_number_exhaustion_exits_1_without_sending() {
    let mut root = fixtures::started(fixtures::kept(u32::MAX));
    root.deliver(c::Delivery::Me(Box::new(fixtures::me(fixtures::PARTY))));
    let call = root.submit(fixtures::TASK_CALL);
    let answer = root.answer(call).expect("exhaustion answered locally");
    assert_eq!((answer.status, answer.stderr.as_ref()), (1, b"call number exhausted\n".as_slice()));
    assert_eq!(root.reads.len(), 0);
}

#[test]
fn every_call_handle_uses_its_prefix_under_one_owner() {
    use crate::jig::runner;
    let mut root = fixtures::signed_in(fixtures::PARTY);
    for prefix in [1, fixtures::RESTORED_PREFIX] {
        let mut out = skein_lib::Queue::with_capacity(fixtures::OUTPUTS);
        let env =
            skein_lib::Env { now: skein_lib::Time::ZERO, wall: skein_lib::Wall::EPOCH, limits: root.limits.tools };
        runner::call(
            &mut root.root.runner,
            &root.root.client,
            &env,
            runner::Called {
                number: fixtures::CORRELATION,
                call_number: prefix,
                call: fixtures::call(fixtures::TASK_CALL),
                json: false,
                value: None,
            },
            &mut out,
        );
        let runner::Out::Client { event: client::Event::Read { handle, .. } } = out.pop().expect("read once") else {
            panic!("read event")
        };
        assert_eq!(handle, client::Handle { asker: super::RUNNER.asker(), number: (u64::from(prefix) << 32u32) | 1 });
    }
}
