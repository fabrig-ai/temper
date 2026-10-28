//! A hosted run's way through the root: its start, its agent's end, its
//! answer's items, and the worker's stop.

use super::fixtures::{ATTEMPT, RUN, assignment, assignment_of, grant, items, parked, turn};
use super::harness::{Harness, answered, operated};
use crate::{Event, Request, agent, host};
use alloc::boxed::Box;

/// The start smith's agent is sent, among `rows`.
fn sent_start(rows: &[Request]) -> &agent::Start {
    for row in rows {
        if let Request::Process(process) = row
            && let agent::process::Request::Send { message: agent::Down::Start { start, .. }, .. } = process.as_ref()
        {
            return start;
        }
    }
    panic!("smith's start is sent: {rows:?}")
}

#[test]
fn a_run_with_no_repository_starts_smith_with_no_workspace() {
    let mut h = Harness::new();
    let assign = Event::Assign { assignment: assignment(true, Box::new([])), items: Ok(items(&[])) };
    let rows = h.run(assign);
    let start = sent_start(&rows);
    assert_eq!((start.workspace, start.directories.len()), (None, 0));
}

#[test]
fn only_the_llm_accounts_grants_reach_smith() {
    for (form, expected_form) in [
        (host::Form::Credential(host::Scheme::Bearer), agent::Form::Credential(agent::Scheme::Bearer)),
        (host::Form::Credential(host::Scheme::Key), agent::Form::Credential(agent::Scheme::Key)),
        (
            host::Form::Program(host::SourceName::new(b"claude").unwrap()),
            agent::Form::Program(agent::SourceName::new(b"claude").unwrap()),
        ),
    ] {
        let mut h = Harness::new();
        let mut llm = grant(host::GrantTable::Accounts, 1);
        llm.form = form;
        let grants = Box::new([llm, grant(host::GrantTable::Application, 7)]);
        let assign = Event::Assign { assignment: assignment(true, grants), items: Ok(items(&[])) };
        let rows = h.run(assign);
        let expected =
            agent::Grant { account: llm.account, generation: llm.generation, valid: llm.valid, form: expected_form };
        assert_eq!(&*sent_start(&rows).grants, [expected]);
    }
}

#[test]
fn a_program_grant_renewal_reaches_smith_through_the_hub() {
    let mut h = Harness::new();
    let mut grant = grant(host::GrantTable::Name, 1);
    let rows = h.run(Event::Link(host::LinkDown::Assign { assignment: assignment(false, Box::new([grant])) }));
    let mut expected = agent::Grant {
        account: grant.account,
        generation: 1,
        valid: grant.valid,
        form: agent::Form::Program(agent::SourceName::new(b"alice").unwrap()),
    };
    assert_eq!(&*sent_start(&rows).grants, [expected]);
    h.say(agent::Up::Admitted);
    grant.generation = 2;
    let rows = h.run(Event::Link(host::LinkDown::Grant { run: RUN, attempt: ATTEMPT, grant }));
    let mut sent = None;
    for row in &rows {
        if let Request::Process(process) = row
            && let agent::process::Request::Send { message: agent::Down::Grant { grant }, .. } = process.as_ref()
        {
            assert!(sent.replace(*grant).is_none(), "one renewal");
        }
    }
    expected.generation = 2;
    assert_eq!(sent, Some(expected));
}

#[test]
fn an_agent_that_fails_before_its_run_is_admitted_ends_a_cancelled_start() {
    let mut h = Harness::new();
    h.run(Event::Link(host::LinkDown::Assign { assignment: assignment(false, Box::new([])) }));
    h.run(Event::Link(host::LinkDown::Cancel { run: RUN, attempt: ATTEMPT }));
    let rows = h.gone();
    let cancelled = host::Failure::Cancelled(host::Reason::Engine);
    let ending =
        host::Ending::Failed { failure: cancelled, detail: Box::new([]), work: host::Work { left: None, saved: None } };
    assert!(answered(&rows, &ending), "{rows:?}");
}

#[test]
fn an_agents_end_waits_behind_its_hand_outs() {
    let mut h = Harness::new();
    h.started();
    h.hold = true;
    h.say(parked(0));
    let rows = h.gone();
    let owner = h.owner.expect("a hosted run");
    let hosting = h.domain.host().hosting(owner).expect("a hosted run");
    assert_eq!(hosting.phase, host::Phase::Active, "{rows:?}");
    let ending = host::Ending::Parked { work: host::Work { left: None, saved: None } };
    assert!(answered(&h.release(), &ending));
}

#[test]
fn a_worker_shutting_down_waits_for_the_engine_to_have_its_turns() {
    let mut h = Harness::new();
    h.started();
    h.say(agent::Up::Turn { turn: turn(1) });
    h.say(parked(1));
    h.gone();
    h.run(Event::Link(host::LinkDown::Acknowledge { run: RUN, attempt: ATTEMPT }));
    assert!(!h.run(Event::Shutdown).contains(&Request::Stop));
    let rows = h.run(Event::Link(host::LinkDown::TurnKept { run: RUN, attempt: ATTEMPT, turn: 1 }));
    assert!(rows.contains(&Request::Stop));
}

#[test]
fn assignments_the_hub_refuses_leave_room_for_one_in_the_same_iteration() {
    let mut h = Harness::new();
    for run in 1..=4 {
        let rows = h.unreclaimed(assignment_of(run, &[b"app"], 3));
        let [Request::Answer { answer, .. }] = &*rows else { panic!("one answer: {rows:?}") };
        assert_eq!(answer.ending, host::Ending::Refused(host::Refusal::Invalid(host::Invalid::Messages)));
    }
    assert!(operated(&h.unreclaimed(assignment_of(5, &[b"app"], 0))), "the fifth is prepared");
}

#[test]
fn acknowledged_answers_free_their_workspace_items() {
    let mut h = Harness::new();
    let unprepared = host::Failure::Unprepared(host::Preparation::Transient);
    let ending = host::Ending::Failed {
        failure: unprepared,
        detail: Box::new([]),
        work: host::Work { left: None, saved: None },
    };
    for run in 1..=4 {
        assert!(answered(&h.run(assignment_of(run, &[b"app"], 0)), &ending), "run {run} is hosted, not refused");
        h.run(Event::Link(host::LinkDown::Acknowledge { run: skein_lib::Token::new(run), attempt: ATTEMPT }));
    }
}

#[test]
fn notices_keep_the_source_and_reset_and_do_not_replace_the_terminal() {
    use skein_lib::Duration;
    const SOURCE: u32 = 17;
    for notice in [
        agent::Up::Unauthorized { account: SOURCE, generation: 1 },
        agent::Up::Limited { account: SOURCE, reset: None },
        agent::Up::Limited { account: SOURCE, reset: Some(Duration::ZERO) },
        agent::Up::Limited { account: SOURCE, reset: Some(Duration::from_secs(9)) },
        agent::Up::Refused {
            account: SOURCE,
            code: b"quota".as_slice().into(),
            reason: b"provider refused".as_slice().into(),
        },
    ] {
        let expected = match &notice {
            agent::Up::Unauthorized { account, generation } => {
                host::LinkUp::Unauthorized { run: RUN, attempt: ATTEMPT, account: *account, generation: *generation }
            }
            agent::Up::Limited { account, reset } => {
                host::LinkUp::Limited { run: RUN, attempt: ATTEMPT, account: *account, reset: *reset }
            }
            agent::Up::Refused { account, code, reason } => host::LinkUp::AccountRefused {
                run: RUN,
                attempt: ATTEMPT,
                account: *account,
                code: code.clone(),
                reason: reason.clone(),
            },
            agent::Up::Admitted
            | agent::Up::MessageRefused { .. }
            | agent::Up::Call { .. }
            | agent::Up::Withdraw { .. }
            | agent::Up::Turn { .. }
            | agent::Up::Fact { .. }
            | agent::Up::Long { .. }
            | agent::Up::LongDone
            | agent::Up::Waiting { .. }
            | agent::Up::Answer { .. } => unreachable!(),
        };
        let mut h = Harness::new();
        h.run(Event::Link(host::LinkDown::Assign {
            assignment: assignment(false, Box::new([grant(host::GrantTable::Accounts, SOURCE)])),
        }));
        h.say(agent::Up::Admitted);
        let again = match &notice {
            agent::Up::Unauthorized { account, generation } => {
                agent::Up::Unauthorized { account: *account, generation: *generation }
            }
            agent::Up::Limited { account, reset } => agent::Up::Limited { account: *account, reset: *reset },
            agent::Up::Refused { account, code, reason } => {
                agent::Up::Refused { account: *account, code: code.clone(), reason: reason.clone() }
            }
            agent::Up::Admitted
            | agent::Up::MessageRefused { .. }
            | agent::Up::Call { .. }
            | agent::Up::Withdraw { .. }
            | agent::Up::Turn { .. }
            | agent::Up::Fact { .. }
            | agent::Up::Long { .. }
            | agent::Up::LongDone
            | agent::Up::Waiting { .. }
            | agent::Up::Answer { .. } => unreachable!(),
        };
        assert!(h.say(again).contains(&Request::Link(expected)), "{notice:?}");
        h.say(parked(0));
        let ended = h.gone();
        assert!(answered(&ended, &host::Ending::Parked { work: host::Work { left: None, saved: None } }));
        let stale = h.run(Event::Process(Box::new(agent::process::Event::Received {
            owner: h.agent.unwrap(),
            message: notice,
        })));
        no_notices(&stale);
        h.run(Event::Link(host::LinkDown::Acknowledge { run: RUN, attempt: ATTEMPT }));
    }
}

#[test]
fn a_terminal_authentication_failure_does_not_synthesize_an_account_notice() {
    let mut h = Harness::new();
    h.started();
    let rows = h.say(agent::Up::Answer {
        answer: agent::Answer {
            read: None,
            turns: 0,
            spent: 0,
            result: agent::RunResult::Failed {
                failure: agent::RunFailure::Model(agent::ModelFault::Unauthorized { account: 17 }),
            },
        },
    });
    no_notices(&rows);
    let ended = h.gone();
    assert!(answered(
        &ended,
        &host::Ending::Failed {
            failure: host::Failure::Run(host::RunFailure::Model),
            detail: Box::default(),
            work: host::Work { left: None, saved: None }
        }
    ));
    no_notices(&ended);
}

fn no_notices(rows: &[Request]) {
    for row in rows {
        let Request::Link(up) = row else { continue };
        match up {
            host::LinkUp::Unauthorized { .. } | host::LinkUp::Limited { .. } | host::LinkUp::AccountRefused { .. } => {
                panic!("unexpected notice: {up:?}")
            }
            host::LinkUp::Logins { .. }
            | host::LinkUp::SignedOut { .. }
            | host::LinkUp::Turn { .. }
            | host::LinkUp::Call { .. }
            | host::LinkUp::Piece { .. }
            | host::LinkUp::Activity { .. }
            | host::LinkUp::Answer { .. } => {}
        }
    }
}

#[test]
fn earlier_host_answers_take_the_same_agents_reply_bound() {
    for error in [false, true] {
        for extra in [0, 1] {
            let mut h = Harness::new();
            let (owner, source) = h.decode(Box::new([]));
            let mut start = super::fixtures::start(RUN, &source);
            let bytes = h.env.limits.reply_bytes + extra;
            let mut body = skein_lib::List::with_capacity(bytes);
            for _ in 0..bytes {
                body.push(b'x').expect("room for each byte");
            }
            let body = body.into_boxed();
            start.answered = Box::new([agent::AnsweredCall {
                name: agent::CallName { activation: ATTEMPT.raw(), completion: 1, position: 0 },
                tool: Box::from(&b"recall"[..]),
                reply: agent::SavedReply::Host { error, body: body.clone() },
            }]);
            let rows = h.run(Event::Decoded { agent: owner, start: Ok(Box::new(start)) });
            let [call] = sent_start(&rows).answered.as_ref() else { panic!("one earlier answer") };
            assert_eq!(call.tool.as_ref(), b"recall");
            assert_eq!(call.name, agent::CallName { activation: ATTEMPT.raw(), completion: 1, position: 0 });
            let expected =
                if extra == 0 { agent::SavedReply::Host { error, body } } else { agent::SavedReply::TooLarge };
            assert_eq!(call.reply, expected);
        }
    }
}
