//! Family 27's total translations (protocol/store.md, 2).
use super::{FITS, names, w};
use crate::Undecodable;
use skein_lib::{List, Wall};
use temper_engine_domain_forge as d;

pub(super) fn encode(row: &d::LandingLog) -> w::LandingLog {
    let mut landings = List::with_capacity(u32::try_from(row.landings.len()).expect(FITS));
    for entry in &row.landings {
        let mut changes = List::with_capacity(u32::try_from(entry.changes.len()).expect(FITS));
        for change in &entry.changes {
            changes.push(*change).expect(FITS);
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
        landings
            .push(
                w::Logged::new(&w::CEILINGS, w::LoggedParts { commit: entry.commit, seen, changes, verdicts })
                    .expect(FITS),
            )
            .expect(FITS);
    }
    w::LandingLog::new(
        &w::CEILINGS,
        w::LandingLogParts { repository: names::encode_repository(row.repository), landings },
    )
    .expect(FITS)
}

pub(super) fn decode(row: w::LandingLog) -> Result<d::LandingLog, Undecodable> {
    let w::LandingLogParts { repository, landings } = row.into_parts();
    let mut decoded = List::with_capacity(landings.len());
    for entry in landings.into_boxed() {
        let w::LoggedParts { commit, seen, changes, verdicts } = entry.into_parts();
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
        decoded
            .push(d::Logged {
                commit,
                seen: Wall::from_nanos(seen),
                changes: changes.into_boxed(),
                verdicts: marks.into_boxed(),
            })
            .expect(FITS);
    }
    Ok(d::LandingLog { repository: names::decode_repository(repository), landings: decoded.into_boxed() })
}
