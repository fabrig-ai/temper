use super::api;
use super::{Bounds, FITS, Undecodable, names, topics, w};
use alloc::boxed::Box;
use skein_forge::api as d;
use skein_lib::{List, Reader, Writer};
pub(super) fn encode_permission(value: d::Permission) -> w::Permission {
    match value {
        d::Permission::None => w::Permission::None,
        d::Permission::Read => w::Permission::Read,
        d::Permission::Write => w::Permission::Write,
        d::Permission::Admin => w::Permission::Admin,
    }
}
pub(super) fn decode_permission(value: w::Permission) -> d::Permission {
    match value {
        w::Permission::None => d::Permission::None,
        w::Permission::Read => d::Permission::Read,
        w::Permission::Write => d::Permission::Write,
        w::Permission::Admin => d::Permission::Admin,
    }
}
fn encode_branch_creation(value: d::BranchCreation) -> w::BranchCreation {
    match value {
        d::BranchCreation::Created => w::BranchCreation::Created,
        d::BranchCreation::Exists => w::BranchCreation::Exists,
    }
}
fn decode_branch_creation(value: w::BranchCreation) -> d::BranchCreation {
    match value {
        w::BranchCreation::Created => d::BranchCreation::Created,
        w::BranchCreation::Exists => d::BranchCreation::Exists,
    }
}
fn encode_reviewers(values: &[u64]) -> List<u64> {
    let mut result = List::with_capacity(u32::try_from(values.len()).expect("admitted count"));
    for value in values {
        result.push(*value).expect("same count");
    }
    result
}
pub(super) fn encode_pull(value: &d::Pull) -> w::Pull {
    let d::Pull {
        reviewers,
        number,
        state,
        head,
        base,
        commit,
        base_commit,
        merged,
        mergeable,
        ci,
        queued: _,
        dequeued: _,
    } = value;
    w::Pull::new(
        &w::CEILINGS,
        w::PullParts {
            reviewers: encode_reviewers(reviewers),
            number: *number,
            state: topics::encode_state(*state),
            head: head.clone(),
            base: base.clone(),
            commit: *commit,
            base_commit: *base_commit,
            merged: *merged,
            mergeable: *mergeable,
            ci: topics::encode_ci(*ci),
        },
    )
    .expect(FITS)
}
pub(super) fn decode_pull(value: w::Pull) -> d::Pull {
    let w::PullParts { reviewers, number, state, head, base, commit, base_commit, merged, mergeable, ci } =
        value.into_parts();
    d::Pull {
        reviewers: reviewers.into_boxed(),
        number,
        state: topics::decode_state(state),
        head,
        base,
        commit,
        base_commit,
        merged,
        mergeable,
        ci: topics::decode_ci(ci),
        queued: None,
        dequeued: None,
    }
}
fn encode_step(value: &d::Step) -> w::Step {
    w::Step::new(&w::CEILINGS, w::StepParts { number: value.number, name: value.name.clone() }).expect(FITS)
}
fn decode_step(value: w::Step) -> d::Step {
    let w::StepParts { number, name } = value.into_parts();
    d::Step { number, name }
}
pub(super) fn encode_job_attempt(value: &d::JobAttempt) -> w::JobAttempt {
    let d::JobAttempt { head, run, job, attempt } = value;
    w::JobAttempt::new(&w::CEILINGS, w::JobAttemptParts { head: *head, run: *run, job: *job, attempt: *attempt })
        .expect(FITS)
}
pub(super) fn decode_job_attempt(value: w::JobAttempt) -> d::JobAttempt {
    let w::JobAttemptParts { head, run, job, attempt } = value.into_parts();
    d::JobAttempt { head, run, job, attempt }
}
#[expect(clippy::manual_map, reason = "step translations use explicit matches without closures")]
pub(super) fn encode(value: &d::Answer) -> w::Answer {
    match value {
        d::Answer::Located { repository } => {
            w::Answer::Located(w::Located::new(&w::CEILINGS, w::LocatedParts { repository: *repository }).expect(FITS))
        }
        d::Answer::Statuses { ci, statuses, more } => w::Answer::Statuses(encode_statuses_answer(*ci, statuses, *more)),
        d::Answer::Remarks { remarks, more } => w::Answer::Remarks(encode_remarks_answer(remarks, *more)),
        d::Answer::PullFiles { head, files, more } => {
            w::Answer::PullFiles(encode_pull_files_answer(*head, files, *more))
        }
        d::Answer::Compare { before, after, contains_before, files, commits } => {
            w::Answer::Compare(encode_compare_answer(*before, *after, *contains_before, files, commits))
        }
        d::Answer::Checks(statuses) => w::Answer::Checks(encode_checks_answer(statuses)),
        d::Answer::Collaborators { collaborators, more } => {
            w::Answer::Collaborators(encode_collaborators_answer(collaborators, *more))
        }

        d::Answer::Runs { .. } | d::Answer::Run(_) | d::Answer::Tree { .. } | d::Answer::Commits { .. } => {
            unreachable!("queue does not persist Actions, tree or commit-range answers")
        }
        d::Answer::Items { items, more, now } => w::Answer::Items(encode_items_answer(items, *more, *now)),
        d::Answer::Item { item, comments, more } => w::Answer::Item(encode_item_answer(item, comments, *more)),
        d::Answer::Reviews { reviews, more } => w::Answer::Reviews(encode_reviews_answer(reviews, *more)),
        d::Answer::Pull(value) => w::Answer::Pull(encode_pull(value)),
        d::Answer::Commit(commit) => {
            w::Answer::Commit(w::Commit::new(&w::CEILINGS, w::CommitParts { commit: *commit }).expect(FITS))
        }
        d::Answer::Branches(values) => w::Answer::Branches(
            w::BranchesAnswer::new(&w::CEILINGS, w::BranchesAnswerParts { branches: names::encode_piece(values) })
                .expect(FITS),
        ),
        d::Answer::Job { attempt, step, log, truncated } => w::Answer::Job(
            w::JobAnswer::new(
                &w::CEILINGS,
                w::JobAnswerParts {
                    attempt: encode_job_attempt(attempt),
                    step: match step {
                        Some(step) => Some(encode_step(step)),
                        None => None,
                    },
                    log: log.clone(),
                    truncated: *truncated,
                },
            )
            .expect(FITS),
        ),
        d::Answer::File { head, path, bytes, truncated } => w::Answer::File(
            w::FileAnswer::new(
                &w::CEILINGS,
                w::FileAnswerParts { head: *head, path: path.clone(), bytes: bytes.clone(), truncated: *truncated },
            )
            .expect(FITS),
        ),
        d::Answer::Protection(value) => w::Answer::Protection(
            w::ProtectionAnswer::new(
                &w::CEILINGS,
                w::ProtectionAnswerParts {
                    protection: match value {
                        Some(value) => Some(encode_protection(value)),
                        None => None,
                    },
                },
            )
            .expect(FITS),
        ),
        d::Answer::Settings(value) => w::Answer::Settings(names::encode_settings(value)),
        d::Answer::Permission(value) => w::Answer::Permission(
            w::PermissionValue::new(&w::CEILINGS, w::PermissionValueParts { permission: encode_permission(*value) })
                .expect(FITS),
        ),
        d::Answer::Created(number) => {
            w::Answer::Created(w::Number::new(&w::CEILINGS, w::NumberParts { number: *number }).expect(FITS))
        }
        d::Answer::Commented(number) => {
            w::Answer::Commented(w::Number::new(&w::CEILINGS, w::NumberParts { number: *number }).expect(FITS))
        }
        d::Answer::Reviewed(number) => {
            w::Answer::Reviewed(w::Number::new(&w::CEILINGS, w::NumberParts { number: *number }).expect(FITS))
        }
        d::Answer::Merged(commit) => {
            w::Answer::Merged(w::Commit::new(&w::CEILINGS, w::CommitParts { commit: *commit }).expect(FITS))
        }
        d::Answer::Branch(value) => w::Answer::Branch(
            w::BranchCreationValue::new(
                &w::CEILINGS,
                w::BranchCreationValueParts { creation: encode_branch_creation(*value) },
            )
            .expect(FITS),
        ),
        d::Answer::Queued => w::Answer::Queued,
        d::Answer::Done => w::Answer::Done,
    }
}
#[expect(clippy::manual_map, reason = "Step translations use explicit matches without closures")]
pub(super) fn decode(value: w::Answer, bounds: &Bounds) -> Result<d::Answer, Undecodable> {
    Ok(match value {
        w::Answer::Queued => d::Answer::Queued,
        w::Answer::Located(value) => d::Answer::Located { repository: value.into_parts().repository },
        w::Answer::Statuses(value) => {
            let w::StatusesAnswerParts { ci, statuses, more } = value.into_parts();
            d::Answer::Statuses { ci: topics::decode_ci(ci), statuses: decode_statuses(&statuses, bounds)?, more }
        }
        w::Answer::Remarks(value) => {
            let w::RemarksAnswerParts { remarks, more } = value.into_parts();
            d::Answer::Remarks { remarks: decode_remarks(&remarks, bounds)?, more }
        }
        w::Answer::PullFiles(value) => {
            let w::PullFilesAnswerParts { head, files, more } = value.into_parts();
            d::Answer::PullFiles { head, files: decode_files(&files, bounds)?, more }
        }
        w::Answer::Compare(value) => {
            let w::CompareAnswerParts { before, after, contains_before, files, commits } = value.into_parts();
            d::Answer::Compare {
                before,
                after,
                contains_before,
                files: decode_files(&files, bounds)?,
                commits: commits.into_boxed(),
            }
        }
        w::Answer::Checks(value) => {
            let w::ChecksAnswerParts { statuses } = value.into_parts();
            d::Answer::Checks(decode_statuses(&statuses, bounds)?)
        }
        w::Answer::Collaborators(value) => {
            let w::CollaboratorsAnswerParts { collaborators, more } = value.into_parts();
            d::Answer::Collaborators { collaborators: decode_collaborators(collaborators), more }
        }

        w::Answer::Items(value) => {
            let w::ItemsAnswerParts { items, more, now } = value.into_parts();
            d::Answer::Items { items: decode_summaries(&items, bounds)?, more, now: skein_lib::Time::from_nanos(now) }
        }
        w::Answer::Item(value) => {
            let w::ItemAnswerParts { item, comments, more } = value.into_parts();
            d::Answer::Item {
                item: api::decode_summary(item, bounds)?,
                comments: decode_comments(&comments, bounds)?,
                more,
            }
        }
        w::Answer::Reviews(value) => {
            let w::ReviewsAnswerParts { reviews, more } = value.into_parts();
            d::Answer::Reviews { reviews: decode_reviews(&reviews, bounds)?, more }
        }
        w::Answer::Pull(value) => d::Answer::Pull(decode_pull(value)),
        w::Answer::Commit(value) => d::Answer::Commit(value.into_parts().commit),
        w::Answer::Branches(value) => {
            d::Answer::Branches(names::decode_contexts(&value.into_parts().branches, bounds)?)
        }
        w::Answer::Job(value) => {
            let w::JobAnswerParts { attempt, step, log, truncated } = value.into_parts();
            d::Answer::Job {
                attempt: decode_job_attempt(attempt),
                step: match step {
                    Some(step) => Some(decode_step(step)),
                    None => None,
                },
                log,
                truncated,
            }
        }
        w::Answer::File(value) => {
            let w::FileAnswerParts { head, path, bytes, truncated } = value.into_parts();
            d::Answer::File { head, path, bytes, truncated }
        }
        w::Answer::Protection(value) => d::Answer::Protection(match value.into_parts().protection {
            Some(value) => Some(decode_protection(value, bounds)?),
            None => None,
        }),
        w::Answer::Settings(value) => d::Answer::Settings(names::decode_settings(value)),
        w::Answer::Permission(value) => d::Answer::Permission(decode_permission(value.into_parts().permission)),
        w::Answer::Created(value) => d::Answer::Created(value.number()),
        w::Answer::Commented(value) => d::Answer::Commented(value.number()),
        w::Answer::Reviewed(value) => d::Answer::Reviewed(value.number()),
        w::Answer::Merged(value) => d::Answer::Merged(value.into_parts().commit),
        w::Answer::Branch(value) => d::Answer::Branch(decode_branch_creation(value.into_parts().creation)),
        w::Answer::Done => d::Answer::Done,
    })
}
pub(super) fn encode_protection(value: &d::Protection) -> w::Protection {
    let d::Protection { branch, contexts, approvals, dismiss_stale, queue, signatures, linear, code_owners, bypass } =
        value;
    w::Protection::new(
        &w::CEILINGS,
        w::ProtectionParts {
            branch: branch.clone(),
            contexts: names::encode_piece(contexts),
            approvals: *approvals,
            skein_bools: [*dismiss_stale, *queue, *signatures, *linear, *code_owners],
            bypass: match bypass {
                d::Bypass::Never => w::Bypass::Never,
                d::Bypass::PullsOnly => w::Bypass::PullsOnly,
                d::Bypass::Always => w::Bypass::Always,
            },
        },
    )
    .expect(FITS)
}
pub(super) fn decode_protection(value: w::Protection, bounds: &Bounds) -> Result<d::Protection, Undecodable> {
    let w::ProtectionParts {
        branch,
        contexts,
        approvals,
        skein_bools: [dismiss_stale, queue, signatures, linear, code_owners],
        bypass,
    } = value.into_parts();
    Ok(d::Protection {
        queue,
        signatures,
        linear,
        code_owners,
        bypass: match bypass {
            w::Bypass::Never => d::Bypass::Never,
            w::Bypass::PullsOnly => d::Bypass::PullsOnly,
            w::Bypass::Always => d::Bypass::Always,
        },
        branch,
        contexts: names::decode_contexts(&contexts, bounds)?,
        approvals,
        dismiss_stale,
    })
}

fn encode_summaries(values: &[d::Summary]) -> Box<[u8]> {
    let mut size = 0_usize;
    for value in values {
        size = size
            .checked_add(usize::try_from(api::encode_summary(value).measure()).expect("item length fits"))
            .expect("bounded piece")
            .checked_add(4)
            .expect("bounded piece");
    }
    let mut writer = Writer::new(size);
    for value in values {
        let item = api::encode_summary(value);
        let mut encoded = Writer::new(usize::try_from(item.measure()).expect("item length fits"));
        item.encode(&mut encoded).expect("measured item room");
        skein_codec::piece::write(&mut writer, &encoded.finish()).expect("measured piece room");
    }
    writer.finish()
}
fn decode_summaries(bytes: &[u8], bounds: &Bounds) -> Result<Box<[d::Summary]>, Undecodable> {
    let mut reader = Reader::new(bytes);
    let mut values = List::with_capacity(bounds.forge().client.rows);
    for _ in 0..=bounds.forge().client.rows {
        let Some(item) = skein_codec::piece::split(&mut reader).or(Err(Undecodable))? else {
            return Ok(values.into_boxed());
        };
        let item = w::Summary::decode(bounds.limits(), &mut Reader::new(&item)).or(Err(Undecodable))?;
        let value = api::decode_summary(item, bounds)?;
        values.push(value).or(Err(Undecodable))?;
    }
    Err(Undecodable)
}
fn encode_comments(values: &[d::Comment]) -> Box<[u8]> {
    let mut size = 0_usize;
    for value in values {
        size = size
            .checked_add(usize::try_from(api::encode_comment(value).measure()).expect("item length fits"))
            .expect("bounded piece")
            .checked_add(4)
            .expect("bounded piece");
    }
    let mut writer = Writer::new(size);
    for value in values {
        let item = api::encode_comment(value);
        let mut encoded = Writer::new(usize::try_from(item.measure()).expect("item length fits"));
        item.encode(&mut encoded).expect("measured item room");
        skein_codec::piece::write(&mut writer, &encoded.finish()).expect("measured piece room");
    }
    writer.finish()
}
fn decode_comments(bytes: &[u8], bounds: &Bounds) -> Result<Box<[d::Comment]>, Undecodable> {
    let mut reader = Reader::new(bytes);
    let mut values = List::with_capacity(bounds.forge().client.rows);
    for _ in 0..=bounds.forge().client.rows {
        let Some(item) = skein_codec::piece::split(&mut reader).or(Err(Undecodable))? else {
            return Ok(values.into_boxed());
        };
        let item = w::Comment::decode(bounds.limits(), &mut Reader::new(&item)).or(Err(Undecodable))?;
        let value = api::decode_comment(item);
        values.push(value).or(Err(Undecodable))?;
    }
    Err(Undecodable)
}
fn encode_reviews(values: &[d::Review]) -> Box<[u8]> {
    let mut size = 0_usize;
    for value in values {
        size = size
            .checked_add(usize::try_from(api::encode_review(value).measure()).expect("item length fits"))
            .expect("bounded piece")
            .checked_add(4)
            .expect("bounded piece");
    }
    let mut writer = Writer::new(size);
    for value in values {
        let item = api::encode_review(value);
        let mut encoded = Writer::new(usize::try_from(item.measure()).expect("item length fits"));
        item.encode(&mut encoded).expect("measured item room");
        skein_codec::piece::write(&mut writer, &encoded.finish()).expect("measured piece room");
    }
    writer.finish()
}
fn decode_reviews(bytes: &[u8], bounds: &Bounds) -> Result<Box<[d::Review]>, Undecodable> {
    let mut reader = Reader::new(bytes);
    let mut values = List::with_capacity(bounds.forge().client.rows);
    for _ in 0..=bounds.forge().client.rows {
        let Some(item) = skein_codec::piece::split(&mut reader).or(Err(Undecodable))? else {
            return Ok(values.into_boxed());
        };
        let item = w::Review::decode(bounds.limits(), &mut Reader::new(&item)).or(Err(Undecodable))?;
        let value = api::decode_review(item);
        values.push(value).or(Err(Undecodable))?;
    }
    Err(Undecodable)
}

fn encode_statuses(values: &[d::Status]) -> Box<[u8]> {
    let mut size = 0_usize;
    for value in values {
        size = size
            .checked_add(usize::try_from(api::encode_status(value).measure()).expect("item length fits"))
            .expect("bounded piece")
            .checked_add(4)
            .expect("bounded piece");
    }
    let mut writer = Writer::new(size);
    for value in values {
        let item = api::encode_status(value);
        let mut encoded = Writer::new(usize::try_from(item.measure()).expect("item length fits"));
        item.encode(&mut encoded).expect("measured item room");
        skein_codec::piece::write(&mut writer, &encoded.finish()).expect("measured piece room");
    }
    writer.finish()
}
fn decode_statuses(bytes: &[u8], bounds: &Bounds) -> Result<Box<[d::Status]>, Undecodable> {
    let mut reader = Reader::new(bytes);
    let mut values = List::with_capacity(bounds.forge().client.rows);
    for _ in 0..=bounds.forge().client.rows {
        let Some(item) = skein_codec::piece::split(&mut reader).or(Err(Undecodable))? else {
            return Ok(values.into_boxed());
        };
        let item = w::Status::decode(bounds.limits(), &mut Reader::new(&item)).or(Err(Undecodable))?;
        let value = api::decode_status(item);
        values.push(value).or(Err(Undecodable))?;
    }
    Err(Undecodable)
}
fn encode_remarks(values: &[d::Remark]) -> Box<[u8]> {
    let mut size = 0_usize;
    for value in values {
        size = size
            .checked_add(usize::try_from(api::encode_remark(value).measure()).expect("item length fits"))
            .expect("bounded piece")
            .checked_add(4)
            .expect("bounded piece");
    }
    let mut writer = Writer::new(size);
    for value in values {
        let item = api::encode_remark(value);
        let mut encoded = Writer::new(usize::try_from(item.measure()).expect("item length fits"));
        item.encode(&mut encoded).expect("measured item room");
        skein_codec::piece::write(&mut writer, &encoded.finish()).expect("measured piece room");
    }
    writer.finish()
}
fn decode_remarks(bytes: &[u8], bounds: &Bounds) -> Result<Box<[d::Remark]>, Undecodable> {
    let mut reader = Reader::new(bytes);
    let mut values = List::with_capacity(bounds.forge().client.rows);
    for _ in 0..=bounds.forge().client.rows {
        let Some(item) = skein_codec::piece::split(&mut reader).or(Err(Undecodable))? else {
            return Ok(values.into_boxed());
        };
        let item = w::Remark::decode(bounds.limits(), &mut Reader::new(&item)).or(Err(Undecodable))?;
        let value = api::decode_remark(item);
        values.push(value).or(Err(Undecodable))?;
    }
    Err(Undecodable)
}
fn encode_files(values: &[d::File]) -> Box<[u8]> {
    let mut size = 0_usize;
    for value in values {
        size = size
            .checked_add(usize::try_from(api::encode_file(value).measure()).expect("item length fits"))
            .expect("bounded piece")
            .checked_add(4)
            .expect("bounded piece");
    }
    let mut writer = Writer::new(size);
    for value in values {
        let item = api::encode_file(value);
        let mut encoded = Writer::new(usize::try_from(item.measure()).expect("item length fits"));
        item.encode(&mut encoded).expect("measured item room");
        skein_codec::piece::write(&mut writer, &encoded.finish()).expect("measured piece room");
    }
    writer.finish()
}
fn decode_files(bytes: &[u8], bounds: &Bounds) -> Result<Box<[d::File]>, Undecodable> {
    let mut reader = Reader::new(bytes);
    let mut values = List::with_capacity(bounds.forge().client.rows);
    for _ in 0..=bounds.forge().client.rows {
        let Some(item) = skein_codec::piece::split(&mut reader).or(Err(Undecodable))? else {
            return Ok(values.into_boxed());
        };
        let item = w::File::decode(bounds.limits(), &mut Reader::new(&item)).or(Err(Undecodable))?;
        let value = api::decode_file(item);
        values.push(value).or(Err(Undecodable))?;
    }
    Err(Undecodable)
}
fn encode_commits(values: &[d::Commit]) -> List<d::Commit> {
    let mut result = List::with_capacity(u32::try_from(values.len()).expect("admitted count"));
    for value in values {
        result.push(*value).expect("same count");
    }
    result
}
fn encode_collaborators(values: &[d::Collaborator]) -> List<w::Collaborator> {
    let mut result = List::with_capacity(u32::try_from(values.len()).expect("admitted count"));
    for value in values {
        let item = api::encode_collaborator(value);
        result.push(item).expect("same count");
    }
    result
}
fn decode_collaborators(values: List<w::Collaborator>) -> Box<[d::Collaborator]> {
    let mut result = List::with_capacity(values.len());
    for value in values.into_boxed() {
        let item = api::decode_collaborator(value);
        result.push(item).expect("same count");
    }
    result.into_boxed()
}

fn encode_items_answer(items: &[d::Summary], more: bool, now: skein_lib::Time) -> w::ItemsAnswer {
    w::ItemsAnswer::new(&w::CEILINGS, w::ItemsAnswerParts { items: encode_summaries(items), more, now: now.as_nanos() })
        .expect(FITS)
}
fn encode_item_answer(item: &d::Summary, comments: &[d::Comment], more: bool) -> w::ItemAnswer {
    w::ItemAnswer::new(
        &w::CEILINGS,
        w::ItemAnswerParts { item: api::encode_summary(item), comments: encode_comments(comments), more },
    )
    .expect(FITS)
}
fn encode_reviews_answer(reviews: &[d::Review], more: bool) -> w::ReviewsAnswer {
    w::ReviewsAnswer::new(&w::CEILINGS, w::ReviewsAnswerParts { reviews: encode_reviews(reviews), more }).expect(FITS)
}
fn encode_statuses_answer(ci: d::Ci, statuses: &[d::Status], more: bool) -> w::StatusesAnswer {
    w::StatusesAnswer::new(
        &w::CEILINGS,
        w::StatusesAnswerParts { ci: topics::encode_ci(ci), statuses: encode_statuses(statuses), more },
    )
    .expect(FITS)
}
fn encode_remarks_answer(remarks: &[d::Remark], more: bool) -> w::RemarksAnswer {
    w::RemarksAnswer::new(&w::CEILINGS, w::RemarksAnswerParts { remarks: encode_remarks(remarks), more }).expect(FITS)
}
pub(super) fn encode_pull_files_answer(head: d::Commit, files: &[d::File], more: bool) -> w::PullFilesAnswer {
    w::PullFilesAnswer::new(&w::CEILINGS, w::PullFilesAnswerParts { head, files: encode_files(files), more })
        .expect(FITS)
}
fn encode_compare_answer(
    before: d::Commit,
    after: d::Commit,
    contains_before: bool,
    files: &[d::File],
    commits: &[d::Commit],
) -> w::CompareAnswer {
    w::CompareAnswer::new(
        &w::CEILINGS,
        w::CompareAnswerParts {
            before,
            after,
            contains_before,
            files: encode_files(files),
            commits: encode_commits(commits),
        },
    )
    .expect(FITS)
}
fn encode_checks_answer(statuses: &[d::Status]) -> w::ChecksAnswer {
    w::ChecksAnswer::new(&w::CEILINGS, w::ChecksAnswerParts { statuses: encode_statuses(statuses) }).expect(FITS)
}
fn encode_collaborators_answer(collaborators: &[d::Collaborator], more: bool) -> w::CollaboratorsAnswer {
    w::CollaboratorsAnswer::new(
        &w::CEILINGS,
        w::CollaboratorsAnswerParts { collaborators: encode_collaborators(collaborators), more },
    )
    .expect(FITS)
}
