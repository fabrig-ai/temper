//! The public admission door and the real Held -> ToChild -> Released consumer.
use super::resume_fixture;
use crate::{Domain, Event, Family, Limits, Record, Request, StoreInput, Write};
use jig_domain_core::{
    CoreRecord, ProjectionFailure, ProjectionFailureSource, ProjectionFailureState, ProjectionGoal, ProjectionRecord,
    ProjectionRecovery,
};
use skein_lib::{Env, Queue, Time, Token, Wall};
use temper_engine_domain_forge as forge;

fn fixture() -> (Domain, Env<Limits>) {
    let (mut domain, env) = resume_fixture::root();
    crate::fresh(&mut domain.core.counters, Family::Task).unwrap();
    crate::fresh(&mut domain.core.counters, Family::ConnectorRow).unwrap();
    assert_eq!(
        domain.core.restore_core(
            CoreRecord::Projection(ProjectionRecord::Goal {
                goal: ProjectionGoal {
                    number: 1,
                    project: 1,
                    priority: 1,
                    words: b"goal".as_slice().into(),
                    phase: jig_domain_tasks::Phase::Waiting
                },
                pending: Box::new([]),
                changed: false,
                fed_at: None,
                closing_at: None,
            }),
            &crate::core_limits(&env.limits)
        ),
        jig_domain_core::Restored::Live
    );
    assert_eq!(
        domain.core.restore_core(
            CoreRecord::Projection(ProjectionRecord::Failure(ProjectionFailure {
                goal: 1,
                connector: domain.config.forge_connector,
                after: 0,
                state: ProjectionFailureState::Reported(ProjectionFailureSource { source: 1, at: Wall::EPOCH }),
            })),
            &crate::core_limits(&env.limits)
        ),
        jig_domain_core::Restored::Live
    );
    (domain, env)
}

#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "this bounded recovery script must produce only commits and child continuations"
)]
fn consume(domain: &mut Domain, env: &Env<Limits>) -> Vec<Write> {
    let mut writes = Vec::new();
    let mut out = Queue::with_capacity(crate::max_out(&env.limits));
    for _ in 0..32 {
        crate::release(domain, env, &mut out);
        if out.is_empty() {
            if domain.journal.idle() && !domain.door_pass {
                return writes;
            }
            continue;
        }
        while let Some(request) = out.pop() {
            match request {
                Request::Store(crate::StoreRequest::Commit { number, writes: rows }) => {
                    writes.extend(rows.into_vec());
                    crate::try_step(domain, env, Event::Store(StoreInput::Committed { number, fill: 0 })).unwrap();
                }
                Request::ToChild(released) => crate::try_step(domain, env, Event::Released(released)).unwrap(),
                other => panic!("unexpected recovery output: {other:?}"),
            }
        }
    }
    panic!("recovery consumer did not drain");
}

#[test]
fn projection_recovery_returns_competing_ownership_and_consumes_each_real_terminal_once() {
    let (mut domain, env) = fixture();
    let owner = Token::new(900);
    domain.recover_projection(&env, 1, owner).unwrap();
    let after_stage = domain.deployment();
    assert_eq!(domain.recover_projection(&env, 1, Token::new(901)), Err(Token::new(901)));
    assert_eq!(domain.deployment(), after_stage, "refusal has zero core mutation");
    assert_eq!(domain.take_projection_recovery(owner), None, "Held has not reached Released yet");
    let writes = consume(&mut domain, &env);
    assert!(writes.iter().any(|write| matches!(write, Write::Save(Record::Core(jig_domain_core::Record::Core(CoreRecord::Projection(ProjectionRecord::Failure(row))))) if row.after == 1 && row.state == ProjectionFailureState::Armed)));
    assert_eq!(domain.take_projection_recovery(Token::new(901)), None);
    assert_eq!(domain.take_projection_recovery(owner), Some(ProjectionRecovery::Recovered { after: 1 }));
    assert_eq!(domain.take_projection_recovery(owner), None);
    assert_eq!(domain.forge.next_wall_deadline(), Some(Wall::EPOCH), "consumption preserves the reset retry");
    domain.recover_projection(&env, 1, Token::new(901)).unwrap();
    assert!(consume(&mut domain, &env).is_empty(), "Armed returns its old fence without a reset or write");
    assert_eq!(domain.take_projection_recovery(Token::new(901)), Some(ProjectionRecovery::Recovered { after: 1 }));
    // Missing goals still reserve the same bounded slot for their keyed terminal.
    for index in 1..env.limits.forge.issues {
        domain.recover_projection(&env, 100 + u64::from(index), Token::new(1000 + u64::from(index))).unwrap();
        assert!(consume(&mut domain, &env).is_empty());
    }
    let before_full = domain.deployment();
    assert_eq!(domain.recover_projection(&env, 999, Token::new(9999)), Err(Token::new(9999)));
    assert_eq!(domain.deployment(), before_full);
    for index in 1..env.limits.forge.issues {
        let owner = Token::new(1000 + u64::from(index));
        assert_eq!(domain.take_projection_recovery(owner), Some(ProjectionRecovery::Missing));
        assert_eq!(domain.take_projection_recovery(owner), None);
    }
    domain.recover_projection(&env, 999, Token::new(9999)).unwrap();
    assert!(consume(&mut domain, &env).is_empty());
    assert_eq!(domain.take_projection_recovery(Token::new(9999)), Some(ProjectionRecovery::Missing));
}

fn restore(domain: &mut Domain, env: &Env<Limits>, record: forge::Stored) {
    let mut out = Queue::with_capacity(forge::max_out(&env.limits.forge));
    forge::step(
        &mut domain.forge,
        &Env { now: env.now, wall: env.wall, limits: env.limits.forge },
        forge::Event::Restore { record },
        &mut out,
    );
    assert!(out.is_empty());
}

#[test]
fn projection_recovery_busy_is_unchanged_then_the_same_request_resets_a_retained_held_entry() {
    let (mut domain, env) = fixture();
    let repository = skein_forge::api::Repository { forge: 1, repository: 2 };
    let entry = skein_forge_client::Entry {
        number: 1,
        task: 1,
        repository,
        effect: skein_forge_client::Effect {
            write: skein_forge::api::Write::Close { number: 1 },
            condition: skein_forge_client::Condition::None,
        },
        start: None,
        attempt: None,
        failures: 0,
        queued: false,
    };
    restore(&mut domain, &env, forge::Stored::Entry { entry: entry.clone(), held: false });
    let issue = forge::IssueRow {
        goal: 1,
        repository,
        state: temper_engine_domain_forge_issues::Projected::default(),
        number: Some(1),
        pending: Some(1),
        before: Some(temper_engine_domain_forge_issues::Projected::default()),
        failed: true,
        desired: None,
        closing_generation: None,
    };
    restore(&mut domain, &env, forge::Stored::Issue(issue.clone()));
    let owner = Token::new(902);
    let before = domain.deployment();
    domain.recover_projection(&env, 1, owner).unwrap();
    assert_eq!(domain.take_projection_recovery(owner), Some(ProjectionRecovery::Busy));
    assert_eq!(domain.take_projection_recovery(owner), None);
    assert_eq!(domain.deployment(), before);
    assert_eq!(domain.forge.issue(1), Some(&issue));
    assert!(consume(&mut domain, &env).is_empty());
    restore(&mut domain, &env, forge::Stored::Entry { entry, held: true });
    let deadline = Wall::from_nanos(123);
    domain.forge.projection_after(1, deadline);
    domain.recover_projection(&env, 1, owner).unwrap();
    let writes = consume(&mut domain, &env);
    assert!(writes.iter().any(|write| matches!(write, Write::Erase(crate::Key::Forge(forge::Key::Entry(1))))));
    let issue = domain.forge.issue(1).unwrap();
    assert_eq!(issue.pending, None);
    assert_eq!(issue.before, None);
    assert!(!issue.failed);
    assert_eq!(domain.take_projection_recovery(owner), Some(ProjectionRecovery::Recovered { after: 1 }));
    assert_eq!(domain.take_projection_recovery(owner), None);
    assert_eq!(domain.forge.next_wall_deadline(), Some(deadline));
    assert_eq!(env.now, Time::ZERO);
}
