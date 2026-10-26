use super::names::{decode_name, decode_repository, encode_name, encode_repository};
use super::{Bounds, FITS, Undecodable, w};
use alloc::boxed::Box;
use skein_forge::api;
use skein_lib::List;
use temper_engine_domain_forge as d;

fn encode_topic(value: &d::Topic) -> w::Topic {
    match value {
        d::Topic::Landings { repository, branch } => w::Topic::Landings(
            w::LandingsTopic::new(
                &w::CEILINGS,
                w::LandingsTopicParts { repository: encode_repository(*repository), branch: branch.clone() },
            )
            .expect(FITS),
        ),
        d::Topic::Ci { repository, head } => w::Topic::Ci(
            w::CiTopic::new(&w::CEILINGS, w::CiTopicParts { repository: encode_repository(*repository), head: *head })
                .expect(FITS),
        ),
        d::Topic::Pull { repository, number } => w::Topic::Pull(
            w::NumberTopic::new(
                &w::CEILINGS,
                w::NumberTopicParts { repository: encode_repository(*repository), number: *number },
            )
            .expect(FITS),
        ),
        d::Topic::Participation { repository, number } => w::Topic::Participation(
            w::NumberTopic::new(
                &w::CEILINGS,
                w::NumberTopicParts { repository: encode_repository(*repository), number: *number },
            )
            .expect(FITS),
        ),
    }
}
fn decode_topic(value: w::Topic) -> d::Topic {
    match value {
        w::Topic::Landings(value) => {
            let w::LandingsTopicParts { repository, branch } = value.into_parts();
            d::Topic::Landings { repository: decode_repository(repository), branch }
        }
        w::Topic::Ci(value) => {
            let w::CiTopicParts { repository, head } = value.into_parts();
            d::Topic::Ci { repository: decode_repository(repository), head }
        }
        w::Topic::Pull(value) => {
            let w::NumberTopicParts { repository, number } = value.into_parts();
            d::Topic::Pull { repository: decode_repository(repository), number }
        }
        w::Topic::Participation(value) => {
            let w::NumberTopicParts { repository, number } = value.into_parts();
            d::Topic::Participation { repository: decode_repository(repository), number }
        }
    }
}
pub(super) fn encode_names(task: u64, resources: &[d::Name]) -> w::Names {
    let mut names = List::with_capacity(u32::try_from(resources.len()).expect("admitted count"));
    for name in resources {
        names.push(encode_name(name)).expect("same count");
    }
    w::Names::new(&w::CEILINGS, w::NamesParts { task, resources: names }).expect(FITS)
}
pub(super) fn decode_names(resources: List<w::Name>, bounds: &Bounds) -> Result<Box<[d::Name]>, Undecodable> {
    let mut names = List::with_capacity(resources.len());
    for name in resources.into_boxed() {
        names.push(decode_name(name, bounds)?).expect("same count");
    }
    Ok(names.into_boxed())
}

pub(super) fn encode_ci(value: api::Ci) -> w::Ci {
    match value {
        api::Ci::None => w::Ci::None,
        api::Ci::Pending => w::Ci::Pending,
        api::Ci::Passed => w::Ci::Passed,
        api::Ci::Failed => w::Ci::Failed,
    }
}
pub(super) fn decode_ci(value: w::Ci) -> api::Ci {
    match value {
        w::Ci::None => api::Ci::None,
        w::Ci::Pending => api::Ci::Pending,
        w::Ci::Passed => api::Ci::Passed,
        w::Ci::Failed => api::Ci::Failed,
    }
}

pub(super) fn encode_state(value: api::State) -> w::ItemState {
    match value {
        api::State::Open => w::ItemState::Open,
        api::State::Closed => w::ItemState::Closed,
    }
}
pub(super) fn decode_state(value: w::ItemState) -> api::State {
    match value {
        w::ItemState::Open => api::State::Open,
        w::ItemState::Closed => api::State::Closed,
    }
}

fn encode_ending(value: d::ReleaseEnding) -> w::ReleaseEnding {
    match value {
        d::ReleaseEnding::Done => w::ReleaseEnding::Done,
        d::ReleaseEnding::Failed => w::ReleaseEnding::Failed,
        d::ReleaseEnding::Cancelled => w::ReleaseEnding::Cancelled,
    }
}
fn decode_ending(value: w::ReleaseEnding) -> d::ReleaseEnding {
    match value {
        w::ReleaseEnding::Done => d::ReleaseEnding::Done,
        w::ReleaseEnding::Failed => d::ReleaseEnding::Failed,
        w::ReleaseEnding::Cancelled => d::ReleaseEnding::Cancelled,
    }
}

pub(super) fn encode_branch_head(value: &d::BranchHead) -> w::BranchHead {
    let d::BranchHead { name, commit, owned } = value;
    w::BranchHead::new(&w::CEILINGS, w::BranchHeadParts { name: encode_name(name), commit: *commit, owned: *owned })
        .expect(FITS)
}
pub(super) fn decode_branch_head(value: w::BranchHead, bounds: &Bounds) -> Result<d::BranchHead, Undecodable> {
    let w::BranchHeadParts { name, commit, owned } = value.into_parts();
    Ok(d::BranchHead { name: decode_name(name, bounds)?, commit, owned })
}

pub(super) fn encode_pull_state(value: &d::PullState) -> w::PullState {
    let d::PullState { name, head, ci, state } = value;
    w::PullState::new(
        &w::CEILINGS,
        w::PullStateParts { name: encode_name(name), head: *head, ci: encode_ci(*ci), state: encode_state(*state) },
    )
    .expect(FITS)
}
pub(super) fn decode_pull_state(value: w::PullState, bounds: &Bounds) -> Result<d::PullState, Undecodable> {
    let w::PullStateParts { name, head, ci, state } = value.into_parts();
    Ok(d::PullState { name: decode_name(name, bounds)?, head, ci: decode_ci(ci), state: decode_state(state) })
}

pub(super) fn encode_ci_state(value: &d::CiState) -> w::CiState {
    let d::CiState { repository, head, status } = value;
    w::CiState::new(
        &w::CEILINGS,
        w::CiStateParts { repository: encode_repository(*repository), head: *head, status: encode_ci(*status) },
    )
    .expect(FITS)
}
pub(super) fn decode_ci_state(value: w::CiState) -> d::CiState {
    let w::CiStateParts { repository, head, status } = value.into_parts();
    d::CiState { repository: decode_repository(repository), head, status: decode_ci(status) }
}

#[expect(clippy::manual_map, reason = "step translations use explicit matches without closures")]
pub(super) fn encode_subscriber(value: &d::Subscriber) -> w::Subscriber {
    let d::Subscriber { task, number, topic, own_change, goal_tasks, paths } = value;
    w::Subscriber::new(
        &w::CEILINGS,
        w::SubscriberParts {
            task: *task,
            number: *number,
            topic: encode_topic(topic),
            own_change: *own_change,
            goal_tasks: match goal_tasks {
                Some(v) => Some(encode_numbers(v)),
                None => None,
            },
            paths: encode_paths(paths),
        },
    )
    .expect(FITS)
}
#[expect(clippy::manual_map, reason = "step translations use explicit matches without closures")]
pub(super) fn decode_subscriber(value: w::Subscriber) -> d::Subscriber {
    let w::SubscriberParts { task, number, topic, own_change, goal_tasks, paths } = value.into_parts();
    d::Subscriber {
        task,
        number,
        topic: decode_topic(topic),
        own_change,
        goal_tasks: match goal_tasks {
            Some(v) => Some(v.into_boxed()),
            None => None,
        },
        paths: paths.into_boxed(),
    }
}

pub(super) fn encode_landed(commit: api::Commit, task: u64) -> w::Landed {
    w::Landed::new(&w::CEILINGS, w::LandedParts { commit, task }).expect(FITS)
}
#[expect(clippy::manual_map, reason = "step translations use explicit matches without closures")]
pub(super) fn encode_release(value: &d::ReleaseRow) -> w::ReleaseRow {
    let d::ReleaseRow { task, root, ending, close_pull, comment, pending, failed, effects_settled, releasing } = value;
    let pending = match pending {
        Some((entry, name)) => {
            let name = match name {
                Some(name) => Some(encode_name(name)),
                None => None,
            };
            Some(w::PendingRelease::new(&w::CEILINGS, w::PendingReleaseParts { entry: *entry, name }).expect(FITS))
        }
        None => None,
    };
    w::ReleaseRow::new(
        &w::CEILINGS,
        w::ReleaseRowParts {
            task: *task,
            root: *root,
            ending: encode_ending(*ending),
            close_pull: *close_pull,
            comment: comment.clone(),
            pending_release: pending,
            failed: *failed,
            effects_settled: *effects_settled,
            releasing: *releasing,
        },
    )
    .expect(FITS)
}
pub(super) fn decode_release(value: w::ReleaseRow, bounds: &Bounds) -> Result<d::ReleaseRow, Undecodable> {
    let w::ReleaseRowParts {
        task,
        root,
        ending,
        close_pull,
        comment,
        pending_release: pending,
        failed,
        effects_settled,
        releasing,
    } = value.into_parts();
    let pending = match pending {
        Some(value) => {
            let w::PendingReleaseParts { entry, name } = value.into_parts();
            let name = match name {
                Some(name) => Some(decode_name(name, bounds)?),
                None => None,
            };
            Some((entry, name))
        }
        None => None,
    };
    Ok(d::ReleaseRow {
        task,
        root,
        ending: decode_ending(ending),
        close_pull,
        comment,
        pending,
        failed,
        effects_settled,
        releasing,
    })
}
fn encode_numbers(values: &[u64]) -> List<u64> {
    let mut numbers = List::with_capacity(u32::try_from(values.len()).expect("admitted count"));
    for number in values {
        numbers.push(*number).expect("same count");
    }
    numbers
}
fn encode_paths(values: &[Box<[u8]>]) -> List<Box<[u8]>> {
    let mut paths = List::with_capacity(u32::try_from(values.len()).expect("admitted count"));
    for path in values {
        paths.push(path.clone()).expect("same count");
    }
    paths
}
