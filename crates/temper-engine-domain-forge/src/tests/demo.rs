use super::*;

#[test]
fn only_the_durable_demo_end_retires_restart_observations() {
    let mut failures = skein_lib::List::with_capacity(6);
    for (case, end_before, restored_ended, end_during, live) in [
        ("end during fresh", false, false, true, false),
        ("end before fresh", true, false, false, false),
        ("already ended at restore", false, true, false, false),
        ("another live forge owes a read", false, false, true, true),
        ("ordinary disconnection", false, false, false, false),
    ] {
        let mut d = restart_observations(restored_ended, live);
        if end_before {
            connected(&mut d, 2);
        }
        let mut done = fresh_done(&outputs(&mut d, Event::ReadAfresh));
        if end_during {
            assert!(!d.goal_file_reads.is_empty(), "file read owed before the end");
            done |= fresh_done(&connected(&mut d, 2));
            assert!(d.goal_file_reads.is_empty(), "the end retires file reads");
            assert_eq!(done, !live, "only the live forge can still hold the fresh pass");
        }
        for _ in 0_u32..20 {
            for request in ready(&mut d) {
                if let Request::Call { call, .. } = request {
                    done |= fresh_done(&fixtures::answer(&mut d, call, Err(skein_forge::api::Error::NotConnected)));
                }
            }
            d.reclaim();
        }
        let ended = end_before || restored_ended || end_during;
        let expected = ended && !live;
        if done != expected {
            failures.push((case, done, expected)).expect("one mismatch per case");
        }
        if ended && !live {
            // Complete replacement sets must never revive historical watches.
            outputs(&mut d, Event::Names { task: 8, resources: Box::new([run_branch(b"7")]) });
            outputs(&mut d, Event::Unnamed { task: 8 });
            let released = outputs(
                &mut d,
                Event::Release { task: 7, root: 7, ending: ReleaseEnding::Cancelled, reason: None, entry: 20 },
            );
            assert!(released.contains(&Request::Released { task: 7 }), "{case}: local release: {released:?}");
            assert!(d.hold(&run_branch(b"7")).is_none(), "{case}: hold erased");
            for request in ready(&mut d) {
                if let Request::Call { repository, .. } = request {
                    assert_ne!(repository.forge, 1, "{case}: retired watches stay retired");
                }
            }
        }
        if live {
            assert!(!done, "the live forge still owes its observation");
            connected(&mut d, 2);
            for _ in 0_u32..20 {
                for request in ready(&mut d) {
                    if let Request::Call { call, repository, .. } = request {
                        let result = if repository.forge == 2 {
                            Err(skein_forge::api::Error::Missing)
                        } else {
                            Err(skein_forge::api::Error::NotConnected)
                        };
                        done |= fresh_done(&fixtures::answer(&mut d, call, result));
                    }
                }
                d.reclaim();
            }
            if !done {
                failures.push(("live forge answered", done, true)).expect("one live completion mismatch");
            }
        }
    }
    assert!(failures.is_empty(), "restart table: {failures:?}");
}

fn restart_observations(restored_ended: bool, live: bool) -> Domain {
    let mut d = configured(true);
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    outputs(&mut d, Event::Names { task: 7, resources: Box::new([run_branch(b"7")]) });
    outputs(
        &mut d,
        Event::Restore {
            record: Stored::Demo(Demo {
                forge: 1,
                state: if restored_ended { DemoState::Ended } else { DemoState::Running },
            }),
        },
    );
    for forge in 1..=if live { 2 } else { 1 } {
        let mut name = run_branch(b"7");
        name.forge = forge;
        outputs(
            &mut d,
            Event::Restore {
                record: Stored::Hold(Hold {
                    name,
                    task: 7,
                    writer: Some(Writer::Lost { task: 7, attempt: 1 }),
                    drift: None,
                }),
            },
        );
    }
    let mut change = fixtures::opening_row();
    change.pull = Some(1);
    outputs(&mut d, Event::Restore { record: Stored::Change(change) });
    for topic in [
        Topic::Ci { repository: REPO, head: fixtures::HEAD },
        Topic::Landings { repository: REPO, branch: b"main".as_slice().into() },
    ] {
        outputs(
            &mut d,
            Event::Restore {
                record: Stored::Subscription(Subscriber {
                    task: 8,
                    number: 1,
                    topic,
                    own_change: None,
                    goal_tasks: Some(Box::new([fixtures::CHANGE])),
                    paths: Box::new([]),
                }),
            },
        );
    }
    outputs(&mut d, Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic });
    d
}

fn fresh_done(sent: &[Request]) -> bool {
    for request in sent {
        if let Request::RestartDone { stage: RestartStage::ReadAfresh } = request {
            return true;
        }
    }
    false
}

fn configured(under_box: bool) -> Domain {
    let mut d = domain();
    d.under_box = under_box;
    for number in [1, 2] {
        d.configured
            .insert(
                number,
                Configured {
                    admin_connect: false,
                    bot: Box::new([]),
                    owned: None,
                    git: Box::new([]),
                    web: None,
                    read_only: false,
                    number,
                    kind: Kind::Forgejo,
                    mode: Mode::Fake,
                    asked: true,
                },
            )
            .unwrap();
    }
    d.connection_limits(3, 1);
    d.configure_demo(Some(DemoConfig {
        provider: 1,
        adoption: fixtures::adoption(),
        deadline: Duration::from_secs(10),
    }));
    d
}
fn start(d: &mut Domain, number: u64) -> Box<[Request]> {
    outputs(
        d,
        Event::Application {
            role: None,
            carried: None,
            request: Token::new(number),
            token: Token::new(number.checked_add(100).unwrap()),
            retention: Duration::from_secs(60),
            ask: Ask::StartDemo,
        },
    )
}

#[test]
fn demo_start_is_not_offered_without_a_demo_or_after_its_end() {
    for ended in [false, true] {
        let mut d = if ended { configured(true) } else { domain() };
        outputs(&mut d, Event::Restore { record: Stored::Demo(Demo { forge: 1, state: DemoState::Ended }) });
        assert_eq!(
            start(&mut d, 1).as_ref(),
            [Request::ApplicationDone { request: Token::new(1), token: Token::new(101) }]
        );
        assert_eq!(d.application_answer(Token::new(101)), Some(Answered::StartDemo(StartAnswer::NotOffered)));
    }
}

#[test]
fn demo_first_start_saves_running_and_only_asks_a_dormant_group_under_the_box() {
    for under_box in [false, true] {
        for connection in [
            Connection::NotConnected,
            Connection::Connecting,
            Connection::Connected { id: 7, login: b"temper".as_slice().into() },
        ] {
            let mut d = configured(under_box);
            outputs(&mut d, Event::Connection { forge: 1, state: connection.clone() });
            let sent = start(&mut d, 1);
            assert_eq!(
                sent.first(),
                Some(&Request::Save {
                    read_afresh: None,
                    release: None,
                    record: Stored::Demo(Demo { forge: 1, state: DemoState::Running }),
                })
            );
            let asks = group_asks(&sent);
            assert_eq!(asks, usize::from(under_box && connection == Connection::NotConnected));
            assert!(sent.contains(&Request::DemoStart { request: Token::new(1) }));
            outputs(&mut d, Event::DemoAnswered { answer: StartAnswer::Running });
            assert!(!saved(&start(&mut d, 2)));
        }
    }
}

#[test]
fn simultaneous_demo_starts_share_the_answer_and_bound_their_waiters() {
    let mut d = configured(true);
    start(&mut d, 1);
    for number in 2..=u64::from(LIMITS.applications) {
        assert!(start(&mut d, number).is_empty());
    }
    assert_eq!(
        start(&mut d, 20).as_ref(),
        [Request::ApplicationAgain { request: Token::new(20), why: jig_domain_people::Again::Busy }]
    );
    let answer = StartAnswer::Failed(StartFailure::Account);
    let sent = outputs(&mut d, Event::DemoAnswered { answer: answer.clone() });
    assert_eq!(sent.len(), usize::try_from(LIMITS.applications).unwrap());
    for number in 1..=u64::from(LIMITS.applications) {
        assert_eq!(
            d.application_answer(Token::new(number.checked_add(100).unwrap())),
            Some(Answered::StartDemo(answer.clone()))
        );
    }
    assert_eq!(d.application_answer(Token::new(900)), None);
}

#[test]
fn demo_restore_asks_only_for_a_running_dormant_group_and_makes_no_act() {
    for under_box in [false, true] {
        for state in [DemoState::Running, DemoState::Ended] {
            for connection in [
                Connection::NotConnected,
                Connection::Connecting,
                Connection::Connected { id: 7, login: b"temper".as_slice().into() },
            ] {
                let mut d = configured(under_box);
                outputs(&mut d, Event::Connection { forge: 1, state: connection.clone() });
                outputs(&mut d, Event::Restore { record: Stored::Demo(Demo { forge: 1, state }) });
                let sent = outputs(&mut d, Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic });
                assert_eq!(
                    group_asks(&sent),
                    usize::from(under_box && state == DemoState::Running && connection == Connection::NotConnected)
                );
                assert!(!makes_act(&sent));
                assert_eq!(d.demo_state(), Some(state));
                assert_eq!(d.application_answer(Token::new(101)), None);
            }
        }
    }
}

fn group_asks(sent: &[Request]) -> usize {
    let mut count = 0_usize;
    for request in sent {
        if let Request::SetUpGroup { .. } = request {
            count = count.checked_add(1).unwrap();
        }
    }
    count
}
fn saved(sent: &[Request]) -> bool {
    for request in sent {
        if let Request::Save { .. } = request {
            return true;
        }
    }
    false
}
fn makes_act(sent: &[Request]) -> bool {
    for request in sent {
        if let Request::DemoStart { .. } = request {
            return true;
        }
    }
    false
}

fn connected(d: &mut Domain, forge: u16) -> Box<[Request]> {
    outputs(d, Event::Connection { forge, state: Connection::Connected { id: 7, login: b"temper".as_slice().into() } })
}

#[test]
fn first_other_connection_ends_once_from_none_or_running_and_answers_held_starts() {
    for under_box in [false, true] {
        for running in [false, true] {
            let mut d = configured(under_box);
            if running {
                start(&mut d, 1);
            }
            let sent = connected(&mut d, 2);
            assert!(sent.contains(&Request::Save {
                read_afresh: None,
                release: None,
                record: Stored::Demo(Demo { forge: 1, state: DemoState::Ended })
            }));
            assert_eq!(sent.contains(&Request::StopGroup { forge: 1 }), under_box && running);
            assert_eq!(d.demo_state(), Some(DemoState::Ended));
            assert!(d.forge_ended(1));
            assert!(!d.forge_ended(2));
            let forges = d.forges();
            let [forge] = forges.rows.as_ref() else { panic!("only the other forge remains listed") };
            assert_eq!(forge.number, 2);
            assert_eq!(d.offer(1), Offer::NotOffered);
            if running {
                assert!(sent.contains(&Request::ApplicationDone { request: Token::new(1), token: Token::new(101) }));
                assert_eq!(d.application_answer(Token::new(101)), Some(Answered::StartDemo(StartAnswer::NotOffered)));
            }
            start(&mut d, 2);
            assert_eq!(d.application_answer(Token::new(102)), Some(Answered::StartDemo(StartAnswer::NotOffered)));
            let sent = connected(&mut d, 2);
            assert!(!saved(&sent));
            assert!(!sent.contains(&Request::StopGroup { forge: 1 }));
            assert!(connected(&mut d, 1).is_empty());
            assert_eq!(d.connection(1), Connection::NotConnected);
        }
    }
}

#[test]
fn own_unconfigured_and_no_demo_connections_do_not_end_the_demo() {
    let mut d = configured(true);
    assert!(!saved(&connected(&mut d, 1)));
    connected(&mut d, 3);
    assert_eq!(d.demo_state(), None);
    let mut d = configured(true);
    d.configure_demo(None);
    connected(&mut d, 2);
    assert_eq!(d.demo_state(), None);
}

#[test]
fn restored_ended_demo_hides_repositories_but_keeps_lookups_and_refuses_setup() {
    let mut d = configured(true);
    let home = adopted();
    let mut context = home.clone();
    context.provider.repository = 3;
    context.home = false;
    context.role = Role::Context;
    context.name = b"context".as_slice().into();
    for row in [home.clone(), context] {
        outputs(&mut d, Event::Restore { record: Stored::Repository(row) });
    }
    outputs(&mut d, Event::Restore { record: Stored::Demo(Demo { forge: 1, state: DemoState::Ended }) });
    let sent = outputs(&mut d, Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic });
    assert_eq!(group_asks(&sent), 0);
    assert!(d.forge_ended(1));
    assert_eq!(d.home(5), None);
    assert!(d.context_repositories(5, 2).unwrap().is_empty());
    assert!(d.adopted_ids(1).is_empty());
    assert_eq!(d.repository(REPO), Some(&home));
    assert_eq!(d.repository_tag(5, tag(REPO.forge, REPO.repository)), Some(&home));
    assert!(connected(&mut d, 1).is_empty());
    outputs(
        &mut d,
        Event::Application {
            role: None,
            carried: None,
            request: Token::new(4),
            token: Token::new(104),
            retention: Duration::from_secs(60),
            ask: Ask::SetUp {
                playground: false,
                forge: 1,
                org: b"org".as_slice().into(),
                repository: b"repo".as_slice().into(),
                project: SetUpProject::Existing(5),
            },
        },
    );
    assert_eq!(
        d.application_answer(Token::new(104)),
        Some(Answered::SetUp(SetUpAnswer::Refused(SetUpRefusal::NotOffered)))
    );
}

#[test]
fn an_ended_home_does_not_prevent_a_new_home_adoption() {
    let mut d = configured(false);
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    outputs(&mut d, Event::Restore { record: Stored::Demo(Demo { forge: 1, state: DemoState::Ended }) });
    connected(&mut d, 2);
    let mut request = fixtures::adoption();
    request.forge = 2;
    outputs(
        &mut d,
        Event::Application {
            role: None,
            carried: None,
            request: Token::new(4),
            token: Token::new(104),
            retention: Duration::from_secs(60),
            ask: Ask::Adopt(request),
        },
    );
    let sent = ready(&mut d);
    let [Request::Call { call, .. }] = sent.as_ref() else { panic!("locate on the other forge") };
    outputs(
        &mut d,
        Event::Client(skein_forge_client::Event::Answered {
            call: *call,
            cost: 1,
            free: 0,
            rate: None,
            result: Ok(skein_forge::api::Answer::Located { repository: 9 }),
        }),
    );
    let sent = outputs(&mut d, Event::Adopt { request: Token::new(4) });
    assert!(sent.is_empty());
    let mut permission = false;
    for request in ready(&mut d).as_ref() {
        if let Request::Call { op: skein_forge::api::Op::Read(skein_forge::api::Read::Permission { .. }), .. } = request
        {
            permission = true;
        }
    }
    assert!(permission);
    assert_eq!(d.application_outcome(Token::new(104)), None);
}

#[test]
fn closing_the_box_before_the_end_suppresses_stop_and_ended_connections_stay_inert() {
    let mut d = configured(true);
    start(&mut d, 1);
    outputs(&mut d, Event::BoxClosed);
    assert!(!connected(&mut d, 2).contains(&Request::StopGroup { forge: 1 }));
    for state in [
        Connection::NotConnected,
        Connection::NotSetUp,
        Connection::Connecting,
        Connection::SettingUp { step: Some(SetUpStep::Run) },
        Connection::SetUpFailed { why: b"failed".as_slice().into() },
        Connection::Stopped { reason: b"stopped".as_slice().into() },
        Connection::Connected { id: 99, login: b"different".as_slice().into() },
    ] {
        assert!(outputs(&mut d, Event::Connection { forge: 1, state }).is_empty());
        assert_eq!(d.connection(1), Connection::NotConnected);
        assert_eq!(d.demo_state(), Some(DemoState::Ended));
    }
}

#[test]
fn a_lost_writers_read_waiting_on_the_ended_demo_forge_is_never_asked_again() {
    let mut d = configured(true);
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    outputs(
        &mut d,
        Event::Restore {
            record: Stored::Hold(Hold {
                name: run_branch(b"7"),
                task: 7,
                writer: Some(Writer::Lost { task: 9, attempt: 1 }),
                drift: None,
            }),
        },
    );
    outputs(&mut d, Event::Restore { record: Stored::Demo(Demo { forge: 1, state: DemoState::Running }) });
    outputs(&mut d, Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic });
    outputs(&mut d, Event::ReadAfresh);
    outputs(&mut d, Event::SettleOutbox);
    fixtures::answer_next(&mut d, Err(skein_forge::api::Error::NotConnected));
    assert!(!d.is_ready(), "the read waits for the demo forge's connection");
    connected(&mut d, 2);
    assert!(d.forge_ended(1));
    assert!(connected(&mut d, 1).is_empty());
    assert!(!d.is_ready(), "an ended forge's connection asks no read again");
    assert!(ready(&mut d).is_empty());
    assert_eq!(d.hold(&run_branch(b"7")).unwrap().writer, Some(Writer::Lost { task: 9, attempt: 1 }));
}

#[test]
fn demo_acts_are_saved_before_send_restored_kept_and_erased_on_answer() {
    let rows = Box::new([
        DemoAct { key: [1; 16], act: DemoActKind::Close { project: 5, task: 7 } },
        DemoAct { key: [2; 16], act: DemoActKind::RemoveAccount { account: 3 } },
    ]);
    let mut d = configured(false);
    let sent = outputs(&mut d, Event::DemoActs { acts: rows.clone() });
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(sent[index], Request::Save { read_afresh: None, release: None, record: Stored::DemoAct(*row) });
        assert_eq!(sent[index + 2], Request::SendDemoAct(*row));
    }
    let mut restored = configured(false);
    for row in rows.as_ref() {
        assert!(outputs(&mut restored, Event::Restore { record: Stored::DemoAct(*row) }).is_empty());
    }
    assert_eq!(
        outputs(&mut restored, Event::SendDemoActs).as_ref(),
        [Request::SendDemoAct(rows[0]), Request::SendDemoAct(rows[1])]
    );
    assert!(outputs(&mut restored, Event::DemoActAnswered { key: rows[0].key, keep: true }).is_empty());
    for row in rows.as_ref() {
        assert_eq!(
            outputs(&mut restored, Event::DemoActAnswered { key: row.key, keep: false }).as_ref(),
            [Request::Erase { key: Key::DemoAct(row.key) }]
        );
    }
    assert!(outputs(&mut restored, Event::SendDemoActs).is_empty());
}

#[test]
fn demo_act_room_is_tasks_plus_one_and_reclaimed_on_answer() {
    let mut d = configured(false);
    let bound = demo_act_bound(&LIMITS).unwrap();
    assert_eq!(bound, LIMITS.tasks + 1);
    for number in 0..bound {
        let key = [u8::try_from(number).expect("small act bound"); 16];
        outputs(
            &mut d,
            Event::Restore {
                record: Stored::DemoAct(DemoAct {
                    key,
                    act: DemoActKind::Close { project: 5, task: u64::from(number) },
                }),
            },
        );
    }
    assert_eq!(outputs(&mut d, Event::SendDemoActs).len(), usize::try_from(bound).unwrap());
    outputs(&mut d, Event::DemoActAnswered { key: [0; 16], keep: false });
    outputs(
        &mut d,
        Event::Restore {
            record: Stored::DemoAct(DemoAct { key: [255; 16], act: DemoActKind::RemoveAccount { account: 3 } }),
        },
    );
    assert_eq!(outputs(&mut d, Event::SendDemoActs).len(), usize::try_from(bound).unwrap());
    let mut overflowing = LIMITS;
    overflowing.tasks = u32::MAX;
    assert_eq!(demo_act_bound(&overflowing), None);
    assert_eq!(worst_case(&overflowing), None);
}

#[test]
#[should_panic(expected = "restored demo acts within limits")]
fn restoring_more_than_tasks_plus_one_demo_acts_is_refused() {
    let mut d = configured(false);
    for number in 0..=demo_act_bound(&LIMITS).unwrap() {
        let key = [u8::try_from(number).expect("small act bound"); 16];
        outputs(
            &mut d,
            Event::Restore {
                record: Stored::DemoAct(DemoAct {
                    key,
                    act: DemoActKind::Close { project: 5, task: u64::from(number) },
                }),
            },
        );
    }
}
