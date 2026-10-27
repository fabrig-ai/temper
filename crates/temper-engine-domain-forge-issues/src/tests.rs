//! Projection cells, intervals, bounds and keyed history.
use alloc::boxed::Box;
use skein_lib::{Duration, List, Wall, bytes::copy_of};

use crate::{Decision, Effect, GoalView, Key, Limits, Milestone, MilestoneKey, PlanItem, Projected, project};

const LIMITS: Limits = Limits {
    plan_items: 4,
    milestones: 8,
    title_bytes: 80,
    body_bytes: 512,
    comment_bytes: 120,
    interval: Duration::from_secs(60),
};

fn at(seconds: u64) -> Wall {
    Wall::from_nanos(seconds.saturating_mul(1_000_000_000))
}

fn goal() -> GoalView {
    GoalView {
        phase: crate::Phase::Waiting,
        goal: 7,
        repository: 12,
        title: "Ship the goal".into(),
        goal_text: "A goal with a plan.".into(),
        plan: Box::new([PlanItem { text: "First step".into(), phase: crate::Phase::Waiting }]),
        milestones: Box::new([]),
        finished: None,
    }
}

#[test]
fn opening_is_keyed_and_the_body_is_a_task_list() {
    let opened = project(&goal(), &Projected::default(), at(1), &LIMITS);
    assert_eq!(
        opened.decision,
        Decision::Effect(Effect::Open {
            goal: 7,
            repository: 12,
            key: Key::Open,
            title: "Ship the goal".into(),
            body: copy_of(b"A goal with a plan.\nState: waiting\n- [ ] First step (waiting)\n"),
        })
    );
    assert_eq!(project(&goal(), &opened.projected, at(1), &LIMITS).decision, Decision::None);
    assert_eq!(project(&goal(), &Projected::default(), at(1), &LIMITS), opened);
}

#[test]
fn a_changed_plan_writes_once_after_its_interval() {
    let old = project(&goal(), &Projected::default(), at(1), &LIMITS).projected;
    let mut changed = goal();
    changed.plan = Box::new([PlanItem { text: "First step".into(), phase: crate::Phase::Done }]);
    assert_eq!(project(&changed, &old, at(30), &LIMITS).decision, Decision::Wait(at(61)));
    let rewritten = project(&changed, &old, at(61), &LIMITS);
    assert_eq!(
        rewritten.decision,
        Decision::Effect(Effect::Body {
            goal: 7,
            repository: 12,
            key: Key::Body(1),
            title: "Ship the goal".into(),
            body: copy_of(b"A goal with a plan.\nState: waiting\n- [x] First step (done)\n"),
        })
    );
    assert_eq!(project(&changed, &rewritten.projected, at(61), &LIMITS).decision, Decision::None);
    changed.plan = Box::new([PlanItem { text: "Second step".into(), phase: crate::Phase::Waiting }]);
    assert_eq!(project(&changed, &rewritten.projected, at(120), &LIMITS).decision, Decision::Wait(at(121)));
    let second = project(&changed, &rewritten.projected, at(121), &LIMITS);
    assert_eq!(
        second.decision,
        Decision::Effect(Effect::Body {
            goal: 7,
            repository: 12,
            key: Key::Body(2),
            title: "Ship the goal".into(),
            body: copy_of(b"A goal with a plan.\nState: waiting\n- [ ] Second step (waiting)\n"),
        })
    );
}

#[test]
fn milestones_are_comments_once_even_while_a_body_waits() {
    let old = project(&goal(), &Projected::default(), at(1), &LIMITS).projected;
    let mut changed = goal();
    changed.plan = Box::new([PlanItem { text: "First step".into(), phase: crate::Phase::Done }]);
    changed.milestones = Box::new([
        Milestone { key: MilestoneKey::PlanAccepted, text: "Plan accepted".into() },
        Milestone { key: MilestoneKey::Revision(2), text: "Revised because tests changed".into() },
    ]);
    let first = project(&changed, &old, at(30), &LIMITS);
    assert_eq!(
        first.decision,
        Decision::Effect(Effect::Comment {
            goal: 7,
            repository: 12,
            key: Key::Milestone(MilestoneKey::PlanAccepted),
            body: "Plan accepted".into(),
        })
    );
    let second = project(&changed, &first.projected, at(30), &LIMITS);
    assert_eq!(
        second.decision,
        Decision::Effect(Effect::Comment {
            goal: 7,
            repository: 12,
            key: Key::Milestone(MilestoneKey::Revision(2)),
            body: "Revised because tests changed".into(),
        })
    );
    assert_eq!(project(&changed, &second.projected, at(30), &LIMITS).decision, Decision::Wait(at(61)));
}

#[test]
fn final_comment_precedes_close_and_close_is_stable() {
    let old = project(&goal(), &Projected::default(), at(0), &LIMITS).projected;
    let mut completed = goal();
    completed.finished = Some("Goal finished".into());
    let commented = project(&completed, &old, at(1), &LIMITS);
    assert_eq!(
        commented.decision,
        Decision::Effect(Effect::Comment {
            goal: 7,
            repository: 12,
            key: Key::Milestone(MilestoneKey::Finished),
            body: "Goal finished".into(),
        })
    );
    let closed = project(&completed, &commented.projected, at(1), &LIMITS);
    assert_eq!(closed.decision, Decision::Effect(Effect::Close { goal: 7, repository: 12, key: Key::Close }));
    assert_eq!(project(&completed, &closed.projected, at(2), &LIMITS).decision, Decision::None);
}

#[test]
fn changed_final_body_is_written_before_close() {
    let old = project(&goal(), &Projected::default(), at(0), &LIMITS).projected;
    let mut completed = goal();
    completed.plan = Box::new([PlanItem { text: "First step".into(), phase: crate::Phase::Done }]);
    completed.finished = Some("Goal finished".into());
    assert_eq!(project(&completed, &old, at(1), &LIMITS).decision, Decision::Wait(at(60)));
    let rewritten = project(&completed, &old, at(60), &LIMITS);
    assert_eq!(
        rewritten.decision,
        Decision::Effect(Effect::Body {
            goal: 7,
            repository: 12,
            key: Key::Body(1),
            title: "Ship the goal".into(),
            body: copy_of(b"A goal with a plan.\nState: waiting\n- [x] First step (done)\n"),
        })
    );
    let commented = project(&completed, &rewritten.projected, at(60), &LIMITS);
    assert_eq!(
        commented.decision,
        Decision::Effect(Effect::Comment {
            goal: 7,
            repository: 12,
            key: Key::Milestone(MilestoneKey::Finished),
            body: "Goal finished".into(),
        })
    );
    assert_eq!(
        project(&completed, &commented.projected, at(60), &LIMITS).decision,
        Decision::Effect(Effect::Close { goal: 7, repository: 12, key: Key::Close })
    );
}

#[test]
fn contradictory_history_is_held_without_mutation() {
    let old = project(&goal(), &Projected::default(), at(0), &LIMITS).projected;
    let mut duplicate = goal();
    duplicate.milestones = Box::new([
        Milestone { key: MilestoneKey::PlanAccepted, text: "accepted".into() },
        Milestone { key: MilestoneKey::PlanAccepted, text: "again".into() },
    ]);
    assert_eq!(project(&duplicate, &old, at(60), &LIMITS).decision, Decision::Hold);
    let mut retreated = old.clone();
    retreated.milestones = Box::new([MilestoneKey::Revision(2)]);
    assert_eq!(project(&goal(), &retreated, at(60), &LIMITS).decision, Decision::Hold);
}

#[test]
fn projection_steps_repeat_from_identical_snapshots() {
    for changed in [false, true] {
        for finished in [false, true] {
            for milestone in [false, true] {
                let mut view = goal();
                if changed {
                    view.plan = Box::new([PlanItem { text: "First step".into(), phase: crate::Phase::Done }]);
                }
                if finished {
                    view.finished = Some("done".into());
                }
                if milestone {
                    view.milestones =
                        Box::new([Milestone { key: MilestoneKey::PlanAccepted, text: "accepted".into() }]);
                }
                let previous = project(&goal(), &Projected::default(), at(0), &LIMITS).projected;
                assert_eq!(project(&view, &previous, at(30), &LIMITS), project(&view, &previous, at(30), &LIMITS));
            }
        }
    }
}

// The deployed box's issue policy and client operation bound.
const BODY_BYTES: u32 = 512;
const OP_BYTES: u32 = 512;
const TITLE_BYTES: u32 = 128;
const COMMENT_BYTES: u32 = 256;
const PLAN_ITEMS: u32 = 8;
const MILESTONES: u32 = 8;
const LONG_GOAL_BYTES: usize = 300;
const LONG_ITEM_BYTES: usize = 200;
const SHORT_ITEM_BYTES: usize = 10;
const HISTORY_WORDS: [&str; 10] = [
    "Milestone 0",
    "Milestone 1",
    "Milestone 2",
    "Milestone 3",
    "Milestone 4",
    "Milestone 5",
    "Milestone 6",
    "Milestone 7",
    "Milestone 8",
    "Milestone 9",
];
const BODY_LINES: u32 = PLAN_ITEMS + 2; // The goal, state, and one line per item.
const INDIVIDUAL_COMMENTS: usize = 6; // Eight slots reserve the count and Finished.
const BOX_LIMITS: Limits = Limits {
    body_bytes: BODY_BYTES,
    title_bytes: TITLE_BYTES,
    comment_bytes: COMMENT_BYTES,
    plan_items: PLAN_ITEMS,
    milestones: MILESTONES,
    ..LIMITS
};

fn long_goal(short_items: usize) -> GoalView {
    let mut view = goal();
    view.goal_text = "g".repeat(LONG_GOAL_BYTES).into_boxed_str();
    let mut plan = List::with_capacity(PLAN_ITEMS);
    for index in 0..PLAN_ITEMS {
        let bytes =
            if usize::try_from(index).expect("small index") < short_items { SHORT_ITEM_BYTES } else { LONG_ITEM_BYTES };
        plan.push(PlanItem { text: "t".repeat(bytes).into_boxed_str(), phase: crate::Phase::Waiting })
            .expect("plan capacity");
    }
    view.plan = plan.into_boxed();
    view
}

#[expect(clippy::disallowed_methods, reason = "the assertion inspects the projection's rendered UTF-8 body")]
fn opened_body(view: &GoalView, limits: &Limits) -> Box<str> {
    let Decision::Effect(Effect::Open { body, .. }) = project(view, &Projected::default(), at(0), limits).decision
    else {
        panic!("projection opens");
    };
    Box::from(core::str::from_utf8(&body).expect("UTF-8 body"))
}

fn body_lines(body: &str) -> Box<[&str]> {
    let mut lines = List::with_capacity(BODY_LINES);
    for line in body.lines() {
        lines.push(line).expect("body line capacity");
    }
    lines.into_boxed()
}

#[test]
fn a_long_goal_and_nine_long_items_are_cut_to_the_body() {
    let body = opened_body(&long_goal(0), &BOX_LIMITS);
    assert!(body.len() <= usize::try_from(BODY_BYTES).expect("box limit"));
    let lines = body_lines(&body);
    assert!(lines[0].ends_with('…'));
    assert_eq!(lines[1], "State: waiting");
    assert_eq!(lines.len(), usize::try_from(PLAN_ITEMS).expect("box limit") + 2);
    for line in &lines[2..] {
        assert!(line.starts_with("- [ ] "));
        assert!(line.ends_with("… (waiting)"));
    }
}

#[test]
fn short_items_leave_their_room_to_long_ones() {
    const SHORT_ITEMS: usize = 2;
    // Skeleton 152 bytes; goal gets 180 of the remaining 360, two items
    // use 10 each, leaving 160: four shares of 27 and two of 26.
    let body = opened_body(&long_goal(SHORT_ITEMS), &BOX_LIMITS);
    let lines = body_lines(&body);
    assert_eq!(&lines[2..4], ["- [ ] tttttttttt (waiting)", "- [ ] tttttttttt (waiting)"]);
    let mut lengths = List::with_capacity(PLAN_ITEMS);
    for line in &lines[4..] {
        lengths.push(line.len() - "- [ ]  (waiting)".len()).expect("plan capacity");
        assert!(line.ends_with("… (waiting)"));
    }
    assert_eq!(lengths.as_slice(), [27, 27, 27, 27, 26, 26]);
    assert_eq!(body.len(), usize::try_from(BODY_BYTES).expect("box limit"));
}

#[test]
fn a_body_that_fits_is_rendered_as_before() {
    assert_eq!(
        opened_body(&goal(), &BOX_LIMITS).as_ref(),
        "A goal with a plan.\nState: waiting\n- [ ] First step (waiting)\n"
    );
}

#[test]
fn the_edit_fits_the_operation_with_its_title() {
    const EDIT_TITLE_BYTES: usize = 120;
    let mut view = long_goal(0);
    view.title = "s".repeat(EDIT_TITLE_BYTES).into_boxed_str();
    let policy = Limits {
        body_bytes: BODY_BYTES.min(OP_BYTES - u32::try_from(EDIT_TITLE_BYTES).expect("fixture title size")),
        ..BOX_LIMITS
    };
    let before = project(&goal(), &Projected::default(), at(0), &policy).projected;
    let Decision::Effect(Effect::Body { title, body, .. }) = project(&view, &before, at(60), &policy).decision else {
        panic!("changed body is due");
    };
    assert!(title.len() + body.len() <= usize::try_from(OP_BYTES).expect("box limit"));
}

fn history_goal(finished: bool) -> GoalView {
    let mut view = goal();
    let mut milestones = List::with_capacity(u32::try_from(HISTORY_WORDS.len()).expect("fixture history"));
    for (number, words) in HISTORY_WORDS.into_iter().enumerate() {
        milestones
            .push(Milestone {
                key: MilestoneKey::Revision(u64::try_from(number).expect("fixture position")),
                text: Box::from(words),
            })
            .expect("history capacity");
    }
    view.milestones = milestones.into_boxed();
    if finished {
        view.finished = Some("Goal finished".into());
    }
    view
}

#[test]
fn milestones_past_the_bound_are_counted_in_one_comment_before_finished() {
    let live = crate::gather(None, history_goal(false), &BOX_LIMITS).expect("bounded history");
    let mut state = project(&live, &Projected::default(), at(0), &BOX_LIMITS).projected;
    let mut comments = List::with_capacity(MILESTONES);
    for _ in 0..INDIVIDUAL_COMMENTS {
        let step = project(&live, &state, at(0), &BOX_LIMITS);
        let Decision::Effect(Effect::Comment { body, .. }) = step.decision else {
            panic!("prefix comment");
        };
        comments.push(body).expect("comment capacity");
        state = step.projected;
    }
    assert_eq!(project(&live, &state, at(0), &BOX_LIMITS).decision, Decision::None);
    let finished = crate::gather(Some(&live), history_goal(true), &BOX_LIMITS).expect("bounded finish");
    assert_eq!(finished.milestones.len(), INDIVIDUAL_COMMENTS + 1);
    assert_eq!(crate::gather(Some(&finished), finished.clone(), &BOX_LIMITS), Some(finished.clone()));
    for _ in 0_u32..2 {
        let step = project(&finished, &state, at(0), &BOX_LIMITS);
        let Decision::Effect(Effect::Comment { body, .. }) = step.decision else {
            panic!("finishing comments");
        };
        comments.push(body).expect("comment capacity");
        state = step.projected;
    }
    assert_eq!(comments.len(), MILESTONES);
    for (comment, words) in comments.iter().zip(HISTORY_WORDS).take(INDIVIDUAL_COMMENTS) {
        assert_eq!(comment.as_ref(), words);
    }
    assert_eq!(
        comments.get(u32::try_from(INDIVIDUAL_COMMENTS).expect("prefix")).expect("count").as_ref(),
        "4 more milestones; the last: Milestone 9"
    );
    assert_eq!(comments.last().expect("Finished").as_ref(), "Goal finished");
    assert_eq!(state.milestones.last(), Some(&MilestoneKey::Finished));
    let closed = project(&finished, &state, at(0), &BOX_LIMITS);
    assert_eq!(
        closed.decision,
        Decision::Effect(Effect::Close { goal: finished.goal, repository: finished.repository, key: Key::Close })
    );
    assert_eq!(project(&finished, &closed.projected, at(0), &BOX_LIMITS).decision, Decision::None);
}

#[test]
fn a_long_milestone_is_cut_to_a_comment() {
    const LONG_COMMENT_BYTES: usize = 400;
    let mut view = goal();
    view.milestones = Box::new([Milestone {
        key: MilestoneKey::PlanAccepted,
        text: "λ".repeat(LONG_COMMENT_BYTES / 2).into_boxed_str(),
    }]);
    let before = project(&view, &Projected::default(), at(0), &BOX_LIMITS).projected;
    let Decision::Effect(Effect::Comment { body, .. }) = project(&view, &before, at(0), &BOX_LIMITS).decision else {
        panic!("milestone comment");
    };
    assert!(body.len() <= usize::try_from(COMMENT_BYTES).expect("box limit"));
    assert!(body.ends_with('…'));
}

#[test]
fn a_title_is_cut_at_its_utf8_boundary() {
    let mut view = goal();
    view.title = "λ".repeat(usize::try_from(TITLE_BYTES).expect("box limit")).into_boxed_str();
    let Decision::Effect(Effect::Open { title, .. }) =
        project(&view, &Projected::default(), at(0), &BOX_LIMITS).decision
    else {
        panic!("issue opens");
    };
    assert_eq!(title.len(), 127); // 62 two-byte letters plus the three-byte marker.
    assert!(title.ends_with('…'));
}

#[test]
fn incremental_milestones_keep_the_count_across_empty_feeds() {
    let mut prior = None;
    for milestone in history_goal(false).milestones {
        let mut next = goal();
        next.milestones = Box::new([milestone]);
        prior = crate::gather(prior.as_ref(), next, &BOX_LIMITS);
    }
    let prior = prior.expect("incremental history");
    let mut finished = goal();
    finished.finished = Some("Goal finished".into());
    let gathered = crate::gather(Some(&prior), finished, &BOX_LIMITS).expect("finish without new milestones");
    assert_eq!(gathered.milestones.len(), INDIVIDUAL_COMMENTS + 1);
    assert_eq!(gathered.milestones.last().expect("count").text.as_ref(), "4 more milestones; the last: Milestone 9");
    assert_eq!(gathered.milestones.last().expect("key").key, MilestoneKey::Revision(9));
}
