//! Ownership identities beside the byte pacing world stories.
use super::{held_assignment, resume_fixture, turn};
use crate::{Decision, Delivery, Domain, Event, Limits, Payload, Request, Write};
use alloc::boxed::Box;
use jig_domain_core as core;
use jig_domain_tasks as tasks;
use skein_lib::{Env, Queue, ReplyTo, Token};

fn now(domain: &mut Domain, env: &Env<Limits>, decision: &mut Decision, output: core::Now) {
    let mut requests = Queue::with_capacity(1);
    requests.push(core::Request::Now(Box::new(output)));
    crate::route::route_core_requests(domain, env, decision, core::Requests::Out(requests));
}

fn payload(domain: &mut Domain) -> Token {
    let payload = Payload::Turn {
        task: 1,
        attempt: 1,
        turn: 1,
        cumulative: 1,
        read: None,
        transcript: b"turn body".as_slice().into(),
        opens_window: true,
        refused: Box::new([]),
    };
    domain.hold_bytes(crate::budget::payload_bytes(&payload));
    domain.payloads.insert(Some(payload)).unwrap().token()
}

#[test]
fn dropped_and_refused_turn_payloads_release_their_bodies() {
    for refused in [false, true] {
        let (mut domain, env) = resume_fixture::root();
        let mut decision = Decision::decide(&mut domain.journal, &env.limits.journal);
        let token = payload(&mut domain);
        assert_eq!(domain.held_bytes(), 9);
        let output = if refused {
            core::Now::RefusedPayload { request: token, problem: tasks::Problem::new(Some(1), tasks::Refusal::Read) }
        } else {
            core::Now::DropPayload { payload: token }
        };
        now(&mut domain, &env, &mut decision, output);
        assert_eq!(domain.held_bytes(), 0);
        domain.reclaim();
        assert_eq!(domain.held_bytes(), 0, "slot reclamation changes no body ownership");
    }
}

#[test]
fn completed_sections_are_counted_until_a_failed_preparation_drops_them() {
    let (mut domain, env) = resume_fixture::root();
    let mut decision = Decision::decide(&mut domain.journal, &env.limits.journal);
    now(
        &mut domain,
        &env,
        &mut decision,
        core::Now::CompleteBrief {
            brief: Token::new(1),
            order: Box::new([jig_domain_brief::GatherPlaced::Core {
                kind: jig_domain_brief::Core::Task,
                text: b"completed section".as_slice().into(),
            }]),
        },
    );
    assert_eq!(domain.held_bytes(), 17);
    now(&mut domain, &env, &mut decision, core::Now::RunPreparationFailed { task: 1 });
    assert_eq!(domain.held_bytes(), 0);
}

#[test]
fn dropped_and_released_assignments_release_their_turns_and_sections_once() {
    for path in 0..3 {
        let (mut domain, env) = resume_fixture::root();
        let mut assignment = held_assignment();
        assignment.transcript = Box::new([b"history".as_slice().into()]);
        let bytes = crate::budget::assignment_bytes(&assignment);
        assert_eq!(bytes, 12);
        domain.hold_bytes(bytes);
        let mut decision = Decision::decide(&mut domain.journal, &env.limits.journal);
        let mut out = Queue::uncapped();
        match path {
            0 => {
                domain.assignments.insert(1, assignment).unwrap();
                now(&mut domain, &env, &mut decision, core::Now::DropAssignment { task: 1 });
                assert_eq!(domain.held_bytes(), 0);
            }
            1 => {
                decision.carry_delivery(&env.limits.journal, Delivery::Assigned { channel: Token::new(7), assignment });
                assert_eq!(domain.held_bytes(), bytes, "a move into the decision changes nothing");
                crate::decision::accept_routed(
                    &mut domain.journal,
                    &mut domain.core.counters,
                    &env.limits.journal,
                    decision,
                    false,
                )
                .unwrap();
                crate::release(&mut domain, &env, &mut out);
                assert_eq!(domain.held_bytes(), 0);
                assert!(matches!(
                    out.pop(),
                    Some(Request::Worker(crate::WorkerRequest::Deliver(Delivery::Assigned { .. })))
                ));
                crate::release(&mut domain, &env, &mut out);
                assert!(out.is_empty());
            }
            2 => {
                out.push(crate::boundary::delivery_output(Delivery::Assigned { channel: Token::new(7), assignment }));
                crate::domain::stage_now(&mut domain, &mut out);
                assert_eq!(domain.held_bytes(), bytes, "the journal door still owns the assignment");
                crate::release(&mut domain, &env, &mut out);
                assert_eq!(domain.held_bytes(), 0, "the door release pass also transfers ownership");
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn replacing_a_turn_write_releases_only_the_old_body_and_issue_releases_the_new_one() {
    let (mut domain, env) = resume_fixture::root();
    let mut decision = Decision::decide(&mut domain.journal, &env.limits.journal);
    let mut requests = Queue::with_capacity(2);
    for length in [5, 7] {
        let write = turn(1, length);
        domain.hold_bytes(crate::budget::turn_bytes(&write));
        let Write::Save(crate::Record::Core(row)) = write else { unreachable!() };
        requests.push(core::Request::Write(core::Write::Save(row)));
    }
    crate::route::route_core_requests(&mut domain, &env, &mut decision, core::Requests::Out(requests));
    assert_eq!(domain.held_bytes(), 7);
    crate::decision::accept_routed(&mut domain.journal, &mut domain.core.counters, &env.limits.journal, decision, true)
        .unwrap();
    assert_eq!(domain.held_bytes(), 7, "unsent journal write keeps its body charged");
    let mut out = Queue::uncapped();
    crate::release(&mut domain, &env, &mut out);
    assert_eq!(domain.held_bytes(), 0, "issue, before durability, transfers the body to the store");
    let Some(Request::Store(crate::StoreRequest::Commit { number, .. })) = out.pop() else { panic!("issued write") };
    crate::try_step(&mut domain, &env, Event::Store(crate::StoreInput::Committed { number, fill: 0 })).unwrap();
    assert_eq!(domain.held_bytes(), 0, "durability acknowledges metadata only");
}

// A real task in the child with its current proof, for accepted-turn callbacks
// that the fleet would ordinarily produce. Its bodies enter through the slab.
fn turn_root() -> (Domain, Env<Limits>) {
    let (mut domain, env) = resume_fixture::root();
    let mut out = Queue::uncapped();
    let task_env = Env { now: env.now, wall: env.wall, limits: env.limits.tasks };
    for event in [
        tasks::Event::Restored,
        tasks::Event::OpenPeriod { reply_to: ReplyTo::new(Token::new(2)), project: 1, period: 1, budget: 100 },
        tasks::Event::Make {
            reply_to: ReplyTo::new(Token::new(3)),
            creator: tasks::Party::Deployment { project: 1 },
            batch: Box::new([tasks::New {
                number: 1,
                project: 1,
                label: 0,
                flags: tasks::Flags::PLAIN,
                executor: tasks::Executor::Agent { charter: 1 },
                llm: None,
                spec: tasks::Spec {
                    words: b"turn task".as_slice().into(),
                    parameters: Box::new([]),
                    inputs: Box::new([]),
                },
                contract: tasks::Contract::Report { words: 1 },
                deadline: tasks::Deadline::Named,
                authority: held_assignment().run.authority,
                numbers: tasks::Numbers { budget: 1, spent: 0, spent_below: 0, reserved: 0 },
                funder: tasks::Funder::Period { project: 1, period: 1 },
                dependencies: Box::new([]),
                holdings: Box::new([]),
                wake: tasks::WakePolicy::DEFAULT,
                recurring: None,
                tracked: None,
            }]),
        },
    ] {
        tasks::step(&mut domain.core.tasks, &task_env, event, &mut out);
        while out.pop().is_some() {}
    }
    assert!(domain.core.tasks.task(1).is_some());
    domain
        .core
        .proofs
        .insert(
            1,
            core::RunProof {
                task: 1,
                attempt: 1,
                conversation: 1,
                credentials: Box::new([]),
                accounts: Box::new([]),
                placement: Some(Token::new(7)),
                account_failure: None,
                account_terminal: false,
                window: None,
                crossed: false,
                host: jig_domain_fleet::HostKind::Worker,
                offered: None,
                outstanding: Box::new([]),
                turn: None,
                terminal: None,
            },
        )
        .unwrap();
    (domain, env)
}

#[test]
fn accepted_new_turns_move_the_charge_to_a_write_and_already_turns_release_it() {
    for accepted in [tasks::Accepted::New, tasks::Accepted::Already] {
        let (mut domain, env) = turn_root();
        let mut decision = Decision::decide(&mut domain.journal, &env.limits.journal);
        let token = payload(&mut domain);
        now(
            &mut domain,
            &env,
            &mut decision,
            core::Now::AcceptedTurn { payload: token, task: 1, attempt: 1, turn: 1, accepted },
        );
        assert_eq!(domain.held_bytes(), 9, "moving from slab into work keeps the charge");
        crate::route::route_into(&mut domain, &env, &mut decision);
        let expected = if accepted == tasks::Accepted::New { 9 } else { 0 };
        assert_eq!(domain.held_bytes(), expected);
        crate::decision::accept_routed(
            &mut domain.journal,
            &mut domain.core.counters,
            &env.limits.journal,
            decision,
            true,
        )
        .unwrap();
        let mut out = Queue::uncapped();
        crate::release(&mut domain, &env, &mut out);
        assert_eq!(domain.held_bytes(), 0);
    }
}

#[test]
fn stray_and_stopping_worker_turns_release_their_bodies() {
    for stopping in [false, true] {
        let (mut domain, env) = resume_fixture::root();
        if stopping {
            crate::step(&mut domain, &env, Event::Stop);
        }
        crate::try_step(
            &mut domain,
            &env,
            Event::Worker(crate::WorkerInput::Link {
                channel: Token::new(7),
                link: crate::Link::Up {
                    up: jig_domain_host::LinkUp::Turn {
                        run: Token::new(99),
                        attempt: Token::new(99),
                        turn: jig_domain_host::Turn {
                            turn: 1,
                            spent: 1,
                            read: None,
                            opens_window: true,
                            refused: Box::new([]),
                            body: b"stray turn".as_slice().into(),
                        },
                    },
                    parts: crate::Parts::none(),
                },
            }),
        )
        .unwrap();
        for _ in 0..4 {
            crate::step(&mut domain, &env, Event::Resume);
        }
        assert_eq!(domain.held_bytes(), 0, "fleet drops stray bodies, including while stopping");
    }
}
