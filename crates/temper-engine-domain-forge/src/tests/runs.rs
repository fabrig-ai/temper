use crate::runs;
use alloc::boxed::Box;
use skein_forge::api as a;
use skein_lib::{Duration, Time};
use temper_engine_domain_forge_landing as policy;
const COMMIT: a::Commit = [3; 32];
fn run() -> a::Run {
    a::Run {
        id: 7,
        number: 8,
        workflow: Box::from(b"gate.yml".as_slice()),
        commit: COMMIT,
        event: a::Event::Dispatch,
        actor: 1,
        state: a::RunState::Completed,
        conclusion: Some(a::Conclusion::Success),
        url: Box::from(b"https://forge/run/7".as_slice()),
        created: Time::from_nanos(20),
        jobs: Box::new([]),
    }
}
pub(super) fn follow() -> runs::Follow {
    let mut follow = runs::follow(
        61,
        9,
        Box::new([6; 18]),
        super::REPO,
        Box::from(b"gate.yml".as_slice()),
        Box::from(b"temper/51".as_slice()),
        COMMIT,
        Box::new([]),
    );
    follow.writes = Box::new([]);
    follow.start = Some(Time::from_nanos(10));
    follow.since = Some(Time::from_nanos(10));
    follow
}
#[test]
fn only_our_dispatch_at_the_commit_after_start_is_followed() {
    let mut follow = follow();
    let valid = run();
    let mut other_actor = valid.clone();
    other_actor.actor = 2;
    let mut other_event = valid.clone();
    other_event.event = a::Event::Other;
    let mut other_commit = valid.clone();
    other_commit.commit = [4; 32];
    let mut old = valid.clone();
    old.created = Time::from_nanos(9);
    assert!(
        follow
            .answered(
                Ok(a::Answer::Runs {
                    runs: Box::new([other_actor, other_event, other_commit, old, valid]),
                    more: false
                }),
                1,
                Time::from_nanos(20),
                Duration::from_secs(10)
            )
            .is_none()
    );
    assert_eq!(follow.read(), Some(a::Read::Run { run: 7 }));
    let ended =
        follow.answered(Ok(a::Answer::Run(run())), 1, Time::from_nanos(20), Duration::from_secs(10)).expect("one end");
    assert_eq!(ended.outcome, policy::Verdict::Passed);
    assert!(follow.answered(Ok(a::Answer::Run(run())), 1, Time::from_nanos(20), Duration::from_secs(10)).is_none());
}
#[test]
fn a_missing_run_is_lost_once_at_its_local_lifetime() {
    let mut follow = follow();
    follow.start = Some(Time::from_nanos(1_000));
    let empty = a::Answer::Runs { runs: Box::new([]), more: false };
    assert!(follow.answered(Ok(empty.clone()), 1, Time::from_nanos(19), Duration::from_nanos(10)).is_none());
    assert_eq!(
        follow.answered(Ok(empty.clone()), 1, Time::from_nanos(20), Duration::from_nanos(10)).expect("lost").outcome,
        policy::Verdict::Fault(policy::Fault::Lost)
    );
    assert!(follow.answered(Ok(empty), 1, Time::from_nanos(21), Duration::from_nanos(10)).is_none());
}
#[test]
fn a_job_fault_reruns_each_failed_attempt_and_clears_its_old_verdict() {
    let mut follow = follow();
    let mut run = run();
    run.conclusion = Some(a::Conclusion::Failure);
    let job = a::RunJob {
        id: 11,
        name: Box::from(b"test".as_slice()),
        attempt: 2,
        state: a::RunState::Completed,
        conclusion: Some(a::Conclusion::Failure),
        step: Some(a::Step { number: 0, name: Box::from(b"Set up".as_slice()) }),
        steps: 3,
    };
    let mut second = job.clone();
    second.id = 12;
    run.jobs = Box::new([job, second]);
    follow.outcome = Some(runs::verdict(&run, COMMIT));
    follow.run = Some(run);
    runs::retry(&mut follow, 2);
    assert_eq!(follow.outcome, None);
    assert_eq!(
        follow.writes.as_ref(),
        &[
            a::Write::Rerun { attempt: a::JobAttempt { head: COMMIT, run: 7, job: 11, attempt: 2 } },
            a::Write::Rerun { attempt: a::JobAttempt { head: COMMIT, run: 7, job: 12, attempt: 2 } }
        ]
    );
    assert!(follow.write(10).is_some());
    assert_eq!(follow.read(), None);
}
