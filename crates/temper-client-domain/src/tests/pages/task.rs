use super::fixtures::{self, chat as f};
use crate::pages::{
    Request,
    task::{Shape, Task},
};
use jig_client::{Event, Readable, Told, Watchable, contract as c};
use skein_lib::{Queue, Time};
const OUTPUTS: u32 = 16;
const WINDOW: u32 = 2;
const EMPTY_OFFSET: u32 = 0;
const FIRST: u64 = f::FIRST;
const PRIORITY: u32 = 1;
fn opened() -> (Task, f::Fixture) {
    let mut fixture = f::opened();
    let mut page = Task::new(f::TASK, FIRST, &fixtures::limits(), &mut Queue::with_capacity(OUTPUTS));
    while fixture.out.pop().is_some() {}
    page.told(Time::ZERO, &fixture.client, Told::Changed { handle: page.document }, &mut fixture.out);
    (page, fixture)
}
#[test]
fn opening_a_task_watches_task_and_tree_and_reads_activity() {
    let mut out = Queue::with_capacity(OUTPUTS);
    let page = Task::new(f::TASK, FIRST, &fixtures::limits(), &mut out);
    assert_eq!(
        out.pop(),
        Some(Request::Client(Event::Watch {
            handle: page.document,
            subject: Watchable::Jig(c::Subject::Task { task: f::TASK }),
        }))
    );
    assert_eq!(
        out.pop(),
        Some(Request::Client(Event::Watch {
            handle: page.tree,
            subject: Watchable::Jig(c::Subject::Tree { task: f::TASK, window: WINDOW }),
        }))
    );
    assert_eq!(
        out.pop(),
        Some(Request::Client(Event::Read {
            handle: page.activity,
            read: Readable::Jig(c::Read::Activity { task: f::TASK, before: None }),
        }))
    );
    assert!(out.is_empty());
}
#[test]
fn a_live_run_on_a_tasks_page_is_watched() {
    let (page, mut f) = opened();
    assert_eq!(
        f.out.pop(),
        Some(Request::Client(Event::Watch {
            handle: page.run.expect("run handle"),
            subject: Watchable::Jig(c::Subject::Run { task: f::TASK, turn: None, offset: EMPTY_OFFSET }),
        }))
    );
}
#[test]
fn a_tasks_live_run_is_unwatched_when_it_ends() {
    let (mut page, mut f) = opened();
    let run = page.run.expect("live run");
    while f.out.pop().is_some() {}
    let mut task = f::task();
    f::phase(&mut task, c::OwnerChipPhase::Active(c::RunStage::Idle));
    f::deliver(&mut f.client, f.watch, task);
    page.told(Time::ZERO, &f.client, Told::Changed { handle: page.document }, &mut f.out);
    assert_eq!(page.run, None);
    assert_eq!(f::handles(&mut f.out).as_ref(), [run]);
}
#[test]
fn leaving_a_task_unwatches_each_of_its_handles() {
    let (page, mut f) = opened();
    while f.out.pop().is_some() {}
    page.leave(&mut f.out);
    assert_eq!(f::handles(&mut f.out).as_ref(), [page.document, page.tree, page.activity, page.run.expect("run")]);
}
#[test]
fn an_agents_task_without_a_priority_has_the_agent_shape() {
    const CHARTER: u32 = 1;
    const PARTY: u64 = 73;
    const PROCEDURE: u32 = 2;
    const CASES: &[(Option<u32>, c::Executor, Shape)] = &[
        (Some(PRIORITY), c::Executor::Agent { charter: CHARTER }, Shape::Tracked),
        (None, c::Executor::Agent { charter: CHARTER }, Shape::Agent),
        (Some(PRIORITY), c::Executor::Procedure { connector: fixtures::FORGE, kind: PROCEDURE }, Shape::HeaderAndPlan),
        (Some(PRIORITY), c::Executor::Party { party: PARTY }, Shape::HeaderAndPlan),
        (None, c::Executor::Procedure { connector: fixtures::FORGE, kind: PROCEDURE }, Shape::HeaderAndPlan),
        (None, c::Executor::Party { party: PARTY }, Shape::HeaderAndPlan),
        (None, c::Executor::Role { project: fixtures::PROJECT, role: 1 }, Shape::HeaderAndPlan),
    ];
    for (priority, executor, expected) in CASES {
        let (mut page, mut f) = opened();
        let mut task = f::task();
        task.priority = *priority;
        task.chip.executor = *executor;
        f::deliver(&mut f.client, f.watch, task);
        page.told(Time::ZERO, &f.client, Told::Changed { handle: page.document }, &mut f.out);
        assert_eq!(page.shape, *expected, "{priority:?}, {executor:?}");
    }
}
const HELD: u64 = 12;
const FIRST_CARD: u32 = 0;
#[test]
fn releasing_from_the_plan_names_the_holds_revision_the_card_showed() {
    use crate::pages::Act;
    let mut pages = fixtures::task::held_pages();
    let mut out = fixtures::acts::act(
        &mut pages,
        Act::Decide {
            object: c::ObjectKey::Task(HELD),
            revision: Some(fixtures::acts::REVISION),
            act: c::ActKind::Release,
        },
    );
    assert!(fixtures::acts::sent(&mut out).is_empty(), "release waits for confirmation");
    let mut out = fixtures::acts::act(&mut pages, Act::Confirm);
    assert_eq!(
        fixtures::acts::sent(&mut out).as_ref(),
        [c::Ask::Release { project: fixtures::PROJECT, task: HELD, revision: fixtures::acts::REVISION, again: None }]
    );
    assert!(fixtures::acts::sent(&mut fixtures::acts::act(&mut pages, Act::Confirm)).is_empty());
}
#[test]
fn a_task_held_anew_reads_its_released_card_again() {
    let (mut page, mut fixture) = opened();
    let mut task = f::task();
    let mut held = |revision| {
        f::phase(&mut task, c::OwnerChipPhase::Held { reason: c::HoldCategory::Stopped, revision });
        task.chip.clone()
    };
    page.held(&held(fixtures::acts::REVISION.value), &mut fixture.out);
    let card = page.cards.get_mut(FIRST_CARD).expect("held card");
    let first = card.handle;
    let c::ReadValue::Escalation(read) = fixtures::acts::held(c::ActKind::Release) else { panic!("hold") };
    let mut stopped = read.hold;
    stopped.revision = fixtures::acts::REVISION;
    let mut document = f::task();
    document.hold = Some(c::TaskHold::Document(Box::new(stopped)));
    card.page = Some(c::ReadValue::Task(Box::new(document)));
    card.outcome = Some(jig_client::Outcome::Unknown);
    while fixture.out.pop().is_some() {}
    page.held(&held(fixtures::acts::REVISION.value), &mut fixture.out);
    assert!(fixture.out.pop().is_none(), "the same hold keeps its card");
    page.held(&held(fixtures::acts::NEW_REVISION.value), &mut fixture.out);
    let card = page.cards.get(FIRST_CARD).expect("held card");
    assert_eq!(page.cards.len(), 1);
    assert_ne!(card.handle, first);
    assert!(card.page.is_none() && card.outcome.is_none());
    assert_eq!(fixture.out.pop(), Some(Request::Client(Event::Unwatch { handle: first })),);
}
#[test]
fn a_moved_hold_is_refused_and_the_card_asks_to_look_again() {
    use crate::pages::Act;
    let mut pages = fixtures::task::held_pages();
    fixtures::acts::act(
        &mut pages,
        Act::Decide {
            object: c::ObjectKey::Task(HELD),
            revision: Some(fixtures::acts::REVISION),
            act: c::ActKind::Release,
        },
    );
    fixtures::acts::act(&mut pages, Act::Confirm);
    let key = pages.cards().expect("cards").get(FIRST_CARD).expect("held").key.expect("sent");
    let outcome = jig_client::Outcome::Answered(jig_client::Answered::Jig(c::Answer::Refused {
        refusal: c::Refusal::Moved { now: fixtures::acts::NEW_REVISION, what: c::Moved::Hold { reason: None } },
        at: None,
    }));
    crate::acts::told(&mut pages, &Told::Answered { key, outcome: outcome.clone() });
    assert_eq!(pages.cards().expect("cards").get(FIRST_CARD).expect("held").outcome, Some(outcome));
    assert!(fixtures::acts::sent(&mut fixtures::acts::act(&mut pages, Act::Confirm)).is_empty());
}

use crate::pages::task::{State, header_state, link_state, row_state};
use skein_lib::{List, Wall};
const DEPENDENCY: u64 = 17;
const OTHER_DEPENDENCY: u64 = 23;
#[test]
fn a_tasks_state_says_what_it_waits_for() {
    let cases: &[(&[u64], bool, bool, State)] = &[
        (&[DEPENDENCY], false, false, State::WaitingFor(DEPENDENCY)),
        (&[DEPENDENCY], false, true, State::WaitingFor(DEPENDENCY)),
        (&[DEPENDENCY, OTHER_DEPENDENCY], false, false, State::WaitingForTasks { count: 2, more: false }),
        (&[DEPENDENCY], true, true, State::WaitingForTasks { count: 1, more: true }),
        (&[], true, true, State::WaitingForTasks { count: 0, more: true }),
        (&[], false, true, State::WaitingForHoldings),
        (&[], false, false, State::Phase(c::Phase::Waiting)),
    ];
    for (numbers, more, holds, expected) in cases {
        let mut task = f::task();
        let mut dependencies = List::with_capacity(WINDOW);
        for number in *numbers {
            dependencies.push(*number).expect("waiting dependencies");
        }
        f::phase(&mut task, c::OwnerChipPhase::Waiting { dependencies, more: *more, holds: *holds });
        assert_eq!(row_state(&task.chip), expected.clone());
    }
}
#[test]
fn an_ended_task_says_its_owners_ending() {
    for (ending, expected) in [
        (
            Some(c::OwnerChipEnding::Done(c::ResultValue::Report(f::document_text(b"Done")))),
            State::Ended(c::OwnerTaskStatus::Done),
        ),
        (Some(c::OwnerChipEnding::Failed(f::document_text(b"Failed"))), State::Ended(c::OwnerTaskStatus::Failed)),
        (
            Some(c::OwnerChipEnding::Cancelled {
                reason: f::document_text(b"Cancelled"),
                result: None,
                by: c::OwnerRequester::Person(fixtures::PARTY),
                at: Wall::EPOCH,
            }),
            State::Ended(c::OwnerTaskStatus::Cancelled),
        ),
        (None, State::Phase(c::Phase::Ended)),
    ] {
        let mut task = f::task();
        f::phase(&mut task, c::OwnerChipPhase::Ended);
        let c::ChipPhase::Owner { details, .. } = task.chip.details.as_mut() else { panic!("owner") };
        details.ending = ending;
        assert_eq!(row_state(&task.chip), expected);
        task.waits_on = c::WaitsOn::News;
        assert_eq!(header_state(&task, None), expected);
    }
}
#[test]
fn another_phase_says_jigs_phase() {
    let mut task = f::task();
    f::phase(&mut task, c::OwnerChipPhase::Active(c::RunStage::Idle));
    let c::ChipPhase::Owner { details, .. } = task.chip.details.as_mut() else { panic!("owner") };
    details.ending = Some(c::OwnerChipEnding::Failed(f::document_text(b"Old ending")));
    assert_eq!(row_state(&task.chip), State::Phase(task.chip.phase));
    task.chip.phase = c::Phase::Waiting;
    *task.chip.details = c::ChipPhase::Waiting {
        dependencies: {
            let mut dependencies = List::with_capacity(1);
            dependencies.push(DEPENDENCY).expect("dependency");
            dependencies
        },
        hold: None,
    };
    assert_eq!(row_state(&task.chip), State::Phase(c::Phase::Waiting));
}
#[test]
fn a_dependencys_state_comes_from_its_link() {
    let mut task = f::task();
    let mut dependencies = List::with_capacity(1);
    dependencies.push(DEPENDENCY).expect("dependency");
    f::phase(&mut task, c::OwnerChipPhase::Waiting { dependencies, more: false, holds: false });
    let chip = task.chip;
    let expected = State::WaitingFor(DEPENDENCY);
    assert_eq!(link_state(&c::OwnerTaskLink::Live(chip.clone())), Some(expected.clone()));
    assert_eq!(link_state(&c::OwnerTaskLink::Retained { chip, at: Some(Wall::EPOCH) }), Some(expected.clone()));
    for status in [c::OwnerTaskStatus::Done, c::OwnerTaskStatus::Failed, c::OwnerTaskStatus::Cancelled] {
        assert_eq!(
            link_state(&c::OwnerTaskLink::Stored { number: DEPENDENCY, status: Some(status) }),
            Some(State::Ended(status))
        );
    }
    assert_eq!(link_state(&c::OwnerTaskLink::Stored { number: DEPENDENCY, status: None }), None);
}

const ATTEMPT: u64 = f::ATTEMPT;
fn observation(work: c::OwnerChipWork) -> c::OwnerChipObservation {
    c::OwnerChipObservation {
        channel: skein_lib::Token::new(1),
        work,
        since: Wall::EPOCH,
        reported_at: Wall::EPOCH,
        started: None,
    }
}
#[test]
fn a_running_rows_state_says_its_observed_work() {
    for (work, expected) in [
        (None, State::Running),
        (Some(c::OwnerChipWork::Writing), State::Writing),
        (
            Some(c::OwnerChipWork::Tool { call: Box::from(*b"call"), tool: Box::from(*b"read_file") }),
            State::Tool(Box::from(*b"read_file")),
        ),
        (Some(c::OwnerChipWork::Preparing), State::Running),
        (Some(c::OwnerChipWork::Starting), State::Running),
        (Some(c::OwnerChipWork::Active), State::Running),
        (Some(c::OwnerChipWork::Waiting), State::Running),
        (Some(c::OwnerChipWork::Ending), State::Running),
    ] {
        let mut task = f::task();
        f::phase(&mut task, c::OwnerChipPhase::Active(c::RunStage::Running { attempt: ATTEMPT }));
        let c::ChipPhase::Owner { details, .. } = task.chip.details.as_mut() else { panic!("owner") };
        if let Some(work) = work {
            details.observation = Some(observation(work));
        } else {
            details.observation = None;
        }
        assert_eq!(row_state(&task.chip), expected);
        task.waits_on = c::WaitsOn::Nothing;
        assert_eq!(header_state(&task, None), expected);
    }
}
fn pending(numbers: &[u64]) -> List<u64> {
    let mut pending = List::with_capacity(WINDOW);
    for number in numbers {
        pending.push(*number).expect("pending task");
    }
    pending
}
#[test]
fn the_headers_dependencies_use_the_supplied_exact_count() {
    for (numbers, count, expected) in [
        (&[DEPENDENCY][..], 1, State::WaitingFor(DEPENDENCY)),
        (&[DEPENDENCY, OTHER_DEPENDENCY][..], 2, State::WaitingForTasks { count: 2, more: false }),
        (&[DEPENDENCY][..], 3, State::WaitingForTasks { count: 3, more: false }),
        (&[][..], 1, State::WaitingForTasks { count: 1, more: false }),
    ] {
        let mut task = f::task();
        task.waits_on = c::WaitsOn::Dependencies { pending: pending(numbers), count };
        assert_eq!(header_state(&task, None), expected);
    }
}
#[test]
fn the_headers_delegates_use_the_chips_live_count() {
    for count in [0, 1, 3] {
        let mut task = f::task();
        task.waits_on = c::WaitsOn::Delegates;
        let c::ChipPhase::Owner { details, .. } = task.chip.details.as_mut() else { panic!("owner") };
        details.live_delegates = count;
        assert_eq!(header_state(&task, None), State::WaitingForDelegates(count));
    }
}
fn party_wait(whom: c::Whom) -> c::WaitsOn {
    c::WaitsOn::Party { whom, entries: 1, task: DEPENDENCY, kind: c::WaitedFor::Question }
}
#[test]
fn the_headers_party_is_you_only_for_the_person_or_a_held_project_role() {
    let mut task = f::task();
    let mut me = fixtures::chats::me();
    task.waits_on = party_wait(c::Whom::Person(me.party.number));
    assert_eq!(header_state(&task, Some(&me)), State::WaitingForYou);
    assert_eq!(header_state(&task, None), State::WaitingForParty(c::Whom::Person(me.party.number)));
    task.waits_on = party_wait(c::Whom::Person(DEPENDENCY));
    assert_eq!(header_state(&task, Some(&me)), State::WaitingForParty(c::Whom::Person(DEPENDENCY)));
    for (held, number) in [
        (c::HeldRole::Owner, 0),
        (c::HeldRole::Maintainer, 1),
        (c::HeldRole::Member, 2),
        (c::HeldRole::Observer, 3),
        (c::HeldRole::Policy { role: 7 }, 7),
    ] {
        me.projects.get_mut(0).expect("project").role = held;
        let whom = c::Whom::Role { project: task.project, role: number };
        task.waits_on = party_wait(whom);
        assert_eq!(header_state(&task, Some(&me)), State::WaitingForYou);
        assert_eq!(header_state(&task, None), State::WaitingForParty(whom));
        let other_project = c::Whom::Role { project: task.project + 1, role: number };
        task.waits_on = party_wait(other_project);
        assert_eq!(header_state(&task, Some(&me)), State::WaitingForParty(other_project));
    }
    let whom = c::Whom::Role { project: task.project, role: 8 };
    task.waits_on = party_wait(whom);
    assert_eq!(header_state(&task, Some(&me)), State::WaitingForParty(whom));
    me.projects.clear();
    assert_eq!(header_state(&task, Some(&me)), State::WaitingForParty(whom));
}
#[test]
fn the_header_says_news_or_falls_back_to_the_row_when_nothing_is_pending() {
    let mut task = f::task();
    task.waits_on = c::WaitsOn::News;
    assert_eq!(header_state(&task, None), State::WaitingForNews);
    f::phase(&mut task, c::OwnerChipPhase::Waiting { dependencies: pending(&[DEPENDENCY]), more: false, holds: false });
    for waits_on in [c::WaitsOn::Nothing, c::WaitsOn::Dependencies { pending: pending(&[]), count: 0 }] {
        task.waits_on = waits_on;
        assert_eq!(header_state(&task, None), State::WaitingFor(DEPENDENCY));
    }
}

fn agent_document() -> c::Task {
    let mut task = f::task();
    task.priority = None;
    task
}
fn activity_page(position: u64) -> jig_client::Readout {
    let mut rows = List::with_capacity(1);
    rows.push(c::ActivityLine {
        position,
        at: Wall::EPOCH,
        event: c::Activity::RunBegan {
            attempt: ATTEMPT,
            how: c::RunBeginning::Fresh { tail: 0 },
            why: c::Text::new(Box::from(*b"Begin"), 5).expect("why"),
        },
    })
    .expect("activity");
    jig_client::Readout::Jig(c::ReadResult::Value(c::ReadValue::Activity(Box::new(c::ActivityPage {
        rows,
        before: None,
        after: None,
        cut: false,
    }))))
}
fn activity_read(page: &Task) -> Request {
    Request::Client(Event::Read {
        handle: page.activity,
        read: Readable::Jig(c::Read::Activity { task: f::TASK, before: None }),
    })
}
#[test]
fn an_agents_task_reads_its_thread_as_a_chat() {
    const NEXT: u64 = f::POSITION + 1;
    for first_page_before_document in [true, false] {
        let mut f = f::opened();
        let mut page = Task::new(f::TASK, FIRST, &fixtures::limits(), &mut Queue::with_capacity(OUTPUTS));
        let mut task = agent_document();
        f::deliver(&mut f.client, f.watch, task.clone());
        while f.out.pop().is_some() {}
        if first_page_before_document {
            assert!(page.told(
                Time::ZERO,
                &f.client,
                Told::Page { handle: page.activity, page: activity_page(f::POSITION) },
                &mut f.out,
            ));
            assert!(page.latest.is_some());
        }
        page.told(Time::ZERO, &f.client, Told::Changed { handle: page.document }, &mut f.out);
        while f.out.pop().is_some() {}
        if !first_page_before_document {
            page.told(
                Time::ZERO,
                &f.client,
                Told::Page { handle: page.activity, page: activity_page(f::POSITION) },
                &mut f.out,
            );
        }
        assert!(page.latest.is_none());
        assert_eq!(page.thread.loaded, f::POSITION);
        assert_eq!(page.thread.lines.rows.len(), 1);
        assert!(page.thread.reading.is_none());
        task.activity = NEXT;
        f::deliver(&mut f.client, f.watch, task.clone());
        page.told(Time::ZERO, &f.client, Told::Changed { handle: page.document }, &mut f.out);
        assert_eq!(f.out.pop(), Some(activity_read(&page)));
        assert!(f.out.is_empty());
        page.told(
            Time::ZERO,
            &f.client,
            Told::Page { handle: page.activity, page: jig_client::Readout::Limit },
            &mut f.out,
        );
        assert!(page.thread.reading.is_none(), "failed page frees the read");
        page.told(Time::ZERO, &f.client, Told::Changed { handle: page.document }, &mut f.out);
        assert_eq!(f.out.pop(), Some(activity_read(&page)));
        page.thread.run(
            &c::RunState { task: f::TASK, attempt: Some(ATTEMPT), committed_turn: 0, writing: None, call: None },
            Some(&c::RunText {
                attempt: ATTEMPT,
                began: None,
                turn: c::ProvisionalTurnName { attempt: ATTEMPT, number: 1, retry: 0 },
                from: 0,
                through: 5,
                text: c::Text::new(Box::from(*b"Hello"), 5).expect("provisional text"),
                cut: false,
                call: None,
            }),
        );
        f::phase(&mut task, c::OwnerChipPhase::Active(c::RunStage::Idle));
        f::deliver(&mut f.client, f.watch, task);
        page.told(Time::ZERO, &f.client, Told::Changed { handle: page.document }, &mut f.out);
        assert!(!page.thread.live.is_empty(), "unread committed activity keeps the run's text");
        page.told(Time::ZERO, &f.client, Told::Page { handle: page.activity, page: activity_page(NEXT) }, &mut f.out);
        assert_eq!(page.thread.loaded, NEXT);
        assert!(page.thread.live.is_empty(), "no provisional text remains once its activity is loaded");
    }
}
#[test]
fn a_failed_first_page_frees_an_unknown_tasks_thread() {
    let mut f = f::opened();
    let mut page = Task::new(f::TASK, FIRST, &fixtures::limits(), &mut Queue::with_capacity(OUTPUTS));
    page.told(
        Time::ZERO,
        &f.client,
        Told::Page { handle: page.activity, page: jig_client::Readout::Limit },
        &mut f.out,
    );
    assert_eq!(page.shape, Shape::Unknown);
    assert!(page.thread.reading.is_none());
    f::deliver(&mut f.client, f.watch, agent_document());
    while f.out.pop().is_some() {}
    page.told(Time::ZERO, &f.client, Told::Changed { handle: page.document }, &mut f.out);
    assert_eq!(f.out.pop(), Some(activity_read(&page)));
}
#[test]
fn an_agents_live_run_text_reaches_its_thread() {
    use skein_lib::Time;
    let (mut page, mut f) = opened();
    f::deliver(&mut f.client, f.watch, agent_document());
    page.told(Time::ZERO, &f.client, Told::Changed { handle: page.document }, &mut f.out);
    let handle = page.run.expect("live run");
    let mut emitted = Queue::with_capacity(OUTPUTS);
    f.client.step(
        Time::ZERO,
        Wall::EPOCH,
        Event::Watch { handle, subject: Watchable::Jig(c::Subject::Run { task: f::TASK, turn: None, offset: 0 }) },
        &mut emitted,
    );
    let mut watch = None;
    while let Some(request) = emitted.pop() {
        if let jig_client::Request::Down {
            down: jig_client::Down::Open { watch: token, subject: Watchable::Jig(c::Subject::Run { .. }) },
        } = request
        {
            watch = Some(token);
        }
    }
    let watch = watch.expect("run opened");
    let mut run = c::RunState { task: f::TASK, attempt: Some(ATTEMPT), committed_turn: 0, writing: None, call: None };
    run.writing = Some(c::RunText {
        attempt: ATTEMPT,
        began: Some(Wall::EPOCH),
        turn: c::ProvisionalTurnName { attempt: ATTEMPT, number: 1, retry: 0 },
        from: 0,
        through: 5,
        text: c::Text::new(Box::from(*b"Hello"), 5).expect("run text"),
        cut: false,
        call: None,
    });
    f.client.step(
        Time::ZERO,
        Wall::EPOCH,
        Event::Up {
            up: jig_client::Up::Watched {
                watch,
                event: c::WatchEvent::Delivered(c::Delivery::RunState(Box::new(run))),
            },
        },
        &mut emitted,
    );
    for told in [Told::Changed { handle }, Told::Text { handle }] {
        assert!(page.told(Time::ZERO, &f.client, told, &mut f.out));
        assert_eq!(page.thread.live.get(0).expect("provisional turn").text.bytes(), b"Hello");
    }
    f.client.step(
        Time::ZERO,
        Wall::EPOCH,
        Event::Up {
            up: jig_client::Up::Watched {
                watch,
                event: c::WatchEvent::Delivered(c::Delivery::RunState(Box::new(c::RunState {
                    task: f::TASK,
                    attempt: Some(ATTEMPT),
                    committed_turn: 0,
                    writing: Some(c::RunText {
                        attempt: ATTEMPT,
                        began: Some(Wall::EPOCH),
                        turn: c::ProvisionalTurnName { attempt: ATTEMPT, number: 1, retry: 0 },
                        from: 5,
                        through: 11,
                        text: c::Text::new(Box::from(*b" again"), 6).expect("next run text"),
                        cut: false,
                        call: None,
                    }),
                    call: None,
                }))),
            },
        },
        &mut emitted,
    );
    assert!(page.told(Time::ZERO, &f.client, Told::Text { handle }, &mut f.out));
    assert_eq!(page.thread.live.get(0).expect("provisional turn").text.bytes(), b"Hello again");
}
#[test]
fn a_runs_heading_follows_its_chips_phase() {
    use crate::pages::task::{Run, run};
    const LAST: u64 = ATTEMPT - 1;
    let since = Some(Wall::EPOCH);
    for (phase, attempt, since, expected) in [
        (
            c::OwnerChipPhase::Active(c::RunStage::Claimed { attempt: ATTEMPT }),
            Some(LAST),
            since,
            Some(Run::Live { attempt: ATTEMPT, since }),
        ),
        (
            c::OwnerChipPhase::Active(c::RunStage::Running { attempt: ATTEMPT }),
            Some(LAST),
            since,
            Some(Run::Live { attempt: ATTEMPT, since }),
        ),
        (
            c::OwnerChipPhase::Closing(c::ClosingStage::Run { attempt: ATTEMPT }),
            Some(LAST),
            since,
            Some(Run::Live { attempt: ATTEMPT, since }),
        ),
        (
            c::OwnerChipPhase::Active(c::RunStage::Running { attempt: ATTEMPT }),
            None,
            None,
            Some(Run::Live { attempt: ATTEMPT, since: None }),
        ),
        (c::OwnerChipPhase::Active(c::RunStage::Idle), Some(LAST), since, Some(Run::Waiting { attempt: LAST, since })),
        (
            c::OwnerChipPhase::Active(c::RunStage::Idle),
            Some(LAST),
            None,
            Some(Run::Waiting { attempt: LAST, since: None }),
        ),
        (c::OwnerChipPhase::Active(c::RunStage::Idle), None, since, Some(Run::NoRun)),
        (c::OwnerChipPhase::Ended, Some(LAST), since, Some(Run::Ended { attempt: LAST })),
        (c::OwnerChipPhase::Ended, None, since, None),
    ] {
        let mut task = agent_document();
        f::phase(&mut task, phase.clone());
        let c::ChipPhase::Owner { details, .. } = task.chip.details.as_mut() else { panic!("owner") };
        details.attempt = attempt;
        details.run_since = since;
        assert_eq!(run(&task), expected, "{phase:?}, {attempt:?}, {since:?}");
    }
    let mut task = agent_document();
    *task.chip.details = c::ChipPhase::Starting;
    assert_eq!(run(&task), None);
}
#[test]
fn an_ended_tasks_historical_document_keeps_its_result() {
    let (mut page, mut f) = opened();
    let mut task = agent_document();
    f::phase(&mut task, c::OwnerChipPhase::Ended);
    task.result = None;
    let result = f::result(&task, c::Ending::Done, Some(c::ResultValue::Report(f::document_text(b"Finished"))));
    page.told(
        Time::ZERO,
        &f.client,
        Told::Page {
            handle: page.document,
            page: jig_client::Readout::Jig(c::ReadResult::Value(c::ReadValue::Historical(Box::new(
                c::OwnerHistorical {
                    key: c::ObjectKey::Task(f::TASK),
                    value: c::OwnerHistoricalValue::Task { document: Box::new(task), result: Box::new(result.clone()) },
                    at: None,
                    by: None,
                },
            )))),
        },
        &mut f.out,
    );
    assert_eq!(page.document(&f.client).expect("final task").result, Some(c::TaskResult::Document(Box::new(result))));
}

#[test]
fn an_agents_thread_reads_on_until_the_announced_tail() {
    let (mut page, mut f) = opened();
    f::deliver(&mut f.client, f.watch, agent_document());
    page.told(Time::ZERO, &f.client, Told::Changed { handle: page.document }, &mut f.out);
    while f.out.pop().is_some() {}
    page.told(
        Time::ZERO,
        &f.client,
        Told::Page { handle: page.activity, page: activity_page(f::POSITION - 1) },
        &mut f.out,
    );
    assert_eq!(f.out.pop(), Some(activity_read(&page)));
    assert!(f.out.is_empty());
    page.told(
        Time::ZERO,
        &f.client,
        Told::Page { handle: page.activity, page: activity_page(f::POSITION) },
        &mut f.out,
    );
    assert!(page.thread.reading.is_none());
    assert_eq!(page.thread.loaded, f::POSITION);
    assert!(f.out.is_empty());
}

use crate::pages::{self, Act, Pages, task::Sending};
const WORDS: &[u8] = b"Keep the public interface";
fn task_pages(mut task: c::Task) -> (Pages, jig_client::Client) {
    task.label = 1234; // A task page, distinct from the chat template.
    let fixture = f::with_delegate();
    let mut pages = Pages::new();
    let env = fixtures::env(&fixtures::limits());
    let mut out = Queue::with_capacity(OUTPUTS);
    pages::step(
        &mut pages,
        &env,
        &fixture.client,
        pages::Event::Went { address: crate::Address::Task { task: f::TASK, filter: crate::PlanFilter::All } },
        &mut out,
    );
    let handle = pages.task().expect("task page").document;
    pages::step(
        &mut pages,
        &env,
        &fixture.client,
        pages::Event::Told {
            told: Told::Page {
                handle,
                page: jig_client::Readout::Jig(c::ReadResult::Value(c::ReadValue::Task(Box::new(task)))),
            },
        },
        &mut out,
    );
    (pages, fixture.client)
}
fn page_act(pages: &mut Pages, client: &jig_client::Client, act: Act) -> Queue<Request> {
    let mut out = Queue::with_capacity(OUTPUTS);
    pages::step(pages, &fixtures::env(&fixtures::limits()), client, pages::Event::Act { act }, &mut out);
    out
}
fn offered_task(kind: c::ActKind, admission: Option<c::Admission>) -> c::Task {
    let mut task = f::task();
    task.priority = None;
    task.chip.executor = c::Executor::Agent { charter: 1 };
    task.acts = match admission {
        Some(admission) => fixtures::acts::offered(kind, admission),
        None => List::with_capacity(0),
    };
    task
}
#[test]
fn an_agents_send_is_one_message_only_when_allowed() {
    for (admission, sent) in [
        (Some(c::Admission::Allowed), true),
        (Some(c::Admission::Refused(c::Refusal::NotOffered)), false),
        (Some(c::Admission::Proposed { to: c::Recipient::Party(fixtures::PARTY) }), false),
        (None, false),
    ] {
        let (mut pages, client) = task_pages(offered_task(c::ActKind::Message, admission));
        page_act(&mut pages, &client, Act::MessageWords { words: Box::from(WORDS) });
        let mut out = page_act(&mut pages, &client, Act::Send);
        let asks = fixtures::acts::sent(&mut out);
        if sent {
            assert_eq!(
                asks.as_ref(),
                [c::Ask::Message { project: fixtures::PROJECT, task: f::TASK, words: fixtures::acts::text(WORDS) }]
            );
            assert!(pages.task().expect("page").words.is_empty());
            // A second nonempty message joins the same in-flight kind.
            page_act(&mut pages, &client, Act::MessageWords { words: Box::from(*b"Again") });
            assert!(fixtures::acts::sent(&mut page_act(&mut pages, &client, Act::Send)).is_empty());
        } else {
            assert!(asks.is_empty());
            assert_eq!(pages.task().expect("page").words.as_ref(), WORDS);
        }
    }
}
#[test]
fn a_goals_page_has_no_message_composer_or_older_read() {
    let mut task = offered_task(c::ActKind::Message, Some(c::Admission::Allowed));
    task.priority = Some(PRIORITY);
    let (mut pages, client) = task_pages(task);
    page_act(&mut pages, &client, Act::MessageWords { words: Box::from(WORDS) });
    assert!(pages.task().expect("page").words.is_empty());
    assert!(fixtures::acts::sent(&mut page_act(&mut pages, &client, Act::Send)).is_empty());
    assert!(page_act(&mut pages, &client, Act::Older).is_empty());
}
#[test]
fn task_stop_waits_for_confirmation_and_joins_its_pending_kind() {
    let (mut pages, client) = task_pages(offered_task(c::ActKind::Stop, Some(c::Admission::Allowed)));
    assert!(fixtures::acts::sent(&mut page_act(&mut pages, &client, Act::Stop)).is_empty());
    assert_eq!(pages.confirmation().expect("confirmation").act, c::ActKind::Stop);
    assert_eq!(
        fixtures::acts::sent(&mut page_act(&mut pages, &client, Act::Confirm)).as_ref(),
        [c::Ask::Stop { project: fixtures::PROJECT, task: f::TASK }]
    );
    assert!(pages.confirmation().is_none());
    page_act(&mut pages, &client, Act::Stop);
    assert!(fixtures::acts::sent(&mut page_act(&mut pages, &client, Act::Confirm)).is_empty());
    assert_eq!(pages.task().expect("page").pending.as_slice()[0].1, Sending::Stop);
}
#[test]
fn dismissing_task_stop_sends_nothing() {
    let (mut pages, client) = task_pages(offered_task(c::ActKind::Stop, Some(c::Admission::Allowed)));
    page_act(&mut pages, &client, Act::Stop);
    assert!(page_act(&mut pages, &client, Act::Cancel).is_empty());
    assert!(pages.confirmation().is_none());
    assert!(fixtures::acts::sent(&mut page_act(&mut pages, &client, Act::Confirm)).is_empty());
}
#[test]
fn an_unavailable_task_act_opens_no_confirmation() {
    for admission in [
        None,
        Some(c::Admission::Refused(c::Refusal::NotOffered)),
        Some(c::Admission::Proposed { to: c::Recipient::Party(fixtures::PARTY) }),
    ] {
        for (kind, act) in [(c::ActKind::Stop, Act::Stop), (c::ActKind::Cancel, Act::Close)] {
            let (mut pages, client) = task_pages(offered_task(kind, admission.clone()));
            assert!(fixtures::acts::sent(&mut page_act(&mut pages, &client, act)).is_empty());
            assert!(pages.confirmation().is_none());
        }
    }
}
#[test]
fn task_close_confirms_cancel_with_the_pages_reason_and_no_grace() {
    let (mut pages, client) = task_pages(offered_task(c::ActKind::Cancel, Some(c::Admission::Allowed)));
    assert!(fixtures::acts::sent(&mut page_act(&mut pages, &client, Act::Close)).is_empty());
    assert_eq!(pages.confirmation().expect("confirmation").act, c::ActKind::Cancel);
    assert_eq!(
        fixtures::acts::sent(&mut page_act(&mut pages, &client, Act::Confirm)).as_ref(),
        [c::Ask::Cancel {
            project: fixtures::PROJECT,
            task: f::TASK,
            reason: fixtures::acts::text(b"Cancelled from its page"),
            grace: None,
        }]
    );
}
#[test]
fn a_task_answer_sets_the_outcome_and_clears_pending() {
    let (mut pages, client) = task_pages(offered_task(c::ActKind::Stop, Some(c::Admission::Allowed)));
    page_act(&mut pages, &client, Act::Stop);
    page_act(&mut pages, &client, Act::Confirm);
    let key = pages.task().expect("page").pending.as_slice()[0].0;
    let mut out = Queue::with_capacity(OUTPUTS);
    pages::step(
        &mut pages,
        &fixtures::env(&fixtures::limits()),
        &client,
        pages::Event::Told { told: Told::Pending { key } },
        &mut out,
    );
    assert!(!pages.task().expect("page").pending.is_empty());
    pages::step(
        &mut pages,
        &fixtures::env(&fixtures::limits()),
        &client,
        pages::Event::Told { told: Told::Answered { key, outcome: jig_client::Outcome::Unknown } },
        &mut out,
    );
    let page = pages.task().expect("page");
    assert!(page.pending.is_empty());
    assert_eq!(page.outcome, Some(jig_client::Outcome::Unknown));
}
#[test]
fn older_on_an_agents_page_reads_the_cursor_before_its_window() {
    use jig_client::thread::window::Edge;
    let (mut page, mut fixture) = opened();
    page.shape = Shape::Agent;
    page.thread.reading = None;
    let cursor = c::Cursor::new(Box::from(*b"older"), 32).expect("cursor");
    page.thread.lines.edges.after = Edge::More(cursor.clone());
    while fixture.out.pop().is_some() {}
    page.act(&fixtures::env(&fixtures::limits()), &fixture.client, Act::Older, &mut 0, &mut fixture.out);
    assert_eq!(
        fixture.out.pop(),
        Some(Request::Client(Event::Read {
            handle: page.activity,
            read: Readable::Jig(c::Read::Activity { task: f::TASK, before: Some(cursor) }),
        }))
    );
}

#[test]
fn a_message_answer_reconciles_its_conversation_words() {
    for outcome in [
        jig_client::Outcome::Unknown,
        jig_client::Outcome::Answered(jig_client::Answered::Jig(c::Answer::Refused {
            refusal: c::Refusal::NotWaiting,
            at: None,
        })),
    ] {
        let (mut pages, client) = task_pages(offered_task(c::ActKind::Message, Some(c::Admission::Allowed)));
        page_act(&mut pages, &client, Act::MessageWords { words: Box::from(WORDS) });
        page_act(&mut pages, &client, Act::Send);
        let page = pages.task().expect("page");
        let key = page.pending.as_slice()[0].0;
        assert!(!page.thread.words.rows.is_empty());
        let mut out = Queue::with_capacity(OUTPUTS);
        pages::step(
            &mut pages,
            &fixtures::env(&fixtures::limits()),
            &client,
            pages::Event::Told { told: Told::Answered { key, outcome: outcome.clone() } },
            &mut out,
        );
        let page = pages.task().expect("page");
        assert!(page.pending.is_empty());
        assert_eq!(page.outcome, Some(outcome));
        assert!(page.thread.words.rows.is_empty());
    }
}
#[test]
fn a_delivered_message_is_said_by_its_words_not_an_outcome() {
    let (mut pages, client) = task_pages(offered_task(c::ActKind::Message, Some(c::Admission::Allowed)));
    page_act(&mut pages, &client, Act::MessageWords { words: Box::from(WORDS) });
    page_act(&mut pages, &client, Act::Send);
    let key = pages.task().expect("page").pending.as_slice()[0].0;
    let mut out = Queue::with_capacity(OUTPUTS);
    let done = jig_client::Outcome::Answered(jig_client::Answered::Jig(c::Answer::Done(c::Outcome::Changed)));
    pages::step(
        &mut pages,
        &fixtures::env(&fixtures::limits()),
        &client,
        pages::Event::Told { told: Told::Answered { key, outcome: done } },
        &mut out,
    );
    let page = pages.task().expect("page");
    assert!(page.pending.is_empty());
    assert_eq!(page.outcome, None);
}

fn amend_pages(goal: bool) -> (Pages, jig_client::Client) {
    let mut task = offered_task(c::ActKind::Amend, Some(c::Admission::Allowed));
    if goal {
        task.priority = Some(PRIORITY);
    }
    task_pages(task)
}
#[test]
fn amend_opens_only_when_the_task_offers_it_allowed() {
    for (admission, opens) in [
        (Some(c::Admission::Allowed), true),
        (Some(c::Admission::Refused(c::Refusal::NotOffered)), false),
        (Some(c::Admission::Proposed { to: c::Recipient::Party(fixtures::PARTY) }), false),
        (None, false),
    ] {
        let (mut pages, client) = task_pages(offered_task(c::ActKind::Amend, admission));
        assert!(page_act(&mut pages, &client, Act::Amend).is_empty());
        assert_eq!(pages.task().unwrap().amending.is_some(), opens);
    }
}
#[test]
fn edited_words_send_one_amendment_with_the_base_revision_and_pages_reason() {
    let (mut pages, client) = amend_pages(false);
    let base = pages.task().unwrap().document(&client).unwrap().spec_revision;
    page_act(&mut pages, &client, Act::Amend);
    page_act(&mut pages, &client, Act::AmendWords { words: Box::from(WORDS) });
    let asks = fixtures::acts::sent(&mut page_act(&mut pages, &client, Act::SendAmendment));
    let [c::Ask::Amend { project, task, revision, change, reason }] = asks.as_ref() else { panic!("one amendment") };
    assert_eq!((*project, *task, *revision), (fixtures::PROJECT, f::TASK, Some(base)));
    assert_eq!(reason.bytes(), b"Amended from its page");
    let mut spec = pages.task().unwrap().document(&client).unwrap().spec.whole().unwrap();
    spec.words = fixtures::acts::text(WORDS);
    assert_eq!(change.spec, Some(spec));
    assert!(fixtures::acts::sent(&mut page_act(&mut pages, &client, Act::SendAmendment)).is_empty());
}
#[test]
fn a_goals_priority_alone_sends_no_spec_revision() {
    let (mut pages, client) = amend_pages(true);
    page_act(&mut pages, &client, Act::Amend);
    page_act(&mut pages, &client, Act::AmendPriority { priority: PRIORITY + 1 });
    let asks = fixtures::acts::sent(&mut page_act(&mut pages, &client, Act::SendAmendment));
    let [c::Ask::Amend { revision, change, .. }] = asks.as_ref() else { panic!("priority amendment") };
    assert_eq!(*revision, None);
    assert_eq!(change.tracked, Some(c::Tracking::Tracked { priority: PRIORITY + 1 }));
}
#[test]
fn sending_no_change_says_so_until_an_edit_and_discard_drops_the_editor() {
    let (mut pages, client) = amend_pages(true);
    page_act(&mut pages, &client, Act::Amend);
    assert!(page_act(&mut pages, &client, Act::SendAmendment).is_empty());
    assert!(pages.task().unwrap().amending.as_ref().unwrap().no_change);
    page_act(&mut pages, &client, Act::AmendTracked);
    assert!(!pages.task().unwrap().amending.as_ref().unwrap().no_change);
    assert_eq!(pages.task().unwrap().amending.as_ref().unwrap().tracking, Some(c::Tracking::Untracked));
    page_act(&mut pages, &client, Act::DiscardAmendment);
    assert!(pages.task().unwrap().amending.is_none());
}
#[test]
fn a_cut_specification_opens_read_only_and_ignores_words() {
    let mut task = offered_task(c::ActKind::Amend, Some(c::Admission::Allowed));
    task.spec.specification.words.cut = true;
    let (mut pages, client) = task_pages(task);
    page_act(&mut pages, &client, Act::Amend);
    page_act(&mut pages, &client, Act::AmendWords { words: Box::from(WORDS) });
    let open = pages.task().unwrap().amending.as_ref().unwrap();
    assert!(open.cut);
    assert!(open.words.is_empty());
    assert!(open.draft.amendment().is_none());
}
fn amend_answer(pages: &mut Pages, client: &jig_client::Client, answer: c::Answer) {
    let key = pages.task().unwrap().pending.as_slice()[0].0;
    pages::step(
        pages,
        &fixtures::env(&fixtures::limits()),
        client,
        pages::Event::Told {
            told: Told::Answered { key, outcome: jig_client::Outcome::Answered(jig_client::Answered::Jig(answer)) },
        },
        &mut Queue::with_capacity(OUTPUTS),
    );
}
#[test]
fn a_done_amendment_stays_as_its_outcome_until_amend_again() {
    let (mut pages, client) = amend_pages(false);
    page_act(&mut pages, &client, Act::Amend);
    page_act(&mut pages, &client, Act::AmendWords { words: Box::from(WORDS) });
    page_act(&mut pages, &client, Act::SendAmendment);
    amend_answer(&mut pages, &client, c::Answer::Done(c::Outcome::Changed));
    let open = pages.task().unwrap().amending.as_ref().unwrap();
    assert!(open.done);
    assert_eq!(
        open.outcome,
        Some(jig_client::Outcome::Answered(jig_client::Answered::Jig(c::Answer::Done(c::Outcome::Changed))))
    );
    assert!(pages.task().unwrap().pending.is_empty());
    assert!(page_act(&mut pages, &client, Act::SendAmendment).is_empty());
    page_act(&mut pages, &client, Act::DiscardAmendment);
    assert!(pages.task().unwrap().amending.as_ref().unwrap().done);
    page_act(&mut pages, &client, Act::AmendWords { words: Box::from(*b"Ignored") });
    assert_eq!(pages.task().unwrap().amending.as_ref().unwrap().words.as_ref(), WORDS);
    page_act(&mut pages, &client, Act::Amend);
    let open = pages.task().unwrap().amending.as_ref().unwrap();
    assert!(!open.done);
    assert!(open.outcome.is_none());
    assert!(open.draft.amendment().is_none());
}
#[test]
fn a_moved_refusal_keeps_the_editor_and_look_again_redrafts_the_current_document() {
    let (mut pages, client) = amend_pages(false);
    page_act(&mut pages, &client, Act::Amend);
    page_act(&mut pages, &client, Act::AmendWords { words: Box::from(WORDS) });
    page_act(&mut pages, &client, Act::SendAmendment);
    let mut task = pages.task().unwrap().document(&client).unwrap().clone();
    task.spec_revision.value += 1;
    let revision = task.spec_revision;
    let handle = pages.task().unwrap().document;
    pages::step(
        &mut pages,
        &fixtures::env(&fixtures::limits()),
        &client,
        pages::Event::Told {
            told: Told::Page {
                handle,
                page: jig_client::Readout::Jig(c::ReadResult::Value(c::ReadValue::Task(Box::new(task)))),
            },
        },
        &mut Queue::with_capacity(OUTPUTS),
    );
    amend_answer(
        &mut pages,
        &client,
        c::Answer::Refused { refusal: c::Refusal::Moved { now: revision, what: c::Moved::Spec }, at: None },
    );
    assert!(
        pages.task().unwrap().amending.as_ref().unwrap().draft.moved(pages.task().unwrap().document(&client).unwrap())
    );
    assert!(page_act(&mut pages, &client, Act::SendAmendment).is_empty());
    page_act(&mut pages, &client, Act::AmendAgain);
    let open = pages.task().unwrap().amending.as_ref().unwrap();
    assert_eq!(open.draft.revision(), revision);
    assert!(open.draft.amendment().is_none());
    assert!(open.outcome.is_none());
}
#[test]
fn amendment_words_are_bounded_and_must_be_utf8() {
    let (mut pages, client) = amend_pages(false);
    page_act(&mut pages, &client, Act::Amend);
    let original = pages.task().unwrap().amending.as_ref().unwrap().words.clone();
    let bound = usize::try_from(fixtures::limits().client.domain.text).unwrap();
    for words in [skein_lib::bytes::zeroed(bound + 1), Box::from([0xff])] {
        page_act(&mut pages, &client, Act::AmendWords { words });
        assert_eq!(pages.task().unwrap().amending.as_ref().unwrap().words, original);
    }
    page_act(&mut pages, &client, Act::AmendWords { words: skein_lib::bytes::zeroed(bound) });
    assert_eq!(pages.task().unwrap().amending.as_ref().unwrap().words.len(), bound);
}

fn procedure() -> (Task, f::Fixture) {
    let (mut page, mut fixture) = opened();
    while fixture.out.pop().is_some() {}
    let mut task = f::task();
    task.chip.executor = c::Executor::Procedure { connector: fixtures::FORGE, kind: 2 };
    f::deliver(&mut fixture.client, fixture.watch, task);
    page.told(Time::ZERO, &fixture.client, Told::Changed { handle: page.document }, &mut fixture.out);
    (page, fixture)
}
fn ended_read(page: &Task) -> Request {
    Request::Client(Event::Read {
        handle: page.ended.as_ref().expect("procedure's ended children").handle,
        read: Readable::Jig(c::Read::Tasks {
            query: c::TaskQuery {
                project: None,
                requester: None,
                root: None,
                parent: Some(f::TASK),
                label: None,
                phase: Some(c::PhaseClass::Ended),
                executor: None,
                tracked: None,
                order: c::QueryOrder::Ended,
            },
            live: None,
            ended: None,
        }),
    })
}
fn ended_page(cut: bool) -> c::TaskPage {
    c::TaskPage {
        live: List::with_capacity(0),
        live_after: None,
        ended: c::EndedGroup::Rows { rows: List::with_capacity(0), after: None, cut },
    }
}
fn answer_ended(page: &mut Task, fixture: &mut f::Fixture, reply: c::TaskPage) {
    let handle = page.ended.as_ref().expect("ended children").handle;
    assert!(page.told(
        Time::ZERO,
        &fixture.client,
        Told::Page {
            handle,
            page: jig_client::Readout::Jig(c::ReadResult::Value(c::ReadValue::Tasks(Box::new(reply)))),
        },
        &mut fixture.out,
    ));
}
#[test]
fn every_procedure_reads_its_ended_children_on_its_own_handle() {
    for connector in [fixtures::FORGE, fixtures::FORGE + 1] {
        let (mut page, mut fixture) = opened();
        while fixture.out.pop().is_some() {}
        let next = page.next;
        let mut task = f::task();
        task.chip.executor = c::Executor::Procedure { connector, kind: 2 };
        f::deliver(&mut fixture.client, fixture.watch, task);
        page.told(Time::ZERO, &fixture.client, Told::Changed { handle: page.document }, &mut fixture.out);
        assert_eq!(page.ended.as_ref().unwrap().handle.number, next);
        assert_eq!(page.next, next + 1);
        assert_eq!(fixture.out.pop(), Some(ended_read(&page)));
        assert!(fixture.out.is_empty());
        assert!(page.ended.as_ref().unwrap().reading);
        assert!(!page.ended.as_ref().unwrap().stale);
    }
}
#[test]
fn agent_and_person_tasks_do_not_read_ended_children() {
    for executor in [
        c::Executor::Agent { charter: 1 },
        c::Executor::Party { party: 73 },
        c::Executor::Role { project: fixtures::PROJECT, role: 1 },
    ] {
        let (mut page, mut fixture) = opened();
        while fixture.out.pop().is_some() {}
        let mut task = f::task();
        task.chip.executor = executor;
        task.priority = None;
        f::deliver(&mut fixture.client, fixture.watch, task);
        for handle in [page.document, page.tree] {
            page.told(Time::ZERO, &fixture.client, Told::Changed { handle }, &mut fixture.out);
        }
        assert!(page.ended.is_none());
        assert!(fixture.out.is_empty());
    }
}
#[test]
fn tree_and_document_changes_read_ended_children_again() {
    let (mut page, mut fixture) = procedure();
    assert_eq!(fixture.out.pop(), Some(ended_read(&page)));
    for handle in [page.tree, page.document] {
        answer_ended(&mut page, &mut fixture, ended_page(false));
        assert!(fixture.out.is_empty());
        page.told(Time::ZERO, &fixture.client, Told::Changed { handle }, &mut fixture.out);
        assert_eq!(fixture.out.pop(), Some(ended_read(&page)));
        assert!(fixture.out.is_empty());
    }
}
#[test]
fn changes_during_an_ended_read_coalesce_into_one_more_read() {
    let (mut page, mut fixture) = procedure();
    assert_eq!(fixture.out.pop(), Some(ended_read(&page)));
    for handle in [page.tree, page.document, page.tree] {
        page.told(Time::ZERO, &fixture.client, Told::Changed { handle }, &mut fixture.out);
        assert!(fixture.out.is_empty());
    }
    assert!(page.ended.as_ref().unwrap().stale);
    answer_ended(&mut page, &mut fixture, ended_page(false));
    assert_eq!(fixture.out.pop(), Some(ended_read(&page)));
    assert!(fixture.out.is_empty());
    assert!(page.ended.as_ref().unwrap().reading);
    assert!(!page.ended.as_ref().unwrap().stale);
    answer_ended(&mut page, &mut fixture, ended_page(true));
    assert!(fixture.out.is_empty());
    assert!(!page.ended.as_ref().unwrap().reading);
}
#[test]
fn an_ended_reply_replaces_the_kept_page() {
    let (mut page, mut fixture) = procedure();
    while fixture.out.pop().is_some() {}
    for cut in [false, true] {
        if cut {
            page.told(Time::ZERO, &fixture.client, Told::Changed { handle: page.tree }, &mut fixture.out);
            assert_eq!(fixture.out.pop(), Some(ended_read(&page)));
        }
        let reply = ended_page(cut);
        answer_ended(&mut page, &mut fixture, reply.clone());
        assert_eq!(page.ended.as_ref().unwrap().page.as_deref(), Some(&reply));
        assert!(!page.ended.as_ref().unwrap().reading);
        assert!(fixture.out.is_empty());
    }
}
#[test]
fn another_reply_keeps_the_ended_page_and_frees_the_next_read() {
    let (mut page, mut fixture) = procedure();
    while fixture.out.pop().is_some() {}
    let kept = ended_page(false);
    answer_ended(&mut page, &mut fixture, kept.clone());
    let handle = page.ended.as_ref().unwrap().handle;
    for stale in [false, true] {
        page.told(Time::ZERO, &fixture.client, Told::Changed { handle: page.tree }, &mut fixture.out);
        assert_eq!(fixture.out.pop(), Some(ended_read(&page)));
        if stale {
            page.told(Time::ZERO, &fixture.client, Told::Changed { handle: page.document }, &mut fixture.out);
        }
        page.told(
            Time::ZERO,
            &fixture.client,
            Told::Page {
                handle,
                page: jig_client::Readout::Jig(c::ReadResult::Value(c::ReadValue::Task(Box::new(f::task())))),
            },
            &mut fixture.out,
        );
        assert_eq!(page.ended.as_ref().unwrap().page.as_deref(), Some(&kept));
        assert_eq!(page.ended.as_ref().unwrap().reading, stale);
        if stale {
            assert_eq!(fixture.out.pop(), Some(ended_read(&page)));
        }
        assert!(fixture.out.is_empty());
    }
}
#[test]
fn leaving_a_procedure_unwatches_its_ended_read_handle() {
    let (page, mut fixture) = procedure();
    while fixture.out.pop().is_some() {}
    let next = page.leave(&mut fixture.out);
    assert_eq!(next, page.next);
    assert_eq!(
        f::handles(&mut fixture.out).as_ref(),
        [page.document, page.tree, page.activity, page.run.unwrap(), page.ended.as_ref().unwrap().handle],
    );
}

fn goal() -> (Task, f::Fixture) {
    let (mut page, mut fixture) = opened();
    while fixture.out.pop().is_some() {}
    let mut task = f::task();
    task.chip.executor = c::Executor::Agent { charter: 1 };
    task.priority = Some(PRIORITY);
    f::deliver(&mut fixture.client, fixture.watch, task);
    page.told(Time::ZERO, &fixture.client, Told::Changed { handle: page.document }, &mut fixture.out);
    (page, fixture)
}
#[test]
fn a_goal_reads_its_ended_delegates_on_its_own_handle() {
    let (page, mut fixture) = goal();
    let ended = page.ended.as_ref().unwrap();
    assert_eq!(page.shape, Shape::Tracked);
    assert_eq!(page.next, ended.handle.number + 1);
    assert_ne!(ended.handle, page.document);
    assert_ne!(ended.handle, page.tree);
    assert_ne!(ended.handle, page.activity);
    assert_ne!(Some(ended.handle), page.run);
    assert!(ended.reading);
    assert!(!ended.stale);
    assert_eq!(fixture.out.pop(), Some(ended_read(&page)));
    assert!(fixture.out.is_empty());
}
#[test]
fn a_goals_tree_and_document_changes_reread_one_page_at_a_time() {
    let (mut page, mut fixture) = goal();
    assert_eq!(fixture.out.pop(), Some(ended_read(&page)));
    for handle in [page.tree, page.document] {
        let kept = ended_page(false);
        answer_ended(&mut page, &mut fixture, kept.clone());
        assert!(fixture.out.is_empty());
        page.told(Time::ZERO, &fixture.client, Told::Changed { handle }, &mut fixture.out);
        assert_eq!(fixture.out.pop(), Some(ended_read(&page)));
        for handle in [page.document, page.tree, handle] {
            page.told(Time::ZERO, &fixture.client, Told::Changed { handle }, &mut fixture.out);
            assert!(fixture.out.is_empty());
        }
        let ended = page.ended.as_ref().unwrap();
        assert!(ended.reading && ended.stale);
        assert_eq!(ended.page.as_deref(), Some(&kept));
        answer_ended(&mut page, &mut fixture, ended_page(true));
        assert_eq!(fixture.out.pop(), Some(ended_read(&page)));
        assert!(fixture.out.is_empty());
        let ended = page.ended.as_ref().unwrap();
        assert!(ended.reading && !ended.stale);
    }
    answer_ended(&mut page, &mut fixture, ended_page(false));
    assert!(!page.ended.as_ref().unwrap().reading);
    assert!(fixture.out.is_empty());
}
#[test]
fn plan_filter_moves_keep_the_ended_page_and_drafts_without_reading_again() {
    use crate::PlanFilter as Filter;
    let (mut pages, client) = amend_pages(true);
    let env = fixtures::env(&fixtures::limits());
    let mut out = Queue::with_capacity(OUTPUTS);
    let handle = pages.task().unwrap().ended.as_ref().unwrap().handle;
    let closed = ended_page(false);
    pages::step(
        &mut pages,
        &env,
        &client,
        pages::Event::Told {
            told: Told::Page {
                handle,
                page: jig_client::Readout::Jig(c::ReadResult::Value(c::ReadValue::Tasks(Box::new(closed.clone())))),
            },
        },
        &mut out,
    );
    assert!(out.is_empty());
    page_act(&mut pages, &client, Act::Amend);
    page_act(&mut pages, &client, Act::AmendWords { words: Box::from(WORDS) });
    let document = pages.task().unwrap().document;
    for filter in [Filter::Live, Filter::All] {
        let address = crate::Address::Task { task: f::TASK, filter };
        let mut chosen = page_act(&mut pages, &client, Act::PlanFilter { filter });
        assert_eq!(chosen.pop(), Some(Request::Show { address: address.clone() }));
        assert!(chosen.is_empty());
        pages::step(&mut pages, &env, &client, pages::Event::Went { address }, &mut out);
        let page = pages.task().unwrap();
        assert_eq!(page.filter, filter);
        assert_eq!(page.document, document);
        assert_eq!(page.ended.as_ref().unwrap().page.as_deref(), Some(&closed));
        assert_eq!(page.amending.as_ref().unwrap().words.as_ref(), WORDS);
        assert!(out.is_empty(), "the ended delegates stay read under either filter");
    }
}
