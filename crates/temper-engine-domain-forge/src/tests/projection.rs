use super::*;
use skein_forge::api::{Answer, Kind, Op, Read, State, Summary, Write};

const GOAL: u64 = 7;
const ISSUE: u64 = 1;
const LOST: u64 = 40;
const NEXT: u64 = LOST + 1;
const FINISHED: u64 = NEXT + 1;
const CLOSE: u64 = FINISHED + 1;

fn requests(d: &mut Domain, environment: &Env<Limits>, event: Option<Event>) -> Box<[Request]> {
    let mut out = Queue::with_capacity(max_out(&environment.limits));
    match event {
        Some(event) => step(d, environment, event, &mut out),
        None => resume(d, environment, &mut out),
    }
    let mut kept = skein_lib::List::with_capacity(out.len());
    while let Some(request) = out.pop() {
        kept.push(request).expect("counted output");
    }
    kept.into_boxed()
}

fn call(d: &mut Domain, environment: &Env<Limits>) -> (Token, Op) {
    for request in requests(d, environment, None) {
        if let Request::Call { call, op, .. } = request {
            return (call, op);
        }
    }
    panic!("one forge call")
}

fn answer(d: &mut Domain, environment: &Env<Limits>, call: Token, value: Answer) -> Box<[Request]> {
    let out = requests(
        d,
        environment,
        Some(Event::Client(skein_forge_client::Event::Answered {
            free: 0,
            rate: None,
            call,
            cost: 1,
            result: Ok(value),
        })),
    );
    d.reclaim();
    out
}

fn keep_projection(d: &mut Domain, environment: &Env<Limits>, number: u64) {
    let out = requests(d, environment, Some(Event::ProjectDesired { entry: number, goal: GOAL }));
    for request in out {
        if let Request::ProjectionEffect { row, entry, .. } = request {
            requests(d, environment, Some(Event::KeepProjection { row, entry }));
            requests(d, environment, Some(Event::Committed { entry: number }));
            return;
        }
    }
    panic!("one next projection effect")
}

fn write(d: &mut Domain, environment: &Env<Limits>) -> (Token, Write) {
    let (clock, op) = call(d, environment);
    let Op::Read(Read::Items { .. }) = op else { panic!("entry clock: {op:?}") };
    answer(d, environment, clock, Answer::Items { items: Box::new([]), more: false, now: environment.now });
    let (token, op) = call(d, environment);
    let Op::Write(write) = op else { panic!("projection write: {op:?}") };
    (token, write)
}

fn missing_comment() -> Answer {
    Answer::Item {
        item: Summary {
            labels: Box::new([]),
            number: ISSUE,
            kind: Kind::Issue,
            state: State::Open,
            author: 7,
            key: None,
            title: Box::from(b"Work".as_slice()),
            body: Box::new([]),
            updated: env().now,
        },
        comments: Box::new([]),
        more: false,
    }
}

fn goal_view() -> issues::GoalView {
    issues::GoalView {
        phase: issues::Phase::Done,
        goal: GOAL,
        repository: u64::from(REPO.repository),
        title: "Work".into(),
        goal_text: "Complete the work".into(),
        plan: Box::new([]),
        milestones: Box::new([
            issues::Milestone { key: issues::MilestoneKey::PlanAccepted, text: "Plan accepted".into() },
            issues::Milestone { key: issues::MilestoneKey::Report(1), text: "Next milestone".into() },
        ]),
        finished: Some("Goal finished".into()),
    }
}

fn started(view: issues::GoalView, state: issues::Projected, number: Option<u64>) -> Domain {
    let mut d = domain();
    outputs(&mut d, Event::Restore { record: Stored::Repository(adopted()) });
    outputs(
        &mut d,
        Event::Restore {
            record: Stored::Issue(IssueRow {
                goal: GOAL,
                repository: REPO,
                state,
                number,
                pending: None,
                before: None,
                failed: false,
                desired: Some(view),
            }),
        },
    );
    outputs(&mut d, Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic });
    outputs(&mut d, Event::ReadAfresh);
    outputs(&mut d, Event::SettleOutbox);
    d
}

fn restart_inflight(d: &mut Domain, environment: &mut Env<Limits>, number: u64) {
    let issue = d.issue(GOAL).expect("committed issue").clone();
    let entry = d.entries.get(&number).expect("write in flight").entry.clone();
    // Restart after the saved attempt's lifetime; the call reached no forge.
    environment.now = entry.attempt.expect("durable attempt").deadline;
    environment.wall = Wall::from_nanos(environment.now.as_nanos());
    *d = domain();
    for record in [Stored::Repository(adopted()), Stored::Issue(issue), Stored::Entry { entry, held: false }] {
        requests(d, environment, Some(Event::Restore { record }));
    }
    requests(d, environment, Some(Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic }));
    requests(d, environment, Some(Event::ReadAfresh));
    requests(d, environment, Some(Event::SettleOutbox));
}

#[test]
fn a_milestone_lost_to_a_crash_counts_as_posted_and_the_issue_closes() {
    let mut environment = env();
    let view = goal_view();
    let state = issues::project(&view, &issues::Projected::default(), environment.wall, &LIMITS.issue_policy).projected;
    let mut d = started(view, state, Some(ISSUE));
    keep_projection(&mut d, &environment, LOST);
    let (_, lost_write) = write(&mut d, &environment);
    restart_inflight(&mut d, &mut environment, LOST);
    let (find, op) = call(&mut d, &environment);
    assert_eq!(op, Op::Read(Read::Item { number: ISSUE, after: 0 }));
    let recovered = answer(&mut d, &environment, find, missing_comment());
    assert!(
        recovered.contains(&Request::ProjectionOutcome { entry: LOST, outcome: skein_forge_client::Outcome::Held })
    );
    assert!(!d.issue(GOAL).expect("lost milestone counted").failed);
    assert!(!d.entries.contains_key(&LOST), "the consumed loss frees its outbox room");
    // Every subsequent call is checked, so neither the lost comment nor any
    // other projection write can be sent twice unnoticed.
    for (number, body, comment) in [(NEXT, b"Next milestone".as_slice(), 2), (FINISHED, b"Goal finished".as_slice(), 3)]
    {
        keep_projection(&mut d, &environment, number);
        let (token, next_write) = write(&mut d, &environment);
        let Write::Post { number: issue, body: words, .. } = &next_write else { panic!("next comment") };
        assert_eq!((*issue, words.as_ref()), (ISSUE, body));
        assert_ne!(next_write, lost_write);
        answer(&mut d, &environment, token, Answer::Commented(comment));
    }
    keep_projection(&mut d, &environment, CLOSE);
    let (token, closing) = write(&mut d, &environment);
    assert_eq!(closing, Write::Close { number: ISSUE });
    answer(&mut d, &environment, token, Answer::Done);
    let settled = requests(&mut d, &environment, Some(Event::ProjectDesired { entry: CLOSE + 1, goal: GOAL }));
    assert!(settled.contains(&Request::ProjectionSettled { goal: GOAL }));
    let issue = d.issue(GOAL).expect("closed issue");
    assert!(issue.state.closed && !issue.failed && issue.pending.is_none());
    assert_eq!(
        issue.state.milestones.as_ref(),
        &[issues::MilestoneKey::PlanAccepted, issues::MilestoneKey::Report(1), issues::MilestoneKey::Finished,]
    );
}

#[test]
fn a_lost_open_stays_failed_without_ever_opening_again() {
    let mut environment = env();
    let mut view = goal_view();
    view.phase = issues::Phase::Active;
    view.finished = None;
    let mut d = started(view.clone(), issues::Projected::default(), None);
    keep_projection(&mut d, &environment, LOST);
    let (_, opening) = write(&mut d, &environment);
    let Write::CreateIssue { .. } = opening else { panic!("opening write") };
    restart_inflight(&mut d, &mut environment, LOST);
    let (find, op) = call(&mut d, &environment);
    let Op::Read(Read::Items { kind, .. }) = op else { panic!("issue find") };
    assert_eq!(kind, Some(Kind::Issue));
    let recovered =
        answer(&mut d, &environment, find, Answer::Items { items: Box::new([]), more: false, now: environment.now });
    assert!(recovered.contains(&Request::ProjectionFailed { goal: GOAL }));
    let after_loss = d.issue(GOAL).expect("failed Open").clone();
    // The closing task may settle its held outbox entry before the final
    // feed. Only the existing issue row remains across this second restart.
    requests(&mut d, &environment, Some(Event::SettleEffects { task: GOAL, root: GOAL, ending: ReleaseEnding::Done }));
    d = domain();
    for record in [Stored::Repository(adopted()), Stored::Issue(after_loss)] {
        requests(&mut d, &environment, Some(Event::Restore { record }));
    }
    requests(&mut d, &environment, Some(Event::Restored { clock: skein_forge_client::RecoveryClock::Monotonic }));
    requests(&mut d, &environment, Some(Event::ReadAfresh));
    requests(&mut d, &environment, Some(Event::SettleOutbox));
    view.goal_text = "Changed facts must not resend a lost Open".into();
    let retried =
        requests(&mut d, &environment, Some(Event::Project { entry: NEXT, repository: REPO, view: view.clone() }));
    assert!(!has_projection_effect(&retried), "a lost Open is final even when facts change");
    view.phase = issues::Phase::Done;
    view.finished = Some("Goal finished".into());
    let closing = requests(&mut d, &environment, Some(Event::Project { entry: FINISHED, repository: REPO, view }));
    assert!(closing.contains(&Request::ProjectionSettled { goal: GOAL }));
    let issue = d.issue(GOAL).expect("retained failed Open");
    assert!(issue.failed && issue.number.is_none() && issue.pending.is_none());
}
