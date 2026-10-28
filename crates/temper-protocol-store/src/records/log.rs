//! Family 27's total translations (protocol/store.md, 2).
use super::{Bounds, FITS, names, topics, w};
use crate::Undecodable;
use alloc::boxed::Box;
use skein_lib::{List, Reader, Wall, Writer};
use temper_engine_domain_forge as d;

pub(super) fn encode(row: &d::LandingLog) -> w::LandingLog {
    let mut landings = List::with_capacity(u32::try_from(row.landings.len()).expect(FITS));
    for entry in &row.landings {
        let mut changes = List::with_capacity(u32::try_from(entry.changes.len()).expect(FITS));
        for change in &entry.changes {
            changes
                .push(
                    w::LoggedChange::new(&w::CEILINGS, w::LoggedChangeParts { task: change.0, pull: change.1 })
                        .expect(FITS),
                )
                .expect(FITS);
        }
        let mut verdicts = List::with_capacity(u32::try_from(entry.verdicts.len()).expect(FITS));
        for mark in &entry.verdicts {
            let verdict = match mark.verdict {
                d::Verdict::Verifying => w::LoggedVerdict::Verifying,
                d::Verdict::Cleared => w::LoggedVerdict::Cleared,
                d::Verdict::Red { tests } => {
                    w::LoggedVerdict::Red(w::LoggedRed::new(&w::CEILINGS, w::LoggedRedParts { tests }).expect(FITS))
                }
            };
            verdicts
                .push(w::Mark::new(&w::CEILINGS, w::MarkParts { tier: mark.tier.clone(), verdict }).expect(FITS))
                .expect(FITS);
        }
        let seen = entry.seen.as_nanos();
        let mut trailers = List::with_capacity(u32::try_from(entry.trailers.len()).expect(FITS));
        for line in &entry.trailers {
            trailers.push(line.clone()).expect(FITS);
        }
        landings
            .push(
                w::Logged::new(
                    &w::CEILINGS,
                    w::LoggedParts { trailers, cut: entry.cut, commit: entry.commit, seen, changes, verdicts },
                )
                .expect(FITS),
            )
            .expect(FITS);
    }
    let mut pending = List::with_capacity(u32::try_from(row.pending.len()).expect(FITS));
    for read in &row.pending {
        pending.push(encode_pending(read)).expect(FITS);
    }
    w::LandingLog::new(
        &w::CEILINGS,
        w::LandingLogParts { pending, repository: names::encode_repository(row.repository), landings },
    )
    .expect(FITS)
}

pub(super) fn decode(row: w::LandingLog, bounds: &Bounds) -> Result<d::LandingLog, Undecodable> {
    let w::LandingLogParts { pending, repository, landings } = row.into_parts();
    let mut decoded = List::with_capacity(landings.len());
    for entry in landings.into_boxed() {
        let w::LoggedParts { trailers, cut, commit, seen, changes, verdicts } = entry.into_parts();
        let mut marks = List::with_capacity(verdicts.len());
        for mark in verdicts.into_boxed() {
            let w::MarkParts { tier, verdict } = mark.into_parts();
            if tier.is_empty() {
                return Err(Undecodable);
            }
            for byte in &tier {
                if !byte.is_ascii_alphanumeric() && *byte != b'-' && *byte != b'_' {
                    return Err(Undecodable);
                }
            }
            let verdict = match verdict {
                w::LoggedVerdict::Verifying => d::Verdict::Verifying,
                w::LoggedVerdict::Cleared => d::Verdict::Cleared,
                w::LoggedVerdict::Red(red) => d::Verdict::Red { tests: *red.tests() },
            };
            marks.push(d::Mark { tier, verdict }).expect(FITS);
        }
        let mut members = List::with_capacity(changes.len());
        for member in changes.into_boxed() {
            let w::LoggedChangeParts { task, pull } = member.into_parts();
            members.push((task, pull)).expect(FITS);
        }
        decoded
            .push(d::Logged {
                trailers: trailers.into_boxed(),
                cut,
                commit,
                seen: Wall::from_nanos(seen),
                changes: members.into_boxed(),
                verdicts: marks.into_boxed(),
            })
            .expect(FITS);
    }
    let mut reads = List::with_capacity(pending.len());
    for read in pending.into_boxed() {
        reads.push(decode_pending(read, bounds)?).expect(FITS);
    }
    Ok(d::LandingLog {
        pending: reads.into_boxed(),
        repository: names::decode_repository(repository),
        landings: decoded.into_boxed(),
    })
}

fn lines(values: &[Box<[u8]>]) -> List<Box<[u8]>> {
    let mut lines = List::with_capacity(u32::try_from(values.len()).expect(FITS));
    for value in values {
        lines.push(value.clone()).expect(FITS);
    }
    lines
}
#[expect(clippy::manual_map, reason = "step translations use explicit matches")]
fn encode_pending(read: &d::LandingNews) -> w::PendingLanding {
    let files = match &read.files {
        Some(files) => Some(encode_files(files)),
        None => None,
    };
    w::PendingLanding::new(
        &w::CEILINGS,
        w::PendingLandingParts {
            topic: topics::encode_topic(&read.topic),
            before: read.before,
            after: read.after,
            files,
            trailers: lines(&read.trailers),
            page: read.page,
            cut: read.cut,
        },
    )
    .expect(FITS)
}
fn decode_pending(read: w::PendingLanding, bounds: &Bounds) -> Result<d::LandingNews, Undecodable> {
    let w::PendingLandingParts { topic, before, after, files, trailers, page, cut } = read.into_parts();
    let topic = topics::decode_topic(topic);
    match &topic {
        d::Topic::Landings { .. } => {}
        d::Topic::Ci { .. } | d::Topic::Pull { .. } | d::Topic::Participation { .. } => return Err(Undecodable),
    }
    let files = match files {
        Some(files) => Some(decode_files(&files, bounds)?),
        None => None,
    };
    Ok(d::LandingNews { topic, before, after, files, trailers: trailers.into_boxed(), page, cut })
}

// Compare's paths share one answer-byte allowance; framing them as pieces
// preserves that total bound instead of multiplying it by the row count.
fn encode_files(files: &[Box<[u8]>]) -> Box<[u8]> {
    let mut bytes = 0_usize;
    for file in files {
        bytes = bytes.checked_add(file.len()).expect(FITS).checked_add(4).expect(FITS);
    }
    let mut writer = Writer::new(bytes);
    for file in files {
        skein_codec::piece::write(&mut writer, file).expect(FITS);
    }
    writer.finish()
}
fn decode_files(bytes: &[u8], bounds: &Bounds) -> Result<Box<[Box<[u8]>]>, Undecodable> {
    let mut reader = Reader::new(bytes);
    let mut files = List::with_capacity(bounds.forge().client.rows);
    for _ in 0..=bounds.forge().client.rows {
        let Some(file) = skein_codec::piece::split(&mut reader).or(Err(Undecodable))? else {
            return Ok(files.into_boxed());
        };
        files.push(file).or(Err(Undecodable))?;
    }
    Err(Undecodable)
}
