//! Stateless application parts between the root and its channel (protocol/channel.md, 7).
use crate::settings;
use alloc::boxed::Box;
use skein_lib::{List, Secret, Writer};
use temper_engine_domain as engine;
use temper_protocol_worker_wire::{self as wire, v1};

/// Application limits from the engine's forge and checkout bounds.
#[must_use]
pub fn limits(own: &crate::limits::Limits) -> Option<v1::Limits> {
    wire::limits(&wire::Bounds {
        repositories: own.checkout.repositories.min(own.forge.resources_per_task),
        name_bytes: own.checkout.name_bytes.min(own.forge.name_bytes),
        remote_bytes: own.checkout.name_bytes,
        conflicts: own.checkout.conflicts,
        path_bytes: own.checkout.path_bytes,
        token_bytes: v1::CEILINGS.token_token,
        entries: own.forges.installations,
        entry_token_bytes: own.forge_component.token_bytes,
        text_bytes: own.checkout.name_bytes.min(v1::CEILINGS.identity_name),
    })
}

/// A workspace's remotes and programs, made from each configured forge.
pub fn items(
    assignment: &engine::Assignment,
    forges: &[settings::Forge],
    tokens: &crate::forge::tokens::Tokens,
    limits: &v1::Limits,
) -> Result<Option<v1::Items>, v1::Problem> {
    if assignment.workspace.repositories.is_empty() {
        return Ok(None);
    }
    let mut repositories = List::with_capacity(limits.items_repositories);
    for repository in &assignment.workspace.repositories {
        let value = repository_item(repository, forges, tokens, limits)?;
        repositories.push(value).or(Err(problem(v1::Path::ItemsRepositories)))?;
    }
    Ok(Some(v1::Items::new(limits, v1::ItemsParts { repositories, save: None })?))
}
fn repository_item(
    repository: &engine::ForgeRepository,
    forges: &[settings::Forge],
    tokens: &crate::forge::tokens::Tokens,
    limits: &v1::Limits,
) -> Result<v1::Repository, v1::Problem> {
    let mut configured = None;
    for forge in forges {
        if forge.number == repository.provider.forge {
            configured = Some(forge);
            break;
        }
    }
    let forge = configured.ok_or(problem(v1::Path::RepositoryRemote))?;
    let base = forge.git.as_ref();
    let length = base.len().checked_add(repository.owner.len()).ok_or(problem(v1::Path::RepositoryRemote))?;
    let length = length.checked_add(repository.name.len()).ok_or(problem(v1::Path::RepositoryRemote))?;
    let length = length.checked_add(6).ok_or(problem(v1::Path::RepositoryRemote))?;
    if length > usize::try_from(limits.repository_remote).expect("u32 fits usize") {
        return Err(problem(v1::Path::RepositoryRemote));
    }
    let mut remote = Writer::new(length);
    for segment in [base, b"/", &repository.owner, b"/", &repository.name, b".git"] {
        remote.put(segment).expect("measured remote");
    }
    let access = match &repository.push {
        Some(push) => v1::Access::Writable(v1::Writable::new(
            limits,
            v1::WritableParts { push: push.clone(), expected: None, landing: repository.landing.clone() },
        )?),
        None => v1::Access::ReadOnly,
    };
    v1::Repository::new(
        limits,
        v1::RepositoryParts {
            tag: repository.tag,
            name: repository.name.clone(),
            remote: remote.finish(),
            git: forge.git_program.clone(),
            start: start(&repository.start, limits)?,
            access,
            identity: repository.identity,
            account: match forge.kind {
                settings::Kind::GitHub => crate::forge::tokens::account(tokens, forge.number, &repository.owner),
                settings::Kind::Forgejo => None,
            },
        },
    )
}
fn start(start: &engine::ForgeStart, limits: &v1::Limits) -> Result<v1::Start, v1::Problem> {
    Ok(match start {
        engine::ForgeStart::Base(branch) => {
            v1::Start::Base(v1::Branch::new(limits, v1::BranchParts { branch: branch.clone() })?)
        }
        engine::ForgeStart::Branch(branch) => {
            v1::Start::Branch(v1::Branch::new(limits, v1::BranchParts { branch: branch.clone() })?)
        }
        engine::ForgeStart::Saved(branch) => {
            v1::Start::Saved(v1::Branch::new(limits, v1::BranchParts { branch: branch.clone() })?)
        }
        engine::ForgeStart::Merge { branch, base } => {
            v1::Start::Merge(v1::Merge::new(limits, v1::MergeParts { branch: branch.clone(), base: *base })?)
        }
    })
}
fn problem(path: v1::Path) -> v1::Problem {
    v1::Problem { path, reason: skein_codec::Reason::Bound }
}

/// Each decoded optional part reaches its matching root field.
#[must_use]
pub fn parts(left: Option<v1::Left>, saved: Option<v1::Saved>, preparation: Option<v1::Preparation>) -> engine::Parts {
    let mut pushed = List::with_capacity(match &left {
        Some(left) => left.landed().len(),
        None => 0,
    });
    if let Some(left) = left {
        for head in left.into_parts().landed.into_boxed() {
            let head = head.into_parts();
            pushed
                .push(temper_engine_domain_forge::Pushed { tag: head.tag, commit: head.commit })
                .expect("bounded heads");
        }
    }
    let saved = match saved {
        Some(saved) => {
            let saved = saved.into_parts();
            let mut tags = List::with_capacity(saved.repositories.len());
            for repository in saved.repositories.into_boxed() {
                let repository = repository.into_parts();
                let kept = match repository.landing {
                    v1::Landing::Landed(_) => true,
                    v1::Landing::Conflicted(_)
                    | v1::Landing::Explained(_)
                    | v1::Landing::Moved
                    | v1::Landing::Failed
                    | v1::Landing::Refused
                    | v1::Landing::Unchanged => repository.from_saved,
                };
                if kept {
                    tags.push(repository.tag).expect("bounded tags");
                }
            }
            Some(tags.into_boxed())
        }
        None => None,
    };
    let preparation = match preparation {
        Some(v1::Preparation::Missing(missed)) => {
            let missed = missed.into_parts();
            let missing = match missed.missing {
                v1::Missing::Repository => engine::Missing::Repository,
                v1::Missing::Branch => engine::Missing::Branch,
                v1::Missing::Commit => engine::Missing::Commit,
            };
            Some(engine::Preparation::Missing { tag: missed.tag, missing })
        }
        Some(v1::Preparation::Refused(tag)) => Some(engine::Preparation::Refused { tag: tag.tag() }),
        Some(v1::Preparation::Transient) | None => None,
    };
    engine::Parts { pushed: pushed.into_boxed(), saved, preparation }
}

/// Encode one configured identity as the single secret the table holds.
pub fn identity(identity: &settings::Identity, limits: &v1::Limits) -> Result<Secret, v1::Problem> {
    let value = v1::Identity::new(
        limits,
        v1::IdentityParts {
            kind: v1::Kind::Forgejo(v1::Token::new(limits, v1::TokenParts { token: identity.token.clone() })?),
            login: identity.login.clone(),
            name: identity.author.name.clone(),
            email: identity.author.email.clone(),
        },
    )?;
    Ok(wire::identity(&value))
}

/// Encode a GitHub value with configuration's login and author or the App bot's defaults.
pub fn github_identity(
    entries: List<v1::Entry>,
    login: &[u8],
    author: &settings::Author,
    slug: &[u8],
    id: u64,
    limits: &v1::Limits,
) -> Result<Secret, v1::Problem> {
    let name = if author.name.is_empty() {
        let mut name = Writer::new(slug.len().checked_add(5).expect("bot name"));
        name.put(slug).expect("slug room");
        name.put(b"[bot]").expect("suffix room");
        name.finish()
    } else {
        author.name.clone()
    };
    let email = if author.email.is_empty() {
        let id = skein_lib::Decimal::of(id);
        let suffix = b"[bot]@users.noreply.github.com";
        let mut email = Writer::new(
            id.as_bytes()
                .len()
                .checked_add(1)
                .expect("separator")
                .checked_add(slug.len())
                .expect("slug")
                .checked_add(suffix.len())
                .expect("email"),
        );
        for part in [id.as_bytes(), b"+", slug, suffix] {
            email.put(part).expect("measured email");
        }
        email.finish()
    } else {
        author.email.clone()
    };
    let value = v1::Identity::new(
        limits,
        v1::IdentityParts {
            kind: v1::Kind::Github(v1::Entries::new(limits, v1::EntriesParts { entries })?),
            login: if login.is_empty() { b"x-access-token".as_slice().into() } else { login.into() },
            name,
            email,
        },
    )?;
    Ok(wire::identity(&value))
}

/// The worker's decoded workspace and saved-work branch (protocol/channel.md, 7).
pub type Assigned = (temper_worker_domain::wire::Workspace, Option<Box<[u8]>>);

/// Copy every workspace field from its channel record, or an empty workspace.
#[must_use]
pub fn workspace(items: Option<&v1::Items>) -> Assigned {
    use temper_worker_domain::wire as worker;
    let Some(items) = items else { return (worker::Workspace { repositories: Box::new([]) }, None) };
    let mut repositories = List::with_capacity(items.repositories().len());
    for repository in items.repositories() {
        let start = match repository.start() {
            v1::Start::Merge(value) => worker::Start::Merge { branch: value.branch().into(), base: *value.base() },
            v1::Start::Base(value) => worker::Start::Base { branch: value.branch().into() },
            v1::Start::Branch(value) => worker::Start::Branch { branch: value.branch().into() },
            v1::Start::Commit(value) => worker::Start::Commit { commit: *value.commit() },
            v1::Start::Saved(value) => worker::Start::Saved { branch: value.branch().into() },
        };
        let access = match repository.access() {
            v1::Access::Writable(value) => worker::Access::Writable {
                push: value.push().into(),
                expected: *value.expected(),
                landing: value.landing().clone(),
            },
            v1::Access::ReadOnly => worker::Access::ReadOnly,
        };
        repositories
            .push(worker::Repository {
                tag: repository.tag(),
                name: repository.name().into(),
                remote: repository.remote().into(),
                git: repository.git().clone(),
                start,
                access,
                identity: repository.identity(),
                account: *repository.account(),
            })
            .expect("one result per decoded repository");
    }
    let save = items.save().clone();
    (worker::Workspace { repositories: repositories.into_boxed() }, save)
}

/// What a run left and saved, field for field under the payload's ceilings.
#[must_use]
pub fn work(work: &temper_worker_domain::wire::Work) -> (v1::Left, Option<v1::Saved>) {
    let limits = &v1::CEILINGS;
    let mut landed = List::with_capacity(u32::try_from(work.landed.len()).expect("bounded repositories"));
    for head in &work.landed {
        landed
            .push(
                v1::Landed::new(limits, v1::LandedParts { tag: head.tag, commit: head.commit }).expect("fixed fields"),
            )
            .expect("one head per repository");
    }
    let left = v1::Left::new(limits, v1::LeftParts { landed }).expect("root repositories fit the payload ceilings");
    let saved = match &work.saved {
        Some(rows) => {
            let mut repositories = List::with_capacity(u32::try_from(rows.len()).expect("bounded repositories"));
            for row in rows {
                let landing = landing(&row.landing);
                repositories
                    .push(
                        v1::SavedRepository::new(
                            limits,
                            v1::SavedRepositoryParts { tag: row.tag, from_saved: row.from_saved, landing },
                        )
                        .expect("fixed fields"),
                    )
                    .expect("one saved row per repository");
            }
            Some(
                v1::Saved::new(limits, v1::SavedParts { repositories })
                    .expect("root repositories fit payload ceilings"),
            )
        }
        None => None,
    };
    (left, saved)
}

fn landing(landing: &temper_worker_domain::wire::Landing) -> v1::Landing {
    use temper_worker_domain::wire as worker;
    let limits = &v1::CEILINGS;
    match landing {
        worker::Landing::Conflicted { files } => {
            let mut paths = List::with_capacity(u32::try_from(files.len()).expect("bounded conflict paths"));
            for path in files {
                paths.push(path.clone()).expect("one path per conflict");
            }
            v1::Landing::Conflicted(
                v1::Files::new(limits, v1::FilesParts { files: paths }).expect("root paths fit ceilings"),
            )
        }
        worker::Landing::Explained { failure } => {
            let reason = push_reason(failure.reason);
            let diagnostic = v1::Diagnostic::new(
                limits,
                v1::DiagnosticParts { output: failure.diagnostic.output().into(), cut: failure.diagnostic.cut() },
            )
            .expect("fixed diagnostic cap fits payload");
            v1::Landing::Explained(
                v1::PushFailure::new(
                    limits,
                    v1::PushFailureParts { repository: failure.repository, reason, diagnostic },
                )
                .expect("fixed fields"),
            )
        }
        worker::Landing::Landed { commit } => {
            v1::Landing::Landed(v1::Commit::new(limits, v1::CommitParts { commit: *commit }).expect("fixed id"))
        }
        worker::Landing::Moved => v1::Landing::Moved,
        worker::Landing::Failed => v1::Landing::Failed,
        worker::Landing::Refused => v1::Landing::Refused,
        worker::Landing::Unchanged => v1::Landing::Unchanged,
    }
}
fn push_reason(reason: temper_worker_domain::wire::PushReason) -> v1::PushReason {
    use temper_worker_domain::wire::PushReason;
    match reason {
        PushReason::MissingRepository => v1::PushReason::MissingRepository,
        PushReason::MissingBranch => v1::PushReason::MissingBranch,
        PushReason::MissingCommit => v1::PushReason::MissingCommit,
        PushReason::Refused => v1::PushReason::Refused,
        PushReason::Unreachable => v1::PushReason::Unreachable,
        PushReason::Broken => v1::PushReason::Broken,
        PushReason::TimedOut => v1::PushReason::TimedOut,
        PushReason::Cancelled => v1::PushReason::Cancelled,
        PushReason::Unavailable => v1::PushReason::Unavailable,
        PushReason::Busy => v1::PushReason::Busy,
        PushReason::TooLarge => v1::PushReason::TooLarge,
        PushReason::Nothing => v1::PushReason::Nothing,
        PushReason::Unknown => v1::PushReason::Unknown,
    }
}

/// A permanent failed preparation by tag; no resource for a transient one.
#[must_use]
pub fn preparation(preparation: &temper_worker_domain::wire::Preparation) -> Option<v1::Preparation> {
    use temper_worker_domain::wire as worker;
    let limits = &v1::CEILINGS;
    match preparation {
        worker::Preparation::Transient => None,
        worker::Preparation::Refused { tag } => {
            Some(v1::Preparation::Refused(v1::Tag::new(limits, v1::TagParts { tag: *tag }).expect("fixed tag")))
        }
        worker::Preparation::Missing { tag, missing } => {
            let missing = match missing {
                worker::Missing::Repository => v1::Missing::Repository,
                worker::Missing::Branch => v1::Missing::Branch,
                worker::Missing::Commit => v1::Missing::Commit,
            };
            Some(v1::Preparation::Missing(
                v1::Missed::new(limits, v1::MissedParts { tag: *tag, missing }).expect("fixed fields"),
            ))
        }
    }
}

/// The worker's value, once its login and author fields pass their rules.
#[must_use]
pub fn value(identity: &v1::Identity) -> Option<crate::identities::Value> {
    use crate::identities::{Entry, Value};
    if !wire::valid(identity) {
        return None;
    }
    Some(match identity.kind() {
        v1::Kind::Forgejo(value) => Value::Forgejo(temper_worker_protocol_git::Identity {
            token: value.token().clone(),
            login: identity.login().into(),
            name: identity.name().into(),
            email: identity.email().into(),
        }),
        v1::Kind::Github(value) => {
            let mut entries = List::with_capacity(value.entries().len());
            for entry in value.entries() {
                entries
                    .push(Entry { account: entry.account(), token: entry.token().clone() })
                    .expect("one entry per decoded installation");
            }
            Value::GitHub {
                login: identity.login().into(),
                name: identity.name().into(),
                email: identity.email().into(),
                entries,
            }
        }
    })
}
