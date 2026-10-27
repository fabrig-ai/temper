//! Forge-owned brief sections, gathered from current provider facts
//! (jig's domain/connectors.md, section 9).
use crate::{BriefCommit, BriefItem, BriefRead, BriefSource, Domain, Request};
use alloc::boxed::Box;
use skein_lib::{List, Queue, Token};
use temper_engine_domain_forge_change as change;

/// The home-access reason's 209 fixed bytes plus three owner/repository pairs.
/// Adoption bounds each name by `Limits::name_bytes`, whose ceiling is
/// `PANEL_NAME_BYTES` (1,024 bytes for each owner and repository name).
pub const HOME_SECTION_BYTES: u32 = 209 + 6 * crate::PANEL_NAME_BYTES;

/// The adoption hold's words (domain/forge.md, section 11), bounded by the
/// caller's read allotment and the longest admitted names. Omitted bytes follow
/// the other forge sections' cuts.
#[must_use]
pub fn home_access_reason(owner: &[u8], repository: &[u8], bytes: u32) -> BriefRead {
    let mut words = BriefWords::with_capacity(bytes.min(HOME_SECTION_BYTES), false);
    let parts: [&[u8]; 12] = [
        b"This chat began before ",
        owner,
        b"/",
        repository,
        b" was adopted, so you can read ",
        owner,
        b"/",
        repository,
        b" but not push to it. Tell the person, in your next reply: \"This chat began before ",
        owner,
        b"/",
        repository,
    ];
    for part in parts {
        append(&mut words, part);
    }
    append(&mut words, b" was adopted, so I can read it but not push. Start a new chat to push.\"");
    brief_part(&words, 1, bytes)
}

#[derive(Debug)]
pub(crate) struct BriefFetch {
    owner: Token,
    item: BriefItem,
    head: BriefCommit,
    repository: skein_forge::api::Repository,
    parts: u32,
    bytes: u32,
    stage: BriefStage,
    mode: PullMode,
    base: Option<skein_forge::api::Commit>,
    comparison: Option<(skein_forge::api::Commit, skein_forge::api::Commit)>,
    changed: Box<[Box<[u8]>]>,
    review_ids: Box<[u64]>,
    next_review: u32,
    checks: Box<[skein_forge::api::Status]>,
    next_check: u32,
    max_job_bytes: u32,
    words: BriefWords,
}

#[derive(Debug)]
pub(crate) struct BriefWords {
    bytes: List<u8>,
    left: u64,
    tail: bool,
}

impl BriefWords {
    pub(crate) fn with_capacity(capacity: u32, tail: bool) -> BriefWords {
        BriefWords { bytes: List::with_capacity(capacity), left: 0, tail }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BriefStage {
    Item,
    Pull,
    Files,
    Compare,
    Reviews,
    Remarks,
    Checks,
    Job,
}

#[derive(Clone, Copy, Debug)]
enum PullMode {
    Review,
    Conflict(skein_forge::api::Commit),
    Semantic(Option<skein_forge::api::Commit>),
    Other,
}

pub(crate) fn append(out: &mut BriefWords, bytes: &[u8]) {
    for byte in bytes {
        if out.bytes.room() > 0 {
            out.bytes.push(*byte).expect("checked message room");
        } else {
            out.left = out.left.saturating_add(1);
            if out.tail && !out.bytes.is_empty() {
                for index in 1..out.bytes.len() {
                    *out.bytes.get_mut(index.saturating_sub(1)).expect("Within words") =
                        *out.bytes.get(index).expect("Within words");
                }
                *out.bytes.get_mut(out.bytes.len().saturating_sub(1)).expect("Nonempty words") = *byte;
            }
        }
    }
}

/// One byte's two lowercase hexadecimal digits, the high nibble first.
#[must_use]
pub fn hex(byte: u8) -> [u8; 2] {
    let high = *b"0123456789abcdef".get(usize::from(byte >> 4_u8)).expect("high nibble is hexadecimal");
    let low = *b"0123456789abcdef".get(usize::from(byte & 15_u8)).expect("low nibble is hexadecimal");
    [high, low]
}

fn append_hex(out: &mut BriefWords, bytes: &[u8]) {
    for byte in bytes {
        append(out, &hex(*byte));
    }
}

pub(crate) fn brief_part(words: &BriefWords, parts: u32, bytes: u32) -> BriefRead {
    if parts == 0 {
        return BriefRead::Failed;
    }
    let kept = words.bytes.as_slice().len().min(usize::try_from(bytes).expect("u32 fits usize"));
    let start = if words.tail { words.bytes.as_slice().len().saturating_sub(kept) } else { 0 };
    BriefRead::Got {
        bytes: Box::from(words.bytes.as_slice().get(start..start.saturating_add(kept)).expect("Kept is within source")),
        left: words.left.saturating_add(
            u64::try_from(words.bytes.as_slice().len().checked_sub(kept).expect("Kept is within source"))
                .expect("bounded section"),
        ),
    }
}

pub(super) fn brief_pull(
    domain: &mut Domain,
    owner: Token,
    item: BriefItem,
    head: BriefCommit,
    parts: u32,
    bytes: u32,
    out: &mut Queue<Request>,
) -> Option<BriefRead> {
    let Some(row) = domain.change_for_pull(item.repository, item.number) else {
        return Some(BriefRead::Failed);
    };
    if row.change.last_head != Some(head.0)
        || parts == 0
        || domain.brief_fetches.len() == domain.brief_fetches.capacity()
    {
        return Some(BriefRead::Failed);
    }
    let repository = row.repository;
    let mode = match &row.change.state {
        change::State::Resolving { base, .. } => PullMode::Conflict(*base),
        change::State::Repairing { why: change::Repair::Semantic, .. } => {
            PullMode::Semantic(row.change.clean.last().copied())
        }
        change::State::Gating { .. } => PullMode::Review,
        change::State::Producing { .. }
        | change::State::Opening { .. }
        | change::State::Recreating { .. }
        | change::State::Reopening { .. }
        | change::State::Checking { .. }
        | change::State::Queued { .. }
        | change::State::First { .. }
        | change::State::Updating { .. }
        | change::State::Repairing { .. }
        | change::State::Landing { .. }
        | change::State::Ended
        | change::State::Landed { .. }
        | change::State::Held { .. } => PullMode::Other,
    };
    let words = BriefWords::with_capacity(bytes, false);
    let call = Token::new(owner.raw() | (1_u64 << 61_u32) | (1_u64 << 59_u32));
    let fetch = BriefFetch {
        owner,
        item,
        head,
        repository,
        parts,
        bytes,
        stage: BriefStage::Item,
        mode,
        base: None,
        comparison: None,
        changed: Box::new([]),
        review_ids: Box::new([]),
        next_review: 0,
        checks: Box::new([]),
        next_check: 0,
        max_job_bytes: 0,
        words,
    };
    domain.brief_fetches.insert(call, fetch).expect("brief read room checked");
    out.push(Request::BriefClient {
        event: skein_forge_client::Event::Read {
            owner: call,
            repository,
            read: skein_forge::api::Read::Item { number: item.number, after: 0 },
        },
    });
    // The brief child waits for the connector's bounded answer.
    None
}

#[expect(clippy::too_many_lines, reason = "the bounded forge brief fetch handles each connector reply in order")]
pub(crate) fn brief_answer(
    domain: &mut Domain,
    owner: Token,
    result: Result<skein_forge::api::Answer, skein_forge::api::Error>,
    out: &mut Queue<Request>,
) -> bool {
    let Some(mut fetch) = domain.brief_fetches.remove(&owner) else { return false };
    let next = match result {
        Ok(skein_forge::api::Answer::Item { item, .. })
            if fetch.stage == BriefStage::Item && item.number == fetch.item.number =>
        {
            match fetch.mode {
                PullMode::Review | PullMode::Other => append(&mut fetch.words, b"Pull request at head "),
                PullMode::Conflict(_) => append(&mut fetch.words, b"Resolve the conflict from change head "),
                PullMode::Semantic(_) => append(&mut fetch.words, b"Repair after the base merged into change head "),
            }
            append_hex(&mut fetch.words, &fetch.head.0);
            append(&mut fetch.words, b"\nTitle: ");
            append(&mut fetch.words, &item.title);
            append(&mut fetch.words, b"\nBody: ");
            append(&mut fetch.words, &item.body);
            append(&mut fetch.words, b"\n");
            fetch.stage = BriefStage::Pull;
            Some(skein_forge::api::Read::Pull { number: fetch.item.number })
        }
        Ok(skein_forge::api::Answer::Pull(pull)) if fetch.stage == BriefStage::Pull && pull.commit == fetch.head.0 => {
            fetch.base = pull.base_commit;
            fetch.stage = BriefStage::Files;
            Some(skein_forge::api::Read::PullFiles { number: fetch.item.number, head: fetch.head.0, page: 1 })
        }
        Ok(skein_forge::api::Answer::PullFiles { head, files, .. })
            if fetch.stage == BriefStage::Files && head == fetch.head.0 =>
        {
            append(&mut fetch.words, b"Files and diff:\n");
            let mut changed = List::with_capacity(u32::try_from(files.len()).expect("bounded file list"));
            for file in files {
                changed.push(file.path.clone()).expect("sized from bounded file list");
                append(&mut fetch.words, b"File: ");
                append(&mut fetch.words, &file.path);
                append(&mut fetch.words, b"\nBefore:\n");
                if let Some(before) = file.before {
                    append(&mut fetch.words, &before);
                }
                append(&mut fetch.words, b"\nAfter:\n");
                if let Some(after) = file.after {
                    append(&mut fetch.words, &after);
                }
                append(&mut fetch.words, b"\n");
            }
            fetch.changed = changed.into_boxed();
            let comparison = match fetch.mode {
                PullMode::Review | PullMode::Other => match fetch.base {
                    Some(base) => Some((base, fetch.head.0)),
                    None => None,
                },
                PullMode::Conflict(base) => Some((fetch.head.0, base)),
                PullMode::Semantic(before) => match before {
                    Some(before) => Some((before, fetch.head.0)),
                    None => None,
                },
            };
            fetch.comparison = comparison;
            match comparison {
                Some((before, after)) => {
                    fetch.stage = BriefStage::Compare;
                    Some(skein_forge::api::Read::Compare { before, after })
                }
                None => None,
            }
        }
        Ok(skein_forge::api::Answer::Compare { before, after, files, commits, .. })
            if fetch.stage == BriefStage::Compare && Some((before, after)) == fetch.comparison =>
        {
            match fetch.mode {
                PullMode::Conflict(_) | PullMode::Semantic(_) => {
                    append(&mut fetch.words, b"What landed in the base:\n");
                    for commit in commits {
                        append_hex(&mut fetch.words, &commit);
                        append(&mut fetch.words, b"\n");
                    }
                    for file in files {
                        append(&mut fetch.words, b"Base file: ");
                        append(&mut fetch.words, &file.path);
                        append(&mut fetch.words, b"\n");
                        if match fetch.mode {
                            PullMode::Conflict(_) => true,
                            PullMode::Review | PullMode::Semantic(_) | PullMode::Other => false,
                        } {
                            for changed in &fetch.changed {
                                if changed == &file.path {
                                    append(&mut fetch.words, b"Conflicting file: ");
                                    append(&mut fetch.words, &file.path);
                                    append(&mut fetch.words, b"\n");
                                }
                            }
                        }
                    }
                }
                PullMode::Review | PullMode::Other => {
                    append(&mut fetch.words, b"Against base:\n");
                    for file in files {
                        append(&mut fetch.words, &file.path);
                        append(&mut fetch.words, b"\n");
                    }
                }
            }
            None
        }
        Ok(skein_forge::api::Answer::Reviews { reviews, .. }) if fetch.stage == BriefStage::Reviews => {
            let mut ids = List::with_capacity(u32::try_from(reviews.len()).expect("bounded review list"));
            for review in reviews {
                if review.commit == fetch.head.0 {
                    append(&mut fetch.words, b"Review: ");
                    append(&mut fetch.words, &review.body);
                    append(&mut fetch.words, b"\n");
                    ids.push(review.id).expect("sized from bounded review list");
                }
            }
            fetch.review_ids = ids.into_boxed();
            next_remarks(&mut fetch)
        }
        Ok(skein_forge::api::Answer::Remarks { remarks, .. }) if fetch.stage == BriefStage::Remarks => {
            for remark in remarks {
                append(&mut fetch.words, b"Remark at ");
                append(&mut fetch.words, &remark.path);
                append(&mut fetch.words, b":");
                append(&mut fetch.words, &decimal(u64::from(remark.line)));
                append(&mut fetch.words, b" ");
                append(&mut fetch.words, &remark.body);
                append(&mut fetch.words, b"\n");
            }
            next_remarks(&mut fetch)
        }
        Ok(skein_forge::api::Answer::Checks(checks)) if fetch.stage == BriefStage::Checks => {
            fetch.checks = checks;
            let next = next_failed_job(&mut fetch);
            if next.is_none() {
                append(&mut fetch.words, b"Repair the failed check.\n");
            }
            next
        }
        answer if fetch.stage == BriefStage::Job => {
            match answer {
                Ok(skein_forge::api::Answer::Job { attempt, step, log, truncated }) if attempt.head == fetch.head.0 => {
                    if let Some(skein_forge::api::Step { number, name }) = step {
                        append(&mut fetch.words, b"Failed step: ");
                        append(&mut fetch.words, &decimal(u64::from(number)));
                        if !name.is_empty() {
                            append(&mut fetch.words, b" ");
                            append(&mut fetch.words, &name);
                        }
                        append(&mut fetch.words, b"\nEnd of its log:\n");
                    } else {
                        append(&mut fetch.words, b"No step failed. End of the job's log:\n");
                    }
                    if truncated {
                        append(&mut fetch.words, b"[log truncated before this]\n");
                    }
                    append(&mut fetch.words, &log);
                    append(&mut fetch.words, b"\n");
                }
                Ok(_) | Err(_) => append(&mut fetch.words, b"[job log could not be read]\n"),
            }
            let next = next_failed_job(&mut fetch);
            if next.is_none() {
                append(&mut fetch.words, b"Repair the failed check.\n");
            }
            next
        }
        Ok(_) | Err(_) => {
            crate::held::ready(domain, fetch.owner, BriefRead::Failed, out);
            return true;
        }
    };
    if let Some(read) = next {
        let repository = fetch.repository;
        domain.brief_fetches.insert(owner, fetch).expect("same brief read room");
        out.push(Request::BriefClient { event: skein_forge_client::Event::Read { owner, repository, read } });
    } else {
        let read = brief_part(&fetch.words, fetch.parts, fetch.bytes);
        crate::held::ready(domain, fetch.owner, read, out);
    }
    true
}

/// Start gathering a connector-owned section while its parent holds only
/// the section token and its budget.
#[expect(clippy::too_many_arguments, reason = "one connector gather names its owner, source and bounded read")]
pub(crate) fn gather(
    domain: &mut Domain,
    owner: Token,
    source: BriefSource,
    parts: u32,
    bytes: u32,
    max_job_bytes: u32,
    limit: u32,
    out: &mut Queue<Request>,
) {
    if bytes > limit {
        crate::held::ready(domain, owner, BriefRead::Failed, out);
        return;
    }
    let read = match source {
        BriefSource::Home { repository } => Some(match domain.repository(repository) {
            Some(home) if parts > 0 => home_access_reason(&home.owner, &home.name, bytes),
            Some(_) | None => BriefRead::Failed,
        }),
        BriefSource::Ci { item, head } => brief_ci(domain, owner, item, head, parts, bytes, max_job_bytes, out),
        BriefSource::Reviews { item, head } => brief_reviews(domain, owner, item, head, parts, bytes, out),
        BriefSource::Pull { item, head } => brief_pull(domain, owner, item, head, parts, bytes, out),
    };
    if let Some(read) = read {
        crate::held::ready(domain, owner, read, out);
    }
}

fn next_remarks(fetch: &mut BriefFetch) -> Option<skein_forge::api::Read> {
    let index = usize::try_from(fetch.next_review).expect("u32 fits usize");
    let review = *fetch.review_ids.get(index)?;
    fetch.next_review = fetch.next_review.checked_add(1).expect("bounded review count");
    fetch.stage = BriefStage::Remarks;
    Some(skein_forge::api::Read::Remarks { number: fetch.item.number, review, page: 1 })
}

/// Append each failed status before asking for its pinned attempt's log.
fn next_failed_job(fetch: &mut BriefFetch) -> Option<skein_forge::api::Read> {
    let total = u32::try_from(fetch.checks.len()).expect("bounded check list");
    for index in fetch.next_check..total {
        let status = fetch.checks.get(usize::try_from(index).expect("u32 fits usize")).expect("index in check list");
        fetch.next_check = index.checked_add(1).expect("bounded check count");
        if status.check != skein_forge::api::Check::Failed {
            continue;
        }
        append(&mut fetch.words, b"Failed check: ");
        append(&mut fetch.words, &status.context);
        append(&mut fetch.words, b"\nDescription: ");
        append(&mut fetch.words, &status.description);
        append(&mut fetch.words, b"\nLink: ");
        append(&mut fetch.words, &status.url);
        append(&mut fetch.words, b"\n");
        match status.job {
            Some(attempt) if attempt.head == fetch.head.0 => {
                fetch.stage = BriefStage::Job;
                let count = total.max(1);
                let share = fetch
                    .bytes
                    .checked_div(count)
                    .expect("positive check count")
                    .checked_div(2)
                    .expect("positive divisor");
                return Some(skein_forge::api::Read::Job { attempt, max_bytes: share.max(1).min(fetch.max_job_bytes) });
            }
            Some(_) | None => append(&mut fetch.words, b"[no pinned job log for this head]\n"),
        }
    }
    None
}

pub(super) fn brief_reviews(
    domain: &mut Domain,
    owner: Token,
    item: BriefItem,
    head: BriefCommit,
    parts: u32,
    bytes: u32,
    out: &mut Queue<Request>,
) -> Option<BriefRead> {
    let Some(row) = domain.change_for_pull(item.repository, item.number) else {
        return Some(BriefRead::Failed);
    };
    if row.change.last_head != Some(head.0)
        || parts == 0
        || domain.brief_fetches.len() == domain.brief_fetches.capacity()
    {
        return Some(BriefRead::Failed);
    }
    let repository = row.repository;
    let mut words = BriefWords::with_capacity(bytes, false);
    for remark in &row.gate_remarks {
        if remark.head == head.0 {
            append(&mut words, b"Gate remarks: ");
            append(&mut words, &remark.words);
            append(&mut words, b"\n");
        }
    }
    let call = Token::new(owner.raw() | (1_u64 << 61_u32) | (1_u64 << 59_u32));
    let fetch = BriefFetch {
        owner,
        item,
        head,
        repository,
        parts,
        bytes,
        stage: BriefStage::Reviews,
        mode: PullMode::Other,
        base: None,
        comparison: None,
        changed: Box::new([]),
        review_ids: Box::new([]),
        next_review: 0,
        checks: Box::new([]),
        next_check: 0,
        max_job_bytes: 0,
        words,
    };
    domain.brief_fetches.insert(call, fetch).expect("brief read room checked");
    out.push(Request::BriefClient {
        event: skein_forge_client::Event::Read {
            owner: call,
            repository,
            read: skein_forge::api::Read::Reviews { number: item.number, page: 1 },
        },
    });
    None
}

#[expect(clippy::too_many_arguments, reason = "the pinned CI section carries its read and job-log bounds")]
pub(super) fn brief_ci(
    domain: &mut Domain,
    owner: Token,
    item: BriefItem,
    head: BriefCommit,
    parts: u32,
    bytes: u32,
    max_job_bytes: u32,
    out: &mut Queue<Request>,
) -> Option<BriefRead> {
    let Some(row) = domain.change_for_pull(item.repository, item.number) else {
        return Some(BriefRead::Failed);
    };
    let repairing = match row.change.state {
        change::State::Repairing { .. } => true,
        change::State::Producing { .. }
        | change::State::Opening { .. }
        | change::State::Recreating { .. }
        | change::State::Reopening { .. }
        | change::State::Checking { .. }
        | change::State::Queued { .. }
        | change::State::First { .. }
        | change::State::Updating { .. }
        | change::State::Gating { .. }
        | change::State::Resolving { .. }
        | change::State::Landing { .. }
        | change::State::Ended
        | change::State::Landed { .. }
        | change::State::Held { .. } => false,
    };
    if !repairing
        || row.change.last_head != Some(head.0)
        || parts == 0
        || domain.brief_fetches.len() == domain.brief_fetches.capacity()
    {
        return Some(BriefRead::Failed);
    }
    let repository = row.repository;
    let mut words = BriefWords::with_capacity(bytes, true);
    append(&mut words, b"Repair the failed check at change head ");
    append_hex(&mut words, &head.0[..8]);
    append(&mut words, b". Push a new head and wait for checks there.\n");
    let call = Token::new(owner.raw() | (1_u64 << 61_u32) | (1_u64 << 59_u32));
    let fetch = BriefFetch {
        owner,
        item,
        head,
        repository,
        parts,
        bytes,
        stage: BriefStage::Checks,
        mode: PullMode::Other,
        base: None,
        comparison: None,
        changed: Box::new([]),
        review_ids: Box::new([]),
        next_review: 0,
        checks: Box::new([]),
        next_check: 0,
        max_job_bytes,
        words,
    };
    domain.brief_fetches.insert(call, fetch).expect("brief read room checked");
    out.push(Request::BriefClient {
        event: skein_forge_client::Event::Read {
            owner: call,
            repository,
            read: skein_forge::api::Read::Checks { commit: head.0 },
        },
    });
    None
}

fn decimal(number: u64) -> Box<[u8]> {
    let mut digits = List::with_capacity(20);
    let mut value = number;
    for _ in 0_u32..20_u32 {
        digits
            .push(b'0'.checked_add(u8::try_from(value % 10).expect("decimal digit")).expect("decimal ascii digit"))
            .expect("u64 decimal width");
        value /= 10;
        if value == 0 {
            break;
        }
    }
    let mut forward = List::with_capacity(20);
    for at in (0..digits.len()).rev() {
        forward.push(*digits.get(at).expect("measured decimal digit")).expect("same width");
    }
    forward.into_boxed()
}
