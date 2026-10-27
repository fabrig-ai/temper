use super::*;
use crate::tests::fixtures as common;
mod fixtures;
#[path = "../../tests/support/mod.rs"]
mod machine;
const CURRENT_PARTY: u64 = 9;
const PRINTED: skein_lib::Duration = skein_lib::Duration::from_millis(30);
const STILL_PRINTED: skein_lib::Duration = skein_lib::Duration::from_secs(1);
#[test]
fn held_startup_keeps_the_first_quiet_chat_activity_answer() {
    let mut host = fixtures::loaded_chat();
    let (exchange, credential) = fixtures::activity_read(&mut host);
    let message = fixtures::activity_answer(true);
    host.transport(jig_client::Event::Reply { exchange, message: message.clone() });
    let chat = host.root.pages().chat().expect("chat remains open");
    assert_eq!(chat.thread.lines.rows.len(), 1, "the first activity answer reaches the actual page");
    assert_eq!(credential, Some(fixtures::credential()), "first read uses the installed held credential");
    assert!(matches!(
        chat.thread.lines.rows.as_slice()[0].event,
        jig_client::contract::Activity::RunBegan { attempt: fixtures::RUN, .. }
    ));
    assert!(
        matches!(chat.thread.lines().as_slice(), [jig_client::thread::Line::RunBegan { attempt: fixtures::RUN }]),
        "the view receives the intended line"
    );
    assert!(chat.thread.reading.is_none(), "the page consumed its answer");
    host.transport(jig_client::Event::Reply { exchange, message });
    host.transport(jig_client::Event::Lost { exchange });
    assert_eq!(
        host.root.pages().chat().expect("chat").thread.lines.rows.len(),
        1,
        "late terminals do not duplicate it"
    );
    assert!(host.out.is_empty(), "a quiet page needs no later activity or person act");
}

#[test]
fn held_startup_keeps_the_first_empty_chat_activity_answer() {
    let mut host = fixtures::loaded_chat();
    let (exchange, credential) = fixtures::activity_read(&mut host);
    assert!(host.root.pages().chat().expect("chat").thread.reading.is_some(), "the first empty read is still owned");
    host.transport(jig_client::Event::Reply { exchange, message: fixtures::activity_answer(false) });
    let chat = host.root.pages().chat().expect("chat");
    assert!(chat.thread.reading.is_none(), "the empty answer completes the page's read");
    assert!(chat.thread.lines.rows.is_empty());
    assert_eq!(credential, Some(fixtures::credential()));
    assert!(host.out.is_empty(), "an empty quiet page does not poll");
}

#[test]
fn an_explicit_startup_link_takes_precedence_over_the_stored_credential() {
    let (host, signing, activity) = fixtures::link_chat(Some(fixtures::credential()));
    assert_ne!(signing, activity);
    assert!(host.loaded && host.started());
    assert_eq!(host.credential, Some(fixtures::credential()), "the loaded credential remains shell-owned");
    assert!(host.root.pages().chat().expect("chat").thread.reading.is_some());
}

#[test]
fn a_kept_link_sign_in_holds_the_stored_credential_without_writing_it() {
    let (mut host, signing, _) = fixtures::link_chat(Some(fixtures::credential()));
    host.transport(jig_client::Event::Reply { exchange: signing, message: fixtures::sign_in_answer(None) });
    assert_eq!(host.root.client().link().credential, Some(&fixtures::credential()));
    assert_eq!(host.credential, Some(fixtures::credential()));
    assert!(host.saving_signing.is_none(), "no token write delays the answer");
    assert_eq!(host.signing(), Some(Signing::SignedIn { party: CURRENT_PARTY }));
}

#[test]
fn a_refused_or_lost_startup_link_keeps_the_stored_credential() {
    for refused in [true, false] {
        let (mut host, signing, _) = fixtures::link_chat(Some(fixtures::credential()));
        let event = if refused {
            let answer = jig_client_protocol_wire::SignInAnswer::Refused;
            let mut writer = skein_lib::Writer::new(usize::try_from(answer.measure()).expect("answer size"));
            answer.encode(&mut writer).expect("answer room");
            jig_client::Event::Reply { exchange: signing, message: writer.finish() }
        } else {
            jig_client::Event::Lost { exchange: signing }
        };
        host.transport(event);
        assert_eq!(host.credential, Some(fixtures::credential()));
        assert!(host.saving_signing.is_none(), "no token write on a failed link");
        assert_eq!(host.signing(), Some(if refused { Signing::Refused } else { Signing::Lost }));
    }
}
#[test]
fn busy_keeps_the_link_and_held_token_for_the_next_sign_in() {
    use jig_client_protocol_wire as w;
    let (mut host, signing, _) = fixtures::link_chat(Some(fixtures::credential()));
    let answer = w::SignInAnswer::Busy;
    let mut writer = skein_lib::Writer::new(usize::try_from(answer.measure()).expect("answer size"));
    answer.encode(&mut writer).expect("answer room");
    host.transport(jig_client::Event::Reply { exchange: signing, message: writer.finish() });
    assert_eq!(host.signing(), Some(Signing::Busy));
    assert!(matches!(host.notices(), Some(Shown::Terminal(crate::terminal::Notice::SignInBusy))));
    assert_eq!(host.credential, Some(fixtures::credential()));
    assert!(host.saving_signing.is_none());
    host.sign_in(fixtures::LINK).expect("same printed link");
    let mut retry = None;
    for (exchange, out) in &host.authentication {
        assert!(!out, "retry signs in");
        assert!(retry.replace(*exchange).is_none(), "one retry");
    }
    host.transport(jig_client::Event::Reply {
        exchange: retry.expect("next sign-in exchange"),
        message: fixtures::sign_in_answer(None),
    });
    assert_eq!(host.signing(), Some(Signing::SignedIn { party: CURRENT_PARTY }));
    assert_eq!(
        host.root.client().link().credential,
        Some(&fixtures::credential()),
        "the retry's With::Link carried the same held token"
    );
    assert!(host.saving_signing.is_none(), "the held sign-in writes no token");
}

#[test]
fn full_installs_nothing_and_hands_both_times_to_the_terminal() {
    use jig_client_protocol_wire as w;
    let (mut host, signing, _) = fixtures::link_chat(Some(fixtures::credential()));
    let earliest = skein_lib::Wall::from_nanos(3_600_000_000_000);
    let link_until = skein_lib::Wall::from_nanos(300_000_000_000);
    let answer = w::SignInAnswer::Full(
        w::SignInsFull::new(
            &w::CEILINGS,
            w::SignInsFullParts { sign_ins: 2, earliest: earliest.as_nanos(), link_until: link_until.as_nanos() },
        )
        .expect("full answer"),
    );
    let mut writer = skein_lib::Writer::new(usize::try_from(answer.measure()).expect("answer size"));
    answer.encode(&mut writer).expect("answer room");
    host.transport(jig_client::Event::Reply { exchange: signing, message: writer.finish() });
    assert_eq!(host.signing(), Some(Signing::Full { sign_ins: 2, earliest, link_until }));
    assert!(
        matches!(host.notices(), Some(Shown::Terminal(crate::terminal::Notice::SignInFull { sign_ins: 2, earliest: t, link_until: u })) if t == earliest && u == link_until)
    );
    assert!(host.root.client().link().credential.is_none());
    assert_eq!(host.credential, Some(fixtures::credential()));
    assert!(host.saving_signing.is_none());
}

#[test]
fn first_link_sign_in_recovers_the_cancelled_nonempty_activity_read() {
    first_link_activity(true);
}

#[test]
fn first_link_sign_in_recovers_the_cancelled_empty_activity_read() {
    first_link_activity(false);
}

fn first_link_activity(nonempty: bool) {
    let (mut host, signing, old) = fixtures::link_chat(None);
    host.transport(jig_client::Event::Reply { exchange: signing, message: fixtures::signed_in() });
    assert_eq!(host.root.client().link().credential, Some(&fixtures::signed_credential()), "new identity installed");
    let current = fixtures::activity_reads(&mut host);
    // A late answer from before sign-in must never populate the new identity's page.
    host.transport(jig_client::Event::Reply { exchange: old, message: fixtures::activity_answer(nonempty) });
    host.transport(jig_client::Event::Lost { exchange: old });
    assert!(host.root.pages().chat().expect("chat").thread.lines.rows.is_empty(), "old answer is fenced");
    assert_eq!(current.len(), 1, "after successful first sign-in the page must ask again, even when quiet");
    let (exchange, credential) = &current[0];
    assert_ne!(*exchange, old, "the consumer asks anew");
    assert_eq!(credential.as_ref(), Some(&fixtures::signed_credential()), "fresh read uses the current identity");
    let message = fixtures::activity_answer(nonempty);
    host.transport(jig_client::Event::Reply { exchange: *exchange, message: message.clone() });
    let chat = host.root.pages().chat().expect("chat");
    assert!(chat.thread.reading.is_none(), "the current answer completes the read");
    assert_eq!(chat.thread.lines.rows.len(), u32::from(nonempty), "the actual page consumes the answer");
    host.transport(jig_client::Event::Reply { exchange: *exchange, message });
    host.transport(jig_client::Event::Lost { exchange: *exchange });
    assert_eq!(host.root.pages().chat().expect("chat").thread.lines.rows.len(), u32::from(nonempty));
    assert!(host.out.is_empty(), "no later activity or person act is needed");
}

#[test]
fn starting_retold_by_a_lost_sign_out_keeps_the_pending_link_activity_read() {
    let (mut host, current) = fixtures::recovered_link_chat();
    assert!(host.root.pages().chat().expect("chat").thread.reading.is_some());
    fixtures::lost_sign_out(&mut host);
    assert!(fixtures::activity_reads(&mut host).is_empty(), "Starting again must not issue a duplicate read");
    host.transport(jig_client::Event::Reply { exchange: current, message: fixtures::activity_answer(true) });
    let chat = host.root.pages().chat().expect("chat");
    assert_eq!(chat.thread.lines.rows.len(), 1, "the original pending R2 answer is still accepted");
    assert!(matches!(chat.thread.lines().as_slice(), [jig_client::thread::Line::RunBegan { attempt: fixtures::RUN }]));
    assert!(chat.thread.reading.is_none());
    assert!(host.out.is_empty(), "answering R2 quietly completes the read");
}

#[test]
fn starting_retold_after_an_empty_held_answer_does_not_read_again() {
    answered_held_activity(false);
}

#[test]
fn starting_retold_after_a_nonempty_held_answer_does_not_read_again() {
    answered_held_activity(true);
}

fn answered_held_activity(nonempty: bool) {
    let mut host = fixtures::loaded_chat();
    let (exchange, _) = fixtures::activity_read(&mut host);
    answered_activity(&mut host, exchange, nonempty);
}

#[test]
fn starting_retold_after_an_empty_link_answer_does_not_read_again() {
    answered_link_activity(false);
}

#[test]
fn starting_retold_after_a_nonempty_link_answer_does_not_read_again() {
    answered_link_activity(true);
}

fn answered_link_activity(nonempty: bool) {
    let (mut host, exchange) = fixtures::recovered_link_chat();
    answered_activity(&mut host, exchange, nonempty);
}

fn answered_activity(host: &mut NativeClient, exchange: skein_lib::Token, nonempty: bool) {
    host.transport(jig_client::Event::Reply { exchange, message: fixtures::activity_answer(nonempty) });
    let chat = host.root.pages().chat().expect("chat");
    assert!(chat.thread.reading.is_none(), "the first answer completed its read");
    assert_eq!(chat.thread.lines.rows.len(), u32::from(nonempty));
    assert!(host.out.is_empty(), "the first answer completes quietly");
    fixtures::lost_sign_out(host);
    assert!(fixtures::activity_reads(host).is_empty(), "an answered page does not reread on Starting");
    let chat = host.root.pages().chat().expect("chat");
    assert!(chat.thread.reading.is_none());
    assert_eq!(chat.thread.lines.rows.len(), u32::from(nonempty), "the answered page is retained");
    assert!(host.out.is_empty());
}

#[test]
fn quiet_chat_snapshot_before_the_fresh_empty_answer_does_not_duplicate_the_read() {
    quiet_snapshot(domain::Address::Chat { task: fixtures::TASK }, true);
}

#[test]
fn quiet_chat_snapshot_after_the_fresh_empty_answer_does_not_read_again() {
    quiet_snapshot(domain::Address::Chat { task: fixtures::TASK }, false);
}

#[test]
fn quiet_tracked_task_snapshot_before_the_fresh_empty_answer_does_not_duplicate_the_read() {
    quiet_snapshot(domain::Address::Task { task: fixtures::TASK, filter: domain::PlanFilter::All }, true);
}

#[test]
fn quiet_tracked_task_snapshot_after_the_fresh_empty_answer_does_not_read_again() {
    quiet_snapshot(domain::Address::Task { task: fixtures::TASK, filter: domain::PlanFilter::All }, false);
}

fn quiet_snapshot(address: domain::Address, snapshot_first: bool) {
    let (mut host, old, current, watch) = fixtures::recovered_unlinked_page(address);
    if snapshot_first {
        fixtures::quiet_task(&mut host, watch);
        assert!(fixtures::page_work(&mut host).0.is_empty(), "the initial quiet snapshot does not duplicate R2");
    }
    fixtures::activity_done(&mut host, current, false);
    assert!(host.out.is_empty(), "the fresh empty answer completes without traffic");
    fixtures::quiet_task(&mut host, watch);
    assert!(fixtures::page_work(&mut host).0.is_empty(), "a quiet snapshot needs no new activity read");
    assert_quiet_page(&host);
    fixtures::activity_done(&mut host, old, true);
    assert_quiet_page(&host);
    assert!(host.out.is_empty(), "the old nonempty answer is ignored without traffic or a person act");
}

fn assert_quiet_page(host: &NativeClient) {
    if let Some(chat) = host.root.pages().chat() {
        assert!(chat.task(host.root.client()).is_some(), "the initial task snapshot was consumed");
        assert!(chat.thread.reading.is_none(), "the empty read completed");
        assert!(chat.thread.lines.rows.is_empty(), "the old answer cannot populate the chat");
    } else {
        let task = host.root.pages().task().expect("task page");
        assert_eq!(task.shape, domain::pages::task::Shape::Tracked);
        assert_eq!(task.through, Some(0), "the initial quiet snapshot was consumed");
        let page = task.latest.as_ref().expect("the tracked page retained the empty activity answer");
        assert!(page.rows.is_empty(), "the old answer cannot replace the empty tracked page");
        assert!(task.thread.lines.rows.is_empty());
    }
}

#[test]
fn direct_sign_out_clears_the_pending_fresh_read_without_retrying_at_later_sign_in() {
    let (mut host, _, current, _) = fixtures::recovered_unlinked_page(domain::Address::Chat { task: fixtures::TASK });
    assert!(host.root.pages().chat().expect("chat").thread.reading.is_some(), "R2 is pending");
    host.event(domain::Event::SignedOut);
    assert_eq!(host.root.client().link().state, jig_client::Link::SignedOut);
    assert!(host.root.pages().chat().expect("chat").thread.reading.is_none(), "sign-out clears the read intent");
    assert!(fixtures::page_work(&mut host).0.is_empty(), "sign-out must not retry activity");
    host.event(domain::Event::Client { event: jig_client::Event::SignIn { with: jig_client::With::Transport } });
    assert_eq!(host.root.client().link().state, jig_client::Link::Starting);
    assert!(fixtures::page_work(&mut host).0.is_empty(), "later sign-in cannot invent new page work");
    fixtures::activity_done(&mut host, current, true);
    let chat = host.root.pages().chat().expect("chat");
    assert!(chat.thread.reading.is_none());
    assert!(chat.thread.lines.rows.is_empty(), "the signed-out read's late answer is fenced");
    assert!(host.out.is_empty());
}

#[test]
fn another_partys_kept_work_prints_its_line_once() {
    let (mut host, watch) = fixtures::started_kept();
    for _ in 0..2 {
        host.event(domain::Event::Client {
            event: jig_client::Event::Up {
                up: jig_client::Up::Watched {
                    watch,
                    event: jig_client::contract::WatchEvent::Delivered(jig_client::contract::Delivery::Me(Box::new(
                        jig_client_world::stories::frame(CURRENT_PARTY),
                    ))),
                },
            },
        });
    }
    let mut world = machine::World::signed_in();
    let Some(Shown::Terminal(notice @ crate::terminal::Notice::OtherPartyKept)) = host.notices() else {
        panic!(
            "other-party notice: link {:?}, pending {:?}, keys {:?}",
            host.root.client().link(),
            host.root.client().pending(common::table_record().request.key),
            host.kept_keys
        )
    };
    world.client.notice_at_once(notice);
    assert!(host.notices().is_none(), "the native host emits it once");
    world.run_until(PRINTED);
    world.run_until(STILL_PRINTED);
    assert_eq!(
        world.shell_lines(),
        vec![Box::<[u8]>::from(b"temper: work of another party is kept in client, and waits for them\n".as_slice())]
    );
}
#[test]
fn a_service_credential_is_a_notice_never_a_root_event() {
    let mut host = common::native();
    host.transport(jig_client::Event::Reply { exchange: common::ATTEMPT, message: common::service_answer() });
    let Some(Shown::Notice(route::Notice::Credential { party, credential })) = host.notices() else {
        panic!("credential notice")
    };
    assert_eq!(party, common::SERVICE);
    assert_eq!(credential.expose(), common::CREDENTIAL);
    host.outputs();
    assert!(host.notices().is_none());
}

#[test]
fn kept_work_of_another_version_prints_its_line_once() {
    let mut host = common::native();
    host.out.push(domain::Request::Dropped { count: 2 });
    host.outputs();
    let notice = crate::terminal::Client::notice(&mut host).expect("dropped-work notice");
    assert!(crate::terminal::Client::notice(&mut host).is_none(), "the host emits it once");
    let mut world = machine::World::signed_in();
    world.client.notice_at_once(notice);
    world.run_until(PRINTED);
    world.run_until(STILL_PRINTED);
    assert_eq!(
        world.shell_lines(),
        vec![Box::<[u8]>::from(
            b"temper: 2 requests kept in client were not sent: another version of temper made them\n".as_slice()
        )]
    );
}

#[test]
fn the_deadline_takes_the_earlier_policy_or_view_time_from_the_same_clocks() {
    let now = Time::from_nanos(100);
    let wall = Wall::from_nanos(1000);
    for (policy, next, expected) in [
        (Some(105), Some(1020), Some(105)),
        (Some(130), Some(1020), Some(120)),
        (None, Some(1020), Some(120)),
        (Some(130), None, Some(130)),
        (None, None, None),
        (None, Some(990), Some(100)),
    ] {
        assert_eq!(
            deadline(policy.map(Time::from_nanos), next.map(Wall::from_nanos), now, wall),
            expected.map(Time::from_nanos)
        );
    }
}

#[test]
fn a_chat_keeps_its_activity_read_through_local_exchange_pressure() {
    use jig_client::{Asker, Handle, Message, Readable, Request, contract as c};
    let mut host = fixtures::loaded_chat();
    let mut occupied = 0_u32;
    let mut old_activity = None;
    while let Some(request) = host.out.pop() {
        if let domain::Request::Client { request: Request::Exchange { exchange, message, .. } } = request {
            occupied += 1;
            if let Message::Read(bytes) = message {
                let owned = jig_client_protocol_wire::Owned::decode(
                    &jig_client_protocol_wire::CEILINGS,
                    &mut skein_lib::Reader::new(&bytes),
                )
                .expect("read envelope");
                if owned.owner() == &jig_client_protocol_wire::Owner::Jig {
                    let read = jig_client_protocol_wire::Read::decode(
                        &jig_client_protocol_wire::CEILINGS,
                        &mut skein_lib::Reader::new(owned.record()),
                    )
                    .expect("jig read");
                    if matches!(read, jig_client_protocol_wire::Read::Activity(_)) {
                        assert!(old_activity.replace(exchange).is_none());
                    }
                }
            }
        }
    }
    for slot in occupied..crate::limits::EXCHANGES {
        host.event(domain::Event::Client {
            event: jig_client::Event::Read {
                handle: Handle { asker: Asker::JIG, number: 100 + u64::from(slot) },
                read: Readable::Jig(c::Read::Project { project: 1 }),
            },
        });
        let Some(domain::Request::Client { request: Request::Exchange { .. } }) = host.out.pop() else {
            panic!("fill one native exchange slot")
        };
        assert!(host.out.is_empty());
    }
    let handle = host.root.pages().chat().expect("chat").activity;
    host.event(domain::Event::Client {
        event: jig_client::Event::Read {
            handle,
            read: Readable::Jig(c::Read::Activity { task: fixtures::TASK, before: None }),
        },
    });
    assert!(host.out.is_empty(), "local pressure emits no failed page or exchange");
    assert!(host.root.pages().chat().expect("chat").thread.reading.is_some(), "the page keeps its demand");
    let old = old_activity.expect("initial activity exchange occupies a slot");
    host.transport(jig_client::Event::Reply { exchange: old, message: fixtures::activity_answer(true) });
    assert!(host.root.pages().chat().expect("chat").thread.lines.rows.is_empty(), "the superseded answer is fenced");
    let (current, _) = fixtures::activity_read(&mut host);
    assert_ne!(current, old, "the slot release admits the pending read");
    host.transport(jig_client::Event::Reply { exchange: current, message: fixtures::activity_answer(true) });
    let chat = host.root.pages().chat().expect("chat");
    assert_eq!(chat.thread.lines.rows.len(), 1);
    assert_eq!(chat.thread.loaded, 1);
    assert!(chat.thread.reading.is_none(), "the actual activity answer completed the demand");
    assert!(fixtures::activity_reads(&mut host).is_empty(), "the demand was sent only once");
}
