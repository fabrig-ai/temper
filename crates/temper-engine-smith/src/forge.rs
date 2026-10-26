//! Forge read and effect values decoded at the Smith host-tool boundary.
//! The root still checks adoption, grants, resource pins and outbox order.

use alloc::boxed::Box;
use skein_forge::api;
use skein_lib::{List, Time};
use temper_engine_domain_forge as forge;

use crate::{Problem, json::Value};

pub(crate) fn repository(value: &Value) -> Result<api::Repository, Problem> {
    let value = value.object(b"repository")?;
    Ok(api::Repository {
        forge: value.required(b"forge")?.narrow(b"forge")?,
        repository: value.required(b"repository")?.small(b"repository")?,
    })
}

pub(crate) fn resource(value: &Value) -> Result<forge::What, Problem> {
    let value = value.object(b"resource")?;
    match value.required(b"kind")?.text(b"kind")?.as_ref() {
        b"repository" => Ok(forge::What::Repository),
        b"pull" => Ok(forge::What::Pull(value.required(b"number")?.number(b"number")?)),
        b"issue" => Ok(forge::What::Issue(value.required(b"number")?.number(b"number")?)),
        b"branch" => Ok(forge::What::Branch(texts(value.required(b"segments")?, b"segments")?)),
        _ => Err(Problem::Range(b"kind")),
    }
}

pub(crate) fn topic(value: &Value) -> Result<forge::Topic, Problem> {
    let value = value.object(b"topic")?;
    let repository = repository(value.required(b"repository")?)?;
    match value.required(b"kind")?.text(b"kind")?.as_ref() {
        b"landings" => Ok(forge::Topic::Landings { repository, branch: value.required(b"branch")?.text(b"branch")? }),
        b"ci" => Ok(forge::Topic::Ci { repository, head: commit(value.required(b"head")?, b"head")? }),
        b"pull" => Ok(forge::Topic::Pull { repository, number: value.required(b"number")?.number(b"number")? }),
        b"participation" => {
            Ok(forge::Topic::Participation { repository, number: value.required(b"number")?.number(b"number")? })
        }
        _ => Err(Problem::Range(b"kind")),
    }
}

fn read(kind: &[u8], value: &Value) -> Result<api::Read, Problem> {
    match kind {
        b"items" => Ok(api::Read::Items {
            since: Time::from_nanos(optional_number(value, b"since", 0)?),
            page: page(value)?,
            kind: match value.field(b"item_kind")? {
                Some(kind) => match kind.text(b"item_kind")?.as_ref() {
                    b"issue" => Some(api::Kind::Issue),
                    b"pull" => Some(api::Kind::Pull),
                    _ => return Err(Problem::Range(b"item_kind")),
                },
                None => None,
            },
        }),
        b"item" => Ok(api::Read::Item {
            number: positive_number(value, b"number")?,
            after: optional_number(value, b"after", 0)?,
        }),
        b"pull" => Ok(api::Read::Pull { number: positive_number(value, b"number")? }),
        b"pull_for" => Ok(api::Read::PullFor {
            head: value.required(b"head")?.text(b"head")?,
            base: value.required(b"base")?.text(b"base")?,
        }),
        b"reviews" => Ok(api::Read::Reviews { number: positive_number(value, b"number")?, page: page(value)? }),
        b"statuses" => {
            Ok(api::Read::Statuses { commit: commit(value.required(b"commit")?, b"commit")?, page: page(value)? })
        }
        b"remarks" => Ok(api::Read::Remarks {
            number: positive_number(value, b"number")?,
            review: positive_number(value, b"review")?,
            page: page(value)?,
        }),
        b"branch" => Ok(api::Read::Branch { branch: value.required(b"branch")?.text(b"branch")? }),
        b"branches" => Ok(api::Read::Branches { prefix: value.required(b"prefix")?.text(b"prefix")? }),
        b"pull_files" => Ok(api::Read::PullFiles {
            number: positive_number(value, b"number")?,
            head: commit(value.required(b"head")?, b"head")?,
            page: page(value)?,
        }),
        b"compare" => Ok(api::Read::Compare {
            before: commit(value.required(b"before")?, b"before")?,
            after: commit(value.required(b"after")?, b"after")?,
        }),
        b"checks" => Ok(api::Read::Checks { commit: commit(value.required(b"commit")?, b"commit")? }),
        b"job" => Ok(api::Read::Job {
            attempt: api::JobAttempt {
                head: commit(value.required(b"head")?, b"head")?,
                run: positive_number(value, b"run")?,
                job: value.required(b"job")?.number(b"job")?,
                attempt: value.required(b"attempt")?.small(b"attempt")?,
            },
            max_bytes: max_bytes(value)?,
        }),
        b"file" => Ok(api::Read::File {
            head: commit(value.required(b"head")?, b"head")?,
            path: value.required(b"path")?.text(b"path")?,
            max_bytes: max_bytes(value)?,
        }),
        b"protection" => Ok(api::Read::Protection { branch: value.required(b"branch")?.text(b"branch")? }),
        b"settings" => Ok(api::Read::Settings),
        b"collaborators" => Ok(api::Read::Collaborators { page: page(value)? }),
        b"permission" => Ok(api::Read::Permission { user: positive_number(value, b"user")? }),
        _ => Err(Problem::Range(b"kind")),
    }
}

/// Bind one declared connector read name to its decoder discriminator.
pub(crate) fn read_named(name: &[u8], value: &Value) -> Result<api::Read, Problem> {
    let kind: &[u8] = match name {
        b"read_items" => b"items",
        b"read_item" => b"item",
        b"read_pull" => b"pull",
        b"read_pull_for" => b"pull_for",
        b"read_reviews" => b"reviews",
        b"read_statuses" => b"statuses",
        b"read_remarks" => b"remarks",
        b"read_branch" => b"branch",
        b"read_branches" => b"branches",
        b"read_pull_files" => b"pull_files",
        b"read_compare" => b"compare",
        b"read_checks" => b"checks",
        b"read_job" => b"job",
        b"read_file" => b"file",
        b"read_protection" => b"protection",
        b"read_settings" => b"settings",
        b"read_collaborators" => b"collaborators",
        b"read_permission" => b"permission",
        _ => return Err(Problem::UnknownTool),
    };
    if value.field(b"kind")?.is_some() {
        return Err(Problem::Range(b"kind"));
    }
    read(kind, value)
}

pub(crate) fn write(value: &Value) -> Result<api::Write, Problem> {
    let value = value.object(b"write")?;
    let kind = value.required(b"kind")?.text(b"kind")?;
    match kind.as_ref() {
        b"create_issue" => Ok(api::Write::CreateIssue {
            key: value.required(b"key")?.text(b"key")?,
            title: value.required(b"title")?.text(b"title")?,
            body: value.required(b"body")?.text(b"body")?,
        }),
        b"open_pull" => Ok(api::Write::OpenPull {
            title: value.required(b"title")?.text(b"title")?,
            body: value.required(b"body")?.text(b"body")?,
            head: value.required(b"head")?.text(b"head")?,
            base: value.required(b"base")?.text(b"base")?,
        }),
        b"post" => Ok(api::Write::Post {
            number: value.required(b"number")?.number(b"number")?,
            key: value.required(b"key")?.text(b"key")?,
            body: value.required(b"body")?.text(b"body")?,
        }),
        b"review" => {
            let verdict = match value.required(b"verdict")?.text(b"verdict")?.as_ref() {
                b"approve" => api::Verdict::Approve,
                b"request_changes" => api::Verdict::RequestChanges,
                b"comment" => api::Verdict::Comment,
                _ => return Err(Problem::Range(b"verdict")),
            };
            Ok(api::Write::Review {
                number: value.required(b"number")?.number(b"number")?,
                key: value.required(b"key")?.text(b"key")?,
                verdict,
                body: value.required(b"body")?.text(b"body")?,
            })
        }
        b"edit" => Ok(api::Write::Edit {
            number: value.required(b"number")?.number(b"number")?,
            title: optional_text(value, b"title")?,
            body: optional_text(value, b"body")?,
        }),
        b"set_reviewers" => Ok(api::Write::SetReviewers {
            number: value.required(b"number")?.number(b"number")?,
            reviewers: numbers(value.required(b"reviewers")?)?,
        }),
        b"close" => Ok(api::Write::Close { number: value.required(b"number")?.number(b"number")? }),
        b"reopen" => Ok(api::Write::Reopen { number: value.required(b"number")?.number(b"number")? }),
        b"merge" => Ok(api::Write::Merge {
            number: value.required(b"number")?.number(b"number")?,
            head: commit(value.required(b"head")?, b"head")?,
        }),
        b"update" => Ok(api::Write::Update { number: value.required(b"number")?.number(b"number")? }),
        b"status" => {
            let check = match value.required(b"check")?.text(b"check")?.as_ref() {
                b"pending" => api::Check::Pending,
                b"passed" => api::Check::Passed,
                b"failed" => api::Check::Failed,
                _ => return Err(Problem::Range(b"check")),
            };
            Ok(api::Write::Status {
                commit: commit(value.required(b"commit")?, b"commit")?,
                context: value.required(b"context")?.text(b"context")?,
                check,
            })
        }
        b"create_branch" => Ok(api::Write::CreateBranch {
            branch: value.required(b"branch")?.text(b"branch")?,
            commit: commit(value.required(b"commit")?, b"commit")?,
        }),
        b"delete_branch" => Ok(api::Write::DeleteBranch { branch: value.required(b"branch")?.text(b"branch")? }),
        _ => Err(Problem::Range(b"kind")),
    }
}

fn commit(value: &Value, member: &'static [u8]) -> Result<api::Commit, Problem> {
    let text = value.text(member)?;
    if text.len() != 40 && text.len() != 64 {
        return Err(Problem::Range(member));
    }
    let mut commit = [0_u8; 32];
    for (pair, cell) in text.chunks_exact(2).zip(&mut commit) {
        let high = high_hex(*pair.first().expect("hex pair"), member)?;
        let low = high_hex(*pair.get(1).expect("hex pair"), member)?;
        *cell = high.saturating_mul(16).saturating_add(low);
    }
    Ok(commit)
}

fn high_hex(value: u8, member: &'static [u8]) -> Result<u8, Problem> {
    match value {
        b'0'..=b'9' => Ok(value.saturating_sub(b'0')),
        b'a'..=b'f' => Ok(value.saturating_sub(b'a').saturating_add(10)),
        _ => Err(Problem::Range(member)),
    }
}

fn positive_number(value: &Value, member: &'static [u8]) -> Result<u64, Problem> {
    let number = value.required(member)?.number(member)?;
    if number == 0 { Err(Problem::Range(member)) } else { Ok(number) }
}

fn optional_number(value: &Value, member: &'static [u8], default: u64) -> Result<u64, Problem> {
    match value.field(member)? {
        Some(value) => value.number(member),
        None => Ok(default),
    }
}

fn page(value: &Value) -> Result<u32, Problem> {
    let page = optional_number(value, b"page", 1)?;
    if page == 0 {
        return Err(Problem::Range(b"page"));
    }
    u32::try_from(page).or(Err(Problem::Range(b"page")))
}

fn max_bytes(value: &Value) -> Result<u32, Problem> {
    // Zero denotes the root's configured read limit; explicit zero is invalid.
    match value.field(b"max_bytes")? {
        None => Ok(0),
        Some(value) => {
            let max = value.small(b"max_bytes")?;
            if max == 0 { Err(Problem::Range(b"max_bytes")) } else { Ok(max) }
        }
    }
}

fn optional_text(value: &Value, name: &'static [u8]) -> Result<Option<Box<[u8]>>, Problem> {
    match value.field(name)? {
        Some(value) => Ok(Some(value.text(name)?)),
        None => Ok(None),
    }
}

pub(crate) fn texts(value: &Value, member: &'static [u8]) -> Result<Box<[Box<[u8]>]>, Problem> {
    let items = value.array(member)?;
    let Ok(capacity) = u32::try_from(items.len()) else { return Err(Problem::TooLarge) };
    let mut out = List::with_capacity(capacity);
    for item in items {
        let Ok(()) = out.push(item.text(member)?) else { return Err(Problem::TooLarge) };
    }
    Ok(out.into_boxed())
}

fn numbers(value: &Value) -> Result<Box<[u64]>, Problem> {
    let items = value.array(b"reviewers")?;
    let Ok(capacity) = u32::try_from(items.len()) else { return Err(Problem::TooLarge) };
    let mut out = List::with_capacity(capacity);
    for item in items {
        let Ok(()) = out.push(item.number(b"reviewers")?) else { return Err(Problem::TooLarge) };
    }
    Ok(out.into_boxed())
}
