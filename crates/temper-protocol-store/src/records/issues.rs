#![expect(clippy::manual_map, reason = "step translations use explicit matches without closures")]
use super::{FITS, names, w};
use alloc::boxed::Box;
use skein_lib::{List, Wall};
use temper_engine_domain_forge as forge;
use temper_engine_domain_forge_issues as d;

fn encode_phase(value: d::Phase) -> w::Phase {
    match value {
        d::Phase::Waiting => w::Phase::Waiting,
        d::Phase::Active => w::Phase::Active,
        d::Phase::Settling => w::Phase::Settling,
        d::Phase::Held => w::Phase::Held,
        d::Phase::Done => w::Phase::Done,
        d::Phase::Failed => w::Phase::Failed,
        d::Phase::Cancelled => w::Phase::Cancelled,
    }
}
fn decode_phase(value: w::Phase) -> d::Phase {
    match value {
        w::Phase::Waiting => d::Phase::Waiting,
        w::Phase::Active => d::Phase::Active,
        w::Phase::Settling => d::Phase::Settling,
        w::Phase::Held => d::Phase::Held,
        w::Phase::Done => d::Phase::Done,
        w::Phase::Failed => d::Phase::Failed,
        w::Phase::Cancelled => d::Phase::Cancelled,
    }
}
fn encode_key(value: d::MilestoneKey) -> w::MilestoneKey {
    match value {
        d::MilestoneKey::Lifecycle { task, position } => w::MilestoneKey::Lifecycle(
            w::Lifecycle::new(&w::CEILINGS, w::LifecycleParts { task, position }).expect(FITS),
        ),
        d::MilestoneKey::TaskRevision { task, revision } => w::MilestoneKey::TaskRevision(
            w::TaskRevision::new(&w::CEILINGS, w::TaskRevisionParts { task, revision }).expect(FITS),
        ),
        d::MilestoneKey::PlanAccepted => w::MilestoneKey::PlanAccepted,
        d::MilestoneKey::Revision(number) => w::MilestoneKey::Revision(number_record(number)),
        d::MilestoneKey::ChangeLanded(number) => w::MilestoneKey::ChangeLanded(number_record(number)),
        d::MilestoneKey::ChangeHeld(number) => w::MilestoneKey::ChangeHeld(number_record(number)),
        d::MilestoneKey::Report(number) => w::MilestoneKey::Report(number_record(number)),
        d::MilestoneKey::Finished => w::MilestoneKey::Finished,
    }
}
fn number_record(number: u64) -> w::Number {
    w::Number::new(&w::CEILINGS, w::NumberParts { number }).expect(FITS)
}
fn decode_key(value: w::MilestoneKey) -> d::MilestoneKey {
    match value {
        w::MilestoneKey::Lifecycle(value) => {
            let w::LifecycleParts { task, position } = value.into_parts();
            d::MilestoneKey::Lifecycle { task, position }
        }
        w::MilestoneKey::TaskRevision(value) => {
            let w::TaskRevisionParts { task, revision } = value.into_parts();
            d::MilestoneKey::TaskRevision { task, revision }
        }
        w::MilestoneKey::PlanAccepted => d::MilestoneKey::PlanAccepted,
        w::MilestoneKey::Revision(value) => d::MilestoneKey::Revision(value.number()),
        w::MilestoneKey::ChangeLanded(value) => d::MilestoneKey::ChangeLanded(value.number()),
        w::MilestoneKey::ChangeHeld(value) => d::MilestoneKey::ChangeHeld(value.number()),
        w::MilestoneKey::Report(value) => d::MilestoneKey::Report(value.number()),
        w::MilestoneKey::Finished => d::MilestoneKey::Finished,
    }
}
fn encode_projected(value: &d::Projected) -> w::Projected {
    let d::Projected { opened, digest, last_body, body_revision, milestones, closed } = value;
    let mut keys = List::with_capacity(u32::try_from(milestones.len()).expect("admitted count"));
    for key in milestones {
        keys.push(encode_key(*key)).expect("same count");
    }
    let last_body = match last_body {
        Some(value) => Some(value.as_nanos()),
        None => None,
    };
    w::Projected::new(
        &w::CEILINGS,
        w::ProjectedParts {
            opened: *opened,
            digest: *digest,
            last_body,
            body_revision: *body_revision,
            milestones: keys,
            closed: *closed,
        },
    )
    .expect(FITS)
}
fn decode_projected(value: w::Projected) -> d::Projected {
    let w::ProjectedParts { opened, digest, last_body, body_revision, milestones, closed } = value.into_parts();
    let mut keys = List::with_capacity(milestones.len());
    for key in milestones.into_boxed() {
        keys.push(decode_key(key)).expect("same count");
    }
    let last_body = match last_body {
        Some(value) => Some(Wall::from_nanos(value)),
        None => None,
    };
    d::Projected { opened, digest, last_body, body_revision, milestones: keys.into_boxed(), closed }
}
fn encode_view(value: &d::GoalView) -> w::GoalView {
    let d::GoalView { phase, goal, repository, title, goal_text, plan, milestones, finished } = value;
    let mut items = List::with_capacity(u32::try_from(plan.len()).expect("admitted count"));
    for d::PlanItem { text, phase } in plan {
        let item = w::PlanItem::new(
            &w::CEILINGS,
            w::PlanItemParts { text: text.as_bytes().into(), phase: encode_phase(*phase) },
        )
        .expect(FITS);
        items.push(item).expect("same count");
    }
    let mut comments = List::with_capacity(u32::try_from(milestones.len()).expect("admitted count"));
    for d::Milestone { key, text } in milestones {
        let item =
            w::Milestone::new(&w::CEILINGS, w::MilestoneParts { key: encode_key(*key), text: text.as_bytes().into() })
                .expect(FITS);
        comments.push(item).expect("same count");
    }
    w::GoalView::new(
        &w::CEILINGS,
        w::GoalViewParts {
            phase: encode_phase(*phase),
            goal: *goal,
            repository: *repository,
            title: title.as_bytes().into(),
            goal_text: goal_text.as_bytes().into(),
            plan: items,
            milestones: comments,
            finished: match finished {
                Some(value) => Some(value.as_bytes().into()),
                None => None,
            },
        },
    )
    .expect(FITS)
}
fn decode_view(value: w::GoalView) -> d::GoalView {
    let w::GoalViewParts { phase, goal, repository, title, goal_text, plan, milestones, finished } = value.into_parts();
    let mut items = List::with_capacity(plan.len());
    for item in plan.into_boxed() {
        let w::PlanItemParts { text, phase } = item.into_parts();
        items.push(d::PlanItem { text: decode_text(text), phase: decode_phase(phase) }).expect("same count");
    }
    let mut comments = List::with_capacity(milestones.len());
    for item in milestones.into_boxed() {
        let w::MilestoneParts { key, text } = item.into_parts();
        comments.push(d::Milestone { key: decode_key(key), text: decode_text(text) }).expect("same count");
    }
    d::GoalView {
        phase: decode_phase(phase),
        goal,
        repository,
        title: decode_text(title),
        goal_text: decode_text(goal_text),
        plan: items.into_boxed(),
        milestones: comments.into_boxed(),
        finished: match finished {
            Some(value) => Some(decode_text(value)),
            None => None,
        },
    }
}
pub(super) fn encode(value: &forge::IssueRow) -> w::IssueRow {
    let forge::IssueRow { goal, repository, state, number, pending, before, failed, desired, closing_generation } =
        value;
    let before = match before {
        Some(value) => Some(encode_projected(value)),
        None => None,
    };
    let desired = match desired {
        Some(value) => Some(encode_view(value)),
        None => None,
    };
    w::IssueRow::new(
        &w::CEILINGS,
        w::IssueRowParts {
            goal: *goal,
            repository: names::encode_repository(*repository),
            state: encode_projected(state),
            number: *number,
            pending: *pending,
            before,
            failed: *failed,
            desired,
            closing_generation: *closing_generation,
        },
    )
    .expect(FITS)
}
pub(super) fn decode(value: w::IssueRow) -> forge::IssueRow {
    let w::IssueRowParts { goal, repository, state, number, pending, before, failed, desired, closing_generation } =
        value.into_parts();
    let before = match before {
        Some(value) => Some(decode_projected(value)),
        None => None,
    };
    let desired = match desired {
        Some(value) => Some(decode_view(value)),
        None => None,
    };
    forge::IssueRow {
        goal,
        repository: names::decode_repository(repository),
        state: decode_projected(state),
        number,
        pending,
        before,
        failed,
        desired,
        closing_generation,
    }
}
#[expect(clippy::disallowed_methods, reason = "the generated protocol decoder already validated this text")]
fn decode_text(bytes: Box<[u8]>) -> Box<str> {
    core::str::from_utf8(&bytes).expect("generated text validated UTF-8").into()
}
