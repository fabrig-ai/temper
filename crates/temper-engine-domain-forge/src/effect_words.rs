//! An outbox hold keeps its operation, target and terminal outcome (jig's client/domain/objects.md, 5).
use alloc::boxed::Box;
use skein_forge::api::{Error, Write};
use skein_forge_client::{Entry, Outcome};
use skein_lib::List;
const ACCOUNT_BYTES: u32 = crate::EFFECT_ACCOUNT_BYTES;

fn put(words: &mut List<u8>, bytes: &[u8]) {
    for byte in bytes {
        if words.room() == 0 {
            break;
        }
        words.push(*byte).expect("account room");
    }
}

pub(crate) fn account(entry: &Entry, repository: Option<&crate::Repository>, outcome: Outcome) -> Option<Box<[u8]>> {
    let result = match outcome {
        Outcome::Held => b"uncertain: it may or may not have been made".as_slice(),
        Outcome::Failed(error) => error_words(error),
        Outcome::Withdrawn => b"withdrawn before completion",
        Outcome::Raced { why, .. } => error_words(why),
        Outcome::Made { .. } | Outcome::Queued | Outcome::Uncertain => return None,
    };
    let mut words = List::with_capacity(ACCOUNT_BYTES);
    put(&mut words, operation(&entry.effect.write));
    put(&mut words, b": ");
    put(&mut words, result);
    put(&mut words, b"; resource ");
    if let Some(repository) = repository {
        put(&mut words, &repository.owner);
        put(&mut words, b"/");
        put(&mut words, &repository.name);
    } else {
        put(&mut words, b"repository ");
        put(&mut words, skein_lib::Decimal::of(u64::from(entry.repository.repository)).as_bytes());
    }
    target(&mut words, &entry.effect.write);
    Some(words.into_boxed())
}

fn operation(write: &Write) -> &'static [u8] {
    match write {
        Write::CreateIssue { .. } => b"create issue",
        Write::OpenPull { .. } => b"open pull request",
        Write::Post { .. } => b"post comment",
        Write::Review { .. } => b"post review",
        Write::Edit { .. } => b"edit item",
        Write::SetReviewers { .. } => b"set reviewers",
        Write::Close { .. } => b"close item",
        Write::Reopen { .. } => b"reopen item",
        Write::Merge { .. } => b"merge pull request",
        Write::Update { .. } => b"update pull request",
        Write::Status { .. } => b"set status",
        Write::Dispatch { .. } => b"dispatch workflow",
        Write::Rerun { .. } => b"rerun job",
        Write::CreateBranch { .. } => b"create branch",
        Write::DeleteBranch { .. } => b"delete branch",
    }
}

fn target(words: &mut List<u8>, write: &Write) {
    match write {
        Write::Post { number, .. }
        | Write::Review { number, .. }
        | Write::Edit { number, .. }
        | Write::SetReviewers { number, .. }
        | Write::Close { number }
        | Write::Reopen { number }
        | Write::Merge { number, .. }
        | Write::Update { number, .. } => {
            put(words, b" #");
            put(words, skein_lib::Decimal::of(*number).as_bytes());
        }
        Write::OpenPull { head, base, .. } => {
            put(words, b" ");
            put(words, head);
            put(words, b" -> ");
            put(words, base);
        }
        Write::CreateBranch { branch, .. } | Write::DeleteBranch { branch } => {
            put(words, b" branch ");
            put(words, branch);
        }
        Write::Status { context, .. } => {
            put(words, b" status ");
            put(words, context);
        }
        Write::Dispatch { workflow, branch, .. } => {
            put(words, b" workflow ");
            put(words, workflow);
            put(words, b" on branch ");
            put(words, branch);
        }
        Write::Rerun { attempt } => {
            put(words, b" run ");
            put(words, skein_lib::Decimal::of(attempt.run).as_bytes());
            put(words, b" job ");
            put(words, skein_lib::Decimal::of(attempt.job).as_bytes());
            put(words, b" attempt ");
            put(words, skein_lib::Decimal::of(u64::from(attempt.attempt)).as_bytes());
        }
        Write::CreateIssue { .. } => {}
    }
}

fn error_words(error: Error) -> &'static [u8] {
    match error {
        Error::Forbidden => b"forge refused permission",
        Error::Protected => b"forge protection refused the write",
        Error::Missing => b"target missing",
        Error::MissingJob => b"job attempt missing",
        Error::TooLarge => b"configured size bound exceeded",
        Error::Empty => b"empty operation",
        Error::Full => b"forge has no room",
        Error::Exists => b"target already exists",
        Error::NothingToMerge => b"nothing to merge",
        Error::Closed => b"target closed",
        Error::Stale => b"head changed",
        Error::Conflict => b"conflict with base",
        Error::Refused => b"forge policy refused the operation",
        Error::InvalidAnswer => b"invalid forge answer",
        Error::Busy => b"connector admission busy",
        Error::NotConnected => b"forge not connected",
        Error::NotInstalled => b"installation not accepted",
        Error::Unavailable => b"forge unavailable",
        Error::Timeout => b"forge timed out",
        Error::RateLimited { .. } => b"forge rate limited",
    }
}
